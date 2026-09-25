//! amortization-schedule core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Builds a full loan amortization schedule: one row per payment with the split
//! into principal and interest, the extra paid that period, and the balance
//! left — plus the level payment, the payoff date and the lifetime totals.
//!
//! The level payment comes from the standard amortizing-loan formula
//!
//! ```text
//! P = L * i / (1 - (1 + i)^-n)      (i = annual_rate/100/payments_per_year)
//! ```
//!
//! and falls back to `L / n` at a 0% rate. The schedule is then simulated period
//! by period in integer **cents**, so every surface (chat, CLI, page) produces
//! byte-identical numbers:
//!
//! * interest for the period = `round(balance * i)`, rounded to the cent;
//! * principal = `payment - interest`, plus any extra payment;
//! * the LAST scheduled payment absorbs the rounding drift (it clears whatever
//!   balance is left), which is what a real lender's final payment does;
//! * an extra payment goes straight to principal, so it shortens the term and
//!   cuts total interest instead of lowering the payment.
//!
//! Output comes in three shapes — an aligned text `table`, spreadsheet-ready
//! `csv`, or structured `json` — and in two views: every `period`, or one row
//! per loan `annual` year.

use chrono::{Days, Months, NaiveDate};
use serde::Serialize;

/// Hard cap on the number of scheduled payments, so the simulation is bounded
/// and the output stays a sane size (weekly payments for 96 years).
pub const MAX_PERIODS: usize = 5000;
/// Hard cap on the loan term in years.
pub const MAX_YEARS: f64 = 100.0;
/// Hard cap on the nominal annual interest rate (percent).
pub const MAX_RATE: f64 = 100.0;
/// Per-field money ceiling — guards against typos and overflow.
pub const MAX_MONEY: f64 = 1_000_000_000.0;

/// How the calendar advances between two payments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// A fixed number of days (weekly / biweekly).
    Days(u64),
    /// A fixed number of months, with the day-of-month clamped to the month
    /// length (Jan 31 + 1 month = Feb 28).
    Months(u32),
}

/// A supported payment frequency.
struct Freq {
    /// Canonical parameter value.
    name: &'static str,
    /// Adjective used in prose ("360 monthly payments").
    label: &'static str,
    /// Payments per year — also the divisor for the periodic interest rate.
    per_year: f64,
    step: Step,
}

const FREQUENCIES: [Freq; 6] = [
    Freq {
        name: "weekly",
        label: "weekly",
        per_year: 52.0,
        step: Step::Days(7),
    },
    Freq {
        name: "biweekly",
        label: "biweekly",
        per_year: 26.0,
        step: Step::Days(14),
    },
    Freq {
        name: "monthly",
        label: "monthly",
        per_year: 12.0,
        step: Step::Months(1),
    },
    Freq {
        name: "quarterly",
        label: "quarterly",
        per_year: 4.0,
        step: Step::Months(3),
    },
    Freq {
        name: "semiannual",
        label: "semiannual",
        per_year: 2.0,
        step: Step::Months(6),
    },
    Freq {
        name: "annual",
        label: "annual",
        per_year: 1.0,
        step: Step::Months(12),
    },
];

/// The accepted `payment_frequency` values, in schedule-density order.
pub const FREQUENCY_NAMES: [&str; 6] = [
    "weekly",
    "biweekly",
    "monthly",
    "quarterly",
    "semiannual",
    "annual",
];
/// The accepted `schedule_view` values.
pub const VIEW_NAMES: [&str; 2] = ["period", "annual"];
/// The accepted `format` values.
pub const FORMAT_NAMES: [&str; 3] = ["table", "csv", "json"];

fn find_freq(name: &str) -> Option<&'static Freq> {
    let n = name.trim().to_ascii_lowercase();
    FREQUENCIES.iter().find(|f| f.name == n)
}

/// One scheduled payment.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// 1-based payment number.
    pub period: usize,
    /// Payment date (`YYYY-MM-DD`).
    pub date: String,
    /// Total paid this period (principal + interest, including any extra).
    pub payment: f64,
    /// Principal repaid this period (including any extra).
    pub principal: f64,
    /// Interest accrued and paid this period.
    pub interest: f64,
    /// Extra principal paid this period (recurring extra + any lump sum).
    pub extra: f64,
    /// Balance still owed after this payment.
    pub balance: f64,
}

/// One loan year (a group of `payments_per_year` consecutive payments).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YearRow {
    /// 1-based loan year.
    pub year: usize,
    /// Date of the last payment in this year.
    pub through: String,
    /// Number of payments made in this year.
    pub payments: usize,
    /// Total paid in this year.
    pub paid: f64,
    /// Principal repaid in this year.
    pub principal: f64,
    /// Interest paid in this year.
    pub interest: f64,
    /// Extra principal paid in this year.
    pub extra: f64,
    /// Balance still owed at the end of this year.
    pub ending_balance: f64,
}

