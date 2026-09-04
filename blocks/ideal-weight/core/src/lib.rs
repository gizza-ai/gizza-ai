//! ideal-weight core — pure compute, shared by the chat skill block and the web page.
//!
//! Estimates adult ideal-body-weight (IBW) ranges from height and sex using the
//! four classic clinical equations (Hamwi 1964, Devine 1974, Robinson 1983,
//! Miller 1983) plus a healthy-BMI weight band, with an optional body-frame
//! adjustment. No wafer/wasm-bindgen deps; deterministic, no I/O.

use serde::{Deserialize, Serialize};

/// Inches per centimetre conversion divisor (1 in = 2.54 cm exactly).
pub const CM_PER_INCH: f64 = 2.54;
/// Pounds per kilogram (international avoirdupois pound).
pub const LB_PER_KG: f64 = 2.204_622_621_848_775_9;
/// Height (inches) the four IBW equations are anchored at.
pub const BASELINE_INCHES: f64 = 60.0;
/// Smallest height the equations are allowed to be evaluated at, in inches.
/// Below this they extrapolate past any published data and (for Hamwi male)
/// approach zero.
pub const MIN_INCHES: f64 = 48.0;
/// Largest accepted height, in inches (250 cm).
pub const MAX_INCHES: f64 = 98.425_196_850_393_7;
/// Frame adjustment applied to every formula for a small/large frame.
pub const FRAME_ADJUST_PCT: f64 = 10.0;

/// `(key, label, male_base_kg, male_per_inch, female_base_kg, female_per_inch)`
/// for each supported equation, in chronological order.
pub const FORMULAS: [(&str, &str, f64, f64, f64, f64); 4] = [
    ("hamwi", "Hamwi (1964)", 48.0, 2.7, 45.5, 2.2),
    ("devine", "Devine (1974)", 50.0, 2.3, 45.5, 2.3),
    ("robinson", "Robinson (1983)", 52.0, 1.9, 49.0, 1.7),
    ("miller", "Miller (1983)", 56.2, 1.41, 53.1, 1.36),
];

/// One equation's estimate for the supplied height, sex and frame.
#[derive(Debug, Clone, Serialize)]
pub struct FormulaRow {
    /// Machine-readable formula key, e.g. `devine`.
    pub formula: String,
    /// Human-readable name and publication year.
    pub label: String,
    /// Ideal weight in kilograms, frame-adjusted, rounded to one decimal.
    pub kg: f64,
    /// The same weight in pounds, rounded to one decimal.
    pub lb: f64,
    /// BMI this ideal weight represents at the supplied height.
    pub bmi_at_ideal: f64,
}

/// A low/high pair of weights in both units.
#[derive(Debug, Clone, Serialize)]
pub struct WeightRange {
    /// Lower bound in kilograms.
    pub min_kg: f64,
    /// Upper bound in kilograms.
    pub max_kg: f64,
    /// Lower bound in pounds.
    pub min_lb: f64,
    /// Upper bound in pounds.
    pub max_lb: f64,
}

/// The healthy-weight band implied by a BMI window at the supplied height.
#[derive(Debug, Clone, Serialize)]
pub struct HealthyBmiRange {
    /// Lower BMI bound used (default 18.5).
    pub bmi_min: f64,
    /// Upper BMI bound used (default 24.9).
    pub bmi_max: f64,
    /// Weight at `bmi_min`, in kilograms.
    pub min_kg: f64,
    /// Weight at `bmi_max`, in kilograms.
    pub max_kg: f64,
    /// Weight at `bmi_min`, in pounds.
    pub min_lb: f64,
    /// Weight at `bmi_max`, in pounds.
    pub max_lb: f64,
}

