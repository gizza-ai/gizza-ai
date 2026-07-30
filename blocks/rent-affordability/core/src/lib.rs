//! rent-affordability core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Answers "how much rent can I afford?" from an income figure using the classic
//! **30% rule** (rent should be at most 30% of gross monthly income) — or any
//! custom rent-to-income ratio. It also optionally factors in existing monthly
//! **debt** payments via a back-end debt-to-income (DTI) cap, converts a take-home
//! (net) income to gross with a flat assumed tax rate, and reports a multi-scenario
//! guideline range plus the money left over each month.
//!
//! Definitions (all money in the caller's currency, all math `f64`):
//!
//! ```text
//! gross_monthly       = income normalised to a gross monthly amount
//! max_affordable_rent = gross_monthly * rent_to_income_ratio / 100      (the rule)
//! debt_adjusted_rent  = max(0, gross_monthly * max_dti / 100 - monthly_debts)
//! recommended_rent    = min(max_affordable_rent, debt_adjusted_rent)
//! remaining           = gross_monthly - recommended_rent - monthly_debts
//! ```
//!
//! When `income_type = net`, gross is backed out as `net / (1 - tax_rate/100)`
//! (Zillow-style flat gross-up) so the ratio still applies to pre-tax income the way
//! landlords screen. Money outputs are rounded to the caller's `decimals` (default 2).

use serde::Serialize;

/// Conservative / moderate / aggressive rent anchors as a percent of gross monthly
/// income — the standard multi-scenario framing, independent of the custom ratio.
const CONSERVATIVE_PCT: f64 = 25.0;
const MODERATE_PCT: f64 = 30.0;
const AGGRESSIVE_PCT: f64 = 35.0;

/// A guideline rent range: the same gross monthly income seen through three common
/// rent-to-income anchors (25% / 30% / 35%).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GuidelineRange {
    /// 25% of gross monthly income — a cautious budget with more cushion.
    pub conservative: f64,
    /// 30% of gross monthly income — the classic 30% rule.
    pub moderate: f64,
    /// 35% of gross monthly income — a stretch budget.
    pub aggressive: f64,
}

/// Structured affordability result. Every money field is rounded to the requested
/// number of decimals.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RentResult {
    /// Gross (pre-tax) annual income used for the calculation (money).
    pub gross_annual_income: f64,
    /// Gross (pre-tax) monthly income used for the calculation (money).
    pub gross_monthly_income: f64,
    /// The rent-to-income ratio applied, echoed as a percent.
    pub rent_to_income_ratio: f64,
    /// Headline maximum rent from the rule: `gross_monthly * ratio / 100` (money).
    pub max_affordable_rent: f64,
    /// Debt-adjusted rent ceiling from the back-end DTI cap:
    /// `max(0, gross_monthly * max_dti / 100 - monthly_debts)` (money).
    pub debt_adjusted_max_rent: f64,
    /// Recommended max rent = the smaller of `max_affordable_rent` and
    /// `debt_adjusted_max_rent`, so existing debts pull the number down (money).
    pub recommended_max_rent: f64,
    /// Recommended rent × 12 — the yearly rent commitment (money).
    pub annual_rent_at_recommended: f64,
    /// Gross monthly income left after the recommended rent and monthly debts (money).
    pub remaining_after_rent_and_debts: f64,
    /// Gross monthly income ÷ recommended rent — the "landlords want N× the rent in
    /// income" multiple (e.g. 3.33 for the 30% rule). 0 when rent is 0.
    pub income_to_rent_multiple: f64,
    /// The 25% / 30% / 35% guideline range on gross monthly income.
    pub guideline_range: GuidelineRange,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// Hard cap on any single money input (dollars), guarding against overflow / typos.
pub const MAX_MONEY: f64 = 1_000_000_000.0;

/// All inputs. Each field is `None` when unset; [`compute`] applies the documented
/// default for any `None`, so every surface (chat, CLI, page) funnels through the
/// same defaults + validation.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Income amount. Default 60000.
    pub income: Option<f64>,
    /// Whether `income` is an `annual` or `monthly` figure. Default `annual`.
    pub income_period: Option<String>,
    /// Whether `income` is `gross` (pre-tax) or `net` (take-home). Default `gross`.
    pub income_type: Option<String>,
    /// Assumed tax+deductions rate (percent) used to gross up a net income. Default 25.
    pub tax_rate_percent: Option<f64>,
    /// Rent-to-income ratio as a percent — the rule. Default 30.
    pub rent_to_income_ratio: Option<f64>,
    /// Existing recurring monthly debt payments. Default 0.
    pub monthly_debts: Option<f64>,
    /// Back-end debt-to-income cap (percent): `(rent + debts) ≤ max_dti% of gross`.
    /// Default 36.
    pub max_dti_ratio: Option<f64>,
    /// Currency symbol prefixed to amounts in the summary. Default `$`.
    pub currency: Option<String>,
    /// Decimal places for money outputs. Default 2.
    pub decimals: Option<f64>,
}

