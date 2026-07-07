//! csv-to-xlsx core — parse CSV / TSV / semicolon- / pipe-delimited / JSON /
//! NDJSON table data and write a real binary `.xlsx` workbook (typed cells, an
//! optional bold + frozen header row, autofit column widths). Pure logic shared
//! by the chat skill block, the CLI and the web page — no wafer / wasm-bindgen
//! deps.
//!
//! The workbook is produced by `rust_xlsxwriter` (pure Rust). Numbers and
//! booleans are written as native Excel cell types so spreadsheets sort and
//! total them correctly; leading-zero values like `007` stay text.

use rust_xlsxwriter::{Format, Workbook, Worksheet, XlsxError};
use serde_json::Value;

/// Source table format. `Auto` sniffs the concrete format from the text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputFormat {
    /// Detect the source format from the input text.
    Auto,
    /// Comma-separated values.
    Csv,
    /// Tab-separated values.
    Tsv,
    /// Semicolon-separated values (common in locales where `,` is the decimal mark).
    Semicolon,
    /// Pipe (`|`)-separated values.
    Pipe,
    /// A single JSON array of records (a top-level object is accepted as one row).
    Json,
    /// Newline-delimited JSON — one JSON value per line (a.k.a. JSONL).
    Ndjson,
}

impl InputFormat {
    /// Parse a format name (canonical + common aliases).
    pub fn parse(s: &str) -> Result<InputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(InputFormat::Auto),
            "csv" | "comma" => Ok(InputFormat::Csv),
            "tsv" | "tab" => Ok(InputFormat::Tsv),
            "semicolon" | "semi" => Ok(InputFormat::Semicolon),
            "pipe" | "bar" => Ok(InputFormat::Pipe),
            "json" => Ok(InputFormat::Json),
            "ndjson" | "jsonl" | "json-lines" | "jsonlines" => Ok(InputFormat::Ndjson),
            other => Err(format!(
                "unknown input_format '{other}' (use auto, csv, tsv, semicolon, pipe, json, or ndjson)"
            )),
        }
    }

    /// The single-byte field separator for the delimited formats.
    fn delimiter(self) -> Option<u8> {
        match self {
            InputFormat::Csv => Some(b','),
            InputFormat::Tsv => Some(b'\t'),
            InputFormat::Semicolon => Some(b';'),
            InputFormat::Pipe => Some(b'|'),
            _ => None,
        }
    }
}

const MAX_SHEET_NAME: usize = 31;
/// Characters Excel forbids in a worksheet name.
const FORBIDDEN_SHEET_CHARS: [char; 7] = ['[', ']', ':', '*', '?', '/', '\\'];

/// Clamp a worksheet name to Excel's rules: non-empty, at most 31 characters,
/// and none of `[ ] : * ? / \` (each replaced with `_`). An empty / blank name
/// falls back to `Sheet1`.
pub fn sanitize_sheet_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if FORBIDDEN_SHEET_CHARS.contains(&c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return "Sheet1".to_string();
    }
    trimmed.chars().take(MAX_SHEET_NAME).collect()
}

/// A normalized rectangular table ready to write.
struct Table {
    /// Column names for a bold + frozen header row, when one should be written.
    header: Option<Vec<String>>,
    /// Data rows (already typed). Ragged rows are fine — missing trailing cells
    /// are simply left blank in the sheet.
    rows: Vec<Vec<Value>>,
}

/// Convert `data` in the given `input_format` into a binary `.xlsx` workbook.
///
/// - `sheet_name` — the worksheet tab name (sanitized to Excel's rules).
/// - `header` — treat the first row as a header (bold + frozen); for JSON
///   objects the keys become the header. Off writes data with no header row.
/// - `detect_types` — coerce delimited cells to numbers/booleans (native Excel
///   types); off writes every delimited cell as text. Leading-zero values stay
///   text. JSON / NDJSON keep their own types regardless.
/// - `autofit` — auto-size column widths to fit their content.
pub fn to_xlsx(
    data: &str,
    input_format: InputFormat,
    sheet_name: &str,
    header: bool,
    detect_types: bool,
    autofit: bool,
) -> Result<Vec<u8>, String> {
    to_xlsx_with_stats(data, input_format, sheet_name, header, detect_types, autofit).map(|(b, _, _)| b)
}

