//! beancount-to-csv core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Flattens a Beancount / Ledger (plain-text-accounting) journal into a flat CSV
//! of one row per posting — `date,flag,payee,narration,account,amount,currency,
//! cost,price,comment` — for spreadsheet use, and rebuilds a simple journal from
//! that same CSV shape.
//!
//! It is NOT a full Beancount/Ledger engine: it preserves common transaction
//! postings. Non-transaction directives (open/close/balance/price/pad/note/…,
//! `option`/`plugin`/`include`) are ignored, elided amounts are left blank
//! (no interpolation), balance assertions and cost-lot matching are not
//! evaluated, and cost `{…}` / price `@…` expressions are carried through
//! verbatim rather than computed.

use csv::{ReaderBuilder, WriterBuilder};

/// Hard cap on postings (CSV rows) converted in one call, so a huge paste can't
/// blow up memory. The chat/CLI schema advertises this same bound.
pub const MAX_POSTINGS: usize = 20_000;

/// Fixed CSV column order — the flat shape produced by `to-csv` and consumed by
/// `from-csv`. Kept public so callers/tests can reference it.
pub const COLUMNS: [&str; 10] = [
    "date", "flag", "payee", "narration", "account", "amount", "currency", "cost", "price",
    "comment",
];

/// Beancount non-transaction directive keywords (the second token on a dated
/// line). Such lines are skipped rather than flattened — they carry no posting.
const DIRECTIVES: [&str; 11] = [
    "open", "close", "commodity", "balance", "pad", "note", "document", "event", "price", "query",
    "custom",
];

/// Which plain-text-accounting dialect to emit for `from-csv`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dialect {
    Beancount,
    Ledger,
}

impl Dialect {
    fn parse(s: &str) -> Result<Dialect, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "beancount" | "bean" => Ok(Dialect::Beancount),
            "ledger" | "hledger" => Ok(Dialect::Ledger),
            other => Err(format!(
                "unknown journal_format '{other}' (use beancount or ledger)"
            )),
        }
    }
}

fn delimiter_byte(s: &str) -> Result<u8, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "comma" | "," => Ok(b','),
        "semicolon" | ";" => Ok(b';'),
        "tab" | "\t" => Ok(b'\t'),
        "pipe" | "|" => Ok(b'|'),
        other => Err(format!(
            "unknown delimiter '{other}' (use comma, semicolon, tab, or pipe)"
        )),
    }
}

/// Entry point. `direction` is `to-csv` (journal → CSV) or `from-csv` (CSV →
/// journal); `journal_format` picks the dialect emitted by `from-csv`; `delimiter`
/// is the CSV field separator (output for to-csv, input for from-csv).
pub fn convert(
    input: &str,
    direction: &str,
    journal_format: &str,
    delimiter: &str,
) -> Result<String, String> {
    let delim = delimiter_byte(delimiter)?;
    match direction.trim().to_ascii_lowercase().as_str() {
        "" | "to-csv" | "to_csv" | "tocsv" | "csv" | "journal-to-csv" => {
            journal_to_csv(input, delim)
        }
        "from-csv" | "from_csv" | "fromcsv" | "journal" | "csv-to-journal" => {
            csv_to_journal(input, Dialect::parse(journal_format)?, delim)
        }
        other => Err(format!(
            "unknown direction '{other}' (use to-csv or from-csv)"
        )),
    }
}

// ---------------------------------------------------------------------------
// to-csv: journal → flat CSV
// ---------------------------------------------------------------------------

/// One flattened posting row.
#[derive(Default)]
struct Row {
    date: String,
    flag: String,
    payee: String,
    narration: String,
    account: String,
    amount: String,
    currency: String,
    cost: String,
    price: String,
    comment: String,
}

