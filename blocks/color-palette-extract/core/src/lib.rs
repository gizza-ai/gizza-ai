//! gizza-ai/color-palette-extract core — extract the dominant color palette from
//! an image as hex codes. No wafer/wasm-bindgen deps. Pure-Rust (`image` +
//! `color_quant`). Shared by the chat skill block, the CLI, and the unit tests.
//!
//! Approach: decode → NeuQuant derives an optimal `colors`-entry palette from the
//! image itself → re-map every (alpha-aware) pixel to its nearest palette index →
//! count pixels per palette entry → sort by descending pixel share so the most
//! *dominant* colors come first. Output is per-color hex + rgb + share-of-image.

use color_quant::NeuQuant;
use image::GenericImageView;

/// One entry of the extracted palette.
#[derive(Debug, Clone, PartialEq)]
pub struct Swatch {
    /// `#rrggbb` (lowercase).
    pub hex: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Number of (opaque) pixels mapped to this color.
    pub count: u64,
    /// Fraction of counted pixels (0.0..=1.0), rounded to 4 decimals.
    pub fraction: f64,
}

/// The full extraction result.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub width: u32,
    pub height: u32,
    pub colors: Vec<Swatch>,
}

const MIN_COLORS: usize = 1;
const MAX_COLORS: usize = 64;
/// Treat pixels with alpha below this as transparent and exclude them from the
/// palette (a transparent background shouldn't count as a "dominant color").
const ALPHA_THRESHOLD: u8 = 16;

fn to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Extract up to `colors` (clamped to 1..=64) dominant colors from the image
/// `bytes`, ordered most-dominant first. `colors` is the *requested* palette
/// size; fewer may be returned for images with few distinct colors.
pub fn extract(bytes: &[u8], colors: usize) -> Result<Palette, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let n = colors.clamp(MIN_COLORS, MAX_COLORS);
    let rgba = img.to_rgba8();

    // NeuQuant needs at least 2 colors to train; for a requested palette of 1 we
    // still train on 2 and then keep the single most-dominant swatch.
    let train_n = n.max(2);
    let nq = NeuQuant::new(10, train_n, rgba.as_raw());
    let pal = nq.color_map_rgba(); // train_n * 4 bytes (r,g,b,a)

    // Tally opaque pixels per palette index.
    let mut counts = vec![0u64; train_n];
    let mut total: u64 = 0;
    for px in rgba.pixels() {
        if px.0[3] < ALPHA_THRESHOLD {
            continue; // skip (near-)transparent pixels
        }
        let idx = nq.index_of(&px.0);
        if idx < counts.len() {
            counts[idx] += 1;
            total += 1;
        }
    }

    // If the image was fully transparent, fall back to counting every pixel so we
    // still return *something* meaningful rather than an empty palette.
    if total == 0 {
        for px in rgba.pixels() {
            let idx = nq.index_of(&px.0);
            if idx < counts.len() {
                counts[idx] += 1;
                total += 1;
            }
        }
    }
    let total = total.max(1);

    // Build swatches for palette entries that actually appear, dedup'd by RGB
    // (NeuQuant can emit near-identical entries; merge them by exact RGB).
    use std::collections::BTreeMap;
    let mut by_rgb: BTreeMap<(u8, u8, u8), u64> = BTreeMap::new();
    for (i, &c) in counts.iter().enumerate() {
        if c == 0 {
            continue;
        }
        let o = i * 4;
        let key = (pal[o], pal[o + 1], pal[o + 2]);
        *by_rgb.entry(key).or_insert(0) += c;
    }

    let mut swatches: Vec<Swatch> = by_rgb
        .into_iter()
        .map(|((r, g, b), count)| Swatch {
            hex: to_hex(r, g, b),
            r,
            g,
            b,
            count,
            fraction: ((count as f64 / total as f64) * 10_000.0).round() / 10_000.0,
        })
        .collect();

    // Most-dominant first; tie-break by hex for deterministic ordering.
    swatches.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.hex.cmp(&b.hex)));
    swatches.truncate(n);

    Ok(Palette {
        width: w,
        height: h,
        colors: swatches,
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

    /// A 10x10 image that is 75% red, 25% blue.
    fn red_blue() -> Vec<u8> {
        let mut img = RgbaImage::new(10, 10);
        for y in 0..10u32 {
            for x in 0..10u32 {
                let c = if x < 5 || y < 5 {
                    Rgba([255, 0, 0, 255])
                } else {
                    Rgba([0, 0, 255, 255])
                };
                img.put_pixel(x, y, c);
            }
        }
        encode(img)
    }

    #[test]
    fn extracts_dominant_first() {
        let p = extract(&red_blue(), 8).unwrap();
        assert_eq!((p.width, p.height), (10, 10));
        assert!(!p.colors.is_empty());
        // The most dominant swatch must be reddish (R dominant), not blue.
        let top = &p.colors[0];
        assert!(top.r > top.b, "expected red dominant, got {}", top.hex);
        assert!(top.fraction > 0.5, "red should be > 50%, got {}", top.fraction);
    }

    #[test]
    fn hex_is_lowercase_six_digits() {
        let p = extract(&red_blue(), 4).unwrap();
        for s in &p.colors {
            assert!(s.hex.starts_with('#'), "{}", s.hex);
            assert_eq!(s.hex.len(), 7, "{}", s.hex);
            assert_eq!(s.hex, s.hex.to_ascii_lowercase());
        }
    }

    #[test]
    fn respects_requested_count() {
        // A gradient has many colors; requesting 5 must return <= 5.
        let mut img = RgbaImage::new(16, 16);
        for y in 0..16u32 {
            for x in 0..16u32 {
                img.put_pixel(
                    x,
                    y,
                    Rgba([(x * 16) as u8, (y * 16) as u8, ((x + y) * 8) as u8, 255]),
                );
            }
        }
        let p = extract(&encode(img), 5).unwrap();
        assert!(p.colors.len() <= 5, "got {} colors", p.colors.len());
        assert!(!p.colors.is_empty());
    }

    #[test]
    fn count_is_clamped() {
        let p = extract(&red_blue(), 9999).unwrap();
        assert!(p.colors.len() <= MAX_COLORS);
    }

    #[test]
    fn fractions_sum_to_about_one() {
        let p = extract(&red_blue(), 8).unwrap();
        let sum: f64 = p.colors.iter().map(|s| s.fraction).sum();
        assert!((sum - 1.0).abs() < 0.02, "fractions sum {sum}");
    }

    #[test]
    fn transparent_pixels_ignored() {
        // Half opaque green, half fully transparent: palette should be green only.
        let mut img = RgbaImage::new(10, 10);
        for y in 0..10u32 {
            for x in 0..10u32 {
                let c = if x < 5 {
                    Rgba([0, 200, 0, 255])
                } else {
                    Rgba([255, 255, 255, 0])
                };
                img.put_pixel(x, y, c);
            }
        }
        let p = extract(&encode(img), 8).unwrap();
        let top = &p.colors[0];
        assert!(top.g > top.r && top.g > top.b, "expected green, got {}", top.hex);
    }

    #[test]
    fn rejects_garbage() {
        assert!(extract(b"not an image", 4).is_err());
    }
}
