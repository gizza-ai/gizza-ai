//! heart-rate-zones core — pure compute, shared by the chat skill block and the web page.
//!
//! Turns an age, a resting heart rate and (optionally) a measured maximum heart
//! rate into training-zone heart-rate bands. Three band-derivation methods are
//! supported — Karvonen (percentages of heart-rate reserve), plain percentage of
//! maximum heart rate, and Zoladz (fixed bpm offsets below maximum) — across
//! three zone models. Everything is integer bpm rounded to nearest; no I/O.

/// Estimation formulas for maximum heart rate when none is measured.
const FORMULAS: [&str; 6] = ["tanaka", "fox", "gulati", "nes", "inbar", "oakland"];
/// How a zone's heart-rate band is derived.
const METHODS: [&str; 3] = ["karvonen", "percent-max", "zoladz"];
/// Which set of zones to report.
const MODELS: [&str; 3] = ["five-zone", "three-zone", "aha"];
/// Output shapes.
const OUTPUTS: [&str; 3] = ["summary", "table", "json"];

const AGE_MIN: f64 = 5.0;
const AGE_MAX: f64 = 120.0;
const RESTING_MIN: f64 = 25.0;
const RESTING_MAX: f64 = 120.0;
const MAX_HR_MIN: f64 = 100.0;
const MAX_HR_MAX: f64 = 250.0;
const INTENSITY_MIN: f64 = 30.0;
const INTENSITY_MAX: f64 = 100.0;

/// One zone of a model: percentage band plus what it is used for.
struct Band {
    name: &'static str,
    lo_pct: f64,
    hi_pct: f64,
    focus: &'static str,
}

/// A computed zone row, ready to render.
struct Zone {
    index: usize,
    name: &'static str,
    intensity: String,
    lo_bpm: i64,
    hi_bpm: i64,
    focus: &'static str,
}

const FIVE_ZONE: [Band; 5] = [
    Band {
        name: "Recovery",
        lo_pct: 50.0,
        hi_pct: 60.0,
        focus: "Active recovery, warm-up and cool-down; conversation stays effortless",
    },
    Band {
        name: "Aerobic base",
        lo_pct: 60.0,
        hi_pct: 70.0,
        focus: "Long easy endurance work; the highest share of fat as fuel",
    },
    Band {
        name: "Tempo",
        lo_pct: 70.0,
        hi_pct: 80.0,
        focus: "Steady aerobic development around marathon pace",
    },
    Band {
        name: "Threshold",
        lo_pct: 80.0,
        hi_pct: 90.0,
        focus: "Lactate-threshold and 10K-pace intervals; talking gets hard",
    },
    Band {
        name: "VO2 max",
        lo_pct: 90.0,
        hi_pct: 100.0,
        focus: "Short maximal intervals and sprints; minutes, not hours",
    },
];

const THREE_ZONE: [Band; 3] = [
    Band {
        name: "Easy",
        lo_pct: 50.0,
        hi_pct: 81.0,
        focus: "Below the first lactate turn point; roughly 80% of a polarized week",
    },
    Band {
        name: "Moderate",
        lo_pct: 81.0,
        hi_pct: 87.0,
        focus: "Between the two turn points; the tempo/threshold middle ground",
    },
    Band {
        name: "Hard",
        lo_pct: 87.0,
        hi_pct: 100.0,
        focus: "Above the second turn point; hard intervals and races",
    },
];

const AHA_ZONE: [Band; 2] = [
    Band {
        name: "Moderate intensity",
        lo_pct: 50.0,
        hi_pct: 70.0,
        focus: "Brisk walking, easy cycling, doubles tennis",
    },
    Band {
        name: "Vigorous intensity",
        lo_pct: 70.0,
        hi_pct: 85.0,
        focus: "Running, fast cycling, lap swimming, singles tennis",
    },
];

/// Zoladz bands are fixed bpm distances below maximum heart rate: the classic
/// anchors are max-50, max-40, max-30, max-20 and max-10, each +/- 5 bpm.
const ZOLADZ_OFFSETS: [(f64, f64); 5] = [
    (-55.0, -45.0),
    (-45.0, -35.0),
    (-35.0, -25.0),
    (-25.0, -15.0),
    (-15.0, -5.0),
];

