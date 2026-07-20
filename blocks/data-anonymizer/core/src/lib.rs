//! gizza-ai/data-anonymizer core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Column-role anonymization over a CSV with a k-anonymity report:
//!
//! - **identifier** columns (name, email, SSN, …) are redacted entirely — every
//!   data cell becomes the fixed `label`.
//! - **quasi-identifier** columns are GENERALIZED per auto-detected column type:
//!   numeric → fixed-width bins (`34` → `30-39`), ISO dates → year
//!   (`1987-04-12` → `1987`), text → keep the first `text_keep` characters and
//!   mask the rest with `*` (`London` with keep 3 → `Lon***`; keep 0 → `*`).
//!   A per-column level override rides on the quasi spec: `zipcode:100` sets that
//!   column's bin width (numeric) or keep-count (text).
//! - the **k-anonymity report** groups rows by their generalized quasi-identifier
//!   tuple (equivalence classes) and reports the achieved k (smallest class),
//!   class count/sizes, how many rows fall below the target `k`, optional
//!   suppression of those rows, and — when a `sensitive` column is named —
//!   the distinct l-diversity (min distinct sensitive values per class).
//!
//! Distinct from `csv-pii-redactor` (masks/hashes chosen columns, no
//! generalization and no anonymity metric).

use std::collections::BTreeMap;

/// Hard cap on data rows per run (header row excluded).
pub const MAX_ROWS: usize = 10_000;

/// What `anonymize_csv` returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Anonymized CSV, a blank line, then the report (default).
    Both,
    /// Anonymized CSV only.
    Csv,
    /// k-anonymity report only.
    Report,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "both" | "" => Ok(Output::Both),
            "csv" => Ok(Output::Csv),
            "report" => Ok(Output::Report),
            other => Err(format!(
                "unknown output '{other}' (use 'both', 'csv', or 'report')"
            )),
        }
    }
}

/// Options for one anonymization pass.
#[derive(Debug, Clone)]
pub struct Options {
    /// Target k for the report (and for suppression). Minimum 2.
    pub k: usize,
    /// Default bin width for numeric quasi-identifier columns (> 0).
    pub numeric_bin: f64,
    /// Default number of leading characters kept in text quasi-identifiers.
    pub text_keep: usize,
    /// Generalize ISO dates (YYYY-MM-DD / YYYY-MM) to the year.
    pub dates_to_year: bool,
    /// Drop rows whose equivalence class is smaller than `k`.
    pub suppress: bool,
    /// Replacement string for identifier columns.
    pub label: String,
    /// What to return.
    pub output: Output,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            k: 2,
            numeric_bin: 10.0,
            text_keep: 3,
            dates_to_year: true,
            suppress: false,
            label: "[REDACTED]".to_string(),
            output: Output::Both,
        }
    }
}

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

/// Resolve one column key (a header name when `header` is present, else a
/// 1-based index) to a 0-based column index.
fn resolve_one(
    key: &str,
    header: Option<&csv::StringRecord>,
    width: usize,
) -> Result<usize, String> {
    if let Some(hdr) = header {
        for (i, name) in hdr.iter().enumerate() {
            if name.trim() == key {
                return Ok(i);
            }
        }
    }
    let n: usize = key.parse().map_err(|_| {
        if header.is_some() {
            format!("no column named '{key}' and it is not a valid index")
        } else {
            format!("column must be a 1-based index, got '{key}'")
        }
    })?;
    if n == 0 || n > width {
        return Err(format!(
            "column index {n} out of range (file has {width} column(s))"
        ));
    }
    Ok(n - 1)
}

/// A resolved quasi-identifier column: index + optional per-column level
/// override (`age:5` → bin width 5 for a numeric column / keep 5 chars for a
/// text column).
struct QuasiCol {
    idx: usize,
    level: Option<f64>,
}

