//! gizza-ai/image-blank-detector core — decide whether an image is BLANK: an
//! all-white/all-black page, a single flat fill, a fully transparent canvas, or
//! a frame that is empty apart from a stray watermark. These are the signatures
//! of a render, screenshot or export that silently failed. No wafer/wasm-bindgen
//! deps. Pure-Rust (`image` decode only), so the block runs on every backend
//! including the chat Service Worker. Shared by the chat skill block, the CLI
//! and the unit tests.
//!
//! Approach — a measurable "is everything the same colour?" test:
//!   1. Decode to RGBA and inspect **every** pixel (no sub-sampling — a 4x4 logo
//!      in a corner is exactly the content a sampling detector would miss).
//!   2. Canonicalise near-transparent pixels to a single "empty" value when
//!      `ignore_transparency` is on, so a PNG that stores junk RGB underneath
//!      `alpha = 0` still reads as one flat, empty colour.
//!   3. Quantise into coarse RGBA buckets, take the most populous bucket, then
//!      mean-shift once (average the pixels within `tolerance` of that bucket's
//!      mean) so a noisy near-uniform image that straddles a bucket boundary
//!      still lands on the true fill colour.
//!   4. Count how many pixels sit within `tolerance` of that colour. That share
//!      is `coverage_percent`; at or above `blank_threshold` the image is blank.
//!   5. Report the supporting evidence — distinct colour count, luma range and
//!      standard deviation, and the Shannon entropy of the luma histogram — so
//!      the verdict can be audited instead of trusted.
//!
//! Alpha is part of the comparison, not a filter: a half-transparent image is a
//! two-colour image and is correctly reported as NOT blank.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};

/// Input bytes + decoded raster must fit alongside the runtime in the wasm sandbox.
const MAX_DECODE_BYTES: u64 = 48 * 1024 * 1024;
/// Pixels with alpha below this read as transparent.
pub const ALPHA_THRESHOLD: u8 = 16;
/// Per-channel quantisation step used to pick the CANDIDATE fill bucket. 16 keeps
/// the bucket space at 16^4 = 65536 entries, so memory is bounded for any input.
const BUCKET: u32 = 16;
/// Stop counting distinct colours here; past this an image is plainly detailed
/// and the exact count stops being interesting.
const UNIQUE_COLOR_CAP: usize = 65_536;
/// Channel level at or above which a fill counts as white.
const WHITE_LEVEL: u8 = 250;
/// Channel level at or below which a fill counts as black.
const BLACK_LEVEL: u8 = 5;
/// Luma standard deviation at which an image is considered decisively textured;
/// used only to scale `confidence`.
const DETAIL_STDDEV: f64 = 32.0;

/// The image is a fully transparent canvas.
pub const TRANSPARENT: &str = "transparent";
/// Every pixel is white (within `tolerance`).
pub const ALL_WHITE: &str = "all_white";
/// Every pixel is black (within `tolerance`).
pub const ALL_BLACK: &str = "all_black";
/// Every pixel is the same non-white, non-black colour (within `tolerance`).
pub const SOLID_COLOR: &str = "solid_color";
/// Blank apart from a small amount of content (a watermark, a stray glyph).
pub const NEAR_BLANK: &str = "near_blank";
/// The image carries real content.
pub const NOT_BLANK: &str = "not_blank";

