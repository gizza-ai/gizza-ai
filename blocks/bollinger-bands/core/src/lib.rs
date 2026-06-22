//! gizza-ai/bollinger-bands core — Bollinger Bands for a price/value series.
//!
//! For each window of `period` consecutive values the middle band is the simple
//! moving average (SMA); the upper and lower bands are the SMA plus/minus
//! `num_std` times the **population** standard deviation of that same window
//! (the conventional Bollinger definition, ÷N). Also reports %B (where the last
//! price sits within the bands) and the bandwidth ((upper−lower)/middle).
//!
//! Pure-Rust, dependency-free besides serde for the output struct.

use serde::Serialize;

/// One Bollinger Bands data point, aligned to a source index in the input series.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Band {
    /// 0-based index of this value in the original input series.
    pub index: usize,
    /// The price/value at this index.
    pub price: f64,
    /// Middle band = simple moving average over the trailing `period` values.
    pub middle: f64,
    pub upper: f64,
    pub lower: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BollingerResult {
    pub count: usize,
    pub period: usize,
    pub num_std: f64,
    /// One band per window; empty rows (first `period-1`) are omitted.
    pub bands: Vec<Band>,
    /// %B of the most recent point: (price−lower)/(upper−lower). 0 = on the
    /// lower band, 1 = on the upper band. `None` if upper==lower.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_b: Option<f64>,
    /// Bandwidth of the most recent point: (upper−lower)/middle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bandwidth: Option<f64>,
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
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
        return Err("no numbers found — provide a list of prices separated by spaces, commas, or newlines".into());
    }
    Ok(nums)
}

/// Compute Bollinger Bands from a text list of prices/values.
///
/// * `period` — moving-average window length (typically 20). Must be ≥ 1.
/// * `num_std` — standard-deviation multiplier (typically 2.0). Must be ≥ 0.
pub fn compute(text: &str, period: usize, num_std: f64) -> Result<BollingerResult, String> {
    let nums = parse_numbers(text)?;
    compute_from(&nums, period, num_std)
}

/// Core computation over an already-parsed slice.
pub fn compute_from(nums: &[f64], period: usize, num_std: f64) -> Result<BollingerResult, String> {
    if period < 1 {
        return Err("period must be at least 1".into());
    }
    if !num_std.is_finite() || num_std < 0.0 {
        return Err("num_std must be a non-negative number".into());
    }
    let n = nums.len();
    if n < period {
        return Err(format!(
            "need at least {period} values for a period of {period}, but got {n}"
        ));
    }

    let mut bands = Vec::with_capacity(n - period + 1);
    for end in period..=n {
        let window = &nums[end - period..end];
        let mean = window.iter().sum::<f64>() / period as f64;
        // Population standard deviation (÷N) — the conventional Bollinger band.
        let var = window.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / period as f64;
        let sd = var.sqrt();
        let idx = end - 1;
        bands.push(Band {
            index: idx,
            price: round6(nums[idx]),
            middle: round6(mean),
            upper: round6(mean + num_std * sd),
            lower: round6(mean - num_std * sd),
        });
    }

    let last = bands.last().cloned();
    let (percent_b, bandwidth) = match last {
        Some(b) => {
            let span = b.upper - b.lower;
            let pb = if span.abs() < f64::EPSILON {
                None
            } else {
                Some(round6((b.price - b.lower) / span))
            };
            let bw = if b.middle.abs() < f64::EPSILON {
                None
            } else {
                Some(round6((b.upper - b.lower) / b.middle))
            };
            (pb, bw)
        }
        None => (None, None),
    };

    Ok(BollingerResult {
        count: n,
        period,
        num_std,
        bands,
        percent_b,
        bandwidth,
    })
}

/// Human-readable summary (used by the page).
pub fn summary(text: &str, period: usize, num_std: f64) -> Result<String, String> {
    let r = compute(text, period, num_std)?;
    let mut out = String::new();
    out.push_str(&format!(
        "Bollinger Bands — period {}, {}\u{3c3} ({} values)\n\n",
        r.period, r.num_std, r.count
    ));
    out.push_str("index\tprice\tlower\tmiddle\tupper\n");
    for b in &r.bands {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            b.index, b.price, b.lower, b.middle, b.upper
        ));
    }
    if let Some(last) = r.bands.last() {
        out.push_str(&format!(
            "\nLatest (index {}): lower {}, middle {}, upper {}",
            last.index, last.lower, last.middle, last.upper
        ));
        if let Some(pb) = r.percent_b {
            out.push_str(&format!("\n%B: {pb}  (0 = lower band, 1 = upper band)"));
        }
        if let Some(bw) = r.bandwidth {
            out.push_str(&format!("\nBandwidth: {bw}"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_series_zero_width() {
        // All equal → std dev 0 → all three bands equal.
        let r = compute("10 10 10 10 10", 5, 2.0).unwrap();
        assert_eq!(r.bands.len(), 1);
        let b = &r.bands[0];
        assert_eq!(b.middle, 10.0);
        assert_eq!(b.upper, 10.0);
        assert_eq!(b.lower, 10.0);
        // span 0 → %B undefined.
        assert!(r.percent_b.is_none());
        assert_eq!(r.bandwidth, Some(0.0));
    }

    #[test]
    fn known_window() {
        // Values 1..=5, period 5. mean=3, pop var = (4+1+0+1+4)/5 = 2, sd=sqrt(2).
        let r = compute("1 2 3 4 5", 5, 2.0).unwrap();
        assert_eq!(r.bands.len(), 1);
        let b = &r.bands[0];
        assert_eq!(b.middle, 3.0);
        assert_eq!(b.upper, round6(3.0 + 2.0 * 2.0f64.sqrt()));
        assert_eq!(b.lower, round6(3.0 - 2.0 * 2.0f64.sqrt()));
        // last price is 5, at the top → %B should be > 0.5.
        assert!(r.percent_b.unwrap() > 0.5);
    }

    #[test]
    fn rolling_windows_count() {
        // 7 values, period 3 → 5 windows (indices 2..6).
        let r = compute("1 2 3 4 5 6 7", 3, 2.0).unwrap();
        assert_eq!(r.bands.len(), 5);
        assert_eq!(r.bands[0].index, 2);
        assert_eq!(r.bands.last().unwrap().index, 6);
        // first window [1,2,3]: mean 2.
        assert_eq!(r.bands[0].middle, 2.0);
    }

    #[test]
    fn percent_b_midpoint() {
        // Symmetric window where the last value equals the mean → %B = 0.5.
        let r = compute("1 5 3 5 1 3", 6, 2.0).unwrap();
        let b = &r.bands[0];
        assert_eq!(b.middle, 3.0);
        assert_eq!(b.price, 3.0);
        assert_eq!(r.percent_b, Some(0.5));
    }

    #[test]
    fn default_multiplier_one() {
        let r = compute("2 4 6 8 10", 5, 1.0).unwrap();
        let b = &r.bands[0];
        // mean 6, pop var = (16+4+0+4+16)/5 = 8, sd = sqrt(8).
        assert_eq!(b.middle, 6.0);
        assert_eq!(b.upper, round6(6.0 + 8.0f64.sqrt()));
    }

    #[test]
    fn errors() {
        assert!(compute("", 5, 2.0).is_err());
        assert!(compute("1 2 abc", 5, 2.0).is_err());
        // too few values for the period.
        assert!(compute("1 2 3", 5, 2.0).is_err());
        // bad params.
        assert!(compute("1 2 3 4 5", 0, 2.0).is_err());
        assert!(compute("1 2 3 4 5", 5, -1.0).is_err());
    }
}
