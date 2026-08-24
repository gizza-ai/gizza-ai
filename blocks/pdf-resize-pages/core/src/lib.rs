//! gizza-ai/pdf-resize-pages core — pure PDF page resizing shared by the chat
//! skill block + CLI. No wafer/wasm-bindgen deps.
//!
//! Each selected page gets a new `/MediaBox` (and matching `/CropBox`) at the
//! requested paper size, and its existing content streams are wrapped in a
//! `q … cm … Q` transform so the artwork is scaled and centred inside the new
//! page instead of being clipped. Text and vectors stay vector (the transform
//! is applied by the PDF viewer/printer), so nothing is rasterised.
//!
//! Geometry notes:
//! * PDF user space is points (1/72"), origin bottom-left.
//! * The source area is the page's current visible box (`/CropBox` if present,
//!   else `/MediaBox`, both possibly inherited via `/Parent`).
//! * A page with `/Rotate 90`/`270` displays its box turned on its side, so the
//!   new `/MediaBox` is written with width/height swapped — the *displayed*
//!   page ends up at the requested size and `/Rotate` is left untouched.
//! * The wrapper also clips to the old visible box, so content that used to sit
//!   outside the `/CropBox` cannot suddenly appear on the resized page.

use std::collections::BTreeSet;

use lopdf::{dictionary, Document, Object, ObjectId, Stream};

/// Smallest / largest page side Acrobat and the PDF spec accept, in points
/// (1/24" … 200").
const MIN_SIDE_PT: f64 = 3.0;
const MAX_SIDE_PT: f64 = 14400.0;

/// Named paper sizes, PORTRAIT, in points. ISO sizes are the mm dimensions
/// converted at 72/25.4 and rounded to whole points (the conventional values).
const PRESETS: &[(&str, f64, f64)] = &[
    ("a0", 2384.0, 3370.0),
    ("a1", 1684.0, 2384.0),
    ("a2", 1191.0, 1684.0),
    ("a3", 842.0, 1191.0),
    ("a4", 595.0, 842.0),
    ("a5", 420.0, 595.0),
    ("a6", 298.0, 420.0),
    ("b4", 709.0, 1001.0),
    ("b5", 499.0, 709.0),
    ("letter", 612.0, 792.0),
    ("legal", 612.0, 1008.0),
    ("tabloid", 792.0, 1224.0),
    ("executive", 522.0, 756.0),
    ("statement", 396.0, 612.0),
];

/// The `size` values the descriptor advertises (presets + `custom`).
pub fn size_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = PRESETS.iter().map(|(n, _, _)| *n).collect();
    v.push("custom");
    v
}

/// Points per unit.
fn unit_to_points(unit: &str) -> Result<f64, String> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "pt" | "point" | "points" => Ok(1.0),
        "mm" | "millimeter" | "millimeters" => Ok(72.0 / 25.4),
        "cm" | "centimeter" | "centimeters" => Ok(720.0 / 25.4),
        "in" | "inch" | "inches" => Ok(72.0),
        other => Err(format!("unknown unit '{other}' (use pt, mm, cm, or in)")),
    }
}

/// Parse a 1-based page spec into the set of pages to resize.
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

/// How the existing artwork is fitted into the new page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScaleMode {
    /// Scale down/up proportionally until the whole old page fits (default).
    Fit,
    /// Scale proportionally until the new page is fully covered (may overflow).
    Fill,
    /// Scale each axis independently to exactly fill the new page (distorts).
    Stretch,
    /// Keep the content at its original size, centred on the new page.
    None,
}

impl ScaleMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "fit" => Ok(Self::Fit),
            "fill" => Ok(Self::Fill),
            "stretch" => Ok(Self::Stretch),
            "none" | "center" | "centre" => Ok(Self::None),
            other => Err(format!(
                "unknown scale '{other}' (use fit, fill, stretch, or none)"
            )),
        }
    }
}

/// Requested orientation of the new page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Orientation {
    /// Keep each page's current orientation (default).
    Auto,
    Portrait,
    Landscape,
}

impl Orientation {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" | "keep" => Ok(Self::Auto),
            "portrait" => Ok(Self::Portrait),
            "landscape" => Ok(Self::Landscape),
            other => Err(format!(
                "unknown orientation '{other}' (use auto, portrait, or landscape)"
            )),
        }
    }
}

