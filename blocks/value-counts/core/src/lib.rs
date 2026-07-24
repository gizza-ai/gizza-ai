//! gizza-ai/value-counts core — count the distinct values in one chosen column of
//! a CSV/table, with each value's count and its percentage of the total, ranked
//! most-frequent-first (the pandas `Series.value_counts()` idiom). Pure-Rust; the
//! only dependency is `csv` for RFC-4180-correct quoted-field parsing/writing.

use std::collections::HashMap;

/// Resolve a single-char / named delimiter to its byte.
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

/// Resolve a column name or 1-based index to a 0-based column position.
fn resolve(name: &str, header: &csv::StringRecord) -> Result<usize, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("column is required (a header name or 1-based index)".into());
    }
    if let Ok(n) = name.parse::<usize>() {
        if n == 0 {
            return Err("column index is 1-based (>= 1)".into());
        }
        if n > header.len() {
            return Err(format!(
                "column index {n} is out of range (the header has {} columns)",
                header.len()
            ));
        }
        return Ok(n - 1);
    }
    header
        .iter()
        .position(|c| c == name)
        .ok_or_else(|| format!("column '{name}' not found in the header"))
}

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Count,
    Value,
}

fn parse_sort(s: &str) -> Result<Sort, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "count" => Ok(Sort::Count),
        "value" => Ok(Sort::Value),
        other => Err(format!("sort must be 'count' or 'value', got '{other}'")),
    }
}

/// Format a percentage to at most 2 decimals, trimming trailing zeros, with a '%'.
fn fmt_pct(p: f64) -> String {
    let s = format!("{p:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s}%")
}

/// Count the distinct values in `column` of the CSV `data` (a header row is
/// required). Returns a small CSV table with a `value,count,percent` header, one
/// row per distinct value. Percentages are of the total counted values.
///
/// - `sort`: `"count"` (default) ranks most-frequent-first (ties keep first-seen
///   order); `"value"` sorts by the value ascending.
/// - `case_sensitive` (default true): when false, values that differ only in case
///   are grouped together (the first-seen spelling is shown).
/// - `include_empty` (default false): when true, blank cells are counted as an
///   `(empty)` value instead of being skipped (pandas `dropna=False`).
pub fn value_counts(
    data: &str,
    column: &str,
    delimiter: &str,
    sort: &str,
    case_sensitive: bool,
    include_empty: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let sort = parse_sort(sort)?;
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
        return Err("no rows found".into());
    }
    let header = &records[0];
    let col = resolve(column, header)?;
    let col_name = header.get(col).unwrap_or("").to_string();

    // key -> (display value, count); `order` preserves first-seen order for stable ties.
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, (String, u64)> = HashMap::new();
    let mut total: u64 = 0;
    for rec in records.iter().skip(1) {
        let cell = rec.get(col).unwrap_or("");
        let is_empty = cell.trim().is_empty();
        if is_empty && !include_empty {
            continue;
        }
        let display = if is_empty {
            "(empty)".to_string()
        } else {
            cell.to_string()
        };
        let key = if case_sensitive {
            display.clone()
        } else {
            display.to_lowercase()
        };
        let e = map.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            (display.clone(), 0)
        });
        e.1 += 1;
        total += 1;
    }
    if total == 0 {
        return Err(format!("column '{col_name}' has no values to count"));
    }

    let mut entries: Vec<(String, u64)> = order.iter().map(|k| map[k].clone()).collect();
    match sort {
        // stable sort keeps first-seen order for equal counts
        Sort::Count => entries.sort_by(|a, b| b.1.cmp(&a.1)),
        Sort::Value => entries.sort_by(|a, b| a.0.cmp(&b.0)),
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .from_writer(vec![]);
    wtr.write_record(["value", "count", "percent"])
        .map_err(|e| format!("CSV write error: {e}"))?;
    for (display, count) in &entries {
        let pct = *count as f64 * 100.0 / total as f64;
        wtr.write_record([display.as_str(), &count.to_string(), &fmt_pct(pct)])
            .map_err(|e| format!("CSV write error: {e}"))?;
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
    fn counts_and_percentages_ranked() {
        let d = "fruit\napple\nbanana\napple\ncherry\napple\nbanana";
        let out = value_counts(d, "fruit", ",", "count", true, false).unwrap();
        assert_eq!(
            out,
            "value,count,percent\napple,3,50%\nbanana,2,33.33%\ncherry,1,16.67%\n"
        );
    }

    #[test]
    fn column_by_index_and_delimiter() {
        // pick the 2nd column via 1-based index, tab-delimited
        let d = "id\tcolor\n1\tred\n2\tblue\n3\tred";
        let out = value_counts(d, "2", "tab", "count", true, false).unwrap();
        assert_eq!(
            out,
            "value\tcount\tpercent\nred\t2\t66.67%\nblue\t1\t33.33%\n"
        );
    }

    #[test]
    fn sort_by_value_is_ascending() {
        let d = "c\nb\na\nb\na\na";
        let out = value_counts(d, "c", ",", "value", true, false).unwrap();
        assert_eq!(out, "value,count,percent\na,3,60%\nb,2,40%\n");
    }

    #[test]
    fn case_insensitive_groups_and_shows_first_seen() {
        let d = "x\nApple\napple\nAPPLE\nbanana";
        let out = value_counts(d, "x", ",", "count", false, false).unwrap();
        assert_eq!(out, "value,count,percent\nApple,3,75%\nbanana,1,25%\n");
    }

    #[test]
    fn include_empty_counts_blanks() {
        let d = "x\na\n,\nb\n \na";
        // without include_empty the two blank cells are skipped
        let out = value_counts(d, "x", ",", "count", true, false).unwrap();
        assert_eq!(out, "value,count,percent\na,2,66.67%\nb,1,33.33%\n");
        // with include_empty they are counted as (empty)
        let out2 = value_counts(d, "x", ",", "count", true, true).unwrap();
        assert_eq!(
            out2,
            "value,count,percent\na,2,40%\n(empty),2,40%\nb,1,20%\n"
        );
    }

    #[test]
    fn errors() {
        assert!(value_counts("  ", "x", ",", "count", true, false).is_err());
        assert!(value_counts("a,b\n1,2", "nope", ",", "count", true, false).is_err());
        assert!(value_counts("a,b\n1,2", "9", ",", "count", true, false).is_err());
        assert!(value_counts("a,b\n1,2", "0", ",", "count", true, false).is_err());
        assert!(value_counts("a,b\n1,2", "a", ",", "bogus", true, false).is_err());
        // a header-only column with no data rows has nothing to count
        assert!(value_counts("x", "x", ",", "count", true, false).is_err());
    }
}
