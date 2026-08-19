//! iqr-outlier-trimmer core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Computes Tukey fences (`Q1 − k·IQR` … `Q3 + k·IQR`) for one or more numeric
//! columns of a CSV/TSV table, then ACTS on the rows: drop the out-of-fence rows
//! (`remove`), keep only them (`keep`), clamp the offending cells to their fence
//! (`clip`, i.e. winsorize), or append a boolean `outlier` column (`flag`).
//! `output = "report"` returns the quartile/fence statistics instead of a table.
//!
//! Quartiles are configurable because there is no single convention:
//!
//! - `linear` — linear interpolation between order statistics (the numpy/pandas
//!   default, and Excel's `QUARTILE.INC`). Default.
//! - `exclusive` — Moore & McCabe / TI-83: split the sorted values at the median
//!   and EXCLUDE the median from both halves when the count is odd; each
//!   quartile is that half's median.
//! - `inclusive` — Tukey's hinges: the same split, but the median is INCLUDED in
//!   both halves when the count is odd.
//!
//! Blank and non-numeric cells never take part in the quartile maths; how their
//! ROWS are treated is the `non_numeric` parameter's job.

/// Maximum number of data rows (header excluded) accepted in one run.
pub const MAX_ROWS: usize = 5_000;

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

/// Parse a numeric cell, rejecting non-finite (`NaN`/`inf`) so those strings
/// don't silently join the quartile maths.
fn parse_num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Round to 6 decimals and print minimally (`0.5` stays `0.5`, `1.0` -> `1`,
/// `-0.0` -> `0`).
fn fmt_num(x: f64) -> String {
    let r = (x * 1e6).round() / 1e6;
    let r = if r == 0.0 { 0.0 } else { r };
    format!("{r}")
}

/// Linear-interpolation percentile (numpy default) over an already-sorted slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let rank = p / 100.0 * (n as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        sorted[lo] + (rank - lo as f64) * (sorted[hi] - sorted[lo])
    }
}

