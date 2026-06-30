//! gizza-ai/rsi-calculator core — the Relative Strength Index (RSI) over a
//! numeric price series. Pure-Rust, dependency-free besides serde for the
//! output struct.
//!
//! RSI is the classic momentum oscillator introduced by J. Welles Wilder. It
//! measures the speed and magnitude of recent price changes on a 0–100 scale:
//!   * compute the period-over-period price changes (deltas),
//!   * split each delta into a **gain** (positive change) and a **loss**
//!     (absolute value of a negative change),
//!   * seed the **average gain** and **average loss** with the simple mean of
//!     the first `period` gains/losses (Wilder's seeding convention),
//!   * advance them with Wilder's smoothing:
//!     `avg = (avg_prev * (period - 1) + current) / period`,
//!   * `RS = avgGain / avgLoss`, `RSI = 100 - 100 / (1 + RS)`.
//!
//! By convention RSI is `100` when there are no losses in the window and `0`
//! when there are no gains. The first RSI value appears at index `period` (it
//! needs `period` deltas, i.e. `period + 1` prices); earlier points are `None`
//! (JSON `null`) during the warm-up region.
//!
//! The price series is parsed exactly like the moving-average / MACD tools:
//! numbers separated by whitespace, commas, semicolons, or newlines, bounded by
//! `MAX_POINTS` to keep hostile input cheap.

use serde::Serialize;

/// Max series length we'll process, to bound work on hostile input.
pub const MAX_POINTS: usize = 100_000;
/// Max RSI look-back period.
pub const MAX_PERIOD: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Rsi {
    /// Number of input data points.
    pub count: usize,
    /// Look-back period used (default 14).
    pub period: usize,
    /// Overbought threshold used to classify the latest reading (default 70).
    pub overbought: f64,
    /// Oversold threshold used to classify the latest reading (default 30).
    pub oversold: f64,
    /// RSI value at each input point. `None` during the warm-up before
    /// `period` deltas are available (i.e. for the first `period` points).
    pub rsi: Vec<Option<f64>>,
    /// Wilder-smoothed average gain at each point (`None` during warm-up).
    pub avg_gain: Vec<Option<f64>>,
    /// Wilder-smoothed average loss at each point (`None` during warm-up).
    pub avg_loss: Vec<Option<f64>>,
    /// RSI value at the most recent point (`None` if still warming up).
    pub latest_rsi: Option<f64>,
    /// Classification of the latest RSI: `"overbought"` (>= overbought),
    /// `"oversold"` (<= oversold), `"neutral"`, or `"warming-up"` when there
    /// is not yet a value.
    pub latest_signal: String,
}

