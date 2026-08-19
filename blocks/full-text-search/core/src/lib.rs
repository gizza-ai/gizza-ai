//! full-text-search core — pure, dependency-free BM25 / TF-IDF ranking over a
//! pasted corpus of documents. Shared by the chat skill block and the browser
//! page.
//!
//! The pipeline mirrors what a Lucene-class engine does, minus the persistence:
//!
//! 1. **Split** the paste into documents on a separator (`---` rules, blank
//!    lines, or form feeds). The first non-blank line of each document is its
//!    *title*.
//! 2. **Tokenise** every document into lowercased alphanumeric runs, keeping each
//!    token's byte range (for snippets) and its position (for phrase search).
//! 3. **Normalise** each token: optional English stop-word removal (query side)
//!    and an optional dependency-free Porter stemmer (`running` → `run`).
//! 4. **Score** each document against the query with BM25 (smoothed IDF, `k1` TF
//!    saturation, `b` length normalisation) or classic log-TF × smoothed-IDF,
//!    with matches in the title weighted by `title_boost`.
//! 5. **Render** the ranked hits as plain text with the matched words wrapped in
//!    guillemets («…»), or as JSON.
//!
//! Query syntax: bare words are terms, `"quoted runs"` must match as a
//! contiguous token sequence, and `-term` excludes any document containing it.
//!
//! No `wafer`/`wasm-bindgen`/regex deps: it compiles natively for unit tests, to
//! `wasm32-wasip1` (the `wafer build` chat target), and to
//! `wasm32-unknown-unknown` (the wasm-pack page target).

use serde::Serialize;
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Enums + options
// ---------------------------------------------------------------------------

/// How the pasted corpus is cut into separate documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Separator {
    /// A line of three or more dashes (`---`). The default.
    Dashes,
    /// One or more blank lines.
    BlankLine,
    /// An ASCII form feed (`\x0C`), as emitted by page-oriented extractors.
    FormFeed,
}

impl Separator {
    /// Parse the page/CLI string form. Errors on an unknown value.
    pub fn parse(s: &str) -> Result<Separator, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "dashes" => Ok(Separator::Dashes),
            "blank-line" | "blank_line" => Ok(Separator::BlankLine),
            "form-feed" | "form_feed" => Ok(Separator::FormFeed),
            other => Err(format!(
                "unknown separator {other:?}: expected dashes, blank-line, or form-feed"
            )),
        }
    }
}

/// The relevance-scoring function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// Okapi BM25 — smoothed IDF, `k1` term-frequency saturation, `b` document
    /// length normalisation. The default.
    Bm25,
    /// Classic log-scaled TF × smoothed IDF, length-normalised by `sqrt(len)`.
    TfIdf,
}

impl Algorithm {
    /// Parse the page/CLI string form. Errors on an unknown value.
    pub fn parse(s: &str) -> Result<Algorithm, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "bm25" => Ok(Algorithm::Bm25),
            "tfidf" | "tf-idf" => Ok(Algorithm::TfIdf),
            other => Err(format!(
                "unknown algorithm {other:?}: expected bm25 or tfidf"
            )),
        }
    }
    /// Human label used in the rendered header and the JSON output.
    pub fn label(self) -> &'static str {
        match self {
            Algorithm::Bm25 => "BM25",
            Algorithm::TfIdf => "TF-IDF",
        }
    }
}

/// Whether a document needs one query term or all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    /// At least one query term/phrase (OR). The default.
    Any,
    /// Every query term/phrase (AND).
    All,
}

impl MatchMode {
    /// Parse the page/CLI string form. Errors on an unknown value.
    pub fn parse(s: &str) -> Result<MatchMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "any" => Ok(MatchMode::Any),
            "all" => Ok(MatchMode::All),
            other => Err(format!("unknown match {other:?}: expected any or all")),
        }
    }
}

/// The rendering surface for the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Ranked plain text with «…» highlights. The default.
    Text,
    /// Pretty-printed JSON of the full [`SearchOutput`].
    Json,
}

impl OutputFormat {
    /// Parse the page/CLI string form. Errors on an unknown value.
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => Ok(OutputFormat::Text),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!("unknown output {other:?}: expected text or json")),
        }
    }
}

/// Hard cap on `max_results` (also the page/schema max).
pub const MAX_RESULTS_CAP: usize = 50;
/// Hard cap on `snippet_words` (also the page/schema max). `0` disables snippets.
pub const SNIPPET_WORDS_CAP: usize = 120;
/// Hard cap on `k1` (also the page/schema max).
pub const K1_CAP: f64 = 3.0;
/// Hard cap on `title_boost` (also the page/schema max).
pub const TITLE_BOOST_CAP: f64 = 5.0;

/// Search options. Every numeric field is clamped into range by [`search`].
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// How the paste is cut into documents.
    pub separator: Separator,
    /// The scoring function.
    pub algorithm: Algorithm,
    /// Reduce words to their Porter stem so `running` matches `run`.
    pub stemming: bool,
    /// Drop English stop words (`the`, `of`, …) from the query. Skipped when it
    /// would leave nothing to search for.
    pub stopwords: bool,
    /// Require any (OR) or all (AND) query terms.
    pub match_mode: MatchMode,
    /// Also match terms that merely *start with* a query term (`moto` →
    /// `motorcycle`).
    pub prefix: bool,
    /// Maximum ranked documents returned, 1..=[`MAX_RESULTS_CAP`].
    pub max_results: usize,
    /// Words of context around the best match, 0..=[`SNIPPET_WORDS_CAP`].
    /// `0` returns hits without a snippet.
    pub snippet_words: usize,
    /// BM25 term-frequency saturation, 0.0..=[`K1_CAP`] (standard 1.2–2.0).
    pub k1: f64,
    /// BM25 length normalisation, 0.0..=1.0 (standard 0.75; 0 = off).
    pub b: f64,
    /// Weight applied to matches in a document's first line (its title),
    /// 1.0..=[`TITLE_BOOST_CAP`]. 1.0 = no boost.
    pub title_boost: f64,
    /// Text or JSON rendering.
    pub output: OutputFormat,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            separator: Separator::Dashes,
            algorithm: Algorithm::Bm25,
            stemming: true,
            stopwords: true,
            match_mode: MatchMode::Any,
            prefix: false,
            max_results: 10,
            snippet_words: 30,
            k1: 1.2,
            b: 0.75,
            title_boost: 2.0,
            output: OutputFormat::Text,
        }
    }
}

