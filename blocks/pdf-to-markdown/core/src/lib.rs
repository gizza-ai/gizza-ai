//! gizza-ai/pdf-to-markdown core — convert a PDF's text layer into structured
//! Markdown (headings, lists, paragraphs). Pure Rust, no wafer/wasm-bindgen
//! deps, so it compiles natively for unit tests and to `wasm32-wasip1` (the
//! `wafer build` target) for the chat block.
//!
//! ## Why this is not `pdf-extract-text`
//!
//! `pdf-extract-text` returns the flat selectable-text layer (lopdf's
//! `extract_text`). This block re-implements extraction on top of lopdf's
//! *public* API (`get_page_fonts` + `get_font_encoding` + `decode_text` +
//! `get_and_decode_page_content`) so it can additionally track, per text run,
//! the active FONT SIZE (`Tf`) and the vertical TEXT POSITION
//! (`Tm`/`Td`/`TD`/`T*`/`'`/`"`). lopdf's own extractor discards both. From
//! those two signals it reconstructs:
//!   - **headings** (`#`/`##`/`###` …) from document-wide font-size statistics;
//!   - **lines** from vertical text moves (lopdf only inserts a newline at `ET`);
//!   - **paragraphs** by grouping lines whose vertical gap is small, joining
//!     wrapped lines and de-hyphenating words split across a line break;
//!   - **list items** from a line-leading bullet (•, ‣, ◦, –, —, -, *) or a
//!     numeric marker (`1.`, `2)`).
//!
//! It also handles the `'` and `"` show-text operators, which lopdf's extractor
//! drops entirely (they carry text on real, older PDFs).
//!
//! ## Limits (the text layer only)
//!
//! Extracts the embedded selectable text ONLY — it does not OCR scanned /
//! image-only PDFs (those yield empty Markdown). Tables, inline bold/italic,
//! monospace→code-fence detection, multi-column reading order, and header/footer
//! removal are out of scope (text is emitted in content-stream order).

use lopdf::content::Content;
use lopdf::{Document, Object};

/// Cap on pages iterated when converting "all" pages — guards a pathological
/// document that claims an absurd page count.
const MAX_PAGES: usize = 10_000;

/// How to divide consecutive pages in the Markdown output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSeparator {
    /// A Markdown horizontal rule (`---`) between pages.
    Rule,
    /// Just a blank line between pages.
    Blank,
}

impl PageSeparator {
    /// Parse the descriptor/CLI string form. Returns `None` for an unknown value.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "rule" => Some(PageSeparator::Rule),
            "blank" => Some(PageSeparator::Blank),
            _ => None,
        }
    }
    fn joiner(self) -> &'static str {
        match self {
            PageSeparator::Rule => "\n\n---\n\n",
            PageSeparator::Blank => "\n\n",
        }
    }
}

/// Conversion knobs. See [`Options::default`] for the defaults.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Optional 1-based page number. `None` converts every page.
    pub page: Option<usize>,
    /// How to divide pages in the output.
    pub page_separator: PageSeparator,
    /// Convert leading bullet/number markers into Markdown list items.
    pub detect_lists: bool,
    /// Rejoin words split by a trailing hyphen at a line break.
    pub dehyphenate: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            page: None,
            page_separator: PageSeparator::Rule,
            detect_lists: true,
            dehyphenate: true,
        }
    }
}

/// Result of a conversion: the Markdown plus the count of text runs whose font
/// encoding could not be decoded (a non-zero count means the Markdown is
/// partial).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    /// The structured Markdown.
    pub markdown: String,
    /// Text runs skipped because their font encoding failed to parse.
    pub dropped_runs: usize,
}

/// One reconstructed visual line: its joined text, the largest font size seen
/// on it, and the vertical gap to the previous line on the same page.
#[derive(Debug, Clone)]
struct Line {
    text: String,
    size: f64,
    /// `prev_line_y - this_line_y` (≈ leading for a wrapped line; larger for a
    /// paragraph break; negative when the layout jumps back up a column).
    gap_before: f64,
}