/// The full ideal-weight report.
#[derive(Debug, Clone, Serialize)]
pub struct IdealWeightResult {
    /// Height as entered, converted to centimetres.
    pub height_cm: f64,
    /// Height as entered, converted to total inches.
    pub height_in: f64,
    /// Height rendered as feet and inches, e.g. `5'10"`.
    pub height_ft_in: String,
    /// Sex used by the equations.
    pub sex: String,
    /// Body frame actually applied (`small`, `medium` or `large`).
    pub frame: String,
    /// How the frame was determined: `specified` or `wrist`.
    pub frame_source: String,
    /// Percentage applied to every formula for the frame (−10, 0 or +10).
    pub frame_adjustment_pct: f64,
    /// One row per equation, in chronological order.
    pub formulas: Vec<FormulaRow>,
    /// Mean of the four equations, in kilograms.
    pub average_kg: f64,
    /// Mean of the four equations, in pounds.
    pub average_lb: f64,
    /// Lowest and highest of the four equations.
    pub formula_range: WeightRange,
    /// Weight band implied by the healthy BMI window.
    pub healthy_bmi_range: HealthyBmiRange,
    /// Caveats that apply to this particular input.
    pub notes: Vec<String>,
    /// One-line plain-language summary.
    pub summary: String,
}

/// Every field is optional; unset fields fall back to the documented default so
/// the tool always returns a result.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Inputs {
    /// Height in cm (metric) or total inches (imperial). Default 175 cm.
    pub height: Option<f64>,
    /// `male` or `female`. Default `male`.
    pub sex: Option<String>,
    /// `metric` (cm) or `imperial` (inches). Default `metric`.
    pub units: Option<String>,
    /// `small`, `medium`, `large` or `auto`. Default `medium`.
    pub frame: Option<String>,
    /// Wrist circumference, in cm or inches per `units`. Required by `frame=auto`.
    pub wrist: Option<f64>,
    /// Age in years — used only for an adult-range caveat.
    pub age: Option<f64>,
    /// Lower healthy-BMI bound. Default 18.5.
    pub bmi_min: Option<f64>,
    /// Upper healthy-BMI bound. Default 24.9.
    pub bmi_max: Option<f64>,
}

/// Round to one decimal place.
fn r1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// Lowercase + trim, mapping spaces and hyphens onto underscores so `Very Active`
/// and `very-active` both resolve.
fn normalize(s: &str) -> String {
    s.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

/// Reject NaN/infinite inputs with a message naming the offending field.
fn require_finite(label: &str, v: f64) -> Result<(), String> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(format!("{label} must be a finite number (got {v})"))
    }
}

/// Format total inches as `5'10"`, rounding to the nearest inch.
fn feet_inches(total_in: f64) -> String {
    let whole = total_in.round() as i64;
    format!("{}'{}\"", whole / 12, whole % 12)
}

/// Standard clinical wrist-circumference table (inches) → body frame. Women use
/// three height bands; men use one band above 5'5" which is applied throughout.
fn frame_from_wrist(sex: &str, height_in: f64, wrist_in: f64) -> &'static str {
    let (small, large) = if sex == "female" {
        if height_in < 62.0 {
            (5.5, 5.75)
        } else if height_in <= 65.0 {
            (6.0, 6.25)
        } else {
            (6.25, 6.5)
        }
    } else {
        (6.5, 7.5)
    };
    if wrist_in < small {
        "small"
    } else if wrist_in > large {
        "large"
    } else {
        "medium"
    }
}

