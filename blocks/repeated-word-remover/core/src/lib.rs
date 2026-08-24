//! repeated-word-remover core — find and delete accidentally doubled words
//! (`the the`, `is is`, `on on on`) while leaving legitimate English repeats
//! such as `had had` and `that that` alone.
//!
//! Pure Rust, dependency-free apart from `serde`. Shared by the chat skill
//! block, the CLI and the web page.
//!
//! The scan is deliberately **adjacent only**: a word that reappears later in
//! the sentence is never touched (that is line/list deduplication, a different
//! job). Within a run of repeats the FIRST occurrence always wins, so its
//! capitalisation, indentation and surrounding punctuation survive.

use serde::Serialize;

/// Hard cap on input size (bytes).
pub const MAX_INPUT_BYTES: usize = 200_000;

/// Words that are legitimately doubled in English, protected out of the box.
/// "He had had enough", "the fact that that happened", "what it is is simple",
/// "a long long time", "ha ha", "bye bye", "chop chop"…
pub const DEFAULT_KEEP_WORDS: &[&str] = &[
    "had", "that", "is", "do", "no", "very", "long", "many", "far", "ha", "blah", "bye", "night",
    "so", "chop", "tut", "yum",
];

/// Punctuation that may sit between two repeats and still count as a repeat
/// when `ignore_punctuation` is on. Sentence-enders (`.`/`!`/`?`) and dashes
/// are deliberately absent — they separate two deliberate uses of a word.
const SOFT_PUNCTUATION: &[char] = &[
    ',', ';', ':', '(', ')', '[', ']', '{', '}', '"', '\'', '\u{2018}', '\u{2019}', '\u{201C}',
    '\u{201D}', '\u{00AB}', '\u{00BB}', '/', '|',
];

/// Which rendering of the result `output` carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// The text with every accidental repeat deleted (default).
    Clean,
    /// The ORIGINAL text with each deleted occurrence wrapped in markdown
    /// strikethrough (`~~the~~`), so the change is reviewable before applying.
    Marked,
    /// A human-readable audit: counts plus one `line, col` entry per repeat.
    Report,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "clean" | "" => Ok(OutputFormat::Clean),
            "marked" => Ok(OutputFormat::Marked),
            "report" => Ok(OutputFormat::Report),
            other => Err(format!(
                "output must be \"clean\", \"marked\" or \"report\", got \"{other}\""
            )),
        }
    }
}

/// How repeats are detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Compare the two words case-sensitively. Off by default, so `The the`
    /// counts as a repeat.
    pub case_sensitive: bool,
    /// Treat a repeat split by a single line break (`… the\nthe …`) as a
    /// repeat. On by default — it is the commonest real typo shape. A blank
    /// line (paragraph break) never bridges a repeat.
    pub across_line_breaks: bool,
    /// Let commas, semicolons, colons, brackets and quotes sit between the two
    /// words (`word, word`). Sentence-enders and dashes never bridge.
    pub ignore_punctuation: bool,
    /// Also collapse repeated tokens that contain no letters (`2024 2024`).
    pub include_numbers: bool,
    /// Ignore repeats of words shorter than this many characters (1 = no floor).
    pub min_length: usize,
    /// Words that are never removed, compared case-insensitively.
    pub keep_words: Vec<String>,
    /// Which rendering `Analysis::output` carries.
    pub format: OutputFormat,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            case_sensitive: false,
            across_line_breaks: true,
            ignore_punctuation: false,
            include_numbers: false,
            min_length: 1,
            keep_words: DEFAULT_KEEP_WORDS.iter().map(|w| w.to_string()).collect(),
            format: OutputFormat::Clean,
        }
    }
}

/// One doubled-word spot found in the input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// 1-based line of the first occurrence.
    pub line: usize,
    /// 1-based column (in characters) of the first occurrence.
    pub column: usize,
    /// The word as it appears at the first occurrence.
    pub word: String,
    /// How many times in a row it appears (always ≥ 2).
    pub occurrences: usize,
    /// How many copies were deleted (`occurrences - 1`).
    pub removed: usize,
}

