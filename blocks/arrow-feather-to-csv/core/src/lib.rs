//! arrow-feather-to-csv core — read an Apache Arrow IPC / Feather (V2) file and
//! write its table out as CSV text. No wafer/wasm-bindgen deps; pure logic
//! shared by the chat skill block (and host-testable).
//!
//! Reading uses the modular `arrow-ipc` reader (both the "file" format that
//! Feather V2 uses — `ARROW1` magic + footer — and the footer-less "stream"
//! format) and writes with `arrow-csv`. All values are rendered to text (arrow's
//! standard display: RFC-3339 for timestamps/dates, plain decimals for numbers).
//! LZ4-compressed buffers are decoded (pure-Rust `lz4_flex`); ZSTD is not.

use std::io::Cursor;

use arrow_array::RecordBatch;
use arrow_schema::ArrowError;

/// Convert the in-memory Arrow IPC / Feather bytes to a CSV string.
///
/// - `delimiter` — the field separator. A single character, or the word `"tab"`
///   (or a literal tab) for a tab. Empty falls back to `","`.
/// - `header` — write a leading row of column names when `true`.
/// - `null` — the text written for null / missing cells (empty = a blank field).
/// - `columns` — a comma-separated list of columns to keep, by name or 0-based
///   index, in the given order (e.g. `"name,0,price"`). Empty keeps every column
///   in the original order.
/// - `limit` — maximum number of data rows to emit (`0` = all rows), for
///   previewing a large table.
///
/// The output uses `\n` line terminators and RFC-4180 quoting (a field holding
/// the delimiter, a quote, CR, or LF is wrapped in double quotes with inner
/// quotes doubled). Returns `Err` on empty / unreadable / unsupported input.
pub fn to_csv(
    bytes: &[u8],
    delimiter: &str,
    header: bool,
    null: &str,
    columns: &str,
    limit: usize,
) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("empty input: there are no Arrow / Feather bytes to convert".to_string());
    }
    // Legacy Feather V1 is a different, pre-IPC container ("FEA1" magic) that the
    // Arrow IPC reader cannot parse — flag it explicitly instead of a cryptic
    // decode error.
    if bytes.starts_with(b"FEA1") {
        return Err(
            "this is a legacy Feather V1 file, which is not supported; re-save it as Feather V2 / \
             Arrow IPC (e.g. pyarrow `feather.write_feather(df, path, version=2)`)"
                .to_string(),
        );
    }

    let delim = parse_delimiter(delimiter)?;
    let mut batches = read_batches(bytes)?;

    // Optional column projection (by name or 0-based index), then a row cap.
    if !columns.trim().is_empty() {
        let indices = resolve_columns(columns, batches.first())?;
        batches = batches
            .iter()
            .map(|b| b.project(&indices))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("failed to select columns: {e}"))?;
    }
    if limit > 0 {
        batches = apply_row_limit(batches, limit);
    }

    let mut out: Vec<u8> = Vec::new();
    {
        let mut writer = arrow_csv::WriterBuilder::new()
            .with_header(header)
            .with_delimiter(delim)
            .with_null(null.to_string())
            .build(&mut out);
        for batch in &batches {
            writer
                .write(batch)
                .map_err(|e| format!("failed to write CSV: {e}"))?;
        }
    }
    String::from_utf8(out).map_err(|e| format!("CSV output was not valid UTF-8: {e}"))
}

/// Number of columns and rows in the Arrow table (for a short summary line).
pub fn dimensions(bytes: &[u8]) -> Result<(usize, usize), String> {
    let batches = read_batches(bytes)?;
    let cols = batches.first().map(|b| b.num_columns()).unwrap_or(0);
    let rows = batches.iter().map(|b| b.num_rows()).sum();
    Ok((cols, rows))
}

