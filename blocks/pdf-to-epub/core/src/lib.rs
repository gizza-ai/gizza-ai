//! gizza-ai/pdf-to-epub core — convert a text-based PDF into a reflowable EPUB.
//!
//! No wafer/wasm-bindgen deps so it compiles natively for unit tests and to
//! `wasm32-wasip1` (the `wafer build` target) for the chat block.
//!
//! ## Pipeline
//!
//! 1. Parse the PDF with `lopdf` and extract the embedded selectable text layer
//!    per page (same approach as `pdf-extract-text`: per-chunk failure isolation
//!    so an unparseable font CMap drops one run instead of failing the document).
//! 2. Wrap each page's text in a minimal XHTML chapter (one chapter per page),
//!    paragraphs split on blank lines.
//! 3. Assemble an EPUB 3 (a ZIP container): the `mimetype` entry stored first
//!    and uncompressed (per the OCF spec), `META-INF/container.xml`, a
//!    `content.opf` package document (manifest + spine in page order), a
//!    `toc.ncx` (EPUB 2 nav, for back-compat) plus an XHTML nav doc, and the
//!    per-page XHTML files.
//!
//! Output is deterministic: a fixed ZIP timestamp and a caller-supplied book id
//! mean the same PDF + title + id always produce byte-identical EPUB output.
//!
//! Scanned/image-only PDFs have no text layer and produce empty chapters — this
//! is a text-PDF → reflowable-EPUB converter, not an OCR step.

use std::io::Write;

use lopdf::Document;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Guard against a pathological PDF claiming an absurd page count.
const MAX_PAGES: usize = 10_000;

/// Result of a conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    /// The EPUB file bytes.
    pub epub: Vec<u8>,
    /// Number of chapters written (one per PDF page).
    pub chapters: usize,
    /// Count of text runs skipped because their font encoding failed to parse.
    /// Non-zero means some pages have partial text.
    pub dropped_chunks: usize,
}

/// Convert a text-based PDF into a reflowable EPUB.
///
/// - `bytes` — the raw PDF file.
/// - `title` — the book title (also the `dc:title`). Trimmed; falls back to
///   `"Untitled"` when empty.
/// - `author` — optional author for `dc:creator`. Empty = omitted.
/// - `book_id` — a stable identifier used for `dc:identifier` (e.g. a UUID or a
///   hash). Kept explicit so the output is deterministic for tests/callers.
/// - `preserve_line_breaks` — when `false` (the default behaviour for clean
///   reflow), the hard line wraps a PDF embeds inside a paragraph are joined
///   into flowing text (line-unwrapping). When `true`, each PDF line break is
///   kept as an explicit `<br/>`, preserving the original layout (useful for
///   poetry, code, or tables).
///
/// Returns the EPUB bytes plus chapter/dropped-run counts. Errors when the bytes
/// don't parse as a PDF or the PDF has no pages.
pub fn pdf_to_epub(
    bytes: &[u8],
    title: &str,
    author: &str,
    book_id: &str,
    preserve_line_breaks: bool,
) -> Result<Conversion, String> {
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

    let title = {
        let t = title.trim();
        if t.is_empty() {
            "Untitled".to_string()
        } else {
            t.to_string()
        }
    };
    let author = author.trim();
    let book_id = {
        let b = book_id.trim();
        if b.is_empty() {
            "gizza-ai-pdf-to-epub"
        } else {
            b
        }
    };

    // Extract each page's text (per-chunk isolation).
    let mut dropped_chunks = 0usize;
    let mut pages_text: Vec<String> = Vec::with_capacity(page_numbers.len());
    for n in &page_numbers {
        let mut page_text = String::new();
        for chunk in doc.extract_text_chunks(&[*n]) {
            match chunk {
                Ok(t) => page_text.push_str(&t),
                Err(_) => dropped_chunks += 1,
            }
        }
        pages_text.push(page_text);
    }

    let chapters = pages_text.len();
    let epub = build_epub(&title, author, book_id, &pages_text, preserve_line_breaks)?;

    Ok(Conversion {
        epub,
        chapters,
        dropped_chunks,
    })
}

