//! gizza-ai/macd-calculator core — Moving Average Convergence Divergence (MACD)
//! over a numeric price series. Pure-Rust, dependency-free besides serde for the
//! output struct.
//!
//! MACD is the standard momentum indicator built from three exponential moving
//! averages:
//!   * fast EMA (default period 12) and slow EMA (default period 26) of the price
//!     series,
//!   * the **MACD line** = fastEMA - slowEMA,
//!   * the **signal line** = EMA (default period 9) of the MACD line,
//!   * the **histogram** = MACD line - signal line.
//!
//! Each EMA is seeded with the simple mean of its first window (the same finance
//! convention used by the moving-average tool), then advanced with the recurrence
//! `ema_i = price_i * k + ema_{i-1} * (1 - k)`, smoothing factor `k = 2/(p+1)`.
//! Values are `None` (JSON `null`) during the warm-up region before enough points
//! are available — the MACD line starts once the slow EMA is available, and the
//! signal line starts once `signal` MACD values exist.
//!
//! The price series is parsed exactly like the moving-average tool: numbers
//! separated by whitespace, commas, semicolons, or newlines, bounded by
//! `MAX_POINTS` to keep hostile input cheap.

use serde::Serialize;

/// Max series length we'll process, to bound work on hostile input.
pub const MAX_POINTS: usize = 100_000;
/// Max period for any of the fast / slow / signal EMAs.
pub const MAX_PERIOD: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Macd {
    /// Number of input data points.
    pub count: usize,
    /// Fast EMA period (default 12).
    pub fast_period: usize,
    /// Slow EMA period (default 26).
    pub slow_period: usize,
    /// Signal EMA period (default 9).
    pub signal_period: usize,
    /// Fast EMA of the price series, one entry per input point. `None` during
    /// the warm-up before `fast_period` points are available.
    pub fast_ema: Vec<Option<f64>>,
    /// Slow EMA of the price series, one entry per input point. `None` during
    /// the warm-up before `slow_period` points are available.
    pub slow_ema: Vec<Option<f64>>,
    /// MACD line = fastEMA - slowEMA. `None` until the slow EMA is available.
    pub macd: Vec<Option<f64>>,
    /// Signal line = EMA(signal_period) of the MACD line. `None` until
    /// `signal_period` MACD values are available.
    pub signal: Vec<Option<f64>>,
    /// Histogram = MACD line - signal line. `None` until both are available.
    pub histogram: Vec<Option<f64>>,
    /// MACD line value at the most recent point (`None` if still warming up).
    pub latest_macd: Option<f64>,
    /// Signal line value at the most recent point (`None` if still warming up).
    pub latest_signal: Option<f64>,
    /// Histogram value at the most recent point (`None` if still warming up).
    pub latest_histogram: Option<f64>,
}