fn journal_to_csv(input: &str, delim: u8) -> Result<String, String> {
    let mut rows: Vec<Row> = Vec::new();
    // Current transaction header, carried onto each of its posting rows.
    let mut cur: Option<(String, String, String, String)> = None; // date,flag,payee,narration
    let mut cur_has_posting = false;

    for raw in input.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');

        if indented {
            // A posting (or a stray comment) belonging to the current txn.
            if let Some((ref d, ref f, ref p, ref n)) = cur {
                if trimmed.starts_with(';') || trimmed.starts_with('#') {
                    continue; // standalone comment line inside a transaction
                }
                if let Some(mut row) = parse_posting(trimmed) {
                    row.date = d.clone();
                    row.flag = f.clone();
                    row.payee = p.clone();
                    row.narration = n.clone();
                    rows.push(row);
                    cur_has_posting = true;
                    if rows.len() > MAX_POSTINGS {
                        return Err(format!(
                            "too many postings (> {MAX_POSTINGS}); split the journal into smaller batches"
                        ));
                    }
                }
            }
            continue;
        }

        // A non-indented line starts a new top-level entry, so the previous
        // transaction is finished. If it had a header but no postings, keep a
        // single header-only row so the transaction isn't silently dropped.
        flush_headeronly(&mut cur, &mut cur_has_posting, &mut rows);

        if trimmed.starts_with(';') || trimmed.starts_with('#') || trimmed.starts_with('*') {
            continue; // top-level comment / org heading
        }

        // Dated line? Split the first whitespace-delimited token.
        let (first, rest) = split_first_token(trimmed);
        let date = match normalize_date(first) {
            Some(d) => d,
            None => continue, // non-dated directive (option/plugin/include/…)
        };
        let rest = rest.trim_start();
        // Directive (open/close/balance/…) rather than a transaction?
        let (kw, _) = split_first_token(rest);
        if DIRECTIVES.contains(&kw.to_ascii_lowercase().as_str()) {
            continue;
        }
        let (flag, payee, narration) = parse_header(rest);
        cur = Some((date, flag, payee, narration));
        cur_has_posting = false;
    }
    flush_headeronly(&mut cur, &mut cur_has_posting, &mut rows);

    if rows.is_empty() {
        return Err(
            "no transactions found — paste a Beancount/Ledger journal with dated transactions and indented postings".into(),
        );
    }

    let mut wtr = WriterBuilder::new().delimiter(delim).from_writer(Vec::new());
    wtr.write_record(COLUMNS)
        .map_err(|e| format!("CSV write error: {e}"))?;
    for r in &rows {
        wtr.write_record([
            &r.date, &r.flag, &r.payee, &r.narration, &r.account, &r.amount, &r.currency, &r.cost,
            &r.price, &r.comment,
        ])
        .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("CSV encoding error: {e}"))
}

/// If a transaction header was parsed but produced no posting rows, emit one
/// header-only row so the entry survives the round trip.
fn flush_headeronly(
    cur: &mut Option<(String, String, String, String)>,
    cur_has_posting: &mut bool,
    rows: &mut Vec<Row>,
) {
    if let Some((d, f, p, n)) = cur.take() {
        if !*cur_has_posting {
            rows.push(Row {
                date: d,
                flag: f,
                payee: p,
                narration: n,
                ..Default::default()
            });
        }
    }
    *cur_has_posting = false;
}

/// Split `s` into (first whitespace-delimited token, remainder).
fn split_first_token(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    match s.find(|c: char| c.is_whitespace()) {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, ""),
    }
}

/// Accept `YYYY-MM-DD` or `YYYY/MM/DD` (Beancount uses `-`, Ledger allows `/`),
/// normalizing to ISO `YYYY-MM-DD`. Returns None if the token is not a date.
fn normalize_date(tok: &str) -> Option<String> {
    let b = tok.as_bytes();
    if b.len() != 10 {
        return None;
    }
    let sep = b[4];
    if (sep != b'-' && sep != b'/') || b[7] != sep {
        return None;
    }
    for (i, &c) in b.iter().enumerate() {
        if i == 4 || i == 7 {
            continue;
        }
        if !c.is_ascii_digit() {
            return None;
        }
    }
    Some(tok.replace('/', "-"))
}

