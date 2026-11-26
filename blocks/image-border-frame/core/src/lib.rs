//! gizza-ai/image-border-frame core — add a solid border/frame of a chosen color
//! and thickness around an image. Pure-Rust `image`. Returns PNG bytes (alpha
//! preserved). Independent top/right/bottom/left thickness via `thickness` (all
//! sides) — a uniform frame.

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};
use serde::Deserialize;

/// Largest accepted border thickness in pixels (matches the schema maximum).
pub const MAX_THICKNESS: u64 = 2000;

/// Output filename for every surface.
pub const OUT_NAME: &str = "framed.png";

/// The tool's options, shared by every surface: the chat/CLI handler flattens
/// them from its JSON args, the browser page deserializes them from its params
/// object. Missing fields take the schema defaults (20 px, black).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Params {
    /// Border width in pixels on every side; clamped to `0..=MAX_THICKNESS`.
    pub thickness: u64,
    /// `#rgb`, `#rrggbb`, or `#rrggbbaa`.
    pub color: String,
}

impl Default for Params {
    fn default() -> Self {
        Self { thickness: 20, color: "#000000".to_string() }
    }
}

impl Params {
    /// The thickness actually applied (clamped to `MAX_THICKNESS`).
    pub fn thickness_px(&self) -> u32 {
        self.thickness.min(MAX_THICKNESS) as u32
    }
}

/// Apply `params` to an encoded image. Returns the PNG bytes and their filename.
/// The color is validated before the image is decoded.
pub fn apply(bytes: &[u8], params: &Params) -> Result<(Vec<u8>, String), String> {
    let color = parse_color(&params.color)?;
    Ok((add_border(bytes, params.thickness_px(), color)?, OUT_NAME.to_string()))
}

/// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa` into RGBA.
pub fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
    let h = s.trim().trim_start_matches('#');
    let v = |a: &str| u8::from_str_radix(a, 16).map_err(|_| format!("invalid color '{s}'"));
    let (r, g, b, a) = match h.len() {
        3 => {
            let cs: Vec<char> = h.chars().collect();
            let d = |c: char| v(&c.to_string().repeat(2));
            (d(cs[0])?, d(cs[1])?, d(cs[2])?, 255)
        }
        6 => (v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?, 255),
        8 => (v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?, v(&h[6..8])?),
        _ => return Err(format!("invalid color '{s}' (use #rgb, #rrggbb, or #rrggbbaa)")),
    };
    Ok(Rgba([r, g, b, a]))
}