/// Like [`to_xlsx`] but also returns `(data_rows, columns)` so callers can
/// summarize the workbook without re-parsing it.
pub fn to_xlsx_with_stats(
    data: &str,
    input_format: InputFormat,
    sheet_name: &str,
    header: bool,
    detect_types: bool,
    autofit: bool,
) -> Result<(Vec<u8>, usize, usize), String> {
    if data.trim().is_empty() {
        return Err("input is empty".to_string());
    }
    let fmt = match input_format {
        InputFormat::Auto => detect(data),
        f => f,
    };
    let table = build_table(data, fmt, header, detect_types)?;
    let cols = table
        .header
        .as_ref()
        .map(|h| h.len())
        .unwrap_or(0)
        .max(table.rows.iter().map(Vec::len).max().unwrap_or(0));
    let data_rows = table.rows.len();
    let bytes = write_workbook(&table, sheet_name, autofit)?;
    Ok((bytes, data_rows, cols))
}

/// Sniff the concrete source format of `data` for [`InputFormat::Auto`].
fn detect(data: &str) -> InputFormat {
    let first = data.trim_start().chars().next().unwrap_or(' ');
    if first == '[' {
        return InputFormat::Json;
    }
    if first == '{' {
        let lines: Vec<&str> = data.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        if lines.len() >= 2 && lines.iter().all(|l| serde_json::from_str::<Value>(l).is_ok()) {
            return InputFormat::Ndjson;
        }
        return InputFormat::Json;
    }
    // Delimited: pick whichever delimiter is most frequent on the first non-empty
    // line. `max_by_key` returns the LAST maximum, so ordering comma last makes
    // it win ties — the most common CSV case.
    let first_line = data.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let counts = [
        (InputFormat::Tsv, first_line.matches('\t').count()),
        (InputFormat::Semicolon, first_line.matches(';').count()),
        (InputFormat::Pipe, first_line.matches('|').count()),
        (InputFormat::Csv, first_line.matches(',').count()),
    ];
    counts
        .iter()
        .filter(|(_, n)| *n > 0)
        .max_by_key(|(_, n)| *n)
        .map(|(fmt, _)| *fmt)
        .unwrap_or(InputFormat::Csv)
}

/// Parse `data` (concrete `fmt`, never [`InputFormat::Auto`]) into a [`Table`].
fn build_table(data: &str, fmt: InputFormat, header: bool, detect_types: bool) -> Result<Table, String> {
    match fmt {
        InputFormat::Json | InputFormat::Ndjson => {
            let records = parse_json_records(data, fmt)?;
            table_from_records(records, header)
        }
        InputFormat::Auto => unreachable!("source format is resolved before build_table"),
        delimited => build_delimited_table(data, delimited.delimiter().unwrap(), header, detect_types),
    }
}

/// Parse a JSON array / single object, or NDJSON, into a list of record values.
fn parse_json_records(data: &str, fmt: InputFormat) -> Result<Vec<Value>, String> {
    match fmt {
        InputFormat::Json => {
            let parsed: Value =
                serde_json::from_str(data).map_err(|e| format!("JSON parse error: {e}"))?;
            match parsed {
                Value::Array(a) => Ok(a),
                obj @ Value::Object(_) => Ok(vec![obj]),
                _ => Err("JSON input must be an array of rows (or a single object)".to_string()),
            }
        }
        InputFormat::Ndjson => {
            let mut out = Vec::new();
            for (i, line) in data.lines().enumerate() {
                let t = line.trim();
                if t.is_empty() {
                    continue;
                }
                let v: Value = serde_json::from_str(t)
                    .map_err(|e| format!("NDJSON parse error on line {}: {e}", i + 1))?;
                out.push(v);
            }
            Ok(out)
        }
        _ => unreachable!("parse_json_records only handles Json/Ndjson"),
    }
}

