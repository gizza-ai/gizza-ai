//! log-return-calculator core — turns a price/level series into
//! continuously-compounded (log) returns. Pure compute, shared by the chat
//! skill block and the web page. No wafer/wasm-bindgen deps.
//!
//! The log return of one step is `r_t = ln(p_t / p_{t-1})`. Two properties make
//! it the standard input to quantitative work:
//!
//! * **time-additive** — the log returns of consecutive steps SUM to the log
//!   return of the whole span, `ln(p_n / p_0)`, so no chain-linking is needed;
//! * **symmetric** — a move up and the move that exactly undoes it are `+x` and
//!   `-x`, where simple returns give `+40%` and `-28.57%`.
//!
//! The price ratio must be positive, so a price of 0 or less is a hard error
//! rather than a silent `NaN`/`-inf`.
//!
//! Educational only — not financial advice.

use serde::Serialize;

/// Largest accepted series length. Well past any pasted spreadsheet column and
/// small enough that the rendered table stays usable.
pub const MAX_POINTS: usize = 2000;

/// One parsed price point: an optional row label (a date, a ticker period) and
/// its price/level.
#[derive(Debug, Clone, Serialize)]
pub struct Point {
    /// The row label as typed (`"2024-01-02"`), or the 1-based position when
    /// the series had no label column.
    pub label: String,
    pub price: f64,
}

/// One step between two consecutive prices.
#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub from_label: String,
    pub to_label: String,
    pub from: f64,
    pub to: f64,
    /// `ln(to / from)` — the continuously-compounded return of the step.
    pub log_return: f64,
    /// `to / from - 1` — the simple (arithmetic) return of the same step,
    /// shown alongside so the two conventions can be compared.
    pub simple_return: f64,
    /// Running sum of `log_return` from the first step up to this one.
    pub cumulative_log_return: f64,
}

/// The full analysis of a price series.
#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    /// Number of prices parsed.
    pub count: usize,
    /// Number of returns produced (`count - 1`).
    pub periods: usize,
    pub start_label: String,
    pub end_label: String,
    pub start_price: f64,
    pub end_price: f64,
    /// Periods per year used to annualize.
    pub periods_per_year: f64,
    /// Human name for `periods_per_year` (`"daily (trading days)"`, …).
    pub frequency: String,
    /// `ln(end / start)` — equal to the sum of every step's log return.
    pub total_log_return: f64,
    /// `exp(total_log_return) - 1` — the same span as a simple return.
    pub total_simple_return: f64,
    /// Arithmetic mean log return per period.
    pub mean_log_return: f64,
    /// Sample standard deviation (n−1) of the per-period log returns — the
    /// per-period volatility. `None` with fewer than 2 returns.
    pub stdev_log_return: Option<f64>,
    /// `mean_log_return × periods_per_year`.
    pub annualized_log_return: f64,
    /// `exp(annualized_log_return) - 1` — the annualized figure as a simple
    /// rate, i.e. the CAGR implied by the series.
    pub annualized_simple_return: f64,
    /// `stdev × √periods_per_year`. `None` with fewer than 2 returns.
    pub annualized_volatility: Option<f64>,
    /// Largest single-period log return, with the label of the step's end.
    pub best_period: (String, f64),
    /// Smallest single-period log return.
    pub worst_period: (String, f64),
    /// How many steps were strictly positive.
    pub positive_periods: usize,
    /// How many steps were strictly negative.
    pub negative_periods: usize,
    pub steps: Vec<Step>,
    pub note: String,
}

/// `periods_per_year` → a human frequency name for the header line.
fn frequency_label(ppy: f64) -> &'static str {
    match ppy as i64 {
        252 => "daily (trading days)",
        365 => "daily (calendar days)",
        52 => "weekly",
        26 => "biweekly",
        12 => "monthly",
        4 => "quarterly",
        1 => "annual",
        _ => "custom",
    }
}

fn round8(x: f64) -> f64 {
    (x * 1e8).round() / 1e8
}

/// Is every comma in `s` a valid thousands separator (a 1–3 digit lead group,
/// then groups of exactly 3 digits)? This is what keeps `1,234.50` a single
/// price while `2024-01-02,1234.50` stays a `label,price` row — a real
/// thousands-grouped number never has 4+ digits before its first comma.
fn commas_are_thousands(s: &str) -> bool {
    let b: Vec<char> = s.chars().collect();
    let first = match b.iter().position(|c| *c == ',') {
        Some(i) => i,
        None => return false,
    };
    let lead: Vec<char> = b[..first].to_vec();
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
        // Exactly three digits must follow, then end / `.` / the next group.
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

/// Parse one numeric cell, tolerating what people actually paste out of a
/// broker export or a spreadsheet: currency symbols, thousands separators
/// (comma, space, underscore) and a leading sign.
pub fn parse_number(raw: &str) -> Option<f64> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
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
    Some(v)
}

