//! gizza-ai/coefficient-of-variation-calculator core — the coefficient of
//! variation (standard deviation ÷ mean) for one or more datasets, ranked by
//! relative dispersion.
//!
//! Unlike a plain standard deviation, the CV is unitless, so it compares the
//! consistency of datasets measured on different scales. This core parses one
//! dataset per line (or a single pooled dataset), computes n / mean / sum of
//! squares / standard deviation on either the sample (n−1) or population (N)
//! basis, derives the CV as a ratio and a percentage, ranks the datasets by
//! |CV|, and renders a readable report, a markdown table, or JSON.
//!
//! Pure Rust, no I/O; `serde` is used only for the JSON output shape.

use serde::Serialize;

/// Hard caps so a pasted spreadsheet can't wedge the browser.
pub const MAX_VALUES: usize = 200_000;
pub const MAX_DATASETS: usize = 1_000;

#[derive(Debug, Clone, Serialize)]
pub struct Dataset {
    pub label: String,
    /// Rank by relative dispersion, 1 = lowest |CV|. Datasets with an
    /// undefined CV are ranked last.
    pub rank: usize,
    pub n: usize,
    pub mean: f64,
    pub std_dev: f64,
    /// `"sample (n-1)"`, `"population (N)"`, or `"given"` in summary mode.
    pub basis: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sum_of_squares: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// `sd / mean`; `None` when the mean is 0 or the standard deviation is
    /// undefined (n < 2 on the sample basis).
    pub cv: Option<f64>,
    pub cv_percent: Option<f64>,
    pub relative_spread: String,
    /// Values dropped by the Tukey 1.5×IQR filter, when `exclude_outliers`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub outliers_removed: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub basis: String,
    pub dataset_count: usize,
    pub datasets: Vec<Dataset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub most_consistent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub most_variable: Option<String>,
    /// Highest |CV| ÷ lowest |CV| across the ranked datasets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spread_ratio: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

fn round_to(v: f64, decimals: u32) -> f64 {
    let f = 10f64.powi(decimals as i32);
    (v * f).round() / f
}

fn fmt(v: f64, decimals: u32) -> String {
    format!("{:.*}", decimals as usize, v)
}

fn fmt_opt(v: Option<f64>, decimals: u32) -> String {
    v.map(|x| fmt(x, decimals))
        .unwrap_or_else(|| "undefined".to_string())
}

/// Rule-of-thumb band for |CV| expressed as a percentage. These are reporting
/// conventions, not statistical tests — the page says so too.
fn spread_band(cv: Option<f64>) -> &'static str {
    match cv {
        None => "undefined",
        Some(c) => {
            let p = c.abs() * 100.0;
            if p < 1.0 {
                "very low"
            } else if p < 10.0 {
                "low"
            } else if p < 20.0 {
                "moderate"
            } else if p < 30.0 {
                "high"
            } else {
                "very high"
            }
        }
    }
}

fn split_tokens<'a>(line: &'a str, delimiter: &str) -> Vec<&'a str> {
    let parts: Vec<&str> = match delimiter {
        "comma" => line.split(',').collect(),
        "tab" => line.split('\t').collect(),
        "semicolon" => line.split(';').collect(),
        "pipe" => line.split('|').collect(),
        "space" => line.split_whitespace().collect(),
        // auto: every common separator at once. A number never contains one of
        // these, so splitting on all of them is safe and needs no detection.
        _ => line
            .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '|'))
            .collect(),
    };
    parts
        .into_iter()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect()
}

struct RawSet {
    label: Option<String>,
    values: Vec<f64>,
}

/// Pull an optional `Label:` prefix off a line. A leading segment is treated as
/// a label only when it is not itself a number, so `12:30` style data is safe.
fn split_label(line: &str) -> (Option<String>, &str) {
    if let Some(idx) = line.find(':') {
        let (head, tail) = line.split_at(idx);
        let head = head.trim();
        if !head.is_empty() && head.parse::<f64>().is_err() {
            return (Some(head.to_string()), &tail[1..]);
        }
    }
    (None, line)
}

