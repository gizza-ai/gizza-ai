//! gizza-ai/pdf-notes-outliner core — extract a lecture/course PDF's text layer,
//! reconstruct its heading hierarchy, and build a structured outline (table of
//! contents) with a short **extractive** summary under each section.
//!
//! Pure Rust, no wafer/wasm-bindgen deps, so it compiles natively for unit tests
//! and to `wasm32-wasip1` (the `wafer build` target) for the chat block.
//!
//! ## How it works (no ML model)
//!
//! 1. `pdf-to-markdown-core` reconstructs the document as Markdown using
//!    document-wide **font-size statistics** — the same signal the desktop
//!    auto-ToC tools use — so `#`/`##`/`###` levels are consistent across the
//!    whole document, not guessed per page.
//! 2. We split that Markdown back into headings + the body text under each one,
//!    tagging every heading with the 1-based page it appears on (real page
//!    numbers, recovered from a cheap per-page text probe on the parsed
//!    document so blank/image-only pages don't shift the count).
//! 3. Each section's body is summarised with **TextRank**
//!    (`textrank-summarize-core`) — the top-ranked *verbatim* source sentences,
//!    in original order. `summary_sentences = 0` yields a pure outline / ToC.
//!
//! ## Limits (the text layer only)
//!
//! Extracts the embedded selectable text ONLY — it does **not** OCR
//! scanned/image-only PDFs (those yield no headings and a warning). Summaries are
//! **extractive** (real source sentences ranked by importance), not abstractive —
//! there is no model rewriting the text. Heading detection relies on font-size
//! contrast, so a document typeset in a single uniform size has no detectable
//! headings.

use gizza_ai_pdf_to_markdown_core::{to_markdown, Options, PageSeparator};
use gizza_ai_textrank_summarize_core::summarize;
use lopdf::Document;

/// Hard cap on the number of outline sections returned — guards a pathological
/// document that reports thousands of "headings".
const MAX_SECTIONS: usize = 5_000;

/// Knobs for [`outline`]. See [`OutlineOptions::default`].
#[derive(Debug, Clone, Copy)]
pub struct OutlineOptions {
    /// Deepest heading level to keep as its own section (1 = only the top level).
    /// Headings deeper than this fold into the nearest kept section's body, so
    /// their text still feeds that section's summary. Clamped to `1..=6`.
    pub max_depth: usize,
    /// Number of TextRank sentences to summarise each section with. `0` returns a
    /// pure outline (headings only, no summaries). Clamped to `0..=10`.
    pub summary_sentences: usize,
}

impl Default for OutlineOptions {
    fn default() -> Self {
        OutlineOptions {
            max_depth: 3,
            summary_sentences: 2,
        }
    }
}

impl OutlineOptions {
    fn normalized(self) -> Self {
        OutlineOptions {
            max_depth: self.max_depth.clamp(1, 6),
            summary_sentences: self.summary_sentences.min(10),
        }
    }
}

/// One outline entry: a heading, the level it was detected at, the page it
/// appears on, and an extractive summary of the text under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Detected heading level (1 = biggest font). Reflects the document-wide
    /// font-size ranking, so it's consistent across pages.
    pub level: u8,
    /// The heading text.
    pub title: String,
    /// 1-based page number the heading appears on.
    pub page: usize,
    /// Extractive TextRank summary of the section body (may be empty when the
    /// section has no body text, or when `summary_sentences == 0`).
    pub summary: String,
}

/// The full result of [`outline`]: the sections, a rendered plain-text outline,
/// and an optional advisory note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outline {
    /// The sections in reading order.
    pub sections: Vec<Section>,
    /// A ready-to-read nested outline (indented, with page numbers + summaries).
    pub rendered: String,
    /// Set when the outline is partial/degenerate: no extractable text, no
    /// detectable headings (uniform font), or undecodable font runs. Omitted when
    /// the outline is complete.
    pub note: Option<String>,
}