/// Resolve the CSV delimiter to a single byte.
fn parse_delimiter(delimiter: &str) -> Result<u8, String> {
    let d = if delimiter.is_empty() { "," } else { delimiter };
    match d {
        "\\t" | "tab" | "\t" => return Ok(b'\t'),
        _ => {}
    }
    let bytes = d.as_bytes();
    if bytes.len() != 1 || !bytes[0].is_ascii() {
        return Err(format!(
            "delimiter must be a single ASCII character (or the word \"tab\"); got {d:?}"
        ));
    }
    Ok(bytes[0])
}

/// Resolve a comma-separated `columns` spec to 0-based column indices against
/// the table's schema. Each token is matched as a column NAME first (so a column
/// literally named `"0"` stays reachable), then as a 0-based index. Order and
/// duplicates are preserved. Errors name the unknown token and list the
/// available columns.
fn resolve_columns(spec: &str, first: Option<&RecordBatch>) -> Result<Vec<usize>, String> {
    let batch =
        first.ok_or_else(|| "cannot select columns: the file has no columns".to_string())?;
    let schema = batch.schema();
    let ncols = batch.num_columns();
    let mut indices = Vec::new();
    for raw in spec.split(',') {
        let tok = raw.trim();
        if tok.is_empty() {
            continue;
        }
        if let Ok(i) = schema.index_of(tok) {
            indices.push(i);
        } else if let Ok(i) = tok.parse::<usize>() {
            if i >= ncols {
                return Err(format!(
                    "column index {i} is out of range: the file has {ncols} column(s) (valid 0..={})",
                    ncols.saturating_sub(1)
                ));
            }
            indices.push(i);
        } else {
            let names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
            return Err(format!(
                "column {tok:?} not found; available columns: {}",
                names.join(", ")
            ));
        }
    }
    if indices.is_empty() {
        return Err("no columns selected: `columns` matched nothing".to_string());
    }
    Ok(indices)
}

/// Keep at most `limit` data rows across the batch sequence, slicing the batch
/// that straddles the cap. `limit` is assumed > 0 (the caller guards `0 = all`).
fn apply_row_limit(batches: Vec<RecordBatch>, limit: usize) -> Vec<RecordBatch> {
    let mut out = Vec::new();
    let mut remaining = limit;
    for b in batches {
        if remaining == 0 {
            break;
        }
        let n = b.num_rows();
        if n <= remaining {
            remaining -= n;
            out.push(b);
        } else {
            out.push(b.slice(0, remaining));
            remaining = 0;
        }
    }
    out
}

/// Read every record batch, trying the Arrow IPC "file" format first (what
/// Feather V2 writes), then falling back to the footer-less "stream" format.
fn read_batches(bytes: &[u8]) -> Result<Vec<RecordBatch>, String> {
    if let Ok(reader) = arrow_ipc::reader::FileReader::try_new(Cursor::new(bytes.to_vec()), None) {
        return reader.collect::<Result<Vec<_>, _>>().map_err(decode_err);
    }
    match arrow_ipc::reader::StreamReader::try_new(Cursor::new(bytes.to_vec()), None) {
        Ok(reader) => reader.collect::<Result<Vec<_>, _>>().map_err(decode_err),
        Err(e) => Err(decode_err(e)),
    }
}