fn bands(model: &str) -> &'static [Band] {
    match model {
        "three-zone" => &THREE_ZONE,
        "aha" => &AHA_ZONE,
        _ => &FIVE_ZONE,
    }
}

/// Human-readable formula text, used in the report and the JSON payload.
fn formula_text(formula: &str) -> &'static str {
    match formula {
        "fox" => "220 - age",
        "gulati" => "206 - 0.88 x age",
        "nes" => "211 - 0.64 x age",
        "inbar" => "205.8 - 0.685 x age",
        "oakland" => "192 - 0.007 x age^2",
        _ => "208 - 0.7 x age",
    }
}

fn formula_label(formula: &str) -> &'static str {
    match formula {
        "fox" => "Fox",
        "gulati" => "Gulati",
        "nes" => "Nes",
        "inbar" => "Inbar",
        "oakland" => "Oakland nonlinear",
        _ => "Tanaka",
    }
}

fn estimate_max_hr(age: f64, formula: &str) -> f64 {
    match formula {
        "fox" => 220.0 - age,
        "gulati" => 206.0 - 0.88 * age,
        "nes" => 211.0 - 0.64 * age,
        "inbar" => 205.8 - 0.685 * age,
        "oakland" => 192.0 - 0.007 * age * age,
        _ => 208.0 - 0.7 * age,
    }
}

fn round_bpm(v: f64) -> i64 {
    v.round() as i64
}

/// Trim a whole-number float so `30` prints as `30`, not `30.0`.
fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn check_choice(name: &str, value: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "expected {name} to be one of {}, got `{value}`",
            allowed.join(", ")
        ))
    }
}

fn check_range(name: &str, value: f64, lo: f64, hi: f64, unit: &str) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("expected a number for {name}, got `{value}`"));
    }
    if value < lo || value > hi {
        return Err(format!(
            "expected {name} between {} and {} {unit}, got {}",
            fmt_num(lo),
            fmt_num(hi),
            fmt_num(value)
        ));
    }
    Ok(())
}

/// Heart-rate value for one intensity percentage under the chosen method.
fn hr_at(pct: f64, method: &str, max_hr: f64, resting_hr: f64) -> f64 {
    match method {
        "percent-max" => pct / 100.0 * max_hr,
        // Zoladz never routes through here; treat it as Karvonen for the
        // stand-alone custom-intensity target so the value stays meaningful.
        _ => pct / 100.0 * (max_hr - resting_hr) + resting_hr,
    }
}

/// Compute the zone table for the chosen method/model.
fn zones(method: &str, model: &str, max_hr: f64, resting_hr: f64) -> Vec<Zone> {
    if method == "zoladz" {
        return ZOLADZ_OFFSETS
            .iter()
            .zip(FIVE_ZONE.iter())
            .enumerate()
            .map(|(i, ((lo_off, hi_off), band))| Zone {
                index: i + 1,
                name: band.name,
                intensity: format!("max {} to {} bpm", fmt_num(*lo_off), fmt_num(*hi_off)),
                lo_bpm: round_bpm((max_hr + lo_off).max(0.0)),
                hi_bpm: round_bpm((max_hr + hi_off).max(0.0)),
                focus: band.focus,
            })
            .collect();
    }
    bands(model)
        .iter()
        .enumerate()
        .map(|(i, band)| Zone {
            index: i + 1,
            name: band.name,
            intensity: format!("{}-{}%", fmt_num(band.lo_pct), fmt_num(band.hi_pct)),
            lo_bpm: round_bpm(hr_at(band.lo_pct, method, max_hr, resting_hr)),
            hi_bpm: round_bpm(hr_at(band.hi_pct, method, max_hr, resting_hr)),
            focus: band.focus,
        })
        .collect()
}

fn method_label(method: &str) -> &'static str {
    match method {
        "percent-max" => "percentage of maximum heart rate",
        "zoladz" => "Zoladz fixed bpm offsets below maximum",
        _ => "Karvonen heart-rate reserve",
    }
}

