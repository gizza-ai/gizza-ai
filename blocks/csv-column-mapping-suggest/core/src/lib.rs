//! csv-column-mapping-suggest core — suggest a one-to-one mapping between two CSV headers.

use csv::ReaderBuilder;
use serde::Serialize;
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delimiter {
    Comma,
    Tab,
    Semicolon,
    Pipe,
}

impl Delimiter {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "tab" | "\\t" => Delimiter::Tab,
            "semicolon" | ";" => Delimiter::Semicolon,
            "pipe" | "|" => Delimiter::Pipe,
            _ => Delimiter::Comma,
        }
    }
    fn byte(self) -> u8 {
        match self {
            Delimiter::Comma => b',',
            Delimiter::Tab => b'\t',
            Delimiter::Semicolon => b';',
            Delimiter::Pipe => b'|',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" => OutputFormat::Json,
            "csv" => OutputFormat::Csv,
            _ => OutputFormat::Table,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub delimiter: Delimiter,
    pub header: bool,
    pub sample_rows: usize,
    pub header_weight: f64,
    pub threshold: f64,
    pub format: OutputFormat,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            delimiter: Delimiter::Comma,
            header: true,
            sample_rows: 50,
            header_weight: 0.6,
            threshold: 0.30,
            format: OutputFormat::Table,
        }
    }
}

