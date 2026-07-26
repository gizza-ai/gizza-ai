//! bank-statement-reconcile core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Reconciles a bank-statement CSV against a bookkeeping ledger CSV. Each side names a
//! date, a signed amount, and a memo/description column (by header NAME or 1-based index).
//! A statement transaction matches a ledger transaction when their signed amounts agree
//! within `amount_tolerance`, their dates fall within `date_tolerance_days` of each other,
//! and — among the candidates that pass those two gates — the fuzzy memo token similarity
//! (Sørensen–Dice over lowercased alphanumeric tokens, 0..=100) is highest. Matching is a
//! deterministic one-to-one greedy pass in statement order: each statement transaction claims
//! its single best still-available ledger candidate.
//!
//! Results are partitioned into four buckets:
//!   - `matched`              — a candidate was claimed AND its memo similarity ≥ `memo_threshold`.
//!   - `suggested_matches`    — a candidate was claimed but its memo similarity < `memo_threshold`
//!                              (amount + date line up, memo is weaker) — review before accepting.
//!   - `unmatched_statement`  — statement rows with no candidate inside the amount + date window.
//!   - `unmatched_ledger`     — ledger rows never claimed by any statement row.
//!
//! Output is Markdown, JSON, or a flat CSV (one row per bucketed item).

use serde_json::{json, Value};
use std::collections::BTreeSet;

/// Hard cap on rows parsed per CSV (excluding the header). Keeps the O(n·m) greedy pass bounded.
pub const MAX_ROWS: usize = 1000;

/// One parsed transaction from either side.
struct Txn {
    date_raw: String,
    date_days: i64,
    amount: f64,
    memo: String,
}

/// A claimed statement↔ledger pairing (matched or merely suggested).
struct Pairing {
    stmt: usize,
    ledger: usize,
    similarity: u8,
    amount_diff: f64,
    date_diff: i64,
}

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d.trim() {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            return Err(format!(
                "delimiter must be comma/tab/semicolon/pipe, got '{other}'"
            ))
        }
    })
}

/// Parse CSV text into (header, rows). Errors if there is no header row or too many rows.
fn parse_csv(
    data: &str,
    delim: u8,
    side: &str,
) -> Result<(csv::StringRecord, Vec<csv::StringRecord>), String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let mut recs = rdr
        .records()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{side} CSV parse error: {e}"))?;
    if recs.is_empty() {
        return Err(format!("{side} CSV is empty — a header row is required"));
    }
    let header = recs.remove(0);
    if recs.len() > MAX_ROWS {
        return Err(format!(
            "{side} CSV has {} data rows, over the {MAX_ROWS}-row limit — split the file and reconcile in batches",
            recs.len()
        ));
    }
    Ok((header, recs))
}

/// Resolve ONE column reference (header name, else 1-based index) to a 0-based index.
fn resolve_col(
    header: &csv::StringRecord,
    key: &str,
    side: &str,
    role: &str,
) -> Result<usize, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err(format!("{side} {role} column is required"));
    }
    if let Some(pos) = header.iter().position(|h| h == key) {
        return Ok(pos);
    }
    if let Ok(n) = key.parse::<usize>() {
        if n >= 1 && n <= header.len() {
            return Ok(n - 1);
        }
        return Err(format!(
            "{side} {role} index {n} out of range 1..={} (header has {} columns)",
            header.len(),
            header.len()
        ));
    }
    Err(format!(
        "{side} {role} column '{key}' not found in header [{}]",
        header.iter().collect::<Vec<_>>().join(", ")
    ))
}

