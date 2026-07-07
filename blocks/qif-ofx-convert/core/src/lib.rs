//! qif-ofx-convert core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Parses QIF (Quicken Interchange Format)
//! and OFX/QFX (Open Financial Exchange, both 1.x SGML and 2.x XML) bank exports
//! into a single normalized CSV with a fixed column set, ready for import into
//! budgeting/spreadsheet apps.

/// Normalized output columns, in fixed order. Every transaction row carries all
/// eight; columns that never apply to a format (e.g. Category/FITID for OFX vs
/// QIF) stay blank unless `drop_empty_columns` removes the all-blank ones.
pub const COLUMNS: [&str; 8] = [
    "Date",
    "Amount",
    "Payee",
    "Memo",
    "Category",
    "Check Number",
    "Type",
    "FITID",
];

/// Input format. `Auto` sniffs the payload: an OFX marker (`<STMTTRN`, `<OFX`, or
/// `OFXHEADER`) selects OFX, otherwise the input is parsed as QIF.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Auto,
    Qif,
    Ofx,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Format::Auto),
            "qif" => Ok(Format::Qif),
            "ofx" | "qfx" => Ok(Format::Ofx),
            other => Err(format!("unknown format '{other}' (use auto, qif, or ofx)")),
        }
    }
}

/// How the output Date column is written. `Raw` keeps the source date text
/// unchanged; the others parse and reformat it.
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
            other => Err(format!(
                "unknown date_format '{other}' (use iso, us, eu, or raw)"
            )),
        }
    }
}

#[derive(Default)]
struct Txn {
    date: String,
    amount: String,
    payee: String,
    memo: String,
    category: String,
    check: String,
    ttype: String,
    fitid: String,
}

impl Txn {
    fn cell(&self, i: usize) -> &str {
        match i {
            0 => &self.date,
            1 => &self.amount,
            2 => &self.payee,
            3 => &self.memo,
            4 => &self.category,
            5 => &self.check,
            6 => &self.ttype,
            7 => &self.fitid,
            _ => "",
        }
    }

    fn is_empty(&self) -> bool {
        (0..8).all(|i| self.cell(i).is_empty())
    }
}

/// Convert a QIF/OFX export into normalized CSV.
///
/// `format` picks the parser (or auto-detects). `date_format` controls the Date
/// column. `delimiter` is the output CSV separator. `invert_amounts` flips every
/// amount's sign (for banks/apps that expect debits positive). `drop_empty_columns`
/// omits columns that hold no data across all rows.
pub fn convert(
    data: &str,
    format: Format,
    date_format: DateFormat,
    delimiter: &str,
    invert_amounts: bool,
    drop_empty_columns: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let fmt = match format {
        Format::Auto => detect(data),
        f => f,
    };
    let txns = match fmt {
        Format::Qif => parse_qif(data, date_format, invert_amounts),
        Format::Ofx => parse_ofx(data, date_format, invert_amounts),
        Format::Auto => unreachable!(),
    };
    if txns.is_empty() {
        let kind = if fmt == Format::Ofx { "OFX" } else { "QIF" };
        return Err(format!(
            "no transactions found in the {kind} input (expected {})",
            if fmt == Format::Ofx {
                "<STMTTRN> blocks"
            } else {
                "records ending in '^'"
            }
        ));
    }
    write_csv(&txns, delimiter, drop_empty_columns)
}

/// Sniff QIF vs OFX. Any OFX structural marker selects OFX; otherwise QIF.
fn detect(data: &str) -> Format {
    let lower = data.to_ascii_lowercase();
    if lower.contains("<stmttrn") || lower.contains("<ofx") || lower.contains("ofxheader") {
        Format::Ofx
    } else {
        Format::Qif
    }
}

// ---------------------------------------------------------------------------
// QIF
// ---------------------------------------------------------------------------

fn parse_qif(data: &str, date_format: DateFormat, invert: bool) -> Vec<Txn> {
    let mut out = Vec::new();
    let mut cur = Txn::default();
    let mut splits: Vec<String> = Vec::new();
    let mut has_amount = false;

    let flush = |cur: &mut Txn, splits: &mut Vec<String>, out: &mut Vec<Txn>| {
        if cur.category.is_empty() && !splits.is_empty() {
            cur.category = splits.join("; ");
        }
        if !cur.is_empty() {
            out.push(std::mem::take(cur));
        }
        splits.clear();
    };

    for raw_line in data.lines() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        // Header / option / account lines start with '!' — ignore.
        if line.starts_with('!') {
            continue;
        }
        let (code, val) = line.split_at(1);
        let val = val.trim();
        match code {
            "^" => {
                flush(&mut cur, &mut splits, &mut out);
                has_amount = false;
            }
            "D" => cur.date = format_date(val, false, date_format),
            "T" => {
                // T is the primary amount; U is a Quicken duplicate — only take U
                // if T was absent.
                cur.amount = normalize_amount(val, invert);
                has_amount = true;
            }
            "U" => {
                if !has_amount {
                    cur.amount = normalize_amount(val, invert);
                }
            }
            "P" => cur.payee = val.to_string(),
            "M" => cur.memo = val.to_string(),
            "L" => cur.category = val.to_string(),
            "N" => cur.check = val.to_string(),
            "S" => splits.push(val.to_string()),
            _ => {} // C (cleared), A (address), E (split memo), etc. — ignored
        }
    }
    // Some exports omit the trailing '^' on the last record.
    flush(&mut cur, &mut splits, &mut out);
    out
}

