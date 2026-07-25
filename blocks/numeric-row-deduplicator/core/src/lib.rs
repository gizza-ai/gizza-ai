//! gizza-ai/numeric-row-deduplicator core — pure compute, shared by the chat
//! skill block and the web page. No wafer/wasm-bindgen deps. Removes duplicate
//! numeric rows from a table, comparing each cell by NUMERIC VALUE so that
//! different textual forms of the same number (1, 1.0, 1.00, +1, 1e0, 100e-2)
//! all count as the same value. Non-numeric cells fall back to a trimmed-string
//! compare so mixed tables still work. Optionally keys on a subset of columns,
//! rounds to a chosen precision before comparing, and keeps the first or last
//! occurrence — preserving the original order of the kept rows.

use std::collections::HashMap;

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
                    "delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Resolve `columns_csv` (1-based indices and/or header names) to 0-based column
/// indices. Empty → None (key on the whole row).
fn resolve_columns(
    columns_csv: &str,
    header: Option<&csv::StringRecord>,
) -> Result<Option<Vec<usize>>, String> {
    let toks: Vec<&str> = columns_csv
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if toks.is_empty() {
        return Ok(None);
    }
    let mut idxs = Vec::new();
    for t in toks {
        if let Ok(n) = t.parse::<usize>() {
            if n == 0 {
                return Err("column indices are 1-based (>= 1)".into());
            }
            idxs.push(n - 1);
        } else if let Some(h) = header {
            match h.iter().position(|c| c == t) {
                Some(p) => idxs.push(p),
                None => return Err(format!("column '{t}' not found in the header")),
            }
        } else {
            return Err(format!(
                "column '{t}' is not a number and there is no header to match names"
            ));
        }
    }
    Ok(Some(idxs))
}

/// Canonical comparison key for one cell. If it parses as a finite number, the
/// key is its numeric value (so 1, 1.0, 1.00, +1, 1e0 all collapse), optionally
/// rounded to `precision` decimals. Otherwise the trimmed text is used verbatim,
/// tagged so the string "1" and a stray token never collide across the two paths.
fn cell_key(cell: &str, precision: i64) -> String {
    let t = cell.trim();
    match t.parse::<f64>() {
        Ok(v) if v.is_finite() => {
            if precision >= 0 {
                // Fixed-decimal render is a stable canonical key; normalize -0.
                let v = if v == 0.0 { 0.0 } else { v };
                format!("n:{:.*}", precision as usize, v)
            } else {
                // Shortest round-trip repr canonicalizes equal values; normalize -0.
                let v = if v == 0.0 { 0.0 } else { v };
                format!("n:{v}")
            }
        }
        _ => format!("s:{t}"),
    }
}

