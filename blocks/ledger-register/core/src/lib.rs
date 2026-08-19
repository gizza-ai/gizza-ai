//! ledger-register core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Parses a ledger-cli / hledger
//! plain-text journal and prints the **register**: one line per matching
//! posting, in date order, with a running total in the last column — the
//! checkbook view of a set of accounts.
//!
//! The parser understands the common journal surface: transaction headers with
//! an optional status flag / `(code)` / description, indented postings
//! separated from their amount by two-or-more spaces (or a tab), virtual
//! postings in `(…)` / `[…]`, `@` / `@@` prices, `= AMOUNT` balance assertions
//! (parsed, never counted as an amount), a single amount-less posting per
//! transaction (inferred), and the `account` / `alias` / `commodity` / `D` /
//! `Y` / `P` / `apply account` / `comment` directives. `include` has no
//! meaning without a filesystem, so those lines are skipped and reported as a
//! note.
//!
//! Amounts are held as fixed-point integers scaled by 1e8 so that a running
//! total over thousands of postings stays exact; each commodity is printed back
//! with the decimal precision it was written with (or the one a `commodity` /
//! `D` directive declared).

use std::collections::BTreeMap;

/// Hard cap on transactions parsed in one call, so a huge paste can't blow up
/// memory in the browser or the wasm sandbox. The chat/CLI schema advertises
/// this same bound.
pub const MAX_TRANSACTIONS: usize = 5_000;

/// Deepest account level the `depth` fold accepts. Real charts of accounts are
/// nowhere near this deep, and the page renders `depth` as a 0–10 slider.
pub const MAX_DEPTH: usize = 10;

/// Narrowest and widest total output width the text report accepts.
pub const MIN_WIDTH: usize = 40;
pub const MAX_WIDTH: usize = 400;

/// Most rows `limit` may ask for.
pub const MAX_LIMIT: usize = 10_000;

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
    description: String,
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

/// Pull the human description out of a transaction header, dropping the date,
/// the status flag and a `(code)`.
fn header_description(rest: &str) -> String {
    let mut s = rest.trim();
    if let Some(r) = s.strip_prefix('*').or_else(|| s.strip_prefix('!')) {
        s = r.trim_start();
    }
    if s.starts_with('(') {
        if let Some(i) = s.find(')') {
            s = s[i + 1..].trim_start();
        }
    }
    s.trim().to_string()
}

fn parse_journal(journal: &str) -> Result<Parsed, String> {
    let mut txns: Vec<Txn> = Vec::new();
    let mut styles: BTreeMap<String, Style> = BTreeMap::new();
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
            let status = if rest.starts_with('*') {
                Status::Cleared
            } else if rest.starts_with('!') {
                Status::Pending
            } else {
                Status::Unmarked
            };
            if txns.len() >= MAX_TRANSACTIONS {
                return Err(format!(
                    "journal has more than {MAX_TRANSACTIONS} transactions — split it into smaller files and register them separately"
                ));
            }
            current = Some(Txn {
                date,
                status,
                description: header_description(rest),
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
            "account" | "P" | "N" | "C" | "bucket" | "tag" | "payee" | "define" | "assert"
            | "check" => {}
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

/// Truncate a description on the right, marking the cut with `..`.
fn clip_right(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n <= width {
        return s.to_string();
    }
    if width <= 2 {
        return "..".chars().take(width).collect();
    }
    let keep: String = s.chars().take(width - 2).collect();
    format!("{}..", keep.trim_end())
}

/// Truncate an account name on the LEFT, so the leaf sub-account — the part
/// that identifies the posting — always survives, as the register CLIs do.
fn clip_account(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n <= width {
        return s.to_string();
    }
    if width <= 2 {
        return "..".chars().take(width).collect();
    }
    let tail: String = s.chars().skip(n - (width - 2)).collect();
    format!("..{tail}")
}

fn pad_right(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - n))
    }
}

fn pad_left(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(width - n))
    }
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
// filters
// ---------------------------------------------------------------------------

