//! gizza-ai/document-text-extract core — extract plain text from a PDF, DOCX, or
//! EPUB document, auto-detecting the format from the file's magic bytes. Pure
//! Rust with no wafer/wasm-bindgen deps, so it compiles natively for the unit
//! tests and to `wasm32-wasip1` (the `wafer build` target) for the chat block.
//!
//! ## Format detection + delegation
//!
//! - **PDF** (`%PDF-` header) → delegates to the shared `pdf-extract-text` core
//!   (lopdf). Extracts the embedded selectable text layer only — it does NOT OCR
//!   scanned/image-only pages (those legitimately return empty text). Reports a
//!   count of text runs whose font encoding could not be decoded.
//! - **DOCX** (a ZIP whose entries include `word/document.xml`) → reads the main
//!   document part and flattens its WordprocessingML runs: `<w:t>` is the text,
//!   `<w:p>` ends a paragraph (newline), `<w:tab>` is a tab, `<w:br>`/`<w:cr>`
//!   are line breaks.
//! - **EPUB** (a ZIP with `META-INF/container.xml` / a `mimetype` entry) →
//!   delegates to the shared `epub-to-markdown` core in plain-text mode (reading
//!   order via the OPF spine).
//!
//! DOCX and EPUB are both ZIP containers, so the ZIP entry list disambiguates
//! them.

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;

/// Which document format was detected and extracted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocFormat {
    Pdf,
    Docx,
    Epub,
}

impl DocFormat {
    /// Lower-case format tag for the JSON response (`"pdf"`/`"docx"`/`"epub"`).
    pub fn as_str(self) -> &'static str {
        match self {
            DocFormat::Pdf => "pdf",
            DocFormat::Docx => "docx",
            DocFormat::Epub => "epub",
        }
    }
}

/// A successful extraction: the detected format, the decoded plain text, and the
/// number of text runs that could not be decoded. `dropped_chunks` is PDF-only
/// (an unparseable font `ToUnicode` CMap); a non-zero value means `text` is
/// partial. It is always `0` for DOCX and EPUB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    /// The detected document format.
    pub format: DocFormat,
    /// The decoded plain text.
    pub text: String,
    /// Count of text runs skipped because their font encoding failed to parse.
    pub dropped_chunks: usize,
}

/// Detect the document format from the leading magic bytes (and, for ZIP
/// containers, the entry list). Returns `None` when the bytes are neither a PDF
/// nor a recognized ZIP-based document.
fn sniff(bytes: &[u8]) -> Option<DocFormat> {
    if bytes.starts_with(b"%PDF-") {
        return Some(DocFormat::Pdf);
    }
    // ZIP local-file header ("PK\x03\x04") or empty-archive header ("PK\x05\x06").
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        let zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
        let names: Vec<String> = zip.file_names().map(|s| s.to_string()).collect();
        if names.iter().any(|n| n == "word/document.xml") {
            return Some(DocFormat::Docx);
        }
        if names.iter().any(|n| n == "META-INF/container.xml")
            || names.iter().any(|n| n.eq_ignore_ascii_case("mimetype"))
        {
            return Some(DocFormat::Epub);
        }
    }
    None
}

/// Extract the plain text from a PDF, DOCX, or EPUB document, auto-detecting the
/// format from its magic bytes.
///
/// Returns `Err` when the bytes are empty, are not a recognized document format,
/// or the recognized container fails to parse.
pub fn extract(bytes: &[u8]) -> Result<Extraction, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    match sniff(bytes) {
        Some(DocFormat::Pdf) => {
            let ex = gizza_ai_pdf_extract_text_core::extract(bytes, None)?;
            Ok(Extraction {
                format: DocFormat::Pdf,
                text: ex.text,
                dropped_chunks: ex.dropped_chunks,
            })
        }
        Some(DocFormat::Docx) => extract_docx(bytes),
        Some(DocFormat::Epub) => {
            let book = gizza_ai_epub_to_markdown_core::convert(
                bytes,
                gizza_ai_epub_to_markdown_core::Mode::Text,
            )?;
            Ok(Extraction {
                format: DocFormat::Epub,
                text: book.content,
                dropped_chunks: 0,
            })
        }
        None => Err("unrecognized document format — expected a PDF (%PDF header), a DOCX \
             (a ZIP containing word/document.xml), or an EPUB (a ZIP containing \
             META-INF/container.xml)"
            .into()),
    }
}

/// Local element name with any `ns:` prefix stripped (WordprocessingML tags are
/// namespaced, e.g. `w:t`, `w:p`).
fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    }
}

/// Read the main document part of a DOCX (`word/document.xml`) and flatten its
/// WordprocessingML runs to plain text.
fn extract_docx(bytes: &[u8]) -> Result<Extraction, String> {
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
    Ok(Extraction {
        format: DocFormat::Docx,
        text: docx_xml_to_text(&xml)?,
        dropped_chunks: 0,
    })
}

