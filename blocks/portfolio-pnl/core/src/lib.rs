//! portfolio-pnl core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps.
//!
//! Turns a pasted list of positions (`label, quantity, entry price, current
//! price`) into a profit/loss table: per-position cost basis, market value,
//! gross and net P/L, return percent and break-even price, plus portfolio
//! totals, an optional tax estimate, and best/worst winners and losers.
//! Long and short positions are both supported. Deterministic, offline, and
//! identical on every surface — no prices are fetched, every number comes from
//! the input.

/// Guard rails so a giant paste can't exhaust the wasm sandbox.
const MAX_INPUT_BYTES: usize = 5_000_000;
/// Hard cap on parsed positions (the page states this limit).
pub const MAX_POSITIONS: usize = 5_000;

/// Which direction a position is held in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Bought first — gains when the price rises.
    Long,
    /// Sold first — gains when the price falls.
    Short,
}

impl Side {
    /// Parse the `side` parameter (empty = long, the default).
    pub fn parse(s: &str) -> Result<Side, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "long" | "l" | "buy" => Ok(Side::Long),
            "short" | "s" | "sell" => Ok(Side::Short),
            other => Err(format!("side must be 'long' or 'short', got '{other}'")),
        }
    }
    /// Parse a per-row side token; `None` when the cell isn't a side word.
    fn token(s: &str) -> Option<Side> {
        match s.trim().to_ascii_lowercase().as_str() {
            "long" | "l" | "buy" => Some(Side::Long),
            "short" | "s" | "sell" => Some(Side::Short),
            _ => None,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Side::Long => "long",
            Side::Short => "short",
        }
    }
}

/// How to order the position rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Keep the pasted order (default) so the table lines up with a statement.
    Input,
    /// Largest net gain first, biggest loss last.
    Pnl,
    /// Largest percentage return first.
    PnlPercent,
    /// Largest current market value first.
    Value,
    /// Alphabetical by position label (case-insensitive).
    Label,
}

impl Sort {
    /// Parse the `sort` parameter (empty = input order, the default).
    pub fn parse(s: &str) -> Result<Sort, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "input" | "none" | "as-pasted" => Ok(Sort::Input),
            "pnl" | "p&l" | "profit" => Ok(Sort::Pnl),
            "pnl_percent" | "percent" | "return" => Ok(Sort::PnlPercent),
            "value" => Ok(Sort::Value),
            "label" | "name" | "alpha" => Ok(Sort::Label),
            other => Err(format!(
                "sort must be 'input', 'pnl', 'pnl_percent', 'value', or 'label', got '{other}'"
            )),
        }
    }
}

/// One parsed input row, before any P/L maths.
#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    /// Ticker or free-text name from column 1.
    pub label: String,
    /// Units held (always positive; a negative input flips `side` to short).
    pub quantity: f64,
    /// Price paid (long) or sold at (short), per unit.
    pub entry: f64,
    /// Latest price you supplied, per unit.
    pub current: f64,
    /// Flat fee/commission for the whole position, from the optional 5th column.
    pub fee: f64,
    /// Dividends/interest collected on the position, from the optional 6th
    /// column. Counts towards total return but not towards the price P/L.
    pub dividends: f64,
    /// How long the position has been held, in days, from the optional 7th
    /// column. 0 means "not supplied" and turns the annualized return off.
    pub days: f64,
    /// Long or short, from the optional trailing word or the `side` default.
    pub side: Side,
}

/// One position with its P/L worked out.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionPnl {
    pub label: String,
    pub side: Side,
    pub quantity: f64,
    pub entry: f64,
    pub current: f64,
    /// `quantity × entry` — what the position cost to open.
    pub cost: f64,
    /// `quantity × current` — what it is worth now (for a short, the buy-back cost).
    pub value: f64,
    /// Flat row fee plus the percentage fee charged on both legs.
    pub fees: f64,
    /// Dividends/interest collected, carried through from the input row.
    pub dividends: f64,
    /// Days held, carried through from the input row; 0 when not supplied.
    pub days: f64,
    /// Price-only P/L, before dividends and fees.
    pub gross: f64,
    /// Total return: `gross + dividends - fees`.
    pub net: f64,
    /// `net / cost × 100`; 0 when the cost basis is 0.
    pub percent: f64,
    /// Annualized (CAGR-equivalent) return percent over `days`; NaN when `days`
    /// is 0 or the position lost more than its cost basis.
    pub annualized: f64,
    /// Price at which this position's net P/L reaches 0; NaN when unreachable.
    pub breakeven: f64,
}

