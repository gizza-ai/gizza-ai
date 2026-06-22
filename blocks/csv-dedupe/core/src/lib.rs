//! gizza-ai/csv-dedupe core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Removes duplicate rows from CSV,
//! keeping the first occurrence. Optionally keys on a subset of columns (by
//! 1-based index or, when there's a header, by name).

use std::collections::HashSet;

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 { b[0] } else { return Err(format!("delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'")); }
        }
    })
}

/// Resolve `columns_csv` (1-based indices and/or header names) to 0-based column
/// indices. Empty → None (key on the whole row).
fn resolve_columns(columns_csv: &str, header: Option<&csv::StringRecord>) -> Result<Option<Vec<usize>>, String> {
    let toks: Vec<&str> = columns_csv.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
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
            return Err(format!("column '{t}' is not a number and there is no header to match names"));
        }
    }
    Ok(Some(idxs))
}

/// Remove duplicate rows (first kept). `has_header` preserves + ignores row 1 for
/// dedup. `columns` keys on a subset; empty keys on the full row.
pub fn dedupe(data: &str, columns: &str, has_header: bool, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).has_headers(false).flexible(true).from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr.records().collect::<Result<_, _>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Ok(String::new());
    }

    let header = if has_header { records.first() } else { None };
    let key_cols = resolve_columns(columns, header)?;

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).flexible(true).from_writer(vec![]);
    let mut seen: HashSet<String> = HashSet::new();
    for (i, rec) in records.iter().enumerate() {
        if has_header && i == 0 {
            wtr.write_record(rec).map_err(|e| format!("CSV write error: {e}"))?;
            continue;
        }
        let key = match &key_cols {
            Some(cols) => cols.iter().map(|&c| rec.get(c).unwrap_or("")).collect::<Vec<_>>().join("\u{1}"),
            None => rec.iter().collect::<Vec<_>>().join("\u{1}"),
        };
        if seen.insert(key) {
            wtr.write_record(rec).map_err(|e| format!("CSV write error: {e}"))?;
        }
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_full_row_duplicates_keeps_header() {
        let d = "name,age\nAlice,30\nBob,25\nAlice,30\nBob,25\nCarol,40";
        assert_eq!(dedupe(d, "", true, ",").unwrap(), "name,age\nAlice,30\nBob,25\nCarol,40\n");
    }

    #[test]
    fn keyed_on_column_name() {
        // dedupe on 'name' only → keep first row per name
        let d = "name,age\nAlice,30\nAlice,31\nBob,25";
        assert_eq!(dedupe(d, "name", true, ",").unwrap(), "name,age\nAlice,30\nBob,25\n");
    }

    #[test]
    fn keyed_on_index_no_header() {
        // 1-based index 1, no header → dedupe on first column
        let d = "a,1\na,2\nb,3";
        assert_eq!(dedupe(d, "1", false, ",").unwrap(), "a,1\nb,3\n");
    }

    #[test]
    fn no_header_full_row() {
        let d = "x,y\nx,y\nz,w";
        assert_eq!(dedupe(d, "", false, ",").unwrap(), "x,y\nz,w\n");
    }

    #[test]
    fn errors() {
        assert!(dedupe("  ", "", true, ",").is_err());
        assert!(dedupe("a,b\n1,2", "nope", true, ",").is_err());   // unknown column name
        assert!(dedupe("a,b\n1,2", "0", true, ",").is_err());      // 0 index
        assert!(dedupe("a,b\n1,2", "name", false, ",").is_err());  // name w/o header
    }
}
