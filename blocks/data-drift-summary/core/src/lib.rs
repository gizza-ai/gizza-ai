//! data-drift-summary core — per-column drift report between two tabular datasets.
//!
//! Compares a `reference` (baseline) CSV against a `current` (new) CSV column by
//! column and reports, for every column present in both:
//!
//! * the inferred type on each side (and whether it changed),
//! * the null rate on each side and its delta,
//! * the distinct-value count (cardinality) on each side,
//! * the numeric range (min–max) and mean for numeric columns,
//! * the category values that are **new** in the current data and the ones that
//!   went **missing**, for categorical columns,
//! * a distribution **drift score** — Population Stability Index (default) or
//!   Jensen–Shannon distance — plus a per-column drifted flag.
//!
//! Columns that exist on only one side are reported as schema drift. A
//! dataset-level verdict fires when the share of drifted columns reaches
//! `drift_share`.
//!
//! Pure Rust, no I/O — shared by the chat block, the CLI and the web page.

use serde_json::json;
use std::collections::HashMap;

/// Hard cap on rows per side, so a runaway paste can't wedge the browser tab.
pub const MAX_ROWS: usize = 100_000;
/// How many new/missing category values are listed before an overflow marker.
const MAX_LISTED_CATEGORIES: usize = 8;
/// Share floor used for PSI so an empty bucket doesn't blow up to infinity.
const PSI_EPSILON: f64 = 1e-6;

/// Map a delimiter name/char to its byte, mirroring the other csv-* blocks.
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
                    "delimiter must be a single char or one of comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// A parsed dataset: column labels + data rows.
struct Table {
    cols: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// Parse CSV text. With `header` the first record supplies the column labels;
/// without it labels are positional (`col1`, `col2`, …). Ragged rows are allowed
/// (missing trailing cells read as empty).
fn parse(data: &str, delim: u8, header: bool, side: &str) -> Result<Table, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let recs: Vec<Vec<String>> = rdr
        .records()
        .map(|r| r.map(|rec| rec.iter().map(|c| c.to_string()).collect()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{side} dataset parse error: {e}"))?;
    if recs.is_empty() {
        return Err(format!("{side} dataset is empty — paste at least a header row and one data row"));
    }
    let table = if header {
        let mut it = recs.into_iter();
        let cols = it.next().unwrap();
        if cols.iter().all(|c| c.trim().is_empty()) {
            return Err(format!("{side} dataset header row is empty"));
        }
        Table {
            cols,
            rows: it.collect(),
        }
    } else {
        let width = recs.iter().map(|r| r.len()).max().unwrap_or(0);
        Table {
            cols: (1..=width).map(|n| format!("col{n}")).collect(),
            rows: recs,
        }
    };
    if table.rows.len() > MAX_ROWS {
        return Err(format!(
            "{side} dataset has {} data rows, which is over the {MAX_ROWS} row limit",
            table.rows.len()
        ));
    }
    Ok(table)
}

/// Is this cell a null? Blank/whitespace-only, or one of the conventional
/// missing-value tokens exports use.
fn is_null(v: &str) -> bool {
    let t = v.trim();
    if t.is_empty() {
        return true;
    }
    matches!(
        t.to_ascii_lowercase().as_str(),
        "na" | "n/a" | "null" | "nan" | "none" | "nil" | "-"
    )
}

/// Inferred column type, checked in this order so `0`/`1` stay integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColType {
    Empty,
    Bool,
    Int,
    Number,
    Date,
    Text,
}

impl ColType {
    fn label(self) -> &'static str {
        match self {
            ColType::Empty => "empty",
            ColType::Bool => "bool",
            ColType::Int => "int",
            ColType::Number => "number",
            ColType::Date => "date",
            ColType::Text => "string",
        }
    }
    fn is_numeric(self) -> bool {
        matches!(self, ColType::Int | ColType::Number)
    }
}

fn looks_bool(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no" | "y" | "n" | "t" | "f"
    )
}

/// `YYYY-MM-DD` or `YYYY/MM/DD`, optionally followed by a time part.
fn looks_date(v: &str) -> bool {
    let t = v.trim();
    let head: &str = t.split(['T', ' ']).next().unwrap_or("");
    let b = head.as_bytes();
    if b.len() != 10 {
        return false;
    }
    let sep = b[4];
    if (sep != b'-' && sep != b'/') || b[7] != sep {
        return false;
    }
    b.iter()
        .enumerate()
        .all(|(i, c)| if i == 4 || i == 7 { true } else { c.is_ascii_digit() })
}

fn infer_type(values: &[&str]) -> ColType {
    if values.is_empty() {
        return ColType::Empty;
    }
    if values.iter().all(|v| looks_bool(v)) {
        return ColType::Bool;
    }
    if values.iter().all(|v| v.trim().parse::<i64>().is_ok()) {
        return ColType::Int;
    }
    if values
        .iter()
        .all(|v| v.trim().parse::<f64>().map(|f| f.is_finite()).unwrap_or(false))
    {
        return ColType::Number;
    }
    if values.iter().all(|v| looks_date(v)) {
        return ColType::Date;
    }
    ColType::Text
}

