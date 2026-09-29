//! aspect-ratio-validator core — reduce a width×height to its aspect ratio,
//! name the nearest standard ratio, and check it against a target within a
//! tolerance. Pure arithmetic: no wafer, no wasm-bindgen, no I/O, so the chat
//! block, the CLI and the browser page all run this exact code.

use serde::Serialize;

/// Largest accepted pixel dimension (a 1000×1000 megapixel grid is already far
/// past any real asset; the cap keeps the reduced ratio and the crop/pad maths
/// inside comfortable float precision).
pub const MAX_DIMENSION: f64 = 1_000_000.0;
/// Default slack around the target ratio, in percent of the target.
pub const DEFAULT_TOLERANCE_PERCENT: f64 = 1.0;
/// Upper bound on the tolerance (100% of the target ratio — anything larger
/// would pass every input).
pub const MAX_TOLERANCE_PERCENT: f64 = 100.0;

/// Inputs to [`analyze`].
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Width in pixels (any consistent unit works — a ratio is unit-free).
    pub width: f64,
    /// Height in the same unit as `width`.
    pub height: f64,
    /// Target ratio to check against — `"16:9"`, `"1.85:1"`, `"4/5"`,
    /// `"1920x1080"` or a bare decimal like `"1.7778"`. Empty = report only.
    pub target: String,
    /// Allowed deviation from the target, in percent of the target ratio.
    pub tolerance_percent: f64,
    /// Treat a rotated target as a match (9:16 satisfies a 16:9 target).
    pub orientation_agnostic: bool,
    /// Round the crop/pad suggestions to even numbers (most video encoders
    /// reject odd dimensions).
    pub even_dimensions: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            target: String::new(),
            tolerance_percent: DEFAULT_TOLERANCE_PERCENT,
            orientation_agnostic: false,
            even_dimensions: false,
        }
    }
}

impl Options {
    /// Reject impossible dimensions and rules before any maths runs.
    pub fn validate(&self) -> Result<(), String> {
        check_dimension(self.width, "width")?;
        check_dimension(self.height, "height")?;
        if !self.tolerance_percent.is_finite() || self.tolerance_percent < 0.0 {
            return Err("tolerance_percent must not be negative".into());
        }
        if self.tolerance_percent > MAX_TOLERANCE_PERCENT {
            return Err(format!(
                "tolerance_percent must be at most {MAX_TOLERANCE_PERCENT} (a larger window accepts every ratio)"
            ));
        }
        if !self.target.trim().is_empty() {
            parse_ratio(&self.target)?;
        }
        Ok(())
    }
}

fn check_dimension(v: f64, label: &str) -> Result<(), String> {
    if !v.is_finite() || v <= 0.0 {
        return Err(format!("{label} must be a positive number of pixels"));
    }
    if v > MAX_DIMENSION {
        return Err(format!(
            "{label} must be at most {} pixels",
            MAX_DIMENSION as u64
        ));
    }
    Ok(())
}

/// The verdict + everything a caller needs to act on it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Report {
    /// `PASS`, `FAIL`, or `INFO` when no target was given.
    pub status: String,
    /// Absent in report-only mode (no target).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pass: Option<bool>,
    pub width: f64,
    pub height: f64,
    /// Greatest-common-divisor reduced ratio, e.g. `16:9`. Falls back to the
    /// `x:1` form when the dimensions are not whole numbers.
    pub ratio: String,
    /// width ÷ height, rounded to 4 decimals.
    pub ratio_decimal: f64,
    /// The same ratio written as `1.778:1`.
    pub ratio_x_to_1: String,
    /// `landscape`, `portrait`, or `square`.
    pub orientation: String,
    /// Closest entry in the standard-ratio table, e.g. `16:9`.
    pub nearest_standard: String,
    /// Friendly name of that entry, e.g. `Widescreen HD video`.
    pub nearest_standard_name: String,
    /// How far the actual ratio sits from that standard, in percent.
    pub nearest_standard_deviation_percent: f64,
    /// Canonical form of the requested target, e.g. `16:9`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_decimal: Option<f64>,
    /// Signed deviation from the target: positive = wider than the target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deviation_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance_percent: Option<f64>,
    /// `ok`, `too_wide`, or `too_tall`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// True when the match was made against the rotated target
    /// (`orientation_agnostic`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation_flipped: Option<bool>,
    /// Largest crop inside the current frame that hits the target ratio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop_width: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop_height: Option<u64>,
    /// Share of the frame area that crop would discard.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop_loss_percent: Option<f64>,
    /// Smallest pad around the current frame that hits the target ratio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pad_width: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pad_height: Option<u64>,
    /// One-line human verdict.
    pub summary: String,
}