/// Parse a transaction header's post-date remainder into (flag, payee, narration).
/// Handles `* "Payee" "Narration"`, `* "Narration"`, `! "Narration"`, `txn "…"`,
/// and Ledger's unquoted `* Payee` / bare `Payee` (whole description → narration).
fn parse_header(rest: &str) -> (String, String, String) {
    let rest = rest.trim();
    // Leading flag.
    let (flag, after) = if let Some(r) = rest.strip_prefix("txn") {
        if r.is_empty() || r.starts_with(char::is_whitespace) {
            ("txn".to_string(), r.trim_start())
        } else {
            (String::new(), rest)
        }
    } else {
        let mut ch = rest.chars();
        match ch.next() {
            Some(c @ ('*' | '!')) => (c.to_string(), ch.as_str().trim_start()),
            _ => (String::new(), rest),
        }
    };

    let quoted = extract_quoted(after);
    let (payee, narration) = match quoted.len() {
        0 => {
            // Unquoted (Ledger): strip trailing `; comment`, use whole line as
            // the single narration field (payee left blank).
            let desc = after.split(';').next().unwrap_or("").trim();
            (String::new(), desc.to_string())
        }
        1 => (String::new(), quoted[0].clone()),
        _ => (quoted[0].clone(), quoted[1].clone()),
    };
    (flag, payee, narration)
}

/// Extract double-quoted substrings, honoring `\"` escapes. Returns their
/// unescaped contents in order.
fn extract_quoted(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut buf = String::new();
        while let Some(&nc) = chars.peek() {
            chars.next();
            if nc == '\\' {
                if let Some(&esc) = chars.peek() {
                    chars.next();
                    buf.push(esc);
                }
                continue;
            }
            if nc == '"' {
                break;
            }
            buf.push(nc);
        }
        out.push(buf);
    }
    out
}

/// Parse an indented posting line into a Row (posting fields only). Returns None
/// for a line that carries no account.
fn parse_posting(line: &str) -> Option<Row> {
    let mut body = line.trim().to_string();
    // Optional posting-level flag (`* Account …` / `! Account …`).
    if let Some(rest) = body.strip_prefix("* ").or_else(|| body.strip_prefix("! ")) {
        body = rest.trim_start().to_string();
    }
    if body.is_empty() {
        return None;
    }

    // Trailing `; comment`.
    let mut comment = String::new();
    if let Some(i) = body.find(';') {
        comment = body[i + 1..].trim().to_string();
        body.truncate(i);
    }
    let mut body = body.trim().to_string();

    // Price (`@ …` / `@@ …`) comes last; carry it through verbatim.
    let mut price = String::new();
    if let Some(i) = body.find('@') {
        price = body[i..].trim().to_string();
        body.truncate(i);
    }
    // Cost (`{…}` / `{{…}}`); carry through verbatim.
    let mut cost = String::new();
    if let Some(start) = body.find('{') {
        if let Some(rel) = body[start..].rfind('}') {
            let end = start + rel + 1;
            cost = body[start..end].trim().to_string();
            let mut b = body[..start].to_string();
            b.push_str(&body[end..]);
            body = b;
        }
    }
    let body = body.trim();

    // Split account from amount on the first run of 2+ spaces or a tab.
    let (account, amount_expr) = split_account_amount(body);
    if account.is_empty() {
        return None;
    }
    let (amount, currency) = parse_amount(amount_expr)?;

    Some(Row {
        account: account.to_string(),
        amount,
        currency,
        cost,
        price,
        comment,
        ..Default::default()
    })
}

/// Split a posting body into (account, amount-expression) on the first tab or
/// run of 2+ spaces (Beancount/Ledger both use that gap; account names may hold
/// single spaces in Ledger).
fn split_account_amount(body: &str) -> (&str, &str) {
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\t' {
            return (body[..i].trim(), body[i + 1..].trim());
        }
        if bytes[i] == b' ' && i + 1 < bytes.len() && bytes[i + 1] == b' ' {
            return (body[..i].trim(), body[i..].trim());
        }
        i += 1;
    }
    (body.trim(), "")
}

/// Split an amount expression like `4.50 USD`, `$-4.50`, `USD 4.50`, `1,234.56`,
/// or `(20.00)` into (numeric-string, currency). Dialect-agnostic: number-ish
/// characters form the amount, the remaining non-space characters the currency.
/// Returns None only when the expression is non-empty but numerically invalid.
fn parse_amount(expr: &str) -> Option<(String, String)> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Some((String::new(), String::new()));
    }
    let mut num = String::new();
    let mut cur = String::new();
    for c in expr.chars() {
        if c.is_whitespace() {
            continue;
        }
        if c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+' | '(' | ')') {
            num.push(c);
        } else {
            cur.push(c);
        }
    }
    // Parentheses → negative accounting notation.
    let mut negative = false;
    if num.starts_with('(') && num.ends_with(')') {
        negative = true;
        num = num[1..num.len() - 1].to_string();
    }
    // Drop thousands separators (assumes '.' decimal — see limits).
    num = num.replace(',', "");
    if negative && !num.starts_with('-') {
        num = format!("-{num}");
    }
    if num.is_empty() {
        // Currency present but no number — treat the whole thing as a bare label.
        return Some((String::new(), cur));
    }
    if num.parse::<f64>().is_err() {
        return None;
    }
    Some((num, cur))
}

