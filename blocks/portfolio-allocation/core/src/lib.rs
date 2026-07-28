//! portfolio-allocation core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps (besides serde for the output struct).
//!
//! Turns a pasted list of holdings into a percentage allocation broken down by a
//! chosen dimension — the holding itself, its asset class, its sector, or the
//! account it sits in. Each slice reports its total value, its share of the
//! portfolio (percent), and how many holdings it covers, ranked largest first —
//! chart-ready for a pie/bar/donut. Deterministic; runs locally on every surface.

use std::collections::HashMap;

use serde::Serialize;

/// Guard rails so a giant paste can't exhaust the wasm sandbox.
const MAX_INPUT_BYTES: usize = 5_000_000;
const MAX_HOLDINGS: usize = 100_000;

/// The dimension to break the portfolio down by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    /// Each holding is its own slice (same-labelled rows are merged).
    Holding,
    /// Group by asset class (column 3), e.g. Stocks / Bonds / Cash.
    Asset,
    /// Group by sector (column 4), e.g. Technology / Healthcare.
    Sector,
    /// Group by account (column 5), e.g. 401(k) / Roth IRA / Taxable.
    Account,
}

impl GroupBy {
    fn parse(s: &str) -> Result<GroupBy, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "holding" | "holdings" | "position" => Ok(GroupBy::Holding),
            "" | "asset" | "asset_class" | "class" => Ok(GroupBy::Asset),
            "sector" | "industry" => Ok(GroupBy::Sector),
            "account" => Ok(GroupBy::Account),
            other => Err(format!(
                "group_by must be 'holding', 'asset', 'sector', or 'account', got '{other}'"
            )),
        }
    }
    fn noun(self) -> &'static str {
        match self {
            GroupBy::Holding => "holding",
            GroupBy::Asset => "asset class",
            GroupBy::Sector => "sector",
            GroupBy::Account => "account",
        }
    }
}

/// How to order the slices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Largest value first (default), ties broken by first-seen order.
    Value,
    /// Alphabetical by label (case-insensitive).
    Label,
}

impl Sort {
    fn parse(s: &str) -> Result<Sort, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "value" | "" => Ok(Sort::Value),
            "label" | "name" | "alpha" | "alphabetical" => Ok(Sort::Label),
            other => Err(format!("sort must be 'value' or 'label', got '{other}'")),
        }
    }
}

/// A parsed holding row.
#[derive(Debug, Clone, PartialEq)]
pub struct Holding {
    pub label: String,
    pub value: f64,
    pub asset: Option<String>,
    pub sector: Option<String>,
    pub account: Option<String>,
}

/// One slice of the allocation — chart-ready (label + value + percent).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Slice {
    /// The group label (holding name, asset class, sector, account, or "Other").
    pub label: String,
    /// Total value of the holdings in this slice.
    pub value: f64,
    /// Share of the portfolio total, 0–100, rounded to 2 decimals.
    pub percent: f64,
    /// How many holdings fall in this slice.
    pub count: usize,
}

/// The full breakdown returned to chat/CLI (as JSON) — chart-ready.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Allocation {
    /// Slices, ranked per the requested sort.
    pub slices: Vec<Slice>,
    /// Sum of every holding's value (the percentage denominator).
    pub total: f64,
    /// What the breakdown is grouped by ("holding" | "asset" | "sector" | "account").
    pub group_by: String,
    /// Number of holdings parsed from the input.
    pub holdings: usize,
    /// Herfindahl–Hirschman concentration index over the (unfolded) group shares,
    /// 0–10000 — the sum of each slice's squared percentage. Higher = more
    /// concentrated. Computed before any `top_n` folding.
    pub hhi: f64,
    /// Plain-language reading of `hhi` ("Well diversified" … "Highly concentrated").
    pub concentration: String,
}

