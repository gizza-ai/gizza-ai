//! gizza-ai/returns-risk-analyzer core — performance & risk metrics for a series
//! of periodic investment returns: annualized (compound) return, annualized
//! volatility, downside deviation, and the Sharpe & Sortino ratios, from a
//! configurable periods-per-year, risk-free rate and Sortino target.
//!
//! Conventions (fixed, documented on the page):
//! - Volatility uses the SAMPLE standard deviation (÷ n−1).
//! - Downside deviation divides by n (population) using all observations.
//! - Annualized return is GEOMETRIC (compound): (∏(1+r))^(ppy/n) − 1.
//! - Sharpe uses the risk-free rate; Sortino uses the target return as the
//!   minimum acceptable return (MAR). Both ratios are annualized by √ppy.
//! Pure-Rust, only serde for the output struct. Not financial advice.

use serde::Serialize;

/// The computed metrics. Rates/returns are decimals (0.012 = 1.2%); the page
/// renders them as percentages.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Analysis {
    /// Number of return observations.
    pub count: usize,
    /// Periods per year used for annualization (e.g. 252 daily).
    pub periods_per_year: f64,
    /// Frequency label for `periods_per_year` (daily/weekly/monthly/…/custom).
    pub frequency: String,
    /// Arithmetic mean of the per-period returns (decimal).
    pub period_mean: f64,
    /// Compound (geometric) annualized return (decimal).
    pub annualized_return: f64,
    /// Total compounded return over the whole series (decimal).
    pub cumulative_return: f64,
    /// Annualized standard deviation of returns — sample stdev × √ppy (decimal).
    pub annualized_volatility: f64,
    /// Annualized downside deviation below the target — population divisor ÷ n
    /// × √ppy (decimal).
    pub downside_deviation: f64,
    /// Annualized Sharpe ratio; `None` when volatility is zero (undefined).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharpe: Option<f64>,
    /// Annualized Sortino ratio; `None` when there is no downside vs the target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sortino: Option<f64>,
    /// Largest peak-to-trough drop of the compounded equity curve (decimal,
    /// ≤ 0; e.g. −0.15 = a 15% drawdown).
    pub max_drawdown: f64,
    /// Calmar ratio = annualized return ÷ |max drawdown|; `None` when there was
    /// no drawdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calmar: Option<f64>,
    /// Share of periods with a return above zero (decimal 0..=1).
    pub positive_period_ratio: f64,
    /// Best single-period return (decimal).
    pub best_period: f64,
    /// Worst single-period return (decimal).
    pub worst_period: f64,
    /// A plain-language note on the conventions used. Not financial advice.
    pub note: String,
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// Human label for a periods-per-year value.
fn frequency_label(ppy: f64) -> &'static str {
    match ppy as i64 {
        252 => "daily",
        52 => "weekly",
        26 => "biweekly",
        12 => "monthly",
        4 => "quarterly",
        1 => "annual",
        _ => "custom",
    }
}

/// Parse one return token: a plain decimal (`0.012`) or a percent (`1.2%`,
/// which becomes `0.012`). Leading `+` and surrounding whitespace are allowed.
fn parse_return(tok: &str) -> Result<f64, String> {
    let t = tok.trim();
    let (body, is_pct) = match t.strip_suffix('%') {
        Some(rest) => (rest.trim_end(), true),
        None => (t, false),
    };
    let body = body.strip_prefix('+').unwrap_or(body);
    let n: f64 = body.parse().map_err(|_| format!("'{t}' is not a number"))?;
    if !n.is_finite() {
        return Err(format!("'{t}' is not a finite number"));
    }
    Ok(if is_pct { n / 100.0 } else { n })
}

/// Split a returns series on newlines, commas, semicolons, tabs and whitespace,
/// optionally dropping a first header line, and parse each token as a return.
fn parse_returns(text: &str, has_header: bool) -> Result<Vec<f64>, String> {
    let text = if has_header {
        // Drop everything up to and including the first newline.
        match text.find('\n') {
            Some(i) => &text[i + 1..],
            None => "",
        }
    } else {
        text
    };
    let mut nums = Vec::new();
    for tok in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        if tok.trim().is_empty() {
            continue;
        }
        nums.push(parse_return(tok)?);
    }
    Ok(nums)
}