impl Options {
    /// Clamp every numeric knob into its documented range.
    fn clamped(mut self) -> Options {
        self.max_results = self.max_results.clamp(1, MAX_RESULTS_CAP);
        self.snippet_words = self.snippet_words.min(SNIPPET_WORDS_CAP);
        self.k1 = clamp_f64(self.k1, 0.0, K1_CAP);
        self.b = clamp_f64(self.b, 0.0, 1.0);
        self.title_boost = clamp_f64(self.title_boost, 1.0, TITLE_BOOST_CAP);
        self
    }
}

/// `f64::clamp` panics on NaN bounds; this also maps a NaN value to `lo`.
fn clamp_f64(v: f64, lo: f64, hi: f64) -> f64 {
    if v.is_nan() {
        lo
    } else {
        v.max(lo).min(hi)
    }
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// One ranked document hit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    /// 1-based rank in the returned list (1 = best).
    pub rank: usize,
    /// 1-based index of the document within the pasted corpus.
    pub doc: usize,
    /// The document's first non-blank line, trimmed (its "title").
    pub title: String,
    /// Relevance score in the chosen algorithm's units, rounded to 4 decimals.
    /// Comparable within one result set only — not a 0–100 percentage.
    pub score: f64,
    /// Which normalised query terms/phrases this document actually matched.
    pub matched: Vec<String>,
    /// Keyword-in-context window around the best match, matched words wrapped in
    /// «…». Empty when `snippet_words = 0`.
    pub snippet: String,
}

/// The full structured search result (what the chat/CLI/JSON surface returns).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchOutput {
    /// The raw query as typed.
    pub query: String,
    /// The scoring function used (`BM25` / `TF-IDF`).
    pub algorithm: String,
    /// Normalised (lowercased, optionally stemmed) query terms actually searched.
    pub terms: Vec<String>,
    /// Normalised quoted phrases, each rendered as space-joined tokens.
    pub phrases: Vec<String>,
    /// Normalised `-term` exclusions applied.
    pub excluded: Vec<String>,
    /// Number of documents the corpus was split into.
    pub documents: usize,
    /// Number of hits returned (≤ `max_results`).
    pub count: usize,
    /// Documents that matched before the `max_results` cap.
    pub total_matches: usize,
    /// The ranked hits.
    pub hits: Vec<Hit>,
}

// ---------------------------------------------------------------------------
// Tokenising
// ---------------------------------------------------------------------------

/// One token of a document: its normalised text plus its byte range in the
/// document, so a snippet can be cut from the ORIGINAL text.
#[derive(Debug, Clone)]
struct Token {
    text: String,
    start: usize,
    end: usize,
}

/// Split `text` into lowercased alphanumeric runs, applying the Porter stemmer
/// when `stemming` is on. Byte ranges refer to `text`.
fn tokenize(text: &str, stemming: bool) -> Vec<Token> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut start = 0usize;
    for (i, ch) in text.char_indices() {
        if ch.is_alphanumeric() {
            if cur.is_empty() {
                start = i;
            }
            cur.extend(ch.to_lowercase());
        } else if !cur.is_empty() {
            out.push(Token {
                text: normalize(&cur, stemming),
                start,
                end: i,
            });
            cur.clear();
        }
    }
    if !cur.is_empty() {
        out.push(Token {
            text: normalize(&cur, stemming),
            start,
            end: text.len(),
        });
    }
    out
}

/// Apply the optional stemmer to an already-lowercased word.
fn normalize(word: &str, stemming: bool) -> String {
    if stemming {
        stem(word)
    } else {
        word.to_string()
    }
}

/// Tokenise a query fragment to normalised words (no byte ranges needed).
fn tokenize_words(text: &str, stemming: bool) -> Vec<String> {
    tokenize(text, stemming)
        .into_iter()
        .map(|t| t.text)
        .collect()
}

// ---------------------------------------------------------------------------
// Stop words
// ---------------------------------------------------------------------------

/// English stop words, the conventional search-engine list. Applied to the
/// query only, so phrase positions in the documents stay intact.
const STOP_WORDS: &[&str] = &[
    "a",
    "about",
    "above",
    "after",
    "again",
    "against",
    "all",
    "am",
    "an",
    "and",
    "any",
    "are",
    "as",
    "at",
    "be",
    "because",
    "been",
    "before",
    "being",
    "below",
    "between",
    "both",
    "but",
    "by",
    "can",
    "cannot",
    "could",
    "did",
    "do",
    "does",
    "doing",
    "down",
    "during",
    "each",
    "few",
    "for",
    "from",
    "further",
    "had",
    "has",
    "have",
    "having",
    "he",
    "her",
    "here",
    "hers",
    "herself",
    "him",
    "himself",
    "his",
    "how",
    "i",
    "if",
    "in",
    "into",
    "is",
    "it",
    "its",
    "itself",
    "just",
    "me",
    "more",
    "most",
    "my",
    "myself",
    "no",
    "nor",
    "not",
    "now",
    "of",
    "off",
    "on",
    "once",
    "only",
    "or",
    "other",
    "ought",
    "our",
    "ours",
    "ourselves",
    "out",
    "over",
    "own",
    "same",
    "she",
    "should",
    "so",
    "some",
    "such",
    "than",
    "that",
    "the",
    "their",
    "theirs",
    "them",
    "themselves",
    "then",
    "there",
    "these",
    "they",
    "this",
    "those",
    "through",
    "to",
    "too",
    "under",
    "until",
    "up",
    "very",
    "was",
    "we",
    "were",
    "what",
    "when",
    "where",
    "which",
    "while",
    "who",
    "whom",
    "why",
    "will",
    "with",
    "would",
    "you",
    "your",
    "yours",
    "yourself",
    "yourselves",
];

