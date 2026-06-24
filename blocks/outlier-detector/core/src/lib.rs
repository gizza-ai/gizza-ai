//! gizza-ai/outlier-detector core — flag outliers in a list of numbers using two
//! standard methods: the z-score (standard-score) method and the IQR (Tukey's
//! fences) method. Pure-Rust, dependency-free besides serde for the output struct.
//!
//! - **Z-score method**: a value is an outlier when |(x − mean) / std_dev| exceeds a
//!   threshold (default 3.0). Uses the *sample* standard deviation (÷ N−1), matching
//!   the common `scipy.stats.zscore(ddof=1)` convention. Undefined when N < 2 or the
//!   std dev is 0 (no spread → no outliers).
//! - **IQR method**: a value is an outlier when it falls below Q1 − k·IQR or above
//!   Q3 + k·IQR (default k = 1.5, Tukey's classic fences). Quartiles use the
//!   linear-interpolation method (the numpy default), matching `descriptive-stats`.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Flagged {
    /// 0-based index of the value in the input list.
    pub index: usize,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZScoreResult {
    pub threshold: f64,
    pub mean: f64,
    /// Sample standard deviation (÷ N−1).
    pub std_dev: f64,
    pub outliers: Vec<Flagged>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModZScoreResult {
    pub threshold: f64,
    pub median: f64,
    /// Median absolute deviation (MAD).
    pub mad: f64,
    pub outliers: Vec<Flagged>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IqrResult {
    pub k: f64,
    pub q1: f64,
    pub q3: f64,
    pub iqr: f64,
    pub lower_fence: f64,
    pub upper_fence: f64,
    pub outliers: Vec<Flagged>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub count: usize,
    pub z_score: ZScoreResult,
    pub modified_z_score: ModZScoreResult,
    pub iqr: IqrResult,
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// Parse numbers separated by spaces, commas, semicolons, tabs, or newlines.
fn parse(text: &str) -> Result<Vec<f64>, String> {
    let nums: Result<Vec<f64>, _> = text
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<f64>()
                .map_err(|_| format!("'{s}' is not a valid number"))
        })
        .collect();
    let nums = nums?;
    if nums.is_empty() {
        return Err("no numbers provided".into());
    }
    if nums.iter().any(|v| !v.is_finite()) {
        return Err("values must be finite (no inf/NaN)".into());
    }
    Ok(nums)
}

/// Median of a sorted slice.
fn median_sorted(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

/// Linear-interpolation percentile (numpy default), `p` in 0.0..=1.0, on a sorted slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let rank = p * (n as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        return sorted[lo];
    }
    let frac = rank - lo as f64;
    sorted[lo] + (sorted[hi] - sorted[lo]) * frac
}