/// XML-escape text for use in element content / attribute values.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Wrap one page's plain text into an XHTML chapter body. Blank lines split
/// paragraphs. Within a paragraph, intra-paragraph line breaks are joined into
/// flowing text (`preserve_line_breaks = false`, for clean reflow) or kept as
/// explicit `<br/>` (`preserve_line_breaks = true`). Empty pages yield an empty
/// (but well-formed) document.
fn page_to_xhtml(title: &str, idx: usize, text: &str, preserve_line_breaks: bool) -> String {
    let mut body = String::new();
    // Split into paragraphs on blank lines.
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    for para in paragraphs {
        let para = para.trim_matches('\r');
        if para.trim().is_empty() {
            continue;
        }
        let lines: Vec<String> = para
            .split('\n')
            .map(|l| xml_escape(l.trim_end_matches('\r')))
            .filter(|l| !l.trim().is_empty())
            .collect();
        let joined = if preserve_line_breaks {
            // Keep the original layout: each line break is a <br/>.
            lines.join("<br/>")
        } else {
            // Line-unwrap: merge wrapped lines into flowing text for reflow.
            lines.join(" ")
        };
        body.push_str("    <p>");
        body.push_str(&joined);
        body.push_str("</p>\n");
    }
    let heading = format!("Page {}", idx + 1);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="en">
  <head>
    <meta charset="UTF-8"/>
    <title>{title} — {heading}</title>
  </head>
  <body>
    <h2>{heading}</h2>
{body}  </body>
</html>
"#,
        title = xml_escape(title),
        heading = xml_escape(&heading),
        body = body,
    )
}