/// Whether `word` is an English stop word. The list is compared against the
/// *unstemmed* lowercase form, so stemming and stop words compose correctly.
fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word)
}

// ---------------------------------------------------------------------------
// Porter stemmer (Porter, 1980) — dependency-free
// ---------------------------------------------------------------------------

/// Reduce an English word to its Porter stem (`running` → `run`, `policies` →
/// `polici`). Non-ASCII or very short words are returned unchanged: the
/// algorithm is defined for English ASCII only.
pub fn stem(word: &str) -> String {
    if word.len() <= 2 || !word.chars().all(|c| c.is_ascii_lowercase()) {
        return word.to_string();
    }
    let mut w: Vec<char> = word.chars().collect();
    step1a(&mut w);
    step1b(&mut w);
    step1c(&mut w);
    step2(&mut w);
    step3(&mut w);
    step4(&mut w);
    step5(&mut w);
    w.into_iter().collect()
}

/// Porter's consonant test: `y` is a consonant unless preceded by a consonant.
fn is_consonant(w: &[char], i: usize) -> bool {
    match w[i] {
        'a' | 'e' | 'i' | 'o' | 'u' => false,
        'y' => i == 0 || !is_consonant(w, i - 1),
        _ => true,
    }
}

/// Porter's measure `m`: the number of vowel-consonant sequences in `w`.
fn measure(w: &[char]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < w.len() && is_consonant(w, i) {
        i += 1;
    }
    while i < w.len() {
        while i < w.len() && !is_consonant(w, i) {
            i += 1;
        }
        if i >= w.len() {
            break;
        }
        while i < w.len() && is_consonant(w, i) {
            i += 1;
        }
        n += 1;
    }
    n
}

/// Whether the slice contains a vowel.
fn has_vowel(w: &[char]) -> bool {
    (0..w.len()).any(|i| !is_consonant(w, i))
}

/// Whether the slice ends in a doubled consonant (`hopp`, `fall`).
fn ends_double_consonant(w: &[char]) -> bool {
    w.len() >= 2 && w[w.len() - 1] == w[w.len() - 2] && is_consonant(w, w.len() - 1)
}

/// Porter's `*o` test: ends consonant-vowel-consonant where the final consonant
/// is not `w`, `x` or `y`.
fn ends_cvc(w: &[char]) -> bool {
    let n = w.len();
    n >= 3
        && is_consonant(w, n - 3)
        && !is_consonant(w, n - 2)
        && is_consonant(w, n - 1)
        && !matches!(w[n - 1], 'w' | 'x' | 'y')
}

/// Whether `w` ends with `suffix`.
fn ends_with(w: &[char], suffix: &str) -> bool {
    let s: Vec<char> = suffix.chars().collect();
    w.len() > s.len() && w[w.len() - s.len()..] == s[..]
}

/// Replace a trailing `suffix` with `with`.
fn replace_suffix(w: &mut Vec<char>, suffix: &str, with: &str) {
    let cut = w.len() - suffix.chars().count();
    w.truncate(cut);
    w.extend(with.chars());
}

/// Apply the first rule whose suffix matches AND whose stem has measure > `min_m`.
/// Rules must be ordered longest-suffix-first.
fn apply_rules(w: &mut Vec<char>, rules: &[(&str, &str)], min_m: usize) -> bool {
    for (suffix, with) in rules {
        if ends_with(w, suffix) {
            let stem_len = w.len() - suffix.chars().count();
            if measure(&w[..stem_len]) > min_m {
                replace_suffix(w, suffix, with);
            }
            return true;
        }
    }
    false
}

/// Plurals: `caresses` → `caress`, `ponies` → `poni`, `cats` → `cat`.
fn step1a(w: &mut Vec<char>) {
    if ends_with(w, "sses") {
        replace_suffix(w, "sses", "ss");
    } else if ends_with(w, "ies") {
        replace_suffix(w, "ies", "i");
    } else if ends_with(w, "ss") {
        // unchanged
    } else if ends_with(w, "s") {
        replace_suffix(w, "s", "");
    }
}

/// Past participles / gerunds: `agreed` → `agree`, `hopping` → `hop`.
fn step1b(w: &mut Vec<char>) {
    let mut ran = false;
    if ends_with(w, "eed") {
        let stem_len = w.len() - 3;
        if measure(&w[..stem_len]) > 0 {
            replace_suffix(w, "eed", "ee");
        }
        return;
    }
    if ends_with(w, "ed") {
        let stem_len = w.len() - 2;
        if has_vowel(&w[..stem_len]) {
            replace_suffix(w, "ed", "");
            ran = true;
        }
    } else if ends_with(w, "ing") {
        let stem_len = w.len() - 3;
        if has_vowel(&w[..stem_len]) {
            replace_suffix(w, "ing", "");
            ran = true;
        }
    }
    if !ran {
        return;
    }
    if ends_with(w, "at") || ends_with(w, "bl") || ends_with(w, "iz") {
        w.push('e');
    } else if ends_double_consonant(w) && !matches!(w[w.len() - 1], 'l' | 's' | 'z') {
        w.pop();
    } else if measure(w) == 1 && ends_cvc(w) {
        w.push('e');
    }
}

/// Terminal `y` → `i` when the stem has a vowel (`happy` → `happi`).
fn step1c(w: &mut Vec<char>) {
    if ends_with(w, "y") && has_vowel(&w[..w.len() - 1]) {
        replace_suffix(w, "y", "i");
    }
}