fn cell<'a>(rec: &'a csv::StringRecord, idx: usize) -> &'a str {
    rec.get(idx).unwrap_or("")
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Days since 1970-01-01 (Howard Hinnant's days_from_civil).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Parse a date cell into a serial day number. Accepts ISO `YYYY-MM-DD` / `YYYY/MM/DD`,
/// and day/month-first `D/M/YYYY`, `M/D/YYYY`, `D.M.YYYY` (also `-` separated). An optional
/// time part after a space or `T` is ignored. Two-digit years map to 2000–2099.
/// For an ambiguous slash/dot/dash date the first field is read as the MONTH (US style)
/// unless it is >12, in which case it is the day.
fn parse_date(s: &str) -> Result<i64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty date".into());
    }
    // Drop an optional time component.
    let date_part = s.split(['T', ' ']).next().unwrap_or(s);
    let sep = date_part
        .chars()
        .find(|c| matches!(c, '-' | '/' | '.'))
        .ok_or_else(|| format!("unrecognized date '{s}' — use YYYY-MM-DD or DD/MM/YYYY"))?;
    let parts: Vec<&str> = date_part.split(sep).collect();
    if parts.len() != 3 {
        return Err(format!(
            "unrecognized date '{s}' — expected 3 fields, use YYYY-MM-DD or DD/MM/YYYY"
        ));
    }
    let num = |p: &str| -> Result<i64, String> {
        p.trim()
            .parse::<i64>()
            .map_err(|_| format!("unrecognized date '{s}' — non-numeric field '{p}'"))
    };
    let (y, mo, d) = if parts[0].trim().len() == 4 {
        // ISO: YYYY-MM-DD
        (num(parts[0])?, num(parts[1])?, num(parts[2])?)
    } else {
        // Day/month-first with the year last.
        let mut y = num(parts[2])?;
        let a = num(parts[0])?;
        let b = num(parts[1])?;
        let (mo, d) = if a > 12 && b <= 12 { (b, a) } else { (a, b) };
        if y < 100 {
            y += 2000;
        }
        (y, mo, d)
    };
    if !(1..=12).contains(&mo) {
        return Err(format!("date '{s}' has month {mo} out of range 1..=12"));
    }
    if d < 1 || d > days_in_month(y, mo) {
        return Err(format!(
            "date '{s}' has day {d} out of range for month {mo}"
        ));
    }
    Ok(days_from_civil(y, mo, d))
}

/// Parse a signed amount, tolerating currency symbols, thousands separators, and
/// accounting-style `(1,234.56)` negatives.
fn parse_amount(s: &str) -> Result<f64, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("empty amount".into());
    }
    let mut neg = false;
    let mut body = t;
    if body.starts_with('(') && body.ends_with(')') {
        neg = true;
        body = &body[1..body.len() - 1];
    }
    let mut buf = String::new();
    let mut digits = 0usize;
    for ch in body.chars() {
        match ch {
            '0'..='9' => {
                buf.push(ch);
                digits += 1;
            }
            '.' => buf.push(ch),
            '-' => neg = true,
            '+' | ',' | ' ' | '\u{a0}' | '\'' | '_' => {}
            '$' | '€' | '£' | '¥' | '₹' | '¢' => {}
            c if c.is_alphabetic() => {} // trailing/leading currency codes like "USD"
            _ => return Err(format!("unrecognized amount '{s}' — bad character '{ch}'")),
        }
    }
    if digits == 0 {
        return Err(format!("unrecognized amount '{s}' — no digits"));
    }
    let mut v: f64 = buf
        .parse()
        .map_err(|_| format!("unrecognized amount '{s}'"))?;
    if neg {
        v = -v;
    }
    Ok(v)
}

/// Lowercased alphanumeric token set of a memo.
fn tokenize(s: &str) -> BTreeSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

/// Sørensen–Dice memo similarity, 0..=100. Two empty memos are identical (100);
/// one empty and one non-empty share nothing (0).
fn memo_similarity(a: &str, b: &str) -> u8 {
    let ta = tokenize(a);
    let tb = tokenize(b);
    if ta.is_empty() && tb.is_empty() {
        return 100;
    }
    if ta.is_empty() || tb.is_empty() {
        return 0;
    }
    let inter = ta.intersection(&tb).count();
    let dice = 2.0 * inter as f64 / (ta.len() + tb.len()) as f64;
    (dice * 100.0).round() as u8
}

