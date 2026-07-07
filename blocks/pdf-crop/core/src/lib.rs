//! gizza-ai/pdf-crop core — pure PDF page cropping shared by the chat skill
//! block + CLI. No wafer/wasm-bindgen deps.
//!
//! Cropping is non-destructive: for each selected page it writes a new
//! `/CropBox` inset from the page's current visible box (its `/CropBox` if it
//! has one, else its `/MediaBox`, following `/Parent` inheritance). Page
//! content streams are untouched, so a crop can be undone by resetting the box.
//! Margins are given per side (top/bottom/left/right) in `pt` | `mm` | `in`.

use std::collections::BTreeSet;

use lopdf::{Document, Object, ObjectId};

/// Points per unit. PDF user space is points (1/72 inch) with the origin at the
/// bottom-left of the page.
fn unit_to_points(unit: &str) -> Result<f64, String> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "pt" | "point" | "points" => Ok(1.0),
        "mm" | "millimeter" | "millimeters" => Ok(72.0 / 25.4),
        "in" | "inch" | "inches" => Ok(72.0),
        other => Err(format!("unknown unit '{other}' (use pt, mm, or in)")),
    }
}

/// Parse a 1-based page spec into the set of pages to crop.
/// Accepts `all`/`""`, `odd`, `even`, or a comma list of numbers and ranges
/// (`1,3-5,8`). Errors on out-of-range or empty selections.
pub fn parse_pages(spec: &str, total: u32) -> Result<BTreeSet<u32>, String> {
    let s = spec.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("all") {
        return Ok((1..=total).collect());
    }
    if s.eq_ignore_ascii_case("odd") {
        let keep: BTreeSet<u32> = (1..=total).filter(|p| p % 2 == 1).collect();
        if keep.is_empty() {
            return Err(format!("no odd pages (document has {total} page(s))"));
        }
        return Ok(keep);
    }
    if s.eq_ignore_ascii_case("even") {
        let keep: BTreeSet<u32> = (1..=total).filter(|p| p % 2 == 0).collect();
        if keep.is_empty() {
            return Err(format!("no even pages (document has {total} page(s))"));
        }
        return Ok(keep);
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
            (lo..=hi).for_each(|p| {
                keep.insert(p);
            });
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
            return Err(format!(
                "page {max} is out of range (document has {total} page(s))"
            ));
        }
    }
    if keep.is_empty() {
        return Err("no pages selected".into());
    }
    Ok(keep)
}

/// Follow references to the concrete object.
fn deref<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Object> {
    match obj {
        Object::Reference(id) => doc.get_object(*id).ok(),
        other => Some(other),
    }
}