/// Build a heading outline with section summaries from a PDF's text layer.
///
/// - `bytes` — the raw PDF file.
/// - `opts` — see [`OutlineOptions`].
///
/// Returns `Err` when the bytes don't parse as a PDF or the PDF has no pages. A
/// PDF with no extractable text, or no font-size heading contrast, returns `Ok`
/// with a `note` (and, for the no-heading case, a single whole-document section).
pub fn outline(bytes: &[u8], opts: &OutlineOptions) -> Result<Outline, String> {
    let opts = opts.normalized();

    // Real page numbers of the pages that carry extractable text, in order. Done
    // on the parsed document so a blank/image-only page doesn't shift the count
    // of the content pages that `to_markdown` renders.
    let content_pages = content_page_numbers(bytes)?;

    // One whole-document conversion → consistent heading levels + body text.
    // `Rule` separates content pages with a `\n\n---\n\n` marker we split back on.
    let conv = to_markdown(
        bytes,
        &Options {
            page: None,
            page_separator: PageSeparator::Rule,
            detect_lists: true,
            dehyphenate: true,
        },
    )?;

    // Parse the Markdown into ordered (heading | body) items tagged with a page.
    let items = parse_items(&conv.markdown, &content_pages);

    // Group items into sections, folding sub-`max_depth` headings into the body.
    let mut sections = sectionize(&items, opts.max_depth);

    let no_text = conv.markdown.trim().is_empty();
    let mut note: Option<String> = None;

    if sections.is_empty() {
        if no_text {
            note = Some(
                "no extractable text layer was found — the PDF may be scanned/image-only \
                 (OCR is not supported)"
                    .to_string(),
            );
        } else {
            // Text but no font-size heading contrast: summarise the whole document
            // as one section so the tool still returns something useful.
            note = Some(
                "no headings were detected (the document uses a uniform font size); \
                 showing a single whole-document summary"
                    .to_string(),
            );
            let page = content_pages.first().copied().unwrap_or(1);
            sections.push(RawSection {
                level: 1,
                title: "Document".to_string(),
                page,
                body: conv.markdown.clone(),
            });
        }
    } else if conv.dropped_runs > 0 {
        note = Some(format!(
            "{} text run(s) could not be decoded (unsupported font encoding); the outline is partial",
            conv.dropped_runs
        ));
    }

    if sections.len() > MAX_SECTIONS {
        sections.truncate(MAX_SECTIONS);
    }

    // Normalise the display level so the outline starts flush-left even when the
    // shallowest heading is an H2.
    let min_level = sections.iter().map(|s| s.level).min().unwrap_or(1);
    let out_sections: Vec<Section> = sections
        .iter()
        .map(|s| Section {
            level: s.level,
            title: s.title.clone(),
            page: s.page,
            summary: if opts.summary_sentences == 0 {
                String::new()
            } else {
                summarize(&s.body, opts.summary_sentences)
            },
        })
        .collect();

    let rendered = render_outline(&out_sections, min_level);

    Ok(Outline {
        sections: out_sections,
        rendered,
        note,
    })
}

/// A section before summarisation: the accumulated body text is kept raw.
#[derive(Debug, Clone)]
struct RawSection {
    level: u8,
    title: String,
    page: usize,
    body: String,
}

/// An ordered item parsed from the whole-document Markdown.
enum Item {
    Heading { level: u8, title: String, page: usize },
    Body(String),
}

/// The real 1-based page numbers of pages that carry decodable text, in page
/// order. `to_markdown` drops text-less pages before joining, so these map its
/// rendered content-pages back onto true page numbers.
fn content_page_numbers(bytes: &[u8]) -> Result<Vec<usize>, String> {
    let doc = Document::load_mem(bytes).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let mut page_numbers: Vec<u32> = doc.get_pages().keys().copied().collect();
    page_numbers.sort_unstable();
    if page_numbers.is_empty() {
        return Err("PDF has no pages".to_string());
    }
    let mut content: Vec<usize> = Vec::new();
    for n in page_numbers {
        let has_text = doc
            .extract_text_chunks(&[n])
            .into_iter()
            .any(|c| c.map(|t| !t.trim().is_empty()).unwrap_or(false));
        if has_text {
            content.push(n as usize);
        }
    }
    Ok(content)
}

/// True when `block` is a single-line ATX Markdown heading (`#`..`######` + a
/// space + text). Returns `(level, title)`. Mirrors how `to_markdown` emits
/// headings (each is its own `\n\n`-delimited block).
fn parse_heading(block: &str) -> Option<(u8, String)> {
    if block.contains('\n') {
        return None;
    }
    let hashes = block.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &block[hashes..];
    let title = rest.strip_prefix(' ')?.trim();
    if title.is_empty() {
        None
    } else {
        Some((hashes as u8, title.to_string()))
    }
}

