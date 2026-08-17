//! ndjson-to-matrix core — turn NDJSON / JSON Lines records into an aligned 2D matrix.
//!
//! Every non-blank line is parsed on its own, flattened to dotted column paths, and
//! written into a rectangular grid whose column set is the union of the paths seen
//! across all records. Missing cells get a caller-chosen fill, so the result loads
//! straight into a spreadsheet, numpy/R, or a plotting library.
//!
//! Pure and deterministic: text in, text out, no I/O and no clock.

use serde_json::Value;
use std::collections::HashMap;

/// Maximum accepted input size.
pub const MAX_BYTES: usize = 5_000_000;
/// Maximum accepted number of non-blank input lines.
pub const MAX_LINES: usize = 50_000;
/// Maximum number of distinct column paths the grid may hold.
pub const MAX_COLUMNS: usize = 2_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Arrays {
    Index,
    Json,
    Skip,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Csv,
    Tsv,
    Matrix,
    Json,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
    FirstSeen,
    Alpha,
    Coverage,
}

struct Flatten<'a> {
    separator: &'a str,
    arrays: Arrays,
    max_depth: usize,
}

/// Convert NDJSON text into a matrix / CSV / TSV / JSON table.
///
/// * `data` — one complete JSON value per non-blank line.
/// * `format` — `csv`, `tsv`, `matrix` (whitespace-aligned) or `json` (array of arrays).
/// * `delimiter` — `comma`, `tab`, `semicolon`, `pipe`, `space` or a single character (csv only).
/// * `separator` — the joiner used to build nested column paths (default `.`).
/// * `arrays` — nested arrays become indexed columns, one JSON cell, or are skipped.
/// * `columns` — comma-separated column paths to keep, in that exact order (empty = all).
/// * `column_order` — `first-seen`, `alpha` or `coverage` (most-populated first).
/// * `fill` — text written into cells whose record lacks the path (or holds JSON null).
/// * `headers` — emit the header row.
/// * `row_index` — prepend a 1-based `row` column.
/// * `numeric_only` — keep only columns whose present values are all finite numbers.
/// * `transpose` — emit one row per column and one column per record.
/// * `max_depth` — flatten only this many levels (0 = unlimited); deeper values become JSON text.
/// * `limit` — keep only the first N records (0 = all).
/// * `invalid` — `error` (stop, naming the line) or `skip` (drop unparsable/unsupported lines).
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    format: &str,
    delimiter: &str,
    separator: &str,
    arrays: &str,
    columns: &str,
    column_order: &str,
    fill: &str,
    headers: bool,
    row_index: bool,
    numeric_only: bool,
    transpose: bool,
    max_depth: i64,
    limit: i64,
    invalid: &str,
) -> Result<String, String> {
    if data.len() > MAX_BYTES {
        return Err(format!(
            "input is too large: expected at most {MAX_BYTES} bytes, got {}",
            data.len()
        ));
    }
    let format = parse_format(format)?;
    let arrays = parse_arrays(arrays)?;
    let order = parse_order(column_order)?;
    let strict = parse_invalid(invalid)?;
    let delimiter = match format {
        Format::Tsv => '\t',
        _ => parse_delimiter(delimiter)?,
    };
    if separator.is_empty() {
        return Err("expected a non-empty column path separator, e.g. \".\" or \"_\"".to_string());
    }
    if max_depth < 0 {
        return Err(format!(
            "expected max_depth to be 0 (unlimited) or greater, got {max_depth}"
        ));
    }
    if limit < 0 {
        return Err(format!(
            "expected limit to be 0 (all records) or greater, got {limit}"
        ));
    }
    let cfg = Flatten {
        separator,
        arrays,
        max_depth: max_depth as usize,
    };
    let limit = limit as usize;

    // ---- parse + flatten ------------------------------------------------
    let mut paths: Vec<ColumnMeta> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut rows: Vec<Vec<(usize, Option<String>)>> = Vec::new();
    let mut skipped = 0usize;
    let mut lines_seen = 0usize;

    for (idx, raw) in data.lines().enumerate() {
        let line_no = idx + 1;
        let mut line = raw.trim_end_matches('\r');
        if idx == 0 {
            line = line.trim_start_matches('\u{feff}');
        }
        if line.trim().is_empty() {
            continue;
        }
        lines_seen += 1;
        if lines_seen > MAX_LINES {
            return Err(format!(
                "too many records: expected at most {MAX_LINES} non-blank lines, got more at line {line_no}"
            ));
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                if strict {
                    return Err(format!(
                        "invalid JSON on line {line_no}: {} — expected one complete JSON value per line",
                        clean_serde_error(&e.to_string())
                    ));
                }
                skipped += 1;
                continue;
            }
        };
        let mut pairs: Vec<(String, Option<String>)> = Vec::new();
        match &value {
            Value::Object(_) => flatten(&cfg, "", &value, 0, &mut pairs),
            // A bare JSON array line is already a matrix row: positional columns 0,1,2…
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    flatten(&cfg, &i.to_string(), item, 1, &mut pairs);
                }
            }
            _ => pairs.push(("value".to_string(), render_leaf(&value))),
        }

        let mut row: Vec<(usize, Option<String>)> = Vec::with_capacity(pairs.len());
        for (path, cell) in pairs {
            let col = match index.get(&path) {
                Some(&i) => i,
                None => {
                    if paths.len() >= MAX_COLUMNS {
                        return Err(format!(
                            "too many columns: expected at most {MAX_COLUMNS} distinct paths, hit the cap at line {line_no} — lower max_depth, set arrays=json, or select columns"
                        ));
                    }
                    index.insert(path.clone(), paths.len());
                    paths.push(ColumnMeta::new(path));
                    paths.len() - 1
                }
            };
            let meta = &mut paths[col];
            if let Some(text) = &cell {
                meta.present += 1;
                if is_numeric_text(text) {
                    meta.numeric += 1;
                }
            }
            // A duplicate path within one record keeps the last value written.
            if let Some(slot) = row.iter_mut().find(|(c, _)| *c == col) {
                slot.1 = cell;
            } else {
                row.push((col, cell));
            }
        }
        rows.push(row);
        if limit > 0 && rows.len() >= limit {
            break;
        }
    }

    if rows.is_empty() {
        return if skipped > 0 {
            Err(format!(
                "no valid records: all {skipped} non-blank line(s) failed to parse — set invalid=error to see the first parse error"
            ))
        } else {
            Err("expected NDJSON input: one complete JSON value per line, got no records".to_string())
        };
    }

    // ---- choose + order columns ----------------------------------------
    let mut kept: Vec<usize> = (0..paths.len()).collect();
    if numeric_only {
        kept.retain(|&i| paths[i].present > 0 && paths[i].numeric == paths[i].present);
        if kept.is_empty() {
            return Err(
                "no numeric columns: numeric_only kept nothing — every column holds at least one non-numeric value"
                    .to_string(),
            );
        }
    }
    match order {
        Order::FirstSeen => {}
        Order::Alpha => kept.sort_by(|&a, &b| paths[a].path.cmp(&paths[b].path)),
        Order::Coverage => kept.sort_by(|&a, &b| {
            paths[b]
                .present
                .cmp(&paths[a].present)
                .then_with(|| a.cmp(&b))
        }),
    }
    let selection = columns.trim();
    if !selection.is_empty() {
        let mut chosen = Vec::new();
        for want in selection.split(',') {
            let want = want.trim();
            if want.is_empty() {
                continue;
            }
            match kept.iter().copied().find(|&i| paths[i].path == want) {
                Some(i) => chosen.push(i),
                None => {
                    return Err(format!(
                        "unknown column \"{want}\": available columns are {}",
                        preview_paths(&paths, &kept)
                    ))
                }
            }
        }
        if chosen.is_empty() {
            return Err("expected columns to name at least one column path, got only separators"
                .to_string());
        }
        kept = chosen;
    }

    // ---- build the grid -------------------------------------------------
    let mut header: Vec<String> = kept.iter().map(|&i| paths[i].path.clone()).collect();
    let mut grid: Vec<Vec<String>> = Vec::with_capacity(rows.len());
    let mut scratch: Vec<Option<String>> = vec![None; paths.len()];
    for row in &rows {
        for (col, cell) in row {
            scratch[*col] = cell.clone();
        }
        let mut out = Vec::with_capacity(kept.len());
        for &col in &kept {
            out.push(scratch[col].clone().unwrap_or_else(|| fill.to_string()));
        }
        grid.push(out);
        for (col, _) in row {
            scratch[*col] = None;
        }
    }
    if row_index {
        header.insert(0, "row".to_string());
        for (i, row) in grid.iter_mut().enumerate() {
            row.insert(0, (i + 1).to_string());
        }
    }
    if transpose {
        let (h, g) = transpose_grid(&header, &grid, headers);
        header = h;
        grid = g;
    }

    let mut table: Vec<Vec<String>> = Vec::with_capacity(grid.len() + 1);
    if headers {
        table.push(header);
    }
    table.extend(grid);

    Ok(match format {
        Format::Csv | Format::Tsv => render_delimited(&table, delimiter),
        Format::Matrix => render_matrix(&table),
        Format::Json => render_json(&table),
    })
}