/// Split a labelled row into `(label, price)`. `2024-01-02,1,234.50` keeps the
/// thousands-grouped price by re-joining the tail before falling back to the
/// last field (so `2024-01-02,AAPL,187.15` still works).
fn split_labelled(line: &str, sep: char) -> Option<(String, f64)> {
    let fields: Vec<&str> = line.split(sep).collect();
    if fields.len() < 2 {
        return None;
    }
    let label = fields[0].trim().trim_matches('"').to_string();
    let joined = fields[1..].join(&sep.to_string());
    if let Some(v) = parse_number(&joined) {
        return Some((label, v));
    }
    let last = fields[fields.len() - 1];
    parse_number(last).map(|v| (label, v))
}

/// Parse the pasted series into price points. One point per line; a lone line
/// is instead split into many prices on commas/spaces/semicolons/tabs (so
/// `100, 105, 103` works).
pub fn parse_series(raw: &str, has_header: bool) -> Result<Vec<Point>, String> {
    let mut lines: Vec<&str> = raw
        .lines()
        .map(|l| l.trim_end_matches('\r').trim())
        .filter(|l| !l.is_empty())
        .collect();
    if has_header && !lines.is_empty() {
        lines.remove(0);
    }
    if lines.is_empty() {
        return Err(
            "no prices found — paste at least 2 prices, one per line or separated by commas".into(),
        );
    }

    // Single line → a delimiter-separated list of prices.
    if lines.len() == 1 {
        let parts: Vec<&str> = lines[0]
            .split([',', ';', '\t', ' '])
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .collect();
        let mut pts = Vec::new();
        for (i, p) in parts.iter().enumerate() {
            let price = parse_number(p).ok_or_else(|| {
                format!("price '{p}' is not a number (a one-line series must be all numbers, e.g. 100, 105, 103)")
            })?;
            pts.push(Point {
                label: (i + 1).to_string(),
                price,
            });
        }
        return finish_series(pts);
    }

    let mut pts = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(price) = parse_number(line) {
            pts.push(Point {
                label: (i + 1).to_string(),
                price,
            });
            continue;
        }
        let parsed = ['\t', ';', ','].iter().find_map(|s| split_labelled(line, *s));
        match parsed {
            Some((label, price)) => {
                let label = if label.is_empty() {
                    (i + 1).to_string()
                } else {
                    label
                };
                pts.push(Point { label, price });
            }
            None => {
                return Err(format!(
                    "line {} ('{}') is not a price or a 'label,price' pair — write one price per line, optionally as '2024-01-02,187.15'",
                    i + 1,
                    line
                ))
            }
        }
    }
    finish_series(pts)
}

/// Shared length + positivity checks. `ln(p_t / p_{t-1})` needs strictly
/// positive prices, so a 0 or negative level is rejected with its row.
fn finish_series(pts: Vec<Point>) -> Result<Vec<Point>, String> {
    if pts.len() < 2 {
        return Err(format!(
            "need at least 2 prices to compute a log return, got {}",
            pts.len()
        ));
    }
    if pts.len() > MAX_POINTS {
        return Err(format!(
            "series is too long: {} prices, the maximum is {MAX_POINTS}",
            pts.len()
        ));
    }
    for (i, p) in pts.iter().enumerate() {
        if p.price <= 0.0 {
            return Err(format!(
                "price {} at row {} is not positive — a log return is ln(price ÷ previous price), which is undefined for 0 or negative prices",
                trim_num(p.price),
                i + 1
            ));
        }
    }
    Ok(pts)
}

