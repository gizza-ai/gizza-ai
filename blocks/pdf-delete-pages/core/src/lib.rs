//! gizza-ai/pdf-delete-pages core — pure page removal shared by the chat skill block.
//! No wafer/wasm-bindgen deps so it is unit-testable on the host.
//!
//! [`delete_pages`] removes the pages named by a 1-based page spec (e.g. `"2,5"`
//! or `"3-7"`) and keeps the rest, then re-serializes. The spec is a *set* of
//! pages to drop; the surviving pages stay in their original document order. This
//! is the inverse of `pdf-split`, which keeps the named pages and drops the rest,
//! but the two tools share the same page-spec grammar on purpose.

use std::collections::BTreeSet;

use lopdf::Document;

/// Parse a page spec like `"2,5,8-10"` (1-based, inclusive ranges) into the sorted
/// set of pages to DELETE. `total` bounds validation. Whitespace is ignored;
/// `"odd"`/`"even"` select those page sets. Unlike `pdf-split`, an empty spec is an
/// error (you must say which pages to delete) and `"all"` is rejected below by the
/// caller (deleting every page would leave an empty document).
pub fn parse_pages(spec: &str, total: u32) -> Result<BTreeSet<u32>, String> {
    let s = spec.trim();
    if s.is_empty() {
        return Err("specify which pages to delete, e.g. '2,5' or '3-7'".into());
    }
    if s.eq_ignore_ascii_case("all") {
        return Err(format!("cannot delete all {total} pages — the output must keep at least one page"));
    }
    if s.eq_ignore_ascii_case("odd") {
        return Ok((1..=total).filter(|p| p % 2 == 1).collect());
    }
    if s.eq_ignore_ascii_case("even") {
        return Ok((1..=total).filter(|p| p % 2 == 0).collect());
    }
    let mut drop = BTreeSet::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let start: u32 = a.trim().parse().map_err(|_| format!("invalid page '{a}'"))?;
            let end: u32 = b.trim().parse().map_err(|_| format!("invalid page '{b}'"))?;
            if start == 0 || end == 0 {
                return Err("page numbers are 1-based (must be >= 1)".into());
            }
            let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
            for p in lo..=hi {
                drop.insert(p);
            }
        } else {
            let p: u32 = part.parse().map_err(|_| format!("invalid page '{part}'"))?;
            if p == 0 {
                return Err("page numbers are 1-based (must be >= 1)".into());
            }
            drop.insert(p);
        }
    }
    if let Some(&max) = drop.iter().next_back() {
        if max > total {
            return Err(format!("page {max} is out of range (document has {total} pages)"));
        }
    }
    if drop.is_empty() {
        return Err("no pages selected to delete".into());
    }
    Ok(drop)
}

/// Remove the pages named by `pages_spec` (1-based) and return the serialized PDF
/// with the surviving pages in their original order. Errors as a human-readable
/// string on a parse/range failure, or if the selection would delete every page.
pub fn delete_pages(pdf: &[u8], pages_spec: &str) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let total = doc.get_pages().len() as u32;
    if total == 0 {
        return Err("PDF has no pages".into());
    }
    let drop = parse_pages(pages_spec, total)?;
    if drop.len() as u32 >= total {
        return Err(format!(
            "that selection removes all {total} pages — the output must keep at least one page"
        ));
    }
    // delete_pages is 1-based and takes the pages to remove.
    let to_delete: Vec<u32> = drop.into_iter().collect();
    doc.delete_pages(&to_delete);
    doc.prune_objects();
    doc.renumber_objects();
    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    /// Build an N-page PDF (each page a trivial labeled content stream).
    fn n_page_pdf(n: u32) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Courier",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let mut kids: Vec<Object> = Vec::new();
        for i in 0..n {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 24.into()]),
                    Operation::new("Td", vec![100.into(), 700.into()]),
                    Operation::new("Tj", vec![Object::string_literal(format!("Page {}", i + 1))]),
                    Operation::new("ET", vec![]),
                ],
            };
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "Resources" => resources_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            });
            kids.push(page_id.into());
        }
        let pages = dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => n };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).expect("serialize fixture pdf");
        out
    }

    fn page_count(bytes: &[u8]) -> usize {
        Document::load_mem(bytes).expect("output should parse").get_pages().len()
    }

    #[test]
    fn parse_pages_handles_lists_and_ranges() {
        assert_eq!(parse_pages("2,5,8-10", 12).unwrap(), [2, 5, 8, 9, 10].into_iter().collect());
        // reversed range is normalized
        assert_eq!(parse_pages("5-3", 10).unwrap(), [3, 4, 5].into_iter().collect());
    }

    #[test]
    fn parse_pages_odd_even_keywords() {
        assert_eq!(parse_pages("odd", 6).unwrap(), [1, 3, 5].into_iter().collect());
        assert_eq!(parse_pages("even", 6).unwrap(), [2, 4, 6].into_iter().collect());
        assert_eq!(parse_pages("EVEN", 4).unwrap(), [2, 4].into_iter().collect());
    }

    #[test]
    fn parse_pages_rejects_empty_all_and_bad_input() {
        assert!(parse_pages("", 10).is_err());
        assert!(parse_pages("all", 10).is_err());
        assert!(parse_pages("11", 10).is_err());
        assert!(parse_pages("0", 10).is_err());
        assert!(parse_pages("abc", 10).is_err());
    }

    #[test]
    fn delete_removes_only_selected_pages() {
        let pdf = n_page_pdf(5);
        assert_eq!(page_count(&pdf), 5);
        let out = delete_pages(&pdf, "2,4").unwrap();
        assert_eq!(page_count(&out), 3);
    }

    #[test]
    fn delete_range_keeps_the_rest() {
        let pdf = n_page_pdf(6);
        let out = delete_pages(&pdf, "2-4").unwrap();
        assert_eq!(page_count(&out), 3);
    }

    #[test]
    fn delete_even_keeps_odd() {
        let pdf = n_page_pdf(6);
        let out = delete_pages(&pdf, "even").unwrap();
        assert_eq!(page_count(&out), 3);
    }

    #[test]
    fn delete_all_pages_errors() {
        let pdf = n_page_pdf(3);
        assert!(delete_pages(&pdf, "1-3").is_err());
        assert!(delete_pages(&pdf, "all").is_err());
    }

    #[test]
    fn delete_out_of_range_errors() {
        let pdf = n_page_pdf(3);
        assert!(delete_pages(&pdf, "5").is_err());
    }

    #[test]
    fn delete_empty_spec_errors() {
        let pdf = n_page_pdf(3);
        assert!(delete_pages(&pdf, "").is_err());
    }

    #[test]
    fn delete_rejects_non_pdf() {
        assert!(delete_pages(b"not a pdf", "1").is_err());
    }
}
