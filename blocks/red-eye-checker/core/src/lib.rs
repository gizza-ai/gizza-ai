//! red-eye-checker core — find the RED-EYE spots in a flash photo and say where
//! they are, how big they are, and how sure we are. No wafer/wasm-bindgen deps,
//! pure Rust (`image` decode only), so the same code backs the chat skill block,
//! the CLI and the browser page.
//!
//! This is a DETECTOR, not a corrector: it reports regions to fix, it never
//! rewrites pixels.
//!
//! Approach — the classic redness-mask + shape-filter heuristic, made measurable:
//!   1. Build a boolean mask of "flash red" pixels. A pixel qualifies when red
//!      clearly dominates the other two channels (`redness` ratio), the red
//!      channel is bright enough to be a lit retina rather than dark maroon, and
//!      the colour is saturated (a pale pink skin tone fails). [`Sensitivity`]
//!      picks the three thresholds together, so callers tune one knob.
//!   2. Group the mask into connected components (8-connectivity, iterative
//!      flood fill — no recursion, so a full-frame red image cannot blow the
//!      wasm stack).
//!   3. Keep only components whose equivalent-circle radius falls inside
//!      `[min_radius, max_radius]` and whose bounding box is roughly square and
//!      roughly disc-filled. A red jumper is large and rectangular; a pupil is
//!      small and round, so shape does most of the false-positive rejection that
//!      redness alone cannot.
//!   4. Score each survivor 0-1 from how red, how round and how square it is, and
//!      return them highest-confidence first.
//!
//! Everything the caller might want to explain to a user (thresholds not met,
//! regions clipped by `max_regions`, a red-dominant scene) comes back in
//! `warnings` rather than as a silent empty result.

use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};
use serde::Serialize;

/// Input bytes + decoded raster must fit alongside the runtime in the wasm sandbox.
const MAX_DECODE_BYTES: u64 = 48 * 1024 * 1024;
/// Pixels below this alpha are treated as absent (never part of a candidate).
pub const ALPHA_THRESHOLD: u8 = 16;
/// Inclusive bounds the public params are validated against.
pub const MIN_RADIUS_MAX: u32 = 80;
pub const MAX_RADIUS_MAX: u32 = 300;
pub const MAX_REGIONS_MAX: u32 = 100;
/// A red mask covering more of the frame than this is a red SCENE, not red-eye.
const RED_SCENE_PERCENT: f64 = 25.0;
/// Widest bounding box aspect ratio a pupil-shaped blob may have.
const MAX_ASPECT: f64 = 3.0;
/// Area fraction of its bounding box that a perfect disc fills (pi/4).
const DISC_FILL: f64 = 0.785_398_163_397_448_3;

/// How eagerly a pixel counts as "flash red". One knob moves the three
/// thresholds that matter together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    /// Only unmistakable, bright, strongly saturated red — fewest false positives.
    Low,
    /// The default: catches typical compact-camera and phone-flash red-eye.
    Medium,
    /// Also flags dim, partly-corrected or orange-ish "amber eye" — more noise.
    High,
}

/// The three per-pixel thresholds plus the shape floor a [`Sensitivity`] implies.
#[derive(Debug, Clone, Copy)]
struct Thresholds {
    /// Minimum red / mean(green, blue) ratio.
    ratio: f64,
    /// Minimum red channel value (0-255) — rejects dark maroon shadows.
    min_red: f64,
    /// Minimum HSV-style saturation (0-1) — rejects pale pink skin.
    min_sat: f64,
    /// Minimum share of its bounding box a candidate blob must fill.
    min_fill: f64,
}

impl Sensitivity {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(Sensitivity::Low),
            "medium" | "" => Ok(Sensitivity::Medium),
            "high" => Ok(Sensitivity::High),
            other => Err(format!(
                "unknown sensitivity '{other}': expected low, medium or high"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Sensitivity::Low => "low",
            Sensitivity::Medium => "medium",
            Sensitivity::High => "high",
        }
    }

