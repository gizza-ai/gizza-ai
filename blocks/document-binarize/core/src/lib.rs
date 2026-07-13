//! gizza-ai/document-binarize core — binarize an image to pure black/white.
//! Pure-Rust (`image`). No wafer/wasm-bindgen deps.
//!
//! Decodes image bytes, converts to 8-bit luma, then thresholds every pixel to
//! either black (0) or white (255) using one of four methods:
//!   * `otsu`    — global threshold that maximizes between-class variance.
//!   * `fixed`   — a caller-supplied global threshold (0..=255).
//!   * `sauvola` — local adaptive threshold `T = mean*(1 + k*(std/128 - 1))`.
//!   * `niblack` — local adaptive threshold `T = mean + k*std`.
//! The local methods use a square `window` neighbourhood (odd side length) and
//! are computed in O(1) per pixel via integral images. Output is PNG.
//!
//! Convention: a pixel brighter than its threshold becomes white (background),
//! otherwise black (foreground). `invert` swaps the two.

use std::io::Cursor;

use image::{GrayImage, ImageFormat, Luma};

/// Binarization method. See module docs for the threshold each one applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Otsu,
    Fixed,
    Sauvola,
    Niblack,
}

/// Parse the `method` argument. `None` (omitted) defaults to `sauvola`.
pub fn parse_method(s: Option<&str>) -> Result<Method, String> {
    match s {
        None | Some("sauvola") => Ok(Method::Sauvola),
        Some("otsu") => Ok(Method::Otsu),
        Some("fixed") => Ok(Method::Fixed),
        Some("niblack") => Ok(Method::Niblack),
        Some(other) => Err(format!(
            "method must be one of otsu|fixed|sauvola|niblack (got {other:?})"
        )),
    }
}

/// Decode `image_bytes`, binarize to black/white, and return PNG bytes.
///
/// * `threshold` — used only by `Method::Fixed` (a luma value 0..=255).
/// * `window`    — square side length for the local methods; clamped to
///   `3..=101` and forced odd internally.
/// * `k`         — sensitivity for the local methods; must be finite and in
///   `-1.0..=1.0`.
/// * `invert`    — swap foreground/background in the output.
pub fn binarize(
    image_bytes: &[u8],
    method: Method,
    threshold: u8,
    window: u32,
    k: f64,
    invert: bool,
) -> Result<Vec<u8>, String> {
    if !k.is_finite() || !(-1.0..=1.0).contains(&k) {
        return Err("k must be a number between -1.0 and 1.0".into());
    }
    // Force an odd window in `3..=101`: even values round up, out-of-range
    // values clamp. Odd guarantees a symmetric neighbourhood around each pixel.
    let window = window.clamp(3, 101) | 1;

    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?;
    let gray = img.to_luma8();

    let out = match method {
        Method::Otsu => {
            let t = otsu_threshold(&gray) as f64;
            apply_global(&gray, t, invert)
        }
        Method::Fixed => apply_global(&gray, threshold as f64, invert),
        Method::Sauvola => apply_local(&gray, window, k, invert, LocalRule::Sauvola),
        Method::Niblack => apply_local(&gray, window, k, invert, LocalRule::Niblack),
    };

    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(out)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(buf.into_inner())
}

/// Threshold every pixel against a single global value `t`.
fn apply_global(gray: &GrayImage, t: f64, invert: bool) -> GrayImage {
    let (w, h) = gray.dimensions();
    let mut out = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let v = gray.get_pixel(x, y)[0] as f64;
            out.put_pixel(x, y, Luma([pixel(v > t, invert)]));
        }
    }
    out
}

enum LocalRule {
    Sauvola,
    Niblack,
}

