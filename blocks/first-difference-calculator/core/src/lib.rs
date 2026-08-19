//! gizza-ai/first-difference-calculator core — first differences (and higher
//! orders) of a numeric series, in absolute, percent, ratio, or log form.
//! Pure-Rust, dependency-free besides serde for the output struct.
//!
//! Semantics, chosen to match what practitioners already expect:
//!
//! * `lag` is how far back the baseline sits. `lag = 1` (the default) compares
//!   each value with the one before it; `lag = 12` compares with the value 12
//!   positions back (seasonal differencing). A NEGATIVE lag compares each value
//!   with a LATER one (a lead), which moves the warm-up region to the end.
//! * `order` repeats the transform on its own output (`order = 2` is the second
//!   difference), growing the warm-up region by `|lag|` each pass.
//! * Output is ALIGNED to the input by default — the warm-up positions come back
//!   as `null` so row `i` of the output still means row `i` of the input. Set
//!   `drop_warmup` for the shorter form where those positions are removed.
//! * A comparison whose maths is undefined (a zero baseline for percent/ratio, a
//!   non-positive value for log) is reported as `null` and counted in
//!   `summary.undefined` — never as an infinity, which JSON cannot represent.

use serde::Serialize;

/// Max series length we'll process, to bound work on hostile input.
pub const MAX_POINTS: usize = 20_000;
/// Max `|lag|`.
pub const MAX_LAG: i64 = 1_000;
/// Max differencing order.
pub const MAX_ORDER: i64 = 10;
/// Max decimal places for rounding.
pub const MAX_DECIMALS: i64 = 10;

/// How each value is compared with its baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `current - baseline` — the plain first difference.
    Difference,
    /// `(current - baseline) / baseline * 100` — signed percent change.
    Percent,
    /// `current / baseline` — the growth factor.
    Ratio,
    /// `ln(current / baseline)` — the log difference (continuous growth rate).
    Log,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "difference" | "diff" | "absolute" => Ok(Mode::Difference),
            "percent" | "pct" | "percentage" => Ok(Mode::Percent),
            "ratio" => Ok(Mode::Ratio),
            "log" => Ok(Mode::Log),
            other => Err(format!(
                "mode must be one of difference, percent, ratio, log (got {other:?})"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mode::Difference => "difference",
            Mode::Percent => "percent",
            Mode::Ratio => "ratio",
            Mode::Log => "log",
        }
    }

    /// The value that means "no change" in this mode: 1 for a ratio, 0 for the
    /// additive modes. Used for the up/down/flat counts.
    fn neutral(self) -> f64 {
        match self {
            Mode::Ratio => 1.0,
            _ => 0.0,
        }
    }

    /// `None` when the comparison is mathematically undefined.
    fn apply(self, current: f64, baseline: f64) -> Option<f64> {
        let v = match self {
            Mode::Difference => current - baseline,
            Mode::Percent => {
                if baseline == 0.0 {
                    return None;
                }
                (current - baseline) / baseline * 100.0
            }
            Mode::Ratio => {
                if baseline == 0.0 {
                    return None;
                }
                current / baseline
            }
            Mode::Log => {
                if baseline <= 0.0 || current <= 0.0 {
                    return None;
                }
                (current / baseline).ln()
            }
        };
        if v.is_finite() {
            Some(v)
        } else {
            None
        }
    }
}

/// Counts and statistics over the non-warm-up part of the result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    /// Positions with no baseline to compare against (`|lag| * order`).
    pub warmup: usize,
    /// Comparisons that produced a number.
    pub defined: usize,
    /// Comparisons whose maths was undefined (zero baseline, non-positive log
    /// input) and are reported as `null`. Excludes the warm-up positions.
    pub undefined: usize,
    /// Defined values above the no-change level (0, or 1 in ratio mode).
    pub increases: usize,
    /// Defined values below the no-change level.
    pub decreases: usize,
    /// Defined values exactly at the no-change level.
    pub unchanged: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub sum: Option<f64>,
    /// The defined value furthest from the no-change level.
    pub largest_move: Option<f64>,
    /// Index (into the ORIGINAL series) of `largest_move`.
    pub largest_move_index: Option<usize>,
    /// True when every defined value is identical and nothing was undefined —
    /// the classic "constant differences" reading.
    pub constant: bool,
}