/// Split the whole-document Markdown into ordered heading/body items, tagging
/// each with its content page's real page number.
fn parse_items(markdown: &str, content_pages: &[usize]) -> Vec<Item> {
    let mut items = Vec::new();
    // `Rule` joins content pages with `\n\n---\n\n`.
    for (i, page_md) in markdown.split("\n\n---\n\n").enumerate() {
        let page = content_pages.get(i).copied().unwrap_or(i + 1);
        for block in page_md.split("\n\n") {
            let block = block.trim();
            if block.is_empty() {
                continue;
            }
            match parse_heading(block) {
                Some((level, title)) => items.push(Item::Heading { level, title, page }),
                None => items.push(Item::Body(block.to_string())),
            }
        }
    }
    items
}

/// Group items into sections. A heading at level `<= max_depth` starts a new
/// section; a deeper heading (and its following body) folds into the current
/// section's body so its text still feeds the summary. Body before the first
/// kept heading is dropped (front matter).
fn sectionize(items: &[Item], max_depth: usize) -> Vec<RawSection> {
    let mut sections: Vec<RawSection> = Vec::new();
    for item in items {
        match item {
            Item::Heading { level, title, page } => {
                if *level as usize <= max_depth {
                    sections.push(RawSection {
                        level: *level,
                        title: title.clone(),
                        page: *page,
                        body: String::new(),
                    });
                } else if let Some(cur) = sections.last_mut() {
                    // Fold the too-deep heading title in as body content.
                    push_body(&mut cur.body, title);
                }
            }
            Item::Body(text) => {
                if let Some(cur) = sections.last_mut() {
                    push_body(&mut cur.body, text);
                }
            }
        }
    }
    sections
}

/// Append a body fragment, separating with a newline so TextRank sees sentence
/// boundaries even across Markdown blocks.
fn push_body(body: &mut String, fragment: &str) {
    if !body.is_empty() {
        body.push('\n');
    }
    body.push_str(fragment);
}