/// Per-column facts for one side of the comparison.
struct Profile {
    rows: usize,
    nulls: usize,
    ctype: ColType,
    /// Distinct non-null values (after case/whitespace folding when enabled).
    distinct: usize,
    /// Parsed numeric values, ascending — only for numeric columns.
    numeric: Vec<f64>,
    numeric_mean: Option<f64>,
    /// Counts per folded category key, with the first-seen display text.
    cats: HashMap<String, (String, usize)>,
}

fn fold(v: &str, ignore_case: bool) -> String {
    if ignore_case {
        v.trim().to_ascii_lowercase()
    } else {
        v.to_string()
    }
}

fn profile(values: &[&str], ignore_case: bool) -> Profile {
    let rows = values.len();
    let non_null: Vec<&str> = values.iter().copied().filter(|v| !is_null(v)).collect();
    let nulls = rows - non_null.len();
    let ctype = infer_type(&non_null);

    let mut cats: HashMap<String, (String, usize)> = HashMap::new();
    for v in &non_null {
        let key = fold(v, ignore_case);
        let e = cats.entry(key).or_insert_with(|| (v.to_string(), 0));
        e.1 += 1;
    }

    let mut numeric: Vec<f64> = if ctype.is_numeric() {
        non_null
            .iter()
            .filter_map(|v| v.trim().parse::<f64>().ok())
            .filter(|f| f.is_finite())
            .collect()
    } else {
        Vec::new()
    };
    numeric.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let numeric_mean = if numeric.is_empty() {
        None
    } else {
        Some(numeric.iter().sum::<f64>() / numeric.len() as f64)
    };

    Profile {
        rows,
        nulls,
        ctype,
        distinct: cats.len(),
        numeric,
        numeric_mean,
        cats,
    }
}

/// Interior bin edges at equal-population quantiles of the (sorted) reference
/// values, deduplicated so ties collapse instead of producing empty bins.
fn quantile_edges(sorted: &[f64], bins: usize) -> Vec<f64> {
    let n = sorted.len();
    let mut edges: Vec<f64> = Vec::new();
    if n == 0 || bins < 2 {
        return edges;
    }
    for i in 1..bins {
        let pos = (i as f64) * (n as f64) / (bins as f64);
        let idx = (pos.floor() as usize).min(n - 1);
        let v = sorted[idx];
        if edges.last().map_or(true, |&last| v > last) {
            edges.push(v);
        }
    }
    edges
}

fn bin_counts(values: &[f64], edges: &[f64]) -> Vec<f64> {
    let mut counts = vec![0.0; edges.len() + 1];
    for &v in values {
        counts[edges.partition_point(|&e| e <= v)] += 1.0;
    }
    counts
}

fn shares(counts: &[f64]) -> Vec<f64> {
    let total: f64 = counts.iter().sum();
    if total <= 0.0 {
        return vec![0.0; counts.len()];
    }
    counts.iter().map(|c| c / total).collect()
}

/// Population Stability Index: Σ (q − p) · ln(q / p), with a share floor so an
/// empty bucket contributes a large-but-finite amount instead of infinity.
fn psi(p: &[f64], q: &[f64]) -> f64 {
    p.iter()
        .zip(q)
        .map(|(&pi, &qi)| {
            let pi = pi.max(PSI_EPSILON);
            let qi = qi.max(PSI_EPSILON);
            (qi - pi) * (qi / pi).ln()
        })
        .sum()
}

/// Jensen–Shannon distance (base-2 divergence, square-rooted) — always in 0..=1.
fn jsd(p: &[f64], q: &[f64]) -> f64 {
    fn kl(a: &[f64], m: &[f64]) -> f64 {
        a.iter()
            .zip(m)
            .map(|(&ai, &mi)| if ai > 0.0 && mi > 0.0 { ai * (ai / mi).log2() } else { 0.0 })
            .sum()
    }
    let m: Vec<f64> = p.iter().zip(q).map(|(&pi, &qi)| (pi + qi) / 2.0).collect();
    let div = 0.5 * kl(p, &m) + 0.5 * kl(q, &m);
    div.max(0.0).sqrt().min(1.0)
}

fn score(method: &str, p: &[f64], q: &[f64]) -> f64 {
    match method {
        "jsd" => jsd(p, q),
        _ => psi(p, q),
    }
}

/// Format a float without a trailing `.0` and without scientific noise.
fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "n/a".to_string();
    }
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

fn fmt_pct(v: f64) -> String {
    format!("{v:.1}%")
}

fn fmt_score(v: Option<f64>) -> String {
    match v {
        Some(s) => format!("{s:.4}"),
        None => "n/a".to_string(),
    }
}

