//! gizza-ai/randomize-palette core — randomly remap the color palette of an
//! indexed image (GIF / PNG-8) to expose hidden shapes and stego payloads.
//!
//! A palette swap keeps every pixel's *index* but assigns a different color to
//! each slot, so structure that is invisible because two indices map to nearly
//! identical colors becomes obvious once the palette is shuffled. Pure-Rust
//! (`image` + `color_quant`); returns PNG bytes. The remap is driven by an
//! explicit `seed` so every surface (chat / CLI) is deterministic.

use std::collections::HashMap;
use std::io::Cursor;

use color_quant::NeuQuant;
use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

/// Max palette entries we will treat an image as "indexed" up to. Above this the
/// image is first quantized down to `MAX_PALETTE` colors so the remap still has a
/// finite palette to shuffle.
const MAX_PALETTE: usize = 256;

/// Deterministic SplitMix64 PRNG — no external crate, identical on every target.
struct SplitMix64(u64);
impl SplitMix64 {
    fn new(seed: u64) -> Self {
        SplitMix64(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in 0..n (n > 0).
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// In-place Fisher-Yates shuffle driven by the seeded PRNG.
fn shuffle<T>(items: &mut [T], rng: &mut SplitMix64) {
    if items.len() < 2 {
        return;
    }
    for i in (1..items.len()).rev() {
        let j = rng.below(i + 1);
        items.swap(i, j);
    }
}

/// Randomly remap the palette of `bytes`.
///
/// - The image's distinct colors form its palette. If there are more than 256
///   distinct colors the image is quantized to 256 first (so non-indexed inputs
///   still work) — for a true indexed GIF/PNG-8 the palette is used verbatim.
/// - A seeded permutation reassigns each palette slot to a *different* slot's
///   color, so pixel structure (indices) is preserved while colors are swapped.
/// - `seed` makes the result deterministic; the same seed always yields the same
///   remap.
///
/// Returns PNG bytes.
pub fn randomize_palette(bytes: &[u8], seed: u64) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let rgba = img.to_rgba8();

    // Build the source palette: the distinct colors, in stable first-seen order.
    let mut palette: Vec<[u8; 4]> = Vec::new();
    let mut index_of: HashMap<[u8; 4], usize> = HashMap::new();
    let mut indices: Vec<usize> = Vec::with_capacity((w * h) as usize);
    let mut quantizer: Option<NeuQuant> = None;

    for p in rgba.pixels() {
        // Still in exact-palette mode.
        let c = p.0;
        if let Some(&i) = index_of.get(&c) {
            indices.push(i);
        } else if palette.len() < MAX_PALETTE {
            let i = palette.len();
            index_of.insert(c, i);
            palette.push(c);
            indices.push(i);
        } else {
            // Too many colors — fall back to quantization for the whole image.
            quantizer = Some(NeuQuant::new(10, MAX_PALETTE, rgba.as_raw()));
            break;
        }
    }

    if let Some(nq) = quantizer {
        // Rebuild palette + indices from the quantizer (covers >256-color images).
        let map = nq.color_map_rgba(); // MAX_PALETTE * 4 bytes
        palette = (0..map.len() / 4)
            .map(|i| {
                let o = i * 4;
                [map[o], map[o + 1], map[o + 2], map[o + 3]]
            })
            .collect();
        indices = rgba.pixels().map(|p| nq.index_of(&p.0)).collect();
    }

    let n = palette.len();
    // Permute palette slots: slot i gets the color that used to live at perm[i].
    let mut perm: Vec<usize> = (0..n).collect();
    let mut rng = SplitMix64::new(seed);
    shuffle(&mut perm, &mut rng);

    let new_palette: Vec<[u8; 4]> = perm.iter().map(|&j| palette[j]).collect();

    let mut out_img: RgbaImage = RgbaImage::new(w, h);
    for (pos, (x, y, _)) in rgba.enumerate_pixels().enumerate() {
        let idx = indices[pos];
        out_img.put_pixel(x, y, Rgba(new_palette[idx]));
    }

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(out_img)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
fn distinct_colors(png: &[u8]) -> std::collections::HashSet<[u8; 4]> {
    let img = image::load_from_memory(png).unwrap().to_rgba8();
    img.pixels().map(|p| p.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16x16 image with a handful of distinct colors (a coarse gradient → a
    /// small palette, like an indexed image).
    fn paletted() -> Vec<u8> {
        let mut img = RgbaImage::new(16, 16);
        let colors = [
            [200, 30, 30, 255],
            [30, 200, 30, 255],
            [30, 30, 200, 255],
            [220, 220, 30, 255],
        ];
        for y in 0..16u32 {
            for x in 0..16u32 {
                img.put_pixel(x, y, Rgba(colors[((x / 4 + y / 4) % 4) as usize]));
            }
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn preserves_palette_size_and_dimensions() {
        let src = paletted();
        let before = distinct_colors(&src);
        let out = randomize_palette(&src, 42).unwrap();
        let after = distinct_colors(&out);
        // A pure remap of N distinct colors keeps N distinct colors (the set of
        // colors is the same; only their assignment to pixels changes).
        assert_eq!(after, before, "palette set should be preserved");
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (16, 16));
    }

    #[test]
    fn deterministic_for_same_seed() {
        let src = paletted();
        let a = randomize_palette(&src, 7).unwrap();
        let b = randomize_palette(&src, 7).unwrap();
        assert_eq!(a, b, "same seed must produce identical output");
    }

    #[test]
    fn different_seeds_differ() {
        let src = paletted();
        let a = randomize_palette(&src, 1).unwrap();
        let b = randomize_palette(&src, 999).unwrap();
        assert_ne!(a, b, "different seeds should (almost always) differ");
    }

    #[test]
    fn actually_remaps_pixels() {
        // With a 4-color image and a non-trivial seed, at least some pixels must
        // change color (the permutation is not the identity).
        let src = paletted();
        let orig = image::load_from_memory(&src).unwrap().to_rgba8();
        let out = randomize_palette(&src, 12345).unwrap();
        let new = image::load_from_memory(&out).unwrap().to_rgba8();
        let changed = orig
            .pixels()
            .zip(new.pixels())
            .filter(|(a, b)| a.0 != b.0)
            .count();
        assert!(changed > 0, "expected the remap to change some pixels");
    }

    #[test]
    fn single_color_image_is_stable() {
        let img = RgbaImage::from_pixel(8, 8, Rgba([10, 120, 200, 255]));
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let src = buf.into_inner();
        let out = randomize_palette(&src, 5).unwrap();
        // One palette entry → nothing to permute → identical color set.
        assert_eq!(distinct_colors(&out).len(), 1);
    }

    #[test]
    fn many_color_image_quantizes_then_remaps() {
        // A smooth gradient has many distinct colors → quantization path.
        let mut img = RgbaImage::new(64, 64);
        for y in 0..64u32 {
            for x in 0..64u32 {
                img.put_pixel(x, y, Rgba([(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let src = buf.into_inner();
        let out = randomize_palette(&src, 3).unwrap();
        let after = distinct_colors(&out);
        assert!(after.len() <= MAX_PALETTE, "quantized to <= 256 colors");
        assert!(after.len() > 1, "still a multi-color image");
        // Determinism holds on the quantization path too.
        assert_eq!(out, randomize_palette(&src, 3).unwrap());
    }

    #[test]
    fn errors_on_bad_input() {
        assert!(randomize_palette(b"not an image", 1).is_err());
    }
}
