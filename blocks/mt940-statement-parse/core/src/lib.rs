//! mt940-statement-parse core — parse a SWIFT MT940 customer statement message
//! (the tagged-field format used for bank account statements) into structured
//! statements: opening/closing balances plus the individual `:61:`/`:86:`
//! transaction lines. Pure compute — no I/O, no wafer/wasm-bindgen deps. Shared
//! by the chat skill block and the web page.
//!
//! MT940 layout (one or more statements, each begins at a `:20:` tag):
//!   :20:  transaction reference        :25:  account identification
//!   :28C: statement / sequence number  :60F: opening balance (final)
//!   :61:  statement line (a transaction)
//!   :86:  narrative for the preceding :61: (or statement-level)
//!   :62F: closing balance (final)      :64:  closing available balance
//!   :65:  forward available balance
//! Balance value = `<C|D><YYMMDD><CCC currency><amount, comma-decimal>`.
//! `:61:` = `<value date 6n><entry date 4n?><mark C|D|RC|RD><funds 1a?>`
//!          `<amount, comma-decimal><type 4c><customer ref>[//<bank ref>]`
//!          then optional continuation line(s) = supplementary details.

use serde::Serialize;

/// Output shape: structured JSON or a flat transaction CSV.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Json,
    Csv,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(Output::Json),
            "csv" => Ok(Output::Csv),
            other => Err(format!("unknown output '{other}' (use json or csv)")),
        }
    }
}

/// How the value/entry dates are rendered in the output.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    Iso,
    Us,
    Eu,
    Raw,
}

impl DateFormat {
    pub fn parse(s: &str) -> Result<DateFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "iso" => Ok(DateFormat::Iso),
            "us" => Ok(DateFormat::Us),
            "eu" => Ok(DateFormat::Eu),
            "raw" => Ok(DateFormat::Raw),
            other => Err(format!("unknown date_format '{other}' (use iso, us, eu, or raw)")),
        }
    }
}

/// A parsed balance record (`:60F:`, `:62F:`, `:64:`, `:65:`).
#[derive(Serialize)]
pub struct Balance {
    /// `"C"` (credit / positive) or `"D"` (debit / overdrawn).
    pub mark: String,
    /// Date, formatted per `date_format`.
    pub date: String,
    /// ISO-4217 currency code, e.g. `EUR`.
    pub currency: String,
    /// Amount. Signed per `signed_amounts` (a debit balance is negative).
    pub amount: f64,
}

/// A single `:61:` statement line plus its `:86:` narrative.
#[derive(Serialize)]
pub struct Transaction {
    pub value_date: String,
    /// Booking / entry date (`MMDD` in the source), or empty if absent.
    pub entry_date: String,
    /// `"C"`, `"D"`, `"RC"` (reversal of credit) or `"RD"` (reversal of debit).
    pub mark: String,
    /// Amount. Signed per `signed_amounts` (debits negative when enabled).
    pub amount: f64,
    /// Statement currency (from the opening balance), for convenience.
    pub currency: String,
    /// 4-char transaction type code, e.g. `NTRF`, `NMSC`, `FCHG`.
    pub transaction_type: String,
    /// Reference for the account owner (before `//`).
    pub customer_reference: String,
    /// Reference of the account-servicing institution (after `//`).
    pub bank_reference: String,
    /// `:61:` supplementary details (continuation line), if any.
    pub supplementary: String,
    /// `:86:` narrative attached to this line (newline-joined).
    pub description: String,
}

/// A single statement block (one `:20:` … `:62F:`).
#[derive(Serialize)]
pub struct Statement {
    /// `:20:` transaction reference number.
    pub reference: String,
    /// `:25:` account identification.
    pub account: String,
    /// `:28C:` statement / sequence number.
    pub statement_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opening_balance: Option<Balance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closing_balance: Option<Balance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_balance: Option<Balance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forward_available_balance: Option<Balance>,
    pub transaction_count: usize,
    pub transactions: Vec<Transaction>,
}