/// Round to 4 decimals for stable display, normalizing -0.0 → 0.0.
fn round4(x: f64) -> f64 {
    let r = (x * 10000.0).round() / 10000.0;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn fmt_num(x: f64) -> String {
    format!("{}", round4(x))
}

fn parse_side(
    csv_text: &str,
    delim: u8,
    date_col: &str,
    amount_col: &str,
    memo_col: &str,
    side: &str,
) -> Result<Vec<Txn>, String> {
    let (header, rows) = parse_csv(csv_text, delim, side)?;
    let di = resolve_col(&header, date_col, side, "date")?;
    let ai = resolve_col(&header, amount_col, side, "amount")?;
    let mi = resolve_col(&header, memo_col, side, "memo")?;
    let mut out = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        let rownum = i + 2; // 1-based, +1 for the header row
        let date_raw = cell(r, di).trim().to_string();
        let date_days = parse_date(&date_raw).map_err(|e| format!("{side} row {rownum}: {e}"))?;
        let amount = parse_amount(cell(r, ai)).map_err(|e| format!("{side} row {rownum}: {e}"))?;
        let memo = cell(r, mi).trim().to_string();
        out.push(Txn {
            date_raw,
            date_days,
            amount,
            memo,
        });
    }
    Ok(out)
}

/// Reconcile a statement CSV against a ledger CSV. See the module docs for the algorithm.
#[allow(clippy::too_many_arguments)]
pub fn reconcile(
    statement_csv: &str,
    ledger_csv: &str,
    stmt_date: &str,
    stmt_amount: &str,
    stmt_memo: &str,
    ledger_date: &str,
    ledger_amount: &str,
    ledger_memo: &str,
    date_tolerance_days: i64,
    amount_tolerance: f64,
    memo_threshold: u8,
    delimiter: &str,
    output: &str,
) -> Result<String, String> {
    if statement_csv.trim().is_empty() {
        return Err("statement CSV is empty".into());
    }
    if ledger_csv.trim().is_empty() {
        return Err("ledger CSV is empty".into());
    }
    if date_tolerance_days < 0 {
        return Err("date_tolerance_days must be 0 or more".into());
    }
    if amount_tolerance < 0.0 || !amount_tolerance.is_finite() {
        return Err("amount_tolerance must be 0 or more".into());
    }
    if memo_threshold > 100 {
        return Err("memo_threshold must be between 0 and 100".into());
    }
    let out_fmt = match output.trim().to_ascii_lowercase().as_str() {
        "" | "markdown" | "md" => "markdown",
        "json" => "json",
        "csv" => "csv",
        other => return Err(format!("output must be markdown/json/csv, got '{other}'")),
    };
    let delim = delim_byte(delimiter)?;

    let stmt = parse_side(
        statement_csv,
        delim,
        stmt_date,
        stmt_amount,
        stmt_memo,
        "statement",
    )?;
    let ledger = parse_side(
        ledger_csv,
        delim,
        ledger_date,
        ledger_amount,
        ledger_memo,
        "ledger",
    )?;

    let amount_tol = amount_tolerance + 1e-9; // absorb float noise at the boundary
    let mut ledger_taken = vec![false; ledger.len()];
    let mut matched: Vec<Pairing> = Vec::new();
    let mut suggested: Vec<Pairing> = Vec::new();
    let mut unmatched_stmt: Vec<usize> = Vec::new();

    for (si, s) in stmt.iter().enumerate() {
        // Best still-available ledger candidate inside the amount + date window.
        let mut best: Option<(usize, u8, f64, i64)> = None; // (ledger idx, sim, amt_diff, date_diff)
        for (li, l) in ledger.iter().enumerate() {
            if ledger_taken[li] {
                continue;
            }
            let amt_diff = (s.amount - l.amount).abs();
            if amt_diff > amount_tol {
                continue;
            }
            let date_diff = (s.date_days - l.date_days).abs();
            if date_diff > date_tolerance_days {
                continue;
            }
            let sim = memo_similarity(&s.memo, &l.memo);
            let better = match best {
                None => true,
                Some((_, bsim, bamt, bdate)) => {
                    sim > bsim
                        || (sim == bsim
                            && (amt_diff < bamt - 1e-12
                                || (amt_diff <= bamt + 1e-12 && date_diff < bdate)))
                }
            };
            if better {
                best = Some((li, sim, amt_diff, date_diff));
            }
        }
        match best {
            Some((li, sim, amt_diff, date_diff)) => {
                ledger_taken[li] = true;
                let p = Pairing {
                    stmt: si,
                    ledger: li,
                    similarity: sim,
                    amount_diff: amt_diff,
                    date_diff,
                };
                if sim >= memo_threshold {
                    matched.push(p);
                } else {
                    suggested.push(p);
                }
            }
            None => unmatched_stmt.push(si),
        }
    }
    let unmatched_ledger: Vec<usize> = (0..ledger.len()).filter(|&i| !ledger_taken[i]).collect();

    Ok(match out_fmt {
        "json" => render_json(
            &stmt,
            &ledger,
            &matched,
            &suggested,
            &unmatched_stmt,
            &unmatched_ledger,
        ),
        "csv" => render_csv(
            &stmt,
            &ledger,
            &matched,
            &suggested,
            &unmatched_stmt,
            &unmatched_ledger,
        )?,
        _ => render_markdown(
            &stmt,
            &ledger,
            &matched,
            &suggested,
            &unmatched_stmt,
            &unmatched_ledger,
        ),
    })
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}