fn model_label(method: &str, model: &str) -> &'static str {
    if method == "zoladz" {
        return "five Zoladz zones";
    }
    match model {
        "three-zone" => "three-zone polarized model",
        "aha" => "two American Heart Association bands",
        _ => "five-zone model",
    }
}

/// Compute heart-rate training zones.
///
/// * `age` — years, 5-120, used only when `max_hr` is 0.
/// * `resting_hr` — resting beats per minute, 25-120.
/// * `max_hr` — measured maximum bpm, or 0 to estimate it from `age`.
/// * `max_hr_formula` — `tanaka` | `fox` | `gulati` | `nes`.
/// * `method` — `karvonen` | `percent-max` | `zoladz`.
/// * `model` — `five-zone` | `three-zone` | `aha` (ignored by `zoladz`).
/// * `intensity` — extra single-intensity target in percent, or 0 for none.
/// * `output` — `summary` | `table` | `json`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    age: f64,
    resting_hr: f64,
    max_hr: f64,
    max_hr_formula: &str,
    method: &str,
    model: &str,
    intensity: f64,
    output: &str,
) -> Result<String, String> {
    check_choice("max_hr_formula", max_hr_formula, &FORMULAS)?;
    check_choice("method", method, &METHODS)?;
    check_choice("model", model, &MODELS)?;
    check_choice("output", output, &OUTPUTS)?;
    check_range("age", age, AGE_MIN, AGE_MAX, "years")?;
    check_range("resting_hr", resting_hr, RESTING_MIN, RESTING_MAX, "bpm")?;
    if !max_hr.is_finite() {
        return Err(format!("expected a number for max_hr, got `{max_hr}`"));
    }
    if max_hr != 0.0 {
        check_range("max_hr", max_hr, MAX_HR_MIN, MAX_HR_MAX, "bpm")?;
    }
    if !intensity.is_finite() {
        return Err(format!("expected a number for intensity, got `{intensity}`"));
    }
    if intensity != 0.0 {
        check_range("intensity", intensity, INTENSITY_MIN, INTENSITY_MAX, "percent")?;
    }

    let measured = max_hr != 0.0;
    let max = if measured {
        max_hr
    } else {
        estimate_max_hr(age, max_hr_formula)
    };
    if max <= resting_hr {
        return Err(format!(
            "expected max_hr above resting_hr, got max {} bpm and resting {} bpm",
            fmt_num(max),
            fmt_num(resting_hr)
        ));
    }
    let reserve = max - resting_hr;
    let rows = zones(method, model, max, resting_hr);
    let max_bpm = round_bpm(max);
    let reserve_bpm = round_bpm(reserve);
    let target = if intensity != 0.0 {
        Some(round_bpm(hr_at(intensity, method, max, resting_hr)))
    } else {
        None
    };
    let max_source = if measured {
        "Measured maximum heart rate".to_string()
    } else {
        format!(
            "{} estimate ({})",
            formula_label(max_hr_formula),
            formula_text(max_hr_formula)
        )
    };

    Ok(match output {
        "json" => render_json(
            age,
            resting_hr,
            max_bpm,
            measured,
            max_hr_formula,
            reserve_bpm,
            method,
            model,
            intensity,
            target,
            &rows,
        ),
        "table" => render_table(
            resting_hr,
            max_bpm,
            reserve_bpm,
            &max_source,
            method,
            model,
            intensity,
            target,
            &rows,
        ),
        _ => render_summary(
            age,
            resting_hr,
            max_bpm,
            reserve_bpm,
            &max_source,
            method,
            model,
            intensity,
            target,
            &rows,
        ),
    })
}

