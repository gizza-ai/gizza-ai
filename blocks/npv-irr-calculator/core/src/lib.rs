//! npv-irr-calculator core — discounted-cash-flow analysis of a cash-flow
//! series: net present value, internal rate of return, modified IRR, payback,
//! and a per-period discounted table. Pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Conventions:
//! * The series is indexed from period 0. Period 0 is NOT discounted.
//! * `initial_investment` is entered as a positive cost; when non-zero it is
//!   inserted as a negative period-0 flow and the pasted series starts at
//!   period 1.
//! * `timing = "end"` (the default) discounts a period-`t` flow by `t`;
//!   `timing = "begin"` discounts flows from period 1 onwards by `t - 1`
//!   (an annuity-due), leaving period 0 undiscounted either way.
//! * `discount_rate` is a nominal ANNUAL percentage; with a non-annual
//!   `period` the per-period rate is `annual / periods_per_year` and the IRR is
//!   reported both per period and annualized as `(1 + r)^ppy - 1`.
//!
//! Educational only — not financial advice.

use serde::Serialize;

/// Largest accepted series length (period 0 included). 1,200 monthly periods is
/// a century of data — far past any competitor's 25/50-row grid — and still
/// renders as a readable table.
pub const MAX_FLOWS: usize = 1200;

/// One period of the discounted cash-flow table.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// Period index; 0 is "today".
    pub period: usize,
    /// The cash flow as entered (negative = money out).
    pub cash_flow: f64,
    /// Exponent the discount factor was raised to (differs from `period` when
    /// `timing = "begin"`).
    pub discount_periods: f64,
    /// `1 / (1 + rate)^discount_periods`.
    pub discount_factor: f64,
    /// `cash_flow * discount_factor`.
    pub present_value: f64,
    /// Running total of `present_value` — the NPV if the project stopped here.
    pub cumulative_pv: f64,
    /// Running total of the undiscounted `cash_flow`.
    pub cumulative_cash_flow: f64,
}

/// The full discounted-cash-flow analysis.
#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    /// Number of cash flows, period 0 included.
    pub count: usize,
    /// Highest period index (`count - 1`).
    pub last_period: usize,
    /// Series spacing, echoed back.
    pub period: String,
    /// Periods per year implied by `period`.
    pub periods_per_year: f64,
    /// `"end"` or `"begin"`.
    pub timing: String,
    /// Nominal annual discount rate as a decimal (0.1 = 10%/yr).
    pub annual_discount_rate: f64,
    /// Per-period discount rate as a decimal.
    pub period_discount_rate: f64,
    /// Net present value in input currency units.
    pub npv: f64,
    /// Present value of the positive flows only.
    pub pv_inflows: f64,
    /// Present value of the negative flows only (a negative number).
    pub pv_outflows: f64,
    /// `pv_inflows / -pv_outflows`; `None` when nothing is invested.
    pub profitability_index: Option<f64>,
    /// IRR per PERIOD as a decimal; `None` when no sign change brackets a root.
    pub irr_period: Option<f64>,
    /// IRR annualized as `(1 + irr_period)^periods_per_year - 1`.
    pub irr_annual: Option<f64>,
    /// Modified IRR per period, financing and reinvesting at the discount rate.
    pub mirr_period: Option<f64>,
    /// Modified IRR annualized.
    pub mirr_annual: Option<f64>,
    /// Undiscounted sum of every cash flow.
    pub total_cash_flow: f64,
    pub total_inflows: f64,
    /// Negative number.
    pub total_outflows: f64,
    /// Fractional periods until the cumulative undiscounted flow turns
    /// non-negative; `None` when it never does.
    pub payback_periods: Option<f64>,
    /// Same, on the discounted cumulative column.
    pub discounted_payback_periods: Option<f64>,
    /// How many times the sign of the cash flows changes. More than one means
    /// the IRR may not be unique.
    pub sign_changes: usize,
    /// Plain-language verdict at the given discount rate.
    pub verdict: String,
    /// Warnings worth surfacing (multiple IRR, no IRR, no payback…).
    pub notes: Vec<String>,
    pub rows: Vec<Row>,
}

fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// Series spacing → periods per year.
pub fn periods_per_year(period: &str) -> Result<f64, String> {
    match period.trim().to_ascii_lowercase().as_str() {
        "annual" | "" => Ok(1.0),
        "semiannual" => Ok(2.0),
        "quarterly" => Ok(4.0),
        "monthly" => Ok(12.0),
        "weekly" => Ok(52.0),
        other => Err(format!(
            "period must be one of annual, semiannual, quarterly, monthly, weekly — got '{other}'"
        )),
    }
}

