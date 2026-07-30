//! rent-vs-buy core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Answers "over the long run, am I better off renting or buying?" using the standard
//! **"invest the difference" net-worth race** that credible rent-vs-buy calculators run
//! — not a naive rent-vs-mortgage monthly comparison.
//!
//! The model, month by month over the chosen `years` horizon:
//!
//! ```text
//! Up front  buyer spends  down_payment + closing_costs; the renter INVESTS that same
//!           cash (opportunity cost). loan = home_price - down_payment.
//! Monthly   buyer outflow = P&I + property tax + insurance + maintenance + HOA
//!           (tax/insurance/maintenance are %/yr of the CURRENT home value ÷ 12);
//!           renter outflow = rent (grown once a year by rent_growth_percent).
//!           Whoever pays LESS invests the difference; both side-funds compound at
//!           investment_return_percent. The home value compounds at
//!           home_appreciation_percent; the mortgage amortises normally.
//! End       buyer net worth = home_value − selling costs − remaining loan + side-fund;
//!           renter net worth = side-fund. Buying "wins" when buyer ≥ renter.
//! ```
//!
//! `break_even_year` is the first whole year (1..=years) at which the buyer's net worth
//! — computed as if the home were sold that year — catches the renter's; `null`/`None`
//! when buying never gets ahead within the horizon. All math is `f64`, monthly
//! compounding; money outputs are rounded to the caller's `decimals` (default 0).

use serde::Serialize;

/// Hard cap on any single money input (dollars), guarding against overflow / typos.
pub const MAX_MONEY: f64 = 1_000_000_000.0;
/// Hard cap on the horizon / loan term in years.
pub const MAX_YEARS: f64 = 100.0;

/// A year-end snapshot of the net-worth race (money rounded to `decimals`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YearPoint {
    /// Year number (1-based) this snapshot is the end of.
    pub year: u32,
    /// Buyer net worth if the home were sold at the end of this year (money).
    pub buy_net_worth: f64,
    /// Renter net worth (invested side-fund) at the end of this year (money).
    pub rent_net_worth: f64,
}

/// Structured rent-vs-buy result. Every money field is rounded to `decimals`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RentVsBuyResult {
    /// Amount financed: `home_price − down_payment` (money).
    pub loan_amount: f64,
    /// Down payment: `home_price × down_payment_percent / 100` (money).
    pub down_payment: f64,
    /// Cash needed at closing: `down_payment + buying closing costs` (money).
    pub total_upfront_cost: f64,
    /// Initial fixed monthly principal + interest on the loan (money).
    pub monthly_principal_interest: f64,
    /// First-month total cost of owning (P&I + tax + insurance + maintenance + HOA) (money).
    pub first_month_buy_cost: f64,
    /// First-month cost of renting (money).
    pub first_month_rent_cost: f64,
    /// Buyer net worth at the end of the horizon (money).
    pub buy_net_worth: f64,
    /// Renter net worth at the end of the horizon (money).
    pub rent_net_worth: f64,
    /// `buy_net_worth − rent_net_worth`; positive means buying comes out ahead (money).
    pub net_worth_difference: f64,
    /// `"buy"`, `"rent"`, or `"even"` at the end of the horizon.
    pub verdict: String,
    /// First whole year buying gets ahead, or `null` if never within the horizon.
    pub break_even_year: Option<u32>,
    /// Home value at the end of the horizon after appreciation (money).
    pub home_value_at_horizon: f64,
    /// Remaining mortgage balance at the end of the horizon (money).
    pub remaining_mortgage_at_horizon: f64,
    /// Total rent paid across the whole horizon (money).
    pub total_rent_paid: f64,
    /// Year-by-year net-worth race, one entry per year of the horizon.
    pub yearly: Vec<YearPoint>,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// All inputs. Each field is `None` when unset; [`compute`] applies the documented