#[derive(Debug)]
struct CsvData {
    headers: Vec<String>,
    values: Vec<Vec<String>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Suggestion {
    pub source_column: String,
    pub target_column: String,
    pub score: f64,
    pub header_score: f64,
    pub value_score: f64,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct MappingReport {
    pub suggestions: Vec<Suggestion>,
    pub unmapped_source_columns: Vec<String>,
    pub unmapped_target_columns: Vec<String>,
}

fn parse_csv(text: &str, opts: &Options, label: &str) -> Result<CsvData, String> {
    if text.trim().is_empty() {
        return Err(format!("{label} CSV is empty"));
    }
    let mut rdr = ReaderBuilder::new()
        .delimiter(opts.delimiter.byte())
        .has_headers(opts.header)
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = if opts.header {
        rdr.headers()
            .map_err(|e| format!("{label} CSV header parse error: {e}"))?
            .iter()
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    } else {
        let first = rdr
            .records()
            .next()
            .ok_or_else(|| format!("{label} CSV has no rows"))?
            .map_err(|e| format!("{label} CSV parse error: {e}"))?;
        let cols = first.len();
        let mut rows = vec![first
            .iter()
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()];
        for rec in rdr.records().take(opts.sample_rows.saturating_sub(1)) {
            let rec = rec.map_err(|e| format!("{label} CSV parse error: {e}"))?;
            rows.push(rec.iter().map(|s| s.trim().to_string()).collect());
        }
        return Ok(CsvData {
            headers: (1..=cols).map(|i| format!("column_{i}")).collect(),
            values: rows,
        });
    };

    if headers.is_empty() {
        return Err(format!("{label} CSV has no columns"));
    }
    let mut rows = Vec::new();
    for rec in rdr.records().take(opts.sample_rows) {
        let rec = rec.map_err(|e| format!("{label} CSV parse error: {e}"))?;
        rows.push(rec.iter().map(|s| s.trim().to_string()).collect());
    }
    Ok(CsvData {
        headers,
        values: rows,
    })
}

fn normalize(s: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in s.chars() {
        if ch.is_ascii_uppercase() && prev_lower {
            out.push(' ');
        }
        if ch.is_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            out.push(' ');
            prev_lower = false;
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tokens(s: &str) -> HashSet<String> {
    normalize(s)
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn bigrams(s: &str) -> HashSet<String> {
    let compact: String = normalize(s)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let chars: Vec<char> = compact.chars().collect();
    if chars.len() < 2 {
        return if compact.is_empty() {
            HashSet::new()
        } else {
            [compact].into_iter().collect()
        };
    }
    chars.windows(2).map(|w| w.iter().collect()).collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let inter = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

fn dice(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let inter = a.intersection(b).count() as f64;
    let denom = (a.len() + b.len()) as f64;
    if denom == 0.0 {
        0.0
    } else {
        2.0 * inter / denom
    }
}

fn header_score(a: &str, b: &str) -> f64 {
    let na = normalize(a);
    let nb = normalize(b);
    if na == nb {
        return 1.0;
    }
    jaccard(&tokens(a), &tokens(b)).max(dice(&bigrams(a), &bigrams(b)))
}

fn col_values(data: &CsvData, idx: usize) -> HashSet<String> {
    data.values
        .iter()
        .filter_map(|row| row.get(idx))
        .map(|v| normalize(v))
        .filter(|v| !v.is_empty())
        .collect()
}

fn value_score(src: &CsvData, si: usize, tgt: &CsvData, ti: usize) -> f64 {
    jaccard(&col_values(src, si), &col_values(tgt, ti))
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

pub fn suggest(
    source_csv: &str,
    target_csv: &str,
    opts: &Options,
) -> Result<MappingReport, String> {
    let source = parse_csv(source_csv, opts, "source")?;
    let target = parse_csv(target_csv, opts, "target")?;
    let hw = opts.header_weight.clamp(0.0, 1.0);
    let threshold = opts.threshold.clamp(0.0, 1.0);

    let mut candidates = Vec::new();
    for (si, sh) in source.headers.iter().enumerate() {
        for (ti, th) in target.headers.iter().enumerate() {
            let hs = header_score(sh, th);
            let vs = if opts.sample_rows == 0 {
                0.0
            } else {
                value_score(&source, si, &target, ti)
            };
            let score = if opts.sample_rows == 0 {
                hs
            } else {
                hw * hs + (1.0 - hw) * vs
            };
            candidates.push((score, hs, vs, si, ti));
        }
    }
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut used_source = BTreeSet::new();
    let mut used_target = BTreeSet::new();
    let mut suggestions = Vec::new();
    for (score, hs, vs, si, ti) in candidates {
        if score + f64::EPSILON < threshold
            || used_source.contains(&si)
            || used_target.contains(&ti)
        {
            continue;
        }
        used_source.insert(si);
        used_target.insert(ti);
        suggestions.push(Suggestion {
            source_column: source.headers[si].clone(),
            target_column: target.headers[ti].clone(),
            score: round3(score),
            header_score: round3(hs),
            value_score: round3(vs),
            reason: format!("header {:.2}, value {:.2}", hs, vs),
        });
    }
    suggestions.sort_by(|a, b| a.source_column.cmp(&b.source_column));

    let unmapped_source_columns = source
        .headers
        .iter()
        .enumerate()
        .filter(|(i, _)| !used_source.contains(i))
        .map(|(_, h)| h.clone())
        .collect();
    let unmapped_target_columns = target
        .headers
        .iter()
        .enumerate()
        .filter(|(i, _)| !used_target.contains(i))
        .map(|(_, h)| h.clone())
        .collect();

    Ok(MappingReport {
        suggestions,
        unmapped_source_columns,
        unmapped_target_columns,
    })
}

pub fn run(source_csv: &str, target_csv: &str, opts: &Options) -> Result<String, String> {
    let report = suggest(source_csv, target_csv, opts)?;
    match opts.format {
        OutputFormat::Json => serde_json::to_string_pretty(&report).map_err(|e| e.to_string()),
        OutputFormat::Csv => {
            let mut out =
                String::from("source_column,target_column,score,header_score,value_score,reason\n");
            for s in report.suggestions {
                out.push_str(&format!(
                    "{},{},{:.3},{:.3},{:.3},{}\n",
                    s.source_column,
                    s.target_column,
                    s.score,
                    s.header_score,
                    s.value_score,
                    s.reason
                ));
            }
            Ok(out)
        }
        OutputFormat::Table => {
            let mut out = String::from(
                "Source column | Target column | Score | Reason\n--- | --- | ---: | ---\n",
            );
            for s in &report.suggestions {
                out.push_str(&format!(
                    "{} | {} | {:.3} | {}\n",
                    s.source_column, s.target_column, s.score, s.reason
                ));
            }
            if report.suggestions.is_empty() {
                out.push_str("(no suggestions above threshold) |  |  | Lower the threshold or inspect headers manually.\n");
            }
            out.push_str(&format!(
                "\nUnmapped source columns: {}\n",
                if report.unmapped_source_columns.is_empty() {
                    "(none)".into()
                } else {
                    report.unmapped_source_columns.join(", ")
                }
            ));
            out.push_str(&format!(
                "Unmapped target columns: {}",
                if report.unmapped_target_columns.is_empty() {
                    "(none)".into()
                } else {
                    report.unmapped_target_columns.join(", ")
                }
            ));
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_fuzzy_headers_map() {
        let src =
            "First Name,Email Address,Zip Code\nAda,a@example.com,02139\nBo,b@example.com,94107\n";
        let tgt = "email,postal_code,first_name\na@example.com,02139,Ada\nb@example.com,94107,Bo\n";
        let report = suggest(src, tgt, &Options::default()).unwrap();
        assert_eq!(
            report
                .suggestions
                .iter()
                .map(|s| (s.source_column.as_str(), s.target_column.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("Email Address", "email"),
                ("First Name", "first_name"),
                ("Zip Code", "postal_code")
            ]
        );
    }

    #[test]
    fn value_overlap_can_rescue_weak_header_match() {
        let src = "customer_id\nA1\nB2\nC3\n";
        let tgt = "account\nA1\nB2\nC3\n";
        let report = suggest(
            src,
            tgt,
            &Options {
                header_weight: 0.2,
                threshold: 0.5,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(report.suggestions[0].target_column, "account");
        assert!(report.suggestions[0].value_score > 0.99);
    }

    #[test]
    fn threshold_leaves_unmapped() {
        let report = suggest(
            "a\n1\n",
            "z\n9\n",
            &Options {
                threshold: 0.95,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(report.suggestions.is_empty());
        assert_eq!(report.unmapped_source_columns, vec!["a"]);
        assert_eq!(report.unmapped_target_columns, vec!["z"]);
    }

    #[test]
    fn table_output_is_stable() {
        let out = run(
            "name\nAda\n",
            "full_name\nAda\n",
            &Options {
                format: OutputFormat::Table,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(out.contains("Source column | Target column | Score | Reason"));
        assert!(out.contains("name | full_name"));
    }

    #[test]
    fn rejects_empty_csv() {
        assert!(run("", "a\n1\n", &Options::default())
            .unwrap_err()
            .contains("source CSV is empty"));
    }
}