/// One row of the standard-ratio reference table.
struct Standard {
    label: &'static str,
    name: &'static str,
    value: f64,
}

/// Standard display / image / cinema ratios, landscape then portrait. The
/// nearest entry is reported for every input, so a 1920×1081 asset is described
/// as "16:9, off by 0.09%" rather than as the meaningless 1920:1081.
const STANDARDS: &[Standard] = &[
    Standard { label: "1:1", name: "Square", value: 1.0 },
    Standard { label: "5:4", name: "Classic monitor / 8x10 print", value: 1.25 },
    Standard { label: "4:3", name: "Standard / classic TV", value: 4.0 / 3.0 },
    Standard { label: "1.414:1", name: "ISO A-series paper, landscape", value: 1.414_213_6 },
    Standard { label: "3:2", name: "35 mm photo / DSLR", value: 1.5 },
    Standard { label: "16:10", name: "Widescreen monitor", value: 1.6 },
    Standard { label: "5:3", name: "Super 16 / 15:9", value: 5.0 / 3.0 },
    Standard { label: "16:9", name: "Widescreen HD video", value: 16.0 / 9.0 },
    Standard { label: "1.85:1", name: "Cinema flat", value: 1.85 },
    Standard { label: "1.91:1", name: "Social link card", value: 1.91 },
    Standard { label: "2:1", name: "Univisium", value: 2.0 },
    Standard { label: "21:9", name: "Ultrawide (64:27)", value: 64.0 / 27.0 },
    Standard { label: "2.35:1", name: "CinemaScope, classic", value: 2.35 },
    Standard { label: "2.39:1", name: "Anamorphic scope", value: 2.39 },
    Standard { label: "2.76:1", name: "Ultra Panavision", value: 2.76 },
    Standard { label: "3:1", name: "Panorama", value: 3.0 },
    Standard { label: "32:9", name: "Super ultrawide", value: 32.0 / 9.0 },
    Standard { label: "4:5", name: "Portrait photo / social feed", value: 0.8 },
    Standard { label: "3:4", name: "Portrait standard", value: 0.75 },
    Standard { label: "1:1.414", name: "ISO A-series paper, portrait", value: 1.0 / 1.414_213_6 },
    Standard { label: "2:3", name: "Portrait 35 mm photo", value: 2.0 / 3.0 },
    Standard { label: "10:16", name: "Portrait widescreen monitor", value: 0.625 },
    Standard { label: "9:16", name: "Vertical video / stories", value: 9.0 / 16.0 },
    Standard { label: "1:2", name: "Tall portrait", value: 0.5 },
    Standard { label: "9:19.5", name: "Tall phone screen", value: 9.0 / 19.5 },
];

/// Parse a target ratio written as `16:9`, `16/9`, `1920x1080`, `1.85:1`, or a
/// bare decimal like `1.7778`.
pub fn parse_ratio(s: &str) -> Result<f64, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("target must not be empty".into());
    }
    let cleaned = t.to_ascii_lowercase().replace('×', "x");
    let sep = cleaned.find([':', '/', 'x']);
    let value = match sep {
        Some(i) => {
            let (a, b) = cleaned.split_at(i);
            let b = &b[1..];
            let w = parse_part(a, t)?;
            let h = parse_part(b, t)?;
            if h == 0.0 {
                return Err(format!(
                    "target {t:?} divides by zero — the second number must be greater than 0"
                ));
            }
            w / h
        }
        None => parse_part(&cleaned, t)?,
    };
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("target {t:?} must describe a positive ratio"));
    }
    Ok(value)
}

