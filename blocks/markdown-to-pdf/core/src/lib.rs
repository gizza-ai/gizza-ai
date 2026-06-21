//! gizza-ai/markdown-to-pdf core — convert a Markdown document into a formatted PDF.
//!
//! Pure-Rust: `pulldown-cmark` parses CommonMark (+ tables, strikethrough, task
//! lists), and `lopdf` writes the PDF. No font embedding — the built-in base-14
//! fonts (Helvetica family for body text, Courier for code) keep the PDF tiny and
//! the layout deterministic. US Letter pages; content flows across as many pages as
//! needed.
//!
//! Supported block elements: headings (H1-H6, scaled + bold), paragraphs, ordered
//! and unordered lists (nested, with bullets / numbers), fenced & indented code
//! blocks (monospace), block quotes (indented), horizontal rules, and table rows
//! flattened to text. Inline: bold, italic, bold-italic, and inline `code` switch
//! the active font; strikethrough and links render their text. Images render alt
//! text.

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

// US Letter, the default page size.
const LETTER_W: f64 = 612.0;
const LETTER_H: f64 = 792.0;
// ISO A4 in points (210 x 297 mm).
const A4_W: f64 = 595.0;
const A4_H: f64 = 842.0;
const LINE_FACTOR: f64 = 1.35;
const PARA_GAP: f64 = 0.5; // extra blank space (in line-heights) after a block
const TAB_WIDTH: usize = 4;
const INDENT_PT: f64 = 18.0; // per nesting level / quote indent

/// The base-14 fonts we map onto. Index = resource name F1..F5.
#[derive(Clone, Copy, PartialEq, Debug)]
enum FontKind {
    Regular,    // Helvetica
    Bold,       // Helvetica-Bold
    Italic,     // Helvetica-Oblique
    BoldItalic, // Helvetica-BoldOblique
    Mono,       // Courier
}

impl FontKind {
    fn resource(self) -> &'static str {
        match self {
            FontKind::Regular => "F1",
            FontKind::Bold => "F2",
            FontKind::Italic => "F3",
            FontKind::BoldItalic => "F4",
            FontKind::Mono => "F5",
        }
    }
    fn base_font(self) -> &'static str {
        match self {
            FontKind::Regular => "Helvetica",
            FontKind::Bold => "Helvetica-Bold",
            FontKind::Italic => "Helvetica-Oblique",
            FontKind::BoldItalic => "Helvetica-BoldOblique",
            FontKind::Mono => "Courier",
        }
    }
    /// Average glyph advance as a fraction of the em (used for word-wrap width
    /// estimation). Courier is exactly 0.6; the Helvetica family averages ~0.5.
    fn char_em(self) -> f64 {
        match self {
            FontKind::Mono => 0.6,
            _ => 0.5,
        }
    }
    /// Resolve the active font from the bold / italic / code flags.
    fn from_flags(bold: bool, italic: bool, code: bool) -> FontKind {
        if code {
            FontKind::Mono
        } else {
            match (bold, italic) {
                (true, true) => FontKind::BoldItalic,
                (true, false) => FontKind::Bold,
                (false, true) => FontKind::Italic,
                (false, false) => FontKind::Regular,
            }
        }
    }
}

/// A run of text with a single font, the atom of a laid-out line.
#[derive(Clone, Debug)]
struct Run {
    text: String,
    font: FontKind,
    size: f64,
}

/// A laid-out line: a list of runs plus the left indent (points) and height.
#[derive(Clone, Debug, Default)]
struct Line {
    runs: Vec<Run>,
    indent: f64,
    height: f64,
}

impl Line {
    fn is_blank(&self) -> bool {
        self.runs.iter().all(|r| r.text.trim().is_empty())
    }
}

/// Escape a string for a PDF literal and fold to Latin-1.
fn pdf_escape(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 2);
    for ch in s.chars() {
        let b = if (ch as u32) <= 0xFF { ch as u8 } else { b'?' };
        match b {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'(' => out.extend_from_slice(b"\\("),
            b')' => out.extend_from_slice(b"\\)"),
            _ => out.push(b),
        }
    }
    out
}

fn heading_size(level: HeadingLevel, base: f64) -> f64 {
    let scale = match level {
        HeadingLevel::H1 => 2.0,
        HeadingLevel::H2 => 1.6,
        HeadingLevel::H3 => 1.3,
        HeadingLevel::H4 => 1.15,
        HeadingLevel::H5 => 1.0,
        HeadingLevel::H6 => 0.9,
    };
    base * scale
}

