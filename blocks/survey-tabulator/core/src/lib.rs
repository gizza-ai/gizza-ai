//! survey-tabulator core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Two modes over a survey-response CSV (first row = question headers, each later
//! row = one respondent):
//!   * `overview`  — a frequency table (count + % of responses) for every question
//!                   column, or just one if `question` is given.
//!   * `crosstab`  — a two-way contingency table of `question` (rows) against `by`
//!                   (columns) with counts + row/column/total percentages, marginal
//!                   totals, and optional chi-square / Cramér's V / p-value stats.

use std::collections::HashMap;

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

/// Resolve a column reference (1-based index OR header name) to a 0-based index.
fn resolve(name: &str, header: &csv::StringRecord) -> Result<usize, String> {
    let name = name.trim();
    if let Ok(n) = name.parse::<usize>() {
        if n == 0 {
            return Err("column index is 1-based (>= 1)".into());
        }
        if n > header.len() {
            return Err(format!(
                "column index {n} is out of range (header has {} columns)",
                header.len()
            ));
        }
        return Ok(n - 1);
    }
    header
        .iter()
        .position(|c| c == name)
        .ok_or_else(|| format!("column '{name}' not found in the header row"))
}

const BLANK_LABEL: &str = "(blank)";

fn fmt_pct(part: f64, whole: f64) -> String {
    if whole <= 0.0 {
        "0.0%".to_string()
    } else {
        format!("{:.1}%", part / whole * 100.0)
    }
}

/// Pad `s` to `w` columns (left-justified).
fn padl(s: &str, w: usize) -> String {
    let len = s.chars().count();
    if len >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - len))
    }
}
/// Pad `s` to `w` columns (right-justified).
fn padr(s: &str, w: usize) -> String {
    let len = s.chars().count();
    if len >= w {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(w - len))
    }
}

fn parse_records(data: &str, delim: u8) -> Result<Vec<csv::StringRecord>, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    rdr.records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))
}

/// Public entry point. See module docs for `mode`/`percent`/`sort` values.
#[allow(clippy::too_many_arguments)]
pub fn tabulate(
    data: &str,
    mode: &str,
    question: &str,
    by: &str,
    percent: &str,
    include_blanks: bool,
    stats: bool,
    sort: &str,
    top: i64,
    delimiter: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty — paste the survey CSV (first row = question headers)".into());
    }
    let mode = if mode.trim().is_empty() { "overview" } else { mode.trim() };
    let sort = if sort.trim().is_empty() { "count" } else { sort.trim() };
    if !matches!(sort, "count" | "label") {
        return Err(format!("sort must be 'count' or 'label', got '{sort}'"));
    }
    let delim = delim_byte(delimiter)?;
    let records = parse_records(data, delim)?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    if records.len() < 2 {
        return Err("no data rows — need a header row plus at least one response".into());
    }
    let header = records[0].clone();
    let rows = &records[1..];

    match mode {
        "overview" => overview(&header, rows, question, include_blanks, sort, top),
        "crosstab" => crosstab(&header, rows, question, by, percent, include_blanks, stats, sort),
        other => Err(format!("mode must be 'overview' or 'crosstab', got '{other}'")),
    }
}

fn cell(rec: &csv::StringRecord, col: usize) -> &str {
    rec.get(col).unwrap_or("").trim()
}