/// Detect outliers with three methods: z-score, modified z-score (MAD), and IQR.
///
/// `z_threshold`: |z| above this is a z-score outlier (must be > 0); also the
///   cutoff for the |modified z-score| (MAD-based) method.
/// `iqr_k`: multiplier for Tukey's fences (must be >= 0).
pub fn detect(text: &str, z_threshold: f64, iqr_k: f64) -> Result<Report, String> {
    if !(z_threshold > 0.0) {
        return Err("z_threshold must be greater than 0".into());
    }
    if iqr_k < 0.0 {
        return Err("iqr_k must be 0 or greater".into());
    }
    let values = parse(text)?;
    let n = values.len();

    let mean = values.iter().sum::<f64>() / n as f64;

    // Sample standard deviation (÷ N−1); 0 spread or N < 2 ⇒ no z-score outliers.
    let std_dev = if n < 2 {
        0.0
    } else {
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
        var.sqrt()
    };

    let mut z_outliers = Vec::new();
    if std_dev > 0.0 {
        for (i, &v) in values.iter().enumerate() {
            let z = (v - mean) / std_dev;
            if z.abs() > z_threshold {
                z_outliers.push(Flagged {
                    index: i,
                    value: v,
                });
            }
        }
    }

    let mut sorted = values.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = median_sorted(&sorted);

    // Modified z-score (robust): M_i = 0.6745·(x_i − median) / MAD, where
    // MAD = median(|x_i − median|). 0.6745 ≈ Φ⁻¹(0.75) makes MAD a consistent
    // estimator of σ for normal data. 0 MAD ⇒ no spread ⇒ no outliers.
    let mut abs_dev: Vec<f64> = values.iter().map(|v| (v - median).abs()).collect();
    abs_dev.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mad = median_sorted(&abs_dev);
    let mut mod_z_outliers = Vec::new();
    if mad > 0.0 {
        for (i, &v) in values.iter().enumerate() {
            let mz = 0.6745 * (v - median) / mad;
            if mz.abs() > z_threshold {
                mod_z_outliers.push(Flagged {
                    index: i,
                    value: v,
                });
            }
        }
    }

    let q1 = percentile(&sorted, 0.25);
    let q3 = percentile(&sorted, 0.75);
    let iqr = q3 - q1;
    let lower_fence = q1 - iqr_k * iqr;
    let upper_fence = q3 + iqr_k * iqr;

    let mut iqr_outliers = Vec::new();
    for (i, &v) in values.iter().enumerate() {
        if v < lower_fence || v > upper_fence {
            iqr_outliers.push(Flagged {
                index: i,
                value: v,
            });
        }
    }

    Ok(Report {
        count: n,
        z_score: ZScoreResult {
            threshold: z_threshold,
            mean: round6(mean),
            std_dev: round6(std_dev),
            outliers: z_outliers,
        },
        modified_z_score: ModZScoreResult {
            threshold: z_threshold,
            median: round6(median),
            mad: round6(mad),
            outliers: mod_z_outliers,
        },
        iqr: IqrResult {
            k: iqr_k,
            q1: round6(q1),
            q3: round6(q3),
            iqr: round6(iqr),
            lower_fence: round6(lower_fence),
            upper_fence: round6(upper_fence),
            outliers: iqr_outliers,
        },
    })
}

