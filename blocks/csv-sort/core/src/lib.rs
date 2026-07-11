//! gizza-ai/csv-sort core — sort CSV/TSV rows by one or more columns, each
//! ascending or descending, with numeric-aware or lexical ordering. Pure-Rust
//! (`csv`). No wafer/wasm-bindgen deps.

use std::cmp::Ordering;

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

/// One resolved sort key: 0-based source column + whether it descends.
struct SortKey {
    col: usize,
    desc: bool,
}

fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok()
}

fn parse_dir(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "asc" | "ascending" | "a" => Some(false),
        "desc" | "descending" | "d" => Some(true),
        _ => None,
    }
}

/// Compare two cells within one column. Numbers sort before blanks/non-numbers
/// in ascending order; ties fall through to a (optionally case-insensitive)
/// string compare.
fn cmp_cells(a: &str, b: &str, numeric: bool, case_sensitive: bool) -> Ordering {
    if numeric {
        match (parse_num(a), parse_num(b)) {
            (Some(x), Some(y)) => return x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            (None, None) => {}
        }
    }
    if case_sensitive {
        a.cmp(b)
    } else {
        a.to_lowercase().cmp(&b.to_lowercase())
    }
}

/// Resolve a column name (header) or 1-based index to a 0-based source index.
fn resolve_col(
    name: &str,
    header: Option<&csv::StringRecord>,
    has_header: bool,
    width: usize,
) -> Result<usize, String> {
    if let Some(i) = header.and_then(|h| h.iter().position(|c| c == name)) {
        return Ok(i);
    }
    let n: usize = name.parse().map_err(|_| {
        if has_header {
            format!("column '{name}' not found in the header (and is not a number)")
        } else {
            format!("column index '{name}' must be a number (no header)")
        }
    })?;
    if n == 0 || n > width {
        return Err(format!("column index {n} is out of range (1..={width})"));
    }
    Ok(n - 1)
}

