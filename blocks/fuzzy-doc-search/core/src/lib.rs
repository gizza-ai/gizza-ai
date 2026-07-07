//! fuzzy-doc-search core — pure, dependency-free full-text fuzzy search over a
//! block of text. Shared by the chat skill block and the browser page.
//!
//! The document is segmented into snippets (one per line / sentence / paragraph),
//! each snippet is scored against the query terms with typo tolerance (bounded
//! Levenshtein edit distance) plus substring/prefix matching, and the top-ranked
//! snippets are returned with their location, a 0–100 relevance score, and the
//! matched words wrapped in guillemets («…») so the match is visible in the
//! plain-text output.
//!
//! No `wafer`/`wasm-bindgen`/regex deps: it compiles natively for unit tests,
//! to `wasm32-wasip1` (the `wafer build` chat target), and to
//! `wasm32-unknown-unknown` (the wasm-pack page target).

use serde::Serialize;

/// How the document is split into rankable snippets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// One snippet per line (split on `\n`). The default.
    Line,
    /// One snippet per sentence (split on `.`/`!`/`?` boundaries).
    Sentence,
    /// One snippet per paragraph (split on blank-line runs).
    Paragraph,
}

impl Unit {
    /// Parse the page/CLI string form. Errors on an unknown value.
    pub fn parse(s: &str) -> Result<Unit, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "line" => Ok(Unit::Line),
            "sentence" => Ok(Unit::Sentence),
            "paragraph" => Ok(Unit::Paragraph),
            other => Err(format!(
                "unknown unit {other:?}: expected line, sentence, or paragraph"
            )),
        }
    }
    /// Human label used in the rendered location line.
    fn label(self) -> &'static str {
        match self {
            Unit::Line => "line",
            Unit::Sentence => "sentence",
            Unit::Paragraph => "paragraph",
        }
    }
}

/// Search options. All fields are validated/clamped by [`search`].
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Require ALL query terms to match a snippet (`true`, AND) vs at least one
    /// (`false`, OR — the default).
    pub match_all: bool,
    /// Maximum edit distance (typos) tolerated per query term, 0..=3. 0 = exact
    /// (or substring/prefix) matches only. Clamped into range.
    pub fuzziness: usize,
    /// Case-sensitive matching. Default `false` (case-insensitive).
    pub case_sensitive: bool,
    /// Match whole words only. Default `false` (a term also matches when it is a
    /// substring/prefix of a longer document word).
    pub whole_word: bool,
    /// Snippet granularity.
    pub unit: Unit,
    /// Maximum number of ranked snippets to return, 1..=50. Clamped into range.
    pub max_results: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            match_all: false,
            fuzziness: 1,
            case_sensitive: false,
            whole_word: false,
            unit: Unit::Line,
            max_results: 10,
        }
    }
}

/// Hard cap on `max_results` (also the page/schema max).
pub const MAX_RESULTS_CAP: usize = 50;
/// Hard cap on `fuzziness` (also the page/schema max).
pub const FUZZINESS_CAP: usize = 3;

/// One ranked search hit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    /// 1-based rank in the returned list (1 = best).
    pub rank: usize,
    /// Relevance score, 0–100 (higher = better).
    pub score: u32,
    /// 1-based location of the snippet within the document, in `unit` units
    /// (e.g. the line number for `unit = line`).
    pub location: usize,
    /// The matching snippet text, with matched document words wrapped in «…».
    pub snippet: String,
}

/// The full structured search result (what the chat/CLI surface returns).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchOutput {
    /// The query terms actually searched for (whitespace-split, in order).
    pub terms: Vec<String>,
    /// Number of hits returned (≤ `max_results`).
    pub count: usize,
    /// Total number of snippets that matched before the `max_results` cap.
    pub total_matches: usize,
    /// The ranked hits.
    pub hits: Vec<Hit>,
}

/// A candidate snippet extracted from the document, with its 1-based location.
struct Segment<'a> {
    location: usize,
    text: &'a str,
}