fn render_markdown(
    stmt: &[Txn],
    ledger: &[Txn],
    matched: &[Pairing],
    suggested: &[Pairing],
    unmatched_stmt: &[usize],
    unmatched_ledger: &[usize],
) -> String {
    let mut out = String::new();
    out.push_str("# Bank statement reconciliation\n\n");
    out.push_str(&format!("- Statement transactions: {}\n", stmt.len()));
    out.push_str(&format!("- Ledger transactions: {}\n", ledger.len()));
    out.push_str(&format!("- Matched: {}\n", matched.len()));
    out.push_str(&format!("- Suggested matches: {}\n", suggested.len()));
    out.push_str(&format!(
        "- Unmatched statement: {}\n",
        unmatched_stmt.len()
    ));
    out.push_str(&format!("- Unmatched ledger: {}\n", unmatched_ledger.len()));

    let pair_table = |title: &str, rows: &[Pairing], out: &mut String| {
        out.push_str(&format!("\n## {title} ({})\n\n", rows.len()));
        if rows.is_empty() {
            out.push_str("_None._\n");
            return;
        }
        out.push_str("| Statement date | Amount | Statement memo | Ledger date | Ledger memo | Memo % | Δ days | Δ amount |\n");
        out.push_str("|---|---|---|---|---|---|---|---|\n");
        for p in rows {
            let s = &stmt[p.stmt];
            let l = &ledger[p.ledger];
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                md_cell(&s.date_raw),
                fmt_num(s.amount),
                md_cell(&s.memo),
                md_cell(&l.date_raw),
                md_cell(&l.memo),
                p.similarity,
                p.date_diff,
                fmt_num(p.amount_diff),
            ));
        }
    };
    pair_table("Matched", matched, &mut out);
    pair_table("Suggested matches", suggested, &mut out);

    let unmatched_table = |title: &str, idxs: &[usize], src: &[Txn], out: &mut String| {
        out.push_str(&format!("\n## {title} ({})\n\n", idxs.len()));
        if idxs.is_empty() {
            out.push_str("_None._\n");
            return;
        }
        out.push_str("| Date | Amount | Memo |\n|---|---|---|\n");
        for &i in idxs {
            let t = &src[i];
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                md_cell(&t.date_raw),
                fmt_num(t.amount),
                md_cell(&t.memo),
            ));
        }
    };
    unmatched_table("Unmatched statement", unmatched_stmt, stmt, &mut out);
    unmatched_table("Unmatched ledger", unmatched_ledger, ledger, &mut out);
    out
}

