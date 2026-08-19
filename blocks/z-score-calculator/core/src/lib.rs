//! gizza-ai/z-score-calculator core — standard scores against a known normal distribution.
//!
//! Five modes, all sharing one `values` list and the same `mean` / `std_dev` pair:
//!
//! - **score** (default) — raw score(s) → `z = (x − μ) / σ`, plus the percentile and the
//!   left-tail, right-tail and two-tailed probabilities off the standard normal curve.
//!   When `n > 1` the denominator becomes the standard error `σ / √n`, which is the form
//!   used to test a *sample mean* against a known population mean.
//! - **raw** — the inverse: z-score(s) → `x = μ + zσ` (again `σ/√n` when `n > 1`).
//! - **critical** — inverse normal CDF: left-tail probability p ∈ (0, 1) → the z with that
//!   area below it, plus the raw score it corresponds to.
//! - **between** — exactly two bounds → the area between them. With the default μ = 0 and
//!   σ = 1 the bounds are read as z-scores; give a real μ/σ and they are raw values.
//! - **dataset** — derive μ and σ from the pasted numbers themselves, then standardize.
//!   `sample` switches to the sample standard deviation (÷N−1).
//!
//! The normal CDF is computed from `erf`/`erfc` via the incomplete gamma function (series
//! below the crossover, continued fraction above), which is accurate to roughly 1e-14 —
//! comfortably past the 12 decimal places `decimals` can ask for. Pure Rust, no dependencies
//! besides serde for the output struct.

use serde::Serialize;
use std::fmt::Write as _;

/// The largest number of values accepted in one call.
pub const MAX_VALUES: usize = 10_000;

/// One row of the result table — which fields are populated depends on the mode.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// The raw score x. Present in every mode except `critical`, where it is the
    /// reconstructed score for the critical z.
    pub value: f64,
    /// The standard score.
    pub z: f64,
    /// Percentage of the distribution below this value, 0–100.
    pub percentile: f64,
    /// P(X < value) — the left-tail area.
    pub p_left: f64,
    /// P(X > value) — the right-tail area.
    pub p_right: f64,
    /// Two-tailed p-value: P(|Z| > |z|).
    pub p_two_tailed: f64,
}

/// The full result of one calculation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZResult {
    /// "score", "raw", "critical", "between" or "dataset".
    pub mode: String,
    pub count: usize,
    /// The population mean used (given, or derived in `dataset` mode).
    pub mean: f64,
    /// The standard deviation used (given, or derived in `dataset` mode).
    pub std_dev: f64,
    /// Sample size; 1 unless the standard-error form was requested.
    pub n: u32,
    /// σ/√n — present only when `n > 1`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standard_error: Option<f64>,
    /// Whether the sample (÷N−1) standard deviation was derived; `dataset` mode only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample: Option<bool>,
    /// One row per input value; empty in `between` mode.
    pub rows: Vec<Row>,
    /// Lower bound; `between` mode only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lower: Option<f64>,
    /// Upper bound; `between` mode only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upper: Option<f64>,
    /// P(lower < X < upper); `between` mode only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probability: Option<f64>,
}

// ---------------------------------------------------------------------------
// Normal distribution maths
// ---------------------------------------------------------------------------

/// ln Γ(x) for x > 0 (Lanczos approximation, g = 5, n = 6 — ~1e-15 relative).
fn ln_gamma(x: f64) -> f64 {
    const COF: [f64; 6] = [
        76.180_091_729_471_46,
        -86.505_320_329_416_77,
        24.014_098_240_830_91,
        -1.231_739_572_450_155,
        0.120_865_097_386_617_7e-2,
        -0.539_523_938_495_3e-5,
    ];
    let mut y = x;
    let tmp = x + 5.5 - (x + 0.5) * (x + 5.5).ln();
    let mut ser = 1.000_000_000_190_015;
    for c in COF {
        y += 1.0;
        ser += c / y;
    }
    -tmp + (2.506_628_274_631_000_5 * ser / x).ln()
}

