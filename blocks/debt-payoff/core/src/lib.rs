//! debt-payoff core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Simulates a debt-payoff plan from a list of debts (name, balance, APR%,
//! minimum payment), a payoff **method** (snowball = smallest balance first,
//! avalanche = highest APR first), a constant **extra monthly payment**, and a
//! **start date**. It uses the standard *rollover* (a.k.a. cascade) method: the
//! total monthly budget stays constant (sum of every debt's minimum + the extra),
//! and as each debt is cleared its freed-up minimum rolls onto the next
//! highest-priority debt.
//!
//! All money is tracked in integer **cents** so the simulation is fully
//! deterministic across surfaces (CLI, page, chat). Interest is compounded
//! monthly and rounded to the cent each month.

use chrono::{Datelike, NaiveDate, NaiveDateTime};
use serde::Serialize;

/// Hard caps (kept in sync with the page copy and competitor analysis).
const MAX_DEBTS: usize = 50;
/// Simulation cap: if a plan can't clear inside this many months it's treated as
/// impossible and reported as an actionable error rather than looping forever.
const MAX_MONTHS: i64 = 1200; // 100 years
const MAX_APR: f64 = 100.0;
/// Per-field money ceiling (dollars) — guards against overflow / typos.
const MAX_MONEY: f64 = 100_000_000.0;

/// One parsed debt.
#[derive(Debug, Clone)]
struct Debt {
    name: String,
    balance_cents: i64,
    apr: f64,
    min_cents: i64,
}

impl Debt {
    fn monthly_rate(&self) -> f64 {
        self.apr / 100.0 / 12.0
    }
}

/// Per-debt result inside the chosen plan, in payoff order.
#[derive(Debug, Clone, Serialize)]
pub struct DebtPayoff {
    /// 1-based position in the payoff order.
    pub order: usize,
    pub name: String,
    pub original_balance: f64,
    pub apr: f64,
    pub minimum_payment: f64,
    /// Total interest this debt accrues before it is cleared.
    pub interest_paid: f64,
    /// Total money paid toward this debt (principal + interest).
    pub total_paid: f64,
    /// Number of months from the start until this debt hits zero.
    pub months_to_payoff: i64,
    /// Calendar date this debt is cleared (`YYYY-MM-DD`).
    pub payoff_date: String,
}

/// One method's headline totals (used for the chosen plan and the comparison).
#[derive(Debug, Clone, Serialize)]
pub struct MethodSummary {
    pub method: String,
    pub months: i64,
    pub debt_free_date: String,
    pub total_interest: f64,
    pub total_paid: f64,
}

/// Minimum-only baseline (no extra, no rollover — each debt paid on its own
/// minimum until cleared). May be infeasible when a minimum doesn't cover its
/// interest.
#[derive(Debug, Clone, Serialize)]
pub struct Baseline {
    pub feasible: bool,
    pub months: i64,
    pub debt_free_date: String,
    pub total_interest: f64,
    pub total_paid: f64,
    pub note: String,
}

/// Snowball-vs-avalanche side-by-side.
#[derive(Debug, Clone, Serialize)]
pub struct Comparison {
    pub snowball: MethodSummary,
    pub avalanche: MethodSummary,
    /// The method with the lower total interest (ties → fewer months → snowball).
    pub recommended: String,
    /// Extra interest the non-recommended method would cost (≥ 0).
    pub interest_difference: f64,
    /// Month difference between the two methods (absolute value).
    pub months_difference: i64,
}

/// Full structured plan.
#[derive(Debug, Clone, Serialize)]
pub struct PlanResult {
    /// The method actually used for this plan (`snowball` | `avalanche`).
    pub method: String,
    /// Human-readable method label.
    pub method_label: String,
    pub start_date: String,
    pub monthly_budget: f64,
    pub extra_payment: f64,
    pub months: i64,
    pub debt_free_date: String,
    pub total_paid: f64,
    pub total_interest: f64,
    pub total_principal: f64,
    /// Debts in the order they are paid off under the chosen plan.
    pub payoff_order: Vec<DebtPayoff>,
    pub minimum_only: Baseline,
    /// Interest saved vs. the minimum-only baseline (null if baseline infeasible).
    pub interest_saved_vs_minimum: Option<f64>,
    /// Months saved vs. the minimum-only baseline (null if baseline infeasible).
    pub months_saved_vs_minimum: Option<i64>,
    pub comparison: Comparison,
    pub summary: String,
}