/// Render the sections as a nested plain-text outline. `min_level` is the
/// shallowest detected level, so indentation starts flush-left.
fn render_outline(sections: &[Section], min_level: u8) -> String {
    let mut out = String::new();
    for (i, s) in sections.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let depth = s.level.saturating_sub(min_level) as usize;
        let indent = "  ".repeat(depth);
        out.push_str(&format!("{indent}- {}  (p.{})", s.title, s.page));
        if !s.summary.is_empty() {
            out.push('\n');
            out.push_str(&format!("{indent}  {}", s.summary));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    /// A text run to draw: `(font_size, dy, text)`. `dy` is the vertical move
    /// applied BEFORE the run — 0 keeps it on the current line, negative moves
    /// down to a new line.
    struct Run(f64, f64, &'static str);

    /// Build a multi-page PDF; each inner slice is one page's runs, drawn with a
    /// standard-encoded Helvetica font so `decode_text` succeeds.
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
                ops.push(Operation::new("Tf", vec!["F1".into(), (*size as i64).into()]));
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
    fn builds_outline_with_levels_and_summaries() {
        // 24pt title (H1) → 16pt section (H2) → 12pt body.
        let pdf = build_pdf(&[&[
            Run(24.0, 0.0, "Photosynthesis"),
            Run(16.0, -40.0, "Light Reactions"),
            Run(12.0, -30.0, "Chlorophyll absorbs light energy."),
            Run(12.0, -14.0, "The energy splits water molecules."),
            Run(12.0, -14.0, "Oxygen is released as a by-product."),
        ]]);
        let out = outline(&pdf, &OutlineOptions::default()).unwrap();
        assert_eq!(out.note, None);
        assert_eq!(out.sections.len(), 2);
        assert_eq!(out.sections[0].level, 1);
        assert_eq!(out.sections[0].title, "Photosynthesis");
        assert_eq!(out.sections[0].page, 1);
        assert_eq!(out.sections[1].level, 2);
        assert_eq!(out.sections[1].title, "Light Reactions");
        // The H2 section carries the body sentences; its summary is drawn from them.
        assert!(
            out.sections[1].summary.contains("Chlorophyll")
                || out.sections[1].summary.contains("energy")
                || out.sections[1].summary.contains("Oxygen"),
            "summary should quote a body sentence: {:?}",
            out.sections[1].summary
        );
        assert!(out.rendered.contains("- Photosynthesis  (p.1)"));
        assert!(out.rendered.contains("  - Light Reactions  (p.1)"));
    }

    #[test]
    fn page_numbers_track_real_pages() {
        let pdf = build_pdf(&[
            &[Run(20.0, 0.0, "Chapter One"), Run(12.0, -30.0, "Body of chapter one here.")],
            &[Run(20.0, 0.0, "Chapter Two"), Run(12.0, -30.0, "Body of chapter two here.")],
        ]);
        let out = outline(&pdf, &OutlineOptions::default()).unwrap();
        assert_eq!(out.sections.len(), 2);
        assert_eq!(out.sections[0].page, 1);
        assert_eq!(out.sections[1].page, 2);
    }

    #[test]
    fn summary_sentences_zero_gives_pure_toc() {
        let pdf = build_pdf(&[&[
            Run(20.0, 0.0, "Overview"),
            Run(12.0, -30.0, "First fact. Second fact. Third fact. Fourth fact."),
        ]]);
        let opts = OutlineOptions {
            summary_sentences: 0,
            ..OutlineOptions::default()
        };
        let out = outline(&pdf, &opts).unwrap();
        assert_eq!(out.sections.len(), 1);
        assert_eq!(out.sections[0].summary, "");
        assert_eq!(out.rendered, "- Overview  (p.1)");
    }

    #[test]
    fn max_depth_folds_deeper_headings() {
        // H1(24) / H2(18) / H3(14). max_depth=1 keeps only H1; H2/H3 fold in.
        let pdf = build_pdf(&[&[
            Run(24.0, 0.0, "Unit"),
            Run(18.0, -40.0, "Topic"),
            Run(14.0, -30.0, "Subtopic"),
            Run(12.0, -30.0, "Some detail sentence about the subtopic."),
        ]]);
        let opts = OutlineOptions {
            max_depth: 1,
            summary_sentences: 3,
        };
        let out = outline(&pdf, &opts).unwrap();
        assert_eq!(out.sections.len(), 1, "only the H1 survives max_depth=1");
        assert_eq!(out.sections[0].title, "Unit");
        // The folded deeper headings' text is available to the summary.
        assert!(
            out.sections[0].summary.contains("Topic")
                || out.sections[0].summary.contains("Subtopic")
                || out.sections[0].summary.contains("detail"),
            "folded text should feed the summary: {:?}",
            out.sections[0].summary
        );
    }

    #[test]
    fn no_heading_contrast_yields_single_section_and_note() {
        // Every run is the same 12pt size → no heading contrast.
        let pdf = build_pdf(&[&[
            Run(12.0, 0.0, "This document has no headings at all."),
            Run(12.0, -14.0, "It is one uniform block of prose."),
        ]]);
        let out = outline(&pdf, &OutlineOptions::default()).unwrap();
        assert_eq!(out.sections.len(), 1);
        assert_eq!(out.sections[0].title, "Document");
        assert!(out.note.as_deref().unwrap().contains("no headings"));
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        let err = outline(b"definitely not a pdf", &OutlineOptions::default()).unwrap_err();
        assert!(err.contains("failed to parse PDF"), "got: {err}");
    }

    #[test]
    fn parse_heading_requires_space() {
        assert_eq!(parse_heading("# Title"), Some((1, "Title".to_string())));
        assert_eq!(parse_heading("### Deep One"), Some((3, "Deep One".to_string())));
        assert_eq!(parse_heading("#1 pick was great"), None); // no space after #
        assert_eq!(parse_heading("plain paragraph"), None);
        assert_eq!(parse_heading("# a\n# b"), None); // multi-line block is not a heading
    }

    #[test]
    fn options_are_clamped() {
        let o = OutlineOptions {
            max_depth: 99,
            summary_sentences: 999,
        }
        .normalized();
        assert_eq!(o.max_depth, 6);
        assert_eq!(o.summary_sentences, 10);
    }
}