/// The full blank-detection report.
#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
    pub width: u32,
    pub height: u32,
    /// Every pixel is inspected, so this is also the number analysed.
    pub total_pixels: u64,
    pub transparent_pixels: u64,
    pub transparent_percent: f64,
    /// Any pixel has alpha below 255.
    pub has_alpha: bool,
    /// The headline answer: is this image empty?
    pub is_blank: bool,
    /// One of [`TRANSPARENT`], [`ALL_WHITE`], [`ALL_BLACK`], [`SOLID_COLOR`],
    /// [`NEAR_BLANK`], [`NOT_BLANK`].
    pub verdict: &'static str,
    /// Plain-English justification of `verdict`, with the numbers behind it.
    pub reason: String,
    /// How strongly the evidence supports `verdict`, 0-1.
    pub confidence: f64,
    /// `#rrggbb` (lowercase) of the dominant fill colour.
    pub dominant_hex: String,
    /// `#rrggbbaa` (lowercase), including the fill's alpha.
    pub dominant_hex_rgba: String,
    /// CSS `rgb(r, g, b)` of the dominant fill colour.
    pub dominant_rgb: String,
    /// Nearest plain-English colour name, for a human-readable summary.
    pub color_name: &'static str,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
    /// Share of pixels within `tolerance` of the dominant fill colour.
    pub coverage_percent: f64,
    /// Distinct RGBA values, capped at 65536.
    pub unique_colors: u64,
    /// The distinct-colour count hit the cap and is a lower bound.
    pub unique_colors_capped: bool,
    /// `#rrggbb` arithmetic mean of every pixel — differs from the dominant
    /// colour on a detailed image, matches it on a flat one.
    pub mean_hex: String,
    pub luma_min: u8,
    pub luma_max: u8,
    pub luma_mean: f64,
    /// Standard deviation of luma, 0-127.5. Zero on a perfectly flat image.
    pub luma_stddev: f64,
    /// Largest per-channel spread (max - min) across R, G and B, 0-255.
    pub channel_range: u8,
    /// Shannon entropy of the 256-bin luma histogram, 0-8 bits. Zero on a flat
    /// image; a photograph is typically above 6.
    pub entropy: f64,
    pub warnings: Vec<String>,
}

/// Accumulator for one quantised colour bucket.
#[derive(Default, Clone, Copy)]
struct Bucket {
    n: u64,
    sr: u64,
    sg: u64,
    sb: u64,
    sa: u64,
}

impl Bucket {
    fn mean(&self) -> [u8; 4] {
        let n = self.n.max(1) as f64;
        [
            (self.sr as f64 / n).round().clamp(0.0, 255.0) as u8,
            (self.sg as f64 / n).round().clamp(0.0, 255.0) as u8,
            (self.sb as f64 / n).round().clamp(0.0, 255.0) as u8,
            (self.sa as f64 / n).round().clamp(0.0, 255.0) as u8,
        ]
    }
}

fn to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Rec. 709 luma, 0-255.
fn luma(r: u8, g: u8, b: u8) -> f64 {
    0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b)
}

/// A small plain-English colour vocabulary so the report reads naturally.
const NAMES: &[(&str, [u8; 3])] = &[
    ("black", [0, 0, 0]),
    ("white", [255, 255, 255]),
    ("gray", [128, 128, 128]),
    ("light gray", [211, 211, 211]),
    ("dark gray", [64, 64, 64]),
    ("off-white", [245, 245, 240]),
    ("beige", [222, 205, 175]),
    ("brown", [140, 90, 50]),
    ("red", [220, 40, 40]),
    ("maroon", [128, 0, 0]),
    ("orange", [255, 150, 40]),
    ("yellow", [245, 225, 60]),
    ("olive", [128, 128, 0]),
    ("green", [50, 170, 70]),
    ("dark green", [0, 90, 40]),
    ("teal", [0, 128, 128]),
    ("cyan", [80, 210, 220]),
    ("light blue", [150, 200, 235]),
    ("blue", [50, 100, 220]),
    ("navy", [20, 30, 90]),
    ("purple", [130, 70, 180]),
    ("magenta", [220, 60, 200]),
    ("pink", [245, 170, 190]),
];

fn nearest_name(r: u8, g: u8, b: u8) -> &'static str {
    let mut best = ("black", f64::MAX);
    for (name, [nr, ng, nb]) in NAMES {
        let d = (f64::from(r) - f64::from(*nr)).powi(2) * 0.3
            + (f64::from(g) - f64::from(*ng)).powi(2) * 0.59
            + (f64::from(b) - f64::from(*nb)).powi(2) * 0.11;
        if d < best.1 {
            best = (name, d);
        }
    }
    best.0
}