/// The complete schedule plus its headline figures.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Schedule {
    /// Amount financed.
    pub loan_amount: f64,
    /// Nominal annual interest rate, as a percent.
    pub annual_interest_rate_percent: f64,
    /// Canonical payment frequency name.
    pub payment_frequency: String,
    /// Payments per year for that frequency.
    pub payments_per_year: u32,
    /// Term in whole years (`loan_years` + `loan_months` normalized).
    pub term_years: u32,
    /// Leftover months of the term beyond `term_years`.
    pub term_months: u32,
    /// Payments the loan was scheduled for, before any extra payment.
    pub scheduled_payments: usize,
    /// The level payment for every period except a possibly-adjusted last one.
    pub payment_per_period: f64,
    /// Recurring extra principal paid each period.
    pub extra_payment: f64,
    /// One-time extra principal amount.
    pub extra_one_time: f64,
    /// Period the one-time amount is applied in.
    pub extra_one_time_period: usize,
    /// Date of the first payment (`YYYY-MM-DD`).
    pub first_payment_date: String,
    /// Date of the final payment (`YYYY-MM-DD`).
    pub payoff_date: String,
    /// Payments actually made (fewer than `scheduled_payments` with extras).
    pub payments_made: usize,
    /// Total principal repaid — equals `loan_amount`.
    pub total_principal: f64,
    /// Total interest paid over the life of the loan.
    pub total_interest: f64,
    /// Total extra principal paid.
    pub total_extra: f64,
    /// Total cash paid: `total_principal + total_interest`.
    pub total_paid: f64,
    /// Interest the extra payments save vs. the same loan with no extras
    /// (0 when there are no extras).
    pub interest_saved: f64,
    /// Payments the extra payments remove from the term (0 when none).
    pub payments_saved: usize,
    /// Which view the rows below were built for (`period` | `annual`).
    pub view: String,
    /// Per-payment rows (empty in the `annual` view).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub schedule: Vec<Row>,
    /// Per-loan-year rows (empty in the `period` view).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub annual_schedule: Vec<YearRow>,
    /// Plain-language one-line summary.
    pub summary: String,
}

/// Every input, each `None` when unset so the documented default applies. All
/// surfaces funnel through the same defaults and validation.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Amount borrowed. Default 300000.
    pub loan_amount: Option<f64>,
    /// Nominal annual rate as a percent. Default 6.
    pub annual_interest_rate_percent: Option<f64>,
    /// Term in whole years. Default 30.
    pub loan_years: Option<f64>,
    /// Extra months on top of `loan_years`. Default 0.
    pub loan_months: Option<f64>,
    /// One of [`FREQUENCY_NAMES`]. Default `monthly`.
    pub payment_frequency: Option<String>,
    /// First payment date (`YYYY-MM-DD`). Blank → today.
    pub start_date: Option<String>,
    /// Extra principal every period. Default 0.
    pub extra_payment: Option<f64>,
    /// One-time extra principal. Default 0.
    pub extra_one_time: Option<f64>,
    /// Period the one-time amount lands in. Default 1.
    pub extra_one_time_period: Option<f64>,
    /// One of [`VIEW_NAMES`]. Default `period`.
    pub schedule_view: Option<String>,
    /// One of [`FORMAT_NAMES`]. Default `table`.
    pub format: Option<String>,
    /// Symbol prefixed to money in the summary/totals. Default `$`.
    pub currency_symbol: Option<String>,
}

fn cents(dollars: f64) -> i64 {
    (dollars * 100.0).round() as i64
}
fn dollars(c: i64) -> f64 {
    c as f64 / 100.0
}

/// `1798.65` — raw, no grouping (CSV + JSON shape).
fn plain(c: i64) -> String {
    format!("{:.2}", dollars(c))
}

/// `1,798.65` — grouped thousands, always 2 decimals.
fn grouped(c: i64) -> String {
    let neg = c < 0;
    let abs = c.unsigned_abs();
    let whole = abs / 100;
    let frac = abs % 100;
    let digits = whole.to_string();
    let bytes = digits.as_bytes();
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    for (idx, ch) in bytes.iter().enumerate() {
        if idx > 0 && (bytes.len() - idx) % 3 == 0 {
            out.push(',');
        }
        out.push(*ch as char);
    }
    out.push('.');
    if frac < 10 {
        out.push('0');
    }
    out.push_str(&frac.to_string());
    out
}

/// `$1,798.65` — grouped with the caller's currency symbol.
fn money(c: i64, symbol: &str) -> String {
    format!("{}{}", symbol, grouped(c))
}

/// Trim trailing zeros off a rate so `6.0` prints as `6` and `6.25` stays `6.25`.
fn rate_str(rate: f64) -> String {
    let s = format!("{:.4}", rate);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() {
        "0".into()
    } else {
        s
    }
}