/// Everything the caller can choose. `width`/`height` are only read when
/// `size == "custom"`, and are expressed in `unit` like `margin`.
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    pub size: &'a str,
    pub width: f64,
    pub height: f64,
    pub unit: &'a str,
    pub orientation: &'a str,
    pub scale: &'a str,
    /// Extra zoom applied on top of `scale`, in percent (100 = no extra zoom).
    /// Over 100 enlarges the artwork (it may overflow the page); under 100
    /// shrinks it and leaves more whitespace. Always re-centred afterwards.
    pub zoom: f64,
    pub margin: f64,
    pub pages: &'a str,
}

/// What one page ended up as.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageResult {
    pub page: u32,
    /// Displayed page size after the resize, in points.
    pub width_pt: f64,
    pub height_pt: f64,
    /// Content scale factors actually applied (1.0 = unscaled).
    pub scale_x: f64,
    pub scale_y: f64,
}

/// The resized document plus a per-page report.
#[derive(Clone, Debug)]
pub struct Resized {
    pub pdf: Vec<u8>,
    pub total_pages: u32,
    pub pages: Vec<PageResult>,
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
/// walking `/Parent` for inherited attributes. Returns `[x0, y0, x1, y1]`
/// normalized so `x0 <= x1` and `y0 <= y1`.
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

/// The page's inherited `/Rotate`, normalized to 0/90/180/270.
fn inherited_rotate(doc: &Document, page_id: ObjectId) -> i64 {
    let mut current = page_id;
    for _ in 0..64 {
        let Ok(dict) = doc.get_dictionary(current) else {
            return 0;
        };
        if let Ok(raw) = dict.get(b"Rotate") {
            if let Some(v) = deref(doc, raw).and_then(number) {
                return (((v.round() as i64) % 360) + 360) % 360;
            }
        }
        match dict.get(b"Parent").and_then(|o| o.as_reference()) {
            Ok(parent) => current = parent,
            Err(_) => return 0,
        }
    }
    0
}

/// Format a number for a content stream: fixed notation, no exponent, trimmed.
fn num(v: f64) -> String {
    let mut s = format!("{v:.6}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

/// Look up the target page size (portrait, in points) for `size`.
fn target_portrait_pt(opts: &Options, factor: f64) -> Result<(f64, f64), String> {
    let name = opts.size.trim().to_ascii_lowercase();
    let custom = name == "custom";
    let given = opts.width > 0.0 || opts.height > 0.0;
    if custom {
        if !(opts.width.is_finite() && opts.height.is_finite())
            || opts.width <= 0.0
            || opts.height <= 0.0
        {
            return Err(
                "size=custom needs both width and height > 0 (in `unit`), e.g. width=210 height=297 unit=mm"
                    .into(),
            );
        }
        let (w, h) = (opts.width * factor, opts.height * factor);
        for (label, v) in [("width", w), ("height", h)] {
            if !(MIN_SIDE_PT..=MAX_SIDE_PT).contains(&v) {
                return Err(format!(
                    "custom {label} {v:.1}pt is outside the PDF page limits (3pt to 14400pt, i.e. 1/24\" to 200\")"
                ));
            }
        }
        return Ok((w.min(h), w.max(h)));
    }
    if given {
        return Err(format!(
            "width/height only apply to size=custom (got size={name}); drop them or set size=custom"
        ));
    }
    PRESETS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, w, h)| (*w, *h))
        .ok_or_else(|| {
            format!(
                "unknown size '{name}' (use one of {}, or custom with width/height)",
                size_names().join(", ")
            )
        })
}

/// Resize the selected pages of `pdf` to the requested paper size.
pub fn resize(pdf: &[u8], opts: &Options) -> Result<Resized, String> {
    let factor = unit_to_points(opts.unit)?;
    let (portrait_w, portrait_h) = target_portrait_pt(opts, factor)?;
    let orientation = Orientation::parse(opts.orientation)?;
    let mode = ScaleMode::parse(opts.scale)?;
    if !opts.zoom.is_finite() || !(1.0..=1000.0).contains(&opts.zoom) {
        return Err("zoom must be a percentage between 1 and 1000 (100 = no extra zoom)".into());
    }
    let zoom = opts.zoom / 100.0;
    if !opts.margin.is_finite() || opts.margin < 0.0 {
        return Err("margin must be a finite number >= 0".into());
    }
    let margin_pt = opts.margin * factor;
    if 2.0 * margin_pt >= portrait_w {
        return Err(format!(
            "margin {} {} is too large for the target page ({:.1}pt x {:.1}pt) — it leaves no room for content",
            num(opts.margin),
            opts.unit,
            portrait_w,
            portrait_h
        ));
    }

    let mut doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let pages = doc.get_pages();
    let total = pages.len() as u32;
    if total == 0 {
        return Err("PDF has no pages".into());
    }
    let selected = parse_pages(opts.pages, total)?;

    let mut report: Vec<PageResult> = Vec::new();
    for (num_page, id) in pages {
        if !selected.contains(&num_page) {
            continue;
        }
        let [vx0, vy0, vx1, vy1] = inherited_rect(&doc, id, b"CropBox")
            .or_else(|| inherited_rect(&doc, id, b"MediaBox"))
            .ok_or_else(|| format!("page {num_page} has no MediaBox"))?;
        let (sw, sh) = (vx1 - vx0, vy1 - vy0);
        if sw <= 0.0 || sh <= 0.0 {
            return Err(format!("page {num_page} has an empty page box"));
        }
        let rotate = inherited_rotate(&doc, id);
        let quarter_turned = rotate == 90 || rotate == 270;
        // Dimensions as a reader SEES them (after /Rotate).
        let (disp_w, disp_h) = if quarter_turned { (sh, sw) } else { (sw, sh) };

        // Displayed target size.
        let landscape = match orientation {
            Orientation::Auto => disp_w > disp_h,
            Orientation::Portrait => false,
            Orientation::Landscape => true,
        };
        let (tw, th) = if landscape {
            (portrait_h, portrait_w)
        } else {
            (portrait_w, portrait_h)
        };
        // …expressed in the page's own (unrotated) coordinate space.
        let (tw_u, th_u) = if quarter_turned { (th, tw) } else { (tw, th) };

        let (cw, ch) = (tw_u - 2.0 * margin_pt, th_u - 2.0 * margin_pt);
        if cw <= 0.0 || ch <= 0.0 {
            return Err(format!(
                "margin {} {} leaves no content area on page {num_page}",
                num(opts.margin),
                opts.unit
            ));
        }
        let (sx, sy) = match mode {
            ScaleMode::Fit => {
                let s = (cw / sw).min(ch / sh);
                (s, s)
            }
            ScaleMode::Fill => {
                let s = (cw / sw).max(ch / sh);
                (s, s)
            }
            ScaleMode::Stretch => (cw / sw, ch / sh),
            ScaleMode::None => (1.0, 1.0),
        };
        let (sx, sy) = (sx * zoom, sy * zoom);
        let tx = margin_pt + (cw - sw * sx) / 2.0 - vx0 * sx;
        let ty = margin_pt + (ch - sh * sy) / 2.0 - vy0 * sy;

        transform_page(&mut doc, id, sx, sy, tx, ty, [vx0, vy0, sw, sh]);
        scale_annotations(&mut doc, id, sx, sy, tx, ty);

        let page = doc
            .get_dictionary_mut(id)
            .map_err(|e| format!("page {num_page}: {e}"))?;
        let boxv = vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(tw_u as f32),
            Object::Real(th_u as f32),
        ];
        page.set("MediaBox", boxv.clone());
        page.set("CropBox", boxv);
        // The remaining boxes described the OLD geometry; a stale TrimBox
        // larger than the new MediaBox is invalid, so drop them.
        for key in ["BleedBox", "TrimBox", "ArtBox"] {
            page.remove(key.as_bytes());
        }

        report.push(PageResult {
            page: num_page,
            width_pt: tw,
            height_pt: th,
            scale_x: sx,
            scale_y: sy,
        });
    }

    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(Resized {
        pdf: out,
        total_pages: total,
        pages: report,
    })
}

