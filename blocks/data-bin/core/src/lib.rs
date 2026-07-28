//! data-bin core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps. Bins ONE numeric column of a CSV into
//! equal-width, quantile (equal-frequency), or custom-edge buckets and labels
//! each row with the bucket it falls in.
//!
//! The chosen column must be NUMERIC — every present (non-blank) value in it
//! parses as a finite number. Blank cells stay blank (they carry no bin label
//! and are excluded from the bin-edge statistics). The label of every other row
//! is the bin it falls in; in `append` mode a new column is added, in `replace`
//! mode the source column's values are overwritten with the label. The output
//! delimiter and header handling match the input.

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
/// don't silently mark a text column as numeric.
fn parse_num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Format a numeric edge: round to `precision` decimals and print minimally
/// (`0.5` stays `0.5`, `1.0` -> `1`, `-0.0` -> `0`).
fn fmt_edge(x: f64, precision: u32) -> String {
    let f = 10f64.powi(precision as i32);
    let r = (x * f).round() / f;
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

/// Bin one numeric column of a CSV and label each row.
///
/// * `header` — treat the first row as a header (used for `column` name lookup and kept verbatim).
/// * `delimiter` — the field separator (char or comma/tab/semicolon/pipe).
/// * `column` — the single column to bin: a header NAME (needs a header) or a 1-based index.
/// * `method` — `equal_width` | `quantile` | `custom`.
/// * `bins` — number of bins for `equal_width`/`quantile` (ignored for `custom`).
/// * `edges` — comma-separated ascending edge list for `method = custom`.
/// * `labels` — comma-separated custom bin labels (one per bin); blank = auto-generate.
/// * `label_style` — `range` (interval string) or `index` (1-based bin number) when `labels` is blank.
/// * `right` — right-closed intervals `(a, b]` (true) or left-closed `[a, b)` (false).
/// * `precision` — decimal places for numbers shown in `range` labels.
/// * `output` — `append` (add a bin column) or `replace` (overwrite the source column).
#[allow(clippy::too_many_arguments)]
pub fn bin(
    data: &str,
    header: bool,
    delimiter: &str,
    column: &str,
    method: &str,
    bins: u32,
    edges: &str,
    labels: &str,
    label_style: &str,
    right: bool,
    precision: u32,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let method = method.trim();
    if !matches!(method, "equal_width" | "quantile" | "custom") {
        return Err(format!(
            "method must be equal_width/quantile/custom, got '{method}'"
        ));
    }
    let label_style = if label_style.trim().is_empty() {
        "range"
    } else {
        label_style.trim()
    };
    if !matches!(label_style, "range" | "index") {
        return Err(format!(
            "label_style must be range/index, got '{label_style}'"
        ));
    }
    let output = if output.trim().is_empty() {
        "append"
    } else {
        output.trim()
    };
    if !matches!(output, "append" | "replace") {
        return Err(format!("output must be append/replace, got '{output}'"));
    }
    if column.trim().is_empty() {
        return Err("column is required (a header name or a 1-based index)".into());
    }
    if precision > 12 {
        return Err(format!("precision must be 0..=12, got {precision}"));
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

    // Resolve the single target column.
    let col = resolve_column(column.trim(), &header_names, ncols)?;

    if grid.len() <= data_start {
        // Header-only (or empty) input — nothing to bin. In append mode still add
        // the header for the new bin column so the shape is predictable.
        if output == "append" {
            let name = bin_column_name(&header_names, col);
            for (i, row) in grid.iter_mut().enumerate() {
                row.push(if header && i == 0 { name.clone() } else { String::new() });
            }
        }
        return write_csv(&grid, delim);
    }

    // The target column must be numeric.
    let all_numeric = grid[data_start..]
        .iter()
        .all(|row| is_blank(&row[col]) || parse_num(&row[col]).is_some());
    if !all_numeric {
        return Err(format!(
            "column '{}' is not numeric — every present value must parse as a finite number",
            column.trim()
        ));
    }

    // Present values (blanks excluded) drive the edges.
    let present: Vec<f64> = grid[data_start..]
        .iter()
        .filter(|row| !is_blank(&row[col]))
        .filter_map(|row| parse_num(&row[col]))
        .collect();
    if present.is_empty() {
        return Err("column has no numeric values to bin".into());
    }

    let edge_vals = compute_edges(method, &present, bins, edges)?;
    let nbins = edge_vals.len() - 1;

    // Bin labels: custom (if provided) else auto per label_style.
    let bin_labels: Vec<String> = if labels.trim().is_empty() {
        (0..nbins)
            .map(|i| auto_label(&edge_vals, i, nbins, right, label_style, precision))
            .collect()
    } else {
        let custom: Vec<String> = labels.split(',').map(|s| s.trim().to_string()).collect();
        if custom.len() != nbins {
            return Err(format!(
                "labels has {} entries but there are {} bins — provide exactly {} comma-separated labels",
                custom.len(),
                nbins,
                nbins
            ));
        }
        custom
    };

    // Assign each present value to a bin, then materialize the label per row.
    let name = bin_column_name(&header_names, col);
    for (i, row) in grid.iter_mut().enumerate() {
        if i < data_start {
            if output == "append" {
                row.push(if header && i == 0 { name.clone() } else { String::new() });
            }
            continue;
        }
        let label = if is_blank(&row[col]) {
            String::new()
        } else {
            match parse_num(&row[col]).and_then(|v| assign_bin(v, &edge_vals, right)) {
                Some(b) => bin_labels[b].clone(),
                None => String::new(), // out of range (custom edges only)
            }
        };
        match output {
            "append" => row.push(label),
            "replace" => row[col] = label,
            _ => unreachable!(),
        }
    }

    write_csv(&grid, delim)
}

/// Resolve a single column spec to a 0-based index.
fn resolve_column(
    tok: &str,
    header_names: &Option<Vec<String>>,
    ncols: usize,
) -> Result<usize, String> {
    if let Some(names) = header_names {
        if let Some(pos) = names.iter().position(|n| n == tok) {
            return Ok(pos);
        }
    }
    let n: usize = tok
        .parse()
        .map_err(|_| format!("unknown column '{tok}' (use a header name or a 1-based index)"))?;
    if n == 0 || n > ncols {
        return Err(format!("column index {n} out of range 1..={ncols}"));
    }
    Ok(n - 1)
}

/// Name for the appended bin column: `<header>_bin` when a header exists, else `bin`.
fn bin_column_name(header_names: &Option<Vec<String>>, col: usize) -> String {
    match header_names {
        Some(names) if !names[col].is_empty() => format!("{}_bin", names[col]),
        _ => "bin".to_string(),
    }
}

/// Compute the ascending bin edges (length = nbins + 1) for the chosen method.
fn compute_edges(
    method: &str,
    present: &[f64],
    bins: u32,
    edges: &str,
) -> Result<Vec<f64>, String> {
    match method {
        "custom" => {
            let mut out: Vec<f64> = Vec::new();
            for tok in edges.split(',') {
                let tok = tok.trim();
                if tok.is_empty() {
                    continue;
                }
                let v = parse_num(tok)
                    .ok_or_else(|| format!("edges must be finite numbers, got '{tok}'"))?;
                out.push(v);
            }
            if out.len() < 2 {
                return Err(
                    "method='custom' needs at least 2 comma-separated edges (e.g. '0,10,20')".into(),
                );
            }
            for w in out.windows(2) {
                if !(w[0] < w[1]) {
                    return Err(format!(
                        "custom edges must be strictly ascending, got {} then {}",
                        w[0], w[1]
                    ));
                }
            }
            Ok(out)
        }
        "equal_width" | "quantile" => {
            if bins < 1 {
                return Err("bins must be >= 1".into());
            }
            if bins > 1000 {
                return Err(format!("bins must be <= 1000, got {bins}"));
            }
            let min = present.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = present.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            if min == max {
                // Constant column: a single bin spanning [min, max].
                return Ok(vec![min, max]);
            }
            let raw: Vec<f64> = if method == "equal_width" {
                (0..=bins)
                    .map(|i| min + (max - min) * (i as f64) / (bins as f64))
                    .collect()
            } else {
                let mut sorted = present.to_vec();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                (0..=bins)
                    .map(|i| percentile(&sorted, 100.0 * (i as f64) / (bins as f64)))
                    .collect()
            };
            // Drop duplicate edges (skewed quantiles / float ties) so every bin is
            // non-empty. Keeps the first and last edge; collapses interior repeats.
            let mut deduped: Vec<f64> = Vec::with_capacity(raw.len());
            for &e in &raw {
                if deduped.last().map(|&l| l != e).unwrap_or(true) {
                    deduped.push(e);
                }
            }
            if deduped.len() < 2 {
                deduped = vec![min, max];
            }
            Ok(deduped)
        }
        _ => unreachable!(),
    }
}

/// Assign a value to a 0-based bin index given ascending edges.
/// `right`: intervals are `(e[i-1], e[i]]` with the first also including its low
/// edge. `!right`: `[e[i-1], e[i])` with the last also including its high edge.
fn assign_bin(v: f64, edges: &[f64], right: bool) -> Option<usize> {
    let n = edges.len() - 1; // number of bins
    let lo = edges[0];
    let hi = edges[n];
    if v < lo || v > hi {
        return None; // outside the covered range (custom edges)
    }
    if right {
        // First bin is closed on both ends [lo, e1]; others (e[i-1], e[i]].
        if v <= edges[1] {
            return Some(0);
        }
        for i in 1..n {
            if v > edges[i] && v <= edges[i + 1] {
                return Some(i);
            }
        }
        Some(n - 1)
    } else {
        // Last bin is closed on both ends [e[n-1], hi]; others [e[i-1], e[i]).
        if v >= edges[n - 1] {
            return Some(n - 1);
        }
        for i in 0..n {
            if v >= edges[i] && v < edges[i + 1] {
                return Some(i);
            }
        }
        Some(0)
    }
}

/// Auto-generate a bin's label (range interval or 1-based index).
fn auto_label(
    edges: &[f64],
    i: usize,
    nbins: usize,
    right: bool,
    style: &str,
    precision: u32,
) -> String {
    if style == "index" {
        return format!("{}", i + 1);
    }
    let a = fmt_edge(edges[i], precision);
    let b = fmt_edge(edges[i + 1], precision);
    if right {
        // First bin closed-closed, rest open-closed.
        if i == 0 {
            format!("[{a}, {b}]")
        } else {
            format!("({a}, {b}]")
        }
    } else {
        // Last bin closed-closed, rest closed-open.
        if i == nbins - 1 {
            format!("[{a}, {b}]")
        } else {
            format!("[{a}, {b})")
        }
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

    #[allow(clippy::too_many_arguments)]
    fn run(
        data: &str,
        column: &str,
        method: &str,
        bins: u32,
        edges: &str,
        labels: &str,
        style: &str,
        right: bool,
        output: &str,
    ) -> Result<String, String> {
        bin(data, true, ",", column, method, bins, edges, labels, style, right, 3, output)
    }

    #[test]
    fn equal_width_range_labels_append() {
        // 0..100 into 2 equal-width bins: edges 0,50,100.
        let d = "id,score\na,0\nb,25\nc,50\nd,75\ne,100";
        let got = run(d, "score", "equal_width", 2, "", "", "range", true, "append").unwrap();
        assert_eq!(
            got,
            "id,score,score_bin\na,0,\"[0, 50]\"\nb,25,\"[0, 50]\"\nc,50,\"[0, 50]\"\nd,75,\"(50, 100]\"\ne,100,\"(50, 100]\"\n"
        );
    }

    #[test]
    fn equal_width_index_labels() {
        let d = "score\n0\n50\n100";
        let got = run(d, "score", "equal_width", 2, "", "", "index", true, "append").unwrap();
        assert_eq!(got, "score,score_bin\n0,1\n50,1\n100,2\n");
    }

    #[test]
    fn left_closed_intervals() {
        // right=false: [0,50) , [50,100]  -> 50 lands in the last bin.
        let d = "score\n0\n50\n100";
        let got = run(d, "score", "equal_width", 2, "", "", "range", false, "append").unwrap();
        assert_eq!(
            got,
            "score,score_bin\n0,\"[0, 50)\"\n50,\"[50, 100]\"\n100,\"[50, 100]\"\n"
        );
    }

    #[test]
    fn quantile_equal_frequency() {
        // 1..4 into 2 quantile bins: median 2.5 -> edges 1,2.5,4.
        let d = "v\n1\n2\n3\n4";
        let got = run(d, "v", "quantile", 2, "", "", "range", true, "append").unwrap();
        assert_eq!(
            got,
            "v,v_bin\n1,\"[1, 2.5]\"\n2,\"[1, 2.5]\"\n3,\"(2.5, 4]\"\n4,\"(2.5, 4]\"\n"
        );
    }

    #[test]
    fn custom_edges_with_labels_replace() {
        // Age bands via edges 0,18,65,120 -> child/adult/senior.
        let d = "name,age\nA,10\nB,40\nC,80";
        let got = bin(
            d, true, ",", "age", "custom", 4, "0,18,65,120", "child,adult,senior", "range", false,
            3, "replace",
        )
        .unwrap();
        assert_eq!(got, "name,age\nA,child\nB,adult\nC,senior\n");
    }

    #[test]
    fn custom_edges_out_of_range_blank() {
        // 200 is above the top edge 120 -> blank label.
        let d = "age\n40\n200";
        let got = bin(
            d, true, ",", "age", "custom", 4, "0,18,65,120", "", "index", true, 3, "append",
        )
        .unwrap();
        assert_eq!(got, "age,age_bin\n40,2\n200,\n");
    }

    #[test]
    fn blank_cells_stay_blank() {
        // A blank target cell (in a multi-column row) carries no label and is left
        // out of the min/max used for the edges (edges here span 10..90).
        let d = "name,score\nA,10\nB,\nC,90";
        let got = run(d, "score", "equal_width", 2, "", "", "index", true, "append").unwrap();
        assert_eq!(got, "name,score,score_bin\nA,10,1\nB,,\nC,90,2\n");
    }

    #[test]
    fn constant_column_single_bin() {
        let d = "x\n5\n5\n5";
        let got = run(d, "x", "equal_width", 4, "", "", "index", true, "append").unwrap();
        assert_eq!(got, "x,x_bin\n5,1\n5,1\n5,1\n");
    }

    #[test]
    fn column_by_index_and_tab_delimiter() {
        let d = "1\t10\n2\t20\n3\t30";
        let got = bin(
            d, false, "tab", "2", "equal_width", 2, "", "", "index", true, 3, "append",
        )
        .unwrap();
        // No header: 3 data rows, edges 10,20,30 -> bins 1,1,2.
        assert_eq!(got, "1\t10\t1\n2\t20\t1\n3\t30\t2\n");
    }

    #[test]
    fn errors() {
        // empty input
        assert!(run("  ", "x", "equal_width", 2, "", "", "range", true, "append").is_err());
        // bad method
        assert!(run("x\n1", "x", "bogus", 2, "", "", "range", true, "append").is_err());
        // non-numeric target column
        assert!(run("x\nfoo\nbar", "x", "equal_width", 2, "", "", "range", true, "append").is_err());
        // unknown column
        assert!(run("x\n1", "zzz", "equal_width", 2, "", "", "range", true, "append").is_err());
        // custom with too few edges
        assert!(run("x\n1", "x", "custom", 2, "5", "", "range", true, "append").is_err());
        // custom non-ascending edges
        assert!(run("x\n1", "x", "custom", 2, "5,3", "", "range", true, "append").is_err());
        // wrong label count
        assert!(run("x\n1\n2\n3", "x", "equal_width", 2, "", "only-one", "range", true, "append").is_err());
        // bad label_style
        assert!(bin("x\n1", true, ",", "x", "equal_width", 2, "", "", "nope", true, 3, "append").is_err());
        // bad output mode
        assert!(bin("x\n1", true, ",", "x", "equal_width", 2, "", "", "range", true, 3, "nope").is_err());
    }
}
