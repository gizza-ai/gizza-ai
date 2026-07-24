//! text-corpus-cleaner core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Cleans a raw, line-oriented text corpus (one item/record per line) in one
//! deterministic pass, in this fixed order:
//!   1. **split** — normalize `\r\n` / `\r` line endings to `\n`, split on `\n`.
//!   2. **transform** each line: Unicode form (NFC/NFKC) -> whitespace
//!      (trim / collapse interior runs) -> lowercase.
//!   3. **filter** each line: drop lines shorter than `min_chars`, with fewer
//!      than `min_words`, whose symbol ratio exceeds `max_symbol_ratio`, or
//!      (when a target language is set) whose detected language differs.
//!   4. **deduplicate** non-blank lines (exact or normalized), keeping the
//!      first occurrence.
//!   5. **blank-line handling** — optionally collapse runs of blank lines to a
//!      single blank and trim leading/trailing blanks.
//!
//! Language detection is trigram/script based (the `whatlang` crate) — no model
//! files, no network. Blank lines are structural: they are never language- or
//! symbol-filtered, and never deduplicated (paragraph breaks are preserved).

use unicode_normalization::UnicodeNormalization;
use whatlang::Lang;

/// Unicode normalization form applied to each line before other transforms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    /// Leave code points untouched.
    None,
    /// Canonical composition (NFC) — the web-recommended default.
    Nfc,
    /// Compatibility composition (NFKC) — also folds ligatures (ﬁ -> fi),
    /// full-width and circled/styled letters, and fractions to plain forms.
    Nfkc,
}

impl Form {
    /// Parse a form name (case-insensitive; blank -> `Nfc`). Unknown -> `Err`.
    pub fn parse(s: &str) -> Result<Form, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "nfc" => Ok(Form::Nfc),
            "none" | "off" => Ok(Form::None),
            "nfkc" => Ok(Form::Nfkc),
            other => Err(format!(
                "invalid unicode_form {other:?}: expected one of none, nfc, nfkc"
            )),
        }
    }

    fn apply(self, line: &str) -> String {
        match self {
            Form::None => line.to_string(),
            Form::Nfc => line.nfc().collect(),
            Form::Nfkc => line.nfkc().collect(),
        }
    }
}

/// How to normalize whitespace within each line.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Whitespace {
    /// Leave interior and edge whitespace untouched.
    Keep,
    /// Trim leading/trailing whitespace; keep interior spacing.
    Trim,
    /// Collapse every interior run of whitespace to a single ASCII space and
    /// trim the ends.
    Collapse,
}

impl Whitespace {
    /// Parse a whitespace mode (case-insensitive; blank -> `Trim`). Unknown -> `Err`.
    pub fn parse(s: &str) -> Result<Whitespace, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "trim" => Ok(Whitespace::Trim),
            "keep" | "none" => Ok(Whitespace::Keep),
            "collapse" => Ok(Whitespace::Collapse),
            other => Err(format!(
                "invalid whitespace {other:?}: expected one of keep, trim, collapse"
            )),
        }
    }

    fn apply(self, line: &str) -> String {
        match self {
            Whitespace::Keep => line.to_string(),
            Whitespace::Trim => line.trim().to_string(),
            Whitespace::Collapse => line.split_whitespace().collect::<Vec<_>>().join(" "),
        }
    }
}

/// Line-deduplication mode.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dedupe {
    /// Keep every line.
    None,
    /// Drop lines byte-for-byte identical to an earlier line.
    Exact,
    /// Drop lines that match an earlier line after case-folding and collapsing
    /// whitespace (so "Hello   World" and "hello world" are duplicates).
    Normalized,
}