/// Local adaptive threshold using integral images for O(1) window statistics.
fn apply_local(gray: &GrayImage, window: u32, k: f64, invert: bool, rule: LocalRule) -> GrayImage {
    let (w, h) = gray.dimensions();
    let iw = (w + 1) as usize;

    // Integral image of values and of squared values (f64 to avoid overflow on
    // large images: max sum of squares is w*h*255^2).
    let mut sum = vec![0f64; iw * (h as usize + 1)];
    let mut sqsum = vec![0f64; iw * (h as usize + 1)];
    for y in 0..h as usize {
        let mut row = 0f64;
        let mut row_sq = 0f64;
        for x in 0..w as usize {
            let v = gray.get_pixel(x as u32, y as u32)[0] as f64;
            row += v;
            row_sq += v * v;
            let idx = (y + 1) * iw + (x + 1);
            sum[idx] = sum[idx - iw] + row;
            sqsum[idx] = sqsum[idx - iw] + row_sq;
        }
    }

    let rect = |ix: &[f64], x0: u32, y0: u32, x1: u32, y1: u32| -> f64 {
        let (x0, y0, x1, y1) = (x0 as usize, y0 as usize, x1 as usize + 1, y1 as usize + 1);
        ix[y1 * iw + x1] - ix[y0 * iw + x1] - ix[y1 * iw + x0] + ix[y0 * iw + x0]
    };

    let half = window / 2;
    let mut out = GrayImage::new(w, h);
    for y in 0..h {
        let y0 = y.saturating_sub(half);
        let y1 = (y + half).min(h - 1);
        for x in 0..w {
            let x0 = x.saturating_sub(half);
            let x1 = (x + half).min(w - 1);
            let area = ((x1 - x0 + 1) * (y1 - y0 + 1)) as f64;
            let s = rect(&sum, x0, y0, x1, y1);
            let sq = rect(&sqsum, x0, y0, x1, y1);
            let mean = s / area;
            // Guard tiny negatives from floating-point cancellation.
            let var = (sq / area - mean * mean).max(0.0);
            let std = var.sqrt();
            let t = match rule {
                LocalRule::Sauvola => mean * (1.0 + k * (std / 128.0 - 1.0)),
                LocalRule::Niblack => mean + k * std,
            };
            let v = gray.get_pixel(x, y)[0] as f64;
            out.put_pixel(x, y, Luma([pixel(v > t, invert)]));
        }
    }
    out
}

/// Map "is background" → output luma, honouring `invert`.
#[inline]
fn pixel(is_white: bool, invert: bool) -> u8 {
    if is_white ^ invert {
        255
    } else {
        0
    }
}