/// A positioned text run captured during the content-stream walk.
struct Piece {
    y: f64,
    size: f64,
    text: String,
}

/// Convert a PDF's text layer to structured Markdown.
///
/// - `bytes` — the raw PDF file.
/// - `opts` — see [`Options`].
///
/// Returns `Err` when the bytes don't parse as a PDF or when `opts.page` is out
/// of range. A PDF with no decodable text returns `Ok` with empty `markdown`.
pub fn to_markdown(bytes: &[u8], opts: &Options) -> Result<Conversion, String> {
    let doc = Document::load_mem(bytes).map_err(|e| format!("failed to parse PDF: {e}"))?;

    let mut page_numbers: Vec<u32> = doc.get_pages().keys().copied().collect();
    page_numbers.sort_unstable();
    if page_numbers.is_empty() {
        return Err("PDF has no pages".to_string());
    }
    if page_numbers.len() > MAX_PAGES {
        return Err(format!(
            "PDF has too many pages: {} (cap {MAX_PAGES})",
            page_numbers.len()
        ));
    }

    let selected: Vec<u32> = match opts.page {
        Some(p) => {
            let p_u32 = u32::try_from(p)
                .map_err(|_| format!("page {p} out of range (1..={})", page_numbers.len()))?;
            if !page_numbers.contains(&p_u32) {
                return Err(format!("page {p} out of range (1..={})", page_numbers.len()));
            }
            vec![p_u32]
        }
        None => page_numbers,
    };

    // Phase 1: extract reconstructed lines per page, plus font-size statistics
    // gathered document-wide (across all selected pages) so heading levels are
    // consistent across the whole document.
    let mut dropped_runs = 0usize;
    let mut pages_lines: Vec<Vec<Line>> = Vec::with_capacity(selected.len());
    for n in &selected {
        let lines = extract_page_lines(&doc, *n, &mut dropped_runs);
        pages_lines.push(lines);
    }

    let body = body_size(&pages_lines);
    let heading_sizes = heading_size_ranking(&pages_lines, body);

    // Phase 2: render each page's lines to Markdown, join with the separator.
    let rendered: Vec<String> = pages_lines
        .iter()
        .map(|lines| render_page(lines, body, &heading_sizes, opts))
        .filter(|s| !s.is_empty())
        .collect();

    Ok(Conversion {
        markdown: rendered.join(opts.page_separator.joiner()),
        dropped_runs,
    })
}

