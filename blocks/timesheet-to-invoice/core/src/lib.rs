//! timesheet-to-invoice core — pure compute, shared by the chat/CLI block and the
//! web page. No wafer/wasm-bindgen deps, no clock, no I/O.
//!
//! Turns tracked hours into a client-ready invoice document. Each entry line is
//! pipe- or tab-delimited and takes one of four shapes:
//!
//! ```text
//! Description | HOURS
//! Description | HOURS | RATE
//! YYYY-MM-DD  | Description | HOURS
//! YYYY-MM-DD  | Description | HOURS | RATE
//! ```
//!
//! `HOURS` accepts decimal hours (`3.5`), `2h 30m`, `2:30`, or a clock range
//! (`09:00-12:30`, `9am-5pm`, `10pm-2am` rolls past midnight). A 3-field row is
//! read as `date | description | hours` when the first field parses as a date,
//! otherwise as `description | hours | rate`.
//!
//! Money is canonicalised through 2-dp rounding per line, then subtotal →
//! discount → tax → total, so every surface produces byte-identical output.

use serde::Serialize;

/// One billable invoice row after parsing, rounding and pricing.
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct Line {
    /// Optional `YYYY-MM-DD` service date, if the row carried one.
    pub date: Option<String>,
    pub description: String,
    /// Billed time in whole minutes (after any rounding increment).
    pub minutes: i64,
    /// Billed time in decimal hours, rounded to 2 dp.
    pub hours: f64,
    /// Hourly rate applied to this row.
    pub rate: f64,
    /// `hours * rate`, rounded to 2 dp.
    pub amount: f64,
}

/// The full invoice, returned as JSON by `format = "json"`.
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct Invoice {
    pub invoice_number: String,
    pub issue_date: String,
    pub due_date: String,
    pub payment_terms: i64,
    pub business: String,
    pub client: String,
    pub currency: String,
    pub lines: Vec<Line>,
    pub total_minutes: i64,
    pub total_hours: f64,
    pub subtotal: f64,
    pub discount_percent: f64,
    pub discount: f64,
    pub tax_label: String,
    pub tax_rate: f64,
    pub tax: f64,
    pub total: f64,
    pub notes: String,
}

/// How repeated rows are collapsed before billing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    /// One invoice line per source row (default).
    Entry,
    /// Merge rows that share a description (and rate).
    Description,
    /// Merge rows that share a service date (and rate).
    Date,
}

impl GroupBy {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "entry" | "" => Ok(GroupBy::Entry),
            "description" => Ok(GroupBy::Description),
            "date" => Ok(GroupBy::Date),
            other => Err(format!(
                "unknown group_by '{other}' (use entry, description or date)"
            )),
        }
    }
}

/// Output document format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Markdown,
    Text,
    Csv,
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" | "" => Ok(OutputFormat::Markdown),
            "text" | "plain" => Ok(OutputFormat::Text),
            "csv" => Ok(OutputFormat::Csv),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "unknown format '{other}' (use markdown, text, csv or json)"
            )),
        }
    }
}

/// Everything the caller can tune. Each surface fills this from its own params.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub rate: f64,
    pub currency: String,
    pub client: String,
    pub business: String,
    pub invoice_number: String,
    pub issue_date: String,
    pub due_date: String,
    pub payment_terms: i64,
    pub tax_label: String,
    pub tax_rate: f64,
    pub discount_percent: f64,
    /// Billing increment in minutes; 0 = bill exactly what was tracked.
    pub round: i64,
    pub group_by: GroupBy,
    pub notes: String,
    pub format: OutputFormat,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            rate: 100.0,
            currency: "$".into(),
            client: String::new(),
            business: String::new(),
            invoice_number: "INV-001".into(),
            issue_date: String::new(),
            due_date: String::new(),
            payment_terms: 30,
            tax_label: "Tax".into(),
            tax_rate: 0.0,
            discount_percent: 0.0,
            round: 0,
            group_by: GroupBy::Entry,
            notes: String::new(),
            format: OutputFormat::Markdown,
        }
    }
}

