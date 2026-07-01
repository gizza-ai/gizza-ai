//! gizza-ai/z-score-normalize core — standardize a list of numbers.
//!
//! Two methods:
//! - **z-score** (standardization): subtract the mean and divide by the standard
//!   deviation, so the output has mean 0 and standard deviation 1. Uses the
//!   population standard deviation (÷N) by default — matching scikit-learn's
//!   `StandardScaler` and the numpy default — or the sample standard deviation
//!   (÷N−1) when `sample` is set.
//! - **min-max** scaling: rescale linearly into the 0–1 range, so the smallest
//!   value maps to 0 and the largest to 1.
//! - **max-abs** scaling: divide by the largest absolute value, mapping into
//!   [−1, 1] while preserving sign and sparsity (matching scikit-learn's
//!   `MaxAbsScaler`).
//! - **robust** scaling: subtract the median and divide by the interquartile
//!   range (Q3−Q1), so outliers barely affect the scale (matching scikit-learn's
//!   `RobustScaler`).
//!
//! Pure-Rust, dependency-free besides serde for the output struct.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Normalized {
    /// "z-score", "min-max", "max-abs", or "robust".
    pub method: String,
    pub count: usize,
    /// The standardized / scaled values, in input order.
    pub values: Vec<f64>,
    // z-score parameters (present only for method = "z-score").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub std_dev: Option<f64>,
    /// Whether the sample (÷N−1) standard deviation was used; only for z-score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample: Option<bool>,
    // min-max parameters (present only for method = "min-max").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Largest absolute value; present only for method = "max-abs".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_abs: Option<f64>,
    // robust parameters (present only for method = "robust").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub median: Option<f64>,
    /// Interquartile range Q3−Q1; present only for method = "robust".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iqr: Option<f64>,
}

