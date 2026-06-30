//! percentage-calculator core — pure percentage math, shared by the chat skill
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! One tool covers the five everyday percentage questions:
//! - `percent_of`        — "what is P% of X?"            (percent, base)
//! - `what_percent`      — "X is what percent of Y?"      (part, whole)
//! - `change`            — "% change from X to Y"         (from, to)
//! - `apply_change`      — "increase/decrease X by P%"     (base, percent)
//! - `percent_of_total`  — "X is what share of a total?"  (value, total)
//!
//! Inputs may be negative; only divisors are required to be non-zero. All math
//! is `f64` and every reported value is rounded to 6 decimal places.

use serde::Serialize;

/// One named numeric measure of the result (e.g. "result", "absolute_change").
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measure {
    /// Measure name.
    pub name: String,
    /// The computed value, rounded to 6 decimal places.
    pub value: f64,
    /// Unit suffix: "%" for a percentage, "" for a plain number.
    pub unit: String,
}

/// An input value echoed back in the result for transparency.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InputEcho {
    /// Input label (e.g. "percent", "base").
    pub name: String,
    /// The supplied value.
    pub value: f64,
}

/// Structured percentage result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Percentage {
    /// The canonical mode name (e.g. "percent_of", "change").
    pub mode: String,
    /// The inputs echoed back, in the order they were consumed.
    pub inputs: Vec<InputEcho>,
    /// The primary answer plus any supporting values (e.g. the absolute change).
    pub measures: Vec<Measure>,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// Round to 6 decimals to keep output tidy and deterministic across platforms.
fn r6(x: f64) -> f64 {
    (x * 1_000_000.0).round() / 1_000_000.0
}

fn echo(name: &str, value: f64) -> InputEcho {
    InputEcho {
        name: name.to_string(),
        value,
    }
}

fn measure(name: &str, value: f64, unit: &str) -> Measure {
    Measure {
        name: name.to_string(),
        value: r6(value),
        unit: unit.to_string(),
    }
}

/// Validate that a value is a finite number.
fn require_finite(label: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() {
        return Err(format!("{label} must be a finite number"));
    }
    Ok(())
}

/// Validate that a divisor is finite and non-zero.
fn require_nonzero(label: &str, v: f64) -> Result<(), String> {
    require_finite(label, v)?;
    if v == 0.0 {
        return Err(format!("{label} must not be zero (division by zero)"));
    }
    Ok(())
}

