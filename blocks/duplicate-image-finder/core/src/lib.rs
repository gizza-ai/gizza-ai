//! gizza-ai/duplicate-image-finder core — batch exact + near-duplicate image
//! detection. No wafer/wasm-bindgen deps; pure compute over already-fetched
//! image bytes, so it runs on every backend (chat Service Worker + native CLI).
//!
//! Algorithm:
//!   - Decode each image with the pure-Rust `image` crate (no fast_image_resize,
//!     so it instantiates under `wafer build` with SIMD off).
//!   - Fingerprint each with a 64-bit **dHash** (difference hash): grayscale →
//!     downscale to 9×8 (bilinear) → 1 bit per adjacent-pixel comparison across
//!     each row. dHash is invariant to brightness/contrast and robust to
//!     resizing and re-compression, so visually-equal images hash close together.
//!   - **Exact** duplicate = byte-identical file content (same FNV-1a-64 of the
//!     input bytes, same dimensions, dHash distance 0).
//!   - **Near** duplicate = not byte-identical but perceptual similarity ≥ the
//!     threshold, where `similarity% = 100·(1 − hamming/64)`.
//!   - Duplicates are grouped by transitive closure over the reported pairs; in
//!     each group the highest-resolution image is the suggested keep, the rest
//!     are delete candidates.

use std::io::Cursor;

use image::{imageops::FilterType, GenericImageView, ImageFormat};
use serde::Serialize;

/// dHash grid: 9 wide × 8 tall → 8 horizontal comparisons per row × 8 rows = 64 bits.
const HASH_W: u32 = 9;
const HASH_H: u32 = 8;

/// Decode guard: reject any single image larger than this many pixels so a huge
/// input can't OOM-trap the 64 MiB chat runtime. 24 MP ≈ a 6000×4000 photo.
pub const MAX_PIXELS: u64 = 24_000_000;

/// One image to scan, with a human-facing label (filename / URL) for the report.
pub struct InputImage {
    pub label: String,
    pub bytes: Vec<u8>,
}

/// Per-image facts surfaced in the report so a user can decide what to delete.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ImageInfo {
    pub index: usize,
    pub source: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
    /// 16-hex-digit perceptual (dHash) fingerprint.
    pub phash: String,
}

/// One reported duplicate pair.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Pair {
    pub a: usize,
    pub b: usize,
    /// Perceptual similarity 0–100 (one decimal). 100 = visually identical.
    pub similarity: f64,
    /// Hamming distance between the two dHashes (0–64).
    pub distance: u32,
    /// "exact" (byte-identical file) or "near" (perceptually similar).
    pub kind: String,
}

/// A cluster of mutually-duplicate images with a keep/delete suggestion.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Group {
    /// Member image indices, ascending.
    pub members: Vec<usize>,
    /// Suggested image to keep (highest resolution; ties → largest file → lowest index).
    pub keep: usize,
    /// Suggested images to delete (every member except `keep`).
    pub delete: Vec<usize>,
    /// "exact" if every member is byte-identical, else "near".
    pub kind: String,
}

/// Roll-up counts.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Summary {
    pub exact_pairs: usize,
    pub near_pairs: usize,
    pub duplicate_groups: usize,
    pub images_flagged_for_deletion: usize,
    pub bytes_reclaimable: usize,
}

/// Full result.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Report {
    pub threshold: f64,
    pub image_count: usize,
    pub images: Vec<ImageInfo>,
    pub pairs: Vec<Pair>,
    pub groups: Vec<Group>,
    pub summary: Summary,
}

struct Fingerprint {
    format: String,
    width: u32,
    height: u32,
    bytes: usize,
    /// FNV-1a-64 over the raw input bytes (exact-file identity).
    content: u64,
    /// 64-bit dHash (perceptual identity).
    phash: u64,
}

fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn format_name(f: ImageFormat) -> String {
    match f {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpeg",
        ImageFormat::WebP => "webp",
        ImageFormat::Gif => "gif",
        ImageFormat::Bmp => "bmp",
        _ => "other",
    }
    .to_string()
}

/// dHash of an already-downscaled 9×8 grayscale image: for each row, set a bit
/// when the right pixel is brighter than its left neighbour.
fn dhash(small: &image::GrayImage) -> u64 {
    let mut hash: u64 = 0;
    let mut bit = 0u32;
    for y in 0..HASH_H {
        for x in 0..(HASH_W - 1) {
            let left = small.get_pixel(x, y).0[0];
            let right = small.get_pixel(x + 1, y).0[0];
            if right > left {
                hash |= 1u64 << bit;
            }
            bit += 1;
        }
    }
    hash
}

