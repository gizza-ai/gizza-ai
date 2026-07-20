//! gizza-ai/document-skew-detector core — estimate the skew (rotation) angle of a
//! scanned document or text image so it can be deskewed.
//!
//! No wafer/wasm-bindgen deps. Pure-Rust `image` crate. The algorithm is the
//! classic projection-profile method (the same family ImageMagick's -deskew and
//! Leptonica's sweep-search use), which keys on TEXT LINES rather than long
//! straight edges — so it works on pages that have no ruled lines at all:
//!   1. decode + downscale (cap the longest side) + convert to grayscale;
//!   2. binarize into ink/paper (Otsu by default, or an explicit brightness
//!      threshold); auto-invert white-on-black scans; trim a small border margin
//!      so scanner-bed shadows don't vote;
//!   3. for candidate angles θ, project every ink pixel onto rotated rows
//!      (r = y·cosθ − x·sinθ) and score the profile's sum of squared bin counts
//!      — maximal when text lines align with the projection rows;
//!   4. coarse 0.5° sweep over ±`max_angle`, then a 0.05° fine sweep around the
//!      coarse peak, then a parabolic refine → 0.01° reporting resolution.
//!
//! Sign convention (matches image-horizon-tilt-checker / rotate-image): a
//! positive `angle` means the page is skewed CLOCKWISE (text baselines' right
//! end sits lower on screen). To deskew, rotate by `suggested_rotation = -angle`
//! degrees (positive = clockwise).

use std::io::Cursor;

use image::{ColorType, DynamicImage, GrayImage, ImageDecoder, ImageReader};

/// Longest side after downscale — bounds work + memory for phone-camera scans.
const MAX_SIDE: u32 = 1400;
/// Decode-memory budget: input bytes + decoded raster (+ any grayscale copy)
/// must fit alongside the runtime in the 64 MiB wasm sandbox.
const MEM_BUDGET: u64 = 48 * 1024 * 1024;
/// Fraction of each border trimmed before analysis (scanner-bed shadow guard).
const MARGIN_FRAC: f64 = 0.03;
/// Coarse sweep step (degrees).
const COARSE_STEP: f64 = 0.5;
/// Fine sweep step (degrees) around the coarse peak.
const FINE_STEP: f64 = 0.05;
/// Minimum ink pixels needed for a meaningful estimate.
const MIN_INK_PIXELS: u64 = 200;
/// Maximum vertical ink-run length (px, at ≤`MAX_SIDE` scale) for a pixel to
/// count as text-like — filters solid regions out of the projection profile.
const MAX_RUN: u32 = 10;

/// The detection report.
#[derive(Debug, Clone, PartialEq)]
pub struct SkewResult {
    /// Detected skew of the text lines, in degrees. Positive = clockwise
    /// (right end of the lines sits lower). 0 when perfectly straight.
    pub angle: f64,
    /// Rotation to apply to deskew the page, in degrees (positive = clockwise).
    /// Equal to `-angle`.
    pub suggested_rotation: f64,
    /// "straight", "clockwise", "counterclockwise", or "undetermined".
    pub direction: &'static str,
    /// Whether |angle| is within `tolerance` of straight.
    pub is_straight: bool,
    /// 0..1 — how sharply the best angle's alignment score stands out from the
    /// sweep average. Text pages with clean lines score high; noise scores low.
    pub confidence: f64,
    /// The effective binarization threshold used, as a 0-255 gray level.
    pub threshold_used: u8,
    /// Number of ink (text) pixels that voted, after downscale + margin trim.
    pub ink_pixels: u64,
}