/// Regularized lower incomplete gamma P(a, x) by its series expansion (converges
/// quickly while x < a + 1).
fn gamma_p_series(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut ap = a;
    let mut del = 1.0 / a;
    let mut sum = del;
    for _ in 0..300 {
        ap += 1.0;
        del *= x / ap;
        sum += del;
        if del.abs() < sum.abs() * 1e-16 {
            break;
        }
    }
    sum * (-x + a * x.ln() - ln_gamma(a)).exp()
}

/// Regularized upper incomplete gamma Q(a, x) by the modified Lentz continued
/// fraction (converges quickly while x >= a + 1).
fn gamma_q_continued_fraction(a: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..300 {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < TINY {
            d = TINY;
        }
        c = b + an / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
    }
    h * (-x + a * x.ln() - ln_gamma(a)).exp()
}

/// The error function, via the incomplete gamma relation erf(x) = P(1/2, x²).
fn erf(x: f64) -> f64 {
    let ax = x.abs();
    let x2 = ax * ax;
    let p = if x2 < 1.5 {
        gamma_p_series(0.5, x2)
    } else {
        1.0 - gamma_q_continued_fraction(0.5, x2)
    };
    if x < 0.0 {
        -p
    } else {
        p
    }
}

/// The complementary error function. Uses the continued fraction directly in the
/// tail so that erfc stays accurate where `1 − erf` would cancel to nothing.
fn erfc(x: f64) -> f64 {
    let ax = x.abs();
    let x2 = ax * ax;
    let q = if x2 < 1.5 {
        1.0 - gamma_p_series(0.5, x2)
    } else {
        gamma_q_continued_fraction(0.5, x2)
    };
    if x < 0.0 {
        2.0 - q
    } else {
        q
    }
}

/// Φ(z) — the standard normal CDF, i.e. the area to the LEFT of `z`.
pub fn norm_cdf(z: f64) -> f64 {
    0.5 * erfc(-z * std::f64::consts::FRAC_1_SQRT_2)
}

/// The area to the RIGHT of `z`, computed without the cancellation that
/// `1 − norm_cdf(z)` would suffer in the upper tail.
pub fn norm_sf(z: f64) -> f64 {
    0.5 * erfc(z * std::f64::consts::FRAC_1_SQRT_2)
}

/// Φ⁻¹(p) — the inverse standard normal CDF, for p in (0, 1).
///
/// Acklam's rational approximation (≈1.15e-9 relative) seeds two Halley steps
/// against our own `erfc`, which lands on full double precision.
pub fn norm_inv(p: f64) -> f64 {
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_690e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    const P_LOW: f64 = 0.02425;

    let mut x = if p < P_LOW {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - P_LOW {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };

    // Halley refinement: e = Φ(x) − p, u = e·√(2π)·e^(x²/2).
    for _ in 0..2 {
        let e = norm_cdf(x) - p;
        if e == 0.0 {
            break;
        }
        let u = e * (2.0 * std::f64::consts::PI).sqrt() * (x * x / 2.0).exp();
        x -= u / (1.0 + x * u / 2.0);
    }
    x
}

// ---------------------------------------------------------------------------
// Parsing + formatting helpers
// ---------------------------------------------------------------------------

/// Round to `decimals` places, collapsing a negative zero to plain 0.
fn round_to(v: f64, decimals: u32) -> f64 {
    let f = 10f64.powi(decimals as i32);
    let r = (v * f).round() / f;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Round to `sig` significant digits.
fn round_sig(v: f64, sig: u32) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return 0.0;
    }
    let mag = v.abs().log10().floor() as i32;
    let f = 10f64.powi(sig as i32 - 1 - mag);
    (v * f).round() / f
}

/// Round a probability-like quantity. Normally that is `decimals` decimal
/// places, but a deep-tail probability (P(Z > 6) ≈ 9.9e-10) would round flat to
/// zero and lose the whole answer — so when that happens, keep `decimals`
/// significant digits instead.
fn round_prob(v: f64, decimals: u32) -> f64 {
    let r = round_to(v, decimals);
    if r != 0.0 || v == 0.0 {
        return r;
    }
    round_sig(v, decimals.max(2))
}

