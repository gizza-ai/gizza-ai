//! percent-difference-calculator core — pure two-value comparison math, shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Three measures, all from the same pair of numbers `a` and `b`:
//!
//! - **absolute difference** — `|a - b|`, plus the signed `b - a` and the mean.
//! - **percent difference** (symmetric) — `|a - b| / |(a + b) / 2| * 100`. The
//!   reference point is the midpoint of the two values, so swapping `a` and `b`
//!   gives the same answer. Undefined when `a + b == 0`.
//! - **percent change** (directional) — `(b - a) / |a| * 100`, i.e. the change
//!   from `a` to `b` relative to the starting value. The reverse direction
//!   `(a - b) / |b| * 100` is reported too. Dividing by `|a|` rather than `a`
//!   keeps the sign of the change equal to the sign of `b - a` on a negative
//!   baseline. Undefined when the respective denominator is zero.
//!
//! `mode` selects which block of measures is reported: `all` (default),
//! `difference`, or `change`. In `all` mode an undefined measure is omitted and
//! explained in a note; in the narrow modes it is a hard error, because the
//! caller asked for exactly that measure.

/// Modes accepted by [`compare`], in the order they appear in the schema.
pub const MODES: [&str; 3] = ["all", "difference", "change"];

/// Largest `decimals` value [`summary`] accepts.
pub const MAX_DECIMALS: u32 = 10;

/// A magnitude ratio above this makes the symmetric percent difference a weak
/// comparison; the result carries an advisory note past it.
const ORDER_OF_MAGNITUDE: f64 = 10.0;

/// Structured comparison result.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// First value as supplied.
    pub a: f64,
    /// Second value as supplied.
    pub b: f64,
    /// Canonical mode name.
    pub mode: String,
    /// `|a - b|`.
    pub absolute_difference: f64,
    /// `b - a`.
    pub signed_difference: f64,
    /// `(a + b) / 2`.
    pub mean: f64,
    /// `|a - b| / |mean| * 100`; `None` when `a + b == 0`.
    pub percent_difference: Option<f64>,
    /// `(b - a) / |a| * 100`; `None` when `a == 0`.
    pub percent_change: Option<f64>,
    /// `(a - b) / |b| * 100`; `None` when `b == 0`.
    pub percent_change_reverse: Option<f64>,
    /// `b / a`; `None` when `a == 0`.
    pub ratio: Option<f64>,
    /// "increase", "decrease" or "no change".
    pub direction: &'static str,
    /// Advisory messages about undefined measures or weak comparisons.
    pub notes: Vec<String>,
}

impl Comparison {
    /// Whether this mode reports the symmetric percent difference.
    fn wants_difference(&self) -> bool {
        self.mode == "all" || self.mode == "difference"
    }

    /// Whether this mode reports the directional percent change and ratio.
    fn wants_change(&self) -> bool {
        self.mode == "all" || self.mode == "change"
    }
}

/// Validate one user-supplied value.
fn finite(label: &str, v: f64) -> Result<(), String> {
    if v.is_finite() {
        return Ok(());
    }
    Err(format!(
        "{label} must be a finite number (got {v}); NaN and infinity cannot be compared"
    ))
}

/// Normalize and validate the mode name.
fn canonical_mode(mode: &str) -> Result<String, String> {
    let m = mode.trim().to_ascii_lowercase();
    let m = if m.is_empty() { "all".to_string() } else { m };
    if MODES.contains(&m.as_str()) {
        return Ok(m);
    }
    Err(format!(
        "mode must be one of: {} (got '{}')",
        MODES.join(", "),
        mode.trim()
    ))
}