/// Parse a date string. Accepts `YYYY-MM-DD` (canonical), `YYYY/MM/DD`,
/// `MM/DD/YYYY` and `DD.MM.YYYY`.
fn parse_date(s: &str) -> Result<NaiveDate, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("empty date".into());
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

/// The date of payment `period` (1-based), starting from the first payment date.
fn payment_date(first: NaiveDate, step: Step, period: usize) -> NaiveDate {
    let n = (period - 1) as u64;
    match step {
        Step::Days(d) => first.checked_add_days(Days::new(d * n)).unwrap_or(first),
        Step::Months(m) => first
            .checked_add_months(Months::new(m * n as u32))
            .unwrap_or(first),
    }
}

/// Read a numeric input, applying the default and rejecting non-finite values.
fn num(value: Option<f64>, default: f64, label: &str) -> Result<f64, String> {
    let v = value.unwrap_or(default);
    if !v.is_finite() {
        return Err(format!("{} must be a finite number", label));
    }
    Ok(v)
}

/// Validate a money input against 0 ≤ v ≤ [`MAX_MONEY`].
fn money_in(v: f64, label: &str) -> Result<i64, String> {
    if v < 0.0 {
        return Err(format!("{} must be zero or positive (got {})", label, v));
    }
    if v > MAX_MONEY {
        return Err(format!(
            "{} must be at most {} (got {})",
            label, MAX_MONEY, v
        ));
    }
    Ok(cents(v))
}

/// Run the amortization simulation. Returns `(rows, total_interest_cents)` where
/// rows are `(interest, principal, extra, payment, balance)` in cents.
struct Sim {
    rows: Vec<(i64, i64, i64, i64, i64)>,
    total_interest: i64,
}

fn simulate(
    loan_cents: i64,
    periodic_rate: f64,
    periods: usize,
    payment_cents: i64,
    extra_cents: i64,
    one_time_cents: i64,
    one_time_period: usize,
) -> Result<Sim, String> {
    let mut balance = loan_cents;
    let mut rows = Vec::with_capacity(periods.min(MAX_PERIODS));
    let mut total_interest = 0i64;
    for period in 1..=periods {
        let interest = (balance as f64 * periodic_rate).round() as i64;
        let extra = extra_cents
            + if one_time_cents > 0 && period == one_time_period {
                one_time_cents
            } else {
                0
            };
        let mut principal = payment_cents - interest + extra;
        if principal <= 0 {
            return Err(format!(
                "the level payment of {} does not cover the {} of interest that accrues in period \
                 {} — lower the interest rate, shorten the term, or add an extra payment",
                plain(payment_cents),
                plain(interest),
                period
            ));
        }
        // The last scheduled payment absorbs the cent-level rounding drift, so
        // the balance lands exactly on zero — as a real final payment does.
        if principal >= balance || period == periods {
            principal = balance;
        }
        let paid = interest + principal;
        balance -= principal;
        total_interest += interest;
        rows.push((interest, principal, extra.min(principal), paid, balance));
        if balance <= 0 {
            break;
        }
    }
    Ok(Sim {
        rows,
        total_interest,
    })
}

