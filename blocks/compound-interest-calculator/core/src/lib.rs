//! compound-interest-calculator core — pure future-value math, shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Computes the future value of an initial principal that grows with compound
//! interest, plus the future value of optional regular contributions (an
//! annuity). Any combination of compounding frequency and contribution
//! frequency is supported by first reducing the nominal annual rate to an
//! effective annual rate (EAR):
//!
//! - `EAR = (1 + r/n)^n - 1`   for periodic compounding (`n` periods/yr), or
//! - `EAR = e^r - 1`            for continuous compounding, where `r = rate/100`.
//!
//! Principal grows by `(1 + EAR)^t` over `t = years + months/12`. Contributions
//! use the annuity future-value at an equivalent contribution-period rate
//! `i_c = (1 + EAR)^(1/m) - 1` (`m` = contribution periods/yr), multiplied by
//! `(1 + i_c)` when deposits are made at the start of each period (annuity-due).
//! A zero rate falls back to `PMT * n`.
//!
//! All math is `f64`; money is rounded to 2 decimals and rates to 4 decimals.

use serde::Serialize;

/// One row of the year-by-year growth schedule.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YearRow {
    /// Elapsed years at this row (whole years, plus a final fractional row when
    /// the term includes extra months).
    pub year: f64,
    /// Account balance at the end of this period (money, 2 dp).
    pub balance: f64,
    /// Total contributed so far, excluding the initial principal (money, 2 dp).
    pub contributions_to_date: f64,
    /// Interest earned so far (money, 2 dp).
    pub interest_to_date: f64,
}

/// Structured compound-interest result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompoundResult {
    /// Value of the account at the end of the term (money, 2 dp).
    pub future_value: f64,
    /// The initial principal echoed back (money, 2 dp).
    pub principal: f64,
    /// Sum of all regular contributions over the term, excluding principal
    /// (money, 2 dp; negative when contributions are net withdrawals).
    pub total_contributions: f64,
    /// Future value minus principal minus total contributions (money, 2 dp).
    pub total_interest: f64,
    /// Effective annual rate implied by the nominal rate + compounding, as a
    /// percentage (4 dp).
    pub effective_annual_rate: f64,
    /// Years for the principal alone to double at the effective rate, or `null`
    /// when the rate is zero or negative (4 dp).
    pub years_to_double: Option<f64>,
    /// Total term in years (`years + months/12`).
    pub years: f64,
    /// Per-year growth schedule (capped — see [`MAX_SCHEDULE_ROWS`]).
    pub schedule: Vec<YearRow>,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// Cap on the number of schedule rows returned, so a 1000-year term can't emit
/// a huge JSON blob. The final term boundary is always the last row.
pub const MAX_SCHEDULE_ROWS: usize = 121;

/// Hard cap on the term to keep the closed-form powers well-behaved.
pub const MAX_YEARS: f64 = 1000.0;

/// All optional inputs. Each field is `None` when unset; [`compute`] applies the
/// documented default for any `None`, so every surface (chat, CLI, page) funnels
/// through the same defaults + validation.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Initial principal / starting balance. Default 1000.
    pub principal: Option<f64>,
    /// Nominal annual interest rate as a percent (e.g. 5 for 5%). Default 5.
    pub annual_rate: Option<f64>,
    /// Whole/partial years in the term. Default 10.
    pub years: Option<f64>,
    /// Additional months added to `years`. Default 0.
    pub months: Option<f64>,
    /// Compounding frequency keyword. Default "monthly".
    pub compounding: Option<String>,
    /// Regular contribution amount per contribution period (negative =
    /// withdrawal). Default 0.
    pub contribution: Option<f64>,
    /// Contribution frequency keyword. Default "monthly".
    pub contribution_frequency: Option<String>,
    /// "end" (ordinary annuity) or "start" (annuity-due). Default "end".
    pub contribution_timing: Option<String>,
}

fn r2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn r4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// Format a money value with thousands separators and two decimals for the
/// summary line (e.g. `12345.6` → `12,345.60`).
fn money(v: f64) -> String {
    let v = r2(v);
    let neg = v < 0.0;
    let cents = (v.abs() * 100.0).round() as u128;
    let whole = cents / 100;
    let frac = cents % 100;
    let digits = whole.to_string();
    let mut grouped = String::new();
    let bytes = digits.as_bytes();
    for (idx, ch) in bytes.iter().enumerate() {
        if idx > 0 && (bytes.len() - idx) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*ch as char);
    }
    format!("{}{}.{:02}", if neg { "-" } else { "" }, grouped, frac)
}