// ---------------------------------------------------------------------------
// OFX (1.x SGML + 2.x XML)
// ---------------------------------------------------------------------------

fn parse_ofx(data: &str, date_format: DateFormat, invert: bool) -> Vec<Txn> {
    let lower = data.to_ascii_lowercase();
    let mut out = Vec::new();
    let open = "<stmttrn>";
    let close = "</stmttrn>";
    let mut search = 0usize;
    while let Some(rel) = lower[search..].find(open) {
        let start = search + rel + open.len();
        // Block ends at the matching close tag, or the next STMTTRN, or EOF.
        let after = &lower[start..];
        let end_rel = after.find(close).or_else(|| after.find(open));
        let end = match end_rel {
            Some(r) => start + r,
            None => data.len(),
        };
        let block = &data[start..end];
        out.push(parse_ofx_block(block, date_format, invert));
        search = end;
    }
    out
}

fn parse_ofx_block(block: &str, date_format: DateFormat, invert: bool) -> Txn {
    let mut t = Txn::default();
    if let Some(v) = extract_tag(block, "TRNTYPE") {
        t.ttype = v;
    }
    if let Some(v) = extract_tag(block, "DTPOSTED") {
        t.date = format_date(&v, true, date_format);
    }
    if let Some(v) = extract_tag(block, "TRNAMT") {
        t.amount = normalize_amount(&v, invert);
    }
    if let Some(v) = extract_tag(block, "NAME") {
        t.payee = v;
    }
    if let Some(v) = extract_tag(block, "MEMO") {
        t.memo = v;
    }
    if let Some(v) = extract_tag(block, "CHECKNUM") {
        t.check = v;
    }
    if let Some(v) = extract_tag(block, "FITID") {
        t.fitid = v;
    }
    t
}

