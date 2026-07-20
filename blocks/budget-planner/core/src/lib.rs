//! budget-planner core — pure compute, shared by the chat skill block and the web page.
//! Builds a 50/30/20 (or custom-split) budget, or a zero-based budget from expense
//! categories, and reports what's left to allocate. All money arithmetic is done in
//! integer cents so buckets and totals always reconcile to the penny. No wafer or
//! wasm-bindgen deps.

use serde::Serialize;

/// Hard caps (stated on the page + in error messages).
const MAX_EXPENSE_LINES: usize = 100;
const MAX_NAME_LEN: usize = 60;
const MAX_AMOUNT_CENTS: i64 = 100_000_000_000; // $1,000,000,000.00
const MAX_INCOME: f64 = 1_000_000_000.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BucketKind {
    Needs,
    Wants,
    Savings,
}

impl BucketKind {
    fn name(self) -> &'static str {
        match self {
            BucketKind::Needs => "Needs",
            BucketKind::Wants => "Wants",
            BucketKind::Savings => "Savings",
        }
    }
}

#[derive(Serialize, Debug)]
pub struct Bucket {
    pub name: String,
    /// Share of income in percent, as given in `split` (e.g. 50.0).
    pub share_pct: f64,
    /// Target dollars for this bucket (targets always sum exactly to income).
    pub target: f64,
    /// Sum of tagged expenses planned into this bucket (only when expenses given).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planned: Option<f64>,
    /// target − planned (negative = over target; only when expenses given).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<f64>,
}