/// The full result: the rendered `output` plus the structured audit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Analysis {
    /// The result rendered per `Options::format`.
    pub output: String,
    /// Words in the input.
    pub total_words: usize,
    /// Words left after the repeats were deleted.
    pub kept_words: usize,
    /// Word copies deleted.
    pub removed_words: usize,
    /// Doubled-word spots found (a run of three counts as one spot).
    pub repeats_found: usize,
    /// `removed_words / total_words`, as a percentage rounded to 1 decimal.
    pub reduction_percent: f64,
    /// One entry per spot, in document order.
    pub findings: Vec<Finding>,
}

/// The default keep list rendered as the comma-separated spec the CLI, chat
/// schema and page field all show. Single source: every surface's default value
/// comes from here, so the descriptor default and `Options::default()` agree.
pub fn default_keep_words() -> String {
    DEFAULT_KEEP_WORDS.join(", ")
}

/// Parse a user-supplied keep list. Accepts commas, semicolons, newlines or
/// plain spaces as separators; entries are lower-cased and de-duplicated.
pub fn parse_keep_words(spec: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in spec.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
        let w = raw.trim().to_lowercase();
        if w.is_empty() || out.contains(&w) {
            continue;
        }
        out.push(w);
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '\u{2019}' || c == '-'
}

fn is_edge_char(c: char) -> bool {
    c == '\'' || c == '\u{2019}' || c == '-'
}

/// Byte spans of every word token in `text`, in order.
fn tokenize(text: &str) -> Vec<(usize, usize)> {
    let mut toks = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if is_word_char(c) {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            push_token(text, s, i, &mut toks);
        }
    }
    if let Some(s) = start {
        push_token(text, s, text.len(), &mut toks);
    }
    toks
}

/// Trim leading/trailing apostrophes and hyphens off a raw span, dropping it if
/// nothing is left (`--`, `'''`).
fn push_token(text: &str, start: usize, end: usize, out: &mut Vec<(usize, usize)>) {
    let raw = &text[start..end];
    let trimmed = raw.trim_matches(is_edge_char);
    if trimmed.is_empty() {
        return;
    }
    let offset = raw.find(trimmed).unwrap_or(0);
    out.push((start + offset, start + offset + trimmed.len()));
}

/// The comparison key for a token: curly apostrophes folded to straight ones,
/// lower-cased unless the caller asked for case-sensitivity.
fn key(word: &str, case_sensitive: bool) -> String {
    let folded: String = word
        .chars()
        .map(|c| if c == '\u{2019}' { '\'' } else { c })
        .collect();
    if case_sensitive {
        folded
    } else {
        folded.to_lowercase()
    }
}

/// Does the text between two identical words let them count as a repeat?
fn bridges(gap: &str, opts: &Options) -> bool {
    let newlines = gap.matches('\n').count();
    if newlines > 1 {
        // A blank line is a paragraph break, never a typo.
        return false;
    }
    if newlines == 1 && !opts.across_line_breaks {
        return false;
    }
    if gap.chars().all(char::is_whitespace) {
        return !gap.is_empty();
    }
    if !opts.ignore_punctuation {
        return false;
    }
    gap.chars()
        .all(|c| c.is_whitespace() || SOFT_PUNCTUATION.contains(&c))
}

/// Is this word eligible to have its repeats deleted?
fn removable(word: &str, opts: &Options) -> bool {
    if !opts.include_numbers && !word.chars().any(char::is_alphabetic) {
        return false;
    }
    if word.chars().count() < opts.min_length {
        return false;
    }
    let k = word.to_lowercase();
    let k = k.replace('\u{2019}', "'");
    !opts.keep_words.iter().any(|w| *w == k)
}

/// 1-based (line, column-in-characters) of a byte offset.
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let col = match before.rfind('\n') {
        Some(i) => before[i + 1..].chars().count() + 1,
        None => before.chars().count() + 1,
    };
    (line, col)
}