struct ColumnMeta {
    path: String,
    present: usize,
    numeric: usize,
}

impl ColumnMeta {
    fn new(path: String) -> Self {
        ColumnMeta {
            path,
            present: 0,
            numeric: 0,
        }
    }
}

fn flatten(cfg: &Flatten, path: &str, value: &Value, depth: usize, out: &mut Vec<(String, Option<String>)>) {
    let container = value.is_object() || value.is_array();
    if container && cfg.max_depth > 0 && depth >= cfg.max_depth {
        out.push((path.to_string(), Some(value.to_string())));
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                flatten(cfg, &join(path, key, cfg.separator), child, depth + 1, out);
            }
        }
        Value::Array(items) => match cfg.arrays {
            Arrays::Index => {
                for (i, item) in items.iter().enumerate() {
                    flatten(
                        cfg,
                        &join(path, &i.to_string(), cfg.separator),
                        item,
                        depth + 1,
                        out,
                    );
                }
            }
            Arrays::Json => out.push((path.to_string(), Some(value.to_string()))),
            Arrays::Skip => {}
        },
        leaf => out.push((path.to_string(), render_leaf(leaf))),
    }
}

fn join(prefix: &str, key: &str, separator: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}{separator}{key}")
    }
}

/// JSON null is treated as a missing cell so the caller's fill applies to it too.
fn render_leaf(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

fn is_numeric_text(text: &str) -> bool {
    let t = text.trim();
    !t.is_empty() && t.parse::<f64>().map(f64::is_finite).unwrap_or(false)
}

fn transpose_grid(
    header: &[String],
    grid: &[Vec<String>],
    headers: bool,
) -> (Vec<String>, Vec<Vec<String>>) {
    let width = header.len();
    let mut out_header = Vec::with_capacity(grid.len() + 1);
    if headers {
        out_header.push("column".to_string());
    }
    for i in 0..grid.len() {
        out_header.push((i + 1).to_string());
    }
    let mut out_grid = Vec::with_capacity(width);
    for col in 0..width {
        let mut row = Vec::with_capacity(grid.len() + 1);
        if headers {
            row.push(header[col].clone());
        }
        for record in grid {
            row.push(record.get(col).cloned().unwrap_or_default());
        }
        out_grid.push(row);
    }
    (out_header, out_grid)
}

fn render_delimited(table: &[Vec<String>], delimiter: char) -> String {
    let mut out = String::new();
    for (i, row) in table.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        for (j, cell) in row.iter().enumerate() {
            if j > 0 {
                out.push(delimiter);
            }
            out.push_str(&quote_cell(cell, delimiter));
        }
    }
    out
}

