//! xlsx-to-csv core — converts a spreadsheet (`.xlsx`/`.xlsm`/`.xls`/`.ods`)
//! into RFC-4180 CSV text. No wafer/wasm-bindgen deps; pure logic shared by the
//! chat skill block (and host-testable). Reads via `calamine` (pure Rust, no C
//! deps → compiles to wasm32-unknown-unknown).

use std::io::Cursor;

use calamine::{open_workbook_auto_from_rs, Data, Reader};

/// Convert the in-memory spreadsheet `bytes` to a CSV string.
///
/// `sheet` selects the worksheet:
/// - `None` → the first sheet.
/// - `Some("Sales")` → the sheet named `Sales`.
/// - `Some("0")` / `Some("2")` → the 0-based sheet **index** (a string that
///   parses as a non-negative integer is treated as an index; a sheet whose
///   *name* is literally a number still wins via the name match below).
///
/// The output is RFC-4180 CSV: `\r\n` line terminators, and any cell containing
/// a comma, double-quote, CR, or LF is wrapped in double-quotes with inner
/// quotes doubled. Empty cells become empty fields. A trailing CRLF is emitted
/// after the final row.
///
/// Returns `Err` on unreadable/empty/corrupt bytes or an unknown sheet.
pub fn to_csv(bytes: &[u8], sheet: Option<&str>) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("empty spreadsheet bytes".to_string());
    }

    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("not a readable spreadsheet: {e}"))?;

    let names = workbook.sheet_names();
    if names.is_empty() {
        return Err("spreadsheet has no worksheets".to_string());
    }

    let target = resolve_sheet_name(&names, sheet)?;

    let range = workbook
        .worksheet_range(&target)
        .map_err(|e| format!("failed to read sheet {target:?}: {e}"))?;

    let mut out = String::new();
    for row in range.rows() {
        let mut first = true;
        for cell in row {
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str(&escape_field(&cell_to_string(cell)));
        }
        out.push_str("\r\n");
    }
    Ok(out)
}

/// Pick the worksheet name to read from the workbook's `names`, honoring the
/// `sheet` selector (name first, then 0-based index for numeric selectors).
fn resolve_sheet_name(names: &[String], sheet: Option<&str>) -> Result<String, String> {
    let Some(sel) = sheet else {
        return Ok(names[0].clone());
    };
    let sel = sel.trim();
    if sel.is_empty() {
        return Ok(names[0].clone());
    }
    // Exact name match wins (so a sheet literally named "0" is reachable).
    if let Some(found) = names.iter().find(|n| n.as_str() == sel) {
        return Ok(found.clone());
    }
    // Otherwise a non-negative integer selects by 0-based index.
    if let Ok(idx) = sel.parse::<usize>() {
        return names.get(idx).cloned().ok_or_else(|| {
            format!(
                "sheet index {idx} out of range (workbook has {} sheet(s): {names:?})",
                names.len()
            )
        });
    }
    Err(format!("no sheet named {sel:?} (available: {names:?})"))
}

/// Render one calamine cell as a plain string. Floats that are whole numbers
/// print without a trailing `.0` (so `42.0` → `"42"`); other floats use the
/// default `{}` formatting. Errors render as their Excel error text.
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => format_float(*f),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => {
            if *b {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        Data::DateTime(dt) => format_float(dt.as_f64()),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("{e:?}"),
    }
}

/// Format an `f64` so integral values drop the `.0` (Excel-style display).
fn format_float(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

/// RFC-4180-quote a field: wrap in `"…"` (doubling inner `"`) iff it contains a
/// comma, double-quote, CR, or LF. Otherwise return it unchanged.
fn escape_field(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        let escaped = field.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_xlsxwriter::Workbook;

    /// Build a tiny in-memory .xlsx with two sheets and return its bytes.
    /// Sheet "First": a header row + a row exercising the quoting rules.
    /// Sheet "Second": a single distinguishing cell.
    fn sample_xlsx() -> Vec<u8> {
        let mut wb = Workbook::new();

        let s1 = wb.add_worksheet().set_name("First").unwrap();
        s1.write_string(0, 0, "name").unwrap();
        s1.write_string(0, 1, "qty").unwrap();
        s1.write_string(0, 2, "note").unwrap();
        s1.write_string(1, 0, "apple, red").unwrap(); // comma → must be quoted
        s1.write_number(1, 1, 3.0).unwrap(); // whole float → "3"
        s1.write_string(1, 2, "say \"hi\"").unwrap(); // quote → doubled + wrapped

        let s2 = wb.add_worksheet().set_name("Second").unwrap();
        s2.write_string(0, 0, "sheet2-marker").unwrap();

        wb.save_to_buffer().unwrap()
    }

    #[test]
    fn converts_first_sheet_with_rfc4180_quoting() {
        let bytes = sample_xlsx();
        let csv = to_csv(&bytes, None).unwrap();
        assert_eq!(
            csv,
            "name,qty,note\r\n\"apple, red\",3,\"say \"\"hi\"\"\"\r\n",
            "got: {csv:?}"
        );
    }

    #[test]
    fn selects_sheet_by_name_and_by_index() {
        let bytes = sample_xlsx();
        let by_name = to_csv(&bytes, Some("Second")).unwrap();
        assert_eq!(by_name, "sheet2-marker\r\n");
        // 0-based index: "1" → the second sheet.
        let by_index = to_csv(&bytes, Some("1")).unwrap();
        assert_eq!(by_index, "sheet2-marker\r\n");
        // "0" → the first sheet.
        let first = to_csv(&bytes, Some("0")).unwrap();
        assert!(first.starts_with("name,qty,note\r\n"), "got: {first:?}");
    }

    #[test]
    fn empty_bytes_error() {
        let err = to_csv(&[], None).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn garbage_bytes_error() {
        let err = to_csv(b"this is not a spreadsheet", None).unwrap_err();
        assert!(err.contains("not a readable spreadsheet"), "got: {err}");
    }

    #[test]
    fn unknown_sheet_name_error() {
        let bytes = sample_xlsx();
        let err = to_csv(&bytes, Some("Nope")).unwrap_err();
        assert!(err.contains("no sheet named"), "got: {err}");
    }

    #[test]
    fn sheet_index_out_of_range_error() {
        let bytes = sample_xlsx();
        let err = to_csv(&bytes, Some("9")).unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn escape_field_quotes_only_when_needed() {
        assert_eq!(escape_field("plain"), "plain");
        assert_eq!(escape_field("a,b"), "\"a,b\"");
        assert_eq!(escape_field("a\"b"), "\"a\"\"b\"");
        assert_eq!(escape_field("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn format_float_drops_trailing_zero() {
        assert_eq!(format_float(42.0), "42");
        assert_eq!(format_float(3.5), "3.5");
        assert_eq!(format_float(-7.0), "-7");
    }
}