#[derive(Serialize, Debug)]
pub struct Category {
    pub name: String,
    pub amount: f64,
    /// Share of income in percent, rounded to 1 decimal.
    pub share_pct: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct Plan {
    pub mode: String,
    pub income: f64,
    pub currency: String,
    /// Normalized needs/wants/savings shares, e.g. "50/30/20" (rule mode only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buckets: Option<Vec<Bucket>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<Category>,
    pub total_planned: f64,
    pub left_to_allocate: f64,
    /// "surplus" | "deficit" | "balanced" (from left_to_allocate).
    pub status: String,
    pub summary: String,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a money amount ("1200", "$1,200.50", "1_200") to whole cents.
fn parse_amount_cents(raw: &str, cur: &str) -> Result<i64, String> {
    let mut s = raw.trim();
    if !cur.is_empty() {
        s = s.trim_start_matches(cur).trim();
    }
    let cleaned: String = s.chars().filter(|c| !matches!(c, ',' | '_')).collect();
    let cleaned = cleaned
        .trim()
        .trim_start_matches(['$', '€', '£', '¥', '₹'])
        .trim_start_matches('+')
        .trim()
        .to_string();
    if cleaned.starts_with('-') {
        return Err(format!("`{}` is negative — amounts must be 0 or more", raw.trim()));
    }
    let (int_part, frac_part) = match cleaned.split_once('.') {
        Some((a, b)) => (a, b),
        None => (cleaned.as_str(), ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(format!("`{}` is not an amount", raw.trim()));
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!(
            "`{}` is not an amount — use digits like 1200 or 1,200.50",
            raw.trim()
        ));
    }
    if frac_part.len() > 2 {
        return Err(format!(
            "`{}` has more than 2 decimal places — amounts are whole cents",
            raw.trim()
        ));
    }
    if int_part.len() > 10 {
        return Err(format!("`{}` is too large (max 1,000,000,000)", raw.trim()));
    }
    let dollars: i64 = if int_part.is_empty() {
        0
    } else {
        int_part
            .parse()
            .map_err(|_| format!("`{}` is not an amount", raw.trim()))?
    };
    let cents_frac: i64 = match frac_part.len() {
        0 => 0,
        1 => frac_part.parse::<i64>().unwrap_or(0) * 10,
        _ => frac_part.parse::<i64>().unwrap_or(0),
    };
    let cents = dollars * 100 + cents_frac;
    if cents > MAX_AMOUNT_CENTS {
        return Err(format!("`{}` is too large (max 1,000,000,000)", raw.trim()));
    }
    Ok(cents)
}

fn parse_bucket_tag(tok: &str) -> Option<BucketKind> {
    let t = tok
        .trim_matches(|c| matches!(c, '(' | ')' | '[' | ']'))
        .to_ascii_lowercase();
    match t.as_str() {
        "needs" | "need" => Some(BucketKind::Needs),
        "wants" | "want" => Some(BucketKind::Wants),
        "savings" | "saving" | "save" | "debt" => Some(BucketKind::Savings),
        _ => None,
    }
}

struct Expense {
    name: String,
    cents: i64,
    bucket: Option<BucketKind>,
}

/// Parse one expense entry: `Name: amount [(needs|wants|savings)]` or
/// `Name amount [tag]`. `=` works like `:`.
fn parse_entry(entry: &str, cur: &str, n: usize) -> Result<Expense, String> {
    let ctx = |msg: &str| format!("expense line {} (`{}`): {}", n, entry.trim(), msg);
    let (name, cents, bucket) = if let Some((name_part, rest)) = entry.split_once([':', '=']) {
        let name = name_part.trim().to_string();
        let mut toks: Vec<&str> = rest.split_whitespace().collect();
        let mut bucket = None;
        if let Some(last) = toks.last() {
            if let Some(b) = parse_bucket_tag(last) {
                bucket = Some(b);
                toks.pop();
            }
        }
        if toks.len() != 1 {
            return Err(ctx(
                "expected `Name: amount` with an optional (needs), (wants) or (savings) tag",
            ));
        }
        let cents = parse_amount_cents(toks[0], cur).map_err(|e| ctx(&e))?;
        (name, cents, bucket)
    } else {
        let mut toks: Vec<&str> = entry.split_whitespace().collect();
        let mut bucket = None;
        if let Some(last) = toks.last() {
            if let Some(b) = parse_bucket_tag(last) {
                bucket = Some(b);
                toks.pop();
            }
        }
        let amt_tok = toks.pop().ok_or_else(|| {
            ctx("expected `Name: amount` with an optional (needs), (wants) or (savings) tag")
        })?;
        let cents = parse_amount_cents(amt_tok, cur).map_err(|e| ctx(&e))?;
        let name = toks.join(" ");
        (name, cents, bucket)
    };
    if name.is_empty() {
        return Err(ctx("missing category name — write `Name: amount`, e.g. `Rent: 1200`"));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(ctx(&format!(
            "category name is too long (max {} characters)",
            MAX_NAME_LEN
        )));
    }
    Ok(Expense { name, cents, bucket })
}

/// Split the expenses text on newlines/semicolons, skip blanks and `#`/`//`
/// comment lines, and parse each entry.
fn parse_expenses(text: &str, cur: &str) -> Result<Vec<Expense>, String> {
    let mut out = Vec::new();
    let mut n = 0usize;
    for raw in text.split(['\n', ';']) {
        let entry = raw.trim();
        if entry.is_empty() || entry.starts_with('#') || entry.starts_with("//") {
            continue;
        }
        n += 1;
        if n > MAX_EXPENSE_LINES {
            return Err(format!("too many expense lines (max {})", MAX_EXPENSE_LINES));
        }
        out.push(parse_entry(entry, cur, n)?);
    }
    Ok(out)
}

/// Parse the needs/wants/savings split ("50/30/20", "60,30,10", "55 25 20",
/// with optional `%` suffixes). Must be three shares summing to 100.
fn parse_split(s: &str) -> Result<[f64; 3], String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok([50.0, 30.0, 20.0]);
    }
    let parts: Vec<&str> = t
        .split(|c: char| c == '/' || c == ',' || c == ':' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 3 {
        return Err(format!(
            "split `{}` must have exactly three shares (needs/wants/savings), e.g. 50/30/20",
            t
        ));
    }
    let mut vals = [0.0f64; 3];
    for (i, p) in parts.iter().enumerate() {
        let v: f64 = p
            .trim_end_matches('%')
            .parse()
            .map_err(|_| format!("split share `{}` is not a number", p))?;
        if !v.is_finite() || !(0.0..=100.0).contains(&v) {
            return Err(format!("split share `{}` must be between 0 and 100", p));
        }
        vals[i] = v;
    }
    let sum: f64 = vals.iter().sum();
    if (sum - 100.0).abs() > 0.01 {
        return Err(format!(
            "split shares must add up to 100 (needs {} + wants {} + savings {} = {})",
            fmt_share(vals[0]),
            fmt_share(vals[1]),
            fmt_share(vals[2]),
            fmt_share(sum)
        ));
    }
    Ok(vals)
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Format cents as money with thousands separators: 225000 → "$2,250.00";
/// -8000 → "-$80.00".
fn fmt_money(cents: i64, cur: &str) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    let dollars = abs / 100;
    let frac = abs % 100;
    let digits = dollars.to_string();
    let mut int_str = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            int_str.push(',');
        }
        int_str.push(c);
    }
    format!("{}{}{}.{:02}", sign, cur, int_str, frac)
}

