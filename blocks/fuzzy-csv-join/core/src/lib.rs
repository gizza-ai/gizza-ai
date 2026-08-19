//! fuzzy-csv-join core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps.
//!
//! Joins TWO CSV inputs on a key column using **approximate** string similarity
//! instead of exact equality, so `Acme Ltd.` on one side can match `ACME Limited`
//! on the other. The first row of each input is its header; the key is referenced
//! by header NAME or 1-based column index on each side independently.
//!
//! For every left row the key value is scored against every right key value with
//! the chosen algorithm (0–100). Candidates scoring at or above `threshold` are
//! kept, best first, and truncated to `max_matches`. Ties keep the earlier right
//! row, so the output is deterministic.
//!
//! Output columns = the whole left header, then the whole RIGHT header (the right
//! key column is KEPT — unlike an exact join, its value differs from the left key
//! and is the evidence for the match), with any colliding name suffixed `_right`,
//! then an optional `match_score` column. `join_type` decides whether unmatched
//! rows are padded in; `output` can instead return just the unmatched rows of
//! either side, or a JSON report carrying matches, both unmatched lists, and
//! coverage stats.
//!
//! Complexity is left×right comparisons, so each side is capped at
//! [`MAX_ROWS`] data rows.

use std::collections::HashMap;

use serde::Serialize;

/// Maximum data rows (excluding the header) accepted on EACH side. The join
/// compares every left key against every right key, so the work is the product of
/// the two counts; this cap keeps the worst case bounded in a browser tab.
pub const MAX_ROWS: usize = 2000;

/// SQL-style join variants (which unmatched rows survive).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum JoinType {
    Inner,
    Left,
    Right,
    Outer,
}

impl JoinType {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "inner" => Ok(JoinType::Inner),
            "left" => Ok(JoinType::Left),
            "right" => Ok(JoinType::Right),
            "outer" | "full" | "full outer" | "full-outer" => Ok(JoinType::Outer),
            other => Err(format!(
                "join_type must be inner/left/right/outer, got '{other}'"
            )),
        }
    }
    fn keep_left_unmatched(self) -> bool {
        matches!(self, JoinType::Left | JoinType::Outer)
    }
    fn keep_right_unmatched(self) -> bool {
        matches!(self, JoinType::Right | JoinType::Outer)
    }
}

/// Which view of the join to return.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Output {
    Csv,
    UnmatchedLeft,
    UnmatchedRight,
    Json,
}

impl Output {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "csv" | "joined" => Ok(Output::Csv),
            "unmatched_left" => Ok(Output::UnmatchedLeft),
            "unmatched_right" => Ok(Output::UnmatchedRight),
            "json" => Ok(Output::Json),
            other => Err(format!(
                "output must be csv/unmatched_left/unmatched_right/json, got '{other}'"
            )),
        }
    }
}

/// Validate the algorithm name up front so a typo fails loudly instead of
/// silently falling back to the default.
fn check_algorithm(algorithm: &str) -> Result<&str, String> {
    match algorithm.trim() {
        "" => Ok("jaro_winkler"),
        a @ ("levenshtein" | "jaro_winkler" | "token_sort" | "soundex") => Ok(a),
        other => Err(format!(
            "algorithm must be levenshtein/jaro_winkler/token_sort/soundex, got '{other}'"
        )),
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

/// Parse CSV text into (header, rows). Errors if there is no header row or the
/// row count exceeds [`MAX_ROWS`].
fn parse(
    data: &str,
    delim: u8,
    side: &str,
) -> Result<(csv::StringRecord, Vec<csv::StringRecord>), String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let mut recs = rdr
        .records()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{side} CSV parse error: {e}"))?;
    if recs.is_empty() {
        return Err(format!("{side} CSV is empty — a header row is required"));
    }
    let header = recs.remove(0);
    if recs.len() > MAX_ROWS {
        return Err(format!(
            "{side} CSV has {} data rows — a fuzzy join compares every left row against every right row, so each side is capped at {MAX_ROWS} rows. Filter or split the file first.",
            recs.len()
        ));
    }
    Ok((header, recs))
}

/// Resolve a column reference (header name, else 1-based index) to a 0-based index.
fn resolve_col(header: &csv::StringRecord, key: &str, side: &str) -> Result<usize, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err(format!("{side} key column is required"));
    }
    if let Some(pos) = header.iter().position(|h| h == key) {
        return Ok(pos);
    }
    if let Ok(n) = key.parse::<usize>() {
        if n >= 1 && n <= header.len() {
            return Ok(n - 1);
        }
        return Err(format!(
            "{side} key index {n} out of range 1..={} (header has {} columns)",
            header.len(),
            header.len()
        ));
    }
    Err(format!(
        "{side} key column '{key}' not found in header [{}]",
        header.iter().collect::<Vec<_>>().join(", ")
    ))
}