fn quote_cell(cell: &str, delimiter: char) -> String {
    let needs = cell.contains(delimiter)
        || cell.contains('"')
        || cell.contains('\n')
        || cell.contains('\r');
    if needs {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

fn render_matrix(table: &[Vec<String>]) -> String {
    let width = table.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; width];
    let mut right = vec![true; width];
    for row in table {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    // A column is right-aligned only when every data cell in it looks numeric.
    for (r, row) in table.iter().enumerate() {
        for (i, cell) in row.iter().enumerate() {
            let is_header_row = r == 0 && table.len() > 1;
            if !is_header_row && !cell.is_empty() && !is_numeric_text(cell) {
                right[i] = false;
            }
        }
    }
    let mut out = String::new();
    for (r, row) in table.iter().enumerate() {
        if r > 0 {
            out.push('\n');
        }
        let mut line = String::new();
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                line.push_str("  ");
            }
            let pad = widths[i].saturating_sub(cell.chars().count());
            if right[i] {
                line.push_str(&" ".repeat(pad));
                line.push_str(cell);
            } else {
                line.push_str(cell);
                line.push_str(&" ".repeat(pad));
            }
        }
        out.push_str(line.trim_end());
    }
    out
}

fn render_json(table: &[Vec<String>]) -> String {
    let mut out = String::from("[\n");
    for (r, row) in table.iter().enumerate() {
        out.push_str("  [");
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            if cell.is_empty() {
                out.push_str("null");
            } else if is_numeric_text(cell) {
                out.push_str(cell.trim());
            } else {
                out.push_str(&Value::String(cell.clone()).to_string());
            }
        }
        out.push(']');
        if r + 1 < table.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push(']');
    out
}