/// Parse a whitespace/comma/semicolon-separated list of finite numbers.
fn parse_numbers(text: &str, what: &str) -> Result<Vec<f64>, String> {
    let mut nums = Vec::new();
    for tok in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        let n: f64 = t
            .parse()
            .map_err(|_| format!("'{t}' is not a number — {what}"))?;
        if !n.is_finite() {
            return Err(format!("'{t}' is not a finite number — {what}"));
        }
        nums.push(n);
        if nums.len() > MAX_VALUES {
            return Err(format!(
                "too many values — this tool accepts at most {MAX_VALUES} at a time"
            ));
        }
    }
    if nums.is_empty() {
        return Err(format!(
            "no numbers found — {what} (separate them with spaces, commas, semicolons, or newlines)"
        ));
    }
    Ok(nums)
}

/// Canonical mode name, accepting the obvious aliases.
fn canon_mode(mode: &str) -> Result<&'static str, String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "" | "score" | "z" | "z-score" | "zscore" | "standard" => Ok("score"),
        "raw" | "raw-score" | "raw_score" | "inverse" | "unstandardize" => Ok("raw"),
        "critical" | "critical-value" | "p-to-z" | "p_to_z" | "inverse-normal" => Ok("critical"),
        "between" | "area" | "range" => Ok("between"),
        "dataset" | "data" | "standardize" => Ok("dataset"),
        other => Err(format!(
            "unknown mode '{other}' — expected one of: score, raw, critical, between, dataset"
        )),
    }
}

/// Build a result row for a raw score `x` measured against `mean` and `spread`.
fn row_for(x: f64, mean: f64, spread: f64, decimals: u32) -> Row {
    let z = (x - mean) / spread;
    let left = norm_cdf(z);
    let right = norm_sf(z);
    // Two-tailed: twice the smaller tail, which avoids cancellation either side.
    let two = 2.0 * norm_sf(z.abs());
    Row {
        value: round_to(x, decimals),
        z: round_to(z, decimals),
        percentile: round_prob(left * 100.0, decimals),
        p_left: round_prob(left, decimals),
        p_right: round_prob(right, decimals),
        p_two_tailed: round_prob(two.min(1.0), decimals),
    }
}

// ---------------------------------------------------------------------------
// The calculation
// ---------------------------------------------------------------------------

