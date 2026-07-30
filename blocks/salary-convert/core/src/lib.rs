//! salary-convert core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Converts a single pay figure between hourly, daily, weekly, biweekly, monthly
//! and annual rates. The conversion pivots through an ANNUAL salary computed from
//! the input period, using an explicit work schedule (hours per week, days per
//! week, weeks per year). Every call reports ALL six period figures at once, so
//! the caller never has to convert twice. All arithmetic is deterministic and
//! money figures are rounded to 2 decimal places for display.

use serde::Serialize;

/// A fully-resolved salary breakdown, reported the same way from every surface.
#[derive(Serialize, Debug, PartialEq)]
pub struct Breakdown {
    /// The raw pay figure that was entered.
    pub input_amount: f64,
    /// The period the input figure was given in (hourly/daily/weekly/…).
    pub input_period: String,
    /// Hours worked per week used for the conversion.
    pub hours_per_week: f64,
    /// Days worked per week used for the daily figure.
    pub days_per_week: f64,
    /// Paid weeks per year used for the conversion.
    pub weeks_per_year: f64,
    /// Equivalent hourly pay.
    pub hourly: f64,
    /// Equivalent daily pay (one work day).
    pub daily: f64,
    /// Equivalent weekly pay.
    pub weekly: f64,
    /// Equivalent biweekly pay (every two weeks).
    pub biweekly: f64,
    /// Equivalent monthly pay (annual ÷ 12).
    pub monthly: f64,
    /// Equivalent annual pay.
    pub annual: f64,
    /// A human-readable one-line summary.
    pub summary: String,
}

/// Round a money figure to 2 decimal places.
fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Format a money figure with a currency prefix and thousands separators, 2 dp.
fn money(prefix: &str, x: f64) -> String {
    let neg = x < 0.0;
    let v = round2(x.abs());
    let whole = v.trunc() as i64;
    let cents = ((v - whole as f64) * 100.0).round() as i64;
    // Group the integer part with commas.
    let digits = whole.to_string();
    let mut grouped = String::new();
    let bytes = digits.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*b as char);
    }
    format!(
        "{}{}{}.{:02}",
        if neg { "-" } else { "" },
        prefix,
        grouped,
        cents
    )
}

