//! ledger-balance core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Parses a ledger-cli / hledger
//! plain-text journal and reports the balance of every account and sub-account.
//!
//! The parser understands the common journal surface: transaction headers with
//! an optional status flag / `(code)` / `payee | note`, indented postings
//! separated from their amount by two-or-more spaces (or a tab), virtual
//! postings in `(…)` / `[…]`, `@` / `@@` prices, `= AMOUNT` balance assertions
//! (parsed, never counted as an amount), a single amount-less posting per
//! transaction (inferred), and the `account` / `alias` / `commodity` / `D` /
//! `Y` / `P` / `apply account` / `comment` directives. `include` has no
//! meaning without a filesystem, so those lines are skipped and reported as a
//! note.
//!
//! Amounts are held as fixed-point integers scaled by 1e8 so that summing
//! thousands of postings stays exact; each commodity is printed back with the
//! decimal precision it was written with (or the one a `commodity` / `D`
//! directive declared).

use std::collections::{BTreeMap, BTreeSet};

/// Hard cap on transactions parsed in one call, so a huge paste can't blow up
/// memory in the browser or the wasm sandbox. The chat/CLI schema advertises
/// this same bound.
pub const MAX_TRANSACTIONS: usize = 5_000;

/// Deepest account level the `depth` fold accepts. Real charts of accounts are
/// nowhere near this deep, and the page renders `depth` as a 0–10 slider.
pub const MAX_DEPTH: usize = 10;

/// Fixed-point scale: every quantity is stored in units of 1e-8.
const SCALE: i128 = 100_000_000;
/// Most decimal places we will ever keep or print.
const MAX_PREC: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Placement {
    Prefix,
    Suffix,
}

/// A quantity in one commodity. `prec` is the number of decimals as written.
#[derive(Clone, Debug, PartialEq)]
struct Amount {
    commodity: String,
    qty: i128,
    prec: usize,
    placement: Placement,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    Unmarked,
    Pending,
    Cleared,
}

#[derive(Clone, Debug)]
struct Posting {
    account: String,
    virtual_posting: bool,
    /// `None` when the posting had no amount (to be inferred).
    amount: Option<Amount>,
    /// Amount converted to its cost commodity, when an `@` / `@@` price was given.
    cost: Option<Amount>,
}

#[derive(Clone, Debug)]
struct Txn {
    date: String,
    status: Status,
    postings: Vec<Posting>,
}

/// Display style for one commodity: where the symbol goes and how many
/// decimals to print.
#[derive(Clone, Copy, Debug)]
struct Style {
    placement: Placement,
    prec: usize,
}

// ---------------------------------------------------------------------------
// number / amount parsing
// ---------------------------------------------------------------------------

/// Parse a decimal number that may use `,` or `.` as the decimal separator and
/// the other as a thousands separator. Returns the value scaled by 1e8 plus the
/// number of decimals as written.
fn parse_number(raw: &str) -> Result<(i128, usize), String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != '\u{a0}')
        .collect();
    if cleaned.is_empty() {
        return Err("missing number".to_string());
    }
    let (neg, body) = match cleaned.strip_prefix('-') {
        Some(rest) => (true, rest.to_string()),
        None => (
            false,
            cleaned.strip_prefix('+').unwrap_or(&cleaned).to_string(),
        ),
    };
    if body.is_empty()
        || !body
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
    {
        return Err(format!("'{raw}' is not a number"));
    }

    let dots = body.matches('.').count();
    let commas = body.matches(',').count();
    // Which separator (if any) marks the decimal point?
    let decimal_sep: Option<char> = match (dots, commas) {
        (0, 0) => None,
        (_, 0) => {
            // A single '.' is a decimal point; several are thousands separators.
            if dots == 1 {
                Some('.')
            } else {
                None
            }
        }
        (0, _) => {
            // A single ',' followed by exactly three digits is the usual
            // thousands grouping; anything else is a decimal comma.
            if commas == 1 && body.split(',').nth(1).map(|t| t.len()) == Some(3) {
                None
            } else if commas == 1 {
                Some(',')
            } else {
                None
            }
        }
        // Both present: the rightmost one is the decimal separator.
        _ => {
            if body.rfind('.') > body.rfind(',') {
                Some('.')
            } else {
                Some(',')
            }
        }
    };

    let (int_str, frac_str) = match decimal_sep {
        Some(sep) => {
            let idx = body.rfind(sep).expect("separator present");
            (body[..idx].to_string(), body[idx + 1..].to_string())
        }
        None => (body.clone(), String::new()),
    };
    let int_digits: String = int_str.chars().filter(|c| c.is_ascii_digit()).collect();
    if frac_str.chars().any(|c| !c.is_ascii_digit()) {
        return Err(format!("'{raw}' is not a number"));
    }
    let prec = frac_str.len().min(MAX_PREC);
    let frac_digits: String = frac_str.chars().take(MAX_PREC).collect();

    let int_val: i128 = if int_digits.is_empty() {
        0
    } else {
        int_digits
            .parse::<i128>()
            .map_err(|_| format!("'{raw}' is too large to add up"))?
    };
    let mut qty = int_val
        .checked_mul(SCALE)
        .ok_or_else(|| format!("'{raw}' is too large to add up"))?;
    if !frac_digits.is_empty() {
        let scale_up = 10i128.pow((MAX_PREC - frac_digits.len()) as u32);
        qty += frac_digits.parse::<i128>().unwrap_or(0) * scale_up;
    }
    Ok((if neg { -qty } else { qty }, prec))
}

fn is_number_char(c: char) -> bool {
    c.is_ascii_digit() || c == '.' || c == ','
}

/// Parse an amount such as `$1,234.56`, `-$5`, `1234.56 USD`, `10 AAPL`,
/// `"my fund" 3`. `default_commodity` supplies the commodity for a bare number.
fn parse_amount(
    raw: &str,
    default_commodity: Option<&(String, Placement)>,
) -> Result<Amount, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("missing amount".to_string());
    }
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    let mut sign = 1i128;

    // Leading sign, then optionally a prefix commodity, then optionally another sign.
    if chars[i] == '-' || chars[i] == '+' {
        if chars[i] == '-' {
            sign = -1;
        }
        i += 1;
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
        }
    }
    let mut prefix = String::new();
    if i < chars.len() && chars[i] == '"' {
        let start = i + 1;
        let mut j = start;
        while j < chars.len() && chars[j] != '"' {
            j += 1;
        }
        if j >= chars.len() {
            return Err(format!("unterminated quoted commodity in '{raw}'"));
        }
        prefix = chars[start..j].iter().collect();
        i = j + 1;
    } else {
        while i < chars.len() && !is_number_char(chars[i]) && chars[i] != '-' && chars[i] != '+' {
            prefix.push(chars[i]);
            i += 1;
        }
    }
    let prefix = prefix.trim().to_string();
    while i < chars.len() && chars[i] == ' ' {
        i += 1;
    }
    if i < chars.len() && (chars[i] == '-' || chars[i] == '+') {
        if chars[i] == '-' {
            sign = -sign;
        }
        i += 1;
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
        }
    }

    let num_start = i;
    while i < chars.len() && (is_number_char(chars[i]) || chars[i] == ' ') {
        i += 1;
    }
    // Trailing spaces belong to the commodity separator, not the number.
    while i > num_start && chars[i - 1] == ' ' {
        i -= 1;
    }
    let num: String = chars[num_start..i].iter().collect();
    if num.trim().is_empty() {
        return Err(format!("'{}' has no number in it", raw.trim()));
    }
    let (mut qty, prec) = parse_number(&num)?;
    qty *= sign;

    let mut suffix: String = chars[i..].iter().collect();
    suffix = suffix.trim().to_string();
    if suffix.starts_with('"') && suffix.ends_with('"') && suffix.len() >= 2 {
        suffix = suffix[1..suffix.len() - 1].to_string();
    }

    if !prefix.is_empty() && !suffix.is_empty() {
        return Err(format!(
            "'{}' has leftover text after the amount ('{suffix}')",
            raw.trim()
        ));
    }

    let (commodity, placement) = if !prefix.is_empty() {
        (prefix, Placement::Prefix)
    } else if !suffix.is_empty() {
        (suffix, Placement::Suffix)
    } else if let Some((c, p)) = default_commodity {
        (c.clone(), *p)
    } else {
        (String::new(), Placement::Suffix)
    };

    Ok(Amount {
        commodity,
        qty,
        prec,
        placement,
    })
}