/// Collapse whitespace runs so a multi-line repeat prints on one report line.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Find and remove accidental doubled words.
pub fn analyze(text: &str, opts: &Options) -> Result<Analysis, String> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {MAX_INPUT_BYTES} bytes — split the text and run again",
            text.len()
        ));
    }
    if opts.min_length < 1 || opts.min_length > 20 {
        return Err(format!(
            "min_length must be between 1 and 20, got {}",
            opts.min_length
        ));
    }

    let toks = tokenize(text);
    let mut findings: Vec<Finding> = Vec::new();
    // Byte spans deleted from the CLEAN output: end-of-first-word → end-of-run.
    let mut cut: Vec<(usize, usize)> = Vec::new();
    // Byte spans of the individual deleted words, for the MARKED view.
    let mut struck: Vec<(usize, usize)> = Vec::new();
    let mut removed_words = 0usize;

    let mut i = 0usize;
    while i < toks.len() {
        let first = &text[toks[i].0..toks[i].1];
        let k = key(first, opts.case_sensitive);
        let mut j = i + 1;
        while j < toks.len()
            && bridges(&text[toks[j - 1].1..toks[j].0], opts)
            && key(&text[toks[j].0..toks[j].1], opts.case_sensitive) == k
        {
            j += 1;
        }
        let run = j - i;
        if run >= 2 && removable(first, opts) {
            let (line, column) = line_col(text, toks[i].0);
            findings.push(Finding {
                line,
                column,
                word: first.to_string(),
                occurrences: run,
                removed: run - 1,
            });
            removed_words += run - 1;
            cut.push((toks[i].1, toks[j - 1].1));
            for t in &toks[i + 1..j] {
                struck.push(*t);
            }
        }
        i = if run >= 2 { j } else { i + 1 };
    }

    let total_words = toks.len();
    let kept_words = total_words - removed_words;
    let reduction_percent = if total_words == 0 {
        0.0
    } else {
        ((removed_words as f64 / total_words as f64) * 1000.0).round() / 10.0
    };

    let output = match opts.format {
        OutputFormat::Clean => splice_out(text, &cut),
        OutputFormat::Marked => wrap_spans(text, &struck),
        OutputFormat::Report => render_report(
            text,
            &findings,
            total_words,
            kept_words,
            removed_words,
            reduction_percent,
        ),
    };

    Ok(Analysis {
        output,
        total_words,
        kept_words,
        removed_words,
        repeats_found: findings.len(),
        reduction_percent,
        findings,
    })
}

