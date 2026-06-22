//! gizza-ai/blur-image core — apply a Gaussian blur of adjustable radius to an
//! image. Pure-Rust (`image`). No wafer/wasm-bindgen deps.
//!
//! Wraps `image::imageops::blur`, which performs a true Gaussian blur where the
//! `radius` argument is the Gaussian standard deviation (sigma) in pixels:
//! larger values spread each pixel over a wider neighbourhood, producing a
//! softer result. Output is PNG.

use std::io::Cursor;

use image::ImageFormat;

/// Blur `image_bytes` with a Gaussian of standard deviation `radius` (pixels)
/// and return PNG bytes. `radius` must be a finite, positive number.
pub fn blur(image_bytes: &[u8], radius: f64) -> Result<Vec<u8>, String> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err("radius must be a positive number".into());
    }
    // Cap to keep blur of large images bounded; sigma above this is already a
    // near-flat wash and only burns CPU.
    let radius = radius.min(200.0) as f32;

    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?;
    let blurred = image::imageops::blur(&img, radius);

    let mut out = Cursor::new(Vec::new());
    blurred
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgba, RgbaImage};

    /// A 16x16 hard black/white vertical split so a blur visibly smooths the
    /// boundary into intermediate grey values.
    fn split_png() -> Vec<u8> {
        let mut img = RgbaImage::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let v = if x < 8 { 0u8 } else { 255u8 };
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
        let png = blur(&split_png(), 2.0).unwrap();
        let out = image::load_from_memory(&png).unwrap();
        assert_eq!(out.dimensions(), (16, 16));
        // PNG magic bytes.
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[test]
    fn blur_smooths_the_boundary() {
        let src = split_png();
        let png = blur(&src, 3.0).unwrap();
        let out = image::load_from_memory(&png).unwrap();
        // After blurring the black/white split, pixels straddling the boundary
        // should take on intermediate (grey) values, neither 0 nor 255.
        let mut intermediate = 0u32;
        for y in 0..16 {
            for x in 0..16 {
                let p = out.get_pixel(x, y);
                let r = p.0[0];
                if r > 10 && r < 245 {
                    intermediate += 1;
                }
            }
        }
        assert!(
            intermediate > 0,
            "blurring a hard edge should create intermediate grey pixels"
        );
    }

    #[test]
    fn stronger_radius_blurs_more() {
        let src = split_png();
        // Count intermediate-grey pixels at a small vs large radius; a larger
        // sigma spreads the transition wider, so it should produce at least as
        // many of them.
        let count_grey = |png: &[u8]| -> u32 {
            let out = image::load_from_memory(png).unwrap();
            let mut n = 0u32;
            for y in 0..16 {
                for x in 0..16 {
                    let r = out.get_pixel(x, y).0[0];
                    if r > 10 && r < 245 {
                        n += 1;
                    }
                }
            }
            n
        };
        let small = count_grey(&blur(&src, 1.0).unwrap());
        let large = count_grey(&blur(&src, 5.0).unwrap());
        assert!(large >= small, "larger radius should blur at least as wide");
    }

    #[test]
    fn errors() {
        assert!(blur(b"not an image", 2.0).is_err());
        assert!(blur(&split_png(), 0.0).is_err());
        assert!(blur(&split_png(), -1.0).is_err());
        assert!(blur(&split_png(), f64::NAN).is_err());
    }
}