/// Format a split share trimming trailing zeros: 50 → "50", 12.5 → "12.5".
fn fmt_share(p: f64) -> String {
    let s = format!("{:.2}", p);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Percent of income rounded to 1 decimal.
fn pct_of(part: i64, whole: i64) -> f64 {
    ((part as f64) * 1000.0 / (whole as f64)).round() / 10.0
}

fn cents_to_f64(c: i64) -> f64 {
    (c as f64) / 100.0
}

// ---------------------------------------------------------------------------
// Planning
// ---------------------------------------------------------------------------

/// Build the budget plan. `mode` is `50-30-20` (default) or `zero-based`;
/// empty `split`/`currency` take their defaults ("50/30/20", "$").
pub fn plan(
    income: f64,
    mode: &str,
    split: &str,
    expenses: &str,
    currency: &str,
) -> Result<Plan, String> {
    if !income.is_finite() || income <= 0.0 {
        return Err(
            "income must be a positive number — your monthly take-home (after-tax) pay, e.g. 4500"
                .into(),
        );
    }
    if income > MAX_INCOME {
        return Err("income is too large (max 1,000,000,000)".into());
    }
    let income_cents = (income * 100.0).round() as i64;
    if income_cents < 1 {
        return Err("income must be at least 0.01".into());
    }
    let cur = {
        let c = currency.trim();
        if c.is_empty() {
            "$".to_string()
        } else if c.chars().count() > 8 {
            return Err("currency symbol is too long (max 8 characters)".into());
        } else {
            c.to_string()
        }
    };
    let mode = match mode.trim() {
        "" | "50-30-20" => "50-30-20",
        "zero-based" => "zero-based",
        other => {
            return Err(format!(
                "unknown mode `{}` — use `50-30-20` or `zero-based`",
                other
            ))
        }
    };
    let items = parse_expenses(expenses, &cur)?;

    if mode == "zero-based" {
        plan_zero_based(income_cents, cur, items)
    } else {
        plan_rule(income_cents, cur, split, items)
    }
}

fn plan_rule(
    income_cents: i64,
    cur: String,
    split: &str,
    items: Vec<Expense>,
) -> Result<Plan, String> {
    let shares = parse_split(split)?;
    let split_disp = format!(
        "{}/{}/{}",
        fmt_share(shares[0]),
        fmt_share(shares[1]),
        fmt_share(shares[2])
    );
    // Cumulative rounding so the three targets always sum exactly to income.
    let inc_f = income_cents as f64;
    let t1 = ((inc_f * shares[0] / 100.0).round() as i64).min(income_cents);
    let t12 =
        (((inc_f * (shares[0] + shares[1]) / 100.0).round()) as i64).clamp(t1, income_cents);
    let targets = [t1, t12 - t1, income_cents - t12];

    let with_expenses = !items.is_empty();
    let mut planned = [0i64; 3];
    let mut categories = Vec::with_capacity(items.len());
    for (i, e) in items.iter().enumerate() {
        let bucket = e.bucket.ok_or_else(|| {
            format!(
                "expense line {} (`{}`) has no bucket tag — in 50-30-20 mode end each line with (needs), (wants) or (savings), e.g. `Rent: 1200 (needs)`",
                i + 1,
                e.name
            )
        })?;
        let idx = match bucket {
            BucketKind::Needs => 0,
            BucketKind::Wants => 1,
            BucketKind::Savings => 2,
        };
        planned[idx] += e.cents;
        categories.push(Category {
            name: e.name.clone(),
            amount: cents_to_f64(e.cents),
            share_pct: pct_of(e.cents, income_cents),
            bucket: Some(bucket.name().to_string()),
        });
    }
    let total_planned: i64 = planned.iter().sum();
    let left = income_cents - total_planned;
    let kinds = [BucketKind::Needs, BucketKind::Wants, BucketKind::Savings];
    let buckets: Vec<Bucket> = (0..3)
        .map(|i| Bucket {
            name: kinds[i].name().to_string(),
            share_pct: shares[i],
            target: cents_to_f64(targets[i]),
            planned: with_expenses.then(|| cents_to_f64(planned[i])),
            left: with_expenses.then(|| cents_to_f64(targets[i] - planned[i])),
        })
        .collect();
    let status = status_of(left);
    let summary = if with_expenses {
        match status {
            "deficit" => format!(
                "{} of {}: planned {}, over budget by {}",
                split_disp,
                fmt_money(income_cents, &cur),
                fmt_money(total_planned, &cur),
                fmt_money(-left, &cur)
            ),
            "balanced" => format!(
                "{} of {}: planned {}, every dollar allocated",
                split_disp,
                fmt_money(income_cents, &cur),
                fmt_money(total_planned, &cur)
            ),
            _ => format!(
                "{} of {}: planned {}, left to allocate {}",
                split_disp,
                fmt_money(income_cents, &cur),
                fmt_money(total_planned, &cur),
                fmt_money(left, &cur)
            ),
        }
    } else {
        format!(
            "{} of {}: needs {} · wants {} · savings {}",
            split_disp,
            fmt_money(income_cents, &cur),
            fmt_money(targets[0], &cur),
            fmt_money(targets[1], &cur),
            fmt_money(targets[2], &cur)
        )
    };
    Ok(Plan {
        mode: "50-30-20".into(),
        income: cents_to_f64(income_cents),
        currency: cur,
        split: Some(split_disp),
        buckets: Some(buckets),
        categories,
        total_planned: cents_to_f64(total_planned),
        left_to_allocate: cents_to_f64(left),
        status: status.into(),
        summary,
    })
}

fn plan_zero_based(income_cents: i64, cur: String, items: Vec<Expense>) -> Result<Plan, String> {
    if items.is_empty() {
        return Err(
            "zero-based mode needs at least one expense line — add your categories as `Name: amount`, one per line"
                .into(),
        );
    }
    let categories: Vec<Category> = items
        .iter()
        .map(|e| Category {
            name: e.name.clone(),
            amount: cents_to_f64(e.cents),
            share_pct: pct_of(e.cents, income_cents),
            bucket: e.bucket.map(|b| b.name().to_string()),
        })
        .collect();
    let total_planned: i64 = items.iter().map(|e| e.cents).sum();
    let left = income_cents - total_planned;
    let status = status_of(left);
    let summary = match status {
        "deficit" => format!(
            "planned {} of {} · over budget by {}",
            fmt_money(total_planned, &cur),
            fmt_money(income_cents, &cur),
            fmt_money(-left, &cur)
        ),
        "balanced" => format!(
            "planned {} of {} · every dollar assigned",
            fmt_money(total_planned, &cur),
            fmt_money(income_cents, &cur)
        ),
        _ => format!(
            "planned {} of {} · {} left to allocate",
            fmt_money(total_planned, &cur),
            fmt_money(income_cents, &cur),
            fmt_money(left, &cur)
        ),
    };
    Ok(Plan {
        mode: "zero-based".into(),
        income: cents_to_f64(income_cents),
        currency: cur,
        split: None,
        buckets: None,
        categories,
        total_planned: cents_to_f64(total_planned),
        left_to_allocate: cents_to_f64(left),
        status: status.into(),
        summary,
    })
}

fn status_of(left_cents: i64) -> &'static str {
    match left_cents {
        0 => "balanced",
        l if l < 0 => "deficit",
        _ => "surplus",
    }
}