/// Build the full schedule. `today` supplies the first payment date when
/// `start_date` is blank (each surface passes its own clock).
pub fn build(inputs: &Inputs, today: NaiveDate) -> Result<Schedule, String> {
    let loan = num(inputs.loan_amount, 300_000.0, "loan amount")?;
    if loan <= 0.0 {
        return Err(format!("loan amount must be greater than 0 (got {})", loan));
    }
    let loan_cents = money_in(loan, "loan amount")?;

    let rate = num(
        inputs.annual_interest_rate_percent,
        6.0,
        "annual interest rate",
    )?;
    if !(0.0..=MAX_RATE).contains(&rate) {
        return Err(format!(
            "annual interest rate must be between 0 and {} percent (got {})",
            MAX_RATE, rate
        ));
    }

    let years = num(inputs.loan_years, 30.0, "loan years")?;
    if !(0.0..=MAX_YEARS).contains(&years) {
        return Err(format!(
            "loan term must be between 0 and {} years (got {})",
            MAX_YEARS, years
        ));
    }
    let months = num(inputs.loan_months, 0.0, "loan months")?;
    if !(0.0..=1200.0).contains(&months) {
        return Err(format!(
            "extra term months must be between 0 and 1200 (got {})",
            months
        ));
    }
    let total_months = years * 12.0 + months;
    if total_months < 1.0 {
        return Err("loan term must be at least 1 month — set loan_years or loan_months".into());
    }

    let freq_name = inputs
        .payment_frequency
        .clone()
        .unwrap_or_else(|| "monthly".into());
    let freq = find_freq(&freq_name).ok_or_else(|| {
        format!(
            "unknown payment_frequency '{}' — expected one of {}",
            freq_name.trim(),
            FREQUENCY_NAMES.join(", ")
        )
    })?;

    let periods = (total_months * freq.per_year / 12.0).round() as i64;
    if periods < 1 {
        return Err(format!(
            "the term is shorter than one {} payment — lengthen the term or choose a more \
             frequent payment schedule",
            freq.label
        ));
    }
    if periods as usize > MAX_PERIODS {
        return Err(format!(
            "that term needs {} payments, over the {} cap — shorten the term or choose a less \
             frequent payment schedule",
            periods, MAX_PERIODS
        ));
    }
    let periods = periods as usize;

    let extra = num(inputs.extra_payment, 0.0, "extra payment")?;
    let extra_cents = money_in(extra, "extra payment")?;
    let one_time = num(inputs.extra_one_time, 0.0, "one-time extra payment")?;
    let one_time_cents = money_in(one_time, "one-time extra payment")?;
    let one_time_period = num(inputs.extra_one_time_period, 1.0, "one-time payment period")?;
    if one_time_period.fract() != 0.0 || one_time_period < 1.0 {
        return Err(format!(
            "extra_one_time_period must be a whole payment number of 1 or more (got {})",
            one_time_period
        ));
    }
    let one_time_period = one_time_period as usize;
    if one_time_cents > 0 && one_time_period > periods {
        return Err(format!(
            "extra_one_time_period {} is past the last scheduled payment ({})",
            one_time_period, periods
        ));
    }

    let view = inputs
        .schedule_view
        .clone()
        .unwrap_or_else(|| "period".into())
        .trim()
        .to_ascii_lowercase();
    if !VIEW_NAMES.contains(&view.as_str()) {
        return Err(format!(
            "unknown schedule_view '{}' — expected one of {}",
            view,
            VIEW_NAMES.join(", ")
        ));
    }

    let symbol = inputs
        .currency_symbol
        .clone()
        .unwrap_or_else(|| "$".into())
        .trim()
        .to_string();
    if symbol.chars().count() > 3 {
        return Err(format!(
            "currency_symbol must be at most 3 characters (got '{}')",
            symbol
        ));
    }

    let start_input = inputs.start_date.clone().unwrap_or_default();
    let first_date = if start_input.trim().is_empty() {
        today
    } else {
        parse_date(&start_input)?
    };

    // Level payment from the standard amortizing-loan formula (L/n at 0%).
    let periodic_rate = rate / 100.0 / freq.per_year;
    let payment_cents = if periodic_rate == 0.0 {
        (loan_cents as f64 / periods as f64).round() as i64
    } else {
        let p = loan_cents as f64 * periodic_rate
            / (1.0 - (1.0 + periodic_rate).powf(-(periods as f64)));
        p.round() as i64
    };

    let sim = simulate(
        loan_cents,
        periodic_rate,
        periods,
        payment_cents,
        extra_cents,
        one_time_cents,
        one_time_period,
    )?;

    // Same loan with no extras — the baseline the savings are measured against.
    let (baseline_interest, baseline_periods) = if extra_cents > 0 || one_time_cents > 0 {
        let b = simulate(
            loan_cents,
            periodic_rate,
            periods,
            payment_cents,
            0,
            0,
            one_time_period,
        )?;
        (b.total_interest, b.rows.len())
    } else {
        (sim.total_interest, sim.rows.len())
    };

    let mut schedule = Vec::with_capacity(sim.rows.len());
    let mut total_extra = 0i64;
    for (idx, (interest, principal, ex, paid, balance)) in sim.rows.iter().enumerate() {
        total_extra += *ex;
        schedule.push(Row {
            period: idx + 1,
            date: payment_date(first_date, freq.step, idx + 1).to_string(),
            payment: dollars(*paid),
            principal: dollars(*principal),
            interest: dollars(*interest),
            extra: dollars(*ex),
            balance: dollars(*balance),
        });
    }

    let per_year = freq.per_year as usize;
    let mut annual_schedule: Vec<YearRow> = Vec::new();
    for chunk in schedule.chunks(per_year.max(1)) {
        let year = annual_schedule.len() + 1;
        annual_schedule.push(YearRow {
            year,
            through: chunk[chunk.len() - 1].date.clone(),
            payments: chunk.len(),
            paid: dollars(chunk.iter().map(|r| cents(r.payment)).sum()),
            principal: dollars(chunk.iter().map(|r| cents(r.principal)).sum()),
            interest: dollars(chunk.iter().map(|r| cents(r.interest)).sum()),
            extra: dollars(chunk.iter().map(|r| cents(r.extra)).sum()),
            ending_balance: chunk[chunk.len() - 1].balance,
        });
    }

    let payments_made = schedule.len();
    let payoff_date = schedule[payments_made - 1].date.clone();
    let total_interest = sim.total_interest;
    let total_paid = loan_cents + total_interest;
    let interest_saved = (baseline_interest - total_interest).max(0);
    let payments_saved = baseline_periods.saturating_sub(payments_made);

    let term_years = (total_months / 12.0).floor() as u32;
    let term_months = (total_months - term_years as f64 * 12.0).round() as u32;

    let mut summary = format!(
        "{} borrowed at {}% over {} in {} {} payments of {} — total interest {}, total paid {}, \
         paid off {}.",
        money(loan_cents, &symbol),
        rate_str(rate),
        term_label(term_years, term_months),
        payments_made,
        freq.label,
        money(payment_cents, &symbol),
        money(total_interest, &symbol),
        money(total_paid, &symbol),
        payoff_date
    );
    if interest_saved > 0 || payments_saved > 0 {
        summary.push_str(&format!(
            " The extra payments save {} in interest and cut {} payments ({}).",
            money(interest_saved, &symbol),
            payments_saved,
            time_saved_label(payments_saved, freq.per_year)
        ));
    }

    let (schedule, annual_schedule) = if view == "annual" {
        (Vec::new(), annual_schedule)
    } else {
        (schedule, Vec::new())
    };

    Ok(Schedule {
        loan_amount: dollars(loan_cents),
        annual_interest_rate_percent: rate,
        payment_frequency: freq.name.to_string(),
        payments_per_year: freq.per_year as u32,
        term_years,
        term_months,
        scheduled_payments: periods,
        payment_per_period: dollars(payment_cents),
        extra_payment: dollars(extra_cents),
        extra_one_time: dollars(one_time_cents),
        extra_one_time_period: one_time_period,
        first_payment_date: first_date.to_string(),
        payoff_date,
        payments_made,
        total_principal: dollars(loan_cents),
        total_interest: dollars(total_interest),
        total_extra: dollars(total_extra),
        total_paid: dollars(total_paid),
        interest_saved: dollars(interest_saved),
        payments_saved,
        view,
        schedule,
        annual_schedule,
        summary,
    })
}

