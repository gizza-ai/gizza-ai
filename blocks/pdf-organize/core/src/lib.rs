//! gizza-ai/pdf-organize core — reorder, duplicate, delete, and rotate PDF pages
//! in one pass. No wafer/wasm-bindgen deps so it is unit-testable on the host.
//!
//! Unlike `pdf-split`/`pdf-delete-pages` (which treat the page spec as a *set* in
//! original document order), [`organize`] treats `order` as an ordered *sequence*:
//! a page number may appear zero times (deleted), once (kept), or many times
//! (duplicated), in any order (reordered). An optional `rotate` (a multiple of 90)
//! is applied to the selected *original* pages before reordering, so duplicated
//! pages inherit the rotation.

use std::collections::{BTreeSet, HashSet};

use lopdf::{Document, Object};

/// Parse the ordered output sequence. Comma-separated 1-based page numbers and
/// inclusive ranges, in the exact order written; a page may repeat (duplicate)
/// or be omitted (delete). `"all"`/`""` is the identity order; `"reverse"` flips
/// it. A range `"a-b"` with `a > b` (e.g. `"4-2"`) counts down (`4,3,2`).
pub fn parse_order(spec: &str, total: u32) -> Result<Vec<u32>, String> {
    let s = spec.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("all") {
        return Ok((1..=total).collect());
    }
    if s.eq_ignore_ascii_case("reverse") {
        return Ok((1..=total).rev().collect());
    }
    let mut order = Vec::new();
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
            let hi = start.max(end);
            if hi > total {
                return Err(format!("page {hi} is out of range (document has {total} pages)"));
            }
            if start <= end {
                order.extend(start..=end);
            } else {
                order.extend((end..=start).rev());
            }
        } else {
            let p: u32 = part.parse().map_err(|_| format!("invalid page '{part}'"))?;
            if p == 0 {
                return Err("page numbers are 1-based (must be >= 1)".into());
            }
            if p > total {
                return Err(format!("page {p} is out of range (document has {total} pages)"));
            }
            order.push(p);
        }
    }
    if order.is_empty() {
        return Err("no pages selected — give an order like '3,1,2', 'all', or 'reverse'".into());
    }
    Ok(order)
}

/// Parse a 1-based page *set* ("all"/""/"1,3-5,8") for the rotate selection.
pub fn parse_page_set(spec: &str, total: u32) -> Result<BTreeSet<u32>, String> {
    let s = spec.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("all") {
        return Ok((1..=total).collect());
    }
    let mut set = BTreeSet::new();
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
            if hi > total {
                return Err(format!("page {hi} is out of range (document has {total} pages)"));
            }
            set.extend(lo..=hi);
        } else {
            let p: u32 = part.parse().map_err(|_| format!("invalid page '{part}'"))?;
            if p == 0 {
                return Err("page numbers are 1-based (must be >= 1)".into());
            }
            if p > total {
                return Err(format!("page {p} is out of range (document has {total} pages)"));
            }
            set.insert(p);
        }
    }
    if set.is_empty() {
        return Err("no pages selected".into());
    }
    Ok(set)
}

/// The root `/Pages` node id (catalog `/Pages`). Reordering rebuilds its `/Kids`.
fn pages_root(doc: &Document) -> Result<lopdf::ObjectId, String> {
    let root = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(|e| format!("PDF has no /Root: {e}"))?;
    doc.get_dictionary(root)
        .and_then(|d| d.get(b"Pages"))
        .and_then(Object::as_reference)
        .map_err(|e| format!("PDF catalog has no /Pages: {e}"))
}