/// String-typed entry point shared by the chat block, CLI, and web page. Parses
/// the `output` and `date_format` selectors, then delegates to [`parse`]. This is
/// the single place the three surfaces converge, so option names stay in sync.
///
/// - `output`: `json` (default) or `csv`.
/// - `date_format`: `iso` (default), `us`, `eu`, or `raw`.
/// - `delimiter`: CSV field separator — `comma` (default), `semicolon`, `tab`, or
///   `pipe`. Only consulted for CSV output.
/// - `signed_amounts`: when true (default), debit amounts/balances are negative;
///   when false, amounts stay positive and the D/C mark carries the direction.
pub fn run(
    data: &str,
    output: &str,
    date_format: &str,
    delimiter: &str,
    signed_amounts: bool,
) -> Result<String, String> {
    let out = Output::parse(output)?;
    let fmt = DateFormat::parse(date_format)?;
    parse(data, out, fmt, delimiter, signed_amounts)
}

/// Parse `data` (raw MT940 text) and render it as `output` (JSON or CSV).
///
/// - `date_format`: how value/entry dates are written (iso/us/eu/raw).
/// - `delimiter`: CSV field separator (comma/semicolon/tab/pipe) — CSV output only.
/// - `signed_amounts`: when true (default), debit amounts/balances are negative
///   and credits positive; when false, amounts stay positive and the D/C mark
///   carries the direction.
pub fn parse(
    data: &str,
    output: Output,
    date_format: DateFormat,
    delimiter: &str,
    signed_amounts: bool,
) -> Result<String, String> {
    let statements = parse_statements(data, date_format, signed_amounts)?;
    match output {
        Output::Json => serde_json::to_string_pretty(&statements)
            .map_err(|e| format!("failed to serialize JSON: {e}")),
        Output::Csv => write_csv(&statements, delimiter),
    }
}

/// Parse `data` into structured statements (the shared front half of `parse`).
pub fn parse_statements(
    data: &str,
    date_format: DateFormat,
    signed: bool,
) -> Result<Vec<Statement>, String> {
    if data.trim().is_empty() {
        return Err("no MT940 content: the input is empty".into());
    }
    let fields = tokenize(data);
    if !fields.iter().any(|(t, _)| t == "20") {
        return Err(
            "no MT940 statement found: expected a ':20:' transaction-reference tag. \
             Paste the statement body (the block-4 field lines)."
                .into(),
        );
    }

    let mut statements: Vec<Statement> = Vec::new();
    // Currency of the statement in progress (from its opening balance), applied
    // to each transaction; and the index of the last :61: for :86: attachment.
    let mut currency = String::new();
    let mut last_txn: Option<usize> = None;

    for (tag, value) in &fields {
        match tag.as_str() {
            "20" => {
                statements.push(Statement {
                    reference: value.trim().to_string(),
                    account: String::new(),
                    statement_number: String::new(),
                    opening_balance: None,
                    closing_balance: None,
                    available_balance: None,
                    forward_available_balance: None,
                    transaction_count: 0,
                    transactions: Vec::new(),
                });
                currency.clear();
                last_txn = None;
            }
            _ => {
                let Some(st) = statements.last_mut() else {
                    // A field before any :20: — ignore (shouldn't happen; the
                    // guard above ensures at least one :20: exists).
                    continue;
                };
                match tag.as_str() {
                    "25" => st.account = value.trim().to_string(),
                    "28" | "28C" => st.statement_number = value.trim().to_string(),
                    "60F" | "60M" => {
                        let b = parse_balance(value, date_format, signed)?;
                        currency = b.currency.clone();
                        st.opening_balance = Some(b);
                    }
                    "62F" | "62M" => {
                        st.closing_balance = Some(parse_balance(value, date_format, signed)?);
                    }
                    "64" => {
                        st.available_balance = Some(parse_balance(value, date_format, signed)?);
                    }
                    "65" => {
                        st.forward_available_balance =
                            Some(parse_balance(value, date_format, signed)?);
                    }
                    "61" => {
                        let txn = parse_transaction(value, &currency, date_format, signed)?;
                        st.transactions.push(txn);
                        st.transaction_count = st.transactions.len();
                        last_txn = Some(st.transactions.len() - 1);
                    }
                    "86" => {
                        let text = value.trim().to_string();
                        match last_txn {
                            Some(i) if st.transactions[i].description.is_empty() => {
                                st.transactions[i].description = text;
                            }
                            // A second :86: for the same line, or a
                            // statement-level narrative: append to the last
                            // line's description if there is one.
                            Some(i) => {
                                st.transactions[i].description.push('\n');
                                st.transactions[i].description.push_str(&text);
                            }
                            None => {} // statement-level narrative before any txn — dropped
                        }
                    }
                    _ => {} // :21: and other tags are ignored
                }
            }
        }
    }

    Ok(statements)
}