fn parse_part(s: &str, whole: &str) -> Result<f64, String> {
    let t = s.trim();
    t.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or_else(|| {
            format!(
                "could not read target {whole:?} — use a form like 16:9, 4/5, 1.85:1, 1920x1080 or 1.7778"
            )
        })
}

/// Analyse the dimensions and, when a target is given, judge them against it.
pub fn analyze(opts: &Options) -> Result<Report, String> {
    opts.validate()?;
    let (w, h) = (opts.width, opts.height);
    let actual = w / h;

    let orientation = if (w - h).abs() < f64::EPSILON * w.max(h).max(1.0) {
        "square"
    } else if w > h {
        "landscape"
    } else {
        "portrait"
    };
    let (near, near_dev) = nearest_standard(actual);

    let mut report = Report {
        status: "INFO".into(),
        pass: None,
        width: round4(w),
        height: round4(h),
        ratio: reduced_ratio(w, h),
        ratio_decimal: round4(actual),
        ratio_x_to_1: format!("{}:1", trim_num(round4(actual))),
        orientation: orientation.into(),
        nearest_standard: near.label.into(),
        nearest_standard_name: near.name.into(),
        nearest_standard_deviation_percent: round3(near_dev),
        target_ratio: None,
        target_decimal: None,
        deviation_percent: None,
        tolerance_percent: None,
        reason: None,
        orientation_flipped: None,
        crop_width: None,
        crop_height: None,
        crop_loss_percent: None,
        pad_width: None,
        pad_height: None,
        summary: String::new(),
    };

    let target_raw = opts.target.trim();
    if target_raw.is_empty() {
        report.summary = format!(
            "{} ({}) is {} — {}, closest standard {} ({}), {}% away.",
            report.ratio,
            report.ratio_x_to_1,
            orientation,
            format_dims(w, h),
            near.label,
            near.name,
            trim_num(report.nearest_standard_deviation_percent),
        );
        return Ok(report);
    }

    let target = parse_ratio(target_raw)?;
    // orientation_agnostic: judge against whichever of the target and its
    // rotation is closer, so a 1080x1920 asset can satisfy a "16:9" spec.
    let flipped = 1.0 / target;
    let use_flipped =
        opts.orientation_agnostic && (actual - flipped).abs() < (actual - target).abs();
    let effective = if use_flipped { flipped } else { target };

    let deviation = (actual / effective - 1.0) * 100.0;
    let pass = deviation.abs() <= opts.tolerance_percent + 1e-9;
    let reason = if pass {
        "ok"
    } else if deviation > 0.0 {
        "too_wide"
    } else {
        "too_tall"
    };

    // Crop = the largest frame INSIDE the current one at the target ratio;
    // pad = the smallest frame that CONTAINS it. One of the two dimensions is
    // always unchanged, which is exactly what a crop/pad filter needs.
    let (cw, ch) = if actual > effective {
        (h * effective, h)
    } else {
        (w, w / effective)
    };
    let (pw, ph) = if actual > effective {
        (w, w / effective)
    } else {
        (h * effective, h)
    };
    let (cw, ch) = (snap(cw, opts.even_dimensions), snap(ch, opts.even_dimensions));
    let (pw, ph) = (snap(pw, opts.even_dimensions), snap(ph, opts.even_dimensions));
    let loss = (1.0 - (cw as f64 * ch as f64) / (w * h)).max(0.0) * 100.0;

    report.status = if pass { "PASS" } else { "FAIL" }.into();
    report.pass = Some(pass);
    report.target_ratio = Some(canonical_target(target_raw, effective));
    report.target_decimal = Some(round4(effective));
    report.deviation_percent = Some(round3(deviation));
    report.tolerance_percent = Some(round4(opts.tolerance_percent));
    report.reason = Some(reason.into());
    report.orientation_flipped = Some(use_flipped);
    report.crop_width = Some(cw);
    report.crop_height = Some(ch);
    report.crop_loss_percent = Some(round3(loss));
    report.pad_width = Some(pw);
    report.pad_height = Some(ph);
    report.summary = summary(&report, opts, use_flipped);
    Ok(report)
}