/// A PDF rectangle number can be an Integer or a Real.
fn number(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

/// Read a 4-number rectangle attribute (`MediaBox`/`CropBox`) from a page,
/// walking `/Parent` for inherited attributes. Returns the box normalized so
/// that `[x0, y0, x1, y1]` has `x0 <= x1` and `y0 <= y1`.
fn inherited_rect(doc: &Document, page_id: ObjectId, key: &[u8]) -> Option<[f64; 4]> {
    let mut current = page_id;
    for _ in 0..64 {
        let dict = doc.get_dictionary(current).ok()?;
        if let Ok(raw) = dict.get(key) {
            if let Some(arr) = deref(doc, raw).and_then(|o| o.as_array().ok()) {
                if arr.len() == 4 {
                    let mut r = [0.0f64; 4];
                    let mut ok = true;
                    for (i, e) in arr.iter().enumerate() {
                        match deref(doc, e).and_then(number) {
                            Some(v) => r[i] = v,
                            None => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        let (x0, x1) = (r[0].min(r[2]), r[0].max(r[2]));
                        let (y0, y1) = (r[1].min(r[3]), r[1].max(r[3]));
                        return Some([x0, y0, x1, y1]);
                    }
                }
            }
        }
        match dict.get(b"Parent").and_then(|o| o.as_reference()) {
            Ok(parent) => current = parent,
            Err(_) => return None,
        }
    }
    None
}

/// Crop the selected pages by insetting each side by the given margin.
///
/// `top`/`bottom`/`left`/`right` are the amounts to trim off each edge, in
/// `unit` (`pt`|`mm`|`in`); all must be `>= 0` and at least one must be `> 0`.
/// The crop is applied relative to each page's current visible box.
pub fn crop(
    pdf: &[u8],
    top: f64,
    bottom: f64,
    left: f64,
    right: f64,
    unit: &str,
    pages_spec: &str,
) -> Result<Vec<u8>, String> {
    for (name, v) in [("top", top), ("bottom", bottom), ("left", left), ("right", right)] {
        if !v.is_finite() {
            return Err(format!("{name} margin must be a finite number"));
        }
        if v < 0.0 {
            return Err(format!(
                "{name} margin must be >= 0 (this tool crops inward; it does not add margins)"
            ));
        }
    }
    if top == 0.0 && bottom == 0.0 && left == 0.0 && right == 0.0 {
        return Err("no crop specified — set at least one of top/bottom/left/right > 0".into());
    }
    let factor = unit_to_points(unit)?;
    let (top_pt, bottom_pt, left_pt, right_pt) =
        (top * factor, bottom * factor, left * factor, right * factor);

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
        // Crop relative to the page's current visible box: its CropBox if set,
        // otherwise its MediaBox (both may be inherited from /Parent).
        let [vx0, vy0, vx1, vy1] = inherited_rect(&doc, id, b"CropBox")
            .or_else(|| inherited_rect(&doc, id, b"MediaBox"))
            .ok_or_else(|| format!("page {num} has no MediaBox"))?;

        let nx0 = vx0 + left_pt;
        let ny0 = vy0 + bottom_pt;
        let nx1 = vx1 - right_pt;
        let ny1 = vy1 - top_pt;
        if nx0 >= nx1 || ny0 >= ny1 {
            let w = (vx1 - vx0) / factor;
            let h = (vy1 - vy0) / factor;
            return Err(format!(
                "crop margins exceed page {num} size ({w:.2}{unit} x {h:.2}{unit}) — reduce the margins"
            ));
        }

        let dict = doc
            .get_dictionary_mut(id)
            .map_err(|e| format!("page {num}: {e}"))?;
        dict.set(
            "CropBox",
            vec![
                Object::Real(nx0 as f32),
                Object::Real(ny0 as f32),
                Object::Real(nx1 as f32),
                Object::Real(ny1 as f32),
            ],
        );
    }

    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Object};

    /// Build an N-page PDF; every page has a US-Letter MediaBox (612 x 792 pt).
    fn n_page_pdf(n: u32) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids: Vec<Object> = Vec::new();
        for _ in 0..n {
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
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

    /// Build a 1-page PDF whose MediaBox lives on the parent Pages node (tests
    /// inheritance) — the page dict itself has no MediaBox.
    fn inherited_mediabox_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    fn cropbox_of(pdf: &[u8], page_num: u32) -> Option<[f64; 4]> {
        let doc = Document::load_mem(pdf).unwrap();
        let id = doc.get_pages()[&page_num];
        inherited_rect(&doc, id, b"CropBox")
    }

    #[test]
    fn crops_all_pages_uniformly_in_points() {
        let out = crop(&n_page_pdf(2), 10.0, 10.0, 10.0, 10.0, "pt", "all").unwrap();
        // 612x792 inset 10pt each side -> [10,10,602,782]
        let b = cropbox_of(&out, 1).unwrap();
        assert_eq!(b, [10.0, 10.0, 602.0, 782.0]);
        assert_eq!(cropbox_of(&out, 2).unwrap(), [10.0, 10.0, 602.0, 782.0]);
    }

    #[test]
    fn per_side_margins_are_independent() {
        // trim 72pt (1in) off the top only
        let out = crop(&n_page_pdf(1), 72.0, 0.0, 0.0, 0.0, "pt", "1").unwrap();
        assert_eq!(cropbox_of(&out, 1).unwrap(), [0.0, 0.0, 612.0, 720.0]);
    }

    #[test]
    fn inch_unit_converts_to_points() {
        // 1in top/bottom, 0.5in left/right -> 72pt and 36pt
        let out = crop(&n_page_pdf(1), 1.0, 1.0, 0.5, 0.5, "in", "all").unwrap();
        assert_eq!(cropbox_of(&out, 1).unwrap(), [36.0, 72.0, 576.0, 720.0]);
    }

    #[test]
    fn mm_unit_converts_to_points() {
        // 25.4mm == 1in == 72pt on the left
        let out = crop(&n_page_pdf(1), 0.0, 0.0, 25.4, 0.0, "mm", "all").unwrap();
        let b = cropbox_of(&out, 1).unwrap();
        assert!((b[0] - 72.0).abs() < 0.01, "left edge ~72pt, got {}", b[0]);
        assert!((b[2] - 612.0).abs() < 0.01);
    }

    #[test]
    fn only_selected_pages_get_a_cropbox() {
        let out = crop(&n_page_pdf(3), 20.0, 0.0, 0.0, 0.0, "pt", "2").unwrap();
        assert!(cropbox_of(&out, 1).is_none(), "page 1 untouched");
        assert_eq!(cropbox_of(&out, 2).unwrap(), [0.0, 0.0, 612.0, 772.0]);
        assert!(cropbox_of(&out, 3).is_none(), "page 3 untouched");
    }

    #[test]
    fn odd_even_selectors() {
        let out = crop(&n_page_pdf(4), 5.0, 0.0, 0.0, 0.0, "pt", "odd").unwrap();
        assert!(cropbox_of(&out, 1).is_some());
        assert!(cropbox_of(&out, 2).is_none());
        assert!(cropbox_of(&out, 3).is_some());
    }

    #[test]
    fn crops_relative_to_inherited_mediabox() {
        let out = crop(&inherited_mediabox_pdf(), 10.0, 10.0, 10.0, 10.0, "pt", "all").unwrap();
        assert_eq!(cropbox_of(&out, 1).unwrap(), [10.0, 10.0, 602.0, 782.0]);
    }

    #[test]
    fn rejects_no_margins() {
        assert!(crop(&n_page_pdf(1), 0.0, 0.0, 0.0, 0.0, "pt", "all").is_err());
    }

    #[test]
    fn rejects_negative_margin() {
        assert!(crop(&n_page_pdf(1), -5.0, 0.0, 0.0, 0.0, "pt", "all").is_err());
    }

    #[test]
    fn rejects_margins_exceeding_page() {
        // 400pt left + 400pt right > 612pt width
        assert!(crop(&n_page_pdf(1), 0.0, 0.0, 400.0, 400.0, "pt", "all").is_err());
    }

    #[test]
    fn rejects_unknown_unit() {
        assert!(crop(&n_page_pdf(1), 10.0, 0.0, 0.0, 0.0, "cm", "all").is_err());
    }

    #[test]
    fn rejects_bad_pdf_and_out_of_range_pages() {
        assert!(crop(b"not a pdf", 10.0, 0.0, 0.0, 0.0, "pt", "all").is_err());
        assert!(crop(&n_page_pdf(2), 10.0, 0.0, 0.0, 0.0, "pt", "5").is_err());
    }
}
