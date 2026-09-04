//! vector-similarity core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Parses a query vector plus a list of candidate vectors, scores every candidate with the
//! selected metric (cosine, cosine distance, dot product, Euclidean, Manhattan, Chebyshev or
//! Hamming), and returns the nearest `top_k` neighbours as an aligned table, JSON or CSV.

/// Maximum number of candidate vectors accepted in one run.
pub const MAX_VECTORS: usize = 2000;
/// Maximum dimensionality accepted for the query (and therefore every candidate).
pub const MAX_DIMS: usize = 8192;

const METRICS: [&str; 7] = [
    "cosine",
    "cosine_distance",
    "dot",
    "euclidean",
    "manhattan",
    "chebyshev",
    "hamming",
];

/// Every metric shown in the all-metrics table, in display order.
const TABLE_METRICS: [&str; 6] = [
    "cosine",
    "dot",
    "euclidean",
    "manhattan",
    "chebyshev",
    "hamming",
];

/// A parsed candidate vector.
struct Vector {
    label: String,
    line: usize,
    index: usize,
    values: Vec<f64>,
    magnitude: f64,
}

/// All metric values for one candidate. `cosine` is `None` when either magnitude is zero.
struct Scores {
    cosine: Option<f64>,
    dot: f64,
    euclidean: f64,
    manhattan: f64,
    chebyshev: f64,
    hamming: usize,
}

impl Scores {
    fn get(&self, metric: &str) -> Option<f64> {
        match metric {
            "cosine" => self.cosine,
            "cosine_distance" => self.cosine.map(|c| 1.0 - c),
            "dot" => Some(self.dot),
            "euclidean" => Some(self.euclidean),
            "manhattan" => Some(self.manhattan),
            "chebyshev" => Some(self.chebyshev),
            "hamming" => Some(self.hamming as f64),
            _ => None,
        }
    }
}

fn higher_is_better(metric: &str) -> bool {
    matches!(metric, "cosine" | "dot")
}

fn metric_label(metric: &str) -> &'static str {
    match metric {
        "cosine" => "cosine similarity",
        "cosine_distance" => "cosine distance",
        "dot" => "dot product",
        "euclidean" => "Euclidean distance (L2)",
        "manhattan" => "Manhattan distance (L1)",
        "chebyshev" => "Chebyshev distance (L-inf)",
        "hamming" => "Hamming distance",
        _ => "unknown metric",
    }
}