// ---------------------------------------------------------------------------
// Text report (what the page shows)
// ---------------------------------------------------------------------------

fn money(v: f64, cur: &str) -> String {
    fmt_money((v * 100.0).round() as i64, cur)
}

fn pad_row(name: &str, name_w: usize, amt: &str, amt_w: usize, share: &str, share_w: usize) -> String {
    format!("{:<name_w$}  {:>amt_w$}  {:>share_w$}\n", name, amt, share)
}

/// Render the plan as an aligned plain-text report.
pub fn render_report(p: &Plan) -> String {
    let cur = &p.currency;
    let mut out = String::new();
    if p.mode == "zero-based" {
        out.push_str(&format!(
            "Zero-based budget · take-home income {}/month\n\n",
            money(p.income, cur)
        ));
        let total_label = "Total planned";
        let left_label = "Left to allocate";
        let name_w = p
            .categories
            .iter()
            .map(|c| c.name.chars().count())
            .chain(["Category".len(), total_label.len(), left_label.len()])
            .max()
            .unwrap_or(8);
        let rows: Vec<(String, String, String)> = p
            .categories
            .iter()
            .map(|c| {
                (
                    c.name.clone(),
                    money(c.amount, cur),
                    format!("{:.1}%", c.share_pct),
                )
            })
            .collect();
        let income_cents = (p.income * 100.0).round() as i64;
        let total_m = money(p.total_planned, cur);
        let left_m = money(p.left_to_allocate, cur);
        let total_s = format!(
            "{:.1}%",
            pct_of((p.total_planned * 100.0).round() as i64, income_cents)
        );
        let left_s = format!(
            "{:.1}%",
            pct_of((p.left_to_allocate * 100.0).round() as i64, income_cents)
        );
        let amt_w = rows
            .iter()
            .map(|r| r.1.chars().count())
            .chain(["Planned".len(), total_m.chars().count(), left_m.chars().count()])
            .max()
            .unwrap_or(7);
        let share_w = rows
            .iter()
            .map(|r| r.2.chars().count())
            .chain(["Share".len(), total_s.len(), left_s.len()])
            .max()
            .unwrap_or(5);
        out.push_str(&pad_row("Category", name_w, "Planned", amt_w, "Share", share_w));
        for (name, amt, share) in &rows {
            out.push_str(&pad_row(name, name_w, amt, amt_w, share, share_w));
        }
        out.push_str(&pad_row(total_label, name_w, &total_m, amt_w, &total_s, share_w));
        out.push_str(&pad_row(left_label, name_w, &left_m, amt_w, &left_s, share_w));
        out.push('\n');
        out.push_str(&match p.status.as_str() {
            "deficit" => format!(
                "Over budget by {} — trim planned spending to get income minus expenses back to zero.",
                money(-p.left_to_allocate, cur)
            ),
            "balanced" => "Every dollar is assigned — your budget zeroes out.".to_string(),
            _ => format!(
                "Assign the remaining {} — a zero-based budget gives every dollar a job.",
                money(p.left_to_allocate, cur)
            ),
        });
        out.push('\n');
        return out;
    }

    // Rule (50-30-20 / custom split) mode.
    let split_disp = p.split.clone().unwrap_or_else(|| "50/30/20".into());
    out.push_str(&format!(
        "{} budget · take-home income {}/month\n\n",
        split_disp,
        money(p.income, cur)
    ));
    let buckets = p.buckets.as_deref().unwrap_or(&[]);
    let with_planned = buckets.iter().any(|b| b.planned.is_some());
    let name_w = "Savings".len().max("Bucket".len());
    let shares: Vec<String> = buckets
        .iter()
        .map(|b| format!("{}%", fmt_share(b.share_pct)))
        .collect();
    let share_w = shares
        .iter()
        .map(|s| s.chars().count())
        .chain(["Share".len()])
        .max()
        .unwrap();
    let targets: Vec<String> = buckets.iter().map(|b| money(b.target, cur)).collect();
    let target_w = targets
        .iter()
        .map(|s| s.chars().count())
        .chain(["Target".len()])
        .max()
        .unwrap();
    if with_planned {
        let planned: Vec<String> = buckets
            .iter()
            .map(|b| money(b.planned.unwrap_or(0.0), cur))
            .collect();
        let planned_w = planned
            .iter()
            .map(|s| s.chars().count())
            .chain(["Planned".len()])
            .max()
            .unwrap();
        let lefts: Vec<String> = buckets
            .iter()
            .map(|b| money(b.left.unwrap_or(0.0), cur))
            .collect();
        let left_w = lefts
            .iter()
            .map(|s| s.chars().count())
            .chain(["Left".len()])
            .max()
            .unwrap();
        out.push_str(&format!(
            "{:<name_w$}  {:>share_w$}  {:>target_w$}  {:>planned_w$}  {:>left_w$}\n",
            "Bucket", "Share", "Target", "Planned", "Left"
        ));
        for (i, b) in buckets.iter().enumerate() {
            let over = if b.left.unwrap_or(0.0) < 0.0 { "  (over)" } else { "" };
            out.push_str(&format!(
                "{:<name_w$}  {:>share_w$}  {:>target_w$}  {:>planned_w$}  {:>left_w$}{}\n",
                b.name, shares[i], targets[i], planned[i], lefts[i], over
            ));
        }
        out.push('\n');
        out.push_str(&match p.status.as_str() {
            "deficit" => format!(
                "Planned {} of {} · over budget by {}",
                money(p.total_planned, cur),
                money(p.income, cur),
                money(-p.left_to_allocate, cur)
            ),
            "balanced" => format!(
                "Planned {} of {} · every dollar allocated",
                money(p.total_planned, cur),
                money(p.income, cur)
            ),
            _ => format!(
                "Planned {} of {} · left to allocate {}",
                money(p.total_planned, cur),
                money(p.income, cur),
                money(p.left_to_allocate, cur)
            ),
        });
        out.push('\n');
    } else {
        out.push_str(&format!(
            "{:<name_w$}  {:>share_w$}  {:>target_w$}\n",
            "Bucket", "Share", "Target"
        ));
        for (i, b) in buckets.iter().enumerate() {
            out.push_str(&format!(
                "{:<name_w$}  {:>share_w$}  {:>target_w$}\n",
                b.name, shares[i], targets[i]
            ));
        }
    }
    out
}

