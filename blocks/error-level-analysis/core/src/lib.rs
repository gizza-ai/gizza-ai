//! error-level-analysis core — pure-Rust Error Level Analysis (ELA).
//!
//! ELA is a classic image-forensics technique. A JPEG accumulates a fixed amount
//! of compression error each time it is saved. If you take a JPEG, re-save it at a
//! known quality, and subtract the result from the original, every region that has
//! been compressed the same number of times shows a similar (low) error level —
//! the whole image goes roughly uniformly dark. A region that was *added* or
//! *edited* (e.g. pasted in from another image, or painted over) has a different
//! compression history, so it differs more from its recompression and shows up
//! **brighter** in the difference map. Amplifying that difference makes the seams
//! of edits, splices, and text overlays visible to the eye.
//!
//! Pipeline: decode the input → re-encode it as JPEG at `quality` → decode that →
//! take the absolute per-channel difference between the two RGB images → multiply
//! by `scale` (saturating) → emit a lossless PNG so the amplified map is exact.
//! Optionally normalise so the brightest pixel maps to 255 for maximum contrast.
//!
//! Pure (no wafer/wasm deps): unit-testable on the host, reused by the chat skill
//! block. Image-bytes output → chat + CLI only, no page (same as lsb-embed /
//! add-text-to-image).

use image::codecs::jpeg::JpegEncoder;
use image::{ImageFormat, RgbImage};
use std::io::Cursor;

/// Run Error Level Analysis on `image_bytes`.
///
/// * `quality` — the JPEG recompression quality, 1..=100. The standard ELA value
///   is around 90–95: high enough that genuinely-unedited regions barely change,
///   so only differing-history regions stand out.
/// * `scale` — brightness multiplier applied to the raw difference, 1..=100.
///   Raw ELA differences are tiny (often 0–5 per channel); scaling makes them
///   visible. ~10–20 is typical.
/// * `normalize` — if true, the brightest difference pixel is stretched to 255
///   (auto-contrast) instead of using `scale`. Useful when the right scale is
///   unknown.
/// * `grayscale` — if true, collapse the difference map to luminance (often
///   easier to read than the per-channel colour map).
///
/// Returns a lossless PNG visualisation. Bright/colourful areas indicate a
/// different compression history (a likely edit); near-black areas are consistent
/// with the rest of the image.
pub fn analyze(
    image_bytes: &[u8],
    quality: u8,
    scale: u8,
    normalize: bool,
    grayscale: bool,
) -> Result<Vec<u8>, String> {
    let quality = validate_range(quality, 1, 100, "quality")?;
    let scale = validate_range(scale, 1, 100, "scale")?;

    let original: RgbImage = image::load_from_memory(image_bytes)
        .map_err(|e| format!("decode image: {e}"))?
        .to_rgb8();
    let (w, h) = original.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimensions".into());
    }

    // Re-encode as JPEG at the chosen quality, then decode it back.
    let mut buf = Cursor::new(Vec::new());
    {
        let mut enc = JpegEncoder::new_with_quality(&mut buf, quality);
        enc.encode(original.as_raw(), w, h, image::ExtendedColorType::Rgb8)
            .map_err(|e| format!("re-encode JPEG: {e}"))?;
    }
    let recompressed: RgbImage = image::load_from_memory(buf.get_ref())
        .map_err(|e| format!("decode recompressed JPEG: {e}"))?
        .to_rgb8();

    // Per-channel absolute difference.
    let a = original.as_raw();
    let b = recompressed.as_raw();
    let n = a.len().min(b.len());
    let mut diff = vec![0u8; n];
    let mut max_diff = 0u8;
    for i in 0..n {
        let d = (a[i] as i16 - b[i] as i16).unsigned_abs() as u8;
        diff[i] = d;
        if d > max_diff {
            max_diff = d;
        }
    }

    // Amplify: either auto-normalise to the brightest pixel, or fixed scale.
    if normalize && max_diff > 0 {
        let factor = 255.0 / max_diff as f32;
        for v in diff.iter_mut() {
            *v = (*v as f32 * factor).round().min(255.0) as u8;
        }
    } else {
        for v in diff.iter_mut() {
            *v = (*v as u16 * scale as u16).min(255) as u8;
        }
    }

    if grayscale {
        // BT.601 luma per pixel, written back into all three channels so the PNG
        // stays RGB (uniform output shape regardless of mode).
        for px in diff.chunks_exact_mut(3) {
            let l = (0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32)
                .round()
                .min(255.0) as u8;
            px[0] = l;
            px[1] = l;
            px[2] = l;
        }
    }

    let out_img = RgbImage::from_raw(w, h, diff)
        .ok_or_else(|| "internal: difference buffer size mismatch".to_string())?;
    let mut out = Cursor::new(Vec::new());
    out_img
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("encode PNG: {e}"))?;
    Ok(out.into_inner())
}