/// The whole portfolio, worked out.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub positions: Vec<PositionPnl>,
    pub total_cost: f64,
    pub total_value: f64,
    pub gross: f64,
    pub fees: f64,
    /// Total dividends/interest collected across every position.
    pub dividends: f64,
    pub net: f64,
    pub net_percent: f64,
    /// Cost-weighted average holding period in days; 0 when no row supplied one.
    pub avg_days: f64,
    /// Portfolio annualized return percent over `avg_days`; NaN when unavailable.
    pub annualized: f64,
    /// Estimated tax on a positive net gain (0 when `tax_rate` is 0 or at a loss).
    pub tax: f64,
    /// `net - tax`.
    pub after_tax: f64,
    pub winners: usize,
    pub losers: usize,
    pub flat: usize,
}

/// Split one input line into cells. Prefers a tab delimiter (spreadsheet paste)
/// so a thousands-separated price like `1,500` survives; otherwise commas.
fn split_cells(line: &str) -> Vec<String> {
    let delim = if line.contains('\t') { '\t' } else { ',' };
    line.split(delim).map(|c| c.trim().to_string()).collect()
}

/// Strip currency/grouping ornamentation from a numeric string and parse it.
/// Accounting-style `(123.45)` denotes a negative amount.
fn clean_number(s: &str) -> Result<f64, String> {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, '$' | '€' | '£' | '¥' | ',' | '_' | ' ' | '\''))
        .collect();
    let (neg, body) = match cleaned.strip_prefix('(').and_then(|b| b.strip_suffix(')')) {
        Some(inner) => (true, inner.to_string()),
        None => (false, cleaned),
    };
    if body.is_empty() {
        return Err("value is empty".to_string());
    }
    let n: f64 = body
        .parse()
        .map_err(|_| format!("expected a number, got '{}'", s.trim()))?;
    if !n.is_finite() {
        return Err("value must be a finite number".to_string());
    }
    Ok(if neg { -n } else { n })
}

/// Does this cell look numeric? Used to spot (and drop) a header row.
fn looks_numeric(cell: &str) -> bool {
    clean_number(cell).is_ok()
}

/// Parse the pasted positions block. Blank lines and `#` comments are skipped;
/// a leading header row (whose quantity column isn't a number) is dropped.
/// Each row is `label, quantity, entry price, current price[, fees][, long|short]`.
pub fn parse_positions(input: &str, default_side: Side) -> Result<Vec<Position>, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is too large ({} bytes; max {})",
            input.len(),
            MAX_INPUT_BYTES
        ));
    }
    let mut positions = Vec::new();
    let mut seen_data = false;
    for (i, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cells = split_cells(line);
        // A header row: the first data-ish line whose quantity cell isn't a number.
        if !seen_data && cells.len() >= 2 && !looks_numeric(&cells[1]) {
            continue;
        }
        let n = i + 1;
        let label = cells.first().map(|s| s.trim()).unwrap_or("");
        if label.is_empty() {
            return Err(format!("line {n}: missing position name in column 1"));
        }
        if cells.len() < 4 {
            return Err(format!(
                "line {n}: '{label}' needs 4 columns \
                 (name, quantity, entry price, current price) — got {}",
                cells.len()
            ));
        }
        let num = |idx: usize, what: &str| -> Result<f64, String> {
            clean_number(&cells[idx]).map_err(|e| format!("line {n}: {what} for '{label}' — {e}"))
        };
        let quantity = num(1, "quantity")?;
        let entry = num(2, "entry price")?;
        let current = num(3, "current price")?;

        // Trailing cells: up to three numbers — fees, dividends, days held, in
        // that order — plus an optional long/short word anywhere among them. A
        // side word is never numeric, so the two can't collide. An empty cell
        // still holds its slot, so `A,1,1,2,,,90` supplies only days held.
        let mut tail = [0.0f64; 3];
        let mut slot = 0usize;
        let mut side = default_side;
        // A negative quantity is the spreadsheet shorthand for a short position.
        if quantity < 0.0 {
            side = Side::Short;
        }
        for (idx, cell) in cells.iter().enumerate().skip(4) {
            let cell = cell.trim();
            if let Some(s) = Side::token(cell) {
                side = s;
                continue;
            }
            if slot >= tail.len() {
                if cell.is_empty() {
                    continue;
                }
                return Err(format!(
                    "line {n}: '{label}' has more columns than expected — after the current \
                     price only fees, dividends, days held, and 'long'/'short' are read"
                ));
            }
            if !cell.is_empty() {
                tail[slot] = clean_number(cell).map_err(|e| {
                    format!(
                        "line {n}: column {} for '{label}' — {e} \
                         (expected fees, dividends, days held, 'long', or 'short')",
                        idx + 1
                    )
                })?;
            }
            slot += 1;
        }
        let [fee, dividends, days] = tail;

        let quantity = quantity.abs();
        if quantity == 0.0 {
            return Err(format!("line {n}: quantity for '{label}' must not be zero"));
        }
        if entry < 0.0 || current < 0.0 {
            return Err(format!(
                "line {n}: prices for '{label}' must not be negative \
                 (use 'short' for a short position, not a negative price)"
            ));
        }
        if fee < 0.0 {
            return Err(format!("line {n}: fees for '{label}' must not be negative"));
        }
        if dividends < 0.0 {
            return Err(format!(
                "line {n}: dividends for '{label}' must not be negative \
                 (column 6 is income collected, not a cost — put costs in the fees column)"
            ));
        }
        if days < 0.0 {
            return Err(format!(
                "line {n}: days held for '{label}' must not be negative"
            ));
        }

        positions.push(Position {
            label: label.to_string(),
            quantity,
            entry,
            current,
            fee,
            dividends,
            days,
            side,
        });
        seen_data = true;
        if positions.len() > MAX_POSITIONS {
            return Err(format!("too many positions (max {MAX_POSITIONS})"));
        }
    }
    if positions.is_empty() {
        return Err("no positions found — enter at least one line as \
             'name, quantity, entry price, current price[, fees][, long|short]'"
            .to_string());
    }
    Ok(positions)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Annualize a simple return of `percent` earned over `days`, as a percent.
