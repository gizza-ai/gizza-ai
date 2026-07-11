//! gizza-ai/image-document-shadow-remove core — remove cast shadows / uneven
//! lighting from a photo of a document to produce a clean, flat, near-white page.
//!
//! No wafer/wasm-bindgen deps. Pure-Rust `image` crate (decode → per-channel
//! illumination estimate → division-normalise → encode PNG), so it runs on every
//! backend including the chat Service Worker.
//!
//! Algorithm (per output channel):
//!   1. Estimate the local "paper white" background: downscale the channel to a
//!      low resolution (the illumination envelope is low-frequency), run a
//!      max-filter (dilation) to erase dark text/ink, blur, then upscale back.
//!   2. Division-normalise: `out = clamp(pixel / background * 255)`. A shadowed
//!      region has a correspondingly darker local background, so the division
//!      flattens the shadow gradient and lifts the page to uniform white while
//!      preserving strokes.
//!   3. Apply a white-point lift (`whiteness`) so near-white paper snaps to 255.
//!
//! Modes:
//!   * `color`      — normalise each RGB channel; keeps coloured ink/highlights.
//!   * `grayscale`  — normalise the luma channel; neutral gray output.
//!   * `blackwhite` — grayscale, then an Otsu threshold for a crisp 1-bit look.

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, GrayImage, ImageFormat, Luma, Rgb, RgbImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Color,
    Grayscale,
    BlackWhite,
}

pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "color" | "colour" | "rgb" => Ok(Mode::Color),
        "grayscale" | "greyscale" | "gray" | "grey" | "mono" => Ok(Mode::Grayscale),
        "blackwhite" | "black-white" | "bw" | "binary" | "threshold" => Ok(Mode::BlackWhite),
        other => Err(format!(
            "mode {other:?} not supported (color|grayscale|blackwhite)"
        )),
    }
}

/// Remove shadows / uneven lighting from `bytes`. `whiteness` is 0..100 (how hard
/// the background is snapped to pure white). Returns PNG bytes.
pub fn remove_shadow(bytes: &[u8], mode: Mode, whiteness: u32) -> Result<Vec<u8>, String> {
    let src = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    if src.width() == 0 || src.height() == 0 {
        return Err("image has zero dimensions".into());
    }
    let wp = white_point(whiteness);

    let out: DynamicImage = match mode {
        Mode::Color => {
            let rgb: RgbImage = src.to_rgb8();
            let (w, h) = rgb.dimensions();
            // Split into channels, normalise each independently, recombine.
            let mut chans: Vec<GrayImage> = (0..3)
                .map(|c| {
                    let mut ch = GrayImage::new(w, h);
                    for (x, y, p) in rgb.enumerate_pixels() {
                        ch.put_pixel(x, y, Luma([p[c]]));
                    }
                    normalize_channel(&ch, wp)
                })
                .collect();
            let b = chans.pop().unwrap();
            let g = chans.pop().unwrap();
            let r = chans.pop().unwrap();
            let mut outi = RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    outi.put_pixel(
                        x,
                        y,
                        Rgb([
                            r.get_pixel(x, y)[0],
                            g.get_pixel(x, y)[0],
                            b.get_pixel(x, y)[0],
                        ]),
                    );
                }
            }
            DynamicImage::ImageRgb8(outi)
        }
        Mode::Grayscale => {
            let gray = normalize_channel(&src.to_luma8(), wp);
            DynamicImage::ImageLuma8(gray)
        }
        Mode::BlackWhite => {
            let gray = normalize_channel(&src.to_luma8(), wp);
            let t = otsu_threshold(&gray);
            let mut outi = gray;
            for p in outi.pixels_mut() {
                p[0] = if p[0] >= t { 255 } else { 0 };
            }
            DynamicImage::ImageLuma8(outi)
        }
    };

    let mut buf = Cursor::new(Vec::new());
    out.write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(buf.into_inner())
}