/// Add a uniform border of `thickness` pixels in `color` around `bytes`.
/// Returns PNG bytes. The output is `(w + 2t) × (h + 2t)`.
pub fn add_border(bytes: &[u8], thickness: u32, color: Rgba<u8>) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let t = thickness;
    let nw = w + 2 * t;
    let nh = h + 2 * t;
    let mut canvas: RgbaImage = RgbaImage::from_pixel(nw, nh, color);
    let src = img.to_rgba8();
    // `replace`, not `overlay`: the frame is only the band around the image.
    // Compositing the image OVER a canvas filled with the frame color would
    // flood the image's own transparent pixels with that color.
    image::imageops::replace(&mut canvas, &src, t as i64, t as i64);

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, c: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, c);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#fff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("#ff8800").unwrap(), Rgba([255, 136, 0, 255]));
        assert_eq!(parse_color("#00000080").unwrap(), Rgba([0, 0, 0, 128]));
        assert!(parse_color("zzz").is_err());
    }

    #[test]
    fn grows_by_twice_thickness() {
        let src = png(10, 6, Rgba([0, 0, 0, 255]));
        let out = add_border(&src, 4, Rgba([255, 0, 0, 255])).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (18, 14)); // 10+8, 6+8
    }

    #[test]
    fn corner_is_border_color_center_is_original() {
        let src = png(4, 4, Rgba([0, 0, 255, 255])); // blue image
        let out = add_border(&src, 2, Rgba([255, 0, 0, 255])).unwrap(); // red border
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0), Rgba([255, 0, 0, 255])); // corner = border
        assert_eq!(img.get_pixel(4, 4), Rgba([0, 0, 255, 255])); // inside = original
    }

    #[test]
    fn transparent_pixels_inside_the_frame_stay_transparent() {
        // 4x4: transparent except one opaque blue pixel and one half-alpha pixel.
        let mut src = RgbaImage::from_pixel(4, 4, Rgba([0, 200, 0, 0]));
        src.put_pixel(1, 1, Rgba([0, 0, 255, 255]));
        src.put_pixel(2, 2, Rgba([0, 0, 255, 128]));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(src).write_to(&mut bytes, ImageFormat::Png).unwrap();
        let out = add_border(&bytes.into_inner(), 2, Rgba([255, 0, 0, 255])).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0), Rgba([255, 0, 0, 255])); // frame
        assert_eq!(img.get_pixel(2, 2)[3], 0, "transparent pixel must not take the frame color");
        assert_eq!(img.get_pixel(3, 3), Rgba([0, 0, 255, 255]));
        assert_eq!(img.get_pixel(4, 4), Rgba([0, 0, 255, 128]), "partial alpha kept as-is");
    }

    #[test]
    fn zero_thickness_is_noop_size() {
        let src = png(5, 5, Rgba([1, 2, 3, 255]));
        let out = add_border(&src, 0, Rgba([0, 0, 0, 255])).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (5, 5));
    }

    #[test]
    fn params_defaults_and_clamp() {
        let d = Params::default();
        assert_eq!(d, Params { thickness: 20, color: "#000000".into() });
        assert_eq!(d.thickness_px(), 20);
        assert_eq!(Params { thickness: 99_999, ..d }.thickness_px(), 2000);
    }

    #[test]
    fn params_deserialize_with_missing_fields() {
        let p: Params = serde_json::from_str(r#"{"thickness": 3}"#).unwrap();
        assert_eq!(p, Params { thickness: 3, color: "#000000".into() });
        let p: Params = serde_json::from_str(r##"{"color": "#fff"}"##).unwrap();
        assert_eq!(p, Params { thickness: 20, color: "#fff".into() });
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p, Params::default());
    }

    #[test]
    fn apply_rejects_bad_color() {
        let src = png(2, 2, Rgba([255, 0, 0, 255]));
        let e = apply(&src, &Params { thickness: 1, color: "nope".into() }).unwrap_err();
        assert!(e.contains("invalid color"), "{e}");
    }

    #[test]
    fn apply_returns_framed_png_and_name() {
        // Mirrors the page spec: red 2x2, 3px #0000ff border → 8x8.
        let src = png(2, 2, Rgba([255, 0, 0, 255]));
        let (out, name) = apply(&src, &Params { thickness: 3, color: "#0000ff".into() }).unwrap();
        assert_eq!(name, "framed.png");
        let img = image::load_from_memory_with_format(&out, ImageFormat::Png).unwrap();
        assert_eq!(img.dimensions(), (8, 8));
        assert_eq!(img.get_pixel(0, 0), Rgba([0, 0, 255, 255]));
        assert_eq!(img.get_pixel(7, 7), Rgba([0, 0, 255, 255]));
        assert_eq!(img.get_pixel(3, 3), Rgba([255, 0, 0, 255]));
        assert_eq!(img.get_pixel(4, 4), Rgba([255, 0, 0, 255]));
        assert_eq!(img.get_pixel(2, 3), Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn apply_defaults_on_a_photo_sized_image() {
        // Worked example in page/content.md: 800x600 + default 20px → 840x640.
        let src = png(800, 600, Rgba([10, 20, 30, 255]));
        let (out, _) = apply(&src, &Params::default()).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (840, 640));
        assert_eq!(img.get_pixel(0, 0), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn errors() {
        assert!(add_border(b"not an image", 2, Rgba([0, 0, 0, 255])).is_err());
    }
}
