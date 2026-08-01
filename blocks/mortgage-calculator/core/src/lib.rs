//! mortgage-calculator core — pure mortgage math shared by the chat skill block
//! and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Given a home price, a down-payment amount, a loan term and a nominal annual
//! interest rate — plus optional property tax, homeowner's insurance, HOA dues
//! and an extra monthly principal payment — it computes the fixed-rate monthly
//! payment and the full cost of the loan.
//!
//! The financed amount is `loan = home_price - down_payment`. The monthly
//! principal-and-interest payment uses the standard amortizing-loan formula
//!
//! ```text
//! M = L * i / (1 - (1 + i)^-n)      (i = rate/100/12, n = loan_years * 12)
//! ```
//!
//! and reduces to `L / n` when the rate is zero. Property tax and insurance are
//! quoted annually and spread evenly across the year; HOA is already monthly.
//! The loan is then amortized month by month (applying any `extra_monthly_payment`
//! straight to principal) to find the real payoff month and the exact total
//! interest — so an extra payment correctly shortens the term and cuts interest.
//!
//! All math is `f64`; money is rounded to the caller's `decimals` (default 2).

use serde::Serialize;

/// Structured mortgage result. Every money field is rounded to the requested
/// number of decimals; `payoff_months` is a whole month count.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MortgageResult {
    /// Amount financed: `home_price - down_payment` (money).
    pub loan_amount: f64,
    /// Monthly principal + interest on the loan (money).
    pub monthly_principal_interest: f64,
    /// Monthly property tax (`annual_property_tax / 12`) (money).
    pub monthly_taxes: f64,
    /// Monthly homeowner's insurance (`annual_insurance / 12`) (money).
    pub monthly_insurance: f64,
    /// Monthly HOA / condo dues, echoed from the input (money).
    pub monthly_hoa: f64,
    /// Full monthly housing payment: principal + interest + taxes + insurance +
    /// HOA (money). The `extra_monthly_payment` is applied on top of this during
    /// amortization but is not part of this "base" figure.
    pub monthly_payment: f64,
    /// Number of monthly payments until the loan is paid off. Equals
    /// `loan_years * 12` with no extra payment, and fewer when an extra monthly
    /// payment is supplied.
    pub payoff_months: u32,
    /// Total principal repaid over the life of the loan — equals `loan_amount`
    /// (money).
    pub total_principal: f64,
    /// Total interest paid over the life of the loan (money).
    pub total_interest: f64,
    /// Total property tax paid across the payoff period (money).
    pub total_tax: f64,
    /// Total insurance paid across the payoff period (money).
    pub total_insurance: f64,
    /// Total HOA dues paid across the payoff period (money).
    pub total_hoa: f64,
    /// Total cost of the purchase: down payment + principal + interest + tax +
    /// insurance + HOA over the payoff period (money).
    pub total_cost: f64,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// Hard cap on the loan term in years, to keep the amortization loop bounded.
pub const MAX_YEARS: f64 = 100.0;

/// Hard cap on the nominal annual interest rate (percent).
pub const MAX_RATE: f64 = 100.0;

/// All inputs. Each field is `None` when unset; [`compute`] applies the
/// documented default for any `None`, so every surface (chat, CLI, page) funnels
/// through the same defaults + validation.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Purchase price of the home. Default 400000.
    pub home_price: Option<f64>,
    /// Down-payment amount (not a percent). Default 80000.
    pub down_payment: Option<f64>,
    /// Loan term in whole years. Default 30.
    pub loan_years: Option<f64>,
    /// Nominal annual interest rate as a percent (e.g. 6.5 for 6.5%). Default 6.5.
    pub annual_interest_rate_percent: Option<f64>,
    /// Annual property tax amount. Default 0.
    pub annual_property_tax: Option<f64>,
    /// Annual homeowner's insurance premium. Default 0.
    pub annual_insurance: Option<f64>,
    /// Monthly HOA / condo dues. Default 0.
    pub monthly_hoa: Option<f64>,
    /// Extra amount paid toward principal every month. Default 0.
    pub extra_monthly_payment: Option<f64>,
    /// Decimal places for money outputs. Default 2.
    pub decimals: Option<f64>,
}