fn cents(dollars: f64) -> i64 {
    (dollars * 100.0).round() as i64
}
fn dollars(c: i64) -> f64 {
    c as f64 / 100.0
}
fn usd(c: i64) -> String {
    format!("${:.2}", dollars(c))
}

/// Parse a flexible date string into a naive date. Accepts `YYYY-MM-DD`,
/// `YYYY/MM/DD`, `MM/DD/YYYY`, `DD.MM.YYYY`, and datetime forms (time dropped).
fn parse_date(s: &str) -> Result<NaiveDate, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("empty date".into());
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(t) {
        return Ok(dt.naive_local().date());
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(t, fmt) {
            return Ok(dt.date());
        }
    }
    for fmt in ["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y", "%d.%m.%Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(t, fmt) {
            return Ok(d);
        }
    }
    Err(format!(
        "could not parse start date '{}' — use YYYY-MM-DD (e.g. 2026-01-01)",
        t
    ))
}

/// Last day (28–31) of the given month.
fn last_day_of_month(year: i32, month: u32) -> u32 {
    for day in (28u32..=31).rev() {
        if NaiveDate::from_ymd_opt(year, month, day).is_some() {
            return day;
        }
    }
    28
}

/// Add `months` whole calendar months to `d`, clamping the day to the target
/// month's length (e.g. Jan 31 + 1 month → Feb 28/29).
fn add_months(d: NaiveDate, months: i64) -> NaiveDate {
    let total = d.year() as i64 * 12 + (d.month() as i64 - 1) + months;
    let year = total.div_euclid(12) as i32;
    let month = total.rem_euclid(12) as u32 + 1;
    let day = d.day().min(last_day_of_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

/// Strip `$`, `%`, spaces and tabs, then parse a non-negative money/percent value.
fn parse_money(raw: &str, label: &str, line_no: usize) -> Result<f64, String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| !matches!(c, '$' | '%' | ' ' | '\t'))
        .collect();
    if cleaned.is_empty() {
        return Err(format!("line {}: missing {}", line_no, label));
    }
    let v: f64 = cleaned.parse().map_err(|_| {
        format!(
            "line {}: {} '{}' is not a number (use plain digits like 2500.00; no thousands separators)",
            line_no, label, raw.trim()
        )
    })?;
    if !v.is_finite() || v < 0.0 {
        return Err(format!("line {}: {} must be zero or positive", line_no, label));
    }
    Ok(v)
}

/// Parse the multiline `debts` field: one debt per line as
/// `name, balance, APR%, minimum payment`.
fn parse_debts(input: &str) -> Result<Vec<Debt>, String> {
    let mut debts = Vec::new();
    for (i, raw_line) in input.lines().enumerate() {
        let line_no = i + 1;
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 4 {
            return Err(format!(
                "line {}: expected 4 comma-separated fields (name, balance, APR%, minimum payment) but found {} — e.g. \"Visa, 2500, 19.99, 75\"",
                line_no,
                parts.len()
            ));
        }
        let name = {
            let n = parts[0].trim();
            if n.is_empty() {
                format!("Debt {}", debts.len() + 1)
            } else {
                n.to_string()
            }
        };
        let balance = parse_money(parts[1], "balance", line_no)?;
        let apr = parse_money(parts[2], "APR", line_no)?;
        let min = parse_money(parts[3], "minimum payment", line_no)?;

        if balance <= 0.0 {
            return Err(format!("line {}: balance must be greater than zero", line_no));
        }
        if balance > MAX_MONEY || min > MAX_MONEY {
            return Err(format!(
                "line {}: amounts must be under {}",
                line_no,
                usd(cents(MAX_MONEY))
            ));
        }
        if apr > MAX_APR {
            return Err(format!(
                "line {}: APR {}% looks too high (max {}%)",
                line_no, apr, MAX_APR
            ));
        }
        debts.push(Debt {
            name,
            balance_cents: cents(balance),
            apr,
            min_cents: cents(min),
        });
    }
    if debts.is_empty() {
        return Err("no debts entered — add one debt per line as: name, balance, APR%, minimum payment".into());
    }
    if debts.len() > MAX_DEBTS {
        return Err(format!(
            "too many debts ({}) — this planner supports up to {}",
            debts.len(),
            MAX_DEBTS
        ));
    }
    Ok(debts)
}