/// `whiteness` 0..100 → the pre-lift value that maps to pure white (255). Higher
/// whiteness lowers the white point, so more of the near-white paper snaps to 255.
/// 0 = no extra lift (255), 100 = aggressive (~165).
fn white_point(whiteness: u32) -> f32 {
    let w = whiteness.min(100) as f32;
    255.0 - (w / 100.0) * 90.0
}

/// Division-normalise one channel against its estimated background, then lift the
/// white point.
fn normalize_channel(ch: &GrayImage, wp: f32) -> GrayImage {
    let (w, h) = ch.dimensions();
    let bg = estimate_background(ch);
    let mut out = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let v = ch.get_pixel(x, y)[0] as f32;
            let b = (bg.get_pixel(x, y)[0] as f32).max(1.0);
            // Illumination-flattened value, then white-point lift.
            let flat = (v / b) * 255.0;
            let lifted = flat * 255.0 / wp;
            out.put_pixel(x, y, Luma([lifted.clamp(0.0, 255.0).round() as u8]));
        }
    }
    out
}

/// Estimate the low-frequency "paper white" background of a channel. Works on a
/// downscaled copy (illumination is low-frequency and this keeps it fast on large
/// scans), max-filters to erase dark text, blurs, then upscales back to full size.
fn estimate_background(ch: &GrayImage) -> GrayImage {
    let (w, h) = ch.dimensions();
    let max_dim = w.max(h);
    // Cap the working resolution so big scans stay fast.
    const TARGET: u32 = 220;
    let (sw, sh) = if max_dim > TARGET {
        let s = TARGET as f32 / max_dim as f32;
        (
            ((w as f32 * s).round() as u32).max(1),
            ((h as f32 * s).round() as u32).max(1),
        )
    } else {
        (w, h)
    };
    let small = if (sw, sh) == (w, h) {
        ch.clone()
    } else {
        image::imageops::resize(ch, sw, sh, FilterType::Triangle)
    };
    // Dilation radius ~5% of the working size erases text but keeps illumination.
    let radius = ((sw.max(sh) / 20).max(1)) as i32;
    let dilated = max_filter(&small, radius);
    let blurred = image::imageops::blur(&dilated, (radius as f32).max(1.0));
    if (sw, sh) == (w, h) {
        blurred
    } else {
        image::imageops::resize(&blurred, w, h, FilterType::Triangle)
    }
}

/// Separable max filter (grayscale dilation) with a square window of radius `r`.
fn max_filter(img: &GrayImage, r: i32) -> GrayImage {
    let (w, h) = img.dimensions();
    let mut tmp = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut m = 0u8;
            for dx in -r..=r {
                let xx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                m = m.max(img.get_pixel(xx, y)[0]);
            }
            tmp.put_pixel(x, y, Luma([m]));
        }
    }
    let mut out = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut m = 0u8;
            for dy in -r..=r {
                let yy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                m = m.max(tmp.get_pixel(x, yy)[0]);
            }
            out.put_pixel(x, y, Luma([m]));
        }
    }
    out
}