/// Double suffixes to single ones (`relational` → `relate`).
fn step2(w: &mut Vec<char>) {
    const RULES: &[(&str, &str)] = &[
        ("ational", "ate"),
        ("ization", "ize"),
        ("iveness", "ive"),
        ("fulness", "ful"),
        ("ousness", "ous"),
        ("biliti", "ble"),
        ("tional", "tion"),
        ("ousli", "ous"),
        ("entli", "ent"),
        ("ation", "ate"),
        ("alism", "al"),
        ("aliti", "al"),
        ("iviti", "ive"),
        ("enci", "ence"),
        ("anci", "ance"),
        ("izer", "ize"),
        ("alli", "al"),
        ("ator", "ate"),
        ("logi", "log"),
        ("bli", "ble"),
        ("eli", "e"),
    ];
    apply_rules(w, RULES, 0);
}

/// `-ic-`, `-ful`, `-ness` families (`triplicate` → `triplic`).
fn step3(w: &mut Vec<char>) {
    const RULES: &[(&str, &str)] = &[
        ("icate", "ic"),
        ("ative", ""),
        ("alize", "al"),
        ("iciti", "ic"),
        ("ical", "ic"),
        ("ness", ""),
        ("ful", ""),
    ];
    apply_rules(w, RULES, 0);
}

/// Strip residual derivational suffixes on longer stems (`allowance` → `allow`).
fn step4(w: &mut Vec<char>) {
    const RULES: &[(&str, &str)] = &[
        ("ement", ""),
        ("ance", ""),
        ("ence", ""),
        ("able", ""),
        ("ible", ""),
        ("ment", ""),
        ("ant", ""),
        ("ent", ""),
        ("ism", ""),
        ("ate", ""),
        ("iti", ""),
        ("ous", ""),
        ("ive", ""),
        ("ize", ""),
        ("al", ""),
        ("er", ""),
        ("ic", ""),
        ("ou", ""),
    ];
    // `-ion` only applies after `s` or `t`; handle it before the generic list so
    // "adoption" -> "adopt" but "lion" is left alone.
    if ends_with(w, "ion") {
        let stem_len = w.len() - 3;
        if measure(&w[..stem_len]) > 1 && matches!(w[stem_len - 1], 's' | 't') {
            replace_suffix(w, "ion", "");
        }
        return;
    }
    apply_rules(w, RULES, 1);
}

/// Tidy-up: drop a final `e`, undo a doubled `l` (`controll` → `control`).
fn step5(w: &mut Vec<char>) {
    if ends_with(w, "e") {
        let stem_len = w.len() - 1;
        let m = measure(&w[..stem_len]);
        if m > 1 || (m == 1 && !ends_cvc(&w[..stem_len])) {
            w.pop();
        }
    }
    if w.len() > 1 && w[w.len() - 1] == 'l' && ends_double_consonant(w) && measure(w) > 1 {
        w.pop();
    }
}

// ---------------------------------------------------------------------------
// Query parsing
// ---------------------------------------------------------------------------

/// A parsed query: bare terms, quoted phrases, and `-term` exclusions, all
/// normalised (lowercased + optionally stemmed).
#[derive(Debug, Default, Clone, PartialEq)]
struct Query {
    terms: Vec<String>,
    phrases: Vec<Vec<String>>,
    excluded: Vec<String>,
    excluded_phrases: Vec<Vec<String>>,
}

impl Query {
    /// Nothing left to search for (exclusions alone are not a query).
    fn is_empty(&self) -> bool {
        self.terms.is_empty() && self.phrases.is_empty()
    }
}

/// Parse the query string: `"quoted phrase"` runs, `-excluded` terms, bare
/// words. Stop words are dropped from bare terms when `stopwords` is on, unless
/// that would empty the query.
fn parse_query(raw: &str, stemming: bool, stopwords: bool) -> Query {
    let chars: Vec<char> = raw.chars().collect();
    let mut q = Query::default();
    // Bare terms before stop-word filtering, so we can fall back to them.
    let mut raw_terms: Vec<String> = Vec::new();
    let mut kept_terms: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let negated = chars[i] == '-' && i + 1 < chars.len() && !chars[i + 1].is_whitespace();
        if negated {
            i += 1;
        }
        if i < chars.len() && chars[i] == '"' {
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            let body: String = chars[start..i].iter().collect();
            if i < chars.len() {
                i += 1; // closing quote
            }
            let tokens = tokenize_words(&body, stemming);
            if tokens.is_empty() {
                continue;
            }
            if negated {
                q.excluded_phrases.push(tokens);
            } else if !q.phrases.contains(&tokens) {
                q.phrases.push(tokens);
            }
            continue;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        for token in tokenize(&word, false) {
            let normalized = normalize(&token.text, stemming);
            if negated {
                if !q.excluded.contains(&normalized) {
                    q.excluded.push(normalized);
                }
            } else {
                if !raw_terms.contains(&normalized) {
                    raw_terms.push(normalized.clone());
                }
                if (!stopwords || !is_stop_word(&token.text)) && !kept_terms.contains(&normalized) {
                    kept_terms.push(normalized);
                }
            }
        }
    }
    // Stop-word filtering must never leave the user with nothing to search for.
    q.terms = if kept_terms.is_empty() && q.phrases.is_empty() {
        raw_terms
    } else {
        kept_terms
    };
    q
}

// ---------------------------------------------------------------------------
// Corpus splitting + indexing
// ---------------------------------------------------------------------------

/// One indexed document.
struct Doc {
    /// The original text of this document (untrimmed, for exact snippet slices).
    text: String,
    /// First non-blank line, trimmed.
    title: String,
    tokens: Vec<Token>,
    /// Byte offset at which the title line ends; tokens before it get the boost.
    title_end: usize,
    /// Normalised term → ascending token positions.
    postings: HashMap<String, Vec<usize>>,
}

impl Doc {
    /// Weighted term frequency: title occurrences count `title_boost` times.
    fn weighted_tf(&self, term: &str, title_boost: f64) -> f64 {
        match self.postings.get(term) {
            None => 0.0,
            Some(positions) => positions
                .iter()
                .map(|&p| {
                    if self.tokens[p].start < self.title_end {
                        title_boost
                    } else {
                        1.0
                    }
                })
                .sum(),
        }
    }
}