fn fingerprint(img: &InputImage) -> Result<Fingerprint, String> {
    if img.bytes.is_empty() {
        return Err(format!("{}: empty input (0 bytes)", img.label));
    }
    // Header-only dimension read first, so a giant image is rejected cleanly
    // instead of OOM-trapping during full decode.
    let reader = image::ImageReader::new(Cursor::new(&img.bytes))
        .with_guessed_format()
        .map_err(|e| format!("{}: not a readable image ({e})", img.label))?;
    let format = reader.format();
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| format!("{}: could not read image dimensions ({e})", img.label))?;
    let pixels = (w as u64) * (h as u64);
    if pixels > MAX_PIXELS {
        return Err(format!(
            "{}: {w}×{h} = {pixels} pixels exceeds the {MAX_PIXELS}-pixel cap",
            img.label
        ));
    }
    let decoded = image::load_from_memory(&img.bytes)
        .map_err(|e| format!("{}: could not decode image ({e})", img.label))?;
    let (w, h) = decoded.dimensions();
    let gray = decoded.to_luma8();
    let small = image::imageops::resize(&gray, HASH_W, HASH_H, FilterType::Triangle);
    Ok(Fingerprint {
        format: format.map(format_name).unwrap_or_else(|| "unknown".to_string()),
        width: w,
        height: h,
        bytes: img.bytes.len(),
        content: fnv1a64(&img.bytes),
        phash: dhash(&small),
    })
}

/// Union-find for grouping duplicates by transitive closure over reported pairs.
struct UnionFind {
    parent: Vec<usize>,
}
impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }
    fn find(&mut self, x: usize) -> usize {
        let mut root = x;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        // path compression
        let mut cur = x;
        while self.parent[cur] != root {
            let next = self.parent[cur];
            self.parent[cur] = root;
            cur = next;
        }
        root
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra] = rb;
        }
    }
}

