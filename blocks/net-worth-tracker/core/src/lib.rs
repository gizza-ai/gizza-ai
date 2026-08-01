//! net-worth-tracker core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Turns a pasted list of assets and liabilities into a personal balance sheet:
//! total assets, total liabilities, and **net worth = assets − liabilities**,
//! plus a per-category breakdown of each side (value, share of that side, a
//! proportional bar and an item count) and the debt-to-asset ratio. Every row is
//! `label, amount, type, category`; the type column (asset/liability) is optional
//! — a negative amount is read as a liability. Deterministic; runs locally on
//! every surface. Money math is done in whole cents so the two sides always
//! reconcile to the penny.

use std::collections::HashMap;

/// Guard rails so a giant paste can't exhaust the wasm sandbox.
const MAX_INPUT_BYTES: usize = 5_000_000;
const MAX_ENTRIES: usize = 100_000;

/// Bucket label used when a row gives no category.
const UNCATEGORIZED: &str = "Uncategorized";

/// Which side of the balance sheet a row lands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Asset,
    Liability,
}

/// How to order the category rows within each side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Largest value first (default), ties broken by first-seen order.
    Value,
    /// Alphabetical by category label (case-insensitive).
    Label,
}

impl Sort {
    fn parse(s: &str) -> Result<Sort, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "value" | "" => Ok(Sort::Value),
            "label" | "name" | "alpha" | "alphabetical" | "category" => Ok(Sort::Label),
            other => Err(format!("sort must be 'value' or 'label', got '{other}'")),
        }
    }
}

/// A parsed balance-sheet row: a positive magnitude in cents on one side.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub label: String,
    /// Positive magnitude, in whole cents.
    pub cents: i64,
    pub kind: Kind,
    pub category: String,
}

/// One category line within a side — value, its share of that side, item count.
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryLine {
    pub label: String,
    /// Total of the entries in this category, in cents.
    pub cents: i64,
    /// Share of this SIDE's total (0–100, rounded to 2 decimals).
    pub percent: f64,
    pub count: usize,
}

/// The full net-worth breakdown.
#[derive(Debug, Clone, PartialEq)]
pub struct NetWorth {
    /// Asset categories, ranked per the requested sort.
    pub assets: Vec<CategoryLine>,
    /// Liability categories, ranked per the requested sort.
    pub liabilities: Vec<CategoryLine>,
    /// Sum of every asset, in cents.
    pub total_assets: i64,
    /// Sum of every liability, in cents.
    pub total_liabilities: i64,
    /// total_assets − total_liabilities, in cents (may be negative).
    pub net_worth: i64,
    /// liabilities ÷ assets as a percentage (0–100+), rounded to 2 decimals;
    /// `None` when there are no assets.
    pub debt_to_asset: Option<f64>,
    /// Number of asset entries parsed.
    pub asset_count: usize,
    /// Number of liability entries parsed.
    pub liability_count: usize,
}

/// Split one input line into cells. Prefers a tab delimiter (spreadsheet paste)
/// so a thousands-separated value like `1,500` survives; otherwise commas.
fn split_cells(line: &str) -> Vec<String> {
    let delim = if line.contains('\t') { '\t' } else { ',' };
    line.split(delim).map(|c| c.trim().to_string()).collect()
}

/// Recognise an asset/liability type keyword. Returns `None` when the cell isn't
/// a type token (so it can be treated as a category instead).
fn parse_kind(cell: &str) -> Option<Kind> {
    match cell.trim().to_ascii_lowercase().as_str() {
        "asset" | "assets" | "a" => Some(Kind::Asset),
        "liability" | "liabilities" | "liab" | "debt" | "l" => Some(Kind::Liability),
        _ => None,
    }
}

/// Strip currency/grouping ornamentation from a numeric string and parse it to a
/// signed amount. Accepts `$1,234.50`, `-500`, and accounting negatives `(500)`.
fn clean_number(s: &str) -> Result<f64, String> {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, '$' | '€' | '£' | '¥' | ',' | '_' | ' ' | '\''))
        .collect();
    // An accounting-style "(123.45)" denotes a negative amount.
    let (neg, body) = match cleaned.strip_prefix('(').and_then(|b| b.strip_suffix(')')) {
        Some(inner) => (true, inner.to_string()),
        None => (false, cleaned),
    };
    if body.is_empty() {
        return Err("missing amount".to_string());
    }
    let n: f64 = body
        .parse()
        .map_err(|_| format!("expected a number, got '{}'", s.trim()))?;
    if !n.is_finite() {
        return Err("amount must be a finite number".to_string());
    }
    Ok(if neg { -n } else { n })
}

