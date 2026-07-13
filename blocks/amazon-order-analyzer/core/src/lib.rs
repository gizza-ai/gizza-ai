//! amazon-order-analyzer core — pure, no wafer/wasm-bindgen deps.
//!
//! Parses an Amazon order-history CSV export and produces a spending SUMMARY:
//! total spend, item/order counts and date range, a **spend-by-month** table,
//! the **top items** by spend, and a **category breakdown** (when the export
//! carries a Category column). Handles both common Amazon export shapes:
//!   * the classic **Order History Report** (`Order Date`, `Title`, `Category`,
//!     `Item Total`, `Quantity`, `Order ID`, …), and
//!   * the newer **Request My Data / Retail.OrderHistory** export (`Order Date`,
//!     `Product Name`, `Total Owed`, `Quantity`, `Order ID`, …).
//!
//! Output is either a Markdown summary (default) or a structured JSON object.
//! No system clock, no I/O — instantiates under both wafer (wasm32-wasip1) and
//! wasm-pack (wasm32-unknown-unknown).

use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Hard cap on how many "top items" can be requested.
pub const MAX_TOP: u32 = 50;
const DEFAULT_TOP: u32 = 10;

// ---------------------------------------------------------------------------
// Output shape enum.
// ---------------------------------------------------------------------------

/// How to render the analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Summary,
    Json,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "summary" | "markdown" | "md" => Output::Summary,
            "json" => Output::Json,
            other => return Err(format!("unknown output '{other}' (use summary or json)")),
        })
    }
}

// ---------------------------------------------------------------------------
// Column detection + field parsing.
// ---------------------------------------------------------------------------

fn norm(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

/// First header (by the priority order of `names`) present in `headers`.
fn find_col(headers: &[String], names: &[&str]) -> Option<usize> {
    for name in names {
        if let Some(i) = headers.iter().position(|h| norm(h) == *name) {
            return Some(i);
        }
    }
    None
}

/// Parse a money field: strip a leading currency symbol, thousands separators
/// and stray letters (e.g. `"USD"`), returning the amount and the symbol seen.
/// US-style grouping is assumed (`1,234.56`). Returns `None` for blank / "N/A".
fn parse_money(s: &str) -> (Option<f64>, Option<char>) {
    let t = s.trim();
    if t.is_empty() {
        return (None, None);
    }
    let mut cleaned = String::new();
    let mut sym = None;
    let mut negative = false;
    for c in t.chars() {
        match c {
            '0'..='9' | '.' => cleaned.push(c),
            '-' => negative = true,
            '(' => negative = true, // accounting-style negatives: (12.34)
            '$' | '£' | '€' | '¥' | '₹' => {
                if sym.is_none() {
                    sym = Some(c);
                }
            }
            _ => {} // commas, spaces, NBSP, letters like USD → dropped
        }
    }
    match cleaned.parse::<f64>() {
        Ok(v) if v.is_finite() => (Some(if negative { -v } else { v }), sym),
        _ => (None, sym),
    }
}

/// Parse a quantity field; defaults to 1 and floors at 1.
fn parse_qty(s: &str) -> i64 {
    s.trim().parse::<i64>().ok().filter(|&q| q >= 1).unwrap_or(1)
}

/// Parse an Amazon date field into `(year, month)`. Handles ISO
/// (`2024-01-15`, `2024-01-15T08:12:00Z`, `2024-01-15 08:12`), US slash dates
/// (`01/15/2024`, `1/15/24`), and long/abbrev month names (`January 15, 2024`).
fn parse_month(s: &str) -> Option<(i32, u32)> {
    let t = s.trim();
    if t.len() < 6 {
        return None;
    }
    let b = t.as_bytes();
    // Leading ISO date: YYYY-MM-DD…
    if b.len() >= 7 && b[4] == b'-' {
        if let (Ok(y), Ok(m)) = (t[0..4].parse::<i32>(), t[5..7].parse::<u32>()) {
            if (1..=12).contains(&m) && (1900..=2200).contains(&y) {
                return Some((y, m));
            }
        }
    }
    // US slash dates — take the date token before any time-of-day.
    let date_only = t.split_whitespace().next().unwrap_or(t);
    for fmt in ["%m/%d/%Y", "%m/%d/%y"] {
        if let Ok(d) = NaiveDate::parse_from_str(date_only, fmt) {
            return Some((d.year(), d.month()));
        }
    }
    // Named-month dates use the whole string.
    for fmt in ["%B %d, %Y", "%b %d, %Y", "%d %B %Y", "%d %b %Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(t, fmt) {
            return Some((d.year(), d.month()));
        }
    }
    None
}