/// Largest accepted entry-text size, mirrored in the descriptor + page copy.
pub const MAX_INPUT_BYTES: usize = 1_000_000;
/// Largest number of billable rows on one invoice.
pub const MAX_LINES: usize = 500;

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// `1234.5` → `1,234.50`. Grouping is always comma/period — deterministic across
/// surfaces, and the caller supplies whatever currency symbol they want.
fn money(currency: &str, v: f64) -> String {
    let neg = v < 0.0;
    let cents = (v.abs() * 100.0).round() as i64;
    let whole = cents / 100;
    let frac = cents % 100;
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!(
        "{}{}{}.{:02}",
        if neg { "-" } else { "" },
        currency,
        grouped,
        frac
    )
}

/// Trim trailing zeros off a percentage so `20.0` prints as `20` but `7.5` stays.
fn pct(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() {
        "0".into()
    } else {
        s
    }
}

fn hours_str(h: f64) -> String {
    format!("{h:.2}")
}

/// `YYYY-MM-DD` → (y, m, d), rejecting impossible calendar dates.
fn parse_date(s: &str) -> Option<(i64, i64, i64)> {
    let t = s.trim();
    let b = t.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    if !b.iter().enumerate().all(|(i, c)| {
        if i == 4 || i == 7 {
            true
        } else {
            c.is_ascii_digit()
        }
    }) {
        return None;
    }
    let y: i64 = t[0..4].parse().ok()?;
    let m: i64 = t[5..7].parse().ok()?;
    let d: i64 = t[8..10].parse().ok()?;
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    Some((y, m, d))
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Howard Hinnant's days-from-civil — exact, no date crate needed.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn add_days(date: &str, days: i64) -> Option<String> {
    let (y, m, d) = parse_date(date)?;
    let (y2, m2, d2) = civil_from_days(days_from_civil(y, m, d) + days);
    Some(format!("{y2:04}-{m2:02}-{d2:02}"))
}

/// `9:30`, `0930`, `9:30am`, `5pm` → minutes since midnight.
fn parse_clock(tok: &str) -> Option<i64> {
    let t = tok.trim().to_ascii_lowercase();
    if t.is_empty() {
        return None;
    }
    let (body, ampm) = if let Some(b) = t.strip_suffix("am") {
        (b.trim().to_string(), Some(false))
    } else if let Some(b) = t.strip_suffix("pm") {
        (b.trim().to_string(), Some(true))
    } else if let Some(b) = t.strip_suffix('a') {
        (b.trim().to_string(), Some(false))
    } else if let Some(b) = t.strip_suffix('p') {
        (b.trim().to_string(), Some(true))
    } else {
        (t.clone(), None)
    };
    let (h, m) = if let Some((a, b)) = body.split_once(':') {
        (a.parse::<i64>().ok()?, b.parse::<i64>().ok()?)
    } else if body.len() == 4 && body.chars().all(|c| c.is_ascii_digit()) {
        (body[0..2].parse().ok()?, body[2..4].parse().ok()?)
    } else if body.chars().all(|c| c.is_ascii_digit()) && !body.is_empty() {
        (body.parse::<i64>().ok()?, 0)
    } else {
        return None;
    };
    if !(0..=59).contains(&m) {
        return None;
    }
    let h = match ampm {
        Some(_) if !(1..=12).contains(&h) => return None,
        Some(true) => {
            if h == 12 {
                12
            } else {
                h + 12
            }
        }
        Some(false) => {
            if h == 12 {
                0
            } else {
                h
            }
        }
        None => {
            if !(0..=23).contains(&h) {
                return None;
            }
            h
        }
    };
    Some(h * 60 + m)
}