/// Parse an amount cell, supporting a `shares @ price` / `shares x price` form
/// (e.g. `10 @ 150` = 1500). Returns a signed dollar amount.
fn parse_amount(cell: &str) -> Result<f64, String> {
    let cell = cell.trim();
    if cell.is_empty() {
        return Err("missing amount".to_string());
    }
    for sep in ['@', 'x', 'X', '*'] {
        if let Some((a, b)) = cell.split_once(sep) {
            let qty = clean_number(a)?;
            let price = clean_number(b)?;
            return Ok(qty * price);
        }
    }
    clean_number(cell)
}

/// Convert a signed dollar amount to whole cents (rounded).
fn to_cents(dollars: f64) -> i64 {
    (dollars * 100.0).round() as i64
}

/// Does this cell look like an amount (used to detect a header row)?
fn looks_numeric(cell: &str) -> bool {
    parse_amount(cell).is_ok()
}

/// Parse the pasted balance-sheet block. Blank lines and `#` comments are
/// skipped; a leading header row (whose amount column isn't a number) is dropped.
///
/// Row shape: `label, amount[, type][, category]`. The type token (asset /
/// liability) may appear in either of the two columns after the amount, in any
/// order relative to the category; a row with no type token is classified by the
/// sign of its amount (negative → liability, otherwise → asset).
pub fn parse_entries(input: &str) -> Result<Vec<Entry>, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is too large ({} bytes; max {})",
            input.len(),
            MAX_INPUT_BYTES
        ));
    }
    let mut entries = Vec::new();
    let mut seen_data = false;
    for (i, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cells = split_cells(line);
        // A header row: first data-ish line whose amount cell isn't numeric.
        if !seen_data && cells.len() >= 2 && !looks_numeric(&cells[1]) {
            continue;
        }
        let label = cells.first().map(|s| s.as_str()).unwrap_or("").trim();
        if label.is_empty() {
            return Err(format!("line {}: missing label (item name)", i + 1));
        }
        if cells.len() < 2 {
            return Err(format!(
                "line {}: '{}' has no amount — enter 'label, amount[, type][, category]'",
                i + 1,
                label
            ));
        }
        let signed = parse_amount(&cells[1])
            .map_err(|e| format!("line {}: amount for '{}' — {}", i + 1, label, e))?;

        // Classify the two optional trailing cells: one may be a type keyword,
        // the rest is the category label.
        let mut explicit_kind: Option<Kind> = None;
        let mut category: Option<String> = None;
        for cell in cells.iter().skip(2) {
            let c = cell.trim();
            if c.is_empty() {
                continue;
            }
            match parse_kind(c) {
                Some(k) if explicit_kind.is_none() => explicit_kind = Some(k),
                _ => {
                    if category.is_none() {
                        category = Some(c.to_string());
                    }
                }
            }
        }

        let kind = explicit_kind.unwrap_or(if signed < 0.0 {
            Kind::Liability
        } else {
            Kind::Asset
        });
        let cents = to_cents(signed).abs();

        entries.push(Entry {
            label: label.to_string(),
            cents,
            kind,
            category: category.unwrap_or_else(|| UNCATEGORIZED.to_string()),
        });
        seen_data = true;
        if entries.len() > MAX_ENTRIES {
            return Err(format!("too many entries (max {})", MAX_ENTRIES));
        }
    }
    if entries.is_empty() {
        return Err(
            "no entries found — enter at least one line as 'label, amount[, type][, category]'"
                .to_string(),
        );
    }
    Ok(entries)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Build the per-side category lines (value + share of side + count), ranked.