/// [`analyze`] with the report serialized as pretty JSON.
pub fn analyze_json(opts: &Options) -> Result<String, String> {
    let report = analyze(opts)?;
    serde_json::to_string_pretty(&report).map_err(|e| format!("serialize report: {e}"))
}

fn summary(r: &Report, opts: &Options, flipped: bool) -> String {
    let target = r.target_ratio.clone().unwrap_or_default();
    let dev = r.deviation_percent.unwrap_or(0.0);
    let rotated = if flipped { " (rotated)" } else { "" };
    if r.pass == Some(true) {
        return format!(
            "PASS — {} is {} ({}), within {}% of the {} target{}.",
            format_dims(opts.width, opts.height),
            r.ratio,
            r.ratio_x_to_1,
            trim_num(round4(opts.tolerance_percent)),
            target,
            rotated,
        );
    }
    let direction = if dev > 0.0 { "wide" } else { "tall" };
    format!(
        "FAIL — {} is {} ({}), {}% too {} for the {} target{} (tolerance {}%). Crop to {}x{} or pad to {}x{}.",
        format_dims(opts.width, opts.height),
        r.ratio,
        r.ratio_x_to_1,
        trim_num(round3(dev.abs())),
        direction,
        target,
        rotated,
        trim_num(round4(opts.tolerance_percent)),
        r.crop_width.unwrap_or(0),
        r.crop_height.unwrap_or(0),
        r.pad_width.unwrap_or(0),
        r.pad_height.unwrap_or(0),
    )
}

/// Echo the target the way the caller wrote it when that is already a ratio
/// form, otherwise render the decimal as `x:1`. When the rotated target was
/// used, the canonical form is rotated too so the report is unambiguous.
fn canonical_target(raw: &str, effective: f64) -> String {
    let t = raw.trim();
    let written_value = parse_ratio(t).unwrap_or(effective);
    if (written_value - effective).abs() < 1e-9 && t.contains([':', '/', 'x', 'X', '×']) {
        return t.to_string();
    }
    let (near, dev) = nearest_standard(effective);
    if dev < 0.05 {
        return near.label.to_string();
    }
    format!("{}:1", trim_num(round4(effective)))
}

fn nearest_standard(value: f64) -> (&'static Standard, f64) {
    let mut best = &STANDARDS[0];
    let mut best_dev = f64::INFINITY;
    for s in STANDARDS {
        let dev = ((value / s.value) - 1.0).abs() * 100.0;
        if dev < best_dev {
            best_dev = dev;
            best = s;
        }
    }
    (best, best_dev)
}

