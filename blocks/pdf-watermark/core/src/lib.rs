//! gizza-ai/pdf-watermark core — stamp a text watermark onto an existing PDF.
//! Pure-Rust (`lopdf`), no font embedding: uses the built-in base-14 Type1 fonts
//! (Helvetica / Times-Roman / Courier) so the output stays small and the tool
//! runs on every backend. Each selected page gets an overlaid, rotated text
//! stamp whose graphics state is isolated from the page's own content (a leading
//! `q` / trailing `Q` restore), so existing content is never disturbed.
//!
//! Unlike `pdf-page-numbers` (an upright, per-page incrementing corner label),
//! a watermark is one FIXED string stamped identically on every page — large,
//! faint and rotated (diagonal by default), optionally TILED in a grid across
//! the whole page (the "mosaic" layout).

use std::collections::{BTreeSet, HashSet};

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Dictionary, Document, Object, Stream};

const FONT_RES_NAME: &str = "GZWM";
const EXTG_RES_NAME: &str = "GZWMgs";

/// Distance kept between the watermark and the page edge for non-centre
/// positions, in points (72 pt = 1 inch). Watermarks don't expose a margin
/// knob — a fixed sensible inset keeps the mark clear of the trim.
const EDGE_INSET: f64 = 24.0;

/// Upper bound on the number of stamps drawn per page in tile mode, so a tiny
/// font on a huge page can't balloon the content stream.
const MAX_TILES: usize = 600;

/// Horizontal placement of the stamp anchor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HAlign {
    Left,
    Center,
    Right,
}

/// Vertical placement of the stamp anchor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum VAlign {
    Top,
    Middle,
    Bottom,
}

/// Where on the page the watermark is anchored (its centre point).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Position {
    h: HAlign,
    v: VAlign,
}

impl Position {
    fn parse(s: &str) -> Result<Self, String> {
        use HAlign::*;
        use VAlign::*;
        let (h, v) = match s.trim().to_ascii_lowercase().as_str() {
            "center" | "centre" | "middle" | "middle-center" | "" => (Center, Middle),
            "top-left" => (Left, Top),
            "top-center" | "top" => (Center, Top),
            "top-right" => (Right, Top),
            "middle-left" | "left" => (Left, Middle),
            "middle-right" | "right" => (Right, Middle),
            "bottom-left" => (Left, Bottom),
            "bottom-center" | "bottom" => (Center, Bottom),
            "bottom-right" => (Right, Bottom),
            other => {
                return Err(format!(
                    "unknown position '{other}': use center, top-left, top-center, top-right, middle-left, middle-right, bottom-left, bottom-center, or bottom-right"
                ))
            }
        };
        Ok(Position { h, v })
    }
}

/// A base-14 Type1 font (no embedding required).
#[derive(Clone, Copy)]
enum FontFamily {
    Helvetica,
    Times,
    Courier,
}

impl FontFamily {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "helvetica" | "arial" | "sans" | "" => FontFamily::Helvetica,
            "times" | "times-roman" | "serif" => FontFamily::Times,
            "courier" | "mono" | "monospace" => FontFamily::Courier,
            other => {
                return Err(format!(
                    "unknown font '{other}': use helvetica, times, or courier"
                ))
            }
        })
    }
    fn base_font(self) -> &'static str {
        match self {
            FontFamily::Helvetica => "Helvetica",
            FontFamily::Times => "Times-Roman",
            FontFamily::Courier => "Courier",
        }
    }
}

/// All watermark options (raw forms as they arrive from the block/CLI).
pub struct Options {
    pub text: String,
    pub position: String,
    pub rotation: f64,
    pub font: String,
    pub font_size: f64,
    pub color: String,
    pub opacity: f64,
    pub tile: bool,
    pub pages: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            text: "CONFIDENTIAL".to_string(),
            position: "center".to_string(),
            rotation: 45.0,
            font: "helvetica".to_string(),
            font_size: 48.0,
            color: "#808080".to_string(),
            opacity: 0.3,
            tile: false,
            pages: "all".to_string(),
        }
    }
}