/// Turn parsed JSON records into a [`Table`]. All-object records become named
/// columns (union of keys, first-seen order); anything else is written
/// positionally, with the first row used as the header when `header` is set.
fn table_from_records(records: Vec<Value>, header: bool) -> Result<Table, String> {
    if records.is_empty() {
        return Err("no records found in the input".to_string());
    }
    if records.iter().all(Value::is_object) {
        let mut keys: Vec<String> = Vec::new();
        for r in &records {
            if let Value::Object(m) = r {
                for k in m.keys() {
                    if !keys.iter().any(|e| e == k) {
                        keys.push(k.clone());
                    }
                }
            }
        }
        let rows: Vec<Vec<Value>> = records
            .iter()
            .map(|r| {
                let m = r.as_object().unwrap();
                keys.iter().map(|k| m.get(k).cloned().unwrap_or(Value::Null)).collect()
            })
            .collect();
        Ok(Table {
            header: header.then_some(keys),
            rows,
        })
    } else {
        let mut rows: Vec<Vec<Value>> = records
            .into_iter()
            .map(|r| match r {
                Value::Array(a) => a,
                other => vec![other],
            })
            .collect();
        let header_row = if header && !rows.is_empty() {
            Some(rows.remove(0).iter().map(value_to_header_string).collect())
        } else {
            None
        };
        Ok(Table { header: header_row, rows })
    }
}

/// Parse delimited text into a [`Table`] via the `csv` crate.
fn build_delimited_table(
    data: &str,
    delim: u8,
    header: bool,
    detect_types: bool,
) -> Result<Table, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(header)
        .flexible(true)
        .from_reader(data.as_bytes());

    if header {
        let hdr: Vec<String> = rdr
            .headers()
            .map_err(|e| format!("CSV/TSV parse error: {e}"))?
            .iter()
            .map(str::to_string)
            .collect();
        let mut rows = Vec::new();
        for rec in rdr.records() {
            let rec = rec.map_err(|e| format!("CSV/TSV parse error: {e}"))?;
            rows.push(rec.iter().map(|f| cell_value(f, detect_types)).collect());
        }
        Ok(Table { header: Some(hdr), rows })
    } else {
        let mut rows = Vec::new();
        for rec in rdr.records() {
            let rec = rec.map_err(|e| format!("CSV/TSV parse error: {e}"))?;
            rows.push(rec.iter().map(|f| cell_value(f, detect_types)).collect());
        }
        if rows.is_empty() {
            return Err("no rows found in the input".to_string());
        }
        Ok(Table { header: None, rows })
    }
}

/// Turn one delimited cell into a JSON value. With `detect = false` every cell
/// stays a string; with `detect = true` see [`infer_scalar`].
fn cell_value(s: &str, detect: bool) -> Value {
    if detect {
        infer_scalar(s)
    } else {
        Value::String(s.to_string())
    }
}