fn render_csv(
    stmt: &[Txn],
    ledger: &[Txn],
    matched: &[Pairing],
    suggested: &[Pairing],
    unmatched_stmt: &[usize],
    unmatched_ledger: &[usize],
) -> Result<String, String> {
    let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
    wtr.write_record([
        "category",
        "statement_date",
        "statement_amount",
        "statement_memo",
        "ledger_date",
        "ledger_amount",
        "ledger_memo",
        "memo_similarity",
        "date_diff_days",
        "amount_diff",
    ])
    .map_err(|e| format!("CSV write error: {e}"))?;

    let pair_rows =
        |cat: &str, rows: &[Pairing], wtr: &mut csv::Writer<Vec<u8>>| -> Result<(), String> {
            for p in rows {
                let s = &stmt[p.stmt];
                let l = &ledger[p.ledger];
                wtr.write_record([
                    cat,
                    &s.date_raw,
                    &fmt_num(s.amount),
                    &s.memo,
                    &l.date_raw,
                    &fmt_num(l.amount),
                    &l.memo,
                    &p.similarity.to_string(),
                    &p.date_diff.to_string(),
                    &fmt_num(p.amount_diff),
                ])
                .map_err(|e| format!("CSV write error: {e}"))?;
            }
            Ok(())
        };
    pair_rows("matched", matched, &mut wtr)?;
    pair_rows("suggested", suggested, &mut wtr)?;

    for &i in unmatched_stmt {
        let t = &stmt[i];
        wtr.write_record([
            "unmatched_statement",
            &t.date_raw,
            &fmt_num(t.amount),
            &t.memo,
            "",
            "",
            "",
            "",
            "",
            "",
        ])
        .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for &i in unmatched_ledger {
        let t = &ledger[i];
        wtr.write_record([
            "unmatched_ledger",
            "",
            "",
            "",
            &t.date_raw,
            &fmt_num(t.amount),
            &t.memo,
            "",
            "",
            "",
        ])
        .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("output not valid UTF-8: {e}"))
}

fn txn_json(t: &Txn) -> Value {
    json!({
        "date": t.date_raw,
        "amount": round4(t.amount),
        "memo": t.memo,
    })
}

fn pairing_json(t_stmt: &[Txn], t_ledger: &[Txn], p: &Pairing) -> Value {
    json!({
        "statement": txn_json(&t_stmt[p.stmt]),
        "ledger": txn_json(&t_ledger[p.ledger]),
        "memo_similarity": p.similarity,
        "date_diff_days": p.date_diff,
        "amount_diff": round4(p.amount_diff),
    })
}

fn render_json(
    stmt: &[Txn],
    ledger: &[Txn],
    matched: &[Pairing],
    suggested: &[Pairing],
    unmatched_stmt: &[usize],
    unmatched_ledger: &[usize],
) -> String {
    let v = json!({
        "summary": {
            "statement_count": stmt.len(),
            "ledger_count": ledger.len(),
            "matched": matched.len(),
            "suggested_matches": suggested.len(),
            "unmatched_statement": unmatched_stmt.len(),
            "unmatched_ledger": unmatched_ledger.len(),
        },
        "matched": matched.iter().map(|p| pairing_json(stmt, ledger, p)).collect::<Vec<_>>(),
        "suggested_matches": suggested.iter().map(|p| pairing_json(stmt, ledger, p)).collect::<Vec<_>>(),
        "unmatched_statement": unmatched_stmt.iter().map(|&i| txn_json(&stmt[i])).collect::<Vec<_>>(),
        "unmatched_ledger": unmatched_ledger.iter().map(|&i| txn_json(&ledger[i])).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STMT: &str =
        "date,amount,description\n2024-01-03,-45.00,COFFEE ROASTERS LLC\n2024-01-05,-120.50,ACME UTILITIES\n2024-01-09,2000.00,PAYROLL DEPOSIT";
    const LEDGER: &str =
        "date,amount,memo\n2024-01-04,-45.00,Coffee Roasters\n2024-01-05,-120.50,Acme Utilities Co\n2024-01-31,-9.99,Bank fee";

    fn run_md() -> String {
        reconcile(
            STMT,
            LEDGER,
            "date",
            "amount",
            "description",
            "date",
            "amount",
            "memo",
            3,
            0.01,
            70,
            "comma",
            "markdown",
        )
        .unwrap()
    }

    #[test]
    fn matches_within_date_and_amount_window_with_fuzzy_memo() {
        let out = run_md();
        assert!(out.contains("- Matched: 2"), "{out}");
        assert!(out.contains("- Suggested matches: 0"), "{out}");
        assert!(out.contains("- Unmatched statement: 1"), "{out}"); // payroll deposit
        assert!(out.contains("- Unmatched ledger: 1"), "{out}"); // bank fee
        assert!(out.contains("COFFEE ROASTERS LLC"), "{out}");
    }

    #[test]
    fn below_threshold_candidate_is_suggested_not_matched() {
        // Same amount + date, but memos share no tokens → weak similarity → suggested.
        let s = "date,amount,memo\n2024-02-01,-30.00,STARBUCKS 4412";
        let l = "date,amount,memo\n2024-02-01,-30.00,Rent payment";
        let out = reconcile(
            s, l, "date", "amount", "memo", "date", "amount", "memo", 3, 0.01, 70, "comma",
            "markdown",
        )
        .unwrap();
        assert!(out.contains("- Matched: 0"), "{out}");
        assert!(out.contains("- Suggested matches: 1"), "{out}");
    }

    #[test]
    fn amount_sign_must_agree() {
        // A +45 statement credit must not match a -45 ledger debit.
        let s = "date,amount,memo\n2024-01-03,45.00,Refund";
        let l = "date,amount,memo\n2024-01-03,-45.00,Refund";
        let out = reconcile(
            s, l, "date", "amount", "memo", "date", "amount", "memo", 3, 0.01, 70, "comma",
            "markdown",
        )
        .unwrap();
        assert!(out.contains("- Matched: 0"), "{out}");
        assert!(out.contains("- Unmatched statement: 1"), "{out}");
        assert!(out.contains("- Unmatched ledger: 1"), "{out}");
    }

    #[test]
    fn date_outside_window_does_not_match() {
        let s = "date,amount,memo\n2024-01-01,-10.00,Netflix";
        let l = "date,amount,memo\n2024-01-20,-10.00,Netflix";
        let out = reconcile(
            s, l, "date", "amount", "memo", "date", "amount", "memo", 3, 0.01, 70, "comma",
            "markdown",
        )
        .unwrap();
        assert!(out.contains("- Matched: 0"), "{out}");
    }

    #[test]
    fn amount_tolerance_allows_small_difference() {
        let s = "date,amount,memo\n2024-01-01,-10.00,Netflix";
        let l = "date,amount,memo\n2024-01-01,-10.02,Netflix";
        let out = reconcile(
            s, l, "date", "amount", "memo", "date", "amount", "memo", 3, 0.05, 70, "comma",
            "markdown",
        )
        .unwrap();
        assert!(out.contains("- Matched: 1"), "{out}");
    }

    #[test]
    fn greedy_prefers_best_memo_candidate() {
        // Two ledger rows fit the amount+date window; the better memo wins the single statement row.
        let s = "date,amount,memo\n2024-03-01,-50.00,AMAZON MARKETPLACE";
        let l = "date,amount,memo\n2024-03-01,-50.00,Grocery store\n2024-03-02,-50.00,Amazon Marketplace order";
        let out = reconcile(
            s, l, "date", "amount", "memo", "date", "amount", "memo", 3, 0.01, 40, "comma", "csv",
        )
        .unwrap();
        // The matched row must pair with the Amazon ledger memo, not the grocery one.
        assert!(
            out.contains(
                "matched,2024-03-01,-50,AMAZON MARKETPLACE,2024-03-02,-50,Amazon Marketplace order"
            ),
            "{out}"
        );
        assert!(
            out.contains("unmatched_ledger,,,,2024-03-01,-50,Grocery store"),
            "{out}"
        );
    }

    #[test]
    fn json_output_has_buckets_and_summary() {
        let out = reconcile(
            STMT,
            LEDGER,
            "date",
            "amount",
            "description",
            "date",
            "amount",
            "memo",
            3,
            0.01,
            70,
            "comma",
            "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["matched"], 2);
        assert_eq!(v["summary"]["unmatched_statement"], 1);
        assert_eq!(v["matched"][0]["statement"]["memo"], "COFFEE ROASTERS LLC");
        assert_eq!(v["unmatched_ledger"][0]["memo"], "Bank fee");
    }

    #[test]
    fn columns_by_index_and_alternate_delimiter() {
        let s = "when;value;note\n2024-01-01;-10.00;Netflix";
        let l = "when;value;note\n2024-01-01;-10.00;Netflix sub";
        let out = reconcile(
            s,
            l,
            "1",
            "2",
            "3",
            "1",
            "2",
            "3",
            3,
            0.01,
            40,
            "semicolon",
            "markdown",
        )
        .unwrap();
        assert!(out.contains("- Matched: 1"), "{out}");
    }

    #[test]
    fn accounting_negatives_and_currency_symbols_parse() {
        let s = "date,amount,memo\n2024-01-01,\"($1,234.56)\",Big invoice";
        let l = "date,amount,memo\n2024-01-02,-1234.56,Big invoice";
        let out = reconcile(
            s, l, "date", "amount", "memo", "date", "amount", "memo", 3, 0.01, 70, "comma",
            "markdown",
        )
        .unwrap();
        assert!(out.contains("- Matched: 1"), "{out}");
    }

    #[test]
    fn dd_mm_yyyy_dates_parse() {
        let s = "date,amount,memo\n15/01/2024,-10.00,Netflix";
        let l = "date,amount,memo\n16/01/2024,-10.00,Netflix";
        let out = reconcile(
            s, l, "date", "amount", "memo", "date", "amount", "memo", 3, 0.01, 70, "comma",
            "markdown",
        )
        .unwrap();
        assert!(out.contains("- Matched: 1"), "{out}");
    }

    // --- error paths ---

    #[test]
    fn missing_column_errors() {
        let err = reconcile(
            STMT,
            LEDGER,
            "nope",
            "amount",
            "description",
            "date",
            "amount",
            "memo",
            3,
            0.01,
            70,
            "comma",
            "markdown",
        )
        .unwrap_err();
        assert!(
            err.contains("statement date column 'nope' not found"),
            "{err}"
        );
    }

    #[test]
    fn bad_amount_errors() {
        let s = "date,amount,memo\n2024-01-01,not-a-number,Netflix";
        let err = reconcile(
            s, LEDGER, "date", "amount", "memo", "date", "amount", "memo", 3, 0.01, 70, "comma",
            "markdown",
        )
        .unwrap_err();
        assert!(
            err.contains("statement row 2") && err.contains("unrecognized amount"),
            "{err}"
        );
    }

    #[test]
    fn bad_date_errors() {
        let s = "date,amount,memo\n2024-13-01,-10.00,Netflix";
        let err = reconcile(
            s, LEDGER, "date", "amount", "memo", "date", "amount", "memo", 3, 0.01, 70, "comma",
            "markdown",
        )
        .unwrap_err();
        assert!(
            err.contains("statement row 2") && err.contains("month 13"),
            "{err}"
        );
    }

    #[test]
    fn bad_output_errors() {
        let err = reconcile(
            STMT,
            LEDGER,
            "date",
            "amount",
            "description",
            "date",
            "amount",
            "memo",
            3,
            0.01,
            70,
            "comma",
            "xml",
        )
        .unwrap_err();
        assert!(err.contains("output must be markdown/json/csv"), "{err}");
    }

    #[test]
    fn bad_delimiter_errors() {
        let err = reconcile(
            STMT,
            LEDGER,
            "date",
            "amount",
            "description",
            "date",
            "amount",
            "memo",
            3,
            0.01,
            70,
            "colon",
            "markdown",
        )
        .unwrap_err();
        assert!(err.contains("delimiter must be"), "{err}");
    }

    #[test]
    fn row_cap_enforced() {
        let mut s = String::from("date,amount,memo\n");
        for i in 0..(MAX_ROWS + 1) {
            s.push_str(&format!("2024-01-01,-1.00,row{i}\n"));
        }
        let err = reconcile(
            &s, LEDGER, "date", "amount", "memo", "date", "amount", "memo", 3, 0.01, 70, "comma",
            "markdown",
        )
        .unwrap_err();
        assert!(
            err.contains("over the") && err.contains("row limit"),
            "{err}"
        );
    }
}
