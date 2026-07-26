//! gizza-ai/histogram-equalizer core — equalize an image's histogram to boost
//! contrast. Pure-Rust (`image`), no wafer/wasm-bindgen deps. Output is PNG.
//!
//! Two methods:
//!   * `global`   — classic histogram equalization: one CDF mapping (LUT) built
//!     from the whole image's histogram and applied to every pixel.
//!   * `adaptive` — CLAHE (Contrast-Limited Adaptive Histogram Equalization):
//!     the image is split into `tile_grid x tile_grid` tiles, each tile's
//!     histogram is clipped at `clip_limit * tile_pixels / 256` (the excess
//!     redistributed evenly) and turned into a per-tile CDF LUT; each output
//!     pixel bilinearly interpolates the four nearest tile LUTs across tile
//!     centres. This limits noise amplification and avoids tile-boundary seams.
//!
//! Three channel modes decide *what* gets equalized:
//!   * `luminance`   — equalize a Rec.601 luma plane, then rescale each pixel's
//!     RGB by `new_y / old_y` so contrast lifts while hue is approximately kept.
//!   * `per_channel` — equalize R, G and B independently (can shift colour).
//!   * `grayscale`   — output a grayscale image (R=G=B) of the equalized luma.
//! Alpha is always preserved. Everything is deterministic so the chat block and
//! CLI produce identical bytes.

use std::io::Cursor;

use image::ImageFormat;

/// Contrast-limit bounds for the adaptive/CLAHE method.
pub const CLIP_LIMIT_MIN: f64 = 1.0;
pub const CLIP_LIMIT_MAX: f64 = 40.0;
pub const DEFAULT_CLIP_LIMIT: f64 = 2.0;

/// Tiles-per-axis bounds for the adaptive/CLAHE method.
pub const TILE_GRID_MIN: u32 = 1;
pub const TILE_GRID_MAX: u32 = 32;
pub const DEFAULT_TILE_GRID: u32 = 8;

/// The canonical method names, in display order. KEEP IN SYNC with `Method::parse`.
pub const METHODS: [&str; 2] = ["adaptive", "global"];
pub const DEFAULT_METHOD: &str = "adaptive";

/// The canonical channel-mode names, in display order. KEEP IN SYNC with
/// `ChannelMode::parse`.
pub const CHANNEL_MODES: [&str; 3] = ["luminance", "per_channel", "grayscale"];
pub const DEFAULT_CHANNEL_MODE: &str = "luminance";

/// Equalization method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// CLAHE — tiled local equalization with contrast limiting.
    Adaptive,
    /// Classic whole-image histogram equalization.
    Global,
}

impl Method {
    /// Parse the `method` argument. `None` / `""` default to `adaptive`.
    pub fn parse(s: Option<&str>) -> Result<Method, String> {
        match s.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            None | Some("") | Some("adaptive") | Some("clahe") => Ok(Method::Adaptive),
            Some("global") | Some("he") => Ok(Method::Global),
            Some(other) => Err(format!(
                "method must be one of {} (got {other:?})",
                METHODS.join("|")
            )),
        }
    }
}

/// Which channel(s) to equalize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    /// Equalize luma, rescale RGB (preserve colour).
    Luminance,
    /// Equalize R, G, B independently.
    PerChannel,
    /// Output grayscale of the equalized luma.
    Grayscale,
}

impl ChannelMode {
    /// Parse the `channel_mode` argument. `None` / `""` default to `luminance`.
    pub fn parse(s: Option<&str>) -> Result<ChannelMode, String> {
        match s.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            None | Some("") | Some("luminance") | Some("luma") => Ok(ChannelMode::Luminance),
            Some("per_channel") | Some("per-channel") | Some("rgb") => Ok(ChannelMode::PerChannel),
            Some("grayscale") | Some("greyscale") | Some("gray") | Some("grey") => {
                Ok(ChannelMode::Grayscale)
            }
            Some(other) => Err(format!(
                "channel_mode must be one of {} (got {other:?})",
                CHANNEL_MODES.join("|")
            )),
        }
    }
}