fn month_key(ym: (i32, u32)) -> String {
    format!("{:04}-{:02}", ym.0, ym.1)
}

/// Format money with US-style thousands grouping and 2 decimals.
fn fmt_money(sym: char, v: f64) -> String {
    let neg = v < 0.0;
    let cents = (v.abs() * 100.0).round() as u64;
    let whole = cents / 100;
    let frac = cents % 100;
    // Group the integer part with commas.
    let ws = whole.to_string();
    let mut grouped = String::new();
    let bytes = ws.as_bytes();
    for (i, ch) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*ch as char);
    }
    format!("{}{}{}.{:02}", if neg { "-" } else { "" }, sym, grouped, frac)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

// ---------------------------------------------------------------------------
// Aggregation result.
// ---------------------------------------------------------------------------

struct Analysis {
    currency: char,
    total: f64,
    items: usize,
    orders: usize,
    have_orders: bool,
    rows_skipped: usize,
    by_month: Vec<(String, usize, f64)>,       // key, items, spend (sorted)
    top_items: Vec<(String, i64, f64)>,        // title, qty, spend
    categories: Option<Vec<(String, usize, f64)>>, // name, items, spend (sorted by spend)
}

/// Analyze `csv` order-history text and render a summary (default) or JSON.
///
/// - `output`: `summary` (default, Markdown) | `json`.
/// - `top`: how many top items to list (0 → default 10; capped at MAX_TOP).
pub fn analyze(csv: &str, output: &str, top: u32) -> Result<String, String> {
    // Strip a UTF-8 BOM the "Request My Data" export sometimes prepends, so the
    // first header ("Order Date") is detected.
    let csv = csv.strip_prefix('\u{feff}').unwrap_or(csv);
    if csv.trim().is_empty() {
        return Err("input is empty — paste your Amazon order-history CSV".into());
    }
    let out = Output::parse(output)?;
    let top = if top == 0 {
        DEFAULT_TOP
    } else {
        top.clamp(1, MAX_TOP)
    } as usize;

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found in the CSV".into());
    }

    let headers: Vec<String> = records[0].iter().map(|s| s.to_string()).collect();
    let rows = &records[1..];

    // Locate columns (case-insensitive, priority order).
    let amount_idx = find_col(
        &headers,
        &[
            "item total",
            "total owed",
            "item subtotal",
            "total charged",
            "subtotal",
            "purchase price per unit",
        ],
    )
    .ok_or_else(|| {
        "couldn't find an amount column (e.g. \"Item Total\" or \"Total Owed\") — \
         is this an Amazon order-history CSV export?"
            .to_string()
    })?;
    let per_unit = norm(&headers[amount_idx]) == "purchase price per unit";
    let date_idx = find_col(
        &headers,
        &["order date", "order date/time", "date", "shipment date", "ship date"],
    );
    let title_idx = find_col(&headers, &["title", "product name", "item name", "name"]);
    let category_idx = find_col(&headers, &["category"]);
    let qty_idx = find_col(&headers, &["quantity", "qty"]);
    let order_idx = find_col(&headers, &["order id"]);

    let get = |r: &csv::StringRecord, i: usize| r.get(i).unwrap_or("").to_string();

    let mut currency: Option<char> = None;
    let mut total = 0.0f64;
    let mut items = 0usize;
    let mut rows_skipped = 0usize;

    let mut month_map: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    // Top items: preserve first-seen order for stable ties.
    let mut item_idx: HashMap<String, usize> = HashMap::new();
    let mut item_vec: Vec<(String, i64, f64)> = Vec::new();
    let mut cat_idx: HashMap<String, usize> = HashMap::new();
    let mut cat_vec: Vec<(String, usize, f64)> = Vec::new();
    let mut order_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for r in rows {
        // Skip fully-blank lines the CSV reader may surface.
        if r.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        let (amt, sym) = parse_money(&get(r, amount_idx));
        if currency.is_none() {
            currency = sym;
        }
        let qty = qty_idx.map(|i| parse_qty(&get(r, i))).unwrap_or(1);
        let spend = match amt {
            Some(v) => {
                if per_unit {
                    v * qty as f64
                } else {
                    v
                }
            }
            None => {
                rows_skipped += 1;
                continue;
            }
        };

        items += 1;
        total += spend;

        if let Some(oi) = order_idx {
            let id = get(r, oi);
            if !id.trim().is_empty() {
                order_ids.insert(id.trim().to_string());
            }
        }

        if let Some(di) = date_idx {
            if let Some(ym) = parse_month(&get(r, di)) {
                let e = month_map.entry(month_key(ym)).or_insert((0, 0.0));
                e.0 += 1;
                e.1 += spend;
            }
        }

        let title = title_idx
            .map(|i| get(r, i).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "(untitled item)".to_string());
        match item_idx.get(&title) {
            Some(&i) => {
                item_vec[i].1 += qty;
                item_vec[i].2 += spend;
            }
            None => {
                item_idx.insert(title.clone(), item_vec.len());
                item_vec.push((title, qty, spend));
            }
        }

        if category_idx.is_some() {
            let cat = category_idx
                .map(|i| get(r, i).trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "(uncategorized)".to_string());
            match cat_idx.get(&cat) {
                Some(&i) => {
                    cat_vec[i].1 += 1;
                    cat_vec[i].2 += spend;
                }
                None => {
                    cat_idx.insert(cat.clone(), cat_vec.len());
                    cat_vec.push((cat, 1, spend));
                }
            }
        }
    }

    if items == 0 {
        return Err(
            "found no rows with a parseable amount — check that the amount column has values"
                .into(),
        );
    }

    let currency = currency.unwrap_or('$');

    // Rank top items by spend (desc); stable sort keeps first-seen ties.
    item_vec.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    item_vec.truncate(top);

    let categories = if category_idx.is_some() {
        cat_vec.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        Some(cat_vec)
    } else {
        None
    };

    let by_month: Vec<(String, usize, f64)> = month_map
        .into_iter()
        .map(|(k, (n, s))| (k, n, s))
        .collect();

    let orders = if order_idx.is_some() && !order_ids.is_empty() {
        order_ids.len()
    } else {
        items
    };

    let analysis = Analysis {
        currency,
        total,
        items,
        orders,
        have_orders: order_idx.is_some() && !order_ids.is_empty(),
        rows_skipped,
        by_month,
        top_items: item_vec,
        categories,
    };

    match out {
        Output::Summary => Ok(render_summary(&analysis)),
        Output::Json => render_json(&analysis),
    }
}