/// Parse the hours field in any advertised form → whole minutes.
fn parse_hours(tok: &str) -> Result<i64, String> {
    let t = tok.trim().to_ascii_lowercase();
    if t.is_empty() {
        return Err("empty hours value".into());
    }

    // Clock range: 09:00-12:30, 9am-5pm, 10pm-2am (rolls past midnight).
    for sep in ["–", "—", "-", " to ", ".."] {
        if let Some((a, b)) = t.split_once(sep) {
            if !a.trim().is_empty() && !b.trim().is_empty() {
                if let (Some(start), Some(end)) = (parse_clock(a), parse_clock(b)) {
                    let mut span = end - start;
                    if span < 0 {
                        span += 24 * 60;
                    }
                    return Ok(span);
                }
            }
        }
    }

    // H:MM duration.
    if let Some((a, b)) = t.split_once(':') {
        let h: i64 = a
            .trim()
            .parse()
            .map_err(|_| format!("'{tok}' is not a valid duration"))?;
        let m: i64 = b
            .trim()
            .parse()
            .map_err(|_| format!("'{tok}' is not a valid duration"))?;
        if h < 0 || !(0..=59).contains(&m) {
            return Err(format!("'{tok}': minutes must be 0-59"));
        }
        return Ok(h * 60 + m);
    }

    // 2h 30m / 2h / 45m / 90min.
    if t.contains('h') || t.contains('m') {
        let mut minutes = 0f64;
        let mut num = String::new();
        let mut saw_unit = false;
        let mut chars = t.chars().peekable();
        while let Some(c) = chars.next() {
            if c.is_ascii_digit() || c == '.' {
                num.push(c);
            } else if c == 'h' {
                let v: f64 = num
                    .parse()
                    .map_err(|_| format!("'{tok}' is not a valid duration"))?;
                minutes += v * 60.0;
                num.clear();
                saw_unit = true;
            } else if c == 'm' {
                // Swallow the "in" of "min"/"mins".
                while matches!(chars.peek(), Some('i') | Some('n') | Some('s')) {
                    chars.next();
                }
                let v: f64 = num
                    .parse()
                    .map_err(|_| format!("'{tok}' is not a valid duration"))?;
                minutes += v;
                num.clear();
                saw_unit = true;
            } else if c.is_whitespace() {
                continue;
            } else {
                return Err(format!("'{tok}' is not a valid duration"));
            }
        }
        if saw_unit && num.trim().is_empty() {
            if minutes < 0.0 {
                return Err(format!("'{tok}': hours cannot be negative"));
            }
            return Ok(minutes.round() as i64);
        }
        return Err(format!("'{tok}' is not a valid duration"));
    }

    // Bare decimal hours.
    let v: f64 = t
        .parse()
        .map_err(|_| format!("'{tok}' is not a number of hours"))?;
    if v < 0.0 {
        return Err(format!("'{tok}': hours cannot be negative"));
    }
    if !v.is_finite() {
        return Err(format!("'{tok}' is not a finite number of hours"));
    }
    Ok((v * 60.0).round() as i64)
}

fn parse_money(tok: &str) -> Result<f64, String> {
    let cleaned: String = tok
        .trim()
        .chars()
        .filter(|c| !matches!(c, ',' | '$' | '£' | '€' | '¥' | ' '))
        .collect();
    let v: f64 = cleaned
        .parse()
        .map_err(|_| format!("'{tok}' is not a number"))?;
    if !v.is_finite() {
        return Err(format!("'{tok}' is not a finite number"));
    }
    Ok(v)
}

fn split_fields(line: &str) -> Vec<String> {
    let sep = if line.contains('|') { '|' } else { '\t' };
    line.split(sep).map(|f| f.trim().to_string()).collect()
}

fn round_minutes(minutes: i64, increment: i64) -> i64 {
    if increment <= 1 {
        return minutes;
    }
    if minutes == 0 {
        return 0;
    }
    let up = minutes.div_euclid(increment) * increment;
    let rem = minutes - up;
    if rem == 0 {
        minutes
    } else {
        up + increment
    }
}

