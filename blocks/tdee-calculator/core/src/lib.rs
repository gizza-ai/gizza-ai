//! tdee-calculator core — pure BMR / TDEE math, shared by the chat skill block
//! and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Estimates Basal Metabolic Rate (BMR) — the calories the body burns at rest —
//! and Total Daily Energy Expenditure (TDEE) — BMR scaled by an activity factor.
//! Three standard, public-domain BMR equations are supported:
//!
//! - **Mifflin-St Jeor (1990):** `10·kg + 6.25·cm − 5·age + (5 male / −161 female)`
//! - **Harris-Benedict (Roza–Shizgal 1984 revision):**
//!   men `88.362 + 13.397·kg + 4.799·cm − 5.677·age`,
//!   women `447.593 + 9.247·kg + 3.098·cm − 4.330·age`
//! - **Katch-McArdle:** `370 + 21.6·LBM`, `LBM = kg·(1 − body_fat/100)`
//!   (ignores age/sex/height — uses lean body mass instead).
//!
//! TDEE = BMR × activity multiplier (sedentary 1.2 … extra active 1.9). The tool
//! also returns TDEE at every activity level, common calorie goals for cutting /
//! maintaining / bulking, and BMI as a bonus. Energy is reported in Calories
//! (kcal) or kilojoules (1 kcal = 4.184 kJ).
//!
//! All math is `f64`; energy is rounded to whole units and BMI to one decimal.
//! These are population estimates, not medical advice.

use serde::Serialize;

/// The five standard activity levels and their TDEE multipliers.
pub const ACTIVITY_LEVELS: [(&str, f64); 5] = [
    ("sedentary", 1.2),
    ("light", 1.375),
    ("moderate", 1.55),
    ("very_active", 1.725),
    ("extra_active", 1.9),
];

/// kilocalories → kilojoules.
pub const KJ_PER_KCAL: f64 = 4.184;

/// TDEE at one activity level (energy in the requested unit).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActivityRow {
    /// Activity keyword (sedentary, light, moderate, very_active, extra_active).
    pub level: String,
    /// The multiplier applied to BMR for this level.
    pub multiplier: f64,
    /// TDEE at this level, in the requested energy unit (whole units).
    pub tdee: f64,
}

/// Calorie targets for common goals, in the requested energy unit (whole units).
/// Deficits/surpluses are fixed daily offsets from TDEE (floored at 0).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Goals {
    /// TDEE − 250 (~0.25 kg / 0.5 lb loss per week).
    pub mild_loss: f64,
    /// TDEE − 500 (~0.5 kg / 1 lb loss per week).
    pub loss: f64,
    /// TDEE − 1000 (~1 kg / 2 lb loss per week).
    pub extreme_loss: f64,
    /// TDEE (stay the same weight).
    pub maintain: f64,
    /// TDEE + 250 (~0.25 kg / 0.5 lb gain per week).
    pub mild_gain: f64,
    /// TDEE + 500 (~0.5 kg / 1 lb gain per week).
    pub gain: f64,
}

/// Structured BMR / TDEE result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TdeeResult {
    /// Basal Metabolic Rate in the requested energy unit (whole units).
    pub bmr: f64,
    /// Total Daily Energy Expenditure at the selected activity level (whole units).
    pub tdee: f64,
    /// The selected activity keyword, echoed back.
    pub activity: String,
    /// The multiplier applied to BMR for the selected activity level.
    pub activity_multiplier: f64,
    /// The BMR formula actually used.
    pub formula: String,
    /// Energy unit of all calorie fields: "calories" (kcal) or "kilojoules".
    pub energy_unit: String,
    /// Body Mass Index (kg/m², 1 dp).
    pub bmi: f64,
    /// Plain-language BMI category (underweight / normal / overweight / obese).
    pub bmi_category: String,
    /// Calorie goals for cutting, maintaining and bulking.
    pub goals: Goals,
    /// TDEE at each of the five activity levels.
    pub tdee_by_activity: Vec<ActivityRow>,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// All optional inputs. Each field is `None` when unset; [`compute`] applies the