// ---------------------------------------------------------------------------
// dates
// ---------------------------------------------------------------------------

/// Normalise a journal date to `YYYY-MM-DD` so plain string comparison orders
/// and ranges correctly. `default_year` fills in a yearless `MM/DD` date.
fn parse_date(raw: &str, default_year: Option<i32>) -> Result<String, String> {
    let s = raw.trim();
    let parts: Vec<&str> = s.split(['-', '/', '.']).collect();
    let nums: Result<Vec<i64>, String> = parts
        .iter()
        .map(|p| {
            p.trim()
                .parse::<i64>()
                .map_err(|_| format!("'{s}' is not a date (expected YYYY-MM-DD)"))
        })
        .collect();
    let nums = nums?;
    let (y, m, d) = match nums.len() {
        3 => (nums[0], nums[1], nums[2]),
        2 => match default_year {
            Some(y) => (y as i64, nums[0], nums[1]),
            None => {
                return Err(format!(
                    "'{s}' has no year — add one, or declare a default with a 'Y 2024' line"
                ))
            }
        },
        _ => return Err(format!("'{s}' is not a date (expected YYYY-MM-DD)")),
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || !(1..=9999).contains(&y) {
        return Err(format!("'{s}' is not a valid date"));
    }
    Ok(format!("{y:04}-{m:02}-{d:02}"))
}

// ---------------------------------------------------------------------------
// journal parsing
// ---------------------------------------------------------------------------

struct Parsed {
    txns: Vec<Txn>,
    styles: BTreeMap<String, Style>,
    declared: BTreeSet<String>,
    notes: Vec<String>,
}

/// Split a posting line into `(account, rest)` at the first two-space run or tab.
fn split_account(line: &str) -> (String, String) {
    let bytes: Vec<char> = line.chars().collect();
    for (i, c) in bytes.iter().enumerate() {
        if *c == '\t' {
            return (
                bytes[..i].iter().collect::<String>().trim().to_string(),
                bytes[i + 1..].iter().collect::<String>().trim().to_string(),
            );
        }
        if *c == ' ' && bytes.get(i + 1) == Some(&' ') {
            return (
                bytes[..i].iter().collect::<String>().trim().to_string(),
                bytes[i..].iter().collect::<String>().trim().to_string(),
            );
        }
    }
    (line.trim().to_string(), String::new())
}

/// Strip an end-of-line comment (`;`), returning the code part.
fn strip_comment(line: &str) -> &str {
    match line.find(';') {
        Some(i) => &line[..i],
        None => line,
    }
}

fn record_style(styles: &mut BTreeMap<String, Style>, a: &Amount) {
    let e = styles.entry(a.commodity.clone()).or_insert(Style {
        placement: a.placement,
        prec: a.prec,
    });
    if a.prec > e.prec {
        e.prec = a.prec;
    }
}

fn apply_aliases(account: &str, aliases: &[(String, String)]) -> String {
    let mut out = account.to_string();
    for (from, to) in aliases {
        if out == *from {
            out = to.clone();
        } else if let Some(rest) = out.strip_prefix(&format!("{from}:")) {
            out = format!("{to}:{rest}");
        }
    }
    out
}

fn parse_journal(journal: &str) -> Result<Parsed, String> {
    let mut txns: Vec<Txn> = Vec::new();
    let mut styles: BTreeMap<String, Style> = BTreeMap::new();
    let mut declared: BTreeSet<String> = BTreeSet::new();
    let mut notes: Vec<String> = Vec::new();
    let mut aliases: Vec<(String, String)> = Vec::new();
    let mut apply_prefix: Vec<String> = Vec::new();
    let mut default_commodity: Option<(String, Placement)> = None;
    let mut default_year: Option<i32> = None;
    let mut in_comment_block = false;
    let mut current: Option<Txn> = None;
    let mut include_skipped = 0usize;

    for (idx, raw_line) in journal.lines().enumerate() {
        let lineno = idx + 1;
        let line = raw_line.trim_end();

        if in_comment_block {
            if line.trim_start().starts_with("end comment") {
                in_comment_block = false;
            }
            continue;
        }

        let indented = line.starts_with(' ') || line.starts_with('\t');
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if let Some(t) = current.take() {
                txns.push(finish_txn(t, lineno, &mut styles)?);
            }
            continue;
        }

        // Whole-line comments (ledger accepts ; # % | * at column 0).
        if !indented && matches!(trimmed.chars().next(), Some(';' | '#' | '%' | '|' | '*')) {
            continue;
        }
        if indented && trimmed.starts_with(';') {
            continue; // transaction / posting comment
        }

        if indented {
            let Some(txn) = current.as_mut() else {
                return Err(format!(
                    "line {lineno}: indented posting '{trimmed}' is not inside a transaction"
                ));
            };
            let code = strip_comment(trimmed).trim();
            if code.is_empty() {
                continue;
            }
            // Optional posting status flag.
            let code = match code.strip_prefix("* ").or_else(|| code.strip_prefix("! ")) {
                Some(rest) => rest.trim_start(),
                None => code,
            };
            let (mut account, rest) = split_account(code);
            if account.is_empty() {
                return Err(format!("line {lineno}: posting has no account name"));
            }
            let mut virtual_posting = false;
            if (account.starts_with('(') && account.ends_with(')'))
                || (account.starts_with('[') && account.ends_with(']'))
            {
                virtual_posting = true;
                account = account[1..account.len() - 1].trim().to_string();
            }
            if !apply_prefix.is_empty() {
                account = format!("{}:{}", apply_prefix.join(":"), account);
            }
            account = apply_aliases(&account, &aliases);

            // Split off a balance assertion (`= AMT` / `== AMT`) — parsed so it
            // can never be mistaken for part of the amount, then discarded.
            let expr = match rest.find('=') {
                Some(i) => rest[..i].trim(),
                None => rest.trim(),
            };
            // Split off a price annotation.
            let (amt_str, price_str, price_total) = if let Some(i) = expr.find("@@") {
                (expr[..i].trim(), expr[i + 2..].trim(), true)
            } else if let Some(i) = expr.find('@') {
                (expr[..i].trim(), expr[i + 1..].trim(), false)
            } else {
                (expr, "", false)
            };

            let amount = if amt_str.is_empty() {
                None
            } else {
                Some(
                    parse_amount(amt_str, default_commodity.as_ref())
                        .map_err(|e| format!("line {lineno}: {e}"))?,
                )
            };
            let cost = match (&amount, price_str.is_empty()) {
                (Some(a), false) => {
                    let p = parse_amount(price_str, default_commodity.as_ref())
                        .map_err(|e| format!("line {lineno}: price {e}"))?;
                    record_style(&mut styles, &p);
                    let qty = if price_total {
                        if a.qty < 0 {
                            -p.qty.abs()
                        } else {
                            p.qty.abs()
                        }
                    } else {
                        a.qty.saturating_mul(p.qty) / SCALE
                    };
                    Some(Amount {
                        commodity: p.commodity,
                        qty,
                        prec: p.prec.max(2),
                        placement: p.placement,
                    })
                }
                _ => None,
            };
            if let Some(a) = &amount {
                record_style(&mut styles, a);
            }
            if let Some(c) = &cost {
                record_style(&mut styles, c);
            }
            txn.postings.push(Posting {
                account,
                virtual_posting,
                amount,
                cost,
            });
            continue;
        }

        // Un-indented: a new transaction closes the previous one.
        if let Some(t) = current.take() {
            txns.push(finish_txn(t, lineno, &mut styles)?);
        }

        let body = trimmed.trim_start_matches(['!', '@']).trim_start();
        let (word, args) = match body.split_once(char::is_whitespace) {
            Some((w, a)) => (w, a.trim()),
            None => (body, ""),
        };

        if word.starts_with(|c: char| c.is_ascii_digit()) {
            // Transaction header: DATE[=DATE2] [STATUS] [(CODE)] [DESCRIPTION]
            let head = strip_comment(trimmed).trim();
            let (date_field, rest) = match head.split_once(char::is_whitespace) {
                Some((d, r)) => (d, r.trim()),
                None => (head, ""),
            };
            let primary = date_field.split('=').next().unwrap_or(date_field);
            let date =
                parse_date(primary, default_year).map_err(|e| format!("line {lineno}: {e}"))?;
            let status = if let Some(r) = rest.strip_prefix('*') {
                let _ = r;
                Status::Cleared
            } else if rest.starts_with('!') {
                Status::Pending
            } else {
                Status::Unmarked
            };
            if txns.len() >= MAX_TRANSACTIONS {
                return Err(format!(
                    "journal has more than {MAX_TRANSACTIONS} transactions — split it into smaller files and total the reports"
                ));
            }
            current = Some(Txn {
                date,
                status,
                postings: Vec::new(),
            });
            continue;
        }

        match word {
            "comment" => in_comment_block = true,
            "end" => {
                // `end comment` (handled above) or `end apply account`
                if args.starts_with("apply") {
                    apply_prefix.pop();
                }
            }
            "apply" => {
                if let Some(acct) = args.strip_prefix("account") {
                    apply_prefix.push(acct.trim().to_string());
                }
            }
            "account" => {
                if !args.is_empty() {
                    declared.insert(apply_aliases(strip_comment(args).trim(), &aliases));
                }
            }
            "alias" => {
                if let Some((from, to)) = args.split_once('=') {
                    aliases.push((from.trim().to_string(), to.trim().to_string()));
                }
            }
            "commodity" => {
                if let Ok(a) = parse_amount(strip_comment(args).trim(), None) {
                    if !a.commodity.is_empty() {
                        styles.insert(
                            a.commodity.clone(),
                            Style {
                                placement: a.placement,
                                prec: a.prec,
                            },
                        );
                    }
                }
            }
            "D" => {
                if let Ok(a) = parse_amount(strip_comment(args).trim(), None) {
                    if !a.commodity.is_empty() {
                        default_commodity = Some((a.commodity.clone(), a.placement));
                        styles.insert(
                            a.commodity.clone(),
                            Style {
                                placement: a.placement,
                                prec: a.prec,
                            },
                        );
                    }
                }
            }
            "Y" | "year" => {
                if let Ok(y) = args.trim().parse::<i32>() {
                    default_year = Some(y);
                }
            }
            "P" | "N" | "C" | "bucket" | "tag" | "payee" | "define" | "assert" | "check" => {}
            "include" => include_skipped += 1,
            other => {
                notes.push(format!(
                    "line {lineno}: ignored unknown directive '{other}'"
                ));
            }
        }
    }

    if in_comment_block {
        return Err("a 'comment' block was never closed with 'end comment'".to_string());
    }
    if let Some(t) = current.take() {
        let n = journal.lines().count() + 1;
        txns.push(finish_txn(t, n, &mut styles)?);
    }
    if include_skipped > 0 {
        notes.push(format!(
            "skipped {include_skipped} 'include' line(s) — there is no filesystem here, so paste the included journals in instead"
        ));
    }
    if txns.is_empty() {
        return Err("no transactions found — paste a ledger/hledger journal with at least one dated transaction".to_string());
    }
    Ok(Parsed {
        txns,
        styles,
        declared,
        notes,
    })
}