/// Split a comma-separated filter into (include, exclude) lowercase substrings.
/// A pattern prefixed `not:` or `-` excludes.
fn split_patterns(spec: &str) -> (Vec<String>, Vec<String>) {
    let mut includes: Vec<String> = Vec::new();
    let mut excludes: Vec<String> = Vec::new();
    for part in spec.split(',') {
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
    (includes, excludes)
}

fn matches_patterns(text: &str, includes: &[String], excludes: &[String]) -> bool {
    let lower = text.to_lowercase();
    if excludes.iter().any(|p| lower.contains(p)) {
        return false;
    }
    includes.is_empty() || includes.iter().any(|p| lower.contains(p))
}

fn fold_depth(account: &str, depth: usize) -> String {
    if depth == 0 {
        return account.to_string();
    }
    account.split(':').take(depth).collect::<Vec<_>>().join(":")
}

// ---------------------------------------------------------------------------
// rows
// ---------------------------------------------------------------------------

type Totals = BTreeMap<String, i128>;

struct Row {
    date: String,
    description: String,
    account: String,
    /// The posting amount, one entry per commodity (normally exactly one).
    amounts: Totals,
    /// Running total / average as of this row.
    total: Totals,
}

fn add_into(dst: &mut Totals, src: &Totals) {
    for (c, v) in src {
        *dst.entry(c.clone()).or_insert(0) += v;
    }
}

/// Which postings of a transaction get printed, given the account filter and
/// the `related` flag. Empty when the transaction doesn't touch the filter.
fn select_postings<'a>(
    t: &'a Txn,
    acct_in: &[String],
    acct_ex: &[String],
    real_only: bool,
    related: bool,
) -> Vec<&'a Posting> {
    let hit: Vec<&Posting> = t
        .postings
        .iter()
        .filter(|p| !(real_only && p.virtual_posting))
        .filter(|p| matches_patterns(&p.account, acct_in, acct_ex))
        .collect();
    if hit.is_empty() {
        return Vec::new();
    }
    if related {
        t.postings
            .iter()
            .filter(|p| !(real_only && p.virtual_posting))
            .filter(|p| !matches_patterns(&p.account, acct_in, acct_ex))
            .collect()
    } else {
        hit
    }
}

