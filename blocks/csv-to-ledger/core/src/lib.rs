//! csv-to-ledger core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps. Turns a bank / credit-card CSV export into
//! double-entry ledger-cli / hledger journal transactions.
//!
//! For each row it reads a date, a description/payee, and an amount (either a
//! single signed `Amount` column, or separate `Debit`/`Credit` columns), then
//! emits a balanced transaction: one posting to the configured asset/bank
//! account and a balancing posting to a *guessed* account. The counter-account
//! is chosen by (1) user-supplied substring rules, (2) a built-in keyword table
//! (groceries, transport, utilities, salary, …), or (3) a sign-based default
//! (`Expenses:Unknown` for money out, `Income:Unknown` for money in) — the same
//! `expenses:unknown` / `income:unknown` fallback hledger uses.

use csv::ReaderBuilder;

/// Hard cap on rows converted in one call, so a huge paste can't blow up memory.
/// The chat/CLI schema advertises this same bound.
pub const MAX_ROWS: usize = 10_000;

/// How to interpret the numeric parts of a date.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DateOrder {
    Auto,
    Ymd,
    Mdy,
    Dmy,
}

impl DateOrder {
    fn parse(s: &str) -> Result<DateOrder, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(DateOrder::Auto),
            "ymd" | "iso" => Ok(DateOrder::Ymd),
            "mdy" | "us" => Ok(DateOrder::Mdy),
            "dmy" | "eu" => Ok(DateOrder::Dmy),
            other => Err(format!(
                "unknown date_format '{other}' (use auto, ymd, mdy, or dmy)"
            )),
        }
    }
}

/// Ledger vs hledger output style. Both tools accept both forms; the toggle
/// picks whether the balancing (asset) posting carries an explicit amount.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    /// Explicit amount on both postings (fully balanced, safest).
    Ledger,
    /// Omit the amount on the asset posting and let the tool infer it (compact).
    Hledger,
}

impl OutputFormat {
    fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "ledger" => Ok(OutputFormat::Ledger),
            "hledger" => Ok(OutputFormat::Hledger),
            other => Err(format!(
                "unknown output_format '{other}' (use ledger or hledger)"
            )),
        }
    }
}

/// Collapse a header to lowercase alphanumerics only, so column matching is
/// independent of spacing/punctuation: "Transaction Date" → "transactiondate".
fn collapse(header: &str) -> String {
    header
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Sniff the most likely CSV delimiter from the first non-empty line.
fn sniff_delimiter(data: &str) -> u8 {
    let line = data.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let candidates = [(b',', ','), (b';', ';'), (b'\t', '\t'), (b'|', '|')];
    let mut best = b',';
    let mut best_count = 0usize;
    for (byte, ch) in candidates {
        let n = line.matches(ch).count();
        if n > best_count {
            best_count = n;
            best = byte;
        }
    }
    best
}

fn delimiter_byte(delimiter: &str, data: &str) -> Result<u8, String> {
    Ok(match delimiter.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => sniff_delimiter(data),
        "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let bytes = other.as_bytes();
            if bytes.len() == 1 {
                bytes[0]
            } else {
                return Err(format!(
                    "delimiter must be auto/comma/semicolon/tab/pipe (or a single char), got '{other}'"
                ));
            }
        }
    })
}

/// Parse CSV text into (headers, rows).
fn parse_csv(data: &str, delimiter: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let delim = delimiter_byte(delimiter, data)?;
    let mut rdr = ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(data.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("could not read the CSV header row: {e}"))?
        .iter()
        .map(|s| s.to_string())
        .collect();
    if headers.is_empty() {
        return Err("the CSV has no header row (the first row must be the column names)".into());
    }
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("could not parse a CSV row: {e}"))?;
        rows.push(rec.iter().map(|s| s.to_string()).collect());
    }
    Ok((headers, rows))
}