/// Near-transparent pixels collapse to one "empty" value so the RGB junk stored
/// underneath `alpha = 0` (which varies per encoder) cannot fake a busy image.
fn canonical(px: [u8; 4], ignore_transparency: bool) -> [u8; 4] {
    if ignore_transparency && px[3] < ALPHA_THRESHOLD {
        [0, 0, 0, 0]
    } else {
        px
    }
}

/// Largest per-channel distance between two RGBA quadruples.
fn channel_distance(a: [u8; 4], b: [u8; 4]) -> f64 {
    (0..4)
        .map(|i| (f64::from(a[i]) - f64::from(b[i])).abs())
        .fold(0.0, f64::max)
}

fn decode(bytes: &[u8]) -> Result<RgbaImage, String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read the image header: {e}"))?;
    let decoder = reader.into_decoder().map_err(|e| {
        format!("could not decode the image (PNG, JPEG, WebP, GIF and BMP are supported): {e}")
    })?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return Err("the image has zero width or height".into());
    }
    let needed = bytes.len() as u64 + decoder.total_bytes();
    if needed > MAX_DECODE_BYTES {
        return Err(format!(
            "image is too large to analyse in the sandbox ({w}x{h} needs about {} MB, the limit is \
             {} MB) — re-export it at a lower resolution",
            needed / (1024 * 1024),
            MAX_DECODE_BYTES / (1024 * 1024)
        ));
    }
    let img = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("could not decode the image: {e}"))?;
    Ok(img.to_rgba8())
}