fn round6(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Parse whitespace/comma/semicolon/newline-separated numbers (same as the
/// moving-average / MACD tools).
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

/// Classify an RSI value against the overbought / oversold thresholds.
fn classify(rsi: f64, overbought: f64, oversold: f64) -> &'static str {
    if rsi >= overbought {
        "overbought"
    } else if rsi <= oversold {
        "oversold"
    } else {
        "neutral"
    }
}

/// Compute the RSI for `text` (a numeric price series) using Wilder's smoothing
/// over the given look-back `period`, classifying the latest reading against the
/// `overbought` / `oversold` thresholds.
///
/// Requires `period` in `1..=MAX_PERIOD` and enough data points (the first RSI
/// needs `period` deltas, i.e. `period + 1` prices). With fewer points the
/// function errors rather than returning an all-`null` series.
pub fn compute(text: &str, period: u32, overbought: f64, oversold: f64) -> Result<Rsi, String> {
    if period == 0 {
        return Err("the period must be at least 1".into());
    }
    if period > MAX_PERIOD {
        return Err(format!("period too large (max {MAX_PERIOD})"));
    }
    if !overbought.is_finite() || !oversold.is_finite() {
        return Err("the overbought and oversold thresholds must be finite numbers".into());
    }
    if !(0.0..=100.0).contains(&overbought) || !(0.0..=100.0).contains(&oversold) {
        return Err("the overbought and oversold thresholds must be between 0 and 100".into());
    }
    if oversold > overbought {
        return Err(format!(
            "the oversold threshold ({oversold}) must not exceed the overbought threshold ({overbought})"
        ));
    }
    let series = parse_series(text)?;
    let n = series.len();
    let p = period as usize;
    if p + 1 > n {
        return Err(format!(
            "the period ({p}) needs at least {} data points (period + 1) — only {n} provided",
            p + 1
        ));
    }

    // Per-price gains/losses (index 0 has no delta and stays 0).
    let mut gains = vec![0.0f64; n];
    let mut losses = vec![0.0f64; n];
    for i in 1..n {
        let delta = series[i] - series[i - 1];
        if delta > 0.0 {
            gains[i] = delta;
        } else {
            losses[i] = -delta;
        }
    }

    let mut rsi: Vec<Option<f64>> = vec![None; n];
    let mut avg_gain_out: Vec<Option<f64>> = vec![None; n];
    let mut avg_loss_out: Vec<Option<f64>> = vec![None; n];

    // Seed the averages with the simple mean of the first `period` deltas
    // (deltas live at indices 1..=p, so the first RSI lands at index p).
    let mut avg_gain = gains[1..=p].iter().sum::<f64>() / p as f64;
    let mut avg_loss = losses[1..=p].iter().sum::<f64>() / p as f64;
    let rsi_at = |g: f64, l: f64| -> f64 {
        if l == 0.0 {
            if g == 0.0 {
                50.0 // flat window — no gains and no losses
            } else {
                100.0
            }
        } else {
            100.0 - 100.0 / (1.0 + g / l)
        }
    };
    rsi[p] = Some(rsi_at(avg_gain, avg_loss));
    avg_gain_out[p] = Some(avg_gain);
    avg_loss_out[p] = Some(avg_loss);

    let pf = p as f64;
    for i in (p + 1)..n {
        avg_gain = (avg_gain * (pf - 1.0) + gains[i]) / pf;
        avg_loss = (avg_loss * (pf - 1.0) + losses[i]) / pf;
        rsi[i] = Some(rsi_at(avg_gain, avg_loss));
        avg_gain_out[i] = Some(avg_gain);
        avg_loss_out[i] = Some(avg_loss);
    }

    let round_vec =
        |v: &[Option<f64>]| -> Vec<Option<f64>> { v.iter().map(|x| x.map(round6)).collect() };
    let rsi = round_vec(&rsi);
    let latest_rsi = rsi.last().copied().flatten();
    let latest_signal = match latest_rsi {
        Some(v) => classify(v, overbought, oversold).to_string(),
        None => "warming-up".to_string(),
    };

    Ok(Rsi {
        count: n,
        period: p,
        overbought,
        oversold,
        rsi,
        avg_gain: round_vec(&avg_gain_out),
        avg_loss: round_vec(&avg_loss_out),
        latest_rsi,
        latest_signal,
    })
}

/// Convenience wrapper returning the JSON string (used by the chat + web surfaces).
pub fn compute_json(
    text: &str,
    period: u32,
    overbought: f64,
    oversold: f64,
) -> Result<String, String> {
    let r = compute(text, period, overbought, oversold)?;
    serde_json::to_string(&r).map_err(|e| format!("serialization failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hand-checkable alternating series 1,2,1,2,1,2 with period 2.
    // deltas:        +1, -1, +1, -1, +1     (indices 1..5)
    // seed (d1,d2):  avgGain = (1+0)/2 = 0.5, avgLoss = (0+1)/2 = 0.5 → RS 1   → RSI 50  @idx2
    // idx3 (+1):     avgGain = (0.5+1)/2 = 0.75, avgLoss = 0.25       → RS 3   → RSI 75
    // idx4 (-1):     avgGain = 0.375, avgLoss = (0.25+1)/2 = 0.625    → RS 0.6 → RSI 37.5
    // idx5 (+1):     avgGain = 0.6875, avgLoss = 0.3125              → RS 2.2 → RSI 68.75
    #[test]
    fn rsi_alternating_series_pinned() {
        let r = compute("1 2 1 2 1 2", 2, 70.0, 30.0).unwrap();
        assert_eq!(r.count, 6);
        assert_eq!(r.period, 2);
        assert_eq!(
            r.rsi,
            vec![None, None, Some(50.0), Some(75.0), Some(37.5), Some(68.75)]
        );
        assert_eq!(r.latest_rsi, Some(68.75));
        assert_eq!(r.latest_signal, "neutral");
    }

    #[test]
    fn rsi_all_gains_is_100() {
        let r = compute("1 2 3 4 5", 2, 70.0, 30.0).unwrap();
        assert_eq!(r.rsi, vec![None, None, Some(100.0), Some(100.0), Some(100.0)]);
        assert_eq!(r.latest_rsi, Some(100.0));
        assert_eq!(r.latest_signal, "overbought");
    }

    #[test]
    fn rsi_all_losses_is_0() {
        let r = compute("5 4 3 2 1", 2, 70.0, 30.0).unwrap();
        assert_eq!(r.rsi, vec![None, None, Some(0.0), Some(0.0), Some(0.0)]);
        assert_eq!(r.latest_rsi, Some(0.0));
        assert_eq!(r.latest_signal, "oversold");
    }

    // Wilder's classic 14-period worked example. The first RSI lands at index 14
    // and standard references put it in the low-70s for this rising series.
    #[test]
    fn rsi_wilder_default_period() {
        let prices = "44.34,44.09,44.15,43.61,44.33,44.83,45.10,45.42,45.84,46.08,45.89,46.03,45.61,46.28,46.28,46.00,46.03,46.41,46.22,45.64";
        let r = compute(prices, 14, 70.0, 30.0).unwrap();
        assert_eq!(r.count, 20);
        assert_eq!(r.period, 14);
        assert_eq!(r.rsi[13], None);
        let first = r.rsi[14].unwrap();
        assert!((70.0..71.0).contains(&first), "first RSI was {first}");
        let last = r.latest_rsi.unwrap();
        assert!((57.0..58.5).contains(&last), "latest RSI was {last}");
    }

    #[test]
    fn custom_thresholds_classify() {
        // RSI latest 68.75; with overbought lowered to 60 it reads overbought.
        let r = compute("1 2 1 2 1 2", 2, 60.0, 40.0).unwrap();
        assert_eq!(r.latest_rsi, Some(68.75));
        assert_eq!(r.latest_signal, "overbought");
        assert_eq!(r.overbought, 60.0);
        assert_eq!(r.oversold, 40.0);
    }

    #[test]
    fn parses_newline_and_semicolon_separators() {
        let r = compute("1\n2;1\t2 1 2", 2, 70.0, 30.0).unwrap();
        assert_eq!(r.count, 6);
    }

    #[test]
    fn rejects_zero_period() {
        let err = compute("1 2 3 4", 0, 70.0, 30.0).unwrap_err();
        assert!(err.contains("at least 1"), "got: {err}");
    }

    #[test]
    fn rejects_period_too_large() {
        let err = compute("1 2 3", 20001, 70.0, 30.0).unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn rejects_not_enough_points() {
        let err = compute("1 2 3", 14, 70.0, 30.0).unwrap_err();
        assert!(err.contains("needs at least"), "got: {err}");
    }

    #[test]
    fn rejects_thresholds_out_of_range() {
        let err = compute("1 2 3 4", 2, 120.0, 30.0).unwrap_err();
        assert!(err.contains("between 0 and 100"), "got: {err}");
    }

    #[test]
    fn rejects_oversold_above_overbought() {
        let err = compute("1 2 3 4", 2, 30.0, 70.0).unwrap_err();
        assert!(err.contains("must not exceed"), "got: {err}");
    }

    #[test]
    fn rejects_non_numeric() {
        let err = compute("1 2 oops 4 5", 2, 70.0, 30.0).unwrap_err();
        assert!(err.contains("not a number"), "got: {err}");
    }

    #[test]
    fn rejects_empty() {
        let err = compute("   ", 14, 70.0, 30.0).unwrap_err();
        assert!(err.contains("no numbers found"), "got: {err}");
    }

    #[test]
    fn json_has_all_fields() {
        let s = compute_json("1 2 1 2 1 2", 2, 70.0, 30.0).unwrap();
        for key in [
            "\"rsi\"",
            "\"avg_gain\"",
            "\"avg_loss\"",
            "\"period\":2",
            "\"overbought\":70.0",
            "\"oversold\":30.0",
            "\"latest_rsi\":68.75",
            "\"latest_signal\":\"neutral\"",
        ] {
            assert!(s.contains(key), "missing {key} in {s}");
        }
    }
}