/// NaN when the holding period is unknown or the loss wiped out more than the
/// cost basis (no real annual rate exists below -100%).
fn annualize(percent: f64, days: f64) -> f64 {
    if days <= 0.0 {
        return f64::NAN;
    }
    let growth = 1.0 + percent / 100.0;
    if growth <= 0.0 {
        return f64::NAN;
    }
    round2((growth.powf(365.0 / days) - 1.0) * 100.0)
}

/// Work out per-position and portfolio P/L.
///
/// `fee_percent` is charged on BOTH legs of every position (entry notional and
/// current notional) and is added to each row's flat fee. `tax_rate` is applied
/// to the portfolio's net gain only when that gain is positive — losses offset
/// gains before any tax is estimated.
pub fn compute(
    positions: &[Position],
    fee_percent: f64,
    tax_rate: f64,
    sort: Sort,
) -> Result<Report, String> {
    if !(0.0..=100.0).contains(&fee_percent) {
        return Err(format!(
            "fee_percent must be between 0 and 100, got {fee_percent}"
        ));
    }
    if !(0.0..=100.0).contains(&tax_rate) {
        return Err(format!(
            "tax_rate must be between 0 and 100, got {tax_rate}"
        ));
    }
    let fp = fee_percent / 100.0;

    let mut rows: Vec<PositionPnl> = Vec::with_capacity(positions.len());
    for p in positions {
        let cost = p.quantity * p.entry;
        let value = p.quantity * p.current;
        let fees = p.fee + fp * cost + fp * value;
        let gross = match p.side {
            Side::Long => (p.current - p.entry) * p.quantity,
            Side::Short => (p.entry - p.current) * p.quantity,
        };
        let net = gross + p.dividends - fees;
        let percent = if cost == 0.0 {
            0.0
        } else {
            round2(net / cost * 100.0)
        };
        // Solve net(price) = 0 for the exit price, with the percentage fee
        // charged on the entry notional and on that same unknown exit notional.
        // Dividends already banked lower the price the position has to reach.
        let breakeven = match p.side {
            Side::Long => {
                let denom = p.quantity * (1.0 - fp);
                if denom <= 0.0 {
                    f64::NAN
                } else {
                    // Income above the whole cost basis already breaks even at 0.
                    ((cost * (1.0 + fp) + p.fee - p.dividends) / denom).max(0.0)
                }
            }
            Side::Short => {
                let be = (cost * (1.0 - fp) - p.fee + p.dividends) / (p.quantity * (1.0 + fp));
                if be < 0.0 {
                    f64::NAN
                } else {
                    be
                }
            }
        };

        rows.push(PositionPnl {
            label: p.label.clone(),
            side: p.side,
            quantity: p.quantity,
            entry: p.entry,
            current: p.current,
            cost: round2(cost),
            value: round2(value),
            fees: round2(fees),
            dividends: round2(p.dividends),
            days: p.days,
            gross: round2(gross),
            net: round2(net),
            percent,
            annualized: annualize(percent, p.days),
            breakeven,
        });
    }

    let total_cost = round2(rows.iter().map(|r| r.cost).sum());
    let total_value = round2(rows.iter().map(|r| r.value).sum());
    let gross = round2(rows.iter().map(|r| r.gross).sum());
    let fees = round2(rows.iter().map(|r| r.fees).sum());
    let dividends = round2(rows.iter().map(|r| r.dividends).sum());
    let net = round2(gross + dividends - fees);
    let net_percent = if total_cost == 0.0 {
        0.0
    } else {
        round2(net / total_cost * 100.0)
    };
    let tax = if net > 0.0 {
        round2(net * tax_rate / 100.0)
    } else {
        0.0
    };
    let avg_days = if total_cost == 0.0 {
        0.0
    } else {
        let weighted: f64 = rows
            .iter()
            .filter(|r| r.days > 0.0)
            .map(|r| r.days * r.cost)
            .sum();
        round2(weighted / total_cost)
    };
    let annualized = annualize(net_percent, avg_days);
    let winners = rows.iter().filter(|r| r.net > 0.0).count();
    let losers = rows.iter().filter(|r| r.net < 0.0).count();
    let flat = rows.len() - winners - losers;

    // Sort last so the counts and totals never depend on presentation order.
    let by = |a: f64, b: f64| b.partial_cmp(&a).unwrap_or(std::cmp::Ordering::Equal);
    match sort {
        Sort::Input => {}
        Sort::Pnl => rows.sort_by(|x, y| by(x.net, y.net)),
        Sort::PnlPercent => rows.sort_by(|x, y| by(x.percent, y.percent)),
        Sort::Value => rows.sort_by(|x, y| by(x.value, y.value)),
        Sort::Label => rows.sort_by(|x, y| x.label.to_lowercase().cmp(&y.label.to_lowercase())),
    }

    Ok(Report {
        positions: rows,
        total_cost,
        total_value,
        gross,
        fees,
        dividends,
        net,
        net_percent,
        avg_days,
        annualized,
        tax,
        after_tax: round2(net - tax),
        winners,
        losers,
        flat,
    })
}