fn preview_paths(paths: &[ColumnMeta], kept: &[usize]) -> String {
    let names: Vec<&str> = kept
        .iter()
        .take(20)
        .map(|&i| paths[i].path.as_str())
        .collect();
    if kept.len() > 20 {
        format!("{} … (+{} more)", names.join(", "), kept.len() - 20)
    } else {
        names.join(", ")
    }
}

/// serde_json reports positions relative to the single line it was handed; rewrite
/// "at line 1 column N" so the caller's own line number stays unambiguous.
fn clean_serde_error(msg: &str) -> String {
    match msg.find(" at line 1 column ") {
        Some(pos) => format!("{} at column {}", &msg[..pos], &msg[pos + 18..]),
        None => msg.to_string(),
    }
}

fn parse_format(value: &str) -> Result<Format, String> {
    match value.trim() {
        "" | "csv" => Ok(Format::Csv),
        "tsv" => Ok(Format::Tsv),
        "matrix" => Ok(Format::Matrix),
        "json" => Ok(Format::Json),
        other => Err(format!(
            "expected format to be csv, tsv, matrix or json, got \"{other}\""
        )),
    }
}

fn parse_arrays(value: &str) -> Result<Arrays, String> {
    match value.trim() {
        "" | "index" => Ok(Arrays::Index),
        "json" => Ok(Arrays::Json),
        "skip" => Ok(Arrays::Skip),
        other => Err(format!(
            "expected arrays to be index, json or skip, got \"{other}\""
        )),
    }
}

fn parse_order(value: &str) -> Result<Order, String> {
    match value.trim() {
        "" | "first-seen" => Ok(Order::FirstSeen),
        "alpha" => Ok(Order::Alpha),
        "coverage" => Ok(Order::Coverage),
        other => Err(format!(
            "expected column_order to be first-seen, alpha or coverage, got \"{other}\""
        )),
    }
}

/// Returns true when a bad line must abort the run.
fn parse_invalid(value: &str) -> Result<bool, String> {
    match value.trim() {
        "" | "error" => Ok(true),
        "skip" => Ok(false),
        other => Err(format!(
            "expected invalid to be error or skip, got \"{other}\""
        )),
    }
}

