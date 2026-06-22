//! gizza-ai/normalize-image core — auto-normalize (contrast-stretch) an image by
//! mapping each color channel's used range to the full 0–255 dynamic range. No
//! wafer/wasm-bindgen deps. Pure-Rust `image`. An optional `clip_percent` ignores
//! that fraction of the darkest/lightest pixels per channel before stretching, so
//! a few outliers don't flatten the result (ImageMagick `-normalize` style).

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

/// Find the (lo, hi) channel values bounding the central `100 - 2*clip%` of a
/// 256-bin histogram. Returns (0, 255) when the channel is effectively flat.
fn bounds(hist: &[u32; 256], total: u32, clip_percent: f64) -> (u8, u8) {
    let clip = ((total as f64) * clip_percent / 100.0).round() as u32;
    let mut lo = 0u8;
    let mut acc = 0u32;
    for (v, &count) in hist.iter().enumerate() {
        acc += count;
        if acc > clip {
            lo = v as u8;
            break;
        }
    }
    let mut hi = 255u8;
    acc = 0;
    for v in (0..256).rev() {
        acc += hist[v];
        if acc > clip {
            hi = v as u8;
            break;
        }
    }
    if hi <= lo {
        (0, 255)
    } else {
        (lo, hi)
    }
}

/// Build a 0..=255 lookup table that stretches [lo, hi] to [0, 255].
fn lut(lo: u8, hi: u8) -> [u8; 256] {
    let mut t = [0u8; 256];
    let span = (hi - lo) as f64;
    for (v, out) in t.iter_mut().enumerate() {
        *out = if span <= 0.0 {
            v as u8
        } else {
            (((v as f64 - lo as f64) * 255.0 / span).round()).clamp(0.0, 255.0) as u8
        };
    }
    t
}

/// Normalize `bytes`. `clip_percent` (0–45) trims that % of extreme pixels per
/// channel before stretching. Returns PNG bytes. Alpha is preserved.
pub fn normalize(bytes: &[u8], clip_percent: f64) -> Result<Vec<u8>, String> {
    let clip = clip_percent.clamp(0.0, 45.0);
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let mut buf: RgbaImage = img.to_rgba8();
    let total = w * h;

    // Per-channel histograms.
    let mut hist = [[0u32; 256]; 3];
    for p in buf.pixels() {
        for c in 0..3 {
            hist[c][p.0[c] as usize] += 1;
        }
    }
    let luts: Vec<[u8; 256]> = (0..3)
        .map(|c| {
            let (lo, hi) = bounds(&hist[c], total, clip);
            lut(lo, hi)
        })
        .collect();

    for p in buf.pixels_mut() {
        let a = p.0[3];
        *p = Rgba([luts[0][p.0[0] as usize], luts[1][p.0[1] as usize], luts[2][p.0[2] as usize], a]);
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

    fn gray_row(vals: &[u8]) -> Vec<u8> {
        let mut img = RgbaImage::new(vals.len() as u32, 1);
        for (x, &v) in vals.iter().enumerate() {
            img.put_pixel(x as u32, 0, Rgba([v, v, v, 255]));
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn stretches_low_contrast_range_to_full() {
        // values 100..150 → after auto-level: 100->0, 150->255.
        let png = gray_row(&[100, 120, 140, 150]);
        let out = normalize(&png, 0.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0).0[0], 0, "min stretches to 0");
        assert_eq!(img.get_pixel(3, 0).0[0], 255, "max stretches to 255");
        // 120 → (20/50)*255 ≈ 102
        assert_eq!(img.get_pixel(1, 0).0[0], 102);
    }

    #[test]
    fn already_full_range_is_unchanged() {
        let png = gray_row(&[0, 128, 255]);
        let out = normalize(&png, 0.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0).0[0], 0);
        assert_eq!(img.get_pixel(1, 0).0[0], 128);
        assert_eq!(img.get_pixel(2, 0).0[0], 255);
    }

    #[test]
    fn flat_image_does_not_panic_or_divide_by_zero() {
        let png = gray_row(&[50, 50, 50, 50]);
        let out = normalize(&png, 0.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        // Flat channel → identity map, stays 50.
        assert_eq!(img.get_pixel(0, 0).0[0], 50);
    }

    #[test]
    fn clip_ignores_outliers() {
        // A bulk spanning 120..140 plus a black + white outlier. With a ~3% clip
        // the outliers are ignored and the bulk's [120,140] stretches to [0,255].
        let mut vals = vec![0u8; 100];
        vals[0] = 0; // black outlier
        vals[99] = 255; // white outlier
        for v in vals.iter_mut().take(50).skip(1) {
            *v = 120;
        }
        for v in vals.iter_mut().take(99).skip(50) {
            *v = 140;
        }
        let png = gray_row(&vals);
        let out = normalize(&png, 3.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        // After clipping the outliers, lo=120 → 0 and hi=140 → 255.
        assert_eq!(img.get_pixel(1, 0).0[0], 0, "bulk min 120 stretches to 0");
        assert_eq!(img.get_pixel(98, 0).0[0], 255, "bulk max 140 stretches to 255");
    }

    #[test]
    fn alpha_preserved() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([100, 100, 100, 40]));
        img.put_pixel(1, 0, Rgba([150, 150, 150, 200]));
        let mut enc = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut enc, ImageFormat::Png).unwrap();
        let out = normalize(&enc.into_inner(), 0.0).unwrap();
        let res = image::load_from_memory(&out).unwrap();
        assert_eq!(res.get_pixel(0, 0).0[3], 40);
        assert_eq!(res.get_pixel(1, 0).0[3], 200);
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(normalize(b"not an image", 0.0).is_err());
    }
}