/// documented default for any `None`, so every surface (chat, CLI, page) funnels
/// through the same defaults + validation.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Age in years. Default 30.
    pub age: Option<f64>,
    /// Biological sex: "male" or "female". Default "male".
    pub sex: Option<String>,
    /// Weight in the chosen unit (kg if metric, lb if imperial). Default 70.
    pub weight: Option<f64>,
    /// Height in the chosen unit (cm if metric, inches if imperial). Default 175.
    pub height: Option<f64>,
    /// Unit system: "metric" (kg/cm) or "imperial" (lb/in). Default "metric".
    pub units: Option<String>,
    /// Activity level keyword. Default "moderate".
    pub activity: Option<String>,
    /// BMR formula keyword. Default "mifflin_st_jeor".
    pub formula: Option<String>,
    /// Body-fat percentage (0–100), used by the Katch-McArdle formula. Default 20.
    pub body_fat: Option<f64>,
    /// Energy unit: "calories" or "kilojoules". Default "calories".
    pub energy_unit: Option<String>,
}

const LB_PER_KG: f64 = 0.453_592_37;
const CM_PER_IN: f64 = 2.54;

fn r1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// Lowercase and strip spaces/hyphens/underscores so "Very Active",
/// "very-active" and "very_active" all normalize the same.
fn normalize(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .collect()
}

fn activity_multiplier(kw: &str) -> Result<(&'static str, f64), String> {
    match normalize(kw).as_str() {
        "sedentary" | "none" => Ok(("sedentary", 1.2)),
        "light" | "lightlyactive" | "lightlyactivity" => Ok(("light", 1.375)),
        "moderate" | "moderatelyactive" => Ok(("moderate", 1.55)),
        "veryactive" | "very" | "heavy" => Ok(("very_active", 1.725)),
        "extraactive" | "extra" | "extremelyactive" | "athlete" => Ok(("extra_active", 1.9)),
        other => Err(format!(
            "unknown activity '{other}'. Supported: sedentary, light, moderate, \
             very_active, extra_active"
        )),
    }
}

fn require_finite(label: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() {
        return Err(format!("{label} must be a finite number"));
    }
    Ok(())
}

/// BMI category thresholds (WHO adult classification).
fn bmi_category(bmi: f64) -> &'static str {
    if bmi < 18.5 {
        "underweight"
    } else if bmi < 25.0 {
        "normal"
    } else if bmi < 30.0 {
        "overweight"
    } else {
        "obese"
    }
}

