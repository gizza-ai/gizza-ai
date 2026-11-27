//! gizza-ai/image-hsl-adjust core — shift hue and scale saturation and lightness
//! of an image in HSL space. Pure-Rust (`image` for I/O, hand-rolled HSL math).
//! Returns PNG bytes (alpha preserved).

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};
use serde::Deserialize;

/// The tool's options, shared by every surface: the chat/CLI handler flattens
/// them from its JSON args, the browser page deserializes them from its params
/// object. Missing fields take the schema defaults (no change).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Params {
    /// Hue shift in degrees, -360..=360 (0 = no change).
    pub hue: f64,
    /// Saturation scale, 0..=4 (0 = grayscale, 1 = unchanged).
    pub saturation: f64,
    /// Lightness scale, 0..=4 (0 = black, 1 = unchanged).
    pub lightness: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self { hue: 0.0, saturation: 1.0, lightness: 1.0 }
    }
}

impl Params {
    /// The values actually applied, as `(hue, saturation, lightness)`: hue
    /// clamped to -360..=360, both scales to 0..=4. A non-finite value means
    /// "no change" for that field.
    pub fn normalized(&self) -> (f32, f32, f32) {
        let pick = |v: f64, lo: f64, hi: f64, none: f64| if v.is_finite() { v.clamp(lo, hi) } else { none } as f32;
        (pick(self.hue, -360.0, 360.0, 0.0), pick(self.saturation, 0.0, 4.0, 1.0), pick(self.lightness, 0.0, 4.0, 1.0))
    }
}

/// Output filename for every surface.
pub const OUT_NAME: &str = "hsl-adjusted.png";

/// Apply `params` to an encoded image. Returns the PNG bytes and their filename.
pub fn apply(bytes: &[u8], params: &Params) -> Result<(Vec<u8>, String), String> {
    let (hue, sat, light) = params.normalized();
    Ok((adjust(bytes, hue, sat, light)?, OUT_NAME.to_string()))
}

/// RGB (0..=255) → HSL (h in 0..360, s/l in 0..1).
fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < f32::EPSILON {
        return (0.0, 0.0, l); // achromatic
    }
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        ((g - b) / d) % 6.0
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    let h = if h < 0.0 { h + 360.0 } else { h };
    (h, s, l)
}

/// HSL → RGB (0..=255).
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    if s <= 0.0 {
        let v = (l * 255.0).round().clamp(0.0, 255.0) as u8;
        return (v, v, v);
    }
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let conv = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (conv(r1), conv(g1), conv(b1))
}