/// Format a value as `<sym>1,234.56` (accounting-negative `-<sym>1,234.56`).
fn format_money(v: f64, sym: &str) -> String {
    if !v.is_finite() {
        return "n/a".to_string();
    }
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
    let grouped = format!("{digits}{grouped}");
    format!(
        "{}{}{}.{:02}",
        if neg { "-" } else { "" },
        sym,
        grouped,
        cents
    )
}

/// Money with an explicit `+` on gains, so a P/L column reads at a glance.
fn format_signed(v: f64, sym: &str) -> String {
    if !v.is_finite() {
        return "n/a".to_string();
    }
    if round2(v) > 0.0 {
        format!("+{}", format_money(v, sym))
    } else {
        format_money(v, sym)
    }
}

/// Percent with an explicit sign, e.g. `+25.00%`.
fn format_percent(v: f64) -> String {
    if !v.is_finite() {
        return "n/a".to_string();
    }
    if round2(v) > 0.0 {
        format!("+{v:.2}%")
    } else {
        format!("{v:.2}%")
    }
}

/// Trim a quantity to the shortest exact-looking form (`50`, not `50.00`).
fn format_qty(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Lay out a fixed-width text table. `right` marks the right-aligned columns.
fn render_table(headers: &[&str], rows: &[Vec<String>], right: &[bool]) -> Vec<String> {
    let cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for c in 0..cols {
            widths[c] = widths[c].max(row[c].chars().count());
        }
    }
    let line = |cells: &[String]| -> String {
        let mut out = String::new();
        for c in 0..cols {
            if c > 0 {
                out.push_str("  ");
            }
            let pad = widths[c].saturating_sub(cells[c].chars().count());
            if right[c] {
                out.push_str(&" ".repeat(pad));
                out.push_str(&cells[c]);
            } else {
                out.push_str(&cells[c]);
                out.push_str(&" ".repeat(pad));
            }
        }
        out.trim_end().to_string()
    };
    let mut out = vec![line(
        &headers.iter().map(|h| h.to_string()).collect::<Vec<_>>(),
    )];
    for row in rows {
        out.push(line(row));
    }
    out
}

