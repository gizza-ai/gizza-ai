//! gizza-ai/vectorize core — pure raster→SVG tracing.
//!
//! Wraps the `vtracer` (visioncortex) crate: decode PNG/JPEG/WebP/GIF/etc.
//! bytes to an RGBA `ColorImage`, run the cluster-tracer, and serialize the
//! resulting paths to an SVG document string. No I/O — bytes in, SVG text out,
//! so it builds for `wasm32-wasip1` (the chat/CLI surface) and native (tests).

use vtracer::{convert, ColorImage, ColorMode, Config};

/// Color-handling mode for the trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Full multi-color trace (logos, icons, posters). Produces stacked
    /// color clusters.
    Color,
    /// Black-and-white / single-layer trace (sketches, line art). Pixels
    /// darker than mid-grey become filled paths.
    Bw,
}

impl Mode {
    /// Parse the user-facing `mode` string. Accepts `color` (default) and
    /// `bw` / `binary` / `black-and-white`.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "color" | "colour" => Ok(Mode::Color),
            "bw" | "binary" | "black-and-white" | "blackandwhite" => Ok(Mode::Bw),
            other => Err(format!("unknown mode '{other}' (expected 'color' or 'bw')")),
        }
    }
}

/// Tracing knobs. Kept deliberately small — sane defaults match vtracer's own
/// `Config::default()`, callers only override what they care about.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Color vs black-and-white trace.
    pub mode: Mode,
    /// Number of significant bits per color channel kept when clustering,
    /// `1..=8`. Higher = more colors / larger SVG, lower = flatter / smaller.
    /// Only affects `Mode::Color`. Defaults to vtracer's `6`.
    pub color_precision: i32,
    /// Discard speckles whose area is below `filter_speckle^2` pixels,
    /// `0..=128`. Higher = cleaner output, fewer tiny artifacts. Defaults to
    /// vtracer's `4`.
    pub filter_speckle: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Color,
            color_precision: 6,
            filter_speckle: 4,
        }
    }
}

/// Maximum decoded image dimension on a side. visioncortex clustering is
/// O(pixels); cap the input so a chat/CLI call can't be handed a 50MP photo
/// that runs for minutes. 2000px covers any logo/icon/sketch use case.
const MAX_DIMENSION: usize = 2000;

/// Trace `bytes` (a PNG / JPEG / WebP / GIF / BMP raster) into an SVG document.
///
/// Returns the SVG as a UTF-8 string (starting with the `<?xml ...?>` prolog
/// and a `<svg ...>` root) on success, or a human-readable error string.
pub fn vectorize(bytes: &[u8], opts: Options) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("empty image input".into());
    }
    if !(1..=8).contains(&opts.color_precision) {
        return Err(format!(
            "color_precision must be 1-8, got {}",
            opts.color_precision
        ));
    }
    if opts.filter_speckle > 128 {
        return Err(format!(
            "filter_speckle must be 0-128, got {}",
            opts.filter_speckle
        ));
    }

    let dynimg = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let rgba = dynimg.to_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    if width == 0 || height == 0 {
        return Err("image has zero width or height".into());
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(format!(
            "image too large: {width}x{height} (max {MAX_DIMENSION}px per side)"
        ));
    }

    let img = ColorImage {
        pixels: rgba.into_raw(),
        width,
        height,
    };

    let mut cfg = Config {
        color_mode: match opts.mode {
            Mode::Color => ColorMode::Color,
            Mode::Bw => ColorMode::Binary,
        },
        ..Config::default()
    };
    cfg.color_precision = opts.color_precision;
    cfg.filter_speckle = opts.filter_speckle;

    let svg = convert(img, cfg)?;
    Ok(svg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a tiny solid-color RGBA raster as an in-memory PNG so tests don't
    /// depend on any fixture file.
    fn tiny_png(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageFormat, RgbaImage};
        // Two diagonal quadrants of different colors so the tracer has at least
        // one real boundary to emit a path for.
        let mut buf = RgbaImage::new(width, height);
        for (x, y, px) in buf.enumerate_pixels_mut() {
            *px = if x + y < (width + height) / 2 {
                image::Rgba([20, 30, 200, 255])
            } else {
                image::Rgba([230, 40, 30, 255])
            };
        }
        let mut out = std::io::Cursor::new(Vec::new());
        buf.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn traces_tiny_raster_to_svg_color() {
        let png = tiny_png(16, 16);
        let svg = vectorize(&png, Options::default()).expect("vectorize should succeed");
        assert!(svg.contains("<svg"), "output should contain an <svg root: {svg}");
        assert!(svg.contains("width=\"16\""), "svg should carry the source width");
        assert!(svg.contains("<path"), "a 2-color raster should emit at least one path");
    }

    #[test]
    fn traces_tiny_raster_to_svg_bw() {
        let png = tiny_png(20, 20);
        let opts = Options {
            mode: Mode::Bw,
            ..Options::default()
        };
        let svg = vectorize(&png, opts).expect("bw vectorize should succeed");
        assert!(svg.contains("<svg"), "bw output should contain an <svg root");
    }

    #[test]
    fn rejects_non_image_bytes() {
        let err = vectorize(b"this is not an image at all", Options::default()).unwrap_err();
        assert!(
            err.contains("could not decode image"),
            "bad bytes should report a decode error, got: {err}"
        );
    }

    #[test]
    fn rejects_empty_input() {
        let err = vectorize(&[], Options::default()).unwrap_err();
        assert!(err.contains("empty"), "empty input should be rejected, got: {err}");
    }

    #[test]
    fn rejects_out_of_range_color_precision() {
        let png = tiny_png(8, 8);
        let opts = Options {
            color_precision: 0,
            ..Options::default()
        };
        let err = vectorize(&png, opts).unwrap_err();
        assert!(err.contains("color_precision"), "got: {err}");
    }

    #[test]
    fn mode_parse_accepts_aliases() {
        assert_eq!(Mode::parse("color").unwrap(), Mode::Color);
        assert_eq!(Mode::parse(" COLOR ").unwrap(), Mode::Color);
        assert_eq!(Mode::parse("bw").unwrap(), Mode::Bw);
        assert_eq!(Mode::parse("binary").unwrap(), Mode::Bw);
        assert!(Mode::parse("rainbow").is_err());
    }
}