/// `16:9` when both dimensions are whole numbers, else the decimal `x:1` form.
fn reduced_ratio(w: f64, h: f64) -> String {
    if w.fract() == 0.0 && h.fract() == 0.0 {
        let (wi, hi) = (w as u64, h as u64);
        let g = gcd(wi, hi).max(1);
        format!("{}:{}", wi / g, hi / g)
    } else {
        format!("{}:1", trim_num(round4(w / h)))
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Round a suggested dimension to whole (or even) pixels, never below 1 (2 when
/// even dimensions are required).
fn snap(v: f64, even: bool) -> u64 {
    if even {
        let n = (v / 2.0).round() * 2.0;
        return (n.max(2.0)) as u64;
    }
    (v.round().max(1.0)) as u64
}

fn format_dims(w: f64, h: f64) -> String {
    format!("{}x{}", trim_num(round4(w)), trim_num(round4(h)))
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

/// Render a number without trailing zeros: `16` not `16.0000`, `1.778` not `1.7780`.
fn trim_num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(w: f64, h: f64, target: &str) -> Options {
        Options {
            width: w,
            height: h,
            target: target.into(),
            ..Options::default()
        }
    }

    #[test]
    fn exact_match_passes_and_reduces_the_ratio() {
        let r = analyze(&opts(1920.0, 1080.0, "16:9")).unwrap();
        assert_eq!(r.status, "PASS");
        assert_eq!(r.pass, Some(true));
        assert_eq!(r.ratio, "16:9");
        assert_eq!(r.ratio_decimal, 1.7778);
        assert_eq!(r.ratio_x_to_1, "1.7778:1");
        assert_eq!(r.orientation, "landscape");
        assert_eq!(r.nearest_standard, "16:9");
        assert_eq!(r.nearest_standard_name, "Widescreen HD video");
        assert_eq!(r.deviation_percent, Some(0.0));
        assert_eq!(r.reason.as_deref(), Some("ok"));
        assert!(r.summary.starts_with("PASS — 1920x1080 is 16:9"), "{}", r.summary);
    }

    #[test]
    fn a_ratio_outside_the_tolerance_fails_with_crop_and_pad_fixes() {
        // 1600x1200 is 4:3 — far too tall for a 16:9 slot.
        let r = analyze(&opts(1600.0, 1200.0, "16:9")).unwrap();
        assert_eq!(r.status, "FAIL");
        assert_eq!(r.pass, Some(false));
        assert_eq!(r.reason.as_deref(), Some("too_tall"));
        assert_eq!(r.ratio, "4:3");
        // Crop keeps the width and trims the height; pad keeps the height and
        // widens the frame.
        assert_eq!((r.crop_width, r.crop_height), (Some(1600), Some(900)));
        assert_eq!((r.pad_width, r.pad_height), (Some(2133), Some(1200)));
        assert_eq!(r.crop_loss_percent, Some(25.0));
        assert!(r.deviation_percent.unwrap() < 0.0);
        assert!(r.summary.contains("Crop to 1600x900 or pad to 2133x1200"), "{}", r.summary);
    }

    #[test]
    fn tolerance_absorbs_a_near_miss_and_rejects_a_bigger_one() {
        // 1920x1081 is 0.09% off 16:9.
        let mut o = opts(1920.0, 1081.0, "16:9");
        o.tolerance_percent = 0.5;
        assert_eq!(analyze(&o).unwrap().status, "PASS");
        o.tolerance_percent = 0.0;
        let r = analyze(&o).unwrap();
        assert_eq!(r.status, "FAIL");
        assert_eq!(r.reason.as_deref(), Some("too_tall"));
    }

    #[test]
    fn orientation_agnostic_accepts_the_rotated_target() {
        let mut o = opts(1080.0, 1920.0, "16:9");
        assert_eq!(analyze(&o).unwrap().status, "FAIL");
        o.orientation_agnostic = true;
        let r = analyze(&o).unwrap();
        assert_eq!(r.status, "PASS");
        assert_eq!(r.orientation, "portrait");
        assert_eq!(r.orientation_flipped, Some(true));
        assert_eq!(r.target_ratio.as_deref(), Some("9:16"));
        assert!(r.summary.contains("(rotated)"), "{}", r.summary);
    }

    #[test]
    fn even_dimensions_rounds_the_suggestions_for_encoders() {
        let mut o = opts(1600.0, 1200.0, "1.85:1");
        let odd = analyze(&o).unwrap();
        assert_eq!(odd.crop_height, Some(865));
        o.even_dimensions = true;
        let even = analyze(&o).unwrap();
        assert_eq!(even.crop_height, Some(864));
        assert_eq!(even.crop_width, Some(1600));
    }

    #[test]
    fn no_target_reports_the_nearest_standard_without_a_verdict() {
        let r = analyze(&opts(1080.0, 1350.0, "")).unwrap();
        assert_eq!(r.status, "INFO");
        assert_eq!(r.pass, None);
        assert_eq!(r.ratio, "4:5");
        assert_eq!(r.orientation, "portrait");
        assert_eq!(r.nearest_standard, "4:5");
        assert_eq!(r.target_ratio, None);
        assert_eq!(r.deviation_percent, None);
        assert!(r.summary.starts_with("4:5 (0.8:1) is portrait"), "{}", r.summary);
    }

    #[test]
    fn square_and_odd_dimensions_are_described_correctly() {
        let r = analyze(&opts(512.0, 512.0, "1:1")).unwrap();
        assert_eq!(r.orientation, "square");
        assert_eq!(r.ratio, "1:1");
        assert_eq!(r.status, "PASS");

        // Nothing reduces 1921:1081, so the nearest standard carries the meaning.
        let r = analyze(&opts(1921.0, 1081.0, "")).unwrap();
        assert_eq!(r.ratio, "1921:1081");
        assert_eq!(r.nearest_standard, "16:9");
        assert!(r.nearest_standard_deviation_percent < 0.2);
    }

    #[test]
    fn every_accepted_target_spelling_parses_to_the_same_ratio() {
        for form in ["16:9", "16/9", "1920x1080", "1920X1080", "1920×1080", " 16 : 9 "] {
            let v = parse_ratio(form).unwrap();
            assert!((v - 16.0 / 9.0).abs() < 1e-6, "{form} -> {v}");
        }
        assert!((parse_ratio("1.85:1").unwrap() - 1.85).abs() < 1e-9);
        assert!((parse_ratio("1.7778").unwrap() - 1.7778).abs() < 1e-9);
        assert!((parse_ratio("4/5").unwrap() - 0.8).abs() < 1e-9);
    }

    #[test]
    fn non_integer_dimensions_fall_back_to_the_decimal_form() {
        let r = analyze(&opts(10.5, 7.0, "")).unwrap();
        assert_eq!(r.ratio, "1.5:1");
        assert_eq!(r.nearest_standard, "3:2");
    }

    #[test]
    fn bad_input_is_rejected_with_an_actionable_message() {
        assert!(analyze(&opts(0.0, 1080.0, "16:9"))
            .unwrap_err()
            .contains("width must be a positive number"));
        assert!(analyze(&opts(1920.0, -1.0, "16:9"))
            .unwrap_err()
            .contains("height must be a positive number"));
        assert!(analyze(&opts(2_000_000.0, 1080.0, ""))
            .unwrap_err()
            .contains("at most 1000000 pixels"));
        assert!(analyze(&opts(1920.0, 1080.0, "wide"))
            .unwrap_err()
            .contains("use a form like 16:9"));
        assert!(analyze(&opts(1920.0, 1080.0, "16:0"))
            .unwrap_err()
            .contains("divides by zero"));

        let mut o = opts(1920.0, 1080.0, "16:9");
        o.tolerance_percent = -1.0;
        assert!(analyze(&o).unwrap_err().contains("must not be negative"));
        o.tolerance_percent = 500.0;
        assert!(analyze(&o).unwrap_err().contains("at most 100"));
    }

    #[test]
    fn the_json_report_carries_the_documented_fields() {
        let json = analyze_json(&opts(1600.0, 1200.0, "16:9")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["status"], "FAIL");
        assert_eq!(v["ratio"], "4:3");
        assert_eq!(v["target_ratio"], "16:9");
        assert_eq!(v["crop_width"], 1600);
        assert_eq!(v["pad_height"], 1200);
        assert_eq!(v["orientation"], "landscape");
        // Report-only mode omits every target field rather than emitting nulls.
        let v: serde_json::Value =
            serde_json::from_str(&analyze_json(&opts(1920.0, 1080.0, "")).unwrap()).unwrap();
        assert!(v.get("pass").is_none());
        assert!(v.get("target_ratio").is_none());
        assert!(v.get("crop_width").is_none());
    }
}