// ---------------------------------------------------------------------------
// Rendering.
// ---------------------------------------------------------------------------

fn md_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|").replace('\n', " ")
}

fn render_summary(a: &Analysis) -> String {
    let mut out = String::new();
    let sym = a.currency;

    // Caption.
    let _ = write!(
        out,
        "{} across {} item{}",
        fmt_money(sym, a.total),
        a.items,
        if a.items == 1 { "" } else { "s" }
    );
    if a.have_orders {
        let _ = write!(
            out,
            " in {} order{}",
            a.orders,
            if a.orders == 1 { "" } else { "s" }
        );
    }
    if let (Some(first), Some(last)) = (a.by_month.first(), a.by_month.last()) {
        if first.0 == last.0 {
            let _ = write!(out, " · {}", first.0);
        } else {
            let _ = write!(out, " · {} → {}", first.0, last.0);
        }
    } else {
        out.push_str(" · dates unavailable");
    }

    // Spend by month.
    out.push_str("\n\n## Spend by month\n\n");
    if a.by_month.is_empty() {
        out.push_str("No recognizable order dates found — monthly breakdown unavailable.\n");
    } else {
        let peak = a
            .by_month
            .iter()
            .map(|(_, _, s)| *s)
            .fold(0.0f64, f64::max)
            .max(0.01);
        out.push_str("| month | items | spend | |\n| --- | --- | --- | --- |\n");
        for (m, n, s) in &a.by_month {
            let bars = ((s / peak) * 20.0).round() as usize;
            let bar = "▇".repeat(bars.max(1));
            let _ = writeln!(out, "| {} | {} | {} | {} |", m, n, fmt_money(sym, *s), bar);
        }
    }

    // Top items.
    let _ = write!(out, "\n## Top items ({})\n\n", a.top_items.len());
    out.push_str("| spend | qty | item |\n| --- | --- | --- |\n");
    for (title, qty, spend) in &a.top_items {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            fmt_money(sym, *spend),
            qty,
            md_escape(title)
        );
    }

    // Categories.
    if let Some(cats) = &a.categories {
        out.push_str("\n## Categories\n\n");
        out.push_str("| category | items | spend | share |\n| --- | --- | --- | --- |\n");
        for (name, n, spend) in cats {
            let pct = if a.total != 0.0 {
                100.0 * spend / a.total
            } else {
                0.0
            };
            let _ = writeln!(
                out,
                "| {} | {} | {} | {:.1}% |",
                md_escape(name),
                n,
                fmt_money(sym, *spend),
                pct
            );
        }
    } else {
        out.push_str(
            "\n_No Category column was found in this export, so the category breakdown is omitted._\n",
        );
    }

    if a.rows_skipped > 0 {
        let _ = write!(
            out,
            "\n_{} row{} skipped (no parseable amount)._",
            a.rows_skipped,
            if a.rows_skipped == 1 { "" } else { "s" }
        );
    }

    out.trim_end().to_string()
}