/// Find the column index for `wanted`. If `explicit` is given, match it exactly
/// (case-insensitively, punctuation-independent) and error if it's absent. Else
/// return the first header whose collapsed form contains any of `auto_keys`.
fn find_column(
    headers: &[String],
    explicit: &str,
    auto_keys: &[&str],
    wanted: &str,
) -> Result<Option<usize>, String> {
    let collapsed: Vec<String> = headers.iter().map(|h| collapse(h)).collect();
    if !explicit.trim().is_empty() {
        let want = collapse(explicit);
        return collapsed
            .iter()
            .position(|c| *c == want)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "{wanted} column '{}' not found — available columns: {}",
                    explicit.trim(),
                    headers.join(", ")
                )
            });
    }
    for key in auto_keys {
        if let Some(pos) = collapsed.iter().position(|c| c.contains(key)) {
            return Ok(Some(pos));
        }
    }
    Ok(None)
}

/// Parse a money string into a signed f64. Strips currency symbols/spaces,
/// treats `(1,234.56)` and a trailing `DR`/leading `-` as negative, and handles
/// both US (`1,234.56`) and EU (`1.234,56`) thousands/decimal separators.
fn parse_amount(raw: &str) -> Result<Option<f64>, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let mut neg = false;
    // Parentheses or a trailing DR/CR marker denote sign.
    let mut body = s.to_string();
    if body.starts_with('(') && body.ends_with(')') {
        neg = true;
        body = body[1..body.len() - 1].to_string();
    }
    let upper = body.to_ascii_uppercase();
    if upper.trim_end().ends_with("DR") {
        neg = true;
        let cut = body.trim_end().len() - 2;
        body = body[..cut].to_string();
    } else if upper.trim_end().ends_with("CR") {
        let cut = body.trim_end().len() - 2;
        body = body[..cut].to_string();
    }
    // Keep only digits, separators and a sign.
    let mut cleaned: String = body
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',' || *c == '-' || *c == '+')
        .collect();
    if cleaned.contains('-') {
        neg = !neg;
    }
    cleaned = cleaned.replace(['-', '+'], "");
    if cleaned.is_empty() {
        return Err(format!("could not read an amount from '{raw}'"));
    }
    // Decide the decimal separator when both '.' and ',' appear: the *last* one.
    let has_dot = cleaned.contains('.');
    let has_comma = cleaned.contains(',');
    let normalized = if has_dot && has_comma {
        if cleaned.rfind('.') > cleaned.rfind(',') {
            cleaned.replace(',', "") // comma = thousands
        } else {
            cleaned.replace('.', "").replace(',', ".") // dot = thousands, comma = decimal
        }
    } else if has_comma {
        // Only commas: decimal comma if exactly one with 1-2 trailing digits.
        let parts: Vec<&str> = cleaned.split(',').collect();
        if parts.len() == 2 && parts[1].len() <= 2 {
            cleaned.replace(',', ".")
        } else {
            cleaned.replace(',', "") // thousands grouping
        }
    } else {
        cleaned
    };
    let value: f64 = normalized
        .parse()
        .map_err(|_| format!("could not read an amount from '{raw}'"))?;
    Ok(Some(if neg { -value } else { value }))
}