/// Copy `text` while skipping every `cut` span (spans are in order, disjoint).
fn splice_out(text: &str, cut: &[(usize, usize)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0usize;
    for &(s, e) in cut {
        out.push_str(&text[at..s]);
        at = e;
    }
    out.push_str(&text[at..]);
    out
}

/// Copy `text`, wrapping every span in markdown strikethrough.
fn wrap_spans(text: &str, spans: &[(usize, usize)]) -> String {
    let mut out = String::with_capacity(text.len() + spans.len() * 4);
    let mut at = 0usize;
    for &(s, e) in spans {
        out.push_str(&text[at..s]);
        out.push_str("~~");
        out.push_str(&text[s..e]);
        out.push_str("~~");
        at = e;
    }
    out.push_str(&text[at..]);
    out
}

fn render_report(
    text: &str,
    findings: &[Finding],
    total: usize,
    kept: usize,
    removed: usize,
    pct: f64,
) -> String {
    if findings.is_empty() {
        return format!("No repeated words found — {total} words checked.");
    }
    let spots = if findings.len() == 1 { "spot" } else { "spots" };
    let copies = if removed == 1 { "copy" } else { "copies" };
    let mut out = format!(
        "Found {} doubled-word {spots}; removed {removed} word {copies}.\nWords: {total} in \u{2192} {kept} out ({pct}% shorter).\n",
        findings.len()
    );
    for f in findings {
        let run = one_line(&repeat_snippet(text, f));
        out.push_str(&format!(
            "\nline {}, col {}: \"{}\" \u{2192} \"{}\"",
            f.line, f.column, run, f.word
        ));
    }
    out
}

/// Re-render a finding as `word word …` for the report (the original spacing is
/// collapsed by `one_line`, so rebuilding from the word is faithful enough and
/// avoids a second byte-span bookkeeping pass).
fn repeat_snippet(_text: &str, f: &Finding) -> String {
    vec![f.word.as_str(); f.occurrences].join(" ")
}

/// Convenience for the web wrapper: just the rendered `output` string.
pub fn render(text: &str, opts: &Options) -> Result<String, String> {
    analyze(text, opts).map(|a| a.output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn removes_a_simple_doubled_word() {
        let a = analyze("I think the the cat sat down.", &opts()).unwrap();
        assert_eq!(a.output, "I think the cat sat down.");
        assert_eq!(a.removed_words, 1);
        assert_eq!(a.repeats_found, 1);
        assert_eq!(a.total_words, 7);
        assert_eq!(a.kept_words, 6);
        assert_eq!(a.findings[0].word, "the");
        assert_eq!(a.findings[0].line, 1);
        assert_eq!(a.findings[0].column, 9);
        assert_eq!(a.findings[0].occurrences, 2);
    }

    #[test]
    fn errors_when_the_input_exceeds_the_cap() {
        let big = "a".repeat(MAX_INPUT_BYTES + 1);
        let err = analyze(&big, &opts()).unwrap_err();
        assert!(err.contains("200000 bytes"), "{err}");
        // exactly at the cap is fine
        assert!(analyze(&"a".repeat(MAX_INPUT_BYTES), &opts()).is_ok());
    }

    #[test]
    fn errors_on_an_out_of_range_min_length() {
        let mut o = opts();
        o.min_length = 0;
        assert!(analyze("the the", &o).unwrap_err().contains("min_length"));
        o.min_length = 21;
        assert!(analyze("the the", &o).unwrap_err().contains("min_length"));
    }

    #[test]
    fn errors_on_an_unknown_output_format() {
        assert!(OutputFormat::parse("striked").is_err());
        assert_eq!(OutputFormat::parse("").unwrap(), OutputFormat::Clean);
        assert_eq!(OutputFormat::parse(" Report ").unwrap(), OutputFormat::Report);
    }

    #[test]
    fn collapses_a_run_of_three_or_more() {
        let a = analyze("it depends on on on on the country", &opts()).unwrap();
        assert_eq!(a.output, "it depends on the country");
        assert_eq!(a.removed_words, 3);
        assert_eq!(a.repeats_found, 1);
        assert_eq!(a.findings[0].occurrences, 4);
        assert_eq!(a.findings[0].removed, 3);
    }

    #[test]
    fn keeps_legitimate_english_repeats_by_default() {
        for s in [
            "He had had enough.",
            "The fact that that happened.",
            "What it is is simple.",
            "a long long time ago",
            "ha ha, very funny",
        ] {
            let a = analyze(s, &opts()).unwrap();
            assert_eq!(a.output, s, "should not touch {s:?}");
            assert_eq!(a.removed_words, 0);
        }
    }

    #[test]
    fn an_empty_keep_list_removes_even_legitimate_repeats() {
        let mut o = opts();
        o.keep_words = parse_keep_words("");
        assert_eq!(analyze("He had had enough.", &o).unwrap().output, "He had enough.");
    }

    #[test]
    fn case_insensitive_by_default_and_the_first_spelling_wins() {
        let a = analyze("The the cat", &opts()).unwrap();
        assert_eq!(a.output, "The cat");
        let mut o = opts();
        o.case_sensitive = true;
        assert_eq!(analyze("The the cat", &o).unwrap().output, "The the cat");
        assert_eq!(analyze("the the cat", &o).unwrap().output, "the cat");
    }

    #[test]
    fn bridges_a_single_line_break_but_not_a_paragraph_break() {
        let a = analyze("wrapped at the\nthe margin", &opts()).unwrap();
        assert_eq!(a.output, "wrapped at the margin");

        let mut o = opts();
        o.across_line_breaks = false;
        assert_eq!(
            analyze("wrapped at the\nthe margin", &o).unwrap().output,
            "wrapped at the\nthe margin"
        );

        // blank line = paragraph break, never bridged
        assert_eq!(
            analyze("ends with the\n\nthe next para", &opts()).unwrap().output,
            "ends with the\n\nthe next para"
        );
    }

    #[test]
    fn punctuation_blocks_a_repeat_unless_asked_otherwise() {
        assert_eq!(analyze("stop. Stop now", &opts()).unwrap().output, "stop. Stop now");
        assert_eq!(analyze("well, well now", &opts()).unwrap().output, "well, well now");

        let mut o = opts();
        o.ignore_punctuation = true;
        assert_eq!(analyze("well, well now", &o).unwrap().output, "well now");
        // sentence-enders still never bridge
        assert_eq!(analyze("stop. Stop now", &o).unwrap().output, "stop. Stop now");
        // nor do dashes
        assert_eq!(analyze("wait — wait here", &o).unwrap().output, "wait — wait here");
    }

    #[test]
    fn numbers_are_left_alone_unless_included() {
        assert_eq!(analyze("2024 2024 totals", &opts()).unwrap().output, "2024 2024 totals");
        let mut o = opts();
        o.include_numbers = true;
        assert_eq!(analyze("2024 2024 totals", &o).unwrap().output, "2024 totals");
    }

    #[test]
    fn min_length_sets_a_floor() {
        let mut o = opts();
        o.min_length = 3;
        assert_eq!(analyze("I I saw the the sign", &o).unwrap().output, "I I saw the sign");
        o.min_length = 1;
        assert_eq!(analyze("I I saw the the sign", &o).unwrap().output, "I saw the sign");
    }

    #[test]
    fn handles_hyphenated_and_apostrophised_words() {
        assert_eq!(
            analyze("a well-known well-known author", &opts()).unwrap().output,
            "a well-known author"
        );
        assert_eq!(
            analyze("don't don\u{2019}t worry", &opts()).unwrap().output,
            "don't worry"
        );
    }

    #[test]
    fn marked_view_strikes_the_deleted_copy_only() {
        let mut o = opts();
        o.format = OutputFormat::Marked;
        let a = analyze("I think the the cat sat.", &o).unwrap();
        assert_eq!(a.output, "I think the ~~the~~ cat sat.");
        assert_eq!(a.removed_words, 1);
    }

    #[test]
    fn report_view_lists_every_spot() {
        let mut o = opts();
        o.format = OutputFormat::Report;
        let a = analyze("the the cat\nsat on on the mat", &o).unwrap();
        assert!(a.output.starts_with("Found 2 doubled-word spots; removed 2 word copies."));
        assert!(a.output.contains("Words: 8 in \u{2192} 6 out (25% shorter)."));
        assert!(a.output.contains("line 1, col 1: \"the the\" \u{2192} \"the\""));
        assert!(a.output.contains("line 2, col 5: \"on on\" \u{2192} \"on\""));

        let clean = analyze("nothing doubled here", &o).unwrap();
        assert_eq!(clean.output, "No repeated words found — 3 words checked.");
    }

    #[test]
    fn preserves_indentation_and_trailing_text() {
        let a = analyze("    - the the item\n    - second", &opts()).unwrap();
        assert_eq!(a.output, "    - the the item\n    - second".replace("the the", "the"));
    }

    #[test]
    fn empty_input_is_a_no_op() {
        let a = analyze("", &opts()).unwrap();
        assert_eq!(a.output, "");
        assert_eq!(a.total_words, 0);
        assert_eq!(a.reduction_percent, 0.0);
    }

    #[test]
    fn the_default_keep_spec_round_trips_to_the_default_options() {
        assert_eq!(parse_keep_words(&default_keep_words()), opts().keep_words);
        assert!(default_keep_words().starts_with("had, that, is,"));
    }

    #[test]
    fn parse_keep_words_accepts_mixed_separators() {
        assert_eq!(
            parse_keep_words("had, that;IS  had\nno"),
            vec!["had", "that", "is", "no"]
        );
        assert!(parse_keep_words("  ,, ").is_empty());
    }

    #[test]
    fn separate_runs_after_a_blocked_gap_are_still_found() {
        let a = analyze("the. the the cat", &opts()).unwrap();
        assert_eq!(a.output, "the. the cat");
        assert_eq!(a.repeats_found, 1);
    }
}