fn parse_line(
    line: &str,
    delimiter: &str,
    ignore_non_numeric: bool,
    line_no: usize,
) -> Result<RawSet, String> {
    let (label, rest) = split_label(line);
    let mut values = Vec::new();
    for tok in split_tokens(rest, delimiter) {
        match tok.parse::<f64>() {
            Ok(v) if v.is_finite() => values.push(v),
            _ => {
                if !ignore_non_numeric {
                    return Err(format!(
                        "line {line_no}: `{tok}` is not a number. Fix the value, prefix the row with a label like `Machine A: 1 2 3`, or set ignore_non_numeric=true to skip non-numeric cells."
                    ));
                }
            }
        }
    }
    Ok(RawSet { label, values })
}

/// Tukey 1.5×IQR filter. Returns (kept, removed).
fn strip_outliers(values: &[f64]) -> (Vec<f64>, Vec<f64>) {
    if values.len() < 4 {
        return (values.to_vec(), Vec::new());
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |p: f64| -> f64 {
        let rank = p * (sorted.len() as f64 - 1.0);
        let lo = rank.floor() as usize;
        let hi = rank.ceil() as usize;
        if lo == hi {
            sorted[lo]
        } else {
            sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo as f64)
        }
    };
    let (q1, q3) = (pct(0.25), pct(0.75));
    let iqr = q3 - q1;
    let (lo, hi) = (q1 - 1.5 * iqr, q3 + 1.5 * iqr);
    let mut kept = Vec::new();
    let mut removed = Vec::new();
    for &v in values {
        if v < lo || v > hi {
            removed.push(v);
        } else {
            kept.push(v);
        }
    }
    (kept, removed)
}

#[allow(clippy::too_many_arguments)]
fn build_dataset(
    label: String,
    values: &[f64],
    basis: &str,
    exclude_outliers: bool,
    decimals: u32,
) -> Dataset {
    let mut notes = Vec::new();
    let (kept, removed) = if exclude_outliers {
        strip_outliers(values)
    } else {
        (values.to_vec(), Vec::new())
    };
    if exclude_outliers && values.len() < 4 {
        notes.push(
            "too few values for the 1.5×IQR outlier filter (needs 4); nothing removed".into(),
        );
    }

    let n = kept.len();
    let sum: f64 = kept.iter().sum();
    let mean = sum / n as f64;
    let ss: f64 = kept.iter().map(|x| (x - mean) * (x - mean)).sum();

    let sample = basis == "sample";
    let variance = if sample {
        if n >= 2 {
            Some(ss / (n as f64 - 1.0))
        } else {
            None
        }
    } else {
        Some(ss / n as f64)
    };
    if sample && n < 2 {
        notes.push("the sample basis needs at least 2 values; switch basis=population for a single reading".into());
    }

    let std_dev = variance.map(f64::sqrt);
    let cv = match (std_dev, mean) {
        (Some(_), m) if m == 0.0 => {
            notes.push("mean is 0, so the coefficient of variation is undefined".into());
            None
        }
        (Some(sd), m) => Some(sd / m),
        (None, _) => None,
    };
    if mean < 0.0 {
        notes.push(
            "mean is negative; the CV is reported with its sign and the ranking uses |CV|".into(),
        );
    }
    if kept.iter().any(|v| *v > 0.0) && kept.iter().any(|v| *v < 0.0) {
        notes.push(
            "values straddle zero; the CV is unreliable on data that is not on a ratio scale"
                .into(),
        );
    }
    if let (Some(sd), true) = (std_dev, mean != 0.0) {
        if mean.abs() < sd / 10.0 {
            notes.push("the mean is very close to zero relative to the spread, so the CV is numerically unstable".into());
        }
    }

    let min = kept.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = kept.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    Dataset {
        label,
        rank: 0,
        n,
        mean: round_to(mean, decimals),
        std_dev: round_to(std_dev.unwrap_or(f64::NAN), decimals),
        basis: if sample {
            "sample (n-1)".into()
        } else {
            "population (N)".into()
        },
        sum: Some(round_to(sum, decimals)),
        sum_of_squares: Some(round_to(ss, decimals)),
        min: (n > 0).then(|| round_to(min, decimals)),
        max: (n > 0).then(|| round_to(max, decimals)),
        cv: cv.map(|c| round_to(c, decimals)),
        cv_percent: cv.map(|c| round_to(c * 100.0, decimals)),
        relative_spread: spread_band(cv).to_string(),
        outliers_removed: removed.into_iter().map(|v| round_to(v, decimals)).collect(),
        notes,
    }
}

