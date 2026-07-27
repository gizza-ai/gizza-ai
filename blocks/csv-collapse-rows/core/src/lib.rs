//! gizza-ai/csv-collapse-rows core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps. Groups CSV rows by one or more
//! key columns and collapses a chosen column's values from every row in the group
//! into a single delimited cell (the CSV equivalent of SQL `GROUP_CONCAT`). Groups
//! are emitted in first-seen order.

use std::collections::HashMap;

/// Resolve a single-character CSV field delimiter from a name or literal.
fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Resolve a column reference to a 0-based index. When a header row is present
/// (`Some`) a reference may be a header name OR a 1-based index; without a header
/// (`None`) only 1-based indices are accepted.
fn resolve(name: &str, header: Option<&csv::StringRecord>) -> Result<usize, String> {
    if let Ok(n) = name.parse::<usize>() {
        if n == 0 {
            return Err("column index is 1-based (>= 1)".into());
        }
        return Ok(n - 1);
    }
    match header {
        Some(h) => h
            .iter()
            .position(|c| c == name)
            .ok_or_else(|| format!("column '{name}' not found in the header")),
        None => Err(format!(
            "with has_header=false, columns must be 1-based indices — got '{name}'"
        )),
    }
}

/// Output label for column `idx`: the header name when present, else a generated
/// `col_N` label (N = 1-based position) so headerless input still yields a header.
fn col_label(header: Option<&csv::StringRecord>, idx: usize) -> String {
    match header {
        Some(h) => h.get(idx).unwrap_or("").to_string(),
        None => format!("col_{}", idx + 1),
    }
}

/// How to order the collapsed values inside each cell.
#[derive(Clone, Copy, PartialEq)]
enum Sort {
    None,
    Asc,
    Desc,
}

impl Sort {
    fn parse(s: &str) -> Result<Sort, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" => Ok(Sort::None),
            "asc" | "ascending" => Ok(Sort::Asc),
            "desc" | "descending" => Ok(Sort::Desc),
            other => Err(format!("sort_values must be none/asc/desc, got '{other}'")),
        }
    }
}