/// `30 years`, `5 years 6 months`, `18 months`.
fn term_label(years: u32, months: u32) -> String {
    match (years, months) {
        (0, m) => format!("{} month{}", m, plural(m)),
        (y, 0) => format!("{} year{}", y, plural(y)),
        (y, m) => format!("{} year{} {} month{}", y, plural(y), m, plural(m)),
    }
}

fn plural(n: u32) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Turn a saved-payment count into an approximate calendar span.
fn time_saved_label(payments_saved: usize, per_year: f64) -> String {
    let months = (payments_saved as f64 * 12.0 / per_year).round() as u32;
    term_label(months / 12, months % 12)
}

/// Right-align every cell under its header and join the columns with two spaces.
fn render_grid(headers: &[String], rows: &[Vec<String>], left_align: &[usize]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    let line = |cells: &[String], out: &mut String| {
        let parts: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let pad = widths[i].saturating_sub(c.chars().count());
                if left_align.contains(&i) {
                    format!("{}{}", c, " ".repeat(pad))
                } else {
                    format!("{}{}", " ".repeat(pad), c)
                }
            })
            .collect();
        out.push_str(parts.join("  ").trim_end());
        out.push('\n');
    };
    line(headers, &mut out);
    let rule: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    line(&rule, &mut out);
    for row in rows {
        line(row, &mut out);
    }
    out
}