/// Reorder / duplicate / delete pages per `order`, optionally rotating the
/// selected *original* pages by `degrees` (a multiple of 90) first. Returns the
/// re-serialized PDF.
pub fn organize(
    pdf: &[u8],
    order_spec: &str,
    degrees: i64,
    rotate_spec: &str,
) -> Result<Vec<u8>, String> {
    if degrees % 90 != 0 {
        return Err("rotate must be a multiple of 90 (e.g. 90, 180, 270, -90, or 0)".into());
    }
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let pages = doc.get_pages(); // owned BTreeMap<u32, ObjectId>, ordered by page number
    let total = pages.len() as u32;
    if total == 0 {
        return Err("PDF has no pages".into());
    }
    let order = parse_order(order_spec, total)?;

    // 1) Rotate selected ORIGINAL pages first so any duplicates inherit it.
    if degrees != 0 {
        let rot = parse_page_set(rotate_spec, total)?;
        for (&num, &id) in &pages {
            if !rot.contains(&num) {
                continue;
            }
            let dict = doc
                .get_dictionary_mut(id)
                .map_err(|e| format!("page {num}: {e}"))?;
            let current = dict.get(b"Rotate").and_then(Object::as_i64).unwrap_or(0);
            let next = ((current + degrees) % 360 + 360) % 360;
            dict.set("Rotate", next);
        }
    }

    // 2) Build the new /Kids in output order. First use of a page reuses its
    //    object; a repeat clones the (already-rotated) page dict into a new object.
    let root_id = pages_root(&doc)?;
    let mut new_kids: Vec<lopdf::ObjectId> = Vec::with_capacity(order.len());
    let mut seen: HashSet<u32> = HashSet::new();
    for &p in &order {
        let src_id = pages[&p];
        if seen.insert(p) {
            new_kids.push(src_id);
        } else {
            let dict = doc
                .get_dictionary(src_id)
                .map_err(|e| format!("page {p}: {e}"))?
                .clone();
            new_kids.push(doc.add_object(Object::Dictionary(dict)));
        }
    }

    // 3) Re-parent every output page onto the root and rewrite /Kids + /Count.
    for &id in &new_kids {
        doc.get_dictionary_mut(id)
            .map_err(|e| format!("set parent: {e}"))?
            .set("Parent", root_id);
    }
    let kids: Vec<Object> = new_kids.iter().map(|&id| Object::Reference(id)).collect();
    let count = new_kids.len() as i64;
    let root = doc
        .get_dictionary_mut(root_id)
        .map_err(|e| format!("pages root: {e}"))?;
    root.set("Kids", kids);
    root.set("Count", count);

    // 4) Drop now-unreferenced pages (deleted / old tree nodes), renumber, save.
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
    use lopdf::{dictionary, Object};

    /// Build an N-page PDF where page i (1-based) carries a distinct MediaBox
    /// width `100 + i`, so tests can tell which source page landed where.
    fn labeled_pdf(n: u32) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids: Vec<Object> = Vec::new();
        for i in 1..=n {
            let w = 100 + i as i64;
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), w.into(), 842.into()],
            });
            kids.push(page_id.into());
        }
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => kids, "Count" => n,
            }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    /// The source-page label (1..=N) at each output position, i.e. the observed
    /// order, recovered from the distinct MediaBox widths.
    fn observed_order(pdf: &[u8]) -> Vec<u32> {
        let doc = Document::load_mem(pdf).unwrap();
        let pages = doc.get_pages();
        pages
            .values()
            .map(|&id| {
                let mb = doc
                    .get_object(id)
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"MediaBox")
                    .unwrap()
                    .as_array()
                    .unwrap();
                (mb[2].as_i64().unwrap() - 100) as u32
            })
            .collect()
    }

    fn rotation_at(pdf: &[u8], position: u32) -> i64 {
        let doc = Document::load_mem(pdf).unwrap();
        let id = doc.get_pages()[&position];
        doc.get_object(id)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Rotate")
            .and_then(Object::as_i64)
            .unwrap_or(0)
    }

    #[test]
    fn parse_order_reorders_and_keeps_duplicates() {
        assert_eq!(parse_order("3,1,2", 3).unwrap(), vec![3, 1, 2]);
        assert_eq!(parse_order("1,1,2", 3).unwrap(), vec![1, 1, 2]);
        assert_eq!(parse_order("all", 3).unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_order("", 3).unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_order("reverse", 3).unwrap(), vec![3, 2, 1]);
        assert_eq!(parse_order("2-4", 5).unwrap(), vec![2, 3, 4]);
        assert_eq!(parse_order("4-2", 5).unwrap(), vec![4, 3, 2]); // descending range
    }

    #[test]
    fn parse_order_rejects_bad_input() {
        assert!(parse_order("0", 3).is_err());
        assert!(parse_order("5", 3).is_err()); // out of range
        assert!(parse_order("abc", 3).is_err());
        assert!(parse_order("2-9", 3).is_err()); // range past end
    }

    #[test]
    fn reorders_pages() {
        let out = organize(&labeled_pdf(3), "3,1,2", 0, "all").unwrap();
        assert_eq!(observed_order(&out), vec![3, 1, 2]);
    }

    #[test]
    fn duplicates_pages() {
        let out = organize(&labeled_pdf(3), "1,1,2", 0, "all").unwrap();
        assert_eq!(observed_order(&out), vec![1, 1, 2]);
    }

    #[test]
    fn deletes_omitted_pages() {
        let out = organize(&labeled_pdf(3), "1,3", 0, "all").unwrap();
        assert_eq!(observed_order(&out), vec![1, 3]);
    }

    #[test]
    fn reverse_keyword() {
        let out = organize(&labeled_pdf(4), "reverse", 0, "all").unwrap();
        assert_eq!(observed_order(&out), vec![4, 3, 2, 1]);
    }

    #[test]
    fn rotates_selected_original_pages_only() {
        let out = organize(&labeled_pdf(3), "all", 90, "2").unwrap();
        assert_eq!(rotation_at(&out, 1), 0);
        assert_eq!(rotation_at(&out, 2), 90);
        assert_eq!(rotation_at(&out, 3), 0);
    }

    #[test]
    fn duplicates_inherit_rotation() {
        // page 1 rotated 90, duplicated into positions 1 and 2 — both rotated.
        let out = organize(&labeled_pdf(2), "1,1,2", 90, "1").unwrap();
        assert_eq!(observed_order(&out), vec![1, 1, 2]);
        assert_eq!(rotation_at(&out, 1), 90);
        assert_eq!(rotation_at(&out, 2), 90);
        assert_eq!(rotation_at(&out, 3), 0);
    }

    #[test]
    fn rotation_is_additive_and_normalized() {
        let once = organize(&labeled_pdf(1), "all", 270, "all").unwrap();
        let twice = organize(&once, "all", 180, "all").unwrap(); // 270+180=450 -> 90
        assert_eq!(rotation_at(&twice, 1), 90);
    }

    #[test]
    fn rejects_non_multiple_of_90() {
        assert!(organize(&labeled_pdf(1), "all", 45, "all").is_err());
    }

    #[test]
    fn rejects_bad_pdf_and_out_of_range() {
        assert!(organize(b"not a pdf", "all", 0, "all").is_err());
        assert!(organize(&labeled_pdf(2), "5", 0, "all").is_err());
    }
}
