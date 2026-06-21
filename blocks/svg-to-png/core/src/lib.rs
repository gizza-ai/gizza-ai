//! gizza-ai/svg-to-png core — rasterize an SVG document into a PNG or JPEG at a
//! chosen resolution, dependency-light and pure-Rust (resvg + tiny-skia), so it
//! runs on every backend including the chat Service Worker.
//!
//! Sizing model (matches every other raster tool the LLM has seen):
//!   * `width`/`height` (px) override the rendered output size. If only one is
//!     given the other is derived from the SVG's intrinsic aspect ratio.
//!   * If neither is given, a `scale` multiplier is applied to the SVG's
//!     intrinsic size (`scale = 1.0` → native size).
//!   * The intrinsic size comes from the SVG's `width`/`height` or `viewBox`.
//!
//! Text note: glyphs need a font. resvg renders text only with a loaded font
//! database; we ship an empty one (no system fonts in wasm), so shape/path SVGs
//! rasterize perfectly while `<text>` without an embedded/converted font is
//! skipped. Convert text to paths in your SVG editor for pixel-perfect labels.

/// Output raster format.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    Png,
    Jpeg,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "png" => Ok(OutputFormat::Png),
            "jpg" | "jpeg" => Ok(OutputFormat::Jpeg),
            other => Err(format!(
                "unknown format '{other}' (expected 'png' or 'jpeg')"
            )),
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            OutputFormat::Png => "image/png",
            OutputFormat::Jpeg => "image/jpeg",
        }
    }
}

/// Rasterization options.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub format: OutputFormat,
    /// Explicit output width in px (0 = unset → derive from height/scale).
    pub width: u32,
    /// Explicit output height in px (0 = unset → derive from width/scale).
    pub height: u32,
    /// Scale multiplier applied to the intrinsic size when no explicit
    /// width/height is given (must be > 0).
    pub scale: f32,
    /// JPEG quality 1..=100 (ignored for PNG).
    pub quality: u8,
    /// Background colour as `#rgb`/`#rrggbb`/`#rrggbbaa` or `transparent`.
    /// PNG keeps alpha; JPEG (no alpha) composites onto white when transparent.
    pub background: BgColor,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: OutputFormat::Png,
            width: 0,
            height: 0,
            scale: 1.0,
            quality: 90,
            background: BgColor::Transparent,
        }
    }
}

/// A parsed background colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BgColor {
    Transparent,
    Rgba(u8, u8, u8, u8),
}

impl BgColor {
    /// Parse `transparent`, `#rgb`, `#rrggbb`, `#rrggbbaa` (a leading `#` is
    /// optional). Named CSS colours are not supported — use a hex value.
    pub fn parse(s: &str) -> Result<Self, String> {
        let t = s.trim();
        if t.is_empty() || t.eq_ignore_ascii_case("transparent") || t.eq_ignore_ascii_case("none") {
            return Ok(BgColor::Transparent);
        }
        let hex = t.strip_prefix('#').unwrap_or(t);
        let parse2 = |h: &str| u8::from_str_radix(h, 16).map_err(|_| format!("invalid hex colour '{t}'"));
        match hex.len() {
            3 => {
                let r = parse2(&hex[0..1].repeat(2))?;
                let g = parse2(&hex[1..2].repeat(2))?;
                let b = parse2(&hex[2..3].repeat(2))?;
                Ok(BgColor::Rgba(r, g, b, 255))
            }
            6 => {
                let r = parse2(&hex[0..2])?;
                let g = parse2(&hex[2..4])?;
                let b = parse2(&hex[4..6])?;
                Ok(BgColor::Rgba(r, g, b, 255))
            }
            8 => {
                let r = parse2(&hex[0..2])?;
                let g = parse2(&hex[2..4])?;
                let b = parse2(&hex[4..6])?;
                let a = parse2(&hex[6..8])?;
                Ok(BgColor::Rgba(r, g, b, a))
            }
            _ => Err(format!(
                "invalid colour '{t}' (use 'transparent' or a hex value like #ffffff)"
            )),
        }
    }
}

const MAX_DIM: u32 = 8000;