impl Dedupe {
    /// Parse a dedupe mode (case-insensitive; blank -> `Exact`). Unknown -> `Err`.
    pub fn parse(s: &str) -> Result<Dedupe, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "exact" => Ok(Dedupe::Exact),
            "none" | "off" | "keep" => Ok(Dedupe::None),
            "normalized" | "normalize" | "fuzzy" => Ok(Dedupe::Normalized),
            other => Err(format!(
                "invalid dedupe {other:?}: expected one of none, exact, normalized"
            )),
        }
    }

    /// Dedup key for a (post-transform) line, or `None` for the `None` mode.
    fn key(self, line: &str) -> Option<String> {
        match self {
            Dedupe::None => None,
            Dedupe::Exact => Some(line.to_string()),
            Dedupe::Normalized => {
                Some(line.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase())
            }
        }
    }
}

/// Optional per-line language filter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Language(Option<Lang>);

impl Language {
    /// The language codes (ISO 639-3) offered by this tool, in display order,
    /// with `any` (no filter) last.
    pub const CODES: [&'static str; 14] = [
        "eng", "spa", "fra", "deu", "ita", "por", "nld", "rus", "ara", "cmn", "jpn", "kor", "hin",
        "any",
    ];

    /// Parse a language code (case-insensitive; blank/`any` -> no filter).
    /// Unknown or unsupported codes -> `Err`.
    pub fn parse(s: &str) -> Result<Language, String> {
        let t = s.trim().to_ascii_lowercase();
        if t.is_empty() || t == "any" {
            return Ok(Language(None));
        }
        // "cmn" (whatlang) is commonly written "zho"/"chi"/"zh" — accept aliases.
        let code = if t == "zho" || t == "chi" || t == "zh" { "cmn" } else { t.as_str() };
        match Lang::from_code(code) {
            Some(l) if Self::CODES.contains(&code) => Ok(Language(Some(l))),
            _ => Err(format!(
                "invalid language {s:?}: expected one of {} or any",
                Self::CODES
                    .iter()
                    .filter(|c| **c != "any")
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    /// Whether a non-blank line passes the filter. A line whose language cannot
    /// be detected (too short / ambiguous) is KEPT, so short valid records are
    /// not silently dropped.
    fn keeps(self, line: &str) -> bool {
        match self.0 {
            None => true,
            Some(target) => match whatlang::detect_lang(line) {
                Some(detected) => detected == target,
                None => true,
            },
        }
    }
}

/// All corpus-cleaning options. Build with [`Options::default`] then override.
#[derive(Clone, Copy)]
pub struct Options {
    pub unicode_form: Form,
    pub whitespace: Whitespace,
    pub lowercase: bool,
    pub dedupe: Dedupe,
    pub language: Language,
    /// Drop lines with fewer than this many characters (0 = keep all).
    pub min_chars: usize,
    /// Drop lines with fewer than this many whitespace-separated words (0 = keep all).
    pub min_words: usize,
    /// Drop lines whose ratio of symbol chars (non-alphanumeric, non-space) to
    /// non-space chars exceeds this (0.0–1.0; 1.0 = keep all).
    pub max_symbol_ratio: f64,
    /// Collapse runs of blank lines to a single blank and trim leading/trailing
    /// blank lines.
    pub collapse_blank: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            unicode_form: Form::Nfc,
            whitespace: Whitespace::Trim,
            lowercase: false,
            dedupe: Dedupe::Exact,
            language: Language(None),
            min_chars: 0,
            min_words: 0,
            max_symbol_ratio: 1.0,
            collapse_blank: true,
        }
    }
}

fn is_blank(line: &str) -> bool {
    line.chars().all(char::is_whitespace)
}

/// Ratio of symbol characters (not alphanumeric, not whitespace) to non-space
/// characters. A line with no visible characters returns 0.0.
fn symbol_ratio(line: &str) -> f64 {
    let mut visible = 0usize;
    let mut symbols = 0usize;
    for c in line.chars() {
        if c.is_whitespace() {
            continue;
        }
        visible += 1;
        if !c.is_alphanumeric() {
            symbols += 1;
        }
    }
    if visible == 0 {
        0.0
    } else {
        symbols as f64 / visible as f64
    }
}

/// Clean a raw text corpus per `opts`. Returns the cleaned text (lines joined
/// with `\n`, no trailing newline).
pub fn clean(input: &str, opts: Options) -> String {
    // 1. Split on normalized line endings.
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");

    // 2 + 3. Transform then filter each line.
    let mut kept: Vec<String> = Vec::new();
    for raw in normalized.split('\n') {
        let mut line = opts.unicode_form.apply(raw);
        line = opts.whitespace.apply(&line);
        if opts.lowercase {
            line = line.to_lowercase();
        }

        if is_blank(&line) {
            // Blank lines skip content filters; min_chars/min_words still apply
            // (a blank line has 0 chars / 0 words), which lets a positive
            // threshold drop them while 0 keeps them for blank-collapsing.
            if opts.min_chars == 0 && opts.min_words == 0 {
                kept.push(line);
            }
            continue;
        }

        if line.chars().count() < opts.min_chars {
            continue;
        }
        if line.split_whitespace().count() < opts.min_words {
            continue;
        }
        if symbol_ratio(&line) > opts.max_symbol_ratio {
            continue;
        }
        if !opts.language.keeps(&line) {
            continue;
        }
        kept.push(line);
    }

    // 4. Deduplicate non-blank lines (blank lines are never deduplicated).
    if opts.dedupe != Dedupe::None {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        kept.retain(|line| {
            if is_blank(line) {
                return true;
            }
            match opts.dedupe.key(line) {
                Some(k) => seen.insert(k),
                None => true,
            }
        });
    }

    // 5. Blank-line handling.
    if opts.collapse_blank {
        let mut out: Vec<String> = Vec::with_capacity(kept.len());
        let mut prev_blank = true; // seeded true to trim leading blanks
        for line in kept {
            let blank = is_blank(&line);
            if blank && prev_blank {
                continue;
            }
            prev_blank = blank;
            out.push(line);
        }
        while out.last().map(|l| is_blank(l)).unwrap_or(false) {
            out.pop();
        }
        kept = out;
    }

    kept.join("\n")
}

/// String-in / string-out entry point shared by the chat block, the CLI, and the
/// web page. Parses the option names (case-insensitive), validates numeric
/// ranges, then runs [`clean`]. Returns the cleaned corpus, or an `Err` message
/// naming what was expected on any invalid option.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    unicode_form: &str,
    whitespace: &str,
    lowercase: bool,
    dedupe: &str,
    language: &str,
    min_chars: u32,
    min_words: u32,
    max_symbol_ratio: f64,
    collapse_blank: bool,
) -> Result<String, String> {
    if !max_symbol_ratio.is_finite() || !(0.0..=1.0).contains(&max_symbol_ratio) {
        return Err(format!(
            "invalid max_symbol_ratio {max_symbol_ratio}: expected a number from 0.0 to 1.0"
        ));
    }
    let opts = Options {
        unicode_form: Form::parse(unicode_form)?,
        whitespace: Whitespace::parse(whitespace)?,
        lowercase,
        dedupe: Dedupe::parse(dedupe)?,
        language: Language::parse(language)?,
        min_chars: min_chars as usize,
        min_words: min_words as usize,
        max_symbol_ratio,
        collapse_blank,
    };
    Ok(clean(input, opts))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn dedupes_exact_lines_keeping_first() {
        let out = clean("apple\nbanana\napple\ncherry\nbanana\n", opts());
        assert_eq!(out, "apple\nbanana\ncherry");
    }

    #[test]
    fn trims_and_normalized_dedupe_folds_case_and_spacing() {
        let mut o = opts();
        o.dedupe = Dedupe::Normalized;
        // Normalized dedupe folds case + interior spacing for the COMPARISON key,
        // but the kept line is the first occurrence verbatim (after trim).
        let out = clean("Hello   World\nhello world\n  HELLO WORLD  \n", o);
        assert_eq!(out, "Hello   World");
    }

    #[test]
    fn collapse_whitespace_and_lowercase() {
        let mut o = opts();
        o.whitespace = Whitespace::Collapse;
        o.lowercase = true;
        let out = clean("  The   Quick\tBrown  \n", o);
        assert_eq!(out, "the quick brown");
    }

    #[test]
    fn min_chars_and_min_words_drop_short_lines() {
        let mut o = opts();
        o.min_chars = 5;
        o.min_words = 2;
        let out = clean("hi\nthis is fine\nok\na longer line here\n", o);
        assert_eq!(out, "this is fine\na longer line here");
    }

    #[test]
    fn symbol_ratio_drops_junk_lines() {
        let mut o = opts();
        o.max_symbol_ratio = 0.5;
        let out = clean("real sentence here\n===== >>>>> #####\nanother good one\n", o);
        assert_eq!(out, "real sentence here\nanother good one");
    }

    #[test]
    fn collapse_blank_lines_and_trim_edges() {
        let mut o = opts();
        o.collapse_blank = true;
        let out = clean("\n\nfirst\n\n\nsecond\n\n", o);
        assert_eq!(out, "first\n\nsecond");
    }

    #[test]
    fn keep_blank_lines_when_not_collapsing() {
        let mut o = opts();
        o.collapse_blank = false;
        o.dedupe = Dedupe::None;
        let out = clean("first\n\n\nsecond", o);
        assert_eq!(out, "first\n\n\nsecond");
    }

    #[test]
    fn nfkc_flattens_fancy_forms() {
        let mut o = opts();
        o.unicode_form = Form::Nfkc;
        let out = clean("ﬁle ＡＢＣ", o);
        assert_eq!(out, "file ABC");
    }

    #[test]
    fn language_filter_keeps_target_drops_others() {
        let mut o = opts();
        o.language = Language::parse("eng").unwrap();
        let out = clean(
            "The quick brown fox jumps over the lazy dog every morning.\n\
             Le renard brun rapide saute par-dessus le chien paresseux chaque matin.\n",
            o,
        );
        assert_eq!(out, "The quick brown fox jumps over the lazy dog every morning.");
    }

    #[test]
    fn form_parse_rejects_unknown() {
        let err = Form::parse("nfd").unwrap_err();
        assert!(err.contains("invalid unicode_form"), "{err}");
    }

    #[test]
    fn language_parse_rejects_unknown() {
        let err = Language::parse("xyz").unwrap_err();
        assert!(err.contains("invalid language"), "{err}");
    }

    #[test]
    fn language_parse_accepts_zh_alias() {
        assert_eq!(Language::parse("zh"), Language::parse("cmn"));
    }

    #[test]
    fn symbol_ratio_of_pure_symbols_is_one() {
        assert!((symbol_ratio("====") - 1.0).abs() < 1e-9);
        assert!((symbol_ratio("abcd") - 0.0).abs() < 1e-9);
    }

    #[test]
    fn run_cleans_with_named_options() {
        let out = run(
            "  The Quick  \nthe quick\nThe Quick\n",
            "nfc",
            "collapse",
            true,
            "exact",
            "any",
            0,
            0,
            1.0,
            true,
        )
        .unwrap();
        assert_eq!(out, "the quick");
    }

    #[test]
    fn run_rejects_bad_symbol_ratio() {
        let err = run("x", "nfc", "trim", false, "exact", "any", 0, 0, 1.5, true).unwrap_err();
        assert!(err.contains("invalid max_symbol_ratio"), "{err}");
    }

    #[test]
    fn run_rejects_bad_enum() {
        let err = run("x", "nfd", "trim", false, "exact", "any", 0, 0, 1.0, true).unwrap_err();
        assert!(err.contains("invalid unicode_form"), "{err}");
    }
}