/// Cell value at `idx`, or "" if the row is shorter than the header (ragged rows).
fn cell(rec: &csv::StringRecord, idx: usize) -> &str {
    rec.get(idx).unwrap_or("")
}

// ---------------------------------------------------------------------------
// similarity
// ---------------------------------------------------------------------------

/// Full Levenshtein edit distance (key values are short, no bounding needed).
fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Normalized Levenshtein similarity of two strings in 0..=100 (100 = identical).
fn levenshtein_ratio(a: &str, b: &str) -> f64 {
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let m = ca.len().max(cb.len());
    if m == 0 {
        return 100.0;
    }
    let d = levenshtein(&ca, &cb);
    (1.0 - d as f64 / m as f64) * 100.0
}

/// Jaro similarity of two strings in 0..=1.
fn jaro(a: &[char], b: &[char]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let match_dist = (a.len().max(b.len()) / 2).saturating_sub(1);
    let mut a_match = vec![false; a.len()];
    let mut b_match = vec![false; b.len()];
    let mut matches = 0usize;
    for (i, &ca) in a.iter().enumerate() {
        let lo = i.saturating_sub(match_dist);
        let hi = (i + match_dist + 1).min(b.len());
        for j in lo..hi {
            if !b_match[j] && b[j] == ca {
                a_match[i] = true;
                b_match[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }
    let mut transpositions = 0usize;
    let mut k = 0usize;
    for i in 0..a.len() {
        if a_match[i] {
            while !b_match[k] {
                k += 1;
            }
            if a[i] != b[k] {
                transpositions += 1;
            }
            k += 1;
        }
    }
    let m = matches as f64;
    let t = transpositions as f64 / 2.0;
    (m / a.len() as f64 + m / b.len() as f64 + (m - t) / m) / 3.0
}

/// Jaro-Winkler similarity in 0..=100 (adds a bonus for a shared prefix up to 4).
fn jaro_winkler_ratio(a: &str, b: &str) -> f64 {
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let j = jaro(&ca, &cb);
    let prefix = ca
        .iter()
        .zip(cb.iter())
        .take(4)
        .take_while(|(x, y)| x == y)
        .count();
    (j + prefix as f64 * 0.1 * (1.0 - j)) * 100.0
}

/// Soundex code (American Soundex): a retained first letter + three digits.
fn soundex_token(token: &str) -> String {
    let letters: Vec<char> = token
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if letters.is_empty() {
        return String::new();
    }
    fn code(c: char) -> char {
        match c {
            'B' | 'F' | 'P' | 'V' => '1',
            'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => '2',
            'D' | 'T' => '3',
            'L' => '4',
            'M' | 'N' => '5',
            'R' => '6',
            _ => '0', // vowels + H, W, Y
        }
    }
    let first = letters[0];
    let mut out = String::new();
    out.push(first);
    let mut last = code(first);
    for &c in &letters[1..] {
        let d = code(c);
        // H and W are transparent (they don't reset the previous code); vowels do.
        if d != '0' && d != last {
            out.push(d);
            if out.len() == 4 {
                break;
            }
        }
        if c != 'H' && c != 'W' {
            last = d;
        }
    }
    while out.len() < 4 {
        out.push('0');
    }
    out
}

/// Phonetic key for a whole value: Soundex each whitespace token, join with spaces.
fn phonetic_key(value: &str) -> String {
    value
        .split_whitespace()
        .map(soundex_token)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Sort a value's whitespace tokens so word ORDER stops mattering.
fn token_sort_key(value: &str) -> String {
    let mut toks: Vec<&str> = value.split_whitespace().collect();
    toks.sort_unstable();
    toks.join(" ")
}

/// Similarity of two ALREADY-NORMALIZED key values in 0..=100.
fn similarity(a: &str, b: &str, algorithm: &str) -> f64 {
    match algorithm {
        "levenshtein" => levenshtein_ratio(a, b),
        "token_sort" => levenshtein_ratio(&token_sort_key(a), &token_sort_key(b)),
        "soundex" => levenshtein_ratio(&phonetic_key(a), &phonetic_key(b)),
        // "jaro_winkler" — the default, already validated by check_algorithm.
        _ => jaro_winkler_ratio(a, b),
    }
}

/// Normalize a key value for COMPARISON only (the original text is always what
/// gets written out). Whitespace is always collapsed; case and punctuation are
/// caller-controlled.
fn normalize(value: &str, case: bool, punctuation: bool) -> String {
    let mut s = value.to_string();
    if punctuation {
        s = s
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() {
                    c
                } else {
                    ' '
                }
            })
            .collect();
    }
    if case {
        s = s.to_lowercase();
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Round a 0–100 score to one decimal for stable display and tests.
fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

// ---------------------------------------------------------------------------
// JSON report shapes
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct JsonMatch {
    left_key: String,
    right_key: String,
    score: f64,
    left: HashMap<String, String>,
    right: HashMap<String, String>,
}

#[derive(Serialize)]
struct JsonStats {
    left_rows: usize,
    right_rows: usize,
    matched_pairs: usize,
    matched_left_rows: usize,
    unmatched_left_rows: usize,
    matched_right_rows: usize,
    unmatched_right_rows: usize,
}

#[derive(Serialize)]
struct JsonReport {
    algorithm: String,
    threshold: f64,
    max_matches: usize,
    stats: JsonStats,
    matches: Vec<JsonMatch>,
    unmatched_left: Vec<HashMap<String, String>>,
    unmatched_right: Vec<HashMap<String, String>>,
}

/// A row as a `header -> cell` map (ragged rows pad with "").
fn row_map(header: &csv::StringRecord, rec: &csv::StringRecord) -> HashMap<String, String> {
    header
        .iter()
        .enumerate()
        .map(|(i, h)| (h.to_string(), cell(rec, i).to_string()))
        .collect()
}

/// Serialize a header + rows as CSV text.
fn write_csv(header: &[String], rows: &[Vec<String>], delim: u8) -> Result<String, String> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    wtr.write_record(header)
        .map_err(|e| format!("CSV write error: {e}"))?;
    for r in rows {
        wtr.write_record(r)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("output not valid UTF-8: {e}"))
}

// ---------------------------------------------------------------------------
// the join
// ---------------------------------------------------------------------------

/// Fuzzy-join `left` and `right` CSV text on their key columns.
///
/// * `left_key` / `right_key` — header name or 1-based index; a blank
///   `right_key` reuses `left_key`'s reference.
/// * `algorithm` — `levenshtein` | `jaro_winkler` | `token_sort` | `soundex`.
/// * `threshold` — 0–100 similarity cutoff (inclusive).
/// * `join_type` — `inner` | `left` | `right` | `outer`.
/// * `max_matches` — how many right rows a single left row may match (best first).
/// * `show_score` — append a `match_score` column to the CSV output.
/// * `normalize_case` / `ignore_punctuation` — comparison-only normalisations.
/// * `output` — `csv` | `unmatched_left` | `unmatched_right` | `json`. The
///   `unmatched_*` and `json` views always report BOTH sides' leftovers,
///   independent of `join_type` (which only shapes the joined CSV).
#[allow(clippy::too_many_arguments)]
pub fn fuzzy_join(
    left: &str,
    right: &str,
    left_key: &str,
    right_key: &str,
    algorithm: &str,
    threshold: f64,
    join_type: &str,
    max_matches: usize,
    show_score: bool,
    normalize_case: bool,
    ignore_punctuation: bool,
    delimiter: &str,
    output: &str,
) -> Result<String, String> {
    if left.trim().is_empty() {
        return Err("left CSV is empty".into());
    }
    if right.trim().is_empty() {
        return Err("right CSV is empty".into());
    }
    if !(0.0..=100.0).contains(&threshold) {
        return Err(format!(
            "threshold must be between 0 and 100, got {threshold}"
        ));
    }
    if max_matches == 0 {
        return Err("max_matches must be at least 1".into());
    }
    let algo = check_algorithm(algorithm)?;
    let jt = JoinType::parse(join_type)?;
    let out_mode = Output::parse(output)?;
    let delim = delim_byte(delimiter)?;

    let (lh, lrows) = parse(left, delim, "left")?;
    let (rh, rrows) = parse(right, delim, "right")?;

    let lk = resolve_col(&lh, left_key, "left")?;
    // Blank right key → reuse the left key reference (name or index).
    let rk_ref = if right_key.trim().is_empty() {
        left_key
    } else {
        right_key
    };
    let rk = resolve_col(&rh, rk_ref, "right")?;

    // Pre-normalize every right key once (each is compared against every left key).
    let r_norm: Vec<String> = rrows
        .iter()
        .map(|r| normalize(cell(r, rk), normalize_case, ignore_punctuation))
        .collect();

    // Best-first candidate lists, one per left row.
    let mut matches_per_left: Vec<Vec<(usize, f64)>> = Vec::with_capacity(lrows.len());
    let mut right_matched = vec![false; rrows.len()];
    for l in &lrows {
        let ln = normalize(cell(l, lk), normalize_case, ignore_punctuation);
        let mut cands: Vec<(usize, f64)> = Vec::new();
        for (ri, rn) in r_norm.iter().enumerate() {
            let score = round1(similarity(&ln, rn, algo));
            if score >= threshold {
                cands.push((ri, score));
            }
        }
        // Best score first; ties keep the earlier right row, so output is stable.
        cands.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        cands.truncate(max_matches);
        for &(ri, _) in &cands {
            right_matched[ri] = true;
        }
        matches_per_left.push(cands);
    }

    match out_mode {
        Output::UnmatchedLeft => {
            let header: Vec<String> = lh.iter().map(|s| s.to_string()).collect();
            let rows: Vec<Vec<String>> = lrows
                .iter()
                .enumerate()
                .filter(|(i, _)| matches_per_left[*i].is_empty())
                .map(|(_, l)| (0..lh.len()).map(|c| cell(l, c).to_string()).collect())
                .collect();
            return write_csv(&header, &rows, delim);
        }
        Output::UnmatchedRight => {
            let header: Vec<String> = rh.iter().map(|s| s.to_string()).collect();
            let rows: Vec<Vec<String>> = rrows
                .iter()
                .enumerate()
                .filter(|(i, _)| !right_matched[*i])
                .map(|(_, r)| (0..rh.len()).map(|c| cell(r, c).to_string()).collect())
                .collect();
            return write_csv(&header, &rows, delim);
        }
        Output::Json => {
            let mut out_matches = Vec::new();
            for (li, l) in lrows.iter().enumerate() {
                for &(ri, score) in &matches_per_left[li] {
                    out_matches.push(JsonMatch {
                        left_key: cell(l, lk).to_string(),
                        right_key: cell(&rrows[ri], rk).to_string(),
                        score,
                        left: row_map(&lh, l),
                        right: row_map(&rh, &rrows[ri]),
                    });
                }
            }
            let matched_left = matches_per_left.iter().filter(|m| !m.is_empty()).count();
            let matched_right = right_matched.iter().filter(|m| **m).count();
            let report = JsonReport {
                algorithm: algo.to_string(),
                threshold,
                max_matches,
                stats: JsonStats {
                    left_rows: lrows.len(),
                    right_rows: rrows.len(),
                    matched_pairs: out_matches.len(),
                    matched_left_rows: matched_left,
                    unmatched_left_rows: lrows.len() - matched_left,
                    matched_right_rows: matched_right,
                    unmatched_right_rows: rrows.len() - matched_right,
                },
                matches: out_matches,
                unmatched_left: lrows
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| matches_per_left[*i].is_empty())
                    .map(|(_, l)| row_map(&lh, l))
                    .collect(),
                unmatched_right: rrows
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !right_matched[*i])
                    .map(|(_, r)| row_map(&rh, r))
                    .collect(),
            };
            return serde_json::to_string_pretty(&report)
                .map_err(|e| format!("JSON encode error: {e}"));
        }
        Output::Csv => {}
    }

    // --- the joined CSV -----------------------------------------------------
    // Full left header, then the full right header (the right key column is kept:
    // in a fuzzy join its value differs from the left key and is the evidence).
    let mut out_header: Vec<String> = lh.iter().map(|s| s.to_string()).collect();
    for (ci, name) in rh.iter().enumerate() {
        let mut out_name = name.to_string();
        if out_header.iter().any(|h| h == &out_name) {
            out_name = format!("{name}_right");
            // Extremely rare second collision: disambiguate with the column index.
            while out_header.iter().any(|h| h == &out_name) {
                out_name = format!("{name}_right{ci}");
            }
        }
        out_header.push(out_name);
    }
    if show_score {
        let mut name = "match_score".to_string();
        while out_header.iter().any(|h| h == &name) {
            name.push('_');
        }
        out_header.push(name);
    }

    let l_width = lh.len();
    let r_width = rh.len();
    let width = out_header.len();

    let build_row = |lrow: Option<&csv::StringRecord>,
                     rrow: Option<&csv::StringRecord>,
                     score: Option<f64>|
     -> Vec<String> {
        let mut out = vec![String::new(); width];
        if let Some(l) = lrow {
            for (ci, slot) in out.iter_mut().enumerate().take(l_width) {
                *slot = cell(l, ci).to_string();
            }
        }
        if let Some(r) = rrow {
            for ci in 0..r_width {
                out[l_width + ci] = cell(r, ci).to_string();
            }
        }
        if show_score {
            out[width - 1] = match score {
                Some(s) => format!("{s}"),
                None => String::new(),
            };
        }
        out
    };

    let mut rows: Vec<Vec<String>> = Vec::new();
    for (li, l) in lrows.iter().enumerate() {
        if matches_per_left[li].is_empty() {
            if jt.keep_left_unmatched() {
                rows.push(build_row(Some(l), None, None));
            }
            continue;
        }
        for &(ri, score) in &matches_per_left[li] {
            rows.push(build_row(Some(l), Some(&rrows[ri]), Some(score)));
        }
    }
    if jt.keep_right_unmatched() {
        for (ri, r) in rrows.iter().enumerate() {
            if !right_matched[ri] {
                rows.push(build_row(None, Some(r), None));
            }
        }
    }

    write_csv(&out_header, &rows, delim)
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: &str = "id,company\n1,Acme Ltd\n2,Globex Corporation\n3,Initech";
    const R: &str = "name,city\nAcme Ltd.,Berlin\nGlobex Corp,Cairo\nUmbrella,Delhi";

    /// Default-ish call helper: jaro_winkler, threshold 85, inner, 1 match, score on.
    fn run(left: &str, right: &str, lk: &str, rk: &str, algo: &str, thr: f64) -> String {
        fuzzy_join(
            left, right, lk, rk, algo, thr, "inner", 1, true, true, false, ",", "csv",
        )
        .unwrap()
    }

    #[test]
    fn inner_join_matches_near_identical_keys() {
        let out = run(L, R, "company", "name", "levenshtein", 80.0);
        assert_eq!(
            out,
            "id,company,name,city,match_score\n\
             1,Acme Ltd,Acme Ltd.,Berlin,88.9\n"
        );
    }

    #[test]
    fn jaro_winkler_catches_the_abbreviation_levenshtein_misses() {
        // "Globex Corporation" vs "Globex Corp" is only 61% by edit distance but
        // scores high under the prefix-weighted Jaro-Winkler.
        let out = run(L, R, "company", "name", "jaro_winkler", 85.0);
        assert_eq!(
            out,
            "id,company,name,city,match_score\n\
             1,Acme Ltd,Acme Ltd.,Berlin,97.8\n\
             2,Globex Corporation,Globex Corp,Cairo,92.2\n"
        );
    }

    #[test]
    fn exact_threshold_boundary_is_inclusive() {
        // "Acme Ltd" vs "Acme Ltd." is 8/9 characters = 88.9 by edit distance.
        let at = run(L, R, "company", "name", "levenshtein", 88.9);
        assert!(
            at.contains("Acme Ltd."),
            "88.9 must match at the boundary: {at}"
        );
        let over = run(L, R, "company", "name", "levenshtein", 89.0);
        assert_eq!(over, "id,company,name,city,match_score\n", "89.0 must not");
    }

    #[test]
    fn left_join_keeps_unmatched_left_rows_with_blank_right_cells() {
        let out = fuzzy_join(
            L,
            R,
            "company",
            "name",
            "levenshtein",
            80.0,
            "left",
            1,
            true,
            true,
            false,
            ",",
            "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "id,company,name,city,match_score\n\
             1,Acme Ltd,Acme Ltd.,Berlin,88.9\n\
             2,Globex Corporation,,,\n\
             3,Initech,,,\n"
        );
    }

    #[test]
    fn outer_join_appends_unmatched_right_rows() {
        let out = fuzzy_join(
            L,
            R,
            "company",
            "name",
            "jaro_winkler",
            85.0,
            "outer",
            1,
            false,
            true,
            false,
            ",",
            "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "id,company,name,city\n\
             1,Acme Ltd,Acme Ltd.,Berlin\n\
             2,Globex Corporation,Globex Corp,Cairo\n\
             3,Initech,,\n\
             ,,Umbrella,Delhi\n"
        );
    }

    #[test]
    fn max_matches_keeps_several_candidates_best_first() {
        let l = "k\nJon Smith";
        let r = "k,tag\nJohn Smith,a\nJon Smyth,b\nJon Smith,c";
        let out = fuzzy_join(
            l,
            r,
            "k",
            "k",
            "levenshtein",
            70.0,
            "inner",
            3,
            true,
            true,
            false,
            ",",
            "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "k,k_right,tag,match_score\n\
             Jon Smith,Jon Smith,c,100\n\
             Jon Smith,John Smith,a,90\n\
             Jon Smith,Jon Smyth,b,88.9\n"
        );
    }

    #[test]
    fn soundex_matches_values_that_only_sound_alike() {
        let l = "k\nSmyth";
        let r = "k,city\nSmith,Berlin";
        // Edit distance alone rates Smyth/Smith at 80; Soundex codes both S530.
        let out = run(l, r, "k", "k", "soundex", 100.0);
        assert_eq!(out, "k,k_right,city,match_score\nSmyth,Smith,Berlin,100\n");
    }

    #[test]
    fn token_sort_ignores_word_order() {
        let l = "k\nLtd Acme";
        let r = "k,city\nAcme Ltd,Berlin";
        let out = run(l, r, "k", "k", "token_sort", 100.0);
        assert_eq!(
            out,
            "k,k_right,city,match_score\nLtd Acme,Acme Ltd,Berlin,100\n"
        );
    }

    #[test]
    fn ignore_punctuation_makes_a_punctuated_key_match_exactly() {
        let l = "k\n\"Acme, Ltd.\"";
        let r = "k,city\nAcme Ltd,Berlin";
        let out = fuzzy_join(
            l,
            r,
            "k",
            "k",
            "levenshtein",
            100.0,
            "inner",
            1,
            true,
            true,
            true,
            ",",
            "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "k,k_right,city,match_score\n\"Acme, Ltd.\",Acme Ltd,Berlin,100\n"
        );
    }

    #[test]
    fn case_sensitivity_can_be_turned_off() {
        let l = "k\nACME";
        let r = "k,city\nacme,Berlin";
        let strict = fuzzy_join(
            l,
            r,
            "k",
            "k",
            "levenshtein",
            100.0,
            "inner",
            1,
            false,
            false,
            false,
            ",",
            "csv",
        )
        .unwrap();
        assert_eq!(strict, "k,k_right,city\n", "case-sensitive: no exact match");
        let loose = fuzzy_join(
            l,
            r,
            "k",
            "k",
            "levenshtein",
            100.0,
            "inner",
            1,
            false,
            true,
            false,
            ",",
            "csv",
        )
        .unwrap();
        assert_eq!(loose, "k,k_right,city\nACME,acme,Berlin\n");
    }

    #[test]
    fn unmatched_left_view_lists_only_rows_with_no_partner() {
        let out = fuzzy_join(
            L,
            R,
            "company",
            "name",
            "levenshtein",
            80.0,
            "inner",
            1,
            true,
            true,
            false,
            ",",
            "unmatched_left",
        )
        .unwrap();
        assert_eq!(out, "id,company\n2,Globex Corporation\n3,Initech\n");
    }

    #[test]
    fn unmatched_right_view_lists_the_other_sides_leftovers() {
        let out = fuzzy_join(
            L,
            R,
            "company",
            "name",
            "jaro_winkler",
            85.0,
            "inner",
            1,
            true,
            true,
            false,
            ",",
            "unmatched_right",
        )
        .unwrap();
        assert_eq!(out, "name,city\nUmbrella,Delhi\n");
    }

    #[test]
    fn json_report_carries_stats_and_both_unmatched_lists() {
        let out = fuzzy_join(
            L,
            R,
            "company",
            "name",
            "jaro_winkler",
            85.0,
            "inner",
            1,
            true,
            true,
            false,
            ",",
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["stats"]["matched_pairs"], 2);
        assert_eq!(v["stats"]["unmatched_left_rows"], 1);
        assert_eq!(v["stats"]["unmatched_right_rows"], 1);
        assert_eq!(v["matches"][0]["left_key"], "Acme Ltd");
        assert_eq!(v["matches"][0]["right_key"], "Acme Ltd.");
        assert_eq!(v["unmatched_left"][0]["company"], "Initech");
        assert_eq!(v["unmatched_right"][0]["name"], "Umbrella");
    }

    #[test]
    fn key_by_index_and_semicolon_delimiter() {
        let l = "id;company\n1;Acme Ltd";
        let r = "name;city\nAcme Ltd.;Berlin";
        let out = fuzzy_join(
            l,
            r,
            "2",
            "1",
            "levenshtein",
            80.0,
            "inner",
            1,
            false,
            true,
            false,
            "semicolon",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "id;company;name;city\n1;Acme Ltd;Acme Ltd.;Berlin\n");
    }

    #[test]
    fn colliding_column_names_get_a_right_suffix() {
        let l = "k,city\nAcme,Berlin";
        let r = "k,city\nAcme,Cairo";
        let out = run(l, r, "k", "k", "levenshtein", 90.0);
        assert_eq!(
            out,
            "k,city,k_right,city_right,match_score\nAcme,Berlin,Acme,Cairo,100\n"
        );
    }

    #[test]
    fn blank_right_key_reuses_the_left_reference() {
        let l = "k,a\nAcme Ltd,1";
        let r = "k,b\nAcme Ltd.,2";
        let out = run(l, r, "k", "", "levenshtein", 80.0);
        assert_eq!(
            out,
            "k,a,k_right,b,match_score\nAcme Ltd,1,Acme Ltd.,2,88.9\n"
        );
    }

    // --- error paths --------------------------------------------------------

    #[test]
    fn unknown_key_column_errors() {
        let err = fuzzy_join(
            L,
            R,
            "nope",
            "name",
            "levenshtein",
            85.0,
            "inner",
            1,
            true,
            true,
            false,
            ",",
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("left key column 'nope' not found"), "{err}");
    }

    #[test]
    fn unknown_algorithm_errors() {
        let err = fuzzy_join(
            L, R, "company", "name", "cosine", 85.0, "inner", 1, true, true, false, ",", "csv",
        )
        .unwrap_err();
        assert!(err.contains("algorithm must be"), "{err}");
    }

    #[test]
    fn out_of_range_threshold_errors() {
        let err = fuzzy_join(
            L,
            R,
            "company",
            "name",
            "levenshtein",
            140.0,
            "inner",
            1,
            true,
            true,
            false,
            ",",
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("threshold must be between 0 and 100"), "{err}");
    }

    #[test]
    fn empty_input_errors() {
        let err = fuzzy_join(
            "",
            R,
            "company",
            "name",
            "levenshtein",
            85.0,
            "inner",
            1,
            true,
            true,
            false,
            ",",
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("left CSV is empty"), "{err}");
    }

    #[test]
    fn row_cap_is_enforced_at_the_boundary() {
        let mut big = String::from("k\n");
        for i in 0..MAX_ROWS {
            big.push_str(&format!("v{i}\n"));
        }
        // Exactly MAX_ROWS data rows is accepted.
        assert!(fuzzy_join(
            &big,
            "k\nv1",
            "k",
            "k",
            "levenshtein",
            100.0,
            "inner",
            1,
            false,
            true,
            false,
            ",",
            "csv",
        )
        .is_ok());
        // One more is rejected with a message naming the cap.
        big.push_str("overflow\n");
        let err = fuzzy_join(
            &big,
            "k\nv1",
            "k",
            "k",
            "levenshtein",
            100.0,
            "inner",
            1,
            false,
            true,
            false,
            ",",
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("capped at 2000 rows"), "{err}");
    }
}