/// Rec.601 luma of an sRGB pixel, rounded to 0..=255.
fn luma(r: u8, g: u8, b: u8) -> u8 {
    (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn scale_u8(v: u8, s: f32) -> u8 {
    (v as f32 * s).round().clamp(0.0, 255.0) as u8
}

/// Decode `image_bytes`, equalize its histogram, and return PNG bytes.
///
/// * `clip_limit` — CLAHE contrast limit, must be finite and in
///   `CLIP_LIMIT_MIN..=CLIP_LIMIT_MAX` (ignored by the global method).
/// * `tile_grid`  — CLAHE tiles per axis, must be finite and in
///   `TILE_GRID_MIN..=TILE_GRID_MAX` (rounded to a whole number; ignored by the
///   global method).
pub fn equalize(
    image_bytes: &[u8],
    method: Method,
    channel_mode: ChannelMode,
    clip_limit: f64,
    tile_grid: f64,
) -> Result<Vec<u8>, String> {
    if !clip_limit.is_finite() || !(CLIP_LIMIT_MIN..=CLIP_LIMIT_MAX).contains(&clip_limit) {
        return Err(format!(
            "clip_limit must be a number between {CLIP_LIMIT_MIN} and {CLIP_LIMIT_MAX}, got {clip_limit}"
        ));
    }
    if !tile_grid.is_finite()
        || !((TILE_GRID_MIN as f64)..=(TILE_GRID_MAX as f64)).contains(&tile_grid)
    {
        return Err(format!(
            "tile_grid must be a number between {TILE_GRID_MIN} and {TILE_GRID_MAX}, got {tile_grid}"
        ));
    }
    let tiles = tile_grid
        .round()
        .clamp(TILE_GRID_MIN as f64, TILE_GRID_MAX as f64) as u32;

    let img =
        image::load_from_memory(image_bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let mut rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimensions".into());
    }

    match channel_mode {
        ChannelMode::Grayscale => {
            let plane: Vec<u8> = rgba.pixels().map(|p| luma(p.0[0], p.0[1], p.0[2])).collect();
            let eq = equalize_plane(&plane, w, h, method, clip_limit, tiles);
            for (i, px) in rgba.pixels_mut().enumerate() {
                let v = eq[i];
                px.0 = [v, v, v, px.0[3]];
            }
        }
        ChannelMode::PerChannel => {
            for c in 0..3 {
                let plane: Vec<u8> = rgba.pixels().map(|p| p.0[c]).collect();
                let eq = equalize_plane(&plane, w, h, method, clip_limit, tiles);
                for (i, px) in rgba.pixels_mut().enumerate() {
                    px.0[c] = eq[i];
                }
            }
        }
        ChannelMode::Luminance => {
            let plane: Vec<u8> = rgba.pixels().map(|p| luma(p.0[0], p.0[1], p.0[2])).collect();
            let eq = equalize_plane(&plane, w, h, method, clip_limit, tiles);
            for (i, px) in rgba.pixels_mut().enumerate() {
                let old_y = plane[i];
                let new_y = eq[i];
                if old_y == 0 {
                    // Black pixel carries no colour to rescale — emit gray luma.
                    px.0 = [new_y, new_y, new_y, px.0[3]];
                } else {
                    let s = new_y as f32 / old_y as f32;
                    let [r, g, b, a] = px.0;
                    px.0 = [scale_u8(r, s), scale_u8(g, s), scale_u8(b, s), a];
                }
            }
        }
    }

    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out.into_inner())
}

/// Equalize a single 8-bit plane (`w*h` values), returning a new plane.
fn equalize_plane(
    plane: &[u8],
    w: u32,
    h: u32,
    method: Method,
    clip_limit: f64,
    tiles: u32,
) -> Vec<u8> {
    match method {
        Method::Global => {
            let lut = global_lut(plane);
            plane.iter().map(|&v| lut[v as usize]).collect()
        }
        Method::Adaptive => clahe(plane, w, h, clip_limit, tiles),
    }
}

/// Standard global histogram-equalization LUT (CDF mapping with the classic
/// `cdf_min` offset). A uniform plane maps to the identity.
fn global_lut(plane: &[u8]) -> [u8; 256] {
    let mut hist = [0u32; 256];
    for &v in plane {
        hist[v as usize] += 1;
    }
    let total = plane.len() as u32;
    let mut cdf = [0u32; 256];
    let mut acc = 0u32;
    for i in 0..256 {
        acc += hist[i];
        cdf[i] = acc;
    }
    let cdf_min = cdf.iter().copied().find(|&c| c > 0).unwrap_or(0);
    let denom = total.saturating_sub(cdf_min);
    let mut lut = [0u8; 256];
    if denom == 0 {
        // Uniform (or empty) plane — nothing to stretch, stay identity.
        for (i, v) in lut.iter_mut().enumerate() {
            *v = i as u8;
        }
        return lut;
    }
    for i in 0..256 {
        let val = (cdf[i].saturating_sub(cdf_min) as f64 / denom as f64 * 255.0).round();
        lut[i] = val.clamp(0.0, 255.0) as u8;
    }
    lut
}

/// CLAHE over a single plane: build a contrast-limited CDF LUT per tile, then
/// bilinearly interpolate the four nearest tile LUTs for every pixel.
fn clahe(plane: &[u8], w: u32, h: u32, clip_limit: f64, tiles: u32) -> Vec<u8> {
    let tiles = tiles.max(1);
    // Exact tile boundaries partition [0,w) and [0,h) with no gaps/overlaps.
    let xb = |t: u32| ((t as u64 * w as u64) / tiles as u64) as u32;
    let yb = |t: u32| ((t as u64 * h as u64) / tiles as u64) as u32;

    let mut luts: Vec<[u8; 256]> = Vec::with_capacity((tiles * tiles) as usize);
    for ty in 0..tiles {
        let (y0, y1) = (yb(ty), yb(ty + 1));
        for tx in 0..tiles {
            let (x0, x1) = (xb(tx), xb(tx + 1));
            luts.push(tile_lut(plane, w, x0, x1, y0, y1, clip_limit));
        }
    }

    let mut out = vec![0u8; plane.len()];
    for y in 0..h {
        let fy = (y as f64 + 0.5) * tiles as f64 / h as f64 - 0.5;
        let (ty0, ty1, wy) = neighbors(fy, tiles);
        for x in 0..w {
            let fx = (x as f64 + 0.5) * tiles as f64 / w as f64 - 0.5;
            let (tx0, tx1, wx) = neighbors(fx, tiles);
            let v = plane[(y * w + x) as usize] as usize;
            let l00 = luts[(ty0 * tiles + tx0) as usize][v] as f64;
            let l01 = luts[(ty0 * tiles + tx1) as usize][v] as f64;
            let l10 = luts[(ty1 * tiles + tx0) as usize][v] as f64;
            let l11 = luts[(ty1 * tiles + tx1) as usize][v] as f64;
            let top = l00 * (1.0 - wx) + l01 * wx;
            let bot = l10 * (1.0 - wx) + l11 * wx;
            let val = top * (1.0 - wy) + bot * wy;
            out[(y * w + x) as usize] = val.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// The two bracketing tile indices and the interpolation weight for a
/// tile-centre coordinate `f`. Out-of-range coords clamp to the edge tile (so
/// borders use that tile's LUT directly).
fn neighbors(f: f64, tiles: u32) -> (u32, u32, f64) {
    if tiles == 1 {
        return (0, 0, 0.0);
    }
    let t0 = f.floor();
    let frac = (f - t0).clamp(0.0, 1.0);
    let i0 = t0 as i64;
    let max = (tiles - 1) as i64;
    let c0 = i0.clamp(0, max) as u32;
    let c1 = (i0 + 1).clamp(0, max) as u32;
    (c0, c1, frac)
}

/// Contrast-limited CDF LUT for one tile spanning `[x0,x1) x [y0,y1)`.
fn tile_lut(plane: &[u8], w: u32, x0: u32, x1: u32, y0: u32, y1: u32, clip_limit: f64) -> [u8; 256] {
    let tile_pixels = (x1.saturating_sub(x0)) as u64 * (y1.saturating_sub(y0)) as u64;
    let mut lut = [0u8; 256];
    if tile_pixels == 0 {
        // Degenerate tile (image smaller than the grid) — identity mapping.
        for (i, v) in lut.iter_mut().enumerate() {
            *v = i as u8;
        }
        return lut;
    }

    let mut hist = [0u32; 256];
    for y in y0..y1 {
        let row = (y * w) as usize;
        for x in x0..x1 {
            hist[plane[row + x as usize] as usize] += 1;
        }
    }

    // Clip each bin to the normalized average count, then redistribute the
    // clipped excess evenly across all 256 bins (remainder to the first bins,
    // deterministically). This preserves the tile's total pixel count.
    let clip_count = (clip_limit * tile_pixels as f64 / 256.0).floor().max(1.0) as u32;
    let mut excess: u64 = 0;
    for h in hist.iter_mut() {
        if *h > clip_count {
            excess += (*h - clip_count) as u64;
            *h = clip_count;
        }
    }
    let inc = (excess / 256) as u32;
    let rem = (excess % 256) as usize;
    for h in hist.iter_mut() {
        *h += inc;
    }
    for h in hist.iter_mut().take(rem) {
        *h += 1;
    }

    let mut acc: u64 = 0;
    for i in 0..256 {
        acc += hist[i] as u64;
        let val = (acc as f64 / tile_pixels as f64 * 255.0).round();
        lut[i] = val.clamp(0.0, 255.0) as u8;
    }
    lut
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgba, RgbaImage};

    fn encode(img: RgbaImage) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn solid_png(color: [u8; 4]) -> Vec<u8> {
        encode(RgbaImage::from_pixel(8, 8, Rgba(color)))
    }

    /// A low-contrast, spatially-textured grayscale image (luma in a narrow band).
    fn low_contrast_png() -> Vec<u8> {
        encode(RgbaImage::from_fn(16, 16, |x, y| {
            let v = 110 + ((x * 7 + y * 13) % 20) as u8; // 110..=129, range 19
            Rgba([v, v, v, 255])
        }))
    }

    fn luma_stats(png: &[u8]) -> (u8, u8) {
        let img = image::load_from_memory(png).unwrap();
        let mut lo = 255u8;
        let mut hi = 0u8;
        for (_, _, p) in img.pixels() {
            let y = luma(p.0[0], p.0[1], p.0[2]);
            lo = lo.min(y);
            hi = hi.max(y);
        }
        (lo, hi)
    }

    #[test]
    fn output_is_valid_png_same_dimensions() {
        let out = equalize(
            &low_contrast_png(),
            Method::Global,
            ChannelMode::Grayscale,
            2.0,
            8.0,
        )
        .unwrap();
        assert_eq!(&out[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(image::load_from_memory(&out).unwrap().dimensions(), (16, 16));
    }

    #[test]
    fn global_grayscale_stretches_to_full_range() {
        let src = low_contrast_png();
        let (in_lo, in_hi) = luma_stats(&src);
        assert!(in_hi - in_lo < 30, "fixture should be low-contrast");
        let out = equalize(&src, Method::Global, ChannelMode::Grayscale, 2.0, 8.0).unwrap();
        // Global HE pins the least/most common values to 0 and 255.
        let (lo, hi) = luma_stats(&out);
        assert_eq!(lo, 0, "darkest maps to 0");
        assert_eq!(hi, 255, "brightest maps to 255");
        // Grayscale output has R==G==B and preserved alpha.
        let img = image::load_from_memory(&out).unwrap();
        let p = img.get_pixel(0, 0).0;
        assert!(p[0] == p[1] && p[1] == p[2], "grayscale pixel: {p:?}");
        assert_eq!(p[3], 255, "alpha preserved");
    }

    #[test]
    fn adaptive_increases_contrast_of_low_contrast_image() {
        let src = low_contrast_png();
        let (in_lo, in_hi) = luma_stats(&src);
        let out = equalize(&src, Method::Adaptive, ChannelMode::Luminance, 4.0, 8.0).unwrap();
        assert_ne!(src, out, "adaptive equalization must change the pixels");
        let (lo, hi) = luma_stats(&out);
        assert!(
            (hi - lo) > (in_hi - in_lo),
            "adaptive should widen the luma range: {} -> {}",
            in_hi - in_lo,
            hi - lo
        );
    }

    #[test]
    fn per_channel_preserves_alpha() {
        // Semi-transparent, varied-colour image.
        let src = encode(RgbaImage::from_fn(12, 12, |x, y| {
            Rgba([
                100 + (x * 5) as u8,
                90 + (y * 6) as u8,
                120 + ((x + y) * 3) as u8,
                128,
            ])
        }));
        let out = equalize(&src, Method::Global, ChannelMode::PerChannel, 2.0, 8.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (12, 12));
        assert_eq!(img.get_pixel(0, 0).0[3], 128, "alpha preserved");
    }

    #[test]
    fn tile_grid_one_is_whole_image_adaptive() {
        // tile_grid = 1 → a single tile → deterministic, still a valid PNG.
        let out = equalize(
            &low_contrast_png(),
            Method::Adaptive,
            ChannelMode::Grayscale,
            2.0,
            1.0,
        )
        .unwrap();
        assert_eq!(image::load_from_memory(&out).unwrap().dimensions(), (16, 16));
    }

    #[test]
    fn invalid_image_bytes_error() {
        assert!(equalize(
            b"definitely not an image",
            Method::Global,
            ChannelMode::Grayscale,
            2.0,
            8.0
        )
        .is_err());
    }

    #[test]
    fn param_bounds_are_enforced() {
        let img = solid_png([120, 120, 120, 255]);
        // clip_limit out of range / non-finite.
        assert!(equalize(&img, Method::Adaptive, ChannelMode::Luminance, 0.5, 8.0).is_err());
        assert!(equalize(&img, Method::Adaptive, ChannelMode::Luminance, 41.0, 8.0).is_err());
        assert!(equalize(&img, Method::Adaptive, ChannelMode::Luminance, f64::NAN, 8.0).is_err());
        // tile_grid out of range.
        assert!(equalize(&img, Method::Adaptive, ChannelMode::Luminance, 2.0, 0.0).is_err());
        assert!(equalize(&img, Method::Adaptive, ChannelMode::Luminance, 2.0, 33.0).is_err());
        // Boundary values are accepted.
        assert!(equalize(&img, Method::Adaptive, ChannelMode::Luminance, 1.0, 1.0).is_ok());
        assert!(equalize(&img, Method::Adaptive, ChannelMode::Luminance, 40.0, 32.0).is_ok());
    }

    #[test]
    fn method_parse_variants_and_errors() {
        assert_eq!(Method::parse(None).unwrap(), Method::Adaptive);
        assert_eq!(Method::parse(Some("")).unwrap(), Method::Adaptive);
        assert_eq!(Method::parse(Some("ADAPTIVE")).unwrap(), Method::Adaptive);
        assert_eq!(Method::parse(Some("clahe")).unwrap(), Method::Adaptive);
        assert_eq!(Method::parse(Some("global")).unwrap(), Method::Global);
        assert!(Method::parse(Some("bogus")).is_err());
    }

    #[test]
    fn channel_mode_parse_variants_and_errors() {
        assert_eq!(ChannelMode::parse(None).unwrap(), ChannelMode::Luminance);
        assert_eq!(ChannelMode::parse(Some("luma")).unwrap(), ChannelMode::Luminance);
        assert_eq!(
            ChannelMode::parse(Some("per_channel")).unwrap(),
            ChannelMode::PerChannel
        );
        assert_eq!(
            ChannelMode::parse(Some("per-channel")).unwrap(),
            ChannelMode::PerChannel
        );
        assert_eq!(
            ChannelMode::parse(Some("grayscale")).unwrap(),
            ChannelMode::Grayscale
        );
        assert_eq!(
            ChannelMode::parse(Some("grey")).unwrap(),
            ChannelMode::Grayscale
        );
        assert!(ChannelMode::parse(Some("cmyk")).is_err());
    }

    #[test]
    fn methods_and_modes_consts_round_trip_parsers() {
        for m in METHODS {
            assert!(Method::parse(Some(m)).is_ok(), "METHODS entry {m} must parse");
        }
        for m in CHANNEL_MODES {
            assert!(
                ChannelMode::parse(Some(m)).is_ok(),
                "CHANNEL_MODES entry {m} must parse"
            );
        }
    }
}