fn timing_kind(timing: &str) -> Result<&'static str, String> {
    match timing.trim().to_ascii_lowercase().as_str() {
        "end" | "" => Ok("end"),
        "begin" | "beginning" | "start" => Ok("begin"),
        other => Err(format!(
            "timing must be 'end' (ordinary, flows land at period end) or 'begin' (annuity due) — got '{other}'"
        )),
    }
}

/// Is every comma in `s` a valid thousands separator (a 1–3 digit lead group
/// followed by groups of exactly 3 digits)? Keeps `1,234` a single number while
/// `100,200` still splits into two cash flows.
fn commas_are_thousands(s: &str) -> bool {
    let b: Vec<char> = s.chars().collect();
    let first = match b.iter().position(|c| *c == ',') {
        Some(i) => i,
        None => return false,
    };
    let lead = &b[..first];
    let digits_start = usize::from(matches!(lead.first(), Some('+' | '-')));
    let lead_digits = &lead[digits_start..];
    if lead_digits.is_empty()
        || lead_digits.len() > 3
        || !lead_digits.iter().all(|c| c.is_ascii_digit())
    {
        return false;
    }
    for (i, c) in b.iter().enumerate() {
        if *c != ',' {
            continue;
        }
        let after: Vec<char> = b[i + 1..].iter().take(4).copied().collect();
        if after.len() < 3 || !after[..3].iter().all(|c| c.is_ascii_digit()) {
            return false;
        }
        if after.len() == 4 && after[3] != '.' && after[3] != ',' {
            return false;
        }
    }
    true
}

/// Parse one numeric cell, tolerating what people paste out of a spreadsheet:
/// currency symbols, thousands separators, and accounting parentheses.
pub fn parse_number(raw: &str) -> Option<f64> {
    let mut t = raw.trim();
    if t.is_empty() {
        return None;
    }
    let mut negate = false;
    if t.starts_with('(') && t.ends_with(')') && t.len() >= 3 {
        negate = true;
        t = t[1..t.len() - 1].trim();
    }
    let mut s: String = t
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '$' | '€' | '£' | '¥' | '₹' | '₽' | '"' | '\'' | '\u{a0}' | '\u{202f}'
            )
        })
        .collect();
    s = s.trim().to_string();
    if s.is_empty() {
        return None;
    }
    if s.contains(',') {
        if !commas_are_thousands(&s) {
            return None;
        }
        s = s.replace(',', "");
    }
    s = s.replace(['_', ' '], "");
    let v: f64 = s.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    Some(if negate { -v } else { v })
}

/// Split the pasted series into raw tokens. Newlines, semicolons, tabs and
/// spaces always separate; a comma separates unless it is a thousands
/// separator inside one number.
fn tokenize(raw: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    for chunk in raw.split(['\n', '\r', ';', '\t', ' ']) {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        if !chunk.contains(',') || commas_are_thousands(chunk) {
            tokens.push(chunk.to_string());
            continue;
        }
        // Split on the commas that are NOT thousands separators: a comma is a
        // thousands separator only when exactly three digits follow it and a
        // digit precedes it.
        let chars: Vec<char> = chunk.chars().collect();
        let mut cur = String::new();
        for (i, c) in chars.iter().enumerate() {
            if *c == ',' {
                let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
                let after: Vec<char> = chars[i + 1..].iter().take(4).copied().collect();
                let thousands = prev_digit
                    && after.len() >= 3
                    && after[..3].iter().all(|d| d.is_ascii_digit())
                    && (after.len() < 4 || !after[3].is_ascii_digit());
                if thousands {
                    cur.push(',');
                    continue;
                }
                let t = cur.trim().to_string();
                if !t.is_empty() {
                    tokens.push(t);
                }
                cur.clear();
                continue;
            }
            cur.push(*c);
        }
        let t = cur.trim().to_string();
        if !t.is_empty() {
            tokens.push(t);
        }
    }
    tokens
}

/// Parse the pasted cash-flow series into flows, expanding the `Nx<value>`
/// repeat shorthand (`12x2500` = twelve flows of 2,500; `*` works too).
pub fn parse_flows(raw: &str) -> Result<Vec<f64>, String> {
    let mut flows: Vec<f64> = Vec::new();
    for token in tokenize(raw) {
        if let Some((count, value)) = split_repeat(&token) {
            if count == 0 {
                return Err(format!(
                    "repeat count must be at least 1 in '{token}' — write e.g. 12x2500"
                ));
            }
            if flows.len() + count > MAX_FLOWS {
                return Err(too_many(flows.len() + count));
            }
            for _ in 0..count {
                flows.push(value);
            }
            continue;
        }
        let v = parse_number(&token).ok_or_else(|| {
            format!(
                "'{token}' is not a cash flow — write plain numbers like -500000, 120000, (1,234) or a repeat like 12x2500"
            )
        })?;
        flows.push(v);
        if flows.len() > MAX_FLOWS {
            return Err(too_many(flows.len()));
        }
    }
    Ok(flows)
}