/// Priority order (indices into `debts`) for a method. Ties break by input order.
fn priority_order(debts: &[Debt], method: &str) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..debts.len()).collect();
    match method {
        "avalanche" => idx.sort_by(|&a, &b| {
            debts[b]
                .apr
                .partial_cmp(&debts[a].apr)
                .unwrap()
                .then(a.cmp(&b))
        }),
        _ => idx.sort_by(|&a, &b| {
            debts[a]
                .balance_cents
                .cmp(&debts[b].balance_cents)
                .then(a.cmp(&b))
        }),
    }
    idx
}

/// Outcome of one accelerated (rollover) simulation.
struct Sim {
    months: i64,
    total_interest: i64,
    total_paid: i64,
    per_debt_interest: Vec<i64>,
    per_debt_paid: Vec<i64>,
    payoff_month: Vec<i64>,
}

/// Run the rollover simulation: constant `budget_cents` per month, minimums paid
/// first, then any remaining budget cascades down `priority`.
fn simulate(debts: &[Debt], priority: &[usize], budget_cents: i64) -> Result<Sim, String> {
    let n = debts.len();
    let mut bal: Vec<i64> = debts.iter().map(|d| d.balance_cents).collect();
    let mut per_debt_interest = vec![0i64; n];
    let mut per_debt_paid = vec![0i64; n];
    let mut payoff_month = vec![0i64; n];
    let mut total_interest = 0i64;
    let mut month = 0i64;

    loop {
        if bal.iter().all(|&b| b <= 0) {
            break;
        }
        if month >= MAX_MONTHS {
            return Err(format!(
                "these debts can't be paid off within {} months with this budget — increase the minimum payments or the extra monthly payment",
                MAX_MONTHS
            ));
        }
        month += 1;

        // Accrue one month of interest on every active debt.
        for i in 0..n {
            if bal[i] > 0 {
                let interest = (bal[i] as f64 * debts[i].monthly_rate()).round() as i64;
                bal[i] += interest;
                per_debt_interest[i] += interest;
                total_interest += interest;
            }
        }

        // Pay each active debt its minimum (capped at balance), in input order.
        let mut pool = budget_cents;
        for i in 0..n {
            if bal[i] > 0 && pool > 0 {
                let pay = debts[i].min_cents.min(bal[i]).min(pool);
                bal[i] -= pay;
                pool -= pay;
                per_debt_paid[i] += pay;
            }
        }

        // Cascade any remaining budget down the priority order.
        for &i in priority {
            if pool <= 0 {
                break;
            }
            if bal[i] > 0 {
                let pay = bal[i].min(pool);
                bal[i] -= pay;
                pool -= pay;
                per_debt_paid[i] += pay;
            }
        }

        for i in 0..n {
            if bal[i] <= 0 && payoff_month[i] == 0 {
                payoff_month[i] = month;
            }
        }
    }

    let total_paid: i64 = per_debt_paid.iter().sum();
    Ok(Sim {
        months: month,
        total_interest,
        total_paid,
        per_debt_interest,
        per_debt_paid,
        payoff_month,
    })
}