/// Split `text` into candidate snippets according to `unit`, dropping empties.
/// Locations are 1-based sequential indices within the chosen unit.
fn segment(text: &str, unit: Unit) -> Vec<Segment<'_>> {
    match unit {
        Unit::Line => text
            .split('\n')
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
            .map(|(i, l)| Segment { location: i + 1, text: l })
            .collect(),
        Unit::Paragraph => {
            // A paragraph is a run of non-blank lines; blank line(s) separate
            // paragraphs. Locations count only non-empty paragraphs.
            let mut out = Vec::new();
            let mut loc = 0usize;
            for para in text.split("\n\n") {
                let t = para.trim();
                if t.is_empty() {
                    continue;
                }
                loc += 1;
                out.push(Segment { location: loc, text: para });
            }
            out
        }
        Unit::Sentence => split_sentences(text),
    }
}

/// Split into sentences on `.`/`!`/`?` terminators (kept with the sentence),
/// numbering non-empty sentences 1-based. A pure, dependency-free heuristic.
fn split_sentences(text: &str) -> Vec<Segment<'_>> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut loc = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'.' || c == b'!' || c == b'?' {
            // Extend over consecutive terminators (e.g. "?!" or "...").
            let mut end = i + 1;
            while end < bytes.len()
                && (bytes[end] == b'.' || bytes[end] == b'!' || bytes[end] == b'?')
            {
                end += 1;
            }
            let seg = &text[start..end];
            if !seg.trim().is_empty() {
                loc += 1;
                out.push(Segment { location: loc, text: seg });
            }
            start = end;
            i = end;
        } else {
            i += 1;
        }
    }
    // Trailing text with no terminator.
    if start < text.len() {
        let seg = &text[start..];
        if !seg.trim().is_empty() {
            loc += 1;
            out.push(Segment { location: loc, text: seg });
        }
    }
    out
}