/// Decide whether the encoded image `bytes` is blank.
///
/// * `tolerance` — per-channel match distance as a % of the full 0-255 range,
///   0-100. Two pixels count as the same colour within it; 0 demands exactness.
/// * `blank_threshold` — % of pixels that must match the dominant colour before
///   the image is called blank, 50-100.
/// * `ignore_transparency` — collapse near-transparent pixels to one "empty"
///   value before comparing, so the RGB stored under `alpha = 0` is irrelevant.
pub fn detect(
    bytes: &[u8],
    tolerance: f64,
    blank_threshold: f64,
    ignore_transparency: bool,
) -> Result<Detection, String> {
    if !(0.0..=100.0).contains(&tolerance) {
        return Err(format!(
            "tolerance must be between 0 and 100 (got {tolerance})"
        ));
    }
    if !(50.0..=100.0).contains(&blank_threshold) {
        return Err(format!(
            "blank_threshold must be between 50 and 100 (got {blank_threshold})"
        ));
    }
    if bytes.is_empty() {
        return Err("no image data was provided".into());
    }

    let img = decode(bytes)?;
    let (w, h) = (img.width(), img.height());
    let total = u64::from(w) * u64::from(h);
    let tol = tolerance / 100.0 * 255.0;
    let mut warnings: Vec<String> = Vec::new();

    // Pass 1 — bucket every pixel and gather the supporting statistics.
    let mut buckets: HashMap<u16, Bucket> = HashMap::new();
    let mut unique: HashSet<u32> = HashSet::new();
    let mut unique_capped = false;
    let mut hist = [0u64; 256];
    let (mut sum_r, mut sum_g, mut sum_b) = (0u64, 0u64, 0u64);
    let (mut sum_l, mut sum_l2) = (0.0f64, 0.0f64);
    let (mut luma_min, mut luma_max) = (255u8, 0u8);
    let mut min_c = [255u8; 3];
    let mut max_c = [0u8; 3];
    let mut transparent = 0u64;
    let mut has_alpha = false;

    for px in img.pixels() {
        let raw = px.0;
        if raw[3] < 255 {
            has_alpha = true;
        }
        if raw[3] < ALPHA_THRESHOLD {
            transparent += 1;
        }
        let [r, g, b, a] = canonical(raw, ignore_transparency);

        sum_r += u64::from(r);
        sum_g += u64::from(g);
        sum_b += u64::from(b);
        for (i, c) in [r, g, b].into_iter().enumerate() {
            min_c[i] = min_c[i].min(c);
            max_c[i] = max_c[i].max(c);
        }

        let l = luma(r, g, b);
        sum_l += l;
        sum_l2 += l * l;
        let lb = l.round().clamp(0.0, 255.0) as u8;
        hist[lb as usize] += 1;
        luma_min = luma_min.min(lb);
        luma_max = luma_max.max(lb);

        if !unique_capped {
            unique.insert(
                (u32::from(r) << 24) | (u32::from(g) << 16) | (u32::from(b) << 8) | u32::from(a),
            );
            if unique.len() >= UNIQUE_COLOR_CAP {
                unique_capped = true;
            }
        }

        let key = (((u32::from(r) / BUCKET) << 12)
            | ((u32::from(g) / BUCKET) << 8)
            | ((u32::from(b) / BUCKET) << 4)
            | (u32::from(a) / BUCKET)) as u16;
        let e = buckets.entry(key).or_default();
        e.n += 1;
        e.sr += u64::from(r);
        e.sg += u64::from(g);
        e.sb += u64::from(b);
        e.sa += u64::from(a);
    }

    let n = total.max(1) as f64;

    // The most populous bucket is only a CANDIDATE: quantisation can split one
    // noisy fill across two neighbouring buckets. Ties break on the lower key so
    // the answer is deterministic.
    let mut best_key = 0u16;
    let mut best = Bucket::default();
    for (k, bkt) in &buckets {
        if bkt.n > best.n || (bkt.n == best.n && *k < best_key) {
            best_key = *k;
            best = *bkt;
        }
    }

    // Pass 2 — mean-shift once: re-average the pixels within `tolerance` of the
    // candidate so the reported fill is the true centre of the cluster.
    let candidate = best.mean();
    let mut shifted = Bucket::default();
    for px in img.pixels() {
        let c = canonical(px.0, ignore_transparency);
        if channel_distance(c, candidate) <= tol {
            shifted.n += 1;
            shifted.sr += u64::from(c[0]);
            shifted.sg += u64::from(c[1]);
            shifted.sb += u64::from(c[2]);
            shifted.sa += u64::from(c[3]);
        }
    }
    let fill = if shifted.n == 0 {
        candidate
    } else {
        shifted.mean()
    };

    // Pass 3 — the actual coverage measurement, against the refined fill colour.
    let mut within = 0u64;
    for px in img.pixels() {
        if channel_distance(canonical(px.0, ignore_transparency), fill) <= tol {
            within += 1;
        }
    }

    let [fr, fg, fb, fa] = fill;
    let coverage_percent = round2(within as f64 / n * 100.0);
    let transparent_percent = round2(transparent as f64 / n * 100.0);

    let mut entropy = 0.0f64;
    for &c in hist.iter() {
        if c > 0 {
            let p = c as f64 / n;
            entropy -= p * p.log2();
        }
    }
    let luma_mean = sum_l / n;
    let luma_stddev = (sum_l2 / n - luma_mean * luma_mean).max(0.0).sqrt();
    let channel_range = (0..3)
        .map(|i| max_c[i].saturating_sub(min_c[i]))
        .max()
        .unwrap_or(0);

    // `within == total` means EVERY pixel matched — the difference between a
    // genuinely uniform frame and one that is blank apart from a watermark.
    let all_match = within == total;
    let is_blank = coverage_percent + 1e-9 >= blank_threshold;
    let verdict = if !is_blank {
        NOT_BLANK
    } else if fa < ALPHA_THRESHOLD {
        TRANSPARENT
    } else if all_match && fr >= WHITE_LEVEL && fg >= WHITE_LEVEL && fb >= WHITE_LEVEL {
        ALL_WHITE
    } else if all_match && fr <= BLACK_LEVEL && fg <= BLACK_LEVEL && fb <= BLACK_LEVEL {
        ALL_BLACK
    } else if all_match {
        SOLID_COLOR
    } else {
        NEAR_BLANK
    };

    // Confidence blends two independent signals: how much of the frame the fill
    // covers, and how much luma variation there is. Entropy is reported but not
    // used here — a two-tone image has barely 1 bit of it yet is obviously not
    // blank, whereas its luma spread is decisive.
    let cov = (coverage_percent / 100.0).clamp(0.0, 1.0);
    let detail = (luma_stddev / DETAIL_STDDEV).clamp(0.0, 1.0);
    let confidence = if verdict == TRANSPARENT {
        round3((transparent_percent / 100.0).clamp(0.0, 1.0))
    } else if is_blank {
        round3((0.7 * cov + 0.3 * (1.0 - detail)).clamp(0.0, 1.0))
    } else {
        round3((0.7 * (1.0 - cov) + 0.3 * detail).clamp(0.0, 1.0))
    };

    if !is_blank && coverage_percent >= blank_threshold - 2.0 {
        warnings.push(format!(
            "borderline: the dominant colour covers {coverage_percent}%, just under the \
             {blank_threshold}% blank_threshold — raise tolerance or lower blank_threshold if \
             this should count as blank"
        ));
    }
    if unique_capped {
        warnings.push(format!(
            "the image has more than {UNIQUE_COLOR_CAP} distinct colours — unique_colors is a \
             lower bound"
        ));
    }
    if has_alpha && !ignore_transparency {
        warnings.push(
            "ignore_transparency is off, so the RGB stored underneath transparent pixels is \
             compared as-is — a transparent canvas may read as detailed"
                .into(),
        );
    }

    let color_name = nearest_name(fr, fg, fb);
    let content = total - within;
    let content_percent = round2(content as f64 / n * 100.0);
    let reason = match verdict {
        TRANSPARENT => format!(
            "Blank: {transparent_percent}% of the {total} pixels are transparent, so the image \
             renders as an empty canvas."
        ),
        ALL_WHITE => format!(
            "Blank: every one of the {total} pixels is white ({}) within the {tolerance}% \
             tolerance — the signature of a failed render or export.",
            to_hex(fr, fg, fb)
        ),
        ALL_BLACK => format!(
            "Blank: every one of the {total} pixels is black ({}) within the {tolerance}% \
             tolerance — the signature of a failed render or export.",
            to_hex(fr, fg, fb)
        ),
        SOLID_COLOR => format!(
            "Blank: the image is one flat {color_name} fill ({}) across all {total} pixels; \
             luma stays at {luma_max} and the entropy is {} bits.",
            to_hex(fr, fg, fb),
            round2(entropy)
        ),
        NEAR_BLANK => format!(
            "Nearly blank: {coverage_percent}% of the pixels are {color_name} ({}); only \
             {content} pixels ({content_percent}%) carry any other content — likely a watermark \
             or a stray artefact on an otherwise empty frame.",
            to_hex(fr, fg, fb)
        ),
        _ => format!(
            "Not blank: the dominant colour {} covers only {coverage_percent}% of the frame \
             (blank_threshold is {blank_threshold}%); luma spans {luma_min}-{luma_max} and the \
             entropy is {} bits.",
            to_hex(fr, fg, fb),
            round2(entropy)
        ),
    };

    Ok(Detection {
        width: w,
        height: h,
        total_pixels: total,
        transparent_pixels: transparent,
        transparent_percent,
        has_alpha,
        is_blank,
        verdict,
        reason,
        confidence,
        dominant_hex: to_hex(fr, fg, fb),
        dominant_hex_rgba: format!("#{fr:02x}{fg:02x}{fb:02x}{fa:02x}"),
        dominant_rgb: format!("rgb({fr}, {fg}, {fb})"),
        color_name,
        r: fr,
        g: fg,
        b: fb,
        a: fa,
        coverage_percent,
        unique_colors: unique.len() as u64,
        unique_colors_capped: unique_capped,
        mean_hex: to_hex(
            (sum_r as f64 / n).round().clamp(0.0, 255.0) as u8,
            (sum_g as f64 / n).round().clamp(0.0, 255.0) as u8,
            (sum_b as f64 / n).round().clamp(0.0, 255.0) as u8,
        ),
        luma_min,
        luma_max,
        luma_mean: round2(luma_mean),
        luma_stddev: round2(luma_stddev),
        channel_range,
        entropy: round3(entropy),
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba};

    /// Default knobs, mirrored by the block's descriptor.
    const TOL: f64 = 2.0;
    const THRESHOLD: f64 = 99.5;

    fn encode(img: RgbaImage) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    fn solid(w: u32, h: u32, px: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(px))
    }

    fn run(img: RgbaImage) -> Detection {
        detect(&encode(img), TOL, THRESHOLD, true).unwrap()
    }

    #[test]
    fn all_white_page_is_blank() {
        let d = run(solid(40, 30, [255, 255, 255, 255]));
        assert!(d.is_blank);
        assert_eq!(d.verdict, ALL_WHITE);
        assert_eq!(d.dominant_hex, "#ffffff");
        assert_eq!(d.coverage_percent, 100.0);
        assert_eq!(d.unique_colors, 1);
        assert_eq!(d.entropy, 0.0);
        assert_eq!(d.luma_stddev, 0.0);
        assert_eq!(d.channel_range, 0);
        assert_eq!(d.total_pixels, 1200);
        assert!(d.confidence > 0.99, "confidence {}", d.confidence);
    }

    #[test]
    fn all_black_frame_is_blank() {
        let d = run(solid(16, 16, [0, 0, 0, 255]));
        assert!(d.is_blank);
        assert_eq!(d.verdict, ALL_BLACK);
        assert_eq!(d.dominant_hex, "#000000");
        assert_eq!(d.color_name, "black");
        assert_eq!(d.luma_max, 0);
    }

    #[test]
    fn solid_color_fill_is_blank_but_not_white_or_black() {
        let d = run(solid(20, 20, [50, 100, 220, 255]));
        assert!(d.is_blank);
        assert_eq!(d.verdict, SOLID_COLOR);
        assert_eq!(d.dominant_hex, "#3264dc");
        assert_eq!(d.dominant_rgb, "rgb(50, 100, 220)");
        assert_eq!(d.color_name, "blue");
        assert_eq!(d.mean_hex, "#3264dc");
    }

    #[test]
    fn fully_transparent_canvas_is_blank() {
        // Junk RGB under alpha=0 must not fake content: the canonicaliser
        // collapses every transparent pixel to one "empty" value.
        let mut img = solid(10, 10, [0, 0, 0, 0]);
        img.put_pixel(0, 0, Rgba([200, 30, 90, 0]));
        img.put_pixel(9, 9, Rgba([10, 240, 15, 0]));
        let d = run(img);
        assert!(d.is_blank);
        assert_eq!(d.verdict, TRANSPARENT);
        assert_eq!(d.transparent_percent, 100.0);
        assert!(d.has_alpha);
        assert_eq!(d.unique_colors, 1);
        assert_eq!(d.confidence, 1.0);
    }

    #[test]
    fn watermark_on_an_empty_page_reads_near_blank() {
        // 100x100 white page with a 4x4 dark mark = 0.16% content, under the
        // 0.5% the default threshold allows.
        let mut img = solid(100, 100, [255, 255, 255, 255]);
        for y in 0..4 {
            for x in 0..4 {
                img.put_pixel(x, y, Rgba([20, 20, 20, 255]));
            }
        }
        let d = run(img);
        assert!(d.is_blank);
        assert_eq!(d.verdict, NEAR_BLANK);
        assert_eq!(d.coverage_percent, 99.84);
        assert_eq!(d.dominant_hex, "#ffffff");
        assert!(d.reason.contains("watermark"), "{}", d.reason);
    }

    #[test]
    fn real_content_is_not_blank() {
        // Half black, half white — the classic two-tone image: 1 bit of entropy.
        let mut img = solid(20, 20, [255, 255, 255, 255]);
        for y in 0..20 {
            for x in 0..10 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let d = run(img);
        assert!(!d.is_blank);
        assert_eq!(d.verdict, NOT_BLANK);
        assert_eq!(d.coverage_percent, 50.0);
        assert_eq!(d.entropy, 1.0);
        assert_eq!(d.channel_range, 255);
        assert_eq!(d.luma_min, 0);
        assert_eq!(d.luma_max, 255);
        assert!(d.confidence > 0.5, "confidence {}", d.confidence);
    }

    #[test]
    fn logo_on_a_transparent_canvas_is_not_blank() {
        // Transparency is compared, not filtered: a 25%-covered logo is content.
        let mut img = solid(20, 20, [0, 0, 0, 0]);
        for y in 0..10 {
            for x in 0..10 {
                img.put_pixel(x, y, Rgba([220, 40, 40, 255]));
            }
        }
        let d = run(img);
        assert!(!d.is_blank);
        assert_eq!(d.verdict, NOT_BLANK);
        assert_eq!(d.coverage_percent, 75.0);
        assert_eq!(d.transparent_percent, 75.0);
    }

    #[test]
    fn tolerance_absorbs_compression_noise_on_a_flat_fill() {
        // A near-white page whose pixels wobble by ±3 levels (JPEG-style noise).
        // At tolerance 0 the wobble reads as content; at the 2% default it does not.
        let mut img = solid(50, 50, [252, 252, 252, 255]);
        for (i, p) in img.pixels_mut().enumerate() {
            let v = 252 + (i % 4) as u8 - 2;
            *p = Rgba([v, v, v, 255]);
        }
        let bytes = encode(img);
        let strict = detect(&bytes, 0.0, THRESHOLD, true).unwrap();
        assert!(!strict.is_blank, "exact matching sees the noise as content");
        let lenient = detect(&bytes, TOL, THRESHOLD, true).unwrap();
        assert!(lenient.is_blank);
        assert_eq!(lenient.coverage_percent, 100.0);
        assert!(lenient.unique_colors > 1);
    }

    #[test]
    fn threshold_controls_how_much_content_is_forgiven() {
        // 1% of a white page is dark: blank at a 98% threshold, not at 99.5%.
        let mut img = solid(100, 100, [255, 255, 255, 255]);
        for y in 0..100 {
            for x in 0..1 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let bytes = encode(img);
        assert!(!detect(&bytes, TOL, 99.5, true).unwrap().is_blank);
        let loose = detect(&bytes, TOL, 98.0, true).unwrap();
        assert!(loose.is_blank);
        assert_eq!(loose.verdict, NEAR_BLANK);
    }

    #[test]
    fn borderline_result_is_flagged_as_a_warning() {
        // 1% content against a 99.5% threshold: not blank, but only just.
        let mut img = solid(100, 100, [255, 255, 255, 255]);
        for y in 0..100 {
            img.put_pixel(0, y, Rgba([0, 0, 0, 255]));
        }
        let d = run(img);
        assert!(!d.is_blank);
        assert!(
            d.warnings.iter().any(|w| w.contains("borderline")),
            "{:?}",
            d.warnings
        );
    }

    #[test]
    fn keeping_transparency_compares_the_rgb_underneath() {
        let mut img = solid(10, 10, [0, 0, 0, 0]);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 0]));
        let d = detect(&encode(img), TOL, THRESHOLD, false).unwrap();
        assert_eq!(d.unique_colors, 2, "raw RGBA values are distinct");
        assert!(
            d.warnings.iter().any(|w| w.contains("ignore_transparency")),
            "{:?}",
            d.warnings
        );
    }

    #[test]
    fn rejects_out_of_range_knobs_and_garbage_input() {
        assert!(detect(b"", TOL, THRESHOLD, true).is_err());
        assert!(detect(b"definitely not an image", TOL, THRESHOLD, true).is_err());
        let png = encode(solid(2, 2, [255, 255, 255, 255]));
        assert!(detect(&png, -1.0, THRESHOLD, true).is_err());
        assert!(detect(&png, 101.0, THRESHOLD, true).is_err());
        assert!(detect(&png, TOL, 49.0, true).is_err());
        assert!(detect(&png, TOL, 100.1, true).is_err());
    }
}