// ---------------------------------------------------------------------------
// from-csv: flat CSV → journal
// ---------------------------------------------------------------------------

fn csv_to_journal(input: &str, dialect: Dialect, delim: u8) -> Result<String, String> {
    let mut rdr = ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(input.as_bytes());

    let headers = rdr
        .headers()
        .map_err(|e| format!("cannot read CSV header row: {e}"))?
        .clone();
    let idx = |name: &str| {
        headers
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(name))
    };
    let (Some(i_date), Some(i_account)) = (idx("date"), idx("account")) else {
        return Err(
            "CSV must have at least a 'date' and an 'account' column (header row: date,flag,payee,narration,account,amount,currency,cost,price,comment)".into(),
        );
    };
    let i_flag = idx("flag");
    let i_payee = idx("payee");
    let i_narration = idx("narration");
    let i_amount = idx("amount");
    let i_currency = idx("currency");
    let i_cost = idx("cost");
    let i_price = idx("price");
    let i_comment = idx("comment");
    let get = |rec: &csv::StringRecord, i: Option<usize>| -> String {
        i.and_then(|i| rec.get(i)).unwrap_or("").trim().to_string()
    };

    // Group consecutive rows into transactions: a row with a blank date
    // continues the current transaction; otherwise a changed (date,flag,payee,
    // narration) starts a new one.
    struct Txn {
        date: String,
        flag: String,
        payee: String,
        narration: String,
        postings: Vec<Row>,
    }
    let mut txns: Vec<Txn> = Vec::new();
    let mut count = 0usize;

    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("CSV parse error: {e}"))?;
        let account = get(&rec, Some(i_account));
        let date = get(&rec, Some(i_date));
        if date.is_empty() && account.is_empty() {
            continue; // blank spacer row
        }
        count += 1;
        if count > MAX_POSTINGS {
            return Err(format!(
                "too many rows (> {MAX_POSTINGS}); split the CSV into smaller batches"
            ));
        }
        let flag = get(&rec, i_flag);
        let payee = get(&rec, i_payee);
        let narration = get(&rec, i_narration);
        let amount = get(&rec, i_amount);
        if !amount.is_empty() && amount.parse::<f64>().is_err() {
            return Err(format!(
                "amount '{amount}' is not a number (row {count}); use a plain signed number like -4.50"
            ));
        }
        let posting = Row {
            account,
            amount,
            currency: get(&rec, i_currency),
            cost: get(&rec, i_cost),
            price: get(&rec, i_price),
            comment: get(&rec, i_comment),
            ..Default::default()
        };

        let same = txns.last().is_some_and(|t| {
            date.is_empty()
                || (t.date == date && t.flag == flag && t.payee == payee && t.narration == narration)
        });
        if same {
            txns.last_mut().unwrap().postings.push(posting);
        } else {
            if date.is_empty() {
                return Err(
                    "first data row has a blank date; the first transaction needs a date".into(),
                );
            }
            txns.push(Txn {
                date,
                flag,
                payee,
                narration,
                postings: vec![posting],
            });
        }
    }

    if txns.is_empty() {
        return Err("no data rows found in the CSV".into());
    }

    let mut out = String::new();
    for (n, t) in txns.iter().enumerate() {
        if n > 0 {
            out.push('\n');
        }
        match dialect {
            Dialect::Beancount => {
                write_beancount(&mut out, &t.date, &t.flag, &t.payee, &t.narration, &t.postings)
            }
            Dialect::Ledger => {
                write_ledger(&mut out, &t.date, &t.flag, &t.payee, &t.narration, &t.postings)
            }
        }
    }
    Ok(out)
}

fn flag_or_default(flag: &str) -> &str {
    let f = flag.trim();
    if f.is_empty() {
        "*"
    } else {
        f
    }
}

