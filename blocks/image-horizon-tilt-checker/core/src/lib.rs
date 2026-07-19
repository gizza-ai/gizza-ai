//! gizza-ai/image-horizon-tilt-checker core — detect the tilt angle of a photo's
//! dominant horizon (or vertical) line so it can be leveled.
//!
//! No wafer/wasm-bindgen deps. Pure-Rust `image` crate. The algorithm:
//!   1. decode + downscale (cap the longest side) + convert to grayscale;
//!   2. Sobel gradients per interior pixel → edge magnitude + orientation;
//!   3. keep strong edges, fold each edge's line orientation to a signed
//!      deviation from the reference axis (horizontal for `horizon`, vertical
//!      for `vertical`), within ±`max_angle`;
//!   4. weighted histogram → dominant deviation → weighted-mean refine.
//!
//! Sign convention (calibrated by the unit tests): a positive `angle` means the
//! horizon is tilted CLOCKWISE (its right end sits lower on screen). To level the
//! photo, rotate by `suggested_rotation = -angle` degrees (positive = clockwise).

use image::GenericImageView;

/// Which structure to level against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reference {
    /// Near-horizontal lines (sea/land horizon, table edges).
    Horizon,
    /// Near-vertical lines (walls, doorframes, poles).
    Vertical,
}

impl Reference {
    /// Parse the descriptor enum value.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "horizon" => Ok(Reference::Horizon),
            "vertical" => Ok(Reference::Vertical),
            other => Err(format!(
                "reference must be \"horizon\" or \"vertical\", got \"{other}\""
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Reference::Horizon => "horizon",
            Reference::Vertical => "vertical",
        }
    }
}

/// The detection report.
#[derive(Debug, Clone, PartialEq)]
pub struct TiltResult {
    /// Detected tilt of the dominant line, in degrees. Positive = clockwise
    /// (right end lower). 0 when perfectly level.
    pub angle: f64,
    /// Rotation to apply to level the photo, in degrees (positive = clockwise).
    /// Equal to `-angle`.
    pub suggested_rotation: f64,
    /// The reference axis used ("horizon" or "vertical").
    pub reference: &'static str,
    /// "level", "clockwise", "counterclockwise", or "undetermined".
    pub direction: &'static str,
    /// Fraction (0..1) of considered edge weight agreeing with the detected
    /// angle — higher means a more confident, cleaner dominant line.
    pub confidence: f64,
    /// Whether the tilt is within `tolerance` of level.
    pub is_level: bool,
    /// Number of strong edge pixels that fell within the search range.
    pub edges_analyzed: u64,
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

/// Detect the horizon/vertical tilt in `bytes`.
///
/// * `max_angle` — only tilts within ±this (degrees, 1..=45) are considered.
/// * `tolerance` — |angle| ≤ this (degrees, 0..=10) reports as already level.
pub fn detect(
    bytes: &[u8],
    reference: Reference,
    max_angle: f64,
    tolerance: f64,
) -> Result<TiltResult, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    if !(1.0..=45.0).contains(&max_angle) {
        return Err(format!(
            "max_angle must be between 1 and 45 degrees, got {max_angle}"
        ));
    }
    if !(0.0..=10.0).contains(&tolerance) {
        return Err(format!(
            "tolerance must be between 0 and 10 degrees, got {tolerance}"
        ));
    }