/// Format a number the way the summary should read it: trim a trailing `.0` so
/// whole numbers show as `30` rather than `30.0`, but keep real fractions.
fn fmt(v: f64) -> String {
    let r = r6(v);
    if r.fract() == 0.0 && r.abs() < 1e15 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// Compute a percentage result for the chosen `mode` from its inputs.
///
/// `mode` is matched case-insensitively and tolerates spaces, hyphens and
/// underscores (e.g. "Percent Of" == "percent_of"); a few common aliases are
/// accepted too. Only the inputs relevant to the chosen mode are read; the rest
/// are ignored. A missing required input — or a zero divisor — is an error.
///
/// Recognised modes and the inputs they use:
/// - `percent_of` — `percent`, `base`
/// - `what_percent` — `part`, `whole`
/// - `change` — `from`, `to`
/// - `apply_change` — `base`, `percent`
/// - `percent_of_total` — `value`, `total`
pub fn compute(mode: &str, i: &Inputs) -> Result<Percentage, String> {
    let canon = normalize_mode(mode);
    match canon.as_str() {
        "percent_of" => {
            let percent = i.get("percent")?;
            let base = i.get("base")?;
            require_finite("percent", percent)?;
            require_finite("base", base)?;
            let result = percent / 100.0 * base;
            let summary = format!("{}% of {} = {}", fmt(percent), fmt(base), fmt(result));
            Ok(Percentage {
                mode: "percent_of".into(),
                inputs: vec![echo("percent", percent), echo("base", base)],
                measures: vec![measure("result", result, "")],
                summary,
            })
        }
        "what_percent" => {
            let part = i.get("part")?;
            let whole = i.get("whole")?;
            require_finite("part", part)?;
            require_nonzero("whole", whole)?;
            let result = part / whole * 100.0;
            let summary = format!("{} is {}% of {}", fmt(part), fmt(result), fmt(whole));
            Ok(Percentage {
                mode: "what_percent".into(),
                inputs: vec![echo("part", part), echo("whole", whole)],
                measures: vec![measure("result", result, "%")],
                summary,
            })
        }
        "change" => {
            let from = i.get("from")?;
            let to = i.get("to")?;
            require_nonzero("from", from)?;
            require_finite("to", to)?;
            let abs_change = to - from;
            let result = abs_change / from * 100.0;
            let direction = if abs_change > 0.0 {
                "increase"
            } else if abs_change < 0.0 {
                "decrease"
            } else {
                "no change"
            };
            let summary = format!(
                "from {} to {} is a {}% {} (change of {})",
                fmt(from),
                fmt(to),
                fmt(result.abs()),
                direction,
                fmt(abs_change)
            );
            Ok(Percentage {
                mode: "change".into(),
                inputs: vec![echo("from", from), echo("to", to)],
                measures: vec![
                    measure("result", result, "%"),
                    measure("absolute_change", abs_change, ""),
                ],
                summary,
            })
        }
        "apply_change" => {
            let base = i.get("base")?;
            let percent = i.get("percent")?;
            require_finite("base", base)?;
            require_finite("percent", percent)?;
            let abs_change = base * percent / 100.0;
            let result = base + abs_change;
            let verb = if percent >= 0.0 {
                "increased"
            } else {
                "decreased"
            };
            let summary = format!(
                "{} {} by {}% = {}",
                fmt(base),
                verb,
                fmt(percent.abs()),
                fmt(result)
            );
            Ok(Percentage {
                mode: "apply_change".into(),
                inputs: vec![echo("base", base), echo("percent", percent)],
                measures: vec![
                    measure("result", result, ""),
                    measure("absolute_change", abs_change, ""),
                ],
                summary,
            })
        }
        "percent_of_total" => {
            let value = i.get("value")?;
            let total = i.get("total")?;
            require_finite("value", value)?;
            require_nonzero("total", total)?;
            let result = value / total * 100.0;
            let remaining = total - value;
            let remaining_percent = remaining / total * 100.0;
            let summary = format!(
                "{} is {}% of {} (remaining {} = {}%)",
                fmt(value),
                fmt(result),
                fmt(total),
                fmt(remaining),
                fmt(remaining_percent)
            );
            Ok(Percentage {
                mode: "percent_of_total".into(),
                inputs: vec![echo("value", value), echo("total", total)],
                measures: vec![
                    measure("result", result, "%"),
                    measure("remaining", remaining, ""),
                    measure("remaining_percent", remaining_percent, "%"),
                ],
                summary,
            })
        }
        other => Err(format!(
            "unknown mode '{other}'. Supported: percent_of, what_percent, \
             change, apply_change, percent_of_total"
        )),
    }
}

/// Same as [`compute`] but returns the result as a pretty-printed JSON string
/// (for the web page). Errors are returned as the error string.
pub fn compute_json(mode: &str, i: &Inputs) -> Result<String, String> {
    let p = compute(mode, i)?;
    serde_json::to_string_pretty(&p).map_err(|e| format!("serialization failed: {e}"))
}

/// Normalize a mode name: lowercase, collapse spaces/hyphens to underscores, and
/// map common aliases to the canonical mode.
fn normalize_mode(s: &str) -> String {
    let lower: String = s
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c == ' ' || c == '-' { '_' } else { c })
        .collect();
    match lower.as_str() {
        "percentage_of" | "of" => "percent_of".into(),
        "is_what_percent" | "what_percentage" | "percentage" => "what_percent".into(),
        "percent_change" | "percentage_change" => "change".into(),
        "apply_percent" | "increase" | "decrease" | "add_percent" => "apply_change".into(),
        "share" | "of_total" | "percent_share" => "percent_of_total".into(),
        other => other.into(),
    }
}

/// The full set of optional numeric inputs. Each field is `None` when unset.
/// `compute` reads only the fields relevant to the chosen mode.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    pub percent: Option<f64>,
    pub base: Option<f64>,
    pub part: Option<f64>,
    pub whole: Option<f64>,
    pub from: Option<f64>,
    pub to: Option<f64>,
    pub value: Option<f64>,
    pub total: Option<f64>,
}

impl Inputs {
    fn opt(&self, name: &str) -> Option<f64> {
        match name {
            "percent" => self.percent,
            "base" => self.base,
            "part" => self.part,
            "whole" => self.whole,
            "from" => self.from,
            "to" => self.to,
            "value" => self.value,
            "total" => self.total,
            _ => None,
        }
    }

