//! gizza-ai/search-in-documents core — search a regex or keyword *inside* a
//! binary document (PDF, DOCX, EPUB) or a ZIP archive of documents, returning
//! each hit with the source document and the page/section/line it came from.
//!
//! Pure Rust with no wafer/wasm-bindgen deps, so it compiles natively for the
//! unit tests and to `wasm32-wasip1` (the `wafer build` target) for the chat
//! block.
//!
//! ## Location-aware extraction (this is what makes it distinct from
//! `document-text-extract` + `regex-search`)
//!
//! Rather than flatten a document to one text blob, the input is broken into
//! **units** that carry a location label, and the pattern is matched line by
//! line within each unit:
//!
//! - **PDF** (`%PDF-` header) → one unit per page (lopdf `extract_text_chunks`,
//!   per page). Location = `"page N"`. Text runs whose font encoding cannot be
//!   decoded are skipped and counted (a partial-text note), matching
//!   `pdf-extract-text`.
//! - **DOCX** (a ZIP with `word/document.xml`) → a single unit of the flattened
//!   WordprocessingML runs. A `.docx` stores no hard page breaks, so the
//!   location is the 1-based line number within the document.
//! - **EPUB** (a ZIP with `META-INF/container.xml` / a `mimetype` entry) → a
//!   single unit of the reading-order text (OPF spine). Location is the line
//!   number.
//! - **ZIP archive** (any other ZIP) → each entry is searched: PDF entries as
//!   pages, UTF-8 text entries as line-numbered text. Each match carries the
//!   entry path as its `document`. Nested DOCX/EPUB/ZIP inside the archive are
//!   not recursed (provide those directly); the count of skipped binary entries
//!   is reported.
//!
//! The search itself mirrors `regex-search`: literal-substring by default (the
//! pattern is regex-escaped), optional real regex, case-insensitive by default,
//! optional whole-word, with the matched text wrapped in guillemets («…») so the
//! hit is visible in the plain-text output.

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;
use regex::RegexBuilder;

/// Cap on pages iterated for a single PDF (guards a pathological page count).
const MAX_PAGES: usize = 10_000;
/// Cap on the length (in chars) of a returned match line before it is windowed
/// around the first hit, so a single very long line can't dominate the output.
const MAX_LINE_CHARS: usize = 240;
/// Hard cap on `max_matches` regardless of the requested value.
pub const MAX_MATCHES_CAP: usize = 1_000;

/// How the pattern is interpreted and matched.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Treat `pattern` as a regular expression. When false, it is matched as a
    /// literal substring (every regex metacharacter is escaped).
    pub regex: bool,
    /// Match case exactly. When false, the search is case-insensitive.
    pub case_sensitive: bool,
    /// Only match whole words (word boundaries on both sides).
    pub whole_word: bool,
    /// Maximum number of matching lines to return (clamped to [`MAX_MATCHES_CAP`]).
    pub max_matches: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        SearchOptions {
            regex: false,
            case_sensitive: false,
            whole_word: false,
            max_matches: 200,
        }
    }
}

/// One matching line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// The source document: the input's name for a single file, or the entry
    /// path for a match inside a ZIP archive.
    pub document: String,
    /// Where in the document the match is: `"page N"` for PDFs, `"line N"` for
    /// DOCX/EPUB/text.
    pub location: String,
    /// 1-based line number within the located unit (the page, or the document).
    pub line: usize,
    /// The matching line, with each hit wrapped in guillemets («…»). Long lines
    /// are windowed around the first hit with `…` ellipses.
    pub text: String,
}

/// The result of a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOutcome {
    /// Detected top-level format: `"pdf"`, `"docx"`, `"epub"`, or `"zip"`.
    pub format: String,
    /// Number of distinct documents searched (1 for a single file; the number
    /// of extracted entries for a ZIP archive).
    pub documents_searched: usize,
    /// The matching lines, in document order, capped at `max_matches`.
    pub matches: Vec<Match>,
    /// True when the match cap was reached and further matches were dropped.
    pub truncated: bool,
    /// Set when some content could not be searched: PDF text runs skipped for an
    /// unsupported font encoding, or archive entries skipped as non-text binary.
    pub note: Option<String>,
}