/// Cut the pasted corpus into document texts on the chosen separator, dropping
/// blank documents.
fn split_documents(corpus: &str, separator: Separator) -> Vec<String> {
    let parts: Vec<String> = match separator {
        Separator::FormFeed => corpus.split('\u{000C}').map(|s| s.to_string()).collect(),
        Separator::Dashes => {
            let mut out = vec![String::new()];
            for line in corpus.split('\n') {
                let t = line.trim();
                if t.len() >= 3 && t.chars().all(|c| c == '-') {
                    out.push(String::new());
                } else {
                    let cur = out.last_mut().expect("always non-empty");
                    if !cur.is_empty() {
                        cur.push('\n');
                    }
                    cur.push_str(line);
                }
            }
            out
        }
        Separator::BlankLine => {
            let mut out = vec![String::new()];
            let mut blank_run = false;
            for line in corpus.split('\n') {
                if line.trim().is_empty() {
                    if !blank_run && !out.last().expect("always non-empty").trim().is_empty() {
                        out.push(String::new());
                    }
                    blank_run = true;
                } else {
                    blank_run = false;
                    let cur = out.last_mut().expect("always non-empty");
                    if !cur.is_empty() {
                        cur.push('\n');
                    }
                    cur.push_str(line);
                }
            }
            out
        }
    };
    parts.into_iter().filter(|p| !p.trim().is_empty()).collect()
}

/// Index one document: tokenise, record the title span, build the postings.
fn index_doc(text: String, stemming: bool) -> Doc {
    // The title is the first non-blank line; `title_end` is its byte end.
    let mut title = String::new();
    let mut title_end = 0usize;
    let mut offset = 0usize;
    for line in text.split('\n') {
        if !line.trim().is_empty() {
            title = line.trim().to_string();
            title_end = offset + line.len();
            break;
        }
        offset += line.len() + 1;
    }
    let tokens = tokenize(&text, stemming);
    let mut postings: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, t) in tokens.iter().enumerate() {
        postings.entry(t.text.clone()).or_default().push(i);
    }
    Doc {
        text,
        title,
        tokens,
        title_end,
        postings,
    }
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// One thing the query asks for: a term (possibly prefix-expanded) or a phrase.
struct Unit {
    /// The label reported in `matched` / used for `all` bookkeeping.
    label: String,
    /// Per-document weighted term frequency, parallel to the doc list.
    tf: Vec<f64>,
    /// Per-document matched token positions (for the snippet window).
    positions: Vec<Vec<usize>>,
    /// Documents containing this unit.
    df: usize,
}

/// Expand a query term to the document terms it matches, honouring `prefix`.
fn expand<'a>(term: &str, vocab: &'a HashSet<String>, prefix: bool) -> Vec<&'a str> {
    if !prefix {
        return vocab
            .get(term)
            .map(|s| vec![s.as_str()])
            .unwrap_or_default();
    }
    let mut out: Vec<&str> = vocab
        .iter()
        .filter(|v| v.starts_with(term))
        .map(|v| v.as_str())
        .collect();
    out.sort_unstable();
    out
}

/// Contiguous occurrences of `phrase` in `doc`, returned as the token positions
/// of every token of every occurrence.
fn phrase_positions(doc: &Doc, phrase: &[String]) -> Vec<usize> {
    let mut out = Vec::new();
    let Some(first) = doc.postings.get(&phrase[0]) else {
        return out;
    };
    for &start in first {
        if start + phrase.len() > doc.tokens.len() {
            break;
        }
        if phrase
            .iter()
            .enumerate()
            .all(|(k, w)| doc.tokens[start + k].text == *w)
        {
            out.extend(start..start + phrase.len());
        }
    }
    out
}