/// Parse a 1-based page spec ("all"/""/"1,3-5,8") into the set of pages to stamp.
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
            let a = a.trim();
            let b = b.trim();
            let start: u32 = a.parse().map_err(|_| format!("invalid page '{a}'"))?;
            // An open-ended range like "3-" means page 3 to the end.
            let end: u32 = if b.is_empty() {
                total
            } else {
                b.parse().map_err(|_| format!("invalid page '{b}'"))?
            };
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
                "page {max} is out of range (document has {total} pages)"
            ));
        }
    }
    if keep.is_empty() {
        return Err("no pages selected".into());
    }
    Ok(keep)
}

/// Parse a hex colour ("#rrggbb", "rrggbb", "#rgb", "rgb") into RGB in 0.0..=1.0.
fn parse_color(s: &str) -> Result<[f64; 3], String> {
    let h = s.trim().trim_start_matches('#');
    let bytes = match h.len() {
        6 => {
            let r = u8::from_str_radix(&h[0..2], 16);
            let g = u8::from_str_radix(&h[2..4], 16);
            let b = u8::from_str_radix(&h[4..6], 16);
            (r, g, b)
        }
        3 => {
            let dup = |c: &str| u8::from_str_radix(&format!("{c}{c}"), 16);
            (dup(&h[0..1]), dup(&h[1..2]), dup(&h[2..3]))
        }
        _ => {
            return Err(format!(
                "invalid color '{s}': use a hex code like #808080, #ff0000, or #f00"
            ))
        }
    };
    match bytes {
        (Ok(r), Ok(g), Ok(b)) => Ok([r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0]),
        _ => Err(format!(
            "invalid color '{s}': use a hex code like #808080, #ff0000, or #f00"
        )),
    }
}

/// Advance width (1000-em units) of `ch` in the given base-14 font. Covers the
/// common Latin glyphs a watermark uses; anything else falls back to a sane
/// average so tiling/centring stay stable.
fn glyph_width(font: FontFamily, ch: char) -> u32 {
    if let FontFamily::Courier = font {
        return 600; // Courier is monospaced.
    }
    let helvetica = matches!(font, FontFamily::Helvetica);
    match ch {
        ' ' => {
            if helvetica {
                278
            } else {
                250
            }
        }
        '0'..='9' => {
            if helvetica {
                556
            } else {
                500
            }
        }
        '-' | '\u{2013}' => 333,
        '.' | ',' => {
            if helvetica {
                278
            } else {
                250
            }
        }
        ':' | ';' => 278,
        '/' => 278,
        '(' | ')' => 333,
        _ => {
            let widths = if helvetica { HELV } else { TIMES };
            let (lo, table): (char, &[u16]) = if ch.is_ascii_uppercase() {
                ('A', &widths.0)
            } else if ch.is_ascii_lowercase() {
                ('a', &widths.1)
            } else {
                return if helvetica { 556 } else { 500 };
            };
            table[(ch as u8 - lo as u8) as usize] as u32
        }
    }
}

// (uppercase A-Z, lowercase a-z) AFM advance widths for Helvetica / Times-Roman.
struct FontWidths([u16; 26], [u16; 26]);
const HELV: FontWidths = FontWidths(
    [
        667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722,
        667, 611, 722, 667, 944, 667, 667, 611,
    ],
    [
        556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333,
        500, 278, 556, 500, 722, 500, 500, 500,
    ],
);
const TIMES: FontWidths = FontWidths(
    [
        722, 667, 667, 722, 611, 556, 722, 722, 333, 389, 722, 611, 889, 722, 722, 556, 722, 667,
        556, 611, 722, 722, 944, 722, 722, 611,
    ],
    [
        444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500, 278, 778, 500, 500, 500, 500, 333,
        389, 278, 500, 500, 722, 500, 500, 444,
    ],
);

/// Width of the watermark text in points.
fn text_width(font: FontFamily, size: f64, s: &str) -> f64 {
    let em: f64 = s.chars().map(|c| glyph_width(font, c) as f64).sum();
    em * size / 1000.0
}