/// Plain-text P/L report — the page, CLI, and chat surface.
pub fn format_table(
    input: &str,
    default_side: Side,
    fee_percent: f64,
    tax_rate: f64,
    sort: Sort,
    currency: &str,
) -> Result<String, String> {
    let positions = parse_positions(input, default_side)?;
    let report = compute(&positions, fee_percent, tax_rate, sort)?;
    let sym = currency.trim();

    let has_short = report.positions.iter().any(|p| p.side == Side::Short);
    let has_fees = report.fees != 0.0;

    // Header line: the one-glance answer.
    let mut lines = vec![
        format!(
            "Portfolio P/L: {} on {} cost basis ({})",
            format_signed(report.net, sym),
            format_money(report.total_cost, sym),
            format_percent(report.net_percent)
        ),
        String::new(),
    ];

    // Per-position table. The Side column only appears when it carries
    // information; Break-even only when a fee actually moves it off the entry.
    let mut headers: Vec<&str> = vec!["Position"];
    let mut right: Vec<bool> = vec![false];
    if has_short {
        headers.push("Side");
        right.push(false);
    }
    headers.extend_from_slice(&["Qty", "Entry", "Price", "Cost", "Value", "P/L", "P/L %"]);
    right.extend_from_slice(&[true; 7]);
    if has_fees {
        headers.push("Fees");
        right.push(true);
        headers.push("Break-even");
        right.push(true);
    }

    let mut rows: Vec<Vec<String>> = Vec::with_capacity(report.positions.len() + 1);
    for p in &report.positions {
        let mut row = vec![p.label.clone()];
        if has_short {
            row.push(p.side.label().to_string());
        }
        row.push(format_qty(p.quantity));
        row.push(format_money(p.entry, sym));
        row.push(format_money(p.current, sym));
        row.push(format_money(p.cost, sym));
        row.push(format_money(p.value, sym));
        row.push(format_signed(p.net, sym));
        row.push(format_percent(p.percent));
        if has_fees {
            row.push(format_money(p.fees, sym));
            row.push(format_money(p.breakeven, sym));
        }
        rows.push(row);
    }
    // Totals row: blank through the per-unit columns, which don't add up.
    let mut total = vec!["Total".to_string()];
    if has_short {
        total.push(String::new());
    }
    total.extend_from_slice(&[String::new(), String::new(), String::new()]);
    total.push(format_money(report.total_cost, sym));
    total.push(format_money(report.total_value, sym));
    total.push(format_signed(report.net, sym));
    total.push(format_percent(report.net_percent));
    if has_fees {
        total.push(format_money(report.fees, sym));
        total.push(String::new());
    }
    rows.push(total);

    lines.extend(render_table(&headers, &rows, &right));
    lines.push(String::new());

    // Summary block.
    let label_w = 14;
    let mut push = |k: &str, v: String| lines.push(format!("{k:label_w$}{v}"));
    push("Cost basis", format_money(report.total_cost, sym));
    push("Market value", format_money(report.total_value, sym));
    push("Gross P/L", format_signed(report.gross, sym));
    if has_fees {
        push("Fees", format_money(report.fees, sym));
    }
    push(
        "Net P/L",
        format!(
            "{}  ({})",
            format_signed(report.net, sym),
            format_percent(report.net_percent)
        ),
    );
    if tax_rate > 0.0 {
        push(
            &format!("Tax @ {}%", format_qty(tax_rate)),
            format_money(report.tax, sym),
        );
        push("After-tax P/L", format_signed(report.after_tax, sym));
    }

    lines.push(String::new());
    lines.push(format!(
        "{} winner{} · {} loser{} · {} flat",
        report.winners,
        if report.winners == 1 { "" } else { "s" },
        report.losers,
        if report.losers == 1 { "" } else { "s" },
        report.flat
    ));
    if report.positions.len() > 1 {
        let best = report.positions.iter().max_by(|a, b| {
            a.net
                .partial_cmp(&b.net)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let worst = report.positions.iter().min_by(|a, b| {
            a.net
                .partial_cmp(&b.net)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if let (Some(b), Some(w)) = (best, worst) {
            lines.push(format!(
                "Best   {}  {} ({})",
                b.label,
                format_signed(b.net, sym),
                format_percent(b.percent)
            ));
            lines.push(format!(
                "Worst  {}  {} ({})",
                w.label,
                format_signed(w.net, sym),
                format_percent(w.percent)
            ));
        }
    }

    Ok(lines.join("\n"))
}

/// The scaffold's default entry point — the real surfaces call `format_table`
/// (text report) or `compute` (structured numbers).
pub fn run(input: &str) -> Result<String, String> {
    format_table(input, Side::Long, 0.0, 0.0, Sort::Input, "$")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Vec<Position> {
        parse_positions(s, Side::Long).unwrap()
    }

    #[test]
    fn basic_long_gain() {
        let p = parse("AAPL, 50, 150, 187.50");
        let r = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(r.positions[0].cost, 7500.0);
        assert_eq!(r.positions[0].value, 9375.0);
        assert_eq!(r.positions[0].net, 1875.0);
        assert_eq!(r.positions[0].percent, 25.0);
        assert_eq!(r.total_cost, 7500.0);
        assert_eq!(r.net_percent, 25.0);
        assert_eq!(r.winners, 1);
        assert_eq!(r.losers, 0);
    }

    #[test]
    fn loss_is_negative() {
        let p = parse("TSLA, 10, 250, 220");
        let r = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(r.positions[0].net, -300.0);
        assert_eq!(r.positions[0].percent, -12.0);
        assert_eq!(r.losers, 1);
        assert_eq!(r.winners, 0);
    }

    #[test]
    fn portfolio_totals_net_out() {
        let p = parse("AAPL, 50, 150, 187.50\nTSLA, 10, 250, 220");
        let r = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(r.total_cost, 10000.0);
        assert_eq!(r.total_value, 11575.0);
        assert_eq!(r.net, 1575.0);
        assert_eq!(r.net_percent, 15.75);
        assert_eq!(r.winners, 1);
        assert_eq!(r.losers, 1);
        assert_eq!(r.flat, 0);
    }

    #[test]
    fn short_position_profits_when_price_falls() {
        let p = parse_positions("GME, 100, 40, 25, 0, short", Side::Long).unwrap();
        assert_eq!(p[0].side, Side::Short);
        let r = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(r.positions[0].net, 1500.0);
        assert_eq!(r.positions[0].percent, 37.5);
    }

    #[test]
    fn negative_quantity_means_short() {
        let p = parse("GME, -100, 40, 25");
        assert_eq!(p[0].side, Side::Short);
        assert_eq!(p[0].quantity, 100.0);
    }

    #[test]
    fn side_default_applies_to_every_row() {
        let p = parse_positions("A, 10, 40, 25\nB, 10, 40, 50", Side::Short).unwrap();
        assert!(p.iter().all(|x| x.side == Side::Short));
        let r = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(r.positions[0].net, 150.0);
        assert_eq!(r.positions[1].net, -100.0);
    }

    #[test]
    fn row_side_word_overrides_the_default() {
        let p = parse_positions("A, 10, 40, 25, 0, long", Side::Short).unwrap();
        assert_eq!(p[0].side, Side::Long);
    }

    #[test]
    fn flat_row_registers_when_price_is_unchanged() {
        let p = parse("AAPL, 5, 100, 100");
        let r = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(r.flat, 1);
        assert_eq!(r.net, 0.0);
    }

    #[test]
    fn flat_fee_subtracts_from_net() {
        let p = parse("AAPL, 50, 150, 187.50, 9.99");
        let r = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(r.positions[0].fees, 9.99);
        assert_eq!(r.positions[0].gross, 1875.0);
        assert_eq!(r.positions[0].net, 1865.01);
    }

    #[test]
    fn percent_fee_charges_both_legs() {
        // 0.1% of 7,500 entry + 0.1% of 9,375 exit = 7.50 + 9.375 = 16.88 (rounded).
        let p = parse("AAPL, 50, 150, 187.50");
        let r = compute(&p, 0.1, 0.0, Sort::Input).unwrap();
        assert_eq!(r.positions[0].fees, 16.88);
        assert_eq!(r.positions[0].net, 1858.13);
    }

    #[test]
    fn breakeven_is_entry_when_there_are_no_fees() {
        let p = parse("AAPL, 50, 150, 187.50");
        let r = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(r.positions[0].breakeven, 150.0);
    }

    #[test]
    fn breakeven_covers_a_flat_fee() {
        // 100 shares at $10, $20 of fees → need $10.20 to break even.
        let p = parse("X, 100, 10, 12, 20");
        let r = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert!((r.positions[0].breakeven - 10.2).abs() < 1e-9);
        // Selling exactly at break-even nets zero.
        let at_be = parse("X, 100, 10, 10.20, 20");
        let z = compute(&at_be, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(z.positions[0].net, 0.0);
    }

    #[test]
    fn breakeven_covers_a_percentage_fee_on_both_legs() {
        let p = parse("X, 100, 10, 12");
        let r = compute(&p, 1.0, 0.0, Sort::Input).unwrap();
        let be = r.positions[0].breakeven;
        // Re-price the position at its break-even and confirm net is zero.
        let at_be = parse(&format!("X, 100, 10, {be}"));
        let z = compute(&at_be, 1.0, 0.0, Sort::Input).unwrap();
        assert_eq!(z.positions[0].net, 0.0);
    }

    #[test]
    fn short_breakeven_round_trips() {
        let p = parse_positions("S, 100, 40, 25, 50, short", Side::Long).unwrap();
        let r = compute(&p, 0.5, 0.0, Sort::Input).unwrap();
        let be = r.positions[0].breakeven;
        let at_be = parse_positions(&format!("S, 100, 40, {be}, 50, short"), Side::Long).unwrap();
        let z = compute(&at_be, 0.5, 0.0, Sort::Input).unwrap();
        assert_eq!(z.positions[0].net, 0.0);
    }

    #[test]
    fn tax_applies_only_to_a_net_gain_after_losses_offset() {
        // +1,875 and -300 net to +1,575; 15% of that is 236.25.
        let p = parse("AAPL, 50, 150, 187.50\nTSLA, 10, 250, 220");
        let r = compute(&p, 0.0, 15.0, Sort::Input).unwrap();
        assert_eq!(r.tax, 236.25);
        assert_eq!(r.after_tax, 1338.75);
    }

    #[test]
    fn tax_is_zero_at_an_overall_loss() {
        let p = parse("TSLA, 10, 250, 220");
        let r = compute(&p, 0.0, 30.0, Sort::Input).unwrap();
        assert_eq!(r.tax, 0.0);
        assert_eq!(r.after_tax, -300.0);
    }

    #[test]
    fn sorts_reorder_without_changing_totals() {
        let input = "Zed, 10, 10, 11\nAlpha, 10, 10, 30\nMid, 10, 10, 5";
        let p = parse(input);
        let base = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(base.positions[0].label, "Zed");

        let by_pnl = compute(&p, 0.0, 0.0, Sort::Pnl).unwrap();
        assert_eq!(by_pnl.positions[0].label, "Alpha");
        assert_eq!(by_pnl.positions[2].label, "Mid");

        let by_pct = compute(&p, 0.0, 0.0, Sort::PnlPercent).unwrap();
        assert_eq!(by_pct.positions[0].label, "Alpha");

        let by_value = compute(&p, 0.0, 0.0, Sort::Value).unwrap();
        assert_eq!(by_value.positions[0].label, "Alpha");

        let by_label = compute(&p, 0.0, 0.0, Sort::Label).unwrap();
        assert_eq!(by_label.positions[0].label, "Alpha");
        assert_eq!(by_label.positions[1].label, "Mid");
        assert_eq!(by_label.positions[2].label, "Zed");

        assert_eq!(base.net, by_label.net);
        assert_eq!(base.winners, by_label.winners);
    }

    #[test]
    fn header_row_is_dropped() {
        let p = parse("Name, Qty, Entry, Price\nAAPL, 50, 150, 187.50");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].label, "AAPL");
    }

    #[test]
    fn comments_and_blank_lines_skipped() {
        let p = parse("# my book\nAAPL, 1, 1, 2\n\nMSFT, 1, 1, 2\n");
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn currency_and_thousands_are_stripped() {
        // Comma is the delimiter, so thousands separators need a tab-delimited row.
        let p = parse("Fund\t2\t$1,234.50\t$1,500.00");
        assert_eq!(p[0].entry, 1234.5);
        assert_eq!(p[0].current, 1500.0);
    }

    #[test]
    fn fractional_quantities_work() {
        let p = parse("BTC, 0.25, 40000, 60000");
        let r = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(r.positions[0].cost, 10000.0);
        assert_eq!(r.positions[0].net, 5000.0);
    }

    #[test]
    fn zero_cost_basis_reports_zero_percent_not_infinity() {
        let p = parse("Grant, 100, 0, 12");
        let r = compute(&p, 0.0, 0.0, Sort::Input).unwrap();
        assert_eq!(r.positions[0].percent, 0.0);
        assert_eq!(r.positions[0].net, 1200.0);
        assert_eq!(r.net_percent, 0.0);
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse_positions("", Side::Long).is_err());
        assert!(parse_positions("# just a comment\n", Side::Long).is_err());
    }

    #[test]
    fn too_few_columns_errors_with_the_line_number() {
        let e = parse_positions("AAPL, 50, 150", Side::Long).unwrap_err();
        assert!(e.contains("line 1"), "got: {e}");
        assert!(e.contains("4 columns"), "got: {e}");
    }

    #[test]
    fn bad_number_errors_with_the_line_and_field() {
        let e = parse_positions("AAPL, 50, 150, 187\nMSFT, 10, oops, 12", Side::Long).unwrap_err();
        assert!(e.contains("line 2"), "got: {e}");
        assert!(e.contains("entry price"), "got: {e}");
        assert!(e.contains("MSFT"), "got: {e}");
    }

    #[test]
    fn zero_quantity_errors() {
        let e = parse_positions("AAPL, 0, 150, 187", Side::Long).unwrap_err();
        assert!(e.contains("must not be zero"), "got: {e}");
    }

    #[test]
    fn negative_price_errors() {
        let e = parse_positions("AAPL, 5, -150, 187", Side::Long).unwrap_err();
        assert!(e.contains("must not be negative"), "got: {e}");
    }

    #[test]
    fn out_of_range_rates_error() {
        let p = parse("AAPL, 1, 1, 2");
        assert!(compute(&p, 101.0, 0.0, Sort::Input).is_err());
        assert!(compute(&p, -1.0, 0.0, Sort::Input).is_err());
        assert!(compute(&p, 0.0, 101.0, Sort::Input).is_err());
    }

    #[test]
    fn position_cap_boundary() {
        let row = "A, 1, 1, 2\n";
        let at_cap: String = row.repeat(MAX_POSITIONS);
        assert_eq!(
            parse_positions(&at_cap, Side::Long).unwrap().len(),
            MAX_POSITIONS
        );
        let over: String = row.repeat(MAX_POSITIONS + 1);
        let e = parse_positions(&over, Side::Long).unwrap_err();
        assert!(e.contains("too many positions"), "got: {e}");
    }

    #[test]
    fn side_and_sort_parsers_reject_junk() {
        assert!(Side::parse("sideways").is_err());
        assert_eq!(Side::parse("Short").unwrap(), Side::Short);
        assert_eq!(Side::parse("").unwrap(), Side::Long);
        assert!(Sort::parse("bogus").is_err());
        assert_eq!(Sort::parse("").unwrap(), Sort::Input);
        assert_eq!(Sort::parse("pnl").unwrap(), Sort::Pnl);
    }

    #[test]
    fn money_formatting() {
        assert_eq!(format_money(1234567.5, "$"), "$1,234,567.50");
        assert_eq!(format_money(-500.0, "$"), "-$500.00");
        assert_eq!(format_money(0.999, "$"), "$1.00");
        assert_eq!(format_money(42.0, "€"), "€42.00");
        assert_eq!(format_money(42.0, ""), "42.00");
        assert_eq!(format_signed(1875.0, "$"), "+$1,875.00");
        assert_eq!(format_signed(-300.0, "$"), "-$300.00");
        assert_eq!(format_signed(0.0, "$"), "$0.00");
        assert_eq!(format_percent(25.0), "+25.00%");
        assert_eq!(format_percent(-12.0), "-12.00%");
        assert_eq!(format_qty(50.0), "50");
        assert_eq!(format_qty(0.25), "0.25");
    }

    #[test]
    fn table_renders_the_headline_and_rows() {
        let t = format_table(
            "AAPL, 50, 150, 187.50\nTSLA, 10, 250, 220",
            Side::Long,
            0.0,
            0.0,
            Sort::Input,
            "$",
        )
        .unwrap();
        assert!(
            t.starts_with("Portfolio P/L: +$1,575.00 on $10,000.00 cost basis (+15.75%)"),
            "got: {t}"
        );
        assert!(t.contains("+$1,875.00"), "got: {t}");
        assert!(t.contains("-$300.00"), "got: {t}");
        assert!(t.contains("1 winner · 1 loser · 0 flat"), "got: {t}");
        assert!(t.contains("Best   AAPL"), "got: {t}");
        assert!(t.contains("Worst  TSLA"), "got: {t}");
        // No fees anywhere → the Fees/Break-even columns stay out of the way.
        assert!(!t.contains("Break-even"), "got: {t}");
        // All long → no Side column.
        assert!(!t.contains("Side"), "got: {t}");
    }

    #[test]
    fn table_shows_side_and_fee_columns_when_they_matter() {
        let t = format_table(
            "AAPL, 50, 150, 187.50, 9.99\nGME, 100, 40, 25, 5, short",
            Side::Long,
            0.0,
            15.0,
            Sort::Pnl,
            "$",
        )
        .unwrap();
        assert!(t.contains("Side"), "got: {t}");
        assert!(t.contains("short"), "got: {t}");
        assert!(t.contains("Break-even"), "got: {t}");
        assert!(t.contains("Tax @ 15%"), "got: {t}");
        assert!(t.contains("After-tax P/L"), "got: {t}");
    }

    #[test]
    fn table_errors_are_readable() {
        let e = format_table("nope", Side::Long, 0.0, 0.0, Sort::Input, "$").unwrap_err();
        assert!(e.contains("needs 4 columns"), "got: {e}");
        let empty = format_table("   \n", Side::Long, 0.0, 0.0, Sort::Input, "$").unwrap_err();
        assert!(empty.contains("no positions found"), "got: {empty}");
    }

    #[test]
    fn run_uses_the_documented_defaults() {
        let t = run("AAPL, 50, 150, 187.50").unwrap();
        assert!(t.contains("+$1,875.00"), "got: {t}");
    }
}
