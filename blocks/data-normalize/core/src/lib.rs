//! data-normalize core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Scales the numeric columns of a CSV with
//! min-max, z-score (standard), or robust (median/IQR) normalization.
//!
//! A target column is scaled only when it is NUMERIC — every present (non-blank)
//! value parses as a finite number. Text columns are copied verbatim, and blank
//! cells stay blank (they are excluded from the column statistics). The delimiter
//! and header handling of the output match the input.

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

/// Format a scaled numeric value: round to 6 decimals and print minimally
/// (`0.5` stays `0.5`, `1.0` -> `1`, `-0.0` -> `0`).
fn fmt_num(x: f64) -> String {
    let r = (x * 1e6).round() / 1e6;
    let r = if r == 0.0 { 0.0 } else { r }; // normalize -0.0 to 0
    format!("{r}")
}

/// Linear-interpolation percentile (numpy default) over an already-sorted slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return sorted[0];
    }
    let rank = p / 100.0 * (n as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = rank - lo as f64;
        sorted[lo] + frac * (sorted[hi] - sorted[lo])
    }
}

/// Scale the numeric columns of a CSV.
///
/// * `header` — treat the first row as a header (used for `columns` name lookup and kept verbatim).
/// * `delimiter` — the field separator (char or comma/tab/semicolon/pipe).
/// * `method` — `min_max` | `z_score` | `robust`.
/// * `columns` — comma-separated column NAMES (needs a header) or 1-based indices to scale;
///   blank scales every numeric column.
/// * `range_min` / `range_max` — target range for `min_max` (requires `range_min < range_max`).
/// * `ddof` — delta degrees of freedom for `z_score` stddev: 0 = population, 1 = sample.
/// * `with_centering` / `with_scaling` — for `robust`, whether to subtract the median and/or
///   divide by the IQR. When the IQR is zero, scaling divides by 1.
#[allow(clippy::too_many_arguments)]
pub fn normalize(
    data: &str,
    header: bool,
    delimiter: &str,
    method: &str,
    columns: &str,
    range_min: f64,
    range_max: f64,
    ddof: u32,
    with_centering: bool,
    with_scaling: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let method = method.trim();
    if !matches!(method, "min_max" | "z_score" | "robust") {
        return Err(format!(
            "method must be min_max/z_score/robust, got '{method}'"
        ));
    }
    if method == "min_max" && !(range_min < range_max) {
        return Err(format!(
            "range_min ({range_min}) must be less than range_max ({range_max})"
        ));
    }
    if method == "z_score" && ddof > 1 {
        return Err(format!("ddof must be 0 or 1, got {ddof}"));
    }
    let delim = delim_byte(delimiter)?;

    let is_blank = |cell: &str| cell.trim().is_empty();

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

    // Rectangular grid: pad short rows with empty cells.
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
        // Header only (or empty) — nothing to scale; echo back.
        return write_csv(&grid, delim);
    }

    // Resolve the target columns to scale.
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
                .all(|row| is_blank(&row[c]) || parse_num(&row[c]).is_some())
        })
        .collect();

    for &c in &targets {
        if !numeric[c] {
            continue; // text column — copy verbatim
        }
        // Present values for this column (blanks excluded).
        let present: Vec<f64> = grid[data_start..]
            .iter()
            .filter(|row| !is_blank(&row[c]))
            .filter_map(|row| parse_num(&row[c]))
            .collect();
        if present.is_empty() {
            continue;
        }
        let scale_cell = build_scaler(
            method,
            &present,
            range_min,
            range_max,
            ddof,
            with_centering,
            with_scaling,
        );
        for row in grid[data_start..].iter_mut() {
            if is_blank(&row[c]) {
                continue; // missing stays blank
            }
            if let Some(x) = parse_num(&row[c]) {
                row[c] = fmt_num(scale_cell(x));
            }
        }
    }

    write_csv(&grid, delim)
}