/// Parse the quasi spec: comma-separated `name` / `index` items, each with an
/// optional `:level` suffix (a positive number), e.g. `age,zipcode:100,gender`.
fn resolve_quasi(
    spec: &str,
    header: Option<&csv::StringRecord>,
    width: usize,
) -> Result<Vec<QuasiCol>, String> {
    let mut out: Vec<QuasiCol> = Vec::new();
    for raw in spec.split(',') {
        let item = raw.trim();
        if item.is_empty() {
            continue;
        }
        // Try the whole item as a column key first (a header could contain ':');
        // otherwise split on the LAST ':' and treat a positive-number suffix as
        // the per-column level.
        let (key, level) = match resolve_one(item, header, width) {
            Ok(idx) => (Ok(idx), None),
            Err(whole_err) => match item.rsplit_once(':') {
                Some((head, tail)) => match tail.trim().parse::<f64>() {
                    Ok(v) if v.is_finite() && v > 0.0 => {
                        (resolve_one(head.trim(), header, width), Some(v))
                    }
                    _ => (Err(whole_err), None),
                },
                None => (Err(whole_err), None),
            },
        };
        let idx = key?;
        if out.iter().any(|q| q.idx == idx) {
            return Err(format!("quasi-identifier column '{item}' listed twice"));
        }
        out.push(QuasiCol { idx, level });
    }
    if out.is_empty() {
        return Err("no quasi-identifier columns specified".into());
    }
    Ok(out)
}

fn resolve_list(
    spec: &str,
    header: Option<&csv::StringRecord>,
    width: usize,
) -> Result<Vec<usize>, String> {
    let mut out: Vec<usize> = Vec::new();
    for raw in spec.split(',') {
        let key = raw.trim();
        if key.is_empty() {
            continue;
        }
        let idx = resolve_one(key, header, width)?;
        if !out.contains(&idx) {
            out.push(idx);
        }
    }
    Ok(out)
}

/// True when the value should be binned as a number. Values with a leading zero
/// (`013053`) are kept textual so codes keep their leading zeros.
fn numeric_ok(v: &str) -> bool {
    if v.parse::<f64>().is_err() {
        return false;
    }
    let d = v.strip_prefix(['+', '-']).unwrap_or(v);
    !(d.len() > 1 && d.starts_with('0') && !d.starts_with("0."))
}