/// Format a float with `decimals` places, turning `-0.000` into `0.000`.
fn fmt_num(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

fn parse_values(raw: &str, what: &str) -> Result<Vec<f64>, String> {
    let cleaned: String = raw
        .chars()
        .map(|c| match c {
            '[' | ']' | '(' | ')' | '{' | '}' | ',' | ';' | '"' | '\'' => ' ',
            c => c,
        })
        .collect();
    let mut out = Vec::new();
    for token in cleaned.split_whitespace() {
        let v: f64 = token
            .parse()
            .map_err(|_| format!("{what}: '{token}' is not a number"))?;
        if !v.is_finite() {
            return Err(format!("{what}: '{token}' is not a finite number"));
        }
        out.push(v);
    }
    Ok(out)
}

/// Split an optional `label: values` prefix off a line. A prefix that itself parses as a
/// number is kept as data, so `1:2` is not mistaken for a label.
fn split_label(line: &str) -> (Option<String>, &str) {
    if let Some(pos) = line.find(':') {
        let (head, tail) = line.split_at(pos);
        let head = head.trim();
        if !head.is_empty() && head.parse::<f64>().is_err() {
            return (Some(head.to_string()), &tail[1..]);
        }
    }
    (None, line)
}

fn magnitude(values: &[f64]) -> f64 {
    values.iter().map(|v| v * v).sum::<f64>().sqrt()
}

fn parse_query(query: &str) -> Result<Vec<f64>, String> {
    let (_, body) = split_label(query.trim());
    let values = parse_values(body, "query vector")?;
    if values.is_empty() {
        return Err("query vector is empty — enter at least one number".into());
    }
    if values.len() > MAX_DIMS {
        return Err(format!(
            "query vector has {} dimensions (maximum {MAX_DIMS})",
            values.len()
        ));
    }
    Ok(values)
}

fn parse_vectors(vectors: &str, dims: usize) -> Result<Vec<Vector>, String> {
    let mut out: Vec<Vector> = Vec::new();
    for (i, raw) in vectors.lines().enumerate() {
        let line_no = i + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (label, body) = split_label(trimmed);
        let index = out.len() + 1;
        let name = label.unwrap_or_else(|| format!("v{index}"));
        let values = parse_values(body, &format!("vector {index} '{name}' (line {line_no})"))?;
        if values.len() != dims {
            return Err(format!(
                "vector {index} '{name}' (line {line_no}) has {} values but the query has {dims} — every vector needs the same number of dimensions",
                values.len()
            ));
        }
        if out.len() == MAX_VECTORS {
            return Err(format!(
                "too many vectors (maximum {MAX_VECTORS}) — split the list into smaller batches"
            ));
        }
        let magnitude = magnitude(&values);
        out.push(Vector {
            label: name,
            line: line_no,
            index,
            values,
            magnitude,
        });
    }
    if out.is_empty() {
        return Err("no vectors to compare — enter one vector per line".into());
    }
    Ok(out)
}

fn score(query: &[f64], q_mag: f64, v: &Vector, hamming_tolerance: f64) -> Scores {
    let mut dot = 0.0;
    let mut sq = 0.0;
    let mut manhattan = 0.0;
    let mut chebyshev: f64 = 0.0;
    let mut hamming = 0usize;
    for (a, b) in query.iter().zip(v.values.iter()) {
        let d = a - b;
        dot += a * b;
        sq += d * d;
        manhattan += d.abs();
        chebyshev = chebyshev.max(d.abs());
        if d.abs() > hamming_tolerance {
            hamming += 1;
        }
    }
    let cosine = if q_mag == 0.0 || v.magnitude == 0.0 {
        None
    } else {
        Some(dot / (q_mag * v.magnitude))
    };
    Scores {
        cosine,
        dot,
        euclidean: sq.sqrt(),
        manhattan,
        chebyshev,
        hamming,
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Column order: the ranking metric first, then every remaining metric when
/// `show_all_metrics` is on.
fn columns(metric: &str, show_all_metrics: bool) -> Vec<String> {
    let mut cols = vec![metric.to_string()];
    if show_all_metrics {
        for m in TABLE_METRICS {
            if m != metric {
                cols.push(m.to_string());
            }
        }
    }
    cols
}

fn cell(scores: &Scores, column: &str, decimals: usize) -> Option<String> {
    if column == "hamming" {
        return Some(scores.hamming.to_string());
    }
    scores.get(column).map(|v| fmt_num(v, decimals))
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    query: &str,
    vectors: &str,
    metric: &str,
    top_k: usize,
    normalize: bool,
    hamming_tolerance: f64,
    decimals: usize,
    show_all_metrics: bool,
    output: &str,
) -> Result<String, String> {
    let metric = metric.trim().to_ascii_lowercase();
    let metric = metric.as_str();
    if !METRICS.contains(&metric) {
        return Err(format!(
            "unknown metric '{metric}' — choose one of {}",
            METRICS.join(", ")
        ));
    }
    let output_kind = output.trim().to_ascii_lowercase();
    let output_kind = if output_kind.is_empty() {
        "table"
    } else {
        output_kind.as_str()
    };
    if !matches!(output_kind, "table" | "json" | "csv") {
        return Err(format!(
            "unknown output '{output_kind}' — choose table, json or csv"
        ));
    }
    if top_k == 0 {
        return Err("top_k must be at least 1".into());
    }
    if decimals > 12 {
        return Err("decimals must be between 0 and 12".into());
    }
    if !hamming_tolerance.is_finite() || hamming_tolerance < 0.0 {
        return Err("hamming_tolerance must be zero or a positive number".into());
    }

    let mut query_values = parse_query(query)?;
    let mut candidates = parse_vectors(vectors, query_values.len())?;

    let mut q_mag = magnitude(&query_values);
    if normalize {
        if q_mag == 0.0 {
            return Err("cannot L2-normalize the query vector: its magnitude is zero".into());
        }
        for v in query_values.iter_mut() {
            *v /= q_mag;
        }
        for c in candidates.iter_mut() {
            if c.magnitude == 0.0 {
                return Err(format!(
                    "cannot L2-normalize vector {} '{}' (line {}): its magnitude is zero",
                    c.index, c.label, c.line
                ));
            }
            for v in c.values.iter_mut() {
                *v /= c.magnitude;
            }
            c.magnitude = 1.0;
        }
        q_mag = 1.0;
    }

    let uses_cosine = matches!(metric, "cosine" | "cosine_distance");
    if uses_cosine && q_mag == 0.0 {
        return Err(format!(
            "{} is undefined for a zero-magnitude query vector",
            metric_label(metric)
        ));
    }
    if uses_cosine {
        if let Some(c) = candidates.iter().find(|c| c.magnitude == 0.0) {
            return Err(format!(
                "{} is undefined for zero-magnitude vector {} '{}' (line {})",
                metric_label(metric),
                c.index,
                c.label,
                c.line
            ));
        }
    }

    let total = candidates.len();
    let mut scored: Vec<(usize, Scores)> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| (i, score(&query_values, q_mag, c, hamming_tolerance)))
        .collect();

    let desc = higher_is_better(metric);
    scored.sort_by(|a, b| {
        let (sa, sb) = (
            a.1.get(metric).unwrap_or(f64::NAN),
            b.1.get(metric).unwrap_or(f64::NAN),
        );
        let ord = sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal);
        let ord = if desc { ord.reverse() } else { ord };
        ord.then(a.0.cmp(&b.0))
    });
    let shown = top_k.min(total);
    scored.truncate(shown);

    let cols = columns(metric, show_all_metrics);
    let dims = query_values.len();

    match output_kind {
        "json" => {
            let mut s = String::from("{\n");
            s.push_str(&format!("  \"metric\": \"{metric}\",\n"));
            s.push_str(&format!("  \"higher_is_better\": {desc},\n"));
            s.push_str(&format!("  \"normalized\": {normalize},\n"));
            s.push_str(&format!("  \"dimensions\": {dims},\n"));
            s.push_str(&format!(
                "  \"query_magnitude\": {},\n",
                fmt_num(q_mag, decimals)
            ));
            s.push_str(&format!("  \"vectors_compared\": {total},\n"));
            s.push_str(&format!("  \"returned\": {shown},\n"));
            s.push_str("  \"results\": [\n");
            for (rank, (idx, sc)) in scored.iter().enumerate() {
                let c = &candidates[*idx];
                s.push_str("    {\n");
                s.push_str(&format!("      \"rank\": {},\n", rank + 1));
                s.push_str(&format!("      \"label\": \"{}\",\n", json_escape(&c.label)));
                s.push_str(&format!("      \"index\": {},\n", c.index));
                let score_cell = cell(sc, metric, decimals).unwrap_or_else(|| "null".into());
                s.push_str(&format!("      \"score\": {score_cell},\n"));
                for (i, col) in cols.iter().enumerate() {
                    let value = cell(sc, col, decimals).unwrap_or_else(|| "null".into());
                    let comma = if i + 1 == cols.len() { "" } else { "," };
                    s.push_str(&format!("      \"{col}\": {value}{comma}\n"));
                }
                let comma = if rank + 1 == scored.len() { "" } else { "," };
                s.push_str(&format!("    }}{comma}\n"));
            }
            s.push_str("  ]\n}\n");
            Ok(s)
        }
        "csv" => {
            let mut s = String::new();
            s.push_str("rank,label,");
            s.push_str(&cols.join(","));
            s.push('\n');
            for (rank, (idx, sc)) in scored.iter().enumerate() {
                let c = &candidates[*idx];
                let mut row = vec![(rank + 1).to_string(), csv_escape(&c.label)];
                for col in &cols {
                    row.push(cell(sc, col, decimals).unwrap_or_default());
                }
                s.push_str(&row.join(","));
                s.push('\n');
            }
            Ok(s)
        }
        _ => {
            let mut header = vec!["rank".to_string(), "label".to_string()];
            header.extend(cols.iter().cloned());
            let mut rows: Vec<Vec<String>> = vec![header];
            for (rank, (idx, sc)) in scored.iter().enumerate() {
                let c = &candidates[*idx];
                let mut row = vec![(rank + 1).to_string(), c.label.clone()];
                for col in &cols {
                    row.push(cell(sc, col, decimals).unwrap_or_else(|| "undefined".into()));
                }
                rows.push(row);
            }
            let width = |i: usize| rows.iter().map(|r| r[i].chars().count()).max().unwrap_or(0);
            let widths: Vec<usize> = (0..rows[0].len()).map(width).collect();

            let mut s = String::new();
            s.push_str(&format!(
                "Query: {dims} dimension{}, magnitude {}\n",
                if dims == 1 { "" } else { "s" },
                fmt_num(q_mag, decimals)
            ));
            s.push_str(&format!(
                "Metric: {} ({} is better)\n",
                metric_label(metric),
                if desc { "higher" } else { "lower" }
            ));
            if normalize {
                s.push_str("Normalized: every vector scaled to unit length before comparing\n");
            }
            if metric == "hamming" || show_all_metrics {
                s.push_str(&format!(
                    "Hamming tolerance: {}\n",
                    fmt_num(hamming_tolerance, decimals)
                ));
            }
            s.push_str(&format!(
                "Compared: {total} vector{}, showing {shown}\n\n",
                if total == 1 { "" } else { "s" }
            ));
            for row in &rows {
                let mut line = String::new();
                for (i, c) in row.iter().enumerate() {
                    if i > 0 {
                        line.push_str("  ");
                    }
                    let pad = widths[i].saturating_sub(c.chars().count());
                    // rank + metric columns read as numbers (right aligned); labels stay left.
                    if i == 1 {
                        line.push_str(c);
                        if i + 1 < row.len() {
                            line.push_str(&" ".repeat(pad));
                        }
                    } else {
                        line.push_str(&" ".repeat(pad));
                        line.push_str(c);
                    }
                }
                s.push_str(line.trim_end());
                s.push('\n');
            }
            Ok(s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VECTORS: &str = "a: 1, 2, 3\nb: 3, 2, 1\nc: -3, -2, -1\n";

    fn defaults(query: &str, vectors: &str, metric: &str) -> Result<String, String> {
        run(query, vectors, metric, 5, false, 0.0, 6, true, "table")
    }

    #[test]
    fn ranks_by_cosine_similarity() {
        let out = defaults("3, 2, 1", VECTORS, "cosine").unwrap();
        assert_eq!(
            out,
            "Query: 3 dimensions, magnitude 3.741657\n\
             Metric: cosine similarity (higher is better)\n\
             Hamming tolerance: 0.000000\n\
             Compared: 3 vectors, showing 3\n\
             \n\
             rank  label     cosine         dot  euclidean  manhattan  chebyshev  hamming\n\
             \x20  1  b       1.000000   14.000000   0.000000   0.000000   0.000000        0\n\
             \x20  2  a       0.714286   10.000000   2.828427   4.000000   2.000000        2\n\
             \x20  3  c      -1.000000  -14.000000   7.483315  12.000000   6.000000        3\n"
        );
    }

    #[test]
    fn euclidean_ranks_lowest_distance_first() {
        let out = run("3, 2, 1", VECTORS, "euclidean", 2, false, 0.0, 3, false, "csv").unwrap();
        assert_eq!(out, "rank,label,euclidean\n1,b,0.000\n2,a,2.828\n");
    }

    #[test]
    fn hamming_counts_differing_positions_within_tolerance() {
        let out = run(
            "1, 2, 3",
            "near: 1.05, 2, 3\nfar: 9, 9, 9\n",
            "hamming",
            5,
            false,
            0.1,
            2,
            false,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "rank,label,hamming\n1,near,0\n2,far,3\n");
    }

    #[test]
    fn normalize_makes_dot_product_match_cosine() {
        let out = run("3, 2, 1", VECTORS, "dot", 1, true, 0.0, 6, false, "csv").unwrap();
        assert_eq!(out, "rank,label,dot\n1,b,1.000000\n");
    }

    #[test]
    fn accepts_json_arrays_and_bare_whitespace() {
        let out = run(
            "[3, 2, 1]",
            "[1 2 3]\n[3;2;1]\n",
            "cosine",
            5,
            false,
            0.0,
            4,
            false,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "rank,label,cosine\n1,v2,1.0000\n2,v1,0.7143\n");
    }

    #[test]
    fn json_output_reports_every_metric() {
        let out = run("1, 0", "x: 0, 1\n", "cosine", 5, false, 0.0, 2, true, "json").unwrap();
        assert!(out.contains("\"metric\": \"cosine\""), "{out}");
        assert!(out.contains("\"label\": \"x\""), "{out}");
        assert!(out.contains("\"cosine\": 0.00"), "{out}");
        assert!(out.contains("\"euclidean\": 1.41"), "{out}");
        assert!(out.contains("\"hamming\": 2"), "{out}");
    }

    #[test]
    fn undefined_cosine_is_reported_when_another_metric_ranks() {
        let out = run("1, 1", "zero: 0, 0\n", "euclidean", 5, false, 0.0, 3, true, "table")
            .unwrap();
        assert!(out.contains("undefined"), "{out}");
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let out = run(
            "1, 1",
            "# ignored\n\nkeep: 1, 1\n",
            "cosine",
            5,
            false,
            0.0,
            1,
            false,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "rank,label,cosine\n1,keep,1.0\n");
    }

    #[test]
    fn dimension_mismatch_is_an_error() {
        let err = defaults("1, 2, 3", "a: 1, 2, 3\nb: 1, 2\n", "cosine").unwrap_err();
        assert_eq!(
            err,
            "vector 2 'b' (line 2) has 2 values but the query has 3 — every vector needs the same number of dimensions"
        );
    }

    #[test]
    fn non_numeric_token_is_an_error() {
        let err = defaults("1, 2", "a: 1, two\n", "cosine").unwrap_err();
        assert_eq!(err, "vector 1 'a' (line 1): 'two' is not a number");
    }

    #[test]
    fn empty_query_is_an_error() {
        let err = defaults("   ", "a: 1\n", "cosine").unwrap_err();
        assert_eq!(err, "query vector is empty — enter at least one number");
    }

    #[test]
    fn no_vectors_is_an_error() {
        let err = defaults("1, 2", "\n# only a comment\n", "cosine").unwrap_err();
        assert_eq!(err, "no vectors to compare — enter one vector per line");
    }

    #[test]
    fn zero_magnitude_vector_is_an_error_for_cosine() {
        let err = defaults("1, 2", "z: 0, 0\n", "cosine").unwrap_err();
        assert_eq!(
            err,
            "cosine similarity is undefined for zero-magnitude vector 1 'z' (line 1)"
        );
    }

    #[test]
    fn unknown_metric_is_an_error() {
        let err = defaults("1, 2", "a: 1, 2\n", "jaccard").unwrap_err();
        assert!(err.starts_with("unknown metric 'jaccard'"), "{err}");
    }

    #[test]
    fn too_many_vectors_is_an_error() {
        let mut list = String::new();
        for i in 0..(MAX_VECTORS + 1) {
            list.push_str(&format!("{i}\n"));
        }
        let err = defaults("1", &list, "cosine").unwrap_err();
        assert!(err.starts_with("too many vectors (maximum 2000)"), "{err}");
    }

    #[test]
    fn exactly_max_vectors_is_accepted() {
        let mut list = String::new();
        for i in 0..MAX_VECTORS {
            list.push_str(&format!("{i}\n"));
        }
        // vector 0 has zero magnitude, so rank by a metric that stays defined.
        let out = run("1", &list, "euclidean", 1, false, 0.0, 0, false, "csv").unwrap();
        assert_eq!(out, "rank,label,euclidean\n1,v2,0\n");
    }

    #[test]
    fn too_many_dimensions_is_an_error() {
        let query = vec!["1"; MAX_DIMS + 1].join(",");
        let err = defaults(&query, "a: 1\n", "cosine").unwrap_err();
        assert_eq!(err, "query vector has 8193 dimensions (maximum 8192)");
    }

    #[test]
    fn zero_magnitude_vector_cannot_be_normalized() {
        let err = run("1, 1", "z: 0, 0\n", "euclidean", 5, true, 0.0, 6, false, "table")
            .unwrap_err();
        assert_eq!(
            err,
            "cannot L2-normalize vector 1 'z' (line 1): its magnitude is zero"
        );
    }

    #[test]
    fn ties_keep_input_order() {
        let out = run(
            "1, 0",
            "first: 1, 0\nsecond: 1, 0\n",
            "cosine",
            5,
            false,
            0.0,
            2,
            false,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "rank,label,cosine\n1,first,1.00\n2,second,1.00\n");
    }
}
