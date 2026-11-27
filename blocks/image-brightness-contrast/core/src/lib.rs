//! gizza-ai/image-brightness-contrast core — adjust the brightness and contrast
//! of an image by signed amounts. Pure-Rust `image`. Returns PNG bytes.
//! Contrast is applied first (around mid-gray), then brightness, matching the
//! usual editor pipeline.

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat};
use serde::Deserialize;

/// The tool's options, shared by every surface: the chat/CLI handler flattens
/// them from its JSON args, the browser page deserializes them from its params
/// object. Missing fields take the schema defaults (0 = no change).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Params {
    /// -255..=255; negative darkens, positive brightens.
    pub brightness: i64,
    /// -100..=100; negative flattens, positive increases.
    pub contrast: f64,
}

impl Params {
    /// The values actually applied: brightness clamped to -255..=255, contrast
    /// to -100..=100 (a non-finite contrast means no change).
    pub fn normalized(&self) -> (i32, f32) {
        let brightness = self.brightness.clamp(-255, 255) as i32;
        let contrast = if self.contrast.is_finite() { self.contrast.clamp(-100.0, 100.0) as f32 } else { 0.0 };
        (brightness, contrast)
    }
}

/// Output filename for every surface.
pub const OUT_NAME: &str = "adjusted.png";

/// Apply `params` to an encoded image. Returns the PNG bytes and their filename.
pub fn apply(bytes: &[u8], params: &Params) -> Result<(Vec<u8>, String), String> {
    let (brightness, contrast) = params.normalized();
    Ok((adjust(bytes, brightness, contrast)?, OUT_NAME.to_string()))
}

/// Adjust `bytes`. `brightness` is added per channel (roughly -255..255);
/// `contrast` is a percentage-like factor (negative = less, positive = more,
/// 0 = unchanged). Returns PNG bytes.
pub fn adjust(bytes: &[u8], brightness: i32, contrast: f32) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let c = if contrast.is_finite() { contrast.clamp(-100.0, 100.0) } else { 0.0 };
    let b = brightness.clamp(-255, 255);

    // adjust_contrast pivots around 128; brighten adds a constant.
    let out_img = img.adjust_contrast(c).brighten(b);

    let mut out = Cursor::new(Vec::new());
    out_img
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn solid(w: u32, h: u32, c: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, c);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn brightness_increases_values() {
        let src = solid(4, 4, Rgba([100, 100, 100, 255]));
        let out = adjust(&src, 50, 0.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let p = img.get_pixel(0, 0);
        assert!(p.0[0] > 100, "brightness should raise the value, got {}", p.0[0]);
    }

    #[test]
    fn negative_brightness_darkens() {
        let src = solid(4, 4, Rgba([100, 100, 100, 255]));
        let out = adjust(&src, -40, 0.0).unwrap();
        let p = image::load_from_memory(&out).unwrap().get_pixel(0, 0);
        assert!(p.0[0] < 100);
    }

    #[test]
    fn contrast_pushes_dark_pixels_darker() {
        // a dark pixel (below mid-gray) should get darker with +contrast.
        let src = solid(4, 4, Rgba([80, 80, 80, 255]));
        let out = adjust(&src, 0, 60.0).unwrap();
        let p = image::load_from_memory(&out).unwrap().get_pixel(0, 0);
        assert!(p.0[0] < 80, "+contrast darkens sub-midtone, got {}", p.0[0]);
    }

    #[test]
    fn alpha_and_size_preserved() {
        let src = solid(5, 3, Rgba([10, 20, 30, 128]));
        let out = adjust(&src, 10, 10.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (5, 3));
        assert_eq!(img.get_pixel(0, 0).0[3], 128);
    }

    #[test]
    fn zero_zero_is_noop_dimensions() {
        let src = solid(6, 6, Rgba([123, 45, 67, 255]));
        let out = adjust(&src, 0, 0.0).unwrap();
        assert_eq!(image::load_from_memory(&out).unwrap().dimensions(), (6, 6));
    }

    #[test]
    fn params_default_to_no_change_and_clamp() {
        assert_eq!(Params::default().normalized(), (0, 0.0));
        let p = Params { brightness: 999, contrast: -500.0 };
        assert_eq!(p.normalized(), (255, -100.0));
        let nan = Params { brightness: 0, contrast: f64::NAN };
        assert_eq!(nan.normalized(), (0, 0.0));
    }

    #[test]
    fn params_deserialize_with_missing_fields() {
        let p: Params = serde_json::from_str(r#"{"brightness": 20}"#).unwrap();
        assert_eq!(p, Params { brightness: 20, contrast: 0.0 });
    }

    #[test]
    fn apply_returns_png_and_name() {
        let src = solid(4, 2, Rgba([100, 100, 100, 255]));
        let (out, name) = apply(&src, &Params { brightness: 30, contrast: 0.0 }).unwrap();
        assert_eq!(name, "adjusted.png");
        let img = image::load_from_memory_with_format(&out, ImageFormat::Png).unwrap();
        assert_eq!(img.dimensions(), (4, 2));
        assert!(img.get_pixel(0, 0).0[0] > 100);
    }

    #[test]
    fn errors() {
        assert!(adjust(b"not an image", 0, 0.0).is_err());
    }
}