/// default for any `None`, so chat, CLI and page funnel through the same defaults +
/// validation.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Home purchase price. Default 400000.
    pub home_price: Option<f64>,
    /// Down payment as a percent of the price. Default 20.
    pub down_payment_percent: Option<f64>,
    /// Nominal annual mortgage interest rate (percent). Default 6.5.
    pub mortgage_rate_percent: Option<f64>,
    /// Mortgage term in years. Default 30.
    pub loan_term_years: Option<f64>,
    /// Current monthly rent for a comparable place. Default 2000.
    pub monthly_rent: Option<f64>,
    /// How many years you plan to stay (the comparison horizon). Default 10.
    pub years: Option<f64>,
    /// Annual home-value appreciation (percent). Default 3.
    pub home_appreciation_percent: Option<f64>,
    /// Annual rent growth (percent). Default 3.
    pub rent_growth_percent: Option<f64>,
    /// Annual after-tax investment return on invested cash (percent). Default 5.
    pub investment_return_percent: Option<f64>,
    /// Annual property tax as a percent of home value. Default 1.1.
    pub property_tax_percent: Option<f64>,
    /// Annual home insurance as a percent of home value. Default 0.5.
    pub home_insurance_percent: Option<f64>,
    /// Annual maintenance/repairs as a percent of home value. Default 1.
    pub maintenance_percent: Option<f64>,
    /// Monthly HOA / condo dues. Default 0.
    pub hoa_monthly: Option<f64>,
    /// Buying closing costs as a percent of the price. Default 3.
    pub buying_closing_percent: Option<f64>,
    /// Selling costs (agent + closing) as a percent of the sale price. Default 6.
    pub selling_cost_percent: Option<f64>,
    /// Currency symbol prefixed to amounts in the summary. Default `$`.
    pub currency: Option<String>,
    /// Decimal places for money outputs. Default 0.
    pub decimals: Option<f64>,
}

/// Round `x` to `decimals` places (half-away-from-zero, matching a currency display).
fn round_to(x: f64, decimals: u32) -> f64 {
    let f = 10f64.powi(decimals as i32);
    (x * f).round() / f
}

/// Format a money value with thousands separators and two decimals for the summary line.
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

/// Trim a trailing `.0` from a whole number for the summary (e.g. `10.0` → `10`).
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

/// Convert an annual percentage rate to an equivalent monthly compounding factor.
fn monthly_factor(annual_percent: f64) -> f64 {
    (1.0 + annual_percent / 100.0).powf(1.0 / 12.0)
}