#[allow(clippy::too_many_arguments)]
fn render_summary(
    age: f64,
    resting_hr: f64,
    max_bpm: i64,
    reserve_bpm: i64,
    max_source: &str,
    method: &str,
    model: &str,
    intensity: f64,
    target: Option<i64>,
    rows: &[Zone],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Heart-rate training zones — {}, {}\n\n",
        method_label(method),
        model_label(method, model)
    ));
    out.push_str(&format!(
        "Age {} years · resting HR {} bpm\n",
        fmt_num(age),
        fmt_num(resting_hr)
    ));
    out.push_str(&format!("Maximum HR {max_bpm} bpm — {max_source}\n"));
    out.push_str(&format!(
        "Heart-rate reserve {reserve_bpm} bpm — maximum minus resting\n\n"
    ));
    let name_w = rows.iter().map(|z| z.name.len()).max().unwrap_or(0);
    let int_w = rows.iter().map(|z| z.intensity.len()).max().unwrap_or(0);
    for z in rows {
        out.push_str(&format!(
            "Zone {}  {:<name_w$}  {:>int_w$}  {}-{} bpm  {}\n",
            z.index, z.name, z.intensity, z.lo_bpm, z.hi_bpm, z.focus
        ));
    }
    if let Some(bpm) = target {
        out.push_str(&format!(
            "\nTarget at {}% intensity: {} bpm\n",
            fmt_num(intensity),
            bpm
        ));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn render_table(
    resting_hr: f64,
    max_bpm: i64,
    reserve_bpm: i64,
    max_source: &str,
    method: &str,
    model: &str,
    intensity: f64,
    target: Option<i64>,
    rows: &[Zone],
) -> String {
    let mut out = String::new();
    out.push_str("| Measure | Value |\n| --- | --- |\n");
    out.push_str(&format!("| Maximum HR | {max_bpm} bpm ({max_source}) |\n"));
    out.push_str(&format!("| Resting HR | {} bpm |\n", fmt_num(resting_hr)));
    out.push_str(&format!("| Heart-rate reserve | {reserve_bpm} bpm |\n"));
    out.push_str(&format!("| Method | {} |\n", method_label(method)));
    out.push_str(&format!("| Zone model | {} |\n", model_label(method, model)));
    if let Some(bpm) = target {
        out.push_str(&format!(
            "| Target at {}% | {} bpm |\n",
            fmt_num(intensity),
            bpm
        ));
    }
    out.push_str("\n| Zone | Name | Intensity | Heart rate | Training focus |\n");
    out.push_str("| --- | --- | --- | --- | --- |\n");
    for z in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {}-{} bpm | {} |\n",
            z.index, z.name, z.intensity, z.lo_bpm, z.hi_bpm, z.focus
        ));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn render_json(
    age: f64,
    resting_hr: f64,
    max_bpm: i64,
    measured: bool,
    max_hr_formula: &str,
    reserve_bpm: i64,
    method: &str,
    model: &str,
    intensity: f64,
    target: Option<i64>,
    rows: &[Zone],
) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!("  \"age\": {},\n", fmt_num(age)));
    out.push_str(&format!("  \"resting_hr\": {},\n", fmt_num(resting_hr)));
    out.push_str(&format!("  \"max_hr\": {max_bpm},\n"));
    out.push_str(&format!(
        "  \"max_hr_source\": \"{}\",\n",
        if measured { "measured" } else { max_hr_formula }
    ));
    out.push_str(&format!(
        "  \"max_hr_formula\": {},\n",
        if measured {
            "null".to_string()
        } else {
            format!("\"{}\"", formula_text(max_hr_formula))
        }
    ));
    out.push_str(&format!(
        "  \"heart_rate_reserve\": {reserve_bpm},\n  \"method\": \"{method}\",\n  \"model\": \"{}\",\n",
        if method == "zoladz" { "five-zone" } else { model }
    ));
    out.push_str("  \"zones\": [\n");
    for (i, z) in rows.iter().enumerate() {
        out.push_str(&format!(
            "    {{ \"zone\": {}, \"name\": \"{}\", \"intensity\": \"{}\", \"low_bpm\": {}, \"high_bpm\": {}, \"focus\": \"{}\" }}{}\n",
            z.index,
            z.name,
            z.intensity,
            z.lo_bpm,
            z.hi_bpm,
            z.focus,
            if i + 1 == rows.len() { "" } else { "," }
        ));
    }
    out.push_str("  ]");
    if let Some(bpm) = target {
        out.push_str(&format!(
            ",\n  \"target\": {{ \"intensity_percent\": {}, \"bpm\": {} }}",
            fmt_num(intensity),
            bpm
        ));
    }
    out.push_str("\n}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn karvonen_five_zone_matches_hand_computation() {
        // Tanaka max = 208 - 0.7*30 = 187; reserve = 187 - 60 = 127.
        // Zone 2 = 60-70% of reserve + resting = 136.2-148.9 -> 136-149.
        let out = run(
            30.0, 60.0, 0.0, "tanaka", "karvonen", "five-zone", 0.0, "summary",
        )
        .unwrap();
        assert!(out.contains("Maximum HR 187 bpm"), "{out}");
        assert!(out.contains("Heart-rate reserve 127 bpm"), "{out}");
        assert!(out.contains("136-149 bpm"), "{out}");
        assert!(out.contains("Zone 5"), "{out}");
        assert!(!out.contains("Target at"), "{out}");
    }

    #[test]
    fn percent_max_ignores_resting_heart_rate() {
        // 220 - 40 = 180; zone 1 = 50-60% of max = 90-108 regardless of resting.
        let low = run(40.0, 45.0, 0.0, "fox", "percent-max", "five-zone", 0.0, "summary").unwrap();
        let high = run(40.0, 80.0, 0.0, "fox", "percent-max", "five-zone", 0.0, "summary").unwrap();
        assert!(low.contains("90-108 bpm"), "{low}");
        assert!(high.contains("90-108 bpm"), "{high}");
    }

    #[test]
    fn measured_max_hr_overrides_the_formula() {
        let out = run(
            30.0, 50.0, 195.0, "tanaka", "karvonen", "five-zone", 0.0, "summary",
        )
        .unwrap();
        assert!(
            out.contains("Maximum HR 195 bpm — Measured maximum heart rate"),
            "{out}"
        );
        assert!(out.contains("Heart-rate reserve 145 bpm"), "{out}");
    }

    #[test]
    fn zoladz_bands_are_fixed_offsets_below_maximum() {
        let out = run(
            30.0, 60.0, 200.0, "tanaka", "zoladz", "three-zone", 0.0, "summary",
        )
        .unwrap();
        // Model is ignored: Zoladz always reports its five offset bands.
        assert!(out.contains("five Zoladz zones"), "{out}");
        assert!(out.contains("145-155 bpm"), "{out}");
        assert!(out.contains("185-195 bpm"), "{out}");
        assert!(out.contains("Zone 5"), "{out}");
    }

    #[test]
    fn aha_model_reports_two_bands_and_a_custom_target() {
        let out = run(50.0, 60.0, 0.0, "fox", "karvonen", "aha", 75.0, "summary").unwrap();
        // Fox max = 170; reserve = 110. 50-70% -> 115-137; 70-85% -> 137-154.
        assert!(out.contains("Zone 1  Moderate intensity"), "{out}");
        assert!(out.contains("115-137 bpm"), "{out}");
        assert!(out.contains("137-154 bpm"), "{out}");
        assert!(!out.contains("Zone 3"), "{out}");
        assert!(out.contains("Target at 75% intensity: 143 bpm"), "{out}");
    }

    #[test]
    fn gulati_and_nes_formulas_are_distinct() {
        let g = run(40.0, 60.0, 0.0, "gulati", "karvonen", "aha", 0.0, "json").unwrap();
        let n = run(40.0, 60.0, 0.0, "nes", "karvonen", "aha", 0.0, "json").unwrap();
        assert!(g.contains("\"max_hr\": 171"), "{g}"); // 206 - 35.2
        assert!(n.contains("\"max_hr\": 185"), "{n}"); // 211 - 25.6
        assert!(g.contains("\"max_hr_formula\": \"206 - 0.88 x age\""), "{g}");
    }

    #[test]
    fn inbar_and_oakland_formulas_are_distinct() {
        let i = run(40.0, 60.0, 0.0, "inbar", "karvonen", "aha", 0.0, "json").unwrap();
        let o = run(40.0, 60.0, 0.0, "oakland", "karvonen", "aha", 0.0, "json").unwrap();
        assert!(i.contains("\"max_hr\": 178"), "{i}"); // 205.8 - 27.4
        assert!(o.contains("\"max_hr\": 181"), "{o}"); // 192 - 0.007 * 1600
        assert!(i.contains("\"max_hr_formula\": \"205.8 - 0.685 x age\""), "{i}");
        assert!(o.contains("\"max_hr_formula\": \"192 - 0.007 x age^2\""), "{o}");
    }

    /// The Oakland fit is nonlinear, so its gap to the linear formulas widens
    /// with age — the property that makes it worth offering at all.
    #[test]
    fn oakland_is_nonlinear_in_age() {
        let at = |age: f64| {
            run(age, 50.0, 0.0, "oakland", "percent-max", "aha", 0.0, "summary")
                .unwrap()
                .lines()
                .find(|l| l.starts_with("Maximum HR"))
                .unwrap()
                .to_string()
        };
        assert!(at(20.0).contains("189 bpm"), "{}", at(20.0)); // 192 - 2.8
        assert!(at(60.0).contains("167 bpm"), "{}", at(60.0)); // 192 - 25.2
        assert!(at(20.0).contains("Oakland nonlinear estimate"), "{}", at(20.0));
    }

    #[test]
    fn json_output_is_parseable_shaped_and_omits_absent_target() {
        let out = run(
            30.0, 60.0, 0.0, "tanaka", "karvonen", "three-zone", 0.0, "json",
        )
        .unwrap();
        assert!(out.starts_with('{'), "{out}");
        assert!(out.contains("\"model\": \"three-zone\""), "{out}");
        assert_eq!(out.matches("\"zone\":").count(), 3, "{out}");
        assert!(!out.contains("\"target\""), "{out}");
    }

    #[test]
    fn table_output_is_markdown() {
        let out = run(
            30.0, 60.0, 0.0, "tanaka", "karvonen", "five-zone", 65.0, "table",
        )
        .unwrap();
        assert!(out.contains("| Zone | Name | Intensity | Heart rate | Training focus |"));
        assert!(out.contains("| Target at 65% | 143 bpm |"), "{out}");
    }

    #[test]
    fn rejects_an_out_of_range_age() {
        let err = run(
            2.0, 60.0, 0.0, "tanaka", "karvonen", "five-zone", 0.0, "summary",
        )
        .unwrap_err();
        assert_eq!(err, "expected age between 5 and 120 years, got 2");
    }

    #[test]
    fn rejects_an_unknown_method() {
        let err = run(30.0, 60.0, 0.0, "tanaka", "maffetone", "five-zone", 0.0, "summary")
            .unwrap_err();
        assert!(err.starts_with("expected method to be one of karvonen, percent-max, zoladz"), "{err}");
    }

    #[test]
    fn rejects_a_resting_rate_at_or_above_maximum() {
        let err = run(
            30.0, 110.0, 105.0, "tanaka", "karvonen", "five-zone", 0.0, "summary",
        )
        .unwrap_err();
        assert_eq!(
            err,
            "expected max_hr above resting_hr, got max 105 bpm and resting 110 bpm"
        );
    }

    #[test]
    fn rejects_an_intensity_below_the_floor() {
        let err = run(
            30.0, 60.0, 0.0, "tanaka", "karvonen", "five-zone", 10.0, "summary",
        )
        .unwrap_err();
        assert_eq!(err, "expected intensity between 30 and 100 percent, got 10");
    }

    #[test]
    fn accepts_the_exact_range_boundaries() {
        for (age, resting) in [(AGE_MIN, RESTING_MIN), (AGE_MAX, RESTING_MAX)] {
            run(
                age, resting, MAX_HR_MAX, "tanaka", "karvonen", "five-zone", INTENSITY_MAX,
                "summary",
            )
            .unwrap_or_else(|e| panic!("age={age} resting={resting}: {e}"));
        }
        run(
            30.0,
            60.0,
            MAX_HR_MIN,
            "tanaka",
            "karvonen",
            "five-zone",
            INTENSITY_MIN,
            "summary",
        )
        .unwrap();
    }
}
