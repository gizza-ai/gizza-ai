//! fuzzy-match core — pure compute, shared by the chat skill block and the web page.

use std::cmp::Ordering;

pub const MAX_CANDIDATES: usize = 1_000;
pub const MAX_TEXT_LEN: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    Hybrid,
    Levenshtein,
    Subsequence,
}

impl Algorithm {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "hybrid" | "balanced" | "default" => Ok(Algorithm::Hybrid),
            "levenshtein" | "edit" | "edit_distance" => Ok(Algorithm::Levenshtein),
            "subsequence" | "fzf" | "finder" => Ok(Algorithm::Subsequence),
            other => Err(format!(
                "unknown algorithm '{other}': expected 'hybrid', 'levenshtein' or 'subsequence'"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Csv,
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" | "txt" | "plain" => Ok(OutputFormat::Text),
            "csv" => Ok(OutputFormat::Csv),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "unknown output_format '{other}': expected 'text', 'csv' or 'json'"
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub algorithm: Algorithm,
    pub limit: usize,
    pub threshold: f64,
    pub case_sensitive: bool,
    pub include_reasons: bool,
    pub format: OutputFormat,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            algorithm: Algorithm::Hybrid,
            limit: 10,
            threshold: 0.0,
            case_sensitive: false,
            include_reasons: true,
            format: OutputFormat::Text,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchResult {
    pub rank: usize,
    pub candidate: String,
    pub score: f64,
    pub edit_distance: usize,
    pub reason: String,
}

pub fn parse_candidates(input: &str) -> Result<Vec<String>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(
            "no candidates: enter one candidate per line, or a comma-separated list".into(),
        );
    }
    let raw: Vec<String> = if !trimmed.contains('\n') && trimmed.contains(',') {
        trimmed.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        trimmed
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(strip_marker)
            .map(str::to_string)
            .collect()
    };
    let candidates: Vec<String> = raw.into_iter().filter(|s| !s.trim().is_empty()).collect();
    if candidates.is_empty() {
        return Err("no candidates remained after removing blank lines and comments".into());
    }
    if candidates.len() > MAX_CANDIDATES {
        return Err(format!(
            "too many candidates: {} provided, limit is {MAX_CANDIDATES}",
            candidates.len()
        ));
    }
    for c in &candidates {
        if c.chars().count() > MAX_TEXT_LEN {
            return Err(format!(
                "candidate is too long: limit is {MAX_TEXT_LEN} characters, got {} in '{:.32}…'",
                c.chars().count(),
                c
            ));
        }
    }
    Ok(candidates)
}

fn strip_marker(s: &str) -> &str {
    let t = s.trim();
    for marker in ['-', '*', '•'] {
        if let Some(rest) = t.strip_prefix(marker) {
            return rest.trim();
        }
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 {
        let rest = &t[digits..];
        if let Some(r) = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')')) {
            return r.trim();
        }
    }
    t
}