/// Compounding periods per year for a keyword; `None` = continuous.
fn compounding_periods(kw: &str) -> Result<Option<f64>, String> {
    let n = match normalize(kw).as_str() {
        "annually" | "yearly" | "annual" => 1.0,
        "semiannually" | "semiannual" | "halfyearly" | "biannually" => 2.0,
        "quarterly" => 4.0,
        "monthly" => 12.0,
        "weekly" => 52.0,
        "daily" => 365.0,
        "continuously" | "continuous" => return Ok(None),
        other => {
            return Err(format!(
                "unknown compounding '{other}'. Supported: annually, semiannually, \
                 quarterly, monthly, weekly, daily, continuously"
            ))
        }
    };
    Ok(Some(n))
}

/// Contribution periods per year for a keyword.
fn contribution_periods(kw: &str) -> Result<f64, String> {
    let m = match normalize(kw).as_str() {
        "annually" | "yearly" | "annual" => 1.0,
        "semiannually" | "semiannual" | "halfyearly" | "biannually" => 2.0,
        "quarterly" => 4.0,
        "monthly" => 12.0,
        "biweekly" | "fortnightly" => 26.0,
        "weekly" => 52.0,
        other => {
            return Err(format!(
                "unknown contribution_frequency '{other}'. Supported: annually, \
                 semiannually, quarterly, monthly, biweekly, weekly"
            ))
        }
    };
    Ok(m)
}

/// Lowercase and strip spaces/hyphens/underscores so "Semi-Annually",
/// "semi annually" and "semiannually" all normalize the same.
fn normalize(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .collect()
}

/// Future value at `t` years for the given effective annual rate, contribution
/// per period `pmt`, `m` contribution periods/yr, and start-of-period timing.
fn fv_at(principal: f64, ear: f64, pmt: f64, m: f64, due: bool, t: f64) -> f64 {
    let growth = (1.0 + ear).powf(t);
    let fv_principal = principal * growth;
    let fv_contrib = if pmt == 0.0 {
        0.0
    } else {
        let n = m * t; // number of contribution periods elapsed
        let i_c = (1.0 + ear).powf(1.0 / m) - 1.0;
        let annuity = if i_c.abs() < 1e-12 {
            n
        } else {
            let base = ((1.0 + i_c).powf(n) - 1.0) / i_c;
            if due {
                base * (1.0 + i_c)
            } else {
                base
            }
        };
        pmt * annuity
    };
    fv_principal + fv_contrib
}

/// Compute the compound-interest result from the supplied inputs, applying
/// defaults for any unset field. Errors on non-finite numbers, an out-of-range
/// term, or an unknown frequency keyword.
pub fn compute(i: &Inputs) -> Result<CompoundResult, String> {
    let principal = i.principal.unwrap_or(1000.0);
    let rate_pct = i.annual_rate.unwrap_or(5.0);
    let years = i.years.unwrap_or(10.0);
    let months = i.months.unwrap_or(0.0);
    let pmt = i.contribution.unwrap_or(0.0);
    let compounding = i.compounding.clone().unwrap_or_else(|| "monthly".into());
    let contrib_freq = i
        .contribution_frequency
        .clone()
        .unwrap_or_else(|| "monthly".into());
    let timing = i.contribution_timing.clone().unwrap_or_else(|| "end".into());

    require_finite("principal", principal)?;
    require_finite("annual_rate", rate_pct)?;
    require_finite("years", years)?;
    require_finite("months", months)?;
    require_finite("contribution", pmt)?;
    if years < 0.0 || months < 0.0 {
        return Err("years and months must not be negative".into());
    }

    let t = years + months / 12.0;
    if t <= 0.0 {
        return Err("term must be greater than zero (set years and/or months)".into());
    }
    if t > MAX_YEARS {
        return Err(format!("term must be at most {MAX_YEARS} years"));
    }

    let due = match normalize(&timing).as_str() {
        "end" | "ordinary" => false,
        "start" | "beginning" | "due" => true,
        other => {
            return Err(format!(
                "unknown contribution_timing '{other}'. Supported: end, start"
            ))
        }
    };

    let r = rate_pct / 100.0;
    let ear = match compounding_periods(&compounding)? {
        Some(n) => (1.0 + r / n).powf(n) - 1.0,
        None => r.exp() - 1.0,
    };
    if !ear.is_finite() {
        return Err("the resulting effective rate is not a finite number".into());
    }
    let m = contribution_periods(&contrib_freq)?;

    let future_value = fv_at(principal, ear, pmt, m, due, t);
    if !future_value.is_finite() {
        return Err("the resulting future value overflowed — reduce the term or rate".into());
    }
    let total_contributions = pmt * m * t;
    let total_interest = future_value - principal - total_contributions;

    // Years for the principal alone to double at the effective rate.
    let years_to_double = if ear > 0.0 {
        Some(r4((2.0_f64).ln() / (1.0 + ear).ln()))
    } else {
        None
    };

    // Year-by-year schedule at each whole-year boundary, plus a final row at the
    // exact term when it has extra months. Capped at MAX_SCHEDULE_ROWS.
    let mut schedule = Vec::new();
    let whole_years = t.floor() as i64;
    let mut y = 1_i64;
    while y <= whole_years && schedule.len() + 1 < MAX_SCHEDULE_ROWS {
        push_row(&mut schedule, principal, ear, pmt, m, due, y as f64);
        y += 1;
    }
    // Always include the final term boundary (fractional or capped).
    let last_recorded = schedule.last().map(|r| r.year).unwrap_or(0.0);
    if (t - last_recorded).abs() > 1e-9 {
        push_row(&mut schedule, principal, ear, pmt, m, due, t);
    }

    let summary = format!(
        "{} at {}% compounded {} over {} grows to {} ({} contributed, {} interest)",
        money(principal),
        trim(rate_pct),
        normalize(&compounding),
        fmt_term(years, months),
        money(future_value),
        money(total_contributions),
        money(total_interest),
    );

    Ok(CompoundResult {
        future_value: r2(future_value),
        principal: r2(principal),
        total_contributions: r2(total_contributions),
        total_interest: r2(total_interest),
        effective_annual_rate: r4(ear * 100.0),
        years_to_double,
        years: r4(t),
        schedule,
        summary,
    })
}

