//! gizza-ai/image-color-quantize core — reduce an image to at most N colors using
//! NeuQuant color quantization (an optimal palette derived from the image).
//! Pure-Rust (`image` + `color_quant`). Returns PNG bytes.

use std::io::Cursor;

use color_quant::NeuQuant;
use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

/// Quantize `bytes` to at most `colors` (2..=256) colors. Returns PNG bytes.
pub fn quantize(bytes: &[u8], colors: usize) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let n = colors.clamp(2, 256);
    let rgba = img.to_rgba8();

    // Train the palette on the image. samplefac 10 is a good speed/quality balance.
    let nq = NeuQuant::new(10, n, rgba.as_raw());
    let palette = nq.color_map_rgba(); // n*4 bytes

    let mut out_img: RgbaImage = RgbaImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let idx = nq.index_of(&p.0);
        let o = idx * 4;
        out_img.put_pixel(
            x,
            y,
            Rgba([palette[o], palette[o + 1], palette[o + 2], palette[o + 3]]),
        );
    }

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(out_img)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
fn distinct_colors(png: &[u8]) -> usize {
    let img = image::load_from_memory(png).unwrap().to_rgba8();
    let mut set = std::collections::HashSet::new();
    for p in img.pixels() {
        set.insert(p.0);
    }
    set.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16x16 image with many distinct colors (a gradient).
    fn gradient() -> Vec<u8> {
        let mut img = RgbaImage::new(16, 16);
        for y in 0..16u32 {
            for x in 0..16u32 {
                img.put_pixel(x, y, Rgba([(x * 16) as u8, (y * 16) as u8, ((x + y) * 8) as u8, 255]));
            }
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn reduces_palette_size() {
        let src = gradient();
        let before = distinct_colors(&src);
        let out = quantize(&src, 8).unwrap();
        let after = distinct_colors(&out);
        assert!(after <= 8, "expected <= 8 colors, got {after}");
        assert!(after < before, "should reduce colors ({before} -> {after})");
    }

    #[test]
    fn dimensions_preserved() {
        let out = quantize(&gradient(), 16).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (16, 16));
    }

    #[test]
    fn solid_image_stays_solid() {
        let img = RgbaImage::from_pixel(8, 8, Rgba([10, 120, 200, 255]));
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut buf, ImageFormat::Png).unwrap();
        let out = quantize(&buf.into_inner(), 4).unwrap();
        assert_eq!(distinct_colors(&out), 1);
    }

    #[test]
    fn errors() {
        assert!(quantize(b"not an image", 16).is_err());
    }
}
