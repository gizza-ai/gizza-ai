//! csv-numeric-column-extractor core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Parses CSV/TSV text, decides which columns are numeric (every non-missing cell
//! parses as a number, or at least `min_numeric_ratio` of them), and returns those
//! columns as typed arrays with their headers — plus a note on every column that was
//! rejected and why.

/// Largest accepted input. Roughly 1 MB of pasted CSV; bigger files should be split.
pub const MAX_INPUT_BYTES: usize = 1_000_000;
/// Tokens treated as missing/null in addition to the empty cell.
pub const DEFAULT_NULL_TOKENS: &str = "NA,N/A,NULL,null,None,nan";

const DELIMS: [(char, &str); 4] = [
    (',', "comma"),
    ('\t', "tab"),
    (';', "semicolon"),
    ('|', "pipe"),
];

/// One column that passed the numeric test.
#[derive(Debug, Clone, PartialEq)]
pub struct NumericColumn {
    pub name: String,
    /// 1-based position in the original CSV.
    pub index: usize,
    /// True when every parsed value is a whole number written without `.`/exponent.
    pub is_integer: bool,
    /// One entry per data row; `None` = missing (or unparsable, when the ratio allows it).
    pub values: Vec<Option<f64>>,
    pub numeric_ratio: f64,
    /// Number of cells that parsed as a number.
    pub count: usize,
    pub missing: usize,
}

/// A column that did NOT qualify, with the reason a user can act on.
#[derive(Debug, Clone, PartialEq)]
pub struct SkippedColumn {
    pub name: String,
    pub index: usize,
    pub reason: String,
    pub example: Option<String>,
    pub numeric_ratio: f64,
}

/// Full result of an extraction run.
#[derive(Debug, Clone, PartialEq)]
pub struct Extraction {
    pub delimiter: &'static str,
    pub header: bool,
    pub rows: usize,
    pub columns_total: usize,
    pub numeric: Vec<NumericColumn>,
    pub skipped: Vec<SkippedColumn>,
}

/// Entry point used by the chat block, the CLI and the browser page.
///
/// `delimiter` = `auto|comma|tab|semicolon|pipe`, `header` = `auto|present|absent`,
/// `output` = `columns|records|csv|names`.
#[allow(clippy::too_many_arguments)]
pub fn extract(
    data: &str,
    delimiter: &str,
    header: &str,
    output: &str,
    null_tokens: &str,
    allow_blanks: bool,
    min_numeric_ratio: f64,
    normalize: bool,
) -> Result<String, String> {
    let output = pick(output, &["columns", "records", "csv", "names"], "columns", "output")?;
    let result = analyze(
        data,
        delimiter,
        header,
        null_tokens,
        allow_blanks,
        min_numeric_ratio,
        normalize,
    )?;
    Ok(match output {
        "records" => render_records(&result),
        "csv" => render_csv(&result),
        "names" => render_names(&result),
        _ => render_columns(&result),
    })
}

