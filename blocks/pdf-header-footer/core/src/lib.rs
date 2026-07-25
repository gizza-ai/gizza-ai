//! gizza-ai/pdf-header-footer core — stamp header/footer text onto an existing PDF.
//! Pure-Rust (`lopdf`), no font embedding: uses the built-in base-14 Type1 fonts
//! (Helvetica / Times-Roman / Courier) so the output stays small and the tool
//! runs on every backend. Each selected page gets an overlaid header (top margin)
//! and/or footer (bottom margin) whose graphics state is isolated from the page's
//! own content (a leading `q` / trailing `Q` restore), so existing content is
//! never disturbed.
//!
//! `{page}` (the 1-based page index) and `{pages}` (the document's page count)
//! are substituted per page into the header/footer templates. Glyphs the base-14
//! WinAnsi encoding can't represent degrade to `?`.

use std::collections::{BTreeSet, HashSet};

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Dictionary, Document, Object, Stream};

const FONT_RES_NAME: &str = "GZHF";
const EXTG_RES_NAME: &str = "GZHFgs";

/// Horizontal alignment of a header/footer line within the page band.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Align {
    Left,
    Center,
    Right,
}

impl Align {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "left" => Align::Left,
            "center" | "centre" | "" => Align::Center,
            "right" => Align::Right,
            other => {
                return Err(format!(
                    "unknown alignment '{other}': use left, center, or right"
                ))
            }
        })
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

/// All stamp options (raw string forms as they arrive from the block/CLI).
pub struct Options {
    pub header: String,
    pub footer: String,
    pub header_align: String,
    pub footer_align: String,
    pub font: String,
    pub font_size: f64,
    pub color: String,
    pub margin: f64,
    pub opacity: f64,
    pub pages: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            header: String::new(),
            footer: String::new(),
            header_align: "center".to_string(),
            footer_align: "center".to_string(),
            font: "helvetica".to_string(),
            font_size: 10.0,
            color: "#444444".to_string(),
            margin: 36.0,
            opacity: 1.0,
            pages: "all".to_string(),
        }
    }
}

/// Parse a 1-based page spec ("all"/""/"1,3-5,8"/"2-") into the set of pages to stamp.
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

/// Substitute `{page}` (current page number) and `{pages}` (page count) into `template`.
fn render_label(template: &str, page: u32, pages: u32) -> String {
    template
        .replace("{page}", &page.to_string())
        .replace("{pages}", &pages.to_string())
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
                "invalid color '{s}': use a hex code like #444444, #ffffff, or #f00"
            ))
        }
    };
    match bytes {
        (Ok(r), Ok(g), Ok(b)) => Ok([r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0]),
        _ => Err(format!(
            "invalid color '{s}': use a hex code like #444444, #ffffff, or #f00"
        )),
    }
}

/// Advance width (1000-em units) of `ch` in the given base-14 font. Covers the
/// glyphs a header/footer label uses; anything else falls back to a sane average.
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

/// Width of a whole label in points.
fn text_width(font: FontFamily, size: f64, s: &str) -> f64 {
    let em: f64 = s.chars().map(|c| glyph_width(font, c) as f64).sum();
    em * size / 1000.0
}

/// Escape a label into WinAnsi/Latin-1 bytes for a PDF literal string; code
/// points beyond 0xFF (which the base-14 encoding can't represent) become '?'.
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

/// Draw a single aligned line at baseline `y`, returning the text-drawing ops.
fn line_ops(
    family: FontFamily,
    font_size: f64,
    color: [f64; 3],
    align: Align,
    mb: [f64; 4],
    margin: f64,
    y: f64,
    label: &str,
    use_gs: bool,
) -> Vec<Operation> {
    let (x0, _y0, x1, _y1) = (mb[0], mb[1], mb[2], mb[3]);
    let tw = text_width(family, font_size, label);
    let x = match align {
        Align::Left => x0 + margin,
        Align::Center => x0 + (x1 - x0 - tw) / 2.0,
        Align::Right => x1 - margin - tw,
    };
    let mut ops = vec![Operation::new("q", vec![])];
    if use_gs {
        ops.push(Operation::new("gs", vec![EXTG_RES_NAME.into()]));
    }
    ops.extend([
        Operation::new("rg", vec![color[0].into(), color[1].into(), color[2].into()]),
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec![FONT_RES_NAME.into(), font_size.into()]),
        Operation::new("Td", vec![x.into(), y.into()]),
        Operation::new(
            "Tj",
            vec![Object::String(pdf_literal(label), lopdf::StringFormat::Literal)],
        ),
        Operation::new("ET", vec![]),
        Operation::new("Q", vec![]),
    ]);
    ops
}