/// Read a PDF number `Object` (Integer or Real) as `f64`.
fn num(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

/// Walk one page's content stream, tracking the active font size + vertical text
/// position, and return the reconstructed visual lines. Increments `dropped` for
/// each text run whose font encoding can't be decoded.
fn extract_page_lines(doc: &Document, page_number: u32, dropped: &mut usize) -> Vec<Line> {
    let pages = doc.get_pages();
    let Some(&page_id) = pages.get(&page_number) else {
        return Vec::new();
    };

    // name (without leading slash) -> text encoding, for decode_text.
    let encodings: std::collections::BTreeMap<Vec<u8>, lopdf::Encoding> =
        match doc.get_page_fonts(page_id) {
            Ok(fonts) => fonts
                .into_iter()
                .filter_map(|(name, font)| match font.get_font_encoding(doc) {
                    Ok(enc) => Some((name, enc)),
                    Err(_) => {
                        *dropped += 1;
                        None
                    }
                })
                .collect(),
            Err(_) => std::collections::BTreeMap::new(),
        };

    let content: Content<Vec<lopdf::content::Operation>> =
        match doc.get_and_decode_page_content(page_id) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

    // Text state. `tlm_y` tracks the text-LINE-matrix origin; positioning
    // operators move it. We assume the (near-universal) identity text matrix
    // orientation, which is all that heading/line reconstruction needs.
    let mut cur_size: f64 = 0.0;
    let mut cur_enc: Option<&lopdf::Encoding> = None;
    let mut tlm_y: f64 = 0.0;
    let mut leading: f64 = 0.0;

    let mut pieces: Vec<Piece> = Vec::new();

    for op in &content.operations {
        match op.operator.as_str() {
            "BT" => {
                tlm_y = 0.0;
            }
            "Tf" => {
                if let Some(Object::Name(name)) = op.operands.first() {
                    cur_enc = encodings.get(name);
                }
                if let Some(sz) = op.operands.get(1).and_then(num) {
                    cur_size = sz;
                }
            }
            "TL" => {
                if let Some(l) = op.operands.first().and_then(num) {
                    leading = l;
                }
            }
            "Tm" => {
                if let Some(f) = op.operands.get(5).and_then(num) {
                    tlm_y = f;
                }
            }
            "Td" => {
                if let Some(ty) = op.operands.get(1).and_then(num) {
                    tlm_y += ty;
                }
            }
            "TD" => {
                if let Some(ty) = op.operands.get(1).and_then(num) {
                    leading = -ty;
                    tlm_y += ty;
                }
            }
            "T*" => {
                tlm_y -= leading;
            }
            "Tj" | "TJ" => {
                let text = decode_show(&op.operands, cur_enc, dropped);
                if !text.is_empty() {
                    pieces.push(Piece {
                        y: tlm_y,
                        size: cur_size,
                        text,
                    });
                }
            }
            // `'` = move to next line, then show; `"` = set spacing, next line, show.
            "'" => {
                tlm_y -= leading;
                let text = decode_show(&op.operands, cur_enc, dropped);
                if !text.is_empty() {
                    pieces.push(Piece {
                        y: tlm_y,
                        size: cur_size,
                        text,
                    });
                }
            }
            "\"" => {
                tlm_y -= leading;
                // operands: aw ac string — decode only the trailing string.
                let text = op
                    .operands
                    .get(2)
                    .map(|s| decode_show(std::slice::from_ref(s), cur_enc, dropped))
                    .unwrap_or_default();
                if !text.is_empty() {
                    pieces.push(Piece {
                        y: tlm_y,
                        size: cur_size,
                        text,
                    });
                }
            }
            _ => {}
        }
    }

    group_pieces(pieces)
}

/// Decode a show-text operator's operands to a string, or count a dropped run
/// when the font has no usable encoding.
fn decode_show(
    operands: &[Object],
    enc: Option<&lopdf::Encoding>,
    dropped: &mut usize,
) -> String {
    let mut s = String::new();
    let Some(enc) = enc else {
        *dropped += 1;
        return s;
    };
    collect_text(&mut s, enc, operands, dropped);
    s
}

/// Decode one or more string operands (Tj string, or a TJ array of strings +
/// kerning numbers) into `out`, mirroring lopdf's own spacing rules (a kerning
/// number < -100 becomes a space). Increments `dropped` per undecodable string.
fn collect_text(
    out: &mut String,
    enc: &lopdf::Encoding,
    operands: &[Object],
    dropped: &mut usize,
) {
    for operand in operands {
        match operand {
            Object::String(bytes, _) => match Document::decode_text(enc, bytes) {
                Ok(s) => out.push_str(&s),
                Err(_) => *dropped += 1,
            },
            Object::Array(arr) => {
                collect_text(out, enc, arr, dropped);
                out.push(' ');
            }
            Object::Integer(i) => {
                if *i < -100 {
                    out.push(' ');
                }
            }
            Object::Real(r) => {
                if *r < -100.0 {
                    out.push(' ');
                }
            }
            _ => {}
        }
    }
}

/// Group positioned pieces into visual lines by vertical position (in
/// content-stream order — the natural reading order for single-column PDFs).
fn group_pieces(pieces: Vec<Piece>) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut cur_text = String::new();
    let mut cur_size = 0.0f64;
    let mut cur_y = 0.0f64;
    let mut prev_line_y: Option<f64> = None;
    let mut have_line = false;

    fn flush(
        lines: &mut Vec<Line>,
        text: &mut String,
        size: &mut f64,
        y: f64,
        prev_line_y: &mut Option<f64>,
    ) {
        let normalized = normalize_ws(text);
        if !normalized.is_empty() {
            let gap_before = prev_line_y.map(|p| p - y).unwrap_or(0.0);
            lines.push(Line {
                text: normalized,
                size: *size,
                gap_before,
            });
            *prev_line_y = Some(y);
        }
        text.clear();
        *size = 0.0;
    }

    for p in pieces {
        // Same visual line if the vertical position is within tolerance of the
        // current line's baseline.
        let tol = (0.5 * p.size).max(2.0);
        if have_line && (cur_y - p.y).abs() <= tol {
            cur_text.push_str(&p.text);
            cur_size = cur_size.max(p.size);
        } else {
            if have_line {
                flush(&mut lines, &mut cur_text, &mut cur_size, cur_y, &mut prev_line_y);
            }
            cur_text.push_str(&p.text);
            cur_size = p.size;
            cur_y = p.y;
            have_line = true;
        }
    }
    if have_line {
        flush(&mut lines, &mut cur_text, &mut cur_size, cur_y, &mut prev_line_y);
    }
    lines
}