/// The finished per-column comparison.
struct ColumnReport {
    name: String,
    order: usize,
    mode: &'static str,
    score: Option<f64>,
    drifted: bool,
    reference: Profile,
    current: Profile,
    new_cats: Vec<(String, usize)>,
    missing_cats: Vec<(String, usize)>,
}

impl ColumnReport {
    fn type_changed(&self) -> bool {
        self.reference.ctype != self.current.ctype
            && self.reference.ctype != ColType::Empty
            && self.current.ctype != ColType::Empty
    }
    fn null_pct(p: &Profile) -> f64 {
        if p.rows == 0 {
            0.0
        } else {
            p.nulls as f64 * 100.0 / p.rows as f64
        }
    }
    fn range(p: &Profile) -> Option<(f64, f64)> {
        match (p.numeric.first(), p.numeric.last()) {
            (Some(&lo), Some(&hi)) => Some((lo, hi)),
            _ => None,
        }
    }
}

fn column_values<'a>(table: &'a Table, idx: usize) -> Vec<&'a str> {
    table
        .rows
        .iter()
        .map(|r| r.get(idx).map(|s| s.as_str()).unwrap_or(""))
        .collect()
}

/// Sort category buckets worst-first: biggest count, then value, and cap the
/// listing with an overflow marker handled by the renderers.
fn rank_cats(mut v: Vec<(String, usize)>) -> Vec<(String, usize)> {
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

fn cats_line(list: &[(String, usize)]) -> String {
    let shown: Vec<String> = list
        .iter()
        .take(MAX_LISTED_CATEGORIES)
        .map(|(v, c)| format!("{v} ({c})"))
        .collect();
    let mut s = shown.join(", ");
    if list.len() > MAX_LISTED_CATEGORIES {
        s.push_str(&format!(" (+{} more)", list.len() - MAX_LISTED_CATEGORIES));
    }
    s
}

/// Compare two datasets and render the drift report.
#[allow(clippy::too_many_arguments)]
pub fn run(
    reference: &str,
    current: &str,
    delimiter: &str,
    header: bool,
    columns: &str,
    method: &str,
    threshold: f64,
    bins: usize,
    max_categories: usize,
    drift_share: f64,
    ignore_case: bool,
    sort: &str,
    format: &str,
) -> Result<String, String> {
    let method = match method.trim().to_ascii_lowercase().as_str() {
        "" | "psi" => "psi",
        "jsd" | "js" | "jensen-shannon" => "jsd",
        other => {
            return Err(format!(
                "method must be psi or jsd, got '{other}'"
            ))
        }
    };
    let sort = match sort.trim().to_ascii_lowercase().as_str() {
        "" | "drift" => "drift",
        "name" => "name",
        "order" => "order",
        other => return Err(format!("sort must be drift, name or order, got '{other}'")),
    };
    let format = match format.trim().to_ascii_lowercase().as_str() {
        "" | "table" => "table",
        "markdown" | "md" => "markdown",
        "json" => "json",
        "csv" => "csv",
        other => {
            return Err(format!(
                "format must be table, markdown, json or csv, got '{other}'"
            ))
        }
    };
    if !(threshold.is_finite() && threshold >= 0.0) {
        return Err(format!("threshold must be 0 or greater, got {threshold}"));
    }
    if !(drift_share.is_finite() && (0.0..=1.0).contains(&drift_share)) {
        return Err(format!(
            "drift_share must be between 0 and 1, got {drift_share}"
        ));
    }
    if !(2..=50).contains(&bins) {
        return Err(format!("bins must be between 2 and 50, got {bins}"));
    }
    if !(2..=1000).contains(&max_categories) {
        return Err(format!(
            "max_categories must be between 2 and 1000, got {max_categories}"
        ));
    }

    let delim = delim_byte(delimiter)?;
    let ref_t = parse(reference, delim, header, "reference")?;
    let cur_t = parse(current, delim, header, "current")?;

    // Column alignment: by header name, else positionally.
    let mut cur_index: HashMap<&str, usize> = HashMap::new();
    for (i, c) in cur_t.cols.iter().enumerate() {
        cur_index.entry(c.as_str()).or_insert(i);
    }
    let mut common: Vec<(String, usize, usize)> = Vec::new();
    let mut removed: Vec<(String, ColType)> = Vec::new();
    for (i, c) in ref_t.cols.iter().enumerate() {
        match cur_index.get(c.as_str()) {
            Some(&j) => common.push((c.clone(), i, j)),
            None => {
                let vals = column_values(&ref_t, i);
                let non_null: Vec<&str> = vals.iter().copied().filter(|v| !is_null(v)).collect();
                removed.push((c.clone(), infer_type(&non_null)));
            }
        }
    }
    let ref_names: Vec<&str> = ref_t.cols.iter().map(|s| s.as_str()).collect();
    let mut added: Vec<(String, ColType)> = Vec::new();
    for (j, c) in cur_t.cols.iter().enumerate() {
        if !ref_names.contains(&c.as_str()) {
            let vals = column_values(&cur_t, j);
            let non_null: Vec<&str> = vals.iter().copied().filter(|v| !is_null(v)).collect();
            added.push((c.clone(), infer_type(&non_null)));
        }
    }

    // Optional allow-list.
    let wanted: Vec<String> = columns
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if !wanted.is_empty() {
        let available: Vec<String> = common.iter().map(|(n, _, _)| n.clone()).collect();
        for w in &wanted {
            if !available.iter().any(|a| a == w) {
                return Err(format!(
                    "column '{w}' is not present in both datasets; comparable columns are [{}]",
                    if available.is_empty() {
                        "none".to_string()
                    } else {
                        available.join(", ")
                    }
                ));
            }
        }
        common.retain(|(n, _, _)| wanted.iter().any(|w| w == n));
    }

    if common.is_empty() {
        return Err(format!(
            "the two datasets share no columns to compare — reference has [{}] and current has [{}]",
            ref_t.cols.join(", "),
            cur_t.cols.join(", ")
        ));
    }

    // Per-column comparison.
    let mut reports: Vec<ColumnReport> = Vec::new();
    for (order, (name, i, j)) in common.iter().enumerate() {
        let rp = profile(&column_values(&ref_t, *i), ignore_case);
        let cp = profile(&column_values(&cur_t, *j), ignore_case);

        let numeric_mode = rp.ctype.is_numeric()
            && cp.ctype.is_numeric()
            && rp.distinct > max_categories;

        let (mode, sc) = if numeric_mode {
            if rp.numeric.is_empty() || cp.numeric.is_empty() {
                ("numeric", None)
            } else {
                let edges = quantile_edges(&rp.numeric, bins);
                let p = shares(&bin_counts(&rp.numeric, &edges));
                let q = shares(&bin_counts(&cp.numeric, &edges));
                ("numeric", Some(score(method, &p, &q)))
            }
        } else if rp.cats.is_empty() || cp.cats.is_empty() {
            ("categorical", None)
        } else {
            let mut keys: Vec<&String> = rp.cats.keys().collect();
            for k in cp.cats.keys() {
                if !rp.cats.contains_key(k) {
                    keys.push(k);
                }
            }
            keys.sort();
            let pc: Vec<f64> = keys
                .iter()
                .map(|k| rp.cats.get(*k).map(|e| e.1 as f64).unwrap_or(0.0))
                .collect();
            let qc: Vec<f64> = keys
                .iter()
                .map(|k| cp.cats.get(*k).map(|e| e.1 as f64).unwrap_or(0.0))
                .collect();
            ("categorical", Some(score(method, &shares(&pc), &shares(&qc))))
        };

        let new_cats = rank_cats(
            cp.cats
                .iter()
                .filter(|(k, _)| !rp.cats.contains_key(*k))
                .map(|(_, (disp, c))| (disp.clone(), *c))
                .collect(),
        );
        let missing_cats = rank_cats(
            rp.cats
                .iter()
                .filter(|(k, _)| !cp.cats.contains_key(*k))
                .map(|(_, (disp, c))| (disp.clone(), *c))
                .collect(),
        );

        reports.push(ColumnReport {
            name: name.clone(),
            order,
            mode,
            score: sc,
            drifted: sc.map(|s| s >= threshold).unwrap_or(false),
            reference: rp,
            current: cp,
            new_cats,
            missing_cats,
        });
    }

    match sort {
        "name" => reports.sort_by(|a, b| a.name.cmp(&b.name)),
        "order" => reports.sort_by_key(|r| r.order),
        _ => reports.sort_by(|a, b| {
            b.score
                .unwrap_or(-1.0)
                .partial_cmp(&a.score.unwrap_or(-1.0))
                .unwrap()
                .then_with(|| a.order.cmp(&b.order))
        }),
    }

    let compared = reports.len();
    let drifted = reports.iter().filter(|r| r.drifted).count();
    let share = drifted as f64 / compared as f64;
    let dataset_drift = share >= drift_share;

    let summary = Summary {
        ref_rows: ref_t.rows.len(),
        ref_cols: ref_t.cols.len(),
        cur_rows: cur_t.rows.len(),
        cur_cols: cur_t.cols.len(),
        method,
        threshold,
        drift_share,
        compared,
        drifted,
        share,
        dataset_drift,
    };

    Ok(match format {
        "markdown" => render_markdown(&summary, &reports, &added, &removed),
        "json" => render_json(&summary, &reports, &added, &removed),
        "csv" => render_csv(&reports, &added, &removed),
        _ => render_table(&summary, &reports, &added, &removed),
    })
}

struct Summary {
    ref_rows: usize,
    ref_cols: usize,
    cur_rows: usize,
    cur_cols: usize,
    method: &'static str,
    threshold: f64,
    drift_share: f64,
    compared: usize,
    drifted: usize,
    share: f64,
    dataset_drift: bool,
}

fn verdict(s: &Summary) -> &'static str {
    if s.dataset_drift {
        "DATASET DRIFT DETECTED"
    } else {
        "no dataset drift"
    }
}