/// Compute the rent-vs-buy result from the supplied inputs, applying defaults for any
/// unset field. Errors on non-finite/negative money, a non-positive price/term/horizon,
/// or an out-of-range percentage.
pub fn compute(i: &Inputs) -> Result<RentVsBuyResult, String> {
    let home_price = i.home_price.unwrap_or(400_000.0);
    let dp_pct = i.down_payment_percent.unwrap_or(20.0);
    let rate = i.mortgage_rate_percent.unwrap_or(6.5);
    let term_years = i.loan_term_years.unwrap_or(30.0);
    let monthly_rent = i.monthly_rent.unwrap_or(2_000.0);
    let horizon_years = i.years.unwrap_or(10.0);
    let appreciation = i.home_appreciation_percent.unwrap_or(3.0);
    let rent_growth = i.rent_growth_percent.unwrap_or(3.0);
    let invest_return = i.investment_return_percent.unwrap_or(5.0);
    let tax_pct = i.property_tax_percent.unwrap_or(1.1);
    let ins_pct = i.home_insurance_percent.unwrap_or(0.5);
    let maint_pct = i.maintenance_percent.unwrap_or(1.0);
    let hoa = i.hoa_monthly.unwrap_or(0.0);
    let closing_pct = i.buying_closing_percent.unwrap_or(3.0);
    let selling_pct = i.selling_cost_percent.unwrap_or(6.0);
    let currency = i.currency.clone().unwrap_or_else(|| "$".to_string());
    let decimals_f = i.decimals.unwrap_or(0.0);

    require_nonneg("home_price", home_price)?;
    require_nonneg("monthly_rent", monthly_rent)?;
    require_nonneg("hoa_monthly", hoa)?;
    for (label, v) in [
        ("down_payment_percent", dp_pct),
        ("mortgage_rate_percent", rate),
        ("loan_term_years", term_years),
        ("years", horizon_years),
        ("home_appreciation_percent", appreciation),
        ("rent_growth_percent", rent_growth),
        ("investment_return_percent", invest_return),
        ("property_tax_percent", tax_pct),
        ("home_insurance_percent", ins_pct),
        ("maintenance_percent", maint_pct),
        ("buying_closing_percent", closing_pct),
        ("selling_cost_percent", selling_pct),
        ("decimals", decimals_f),
    ] {
        require_finite(label, v)?;
    }

    if home_price <= 0.0 {
        return Err("home_price must be greater than zero".into());
    }
    if !(0.0..=100.0).contains(&dp_pct) {
        return Err("down_payment_percent must be between 0 and 100".into());
    }
    if term_years <= 0.0 || term_years > MAX_YEARS {
        return Err(format!(
            "loan_term_years must be between 0 (exclusive) and {}",
            trim(MAX_YEARS)
        ));
    }
    if horizon_years <= 0.0 || horizon_years > MAX_YEARS {
        return Err(format!(
            "years must be between 0 (exclusive) and {}",
            trim(MAX_YEARS)
        ));
    }
    if rate < 0.0 {
        return Err("mortgage_rate_percent must not be negative".into());
    }
    for (label, v) in [
        ("home_appreciation_percent", appreciation),
        ("rent_growth_percent", rent_growth),
        ("investment_return_percent", invest_return),
        ("property_tax_percent", tax_pct),
        ("home_insurance_percent", ins_pct),
        ("maintenance_percent", maint_pct),
        ("selling_cost_percent", selling_pct),
        ("buying_closing_percent", closing_pct),
    ] {
        if v < -100.0 {
            return Err(format!("{label} must not be below -100 (percent)"));
        }
    }
    if !(0.0..=10.0).contains(&decimals_f) {
        return Err("decimals must be between 0 and 10".into());
    }
    let decimals = decimals_f as u32;

    let down_payment = home_price * dp_pct / 100.0;
    let loan_amount = (home_price - down_payment).max(0.0);
    let closing_costs = home_price * closing_pct / 100.0;
    let total_upfront = down_payment + closing_costs;

    // Fixed monthly P&I on the loan.
    let n_payments = (term_years * 12.0).round() as u64;
    let i_m = rate / 100.0 / 12.0;
    let monthly_pi = if loan_amount <= 0.0 {
        0.0
    } else if i_m <= 0.0 {
        loan_amount / n_payments as f64
    } else {
        loan_amount * i_m / (1.0 - (1.0 + i_m).powi(-(n_payments as i32)))
    };

    // Monthly compounding factors.
    let appr_m = monthly_factor(appreciation);
    let invest_m = monthly_factor(invest_return);

    let horizon_months = (horizon_years * 12.0).round() as u64;

    // State.
    let mut balance = loan_amount; // mortgage balance
    let mut home_value = home_price;
    // The renter starts by investing the buyer's up-front cash (opportunity cost).
    let mut renter_fund = total_upfront;
    let mut buyer_fund = 0.0_f64;
    let mut total_rent_paid = 0.0_f64;

    let mut first_month_buy_cost = 0.0_f64;
    let mut first_month_rent_cost = 0.0_f64;
    let mut break_even_year: Option<u32> = None;
    let mut yearly: Vec<YearPoint> = Vec::new();

    for m in 1..=horizon_months {
        // Rent for the current year (grows once per completed 12 months).
        let year_index = (m - 1) / 12; // 0-based year
        let rent_this_month = monthly_rent * (1.0 + rent_growth / 100.0).powi(year_index as i32);

        // Owner monthly costs based on the CURRENT home value.
        let pi = if balance > 0.0 { monthly_pi } else { 0.0 };
        let tax = home_value * tax_pct / 100.0 / 12.0;
        let ins = home_value * ins_pct / 100.0 / 12.0;
        let maint = home_value * maint_pct / 100.0 / 12.0;
        let buy_cost = pi + tax + ins + maint + hoa;

        if m == 1 {
            first_month_buy_cost = buy_cost;
            first_month_rent_cost = rent_this_month;
        }
        total_rent_paid += rent_this_month;

        // Amortise the mortgage for this month.
        if balance > 0.0 {
            let interest = balance * i_m;
            let principal = (monthly_pi - interest).min(balance);
            balance = (balance - principal).max(0.0);
        }

        // Whoever pays less invests the difference; both funds compound first.
        buyer_fund *= invest_m;
        renter_fund *= invest_m;
        if buy_cost > rent_this_month {
            renter_fund += buy_cost - rent_this_month;
        } else {
            buyer_fund += rent_this_month - buy_cost;
        }

        // Home appreciates.
        home_value *= appr_m;

        // Year-end snapshot + break-even check (as if sold at this year end).
        if m % 12 == 0 || m == horizon_months {
            let sell_costs = home_value * selling_pct / 100.0;
            let buy_nw = home_value - sell_costs - balance + buyer_fund;
            let rent_nw = renter_fund;
            let ynum = ((m + 11) / 12) as u32;
            if break_even_year.is_none() && buy_nw >= rent_nw {
                break_even_year = Some(ynum);
            }
            yearly.push(YearPoint {
                year: ynum,
                buy_net_worth: round_to(buy_nw, decimals),
                rent_net_worth: round_to(rent_nw, decimals),
            });
        }
    }

    let sell_costs = home_value * selling_pct / 100.0;
    let buy_net_worth = home_value - sell_costs - balance + buyer_fund;
    let rent_net_worth = renter_fund;
    let diff = buy_net_worth - rent_net_worth;

    // "Even" tolerance: half a currency unit at the requested precision.
    let tol = 0.5 * 10f64.powi(-(decimals as i32));
    let verdict = if diff.abs() < tol {
        "even"
    } else if diff > 0.0 {
        "buy"
    } else {
        "rent"
    };

    let be_note = match break_even_year {
        Some(y) if (y as f64) <= horizon_years => {
            format!(" Buying pulls ahead around year {y}.")
        }
        _ => format!(
            " Buying does not catch renting within {} years.",
            trim(horizon_years)
        ),
    };
    let verdict_phrase = match verdict {
        "buy" => format!(
            "buying wins by {cur}{d}",
            cur = currency,
            d = money(diff.abs())
        ),
        "rent" => format!(
            "renting wins by {cur}{d}",
            cur = currency,
            d = money(diff.abs())
        ),
        _ => "it is essentially a wash".to_string(),
    };
    let summary = format!(
        "Over {yrs} years, {verdict}: buying leaves you with {cur}{b} vs {cur}{r} renting (invest-the-difference net worth).{be} Assumes {appr}%/yr home appreciation, {inv}%/yr investment return and {rg}%/yr rent growth — a planning estimate, not advice.",
        yrs = trim(horizon_years),
        verdict = verdict_phrase,
        cur = currency,
        b = money(buy_net_worth),
        r = money(rent_net_worth),
        be = be_note,
        appr = trim(round_to(appreciation, 4)),
        inv = trim(round_to(invest_return, 4)),
        rg = trim(round_to(rent_growth, 4)),
    );

    Ok(RentVsBuyResult {
        loan_amount: round_to(loan_amount, decimals),
        down_payment: round_to(down_payment, decimals),
        total_upfront_cost: round_to(total_upfront, decimals),
        monthly_principal_interest: round_to(monthly_pi, decimals),
        first_month_buy_cost: round_to(first_month_buy_cost, decimals),
        first_month_rent_cost: round_to(first_month_rent_cost, decimals),
        buy_net_worth: round_to(buy_net_worth, decimals),
        rent_net_worth: round_to(rent_net_worth, decimals),
        net_worth_difference: round_to(diff, decimals),
        verdict: verdict.to_string(),
        break_even_year,
        home_value_at_horizon: round_to(home_value, decimals),
        remaining_mortgage_at_horizon: round_to(balance, decimals),
        total_rent_paid: round_to(total_rent_paid, decimals),
        yearly,
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
    fn default_scenario_computes_expected_fixed_figures() {
        let r = compute(&inp()).unwrap();
        // Loan math is closed-form and exact.
        assert_eq!(r.down_payment, 80_000.0); // 20% of 400k
        assert_eq!(r.loan_amount, 320_000.0);
        assert_eq!(r.total_upfront_cost, 92_000.0); // + 3% closing = 12k
        // 320k, 6.5%/30yr → ~2022.63/mo P&I, rounds to 2023 at decimals=0.
        assert_eq!(r.monthly_principal_interest, 2_023.0);
        assert_eq!(r.yearly.len(), 10);
        assert_eq!(r.yearly.last().unwrap().year, 10);
        // Verdict is one of the three known strings.
        assert!(matches!(r.verdict.as_str(), "buy" | "rent" | "even"));
        assert!(r.summary.contains("Over 10 years"));
    }

    #[test]
    fn short_horizon_favours_renting() {
        // Staying only 3 years: high transaction costs make buying lose.
        let mut i = inp();
        i.years = Some(3.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.verdict, "rent");
        assert!(r.net_worth_difference < 0.0);
        assert!(r.yearly.len() == 3);
    }

    #[test]
    fn long_horizon_favours_buying() {
        // Staying 20 years lets appreciation + equity overtake renting.
        let mut i = inp();
        i.years = Some(20.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.verdict, "buy");
        assert!(r.net_worth_difference > 0.0);
        assert!(r.break_even_year.is_some());
        let be = r.break_even_year.unwrap();
        assert!(be >= 1 && be <= 20);
    }

    #[test]
    fn high_investment_return_favours_renting() {
        // A 12%/yr return compounds the down payment faster than the home appreciates.
        let mut i = inp();
        i.years = Some(15.0);
        i.investment_return_percent = Some(12.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.verdict, "rent");
    }

    #[test]
    fn zero_rate_uses_straight_line_pi() {
        let mut i = inp();
        i.mortgage_rate_percent = Some(0.0);
        let r = compute(&i).unwrap();
        // 320000 / 360 = 888.888…  → 889 at decimals=0.
        assert_eq!(r.monthly_principal_interest, 889.0);
    }

    #[test]
    fn full_cash_purchase_has_no_loan() {
        let mut i = inp();
        i.down_payment_percent = Some(100.0);
        let r = compute(&i).unwrap();
        assert_eq!(r.loan_amount, 0.0);
        assert_eq!(r.monthly_principal_interest, 0.0);
        assert_eq!(r.remaining_mortgage_at_horizon, 0.0);
    }

    #[test]
    fn currency_and_decimals_apply() {
        let mut i = inp();
        i.currency = Some("£".into());
        i.decimals = Some(2.0);
        let r = compute(&i).unwrap();
        assert!(r.summary.contains("£"), "{}", r.summary);
        // decimals=2 keeps cents on the P&I.
        assert!((r.monthly_principal_interest - 2_022.62).abs() < 0.02);
    }

    #[test]
    fn zero_home_price_errors() {
        let mut i = inp();
        i.home_price = Some(0.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("home_price must be greater than zero"), "{err}");
    }

    #[test]
    fn negative_home_price_errors() {
        let mut i = inp();
        i.home_price = Some(-5.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("must not be negative"), "{err}");
    }

    #[test]
    fn down_payment_out_of_range_errors() {
        let mut i = inp();
        i.down_payment_percent = Some(150.0);
        let err = compute(&i).unwrap_err();
        assert!(
            err.contains("down_payment_percent must be between 0 and 100"),
            "{err}"
        );
    }

    #[test]
    fn zero_years_errors() {
        let mut i = inp();
        i.years = Some(0.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("years must be between"), "{err}");
    }

    #[test]
    fn decimals_out_of_range_errors() {
        let mut i = inp();
        i.decimals = Some(11.0);
        let err = compute(&i).unwrap_err();
        assert!(err.contains("decimals must be between 0 and 10"), "{err}");
    }

    #[test]
    fn json_round_trips() {
        let json = compute_json(&inp()).unwrap();
        assert!(json.contains("\"break_even_year\""));
        assert!(json.contains("\"buy_net_worth\""));
        assert!(json.contains("\"yearly\""));
        assert!(json.contains("\"summary\""));
    }
}