/// A location-labeled chunk of text to search.
struct Unit {
    document: String,
    /// Location kind: pages carry `"page"`, flat docs carry `"line"`.
    location_kind: LocationKind,
    text: String,
    /// For a page unit, the page number; ignored for line units.
    page: usize,
}

#[derive(Clone, Copy)]
enum LocationKind {
    Page,
    Line,
}

/// A leaf document format we can extract (used both at the top level and per
/// ZIP entry).
enum Leaf {
    Pdf,
    Docx,
    Epub,
    /// UTF-8 text (`.txt`, `.md`, `.html`, `.csv`, `.log`, `.json`, `.xml`, …).
    Text,
}

/// Search `bytes` (a document or archive) for `pattern`.
///
/// `doc_name` is the display name of a single top-level document (e.g. the file
/// name derived from the URL); for a ZIP archive the entry paths are used
/// instead. Returns `Err` on an empty input, an unrecognized format, an invalid
/// regex, or a container that fails to parse.
pub fn search(
    bytes: &[u8],
    pattern: &str,
    doc_name: &str,
    opts: &SearchOptions,
) -> Result<SearchOutcome, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    if pattern.is_empty() {
        return Err("pattern is empty — provide a keyword or regular expression".into());
    }

    // Build the effective matcher, mirroring regex-search: escape for literal
    // mode, wrap in word boundaries for whole-word, case-insensitive by default.
    let mut effective = if opts.regex {
        pattern.to_string()
    } else {
        regex::escape(pattern)
    };
    if opts.whole_word {
        effective = format!(r"\b(?:{effective})\b");
    }
    let re = RegexBuilder::new(&effective)
        .case_insensitive(!opts.case_sensitive)
        .build()
        .map_err(|e| format!("invalid regular expression: {e}"))?;

    let cap = opts.max_matches.clamp(1, MAX_MATCHES_CAP);

    // Extract location-labeled units + a partial-extraction note.
    let format;
    let mut units: Vec<Unit> = Vec::new();
    let mut dropped_chunks = 0usize;
    let mut skipped_entries = 0usize;
    let documents_searched;

    match sniff_top(bytes)? {
        Top::Pdf => {
            format = "pdf";
            documents_searched = 1;
            dropped_chunks += pdf_units(bytes, doc_name, &mut units)?;
        }
        Top::Docx => {
            format = "docx";
            documents_searched = 1;
            units.push(Unit {
                document: doc_name.to_string(),
                location_kind: LocationKind::Line,
                text: extract_docx(bytes)?,
                page: 0,
            });
        }
        Top::Epub => {
            format = "epub";
            documents_searched = 1;
            units.push(Unit {
                document: doc_name.to_string(),
                location_kind: LocationKind::Line,
                text: gizza_ai_epub_to_markdown_core::convert(
                    bytes,
                    gizza_ai_epub_to_markdown_core::Mode::Text,
                )?
                .content,
                page: 0,
            });
        }
        Top::Zip => {
            format = "zip";
            let (docs, dropped, skipped) = zip_units(bytes, &mut units)?;
            documents_searched = docs;
            dropped_chunks += dropped;
            skipped_entries += skipped;
        }
    }

    // Search each unit line by line.
    let mut matches: Vec<Match> = Vec::new();
    let mut truncated = false;
    'outer: for unit in &units {
        for (i, line) in unit.text.lines().enumerate() {
            if let Some(marked) = mark_line(line, &re) {
                if matches.len() >= cap {
                    truncated = true;
                    break 'outer;
                }
                let (location, line_no) = match unit.location_kind {
                    LocationKind::Page => (format!("page {}", unit.page), i + 1),
                    LocationKind::Line => (format!("line {}", i + 1), i + 1),
                };
                matches.push(Match {
                    document: unit.document.clone(),
                    location,
                    line: line_no,
                    text: marked,
                });
            }
        }
    }

    let note = build_note(dropped_chunks, skipped_entries);

    Ok(SearchOutcome {
        format: format.to_string(),
        documents_searched,
        matches,
        truncated,
        note,
    })
}

