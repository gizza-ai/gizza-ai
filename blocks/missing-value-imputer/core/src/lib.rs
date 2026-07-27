//! missing-value-imputer core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Fills missing cells in a CSV using
//! mean, median, most-frequent, constant, or KNN (nan-euclidean) imputation.
//!
//! A cell counts as missing when, after trimming, it is empty OR equals one of the
//! caller-supplied `na_tokens` (e.g. `NA`, `?`, `null`). mean / median / knn operate
//! on NUMERIC columns only (a column whose every present value parses as a finite
//! number); a non-numeric cell is left unchanged under those strategies. most_frequent
//! and constant apply to any column.

use std::collections::HashMap;

/// Parse a delimiter spec: a single char, or a friendly name.
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
                    "delimiter must be a single char or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Parse a numeric cell, rejecting non-finite (`NaN`/`inf`) so those strings don't
/// silently mark a text column as numeric.
fn parse_num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Format an imputed numeric value: round to 6 decimals and print minimally
/// (`30.0` -> `30`, `27.333333` stays `27.333333`).
fn fmt_num(x: f64) -> String {
    let r = (x * 1e6).round() / 1e6;
    format!("{r}")
}

/// Fill missing cells in a CSV.
///
/// * `header` — treat the first row as a header (used for `columns` name lookup and kept verbatim).
/// * `delimiter` — the field separator (char or comma/tab/semicolon/pipe).
/// * `strategy` — `mean` | `median` | `most_frequent` | `constant` | `knn`.
/// * `columns` — comma-separated column NAMES (needs a header) or 1-based indices to impute;
///   blank imputes every applicable column.
/// * `na_tokens` — comma-separated extra strings that count as missing besides blank cells.
/// * `fill_value` — the value written for `strategy = constant`.
/// * `n_neighbors` — neighbours used by `knn` (clamped to ≥1).
/// * `weights` — `uniform` | `distance` for `knn`.
#[allow(clippy::too_many_arguments)]
pub fn impute(
    data: &str,
    header: bool,
    delimiter: &str,
    strategy: &str,
    columns: &str,
    na_tokens: &str,
    fill_value: &str,
    n_neighbors: usize,
    weights: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let strategy = strategy.trim();
    if !matches!(strategy, "mean" | "median" | "most_frequent" | "constant" | "knn") {
        return Err(format!(
            "strategy must be mean/median/most_frequent/constant/knn, got '{strategy}'"
        ));
    }
    if strategy == "knn" && !matches!(weights.trim(), "" | "uniform" | "distance") {
        return Err(format!("weights must be uniform or distance, got '{weights}'"));
    }
    let delim = delim_byte(delimiter)?;
    let k = n_neighbors.max(1);

    // Extra missing markers (trimmed, non-empty). Empty cells are always missing.
    let na_set: Vec<String> = na_tokens
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    let is_missing = |cell: &str| -> bool {
        let t = cell.trim();
        t.is_empty() || na_set.iter().any(|n| n == t)
    };

    // Parse all rows (flexible width).
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
    let ncols = records.iter().map(|r| r.len()).max().unwrap_or(0);

    // Rectangular grid: pad short rows with empty (missing) cells.
    let mut grid: Vec<Vec<String>> = records
        .iter()
        .map(|r| {
            let mut row: Vec<String> = r.iter().map(|s| s.to_string()).collect();
            row.resize(ncols, String::new());
            row
        })
        .collect();

    let header_names: Option<Vec<String>> = if header {
        Some(grid[0].iter().map(|s| s.trim().to_string()).collect())
    } else {
        None
    };
    let data_start = if header { 1 } else { 0 };
    if grid.len() <= data_start {
        // Header only (or empty) — nothing to impute; echo back.
        return write_csv(&grid, delim);
    }

    // Resolve the target columns to impute.
    let targets: Vec<usize> = if columns.trim().is_empty() {
        (0..ncols).collect()
    } else {
        let mut out = Vec::new();
        for tok in columns.split(',') {
            let tok = tok.trim();
            if tok.is_empty() {
                continue;
            }
            let idx = match &header_names {
                Some(names) if names.iter().any(|n| n == tok) => {
                    names.iter().position(|n| n == tok).unwrap()
                }
                _ => {
                    let n: usize = tok.parse().map_err(|_| {
                        format!("unknown column '{tok}' (use a header name or a 1-based index)")
                    })?;
                    if n == 0 || n > ncols {
                        return Err(format!("column index {n} out of range 1..={ncols}"));
                    }
                    n - 1
                }
            };
            if !out.contains(&idx) {
                out.push(idx);
            }
        }
        out
    };

    // Which columns are numeric (every present data cell parses as a finite number).
    let numeric: Vec<bool> = (0..ncols)
        .map(|c| {
            grid[data_start..]
                .iter()
                .all(|row| is_missing(&row[c]) || parse_num(&row[c]).is_some())
        })
        .collect();

    match strategy {
        "constant" => {
            for row in grid[data_start..].iter_mut() {
                for &c in &targets {
                    if is_missing(&row[c]) {
                        row[c] = fill_value.to_string();
                    }
                }
            }
        }
        "most_frequent" => {
            for &c in &targets {
                if let Some(mode) = column_mode(&grid, data_start, c, &is_missing) {
                    for row in grid[data_start..].iter_mut() {
                        if is_missing(&row[c]) {
                            row[c] = mode.clone();
                        }
                    }
                }
            }
        }
        "mean" | "median" => {
            for &c in &targets {
                if !numeric[c] {
                    continue; // mean/median need numbers; leave text cells as-is
                }
                let present: Vec<f64> = grid[data_start..]
                    .iter()
                    .filter(|row| !is_missing(&row[c]))
                    .filter_map(|row| parse_num(&row[c]))
                    .collect();
                if present.is_empty() {
                    continue;
                }
                let fill = if strategy == "mean" {
                    present.iter().sum::<f64>() / present.len() as f64
                } else {
                    median(&present)
                };
                let s = fmt_num(fill);
                for row in grid[data_start..].iter_mut() {
                    if is_missing(&row[c]) {
                        row[c] = s.clone();
                    }
                }
            }
        }
        "knn" => {
            let dist_weight = weights.trim() == "distance";
            knn_impute(&mut grid, data_start, &targets, &numeric, k, dist_weight, &is_missing);
        }
        _ => unreachable!(),
    }

    write_csv(&grid, delim)
}