fn render_json(a: &Analysis) -> Result<String, String> {
    let by_month: Vec<serde_json::Value> = a
        .by_month
        .iter()
        .map(|(m, n, s)| serde_json::json!({ "month": m, "items": n, "spend": round2(*s) }))
        .collect();

    let top_items: Vec<serde_json::Value> = a
        .top_items
        .iter()
        .map(|(t, q, s)| {
            serde_json::json!({ "item": t, "quantity": q, "spend": round2(*s) })
        })
        .collect();

    let categories: serde_json::Value = match &a.categories {
        Some(cats) => serde_json::Value::Array(
            cats.iter()
                .map(|(name, n, s)| {
                    let share = if a.total != 0.0 { s / a.total } else { 0.0 };
                    serde_json::json!({
                        "category": name,
                        "items": n,
                        "spend": round2(*s),
                        "share": round2(share * 100.0) / 100.0,
                    })
                })
                .collect(),
        ),
        None => serde_json::Value::Null,
    };

    let obj = serde_json::json!({
        "currency": a.currency.to_string(),
        "total_spend": round2(a.total),
        "items": a.items,
        "orders": if a.have_orders { serde_json::Value::from(a.orders) } else { serde_json::Value::Null },
        "first_month": a.by_month.first().map(|(m, _, _)| m.clone()),
        "last_month": a.by_month.last().map(|(m, _, _)| m.clone()),
        "rows_skipped": a.rows_skipped,
        "by_month": by_month,
        "top_items": top_items,
        "categories": categories,
    });

    serde_json::to_string_pretty(&obj).map_err(|e| format!("failed to serialize JSON: {e}"))
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Classic Order History Report shape (with Category + Order ID).
    const CLASSIC: &str = "\
Order Date,Order ID,Title,Category,Quantity,Item Total
01/05/2024,111-0000001,USB-C Cable 6ft,Electronics,2,$15.98
01/20/2024,111-0000002,\"The Rust Programming Language, 2nd Ed\",Books,1,$39.99
02/03/2024,111-0000003,AA Batteries 24-pack,Electronics,1,$12.49
02/03/2024,111-0000003,Kitchen Sponges,Home,3,$6.00";

    // Request My Data / Retail.OrderHistory shape (no Category; Total Owed).
    const RMD: &str = "\
Order Date,Order ID,Product Name,Quantity,Total Owed
2024-03-01T08:12:00Z,222-0000001,Coffee Beans 2lb,1,USD 24.50
2024-03-15T10:00:00Z,222-0000002,Notebook A5,2,12.00";

    #[test]
    fn classic_summary_has_totals_months_and_categories() {
        let out = analyze(CLASSIC, "summary", 0).unwrap();
        // Caption: $74.46 across 4 items in 3 orders · 2024-01 → 2024-02
        assert!(
            out.starts_with("$74.46 across 4 items in 3 orders · 2024-01 → 2024-02"),
            "{out}"
        );
        assert!(out.contains("| 2024-01 | 2 | $55.97 |"), "{out}");
        assert!(out.contains("| 2024-02 | 2 | $18.49 |"), "{out}");
        // Top item by spend is the $39.99 book.
        assert!(out.contains("| $39.99 | 1 | The Rust Programming Language, 2nd Ed |"), "{out}");
        // Category breakdown present.
        assert!(out.contains("## Categories"), "{out}");
        assert!(out.contains("| Electronics | 2 | $28.47 |"), "{out}");
    }

    #[test]
    fn request_my_data_shape_no_category() {
        let out = analyze(RMD, "summary", 0).unwrap();
        assert!(out.starts_with("$36.50 across 2 items in 2 orders"), "{out}");
        // No Category column → no Categories section, but an explicit note.
        assert!(!out.contains("## Categories"), "{out}");
        assert!(out.contains("No Category column was found"), "{out}");
        // Product Name column used for items; currency symbol defaults to $.
        assert!(out.contains("Coffee Beans 2lb"), "{out}");
    }

    #[test]
    fn json_output_is_structured() {
        let out = analyze(CLASSIC, "json", 5).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["currency"], "$");
        assert_eq!(v["total_spend"], 74.46);
        assert_eq!(v["items"], 4);
        assert_eq!(v["orders"], 3);
        assert_eq!(v["first_month"], "2024-01");
        assert_eq!(v["last_month"], "2024-02");
        assert_eq!(v["by_month"][0]["month"], "2024-01");
        assert_eq!(v["by_month"][0]["spend"], 55.97);
        assert_eq!(v["top_items"][0]["item"], "The Rust Programming Language, 2nd Ed");
        assert_eq!(v["top_items"][0]["spend"], 39.99);
        // Categories are ranked by spend: Books ($39.99) > Electronics ($28.47).
        assert_eq!(v["categories"][0]["category"], "Books");
        assert_eq!(v["categories"][1]["category"], "Electronics");
    }

    #[test]
    fn top_limit_caps_items() {
        let out = analyze(CLASSIC, "json", 1).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["top_items"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn quantity_multiplies_per_unit_column() {
        let csv = "\
Order Date,Title,Quantity,Purchase Price Per Unit
2024-04-01,Widget,3,$2.00";
        let out = analyze(csv, "json", 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total_spend"], 6.00);
    }

    #[test]
    fn empty_input_errors() {
        let err = analyze("   \n ", "summary", 0).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn missing_amount_column_errors() {
        let err = analyze("Order Date,Title\n2024-01-01,Thing", "summary", 0).unwrap_err();
        assert!(err.contains("amount column"), "{err}");
    }

    #[test]
    fn bad_output_errors() {
        let err = analyze(CLASSIC, "table", 0).unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }

    #[test]
    fn rows_without_amount_are_skipped_not_dropped_silently() {
        let csv = "\
Order Date,Title,Item Total
2024-05-01,Real,$10.00
2024-05-02,Blank,";
        let out = analyze(csv, "summary", 0).unwrap();
        assert!(out.contains("1 row skipped"), "{out}");
        assert!(out.starts_with("$10.00 across 1 item"), "{out}");
    }
}