    /// Fetch a required input, erroring if it was not supplied.
    fn get(&self, name: &str) -> Result<f64, String> {
        self.opt(name)
            .ok_or_else(|| format!("missing required input '{name}' for this mode"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i() -> Inputs {
        Inputs::default()
    }

    fn val(p: &Percentage, name: &str) -> f64 {
        p.measures.iter().find(|m| m.name == name).unwrap().value
    }

    #[test]
    fn percent_of_basic() {
        let mut inp = i();
        inp.percent = Some(15.0);
        inp.base = Some(200.0);
        let p = compute("percent_of", &inp).unwrap();
        assert_eq!(val(&p, "result"), 30.0);
        assert_eq!(p.summary, "15% of 200 = 30");
    }

    #[test]
    fn what_percent_basic() {
        let mut inp = i();
        inp.part = Some(30.0);
        inp.whole = Some(200.0);
        let p = compute("what_percent", &inp).unwrap();
        assert_eq!(val(&p, "result"), 15.0);
        assert_eq!(p.summary, "30 is 15% of 200");
    }

    #[test]
    fn change_increase() {
        let mut inp = i();
        inp.from = Some(200.0);
        inp.to = Some(230.0);
        let p = compute("change", &inp).unwrap();
        assert_eq!(val(&p, "result"), 15.0);
        assert_eq!(val(&p, "absolute_change"), 30.0);
        assert!(p.summary.contains("15% increase"), "{}", p.summary);
    }

    #[test]
    fn change_decrease_is_negative() {
        let mut inp = i();
        inp.from = Some(50.0);
        inp.to = Some(40.0);
        let p = compute("change", &inp).unwrap();
        assert_eq!(val(&p, "result"), -20.0);
        assert_eq!(val(&p, "absolute_change"), -10.0);
        assert!(p.summary.contains("decrease"), "{}", p.summary);
    }

    #[test]
    fn apply_change_increase() {
        let mut inp = i();
        inp.base = Some(200.0);
        inp.percent = Some(15.0);
        let p = compute("apply_change", &inp).unwrap();
        assert_eq!(val(&p, "result"), 230.0);
        assert_eq!(val(&p, "absolute_change"), 30.0);
        assert!(p.summary.contains("increased"), "{}", p.summary);
    }

    #[test]
    fn apply_change_negative_decreases() {
        let mut inp = i();
        inp.base = Some(80.0);
        inp.percent = Some(-25.0);
        let p = compute("apply_change", &inp).unwrap();
        assert_eq!(val(&p, "result"), 60.0);
        assert_eq!(val(&p, "absolute_change"), -20.0);
        assert!(p.summary.contains("decreased"), "{}", p.summary);
    }

    #[test]
    fn percent_of_total_with_remaining() {
        let mut inp = i();
        inp.value = Some(30.0);
        inp.total = Some(200.0);
        let p = compute("percent_of_total", &inp).unwrap();
        assert_eq!(val(&p, "result"), 15.0);
        assert_eq!(val(&p, "remaining"), 170.0);
        assert_eq!(val(&p, "remaining_percent"), 85.0);
    }

    #[test]
    fn fractional_percent_is_rounded() {
        let mut inp = i();
        inp.part = Some(1.0);
        inp.whole = Some(3.0);
        let p = compute("what_percent", &inp).unwrap();
        assert_eq!(val(&p, "result"), 33.333333);
    }

    #[test]
    fn mode_name_is_case_and_separator_insensitive() {
        let mut inp = i();
        inp.percent = Some(10.0);
        inp.base = Some(50.0);
        let p = compute("Percent-Of", &inp).unwrap();
        assert_eq!(p.mode, "percent_of");
    }

    #[test]
    fn aliases_map_to_canonical_modes() {
        let mut inp = i();
        inp.from = Some(10.0);
        inp.to = Some(11.0);
        let p = compute("percent change", &inp).unwrap();
        assert_eq!(p.mode, "change");
        assert_eq!(val(&p, "result"), 10.0);
    }

    #[test]
    fn missing_input_errors() {
        let mut inp = i();
        inp.percent = Some(10.0);
        let err = compute("percent_of", &inp).unwrap_err();
        assert!(err.contains("missing required input 'base'"), "{err}");
    }

    #[test]
    fn division_by_zero_errors() {
        let mut inp = i();
        inp.part = Some(5.0);
        inp.whole = Some(0.0);
        let err = compute("what_percent", &inp).unwrap_err();
        assert!(err.contains("must not be zero"), "{err}");
    }

    #[test]
    fn unknown_mode_errors() {
        let inp = i();
        let err = compute("compound_interest", &inp).unwrap_err();
        assert!(err.contains("unknown mode"), "{err}");
    }

    #[test]
    fn negative_base_is_allowed() {
        let mut inp = i();
        inp.percent = Some(50.0);
        inp.base = Some(-40.0);
        let p = compute("percent_of", &inp).unwrap();
        assert_eq!(val(&p, "result"), -20.0);
    }

    #[test]
    fn json_round_trips() {
        let mut inp = i();
        inp.percent = Some(15.0);
        inp.base = Some(200.0);
        let json = compute_json("percent_of", &inp).unwrap();
        assert!(json.contains("\"mode\": \"percent_of\""));
        assert!(json.contains("\"result\""));
    }
}