/// Round to 2 decimals; collapse -0.0 to 0.0.
fn round2(x: f64) -> f64 {
    let r = (x * 100.0).round() / 100.0;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Otsu's threshold over a 256-bin histogram.
fn otsu(hist: &[u64; 256], total: u64) -> u8 {
    let sum_all: f64 = hist.iter().enumerate().map(|(i, &c)| i as f64 * c as f64).sum();
    let mut sum_b = 0.0f64;
    let mut w_b = 0.0f64;
    let mut best_t = 127u8;
    let mut best_var = -1.0f64;
    let total = total as f64;
    for t in 0..256usize {
        w_b += hist[t] as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0.0 {
            break;
        }
        sum_b += t as f64 * hist[t] as f64;
        let m_b = sum_b / w_b;
        let m_f = (sum_all - sum_b) / w_f;
        let between = w_b * w_f * (m_b - m_f) * (m_b - m_f);
        if between > best_var {
            best_var = between;
            best_t = t as u8;
        }
    }
    best_t
}

/// Score one candidate angle: project every ink pixel onto rotated rows with
/// linear-interpolated (anti-aliased) binning, then score the profile's sum of
/// squared bin counts, normalized by ink count so it's comparable across
/// angles. Text lines aligned with the projection direction concentrate ink
/// into few bins → high score.
fn profile_score(ink: &[(u16, u16)], theta_deg: f64, diag: f64) -> f64 {
    let t = theta_deg.to_radians();
    let (sin_t, cos_t) = (t.sin() as f32, t.cos() as f32);
    // r spans at most the image diagonal; offset keeps indices non-negative.
    let bins = diag.ceil() as usize * 2 + 4;
    let offset = diag as f32 + 2.0;
    let mut hist = vec![0.0f32; bins];
    for &(x, y) in ink {
        let r = y as f32 * cos_t - x as f32 * sin_t + offset;
        let idx = r as usize; // r is guaranteed positive
        let frac = r - idx as f32;
        if idx + 1 < bins {
            hist[idx] += 1.0 - frac;
            hist[idx + 1] += frac;
        }
    }
    let mut s = 0.0f64;
    for &c in &hist {
        s += (c as f64) * (c as f64);
    }
    s / ink.len() as f64
}

/// Streaming luma conversion + integer box downscale (factor `k`) from a raw
/// 8-bit interleaved buffer (`ch` = 1, 2, 3 or 4 channels; alpha ignored).
/// Allocates only the output image plus one row of accumulators, so a 25-MP
/// scan never needs a second full-size copy. `k == 1` is a plain luma pass.
fn gray_box_downscale(raw: &[u8], w: u32, h: u32, ch: usize, k: u32) -> GrayImage {
    let ow = (w / k).max(1);
    let oh = (h / k).max(1);
    let mut out = GrayImage::new(ow, oh);
    let ks = k as usize;
    let stride = w as usize * ch;
    let mut acc = vec![0u32; ow as usize];
    for oy in 0..oh as usize {
        acc.fill(0);
        for iy in oy * ks..(oy + 1) * ks {
            let row = &raw[iy * stride..(iy + 1) * stride];
            for ox in 0..ow as usize {
                let mut sum = 0u32;
                for ix in ox * ks..(ox + 1) * ks {
                    let p = &row[ix * ch..];
                    // Rec. 601 luma for color; channel 0 for gray/gray-alpha.
                    sum += if ch >= 3 {
                        (299 * u32::from(p[0]) + 587 * u32::from(p[1]) + 114 * u32::from(p[2]))
                            / 1000
                    } else {
                        u32::from(p[0])
                    };
                }
                acc[ox] += sum;
            }
        }
        let norm = (ks * ks) as u32;
        for ox in 0..ow as usize {
            out.put_pixel(ox as u32, oy as u32, image::Luma([(acc[ox] / norm) as u8]));
        }
    }
    out
}

/// Collect TEXT-LIKE ink pixels inside `(x0, x1, y0, y1)`: ink pixels whose
/// vertical ink run is at most `MAX_RUN` px. Text strokes and line bands are
/// vertically thin; solid regions (scanner-bed background, photos, filled
/// logos) have long vertical runs and would otherwise drown the text signal in
/// the projection profile — only their thin top/bottom boundary bands survive,
/// which lie along the same skew angle anyway. Two passes (count, then fill)
/// size the Vec exactly — no amortized doubling spikes in the 64 MiB sandbox.
fn collect_text_ink(
    gray: &image::GrayImage,
    (x0, x1, y0, y1): (u32, u32, u32, u32),
    thr: u8,
    inverted: bool,
) -> Vec<(u16, u16)> {
    let is_ink = |x: u32, y: u32| {
        let v = gray.get_pixel(x, y).0[0];
        if inverted {
            v > thr
        } else {
            v <= thr
        }
    };
    let mut count: usize = 0;
    let mut ink: Vec<(u16, u16)> = Vec::new();
    for pass in 0..2u8 {
        if pass == 1 {
            ink.reserve_exact(count);
        }
        for x in x0..x1 {
            let mut run_start: Option<u32> = None;
            for y in y0..=y1 {
                let ink_here = y < y1 && is_ink(x, y);
                if ink_here {
                    if run_start.is_none() {
                        run_start = Some(y);
                    }
                } else if let Some(s) = run_start.take() {
                    if y - s <= MAX_RUN {
                        if pass == 0 {
                            count += (y - s) as usize;
                        } else {
                            for yy in s..y {
                                ink.push((x as u16, yy as u16));
                            }
                        }
                    }
                }
            }
        }
    }
    ink
}

/// Detect the skew angle of the document image in `bytes`.
///
/// * `threshold` — ink/paper split as a brightness percentage 0-99: pixels
///   darker than `threshold`% of full white count as ink. 0 = automatic (Otsu).
/// * `max_angle` — only skews within ±this many degrees are searched (1..=45).
/// * `tolerance` — |angle| at or below this (degrees, 0..=10) reports
///   `is_straight = true`.
pub fn detect(
    bytes: &[u8],
    threshold: f64,
    max_angle: f64,
    tolerance: f64,
) -> Result<SkewResult, String> {
    if !(0.0..=99.0).contains(&threshold) || threshold.fract() != 0.0 {
        return Err(format!(
            "threshold must be a whole number between 0 (auto) and 99, got {threshold}"
        ));
    }
    if !(1.0..=45.0).contains(&max_angle) {
        return Err(format!("max_angle must be between 1 and 45 degrees, got {max_angle}"));
    }
    if !(0.0..=10.0).contains(&tolerance) {
        return Err(format!("tolerance must be between 0 and 10 degrees, got {tolerance}"));
    }

    // Decode header-first so oversized rasters are rejected with a clear error
    // BEFORE allocating them (the wasm sandbox has 64 MiB total; a 25-MP scan
    // decodes fine, a 25-MP RGBA photo would not).
    let decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read image data: {e}"))?
        .into_decoder()
        .map_err(|e| format!("could not decode image (PNG, JPEG, WebP, GIF or BMP expected): {e}"))?;
    let (w0, h0) = decoder.dimensions();
    if w0 < 16 || h0 < 16 {
        return Err(format!(
            "image too small to analyze: {w0}x{h0} (need at least 16x16 pixels)"
        ));
    }
    let ct = decoder.color_type();
    let decoded_bytes = decoder.total_bytes();
    // 8-bit rasters stream straight into the downscaled grayscale; other color
    // types (16-bit/float) go through a full-size grayscale copy first.
    let eight_bit = matches!(
        ct,
        ColorType::L8 | ColorType::La8 | ColorType::Rgb8 | ColorType::Rgba8
    );
    let gray_copy = if eight_bit { 0 } else { u64::from(w0) * u64::from(h0) };
    if bytes.len() as u64 + decoded_bytes + gray_copy > MEM_BUDGET {
        let mp = f64::from(w0) * f64::from(h0) / 1.0e6;
        return Err(format!(
            "image too large to analyze in the sandbox: {w0}x{h0} ({mp:.1} megapixels, ~{} MB decoded); re-export the scan at a lower resolution (300 DPI is plenty) or as grayscale",
            (decoded_bytes + gray_copy) / (1024 * 1024)
        ));
    }
    // Box-downscale by an integer factor so the longest side is ≤ MAX_SIDE —
    // streaming, so no full-size intermediate beyond the decoded raster.
    let k = w0.max(h0).div_ceil(MAX_SIDE).max(1);
    let gray: GrayImage = if eight_bit {
        let mut raw = vec![0u8; decoded_bytes as usize];
        decoder
            .read_image(&mut raw)
            .map_err(|e| format!("could not decode image: {e}"))?;
        gray_box_downscale(&raw, w0, h0, usize::from(ct.channel_count()), k)
    } else {
        let img = DynamicImage::from_decoder(decoder)
            .map_err(|e| format!("could not decode image: {e}"))?;
        let g = img.into_luma8();
        if k > 1 {
            gray_box_downscale(g.as_raw(), w0, h0, 1, k)
        } else {
            g
        }
    };
    let (w, h) = gray.dimensions();

    // Trim a small border margin so scanner-bed shadows / page edges don't vote.
    let mx = ((w as f64 * MARGIN_FRAC) as u32).min(w / 4);
    let my = ((h as f64 * MARGIN_FRAC) as u32).min(h / 4);
    let (x0, x1) = (mx, w - mx);
    let (y0, y1) = (my, h - my);

    // Histogram over the trimmed region → threshold.
    let mut hist = [0u64; 256];
    let mut total = 0u64;
    for y in y0..y1 {
        for x in x0..x1 {
            hist[gray.get_pixel(x, y).0[0] as usize] += 1;
            total += 1;
        }
    }
    let thr: u8 = if threshold == 0.0 {
        otsu(&hist, total)
    } else {
        ((threshold / 100.0) * 255.0).round() as u8
    };

    // Ink = darker than thr. If that selects the majority of pixels, the scan is
    // white-on-black (inverted) — flip the polarity so text stays the minority.
    let dark: u64 = hist.iter().take(thr as usize + 1).sum();
    let inverted = dark * 2 > total;

    let ink = collect_text_ink(&gray, (x0, x1, y0, y1), thr, inverted);
    let ink_pixels = ink.len() as u64;

    if ink_pixels < MIN_INK_PIXELS {
        return Ok(SkewResult {
            angle: 0.0,
            suggested_rotation: 0.0,
            direction: "undetermined",
            is_straight: false,
            confidence: 0.0,
            threshold_used: thr,
            ink_pixels,
        });
    }

    let diag = ((w * w + h * h) as f64).sqrt();

    // Coarse sweep.
    let mut best_theta = 0.0f64;
    let mut best_score = f64::MIN;
    let mut sum_score = 0.0f64;
    let mut n_score = 0u32;
    let steps = (2.0 * max_angle / COARSE_STEP).round() as i64;
    for i in 0..=steps {
        let theta = -max_angle + i as f64 * COARSE_STEP;
        let s = profile_score(&ink, theta, diag);
        sum_score += s;
        n_score += 1;
        if s > best_score {
            best_score = s;
            best_theta = theta;
        }
    }
    let mean_score = sum_score / n_score as f64;

    // Fine sweep around the coarse peak (clamped to the search window).
    let lo = (best_theta - COARSE_STEP).max(-max_angle);
    let hi = (best_theta + COARSE_STEP).min(max_angle);
    let mut fine: Vec<(f64, f64)> = Vec::new();
    let fine_steps = ((hi - lo) / FINE_STEP).round() as i64;
    for i in 0..=fine_steps {
        let theta = lo + i as f64 * FINE_STEP;
        let s = profile_score(&ink, theta, diag);
        fine.push((theta, s));
        if s > best_score {
            best_score = s;
            best_theta = theta;
        }
    }

    // Parabolic refine over the fine peak's neighbors → sub-step resolution.
    let mut angle = best_theta;
    if let Some(k) = fine.iter().position(|&(t, _)| t == best_theta) {
        if k > 0 && k + 1 < fine.len() {
            let (s_l, s_c, s_r) = (fine[k - 1].1, fine[k].1, fine[k + 1].1);
            let denom = s_l - 2.0 * s_c + s_r;
            if denom.abs() > f64::EPSILON {
                let delta = 0.5 * (s_l - s_r) / denom;
                if delta.abs() <= 1.0 {
                    angle = best_theta + delta * FINE_STEP;
                }
            }
        }
    }
    angle = angle.clamp(-max_angle, max_angle);

    // Confidence: how far the peak stands above the sweep mean. A page of
    // aligned text peaks several× the mean; unstructured noise stays near it.
    let confidence = if mean_score > 0.0 {
        (1.0 - mean_score / best_score).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let confidence = (confidence * 100.0).round() / 100.0;

    // A peak indistinguishable from the mean means no line structure was found.
    if best_score <= mean_score * 1.02 {
        return Ok(SkewResult {
            angle: 0.0,
            suggested_rotation: 0.0,
            direction: "undetermined",
            is_straight: false,
            confidence: 0.0,
            threshold_used: thr,
            ink_pixels,
        });
    }

    let angle = round2(angle);
    let is_straight = angle.abs() <= tolerance;
    let direction = if is_straight {
        "straight"
    } else if angle > 0.0 {
        "clockwise"
    } else {
        "counterclockwise"
    };
    Ok(SkewResult {
        angle,
        suggested_rotation: round2(-angle),
        direction,
        is_straight,
        confidence,
        threshold_used: thr,
        ink_pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GrayImage, ImageFormat, Luma};
    use std::io::Cursor;

    /// Render a synthetic "scanned page": white paper with dark dashed text
    /// lines skewed by `deg` degrees (positive = clockwise, right end lower).
    /// Dashes emulate words; 3px stroke emulates a text x-height band.
    fn skewed_page(w: u32, h: u32, deg: f64, fg: u8, bg: u8) -> GrayImage {
        let mut img = GrayImage::from_pixel(w, h, Luma([bg]));
        let slope = deg.to_radians().tan();
        let line_gap = 28.0;
        let cx = w as f64 / 2.0;
        let mut base = 40.0;
        while base < h as f64 - 40.0 {
            let mut x = 30u32;
            while x < w - 30 {
                let dash = 14 + (x % 23); // vary word lengths
                for dx in 0..dash {
                    let xx = x + dx;
                    if xx >= w - 30 {
                        break;
                    }
                    let y = base + (xx as f64 - cx) * slope;
                    for t in 0..3 {
                        let yy = y as i64 + t;
                        if yy >= 0 && (yy as u32) < h {
                            img.put_pixel(xx, yy as u32, Luma([fg]));
                        }
                    }
                }
                x += dash + 8; // word gap
            }
            base += line_gap;
        }
        img
    }

    fn encode(img: GrayImage, fmt: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageLuma8(img).write_to(&mut out, fmt).unwrap();
        out.into_inner()
    }

    fn page_png(deg: f64) -> Vec<u8> {
        encode(skewed_page(600, 800, deg, 20, 245), ImageFormat::Png)
    }

    #[test]
    fn straight_page_reads_zero() {
        let r = detect(&page_png(0.0), 0.0, 15.0, 0.5).unwrap();
        assert!(r.angle.abs() <= 0.1, "expected ~0, got {}", r.angle);
        assert!(r.is_straight);
        assert_eq!(r.direction, "straight");
        assert!(r.confidence > 0.5, "confidence {}", r.confidence);
        assert!(r.ink_pixels > MIN_INK_PIXELS);
    }

    #[test]
    fn clockwise_skew_is_positive() {
        let r = detect(&page_png(4.0), 0.0, 15.0, 0.5).unwrap();
        assert!((r.angle - 4.0).abs() <= 0.2, "expected ~+4, got {}", r.angle);
        assert_eq!(r.direction, "clockwise");
        assert!((r.suggested_rotation + 4.0).abs() <= 0.2);
        assert!(!r.is_straight);
        assert!(r.confidence > 0.5, "confidence {}", r.confidence);
    }

    #[test]
    fn counterclockwise_skew_is_negative() {
        let r = detect(&page_png(-7.5), 0.0, 15.0, 0.5).unwrap();
        assert!((r.angle + 7.5).abs() <= 0.2, "expected ~-7.5, got {}", r.angle);
        assert_eq!(r.direction, "counterclockwise");
        assert!((r.suggested_rotation - 7.5).abs() <= 0.2);
    }

    #[test]
    fn large_skew_found_with_wider_window() {
        let r = detect(&page_png(20.0), 0.0, 30.0, 0.5).unwrap();
        assert!((r.angle - 20.0).abs() <= 0.3, "expected ~+20, got {}", r.angle);
    }

    #[test]
    fn sub_degree_skew_resolved() {
        let r = detect(&page_png(1.3), 0.0, 15.0, 0.5).unwrap();
        assert!((r.angle - 1.3).abs() <= 0.15, "expected ~+1.3, got {}", r.angle);
        assert!(!r.is_straight, "1.3 deg is outside the 0.5 deg tolerance");
    }

    #[test]
    fn explicit_threshold_matches_auto_on_clean_page() {
        // 50% of 255 splits ink (20) from paper (245) exactly like Otsu here.
        let r = detect(&page_png(3.0), 50.0, 15.0, 0.5).unwrap();
        assert!((r.angle - 3.0).abs() <= 0.2, "expected ~+3, got {}", r.angle);
        assert_eq!(r.threshold_used, 128);
    }

    #[test]
    fn inverted_scan_auto_detected() {
        // White text on black paper (photo negative / blueprint scan).
        let img = skewed_page(600, 800, 5.0, 235, 10);
        let r = detect(&encode(img, ImageFormat::Png), 0.0, 15.0, 0.5).unwrap();
        assert!((r.angle - 5.0).abs() <= 0.2, "expected ~+5, got {}", r.angle);
    }

    #[test]
    fn jpeg_input_supported() {
        let img = skewed_page(600, 800, 2.0, 20, 245);
        let r = detect(&encode(img, ImageFormat::Jpeg), 0.0, 15.0, 0.5).unwrap();
        assert!((r.angle - 2.0).abs() <= 0.25, "expected ~+2, got {}", r.angle);
    }

    #[test]
    fn blank_page_is_undetermined() {
        let img = GrayImage::from_pixel(400, 400, Luma([250]));
        let r = detect(&encode(img, ImageFormat::Png), 0.0, 15.0, 0.5).unwrap();
        assert_eq!(r.direction, "undetermined");
        assert_eq!(r.angle, 0.0);
        assert_eq!(r.confidence, 0.0);
        assert!(r.ink_pixels < MIN_INK_PIXELS);
    }

    #[test]
    fn skew_beyond_window_clamps_to_window_edge_not_junk() {
        // 12 deg page searched within ±5: must not report something outside ±5.
        let r = detect(&page_png(12.0), 0.0, 5.0, 0.5).unwrap();
        assert!(r.angle.abs() <= 5.0, "angle {} escaped the window", r.angle);
    }

    #[test]
    fn rejects_bad_bytes() {
        let err = detect(b"not an image", 0.0, 15.0, 0.5).unwrap_err();
        assert!(err.contains("could not decode image"), "{err}");
    }

    #[test]
    fn rejects_tiny_image() {
        let img = GrayImage::from_pixel(8, 8, Luma([255]));
        let err = detect(&encode(img, ImageFormat::Png), 0.0, 15.0, 0.5).unwrap_err();
        assert!(err.contains("too small"), "{err}");
    }

    #[test]
    fn rejects_raster_over_memory_budget() {
        // 5000x5000 RGB decodes to 75 MB > the 48 MB budget; constant color
        // keeps the PNG itself tiny. The header-first check must reject it
        // without decoding.
        let img = image::RgbImage::from_pixel(5000, 5000, image::Rgb([240, 240, 240]));
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        let err = detect(&out.into_inner(), 0.0, 15.0, 0.5).unwrap_err();
        assert!(err.contains("too large to analyze"), "{err}");
        assert!(err.contains("lower resolution"), "{err}");
    }

    #[test]
    fn large_grayscale_scan_fits_budget() {
        // A 3000x4000 grayscale page (12 MB decoded) must pass the budget and
        // downscale (k=3) without changing the detected angle materially.
        let img = skewed_page(3000, 4000, 3.0, 20, 245);
        let r = detect(&encode(img, ImageFormat::Png), 0.0, 15.0, 0.5).unwrap();
        assert!((r.angle - 3.0).abs() <= 0.25, "expected ~+3, got {}", r.angle);
    }

    #[test]
    fn rejects_out_of_range_params() {
        let png = page_png(0.0);
        assert!(detect(&png, -1.0, 15.0, 0.5).is_err());
        assert!(detect(&png, 100.0, 15.0, 0.5).is_err());
        assert!(detect(&png, 12.5, 15.0, 0.5).is_err()); // non-integer threshold
        assert!(detect(&png, 0.0, 0.5, 0.5).is_err()); // max_angle < 1
        assert!(detect(&png, 0.0, 46.0, 0.5).is_err()); // max_angle > 45
        assert!(detect(&png, 0.0, 15.0, -0.1).is_err());
        assert!(detect(&png, 0.0, 15.0, 10.5).is_err());
    }

    #[test]
    fn round2_collapses_negative_zero() {
        assert_eq!(round2(-0.0001), 0.0);
        assert!(round2(-0.0001).is_sign_positive());
        assert_eq!(round2(3.14159), 3.14);
    }
}