/// The amount a posting registers as: its cost when `cost_basis` is on and the
/// posting carried an `@` / `@@` price, otherwise the amount as written.
fn amount_for(p: &Posting, cost_basis: bool) -> Option<&Amount> {
    if cost_basis {
        p.cost.as_ref().or(p.amount.as_ref())
    } else {
        p.amount.as_ref()
    }
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

/// Build a register report from a ledger/hledger journal.
///
/// * `journal` — the pasted journal text.
/// * `account_filter` — comma-separated case-insensitive substrings; a pattern
///   prefixed `not:` or `-` excludes instead of including.
/// * `payee_filter` — the same syntax, matched against the transaction
///   description (payee).
/// * `begin` / `end` — inclusive start and EXCLUSIVE end date (`YYYY-MM-DD`).
/// * `status` — `all`, `cleared`, `pending` or `unmarked`.
/// * `depth` — fold account names to this many `:` levels (0 = full name).
/// * `running_total` — `period` (from the report start), `historical` (carry in
///   the balance from before `begin`), `average` (running average) or `none`.
/// * `related` — show the OTHER side of each matching transaction.
/// * `invert` — flip the sign of every amount and total.
/// * `real_only` — ignore virtual `(…)` / `[…]` postings.
/// * `cost_basis` — report `@` / `@@` priced postings in their cost commodity.
/// * `sort` — `date`, `date-desc`, `amount`, `amount-asc` or `account`.
/// * `limit` — keep at most this many rows (0 = all).
/// * `limit_from` — take the `first` or the `last` rows when limiting.
/// * `width` — total width of the text report, in columns.
/// * `output_format` — `text`, `csv`, `json` or `markdown`.
#[allow(clippy::too_many_arguments)]
pub fn register(
    journal: &str,
    account_filter: &str,
    payee_filter: &str,
    begin: &str,
    end: &str,
    status: &str,
    depth: usize,
    running_total: &str,
    related: bool,
    invert: bool,
    real_only: bool,
    cost_basis: bool,
    sort: &str,
    limit: usize,
    limit_from: &str,
    width: usize,
    output_format: &str,
) -> Result<String, String> {
    if journal.trim().is_empty() {
        return Err("journal is empty — paste a ledger/hledger journal".to_string());
    }

    let pick = |raw: &str, fallback: &str| -> String {
        let v = raw.trim().to_ascii_lowercase();
        if v.is_empty() {
            fallback.to_string()
        } else {
            v
        }
    };
    let status_key = pick(status, "all");
    if !matches!(
        status_key.as_str(),
        "all" | "cleared" | "pending" | "unmarked"
    ) {
        return Err(format!(
            "unknown status '{status_key}' (use all, cleared, pending or unmarked)"
        ));
    }
    let total_mode = pick(running_total, "period");
    if !matches!(
        total_mode.as_str(),
        "period" | "historical" | "average" | "none"
    ) {
        return Err(format!(
            "unknown running_total '{total_mode}' (use period, historical, average or none)"
        ));
    }
    let sort_key = pick(sort, "date");
    if !matches!(
        sort_key.as_str(),
        "date" | "date-desc" | "amount" | "amount-asc" | "account"
    ) {
        return Err(format!(
            "unknown sort '{sort_key}' (use date, date-desc, amount, amount-asc or account)"
        ));
    }
    let limit_side = pick(limit_from, "first");
    if !matches!(limit_side.as_str(), "first" | "last") {
        return Err(format!(
            "unknown limit_from '{limit_side}' (use first or last)"
        ));
    }
    let fmt = pick(output_format, "text");
    if !matches!(fmt.as_str(), "text" | "csv" | "json" | "markdown") {
        return Err(format!(
            "unknown output_format '{fmt}' (use text, csv, json or markdown)"
        ));
    }
    if depth > MAX_DEPTH {
        return Err(format!("depth must be between 0 and {MAX_DEPTH}"));
    }
    if limit > MAX_LIMIT {
        return Err(format!("limit must be between 0 and {MAX_LIMIT}"));
    }
    if width != 0 && !(MIN_WIDTH..=MAX_WIDTH).contains(&width) {
        return Err(format!(
            "width must be between {MIN_WIDTH} and {MAX_WIDTH} columns"
        ));
    }
    let width = if width == 0 { 80 } else { width };

    let (acct_in, acct_ex) = split_patterns(account_filter);
    let (payee_in, payee_ex) = split_patterns(payee_filter);
    if related && acct_in.is_empty() && acct_ex.is_empty() {
        return Err("the related view needs an account filter — it shows the OTHER side of the transactions that touch the accounts you name".to_string());
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

    let parsed = parse_journal(journal)?;
    let mut styles = parsed.styles;

    // Pass 1: rows inside the report period, plus the historical opening
    // balance from everything before it.
    let mut rows: Vec<Row> = Vec::new();
    let mut opening: Totals = BTreeMap::new();
    let mut commodity_hits: BTreeMap<String, usize> = BTreeMap::new();
    for t in &parsed.txns {
        let ok_status = match status_key.as_str() {
            "cleared" => t.status == Status::Cleared,
            "pending" => t.status == Status::Pending,
            "unmarked" => t.status == Status::Unmarked,
            _ => true,
        };
        if !ok_status {
            continue;
        }
        if !matches_patterns(&t.description, &payee_in, &payee_ex) {
            continue;
        }
        let before = begin_date.as_ref().map(|b| t.date < *b).unwrap_or(false);
        let after = end_date.as_ref().map(|e| t.date >= *e).unwrap_or(false);
        if after {
            continue;
        }
        for p in select_postings(t, &acct_in, &acct_ex, real_only, related) {
            let Some(a) = amount_for(p, cost_basis) else {
                continue;
            };
            let qty = if invert { -a.qty } else { a.qty };
            record_style(&mut styles, a);
            if before {
                *opening.entry(a.commodity.clone()).or_insert(0) += qty;
                continue;
            }
            let mut amounts: Totals = BTreeMap::new();
            amounts.insert(a.commodity.clone(), qty);
            *commodity_hits.entry(a.commodity.clone()).or_insert(0) += 1;
            rows.push(Row {
                date: t.date.clone(),
                description: t.description.clone(),
                account: fold_depth(&p.account, depth),
                amounts,
                total: BTreeMap::new(),
            });
        }
    }

    if rows.is_empty() {
        return Err(
            "no postings matched — loosen the account filter, payee filter, date range or status filter"
                .to_string(),
        );
    }

    // The commodity that amount-sorting ranks by: the one used most often.
    let primary = commodity_hits
        .iter()
        .max_by_key(|(c, n)| (**n, std::cmp::Reverse((*c).clone())))
        .map(|(c, _)| c.clone())
        .unwrap_or_default();

    // Sort. `date` keeps the journal's own order within a day (stable sort).
    match sort_key.as_str() {
        "date" => rows.sort_by(|a, b| a.date.cmp(&b.date)),
        "date-desc" => rows.sort_by(|a, b| b.date.cmp(&a.date)),
        "account" => rows.sort_by(|a, b| a.account.cmp(&b.account).then(a.date.cmp(&b.date))),
        "amount" | "amount-asc" => {
            let asc = sort_key == "amount-asc";
            rows.sort_by(|a, b| {
                let va = *a.amounts.get(&primary).unwrap_or(&0);
                let vb = *b.amounts.get(&primary).unwrap_or(&0);
                let ord = va.cmp(&vb);
                let ord = if asc { ord } else { ord.reverse() };
                ord.then(a.date.cmp(&b.date))
            });
        }
        _ => {}
    }

    // Running total / average, accumulated in the order the rows are printed.
    let mut acc: Totals = BTreeMap::new();
    if total_mode == "historical" {
        acc = opening.clone();
    }
    for (i, r) in rows.iter_mut().enumerate() {
        add_into(&mut acc, &r.amounts);
        r.total = match total_mode.as_str() {
            "none" => BTreeMap::new(),
            "average" => {
                let n = (i + 1) as i128;
                acc.iter().map(|(c, v)| (c.clone(), v / n)).collect()
            }
            _ => acc.clone(),
        };
    }

    // head / tail.
    if limit > 0 && rows.len() > limit {
        if limit_side == "last" {
            rows.drain(..rows.len() - limit);
        } else {
            rows.truncate(limit);
        }
    }

    let style_of = |c: &str| -> Style {
        styles.get(c).copied().unwrap_or(Style {
            placement: Placement::Suffix,
            prec: 2,
        })
    };
    let fmt_totals = |t: &Totals| -> Vec<String> {
        let v: Vec<String> = t
            .iter()
            .filter(|(_, q)| **q != 0)
            .map(|(c, q)| format_amount(c, *q, style_of(c)))
            .collect();
        if v.is_empty() {
            vec![format_amount(&primary, 0, style_of(&primary))]
        } else {
            v
        }
    };

    let show_total = total_mode != "none";
    let out = match fmt.as_str() {
        "text" => render_text(&rows, show_total, width, &fmt_totals, &parsed.notes),
        "markdown" => render_markdown(&rows, show_total, &fmt_totals),
        "csv" => render_csv(&rows, show_total, &style_of),
        _ => render_json(&rows, show_total, &style_of, &parsed.notes),
    };
    Ok(out)
}

fn render_text(
    rows: &[Row],
    show_total: bool,
    width: usize,
    fmt_totals: &dyn Fn(&Totals) -> Vec<String>,
    notes: &[String],
) -> String {
    // Column widths: the amount and total columns size to their content, the
    // description and account columns share whatever is left of `width`.
    let mut amt_w = 0usize;
    let mut tot_w = 0usize;
    for r in rows {
        for s in fmt_totals(&r.amounts) {
            amt_w = amt_w.max(s.chars().count());
        }
        if show_total {
            for s in fmt_totals(&r.total) {
                tot_w = tot_w.max(s.chars().count());
            }
        }
    }
    let fixed = 10 + 1 + 1 + 2 + amt_w + if show_total { 2 + tot_w } else { 0 };
    let rem = width.saturating_sub(fixed).max(24);
    let desc_w = (rem * 2 / 5).max(10);
    let acct_w = (rem - desc_w.min(rem)).max(12);

    let mut lines: Vec<String> = Vec::new();
    let mut prev_key: Option<(String, String)> = None;
    for r in rows {
        let amts = fmt_totals(&r.amounts);
        let tots = if show_total {
            fmt_totals(&r.total)
        } else {
            Vec::new()
        };
        let n = amts.len().max(tots.len().max(1));
        let key = (r.date.clone(), r.description.clone());
        let repeat = prev_key.as_ref() == Some(&key);
        prev_key = Some(key);
        for i in 0..n {
            let (date, desc) = if i == 0 && !repeat {
                (r.date.clone(), clip_right(&r.description, desc_w))
            } else {
                (String::new(), String::new())
            };
            let account = if i == 0 {
                clip_account(&r.account, acct_w)
            } else {
                String::new()
            };
            let mut line = format!(
                "{} {} {}  {}",
                pad_right(&date, 10),
                pad_right(&desc, desc_w),
                pad_right(&account, acct_w),
                pad_left(amts.get(i).map(String::as_str).unwrap_or(""), amt_w),
            );
            if show_total {
                line = format!(
                    "{line}  {}",
                    pad_left(tots.get(i).map(String::as_str).unwrap_or(""), tot_w)
                );
            }
            lines.push(line.trim_end().to_string());
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
    show_total: bool,
    fmt_totals: &dyn Fn(&Totals) -> Vec<String>,
) -> String {
    let mut out = String::new();
    if show_total {
        out.push_str(
            "| Date | Description | Account | Amount | Total |\n|---|---|---|---:|---:|\n",
        );
    } else {
        out.push_str("| Date | Description | Account | Amount |\n|---|---|---|---:|\n");
    }
    for r in rows {
        let amt = fmt_totals(&r.amounts).join("<br>");
        let desc = r.description.replace('|', "\\|");
        if show_total {
            let tot = fmt_totals(&r.total).join("<br>");
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                r.date, desc, r.account, amt, tot
            ));
        } else {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                r.date, desc, r.account, amt
            ));
        }
    }
    out.trim_end().to_string()
}