/// Frequency table for one or every question column.
fn overview(
    header: &csv::StringRecord,
    rows: &[csv::StringRecord],
    question: &str,
    include_blanks: bool,
    sort: &str,
    top: i64,
) -> Result<String, String> {
    // Which columns to tabulate.
    let cols: Vec<usize> = if question.trim().is_empty() {
        (0..header.len()).collect()
    } else {
        vec![resolve(question, header)?]
    };
    if cols.is_empty() {
        return Err("no columns to tabulate".into());
    }

    let mut out = String::new();
    for (i, &col) in cols.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let qname = header.get(col).unwrap_or("").trim();
        let qname = if qname.is_empty() {
            format!("Column {}", col + 1)
        } else {
            qname.to_string()
        };

        let mut counts: HashMap<String, u64> = HashMap::new();
        let mut order: Vec<String> = Vec::new(); // first-seen order for stable tie-breaks
        let mut blank = 0u64;
        let mut answered = 0u64;
        for rec in rows {
            let v = cell(rec, col);
            if v.is_empty() {
                blank += 1;
                if !include_blanks {
                    continue;
                }
            }
            let key = if v.is_empty() {
                BLANK_LABEL.to_string()
            } else {
                v.to_string()
            };
            if !counts.contains_key(&key) {
                order.push(key.clone());
            }
            *counts.entry(key).or_insert(0) += 1;
            answered += 1;
        }

        let mut cats: Vec<(String, u64)> = order
            .into_iter()
            .map(|k| {
                let c = counts[&k];
                (k, c)
            })
            .collect();
        // Selection: top-N by frequency when requested.
        let total_cats = cats.len();
        if top > 0 && (top as usize) < cats.len() {
            cats.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            cats.truncate(top as usize);
        }
        // Display sort.
        match sort {
            "label" => cats.sort_by(|a, b| a.0.cmp(&b.0)),
            _ => cats.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0))),
        }

        let shown_note = if top > 0 && (top as usize) < total_cats {
            format!(", showing top {} of {}", top, total_cats)
        } else {
            String::new()
        };
        out.push_str(&format!(
            "{qname} — {answered} responses ({blank} blank, {total_cats} distinct{shown_note})\n"
        ));

        // Column widths.
        let lw = cats
            .iter()
            .map(|(l, _)| l.chars().count())
            .chain(std::iter::once("Value".len()))
            .max()
            .unwrap_or(5)
            .max(5);
        let cw = cats
            .iter()
            .map(|(_, c)| c.to_string().len())
            .chain(std::iter::once("Count".len()))
            .max()
            .unwrap_or(5)
            .max(5);
        out.push_str(&format!("  {}  {}  Percent\n", padl("Value", lw), padr("Count", cw)));
        for (label, count) in &cats {
            out.push_str(&format!(
                "  {}  {}  {}\n",
                padl(label, lw),
                padr(&count.to_string(), cw),
                fmt_pct(*count as f64, answered as f64)
            ));
        }
        out.push_str(&format!(
            "  {}  {}  100.0%\n",
            padl("Total", lw),
            padr(&answered.to_string(), cw)
        ));
    }
    Ok(out)
}

struct ChiSquare {
    chi2: f64,
    dof: u64,
    cramers_v: f64,
    p_value: f64,
}