/// The EPUB package document (content.opf): metadata + manifest + spine.
fn build_opf(title: &str, author: &str, book_id: &str, n_pages: usize) -> String {
    let mut manifest = String::new();
    let mut spine = String::new();
    manifest.push_str(
        "    <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n",
    );
    manifest.push_str(
        "    <item id=\"ncx\" href=\"toc.ncx\" media-type=\"application/x-dtbncx+xml\"/>\n",
    );
    for i in 0..n_pages {
        manifest.push_str(&format!(
            "    <item id=\"page{i}\" href=\"page{i}.xhtml\" media-type=\"application/xhtml+xml\"/>\n"
        ));
        spine.push_str(&format!("    <itemref idref=\"page{i}\"/>\n"));
    }
    let creator = if author.is_empty() {
        String::new()
    } else {
        format!(
            "    <dc:creator id=\"creator\">{}</dc:creator>\n",
            xml_escape(author)
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id" xml:lang="en">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="book-id">{id}</dc:identifier>
    <dc:title>{title}</dc:title>
    <dc:language>en</dc:language>
{creator}    <meta property="dcterms:modified">2020-01-01T00:00:00Z</meta>
  </metadata>
  <manifest>
{manifest}  </manifest>
  <spine toc="ncx">
{spine}  </spine>
</package>
"#,
        id = xml_escape(book_id),
        title = xml_escape(title),
        creator = creator,
        manifest = manifest,
        spine = spine,
    )
}

/// The EPUB 3 navigation document (nav.xhtml).
fn build_nav(title: &str, n_pages: usize) -> String {
    let mut items = String::new();
    for i in 0..n_pages {
        items.push_str(&format!(
            "        <li><a href=\"page{i}.xhtml\">Page {n}</a></li>\n",
            n = i + 1
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="en">
  <head>
    <meta charset="UTF-8"/>
    <title>{title}</title>
  </head>
  <body>
    <nav epub:type="toc" id="toc">
      <h1>Contents</h1>
      <ol>
{items}      </ol>
    </nav>
  </body>
</html>
"#,
        title = xml_escape(title),
        items = items,
    )
}

/// The EPUB 2 NCX table of contents (toc.ncx) — kept for reader back-compat.
fn build_ncx(title: &str, book_id: &str, n_pages: usize) -> String {
    let mut nav_points = String::new();
    for i in 0..n_pages {
        nav_points.push_str(&format!(
            r#"    <navPoint id="navPoint-{n}" playOrder="{n}">
      <navLabel><text>Page {n}</text></navLabel>
      <content src="page{i}.xhtml"/>
    </navPoint>
"#,
            n = i + 1,
            i = i
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd">
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head>
    <meta name="dtb:uid" content="{id}"/>
    <meta name="dtb:depth" content="1"/>
    <meta name="dtb:totalPageCount" content="0"/>
    <meta name="dtb:maxPageNumber" content="0"/>
  </head>
  <docTitle><text>{title}</text></docTitle>
  <navMap>
{nav_points}  </navMap>
</ncx>
"#,
        id = xml_escape(book_id),
        title = xml_escape(title),
        nav_points = nav_points,
    )
}

/// The OCF container descriptor pointing at the package document.
const CONTAINER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

/// Assemble the EPUB ZIP. The `mimetype` entry MUST be the first entry and
/// stored uncompressed (OCF spec) so readers can sniff the type.
fn build_epub(
    title: &str,
    author: &str,
    book_id: &str,
    pages_text: &[String],
    preserve_line_breaks: bool,
) -> Result<Vec<u8>, String> {
    let n = pages_text.len();
    let mut buf: Vec<u8> = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = ZipWriter::new(cursor);

        // A fixed timestamp → deterministic output.
        let ts = zip::DateTime::from_date_and_time(2020, 1, 1, 0, 0, 0)
            .map_err(|e| format!("epub timestamp: {e:?}"))?;
        let stored = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .last_modified_time(ts);
        let deflated = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(ts);

        // 1. mimetype — stored, first.
        zip.start_file("mimetype", stored)
            .map_err(|e| format!("zip mimetype: {e}"))?;
        zip.write_all(b"application/epub+zip")
            .map_err(|e| format!("write mimetype: {e}"))?;

        // 2. container.
        zip.start_file("META-INF/container.xml", deflated)
            .map_err(|e| format!("zip container: {e}"))?;
        zip.write_all(CONTAINER_XML.as_bytes())
            .map_err(|e| format!("write container: {e}"))?;

        // 3. package document.
        zip.start_file("OEBPS/content.opf", deflated)
            .map_err(|e| format!("zip opf: {e}"))?;
        zip.write_all(build_opf(title, author, book_id, n).as_bytes())
            .map_err(|e| format!("write opf: {e}"))?;

        // 4. nav + ncx.
        zip.start_file("OEBPS/nav.xhtml", deflated)
            .map_err(|e| format!("zip nav: {e}"))?;
        zip.write_all(build_nav(title, n).as_bytes())
            .map_err(|e| format!("write nav: {e}"))?;
        zip.start_file("OEBPS/toc.ncx", deflated)
            .map_err(|e| format!("zip ncx: {e}"))?;
        zip.write_all(build_ncx(title, book_id, n).as_bytes())
            .map_err(|e| format!("write ncx: {e}"))?;

        // 5. one XHTML per page.
        for (i, text) in pages_text.iter().enumerate() {
            zip.start_file(format!("OEBPS/page{i}.xhtml"), deflated)
                .map_err(|e| format!("zip page{i}: {e}"))?;
            zip.write_all(page_to_xhtml(title, i, text, preserve_line_breaks).as_bytes())
                .map_err(|e| format!("write page{i}: {e}"))?;
        }

        zip.finish().map_err(|e| format!("finalize epub: {e}"))?;
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};
    use std::io::Read;
    use zip::ZipArchive;

    /// Build a minimal multi-page PDF; each entry becomes a page drawing that string.
    fn build_pdf(pages_text: &[&str]) -> Vec<u8> {
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
        for text in pages_text {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 24.into()]),
                    Operation::new("Td", vec![100.into(), 600.into()]),
                    Operation::new("Tj", vec![Object::string_literal(*text)]),
                    Operation::new("ET", vec![]),
                ],
            };
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
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
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    /// Read one entry from an EPUB zip as a UTF-8 string.
    fn entry(epub: &[u8], name: &str) -> String {
        let mut ar = ZipArchive::new(std::io::Cursor::new(epub)).unwrap();
        let mut f = ar.by_name(name).unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        s
    }

    #[test]
    fn converts_and_produces_valid_epub() {
        let pdf = build_pdf(&["Hello PDF World", "Second Page"]);
        let c = pdf_to_epub(&pdf, "My Book", "Jane Doe", "id-123", false).unwrap();
        assert_eq!(c.chapters, 2);
        assert!(!c.epub.is_empty());

        // mimetype must be the first entry, stored, exact content.
        let mut ar = ZipArchive::new(std::io::Cursor::new(&c.epub)).unwrap();
        {
            let first = ar.by_index(0).unwrap();
            assert_eq!(first.name(), "mimetype");
            assert_eq!(first.compression(), zip::CompressionMethod::Stored);
        }
        let mt = {
            let mut f = ar.by_name("mimetype").unwrap();
            let mut s = String::new();
            f.read_to_string(&mut s).unwrap();
            s
        };
        assert_eq!(mt, "application/epub+zip");

        // OPF has title, author, both pages in the spine.
        let opf = entry(&c.epub, "OEBPS/content.opf");
        assert!(opf.contains("<dc:title>My Book</dc:title>"), "opf: {opf}");
        assert!(opf.contains("Jane Doe"), "opf: {opf}");
        assert!(opf.contains("id-123"), "opf: {opf}");
        assert!(opf.contains("idref=\"page0\""), "opf: {opf}");
        assert!(opf.contains("idref=\"page1\""), "opf: {opf}");

        // The page text made it into a chapter.
        let p0 = entry(&c.epub, "OEBPS/page0.xhtml");
        assert!(p0.contains("Hello"), "page0: {p0}");
    }

    #[test]
    fn deterministic_output() {
        let pdf = build_pdf(&["Same Input"]);
        let a = pdf_to_epub(&pdf, "T", "A", "fixed-id", false).unwrap();
        let b = pdf_to_epub(&pdf, "T", "A", "fixed-id", false).unwrap();
        assert_eq!(a.epub, b.epub, "same inputs must give byte-identical EPUBs");
    }

    #[test]
    fn empty_title_falls_back() {
        let pdf = build_pdf(&["x"]);
        let c = pdf_to_epub(&pdf, "   ", "", "i", false).unwrap();
        let opf = entry(&c.epub, "OEBPS/content.opf");
        assert!(opf.contains("<dc:title>Untitled</dc:title>"), "opf: {opf}");
        // No author → no dc:creator element.
        assert!(!opf.contains("dc:creator"), "opf: {opf}");
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        let err = pdf_to_epub(b"not a pdf at all", "T", "", "i", false).unwrap_err();
        assert!(err.contains("failed to parse PDF"), "got: {err}");
    }

    #[test]
    fn xml_escape_escapes_specials() {
        assert_eq!(xml_escape("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&apos;");
    }

    #[test]
    fn page_xhtml_escapes_and_paragraphs() {
        let x = page_to_xhtml("T", 0, "first para\n\nsecond <b> para", false);
        assert!(x.contains("<p>first para</p>"), "{x}");
        assert!(x.contains("second &lt;b&gt; para"), "{x}");
        assert!(x.contains("<h2>Page 1</h2>"), "{x}");
    }

    #[test]
    fn line_unwrap_vs_preserve() {
        // Two wrapped lines inside one paragraph, separated by a single newline.
        let text = "a wrapped\nparagraph line\n\nnext";
        // Default: unwrap → joined with a space, no <br/>.
        let unwrapped = page_to_xhtml("T", 0, text, false);
        assert!(unwrapped.contains("<p>a wrapped paragraph line</p>"), "{unwrapped}");
        assert!(!unwrapped.contains("<br/>"), "{unwrapped}");
        // Preserve: each line break kept as <br/>.
        let kept = page_to_xhtml("T", 0, text, true);
        assert!(kept.contains("a wrapped<br/>paragraph line"), "{kept}");
    }
}