/// Group `data` by `key_columns` (comma-separated names/indices) and collapse the
/// `collapse_column` values within each group into one `separator`-joined cell.
///
/// - `separator` — string inserted between values (empty ⇒ `", "`).
/// - `dedupe` — drop repeated values within a group (keeps first-seen order).
/// - `skip_empty` — ignore blank cells in the collapse column.
/// - `sort_values` — `none` (first-seen), `asc`, or `desc` (lexicographic).
/// - `delimiter` — CSV field separator for input AND output.
/// - `has_header` — when `true` the first row is a header and column refs may be
///   names or 1-based indices; when `false` every row is data, refs must be
///   1-based indices, and output columns are labelled `col_N`.
///
/// Output columns = the key columns followed by the collapse column; one row per
/// group, in first-seen order. Other columns are dropped.
#[allow(clippy::too_many_arguments)]
pub fn collapse_rows(
    data: &str,
    key_columns: &str,
    collapse_column: &str,
    separator: &str,
    dedupe: bool,
    skip_empty: bool,
    sort_values: &str,
    delimiter: &str,
    has_header: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;
    let sort = Sort::parse(sort_values)?;
    let sep = if separator.is_empty() { ", " } else { separator };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    // With a header, the first record is the header row and data starts at index 1;
    // without one, every record is a data row.
    let header: Option<&csv::StringRecord> = if has_header { Some(&records[0]) } else { None };
    let data_start = if has_header { 1 } else { 0 };

    let kcols: Vec<usize> = {
        let toks: Vec<&str> = key_columns
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if toks.is_empty() {
            return Err("key_columns: at least one column is required".into());
        }
        toks.iter().map(|t| resolve(t, header)).collect::<Result<_, _>>()?
    };
    let knames: Vec<String> = kcols.iter().map(|&c| col_label(header, c)).collect();

    let ccol_ref = collapse_column.trim();
    if ccol_ref.is_empty() {
        return Err("collapse_column is required".into());
    }
    let ccol = resolve(ccol_ref, header)?;
    let cname = col_label(header, ccol);

    // group key -> (key values, collected collapse values), first-seen order in `order`.
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    for rec in records.iter().skip(data_start) {
        let key = kcols
            .iter()
            .map(|&c| rec.get(c).unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\u{1}");
        let entry = groups.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            (
                kcols.iter().map(|&c| rec.get(c).unwrap_or("").to_string()).collect(),
                Vec::new(),
            )
        });
        let cell = rec.get(ccol).unwrap_or("");
        if skip_empty && cell.trim().is_empty() {
            continue;
        }
        entry.1.push(cell.to_string());
    }

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).from_writer(vec![]);
    let mut out_header = knames.clone();
    out_header.push(cname);
    wtr.write_record(&out_header)
        .map_err(|e| format!("CSV write error: {e}"))?;

    for key in &order {
        let (kvals, vals) = &groups[key];
        let mut vals: Vec<String> = vals.clone();
        if dedupe {
            let mut seen = std::collections::HashSet::new();
            vals.retain(|v| seen.insert(v.clone()));
        }
        match sort {
            Sort::None => {}
            Sort::Asc => vals.sort(),
            Sort::Desc => {
                vals.sort();
                vals.reverse();
            }
        }
        let mut row = kvals.clone();
        row.push(vals.join(sep));
        wtr.write_record(&row)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }

    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_values_per_group() {
        let d = "region,product\nEast,Apple\nWest,Banana\nEast,Cherry\nEast,Apple";
        let out =
            collapse_rows(d, "region", "product", ", ", false, true, "none", "comma", true).unwrap();
        assert_eq!(
            out,
            "region,product\nEast,\"Apple, Cherry, Apple\"\nWest,Banana\n"
        );
    }

    #[test]
    fn dedupe_and_sort_desc() {
        let d = "region,product\nEast,Apple\nWest,Banana\nEast,Cherry\nEast,Apple";
        let out =
            collapse_rows(d, "region", "product", ", ", true, true, "desc", "comma", true).unwrap();
        assert_eq!(out, "region,product\nEast,\"Cherry, Apple\"\nWest,Banana\n");
    }

    #[test]
    fn multi_key_and_custom_separator() {
        let d = "a,b,v\nx,1,foo\nx,1,bar\nx,2,baz";
        let out = collapse_rows(d, "a,b", "v", " | ", false, true, "none", "comma", true).unwrap();
        assert_eq!(out, "a,b,v\nx,1,foo | bar\nx,2,baz\n");
    }

    #[test]
    fn skip_empty_drops_blank_cells() {
        let d = "g,v\nx,a\nx,\nx,b";
        let with = collapse_rows(d, "g", "v", ",", false, true, "none", "comma", true).unwrap();
        assert_eq!(with, "g,v\nx,\"a,b\"\n");
        let without = collapse_rows(d, "g", "v", ",", false, false, "none", "comma", true).unwrap();
        assert_eq!(without, "g,v\nx,\"a,,b\"\n");
    }

    #[test]
    fn no_header_uses_indices_and_generated_labels() {
        // No header row: every row is data, columns referenced by 1-based index,
        // output header is generated (col_1, col_2, ...).
        let d = "East,Apple\nWest,Banana\nEast,Cherry";
        let out =
            collapse_rows(d, "1", "2", ", ", false, true, "none", "comma", false).unwrap();
        assert_eq!(out, "col_1,col_2\nEast,\"Apple, Cherry\"\nWest,Banana\n");
    }

    #[test]
    fn errors() {
        // empty input
        assert!(collapse_rows("  ", "a", "b", ",", false, true, "none", "comma", true).is_err());
        // key column not found
        assert!(collapse_rows("a,b\n1,2", "nope", "b", ",", false, true, "none", "comma", true)
            .is_err());
        // collapse column not found
        assert!(collapse_rows("a,b\n1,2", "a", "nope", ",", false, true, "none", "comma", true)
            .is_err());
        // no header + column referenced by name (only indices allowed without a header)
        assert!(collapse_rows("a,b\n1,2", "a", "b", ",", false, true, "none", "comma", false)
            .is_err());
        // index 0 is rejected (indices are 1-based)
        assert!(collapse_rows("a,b\n1,2", "0", "2", ",", false, true, "none", "comma", true)
            .is_err());
        // bad sort mode
        assert!(collapse_rows("a,b\n1,2", "a", "b", ",", false, true, "sideways", "comma", true)
            .is_err());
    }
}