/// Read the first value of `<tag>` from an OFX block. Works for both SGML (value
/// runs from `>` to the next `<`, with no closing tag) and XML (value runs to
/// `</tag>`, which also starts with `<`). Entities are decoded and the result is
/// trimmed. Returns None if the tag is absent or empty.
fn extract_tag(block: &str, tag: &str) -> Option<String> {
    let lower = block.to_ascii_lowercase();
    let needle = format!("<{}>", tag.to_ascii_lowercase());
    let start = lower.find(&needle)? + needle.len();
    let rest = &block[start..];
    let end = rest.find('<').unwrap_or(rest.len());
    let val = decode_entities(rest[..end].trim());
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

fn decode_entities(s: &str) -> String {
    // &amp; is replaced LAST so an escaped entity like "&amp;lt;" decodes to the
    // literal "&lt;" rather than being double-decoded to "<".
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#38;", "&")
        .replace("&amp;", "&")
}

// ---------------------------------------------------------------------------
// Shared field normalization
// ---------------------------------------------------------------------------

/// Strip thousands separators and whitespace, drop a leading `+`, and optionally
/// flip the sign. Keeps the original decimal text (e.g. "50.00" stays "50.00")
/// rather than round-tripping through a float.
fn normalize_amount(raw: &str, invert: bool) -> String {
    let mut s: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .collect();
    if s.is_empty() {
        return String::new();
    }
    // A zero amount never gets a sign flip.
    let is_zero = s
        .trim_start_matches(['-', '+'])
        .chars()
        .all(|c| c == '0' || c == '.');
    if let Some(rest) = s.strip_prefix('+') {
        s = rest.to_string();
    }
    if invert && !is_zero {
        s = match s.strip_prefix('-') {
            Some(rest) => rest.to_string(),
            None => format!("-{s}"),
        };
    }
    s
}

/// Format a source date. `is_ofx` selects the parser: OFX dates are YYYYMMDD
/// (optionally with a time suffix); QIF dates are M/D/Y-ish with US month-first
/// default. On any parse failure the raw text is returned unchanged so one odd
/// date never fails the whole file.
fn format_date(raw: &str, is_ofx: bool, fmt: DateFormat) -> String {
    if fmt == DateFormat::Raw {
        return raw.to_string();
    }
    let ymd = if is_ofx {
        parse_ofx_date(raw)
    } else {
        parse_qif_date(raw)
    };
    match ymd {
        Some((y, m, d)) => match fmt {
            DateFormat::Iso => format!("{y:04}-{m:02}-{d:02}"),
            DateFormat::Us => format!("{m:02}/{d:02}/{y:04}"),
            DateFormat::Eu => format!("{d:02}/{m:02}/{y:04}"),
            DateFormat::Raw => raw.to_string(),
        },
        None => raw.to_string(),
    }
}

fn parse_ofx_date(raw: &str) -> Option<(i32, u32, u32)> {
    let digits: String = raw.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() < 8 {
        return None;
    }
    let y = digits[0..4].parse().ok()?;
    let m = digits[4..6].parse().ok()?;
    let d = digits[6..8].parse().ok()?;
    valid_ymd(y, m, d)
}

fn parse_qif_date(raw: &str) -> Option<(i32, u32, u32)> {
    // Split on any non-digit (handles '/', '-', '.', and Quicken's "'" year sep).
    let parts: Vec<&str> = raw
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 3 {
        return None;
    }
    let mut a: i32 = parts[0].parse().ok()?;
    let mut b: i32 = parts[1].parse().ok()?;
    let y_raw: i32 = parts[2].parse().ok()?;
    // QIF is US month-first by default; if the first field can't be a month but
    // the second can, it's day-first.
    if a > 12 && b <= 12 {
        std::mem::swap(&mut a, &mut b);
    }
    let year = expand_year(y_raw, parts[2].len());
    valid_ymd(year, a as u32, b as u32)
}

/// Expand a 2-digit year (pivot at 70 → 2000s below, 1900s at/above) or keep a
/// 4-digit year as-is.
fn expand_year(y: i32, digits: usize) -> i32 {
    if digits >= 4 || y > 99 {
        y
    } else if y < 70 {
        2000 + y
    } else {
        1900 + y
    }
}

fn valid_ymd(y: i32, m: u32, d: u32) -> Option<(i32, u32, u32)> {
    if (1..=12).contains(&m) && (1..=31).contains(&d) && y > 0 {
        Some((y, m, d))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// CSV output
// ---------------------------------------------------------------------------

fn delimiter_byte(delimiter: &str) -> Result<u8, String> {
    match delimiter.trim().to_ascii_lowercase().as_str() {
        "" | "," | "comma" => Ok(b','),
        "\t" | "tab" | "\\t" => Ok(b'\t'),
        ";" | "semicolon" => Ok(b';'),
        "|" | "pipe" => Ok(b'|'),
        other => Err(format!(
            "unknown delimiter '{other}' (use comma, semicolon, tab, or pipe)"
        )),
    }
}

fn write_csv(txns: &[Txn], delimiter: &str, drop_empty: bool) -> Result<String, String> {
    let delim = delimiter_byte(delimiter)?;
    let cols: Vec<usize> = if drop_empty {
        (0..8)
            .filter(|&i| txns.iter().any(|t| !t.cell(i).is_empty()))
            .collect()
    } else {
        (0..8).collect()
    };
    if cols.is_empty() {
        return Err("no data to write (all columns are empty)".into());
    }
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .from_writer(vec![]);
    let header: Vec<&str> = cols.iter().map(|&i| COLUMNS[i]).collect();
    wtr.write_record(&header)
        .map_err(|e| format!("CSV write error: {e}"))?;
    for t in txns {
        let rec: Vec<&str> = cols.iter().map(|&i| t.cell(i)).collect();
        wtr.write_record(&rec)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("CSV utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const QIF: &str = "!Type:Bank\nD03/15/2010\nT-50.00\nPTarget Store\nMweekly run\nLFood:Groceries\nN1002\n^\nD3/16/2010\nT1,250.00\nPPaycheck\nLIncome:Salary\n^";

    const OFX: &str = "OFXHEADER:100\n<OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS><BANKTRANLIST>\n<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20231026120000<TRNAMT>-75.50<FITID>abc123<NAME>Corner Grocery<MEMO>food & drink</STMTTRN>\n<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20231101<TRNAMT>2000.00<FITID>xyz789<NAME>Employer<CHECKNUM>0<MEMO>payroll</STMTTRN>\n</BANKTRANLIST></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>";

    #[test]
    fn qif_to_iso_csv() {
        let out = convert(QIF, Format::Qif, DateFormat::Iso, "comma", false, true).unwrap();
        // drop_empty removes Type + FITID (empty for QIF).
        assert_eq!(
            out,
            "Date,Amount,Payee,Memo,Category,Check Number\n\
             2010-03-15,-50.00,Target Store,weekly run,Food:Groceries,1002\n\
             2010-03-16,1250.00,Paycheck,,Income:Salary,\n"
        );
    }

    #[test]
    fn qif_full_column_set_when_not_dropping() {
        let out = convert(QIF, Format::Qif, DateFormat::Iso, "comma", false, false).unwrap();
        let first = out.lines().next().unwrap();
        assert_eq!(
            first,
            "Date,Amount,Payee,Memo,Category,Check Number,Type,FITID"
        );
    }

    #[test]
    fn ofx_sgml_parsed() {
        let out = convert(OFX, Format::Ofx, DateFormat::Iso, "comma", false, true).unwrap();
        // Category empty for OFX so dropped; entity &amp; decoded in memo.
        assert_eq!(
            out,
            "Date,Amount,Payee,Memo,Check Number,Type,FITID\n\
             2023-10-26,-75.50,Corner Grocery,food & drink,,DEBIT,abc123\n\
             2023-11-01,2000.00,Employer,payroll,0,CREDIT,xyz789\n"
        );
    }

    #[test]
    fn auto_detects_ofx() {
        let out = convert(OFX, Format::Auto, DateFormat::Iso, "comma", false, true).unwrap();
        assert!(out.contains("Corner Grocery"));
    }

    #[test]
    fn auto_detects_qif() {
        let out = convert(QIF, Format::Auto, DateFormat::Iso, "comma", false, true).unwrap();
        assert!(out.contains("Target Store"));
    }

    #[test]
    fn invert_amounts_flips_sign() {
        let out = convert(QIF, Format::Qif, DateFormat::Iso, "comma", true, true).unwrap();
        // -50.00 -> 50.00 ; 1250.00 -> -1250.00
        assert!(out.contains(",50.00,Target Store,"));
        assert!(out.contains(",-1250.00,Paycheck,"));
    }

    #[test]
    fn date_formats() {
        let us = convert(QIF, Format::Qif, DateFormat::Us, "comma", false, true).unwrap();
        assert!(us.contains("03/15/2010"));
        let eu = convert(QIF, Format::Qif, DateFormat::Eu, "comma", false, true).unwrap();
        assert!(eu.contains("15/03/2010"));
        let raw = convert(QIF, Format::Qif, DateFormat::Raw, "comma", false, true).unwrap();
        assert!(raw.contains("03/15/2010"));
    }

    #[test]
    fn semicolon_delimiter() {
        let out = convert(QIF, Format::Qif, DateFormat::Iso, "semicolon", false, true).unwrap();
        assert!(out.lines().next().unwrap().contains("Date;Amount;Payee"));
    }

    #[test]
    fn qif_day_first_date_detected() {
        // 25/12/2010 — first field > 12, so day-first.
        let out = convert(
            "!Type:Bank\nD25/12/2010\nT-5.00\nPShop\n^",
            Format::Qif,
            DateFormat::Iso,
            "comma",
            false,
            true,
        )
        .unwrap();
        assert!(out.contains("2010-12-25"));
    }

    #[test]
    fn qif_splits_join_into_category() {
        let out = convert(
            "!Type:Bank\nD01/02/2020\nT-30.00\nPStore\nSFood\n$-20.00\nSHousehold\n$-10.00\n^",
            Format::Qif,
            DateFormat::Iso,
            "comma",
            false,
            true,
        )
        .unwrap();
        assert!(out.contains("Food; Household"));
    }

    #[test]
    fn ofx_xml_closing_tags() {
        let xml = "<OFX><STMTTRN><TRNTYPE>DEBIT</TRNTYPE><DTPOSTED>20200115</DTPOSTED><TRNAMT>-9.99</TRNAMT><NAME>Coffee</NAME><FITID>f1</FITID></STMTTRN></OFX>";
        let out = convert(xml, Format::Auto, DateFormat::Iso, "comma", false, true).unwrap();
        assert!(out.contains("2020-01-15,-9.99,Coffee"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("   ", Format::Auto, DateFormat::Iso, "comma", false, false).is_err());
    }

    #[test]
    fn no_transactions_errors() {
        assert!(
            convert("!Type:Bank\n", Format::Qif, DateFormat::Iso, "comma", false, false).is_err()
        );
    }

    #[test]
    fn bad_delimiter_errors() {
        assert!(convert(QIF, Format::Qif, DateFormat::Iso, "colon", false, false).is_err());
    }

    #[test]
    fn format_parse_rejects_unknown() {
        assert!(Format::parse("csv").is_err());
        assert!(DateFormat::parse("julian").is_err());
    }
}
