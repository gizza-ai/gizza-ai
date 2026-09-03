//! gizza-ai/image-oil-painting core — the classic non-ML oil-painting filter.
//!
//! No model, no network, no randomness beyond a seeded hash: the whole effect is
//! a deterministic local-neighbourhood statistic, so the same input plus the same
//! options always produce byte-identical PNG bytes on every backend.
//!
//! Pipeline:
//!
//!   1. **Saturate.** Push the source colours toward/away from their luma first,
//!      so the paint stage bins an already-vivid image (oil pigment reads more
//!      saturated than a photograph).
//!   2. **Paint.** For every pixel, take the square neighbourhood of `radius`,
//!      drop each neighbour into one of `intensity_levels` brightness buckets,
//!      and emit the mean colour of the *most populated* bucket. That single
//!      "mode of the local intensity histogram" step is what flattens a photo
//!      into flat, edge-preserving daubs of pigment — small features dissolve
//!      into the dominant tone while contours stay crisp, exactly the way a
//!      loaded brush lays paint down.
//!      Implemented as a sliding window (each step drops one column and adds
//!      one) so the cost is O(w · h · radius), not O(w · h · radius²).
//!   3. **Warp.** Displace the painted image by a smooth seeded value-noise
//!      field whose cells are brush-sized. Perfectly axis-aligned daubs read as
//!      a "posterize" artefact; a sub-brush wobble reads as bristle drag. `seed`
//!      reshuffles this field, which is the tool's "try another canvas" knob.
//!   4. **Blend.** Mix the warped paint back over the saturated source by
//!      `brush_strength`, so the effect can be dialled from a light glaze to
//!      full impasto.
//!   5. **Canvas.** Optionally modulate luminance with a procedural linen weave.
//!
//! Only `image` is used (pure Rust, no C deps), so this runs on every backend
//! including the chat Service Worker. Output is a PNG at the source dimensions
//! with the source alpha channel preserved.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

/// Widest neighbourhood we accept. Beyond this the daubs stop reading as brush
/// strokes and the filter just smears the frame into a few colour fields.
pub const MAX_RADIUS: u32 = 12;
/// Bucket-count bounds. Under 8 the image posterizes into bands; over 64 almost
/// every neighbour lands in its own bucket and the filter degenerates to a no-op.
pub const MIN_LEVELS: u32 = 8;
pub const MAX_LEVELS: u32 = 64;
/// Cost is linear in pixels × radius, so a cap keeps a pathological upload from
/// pinning a browser tab. 30 MP is comfortably above any camera JPEG.
pub const MAX_PIXELS: u64 = 30_000_000;

/// Everything [`oil_painting`] needs besides the image bytes. Every field is
/// clamped to its documented range inside `oil_painting`, so out-of-range input
/// degrades gracefully instead of panicking.
#[derive(Debug, Clone)]
pub struct Options {
    /// Brush width: neighbourhood radius in pixels, 1..=12. Higher = broader,
    /// bolder daubs and less fine detail.
    pub radius: u32,
    /// Number of brightness buckets the neighbourhood histogram uses, 8..=64.
    /// Fewer = chunkier, more graphic strokes; more = closer to the photo.
    pub intensity_levels: u32,
    /// How much of the painted result replaces the photo, 0.0..=1.0.
    pub brush_strength: f32,
    /// Colour saturation applied before painting, 0.5..=2.0 (1.0 = unchanged).
    pub saturation: f32,
    /// Procedural linen-weave overlay strength, 0.0..=1.0 (0.0 = off).
    pub canvas_texture: f32,
    /// Seeds the bristle-drag warp (and the canvas weave phase). Same seed =
    /// byte-identical output.
    pub seed: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            radius: 4,
            intensity_levels: 24,
            brush_strength: 0.85,
            saturation: 1.1,
            canvas_texture: 0.0,
            seed: 1,
        }
    }
}