fn side_lines(entries: &[Entry], side: Kind, sort: Sort) -> (Vec<CategoryLine>, i64, usize) {
    let mut totals: HashMap<String, (i64, usize)> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut total: i64 = 0;
    let mut count = 0usize;
    for e in entries.iter().filter(|e| e.kind == side) {
        let entry = totals.entry(e.category.clone()).or_insert_with(|| {
            order.push(e.category.clone());
            (0, 0)
        });
        entry.0 += e.cents;
        entry.1 += 1;
        total += e.cents;
        count += 1;
    }

    let mut ranked: Vec<(usize, String)> = order.into_iter().enumerate().collect();
    match sort {
        Sort::Value => {
            ranked.sort_by(|a, b| totals[&b.1].0.cmp(&totals[&a.1].0).then(a.0.cmp(&b.0)))
        }
        Sort::Label => ranked.sort_by(|a, b| {
            a.1.to_lowercase()
                .cmp(&b.1.to_lowercase())
                .then(a.0.cmp(&b.0))
        }),
    }

    let lines = ranked
        .into_iter()
        .map(|(_, k)| {
            let (cents, c) = totals[&k];
            let percent = if total == 0 {
                0.0
            } else {
                round2(cents as f64 / total as f64 * 100.0)
            };
            CategoryLine {
                label: k,
                cents,
                percent,
                count: c,
            }
        })
        .collect();
    (lines, total, count)
}

/// Compute the full net-worth breakdown from parsed entries.
pub fn compute(entries: &[Entry], sort: Sort) -> NetWorth {
    let (assets, total_assets, asset_count) = side_lines(entries, Kind::Asset, sort);
    let (liabilities, total_liabilities, liability_count) =
        side_lines(entries, Kind::Liability, sort);
    let debt_to_asset = if total_assets == 0 {
        None
    } else {
        Some(round2(total_liabilities as f64 / total_assets as f64 * 100.0))
    };
    NetWorth {
        assets,
        liabilities,
        total_assets,
        total_liabilities,
        net_worth: total_assets - total_liabilities,
        debt_to_asset,
        asset_count,
        liability_count,
    }
}

/// Format `cents` as `<sym>1,234.56` (accounting-negative `-<sym>1,234.56`).
fn format_money(cents: i64, sym: &str) -> String {
    let neg = cents < 0;
    let abs = cents.unsigned_abs();
    let whole = abs / 100;
    let rem = abs % 100;
    let mut digits = whole.to_string();
    let mut grouped = String::new();
    while digits.len() > 3 {
        let split = digits.len() - 3;
        grouped = format!(",{}{}", &digits[split..], grouped);
        digits.truncate(split);
    }
    let grouped = format!("{}{}", digits, grouped);
    format!("{}{}{}.{:02}", if neg { "-" } else { "" }, sym, grouped, rem)
}

const BAR: usize = 24;

/// Render one side (Assets / Liabilities) as aligned category lines with bars.
fn render_side(
    title: &str,
    lines: &[CategoryLine],
    total: i64,
    count: usize,
    sym: &str,
    out: &mut Vec<String>,
) {
    out.push(format!(
        "{} — {} total across {} item{}",
        title,
        format_money(total, sym),
        count,
        if count == 1 { "" } else { "s" }
    ));
    if lines.is_empty() {
        out.push("  (none)".to_string());
        return;
    }
    let label_w = lines
        .iter()
        .map(|l| l.label.chars().count())
        .max()
        .unwrap_or(1);
    let money_w = lines
        .iter()
        .map(|l| format_money(l.cents, sym).chars().count())
        .max()
        .unwrap_or(1);
    for l in lines {
        let filled = ((l.percent / 100.0) * BAR as f64).round() as usize;
        let filled = filled.min(BAR);
        let bar: String = "█".repeat(filled);
        let pad: String = "·".repeat(BAR - filled);
        out.push(format!(
            "  {:label_w$}  {:>money_w$}  {:>6.2}%  {}{}  ({} item{})",
            l.label,
            format_money(l.cents, sym),
            l.percent,
            bar,
            pad,
            l.count,
            if l.count == 1 { "" } else { "s" },
            label_w = label_w,
            money_w = money_w,
        ));
    }
}

