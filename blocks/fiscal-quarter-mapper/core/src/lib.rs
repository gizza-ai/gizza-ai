//! fiscal-quarter-mapper core — pure compute, shared by the chat skill block and the web page.
//!
//! Reads one date column of a CSV and appends fiscal-quarter / fiscal-year label
//! columns for a fiscal year that starts in any month. The two hard parts are not
//! the arithmetic:
//!
//! 1. **Which year is "FY2026"?** Spreadsheet recipes label a fiscal year by the
//!    calendar year it BEGINS in; the US federal government and pandas label it by
//!    the year it ENDS in. Both are in use and they disagree by one, so the choice
//!    is an explicit parameter (`fiscal_year_naming`) rather than a silent default.
//! 2. **What does `03/04/2024` mean?** The column is read in full first: rows that
//!    can only be one thing (a day above 12) settle day-first vs month-first for the
//!    whole column, and the audit report says which order was used and why.
//!
//! Pure-Rust (`csv` + `chrono`); no clock is read on any surface — every value comes
//! from the parsed cell, so the same input always produces the same output.

use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use std::collections::BTreeMap;

/// Hard cap on the input size, so a huge paste can't exhaust memory. The chat/CLI
/// schema and the page copy advertise this same bound.
pub const MAX_BYTES: usize = 5_000_000;

/// Most failing values listed in the report.
const FAILURE_CAP: usize = 20;

/// Fraction of a column's non-blank values that must parse as a date before
/// auto-detection will consider the column at all. Among the columns that clear
/// it, the one with the most readable dates wins (ties go to the leftmost).
const AUTO_DETECT_RATIO: f64 = 0.5;

/// Century cut-off for two-digit years (POSIX/Excel rule: 68 -> 2068, 69 -> 1969).
const YEAR_PIVOT: i32 = 68;

// ---------------------------------------------------------------------------
// options
// ---------------------------------------------------------------------------

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

const QUARTER_LABELS: [&str; 6] = ["q-fy", "fy-q", "yyyy-qn", "yyyyqn", "qn", "n"];
const YEAR_LABELS: [&str; 5] = ["fy-yyyy", "yyyy", "fy-yy", "range", "range-short"];

/// Day/month reading order for an all-numeric value like `03/04/2024`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DateOrder {
    Auto,
    DayFirst,
    MonthFirst,
}

fn parse_order(s: &str) -> Result<DateOrder, String> {
    Ok(match s.trim() {
        "" | "auto" => DateOrder::Auto,
        "day-first" => DateOrder::DayFirst,
        "month-first" => DateOrder::MonthFirst,
        other => {
            return Err(format!(
                "unknown date_order '{other}'; expected auto, day-first, or month-first"
            ))
        }
    })
}

fn order_label(o: DateOrder) -> &'static str {
    match o {
        DateOrder::DayFirst => "day-first",
        _ => "month-first",
    }
}

/// Map a fiscal-start month name (or a 1-12 number) to its month index 1..=12.
fn parse_start_month(s: &str) -> Result<u32, String> {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        return Ok(1);
    }
    if let Ok(n) = t.parse::<u32>() {
        if (1..=12).contains(&n) {
            return Ok(n);
        }
        return Err(format!(
            "fiscal_start_month must be between 1 and 12, got {n}"
        ));
    }
    match MONTHS.iter().position(|m| *m == t || m.starts_with(&t) && t.len() >= 3) {
        Some(i) => Ok(i as u32 + 1),
        None => Err(format!(
            "unknown fiscal_start_month '{}'; expected a month name (january … december) or 1-12",
            s.trim()
        )),
    }
}

// ---------------------------------------------------------------------------
// delimiters
// ---------------------------------------------------------------------------

/// Map a delimiter name (or a literal single char) to its byte. `auto` yields the
/// 0 sentinel, which the caller resolves by sniffing the first non-blank line.
fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d.trim() {
        "" | "auto" => 0,
        "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be auto/comma/tab/semicolon/pipe or a single character, got '{other}'"
                ));
            }
        }
    })
}

/// Sniff the delimiter from the first non-blank line: the candidate with the most
/// occurrences wins, ties broken by the preference order below, comma by default.
fn detect_delimiter(text: &str) -> u8 {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut best = (b',', 0usize);
    for cand in [b',', b'\t', b';', b'|'] {
        let n = line.bytes().filter(|b| *b == cand).count();
        if n > best.1 {
            best = (cand, n);
        }
    }
    best.0
}

// ---------------------------------------------------------------------------
// date parsing
// ---------------------------------------------------------------------------

/// A cell that parsed. `Fixed` needed no day/month guess; `Ambiguous` holds the two
/// leading numbers of something like `03/04/2024`, where both are 12 or less.
#[derive(Clone, Copy, Debug)]
enum Parsed {
    Fixed {
        date: NaiveDate,
        /// The order this value PROVES for its column (a day above 12), if any.
        proves: Option<DateOrder>,
    },
    Ambiguous {
        a: u32,
        b: u32,
        year: i32,
    },
}

/// Expand a two-digit year through the POSIX/Excel century pivot.
fn expand_year(n: i32) -> i32 {
    if n >= 100 {
        n
    } else if n <= YEAR_PIVOT {
        2000 + n
    } else {
        1900 + n
    }
}

fn month_from_name(tok: &str) -> Option<u32> {
    let t: String = tok
        .trim_matches(|c: char| !c.is_ascii_alphabetic())
        .to_ascii_lowercase();
    if t.len() < 3 {
        return None;
    }
    MONTHS
        .iter()
        .position(|m| m.starts_with(&t) || t.starts_with(*m))
        .map(|i| i as u32 + 1)
}