/// Rank the documents in `corpus` against `query`.
///
/// Errors when the query or the corpus is empty after normalisation.
pub fn search(query: &str, corpus: &str, opts: Options) -> Result<SearchOutput, String> {
    let opts = opts.clamped();
    if corpus.trim().is_empty() {
        return Err("corpus is empty: paste at least one document".to_string());
    }
    let q = parse_query(query, opts.stemming, opts.stopwords);
    if q.is_empty() {
        return Err(
            "query is empty: enter at least one word to search for (a lone -exclusion is not a query)"
                .to_string(),
        );
    }

    let docs: Vec<Doc> = split_documents(corpus, opts.separator)
        .into_iter()
        .map(|t| index_doc(t, opts.stemming))
        .collect();
    if docs.is_empty() {
        return Err("corpus is empty: paste at least one document".to_string());
    }
    let n = docs.len();
    let lengths: Vec<f64> = docs.iter().map(|d| d.tokens.len() as f64).collect();
    let avgdl = (lengths.iter().sum::<f64>() / n as f64).max(1.0);
    let vocab: HashSet<String> = docs
        .iter()
        .flat_map(|d| d.postings.keys().cloned())
        .collect();

    // Build one Unit per query term and per phrase.
    let mut units: Vec<Unit> = Vec::new();
    for term in &q.terms {
        let matches = expand(term, &vocab, opts.prefix);
        let mut unit = Unit {
            label: term.clone(),
            tf: vec![0.0; n],
            positions: vec![Vec::new(); n],
            df: 0,
        };
        for (i, doc) in docs.iter().enumerate() {
            for m in &matches {
                unit.tf[i] += doc.weighted_tf(m, opts.title_boost);
                if let Some(p) = doc.postings.get(*m) {
                    unit.positions[i].extend(p.iter().copied());
                }
            }
            if unit.tf[i] > 0.0 {
                unit.df += 1;
            }
        }
        units.push(unit);
    }
    for phrase in &q.phrases {
        let mut unit = Unit {
            label: phrase.join(" "),
            tf: vec![0.0; n],
            positions: vec![Vec::new(); n],
            df: 0,
        };
        for (i, doc) in docs.iter().enumerate() {
            let hits = phrase_positions(doc, phrase);
            if !hits.is_empty() {
                // One "occurrence" per contiguous run of phrase.len() tokens.
                let occurrences = (hits.len() / phrase.len()) as f64;
                let in_title = hits
                    .iter()
                    .filter(|&&p| doc.tokens[p].start < doc.title_end)
                    .count() as f64
                    / phrase.len() as f64;
                unit.tf[i] = occurrences - in_title + in_title * opts.title_boost;
                unit.positions[i] = hits;
                unit.df += 1;
            }
        }
        units.push(unit);
    }

    // Exclusions: any document containing an excluded term/phrase is dropped.
    let mut excluded_doc = vec![false; n];
    for term in &q.excluded {
        for m in expand(term, &vocab, opts.prefix) {
            for (i, doc) in docs.iter().enumerate() {
                if doc.postings.contains_key(m) {
                    excluded_doc[i] = true;
                }
            }
        }
    }
    for phrase in &q.excluded_phrases {
        for (i, doc) in docs.iter().enumerate() {
            if !phrase_positions(doc, phrase).is_empty() {
                excluded_doc[i] = true;
            }
        }
    }

    // Score.
    let mut scored: Vec<(usize, f64, Vec<String>, Vec<usize>)> = Vec::new();
    for i in 0..n {
        if excluded_doc[i] {
            continue;
        }
        let matched_units: Vec<&Unit> = units.iter().filter(|u| u.tf[i] > 0.0).collect();
        let required = match opts.match_mode {
            MatchMode::Any => 1,
            MatchMode::All => units.len(),
        };
        if matched_units.len() < required {
            continue;
        }
        let mut score = 0.0f64;
        for u in &matched_units {
            let idf = match opts.algorithm {
                Algorithm::Bm25 => {
                    (1.0 + (n as f64 - u.df as f64 + 0.5) / (u.df as f64 + 0.5)).ln()
                }
                Algorithm::TfIdf => ((n as f64 + 1.0) / (u.df as f64 + 1.0)).ln() + 1.0,
            };
            let tf = u.tf[i];
            score += match opts.algorithm {
                Algorithm::Bm25 => {
                    let denom = tf + opts.k1 * (1.0 - opts.b + opts.b * lengths[i] / avgdl);
                    idf * (tf * (opts.k1 + 1.0)) / denom
                }
                Algorithm::TfIdf => (1.0 + tf.ln()) * idf / lengths[i].max(1.0).sqrt(),
            };
        }
        let matched: Vec<String> = matched_units.iter().map(|u| u.label.clone()).collect();
        let mut positions: Vec<usize> = matched_units
            .iter()
            .flat_map(|u| u.positions[i].iter().copied())
            .collect();
        positions.sort_unstable();
        positions.dedup();
        scored.push((i, round4(score), matched, positions));
    }

    let total_matches = scored.len();
    // Score descending, then document order — deterministic ties.
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    scored.truncate(opts.max_results);

    let hits: Vec<Hit> = scored
        .into_iter()
        .enumerate()
        .map(|(rank, (i, score, matched, positions))| Hit {
            rank: rank + 1,
            doc: i + 1,
            title: docs[i].title.clone(),
            score,
            matched,
            snippet: build_snippet(&docs[i], &positions, opts.snippet_words),
        })
        .collect();

    Ok(SearchOutput {
        query: query.trim().to_string(),
        algorithm: opts.algorithm.label().to_string(),
        terms: q.terms,
        phrases: q.phrases.iter().map(|p| p.join(" ")).collect(),
        excluded: q
            .excluded
            .iter()
            .cloned()
            .chain(q.excluded_phrases.iter().map(|p| p.join(" ")))
            .collect(),
        documents: n,
        count: hits.len(),
        total_matches,
        hits,
    })
}

/// Round to 4 decimals so scores are stable across platforms and readable in JSON.
fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

// ---------------------------------------------------------------------------
// Snippets
// ---------------------------------------------------------------------------

/// Cut a `words`-token window around the densest run of matches and wrap the
/// matched tokens in «…». Returns an empty string when `words == 0`.
fn build_snippet(doc: &Doc, positions: &[usize], words: usize) -> String {
    if words == 0 || doc.tokens.is_empty() {
        return String::new();
    }
    let total = doc.tokens.len();
    let hit_set: HashSet<usize> = positions.iter().copied().collect();
    // Pick the window (centred on a match) containing the most matches.
    let mut best_start = 0usize;
    let mut best_hits = 0usize;
    for &p in positions {
        let start = p
            .saturating_sub(words / 2)
            .min(total.saturating_sub(words).max(0));
        let end = (start + words).min(total);
        let hits = (start..end).filter(|k| hit_set.contains(k)).count();
        if hits > best_hits {
            best_hits = hits;
            best_start = start;
        }
    }
    let start = best_start;
    let end = (start + words).min(total);

    let mut out = String::new();
    let mut cursor = doc.tokens[start].start;
    for k in start..end {
        let t = &doc.tokens[k];
        out.push_str(&doc.text[cursor..t.start]);
        if hit_set.contains(&k) {
            out.push('\u{00AB}'); // «
            out.push_str(&doc.text[t.start..t.end]);
            out.push('\u{00BB}'); // »
        } else {
            out.push_str(&doc.text[t.start..t.end]);
        }
        cursor = t.end;
    }
    let mut snippet = collapse_whitespace(&out);
    if start > 0 {
        snippet.insert(0, '\u{2026}'); // …
    }
    if end < total {
        snippet.push('\u{2026}');
    }
    snippet
}