/// Plain-text balance-sheet report (the page surface).
pub fn format_report(input: &str, sort: Sort, currency: &str) -> Result<String, String> {
    let entries = parse_entries(input)?;
    let nw = compute(&entries, sort);
    let sym = currency.trim();

    let mut out: Vec<String> = Vec::new();
    out.push(format!(
        "Net worth: {}   (Assets {} − Liabilities {})",
        format_money(nw.net_worth, sym),
        format_money(nw.total_assets, sym),
        format_money(nw.total_liabilities, sym),
    ));
    out.push(String::new());
    render_side(
        "Assets",
        &nw.assets,
        nw.total_assets,
        nw.asset_count,
        sym,
        &mut out,
    );
    out.push(String::new());
    render_side(
        "Liabilities",
        &nw.liabilities,
        nw.total_liabilities,
        nw.liability_count,
        sym,
        &mut out,
    );
    out.push(String::new());
    match nw.debt_to_asset {
        Some(ratio) => {
            let equity = round2(100.0 - ratio);
            out.push(format!(
                "Debt-to-asset ratio: {:.2}%   (you own {:.2}% of your assets)",
                ratio, equity
            ));
        }
        None => out.push("Debt-to-asset ratio: n/a (no assets entered)".to_string()),
    }
    Ok(out.join("\n"))
}

/// Scaffold-compatible entry point; the real surfaces call `format_report`.
pub fn run(input: &str) -> Result<String, String> {
    format_report(input, Sort::Value, "$")
}

