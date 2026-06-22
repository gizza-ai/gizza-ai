//! gizza-ai/moving-average core — simple (SMA) and exponential (EMA) moving
//! averages over a numeric price/value series. Pure-Rust, dependency-free
//! besides serde for the output struct.
//!
//! The series is a list of numbers (separated by whitespace, commas, semicolons,
//! or newlines). `period` is the look-back window (>= 1). For each input point
//! the SMA at index i is the mean of the `period` values ending at i (null until
//! `period` points are available — the "warm-up" region). The EMA seeds the first
//! value at index `period-1` with the SMA over the first window, then applies the
//! recurrence `ema_i = price_i * k + ema_{i-1} * (1 - k)` with smoothing factor
//! `k = 2 / (period + 1)` — the standard finance convention.

use serde::Serialize;

/// Max series length we'll process, to bound work on hostile input.
pub const MAX_POINTS: usize = 100_000;
/// Max look-back window.
pub const MAX_PERIOD: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MovingAverages {
    /// Number of input data points.
    pub count: usize,
    /// The look-back window used.
    pub period: usize,
    /// EMA smoothing factor `2 / (period + 1)`.
    pub smoothing_factor: f64,
    /// Simple moving average, one entry per input point. `None` (JSON `null`)
    /// for the warm-up region before `period` points are available.
    pub sma: Vec<Option<f64>>,
    /// Exponential moving average, one entry per input point. `None` until the
    /// first value at index `period - 1`.
    pub ema: Vec<Option<f64>>,
    /// Weighted moving average (linear weights 1..period, newest weighted most),
    /// one entry per input point. `None` during the warm-up region.
    pub wma: Vec<Option<f64>>,
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// Parse whitespace/comma/semicolon-separated numbers.
fn parse_series(text: &str) -> Result<Vec<f64>, String> {
    let mut nums = Vec::new();
    for tok in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        let n: f64 = t
            .parse()
            .map_err(|_| format!("'{t}' is not a number"))?;
        if !n.is_finite() {
            return Err(format!("'{t}' is not a finite number"));
        }
        nums.push(n);
        if nums.len() > MAX_POINTS {
            return Err(format!("too many data points (max {MAX_POINTS})"));
        }
    }
    if nums.is_empty() {
        return Err(
            "no numbers found — provide a series separated by spaces, commas, or newlines".into(),
        );
    }
    Ok(nums)
}

/// Compute simple and exponential moving averages over `text` (a numeric series)
/// with look-back window `period`. `period` of 0 is treated as an error.
pub fn compute(text: &str, period: u32) -> Result<MovingAverages, String> {
    if period == 0 {
        return Err("period must be at least 1".into());
    }
    if period > MAX_PERIOD {
        return Err(format!("period too large (max {MAX_PERIOD})"));
    }
    let series = parse_series(text)?;
    let n = series.len();
    let p = period as usize;
    if p > n {
        return Err(format!(
            "period ({p}) is larger than the series length ({n}) — not enough data points"
        ));
    }

    // SMA via a running window sum.
    let mut sma: Vec<Option<f64>> = vec![None; n];
    let mut window_sum = 0.0_f64;
    for i in 0..n {
        window_sum += series[i];
        if i >= p {
            window_sum -= series[i - p];
        }
        if i + 1 >= p {
            sma[i] = Some(round6(window_sum / p as f64));
        }
    }

    // EMA seeded with the SMA over the first window.
    let k = 2.0 / (p as f64 + 1.0);
    let mut ema: Vec<Option<f64>> = vec![None; n];
    let seed_idx = p - 1;
    // The seed is the simple mean of the first `p` values.
    let first_window_mean: f64 = series[..p].iter().sum::<f64>() / p as f64;
    let mut prev = first_window_mean;
    ema[seed_idx] = Some(round6(prev));
    for i in (seed_idx + 1)..n {
        prev = series[i] * k + prev * (1.0 - k);
        ema[i] = Some(round6(prev));
    }

    // WMA: linear weights 1..=p, the most recent value weighted heaviest.
    // weight_sum = p*(p+1)/2.
    let weight_sum = (p * (p + 1) / 2) as f64;
    let mut wma: Vec<Option<f64>> = vec![None; n];
    for i in (p - 1)..n {
        let mut acc = 0.0_f64;
        for (w, j) in (1..=p).zip((i + 1 - p)..=i) {
            acc += series[j] * w as f64;
        }
        wma[i] = Some(round6(acc / weight_sum));
    }

    Ok(MovingAverages {
        count: n,
        period: p,
        smoothing_factor: round6(k),
        sma,
        ema,
        wma,
    })
}