/// Compute the final integer output dimensions from the intrinsic SVG size and
/// the options. Returns (out_w, out_h) in px, both >= 1.
fn target_size(intrinsic_w: f32, intrinsic_h: f32, opts: &Options) -> Result<(u32, u32), String> {
    if intrinsic_w <= 0.0 || intrinsic_h <= 0.0 {
        return Err("the SVG has no positive intrinsic size (set width/height or a viewBox, or pass an explicit width/height)".into());
    }
    let aspect = intrinsic_w / intrinsic_h;
    let (w, h) = match (opts.width, opts.height) {
        (0, 0) => {
            if opts.scale <= 0.0 {
                return Err("scale must be > 0".into());
            }
            (
                (intrinsic_w * opts.scale).round() as u32,
                (intrinsic_h * opts.scale).round() as u32,
            )
        }
        (w, 0) => (w, (w as f32 / aspect).round() as u32),
        (0, h) => ((h as f32 * aspect).round() as u32, h),
        (w, h) => (w, h),
    };
    let w = w.max(1);
    let h = h.max(1);
    if w > MAX_DIM || h > MAX_DIM {
        return Err(format!(
            "output size {w}x{h} exceeds the {MAX_DIM}px limit per side"
        ));
    }
    Ok((w, h))
}

/// Rasterize `svg` (UTF-8 markup) into PNG/JPEG bytes per `opts`.
pub fn rasterize(svg: &str, opts: &Options) -> Result<Vec<u8>, String> {
    if svg.trim().is_empty() {
        return Err("input is empty".into());
    }
    if !svg.to_ascii_lowercase().contains("<svg") {
        return Err("input does not look like SVG (no <svg> element found)".into());
    }
    if !(1..=100).contains(&opts.quality) {
        return Err("quality must be between 1 and 100".into());
    }

    // usvg's *default* image-href resolver reads referenced files from disk via
    // `std::fs::read` (`ImageHrefResolver::default_string_resolver`). Even if we
    // overwrote that field after `Options::default()`, the default closure would
    // already be instantiated and stay linked — pulling WASI filesystem imports
    // (`path_open`/`fd_close`) the wafer runtime doesn't provide, so the block
    // would fail to instantiate. So we construct the resolver WITHOUT ever calling
    // the fs-backed default: a no-op `resolve_string` (external file refs are not
    // supported — we never have a base dir anyway) plus usvg's pure
    // `default_data_resolver`, which keeps embedded `data:` image hrefs working.
    // NB: we must NOT call `usvg::Options::default()` at all — its struct literal
    // constructs `ImageHrefResolver::default()`, instantiating the fs-backed
    // `default_string_resolver` closure, which stays linked even if the value is
    // immediately overwritten via `..default()`. So every field is set explicitly.
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
            // Custom data resolver for embedded `data:` raster images. We do NOT
            // use usvg's `default_data_resolver`: its SVG branch calls
            // `load_sub_svg` → `Options::default()` → the fs-backed string
            // resolver, re-introducing the WASI filesystem imports. Handling only
            // raster mimes here keeps the whole tree fs-free (nested SVG-in-SVG via
            // a data: href is unsupported — extremely rare; flatten it first).
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

    let tree = usvg::Tree::from_str(svg, &usvg_opts)
        .map_err(|e| format!("could not parse SVG: {e}"))?;
    let size = tree.size();
    let (out_w, out_h) = target_size(size.width(), size.height(), opts)?;

    let mut pixmap = tiny_skia::Pixmap::new(out_w, out_h)
        .ok_or_else(|| format!("could not allocate a {out_w}x{out_h} canvas"))?;

    // Background fill (before the SVG is drawn over it).
    if let BgColor::Rgba(r, g, b, a) = opts.background {
        let color = tiny_skia::Color::from_rgba8(r, g, b, a);
        pixmap.fill(color);
    }

    // Scale the SVG's intrinsic size up/down to fill the canvas.
    let sx = out_w as f32 / size.width();
    let sy = out_h as f32 / size.height();
    let transform = tiny_skia::Transform::from_scale(sx, sy);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    match opts.format {
        OutputFormat::Png => pixmap
            .encode_png()
            .map_err(|e| format!("PNG encode failed: {e}")),
        OutputFormat::Jpeg => {
            // JPEG has no alpha — composite onto the chosen background (white if
            // the background was transparent) so semi/transparent pixels don't
            // turn black.
            let (bgr, bgg, bgb) = match opts.background {
                BgColor::Rgba(r, g, b, _) => (r, g, b),
                BgColor::Transparent => (255, 255, 255),
            };
            let data = pixmap.data();
            let mut rgb = Vec::with_capacity((out_w * out_h * 3) as usize);
            for px in data.chunks_exact(4) {
                // tiny-skia stores premultiplied RGBA; un-premultiply for compositing.
                let (r, g, b, a) = (px[0] as u32, px[1] as u32, px[2] as u32, px[3] as u32);
                let comp = |c: u32, bg: u8| -> u8 {
                    // c is premultiplied by a/255; final = c + bg*(1-a/255)
                    let fg = c; // already premultiplied
                    let out = fg as u32 + (bg as u32 * (255 - a)) / 255;
                    out.min(255) as u8
                };
                rgb.push(comp(r, bgr));
                rgb.push(comp(g, bgg));
                rgb.push(comp(b, bgb));
            }
            let mut buf: Vec<u8> = Vec::new();
            let enc = jpeg_encoder::Encoder::new(&mut buf, opts.quality);
            enc.encode(
                &rgb,
                out_w as u16,
                out_h as u16,
                jpeg_encoder::ColorType::Rgb,
            )
            .map_err(|e| format!("JPEG encode failed: {e}"))?;
            Ok(buf)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50" viewBox="0 0 100 50"><rect width="100" height="50" fill="#3366cc"/></svg>"##;

    fn png_dims(bytes: &[u8]) -> (u32, u32) {
        // PNG IHDR: width at byte 16, height at byte 20 (big-endian).
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        (w, h)
    }

    #[test]
    fn rasterizes_png_native_size() {
        let out = rasterize(SQUARE, &Options::default()).unwrap();
        assert_eq!(&out[1..4], b"PNG", "PNG magic: {:?}", &out[..8]);
        assert_eq!(png_dims(&out), (100, 50));
    }

    #[test]
    fn scale_multiplies_size() {
        let opts = Options {
            scale: 2.0,
            ..Options::default()
        };
        let out = rasterize(SQUARE, &opts).unwrap();
        assert_eq!(png_dims(&out), (200, 100));
    }

    #[test]
    fn explicit_width_keeps_aspect() {
        let opts = Options {
            width: 400,
            ..Options::default()
        };
        let out = rasterize(SQUARE, &opts).unwrap();
        assert_eq!(png_dims(&out), (400, 200));
    }

    #[test]
    fn explicit_width_and_height() {
        let opts = Options {
            width: 300,
            height: 300,
            ..Options::default()
        };
        let out = rasterize(SQUARE, &opts).unwrap();
        assert_eq!(png_dims(&out), (300, 300));
    }

    #[test]
    fn jpeg_output() {
        let opts = Options {
            format: OutputFormat::Jpeg,
            ..Options::default()
        };
        let out = rasterize(SQUARE, &opts).unwrap();
        // JPEG SOI marker.
        assert_eq!(&out[..2], &[0xFF, 0xD8], "JPEG magic");
    }

    #[test]
    fn background_color_fills_png() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"></svg>"#;
        let opts = Options {
            width: 2,
            height: 2,
            background: BgColor::Rgba(255, 0, 0, 255),
            ..Options::default()
        };
        let out = rasterize(svg, &opts).unwrap();
        // decode the PNG with the `png` crate and check the first pixel is red.
        let decoder = png::Decoder::new(out.as_slice());
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        // RGBA8 expected (tiny-skia emits 8-bit RGBA).
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(&buf[0..3], &[255, 0, 0], "first pixel red: {:?}", &buf[0..4]);
    }

    #[test]
    fn errors_on_empty() {
        assert!(rasterize("   ", &Options::default()).is_err());
    }

    #[test]
    fn errors_on_non_svg() {
        assert!(rasterize("<html>nope</html>", &Options::default()).is_err());
    }

    #[test]
    fn errors_on_bad_format() {
        assert!(OutputFormat::parse("gif").is_err());
        assert_eq!(OutputFormat::parse("PNG").unwrap(), OutputFormat::Png);
        assert_eq!(OutputFormat::parse("jpg").unwrap(), OutputFormat::Jpeg);
    }

    #[test]
    fn rejects_oversize() {
        let opts = Options {
            width: 9000,
            height: 9000,
            ..Options::default()
        };
        assert!(rasterize(SQUARE, &opts).is_err());
    }

    #[test]
    fn parses_bg_color() {
        assert_eq!(BgColor::parse("transparent").unwrap(), BgColor::Transparent);
        assert_eq!(BgColor::parse("#fff").unwrap(), BgColor::Rgba(255, 255, 255, 255));
        assert_eq!(BgColor::parse("#ff0000").unwrap(), BgColor::Rgba(255, 0, 0, 255));
        assert_eq!(
            BgColor::parse("00ff0080").unwrap(),
            BgColor::Rgba(0, 255, 0, 128)
        );
        assert!(BgColor::parse("#xyz").is_err());
    }
}