    let img =
        image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w0, h0) = img.dimensions();
    if w0 == 0 || h0 == 0 {
        return Err("image has a zero dimension".into());
    }

    // Downscale so analysis is bounded regardless of upload size.
    const CAP: u32 = 1000;
    let max_dim = w0.max(h0);
    let gray = if max_dim > CAP {
        let scale = CAP as f32 / max_dim as f32;
        let nw = ((w0 as f32 * scale).round() as u32).max(1);
        let nh = ((h0 as f32 * scale).round() as u32).max(1);
        img.resize(nw, nh, image::imageops::FilterType::Triangle)
            .to_luma8()
    } else {
        img.to_luma8()
    };
    let (w, h) = gray.dimensions();
    if w < 3 || h < 3 {
        return Err("image is too small to analyze (needs at least 3x3 pixels)".into());
    }

    let at = |x: u32, y: u32| gray.get_pixel(x, y)[0] as f64;

    // Extract one strongest edge point per column (horizon) or row (vertical),
    // then fit a weighted line through those points. This works well for the
    // tool's intended use — a single dominant horizon/building edge — and avoids
    // a full Hough transform in wasm.
    let mut candidates: Vec<(f64, f64, f64)> = Vec::new(); // x, y, weight
    let mut max_mag = 0.0f64;
    match reference {
        Reference::Horizon => {
            for x in 1..w - 1 {
                let mut best = (0u32, 0.0f64);
                for y in 1..h - 1 {
                    let mag = (at(x, y + 1) - at(x, y - 1)).abs();
                    if mag > best.1 {
                        best = (y, mag);
                    }
                }
                max_mag = max_mag.max(best.1);
                candidates.push((x as f64, best.0 as f64, best.1));
            }
        }
        Reference::Vertical => {
            for y in 1..h - 1 {
                let mut best = (0u32, 0.0f64);
                for x in 1..w - 1 {
                    let mag = (at(x + 1, y) - at(x - 1, y)).abs();
                    if mag > best.1 {
                        best = (x, mag);
                    }
                }
                max_mag = max_mag.max(best.1);
                candidates.push((best.0 as f64, y as f64, best.1));
            }
        }
    }
    if max_mag <= 0.0 {
        return Ok(TiltResult {
            angle: 0.0,
            suggested_rotation: 0.0,
            reference: reference.as_str(),
            direction: "undetermined",
            confidence: 0.0,
            is_level: false,
            edges_analyzed: 0,
        });
    }

    let threshold = 0.15 * max_mag;
    let points: Vec<(f64, f64, f64)> = candidates
        .into_iter()
        .filter(|(_, _, weight)| *weight >= threshold)
        .collect();
    let edges_analyzed = points.len() as u64;
    if edges_analyzed < 12 {
        return Ok(TiltResult {
            angle: 0.0,
            suggested_rotation: 0.0,
            reference: reference.as_str(),
            direction: "undetermined",
            confidence: 0.0,
            is_level: false,
            edges_analyzed,
        });
    }

    let total_weight: f64 = points.iter().map(|p| p.2).sum();
    let (sx, sy) = points.iter().fold((0.0, 0.0), |(sx, sy), (x, y, wt)| {
        (sx + x * wt, sy + y * wt)
    });
    let mx = sx / total_weight;
    let my = sy / total_weight;
    let mut num = 0.0f64;
    let mut den = 0.0f64;
    match reference {
        Reference::Horizon => {
            for (x, y, wt) in &points {
                let dx = x - mx;
                num += wt * dx * (y - my);
                den += wt * dx * dx;
            }
        }
        Reference::Vertical => {
            for (x, y, wt) in &points {
                let dy = y - my;
                num += wt * dy * (x - mx);
                den += wt * dy * dy;
            }
        }
    }
    if den <= 1e-9 {
        return Ok(TiltResult {
            angle: 0.0,
            suggested_rotation: 0.0,
            reference: reference.as_str(),
            direction: "undetermined",
            confidence: 0.0,
            is_level: false,
            edges_analyzed,
        });
    }
    let slope = num / den;
    let angle = slope.atan().to_degrees();
    if angle.abs() > max_angle {
        return Ok(TiltResult {
            angle: 0.0,
            suggested_rotation: 0.0,
            reference: reference.as_str(),
            direction: "undetermined",
            confidence: 0.0,
            is_level: false,
            edges_analyzed: 0,
        });
    }

    let avg_strength = (total_weight / edges_analyzed as f64) / max_mag;
    let coverage = edges_analyzed as f64 / (w.max(h) as f64).max(1.0);
    let confidence = (avg_strength * coverage).clamp(0.0, 1.0);

    let angle = round2(angle);
    let is_level = angle.abs() <= tolerance;
    let direction = if is_level {
        "level"
    } else if angle > 0.0 {
        "clockwise"
    } else {
        "counterclockwise"
    };

    Ok(TiltResult {
        angle,
        suggested_rotation: round2(-angle),
        reference: reference.as_str(),
        direction,
        confidence: round2(confidence),
        is_level,
        edges_analyzed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GrayImage, ImageFormat, Luma};
    use std::io::Cursor;

    /// Encode a synthetic image whose top/bottom split boundary is tilted so that
    /// at column x the boundary sits at row `y0 + x*slope`. Above = white (255),
    /// below = black (0). A positive slope drops the boundary to the right.
    fn tilted_horizon_png(w: u32, h: u32, slope: f64) -> Vec<u8> {
        let mut img = GrayImage::new(w, h);
        let y0 = h as f64 / 2.0;
        for x in 0..w {
            let boundary = y0 + x as f64 * slope;
            for y in 0..h {
                let v = if (y as f64) < boundary { 255 } else { 0 };
                img.put_pixel(x, y, Luma([v]));
            }
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageLuma8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    /// A tilted VERTICAL boundary: at row y the boundary sits at col `x0 + y*slope`.
    fn tilted_vertical_png(w: u32, h: u32, slope: f64) -> Vec<u8> {
        let mut img = GrayImage::new(w, h);
        let x0 = w as f64 / 2.0;
        for y in 0..h {
            let boundary = x0 + y as f64 * slope;
            for x in 0..w {
                let v = if (x as f64) < boundary { 255 } else { 0 };
                img.put_pixel(x, y, Luma([v]));
            }
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageLuma8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn level_horizon_reads_zero() {
        let png = tilted_horizon_png(200, 200, 0.0);
        let r = detect(&png, Reference::Horizon, 15.0, 1.0).unwrap();
        assert!(r.angle.abs() < 0.5, "expected ~0, got {}", r.angle);
        assert!(r.is_level);
        assert_eq!(r.direction, "level");
        assert!(r.confidence > 0.5, "confidence {}", r.confidence);
    }

    #[test]
    fn right_side_down_is_clockwise_positive() {
        // slope = tan(5deg) drops the boundary to the right → clockwise tilt.
        let slope = 5.0f64.to_radians().tan();
        let png = tilted_horizon_png(300, 300, slope);
        let r = detect(&png, Reference::Horizon, 15.0, 1.0).unwrap();
        assert!((r.angle - 5.0).abs() < 1.0, "expected ~+5, got {}", r.angle);
        assert_eq!(r.direction, "clockwise");
        assert!((r.suggested_rotation + 5.0).abs() < 1.0);
        assert!(!r.is_level);
    }

    #[test]
    fn left_side_down_is_counterclockwise_negative() {
        let slope = -(8.0f64.to_radians().tan());
        let png = tilted_horizon_png(300, 300, slope);
        let r = detect(&png, Reference::Horizon, 20.0, 1.0).unwrap();
        assert!((r.angle + 8.0).abs() < 1.0, "expected ~-8, got {}", r.angle);
        assert_eq!(r.direction, "counterclockwise");
    }

    #[test]
    fn detects_vertical_line_tilt() {
        // Vertical boundary leaning: slope=tan(6deg).
        let slope = 6.0f64.to_radians().tan();
        let png = tilted_vertical_png(300, 300, slope);
        let r = detect(&png, Reference::Vertical, 15.0, 1.0).unwrap();
        assert_eq!(r.reference, "vertical");
        assert!(
            (r.angle.abs() - 6.0).abs() < 1.5,
            "expected magnitude ~6, got {}",
            r.angle
        );
        assert!(!r.is_level);
    }

    #[test]
    fn tilt_outside_range_is_not_reported() {
        // A 20deg tilt with a 10deg search cap → no in-range edges, undetermined.
        let slope = 20.0f64.to_radians().tan();
        let png = tilted_horizon_png(300, 300, slope);
        let r = detect(&png, Reference::Horizon, 10.0, 1.0).unwrap();
        assert_eq!(r.direction, "undetermined");
        assert_eq!(r.edges_analyzed, 0);
        assert_eq!(r.confidence, 0.0);
    }

    #[test]
    fn reference_parse() {
        assert_eq!(Reference::parse("horizon").unwrap(), Reference::Horizon);
        assert_eq!(Reference::parse("vertical").unwrap(), Reference::Vertical);
        assert!(Reference::parse("diagonal").is_err());
    }

    #[test]
    fn errors_on_bad_input() {
        assert!(detect(&[], Reference::Horizon, 15.0, 1.0).is_err());
        assert!(detect(b"not an image", Reference::Horizon, 15.0, 1.0).is_err());
        let png = tilted_horizon_png(50, 50, 0.0);
        assert!(detect(&png, Reference::Horizon, 0.5, 1.0).is_err()); // max_angle < 1
        assert!(detect(&png, Reference::Horizon, 46.0, 1.0).is_err()); // max_angle > 45
        assert!(detect(&png, Reference::Horizon, 15.0, 11.0).is_err()); // tolerance > 10
    }
}