/// Compare two values and report the measures selected by `mode`.
///
/// Returns an error for non-finite inputs, an unknown mode, or a narrow mode
/// whose headline measure is undefined for these values.
pub fn compare(a: f64, b: f64, mode: &str) -> Result<Comparison, String> {
    finite("a", a)?;
    finite("b", b)?;
    let mode = canonical_mode(mode)?;

    let signed_difference = b - a;
    let mean = (a + b) / 2.0;
    let direction = if signed_difference > 0.0 {
        "increase"
    } else if signed_difference < 0.0 {
        "decrease"
    } else {
        "no change"
    };

    let percent_difference = if mean == 0.0 {
        None
    } else {
        Some(signed_difference.abs() / mean.abs() * 100.0)
    };
    let percent_change = if a == 0.0 {
        None
    } else {
        Some(signed_difference / a.abs() * 100.0)
    };
    let percent_change_reverse = if b == 0.0 {
        None
    } else {
        Some(-signed_difference / b.abs() * 100.0)
    };
    let ratio = if a == 0.0 { None } else { Some(b / a) };

    let mut c = Comparison {
        a,
        b,
        mode,
        absolute_difference: signed_difference.abs(),
        signed_difference,
        mean,
        percent_difference,
        percent_change,
        percent_change_reverse,
        ratio,
        direction,
        notes: Vec::new(),
    };

    if c.wants_difference() && c.percent_difference.is_none() {
        let msg = format!(
            "percent difference is undefined here: a + b = 0, so the mean is 0 and there is \
             nothing to measure the gap against. The values {} and {} are equal and opposite \
             — compare their magnitudes instead, or use mode=change.",
            trim_num(a),
            trim_num(b)
        );
        if c.mode == "difference" {
            return Err(msg);
        }
        c.notes.push(msg);
    }
    if c.wants_change() && c.percent_change.is_none() {
        let msg = "percent change from a is undefined: a is 0, so there is no baseline to \
                   divide by. Any move away from 0 is an infinite percent change — report the \
                   absolute difference instead, or use mode=difference."
            .to_string();
        if c.mode == "change" {
            return Err(msg);
        }
        c.notes.push(msg);
    }
    if c.wants_change() && c.percent_change_reverse.is_none() {
        c.notes.push(
            "percent change from b is undefined: b is 0, so the reverse direction has no \
             baseline to divide by."
                .to_string(),
        );
    }

    if c.wants_difference() && c.percent_difference.is_some() {
        let (lo, hi) = if a.abs() <= b.abs() {
            (a.abs(), b.abs())
        } else {
            (b.abs(), a.abs())
        };
        if lo > 0.0 && hi > lo * ORDER_OF_MAGNITUDE {
            c.notes.push(format!(
                "the values differ by more than {}x, so the symmetric percent difference is a \
                 weak comparison — it saturates near 200% for very unequal values. Percent \
                 change is usually the more useful measure here.",
                trim_num(ORDER_OF_MAGNITUDE)
            ));
        }
    }

    Ok(c)
}

/// Format a value at full precision with trailing zeros trimmed (used inside
/// error/note text, where the caller's `decimals` is not in scope).
fn trim_num(v: f64) -> String {
    trim_zeros(format!("{v}"))
}

/// Round to `decimals` places and trim trailing zeros so results read cleanly.
fn fmt(v: f64, decimals: u32) -> String {
    trim_zeros(format!("{:.*}", decimals as usize, v))
}

fn trim_zeros(s: String) -> String {
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    // `-0`, `-0.0000` and friends all mean zero.
    if s.trim_start_matches('-').chars().all(|c| c == '0') {
        return s.trim_start_matches('-').to_string();
    }
    s
}