/// Fill in the single amount-less posting of a transaction, if any.
fn finish_txn(
    mut t: Txn,
    lineno: usize,
    styles: &mut BTreeMap<String, Style>,
) -> Result<Txn, String> {
    if t.postings.is_empty() {
        return Err(format!(
            "transaction dated {} (before line {lineno}) has no postings",
            t.date
        ));
    }
    let blanks: Vec<usize> = t
        .postings
        .iter()
        .enumerate()
        .filter(|(_, p)| p.amount.is_none())
        .map(|(i, _)| i)
        .collect();
    if blanks.len() > 1 {
        return Err(format!(
            "transaction dated {} (before line {lineno}) leaves {} postings without an amount — at most one can be inferred",
            t.date,
            blanks.len()
        ));
    }
    if let Some(&i) = blanks.first() {
        if t.postings[i].virtual_posting {
            return Err(format!(
                "transaction dated {} (before line {lineno}): a virtual posting must state its own amount",
                t.date
            ));
        }
        let mut sums: BTreeMap<String, (i128, usize, Placement)> = BTreeMap::new();
        for (j, p) in t.postings.iter().enumerate() {
            if j == i || p.virtual_posting {
                continue;
            }
            let a = p.cost.as_ref().or(p.amount.as_ref());
            if let Some(a) = a {
                let e = sums
                    .entry(a.commodity.clone())
                    .or_insert((0, a.prec, a.placement));
                e.0 += a.qty;
                if a.prec > e.1 {
                    e.1 = a.prec;
                }
            }
        }
        let inferred: Vec<Amount> = sums
            .into_iter()
            .filter(|(_, (q, _, _))| *q != 0)
            .map(|(c, (q, prec, placement))| Amount {
                commodity: c,
                qty: -q,
                prec,
                placement,
            })
            .collect();
        if inferred.is_empty() {
            t.postings[i].amount = None;
            t.postings.remove(i);
        } else {
            for a in &inferred {
                record_style(styles, a);
            }
            let base = t.postings[i].clone();
            t.postings.remove(i);
            for (k, a) in inferred.into_iter().enumerate() {
                t.postings.insert(
                    i + k,
                    Posting {
                        account: base.account.clone(),
                        virtual_posting: false,
                        amount: Some(a),
                        cost: None,
                    },
                );
            }
        }
    }
    Ok(t)
}