/// Split MT940 text into `(tag, value)` fields. A field starts at a line like
/// `:NN[a]:` and continues onto following non-tag lines (appended with `\n`).
/// SWIFT block headers (`{1:…}`), the `-` block trailer, and blank preamble are
/// skipped.
fn tokenize(data: &str) -> Vec<(String, String)> {
    let mut fields: Vec<(String, String)> = Vec::new();
    for raw in data.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        // Lone `-` is the block-4 trailer; skip it.
        if line.trim() == "-" {
            continue;
        }
        if let Some((tag, rest)) = split_tag(line) {
            fields.push((tag, rest.to_string()));
        } else if let Some(last) = fields.last_mut() {
            // Continuation line for the previous field (e.g. multi-line :86:).
            last.1.push('\n');
            last.1.push_str(line);
        }
        // else: preamble / block header before the first tag — ignore.
    }
    fields
}

/// Recognize a `:NN:` / `:NNa:` tag at the start of `line`. Returns
/// `(tag, remainder)` where tag is the 2-3 char code (digits + optional letter).
fn split_tag(line: &str) -> Option<(String, &str)> {
    let rest = line.strip_prefix(':')?;
    let close = rest.find(':')?;
    let tag = &rest[..close];
    // Tag = 2 digits, optionally followed by 1 uppercase letter (e.g. 28C).
    let bytes = tag.as_bytes();
    let ok = tag.len() >= 2
        && tag.len() <= 3
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && (tag.len() == 2 || bytes[2].is_ascii_uppercase());
    if ok {
        Some((tag.to_string(), &rest[close + 1..]))
    } else {
        None
    }
}

/// Parse a balance value `<C|D><YYMMDD><CCC><amount>`.
fn parse_balance(value: &str, date_format: DateFormat, signed: bool) -> Result<Balance, String> {
    let v = value.trim();
    let bytes = v.as_bytes();
    if bytes.is_empty() || (bytes[0] != b'C' && bytes[0] != b'D') {
        return Err(format!(
            "malformed balance '{v}': expected a leading C or D debit/credit mark"
        ));
    }
    let mark = (bytes[0] as char).to_string();
    if v.len() < 10 {
        return Err(format!(
            "malformed balance '{v}': too short for a date (YYMMDD) and currency"
        ));
    }
    let raw_date = &v[1..7];
    let currency = v[7..10].to_string();
    let amount_str = &v[10..];
    let amount = parse_amount(amount_str)
        .ok_or_else(|| format!("malformed balance amount '{amount_str}' in '{v}'"))?;
    let amount = apply_sign(amount, &mark, signed);
    Ok(Balance {
        mark,
        date: format_date6(raw_date, date_format),
        currency,
        amount,
    })
}