/// Round `x` to `decimals` places (half-away-from-zero, matching a currency display).
fn round_to(x: f64, decimals: u32) -> f64 {
    let f = 10f64.powi(decimals as i32);
    (x * f).round() / f
}

/// Format a money value with thousands separators and two decimals for the summary
/// line (e.g. `12345.6` → `12,345.60`).
fn money(v: f64) -> String {
    let v = round_to(v, 2);
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

/// Trim a trailing `.0` from a whole number for the summary (e.g. `30.0` → `30`).
fn trim(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn require_finite(label: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() {
        return Err(format!("{label} must be a finite number"));
    }
    Ok(())
}

fn require_nonneg(label: &str, v: f64) -> Result<(), String> {
    require_finite(label, v)?;
    if v < 0.0 {
        return Err(format!("{label} must not be negative"));
    }
    if v > MAX_MONEY {
        return Err(format!("{label} is too large (max {})", trim(MAX_MONEY)));
    }
    Ok(())
}

/// Normalise a case-insensitive enum choice, erroring with the allowed values.
fn parse_choice(
    label: &str,
    v: &Option<String>,
    allowed: &[&str],
    default: &str,
) -> Result<String, String> {
    match v {
        None => Ok(default.to_string()),
        Some(s) => {
            let t = s.trim().to_ascii_lowercase();
            if t.is_empty() {
                return Ok(default.to_string());
            }
            if allowed.iter().any(|a| *a == t) {
                Ok(t)
            } else {
                Err(format!(
                    "{label} must be one of {} (got '{}')",
                    allowed.join(", "),
                    s.trim()
                ))
            }
        }
    }
}

/// Compute the rent-affordability result from the supplied inputs, applying defaults
/// for any unset field. Errors on non-finite/negative money, a non-positive income,
/// an out-of-range ratio, or a net gross-up with a tax rate of 100% or more.
pub fn compute(i: &Inputs) -> Result<RentResult, String> {
    let income = i.income.unwrap_or(60_000.0);
    let period = parse_choice(
        "income_period",
        &i.income_period,
        &["annual", "monthly"],
        "annual",
    )?;
    let itype = parse_choice("income_type", &i.income_type, &["gross", "net"], "gross")?;
    let tax_rate = i.tax_rate_percent.unwrap_or(25.0);
    let ratio = i.rent_to_income_ratio.unwrap_or(30.0);
    let debts = i.monthly_debts.unwrap_or(0.0);
    let max_dti = i.max_dti_ratio.unwrap_or(36.0);
    let currency = i.currency.clone().unwrap_or_else(|| "$".to_string());
    let decimals_f = i.decimals.unwrap_or(2.0);

    require_nonneg("income", income)?;
    require_finite("tax_rate_percent", tax_rate)?;
    require_finite("rent_to_income_ratio", ratio)?;
    require_nonneg("monthly_debts", debts)?;
    require_finite("max_dti_ratio", max_dti)?;
    require_finite("decimals", decimals_f)?;

    if income <= 0.0 {
        return Err("income must be greater than zero".into());
    }
    if !(1.0..=100.0).contains(&ratio) {
        return Err("rent_to_income_ratio must be between 1 and 100 (percent)".into());
    }
    if !(1.0..=100.0).contains(&max_dti) {
        return Err("max_dti_ratio must be between 1 and 100 (percent)".into());
    }
    if !(0.0..=10.0).contains(&decimals_f) {
        return Err("decimals must be between 0 and 10".into());
    }
    let decimals = decimals_f as u32;

    // Normalise the entered income to a gross monthly figure.
    let monthly_entered = if period == "monthly" {
        income
    } else {
        income / 12.0
    };
    let gross_monthly = if itype == "net" {
        if !(0.0..100.0).contains(&tax_rate) {
            return Err(
                "tax_rate_percent must be between 0 and 99.99 to gross up a net income".into(),
            );
        }
        monthly_entered / (1.0 - tax_rate / 100.0)
    } else {
        monthly_entered
    };
    if !gross_monthly.is_finite() || gross_monthly <= 0.0 {
        return Err(
            "gross monthly income is not a positive finite number — check the inputs".into(),
        );
    }
    let gross_annual = gross_monthly * 12.0;

    let max_affordable_rent = gross_monthly * ratio / 100.0;
    let debt_adjusted = (gross_monthly * max_dti / 100.0 - debts).max(0.0);
    let recommended = max_affordable_rent.min(debt_adjusted);
    let annual_rent = recommended * 12.0;
    let remaining = gross_monthly - recommended - debts;
    let multiple = if max_affordable_rent > 0.0 {
        gross_monthly / max_affordable_rent
    } else {
        0.0
    };

    let range = GuidelineRange {
        conservative: round_to(gross_monthly * CONSERVATIVE_PCT / 100.0, decimals),
        moderate: round_to(gross_monthly * MODERATE_PCT / 100.0, decimals),
        aggressive: round_to(gross_monthly * AGGRESSIVE_PCT / 100.0, decimals),
    };

    let debt_note = if debts > 0.0 {
        format!(
            " Recommended {cur}{rec} /mo after {cur}{d} /mo of debts (≤{dti}% DTI).",
            cur = currency,
            rec = money(recommended),
            d = money(debts),
            dti = trim(round_to(max_dti, 4))
        )
    } else {
        String::new()
    };
    let summary = format!(
        "{cur}{gm} gross monthly income ({cur}{ga}/yr): the {r}% rule puts your max rent at {cur}{max} /mo.{debt} Guideline range {cur}{c}–{cur}{a} /mo (25%–35%). ~{cur}{rem} /mo left after rent{and_debts}.",
        cur = currency,
        gm = money(gross_monthly),
        ga = money(gross_annual),
        r = trim(round_to(ratio, 4)),
        max = money(max_affordable_rent),
        debt = debt_note,
        c = money(range.conservative),
        a = money(range.aggressive),
        rem = money(remaining),
        and_debts = if debts > 0.0 { " and debts" } else { "" },
    );

    Ok(RentResult {
        gross_annual_income: round_to(gross_annual, decimals),
        gross_monthly_income: round_to(gross_monthly, decimals),
        rent_to_income_ratio: round_to(ratio, 4),
        max_affordable_rent: round_to(max_affordable_rent, decimals),
        debt_adjusted_max_rent: round_to(debt_adjusted, decimals),
        recommended_max_rent: round_to(recommended, decimals),
        annual_rent_at_recommended: round_to(annual_rent, decimals),
        remaining_after_rent_and_debts: round_to(remaining, decimals),
        income_to_rent_multiple: round_to(multiple, 2),
        guideline_range: range,
        summary,
    })
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
    fn classic_30pct_rule_on_60k() {
        // $60,000/yr gross → $5,000/mo → 30% rule → $1,500/mo max rent.
        let mut i = inp();
        i.income = Some(60_000.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.gross_monthly_income, 5_000.0);
        assert_eq!(r.gross_annual_income, 60_000.0);
        assert_eq!(r.rent_to_income_ratio, 30.0);
        assert_eq!(r.max_affordable_rent, 1_500.0);
        // No debts → debt-adjusted (36% of 5000 = 1800) is looser, so recommended = 1500.
        assert_eq!(r.debt_adjusted_max_rent, 1_800.0);
        assert_eq!(r.recommended_max_rent, 1_500.0);
        assert_eq!(r.annual_rent_at_recommended, 18_000.0);
        assert_eq!(r.remaining_after_rent_and_debts, 3_500.0);
        assert_eq!(r.guideline_range.conservative, 1_250.0);
        assert_eq!(r.guideline_range.moderate, 1_500.0);
        assert_eq!(r.guideline_range.aggressive, 1_750.0);
        // 30% rule ⇒ landlords want ~3.33× the rent in income.
        assert_eq!(r.income_to_rent_multiple, 3.33);
        assert!(r.summary.contains("$1,500.00"));
    }

    #[test]
    fn monthly_income_period() {
        // Enter $5,000/mo directly → same as $60,000/yr.
        let mut i = inp();
        i.income = Some(5_000.0);
        i.income_period = Some("monthly".into());
        let r = compute(&i).unwrap();
        assert_eq!(r.gross_monthly_income, 5_000.0);
        assert_eq!(r.max_affordable_rent, 1_500.0);
    }

    #[test]
    fn custom_ratio() {
        // 40% ceiling (Zillow-style) on $5,000/mo → $2,000 rent.
        let mut i = inp();
        i.income = Some(5_000.0);
        i.income_period = Some("monthly".into());
        i.rent_to_income_ratio = Some(40.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.max_affordable_rent, 2_000.0);
        // debt-adjusted at 36% = 1800 < 2000 → recommended pulled down to 1800.
        assert_eq!(r.recommended_max_rent, 1_800.0);
    }

    #[test]
    fn debts_pull_down_the_recommendation() {
        // $6,000/mo, 30% rule → 1800 by the rule; $800 debts, 36% DTI →
        // debt-adjusted = 6000*0.36 - 800 = 1360 → recommended = 1360.
        let mut i = inp();
        i.income = Some(6_000.0);
        i.income_period = Some("monthly".into());
        i.monthly_debts = Some(800.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.max_affordable_rent, 1_800.0);
        assert_eq!(r.debt_adjusted_max_rent, 1_360.0);
        assert_eq!(r.recommended_max_rent, 1_360.0);
        assert_eq!(r.remaining_after_rent_and_debts, 6_000.0 - 1_360.0 - 800.0);
        assert!(r.summary.contains("after"));
    }

    #[test]
    fn net_income_is_grossed_up() {
        // $4,000/mo net at a 25% assumed tax → gross = 4000 / 0.75 = 5333.33/mo.
        let mut i = inp();
        i.income = Some(4_000.0);
        i.income_period = Some("monthly".into());
        i.income_type = Some("net".into());
        i.tax_rate_percent = Some(25.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.gross_monthly_income, 5_333.33);
        // 30% of 5333.33 = 1600.00.
        assert_eq!(r.max_affordable_rent, 1_600.0);
    }

    #[test]
    fn defaults_apply_when_unset() {
        let r = compute(&inp()).unwrap();
        assert_eq!(r.gross_monthly_income, 5_000.0);
        assert_eq!(r.max_affordable_rent, 1_500.0);
        assert!(r.summary.contains("30% rule"));
    }

    #[test]
    fn decimals_control_rounding() {
        let mut i = inp();
        i.income = Some(4_000.0);
        i.income_period = Some("monthly".into());
        i.income_type = Some("net".into());
        i.tax_rate_percent = Some(25.0);
        i.decimals = Some(0.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.gross_monthly_income, 5_333.0);
        assert_eq!(r.max_affordable_rent, 1_600.0);
    }

    #[test]
    fn currency_symbol_in_summary() {
        let mut i = inp();
        i.currency = Some("£".into());
        let r = compute(&i).unwrap();
        assert!(r.summary.contains("£1,500.00"), "{}", r.summary);
    }

    #[test]
    fn zero_income_errors() {
        let mut i = inp();
        i.income = Some(0.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("income must be greater than zero"), "{err}");
    }

    #[test]
    fn negative_income_errors() {
        let mut i = inp();
        i.income = Some(-5.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("must not be negative"), "{err}");
    }

    #[test]
    fn ratio_out_of_range_errors() {
        let mut i = inp();
        i.rent_to_income_ratio = Some(0.0);
        let err = compute(&i).unwrap_err();
        assert!(
            err.contains("rent_to_income_ratio must be between 1 and 100"),
            "{err}"
        );
    }

    #[test]
    fn bad_income_type_errors() {
        let mut i = inp();
        i.income_type = Some("after-tax".into());
        let err = compute(&i).unwrap_err();
        assert!(err.contains("income_type must be one of"), "{err}");
    }

    #[test]
    fn net_with_full_tax_rate_errors() {
        let mut i = inp();
        i.income_type = Some("net".into());
        i.tax_rate_percent = Some(100.0);
        let err = compute(&i).unwrap_err();
        assert!(
            err.contains("tax_rate_percent must be between 0 and 99.99"),
            "{err}"
        );
    }

    #[test]
    fn negative_debts_error() {
        let mut i = inp();
        i.monthly_debts = Some(-1.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("monthly_debts must not be negative"), "{err}");
    }

    #[test]
    fn json_round_trips() {
        let json = compute_json(&inp()).unwrap();
        assert!(json.contains("\"max_affordable_rent\""));
        assert!(json.contains("\"recommended_max_rent\""));
        assert!(json.contains("\"guideline_range\""));
        assert!(json.contains("\"conservative\""));
        assert!(json.contains("\"summary\""));
    }
}