// ---------------------------------------------------------------------------
// formatting helpers
// ---------------------------------------------------------------------------

fn format_qty(qty: i128, prec: usize) -> String {
    let prec = prec.min(MAX_PREC);
    let neg = qty < 0;
    let factor = 10i128.pow((MAX_PREC - prec) as u32);
    let mut v = qty.abs();
    v = (v + factor / 2) / factor;
    let unit = 10i128.pow(prec as u32);
    let int_part = v / unit;
    let frac = v % unit;
    let mut s = if prec == 0 {
        format!("{int_part}")
    } else {
        format!("{int_part}.{frac:0width$}", width = prec)
    };
    if neg && !(int_part == 0 && frac == 0) {
        s = format!("-{s}");
    }
    s
}

fn format_amount(commodity: &str, qty: i128, style: Style) -> String {
    let num = format_qty(qty, style.prec);
    if commodity.is_empty() {
        return num;
    }
    match style.placement {
        Placement::Prefix => {
            // Ledger prints the sign after the symbol: $-4.50
            match num.strip_prefix('-') {
                Some(rest) => format!("{commodity}-{rest}"),
                None => format!("{commodity}{num}"),
            }
        }
        Placement::Suffix => format!("{num} {commodity}"),
    }
}

// ---------------------------------------------------------------------------
// report
// ---------------------------------------------------------------------------

type Totals = BTreeMap<String, i128>;

struct Row {
    account: String,
    label: String,
    level: usize,
    totals: Totals,
    percent: Option<f64>,
}

fn fold_depth(account: &str, depth: usize) -> String {
    if depth == 0 {
        return account.to_string();
    }
    account.split(':').take(depth).collect::<Vec<_>>().join(":")
}

fn nonzero(t: &Totals) -> bool {
    t.values().any(|v| *v != 0)
}

fn add_into(dst: &mut Totals, src: &Totals) {
    for (c, v) in src {
        *dst.entry(c.clone()).or_insert(0) += v;
    }
}

fn matches_filter(account: &str, includes: &[String], excludes: &[String]) -> bool {
    let lower = account.to_lowercase();
    if excludes.iter().any(|p| lower.contains(p)) {
        return false;
    }
    includes.is_empty() || includes.iter().any(|p| lower.contains(p))
}