/// Parse a `:61:` statement line.
fn parse_transaction(
    value: &str,
    currency: &str,
    date_format: DateFormat,
    signed: bool,
) -> Result<Transaction, String> {
    // Split the first physical line (the structured field) from any
    // continuation (supplementary details).
    let mut lines = value.splitn(2, '\n');
    let head = lines.next().unwrap_or("").trim();
    let supplementary = lines.next().unwrap_or("").trim().to_string();

    let bytes = head.as_bytes();
    let mut i = 0usize;

    // Value date: 6 digits (YYMMDD).
    if bytes.len() < 6 || !bytes[..6].iter().all(u8::is_ascii_digit) {
        return Err(format!(
            "malformed :61: line '{head}': expected a 6-digit value date (YYMMDD)"
        ));
    }
    let value_date = format_date6(&head[..6], date_format);
    i += 6;

    // Optional entry date: 4 digits (MMDD) — present iff the next char is a digit.
    let mut entry_date = String::new();
    if i + 4 <= bytes.len() && bytes[i].is_ascii_digit() && bytes[i..i + 4].iter().all(u8::is_ascii_digit) {
        entry_date = format_mmdd(&head[i..i + 4], &head[..6], date_format);
        i += 4;
    }

    // Debit/Credit mark: RC/RD (reversal) → 2 chars, else C/D → 1 char.
    if i >= bytes.len() {
        return Err(format!("malformed :61: line '{head}': missing debit/credit mark"));
    }
    let mark = if bytes[i] == b'R' {
        if i + 2 > bytes.len() {
            return Err(format!("malformed :61: line '{head}': truncated reversal mark"));
        }
        let m = head[i..i + 2].to_string();
        i += 2;
        m
    } else if bytes[i] == b'C' || bytes[i] == b'D' {
        let m = head[i..i + 1].to_string();
        i += 1;
        m
    } else {
        return Err(format!(
            "malformed :61: line '{head}': expected a C/D/RC/RD mark after the date"
        ));
    };

    // Optional funds code: a single letter (3rd char of the currency) before the
    // amount. The amount always starts with a digit, so a letter here is the code.
    if i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }

    // Amount: digits and a comma decimal, up to the transaction-type code.
    let amount_start = i;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b',') {
        i += 1;
    }
    if i == amount_start {
        return Err(format!("malformed :61: line '{head}': missing amount"));
    }
    let amount = parse_amount(&head[amount_start..i])
        .ok_or_else(|| format!("malformed :61: amount '{}' in '{head}'", &head[amount_start..i]))?;
    let amount = apply_sign(amount, &mark, signed);

    // Transaction type: a 4-char code (1 alpha + 3 alnum), e.g. NTRF.
    let ttype_end = (i + 4).min(bytes.len());
    let transaction_type = head[i..ttype_end].to_string();
    i = ttype_end;

    // Remainder: customer reference, optionally `//bank reference`.
    let rest = &head[i..];
    let (customer_reference, bank_reference) = match rest.split_once("//") {
        Some((c, b)) => (c.trim().to_string(), b.trim().to_string()),
        None => (rest.trim().to_string(), String::new()),
    };

    Ok(Transaction {
        value_date,
        entry_date,
        mark,
        amount,
        currency: currency.to_string(),
        transaction_type,
        customer_reference,
        bank_reference,
        supplementary,
        description: String::new(),
    })
}

/// Parse an MT940 comma-decimal amount (e.g. `1234,56` or `100,`) into `f64`.
fn parse_amount(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Comma is the decimal separator; a trailing comma means no fractional part.
    let normalized = s.replace(',', ".");
    let normalized = normalized.strip_suffix('.').unwrap_or(&normalized);
    normalized.parse::<f64>().ok()
}

/// Apply the debit/credit sign when `signed` is enabled. Debit (`D`/`RD`) → negative.
fn apply_sign(amount: f64, mark: &str, signed: bool) -> f64 {
    if signed && (mark == "D" || mark == "RD") {
        -amount
    } else {
        amount
    }
}

/// Format a `YYMMDD` date per `fmt`. A 2-digit year maps to 2000–2099.
fn format_date6(raw: &str, fmt: DateFormat) -> String {
    if fmt == DateFormat::Raw || raw.len() != 6 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return raw.to_string();
    }
    let yy: i32 = raw[0..2].parse().unwrap_or(0);
    let mm = &raw[2..4];
    let dd = &raw[4..6];
    let yyyy = 2000 + yy;
    match fmt {
        DateFormat::Iso => format!("{yyyy:04}-{mm}-{dd}"),
        DateFormat::Us => format!("{mm}/{dd}/{yyyy:04}"),
        DateFormat::Eu => format!("{dd}/{mm}/{yyyy:04}"),
        DateFormat::Raw => raw.to_string(),
    }
}

/// Format a `:61:` entry date (`MMDD`, no year) borrowing the year from the
/// value date so iso/us/eu render a full date.
fn format_mmdd(mmdd: &str, value_date6: &str, fmt: DateFormat) -> String {
    if fmt == DateFormat::Raw || mmdd.len() != 4 || value_date6.len() != 6 {
        return mmdd.to_string();
    }
    format_date6(&format!("{}{}", &value_date6[0..2], mmdd), fmt)
}