/// The full result, serialized as the tool's JSON output.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Differences {
    /// Number of input data points.
    pub count: usize,
    pub lag: i64,
    pub order: u32,
    pub mode: String,
    pub decimals: u32,
    pub drop_warmup: bool,
    /// One entry per output row. `null` = warm-up or undefined.
    pub values: Vec<Option<f64>>,
    /// The original-series index each `values` entry belongs to.
    pub indices: Vec<usize>,
    pub summary: Summary,
    /// Plain-language reading of the result.
    pub interpretation: String,
}

fn round_to(v: f64, decimals: u32) -> f64 {
    let f = 10f64.powi(decimals as i32);
    let r = (v * f).round() / f;
    // Normalize -0.0 so "no change" always prints as 0.
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Parse whitespace/comma/semicolon-separated numbers.
fn parse_series(text: &str) -> Result<Vec<f64>, String> {
    let mut nums = Vec::new();
    for tok in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        let n: f64 = t.parse().map_err(|_| {
            format!("'{t}' is not a number — the series must be plain decimals or scientific notation, separated by spaces, commas, semicolons, or newlines")
        })?;
        if !n.is_finite() {
            return Err(format!("'{t}' is not a finite number"));
        }
        nums.push(n);
        if nums.len() > MAX_POINTS {
            return Err(format!("too many data points (max {MAX_POINTS})"));
        }
    }
    if nums.len() < 2 {
        return Err(format!(
            "need at least 2 numbers to compute a difference, got {}",
            nums.len()
        ));
    }
    Ok(nums)
}

/// One differencing pass. `cur[i]` is compared with `cur[i - lag]`; a missing
/// baseline (or a missing current value from an earlier pass) yields `None`.
fn pass(cur: &[Option<f64>], lag: i64, mode: Mode) -> Vec<Option<f64>> {
    let n = cur.len();
    let mut out = vec![None; n];
    for i in 0..n {
        let j = i as i64 - lag;
        if j < 0 || j >= n as i64 {
            continue;
        }
        if let (Some(c), Some(b)) = (cur[i], cur[j as usize]) {
            out[i] = mode.apply(c, b);
        }
    }
    out
}

fn ordinal(order: u32) -> String {
    match order {
        1 => "first".into(),
        2 => "second".into(),
        3 => "third".into(),
        4 => "fourth".into(),
        n => format!("{n}th"),
    }
}

fn polynomial_shape(order: u32) -> &'static str {
    match order {
        1 => "a linear (arithmetic) relation",
        2 => "a quadratic relation",
        3 => "a cubic relation",
        _ => "a polynomial relation of that degree",
    }
}

