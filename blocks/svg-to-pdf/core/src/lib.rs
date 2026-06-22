//! gizza-ai/svg-to-pdf core — convert an SVG drawing into a single-page PDF.
//! Pure-Rust (resvg + tiny-skia rasterizer, lopdf serializer), so it runs on
//! every backend including the chat Service Worker.
//!
//! Pipeline: parse the SVG → render it to a high-resolution RGB raster (DPI
//! configurable) → embed that raster as a Flate-compressed image XObject on one
//! PDF page sized in points to the SVG's intrinsic dimensions (so the document
//! prints at the SVG's real-world size while the raster carries the detail).
//!
//! Why rasterize instead of emitting native PDF vectors? A faithful SVG→PDF
//! vector translator (gradients, clips, filters, text/fonts, blend modes) is a
//! large undertaking; rasterizing with resvg renders the SVG exactly as a
//! browser would and embeds it losslessly at a chosen DPI. Bump `dpi` for
//! crisper print output.
//!
//! Text note: glyphs need a font. resvg renders text only with a loaded font
//! database; we ship none (no system fonts in wasm), so shape/path SVGs convert
//! perfectly while `<text>` without an embedded/converted font is skipped.
//! Convert text to paths in your SVG editor for pixel-perfect labels.

use std::io::Write;

use flate2::{write::ZlibEncoder, Compression};
use lopdf::{dictionary, Document, Object, Stream};

/// 1 CSS px = 1/96 inch; 1 PDF point = 1/72 inch → px → pt factor.
const PX_TO_PT: f32 = 72.0 / 96.0;
/// Cap rasterized pixel dimensions per side (memory guard).
const MAX_DIM: u32 = 10_000;
/// Cap the page size in points (PDF practical limit is 14400 pt = 200 in).
const MAX_PT: f32 = 14_400.0;

/// Conversion options.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Rasterization resolution in dots per inch. Higher = crisper, larger file.
    pub dpi: f32,
    /// Optional background colour painted behind the SVG (PDF pages have no
    /// transparency, so transparent SVG areas show as this colour; default white).
    pub background: BgColor,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            dpi: 150.0,
            background: BgColor::White,
        }
    }
}

/// Page background colour (PDF has no page-level alpha).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BgColor {
    White,
    Rgb(u8, u8, u8),
}

impl BgColor {
    /// Parse `white` (default), `#rgb`, `#rrggbb` (leading `#` optional). Named
    /// CSS colours other than `white` are not supported — use a hex value.
    pub fn parse(s: &str) -> Result<Self, String> {
        let t = s.trim();
        if t.is_empty() || t.eq_ignore_ascii_case("white") {
            return Ok(BgColor::White);
        }
        let hex = t.strip_prefix('#').unwrap_or(t);
        let parse2 = |h: &str| u8::from_str_radix(h, 16).map_err(|_| format!("invalid hex colour '{t}'"));
        match hex.len() {
            3 => {
                let r = parse2(&hex[0..1].repeat(2))?;
                let g = parse2(&hex[1..2].repeat(2))?;
                let b = parse2(&hex[2..3].repeat(2))?;
                Ok(BgColor::Rgb(r, g, b))
            }
            6 => {
                let r = parse2(&hex[0..2])?;
                let g = parse2(&hex[2..4])?;
                let b = parse2(&hex[4..6])?;
                Ok(BgColor::Rgb(r, g, b))
            }
            _ => Err(format!(
                "invalid colour '{t}' (use 'white' or a hex value like #ffffff)"
            )),
        }
    }

    fn rgb(self) -> (u8, u8, u8) {
        match self {
            BgColor::White => (255, 255, 255),
            BgColor::Rgb(r, g, b) => (r, g, b),
        }
    }
}