/// Map an HHI score onto a diversification reading (the classic 1500/2500 bands).
fn concentration_label(hhi: f64) -> &'static str {
    if hhi < 1500.0 {
        "Well diversified"
    } else if hhi < 2500.0 {
        "Moderately concentrated"
    } else {
        "Highly concentrated"
    }
}

/// Bucket label used when a holding has no value for the grouping dimension.
const UNSPECIFIED: &str = "(unspecified)";
/// Bucket label for the tail when `top_n` collapses the smaller slices.
const OTHER: &str = "Other";

/// Split one input line into cells. Prefers a tab delimiter (spreadsheet paste)
/// so a thousands-separated value like `1,500` survives; otherwise commas.
fn split_cells(line: &str) -> Vec<String> {
    let delim = if line.contains('\t') { '\t' } else { ',' };
    line.split(delim).map(|c| c.trim().to_string()).collect()
}

/// Parse a value cell into a number. Strips a leading/trailing currency symbol,
/// spaces, and (for tab-delimited input) thousands separators. Supports a
/// `shares @ price` or `shares x price` form, e.g. `10 @ 150` = 1500.
fn parse_value(cell: &str) -> Result<f64, String> {
    let cell = cell.trim();
    if cell.is_empty() {
        return Err("missing value".to_string());
    }
    // shares @ price  /  shares x price  /  shares * price
    for sep in ['@', 'x', 'X', '*'] {
        if let Some((a, b)) = cell.split_once(sep) {
            let shares = clean_number(a)?;
            let price = clean_number(b)?;
            return Ok(shares * price);
        }
    }
    clean_number(cell)
}

/// Strip currency/grouping ornamentation from a numeric string and parse it.
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
        return Err("missing value".to_string());
    }
    let n: f64 = body
        .parse()
        .map_err(|_| format!("expected a number, got '{}'", s.trim()))?;
    if !n.is_finite() {
        return Err("value must be a finite number".to_string());
    }
    Ok(if neg { -n } else { n })
}

/// Does this cell look like a numeric value (used to detect a header row)?
fn looks_numeric(cell: &str) -> bool {
    parse_value(cell).is_ok()
}

/// Parse the pasted holdings block. Blank lines and `#` comments are skipped; a
/// leading header row (whose value column isn't a number) is dropped.
pub fn parse_holdings(input: &str) -> Result<Vec<Holding>, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is too large ({} bytes; max {})",
            input.len(),
            MAX_INPUT_BYTES
        ));
    }
    let mut holdings = Vec::new();
    let mut seen_data = false;
    for (i, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cells = split_cells(line);
        // A header row: first data-ish line whose value cell isn't numeric.
        if !seen_data && cells.len() >= 2 && !looks_numeric(&cells[1]) {
            continue;
        }
        let label = cells.first().map(|s| s.as_str()).unwrap_or("").trim();
        if label.is_empty() {
            return Err(format!("line {}: missing label (holding name)", i + 1));
        }
        if cells.len() < 2 {
            return Err(format!(
                "line {}: '{}' has no value — enter 'label, value[, asset, sector, account]'",
                i + 1,
                label
            ));
        }
        let value = parse_value(&cells[1])
            .map_err(|e| format!("line {}: value for '{}' — {}", i + 1, label, e))?;
        let tag = |idx: usize| -> Option<String> {
            cells
                .get(idx)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        };
        holdings.push(Holding {
            label: label.to_string(),
            value,
            asset: tag(2),
            sector: tag(3),
            account: tag(4),
        });
        seen_data = true;
        if holdings.len() > MAX_HOLDINGS {
            return Err(format!("too many holdings (max {})", MAX_HOLDINGS));
        }
    }
    if holdings.is_empty() {
        return Err(
            "no holdings found — enter at least one line as 'label, value[, asset, sector, account]'"
                .to_string(),
        );
    }
    Ok(holdings)
}