/// True for ISO-style dates: YYYY-MM or YYYY-MM-DD (also with '/').
fn date_ok(v: &str) -> bool {
    let sep = if v.contains('-') {
        '-'
    } else if v.contains('/') {
        '/'
    } else {
        return false;
    };
    let parts: Vec<&str> = v.split(sep).collect();
    if parts.len() != 2 && parts.len() != 3 {
        return false;
    }
    if parts[0].len() != 4 || !parts[0].bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    parts[1..]
        .iter()
        .all(|p| (1..=2).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColType {
    Numeric,
    Date,
    Text,
}

/// Format a bin bound: integers without decimals, floats via shortest Display.
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 9e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn generalize_numeric(v: f64, width: f64, integer_style: bool) -> String {
    let lo = (v / width).floor() * width;
    let hi = lo + width;
    if integer_style {
        format!("{}-{}", fmt_num(lo), fmt_num(hi - 1.0))
    } else {
        format!("[{},{})", fmt_num(lo), fmt_num(hi))
    }
}

fn generalize_text(v: &str, keep: usize) -> String {
    if keep == 0 {
        return "*".to_string();
    }
    let chars: Vec<char> = v.chars().collect();
    if chars.len() <= keep {
        return v.to_string();
    }
    let mut out: String = chars[..keep].iter().collect();
    for _ in keep..chars.len() {
        out.push('*');
    }
    out
}

fn pct(part: usize, total: usize) -> String {
    if total == 0 {
        return "0.0".into();
    }
    format!("{:.1}", part as f64 * 100.0 / total as f64)
}

/// Anonymize `data` and/or report its k-anonymity level. `quasi` / `identifiers`
/// / `sensitive` are comma-separated column names (when `header` is true) or
/// 1-based indices; `quasi` items accept a `:level` suffix. See the module doc.
pub fn anonymize_csv(
    data: &str,
    quasi: &str,
    identifiers: &str,
    sensitive: &str,
    header: bool,
    delimiter: &str,
    opts: &Options,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if opts.k < 2 {
        return Err("k must be at least 2".into());
    }
    if !opts.numeric_bin.is_finite() || opts.numeric_bin <= 0.0 {
        return Err("numeric_bin must be a positive number".into());
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
        return Err("no rows found".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let header_rec = if header { records.first() } else { None };
    let data_rows: Vec<&csv::StringRecord> =
        records.iter().skip(if header { 1 } else { 0 }).collect();
    if data_rows.is_empty() {
        return Err("no data rows found (only a header)".into());
    }
    if data_rows.len() > MAX_ROWS {
        return Err(format!(
            "too many rows: {} (max {MAX_ROWS} data rows per run)",
            data_rows.len()
        ));
    }

    let quasi_cols = resolve_quasi(quasi, header_rec, width)?;
    let id_cols = resolve_list(identifiers, header_rec, width)?;
    let sens_cols = resolve_list(sensitive, header_rec, width)?;
    if sens_cols.len() > 1 {
        return Err("sensitive must name a single column".into());
    }
    let sens_col = sens_cols.first().copied();
    let col_name = |i: usize| -> String {
        header_rec
            .and_then(|h| h.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("col {}", i + 1))
    };
    for q in &quasi_cols {
        if id_cols.contains(&q.idx) {
            return Err(format!(
                "column '{}' is both an identifier and a quasi-identifier",
                col_name(q.idx)
            ));
        }
    }
    if let Some(s) = sens_col {
        if id_cols.contains(&s) || quasi_cols.iter().any(|q| q.idx == s) {
            return Err(format!(
                "sensitive column '{}' cannot also be an identifier or quasi-identifier",
                col_name(s)
            ));
        }
    }

    // Type + style detection per quasi column, over non-empty trimmed values.
    struct QuasiPlan {
        idx: usize,
        ctype: ColType,
        bin: f64,
        keep: usize,
        integer_style: bool,
    }
    let mut plans: Vec<QuasiPlan> = Vec::with_capacity(quasi_cols.len());
    for q in &quasi_cols {
        let vals: Vec<&str> = data_rows
            .iter()
            .filter_map(|r| r.get(q.idx))
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .collect();
        let ctype = if !vals.is_empty() && vals.iter().all(|v| numeric_ok(v)) {
            ColType::Numeric
        } else if opts.dates_to_year && !vals.is_empty() && vals.iter().all(|v| date_ok(v)) {
            ColType::Date
        } else {
            ColType::Text
        };
        let bin = q.level.unwrap_or(opts.numeric_bin);
        let keep = q
            .level
            .map(|l| l.max(0.0).floor() as usize)
            .unwrap_or(opts.text_keep);
        let integer_style = ctype == ColType::Numeric
            && bin.fract() == 0.0
            && vals.iter().all(|v| {
                v.parse::<f64>()
                    .map(|n| n.fract() == 0.0 && n >= 0.0)
                    .unwrap_or(false)
            });
        plans.push(QuasiPlan {
            idx: q.idx,
            ctype,
            bin,
            keep,
            integer_style,
        });
    }

    // Generalize every data row; build the equivalence-class key per row.
    let generalize_cell = |plan: &QuasiPlan, cell: &str| -> String {
        let v = cell.trim();
        if v.is_empty() {
            return String::new();
        }
        match plan.ctype {
            ColType::Numeric => match v.parse::<f64>() {
                Ok(n) => generalize_numeric(n, plan.bin, plan.integer_style),
                Err(_) => generalize_text(v, plan.keep),
            },
            ColType::Date => v[..4].to_string(),
            ColType::Text => generalize_text(v, plan.keep),
        }
    };
    let mut out_rows: Vec<Vec<String>> = Vec::with_capacity(data_rows.len());
    let mut keys: Vec<String> = Vec::with_capacity(data_rows.len());
    for rec in &data_rows {
        let mut fields: Vec<String> = rec.iter().map(|c| c.to_string()).collect();
        for plan in &plans {
            if let Some(cell) = rec.get(plan.idx) {
                if plan.idx < fields.len() {
                    fields[plan.idx] = generalize_cell(plan, cell);
                }
            }
        }
        for &i in &id_cols {
            if i < fields.len() {
                fields[i] = opts.label.clone();
            }
        }
        let key: String = plans
            .iter()
            .map(|p| fields.get(p.idx).cloned().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\u{1f}");
        keys.push(key);
        out_rows.push(fields);
    }

    // Equivalence classes before suppression.
    let mut class_sizes: BTreeMap<&str, usize> = BTreeMap::new();
    for k in &keys {
        *class_sizes.entry(k.as_str()).or_insert(0) += 1;
    }
    let total = out_rows.len();
    let below: usize = keys
        .iter()
        .filter(|k| class_sizes[k.as_str()] < opts.k)
        .count();
    let suppressed = if opts.suppress { below } else { 0 };

    // Keep flags + post-suppression classes.
    let kept: Vec<bool> = keys
        .iter()
        .map(|k| !opts.suppress || class_sizes[k.as_str()] >= opts.k)
        .collect();
    let mut final_sizes: BTreeMap<&str, usize> = BTreeMap::new();
    let mut sens_values: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (i, k) in keys.iter().enumerate() {
        if !kept[i] {
            continue;
        }
        *final_sizes.entry(k.as_str()).or_insert(0) += 1;
        if let Some(s) = sens_col {
            sens_values
                .entry(k.as_str())
                .or_default()
                .push(out_rows[i].get(s).map(|v| v.as_str()).unwrap_or(""));
        }
    }
    let remaining = total - suppressed;
    let achieved_k = final_sizes.values().copied().min().unwrap_or(0);
    let smallest = achieved_k;
    let largest = final_sizes.values().copied().max().unwrap_or(0);
    let final_below: usize = if opts.suppress { 0 } else { below };

    // Serialize the anonymized CSV.
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    if let Some(h) = header_rec {
        wtr.write_record(h.iter())
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for (i, fields) in out_rows.iter().enumerate() {
        if kept[i] {
            wtr.write_record(fields)
                .map_err(|e| format!("CSV write error: {e}"))?;
        }
    }
    let csv_text = String::from_utf8(
        wtr.into_inner()
            .map_err(|e| format!("CSV write error: {e}"))?,
    )
    .map_err(|e| format!("utf8 error: {e}"))?;

    // Build the report.
    let mut report = String::new();
    report.push_str("K-anonymity report\n");
    report.push_str(&format!("Data rows: {remaining} ({suppressed} suppressed)\n"));
    report.push_str(&format!(
        "Quasi-identifiers: {}\n",
        quasi_cols
            .iter()
            .map(|q| col_name(q.idx))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if !id_cols.is_empty() {
        report.push_str(&format!(
            "Redacted identifiers: {}\n",
            id_cols
                .iter()
                .map(|&i| col_name(i))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    report.push_str(&format!(
        "Equivalence classes: {} (smallest {smallest}, largest {largest})\n",
        final_sizes.len()
    ));
    if remaining == 0 {
        report.push_str("Achieved k = 0 — no data rows remain\n");
        report.push_str(&format!(
            "Target k = {}: NOT MET — all rows were suppressed\n",
            opts.k
        ));
    } else {
        if achieved_k == 1 {
            report.push_str(
                "Achieved k = 1 — at least one row is unique on its quasi-identifiers\n",
            );
        } else {
            report.push_str(&format!(
                "Achieved k = {achieved_k} — every row is indistinguishable from at least {} other row(s)\n",
                achieved_k - 1
            ));
        }
        if final_below == 0 && achieved_k >= opts.k {
            report.push_str(&format!(
                "Target k = {}: MET — no rows fall below the target\n",
                opts.k
            ));
        } else {
            report.push_str(&format!(
                "Target k = {}: NOT MET — {} of {} rows ({}%) are in classes smaller than {}\n",
                opts.k,
                final_below,
                remaining,
                pct(final_below, remaining),
                opts.k
            ));
        }
    }
    if let Some(s) = sens_col {
        if remaining > 0 {
            let l = sens_values
                .values()
                .map(|vals| {
                    let mut uniq: Vec<&str> = vals.clone();
                    uniq.sort_unstable();
                    uniq.dedup();
                    uniq.len()
                })
                .min()
                .unwrap_or(0);
            report.push_str(&format!(
                "Distinct l-diversity on '{}': l = {l}\n",
                col_name(s)
            ));
        }
    }

    Ok(match opts.output {
        Output::Both => format!("{csv_text}\n{report}"),
        Output::Csv => csv_text,
        Output::Report => report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSPITAL: &str = "name,age,zipcode,gender,diagnosis\n\
Ada,34,13053,F,Flu\n\
Bea,38,13068,F,Cold\n\
Cal,52,14850,M,Flu\n\
Dan,57,14853,M,Fever";

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn hospital_demo_generalizes_and_reports() {
        let out = anonymize_csv(
            HOSPITAL,
            "age,zipcode,gender",
            "name",
            "diagnosis",
            true,
            ",",
            &opts(),
        )
        .unwrap();
        let expected = "name,age,zipcode,gender,diagnosis\n\
[REDACTED],30-39,13050-13059,F,Flu\n\
[REDACTED],30-39,13060-13069,F,Cold\n\
[REDACTED],50-59,14850-14859,M,Flu\n\
[REDACTED],50-59,14850-14859,M,Fever\n";
        assert!(out.starts_with(expected), "got: {out}");
        assert!(out.contains("Achieved k = 1"), "got: {out}");
    }

    #[test]
    fn per_column_override_widens_zip_bin() {
        let out = anonymize_csv(
            HOSPITAL,
            "age,zipcode:100,gender",
            "name",
            "diagnosis",
            true,
            ",",
            &opts(),
        )
        .unwrap();
        assert!(
            out.contains("[REDACTED],30-39,13000-13099,F,Flu"),
            "got: {out}"
        );
        assert!(
            out.contains(
                "Achieved k = 2 — every row is indistinguishable from at least 1 other row(s)"
            ),
            "got: {out}"
        );
        assert!(
            out.contains("Target k = 2: MET — no rows fall below the target"),
            "got: {out}"
        );
        assert!(
            out.contains("Distinct l-diversity on 'diagnosis': l = 2"),
            "got: {out}"
        );
        assert!(
            out.contains("Equivalence classes: 2 (smallest 2, largest 2)"),
            "got: {out}"
        );
    }

    #[test]
    fn leading_zero_codes_stay_text_and_prefix_mask() {
        let d = "zip\n01035\n01040\n01035";
        let out = anonymize_csv(d, "zip", "", "", true, ",", &opts()).unwrap();
        assert!(out.contains("010**"), "got: {out}");
        assert!(!out.contains("01035"), "got: {out}");
    }

    #[test]
    fn dates_generalize_to_year_and_can_be_disabled() {
        let d = "dob\n1987-04-12\n1987-11-30\n1990-01-01";
        let out = anonymize_csv(d, "dob", "", "", true, ",", &opts()).unwrap();
        assert!(out.contains("\n1987\n"), "got: {out}");
        let o = Options {
            dates_to_year: false,
            ..opts()
        };
        let out2 = anonymize_csv(d, "dob", "", "", true, ",", &o).unwrap();
        assert!(out2.contains("198*******"), "got: {out2}");
    }

    #[test]
    fn text_keep_zero_suppresses_whole_value() {
        let d = "city\nLondon\nParis";
        let o = Options {
            text_keep: 0,
            ..opts()
        };
        let out = anonymize_csv(d, "city", "", "", true, ",", &o).unwrap();
        assert!(out.contains("\n*\n"), "got: {out}");
        assert!(out.contains("Achieved k = 2"), "got: {out}");
    }

    #[test]
    fn suppress_drops_under_k_rows_and_reports() {
        let d = "age\n34\n36\n71";
        let o = Options {
            suppress: true,
            ..opts()
        };
        let out = anonymize_csv(d, "age", "", "", true, ",", &o).unwrap();
        assert!(!out.contains("70-79"), "got: {out}");
        assert!(out.contains("Data rows: 2 (1 suppressed)"), "got: {out}");
        assert!(out.contains("Target k = 2: MET"), "got: {out}");
    }

    #[test]
    fn suppress_everything_reports_no_rows_remain() {
        let d = "age\n34\n71";
        let o = Options {
            suppress: true,
            k: 5,
            ..opts()
        };
        let out = anonymize_csv(d, "age", "", "", true, ",", &o).unwrap();
        assert!(
            out.contains("Achieved k = 0 — no data rows remain"),
            "got: {out}"
        );
        assert!(
            out.contains("Target k = 5: NOT MET — all rows were suppressed"),
            "got: {out}"
        );
    }

    #[test]
    fn not_met_reports_at_risk_share() {
        let d = "age\n34\n36\n71";
        let out = anonymize_csv(d, "age", "", "", true, ",", &opts()).unwrap();
        assert!(
            out.contains(
                "Target k = 2: NOT MET — 1 of 3 rows (33.3%) are in classes smaller than 2"
            ),
            "got: {out}"
        );
    }

    #[test]
    fn output_modes_select_sections() {
        let d = "age\n34\n36";
        let both = anonymize_csv(d, "age", "", "", true, ",", &opts()).unwrap();
        assert!(both.contains("30-39") && both.contains("K-anonymity report"));
        let o = Options {
            output: Output::Csv,
            ..opts()
        };
        let csv_only = anonymize_csv(d, "age", "", "", true, ",", &o).unwrap();
        assert_eq!(csv_only, "age\n30-39\n30-39\n");
        let o = Options {
            output: Output::Report,
            ..opts()
        };
        let report_only = anonymize_csv(d, "age", "", "", true, ",", &o).unwrap();
        assert!(
            report_only.starts_with("K-anonymity report\n"),
            "got: {report_only}"
        );
        assert!(!report_only.contains("30-39"));
    }

    #[test]
    fn float_bins_use_interval_style() {
        let d = "score\n3.7\n4.2\n12.5";
        let o = Options {
            numeric_bin: 5.0,
            ..opts()
        };
        let out = anonymize_csv(d, "score", "", "", true, ",", &o).unwrap();
        assert!(out.contains("[0,5)"), "got: {out}");
        assert!(out.contains("[10,15)"), "got: {out}");
    }

    #[test]
    fn negative_values_use_interval_style() {
        let d = "t\n-3\n4";
        let out = anonymize_csv(d, "t", "", "", true, ",", &opts()).unwrap();
        assert!(out.contains("[-10,0)"), "got: {out}");
        assert!(out.contains("[0,10)"), "got: {out}");
    }

    #[test]
    fn semicolon_delimiter_roundtrips() {
        let d = "age;city\n34;London\n36;Lisbon";
        let out = anonymize_csv(d, "age,city", "", "", true, ";", &opts()).unwrap();
        assert!(out.contains("30-39;Lon***"), "got: {out}");
    }

    #[test]
    fn no_header_uses_indices() {
        let d = "34,London\n36,Lisbon";
        let out = anonymize_csv(d, "1,2", "", "", false, ",", &opts()).unwrap();
        assert!(out.contains("30-39,Lon***"), "got: {out}");
        assert!(out.contains("Quasi-identifiers: col 1, col 2"), "got: {out}");
    }

    #[test]
    fn row_cap_boundary_at_and_over() {
        let mut at = String::from("v\n");
        for _ in 0..MAX_ROWS {
            at.push_str("7\n");
        }
        let out = anonymize_csv(&at, "v", "", "", true, ",", &opts()).unwrap();
        assert!(
            out.contains(&format!("Achieved k = {MAX_ROWS}")),
            "cap-boundary run failed"
        );
        let mut over = at;
        over.push_str("7\n");
        let err = anonymize_csv(&over, "v", "", "", true, ",", &opts()).unwrap_err();
        assert_eq!(
            err,
            format!(
                "too many rows: {} (max {MAX_ROWS} data rows per run)",
                MAX_ROWS + 1
            )
        );
    }

    #[test]
    fn unknown_quasi_column_errors() {
        let err = anonymize_csv(HOSPITAL, "height", "", "", true, ",", &opts()).unwrap_err();
        assert!(err.contains("no column named 'height'"), "got: {err}");
    }

    #[test]
    fn overlapping_roles_error() {
        let err = anonymize_csv(HOSPITAL, "age", "age", "", true, ",", &opts()).unwrap_err();
        assert!(
            err.contains("both an identifier and a quasi-identifier"),
            "got: {err}"
        );
        let err = anonymize_csv(HOSPITAL, "age", "name", "age", true, ",", &opts()).unwrap_err();
        assert!(err.contains("cannot also be"), "got: {err}");
    }

    #[test]
    fn sensitive_must_be_single_column() {
        let err =
            anonymize_csv(HOSPITAL, "age", "", "diagnosis,gender", true, ",", &opts()).unwrap_err();
        assert!(err.contains("single column"), "got: {err}");
    }

    #[test]
    fn empty_and_header_only_error() {
        assert!(anonymize_csv("  ", "1", "", "", true, ",", &opts()).is_err());
        assert!(anonymize_csv("age,city", "age", "", "", true, ",", &opts())
            .unwrap_err()
            .contains("only a header"));
    }

    #[test]
    fn invalid_k_and_bin_error() {
        let d = "age\n34\n36";
        let o = Options { k: 1, ..opts() };
        assert!(anonymize_csv(d, "age", "", "", true, ",", &o).is_err());
        let o = Options {
            numeric_bin: 0.0,
            ..opts()
        };
        assert!(anonymize_csv(d, "age", "", "", true, ",", &o).is_err());
    }

    #[test]
    fn output_parse() {
        assert_eq!(Output::parse("BOTH").unwrap(), Output::Both);
        assert_eq!(Output::parse("csv").unwrap(), Output::Csv);
        assert_eq!(Output::parse("report").unwrap(), Output::Report);
        assert!(Output::parse("xml").is_err());
    }

    #[test]
    fn empty_cells_form_their_own_class() {
        let d = "age,city\n34,X\n,X\n36,X";
        let out = anonymize_csv(d, "age", "", "", true, ",", &opts()).unwrap();
        assert!(out.contains("Equivalence classes: 2"), "got: {out}");
    }

    #[test]
    fn custom_label_applies_to_identifiers() {
        let d = "name,age\nAda,34\nBob,36";
        let o = Options {
            label: "***".into(),
            ..opts()
        };
        let out = anonymize_csv(d, "age", "name", "", true, ",", &o).unwrap();
        assert!(out.contains("***,30-39"), "got: {out}");
    }
}
