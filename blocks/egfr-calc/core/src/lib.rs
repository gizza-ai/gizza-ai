//! egfr-calc core — pure eGFR math, shared by the chat skill block and the
//! standalone web page. No wafer/wasm-bindgen deps.
//!
//! Estimates the **estimated glomerular filtration rate (eGFR)** — how well the
//! kidneys filter blood — from serum creatinine, age and sex using the CKD-EPI
//! creatinine equations. Two race-free equations are supported:
//!
//! - **CKD-EPI 2021 creatinine (race-free)** — the current NKF/ASN-recommended
//!   US standard (default):
//!   `eGFR = 142 · min(Scr/κ, 1)^α · max(Scr/κ, 1)^−1.200 · 0.9938^Age · (1.012 if female)`
//! - **CKD-EPI 2009 creatinine (race-free form)** — the older equation, provided
//!   for historical comparison. The 2009 **race coefficient is deliberately
//!   omitted** (the 2021 task force recommends against using race):
//!   `eGFR = 141 · min(Scr/κ, 1)^α · max(Scr/κ, 1)^−1.209 · 0.993^Age · (1.018 if female)`
//!
//! where `κ = 0.7` (female) or `0.9` (male), and `α` is the sex/equation-specific
//! small-creatinine exponent. Serum creatinine must be IDMS-standardized and is
//! entered in **mg/dL** or **µmol/L** (converted by ÷88.42). eGFR is reported in
//! **mL/min/1.73 m²**, rounded to a whole number, with the matching KDIGO GFR
//! category (G1–G5).
//!
//! This is an **informational estimate, not a diagnosis or medical advice**.

use serde::Serialize;

/// µmol/L → mg/dL conversion divisor for creatinine.
pub const UMOL_PER_MGDL: f64 = 88.42;

/// A KDIGO GFR category (stage + plain-language label).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EgfrResult {
    /// Estimated GFR in mL/min/1.73 m², rounded to a whole number.
    pub egfr: f64,
    /// Unit of `egfr` — always "mL/min/1.73 m²".
    pub unit: String,
    /// The equation actually used: "ckd_epi_2021" or "ckd_epi_2009".
    pub equation: String,
    /// The serum creatinine used, normalized to mg/dL (2 dp).
    pub creatinine_mg_dl: f64,
    /// Age in years, echoed back.
    pub age: f64,
    /// Biological sex used ("male" or "female"), echoed back.
    pub sex: String,
    /// KDIGO GFR category: G1, G2, G3a, G3b, G4 or G5.
    pub gfr_stage: String,
    /// Plain-language description of the GFR category.
    pub stage_description: String,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// All optional inputs. Each field is `None` when unset; [`compute`] applies the
/// documented default for any `None`, so every surface (chat, CLI, page) funnels
/// through the same defaults + validation.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Serum creatinine value, in the unit given by `creatinine_unit`. Default 1.0.
    pub creatinine: Option<f64>,
    /// Creatinine unit: "mg/dL" (US) or "µmol/L" (SI). Default "mg/dL".
    pub creatinine_unit: Option<String>,
    /// Age in years (adults 18–120). Default 50.
    pub age: Option<f64>,
    /// Biological sex: "male" or "female". Default "male".
    pub sex: Option<String>,
    /// Equation keyword: "ckd_epi_2021" or "ckd_epi_2009". Default "ckd_epi_2021".
    pub equation: Option<String>,
}

/// Lowercase and strip spaces/hyphens/underscores/slashes so "µmol/L",
/// "umol_l" and "UMOLL" all normalize the same.
fn normalize(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_' | '/'))
        .collect()
}

fn require_finite(label: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() {
        return Err(format!("{label} must be a finite number"));
    }
    Ok(())
}

/// Map an eGFR value (mL/min/1.73 m²) to its KDIGO GFR category and label.
/// G1/G2 are normal-range GFRs; a CKD diagnosis in those bands also needs a
/// marker of kidney damage — noted on the page, not decided here.
fn gfr_stage(egfr: f64) -> (&'static str, &'static str) {
    if egfr >= 90.0 {
        ("G1", "Normal or high")
    } else if egfr >= 60.0 {
        ("G2", "Mildly decreased")
    } else if egfr >= 45.0 {
        ("G3a", "Mildly to moderately decreased")
    } else if egfr >= 30.0 {
        ("G3b", "Moderately to severely decreased")
    } else if egfr >= 15.0 {
        ("G4", "Severely decreased")
    } else {
        ("G5", "Kidney failure")
    }
}

