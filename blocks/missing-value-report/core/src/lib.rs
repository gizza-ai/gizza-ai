//! gizza-ai/missing-value-report core — for a CSV/table with a header row, report
//! the number and percentage of missing (blank or NA-token) cells in every column,
//! plus a missingness-pattern grid (which columns are missing together across rows,
//! the R `mice::md.pattern` idiom). Pure-Rust; the only dependency is `csv` for
//! RFC-4180-correct quoted-field parsing/writing.

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

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Missing,
    Column,
    Name,
}

fn parse_sort(s: &str) -> Result<Sort, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "missing" => Ok(Sort::Missing),
        "column" => Ok(Sort::Column),
        "name" => Ok(Sort::Name),
        other => Err(format!(
            "sort must be 'missing', 'column', or 'name', got '{other}'"
        )),
    }
}

/// Parse the comma-separated NA-token list into a lowercased, trimmed, non-empty set.
fn parse_na_tokens(na_values: &str) -> Vec<String> {
    na_values
        .split(',')
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

/// A cell is missing when it is absent (short row), blank/whitespace-only, or equals
/// one of the NA tokens (case-insensitive after trimming).
fn is_missing(cell: Option<&str>, na: &[String]) -> bool {
    match cell {
        None => true,
        Some(c) => {
            let t = c.trim();
            t.is_empty() || na.iter().any(|n| n == &t.to_lowercase())
        }
    }
}

/// Format a percentage to at most 2 decimals, trimming trailing zeros, with a '%'.
fn fmt_pct(p: f64) -> String {
    let s = format!("{p:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s}%")
}

/// Build a per-column missing-value report plus (optionally) a missingness-pattern
/// grid for the CSV `data` (a header row is required).
///
/// - `delimiter`: field separator (single char or 'comma'/'tab'/'semicolon'/'pipe').
/// - `na_values`: extra comma-separated tokens counted as missing besides blank cells,
///   matched case-insensitively (e.g. "NA,N/A,null,NaN,None,#N/A"). Empty = blanks only.
/// - `sort`: column-table order — `"missing"` (default, most-missing-first),
///   `"column"` (original header order), or `"name"` (alphabetical).
/// - `include_patterns`: when true, append the present(1)/missing(0) pattern grid.
/// - `max_patterns`: cap on the number of pattern rows shown (>= 1).
///
/// Returns a text report: a `column,missing,present,total,missing_percent` CSV, then a
/// total-rows / complete-rows summary, then (optionally) a `count,<col>,...` pattern grid.
pub fn run(
    data: &str,
    delimiter: &str,
    na_values: &str,
    sort: &str,
    include_patterns: bool,
    max_patterns: i64,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if max_patterns < 1 {
        return Err("max_patterns must be at least 1".into());
    }
    let sort = parse_sort(sort)?;
    let delim = delim_byte(delimiter)?;
    let na = parse_na_tokens(na_values);

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
    let headers: Vec<String> = header.iter().map(|c| c.to_string()).collect();
    let ncols = headers.len();
    if ncols == 0 {
        return Err("the header row has no columns".into());
    }

    let rows = &records[1..];
    let total_rows = rows.len();
    if total_rows == 0 {
        return Err("no data rows found (only a header row)".into());
    }

    // Per-column missing counts + per-row present/missing pattern.
    let mut missing_per_col = vec![0u64; ncols];
    let mut patterns: Vec<Vec<bool>> = Vec::with_capacity(total_rows);
    for rec in rows {
        let mut present = Vec::with_capacity(ncols);
        for (i, m) in missing_per_col.iter_mut().enumerate() {
            let is_miss = is_missing(rec.get(i), &na);
            if is_miss {
                *m += 1;
            }
            present.push(!is_miss);
        }
        patterns.push(present);
    }

    // Section 1: per-column table, ordered by `sort` (stable — ties keep header order).
    let mut idx: Vec<usize> = (0..ncols).collect();
    match sort {
        Sort::Missing => idx.sort_by(|&a, &b| missing_per_col[b].cmp(&missing_per_col[a])),
        Sort::Column => {}
        Sort::Name => idx.sort_by(|&a, &b| headers[a].cmp(&headers[b])),
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .from_writer(vec![]);
    wtr.write_record(["column", "missing", "present", "total", "missing_percent"])
        .map_err(|e| format!("CSV write error: {e}"))?;
    for &i in &idx {
        let miss = missing_per_col[i];
        let present = total_rows as u64 - miss;
        let pct = miss as f64 * 100.0 / total_rows as f64;
        wtr.write_record([
            headers[i].as_str(),
            &miss.to_string(),
            &present.to_string(),
            &total_rows.to_string(),
            &fmt_pct(pct),
        ])
        .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let table = String::from_utf8(
        wtr.into_inner()
            .map_err(|e| format!("CSV write error: {e}"))?,
    )
    .map_err(|e| format!("utf8 error: {e}"))?;

    let complete = patterns.iter().filter(|p| p.iter().all(|&x| x)).count();
    let complete_pct = complete as f64 * 100.0 / total_rows as f64;

    let mut out = String::new();
    out.push_str(&table);
    out.push_str(&format!("Total rows: {total_rows}\n"));
    out.push_str(&format!(
        "Complete rows (no missing): {complete} ({})\n",
        fmt_pct(complete_pct)
    ));

    if include_patterns {
        // Aggregate identical patterns, preserving first-seen order for stable ties.
        let mut order: Vec<Vec<bool>> = Vec::new();
        let mut counts: HashMap<Vec<bool>, u64> = HashMap::new();
        for p in &patterns {
            let e = counts.entry(p.clone()).or_insert_with(|| {
                order.push(p.clone());
                0
            });
            *e += 1;
        }
        let mut agg: Vec<(Vec<bool>, u64)> =
            order.iter().map(|p| (p.clone(), counts[p])).collect();
        agg.sort_by(|a, b| b.1.cmp(&a.1)); // stable: ties keep first-seen order
        let total_patterns = agg.len();
        let shown = (max_patterns as usize).min(total_patterns);

        out.push('\n');
        out.push_str("Missingness patterns (present=1, missing=0):\n");
        let mut pw = csv::WriterBuilder::new()
            .delimiter(delim)
            .from_writer(vec![]);
        let mut head: Vec<String> = Vec::with_capacity(ncols + 1);
        head.push("count".to_string());
        head.extend(headers.iter().cloned());
        pw.write_record(&head)
            .map_err(|e| format!("CSV write error: {e}"))?;
        for (pat, cnt) in agg.iter().take(shown) {
            let mut row: Vec<String> = Vec::with_capacity(ncols + 1);
            row.push(cnt.to_string());
            for &present in pat {
                row.push(if present { "1" } else { "0" }.to_string());
            }
            pw.write_record(&row)
                .map_err(|e| format!("CSV write error: {e}"))?;
        }
        let grid = String::from_utf8(
            pw.into_inner()
                .map_err(|e| format!("CSV write error: {e}"))?,
        )
        .map_err(|e| format!("utf8 error: {e}"))?;
        out.push_str(&grid);
        if shown < total_patterns {
            out.push_str(&format!(
                "({} more pattern(s) not shown; raise max_patterns)\n",
                total_patterns - shown
            ));
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_NA: &str = "NA,N/A,null,NaN,None,#N/A";

    #[test]
    fn counts_percentages_and_patterns() {
        let d = "name,age,city\nAlice,30,NYC\nBob,,LA\n,25,\nCarol,40,NYC";
        let out = run(d, ",", DEFAULT_NA, "missing", true, 10).unwrap();
        let expected = "\
column,missing,present,total,missing_percent
name,1,3,4,25%
age,1,3,4,25%
city,1,3,4,25%
Total rows: 4
Complete rows (no missing): 2 (50%)

Missingness patterns (present=1, missing=0):
count,name,age,city
2,1,1,1
1,1,0,1
1,0,1,0
";
        assert_eq!(out, expected);
    }

    #[test]
    fn na_tokens_matched_case_insensitively() {
        // NA / n/a / NaN all count as missing besides the blank cell
        let d = "x\nok\nNA\nn/a\nNaN\n ";
        let out = run(d, ",", DEFAULT_NA, "missing", false, 10).unwrap();
        assert_eq!(
            out,
            "column,missing,present,total,missing_percent\nx,4,1,5,80%\nTotal rows: 5\nComplete rows (no missing): 1 (20%)\n"
        );
    }

    #[test]
    fn sort_by_column_keeps_header_order() {
        let d = "a,b,c\n,,x\n,y,z";
        let out = run(d, ",", "", "column", false, 10).unwrap();
        assert_eq!(
            out,
            "column,missing,present,total,missing_percent\na,2,0,2,100%\nb,1,1,2,50%\nc,0,2,2,0%\nTotal rows: 2\nComplete rows (no missing): 0 (0%)\n"
        );
    }

    #[test]
    fn sort_by_name_is_alphabetical() {
        let d = "zeta,alpha\n,\nv,";
        let out = run(d, ",", "", "name", false, 10).unwrap();
        assert_eq!(
            out,
            "column,missing,present,total,missing_percent\nalpha,2,0,2,100%\nzeta,1,1,2,50%\nTotal rows: 2\nComplete rows (no missing): 0 (0%)\n"
        );
    }

    #[test]
    fn tab_delimited_and_short_rows_are_missing() {
        // second row is short (only 1 field) → col 2 counts as missing
        let d = "id\tval\n1\t9\n2";
        let out = run(d, "tab", "", "column", false, 10).unwrap();
        assert_eq!(
            out,
            "column\tmissing\tpresent\ttotal\tmissing_percent\nid\t0\t2\t2\t0%\nval\t1\t1\t2\t50%\nTotal rows: 2\nComplete rows (no missing): 1 (50%)\n"
        );
    }

    #[test]
    fn max_patterns_caps_and_notes_remainder() {
        // 3 distinct patterns, cap at 1 → 2 omitted
        let d = "a,b\nx,y\n,y\nx,";
        let out = run(d, ",", "", "column", true, 1).unwrap();
        assert!(out.contains("count,a,b\n1,1,1\n"));
        assert!(out.contains("(2 more pattern(s) not shown; raise max_patterns)"));
    }

    #[test]
    fn empty_na_list_only_blanks_missing() {
        // "NA" is a real value when na_values is empty
        let d = "x\nNA\n \nok";
        let out = run(d, ",", "", "missing", false, 10).unwrap();
        assert_eq!(
            out,
            "column,missing,present,total,missing_percent\nx,1,2,3,33.33%\nTotal rows: 3\nComplete rows (no missing): 2 (66.67%)\n"
        );
    }

    #[test]
    fn errors() {
        assert!(run("   ", ",", "", "missing", true, 10).is_err());
        // header only, no data rows
        assert!(run("a,b,c", ",", "", "missing", true, 10).is_err());
        // bad sort
        assert!(run("x\n1", ",", "", "bogus", true, 10).is_err());
        // bad delimiter
        assert!(run("x\n1", "nope", "", "missing", true, 10).is_err());
        // max_patterns < 1
        assert!(run("x\n1", ",", "", "missing", true, 0).is_err());
    }
}