/// Median of an already-sorted, non-empty slice.
fn median(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

/// (Q1, Q3) of an already-sorted, non-empty slice under the chosen convention.
fn quartiles(sorted: &[f64], method: &str) -> (f64, f64) {
    let n = sorted.len();
    if n == 1 {
        return (sorted[0], sorted[0]);
    }
    match method {
        "exclusive" => {
            // Odd n: integer division drops the median from both halves.
            let half = n / 2;
            (median(&sorted[..half]), median(&sorted[n - half..]))
        }
        "inclusive" => {
            // Odd n: the median belongs to both halves (Tukey's hinges).
            let half = if n % 2 == 0 { n / 2 } else { n / 2 + 1 };
            (median(&sorted[..half]), median(&sorted[n - half..]))
        }
        // "linear" and anything already validated upstream.
        _ => (percentile(sorted, 25.0), percentile(sorted, 75.0)),
    }
}

/// Fence statistics for one analysed column.
struct ColStats {
    /// 0-based position in each record.
    index: usize,
    /// Header name, or `column N` when there is no header.
    name: String,
    numeric: usize,
    q1: f64,
    q3: f64,
    iqr: f64,
    lower: f64,
    upper: f64,
    out_of_fence: usize,
}

/// Per-cell verdict for one analysed column of one row.
#[derive(Clone, Copy, PartialEq)]
enum Cell {
    InFence,
    OutOfFence,
    NonNumeric,
}

fn resolve_column(spec: &str, header: Option<&csv::StringRecord>, width: usize) -> Result<usize, String> {
    let spec = spec.trim();
    if let Ok(n) = spec.parse::<usize>() {
        if n == 0 {
            return Err("column index is 1-based (>= 1)".into());
        }
        if n > width {
            return Err(format!("column index {n} is past the last column ({width})"));
        }
        return Ok(n - 1);
    }
    match header {
        Some(h) => h
            .iter()
            .position(|c| c.trim() == spec)
            .ok_or_else(|| format!("column '{spec}' not found in the header")),
        None => Err(format!(
            "column '{spec}' is not a number and there is no header to match names against — turn the header option on or use a 1-based index"
        )),
    }
}

/// A column qualifies for automatic selection when every present (non-blank)
/// cell parses as a finite number and at least one such cell exists.
fn is_numeric_column(rows: &[csv::StringRecord], idx: usize) -> bool {
    let mut seen = false;
    for r in rows {
        let cell = r.get(idx).unwrap_or("");
        if cell.trim().is_empty() {
            continue;
        }
        match parse_num(cell) {
            Some(_) => seen = true,
            None => return false,
        }
    }
    seen
}

/// Trim (or clip/flag/report) the IQR outliers of a CSV table.
///
/// * `columns` — comma-separated header names or 1-based indexes; blank selects
///   every numeric column.
/// * `k` — fence multiplier (1.5 = Tukey's classic "mild" fence, 3.0 = "extreme").
/// * `action` — `remove` | `keep` | `clip` | `flag`.
/// * `output` — `csv` (the resulting table) | `report` (the fence statistics).
/// * `quartile_method` — `linear` | `exclusive` | `inclusive`.
/// * `match_mode` — with several columns, is a row an outlier when `any` column
///   is out of fence, or only when `all` of them are?
/// * `non_numeric` — `keep` treats a blank/text cell as in-fence; `remove` treats
///   it as out-of-fence (so `action = "remove"` drops those rows too).
#[allow(clippy::too_many_arguments)]
pub fn trim(
    data: &str,
    columns: &str,
    k: f64,
    action: &str,
    output: &str,
    header: bool,
    delimiter: &str,
    quartile_method: &str,
    match_mode: &str,
    non_numeric: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty — paste a CSV with a numeric column".into());
    }
    if !matches!(action, "remove" | "keep" | "clip" | "flag") {
        return Err(format!("action must be remove, keep, clip or flag, got '{action}'"));
    }
    if !matches!(output, "csv" | "report") {
        return Err(format!("output must be csv or report, got '{output}'"));
    }
    if !matches!(quartile_method, "linear" | "exclusive" | "inclusive") {
        return Err(format!(
            "quartile_method must be linear, exclusive or inclusive, got '{quartile_method}'"
        ));
    }
    if !matches!(match_mode, "any" | "all") {
        return Err(format!("match must be any or all, got '{match_mode}'"));
    }
    if !matches!(non_numeric, "keep" | "remove") {
        return Err(format!("non_numeric must be keep or remove, got '{non_numeric}'"));
    }
    if !k.is_finite() || k < 0.0 {
        return Err(format!("k must be a number >= 0, got '{}'", fmt_num(k)));
    }

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
        return Err("input is empty — paste a CSV with a numeric column".into());
    }
    let (head, rows) = if header {
        let (h, r) = records.split_at(1);
        (Some(h[0].clone()), r.to_vec())
    } else {
        (None, records.clone())
    };
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "{} data rows is over the {MAX_ROWS}-row limit — split the file and trim it in batches",
            rows.len()
        ));
    }
    if rows.is_empty() {
        return Err("the input has a header but no data rows".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);

    // ---- pick the columns -------------------------------------------------
    let picked: Vec<usize> = if columns.trim().is_empty() {
        let auto: Vec<usize> = (0..width).filter(|i| is_numeric_column(&rows, *i)).collect();
        if auto.is_empty() {
            return Err("no numeric column found — name one with 'columns' (a header name or a 1-based index)".into());
        }
        auto
    } else {
        let mut out = Vec::new();
        for spec in columns.split(',').filter(|s| !s.trim().is_empty()) {
            let idx = resolve_column(spec, head.as_ref(), width)?;
            if !out.contains(&idx) {
                out.push(idx);
            }
        }
        if out.is_empty() {
            return Err("'columns' lists no column names".into());
        }
        out
    };

    // ---- fences per column ------------------------------------------------
    let mut stats: Vec<ColStats> = Vec::with_capacity(picked.len());
    for &idx in &picked {
        let mut vals: Vec<f64> = rows.iter().filter_map(|r| parse_num(r.get(idx).unwrap_or(""))).collect();
        if vals.is_empty() {
            let name = head
                .as_ref()
                .and_then(|h| h.get(idx))
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("column {}", idx + 1));
            return Err(format!("column '{name}' has no numeric values to compute quartiles from"));
        }
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let (q1, q3) = quartiles(&vals, quartile_method);
        let iqr = q3 - q1;
        stats.push(ColStats {
            index: idx,
            name: head
                .as_ref()
                .and_then(|h| h.get(idx))
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("column {}", idx + 1)),
            numeric: vals.len(),
            q1,
            q3,
            iqr,
            lower: q1 - k * iqr,
            upper: q3 + k * iqr,
            out_of_fence: 0,
        });
    }

    // ---- classify every row ----------------------------------------------
    let drop_non_numeric = non_numeric == "remove";
    let mut outlier_row = Vec::with_capacity(rows.len());
    for r in &rows {
        let cells: Vec<Cell> = stats
            .iter()
            .map(|s| match parse_num(r.get(s.index).unwrap_or("")) {
                Some(v) if v < s.lower || v > s.upper => Cell::OutOfFence,
                Some(_) => Cell::InFence,
                None => Cell::NonNumeric,
            })
            .collect();
        for (s, c) in stats.iter_mut().zip(&cells) {
            if *c == Cell::OutOfFence {
                s.out_of_fence += 1;
            }
        }
        let flagged = |c: &Cell| *c == Cell::OutOfFence || (drop_non_numeric && *c == Cell::NonNumeric);
        outlier_row.push(match match_mode {
            "all" => cells.iter().all(flagged),
            _ => cells.iter().any(flagged),
        });
    }
    let n_out = outlier_row.iter().filter(|b| **b).count();
    let n_clean = rows.len() - n_out;

    if output == "report" {
        return Ok(render_report(
            &stats,
            rows.len(),
            n_out,
            n_clean,
            k,
            action,
            quartile_method,
            match_mode,
        ));
    }

    // ---- build the output table ------------------------------------------
    let mut wtr = csv::WriterBuilder::new().delimiter(delim).flexible(true).from_writer(vec![]);
    let flag_name = head.as_ref().map(|h| unique_flag_name(h));
    if let Some(h) = &head {
        let mut rec = h.clone();
        if action == "flag" {
            rec.push_field(flag_name.as_deref().unwrap_or("outlier"));
        }
        wtr.write_record(&rec).map_err(|e| format!("CSV write error: {e}"))?;
    }
    for (r, &is_out) in rows.iter().zip(&outlier_row) {
        match action {
            "remove" if is_out => continue,
            "keep" if !is_out => continue,
            _ => {}
        }
        let mut rec = r.clone();
        if action == "clip" {
            for s in &stats {
                if let Some(v) = parse_num(rec.get(s.index).unwrap_or("")) {
                    if v < s.lower || v > s.upper {
                        let clamped = if v < s.lower { s.lower } else { s.upper };
                        let mut fields: Vec<String> = rec.iter().map(|f| f.to_string()).collect();
                        fields[s.index] = fmt_num(clamped);
                        rec = csv::StringRecord::from(fields);
                    }
                }
            }
        } else if action == "flag" {
            rec.push_field(if is_out { "true" } else { "false" });
        }
        wtr.write_record(&rec).map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

/// `outlier`, or `outlier_2`, `outlier_3`… when the header already uses it.
fn unique_flag_name(head: &csv::StringRecord) -> String {
    let taken = |n: &str| head.iter().any(|c| c.trim() == n);
    if !taken("outlier") {
        return "outlier".to_string();
    }
    let mut i = 2;
    loop {
        let cand = format!("outlier_{i}");
        if !taken(&cand) {
            return cand;
        }
        i += 1;
    }
}

fn pct(part: usize, whole: usize) -> String {
    if whole == 0 {
        return "0%".to_string();
    }
    format!("{}%", fmt_num(part as f64 * 100.0 / whole as f64))
}

#[allow(clippy::too_many_arguments)]
fn render_report(
    stats: &[ColStats],
    total: usize,
    n_out: usize,
    n_clean: usize,
    k: f64,
    action: &str,
    quartile_method: &str,
    match_mode: &str,
) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "IQR outlier report — k = {}, quartiles = {quartile_method}, match = {match_mode}, action = {action}\n",
        fmt_num(k)
    ));
    for c in stats {
        s.push_str(&format!("\nColumn: {} (column {})\n", c.name, c.index + 1));
        s.push_str(&format!("  numeric values: {}\n", c.numeric));
        s.push_str(&format!("  Q1: {}\n", fmt_num(c.q1)));
        s.push_str(&format!("  Q3: {}\n", fmt_num(c.q3)));
        s.push_str(&format!("  IQR: {}\n", fmt_num(c.iqr)));
        s.push_str(&format!("  lower fence: {}\n", fmt_num(c.lower)));
        s.push_str(&format!("  upper fence: {}\n", fmt_num(c.upper)));
        s.push_str(&format!(
            "  out of fence: {} of {} ({})\n",
            c.out_of_fence,
            c.numeric,
            pct(c.out_of_fence, c.numeric)
        ));
    }
    let kept = match action {
        "remove" => n_clean,
        "keep" => n_out,
        _ => total,
    };
    s.push_str(&format!(
        "\nRows: {total} total, {n_out} outlier ({}), {n_clean} clean\n",
        pct(n_out, total)
    ));
    s.push_str(&format!("Rows in the '{action}' output: {kept}\n"));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: &str = "name,price\na,10\nb,11\nc,12\nd,13\ne,100";

    fn run(data: &str, columns: &str, k: f64, action: &str, output: &str) -> Result<String, String> {
        trim(data, columns, k, action, output, true, "comma", "linear", "any", "keep")
    }

    #[test]
    fn removes_the_outlier_row() {
        assert_eq!(run(D, "", 1.5, "remove", "csv").unwrap(), "name,price\na,10\nb,11\nc,12\nd,13\n");
    }

    #[test]
    fn keep_returns_only_the_outlier_rows() {
        assert_eq!(run(D, "price", 1.5, "keep", "csv").unwrap(), "name,price\ne,100\n");
    }

    #[test]
    fn clip_winsorizes_and_keeps_the_row_count() {
        // fences are 8 … 16, so 100 is clamped to the upper fence.
        assert_eq!(
            run(D, "price", 1.5, "clip", "csv").unwrap(),
            "name,price\na,10\nb,11\nc,12\nd,13\ne,16\n"
        );
    }

    #[test]
    fn flag_appends_a_boolean_column() {
        assert_eq!(
            run(D, "price", 1.5, "flag", "csv").unwrap(),
            "name,price,outlier\na,10,false\nb,11,false\nc,12,false\nd,13,false\ne,100,true\n"
        );
    }

    #[test]
    fn flag_column_name_is_uniquified() {
        let d = "v,outlier\n1,x\n2,y\n3,z";
        let out = run(d, "v", 1.5, "flag", "csv").unwrap();
        assert!(out.starts_with("v,outlier,outlier_2\n"), "got {out}");
    }

    #[test]
    fn a_larger_k_widens_the_fences_and_trims_nothing() {
        assert_eq!(run(D, "price", 50.0, "remove", "csv").unwrap(), format!("{D}\n"));
    }

    #[test]
    fn report_lists_the_fences_and_row_counts() {
        let r = run(D, "price", 1.5, "remove", "report").unwrap();
        assert!(r.contains("Q1: 11\n"), "{r}");
        assert!(r.contains("Q3: 13\n"), "{r}");
        assert!(r.contains("IQR: 2\n"), "{r}");
        assert!(r.contains("lower fence: 8\n"), "{r}");
        assert!(r.contains("upper fence: 16\n"), "{r}");
        assert!(r.contains("out of fence: 1 of 5 (20%)"), "{r}");
        assert!(r.contains("Rows: 5 total, 1 outlier (20%), 4 clean"), "{r}");
        assert!(r.contains("Rows in the 'remove' output: 4"), "{r}");
    }

    #[test]
    fn quartile_methods_differ_on_odd_counts() {
        // 1..9 plus a high value: linear vs exclusive vs inclusive fences.
        let d = "v\n1\n2\n3\n4\n5\n6\n7\n8\n9";
        let q = |m: &str| {
            let out = trim(d, "v", 1.5, "remove", "report", true, "comma", m, "any", "keep").unwrap();
            let grab = |label: &str| {
                let line = out.lines().find(|l| l.trim().starts_with(label)).unwrap();
                line.trim().split(' ').nth(1).unwrap().to_string()
            };
            (grab("Q1:"), grab("Q3:"))
        };
        assert_eq!(q("linear"), ("3".to_string(), "7".to_string()));
        assert_eq!(q("exclusive"), ("2.5".to_string(), "7.5".to_string()));
        assert_eq!(q("inclusive"), ("3".to_string(), "7".to_string()));
    }

    #[test]
    fn match_all_needs_every_column_out_of_fence() {
        // `a` has an outlier on row 5, `b` on row 6.
        let d = "a,b\n1,1\n2,2\n3,3\n4,4\n999,5\n5,999";
        let any = trim(d, "a,b", 1.5, "remove", "csv", true, "comma", "linear", "any", "keep").unwrap();
        assert_eq!(any, "a,b\n1,1\n2,2\n3,3\n4,4\n");
        let all = trim(d, "a,b", 1.5, "remove", "csv", true, "comma", "linear", "all", "keep").unwrap();
        assert_eq!(all, "a,b\n1,1\n2,2\n3,3\n4,4\n999,5\n5,999\n");
    }

    #[test]
    fn non_numeric_rows_are_kept_by_default_and_removable() {
        let d = "v\n1\n2\n3\n4\nn/a";
        assert_eq!(
            trim(d, "v", 1.5, "remove", "csv", true, "comma", "linear", "any", "keep").unwrap(),
            "v\n1\n2\n3\n4\nn/a\n"
        );
        assert_eq!(
            trim(d, "v", 1.5, "remove", "csv", true, "comma", "linear", "any", "remove").unwrap(),
            "v\n1\n2\n3\n4\n"
        );
    }

    #[test]
    fn tab_delimited_without_a_header_uses_1_based_indexes() {
        let d = "a\t10\nb\t11\nc\t12\nd\t13\ne\t100";
        assert_eq!(
            trim(d, "2", 1.5, "remove", "csv", false, "tab", "linear", "any", "keep").unwrap(),
            "a\t10\nb\t11\nc\t12\nd\t13\n"
        );
    }

    #[test]
    fn auto_selection_skips_text_columns() {
        // only `price` is numeric, so the text column never trims a row.
        assert_eq!(run(D, "", 1.5, "keep", "csv").unwrap(), "name,price\ne,100\n");
    }

    #[test]
    fn row_cap_is_enforced() {
        let mut d = String::from("v\n");
        for i in 0..MAX_ROWS {
            d.push_str(&format!("{i}\n"));
        }
        assert!(trim(&d, "v", 1.5, "remove", "csv", true, "comma", "linear", "any", "keep").is_ok());
        d.push_str("1\n");
        let err = trim(&d, "v", 1.5, "remove", "csv", true, "comma", "linear", "any", "keep").unwrap_err();
        assert!(err.contains("5000-row limit"), "{err}");
    }

    #[test]
    fn errors() {
        assert!(run("   ", "", 1.5, "remove", "csv").is_err()); // empty
        assert!(run(D, "nope", 1.5, "remove", "csv").is_err()); // unknown column
        assert!(run(D, "9", 1.5, "remove", "csv").is_err()); // index past the end
        assert!(run(D, "0", 1.5, "remove", "csv").is_err()); // 0-based index
        assert!(run(D, "name", 1.5, "remove", "csv").is_err()); // no numeric values
        assert!(run("name,city\na,x\nb,y", "", 1.5, "remove", "csv").is_err()); // no numeric column
        assert!(run(D, "price", -1.0, "remove", "csv").is_err()); // negative k
        assert!(run(D, "price", 1.5, "nope", "csv").is_err()); // bad action
        assert!(run(D, "price", 1.5, "remove", "nope").is_err()); // bad output
        assert!(trim(D, "price", 1.5, "remove", "csv", true, "comma", "nope", "any", "keep").is_err());
        assert!(trim(D, "price", 1.5, "remove", "csv", true, "comma", "linear", "nope", "keep").is_err());
        assert!(trim(D, "price", 1.5, "remove", "csv", true, "comma", "linear", "any", "nope").is_err());
        assert!(trim(D, "price", 1.5, "remove", "csv", true, "comma;;", "linear", "any", "keep").is_err());
        assert!(trim("v", "v", 1.5, "remove", "csv", true, "comma", "linear", "any", "keep").is_err()); // header only
    }
}