fn r2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Compute the eGFR result from the supplied inputs, applying defaults for any
/// unset field. Errors on non-finite numbers, non-positive creatinine, an
/// out-of-range age (CKD-EPI is validated for adults), or an unknown keyword.
pub fn compute(i: &Inputs) -> Result<EgfrResult, String> {
    let creatinine = i.creatinine.unwrap_or(1.0);
    let unit_raw = i.creatinine_unit.clone().unwrap_or_else(|| "mg/dL".into());
    let age = i.age.unwrap_or(50.0);
    let sex_raw = i.sex.clone().unwrap_or_else(|| "male".into());
    let equation_raw = i.equation.clone().unwrap_or_else(|| "ckd_epi_2021".into());

    require_finite("creatinine", creatinine)?;
    require_finite("age", age)?;

    let in_umol = match normalize(&unit_raw).as_str() {
        "mgdl" | "mg" | "us" => false,
        "µmoll" | "umoll" | "umol" | "µmol" | "si" | "micromoll" => true,
        other => {
            return Err(format!(
                "unknown creatinine_unit '{other}'. Supported: mg/dL, µmol/L"
            ))
        }
    };

    let sex_male = match normalize(&sex_raw).as_str() {
        "male" | "man" | "m" => true,
        "female" | "woman" | "f" | "w" => false,
        other => return Err(format!("unknown sex '{other}'. Supported: male, female")),
    };

    // Normalize creatinine to mg/dL for the equations.
    let scr = if in_umol {
        creatinine / UMOL_PER_MGDL
    } else {
        creatinine
    };

    if scr <= 0.0 {
        return Err("creatinine must be greater than zero".into());
    }
    if age < 18.0 || age > 120.0 {
        return Err(
            "age must be between 18 and 120 — CKD-EPI is validated for adults; use a \
             pediatric equation (Schwartz/CKiD) for under-18s"
                .into(),
        );
    }

    // κ and α depend on sex; the multiplier/exponents on the rest depend on the
    // equation. Both equations are the race-free form.
    let kappa = if sex_male { 0.9 } else { 0.7 };

    let (equation_key, base, upper_exp, age_base, female_factor, alpha): (
        &str,
        f64,
        f64,
        f64,
        f64,
        f64,
    ) = match normalize(&equation_raw).as_str() {
            "ckdepi2021" | "ckdepicreatinine2021" | "2021" | "ckdepi" => {
                let alpha = if sex_male { -0.302 } else { -0.241 };
                let female_factor = if sex_male { 1.0 } else { 1.012 };
                ("ckd_epi_2021", 142.0, -1.200, 0.9938, female_factor, alpha)
            }
            "ckdepi2009" | "ckdepicreatinine2009" | "2009" => {
                let alpha = if sex_male { -0.411 } else { -0.329 };
                let female_factor = if sex_male { 1.0 } else { 1.018 };
                ("ckd_epi_2009", 141.0, -1.209, 0.993, female_factor, alpha)
            }
            other => {
                return Err(format!(
                    "unknown equation '{other}'. Supported: ckd_epi_2021, ckd_epi_2009"
                ))
            }
        };

    let ratio = scr / kappa;
    let egfr_raw = base
        * ratio.min(1.0).powf(alpha)
        * ratio.max(1.0).powf(upper_exp)
        * age_base.powf(age)
        * female_factor;

    if !egfr_raw.is_finite() || egfr_raw <= 0.0 {
        return Err("the inputs produced an invalid eGFR — check creatinine and age".into());
    }

    let egfr = egfr_raw.round();
    let (stage, stage_desc) = gfr_stage(egfr);
    let sex_word = if sex_male { "male" } else { "female" };
    let creatinine_mg_dl = r2(scr);

    let equation_label = match equation_key {
        "ckd_epi_2021" => "CKD-EPI 2021",
        "ckd_epi_2009" => "CKD-EPI 2009",
        _ => equation_key,
    };
    let summary = format!(
        "eGFR {egfr} mL/min/1.73 m² ({equation_label}) — GFR category {stage} ({stage_desc})",
    );

    Ok(EgfrResult {
        egfr,
        unit: "mL/min/1.73 m²".into(),
        equation: equation_key.to_string(),
        creatinine_mg_dl,
        age,
        sex: sex_word.to_string(),
        gfr_stage: stage.to_string(),
        stage_description: stage_desc.to_string(),
        summary,
    })
}