/// Render the aligned text table (headline figures + the schedule grid).
fn render_table(s: &Schedule, symbol: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Amortization schedule — {} at {}% for {}, paid {}\n\n",
        money(cents(s.loan_amount), symbol),
        rate_str(s.annual_interest_rate_percent),
        term_label(s.term_years, s.term_months),
        s.payment_frequency
    ));
    let mut facts: Vec<(&str, String)> = vec![
        (
            "Payment",
            format!("{} per period", money(cents(s.payment_per_period), symbol)),
        ),
        ("First payment", s.first_payment_date.clone()),
        ("Payoff date", s.payoff_date.clone()),
        (
            "Payments",
            format!("{} of {} scheduled", s.payments_made, s.scheduled_payments),
        ),
        ("Total principal", money(cents(s.total_principal), symbol)),
        ("Total interest", money(cents(s.total_interest), symbol)),
        ("Total paid", money(cents(s.total_paid), symbol)),
    ];
    if s.extra_payment > 0.0 {
        facts.insert(
            1,
            ("Extra per period", money(cents(s.extra_payment), symbol)),
        );
    }
    if s.extra_one_time > 0.0 {
        facts.insert(
            if s.extra_payment > 0.0 { 2 } else { 1 },
            (
                "One-time extra",
                format!(
                    "{} at payment {}",
                    money(cents(s.extra_one_time), symbol),
                    s.extra_one_time_period
                ),
            ),
        );
    }
    if s.interest_saved > 0.0 || s.payments_saved > 0 {
        facts.push(("Interest saved", money(cents(s.interest_saved), symbol)));
        facts.push(("Payments saved", s.payments_saved.to_string()));
    }
    let label_width = facts.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    for (label, value) in &facts {
        out.push_str(&format!("{:<w$}  {}\n", label, value, w = label_width));
    }

    out.push_str(&format!("\nSchedule (amounts in {}):\n\n", symbol));
    if s.view == "annual" {
        let headers = [
            "Year",
            "Through",
            "Payments",
            "Paid",
            "Principal",
            "Interest",
            "Extra",
            "Balance",
        ]
        .iter()
        .map(|h| h.to_string())
        .collect::<Vec<_>>();
        let rows: Vec<Vec<String>> = s
            .annual_schedule
            .iter()
            .map(|r| {
                vec![
                    r.year.to_string(),
                    r.through.clone(),
                    r.payments.to_string(),
                    grouped(cents(r.paid)),
                    grouped(cents(r.principal)),
                    grouped(cents(r.interest)),
                    grouped(cents(r.extra)),
                    grouped(cents(r.ending_balance)),
                ]
            })
            .collect();
        out.push_str(&render_grid(&headers, &rows, &[1]));
    } else {
        let headers = [
            "#",
            "Date",
            "Payment",
            "Principal",
            "Interest",
            "Extra",
            "Balance",
        ]
        .iter()
        .map(|h| h.to_string())
        .collect::<Vec<_>>();
        let rows: Vec<Vec<String>> = s
            .schedule
            .iter()
            .map(|r| {
                vec![
                    r.period.to_string(),
                    r.date.clone(),
                    grouped(cents(r.payment)),
                    grouped(cents(r.principal)),
                    grouped(cents(r.interest)),
                    grouped(cents(r.extra)),
                    grouped(cents(r.balance)),
                ]
            })
            .collect();
        out.push_str(&render_grid(&headers, &rows, &[1]));
    }
    out.push_str(&format!("\n{}\n", s.summary));
    out
}

/// Render the schedule as CSV — the table only, so it pastes straight into a
/// spreadsheet with no preamble rows to strip.
fn render_csv(s: &Schedule) -> String {
    let mut out = String::new();
    if s.view == "annual" {
        out.push_str("year,through,payments,paid,principal,interest,extra,ending_balance\n");
        for r in &s.annual_schedule {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                r.year,
                r.through,
                r.payments,
                plain(cents(r.paid)),
                plain(cents(r.principal)),
                plain(cents(r.interest)),
                plain(cents(r.extra)),
                plain(cents(r.ending_balance))
            ));
        }
    } else {
        out.push_str("period,date,payment,principal,interest,extra,balance\n");
        for r in &s.schedule {
            out.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                r.period,
                r.date,
                plain(cents(r.payment)),
                plain(cents(r.principal)),
                plain(cents(r.interest)),
                plain(cents(r.extra)),
                plain(cents(r.balance))
            ));
        }
    }
    out
}

/// Build the schedule and render it in the requested `format`.
pub fn render(inputs: &Inputs, today: NaiveDate) -> Result<String, String> {
    let format = inputs
        .format
        .clone()
        .unwrap_or_else(|| "table".into())
        .trim()
        .to_ascii_lowercase();
    if !FORMAT_NAMES.contains(&format.as_str()) {
        return Err(format!(
            "unknown format '{}' — expected one of {}",
            format,
            FORMAT_NAMES.join(", ")
        ));
    }
    let symbol = {
        let s = inputs
            .currency_symbol
            .clone()
            .unwrap_or_else(|| "$".into())
            .trim()
            .to_string();
        if s.is_empty() {
            "$".to_string()
        } else {
            s
        }
    };
    let schedule = build(inputs, today)?;
    Ok(match format.as_str() {
        "csv" => render_csv(&schedule),
        "json" => serde_json::to_string_pretty(&schedule)
            .map_err(|e| format!("could not serialize the schedule: {}", e))?,
        _ => render_table(&schedule, &symbol),
    })
}

/// Stable default date used by CLI/page/chat surfaces when the user leaves
/// `start_date` blank. Keeping it deterministic makes examples and tests
/// byte-identical across machines.
pub fn default_today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid fixed default date")
}