fn delimiter_byte(delimiter: &str) -> Result<u8, String> {
    match delimiter.trim().to_ascii_lowercase().as_str() {
        "" | "comma" => Ok(b','),
        "semicolon" => Ok(b';'),
        "tab" => Ok(b'\t'),
        "pipe" => Ok(b'|'),
        other => Err(format!(
            "unknown delimiter '{other}' (use comma, semicolon, tab, or pipe)"
        )),
    }
}

/// Render every transaction across all statements as a flat CSV. A `Statement`
/// column disambiguates multi-statement files; balances live in the JSON output.
fn write_csv(statements: &[Statement], delimiter: &str) -> Result<String, String> {
    let delim = delimiter_byte(delimiter)?;
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .from_writer(vec![]);
    wtr.write_record([
        "Statement",
        "Value Date",
        "Entry Date",
        "D/C",
        "Amount",
        "Currency",
        "Transaction Type",
        "Customer Reference",
        "Bank Reference",
        "Description",
    ])
    .map_err(|e| e.to_string())?;
    for (si, st) in statements.iter().enumerate() {
        for t in &st.transactions {
            // Collapse narrative newlines to spaces so a row stays one line.
            let desc = t
                .description
                .split('\n')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" ");
            let supp = t.supplementary.replace('\n', " ");
            let desc = if supp.is_empty() {
                desc
            } else if desc.is_empty() {
                supp
            } else {
                format!("{desc} {supp}")
            };
            wtr.write_record([
                &(si + 1).to_string(),
                &t.value_date,
                &t.entry_date,
                &t.mark,
                &fmt_amount(t.amount),
                &t.currency,
                &t.transaction_type,
                &t.customer_reference,
                &t.bank_reference,
                &desc,
            ])
            .map_err(|e| e.to_string())?;
        }
    }
    let bytes = wtr.into_inner().map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