/// Strip any clock/timezone/weekday noise so only the date words are left.
/// `2024-01-15T10:30:00Z`, `2024-01-15 10:30:00` and
/// `Mon, 15 Jan 2024 10:30:00 +0000` all reduce to their date part.
fn strip_time(raw: &str) -> String {
    let s = raw.trim();
    // Split an ISO `…T…` only when T sits between two digits (so "Tue" survives).
    let bytes = s.as_bytes();
    let mut head = s;
    for (i, c) in s.char_indices() {
        if (c == 'T' || c == 't')
            && i > 0
            && bytes[i - 1].is_ascii_digit()
            && bytes.get(i + 1).is_some_and(|b| b.is_ascii_digit())
        {
            head = &s[..i];
            break;
        }
    }
    let mut kept: Vec<&str> = Vec::new();
    for (i, tok) in head.split_whitespace().enumerate() {
        let low = tok.to_ascii_lowercase();
        let stripped = low.trim_matches(|c: char| c == ',' || c == '.');
        if tok.contains(':')
            || matches!(stripped, "am" | "pm" | "z" | "utc" | "gmt")
            || (i == 0
                && matches!(
                    stripped,
                    "mon" | "tue" | "tues" | "wed" | "thu" | "thur" | "thurs" | "fri" | "sat"
                        | "sun" | "monday" | "tuesday" | "wednesday" | "thursday" | "friday"
                        | "saturday" | "sunday"
                ))
            || ((tok.starts_with('+') || tok.starts_with('-'))
                && tok[1..].chars().all(|c| c.is_ascii_digit())
                && tok.len() >= 5)
        {
            continue;
        }
        kept.push(tok);
    }
    kept.join(" ")
}

/// Build a date, refusing impossible calendar days (2021-02-30 is an error, never
/// rolled over to 2021-03-02).
fn ymd(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, day)
}

/// Parse one cell. Returns `None` when the value is not a date this tool reads.
fn parse_cell(raw: &str) -> Option<Parsed> {
    let cleaned = strip_time(raw);
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return None;
    }

    // ---- written month names: "15 Jan 2024", "Jan 15, 2024", "March 2024" ----
    if cleaned.chars().any(|c| c.is_ascii_alphabetic()) {
        let toks: Vec<&str> = cleaned
            .split(|c: char| c.is_whitespace() || c == '-' || c == '/')
            .filter(|t| !t.is_empty())
            .collect();
        let mut month = None;
        let mut nums: Vec<i64> = Vec::new();
        for tok in &toks {
            if let Some(m) = month_from_name(tok) {
                if month.is_none() {
                    month = Some(m);
                    continue;
                }
                return None;
            }
            // Numbers may carry an ordinal suffix or a trailing comma: 15th, 15,
            let digits: String = tok.chars().take_while(|c| c.is_ascii_digit()).collect();
            let rest: String = tok
                .chars()
                .skip(digits.len())
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            if digits.is_empty() || !matches!(rest.as_str(), "" | "st" | "nd" | "rd" | "th") {
                return None;
            }
            nums.push(digits.parse().ok()?);
        }
        let month = month?;
        let (day, year) = match nums.len() {
            // "March 2024" — a month with no day maps from the 1st.
            1 => (1i64, nums[0]),
            // "2024 Mar 3" puts the year first; "3 Mar 2024" and "Mar 3, 2024"
            // put the day first. No day is above 31, so the split is exact.
            2 => {
                if nums[0] > 31 {
                    (nums[1], nums[0])
                } else {
                    (nums[0], nums[1])
                }
            }
            _ => return None,
        };
        let year = expand_year(i32::try_from(year).ok()?);
        let date = ymd(year, month, u32::try_from(day).ok()?)?;
        return Some(Parsed::Fixed { date, proves: None });
    }

    // ---- all-numeric ----
    let parts: Vec<&str> = cleaned
        .split(['-', '/', '.'])
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() == 1 {
        // Compact YYYYMMDD.
        let d = parts[0];
        if d.len() == 8 && d.chars().all(|c| c.is_ascii_digit()) {
            let year: i32 = d[0..4].parse().ok()?;
            let month: u32 = d[4..6].parse().ok()?;
            let day: u32 = d[6..8].parse().ok()?;
            return ymd(year, month, day).map(|date| Parsed::Fixed { date, proves: None });
        }
        return None;
    }
    if !parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    if parts.len() == 2 {
        // Month precision: 2024-01 maps from the 1st. (01-2024 too.)
        let (y, m) = if parts[0].len() == 4 {
            (parts[0], parts[1])
        } else if parts[1].len() == 4 {
            (parts[1], parts[0])
        } else {
            return None;
        };
        let year: i32 = y.parse().ok()?;
        let month: u32 = m.parse().ok()?;
        return ymd(year, month, 1).map(|date| Parsed::Fixed { date, proves: None });
    }
    if parts.len() != 3 {
        return None;
    }
    // Year-first (ISO-ish) settles itself.
    if parts[0].len() == 4 {
        let year: i32 = parts[0].parse().ok()?;
        let month: u32 = parts[1].parse().ok()?;
        let day: u32 = parts[2].parse().ok()?;
        return ymd(year, month, day).map(|date| Parsed::Fixed { date, proves: None });
    }
    let year = expand_year(parts[2].parse::<i32>().ok()?);
    let a: u32 = parts[0].parse().ok()?;
    let b: u32 = parts[1].parse().ok()?;
    match (a > 12, b > 12) {
        // Only day-first can read this: the first number is above 12.
        (true, false) => ymd(year, b, a).map(|date| Parsed::Fixed {
            date,
            proves: Some(DateOrder::DayFirst),
        }),
        // Only month-first can read this.
        (false, true) => ymd(year, a, b).map(|date| Parsed::Fixed {
            date,
            proves: Some(DateOrder::MonthFirst),
        }),
        (false, false) => Some(Parsed::Ambiguous { a, b, year }),
        (true, true) => None,
    }
}