/// Compose the partial-extraction note, if any.
fn build_note(dropped_chunks: usize, skipped_entries: usize) -> Option<String> {
    let mut parts = Vec::new();
    if dropped_chunks > 0 {
        parts.push(format!(
            "{dropped_chunks} PDF text run(s) could not be decoded (unsupported font \
             encoding); those pages are searched only over the text that decoded"
        ));
    }
    if skipped_entries > 0 {
        parts.push(format!(
            "{skipped_entries} archive entr(y/ies) skipped as non-text binary (nested \
             DOCX/EPUB/ZIP are not recursed — search them directly)"
        ));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

/// Detected top-level container format.
enum Top {
    Pdf,
    Docx,
    Epub,
    Zip,
}

/// Sniff the top-level format from the magic bytes (and, for a ZIP, its entries).
fn sniff_top(bytes: &[u8]) -> Result<Top, String> {
    if bytes.starts_with(b"%PDF-") {
        return Ok(Top::Pdf);
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        let zip = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| format!("not a valid ZIP container: {e}"))?;
        let names: Vec<String> = zip.file_names().map(|s| s.to_string()).collect();
        if names.iter().any(|n| n == "word/document.xml") {
            return Ok(Top::Docx);
        }
        if names.iter().any(|n| n == "META-INF/container.xml")
            || names.iter().any(|n| n.eq_ignore_ascii_case("mimetype"))
        {
            return Ok(Top::Epub);
        }
        return Ok(Top::Zip);
    }
    Err("unrecognized input — expected a PDF (%PDF header), a DOCX (ZIP with \
         word/document.xml), an EPUB (ZIP with META-INF/container.xml), or a ZIP \
         archive of documents"
        .into())
}

/// Sniff a leaf (per-ZIP-entry) format. Returns `None` for non-text binary.
fn sniff_leaf(bytes: &[u8]) -> Option<Leaf> {
    if bytes.starts_with(b"%PDF-") {
        return Some(Leaf::Pdf);
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        let names: Vec<String> = match zip::ZipArchive::new(Cursor::new(bytes)) {
            Ok(z) => z.file_names().map(|s| s.to_string()).collect(),
            Err(_) => return None,
        };
        if names.iter().any(|n| n == "word/document.xml") {
            return Some(Leaf::Docx);
        }
        if names.iter().any(|n| n == "META-INF/container.xml")
            || names.iter().any(|n| n.eq_ignore_ascii_case("mimetype"))
        {
            return Some(Leaf::Epub);
        }
        // A nested plain ZIP: not recursed.
        return None;
    }
    // Treat valid UTF-8 as searchable text.
    if std::str::from_utf8(bytes).is_ok() {
        return Some(Leaf::Text);
    }
    None
}

/// Extract per-page units from a PDF. Returns the count of dropped text runs.
fn pdf_units(bytes: &[u8], document: &str, out: &mut Vec<Unit>) -> Result<usize, String> {
    use lopdf::Document;
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
    let mut dropped = 0usize;
    for n in page_numbers {
        let mut page_text = String::new();
        for chunk in doc.extract_text_chunks(&[n]) {
            match chunk {
                Ok(t) => page_text.push_str(&t),
                Err(_) => dropped += 1,
            }
        }
        out.push(Unit {
            document: document.to_string(),
            location_kind: LocationKind::Page,
            text: page_text,
            page: n as usize,
        });
    }
    Ok(dropped)
}

/// Extract units from a generic ZIP archive. Returns
/// `(documents_extracted, dropped_pdf_chunks, skipped_binary_entries)`.
fn zip_units(bytes: &[u8], out: &mut Vec<Unit>) -> Result<(usize, usize, usize), String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("not a valid ZIP archive: {e}"))?;
    let mut docs = 0usize;
    let mut dropped = 0usize;
    let mut skipped = 0usize;
    for i in 0..zip.len() {
        let (name, data) = {
            let mut entry = zip
                .by_index(i)
                .map_err(|e| format!("failed to read archive entry {i}: {e}"))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();
            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .map_err(|e| format!("failed to read archive entry {name}: {e}"))?;
            (name, data)
        };
        match sniff_leaf(&data) {
            Some(Leaf::Pdf) => {
                let before = out.len();
                match pdf_units(&data, &name, out) {
                    Ok(d) => {
                        dropped += d;
                        if out.len() > before {
                            docs += 1;
                        }
                    }
                    // A corrupt PDF entry shouldn't fail the whole archive.
                    Err(_) => skipped += 1,
                }
            }
            Some(Leaf::Text) => {
                out.push(Unit {
                    document: name,
                    location_kind: LocationKind::Line,
                    text: String::from_utf8_lossy(&data).into_owned(),
                    page: 0,
                });
                docs += 1;
            }
            // Nested DOCX/EPUB/ZIP or other binary: not recursed.
            Some(Leaf::Docx) | Some(Leaf::Epub) | None => skipped += 1,
        }
    }
    Ok((docs, dropped, skipped))
}