/// Minimum-only baseline: each debt on its own minimum, no rollover, no extra.
/// Returns `None` (infeasible) if any minimum never clears its balance.
fn simulate_min_only(debts: &[Debt]) -> Option<(i64, i64, i64)> {
    let mut max_months = 0i64;
    let mut total_interest = 0i64;
    let mut total_paid = 0i64;
    for d in debts {
        let mut bal = d.balance_cents;
        let mut month = 0i64;
        loop {
            if bal <= 0 {
                break;
            }
            if month >= MAX_MONTHS {
                return None;
            }
            let before = bal;
            month += 1;
            let interest = (bal as f64 * d.monthly_rate()).round() as i64;
            bal += interest;
            total_interest += interest;
            let pay = d.min_cents.min(bal);
            bal -= pay;
            total_paid += pay;
            if bal >= before {
                // Balance didn't shrink this month → will never be paid off.
                return None;
            }
        }
        max_months = max_months.max(month);
    }
    Some((max_months, total_interest, total_paid))
}

fn method_label(method: &str) -> &'static str {
    match method {
        "avalanche" => "Debt Avalanche (highest APR first)",
        _ => "Debt Snowball (smallest balance first)",
    }
}

fn summarize(debts: &[Debt], method: &str, sim: &Sim, start: NaiveDate) -> MethodSummary {
    let _ = debts;
    MethodSummary {
        method: method.to_string(),
        months: sim.months,
        debt_free_date: add_months(start, sim.months).to_string(),
        total_interest: dollars(sim.total_interest),
        total_paid: dollars(sim.total_paid),
    }
}