fn round6(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    // Normalise -0.0 → 0.0 so the histogram doesn't render "-0.0".
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Parse whitespace/comma/semicolon/newline-separated numbers (same as the
/// moving-average tool).
fn parse_series(text: &str) -> Result<Vec<f64>, String> {
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
        if nums.len() > MAX_POINTS {
            return Err(format!("too many data points (max {MAX_POINTS})"));
        }
    }
    if nums.is_empty() {
        return Err(
            "no numbers found — provide a price series separated by spaces, commas, or newlines"
                .into(),
        );
    }
    Ok(nums)
}

/// EMA over `series` with the given `period`, seeded with the simple mean of the
/// first window at index `period-1`, then advanced by the standard recurrence.
/// Returns one `Option<f64>` per input point (raw, un-rounded — callers round).
/// `period` is assumed `1..=series.len()`.
fn ema_series(series: &[f64], period: usize) -> Vec<Option<f64>> {
    let n = series.len();
    let mut out: Vec<Option<f64>> = vec![None; n];
    if period == 0 || period > n {
        return out;
    }
    let k = 2.0 / (period as f64 + 1.0);
    let seed_idx = period - 1;
    let mut prev = series[..period].iter().sum::<f64>() / period as f64;
    out[seed_idx] = Some(prev);
    for (i, &price) in series.iter().enumerate().skip(seed_idx + 1) {
        prev = price * k + prev * (1.0 - k);
        out[i] = Some(prev);
    }
    out
}

/// Compute the MACD line, signal line and histogram for `text` (a numeric price
/// series) using the given fast / slow / signal EMA periods.
///
/// Requires `fast < slow` (otherwise the MACD line is degenerate), all periods in
/// `1..=MAX_PERIOD`, and enough data points (the slow EMA needs `slow` points; the
/// signal line additionally needs `signal` MACD values, i.e. at least
/// `slow + signal - 1` points to fully warm up — fewer points simply leaves the
/// later lines `null`).
pub fn compute(text: &str, fast: u32, slow: u32, signal: u32) -> Result<Macd, String> {
    if fast == 0 || slow == 0 || signal == 0 {
        return Err("fast, slow, and signal periods must each be at least 1".into());
    }
    if fast > MAX_PERIOD || slow > MAX_PERIOD || signal > MAX_PERIOD {
        return Err(format!("periods too large (max {MAX_PERIOD})"));
    }
    if fast >= slow {
        return Err(format!(
            "the fast period ({fast}) must be smaller than the slow period ({slow})"
        ));
    }
    let series = parse_series(text)?;
    let n = series.len();
    let (fp, sp, sig_p) = (fast as usize, slow as usize, signal as usize);
    if sp > n {
        return Err(format!(
            "the slow period ({sp}) is larger than the series length ({n}) — not enough data points to compute MACD"
        ));
    }

    let fast_ema_raw = ema_series(&series, fp);
    let slow_ema_raw = ema_series(&series, sp);

    // MACD line = fast - slow, defined wherever the slow EMA is available
    // (the fast EMA is always available there, since fast < slow).
    let macd_raw: Vec<Option<f64>> = (0..n)
        .map(|i| match (fast_ema_raw[i], slow_ema_raw[i]) {
            (Some(f), Some(s)) => Some(f - s),
            _ => None,
        })
        .collect();

    // Signal = EMA(signal_period) of the contiguous (non-null) MACD values.
    let first_macd = macd_raw.iter().position(|m| m.is_some());
    let mut signal_raw: Vec<Option<f64>> = vec![None; n];
    if let Some(start) = first_macd {
        let macd_vals: Vec<f64> = macd_raw[start..].iter().filter_map(|m| *m).collect();
        let sig_ema = ema_series(&macd_vals, sig_p);
        for (j, v) in sig_ema.iter().enumerate() {
            if let Some(v) = v {
                signal_raw[start + j] = Some(*v);
            }
        }
    }

    // Histogram = MACD - signal, where both exist.
    let histogram_raw: Vec<Option<f64>> = (0..n)
        .map(|i| match (macd_raw[i], signal_raw[i]) {
            (Some(m), Some(s)) => Some(m - s),
            _ => None,
        })
        .collect();

    let round_vec = |v: &[Option<f64>]| -> Vec<Option<f64>> {
        v.iter().map(|x| x.map(round6)).collect()
    };

    let macd = round_vec(&macd_raw);
    let signal_v = round_vec(&signal_raw);
    let histogram = round_vec(&histogram_raw);

    Ok(Macd {
        count: n,
        fast_period: fp,
        slow_period: sp,
        signal_period: sig_p,
        fast_ema: round_vec(&fast_ema_raw),
        slow_ema: round_vec(&slow_ema_raw),
        latest_macd: macd.last().copied().flatten(),
        latest_signal: signal_v.last().copied().flatten(),
        latest_histogram: histogram.last().copied().flatten(),
        macd,
        signal: signal_v,
        histogram,
    })
}

/// Convenience wrapper returning the JSON string (used by the chat + web surfaces).
pub fn compute_json(text: &str, fast: u32, slow: u32, signal: u32) -> Result<String, String> {
    let m = compute(text, fast, slow, signal)?;
    serde_json::to_string(&m).map_err(|e| format!("serialization failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Small, fully hand-checkable series: prices 1..6, fast=2, slow=4, signal=2.
    // fast EMA: [_, 1.5, 2.5, 3.5, 4.5, 5.5]
    // slow EMA: [_, _, _, 2.5, 3.5, 4.5]
    // MACD    : [_, _, _, 1.0, 1.0, 1.0]   (first MACD at index 3)
    // signal  : EMA2 of [1,1,1] seeded at index 4 → [_, _, _, _, 1.0, 1.0]
    // hist    : [_, _, _, _, 0.0, 0.0]
    #[test]
    fn macd_small_series_pinned() {
        let m = compute("1 2 3 4 5 6", 2, 4, 2).unwrap();
        assert_eq!(m.count, 6);
        assert_eq!(m.fast_period, 2);
        assert_eq!(m.slow_period, 4);
        assert_eq!(m.signal_period, 2);
        assert_eq!(m.fast_ema, vec![None, Some(1.5), Some(2.5), Some(3.5), Some(4.5), Some(5.5)]);
        assert_eq!(m.slow_ema, vec![None, None, None, Some(2.5), Some(3.5), Some(4.5)]);
        assert_eq!(m.macd, vec![None, None, None, Some(1.0), Some(1.0), Some(1.0)]);
        assert_eq!(m.signal, vec![None, None, None, None, Some(1.0), Some(1.0)]);
        assert_eq!(m.histogram, vec![None, None, None, None, Some(0.0), Some(0.0)]);
        assert_eq!(m.latest_macd, Some(1.0));
        assert_eq!(m.latest_signal, Some(1.0));
        assert_eq!(m.latest_histogram, Some(0.0));
    }

    // Default periods 12/26/9 over a 40-point linear ramp 1..40. The slow EMA
    // first appears at index 25, leaving 15 MACD values so the signal warms up.
    // On a constant-slope ramp every MACD value converges to fast-slow lag = 7.0,
    // so the signal and histogram settle to 7.0 and 0.0 at the tail.
    #[test]
    fn macd_default_periods_ramp() {
        let series: String = (1..=40)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let m = compute(&series, 12, 26, 9).unwrap();
        assert_eq!(m.count, 40);
        assert_eq!(m.fast_period, 12);
        assert_eq!(m.slow_period, 26);
        assert_eq!(m.signal_period, 9);
        // First MACD value is at index slow_period-1 = 25.
        assert_eq!(m.macd[24], None);
        assert_eq!(m.macd[25], Some(7.0));
        assert_eq!(m.latest_macd, Some(7.0));
        assert_eq!(m.latest_signal, Some(7.0));
        assert_eq!(m.latest_histogram, Some(0.0));
    }

    // Real-ish 30-point series from a standard MACD worked example, fast=12,
    // slow=26, signal=9. Only 5 MACD values exist (< 9) so the signal never
    // warms up — assert the MACD line tail and that signal stays null.
    #[test]
    fn macd_reference_series_signal_warmup() {
        let prices = "22.27,22.19,22.08,22.17,22.18,22.13,22.23,22.43,22.24,22.29,22.15,22.39,22.38,22.61,23.36,24.05,23.75,23.83,23.95,23.63,23.82,23.87,23.65,23.19,23.10,23.33,22.68,23.10,22.40,22.17";
        let m = compute(prices, 12, 26, 9).unwrap();
        assert_eq!(m.count, 30);
        // MACD line first appears at index 25.
        assert_eq!(m.macd[24], None);
        assert_eq!(m.macd[25], Some(0.459715));
        assert_eq!(m.latest_macd, Some(0.149504));
        // Only 5 MACD values < signal period 9 → signal & histogram all null.
        assert!(m.signal.iter().all(|s| s.is_none()));
        assert_eq!(m.latest_signal, None);
        assert_eq!(m.latest_histogram, None);
    }

    #[test]
    fn parses_newline_and_semicolon_separators() {
        let m = compute("1\n2;3\t4 5 6", 2, 4, 2).unwrap();
        assert_eq!(m.count, 6);
    }

    #[test]
    fn rejects_zero_period() {
        let err = compute("1 2 3 4", 0, 4, 2).unwrap_err();
        assert!(err.contains("at least 1"), "got: {err}");
    }

    #[test]
    fn rejects_fast_ge_slow() {
        let err = compute("1 2 3 4 5", 5, 3, 2).unwrap_err();
        assert!(err.contains("smaller than the slow period"), "got: {err}");
    }

    #[test]
    fn rejects_slow_larger_than_series() {
        let err = compute("1 2 3", 2, 26, 9).unwrap_err();
        assert!(err.contains("larger than the series length"), "got: {err}");
    }

    #[test]
    fn rejects_period_too_large() {
        let err = compute("1 2 3", 1, 20001, 9).unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn rejects_non_numeric() {
        let err = compute("1 2 oops 4 5", 2, 4, 2).unwrap_err();
        assert!(err.contains("not a number"), "got: {err}");
    }

    #[test]
    fn rejects_empty() {
        let err = compute("   ", 12, 26, 9).unwrap_err();
        assert!(err.contains("no numbers found"), "got: {err}");
    }

    #[test]
    fn json_has_all_fields() {
        let s = compute_json("1 2 3 4 5 6", 2, 4, 2).unwrap();
        for key in [
            "\"macd\"",
            "\"signal\"",
            "\"histogram\"",
            "\"fast_ema\"",
            "\"slow_ema\"",
            "\"fast_period\":2",
            "\"slow_period\":4",
            "\"signal_period\":2",
            "\"latest_histogram\":0.0",
        ] {
            assert!(s.contains(key), "missing {key} in {s}");
        }
    }
}