/// Compute z-scores (or their inverses) for `values`.
///
/// `mode` is one of `score`, `raw`, `critical`, `between`, `dataset` (aliases accepted).
/// `mean`/`std_dev` describe the reference distribution and are ignored in `dataset`
/// mode, which derives them. `n` is the sample size behind the standard-error form,
/// `sample` selects ÷N−1 when deriving, and `decimals` sets the rounding of every
/// number in the result.
pub fn calculate(
    values: &str,
    mode: &str,
    mean: f64,
    std_dev: f64,
    n: u32,
    sample: bool,
    decimals: u32,
) -> Result<ZResult, String> {
    let mode = canon_mode(mode)?;
    let decimals = decimals.min(12);
    if n == 0 {
        return Err("sample size n must be at least 1".into());
    }
    if !mean.is_finite() {
        return Err("mean must be a finite number".into());
    }
    if !std_dev.is_finite() {
        return Err("standard deviation must be a finite number".into());
    }

    // The divisor: σ for a single observation, σ/√n for a sample mean.
    let spread = std_dev / (n as f64).sqrt();
    let needs_sigma = mode != "dataset";
    if needs_sigma && std_dev <= 0.0 {
        return Err(format!(
            "standard deviation must be greater than 0 (got {std_dev}) — a z-score divides by it"
        ));
    }
    let standard_error = if n > 1 {
        Some(round_to(spread, decimals))
    } else {
        None
    };

    match mode {
        "score" => {
            let xs = parse_numbers(values, "expected one or more raw scores")?;
            let rows = xs
                .iter()
                .map(|&x| row_for(x, mean, spread, decimals))
                .collect::<Vec<_>>();
            Ok(ZResult {
                mode: "score".into(),
                count: rows.len(),
                mean: round_to(mean, decimals),
                std_dev: round_to(std_dev, decimals),
                n,
                standard_error,
                sample: None,
                rows,
                lower: None,
                upper: None,
                probability: None,
            })
        }
        "raw" => {
            let zs = parse_numbers(values, "expected one or more z-scores")?;
            let rows = zs
                .iter()
                .map(|&z| {
                    let x = mean + z * spread;
                    // Recompute from x so every column stays self-consistent.
                    row_for(x, mean, spread, decimals)
                })
                .collect::<Vec<_>>();
            Ok(ZResult {
                mode: "raw".into(),
                count: rows.len(),
                mean: round_to(mean, decimals),
                std_dev: round_to(std_dev, decimals),
                n,
                standard_error,
                sample: None,
                rows,
                lower: None,
                upper: None,
                probability: None,
            })
        }
        "critical" => {
            let ps = parse_numbers(values, "expected one or more probabilities between 0 and 1")?;
            let mut rows = Vec::with_capacity(ps.len());
            for &p in &ps {
                if p <= 0.0 || p >= 1.0 {
                    return Err(format!(
                        "probability {p} is out of range — give a left-tail area strictly between 0 and 1 (e.g. 0.95)"
                    ));
                }
                let z = norm_inv(p);
                rows.push(row_for(mean + z * spread, mean, spread, decimals));
            }
            Ok(ZResult {
                mode: "critical".into(),
                count: rows.len(),
                mean: round_to(mean, decimals),
                std_dev: round_to(std_dev, decimals),
                n,
                standard_error,
                sample: None,
                rows,
                lower: None,
                upper: None,
                probability: None,
            })
        }
        "between" => {
            let bounds = parse_numbers(values, "expected exactly two bounds")?;
            if bounds.len() != 2 {
                return Err(format!(
                    "'between' needs exactly two bounds, got {} — e.g. '-1.96, 1.96'",
                    bounds.len()
                ));
            }
            let (lo, hi) = if bounds[0] <= bounds[1] {
                (bounds[0], bounds[1])
            } else {
                (bounds[1], bounds[0])
            };
            let z_lo = (lo - mean) / spread;
            let z_hi = (hi - mean) / spread;
            // Difference of the two tails on the same side — no cancellation.
            let prob = if z_lo + z_hi > 0.0 {
                norm_sf(z_lo) - norm_sf(z_hi)
            } else {
                norm_cdf(z_hi) - norm_cdf(z_lo)
            };
            let rows = vec![
                row_for(lo, mean, spread, decimals),
                row_for(hi, mean, spread, decimals),
            ];
            Ok(ZResult {
                mode: "between".into(),
                count: 2,
                mean: round_to(mean, decimals),
                std_dev: round_to(std_dev, decimals),
                n,
                standard_error,
                sample: None,
                rows,
                lower: Some(round_to(lo, decimals)),
                upper: Some(round_to(hi, decimals)),
                probability: Some(round_to(prob.clamp(0.0, 1.0), decimals)),
            })
        }
        // "dataset"
        _ => {
            let xs = parse_numbers(values, "expected a list of numbers to standardize")?;
            let count = xs.len();
            if sample && count < 2 {
                return Err("sample standard deviation needs at least 2 values (N−1 = 0)".into());
            }
            let m = xs.iter().sum::<f64>() / count as f64;
            let ss: f64 = xs.iter().map(|x| (x - m) * (x - m)).sum();
            let denom = if sample {
                count as f64 - 1.0
            } else {
                count as f64
            };
            let sd = (ss / denom).sqrt();
            if sd == 0.0 {
                return Err(
                    "all values are identical — z-scores are undefined (the standard deviation is zero)"
                        .into(),
                );
            }
            let derived_spread = sd / (n as f64).sqrt();
            let rows = xs
                .iter()
                .map(|&x| row_for(x, m, derived_spread, decimals))
                .collect::<Vec<_>>();
            Ok(ZResult {
                mode: "dataset".into(),
                count,
                mean: round_to(m, decimals),
                std_dev: round_to(sd, decimals),
                n,
                standard_error: if n > 1 {
                    Some(round_to(derived_spread, decimals))
                } else {
                    None
                },
                sample: Some(sample),
                rows,
                lower: None,
                upper: None,
                probability: None,
            })
        }
    }
}