/// A blank vertical-gap line (PARA_GAP of a line-height).
fn blank_line(base: f64) -> Line {
    Line { runs: Vec::new(), indent: 0.0, height: base * LINE_FACTOR * PARA_GAP }
}

/// A horizontal rule rendered as a row of underscores.
fn rule_line(base: f64, text_w: f64) -> Line {
    let n = (text_w / (base * 0.5)).floor().max(1.0) as usize;
    Line {
        runs: vec![Run { text: "_".repeat(n), font: FontKind::Regular, size: base * 0.8 }],
        indent: 0.0,
        height: base * LINE_FACTOR,
    }
}

/// Emit `buf` as a single verbatim line (no wrapping) at `indent`, then clear it.
fn emit_verbatim(buf: &mut Vec<Run>, lines: &mut Vec<Line>, indent: f64) {
    let runs = std::mem::take(buf);
    let height = runs.iter().map(|r| r.size).fold(0.0_f64, f64::max).max(1.0) * LINE_FACTOR;
    lines.push(Line { runs, indent, height });
}

/// Word-wrap a run-stream into lines fitting `avail` points wide, at `indent`.
fn wrap_runs(buf: &[Run], indent: f64, avail: f64, lines: &mut Vec<Line>) {
    struct Word {
        text: String,
        font: FontKind,
        size: f64,
    }
    let mut words: Vec<Word> = Vec::new();
    for r in buf {
        let mut first = true;
        let only_spaces = !r.text.is_empty() && r.text.chars().all(|c| c == ' ');
        for part in r.text.split(' ') {
            if part.is_empty() {
                first = false;
                continue;
            }
            let text = if first { part.to_string() } else { format!(" {part}") };
            words.push(Word { text, font: r.font, size: r.size });
            first = false;
        }
        if only_spaces {
            words.push(Word { text: " ".to_string(), font: r.font, size: r.size });
        }
    }

    let word_w = |w: &Word| w.text.chars().count() as f64 * w.size * w.font.char_em();

    let mut cur: Vec<Run> = Vec::new();
    let mut cur_w = 0.0;
    let mut max_size = 0.0_f64;

    let push_line = |cur: &mut Vec<Run>, max_size: &mut f64, lines: &mut Vec<Line>| {
        if cur.is_empty() {
            return;
        }
        let height = (*max_size).max(1.0) * LINE_FACTOR;
        lines.push(Line { runs: std::mem::take(cur), indent, height });
        *max_size = 0.0;
    };

    for w in &words {
        let ww = word_w(w);
        if cur_w + ww > avail && !cur.is_empty() {
            push_line(&mut cur, &mut max_size, lines);
            cur_w = 0.0;
            let trimmed = w.text.trim_start().to_string();
            let tw = trimmed.chars().count() as f64 * w.size * w.font.char_em();
            cur.push(Run { text: trimmed, font: w.font, size: w.size });
            cur_w += tw;
            max_size = max_size.max(w.size);
            continue;
        }
        cur.push(Run { text: w.text.clone(), font: w.font, size: w.size });
        cur_w += ww;
        max_size = max_size.max(w.size);
    }
    push_line(&mut cur, &mut max_size, lines);
}