fn render_table(
    s: &Summary,
    reports: &[ColumnReport],
    added: &[(String, ColType)],
    removed: &[(String, ColType)],
) -> String {
    let mut out = String::from("DATA DRIFT SUMMARY\n");
    out.push_str(&format!(
        "reference: {} rows × {} columns · current: {} rows × {} columns\n",
        s.ref_rows, s.ref_cols, s.cur_rows, s.cur_cols
    ));
    out.push_str(&format!(
        "method: {} · threshold: {:.2} · drift share: {:.2}\n",
        s.method, s.threshold, s.drift_share
    ));
    out.push_str(&format!(
        "{} columns compared · {} drifted ({}) · {}\n",
        s.compared,
        s.drifted,
        fmt_pct(s.share * 100.0),
        verdict(s)
    ));

    for r in reports {
        out.push('\n');
        out.push_str(&format!(
            "{}  {} — {} · {} {}\n",
            if r.drifted { "DRIFT" } else { "ok   " },
            r.name,
            r.mode,
            s.method,
            fmt_score(r.score)
        ));
        out.push_str(&format!(
            "  type      {} → {}{}\n",
            r.reference.ctype.label(),
            r.current.ctype.label(),
            if r.type_changed() { "  (type changed)" } else { "" }
        ));
        let nr = ColumnReport::null_pct(&r.reference);
        let nc = ColumnReport::null_pct(&r.current);
        out.push_str(&format!(
            "  nulls     {} → {} ({}{})\n",
            fmt_pct(nr),
            fmt_pct(nc),
            if nc - nr >= 0.0 { "+" } else { "" },
            format!("{:.1}", nc - nr)
        ));
        out.push_str(&format!(
            "  distinct  {} → {}\n",
            r.reference.distinct, r.current.distinct
        ));
        if let (Some((lo1, hi1)), Some((lo2, hi2))) = (
            ColumnReport::range(&r.reference),
            ColumnReport::range(&r.current),
        ) {
            out.push_str(&format!(
                "  range     {} – {} → {} – {}\n",
                fmt_num(lo1),
                fmt_num(hi1),
                fmt_num(lo2),
                fmt_num(hi2)
            ));
        }
        if let (Some(m1), Some(m2)) = (r.reference.numeric_mean, r.current.numeric_mean) {
            out.push_str(&format!("  mean      {} → {}\n", fmt_num(m1), fmt_num(m2)));
        }
        if !r.new_cats.is_empty() {
            out.push_str(&format!("  new       {}\n", cats_line(&r.new_cats)));
        }
        if !r.missing_cats.is_empty() {
            out.push_str(&format!("  missing   {}\n", cats_line(&r.missing_cats)));
        }
    }

    if !added.is_empty() || !removed.is_empty() {
        out.push_str("\nSCHEMA DRIFT\n");
        for (n, t) in added {
            out.push_str(&format!("  + {n}  added in current ({})\n", t.label()));
        }
        for (n, t) in removed {
            out.push_str(&format!("  - {n}  removed from current ({})\n", t.label()));
        }
    }
    out
}