/// Wrap the page's content streams in `q <cm> <clip> … Q`.
///
/// A separate prefix + suffix stream is used (never an edit of the existing
/// stream) so content shared between pages stays shared and untouched. The
/// clip rectangle reproduces the old visible box, so nothing that used to be
/// hidden outside the `/CropBox` becomes visible after the resize.
fn transform_page(
    doc: &mut Document,
    page_id: ObjectId,
    sx: f64,
    sy: f64,
    tx: f64,
    ty: f64,
    clip: [f64; 4],
) {
    let identity = (sx - 1.0).abs() < 1e-9
        && (sy - 1.0).abs() < 1e-9
        && tx.abs() < 1e-9
        && ty.abs() < 1e-9;

    // Existing content, as an object list (a /Contents reference may itself
    // point at an array of streams).
    let mut list: Vec<Object> = match doc.get_dictionary(page_id).and_then(|d| d.get(b"Contents")) {
        Ok(Object::Reference(id)) => match doc.get_object(*id) {
            Ok(Object::Array(arr)) => arr.clone(),
            _ => vec![Object::Reference(*id)],
        },
        Ok(Object::Array(arr)) => arr.clone(),
        Ok(Object::Stream(s)) => {
            let s = s.clone();
            vec![Object::Reference(doc.add_object(s))]
        }
        _ => vec![],
    };
    if list.is_empty() {
        return; // blank page: nothing to transform
    }

    let prefix = if identity {
        format!(
            "q\n{} {} {} {} re W n\n",
            num(clip[0]),
            num(clip[1]),
            num(clip[2]),
            num(clip[3])
        )
    } else {
        format!(
            "q\n{} 0 0 {} {} {} cm\n{} {} {} {} re W n\n",
            num(sx),
            num(sy),
            num(tx),
            num(ty),
            num(clip[0]),
            num(clip[1]),
            num(clip[2]),
            num(clip[3])
        )
    };
    let prefix_id = doc.add_object(Stream::new(dictionary! {}, prefix.into_bytes()));
    let suffix_id = doc.add_object(Stream::new(dictionary! {}, b"\nQ\n".to_vec()));

    let mut new_list = vec![Object::Reference(prefix_id)];
    new_list.append(&mut list);
    new_list.push(Object::Reference(suffix_id));
    if let Ok(page) = doc.get_dictionary_mut(page_id) {
        page.set("Contents", new_list);
    }
}