/// Compute every log return plus the headline statistics.
pub fn analyze(prices: &str, has_header: bool, periods_per_year: f64) -> Result<Analysis, String> {
    if !(periods_per_year.is_finite() && periods_per_year > 0.0) {
        return Err(format!(
            "periods_per_year must be a positive number, got {}",
            trim_num(periods_per_year)
        ));
    }
    let pts = parse_series(prices, has_header)?;

    let mut steps: Vec<Step> = Vec::with_capacity(pts.len() - 1);
    let mut cumulative = 0.0f64;
    for w in pts.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let log_return = (b.price / a.price).ln();
        cumulative += log_return;
        steps.push(Step {
            from_label: a.label.clone(),
            to_label: b.label.clone(),
            from: a.price,
            to: b.price,
            log_return: round8(log_return),
            simple_return: round8(b.price / a.price - 1.0),
            cumulative_log_return: round8(cumulative),
        });
    }

    let n = steps.len() as f64;
    let sum: f64 = steps.iter().map(|s| s.log_return).sum();
    let mean = sum / n;
    let stdev = if steps.len() >= 2 {
        let var = steps
            .iter()
            .map(|s| (s.log_return - mean).powi(2))
            .sum::<f64>()
            / (n - 1.0);
        Some(var.sqrt())
    } else {
        None
    };

    let best = steps
        .iter()
        .max_by(|a, b| a.log_return.total_cmp(&b.log_return))
        .expect("at least one step");
    let worst = steps
        .iter()
        .min_by(|a, b| a.log_return.total_cmp(&b.log_return))
        .expect("at least one step");

    let start_price = pts[0].price;
    let end_price = pts[pts.len() - 1].price;
    let total_log = (end_price / start_price).ln();
    let annualized_log = mean * periods_per_year;

    let note = format!(
        "Log returns are time-additive: the {} period returns sum to the {} total log return. \
Educational only — not financial advice.",
        steps.len(),
        fmt_signed_pct(total_log, 4)
    );

    Ok(Analysis {
        count: pts.len(),
        periods: steps.len(),
        start_label: pts[0].label.clone(),
        end_label: pts[pts.len() - 1].label.clone(),
        start_price,
        end_price,
        periods_per_year,
        frequency: frequency_label(periods_per_year).to_string(),
        total_log_return: round8(total_log),
        total_simple_return: round8(total_log.exp() - 1.0),
        mean_log_return: round8(mean),
        stdev_log_return: stdev.map(round8),
        annualized_log_return: round8(annualized_log),
        annualized_simple_return: round8(annualized_log.exp() - 1.0),
        annualized_volatility: stdev.map(|s| round8(s * periods_per_year.sqrt())),
        best_period: (best.to_label.clone(), best.log_return),
        worst_period: (worst.to_label.clone(), worst.log_return),
        positive_periods: steps.iter().filter(|s| s.log_return > 0.0).count(),
        negative_periods: steps.iter().filter(|s| s.log_return < 0.0).count(),
        steps,
        note,
    })
}

/// Drop trailing zeros from a plain number so error messages read naturally.
fn trim_num(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() {
        "0".to_string()
    } else {
        s
    }
}

/// `+1.1778%` — always signed, for headline figures.
fn fmt_signed_pct(v: f64, dp: usize) -> String {
    format!("{:+.*}%", dp, v * 100.0)
}

/// Render one return in the requested unit.
fn fmt_return(v: f64, unit: Unit, dp: usize) -> String {
    match unit {
        Unit::Percent => format!("{:+.*}%", dp, v * 100.0),
        Unit::Decimal => format!("{:+.*}", dp, v),
    }
}

/// A magnitude (a volatility), rendered unsigned.
fn fmt_magnitude(v: f64, unit: Unit, dp: usize) -> String {
    match unit {
        Unit::Percent => format!("{:.*}%", dp, v * 100.0),
        Unit::Decimal => format!("{:.*}", dp, v),
    }
}

/// A price, with thousands separators and up to 6 decimals.
fn fmt_price(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    match s.split_once('.') {
        Some((i, f)) => format!("{}.{}", group_thousands(i), f),
        None => group_thousands(&s),
    }
}

fn group_thousands(int_part: &str) -> String {
    let (sign, digits) = match int_part.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", int_part),
    };
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{sign}{out}")
}

/// How each return is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// `+1.1778%` — the return scaled by 100.
    Percent,
    /// `+0.011778` — the raw natural-log value.
    Decimal,
}

impl Unit {
    fn parse(s: &str) -> Result<Unit, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "percent" | "" => Ok(Unit::Percent),
            "decimal" => Ok(Unit::Decimal),
            other => Err(format!(
                "unit must be one of percent, decimal — got '{other}'"
            )),
        }
    }
}

fn pad_right(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

fn pad_left(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(w - n))
    }
}