/// Parse + classify, without rendering. Exposed so tests (and future callers) can
/// assert on the structure rather than on formatted text.
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    data: &str,
    delimiter: &str,
    header: &str,
    null_tokens: &str,
    allow_blanks: bool,
    min_numeric_ratio: f64,
    normalize: bool,
) -> Result<Extraction, String> {
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the maximum is {MAX_INPUT_BYTES} bytes (about 1 MB) — split the file and run the parts separately",
            data.len()
        ));
    }
    if !(0.1..=1.0).contains(&min_numeric_ratio) {
        return Err(format!(
            "min_numeric_ratio must be between 0.1 and 1.0, got {}",
            fmt_num(min_numeric_ratio)
        ));
    }
    let delimiter = pick(
        delimiter,
        &["auto", "comma", "tab", "semicolon", "pipe"],
        "auto",
        "delimiter",
    )?;
    let header_mode = pick(header, &["auto", "present", "absent"], "auto", "header")?;

    let nulls: Vec<&str> = null_tokens
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();

    let (delim_char, delim_name) = match delimiter {
        "auto" => sniff_delimiter(data),
        other => DELIMS
            .iter()
            .copied()
            .find(|(_, n)| *n == other)
            .expect("delimiter validated above"),
    };

    let grid = parse_csv(data, delim_char);
    if grid.is_empty() {
        return Err("no CSV data: paste at least one row of comma-, tab-, semicolon- or pipe-separated values".into());
    }
    let columns_total = grid.iter().map(Vec::len).max().unwrap_or(0);

    let has_header = match header_mode {
        "present" => true,
        "absent" => false,
        _ => looks_like_header(&grid[0], normalize, &nulls),
    };
    let names: Vec<String> = (0..columns_total)
        .map(|c| {
            let from_header = if has_header {
                grid[0].get(c).map(|s| s.trim()).unwrap_or("")
            } else {
                ""
            };
            if from_header.is_empty() {
                format!("column_{}", c + 1)
            } else {
                from_header.to_string()
            }
        })
        .collect();
    let body = if has_header { &grid[1..] } else { &grid[..] };

    let mut numeric = Vec::new();
    let mut skipped = Vec::new();
    for c in 0..columns_total {
        let mut values: Vec<Option<f64>> = Vec::with_capacity(body.len());
        let mut count = 0usize;
        let mut missing = 0usize;
        let mut integer_only = true;
        let mut first_bad: Option<String> = None;
        for row in body {
            let raw = row.get(c).map(|s| s.trim()).unwrap_or("");
            if raw.is_empty() || nulls.iter().any(|t| *t == raw) {
                missing += 1;
                values.push(None);
                continue;
            }
            match parse_number(raw, normalize) {
                Some((v, is_int_text)) => {
                    count += 1;
                    integer_only &= is_int_text;
                    values.push(Some(v));
                }
                None => {
                    if first_bad.is_none() {
                        first_bad = Some(raw.to_string());
                    }
                    values.push(None);
                }
            }
        }
        let present = values.len() - missing;
        let ratio = if present == 0 {
            0.0
        } else {
            count as f64 / present as f64
        };
        let reason = if body.is_empty() {
            Some("no data rows".to_string())
        } else if present == 0 {
            Some("every cell is empty or a null token".to_string())
        } else if missing > 0 && !allow_blanks {
            Some(format!(
                "{missing} blank/null cell(s) and allow_blanks is off"
            ))
        } else if ratio + 1e-9 < min_numeric_ratio {
            Some(format!(
                "only {} of {present} value(s) parse as numbers ({}% < the {}% required)",
                count,
                fmt_num((ratio * 1000.0).round() / 10.0),
                fmt_num((min_numeric_ratio * 1000.0).round() / 10.0)
            ))
        } else {
            None
        };
        match reason {
            Some(reason) => skipped.push(SkippedColumn {
                name: names[c].clone(),
                index: c + 1,
                reason,
                example: first_bad,
                numeric_ratio: (ratio * 10000.0).round() / 10000.0,
            }),
            None => numeric.push(NumericColumn {
                name: names[c].clone(),
                index: c + 1,
                is_integer: integer_only,
                values,
                numeric_ratio: (ratio * 10000.0).round() / 10000.0,
                count,
                missing,
            }),
        }
    }

    Ok(Extraction {
        delimiter: delim_name,
        header: has_header,
        rows: body.len(),
        columns_total,
        numeric,
        skipped,
    })
}

// ---------------------------------------------------------------- rendering