/// Two-way contingency table.
#[allow(clippy::too_many_arguments)]
fn crosstab(
    header: &csv::StringRecord,
    rows: &[csv::StringRecord],
    question: &str,
    by: &str,
    percent: &str,
    include_blanks: bool,
    stats: bool,
    sort: &str,
) -> Result<String, String> {
    if question.trim().is_empty() {
        return Err("crosstab needs a 'question' column (the rows of the table)".into());
    }
    if by.trim().is_empty() {
        return Err("crosstab needs a 'by' column (the columns of the table)".into());
    }
    let percent = if percent.trim().is_empty() { "total" } else { percent.trim() };
    if !matches!(percent, "total" | "row" | "column" | "none") {
        return Err(format!(
            "percent must be 'total', 'row', 'column' or 'none', got '{percent}'"
        ));
    }
    let rcol = resolve(question, header)?;
    let ccol = resolve(by, header)?;
    if rcol == ccol {
        return Err("crosstab 'question' and 'by' must be different columns".into());
    }
    let rname = header.get(rcol).unwrap_or("").trim();
    let rname = if rname.is_empty() {
        format!("Column {}", rcol + 1)
    } else {
        rname.to_string()
    };
    let cname = header.get(ccol).unwrap_or("").trim();
    let cname = if cname.is_empty() {
        format!("Column {}", ccol + 1)
    } else {
        cname.to_string()
    };

    let mut row_order: Vec<String> = Vec::new();
    let mut col_order: Vec<String> = Vec::new();
    let mut cells: HashMap<(String, String), u64> = HashMap::new();
    let mut row_tot: HashMap<String, u64> = HashMap::new();
    let mut col_tot: HashMap<String, u64> = HashMap::new();
    let mut grand: u64 = 0;

    for rec in rows {
        let mut rv = cell(rec, rcol);
        let mut cv = cell(rec, ccol);
        if (rv.is_empty() || cv.is_empty()) && !include_blanks {
            continue;
        }
        if rv.is_empty() {
            rv = BLANK_LABEL;
        }
        if cv.is_empty() {
            cv = BLANK_LABEL;
        }
        let (rv, cv) = (rv.to_string(), cv.to_string());
        if !row_tot.contains_key(&rv) {
            row_order.push(rv.clone());
        }
        if !col_tot.contains_key(&cv) {
            col_order.push(cv.clone());
        }
        *cells.entry((rv.clone(), cv.clone())).or_insert(0) += 1;
        *row_tot.entry(rv).or_insert(0) += 1;
        *col_tot.entry(cv).or_insert(0) += 1;
        grand += 1;
    }
    if grand == 0 {
        return Err(
            "no complete rows to cross-tabulate (every row had a blank in one of the two columns)".into(),
        );
    }

    // Sort row/column labels.
    match sort {
        "label" => {
            row_order.sort();
            col_order.sort();
        }
        _ => {
            row_order.sort_by(|a, b| row_tot[b].cmp(&row_tot[a]).then(a.cmp(b)));
            col_order.sort_by(|a, b| col_tot[b].cmp(&col_tot[a]).then(a.cmp(b)));
        }
    }

    // Render a cell as "count" or "count (pct%)".
    let render = |c: u64, rt: u64, ct: u64| -> String {
        match percent {
            "none" => c.to_string(),
            "row" => format!("{c} ({})", fmt_pct(c as f64, rt as f64)),
            "column" => format!("{c} ({})", fmt_pct(c as f64, ct as f64)),
            _ => format!("{c} ({})", fmt_pct(c as f64, grand as f64)),
        }
    };

    // Build the grid of strings so we can size columns.
    let mut headers: Vec<String> = Vec::with_capacity(col_order.len() + 2);
    headers.push(format!("{rname} \\ {cname}"));
    headers.extend(col_order.iter().cloned());
    headers.push("Total".to_string());

    let mut grid: Vec<Vec<String>> = Vec::new();
    for rv in &row_order {
        let rt = row_tot[rv];
        let mut line = vec![rv.clone()];
        for cv in &col_order {
            let c = *cells.get(&(rv.clone(), cv.clone())).unwrap_or(&0);
            line.push(render(c, rt, col_tot[cv]));
        }
        line.push(rt.to_string());
        grid.push(line);
    }
    // Total row.
    let mut totline = vec!["Total".to_string()];
    for cv in &col_order {
        totline.push(col_tot[cv].to_string());
    }
    totline.push(grand.to_string());
    grid.push(totline);

    // Column widths across headers + grid.
    let ncols = headers.len();
    let mut widths = vec![0usize; ncols];
    for (i, h) in headers.iter().enumerate() {
        widths[i] = h.chars().count();
    }
    for line in &grid {
        for (i, c) in line.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }

    let mut out = String::new();
    let percent_note = match percent {
        "none" => "counts only",
        "row" => "cell = count (row %)",
        "column" => "cell = count (column %)",
        _ => "cell = count (% of all)",
    };
    out.push_str(&format!("Crosstab: {rname} × {cname} — n = {grand}, {percent_note}\n"));
    // Header line: first column left-justified, rest right-justified.
    let mut hl = format!("  {}", padl(&headers[0], widths[0]));
    for i in 1..ncols {
        hl.push_str(&format!("  {}", padr(&headers[i], widths[i])));
    }
    out.push_str(&hl);
    out.push('\n');
    for line in &grid {
        let mut l = format!("  {}", padl(&line[0], widths[0]));
        for i in 1..ncols {
            l.push_str(&format!("  {}", padr(&line[i], widths[i])));
        }
        out.push_str(&l);
        out.push('\n');
    }

    if stats {
        let cs = chi_square(&row_order, &col_order, &cells, &row_tot, &col_tot, grand);
        out.push('\n');
        out.push_str(&format!(
            "Chi-square = {:.4}, df = {}, Cramér's V = {:.4}, p = {:.4}\n",
            cs.chi2, cs.dof, cs.cramers_v, cs.p_value
        ));
    }
    Ok(out)
}