fn parse_delimiter(value: &str) -> Result<char, String> {
    match value.trim() {
        "" | "comma" | "," => Ok(','),
        "tab" | "\\t" => Ok('\t'),
        "semicolon" | ";" => Ok(';'),
        "pipe" | "|" => Ok('|'),
        "space" => Ok(' '),
        other => {
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Ok(c),
                _ => Err(format!(
                    "expected delimiter to be a single character or one of comma, tab, semicolon, pipe, space; got \"{other}\""
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv(data: &str) -> String {
        run(
            data, "csv", "comma", ".", "index", "", "first-seen", "", true, false, false, false, 0,
            0, "error",
        )
        .unwrap()
    }

    #[test]
    fn unions_columns_across_heterogeneous_records() {
        let out = csv("{\"a\":1,\"b\":2}\n{\"b\":3,\"c\":4}");
        assert_eq!(out, "a,b,c\n1,2,\n,3,4");
    }

    #[test]
    fn flattens_nested_objects_to_dotted_paths() {
        let out = csv("{\"user\":{\"id\":7,\"geo\":{\"lat\":1.5}}}");
        assert_eq!(out, "user.id,user.geo.lat\n7,1.5");
    }

    #[test]
    fn indexes_nested_arrays_into_columns() {
        let out = csv("{\"v\":[1,2,3]}\n{\"v\":[4,5]}");
        assert_eq!(out, "v.0,v.1,v.2\n1,2,3\n4,5,");
    }

    #[test]
    fn bare_array_line_becomes_positional_row() {
        let out = csv("[1,2]\n[3,4]");
        assert_eq!(out, "0,1\n1,2\n3,4");
    }

    #[test]
    fn scalar_line_becomes_a_value_column() {
        let out = csv("1\n2.5");
        assert_eq!(out, "value\n1\n2.5");
    }

    #[test]
    fn arrays_json_keeps_the_array_in_one_cell() {
        let out = run(
            "{\"v\":[1,2]}", "csv", "comma", ".", "json", "", "first-seen", "", true, false, false,
            false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "v\n\"[1,2]\"");
    }

    #[test]
    fn arrays_skip_drops_array_columns() {
        let out = run(
            "{\"id\":1,\"v\":[1,2]}", "csv", "comma", ".", "skip", "", "first-seen", "", true,
            false, false, false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "id\n1");
    }

    #[test]
    fn fill_replaces_missing_and_null_cells() {
        let out = run(
            "{\"a\":1,\"b\":null}\n{\"a\":2}", "csv", "comma", ".", "index", "", "first-seen", "0",
            true, false, false, false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "a,b\n1,0\n2,0");
    }

    #[test]
    fn headers_off_emits_data_rows_only() {
        let out = run(
            "{\"a\":1}\n{\"a\":2}", "csv", "comma", ".", "index", "", "first-seen", "", false,
            false, false, false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "1\n2");
    }

    #[test]
    fn tsv_format_uses_tabs() {
        let out = run(
            "{\"a\":1,\"b\":2}", "tsv", "comma", ".", "index", "", "first-seen", "", true, false,
            false, false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "a\tb\n1\t2");
    }

    #[test]
    fn custom_delimiter_is_honoured() {
        let out = run(
            "{\"a\":1,\"b\":2}", "csv", "semicolon", ".", "index", "", "first-seen", "", true,
            false, false, false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "a;b\n1;2");
    }

    #[test]
    fn matrix_format_right_aligns_numeric_columns() {
        let out = run(
            "{\"a\":1,\"b\":22}\n{\"a\":333,\"b\":4}", "matrix", "comma", ".", "index", "",
            "first-seen", "", true, false, false, false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "  a   b\n  1  22\n333   4");
    }

    #[test]
    fn json_format_emits_array_of_arrays() {
        let out = run(
            "{\"a\":1,\"b\":\"x\"}\n{\"a\":2}", "json", "comma", ".", "index", "", "first-seen",
            "", true, false, false, false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(
            out,
            "[\n  [\"a\", \"b\"],\n  [1, \"x\"],\n  [2, null]\n]"
        );
    }

    #[test]
    fn numeric_only_drops_non_numeric_columns() {
        let out = run(
            "{\"id\":\"a1\",\"ms\":12}\n{\"id\":\"a2\",\"ms\":31}", "csv", "comma", ".", "index",
            "", "first-seen", "", true, false, true, false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "ms\n12\n31");
    }

    #[test]
    fn column_selection_sets_order_and_subset() {
        let out = run(
            "{\"a\":1,\"b\":2,\"c\":3}", "csv", "comma", ".", "index", "c, a", "first-seen", "",
            true, false, false, false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "c,a\n3,1");
    }

    #[test]
    fn alpha_and_coverage_orders_differ_from_first_seen() {
        let data = "{\"b\":1,\"a\":2}\n{\"b\":3}";
        let alpha = run(
            data, "csv", "comma", ".", "index", "", "alpha", "", true, false, false, false, 0, 0,
            "error",
        )
        .unwrap();
        assert_eq!(alpha, "a,b\n2,1\n,3");
        let coverage = run(
            data, "csv", "comma", ".", "index", "", "coverage", "", true, false, false, false, 0,
            0, "error",
        )
        .unwrap();
        assert_eq!(coverage, "b,a\n1,2\n3,");
    }

    #[test]
    fn row_index_prepends_a_row_column() {
        let out = run(
            "{\"a\":9}\n{\"a\":8}", "csv", "comma", ".", "index", "", "first-seen", "", true, true,
            false, false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "row,a\n1,9\n2,8");
    }

    #[test]
    fn transpose_swaps_records_and_columns() {
        let out = run(
            "{\"a\":1,\"b\":2}\n{\"a\":3,\"b\":4}", "csv", "comma", ".", "index", "",
            "first-seen", "", true, false, false, true, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "column,1,2\na,1,3\nb,2,4");
    }

    #[test]
    fn transpose_without_headers_is_a_bare_grid() {
        let out = run(
            "{\"a\":1,\"b\":2}\n{\"a\":3,\"b\":4}", "csv", "comma", ".", "index", "",
            "first-seen", "", false, false, false, true, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "1,3\n2,4");
    }

    #[test]
    fn max_depth_stringifies_deeper_values() {
        let out = run(
            "{\"a\":{\"b\":{\"c\":1}}}", "csv", "comma", ".", "index", "", "first-seen", "", true,
            false, false, false, 2, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "a.b\n\"{\"\"c\"\":1}\"");
    }

    #[test]
    fn limit_caps_the_record_count() {
        let out = run(
            "{\"a\":1}\n{\"a\":2}\n{\"a\":3}", "csv", "comma", ".", "index", "", "first-seen", "",
            true, false, false, false, 0, 2, "error",
        )
        .unwrap();
        assert_eq!(out, "a\n1\n2");
    }

    #[test]
    fn custom_separator_changes_path_joiner() {
        let out = run(
            "{\"a\":{\"b\":1}}", "csv", "comma", "_", "index", "", "first-seen", "", true, false,
            false, false, 0, 0, "error",
        )
        .unwrap();
        assert_eq!(out, "a_b\n1");
    }

    #[test]
    fn quotes_cells_containing_the_delimiter_or_quotes() {
        let out = csv("{\"a\":\"x,y\",\"b\":\"say \\\"hi\\\"\"}");
        assert_eq!(out, "a,b\n\"x,y\",\"say \"\"hi\"\"\"");
    }

    #[test]
    fn crlf_and_bom_inputs_parse() {
        let out = csv("\u{feff}{\"a\":1}\r\n{\"a\":2}\r\n");
        assert_eq!(out, "a\n1\n2");
    }

    #[test]
    fn invalid_line_error_names_the_line_number() {
        let err = run(
            "{\"a\":1}\n{oops}", "csv", "comma", ".", "index", "", "first-seen", "", true, false,
            false, false, 0, 0, "error",
        )
        .unwrap_err();
        assert!(err.starts_with("invalid JSON on line 2:"), "got {err}");
        assert!(err.contains("column"), "got {err}");
    }

    #[test]
    fn invalid_skip_drops_bad_lines() {
        let out = run(
            "{\"a\":1}\n{oops}\n{\"a\":2}", "csv", "comma", ".", "index", "", "first-seen", "",
            true, false, false, false, 0, 0, "skip",
        )
        .unwrap();
        assert_eq!(out, "a\n1\n2");
    }

    #[test]
    fn all_lines_invalid_is_an_error_even_when_skipping() {
        let err = run(
            "{oops}\n{nope}", "csv", "comma", ".", "index", "", "first-seen", "", true, false,
            false, false, 0, 0, "skip",
        )
        .unwrap_err();
        assert!(err.contains("no valid records"), "got {err}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run(
            "   \n\n", "csv", "comma", ".", "index", "", "first-seen", "", true, false, false,
            false, 0, 0, "error",
        )
        .unwrap_err();
        assert!(err.contains("got no records"), "got {err}");
    }

    #[test]
    fn unknown_selected_column_lists_available_columns() {
        let err = run(
            "{\"a\":1}", "csv", "comma", ".", "index", "zzz", "first-seen", "", true, false, false,
            false, 0, 0, "error",
        )
        .unwrap_err();
        assert_eq!(err, "unknown column \"zzz\": available columns are a");
    }

    #[test]
    fn numeric_only_with_no_numeric_columns_errors() {
        let err = run(
            "{\"a\":\"x\"}", "csv", "comma", ".", "index", "", "first-seen", "", true, false, true,
            false, 0, 0, "error",
        )
        .unwrap_err();
        assert!(err.contains("no numeric columns"), "got {err}");
    }

    #[test]
    fn bad_enum_values_are_rejected_with_the_expected_set() {
        for (format, arrays, order, invalid, needle) in [
            ("nope", "index", "first-seen", "error", "expected format to be"),
            ("csv", "nope", "first-seen", "error", "expected arrays to be"),
            ("csv", "index", "nope", "error", "expected column_order to be"),
            ("csv", "index", "first-seen", "nope", "expected invalid to be"),
        ] {
            let err = run(
                "{\"a\":1}", format, "comma", ".", arrays, "", order, "", true, false, false,
                false, 0, 0, invalid,
            )
            .unwrap_err();
            assert!(err.contains(needle), "got {err}");
        }
    }

    #[test]
    fn bad_delimiter_and_separator_are_rejected() {
        let err = run(
            "{\"a\":1}", "csv", "abc", ".", "index", "", "first-seen", "", true, false, false,
            false, 0, 0, "error",
        )
        .unwrap_err();
        assert!(err.contains("expected delimiter to be"), "got {err}");
        let err = run(
            "{\"a\":1}", "csv", "comma", "", "index", "", "first-seen", "", true, false, false,
            false, 0, 0, "error",
        )
        .unwrap_err();
        assert!(err.contains("non-empty column path separator"), "got {err}");
    }

    #[test]
    fn negative_numeric_params_are_rejected() {
        let err = run(
            "{\"a\":1}", "csv", "comma", ".", "index", "", "first-seen", "", true, false, false,
            false, -1, 0, "error",
        )
        .unwrap_err();
        assert!(err.contains("max_depth"), "got {err}");
        let err = run(
            "{\"a\":1}", "csv", "comma", ".", "index", "", "first-seen", "", true, false, false,
            false, 0, -5, "error",
        )
        .unwrap_err();
        assert!(err.contains("limit"), "got {err}");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "x".repeat(MAX_BYTES + 1);
        let err = run(
            &big, "csv", "comma", ".", "index", "", "first-seen", "", true, false, false, false, 0,
            0, "error",
        )
        .unwrap_err();
        assert!(err.contains("input is too large"), "got {err}");
    }

    #[test]
    fn line_cap_is_enforced() {
        let mut data = String::new();
        for i in 0..(MAX_LINES + 1) {
            data.push_str(&format!("{{\"a\":{i}}}\n"));
        }
        let err = run(
            &data, "csv", "comma", ".", "index", "", "first-seen", "", true, false, false, false,
            0, 0, "error",
        )
        .unwrap_err();
        assert!(err.contains("too many records"), "got {err}");
    }
}