/// Turn an Arrow decode error into an actionable message, calling out the one
/// unsupported case (ZSTD compression) by name.
fn decode_err(e: ArrowError) -> String {
    let s = e.to_string();
    if s.to_lowercase().contains("zstd") {
        return format!(
            "this file uses ZSTD-compressed Arrow buffers, which are not supported; re-save it \
             uncompressed or with LZ4 compression ({s})"
        );
    }
    format!("not a readable Arrow IPC / Feather (V2) file: {s}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Int32Array, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    /// A tiny 2-column, 3-row Arrow IPC (file format) fixture:
    /// id: [1, 2, 3]; name: ["Ann", null, "Bo,b"] (the last exercises quoting).
    fn sample_ipc() -> Vec<u8> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("name", DataType::Utf8, true),
        ]));
        let ids = Arc::new(Int32Array::from(vec![1, 2, 3]));
        let names = Arc::new(StringArray::from(vec![Some("Ann"), None, Some("Bo,b")]));
        let batch = RecordBatch::try_new(schema.clone(), vec![ids, names]).unwrap();

        let mut buf: Vec<u8> = Vec::new();
        {
            let mut w = arrow_ipc::writer::FileWriter::try_new(&mut buf, &schema).unwrap();
            w.write(&batch).unwrap();
            w.finish().unwrap();
        }
        buf
    }

    #[test]
    fn converts_ipc_to_csv_with_header() {
        let csv = to_csv(&sample_ipc(), ",", true, "", "", 0).unwrap();
        assert_eq!(csv, "id,name\n1,Ann\n2,\n3,\"Bo,b\"\n");
    }

    #[test]
    fn header_false_omits_column_names() {
        let csv = to_csv(&sample_ipc(), ",", false, "", "", 0).unwrap();
        assert_eq!(csv, "1,Ann\n2,\n3,\"Bo,b\"\n");
    }

    #[test]
    fn tab_delimiter_and_custom_null() {
        let csv = to_csv(&sample_ipc(), "tab", true, "NA", "", 0).unwrap();
        assert_eq!(csv, "id\tname\n1\tAnn\n2\tNA\n3\tBo,b\n");
    }

    #[test]
    fn dimensions_reports_cols_and_rows() {
        assert_eq!(dimensions(&sample_ipc()).unwrap(), (2, 3));
    }

    #[test]
    fn select_columns_by_name_reorders() {
        // Reverse the two columns by name → the header + data follow the request.
        let csv = to_csv(&sample_ipc(), ",", true, "", "name,id", 0).unwrap();
        assert_eq!(csv, "name,id\nAnn,1\n,2\n\"Bo,b\",3\n");
    }

    #[test]
    fn select_columns_by_index_keeps_one() {
        let csv = to_csv(&sample_ipc(), ",", true, "", "0", 0).unwrap();
        assert_eq!(csv, "id\n1\n2\n3\n");
    }

    #[test]
    fn unknown_column_is_reported_with_available() {
        let err = to_csv(&sample_ipc(), ",", true, "", "nope", 0).unwrap_err();
        assert!(err.contains("not found"), "got: {err}");
        assert!(err.contains("id, name"), "got: {err}");
    }

    #[test]
    fn out_of_range_index_is_reported() {
        let err = to_csv(&sample_ipc(), ",", true, "", "5", 0).unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn row_limit_caps_output() {
        let csv = to_csv(&sample_ipc(), ",", true, "", "", 2).unwrap();
        assert_eq!(csv, "id,name\n1,Ann\n2,\n");
    }

    #[test]
    fn row_limit_larger_than_rows_is_all() {
        let csv = to_csv(&sample_ipc(), ",", true, "", "", 99).unwrap();
        assert_eq!(csv, "id,name\n1,Ann\n2,\n3,\"Bo,b\"\n");
    }

    #[test]
    fn columns_and_limit_combine() {
        let csv = to_csv(&sample_ipc(), ",", false, "", "name", 1).unwrap();
        assert_eq!(csv, "Ann\n");
    }

    #[test]
    fn empty_input_errors() {
        let err = to_csv(&[], ",", true, "", "", 0).unwrap_err();
        assert!(err.contains("empty input"), "got: {err}");
    }

    #[test]
    fn garbage_bytes_error_cleanly() {
        let err = to_csv(b"this is not an arrow file at all", ",", true, "", "", 0).unwrap_err();
        assert!(err.contains("not a readable Arrow"), "got: {err}");
    }

    #[test]
    fn legacy_feather_v1_is_flagged() {
        let err = to_csv(b"FEA1\0\0\0\0some bytes", ",", true, "", "", 0).unwrap_err();
        assert!(err.contains("Feather V1"), "got: {err}");
    }

    #[test]
    fn bad_delimiter_rejected() {
        let err = to_csv(&sample_ipc(), ";;", true, "", "", 0).unwrap_err();
        assert!(err.contains("single ASCII character"), "got: {err}");
    }
}