/// plan() + render_report() — what the web page calls.
pub fn plan_report(
    income: f64,
    mode: &str,
    split: &str,
    expenses: &str,
    currency: &str,
) -> Result<String, String> {
    let p = plan(income, mode, split, expenses, currency)?;
    Ok(render_report(&p))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_50_30_20_matches_the_worked_example() {
        // The classic example: $4,500 → $2,250 / $1,350 / $900.
        let p = plan(4500.0, "", "", "", "").unwrap();
        let b = p.buckets.as_ref().unwrap();
        assert_eq!(b[0].target, 2250.0);
        assert_eq!(b[1].target, 1350.0);
        assert_eq!(b[2].target, 900.0);
        assert_eq!(p.split.as_deref(), Some("50/30/20"));
        assert_eq!(p.status, "surplus");
        assert_eq!(
            p.summary,
            "50/30/20 of $4,500.00: needs $2,250.00 · wants $1,350.00 · savings $900.00"
        );
    }

    #[test]
    fn targets_always_sum_exactly_to_income() {
        for (income, split) in [
            (4500.01, "50/30/20"),
            (100.01, "50/50/0"),
            (3333.33, "33.3/33.3/33.4"),
            (0.01, "50/30/20"),
            (999_999_999.99, "60/30/10"),
        ] {
            let p = plan(income, "50-30-20", split, "", "$").unwrap();
            let b = p.buckets.as_ref().unwrap();
            let sum_cents: i64 = b.iter().map(|x| (x.target * 100.0).round() as i64).sum();
            assert_eq!(
                sum_cents,
                (income * 100.0_f64).round() as i64,
                "income {} split {}",
                income,
                split
            );
            assert!(b.iter().all(|x| x.target >= 0.0));
        }
    }

    #[test]
    fn custom_split_with_tagged_expenses_compares_planned_to_target() {
        let expenses =
            "Rent: $1,800 (needs)\nGroceries: 550 (needs)\nDining out: 260 (wants)\nRoth IRA: 500 savings";
        let p = plan(5200.0, "50-30-20", "60/30/10", expenses, "$").unwrap();
        let b = p.buckets.as_ref().unwrap();
        assert_eq!(b[0].target, 3120.0);
        assert_eq!(b[0].planned, Some(2350.0));
        assert_eq!(b[0].left, Some(770.0));
        assert_eq!(b[1].planned, Some(260.0));
        assert_eq!(b[2].target, 520.0);
        assert_eq!(b[2].planned, Some(500.0));
        assert_eq!(p.total_planned, 3110.0);
        assert_eq!(p.left_to_allocate, 2090.0);
        assert_eq!(p.categories.len(), 4);
        assert_eq!(p.categories[0].bucket.as_deref(), Some("Needs"));
    }

    #[test]
    fn rule_mode_untagged_expense_is_an_error_naming_the_line() {
        let err =
            plan(4500.0, "50-30-20", "", "Rent: 1200 (needs)\nStreaming: 30", "$").unwrap_err();
        assert!(err.contains("expense line 2"), "{}", err);
        assert!(err.contains("Streaming"), "{}", err);
        assert!(err.contains("(needs), (wants) or (savings)"), "{}", err);
    }

    #[test]
    fn zero_based_sums_categories_and_reports_whats_left() {
        let expenses = "Rent: 1400; Groceries: 450; Utilities: 180; Fun money: 200";
        let p = plan(2500.0, "zero-based", "", expenses, "$").unwrap();
        assert_eq!(p.total_planned, 2230.0);
        assert_eq!(p.left_to_allocate, 270.0);
        assert_eq!(p.status, "surplus");
        assert_eq!(p.categories.len(), 4);
        assert_eq!(p.categories[0].share_pct, 56.0);
        assert_eq!(
            p.summary,
            "planned $2,230.00 of $2,500.00 · $270.00 left to allocate"
        );
    }

    #[test]
    fn zero_based_deficit_and_balanced_statuses() {
        let over = plan(1000.0, "zero-based", "", "Rent: 900\nFood: 300", "$").unwrap();
        assert_eq!(over.status, "deficit");
        assert_eq!(over.left_to_allocate, -200.0);
        assert!(over.summary.contains("over budget by $200.00"), "{}", over.summary);

        let exact = plan(1200.0, "zero-based", "", "Rent: 900\nFood: 300", "$").unwrap();
        assert_eq!(exact.status, "balanced");
        assert_eq!(exact.left_to_allocate, 0.0);
        assert!(exact.summary.contains("every dollar assigned"), "{}", exact.summary);
    }

    #[test]
    fn zero_based_without_expenses_is_an_error() {
        let err = plan(4500.0, "zero-based", "", "", "$").unwrap_err();
        assert!(
            err.contains("zero-based mode needs at least one expense line"),
            "{}",
            err
        );
    }

    #[test]
    fn invalid_income_split_and_mode_error_clearly() {
        assert!(plan(0.0, "", "", "", "").is_err());
        assert!(plan(f64::NAN, "", "", "", "").is_err());
        assert!(plan(1_000_000_001.0, "", "", "", "").is_err());
        let e = plan(4500.0, "", "50/30/25", "", "").unwrap_err();
        assert!(e.contains("add up to 100"), "{}", e);
        assert!(e.contains("105"), "{}", e);
        let e = plan(4500.0, "", "50/50", "", "").unwrap_err();
        assert!(e.contains("exactly three shares"), "{}", e);
        let e = plan(4500.0, "monthly", "", "", "").unwrap_err();
        assert!(e.contains("unknown mode"), "{}", e);
    }

    #[test]
    fn amount_forms_dollar_sign_commas_decimals_and_negatives() {
        assert_eq!(parse_amount_cents("1200", "$").unwrap(), 120_000);
        assert_eq!(parse_amount_cents("$1,200.50", "$").unwrap(), 120_050);
        assert_eq!(parse_amount_cents("1_200", "$").unwrap(), 120_000);
        assert_eq!(parse_amount_cents("0.5", "$").unwrap(), 50);
        assert_eq!(parse_amount_cents("€30", "€").unwrap(), 3_000);
        assert!(parse_amount_cents("-5", "$").is_err());
        assert!(parse_amount_cents("1.005", "$").is_err());
        assert!(parse_amount_cents("abc", "$").is_err());
        assert!(parse_amount_cents("1000000001", "$").is_err());
    }

    #[test]
    fn split_accepts_spaces_commas_and_percent_suffixes() {
        assert_eq!(parse_split("55 25 20").unwrap(), [55.0, 25.0, 20.0]);
        assert_eq!(parse_split("60,30,10").unwrap(), [60.0, 30.0, 10.0]);
        assert_eq!(parse_split("50% / 30% / 20%").unwrap(), [50.0, 30.0, 20.0]);
        assert_eq!(parse_split("80/0/20").unwrap(), [80.0, 0.0, 20.0]);
    }

    #[test]
    fn bucket_tags_accept_aliases_and_parens() {
        let p = plan(
            4000.0,
            "50-30-20",
            "",
            "Rent: 1200 needs\nFun: 100 (wants)\nLoan: 200 debt\nSaver: 100 [savings]",
            "$",
        )
        .unwrap();
        let b = p.buckets.as_ref().unwrap();
        assert_eq!(b[0].planned, Some(1200.0));
        assert_eq!(b[1].planned, Some(100.0));
        assert_eq!(b[2].planned, Some(300.0));
    }

    #[test]
    fn line_cap_and_name_cap_enforced() {
        let ok: String = (0..100).map(|i| format!("Cat{}: 1\n", i)).collect();
        assert!(plan(4500.0, "zero-based", "", &ok, "$").is_ok());
        let over: String = (0..101).map(|i| format!("Cat{}: 1\n", i)).collect();
        let e = plan(4500.0, "zero-based", "", &over, "$").unwrap_err();
        assert!(e.contains("max 100"), "{}", e);
        let long = format!("{}: 10", "x".repeat(61));
        let e = plan(4500.0, "zero-based", "", &long, "$").unwrap_err();
        assert!(e.contains("too long"), "{}", e);
    }

    #[test]
    fn comments_blanks_and_semicolons_are_handled() {
        let p = plan(
            1000.0,
            "zero-based",
            "",
            "# fixed costs\nRent: 500\n\n// fun\nGames: 100; Snacks: 50",
            "$",
        )
        .unwrap();
        assert_eq!(p.categories.len(), 3);
        assert_eq!(p.total_planned, 650.0);
    }

    #[test]
    fn custom_currency_prefixes_the_report() {
        let p = plan(4500.0, "", "", "", "€").unwrap();
        assert!(p.summary.contains("€2,250.00"), "{}", p.summary);
        let r = render_report(&p);
        assert!(r.contains("€4,500.00/month"), "{}", r);
    }

    #[test]
    fn rule_report_exact_text() {
        let r = plan_report(4500.0, "50-30-20", "50/30/20", "", "$").unwrap();
        let expected = "\
50/30/20 budget · take-home income $4,500.00/month

Bucket   Share     Target
Needs      50%  $2,250.00
Wants      30%  $1,350.00
Savings    20%    $900.00
";
        assert_eq!(r, expected);
    }

    #[test]
    fn zero_based_report_exact_text() {
        let r = plan_report(
            2500.0,
            "zero-based",
            "",
            "Rent: 1400\nGroceries: 450\nUtilities: 180\nFun money: 200",
            "$",
        )
        .unwrap();
        let expected = "\
Zero-based budget · take-home income $2,500.00/month

Category            Planned  Share
Rent              $1,400.00  56.0%
Groceries           $450.00  18.0%
Utilities           $180.00   7.2%
Fun money           $200.00   8.0%
Total planned     $2,230.00  89.2%
Left to allocate    $270.00  10.8%

Assign the remaining $270.00 — a zero-based budget gives every dollar a job.
";
        assert_eq!(r, expected);
    }

    #[test]
    fn rule_report_with_expenses_exact_text() {
        let r = plan_report(
            5200.0,
            "50-30-20",
            "60/30/10",
            "Rent: 1800 (needs)\nDining out: 260 (wants)\nBrokerage: 600 (savings)",
            "$",
        )
        .unwrap();
        let expected = "\
60/30/10 budget · take-home income $5,200.00/month

Bucket   Share     Target    Planned       Left
Needs      60%  $3,120.00  $1,800.00  $1,320.00
Wants      30%  $1,560.00    $260.00  $1,300.00
Savings    10%    $520.00    $600.00    -$80.00  (over)

Planned $2,660.00 of $5,200.00 · left to allocate $2,540.00
";
        assert_eq!(r, expected);
    }

    #[test]
    fn json_shape_for_chat_includes_summary_and_buckets() {
        let p = plan(4500.0, "", "", "", "").unwrap();
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["mode"], "50-30-20");
        assert_eq!(v["income"], 4500.0);
        assert_eq!(v["buckets"][0]["target"], 2250.0);
        assert!(v["buckets"][0].get("planned").is_none());
        assert!(v.get("categories").is_none(), "empty categories are skipped");
        assert_eq!(v["status"], "surplus");
    }
}