fn prep(s: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        s.to_string()
    } else {
        s.to_lowercase()
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn levenshtein_score(query: &str, candidate: &str) -> (f64, usize) {
    let dist = levenshtein(query, candidate);
    let max_len = query.chars().count().max(candidate.chars().count()).max(1);
    (((max_len - dist) as f64 / max_len as f64).max(0.0), dist)
}

fn subsequence_score(query: &str, candidate: &str) -> (f64, String) {
    if query.is_empty() {
        return (1.0, "empty query matches everything".into());
    }
    let q: Vec<char> = query.chars().collect();
    let c: Vec<char> = candidate.chars().collect();
    let mut qi = 0usize;
    let mut first = None;
    let mut last = 0usize;
    let mut contiguous = 0usize;
    let mut best_contiguous = 0usize;
    let mut prev_match = None;
    for (i, ch) in c.iter().enumerate() {
        if qi < q.len() && *ch == q[qi] {
            if first.is_none() {
                first = Some(i);
            }
            if prev_match == Some(i.saturating_sub(1)) {
                contiguous += 1;
            } else {
                contiguous = 1;
            }
            best_contiguous = best_contiguous.max(contiguous);
            prev_match = Some(i);
            last = i;
            qi += 1;
        }
    }
    if qi != q.len() {
        return (0.0, "query characters do not appear in order".into());
    }
    let span = last + 1 - first.unwrap_or(0);
    let compactness = q.len() as f64 / span.max(1) as f64;
    let coverage = q.len() as f64 / c.len().max(1) as f64;
    let streak = best_contiguous as f64 / q.len().max(1) as f64;
    let score = (0.55 * compactness + 0.25 * streak + 0.20 * coverage).min(1.0);
    (
        score,
        format!("subsequence span {span}, best contiguous run {best_contiguous}"),
    )
}

pub fn rank(query: &str, candidates: &str, opts: &Options) -> Result<Vec<MatchResult>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("query is required".into());
    }
    if query.chars().count() > MAX_TEXT_LEN {
        return Err(format!(
            "query is too long: limit is {MAX_TEXT_LEN} characters"
        ));
    }
    if !(0.0..=100.0).contains(&opts.threshold) {
        return Err("threshold must be between 0 and 100".into());
    }
    if opts.limit == 0 || opts.limit > MAX_CANDIDATES {
        return Err(format!("limit must be between 1 and {MAX_CANDIDATES}"));
    }

    let parsed = parse_candidates(candidates)?;
    let q = prep(query, opts.case_sensitive);
    let mut rows: Vec<MatchResult> = parsed
        .into_iter()
        .map(|candidate| {
            let c = prep(&candidate, opts.case_sensitive);
            let (lev, dist) = levenshtein_score(&q, &c);
            let (sub, sub_reason) = subsequence_score(&q, &c);
            let exact_bonus = if c == q {
                1.0
            } else if c.contains(&q) {
                0.92
            } else {
                0.0
            };
            let (score, reason) = match opts.algorithm {
                Algorithm::Levenshtein => (lev, format!("edit distance {dist}")),
                Algorithm::Subsequence => (sub, sub_reason),
                Algorithm::Hybrid => {
                    let s = lev.max(sub).max(exact_bonus);
                    let why = if exact_bonus > 0.0 && exact_bonus >= lev && exact_bonus >= sub {
                        if c == q {
                            "exact match".to_string()
                        } else {
                            "contains the query".to_string()
                        }
                    } else if lev >= sub {
                        format!("edit distance {dist}")
                    } else {
                        sub_reason
                    };
                    (s, why)
                }
            };
            MatchResult {
                rank: 0,
                candidate,
                score: score * 100.0,
                edit_distance: dist,
                reason,
            }
        })
        .filter(|r| r.score + f64::EPSILON >= opts.threshold)
        .collect();

    rows.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.edit_distance.cmp(&b.edit_distance))
            .then_with(|| a.candidate.to_lowercase().cmp(&b.candidate.to_lowercase()))
    });
    rows.truncate(opts.limit);
    for (i, row) in rows.iter_mut().enumerate() {
        row.rank = i + 1;
    }
    Ok(rows)
}

pub fn render(results: &[MatchResult], opts: &Options) -> String {
    match opts.format {
        OutputFormat::Text => render_text(results, opts.include_reasons),
        OutputFormat::Csv => render_csv(results, opts.include_reasons),
        OutputFormat::Json => render_json(results, opts.include_reasons),
    }
}

fn render_text(results: &[MatchResult], include_reasons: bool) -> String {
    if results.is_empty() {
        return "No matches at or above the threshold.".into();
    }
    let mut out = String::from("rank  score   candidate");
    if include_reasons {
        out.push_str("  reason");
    }
    for r in results {
        if include_reasons {
            out.push_str(&format!(
                "\n{:>4}  {:>5.1}  {}  — {}",
                r.rank, r.score, r.candidate, r.reason
            ));
        } else {
            out.push_str(&format!(
                "\n{:>4}  {:>5.1}  {}",
                r.rank, r.score, r.candidate
            ));
        }
    }
    out
}

fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(results: &[MatchResult], include_reasons: bool) -> String {
    let mut out = if include_reasons {
        "rank,score,candidate,edit_distance,reason".to_string()
    } else {
        "rank,score,candidate,edit_distance".to_string()
    };
    for r in results {
        if include_reasons {
            out.push_str(&format!(
                "\n{},{:.1},{},{},{}",
                r.rank,
                r.score,
                csv_escape(&r.candidate),
                r.edit_distance,
                csv_escape(&r.reason)
            ));
        } else {
            out.push_str(&format!(
                "\n{},{:.1},{},{}",
                r.rank,
                r.score,
                csv_escape(&r.candidate),
                r.edit_distance
            ));
        }
    }
    out
}

fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn render_json(results: &[MatchResult], include_reasons: bool) -> String {
    let mut out = String::from("[\n");
    for (i, r) in results.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        if include_reasons {
            out.push_str(&format!(
                "  {{\"rank\":{},\"score\":{:.1},\"candidate\":\"{}\",\"edit_distance\":{},\"reason\":\"{}\"}}",
                r.rank, r.score, json_escape(&r.candidate), r.edit_distance, json_escape(&r.reason)
            ));
        } else {
            out.push_str(&format!(
                "  {{\"rank\":{},\"score\":{:.1},\"candidate\":\"{}\",\"edit_distance\":{}}}",
                r.rank,
                r.score,
                json_escape(&r.candidate),
                r.edit_distance
            ));
        }
    }
    out.push_str("\n]");
    out
}

pub fn run(
    query: &str,
    candidates: &str,
    algorithm: &str,
    limit: i64,
    threshold: f64,
    case_sensitive: bool,
    include_reasons: bool,
    output_format: &str,
) -> Result<String, String> {
    let opts = Options {
        algorithm: Algorithm::parse(algorithm)?,
        limit: usize::try_from(limit)
            .map_err(|_| "limit must be between 1 and 1000".to_string())?,
        threshold,
        case_sensitive,
        include_reasons,
        format: OutputFormat::parse(output_format)?,
    };
    let rows = rank(query, candidates, &opts)?;
    Ok(render(&rows, &opts))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn ranks_best_matches_first() {
        let rows = rank("apple", "apply\napple pie\nbanana\napplet", &opts()).unwrap();
        assert_eq!(rows[0].candidate, "applet");
        assert!(rows[0].score >= 90.0);
        assert!(rows.iter().any(|r| r.candidate == "apple pie"));
    }

    #[test]
    fn threshold_filters_weak_matches() {
        let mut o = opts();
        o.threshold = 80.0;
        let rows = rank("apple", "banana\napply", &o).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].candidate, "apply");
    }

    #[test]
    fn csv_and_json_render() {
        let mut o = opts();
        o.format = OutputFormat::Csv;
        let rows = rank("ny", "New York\nSydney", &o).unwrap();
        let csv = render(&rows, &o);
        assert!(csv.starts_with("rank,score,candidate,edit_distance,reason"));
        o.format = OutputFormat::Json;
        assert!(render(&rows, &o).contains("\"candidate\":\"New York\""));
    }

    #[test]
    fn rejects_empty_query_and_large_lists() {
        assert!(rank("", "a", &opts()).unwrap_err().contains("query"));
        let many = (0..=MAX_CANDIDATES)
            .map(|i| format!("row{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rank("row", &many, &opts())
            .unwrap_err()
            .contains("too many"));
    }

    #[test]
    fn case_sensitive_changes_scores() {
        let mut o = opts();
        o.case_sensitive = false;
        let insensitive = rank("ABC", "abc", &o).unwrap()[0].score;
        o.case_sensitive = true;
        let sensitive = rank("ABC", "abc", &o).unwrap()[0].score;
        assert!(insensitive > sensitive);
    }
}