fn too_many(n: usize) -> String {
    format!("the series expands to {n} cash flows, the maximum is {MAX_FLOWS}")
}

/// `12x2500` / `12*2500` → `(12, 2500.0)`. Returns `None` when the token is not
/// a repeat (so it can be parsed as a plain number instead).
fn split_repeat(token: &str) -> Option<(usize, f64)> {
    let lower = token.to_ascii_lowercase();
    let idx = lower.find(['x', '*'])?;
    let (head, tail) = (&token[..idx], &token[idx + 1..]);
    let count: usize = head.trim().parse().ok()?;
    let value = parse_number(tail)?;
    Some((count, value))
}

/// Net present value of `flows` at a per-period `rate`, with period 0
/// undiscounted and `begin` timing shifting later flows one period earlier.
pub fn npv_at(flows: &[f64], rate: f64, begin: bool) -> f64 {
    let mut total = 0.0;
    for (t, f) in flows.iter().enumerate() {
        total += f / (1.0 + rate).powf(discount_exp(t, begin));
    }
    total
}

fn discount_exp(t: usize, begin: bool) -> f64 {
    if begin && t > 0 {
        (t - 1) as f64
    } else {
        t as f64
    }
}

/// Internal rate of return per period: the rate where NPV crosses zero. Scans a
/// wide grid for a sign change and then bisects, which converges for every
/// bracketed series (Newton's method can diverge on lumpy real-world flows).
/// Returns `None` when no bracket exists (e.g. every flow has the same sign).
pub fn irr(flows: &[f64], begin: bool) -> Option<f64> {
    if flows.len() < 2 {
        return None;
    }
    let f = |r: f64| npv_at(flows, r, begin);
    // Grid from just above -100% up to +10,000% per period.
    let mut prev_rate = -0.9999;
    let mut prev = f(prev_rate);
    let steps = 4000;
    for i in 1..=steps {
        let rate = -0.9999 + (i as f64 / steps as f64) * (100.0 + 0.9999);
        let v = f(rate);
        if v == 0.0 {
            return Some(round6(rate));
        }
        if prev.is_finite() && v.is_finite() && (prev < 0.0) != (v < 0.0) {
            let (mut lo, mut hi) = (prev_rate, rate);
            let lo_neg = prev < 0.0;
            for _ in 0..200 {
                let mid = (lo + hi) / 2.0;
                let m = f(mid);
                if m == 0.0 {
                    return Some(round6(mid));
                }
                if (m < 0.0) == lo_neg {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some(round6((lo + hi) / 2.0));
        }
        prev_rate = rate;
        prev = v;
    }
    None
}

/// Modified IRR per period: outflows financed at `finance_rate`, inflows
/// reinvested at `reinvest_rate`, both per period. `None` unless there is at
/// least one inflow and one outflow.
pub fn mirr(flows: &[f64], finance_rate: f64, reinvest_rate: f64, begin: bool) -> Option<f64> {
    let n = flows.len().checked_sub(1)? as f64;
    if n <= 0.0 {
        return None;
    }
    let last = discount_exp(flows.len() - 1, begin);
    let mut pv_neg = 0.0;
    let mut fv_pos = 0.0;
    for (t, f) in flows.iter().enumerate() {
        let e = discount_exp(t, begin);
        if *f < 0.0 {
            pv_neg += f / (1.0 + finance_rate).powf(e);
        } else if *f > 0.0 {
            fv_pos += f * (1.0 + reinvest_rate).powf(last - e);
        }
    }
    if pv_neg >= 0.0 || fv_pos <= 0.0 || last <= 0.0 {
        return None;
    }
    let r = (fv_pos / -pv_neg).powf(1.0 / last) - 1.0;
    if r.is_finite() {
        Some(round6(r))
    } else {
        None
    }
}

/// Fractional periods until a running total first turns non-negative, linearly
/// interpolating inside the crossing period.
fn crossing(cumulative: &[f64], exps: &[f64]) -> Option<f64> {
    for (i, c) in cumulative.iter().enumerate() {
        if *c >= 0.0 {
            if i == 0 {
                return Some(0.0);
            }
            let prev = cumulative[i - 1];
            if prev >= 0.0 {
                return Some(exps[i]);
            }
            let step = c - prev;
            let frac = if step == 0.0 { 0.0 } else { -prev / step };
            let span = exps[i] - exps[i - 1];
            return Some(round6(exps[i - 1] + frac * span));
        }
    }
    None
}

/// Run the full analysis.
///
/// * `cash_flows` — the pasted series (see [`parse_flows`]).
/// * `initial_investment` — upfront cost as a POSITIVE number; 0 means the
///   series itself already starts at period 0.
/// * `discount_rate` — nominal annual rate in PERCENT (10 = 10%/yr).
/// * `period` — series spacing: annual/semiannual/quarterly/monthly/weekly.
/// * `timing` — `end` (ordinary) or `begin` (annuity due).
pub fn analyze(
    cash_flows: &str,
    initial_investment: f64,
    discount_rate: f64,
    period: &str,
    timing: &str,
) -> Result<Analysis, String> {
    let ppy = periods_per_year(period)?;
    let timing = timing_kind(timing)?;
    let begin = timing == "begin";
    if !discount_rate.is_finite() {
        return Err("discount_rate must be a finite number of percent, e.g. 10".into());
    }
    if discount_rate <= -100.0 {
        return Err(format!(
            "discount_rate must be greater than -100%, got {discount_rate}"
        ));
    }
    if !initial_investment.is_finite() {
        return Err("initial_investment must be a finite number".into());
    }

    let mut flows = parse_flows(cash_flows)?;
    if initial_investment != 0.0 {
        if flows.len() + 1 > MAX_FLOWS {
            return Err(too_many(flows.len() + 1));
        }
        flows.insert(0, -initial_investment);
    }
    if flows.len() < 2 {
        return Err(format!(
            "need at least 2 cash flows (period 0 plus one later period), got {} — add the future cash flows, e.g. -500000, 120000, 120000",
            flows.len()
        ));
    }

    let annual_rate = discount_rate / 100.0;
    let period_rate = annual_rate / ppy;
    if period_rate <= -1.0 {
        return Err(format!(
            "a {discount_rate}% annual rate over {ppy} periods per year gives a per-period rate of -100% or worse, which cannot be discounted"
        ));
    }

    let mut rows: Vec<Row> = Vec::with_capacity(flows.len());
    let mut cum_pv = 0.0;
    let mut cum_cf = 0.0;
    let mut pv_in = 0.0;
    let mut pv_out = 0.0;
    let mut total_in = 0.0;
    let mut total_out = 0.0;
    for (t, f) in flows.iter().enumerate() {
        let e = discount_exp(t, begin);
        let factor = 1.0 / (1.0 + period_rate).powf(e);
        let pv = f * factor;
        cum_pv += pv;
        cum_cf += f;
        if *f >= 0.0 {
            pv_in += pv;
            total_in += f;
        } else {
            pv_out += pv;
            total_out += f;
        }
        rows.push(Row {
            period: t,
            cash_flow: *f,
            discount_periods: e,
            discount_factor: (factor * 1e10).round() / 1e10,
            present_value: round6(pv),
            cumulative_pv: round6(cum_pv),
            cumulative_cash_flow: round6(cum_cf),
        });
    }

    let npv = cum_pv;
    let exps: Vec<f64> = rows.iter().map(|r| r.discount_periods).collect();
    let cum_cf_col: Vec<f64> = rows.iter().map(|r| r.cumulative_cash_flow).collect();
    let cum_pv_col: Vec<f64> = rows.iter().map(|r| r.cumulative_pv).collect();
    let payback = crossing(&cum_cf_col, &exps);
    let disc_payback = crossing(&cum_pv_col, &exps);

    let mut sign_changes = 0usize;
    let mut last_sign: Option<bool> = None;
    for f in &flows {
        if *f == 0.0 {
            continue;
        }
        let pos = *f > 0.0;
        if let Some(prev) = last_sign {
            if prev != pos {
                sign_changes += 1;
            }
        }
        last_sign = Some(pos);
    }

    let irr_period = irr(&flows, begin);
    let irr_annual = irr_period.and_then(|r| {
        let a = (1.0 + r).powf(ppy) - 1.0;
        if a.is_finite() {
            Some(round6(a))
        } else {
            None
        }
    });
    let mirr_period = mirr(&flows, period_rate, period_rate, begin);
    let mirr_annual = mirr_period.and_then(|r| {
        let a = (1.0 + r).powf(ppy) - 1.0;
        if a.is_finite() {
            Some(round6(a))
        } else {
            None
        }
    });

    let mut notes: Vec<String> = Vec::new();
    if sign_changes > 1 {
        notes.push(format!(
            "The cash flows change sign {sign_changes} times, so more than one rate can zero the NPV. The first root found is reported when a root is bracketed — treat NPV at your own discount rate, or MIRR, as the decision metric."
        ));
    }
    if irr_period.is_none() {
        notes.push(
            "No IRR exists for this series: NPV never crosses zero over the searched range, which normally means every cash flow has the same sign. Add the upfront cost as a negative flow (or fill in the initial investment field).".to_string(),
        );
    }
    if payback.is_none() {
        notes.push(
            "The undiscounted cash flows never recover the outlay, so there is no payback period."
                .to_string(),
        );
    } else if disc_payback.is_none() {
        notes.push(
            "The discounted cash flows never recover the outlay, so there is no discounted payback period even though the plain payback is reached.".to_string(),
        );
    }
    if begin {
        notes.push(
            "Beginning-of-period timing: every flow from period 1 on is discounted one period less than its index (period 0 is never discounted).".to_string(),
        );
    }
    notes.push(
        "MIRR finances the outflows and reinvests the inflows at the discount rate you entered. Educational arithmetic only, not financial advice.".to_string(),
    );

    let verdict = if npv > 0.0 {
        format!(
            "NPV is positive at a {}% annual discount rate, so the series earns more than the required return.",
            trim_num(discount_rate)
        )
    } else if npv < 0.0 {
        format!(
            "NPV is negative at a {}% annual discount rate, so the series earns less than the required return.",
            trim_num(discount_rate)
        )
    } else {
        format!(
            "NPV is exactly zero at a {}% annual discount rate — that rate is the break-even return.",
            trim_num(discount_rate)
        )
    };

    Ok(Analysis {
        count: flows.len(),
        last_period: flows.len() - 1,
        period: {
            let p = period.trim().to_ascii_lowercase();
            if p.is_empty() {
                "annual".to_string()
            } else {
                p
            }
        },
        periods_per_year: ppy,
        timing: timing.to_string(),
        annual_discount_rate: round6(annual_rate),
        period_discount_rate: round6(period_rate),
        npv: round6(npv),
        pv_inflows: round6(pv_in),
        pv_outflows: round6(pv_out),
        profitability_index: if pv_out < 0.0 {
            Some(round6(pv_in / -pv_out))
        } else {
            None
        },
        irr_period,
        irr_annual,
        mirr_period,
        mirr_annual,
        total_cash_flow: round6(total_in + total_out),
        total_inflows: round6(total_in),
        total_outflows: round6(total_out),
        payback_periods: payback,
        discounted_payback_periods: disc_payback,
        sign_changes,
        verdict,
        notes,
        rows,
    })
}

// ---------------------------------------------------------------- formatting

fn group_thousands(int_part: &str) -> String {
    let neg = int_part.starts_with('-');
    let digits = if neg { &int_part[1..] } else { int_part };
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if neg {
        format!("-{out}")
    } else {
        out
    }
}

/// Trim a plain number for prose: `10.0` → `10`, `7.25` → `7.25`.
fn trim_num(v: f64) -> String {
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Money with thousands separators, a fixed number of decimals and the
/// currency symbol in front (negatives keep the sign before the symbol).
pub fn fmt_money(v: f64, dp: usize, currency: &str) -> String {
    let rounded = format!("{:.*}", dp, v.abs());
    let body = match rounded.split_once('.') {
        Some((i, f)) => format!("{}.{}", group_thousands(i), f),
        None => group_thousands(&rounded),
    };
    let sign = if v < 0.0 && rounded.chars().any(|c| c.is_ascii_digit() && c != '0') {
        "-"
    } else {
        ""
    };
    format!("{sign}{currency}{body}")
}

/// A decimal rate as a signed percentage with `dp` decimals.
pub fn fmt_pct(v: f64, dp: usize) -> String {
    format!("{:+.*}%", dp, v * 100.0)
}

fn fmt_periods(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

fn lpad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(w - n))
    }
}

fn period_word(period: &str) -> &'static str {
    match period {
        "semiannual" => "half-year",
        "quarterly" => "quarter",
        "monthly" => "month",
        "weekly" => "week",
        _ => "year",
    }
}