/// Resolve a parsed cell against the order settled for its column. A value that
/// can only be read in the OTHER order (`25/12` under month-first) is not
/// silently re-read the way it wants — it is unreadable in this column's order,
/// so the caller's `on_error` policy applies to it.
fn resolve(p: Parsed, order: DateOrder) -> Option<NaiveDate> {
    match p {
        Parsed::Fixed {
            proves: Some(p), ..
        } if p != order => None,
        Parsed::Fixed { date, .. } => Some(date),
        Parsed::Ambiguous { a, b, year } => match order {
            DateOrder::DayFirst => ymd(year, b, a),
            _ => ymd(year, a, b),
        },
    }
}

// ---------------------------------------------------------------------------
// fiscal arithmetic
// ---------------------------------------------------------------------------

/// Everything the label renderers need about one date's fiscal position.
#[derive(Clone, Copy, Debug)]
struct Fiscal {
    /// 1..=4.
    quarter: u32,
    /// 1..=12, counted from the fiscal-year start month.
    month: u32,
    /// Calendar year the fiscal year BEGINS in.
    start_year: i32,
    /// Calendar year the fiscal year ENDS in (= start_year when the FY starts in January).
    end_year: i32,
    quarter_start: NaiveDate,
    quarter_end: NaiveDate,
}

/// Add `months` whole months to (year, month), returning the 1st of that month.
fn add_months(year: i32, month: u32, months: u32) -> NaiveDate {
    let zero = (month - 1) + months;
    let y = year + (zero / 12) as i32;
    let m = zero % 12 + 1;
    NaiveDate::from_ymd_opt(y, m, 1).expect("month arithmetic stays in range")
}

fn fiscal_of(date: NaiveDate, start_month: u32) -> Fiscal {
    let offset = (date.month() + 12 - start_month) % 12; // 0..=11
    let quarter = offset / 3 + 1;
    let start_year = if date.month() >= start_month {
        date.year()
    } else {
        date.year() - 1
    };
    let end_year = if start_month == 1 {
        start_year
    } else {
        start_year + 1
    };
    let quarter_start = add_months(start_year, start_month, (quarter - 1) * 3);
    let quarter_end = add_months(start_year, start_month, quarter * 3)
        .pred_opt()
        .expect("a quarter always has a day before its successor");
    Fiscal {
        quarter,
        month: offset + 1,
        start_year,
        end_year,
        quarter_start,
        quarter_end,
    }
}

/// Render the fiscal-year label. `range`/`range-short` collapse to a single year
/// when the fiscal year is the calendar year (a January start).
fn year_label(f: &Fiscal, style: &str, naming_end: bool) -> String {
    let n = if naming_end { f.end_year } else { f.start_year };
    match style {
        "yyyy" => n.to_string(),
        "fy-yy" => format!("FY{:02}", n.rem_euclid(100)),
        "range" => {
            if f.start_year == f.end_year {
                n.to_string()
            } else {
                format!("{}-{}", f.start_year, f.end_year)
            }
        }
        "range-short" => {
            if f.start_year == f.end_year {
                n.to_string()
            } else {
                format!("{}-{:02}", f.start_year, f.end_year.rem_euclid(100))
            }
        }
        _ => format!("FY{n}"),
    }
}

/// Render the quarter label. `q-fy`/`fy-q` embed the `year_label` rendering, so
/// `Q3 FY26` and `Q3 2025-2026` are reachable; `yyyy-qn`/`yyyyqn` are the fixed
/// pandas-style forms and always use the bare four-digit fiscal year.
fn quarter_label(f: &Fiscal, style: &str, ystyle: &str, naming_end: bool) -> String {
    let q = f.quarter;
    let bare = if naming_end { f.end_year } else { f.start_year };
    match style {
        "fy-q" => format!("{}-Q{q}", year_label(f, ystyle, naming_end)),
        "yyyy-qn" => format!("{bare}-Q{q}"),
        "yyyyqn" => format!("{bare}Q{q}"),
        "qn" => format!("Q{q}"),
        "n" => q.to_string(),
        _ => format!("Q{q} {}", year_label(f, ystyle, naming_end)),
    }
}

fn iso(d: NaiveDate) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day())
}

// ---------------------------------------------------------------------------
// report
// ---------------------------------------------------------------------------

/// One cell that could not be read as a date.
#[derive(Serialize, Clone)]
pub struct Failure {
    /// 1-based line in the input text.
    pub line: usize,
    /// 1-based data row (the header does not count).
    pub row: usize,
    /// The offending text.
    pub value: String,
}