/// Collapse runs of whitespace to a single space and trim.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Round a font size to the nearest 0.5pt bucket (kills float noise).
fn bucket(size: f64) -> f64 {
    (size * 2.0).round() / 2.0
}

/// The dominant body font size = the 0.5pt bucket carrying the most characters.
/// Returns 0.0 when there are no lines.
fn body_size(pages: &[Vec<Line>]) -> f64 {
    let mut weight: std::collections::BTreeMap<u64, usize> = std::collections::BTreeMap::new();
    for lines in pages {
        for l in lines {
            let key = (bucket(l.size) * 2.0) as u64; // 0.5pt units, as an int key
            *weight.entry(key).or_default() += l.text.chars().count().max(1);
        }
    }
    weight
        .into_iter()
        .max_by_key(|&(_, w)| w)
        .map(|(k, _)| k as f64 / 2.0)
        .unwrap_or(0.0)
}

/// Distinct font-size buckets strictly larger than the body size (by ≥0.75pt),
/// sorted DESCENDING. Index 0 → H1, index 1 → H2, … (capped at H6).
fn heading_size_ranking(pages: &[Vec<Line>], body: f64) -> Vec<f64> {
    let mut sizes: Vec<f64> = Vec::new();
    for lines in pages {
        for l in lines {
            let b = bucket(l.size);
            if b >= body + 0.75 && !sizes.iter().any(|&s| (s - b).abs() < 0.01) {
                sizes.push(b);
            }
        }
    }
    sizes.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    sizes
}

/// A rendered block of Markdown.
enum Block {
    Heading(u8, String),
    /// One list; each item is `(ordered_number, text)` — `None` = bullet.
    List(Vec<(Option<String>, String)>),
    Paragraph(String),
}