/// Convert `amount` (given in `period`) into every period figure.
///
/// - `amount`   — the pay figure to convert. Must be finite and ≥ 0.
/// - `period`   — one of `hourly`, `daily`, `weekly`, `biweekly`, `monthly`, `annual`.
/// - `hours_per_week` — hours worked each week (default 40 upstream). Must be > 0.
/// - `days_per_week`  — days worked each week (default 5 upstream). Must be > 0.
/// - `weeks_per_year` — paid weeks per year (default 52 upstream). Must be > 0.
/// - `currency` — a currency symbol/prefix used only in the summary text.
pub fn convert(
    amount: f64,
    period: &str,
    hours_per_week: f64,
    days_per_week: f64,
    weeks_per_year: f64,
    currency: &str,
) -> Result<Breakdown, String> {
    if !amount.is_finite() {
        return Err("amount must be a finite number".into());
    }
    if amount < 0.0 {
        return Err("amount must be zero or positive".into());
    }
    if !hours_per_week.is_finite() || hours_per_week <= 0.0 {
        return Err("hours_per_week must be greater than 0".into());
    }
    if hours_per_week > 168.0 {
        return Err("hours_per_week cannot exceed 168 (hours in a week)".into());
    }
    if !days_per_week.is_finite() || days_per_week <= 0.0 {
        return Err("days_per_week must be greater than 0".into());
    }
    if days_per_week > 7.0 {
        return Err("days_per_week cannot exceed 7".into());
    }
    if !weeks_per_year.is_finite() || weeks_per_year <= 0.0 {
        return Err("weeks_per_year must be greater than 0".into());
    }
    if weeks_per_year > 53.0 {
        return Err("weeks_per_year cannot exceed 53".into());
    }

    let hours_per_year = hours_per_week * weeks_per_year;
    let days_per_year = days_per_week * weeks_per_year;
    // Number of biweekly (fortnightly) pay periods in a year.
    let biweekly_periods = weeks_per_year / 2.0;

    // Pivot everything through an annual figure derived from the input period.
    let period_key = period.trim().to_lowercase();
    let annual = match period_key.as_str() {
        "hourly" | "hour" | "hr" => amount * hours_per_year,
        "daily" | "day" => amount * days_per_year,
        "weekly" | "week" => amount * weeks_per_year,
        "biweekly" | "fortnightly" | "fortnight" => amount * biweekly_periods,
        "monthly" | "month" => amount * 12.0,
        "annual" | "annually" | "yearly" | "year" => amount,
        other => {
            return Err(format!(
                "unknown period '{other}' — use hourly, daily, weekly, biweekly, monthly, or annual"
            ))
        }
    };

    let hourly = round2(annual / hours_per_year);
    let daily = round2(annual / days_per_year);
    let weekly = round2(annual / weeks_per_year);
    let biweekly = round2(annual / biweekly_periods);
    let monthly = round2(annual / 12.0);
    let annual = round2(annual);

    let prefix = currency.trim();
    let summary = format!(
        "{} hourly · {} daily · {} weekly · {} biweekly · {} monthly · {} annual (at {} h/week, {} weeks/year)",
        money(prefix, hourly),
        money(prefix, daily),
        money(prefix, weekly),
        money(prefix, biweekly),
        money(prefix, monthly),
        money(prefix, annual),
        trim_num(hours_per_week),
        trim_num(weeks_per_year),
    );

    Ok(Breakdown {
        input_amount: round2(amount),
        input_period: period_key,
        hours_per_week,
        days_per_week,
        weeks_per_year,
        hourly,
        daily,
        weekly,
        biweekly,
        monthly,
        annual,
        summary,
    })
}

/// Render a schedule number without a trailing `.0` (40.0 → "40", 37.5 → "37.5").
fn trim_num(x: f64) -> String {
    if x.fract() == 0.0 {
        format!("{}", x as i64)
    } else {
        let s = format!("{x}");
        s
    }
}