fn render_columns(r: &Extraction) -> String {
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"delimiter\": \"{}\",\n", r.delimiter));
    s.push_str(&format!("  \"header\": {},\n", r.header));
    s.push_str(&format!("  \"rows\": {},\n", r.rows));
    s.push_str(&format!("  \"columns_total\": {},\n", r.columns_total));
    s.push_str(&format!("  \"numeric_columns\": {},\n", r.numeric.len()));
    s.push_str("  \"columns\": [");
    for (i, col) in r.numeric.iter().enumerate() {
        s.push_str(if i == 0 { "\n" } else { ",\n" });
        s.push_str("    {\n");
        s.push_str(&format!("      \"name\": {},\n", json_str(&col.name)));
        s.push_str(&format!("      \"index\": {},\n", col.index));
        s.push_str(&format!(
            "      \"type\": \"{}\",\n",
            if col.is_integer { "integer" } else { "float" }
        ));
        s.push_str(&format!("      \"count\": {},\n", col.count));
        s.push_str(&format!("      \"missing\": {},\n", col.missing));
        s.push_str(&format!(
            "      \"numeric_ratio\": {},\n",
            fmt_num(col.numeric_ratio)
        ));
        s.push_str(&format!("      \"values\": {}\n", json_values(&col.values)));
        s.push_str("    }");
    }
    s.push_str(if r.numeric.is_empty() { "],\n" } else { "\n  ],\n" });
    s.push_str("  \"skipped\": [");
    for (i, col) in r.skipped.iter().enumerate() {
        s.push_str(if i == 0 { "\n" } else { ",\n" });
        s.push_str("    {\n");
        s.push_str(&format!("      \"name\": {},\n", json_str(&col.name)));
        s.push_str(&format!("      \"index\": {},\n", col.index));
        s.push_str(&format!("      \"reason\": {},\n", json_str(&col.reason)));
        s.push_str(&format!(
            "      \"example\": {},\n",
            col.example
                .as_deref()
                .map(json_str)
                .unwrap_or_else(|| "null".to_string())
        ));
        s.push_str(&format!(
            "      \"numeric_ratio\": {}\n",
            fmt_num(col.numeric_ratio)
        ));
        s.push_str("    }");
    }
    s.push_str(if r.skipped.is_empty() { "]\n}" } else { "\n  ]\n}" });
    s
}

fn render_records(r: &Extraction) -> String {
    if r.numeric.is_empty() || r.rows == 0 {
        return "[]".to_string();
    }
    let mut s = String::from("[\n");
    for row in 0..r.rows {
        if row > 0 {
            s.push_str(",\n");
        }
        let cells: Vec<String> = r
            .numeric
            .iter()
            .map(|c| {
                format!(
                    "{}: {}",
                    json_str(&c.name),
                    c.values[row].map(fmt_num).unwrap_or_else(|| "null".into())
                )
            })
            .collect();
        s.push_str(&format!("  {{ {} }}", cells.join(", ")));
    }
    s.push_str("\n]");
    s
}