/// Parse the raw entry text into priced invoice lines.
fn parse_lines(entries: &str, opts: &Options) -> Result<Vec<Line>, String> {
    if entries.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "entries too large: {} bytes (maximum {MAX_INPUT_BYTES})",
            entries.len()
        ));
    }
    let mut out: Vec<Line> = Vec::new();
    for (i, raw) in entries.lines().enumerate() {
        let line_no = i + 1;
        let l = raw.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("//") {
            continue;
        }
        let f = split_fields(l);
        let (date, description, hours_tok, rate_tok) = match f.len() {
            2 => (None, f[0].clone(), f[1].clone(), None),
            3 => {
                if parse_date(&f[0]).is_some() {
                    (Some(f[0].clone()), f[1].clone(), f[2].clone(), None)
                } else {
                    (None, f[0].clone(), f[1].clone(), Some(f[2].clone()))
                }
            }
            4 => {
                if parse_date(&f[0]).is_none() {
                    return Err(format!(
                        "line {line_no}: '{}' is not a YYYY-MM-DD date (4-field rows are date | description | hours | rate)",
                        f[0]
                    ));
                }
                (
                    Some(f[0].clone()),
                    f[1].clone(),
                    f[2].clone(),
                    Some(f[3].clone()),
                )
            }
            _ => {
                return Err(format!(
                    "line {line_no}: expected 2-4 fields separated by '|' \
                     (description | hours [| rate], optionally with a leading YYYY-MM-DD date), got {}: {l}",
                    f.len()
                ))
            }
        };
        if description.is_empty() {
            return Err(format!("line {line_no}: description is empty"));
        }
        let minutes = parse_hours(&hours_tok).map_err(|e| format!("line {line_no}: {e}"))?;
        let rate = match rate_tok {
            Some(r) => parse_money(&r).map_err(|e| format!("line {line_no}: {e}"))?,
            None => opts.rate,
        };
        out.push(Line {
            date,
            description,
            minutes,
            hours: 0.0,
            rate,
            amount: 0.0,
        });
        if out.len() > MAX_LINES {
            return Err(format!(
                "too many entry lines: maximum {MAX_LINES} billable rows per invoice"
            ));
        }
    }
    if out.is_empty() {
        return Err(
            "no entry lines found — add at least one 'description | hours' row".to_string(),
        );
    }

    // Merge before rounding so grouped rows round once, not per source row.
    let merged = match opts.group_by {
        GroupBy::Entry => out,
        GroupBy::Description | GroupBy::Date => {
            let mut acc: Vec<Line> = Vec::new();
            for l in out {
                let key = |x: &Line| -> String {
                    let k = if opts.group_by == GroupBy::Description {
                        x.description.to_ascii_lowercase()
                    } else {
                        x.date.clone().unwrap_or_default()
                    };
                    format!("{k}\u{1}{}", x.rate)
                };
                match acc.iter_mut().find(|e| key(e) == key(&l)) {
                    Some(e) => {
                        e.minutes += l.minutes;
                        if opts.group_by == GroupBy::Date && e.description != l.description {
                            e.description = format!("{}; {}", e.description, l.description);
                        }
                        if opts.group_by == GroupBy::Description && e.date != l.date {
                            e.date = None;
                        }
                    }
                    None => acc.push(l),
                }
            }
            acc
        }
    };

    Ok(merged
        .into_iter()
        .map(|mut l| {
            l.minutes = round_minutes(l.minutes, opts.round);
            l.hours = round2(l.minutes as f64 / 60.0);
            l.amount = round2(l.minutes as f64 / 60.0 * l.rate);
            l
        })
        .collect())
}

/// Build the priced invoice from tracked-hours text plus invoice metadata.
pub fn build(entries: &str, opts: &Options) -> Result<Invoice, String> {
    if !(0.0..=100.0).contains(&opts.tax_rate) {
        return Err(format!(
            "tax_rate must be between 0 and 100, got {}",
            opts.tax_rate
        ));
    }
    if !(0.0..=100.0).contains(&opts.discount_percent) {
        return Err(format!(
            "discount_percent must be between 0 and 100, got {}",
            opts.discount_percent
        ));
    }
    if !(0..=60).contains(&opts.round) {
        return Err(format!(
            "round must be between 0 and 60 minutes, got {}",
            opts.round
        ));
    }
    if !opts.issue_date.trim().is_empty() && parse_date(&opts.issue_date).is_none() {
        return Err(format!(
            "issue_date '{}' is not a YYYY-MM-DD date",
            opts.issue_date.trim()
        ));
    }
    if !opts.due_date.trim().is_empty() && parse_date(&opts.due_date).is_none() {
        return Err(format!(
            "due_date '{}' is not a YYYY-MM-DD date",
            opts.due_date.trim()
        ));
    }

    let lines = parse_lines(entries, opts)?;

    let total_minutes: i64 = lines.iter().map(|l| l.minutes).sum();
    let subtotal = round2(lines.iter().map(|l| l.amount).sum());
    let discount = round2(subtotal * opts.discount_percent / 100.0);
    let taxable = round2(subtotal - discount);
    let tax = round2(taxable * opts.tax_rate / 100.0);
    let total = round2(taxable + tax);

    let issue_date = opts.issue_date.trim().to_string();
    let due_date = if !opts.due_date.trim().is_empty() {
        opts.due_date.trim().to_string()
    } else if !issue_date.is_empty() && opts.payment_terms > 0 {
        add_days(&issue_date, opts.payment_terms).unwrap_or_default()
    } else {
        String::new()
    };

    let currency = if opts.currency.trim().is_empty() {
        "$".to_string()
    } else {
        opts.currency.trim().to_string()
    };
    let invoice_number = if opts.invoice_number.trim().is_empty() {
        "INV-001".to_string()
    } else {
        opts.invoice_number.trim().to_string()
    };
    let tax_label = if opts.tax_label.trim().is_empty() {
        "Tax".to_string()
    } else {
        opts.tax_label.trim().to_string()
    };

    Ok(Invoice {
        invoice_number,
        issue_date,
        due_date,
        payment_terms: opts.payment_terms,
        business: opts.business.trim().to_string(),
        client: opts.client.trim().to_string(),
        currency,
        lines,
        total_minutes,
        total_hours: round2(total_minutes as f64 / 60.0),
        subtotal,
        discount_percent: opts.discount_percent,
        discount,
        tax_label,
        tax_rate: opts.tax_rate,
        tax,
        total,
        notes: opts.notes.trim().to_string(),
    })
}