/// Render `svg` (UTF-8 markup) into a single-page PDF per `opts`.
pub fn svg_to_pdf(svg: &str, opts: &Options) -> Result<Vec<u8>, String> {
    if svg.trim().is_empty() {
        return Err("input is empty".into());
    }
    if !svg.to_ascii_lowercase().contains("<svg") {
        return Err("input does not look like SVG (no <svg> element found)".into());
    }
    if !(opts.dpi.is_finite() && opts.dpi >= 24.0 && opts.dpi <= 1200.0) {
        return Err("dpi must be between 24 and 1200".into());
    }

    // Construct usvg options WITHOUT the fs-backed default image-href resolver
    // (it pulls WASI filesystem imports the wafer runtime doesn't provide). Every
    // field is set explicitly; embedded `data:` raster images still resolve.
    let usvg_opts = usvg::Options {
        resources_dir: None,
        dpi: 96.0,
        font_family: "Times New Roman".to_string(),
        font_size: 12.0,
        languages: vec!["en".to_string()],
        shape_rendering: usvg::ShapeRendering::default(),
        text_rendering: usvg::TextRendering::default(),
        image_rendering: usvg::ImageRendering::default(),
        default_size: usvg::Size::from_wh(100.0, 100.0).unwrap(),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(
                |mime: &str, data: std::sync::Arc<Vec<u8>>, _opts: &usvg::Options| match mime
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "image/jpg" | "image/jpeg" => Some(usvg::ImageKind::JPEG(data)),
                    "image/png" => Some(usvg::ImageKind::PNG(data)),
                    "image/gif" => Some(usvg::ImageKind::GIF(data)),
                    "image/webp" => Some(usvg::ImageKind::WEBP(data)),
                    _ => None,
                },
            ),
            resolve_string: Box::new(|_href: &str, _opts: &usvg::Options| None),
        },
        style_sheet: None,
    };

    let tree = usvg::Tree::from_str(svg, &usvg_opts).map_err(|e| format!("could not parse SVG: {e}"))?;
    let size = tree.size();
    let (sw, sh) = (size.width(), size.height());
    if sw <= 0.0 || sh <= 0.0 {
        return Err("the SVG has no positive intrinsic size (set width/height or a viewBox)".into());
    }

    // PDF page size in points from the SVG's intrinsic (CSS px) size.
    let page_w_pt = sw * PX_TO_PT;
    let page_h_pt = sh * PX_TO_PT;
    if page_w_pt > MAX_PT || page_h_pt > MAX_PT {
        return Err(format!(
            "page size {page_w_pt:.0}x{page_h_pt:.0}pt exceeds the {MAX_PT:.0}pt PDF limit"
        ));
    }

    // Raster pixel size at the chosen DPI (px = intrinsic_px * dpi/96).
    let scale = opts.dpi / 96.0;
    let raster_w = (sw * scale).round().max(1.0) as u32;
    let raster_h = (sh * scale).round().max(1.0) as u32;
    if raster_w > MAX_DIM || raster_h > MAX_DIM {
        return Err(format!(
            "rasterized size {raster_w}x{raster_h}px exceeds the {MAX_DIM}px limit per side — lower the dpi"
        ));
    }

    let mut pixmap = tiny_skia::Pixmap::new(raster_w, raster_h)
        .ok_or_else(|| format!("could not allocate a {raster_w}x{raster_h} canvas"))?;
    // Paint the page background first (PDF pages are opaque).
    let (br, bg, bb) = opts.background.rgb();
    pixmap.fill(tiny_skia::Color::from_rgba8(br, bg, bb, 255));

    let transform = tiny_skia::Transform::from_scale(raster_w as f32 / sw, raster_h as f32 / sh);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Flatten premultiplied RGBA → RGB (already composited over the opaque bg).
    let mut rgb = Vec::with_capacity((raster_w * raster_h * 3) as usize);
    for px in pixmap.data().chunks_exact(4) {
        rgb.push(px[0]);
        rgb.push(px[1]);
        rgb.push(px[2]);
    }

    build_pdf(&rgb, raster_w, raster_h, page_w_pt, page_h_pt)
}