    fn thresholds(self) -> Thresholds {
        match self {
            Sensitivity::Low => Thresholds {
                ratio: 2.4,
                min_red: 100.0,
                min_sat: 0.55,
                min_fill: 0.50,
            },
            Sensitivity::Medium => Thresholds {
                ratio: 1.8,
                min_red: 80.0,
                min_sat: 0.45,
                min_fill: 0.42,
            },
            Sensitivity::High => Thresholds {
                ratio: 1.4,
                min_red: 60.0,
                min_sat: 0.32,
                min_fill: 0.30,
            },
        }
    }
}

/// Tunables shared by every surface. [`Options::default`] IS the descriptor's
/// declared default set — the block, the web wrapper and the tests all assert
/// against it, so there is one place to change a default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub sensitivity: Sensitivity,
    pub min_radius: u32,
    pub max_radius: u32,
    pub max_regions: u32,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            sensitivity: Sensitivity::Medium,
            min_radius: 3,
            max_radius: 80,
            max_regions: 20,
        }
    }
}

impl Options {
    /// Reject out-of-range or contradictory params BEFORE decoding, so a bad
    /// call costs nothing and the message names the offending param.
    pub fn validate(&self) -> Result<(), String> {
        if self.min_radius < 1 || self.min_radius > MIN_RADIUS_MAX {
            return Err(format!(
                "min_radius must be between 1 and {MIN_RADIUS_MAX} pixels (got {})",
                self.min_radius
            ));
        }
        if self.max_radius < 1 || self.max_radius > MAX_RADIUS_MAX {
            return Err(format!(
                "max_radius must be between 1 and {MAX_RADIUS_MAX} pixels (got {})",
                self.max_radius
            ));
        }
        if self.min_radius > self.max_radius {
            return Err(format!(
                "min_radius ({}) must not exceed max_radius ({})",
                self.min_radius, self.max_radius
            ));
        }
        if self.max_regions < 1 || self.max_regions > MAX_REGIONS_MAX {
            return Err(format!(
                "max_regions must be between 1 and {MAX_REGIONS_MAX} (got {})",
                self.max_regions
            ));
        }
        Ok(())
    }
}

/// One red-eye candidate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Region {
    /// Centroid of the red blob, in pixels from the left edge.
    pub center_x: u32,
    /// Centroid of the red blob, in pixels from the top edge.
    pub center_y: u32,
    /// Radius of the disc with the same area as the blob (2 decimals).
    pub radius_px: f64,
    /// Red pixels in the blob.
    pub area_px: u64,
    /// Mean red channel (0-255) over the blob (2 decimals).
    pub average_red: f64,
    /// 0-1 blend of redness, roundness and squareness (3 decimals).
    pub confidence: f64,
}

/// The full report — exactly the JSON shape every surface returns.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub width: u32,
    pub height: u32,
    /// Candidates that passed every filter, BEFORE `max_regions` clipping.
    pub candidate_count: usize,
    pub sensitivity: &'static str,
    /// Highest confidence first, at most `max_regions` entries.
    pub regions: Vec<Region>,
    pub warnings: Vec<String>,
}

/// Per-component accumulator filled during the flood fill.
struct Blob {
    area: u64,
    sum_x: u64,
    sum_y: u64,
    sum_red: f64,
    sum_ratio: f64,
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
}