/// Infer a JSON scalar from a delimited cell: integers, finite floats, booleans,
/// and empty → null; everything else stays a string. Leading-zero / `+`-prefixed
/// strings (zip codes, phone numbers) are kept as strings, not coerced.
fn infer_scalar(s: &str) -> Value {
    if s.is_empty() {
        return Value::Null;
    }
    match s {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(i) = s.parse::<i64>() {
        // Only accept it as an integer when it re-serializes identically, so
        // "007", "+1" and "1_000" stay text.
        let canonical = i.to_string();
        if canonical == s {
            return Value::from(i);
        }
    }
    if let Ok(f) = s.parse::<f64>() {
        if f.is_finite() {
            let canonical = Value::from(f).to_string();
            if canonical == s {
                return Value::from(f);
            }
        }
    }
    Value::String(s.to_string())
}

/// Stringify a JSON value for use as a header cell.
fn value_to_header_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// Write a [`Table`] into a single-sheet workbook and return its bytes.
fn write_workbook(table: &Table, sheet_name: &str, autofit: bool) -> Result<Vec<u8>, String> {
    let mut workbook = Workbook::new();
    let bold = Format::new().set_bold();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name(sanitize_sheet_name(sheet_name))
        .map_err(xlsx_err)?;

    let mut row: u32 = 0;
    if let Some(hdr) = &table.header {
        for (c, name) in hdr.iter().enumerate() {
            let col = col_index(c)?;
            worksheet
                .write_string_with_format(row, col, name.as_str(), &bold)
                .map_err(xlsx_err)?;
        }
        worksheet.set_freeze_panes(1, 0).map_err(xlsx_err)?;
        row = 1;
    }

    for data_row in &table.rows {
        for (c, value) in data_row.iter().enumerate() {
            let col = col_index(c)?;
            write_cell(worksheet, row, col, value).map_err(xlsx_err)?;
        }
        row = row
            .checked_add(1)
            .ok_or_else(|| "too many rows for a worksheet".to_string())?;
    }

    if autofit {
        worksheet.autofit();
    }

    workbook.save_to_buffer().map_err(xlsx_err)
}

/// Convert a 0-based column index to Excel's `u16` column, rejecting anything
/// past the 16,384-column sheet limit with a clear message.
fn col_index(c: usize) -> Result<u16, String> {
    u16::try_from(c).map_err(|_| "too many columns (Excel supports at most 16,384)".to_string())
}

/// Write one JSON value into a cell with its native Excel type. `Null` leaves
/// the cell blank; nested arrays/objects are written as their compact JSON text
/// so no data is silently dropped.
fn write_cell(ws: &mut Worksheet, row: u32, col: u16, value: &Value) -> Result<(), XlsxError> {
    match value {
        Value::Null => {}
        Value::Bool(b) => {
            ws.write_boolean(row, col, *b)?;
        }
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() => {
                ws.write_number(row, col, f)?;
            }
            // Integers too large for f64, or non-finite — keep exact text.
            _ => {
                ws.write_string(row, col, n.to_string())?;
            }
        },
        Value::String(s) => {
            ws.write_string(row, col, s.as_str())?;
        }
        other => {
            ws.write_string(row, col, other.to_string())?;
        }
    }
    Ok(())
}