/// Render one page's lines into Markdown.
fn render_page(lines: &[Line], body: f64, heading_sizes: &[f64], opts: &Options) -> String {
    let para_gap = 1.5 * body.max(1.0);
    let col_break = 0.5 * body.max(1.0);

    let mut blocks: Vec<Block> = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut list: Vec<(Option<String>, String)> = Vec::new();

    for l in lines {
        match classify(l, body, heading_sizes, opts) {
            LineKind::Heading(level) => {
                flush_para(&mut blocks, &mut para, opts);
                flush_list(&mut blocks, &mut list);
                blocks.push(Block::Heading(level, l.text.clone()));
            }
            LineKind::Bullet(rest) => {
                flush_para(&mut blocks, &mut para, opts);
                list.push((None, rest));
            }
            LineKind::Ordered(n, rest) => {
                flush_para(&mut blocks, &mut para, opts);
                list.push((Some(n), rest));
            }
            LineKind::Body => {
                flush_list(&mut blocks, &mut list);
                if !para.is_empty() && (l.gap_before > para_gap || l.gap_before < -col_break) {
                    flush_para(&mut blocks, &mut para, opts);
                }
                para.push(l.text.clone());
            }
        }
    }
    flush_para(&mut blocks, &mut para, opts);
    flush_list(&mut blocks, &mut list);

    blocks
        .iter()
        .map(render_block)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_block(b: &Block) -> String {
    match b {
        Block::Heading(level, text) => {
            format!("{} {}", "#".repeat(*level as usize), text)
        }
        Block::List(items) => items
            .iter()
            .map(|(ord, text)| match ord {
                Some(n) => format!("{n}. {text}"),
                None => format!("- {text}"),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Block::Paragraph(text) => text.clone(),
    }
}

fn flush_para(blocks: &mut Vec<Block>, para: &mut Vec<String>, opts: &Options) {
    if !para.is_empty() {
        blocks.push(Block::Paragraph(join_paragraph(para, opts.dehyphenate)));
        para.clear();
    }
}

fn flush_list(blocks: &mut Vec<Block>, list: &mut Vec<(Option<String>, String)>) {
    if !list.is_empty() {
        blocks.push(Block::List(std::mem::take(list)));
    }
}

/// Join wrapped body lines into one paragraph. With `dehyphenate`, a line ending
/// in `-` after a letter is joined to a following letter-initial line without
/// the hyphen or a space; otherwise lines are space-joined.
fn join_paragraph(lines: &[String], dehyphenate: bool) -> String {
    let mut s = String::new();
    for (i, line) in lines.iter().enumerate() {
        let line = line.trim();
        if i == 0 {
            s.push_str(line);
            continue;
        }
        let prev_alpha = s
            .chars()
            .rev()
            .nth(1)
            .map(|c| c.is_alphabetic())
            .unwrap_or(false);
        let next_alpha = line
            .chars()
            .next()
            .map(|c| c.is_alphabetic())
            .unwrap_or(false);
        if dehyphenate && s.ends_with('-') && prev_alpha && next_alpha {
            s.pop();
            s.push_str(line);
        } else {
            s.push(' ');
            s.push_str(line);
        }
    }
    s
}

enum LineKind {
    Heading(u8),
    Bullet(String),
    Ordered(String, String),
    Body,
}

fn classify(l: &Line, body: f64, heading_sizes: &[f64], opts: &Options) -> LineKind {
    let b = bucket(l.size);
    if b >= body + 0.75 && l.text.chars().count() <= 200 {
        if let Some(rank) = heading_sizes.iter().position(|&s| (s - b).abs() < 0.01) {
            let level = (rank as u8 + 1).min(6);
            return LineKind::Heading(level);
        }
    }
    if opts.detect_lists {
        if let Some(rest) = parse_bullet(&l.text) {
            return LineKind::Bullet(rest);
        }
        if let Some((n, rest)) = parse_ordered(&l.text) {
            return LineKind::Ordered(n, rest);
        }
    }
    LineKind::Body
}

/// Bullet markers recognized at the start of a line.
const BULLETS: &[char] = &[
    '•', '◦', '‣', '·', '▪', '■', '●', '○', '–', '—', '-', '*', '+',
];

/// If `text` starts with a bullet marker followed by whitespace, return the rest.
fn parse_bullet(text: &str) -> Option<String> {
    let first = text.chars().next()?;
    if !BULLETS.contains(&first) {
        return None;
    }
    let rest = &text[first.len_utf8()..];
    // The marker must be followed by whitespace (else it's just a hyphenated
    // word or a math expression), and there must be real content after it.
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let rest = rest.trim_start();
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

/// If `text` starts with `<digits>.` or `<digits>)` followed by whitespace,
/// return `(number, rest)`.
fn parse_ordered(text: &str) -> Option<(String, String)> {
    let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    let after = &text[digits.len()..];
    let mut ac = after.chars();
    let delim = ac.next()?;
    if delim != '.' && delim != ')' {
        return None;
    }
    let rest = &after[delim.len_utf8()..];
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let rest = rest.trim_start();
    if rest.is_empty() {
        None
    } else {
        Some((digits, rest.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    /// A text run to draw: `(font_size, dy, text)`. `dy` is the vertical move
    /// (`Td 0 dy`) applied BEFORE the run — 0 keeps it on the current line, a
    /// negative value moves down to a new line.
    struct Run(f64, f64, &'static str);

    /// Build a single-page PDF whose content stream draws the given runs with a
    /// standard-encoded Helvetica font (so `decode_text` succeeds).
    fn build_page_pdf(runs: &[Run]) -> Vec<u8> {
        build_pdf(&[runs])
    }

    /// Build a multi-page PDF; each inner slice is one page's runs.
    fn build_pdf(pages: &[&[Run]]) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });

        let mut kids: Vec<Object> = Vec::new();
        for runs in pages {
            let mut ops = vec![
                Operation::new("BT", vec![]),
                Operation::new("Td", vec![72.into(), 720.into()]),
            ];
            for Run(size, dy, text) in *runs {
                if *dy != 0.0 {
                    ops.push(Operation::new("Td", vec![0.into(), (*dy as i64).into()]));
                }
                ops.push(Operation::new(
                    "Tf",
                    vec!["F1".into(), (*size as i64).into()],
                ));
                ops.push(Operation::new("Tj", vec![Object::string_literal(*text)]));
            }
            ops.push(Operation::new("ET", vec![]));
            let content = Content { operations: ops };
            let content_id =
                doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "Resources" => resources_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            });
            kids.push(page_id.into());
        }

        let count = kids.len() as i64;
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn heading_and_body() {
        // 24pt title over 12pt body → # heading + paragraph.
        let pdf = build_page_pdf(&[
            Run(24.0, 0.0, "Big Title"),
            Run(12.0, -40.0, "Some body text on the page."),
        ]);
        let out = to_markdown(&pdf, &Options::default()).unwrap();
        assert_eq!(out.markdown, "# Big Title\n\nSome body text on the page.");
        assert_eq!(out.dropped_runs, 0);
    }

    #[test]
    fn two_heading_levels() {
        let pdf = build_page_pdf(&[
            Run(24.0, 0.0, "Title"),
            Run(16.0, -40.0, "Section"),
            Run(12.0, -30.0, "Body line."),
        ]);
        let out = to_markdown(&pdf, &Options::default()).unwrap();
        assert_eq!(out.markdown, "# Title\n\n## Section\n\nBody line.");
    }

    #[test]
    fn bullet_list() {
        let pdf = build_page_pdf(&[
            Run(12.0, 0.0, "Shopping:"),
            Run(12.0, -20.0, "- apples"),
            Run(12.0, -20.0, "- pears"),
        ]);
        let out = to_markdown(&pdf, &Options::default()).unwrap();
        assert_eq!(out.markdown, "Shopping:\n\n- apples\n- pears");
    }

    #[test]
    fn ordered_list_preserves_numbers() {
        let pdf = build_page_pdf(&[
            Run(12.0, 0.0, "1. first"),
            Run(12.0, -20.0, "2. second"),
        ]);
        let out = to_markdown(&pdf, &Options::default()).unwrap();
        assert_eq!(out.markdown, "1. first\n2. second");
    }

    #[test]
    fn detect_lists_off_keeps_markers_as_text() {
        let pdf = build_page_pdf(&[Run(12.0, 0.0, "- apples"), Run(12.0, -14.0, "- pears")]);
        let opts = Options {
            detect_lists: false,
            ..Options::default()
        };
        let out = to_markdown(&pdf, &opts).unwrap();
        // Same font size + tight gap → the two lines join into one paragraph.
        assert_eq!(out.markdown, "- apples - pears");
    }

    #[test]
    fn dehyphenation_joins_wrapped_word() {
        let pdf = build_page_pdf(&[
            Run(12.0, 0.0, "This is a conver-"),
            Run(12.0, -14.0, "sion of a document."),
        ]);
        let out = to_markdown(&pdf, &Options::default()).unwrap();
        assert_eq!(out.markdown, "This is a conversion of a document.");
    }

    #[test]
    fn dehyphenation_off_keeps_hyphen() {
        let pdf = build_page_pdf(&[
            Run(12.0, 0.0, "This is a conver-"),
            Run(12.0, -14.0, "sion of a document."),
        ]);
        let opts = Options {
            dehyphenate: false,
            ..Options::default()
        };
        let out = to_markdown(&pdf, &opts).unwrap();
        assert_eq!(out.markdown, "This is a conver- sion of a document.");
    }

    #[test]
    fn paragraph_break_on_large_gap() {
        let pdf = build_page_pdf(&[
            Run(12.0, 0.0, "First paragraph line."),
            Run(12.0, -40.0, "Second paragraph after a big gap."),
        ]);
        let out = to_markdown(&pdf, &Options::default()).unwrap();
        assert_eq!(
            out.markdown,
            "First paragraph line.\n\nSecond paragraph after a big gap."
        );
    }

    #[test]
    fn multipage_rule_separator() {
        let pdf = build_pdf(&[
            &[Run(12.0, 0.0, "Page one text.")],
            &[Run(12.0, 0.0, "Page two text.")],
        ]);
        let out = to_markdown(&pdf, &Options::default()).unwrap();
        assert_eq!(out.markdown, "Page one text.\n\n---\n\nPage two text.");
    }

    #[test]
    fn multipage_blank_separator() {
        let pdf = build_pdf(&[
            &[Run(12.0, 0.0, "Page one text.")],
            &[Run(12.0, 0.0, "Page two text.")],
        ]);
        let opts = Options {
            page_separator: PageSeparator::Blank,
            ..Options::default()
        };
        let out = to_markdown(&pdf, &opts).unwrap();
        assert_eq!(out.markdown, "Page one text.\n\nPage two text.");
    }

    #[test]
    fn single_page_selection() {
        let pdf = build_pdf(&[
            &[Run(12.0, 0.0, "Alpha page.")],
            &[Run(12.0, 0.0, "Beta page.")],
        ]);
        let opts = Options {
            page: Some(2),
            ..Options::default()
        };
        let out = to_markdown(&pdf, &opts).unwrap();
        assert_eq!(out.markdown, "Beta page.");
    }

    #[test]
    fn page_out_of_range_errors() {
        let pdf = build_page_pdf(&[Run(12.0, 0.0, "Only one page.")]);
        let opts = Options {
            page: Some(9),
            ..Options::default()
        };
        let err = to_markdown(&pdf, &opts).unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        let err = to_markdown(b"definitely not a pdf", &Options::default()).unwrap_err();
        assert!(err.contains("failed to parse PDF"), "got: {err}");
    }

    #[test]
    fn parse_separator() {
        assert_eq!(PageSeparator::parse("rule"), Some(PageSeparator::Rule));
        assert_eq!(PageSeparator::parse("blank"), Some(PageSeparator::Blank));
        assert_eq!(PageSeparator::parse("nope"), None);
    }

    #[test]
    fn bullet_parser_requires_space() {
        assert_eq!(parse_bullet("- item").as_deref(), Some("item"));
        assert_eq!(parse_bullet("• item").as_deref(), Some("item"));
        assert_eq!(parse_bullet("-nospace"), None);
        assert_eq!(parse_bullet("plain"), None);
    }

    #[test]
    fn ordered_parser() {
        assert_eq!(
            parse_ordered("3. text"),
            Some(("3".to_string(), "text".to_string()))
        );
        assert_eq!(
            parse_ordered("10) text"),
            Some(("10".to_string(), "text".to_string()))
        );
        assert_eq!(parse_ordered("3.text"), None);
        assert_eq!(parse_ordered("word"), None);
    }
}