/// Embed an RGB raster on one PDF page of the given point size.
fn build_pdf(rgb: &[u8], w: u32, h: u32, page_w_pt: f32, page_h_pt: f32) -> Result<Vec<u8>, String> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(rgb)
        .and_then(|_| enc.flush())
        .map_err(|e| format!("compress raster: {e}"))?;
    let compressed = enc.finish().map_err(|e| format!("compress raster: {e}"))?;

    let mut img_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => w as i64,
            "Height" => h as i64,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
            "Filter" => "FlateDecode",
        },
        compressed,
    );
    img_stream.allows_compression = false;
    let img_id = doc.add_object(img_stream);

    // Paint the image to fill the whole page (CTM maps the unit square → page).
    let content = format!("q {page_w_pt:.2} 0 0 {page_h_pt:.2} 0 0 cm /Img Do Q");
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));

    let resources_id = doc.add_object(dictionary! {
        "XObject" => dictionary! { "Img" => img_id },
    });
    // PDF MediaBox uses numbers; round to 2 dp via i64 of hundredths is messy, so
    // store as real numbers (lopdf accepts f32→Object via .into()).
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(round2(page_w_pt)),
            Object::Real(round2(page_h_pt)),
        ],
        "Contents" => content_id,
        "Resources" => resources_id,
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1_i64,
        }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);

    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(out)
}

fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50" viewBox="0 0 100 50"><rect width="100" height="50" fill="#3366cc"/></svg>"##;

    fn page_box(pdf: &[u8]) -> (f32, f32) {
        let doc = Document::load_mem(pdf).expect("output should parse");
        let (_, page_id) = doc.get_pages().into_iter().next().unwrap();
        let mb = doc
            .get_object(page_id)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap();
        (mb[2].as_float().unwrap(), mb[3].as_float().unwrap())
    }

    #[test]
    fn produces_a_single_page_pdf() {
        let pdf = svg_to_pdf(SQUARE, &Options::default()).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-", "PDF magic");
        let doc = Document::load_mem(&pdf).unwrap();
        assert_eq!(doc.get_pages().len(), 1);
    }

    #[test]
    fn page_size_matches_svg_in_points() {
        let pdf = svg_to_pdf(SQUARE, &Options::default()).unwrap();
        let (w, h) = page_box(&pdf);
        // 100px * 72/96 = 75pt ; 50px -> 37.5pt
        assert!((w - 75.0).abs() < 0.1, "width {w}");
        assert!((h - 37.5).abs() < 0.1, "height {h}");
    }

    #[test]
    fn higher_dpi_still_one_page_same_box() {
        // DPI only changes raster detail, not the page's point dimensions.
        let lo = svg_to_pdf(SQUARE, &Options { dpi: 72.0, ..Options::default() }).unwrap();
        let hi = svg_to_pdf(SQUARE, &Options { dpi: 300.0, ..Options::default() }).unwrap();
        assert_eq!(page_box(&lo), page_box(&hi));
        // Higher DPI embeds more pixels → bigger file.
        assert!(hi.len() > lo.len(), "hi {} should exceed lo {}", hi.len(), lo.len());
    }

    #[test]
    fn errors_on_empty() {
        assert!(svg_to_pdf("   ", &Options::default()).is_err());
    }

    #[test]
    fn errors_on_non_svg() {
        assert!(svg_to_pdf("<html>nope</html>", &Options::default()).is_err());
    }

    #[test]
    fn errors_on_bad_dpi() {
        assert!(svg_to_pdf(SQUARE, &Options { dpi: 5.0, ..Options::default() }).is_err());
        assert!(svg_to_pdf(SQUARE, &Options { dpi: 5000.0, ..Options::default() }).is_err());
    }

    #[test]
    fn parses_bg_color() {
        assert_eq!(BgColor::parse("white").unwrap(), BgColor::White);
        assert_eq!(BgColor::parse("").unwrap(), BgColor::White);
        assert_eq!(BgColor::parse("#fff").unwrap(), BgColor::Rgb(255, 255, 255));
        assert_eq!(BgColor::parse("#ff0000").unwrap(), BgColor::Rgb(255, 0, 0));
        assert!(BgColor::parse("#xyz").is_err());
        assert!(BgColor::parse("chartreuse").is_err());
    }

    #[test]
    fn custom_background_parses_and_renders() {
        let pdf = svg_to_pdf(
            SQUARE,
            &Options { background: BgColor::Rgb(0, 0, 0), ..Options::default() },
        )
        .unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
    }
}