fn render_table(a: &Analysis, dp: usize, currency: &str) -> String {
    let headers = [
        "Period",
        "Cash flow",
        "Factor",
        "Present value",
        "Cumulative PV",
    ];
    let mut cells: Vec<[String; 5]> = Vec::with_capacity(a.rows.len());
    for r in &a.rows {
        cells.push([
            r.period.to_string(),
            fmt_money(r.cash_flow, dp, currency),
            format!("{:.6}", r.discount_factor),
            fmt_money(r.present_value, dp, currency),
            fmt_money(r.cumulative_pv, dp, currency),
        ]);
    }
    let mut widths = [0usize; 5];
    for (i, h) in headers.iter().enumerate() {
        widths[i] = h.chars().count();
    }
    for row in &cells {
        for (i, c) in row.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    let mut out = String::new();
    out.push_str(&pad(headers[0], widths[0]));
    for i in 1..headers.len() {
        out.push_str("  ");
        out.push_str(&lpad(headers[i], widths[i]));
    }
    out.push('\n');
    out.push_str(&"-".repeat(widths.iter().sum::<usize>() + 2 * (headers.len() - 1)));
    out.push('\n');
    for row in &cells {
        out.push_str(&pad(&row[0], widths[0]));
        for i in 1..row.len() {
            out.push_str("  ");
            out.push_str(&lpad(&row[i], widths[i]));
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn render_csv(a: &Analysis) -> String {
    let mut out =
        String::from("period,cash_flow,discount_periods,discount_factor,present_value,cumulative_pv,cumulative_cash_flow\n");
    for r in &a.rows {
        out.push_str(&format!(
            "{},{},{},{:.10},{:.6},{:.6},{:.6}\n",
            r.period,
            r.cash_flow,
            r.discount_periods,
            r.discount_factor,
            r.present_value,
            r.cumulative_pv,
            r.cumulative_cash_flow
        ));
    }
    out.trim_end().to_string()
}

fn render_summary(a: &Analysis, dp: usize, currency: &str) -> String {
    let word = period_word(&a.period);
    let mut out = String::new();
    out.push_str(&format!("NPV: {}\n", fmt_money(a.npv, dp, currency)));
    out.push_str(&format!(
        "IRR: {}\n",
        match (a.irr_period, a.irr_annual) {
            (Some(p), Some(ann)) if a.periods_per_year > 1.0 => format!(
                "{} per {} ({} annualized)",
                fmt_pct(p, dp),
                word,
                fmt_pct(ann, dp)
            ),
            (Some(p), _) => format!("{} per year", fmt_pct(p, dp)),
            _ => "not defined for these cash flows".to_string(),
        }
    ));
    out.push_str(&format!(
        "MIRR: {}\n",
        match (a.mirr_period, a.mirr_annual) {
            (Some(p), Some(ann)) if a.periods_per_year > 1.0 => format!(
                "{} per {} ({} annualized)",
                fmt_pct(p, dp),
                word,
                fmt_pct(ann, dp)
            ),
            (Some(p), _) => format!("{} per year", fmt_pct(p, dp)),
            _ => "not defined for these cash flows".to_string(),
        }
    ));
    out.push_str(&format!(
        "Profitability index: {}\n",
        match a.profitability_index {
            Some(pi) => format!("{pi:.4}"),
            None => "not defined (no outflow)".to_string(),
        }
    ));
    out.push_str(&format!(
        "Payback: {}\n",
        match a.payback_periods {
            Some(p) => format!("{} {}s", fmt_periods(p), word),
            None => "never".to_string(),
        }
    ));
    out.push_str(&format!(
        "Discounted payback: {}\n",
        match a.discounted_payback_periods {
            Some(p) => format!("{} {}s", fmt_periods(p), word),
            None => "never".to_string(),
        }
    ));
    out.push_str(&format!(
        "Total cash flow: {} (in {} / out {})\n",
        fmt_money(a.total_cash_flow, dp, currency),
        fmt_money(a.total_inflows, dp, currency),
        fmt_money(a.total_outflows, dp, currency)
    ));
    out.push_str(&format!(
        "PV of inflows: {} · PV of outflows: {}\n",
        fmt_money(a.pv_inflows, dp, currency),
        fmt_money(a.pv_outflows, dp, currency)
    ));
    out.push_str(&format!(
        "Discount rate: {} per year ({} per {}, {} periods per year, {}-of-period timing)\n",
        fmt_pct(a.annual_discount_rate, dp),
        fmt_pct(a.period_discount_rate, dp),
        word,
        trim_num(a.periods_per_year),
        a.timing
    ));
    out.push_str(&format!(
        "Cash flows: {} periods (0 to {}), {} sign change(s)\n",
        a.count, a.last_period, a.sign_changes
    ));
    out.push('\n');
    out.push_str(&a.verdict);
    out.push_str("\n\n");
    out.push_str(&render_table(a, dp, currency));
    for n in &a.notes {
        out.push_str("\n\nNote: ");
        out.push_str(n);
    }
    out
}

/// Analyse and render in the requested `output` shape.
#[allow(clippy::too_many_arguments)]
pub fn run(
    cash_flows: &str,
    initial_investment: f64,
    discount_rate: f64,
    period: &str,
    timing: &str,
    decimals: i64,
    currency: &str,
    output: &str,
) -> Result<String, String> {
    if !(0..=10).contains(&decimals) {
        return Err(format!("decimals must be between 0 and 10, got {decimals}"));
    }
    let dp = decimals as usize;
    let a = analyze(
        cash_flows,
        initial_investment,
        discount_rate,
        period,
        timing,
    )?;
    match output.trim().to_ascii_lowercase().as_str() {
        "summary" | "" => Ok(render_summary(&a, dp, currency)),
        "table" => Ok(render_table(&a, dp, currency)),
        "csv" => Ok(render_csv(&a)),
        "json" => serde_json::to_string_pretty(&a).map_err(|e| e.to_string()),
        other => Err(format!(
            "output must be one of summary, table, csv, json — got '{other}'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, eps: f64) {
        assert!((a - b).abs() < eps, "expected {b}, got {a}");
    }

    #[test]
    fn npv_matches_textbook_example() {
        // -100000 then 30000 x 5 at 8% → NPV ≈ 19,781 (the worked example every
        // competitor uses).
        let a = analyze(
            "-100000, 30000, 30000, 30000, 30000, 30000",
            0.0,
            8.0,
            "annual",
            "end",
        )
        .unwrap();
        approx(a.npv, 19781.0, 1.0);
        approx(a.irr_period.unwrap(), 0.1524, 0.0005);
        assert_eq!(a.count, 6);
        assert_eq!(a.sign_changes, 1);
    }

    #[test]
    fn initial_investment_field_prepends_period_zero() {
        let with_field = analyze("30000, 30000, 30000", 100000.0, 8.0, "annual", "end").unwrap();
        let inline = analyze("-100000, 30000, 30000, 30000", 0.0, 8.0, "annual", "end").unwrap();
        assert_eq!(with_field.count, inline.count);
        approx(with_field.npv, inline.npv, 1e-9);
    }

    #[test]
    fn irr_zeroes_the_npv() {
        let a = analyze(
            "-500000, 120000, 180000, 260000",
            0.0,
            10.0,
            "annual",
            "end",
        )
        .unwrap();
        let r = a.irr_period.unwrap();
        let flows = parse_flows("-500000, 120000, 180000, 260000").unwrap();
        approx(npv_at(&flows, r, false), 0.0, 0.5);
    }

    #[test]
    fn irr_is_none_when_no_sign_change() {
        let a = analyze("1000, 2000, 3000", 0.0, 10.0, "annual", "end").unwrap();
        assert!(a.irr_period.is_none());
        assert!(a.notes.iter().any(|n| n.contains("No IRR exists")));
    }

    #[test]
    fn monthly_irr_annualizes() {
        // 12 monthly inflows of 1000 against 11000 upfront.
        let a = analyze("-11000, 12x1000", 0.0, 12.0, "monthly", "end").unwrap();
        assert_eq!(a.count, 13);
        approx(a.periods_per_year, 12.0, 1e-9);
        let p = a.irr_period.unwrap();
        approx(a.irr_annual.unwrap(), (1.0 + p).powf(12.0) - 1.0, 1e-6);
        // 12% a year over monthly periods = 1% a month.
        approx(a.period_discount_rate, 0.01, 1e-9);
    }

    #[test]
    fn begin_timing_discounts_one_period_less() {
        let end = analyze("-1000, 600, 600", 0.0, 10.0, "annual", "end").unwrap();
        let begin = analyze("-1000, 600, 600", 0.0, 10.0, "annual", "begin").unwrap();
        assert!(begin.npv > end.npv, "annuity due is worth more");
        // end:   -1000 + 600/1.1 + 600/1.21
        approx(end.npv, -1000.0 + 600.0 / 1.1 + 600.0 / 1.21, 1e-6);
        // begin: -1000 + 600 + 600/1.1
        approx(begin.npv, -1000.0 + 600.0 + 600.0 / 1.1, 1e-6);
        assert_eq!(begin.rows[1].discount_periods, 0.0);
        assert_eq!(begin.rows[2].discount_periods, 1.0);
    }

    #[test]
    fn payback_interpolates_inside_the_period() {
        // -1000 then 500 a year: cumulative hits zero exactly at period 2.
        let a = analyze("-1000, 500, 500, 500", 0.0, 10.0, "annual", "end").unwrap();
        approx(a.payback_periods.unwrap(), 2.0, 1e-9);
        // Discounted payback comes later than plain payback.
        assert!(a.discounted_payback_periods.unwrap() > 2.0);
    }

    #[test]
    fn multiple_sign_changes_are_flagged() {
        let a = analyze("-1000, 3000, -2500", 0.0, 10.0, "annual", "end").unwrap();
        assert_eq!(a.sign_changes, 2);
        assert!(a.notes.iter().any(|n| n.contains("change sign")));
    }

    #[test]
    fn profitability_index_and_totals() {
        let a = analyze(
            "-100000, 30000, 30000, 30000, 30000, 30000",
            0.0,
            8.0,
            "annual",
            "end",
        )
        .unwrap();
        approx(a.total_cash_flow, 50000.0, 1e-6);
        approx(a.total_inflows, 150000.0, 1e-6);
        approx(a.total_outflows, -100000.0, 1e-6);
        approx(a.profitability_index.unwrap(), 1.1978, 0.001);
    }

    #[test]
    fn parses_currency_thousands_and_accounting_negatives() {
        let f = parse_flows("($500,000)\n$120,000\n120000\n1.5e5").unwrap();
        assert_eq!(f, vec![-500000.0, 120000.0, 120000.0, 150000.0]);
    }

    #[test]
    fn repeat_shorthand_expands() {
        assert_eq!(
            parse_flows("-1000, 3x250").unwrap(),
            vec![-1000.0, 250.0, 250.0, 250.0]
        );
        assert_eq!(parse_flows("2*7.5").unwrap(), vec![7.5, 7.5]);
    }

    #[test]
    fn comma_thousands_do_not_split_values() {
        assert_eq!(parse_flows("1,000,-2,000").unwrap(), vec![1000.0, -2000.0]);
    }

    #[test]
    fn rejects_non_numeric_flow() {
        let err = parse_flows("-1000, abc").unwrap_err();
        assert!(err.contains("abc"), "error names the bad token: {err}");
    }

    #[test]
    fn rejects_single_flow() {
        let err = analyze("-1000", 0.0, 10.0, "annual", "end").unwrap_err();
        assert!(err.contains("at least 2 cash flows"), "{err}");
    }

    #[test]
    fn rejects_bad_period_and_timing_and_decimals() {
        assert!(analyze("-1, 2", 0.0, 10.0, "fortnightly", "end")
            .unwrap_err()
            .contains("period must be one of"));
        assert!(analyze("-1, 2", 0.0, 10.0, "annual", "middle")
            .unwrap_err()
            .contains("timing must be"));
        assert!(run("-1, 2", 0.0, 10.0, "annual", "end", 99, "$", "summary")
            .unwrap_err()
            .contains("decimals must be"));
    }

    #[test]
    fn rejects_oversized_series() {
        let err = parse_flows("-1000, 5000x1").unwrap_err();
        assert!(err.contains("maximum is"), "{err}");
    }

    #[test]
    fn accepts_exact_cap_boundary() {
        let flows = parse_flows(&format!("-1000, {}x1", MAX_FLOWS - 1)).unwrap();
        assert_eq!(flows.len(), MAX_FLOWS);
        assert!(parse_flows(&format!("-1000, {}x1", MAX_FLOWS)).is_err());
    }

    #[test]
    fn output_shapes_render() {
        let csv = run("-1000, 600, 600", 0.0, 10.0, "annual", "end", 2, "$", "csv").unwrap();
        assert!(csv.starts_with("period,cash_flow,"));
        assert_eq!(csv.lines().count(), 4);
        let json = run(
            "-1000, 600, 600",
            0.0,
            10.0,
            "annual",
            "end",
            2,
            "$",
            "json",
        )
        .unwrap();
        assert!(json.contains("\"npv\""));
        let table = run(
            "-1000, 600, 600",
            0.0,
            10.0,
            "annual",
            "end",
            2,
            "$",
            "table",
        )
        .unwrap();
        assert!(table.starts_with("Period"));
        assert!(run("-1000, 600", 0.0, 10.0, "annual", "end", 2, "$", "xml")
            .unwrap_err()
            .contains("output must be one of"));
    }

    #[test]
    fn money_formatting_uses_the_currency_symbol() {
        assert_eq!(fmt_money(1234.5, 2, "$"), "$1,234.50");
        assert_eq!(fmt_money(-1234.5, 2, "€"), "-€1,234.50");
        assert_eq!(fmt_money(0.0, 2, "$"), "$0.00");
        assert_eq!(fmt_money(1234.6, 0, ""), "1,235");
    }

    #[test]
    fn mirr_sits_between_the_discount_rate_and_the_irr() {
        let a = analyze(
            "-100000, 30000, 30000, 30000, 30000, 30000",
            0.0,
            8.0,
            "annual",
            "end",
        )
        .unwrap();
        let m = a.mirr_period.unwrap();
        assert!(
            m > 0.08 && m < a.irr_period.unwrap(),
            "MIRR {m} out of range"
        );
    }
}