/// The per-row table: one row per PRICE, so the first row shows the opening
/// price with no return yet (there is no earlier price to divide by).
fn render_table(a: &Analysis, unit: Unit, dp: usize) -> String {
    let headers = [
        "#",
        "label",
        "price",
        "log return",
        "simple return",
        "cumulative log",
    ];
    let mut rows: Vec<[String; 6]> = Vec::with_capacity(a.count);
    rows.push([
        "1".to_string(),
        a.start_label.clone(),
        fmt_price(a.start_price),
        "—".to_string(),
        "—".to_string(),
        "—".to_string(),
    ]);
    for (i, s) in a.steps.iter().enumerate() {
        rows.push([
            (i + 2).to_string(),
            s.to_label.clone(),
            fmt_price(s.to),
            fmt_return(s.log_return, unit, dp),
            fmt_return(s.simple_return, unit, dp),
            fmt_return(s.cumulative_log_return, unit, dp),
        ]);
    }

    let mut widths = [0usize; 6];
    for (c, h) in headers.iter().enumerate() {
        widths[c] = h.chars().count();
    }
    for r in &rows {
        for (c, cell) in r.iter().enumerate() {
            widths[c] = widths[c].max(cell.chars().count());
        }
    }

    let mut out = String::new();
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(c, h)| {
            if c <= 1 {
                pad_right(h, widths[c])
            } else {
                pad_left(h, widths[c])
            }
        })
        .collect();
    out.push_str(header_line.join("  ").trim_end());
    out.push('\n');
    out.push_str(&"-".repeat(widths.iter().sum::<usize>() + 2 * (headers.len() - 1)));
    out.push('\n');
    for r in &rows {
        let line: Vec<String> = r
            .iter()
            .enumerate()
            .map(|(c, cell)| {
                if c <= 1 {
                    pad_right(cell, widths[c])
                } else {
                    pad_left(cell, widths[c])
                }
            })
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    }
    out
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// CSV of the same table, ready to paste back into a spreadsheet. Returns are
/// written as plain numbers (no `%`, no `+`) in the requested unit so the
/// columns stay numeric.
fn render_csv(a: &Analysis, unit: Unit, dp: usize) -> String {
    let scale = if unit == Unit::Percent { 100.0 } else { 1.0 };
    let suffix = if unit == Unit::Percent {
        "_percent"
    } else {
        "_decimal"
    };
    let mut out = format!(
        "label,price,log_return{suffix},simple_return{suffix},cumulative_log_return{suffix}\n"
    );
    out.push_str(&format!(
        "{},{},,,\n",
        csv_cell(&a.start_label),
        a.start_price
    ));
    for s in &a.steps {
        out.push_str(&format!(
            "{},{},{:.*},{:.*},{:.*}\n",
            csv_cell(&s.to_label),
            s.to,
            dp,
            s.log_return * scale,
            dp,
            s.simple_return * scale,
            dp,
            s.cumulative_log_return * scale
        ));
    }
    out
}

fn render_summary(a: &Analysis, unit: Unit, dp: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Log returns for {} prices → {} returns ({}, {} periods/year)\n\n",
        a.count,
        a.periods,
        a.frequency,
        trim_num(a.periods_per_year)
    ));
    out.push_str(&format!(
        "first price: {} at {}\nlast price:  {} at {}\n\n",
        fmt_price(a.start_price),
        a.start_label,
        fmt_price(a.end_price),
        a.end_label
    ));
    out.push_str(&format!(
        "total log return:        {}\n",
        fmt_return(a.total_log_return, unit, dp)
    ));
    out.push_str(&format!(
        "same span, simple:       {}\n",
        fmt_return(a.total_simple_return, unit, dp)
    ));
    out.push_str(&format!(
        "mean log return:         {} per period\n",
        fmt_return(a.mean_log_return, unit, dp)
    ));
    match a.stdev_log_return {
        Some(s) => out.push_str(&format!(
            "volatility (std dev):    {} per period\n",
            fmt_magnitude(s, unit, dp)
        )),
        None => out.push_str("volatility (std dev):    n/a (needs at least 2 returns)\n"),
    }
    out.push_str(&format!(
        "annualized log return:   {} per year\n",
        fmt_return(a.annualized_log_return, unit, dp)
    ));
    out.push_str(&format!(
        "annualized, simple:      {} per year\n",
        fmt_return(a.annualized_simple_return, unit, dp)
    ));
    match a.annualized_volatility {
        Some(v) => out.push_str(&format!(
            "annualized volatility:   {} per year\n",
            fmt_magnitude(v, unit, dp)
        )),
        None => out.push_str("annualized volatility:   n/a (needs at least 2 returns)\n"),
    }
    out.push_str(&format!(
        "best period:             {} at {}\n",
        fmt_return(a.best_period.1, unit, dp),
        a.best_period.0
    ));
    out.push_str(&format!(
        "worst period:            {} at {}\n",
        fmt_return(a.worst_period.1, unit, dp),
        a.worst_period.0
    ));
    out.push_str(&format!(
        "positive periods:        {} of {} ({} negative)\n\n",
        a.positive_periods, a.periods, a.negative_periods
    ));
    out.push_str(&render_table(a, unit, dp));
    out.push('\n');
    out.push_str(&a.note);
    out
}

