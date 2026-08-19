//! gizza-ai/background-color-detector core — decide which colour is an image's
//! BACKDROP (not its average, and not its globally most common colour) and say
//! how confident that verdict is. No wafer/wasm-bindgen deps. Pure-Rust
//! (`image` decode only), so the block runs on every backend including the chat
//! Service Worker. Shared by the chat skill block, the CLI and the unit tests.
//!
//! Approach — the standard corner/edge heuristic, made measurable:
//!   1. Sample only the **border band** of the image (`region`, `border_percent`).
//!      A subject sitting in the middle of the frame therefore cannot outvote the
//!      backdrop the way it does for a whole-image average or dominant-colour scan.
//!   2. Quantise every sampled opaque pixel into a coarse RGB bucket and take the
//!      most populous bucket; the reported colour is the exact mean of that bucket,
//!      so near-identical pixels (JPEG noise, a subtle vignette) group together.
//!   3. Re-scan and count how many sampled pixels sit within `tolerance` of that
//!      colour. That share is `coverage_percent`; at or above `uniform_threshold`
//!      the background is reported as a genuine flat fill (`is_uniform`).
//!   4. Cross-check the four corners independently, so a gradient or a photo
//!      backdrop shows up as disagreeing corners instead of a confident wrong hex.
//!
//! Alpha: by default pixels below [`ALPHA_THRESHOLD`] count as transparent — they
//! are excluded from the colour vote but stay in the denominator, and a border
//! that is mostly transparent is reported as a transparent background.

use std::collections::HashMap;
use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};

/// Input bytes + decoded raster must fit alongside the runtime in the wasm sandbox.
const MAX_DECODE_BYTES: u64 = 48 * 1024 * 1024;
/// Inspect at most this many pixels; larger regions are sub-sampled with a stride.
const MAX_SAMPLED_PIXELS: u64 = 2_000_000;
/// Pixels with alpha below this read as transparent.
pub const ALPHA_THRESHOLD: u8 = 16;
/// Per-channel quantisation step used to pick the CANDIDATE background bucket.
const BUCKET: u32 = 16;

/// Which part of the frame is sampled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// The whole band around all four sides (corners included). The default.
    Border,
    /// Only the four square corner patches.
    Corners,
    /// The four edge strips with the corner patches removed.
    Edges,
    /// Every pixel — the plain dominant-colour fallback.
    Full,
}

impl Region {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "border" => Ok(Region::Border),
            "corners" => Ok(Region::Corners),
            "edges" => Ok(Region::Edges),
            "full" => Ok(Region::Full),
            other => Err(format!(
                "unknown region '{other}': expected border, corners, edges or full"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Region::Border => "border",
            Region::Corners => "corners",
            Region::Edges => "edges",
            Region::Full => "full",
        }
    }
}