/// Convert and return a pretty-printed JSON object (used by the web page).
pub fn convert_json(
    amount: f64,
    period: &str,
    hours_per_week: f64,
    days_per_week: f64,
    weeks_per_year: f64,
    currency: &str,
) -> Result<String, String> {
    let b = convert(amount, period, hours_per_week, days_per_week, weeks_per_year, currency)?;
    serde_json::to_string_pretty(&b).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annual_to_all_defaults() {
        // $52,000/year at 40 h/week, 5 d/week, 52 weeks/year.
        let b = convert(52_000.0, "annual", 40.0, 5.0, 52.0, "$").unwrap();
        assert_eq!(b.annual, 52_000.0);
        assert_eq!(b.monthly, round2(52_000.0 / 12.0)); // 4333.33
        assert_eq!(b.weekly, 1_000.0);
        assert_eq!(b.biweekly, 2_000.0);
        assert_eq!(b.daily, 200.0);
        assert_eq!(b.hourly, 25.0);
        assert_eq!(b.input_period, "annual");
    }

    #[test]
    fn hourly_to_annual_defaults() {
        // $25/hour × 40 × 52 = $52,000/year.
        let b = convert(25.0, "hourly", 40.0, 5.0, 52.0, "$").unwrap();
        assert_eq!(b.annual, 52_000.0);
        assert_eq!(b.weekly, 1_000.0);
        assert_eq!(b.monthly, round2(52_000.0 / 12.0));
    }

    #[test]
    fn monthly_to_annual() {
        let b = convert(5_000.0, "monthly", 40.0, 5.0, 52.0, "$").unwrap();
        assert_eq!(b.annual, 60_000.0);
        assert_eq!(b.weekly, round2(60_000.0 / 52.0));
    }

    #[test]
    fn weekly_and_biweekly_relationship() {
        let b = convert(1_000.0, "weekly", 40.0, 5.0, 52.0, "$").unwrap();
        assert_eq!(b.biweekly, 2_000.0);
        assert_eq!(b.annual, 52_000.0);
    }

    #[test]
    fn biweekly_input() {
        // $2,000 every two weeks → $52,000/year (26 pay periods).
        let b = convert(2_000.0, "biweekly", 40.0, 5.0, 52.0, "$").unwrap();
        assert_eq!(b.annual, 52_000.0);
        assert_eq!(b.weekly, 1_000.0);
    }

    #[test]
    fn part_time_hours() {
        // $30/hour × 20 h/week × 52 = $31,200/year.
        let b = convert(30.0, "hourly", 20.0, 5.0, 52.0, "$").unwrap();
        assert_eq!(b.annual, 31_200.0);
    }

    #[test]
    fn custom_weeks_per_year() {
        // Two unpaid weeks: 50 paid weeks.
        let b = convert(20.0, "hourly", 40.0, 5.0, 50.0, "$").unwrap();
        assert_eq!(b.annual, round2(20.0 * 40.0 * 50.0)); // 40,000
        assert_eq!(b.weekly, 800.0);
    }

    #[test]
    fn daily_input() {
        // $200/day × 5 d/week × 52 = $52,000/year.
        let b = convert(200.0, "daily", 40.0, 5.0, 52.0, "$").unwrap();
        assert_eq!(b.annual, 52_000.0);
        assert_eq!(b.hourly, 25.0);
    }

    #[test]
    fn period_aliases_accepted() {
        assert!(convert(50.0, "hr", 40.0, 5.0, 52.0, "$").is_ok());
        assert!(convert(50.0, "YEARLY", 40.0, 5.0, 52.0, "$").is_ok());
        assert!(convert(50.0, "Fortnightly", 40.0, 5.0, 52.0, "$").is_ok());
    }

    #[test]
    fn zero_amount_is_ok() {
        let b = convert(0.0, "annual", 40.0, 5.0, 52.0, "$").unwrap();
        assert_eq!(b.hourly, 0.0);
        assert_eq!(b.monthly, 0.0);
    }

    #[test]
    fn rejects_negative_amount() {
        assert!(convert(-1.0, "annual", 40.0, 5.0, 52.0, "$").is_err());
    }

    #[test]
    fn rejects_bad_schedule() {
        assert!(convert(50.0, "hourly", 0.0, 5.0, 52.0, "$").is_err());
        assert!(convert(50.0, "hourly", 200.0, 5.0, 52.0, "$").is_err());
        assert!(convert(50.0, "hourly", 40.0, 0.0, 52.0, "$").is_err());
        assert!(convert(50.0, "hourly", 40.0, 8.0, 52.0, "$").is_err());
        assert!(convert(50.0, "hourly", 40.0, 5.0, 0.0, "$").is_err());
        assert!(convert(50.0, "hourly", 40.0, 5.0, 60.0, "$").is_err());
    }

    #[test]
    fn rejects_unknown_period() {
        assert!(convert(50.0, "quarterly", 40.0, 5.0, 52.0, "$").is_err());
    }

    #[test]
    fn money_formats_with_commas() {
        assert_eq!(money("$", 52000.0), "$52,000.00");
        assert_eq!(money("$", 4333.333), "$4,333.33");
        assert_eq!(money("£", 1000000.0), "£1,000,000.00");
        assert_eq!(money("$", 25.0), "$25.00");
    }

    #[test]
    fn json_shape() {
        let j = convert_json(52_000.0, "annual", 40.0, 5.0, 52.0, "$").unwrap();
        assert!(j.contains("\"annual\": 52000.0"));
        assert!(j.contains("\"hourly\": 25.0"));
        assert!(j.contains("\"input_period\": \"annual\""));
    }
}