/// Build the flat list of laid-out lines from the Markdown source.
fn layout(markdown: &str, base: f64, text_w: f64) -> Vec<Line> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(markdown, opts);

    let mut lines: Vec<Line> = Vec::new();

    let mut bold = false;
    let mut italic = false;
    let mut code = false;

    let mut buf: Vec<Run> = Vec::new();
    let mut cur_size = base;

    let mut indent_level: f64 = 0.0;
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut in_code_block = false;
    let mut quote_depth: f64 = 0.0;

    let flush = |buf: &mut Vec<Run>, lines: &mut Vec<Line>, indent: f64, gap: bool| {
        if buf.is_empty() {
            return;
        }
        let avail = (text_w - indent).max(base * 4.0);
        wrap_runs(buf, indent, avail, lines);
        buf.clear();
        if gap {
            lines.push(blank_line(base));
        }
    };

    for ev in parser {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    cur_size = heading_size(level, base);
                    bold = true;
                }
                Tag::Paragraph => {}
                Tag::CodeBlock(_) => {
                    in_code_block = true;
                    code = true;
                    cur_size = base * 0.92;
                }
                Tag::List(start) => {
                    list_stack.push(start);
                    indent_level += 1.0;
                }
                Tag::Item => {
                    let indent = indent_level * INDENT_PT + quote_depth * INDENT_PT;
                    let _ = indent;
                    let marker = match list_stack.last_mut() {
                        Some(Some(n)) => {
                            let m = format!("{n}. ");
                            *n += 1;
                            m
                        }
                        _ => "- ".to_string(),
                    };
                    buf.push(Run { text: marker, font: FontKind::Regular, size: base });
                }
                Tag::BlockQuote(_) => {
                    quote_depth += 1.0;
                }
                Tag::Emphasis => italic = true,
                Tag::Strong => bold = true,
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    flush(&mut buf, &mut lines, quote_depth * INDENT_PT, true);
                    bold = false;
                    cur_size = base;
                }
                TagEnd::Paragraph => {
                    let indent = indent_level * INDENT_PT + quote_depth * INDENT_PT;
                    flush(&mut buf, &mut lines, indent, true);
                }
                TagEnd::CodeBlock => {
                    let indent = indent_level * INDENT_PT + quote_depth * INDENT_PT + INDENT_PT;
                    flush(&mut buf, &mut lines, indent, true);
                    in_code_block = false;
                    code = false;
                    cur_size = base;
                }
                TagEnd::List(_) => {
                    list_stack.pop();
                    indent_level = (indent_level - 1.0).max(0.0);
                    if indent_level == 0.0 && quote_depth == 0.0 {
                        lines.push(blank_line(base));
                    }
                }
                TagEnd::Item => {
                    let indent = indent_level * INDENT_PT + quote_depth * INDENT_PT;
                    flush(&mut buf, &mut lines, indent, false);
                }
                TagEnd::BlockQuote(_) => {
                    quote_depth = (quote_depth - 1.0).max(0.0);
                }
                TagEnd::Emphasis => italic = false,
                TagEnd::Strong => bold = false,
                TagEnd::TableHead | TagEnd::TableRow => {
                    let indent = quote_depth * INDENT_PT;
                    flush(&mut buf, &mut lines, indent, false);
                }
                TagEnd::TableCell => {
                    buf.push(Run { text: " | ".to_string(), font: FontKind::Regular, size: base });
                }
                TagEnd::Table => {
                    lines.push(blank_line(base));
                }
                _ => {}
            },
            Event::Text(t) => {
                let font = FontKind::from_flags(bold, italic, code);
                if in_code_block {
                    let expanded = t.replace('\t', &" ".repeat(TAB_WIDTH));
                    let mut parts = expanded.split('\n').peekable();
                    while let Some(seg) = parts.next() {
                        buf.push(Run { text: seg.to_string(), font, size: cur_size });
                        if parts.peek().is_some() {
                            let indent = indent_level * INDENT_PT + quote_depth * INDENT_PT + INDENT_PT;
                            emit_verbatim(&mut buf, &mut lines, indent);
                        }
                    }
                } else {
                    buf.push(Run { text: t.to_string(), font, size: cur_size });
                }
            }
            Event::Code(t) => {
                buf.push(Run {
                    text: t.to_string(),
                    font: FontKind::Mono,
                    size: cur_size * 0.95,
                });
            }
            Event::SoftBreak => {
                buf.push(Run { text: " ".to_string(), font: FontKind::Regular, size: cur_size });
            }
            Event::HardBreak => {
                let indent = indent_level * INDENT_PT + quote_depth * INDENT_PT;
                emit_verbatim(&mut buf, &mut lines, indent);
            }
            Event::Rule => {
                flush(&mut buf, &mut lines, 0.0, false);
                lines.push(rule_line(base, text_w));
                lines.push(blank_line(base));
            }
            Event::TaskListMarker(done) => {
                let mark = if done { "[x] " } else { "[ ] " };
                buf.push(Run { text: mark.to_string(), font: FontKind::Mono, size: base });
            }
            Event::FootnoteReference(name) => {
                buf.push(Run { text: format!("[^{name}]"), font: FontKind::Regular, size: cur_size });
            }
            _ => {}
        }
    }
    flush(&mut buf, &mut lines, quote_depth * INDENT_PT, false);

    while lines.first().map(|l| l.is_blank() && l.runs.is_empty()).unwrap_or(false) {
        lines.remove(0);
    }
    while lines.last().map(|l| l.is_blank() && l.runs.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines
}