/// Detect red-eye candidates in an encoded PNG or JPEG image.
pub fn analyze(bytes: &[u8], opts: &Options) -> Result<Report, String> {
    opts.validate()?;
    if bytes.is_empty() {
        return Err("no image data was provided".into());
    }
    let img = decode(bytes)?;
    let (width, height) = (img.width(), img.height());
    let th = opts.sensitivity.thresholds();

    // 1. redness mask + per-pixel redness ratio (reused for scoring).
    let n = (width as usize) * (height as usize);
    let mut mask = vec![false; n];
    let mut ratios = vec![0f32; n];
    let mut red_pixels: u64 = 0;
    for (i, px) in img.pixels().enumerate() {
        let [r, g, b, a] = px.0;
        if a < ALPHA_THRESHOLD {
            continue;
        }
        let (rf, gf, bf) = (r as f64, g as f64, b as f64);
        if rf <= gf || rf <= bf || rf < th.min_red {
            continue;
        }
        let ratio = rf / ((gf + bf) / 2.0 + 1.0);
        if ratio < th.ratio {
            continue;
        }
        let lo = gf.min(bf);
        let sat = if rf > 0.0 { (rf - lo) / rf } else { 0.0 };
        if sat < th.min_sat {
            continue;
        }
        mask[i] = true;
        ratios[i] = ratio as f32;
        red_pixels += 1;
    }

    // 2. connected components (8-connectivity, explicit stack).
    let blobs = components(&mask, &ratios, &img, width, height);

    // 3. shape + radius filtering, 4. scoring.
    let mut kept: Vec<Region> = Vec::new();
    let mut too_small = 0usize;
    let mut too_large = 0usize;
    let mut wrong_shape = 0usize;
    for blob in &blobs {
        let area = blob.area as f64;
        let radius = (area / std::f64::consts::PI).sqrt();
        if radius < opts.min_radius as f64 {
            too_small += 1;
            continue;
        }
        if radius > opts.max_radius as f64 {
            too_large += 1;
            continue;
        }
        let bw = (blob.max_x - blob.min_x + 1) as f64;
        let bh = (blob.max_y - blob.min_y + 1) as f64;
        let fill = area / (bw * bh);
        let aspect = if bw >= bh { bw / bh } else { bh / bw };
        if fill < th.min_fill || aspect > MAX_ASPECT {
            wrong_shape += 1;
            continue;
        }
        let mean_ratio = blob.sum_ratio / area;
        let mean_red = blob.sum_red / area;
        // Redness saturates at 4x — past that it is "as red as it gets".
        let redness_score = ((mean_ratio - th.ratio) / (4.0 - th.ratio)).clamp(0.0, 1.0);
        // A disc fills pi/4 of its box; both a sparse blob and a solid square
        // are less eye-like than that, so score the DISTANCE from pi/4.
        let roundness_score = (1.0 - (fill - DISC_FILL).abs() / DISC_FILL).clamp(0.0, 1.0);
        let squareness_score = (1.0 / aspect).clamp(0.0, 1.0);
        let confidence =
            0.5 * redness_score + 0.3 * roundness_score + 0.2 * squareness_score;
        kept.push(Region {
            center_x: (blob.sum_x / blob.area) as u32,
            center_y: (blob.sum_y / blob.area) as u32,
            radius_px: round(radius, 2),
            area_px: blob.area,
            average_red: round(mean_red, 2),
            confidence: round(confidence, 3),
        });
    }

    // Deterministic order: confidence, then size, then position.
    kept.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.area_px.cmp(&a.area_px))
            .then(a.center_y.cmp(&b.center_y))
            .then(a.center_x.cmp(&b.center_x))
    });

    let candidate_count = kept.len();
    let mut warnings = Vec::new();
    if candidate_count > opts.max_regions as usize {
        warnings.push(format!(
            "Only the {} highest-confidence regions of {candidate_count} are listed; raise \
             max_regions to see the rest.",
            opts.max_regions
        ));
        kept.truncate(opts.max_regions as usize);
    }
    if candidate_count == 0 {
        warnings.push(format!(
            "No red-eye candidates matched at sensitivity '{}'. Try sensitivity=high, a smaller \
             min_radius, or crop closer to the face.",
            opts.sensitivity.as_str()
        ));
    }
    if too_small > 0 {
        warnings.push(format!(
            "{too_small} red region(s) were smaller than min_radius={} px and were skipped.",
            opts.min_radius
        ));
    }
    if too_large > 0 {
        warnings.push(format!(
            "{too_large} red region(s) were larger than max_radius={} px and were skipped.",
            opts.max_radius
        ));
    }
    if wrong_shape > 0 {
        warnings.push(format!(
            "{wrong_shape} red region(s) were the right size but not pupil-shaped (too elongated \
             or too sparse) and were skipped."
        ));
    }
    let red_percent = if n > 0 {
        red_pixels as f64 * 100.0 / n as f64
    } else {
        0.0
    };
    if red_percent > RED_SCENE_PERCENT {
        warnings.push(format!(
            "Red pixels cover {:.1}% of the image — this looks like a red-dominant scene rather \
             than flash red-eye, so some regions may be background.",
            red_percent
        ));
    }

    Ok(Report {
        width,
        height,
        candidate_count,
        sensitivity: opts.sensitivity.as_str(),
        regions: kept,
        warnings,
    })
}