/// Adjust `bytes` in HSL: add `hue` degrees, multiply saturation by `ss`,
/// multiply lightness by `ls`. Expects already-normalized values (see
/// [`Params::normalized`]). Returns PNG bytes.
fn adjust(bytes: &[u8], hue: f32, ss: f32, ls: f32) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }

    let mut buf: RgbaImage = img.to_rgba8();
    for p in buf.pixels_mut() {
        let [r, g, b, a] = p.0;
        let (mut hh, mut s, mut l) = rgb_to_hsl(r, g, b);
        hh = (hh + hue).rem_euclid(360.0);
        s = (s * ss).clamp(0.0, 1.0);
        l = (l * ls).clamp(0.0, 1.0);
        let (nr, ng, nb) = hsl_to_rgb(hh, s, l);
        *p = Rgba([nr, ng, nb, a]);
    }

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(buf)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(c: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(4, 4, c);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }
    fn px0(png: &[u8]) -> Rgba<u8> {
        image::load_from_memory(png).unwrap().get_pixel(0, 0)
    }

    #[test]
    fn hsl_roundtrip() {
        for c in [(255, 0, 0), (0, 128, 64), (12, 200, 240), (255, 255, 255), (0, 0, 0)] {
            let (h, s, l) = rgb_to_hsl(c.0, c.1, c.2);
            let back = hsl_to_rgb(h, s, l);
            assert!((back.0 as i32 - c.0 as i32).abs() <= 1);
            assert!((back.1 as i32 - c.1 as i32).abs() <= 1);
            assert!((back.2 as i32 - c.2 as i32).abs() <= 1);
        }
    }

    #[test]
    fn hue_180_red_to_cyan() {
        let out = adjust(&solid(Rgba([255, 0, 0, 255])), 180.0, 1.0, 1.0).unwrap();
        let p = px0(&out);
        // red (h=0) rotated 180° → cyan (0, 255, 255)
        assert!(p.0[0] < 10 && p.0[1] > 245 && p.0[2] > 245, "got {:?}", p.0);
    }

    #[test]
    fn desaturate_to_gray() {
        let out = adjust(&solid(Rgba([200, 50, 50, 255])), 0.0, 0.0, 1.0).unwrap();
        let p = px0(&out);
        assert_eq!(p.0[0], p.0[1]);
        assert_eq!(p.0[1], p.0[2]); // fully desaturated → gray
    }

    #[test]
    fn lightness_zero_is_black_alpha_kept() {
        let out = adjust(&solid(Rgba([100, 150, 200, 128])), 0.0, 1.0, 0.0).unwrap();
        let p = px0(&out);
        assert_eq!([p.0[0], p.0[1], p.0[2]], [0, 0, 0]);
        assert_eq!(p.0[3], 128);
    }

    #[test]
    fn identity_keeps_color() {
        let out = adjust(&solid(Rgba([123, 45, 67, 255])), 0.0, 1.0, 1.0).unwrap();
        let p = px0(&out);
        assert!((p.0[0] as i32 - 123).abs() <= 1);
        assert!((p.0[1] as i32 - 45).abs() <= 1);
        assert!((p.0[2] as i32 - 67).abs() <= 1);
    }

    #[test]
    fn params_default_to_no_change_and_clamp() {
        assert_eq!(Params::default().normalized(), (0.0, 1.0, 1.0));
        let p = Params { hue: 999.0, saturation: -2.0, lightness: 10.0 };
        assert_eq!(p.normalized(), (360.0, 0.0, 4.0));
        let nan = Params { hue: f64::NAN, saturation: f64::INFINITY, lightness: f64::NAN };
        assert_eq!(nan.normalized(), (0.0, 1.0, 1.0));
    }

    #[test]
    fn params_deserialize_with_missing_fields() {
        let p: Params = serde_json::from_str(r#"{"hue": 90}"#).unwrap();
        assert_eq!(p, Params { hue: 90.0, saturation: 1.0, lightness: 1.0 });
        let empty: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, Params::default());
        assert!(serde_json::from_str::<Params>(r#"{"hue": "abc"}"#).is_err());
    }

    #[test]
    fn apply_returns_png_and_name() {
        let src = {
            let img = RgbaImage::from_pixel(4, 2, Rgba([255, 0, 0, 255]));
            let mut out = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
            out.into_inner()
        };
        let (out, name) = apply(&src, &Params { hue: 120.0, ..Params::default() }).unwrap();
        assert_eq!(name, "hsl-adjusted.png");
        let img = image::load_from_memory_with_format(&out, ImageFormat::Png).unwrap();
        assert_eq!(img.dimensions(), (4, 2));
        // red rotated 120° → pure green.
        assert_eq!(img.get_pixel(0, 0).0, [0, 255, 0, 255]);
    }

    /// Pixel values the page spec asserts (fixtures/red-2x2.png = solid red).
    #[test]
    fn spec_expectations() {
        let red = solid(Rgba([255, 0, 0, 255]));
        let p = |hue, saturation, lightness| px0(&apply(&red, &Params { hue, saturation, lightness }).unwrap().0).0;
        assert_eq!(p(180.0, 1.0, 1.0), [0, 255, 255, 255]);
        assert_eq!(p(0.0, 0.0, 1.0), [128, 128, 128, 255]);
        assert_eq!(p(0.0, 0.5, 1.0), [191, 64, 64, 255]);
        assert_eq!(p(0.0, 1.0, 0.5), [128, 0, 0, 255]);
    }

    #[test]
    fn errors() {
        assert!(adjust(b"not an image", 0.0, 1.0, 1.0).is_err());
        let err = apply(b"not an image", &Params::default()).unwrap_err();
        assert!(err.starts_with("could not decode image"), "{err}");
    }
}