#[allow(clippy::too_many_arguments)]
fn build_rows(
    own: &BTreeMap<String, Totals>,
    declared: &BTreeSet<String>,
    tree: bool,
    include_empty: bool,
    sort_key: &str,
    primary: &str,
) -> Vec<Row> {
    let mut accounts: BTreeSet<String> = own.keys().cloned().collect();
    if include_empty {
        for d in declared {
            accounts.insert(d.clone());
        }
    }

    // Inclusive (subtree) totals for every node, including implied parents.
    let mut nodes: BTreeSet<String> = BTreeSet::new();
    for a in &accounts {
        let parts: Vec<&str> = a.split(':').collect();
        for i in 1..=parts.len() {
            nodes.insert(parts[..i].join(":"));
        }
    }
    let mut inclusive: BTreeMap<String, Totals> = BTreeMap::new();
    for node in &nodes {
        let mut t: Totals = BTreeMap::new();
        for (a, v) in own {
            if a == node || a.starts_with(&format!("{node}:")) {
                add_into(&mut t, v);
            }
        }
        inclusive.insert(node.clone(), t);
    }
    let subtree_has_activity = |node: &str| -> bool {
        own.iter()
            .any(|(a, v)| (a == node || a.starts_with(&format!("{node}:"))) && nonzero(v))
    };

    let cmp_amount = |a: &Totals, b: &Totals, asc: bool| -> std::cmp::Ordering {
        let va = *a.get(primary).unwrap_or(&0);
        let vb = *b.get(primary).unwrap_or(&0);
        let ord = va.cmp(&vb);
        if asc {
            ord
        } else {
            ord.reverse()
        }
    };

    let mut rows: Vec<Row> = Vec::new();

    if tree {
        // Depth-first walk over the account hierarchy.
        fn children(nodes: &BTreeSet<String>, parent: &str) -> Vec<String> {
            let prefix = if parent.is_empty() {
                String::new()
            } else {
                format!("{parent}:")
            };
            let mut out: BTreeSet<String> = BTreeSet::new();
            for n in nodes {
                if parent.is_empty() {
                    if !n.contains(':') {
                        out.insert(n.clone());
                    }
                } else if let Some(rest) = n.strip_prefix(&prefix) {
                    if !rest.contains(':') {
                        out.insert(n.clone());
                    }
                }
            }
            out.into_iter().collect()
        }
        #[allow(clippy::too_many_arguments)]
        fn walk(
            nodes: &BTreeSet<String>,
            inclusive: &BTreeMap<String, Totals>,
            parent: &str,
            level: usize,
            keep: &dyn Fn(&str) -> bool,
            order: &dyn Fn(&mut Vec<String>),
            rows: &mut Vec<Row>,
        ) {
            let mut kids = children(nodes, parent);
            kids.retain(|k| keep(k));
            order(&mut kids);
            for k in kids {
                let label = k.rsplit(':').next().unwrap_or(&k).to_string();
                rows.push(Row {
                    account: k.clone(),
                    label,
                    level,
                    totals: inclusive.get(&k).cloned().unwrap_or_default(),
                    percent: None,
                });
                walk(nodes, inclusive, &k, level + 1, keep, order, rows);
            }
        }
        let keep = |n: &str| include_empty || subtree_has_activity(n);
        let inc_ref = &inclusive;
        let order: Box<dyn Fn(&mut Vec<String>)> = match sort_key {
            "amount" => Box::new(move |v: &mut Vec<String>| {
                v.sort_by(|a, b| {
                    cmp_amount(
                        inc_ref.get(a).unwrap_or(&BTreeMap::new()),
                        inc_ref.get(b).unwrap_or(&BTreeMap::new()),
                        false,
                    )
                    .then_with(|| a.cmp(b))
                })
            }),
            "amount-asc" => Box::new(move |v: &mut Vec<String>| {
                v.sort_by(|a, b| {
                    cmp_amount(
                        inc_ref.get(a).unwrap_or(&BTreeMap::new()),
                        inc_ref.get(b).unwrap_or(&BTreeMap::new()),
                        true,
                    )
                    .then_with(|| a.cmp(b))
                })
            }),
            _ => Box::new(|_v: &mut Vec<String>| {}),
        };
        walk(&nodes, &inclusive, "", 0, &keep, &order, &mut rows);
    } else {
        let mut flat: Vec<String> = accounts
            .iter()
            .filter(|a| include_empty || own.get(*a).map(nonzero).unwrap_or(false))
            .cloned()
            .collect();
        match sort_key {
            "amount" | "amount-asc" => {
                let asc = sort_key == "amount-asc";
                let empty: Totals = BTreeMap::new();
                flat.sort_by(|a, b| {
                    cmp_amount(
                        own.get(a).unwrap_or(&empty),
                        own.get(b).unwrap_or(&empty),
                        asc,
                    )
                    .then_with(|| a.cmp(b))
                });
            }
            _ => flat.sort(),
        }
        for a in flat {
            rows.push(Row {
                account: a.clone(),
                label: a.clone(),
                level: 0,
                totals: own.get(&a).cloned().unwrap_or_default(),
                percent: None,
            });
        }
    }

    // Percentages are relative to the row's top-level ancestor, so a balanced
    // journal (whose grand total is zero) still yields useful numbers.
    for r in rows.iter_mut() {
        let root = r.account.split(':').next().unwrap_or("").to_string();
        let base = inclusive
            .get(&root)
            .and_then(|t| t.get(primary))
            .copied()
            .unwrap_or(0);
        let mine = *r.totals.get(primary).unwrap_or(&0);
        r.percent = if base == 0 {
            None
        } else {
            Some(mine as f64 / base as f64 * 100.0)
        };
    }
    rows
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

/// Compute account balances from a ledger/hledger journal.
///
/// * `journal` — the pasted journal text.
/// * `account_filter` — comma-separated case-insensitive substrings; a pattern
///   prefixed `not:` or `-` excludes instead of including.
/// * `depth` — fold accounts deeper than this into their ancestor (0 = no limit).
/// * `layout` — `tree` or `flat`.
/// * `sort` — `account`, `amount` (largest first) or `amount-asc`.
/// * `begin` / `end` — inclusive start and EXCLUSIVE end date (`YYYY-MM-DD`).
/// * `status` — `all`, `cleared`, `pending` or `unmarked`.
/// * `include_empty` — keep zero-balance and declared-but-unused accounts.
/// * `real_only` — ignore virtual `(…)` / `[…]` postings.
/// * `cost_basis` — report `@` / `@@` priced postings in their cost commodity.
/// * `show_total` — print the grand-total row.
/// * `percent` — add a percentage-of-top-level-account column.
/// * `output_format` — `text`, `csv`, `json` or `markdown`.
#[allow(clippy::too_many_arguments)]
pub fn balances(
    journal: &str,
    account_filter: &str,
    depth: usize,
    layout: &str,
    sort: &str,
    begin: &str,
    end: &str,
    status: &str,
    include_empty: bool,
    real_only: bool,
    cost_basis: bool,
    show_total: bool,
    percent: bool,
    output_format: &str,
) -> Result<String, String> {
    if journal.trim().is_empty() {
        return Err("journal is empty — paste a ledger/hledger journal".to_string());
    }
    let layout = layout.trim().to_ascii_lowercase();
    let layout = if layout.is_empty() {
        "tree".to_string()
    } else {
        layout
    };
    if layout != "tree" && layout != "flat" {
        return Err(format!("unknown layout '{layout}' (use tree or flat)"));
    }
    let sort_key = sort.trim().to_ascii_lowercase();
    let sort_key = if sort_key.is_empty() {
        "account".to_string()
    } else {
        sort_key
    };
    if !matches!(sort_key.as_str(), "account" | "amount" | "amount-asc") {
        return Err(format!(
            "unknown sort '{sort_key}' (use account, amount or amount-asc)"
        ));
    }
    let status_key = status.trim().to_ascii_lowercase();
    let status_key = if status_key.is_empty() {
        "all".to_string()
    } else {
        status_key
    };
    if !matches!(
        status_key.as_str(),
        "all" | "cleared" | "pending" | "unmarked"
    ) {
        return Err(format!(
            "unknown status '{status_key}' (use all, cleared, pending or unmarked)"
        ));
    }
    let fmt = output_format.trim().to_ascii_lowercase();
    let fmt = if fmt.is_empty() {
        "text".to_string()
    } else {
        fmt
    };
    if !matches!(fmt.as_str(), "text" | "csv" | "json" | "markdown") {
        return Err(format!(
            "unknown output_format '{fmt}' (use text, csv, json or markdown)"
        ));
    }
    if depth > MAX_DEPTH {
        return Err(format!("depth must be between 0 and {MAX_DEPTH}"));
    }
    let begin_date = if begin.trim().is_empty() {
        None
    } else {
        Some(parse_date(begin, None).map_err(|e| format!("begin: {e}"))?)
    };
    let end_date = if end.trim().is_empty() {
        None
    } else {
        Some(parse_date(end, None).map_err(|e| format!("end: {e}"))?)
    };

    let mut includes: Vec<String> = Vec::new();
    let mut excludes: Vec<String> = Vec::new();
    for part in account_filter.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if let Some(rest) = p.strip_prefix("not:").or_else(|| p.strip_prefix('-')) {
            if !rest.trim().is_empty() {
                excludes.push(rest.trim().to_lowercase());
            }
        } else {
            includes.push(p.to_lowercase());
        }
    }

    let parsed = parse_journal(journal)?;
    let mut styles = parsed.styles;

    // Aggregate postings that survive the filters.
    let mut own: BTreeMap<String, Totals> = BTreeMap::new();
    let mut commodity_hits: BTreeMap<String, usize> = BTreeMap::new();
    let mut matched_txns = 0usize;
    let mut matched_postings = 0usize;
    for t in &parsed.txns {
        if let Some(b) = &begin_date {
            if t.date < *b {
                continue;
            }
        }
        if let Some(e) = &end_date {
            if t.date >= *e {
                continue;
            }
        }
        let ok_status = match status_key.as_str() {
            "cleared" => t.status == Status::Cleared,
            "pending" => t.status == Status::Pending,
            "unmarked" => t.status == Status::Unmarked,
            _ => true,
        };
        if !ok_status {
            continue;
        }
        let mut used = false;
        for p in &t.postings {
            if real_only && p.virtual_posting {
                continue;
            }
            if !matches_filter(&p.account, &includes, &excludes) {
                continue;
            }
            let amount = if cost_basis {
                p.cost.as_ref().or(p.amount.as_ref())
            } else {
                p.amount.as_ref()
            };
            let Some(a) = amount else { continue };
            let folded = fold_depth(&p.account, depth);
            *own.entry(folded)
                .or_default()
                .entry(a.commodity.clone())
                .or_insert(0) += a.qty;
            *commodity_hits.entry(a.commodity.clone()).or_insert(0) += 1;
            record_style(&mut styles, a);
            matched_postings += 1;
            used = true;
        }
        if used {
            matched_txns += 1;
        }
    }

    if own.is_empty() && !include_empty {
        return Err(
            "no postings matched — loosen the account filter, date range or status filter"
                .to_string(),
        );
    }

    let primary = commodity_hits
        .iter()
        .max_by_key(|(c, n)| (**n, std::cmp::Reverse((*c).clone())))
        .map(|(c, _)| c.clone())
        .unwrap_or_default();

    let declared: BTreeSet<String> = parsed
        .declared
        .iter()
        .filter(|a| matches_filter(a, &includes, &excludes))
        .map(|a| fold_depth(a, depth))
        .collect();

    let rows = build_rows(
        &own,
        &declared,
        layout == "tree",
        include_empty,
        &sort_key,
        &primary,
    );

    let mut grand: Totals = BTreeMap::new();
    for v in own.values() {
        add_into(&mut grand, v);
    }
    let style_of = |c: &str| -> Style {
        styles.get(c).copied().unwrap_or(Style {
            placement: Placement::Suffix,
            prec: 2,
        })
    };
    let fmt_totals = |t: &Totals| -> Vec<String> {
        let mut v: Vec<String> = t
            .iter()
            .filter(|(_, q)| **q != 0)
            .map(|(c, q)| format_amount(c, *q, style_of(c)))
            .collect();
        if v.is_empty() {
            v.push(format_amount(&primary, 0, style_of(&primary)));
        }
        v
    };

    let out = match fmt.as_str() {
        "text" => render_text(
            &rows,
            &grand,
            show_total,
            percent,
            &fmt_totals,
            &parsed.notes,
        ),
        "markdown" => render_markdown(&rows, &grand, show_total, percent, &fmt_totals),
        "csv" => render_csv(&rows, &grand, show_total, percent, &style_of),
        _ => render_json(
            &rows,
            &grand,
            show_total,
            percent,
            &style_of,
            matched_txns,
            matched_postings,
            &parsed.notes,
        ),
    };
    Ok(out)
}