/// Levenshtein edit distance between two char slices, short-circuiting once the
/// distance is known to exceed `cap` (returns `cap + 1`). Iterative two-row DP.
fn levenshtein_capped(a: &[char], b: &[char], cap: usize) -> usize {
    if a.len().abs_diff(b.len()) > cap {
        return cap + 1;
    }
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        let mut row_min = cur[0];
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
            row_min = row_min.min(cur[j + 1]);
        }
        if row_min > cap {
            return cap + 1;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Score how well `term` matches document word `word`, both already
/// case-normalised. Returns `None` for no match, else a per-match quality in
/// `0.0..=1.0` (1.0 = exact). `whole_word` disables substring/prefix matching.
fn word_score(term: &[char], word: &[char], fuzziness: usize, whole_word: bool) -> Option<f64> {
    if term.is_empty() || word.is_empty() {
        return None;
    }
    if term == word {
        return Some(1.0);
    }
    if !whole_word {
        // Substring / prefix containment (needs a term of length ≥ 2 to avoid
        // one-letter terms matching almost everything).
        if term.len() >= 2 && contains_subslice(word, term) {
            return Some(0.85);
        }
    }
    if fuzziness > 0 && term.len() >= 2 {
        let d = levenshtein_capped(term, word, fuzziness);
        if d <= fuzziness {
            // dist 1 → 0.7, dist 2 → 0.4, dist 3 → 0.1 (always > 0 and < substring).
            return Some((1.0 - 0.3 * d as f64).max(0.1));
        }
    }
    None
}

/// Whether `hay` contains the contiguous subslice `needle`.
fn contains_subslice(hay: &[char], needle: &[char]) -> bool {
    if needle.len() > hay.len() {
        return false;
    }
    hay.windows(needle.len()).any(|w| w == needle)
}

/// A word extracted from a snippet: its char slice plus byte range in the source.
struct Word {
    chars: Vec<char>,
    start: usize,
    end: usize,
}

/// Tokenise `s` into alphanumeric word runs, recording each word's byte range so
/// the snippet can be rebuilt with matched words highlighted.
fn words(s: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut cur: Vec<char> = Vec::new();
    let mut cur_start = 0usize;
    for (idx, ch) in s.char_indices() {
        if ch.is_alphanumeric() {
            if cur.is_empty() {
                cur_start = idx;
            }
            cur.push(ch);
        } else if !cur.is_empty() {
            out.push(Word { chars: std::mem::take(&mut cur), start: cur_start, end: idx });
        }
    }
    if !cur.is_empty() {
        let end = s.len();
        out.push(Word { chars: cur, start: cur_start, end });
    }
    out
}

/// Normalise a char slice for case-insensitive comparison.
fn norm(chars: &[char], case_sensitive: bool) -> Vec<char> {
    if case_sensitive {
        chars.to_vec()
    } else {
        chars.iter().flat_map(|c| c.to_lowercase()).collect()
    }
}

/// Run a fuzzy full-text search of `text` for `query`.
///
/// - `query` — whitespace-separated search terms. Empty → `Err`.
/// - `text` — the document(s) to search (paste txt/markdown; concatenate to
///   search across several). Empty → `Err`.
/// - `opts` — see [`Options`]; `fuzziness` and `max_results` are clamped.
///
/// Returns the ranked hits (best first), each with a location, a 0–100 score,
/// and the snippet with matched words wrapped in «…».
pub fn search(query: &str, text: &str, opts: Options) -> Result<SearchOutput, String> {
    let raw_terms: Vec<&str> = query.split_whitespace().collect();
    if raw_terms.is_empty() {
        return Err("query is empty: enter one or more words to search for".into());
    }
    if text.trim().is_empty() {
        return Err("document text is empty: paste the text to search".into());
    }

    let fuzziness = opts.fuzziness.min(FUZZINESS_CAP);
    let max_results = opts.max_results.clamp(1, MAX_RESULTS_CAP);

    // Pre-normalise each query term to its char vector.
    let terms: Vec<Vec<char>> = raw_terms
        .iter()
        .map(|t| norm(&t.chars().collect::<Vec<_>>(), opts.case_sensitive))
        .collect();

    // (score, location, snippet) for every matching segment.
    struct Scored {
        score: u32,
        location: usize,
        snippet: String,
    }
    let mut scored: Vec<Scored> = Vec::new();

    for seg in segment(text, opts.unit) {
        let seg_words = words(seg.text);
        // best per-term match quality, and which word byte-ranges to highlight.
        let mut best: Vec<f64> = vec![0.0; terms.len()];
        let mut highlight: Vec<(usize, usize)> = Vec::new();
        for w in &seg_words {
            let wn = norm(&w.chars, opts.case_sensitive);
            let mut word_matched = false;
            for (ti, term) in terms.iter().enumerate() {
                if let Some(q) = word_score(term, &wn, fuzziness, opts.whole_word) {
                    if q > best[ti] {
                        best[ti] = q;
                    }
                    word_matched = true;
                }
            }
            if word_matched {
                highlight.push((w.start, w.end));
            }
        }

        let matched_terms = best.iter().filter(|&&b| b > 0.0).count();
        let qualifies = if opts.match_all {
            matched_terms == terms.len()
        } else {
            matched_terms > 0
        };
        if !qualifies {
            continue;
        }

        // Relevance: mean quality of matched terms × coverage of the query.
        let sum: f64 = best.iter().sum();
        let mean = sum / matched_terms as f64;
        let coverage = matched_terms as f64 / terms.len() as f64;
        let score = (mean * coverage * 100.0).round() as u32;

        scored.push(Scored {
            score,
            location: seg.location,
            snippet: render_snippet(seg.text, &highlight),
        });
    }

    let total_matches = scored.len();
    // Rank: score desc, then location asc (stable, deterministic).
    scored.sort_by(|a, b| b.score.cmp(&a.score).then(a.location.cmp(&b.location)));
    scored.truncate(max_results);

    let hits: Vec<Hit> = scored
        .into_iter()
        .enumerate()
        .map(|(i, s)| Hit {
            rank: i + 1,
            score: s.score,
            location: s.location,
            snippet: s.snippet,
        })
        .collect();

    Ok(SearchOutput {
        terms: raw_terms.iter().map(|s| s.to_string()).collect(),
        count: hits.len(),
        total_matches,
        hits,
    })
}

/// Rebuild a snippet, trimming outer whitespace and wrapping each highlighted
/// word byte-range in «…». `ranges` are byte offsets into `seg` (ascending,
/// non-overlapping by construction).
fn render_snippet(seg: &str, ranges: &[(usize, usize)]) -> String {
    let mut out = String::with_capacity(seg.len() + ranges.len() * 4);
    let mut last = 0usize;
    for &(start, end) in ranges {
        out.push_str(&seg[last..start]);
        out.push('\u{00AB}'); // «
        out.push_str(&seg[start..end]);
        out.push('\u{00BB}'); // »
        last = end;
    }
    out.push_str(&seg[last..]);
    // Collapse leading/trailing whitespace (incl. newlines in a paragraph) so
    // the rendered snippet is compact.
    out.trim().to_string()
}

/// Render a [`SearchOutput`] as the plain-text block the standalone page shows.
pub fn render(out: &SearchOutput, unit: Unit) -> String {
    if out.hits.is_empty() {
        return format!("No matches for {:?}.", out.terms.join(" "));
    }
    let mut s = String::new();
    let noun = if out.total_matches == 1 { "match" } else { "matches" };
    s.push_str(&format!(
        "{} {} for {:?}",
        out.total_matches,
        noun,
        out.terms.join(" ")
    ));
    if out.count < out.total_matches {
        s.push_str(&format!(" (showing top {})", out.count));
    }
    for h in &out.hits {
        s.push_str(&format!(
            "\n\n#{}  {} {}  score {}\n{}",
            h.rank,
            unit.label(),
            h.location,
            h.score,
            h.snippet
        ));
    }
    s
}

/// One-call convenience for the page/CLI text surface: search then render.
pub fn search_text(query: &str, text: &str, opts: Options) -> Result<String, String> {
    let out = search(query, text, opts)?;
    Ok(render(&out, opts.unit))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn finds_exact_and_ranks_by_relevance() {
        let text = "The quick brown fox.\nA slow red fox jumps.\nUnrelated content here.";
        let out = search("fox", text, opts()).unwrap();
        assert_eq!(out.total_matches, 2, "two lines mention fox");
        assert_eq!(out.count, 2);
        // Both lines contain exactly one exact match → equal score → location order.
        assert_eq!(out.hits[0].location, 1);
        assert_eq!(out.hits[0].score, 100);
        assert!(out.hits[0].snippet.contains("\u{00AB}fox\u{00BB}"), "match highlighted");
    }

    #[test]
    fn fuzzy_matches_typos_within_distance() {
        let text = "Please recieve the parcel.\nNothing relevant.";
        // "receive" (correct spelling) is edit-distance 2 from the doc's "recieve".
        let out = search("receive", text, Options { fuzziness: 2, ..opts() }).unwrap();
        assert_eq!(out.count, 1);
        assert_eq!(out.hits[0].location, 1);
        assert!(out.hits[0].snippet.contains("\u{00AB}recieve\u{00BB}"));
    }

    #[test]
    fn fuzziness_zero_requires_exact() {
        let text = "Please recieve the parcel.";
        let out = search("receive", text, Options { fuzziness: 0, ..opts() }).unwrap();
        assert_eq!(out.count, 0, "no exact match at fuzziness 0");
        assert_eq!(out.total_matches, 0);
    }

    #[test]
    fn substring_prefix_matches_when_not_whole_word() {
        let text = "Documentation is important.";
        let out = search("doc", text, Options { fuzziness: 0, ..opts() }).unwrap();
        assert_eq!(out.count, 1, "'doc' is a prefix of 'Documentation'");
        assert!(out.hits[0].score < 100, "substring scores below an exact match");
    }

    #[test]
    fn whole_word_disables_substring() {
        let text = "Documentation is important.";
        let out = search(
            "doc",
            text,
            Options { fuzziness: 0, whole_word: true, ..opts() },
        )
        .unwrap();
        assert_eq!(out.count, 0, "whole_word: 'doc' must not match 'Documentation'");
    }

    #[test]
    fn match_all_requires_every_term() {
        let text = "alpha beta here.\nonly alpha here.";
        let any = search("alpha beta", text, opts()).unwrap();
        assert_eq!(any.total_matches, 2, "OR: both lines match at least one term");
        let all = search("alpha beta", text, Options { match_all: true, ..opts() }).unwrap();
        assert_eq!(all.total_matches, 1, "AND: only the line with both terms");
        assert_eq!(all.hits[0].location, 1);
    }

    #[test]
    fn coverage_ranks_multi_term_hits_higher() {
        let text = "alpha beta gamma.\nalpha only.";
        let out = search("alpha beta", text, opts()).unwrap();
        // Line 1 matches both terms (coverage 1.0) → higher than line 2 (0.5).
        assert_eq!(out.hits[0].location, 1);
        assert!(out.hits[0].score > out.hits[1].score);
    }

    #[test]
    fn case_sensitive_respects_case() {
        let text = "Rust is great. rust never sleeps.";
        let ci = search(
            "rust",
            text,
            Options { unit: Unit::Sentence, fuzziness: 0, ..opts() },
        )
        .unwrap();
        assert_eq!(ci.total_matches, 2, "case-insensitive matches both");
        let cs = search(
            "Rust",
            text,
            Options { unit: Unit::Sentence, case_sensitive: true, fuzziness: 0, ..opts() },
        )
        .unwrap();
        assert_eq!(cs.total_matches, 1, "case-sensitive matches only 'Rust'");
    }

    #[test]
    fn paragraph_unit_segments_on_blank_lines() {
        let text = "First para\nsecond line about cats.\n\nAnother para about dogs.";
        let out = search("cats", text, Options { unit: Unit::Paragraph, ..opts() }).unwrap();
        assert_eq!(out.count, 1);
        assert_eq!(out.hits[0].location, 1);
        assert!(out.hits[0].snippet.contains("cats"));
    }

    #[test]
    fn max_results_caps_and_reports_total() {
        let text = "cat\ncat\ncat\ncat\ncat";
        let out = search("cat", text, Options { max_results: 2, ..opts() }).unwrap();
        assert_eq!(out.total_matches, 5);
        assert_eq!(out.count, 2, "capped to max_results");
        assert_eq!(out.hits.len(), 2);
    }

    #[test]
    fn empty_query_errors() {
        let err = search("   ", "some text", opts()).unwrap_err();
        assert!(err.contains("query is empty"), "got: {err}");
    }

    #[test]
    fn empty_text_errors() {
        let err = search("cat", "   ", opts()).unwrap_err();
        assert!(err.contains("document text is empty"), "got: {err}");
    }

    #[test]
    fn unknown_unit_errors() {
        let err = Unit::parse("chapter").unwrap_err();
        assert!(err.contains("unknown unit"), "got: {err}");
    }

    #[test]
    fn render_no_hits_message() {
        let out = search("zzz", "cat dog", Options { fuzziness: 0, ..opts() }).unwrap();
        let text = render(&out, Unit::Line);
        assert_eq!(text, "No matches for \"zzz\".");
    }

    #[test]
    fn render_exact_format_single_hit() {
        let out = search("fox", "the quick fox", Options { fuzziness: 0, ..opts() }).unwrap();
        let text = render(&out, Unit::Line);
        assert_eq!(
            text,
            "1 match for \"fox\"\n\n#1  line 1  score 100\nthe quick \u{00AB}fox\u{00BB}"
        );
    }

    #[test]
    fn levenshtein_cap_short_circuits() {
        let a: Vec<char> = "kitten".chars().collect();
        let b: Vec<char> = "sitting".chars().collect();
        assert_eq!(levenshtein_capped(&a, &b, 5), 3);
        // Distance is 3, so a cap of 1 short-circuits to cap+1.
        assert_eq!(levenshtein_capped(&a, &b, 1), 2);
    }
}