fn render_csv(r: &Extraction) -> String {
    if r.numeric.is_empty() {
        return String::new();
    }
    let mut lines = vec![r
        .numeric
        .iter()
        .map(|c| csv_escape(&c.name))
        .collect::<Vec<_>>()
        .join(",")];
    for row in 0..r.rows {
        lines.push(
            r.numeric
                .iter()
                .map(|c| c.values[row].map(fmt_num).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    lines.join("\n")
}

fn render_names(r: &Extraction) -> String {
    r.numeric
        .iter()
        .map(|c| c.name.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------- parsing

/// Minimal RFC 4180 reader: quoted fields may hold the delimiter, newlines and
/// doubled quotes; CRLF and LF both end a record; wholly blank lines are dropped.
fn parse_csv(data: &str, delim: char) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = data.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(ch);
            }
            continue;
        }
        match ch {
            '"' if field.is_empty() => quoted = true,
            c if c == delim => row.push(std::mem::take(&mut field)),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                push_row(&mut rows, std::mem::take(&mut row));
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                push_row(&mut rows, std::mem::take(&mut row));
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        push_row(&mut rows, row);
    }
    rows
}

fn push_row(rows: &mut Vec<Vec<String>>, row: Vec<String>) {
    if row.iter().all(|f| f.trim().is_empty()) {
        return; // blank line
    }
    rows.push(row);
}

/// Score each candidate delimiter by "how many consistent columns does it yield",
/// preferring the earliest candidate on a tie (comma, tab, semicolon, pipe).
fn sniff_delimiter(data: &str) -> (char, &'static str) {
    let mut best = (DELIMS[0].0, DELIMS[0].1, -1.0f64);
    for (ch, name) in DELIMS {
        let grid = parse_csv(data, ch);
        if grid.is_empty() {
            continue;
        }
        let mut counts: Vec<usize> = grid.iter().map(Vec::len).collect();
        counts.sort_unstable();
        let modal = counts
            .iter()
            .copied()
            .max_by_key(|m| counts.iter().filter(|c| *c == m).count())
            .unwrap_or(1);
        if modal < 2 {
            continue;
        }
        let agree = counts.iter().filter(|c| **c == modal).count() as f64 / counts.len() as f64;
        let score = modal as f64 * agree;
        if score > best.2 {
            best = (ch, name, score);
        }
    }
    if best.2 < 0.0 {
        // Single-column data: every candidate parses it identically, so report
        // the conventional one.
        return (DELIMS[0].0, DELIMS[0].1);
    }
    (best.0, best.1)
}

/// A first row is a header when none of its cells is a number (a numeric first row
/// is data — `10;20;30` has no header).
fn looks_like_header(first: &[String], normalize: bool, nulls: &[&str]) -> bool {
    let mut saw_text = false;
    for cell in first {
        let raw = cell.trim();
        if raw.is_empty() || nulls.iter().any(|t| *t == raw) {
            continue;
        }
        if parse_number(raw, normalize).is_some() {
            return false;
        }
        saw_text = true;
    }
    saw_text
}

/// Parse one cell. Returns the value plus whether the source text was written as a
/// whole number (no decimal point, no exponent).
///
/// With `normalize` on, accounting shapes are accepted: thousands separators
/// (`1,234.5`), currency symbols (`$1200`), trailing percent (`45%`),
/// parentheses negatives (`(500)`) and trailing minus (`500-`).
pub fn parse_number(raw: &str, normalize: bool) -> Option<(f64, bool)> {
    let mut s = raw.trim().to_string();
    if s.is_empty() {
        return None;
    }
    if normalize {
        let mut negative = false;
        if s.starts_with('(') && s.ends_with(')') && s.len() > 2 {
            negative = true;
            s = s[1..s.len() - 1].trim().to_string();
        }
        s = s
            .trim_start_matches(['$', '€', '£', '¥', '₹'])
            .trim()
            .to_string();
        s = s
            .trim_end_matches(['%', '$', '€', '£', '¥', '₹'])
            .trim()
            .to_string();
        if s.ends_with('-') && s.len() > 1 {
            negative = !negative;
            s = s[..s.len() - 1].trim().to_string();
        }
        if is_grouped(&s) {
            s.retain(|c| c != ',' && c != ' ' && c != '\u{a0}' && c != '_');
        }
        if negative {
            s = if let Some(rest) = s.strip_prefix('-') {
                rest.to_string()
            } else {
                format!("-{s}")
            };
        }
    }
    // Only plain numeric syntax — this rejects "inf", "NaN", "1d0" and friends,
    // which Rust's f64 parser would otherwise accept.
    if !s.chars().all(|c| c.is_ascii_digit() || "+-.eE".contains(c)) {
        return None;
    }
    if !s.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let digits = s.trim_start_matches(['+', '-']);
    // Zero-padded codes (007, 0123) are identifiers, not measurements.
    if digits.len() > 1 && digits.starts_with('0') && digits.as_bytes()[1].is_ascii_digit() {
        return None;
    }
    let value: f64 = s.parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    let integer_text = !s.contains('.') && !s.contains('e') && !s.contains('E');
    Some((value, integer_text))
}

/// `1,234`, `1,234,567.89`, `1 234 567` — grouped thousands, not a stray separator.
fn is_grouped(s: &str) -> bool {
    let body = s.trim_start_matches(['+', '-']);
    let (int_part, rest_ok) = match body.split_once('.') {
        Some((i, frac)) => (i, frac.chars().all(|c| c.is_ascii_digit()) && !frac.is_empty()),
        None => (body, true),
    };
    if !rest_ok {
        return false;
    }
    let groups: Vec<&str> = int_part
        .split(|c| c == ',' || c == ' ' || c == '\u{a0}' || c == '_')
        .collect();
    if groups.len() < 2 {
        return false;
    }
    if groups[0].is_empty() || groups[0].len() > 3 || !groups[0].chars().all(|c| c.is_ascii_digit())
    {
        return false;
    }
    groups[1..]
        .iter()
        .all(|g| g.len() == 3 && g.chars().all(|c| c.is_ascii_digit()))
}

// ---------------------------------------------------------------- helpers

fn pick<'a>(
    value: &'a str,
    allowed: &[&'a str],
    fallback: &'a str,
    field: &str,
) -> Result<&'a str, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(fallback);
    }
    allowed
        .iter()
        .copied()
        .find(|a| a.eq_ignore_ascii_case(v))
        .ok_or_else(|| format!("unknown {field} '{v}': expected one of {}", allowed.join(", ")))
}