fn xlsx_err(e: XlsxError) -> String {
    format!("xlsx write error: {e}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};
    use std::io::Cursor;

    /// Read `bytes` back and return the named worksheet's cells as a grid.
    fn read_sheet(bytes: &[u8], name: &str) -> Vec<Vec<Data>> {
        let mut wb: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes.to_vec())).unwrap();
        let range = wb.worksheet_range(name).unwrap();
        range.rows().map(<[Data]>::to_vec).collect()
    }

    fn to_bytes(data: &str, fmt: &str, sheet: &str, header: bool, detect: bool) -> Vec<u8> {
        to_xlsx(data, InputFormat::parse(fmt).unwrap(), sheet, header, detect, true).unwrap()
    }

    #[test]
    fn csv_with_header_writes_typed_cells() {
        let bytes = to_bytes("name,age,active\nAlice,30,true\nBob,25,false", "csv", "People", true, true);
        let grid = read_sheet(&bytes, "People");
        // Header row.
        assert_eq!(grid[0][0], Data::String("name".into()));
        assert_eq!(grid[0][1], Data::String("age".into()));
        assert_eq!(grid[0][2], Data::String("active".into()));
        // Data row 1: string, native number, native bool.
        assert_eq!(grid[1][0], Data::String("Alice".into()));
        assert_eq!(grid[1][1], Data::Float(30.0));
        assert_eq!(grid[1][2], Data::Bool(true));
        assert_eq!(grid[2][1], Data::Float(25.0));
        assert_eq!(grid[2][2], Data::Bool(false));
    }

    #[test]
    fn leading_zero_stays_text_not_number() {
        let bytes = to_bytes("zip\n007", "csv", "Sheet1", true, true);
        let grid = read_sheet(&bytes, "Sheet1");
        assert_eq!(grid[1][0], Data::String("007".into()));
    }

    #[test]
    fn detect_types_off_keeps_everything_text() {
        let bytes = to_bytes("age\n30", "csv", "Sheet1", true, false);
        let grid = read_sheet(&bytes, "Sheet1");
        assert_eq!(grid[1][0], Data::String("30".into()));
    }

    #[test]
    fn json_array_of_objects_unions_keys() {
        let bytes = to_bytes(r#"[{"a":1,"b":"x"},{"a":2,"c":true}]"#, "json", "Sheet1", true, true);
        let grid = read_sheet(&bytes, "Sheet1");
        // Header = union of keys in first-seen order.
        assert_eq!(grid[0], vec![Data::String("a".into()), Data::String("b".into()), Data::String("c".into())]);
        assert_eq!(grid[1][0], Data::Float(1.0));
        assert_eq!(grid[1][1], Data::String("x".into()));
        assert_eq!(grid[1][2], Data::Empty); // missing "c" in row 1
        assert_eq!(grid[2][0], Data::Float(2.0));
        assert_eq!(grid[2][2], Data::Bool(true));
    }

    #[test]
    fn header_off_writes_no_header_row() {
        let bytes = to_bytes("name,age\nAlice,30", "csv", "Sheet1", false, true);
        let grid = read_sheet(&bytes, "Sheet1");
        // First row is data ("name","age" as literal strings), not a bold header.
        assert_eq!(grid[0][0], Data::String("name".into()));
        assert_eq!(grid[1][0], Data::String("Alice".into()));
        assert_eq!(grid[1][1], Data::Float(30.0));
    }

    #[test]
    fn semicolon_is_auto_detected() {
        let bytes = to_bytes("a;b\n1;2", "auto", "Sheet1", true, true);
        let grid = read_sheet(&bytes, "Sheet1");
        assert_eq!(grid[0], vec![Data::String("a".into()), Data::String("b".into())]);
        assert_eq!(grid[1][0], Data::Float(1.0));
        assert_eq!(grid[1][1], Data::Float(2.0));
    }

    #[test]
    fn sheet_name_is_sanitized() {
        assert_eq!(sanitize_sheet_name("Q1/Q2:Sales"), "Q1_Q2_Sales");
        assert_eq!(sanitize_sheet_name("   "), "Sheet1");
        assert_eq!(sanitize_sheet_name(""), "Sheet1");
        let long = "x".repeat(40);
        assert_eq!(sanitize_sheet_name(&long).chars().count(), 31);
        // A forbidden sheet name still produces a readable workbook.
        let bytes = to_bytes("a\n1", "csv", "Bad:Name*Here", true, true);
        assert!(read_sheet(&bytes, "Bad_Name_Here").len() >= 2);
    }

    #[test]
    fn nested_json_value_becomes_json_text() {
        let bytes = to_bytes(r#"[{"id":1,"tags":["a","b"]}]"#, "json", "Sheet1", true, true);
        let grid = read_sheet(&bytes, "Sheet1");
        assert_eq!(grid[1][1], Data::String(r#"["a","b"]"#.into()));
    }

    #[test]
    fn empty_input_errors() {
        let err = to_xlsx("   ", InputFormat::Auto, "Sheet1", true, true, true).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn bad_json_errors() {
        let err = to_xlsx("{not json", InputFormat::Json, "Sheet1", true, true, true).unwrap_err();
        assert!(err.contains("JSON parse error"), "got: {err}");
    }

    #[test]
    fn unknown_format_errors() {
        assert!(InputFormat::parse("xml").is_err());
    }
}