const MONTHS: &[(&str, u32)] = &[
    ("jan", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("may", 5),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
    ("sep", 9),
    ("oct", 10),
    ("nov", 11),
    ("dec", 12),
];

fn month_from_name(s: &str) -> Option<u32> {
    let low = s.to_ascii_lowercase();
    MONTHS
        .iter()
        .find(|(m, _)| low.starts_with(m))
        .map(|(_, n)| *n)
}

fn expand_year(y: i64) -> i64 {
    if y >= 100 {
        y
    } else if y < 70 {
        2000 + y
    } else {
        1900 + y
    }
}

/// Normalize a date cell to ISO `YYYY-MM-DD`. Accepts `-`, `/`, `.` separators,
/// 2- or 4-digit years, numeric or month-name months, in the requested order
/// (or best-effort auto detection).
fn normalize_date(raw: &str, order: DateOrder) -> Result<String, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("a row is missing its date".into());
    }
    let parts: Vec<&str> = s
        .split(|c: char| c == '-' || c == '/' || c == '.' || c == ' ')
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 3 {
        return Err(format!(
            "could not read a date from '{raw}' (expected day, month and year)"
        ));
    }
    let (a, b, c) = (parts[0], parts[1], parts[2]);

    let name_month = |p: &str| month_from_name(p);
    let num = |p: &str| p.parse::<i64>().ok();

    let (year, month, day);

    // ISO if the first token is a 4-digit year, or ymd was requested.
    let first_is_year = a.len() == 4 && num(a).is_some();
    if first_is_year || order == DateOrder::Ymd {
        year = num(a).ok_or_else(|| format!("could not read the year in '{raw}'"))?;
        month = name_month(b)
            .map(|m| m as i64)
            .or_else(|| num(b))
            .ok_or_else(|| format!("could not read the month in '{raw}'"))?;
        day = num(c).ok_or_else(|| format!("could not read the day in '{raw}'"))?;
    } else {
        // Day/month order from the first two tokens; year is the third.
        year = num(c).ok_or_else(|| format!("could not read the year in '{raw}'"))?;
        if let Some(m) = name_month(a) {
            // "Jan 15 2024"
            month = m as i64;
            day = num(b).ok_or_else(|| format!("could not read the day in '{raw}'"))?;
        } else if let Some(m) = name_month(b) {
            // "15 Jan 2024"
            month = m as i64;
            day = num(a).ok_or_else(|| format!("could not read the day in '{raw}'"))?;
        } else {
            let x = num(a).ok_or_else(|| format!("could not read the date '{raw}'"))?;
            let y = num(b).ok_or_else(|| format!("could not read the date '{raw}'"))?;
            let (d, m) = match order {
                DateOrder::Dmy => (x, y),
                DateOrder::Mdy => (y, x),
                _ => {
                    // Auto: use whichever assignment is unambiguous, else US (mdy).
                    if x > 12 && y <= 12 {
                        (x, y) // x must be the day
                    } else if y > 12 && x <= 12 {
                        (y, x) // y must be the day → mdy
                    } else {
                        (y, x) // default to US month-first
                    }
                }
            };
            day = d;
            month = m;
        }
    }
    let year = expand_year(year);
    if !(1..=12).contains(&month) {
        return Err(format!("month out of range in '{raw}'"));
    }
    if !(1..=31).contains(&day) {
        return Err(format!("day out of range in '{raw}'"));
    }
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