/// Local element name with any `ns:` prefix stripped (WordprocessingML tags are
/// namespaced, e.g. `w:t`, `w:p`).
fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    }
}

/// Read `word/document.xml` from a DOCX and flatten its WordprocessingML runs to
/// plain text (paragraphs → newlines, `<w:tab>` → tab, `<w:br>`/`<w:cr>` →
/// newline). Mirrors `document-text-extract`.
fn extract_docx(bytes: &[u8]) -> Result<String, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("not a valid DOCX/ZIP container: {e}"))?;
    let mut xml = Vec::new();
    {
        let mut entry = zip
            .by_name("word/document.xml")
            .map_err(|_| "DOCX is missing word/document.xml".to_string())?;
        entry
            .read_to_end(&mut xml)
            .map_err(|e| format!("failed to read word/document.xml: {e}"))?;
    }
    let mut reader = Reader::from_reader(xml.as_slice());
    let mut buf = Vec::new();
    let mut text = String::new();
    let mut in_text = false;
    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("malformed DOCX XML: {e}"))?
        {
            Event::Start(e) if local_name(e.name().as_ref()) == b"t" => in_text = true,
            Event::End(e) => match local_name(e.name().as_ref()) {
                b"t" => in_text = false,
                b"p" => text.push('\n'),
                _ => {}
            },
            Event::Empty(e) => match local_name(e.name().as_ref()) {
                b"tab" => text.push('\t'),
                b"br" | b"cr" => text.push('\n'),
                _ => {}
            },
            Event::Text(t) if in_text => {
                let decoded = t
                    .decode()
                    .map_err(|e| format!("failed to decode DOCX text: {e}"))?;
                text.push_str(&decoded);
            }
            Event::GeneralRef(r) if in_text => {
                if let Some(c) = r
                    .resolve_char_ref()
                    .map_err(|e| format!("bad character reference in DOCX: {e}"))?
                {
                    text.push(c);
                } else {
                    let name = r
                        .decode()
                        .map_err(|e| format!("bad entity reference in DOCX: {e}"))?;
                    match name.as_ref() {
                        "amp" => text.push('&'),
                        "lt" => text.push('<'),
                        "gt" => text.push('>'),
                        "quot" => text.push('"'),
                        "apos" => text.push('\''),
                        _ => {}
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    while text.ends_with('\n') {
        text.pop();
    }
    Ok(text)
}

/// Mark every non-empty match in `line` with guillemets. Returns `None` when the
/// line has no (non-empty) match. Long lines are windowed around the first hit.
fn mark_line(line: &str, re: &regex::Regex) -> Option<String> {
    let mut marked = String::new();
    let mut last = 0usize;
    let mut any = false;
    let mut first_start: Option<usize> = None;
    for m in re.find_iter(line) {
        if m.start() == m.end() {
            continue; // skip zero-width matches (e.g. `a*` on an empty span)
        }
        any = true;
        if first_start.is_none() {
            first_start = Some(marked.chars().count() + line[last..m.start()].chars().count());
        }
        marked.push_str(&line[last..m.start()]);
        marked.push('\u{ab}'); // «
        marked.push_str(m.as_str());
        marked.push('\u{bb}'); // »
        last = m.end();
    }
    if !any {
        return None;
    }
    marked.push_str(&line[last..]);
    Some(window(&marked, first_start.unwrap_or(0)))
}

/// Window a marked line to at most [`MAX_LINE_CHARS`] chars around `focus`
/// (a char index), adding `…` where content is trimmed.
fn window(marked: &str, focus: usize) -> String {
    let chars: Vec<char> = marked.chars().collect();
    if chars.len() <= MAX_LINE_CHARS {
        return marked.to_string();
    }
    let half = MAX_LINE_CHARS / 2;
    let start = focus.saturating_sub(half);
    let end = (start + MAX_LINE_CHARS).min(chars.len());
    let start = end.saturating_sub(MAX_LINE_CHARS);
    let mut out = String::new();
    if start > 0 {
        out.push('\u{2026}'); // …
    }
    out.extend(chars[start..end].iter());
    if end < chars.len() {
        out.push('\u{2026}');
    }
    out
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    use super::*;

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            for (name, data) in entries {
                w.start_file(*name, opts).unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf
    }

    fn make_docx(body: &str) -> Vec<u8> {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>{body}</w:body></w:document>"#
        );
        zip_of(&[
            ("[Content_Types].xml", b"<Types/>"),
            ("word/document.xml", xml.as_bytes()),
        ])
    }

    fn make_epub(paragraphs: &[&str]) -> Vec<u8> {
        let container = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
<manifest><item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/></manifest>
<spine><itemref idref="c1"/></spine></package>"#;
        let body: String = paragraphs.iter().map(|p| format!("<p>{p}</p>")).collect();
        let chapter = format!(
            r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body>{body}</body></html>"#
        );
        zip_of(&[
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", container),
            ("content.opf", opf),
            ("chapter1.xhtml", chapter.as_bytes()),
        ])
    }

    /// A two-page PDF: page 1 says "invoice total", page 2 says "thank you".
    fn make_pdf(pages: &[&str]) -> Vec<u8> {
        // Reuse text-to-pdf per page then... simpler: build via text-to-pdf which
        // only makes single-page PDFs, so concatenate is not valid. Instead build
        // with lopdf directly, mirroring pdf-extract-text's fixture.
        use lopdf::content::{Content, Operation};
        use lopdf::{dictionary, Document, Object, Stream};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let mut kids: Vec<Object> = Vec::new();
        for text in pages {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 24.into()]),
                    Operation::new("Td", vec![100.into(), 600.into()]),
                    Operation::new("Tj", vec![Object::string_literal(*text)]),
                    Operation::new("ET", vec![]),
                ],
            };
            let content_id =
                doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Resources" => resources_id,
            });
            kids.push(page_id.into());
        }
        let count = kids.len() as i64;
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog", "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn pdf_reports_the_page_of_each_hit() {
        let pdf = make_pdf(&["invoice total due", "thank you for your invoice"]);
        let out = search(&pdf, "invoice", "statement.pdf", &SearchOptions::default()).unwrap();
        assert_eq!(out.format, "pdf");
        assert_eq!(out.documents_searched, 1);
        assert_eq!(out.matches.len(), 2);
        assert_eq!(out.matches[0].location, "page 1");
        assert_eq!(out.matches[0].document, "statement.pdf");
        assert!(out.matches[0].text.contains("\u{ab}invoice\u{bb}"));
        assert_eq!(out.matches[1].location, "page 2");
    }

    #[test]
    fn docx_line_location_and_case_insensitive_default() {
        let docx = make_docx(
            r#"<w:p><w:r><w:t>The Quarterly Report</w:t></w:r></w:p>
<w:p><w:r><w:t>Revenue grew.</w:t></w:r></w:p>
<w:p><w:r><w:t>See the report appendix.</w:t></w:r></w:p>"#,
        );
        let out = search(&docx, "report", "q3.docx", &SearchOptions::default()).unwrap();
        assert_eq!(out.format, "docx");
        // "Report" (line 1) + "report" (line 3), case-insensitive by default.
        assert_eq!(out.matches.len(), 2);
        assert_eq!(out.matches[0].location, "line 1");
        assert_eq!(out.matches[1].location, "line 3");
    }

    #[test]
    fn epub_is_searched() {
        let epub = make_epub(&["Call me Ishmael.", "The whale surfaced."]);
        let out = search(&epub, "whale", "book.epub", &SearchOptions::default()).unwrap();
        assert_eq!(out.format, "epub");
        assert_eq!(out.matches.len(), 1);
        assert!(out.matches[0].text.contains("\u{ab}whale\u{bb}"));
    }

    #[test]
    fn zip_archive_tags_each_match_with_its_entry() {
        let a = zip_of(&[
            ("notes/todo.txt", b"buy milk\nfix the bug"),
            ("readme.md", b"# Title\nfix the docs"),
        ]);
        let out = search(&a, "fix", "bundle.zip", &SearchOptions::default()).unwrap();
        assert_eq!(out.format, "zip");
        assert_eq!(out.documents_searched, 2);
        assert_eq!(out.matches.len(), 2);
        let docs: Vec<&str> = out.matches.iter().map(|m| m.document.as_str()).collect();
        assert!(docs.contains(&"notes/todo.txt"));
        assert!(docs.contains(&"readme.md"));
    }

    #[test]
    fn regex_mode_and_whole_word() {
        let docx = make_docx(
            r#"<w:p><w:r><w:t>Order 12345 shipped</w:t></w:r></w:p>
<w:p><w:r><w:t>reorder soon</w:t></w:r></w:p>"#,
        );
        // Regex for a 5-digit run finds only the line with the number.
        let opts = SearchOptions {
            regex: true,
            ..SearchOptions::default()
        };
        let out = search(&docx, r"\d{5}", "orders.docx", &opts).unwrap();
        assert_eq!(out.matches.len(), 1);
        assert!(out.matches[0].text.contains("\u{ab}12345\u{bb}"));

        // Whole-word literal "order" matches "Order" (line 1) but not "reorder".
        let opts = SearchOptions {
            whole_word: true,
            ..SearchOptions::default()
        };
        let out = search(&docx, "order", "orders.docx", &opts).unwrap();
        assert_eq!(out.matches.len(), 1);
        assert_eq!(out.matches[0].location, "line 1");
    }

    #[test]
    fn max_matches_caps_and_flags_truncated() {
        let docx = make_docx(
            r#"<w:p><w:r><w:t>hit one</w:t></w:r></w:p>
<w:p><w:r><w:t>hit two</w:t></w:r></w:p>
<w:p><w:r><w:t>hit three</w:t></w:r></w:p>"#,
        );
        let opts = SearchOptions {
            max_matches: 2,
            ..SearchOptions::default()
        };
        let out = search(&docx, "hit", "x.docx", &opts).unwrap();
        assert_eq!(out.matches.len(), 2);
        assert!(out.truncated);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = search(b"", "x", "n", &SearchOptions::default()).unwrap_err();
        assert!(err.contains("empty"), "err was: {err}");
    }

    #[test]
    fn empty_pattern_is_an_error() {
        let err = search(b"%PDF-1.5\n", "", "n", &SearchOptions::default()).unwrap_err();
        assert!(err.contains("pattern is empty"), "err was: {err}");
    }

    #[test]
    fn invalid_regex_is_an_error() {
        let opts = SearchOptions {
            regex: true,
            ..SearchOptions::default()
        };
        let err = search(b"%PDF-1.5\n", "(unclosed", "n", &opts).unwrap_err();
        assert!(err.contains("invalid regular expression"), "err was: {err}");
    }

    #[test]
    fn unrecognized_bytes_are_an_error() {
        let err = search(
            b"just some plain bytes, not a document",
            "x",
            "n",
            &SearchOptions::default(),
        )
        .unwrap_err();
        assert!(err.contains("unrecognized input"), "err was: {err}");
    }

    #[test]
    fn no_matches_returns_empty_list() {
        let docx = make_docx(r#"<w:p><w:r><w:t>nothing to see</w:t></w:r></w:p>"#);
        let out = search(&docx, "zzz", "x.docx", &SearchOptions::default()).unwrap();
        assert!(out.matches.is_empty());
        assert!(!out.truncated);
    }
}