fn render_csv(rows: &[Row], show_total: bool, style_of: &dyn Fn(&str) -> Style) -> String {
    let mut out = String::new();
    out.push_str(if show_total {
        "date,description,account,commodity,amount,total\n"
    } else {
        "date,description,account,commodity,amount\n"
    });
    for r in rows {
        for (c, q) in r.amounts.iter() {
            let st = style_of(c);
            if show_total {
                let t = r.total.get(c).copied().unwrap_or(0);
                out.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    r.date,
                    csv_field(&r.description),
                    csv_field(&r.account),
                    csv_field(c),
                    format_qty(*q, st.prec),
                    format_qty(t, st.prec)
                ));
            } else {
                out.push_str(&format!(
                    "{},{},{},{},{}\n",
                    r.date,
                    csv_field(&r.description),
                    csv_field(&r.account),
                    csv_field(c),
                    format_qty(*q, st.prec)
                ));
            }
        }
    }
    out.trim_end().to_string()
}

fn render_json(
    rows: &[Row],
    show_total: bool,
    style_of: &dyn Fn(&str) -> Style,
    notes: &[String],
) -> String {
    let amounts_json = |t: &Totals| -> String {
        let items: Vec<String> = t
            .iter()
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
    let mut items: Vec<String> = Vec::new();
    for r in rows {
        let total = if show_total {
            format!(",\"total\":{}", amounts_json(&r.total))
        } else {
            String::new()
        };
        items.push(format!(
            "    {{\"date\":\"{}\",\"description\":\"{}\",\"account\":\"{}\",\"amounts\":{}{}}}",
            r.date,
            json_escape(&r.description),
            json_escape(&r.account),
            amounts_json(&r.amounts),
            total
        ));
    }
    let mut out = String::from("{\n  \"postings\": [\n");
    out.push_str(&items.join(",\n"));
    out.push_str("\n  ]");
    out.push_str(&format!(",\n  \"count\": {}", rows.len()));
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

    #[allow(clippy::too_many_arguments)]
    fn reg(
        journal: &str,
        account_filter: &str,
        payee_filter: &str,
        begin: &str,
        end: &str,
        status: &str,
        depth: usize,
        running_total: &str,
        related: bool,
        invert: bool,
        real_only: bool,
        cost_basis: bool,
        sort: &str,
        limit: usize,
        limit_from: &str,
        width: usize,
        output_format: &str,
    ) -> Result<String, String> {
        register(
            journal,
            account_filter,
            payee_filter,
            begin,
            end,
            status,
            depth,
            running_total,
            related,
            invert,
            real_only,
            cost_basis,
            sort,
            limit,
            limit_from,
            width,
            output_format,
        )
    }

    fn simple(journal: &str, account_filter: &str) -> String {
        reg(
            journal,
            account_filter,
            "",
            "",
            "",
            "all",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date",
            0,
            "first",
            80,
            "text",
        )
        .unwrap()
    }

    #[test]
    fn one_account_register_shows_a_running_balance() {
        let out = simple(JOURNAL, "checking");
        // Every line fills the requested 80 columns, as `ledger register -w 80`
        // does: date, description, account, amount, running total.
        let expected = concat!(
            "2024-01-05 Groceries           Assets:Bank:Checking            $-45.20   $-45.20\n",
            "2024-01-10 Salary              Assets:Bank:Checking           $2000.00  $1954.80\n",
            "2024-02-01 Coffee              Assets:Bank:Checking             $-4.80  $1950.00"
        );
        assert_eq!(out, expected);
        assert!(out.lines().all(|l| l.chars().count() == 80), "got {out}");
    }

    #[test]
    fn the_last_running_total_equals_the_account_balance() {
        let out = simple(JOURNAL, "checking");
        let last = out.lines().last().unwrap();
        assert!(last.ends_with("$1950.00"), "got {last}");
    }

    #[test]
    fn every_posting_is_listed_when_no_filter_is_given() {
        let out = simple(JOURNAL, "");
        assert_eq!(out.lines().count(), 6, "6 postings across 3 txns: {out}");
        // The date and description are printed once per transaction.
        assert!(out.lines().nth(1).unwrap().starts_with("           "));
    }

    #[test]
    fn historical_mode_carries_in_the_balance_from_before_the_start_date() {
        let period = reg(
            JOURNAL,
            "checking",
            "",
            "2024-02-01",
            "",
            "all",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date",
            0,
            "first",
            80,
            "text",
        )
        .unwrap();
        assert!(period.ends_with("$-4.80"), "got {period}");
        let hist = reg(
            JOURNAL,
            "checking",
            "",
            "2024-02-01",
            "",
            "all",
            0,
            "historical",
            false,
            false,
            false,
            false,
            "date",
            0,
            "first",
            80,
            "text",
        )
        .unwrap();
        assert!(hist.ends_with("$1950.00"), "got {hist}");
    }

    #[test]
    fn average_mode_divides_the_running_total_by_the_row_count() {
        let out = reg(
            JOURNAL, "food", "", "", "", "all", 0, "average", false, false, false, false, "date",
            0, "first", 80, "text",
        )
        .unwrap();
        // $45.20 then ($45.20 + $4.80) / 2 = $25.00
        assert!(out.lines().next().unwrap().ends_with("$45.20"), "got {out}");
        assert!(out.lines().last().unwrap().ends_with("$25.00"), "got {out}");
    }

    #[test]
    fn none_mode_drops_the_total_column_entirely() {
        let out = reg(
            JOURNAL, "checking", "", "", "", "all", 0, "none", false, false, false, false, "date",
            0, "first", 80, "text",
        )
        .unwrap();
        assert!(out.ends_with("$-4.80"), "no running total column: {out}");
    }

    #[test]
    fn related_shows_the_other_side_of_the_transaction() {
        let out = reg(
            JOURNAL, "checking", "", "", "", "all", 0, "period", true, false, false, false, "date",
            0, "first", 80, "text",
        )
        .unwrap();
        assert!(out.contains("Expenses:Food:Groceries"), "got {out}");
        assert!(out.contains("Income:Salary"), "got {out}");
        assert!(!out.contains("Assets:Bank:Checking"), "got {out}");
    }

    #[test]
    fn invert_flips_the_sign_of_amounts_and_totals() {
        let out = reg(
            JOURNAL, "checking", "", "", "", "all", 0, "period", false, true, false, false, "date",
            0, "first", 80, "text",
        )
        .unwrap();
        assert!(out.lines().next().unwrap().contains("$45.20"), "got {out}");
        assert!(out.ends_with("$-1950.00"), "got {out}");
    }

    #[test]
    fn payee_filter_selects_transactions_by_description() {
        let out = reg(
            JOURNAL, "", "coffee", "", "", "all", 0, "period", false, false, false, false, "date",
            0, "first", 80, "text",
        )
        .unwrap();
        assert_eq!(out.lines().count(), 2, "only the coffee txn: {out}");
        assert!(out.contains("Coffee"));
    }

    #[test]
    fn account_filter_excludes_with_a_not_prefix() {
        let out = simple(JOURNAL, "expenses, not:coffee");
        assert_eq!(out.lines().count(), 1);
        assert!(out.contains("Expenses:Food:Groceries"));
    }

    #[test]
    fn date_range_end_is_exclusive() {
        let out = reg(
            JOURNAL,
            "checking",
            "",
            "2024-01-01",
            "2024-02-01",
            "all",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date",
            0,
            "first",
            80,
            "text",
        )
        .unwrap();
        assert!(!out.contains("Coffee"), "February excluded: {out}");
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn status_filter_selects_pending_only() {
        let out = reg(
            JOURNAL, "", "", "", "", "pending", 0, "period", false, false, false, false, "date", 0,
            "first", 80, "text",
        )
        .unwrap();
        assert_eq!(out.lines().count(), 2, "only the '!' txn: {out}");
        assert!(out.contains("Coffee"));
    }

    #[test]
    fn depth_folds_account_names() {
        let out = reg(
            JOURNAL, "expenses", "", "", "", "all", 2, "period", false, false, false, false,
            "date", 0, "first", 80, "text",
        )
        .unwrap();
        assert!(out.contains("Expenses:Food"), "got {out}");
        // The account leaf is folded away; "Groceries" survives only as the
        // transaction description, which depth never touches.
        assert!(
            !out.contains("Expenses:Food:Groceries"),
            "leaf folded away: {out}"
        );
    }

    #[test]
    fn sort_by_amount_puts_the_biggest_posting_first() {
        let out = reg(
            JOURNAL, "", "", "", "", "all", 0, "period", false, false, false, false, "amount", 0,
            "first", 80, "text",
        )
        .unwrap();
        assert!(
            out.lines().next().unwrap().contains("$2000.00"),
            "got {out}"
        );
        assert!(
            out.lines().last().unwrap().contains("Income:Salary"),
            "got {out}"
        );
    }

    #[test]
    fn sort_by_date_desc_reverses_the_register() {
        let out = reg(
            JOURNAL,
            "checking",
            "",
            "",
            "",
            "all",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date-desc",
            0,
            "first",
            80,
            "text",
        )
        .unwrap();
        assert!(out.starts_with("2024-02-01"), "got {out}");
    }

    #[test]
    fn sort_by_account_groups_the_rows_alphabetically() {
        let out = reg(
            JOURNAL, "", "", "", "", "all", 0, "none", false, false, false, false, "account", 0,
            "first", 80, "text",
        )
        .unwrap();
        assert!(out.starts_with("2024-01-05 Groceries"), "got {out}");
        // Three Assets:Bank:Checking postings sort first, then the expenses.
        assert!(
            out.lines().nth(3).unwrap().contains("Expenses:Food:Coffee"),
            "got {out}"
        );
        assert!(
            out.lines().last().unwrap().contains("Income:Salary"),
            "got {out}"
        );
    }

    #[test]
    fn limit_keeps_the_first_rows_and_last_keeps_the_tail() {
        let head = reg(
            JOURNAL, "checking", "", "", "", "all", 0, "period", false, false, false, false,
            "date", 2, "first", 80, "text",
        )
        .unwrap();
        assert_eq!(head.lines().count(), 2);
        assert!(head.contains("2024-01-05"));
        let tail = reg(
            JOURNAL, "checking", "", "", "", "all", 0, "period", false, false, false, false,
            "date", 2, "last", 80, "text",
        )
        .unwrap();
        assert_eq!(tail.lines().count(), 2);
        assert!(!tail.contains("2024-01-05"));
        // The running total is computed over the whole register, then sliced.
        assert!(tail.ends_with("$1950.00"), "got {tail}");
    }

    #[test]
    fn real_only_drops_virtual_postings() {
        let j = "2024-01-01 budget\n    Expenses:Food  $10.00\n    Assets:Cash  $-10.00\n    (Budget:Food)  $-10.00\n";
        let with = simple(j, "");
        assert!(with.contains("Budget:Food"));
        let without = reg(
            j, "", "", "", "", "all", 0, "period", false, false, true, false, "date", 0, "first",
            80, "text",
        )
        .unwrap();
        assert!(!without.contains("Budget:Food"), "got {without}");
    }

    #[test]
    fn cost_basis_converts_priced_postings() {
        let j =
            "2024-01-01 buy\n    Assets:Broker:Stocks   10 AAPL @ $50.00\n    Assets:Broker:Cash\n";
        let raw = simple(j, "stocks");
        assert!(raw.contains("10 AAPL"), "got {raw}");
        let cost = reg(
            j, "stocks", "", "", "", "all", 0, "period", false, false, false, true, "date", 0,
            "first", 80, "text",
        )
        .unwrap();
        assert!(cost.contains("$500.00"), "got {cost}");
    }

    #[test]
    fn inferred_posting_balances_the_transaction() {
        let out = simple(
            "2024-03-01 rent\n    Expenses:Rent  $1200\n    Assets:Cash\n",
            "cash",
        );
        assert!(out.contains("$-1200"), "got {out}");
    }

    #[test]
    fn multi_commodity_totals_print_on_their_own_lines() {
        let j = "2024-01-01 buy\n    Assets:Broker   10 AAPL\n    Assets:Broker  $-500.00\n\n2024-01-02 fee\n    Assets:Broker  $-1.00\n    Expenses:Fees  $1.00\n";
        let out = simple(j, "broker");
        assert!(out.contains("10 AAPL"), "got {out}");
        assert!(
            out.contains("$-501.00"),
            "running total per commodity: {out}"
        );
    }

    #[test]
    fn width_narrows_the_description_and_account_columns() {
        let j = "2024-01-01 A very long description that will certainly not fit\n    Assets:Bank:Savings:Emergency:Fund  $10.00\n    Income:Gift  $-10.00\n";
        let narrow = reg(
            j,
            "emergency",
            "",
            "",
            "",
            "all",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date",
            0,
            "first",
            40,
            "text",
        )
        .unwrap();
        assert!(narrow.contains(".."), "something was clipped: {narrow}");
        // The description is clipped on the right, the account on the LEFT, so
        // the leaf that identifies the account survives.
        assert!(
            narrow.contains("A very l..") && narrow.contains("..ergency:Fund"),
            "the account leaf survives: {narrow}"
        );
        let wide = reg(
            j,
            "emergency",
            "",
            "",
            "",
            "all",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date",
            0,
            "first",
            200,
            "text",
        )
        .unwrap();
        assert!(
            wide.contains("Assets:Bank:Savings:Emergency:Fund"),
            "got {wide}"
        );
        assert!(wide.contains("certainly not fit"), "got {wide}");
    }

    #[test]
    fn csv_output_has_a_header_and_one_row_per_posting() {
        let out = reg(
            JOURNAL, "checking", "", "", "", "all", 0, "period", false, false, false, false,
            "date", 0, "first", 80, "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            concat!(
                "date,description,account,commodity,amount,total\n",
                "2024-01-05,Groceries,Assets:Bank:Checking,$,-45.20,-45.20\n",
                "2024-01-10,Salary,Assets:Bank:Checking,$,2000.00,1954.80\n",
                "2024-02-01,Coffee,Assets:Bank:Checking,$,-4.80,1950.00"
            )
        );
    }

    #[test]
    fn json_output_is_parseable_shaped_data() {
        let out = reg(
            JOURNAL, "checking", "", "", "", "all", 0, "period", false, false, false, false,
            "date", 0, "first", 80, "json",
        )
        .unwrap();
        assert!(out.starts_with('{'));
        assert!(out.contains("\"account\":\"Assets:Bank:Checking\""));
        assert!(out.contains("\"amount\":\"1950.00\""));
        assert!(out.contains("\"count\": 3"));
    }

    #[test]
    fn markdown_output_is_a_table() {
        let out = reg(
            JOURNAL, "checking", "", "", "", "all", 0, "period", false, false, false, false,
            "date", 0, "first", 80, "markdown",
        )
        .unwrap();
        assert!(out.starts_with("| Date | Description | Account | Amount | Total |"));
        assert!(out.contains("| 2024-02-01 | Coffee | Assets:Bank:Checking | $-4.80 | $1950.00 |"));
    }

    #[test]
    fn european_amounts_and_suffix_commodities_round_trip() {
        let j = "2024-01-01 shop\n    Expenses:Food  1.234,50 EUR\n    Assets:Cash\n";
        let out = simple(j, "food");
        assert!(out.contains("1234.50 EUR"), "got {out}");
    }

    #[test]
    fn directives_alias_apply_and_comment_block_are_honoured() {
        let j = "comment\n2024-01-01 ignored\n    A  $1.00\n    B  $-1.00\nend comment\n\nalias Bank = Assets:Bank\napply account Personal\n\n2024-01-02 pay\n    Bank:Checking  $-20.00\n    Expenses:Fun  $20.00\nend apply account\n";
        let out = simple(j, "");
        assert!(out.contains("Personal:Bank:Checking"), "got {out}");
        assert!(!out.contains("ignored"), "comment block dropped: {out}");
    }

    #[test]
    fn balance_assertion_is_ignored_not_added() {
        let j =
            "2024-01-01 open\n    Assets:Cash  $100.00 = $100.00\n    Equity:Opening  $-100.00\n";
        let out = simple(j, "cash");
        assert!(out.ends_with("$100.00"), "got {out}");
        assert_eq!(out.lines().count(), 1);
    }

    #[test]
    fn include_lines_are_skipped_and_reported_in_the_notes() {
        let j = "include other.journal\n\n2024-01-01 x\n    Assets:Cash  $1.00\n    Income:Gift  $-1.00\n";
        let out = simple(j, "cash");
        assert!(out.contains("Notes:"), "got {out}");
        assert!(out.contains("include"), "got {out}");
    }

    // --- error paths -------------------------------------------------------

    #[test]
    fn empty_journal_is_an_error() {
        let err = reg(
            "   ", "", "", "", "", "all", 0, "period", false, false, false, false, "date", 0,
            "first", 80, "text",
        )
        .unwrap_err();
        assert!(err.contains("empty"), "got {err}");
    }

    #[test]
    fn text_with_no_transactions_is_an_error() {
        let err = reg(
            "hello world\n",
            "",
            "",
            "",
            "",
            "all",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date",
            0,
            "first",
            80,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("no transactions found"), "got {err}");
    }

    #[test]
    fn a_filter_matching_nothing_is_an_actionable_error() {
        let err = reg(
            JOURNAL,
            "nosuchaccount",
            "",
            "",
            "",
            "all",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date",
            0,
            "first",
            80,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("no postings matched"), "got {err}");
    }

    #[test]
    fn related_without_an_account_filter_is_rejected() {
        let err = reg(
            JOURNAL, "", "", "", "", "all", 0, "period", true, false, false, false, "date", 0,
            "first", 80, "text",
        )
        .unwrap_err();
        assert!(
            err.contains("related view needs an account filter"),
            "got {err}"
        );
    }

    #[test]
    fn unknown_enum_values_are_rejected() {
        for (mode, needle) in [("nope", "unknown running_total")] {
            let err = reg(
                JOURNAL, "", "", "", "", "all", 0, mode, false, false, false, false, "date", 0,
                "first", 80, "text",
            )
            .unwrap_err();
            assert!(err.contains(needle), "got {err}");
        }
        let err = reg(
            JOURNAL, "", "", "", "", "all", 0, "period", false, false, false, false, "size", 0,
            "first", 80, "text",
        )
        .unwrap_err();
        assert!(err.contains("unknown sort"), "got {err}");
        let err = reg(
            JOURNAL,
            "",
            "",
            "",
            "",
            "sometimes",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date",
            0,
            "first",
            80,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("unknown status"), "got {err}");
        let err = reg(
            JOURNAL, "", "", "", "", "all", 0, "period", false, false, false, false, "date", 0,
            "middle", 80, "text",
        )
        .unwrap_err();
        assert!(err.contains("unknown limit_from"), "got {err}");
        let err = reg(
            JOURNAL, "", "", "", "", "all", 0, "period", false, false, false, false, "date", 0,
            "first", 80, "xml",
        )
        .unwrap_err();
        assert!(err.contains("unknown output_format"), "got {err}");
    }

    #[test]
    fn out_of_range_numbers_are_rejected() {
        let err = reg(
            JOURNAL, "", "", "", "", "all", 11, "period", false, false, false, false, "date", 0,
            "first", 80, "text",
        )
        .unwrap_err();
        assert!(err.contains("depth must be"), "got {err}");
        let err = reg(
            JOURNAL,
            "",
            "",
            "",
            "",
            "all",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date",
            MAX_LIMIT + 1,
            "first",
            80,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("limit must be"), "got {err}");
        let err = reg(
            JOURNAL, "", "", "", "", "all", 0, "period", false, false, false, false, "date", 0,
            "first", 39, "text",
        )
        .unwrap_err();
        assert!(err.contains("width must be"), "got {err}");
    }

    #[test]
    fn a_bad_amount_reports_its_line_number() {
        let err = reg(
            "2024-01-01 x\n    A  $12x.00\n    B  $-12.00\n",
            "",
            "",
            "",
            "",
            "all",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date",
            0,
            "first",
            80,
            "text",
        )
        .unwrap_err();
        assert!(err.starts_with("line 2:"), "got {err}");
    }

    #[test]
    fn a_bad_date_is_reported() {
        let err = reg(
            JOURNAL,
            "",
            "",
            "yesterday",
            "",
            "all",
            0,
            "period",
            false,
            false,
            false,
            false,
            "date",
            0,
            "first",
            80,
            "text",
        )
        .unwrap_err();
        assert!(err.starts_with("begin:"), "got {err}");
    }

    #[test]
    fn too_many_transactions_is_rejected_at_the_cap() {
        let mut j = String::new();
        for i in 0..(MAX_TRANSACTIONS + 1) {
            j.push_str(&format!(
                "2024-01-01 t{i}\n    Expenses:Fun  $1.00\n    Assets:Cash  $-1.00\n\n"
            ));
        }
        let err = reg(
            &j, "", "", "", "", "all", 0, "period", false, false, false, false, "date", 0, "first",
            80, "text",
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
        let out = reg(
            &j, "cash", "", "", "", "all", 0, "period", false, false, false, false, "date", 1,
            "last", 80, "text",
        )
        .unwrap();
        assert!(out.ends_with("$-5000.00"), "got {out}");
    }

    #[test]
    fn number_parsing_handles_separator_styles() {
        assert_eq!(parse_number("1,234.56").unwrap(), (123456000000, 2));
        assert_eq!(parse_number("1.234,56").unwrap(), (123456000000, 2));
        assert_eq!(parse_number("1,23").unwrap(), (123000000, 2));
        assert_eq!(parse_number("1,234").unwrap(), (123400000000, 0));
        assert!(parse_number("12x").is_err());
    }
}