/// Trim a rounded f64 to a plain decimal string (no scientific notation for the
/// ranges this tool produces, no trailing zeros).
fn fmt(v: f64, decimals: u32) -> String {
    let s = format!("{v:.*}", decimals as usize);
    if !s.contains('.') {
        return s;
    }
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" {
        "0".into()
    } else {
        t.to_string()
    }
}

/// Human-readable rendering, used by the page and as the CLI's text output.
pub fn summary(
    values: &str,
    mode: &str,
    mean: f64,
    std_dev: f64,
    n: u32,
    sample: bool,
    decimals: u32,
) -> Result<String, String> {
    let r = calculate(values, mode, mean, std_dev, n, sample, decimals)?;
    let d = decimals.min(12);
    let mut out = String::new();

    match r.mode.as_str() {
        "between" => {
            let _ = writeln!(
                out,
                "P({} < X < {}) = {}  ({}%)",
                fmt(r.lower.unwrap_or_default(), d),
                fmt(r.upper.unwrap_or_default(), d),
                fmt(r.probability.unwrap_or_default(), d),
                fmt(r.probability.unwrap_or_default() * 100.0, d.max(2))
            );
            let _ = writeln!(
                out,
                "Outside the bounds: {}",
                fmt(1.0 - r.probability.unwrap_or_default(), d)
            );
            let _ = writeln!(out);
            let _ = writeln!(
                out,
                "Lower bound {}: z = {}",
                fmt(r.rows[0].value, d),
                fmt(r.rows[0].z, d)
            );
            let _ = writeln!(
                out,
                "Upper bound {}: z = {}",
                fmt(r.rows[1].value, d),
                fmt(r.rows[1].z, d)
            );
        }
        _ => {
            let label = match r.mode.as_str() {
                "raw" => "Raw scores from z-scores",
                "critical" => "Critical values",
                "dataset" => "Standardized dataset",
                _ => "Z-scores",
            };
            let _ = writeln!(out, "{label} ({} value(s))", r.count);
            let _ = writeln!(out);
            for row in &r.rows {
                let _ = writeln!(
                    out,
                    "x = {}   z = {}   percentile = {}%",
                    fmt(row.value, d),
                    fmt(row.z, d),
                    fmt(row.percentile, d.max(2))
                );
                let _ = writeln!(
                    out,
                    "  P(X < x) = {}   P(X > x) = {}   two-tailed p = {}",
                    fmt(row.p_left, d),
                    fmt(row.p_right, d),
                    fmt(row.p_two_tailed, d)
                );
            }
            let _ = writeln!(out);
        }
    }

    let _ = writeln!(
        out,
        "Mean (μ) = {}   Standard deviation (σ) = {}",
        fmt(r.mean, d),
        fmt(r.std_dev, d)
    );
    if let Some(se) = r.standard_error {
        let _ = writeln!(
            out,
            "Sample size n = {}   Standard error (σ/√n) = {}",
            r.n,
            fmt(se, d)
        );
    }
    if let Some(true) = r.sample {
        let _ = writeln!(out, "Derived with the sample standard deviation (÷N−1)");
    } else if r.sample == Some(false) {
        let _ = writeln!(out, "Derived with the population standard deviation (÷N)");
    }

    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() < tol, "expected {b}, got {a}");
    }

    #[test]
    fn score_mode_matches_the_textbook_example() {
        // IQ 130 against μ=100, σ=15 → z = 2, ~97.72nd percentile.
        let r = calculate("130", "score", 100.0, 15.0, 1, false, 6).unwrap();
        assert_eq!(r.count, 1);
        assert_eq!(r.rows[0].z, 2.0);
        close(r.rows[0].percentile, 97.724987, 1e-6);
        close(r.rows[0].p_left, 0.97725, 1e-6);
        close(r.rows[0].p_right, 0.02275, 1e-6);
        close(r.rows[0].p_two_tailed, 0.0455, 1e-6);
        assert!(r.standard_error.is_none());
    }

    #[test]
    fn score_mode_handles_several_values_and_negatives() {
        let r = calculate("85, 100, 115", "score", 100.0, 15.0, 1, false, 6).unwrap();
        assert_eq!(r.count, 3);
        assert_eq!(r.rows[0].z, -1.0);
        assert_eq!(r.rows[1].z, 0.0);
        assert_eq!(r.rows[2].z, 1.0);
        // Φ(0) is exactly one half.
        assert_eq!(r.rows[1].p_left, 0.5);
    }

    #[test]
    fn standard_error_form_divides_by_sqrt_n() {
        // x̄ = 105, μ = 100, σ = 15, n = 9 → σ/√n = 5 → z = 1.
        let r = calculate("105", "score", 100.0, 15.0, 9, false, 6).unwrap();
        assert_eq!(r.rows[0].z, 1.0);
        assert_eq!(r.standard_error, Some(5.0));
    }

    #[test]
    fn raw_mode_inverts_score_mode() {
        let r = calculate("2", "raw", 100.0, 15.0, 1, false, 6).unwrap();
        assert_eq!(r.rows[0].value, 130.0);
        assert_eq!(r.rows[0].z, 2.0);
    }

    #[test]
    fn critical_mode_returns_the_familiar_z_values() {
        let r = calculate("0.95, 0.975", "critical", 0.0, 1.0, 1, false, 6).unwrap();
        close(r.rows[0].z, 1.644854, 1e-6);
        close(r.rows[1].z, 1.959964, 1e-6);
    }

    #[test]
    fn between_mode_gives_the_95_percent_interval() {
        let r = calculate("-1.959964, 1.959964", "between", 0.0, 1.0, 1, false, 6).unwrap();
        close(r.probability.unwrap(), 0.95, 1e-6);
        assert_eq!(r.lower, Some(-1.959964));
        assert_eq!(r.upper, Some(1.959964));
    }

    #[test]
    fn between_mode_orders_swapped_bounds() {
        let r = calculate("1.96, -1.96", "between", 0.0, 1.0, 1, false, 6).unwrap();
        assert_eq!(r.lower, Some(-1.96));
        assert_eq!(r.upper, Some(1.96));
    }

    #[test]
    fn between_mode_works_in_raw_units() {
        // The 68–95–99.7 rule: μ ± 1σ ≈ 68.27%.
        let r = calculate("85, 115", "between", 100.0, 15.0, 1, false, 6).unwrap();
        close(r.probability.unwrap(), 0.682689, 1e-6);
    }

    #[test]
    fn dataset_mode_derives_mean_and_std_dev() {
        let r = calculate("2 4 4 4 5 5 7 9", "dataset", 0.0, 1.0, 1, false, 6).unwrap();
        assert_eq!(r.mean, 5.0);
        assert_eq!(r.std_dev, 2.0);
        assert_eq!(r.sample, Some(false));
        assert_eq!(r.rows[0].z, -1.5);
        assert_eq!(r.count, 8);
    }

    #[test]
    fn dataset_mode_can_use_the_sample_std_dev() {
        let r = calculate("2 4 4 4 5 5 7 9", "dataset", 0.0, 1.0, 1, true, 6).unwrap();
        close(r.std_dev, 2.13809, 1e-5);
        assert_eq!(r.sample, Some(true));
    }

    #[test]
    fn tails_stay_accurate_far_out() {
        // P(Z > 6) ≈ 9.8659e-10 — a naive 1 − Φ(z) would round this to zero.
        // At 12 decimals the value survives ordinary rounding (1e-12 granularity).
        let r = calculate("6", "score", 0.0, 1.0, 1, false, 12).unwrap();
        close(r.rows[0].p_right, 9.865_876e-10, 1e-12);
        assert!(norm_sf(6.0) > 0.0);
        close(norm_sf(6.0), 9.865_876e-10, 1e-15);
    }

    #[test]
    fn deep_tails_fall_back_to_significant_digits() {
        // At 4 decimals, rounding alone would flatten P(Z > 6) to zero, so
        // round_prob keeps 4 significant digits instead of losing the answer.
        let r = calculate("6", "score", 0.0, 1.0, 1, false, 4).unwrap();
        assert_ne!(r.rows[0].p_right, 0.0);
        close(r.rows[0].p_right, 9.866e-10, 1e-13);
    }

    #[test]
    fn norm_inv_round_trips_through_norm_cdf() {
        for p in [0.001, 0.01, 0.25, 0.5, 0.75, 0.99, 0.999] {
            close(norm_cdf(norm_inv(p)), p, 1e-12);
        }
    }

    #[test]
    fn decimals_controls_rounding() {
        let r = calculate("130", "score", 100.0, 15.0, 1, false, 2).unwrap();
        assert_eq!(r.rows[0].percentile, 97.72);
    }

    #[test]
    fn summary_renders_the_headline_numbers() {
        let s = summary("130", "score", 100.0, 15.0, 1, false, 4).unwrap();
        assert!(s.contains("z = 2"), "{s}");
        assert!(s.contains("Mean (μ) = 100"), "{s}");
    }

    // --- error paths ---

    #[test]
    fn zero_std_dev_is_rejected() {
        let e = calculate("130", "score", 100.0, 0.0, 1, false, 6).unwrap_err();
        assert!(e.contains("greater than 0"), "{e}");
    }

    #[test]
    fn non_numeric_input_is_reported_with_the_bad_token() {
        let e = calculate("130, abc", "score", 100.0, 15.0, 1, false, 6).unwrap_err();
        assert!(e.contains("'abc' is not a number"), "{e}");
    }

    #[test]
    fn empty_input_is_rejected() {
        let e = calculate("   ", "score", 100.0, 15.0, 1, false, 6).unwrap_err();
        assert!(e.contains("no numbers found"), "{e}");
    }

    #[test]
    fn unknown_mode_is_rejected() {
        let e = calculate("1", "sideways", 0.0, 1.0, 1, false, 6).unwrap_err();
        assert!(e.contains("unknown mode"), "{e}");
    }

    #[test]
    fn between_mode_needs_exactly_two_bounds() {
        let e = calculate("1", "between", 0.0, 1.0, 1, false, 6).unwrap_err();
        assert!(e.contains("exactly two bounds"), "{e}");
    }

    #[test]
    fn critical_mode_rejects_out_of_range_probability() {
        let e = calculate("1.5", "critical", 0.0, 1.0, 1, false, 6).unwrap_err();
        assert!(e.contains("out of range"), "{e}");
    }

    #[test]
    fn dataset_mode_rejects_identical_values() {
        let e = calculate("5 5 5", "dataset", 0.0, 1.0, 1, false, 6).unwrap_err();
        assert!(e.contains("identical"), "{e}");
    }

    #[test]
    fn too_many_values_is_rejected() {
        let big = (0..MAX_VALUES + 1)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let e = calculate(&big, "score", 0.0, 1.0, 1, false, 6).unwrap_err();
        assert!(e.contains("too many values"), "{e}");
    }

    #[test]
    fn exactly_max_values_is_accepted() {
        let big = (0..MAX_VALUES)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let r = calculate(&big, "score", 0.0, 1.0, 1, false, 6).unwrap();
        assert_eq!(r.count, MAX_VALUES);
    }
}