/// Convenience wrapper returning the JSON string (used by the chat + web surfaces).
pub fn compute_json(text: &str, period: u32) -> Result<String, String> {
    let ma = compute(text, period)?;
    serde_json::to_string(&ma).map_err(|e| format!("serialization failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sma_basic_window_3() {
        let ma = compute("1, 2, 3, 4, 5", 3).unwrap();
        assert_eq!(ma.count, 5);
        assert_eq!(ma.period, 3);
        // Warm-up: first two are None.
        assert_eq!(ma.sma[0], None);
        assert_eq!(ma.sma[1], None);
        // (1+2+3)/3 = 2, (2+3+4)/3 = 3, (3+4+5)/3 = 4
        assert_eq!(ma.sma[2], Some(2.0));
        assert_eq!(ma.sma[3], Some(3.0));
        assert_eq!(ma.sma[4], Some(4.0));
    }

    #[test]
    fn ema_seeds_with_sma_then_recurs() {
        let ma = compute("1, 2, 3, 4, 5", 3).unwrap();
        // smoothing k = 2/(3+1) = 0.5
        assert_eq!(ma.smoothing_factor, 0.5);
        // warm-up before seed index 2
        assert_eq!(ma.ema[0], None);
        assert_eq!(ma.ema[1], None);
        // seed = mean(1,2,3) = 2
        assert_eq!(ma.ema[2], Some(2.0));
        // ema[3] = 4*0.5 + 2*0.5 = 3
        assert_eq!(ma.ema[3], Some(3.0));
        // ema[4] = 5*0.5 + 3*0.5 = 4
        assert_eq!(ma.ema[4], Some(4.0));
    }

    #[test]
    fn period_one_passes_values_through() {
        let ma = compute("10 20 30", 1).unwrap();
        assert_eq!(ma.sma, vec![Some(10.0), Some(20.0), Some(30.0)]);
        // k = 2/2 = 1 → ema tracks the price exactly
        assert_eq!(ma.smoothing_factor, 1.0);
        assert_eq!(ma.ema, vec![Some(10.0), Some(20.0), Some(30.0)]);
        // WMA with a single weight is also the value itself.
        assert_eq!(ma.wma, vec![Some(10.0), Some(20.0), Some(30.0)]);
    }

    #[test]
    fn wma_weights_recent_values_more() {
        let ma = compute("1, 2, 3, 4, 5", 3).unwrap();
        // warm-up
        assert_eq!(ma.wma[0], None);
        assert_eq!(ma.wma[1], None);
        // weights 1,2,3 over (1,2,3): (1*1 + 2*2 + 3*3)/6 = 14/6 = 2.333333
        assert_eq!(ma.wma[2], Some(2.333333));
        // over (2,3,4): (2 + 6 + 12)/6 = 20/6 = 3.333333
        assert_eq!(ma.wma[3], Some(3.333333));
        // over (3,4,5): (3 + 8 + 15)/6 = 26/6 = 4.333333
        assert_eq!(ma.wma[4], Some(4.333333));
    }

    #[test]
    fn parses_newline_and_semicolon_separators() {
        let ma = compute("1\n2;3\t4", 2).unwrap();
        assert_eq!(ma.count, 4);
        assert_eq!(ma.sma[1], Some(1.5));
    }

    #[test]
    fn rejects_period_zero() {
        let err = compute("1 2 3", 0).unwrap_err();
        assert!(err.contains("at least 1"), "got: {err}");
    }

    #[test]
    fn rejects_period_larger_than_series() {
        let err = compute("1 2", 5).unwrap_err();
        assert!(err.contains("larger than the series length"), "got: {err}");
    }

    #[test]
    fn rejects_non_numeric() {
        let err = compute("1 2 oops", 2).unwrap_err();
        assert!(err.contains("not a number"), "got: {err}");
    }

    #[test]
    fn rejects_empty() {
        let err = compute("   ", 2).unwrap_err();
        assert!(err.contains("no numbers found"), "got: {err}");
    }

    #[test]
    fn json_roundtrips() {
        let s = compute_json("1 2 3 4", 2).unwrap();
        assert!(s.contains("\"sma\""));
        assert!(s.contains("\"ema\""));
        assert!(s.contains("\"wma\""));
        assert!(s.contains("\"period\":2"));
    }
}