/// Built-in expense-side keyword heuristics (checked for money-out rows).
const EXPENSE_RULES: &[(&str, &str)] = &[
    ("grocery", "Expenses:Groceries"),
    ("groceries", "Expenses:Groceries"),
    ("supermarket", "Expenses:Groceries"),
    ("walmart", "Expenses:Groceries"),
    ("costco", "Expenses:Groceries"),
    ("kroger", "Expenses:Groceries"),
    ("safeway", "Expenses:Groceries"),
    ("aldi", "Expenses:Groceries"),
    ("tesco", "Expenses:Groceries"),
    ("sainsbury", "Expenses:Groceries"),
    ("trader joe", "Expenses:Groceries"),
    ("whole foods", "Expenses:Groceries"),
    ("restaurant", "Expenses:Food"),
    ("cafe", "Expenses:Food"),
    ("coffee", "Expenses:Food"),
    ("starbucks", "Expenses:Food"),
    ("mcdonald", "Expenses:Food"),
    ("pizza", "Expenses:Food"),
    ("doordash", "Expenses:Food"),
    ("grubhub", "Expenses:Food"),
    ("uber eats", "Expenses:Food"),
    ("dining", "Expenses:Food"),
    ("uber", "Expenses:Transport"),
    ("lyft", "Expenses:Transport"),
    ("taxi", "Expenses:Transport"),
    ("transit", "Expenses:Transport"),
    ("metro", "Expenses:Transport"),
    ("parking", "Expenses:Transport"),
    ("shell", "Expenses:Transport"),
    ("chevron", "Expenses:Transport"),
    ("fuel", "Expenses:Transport"),
    ("gas station", "Expenses:Transport"),
    ("amazon", "Expenses:Shopping"),
    ("ebay", "Expenses:Shopping"),
    ("target", "Expenses:Shopping"),
    ("rent", "Expenses:Rent"),
    ("landlord", "Expenses:Rent"),
    ("mortgage", "Expenses:Mortgage"),
    ("electric", "Expenses:Utilities"),
    ("water bill", "Expenses:Utilities"),
    ("utility", "Expenses:Utilities"),
    ("utilities", "Expenses:Utilities"),
    ("comcast", "Expenses:Utilities"),
    ("verizon", "Expenses:Utilities"),
    ("internet", "Expenses:Utilities"),
    ("netflix", "Expenses:Subscriptions"),
    ("spotify", "Expenses:Subscriptions"),
    ("hulu", "Expenses:Subscriptions"),
    ("disney", "Expenses:Subscriptions"),
    ("subscription", "Expenses:Subscriptions"),
    ("insurance", "Expenses:Insurance"),
    ("pharmacy", "Expenses:Health"),
    ("cvs", "Expenses:Health"),
    ("walgreens", "Expenses:Health"),
    ("doctor", "Expenses:Health"),
    ("dental", "Expenses:Health"),
    ("gym", "Expenses:Fitness"),
    ("fitness", "Expenses:Fitness"),
    ("atm", "Expenses:Cash"),
    ("fee", "Expenses:Fees"),
    ("service charge", "Expenses:Fees"),
];

/// Built-in income-side keyword heuristics (checked for money-in rows).
const INCOME_RULES: &[(&str, &str)] = &[
    ("salary", "Income:Salary"),
    ("payroll", "Income:Salary"),
    ("paycheck", "Income:Salary"),
    ("wages", "Income:Salary"),
    ("direct deposit", "Income:Salary"),
    ("dividend", "Income:Investments"),
    ("interest", "Income:Interest"),
    ("refund", "Income:Refunds"),
    ("reimbursement", "Income:Refunds"),
    ("rebate", "Income:Refunds"),
];

/// One user rule: a lowercase substring pattern → account name.
struct UserRule {
    pattern: String,
    account: String,
}

/// Parse the `account_rules` text: one `pattern = Account:Name` per line
/// (`=>`, `->`, or `:`-free `=` accepted as the separator). Blank lines and
/// `#` comments are ignored.
fn parse_rules(text: &str) -> Result<Vec<UserRule>, String> {
    let mut rules = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let sep = l
            .find("=>")
            .map(|p| (p, 2))
            .or_else(|| l.find("->").map(|p| (p, 2)))
            .or_else(|| l.find('=').map(|p| (p, 1)));
        let (pos, len) = sep.ok_or_else(|| {
            format!(
                "rule on line {} must be 'pattern = Account' — got '{l}'",
                i + 1
            )
        })?;
        let pattern = l[..pos].trim().to_ascii_lowercase();
        let account = l[pos + len..].trim().to_string();
        if pattern.is_empty() || account.is_empty() {
            return Err(format!(
                "rule on line {} must have both a pattern and an account",
                i + 1
            ));
        }
        rules.push(UserRule { pattern, account });
    }
    Ok(rules)
}

/// Choose the balancing account for a posting from its description and sign.
fn guess_account(
    description: &str,
    signed: f64,
    rules: &[UserRule],
    default_expense: &str,
    default_income: &str,
) -> String {
    let desc = description.to_ascii_lowercase();
    for r in rules {
        if desc.contains(&r.pattern) {
            return r.account.clone();
        }
    }
    if signed <= 0.0 {
        for (kw, acct) in EXPENSE_RULES {
            if desc.contains(kw) {
                return (*acct).to_string();
            }
        }
        default_expense.to_string()
    } else {
        for (kw, acct) in INCOME_RULES {
            if desc.contains(kw) {
                return (*acct).to_string();
            }
        }
        default_income.to_string()
    }
}

