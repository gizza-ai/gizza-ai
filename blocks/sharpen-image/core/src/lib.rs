//! gizza-ai/sharpen-image core — sharpen an image with an adjustable unsharp
//! mask. Pure-Rust (`image`). No wafer/wasm-bindgen deps.
//!
//! Wraps `image::imageops::unsharpen`: blur the image by `amount` (Gaussian
//! sigma) and add back the difference, sharpening edges. `threshold` is the
//! minimum brightness difference a pixel must have from its blurred neighbours
//! before it is sharpened (suppresses noise on flat areas). Output is PNG.

use std::io::Cursor;

use image::ImageFormat;

/// Sharpen `image_bytes` and return PNG bytes.
/// `amount` is the unsharp sigma (>0; higher = stronger). `threshold` is 0-255.
pub fn sharpen(image_bytes: &[u8], amount: f64, threshold: i32) -> Result<Vec<u8>, String> {
    if !(amount.is_finite()) || amount <= 0.0 {
        return Err("amount must be a positive number".into());
    }
    let amount = amount.min(50.0) as f32;
    let threshold = threshold.clamp(0, 255);

    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?;
    let sharpened = image::imageops::unsharpen(&img, amount, threshold);

    let mut out = Cursor::new(Vec::new());
    sharpened
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgba, RgbaImage};

    /// A 16x16 mid-tone checkerboard (values 100/156) so unsharp overshoot is
    /// visible rather than clamped at 0/255 as it would be on a saturated edge.
    fn edge_png() -> Vec<u8> {
        let mut img = RgbaImage::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let v = if (x / 2 + y / 2) % 2 == 0 { 100u8 } else { 156u8 };
                img.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn output_is_png_same_size() {
        let png = sharpen(&edge_png(), 2.0, 0).unwrap();
        let out = image::load_from_memory(&png).unwrap();
        assert_eq!(out.dimensions(), (16, 16));
        // PNG magic bytes.
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[test]
    fn sharpening_changes_pixels_near_edge() {
        let src = edge_png();
        let png = sharpen(&src, 3.0, 0).unwrap();
        let orig = image::load_from_memory(&src).unwrap();
        let out = image::load_from_memory(&png).unwrap();
        // Unsharp masking overshoots near a hard edge, so at least some pixels differ.
        let mut diff = 0u32;
        for y in 0..16 {
            for x in 0..16 {
                if orig.get_pixel(x, y) != out.get_pixel(x, y) {
                    diff += 1;
                }
            }
        }
        assert!(diff > 0, "sharpening should alter pixels near the edge");
    }

    #[test]
    fn errors() {
        assert!(sharpen(b"not an image", 2.0, 0).is_err());
        assert!(sharpen(&edge_png(), 0.0, 0).is_err());
        assert!(sharpen(&edge_png(), -1.0, 0).is_err());
    }
}