fn chi_square(
    row_order: &[String],
    col_order: &[String],
    cells: &HashMap<(String, String), u64>,
    row_tot: &HashMap<String, u64>,
    col_tot: &HashMap<String, u64>,
    grand: u64,
) -> ChiSquare {
    let r = row_order.len();
    let c = col_order.len();
    let n = grand as f64;
    let mut chi2 = 0.0;
    for rv in row_order {
        for cv in col_order {
            let observed = *cells.get(&(rv.clone(), cv.clone())).unwrap_or(&0) as f64;
            let expected = row_tot[rv] as f64 * col_tot[cv] as f64 / n;
            if expected > 0.0 {
                chi2 += (observed - expected).powi(2) / expected;
            }
        }
    }
    let dof = (r.saturating_sub(1) * c.saturating_sub(1)) as u64;
    let k = r.min(c).saturating_sub(1);
    let cramers_v = if k == 0 || n == 0.0 {
        0.0
    } else {
        (chi2 / (n * k as f64)).sqrt()
    };
    let p_value = if dof == 0 { 1.0 } else { chi_sq_pvalue(chi2, dof as f64) };
    ChiSquare {
        chi2,
        dof,
        cramers_v,
        p_value,
    }
}

/// Upper-tail p-value of a chi-square statistic = Q(dof/2, chi2/2), the regularized
/// upper incomplete gamma function (Numerical Recipes gser/gcf split).
fn chi_sq_pvalue(chi2: f64, dof: f64) -> f64 {
    if chi2 <= 0.0 {
        return 1.0;
    }
    gammq(dof / 2.0, chi2 / 2.0)
}

fn gammln(xx: f64) -> f64 {
    const COF: [f64; 6] = [
        76.18009172947146,
        -86.50532032941677,
        24.01409824083091,
        -1.231739572450155,
        0.1208650973866179e-2,
        -0.5395239384953e-5,
    ];
    let x = xx;
    let mut y = xx;
    let tmp = x + 5.5 - (x + 0.5) * (x + 5.5).ln();
    let mut ser = 1.000000000190015;
    for c in COF.iter() {
        y += 1.0;
        ser += c / y;
    }
    -tmp + (2.5066282746310005 * ser / x).ln()
}

/// Regularized upper incomplete gamma Q(a,x) = 1 - P(a,x).
fn gammq(a: f64, x: f64) -> f64 {
    if x < 0.0 || a <= 0.0 {
        return 1.0;
    }
    if x < a + 1.0 {
        1.0 - gser(a, x)
    } else {
        gcf(a, x)
    }
}

/// Series representation of P(a,x) for x < a+1.
fn gser(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let gln = gammln(a);
    let mut ap = a;
    let mut sum = 1.0 / a;
    let mut del = sum;
    for _ in 0..500 {
        ap += 1.0;
        del *= x / ap;
        sum += del;
        if del.abs() < sum.abs() * 1e-14 {
            break;
        }
    }
    sum * (-x + a * x.ln() - gln).exp()
}