fn write_beancount(
    out: &mut String,
    date: &str,
    flag: &str,
    payee: &str,
    narration: &str,
    postings: &[Row],
) {
    out.push_str(date);
    out.push(' ');
    out.push_str(flag_or_default(flag));
    if !payee.is_empty() {
        out.push_str(&format!(
            " \"{}\" \"{}\"",
            escape_bean(payee),
            escape_bean(narration)
        ));
    } else if !narration.is_empty() {
        out.push_str(&format!(" \"{}\"", escape_bean(narration)));
    }
    out.push('\n');
    for p in postings {
        out.push_str("  ");
        out.push_str(&p.account);
        push_amount_suffix(out, &p.amount, &p.currency, &p.cost, &p.price, &p.comment);
        out.push('\n');
    }
}

fn write_ledger(
    out: &mut String,
    date: &str,
    flag: &str,
    payee: &str,
    narration: &str,
    postings: &[Row],
) {
    out.push_str(date);
    out.push(' ');
    out.push_str(flag_or_default(flag));
    // Ledger has a single description field; join payee + narration.
    let desc = match (payee.is_empty(), narration.is_empty()) {
        (false, false) => format!("{payee} {narration}"),
        (false, true) => payee.to_string(),
        (true, false) => narration.to_string(),
        (true, true) => String::new(),
    };
    if !desc.is_empty() {
        out.push(' ');
        out.push_str(&desc);
    }
    out.push('\n');
    for p in postings {
        out.push_str("    ");
        out.push_str(&p.account);
        // Ledger idiom: symbol currencies prefix the number ($-4.50); alphabetic
        // codes suffix it (-4.50 USD).
        if !p.amount.is_empty() {
            out.push_str("  ");
            if !p.currency.is_empty() && p.currency.chars().all(|c| !c.is_ascii_alphanumeric()) {
                out.push_str(&format!("{}{}", p.currency, p.amount));
            } else if p.currency.is_empty() {
                out.push_str(&p.amount);
            } else {
                out.push_str(&format!("{} {}", p.amount, p.currency));
            }
            if !p.cost.is_empty() {
                out.push(' ');
                out.push_str(&p.cost);
            }
            if !p.price.is_empty() {
                out.push(' ');
                out.push_str(&p.price);
            }
        }
        if !p.comment.is_empty() {
            out.push_str("  ; ");
            out.push_str(&p.comment);
        }
        out.push('\n');
    }
}

/// Append `  amount currency cost price  ; comment` (Beancount style, code
/// suffix) to a posting line.
fn push_amount_suffix(
    out: &mut String,
    amount: &str,
    currency: &str,
    cost: &str,
    price: &str,
    comment: &str,
) {
    if !amount.is_empty() {
        out.push_str("  ");
        out.push_str(amount);
        if !currency.is_empty() {
            out.push(' ');
            out.push_str(currency);
        }
        if !cost.is_empty() {
            out.push(' ');
            out.push_str(cost);
        }
        if !price.is_empty() {
            out.push(' ');
            out.push_str(price);
        }
    }
    if !comment.is_empty() {
        out.push_str("  ; ");
        out.push_str(comment);
    }
}