fn block(out: &mut String, heading: &str, body: &str, markdown: bool) {
    if body.is_empty() {
        return;
    }
    if markdown {
        out.push_str(&format!("**{heading}**\n\n"));
        for l in body.lines() {
            out.push_str(&format!("{}  \n", l.trim_end()));
        }
        out.push('\n');
    } else {
        out.push_str(&format!("{heading}:\n"));
        for l in body.lines() {
            out.push_str(&format!("  {}\n", l.trim_end()));
        }
        out.push('\n');
    }
}

fn render_markdown(inv: &Invoice) -> String {
    let cur = &inv.currency;
    let mut s = format!("# Invoice {}\n\n", inv.invoice_number);
    block(&mut s, "From", &inv.business, true);
    block(&mut s, "Bill to", &inv.client, true);
    if !inv.issue_date.is_empty() {
        s.push_str(&format!("- **Issue date:** {}\n", inv.issue_date));
    }
    if !inv.due_date.is_empty() {
        s.push_str(&format!("- **Due date:** {}\n", inv.due_date));
    }
    if inv.payment_terms > 0 {
        s.push_str(&format!("- **Payment terms:** Net {}\n", inv.payment_terms));
    }
    if !inv.issue_date.is_empty() || !inv.due_date.is_empty() || inv.payment_terms > 0 {
        s.push('\n');
    }

    let dated = inv.lines.iter().any(|l| l.date.is_some());
    if dated {
        s.push_str("| Date | Description | Hours | Rate | Amount |\n");
        s.push_str("| --- | --- | ---: | ---: | ---: |\n");
    } else {
        s.push_str("| Description | Hours | Rate | Amount |\n");
        s.push_str("| --- | ---: | ---: | ---: |\n");
    }
    for l in &inv.lines {
        let desc = l.description.replace('|', "\\|");
        if dated {
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                l.date.clone().unwrap_or_default(),
                desc,
                hours_str(l.hours),
                money(cur, l.rate),
                money(cur, l.amount)
            ));
        } else {
            s.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                desc,
                hours_str(l.hours),
                money(cur, l.rate),
                money(cur, l.amount)
            ));
        }
    }
    s.push('\n');

    s.push_str(&format!(
        "**Total hours:** {}  \n",
        hours_str(inv.total_hours)
    ));
    s.push_str(&format!("**Subtotal:** {}  \n", money(cur, inv.subtotal)));
    if inv.discount > 0.0 {
        s.push_str(&format!(
            "**Discount ({}%):** {}  \n",
            pct(inv.discount_percent),
            money(cur, -inv.discount)
        ));
    }
    if inv.tax > 0.0 {
        s.push_str(&format!(
            "**{} ({}%):** {}  \n",
            inv.tax_label,
            pct(inv.tax_rate),
            money(cur, inv.tax)
        ));
    }
    s.push_str(&format!("**Total due: {}**\n", money(cur, inv.total)));

    if !inv.notes.is_empty() {
        s.push('\n');
        block(&mut s, "Notes", &inv.notes, true);
    }
    s.trim_end().to_string() + "\n"
}