/// Compute the ideal-weight report, or an error message naming what was expected.
pub fn compute(i: &Inputs) -> Result<IdealWeightResult, String> {
    let units = normalize(i.units.as_deref().unwrap_or("metric"));
    let units = match units.as_str() {
        "" => "metric".to_string(),
        "metric" | "imperial" => units,
        other => {
            return Err(format!(
                "units must be 'metric' or 'imperial' (got '{other}')"
            ))
        }
    };
    let imperial = units == "imperial";

    let sex = normalize(i.sex.as_deref().unwrap_or("male"));
    let sex = match sex.as_str() {
        "" => "male".to_string(),
        "male" | "female" => sex,
        other => return Err(format!("sex must be 'male' or 'female' (got '{other}')")),
    };

    let height = i.height.unwrap_or(if imperial { 69.0 } else { 175.0 });
    require_finite("height", height)?;
    let height_in = if imperial {
        height
    } else {
        height / CM_PER_INCH
    };
    if height_in < MIN_INCHES || height_in > MAX_INCHES {
        let (lo, hi, unit) = if imperial {
            (MIN_INCHES, MAX_INCHES, "in")
        } else {
            (MIN_INCHES * CM_PER_INCH, MAX_INCHES * CM_PER_INCH, "cm")
        };
        return Err(format!(
            "height must be between {} and {} {unit} for the adult ideal-weight \
             equations (got {}); they are anchored at 5 ft (60 in / 152.4 cm) and \
             produce meaningless values far below that — use a pediatric growth \
             chart instead",
            r1(lo),
            r1(hi),
            r1(height)
        ));
    }
    let height_cm = height_in * CM_PER_INCH;
    let height_m = height_cm / 100.0;

    let mut notes: Vec<String> = Vec::new();

    let frame_req = normalize(i.frame.as_deref().unwrap_or("medium"));
    let frame_req = if frame_req.is_empty() {
        "medium".to_string()
    } else {
        frame_req
    };
    let (frame, frame_source) = match frame_req.as_str() {
        "small" | "medium" | "large" => (frame_req.clone(), "specified".to_string()),
        "auto" => {
            let wrist = i.wrist.ok_or_else(|| {
                format!(
                    "frame='auto' needs a wrist circumference: pass wrist in {} \
                     (measure just below the wrist bone), or set frame to \
                     small/medium/large",
                    if imperial { "inches" } else { "cm" }
                )
            })?;
            require_finite("wrist", wrist)?;
            let wrist_in = if imperial { wrist } else { wrist / CM_PER_INCH };
            if !(3.0..=12.0).contains(&wrist_in) {
                let (lo, hi, unit) = if imperial {
                    (3.0, 12.0, "in")
                } else {
                    (3.0 * CM_PER_INCH, 12.0 * CM_PER_INCH, "cm")
                };
                return Err(format!(
                    "wrist must be between {} and {} {unit} (got {})",
                    r1(lo),
                    r1(hi),
                    r1(wrist)
                ));
            }
            (
                frame_from_wrist(&sex, height_in, wrist_in).to_string(),
                "wrist".to_string(),
            )
        }
        other => {
            return Err(format!(
                "frame must be 'small', 'medium', 'large' or 'auto' (got '{other}')"
            ))
        }
    };
    if frame_req != "auto" && i.wrist.is_some() {
        notes.push(
            "wrist was ignored because frame was set explicitly — set frame=auto to \
             derive the frame from wrist circumference."
                .to_string(),
        );
    }

    let frame_adjustment_pct = match frame.as_str() {
        "small" => -FRAME_ADJUST_PCT,
        "large" => FRAME_ADJUST_PCT,
        _ => 0.0,
    };
    let frame_factor = 1.0 + frame_adjustment_pct / 100.0;

    let bmi_min = i.bmi_min.unwrap_or(18.5);
    let bmi_max = i.bmi_max.unwrap_or(24.9);
    require_finite("bmi_min", bmi_min)?;
    require_finite("bmi_max", bmi_max)?;
    if bmi_min <= 0.0 {
        return Err(format!("bmi_min must be greater than 0 (got {})", r1(bmi_min)));
    }
    if bmi_max <= bmi_min {
        return Err(format!(
            "bmi_max must be greater than bmi_min (got bmi_min {}, bmi_max {})",
            r1(bmi_min),
            r1(bmi_max)
        ));
    }

    let over_baseline = height_in - BASELINE_INCHES;
    let rows: Vec<FormulaRow> = FORMULAS
        .iter()
        .map(|(key, label, m_base, m_step, f_base, f_step)| {
            let (base, step) = if sex == "female" {
                (*f_base, *f_step)
            } else {
                (*m_base, *m_step)
            };
            let kg = (base + step * over_baseline) * frame_factor;
            FormulaRow {
                formula: (*key).to_string(),
                label: (*label).to_string(),
                kg: r1(kg),
                lb: r1(kg * LB_PER_KG),
                bmi_at_ideal: r1(kg / (height_m * height_m)),
            }
        })
        .collect();

    let sum: f64 = rows.iter().map(|r| r.kg).sum();
    let average_kg = r1(sum / rows.len() as f64);
    let lo_kg = rows.iter().map(|r| r.kg).fold(f64::INFINITY, f64::min);
    let hi_kg = rows.iter().map(|r| r.kg).fold(f64::NEG_INFINITY, f64::max);
    // Take the pounds bounds from the rows rather than re-converting the rounded
    // kg, so the range never disagrees with the row it came from by 0.1 lb.
    let lo_lb = rows.iter().map(|r| r.lb).fold(f64::INFINITY, f64::min);
    let hi_lb = rows.iter().map(|r| r.lb).fold(f64::NEG_INFINITY, f64::max);

    let healthy_min_kg = bmi_min * height_m * height_m;
    let healthy_max_kg = bmi_max * height_m * height_m;

    if height_in < BASELINE_INCHES {
        notes.push(format!(
            "height {} cm is below the 5 ft (152.4 cm) baseline these equations were \
             built on, so the values are extrapolated and less reliable.",
            r1(height_cm)
        ));
    }
    if let Some(age) = i.age {
        require_finite("age", age)?;
        if age < 18.0 {
            notes.push(format!(
                "age {} is under 18: these are adult equations. For children and \
                 teenagers use a CDC/WHO growth-chart percentile instead.",
                r1(age)
            ));
        }
    }
    if frame_adjustment_pct != 0.0 {
        notes.push(format!(
            "a {frame} frame applies a {:+}% adjustment to every formula.",
            frame_adjustment_pct
        ));
    }
    notes.push(
        "ideal-body-weight equations were derived for clinical drug dosing and \
         dietetics, not as personal health targets; they ignore body composition, \
         ethnicity and age. Estimates only, not medical advice."
            .to_string(),
    );

    let summary = format!(
        "Ideal weight for a {} at {} ({} cm), {} frame: {}–{} kg ({}–{} lb) across four formulas, \
         average {} kg ({} lb); healthy BMI {}–{} range {}–{} kg ({}–{} lb)",
        sex,
        feet_inches(height_in),
        r1(height_cm),
        frame,
        r1(lo_kg),
        r1(hi_kg),
        lo_lb,
        hi_lb,
        average_kg,
        r1(average_kg * LB_PER_KG),
        r1(bmi_min),
        r1(bmi_max),
        r1(healthy_min_kg),
        r1(healthy_max_kg),
        r1(healthy_min_kg * LB_PER_KG),
        r1(healthy_max_kg * LB_PER_KG),
    );

    Ok(IdealWeightResult {
        height_cm: r1(height_cm),
        height_in: r1(height_in),
        height_ft_in: feet_inches(height_in),
        sex,
        frame,
        frame_source,
        frame_adjustment_pct,
        formulas: rows,
        average_kg,
        average_lb: r1(average_kg * LB_PER_KG),
        formula_range: WeightRange {
            min_kg: r1(lo_kg),
            max_kg: r1(hi_kg),
            min_lb: lo_lb,
            max_lb: hi_lb,
        },
        healthy_bmi_range: HealthyBmiRange {
            bmi_min: r1(bmi_min),
            bmi_max: r1(bmi_max),
            min_kg: r1(healthy_min_kg),
            max_kg: r1(healthy_max_kg),
            min_lb: r1(healthy_min_kg * LB_PER_KG),
            max_lb: r1(healthy_max_kg * LB_PER_KG),
        },
        notes,
        summary,
    })
}