fn push_row(out: &mut Vec<YearRow>, principal: f64, ear: f64, pmt: f64, m: f64, due: bool, t: f64) {
    let balance = fv_at(principal, ear, pmt, m, due, t);
    let contributed = pmt * m * t;
    let interest = balance - principal - contributed;
    out.push(YearRow {
        year: r4(t),
        balance: r2(balance),
        contributions_to_date: r2(contributed),
        interest_to_date: r2(interest),
    });
}

/// Trim a trailing `.0` from a whole number, keeping real fractions.
fn trim(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", r4(v))
    }
}

/// Render the term as "10 years", "6 months", or "10 years 6 months".
fn fmt_term(years: f64, months: f64) -> String {
    let y = years.floor() as i64;
    let extra = (years - y as f64) * 12.0 + months;
    let mo = extra.round() as i64;
    match (y, mo) {
        (0, 0) => "0 months".into(),
        (y, 0) => format!("{y} year{}", plural(y)),
        (0, mo) => format!("{mo} month{}", plural(mo)),
        (y, mo) => format!("{y} year{} {mo} month{}", plural(y), plural(mo)),
    }
}

fn plural(n: i64) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn require_finite(label: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() {
        return Err(format!("{label} must be a finite number"));
    }
    Ok(())
}

/// Same as [`compute`] but returns pretty-printed JSON (for the web page).
pub fn compute_json(i: &Inputs) -> Result<String, String> {
    let res = compute(i)?;
    serde_json::to_string_pretty(&res).map_err(|e| format!("serialization failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp() -> Inputs {
        Inputs::default()
    }

    #[test]
    fn lump_sum_annual_compounding() {
        // 1000 at 5% compounded annually for 10 years = 1000 * 1.05^10.
        let mut i = inp();
        i.principal = Some(1000.0);
        i.annual_rate = Some(5.0);
        i.years = Some(10.0);
        i.months = Some(0.0);
        i.compounding = Some("annually".into());
        i.contribution = Some(0.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.future_value, 1628.89); // 1628.894627...
        assert_eq!(r.total_contributions, 0.0);
        assert_eq!(r.total_interest, 628.89);
        assert_eq!(r.effective_annual_rate, 5.0);
    }

    #[test]
    fn monthly_contributions_ordinary_annuity() {
        // Classic: PV 0, 100/month, 12 monthly-compounded periods at 6% nominal.
        // FV = 100 * ((1.005^12 - 1)/0.005) = 1233.556...
        let mut i = inp();
        i.principal = Some(0.0);
        i.annual_rate = Some(6.0);
        i.years = Some(1.0);
        i.compounding = Some("monthly".into());
        i.contribution = Some(100.0);
        i.contribution_frequency = Some("monthly".into());
        i.contribution_timing = Some("end".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.future_value, 1233.56);
        assert_eq!(r.total_contributions, 1200.0);
        assert_eq!(r.total_interest, 33.56);
    }

    #[test]
    fn annuity_due_beats_ordinary() {
        let mut i = inp();
        i.principal = Some(0.0);
        i.annual_rate = Some(6.0);
        i.years = Some(1.0);
        i.compounding = Some("monthly".into());
        i.contribution = Some(100.0);
        i.contribution_timing = Some("start".into());
        let due = compute(&i).unwrap().future_value;
        i.contribution_timing = Some("end".into());
        let ordinary = compute(&i).unwrap().future_value;
        assert!(due > ordinary, "due {due} should exceed ordinary {ordinary}");
    }

    #[test]
    fn continuous_compounding_uses_e_rt() {
        // 1000 at 5% continuous for 10y = 1000 * e^0.5 = 1648.72...
        let mut i = inp();
        i.principal = Some(1000.0);
        i.annual_rate = Some(5.0);
        i.years = Some(10.0);
        i.compounding = Some("continuously".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.future_value, 1648.72);
        // EAR of 5% continuous = e^0.05 - 1 = 5.1271%.
        assert_eq!(r.effective_annual_rate, 5.1271);
    }

    #[test]
    fn zero_rate_is_just_deposits() {
        let mut i = inp();
        i.principal = Some(500.0);
        i.annual_rate = Some(0.0);
        i.years = Some(2.0);
        i.contribution = Some(50.0);
        i.contribution_frequency = Some("monthly".into());
        let r = compute(&i).unwrap();
        // 500 + 50*24 = 1700, no interest.
        assert_eq!(r.future_value, 1700.0);
        assert_eq!(r.total_interest, 0.0);
        assert_eq!(r.years_to_double, None);
    }

    #[test]
    fn defaults_apply_when_unset() {
        // All-default: 1000, 5%, 10y, monthly, no contributions.
        let r = compute(&inp()).unwrap();
        // 1000 * (1 + 0.05/12)^120 = 1647.01
        assert_eq!(r.future_value, 1647.01);
        assert!(r.summary.contains("grows to"));
    }

    #[test]
    fn schedule_has_final_and_whole_year_rows() {
        let mut i = inp();
        i.principal = Some(1000.0);
        i.annual_rate = Some(10.0);
        i.years = Some(3.0);
        i.compounding = Some("annually".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.schedule.len(), 3);
        assert_eq!(r.schedule[0].year, 1.0);
        assert_eq!(r.schedule[0].balance, 1100.0);
        assert_eq!(r.schedule[2].balance, 1331.0);
    }

    #[test]
    fn fractional_term_adds_final_row() {
        let mut i = inp();
        i.years = Some(2.0);
        i.months = Some(6.0);
        i.compounding = Some("annually".into());
        i.contribution = Some(0.0);
        let r = compute(&i).unwrap();
        // whole-year rows at 1 and 2, plus a final row at 2.5.
        assert_eq!(r.schedule.len(), 3);
        assert_eq!(r.schedule.last().unwrap().year, 2.5);
        assert_eq!(r.years, 2.5);
    }

    #[test]
    fn schedule_is_capped() {
        let mut i = inp();
        i.years = Some(500.0);
        i.compounding = Some("annually".into());
        let r = compute(&i).unwrap();
        assert!(r.schedule.len() <= MAX_SCHEDULE_ROWS);
        assert_eq!(r.schedule.last().unwrap().year, 500.0);
    }

    #[test]
    fn negative_contribution_is_withdrawal() {
        let mut i = inp();
        i.principal = Some(10000.0);
        i.annual_rate = Some(4.0);
        i.years = Some(1.0);
        i.compounding = Some("monthly".into());
        i.contribution = Some(-100.0);
        i.contribution_frequency = Some("monthly".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.total_contributions, -1200.0);
        assert!(r.future_value < 10000.0);
    }

    #[test]
    fn unknown_compounding_errors() {
        let mut i = inp();
        i.compounding = Some("hourly".into());
        let err = compute(&i).unwrap_err();
        assert!(err.contains("unknown compounding"), "{err}");
    }

    #[test]
    fn unknown_frequency_errors() {
        let mut i = inp();
        i.contribution = Some(10.0);
        i.contribution_frequency = Some("hourly".into());
        let err = compute(&i).unwrap_err();
        assert!(err.contains("unknown contribution_frequency"), "{err}");
    }

    #[test]
    fn zero_term_errors() {
        let mut i = inp();
        i.years = Some(0.0);
        i.months = Some(0.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("term must be greater than zero"), "{err}");
    }

    #[test]
    fn negative_term_errors() {
        let mut i = inp();
        i.years = Some(-1.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("must not be negative"), "{err}");
    }

    #[test]
    fn nonfinite_principal_errors() {
        let mut i = inp();
        i.principal = Some(f64::NAN);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("finite"), "{err}");
    }

    #[test]
    fn frequency_keywords_are_normalized() {
        let mut i = inp();
        i.compounding = Some("Semi-Annually".into());
        i.contribution = Some(0.0);
        let r = compute(&i).unwrap();
        // EAR of 5% semiannual = (1.025)^2 - 1 = 5.0625%.
        assert_eq!(r.effective_annual_rate, 5.0625);
    }

    #[test]
    fn json_round_trips() {
        let json = compute_json(&inp()).unwrap();
        assert!(json.contains("\"future_value\""));
        assert!(json.contains("\"schedule\""));
        assert!(json.contains("\"effective_annual_rate\""));
    }
}
