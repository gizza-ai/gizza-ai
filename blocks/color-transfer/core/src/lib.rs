//! color-transfer core — recolor a target photo so it carries the colour mood of a
//! reference photo, by matching per-channel statistics. Pure `image` crate; no
//! wafer/wasm-bindgen deps.
//!
//! Pipeline: image A (target) is decoded at its native size (pixel capped) and keeps
//! its geometry and alpha; image B (reference) is only ever read for statistics, so it
//! is downscaled to a small sampling copy first. `Method` picks how the statistics are
//! matched — Reinhard-style mean/standard-deviation matching in CIELAB (default) or in
//! raw RGB, per-channel histogram (CDF) matching, or a mean-only shift that moves the
//! colour cast without changing contrast. `preserve_luminance`, `saturation` and
//! `strength` are post-passes so every method shares them.

use std::io::Cursor;

use image::{imageops::FilterType, DynamicImage, ImageFormat, RgbImage, RgbaImage};

/// Longest side accepted for the output; larger targets are downscaled first.
const MAX_DIM: u32 = 5000;
/// Output pixel guard (~24 MP) — Lab conversion is per-pixel float work.
const MAX_PIXELS: u64 = 24_000_000;
/// The reference image is only sampled for statistics, so a small copy is enough.
const STATS_DIM: u32 = 512;

/// How the reference image's colour statistics are matched onto the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Reinhard-style mean + standard-deviation matching in CIELAB (perceptual).
    LabStats,
    /// The same mean + standard-deviation matching applied per sRGB channel.
    RgbStats,
    /// Per-channel histogram (CDF) matching in RGB — strongest, film-look match.
    Histogram,
    /// Shift the CIELAB channel means only; contrast/spread of the target is kept.
    MeanOnly,
}

pub fn parse_method(s: &str) -> Result<Method, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "lab-stats" | "lab" | "reinhard" => Ok(Method::LabStats),
        "rgb-stats" | "rgb" => Ok(Method::RgbStats),
        "histogram" | "hist" => Ok(Method::Histogram),
        "mean-only" | "mean" => Ok(Method::MeanOnly),
        other => Err(format!(
            "method {other:?} not supported (lab-stats|rgb-stats|histogram|mean-only)"
        )),
    }
}

/// Output encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Png,
    Jpeg,
}

pub fn parse_format(s: &str) -> Result<OutFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "png" => Ok(OutFormat::Png),
        "jpeg" | "jpg" => Ok(OutFormat::Jpeg),
        other => Err(format!("format {other:?} not supported (png|jpeg)")),
    }
}

impl OutFormat {
    pub fn mime(self) -> &'static str {
        match self {
            OutFormat::Png => "image/png",
            OutFormat::Jpeg => "image/jpeg",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            OutFormat::Png => "png",
            OutFormat::Jpeg => "jpg",
        }
    }
}

/// Every knob the transfer exposes, so the surfaces pass one struct.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Statistics-matching method.
    pub method: Method,
    /// 0..=100 blend of the recoloured result over the untouched target.
    pub strength: f64,
    /// Keep the target's own lightness and take only the reference's colour.
    pub preserve_luminance: bool,
    /// 0..=200 percent scaling of the result's CIELAB chroma (100 = unchanged).
    pub saturation: f64,
    /// Output encoding.
    pub format: OutFormat,
    /// JPEG quality 1..=100 (ignored for PNG).
    pub quality: u8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            method: Method::LabStats,
            strength: 100.0,
            preserve_luminance: false,
            saturation: 100.0,
            format: OutFormat::Png,
            quality: 90,
        }
    }
}

// ---------------------------------------------------------------- colour space

const WHITE_X: f32 = 0.950_47;
const WHITE_Y: f32 = 1.0;
const WHITE_Z: f32 = 1.088_83;
const LAB_EPS: f32 = 216.0 / 24389.0;
const LAB_KAPPA: f32 = 24389.0 / 27.0;

fn srgb_to_linear(c: f32) -> f32 {
    let c = c / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    let v = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v * 255.0).clamp(0.0, 255.0)
}