/// Pretty-printed JSON wrapper used by every surface.
pub fn compute_json(i: &Inputs) -> Result<String, String> {
    let out = compute(i)?;
    serde_json::to_string_pretty(&out).map_err(|e| format!("failed to serialize result: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp() -> Inputs {
        Inputs::default()
    }

    fn row(res: &IdealWeightResult, key: &str) -> f64 {
        res.formulas.iter().find(|r| r.formula == key).unwrap().kg
    }

    #[test]
    fn male_70_inches_matches_published_coefficients() {
        let res = compute(&Inputs {
            height: Some(70.0),
            units: Some("imperial".into()),
            ..inp()
        })
        .unwrap();
        // 10 inches over the 60-inch baseline.
        assert_eq!(row(&res, "hamwi"), 75.0); // 48.0 + 2.70×10
        assert_eq!(row(&res, "devine"), 73.0); // 50.0 + 2.30×10
        assert_eq!(row(&res, "robinson"), 71.0); // 52.0 + 1.90×10
        assert_eq!(row(&res, "miller"), 70.3); // 56.2 + 1.41×10
        assert_eq!(res.average_kg, 72.3);
        assert_eq!(res.formula_range.min_kg, 70.3);
        assert_eq!(res.formula_range.max_kg, 75.0);
        assert_eq!(res.height_ft_in, "5'10\"");
        assert_eq!(res.frame, "medium");
        assert_eq!(res.frame_adjustment_pct, 0.0);
    }

    #[test]
    fn formula_range_pounds_match_the_rows_they_came_from() {
        let res = compute(&inp()).unwrap();
        let lbs: Vec<f64> = res.formulas.iter().map(|r| r.lb).collect();
        let lo = lbs.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = lbs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert_eq!(res.formula_range.min_lb, lo);
        assert_eq!(res.formula_range.max_lb, hi);
        assert!(res.summary.contains(&format!("({lo}–{hi} lb)")), "{}", res.summary);
    }

    #[test]
    fn female_64_inches_matches_published_coefficients() {
        let res = compute(&Inputs {
            height: Some(64.0),
            sex: Some("female".into()),
            units: Some("imperial".into()),
            ..inp()
        })
        .unwrap();
        assert_eq!(row(&res, "hamwi"), 54.3); // 45.5 + 2.20×4
        assert_eq!(row(&res, "devine"), 54.7); // 45.5 + 2.30×4
        assert_eq!(row(&res, "robinson"), 55.8); // 49.0 + 1.70×4
        assert_eq!(row(&res, "miller"), 58.5); // 53.1 + 1.36×4
    }

    #[test]
    fn defaults_apply_when_unset() {
        let res = compute(&inp()).unwrap();
        assert_eq!(res.sex, "male");
        assert_eq!(res.height_cm, 175.0);
        assert_eq!(res.height_in, 68.9);
        assert_eq!(res.healthy_bmi_range.bmi_min, 18.5);
        assert_eq!(res.healthy_bmi_range.bmi_max, 24.9);
        assert_eq!(res.formulas.len(), 4);
    }

    #[test]
    fn metric_and_imperial_agree_on_the_same_height() {
        let metric = compute(&Inputs {
            height: Some(177.8),
            ..inp()
        })
        .unwrap();
        let imperial = compute(&Inputs {
            height: Some(70.0),
            units: Some("imperial".into()),
            ..inp()
        })
        .unwrap();
        assert_eq!(metric.average_kg, imperial.average_kg);
        assert_eq!(metric.height_in, 70.0);
    }

    #[test]
    fn frame_adjusts_every_formula_by_ten_percent() {
        let small = compute(&Inputs {
            height: Some(70.0),
            units: Some("imperial".into()),
            frame: Some("small".into()),
            ..inp()
        })
        .unwrap();
        let large = compute(&Inputs {
            height: Some(70.0),
            units: Some("imperial".into()),
            frame: Some("large".into()),
            ..inp()
        })
        .unwrap();
        assert_eq!(small.frame_adjustment_pct, -10.0);
        assert_eq!(large.frame_adjustment_pct, 10.0);
        assert_eq!(row(&small, "devine"), 65.7); // 73.0 × 0.9
        assert_eq!(row(&large, "devine"), 80.3); // 73.0 × 1.1
    }

    #[test]
    fn auto_frame_derives_from_wrist() {
        let big = compute(&Inputs {
            height: Some(70.0),
            units: Some("imperial".into()),
            frame: Some("auto".into()),
            wrist: Some(8.0),
            ..inp()
        })
        .unwrap();
        assert_eq!(big.frame, "large");
        assert_eq!(big.frame_source, "wrist");

        let small = compute(&Inputs {
            height: Some(165.0),
            sex: Some("female".into()),
            frame: Some("auto".into()),
            wrist: Some(14.0), // 5.51 in, under the 6.0 in small cutoff for 65 in
            ..inp()
        })
        .unwrap();
        assert_eq!(small.frame, "small");
    }

    #[test]
    fn bmi_range_weights_track_custom_bounds() {
        let res = compute(&Inputs {
            height: Some(175.0),
            bmi_min: Some(18.5),
            bmi_max: Some(23.0),
            ..inp()
        })
        .unwrap();
        // 1.75 m² = 3.0625 → 18.5×3.0625 = 56.66, 23×3.0625 = 70.44
        assert_eq!(res.healthy_bmi_range.min_kg, 56.7);
        assert_eq!(res.healthy_bmi_range.max_kg, 70.4);
    }

    #[test]
    fn short_height_notes_extrapolation() {
        let res = compute(&Inputs {
            height: Some(145.0),
            ..inp()
        })
        .unwrap();
        assert!(res.notes.iter().any(|n| n.contains("below the 5 ft")));
    }

    #[test]
    fn under_18_age_adds_growth_chart_note() {
        let res = compute(&Inputs {
            height: Some(160.0),
            age: Some(14.0),
            ..inp()
        })
        .unwrap();
        assert!(res.notes.iter().any(|n| n.contains("growth-chart")));
    }

    #[test]
    fn height_below_the_supported_range_is_rejected() {
        let err = compute(&Inputs {
            height: Some(110.0),
            ..inp()
        })
        .unwrap_err();
        assert!(err.contains("height must be between"), "got: {err}");
        assert!(err.contains("cm"), "got: {err}");
    }

    #[test]
    fn auto_frame_without_wrist_is_rejected() {
        let err = compute(&Inputs {
            frame: Some("auto".into()),
            ..inp()
        })
        .unwrap_err();
        assert!(err.contains("wrist circumference"), "got: {err}");
    }

    #[test]
    fn inverted_bmi_bounds_are_rejected() {
        let err = compute(&Inputs {
            bmi_min: Some(25.0),
            bmi_max: Some(18.5),
            ..inp()
        })
        .unwrap_err();
        assert!(err.contains("bmi_max must be greater than bmi_min"), "got: {err}");
    }

    #[test]
    fn unknown_sex_is_rejected() {
        let err = compute(&Inputs {
            sex: Some("other".into()),
            ..inp()
        })
        .unwrap_err();
        assert!(err.contains("sex must be"), "got: {err}");
    }

    #[test]
    fn json_output_is_pretty_and_complete() {
        let json = compute_json(&inp()).unwrap();
        assert!(json.contains("\"formulas\""));
        assert!(json.contains("\"healthy_bmi_range\""));
        assert!(json.contains("\"summary\""));
        assert!(json.contains('\n'));
    }
}
