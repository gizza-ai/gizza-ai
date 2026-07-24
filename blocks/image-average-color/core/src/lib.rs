//! gizza-ai/image-average-color core — compute the single mean color of an image.
//! No wafer/wasm-bindgen deps. Pure-Rust (`image` decode only). Shared by the chat
//! skill block, the CLI, and the unit tests.
//!
//! Two means are computed from every counted pixel:
//!   * `simple` — the naive per-channel arithmetic mean of the 0-255 sRGB values
//!     (what most "average color" tools return).
//!   * `gamma_correct` — the mean taken in LINEAR light: each channel is decoded
//!     sRGB→linear, averaged, then encoded linear→sRGB. This is the perceptually
//!     correct average (blending light, not gamma-encoded numbers) and is the one
//!     that matches how the colors actually mix to the eye.
//!
//! Alpha handling: by default pixels whose alpha is below `ALPHA_THRESHOLD` are
//! treated as transparent and excluded (a transparent PNG background shouldn't
//! drag the mean toward black). Set `ignore_transparency = false` to fold every
//! pixel in.

use image::GenericImageView;

/// One mean color expressed in several notations.
#[derive(Debug, Clone, PartialEq)]
pub struct MeanColor {
    /// `#rrggbb` (lowercase).
    pub hex: String,
    /// `#rrggbbaa` (lowercase) including the mean alpha.
    pub hex_rgba: String,
    /// CSS `rgb(r, g, b)`.
    pub rgb: String,
    /// CSS `rgba(r, g, b, a)` with alpha 0-1 (2 decimals).
    pub rgba: String,
    /// CSS `hsl(H, S%, L%)`.
    pub hsl: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Mean alpha, 0-255.
    pub a: u8,
    /// Hue 0-360 (degrees), rounded.
    pub h: u16,
    /// Saturation 0-100 (%), rounded.
    pub s: u8,
    /// Lightness 0-100 (%), rounded.
    pub l: u8,
}

/// The full result of averaging an image.
#[derive(Debug, Clone, PartialEq)]
pub struct Average {
    pub width: u32,
    pub height: u32,
    /// Number of pixels that contributed to the mean (opaque pixels when
    /// transparency is ignored, otherwise every pixel).
    pub pixels_counted: u64,
    /// Naive per-channel arithmetic mean in sRGB space.
    pub simple: MeanColor,
    /// Perceptually correct mean taken in linear light.
    pub gamma_correct: MeanColor,
    /// Relative luminance of the gamma-correct mean, 0-100 (perceived brightness).
    pub brightness: u8,
    /// True when the gamma-correct mean is dark (luminance < 50%), i.e. white text
    /// reads better on it than black text.
    pub is_dark: bool,
    /// `#rrggbb` complementary of the gamma-correct mean (channels inverted).
    pub complementary_hex: String,
}

/// Pixels with alpha below this are treated as transparent when
/// `ignore_transparency` is true.
const ALPHA_THRESHOLD: u8 = 16;

fn to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// sRGB 0-255 channel → linear 0-1 (IEC 61966-2-1).
fn srgb_to_linear(c: u8) -> f64 {
    let cf = c as f64 / 255.0;
    if cf <= 0.04045 {
        cf / 12.92
    } else {
        ((cf + 0.055) / 1.055).powf(2.4)
    }
}

/// linear 0-1 → sRGB 0-255 channel.
fn linear_to_srgb(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// RGB (0-255) → HSL with H in 0..360 deg, S/L in 0..100 %.
fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
    let rf = r as f64 / 255.0;
    let gf = g as f64 / 255.0;
    let bf = b as f64 / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let l = (max + min) / 2.0;
    let d = max - min;
    let (h, s) = if d.abs() < f64::EPSILON {
        (0.0, 0.0)
    } else {
        let s = d / (1.0 - (2.0 * l - 1.0).abs());
        let h = if (max - rf).abs() < f64::EPSILON {
            ((gf - bf) / d).rem_euclid(6.0)
        } else if (max - gf).abs() < f64::EPSILON {
            (bf - rf) / d + 2.0
        } else {
            (rf - gf) / d + 4.0
        };
        (h * 60.0, s)
    };
    (
        h.round() as u16 % 360,
        (s * 100.0).round() as u8,
        (l * 100.0).round() as u8,
    )
}

fn mean_color(r: u8, g: u8, b: u8, a: u8) -> MeanColor {
    let (h, s, l) = rgb_to_hsl(r, g, b);
    MeanColor {
        hex: to_hex(r, g, b),
        hex_rgba: format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
        rgb: format!("rgb({r}, {g}, {b})"),
        rgba: format!("rgba({r}, {g}, {b}, {:.2})", a as f64 / 255.0),
        hsl: format!("hsl({h}, {s}%, {l}%)"),
        r,
        g,
        b,
        a,
        h,
        s,
        l,
    }
}