/// Stamp header/footer text onto `pdf` per `opts`, returning the new PDF bytes.
pub fn add_header_footer(pdf: &[u8], opts: &Options) -> Result<Vec<u8>, String> {
    // At least one band must carry text.
    if opts.header.trim().is_empty() && opts.footer.trim().is_empty() {
        return Err("provide a header, a footer, or both (both are empty)".into());
    }

    // Validate + parse the enum-ish options up front.
    let family = FontFamily::parse(&opts.font)?;
    let header_align = Align::parse(&opts.header_align)?;
    let footer_align = Align::parse(&opts.footer_align)?;
    let color = parse_color(&opts.color)?;

    if !opts.font_size.is_finite() || opts.font_size < 4.0 || opts.font_size > 144.0 {
        return Err("font_size must be between 4 and 144 points".into());
    }
    if !opts.margin.is_finite() || opts.margin < 0.0 || opts.margin > 400.0 {
        return Err("margin must be between 0 and 400 points".into());
    }
    if !opts.opacity.is_finite() || opts.opacity < 0.05 || opts.opacity > 1.0 {
        return Err("opacity must be between 0.05 and 1 (1 = fully opaque)".into());
    }

    let mut doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let pages = doc.get_pages();
    let total_pages = pages.len() as u32;
    if total_pages == 0 {
        return Err("PDF has no pages".into());
    }

    let selected = parse_pages(&opts.pages, total_pages)?;

    let has_header = !opts.header.is_empty();
    let has_footer = !opts.footer.is_empty();

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
    let use_gs = extg_id.is_some();

    for (num, id) in pages {
        if !selected.contains(&num) {
            continue;
        }

        let mb = effective_mediabox(&doc, id);
        let (y0, y1) = (mb[1], mb[3]);

        let mut ops: Vec<Operation> = Vec::new();

        if has_header {
            let label = render_label(&opts.header, num, total_pages);
            if !label.is_empty() {
                // Header baseline sits one line below the top margin edge.
                let y = y1 - opts.margin - opts.font_size;
                ops.extend(line_ops(
                    family, opts.font_size, color, header_align, mb, opts.margin, y, &label, use_gs,
                ));
            }
        }
        if has_footer {
            let label = render_label(&opts.footer, num, total_pages);
            if !label.is_empty() {
                // Footer baseline sits at the bottom margin.
                let y = y0 + opts.margin;
                ops.extend(line_ops(
                    family, opts.font_size, color, footer_align, mb, opts.margin, y, &label, use_gs,
                ));
            }
        }

        if ops.is_empty() {
            continue;
        }

        install_resources(&mut doc, id, font_id, extg_id);
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

    fn header(h: &str) -> Options {
        Options {
            header: h.to_string(),
            ..Default::default()
        }
    }

    // --- page range parsing -------------------------------------------------

    #[test]
    fn parse_pages_all_and_empty() {
        assert_eq!(parse_pages("all", 3).unwrap(), BTreeSet::from([1, 2, 3]));
        assert_eq!(parse_pages("", 2).unwrap(), BTreeSet::from([1, 2]));
        assert_eq!(parse_pages("ALL", 2).unwrap(), BTreeSet::from([1, 2]));
    }

    #[test]
    fn parse_pages_lists_and_ranges() {
        assert_eq!(
            parse_pages("1,3-5,2-", 6).unwrap(),
            BTreeSet::from([1, 2, 3, 4, 5, 6])
        );
        assert_eq!(parse_pages("3-", 5).unwrap(), BTreeSet::from([3, 4, 5]));
        assert_eq!(parse_pages("2,4", 5).unwrap(), BTreeSet::from([2, 4]));
        // Reversed ranges are normalised.
        assert_eq!(parse_pages("5-3", 6).unwrap(), BTreeSet::from([3, 4, 5]));
    }

    #[test]
    fn parse_pages_rejects_bad_specs() {
        assert!(parse_pages("0", 3).is_err()); // 1-based
        assert!(parse_pages("9", 3).is_err()); // out of range
        assert!(parse_pages("2-9", 3).is_err()); // range out of range
        assert!(parse_pages("x", 3).is_err()); // non-numeric
    }

    // --- token substitution / escaping / colour -----------------------------

    #[test]
    fn render_label_substitutes_tokens() {
        assert_eq!(render_label("Page {page} of {pages}", 2, 7), "Page 2 of 7");
        assert_eq!(render_label("no tokens", 1, 3), "no tokens");
        assert_eq!(render_label("{pages}", 1, 42), "42");
    }

    #[test]
    fn pdf_literal_escapes_and_degrades() {
        assert_eq!(pdf_literal("a(b)\\c"), b"a\\(b\\)\\\\c".to_vec());
        // A code point beyond 0xFF degrades to '?'.
        assert_eq!(pdf_literal("caf\u{2764}"), b"caf?".to_vec());
        // Latin-1 in range survives.
        assert_eq!(pdf_literal("\u{00e9}"), vec![0xE9]);
    }

    #[test]
    fn parse_color_forms() {
        assert_eq!(parse_color("#ff0000").unwrap(), [1.0, 0.0, 0.0]);
        assert_eq!(parse_color("#f00").unwrap(), [1.0, 0.0, 0.0]);
        assert_eq!(parse_color("444444").unwrap(), [0x44 as f64 / 255.0; 3]);
        assert!(parse_color("not-a-color").is_err());
        assert!(parse_color("#12").is_err());
    }

    // --- happy overlay ------------------------------------------------------

    #[test]
    fn stamps_header_on_every_page_and_keeps_count() {
        let out = add_header_footer(&n_page_pdf(3), &header("Confidential")).unwrap();
        assert_eq!(page_count(&out), 3);
        assert!(page_text(&out, 1).contains("(Confidential)"));
        assert!(page_text(&out, 3).contains("(Confidential)"));
        // Original content survives the overlay.
        assert!(page_text(&out, 1).contains("BT ET"));
        // Output opens as a valid PDF.
        assert_eq!(&out[..5], b"%PDF-");
    }

    #[test]
    fn stamps_both_bands_with_tokens() {
        let opts = Options {
            header: "Report".to_string(),
            footer: "Page {page} / {pages}".to_string(),
            ..Default::default()
        };
        let out = add_header_footer(&n_page_pdf(4), &opts).unwrap();
        assert_eq!(page_count(&out), 4);
        let p2 = page_text(&out, 2);
        assert!(p2.contains("(Report)"), "header present");
        assert!(p2.contains("(Page 2 / 4)"), "footer tokens substituted");
        assert!(page_text(&out, 4).contains("(Page 4 / 4)"));
    }

    #[test]
    fn footer_only_leaves_no_header() {
        let opts = Options {
            footer: "footer only".to_string(),
            ..Default::default()
        };
        let out = add_header_footer(&n_page_pdf(1), &opts).unwrap();
        assert!(page_text(&out, 1).contains("(footer only)"));
    }

    #[test]
    fn page_range_skips_the_cover() {
        let opts = Options {
            header: "H".to_string(),
            pages: "2-".to_string(),
            ..Default::default()
        };
        let out = add_header_footer(&n_page_pdf(3), &opts).unwrap();
        assert!(!page_text(&out, 1).contains("(H)"), "cover untouched");
        assert!(page_text(&out, 2).contains("(H)"));
        assert!(page_text(&out, 3).contains("(H)"));
    }

    #[test]
    fn opacity_installs_a_graphics_state() {
        let opaque = add_header_footer(&n_page_pdf(1), &header("H")).unwrap();
        assert!(
            !page_text(&opaque, 1).contains("/GZHFgs gs"),
            "full opacity uses no ExtGState"
        );
        let faint = Options {
            header: "H".to_string(),
            opacity: 0.4,
            ..Default::default()
        };
        let out = add_header_footer(&n_page_pdf(1), &faint).unwrap();
        assert!(
            page_text(&out, 1).contains("/GZHFgs gs"),
            "faint stamp sets the alpha state"
        );
    }

    // --- errors -------------------------------------------------------------

    #[test]
    fn errors_on_bad_inputs() {
        // Both bands empty.
        assert!(add_header_footer(&n_page_pdf(1), &Options::default()).is_err());
        // Not a PDF.
        assert!(add_header_footer(b"not a pdf", &header("H")).is_err());
        // Bad page range.
        let bad_pages = Options {
            pages: "9".to_string(),
            ..header("H")
        };
        assert!(add_header_footer(&n_page_pdf(2), &bad_pages).is_err());
        // Bad colour.
        let bad_color = Options {
            color: "not-a-color".to_string(),
            ..header("H")
        };
        assert!(add_header_footer(&n_page_pdf(1), &bad_color).is_err());
        // Bad font size.
        let bad_size = Options {
            font_size: 1.0,
            ..header("H")
        };
        assert!(add_header_footer(&n_page_pdf(1), &bad_size).is_err());
        // Bad alignment.
        let bad_align = Options {
            header_align: "middle".to_string(),
            ..header("H")
        };
        assert!(add_header_footer(&n_page_pdf(1), &bad_align).is_err());
        // Bad opacity.
        let bad_opacity = Options {
            opacity: 2.0,
            ..header("H")
        };
        assert!(add_header_footer(&n_page_pdf(1), &bad_opacity).is_err());
    }
}