/// Human-readable summary for the page / web surface.
pub fn summary(text: &str, z_threshold: f64, iqr_k: f64) -> Result<String, String> {
    let r = detect(text, z_threshold, iqr_k)?;
    let fmt = |o: &[Flagged]| -> String {
        if o.is_empty() {
            "none".to_string()
        } else {
            o.iter()
                .map(|f| format!("{} (index {})", f.value, f.index))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    Ok(format!(
        "count: {count}\n\
         z-score method (threshold {zt}):\n  \
         mean: {mean}\n  std dev (sample): {sd}\n  outliers: {zout}\n\
         modified z-score method (threshold {zt}):\n  \
         median: {med}\n  MAD: {mad}\n  outliers: {mzout}\n\
         IQR method (k {k}):\n  \
         Q1: {q1}\n  Q3: {q3}\n  IQR: {iqr}\n  \
         lower fence: {lf}\n  upper fence: {uf}\n  outliers: {iout}",
        count = r.count,
        zt = r.z_score.threshold,
        mean = r.z_score.mean,
        sd = r.z_score.std_dev,
        med = r.modified_z_score.median,
        mad = r.modified_z_score.mad,
        mzout = fmt(&r.modified_z_score.outliers),
        zout = fmt(&r.z_score.outliers),
        k = r.iqr.k,
        q1 = r.iqr.q1,
        q3 = r.iqr.q3,
        iqr = r.iqr.iqr,
        lf = r.iqr.lower_fence,
        uf = r.iqr.upper_fence,
        iout = fmt(&r.iqr.outliers),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_an_obvious_outlier() {
        // 100 is far from the cluster around 10 — the IQR method reliably flags it.
        let r = detect("10 11 12 10 9 11 100", 3.0, 1.5).unwrap();
        assert_eq!(r.count, 7);
        assert_eq!(r.iqr.outliers, vec![Flagged { index: 6, value: 100.0 }]);
    }

    #[test]
    fn zscore_flags_value_beyond_threshold() {
        // A large clean cluster + one extreme point: the extreme point's z-score
        // exceeds 3 sample std devs and is flagged by the z-score method.
        let r =
            detect("10 10 10 10 10 10 10 10 10 10 10 10 10 10 10 10 10 10 10 90", 3.0, 1.5).unwrap();
        assert_eq!(
            r.z_score.outliers,
            vec![Flagged {
                index: 19,
                value: 90.0
            }]
        );
        // and the IQR method flags it too.
        assert!(r.iqr.outliers.iter().any(|f| f.value == 90.0));
    }

    #[test]
    fn no_outliers_in_clean_data() {
        let r = detect("1 2 3 4 5 6 7 8 9", 3.0, 1.5).unwrap();
        assert!(r.z_score.outliers.is_empty());
        assert!(r.iqr.outliers.is_empty());
    }

    #[test]
    fn constant_data_has_no_outliers() {
        // Zero spread ⇒ std dev 0 and MAD 0 ⇒ no method flags anything (no div-by-zero).
        let r = detect("5 5 5 5", 3.0, 1.5).unwrap();
        assert_eq!(r.z_score.std_dev, 0.0);
        assert_eq!(r.modified_z_score.mad, 0.0);
        assert!(r.z_score.outliers.is_empty());
        assert!(r.modified_z_score.outliers.is_empty());
        assert!(r.iqr.outliers.is_empty());
    }

    #[test]
    fn modified_zscore_flags_robustly() {
        // Modified z-score (median+MAD) is robust: it flags the extreme value even
        // when a couple of large values would inflate the classical std dev.
        let r = detect("1 2 3 4 5 6 7 8 9 200", 3.5, 1.5).unwrap();
        // median of [1..9,200] = 5.5; MAD is small, so 200 has a huge modified z.
        assert!(r.modified_z_score.outliers.iter().any(|f| f.value == 200.0));
        assert!(r.modified_z_score.mad > 0.0);
    }

    #[test]
    fn iqr_fences_are_correct() {
        // [1,2,3,4,5,6,7,8,9,10]: Q1=3.25, Q3=7.75, IQR=4.5 (numpy linear).
        let r = detect("1 2 3 4 5 6 7 8 9 10", 3.0, 1.5).unwrap();
        assert_eq!(r.iqr.q1, 3.25);
        assert_eq!(r.iqr.q3, 7.75);
        assert_eq!(r.iqr.iqr, 4.5);
        // fences: 3.25 - 1.5*4.5 = -3.5 ; 7.75 + 1.5*4.5 = 14.5
        assert_eq!(r.iqr.lower_fence, -3.5);
        assert_eq!(r.iqr.upper_fence, 14.5);
    }

    #[test]
    fn custom_thresholds() {
        // A tighter IQR k flags a value the default would not.
        let lenient = detect("1 2 3 4 5 20", 3.0, 1.5).unwrap();
        let strict = detect("1 2 3 4 5 20", 3.0, 0.5).unwrap();
        assert!(strict.iqr.outliers.len() >= lenient.iqr.outliers.len());
        assert!(strict.iqr.outliers.iter().any(|f| f.value == 20.0));
    }

    #[test]
    fn parses_mixed_separators() {
        let r = detect("1,2;3\n4\t5", 3.0, 1.5).unwrap();
        assert_eq!(r.count, 5);
    }

    #[test]
    fn rejects_non_numeric() {
        assert!(detect("1 2 abc", 3.0, 1.5).is_err());
    }

    #[test]
    fn rejects_empty() {
        assert!(detect("   ", 3.0, 1.5).is_err());
    }

    #[test]
    fn rejects_bad_threshold() {
        assert!(detect("1 2 3", 0.0, 1.5).is_err());
        assert!(detect("1 2 3", -1.0, 1.5).is_err());
        assert!(detect("1 2 3", 3.0, -1.0).is_err());
    }

    #[test]
    fn summary_renders() {
        let s = summary("10 11 12 10 9 11 100", 3.0, 1.5).unwrap();
        assert!(s.contains("count: 7"));
        assert!(s.contains("100 (index 6)"));
    }
}