/// Compute the report. `decimals` arrives as `f64` because the page hands every
/// field over as a string and the CLI/chat schema uses a JSON number.
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    data: &str,
    basis: &str,
    grouping: &str,
    delimiter: &str,
    mean_in: Option<f64>,
    std_dev_in: Option<f64>,
    exclude_outliers: bool,
    ignore_non_numeric: bool,
    decimals: f64,
) -> Result<Report, String> {
    let decimals = decimals.clamp(0.0, 10.0).round() as u32;
    let basis = match basis.trim().to_ascii_lowercase().as_str() {
        "" | "sample" => "sample",
        "population" => "population",
        other => return Err(format!("basis must be sample or population, got `{other}`")),
    };
    let grouping = match grouping.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => "auto",
        "single" => "single",
        "lines" => "lines",
        other => {
            return Err(format!(
                "grouping must be auto, single or lines, got `{other}`"
            ))
        }
    };
    let delimiter_normalized = delimiter.trim().to_ascii_lowercase();
    let delimiter = match delimiter_normalized.as_str() {
        "" | "auto" => "auto",
        d @ ("comma" | "tab" | "semicolon" | "space" | "pipe") => d,
        other => {
            return Err(format!(
                "delimiter must be auto, comma, tab, semicolon, space or pipe, got `{other}`"
            ))
        }
    };

    // ---- summary mode: a known standard deviation and mean, no raw data ----
    let has_data = data
        .lines()
        .any(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
    if !has_data {
        let (m, sd) = match (mean_in, std_dev_in) {
            (Some(m), Some(sd)) => (m, sd),
            (None, None) => {
                return Err("provide data (one dataset per line), or both mean and std_dev to compute the CV from summary statistics".into())
            }
            (Some(_), None) => return Err("mean was given without std_dev — summary mode needs both".into()),
            (None, Some(_)) => return Err("std_dev was given without mean — summary mode needs both".into()),
        };
        if !m.is_finite() || !sd.is_finite() {
            return Err("mean and std_dev must be finite numbers".into());
        }
        if sd < 0.0 {
            return Err(format!("std_dev must not be negative, got {sd}"));
        }
        let mut notes = Vec::new();
        let cv = if m == 0.0 {
            notes.push("mean is 0, so the coefficient of variation is undefined".into());
            None
        } else {
            Some(sd / m)
        };
        if m < 0.0 {
            notes.push("mean is negative; the CV is reported with its sign".into());
        }
        let ds = Dataset {
            label: "Summary input".into(),
            rank: 1,
            n: 0,
            mean: round_to(m, decimals),
            std_dev: round_to(sd, decimals),
            basis: "given".into(),
            sum: None,
            sum_of_squares: None,
            min: None,
            max: None,
            cv: cv.map(|c| round_to(c, decimals)),
            cv_percent: cv.map(|c| round_to(c * 100.0, decimals)),
            relative_spread: spread_band(cv).to_string(),
            outliers_removed: Vec::new(),
            notes,
        };
        let warnings = ds.notes.clone();
        return Ok(Report {
            basis: "given".into(),
            dataset_count: 1,
            most_consistent: None,
            most_variable: None,
            spread_ratio: None,
            datasets: vec![ds],
            warnings,
        });
    }

    // ---- raw-data mode ----
    let mut raw: Vec<RawSet> = Vec::new();
    for (i, line) in data.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let set = parse_line(trimmed, delimiter, ignore_non_numeric, i + 1)?;
        if set.values.is_empty() && set.label.is_none() {
            continue;
        }
        raw.push(set);
    }
    if raw.is_empty() || raw.iter().all(|s| s.values.is_empty()) {
        return Err(
            "no numbers found — paste one dataset per line, for example `Machine A: 4.2 5.1 4.8`"
                .into(),
        );
    }

    let total: usize = raw.iter().map(|s| s.values.len()).sum();
    if total > MAX_VALUES {
        return Err(format!(
            "{total} values exceeds the {MAX_VALUES}-value limit — split the input into smaller runs"
        ));
    }

    // `auto` pools every line into one dataset only when the input looks like a
    // single column: no labels anywhere and no line holding two or more values.
    let per_line = match grouping {
        "lines" => true,
        "single" => false,
        _ => raw.iter().any(|s| s.label.is_some() || s.values.len() > 1),
    };

    let mut sets: Vec<(String, Vec<f64>)> = Vec::new();
    if per_line {
        for (i, s) in raw.iter().enumerate() {
            if s.values.is_empty() {
                return Err(format!(
                    "dataset `{}` has no numbers — every labelled row needs at least one value",
                    s.label
                        .clone()
                        .unwrap_or_else(|| format!("Dataset {}", i + 1))
                ));
            }
            sets.push((
                s.label
                    .clone()
                    .unwrap_or_else(|| format!("Dataset {}", i + 1)),
                s.values.clone(),
            ));
        }
    } else {
        let label = raw
            .iter()
            .find_map(|s| s.label.clone())
            .unwrap_or_else(|| "Dataset 1".to_string());
        sets.push((label, raw.iter().flat_map(|s| s.values.clone()).collect()));
    }
    if sets.len() > MAX_DATASETS {
        return Err(format!(
            "{} datasets exceeds the {MAX_DATASETS}-dataset limit",
            sets.len()
        ));
    }

    let mut datasets: Vec<Dataset> = sets
        .into_iter()
        .map(|(label, values)| build_dataset(label, &values, basis, exclude_outliers, decimals))
        .collect();

    // Rank by |CV| ascending; undefined CVs sort last, input order preserved
    // among ties.
    datasets.sort_by(|a, b| match (a.cv, b.cv) {
        (Some(x), Some(y)) => x.abs().partial_cmp(&y.abs()).unwrap(),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    for (i, d) in datasets.iter_mut().enumerate() {
        d.rank = i + 1;
    }

    let defined: Vec<&Dataset> = datasets.iter().filter(|d| d.cv.is_some()).collect();
    let (most_consistent, most_variable, spread_ratio) = if defined.len() >= 2 {
        let lo = defined.first().unwrap();
        let hi = defined.last().unwrap();
        let ratio = if lo.cv.unwrap().abs() > 0.0 {
            Some(round_to(
                hi.cv.unwrap().abs() / lo.cv.unwrap().abs(),
                decimals,
            ))
        } else {
            None
        };
        (Some(lo.label.clone()), Some(hi.label.clone()), ratio)
    } else {
        (None, None, None)
    };

    let mut warnings: Vec<String> = Vec::new();
    for d in &datasets {
        for note in &d.notes {
            let w = format!("{}: {}", d.label, note);
            if !warnings.contains(&w) {
                warnings.push(w);
            }
        }
    }

    Ok(Report {
        basis: if basis == "sample" {
            "sample (n-1)".into()
        } else {
            "population (N)".into()
        },
        dataset_count: datasets.len(),
        datasets,
        most_consistent,
        most_variable,
        spread_ratio,
        warnings,
    })
}

fn render_summary(r: &Report, decimals: u32) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Coefficient of variation — {} dataset{} ({} basis)\n",
        r.dataset_count,
        if r.dataset_count == 1 { "" } else { "s" },
        r.basis
    ));
    for d in &r.datasets {
        let head = if r.dataset_count > 1 {
            format!("\nRank {} — {}\n", d.rank, d.label)
        } else {
            format!("\n{}\n", d.label)
        };
        out.push_str(&head);
        if d.n > 0 {
            out.push_str(&format!("  n                = {}\n", d.n));
        }
        out.push_str(&format!("  mean             = {}\n", fmt(d.mean, decimals)));
        out.push_str(&format!(
            "  std dev          = {}\n",
            if d.std_dev.is_nan() {
                "undefined".to_string()
            } else {
                fmt(d.std_dev, decimals)
            }
        ));
        if let Some(ss) = d.sum_of_squares {
            out.push_str(&format!("  sum of squares   = {}\n", fmt(ss, decimals)));
        }
        if let (Some(min), Some(max)) = (d.min, d.max) {
            out.push_str(&format!(
                "  min / max        = {} / {}\n",
                fmt(min, decimals),
                fmt(max, decimals)
            ));
        }
        out.push_str(&format!(
            "  CV               = {}\n",
            fmt_opt(d.cv, decimals)
        ));
        out.push_str(&format!(
            "  CV %             = {}\n",
            d.cv_percent
                .map(|p| format!("{}%", fmt(p, decimals)))
                .unwrap_or_else(|| "undefined".into())
        ));
        out.push_str(&format!("  relative spread  = {}\n", d.relative_spread));
        if !d.outliers_removed.is_empty() {
            out.push_str(&format!(
                "  outliers removed = {}\n",
                d.outliers_removed
                    .iter()
                    .map(|v| fmt(*v, decimals))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    if let (Some(lo), Some(hi)) = (&r.most_consistent, &r.most_variable) {
        out.push_str("\nComparison\n");
        out.push_str(&format!("  Most consistent  : {lo}\n"));
        out.push_str(&format!("  Most variable    : {hi}\n"));
        if let Some(ratio) = r.spread_ratio {
            out.push_str(&format!(
                "  Spread ratio     = {}x ({hi} varies that much more, relative to its own mean, than {lo})\n",
                fmt(ratio, decimals)
            ));
        }
    }
    if !r.warnings.is_empty() {
        out.push_str("\nNotes\n");
        for w in &r.warnings {
            out.push_str(&format!("  - {w}\n"));
        }
    }
    out
}

fn render_table(r: &Report, decimals: u32) -> String {
    let mut out = String::new();
    out.push_str(&format!("Basis: {}\n\n", r.basis));
    out.push_str("| Rank | Dataset | n | Mean | Std dev | CV | CV % | Relative spread |\n");
    out.push_str("| ---: | --- | ---: | ---: | ---: | ---: | ---: | --- |\n");
    for d in &r.datasets {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            d.rank,
            d.label,
            if d.n > 0 {
                d.n.to_string()
            } else {
                "—".into()
            },
            fmt(d.mean, decimals),
            if d.std_dev.is_nan() {
                "undefined".into()
            } else {
                fmt(d.std_dev, decimals)
            },
            fmt_opt(d.cv, decimals),
            d.cv_percent
                .map(|p| format!("{}%", fmt(p, decimals)))
                .unwrap_or_else(|| "undefined".into()),
            d.relative_spread,
        ));
    }
    if let (Some(lo), Some(hi)) = (&r.most_consistent, &r.most_variable) {
        out.push_str(&format!("\nMost consistent: {lo} · Most variable: {hi}"));
        if let Some(ratio) = r.spread_ratio {
            out.push_str(&format!(" · Spread ratio {}x", fmt(ratio, decimals)));
        }
        out.push('\n');
    }
    if !r.warnings.is_empty() {
        out.push_str("\nNotes:\n");
        for w in &r.warnings {
            out.push_str(&format!("- {w}\n"));
        }
    }
    out
}

/// The one entry point every surface calls: chat, CLI, and the browser page.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    basis: &str,
    grouping: &str,
    delimiter: &str,
    mean_in: Option<f64>,
    std_dev_in: Option<f64>,
    exclude_outliers: bool,
    ignore_non_numeric: bool,
    decimals: f64,
    output: &str,
) -> Result<String, String> {
    let dec = decimals.clamp(0.0, 10.0).round() as u32;
    let report = analyze(
        data,
        basis,
        grouping,
        delimiter,
        mean_in,
        std_dev_in,
        exclude_outliers,
        ignore_non_numeric,
        decimals,
    )?;
    match output.trim().to_ascii_lowercase().as_str() {
        "" | "summary" => Ok(render_summary(&report, dec)),
        "table" => Ok(render_table(&report, dec)),
        "json" => serde_json::to_string_pretty(&report).map_err(|e| e.to_string()),
        other => Err(format!(
            "output must be summary, table or json, got `{other}`"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(data: &str) -> Report {
        analyze(
            data, "sample", "auto", "auto", None, None, false, false, 4.0,
        )
        .unwrap()
    }

    #[test]
    fn ranks_two_datasets_by_relative_dispersion() {
        // Same units, wildly different scales: the small-mean set has the far
        // larger CV even though its absolute spread is smaller.
        let r = analyze(
            "Kittens: 4.2 5.1 4.8 5.6 5.1\nOxen: 792 800 803 795 797",
            "sample",
            "auto",
            "auto",
            None,
            None,
            false,
            false,
            4.0,
        )
        .unwrap();
        assert_eq!(r.dataset_count, 2);
        assert_eq!(r.datasets[0].label, "Oxen");
        assert_eq!(r.datasets[0].rank, 1);
        assert_eq!(r.datasets[1].label, "Kittens");
        assert!(r.datasets[0].cv.unwrap() < r.datasets[1].cv.unwrap());
        assert_eq!(r.most_consistent.as_deref(), Some("Oxen"));
        assert_eq!(r.most_variable.as_deref(), Some("Kittens"));
        assert!(r.spread_ratio.unwrap() > 1.0);
    }

    #[test]
    fn sample_and_population_bases_differ() {
        let s = one("2, 4, 4, 4, 5, 5, 7, 9");
        // Population sd of this classic set is exactly 2, mean 5 → CV 0.4.
        let p = analyze(
            "2, 4, 4, 4, 5, 5, 7, 9",
            "population",
            "auto",
            "auto",
            None,
            None,
            false,
            false,
            4.0,
        )
        .unwrap();
        assert_eq!(p.datasets[0].mean, 5.0);
        assert_eq!(p.datasets[0].std_dev, 2.0);
        assert_eq!(p.datasets[0].cv, Some(0.4));
        assert_eq!(p.datasets[0].cv_percent, Some(40.0));
        // Sample sd = sqrt(32/7) ≈ 2.1381 → a larger CV.
        assert!(s.datasets[0].cv.unwrap() > p.datasets[0].cv.unwrap());
        assert_eq!(s.datasets[0].basis, "sample (n-1)");
    }

    #[test]
    fn auto_grouping_pools_a_single_column() {
        let r = one("2\n4\n4\n4\n5\n5\n7\n9");
        assert_eq!(r.dataset_count, 1);
        assert_eq!(r.datasets[0].n, 8);
        assert_eq!(r.datasets[0].mean, 5.0);
    }

    #[test]
    fn lines_grouping_forces_one_dataset_per_row() {
        let r = analyze(
            "1 2 3\n10 20 30",
            "sample",
            "lines",
            "auto",
            None,
            None,
            false,
            false,
            4.0,
        )
        .unwrap();
        assert_eq!(r.dataset_count, 2);
        // Proportional scaling leaves the CV identical, so both tie.
        assert_eq!(r.datasets[0].cv, r.datasets[1].cv);
        assert_eq!(r.spread_ratio, Some(1.0));
    }

    #[test]
    fn single_grouping_pools_multi_value_rows() {
        let r = analyze(
            "1 2 3\n4 5 6",
            "population",
            "single",
            "auto",
            None,
            None,
            false,
            false,
            4.0,
        )
        .unwrap();
        assert_eq!(r.dataset_count, 1);
        assert_eq!(r.datasets[0].n, 6);
        assert_eq!(r.datasets[0].mean, 3.5);
    }

    #[test]
    fn summary_mode_matches_the_published_worked_example() {
        // sd 0.783, mean 23.41 → CV 0.0334 = 3.34%.
        let r = analyze(
            "",
            "sample",
            "auto",
            "auto",
            Some(23.41),
            Some(0.783),
            false,
            false,
            4.0,
        )
        .unwrap();
        assert_eq!(r.datasets[0].cv, Some(0.0334));
        assert_eq!(r.datasets[0].cv_percent, Some(3.3447));
        assert_eq!(r.datasets[0].basis, "given");
        assert_eq!(r.datasets[0].relative_spread, "low");
    }

    #[test]
    fn zero_mean_gives_an_undefined_cv_not_an_error() {
        let r = analyze(
            "-2 -1 0 1 2",
            "sample",
            "auto",
            "auto",
            None,
            None,
            false,
            false,
            4.0,
        )
        .unwrap();
        assert!(r.datasets[0].cv.is_none());
        assert_eq!(r.datasets[0].relative_spread, "undefined");
        assert!(r.warnings.iter().any(|w| w.contains("undefined")));
    }

    #[test]
    fn undefined_cv_datasets_rank_last() {
        let r = analyze(
            "Zeroed: -2 -1 0 1 2\nSteady: 10 11 10 11",
            "sample",
            "auto",
            "auto",
            None,
            None,
            false,
            false,
            4.0,
        )
        .unwrap();
        assert_eq!(r.datasets[0].label, "Steady");
        assert_eq!(r.datasets[1].label, "Zeroed");
        assert_eq!(r.datasets[1].rank, 2);
        // Only one dataset has a defined CV, so there is nothing to compare.
        assert!(r.most_consistent.is_none());
    }

    #[test]
    fn outlier_filter_removes_the_tukey_fence_values() {
        let base = "10 11 10 12 11 10 11 90";
        let kept = one(base);
        let filtered =
            analyze(base, "sample", "auto", "auto", None, None, true, false, 4.0).unwrap();
        assert_eq!(filtered.datasets[0].outliers_removed, vec![90.0]);
        assert_eq!(filtered.datasets[0].n, 7);
        assert!(filtered.datasets[0].cv.unwrap() < kept.datasets[0].cv.unwrap());
    }

    #[test]
    fn ignore_non_numeric_skips_spreadsheet_junk() {
        let data = "Machine A: 4.2, n/a, 5.1, , 4.8";
        assert!(analyze(data, "sample", "auto", "auto", None, None, false, false, 4.0).is_err());
        let r = analyze(data, "sample", "auto", "auto", None, None, false, true, 4.0).unwrap();
        assert_eq!(r.datasets[0].n, 3);
        assert_eq!(r.datasets[0].label, "Machine A");
    }

    #[test]
    fn delimiters_and_comments_parse() {
        for (delim, data) in [
            ("comma", "1,2,3,4"),
            ("semicolon", "1;2;3;4"),
            ("pipe", "1|2|3|4"),
            ("space", "1 2 3 4"),
            ("tab", "1\t2\t3\t4"),
            ("auto", "1, 2; 3 4"),
        ] {
            let r = analyze(
                data,
                "population",
                "lines",
                delim,
                None,
                None,
                false,
                false,
                4.0,
            )
            .unwrap_or_else(|e| panic!("{delim}: {e}"));
            assert_eq!(r.datasets[0].n, 4, "{delim}");
            assert_eq!(r.datasets[0].mean, 2.5, "{delim}");
        }
        let r = one("# a comment row\n2 4 4 4 5 5 7 9");
        assert_eq!(r.datasets[0].n, 8);
    }

    #[test]
    fn single_reading_needs_the_population_basis() {
        let s = one("42");
        assert!(s.datasets[0].cv.is_none());
        assert!(s.datasets[0].notes.iter().any(|n| n.contains("at least 2")));
        let p = analyze(
            "42",
            "population",
            "auto",
            "auto",
            None,
            None,
            false,
            false,
            4.0,
        )
        .unwrap();
        assert_eq!(p.datasets[0].cv, Some(0.0));
    }

    #[test]
    fn decimals_control_the_rounding() {
        let r = analyze(
            "2 4 4 4 5 5 7 9",
            "population",
            "auto",
            "auto",
            None,
            None,
            false,
            false,
            1.0,
        )
        .unwrap();
        assert_eq!(r.datasets[0].cv_percent, Some(40.0));
        let out = run(
            "2 4 4 4 5 5 7 9",
            "population",
            "auto",
            "auto",
            None,
            None,
            false,
            false,
            0.0,
            "summary",
        )
        .unwrap();
        assert!(out.contains("CV               = 0\n"), "{out}");
    }

    #[test]
    fn output_formats_render() {
        let args = (
            "Kittens: 4.2 5.1 4.8\nOxen: 792 800 803",
            "sample",
            "auto",
            "auto",
        );
        let summary = run(
            args.0, args.1, args.2, args.3, None, None, false, false, 4.0, "summary",
        )
        .unwrap();
        assert!(summary.contains("Rank 1 — Oxen"), "{summary}");
        assert!(summary.contains("Most consistent  : Oxen"), "{summary}");
        let table = run(
            args.0, args.1, args.2, args.3, None, None, false, false, 4.0, "table",
        )
        .unwrap();
        assert!(table.contains("| Rank | Dataset | n | Mean |"), "{table}");
        assert!(table.contains("| 1 | Oxen |"), "{table}");
        let json = run(
            args.0, args.1, args.2, args.3, None, None, false, false, 4.0, "json",
        )
        .unwrap();
        assert!(json.contains("\"cv_percent\""), "{json}");
        assert!(json.contains("\"most_consistent\": \"Oxen\""), "{json}");
    }

    #[test]
    fn errors() {
        // Nothing at all to work with.
        assert!(
            run("", "sample", "auto", "auto", None, None, false, false, 4.0, "summary").is_err()
        );
        // Half a summary input.
        assert!(analyze(
            "",
            "sample",
            "auto",
            "auto",
            Some(10.0),
            None,
            false,
            false,
            4.0
        )
        .is_err());
        assert!(analyze(
            "",
            "sample",
            "auto",
            "auto",
            None,
            Some(2.0),
            false,
            false,
            4.0
        )
        .is_err());
        // A negative standard deviation is not a thing.
        assert!(analyze(
            "",
            "sample",
            "auto",
            "auto",
            Some(10.0),
            Some(-1.0),
            false,
            false,
            4.0
        )
        .is_err());
        // Unparseable token with the strict default.
        let e = analyze(
            "1 2 banana",
            "sample",
            "auto",
            "auto",
            None,
            None,
            false,
            false,
            4.0,
        )
        .unwrap_err();
        assert!(e.contains("banana"), "{e}");
        assert!(e.contains("ignore_non_numeric"), "{e}");
        // Bad enum values.
        assert!(analyze("1 2 3", "nope", "auto", "auto", None, None, false, false, 4.0).is_err());
        assert!(analyze("1 2 3", "sample", "nope", "auto", None, None, false, false, 4.0).is_err());
        assert!(analyze("1 2 3", "sample", "auto", "nope", None, None, false, false, 4.0).is_err());
        assert!(
            run("1 2 3", "sample", "auto", "auto", None, None, false, false, 4.0, "nope").is_err()
        );
        // A labelled row with no numbers.
        assert!(analyze(
            "Machine A:\n1 2 3",
            "sample",
            "lines",
            "auto",
            None,
            None,
            false,
            false,
            4.0
        )
        .is_err());
    }

    #[test]
    fn value_cap_is_enforced_at_the_boundary() {
        let at_cap = (0..MAX_VALUES)
            .map(|i| (i % 7 + 1).to_string())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(analyze(&at_cap, "sample", "auto", "auto", None, None, false, false, 4.0).is_ok());
        let over = format!("{at_cap} 5");
        let e = analyze(
            &over, "sample", "auto", "auto", None, None, false, false, 4.0,
        )
        .unwrap_err();
        assert!(e.contains("exceeds"), "{e}");
    }
}
