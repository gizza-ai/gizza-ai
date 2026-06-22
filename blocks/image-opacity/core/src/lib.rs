//! gizza-ai/image-opacity core — set or scale the alpha/opacity channel of an
//! image, producing a semi-transparent PNG. Pure-Rust (`image` for I/O).
//! Always returns PNG bytes so the alpha channel is preserved.

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

/// How the `opacity` value is applied to each pixel's alpha channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Multiply every pixel's existing alpha by `opacity` (existing
    /// transparency is preserved/attenuated). 1.0 = no change.
    Scale,
    /// Overwrite every pixel's alpha with `opacity * 255` (uniform opacity,
    /// regardless of the source's existing transparency).
    Set,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "scale" | "multiply" => Ok(Mode::Scale),
            "set" | "replace" | "uniform" => Ok(Mode::Set),
            other => Err(format!("unknown mode '{other}' (use 'scale' or 'set')")),
        }
    }
}

/// Apply `opacity` (0.0 = fully transparent, 1.0 = fully opaque) to `bytes`
/// using `mode`. Returns PNG bytes (RGBA, so alpha is retained).
pub fn apply(bytes: &[u8], opacity: f32, mode: Mode) -> Result<Vec<u8>, String> {
    if !opacity.is_finite() {
        return Err("opacity must be a finite number".into());
    }
    let opacity = opacity.clamp(0.0, 1.0);
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }

    let mut buf: RgbaImage = img.to_rgba8();
    for p in buf.pixels_mut() {
        let [r, g, b, a] = p.0;
        let new_a = match mode {
            Mode::Scale => (a as f32 * opacity).round().clamp(0.0, 255.0) as u8,
            Mode::Set => (opacity * 255.0).round().clamp(0.0, 255.0) as u8,
        };
        *p = Rgba([r, g, b, new_a]);
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
    fn mode_parse() {
        assert_eq!(Mode::parse("scale").unwrap(), Mode::Scale);
        assert_eq!(Mode::parse("Multiply").unwrap(), Mode::Scale);
        assert_eq!(Mode::parse("SET").unwrap(), Mode::Set);
        assert_eq!(Mode::parse("replace").unwrap(), Mode::Set);
        assert!(Mode::parse("nope").is_err());
    }

    #[test]
    fn scale_half_opaque() {
        let out = apply(&solid(Rgba([10, 20, 30, 255])), 0.5, Mode::Scale).unwrap();
        let p = px0(&out);
        // RGB preserved, alpha halved
        assert_eq!([p.0[0], p.0[1], p.0[2]], [10, 20, 30]);
        assert_eq!(p.0[3], 128); // 255*0.5 = 127.5 → 128 rounded
    }

    #[test]
    fn scale_respects_existing_alpha() {
        // existing alpha 100, scale by 0.5 → 50
        let out = apply(&solid(Rgba([10, 20, 30, 100])), 0.5, Mode::Scale).unwrap();
        assert_eq!(px0(&out).0[3], 50);
    }

    #[test]
    fn set_overwrites_alpha() {
        // existing alpha 100, set to 0.25 → 64 regardless of source
        let out = apply(&solid(Rgba([10, 20, 30, 100])), 0.25, Mode::Set).unwrap();
        assert_eq!(px0(&out).0[3], 64); // 0.25*255 = 63.75 → 64
    }

    #[test]
    fn zero_is_fully_transparent() {
        let out = apply(&solid(Rgba([10, 20, 30, 255])), 0.0, Mode::Set).unwrap();
        assert_eq!(px0(&out).0[3], 0);
    }

    #[test]
    fn one_scale_is_identity() {
        let out = apply(&solid(Rgba([10, 20, 30, 200])), 1.0, Mode::Scale).unwrap();
        let p = px0(&out);
        assert_eq!(p.0, [10, 20, 30, 200]);
    }

    #[test]
    fn out_of_range_clamps() {
        let out = apply(&solid(Rgba([10, 20, 30, 255])), 5.0, Mode::Set).unwrap();
        assert_eq!(px0(&out).0[3], 255);
        let out = apply(&solid(Rgba([10, 20, 30, 255])), -3.0, Mode::Set).unwrap();
        assert_eq!(px0(&out).0[3], 0);
    }

    #[test]
    fn errors() {
        assert!(apply(b"not an image", 0.5, Mode::Scale).is_err());
        assert!(apply(&solid(Rgba([1, 2, 3, 255])), f32::NAN, Mode::Scale).is_err());
    }
}