/// Compute the mean color(s) of the image `bytes`.
///
/// When `ignore_transparency` is true, pixels whose alpha is below the
/// transparency threshold are excluded. When false, every pixel contributes
/// (fully transparent pixels still carry their RGB and pull the mean alpha down).
pub fn average(bytes: &[u8], ignore_transparency: bool) -> Result<Average, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let rgba = img.to_rgba8();

    // Accumulators. sRGB sums stay integer for exactness; linear sums are f64.
    let mut sum_r: u64 = 0;
    let mut sum_g: u64 = 0;
    let mut sum_b: u64 = 0;
    let mut sum_a: u64 = 0;
    let mut lin_r: f64 = 0.0;
    let mut lin_g: f64 = 0.0;
    let mut lin_b: f64 = 0.0;
    let mut counted: u64 = 0;

    for px in rgba.pixels() {
        let [pr, pg, pb, pa] = px.0;
        if ignore_transparency && pa < ALPHA_THRESHOLD {
            continue;
        }
        sum_r += pr as u64;
        sum_g += pg as u64;
        sum_b += pb as u64;
        sum_a += pa as u64;
        lin_r += srgb_to_linear(pr);
        lin_g += srgb_to_linear(pg);
        lin_b += srgb_to_linear(pb);
        counted += 1;
    }

    // If ignoring transparency left nothing (a fully transparent image), fall back
    // to counting every pixel so we still return a meaningful (transparent) mean.
    if counted == 0 {
        for px in rgba.pixels() {
            let [pr, pg, pb, pa] = px.0;
            sum_r += pr as u64;
            sum_g += pg as u64;
            sum_b += pb as u64;
            sum_a += pa as u64;
            lin_r += srgb_to_linear(pr);
            lin_g += srgb_to_linear(pg);
            lin_b += srgb_to_linear(pb);
            counted += 1;
        }
    }
    let n = counted.max(1);
    let nf = n as f64;

    // Simple: round-half arithmetic mean of the raw sRGB values.
    let simple_r = ((sum_r as f64) / nf).round() as u8;
    let simple_g = ((sum_g as f64) / nf).round() as u8;
    let simple_b = ((sum_b as f64) / nf).round() as u8;
    let mean_a = ((sum_a as f64) / nf).round().clamp(0.0, 255.0) as u8;
    let simple = mean_color(simple_r, simple_g, simple_b, mean_a);

    // Gamma-correct: average in linear light, then re-encode to sRGB.
    let gr = linear_to_srgb(lin_r / nf);
    let gg = linear_to_srgb(lin_g / nf);
    let gb = linear_to_srgb(lin_b / nf);
    let gamma_correct = mean_color(gr, gg, gb, mean_a);

    // Relative luminance (WCAG) of the gamma-correct mean, 0-100.
    let lum = 0.2126 * (lin_r / nf) + 0.7152 * (lin_g / nf) + 0.0722 * (lin_b / nf);
    let brightness = (lum * 100.0).round().clamp(0.0, 100.0) as u8;
    let is_dark = lum < 0.5;

    let complementary_hex = to_hex(255 - gr, 255 - gg, 255 - gb);

    Ok(Average {
        width: w,
        height: h,
        pixels_counted: n,
        simple,
        gamma_correct,
        brightness,
        is_dark,
        complementary_hex,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn encode(img: RgbaImage) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    fn solid(r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        let mut img = RgbaImage::new(4, 4);
        for p in img.pixels_mut() {
            *p = Rgba([r, g, b, a]);
        }
        encode(img)
    }

    #[test]
    fn solid_color_averages_to_itself() {
        let a = average(&solid(10, 120, 200, 255), true).unwrap();
        assert_eq!((a.width, a.height), (4, 4));
        assert_eq!(a.pixels_counted, 16);
        // A solid image's mean IS that color, in both methods.
        assert_eq!(a.simple.hex, "#0a78c8");
        assert_eq!(a.gamma_correct.hex, "#0a78c8");
        assert_eq!(a.simple.rgb, "rgb(10, 120, 200)");
    }

    #[test]
    fn gamma_correct_differs_from_simple_on_black_white() {
        // Half black, half white. Simple mean = 128 (mid-gray). Gamma-correct mean
        // averages linear light (0 and 1 → 0.5 linear → ~188 sRGB), which is much
        // brighter — the classic gamma-averaging demonstration.
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([255, 255, 255, 255]));
        let a = average(&encode(img), true).unwrap();
        assert_eq!(a.simple.hex, "#808080", "simple mean of b/w is mid-gray");
        assert_eq!(
            a.gamma_correct.hex, "#bcbcbc",
            "gamma-correct mean of b/w is brighter"
        );
        assert!(
            a.gamma_correct.r > a.simple.r,
            "gamma-correct must be brighter than simple here"
        );
    }

    #[test]
    fn transparent_pixels_ignored_by_default() {
        // Left half opaque red, right half fully transparent. Ignoring
        // transparency, the mean is pure red — not dragged toward the (0,0,0,0).
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 0]));
        let a = average(&encode(img), true).unwrap();
        assert_eq!(a.pixels_counted, 1);
        assert_eq!(a.simple.hex, "#ff0000");
        assert_eq!(a.simple.a, 255);
    }

    #[test]
    fn including_transparency_folds_all_pixels() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 0]));
        let a = average(&encode(img), false).unwrap();
        assert_eq!(a.pixels_counted, 2);
        // Both pixels counted → mean red channel ~128, mean alpha ~128.
        assert_eq!(a.simple.r, 128);
        assert_eq!(a.simple.a, 128);
    }

    #[test]
    fn dark_image_is_flagged_dark_with_complement() {
        let a = average(&solid(10, 10, 10, 255), true).unwrap();
        assert!(a.is_dark);
        assert!(a.brightness < 10);
        assert_eq!(a.complementary_hex, "#f5f5f5");
    }

    #[test]
    fn light_image_is_not_dark() {
        let a = average(&solid(240, 240, 240, 255), true).unwrap();
        assert!(!a.is_dark);
        assert!(a.brightness > 80);
    }

    #[test]
    fn hsl_and_rgba_notation_is_populated() {
        let a = average(&solid(255, 0, 0, 255), true).unwrap();
        assert_eq!(a.simple.hsl, "hsl(0, 100%, 50%)");
        assert_eq!(a.simple.rgba, "rgba(255, 0, 0, 1.00)");
    }

    #[test]
    fn rejects_garbage() {
        assert!(average(b"not an image", true).is_err());
    }
}