/// Compute the performance & risk metrics from a text returns series.
///
/// * `periods_per_year` — annualization factor (e.g. 252 daily); must be > 0.
/// * `risk_free_pct` — annual risk-free rate as a PERCENT (2.0 = 2%); Sharpe only.
/// * `target_pct` — Sortino minimum acceptable return as an annual PERCENT.
/// * `has_header` — drop the first line before parsing (a column label).
pub fn analyze(
    text: &str,
    periods_per_year: f64,
    risk_free_pct: f64,
    target_pct: f64,
    has_header: bool,
) -> Result<Analysis, String> {
    if !(periods_per_year.is_finite() && periods_per_year > 0.0) {
        return Err("periods_per_year must be a positive number".into());
    }
    let returns = parse_returns(text, has_header)?;
    let n = returns.len();
    if n < 2 {
        return Err(format!(
            "need at least 2 returns to compute volatility, got {n} — enter one return per line or comma-separated (0.012 or 1.2%)"
        ));
    }
    let ppy = periods_per_year;
    let rf_period = (risk_free_pct / 100.0) / ppy;
    let target_period = (target_pct / 100.0) / ppy;

    let sum: f64 = returns.iter().sum();
    let mean = sum / n as f64;

    // Sample standard deviation (÷ n−1) → annualized volatility.
    let ss: f64 = returns.iter().map(|r| (r - mean) * (r - mean)).sum();
    let period_std = (ss / (n as f64 - 1.0)).sqrt();
    let annualized_volatility = period_std * ppy.sqrt();

    // Downside deviation vs the target (÷ n, all observations) → annualized.
    let down_ss: f64 = returns
        .iter()
        .map(|r| {
            let d = (r - target_period).min(0.0);
            d * d
        })
        .sum();
    let dd_period = (down_ss / n as f64).sqrt();
    let downside_deviation = dd_period * ppy.sqrt();

    // Geometric (compound) annualized return. A period of −100% or worse wipes
    // the series out; report a total loss rather than a NaN from a negative base.
    let growth: f64 = returns.iter().map(|r| 1.0 + r).product();
    let (cumulative_return, annualized_return) = if growth <= 0.0 {
        (-1.0, -1.0)
    } else {
        (growth - 1.0, growth.powf(ppy / n as f64) - 1.0)
    };

    // Annualized Sharpe (arithmetic excess mean ÷ sample stdev, × √ppy).
    let sharpe = if period_std > 0.0 {
        Some(round6((mean - rf_period) / period_std * ppy.sqrt()))
    } else {
        None
    };
    // Annualized Sortino (excess over target ÷ downside deviation, × √ppy).
    let sortino = if dd_period > 0.0 {
        Some(round6((mean - target_period) / dd_period * ppy.sqrt()))
    } else {
        None
    };

    // Max drawdown from the compounded equity curve (starts at 1.0).
    let mut equity = 1.0f64;
    let mut peak = 1.0f64;
    let mut max_drawdown = 0.0f64;
    for r in &returns {
        equity *= 1.0 + r;
        if equity > peak {
            peak = equity;
        }
        if peak > 0.0 {
            let dd = equity / peak - 1.0;
            if dd < max_drawdown {
                max_drawdown = dd;
            }
        }
    }
    // Calmar = annualized return ÷ |max drawdown|.
    let calmar = if max_drawdown < 0.0 {
        Some(round6(annualized_return / max_drawdown.abs()))
    } else {
        None
    };
    let positive_period_ratio =
        returns.iter().filter(|r| **r > 0.0).count() as f64 / n as f64;

    let best_period = returns.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let worst_period = returns.iter().cloned().fold(f64::INFINITY, f64::min);

    Ok(Analysis {
        count: n,
        periods_per_year: ppy,
        frequency: frequency_label(ppy).to_string(),
        period_mean: round6(mean),
        annualized_return: round6(annualized_return),
        cumulative_return: round6(cumulative_return),
        annualized_volatility: round6(annualized_volatility),
        downside_deviation: round6(downside_deviation),
        sharpe,
        sortino,
        max_drawdown: round6(max_drawdown),
        calmar,
        positive_period_ratio: round6(positive_period_ratio),
        best_period: round6(best_period),
        worst_period: round6(worst_period),
        note: "Volatility uses the sample standard deviation (÷ n−1); downside deviation divides by n. Annualized return is geometric (compound). Sharpe uses the risk-free rate; Sortino uses the target return as the minimum acceptable return. Educational only — not financial advice.".to_string(),
    })
}

fn pct(v: f64) -> String {
    format!("{:.4}%", v * 100.0)
}

fn ratio(v: Option<f64>, undefined_reason: &str) -> String {
    match v {
        Some(x) => format!("{x:.4}"),
        None => format!("undefined ({undefined_reason})"),
    }
}