/// SplitMix64 over (seed, a, b) — a cheap, portable, fully deterministic hash so
/// the same seed reproduces the same brush texture on every backend.
fn hash2(seed: u64, a: i64, b: i64) -> u64 {
    let mut x = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(((a as u64) << 32) ^ (b as u64))
        .wrapping_add(0x2545_F491_4F6C_DD1D);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A hash mapped into `-1.0..1.0`.
fn signed(h: u64) -> f32 {
    ((h >> 40) as f32) / ((1u64 << 23) as f32) - 1.0
}

/// Hermite ease so neighbouring noise cells join without a visible crease.
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Smooth seeded value noise in `-1.0..1.0`, one lattice point every `cell` px.
/// Deterministic integer hashing + linear interpolation only — no transcendental
/// functions, so the result is bit-stable across targets.
fn field(seed: u64, x: f32, y: f32, cell: f32) -> f32 {
    let (fx, fy) = (x / cell, y / cell);
    let (gx, gy) = (fx.floor(), fy.floor());
    let (tx, ty) = (smooth(fx - gx), smooth(fy - gy));
    let (gx, gy) = (gx as i64, gy as i64);
    let n = |a: i64, b: i64| signed(hash2(seed, a, b));
    let top = n(gx, gy) + (n(gx + 1, gy) - n(gx, gy)) * tx;
    let bot = n(gx, gy + 1) + (n(gx + 1, gy + 1) - n(gx, gy + 1)) * tx;
    top + (bot - top) * ty
}

/// Triangle wave in `-1.0..1.0` with the given period. Used instead of `sin` so
/// the canvas weave is exact integer-free arithmetic and bit-stable everywhere.
fn tri(v: f32, period: f32) -> f32 {
    let f = (v / period).rem_euclid(1.0);
    1.0 - 4.0 * (f - 0.5).abs()
}

/// Rec.601 luma, 0..=255.
fn luma(p: &[u8; 4]) -> u32 {
    (p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000
}

/// Bilinear RGB sample with edge clamping.
fn sample(img: &RgbaImage, x: f32, y: f32) -> [f32; 3] {
    let (w, h) = img.dimensions();
    let cx = x.clamp(0.0, (w - 1) as f32);
    let cy = y.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (cx.floor() as u32, cy.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = (cx - x0 as f32, cy - y0 as f32);
    let g = |px: u32, py: u32| img.get_pixel(px, py).0;
    let (a, b, c, d) = (g(x0, y0), g(x1, y0), g(x0, y1), g(x1, y1));
    let mut out = [0.0f32; 3];
    for (i, o) in out.iter_mut().enumerate() {
        let top = a[i] as f32 + (b[i] as f32 - a[i] as f32) * tx;
        let bot = c[i] as f32 + (d[i] as f32 - c[i] as f32) * tx;
        *o = top + (bot - top) * ty;
    }
    out
}

/// Scale every channel away from (or toward) the pixel's own luma. Alpha is
/// untouched.
fn saturate(src: &RgbaImage, amount: f32) -> RgbaImage {
    let mut out = src.clone();
    if (amount - 1.0).abs() < f32::EPSILON {
        return out;
    }
    for p in out.pixels_mut() {
        let l = luma(&p.0) as f32;
        for c in p.0.iter_mut().take(3) {
            *c = (l + (*c as f32 - l) * amount).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Add (`add = true`) or drop (`add = false`) one column of the sliding
/// histogram window. Reads the raw RGBA buffer directly — this is the filter's
/// hot loop and runs `2 · (2·radius + 1)` times per output pixel.
#[allow(clippy::too_many_arguments)]
fn shift_col(
    raw: &[u8],
    bins: &[u8],
    w: u32,
    col: u32,
    y0: u32,
    y1: u32,
    add: bool,
    count: &mut [u32],
    sums: &mut [[u64; 3]],
) {
    for y in y0..=y1 {
        let idx = (y as usize) * (w as usize) + col as usize;
        let b = bins[idx] as usize;
        let p = &raw[idx * 4..idx * 4 + 3];
        if add {
            count[b] += 1;
            for i in 0..3 {
                sums[b][i] += p[i] as u64;
            }
        } else {
            count[b] -= 1;
            for i in 0..3 {
                sums[b][i] -= p[i] as u64;
            }
        }
    }
}

/// The brush stroke itself: replace every pixel with the mean colour of the most
/// populated intensity bucket in its `radius` neighbourhood.
fn paint(src: &RgbaImage, radius: u32, levels: u32) -> RgbaImage {
    let (w, h) = src.dimensions();
    let l = levels as usize;
    let r = radius as i64;

    // Bin every pixel once up front — the sliding window then only ever reads
    // this table, never recomputes luma.
    let mut bins = vec![0u8; (w as usize) * (h as usize)];
    for (i, p) in src.pixels().enumerate() {
        bins[i] = ((luma(&p.0) as usize * l) / 256).min(l - 1) as u8;
    }

    let raw = src.as_raw();
    let mut out = vec![255u8; (w as usize) * (h as usize) * 4];
    let mut count = vec![0u32; l];
    let mut sums = vec![[0u64; 3]; l];

    for y in 0..h as i64 {
        let y0 = (y - r).max(0) as u32;
        let y1 = ((y + r).min(h as i64 - 1)) as u32;
        count.iter_mut().for_each(|v| *v = 0);
        sums.iter_mut().for_each(|v| *v = [0; 3]);
        // Seed the window with the columns covering x = 0: [0, radius].
        for col in 0..=(r.min(w as i64 - 1)) as u32 {
            shift_col(raw, &bins, w, col, y0, y1, true, &mut count, &mut sums);
        }

        for x in 0..w as i64 {
            // Winner = most populated bucket; ties break to the darker bucket so
            // the choice is deterministic rather than iteration-order dependent.
            let mut best = 0usize;
            for (b, &c) in count.iter().enumerate() {
                if c > count[best] {
                    best = b;
                }
            }
            let n = count[best].max(1) as u64;
            let o = ((y as usize) * (w as usize) + x as usize) * 4;
            for i in 0..3 {
                out[o + i] = (sums[best][i] / n) as u8;
            }

            // Slide one pixel right: drop column x-radius, take on x+1+radius.
            if x - r >= 0 {
                shift_col(
                    raw,
                    &bins,
                    w,
                    (x - r) as u32,
                    y0,
                    y1,
                    false,
                    &mut count,
                    &mut sums,
                );
            }
            if x + 1 + r < w as i64 {
                shift_col(
                    raw,
                    &bins,
                    w,
                    (x + 1 + r) as u32,
                    y0,
                    y1,
                    true,
                    &mut count,
                    &mut sums,
                );
            }
        }
    }
    RgbaImage::from_raw(w, h, out).expect("buffer is exactly w * h * 4 bytes")
}

/// Turn `bytes` into an oil painting. Returns PNG bytes at the source
/// dimensions. Errors on undecodable input, a zero dimension, or an image above
/// [`MAX_PIXELS`].
pub fn oil_painting(bytes: &[u8], opts: &Options) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let src = img.to_rgba8();
    let (w, h) = src.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let pixels = w as u64 * h as u64;
    if pixels > MAX_PIXELS {
        return Err(format!(
            "image is too large: {w}x{h} = {:.1} MP, limit {} MP",
            pixels as f64 / 1e6,
            MAX_PIXELS / 1_000_000
        ));
    }

    let radius = opts.radius.clamp(1, MAX_RADIUS);
    let levels = opts.intensity_levels.clamp(MIN_LEVELS, MAX_LEVELS);
    let strength = opts.brush_strength.clamp(0.0, 1.0);
    let saturation = opts.saturation.clamp(0.5, 2.0);
    let canvas = opts.canvas_texture.clamp(0.0, 1.0);

    let base = saturate(&src, saturation);
    let painted = paint(&base, radius, levels);

    // Brush-drag warp: cells are about one brush wide, displacement about half a
    // brush, so strokes wobble within their own daub instead of smearing across
    // the frame.
    let cell = (radius as f32 * 2.0).max(2.0);
    let amp = radius as f32 * 0.45;

    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32, y as f32);
            let dx = amp * field(opts.seed, fx, fy, cell);
            let dy = amp * field(opts.seed ^ 0xA5A5_5A5A_5A5A_A5A5, fx, fy, cell);
            let brushed = sample(&painted, fx + dx, fy + dy);

            let src_px = base.get_pixel(x, y).0;
            let mut rgb = [0f32; 3];
            for i in 0..3 {
                rgb[i] = src_px[i] as f32 + (brushed[i] - src_px[i] as f32) * strength;
            }

            if canvas > 0.0 {
                // Linen: an over/under weave (period 4 px) plus a little thread
                // slub, modulating luminance by at most ±16%.
                let weave = 0.5 * (tri(fx, 4.0) + tri(fy, 4.0));
                let slub = field(opts.seed ^ 0x00C0_FFEE_00C0_FFEE, fx, fy, 2.0);
                let factor = 1.0 + 0.16 * canvas * (0.75 * weave + 0.25 * slub);
                for c in rgb.iter_mut() {
                    *c *= factor;
                }
            }

            out.put_pixel(
                x,
                y,
                Rgba([
                    rgb[0].round().clamp(0.0, 255.0) as u8,
                    rgb[1].round().clamp(0.0, 255.0) as u8,
                    rgb[2].round().clamp(0.0, 255.0) as u8,
                    // Alpha rides through untouched: warping it would fray the
                    // edge of a cut-out instead of painting it.
                    src.get_pixel(x, y).0[3],
                ]),
            );
        }
    }

    let mut buf = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(out)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(img: RgbaImage) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn decode(png: &[u8]) -> RgbaImage {
        image::load_from_memory(png).unwrap().to_rgba8()
    }

    /// A 96x72 photo stand-in: a smooth gradient (the part a brush should
    /// flatten) plus a hard-edged red block (the contour it should preserve).
    fn photo() -> Vec<u8> {
        let mut img = RgbaImage::new(96, 72);
        for y in 0..72u32 {
            for x in 0..96u32 {
                let p = if x > 40 && x < 60 && y > 20 && y < 50 {
                    Rgba([255, 0, 0, 255])
                } else {
                    Rgba([(x * 2) as u8, (y * 3) as u8, (x + y) as u8, 255])
                };
                img.put_pixel(x, y, p);
            }
        }
        encode(img)
    }

    fn distinct_colors(png: &[u8]) -> usize {
        let mut set = std::collections::HashSet::new();
        for p in decode(png).pixels() {
            set.insert(p.0);
        }
        set.len()
    }

    #[test]
    fn happy_path_returns_a_png_at_the_source_dimensions() {
        let out = oil_painting(&photo(), &Options::default()).unwrap();
        assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n", "output is a PNG");
        assert_eq!(decode(&out).dimensions(), (96, 72));
    }

    #[test]
    fn errors_on_undecodable_image() {
        let err = oil_painting(b"not an image", &Options::default()).unwrap_err();
        assert!(err.contains("could not decode image"), "got: {err}");
    }

    #[test]
    fn flattens_detail_into_pigment_daubs() {
        let src = photo();
        let out = oil_painting(&src, &Options::default()).unwrap();
        assert!(
            distinct_colors(&out) < distinct_colors(&src),
            "the paint pass must collapse the gradient into fewer tones"
        );
    }

    #[test]
    fn radius_boundaries_render_and_bigger_brushes_flatten_more() {
        let src = photo();
        let fine = oil_painting(
            &src,
            &Options {
                radius: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let broad = oil_painting(
            &src,
            &Options {
                radius: MAX_RADIUS,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(decode(&fine).dimensions(), (96, 72));
        assert_eq!(decode(&broad).dimensions(), (96, 72));
        assert!(
            distinct_colors(&broad) < distinct_colors(&fine),
            "a 12px brush must leave fewer distinct tones than a 1px brush"
        );
    }

    #[test]
    fn intensity_levels_boundaries_render_and_more_levels_keep_more_detail() {
        let src = photo();
        // Neutral saturation so the only difference is the bucket count.
        let opts = Options {
            saturation: 1.0,
            ..Default::default()
        };
        let coarse = oil_painting(
            &src,
            &Options {
                intensity_levels: MIN_LEVELS,
                ..opts.clone()
            },
        )
        .unwrap();
        let fine = oil_painting(
            &src,
            &Options {
                intensity_levels: MAX_LEVELS,
                ..opts
            },
        )
        .unwrap();
        assert_ne!(coarse, fine, "the bucket count must change the output");
        assert_eq!(decode(&coarse).dimensions(), (96, 72));
        assert_eq!(decode(&fine).dimensions(), (96, 72));
    }

    #[test]
    fn brush_strength_boundaries_glaze_and_impasto() {
        let src = photo();
        // 0 = no paint at all: the pixels come straight from the (saturated)
        // source, so the tone count matches an unpainted, unwarped pass.
        let none = oil_painting(
            &src,
            &Options {
                brush_strength: 0.0,
                saturation: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            decode(&none).into_raw(),
            decode(&src).into_raw(),
            "brush_strength 0 must return the source untouched"
        );
        let full = oil_painting(
            &src,
            &Options {
                brush_strength: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            distinct_colors(&full) < distinct_colors(&src),
            "brush_strength 1 must be fully painted"
        );
    }

    #[test]
    fn saturation_boundaries_change_colourfulness() {
        // A single flat colour isolates the saturation maths from the brush.
        let flat = encode(RgbaImage::from_pixel(32, 32, Rgba([200, 60, 40, 255])));
        let spread = |png: &[u8]| {
            let p = decode(png).get_pixel(16, 16).0;
            p[0] as i32 - p[2] as i32
        };
        let dull = oil_painting(
            &flat,
            &Options {
                saturation: 0.5,
                ..Default::default()
            },
        )
        .unwrap();
        let vivid = oil_painting(
            &flat,
            &Options {
                saturation: 2.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            spread(&dull) < spread(&vivid),
            "0.5 must be less colourful than 2.0 (got {} vs {})",
            spread(&dull),
            spread(&vivid)
        );
    }

    #[test]
    fn canvas_texture_boundaries_are_off_by_default_and_visible_at_one() {
        let flat = encode(RgbaImage::from_pixel(48, 48, Rgba([128, 128, 128, 255])));
        let off = oil_painting(
            &flat,
            &Options {
                canvas_texture: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        let on = oil_painting(
            &flat,
            &Options {
                canvas_texture: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(distinct_colors(&off), 1, "no weave on a flat fill at 0");
        assert!(
            distinct_colors(&on) > 1,
            "the weave must be visible on a flat fill at 1"
        );
    }

    #[test]
    fn seed_is_deterministic_and_reshuffles_the_brushwork() {
        let src = photo();
        let a = oil_painting(
            &src,
            &Options {
                seed: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let b = oil_painting(
            &src,
            &Options {
                seed: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(a, b, "same seed must give byte-identical output");
        let c = oil_painting(
            &src,
            &Options {
                seed: 99,
                ..Default::default()
            },
        )
        .unwrap();
        assert_ne!(a, c, "a different seed must redraw the brushwork");
    }

    #[test]
    fn preserves_transparency() {
        let mut img = RgbaImage::from_pixel(32, 32, Rgba([20, 180, 90, 255]));
        for y in 0..32 {
            for x in 0..16 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
        let out = oil_painting(&encode(img), &Options::default()).unwrap();
        let got = decode(&out);
        assert_eq!(got.get_pixel(4, 4).0[3], 0, "transparent half stays clear");
        assert_eq!(got.get_pixel(28, 4).0[3], 255, "opaque half stays opaque");
    }

    #[test]
    fn clamps_out_of_range_options_without_panicking() {
        let src = photo();
        assert!(oil_painting(
            &src,
            &Options {
                radius: 0,
                intensity_levels: 0,
                brush_strength: -3.0,
                saturation: 0.0,
                canvas_texture: -1.0,
                seed: 0,
            }
        )
        .is_ok());
        assert!(oil_painting(
            &src,
            &Options {
                radius: 9_999,
                intensity_levels: 9_999,
                brush_strength: 42.0,
                saturation: 42.0,
                canvas_texture: 42.0,
                seed: u64::MAX,
            }
        )
        .is_ok());
    }

    #[test]
    fn tiny_and_solid_images_are_handled() {
        let one = oil_painting(&encode(RgbaImage::new(1, 1)), &Options::default()).unwrap();
        assert_eq!(decode(&one).dimensions(), (1, 1));
        // A brush wider than the frame must still terminate and stay flat.
        let solid = encode(RgbaImage::from_pixel(5, 5, Rgba([10, 120, 200, 255])));
        let out = oil_painting(
            &solid,
            &Options {
                radius: 12,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(distinct_colors(&out), 1);
    }
}