/// Build a per-value scaling closure for a column from its present values.
fn build_scaler(
    method: &str,
    present: &[f64],
    range_min: f64,
    range_max: f64,
    ddof: u32,
    with_centering: bool,
    with_scaling: bool,
) -> Box<dyn Fn(f64) -> f64> {
    match method {
        "min_max" => {
            let min = present.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = present.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let range = max - min;
            let span = range_max - range_min;
            Box::new(move |x| {
                if range == 0.0 {
                    range_min // constant column collapses to the low end
                } else {
                    range_min + (x - min) / range * span
                }
            })
        }
        "z_score" => {
            let n = present.len() as f64;
            let mean = present.iter().sum::<f64>() / n;
            let denom = n - ddof as f64;
            let var = if denom > 0.0 {
                present.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / denom
            } else {
                0.0
            };
            let std = var.sqrt();
            Box::new(move |x| if std == 0.0 { 0.0 } else { (x - mean) / std })
        }
        "robust" => {
            let mut sorted = present.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let center = if with_centering {
                percentile(&sorted, 50.0)
            } else {
                0.0
            };
            let iqr = percentile(&sorted, 75.0) - percentile(&sorted, 25.0);
            let scale = if with_scaling && iqr != 0.0 { iqr } else { 1.0 };
            Box::new(move |x| (x - center) / scale)
        }
        _ => unreachable!(),
    }
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

    fn norm(data: &str, method: &str) -> Result<String, String> {
        normalize(data, true, ",", method, "", 0.0, 1.0, 0, true, true)
    }

    #[test]
    fn min_max_scales_each_numeric_column() {
        // a: 1,2,3 -> 0,0.5,1 ; b: 10,20,30 -> 0,0.5,1
        let d = "a,b\n1,10\n2,20\n3,30";
        assert_eq!(norm(d, "min_max").unwrap(), "a,b\n0,0\n0.5,0.5\n1,1\n");
    }

    #[test]
    fn min_max_custom_range() {
        // range -1..1 on 1,2,3 -> -1,0,1
        let d = "x\n1\n2\n3";
        let got = normalize(d, true, ",", "min_max", "", -1.0, 1.0, 0, true, true).unwrap();
        assert_eq!(got, "x\n-1\n0\n1\n");
    }

    #[test]
    fn z_score_population_stddev() {
        // mean 2, population std sqrt(2/3)=0.816497 -> -1.224745,0,1.224745
        let d = "x\n1\n2\n3";
        assert_eq!(norm(d, "z_score").unwrap(), "x\n-1.224745\n0\n1.224745\n");
    }

    #[test]
    fn z_score_sample_stddev_ddof1() {
        // mean 2, sample std sqrt(2/2)=1 -> -1,0,1
        let d = "x\n1\n2\n3";
        let got = normalize(d, true, ",", "z_score", "", 0.0, 1.0, 1, true, true).unwrap();
        assert_eq!(got, "x\n-1\n0\n1\n");
    }

    #[test]
    fn z_score_zero_stddev_outputs_zero() {
        let d = "x\n5\n5\n5";
        assert_eq!(norm(d, "z_score").unwrap(), "x\n0\n0\n0\n");
    }

    #[test]
    fn robust_median_iqr() {
        // 1..5: median 3, Q1 2, Q3 4, IQR 2 -> (x-3)/2
        let d = "v\n1\n2\n3\n4\n5";
        assert_eq!(norm(d, "robust").unwrap(), "v\n-1\n-0.5\n0\n0.5\n1\n");
    }

    #[test]
    fn robust_scaling_only_no_centering() {
        // with_centering=false: divide by IQR (2) only -> x/2
        let d = "v\n1\n2\n3\n4\n5";
        let got = normalize(d, true, ",", "robust", "", 0.0, 1.0, 0, false, true).unwrap();
        assert_eq!(got, "v\n0.5\n1\n1.5\n2\n2.5\n");
    }

    #[test]
    fn robust_zero_iqr_scales_by_one() {
        // constant column: IQR 0 -> scale by 1; with centering -> all 0
        let d = "v\n7\n7\n7";
        assert_eq!(norm(d, "robust").unwrap(), "v\n0\n0\n0\n");
    }

    #[test]
    fn selected_columns_by_name_and_index() {
        let d = "a,b\n1,10\n2,20\n3,30";
        // only b by name
        let by_name = normalize(d, true, ",", "min_max", "b", 0.0, 1.0, 0, true, true).unwrap();
        assert_eq!(by_name, "a,b\n1,0\n2,0.5\n3,1\n");
        // only b by 1-based index
        let by_idx = normalize(d, true, ",", "min_max", "2", 0.0, 1.0, 0, true, true).unwrap();
        assert_eq!(by_idx, by_name);
    }

    #[test]
    fn tab_delimiter_and_header_false() {
        let d = "1\t10\n2\t20\n3\t30";
        let got = normalize(d, false, "tab", "min_max", "", 0.0, 1.0, 0, true, true).unwrap();
        assert_eq!(got, "0\t0\n0.5\t0.5\n1\t1\n");
    }

    #[test]
    fn blanks_preserved_and_text_column_skipped() {
        // name is text (copied); val has a blank that stays blank.
        let d = "name,val\nAlice,10\nBob,\nCarol,30";
        assert_eq!(
            norm(d, "min_max").unwrap(),
            "name,val\nAlice,0\nBob,\nCarol,1\n"
        );
    }

    #[test]
    fn errors() {
        assert!(norm("   ", "min_max").is_err()); // empty input
        assert!(norm("a,b\n1,2", "bogus").is_err()); // bad method
        assert!(normalize("a,b\n1,2", true, "nope", "min_max", "", 0.0, 1.0, 0, true, true).is_err()); // bad delim
        assert!(normalize("a,b\n1,2", true, ",", "min_max", "zzz", 0.0, 1.0, 0, true, true).is_err()); // unknown column
        assert!(normalize("a,b\n1,2", true, ",", "min_max", "", 1.0, 1.0, 0, true, true).is_err()); // invalid range
    }
}