fn fmt(v: f64) -> String {
    // Trim a trailing `.0` so whole numbers read as `3`, not `3.0`.
    let s = format!("{v}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

fn unit(mode: Mode) -> &'static str {
    match mode {
        Mode::Difference => "",
        Mode::Percent => "%",
        Mode::Ratio => "x",
        Mode::Log => " log units",
    }
}

fn interpret(mode: Mode, lag: i64, order: u32, s: &Summary) -> String {
    let label = match mode {
        Mode::Difference => format!("{} differences", ordinal(order)),
        Mode::Percent => format!("{}-order percent changes", ordinal(order)),
        Mode::Ratio => format!("{}-order ratios", ordinal(order)),
        Mode::Log => format!("{}-order log differences", ordinal(order)),
    };
    if s.defined == 0 {
        return format!(
            "No {label} could be computed — all {} comparisons were undefined (a zero or non-positive baseline).",
            s.undefined
        );
    }
    let mut out = String::new();
    if s.constant {
        let v = s.min.unwrap_or(0.0);
        if mode == Mode::Difference && lag == 1 {
            out.push_str(&format!(
                "The {label} are constant at {}{}, so the series follows {}.",
                fmt(v),
                unit(mode),
                polynomial_shape(order)
            ));
        } else {
            out.push_str(&format!(
                "All {} {label} are identical at {}{}.",
                s.defined,
                fmt(v),
                unit(mode)
            ));
        }
    } else {
        out.push_str(&format!(
            "{} up, {} down, {} flat across {} {label}; values run from {}{} to {}{} with a mean of {}{}.",
            s.increases,
            s.decreases,
            s.unchanged,
            s.defined,
            fmt(s.min.unwrap_or(0.0)),
            unit(mode),
            fmt(s.max.unwrap_or(0.0)),
            unit(mode),
            fmt(s.mean.unwrap_or(0.0)),
            unit(mode),
        ));
        if let (Some(v), Some(i)) = (s.largest_move, s.largest_move_index) {
            out.push_str(&format!(
                " The largest move is {}{} at index {}.",
                fmt(v),
                unit(mode),
                i
            ));
        }
    }
    if s.undefined > 0 {
        out.push_str(&format!(
            " {} comparison(s) were undefined (a zero or non-positive baseline) and are reported as null.",
            s.undefined
        ));
    }
    if lag < 0 {
        out.push_str(&format!(
            " Note: lag {lag} compares each value with the one {} position(s) LATER, so the warm-up sits at the end of the series.",
            -lag
        ));
    }
    out
}

/// Compute the differences of `text` (a numeric series).
///
/// * `lag` — baseline distance; positive looks back, negative looks ahead. Never 0.
/// * `order` — how many times to repeat the transform (1..=10).
/// * `mode_s` — `difference` | `percent` | `ratio` | `log`.
/// * `decimals` — rounding for every returned number (0..=10).
/// * `drop_warmup` — drop the leading (or trailing, for a negative lag) warm-up
///   rows instead of returning them as aligned `null`s.
pub fn compute(
    text: &str,
    lag: i64,
    order: u32,
    mode_s: &str,
    decimals: u32,
    drop_warmup: bool,
) -> Result<Differences, String> {
    let mode = Mode::parse(mode_s)?;
    if lag == 0 {
        return Err(
            "lag must not be 0 — use a positive lag to compare with an earlier value, or a negative lag to compare with a later one"
                .into(),
        );
    }
    if lag.abs() > MAX_LAG {
        return Err(format!(
            "lag must be between -{MAX_LAG} and {MAX_LAG} (got {lag})"
        ));
    }
    if order < 1 || order as i64 > MAX_ORDER {
        return Err(format!("order must be between 1 and {MAX_ORDER} (got {order})"));
    }
    if decimals as i64 > MAX_DECIMALS {
        return Err(format!(
            "decimals must be between 0 and {MAX_DECIMALS} (got {decimals})"
        ));
    }

    let series = parse_series(text)?;
    let n = series.len();
    let step = lag.unsigned_abs() as usize;
    let warm = step * order as usize;
    if warm >= n {
        return Err(format!(
            "not enough data points: lag {lag} at order {order} leaves no comparisons — it needs more than {warm} values, got {n}"
        ));
    }

    let mut cur: Vec<Option<f64>> = series.iter().map(|v| Some(*v)).collect();
    for _ in 0..order {
        cur = pass(&cur, lag, mode);
    }
    let cur: Vec<Option<f64>> = cur
        .iter()
        .map(|o| o.map(|v| round_to(v, decimals)))
        .collect();

    // The rows that actually had a baseline: the tail for a positive lag, the
    // head for a negative one. `offset` maps a body position to a series index.
    let offset = if lag > 0 { warm } else { 0 };
    let body: &[Option<f64>] = if lag > 0 {
        &cur[warm..]
    } else {
        &cur[..n - warm]
    };

    let neutral = mode.neutral();
    let mut defined = 0usize;
    let mut undefined = 0usize;
    let (mut increases, mut decreases, mut unchanged) = (0usize, 0usize, 0usize);
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut sum = 0.0f64;
    let mut largest: Option<(f64, usize)> = None;
    let mut constant = true;
    let mut first_value: Option<f64> = None;
    for (pos, v) in body.iter().enumerate() {
        let Some(v) = *v else {
            undefined += 1;
            continue;
        };
        defined += 1;
        if v > neutral {
            increases += 1;
        } else if v < neutral {
            decreases += 1;
        } else {
            unchanged += 1;
        }
        min = min.min(v);
        max = max.max(v);
        sum += v;
        let dist = (v - neutral).abs();
        if largest.map_or(true, |(bv, _)| dist > (bv - neutral).abs()) {
            largest = Some((v, offset + pos));
        }
        match first_value {
            None => first_value = Some(v),
            Some(f) => {
                if f != v {
                    constant = false;
                }
            }
        }
    }
    let constant = constant && defined >= 2 && undefined == 0;

    let summary = Summary {
        warmup: warm,
        defined,
        undefined,
        increases,
        decreases,
        unchanged,
        min: (defined > 0).then(|| round_to(min, decimals)),
        max: (defined > 0).then(|| round_to(max, decimals)),
        mean: (defined > 0).then(|| round_to(sum / defined as f64, decimals)),
        sum: (defined > 0).then(|| round_to(sum, decimals)),
        largest_move: largest.map(|(v, _)| v),
        largest_move_index: largest.map(|(_, i)| i),
        constant,
    };

    let (values, indices): (Vec<Option<f64>>, Vec<usize>) = if drop_warmup {
        (body.to_vec(), (offset..offset + body.len()).collect())
    } else {
        (cur.clone(), (0..n).collect())
    };

    let interpretation = interpret(mode, lag, order, &summary);

    Ok(Differences {
        count: n,
        lag,
        order,
        mode: mode.name().to_string(),
        decimals,
        drop_warmup,
        values,
        indices,
        summary,
        interpretation,
    })
}

/// Convenience wrapper returning the compact JSON string (chat + CLI surface).
pub fn compute_json(
    text: &str,
    lag: i64,
    order: u32,
    mode_s: &str,
    decimals: u32,
    drop_warmup: bool,
) -> Result<String, String> {
    let d = compute(text, lag, order, mode_s, decimals, drop_warmup)?;
    serde_json::to_string(&d).map_err(|e| format!("serialization failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some(v: &[f64]) -> Vec<Option<f64>> {
        v.iter().map(|x| Some(*x)).collect()
    }

    #[test]
    fn first_difference_aligns_nulls_by_default() {
        let d = compute("2, 5, 9, 14", 1, 1, "difference", 6, false).unwrap();
        assert_eq!(d.count, 4);
        assert_eq!(d.values, vec![None, Some(3.0), Some(4.0), Some(5.0)]);
        assert_eq!(d.indices, vec![0, 1, 2, 3]);
        assert_eq!(d.summary.warmup, 1);
        assert_eq!(d.summary.defined, 3);
        assert_eq!(d.summary.undefined, 0);
        assert_eq!(d.summary.increases, 3);
        assert_eq!(d.summary.min, Some(3.0));
        assert_eq!(d.summary.max, Some(5.0));
        assert_eq!(d.summary.mean, Some(4.0));
        assert_eq!(d.summary.sum, Some(12.0));
        assert_eq!(d.summary.largest_move, Some(5.0));
        assert_eq!(d.summary.largest_move_index, Some(3));
        assert!(!d.summary.constant);
    }

    #[test]
    fn drop_warmup_returns_the_shorter_form() {
        let d = compute("2 5 9 14", 1, 1, "difference", 6, true).unwrap();
        assert_eq!(d.values, some(&[3.0, 4.0, 5.0]));
        assert_eq!(d.indices, vec![1, 2, 3]);
        assert_eq!(d.summary.warmup, 1);
    }

    #[test]
    fn constant_first_differences_read_as_linear() {
        let d = compute("1 4 7 10", 1, 1, "difference", 6, false).unwrap();
        assert!(d.summary.constant);
        assert!(
            d.interpretation.contains("constant at 3")
                && d.interpretation.contains("linear (arithmetic) relation"),
            "got: {}",
            d.interpretation
        );
    }

    #[test]
    fn constant_second_differences_read_as_quadratic() {
        // 1, 4, 9, 16, 25 → first diffs 3,5,7,9 → second diffs 2,2,2
        let d = compute("1 4 9 16 25", 1, 2, "difference", 6, false).unwrap();
        assert_eq!(d.summary.warmup, 2);
        assert_eq!(
            d.values,
            vec![None, None, Some(2.0), Some(2.0), Some(2.0)]
        );
        assert!(d.summary.constant);
        assert!(
            d.interpretation.contains("quadratic relation"),
            "got: {}",
            d.interpretation
        );
    }

    #[test]
    fn negative_lag_compares_with_a_later_value() {
        // lag -1: out[i] = x[i] - x[i+1]; the warm-up moves to the end.
        let d = compute("2 5 9 14", -1, 1, "difference", 6, false).unwrap();
        assert_eq!(d.values, vec![Some(-3.0), Some(-4.0), Some(-5.0), None]);
        assert_eq!(d.summary.warmup, 1);
        assert_eq!(d.summary.decreases, 3);
        assert!(d.interpretation.contains("position(s) LATER"), "got: {}", d.interpretation);
    }

    #[test]
    fn negative_lag_with_drop_warmup_trims_the_tail() {
        let d = compute("2 5 9 14", -1, 1, "difference", 6, true).unwrap();
        assert_eq!(d.values, some(&[-3.0, -4.0, -5.0]));
        assert_eq!(d.indices, vec![0, 1, 2]);
    }

    #[test]
    fn seasonal_lag_skips_back_that_many_positions() {
        let d = compute("10 20 30 12 24 36", 3, 1, "difference", 6, true).unwrap();
        assert_eq!(d.values, some(&[2.0, 4.0, 6.0]));
        assert_eq!(d.indices, vec![3, 4, 5]);
    }

    #[test]
    fn percent_mode_is_signed_and_scaled_to_100() {
        let d = compute("100 110 99", 1, 1, "percent", 2, true).unwrap();
        assert_eq!(d.values, some(&[10.0, -10.0]));
        assert_eq!(d.mode, "percent");
        assert_eq!(d.summary.increases, 1);
        assert_eq!(d.summary.decreases, 1);
    }

    #[test]
    fn ratio_mode_counts_moves_against_one() {
        let d = compute("4 8 8 2", 1, 1, "ratio", 4, true).unwrap();
        assert_eq!(d.values, some(&[2.0, 1.0, 0.25]));
        assert_eq!(d.summary.increases, 1);
        assert_eq!(d.summary.unchanged, 1);
        assert_eq!(d.summary.decreases, 1);
    }

    #[test]
    fn log_mode_returns_the_natural_log_ratio() {
        let d = compute("1 2.718281828459045", 1, 1, "log", 6, true).unwrap();
        assert_eq!(d.values, some(&[1.0]));
    }

    #[test]
    fn zero_baseline_is_null_and_counted_as_undefined() {
        let d = compute("0 5 10", 1, 1, "percent", 6, true).unwrap();
        assert_eq!(d.values, vec![None, Some(100.0)]);
        assert_eq!(d.summary.undefined, 1);
        assert_eq!(d.summary.defined, 1);
        assert!(
            d.interpretation.contains("undefined"),
            "got: {}",
            d.interpretation
        );
    }

    #[test]
    fn non_positive_log_input_is_null_not_infinity() {
        let d = compute("-1 5 10", 1, 1, "log", 6, true).unwrap();
        assert_eq!(d.values[0], None);
        assert_eq!(d.summary.undefined, 1);
        let json = compute_json("-1 5 10", 1, 1, "log", 6, true).unwrap();
        assert!(!json.contains("inf"), "JSON must never carry an infinity: {json}");
    }

    #[test]
    fn decimals_round_the_output() {
        let d = compute("3 10", 1, 1, "percent", 2, true).unwrap();
        // (10-3)/3*100 = 233.3333… → 233.33
        assert_eq!(d.values, some(&[233.33]));
        let d0 = compute("3 10", 1, 1, "percent", 0, true).unwrap();
        assert_eq!(d0.values, some(&[233.0]));
    }

    #[test]
    fn rejects_lag_zero() {
        let err = compute("1 2 3", 0, 1, "difference", 6, false).unwrap_err();
        assert!(err.contains("lag must not be 0"), "got: {err}");
    }

    #[test]
    fn rejects_order_out_of_range() {
        let err = compute("1 2 3", 1, 11, "difference", 6, false).unwrap_err();
        assert!(err.contains("order must be between 1 and 10"), "got: {err}");
        let err0 = compute("1 2 3", 1, 0, "difference", 6, false).unwrap_err();
        assert!(err0.contains("order must be between 1 and 10"), "got: {err0}");
    }

    #[test]
    fn rejects_unknown_mode() {
        let err = compute("1 2 3", 1, 1, "sideways", 6, false).unwrap_err();
        assert!(err.contains("mode must be one of"), "got: {err}");
    }

    #[test]
    fn rejects_series_too_short_for_the_lag() {
        let err = compute("1 2 3", 1, 3, "difference", 6, false).unwrap_err();
        assert!(err.contains("not enough data points"), "got: {err}");
    }

    #[test]
    fn rejects_non_numeric_and_single_value() {
        let err = compute("1 2 oops", 1, 1, "difference", 6, false).unwrap_err();
        assert!(err.contains("is not a number"), "got: {err}");
        let err2 = compute("42", 1, 1, "difference", 6, false).unwrap_err();
        assert!(err2.contains("at least 2 numbers"), "got: {err2}");
    }

    #[test]
    fn rejects_decimals_out_of_range() {
        let err = compute("1 2 3", 1, 1, "difference", 11, false).unwrap_err();
        assert!(err.contains("decimals must be between 0 and 10"), "got: {err}");
    }

    #[test]
    fn parses_newline_and_semicolon_separators() {
        let d = compute("1\n3;6\t10", 1, 1, "difference", 6, true).unwrap();
        assert_eq!(d.values, some(&[2.0, 3.0, 4.0]));
    }

    #[test]
    fn json_carries_the_summary_and_interpretation() {
        let s = compute_json("2 5 9 14", 1, 1, "difference", 6, false).unwrap();
        assert!(s.contains("\"count\":4"));
        assert!(s.contains("\"mode\":\"difference\""));
        assert!(s.contains("\"summary\""));
        assert!(s.contains("\"interpretation\""));
    }
}