/// Format an amount with exactly 2 decimals (currency-style).
fn fmt_amount(a: f64) -> String {
    format!("{a:.2}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // A two-transaction single statement (EUR), debit then credit.
    const SAMPLE: &str = ":20:REF12345\n:25:NL91ABNA0417164300\n:28C:00123/001\n:60F:C240101EUR1000,00\n:61:2401020102D150,50NTRFNONREF//BANKREF1\nEXTRA DETAIL\n:86:Payment to Acme Corp invoice 42\n:61:2401030103C2000,00NTRFPAYROLL//BANKREF2\n:86:Salary March\n:62F:C240131EUR2849,50\n:64:C240131EUR2849,50\n-";

    #[test]
    fn parses_json_balances_and_transactions() {
        let out = parse(SAMPLE, Output::Json, DateFormat::Iso, "comma", true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let st = &v[0];
        assert_eq!(st["reference"], "REF12345");
        assert_eq!(st["account"], "NL91ABNA0417164300");
        assert_eq!(st["statement_number"], "00123/001");
        assert_eq!(st["opening_balance"]["amount"], 1000.0);
        assert_eq!(st["opening_balance"]["currency"], "EUR");
        assert_eq!(st["opening_balance"]["date"], "2024-01-01");
        assert_eq!(st["closing_balance"]["amount"], 2849.5);
        assert_eq!(st["available_balance"]["amount"], 2849.5);
        assert_eq!(st["transaction_count"], 2);
        let t0 = &st["transactions"][0];
        assert_eq!(t0["value_date"], "2024-01-02");
        assert_eq!(t0["entry_date"], "2024-01-02");
        assert_eq!(t0["mark"], "D");
        assert_eq!(t0["amount"], -150.5); // debit → negative when signed
        assert_eq!(t0["currency"], "EUR");
        assert_eq!(t0["transaction_type"], "NTRF");
        assert_eq!(t0["customer_reference"], "NONREF");
        assert_eq!(t0["bank_reference"], "BANKREF1");
        assert_eq!(t0["supplementary"], "EXTRA DETAIL");
        assert_eq!(t0["description"], "Payment to Acme Corp invoice 42");
        assert_eq!(st["transactions"][1]["amount"], 2000.0); // credit → positive
    }

    #[test]
    fn csv_output_has_header_and_rows() {
        let csv = parse(SAMPLE, Output::Csv, DateFormat::Iso, "comma", true).unwrap();
        let mut lines = csv.lines();
        assert_eq!(
            lines.next().unwrap(),
            "Statement,Value Date,Entry Date,D/C,Amount,Currency,Transaction Type,Customer Reference,Bank Reference,Description"
        );
        let row1 = lines.next().unwrap();
        assert_eq!(
            row1,
            "1,2024-01-02,2024-01-02,D,-150.50,EUR,NTRF,NONREF,BANKREF1,Payment to Acme Corp invoice 42 EXTRA DETAIL"
        );
        assert!(lines.next().unwrap().starts_with("1,2024-01-03"));
    }

    #[test]
    fn unsigned_amounts_stay_positive() {
        let out = parse(SAMPLE, Output::Json, DateFormat::Iso, "comma", false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["transactions"][0]["amount"], 150.5);
    }

    #[test]
    fn date_formats() {
        let us = parse(SAMPLE, Output::Json, DateFormat::Us, "comma", true).unwrap();
        assert!(us.contains("01/02/2024"));
        let eu = parse(SAMPLE, Output::Json, DateFormat::Eu, "comma", true).unwrap();
        assert!(eu.contains("02/01/2024"));
        let raw = parse(SAMPLE, Output::Json, DateFormat::Raw, "comma", true).unwrap();
        assert!(raw.contains("240102"));
    }

    #[test]
    fn reversal_mark_rc() {
        let data = ":20:R\n:60F:C240101EUR0,00\n:61:2401010101RC50,00NTRFREF//B\n:62F:C240101EUR50,00";
        let out = parse(data, Output::Json, DateFormat::Iso, "comma", true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["transactions"][0]["mark"], "RC");
        assert_eq!(v[0]["transactions"][0]["amount"], 50.0);
    }

    #[test]
    fn multi_statement() {
        let data = ":20:S1\n:60F:C240101EUR10,00\n:61:2401010101C5,00NTRFA//B\n:62F:C240101EUR15,00\n:20:S2\n:60F:C240201USD20,00\n:61:2402010201D3,00NTRFC//D\n:62F:C240201USD17,00";
        let csv = parse(data, Output::Csv, DateFormat::Iso, "comma", true).unwrap();
        assert!(csv.contains("\n1,2024-01-01"));
        assert!(csv.contains("\n2,2024-02-01"));
        assert!(csv.contains("USD"));
    }

    #[test]
    fn semicolon_delimiter() {
        let csv = parse(SAMPLE, Output::Csv, DateFormat::Iso, "semicolon", true).unwrap();
        assert!(csv.contains("Statement;Value Date;"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse("   ", Output::Json, DateFormat::Iso, "comma", true).is_err());
    }

    #[test]
    fn missing_reference_tag_errors() {
        // Has :61: but no :20: — not a valid statement.
        let err = parse(
            ":61:2401010101C5,00NTRFA//B",
            Output::Json,
            DateFormat::Iso,
            "comma",
            true,
        )
        .unwrap_err();
        assert!(err.contains(":20:"));
    }

    #[test]
    fn bad_delimiter_errors() {
        assert!(parse(SAMPLE, Output::Csv, DateFormat::Iso, "colon", true).is_err());
    }

    #[test]
    fn malformed_balance_errors() {
        let data = ":20:X\n:60F:X240101EUR10,00";
        assert!(parse(data, Output::Json, DateFormat::Iso, "comma", true).is_err());
    }

    #[test]
    fn enum_parsers() {
        assert!(Output::parse("csv").is_ok());
        assert!(Output::parse("xml").is_err());
        assert!(DateFormat::parse("julian").is_err());
    }

    #[test]
    fn run_string_entry_point_parses_json() {
        let out = run(SAMPLE, "json", "iso", "comma", true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["transactions"][0]["amount"], -150.5);
    }

    #[test]
    fn run_rejects_unknown_output_selector() {
        let err = run(SAMPLE, "yaml", "iso", "comma", true).unwrap_err();
        assert!(err.contains("yaml"));
    }
}