/// Scan `images` (≥2) and report exact + near-duplicate pairs at/above
/// `threshold` (0–100 similarity percent), grouped with a keep/delete suggestion.
pub fn find_duplicates(images: &[InputImage], threshold: f64) -> Result<Report, String> {
    if images.len() < 2 {
        return Err(format!(
            "need at least 2 images to compare, got {}",
            images.len()
        ));
    }
    if !(0.0..=100.0).contains(&threshold) {
        return Err(format!(
            "threshold must be between 0 and 100, got {threshold}"
        ));
    }

    let fps: Vec<Fingerprint> = images.iter().map(fingerprint).collect::<Result<_, _>>()?;
    let n = images.len();

    let infos: Vec<ImageInfo> = fps
        .iter()
        .enumerate()
        .map(|(i, f)| ImageInfo {
            index: i,
            source: images[i].label.clone(),
            format: f.format.clone(),
            width: f.width,
            height: f.height,
            bytes: f.bytes,
            phash: format!("{:016x}", f.phash),
        })
        .collect();

    let mut pairs: Vec<Pair> = Vec::new();
    let mut uf = UnionFind::new(n);
    for i in 0..n {
        for j in (i + 1)..n {
            let distance = (fps[i].phash ^ fps[j].phash).count_ones();
            let exact = fps[i].content == fps[j].content
                && fps[i].width == fps[j].width
                && fps[i].height == fps[j].height
                && distance == 0;
            let similarity = round1(100.0 * (1.0 - distance as f64 / 64.0));
            if exact || similarity >= threshold {
                pairs.push(Pair {
                    a: i,
                    b: j,
                    similarity,
                    distance,
                    kind: if exact { "exact" } else { "near" }.to_string(),
                });
                uf.union(i, j);
            }
        }
    }

    // Build groups from the union-find components that gained ≥2 members.
    let mut buckets: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    for i in 0..n {
        let root = uf.find(i);
        buckets.entry(root).or_default().push(i);
    }
    let mut groups: Vec<Group> = Vec::new();
    let mut flagged = 0usize;
    let mut bytes_reclaimable = 0usize;
    for members in buckets.into_values() {
        if members.len() < 2 {
            continue;
        }
        // keep = highest resolution; tie → largest file; tie → lowest index.
        let keep = *members
            .iter()
            .max_by(|&&a, &&b| {
                let area = |k: usize| (fps[k].width as u64) * (fps[k].height as u64);
                area(a)
                    .cmp(&area(b))
                    .then(fps[a].bytes.cmp(&fps[b].bytes))
                    .then(b.cmp(&a)) // lower index wins the tie
            })
            .unwrap();
        let delete: Vec<usize> = members.iter().copied().filter(|&m| m != keep).collect();
        flagged += delete.len();
        bytes_reclaimable += delete.iter().map(|&m| fps[m].bytes).sum::<usize>();
        // A group is "exact" only if every member is byte-identical to the keep.
        let all_exact = members.iter().all(|&m| {
            fps[m].content == fps[keep].content
                && fps[m].width == fps[keep].width
                && fps[m].height == fps[keep].height
        });
        groups.push(Group {
            members,
            keep,
            delete,
            kind: if all_exact { "exact" } else { "near" }.to_string(),
        });
    }

    let exact_pairs = pairs.iter().filter(|p| p.kind == "exact").count();
    let near_pairs = pairs.len() - exact_pairs;

    Ok(Report {
        threshold,
        image_count: n,
        images: infos,
        pairs,
        groups: groups.clone(),
        summary: Summary {
            exact_pairs,
            near_pairs,
            duplicate_groups: groups.len(),
            images_flagged_for_deletion: flagged,
            bytes_reclaimable,
        },
    })
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};

    fn png(img: &RgbaImage) -> Vec<u8> {
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    fn solid(w: u32, h: u32, c: [u8; 3]) -> Vec<u8> {
        png(&RgbaImage::from_pixel(w, h, Rgba([c[0], c[1], c[2], 255])))
    }

    /// Horizontal gradient (left→right ramp), optionally brightness-shifted.
    fn h_gradient(w: u32, h: u32, add: u32) -> Vec<u8> {
        png(&RgbaImage::from_fn(w, h, |x, _| {
            let v = ((x * 255 / (w - 1)) + add).min(255) as u8;
            Rgba([v, v, v, 255])
        }))
    }

    /// Vertical gradient (top→bottom ramp): perceptually very different from the
    /// horizontal one (dHash compares horizontally → all bits flip).
    fn v_gradient(w: u32, h: u32) -> Vec<u8> {
        png(&RgbaImage::from_fn(w, h, |_, y| {
            let v = (y * 255 / (h - 1)) as u8;
            Rgba([v, v, v, 255])
        }))
    }

    fn img(label: &str, bytes: Vec<u8>) -> InputImage {
        InputImage {
            label: label.to_string(),
            bytes,
        }
    }

    #[test]
    fn exact_duplicate_is_grouped_with_keep_delete() {
        // Two byte-identical structured images pair as exact; a structurally
        // different image (vertical vs horizontal gradient) must not match.
        let a = h_gradient(16, 16, 0);
        let images = vec![
            img("a.png", a.clone()),
            img("b.png", a.clone()),
            img("c.png", v_gradient(16, 16)),
        ];
        let r = find_duplicates(&images, 90.0).unwrap();
        assert_eq!(r.image_count, 3);
        assert_eq!(r.pairs.len(), 1, "only the two identical copies pair");
        assert_eq!(r.pairs[0].kind, "exact");
        assert_eq!(r.pairs[0].similarity, 100.0);
        assert_eq!(r.pairs[0].distance, 0);
        assert_eq!(r.summary.exact_pairs, 1);
        assert_eq!(r.summary.near_pairs, 0);
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.groups[0].members, vec![0, 1]);
        assert_eq!(r.groups[0].keep, 0, "tie → lowest index kept");
        assert_eq!(r.groups[0].delete, vec![1]);
        assert_eq!(r.groups[0].kind, "exact");
        assert_eq!(r.summary.images_flagged_for_deletion, 1);
        assert_eq!(r.summary.bytes_reclaimable, a.len());
    }

    #[test]
    fn brightness_shifted_twin_is_near_not_exact() {
        // Same gradient shifted +40 brightness: dHash identical (invariant), but
        // the file bytes differ → classified "near" at 100% similarity.
        let images = vec![
            img("base.png", h_gradient(32, 32, 0)),
            img("bright.png", h_gradient(32, 32, 40)),
            img("rotated.png", v_gradient(32, 32)),
        ];
        let r = find_duplicates(&images, 90.0).unwrap();
        assert_eq!(r.pairs.len(), 1, "the vertical gradient must not match");
        assert_eq!(r.pairs[0].a, 0);
        assert_eq!(r.pairs[0].b, 1);
        assert_eq!(r.pairs[0].kind, "near", "different bytes → not exact");
        assert_eq!(r.pairs[0].similarity, 100.0, "dHash is brightness-invariant");
        assert_eq!(r.summary.exact_pairs, 0);
        assert_eq!(r.summary.near_pairs, 1);
    }

    #[test]
    fn keep_prefers_higher_resolution() {
        // Same gradient at two resolutions → a near match (resize is perceptual-
        // hash-invariant); the larger one is the suggested keep even though it
        // has the higher index.
        let small = h_gradient(8, 8, 0);
        let large = h_gradient(64, 64, 0);
        let images = vec![img("small.png", small), img("large.png", large)];
        let r = find_duplicates(&images, 90.0).unwrap();
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.groups[0].keep, 1, "higher resolution kept");
        assert_eq!(r.groups[0].delete, vec![0]);
    }

    #[test]
    fn too_few_images_errors() {
        let one = vec![img("only.png", solid(4, 4, [1, 2, 3]))];
        let err = find_duplicates(&one, 90.0).unwrap_err();
        assert!(err.contains("at least 2"), "got: {err}");
    }

    #[test]
    fn undecodable_input_errors_with_label() {
        let images = vec![
            img("good.png", solid(4, 4, [10, 20, 30])),
            img("junk.bin", b"not an image at all".to_vec()),
        ];
        let err = find_duplicates(&images, 90.0).unwrap_err();
        assert!(err.contains("junk.bin"), "error names the bad input: {err}");
    }

    #[test]
    fn threshold_out_of_range_errors() {
        let images = vec![
            img("a.png", solid(4, 4, [1, 1, 1])),
            img("b.png", solid(4, 4, [2, 2, 2])),
        ];
        let err = find_duplicates(&images, 150.0).unwrap_err();
        assert!(err.contains("between 0 and 100"), "got: {err}");
    }
}