/// Convert a Markdown document into a paginated PDF. `font_size` is the base body
/// size in points; `margin` is the page margin in points. `page_size` is the
/// (case-insensitive) page name: "letter" (default, US Letter) or "a4".
pub fn markdown_to_pdf(
    markdown: &str,
    font_size: f64,
    margin: f64,
    page_size: &str,
    page_numbers: bool,
) -> Result<Vec<u8>, String> {
    if !font_size.is_finite() || font_size < 6.0 || font_size > 48.0 {
        return Err("font_size must be between 6 and 48 points".into());
    }
    let (page_w, page_h) = match page_size.trim().to_ascii_lowercase().as_str() {
        "" | "letter" | "us-letter" | "us letter" => (LETTER_W, LETTER_H),
        "a4" => (A4_W, A4_H),
        other => return Err(format!("unknown page_size '{other}' (use 'letter' or 'a4')")),
    };
    if !margin.is_finite() || margin < 0.0 || margin * 2.0 >= page_h.min(page_w) {
        return Err("margin is too large for the page".into());
    }

    let text_w = page_w - 2.0 * margin;
    let text_h = page_h - 2.0 * margin;

    let mut lines = layout(markdown, font_size, text_w);
    if lines.is_empty() {
        lines.push(Line {
            runs: vec![Run { text: String::new(), font: FontKind::Regular, size: font_size }],
            indent: 0.0,
            height: font_size * LINE_FACTOR,
        });
    }

    let mut pages: Vec<Vec<Line>> = Vec::new();
    let mut cur: Vec<Line> = Vec::new();
    let mut used = 0.0;
    for line in lines {
        let h = line.height;
        if used + h > text_h && !cur.is_empty() {
            pages.push(std::mem::take(&mut cur));
            used = 0.0;
        }
        cur.push(line);
        used += h;
    }
    if !cur.is_empty() {
        pages.push(cur);
    }
    if pages.is_empty() {
        pages.push(Vec::new());
    }

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let mk_font = |doc: &mut Document, kind: FontKind| {
        doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => kind.base_font(),
            "Encoding" => "WinAnsiEncoding",
        })
    };
    let f1 = mk_font(&mut doc, FontKind::Regular);
    let f2 = mk_font(&mut doc, FontKind::Bold);
    let f3 = mk_font(&mut doc, FontKind::Italic);
    let f4 = mk_font(&mut doc, FontKind::BoldItalic);
    let f5 = mk_font(&mut doc, FontKind::Mono);
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => f1, "F2" => f2, "F3" => f3, "F4" => f4, "F5" => f5,
        },
    });

    let total_pages = pages.len();
    let mut page_ids: Vec<Object> = Vec::new();
    for (page_idx, page_lines) in pages.iter().enumerate() {
        let mut ops: Vec<Operation> = Vec::new();
        ops.push(Operation::new("BT", vec![]));
        let mut active: Option<(FontKind, f64)> = None;
        let mut y = page_h - margin;
        for line in page_lines {
            y -= line.height;
            if line.runs.is_empty() {
                continue;
            }
            let baseline = y + line.height * 0.25;
            let mut x = margin + line.indent;
            for run in &line.runs {
                if run.text.is_empty() {
                    continue;
                }
                if active != Some((run.font, run.size)) {
                    ops.push(Operation::new(
                        "Tf",
                        vec![run.font.resource().into(), run.size.into()],
                    ));
                    active = Some((run.font, run.size));
                }
                ops.push(Operation::new(
                    "Tm",
                    vec![1.into(), 0.into(), 0.into(), 1.into(), x.into(), baseline.into()],
                ));
                ops.push(Operation::new(
                    "Tj",
                    vec![Object::String(pdf_escape(&run.text), lopdf::StringFormat::Literal)],
                ));
                x += run.text.chars().count() as f64 * run.size * run.font.char_em();
            }
        }
        // Centered "n / total" footer in the bottom margin.
        if page_numbers {
            let foot_size = (font_size * 0.8).max(7.0);
            let label = format!("{} / {}", page_idx + 1, total_pages);
            let label_w = label.chars().count() as f64 * foot_size * FontKind::Regular.char_em();
            let fx = (page_w - label_w) / 2.0;
            let fy = (margin - foot_size * 1.5).max(foot_size);
            ops.push(Operation::new("Tf", vec![FontKind::Regular.resource().into(), foot_size.into()]));
            ops.push(Operation::new(
                "Tm",
                vec![1.into(), 0.into(), 0.into(), 1.into(), fx.into(), fy.into()],
            ));
            ops.push(Operation::new(
                "Tj",
                vec![Object::String(pdf_escape(&label), lopdf::StringFormat::Literal)],
            ));
        }
        ops.push(Operation::new("ET", vec![]));

        let content = Content { operations: ops };
        let content_id = doc.add_object(Stream::new(
            dictionary! {},
            content.encode().map_err(|e| format!("content encode: {e}"))?,
        ));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), page_w.into(), page_h.into()],
        });
        page_ids.push(page_id.into());
    }

    let count = page_ids.len() as i64;
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids,
        "Count" => count,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("failed to write PDF: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_count(pdf: &[u8]) -> usize {
        let doc = Document::load_mem(pdf).unwrap();
        doc.page_iter().count()
    }

    #[test]
    fn makes_a_valid_pdf() {
        let pdf = markdown_to_pdf("# Hello\n\nWorld with **bold** and *italic*.", 12.0, 72.0, "letter", false).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        assert_eq!(page_count(&pdf), 1);
    }

    #[test]
    fn handles_lists_and_code() {
        let md = "## List\n\n- one\n- two\n  - nested\n\n```\nlet x = 1;\n```\n\n> a quote\n";
        let pdf = markdown_to_pdf(md, 11.0, 72.0, "letter", false).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        assert!(page_count(&pdf) >= 1);
    }

    #[test]
    fn paginates_long_documents() {
        let md = (0..400).map(|i| format!("Paragraph number {i} with some words.\n\n")).collect::<String>();
        let pdf = markdown_to_pdf(&md, 11.0, 72.0, "letter", false).unwrap();
        assert!(page_count(&pdf) > 1, "400 paragraphs should span multiple pages");
    }

    #[test]
    fn empty_input_still_makes_a_page() {
        let pdf = markdown_to_pdf("", 11.0, 72.0, "letter", false).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        assert_eq!(page_count(&pdf), 1);
    }

    #[test]
    fn handles_tables_and_rules() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |\n\n---\n\nAfter the rule.";
        let pdf = markdown_to_pdf(md, 11.0, 72.0, "letter", false).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
    }

    #[test]
    fn a4_page_size_uses_a4_mediabox() {
        let pdf = markdown_to_pdf("# A4\n\nbody", 11.0, 72.0, "A4", false).unwrap();
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.contains("842"), "A4 MediaBox height (842) should appear");
        // The default (letter) should NOT use the A4 height.
        let letter = markdown_to_pdf("# L\n\nbody", 11.0, 72.0, "letter", false).unwrap();
        assert!(String::from_utf8_lossy(&letter).contains("792"), "letter height 792");
    }

    #[test]
    fn page_numbers_render_a_footer() {
        // Two-page doc with page numbers on; the "/" of "n / total" must appear.
        let md = (0..120).map(|i| format!("Line {i}.\n\n")).collect::<String>();
        let with = markdown_to_pdf(&md, 11.0, 72.0, "letter", true).unwrap();
        let without = markdown_to_pdf(&md, 11.0, 72.0, "letter", false).unwrap();
        assert!(page_count(&with) >= 2);
        // The footer adds the "n / N" label runs, so the file with footers is larger.
        assert!(with.len() > without.len(), "footer should add bytes");
    }

    #[test]
    fn errors_on_bad_params() {
        assert!(markdown_to_pdf("x", 2.0, 72.0, "letter", false).is_err()); // font too small
        assert!(markdown_to_pdf("x", 60.0, 72.0, "letter", false).is_err()); // font too large
        assert!(markdown_to_pdf("x", 12.0, 400.0, "letter", false).is_err()); // margin too big
        assert!(markdown_to_pdf("x", 12.0, 72.0, "legal", false).is_err()); // unknown page size
    }

    #[test]
    fn wraps_long_paragraphs() {
        let md = "word ".repeat(300);
        let pdf = markdown_to_pdf(&md, 12.0, 72.0, "letter", false).unwrap();
        assert!(page_count(&pdf) >= 1);
    }
}