/// Render using the deterministic default first-payment date.
pub fn run_inputs(inputs: &Inputs) -> Result<String, String> {
    render(inputs, default_today())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn base() -> Inputs {
        Inputs {
            start_date: Some("2026-01-01".into()),
            ..Default::default()
        }
    }

    #[test]
    fn defaults_build_a_full_30_year_monthly_schedule() {
        let s = build(&base(), d(2026, 1, 1)).unwrap();
        assert_eq!(s.payment_per_period, 1798.65);
        assert_eq!(s.payments_made, 360);
        assert_eq!(s.scheduled_payments, 360);
        assert_eq!(s.schedule.len(), 360);
        assert_eq!(s.total_interest, 347_515.44);
        assert_eq!(s.total_paid, 647_515.44);
        assert_eq!(s.total_principal, 300_000.0);
        // First row: 1 month of interest at 0.5%, the rest to principal.
        assert_eq!(s.schedule[0].date, "2026-01-01");
        assert_eq!(s.schedule[0].interest, 1500.0);
        assert_eq!(s.schedule[0].principal, 298.65);
        assert_eq!(s.schedule[0].balance, 299_701.35);
        // Last row clears the balance exactly on the payoff date.
        assert_eq!(s.schedule[359].balance, 0.0);
        assert_eq!(s.payoff_date, "2055-12-01");
    }

    #[test]
    fn blank_start_date_uses_today() {
        let s = build(&Inputs::default(), d(2026, 9, 24)).unwrap();
        assert_eq!(s.first_payment_date, "2026-09-24");
        assert_eq!(s.schedule[1].date, "2026-10-24");
    }

    #[test]
    fn extra_payment_shortens_the_term_and_saves_interest() {
        let s = build(
            &Inputs {
                extra_payment: Some(200.0),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap();
        assert_eq!(s.payments_made, 279);
        assert_eq!(s.total_interest, 256_341.57);
        assert_eq!(s.interest_saved, 91_173.87);
        assert_eq!(s.payments_saved, 81);
        assert_eq!(s.total_extra, 55_800.0);
        assert!(s.summary.contains("save $91,173.87 in interest"));
    }

    #[test]
    fn one_time_lump_sum_lands_in_its_period() {
        let s = build(
            &Inputs {
                extra_one_time: Some(5000.0),
                extra_one_time_period: Some(12.0),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap();
        assert_eq!(s.schedule[10].extra, 0.0);
        assert_eq!(s.schedule[11].extra, 5000.0);
        assert_eq!(s.payments_made, 345);
        assert_eq!(s.total_interest, 325_147.46);
    }

    #[test]
    fn zero_rate_splits_the_principal_evenly() {
        let s = build(
            &Inputs {
                loan_amount: Some(1200.0),
                annual_interest_rate_percent: Some(0.0),
                loan_years: Some(1.0),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap();
        assert_eq!(s.payment_per_period, 100.0);
        assert_eq!(s.total_interest, 0.0);
        assert_eq!(s.schedule.len(), 12);
    }

    #[test]
    fn every_frequency_amortizes_to_zero() {
        for (name, expect_periods, expect_payment) in [
            ("weekly", 1560, 414.79),
            ("biweekly", 780, 829.75),
            ("monthly", 360, 1798.65),
            ("quarterly", 120, 5405.56),
            ("semiannual", 60, 10_839.89),
            ("annual", 30, 21_794.67),
        ] {
            let s = build(
                &Inputs {
                    payment_frequency: Some(name.into()),
                    ..base()
                },
                d(2026, 1, 1),
            )
            .unwrap();
            assert_eq!(s.payments_made, expect_periods, "{} periods", name);
            assert_eq!(s.payment_per_period, expect_payment, "{} payment", name);
            assert_eq!(
                s.schedule[expect_periods - 1].balance,
                0.0,
                "{} ends at zero",
                name
            );
        }
    }

    #[test]
    fn term_months_add_to_the_years() {
        let s = build(
            &Inputs {
                loan_years: Some(1.0),
                loan_months: Some(6.0),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap();
        assert_eq!(s.scheduled_payments, 18);
        assert_eq!(s.term_years, 1);
        assert_eq!(s.term_months, 6);
    }

    #[test]
    fn annual_view_groups_payments_by_loan_year() {
        let s = build(
            &Inputs {
                schedule_view: Some("annual".into()),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap();
        assert!(s.schedule.is_empty());
        assert_eq!(s.annual_schedule.len(), 30);
        assert_eq!(s.annual_schedule[0].payments, 12);
        assert_eq!(s.annual_schedule[0].through, "2026-12-01");
        assert_eq!(s.annual_schedule[29].ending_balance, 0.0);
        let interest: f64 = s.annual_schedule.iter().map(|r| r.interest).sum();
        assert!((interest - 347_515.44).abs() < 0.005);
    }

    #[test]
    fn csv_output_is_a_bare_table() {
        let out = render(
            &Inputs {
                loan_amount: Some(1000.0),
                annual_interest_rate_percent: Some(12.0),
                loan_years: Some(1.0),
                format: Some("csv".into()),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "period,date,payment,principal,interest,extra,balance"
        );
        assert_eq!(lines[1], "1,2026-01-01,88.85,78.85,10.00,0.00,921.15");
        assert_eq!(lines.len(), 13);
        assert_eq!(lines[12], "12,2026-12-01,88.84,87.96,0.88,0.00,0.00");
    }

    #[test]
    fn table_output_has_aligned_rows_and_totals() {
        let out = render(
            &Inputs {
                loan_amount: Some(1000.0),
                annual_interest_rate_percent: Some(12.0),
                loan_years: Some(1.0),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap();
        assert!(out.contains("Amortization schedule — $1,000.00 at 12% for 1 year, paid monthly"));
        assert!(out.contains("Total interest   $66.19"));
        let lines: Vec<&str> = out.lines().collect();
        // Header, rule and first data row line up column-for-column.
        assert_eq!(
            lines[12],
            " #  Date        Payment  Principal  Interest  Extra  Balance"
        );
        assert_eq!(
            lines[13],
            "--  ----------  -------  ---------  --------  -----  -------"
        );
        assert_eq!(
            lines[14],
            " 1  2026-01-01    88.85      78.85     10.00   0.00   921.15"
        );
    }

    /// The exact CSV the CLI check and the Playwright spec assert on — three
    /// annual payments, so the whole document is short enough to pin verbatim.
    #[test]
    fn three_annual_payments_render_an_exact_csv() {
        let out = render(
            &Inputs {
                loan_amount: Some(1000.0),
                annual_interest_rate_percent: Some(12.0),
                loan_years: Some(3.0),
                payment_frequency: Some("annual".into()),
                format: Some("csv".into()),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap();
        assert_eq!(
            out,
            "period,date,payment,principal,interest,extra,balance\n\
             1,2026-01-01,416.35,296.35,120.00,0.00,703.65\n\
             2,2027-01-01,416.35,331.91,84.44,0.00,371.74\n\
             3,2028-01-01,416.35,371.74,44.61,0.00,0.00\n"
        );
    }

    #[test]
    fn json_output_carries_the_rows() {
        let out = render(
            &Inputs {
                loan_amount: Some(1000.0),
                annual_interest_rate_percent: Some(12.0),
                loan_years: Some(1.0),
                format: Some("json".into()),
                currency_symbol: Some("€".into()),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["payment_per_period"], 88.85);
        assert_eq!(v["schedule"].as_array().unwrap().len(), 12);
        assert_eq!(v["schedule"][0]["interest"], 10.0);
        assert!(v["annual_schedule"].is_null());
        assert!(v["summary"].as_str().unwrap().contains("€1,000.00"));
    }

    #[test]
    fn rejects_a_zero_loan_amount() {
        let err = build(
            &Inputs {
                loan_amount: Some(0.0),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap_err();
        assert!(
            err.contains("loan amount must be greater than 0"),
            "{}",
            err
        );
    }

    #[test]
    fn rejects_an_unknown_frequency() {
        let err = build(
            &Inputs {
                payment_frequency: Some("fortnightly".into()),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap_err();
        assert!(
            err.contains("unknown payment_frequency 'fortnightly'"),
            "{}",
            err
        );
        assert!(err.contains("biweekly"), "{}", err);
    }

    #[test]
    fn rejects_an_unparseable_start_date() {
        let err = build(
            &Inputs {
                start_date: Some("next tuesday".into()),
                ..Default::default()
            },
            d(2026, 1, 1),
        )
        .unwrap_err();
        assert!(err.contains("could not parse start date"), "{}", err);
    }

    #[test]
    fn rejects_a_schedule_over_the_period_cap() {
        let err = build(
            &Inputs {
                payment_frequency: Some("weekly".into()),
                loan_years: Some(100.0),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap_err();
        assert!(err.contains("over the 5000 cap"), "{}", err);
    }

    #[test]
    fn accepts_the_period_cap_boundary() {
        // 100 years of biweekly payments = 2600 rows, just inside the cap.
        let s = build(
            &Inputs {
                payment_frequency: Some("biweekly".into()),
                loan_years: Some(100.0),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap();
        assert_eq!(s.scheduled_payments, 2600);
    }

    #[test]
    fn rejects_a_lump_sum_past_the_last_payment() {
        let err = build(
            &Inputs {
                loan_years: Some(1.0),
                extra_one_time: Some(100.0),
                extra_one_time_period: Some(99.0),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap_err();
        assert!(
            err.contains("past the last scheduled payment (12)"),
            "{}",
            err
        );
    }

    #[test]
    fn rejects_an_unknown_format() {
        let err = render(
            &Inputs {
                format: Some("pdf".into()),
                ..base()
            },
            d(2026, 1, 1),
        )
        .unwrap_err();
        assert!(err.contains("unknown format 'pdf'"), "{}", err);
    }

    #[test]
    fn month_end_start_dates_clamp() {
        let s = build(
            &Inputs {
                start_date: Some("2026-01-31".into()),
                loan_years: Some(1.0),
                ..Default::default()
            },
            d(2026, 1, 1),
        )
        .unwrap();
        assert_eq!(s.schedule[1].date, "2026-02-28");
        assert_eq!(s.schedule[2].date, "2026-03-31");
    }
}