fn render_markdown(
    s: &Summary,
    reports: &[ColumnReport],
    added: &[(String, ColType)],
    removed: &[(String, ColType)],
) -> String {
    let mut out = String::from("# Data drift summary\n\n");
    out.push_str(&format!(
        "- Reference: **{} rows × {} columns** · Current: **{} rows × {} columns**\n",
        s.ref_rows, s.ref_cols, s.cur_rows, s.cur_cols
    ));
    out.push_str(&format!(
        "- Method: `{}` · threshold `{:.2}` · drift share `{:.2}`\n",
        s.method, s.threshold, s.drift_share
    ));
    out.push_str(&format!(
        "- **{}** — {} of {} compared columns drifted ({})\n\n",
        verdict(s),
        s.drifted,
        s.compared,
        fmt_pct(s.share * 100.0)
    ));
    out.push_str("| Column | Drift | Score | Type | Nulls | Distinct | Range | New / missing |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for r in reports {
        let ty = if r.type_changed() {
            format!(
                "{} → {} ⚠",
                r.reference.ctype.label(),
                r.current.ctype.label()
            )
        } else {
            r.reference.ctype.label().to_string()
        };
        let range = match (
            ColumnReport::range(&r.reference),
            ColumnReport::range(&r.current),
        ) {
            (Some((a, b)), Some((c, d))) => format!(
                "{}–{} → {}–{}",
                fmt_num(a),
                fmt_num(b),
                fmt_num(c),
                fmt_num(d)
            ),
            _ => "—".to_string(),
        };
        let mut cats = Vec::new();
        if !r.new_cats.is_empty() {
            cats.push(format!("new: {}", cats_line(&r.new_cats)));
        }
        if !r.missing_cats.is_empty() {
            cats.push(format!("missing: {}", cats_line(&r.missing_cats)));
        }
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} → {} | {} → {} | {} | {} |\n",
            r.name,
            if r.drifted { "**yes**" } else { "no" },
            fmt_score(r.score),
            ty,
            fmt_pct(ColumnReport::null_pct(&r.reference)),
            fmt_pct(ColumnReport::null_pct(&r.current)),
            r.reference.distinct,
            r.current.distinct,
            range,
            if cats.is_empty() {
                "—".to_string()
            } else {
                cats.join("; ")
            }
        ));
    }
    if !added.is_empty() || !removed.is_empty() {
        out.push_str("\n## Schema drift\n\n");
        for (n, t) in added {
            out.push_str(&format!("- `{n}` added in current ({})\n", t.label()));
        }
        for (n, t) in removed {
            out.push_str(&format!("- `{n}` removed from current ({})\n", t.label()));
        }
    }
    out
}