/// Escape the watermark into WinAnsi/Latin-1 bytes for a PDF literal string;
/// code points beyond 0xFF (which the base-14 encoding can't represent) become '?'.
fn pdf_literal(label: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(label.len() + 2);
    for ch in label.chars() {
        let b = if (ch as u32) <= 0xFF { ch as u8 } else { b'?' };
        match b {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'(' => out.extend_from_slice(b"\\("),
            b')' => out.extend_from_slice(b"\\)"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            _ => out.push(b),
        }
    }
    out
}

/// The effective MediaBox for a page, walking up the Parent chain; falls back to
/// US Letter (612×792) when none is declared.
fn effective_mediabox(doc: &Document, page_id: (u32, u16)) -> [f64; 4] {
    let mut cur = page_id;
    let mut seen = HashSet::new();
    loop {
        let Ok(d) = doc.get_dictionary(cur) else {
            break;
        };
        if let Ok(mb) = d.get(b"MediaBox").and_then(Object::as_array) {
            if mb.len() == 4 {
                let mut v = [0.0f64; 4];
                for (i, o) in mb.iter().enumerate() {
                    v[i] = o.as_float().map(|x| x as f64).unwrap_or(0.0);
                }
                return v;
            }
        }
        match d.get(b"Parent").and_then(Object::as_reference) {
            Ok(pid) if seen.insert(pid) => cur = pid,
            _ => break,
        }
    }
    [0.0, 0.0, 612.0, 792.0]
}

/// The nearest effective /Resources dictionary for a page (cloned), or empty.
fn effective_resources(doc: &Document, page_id: (u32, u16)) -> Dictionary {
    let mut cur = page_id;
    let mut seen = HashSet::new();
    loop {
        let Ok(d) = doc.get_dictionary(cur) else {
            break;
        };
        match d.get(b"Resources") {
            Ok(Object::Dictionary(rd)) => return rd.clone(),
            Ok(Object::Reference(rid)) => {
                if let Ok(rd) = doc.get_dictionary(*rid) {
                    return rd.clone();
                }
            }
            _ => {}
        }
        match d.get(b"Parent").and_then(Object::as_reference) {
            Ok(pid) if seen.insert(pid) => cur = pid,
            _ => break,
        }
    }
    Dictionary::new()
}

/// Give the page a complete direct /Resources (nearest effective set + our font
/// and, if `alpha < 1`, a transparency ExtGState). Nothing inherited is lost.
fn install_resources(
    doc: &mut Document,
    page_id: (u32, u16),
    font_id: (u32, u16),
    extg_id: Option<(u32, u16)>,
) {
    let mut res = effective_resources(doc, page_id);

    let mut fonts: Dictionary = match res.get(b"Font") {
        Ok(Object::Reference(fid)) => doc.get_dictionary(*fid).cloned().unwrap_or_default(),
        Ok(Object::Dictionary(d)) => d.clone(),
        _ => Dictionary::new(),
    };
    fonts.set(FONT_RES_NAME, Object::Reference(font_id));
    res.set("Font", fonts);

    if let Some(gid) = extg_id {
        let mut gs: Dictionary = match res.get(b"ExtGState") {
            Ok(Object::Reference(rid)) => doc.get_dictionary(*rid).cloned().unwrap_or_default(),
            Ok(Object::Dictionary(d)) => d.clone(),
            _ => Dictionary::new(),
        };
        gs.set(EXTG_RES_NAME, Object::Reference(gid));
        res.set("ExtGState", gs);
    }

    if let Ok(page) = doc.get_dictionary_mut(page_id) {
        page.set("Resources", res);
    }
}

/// Wrap an overlay content stream around the page's existing content: a leading
/// `q` saves the pristine graphics state; the overlay begins with `Q` to restore
/// it (so any transform the page left behind is undone) before drawing.
fn stamp_page(doc: &mut Document, page_id: (u32, u16), overlay_ops: Vec<Operation>) {
    let prefix = Content {
        operations: vec![Operation::new("q", vec![])],
    };
    let prefix_id = doc.add_object(Stream::new(
        dictionary! {},
        prefix.encode().unwrap_or_default(),
    ));

    let mut ops = vec![Operation::new("Q", vec![])];
    ops.extend(overlay_ops);
    let suffix = Content { operations: ops };
    let suffix_id = doc.add_object(Stream::new(
        dictionary! {},
        suffix.encode().unwrap_or_default(),
    ));

    let mut list: Vec<Object> = match doc.get_dictionary(page_id).and_then(|d| d.get(b"Contents")) {
        Ok(Object::Reference(id)) => vec![Object::Reference(*id)],
        Ok(Object::Array(arr)) => arr.clone(),
        _ => vec![],
    };
    let mut new_list = vec![Object::Reference(prefix_id)];
    new_list.append(&mut list);
    new_list.push(Object::Reference(suffix_id));

    if let Ok(page) = doc.get_dictionary_mut(page_id) {
        page.set("Contents", new_list);
    }
}

/// Emit the operators that draw the label centred on `(px, py)`, rotated by the
/// pre-computed `(cos, sin)`, in the current colour/opacity graphics state.
/// `tw` is the label width in points; the text is offset so its centre sits on
/// the anchor point (baseline dropped ~0.35·size so glyphs straddle the origin).
fn draw_one(ops: &mut Vec<Operation>, label_bytes: &[u8], size: f64, tw: f64, cos: f64, sin: f64, px: f64, py: f64) {
    ops.push(Operation::new(
        "cm",
        vec![cos.into(), sin.into(), (-sin).into(), cos.into(), px.into(), py.into()],
    ));
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new("Tf", vec![FONT_RES_NAME.into(), size.into()]));
    ops.push(Operation::new("Td", vec![(-tw / 2.0).into(), (-size * 0.35).into()]));
    ops.push(Operation::new(
        "Tj",
        vec![Object::String(label_bytes.to_vec(), lopdf::StringFormat::Literal)],
    ));
    ops.push(Operation::new("ET", vec![]));
}