/// Same as [`compute`] but returns pretty-printed JSON (for the web page).
pub fn compute_json(i: &Inputs) -> Result<String, String> {
    let res = compute(i)?;
    serde_json::to_string_pretty(&res).map_err(|e| format!("serialization failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp() -> Inputs {
        Inputs::default()
    }

    /// Defaults: CKD-EPI 2021, 50 y male, Scr 1.0 mg/dL → ~92, G1.
    #[test]
    fn defaults_apply_when_unset() {
        let r = compute(&inp()).unwrap();
        assert!((r.egfr - 92.0).abs() <= 1.0, "egfr was {}", r.egfr);
        assert_eq!(r.egfr, 92.0);
        assert_eq!(r.equation, "ckd_epi_2021");
        assert_eq!(r.sex, "male");
        assert_eq!(r.gfr_stage, "G1");
        assert_eq!(r.stage_description, "Normal or high");
        assert_eq!(r.creatinine_mg_dl, 1.0);
        assert!(r.summary.contains("CKD-EPI 2021"));
    }

    /// NKF reference: 2021, 50 y male, Scr 0.9 mg/dL → 104 (G1).
    #[test]
    fn ckd_epi_2021_male_low_creatinine() {
        let mut i = inp();
        i.creatinine = Some(0.9);
        i.age = Some(50.0);
        i.sex = Some("male".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.egfr, 104.0);
        assert_eq!(r.gfr_stage, "G1");
    }

    /// 2021, 60 y female, Scr 1.2 mg/dL → 52 (G3a).
    #[test]
    fn ckd_epi_2021_female_g3a() {
        let mut i = inp();
        i.creatinine = Some(1.2);
        i.age = Some(60.0);
        i.sex = Some("female".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.egfr, 52.0);
        assert_eq!(r.gfr_stage, "G3a");
        assert_eq!(r.stage_description, "Mildly to moderately decreased");
    }

    /// µmol/L input converts (1.2 mg/dL ≈ 106.1 µmol/L) to the same result.
    #[test]
    fn umol_unit_converts() {
        let mut i = inp();
        i.creatinine = Some(1.2 * UMOL_PER_MGDL);
        i.creatinine_unit = Some("µmol/L".into());
        i.age = Some(60.0);
        i.sex = Some("female".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.egfr, 52.0);
        assert_eq!(r.creatinine_mg_dl, 1.2);
    }

    /// The 2009 equation gives a slightly different number and is echoed back.
    #[test]
    fn ckd_epi_2009_differs() {
        let mut i = inp();
        i.equation = Some("ckd_epi_2009".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.equation, "ckd_epi_2009");
        // 2009, 50 y male, 1.0 mg/dL ≈ 87 (G2).
        assert!((r.egfr - 87.0).abs() <= 1.0, "egfr was {}", r.egfr);
        assert_eq!(r.gfr_stage, "G2");
    }

    #[test]
    fn keywords_are_normalized() {
        let mut i = inp();
        i.sex = Some("Female".into());
        i.creatinine_unit = Some("MG/DL".into());
        i.equation = Some("CKD-EPI 2021".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.sex, "female");
        assert_eq!(r.equation, "ckd_epi_2021");
    }

    #[test]
    fn low_egfr_stages() {
        // Very high creatinine → low eGFR → advanced stage.
        let mut i = inp();
        i.creatinine = Some(5.0);
        i.age = Some(70.0);
        i.sex = Some("male".into());
        let r = compute(&i).unwrap();
        assert!(r.egfr < 30.0, "egfr was {}", r.egfr);
        assert!(matches!(r.gfr_stage.as_str(), "G4" | "G5"));
    }

    #[test]
    fn stage_boundaries() {
        assert_eq!(gfr_stage(90.0), ("G1", "Normal or high"));
        assert_eq!(gfr_stage(89.0).0, "G2");
        assert_eq!(gfr_stage(60.0).0, "G2");
        assert_eq!(gfr_stage(59.0).0, "G3a");
        assert_eq!(gfr_stage(45.0).0, "G3a");
        assert_eq!(gfr_stage(44.0).0, "G3b");
        assert_eq!(gfr_stage(30.0).0, "G3b");
        assert_eq!(gfr_stage(29.0).0, "G4");
        assert_eq!(gfr_stage(15.0).0, "G4");
        assert_eq!(gfr_stage(14.0).0, "G5");
    }

    #[test]
    fn under_18_age_errors() {
        let mut i = inp();
        i.age = Some(12.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("age must be between 18"), "{err}");
    }

    #[test]
    fn over_120_age_errors() {
        let mut i = inp();
        i.age = Some(130.0);
        assert!(compute(&i).unwrap_err().contains("age must be between 18"));
    }

    #[test]
    fn non_positive_creatinine_errors() {
        let mut i = inp();
        i.creatinine = Some(0.0);
        assert!(compute(&i).unwrap_err().contains("greater than zero"));
    }

    #[test]
    fn unknown_sex_errors() {
        let mut i = inp();
        i.sex = Some("other".into());
        assert!(compute(&i).unwrap_err().contains("unknown sex"));
    }

    #[test]
    fn unknown_unit_errors() {
        let mut i = inp();
        i.creatinine_unit = Some("g/L".into());
        assert!(compute(&i).unwrap_err().contains("unknown creatinine_unit"));
    }

    #[test]
    fn unknown_equation_errors() {
        let mut i = inp();
        i.equation = Some("mdrd".into());
        assert!(compute(&i).unwrap_err().contains("unknown equation"));
    }

    #[test]
    fn nonfinite_creatinine_errors() {
        let mut i = inp();
        i.creatinine = Some(f64::NAN);
        assert!(compute(&i).unwrap_err().contains("finite"));
    }

    #[test]
    fn json_round_trips() {
        let json = compute_json(&inp()).unwrap();
        assert!(json.contains("\"egfr\""));
        assert!(json.contains("\"gfr_stage\""));
        assert!(json.contains("\"equation\""));
        assert!(json.contains("\"summary\""));
    }
}