/// [`analyze`] rendered as the pretty JSON every surface prints.
pub fn analyze_json(bytes: &[u8], opts: &Options) -> Result<String, String> {
    let report = analyze(bytes, opts)?;
    serde_json::to_string_pretty(&report).map_err(|e| format!("could not serialize report: {e}"))
}

/// Iterative 8-connected flood fill over the redness mask.
fn components(
    mask: &[bool],
    ratios: &[f32],
    img: &RgbaImage,
    width: u32,
    height: u32,
) -> Vec<Blob> {
    let mut seen = vec![false; mask.len()];
    let mut blobs = Vec::new();
    let mut stack: Vec<u32> = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.clear();
        stack.push(start as u32);
        let sx = (start as u32) % width;
        let sy = (start as u32) / width;
        let mut blob = Blob {
            area: 0,
            sum_x: 0,
            sum_y: 0,
            sum_red: 0.0,
            sum_ratio: 0.0,
            min_x: sx,
            max_x: sx,
            min_y: sy,
            max_y: sy,
        };
        while let Some(idx) = stack.pop() {
            let x = idx % width;
            let y = idx / width;
            blob.area += 1;
            blob.sum_x += x as u64;
            blob.sum_y += y as u64;
            blob.sum_red += img.get_pixel(x, y).0[0] as f64;
            blob.sum_ratio += ratios[idx as usize] as f64;
            blob.min_x = blob.min_x.min(x);
            blob.max_x = blob.max_x.max(x);
            blob.min_y = blob.min_y.min(y);
            blob.max_y = blob.max_y.max(y);
            let x0 = x.saturating_sub(1);
            let x1 = (x + 1).min(width - 1);
            let y0 = y.saturating_sub(1);
            let y1 = (y + 1).min(height - 1);
            for ny in y0..=y1 {
                for nx in x0..=x1 {
                    let n = (ny * width + nx) as usize;
                    if mask[n] && !seen[n] {
                        seen[n] = true;
                        stack.push(n as u32);
                    }
                }
            }
        }
        blobs.push(blob);
    }
    blobs
}

/// Decode with a header-first size budget so an oversized raster is refused with
/// an actionable message instead of trapping the wasm sandbox on allocation.
fn decode(bytes: &[u8]) -> Result<RgbaImage, String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read the image header: {e}"))?;
    let decoder = reader
        .into_decoder()
        .map_err(|e| format!("could not decode the image (PNG and JPEG are supported): {e}"))?;
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