/// Analyse a price series and render it in the requested `output` shape:
/// `summary` (headline metrics + the table), `table`, `csv` or `json`.
pub fn summary(
    prices: &str,
    has_header: bool,
    periods_per_year: f64,
    unit: &str,
    decimals: i64,
    output: &str,
) -> Result<String, String> {
    if !(0..=10).contains(&decimals) {
        return Err(format!(
            "decimals must be between 0 and 10, got {decimals}"
        ));
    }
    let unit = Unit::parse(unit)?;
    let dp = decimals as usize;
    let a = analyze(prices, has_header, periods_per_year)?;
    match output.trim().to_ascii_lowercase().as_str() {
        "summary" | "" => Ok(render_summary(&a, unit, dp)),
        "table" => Ok(render_table(&a, unit, dp).trim_end().to_string()),
        "csv" => Ok(render_csv(&a, unit, dp).trim_end().to_string()),
        "json" => serde_json::to_string_pretty(&a).map_err(|e| e.to_string()),
        other => Err(format!(
            "output must be one of summary, table, csv, json — got '{other}'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn happy_path_two_prices() {
        let a = analyze("100\n120", false, 252.0).unwrap();
        assert_eq!(a.count, 2);
        assert_eq!(a.periods, 1);
        // ln(120/100) = ln(1.2) = 0.18232156
        approx(a.total_log_return, 0.18232156);
        approx(a.steps[0].log_return, 0.18232156);
        approx(a.total_simple_return, 0.2);
        approx(a.steps[0].simple_return, 0.2);
        // A single return has no sample standard deviation.
        assert!(a.stdev_log_return.is_none());
        assert!(a.annualized_volatility.is_none());
    }

    #[test]
    fn log_returns_are_time_additive() {
        let a = analyze("100\n110\n99\n150", false, 12.0).unwrap();
        let sum: f64 = a.steps.iter().map(|s| s.log_return).sum();
        approx(sum, a.total_log_return);
        approx(a.total_log_return, (150.0f64 / 100.0).ln());
        approx(
            a.steps[a.steps.len() - 1].cumulative_log_return,
            a.total_log_return,
        );
    }

    #[test]
    fn up_and_down_moves_are_symmetric() {
        // +40% then the drop that returns to the start: log returns cancel.
        let a = analyze("50\n70\n50", false, 1.0).unwrap();
        approx(a.steps[0].log_return, -a.steps[1].log_return);
        approx(a.total_log_return, 0.0);
        // The simple returns do NOT cancel: +40% then -28.5714%.
        approx(a.steps[0].simple_return, 0.4);
        approx(a.steps[1].simple_return, -0.28571429);
    }

    #[test]
    fn annualization_uses_periods_per_year() {
        let a = analyze("100\n101\n102\n103", false, 252.0).unwrap();
        approx(a.annualized_log_return, a.mean_log_return * 252.0);
        approx(
            a.annualized_volatility.unwrap(),
            a.stdev_log_return.unwrap() * 252.0f64.sqrt(),
        );
        // The annualized simple rate is exp() of the annualized log rate.
        approx(a.annualized_simple_return, a.annualized_log_return.exp() - 1.0);
    }

    #[test]
    fn monthly_frequency_label_and_stats() {
        let a = analyze("100\n105\n103\n108", false, 12.0).unwrap();
        assert_eq!(a.frequency, "monthly");
        assert_eq!(a.positive_periods, 2);
        assert_eq!(a.negative_periods, 1);
        // ln(105/100)=0.04879 beats ln(108/103)=0.04739, so row 2 is the best
        // step and row 3 (105→103) is the only negative one.
        assert_eq!(a.best_period.0, "2");
        assert_eq!(a.worst_period.0, "3");
    }

    #[test]
    fn labelled_rows_currency_and_thousands() {
        let a = analyze(
            "2024-01-02,$1,000.00\n2024-01-03,€1,100.00\n2024-01-04,1 210",
            false,
            252.0,
        )
        .unwrap();
        assert_eq!(a.count, 3);
        assert_eq!(a.start_label, "2024-01-02");
        assert_eq!(a.end_label, "2024-01-04");
        approx(a.start_price, 1000.0);
        approx(a.end_price, 1210.0);
        approx(a.steps[0].log_return, (1.1f64).ln());
    }

    #[test]
    fn one_line_series_and_header_skip() {
        let one = analyze("100, 105, 103", false, 252.0).unwrap();
        assert_eq!(one.count, 3);
        let hdr = analyze("date,close\n2024-01-02,100\n2024-01-03,105", true, 252.0).unwrap();
        assert_eq!(hdr.count, 2);
        assert_eq!(hdr.start_label, "2024-01-02");
    }

    #[test]
    fn errors() {
        let e = analyze("100", false, 252.0).unwrap_err();
        assert!(e.contains("at least 2 prices"), "{e}");

        let e = analyze("100\n0\n120", false, 252.0).unwrap_err();
        assert!(e.contains("row 2"), "{e}");
        assert!(e.contains("undefined for 0 or negative"), "{e}");

        let e = analyze("100\n-50", false, 252.0).unwrap_err();
        assert!(e.contains("not positive"), "{e}");

        let e = analyze("100\nabc def\n120", false, 252.0).unwrap_err();
        assert!(e.contains("line 2"), "{e}");

        let e = analyze("100\n110", false, 0.0).unwrap_err();
        assert!(e.contains("periods_per_year must be a positive number"), "{e}");
    }

    #[test]
    fn cap_boundary() {
        let at_cap = (0..MAX_POINTS)
            .map(|i| (100 + i).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(analyze(&at_cap, false, 252.0).unwrap().count, MAX_POINTS);

        let over = (0..MAX_POINTS + 1)
            .map(|i| (100 + i).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let e = analyze(&over, false, 252.0).unwrap_err();
        assert!(e.contains("too long"), "{e}");
        assert!(e.contains("2001"), "{e}");
    }

    #[test]
    fn summary_renders_exact_headline() {
        let out = summary("100\n120", false, 252.0, "percent", 4, "summary").unwrap();
        assert!(
            out.starts_with("Log returns for 2 prices → 1 returns (daily (trading days), 252 periods/year)"),
            "{out}"
        );
        assert!(out.contains("total log return:        +18.2322%"), "{out}");
        assert!(out.contains("same span, simple:       +20.0000%"), "{out}");
        assert!(out.contains("time-additive"), "{out}");
    }

    #[test]
    fn decimal_unit_and_decimals() {
        let out = summary("100\n120", false, 252.0, "decimal", 6, "summary").unwrap();
        assert!(out.contains("total log return:        +0.182322"), "{out}");
        let zero = summary("100\n120", false, 12.0, "percent", 0, "table").unwrap();
        assert!(zero.contains("+18%"), "{zero}");
    }

    #[test]
    fn table_csv_and_json_outputs() {
        let table = summary("100\n120", false, 252.0, "percent", 4, "table").unwrap();
        assert!(table.starts_with("#  label"), "{table}");
        // The opening price has no return yet.
        assert!(table.contains("—"), "{table}");

        let csv = summary("100\n120", false, 252.0, "percent", 4, "csv").unwrap();
        assert_eq!(
            csv.lines().next().unwrap(),
            "label,price,log_return_percent,simple_return_percent,cumulative_log_return_percent"
        );
        assert_eq!(csv.lines().nth(1).unwrap(), "1,100,,,");
        assert_eq!(csv.lines().nth(2).unwrap(), "2,120,18.2322,20.0000,18.2322");

        let json = summary("100\n120", false, 252.0, "decimal", 4, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["periods"], 1);
        approx(v["total_log_return"].as_f64().unwrap(), 0.18232156);
    }

    #[test]
    fn bad_unit_output_and_decimals() {
        let e = summary("100\n120", false, 252.0, "bps", 4, "summary").unwrap_err();
        assert!(e.contains("unit must be one of"), "{e}");
        let e = summary("100\n120", false, 252.0, "percent", 4, "graph").unwrap_err();
        assert!(e.contains("output must be one of"), "{e}");
        let e = summary("100\n120", false, 252.0, "percent", 11, "summary").unwrap_err();
        assert!(e.contains("decimals must be between 0 and 10"), "{e}");
    }
}