/// Otsu's method: pick the 0..255 threshold that maximises between-class variance.
fn otsu_threshold(gray: &GrayImage) -> u8 {
    let mut hist = [0u32; 256];
    for p in gray.pixels() {
        hist[p[0] as usize] += 1;
    }
    let total: u32 = gray.width() * gray.height();
    if total == 0 {
        return 128;
    }
    let sum: f64 = (0..256).map(|i| i as f64 * hist[i] as f64).sum();
    let mut sum_b = 0.0f64;
    let mut w_b = 0u32;
    let mut max_var = -1.0f64;
    let mut threshold = 128u8;
    for i in 0..256 {
        w_b += hist[i];
        if w_b == 0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0 {
            break;
        }
        sum_b += i as f64 * hist[i] as f64;
        let m_b = sum_b / w_b as f64;
        let m_f = (sum - sum_b) / w_f as f64;
        let var = w_b as f64 * w_f as f64 * (m_b - m_f) * (m_b - m_f);
        if var > max_var {
            max_var = var;
            threshold = i as u8;
        }
    }
    threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgb, RgbImage};

    /// A "document" with a left-to-right shadow gradient (dark left, light right)
    /// plus a black text bar. Simulates a phone photo of a page with a cast shadow.
    fn shadowed_page(w: u32, h: u32) -> Vec<u8> {
        let mut img = RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let t = x as f32 / (w - 1).max(1) as f32; // 0 left .. 1 right
                let bg = (110.0 + t * 130.0) as u8; // 110 (shadow) .. 240 (lit)
                img.put_pixel(x, y, Rgb([bg, bg, bg]));
            }
        }
        // A dark horizontal "text" bar across the middle.
        for x in (w / 4)..(3 * w / 4) {
            img.put_pixel(x, h / 2, Rgb([20, 20, 20]));
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn parse_mode_values() {
        assert_eq!(parse_mode("color").unwrap(), Mode::Color);
        assert_eq!(parse_mode("").unwrap(), Mode::Color);
        assert_eq!(parse_mode("grayscale").unwrap(), Mode::Grayscale);
        assert_eq!(parse_mode("grey").unwrap(), Mode::Grayscale);
        assert_eq!(parse_mode("blackwhite").unwrap(), Mode::BlackWhite);
        assert_eq!(parse_mode("bw").unwrap(), Mode::BlackWhite);
        assert!(parse_mode("sepia").is_err());
    }

    #[test]
    fn output_is_valid_png_preserving_dimensions() {
        let png = shadowed_page(64, 48);
        let out = remove_shadow(&png, Mode::Color, 55).unwrap();
        assert_eq!(&out[..4], &[0x89, b'P', b'N', b'G'], "PNG magic");
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (64, 48));
    }

    #[test]
    fn flattens_the_shadow_gradient() {
        // Input: left background much darker than right. After removal both sides
        // should be near-white and the left/right gap should shrink dramatically.
        let w = 80;
        let png = shadowed_page(w, 40);
        let inp = image::load_from_memory(&png).unwrap().to_luma8();
        let in_left = inp.get_pixel(2, 8)[0] as i32;
        let in_right = inp.get_pixel(w - 3, 8)[0] as i32;
        assert!(in_right - in_left > 80, "fixture should have a strong gradient");

        let out = remove_shadow(&png, Mode::Color, 55).unwrap();
        let og = image::load_from_memory(&out).unwrap().to_luma8();
        // Sample a text-free row.
        let out_left = og.get_pixel(2, 8)[0] as i32;
        let out_right = og.get_pixel(w - 3, 8)[0] as i32;
        assert!(out_left > 200, "shadowed side lifted to near-white, got {out_left}");
        assert!(out_right > 200, "lit side stays near-white, got {out_right}");
        assert!(
            (out_right - out_left).abs() < (in_right - in_left),
            "gradient flattened: was {} now {}",
            in_right - in_left,
            (out_right - out_left).abs()
        );
    }

    #[test]
    fn preserves_dark_text() {
        // The text bar must stay clearly darker than the surrounding page.
        let w = 80;
        let png = shadowed_page(w, 40);
        let out = remove_shadow(&png, Mode::Grayscale, 55).unwrap();
        let og = image::load_from_memory(&out).unwrap().to_luma8();
        let text = og.get_pixel(w / 2, 20)[0] as i32; // on the bar (row h/2)
        let page = og.get_pixel(w / 2, 8)[0] as i32; // above the bar
        assert!(page - text > 60, "text should stay dark vs page: text={text} page={page}");
    }

    #[test]
    fn blackwhite_is_binary() {
        let png = shadowed_page(64, 48);
        let out = remove_shadow(&png, Mode::BlackWhite, 55).unwrap();
        let og = image::load_from_memory(&out).unwrap().to_luma8();
        for p in og.pixels() {
            assert!(p[0] == 0 || p[0] == 255, "blackwhite must be 1-bit, got {}", p[0]);
        }
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(remove_shadow(b"not an image", Mode::Color, 55).is_err());
        assert!(remove_shadow(&[], Mode::Grayscale, 55).is_err());
    }
}