fn round6(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    // avoid emitting "-0.0"
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Linear-interpolated quantile (numpy/scikit-learn default "linear" method) of
/// an already-sorted slice. `q` is in [0, 1].
fn quantile_sorted(sorted: &[f64], q: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let pos = q * (n - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    let frac = pos - lo as f64;
    sorted[lo] + (sorted[hi] - sorted[lo]) * frac
}

/// Parse whitespace/comma/semicolon-separated numbers.
fn parse_numbers(text: &str) -> Result<Vec<f64>, String> {
    let mut nums = Vec::new();
    for tok in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        let n: f64 = t.parse().map_err(|_| format!("'{t}' is not a number"))?;
        if !n.is_finite() {
            return Err(format!("'{t}' is not a finite number"));
        }
        nums.push(n);
    }
    if nums.is_empty() {
        return Err(
            "no numbers found — provide a list separated by spaces, commas, or newlines".into(),
        );
    }
    Ok(nums)
}

/// Standardize (z-score) or min-max scale a text list of numbers.
///
/// `method` accepts "z-score" (aliases: "zscore", "z", "standardize",
/// "standard") or "min-max" (aliases: "minmax", "min_max", "scale"). `sample`
/// selects the sample (÷N−1) standard deviation for z-score; it is ignored for
/// min-max.
pub fn normalize(text: &str, method: &str, sample: bool) -> Result<Normalized, String> {
    let nums = parse_numbers(text)?;
    let n = nums.len();
    let m = method.trim().to_ascii_lowercase();
    match m.as_str() {
        "z-score" | "zscore" | "z" | "standardize" | "standard" | "" => {
            let mean = nums.iter().sum::<f64>() / n as f64;
            let ss: f64 = nums.iter().map(|x| (x - mean) * (x - mean)).sum();
            if sample && n < 2 {
                return Err(
                    "sample standard deviation needs at least 2 values (n−1 = 0)".into(),
                );
            }
            let denom = if sample { n as f64 - 1.0 } else { n as f64 };
            let std_dev = (ss / denom).sqrt();
            if std_dev == 0.0 {
                return Err(
                    "all values are identical — z-score is undefined (standard deviation is zero)"
                        .into(),
                );
            }
            let values = nums.iter().map(|x| round6((x - mean) / std_dev)).collect();
            Ok(Normalized {
                method: "z-score".into(),
                count: n,
                values,
                mean: Some(round6(mean)),
                std_dev: Some(round6(std_dev)),
                sample: Some(sample),
                min: None,
                max: None,
                max_abs: None,
                median: None,
                iqr: None,
            })
        }
        "min-max" | "minmax" | "min_max" | "scale" => {
            let min = nums.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let range = max - min;
            if range == 0.0 {
                return Err(
                    "all values are identical — min-max scaling is undefined (range is zero)"
                        .into(),
                );
            }
            let values = nums.iter().map(|x| round6((x - min) / range)).collect();
            Ok(Normalized {
                method: "min-max".into(),
                count: n,
                values,
                mean: None,
                std_dev: None,
                sample: None,
                min: Some(round6(min)),
                max: Some(round6(max)),
                max_abs: None,
                median: None,
                iqr: None,
            })
        }
        "max-abs" | "maxabs" | "max_abs" | "maxabsolute" => {
            let max_abs = nums.iter().cloned().fold(0.0_f64, |a, x| a.max(x.abs()));
            if max_abs == 0.0 {
                return Err(
                    "all values are zero — max-abs scaling is undefined (largest absolute value is zero)"
                        .into(),
                );
            }
            let values = nums.iter().map(|x| round6(x / max_abs)).collect();
            Ok(Normalized {
                method: "max-abs".into(),
                count: n,
                values,
                mean: None,
                std_dev: None,
                sample: None,
                min: None,
                max: None,
                max_abs: Some(round6(max_abs)),
                median: None,
                iqr: None,
            })
        }
        "robust" | "iqr" | "median" => {
            let mut sorted = nums.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = quantile_sorted(&sorted, 0.5);
            let q1 = quantile_sorted(&sorted, 0.25);
            let q3 = quantile_sorted(&sorted, 0.75);
            let iqr = q3 - q1;
            if iqr == 0.0 {
                return Err(
                    "interquartile range is zero — robust scaling is undefined (Q3 equals Q1)"
                        .into(),
                );
            }
            let values = nums.iter().map(|x| round6((x - median) / iqr)).collect();
            Ok(Normalized {
                method: "robust".into(),
                count: n,
                values,
                mean: None,
                std_dev: None,
                sample: None,
                min: None,
                max: None,
                max_abs: None,
                median: Some(round6(median)),
                iqr: Some(round6(iqr)),
            })
        }
        other => Err(format!(
            "unknown method '{other}' — use 'z-score', 'min-max', 'max-abs', or 'robust'"
        )),
    }
}

/// Human-readable summary (used by the page): the normalized values, one per
/// line, preceded by a short header describing the transform applied.
pub fn summary(text: &str, method: &str, sample: bool) -> Result<String, String> {
    let r = normalize(text, method, sample)?;
    let values = r
        .values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let header = match r.method.as_str() {
        "z-score" => format!(
            "z-score ({} std dev) — mean {}, std dev {}",
            if sample { "sample ÷N−1" } else { "population ÷N" },
            r.mean.unwrap(),
            r.std_dev.unwrap(),
        ),
        "min-max" => format!(
            "min-max scaled to 0–1 — min {}, max {}",
            r.min.unwrap(),
            r.max.unwrap(),
        ),
        "max-abs" => format!(
            "max-abs scaled to [−1, 1] — max abs {}",
            r.max_abs.unwrap(),
        ),
        _ => format!(
            "robust scaled — median {}, IQR {}",
            r.median.unwrap(),
            r.iqr.unwrap(),
        ),
    };
    Ok(format!("{header}\n\n{values}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zscore_population() {
        // mean 5, population std dev 2.
        let r = normalize("2, 4, 4, 4, 5, 5, 7, 9", "z-score", false).unwrap();
        assert_eq!(r.method, "z-score");
        assert_eq!(r.count, 8);
        assert_eq!(r.mean, Some(5.0));
        assert_eq!(r.std_dev, Some(2.0));
        assert_eq!(
            r.values,
            vec![-1.5, -0.5, -0.5, -0.5, 0.0, 0.0, 1.0, 2.0]
        );
    }

    #[test]
    fn zscore_default_method_is_zscore() {
        let r = normalize("1 2 3", "", false).unwrap();
        assert_eq!(r.method, "z-score");
    }

    #[test]
    fn zscore_output_has_zero_mean_unit_variance() {
        let r = normalize("10 20 30 40 50", "z-score", false).unwrap();
        let mean: f64 = r.values.iter().sum::<f64>() / r.values.len() as f64;
        assert!(mean.abs() < 1e-9, "mean should be ~0, got {mean}");
        let var: f64 =
            r.values.iter().map(|v| v * v).sum::<f64>() / r.values.len() as f64;
        assert!((var - 1.0).abs() < 1e-6, "population variance should be ~1, got {var}");
    }

    #[test]
    fn zscore_sample_uses_n_minus_1() {
        // set with population std 2, sample std = sqrt(32/7) ≈ 2.138090
        let pop = normalize("2, 4, 4, 4, 5, 5, 7, 9", "z-score", false).unwrap();
        let samp = normalize("2, 4, 4, 4, 5, 5, 7, 9", "z-score", true).unwrap();
        assert_eq!(pop.std_dev, Some(2.0));
        assert_eq!(samp.std_dev, Some(round6((32.0f64 / 7.0).sqrt())));
        assert_eq!(samp.sample, Some(true));
    }

    #[test]
    fn minmax_basic() {
        let r = normalize("2, 4, 4, 4, 5, 5, 7, 9", "min-max", false).unwrap();
        assert_eq!(r.method, "min-max");
        assert_eq!(r.min, Some(2.0));
        assert_eq!(r.max, Some(9.0));
        assert_eq!(r.values.first(), Some(&0.0));
        assert_eq!(r.values.last(), Some(&1.0));
        assert_eq!(r.values[1], round6(2.0 / 7.0));
    }

    #[test]
    fn minmax_aliases() {
        assert_eq!(normalize("1 2 3", "minmax", false).unwrap().method, "min-max");
        assert_eq!(normalize("1 2 3", "scale", false).unwrap().method, "min-max");
    }

    #[test]
    fn constant_input_is_error() {
        assert!(normalize("5 5 5", "z-score", false).is_err());
        assert!(normalize("5 5 5", "min-max", false).is_err());
    }

    #[test]
    fn sample_needs_two_values() {
        assert!(normalize("42", "z-score", true).is_err());
    }

    #[test]
    fn maxabs_basic() {
        // largest abs value is 8, sign preserved, range [-1, 1].
        let r = normalize("-8, -4, 0, 2, 8", "max-abs", false).unwrap();
        assert_eq!(r.method, "max-abs");
        assert_eq!(r.max_abs, Some(8.0));
        assert_eq!(r.values, vec![-1.0, -0.5, 0.0, 0.25, 1.0]);
    }

    #[test]
    fn maxabs_all_zero_is_error() {
        assert!(normalize("0 0 0", "max-abs", false).is_err());
    }

    #[test]
    fn robust_basic() {
        // 1..5: median 3, Q1 2, Q3 4, IQR 2 → (x-3)/2.
        let r = normalize("1 2 3 4 5", "robust", false).unwrap();
        assert_eq!(r.method, "robust");
        assert_eq!(r.median, Some(3.0));
        assert_eq!(r.iqr, Some(2.0));
        assert_eq!(r.values, vec![-1.0, -0.5, 0.0, 0.5, 1.0]);
    }

    #[test]
    fn robust_alias_and_zero_iqr() {
        assert_eq!(normalize("1 2 3", "iqr", false).unwrap().method, "robust");
        // all identical → IQR 0 → undefined.
        assert!(normalize("7 7 7 7", "robust", false).is_err());
    }

    #[test]
    fn unknown_method_is_error() {
        assert!(normalize("1 2 3", "logistic", false).is_err());
    }

    #[test]
    fn parse_errors() {
        assert!(normalize("", "z-score", false).is_err());
        assert!(normalize("1 2 abc", "z-score", false).is_err());
    }

    #[test]
    fn summary_has_values_and_header() {
        let s = summary("2, 4, 4, 4, 5, 5, 7, 9", "z-score", false).unwrap();
        assert!(s.contains("mean 5"));
        assert!(s.contains("-1.5"));
        assert!(s.contains("\n2\n") || s.ends_with("\n2"));
    }
}