/// Compute the BMR / TDEE result from the supplied inputs, applying defaults for
/// any unset field. Errors on non-finite numbers, non-positive weight/height,
/// an out-of-range age or body-fat, or an unknown keyword.
pub fn compute(i: &Inputs) -> Result<TdeeResult, String> {
    let age = i.age.unwrap_or(30.0);
    let sex_raw = i.sex.clone().unwrap_or_else(|| "male".into());
    let weight = i.weight.unwrap_or(70.0);
    let height = i.height.unwrap_or(175.0);
    let units = i.units.clone().unwrap_or_else(|| "metric".into());
    let activity = i.activity.clone().unwrap_or_else(|| "moderate".into());
    let formula = i.formula.clone().unwrap_or_else(|| "mifflin_st_jeor".into());
    let body_fat = i.body_fat.unwrap_or(20.0);
    let energy_unit_raw = i.energy_unit.clone().unwrap_or_else(|| "calories".into());

    require_finite("age", age)?;
    require_finite("weight", weight)?;
    require_finite("height", height)?;
    require_finite("body_fat", body_fat)?;

    if age <= 0.0 || age > 120.0 {
        return Err("age must be between 1 and 120 years".into());
    }
    if weight <= 0.0 {
        return Err("weight must be greater than zero".into());
    }
    if height <= 0.0 {
        return Err("height must be greater than zero".into());
    }
    if !(0.0..=100.0).contains(&body_fat) {
        return Err("body_fat must be between 0 and 100 percent".into());
    }

    let is_metric = match normalize(&units).as_str() {
        "metric" | "kg" | "kgcm" | "si" => true,
        "imperial" | "us" | "lb" | "lbin" => false,
        other => {
            return Err(format!(
                "unknown units '{other}'. Supported: metric (kg/cm), imperial (lb/in)"
            ))
        }
    };

    let sex_male = match normalize(&sex_raw).as_str() {
        "male" | "man" | "m" => true,
        "female" | "woman" | "f" | "w" => false,
        other => return Err(format!("unknown sex '{other}'. Supported: male, female")),
    };

    let energy_kj = match normalize(&energy_unit_raw).as_str() {
        "calories" | "calorie" | "kcal" | "cal" => false,
        "kilojoules" | "kilojoule" | "kj" => true,
        other => {
            return Err(format!(
                "unknown energy_unit '{other}'. Supported: calories, kilojoules"
            ))
        }
    };

    // Normalize to kg / cm for the formulas.
    let weight_kg = if is_metric { weight } else { weight * LB_PER_KG };
    let height_cm = if is_metric { height } else { height * CM_PER_IN };

    // BMR in kcal (pre-rounding), from the chosen formula.
    let (formula_key, bmr_kcal) = match normalize(&formula).as_str() {
        "mifflinstjeor" | "mifflin" | "msj" => {
            let base = 10.0 * weight_kg + 6.25 * height_cm - 5.0 * age;
            let bmr = if sex_male { base + 5.0 } else { base - 161.0 };
            ("mifflin_st_jeor", bmr)
        }
        "harrisbenedict" | "harris" | "hb" | "harrisbenedictrevised" => {
            let bmr = if sex_male {
                88.362 + 13.397 * weight_kg + 4.799 * height_cm - 5.677 * age
            } else {
                447.593 + 9.247 * weight_kg + 3.098 * height_cm - 4.330 * age
            };
            ("harris_benedict", bmr)
        }
        "katchmcardle" | "katch" | "km" => {
            let lbm = weight_kg * (1.0 - body_fat / 100.0);
            if lbm <= 0.0 {
                return Err(
                    "Katch-McArdle needs a body_fat below 100% so lean body mass is positive"
                        .into(),
                );
            }
            ("katch_mcardle", 370.0 + 21.6 * lbm)
        }
        other => {
            return Err(format!(
                "unknown formula '{other}'. Supported: mifflin_st_jeor, harris_benedict, \
                 katch_mcardle"
            ))
        }
    };

    if bmr_kcal <= 0.0 || !bmr_kcal.is_finite() {
        return Err("the inputs produced a non-positive BMR — check age, weight and height".into());
    }

    let (level_key, mult) = activity_multiplier(&activity)?;

    // Round BMR to whole kcal, then derive TDEE from the rounded BMR so the
    // returned "TDEE = BMR × multiplier" relationship holds exactly.
    let bmr_kcal = bmr_kcal.round();

    // Convert a whole-kcal value into the requested unit, rounded to whole units.
    let to_unit = |kcal: f64| -> f64 {
        if energy_kj {
            (kcal * KJ_PER_KCAL).round()
        } else {
            kcal.round()
        }
    };

    let tdee_kcal = (bmr_kcal * mult).round();

    let tdee_by_activity = ACTIVITY_LEVELS
        .iter()
        .map(|(name, m)| ActivityRow {
            level: (*name).to_string(),
            multiplier: *m,
            tdee: to_unit((bmr_kcal * m).round()),
        })
        .collect();

    let goal = |offset: f64| -> f64 { to_unit((tdee_kcal + offset).max(0.0)) };
    let goals = Goals {
        mild_loss: goal(-250.0),
        loss: goal(-500.0),
        extreme_loss: goal(-1000.0),
        maintain: to_unit(tdee_kcal),
        mild_gain: goal(250.0),
        gain: goal(500.0),
    };

    let height_m = height_cm / 100.0;
    let bmi = r1(weight_kg / (height_m * height_m));
    let bmi_cat = bmi_category(bmi);

    let unit_word = if energy_kj { "kJ" } else { "kcal" };
    let summary = format!(
        "BMR {} {} ({}); TDEE {} {}/day at {} activity (×{})",
        to_unit(bmr_kcal),
        unit_word,
        formula_key.replace('_', "-"),
        to_unit(tdee_kcal),
        unit_word,
        level_key.replace('_', " "),
        trim(mult),
    );

    Ok(TdeeResult {
        bmr: to_unit(bmr_kcal),
        tdee: to_unit(tdee_kcal),
        activity: level_key.to_string(),
        activity_multiplier: mult,
        formula: formula_key.to_string(),
        energy_unit: if energy_kj {
            "kilojoules".into()
        } else {
            "calories".into()
        },
        bmi,
        bmi_category: bmi_cat.to_string(),
        goals,
        tdee_by_activity,
        summary,
    })
}