/// Remove duplicate numeric rows (comparing cells by numeric value). `has_header`
/// preserves + ignores row 1 for dedup. `columns` keys on a subset; empty keys on
/// the whole row. `precision` >= 0 rounds numeric cells to that many decimals
/// before comparing (-1 = exact numeric value). `keep` is "first" or "last": the
/// kept occurrence of each key, always emitted in original row order.
pub fn dedupe_numeric(
    data: &str,
    columns: &str,
    has_header: bool,
    delimiter: &str,
    precision: i64,
    keep: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if !(-1..=12).contains(&precision) {
        return Err(format!(
            "precision must be between -1 (exact) and 12, got {precision}"
        ));
    }
    let keep_last = match keep {
        "" | "first" => false,
        "last" => true,
        other => return Err(format!("keep must be 'first' or 'last', got '{other}'")),
    };
    let delim = delim_byte(delimiter)?;
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
        return Ok(String::new());
    }

    let header = if has_header { records.first() } else { None };
    let key_cols = resolve_columns(columns, header)?;

    let body_start = if has_header { 1 } else { 0 };
    // Map row key → the row index we keep for that key (first or last seen).
    let mut kept: HashMap<String, usize> = HashMap::new();
    for i in body_start..records.len() {
        let rec = &records[i];
        let key = match &key_cols {
            Some(cols) => cols
                .iter()
                .map(|&c| cell_key(rec.get(c).unwrap_or(""), precision))
                .collect::<Vec<_>>()
                .join("\u{1}"),
            None => rec
                .iter()
                .map(|c| cell_key(c, precision))
                .collect::<Vec<_>>()
                .join("\u{1}"),
        };
        kept.entry(key)
            .and_modify(|idx| {
                if keep_last {
                    *idx = i;
                }
            })
            .or_insert(i);
    }
    let keep_set: std::collections::HashSet<usize> = kept.into_values().collect();

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    for (i, rec) in records.iter().enumerate() {
        if (has_header && i == 0) || keep_set.contains(&i) {
            wtr.write_record(rec)
                .map_err(|e| format!("CSV write error: {e}"))?;
        }
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_numeric_representations() {
        // 1, 1.0, 1.00, +1, 1e0 are all the number 1 → one row kept (the first).
        let d = "1,2\n1.0,2.0\n1.00,2\n+1,2e0\n3,4";
        assert_eq!(
            dedupe_numeric(d, "", false, ",", -1, "first").unwrap(),
            "1,2\n3,4\n"
        );
    }

    #[test]
    fn plain_string_dedupe_would_keep_all() {
        // Sanity: the same rows differ as raw strings — proves the numeric compare.
        let d = "0.5\n.5\n0.50";
        assert_eq!(
            dedupe_numeric(d, "", false, ",", -1, "first").unwrap(),
            "0.5\n"
        );
    }

    #[test]
    fn keyed_on_column_name_with_header() {
        let d = "id,score\n1,90.0\n1,91\n2,80";
        // key on id only → keep first row per id
        assert_eq!(
            dedupe_numeric(d, "id", true, ",", -1, "first").unwrap(),
            "id,score\n1,90.0\n2,80\n"
        );
    }

    #[test]
    fn precision_rounding_collapses_near_duplicates() {
        let d = "0.30000000000000004\n0.3\n0.31";
        // round to 2 decimals → first two collapse to 0.30
        assert_eq!(
            dedupe_numeric(d, "", false, ",", 2, "first").unwrap(),
            "0.30000000000000004\n0.31\n"
        );
    }

    #[test]
    fn keep_last_preserves_order() {
        let d = "1,a\n2,b\n1.0,c\n3,d";
        // dedupe on col 1 numeric → the 1-row's last occurrence kept, in place
        assert_eq!(
            dedupe_numeric(d, "1", false, ",", -1, "last").unwrap(),
            "2,b\n1.0,c\n3,d\n"
        );
    }

    #[test]
    fn non_numeric_cells_fall_back_to_string() {
        let d = "apple,1\napple,1.0\nbanana,2";
        assert_eq!(
            dedupe_numeric(d, "", false, ",", -1, "first").unwrap(),
            "apple,1\nbanana,2\n"
        );
    }

    #[test]
    fn tab_delimiter() {
        let d = "1\t2\n1.0\t2\n3\t4";
        assert_eq!(
            dedupe_numeric(d, "", false, "tab", -1, "first").unwrap(),
            "1\t2\n3\t4\n"
        );
    }

    #[test]
    fn errors() {
        assert!(dedupe_numeric("  ", "", false, ",", -1, "first").is_err()); // empty
        assert!(dedupe_numeric("1,2\n3,4", "nope", true, ",", -1, "first").is_err()); // bad name
        assert!(dedupe_numeric("1,2\n3,4", "0", false, ",", -1, "first").is_err()); // 0 index
        assert!(dedupe_numeric("1,2", "name", false, ",", -1, "first").is_err()); // name w/o header
        assert!(dedupe_numeric("1,2", "", false, ",", 99, "first").is_err()); // precision range
        assert!(dedupe_numeric("1,2", "", false, ",", -1, "middle").is_err()); // bad keep
    }
}
