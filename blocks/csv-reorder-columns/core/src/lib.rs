//! gizza-ai/csv-reorder-columns core — reorder, swap, or drop CSV columns to match
//! a given target order, addressed by column name or 1-based index. Pure-Rust
//! (`csv`). No wafer/wasm-bindgen deps.

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

/// Reorder/drop columns of `data` to match `columns` (a comma-separated list of
/// column names — when `has_header` — or 1-based indices). Columns not listed are
/// dropped; a column may be repeated to duplicate it.
pub fn reorder(data: &str, columns: &str, has_header: bool, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let targets: Vec<String> = columns
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if targets.is_empty() {
        return Err("provide a target column order (names or 1-based indices), comma-separated".into());
    }

    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> =
        rdr.records().collect::<Result<_, _>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let header: Option<&csv::StringRecord> = if has_header { records.first() } else { None };

    // Resolve each target to a 0-based source column index.
    let mut indices: Vec<usize> = Vec::with_capacity(targets.len());
    for t in &targets {
        let by_name = header.and_then(|h| h.iter().position(|c| c == t));
        let idx = match by_name {
            Some(i) => i,
            None => {
                let n: usize = t.parse().map_err(|_| {
                    if has_header {
                        format!("column '{t}' not found in the header (and is not a number)")
                    } else {
                        format!("column index '{t}' must be a number (no header)")
                    }
                })?;
                if n == 0 || n > width {
                    return Err(format!("column index {n} is out of range (1..={width})"));
                }
                n - 1
            }
        };
        indices.push(idx);
    }

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).flexible(true).from_writer(vec![]);
    for rec in &records {
        let out: Vec<&str> = indices.iter().map(|&i| rec.get(i).unwrap_or("")).collect();
        wtr.write_record(&out).map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorder_by_name() {
        let d = "a,b,c\n1,2,3\n4,5,6";
        let out = reorder(d, "c,a", true, ",").unwrap();
        assert_eq!(out, "c,a\n3,1\n6,4\n");
    }

    #[test]
    fn drop_columns() {
        let d = "name,age,city\nAda,36,London";
        // keep only name + city
        let out = reorder(d, "name,city", true, ",").unwrap();
        assert_eq!(out, "name,city\nAda,London\n");
    }

    #[test]
    fn by_index_no_header() {
        let d = "1,2,3\n4,5,6";
        let out = reorder(d, "3,1", false, ",").unwrap();
        assert_eq!(out, "3,1\n6,4\n");
    }

    #[test]
    fn duplicate_column() {
        let d = "a,b\n1,2";
        let out = reorder(d, "a,a,b", true, ",").unwrap();
        assert_eq!(out, "a,a,b\n1,1,2\n");
    }

    #[test]
    fn tab_delimiter() {
        let d = "a\tb\n1\t2";
        let out = reorder(d, "b,a", true, "tab").unwrap();
        assert_eq!(out, "b\ta\n2\t1\n");
    }

    #[test]
    fn errors() {
        assert!(reorder("", "a", true, ",").is_err());
        assert!(reorder("a,b\n1,2", "", true, ",").is_err());
        assert!(reorder("a,b\n1,2", "zzz", true, ",").is_err()); // missing name
        assert!(reorder("1,2\n3,4", "9", false, ",").is_err()); // index out of range
    }
}