/// Collapse every whitespace run (including newlines) to one space and trim, so
/// a snippet is always a single line.
fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            in_ws = true;
        } else {
            if in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = false;
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Truncate a title for the one-line result header.
fn short_title(title: &str) -> String {
    const MAX: usize = 60;
    if title.chars().count() <= MAX {
        return title.to_string();
    }
    let mut s: String = title.chars().take(MAX - 1).collect();
    s.push('\u{2026}');
    s
}

/// Render a [`SearchOutput`] as the plain-text block the page and CLI show.
pub fn render(out: &SearchOutput) -> String {
    let doc_word = if out.documents == 1 {
        "document"
    } else {
        "documents"
    };
    if out.hits.is_empty() {
        return format!(
            "No documents match {:?} (searched {} {doc_word}).",
            out.query, out.documents
        );
    }
    let mut s = format!(
        "{} · {} of {} {doc_word} match {:?}",
        out.algorithm, out.total_matches, out.documents, out.query
    );
    if out.count < out.total_matches {
        s.push_str(&format!(" (showing top {})", out.count));
    }
    for h in &out.hits {
        s.push_str(&format!(
            "\n\n#{}  doc {}  score {:.2}  {}",
            h.rank,
            h.doc,
            h.score,
            short_title(&h.title)
        ));
        if !h.snippet.is_empty() {
            s.push_str("\n    ");
            s.push_str(&h.snippet);
        }
    }
    s
}

/// One-call convenience for the page/CLI/chat surface: search, then render as
/// text or JSON per `opts.output`.
pub fn search_text(query: &str, corpus: &str, opts: Options) -> Result<String, String> {
    let out = search(query, corpus, opts)?;
    match opts.output {
        OutputFormat::Text => Ok(render(&out)),
        OutputFormat::Json => {
            serde_json::to_string_pretty(&out).map_err(|e| format!("failed to render JSON: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORPUS: &str = "Refund policy\nRefunds are processed within five working days of the return arriving.\n---\nShipping policy\nOrders are dispatched within two days. Tracking is emailed on dispatch.\n---\nContact us\nSupport answers email within one working day.";

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn ranks_the_document_that_mentions_the_term_most() {
        let out = search("refund", CORPUS, opts()).unwrap();
        assert_eq!(out.documents, 3);
        assert_eq!(out.total_matches, 1);
        assert_eq!(out.hits[0].doc, 1);
        assert_eq!(out.hits[0].title, "Refund policy");
        assert!(out.hits[0].score > 0.0);
        assert!(
            out.hits[0].snippet.contains('\u{00AB}'),
            "match highlighted: {}",
            out.hits[0].snippet
        );
    }

    #[test]
    fn bm25_idf_prefers_the_rarer_term() {
        // "days" appears in two documents, "tracking" in one → rarer wins.
        let out = search("days tracking", CORPUS, opts()).unwrap();
        assert_eq!(out.hits[0].doc, 2, "the doc with the rare term ranks first");
    }

    #[test]
    fn stemming_matches_inflected_forms() {
        let corpus = "Runners\nThe runner was running fast.\n---\nCycling\nNothing relevant here.";
        let out = search("run", corpus, opts()).unwrap();
        assert_eq!(out.total_matches, 1);
        assert_eq!(out.hits[0].doc, 1, "'run' stems to match 'running'");
        let off = search(
            "run",
            corpus,
            Options {
                stemming: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(
            off.total_matches, 0,
            "without stemming 'run' misses 'running'"
        );
    }

    #[test]
    fn stopwords_are_dropped_but_never_empty_the_query() {
        let corpus = "The\nThe the the.\n---\nOther\nRefund issued.";
        // "the" alone is a stop word, but dropping it would leave nothing.
        let out = search("the", corpus, opts()).unwrap();
        assert_eq!(out.total_matches, 1);
        assert_eq!(out.terms, vec!["the".to_string()]);
        // With a real term alongside it, the stop word is dropped.
        let out2 = search("the refund", corpus, opts()).unwrap();
        assert_eq!(out2.terms, vec!["refund".to_string()]);
    }

    #[test]
    fn phrase_search_requires_adjacency() {
        let corpus = "A\nworking days are counted here.\n---\nB\ndays of working alone.";
        let out = search("\"working days\"", corpus, opts()).unwrap();
        assert_eq!(out.total_matches, 1, "only the adjacent occurrence matches");
        assert_eq!(out.hits[0].doc, 1);
        assert_eq!(
            out.phrases,
            vec!["work dai".to_string()],
            "phrase tokens are stemmed too"
        );
    }

    #[test]
    fn exclusion_drops_documents() {
        let out = search("policy -shipping", CORPUS, opts()).unwrap();
        assert_eq!(out.total_matches, 1);
        assert_eq!(out.hits[0].doc, 1);
        assert_eq!(out.excluded, vec!["ship".to_string()]);
    }

    #[test]
    fn match_all_requires_every_term() {
        let any = search("refund tracking", CORPUS, opts()).unwrap();
        assert_eq!(any.total_matches, 2, "OR matches both documents");
        let all = search(
            "refund tracking",
            CORPUS,
            Options {
                match_mode: MatchMode::All,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(all.total_matches, 0, "AND: no document has both");
    }

    #[test]
    fn prefix_matching_is_opt_in() {
        let corpus = "Bikes\nA motorcycle is parked outside.\n---\nCars\nNothing here.";
        let off = search(
            "motor",
            corpus,
            Options {
                stemming: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(off.total_matches, 0, "prefix is off by default");
        let on = search(
            "motor",
            corpus,
            Options {
                prefix: true,
                stemming: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(on.total_matches, 1);
        assert!(on.hits[0].snippet.contains("\u{00AB}motorcycle\u{00BB}"));
    }

    #[test]
    fn title_boost_lifts_a_title_match() {
        let corpus = "Refund\nNothing else at all here in this line.\n---\nOther\nRefund appears only in the body text of this document.";
        let boosted = search(
            "refund",
            corpus,
            Options {
                title_boost: 5.0,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(boosted.hits[0].doc, 1, "title match wins with a high boost");
        let flat = search(
            "refund",
            corpus,
            Options {
                title_boost: 1.0,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(flat.hits[0].doc, 1, "shorter doc still wins on length norm");
        assert!(
            boosted.hits[0].score > flat.hits[0].score,
            "boost raises the title hit's score"
        );
    }

    #[test]
    fn separators_split_the_corpus() {
        let blank = "Alpha doc\n\nBeta doc\n\nGamma doc";
        let out = search(
            "doc",
            blank,
            Options {
                separator: Separator::BlankLine,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out.documents, 3);
        let ff = "Alpha doc\u{000C}Beta doc";
        let out2 = search(
            "doc",
            ff,
            Options {
                separator: Separator::FormFeed,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out2.documents, 2);
    }

    #[test]
    fn tfidf_is_an_alternative_scorer() {
        let bm25 = search("refund", CORPUS, opts()).unwrap();
        let tfidf = search(
            "refund",
            CORPUS,
            Options {
                algorithm: Algorithm::TfIdf,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(tfidf.algorithm, "TF-IDF");
        assert_eq!(tfidf.hits[0].doc, bm25.hits[0].doc);
        assert_ne!(tfidf.hits[0].score, bm25.hits[0].score);
    }

    #[test]
    fn max_results_caps_and_reports_the_total() {
        let corpus = "a doc\n---\nb doc\n---\nc doc\n---\nd doc";
        let out = search(
            "doc",
            corpus,
            Options {
                max_results: 2,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out.total_matches, 4);
        assert_eq!(out.count, 2);
        assert_eq!(out.hits.len(), 2);
    }

    #[test]
    fn snippet_words_zero_disables_snippets() {
        let out = search(
            "refund",
            CORPUS,
            Options {
                snippet_words: 0,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out.hits[0].snippet, "");
    }

    #[test]
    fn snippet_window_is_capped_and_elided() {
        let body: String = (1..=200).map(|i| format!("word{i} ")).collect();
        let corpus = format!("Long doc\n{body} needle {body}");
        let out = search(
            "needle",
            &corpus,
            Options {
                snippet_words: 6,
                ..opts()
            },
        )
        .unwrap();
        let s = &out.hits[0].snippet;
        assert!(
            s.starts_with('\u{2026}') && s.ends_with('\u{2026}'),
            "elided: {s}"
        );
        assert!(s.contains("\u{00AB}needle\u{00BB}"));
        assert_eq!(s.split_whitespace().count(), 6, "window is 6 words: {s}");
    }

    #[test]
    fn options_are_clamped_not_rejected() {
        let out = search(
            "refund",
            CORPUS,
            Options {
                max_results: 9_999,
                snippet_words: 9_999,
                k1: 99.0,
                b: -4.0,
                title_boost: 99.0,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out.count, 1);
        assert!(out.hits[0].score.is_finite());
    }

    #[test]
    fn json_output_is_structured() {
        let json = search_text(
            "refund",
            CORPUS,
            Options {
                output: OutputFormat::Json,
                ..opts()
            },
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["algorithm"], "BM25");
        assert_eq!(v["documents"], 3);
        assert_eq!(v["hits"][0]["doc"], 1);
        assert_eq!(v["hits"][0]["title"], "Refund policy");
    }

    #[test]
    fn render_exact_text_format() {
        let corpus = "Refund policy\nRefunds take five days.\n---\nShipping\nOrders ship fast.";
        let text = search_text(
            "refund",
            corpus,
            Options {
                snippet_words: 8,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(
            text,
            "BM25 · 1 of 2 documents match \"refund\"\n\n#1  doc 1  score 1.04  Refund policy\n    \u{00AB}Refund\u{00BB} policy \u{00AB}Refunds\u{00BB} take five days"
        );
    }

    #[test]
    fn render_no_matches_message() {
        let out = search("zebra", CORPUS, opts()).unwrap();
        assert_eq!(
            render(&out),
            "No documents match \"zebra\" (searched 3 documents)."
        );
    }

    // --- error paths -------------------------------------------------------

    #[test]
    fn empty_corpus_errors() {
        let err = search("refund", "   \n  ", opts()).unwrap_err();
        assert!(err.contains("corpus is empty"), "got: {err}");
    }

    #[test]
    fn empty_query_errors() {
        let err = search("   ", CORPUS, opts()).unwrap_err();
        assert!(err.contains("query is empty"), "got: {err}");
    }

    #[test]
    fn exclusion_only_query_errors() {
        let err = search("-refund", CORPUS, opts()).unwrap_err();
        assert!(err.contains("query is empty"), "got: {err}");
    }

    #[test]
    fn unknown_enum_values_error() {
        assert!(Separator::parse("commas")
            .unwrap_err()
            .contains("unknown separator"));
        assert!(Algorithm::parse("bm42")
            .unwrap_err()
            .contains("unknown algorithm"));
        assert!(MatchMode::parse("either")
            .unwrap_err()
            .contains("unknown match"));
        assert!(OutputFormat::parse("yaml")
            .unwrap_err()
            .contains("unknown output"));
    }

    // --- stemmer -----------------------------------------------------------

    #[test]
    fn porter_stemmer_handles_the_classic_cases() {
        for (word, want) in [
            ("caresses", "caress"),
            ("ponies", "poni"),
            ("cats", "cat"),
            ("feed", "feed"),
            ("agreed", "agre"),
            ("plastered", "plaster"),
            ("motoring", "motor"),
            ("hopping", "hop"),
            ("happy", "happi"),
            ("relational", "relat"),
            ("conditional", "condit"),
            ("triplicate", "triplic"),
            ("hopefulness", "hope"),
            ("formality", "formal"),
            ("allowance", "allow"),
            ("adoption", "adopt"),
            ("controlling", "control"),
            ("running", "run"),
            ("policies", "polici"),
        ] {
            assert_eq!(stem(word), want, "stem({word})");
        }
        // Short and non-ASCII words pass through untouched.
        assert_eq!(stem("is"), "is");
        assert_eq!(stem("café"), "café");
    }
}