fn median(vals: &[f64]) -> f64 {
    let mut v = vals.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// Most frequent non-missing value in a column; ties broken by first appearance.
fn column_mode(
    grid: &[Vec<String>],
    data_start: usize,
    c: usize,
    is_missing: &impl Fn(&str) -> bool,
) -> Option<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for row in &grid[data_start..] {
        let cell = row[c].trim();
        if is_missing(&row[c]) {
            continue;
        }
        let key = cell.to_string();
        if !counts.contains_key(&key) {
            order.push(key.clone());
        }
        *counts.entry(key).or_insert(0) += 1;
    }
    order
        .into_iter()
        .max_by_key(|v| counts[v])
        .filter(|v| counts[v] > 0)
}

/// KNN (nan-euclidean) imputation of numeric target columns.
fn knn_impute(
    grid: &mut [Vec<String>],
    data_start: usize,
    targets: &[usize],
    numeric: &[bool],
    k: usize,
    dist_weight: bool,
    is_missing: &impl Fn(&str) -> bool,
) {
    let ncols = numeric.len();
    let nrows = grid.len() - data_start;
    // Numeric feature columns used for the distance metric.
    let feats: Vec<usize> = (0..ncols).filter(|&c| numeric[c]).collect();
    let nfeat = feats.len();

    // Snapshot numeric values (None = missing) BEFORE imputing, so imputations
    // don't feed into each other's neighbour search.
    let vals: Vec<Vec<Option<f64>>> = grid[data_start..]
        .iter()
        .map(|row| {
            (0..ncols)
                .map(|c| {
                    if numeric[c] && !is_missing(&row[c]) {
                        parse_num(&row[c])
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();

    for &c in targets {
        if !numeric[c] {
            continue;
        }
        // Column mean fallback for rows with no usable neighbour.
        let present: Vec<f64> = vals.iter().filter_map(|r| r[c]).collect();
        let col_mean = if present.is_empty() {
            None
        } else {
            Some(present.iter().sum::<f64>() / present.len() as f64)
        };

        for i in 0..nrows {
            if vals[i][c].is_some() || !is_missing(&grid[data_start + i][c]) {
                continue; // present already
            }
            // Distances to every donor row that HAS a value in column c.
            let mut cand: Vec<(f64, f64)> = Vec::new(); // (distance, donor value)
            for j in 0..nrows {
                if j == i {
                    continue;
                }
                let dv = match vals[j][c] {
                    Some(v) => v,
                    None => continue,
                };
                let mut sumsq = 0.0;
                let mut shared = 0usize;
                for &f in &feats {
                    if let (Some(a), Some(b)) = (vals[i][f], vals[j][f]) {
                        let d = a - b;
                        sumsq += d * d;
                        shared += 1;
                    }
                }
                if shared == 0 {
                    continue;
                }
                let dist = (sumsq * (nfeat as f64 / shared as f64)).sqrt();
                cand.push((dist, dv));
            }
            let fill = if cand.is_empty() {
                col_mean
            } else {
                cand.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                cand.truncate(k);
                Some(weighted_mean(&cand, dist_weight))
            };
            if let Some(v) = fill {
                grid[data_start + i][c] = fmt_num(v);
            }
        }
    }
}

/// Average the chosen neighbours' values, uniform or inverse-distance weighted.
/// Zero-distance neighbours (exact matches) take all the weight when present.
fn weighted_mean(cand: &[(f64, f64)], dist_weight: bool) -> f64 {
    if !dist_weight {
        return cand.iter().map(|(_, v)| v).sum::<f64>() / cand.len() as f64;
    }
    let zeros: Vec<f64> = cand.iter().filter(|(d, _)| *d == 0.0).map(|(_, v)| *v).collect();
    if !zeros.is_empty() {
        return zeros.iter().sum::<f64>() / zeros.len() as f64;
    }
    let mut num = 0.0;
    let mut den = 0.0;
    for (d, v) in cand {
        let w = 1.0 / d;
        num += w * v;
        den += w;
    }
    num / den
}

fn write_csv(grid: &[Vec<String>], delim: u8) -> Result<String, String> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .terminator(csv::Terminator::Any(b'\n'))
        .flexible(true)
        .from_writer(vec![]);
    for row in grid {
        wtr.write_record(row).map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imp(data: &str, strategy: &str) -> Result<String, String> {
        impute(data, true, ",", strategy, "", "", "", 5, "uniform")
    }

    #[test]
    fn mean_fills_numeric_column() {
        // age column: 30, (missing), 40 -> mean 35 fills the blank.
        let d = "name,age\nAlice,30\nBob,\nCarol,40";
        assert_eq!(imp(d, "mean").unwrap(), "name,age\nAlice,30\nBob,35\nCarol,40\n");
    }

    #[test]
    fn median_fills_numeric_column() {
        // v values 10, 20, 30, (missing) -> median of {10,20,30} = 20.
        let d = "id,v\n1,10\n2,20\n3,30\n4,";
        assert_eq!(imp(d, "median").unwrap(), "id,v\n1,10\n2,20\n3,30\n4,20\n");
    }

    #[test]
    fn most_frequent_fills_text_column() {
        let d = "color,n\nred,1\nred,2\n,3\nblue,4";
        assert_eq!(
            imp(d, "most_frequent").unwrap(),
            "color,n\nred,1\nred,2\nred,3\nblue,4\n"
        );
    }

    #[test]
    fn constant_fills_with_value() {
        let d = "a,b\n1,\n,2";
        let got = impute(d, true, ",", "constant", "", "", "0", 5, "uniform").unwrap();
        assert_eq!(got, "a,b\n1,0\n0,2\n");
    }

    #[test]
    fn na_tokens_and_selected_columns() {
        // Only impute column "b"; treat NA as missing. a stays untouched.
        let d = "a,b\n1,NA\nNA,4";
        let got = impute(d, true, ",", "mean", "b", "NA", "", 5, "uniform").unwrap();
        assert_eq!(got, "a,b\n1,4\nNA,4\n");
    }

    #[test]
    fn knn_fills_from_nearest_neighbours() {
        // Row 2 (x=1) is missing y; nearest by x is row 1 (x=1,y=10) -> 10.
        let d = "x,y\n1,10\n1,\n9,90\n10,100";
        let got = impute(d, true, ",", "knn", "y", "", "", 1, "uniform").unwrap();
        assert_eq!(got, "x,y\n1,10\n1,10\n9,90\n10,100\n");
    }

    #[test]
    fn mean_skips_non_numeric_column() {
        // strategy mean on a text column leaves the blank alone.
        let d = "color,n\nred,1\n,2\nblue,3";
        assert_eq!(imp(d, "mean").unwrap(), "color,n\nred,1\n,2\nblue,3\n");
    }

    #[test]
    fn errors() {
        assert!(imp("   ", "mean").is_err()); // empty input
        assert!(imp("a,b\n1,2", "bogus").is_err()); // bad strategy
        assert!(impute("a,b\n1,2", true, "nope", "mean", "", "", "", 5, "uniform").is_err()); // bad delim
        assert!(impute("a,b\n1,2", true, ",", "mean", "zzz", "", "", 5, "uniform").is_err()); // unknown column
        assert!(impute("a,b\n1,2", true, ",", "knn", "", "", "", 5, "bad").is_err()); // bad weights
    }
}