/// The grouping key for one holding under the chosen dimension.
fn key_for(h: &Holding, by: GroupBy) -> String {
    match by {
        GroupBy::Holding => h.label.clone(),
        GroupBy::Asset => h.asset.clone().unwrap_or_else(|| UNSPECIFIED.to_string()),
        GroupBy::Sector => h.sector.clone().unwrap_or_else(|| UNSPECIFIED.to_string()),
        GroupBy::Account => h.account.clone().unwrap_or_else(|| UNSPECIFIED.to_string()),
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Compute the allocation breakdown. `top_n` (0 = all) keeps the largest N slices
/// and folds the rest into an "Other" bucket (only after value-sorting).
pub fn allocate(
    holdings: &[Holding],
    by: GroupBy,
    sort: Sort,
    top_n: usize,
) -> Result<Allocation, String> {
    // Aggregate value + count per group, preserving first-seen order for stable ties.
    let mut totals: HashMap<String, (f64, usize)> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut grand = 0.0f64;
    for h in holdings {
        let k = key_for(h, by);
        let e = totals.entry(k.clone()).or_insert_with(|| {
            order.push(k.clone());
            (0.0, 0)
        });
        e.0 += h.value;
        e.1 += 1;
        grand += h.value;
    }

    // Concentration (HHI) over the true group shares, before any top_n folding.
    let hhi = if grand == 0.0 {
        0.0
    } else {
        totals
            .values()
            .map(|(v, _)| {
                let share = v / grand * 100.0;
                share * share
            })
            .sum::<f64>()
            .round()
    };

    // Rank: always value-desc first so `top_n` keeps the biggest slices; a
    // label sort is then a stable re-order of the retained slices.
    let mut ranked: Vec<(usize, String)> = order.iter().cloned().enumerate().collect();
    ranked.sort_by(|a, b| {
        totals[&b.1]
            .0
            .partial_cmp(&totals[&a.1].0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });

    // Fold the tail into "Other" when top_n>0 and there are more groups than that.
    let mut slices: Vec<Slice> = Vec::new();
    let percent = |v: f64| {
        if grand == 0.0 {
            0.0
        } else {
            round2(v / grand * 100.0)
        }
    };
    if top_n > 0 && ranked.len() > top_n {
        for (_, k) in ranked.iter().take(top_n) {
            let (v, c) = totals[k];
            slices.push(Slice {
                label: k.clone(),
                value: v,
                percent: percent(v),
                count: c,
            });
        }
        let (mut ov, mut oc) = (0.0, 0usize);
        for (_, k) in ranked.iter().skip(top_n) {
            let (v, c) = totals[k];
            ov += v;
            oc += c;
        }
        slices.push(Slice {
            label: OTHER.to_string(),
            value: ov,
            percent: percent(ov),
            count: oc,
        });
    } else {
        for (_, k) in &ranked {
            let (v, c) = totals[k];
            slices.push(Slice {
                label: k.clone(),
                value: v,
                percent: percent(v),
                count: c,
            });
        }
    }

    // A label sort re-orders the (possibly Other-folded) slices; "Other" stays last.
    if sort == Sort::Label {
        slices.sort_by(|a, b| {
            let ao = a.label == OTHER;
            let bo = b.label == OTHER;
            ao.cmp(&bo)
                .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
        });
    }

    Ok(Allocation {
        slices,
        total: round2(grand),
        group_by: by.noun_key().to_string(),
        holdings: holdings.len(),
        hhi,
        concentration: concentration_label(hhi).to_string(),
    })
}

impl GroupBy {
    /// The canonical short key echoed in the JSON output.
    fn noun_key(self) -> &'static str {
        match self {
            GroupBy::Holding => "holding",
            GroupBy::Asset => "asset",
            GroupBy::Sector => "sector",
            GroupBy::Account => "account",
        }
    }
}

/// Format a value as `<sym>1,234.56` (accounting-negative `-<sym>1,234.56`).
fn format_money(v: f64, sym: &str) -> String {
    let neg = v < 0.0;
    let abs = v.abs();
    let whole = abs.trunc() as i64;
    let cents = ((abs - whole as f64) * 100.0).round() as i64;
    // Rounding can carry into the whole part (e.g. 0.999 → 1.00).
    let (whole, cents) = if cents == 100 {
        (whole + 1, 0)
    } else {
        (whole, cents)
    };
    let mut digits = whole.to_string();
    let mut grouped = String::new();
    while digits.len() > 3 {
        let split = digits.len() - 3;
        grouped = format!(",{}{}", &digits[split..], grouped);
        digits.truncate(split);
    }
    let grouped = format!("{}{}", digits, grouped);
    format!(
        "{}{}{}.{:02}",
        if neg { "-" } else { "" },
        sym,
        grouped,
        cents
    )
}

/// Plain-text allocation table with proportional bars (the page surface).
pub fn format_table(
    input: &str,
    group_by: GroupBy,
    sort: Sort,
    top_n: usize,
    currency: &str,
) -> Result<String, String> {
    let holdings = parse_holdings(input)?;
    let alloc = allocate(&holdings, group_by, sort, top_n)?;
    let sym = if currency.trim().is_empty() {
        ""
    } else {
        currency.trim()
    };

    let label_w = alloc
        .slices
        .iter()
        .map(|s| s.label.chars().count())
        .max()
        .unwrap_or(1);
    let money_w = alloc
        .slices
        .iter()
        .map(|s| format_money(s.value, sym).chars().count())
        .max()
        .unwrap_or(1);

    const BAR: usize = 24;
    let mut lines = Vec::with_capacity(alloc.slices.len() + 2);
    lines.push(format!(
        "Allocation by {} — {} total across {} holding{}",
        group_by.noun(),
        format_money(alloc.total, sym),
        alloc.holdings,
        if alloc.holdings == 1 { "" } else { "s" }
    ));
    lines.push(String::new());
    for s in &alloc.slices {
        let filled = ((s.percent / 100.0) * BAR as f64).round() as usize;
        let filled = filled.min(BAR);
        let bar: String = "█".repeat(filled);
        let pad: String = "·".repeat(BAR - filled);
        lines.push(format!(
            "{:label_w$}  {:>money_w$}  {:>6.2}%  {}{}  ({} holding{})",
            s.label,
            format_money(s.value, sym),
            s.percent,
            bar,
            pad,
            s.count,
            if s.count == 1 { "" } else { "s" },
            label_w = label_w,
            money_w = money_w,
        ));
    }
    lines.push(String::new());
    lines.push(format!(
        "Concentration (HHI): {} — {}",
        alloc.hhi as i64, alloc.concentration
    ));
    Ok(lines.join("\n"))
}

/// The web/CLI/chat entry point kept for the scaffold's default signature — the
/// real surfaces call `format_table` (page) or `allocate` (structured).
pub fn run(input: &str) -> Result<String, String> {
    format_table(input, GroupBy::Asset, Sort::Value, 0, "$")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Vec<Holding> {
        parse_holdings(s).unwrap()
    }

    #[test]
    fn basic_asset_breakdown() {
        let input = "AAPL, 6000, Stocks\nBND, 3000, Bonds\nCash, 1000, Cash";
        let h = parse(input);
        let a = allocate(&h, GroupBy::Asset, Sort::Value, 0).unwrap();
        assert_eq!(a.total, 10000.0);
        assert_eq!(a.slices[0].label, "Stocks");
        assert_eq!(a.slices[0].percent, 60.0);
        assert_eq!(a.slices[1].label, "Bonds");
        assert_eq!(a.slices[1].percent, 30.0);
        assert_eq!(a.slices[2].percent, 10.0);
    }

    #[test]
    fn groups_merge_by_asset() {
        let input = "AAPL, 4000, Stocks\nMSFT, 2000, Stocks\nBND, 4000, Bonds";
        let h = parse(input);
        let a = allocate(&h, GroupBy::Asset, Sort::Value, 0).unwrap();
        // Stocks and Bonds tie at 4000 each after merge — Stocks seen first.
        assert_eq!(a.slices.len(), 2);
        let stocks = a.slices.iter().find(|s| s.label == "Stocks").unwrap();
        assert_eq!(stocks.value, 6000.0);
        assert_eq!(stocks.count, 2);
        assert_eq!(stocks.percent, 60.0);
    }

    #[test]
    fn group_by_holding_merges_same_label() {
        let input = "AAPL, 100\nAAPL, 300\nMSFT, 100";
        let h = parse(input);
        let a = allocate(&h, GroupBy::Holding, Sort::Value, 0).unwrap();
        assert_eq!(a.slices[0].label, "AAPL");
        assert_eq!(a.slices[0].value, 400.0);
        assert_eq!(a.slices[0].count, 2);
    }

    #[test]
    fn missing_tag_goes_to_unspecified() {
        let input = "AAPL, 100, Stocks\nGOLD, 100";
        let h = parse(input);
        let a = allocate(&h, GroupBy::Asset, Sort::Value, 0).unwrap();
        assert!(a.slices.iter().any(|s| s.label == "(unspecified)"));
    }

    #[test]
    fn sector_and_account_dimensions() {
        let input = "AAPL, 100, Stocks, Tech, Roth\nJNJ, 300, Stocks, Health, 401k";
        let h = parse(input);
        let by_sector = allocate(&h, GroupBy::Sector, Sort::Value, 0).unwrap();
        assert_eq!(by_sector.slices[0].label, "Health");
        let by_acct = allocate(&h, GroupBy::Account, Sort::Value, 0).unwrap();
        assert_eq!(by_acct.slices[0].label, "401k");
    }

    #[test]
    fn top_n_folds_other() {
        let input = "A, 50\nB, 30\nC, 15\nD, 5";
        let h = parse(input);
        let a = allocate(&h, GroupBy::Holding, Sort::Value, 2).unwrap();
        assert_eq!(a.slices.len(), 3);
        assert_eq!(a.slices[2].label, "Other");
        assert_eq!(a.slices[2].value, 20.0);
        assert_eq!(a.slices[2].count, 2);
        assert_eq!(a.slices[2].percent, 20.0);
    }

    #[test]
    fn top_n_larger_than_groups_is_noop() {
        let input = "A, 50\nB, 50";
        let h = parse(input);
        let a = allocate(&h, GroupBy::Holding, Sort::Value, 5).unwrap();
        assert_eq!(a.slices.len(), 2);
        assert!(a.slices.iter().all(|s| s.label != "Other"));
    }

    #[test]
    fn label_sort_keeps_other_last() {
        let input = "Zeta, 50\nAlpha, 30\nMid, 15\nBeta, 5";
        let h = parse(input);
        let a = allocate(&h, GroupBy::Holding, Sort::Label, 2).unwrap();
        // Top-2 by value = Zeta, Alpha; rest → Other. Label sort: Alpha, Zeta, Other.
        assert_eq!(a.slices[0].label, "Alpha");
        assert_eq!(a.slices[1].label, "Zeta");
        assert_eq!(a.slices[2].label, "Other");
    }

    #[test]
    fn shares_at_price() {
        let h = parse("AAPL, 10 @ 150");
        assert_eq!(h[0].value, 1500.0);
        let h2 = parse("MSFT, 4 x 25");
        assert_eq!(h2[0].value, 100.0);
    }

    #[test]
    fn currency_and_thousands_stripped() {
        // Comma is the delimiter, so thousands separators need a tab-delimited row.
        let h = parse("Fund\t$1,234.50\tStocks");
        assert_eq!(h[0].value, 1234.5);
        assert_eq!(h[0].asset.as_deref(), Some("Stocks"));
    }

    #[test]
    fn accounting_negative() {
        let h = parse("Margin, (500)");
        assert_eq!(h[0].value, -500.0);
    }

    #[test]
    fn header_row_dropped() {
        let input = "Name, Value, Asset\nAAPL, 100, Stocks";
        let h = parse(input);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].label, "AAPL");
    }

    #[test]
    fn comments_and_blanks_skipped() {
        let input = "# my portfolio\nAAPL, 100\n\nMSFT, 100\n";
        let h = parse(input);
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn percentages_round_to_two() {
        let input = "A, 1\nB, 1\nC, 1";
        let h = parse(input);
        let a = allocate(&h, GroupBy::Holding, Sort::Value, 0).unwrap();
        assert_eq!(a.slices[0].percent, 33.33);
    }

    #[test]
    fn zero_total_no_divide_by_zero() {
        let input = "A, 0\nB, 0";
        let h = parse(input);
        let a = allocate(&h, GroupBy::Holding, Sort::Value, 0).unwrap();
        assert_eq!(a.total, 0.0);
        assert_eq!(a.slices[0].percent, 0.0);
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse_holdings("").is_err());
        assert!(parse_holdings("# only a comment\n").is_err());
    }

    #[test]
    fn bad_value_errors_with_line() {
        let e = parse_holdings("AAPL, 100\nMSFT, notanumber").unwrap_err();
        assert!(e.contains("line 2"), "got: {e}");
        assert!(e.contains("MSFT"), "got: {e}");
    }

    #[test]
    fn missing_value_errors() {
        let e = parse_holdings("AAPL").unwrap_err();
        assert!(e.contains("no value"), "got: {e}");
    }

    #[test]
    fn table_renders_with_bar_and_money() {
        let t = format_table(
            "AAPL, 6000, Stocks\nBND, 4000, Bonds",
            GroupBy::Asset,
            Sort::Value,
            0,
            "$",
        )
        .unwrap();
        assert!(t.contains("$10,000.00 total"));
        assert!(t.contains("60.00%"));
        assert!(t.contains("█"));
        assert!(t.contains("Stocks"));
    }

    #[test]
    fn hhi_and_concentration() {
        // Two equal slices → HHI = 50² + 50² = 5000 → highly concentrated.
        let a = allocate(&parse("A, 50\nB, 50"), GroupBy::Holding, Sort::Value, 0).unwrap();
        assert_eq!(a.hhi, 5000.0);
        assert_eq!(a.concentration, "Highly concentrated");
        // Ten equal slices → HHI = 10 × 10² = 1000 → well diversified.
        let input: String = (0..10).map(|i| format!("H{i}, 10\n")).collect();
        let d = allocate(&parse(&input), GroupBy::Holding, Sort::Value, 0).unwrap();
        assert_eq!(d.hhi, 1000.0);
        assert_eq!(d.concentration, "Well diversified");
    }

    #[test]
    fn hhi_ignores_top_n_folding() {
        // HHI is over the true group shares, not the Other-folded view.
        let a = allocate(&parse("A, 50\nB, 50"), GroupBy::Holding, Sort::Value, 1).unwrap();
        assert_eq!(a.hhi, 5000.0);
    }

    #[test]
    fn format_money_groups_thousands() {
        assert_eq!(format_money(1234567.5, "$"), "$1,234,567.50");
        assert_eq!(format_money(-500.0, "$"), "-$500.00");
        assert_eq!(format_money(0.999, "$"), "$1.00");
        assert_eq!(format_money(42.0, "€"), "€42.00");
    }

    #[test]
    fn parse_rejects_group_by() {
        assert!(GroupBy::parse("bogus").is_err());
        assert_eq!(GroupBy::parse("Asset").unwrap(), GroupBy::Asset);
    }
}

/// Re-export the string parsers for the block/web wrappers.
pub fn parse_group_by(s: &str) -> Result<GroupBy, String> {
    GroupBy::parse(s)
}
pub fn parse_sort(s: &str) -> Result<Sort, String> {
    Sort::parse(s)
}