fn render_json(
    s: &Summary,
    reports: &[ColumnReport],
    added: &[(String, ColType)],
    removed: &[(String, ColType)],
) -> String {
    let cols: Vec<serde_json::Value> = reports
        .iter()
        .map(|r| {
            json!({
                "column": r.name,
                "mode": r.mode,
                "drift_score": r.score,
                "drifted": r.drifted,
                "type_changed": r.type_changed(),
                "reference": {
                    "rows": r.reference.rows,
                    "type": r.reference.ctype.label(),
                    "nulls": r.reference.nulls,
                    "null_pct": (ColumnReport::null_pct(&r.reference) * 10.0).round() / 10.0,
                    "distinct": r.reference.distinct,
                    "min": ColumnReport::range(&r.reference).map(|(lo, _)| lo),
                    "max": ColumnReport::range(&r.reference).map(|(_, hi)| hi),
                    "mean": r.reference.numeric_mean,
                },
                "current": {
                    "rows": r.current.rows,
                    "type": r.current.ctype.label(),
                    "nulls": r.current.nulls,
                    "null_pct": (ColumnReport::null_pct(&r.current) * 10.0).round() / 10.0,
                    "distinct": r.current.distinct,
                    "min": ColumnReport::range(&r.current).map(|(lo, _)| lo),
                    "max": ColumnReport::range(&r.current).map(|(_, hi)| hi),
                    "mean": r.current.numeric_mean,
                },
                "new_categories": r.new_cats.iter().map(|(v, c)| json!({"value": v, "count": c})).collect::<Vec<_>>(),
                "missing_categories": r.missing_cats.iter().map(|(v, c)| json!({"value": v, "count": c})).collect::<Vec<_>>(),
            })
        })
        .collect();
    let v = json!({
        "summary": {
            "reference_rows": s.ref_rows,
            "reference_columns": s.ref_cols,
            "current_rows": s.cur_rows,
            "current_columns": s.cur_cols,
            "method": s.method,
            "threshold": s.threshold,
            "drift_share": s.drift_share,
            "columns_compared": s.compared,
            "columns_drifted": s.drifted,
            "drifted_share": (s.share * 10000.0).round() / 10000.0,
            "dataset_drift": s.dataset_drift,
        },
        "columns": cols,
        "schema_drift": {
            "added": added.iter().map(|(n, t)| json!({"column": n, "type": t.label()})).collect::<Vec<_>>(),
            "removed": removed.iter().map(|(n, t)| json!({"column": n, "type": t.label()})).collect::<Vec<_>>(),
        }
    });
    serde_json::to_string_pretty(&v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn csv_escape(s: &str) -> String {
    if s.contains(['"', ',', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(
    reports: &[ColumnReport],
    added: &[(String, ColType)],
    removed: &[(String, ColType)],
) -> String {
    let mut out = String::from(
        "column,status,drift_score,drifted,type_reference,type_current,null_pct_reference,null_pct_current,distinct_reference,distinct_current,min_reference,max_reference,min_current,max_current,new_categories,missing_categories\n",
    );
    let range_or_blank = |p: &Profile, hi: bool| -> String {
        match ColumnReport::range(p) {
            Some((lo, h)) => fmt_num(if hi { h } else { lo }),
            None => String::new(),
        }
    };
    for r in reports {
        out.push_str(&format!(
            "{},compared,{},{},{},{},{:.1},{:.1},{},{},{},{},{},{},{},{}\n",
            csv_escape(&r.name),
            fmt_score(r.score),
            r.drifted,
            r.reference.ctype.label(),
            r.current.ctype.label(),
            ColumnReport::null_pct(&r.reference),
            ColumnReport::null_pct(&r.current),
            r.reference.distinct,
            r.current.distinct,
            range_or_blank(&r.reference, false),
            range_or_blank(&r.reference, true),
            range_or_blank(&r.current, false),
            range_or_blank(&r.current, true),
            csv_escape(&cats_line(&r.new_cats)),
            csv_escape(&cats_line(&r.missing_cats)),
        ));
    }
    for (n, t) in added {
        out.push_str(&format!(
            "{},added,,,,{},,,,,,,,,,\n",
            csv_escape(n),
            t.label()
        ));
    }
    for (n, t) in removed {
        out.push_str(&format!(
            "{},removed,,{},,,,,,,,,,,,\n",
            csv_escape(n),
            t.label()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const REF: &str = "id,amount,country\n1,10,US\n2,20,US\n3,30,FR\n4,40,FR\n";
    const CUR: &str = "id,amount,country\n1,10,US\n2,20,US\n3,30,DE\n4,,DE\n";

    fn go(reference: &str, current: &str, format: &str) -> Result<String, String> {
        run(
            reference, current, "comma", true, "", "psi", 0.2, 10, 20, 0.5, false, "drift", format,
        )
    }

    #[test]
    fn happy_path_reports_nulls_categories_and_verdict() {
        let out = go(REF, CUR, "table").unwrap();
        assert!(out.contains("reference: 4 rows × 3 columns · current: 4 rows × 3 columns"));
        assert!(out.contains("3 columns compared"));
        // country lost FR and gained DE.
        assert!(out.contains("new       DE (2)"), "{out}");
        assert!(out.contains("missing   FR (2)"), "{out}");
        // amount picked up a null in the current data.
        assert!(out.contains("nulls     0.0% → 25.0% (+25.0)"), "{out}");
        assert!(out.contains("DRIFT  country"), "{out}");
    }

    #[test]
    fn error_on_no_shared_columns() {
        let err = go("a,b\n1,2\n", "c,d\n3,4\n", "table").unwrap_err();
        assert!(
            err.contains("share no columns to compare"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn error_on_empty_reference() {
        let err = go("", "a\n1\n", "table").unwrap_err();
        assert!(err.contains("reference dataset is empty"), "{err}");
    }

    #[test]
    fn error_on_unknown_column_filter_lists_available() {
        let err = run(
            REF, CUR, "comma", true, "nope", "psi", 0.2, 10, 20, 0.5, false, "drift", "table",
        )
        .unwrap_err();
        assert!(err.contains("column 'nope' is not present in both datasets"), "{err}");
        assert!(err.contains("id, amount, country"), "{err}");
    }

    #[test]
    fn bins_cap_boundary() {
        assert!(run(REF, CUR, "comma", true, "", "psi", 0.2, 50, 20, 0.5, false, "drift", "table").is_ok());
        let err = run(REF, CUR, "comma", true, "", "psi", 0.2, 51, 20, 0.5, false, "drift", "table")
            .unwrap_err();
        assert_eq!(err, "bins must be between 2 and 50, got 51");
    }

    #[test]
    fn row_cap_boundary() {
        let mut at = String::from("a\n");
        at.push_str(&"1\n".repeat(MAX_ROWS));
        assert!(go(&at, "a\n1\n", "table").is_ok());
        let mut over = at.clone();
        over.push_str("1\n");
        let err = go(&over, "a\n1\n", "table").unwrap_err();
        assert!(err.contains("over the 100000 row limit"), "{err}");
    }

    #[test]
    fn identical_datasets_have_zero_drift() {
        let out = go(REF, REF, "table").unwrap();
        assert!(out.contains("0 drifted (0.0%) · no dataset drift"), "{out}");
        assert!(!out.contains("DRIFT  "), "{out}");
    }

    #[test]
    fn null_tokens_count_as_missing() {
        let out = go("a\n1\n2\n", "a\nN/A\nNULL\n", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["columns"][0]["current"]["null_pct"], 100.0);
        assert_eq!(v["columns"][0]["current"]["distinct"], 0);
        // No comparable values on one side → score is null, not a fake 0.
        assert!(v["columns"][0]["drift_score"].is_null(), "{out}");
    }

    #[test]
    fn ignore_case_folds_category_variants() {
        let strict = go("city\nNew York\nParis\n", "city\nnew york\nParis\n", "table").unwrap();
        assert!(strict.contains("new       new york (1)"), "{strict}");
        let folded = run(
            "city\nNew York\nParis\n",
            "city\nnew york\nParis\n",
            "comma",
            true,
            "",
            "psi",
            0.2,
            10,
            20,
            0.5,
            true,
            "drift",
            "table",
        )
        .unwrap();
        assert!(!folded.contains("new       "), "{folded}");
        assert!(folded.contains("0 drifted"), "{folded}");
    }

    #[test]
    fn schema_drift_lists_added_and_removed_columns() {
        let out = go("id,old\n1,x\n", "id,new\n1,2\n", "table").unwrap();
        assert!(out.contains("+ new  added in current (int)"), "{out}");
        assert!(out.contains("- old  removed from current (string)"), "{out}");
    }

    #[test]
    fn type_change_is_flagged() {
        let out = go("v\n1\n2\n3\n", "v\na\nb\nc\n", "table").unwrap();
        assert!(out.contains("int → string  (type changed)"), "{out}");
    }

    #[test]
    fn numeric_columns_bin_when_cardinality_exceeds_max_categories() {
        let mut r = String::from("v\n");
        let mut c = String::from("v\n");
        for i in 0..100 {
            r.push_str(&format!("{i}\n"));
            c.push_str(&format!("{}\n", i + 100));
        }
        let out = run(
            &r, &c, "comma", true, "", "psi", 0.2, 10, 20, 0.5, false, "drift", "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["columns"][0]["mode"], "numeric");
        assert!(v["columns"][0]["drifted"].as_bool().unwrap(), "{out}");
        assert_eq!(v["columns"][0]["reference"]["max"], 99.0);
        assert_eq!(v["columns"][0]["current"]["min"], 100.0);
    }

    #[test]
    fn jsd_stays_within_zero_and_one() {
        let out = run(
            "v\na\na\na\n", "v\nb\nb\nb\n", "comma", true, "", "jsd", 0.1, 10, 20, 0.5, false,
            "drift", "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["columns"][0]["drift_score"], 1.0);
        assert_eq!(v["summary"]["method"], "jsd");
    }

    #[test]
    fn markdown_and_csv_render() {
        let md = go(REF, CUR, "markdown").unwrap();
        assert!(md.starts_with("# Data drift summary"), "{md}");
        assert!(md.contains("| `country` | **yes** |"), "{md}");
        let csv = go(REF, CUR, "csv").unwrap();
        assert!(csv.starts_with("column,status,drift_score,drifted,"), "{csv}");
        assert!(csv.contains("country,compared,"), "{csv}");
    }

    #[test]
    fn sort_modes_reorder_columns() {
        let by_name = run(
            REF, CUR, "comma", true, "", "psi", 0.2, 10, 20, 0.5, false, "name", "csv",
        )
        .unwrap();
        let names: Vec<&str> = by_name
            .lines()
            .skip(1)
            .map(|l| l.split(',').next().unwrap())
            .collect();
        assert_eq!(names, vec!["amount", "country", "id"]);
        let by_order = run(
            REF, CUR, "comma", true, "", "psi", 0.2, 10, 20, 0.5, false, "order", "csv",
        )
        .unwrap();
        let names: Vec<&str> = by_order
            .lines()
            .skip(1)
            .map(|l| l.split(',').next().unwrap())
            .collect();
        assert_eq!(names, vec!["id", "amount", "country"]);
    }

    #[test]
    fn headerless_datasets_use_positional_labels() {
        let out = run(
            "1,US\n2,FR\n", "1,US\n2,DE\n", "comma", false, "", "psi", 0.2, 10, 20, 0.5, false,
            "order", "table",
        )
        .unwrap();
        assert!(out.contains("col1"), "{out}");
        assert!(out.contains("col2"), "{out}");
    }

    #[test]
    fn column_filter_restricts_the_report() {
        let out = run(
            REF, CUR, "comma", true, "country", "psi", 0.2, 10, 20, 0.5, false, "drift", "table",
        )
        .unwrap();
        assert!(out.contains("1 columns compared"), "{out}");
        assert!(!out.contains(" amount "), "{out}");
    }

    #[test]
    fn tab_delimiter_is_accepted() {
        let out = go("a\tb\n1\tx\n", "a\tb\n1\ty\n", "table").unwrap();
        // Parsed as one comma column, so tabs only work through the delimiter arg.
        assert!(out.contains("1 columns compared"), "{out}");
        let out = run(
            "a\tb\n1\tx\n", "a\tb\n1\ty\n", "tab", true, "", "psi", 0.2, 10, 20, 0.5, false,
            "order", "table",
        )
        .unwrap();
        assert!(out.contains("2 columns compared"), "{out}");
    }

    #[test]
    fn drift_share_controls_the_dataset_verdict() {
        // country and amount drift, id does not → share is 2/3.
        let lenient = run(
            REF, CUR, "comma", true, "", "psi", 0.2, 10, 20, 0.8, false, "drift", "table",
        )
        .unwrap();
        assert!(lenient.contains("2 drifted (66.7%) · no dataset drift"), "{lenient}");
        let strict = run(
            REF, CUR, "comma", true, "", "psi", 0.2, 10, 20, 0.5, false, "drift", "table",
        )
        .unwrap();
        assert!(strict.contains("2 drifted (66.7%) · DATASET DRIFT DETECTED"), "{strict}");
    }

    #[test]
    fn bad_method_and_format_are_rejected() {
        let err = run(
            REF, CUR, "comma", true, "", "ks", 0.2, 10, 20, 0.5, false, "drift", "table",
        )
        .unwrap_err();
        assert_eq!(err, "method must be psi or jsd, got 'ks'");
        let err = go(REF, CUR, "xml").unwrap_err();
        assert_eq!(err, "format must be table, markdown, json or csv, got 'xml'");
    }
}