/// Compute the full structured plan. `today` supplies the default start date when
/// `start_date` is blank (each surface passes its own clock).
pub fn plan(
    debts_input: &str,
    method_input: &str,
    extra_payment: f64,
    start_date_input: &str,
    today: NaiveDate,
) -> Result<PlanResult, String> {
    // Method.
    let method = match method_input.trim().to_ascii_lowercase().as_str() {
        "" | "snowball" => "snowball",
        "avalanche" => "avalanche",
        other => {
            return Err(format!(
                "unknown method '{}' — use 'snowball' or 'avalanche'",
                other
            ))
        }
    };

    // Extra payment.
    if !extra_payment.is_finite() || extra_payment < 0.0 {
        return Err("extra monthly payment must be zero or positive".into());
    }
    if extra_payment > MAX_MONEY {
        return Err(format!("extra monthly payment must be under {}", usd(cents(MAX_MONEY))));
    }
    let extra_cents = cents(extra_payment);

    // Start date.
    let start = if start_date_input.trim().is_empty() {
        today
    } else {
        parse_date(start_date_input)?
    };

    // Debts.
    let debts = parse_debts(debts_input)?;

    let min_sum: i64 = debts.iter().map(|d| d.min_cents).sum();
    let budget_cents = min_sum + extra_cents;

    // Feasibility: the monthly budget must beat the first month's interest.
    let first_interest: i64 = debts
        .iter()
        .map(|d| (d.balance_cents as f64 * d.monthly_rate()).round() as i64)
        .sum();
    if budget_cents <= first_interest {
        return Err(format!(
            "the monthly budget of {} (total minimums{}) doesn't cover the {} of interest accruing in the first month, so the balances would never be paid off — raise the minimum payments or add an extra monthly payment",
            usd(budget_cents),
            if extra_cents > 0 {
                format!(" + {} extra", usd(extra_cents))
            } else {
                String::new()
            },
            usd(first_interest),
        ));
    }

    // Simulate both methods (accelerated) + the chosen one.
    let snow_order = priority_order(&debts, "snowball");
    let aval_order = priority_order(&debts, "avalanche");
    let snow_sim = simulate(&debts, &snow_order, budget_cents)?;
    let aval_sim = simulate(&debts, &aval_order, budget_cents)?;

    let snow_summary = summarize(&debts, "snowball", &snow_sim, start);
    let aval_summary = summarize(&debts, "avalanche", &aval_sim, start);

    // Recommendation: lower interest wins; tie → fewer months → snowball.
    let recommended = if aval_sim.total_interest < snow_sim.total_interest
        || (aval_sim.total_interest == snow_sim.total_interest && aval_sim.months < snow_sim.months)
    {
        "avalanche"
    } else {
        "snowball"
    };
    let comparison = Comparison {
        interest_difference: dollars((snow_sim.total_interest - aval_sim.total_interest).abs()),
        months_difference: (snow_sim.months - aval_sim.months).abs(),
        recommended: recommended.to_string(),
        snowball: snow_summary.clone(),
        avalanche: aval_summary.clone(),
    };

    // Chosen plan.
    let (chosen_sim, chosen_order) = if method == "avalanche" {
        (&aval_sim, &aval_order)
    } else {
        (&snow_sim, &snow_order)
    };

    // Payoff order: sort by payoff month, ties by priority position.
    let prio_pos: Vec<usize> = {
        let mut pos = vec![0usize; debts.len()];
        for (rank, &i) in chosen_order.iter().enumerate() {
            pos[i] = rank;
        }
        pos
    };
    let mut order_idx: Vec<usize> = (0..debts.len()).collect();
    order_idx.sort_by(|&a, &b| {
        chosen_sim.payoff_month[a]
            .cmp(&chosen_sim.payoff_month[b])
            .then(prio_pos[a].cmp(&prio_pos[b]))
    });
    let payoff_order: Vec<DebtPayoff> = order_idx
        .iter()
        .enumerate()
        .map(|(rank, &i)| DebtPayoff {
            order: rank + 1,
            name: debts[i].name.clone(),
            original_balance: dollars(debts[i].balance_cents),
            apr: debts[i].apr,
            minimum_payment: dollars(debts[i].min_cents),
            interest_paid: dollars(chosen_sim.per_debt_interest[i]),
            total_paid: dollars(chosen_sim.per_debt_paid[i]),
            months_to_payoff: chosen_sim.payoff_month[i],
            payoff_date: add_months(start, chosen_sim.payoff_month[i]).to_string(),
        })
        .collect();

    // Minimum-only baseline + savings.
    let (baseline, interest_saved, months_saved) = match simulate_min_only(&debts) {
        Some((m_months, m_interest, m_paid)) => {
            let baseline = Baseline {
                feasible: true,
                months: m_months,
                debt_free_date: add_months(start, m_months).to_string(),
                total_interest: dollars(m_interest),
                total_paid: dollars(m_paid),
                note: String::new(),
            };
            (
                baseline,
                Some(dollars(m_interest - chosen_sim.total_interest)),
                Some(m_months - chosen_sim.months),
            )
        }
        None => (
            Baseline {
                feasible: false,
                months: 0,
                debt_free_date: String::new(),
                total_interest: 0.0,
                total_paid: 0.0,
                note: "At least one debt's minimum payment doesn't cover its interest, so minimum-only payments would never clear the balances.".into(),
            },
            None,
            None,
        ),
    };

    let debt_free_date = add_months(start, chosen_sim.months);
    let total_principal: i64 = debts.iter().map(|d| d.balance_cents).sum();

    // Summary sentence.
    let extra_phrase = if extra_cents > 0 {
        format!(" with {}/mo extra", usd(extra_cents))
    } else {
        String::new()
    };
    let savings_phrase = match (interest_saved, months_saved) {
        (Some(int_saved), Some(mo_saved)) => format!(
            " That's {} less interest and {} months sooner than minimum-only payments.",
            usd(cents(int_saved)),
            mo_saved
        ),
        _ => " Minimum-only payments would never clear these balances.".to_string(),
    };
    let compare_phrase = if snow_sim.total_interest == aval_sim.total_interest {
        " Snowball and avalanche cost the same here.".to_string()
    } else {
        format!(
            " The {} method saves {} in interest versus the other.",
            method_label(recommended).split(" (").next().unwrap_or(recommended),
            usd((snow_sim.total_interest - aval_sim.total_interest).abs())
        )
    };
    let summary = format!(
        "Using the {}{}, you'll be debt-free in {} months (by {}), paying {} in interest on {} of debt.{}{}",
        method_label(method).split(" (").next().unwrap_or(method),
        extra_phrase,
        chosen_sim.months,
        debt_free_date,
        usd(chosen_sim.total_interest),
        usd(total_principal),
        savings_phrase,
        compare_phrase,
    );

    Ok(PlanResult {
        method: method.to_string(),
        method_label: method_label(method).to_string(),
        start_date: start.to_string(),
        monthly_budget: dollars(budget_cents),
        extra_payment: dollars(extra_cents),
        months: chosen_sim.months,
        debt_free_date: debt_free_date.to_string(),
        total_paid: dollars(chosen_sim.total_paid),
        total_interest: dollars(chosen_sim.total_interest),
        total_principal: dollars(total_principal),
        payoff_order,
        minimum_only: baseline,
        interest_saved_vs_minimum: interest_saved,
        months_saved_vs_minimum: months_saved,
        comparison,
        summary,
    })
}