/// Format a signed amount with its commodity. A symbol currency (`$`, `£`, `€`)
/// is prefixed (`$-4.50`); an alphabetic code (`USD`) is suffixed (`-4.50 USD`).
fn format_amount(signed: f64, currency: &str) -> String {
    let num = format!("{:.2}", signed);
    let cur = currency.trim();
    if cur.is_empty() {
        num
    } else if cur.chars().all(|c| c.is_ascii_alphabetic()) {
        format!("{num} {cur}")
    } else {
        format!("{cur}{num}")
    }
}

#[allow(clippy::too_many_arguments)]
pub fn to_ledger(
    data: &str,
    date_column: &str,
    description_column: &str,
    amount_column: &str,
    debit_column: &str,
    credit_column: &str,
    asset_account: &str,
    default_expense_account: &str,
    default_income_account: &str,
    account_rules: &str,
    currency: &str,
    date_format: &str,
    output_format: &str,
    delimiter: &str,
    invert_amount: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("no input — paste a bank or credit-card CSV export with a header row".into());
    }
    let order = DateOrder::parse(date_format)?;
    let out_fmt = OutputFormat::parse(output_format)?;
    let rules = parse_rules(account_rules)?;

    let trimmed_or = |v: &str, default: &'static str| -> String {
        let t = v.trim();
        if t.is_empty() {
            default.to_string()
        } else {
            t.to_string()
        }
    };
    let asset = trimmed_or(asset_account, "Assets:Bank:Checking");
    let def_expense = trimmed_or(default_expense_account, "Expenses:Unknown");
    let def_income = trimmed_or(default_income_account, "Income:Unknown");

    let (headers, rows) = parse_csv(data, delimiter)?;
    if rows.is_empty() {
        return Err("no data rows found (the CSV has a header row but no transactions)".into());
    }
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "too many rows: {} (max {MAX_ROWS} per conversion)",
            rows.len()
        ));
    }

    let date_idx = find_column(&headers, date_column, &["date", "posted", "posting"], "date")?
        .ok_or_else(|| {
            format!(
                "could not find a date column — name one with date_column. Available: {}",
                headers.join(", ")
            )
        })?;
    let desc_idx = find_column(
        &headers,
        description_column,
        &[
            "description",
            "payee",
            "narration",
            "details",
            "memo",
            "name",
            "reference",
            "particulars",
        ],
        "description",
    )?;

    let amount_idx = find_column(&headers, amount_column, &["amount", "value"], "amount")?;
    let debit_idx = find_column(
        &headers,
        debit_column,
        &["debit", "withdrawal", "moneyout", "paidout", "outflow"],
        "debit",
    )?;
    let credit_idx = find_column(
        &headers,
        credit_column,
        &["credit", "deposit", "moneyin", "paidin", "inflow"],
        "credit",
    )?;

    // Prefer an explicit single amount column; else fall back to debit/credit.
    let use_debit_credit = amount_column.trim().is_empty()
        && amount_idx.is_none()
        && (debit_idx.is_some() || credit_idx.is_some());
    if amount_idx.is_none() && !use_debit_credit {
        return Err(format!(
            "could not find an amount column (or debit/credit columns) — name one with amount_column, debit_column, or credit_column. Available: {}",
            headers.join(", ")
        ));
    }

    let mut out: Vec<String> = Vec::new();
    for (rn, row) in rows.iter().enumerate() {
        let get = |idx: Option<usize>| -> &str {
            idx.and_then(|i| row.get(i)).map(|s| s.trim()).unwrap_or("")
        };
        // Skip fully blank rows.
        if row.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let date = normalize_date(get(Some(date_idx)), order)
            .map_err(|e| format!("row {}: {e}", rn + 1))?;

        let signed = if use_debit_credit {
            let debit = parse_amount(get(debit_idx)).map_err(|e| format!("row {}: {e}", rn + 1))?;
            let credit =
                parse_amount(get(credit_idx)).map_err(|e| format!("row {}: {e}", rn + 1))?;
            // credit (deposit / money in) is positive to the asset; debit is negative.
            credit.unwrap_or(0.0).abs() - debit.unwrap_or(0.0).abs()
        } else {
            match parse_amount(get(amount_idx)).map_err(|e| format!("row {}: {e}", rn + 1))? {
                Some(v) => v,
                None => continue, // no amount on this row → skip
            }
        };
        let signed = if invert_amount { -signed } else { signed };

        let description = {
            let d = get(desc_idx);
            if d.is_empty() {
                "(unknown)".to_string()
            } else {
                d.replace(['\n', '\r'], " ")
            }
        };

        let counter = guess_account(&description, signed, &rules, &def_expense, &def_income);

        // Align the amount column: pad to the wider of the two account names.
        let pad = asset.len().max(counter.len()) + 4;
        let asset_amt = format_amount(signed, currency);
        let counter_amt = format_amount(-signed, currency);

        out.push(format!("{date} {description}"));
        // Balancing (guessed) posting first, then the asset account.
        out.push(format!("    {counter:<pad$}{counter_amt}"));
        match out_fmt {
            OutputFormat::Ledger => {
                out.push(format!("    {asset:<pad$}{asset_amt}"));
            }
            OutputFormat::Hledger => {
                // Omit the amount — hledger/ledger infer the balancing figure.
                out.push(format!("    {asset}"));
            }
        }
        out.push(String::new()); // blank line between transactions
    }

    if out.is_empty() {
        return Err("no transactions produced — every row was blank or had no amount".into());
    }
    // Trim the trailing blank line.
    while out.last().map(|l| l.is_empty()).unwrap_or(false) {
        out.pop();
    }
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(data: &str) -> String {
        to_ledger(
            data, "", "", "", "", "", "", "", "", "", "$", "auto", "ledger", "auto", false,
        )
        .unwrap()
    }

    #[test]
    fn signed_amount_basic() {
        let out = conv(
            "Date,Description,Amount\n2024-01-15,Starbucks Coffee,-4.50\n2024-01-16,ACME Payroll,2000.00",
        );
        // A negative bank amount (money out) decreases the asset (negative
        // posting) and increases the expense counter-account (positive posting);
        // positive income is the mirror. Amounts align to the widest account + 4.
        let expected = "2024-01-15 Starbucks Coffee\n    Expenses:Food           $4.50\n    Assets:Bank:Checking    $-4.50\n\n2024-01-16 ACME Payroll\n    Income:Salary           $-2000.00\n    Assets:Bank:Checking    $2000.00";
        assert_eq!(out, expected);
    }

    #[test]
    fn debit_credit_columns() {
        let out = conv("Date,Payee,Debit,Credit\n01/15/2024,Rent,1500.00,\n01/20/2024,Refund,,25.00");
        assert!(out.contains("2024-01-15 Rent"));
        assert!(out.contains("Expenses:Rent"));
        assert!(out.contains("$-1500.00"));
        assert!(out.contains("2024-01-20 Refund"));
        assert!(out.contains("Income:Refunds"));
        assert!(out.contains("$25.00"));
    }

    #[test]
    fn user_rules_override_heuristics() {
        let out = to_ledger(
            "Date,Description,Amount\n2024-03-01,STARBUCKS #123,-6.00",
            "", "", "", "", "", "", "", "",
            "starbucks = Expenses:Coffee:Treats\n# a comment\n",
            "$", "auto", "ledger", "auto", false,
        )
        .unwrap();
        assert!(out.contains("Expenses:Coffee:Treats"));
        assert!(!out.contains("Expenses:Food"));
    }

    #[test]
    fn eu_date_and_amount() {
        let out = to_ledger(
            "Date;Description;Amount\n15.01.2024;Supermarkt;-1.234,56",
            "", "", "", "", "", "", "", "", "", "€", "dmy", "ledger", "semicolon", false,
        )
        .unwrap();
        assert!(out.contains("2024-01-15 Supermarkt"), "got: {out}");
        assert!(out.contains("€-1234.56"), "got: {out}");
    }

    #[test]
    fn currency_code_suffix_and_hledger() {
        let out = to_ledger(
            "Date,Description,Amount\n2024-01-15,Coffee,-4.50",
            "", "", "", "", "", "", "", "", "", "USD", "auto", "hledger", "auto", false,
        )
        .unwrap();
        // hledger form: guessed posting has the amount, asset posting is bare.
        let expected =
            "2024-01-15 Coffee\n    Expenses:Food           4.50 USD\n    Assets:Bank:Checking";
        assert_eq!(out, expected);
    }

    #[test]
    fn invert_amount_flips_sign() {
        // Bank exports outflows as positive → invert so they become expenses.
        let out = to_ledger(
            "Date,Description,Amount\n2024-01-15,Grocery Store,50.00",
            "", "", "", "", "", "", "", "", "", "$", "auto", "ledger", "auto", true,
        )
        .unwrap();
        assert!(out.contains("Expenses:Groceries"));
        assert!(out.contains("$-50.00"));
    }

    #[test]
    fn parentheses_negative_and_custom_accounts() {
        let out = to_ledger(
            "Date,Description,Amount\n2024-01-15,Mystery,(20.00)",
            "", "", "", "", "", "Assets:Card", "Expenses:Misc", "Income:Misc", "", "$",
            "auto", "ledger", "auto", false,
        )
        .unwrap();
        assert!(out.contains("Expenses:Misc"), "got: {out}");
        assert!(out.contains("Assets:Card"));
        assert!(out.contains("$-20.00"));
    }

    #[test]
    fn explicit_column_names() {
        let out = to_ledger(
            "When,Who,Value\n2024-01-15,Netflix,-15.99",
            "When", "Who", "Value", "", "", "", "", "", "", "$", "auto", "ledger", "auto", false,
        )
        .unwrap();
        assert!(out.contains("2024-01-15 Netflix"));
        assert!(out.contains("Expenses:Subscriptions"));
    }

    #[test]
    fn err_empty_input() {
        assert!(to_ledger("", "", "", "", "", "", "", "", "", "", "$", "auto", "ledger", "auto", false).is_err());
    }

    #[test]
    fn err_no_date_column() {
        let e = to_ledger(
            "Foo,Amount\nbar,-1.00",
            "", "", "", "", "", "", "", "", "", "$", "auto", "ledger", "auto", false,
        )
        .unwrap_err();
        assert!(e.contains("date column"), "got: {e}");
    }

    #[test]
    fn err_no_amount_column() {
        let e = to_ledger(
            "Date,Description\n2024-01-15,Foo",
            "", "", "", "", "", "", "", "", "", "$", "auto", "ledger", "auto", false,
        )
        .unwrap_err();
        assert!(e.contains("amount column"), "got: {e}");
    }

    #[test]
    fn err_explicit_column_missing() {
        let e = to_ledger(
            "Date,Description,Amount\n2024-01-15,Foo,-1.00",
            "TxnDate", "", "", "", "", "", "", "", "", "$", "auto", "ledger", "auto", false,
        )
        .unwrap_err();
        assert!(e.contains("not found"), "got: {e}");
    }

    #[test]
    fn err_bad_date() {
        let e = to_ledger(
            "Date,Description,Amount\nnotadate,Foo,-1.00",
            "", "", "", "", "", "", "", "", "", "$", "auto", "ledger", "auto", false,
        )
        .unwrap_err();
        assert!(e.contains("row 1"), "got: {e}");
    }

    #[test]
    fn month_name_date() {
        let out = to_ledger(
            "Date,Description,Amount\n15 Jan 2024,Coffee,-4.50",
            "", "", "", "", "", "", "", "", "", "$", "auto", "ledger", "auto", false,
        )
        .unwrap();
        assert!(out.contains("2024-01-15 Coffee"), "got: {out}");
    }
}