/// Human-readable summary (used by the page).
pub fn summary(
    text: &str,
    periods_per_year: f64,
    risk_free_pct: f64,
    target_pct: f64,
    has_header: bool,
) -> Result<String, String> {
    let a = analyze(text, periods_per_year, risk_free_pct, target_pct, has_header)?;
    Ok(format!(
        "count: {} returns\nfrequency: {} ({} periods/year)\nrisk-free rate: {:.4}% / yr\nSortino target: {:.4}% / yr\n\nperiod mean: {}\npositive periods: {:.1}%\nbest period: {}\nworst period: {}\ncumulative return: {}\nannualized return: {}\nannualized volatility: {}\ndownside deviation: {}\nmax drawdown: {}\nSharpe ratio: {}\nSortino ratio: {}\nCalmar ratio: {}\n\n{}",
        a.count,
        a.frequency,
        a.periods_per_year,
        risk_free_pct,
        target_pct,
        pct(a.period_mean),
        a.positive_period_ratio * 100.0,
        pct(a.best_period),
        pct(a.worst_period),
        pct(a.cumulative_return),
        pct(a.annualized_return),
        pct(a.annualized_volatility),
        pct(a.downside_deviation),
        pct(a.max_drawdown),
        ratio(a.sharpe, "zero volatility"),
        ratio(a.sortino, "no downside vs target"),
        ratio(a.calmar, "no drawdown"),
        a.note,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-4, "expected ~{b}, got {a}");
    }

    #[test]
    fn monthly_happy_path() {
        // Three monthly returns: 1%, -0.5%, 2%.
        let a = analyze("1%, -0.5%, 2%", 12.0, 0.0, 0.0, false).unwrap();
        assert_eq!(a.count, 3);
        assert_eq!(a.frequency, "monthly");
        approx(a.period_mean, (0.01 - 0.005 + 0.02) / 3.0);
        approx(a.best_period, 0.02);
        approx(a.worst_period, -0.005);
        approx(a.cumulative_return, 1.01 * 0.995 * 1.02 - 1.0);
        approx(
            a.annualized_return,
            (1.01f64 * 0.995 * 1.02).powf(12.0 / 3.0) - 1.0,
        );
    }

    #[test]
    fn decimal_and_percent_inputs_agree() {
        let p = analyze("1%\n-0.5%\n2%", 12.0, 0.0, 0.0, false).unwrap();
        let d = analyze("0.01\n-0.005\n0.02", 12.0, 0.0, 0.0, false).unwrap();
        assert_eq!(p.annualized_return, d.annualized_return);
        assert_eq!(p.annualized_volatility, d.annualized_volatility);
    }

    #[test]
    fn sharpe_and_volatility_known_values() {
        // Returns 0.01 and 0.03: mean 0.02, sample stdev = sqrt(2*(.01)^2/1)=0.01414214
        let a = analyze("0.01, 0.03", 12.0, 0.0, 0.0, false).unwrap();
        let period_std = (2.0f64 * 0.01 * 0.01).sqrt();
        approx(a.annualized_volatility, period_std * 12f64.sqrt());
        approx(a.sharpe.unwrap(), 0.02 / period_std * 12f64.sqrt());
    }

    #[test]
    fn risk_free_lowers_sharpe() {
        let base = analyze("0.01, 0.03", 12.0, 0.0, 0.0, false).unwrap();
        let with_rf = analyze("0.01, 0.03", 12.0, 12.0, 0.0, false).unwrap();
        // rf 12%/yr = 1%/month, subtracts from the 2% mean.
        assert!(with_rf.sharpe.unwrap() < base.sharpe.unwrap());
    }

    #[test]
    fn no_downside_gives_undefined_sortino() {
        // All returns >= target (0) → no downside → Sortino undefined.
        let a = analyze("0.01, 0.02, 0.03", 12.0, 0.0, 0.0, false).unwrap();
        assert_eq!(a.downside_deviation, 0.0);
        assert!(a.sortino.is_none());
        assert!(a.sharpe.is_some());
    }

    #[test]
    fn zero_volatility_gives_undefined_sharpe() {
        let a = analyze("0.01, 0.01, 0.01", 12.0, 0.0, 0.0, false).unwrap();
        assert_eq!(a.annualized_volatility, 0.0);
        assert!(a.sharpe.is_none());
    }

    #[test]
    fn header_is_skipped() {
        let a = analyze("return\n0.01\n0.02", 12.0, 0.0, 0.0, true).unwrap();
        assert_eq!(a.count, 2);
    }

    #[test]
    fn max_drawdown_and_positive_ratio() {
        // +10%, -20%, +5%: equity 1.1, 0.88, 0.924; peak 1.1 → trough 0.88.
        let a = analyze("0.10, -0.20, 0.05", 12.0, 0.0, 0.0, false).unwrap();
        approx(a.max_drawdown, 0.88 / 1.1 - 1.0); // = -0.2
        approx(a.positive_period_ratio, 2.0 / 3.0);
        assert!(a.calmar.is_some());
    }

    #[test]
    fn no_drawdown_gives_undefined_calmar() {
        let a = analyze("0.01, 0.02, 0.03", 12.0, 0.0, 0.0, false).unwrap();
        assert_eq!(a.max_drawdown, 0.0);
        assert!(a.calmar.is_none());
    }

    #[test]
    fn total_loss_is_bounded() {
        let a = analyze("-1.0, 0.5", 12.0, 0.0, 0.0, false).unwrap();
        assert_eq!(a.annualized_return, -1.0);
        assert_eq!(a.cumulative_return, -1.0);
    }

    #[test]
    fn errors() {
        assert!(analyze("", 12.0, 0.0, 0.0, false).is_err());
        assert!(analyze("0.01", 12.0, 0.0, 0.0, false).is_err()); // n<2
        assert!(analyze("0.01, abc", 12.0, 0.0, 0.0, false).is_err());
        assert!(analyze("0.01, 0.02", 0.0, 0.0, 0.0, false).is_err()); // ppy<=0
    }
}