/// Render a human-readable report for the comparison.
///
/// `decimals` controls display precision only (0 to [`MAX_DECIMALS`]); the math
/// itself is always done at full `f64` precision.
pub fn summary(a: f64, b: f64, mode: &str, decimals: u32) -> Result<String, String> {
    if decimals > MAX_DECIMALS {
        return Err(format!(
            "decimals must be between 0 and {MAX_DECIMALS} (got {decimals})"
        ));
    }
    let c = compare(a, b, mode)?;
    let d = decimals;
    let mut out = format!(
        "Comparing a = {} and b = {}\n\nAbsolute difference |a - b| = {}\nSigned difference b - a = {} ({})",
        fmt(c.a, d),
        fmt(c.b, d),
        fmt(c.absolute_difference, d),
        fmt(c.signed_difference, d),
        c.direction
    );
    if c.wants_difference() {
        out.push_str(&format!("\nMean (a + b) / 2 = {}", fmt(c.mean, d)));
    }

    if let Some(pd) = c.percent_difference.filter(|_| c.wants_difference()) {
        out.push_str(&format!(
            "\n\nPercent difference = {}%   (|a - b| / |mean| * 100)",
            fmt(pd, d)
        ));
    }
    if c.wants_change() {
        if let Some(pc) = c.percent_change {
            out.push_str(&format!(
                "\n\nPercent change a -> b = {}%   ((b - a) / |a| * 100)",
                fmt(pc, d)
            ));
        }
        if let Some(pr) = c.percent_change_reverse {
            out.push_str(&format!("\nPercent change b -> a = {}%", fmt(pr, d)));
        }
        if let Some(r) = c.ratio {
            out.push_str(&format!("\nRatio b / a = {}", fmt(r, d)));
        }
    }

    for note in &c.notes {
        out.push_str(&format!("\n\nNote: {note}"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omni_worked_example_70_vs_85() {
        let c = compare(70.0, 85.0, "all").unwrap();
        assert_eq!(c.absolute_difference, 15.0);
        assert_eq!(c.signed_difference, 15.0);
        assert_eq!(c.mean, 77.5);
        assert_eq!(c.direction, "increase");
        let pd = c.percent_difference.unwrap();
        assert!((pd - 19.354838709677_f64).abs() < 1e-9, "got {pd}");
        let pc = c.percent_change.unwrap();
        assert!((pc - 21.428571428571_f64).abs() < 1e-9, "got {pc}");
    }

    #[test]
    fn percent_difference_is_order_independent() {
        let forward = compare(5.0, 7.0, "difference").unwrap();
        let reverse = compare(7.0, 5.0, "difference").unwrap();
        assert_eq!(forward.percent_difference, reverse.percent_difference);
        let pd = forward.percent_difference.unwrap();
        assert!((pd - 33.333333333333_f64).abs() < 1e-9, "got {pd}");
    }

    #[test]
    fn percent_change_is_not_order_independent() {
        let c = compare(100.0, 120.0, "change").unwrap();
        assert_eq!(c.percent_change, Some(20.0));
        let r = compare(120.0, 100.0, "change").unwrap();
        assert!((r.percent_change.unwrap() + 16.666666666667_f64).abs() < 1e-9);
    }

    #[test]
    fn forward_and_reverse_changes_mirror_each_other() {
        let c = compare(100.0, 120.0, "all").unwrap();
        let r = compare(120.0, 100.0, "all").unwrap();
        assert_eq!(c.percent_change, r.percent_change_reverse);
        assert_eq!(c.percent_change_reverse, r.percent_change);
    }

    #[test]
    fn hits_exactly_100_percent_when_one_value_is_triple_the_other() {
        let c = compare(25.0, 75.0, "difference").unwrap();
        assert_eq!(c.percent_difference, Some(100.0));
    }

    #[test]
    fn identical_values_are_zero_and_flat() {
        let c = compare(42.0, 42.0, "all").unwrap();
        assert_eq!(c.absolute_difference, 0.0);
        assert_eq!(c.percent_difference, Some(0.0));
        assert_eq!(c.percent_change, Some(0.0));
        assert_eq!(c.direction, "no change");
    }

    #[test]
    fn negative_pair_matches_its_positive_mirror() {
        let neg = compare(-70.0, -85.0, "difference").unwrap();
        let pos = compare(70.0, 85.0, "difference").unwrap();
        assert_eq!(neg.percent_difference, pos.percent_difference);
    }

    #[test]
    fn negative_baseline_keeps_the_sign_of_the_move() {
        // -50 -> -40 is a rise of 10 on a baseline of magnitude 50.
        let c = compare(-50.0, -40.0, "change").unwrap();
        assert_eq!(c.direction, "increase");
        assert_eq!(c.percent_change, Some(20.0));
    }

    #[test]
    fn equal_and_opposite_values_note_the_undefined_difference_in_all_mode() {
        let c = compare(5.0, -5.0, "all").unwrap();
        assert_eq!(c.percent_difference, None);
        assert_eq!(c.percent_change, Some(-200.0));
        assert!(c.notes.iter().any(|n| n.contains("a + b = 0")));
    }

    #[test]
    fn error_when_difference_mode_has_a_zero_mean() {
        let err = compare(5.0, -5.0, "difference").unwrap_err();
        assert!(err.contains("a + b = 0"), "got {err}");
    }

    #[test]
    fn zero_baseline_is_noted_in_all_mode_but_fatal_in_change_mode() {
        let c = compare(0.0, 8.0, "all").unwrap();
        assert_eq!(c.percent_change, None);
        assert_eq!(c.ratio, None);
        assert_eq!(c.percent_difference, Some(200.0));
        assert!(c.notes.iter().any(|n| n.contains("a is 0")));

        let err = compare(0.0, 8.0, "change").unwrap_err();
        assert!(err.contains("a is 0"), "got {err}");
    }

    #[test]
    fn zero_second_value_only_kills_the_reverse_direction() {
        let c = compare(8.0, 0.0, "change").unwrap();
        assert_eq!(c.percent_change, Some(-100.0));
        assert_eq!(c.percent_change_reverse, None);
        assert!(c.notes.iter().any(|n| n.contains("b is 0")));
    }

    #[test]
    fn far_apart_values_get_a_weak_comparison_note() {
        let c = compare(1.0, 500.0, "all").unwrap();
        assert!(c.notes.iter().any(|n| n.contains("weak comparison")));
        let close = compare(70.0, 85.0, "all").unwrap();
        assert!(!close.notes.iter().any(|n| n.contains("weak comparison")));
    }

    #[test]
    fn error_on_non_finite_value() {
        let err = compare(f64::NAN, 5.0, "all").unwrap_err();
        assert!(err.contains("finite"), "got {err}");
        let err = compare(1.0, f64::INFINITY, "all").unwrap_err();
        assert!(err.contains("finite"), "got {err}");
    }

    #[test]
    fn error_on_unknown_mode() {
        let err = compare(1.0, 2.0, "sideways").unwrap_err();
        assert!(err.contains("mode"), "got {err}");
    }

    #[test]
    fn mode_is_case_insensitive_and_defaults_to_all() {
        assert_eq!(compare(1.0, 2.0, "  ALL ").unwrap().mode, "all");
        assert_eq!(compare(1.0, 2.0, "").unwrap().mode, "all");
    }

    #[test]
    fn error_on_too_many_decimals() {
        let err = summary(1.0, 2.0, "all", 11).unwrap_err();
        assert!(err.contains("decimals must be between 0 and 10"), "got {err}");
        assert!(summary(1.0, 2.0, "all", 10).is_ok());
    }

    #[test]
    fn summary_renders_the_worked_example() {
        let out = summary(70.0, 85.0, "all", 4).unwrap();
        assert_eq!(
            out,
            "Comparing a = 70 and b = 85\n\
             \n\
             Absolute difference |a - b| = 15\n\
             Signed difference b - a = 15 (increase)\n\
             Mean (a + b) / 2 = 77.5\n\
             \n\
             Percent difference = 19.3548%   (|a - b| / |mean| * 100)\n\
             \n\
             Percent change a -> b = 21.4286%   ((b - a) / |a| * 100)\n\
             Percent change b -> a = -17.6471%\n\
             Ratio b / a = 1.2143"
        );
    }

    #[test]
    fn difference_mode_omits_the_change_block() {
        let out = summary(5.0, 7.0, "difference", 2).unwrap();
        assert!(out.contains("Percent difference = 33.33%"), "got {out}");
        assert!(!out.contains("Percent change"), "got {out}");
        assert!(!out.contains("Ratio"), "got {out}");
    }

    #[test]
    fn change_mode_omits_the_difference_block() {
        let out = summary(100.0, 120.0, "change", 2).unwrap();
        assert!(out.contains("Percent change a -> b = 20%"), "got {out}");
        assert!(!out.contains("Percent difference"), "got {out}");
        assert!(!out.contains("Mean"), "got {out}");
    }

    #[test]
    fn zero_decimals_rounds_to_whole_percents() {
        let out = summary(70.0, 85.0, "difference", 0).unwrap();
        assert!(out.contains("Percent difference = 19%"), "got {out}");
    }

    #[test]
    fn negative_zero_never_leaks_into_the_output() {
        let out = summary(1.0, 1.0, "all", 4).unwrap();
        assert!(!out.contains("-0"), "got {out}");
    }
}