/// sRGB (0..=255 per channel) → CIELAB (L 0..=100, a/b roughly ±128), D65.
pub fn rgb_to_lab(rgb: [f32; 3]) -> [f32; 3] {
    let r = srgb_to_linear(rgb[0]);
    let g = srgb_to_linear(rgb[1]);
    let b = srgb_to_linear(rgb[2]);
    let x = (0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / WHITE_X;
    let y = (0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b) / WHITE_Y;
    let z = (0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b) / WHITE_Z;
    let f = |t: f32| {
        if t > LAB_EPS {
            t.cbrt()
        } else {
            (LAB_KAPPA * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIELAB → *linear* sRGB, unclamped (values outside 0..=1 are out of gamut).
fn lab_to_linear(lab: [f32; 3]) -> [f32; 3] {
    let fy = (lab[0] + 16.0) / 116.0;
    let fx = fy + lab[1] / 500.0;
    let fz = fy - lab[2] / 200.0;
    let inv = |t: f32| {
        let t3 = t * t * t;
        if t3 > LAB_EPS {
            t3
        } else {
            (116.0 * t - 16.0) / LAB_KAPPA
        }
    };
    let x = inv(fx) * WHITE_X;
    let y = if lab[0] > LAB_KAPPA * LAB_EPS {
        fy * fy * fy * WHITE_Y
    } else {
        lab[0] / LAB_KAPPA * WHITE_Y
    };
    let z = inv(fz) * WHITE_Z;
    let r = 3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z;
    let g = -0.969_266_0 * x + 1.876_010_8 * y + 0.041_556_0 * z;
    let b = 0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z;
    [r, g, b]
}

/// CIELAB → sRGB (0..=255 per channel, hard-clamped into gamut), D65.
pub fn lab_to_rgb(lab: [f32; 3]) -> [f32; 3] {
    let lin = lab_to_linear(lab);
    [linear_to_srgb(lin[0]), linear_to_srgb(lin[1]), linear_to_srgb(lin[2])]
}

fn in_gamut(lin: [f32; 3]) -> bool {
    lin.iter().all(|c| *c >= -0.001 && *c <= 1.001)
}

/// CIELAB → sRGB, keeping **L** exact by desaturating instead of clipping.
///
/// Hard clipping an out-of-gamut colour changes its lightness (a very saturated dark
/// red clips up to a brighter red), which would break `preserve_luminance`. Binary-search
/// the largest chroma scale that still lands inside the sRGB gamut, so the returned
/// colour keeps the requested lightness and hue and only loses saturation it could not
/// have displayed anyway.
pub fn lab_to_rgb_keep_lightness(lab: [f32; 3]) -> [f32; 3] {
    if in_gamut(lab_to_linear(lab)) {
        return lab_to_rgb(lab);
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..16 {
        let mid = (lo + hi) / 2.0;
        if in_gamut(lab_to_linear([lab[0], lab[1] * mid, lab[2] * mid])) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lab_to_rgb([lab[0], lab[1] * lo, lab[2] * lo])
}

// ----------------------------------------------------------------- statistics

/// Mean and standard deviation of one channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelStats {
    pub mean: f32,
    pub std: f32,
}

/// Per-channel mean/std of a triple-channel pixel list.
pub fn channel_stats(pixels: &[[f32; 3]]) -> [ChannelStats; 3] {
    let n = pixels.len().max(1) as f32;
    let mut sum = [0.0f32; 3];
    for p in pixels {
        for c in 0..3 {
            sum[c] += p[c];
        }
    }
    let mean = [sum[0] / n, sum[1] / n, sum[2] / n];
    let mut var = [0.0f32; 3];
    for p in pixels {
        for c in 0..3 {
            let d = p[c] - mean[c];
            var[c] += d * d;
        }
    }
    [0, 1, 2].map(|c| ChannelStats { mean: mean[c], std: (var[c] / n).sqrt() })
}

/// σ_reference / σ_target, guarded against a flat (zero-variance) target channel and
/// against runaway amplification when the target channel is nearly flat.
fn std_ratio(target: f32, reference: f32) -> f32 {
    if target <= 1e-4 {
        1.0
    } else {
        (reference / target).clamp(0.05, 20.0)
    }
}

/// 256-bin per-channel LUT mapping target levels onto the reference's distribution.
fn histogram_luts(target: &[[f32; 3]], reference: &[[f32; 3]]) -> [[u8; 256]; 3] {
    let cdf = |pixels: &[[f32; 3]], c: usize| {
        let mut hist = [0u64; 256];
        for p in pixels {
            hist[p[c].round().clamp(0.0, 255.0) as usize] += 1;
        }
        let total = pixels.len().max(1) as f64;
        let mut cdf = [0.0f64; 256];
        let mut acc = 0u64;
        for i in 0..256 {
            acc += hist[i];
            cdf[i] = acc as f64 / total;
        }
        cdf
    };
    let mut luts = [[0u8; 256]; 3];
    for c in 0..3 {
        let src = cdf(target, c);
        let dst = cdf(reference, c);
        let mut j = 0usize;
        for i in 0..256 {
            while j < 255 && dst[j] < src[i] {
                j += 1;
            }
            luts[c][i] = j as u8;
        }
    }
    luts
}

// ------------------------------------------------------------------- transfer

/// Downscale so the longest side is at most `dim` (never upscales).
fn sample_copy(img: &DynamicImage, dim: u32) -> RgbImage {
    let (w, h) = (img.width().max(1), img.height().max(1));
    if w <= dim && h <= dim {
        return img.to_rgb8();
    }
    let s = dim as f64 / w.max(h) as f64;
    let nw = ((w as f64 * s).round() as u32).max(1);
    let nh = ((h as f64 * s).round() as u32).max(1);
    img.resize_exact(nw, nh, FilterType::Triangle).to_rgb8()
}

/// Capped output canvas size for the target image.
fn canvas_size(w: u32, h: u32) -> (u32, u32) {
    let mut scale = 1.0f64;
    if w > MAX_DIM {
        scale = scale.min(MAX_DIM as f64 / w as f64);
    }
    if h > MAX_DIM {
        scale = scale.min(MAX_DIM as f64 / h as f64);
    }
    let px = w as u64 * h as u64;
    if px > MAX_PIXELS {
        scale = scale.min((MAX_PIXELS as f64 / px as f64).sqrt());
    }
    (((w as f64 * scale) as u32).max(1), ((h as f64 * scale) as u32).max(1))
}

/// Recolour `target` with the colour mood of `reference`.
///
/// The output keeps the target's dimensions (pixel capped) and alpha channel; the
/// reference contributes statistics only, never geometry.
pub fn transfer(
    target: DynamicImage,
    reference: DynamicImage,
    opts: Options,
) -> Result<Vec<u8>, String> {
    if target.width() == 0 || target.height() == 0 {
        return Err("target image (first) has zero pixels".into());
    }
    if reference.width() == 0 || reference.height() == 0 {
        return Err("reference image (second) has zero pixels".into());
    }
    let (cw, ch) = canvas_size(target.width(), target.height());
    let target = if (cw, ch) == (target.width(), target.height()) {
        target
    } else {
        target.resize_exact(cw, ch, FilterType::Lanczos3)
    };
    let base: RgbaImage = target.to_rgba8();

    // Original RGB (0..255 floats) + the reference's small sampling copy.
    let orig: Vec<[f32; 3]> = base
        .pixels()
        .map(|p| [p.0[0] as f32, p.0[1] as f32, p.0[2] as f32])
        .collect();
    let refc = sample_copy(&reference, STATS_DIM);
    let refr: Vec<[f32; 3]> = refc
        .pixels()
        .map(|p| [p.0[0] as f32, p.0[1] as f32, p.0[2] as f32])
        .collect();

    let mut out: Vec<[f32; 3]> = match opts.method {
        Method::LabStats | Method::MeanOnly => {
            let tl: Vec<[f32; 3]> = orig.iter().map(|p| rgb_to_lab(*p)).collect();
            let rl: Vec<[f32; 3]> = refr.iter().map(|p| rgb_to_lab(*p)).collect();
            let ts = channel_stats(&tl);
            let rs = channel_stats(&rl);
            let gains = [0, 1, 2].map(|c| match opts.method {
                Method::MeanOnly => 1.0,
                _ => std_ratio(ts[c].std, rs[c].std),
            });
            tl.iter()
                .map(|p| {
                    let lab = [0, 1, 2]
                        .map(|c| (p[c] - ts[c].mean) * gains[c] + rs[c].mean);
                    lab_to_rgb([lab[0].clamp(0.0, 100.0), lab[1], lab[2]])
                })
                .collect()
        }
        Method::RgbStats => {
            let ts = channel_stats(&orig);
            let rs = channel_stats(&refr);
            let gains = [0, 1, 2].map(|c| std_ratio(ts[c].std, rs[c].std));
            orig.iter()
                .map(|p| {
                    [0, 1, 2].map(|c| {
                        ((p[c] - ts[c].mean) * gains[c] + rs[c].mean).clamp(0.0, 255.0)
                    })
                })
                .collect()
        }
        Method::Histogram => {
            let luts = histogram_luts(&orig, &refr);
            orig.iter()
                .map(|p| {
                    [0, 1, 2].map(|c| luts[c][p[c].round().clamp(0.0, 255.0) as usize] as f32)
                })
                .collect()
        }
    };

    // Post-passes shared by every method: keep the original lightness, and/or scale
    // the result's chroma. Both live in CIELAB.
    let sat = (opts.saturation / 100.0) as f32;
    if opts.preserve_luminance || (sat - 1.0).abs() > 1e-6 {
        for (o, orig_px) in out.iter_mut().zip(orig.iter()) {
            let mut lab = rgb_to_lab(*o);
            if opts.preserve_luminance {
                lab[0] = rgb_to_lab(*orig_px)[0];
            }
            lab[1] *= sat;
            lab[2] *= sat;
            *o = if opts.preserve_luminance {
                lab_to_rgb_keep_lightness(lab)
            } else {
                lab_to_rgb(lab)
            };
        }
    }

    // Strength blends the recoloured pixels back over the untouched original.
    let s = (opts.strength.clamp(0.0, 100.0) / 100.0) as f32;
    if s < 1.0 {
        for (o, orig_px) in out.iter_mut().zip(orig.iter()) {
            for c in 0..3 {
                o[c] = orig_px[c] * (1.0 - s) + o[c] * s;
            }
        }
    }

    let mut img = base;
    for (px, o) in img.pixels_mut().zip(out.iter()) {
        for c in 0..3 {
            px.0[c] = o[c].round().clamp(0.0, 255.0) as u8;
        }
    }
    encode(img, opts.format, opts.quality)
}

fn encode(img: RgbaImage, format: OutFormat, quality: u8) -> Result<Vec<u8>, String> {
    let mut buf = Cursor::new(Vec::new());
    match format {
        OutFormat::Png => DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .map_err(|e| format!("PNG encode failed: {e}"))?,
        OutFormat::Jpeg => {
            // JPEG has no alpha; drop it (transparent areas become opaque black).
            let rgb = DynamicImage::ImageRgba8(img).to_rgb8();
            let q = quality.clamp(1, 100);
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, q);
            enc.encode_image(&DynamicImage::ImageRgb8(rgb))
                .map_err(|e| format!("JPEG encode failed: {e}"))?;
        }
    }
    Ok(buf.into_inner())
}

/// Decode both raw image byte buffers, then run the transfer.
pub fn transfer_from_bytes(
    target: &[u8],
    reference: &[u8],
    opts: Options,
) -> Result<Vec<u8>, String> {
    let target = image::load_from_memory(target)
        .map_err(|e| format!("target image (first) could not be decoded: {e}"))?;
    let reference = image::load_from_memory(reference)
        .map_err(|e| format!("reference image (second) could not be decoded: {e}"))?;
    transfer(target, reference, opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn solid(w: u32, h: u32, c: [u8; 4]) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, Rgba(c)))
    }

    /// Horizontal gradient from `a` to `b` — gives a channel real variance.
    fn gradient(w: u32, h: u32, a: [u8; 4], b: [u8; 4]) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for x in 0..w {
            let t = if w > 1 { x as f32 / (w - 1) as f32 } else { 0.0 };
            let px = Rgba([
                (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t).round() as u8,
                (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t).round() as u8,
                (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t).round() as u8,
                a[3],
            ]);
            for y in 0..h {
                img.put_pixel(x, y, px);
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    fn png_bytes(img: &DynamicImage) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    fn decode(bytes: &[u8]) -> RgbaImage {
        image::load_from_memory(bytes).unwrap().to_rgba8()
    }

    fn mean_rgb(img: &RgbaImage) -> [f32; 3] {
        let n = (img.width() * img.height()) as f32;
        let mut s = [0.0f32; 3];
        for p in img.pixels() {
            for c in 0..3 {
                s[c] += p.0[c] as f32;
            }
        }
        [s[0] / n, s[1] / n, s[2] / n]
    }

    #[test]
    fn parse_enums() {
        assert_eq!(parse_method("").unwrap(), Method::LabStats);
        assert_eq!(parse_method("Reinhard").unwrap(), Method::LabStats);
        assert_eq!(parse_method("rgb-stats").unwrap(), Method::RgbStats);
        assert_eq!(parse_method("histogram").unwrap(), Method::Histogram);
        assert_eq!(parse_method("mean-only").unwrap(), Method::MeanOnly);
        assert!(parse_method("magic").unwrap_err().contains("lab-stats"));
        assert_eq!(parse_format("jpg").unwrap(), OutFormat::Jpeg);
        assert!(parse_format("tiff").unwrap_err().contains("png|jpeg"));
    }

    #[test]
    fn lab_round_trips_through_rgb() {
        for c in [[10.0, 200.0, 90.0], [0.0, 0.0, 0.0], [255.0, 255.0, 255.0]] {
            let back = lab_to_rgb(rgb_to_lab(c));
            for i in 0..3 {
                assert!((back[i] - c[i]).abs() < 0.6, "{c:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn keep_lightness_desaturates_instead_of_clipping() {
        // An impossibly saturated dark red: hard clipping brightens it, the
        // gamut-mapped conversion keeps L and drops only the chroma it can't show.
        let lab = [16.0, 90.0, 60.0];
        let clipped = rgb_to_lab(lab_to_rgb(lab))[0];
        let mapped = rgb_to_lab(lab_to_rgb_keep_lightness(lab))[0];
        assert!(clipped > lab[0] + 3.0, "expected clipping to brighten, got {clipped}");
        assert!((mapped - lab[0]).abs() < 1.0, "lightness not kept: {mapped}");
        // In-gamut colours are untouched by the mapping.
        let ok = [55.0, 12.0, -20.0];
        assert_eq!(lab_to_rgb_keep_lightness(ok), lab_to_rgb(ok));
    }

    #[test]
    fn output_keeps_target_dimensions_not_reference() {
        let out = transfer_from_bytes(
            &png_bytes(&gradient(40, 20, [10, 10, 10, 255], [200, 200, 200, 255])),
            &png_bytes(&solid(7, 3, [200, 40, 40, 255])),
            Options::default(),
        )
        .unwrap();
        let img = decode(&out);
        assert_eq!((img.width(), img.height()), (40, 20));
    }

    #[test]
    fn lab_stats_moves_mean_toward_reference() {
        let target = gradient(32, 32, [20, 20, 20, 255], [220, 220, 220, 255]);
        let reference = gradient(32, 32, [120, 20, 20, 255], [255, 90, 60, 255]);
        let out = decode(
            &transfer_from_bytes(
                &png_bytes(&target),
                &png_bytes(&reference),
                Options::default(),
            )
            .unwrap(),
        );
        let before = mean_rgb(&target.to_rgba8());
        let after = mean_rgb(&out);
        let want = mean_rgb(&reference.to_rgba8());
        // The neutral grey target picks up the reference's warm cast: red rises,
        // blue falls, and every channel mean lands near the reference's.
        assert!(after[0] > before[0], "red should rise: {before:?} -> {after:?}");
        assert!(after[2] < before[2], "blue should fall: {before:?} -> {after:?}");
        for c in 0..3 {
            assert!(
                (after[c] - want[c]).abs() < 12.0,
                "channel {c}: {after:?} vs reference {want:?}"
            );
        }
    }

    #[test]
    fn strength_zero_returns_the_original_pixels() {
        let target = gradient(16, 8, [30, 60, 90, 255], [200, 180, 160, 255]);
        let out = decode(
            &transfer_from_bytes(
                &png_bytes(&target),
                &png_bytes(&solid(8, 8, [255, 0, 0, 255])),
                Options { strength: 0.0, ..Options::default() },
            )
            .unwrap(),
        );
        assert_eq!(out.as_raw(), target.to_rgba8().as_raw());
    }

    #[test]
    fn strength_half_lands_between_original_and_full() {
        let t = png_bytes(&gradient(16, 8, [30, 30, 30, 255], [210, 210, 210, 255]));
        let r = png_bytes(&gradient(16, 8, [140, 20, 20, 255], [255, 120, 90, 255]));
        let full = mean_rgb(&decode(&transfer_from_bytes(&t, &r, Options::default()).unwrap()));
        let half = mean_rgb(&decode(
            &transfer_from_bytes(&t, &r, Options { strength: 50.0, ..Options::default() })
                .unwrap(),
        ));
        let orig = mean_rgb(&decode(&t));
        for c in 0..3 {
            let mid = (orig[c] + full[c]) / 2.0;
            assert!((half[c] - mid).abs() < 2.0, "channel {c}: {half:?} vs mid {mid}");
        }
    }

    #[test]
    fn preserve_luminance_keeps_target_lightness() {
        let t = png_bytes(&gradient(24, 8, [40, 40, 40, 255], [200, 200, 200, 255]));
        let r = png_bytes(&solid(8, 8, [230, 60, 20, 255]));
        let out = decode(
            &transfer_from_bytes(
                &t,
                &r,
                Options { preserve_luminance: true, ..Options::default() },
            )
            .unwrap(),
        );
        let orig = decode(&t);
        for (a, b) in orig.pixels().zip(out.pixels()) {
            let la = rgb_to_lab([a.0[0] as f32, a.0[1] as f32, a.0[2] as f32])[0];
            let lb = rgb_to_lab([b.0[0] as f32, b.0[1] as f32, b.0[2] as f32])[0];
            assert!((la - lb).abs() < 1.5, "lightness drifted {la} -> {lb}");
        }
    }

    #[test]
    fn saturation_zero_is_greyscale() {
        let out = decode(
            &transfer_from_bytes(
                &png_bytes(&gradient(16, 8, [30, 90, 160, 255], [200, 120, 60, 255])),
                &png_bytes(&solid(8, 8, [220, 60, 30, 255])),
                Options { saturation: 0.0, ..Options::default() },
            )
            .unwrap(),
        );
        for p in out.pixels() {
            let (r, g, b) = (p.0[0] as i32, p.0[1] as i32, p.0[2] as i32);
            assert!(
                (r - g).abs() <= 2 && (g - b).abs() <= 2,
                "expected neutral pixel, got {:?}",
                p.0
            );
        }
    }

    #[test]
    fn mean_only_keeps_target_contrast() {
        let t = png_bytes(&gradient(64, 8, [20, 20, 20, 255], [230, 230, 230, 255]));
        // A flat reference has zero spread: lab-stats collapses contrast, mean-only must not.
        let r = png_bytes(&solid(16, 16, [120, 90, 70, 255]));
        let spread = |img: &RgbaImage| {
            let px: Vec<[f32; 3]> = img
                .pixels()
                .map(|p| rgb_to_lab([p.0[0] as f32, p.0[1] as f32, p.0[2] as f32]))
                .collect();
            channel_stats(&px)[0].std
        };
        let before = spread(&decode(&t));
        let mean_only = spread(&decode(
            &transfer_from_bytes(&t, &r, Options { method: Method::MeanOnly, ..Options::default() })
                .unwrap(),
        ));
        let lab_stats = spread(&decode(&transfer_from_bytes(&t, &r, Options::default()).unwrap()));
        assert!((mean_only - before).abs() < 3.0, "mean-only changed spread {before} -> {mean_only}");
        assert!(lab_stats < before / 2.0, "lab-stats should flatten a flat reference");
    }

    #[test]
    fn histogram_matches_the_reference_distribution() {
        let t = png_bytes(&gradient(64, 4, [0, 0, 0, 255], [255, 255, 255, 255]));
        let r = png_bytes(&gradient(64, 4, [60, 0, 0, 255], [120, 40, 20, 255]));
        let out = decode(
            &transfer_from_bytes(&t, &r, Options { method: Method::Histogram, ..Options::default() })
                .unwrap(),
        );
        let after = mean_rgb(&out);
        let want = mean_rgb(&decode(&r));
        for c in 0..3 {
            assert!((after[c] - want[c]).abs() < 6.0, "channel {c}: {after:?} vs {want:?}");
        }
    }

    #[test]
    fn rgb_stats_matches_channel_means() {
        let t = png_bytes(&gradient(32, 8, [10, 10, 10, 255], [200, 200, 200, 255]));
        let r = png_bytes(&gradient(32, 8, [30, 90, 150, 255], [90, 160, 230, 255]));
        let out = decode(
            &transfer_from_bytes(&t, &r, Options { method: Method::RgbStats, ..Options::default() })
                .unwrap(),
        );
        let after = mean_rgb(&out);
        let want = mean_rgb(&decode(&r));
        for c in 0..3 {
            assert!((after[c] - want[c]).abs() < 3.0, "channel {c}: {after:?} vs {want:?}");
        }
    }

    #[test]
    fn alpha_is_preserved() {
        let mut img = RgbaImage::from_pixel(8, 4, Rgba([120, 130, 140, 255]));
        img.put_pixel(0, 0, Rgba([10, 20, 30, 0]));
        img.put_pixel(1, 0, Rgba([10, 20, 30, 77]));
        let t = png_bytes(&DynamicImage::ImageRgba8(img));
        let out = decode(
            &transfer_from_bytes(&t, &png_bytes(&solid(4, 4, [200, 30, 30, 255])), Options::default())
                .unwrap(),
        );
        assert_eq!(out.get_pixel(0, 0).0[3], 0);
        assert_eq!(out.get_pixel(1, 0).0[3], 77);
        assert_eq!(out.get_pixel(2, 0).0[3], 255);
    }

    #[test]
    fn flat_target_channel_does_not_explode() {
        // Zero-variance target: the std ratio must fall back to 1.0, not divide by 0.
        let out = decode(
            &transfer_from_bytes(
                &png_bytes(&solid(8, 8, [128, 128, 128, 255])),
                &png_bytes(&gradient(16, 4, [10, 200, 10, 255], [240, 20, 200, 255])),
                Options::default(),
            )
            .unwrap(),
        );
        for p in out.pixels() {
            assert!(p.0[..3].iter().all(|c| *c < 255 || *c == 255));
        }
        let m = mean_rgb(&out);
        assert!(m.iter().all(|c| c.is_finite()), "non-finite output {m:?}");
    }

    #[test]
    fn jpeg_output_is_valid_jpeg() {
        let out = transfer_from_bytes(
            &png_bytes(&gradient(16, 16, [20, 40, 60, 255], [200, 180, 160, 255])),
            &png_bytes(&solid(8, 8, [220, 100, 40, 255])),
            Options { format: OutFormat::Jpeg, quality: 70, ..Options::default() },
        )
        .unwrap();
        assert_eq!(&out[..2], &[0xFF, 0xD8], "JPEG SOI marker");
        assert_eq!(
            image::guess_format(&out).unwrap(),
            ImageFormat::Jpeg,
            "decodable as JPEG"
        );
    }

    #[test]
    fn jpeg_quality_changes_file_size() {
        let t = png_bytes(&gradient(64, 64, [10, 90, 160, 255], [240, 200, 120, 255]));
        let r = png_bytes(&solid(16, 16, [200, 90, 40, 255]));
        let low = transfer_from_bytes(
            &t,
            &r,
            Options { format: OutFormat::Jpeg, quality: 20, ..Options::default() },
        )
        .unwrap();
        let high = transfer_from_bytes(
            &t,
            &r,
            Options { format: OutFormat::Jpeg, quality: 95, ..Options::default() },
        )
        .unwrap();
        assert!(high.len() > low.len(), "q95 {} !> q20 {}", high.len(), low.len());
    }

    #[test]
    fn rejects_garbage_bytes() {
        let good = png_bytes(&solid(4, 4, [1, 2, 3, 255]));
        let err = transfer_from_bytes(b"not an image", &good, Options::default()).unwrap_err();
        assert!(err.contains("target image (first)"), "{err}");
        let err = transfer_from_bytes(&good, b"not an image", Options::default()).unwrap_err();
        assert!(err.contains("reference image (second)"), "{err}");
    }
}