/// The full background report.
#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
    pub width: u32,
    pub height: u32,
    /// The region actually sampled (may differ from the request — see `warnings`).
    pub region: &'static str,
    /// Band thickness in pixels for the border/corners/edges regions (0 for full).
    pub band_px: u32,
    /// Sub-sampling stride actually used (1 = every pixel in the region).
    pub stride: u32,
    /// Sampled pixels, transparent ones included.
    pub sampled_pixels: u64,
    pub opaque_pixels: u64,
    pub transparent_pixels: u64,
    pub transparent_percent: f64,
    /// True when the sampled border is transparent enough to call the background
    /// transparent (only when `ignore_transparency` is on).
    pub is_transparent: bool,
    /// `#rrggbb` (lowercase).
    pub hex: String,
    /// `#rrggbbaa` (lowercase), including the mean alpha of the winning cluster.
    pub hex_rgba: String,
    /// CSS `rgb(r, g, b)`.
    pub rgb: String,
    /// CSS `rgba(r, g, b, a)` with alpha 0-1 (2 decimals).
    pub rgba: String,
    /// CSS `hsl(h, s%, l%)`.
    pub hsl: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
    /// Nearest plain-English colour name, for a human-readable summary.
    pub color_name: &'static str,
    /// Share of sampled pixels within `tolerance` of the reported colour.
    pub coverage_percent: f64,
    /// `coverage_percent >= uniform_threshold` — a genuine flat background.
    pub is_uniform: bool,
    /// 0-1 blend of coverage (75 %) and corner agreement (25 %).
    pub confidence: f64,
    /// Runner-up border colour outside `tolerance` of the winner, if any.
    pub second_hex: Option<String>,
    pub second_coverage_percent: f64,
    pub corner_top_left: String,
    pub corner_top_right: String,
    pub corner_bottom_left: String,
    pub corner_bottom_right: String,
    /// All four corner patches sit within `tolerance` of the reported colour.
    pub corners_agree: bool,
    /// Largest per-channel corner-to-background distance, as % of full range.
    pub max_corner_distance_percent: f64,
    /// WCAG relative luminance of the reported colour, 0-1.
    pub luminance: f64,
    /// Luminance below the WCAG black/white crossover — light text reads better.
    pub is_dark: bool,
    /// `#000000` or `#ffffff`, whichever contrasts more with the background.
    pub suggested_text_color: String,
    /// WCAG contrast ratio of `suggested_text_color` on the background, 1-21.
    pub contrast_ratio: f64,
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

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
    let (rf, gf, bf) = (
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    );
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < f64::EPSILON {
        return (0, 0, (l * 100.0).round() as u8);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - rf).abs() < f64::EPSILON {
        ((gf - bf) / d).rem_euclid(6.0)
    } else if (max - gf).abs() < f64::EPSILON {
        (bf - rf) / d + 2.0
    } else {
        (rf - gf) / d + 4.0
    } * 60.0;
    (
        h.round().rem_euclid(360.0) as u16,
        (s * 100.0).round() as u8,
        (l * 100.0).round() as u8,
    )
}

/// sRGB channel → linear light (WCAG).
fn srgb_to_linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance, 0-1.
fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b)
}