fn round(v: f64, places: u32) -> f64 {
    let f = 10f64.powi(places as i32);
    (v * f).round() / f
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba};

    fn encode(img: RgbaImage, format: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        // The JPEG encoder refuses RGBA — drop alpha for that format only, so the
        // PNG fixtures keep exercising the transparency path.
        let dynamic = match format {
            ImageFormat::Jpeg => DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(img).to_rgb8()),
            _ => DynamicImage::ImageRgba8(img),
        };
        dynamic.write_to(&mut out, format).unwrap();
        out.into_inner()
    }

    /// A neutral grey frame with filled red discs at the given centres.
    fn eyes(w: u32, h: u32, discs: &[(u32, u32, u32)]) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(w, h, Rgba([128, 128, 128, 255]));
        for &(cx, cy, r) in discs {
            for y in 0..h {
                for x in 0..w {
                    let dx = x as f64 - cx as f64;
                    let dy = y as f64 - cy as f64;
                    if dx * dx + dy * dy <= (r as f64) * (r as f64) {
                        img.put_pixel(x, y, Rgba([225, 30, 30, 255]));
                    }
                }
            }
        }
        img
    }

    #[test]
    fn defaults_are_the_documented_ones() {
        let d = Options::default();
        assert_eq!(d.sensitivity, Sensitivity::Medium);
        assert_eq!(d.min_radius, 3);
        assert_eq!(d.max_radius, 80);
        assert_eq!(d.max_regions, 20);
    }

    #[test]
    fn detects_a_synthetic_red_disc() {
        let png = encode(eyes(64, 64, &[(20, 24, 6)]), ImageFormat::Png);
        let report = analyze(&png, &Options::default()).unwrap();
        assert_eq!((report.width, report.height), (64, 64));
        assert_eq!(report.candidate_count, 1, "{report:?}");
        assert_eq!(report.sensitivity, "medium");
        let region = &report.regions[0];
        assert!((region.center_x as i64 - 20).abs() <= 1, "{region:?}");
        assert!((region.center_y as i64 - 24).abs() <= 1, "{region:?}");
        assert!(
            (region.radius_px - 6.0).abs() <= 1.0,
            "equivalent radius should track the disc: {region:?}"
        );
        assert!(region.area_px > 90 && region.area_px < 145, "{region:?}");
        assert!(region.average_red > 200.0, "{region:?}");
        assert!(region.confidence > 0.6, "{region:?}");
    }

    #[test]
    fn detects_a_pair_of_eyes_in_a_jpeg() {
        let jpeg = encode(eyes(96, 64, &[(30, 30, 5), (66, 30, 5)]), ImageFormat::Jpeg);
        let report = analyze(&jpeg, &Options::default()).unwrap();
        assert_eq!(report.candidate_count, 2, "{report:?}");
        let mut xs: Vec<u32> = report.regions.iter().map(|r| r.center_x).collect();
        xs.sort_unstable();
        assert!((xs[0] as i64 - 30).abs() <= 2, "{xs:?}");
        assert!((xs[1] as i64 - 66).abs() <= 2, "{xs:?}");
    }

    #[test]
    fn a_neutral_image_reports_no_detections() {
        let mut img = RgbaImage::new(48, 48);
        for (x, y, px) in img.enumerate_pixels_mut() {
            let v = (((x + y) * 2) % 256) as u8;
            *px = Rgba([v, v, v, 255]);
        }
        let report = analyze(&encode(img, ImageFormat::Png), &Options::default()).unwrap();
        assert_eq!(report.candidate_count, 0);
        assert!(report.regions.is_empty());
        assert!(
            report.warnings.iter().any(|w| w.contains("No red-eye")),
            "{report:?}"
        );
    }

    #[test]
    fn radius_window_filters_candidates_and_warns() {
        let png = encode(eyes(80, 80, &[(40, 40, 6)]), ImageFormat::Png);
        let opts = Options {
            min_radius: 20,
            ..Options::default()
        };
        let report = analyze(&png, &opts).unwrap();
        assert_eq!(report.candidate_count, 0);
        assert!(
            report.warnings.iter().any(|w| w.contains("min_radius=20")),
            "{report:?}"
        );

        let opts = Options {
            max_radius: 2,
            min_radius: 1,
            ..Options::default()
        };
        let report = analyze(&png, &opts).unwrap();
        assert_eq!(report.candidate_count, 0);
        assert!(
            report.warnings.iter().any(|w| w.contains("max_radius=2")),
            "{report:?}"
        );
    }

    #[test]
    fn max_regions_clips_and_warns_without_losing_the_count() {
        let discs: Vec<(u32, u32, u32)> = (0..6).map(|i| (12 + i * 20, 20, 5)).collect();
        let png = encode(eyes(140, 44, &discs), ImageFormat::Png);
        let opts = Options {
            max_regions: 2,
            ..Options::default()
        };
        let report = analyze(&png, &opts).unwrap();
        assert_eq!(report.candidate_count, 6, "{report:?}");
        assert_eq!(report.regions.len(), 2);
        assert!(
            report.warnings.iter().any(|w| w.contains("max_regions")),
            "{report:?}"
        );
    }

    #[test]
    fn sensitivity_widens_the_net() {
        // Dull, desaturated red — below the medium thresholds, above the high ones.
        let mut img = RgbaImage::from_pixel(48, 48, Rgba([120, 120, 120, 255]));
        for y in 18..30 {
            for x in 18..30 {
                let dx = x as f64 - 23.5;
                let dy = y as f64 - 23.5;
                if dx * dx + dy * dy <= 36.0 {
                    img.put_pixel(x, y, Rgba([100, 55, 55, 255]));
                }
            }
        }
        let png = encode(img, ImageFormat::Png);
        let strict = analyze(
            &png,
            &Options {
                sensitivity: Sensitivity::Low,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(strict.candidate_count, 0, "{strict:?}");
        let loose = analyze(
            &png,
            &Options {
                sensitivity: Sensitivity::High,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(loose.candidate_count, 1, "{loose:?}");
        assert_eq!(loose.sensitivity, "high");
    }

    #[test]
    fn an_elongated_red_bar_is_not_pupil_shaped() {
        let mut img = RgbaImage::from_pixel(80, 40, Rgba([128, 128, 128, 255]));
        for y in 16..24 {
            for x in 4..76 {
                img.put_pixel(x, y, Rgba([230, 20, 20, 255]));
            }
        }
        let report = analyze(&encode(img, ImageFormat::Png), &Options::default()).unwrap();
        assert_eq!(report.candidate_count, 0, "{report:?}");
        assert!(
            report.warnings.iter().any(|w| w.contains("pupil-shaped")),
            "{report:?}"
        );
    }

    #[test]
    fn transparent_pixels_never_become_candidates() {
        let mut img = RgbaImage::from_pixel(40, 40, Rgba([255, 0, 0, 0]));
        for y in 0..40 {
            for x in 0..40 {
                if x < 4 || y < 4 {
                    img.put_pixel(x, y, Rgba([128, 128, 128, 255]));
                }
            }
        }
        let report = analyze(&encode(img, ImageFormat::Png), &Options::default()).unwrap();
        assert_eq!(report.candidate_count, 0, "{report:?}");
    }

    #[test]
    fn invalid_bytes_are_rejected_with_an_actionable_message() {
        let err = analyze(b"this is not an image at all", &Options::default()).unwrap_err();
        assert!(err.contains("decode") || err.contains("header"), "{err}");
        let err = analyze(&[], &Options::default()).unwrap_err();
        assert!(err.contains("no image data"), "{err}");
    }

    #[test]
    fn params_are_validated_before_any_decoding() {
        let bad = |o: Options| analyze(b"", &o).unwrap_err();
        assert!(bad(Options {
            min_radius: 0,
            ..Options::default()
        })
        .contains("min_radius"));
        assert!(bad(Options {
            min_radius: 81,
            ..Options::default()
        })
        .contains("min_radius"));
        assert!(bad(Options {
            max_radius: 301,
            ..Options::default()
        })
        .contains("max_radius"));
        assert!(bad(Options {
            max_radius: 0,
            min_radius: 1,
            ..Options::default()
        })
        .contains("max_radius"));
        assert!(bad(Options {
            min_radius: 40,
            max_radius: 10,
            ..Options::default()
        })
        .contains("must not exceed"));
        assert!(bad(Options {
            max_regions: 0,
            ..Options::default()
        })
        .contains("max_regions"));
        assert!(bad(Options {
            max_regions: 101,
            ..Options::default()
        })
        .contains("max_regions"));
    }

    #[test]
    fn sensitivity_parsing_is_forgiving_but_bounded() {
        assert_eq!(Sensitivity::parse("LOW").unwrap(), Sensitivity::Low);
        assert_eq!(Sensitivity::parse(" high ").unwrap(), Sensitivity::High);
        assert_eq!(Sensitivity::parse("").unwrap(), Sensitivity::Medium);
        let err = Sensitivity::parse("extreme").unwrap_err();
        assert!(err.contains("low, medium or high"), "{err}");
    }

    #[test]
    fn json_output_is_pretty_and_has_every_documented_key() {
        let png = encode(eyes(64, 64, &[(32, 32, 6)]), ImageFormat::Png);
        let json = analyze_json(&png, &Options::default()).unwrap();
        assert!(json.starts_with("{\n"), "pretty-printed: {json}");
        for key in [
            "\"width\"",
            "\"height\"",
            "\"candidate_count\"",
            "\"sensitivity\"",
            "\"regions\"",
            "\"warnings\"",
            "\"center_x\"",
            "\"center_y\"",
            "\"radius_px\"",
            "\"area_px\"",
            "\"average_red\"",
            "\"confidence\"",
        ] {
            assert!(json.contains(key), "missing {key} in {json}");
        }
    }

    #[test]
    fn a_fully_red_frame_warns_about_a_red_dominant_scene() {
        let img = RgbaImage::from_pixel(64, 64, Rgba([240, 20, 20, 255]));
        let report = analyze(
            &encode(img, ImageFormat::Png),
            &Options {
                max_radius: 300,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(
            report.warnings.iter().any(|w| w.contains("red-dominant")),
            "{report:?}"
        );
    }
}