/// Flatten a WordprocessingML `document.xml` body to plain text.
fn docx_xml_to_text(xml: &[u8]) -> Result<String, String> {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut out = String::new();
    let mut in_text = false;
    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("malformed DOCX XML: {e}"))?
        {
            Event::Start(e) if local_name(e.name().as_ref()) == b"t" => in_text = true,
            Event::End(e) => match local_name(e.name().as_ref()) {
                b"t" => in_text = false,
                b"p" => out.push('\n'),
                _ => {}
            },
            Event::Empty(e) => match local_name(e.name().as_ref()) {
                b"tab" => out.push('\t'),
                b"br" | b"cr" => out.push('\n'),
                _ => {}
            },
            Event::Text(t) if in_text => {
                // quick-xml 0.40 emits entity/char references as separate
                // `GeneralRef` events, so a Text event holds only literal text.
                let decoded = t
                    .decode()
                    .map_err(|e| format!("failed to decode DOCX text: {e}"))?;
                out.push_str(&decoded);
            }
            // Entity/character references inside a `<w:t>` run: `&amp;`, `&#160;`,
            // `&#x2019;`, … quick-xml resolves numeric refs; the five predefined
            // named entities are mapped here.
            Event::GeneralRef(r) if in_text => {
                if let Some(c) = r
                    .resolve_char_ref()
                    .map_err(|e| format!("bad character reference in DOCX: {e}"))?
                {
                    out.push(c);
                } else {
                    let name = r
                        .decode()
                        .map_err(|e| format!("bad entity reference in DOCX: {e}"))?;
                    match name.as_ref() {
                        "amp" => out.push('&'),
                        "lt" => out.push('<'),
                        "gt" => out.push('>'),
                        "quot" => out.push('"'),
                        "apos" => out.push('\''),
                        _ => {} // unknown named entity — DOCX bodies use numeric refs
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    // A DOCX body ends with a final `<w:p>`, so `out` ends with a trailing
    // newline; drop trailing blank lines for a clean result.
    while out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    use super::*;

    /// Build an in-memory ZIP from `(name, bytes)` entries (stored, uncompressed
    /// — deterministic and dependency-light for fixtures).
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

    /// A minimal but valid DOCX carrying two paragraphs, a tab, and an entity.
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

    /// A minimal but valid EPUB with one chapter of body text.
    fn make_epub(body_text: &str) -> Vec<u8> {
        let container = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
<manifest><item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/></manifest>
<spine><itemref idref="c1"/></spine></package>"#;
        let chapter = format!(
            r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body><p>{body_text}</p></body></html>"#
        );
        zip_of(&[
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", container),
            ("content.opf", opf),
            ("chapter1.xhtml", chapter.as_bytes()),
        ])
    }

    #[test]
    fn extracts_docx_paragraphs_tab_and_entity() {
        let docx = make_docx(
            r#"<w:p><w:r><w:t>Hello World</w:t></w:r></w:p>
<w:p><w:r><w:t>Line</w:t><w:tab/><w:t>A &amp; B</w:t></w:r></w:p>"#,
        );
        let ex = extract(&docx).unwrap();
        assert_eq!(ex.format, DocFormat::Docx);
        assert_eq!(ex.dropped_chunks, 0);
        assert_eq!(ex.text, "Hello World\nLine\tA & B");
    }

    #[test]
    fn extracts_epub_body_text() {
        let epub = make_epub("An EPUB chapter body.");
        let ex = extract(&epub).unwrap();
        assert_eq!(ex.format, DocFormat::Epub);
        assert!(
            ex.text.contains("An EPUB chapter body."),
            "epub text was: {:?}",
            ex.text
        );
    }

    #[test]
    fn extracts_pdf_text_layer() {
        let pdf = gizza_ai_text_to_pdf_core::text_to_pdf("Hello World", 12.0, 40.0).unwrap();
        assert!(pdf.starts_with(b"%PDF-"), "text-to-pdf produced a PDF header");
        let ex = extract(&pdf).unwrap();
        assert_eq!(ex.format, DocFormat::Pdf);
        assert!(
            ex.text.contains("Hello World"),
            "pdf text was: {:?}",
            ex.text
        );
    }

    #[test]
    fn sniff_detects_each_format() {
        assert_eq!(sniff(b"%PDF-1.7\n..."), Some(DocFormat::Pdf));
        assert_eq!(sniff(&make_docx("<w:p><w:r><w:t>x</w:t></w:r></w:p>")), Some(DocFormat::Docx));
        assert_eq!(sniff(&make_epub("x")), Some(DocFormat::Epub));
        assert_eq!(sniff(b"not a document"), None);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = extract(b"").unwrap_err();
        assert!(err.contains("empty"), "err was: {err}");
    }

    #[test]
    fn unrecognized_bytes_are_an_error() {
        let err = extract(b"just some plain text, not a document file").unwrap_err();
        assert!(
            err.contains("unrecognized document format"),
            "err was: {err}"
        );
    }

    #[test]
    fn zip_without_doc_or_epub_markers_is_an_error() {
        let bogus = zip_of(&[("readme.txt", b"hello")]);
        let err = extract(&bogus).unwrap_err();
        assert!(
            err.contains("unrecognized document format"),
            "err was: {err}"
        );
    }
}