fn render_text(inv: &Invoice) -> String {
    let cur = &inv.currency;
    let mut s = format!("INVOICE {}\n\n", inv.invoice_number);
    block(&mut s, "From", &inv.business, false);
    block(&mut s, "Bill to", &inv.client, false);
    if !inv.issue_date.is_empty() {
        s.push_str(&format!("Issue date:     {}\n", inv.issue_date));
    }
    if !inv.due_date.is_empty() {
        s.push_str(&format!("Due date:       {}\n", inv.due_date));
    }
    if inv.payment_terms > 0 {
        s.push_str(&format!("Payment terms:  Net {}\n", inv.payment_terms));
    }
    s.push('\n');

    let dated = inv.lines.iter().any(|l| l.date.is_some());
    let desc_w = inv
        .lines
        .iter()
        .map(|l| l.description.chars().count())
        .chain(std::iter::once(11))
        .max()
        .unwrap_or(11)
        .min(48);
    let amt_w = inv
        .lines
        .iter()
        .map(|l| money(cur, l.amount).chars().count())
        .chain(std::iter::once(money(cur, inv.total).chars().count()))
        .max()
        .unwrap_or(8)
        .max(6);
    let rate_w = inv
        .lines
        .iter()
        .map(|l| money(cur, l.rate).chars().count())
        .chain(std::iter::once(4))
        .max()
        .unwrap_or(8);

    let date_col = if dated { 12 } else { 0 };
    let header = format!(
        "{:date_w$}{:desc_w$}  {:>7}  {:>rate_w$}  {:>amt_w$}",
        if dated { "Date" } else { "" },
        "Description",
        "Hours",
        "Rate",
        "Amount",
        date_w = date_col,
        desc_w = desc_w,
        rate_w = rate_w,
        amt_w = amt_w
    );
    let width = header.chars().count();
    s.push_str(&header);
    s.push('\n');
    s.push_str(&"-".repeat(width));
    s.push('\n');
    for l in &inv.lines {
        let mut desc: String = l.description.chars().take(desc_w).collect();
        if l.description.chars().count() > desc_w {
            desc = l.description.chars().take(desc_w - 1).collect::<String>() + "…";
        }
        s.push_str(&format!(
            "{:date_w$}{:desc_w$}  {:>7}  {:>rate_w$}  {:>amt_w$}\n",
            l.date.clone().unwrap_or_default(),
            desc,
            hours_str(l.hours),
            money(cur, l.rate),
            money(cur, l.amount),
            date_w = date_col,
            desc_w = desc_w,
            rate_w = rate_w,
            amt_w = amt_w
        ));
    }
    s.push_str(&"-".repeat(width));
    s.push('\n');

    let label_w = width.saturating_sub(amt_w + 2);
    let total_row = |label: String, value: String, s: &mut String| {
        s.push_str(&format!(
            "{:>label_w$}  {:>amt_w$}\n",
            label,
            value,
            label_w = label_w,
            amt_w = amt_w
        ));
    };
    total_row("Total hours".into(), hours_str(inv.total_hours), &mut s);
    total_row("Subtotal".into(), money(cur, inv.subtotal), &mut s);
    if inv.discount > 0.0 {
        total_row(
            format!("Discount ({}%)", pct(inv.discount_percent)),
            money(cur, -inv.discount),
            &mut s,
        );
    }
    if inv.tax > 0.0 {
        total_row(
            format!("{} ({}%)", inv.tax_label, pct(inv.tax_rate)),
            money(cur, inv.tax),
            &mut s,
        );
    }
    total_row("TOTAL DUE".into(), money(cur, inv.total), &mut s);

    if !inv.notes.is_empty() {
        s.push('\n');
        block(&mut s, "Notes", &inv.notes, false);
    }
    s.trim_end().to_string() + "\n"
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(inv: &Invoice) -> String {
    let mut s = String::from("date,description,hours,rate,amount\n");
    for l in &inv.lines {
        s.push_str(&format!(
            "{},{},{},{:.2},{:.2}\n",
            l.date.clone().unwrap_or_default(),
            csv_field(&l.description),
            hours_str(l.hours),
            l.rate,
            l.amount
        ));
    }
    s.push_str(&format!(",Total hours,{},,\n", hours_str(inv.total_hours)));
    s.push_str(&format!(",Subtotal,,,{:.2}\n", inv.subtotal));
    if inv.discount > 0.0 {
        s.push_str(&format!(
            ",{},,,{:.2}\n",
            csv_field(&format!("Discount ({}%)", pct(inv.discount_percent))),
            -inv.discount
        ));
    }
    if inv.tax > 0.0 {
        s.push_str(&format!(
            ",{},,,{:.2}\n",
            csv_field(&format!("{} ({}%)", inv.tax_label, pct(inv.tax_rate))),
            inv.tax
        ));
    }
    s.push_str(&format!(",Total due,,,{:.2}\n", inv.total));
    s
}

/// Build the invoice and render it in the requested format.
pub fn generate(entries: &str, opts: &Options) -> Result<String, String> {
    let inv = build(entries, opts)?;
    Ok(match opts.format {
        OutputFormat::Markdown => render_markdown(&inv),
        OutputFormat::Text => render_text(&inv),
        OutputFormat::Csv => render_csv(&inv),
        OutputFormat::Json => {
            serde_json::to_string_pretty(&inv).map_err(|e| format!("json error: {e}"))?
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            rate: 120.0,
            issue_date: "2026-08-14".into(),
            ..Default::default()
        }
    }

    #[test]
    fn happy_path_markdown_invoice_totals() {
        let out = generate("Landing page copy | 3.5\nBug fixes | 2h 30m", &opts()).unwrap();
        assert!(out.starts_with("# Invoice INV-001\n"), "{out}");
        assert!(
            out.contains("| Landing page copy | 3.50 | $120.00 | $420.00 |"),
            "{out}"
        );
        assert!(
            out.contains("| Bug fixes | 2.50 | $120.00 | $300.00 |"),
            "{out}"
        );
        assert!(out.contains("**Total hours:** 6.00"), "{out}");
        assert!(out.contains("**Total due: $720.00**"), "{out}");
        // Net 30 from the issue date.
        assert!(out.contains("- **Due date:** 2026-09-13"), "{out}");
    }

    #[test]
    fn error_on_unparsable_hours() {
        let err = generate("Design work | soon", &opts()).unwrap_err();
        assert_eq!(err, "line 1: 'soon' is not a number of hours");
    }

    #[test]
    fn error_on_empty_entries() {
        let err = generate("\n# just a comment\n", &opts()).unwrap_err();
        assert!(err.contains("no entry lines found"), "{err}");
    }

    #[test]
    fn hours_forms_all_parse() {
        assert_eq!(parse_hours("3.5").unwrap(), 210);
        assert_eq!(parse_hours("2h 30m").unwrap(), 150);
        assert_eq!(parse_hours("45m").unwrap(), 45);
        assert_eq!(parse_hours("90min").unwrap(), 90);
        assert_eq!(parse_hours("2:30").unwrap(), 150);
        assert_eq!(parse_hours("09:00-12:30").unwrap(), 210);
        assert_eq!(parse_hours("9am-5pm").unwrap(), 480);
        assert_eq!(parse_hours("10pm-2am").unwrap(), 240);
    }

    #[test]
    fn rounding_uses_billing_increment() {
        assert_eq!(round_minutes(0, 15), 0);
        assert_eq!(round_minutes(1, 15), 15);
        assert_eq!(round_minutes(15, 15), 15);
        assert_eq!(round_minutes(16, 15), 30);
        assert_eq!(round_minutes(23, 6), 24);
        assert_eq!(round_minutes(23, 0), 23);
    }

    #[test]
    fn per_line_rate_override_and_dates() {
        let inv = build(
            "2026-08-01 | Consulting | 2 | 200\nSupport | 1",
            &Options {
                rate: 100.0,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(inv.lines[0].rate, 200.0);
        assert_eq!(inv.lines[0].amount, 400.0);
        assert_eq!(inv.lines[0].date.as_deref(), Some("2026-08-01"));
        assert_eq!(inv.lines[1].rate, 100.0);
        assert_eq!(inv.subtotal, 500.0);
    }

    #[test]
    fn discount_applies_before_tax() {
        let inv = build(
            "Work | 10",
            &Options {
                rate: 100.0,
                discount_percent: 10.0,
                tax_rate: 20.0,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(inv.subtotal, 1000.0);
        assert_eq!(inv.discount, 100.0);
        assert_eq!(inv.tax, 180.0);
        assert_eq!(inv.total, 1080.0);
    }

    #[test]
    fn group_by_description_merges_rows() {
        let inv = build(
            "Meetings | 1\nCoding | 2\nmeetings | 0.5",
            &Options {
                group_by: GroupBy::Description,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(inv.lines.len(), 2);
        assert_eq!(inv.lines[0].hours, 1.5);
        assert_eq!(inv.lines[1].description, "Coding");
    }

    #[test]
    fn group_by_date_merges_days() {
        let inv = build(
            "2026-08-01 | Draft | 1\n2026-08-01 | Review | 2\n2026-08-02 | Ship | 1",
            &Options {
                group_by: GroupBy::Date,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(inv.lines.len(), 2);
        assert_eq!(inv.lines[0].description, "Draft; Review");
        assert_eq!(inv.lines[0].hours, 3.0);
    }

    #[test]
    fn money_groups_thousands() {
        assert_eq!(money("$", 1234.5), "$1,234.50");
        assert_eq!(money("€", 1_000_000.0), "€1,000,000.00");
        assert_eq!(money("$", -42.0), "-$42.00");
        assert_eq!(money("$", 0.0), "$0.00");
    }

    #[test]
    fn explicit_due_date_wins_over_terms() {
        let inv = build(
            "Work | 1",
            &Options {
                due_date: "2026-12-31".into(),
                payment_terms: 30,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(inv.due_date, "2026-12-31");
    }

    #[test]
    fn error_on_bad_issue_date() {
        let err = generate(
            "Work | 1",
            &Options {
                issue_date: "14/08/2026".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("is not a YYYY-MM-DD date"), "{err}");
    }

    #[test]
    fn error_on_wrong_field_count() {
        let err = generate("just a description", &opts()).unwrap_err();
        assert!(err.contains("expected 2-4 fields"), "{err}");
    }

    #[test]
    fn text_and_csv_and_json_render() {
        let e = "2026-08-01 | Consulting | 2 | 200";
        let text = generate(
            e,
            &Options {
                format: OutputFormat::Text,
                ..opts()
            },
        )
        .unwrap();
        assert!(text.starts_with("INVOICE INV-001"), "{text}");
        assert!(text.contains("TOTAL DUE"), "{text}");

        let csv = generate(
            e,
            &Options {
                format: OutputFormat::Csv,
                ..opts()
            },
        )
        .unwrap();
        assert!(
            csv.starts_with("date,description,hours,rate,amount\n"),
            "{csv}"
        );
        assert!(
            csv.contains("2026-08-01,Consulting,2.00,200.00,400.00"),
            "{csv}"
        );
        assert!(csv.contains(",Total due,,,400.00"), "{csv}");

        let json = generate(
            e,
            &Options {
                format: OutputFormat::Json,
                ..opts()
            },
        )
        .unwrap();
        assert!(json.contains("\"total\": 400.0"), "{json}");
        assert!(json.contains("\"due_date\": \"2026-09-13\""), "{json}");
    }

    #[test]
    fn leap_year_due_date_arithmetic() {
        assert_eq!(add_days("2028-02-28", 1).unwrap(), "2028-02-29");
        assert_eq!(add_days("2026-02-28", 1).unwrap(), "2026-03-01");
        assert_eq!(add_days("2026-12-20", 30).unwrap(), "2027-01-19");
    }

    #[test]
    fn line_cap_is_enforced() {
        let big = "Row | 1\n".repeat(MAX_LINES + 1);
        let err = generate(&big, &opts()).unwrap_err();
        assert!(err.contains("too many entry lines"), "{err}");
        let ok = "Row | 1\n".repeat(MAX_LINES);
        assert!(generate(&ok, &opts()).is_ok());
    }

    #[test]
    fn tab_separated_rows_parse() {
        let inv = build("Design\t2\nBuild\t3", &opts()).unwrap();
        assert_eq!(inv.lines.len(), 2);
        assert_eq!(inv.total_hours, 5.0);
    }
}