/// Trim a trailing `.0` from a whole number, keeping real fractions.
fn trim(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
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

    #[test]
    fn defaults_apply_when_unset() {
        // male, 30y, 70 kg, 175 cm, moderate, Mifflin-St Jeor, metric, calories.
        // BMR = 10*70 + 6.25*175 - 5*30 + 5 = 1648.75 → 1649.
        // TDEE = 1649 * 1.55 = 2555.95 → 2556.
        let r = compute(&inp()).unwrap();
        assert_eq!(r.bmr, 1649.0);
        assert_eq!(r.tdee, 2556.0);
        assert_eq!(r.activity, "moderate");
        assert_eq!(r.activity_multiplier, 1.55);
        assert_eq!(r.formula, "mifflin_st_jeor");
        assert_eq!(r.energy_unit, "calories");
        assert_eq!(r.goals.maintain, 2556.0);
        assert!(r.summary.contains("BMR 1649 kcal"));
    }

    #[test]
    fn mifflin_male_known_case() {
        // male, 30y, 80 kg, 180 cm, moderate.
        // BMR = 800 + 1125 - 150 + 5 = 1780. TDEE = 1780 * 1.55 = 2759.
        let mut i = inp();
        i.age = Some(30.0);
        i.sex = Some("male".into());
        i.weight = Some(80.0);
        i.height = Some(180.0);
        i.activity = Some("moderate".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.bmr, 1780.0);
        assert_eq!(r.tdee, 2759.0);
    }

    #[test]
    fn mifflin_female_known_case() {
        // female, 25y, 60 kg, 165 cm, sedentary.
        // BMR = 600 + 1031.25 - 125 - 161 = 1345.25 → 1345. TDEE = 1345 * 1.2 = 1614.
        let mut i = inp();
        i.age = Some(25.0);
        i.sex = Some("female".into());
        i.weight = Some(60.0);
        i.height = Some(165.0);
        i.activity = Some("sedentary".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.bmr, 1345.0);
        assert_eq!(r.tdee, 1614.0);
    }

    #[test]
    fn harris_benedict_revised() {
        // male, 30y, 80 kg, 180 cm.
        // BMR = 88.362 + 13.397*80 + 4.799*180 - 5.677*30
        //     = 88.362 + 1071.76 + 863.82 - 170.31 = 1853.632 → 1854.
        let mut i = inp();
        i.age = Some(30.0);
        i.weight = Some(80.0);
        i.height = Some(180.0);
        i.formula = Some("harris_benedict".into());
        i.activity = Some("sedentary".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.bmr, 1854.0);
        assert_eq!(r.tdee, (1854.0f64 * 1.2).round());
        assert_eq!(r.formula, "harris_benedict");
    }

    #[test]
    fn katch_mcardle_uses_body_fat() {
        // 80 kg at 20% body fat → LBM = 64 kg. BMR = 370 + 21.6*64 = 1752.4 → 1752.
        let mut i = inp();
        i.weight = Some(80.0);
        i.body_fat = Some(20.0);
        i.formula = Some("katch_mcardle".into());
        i.activity = Some("sedentary".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.bmr, 1752.0);
        assert_eq!(r.formula, "katch_mcardle");
    }

    #[test]
    fn imperial_units_convert_to_metric() {
        // male, 30y, 176.37 lb (=80 kg), 70.866 in (=180 cm), moderate.
        // Should match the metric 80 kg / 180 cm case within rounding: BMR ≈ 1780.
        let mut i = inp();
        i.age = Some(30.0);
        i.sex = Some("male".into());
        i.weight = Some(80.0 / LB_PER_KG);
        i.height = Some(180.0 / CM_PER_IN);
        i.units = Some("imperial".into());
        i.activity = Some("moderate".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.bmr, 1780.0);
        assert_eq!(r.tdee, 2759.0);
    }

    #[test]
    fn kilojoules_conversion() {
        // Default kcal BMR is 1649; in kJ that is round(1649 * 4.184) = 6899.
        let mut i = inp();
        i.energy_unit = Some("kilojoules".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.energy_unit, "kilojoules");
        assert_eq!(r.bmr, (1649.0f64 * KJ_PER_KCAL).round());
        assert!(r.summary.contains("kJ"));
    }

    #[test]
    fn goals_offsets_and_floor() {
        let r = compute(&inp()).unwrap();
        assert_eq!(r.goals.maintain, 2556.0);
        assert_eq!(r.goals.mild_loss, 2306.0);
        assert_eq!(r.goals.loss, 2056.0);
        assert_eq!(r.goals.extreme_loss, 1556.0);
        assert_eq!(r.goals.mild_gain, 2806.0);
        assert_eq!(r.goals.gain, 3056.0);
    }

    #[test]
    fn tdee_by_activity_lists_all_levels() {
        let r = compute(&inp()).unwrap();
        assert_eq!(r.tdee_by_activity.len(), 5);
        assert_eq!(r.tdee_by_activity[0].level, "sedentary");
        assert_eq!(r.tdee_by_activity[0].multiplier, 1.2);
        assert_eq!(r.tdee_by_activity[0].tdee, (1649.0f64 * 1.2).round());
        assert_eq!(r.tdee_by_activity[4].level, "extra_active");
        assert_eq!(r.tdee_by_activity[4].multiplier, 1.9);
    }

    #[test]
    fn bmi_and_category() {
        // 70 kg / 1.75 m → 22.857 → 22.9, normal.
        let r = compute(&inp()).unwrap();
        assert_eq!(r.bmi, 22.9);
        assert_eq!(r.bmi_category, "normal");
    }

    #[test]
    fn bmi_category_thresholds() {
        let mut i = inp();
        i.weight = Some(50.0); // 50 / 1.75^2 = 16.3 → underweight
        assert_eq!(compute(&i).unwrap().bmi_category, "underweight");
        i.weight = Some(80.0); // 26.1 → overweight
        assert_eq!(compute(&i).unwrap().bmi_category, "overweight");
        i.weight = Some(100.0); // 32.7 → obese
        assert_eq!(compute(&i).unwrap().bmi_category, "obese");
    }

    #[test]
    fn keywords_are_normalized() {
        let mut i = inp();
        i.sex = Some("Female".into());
        i.activity = Some("Very Active".into());
        i.formula = Some("Mifflin-St Jeor".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.activity, "very_active");
        assert_eq!(r.activity_multiplier, 1.725);
        assert_eq!(r.formula, "mifflin_st_jeor");
    }

    #[test]
    fn unknown_activity_errors() {
        let mut i = inp();
        i.activity = Some("hyperactive".into());
        let err = compute(&i).unwrap_err();
        assert!(err.contains("unknown activity"), "{err}");
    }

    #[test]
    fn unknown_formula_errors() {
        let mut i = inp();
        i.formula = Some("cunningham".into());
        let err = compute(&i).unwrap_err();
        assert!(err.contains("unknown formula"), "{err}");
    }

    #[test]
    fn unknown_sex_errors() {
        let mut i = inp();
        i.sex = Some("other".into());
        let err = compute(&i).unwrap_err();
        assert!(err.contains("unknown sex"), "{err}");
    }

    #[test]
    fn out_of_range_age_errors() {
        let mut i = inp();
        i.age = Some(0.0);
        assert!(compute(&i).unwrap_err().contains("age must be"));
        i.age = Some(130.0);
        assert!(compute(&i).unwrap_err().contains("age must be"));
    }

    #[test]
    fn non_positive_weight_errors() {
        let mut i = inp();
        i.weight = Some(0.0);
        assert!(compute(&i).unwrap_err().contains("weight must be"));
    }

    #[test]
    fn body_fat_out_of_range_errors() {
        let mut i = inp();
        i.body_fat = Some(150.0);
        assert!(compute(&i).unwrap_err().contains("body_fat must be"));
    }

    #[test]
    fn nonfinite_weight_errors() {
        let mut i = inp();
        i.weight = Some(f64::NAN);
        assert!(compute(&i).unwrap_err().contains("finite"));
    }

    #[test]
    fn json_round_trips() {
        let json = compute_json(&inp()).unwrap();
        assert!(json.contains("\"bmr\""));
        assert!(json.contains("\"tdee\""));
        assert!(json.contains("\"tdee_by_activity\""));
        assert!(json.contains("\"goals\""));
        assert!(json.contains("\"bmi\""));
    }
}