/// WCAG contrast ratio between two relative luminances.
fn contrast(l1: f64, l2: f64) -> f64 {
    let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
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

/// Largest per-channel distance between two RGB triples.
fn channel_distance(a: [u8; 3], b: [u8; 3]) -> f64 {
    (0..3)
        .map(|i| (f64::from(a[i]) - f64::from(b[i])).abs())
        .fold(0.0, f64::max)
}

/// Smallest stride keeping the inspected pixel count under [`MAX_SAMPLED_PIXELS`].
fn stride_for(region_pixels: u64) -> u32 {
    if region_pixels <= MAX_SAMPLED_PIXELS {
        return 1;
    }
    let ratio = region_pixels as f64 / MAX_SAMPLED_PIXELS as f64;
    ratio.sqrt().ceil().max(1.0) as u32
}

/// Is `(x, y)` inside the requested sampling region?
fn in_region(region: Region, x: u32, y: u32, w: u32, h: u32, band: u32) -> bool {
    let near_x = x < band || x + band >= w;
    let near_y = y < band || y + band >= h;
    match region {
        Region::Full => true,
        Region::Border => near_x || near_y,
        Region::Corners => near_x && near_y,
        Region::Edges => (near_x || near_y) && !(near_x && near_y),
    }
}

/// Exact pixel count of a region (used only to size the sampling stride).
fn region_pixels(region: Region, w: u32, h: u32, band: u32) -> u64 {
    let (w64, h64) = (u64::from(w), u64::from(h));
    let cx = u64::from(w.min(band.saturating_mul(2)));
    let cy = u64::from(h.min(band.saturating_mul(2)));
    let inner_w = w64.saturating_sub(cx);
    let inner_h = h64.saturating_sub(cy);
    let border = w64 * h64 - inner_w * inner_h;
    match region {
        Region::Full => w64 * h64,
        Region::Border => border,
        Region::Corners => cx * cy,
        Region::Edges => border.saturating_sub(cx * cy),
    }
}

/// Mean colour of a corner patch, alpha-aware, as `#rrggbb`.
fn corner_patch(
    img: &RgbaImage,
    cx: u32,
    cy: u32,
    size: u32,
    ignore_transparency: bool,
) -> [u8; 3] {
    let (w, h) = (img.width(), img.height());
    let x0 = cx.min(w.saturating_sub(1));
    let y0 = cy.min(h.saturating_sub(1));
    let x1 = (x0 + size).min(w);
    let y1 = (y0 + size).min(h);
    let mut acc = Bucket::default();
    let mut all = Bucket::default();
    for y in y0..y1 {
        for x in x0..x1 {
            let [r, g, b, a] = img.get_pixel(x, y).0;
            all.n += 1;
            all.sr += u64::from(r);
            all.sg += u64::from(g);
            all.sb += u64::from(b);
            if ignore_transparency && a < ALPHA_THRESHOLD {
                continue;
            }
            acc.n += 1;
            acc.sr += u64::from(r);
            acc.sg += u64::from(g);
            acc.sb += u64::from(b);
        }
    }
    let src = if acc.n > 0 { acc } else { all };
    let [r, g, b, _] = src.mean();
    [r, g, b]
}

/// Detect the background colour of the encoded image `bytes`.
///
/// * `region` — which part of the frame votes (see [`Region`]).
/// * `border_percent` — band thickness as a % of the shorter side, 1-50.
/// * `tolerance` — per-channel match distance as a % of the full 0-255 range, 0-100.
/// * `uniform_threshold` — coverage % at which the background counts as a flat fill, 0-100.
/// * `ignore_transparency` — exclude near-transparent pixels from the colour vote.
pub fn detect(
    bytes: &[u8],
    region: Region,
    border_percent: f64,
    tolerance: f64,
    uniform_threshold: f64,
    ignore_transparency: bool,
) -> Result<Detection, String> {
    if !(1.0..=50.0).contains(&border_percent) {
        return Err(format!(
            "border_percent must be between 1 and 50 (got {border_percent})"
        ));
    }
    if !(0.0..=100.0).contains(&tolerance) {
        return Err(format!(
            "tolerance must be between 0 and 100 (got {tolerance})"
        ));
    }
    if !(0.0..=100.0).contains(&uniform_threshold) {
        return Err(format!(
            "uniform_threshold must be between 0 and 100 (got {uniform_threshold})"
        ));
    }
    if bytes.is_empty() {
        return Err("no image data was provided".into());
    }

    let img = decode(bytes)?;
    let (w, h) = (img.width(), img.height());
    let mut warnings: Vec<String> = Vec::new();

    // Band thickness: a share of the SHORTER side, never below one pixel and
    // never so thick that the "border" swallows the whole frame silently.
    let band = if region == Region::Full {
        0
    } else {
        let raw = (f64::from(w.min(h)) * border_percent / 100.0).round();
        (raw as u32).max(1)
    };
    if region != Region::Full && band * 2 >= w.min(h) {
        warnings.push(format!(
            "the {band}px band covers the whole {w}x{h} image — every pixel votes, so this is a \
             plain dominant-colour scan; lower border_percent for a true border sample"
        ));
    }

    // Fall back when the requested region has no pixels at all (tiny images).
    let mut region = region;
    if region_pixels(region, w, h, band) == 0 {
        warnings.push(format!(
            "the {} region is empty at this size ({w}x{h}) — sampled the whole border instead",
            region.as_str()
        ));
        region = Region::Border;
    }

    let mut stride = stride_for(region_pixels(region, w, h, band));
    if region != Region::Full {
        stride = stride.min(band.max(1));
    }
    if stride > 1 {
        warnings.push(format!(
            "large image: sampled every {stride} pixels in each direction rather than every pixel"
        ));
    }

    let tol = tolerance / 100.0 * 255.0;

    // Pass 1 — bucket every sampled opaque pixel and count transparency.
    let mut buckets: HashMap<u16, Bucket> = HashMap::new();
    let mut all_pixels = Bucket::default();
    let mut sampled: u64 = 0;
    let mut transparent: u64 = 0;
    for y in (0..h).step_by(stride as usize) {
        for x in (0..w).step_by(stride as usize) {
            if !in_region(region, x, y, w, h, band) {
                continue;
            }
            sampled += 1;
            let [r, g, b, a] = img.get_pixel(x, y).0;
            all_pixels.n += 1;
            all_pixels.sr += u64::from(r);
            all_pixels.sg += u64::from(g);
            all_pixels.sb += u64::from(b);
            all_pixels.sa += u64::from(a);
            if a < ALPHA_THRESHOLD {
                transparent += 1;
                if ignore_transparency {
                    continue;
                }
            }
            let key = (((u32::from(r) / BUCKET) << 8)
                | ((u32::from(g) / BUCKET) << 4)
                | (u32::from(b) / BUCKET)) as u16;
            let e = buckets.entry(key).or_default();
            e.n += 1;
            e.sr += u64::from(r);
            e.sg += u64::from(g);
            e.sb += u64::from(b);
            e.sa += u64::from(a);
        }
    }
    if sampled == 0 {
        return Err(format!(
            "no pixels could be sampled from this {w}x{h} image — try region=full"
        ));
    }
    let opaque = sampled - transparent;
    let transparent_percent = round2(transparent as f64 / sampled as f64 * 100.0);
    let is_transparent = ignore_transparency && transparent_percent >= uniform_threshold;

    // A fully transparent border still deserves an answer: report the RGB that
    // sits underneath the alpha rather than erroring out, and let every pixel
    // vote so a uniformly transparent backdrop still measures as uniform.
    let vote_all = buckets.is_empty();
    if vote_all {
        warnings.push(
            "every sampled pixel is transparent — reporting the colour stored underneath the \
             alpha channel"
                .into(),
        );
        let key = 0u16;
        buckets.insert(key, all_pixels);
    }

    // The candidate: most populous bucket (lowest key wins ties, for determinism).
    let mut ranked: Vec<(u16, Bucket)> = buckets.into_iter().collect();
    ranked.sort_by(|a, b| b.1.n.cmp(&a.1.n).then(a.0.cmp(&b.0)));
    let primary = ranked[0].1.mean();
    let bg = [primary[0], primary[1], primary[2]];
    let alpha = primary[3];

    // The runner-up: the next densest bucket that is a genuinely different colour.
    let second = ranked
        .iter()
        .skip(1)
        .map(|(_, b)| b.mean())
        .find(|m| channel_distance([m[0], m[1], m[2]], bg) > tol);

    // Pass 2 — how much of the sampled region actually matches each candidate?
    let mut matched: u64 = 0;
    let mut matched_second: u64 = 0;
    for y in (0..h).step_by(stride as usize) {
        for x in (0..w).step_by(stride as usize) {
            if !in_region(region, x, y, w, h, band) {
                continue;
            }
            let [r, g, b, a] = img.get_pixel(x, y).0;
            if ignore_transparency && !vote_all && a < ALPHA_THRESHOLD {
                continue;
            }
            let px = [r, g, b];
            if channel_distance(px, bg) <= tol {
                matched += 1;
            }
            if let Some(s) = second {
                if channel_distance(px, [s[0], s[1], s[2]]) <= tol {
                    matched_second += 1;
                }
            }
        }
    }
    let coverage_percent = round2(matched as f64 / sampled as f64 * 100.0);
    let second_coverage_percent = round2(matched_second as f64 / sampled as f64 * 100.0);
    // A backdrop that is uniformly TRANSPARENT is uniform too, even though no
    // opaque pixel matched the reported colour.
    let effective_coverage = if is_transparent {
        coverage_percent.max(transparent_percent)
    } else {
        coverage_percent
    };
    let is_uniform = effective_coverage >= uniform_threshold;

    // Independent corner cross-check.
    let patch = band.max(1).min(w.max(1)).min(h.max(1));
    let tl = corner_patch(&img, 0, 0, patch, ignore_transparency);
    let tr = corner_patch(&img, w.saturating_sub(patch), 0, patch, ignore_transparency);
    let bl = corner_patch(&img, 0, h.saturating_sub(patch), patch, ignore_transparency);
    let br = corner_patch(
        &img,
        w.saturating_sub(patch),
        h.saturating_sub(patch),
        patch,
        ignore_transparency,
    );
    let max_corner = [tl, tr, bl, br]
        .iter()
        .map(|c| channel_distance(*c, bg))
        .fold(0.0, f64::max);
    let agreeing = [tl, tr, bl, br]
        .iter()
        .filter(|c| channel_distance(**c, bg) <= tol)
        .count();
    let corners_agree = agreeing == 4;

    let confidence = round3(
        (0.75 * (effective_coverage / 100.0) + 0.25 * (agreeing as f64 / 4.0)).clamp(0.0, 1.0),
    );

    if !is_uniform {
        warnings.push(format!(
            "the sampled {} is not a single flat colour ({coverage_percent}% of it matches the \
             reported one, below the {uniform_threshold}% threshold) — the backdrop may be a \
             gradient, a photo or a pattern",
            region.as_str()
        ));
    }
    if transparent > 0 && !is_transparent && ignore_transparency {
        warnings.push(format!(
            "{transparent_percent}% of the sampled pixels are transparent and were left out of the \
             colour vote"
        ));
    }
    if sampled < 16 {
        warnings.push(format!(
            "only {sampled} pixels were sampled — the verdict is weak on an image this small"
        ));
    }

    let lum = relative_luminance(bg[0], bg[1], bg[2]);
    let on_white = contrast(lum, 1.0);
    let on_black = contrast(lum, 0.0);
    let (suggested_text_color, contrast_ratio) = if on_white >= on_black {
        ("#ffffff".to_string(), on_white)
    } else {
        ("#000000".to_string(), on_black)
    };
    let (hh, ss, ll) = rgb_to_hsl(bg[0], bg[1], bg[2]);

    Ok(Detection {
        width: w,
        height: h,
        region: region.as_str(),
        band_px: band,
        stride,
        sampled_pixels: sampled,
        opaque_pixels: opaque,
        transparent_pixels: transparent,
        transparent_percent,
        is_transparent,
        hex: to_hex(bg[0], bg[1], bg[2]),
        hex_rgba: format!("{}{alpha:02x}", to_hex(bg[0], bg[1], bg[2])),
        rgb: format!("rgb({}, {}, {})", bg[0], bg[1], bg[2]),
        rgba: format!(
            "rgba({}, {}, {}, {:.2})",
            bg[0],
            bg[1],
            bg[2],
            f64::from(alpha) / 255.0
        ),
        hsl: format!("hsl({hh}, {ss}%, {ll}%)"),
        r: bg[0],
        g: bg[1],
        b: bg[2],
        a: alpha,
        color_name: nearest_name(bg[0], bg[1], bg[2]),
        coverage_percent,
        is_uniform,
        confidence,
        second_hex: second.map(|s| to_hex(s[0], s[1], s[2])),
        second_coverage_percent,
        corner_top_left: to_hex(tl[0], tl[1], tl[2]),
        corner_top_right: to_hex(tr[0], tr[1], tr[2]),
        corner_bottom_left: to_hex(bl[0], bl[1], bl[2]),
        corner_bottom_right: to_hex(br[0], br[1], br[2]),
        corners_agree,
        max_corner_distance_percent: round2(max_corner / 255.0 * 100.0),
        luminance: round3(lum),
        // WCAG's black/white crossover: below it, white text contrasts better.
        is_dark: lum < 0.1791,
        suggested_text_color,
        contrast_ratio: round2(contrast_ratio),
        warnings,
    })
}

/// Decode with a header-first size budget so an oversized raster is refused with
/// an actionable message instead of trapping the wasm sandbox on allocation.
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba};

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

    fn fill(img: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32, px: [u8; 4]) {
        for y in y0..y1 {
            for x in x0..x1 {
                img.put_pixel(x, y, Rgba(px));
            }
        }
    }

    fn detect_default(bytes: &[u8]) -> Detection {
        detect(bytes, Region::Border, 10.0, 6.0, 90.0, true).unwrap()
    }

    #[test]
    fn a_solid_image_reports_that_exact_colour_as_a_uniform_background() {
        let d = detect_default(&encode(solid(40, 40, [255, 0, 0, 255])));
        assert_eq!(d.hex, "#ff0000");
        assert_eq!(d.hex_rgba, "#ff0000ff");
        assert_eq!(d.rgb, "rgb(255, 0, 0)");
        assert_eq!(d.rgba, "rgba(255, 0, 0, 1.00)");
        assert_eq!(d.hsl, "hsl(0, 100%, 50%)");
        assert_eq!(d.color_name, "red");
        assert_eq!(d.coverage_percent, 100.0);
        assert!(d.is_uniform);
        assert!(d.corners_agree);
        assert_eq!(d.max_corner_distance_percent, 0.0);
        assert_eq!(d.confidence, 1.0);
        assert!(d.second_hex.is_none());
        assert!(d.warnings.is_empty(), "{:?}", d.warnings);
    }

    #[test]
    fn a_dominant_centre_subject_does_not_outvote_the_backdrop() {
        // 48x48 black square on a 60x60 white canvas: black is the globally
        // dominant colour (2304 px vs 1296), white is the background.
        let mut img = solid(60, 60, [255, 255, 255, 255]);
        fill(&mut img, 6, 6, 54, 54, [0, 0, 0, 255]);
        let bytes = encode(img);

        let border = detect_default(&bytes);
        assert_eq!(border.hex, "#ffffff");
        assert!(border.is_uniform, "{}", border.coverage_percent);
        assert!(border.corners_agree);

        // The whole-image fallback picks the subject instead — the contrast that
        // makes border sampling the right default.
        let full = detect(&bytes, Region::Full, 10.0, 6.0, 90.0, true).unwrap();
        assert_eq!(full.hex, "#000000");
        assert!(!full.is_uniform);
    }

    #[test]
    fn a_transparent_border_is_reported_as_a_transparent_background() {
        let mut img = solid(40, 40, [0, 0, 0, 0]);
        fill(&mut img, 10, 10, 30, 30, [10, 120, 200, 255]);
        let d = detect_default(&encode(img));
        assert!(d.is_transparent);
        assert!(
            d.is_uniform,
            "a uniformly transparent backdrop is still uniform"
        );
        assert_eq!(d.transparent_percent, 100.0);
        assert_eq!(d.opaque_pixels, 0);
        assert_eq!(d.hex_rgba, "#00000000");
        assert!(
            d.warnings.iter().any(|w| w.contains("every sampled pixel")),
            "{:?}",
            d.warnings
        );
    }

    #[test]
    fn a_gradient_backdrop_is_flagged_instead_of_answered_confidently() {
        let mut img = solid(64, 64, [0, 0, 0, 255]);
        for y in 0..64u32 {
            for x in 0..64u32 {
                let v = (x * 4) as u8;
                img.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
        let d = detect_default(&encode(img));
        assert!(!d.is_uniform);
        assert!(!d.corners_agree);
        assert!(d.max_corner_distance_percent > 50.0);
        assert!(d.confidence < 0.6, "{}", d.confidence);
        assert!(d.second_hex.is_some());
        assert!(
            d.warnings.iter().any(|w| w.contains("not a single flat")),
            "{:?}",
            d.warnings
        );
    }

    #[test]
    fn corners_and_edges_sample_different_pixels() {
        // White frame with a red band across the middle of each side.
        let mut img = solid(40, 40, [255, 255, 255, 255]);
        fill(&mut img, 8, 0, 32, 4, [255, 0, 0, 255]);
        fill(&mut img, 8, 36, 32, 40, [255, 0, 0, 255]);
        fill(&mut img, 0, 8, 4, 32, [255, 0, 0, 255]);
        fill(&mut img, 36, 8, 40, 32, [255, 0, 0, 255]);
        let bytes = encode(img);

        let corners = detect(&bytes, Region::Corners, 10.0, 6.0, 90.0, true).unwrap();
        assert_eq!(corners.hex, "#ffffff");
        assert_eq!(corners.region, "corners");
        assert_eq!(corners.band_px, 4);

        let edges = detect(&bytes, Region::Edges, 10.0, 6.0, 90.0, true).unwrap();
        assert_eq!(edges.hex, "#ff0000");
        assert_eq!(edges.region, "edges");
    }

    #[test]
    fn tolerance_decides_whether_near_identical_pixels_group_together() {
        // Alternating #ffffff / #fafafa border noise.
        let mut img = solid(40, 40, [255, 255, 255, 255]);
        for y in 0..40u32 {
            for x in 0..40u32 {
                if (x + y) % 2 == 0 {
                    img.put_pixel(x, y, Rgba([250, 250, 250, 255]));
                }
            }
        }
        let bytes = encode(img);

        let loose = detect(&bytes, Region::Border, 10.0, 6.0, 90.0, true).unwrap();
        assert!(loose.is_uniform, "{}", loose.coverage_percent);
        assert_eq!(loose.coverage_percent, 100.0);

        let exact = detect(&bytes, Region::Border, 10.0, 0.0, 90.0, true).unwrap();
        assert!(!exact.is_uniform);
        assert!(exact.coverage_percent < 60.0, "{}", exact.coverage_percent);
    }

    #[test]
    fn a_two_tone_border_reports_the_runner_up() {
        let mut img = solid(40, 40, [255, 255, 255, 255]);
        fill(&mut img, 0, 0, 40, 20, [0, 0, 255, 255]);
        let d = detect_default(&encode(img));
        assert!(d.second_hex.is_some());
        assert!(d.second_coverage_percent > 20.0);
        assert!(!d.is_uniform);
    }

    #[test]
    fn a_dark_background_asks_for_light_text_and_a_light_one_for_dark_text() {
        let dark = detect_default(&encode(solid(32, 32, [17, 17, 17, 255])));
        assert!(dark.is_dark);
        assert_eq!(dark.suggested_text_color, "#ffffff");
        assert!(dark.contrast_ratio > 15.0, "{}", dark.contrast_ratio);

        let light = detect_default(&encode(solid(32, 32, [250, 250, 250, 255])));
        assert!(!light.is_dark);
        assert_eq!(light.suggested_text_color, "#000000");
        assert_eq!(light.color_name, "white");
    }

    #[test]
    fn keeping_transparency_folds_alpha_into_the_vote() {
        let mut img = solid(40, 40, [0, 0, 0, 0]);
        fill(&mut img, 10, 10, 30, 30, [10, 120, 200, 255]);
        let bytes = encode(img);
        let kept = detect(&bytes, Region::Border, 10.0, 6.0, 90.0, false).unwrap();
        assert!(!kept.is_transparent);
        assert_eq!(kept.opaque_pixels, 0);
        assert_eq!(kept.a, 0);
        assert_eq!(kept.hex_rgba, "#00000000");
    }

    #[test]
    fn rejects_data_that_is_not_an_image() {
        let err = detect(
            b"definitely not an image",
            Region::Border,
            10.0,
            6.0,
            90.0,
            true,
        )
        .unwrap_err();
        assert!(err.contains("decode"), "{err}");
        let err = detect(&[], Region::Border, 10.0, 6.0, 90.0, true).unwrap_err();
        assert!(err.contains("no image data"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_parameters() {
        let png = encode(solid(8, 8, [1, 2, 3, 255]));
        assert!(detect(&png, Region::Border, 0.0, 6.0, 90.0, true)
            .unwrap_err()
            .contains("border_percent"));
        assert!(detect(&png, Region::Border, 80.0, 6.0, 90.0, true)
            .unwrap_err()
            .contains("border_percent"));
        assert!(detect(&png, Region::Border, 10.0, 120.0, 90.0, true)
            .unwrap_err()
            .contains("tolerance"));
        assert!(detect(&png, Region::Border, 10.0, 6.0, -1.0, true)
            .unwrap_err()
            .contains("uniform_threshold"));
    }

    #[test]
    fn region_names_round_trip_and_reject_junk() {
        for name in ["border", "corners", "edges", "full"] {
            assert_eq!(Region::parse(name).unwrap().as_str(), name);
        }
        assert_eq!(Region::parse("BORDER").unwrap(), Region::Border);
        assert!(Region::parse("middle").unwrap_err().contains("middle"));
    }

    #[test]
    fn a_tiny_image_falls_back_instead_of_failing() {
        // 6x6: the 1px edge sample is tiny but valid and still reports the colour.
        let d = detect(
            &encode(solid(6, 6, [200, 200, 200, 255])),
            Region::Edges,
            10.0,
            6.0,
            90.0,
            true,
        )
        .unwrap();
        assert_eq!(d.region, "edges");
        assert_eq!(d.hex, "#c8c8c8");
        assert!(d.sampled_pixels < 24);
    }
}