/// Shortest faithful rendering: whole values print without a trailing `.0`.
pub fn fmt_num(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn json_values(values: &[Option<f64>]) -> String {
    let inner: Vec<String> = values
        .iter()
        .map(|v| v.map(fmt_num).unwrap_or_else(|| "null".into()))
        .collect();
    format!("[{}]", inner.join(", "))
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
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
    out.push('"');
    out
}

fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(data: &str, output: &str) -> String {
        extract(
            data,
            "auto",
            "auto",
            output,
            DEFAULT_NULL_TOKENS,
            true,
            1.0,
            true,
        )
        .unwrap()
    }

    #[test]
    fn extracts_numeric_columns_with_headers() {
        let out = run("id,name,score\n1,Alice,9.5\n2,Bob,7", "columns");
        assert!(out.contains("\"numeric_columns\": 2"), "{out}");
        assert!(out.contains("\"name\": \"id\""), "{out}");
        assert!(out.contains("\"values\": [9.5, 7]"), "{out}");
        assert!(out.contains("\"type\": \"integer\""), "{out}");
        assert!(out.contains("\"type\": \"float\""), "{out}");
        assert!(out.contains("\"reason\": \"only 0 of 2"), "{out}");
    }

    #[test]
    fn names_output_lists_only_numeric_headers() {
        assert_eq!(run("id,name,score\n1,Alice,9.5\n2,Bob,7", "names"), "id\nscore");
    }

    #[test]
    fn records_output_keeps_numeric_fields_only() {
        assert_eq!(
            run("id,name,score\n1,Alice,9.5\n2,Bob,7", "records"),
            "[\n  { \"id\": 1, \"score\": 9.5 },\n  { \"id\": 2, \"score\": 7 }\n]"
        );
    }

    #[test]
    fn csv_output_round_trips_numeric_columns() {
        assert_eq!(
            run("id,name,score\n1,Alice,9.5\n2,Bob,7", "csv"),
            "id,score\n1,9.5\n2,7"
        );
    }

    #[test]
    fn detects_tab_and_semicolon_delimiters() {
        assert_eq!(run("a\tb\n1\tx\n2\ty", "names"), "a");
        assert_eq!(run("a;b\n1;x\n2;y", "names"), "a");
        assert_eq!(run("a|b\n1|x\n2|y", "names"), "a");
    }

    #[test]
    fn headerless_numeric_grid_generates_column_names() {
        assert_eq!(run("10;20;30\n40;50;60", "names"), "column_1\ncolumn_2\ncolumn_3");
    }

    #[test]
    fn blanks_and_null_tokens_become_null() {
        let out = run("score\n1\n\"\"\nNA\n4", "columns");
        assert!(out.contains("\"values\": [1, null, 4]"), "{out}");
        assert!(out.contains("\"missing\": 1"), "{out}");
    }

    #[test]
    fn allow_blanks_off_rejects_gappy_columns() {
        let out = extract(
            "a,b\n1,2\n,3",
            "auto",
            "auto",
            "names",
            DEFAULT_NULL_TOKENS,
            false,
            1.0,
            true,
        )
        .unwrap();
        assert_eq!(out, "b");
    }

    #[test]
    fn ratio_threshold_admits_mostly_numeric_columns() {
        let out = extract(
            "a\n1\n2\n3\nn/a-ish",
            "auto",
            "present",
            "columns",
            DEFAULT_NULL_TOKENS,
            true,
            0.75,
            true,
        )
        .unwrap();
        assert!(out.contains("\"values\": [1, 2, 3, null]"), "{out}");
        assert!(out.contains("\"numeric_ratio\": 0.75"), "{out}");
    }

    #[test]
    fn normalizes_currency_percent_grouping_and_parens() {
        let out = run(
            "amount,pct,owed\n\"$1,234.50\",45%,(500)\n\"$2,000\",7%,250-",
            "csv",
        );
        assert_eq!(out, "amount,pct,owed\n1234.5,45,-500\n2000,7,-250");
    }

    #[test]
    fn normalize_off_rejects_formatted_numbers() {
        let out = extract(
            "amount\n\"$1,234.50\"\n\"$2,000\"",
            "auto",
            "present",
            "names",
            DEFAULT_NULL_TOKENS,
            true,
            1.0,
            false,
        )
        .unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn zero_padded_codes_stay_non_numeric() {
        let out = run("code,qty\n007,3\n012,4", "names");
        assert_eq!(out, "qty");
    }

    #[test]
    fn quoted_fields_with_delimiters_and_newlines_parse() {
        let out = run("note,qty\n\"a,b\nc\",5\n\"d\"\"e\",6", "csv");
        assert_eq!(out, "qty\n5\n6");
    }

    #[test]
    fn ragged_rows_are_padded() {
        let out = run("a,b,c\n1,2\n3,4,5", "columns");
        assert!(out.contains("\"columns_total\": 3"), "{out}");
        assert!(out.contains("\"values\": [null, 5]"), "{out}");
    }

    #[test]
    fn scientific_notation_is_float() {
        let out = run("v\n1.2e3\n5e-1", "columns");
        assert!(out.contains("\"type\": \"float\""), "{out}");
        assert!(out.contains("\"values\": [1200, 0.5]"), "{out}");
    }

    #[test]
    fn infinity_and_nan_text_are_not_numeric() {
        assert_eq!(run("v\ninf\n1", "names"), "");
        assert_eq!(run("v\nNaN\n1", "names"), "");
    }

    #[test]
    fn forced_header_absent_treats_first_row_as_data() {
        let out = extract(
            "a,1\nb,2",
            "comma",
            "absent",
            "columns",
            DEFAULT_NULL_TOKENS,
            true,
            1.0,
            true,
        )
        .unwrap();
        assert!(out.contains("\"values\": [1, 2]"), "{out}");
        assert!(out.contains("\"name\": \"column_2\""), "{out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = extract("", "auto", "auto", "columns", DEFAULT_NULL_TOKENS, true, 1.0, true)
            .unwrap_err();
        assert!(err.contains("no CSV data"), "{err}");
    }

    #[test]
    fn unknown_output_is_an_error() {
        let err = extract("a\n1", "auto", "auto", "table", DEFAULT_NULL_TOKENS, true, 1.0, true)
            .unwrap_err();
        assert_eq!(
            err,
            "unknown output 'table': expected one of columns, records, csv, names"
        );
    }

    #[test]
    fn ratio_out_of_range_is_an_error() {
        let err = extract("a\n1", "auto", "auto", "columns", DEFAULT_NULL_TOKENS, true, 0.0, true)
            .unwrap_err();
        assert!(err.contains("between 0.1 and 1.0"), "{err}");
    }

    #[test]
    fn oversized_input_is_an_error() {
        let big = "a\n".to_string() + &"1\n".repeat(MAX_INPUT_BYTES / 2);
        let err = extract(&big, "auto", "auto", "names", DEFAULT_NULL_TOKENS, true, 1.0, true)
            .unwrap_err();
        assert!(err.contains("about 1 MB"), "{err}");
    }

    #[test]
    fn input_at_the_cap_is_accepted() {
        let mut data = String::from("v\n");
        while data.len() < MAX_INPUT_BYTES - 2 {
            data.push_str("1\n");
        }
        while data.len() < MAX_INPUT_BYTES {
            data.push('1');
        }
        assert_eq!(data.len(), MAX_INPUT_BYTES);
        assert_eq!(
            extract(&data, "comma", "present", "names", DEFAULT_NULL_TOKENS, true, 1.0, true)
                .unwrap(),
            "v"
        );
    }

    #[test]
    fn custom_null_tokens_are_honoured() {
        let out = extract(
            "v\n1\n-\n3",
            "auto",
            "present",
            "columns",
            "-",
            true,
            1.0,
            true,
        )
        .unwrap();
        assert!(out.contains("\"values\": [1, null, 3]"), "{out}");
    }

    #[test]
    fn no_numeric_columns_reports_every_skip() {
        let out = run("name,city\nAlice,Berlin\nBob,Oslo", "columns");
        assert!(out.contains("\"numeric_columns\": 0"), "{out}");
        assert!(out.contains("\"columns\": [],"), "{out}");
        assert!(out.contains("\"example\": \"Alice\""), "{out}");
    }
}