/// The audit returned by `output = "report"` / `"json"`.
#[derive(Serialize)]
pub struct Report {
    /// The date column that was mapped.
    pub column: String,
    /// Its 0-based index.
    pub column_index: usize,
    /// The fiscal-year start month, as a name.
    pub fiscal_start_month: String,
    /// `end` or `start` — which calendar year names the fiscal year.
    pub fiscal_year_naming: String,
    /// Day/month order used for ambiguous all-numeric values.
    pub date_order: String,
    /// `explicit`, `detected`, `default`, or `conflict`.
    pub order_source: String,
    /// Data rows read.
    pub rows: usize,
    /// Rows that got a fiscal label.
    pub mapped: usize,
    /// Rows whose date cell was blank.
    pub blank: usize,
    /// Rows whose date cell could not be read.
    pub failed: usize,
    /// Rows dropped (only with `on_error = "drop"`).
    pub dropped: usize,
    /// How many rows landed in each quarter label, most common first.
    pub quarters: Vec<(String, usize)>,
    /// Up to 20 unreadable values.
    pub failures: Vec<Failure>,
    /// The columns appended to every row.
    pub added_columns: Vec<String>,
    /// The rewritten CSV.
    pub csv: String,
}

fn render_report(r: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "date column: {} (index {})\n",
        r.column, r.column_index
    ));
    out.push_str(&format!(
        "fiscal year: starts in {}, named by the {} year\n",
        r.fiscal_start_month, r.fiscal_year_naming
    ));
    let why = match r.order_source.as_str() {
        "explicit" => "you set it",
        "detected" => "proven by a value that can only be read one way",
        "conflict" => "CONFLICT — this column proves BOTH orders; month-first was used, check the data",
        _ => "no value in the column proves either order, so the default was used",
    };
    out.push_str(&format!("date order: {} ({why})\n", r.date_order));
    out.push_str(&format!("added columns: {}\n", r.added_columns.join(", ")));
    out.push_str(&format!(
        "\nrows: {} — mapped {}, blank {}, unreadable {}, dropped {}\n",
        r.rows, r.mapped, r.blank, r.failed, r.dropped
    ));
    if !r.quarters.is_empty() {
        out.push_str("\nrows per quarter:\n");
        for (label, n) in &r.quarters {
            out.push_str(&format!("  {label}: {n}\n"));
        }
    }
    if !r.failures.is_empty() {
        out.push_str(&format!(
            "\nunreadable values (first {}):\n",
            r.failures.len()
        ));
        for f in &r.failures {
            out.push_str(&format!("  row {} (line {}): {}\n", f.row, f.line, f.value));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

/// Map a CSV date column to fiscal quarter / fiscal year label columns.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    column_spec: &str,
    fiscal_start_month: &str,
    fiscal_year_naming: &str,
    quarter_label_style: &str,
    fiscal_year_label_style: &str,
    add_fiscal_year: bool,
    add_quarter_dates: bool,
    add_fiscal_month: bool,
    add_quarter_position: bool,
    date_order: &str,
    on_error: &str,
    has_header: bool,
    delimiter: &str,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if data.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {MAX_BYTES} bytes",
            data.len()
        ));
    }
    let start_month = parse_start_month(fiscal_start_month)?;
    let naming_end = match fiscal_year_naming.trim() {
        "" | "end" => true,
        "start" => false,
        other => {
            return Err(format!(
                "unknown fiscal_year_naming '{other}'; expected end (FY named by the year it ends in) or start"
            ))
        }
    };
    let qstyle = match quarter_label_style.trim() {
        "" => "q-fy",
        v if QUARTER_LABELS.contains(&v) => v,
        other => {
            return Err(format!(
                "unknown quarter_label '{other}'; expected one of {}",
                QUARTER_LABELS.join(", ")
            ))
        }
    };
    let ystyle = match fiscal_year_label_style.trim() {
        "" => "fy-yyyy",
        v if YEAR_LABELS.contains(&v) => v,
        other => {
            return Err(format!(
                "unknown fiscal_year_label '{other}'; expected one of {}",
                YEAR_LABELS.join(", ")
            ))
        }
    };
    let on_error = match on_error.trim() {
        "" => "blank",
        v @ ("blank" | "drop" | "error") => v,
        other => {
            return Err(format!(
                "unknown on_error '{other}'; expected blank, drop, or error"
            ))
        }
    };
    let output = match output.trim() {
        "" => "csv",
        v @ ("csv" | "report" | "json") => v,
        other => {
            return Err(format!(
                "unknown output '{other}'; expected csv, report, or json"
            ))
        }
    };
    let requested_order = parse_order(date_order)?;

    let delim = match delim_byte(delimiter)? {
        0 => detect_delimiter(data),
        b => b,
    };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());

    let mut records: Vec<(usize, csv::StringRecord)> = Vec::new();
    let mut rec = csv::StringRecord::new();
    loop {
        match rdr.read_record(&mut rec) {
            Ok(true) => {
                let line = rec.position().map(|p| p.line() as usize).unwrap_or(0);
                records.push((line, rec.clone()));
            }
            Ok(false) => break,
            Err(e) => return Err(format!("CSV parse error: {e}")),
        }
    }
    if records.is_empty() {
        return Err("no rows found".into());
    }

    let width = records.iter().map(|(_, r)| r.len()).max().unwrap_or(0);
    let header = records[0].1.clone();
    let column_names: Vec<String> = (0..width)
        .map(|i| {
            let name = if has_header {
                header.get(i).unwrap_or("").trim()
            } else {
                ""
            };
            if name.is_empty() {
                format!("col{}", i + 1)
            } else {
                name.to_string()
            }
        })
        .collect();

    let data_rows: Vec<(usize, csv::StringRecord)> = if has_header {
        records[1..].to_vec()
    } else {
        records.clone()
    };
    if data_rows.is_empty() {
        return Err(
            "no data rows found (the input has only a header row). Turn off 'First row is a header' for a bare list of dates."
                .into(),
        );
    }

    let target = resolve_column(column_spec, &column_names, has_header, &data_rows)?;

    // Pass 1 — settle day-first vs month-first from the values that can only be
    // read one way.
    let (order, order_source) = if requested_order != DateOrder::Auto {
        (requested_order, "explicit")
    } else {
        let mut day_votes = 0usize;
        let mut month_votes = 0usize;
        for (_, rec) in &data_rows {
            match parse_cell(rec.get(target).unwrap_or("")) {
                Some(Parsed::Fixed {
                    proves: Some(DateOrder::DayFirst),
                    ..
                }) => day_votes += 1,
                Some(Parsed::Fixed {
                    proves: Some(DateOrder::MonthFirst),
                    ..
                }) => month_votes += 1,
                _ => {}
            }
        }
        match (day_votes, month_votes) {
            (0, 0) => (DateOrder::MonthFirst, "default"),
            (_, 0) => (DateOrder::DayFirst, "detected"),
            (0, _) => (DateOrder::MonthFirst, "detected"),
            // The same column proves BOTH orders — genuinely mixed data. Fall back
            // to month-first and flag it loudly in the report.
            _ => (DateOrder::MonthFirst, "conflict"),
        }
    };

    // The appended header names, in the order they are written.
    let mut added: Vec<String> = vec!["fiscal_quarter".to_string()];
    if add_fiscal_year {
        added.push("fiscal_year".to_string());
    }
    if add_quarter_dates {
        added.push("fiscal_quarter_start".to_string());
        added.push("fiscal_quarter_end".to_string());
    }
    if add_fiscal_month {
        added.push("fiscal_month".to_string());
    }
    if add_quarter_position {
        added.push("day_of_quarter".to_string());
        added.push("days_in_quarter".to_string());
    }
    let extra = added.len();

    // Pass 2 — map.
    let mut rows_out: Vec<Vec<String>> = Vec::with_capacity(data_rows.len());
    let mut failures: Vec<Failure> = Vec::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let (mut mapped, mut blank, mut failed, mut dropped) = (0usize, 0usize, 0usize, 0usize);

    for (row_i, (line, rec)) in data_rows.iter().enumerate() {
        let mut fields: Vec<String> = rec.iter().map(|f| f.to_string()).collect();
        while fields.len() < width {
            fields.push(String::new());
        }
        let cell = fields.get(target).cloned().unwrap_or_default();
        let trimmed = cell.trim();
        let date = if trimmed.is_empty() {
            blank += 1;
            None
        } else {
            match parse_cell(trimmed).and_then(|p| resolve(p, order)) {
                Some(d) => Some(d),
                None => {
                    failed += 1;
                    if failures.len() < FAILURE_CAP {
                        failures.push(Failure {
                            line: *line,
                            row: row_i + 1,
                            value: trimmed.to_string(),
                        });
                    }
                    match on_error {
                        "error" => {
                            return Err(format!(
                                "row {} (line {line}), column '{}': cannot read '{trimmed}' as a date. Set 'Unreadable dates' to blank or drop to continue past it.",
                                row_i + 1,
                                column_names[target]
                            ))
                        }
                        "drop" => {
                            dropped += 1;
                            continue;
                        }
                        _ => None,
                    }
                }
            }
        };

        match date {
            Some(d) => {
                mapped += 1;
                let f = fiscal_of(d, start_month);
                let label = quarter_label(&f, qstyle, ystyle, naming_end);
                *counts.entry(label.clone()).or_insert(0) += 1;
                fields.push(label);
                if add_fiscal_year {
                    fields.push(year_label(&f, ystyle, naming_end));
                }
                if add_quarter_dates {
                    fields.push(iso(f.quarter_start));
                    fields.push(iso(f.quarter_end));
                }
                if add_fiscal_month {
                    fields.push(f.month.to_string());
                }
                if add_quarter_position {
                    let day_of = (d - f.quarter_start).num_days() + 1;
                    let days_in = (f.quarter_end - f.quarter_start).num_days() + 1;
                    fields.push(day_of.to_string());
                    fields.push(days_in.to_string());
                }
            }
            None => {
                for _ in 0..extra {
                    fields.push(String::new());
                }
            }
        }
        rows_out.push(fields);
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(Vec::new());
    if has_header {
        let mut head: Vec<String> = (0..width)
            .map(|i| header.get(i).unwrap_or("").to_string())
            .collect();
        head.extend(added.iter().cloned());
        wtr.write_record(&head)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for fields in &rows_out {
        wtr.write_record(fields)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let csv_out = String::from_utf8(
        wtr.into_inner()
            .map_err(|e| format!("CSV write error: {e}"))?,
    )
    .map_err(|e| format!("CSV write error: {e}"))?;

    let mut quarters: Vec<(String, usize)> = counts.into_iter().collect();
    quarters.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let report = Report {
        column: column_names[target].clone(),
        column_index: target,
        fiscal_start_month: MONTHS[start_month as usize - 1].to_string(),
        fiscal_year_naming: if naming_end { "end" } else { "start" }.to_string(),
        date_order: order_label(order).to_string(),
        order_source: order_source.to_string(),
        rows: data_rows.len(),
        mapped,
        blank,
        failed,
        dropped,
        quarters,
        failures,
        added_columns: added,
        csv: csv_out,
    };

    match output {
        "csv" => Ok(report.csv),
        "json" => serde_json::to_string_pretty(&report).map_err(|e| e.to_string()),
        _ => Ok(render_report(&report)),
    }
}

/// Pick the date column: an explicit header name / 0-based index, or `auto` —
/// the column with the MOST readable dates, among those where at least half the
/// non-blank cells parse. Ties go to the leftmost column, so a table with both
/// `created` and `closed` maps `created` unless you name the other.
fn resolve_column(
    spec: &str,
    columns: &[String],
    has_header: bool,
    rows: &[(usize, csv::StringRecord)],
) -> Result<usize, String> {
    let spec = spec.trim();
    if spec.is_empty() || spec == "auto" {
        let mut best: Option<(usize, usize)> = None; // (dates, index)
        for i in 0..columns.len() {
            let mut non_blank = 0usize;
            let mut dates = 0usize;
            for (_, rec) in rows {
                let cell = rec.get(i).unwrap_or("").trim();
                if cell.is_empty() {
                    continue;
                }
                non_blank += 1;
                if parse_cell(cell).is_some() {
                    dates += 1;
                }
            }
            if dates == 0 || (dates as f64) < AUTO_DETECT_RATIO * non_blank as f64 {
                continue;
            }
            if best.is_none_or(|(bd, _)| dates > bd) {
                best = Some((dates, i));
            }
        }
        return match best {
            Some((_, i)) => Ok(i),
            None => Err(
                "no date column detected; name one explicitly (a header name or a 0-based index)"
                    .into(),
            ),
        };
    }
    if let Ok(n) = spec.parse::<usize>() {
        if n >= columns.len() {
            return Err(format!(
                "column index {n} out of range (0..={})",
                columns.len().saturating_sub(1)
            ));
        }
        return Ok(n);
    }
    if !has_header {
        return Err(format!(
            "with 'First row is a header' off, column must be a 0-based index, got '{spec}'"
        ));
    }
    columns
        .iter()
        .position(|c| c == spec)
        .ok_or_else(|| format!("column '{spec}' not found in header ({})", columns.join(", ")))
}

/// The 12 month names, exposed so the block/web layers can validate without
/// duplicating the list.
pub fn month_names() -> &'static [&'static str; 12] {
    &MONTHS
}

/// The three-letter month abbreviations, in calendar order.
pub fn month_abbr() -> &'static [&'static str; 12] {
    &MONTH_ABBR
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(data: &str, start: &str) -> String {
        run(
            data, "auto", start, "end", "q-fy", "fy-yyyy", true, false, false, false, "auto",
            "blank", true, "auto", "csv",
        )
        .unwrap()
    }

    #[test]
    fn us_federal_october_start_labels_by_ending_year() {
        // FY2026 for the US federal government runs Oct 2025 - Sep 2026.
        let out = map("id,d\n1,2025-10-01\n2,2026-04-25\n3,2026-09-30\n", "october");
        assert_eq!(
            out,
            "id,d,fiscal_quarter,fiscal_year\n\
             1,2025-10-01,Q1 FY2026,FY2026\n\
             2,2026-04-25,Q3 FY2026,FY2026\n\
             3,2026-09-30,Q4 FY2026,FY2026\n"
        );
    }

    #[test]
    fn january_start_is_the_calendar_year() {
        let out = map("d\n2024-01-01\n2024-04-01\n2024-07-01\n2024-10-01\n", "january");
        assert_eq!(
            out,
            "d,fiscal_quarter,fiscal_year\n\
             2024-01-01,Q1 FY2024,FY2024\n\
             2024-04-01,Q2 FY2024,FY2024\n\
             2024-07-01,Q3 FY2024,FY2024\n\
             2024-10-01,Q4 FY2024,FY2024\n"
        );
    }

    #[test]
    fn naming_start_shifts_the_year_label_by_one() {
        let out = run(
            "d\n2025-10-01\n", "auto", "october", "start", "q-fy", "fy-yyyy", true, false, false,
            false, "auto", "blank", true, "auto", "csv",
        )
        .unwrap();
        assert_eq!(out, "d,fiscal_quarter,fiscal_year\n2025-10-01,Q1 FY2025,FY2025\n");
    }

    #[test]
    fn uk_april_start_boundary() {
        // 2024-03-31 is the last day of the UK FY that began April 2023.
        let out = map("d\n2024-03-31\n2024-04-01\n", "april");
        assert_eq!(
            out,
            "d,fiscal_quarter,fiscal_year\n\
             2024-03-31,Q4 FY2024,FY2024\n\
             2024-04-01,Q1 FY2025,FY2025\n"
        );
    }

    #[test]
    fn pandas_style_label_and_range_year() {
        let out = run(
            "d\n2025-07-01\n",
            "d",
            "july",
            "end",
            "yyyyqn",
            "range",
            true,
            false,
            false,
            false,
            "auto",
            "blank",
            true,
            "auto",
            "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "d,fiscal_quarter,fiscal_year\n2025-07-01,2026Q1,2025-2026\n"
        );
    }

    #[test]
    fn quarter_dates_month_and_position_columns() {
        let out = run(
            "d\n2026-04-25\n",
            "d",
            "october",
            "end",
            "q-fy",
            "fy-yyyy",
            false,
            true,
            true,
            true,
            "auto",
            "blank",
            true,
            "auto",
            "csv",
        )
        .unwrap();
        // Oct-start FY: Q3 is Apr 1 - Jun 30 (91 days); 25 Apr is day 25;
        // fiscal month 7 (Oct=1).
        assert_eq!(
            out,
            "d,fiscal_quarter,fiscal_quarter_start,fiscal_quarter_end,fiscal_month,day_of_quarter,days_in_quarter\n\
             2026-04-25,Q3 FY2026,2026-04-01,2026-06-30,7,25,91\n"
        );
    }

    #[test]
    fn ambiguous_column_is_settled_by_an_unambiguous_row() {
        // 25/12/2024 can only be day-first, so 03/04/2024 is 3 April, not 4 March.
        let out = map("d\n03/04/2024\n25/12/2024\n", "january");
        assert_eq!(
            out,
            "d,fiscal_quarter,fiscal_year\n\
             03/04/2024,Q2 FY2024,FY2024\n\
             25/12/2024,Q4 FY2024,FY2024\n"
        );
    }

    #[test]
    fn forced_month_first_reads_the_same_value_as_march() {
        let out = run(
            "d\n03/04/2024\n25/12/2024\n",
            "d",
            "january",
            "end",
            "qn",
            "fy-yyyy",
            false,
            false,
            false,
            false,
            "month-first",
            "blank",
            true,
            "auto",
            "csv",
        )
        .unwrap();
        // 03/04 -> 4 March -> Q1; 25/12 is impossible month-first -> unreadable.
        assert_eq!(out, "d,fiscal_quarter\n03/04/2024,Q1\n25/12/2024,\n");
    }

    #[test]
    fn written_months_compact_and_month_precision_parse() {
        let out = map(
            "d\n15 Jan 2024\n\"Mar 3, 2024\"\n20240715\n2024-11\nOctober 2024\n",
            "january",
        );
        assert_eq!(
            out,
            "d,fiscal_quarter,fiscal_year\n\
             15 Jan 2024,Q1 FY2024,FY2024\n\
             \"Mar 3, 2024\",Q1 FY2024,FY2024\n\
             20240715,Q3 FY2024,FY2024\n\
             2024-11,Q4 FY2024,FY2024\n\
             October 2024,Q4 FY2024,FY2024\n"
        );
    }

    #[test]
    fn timestamps_lose_their_clock_part() {
        let out = map("d\n2024-01-15T10:30:00Z\n\"Mon, 15 Jan 2024 10:30:00 +0000\"\n", "january");
        assert_eq!(
            out,
            "d,fiscal_quarter,fiscal_year\n\
             2024-01-15T10:30:00Z,Q1 FY2024,FY2024\n\
             \"Mon, 15 Jan 2024 10:30:00 +0000\",Q1 FY2024,FY2024\n"
        );
    }

    #[test]
    fn unreadable_cell_is_blanked_dropped_or_raised() {
        let blanked = map("d\n2024-01-15\nnot a date\n", "january");
        assert_eq!(
            blanked,
            "d,fiscal_quarter,fiscal_year\n2024-01-15,Q1 FY2024,FY2024\nnot a date,,\n"
        );
        let dropped = run(
            "d\n2024-01-15\nnot a date\n", "d", "january", "end", "q-fy", "fy-yyyy", true, false,
            false, false, "auto", "drop", true, "auto", "csv",
        )
        .unwrap();
        assert_eq!(
            dropped,
            "d,fiscal_quarter,fiscal_year\n2024-01-15,Q1 FY2024,FY2024\n"
        );
    }

    #[test]
    fn error_mode_names_the_row_and_line() {
        let err = run(
            "d\n2024-01-15\nnot a date\n", "d", "january", "end", "q-fy", "fy-yyyy", true, false,
            false, false, "auto", "error", true, "auto", "csv",
        )
        .unwrap_err();
        assert!(err.contains("row 2"), "{err}");
        assert!(err.contains("line 3"), "{err}");
        assert!(err.contains("not a date"), "{err}");
    }

    #[test]
    fn impossible_calendar_date_is_refused_not_rolled_over() {
        let out = map("d\n2021-02-30\n2021-02-28\n", "january");
        assert_eq!(
            out,
            "d,fiscal_quarter,fiscal_year\n2021-02-30,,\n2021-02-28,Q1 FY2021,FY2021\n"
        );
    }

    #[test]
    fn headerless_input_uses_indexes_and_emits_no_header() {
        let out = run(
            "2024-01-15\n2024-05-02\n", "0", "april", "end", "qn", "fy-yyyy", false, false, false,
            false, "auto", "blank", false, "auto", "csv",
        )
        .unwrap();
        assert_eq!(out, "2024-01-15,Q4\n2024-05-02,Q1\n");
    }

    #[test]
    fn tab_delimited_input_round_trips_as_tsv() {
        let out = run(
            "id\td\n1\t2024-02-01\n", "d", "january", "end", "qn", "fy-yyyy", false, false, false,
            false, "auto", "blank", true, "auto", "csv",
        )
        .unwrap();
        assert_eq!(out, "id\td\tfiscal_quarter\n1\t2024-02-01\tQ1\n");
    }

    #[test]
    fn report_states_the_order_evidence_and_quarter_counts() {
        let out = run(
            "d\n03/04/2024\n25/12/2024\n", "d", "january", "end", "q-fy", "fy-yyyy", true, false,
            false, false, "auto", "blank", true, "auto", "report",
        )
        .unwrap();
        assert!(out.contains("date order: day-first (proven"), "{out}");
        assert!(out.contains("Q2 FY2024: 1"), "{out}");
        assert!(out.contains("rows: 2 — mapped 2"), "{out}");
    }

    #[test]
    fn json_output_carries_the_audit_and_the_csv() {
        let out = run(
            "d\n2024-01-15\n", "d", "october", "end", "q-fy", "fy-yyyy", true, false, false, false,
            "auto", "blank", true, "auto", "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["fiscal_start_month"], "october");
        assert_eq!(v["fiscal_year_naming"], "end");
        assert_eq!(v["mapped"], 1);
        assert!(v["csv"].as_str().unwrap().contains("Q2 FY2024"));
    }

    #[test]
    fn every_quarter_label_style_renders() {
        let styles = [
            ("q-fy", "Q1 FY2026"),
            ("fy-q", "FY2026-Q1"),
            ("yyyy-qn", "2026-Q1"),
            ("yyyyqn", "2026Q1"),
            ("qn", "Q1"),
            ("n", "1"),
        ];
        for (style, expected) in styles {
            let out = run(
                "d\n2025-10-01\n", "d", "october", "end", style, "fy-yyyy", false, false, false,
                false, "auto", "blank", true, "auto", "csv",
            )
            .unwrap();
            assert_eq!(out, format!("d,fiscal_quarter\n2025-10-01,{expected}\n"));
        }
    }

    #[test]
    fn every_fiscal_year_label_style_renders() {
        let styles = [
            ("fy-yyyy", "FY2026"),
            ("yyyy", "2026"),
            ("fy-yy", "FY26"),
            ("range", "2025-2026"),
            ("range-short", "2025-26"),
        ];
        for (style, expected) in styles {
            let out = run(
                "d\n2025-10-01\n", "d", "october", "end", "qn", style, true, false, false, false,
                "auto", "blank", true, "auto", "csv",
            )
            .unwrap();
            assert_eq!(
                out,
                format!("d,fiscal_quarter,fiscal_year\n2025-10-01,Q1,{expected}\n")
            );
        }
    }

    #[test]
    fn january_start_collapses_the_range_label_to_one_year() {
        let out = run(
            "d\n2024-05-01\n", "d", "january", "end", "qn", "range", true, false, false, false,
            "auto", "blank", true, "auto", "csv",
        )
        .unwrap();
        assert_eq!(out, "d,fiscal_quarter,fiscal_year\n2024-05-01,Q2,2024\n");
    }

    #[test]
    fn quarter_lengths_cover_the_leap_year() {
        // Jan-start Q1 is 91 days in 2024 (leap) and 90 in 2023.
        for (date, days) in [("2024-01-01", "91"), ("2023-01-01", "90")] {
            let out = run(
                &format!("d\n{date}\n"),
                "d",
                "january",
                "end",
                "qn",
                "fy-yyyy",
                false,
                false,
                false,
                true,
                "auto",
                "blank",
                true,
                "auto",
                "csv",
            )
            .unwrap();
            assert!(out.ends_with(&format!(",Q1,1,{days}\n")), "{out}");
        }
    }

    #[test]
    fn auto_column_detection_skips_a_non_date_column() {
        let out = map("name,started\nada,2024-08-01\nbob,2024-02-02\n", "january");
        assert_eq!(
            out,
            "name,started,fiscal_quarter,fiscal_year\n\
             ada,2024-08-01,Q3 FY2024,FY2024\n\
             bob,2024-02-02,Q1 FY2024,FY2024\n"
        );
    }

    // ---- error paths ----

    #[test]
    fn empty_input_is_an_error() {
        let err = run(
            "   ", "auto", "january", "end", "q-fy", "fy-yyyy", true, false, false, false, "auto",
            "blank", true, "auto", "csv",
        )
        .unwrap_err();
        assert_eq!(err, "input is empty");
    }

    #[test]
    fn unknown_start_month_is_rejected() {
        let err = run(
            "d\n2024-01-15\n", "d", "smarch", "end", "q-fy", "fy-yyyy", true, false, false, false,
            "auto", "blank", true, "auto", "csv",
        )
        .unwrap_err();
        assert!(err.contains("unknown fiscal_start_month 'smarch'"), "{err}");
    }

    #[test]
    fn unknown_column_names_the_header() {
        let err = run(
            "id,d\n1,2024-01-15\n", "when", "january", "end", "q-fy", "fy-yyyy", true, false,
            false, false, "auto", "blank", true, "auto", "csv",
        )
        .unwrap_err();
        assert_eq!(err, "column 'when' not found in header (id, d)");
    }

    #[test]
    fn a_table_with_no_date_column_is_reported() {
        let err = run(
            "a,b\nx,y\n", "auto", "january", "end", "q-fy", "fy-yyyy", true, false, false, false,
            "auto", "blank", true, "auto", "csv",
        )
        .unwrap_err();
        assert!(err.contains("no date column detected"), "{err}");
    }

    #[test]
    fn unknown_label_styles_and_options_are_rejected() {
        for (q, y, n, e) in [
            ("nope", "fy-yyyy", "end", "unknown quarter_label"),
            ("q-fy", "nope", "end", "unknown fiscal_year_label"),
            ("q-fy", "fy-yyyy", "middle", "unknown fiscal_year_naming"),
        ] {
            let err = run(
                "d\n2024-01-15\n", "d", "january", n, q, y, true, false, false, false, "auto",
                "blank", true, "auto", "csv",
            )
            .unwrap_err();
            assert!(err.contains(e), "{err}");
        }
    }

    #[test]
    fn oversized_input_is_refused() {
        let big = format!("d\n{}", "2024-01-15\n".repeat(MAX_BYTES / 11 + 20));
        let err = run(
            &big, "d", "january", "end", "q-fy", "fy-yyyy", true, false, false, false, "auto",
            "blank", true, "auto", "csv",
        )
        .unwrap_err();
        assert!(err.contains("the limit is 5000000 bytes"), "{err}");
    }

    #[test]
    fn month_helpers_are_in_calendar_order() {
        assert_eq!(month_names()[0], "january");
        assert_eq!(month_abbr()[9], "Oct");
    }
}