/// Round `x` to `decimals` places (half-away-from-zero, matching a currency
/// display).
fn round_to(x: f64, decimals: u32) -> f64 {
    let f = 10f64.powi(decimals as i32);
    (x * f).round() / f
}

/// Format a money value with thousands separators and two decimals for the
/// summary line (e.g. `12345.6` → `12,345.60`).
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

/// Trim a trailing `.0` from a whole number for the summary term.
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
    Ok(())
}

/// Compute the mortgage result from the supplied inputs, applying defaults for
/// any unset field. Errors on non-finite numbers, a non-positive price/term, a
/// down payment above the price, or an out-of-range rate.
pub fn compute(i: &Inputs) -> Result<MortgageResult, String> {
    let home_price = i.home_price.unwrap_or(400_000.0);
    let down_payment = i.down_payment.unwrap_or(80_000.0);
    let loan_years = i.loan_years.unwrap_or(30.0);
    let rate_pct = i.annual_interest_rate_percent.unwrap_or(6.5);
    let annual_tax = i.annual_property_tax.unwrap_or(0.0);
    let annual_ins = i.annual_insurance.unwrap_or(0.0);
    let monthly_hoa = i.monthly_hoa.unwrap_or(0.0);
    let extra = i.extra_monthly_payment.unwrap_or(0.0);
    let decimals_f = i.decimals.unwrap_or(2.0);

    require_finite("home_price", home_price)?;
    require_finite("down_payment", down_payment)?;
    require_finite("loan_years", loan_years)?;
    require_finite("annual_interest_rate_percent", rate_pct)?;
    require_nonneg("annual_property_tax", annual_tax)?;
    require_nonneg("annual_insurance", annual_ins)?;
    require_nonneg("monthly_hoa", monthly_hoa)?;
    require_nonneg("extra_monthly_payment", extra)?;
    require_finite("decimals", decimals_f)?;

    if home_price <= 0.0 {
        return Err("home_price must be greater than zero".into());
    }
    if down_payment < 0.0 {
        return Err("down_payment must not be negative".into());
    }
    if down_payment > home_price {
        return Err("down_payment must not exceed home_price".into());
    }
    if loan_years <= 0.0 {
        return Err("loan_years must be greater than zero".into());
    }
    if loan_years > MAX_YEARS {
        return Err(format!("loan_years must be at most {MAX_YEARS} years"));
    }
    if rate_pct < 0.0 {
        return Err("annual_interest_rate_percent must not be negative".into());
    }
    if rate_pct > MAX_RATE {
        return Err(format!(
            "annual_interest_rate_percent must be at most {MAX_RATE}"
        ));
    }
    if !(0.0..=10.0).contains(&decimals_f) {
        return Err("decimals must be between 0 and 10".into());
    }
    let decimals = decimals_f as u32;

    let loan = home_price - down_payment;
    let n = (loan_years * 12.0).round() as u32;
    if n == 0 {
        return Err("loan term rounds to zero months".into());
    }
    let i_m = rate_pct / 100.0 / 12.0;

    // Monthly principal + interest via the amortizing-loan formula; L/n at 0%.
    let monthly_pi = if loan <= 0.0 {
        0.0
    } else if i_m == 0.0 {
        loan / n as f64
    } else {
        loan * i_m / (1.0 - (1.0 + i_m).powi(-(n as i32)))
    };
    if !monthly_pi.is_finite() {
        return Err("the monthly payment is not a finite number — check the rate and term".into());
    }

    let monthly_taxes = annual_tax / 12.0;
    let monthly_insurance = annual_ins / 12.0;

    // Amortize month by month, applying the extra payment straight to principal.
    // The extra can only shorten the term, so `n` bounds the loop; +2 guards
    // against float residue on the final month.
    let mut balance = loan;
    let mut total_interest = 0.0f64;
    let mut payoff_months: u32 = 0;
    if loan > 0.0 {
        let pay = monthly_pi + extra;
        let cap = n + 2;
        for _ in 0..cap {
            let interest = balance * i_m;
            let mut principal_pay = pay - interest;
            if principal_pay <= 0.0 {
                // Payment can't cover interest — cannot happen for an amortizing
                // loan, but guard against a pathological rate anyway.
                return Err(
                    "the monthly payment does not cover the interest — increase the term or payment"
                        .into(),
                );
            }
            if principal_pay >= balance {
                principal_pay = balance;
            }
            total_interest += interest;
            balance -= principal_pay;
            payoff_months += 1;
            if balance <= 1e-6 {
                break;
            }
        }
    }

    let months_f = payoff_months as f64;
    let total_tax = monthly_taxes * months_f;
    let total_insurance = monthly_insurance * months_f;
    let total_hoa = monthly_hoa * months_f;
    let monthly_payment = monthly_pi + monthly_taxes + monthly_insurance + monthly_hoa;
    let total_cost =
        down_payment + loan + total_interest + total_tax + total_insurance + total_hoa;

    let summary = format!(
        "{} home with {} down: {} loan at {}% over {} — {} /mo (P&I {}), {} total interest, {} total cost",
        money(home_price),
        money(down_payment),
        money(loan),
        trim(round_to(rate_pct, 4)),
        fmt_years(loan_years),
        money(monthly_payment),
        money(monthly_pi),
        money(total_interest),
        money(total_cost),
    );

    Ok(MortgageResult {
        loan_amount: round_to(loan, decimals),
        monthly_principal_interest: round_to(monthly_pi, decimals),
        monthly_taxes: round_to(monthly_taxes, decimals),
        monthly_insurance: round_to(monthly_insurance, decimals),
        monthly_hoa: round_to(monthly_hoa, decimals),
        monthly_payment: round_to(monthly_payment, decimals),
        payoff_months,
        total_principal: round_to(loan, decimals),
        total_interest: round_to(total_interest, decimals),
        total_tax: round_to(total_tax, decimals),
        total_insurance: round_to(total_insurance, decimals),
        total_hoa: round_to(total_hoa, decimals),
        total_cost: round_to(total_cost, decimals),
        summary,
    })
}