/// Stamp the text watermark onto `pdf` per `opts`, returning the new PDF bytes.
pub fn add_watermark(pdf: &[u8], opts: &Options) -> Result<Vec<u8>, String> {
    let text = opts.text.trim();
    if text.is_empty() {
        return Err("watermark text is required (e.g. 'CONFIDENTIAL' or 'DRAFT')".into());
    }

    let position = Position::parse(&opts.position)?;
    let family = FontFamily::parse(&opts.font)?;
    let color = parse_color(&opts.color)?;

    if !opts.font_size.is_finite() || opts.font_size < 4.0 || opts.font_size > 288.0 {
        return Err("font_size must be between 4 and 288 points".into());
    }
    if !opts.rotation.is_finite() || opts.rotation < -360.0 || opts.rotation > 360.0 {
        return Err("rotation must be between -360 and 360 degrees".into());
    }
    if !opts.opacity.is_finite() || opts.opacity <= 0.0 || opts.opacity > 1.0 {
        return Err("opacity must be greater than 0 and at most 1 (1 = fully opaque)".into());
    }

    let mut doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let pages = doc.get_pages();
    let total_pages = pages.len() as u32;
    if total_pages == 0 {
        return Err("PDF has no pages".into());
    }

    let selected = parse_pages(&opts.pages, total_pages)?;

    // Rotation matrix components (counter-clockwise, PDF user space).
    let theta = opts.rotation.to_radians();
    let (sin, cos) = theta.sin_cos();
    let size = opts.font_size;
    let tw = text_width(family, size, text);
    // Axis-aligned bounding box of the rotated label, used to inset edge
    // positions and to space tiles so stamps stay on-page and don't overlap.
    let bbox_w = tw * cos.abs() + size * sin.abs();
    let bbox_h = tw * sin.abs() + size * cos.abs();
    let label_bytes = pdf_literal(text);

    // One shared font object for every stamp.
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => family.base_font(),
        "Encoding" => "WinAnsiEncoding",
    });

    // A transparency graphics state, shared across pages, only when < 1.0.
    let extg_id = if opts.opacity < 1.0 {
        Some(doc.add_object(dictionary! {
            "Type" => "ExtGState",
            "ca" => opts.opacity,
            "CA" => opts.opacity,
        }))
    } else {
        None
    };

    for (num, id) in pages {
        if !selected.contains(&num) {
            continue;
        }

        let mb = effective_mediabox(&doc, id);
        let (x0, y0, x1, y1) = (mb[0], mb[1], mb[2], mb[3]);

        install_resources(&mut doc, id, font_id, extg_id);

        let mut ops = vec![Operation::new("q", vec![])];
        if extg_id.is_some() {
            ops.push(Operation::new("gs", vec![EXTG_RES_NAME.into()]));
        }
        ops.push(Operation::new("rg", vec![color[0].into(), color[1].into(), color[2].into()]));

        if opts.tile {
            // Repeat the rotated stamp on a grid covering the whole page.
            let step_x = (bbox_w + size * 1.5).max(size * 2.0);
            let step_y = (bbox_h + size * 1.5).max(size * 2.0);
            let mut drawn = 0usize;
            let mut py = y0 + step_y / 2.0;
            'rows: while py < y1 {
                let mut px = x0 + step_x / 2.0;
                while px < x1 {
                    // Each stamp needs its own q/Q so the cm transforms don't compound.
                    ops.push(Operation::new("q", vec![]));
                    draw_one(&mut ops, &label_bytes, size, tw, cos, sin, px, py);
                    ops.push(Operation::new("Q", vec![]));
                    drawn += 1;
                    if drawn >= MAX_TILES {
                        break 'rows;
                    }
                    px += step_x;
                }
                py += step_y;
            }
        } else {
            // Single stamp anchored at the chosen position.
            let px = match position.h {
                HAlign::Left => x0 + EDGE_INSET + bbox_w / 2.0,
                HAlign::Center => (x0 + x1) / 2.0,
                HAlign::Right => x1 - EDGE_INSET - bbox_w / 2.0,
            };
            let py = match position.v {
                VAlign::Bottom => y0 + EDGE_INSET + bbox_h / 2.0,
                VAlign::Middle => (y0 + y1) / 2.0,
                VAlign::Top => y1 - EDGE_INSET - bbox_h / 2.0,
            };
            ops.push(Operation::new("q", vec![]));
            draw_one(&mut ops, &label_bytes, size, tw, cos, sin, px, py);
            ops.push(Operation::new("Q", vec![]));
        }

        ops.push(Operation::new("Q", vec![]));
        stamp_page(&mut doc, id, ops);
    }

    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n_page_pdf(n: u32) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids: Vec<Object> = Vec::new();
        for _ in 0..n {
            // A tiny real content stream so the page isn't empty.
            let content_id = doc.add_object(Stream::new(dictionary! {}, b"BT ET".to_vec()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
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

    /// Concatenated, decompressed content of a page (1-based).
    fn page_text(pdf: &[u8], page_num: u32) -> String {
        let doc = Document::load_mem(pdf).unwrap();
        let id = doc.get_pages()[&page_num];
        String::from_utf8_lossy(&doc.get_page_content(id).unwrap()).into_owned()
    }

    fn page_count(pdf: &[u8]) -> usize {
        Document::load_mem(pdf).unwrap().get_pages().len()
    }

    fn tj_count(pdf: &[u8], page_num: u32) -> usize {
        page_text(pdf, page_num).matches(" Tj").count()
    }

    #[test]
    fn stamps_every_page_and_keeps_count() {
        let out = add_watermark(&n_page_pdf(3), &Options::default()).unwrap();
        assert_eq!(page_count(&out), 3);
        assert!(page_text(&out, 1).contains("(CONFIDENTIAL)"), "page 1 shows the mark");
        assert!(page_text(&out, 3).contains("(CONFIDENTIAL)"), "page 3 shows the mark");
        // Original content survives the overlay.
        assert!(page_text(&out, 1).contains("BT ET"));
        // Default rotation is diagonal → the cm matrix is not the identity.
        assert!(page_text(&out, 1).contains(" cm"), "rotation emits a cm transform");
    }

    #[test]
    fn custom_text_is_stamped() {
        let opts = Options {
            text: "DRAFT".to_string(),
            ..Default::default()
        };
        let out = add_watermark(&n_page_pdf(2), &opts).unwrap();
        assert!(page_text(&out, 1).contains("(DRAFT)"));
        assert!(!page_text(&out, 1).contains("(CONFIDENTIAL)"));
    }

    #[test]
    fn page_range_skips_the_cover() {
        let opts = Options {
            pages: "2-".to_string(),
            ..Default::default()
        };
        let out = add_watermark(&n_page_pdf(3), &opts).unwrap();
        assert!(!page_text(&out, 1).contains("(CONFIDENTIAL)"), "cover is untouched");
        assert!(page_text(&out, 2).contains("(CONFIDENTIAL)"));
        assert!(page_text(&out, 3).contains("(CONFIDENTIAL)"));
    }

    #[test]
    fn tile_mode_draws_many_stamps() {
        let single = add_watermark(&n_page_pdf(1), &Options::default()).unwrap();
        assert_eq!(tj_count(&single, 1), 1, "single layout stamps once");

        let tiled = Options {
            tile: true,
            ..Default::default()
        };
        let out = add_watermark(&n_page_pdf(1), &tiled).unwrap();
        assert!(tj_count(&out, 1) > 1, "tile layout repeats the stamp");
        assert!(tj_count(&out, 1) <= MAX_TILES);
    }

    #[test]
    fn opacity_installs_a_graphics_state() {
        let opaque = Options {
            opacity: 1.0,
            ..Default::default()
        };
        let out = add_watermark(&n_page_pdf(1), &opaque).unwrap();
        assert!(!page_text(&out, 1).contains("/GZWMgs gs"), "full opacity uses no ExtGState");

        // The default (0.3) is faint → an ExtGState alpha is installed.
        let faint = add_watermark(&n_page_pdf(1), &Options::default()).unwrap();
        assert!(page_text(&faint, 1).contains("/GZWMgs gs"), "faint stamp sets the alpha state");
    }

    #[test]
    fn zero_rotation_still_stamps() {
        let opts = Options {
            rotation: 0.0,
            position: "bottom-right".to_string(),
            ..Default::default()
        };
        let out = add_watermark(&n_page_pdf(1), &opts).unwrap();
        assert!(page_text(&out, 1).contains("(CONFIDENTIAL)"));
    }

    #[test]
    fn output_is_valid_pdf() {
        let out = add_watermark(&n_page_pdf(1), &Options::default()).unwrap();
        assert_eq!(&out[..5], b"%PDF-");
    }

    #[test]
    fn errors_on_bad_inputs() {
        // Not a PDF.
        assert!(add_watermark(b"not a pdf", &Options::default()).is_err());
        // Empty watermark text.
        let empty = Options {
            text: "   ".to_string(),
            ..Default::default()
        };
        assert!(add_watermark(&n_page_pdf(1), &empty).is_err());
        // Bad page range.
        let bad_pages = Options {
            pages: "9".to_string(),
            ..Default::default()
        };
        assert!(add_watermark(&n_page_pdf(2), &bad_pages).is_err());
        // Bad colour.
        let bad_color = Options {
            color: "not-a-color".to_string(),
            ..Default::default()
        };
        assert!(add_watermark(&n_page_pdf(1), &bad_color).is_err());
        // Bad font size.
        let bad_size = Options {
            font_size: 1.0,
            ..Default::default()
        };
        assert!(add_watermark(&n_page_pdf(1), &bad_size).is_err());
        // Bad rotation.
        let bad_rot = Options {
            rotation: 720.0,
            ..Default::default()
        };
        assert!(add_watermark(&n_page_pdf(1), &bad_rot).is_err());
        // Bad opacity.
        let bad_op = Options {
            opacity: 0.0,
            ..Default::default()
        };
        assert!(add_watermark(&n_page_pdf(1), &bad_op).is_err());
    }

    #[test]
    fn helpers_are_correct() {
        assert_eq!(parse_color("#ff0000").unwrap(), [1.0, 0.0, 0.0]);
        assert_eq!(parse_color("#f00").unwrap(), [1.0, 0.0, 0.0]);
        assert!(parse_color("nope").is_err());
        assert_eq!(parse_pages("2-", 3).unwrap(), BTreeSet::from([2, 3]));
        assert_eq!(parse_pages("all", 2).unwrap(), BTreeSet::from([1, 2]));
        assert!(Position::parse("center").is_ok());
        assert!(Position::parse("bottom-right").is_ok());
        assert!(Position::parse("nowhere").is_err());
    }
}