/// Sort the rows of `data` by `columns` (comma-separated column names — when
/// `has_header` — or 1-based indices; each may carry a `:asc`/`:desc` suffix to
/// override the global `order`). `numeric` is `auto` (detect per column), `number`
/// (force numeric), or `text` (force lexical). The header row, if any, stays on top.
#[allow(clippy::too_many_arguments)]
pub fn sort(
    data: &str,
    columns: &str,
    order: &str,
    numeric: &str,
    case_sensitive: bool,
    has_header: bool,
    delimiter: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }

    let global_desc = match order.trim() {
        "" => false,
        o => parse_dir(o).ok_or_else(|| format!("order must be 'asc' or 'desc', got '{o}'"))?,
    };
    let numeric_mode = match numeric.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => 0u8,
        "number" | "numeric" | "num" => 1,
        "text" | "lexical" | "string" => 2,
        other => return Err(format!("numeric must be 'auto', 'number', or 'text', got '{other}'")),
    };

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

    // Resolve each "col[:dir]" target into a SortKey. A `:asc`/`:desc` suffix
    // overrides the global order; any other trailing `:x` is treated as part of
    // the column name (so a column literally named "a:b" still works).
    let raw: Vec<&str> = columns.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if raw.is_empty() {
        return Err("provide at least one sort column (name or 1-based index)".into());
    }
    let mut keys: Vec<SortKey> = Vec::with_capacity(raw.len());
    for t in &raw {
        let (name, desc) = match t.rsplit_once(':') {
            Some((n, dir)) if parse_dir(dir).is_some() => (n.trim(), parse_dir(dir).unwrap()),
            _ => (*t, global_desc),
        };
        let col = resolve_col(name, header, has_header, width)?;
        keys.push(SortKey { col, desc });
    }

    // Per-key numeric decision (auto = column is all-numeric over the data rows).
    let data_start = if has_header { 1 } else { 0 };
    let numeric_key: Vec<bool> = keys
        .iter()
        .map(|k| match numeric_mode {
            1 => true,
            2 => false,
            _ => {
                let mut any = false;
                for rec in &records[data_start..] {
                    let cell = rec.get(k.col).unwrap_or("");
                    if cell.trim().is_empty() {
                        continue;
                    }
                    any = true;
                    if parse_num(cell).is_none() {
                        return false;
                    }
                }
                any
            }
        })
        .collect();

    let mut body: Vec<csv::StringRecord> = records[data_start..].to_vec();
    body.sort_by(|a, b| {
        for (k, &num) in keys.iter().zip(numeric_key.iter()) {
            let av = a.get(k.col).unwrap_or("");
            let bv = b.get(k.col).unwrap_or("");
            let mut ord = cmp_cells(av, bv, num, case_sensitive);
            if k.desc {
                ord = ord.reverse();
            }
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    });

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).flexible(true).from_writer(vec![]);
    if let Some(h) = header {
        wtr.write_record(h).map_err(|e| format!("CSV write error: {e}"))?;
    }
    for rec in &body {
        wtr.write_record(rec).map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_by_name_asc() {
        let d = "name,age\nBob,30\nAda,36\nCy,4";
        let out = sort(d, "name", "asc", "auto", false, true, ",").unwrap();
        assert_eq!(out, "name,age\nAda,36\nBob,30\nCy,4\n");
    }

    #[test]
    fn numeric_auto_beats_lexical() {
        // Ages 4, 30, 36 must sort numerically (4 < 30), not lexically ("30" < "4").
        let d = "name,age\nBob,30\nAda,36\nCy,4";
        let out = sort(d, "age", "asc", "auto", false, true, ",").unwrap();
        assert_eq!(out, "name,age\nCy,4\nBob,30\nAda,36\n");
    }

    #[test]
    fn descending_order() {
        let d = "n\n1\n3\n2";
        let out = sort(d, "n", "desc", "number", false, true, ",").unwrap();
        assert_eq!(out, "n\n3\n2\n1\n");
    }

    #[test]
    fn multi_column_tiebreak_with_per_col_dir() {
        // Primary: dept asc; tie-breaker: salary desc.
        let d = "dept,salary\neng,100\nhr,50\neng,200\nhr,90";
        let out = sort(d, "dept:asc,salary:desc", "asc", "auto", false, true, ",").unwrap();
        assert_eq!(out, "dept,salary\neng,200\neng,100\nhr,90\nhr,50\n");
    }

    #[test]
    fn by_index_no_header() {
        let d = "3,x\n1,y\n2,z";
        let out = sort(d, "1", "asc", "number", false, false, ",").unwrap();
        assert_eq!(out, "1,y\n2,z\n3,x\n");
    }

    #[test]
    fn text_mode_forces_lexical() {
        let d = "v\n10\n2\n1";
        let out = sort(d, "v", "asc", "text", false, true, ",").unwrap();
        assert_eq!(out, "v\n1\n10\n2\n");
    }

    #[test]
    fn case_insensitive_by_default() {
        let d = "w\nbanana\nApple\ncherry";
        let out = sort(d, "w", "asc", "text", false, true, ",").unwrap();
        assert_eq!(out, "w\nApple\nbanana\ncherry\n");
    }

    #[test]
    fn case_sensitive_uppercase_first() {
        // Case-sensitive: ASCII 'A' (65) < 'b' (98) < 'c' (99).
        let d = "w\nbanana\nApple\ncherry";
        let out = sort(d, "w", "asc", "text", true, true, ",").unwrap();
        assert_eq!(out, "w\nApple\nbanana\ncherry\n");
    }

    #[test]
    fn tab_delimiter() {
        let d = "a\tb\n2\ty\n1\tx";
        let out = sort(d, "a", "asc", "number", false, true, "tab").unwrap();
        assert_eq!(out, "a\tb\n1\tx\n2\ty\n");
    }

    #[test]
    fn blanks_sort_after_numbers_ascending() {
        // A blank CELL (not an empty line — those are skipped) sorts after all
        // numbers in ascending numeric order.
        let d = "n,x\n5,a\n,b\n2,c";
        let out = sort(d, "n", "asc", "number", false, true, ",").unwrap();
        assert_eq!(out, "n,x\n2,c\n5,a\n,b\n");
    }

    #[test]
    fn stable_keeps_original_order_on_ties() {
        // Same dept — stable sort preserves first-seen order (Zoe before Amy).
        let d = "dept,name\neng,Zoe\neng,Amy\nhr,Bob";
        let out = sort(d, "dept", "asc", "text", false, true, ",").unwrap();
        assert_eq!(out, "dept,name\neng,Zoe\neng,Amy\nhr,Bob\n");
    }

    #[test]
    fn errors() {
        assert!(sort("", "a", "asc", "auto", false, true, ",").is_err()); // empty
        assert!(sort("a,b\n1,2", "", "asc", "auto", false, true, ",").is_err()); // no columns
        assert!(sort("a,b\n1,2", "zzz", "asc", "auto", false, true, ",").is_err()); // bad name
        assert!(sort("1,2\n3,4", "9", "asc", "auto", false, false, ",").is_err()); // index OOR
        assert!(sort("a\n1", "a", "sideways", "auto", false, true, ",").is_err()); // bad order
        assert!(sort("a\n1", "a", "asc", "roman", false, true, ",").is_err()); // bad numeric
    }
}
