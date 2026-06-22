//! gizza-ai/csv-stats core — per-column summary statistics for a CSV (count,
//! distinct, and for numeric columns min/max/mean/sum). Pure-Rust (`csv`). No
//! wafer/wasm-bindgen deps.

use std::collections::HashSet;

use serde::Serialize;

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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColumnStats {
    pub column: String,
    /// "numeric" if every non-empty value parses as a number, else "text".
    pub kind: String,
    /// Non-empty value count.
    pub count: usize,
    /// Empty/blank value count.
    pub empty: usize,
    /// Number of distinct non-empty values.
    pub distinct: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sum: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    pub columns: Vec<ColumnStats>,
    pub rows: usize,
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Compute per-column statistics for `data`.
pub fn stats(data: &str, has_header: bool, delimiter: &str) -> Result<Stats, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
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

    let (names, data_rows): (Vec<String>, &[csv::StringRecord]) = if has_header {
        let h = &records[0];
        let mut names: Vec<String> = (0..width).map(|i| h.get(i).unwrap_or("").to_string()).collect();
        for (i, n) in names.iter_mut().enumerate() {
            if n.is_empty() {
                *n = format!("col{}", i + 1);
            }
        }
        (names, &records[1..])
    } else {
        ((0..width).map(|i| format!("col{}", i + 1)).collect(), &records[..])
    };

    let mut cols: Vec<ColumnStats> = Vec::with_capacity(width);
    for (ci, name) in names.iter().enumerate() {
        let mut count = 0usize;
        let mut empty = 0usize;
        let mut distinct: HashSet<String> = HashSet::new();
        let mut numeric = true;
        let mut sum = 0.0f64;
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for rec in data_rows {
            let v = rec.get(ci).unwrap_or("").trim();
            if v.is_empty() {
                empty += 1;
                continue;
            }
            count += 1;
            distinct.insert(v.to_string());
            if numeric {
                match v.parse::<f64>() {
                    Ok(n) if n.is_finite() => {
                        sum += n;
                        min = min.min(n);
                        max = max.max(n);
                    }
                    _ => numeric = false,
                }
            }
        }
        let is_num = numeric && count > 0;
        cols.push(ColumnStats {
            column: name.clone(),
            kind: if is_num { "numeric".into() } else { "text".into() },
            count,
            empty,
            distinct: distinct.len(),
            min: is_num.then_some(round3(min)),
            max: is_num.then_some(round3(max)),
            mean: is_num.then(|| round3(sum / count as f64)),
            sum: is_num.then_some(round3(sum)),
        });
    }

    Ok(Stats { columns: cols, rows: data_rows.len() })
}

/// Plain-text summary table (used by the page).
pub fn summary(data: &str, has_header: bool, delimiter: &str) -> Result<String, String> {
    let s = stats(data, has_header, delimiter)?;
    let mut out = format!("{} data row(s)\n", s.rows);
    for c in &s.columns {
        if c.kind == "numeric" {
            out.push_str(&format!(
                "{}: numeric | count {} | distinct {} | min {} | max {} | mean {} | sum {}\n",
                c.column,
                c.count,
                c.distinct,
                c.min.unwrap(),
                c.max.unwrap(),
                c.mean.unwrap(),
                c.sum.unwrap()
            ));
        } else {
            out.push_str(&format!(
                "{}: text | count {} | distinct {} | empty {}\n",
                c.column, c.count, c.distinct, c.empty
            ));
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_and_text_columns() {
        let d = "name,age\nAda,36\nBo,40\nCy,36";
        let s = stats(d, true, ",").unwrap();
        assert_eq!(s.rows, 3);
        let name = &s.columns[0];
        assert_eq!(name.kind, "text");
        assert_eq!(name.count, 3);
        assert_eq!(name.distinct, 3);
        let age = &s.columns[1];
        assert_eq!(age.kind, "numeric");
        assert_eq!(age.min, Some(36.0));
        assert_eq!(age.max, Some(40.0));
        assert_eq!(age.mean, Some(round3(112.0 / 3.0)));
        assert_eq!(age.sum, Some(112.0));
        assert_eq!(age.distinct, 2); // 36, 40
    }

    #[test]
    fn empties_counted_not_in_distinct() {
        // Empty *cell* (not a blank line, which the CSV reader drops).
        let d = "a,b\n1,x\n,y\n2,z";
        let s = stats(d, true, ",").unwrap();
        let a = &s.columns[0];
        assert_eq!(a.count, 2);
        assert_eq!(a.empty, 1);
        assert_eq!(a.distinct, 2);
        assert_eq!(a.sum, Some(3.0));
    }

    #[test]
    fn no_header_uses_col_names() {
        let d = "1,2\n3,4";
        let s = stats(d, false, ",").unwrap();
        assert_eq!(s.rows, 2);
        assert_eq!(s.columns[0].column, "col1");
        assert_eq!(s.columns[1].mean, Some(3.0));
    }

    #[test]
    fn mixed_column_is_text() {
        let d = "x\n1\nabc\n3";
        let s = stats(d, true, ",").unwrap();
        assert_eq!(s.columns[0].kind, "text");
        assert!(s.columns[0].sum.is_none());
    }

    #[test]
    fn summary_format() {
        let out = summary("n\n1\n2\n3", true, ",").unwrap();
        assert!(out.contains("3 data row(s)"));
        assert!(out.contains("mean 2"));
    }

    #[test]
    fn errors() {
        assert!(stats("", true, ",").is_err());
        assert!(stats("a,b\n1,2", true, "xx").is_err()); // bad multi-char delimiter
    }
}
