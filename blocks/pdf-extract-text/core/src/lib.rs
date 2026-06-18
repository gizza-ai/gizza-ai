//! gizza-ai/pdf-extract-text core — pure PDF text extraction shared by the chat
//! skill block. No wafer/wasm-bindgen deps so it compiles natively for unit
//! tests and to `wasm32-wasip1` (the `wafer build` target) for the block.
//!
//! Backed by `lopdf` (pure-Rust, no native libs) with `default-features =
//! false`: `rayon` (threads) and the `chrono`/`jiff`/`time` date crates are
//! disabled, which keeps the dependency tree thread-free so it links cleanly on
//! wasm. The `rand`/crypto deps lopdf pulls (for *writing* encrypted PDFs) are
//! never exercised on the read/extract path used here.
//!
//! ## Per-chunk failure isolation
//!
//! `lopdf` cannot parse every font's `ToUnicode` CMap — many real PDFs embed
//! CMaps that trip its parser. Rather than fail the whole document (as the
//! convenience `Document::extract_text` does — it `?`-propagates the first
//! chunk error), we use `extract_text_chunks`, which returns one `Result` per
//! text run, and keep every `Ok` chunk while counting the `Err` ones. The
//! result is the text we *can* decode plus a count of skipped runs, instead of
//! an all-or-nothing failure.

use lopdf::Document;

/// Maximum number of pages we will iterate when extracting "all" text. A guard
/// against a pathological PDF claiming an absurd page count.
const MAX_PAGES: usize = 10_000;

/// Result of a text extraction: the decoded text plus the number of text runs
/// whose font encoding could not be decoded (e.g. an unparseable `ToUnicode`
/// CMap). A non-zero `dropped_chunks` means the text is partial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    /// The decoded text (all successfully-decoded runs, in document order).
    pub text: String,
    /// Count of text runs skipped because their font encoding failed to parse.
    pub dropped_chunks: usize,
}

/// Extract the selectable text from a PDF, returning the text directly.
///
/// - `bytes` — the raw PDF file.
/// - `page` — optional 1-based page number. `None` extracts every page;
///   `Some(n)` extracts only page `n`.
///
/// Returns the extracted text on success. Returns `Err` when the bytes don't
/// parse as a PDF or when `page` is out of range. Text runs whose font encoding
/// can't be decoded are silently skipped (see [`extract`] for the dropped
/// count); the text returned is whatever could be decoded.
///
/// Note: this extracts the *embedded selectable text layer* only — it does not
/// OCR scanned/image-only PDFs (those legitimately return empty text).
pub fn extract_text(bytes: &[u8], page: Option<usize>) -> Result<String, String> {
    extract(bytes, page).map(|e| e.text)
}

/// Like [`extract_text`] but also reports how many text runs were skipped
/// because their font encoding could not be decoded.
pub fn extract(bytes: &[u8], page: Option<usize>) -> Result<Extraction, String> {
    let doc = Document::load_mem(bytes).map_err(|e| format!("failed to parse PDF: {e}"))?;

    // `get_pages` returns a map of 1-based page number → object id. Collect and
    // sort so output order is deterministic.
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

    let selected: Vec<u32> = match page {
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

    // Extract per page so a `page` selection and the "all" path share one code
    // path and pages stay visually separated by a blank line.
    let mut pages_text: Vec<String> = Vec::with_capacity(selected.len());
    let mut dropped_chunks = 0usize;
    for n in selected {
        let mut page_text = String::new();
        for chunk in doc.extract_text_chunks(&[n]) {
            match chunk {
                Ok(t) => page_text.push_str(&t),
                Err(_) => dropped_chunks += 1,
            }
        }
        pages_text.push(page_text);
    }

    Ok(Extraction {
        text: pages_text.join("\n\n"),
        dropped_chunks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    /// Build a minimal multi-page PDF; each entry in `pages_text` becomes one
    /// page whose content stream draws that string.
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

    #[test]
    fn extracts_all_text() {
        let pdf = build_pdf(&["Hello PDF World"]);
        let out = extract_text(&pdf, None).unwrap();
        assert!(out.contains("Hello"), "got: {out:?}");
        assert!(out.contains("World"), "got: {out:?}");
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        let err = extract_text(b"this is plainly not a pdf", None).unwrap_err();
        assert!(err.contains("failed to parse PDF"), "got: {err}");
    }

    #[test]
    fn selects_a_single_page() {
        let pdf = build_pdf(&["First Page Alpha", "Second Page Beta"]);
        let page2 = extract_text(&pdf, Some(2)).unwrap();
        assert!(page2.contains("Beta"), "page 2 should contain Beta: {page2:?}");
        assert!(
            !page2.contains("Alpha"),
            "page 2 should not contain page 1 text: {page2:?}"
        );
    }

    #[test]
    fn page_out_of_range_errors() {
        let pdf = build_pdf(&["Only One Page"]);
        let err = extract_text(&pdf, Some(5)).unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn all_pages_joined() {
        let pdf = build_pdf(&["Alpha One", "Beta Two"]);
        let out = extract_text(&pdf, None).unwrap();
        assert!(out.contains("Alpha"), "got: {out:?}");
        assert!(out.contains("Beta"), "got: {out:?}");
    }

    #[test]
    fn clean_pdf_reports_no_dropped_chunks() {
        let pdf = build_pdf(&["Standard Encoding Text"]);
        let ex = extract(&pdf, None).unwrap();
        assert_eq!(ex.dropped_chunks, 0, "standard-encoded text drops nothing");
        assert!(ex.text.contains("Standard"));
    }
}