/// Continued-fraction representation of Q(a,x) for x >= a+1.
fn gcf(a: f64, x: f64) -> f64 {
    let gln = gammln(a);
    let fpmin = 1e-300;
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / fpmin;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..500 {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < fpmin {
            d = fpmin;
        }
        c = b + an / c;
        if c.abs() < fpmin {
            c = fpmin;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-14 {
            break;
        }
    }
    (-x + a * x.ln() - gln).exp() * h
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURVEY: &str = "\
gender,satisfaction,region
F,High,North
M,Low,South
F,High,North
F,Medium,North
M,High,South
M,Low,South
F,,North";

    #[test]
    fn overview_single_question() {
        let out = tabulate(SURVEY, "overview", "satisfaction", "", "", false, false, "count", 0, ",").unwrap();
        // 7 rows, one blank satisfaction → 6 answered.
        assert!(out.contains("satisfaction — 6 responses (1 blank, 3 distinct)"));
        assert!(out.contains("High"));
        // High = 3 of 6 = 50.0%
        assert!(out.contains("50.0%"), "output was:\n{out}");
        assert!(out.contains("Total"));
    }

    #[test]
    fn overview_all_questions() {
        let out = tabulate(SURVEY, "overview", "", "", "", false, false, "count", 0, ",").unwrap();
        assert!(out.contains("gender —"));
        assert!(out.contains("satisfaction —"));
        assert!(out.contains("region —"));
    }

    #[test]
    fn overview_include_blanks() {
        let out = tabulate(SURVEY, "overview", "satisfaction", "", "", true, false, "label", 0, ",").unwrap();
        assert!(out.contains("(blank)"), "output was:\n{out}");
        assert!(out.contains("satisfaction — 7 responses"));
    }

    #[test]
    fn crosstab_counts() {
        let out = tabulate(SURVEY, "crosstab", "gender", "satisfaction", "none", false, false, "label", 0, ",").unwrap();
        assert!(out.contains("Crosstab: gender × satisfaction"));
        assert!(out.contains("gender \\ satisfaction"));
        assert!(out.contains("Total"));
    }

    #[test]
    fn crosstab_stats() {
        let out = tabulate(SURVEY, "crosstab", "gender", "region", "total", false, true, "label", 0, ",").unwrap();
        assert!(out.contains("Chi-square ="));
        assert!(out.contains("Cramér's V ="));
        assert!(out.contains("p ="));
    }

    #[test]
    fn pvalue_known_vector() {
        // chi2 = 3.841, df = 1 → p ≈ 0.05 (critical value at α = 0.05).
        let p = chi_sq_pvalue(3.841, 1.0);
        assert!((p - 0.05).abs() < 0.001, "expected ~0.05, got {p}");
        // df = 2, chi2 = 5.991 → p ≈ 0.05
        let p2 = chi_sq_pvalue(5.991, 2.0);
        assert!((p2 - 0.05).abs() < 0.001, "expected ~0.05, got {p2}");
        // chi2 = 0 → p = 1
        assert_eq!(chi_sq_pvalue(0.0, 3.0), 1.0);
    }

    #[test]
    fn err_empty() {
        assert!(tabulate("   ", "overview", "", "", "", false, false, "count", 0, ",").is_err());
    }

    #[test]
    fn err_no_data_rows() {
        assert!(tabulate("a,b,c", "overview", "", "", "", false, false, "count", 0, ",").is_err());
    }

    #[test]
    fn err_unknown_column() {
        assert!(tabulate(SURVEY, "overview", "nope", "", "", false, false, "count", 0, ",").is_err());
    }

    #[test]
    fn err_bad_mode() {
        assert!(tabulate(SURVEY, "sideways", "", "", "", false, false, "count", 0, ",").is_err());
    }

    #[test]
    fn err_crosstab_same_column() {
        assert!(tabulate(SURVEY, "crosstab", "gender", "gender", "total", false, false, "count", 0, ",").is_err());
    }

    #[test]
    fn top_n_limits_categories() {
        let d = "q\na\na\nb\nc\nd";
        let out = tabulate(d, "overview", "q", "", "", false, false, "count", 2, ",").unwrap();
        assert!(out.contains("showing top 2 of 4"), "output was:\n{out}");
    }
}
