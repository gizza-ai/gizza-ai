//! gizza-ai/descriptive-stats core — descriptive statistics for a list of numbers:
//! count, sum, mean, median, mode, variance & standard deviation (population and
//! sample), quartiles (Q1/Q3/IQR), min, max, range. Pure-Rust, dependency-free
//! (besides serde for the output struct).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    pub count: usize,
    pub sum: f64,
    pub mean: f64,
    pub median: f64,
    /// Most frequent value(s); empty when every value is unique.
    pub mode: Vec<f64>,
    pub min: f64,
    pub max: f64,
    pub range: f64,
    pub q1: f64,
    pub q3: f64,
    pub iqr: f64,
    /// Population variance / standard deviation (divide by N).
    pub variance_population: f64,
    pub std_dev_population: f64,
    /// Sample variance / standard deviation (divide by N-1); null when count < 2.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variance_sample: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub std_dev_sample: Option<f64>,
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// Linear-interpolation percentile (numpy default), `p` in 0.0..=1.0, on a sorted
/// slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let rank = p * (n as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = rank - lo as f64;
        sorted[lo] + (sorted[hi] - sorted[lo]) * frac
    }
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
        return Err("no numbers found — provide a list separated by spaces, commas, or newlines".into());
    }
    Ok(nums)
}

/// Compute descriptive statistics from a text list of numbers.
pub fn describe(text: &str) -> Result<Stats, String> {
    let nums = parse_numbers(text)?;
    let n = nums.len();
    let sum: f64 = nums.iter().sum();
    let mean = sum / n as f64;

    let mut sorted = nums.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = percentile(&sorted, 0.5);
    let q1 = percentile(&sorted, 0.25);
    let q3 = percentile(&sorted, 0.75);
    let min = sorted[0];
    let max = sorted[n - 1];

    // Mode: highest frequency; empty if all values appear once.
    let mut counts: Vec<(f64, usize)> = Vec::new();
    for &v in &sorted {
        match counts.last_mut() {
            Some((val, c)) if *val == v => *c += 1,
            _ => counts.push((v, 1)),
        }
    }
    let max_freq = counts.iter().map(|(_, c)| *c).max().unwrap_or(1);
    let mode: Vec<f64> = if max_freq <= 1 {
        Vec::new()
    } else {
        counts.iter().filter(|(_, c)| *c == max_freq).map(|(v, _)| *v).collect()
    };

    let ss: f64 = nums.iter().map(|x| (x - mean) * (x - mean)).sum();
    let variance_population = ss / n as f64;
    let (variance_sample, std_dev_sample) = if n >= 2 {
        let vs = ss / (n as f64 - 1.0);
        (Some(round6(vs)), Some(round6(vs.sqrt())))
    } else {
        (None, None)
    };

    Ok(Stats {
        count: n,
        sum: round6(sum),
        mean: round6(mean),
        median: round6(median),
        mode: mode.into_iter().map(round6).collect(),
        min: round6(min),
        max: round6(max),
        range: round6(max - min),
        q1: round6(q1),
        q3: round6(q3),
        iqr: round6(q3 - q1),
        variance_population: round6(variance_population),
        std_dev_population: round6(variance_population.sqrt()),
        variance_sample,
        std_dev_sample,
    })
}

/// Human-readable summary (used by the page).
pub fn summary(text: &str) -> Result<String, String> {
    let s = describe(text)?;
    let mode = if s.mode.is_empty() {
        "none (all unique)".to_string()
    } else {
        s.mode.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(", ")
    };
    Ok(format!(
        "count: {}\nsum: {}\nmean: {}\nmedian: {}\nmode: {}\nmin: {}\nmax: {}\nrange: {}\nQ1: {}\nQ3: {}\nIQR: {}\nvariance (pop): {}\nstd dev (pop): {}\nvariance (sample): {}\nstd dev (sample): {}",
        s.count, s.sum, s.mean, s.median, mode, s.min, s.max, s.range, s.q1, s.q3, s.iqr,
        s.variance_population, s.std_dev_population,
        s.variance_sample.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into()),
        s.std_dev_sample.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let s = describe("2, 4, 4, 4, 5, 5, 7, 9").unwrap();
        assert_eq!(s.count, 8);
        assert_eq!(s.sum, 40.0);
        assert_eq!(s.mean, 5.0);
        assert_eq!(s.mode, vec![4.0]);
        assert_eq!(s.min, 2.0);
        assert_eq!(s.max, 9.0);
        // population variance of this classic set is 4, std dev 2.
        assert_eq!(s.variance_population, 4.0);
        assert_eq!(s.std_dev_population, 2.0);
        // sample variance = 32/7 ≈ 4.571429
        assert_eq!(s.variance_sample, Some(round6(32.0 / 7.0)));
    }

    #[test]
    fn median_even_and_odd() {
        assert_eq!(describe("1 2 3 4").unwrap().median, 2.5);
        assert_eq!(describe("1 2 3").unwrap().median, 2.0);
    }

    #[test]
    fn quartiles_linear() {
        // [1,2,3,4,5]: Q1=2, median=3, Q3=4 (numpy linear).
        let s = describe("1 2 3 4 5").unwrap();
        assert_eq!(s.q1, 2.0);
        assert_eq!(s.median, 3.0);
        assert_eq!(s.q3, 4.0);
        assert_eq!(s.iqr, 2.0);
    }

    #[test]
    fn no_mode_when_unique() {
        assert!(describe("1 2 3 4").unwrap().mode.is_empty());
    }

    #[test]
    fn multimodal() {
        let m = describe("1 1 2 2 3").unwrap().mode;
        assert_eq!(m, vec![1.0, 2.0]);
    }

    #[test]
    fn single_value_no_sample_variance() {
        let s = describe("42").unwrap();
        assert_eq!(s.count, 1);
        assert_eq!(s.mean, 42.0);
        assert!(s.variance_sample.is_none());
    }

    #[test]
    fn errors() {
        assert!(describe("").is_err());
        assert!(describe("1 2 abc").is_err());
    }
}