/// Re-export the string parser for the block/web wrappers.
pub fn parse_sort(s: &str) -> Result<Sort, String> {
    Sort::parse(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Vec<Entry> {
        parse_entries(s).unwrap()
    }

    #[test]
    fn basic_net_worth() {
        let input = "Checking, 5000, asset, Cash\nBrokerage, 15000, asset, Investments\nMortgage, 120000, liability, Real Estate\nHome, 200000, asset, Real Estate";
        let e = parse(input);
        let nw = compute(&e, Sort::Value);
        assert_eq!(nw.total_assets, 220000_00);
        assert_eq!(nw.total_liabilities, 120000_00);
        assert_eq!(nw.net_worth, 100000_00);
    }

    #[test]
    fn negative_amount_is_liability() {
        // No type column: sign classifies the row.
        let input = "Savings, 10000, Cash\nCredit Card, -2500, Cards";
        let e = parse(input);
        assert_eq!(e[0].kind, Kind::Asset);
        assert_eq!(e[1].kind, Kind::Liability);
        assert_eq!(e[1].cents, 2500_00); // stored as positive magnitude
        let nw = compute(&e, Sort::Value);
        assert_eq!(nw.net_worth, 7500_00);
    }

    #[test]
    fn accounting_negative_is_liability() {
        let e = parse("Loan, (500)");
        assert_eq!(e[0].kind, Kind::Liability);
        assert_eq!(e[0].cents, 500_00);
    }

    #[test]
    fn explicit_type_overrides_sign() {
        // A liability written as a positive number still needs its type column.
        let e = parse("Student Loan, 30000, liability, Education");
        assert_eq!(e[0].kind, Kind::Liability);
        assert_eq!(e[0].category, "Education");
    }

    #[test]
    fn type_and_category_order_insensitive() {
        let a = parse("X, 100, asset, Cash");
        let b = parse("X, 100, Cash, asset");
        assert_eq!(a[0].kind, b[0].kind);
        assert_eq!(a[0].category, b[0].category);
        assert_eq!(a[0].category, "Cash");
    }

    #[test]
    fn categories_merge_and_percent() {
        let input =
            "AAPL, 6000, asset, Investments\nVOO, 2000, asset, Investments\nChecking, 2000, asset, Cash";
        let nw = compute(&parse(input), Sort::Value);
        assert_eq!(nw.total_assets, 10000_00);
        assert_eq!(nw.assets[0].label, "Investments");
        assert_eq!(nw.assets[0].cents, 8000_00);
        assert_eq!(nw.assets[0].count, 2);
        assert_eq!(nw.assets[0].percent, 80.0);
        assert_eq!(nw.assets[1].percent, 20.0);
    }

    #[test]
    fn missing_category_is_uncategorized() {
        let nw = compute(&parse("Cash pile, 100"), Sort::Value);
        assert_eq!(nw.assets[0].label, "Uncategorized");
    }

    #[test]
    fn debt_to_asset_ratio() {
        let input = "Home, 100000, asset\nMortgage, 25000, liability";
        let nw = compute(&parse(input), Sort::Value);
        assert_eq!(nw.debt_to_asset, Some(25.0));
    }

    #[test]
    fn no_assets_ratio_none() {
        let nw = compute(&parse("Card, -500"), Sort::Value);
        assert_eq!(nw.debt_to_asset, None);
        assert_eq!(nw.net_worth, -500_00);
    }

    #[test]
    fn negative_net_worth() {
        let input = "Car, 10000, asset\nAuto Loan, 15000, liability";
        let nw = compute(&parse(input), Sort::Value);
        assert_eq!(nw.net_worth, -5000_00);
    }

    #[test]
    fn label_sort_orders_categories() {
        let input = "A, 1, asset, Zeta\nB, 1, asset, Alpha\nC, 1, asset, Mid";
        let nw = compute(&parse(input), Sort::Label);
        assert_eq!(nw.assets[0].label, "Alpha");
        assert_eq!(nw.assets[1].label, "Mid");
        assert_eq!(nw.assets[2].label, "Zeta");
    }

    #[test]
    fn shares_at_price() {
        let e = parse("AAPL, 10 @ 150, asset, Investments");
        assert_eq!(e[0].cents, 1500_00);
    }

    #[test]
    fn currency_and_thousands_stripped() {
        let e = parse("Fund\t$1,234.50\tasset\tInvestments");
        assert_eq!(e[0].cents, 1234_50);
        assert_eq!(e[0].category, "Investments");
    }

    #[test]
    fn header_row_dropped() {
        let input = "Name, Amount, Type, Category\nChecking, 100, asset, Cash";
        let e = parse(input);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].label, "Checking");
    }

    #[test]
    fn comments_and_blanks_skipped() {
        let e = parse("# my balance sheet\nChecking, 100\n\nSavings, 200\n");
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse_entries("").is_err());
        assert!(parse_entries("# only a comment\n").is_err());
    }

    #[test]
    fn bad_amount_errors_with_line() {
        let e = parse_entries("Checking, 100\nSavings, notanumber").unwrap_err();
        assert!(e.contains("line 2"), "got: {e}");
        assert!(e.contains("Savings"), "got: {e}");
    }

    #[test]
    fn missing_amount_errors() {
        let e = parse_entries("Checking").unwrap_err();
        assert!(e.contains("no amount"), "got: {e}");
    }

    #[test]
    fn report_renders_net_worth_and_bars() {
        let input = "Home, 200000, asset, Real Estate\nBrokerage, 50000, asset, Investments\nMortgage, 150000, liability, Real Estate";
        let r = format_report(input, Sort::Value, "$").unwrap();
        assert!(r.contains("Net worth: $100,000.00"), "got: {r}");
        assert!(
            r.contains("Assets — $250,000.00 total across 2 items"),
            "got: {r}"
        );
        assert!(
            r.contains("Liabilities — $150,000.00 total across 1 item"),
            "got: {r}"
        );
        assert!(r.contains("█"), "got: {r}");
        assert!(r.contains("Debt-to-asset ratio: 60.00%"), "got: {r}");
    }

    #[test]
    fn report_no_liabilities() {
        let r = format_report("Cash, 1000, asset, Cash", Sort::Value, "$").unwrap();
        assert!(r.contains("Net worth: $1,000.00"), "got: {r}");
        assert!(
            r.contains("Liabilities — $0.00 total across 0 items"),
            "got: {r}"
        );
        assert!(r.contains("(none)"), "got: {r}");
    }

    #[test]
    fn format_money_groups_thousands() {
        assert_eq!(format_money(1234567_50, "$"), "$1,234,567.50");
        assert_eq!(format_money(-500_00, "$"), "-$500.00");
        assert_eq!(format_money(42_00, "€"), "€42.00");
        assert_eq!(format_money(0, "$"), "$0.00");
    }

    #[test]
    fn blank_currency_prefix() {
        let r = format_report("Cash, 100, asset", Sort::Value, "").unwrap();
        assert!(r.contains("Net worth: 100.00"), "got: {r}");
    }

    #[test]
    fn parse_sort_rejects_bad() {
        assert!(Sort::parse("bogus").is_err());
        assert_eq!(Sort::parse("Label").unwrap(), Sort::Label);
    }
}