/// Render the term as "30 years" / "1 year".
fn fmt_years(years: f64) -> String {
    let y = trim(round_to(years, 2));
    if (years - 1.0).abs() < 1e-9 {
        format!("{y} year")
    } else {
        format!("{y} years")
    }
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
    fn classic_200k_30yr_6pct() {
        // Well-known: a 200,000 loan at 6% over 30 years = 1199.10 /mo P&I.
        let mut i = inp();
        i.home_price = Some(250_000.0);
        i.down_payment = Some(50_000.0);
        i.loan_years = Some(30.0);
        i.annual_interest_rate_percent = Some(6.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.loan_amount, 200_000.0);
        assert_eq!(r.monthly_principal_interest, 1199.10);
        assert_eq!(r.payoff_months, 360);
        // Total interest ≈ 1199.10*360 - 200000 ≈ 231,676.
        assert!(
            (231_670.0..=231_680.0).contains(&r.total_interest),
            "total_interest={}",
            r.total_interest
        );
        // No taxes/insurance/hoa → monthly_payment == P&I.
        assert_eq!(r.monthly_payment, r.monthly_principal_interest);
    }

    #[test]
    fn taxes_insurance_hoa_roll_into_monthly_payment() {
        let mut i = inp();
        i.home_price = Some(250_000.0);
        i.down_payment = Some(50_000.0);
        i.loan_years = Some(30.0);
        i.annual_interest_rate_percent = Some(6.0);
        i.annual_property_tax = Some(3600.0); // 300/mo
        i.annual_insurance = Some(1200.0); // 100/mo
        i.monthly_hoa = Some(150.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.monthly_taxes, 300.0);
        assert_eq!(r.monthly_insurance, 100.0);
        assert_eq!(r.monthly_hoa, 150.0);
        assert_eq!(
            r.monthly_payment,
            round_to(r.monthly_principal_interest + 550.0, 2)
        );
        assert_eq!(r.total_tax, 300.0 * 360.0);
        assert_eq!(r.total_insurance, 100.0 * 360.0);
        assert_eq!(r.total_hoa, 150.0 * 360.0);
    }

    #[test]
    fn zero_interest_is_straight_line() {
        // 120,000 over 10 years at 0% = 1000/mo, no interest.
        let mut i = inp();
        i.home_price = Some(120_000.0);
        i.down_payment = Some(0.0);
        i.loan_years = Some(10.0);
        i.annual_interest_rate_percent = Some(0.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.monthly_principal_interest, 1000.0);
        assert_eq!(r.total_interest, 0.0);
        assert_eq!(r.payoff_months, 120);
        assert_eq!(r.total_cost, 120_000.0);
    }

    #[test]
    fn extra_payment_at_zero_interest_halves_the_term() {
        // 12,000 over 10 years (120 mo) at 0% → 100/mo; +100 extra → 200/mo → 60 mo.
        let mut i = inp();
        i.home_price = Some(12_000.0);
        i.down_payment = Some(0.0);
        i.loan_years = Some(10.0);
        i.annual_interest_rate_percent = Some(0.0);
        i.extra_monthly_payment = Some(100.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.payoff_months, 60);
        assert_eq!(r.total_interest, 0.0);
        assert_eq!(r.total_principal, 12_000.0);
    }

    #[test]
    fn extra_payment_shortens_term_and_cuts_interest() {
        let mut base = inp();
        base.home_price = Some(300_000.0);
        base.down_payment = Some(60_000.0);
        base.loan_years = Some(30.0);
        base.annual_interest_rate_percent = Some(7.0);
        let no_extra = compute(&base).unwrap();

        let mut with = base.clone();
        with.extra_monthly_payment = Some(300.0);
        let extra = compute(&with).unwrap();

        assert!(extra.payoff_months < no_extra.payoff_months);
        assert!(extra.total_interest < no_extra.total_interest);
        assert_eq!(extra.total_principal, no_extra.total_principal);
    }

    #[test]
    fn defaults_apply_when_unset() {
        let r = compute(&inp()).unwrap();
        assert_eq!(r.loan_amount, 320_000.0); // 400k - 80k
        assert_eq!(r.payoff_months, 360);
        assert!(r.monthly_principal_interest > 0.0);
        assert!(r.summary.contains("total cost"));
    }

    #[test]
    fn cash_purchase_has_no_loan() {
        let mut i = inp();
        i.home_price = Some(200_000.0);
        i.down_payment = Some(200_000.0);
        i.loan_years = Some(30.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.loan_amount, 0.0);
        assert_eq!(r.monthly_principal_interest, 0.0);
        assert_eq!(r.payoff_months, 0);
        assert_eq!(r.total_interest, 0.0);
        assert_eq!(r.total_cost, 200_000.0);
    }

    #[test]
    fn decimals_control_rounding() {
        let mut i = inp();
        i.home_price = Some(250_000.0);
        i.down_payment = Some(50_000.0);
        i.annual_interest_rate_percent = Some(6.0);
        i.decimals = Some(0.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.monthly_principal_interest, 1199.0);
    }

    #[test]
    fn down_payment_above_price_errors() {
        let mut i = inp();
        i.home_price = Some(100_000.0);
        i.down_payment = Some(150_000.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("down_payment must not exceed"), "{err}");
    }

    #[test]
    fn zero_price_errors() {
        let mut i = inp();
        i.home_price = Some(0.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("home_price must be greater than zero"), "{err}");
    }

    #[test]
    fn zero_term_errors() {
        let mut i = inp();
        i.loan_years = Some(0.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("loan_years must be greater than zero"), "{err}");
    }

    #[test]
    fn negative_cost_errors() {
        let mut i = inp();
        i.annual_property_tax = Some(-100.0);
        let err = compute(&i).unwrap_err();
        assert!(
            err.contains("annual_property_tax must not be negative"),
            "{err}"
        );
    }

    #[test]
    fn over_max_term_errors() {
        let mut i = inp();
        i.loan_years = Some(101.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("at most"), "{err}");
    }

    #[test]
    fn nonfinite_price_errors() {
        let mut i = inp();
        i.home_price = Some(f64::NAN);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("finite"), "{err}");
    }

    #[test]
    fn json_round_trips() {
        let json = compute_json(&inp()).unwrap();
        assert!(json.contains("\"monthly_payment\""));
        assert!(json.contains("\"loan_amount\""));
        assert!(json.contains("\"total_cost\""));
        assert!(json.contains("\"payoff_months\""));
    }
}