fn render_text(
    rows: &[Row],
    grand: &Totals,
    show_total: bool,
    percent: bool,
    fmt_totals: &dyn Fn(&Totals) -> Vec<String>,
    notes: &[String],
) -> String {
    let mut width = 0usize;
    for r in rows {
        for s in fmt_totals(&r.totals) {
            width = width.max(s.chars().count());
        }
    }
    if show_total {
        for s in fmt_totals(grand) {
            width = width.max(s.chars().count());
        }
    }
    let mut lines: Vec<String> = Vec::new();
    for r in rows {
        let amts = fmt_totals(&r.totals);
        let last = amts.len() - 1;
        for (i, a) in amts.iter().enumerate() {
            let name = if i == last {
                format!("{}{}", "  ".repeat(r.level), r.label)
            } else {
                String::new()
            };
            let mut line = format!("{a:>width$}  {name}");
            if percent && i == last {
                let p = match r.percent {
                    Some(p) => format!("{p:.1}%"),
                    None => "-".to_string(),
                };
                line = format!("{line}  ({p})");
            }
            lines.push(line.trim_end().to_string());
        }
    }
    if show_total {
        lines.push("-".repeat(width));
        for a in fmt_totals(grand) {
            lines.push(format!("{a:>width$}"));
        }
    }
    let mut out = lines.join("\n");
    if !notes.is_empty() {
        out.push_str("\n\nNotes:\n");
        out.push_str(
            &notes
                .iter()
                .map(|n| format!("  - {n}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    out
}

fn render_markdown(
    rows: &[Row],
    grand: &Totals,
    show_total: bool,
    percent: bool,
    fmt_totals: &dyn Fn(&Totals) -> Vec<String>,
) -> String {
    let mut out = String::new();
    if percent {
        out.push_str("| Account | Balance | % of top account |\n|---|---:|---:|\n");
    } else {
        out.push_str("| Account | Balance |\n|---|---:|\n");
    }
    for r in rows {
        let name = format!("{}{}", "&nbsp;".repeat(r.level * 4), r.label);
        let amt = fmt_totals(&r.totals).join("<br>");
        if percent {
            let p = match r.percent {
                Some(p) => format!("{p:.1}%"),
                None => "-".to_string(),
            };
            out.push_str(&format!("| {name} | {amt} | {p} |\n"));
        } else {
            out.push_str(&format!("| {name} | {amt} |\n"));
        }
    }
    if show_total {
        let amt = fmt_totals(grand).join("<br>");
        if percent {
            out.push_str(&format!("| **Total** | **{amt}** | |\n"));
        } else {
            out.push_str(&format!("| **Total** | **{amt}** |\n"));
        }
    }
    out.trim_end().to_string()
}

fn render_csv(
    rows: &[Row],
    grand: &Totals,
    show_total: bool,
    percent: bool,
    style_of: &dyn Fn(&str) -> Style,
) -> String {
    let mut out = String::new();
    out.push_str(if percent {
        "account,commodity,amount,percent\n"
    } else {
        "account,commodity,amount\n"
    });
    let emit = |account: &str, totals: &Totals, pct: Option<f64>, out: &mut String| {
        let mut any = false;
        for (c, q) in totals.iter().filter(|(_, q)| **q != 0) {
            any = true;
            let st = style_of(c);
            let line = if percent {
                let p = pct.map(|p| format!("{p:.1}")).unwrap_or_default();
                format!(
                    "{},{},{},{}\n",
                    csv_field(account),
                    csv_field(c),
                    format_qty(*q, st.prec),
                    p
                )
            } else {
                format!(
                    "{},{},{}\n",
                    csv_field(account),
                    csv_field(c),
                    format_qty(*q, st.prec)
                )
            };
            out.push_str(&line);
        }
        if !any {
            let line = if percent {
                format!("{},,0,\n", csv_field(account))
            } else {
                format!("{},,0\n", csv_field(account))
            };
            out.push_str(&line);
        }
    };
    for r in rows {
        emit(&r.account, &r.totals, r.percent, &mut out);
    }
    if show_total {
        emit("TOTAL", grand, None, &mut out);
    }
    out.trim_end().to_string()
}

#[allow(clippy::too_many_arguments)]
fn render_json(
    rows: &[Row],
    grand: &Totals,
    show_total: bool,
    percent: bool,
    style_of: &dyn Fn(&str) -> Style,
    txns: usize,
    postings: usize,
    notes: &[String],
) -> String {
    let amounts_json = |t: &Totals| -> String {
        let items: Vec<String> = t
            .iter()
            .filter(|(_, q)| **q != 0)
            .map(|(c, q)| {
                let st = style_of(c);
                format!(
                    "{{\"commodity\":\"{}\",\"amount\":\"{}\"}}",
                    json_escape(c),
                    format_qty(*q, st.prec)
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    };
    let mut out = String::from("{\n  \"accounts\": [\n");
    let mut items: Vec<String> = Vec::new();
    for r in rows {
        let pct = if percent {
            match r.percent {
                Some(p) => format!(",\"percent\":{:.1}", p),
                None => ",\"percent\":null".to_string(),
            }
        } else {
            String::new()
        };
        items.push(format!(
            "    {{\"account\":\"{}\",\"level\":{},\"amounts\":{}{}}}",
            json_escape(&r.account),
            r.level,
            amounts_json(&r.totals),
            pct
        ));
    }
    out.push_str(&items.join(",\n"));
    out.push_str("\n  ]");
    if show_total {
        out.push_str(&format!(",\n  \"total\": {}", amounts_json(grand)));
    }
    out.push_str(&format!(
        ",\n  \"transactions\": {txns},\n  \"postings\": {postings}"
    ));
    if !notes.is_empty() {
        let ns: Vec<String> = notes
            .iter()
            .map(|n| format!("\"{}\"", json_escape(n)))
            .collect();
        out.push_str(&format!(",\n  \"notes\": [{}]", ns.join(",")));
    }
    out.push_str("\n}");
    out
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const JOURNAL: &str = "\
2024-01-05 * Groceries
    Expenses:Food:Groceries   $45.20
    Assets:Bank:Checking

2024-01-10 Salary
    Assets:Bank:Checking      $2,000.00
    Income:Salary            $-2,000.00

2024-02-01 ! Coffee
    Expenses:Food:Coffee      $4.80
    Assets:Bank:Checking     $-4.80
";

    fn run(journal: &str) -> String {
        balances(
            journal, "", 0, "tree", "account", "", "", "all", false, false, false, true, false,
            "text",
        )
        .unwrap()
    }

    #[test]
    fn tree_report_totals_every_account_and_sub_account() {
        let out = run(JOURNAL);
        let expected = concat!(
            " $1950.00  Assets\n",
            " $1950.00    Bank\n",
            " $1950.00      Checking\n",
            "   $50.00  Expenses\n",
            "   $50.00    Food\n",
            "    $4.80      Coffee\n",
            "   $45.20      Groceries\n",
            "$-2000.00  Income\n",
            "$-2000.00    Salary\n",
            "---------\n",
            "    $0.00"
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn flat_layout_prints_only_posted_accounts_with_full_names() {
        let out = balances(
            JOURNAL, "", 0, "flat", "account", "", "", "all", false, false, false, false, false,
            "text",
        )
        .unwrap();
        let expected = concat!(
            " $1950.00  Assets:Bank:Checking\n",
            "    $4.80  Expenses:Food:Coffee\n",
            "   $45.20  Expenses:Food:Groceries\n",
            "$-2000.00  Income:Salary"
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn depth_folds_deeper_accounts_into_their_ancestor() {
        let out = balances(
            JOURNAL, "", 1, "flat", "account", "", "", "all", false, false, false, false, false,
            "text",
        )
        .unwrap();
        assert_eq!(
            out,
            " $1950.00  Assets\n   $50.00  Expenses\n$-2000.00  Income"
        );
    }

    #[test]
    fn account_filter_includes_and_excludes() {
        let out = balances(
            JOURNAL,
            "expenses, not:coffee",
            0,
            "flat",
            "account",
            "",
            "",
            "all",
            false,
            false,
            false,
            false,
            false,
            "text",
        )
        .unwrap();
        assert_eq!(out, "$45.20  Expenses:Food:Groceries");
    }

    #[test]
    fn date_range_end_is_exclusive() {
        let out = balances(
            JOURNAL,
            "",
            0,
            "flat",
            "account",
            "2024-01-01",
            "2024-02-01",
            "all",
            false,
            false,
            false,
            false,
            false,
            "text",
        )
        .unwrap();
        assert!(out.contains("Expenses:Food:Groceries"));
        assert!(!out.contains("Coffee"), "February posting excluded: {out}");
    }

    #[test]
    fn status_filter_selects_cleared_only() {
        let out = balances(
            JOURNAL, "", 0, "flat", "account", "", "", "cleared", false, false, false, false,
            false, "text",
        )
        .unwrap();
        assert_eq!(
            out,
            "$-45.20  Assets:Bank:Checking\n $45.20  Expenses:Food:Groceries"
        );
    }

    #[test]
    fn inferred_posting_balances_the_transaction() {
        let out = balances(
            "2024-03-01 rent\n    Expenses:Rent  $1200\n    Assets:Cash\n",
            "",
            0,
            "flat",
            "account",
            "",
            "",
            "all",
            false,
            false,
            false,
            false,
            false,
            "text",
        )
        .unwrap();
        assert_eq!(out, "$-1200  Assets:Cash\n $1200  Expenses:Rent");
    }

    #[test]
    fn sort_by_amount_orders_largest_first() {
        let out = balances(
            JOURNAL, "", 0, "flat", "amount", "", "", "all", false, false, false, false, false,
            "text",
        )
        .unwrap();
        let first = out.lines().next().unwrap();
        assert!(first.contains("Assets:Bank:Checking"), "got {first}");
        assert!(out.lines().last().unwrap().contains("Income:Salary"));
    }

    #[test]
    fn virtual_postings_are_dropped_by_real_only() {
        let j = "2024-01-01 budget\n    Expenses:Food  $10.00\n    Assets:Cash  $-10.00\n    (Budget:Food)  $-10.00\n";
        let with = balances(
            j, "", 0, "flat", "account", "", "", "all", false, false, false, false, false, "text",
        )
        .unwrap();
        assert!(with.contains("Budget:Food"));
        let without = balances(
            j, "", 0, "flat", "account", "", "", "all", false, true, false, false, false, "text",
        )
        .unwrap();
        assert!(!without.contains("Budget:Food"), "got {without}");
    }

    #[test]
    fn cost_basis_converts_priced_postings() {
        let j = "2024-01-01 buy\n    Assets:Stocks   10 AAPL @ $50.00\n    Assets:Cash\n";
        let raw = balances(
            j, "", 0, "flat", "account", "", "", "all", false, false, false, false, false, "text",
        )
        .unwrap();
        assert!(raw.contains("10 AAPL"), "got {raw}");
        assert!(raw.contains("$-500.00"), "cash leg settles at cost: {raw}");
        let cost = balances(
            j, "", 0, "flat", "account", "", "", "all", false, false, true, false, false, "text",
        )
        .unwrap();
        assert_eq!(cost, "$-500.00  Assets:Cash\n $500.00  Assets:Stocks");
    }

    #[test]
    fn total_price_annotation_uses_the_stated_total() {
        let j = "2024-01-01 buy\n    Assets:Stocks   4 VTI @@ $800.00\n    Assets:Cash\n";
        let out = balances(
            j, "", 0, "flat", "account", "", "", "all", false, false, true, false, false, "text",
        )
        .unwrap();
        assert_eq!(out, "$-800.00  Assets:Cash\n $800.00  Assets:Stocks");
    }

    #[test]
    fn balance_assertion_is_ignored_not_added() {
        let j =
            "2024-01-01 open\n    Assets:Cash  $100.00 = $100.00\n    Equity:Opening  $-100.00\n";
        let out = balances(
            j, "", 0, "flat", "account", "", "", "all", false, false, false, false, false, "text",
        )
        .unwrap();
        assert_eq!(out, " $100.00  Assets:Cash\n$-100.00  Equity:Opening");
    }

    #[test]
    fn european_amounts_and_suffix_commodities_round_trip() {
        let j = "2024-01-01 shop\n    Expenses:Food  1.234,50 EUR\n    Assets:Cash\n";
        let out = balances(
            j, "", 0, "flat", "account", "", "", "all", false, false, false, false, false, "text",
        )
        .unwrap();
        assert_eq!(
            out,
            "-1234.50 EUR  Assets:Cash\n 1234.50 EUR  Expenses:Food"
        );
    }

    #[test]
    fn include_empty_keeps_zero_and_declared_accounts() {
        let j = "account Assets:Savings\n\n2024-01-01 wash\n    Assets:Cash  $10.00\n    Income:Gift  $-10.00\n\n2024-01-02 spend\n    Expenses:Fun  $10.00\n    Assets:Cash  $-10.00\n";
        let out = balances(
            j, "", 0, "flat", "account", "", "", "all", true, false, false, false, false, "text",
        )
        .unwrap();
        assert!(out.contains("Assets:Cash"), "zero account kept: {out}");
        assert!(
            out.contains("Assets:Savings"),
            "declared account kept: {out}"
        );
    }

    #[test]
    fn csv_output_has_a_header_and_a_total_row() {
        let out = balances(
            JOURNAL, "", 1, "flat", "account", "", "", "all", false, false, false, true, false,
            "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "account,commodity,amount\nAssets,$,1950.00\nExpenses,$,50.00\nIncome,$,-2000.00\nTOTAL,,0"
        );
    }

    #[test]
    fn json_output_is_parseable_shaped_data() {
        let out = balances(
            JOURNAL, "", 1, "flat", "account", "", "", "all", false, false, false, true, false,
            "json",
        )
        .unwrap();
        assert!(out.starts_with('{'));
        assert!(out.contains("\"account\":\"Assets\""));
        assert!(out.contains("\"amount\":\"1950.00\""));
        assert!(out.contains("\"transactions\": 3"));
    }

    #[test]
    fn markdown_output_is_a_table() {
        let out = balances(
            JOURNAL, "", 1, "flat", "account", "", "", "all", false, false, false, true, false,
            "markdown",
        )
        .unwrap();
        assert!(out.starts_with("| Account | Balance |"));
        assert!(out.contains("| **Total** |"));
    }

    #[test]
    fn percent_column_is_relative_to_the_top_level_account() {
        let out = balances(
            JOURNAL, "expenses", 0, "tree", "account", "", "", "all", false, false, false, false,
            true, "text",
        )
        .unwrap();
        assert!(
            out.lines().next().unwrap().ends_with("(100.0%)"),
            "got {out}"
        );
        assert!(out.contains("(9.6%)"), "coffee share of expenses: {out}");
    }

    #[test]
    fn directives_alias_apply_and_comment_block_are_honoured() {
        let j = "comment\n2024-01-01 ignored\n    A  $1.00\n    B  $-1.00\nend comment\n\nalias Bank = Assets:Bank\napply account Personal\n\n2024-01-02 pay\n    Bank:Checking  $-20.00\n    Expenses:Fun  $20.00\nend apply account\n";
        let out = balances(
            j, "", 0, "flat", "account", "", "", "all", false, false, false, false, false, "text",
        )
        .unwrap();
        assert_eq!(
            out,
            "$-20.00  Personal:Bank:Checking\n $20.00  Personal:Expenses:Fun"
        );
    }

    #[test]
    fn yearless_dates_use_the_y_directive() {
        let j = "Y 2023\n\n01/15 lunch\n    Expenses:Food  $12.00\n    Assets:Cash\n";
        let out = balances(
            j,
            "",
            0,
            "flat",
            "account",
            "2023-01-01",
            "2023-02-01",
            "all",
            false,
            false,
            false,
            false,
            false,
            "text",
        )
        .unwrap();
        assert_eq!(out, "$-12.00  Assets:Cash\n $12.00  Expenses:Food");
    }

    // --- error paths -------------------------------------------------------

    #[test]
    fn empty_journal_is_an_error() {
        let err = balances(
            "   ", "", 0, "tree", "account", "", "", "all", false, false, false, true, false,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("empty"), "got {err}");
    }

    #[test]
    fn text_with_no_transactions_is_an_error() {
        let err = balances(
            "hello world\n",
            "",
            0,
            "tree",
            "account",
            "",
            "",
            "all",
            false,
            false,
            false,
            true,
            false,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("no transactions found"), "got {err}");
    }

    #[test]
    fn two_amountless_postings_cannot_be_inferred() {
        let err = balances(
            "2024-01-01 x\n    A  $1.00\n    B\n    C\n",
            "",
            0,
            "tree",
            "account",
            "",
            "",
            "all",
            false,
            false,
            false,
            true,
            false,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("at most one can be inferred"), "got {err}");
    }

    #[test]
    fn unknown_enum_values_are_rejected() {
        for (arg, needle) in [("nope", "unknown layout")] {
            let err = balances(
                JOURNAL, "", 0, arg, "account", "", "", "all", false, false, false, true, false,
                "text",
            )
            .unwrap_err();
            assert!(err.contains(needle), "got {err}");
        }
        let err = balances(
            JOURNAL, "", 0, "tree", "account", "", "", "all", false, false, false, true, false,
            "xml",
        )
        .unwrap_err();
        assert!(err.contains("unknown output_format"), "got {err}");
    }

    #[test]
    fn a_bad_amount_reports_its_line_number() {
        let err = balances(
            "2024-01-01 x\n    A  $12x.00\n    B  $-12.00\n",
            "",
            0,
            "tree",
            "account",
            "",
            "",
            "all",
            false,
            false,
            false,
            true,
            false,
            "text",
        )
        .unwrap_err();
        assert!(err.starts_with("line 2:"), "got {err}");
    }

    #[test]
    fn a_filter_matching_nothing_is_an_actionable_error() {
        let err = balances(
            JOURNAL,
            "nosuchaccount",
            0,
            "tree",
            "account",
            "",
            "",
            "all",
            false,
            false,
            false,
            true,
            false,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("no postings matched"), "got {err}");
    }

    #[test]
    fn too_many_transactions_is_rejected_at_the_cap() {
        let mut j = String::new();
        for i in 0..(MAX_TRANSACTIONS + 1) {
            j.push_str(&format!(
                "2024-01-01 t{i}\n    Expenses:Fun  $1.00\n    Assets:Cash  $-1.00\n\n"
            ));
        }
        let err = balances(
            &j, "", 0, "tree", "account", "", "", "all", false, false, false, true, false, "text",
        )
        .unwrap_err();
        assert!(err.contains("more than 5000 transactions"), "got {err}");
    }

    #[test]
    fn exactly_the_cap_still_works() {
        let mut j = String::new();
        for i in 0..MAX_TRANSACTIONS {
            j.push_str(&format!(
                "2024-01-01 t{i}\n    Expenses:Fun  $1.00\n    Assets:Cash  $-1.00\n\n"
            ));
        }
        let out = balances(
            &j, "", 1, "flat", "account", "", "", "all", false, false, false, false, false, "text",
        )
        .unwrap();
        assert_eq!(out, "$-5000.00  Assets\n $5000.00  Expenses");
    }

    #[test]
    fn number_parsing_handles_separator_styles() {
        assert_eq!(parse_number("1,234.56").unwrap(), (123456000000, 2));
        assert_eq!(parse_number("1.234,56").unwrap(), (123456000000, 2));
        assert_eq!(parse_number("1,23").unwrap(), (123000000, 2));
        assert_eq!(parse_number("1,234").unwrap(), (123400000000, 0));
        assert_eq!(parse_number("1.234").unwrap(), (123400000, 3));
        assert!(parse_number("12x").is_err());
    }
}