/// Move the page's annotations with the content (links, highlights, …).
fn scale_annotations(doc: &mut Document, page_id: ObjectId, sx: f64, sy: f64, tx: f64, ty: f64) {
    let ids: Vec<ObjectId> = match doc.get_dictionary(page_id).and_then(|d| d.get(b"Annots")) {
        Ok(Object::Array(arr)) => arr.iter().filter_map(|o| o.as_reference().ok()).collect(),
        Ok(Object::Reference(id)) => match doc.get_object(*id) {
            Ok(Object::Array(arr)) => arr.iter().filter_map(|o| o.as_reference().ok()).collect(),
            _ => vec![],
        },
        _ => vec![],
    };
    for id in ids {
        let Ok(dict) = doc.get_dictionary(id) else {
            continue;
        };
        let rect: Option<Vec<Object>> = dict.get(b"Rect").ok().and_then(|o| o.as_array().ok()).and_then(|a| {
            let v: Vec<f64> = a.iter().filter_map(number).collect();
            (v.len() == 4).then(|| {
                vec![
                    Object::Real((v[0] * sx + tx) as f32),
                    Object::Real((v[1] * sy + ty) as f32),
                    Object::Real((v[2] * sx + tx) as f32),
                    Object::Real((v[3] * sy + ty) as f32),
                ]
            })
        });
        let quads: Option<Vec<Object>> = dict
            .get(b"QuadPoints")
            .ok()
            .and_then(|o| o.as_array().ok())
            .and_then(|a| {
                let v: Vec<f64> = a.iter().filter_map(number).collect();
                (v.len() == a.len() && v.len() % 8 == 0 && !v.is_empty()).then(|| {
                    v.chunks(2)
                        .map(|p| Object::Real((p[0] * sx + tx) as f32))
                        .zip(v.chunks(2).map(|p| Object::Real((p[1] * sy + ty) as f32)))
                        .flat_map(|(x, y)| [x, y])
                        .collect()
                })
            });
        if let Ok(dict) = doc.get_dictionary_mut(id) {
            if let Some(r) = rect {
                dict.set("Rect", r);
            }
            if let Some(q) = quads {
                dict.set("QuadPoints", q);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    fn opts<'a>(size: &'a str, scale: &'a str) -> Options<'a> {
        Options {
            size,
            width: 0.0,
            height: 0.0,
            unit: "mm",
            orientation: "auto",
            scale,
            zoom: 100.0,
            margin: 0.0,
            pages: "all",
        }
    }

    /// Build an N-page PDF; every page is US Letter (612 x 792 pt) with a tiny
    /// content stream so the transform wrapper has something to wrap.
    fn letter_pdf(n: u32) -> Vec<u8> {
        pdf_with(n, [0, 0, 612, 792], None)
    }

    fn pdf_with(n: u32, media: [i64; 4], rotate: Option<i64>) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids: Vec<Object> = Vec::new();
        for _ in 0..n {
            let content_id = doc.add_object(Stream::new(
                dictionary! {},
                b"0 0 1 rg 10 10 100 100 re f\n".to_vec(),
            ));
            let mut page = dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![media[0].into(), media[1].into(), media[2].into(), media[3].into()],
                "Contents" => content_id,
            };
            if let Some(r) = rotate {
                page.set("Rotate", r);
            }
            kids.push(doc.add_object(page).into());
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

    fn mediabox_of(pdf: &[u8], page: u32) -> [f64; 4] {
        let doc = Document::load_mem(pdf).unwrap();
        let id = doc.get_pages()[&page];
        inherited_rect(&doc, id, b"MediaBox").unwrap()
    }

    fn cropbox_of(pdf: &[u8], page: u32) -> Option<[f64; 4]> {
        let doc = Document::load_mem(pdf).unwrap();
        let id = doc.get_pages()[&page];
        inherited_rect(&doc, id, b"CropBox")
    }

    /// The decoded, concatenated content of a page.
    fn content_of(pdf: &[u8], page: u32) -> String {
        let doc = Document::load_mem(pdf).unwrap();
        let id = doc.get_pages()[&page];
        String::from_utf8_lossy(&doc.get_page_content(id).unwrap()).into_owned()
    }

    #[test]
    fn letter_to_a4_sets_the_box_and_scales_content_to_fit() {
        let out = resize(&letter_pdf(1), &opts("a4", "fit")).unwrap();
        assert_eq!(mediabox_of(&out.pdf, 1), [0.0, 0.0, 595.0, 842.0]);
        assert_eq!(cropbox_of(&out.pdf, 1).unwrap(), [0.0, 0.0, 595.0, 842.0]);
        // fit = min(595/612, 842/792) = 0.972222
        let r = out.pages[0];
        assert!((r.scale_x - 595.0 / 612.0).abs() < 1e-9, "{r:?}");
        assert_eq!(r.scale_x, r.scale_y);
        let content = content_of(&out.pdf, 1);
        assert!(
            content.contains("0.972222 0 0 0.972222 0 "),
            "expected a uniform cm transform, got:\n{content}"
        );
        // centred vertically: (842 - 792*595/612)/2 = (842 - 770)/2 = 36
        assert!(content.contains("0.972222 0 0 0.972222 0 36 cm"), "content:\n{content}");
        assert!(content.trim_end().ends_with('Q'), "content:\n{content}");
        // the original drawing survives untouched inside the wrapper
        assert!(content.contains("10 10 100 100 re f"));
    }

    #[test]
    fn a4_to_letter_round_trip_reports_both_axes() {
        let a4 = pdf_with(1, [0, 0, 595, 842], None);
        let out = resize(&a4, &opts("letter", "fit")).unwrap();
        assert_eq!(mediabox_of(&out.pdf, 1), [0.0, 0.0, 612.0, 792.0]);
        let r = out.pages[0];
        // fit = min(612/595, 792/842) = 792/842
        assert!((r.scale_x - 792.0 / 842.0).abs() < 1e-9, "{r:?}");
    }

    #[test]
    fn scale_none_centres_at_original_size() {
        let out = resize(&letter_pdf(1), &opts("a3", "none")).unwrap();
        assert_eq!(mediabox_of(&out.pdf, 1), [0.0, 0.0, 842.0, 1191.0]);
        let r = out.pages[0];
        assert_eq!((r.scale_x, r.scale_y), (1.0, 1.0));
        // centred: tx = (842-612)/2 = 115, ty = (1191-792)/2 = 199.5
        let content = content_of(&out.pdf, 1);
        assert!(content.contains("1 0 0 1 115 199.5 cm"), "content:\n{content}");
    }

    #[test]
    fn stretch_uses_independent_axis_scales() {
        let out = resize(&letter_pdf(1), &opts("a4", "stretch")).unwrap();
        let r = out.pages[0];
        assert!((r.scale_x - 595.0 / 612.0).abs() < 1e-9);
        assert!((r.scale_y - 842.0 / 792.0).abs() < 1e-9);
        assert_ne!(r.scale_x, r.scale_y);
    }

    #[test]
    fn fill_covers_the_page() {
        let out = resize(&letter_pdf(1), &opts("a4", "fill")).unwrap();
        let r = out.pages[0];
        // fill = max(595/612, 842/792) = 842/792 -> content overflows in x
        assert!((r.scale_x - 842.0 / 792.0).abs() < 1e-9, "{r:?}");
        assert!(612.0 * r.scale_x > 595.0);
    }

    #[test]
    fn orientation_auto_keeps_landscape_pages_landscape() {
        let landscape = pdf_with(1, [0, 0, 792, 612], None);
        let out = resize(&landscape, &opts("a4", "fit")).unwrap();
        // A4 landscape = 842 x 595
        assert_eq!(mediabox_of(&out.pdf, 1), [0.0, 0.0, 842.0, 595.0]);
        assert_eq!((out.pages[0].width_pt, out.pages[0].height_pt), (842.0, 595.0));
    }

    #[test]
    fn orientation_can_be_forced() {
        let landscape = pdf_with(1, [0, 0, 792, 612], None);
        let mut o = opts("a4", "fit");
        o.orientation = "portrait";
        let out = resize(&landscape, &o).unwrap();
        assert_eq!(mediabox_of(&out.pdf, 1), [0.0, 0.0, 595.0, 842.0]);

        let mut o = opts("letter", "fit");
        o.orientation = "landscape";
        let out = resize(&letter_pdf(1), &o).unwrap();
        assert_eq!(mediabox_of(&out.pdf, 1), [0.0, 0.0, 792.0, 612.0]);
    }

    #[test]
    fn rotated_pages_get_a_swapped_mediabox_so_the_visible_size_is_right() {
        // A portrait box with /Rotate 90 DISPLAYS as landscape.
        let rotated = pdf_with(1, [0, 0, 612, 792], Some(90));
        let out = resize(&rotated, &opts("a4", "fit")).unwrap();
        // displayed target is A4 landscape (842 x 595) -> box is 595 x 842
        assert_eq!(mediabox_of(&out.pdf, 1), [0.0, 0.0, 595.0, 842.0]);
        assert_eq!((out.pages[0].width_pt, out.pages[0].height_pt), (842.0, 595.0));
        // /Rotate is preserved
        let doc = Document::load_mem(&out.pdf).unwrap();
        let id = doc.get_pages()[&1];
        assert_eq!(inherited_rotate(&doc, id), 90);
    }

    #[test]
    fn custom_size_in_millimetres() {
        let mut o = opts("custom", "fit");
        o.width = 100.0;
        o.height = 150.0;
        o.unit = "mm";
        let out = resize(&letter_pdf(1), &o).unwrap();
        let b = mediabox_of(&out.pdf, 1);
        assert!((b[2] - 100.0 * 72.0 / 25.4).abs() < 0.01, "{b:?}");
        assert!((b[3] - 150.0 * 72.0 / 25.4).abs() < 0.01, "{b:?}");
    }

    #[test]
    fn custom_size_units_agree() {
        for (unit, w, h) in [("in", 8.5, 11.0), ("mm", 215.9, 279.4), ("cm", 21.59, 27.94), ("pt", 612.0, 792.0)] {
            let mut o = opts("custom", "none");
            o.width = w;
            o.height = h;
            o.unit = unit;
            let out = resize(&letter_pdf(1), &o).unwrap();
            let b = mediabox_of(&out.pdf, 1);
            assert!((b[2] - 612.0).abs() < 0.05, "{unit}: {b:?}");
            assert!((b[3] - 792.0).abs() < 0.05, "{unit}: {b:?}");
        }
    }

    #[test]
    fn margin_shrinks_the_content_area() {
        let mut o = opts("letter", "fit");
        o.margin = 1.0;
        o.unit = "in";
        let out = resize(&letter_pdf(1), &o).unwrap();
        // same page size, content fitted into 612-144 x 792-144
        assert_eq!(mediabox_of(&out.pdf, 1), [0.0, 0.0, 612.0, 792.0]);
        let r = out.pages[0];
        let expected = ((612.0 - 144.0) / 612.0f64).min((792.0 - 144.0) / 792.0);
        assert!((r.scale_x - expected).abs() < 1e-9, "{r:?}");
    }

    #[test]
    fn only_selected_pages_change() {
        let mut o = opts("a4", "fit");
        o.pages = "2";
        let out = resize(&letter_pdf(3), &o).unwrap();
        assert_eq!(mediabox_of(&out.pdf, 1), [0.0, 0.0, 612.0, 792.0]);
        assert_eq!(mediabox_of(&out.pdf, 2), [0.0, 0.0, 595.0, 842.0]);
        assert_eq!(mediabox_of(&out.pdf, 3), [0.0, 0.0, 612.0, 792.0]);
        assert_eq!(out.pages.len(), 1);
        assert_eq!(out.total_pages, 3);
    }

    #[test]
    fn odd_even_selectors_work() {
        let mut o = opts("a5", "fit");
        o.pages = "odd";
        let out = resize(&letter_pdf(4), &o).unwrap();
        assert_eq!(out.pages.iter().map(|p| p.page).collect::<Vec<_>>(), vec![1, 3]);
    }

    #[test]
    fn non_zero_origin_boxes_are_normalised_to_zero() {
        let shifted = pdf_with(1, [20, 30, 632, 822], None);
        let out = resize(&shifted, &opts("letter", "none")).unwrap();
        assert_eq!(mediabox_of(&out.pdf, 1), [0.0, 0.0, 612.0, 792.0]);
        // content shifted by -20/-30 so the old visible area lands on the page
        let content = content_of(&out.pdf, 1);
        assert!(content.contains("1 0 0 1 -20 -30 cm"), "content:\n{content}");
    }

    #[test]
    fn annotation_rectangles_follow_the_content() {
        // 1-page letter PDF with a link annotation over the drawn square.
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content_id = doc.add_object(Stream::new(dictionary! {}, b"0 0 1 rg 10 10 100 100 re f\n".to_vec()));
        let annot_id = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![10.into(), 10.into(), 110.into(), 110.into()],
        });
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Annots" => vec![annot_id.into()],
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1 }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut src = Vec::new();
        doc.save_to(&mut src).unwrap();

        let out = resize(&src, &opts("a4", "fit")).unwrap();
        let doc = Document::load_mem(&out.pdf).unwrap();
        let id = doc.get_pages()[&1];
        let annots = doc.get_dictionary(id).unwrap().get(b"Annots").unwrap().as_array().unwrap();
        let aid = annots[0].as_reference().unwrap();
        let rect: Vec<f64> = doc
            .get_dictionary(aid)
            .unwrap()
            .get(b"Rect")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(number)
            .collect();
        let s = 595.0 / 612.0;
        let ty = (842.0 - 792.0 * s) / 2.0;
        assert!((rect[0] - 10.0 * s).abs() < 0.01, "{rect:?}");
        assert!((rect[1] - (10.0 * s + ty)).abs() < 0.01, "{rect:?}");
        assert!((rect[2] - 110.0 * s).abs() < 0.01, "{rect:?}");
    }

    #[test]
    fn stale_trim_and_bleed_boxes_are_dropped() {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content_id = doc.add_object(Stream::new(dictionary! {}, b"0 0 1 rg 10 10 10 10 re f\n".to_vec()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "TrimBox" => vec![9.into(), 9.into(), 603.into(), 783.into()],
            "Contents" => content_id,
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1 }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut src = Vec::new();
        doc.save_to(&mut src).unwrap();

        let out = resize(&src, &opts("a5", "fit")).unwrap();
        let doc = Document::load_mem(&out.pdf).unwrap();
        let id = doc.get_pages()[&1];
        assert!(doc.get_dictionary(id).unwrap().get(b"TrimBox").is_err());
    }

    #[test]
    fn output_reparses_and_keeps_every_page() {
        let out = resize(&letter_pdf(5), &opts("a4", "fit")).unwrap();
        let doc = Document::load_mem(&out.pdf).unwrap();
        assert_eq!(doc.get_pages().len(), 5);
        assert_eq!(out.pages.len(), 5);
    }

    #[test]
    fn rejects_unknown_size_unit_scale_and_orientation() {
        assert!(resize(&letter_pdf(1), &opts("a9", "fit")).is_err());
        assert!(resize(&letter_pdf(1), &opts("a4", "squish")).is_err());
        let mut o = opts("a4", "fit");
        o.unit = "furlong";
        assert!(resize(&letter_pdf(1), &o).is_err());
        let mut o = opts("a4", "fit");
        o.orientation = "sideways";
        assert!(resize(&letter_pdf(1), &o).is_err());
    }

    #[test]
    fn rejects_custom_without_dimensions_and_dimensions_without_custom() {
        assert!(resize(&letter_pdf(1), &opts("custom", "fit")).is_err());
        let mut o = opts("a4", "fit");
        o.width = 200.0;
        o.height = 200.0;
        let err = resize(&letter_pdf(1), &o).unwrap_err();
        assert!(err.contains("size=custom"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_custom_sizes() {
        let mut o = opts("custom", "fit");
        o.unit = "pt";
        o.width = 2.0;
        o.height = 500.0;
        assert!(resize(&letter_pdf(1), &o).is_err());
        o.width = 20000.0;
        assert!(resize(&letter_pdf(1), &o).is_err());
    }

    #[test]
    fn rejects_oversized_margin_and_bad_pages() {
        let mut o = opts("a4", "fit");
        o.margin = 200.0; // mm, wider than A4
        assert!(resize(&letter_pdf(1), &o).is_err());
        let mut o = opts("a4", "fit");
        o.margin = -1.0;
        assert!(resize(&letter_pdf(1), &o).is_err());
        let mut o = opts("a4", "fit");
        o.pages = "9";
        assert!(resize(&letter_pdf(1), &o).is_err());
    }

    #[test]
    fn rejects_a_non_pdf() {
        let err = resize(b"not a pdf at all", &opts("a4", "fit")).unwrap_err();
        assert!(err.contains("failed to parse PDF"), "{err}");
    }

    #[test]
    fn zoom_multiplies_the_fitted_scale_and_recentres() {
        let mut o = opts("letter", "fit");
        o.zoom = 50.0;
        let out = resize(&letter_pdf(1), &o).unwrap();
        // same page, content at half size (fit on an identical box = 1.0)
        assert_eq!(mediabox_of(&out.pdf, 1), [0.0, 0.0, 612.0, 792.0]);
        let r = out.pages[0];
        assert!((r.scale_x - 0.5).abs() < 1e-9, "{r:?}");
        // still centred: tx = (612 - 612*0.5)/2 = 153, ty = (792 - 792*0.5)/2 = 198
        let content = content_of(&out.pdf, 1);
        assert!(content.contains("0.5 0 0 0.5 153 198 cm"), "content:\n{content}");
    }

    #[test]
    fn zoom_over_100_enlarges_past_the_page() {
        let mut o = opts("letter", "none");
        o.zoom = 200.0;
        let out = resize(&letter_pdf(1), &o).unwrap();
        let r = out.pages[0];
        assert_eq!((r.scale_x, r.scale_y), (2.0, 2.0));
        assert!(612.0 * r.scale_x > 612.0);
    }

    #[test]
    fn rejects_out_of_range_zoom() {
        for bad in [0.0, -10.0, 1001.0, f64::NAN] {
            let mut o = opts("a4", "fit");
            o.zoom = bad;
            assert!(resize(&letter_pdf(1), &o).is_err(), "zoom={bad}");
        }
    }

    #[test]
    fn every_advertised_preset_resolves() {
        for name in size_names() {
            if name == "custom" {
                continue;
            }
            let out = resize(&letter_pdf(1), &opts(name, "fit")).unwrap();
            let b = mediabox_of(&out.pdf, 1);
            assert!(b[2] >= MIN_SIDE_PT && b[3] <= MAX_SIDE_PT, "{name}: {b:?}");
        }
    }
}
