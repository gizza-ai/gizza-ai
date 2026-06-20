//! gizza-ai/pdf-rotate core — pure PDF page rotation shared by the chat skill
//! block + CLI. No wafer/wasm-bindgen deps. Adds `degrees` (a multiple of 90) to
//! each selected page's `/Rotate` entry (normalized to 0/90/180/270) and
//! re-serializes. Rotation is non-destructive metadata — content is untouched.

use std::collections::BTreeSet;

use lopdf::{Document, Object};

/// Parse a 1-based page spec ("all"/""/"1,3-5,8") into the set of pages to act on.
pub fn parse_pages(spec: &str, total: u32) -> Result<BTreeSet<u32>, String> {
    let s = spec.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("all") {
        return Ok((1..=total).collect());
    }
    let mut keep = BTreeSet::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let (start, end): (u32, u32) = (
                a.trim().parse().map_err(|_| format!("invalid page '{a}'"))?,
                b.trim().parse().map_err(|_| format!("invalid page '{b}'"))?,
            );
            if start == 0 || end == 0 {
                return Err("page numbers are 1-based (>= 1)".into());
            }
            let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
            (lo..=hi).for_each(|p| { keep.insert(p); });
        } else {
            let p: u32 = part.parse().map_err(|_| format!("invalid page '{part}'"))?;
            if p == 0 {
                return Err("page numbers are 1-based (>= 1)".into());
            }
            keep.insert(p);
        }
    }
    if let Some(&max) = keep.iter().next_back() {
        if max > total {
            return Err(format!("page {max} is out of range (document has {total} pages)"));
        }
    }
    if keep.is_empty() {
        return Err("no pages selected".into());
    }
    Ok(keep)
}

/// Rotate the selected pages by `degrees` (a multiple of 90, may be negative).
pub fn rotate(pdf: &[u8], degrees: i64, pages_spec: &str) -> Result<Vec<u8>, String> {
    if degrees % 90 != 0 {
        return Err("degrees must be a multiple of 90 (e.g. 90, 180, 270, -90)".into());
    }
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let pages = doc.get_pages();
    let total = pages.len() as u32;
    if total == 0 {
        return Err("PDF has no pages".into());
    }
    let selected = parse_pages(pages_spec, total)?;
    for (num, id) in pages {
        if !selected.contains(&num) {
            continue;
        }
        let dict = doc
            .get_dictionary_mut(id)
            .map_err(|e| format!("page {num}: {e}"))?;
        let current = dict.get(b"Rotate").and_then(Object::as_i64).unwrap_or(0);
        let next = ((current + degrees) % 360 + 360) % 360;
        dict.set("Rotate", next);
    }
    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Object};

    fn n_page_pdf(n: u32) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids: Vec<Object> = Vec::new();
        for _ in 0..n {
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            });
            kids.push(page_id.into());
        }
        doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => kids, "Count" => n,
        }));
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    fn rotation_of(pdf: &[u8], page_num: u32) -> i64 {
        let doc = Document::load_mem(pdf).unwrap();
        let id = doc.get_pages()[&page_num];
        doc.get_object(id).unwrap().as_dict().unwrap()
            .get(b"Rotate").and_then(Object::as_i64).unwrap_or(0)
    }

    #[test]
    fn rotates_all_pages() {
        let out = rotate(&n_page_pdf(3), 90, "all").unwrap();
        assert_eq!(rotation_of(&out, 1), 90);
        assert_eq!(rotation_of(&out, 3), 90);
    }

    #[test]
    fn rotates_only_selected_pages() {
        let out = rotate(&n_page_pdf(3), 180, "2").unwrap();
        assert_eq!(rotation_of(&out, 1), 0);
        assert_eq!(rotation_of(&out, 2), 180);
    }

    #[test]
    fn rotation_is_additive_and_normalized() {
        let once = rotate(&n_page_pdf(1), 270, "all").unwrap();
        let twice = rotate(&once, 180, "all").unwrap(); // 270+180=450 -> 90
        assert_eq!(rotation_of(&twice, 1), 90);
    }

    #[test]
    fn negative_rotation_normalized() {
        let out = rotate(&n_page_pdf(1), -90, "all").unwrap(); // -> 270
        assert_eq!(rotation_of(&out, 1), 270);
    }

    #[test]
    fn rejects_non_multiple_of_90() {
        assert!(rotate(&n_page_pdf(1), 45, "all").is_err());
    }

    #[test]
    fn rejects_bad_inputs() {
        assert!(rotate(b"not a pdf", 90, "all").is_err());
        assert!(rotate(&n_page_pdf(2), 90, "5").is_err());
    }
}