/// Otsu's method: the global threshold maximizing between-class variance.
fn otsu_threshold(gray: &GrayImage) -> u8 {
    let mut hist = [0u32; 256];
    for p in gray.pixels() {
        hist[p[0] as usize] += 1;
    }
    let total: u32 = gray.width() * gray.height();
    if total == 0 {
        return 0;
    }
    let sum_total: f64 = (0..256).map(|i| i as f64 * hist[i] as f64).sum();

    let mut sum_b = 0f64;
    let mut w_b = 0u32;
    let mut max_var = -1f64;
    let mut thresh = 0u8;
    for t in 0..256 {
        w_b += hist[t];
        if w_b == 0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0 {
            break;
        }
        sum_b += t as f64 * hist[t] as f64;
        let m_b = sum_b / w_b as f64;
        let m_f = (sum_total - sum_b) / w_f as f64;
        let between = w_b as f64 * w_f as f64 * (m_b - m_f) * (m_b - m_f);
        if between > max_var {
            max_var = between;
            thresh = t as u8;
        }
    }
    thresh
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    /// A 32x16 image: left half dark (luma 50), right half light (luma 200).
    /// A hard bimodal split so global thresholds land cleanly between the peaks.
    fn split_png() -> Vec<u8> {
        let mut img = GrayImage::new(32, 16);
        for y in 0..16 {
            for x in 0..32 {
                let v = if x < 16 { 50u8 } else { 200u8 };
                img.put_pixel(x, y, Luma([v]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageLuma8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    /// Assert the PNG magic and that every pixel is pure black or pure white.
    fn assert_binary(png: &[u8]) -> image::DynamicImage {
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let out = image::load_from_memory(png).unwrap();
        for (_, _, p) in out.pixels() {
            let v = p.0[0];
            assert!(v == 0 || v == 255, "non-binary pixel {v}");
        }
        out
    }

    #[test]
    fn fixed_thresholds_dark_to_black_light_to_white() {
        let out = assert_binary(&binarize(&split_png(), Method::Fixed, 128, 25, 0.34, false).unwrap());
        assert_eq!(out.dimensions(), (32, 16));
        // Dark left half (50 < 128) → black; light right half (200 > 128) → white.
        assert_eq!(out.get_pixel(0, 0).0[0], 0);
        assert_eq!(out.get_pixel(31, 0).0[0], 255);
    }

    #[test]
    fn invert_swaps_black_and_white() {
        let normal = image::load_from_memory(
            &binarize(&split_png(), Method::Fixed, 128, 25, 0.34, false).unwrap(),
        )
        .unwrap();
        let inverted = image::load_from_memory(
            &binarize(&split_png(), Method::Fixed, 128, 25, 0.34, true).unwrap(),
        )
        .unwrap();
        assert_eq!(normal.get_pixel(0, 0).0[0], 0);
        assert_eq!(inverted.get_pixel(0, 0).0[0], 255);
        assert_eq!(normal.get_pixel(31, 0).0[0], 255);
        assert_eq!(inverted.get_pixel(31, 0).0[0], 0);
    }

    #[test]
    fn otsu_separates_the_two_modes() {
        let out = assert_binary(&binarize(&split_png(), Method::Otsu, 0, 25, 0.34, false).unwrap());
        // Otsu should place its threshold between 50 and 200.
        assert_eq!(out.get_pixel(0, 0).0[0], 0);
        assert_eq!(out.get_pixel(31, 0).0[0], 255);
    }

    #[test]
    fn sauvola_and_niblack_produce_binary_output() {
        for method in [Method::Sauvola, Method::Niblack] {
            let out = assert_binary(&binarize(&split_png(), method, 128, 25, 0.34, false).unwrap());
            assert_eq!(out.dimensions(), (32, 16));
        }
    }

    #[test]
    fn even_window_is_accepted_and_forced_odd() {
        // window=24 (even, in range) must not error — it is forced to odd.
        assert!(binarize(&split_png(), Method::Sauvola, 128, 24, 0.34, false).is_ok());
        // out-of-range window clamps rather than errors.
        assert!(binarize(&split_png(), Method::Niblack, 128, 999, 0.2, false).is_ok());
    }

    #[test]
    fn parse_method_maps_names_and_default() {
        assert_eq!(parse_method(None).unwrap(), Method::Sauvola);
        assert_eq!(parse_method(Some("otsu")).unwrap(), Method::Otsu);
        assert_eq!(parse_method(Some("fixed")).unwrap(), Method::Fixed);
        assert_eq!(parse_method(Some("sauvola")).unwrap(), Method::Sauvola);
        assert_eq!(parse_method(Some("niblack")).unwrap(), Method::Niblack);
        assert!(parse_method(Some("threshold")).is_err());
    }

    #[test]
    fn invalid_input_errors() {
        // Undecodable bytes.
        assert!(binarize(b"not an image", Method::Otsu, 128, 25, 0.34, false).is_err());
        // Out-of-range / non-finite k.
        assert!(binarize(&split_png(), Method::Sauvola, 128, 25, 2.0, false).is_err());
        assert!(binarize(&split_png(), Method::Sauvola, 128, 25, -5.0, false).is_err());
        assert!(binarize(&split_png(), Method::Sauvola, 128, 25, f64::NAN, false).is_err());
    }
}
