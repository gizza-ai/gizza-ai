//! gizza-ai/image-to-pixel-art core — turn a photo into limited-palette pixel art.
//!
//! The classic pixel-art look = two steps:
//!   1. Downscale the image to a small grid (each output "pixel" is a block of the
//!      source) using a box/triangle filter so each block is an averaged colour.
//!   2. Reduce that small image to at most N colours with NeuQuant (a palette
//!      derived from the image itself), then upscale back to (near) the original
//!      size with nearest-neighbour so every grid cell becomes a solid chunky block.
//!
//! Pure-Rust (`image` + `color_quant`) so it runs on every backend incl. the chat SW.
//! Returns PNG bytes.

use std::io::Cursor;

use color_quant::NeuQuant;
use image::{
    imageops::FilterType, DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage,
};

/// Convert `bytes` to pixel art.
///
/// * `pixel_size` — the source block size in pixels (2..=64). Larger = chunkier,
///   fewer output cells. The image is downscaled so each `pixel_size`x`pixel_size`
///   source block becomes one art pixel, then upscaled back to roughly the original
///   dimensions so the blocks stay crisp.
/// * `colors` — palette size, 2..=256. The palette is derived from the image.
///
/// Returns PNG bytes at (approximately) the original dimensions.
pub fn pixelate(bytes: &[u8], pixel_size: u32, colors: usize) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }

    let block = pixel_size.clamp(2, 64);
    let n = colors.clamp(2, 256);

    // Grid dimensions: how many art pixels across/down (at least 1 each).
    let gw = (w / block).max(1);
    let gh = (h / block).max(1);

    // 1. Downscale to the coarse grid. Triangle filter averages the source blocks
    //    (a clean, anti-aliased shrink) before we quantize.
    let small = img.resize_exact(gw, gh, FilterType::Triangle).to_rgba8();

    // 2. Quantize the small image to <= n colours with an image-derived palette.
    let nq = NeuQuant::new(10, n, small.as_raw());
    let palette = nq.color_map_rgba(); // n*4 bytes
    let mut quantized: RgbaImage = RgbaImage::new(gw, gh);
    for (x, y, p) in small.enumerate_pixels() {
        let idx = nq.index_of(&p.0);
        let o = idx * 4;
        quantized.put_pixel(
            x,
            y,
            Rgba([palette[o], palette[o + 1], palette[o + 2], palette[o + 3]]),
        );
    }

    // 3. Upscale back to ~original size with nearest-neighbour so each grid cell is
    //    a solid block. Use an exact multiple of the grid so blocks are uniform.
    let out_w = gw * block;
    let out_h = gh * block;
    let big = DynamicImage::ImageRgba8(quantized).resize_exact(out_w, out_h, FilterType::Nearest);

    let mut out = Cursor::new(Vec::new());
    big.write_to(&mut out, ImageFormat::Png)
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

    /// A 64x64 image with many distinct colours (a smooth gradient).
    fn gradient() -> Vec<u8> {
        let mut img = RgbaImage::new(64, 64);
        for y in 0..64u32 {
            for x in 0..64u32 {
                img.put_pixel(
                    x,
                    y,
                    Rgba([(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8, 255]),
                );
            }
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn reduces_palette() {
        let src = gradient();
        let before = distinct_colors(&src);
        let out = pixelate(&src, 8, 8).unwrap();
        let after = distinct_colors(&out);
        assert!(after <= 8, "expected <= 8 colours, got {after}");
        assert!(after < before, "should reduce colours ({before} -> {after})");
    }

    #[test]
    fn produces_blocky_grid() {
        // pixel_size 8 on a 64x64 image -> an 8x8 grid -> output is exactly
        // 8*8 = 64 wide, and each 8x8 block is one solid colour.
        let out = pixelate(&gradient(), 8, 16).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (64, 64));
        // Every 8x8 block must be a single colour (nearest-neighbour upscale).
        for by in (0..64).step_by(8) {
            for bx in (0..64).step_by(8) {
                let first = *img.get_pixel(bx, by);
                for dy in 0..8 {
                    for dx in 0..8 {
                        assert_eq!(
                            *img.get_pixel(bx + dx, by + dy),
                            first,
                            "block at ({bx},{by}) is not uniform"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn solid_image_stays_solid() {
        let img = RgbaImage::from_pixel(32, 32, Rgba([10, 120, 200, 255]));
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let out = pixelate(&buf.into_inner(), 8, 4).unwrap();
        assert_eq!(distinct_colors(&out), 1);
    }

    #[test]
    fn clamps_out_of_range() {
        // pixel_size 0 -> clamped to 2; colors 9999 -> clamped to 256. Should not panic.
        assert!(pixelate(&gradient(), 0, 9999).is_ok());
        assert!(pixelate(&gradient(), 1000, 1).is_ok());
    }

    #[test]
    fn errors_on_bad_input() {
        assert!(pixelate(b"not an image", 8, 16).is_err());
    }
}