/// Convenience wrapper returning pretty-printed JSON (used by the web page + CLI
/// exact-output tests).
pub fn plan_json(
    debts_input: &str,
    method_input: &str,
    extra_payment: f64,
    start_date_input: &str,
    today: NaiveDate,
) -> Result<String, String> {
    let plan = plan(debts_input, method_input, extra_payment, start_date_input, today)?;
    serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn snowball_happy_path_clears_all_debts() {
        let input = "Visa, 2500, 19.99, 75\nCar Loan, 8000, 6.5, 200\nStore Card, 600, 24.99, 25";
        let p = plan(input, "snowball", 300.0, "2026-01-01", d(2026, 1, 1)).unwrap();
        assert_eq!(p.method, "snowball");
        assert_eq!(p.payoff_order.len(), 3);
        // Snowball pays the smallest balance (Store Card) first.
        assert_eq!(p.payoff_order[0].name, "Store Card");
        assert_eq!(p.payoff_order[2].name, "Car Loan");
        // Payoff months are non-decreasing.
        assert!(p.payoff_order[0].months_to_payoff <= p.payoff_order[1].months_to_payoff);
        assert!(p.payoff_order[1].months_to_payoff <= p.payoff_order[2].months_to_payoff);
        // Debt-free date == start + total months.
        assert_eq!(p.debt_free_date, add_months(d(2026, 1, 1), p.months).to_string());
        // Money identity: total paid == principal + interest.
        let sum = (p.total_principal * 100.0).round() + (p.total_interest * 100.0).round();
        assert_eq!((p.total_paid * 100.0).round(), sum);
    }

    #[test]
    fn avalanche_targets_highest_apr_first() {
        let input = "Visa, 2500, 19.99, 75\nCar Loan, 8000, 6.5, 200\nStore Card, 600, 24.99, 25";
        let p = plan(input, "avalanche", 300.0, "2026-01-01", d(2026, 1, 1)).unwrap();
        assert_eq!(p.method, "avalanche");
        // Highest APR (Store Card 24.99) is paid first.
        assert_eq!(p.payoff_order[0].name, "Store Card");
        // Avalanche never costs more interest than snowball.
        assert!(p.total_interest <= p.comparison.snowball.total_interest + 1e-9);
    }

    #[test]
    fn comparison_recommends_avalanche_when_cheaper() {
        let input = "A, 1000, 30, 30\nB, 1000, 5, 30";
        let p = plan(input, "snowball", 200.0, "2026-01-01", d(2026, 1, 1)).unwrap();
        // Avalanche should cost no more interest than snowball.
        assert!(p.comparison.avalanche.total_interest <= p.comparison.snowball.total_interest);
        assert!(p.comparison.interest_difference >= 0.0);
    }

    #[test]
    fn minimum_only_baseline_and_savings() {
        let input = "Visa, 2500, 19.99, 75\nStore Card, 600, 24.99, 25";
        let p = plan(input, "snowball", 200.0, "2026-01-01", d(2026, 1, 1)).unwrap();
        assert!(p.minimum_only.feasible);
        // Accelerated plan is faster + cheaper than the baseline.
        assert!(p.months < p.minimum_only.months);
        assert!(p.interest_saved_vs_minimum.unwrap() > 0.0);
        assert!(p.months_saved_vs_minimum.unwrap() > 0);
    }

    #[test]
    fn minimum_below_interest_is_flagged_baseline_infeasible() {
        // Min (10) far below monthly interest (~50) but the extra payment clears it.
        let input = "Trap, 3000, 24, 10";
        let p = plan(input, "snowball", 500.0, "2026-01-01", d(2026, 1, 1)).unwrap();
        assert!(!p.minimum_only.feasible);
        assert!(p.interest_saved_vs_minimum.is_none());
        assert!(!p.minimum_only.note.is_empty());
    }

    #[test]
    fn impossible_budget_errors_with_guidance() {
        // Min covers nothing and no extra → never pays off.
        let input = "Payday, 5000, 90, 10";
        let err = plan(input, "snowball", 0.0, "2026-01-01", d(2026, 1, 1)).unwrap_err();
        assert!(err.contains("never be paid off"), "got: {err}");
    }

    #[test]
    fn parse_errors_are_actionable() {
        assert!(plan("", "snowball", 0.0, "2026-01-01", d(2026, 1, 1))
            .unwrap_err()
            .contains("no debts"));
        assert!(plan("Visa, 2500, 19.99", "snowball", 0.0, "2026-01-01", d(2026, 1, 1))
            .unwrap_err()
            .contains("4 comma-separated"));
        assert!(plan("Visa, abc, 19.99, 75", "snowball", 0.0, "2026-01-01", d(2026, 1, 1))
            .unwrap_err()
            .contains("not a number"));
        assert!(plan("Visa, 2500, 19.99, 75", "diamond", 0.0, "2026-01-01", d(2026, 1, 1))
            .unwrap_err()
            .contains("unknown method"));
        assert!(plan("Visa, 2500, 19.99, 75", "snowball", 0.0, "nope", d(2026, 1, 1))
            .unwrap_err()
            .contains("could not parse start date"));
    }

    #[test]
    fn currency_symbols_and_percent_are_stripped() {
        let a = plan("Visa, $2,?", "snowball", 0.0, "2026-01-01", d(2026, 1, 1));
        // (malformed — thousands separator adds a field) just ensure it errors cleanly
        assert!(a.is_err());
        let p = plan("Visa, $2500, 19.99%, $75", "snowball", 100.0, "2026-01-01", d(2026, 1, 1)).unwrap();
        assert_eq!(p.payoff_order[0].original_balance, 2500.0);
        assert_eq!(p.payoff_order[0].apr, 19.99);
        assert_eq!(p.payoff_order[0].minimum_payment, 75.0);
    }

    #[test]
    fn blank_start_date_uses_today() {
        let input = "Visa, 2500, 19.99, 75";
        let p = plan(input, "snowball", 200.0, "", d(2026, 7, 23)).unwrap();
        assert_eq!(p.start_date, "2026-07-23");
    }

    #[test]
    fn add_months_clamps_end_of_month() {
        assert_eq!(add_months(d(2026, 1, 31), 1), d(2026, 2, 28));
        assert_eq!(add_months(d(2024, 1, 31), 1), d(2024, 2, 29)); // leap
        assert_eq!(add_months(d(2026, 1, 1), 12), d(2027, 1, 1));
    }

    #[test]
    fn json_is_pretty_and_parses() {
        let s = plan_json("Visa, 2500, 19.99, 75", "snowball", 200.0, "2026-01-01", d(2026, 1, 1)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["method"], "snowball");
        assert!(v["payoff_order"].as_array().unwrap().len() == 1);
        assert!(s.contains('\n')); // pretty
    }
}