fn escape_bean(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beancount_to_csv_happy() {
        let journal = "\
2024-01-15 * \"Starbucks\" \"Morning coffee\"
    Expenses:Food:Coffee    4.50 USD
    Assets:Bank:Checking   -4.50 USD
";
        let csv = convert(journal, "to-csv", "beancount", "comma").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines[0],
            "date,flag,payee,narration,account,amount,currency,cost,price,comment"
        );
        assert_eq!(
            lines[1],
            "2024-01-15,*,Starbucks,Morning coffee,Expenses:Food:Coffee,4.50,USD,,,"
        );
        assert_eq!(
            lines[2],
            "2024-01-15,*,Starbucks,Morning coffee,Assets:Bank:Checking,-4.50,USD,,,"
        );
    }

    #[test]
    fn ledger_dollar_and_directive_skip() {
        let journal = "\
2024-01-01 open Assets:Bank:Checking
2024-01-16 * Grocery Store
    Expenses:Groceries    $25.00
    Assets:Bank:Checking  $-25.00
";
        let csv = convert(journal, "to-csv", "beancount", "comma").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        // open directive skipped → only 2 posting rows + header.
        assert_eq!(lines.len(), 3);
        assert_eq!(
            lines[1],
            "2024-01-16,*,,Grocery Store,Expenses:Groceries,25.00,$,,,"
        );
    }

    #[test]
    fn cost_price_and_comment_preserved() {
        let journal = "\
2024-02-01 * \"Broker\" \"Buy shares\"
    Assets:Stocks   10 HOOL {100.00 USD} @ 120.00 USD  ; lot A
    Assets:Cash    -1000.00 USD
";
        let csv = convert(journal, "to-csv", "beancount", "comma").unwrap();
        let row = csv.lines().nth(1).unwrap();
        assert_eq!(
            row,
            "2024-02-01,*,Broker,Buy shares,Assets:Stocks,10,HOOL,{100.00 USD},@ 120.00 USD,lot A"
        );
    }

    #[test]
    fn csv_to_beancount_roundtrip() {
        let csv = "\
date,flag,payee,narration,account,amount,currency,cost,price,comment
2024-01-15,*,Starbucks,Morning coffee,Expenses:Food:Coffee,4.50,USD,,,
2024-01-15,*,Starbucks,Morning coffee,Assets:Bank:Checking,-4.50,USD,,,
";
        let journal = convert(csv, "from-csv", "beancount", "comma").unwrap();
        assert_eq!(
            journal,
            "2024-01-15 * \"Starbucks\" \"Morning coffee\"\n  Expenses:Food:Coffee  4.50 USD\n  Assets:Bank:Checking  -4.50 USD\n"
        );
        // Round trip back to CSV is stable.
        let csv2 = convert(&journal, "to-csv", "beancount", "comma").unwrap();
        assert_eq!(csv.trim_end(), csv2.trim_end());
    }

    #[test]
    fn csv_blank_date_continuation() {
        let csv = "\
date,account,amount,currency
2024-03-01,Expenses:Rent,1200.00,USD
,Assets:Bank,-1200.00,USD
";
        let journal = convert(csv, "from-csv", "beancount", "comma").unwrap();
        assert_eq!(
            journal,
            "2024-03-01 *\n  Expenses:Rent  1200.00 USD\n  Assets:Bank  -1200.00 USD\n"
        );
    }

    #[test]
    fn csv_to_ledger_symbol_prefix() {
        let csv = "\
date,flag,payee,narration,account,amount,currency,cost,price,comment
2024-01-16,*,,Grocery Store,Expenses:Groceries,25.00,$,,,
2024-01-16,*,,Grocery Store,Assets:Bank:Checking,-25.00,$,,,
";
        let journal = convert(csv, "from-csv", "ledger", "comma").unwrap();
        assert_eq!(
            journal,
            "2024-01-16 * Grocery Store\n    Expenses:Groceries  $25.00\n    Assets:Bank:Checking  $-25.00\n"
        );
    }

    #[test]
    fn semicolon_delimiter_and_parens_negative() {
        let journal = "2024-04-01 * \"Refund\"\n    Assets:Cash   (20.00) USD\n";
        let csv = convert(journal, "to-csv", "beancount", "semicolon").unwrap();
        let row = csv.lines().nth(1).unwrap();
        assert_eq!(row, "2024-04-01;*;;Refund;Assets:Cash;-20.00;USD;;;");
    }

    #[test]
    fn err_bad_direction() {
        let e = convert("x", "sideways", "beancount", "comma").unwrap_err();
        assert!(e.contains("unknown direction"), "{e}");
    }

    #[test]
    fn err_no_transactions() {
        let e = convert("; just a comment\noption \"x\" \"y\"\n", "to-csv", "beancount", "comma")
            .unwrap_err();
        assert!(e.contains("no transactions"), "{e}");
    }

    #[test]
    fn err_missing_columns() {
        let e = convert("foo,bar\n1,2\n", "from-csv", "beancount", "comma").unwrap_err();
        assert!(e.contains("date") && e.contains("account"), "{e}");
    }

    #[test]
    fn err_bad_amount_in_csv() {
        let csv = "date,account,amount\n2024-01-01,Assets:Cash,notanumber\n";
        let e = convert(csv, "from-csv", "beancount", "comma").unwrap_err();
        assert!(e.contains("not a number"), "{e}");
    }
}