fn validate_range(v: u8, lo: u8, hi: u8, name: &str) -> Result<u8, String> {
    if (lo..=hi).contains(&v) {
        Ok(v)
    } else {
        Err(format!("{name} must be {lo}..={hi}, got {v}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    /// A smooth gradient image saved as JPEG — a realistic "single-history" input.
    fn jpeg_gradient(w: u32, h: u32) -> Vec<u8> {
        let mut img = RgbImage::new(w, h);
        for (x, y, px) in img.enumerate_pixels_mut() {
            *px = Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8]);
        }
        let mut buf = Cursor::new(Vec::new());
        JpegEncoder::new_with_quality(&mut buf, 92)
            .encode(img.as_raw(), w, h, image::ExtendedColorType::Rgb8)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn output_is_valid_png_same_size() {
        let src = jpeg_gradient(64, 48);
        let out = analyze(&src, 90, 15, false, false).unwrap();
        assert_eq!(image::guess_format(&out).unwrap(), ImageFormat::Png);
        let decoded = image::load_from_memory(&out).unwrap();
        assert_eq!(decoded.width(), 64);
        assert_eq!(decoded.height(), 48);
    }

    #[test]
    fn edited_region_shows_higher_error_than_clean_region() {
        // Build a JPEG, decode it (so it has one compression history), then splice
        // a sharp synthetic block into it (no JPEG history) and re-save as JPEG.
        // The spliced block should have a higher mean ELA value than the rest.
        let base = jpeg_gradient(96, 96);
        let mut img = image::load_from_memory(&base).unwrap().to_rgb8();
        for y in 10..40 {
            for x in 10..40 {
                // hard-edged high-frequency checkerboard = fresh, un-compressed content
                let on = (x + y) % 2 == 0;
                img.put_pixel(x, y, if on { Rgb([255, 0, 255]) } else { Rgb([0, 255, 0]) });
            }
        }
        let mut spliced = Cursor::new(Vec::new());
        JpegEncoder::new_with_quality(&mut spliced, 95)
            .encode(img.as_raw(), 96, 96, image::ExtendedColorType::Rgb8)
            .unwrap();

        let ela = analyze(spliced.get_ref(), 90, 10, false, false).unwrap();
        let map = image::load_from_memory(&ela).unwrap().to_rgb8();

        let mean = |x0: u32, y0: u32, x1: u32, y1: u32| -> f64 {
            let mut sum = 0u64;
            let mut cnt = 0u64;
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = map.get_pixel(x, y);
                    sum += p[0] as u64 + p[1] as u64 + p[2] as u64;
                    cnt += 3;
                }
            }
            sum as f64 / cnt as f64
        };
        let edited = mean(12, 12, 38, 38);
        let clean = mean(60, 60, 90, 90);
        assert!(
            edited > clean * 1.5,
            "edited region ELA ({edited:.1}) should exceed clean ({clean:.1})"
        );
    }

    #[test]
    fn normalize_produces_a_255_peak() {
        let src = jpeg_gradient(64, 64);
        let out = analyze(&src, 80, 1, true, false).unwrap();
        let map = image::load_from_memory(&out).unwrap().to_rgb8();
        let peak = map.as_raw().iter().copied().max().unwrap();
        assert_eq!(peak, 255, "normalize should stretch the brightest pixel to 255");
    }

    #[test]
    fn grayscale_output_has_equal_channels() {
        let src = jpeg_gradient(48, 48);
        let out = analyze(&src, 90, 20, false, true).unwrap();
        let map = image::load_from_memory(&out).unwrap().to_rgb8();
        for px in map.pixels() {
            assert_eq!(px[0], px[1]);
            assert_eq!(px[1], px[2]);
        }
    }

    #[test]
    fn invalid_quality_errors() {
        let src = jpeg_gradient(16, 16);
        assert!(analyze(&src, 0, 10, false, false).is_err());
        assert!(analyze(&src, 101, 10, false, false).is_err());
    }

    #[test]
    fn invalid_scale_errors() {
        let src = jpeg_gradient(16, 16);
        assert!(analyze(&src, 90, 0, false, false).is_err());
        assert!(analyze(&src, 90, 101, false, false).is_err());
    }

    #[test]
    fn non_image_bytes_error() {
        assert!(analyze(b"not an image", 90, 10, false, false).is_err());
    }
}
