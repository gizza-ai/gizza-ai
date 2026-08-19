//! multilingual-stemmer core — pure compute, shared by the chat skill block and the web page.
//!
//! Reduces words to their Snowball stem in 18 languages (Arabic, Danish, Dutch,
//! English, Finnish, French, German, Greek, Hungarian, Italian, Norwegian,
//! Portuguese, Romanian, Russian, Spanish, Swedish, Tamil, Turkish) via the
//! pure-Rust `rust_stemmers` port of the Snowball algorithms. No I/O, no
//! wafer/wasm-bindgen deps — the same logic runs in chat, the CLI, and the page.
//!
//! Stemming is a *suffix-stripping* transform, not a dictionary lookup: a stem
//! is often not a real word (`studies` -> `studi`). That is expected and is what
//! makes stems useful as search/index keys.

use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashMap;

/// Longest input accepted, in characters. Keeps a single run bounded inside the
/// 64 MiB wasm sandbox with room for the per-form bookkeeping.
pub const MAX_CHARS: usize = 200_000;

/// Widest accepted `min_length` — beyond this every ordinary word would be
/// skipped, which is never what a user means.
pub const MAX_MIN_LENGTH: usize = 30;

/// A Snowball stemming algorithm, one per supported language.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    Arabic,
    Danish,
    Dutch,
    English,
    Finnish,
    French,
    German,
    Greek,
    Hungarian,
    Italian,
    Norwegian,
    Portuguese,
    Romanian,
    Russian,
    Spanish,
    Swedish,
    Tamil,
    Turkish,
}

impl Language {
    /// Every supported language, as `(canonical name, variant)` pairs. This is
    /// the single source for the descriptor enum, the parser, and the error
    /// message, so the three can never drift apart.
    pub const ALL: [(&'static str, Language); 18] = [
        ("arabic", Language::Arabic),
        ("danish", Language::Danish),
        ("dutch", Language::Dutch),
        ("english", Language::English),
        ("finnish", Language::Finnish),
        ("french", Language::French),
        ("german", Language::German),
        ("greek", Language::Greek),
        ("hungarian", Language::Hungarian),
        ("italian", Language::Italian),
        ("norwegian", Language::Norwegian),
        ("portuguese", Language::Portuguese),
        ("romanian", Language::Romanian),
        ("russian", Language::Russian),
        ("spanish", Language::Spanish),
        ("swedish", Language::Swedish),
        ("tamil", Language::Tamil),
        ("turkish", Language::Turkish),
    ];

    /// Parse a language name. Empty falls back to `english` so an unset page
    /// `<select>` behaves like the schema default.
    pub fn parse(s: &str) -> Result<Self, String> {
        let key = s.trim().to_lowercase();
        if key.is_empty() {
            return Ok(Language::English);
        }
        Self::ALL
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, lang)| *lang)
            .ok_or_else(|| {
                let names: Vec<&str> = Self::ALL.iter().map(|(n, _)| *n).collect();
                format!(
                    "unknown language '{}' — expected one of: {}",
                    s.trim(),
                    names.join(", ")
                )
            })
    }

    /// The canonical lowercase name (what the schema enum uses).
    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, lang)| *lang == self)
            .map(|(n, _)| *n)
            .unwrap_or("english")
    }

    fn algorithm(self) -> Algorithm {
        match self {
            Language::Arabic => Algorithm::Arabic,
            Language::Danish => Algorithm::Danish,
            Language::Dutch => Algorithm::Dutch,
            Language::English => Algorithm::English,
            Language::Finnish => Algorithm::Finnish,
            Language::French => Algorithm::French,
            Language::German => Algorithm::German,
            Language::Greek => Algorithm::Greek,
            Language::Hungarian => Algorithm::Hungarian,
            Language::Italian => Algorithm::Italian,
            Language::Norwegian => Algorithm::Norwegian,
            Language::Portuguese => Algorithm::Portuguese,
            Language::Romanian => Algorithm::Romanian,
            Language::Russian => Algorithm::Russian,
            Language::Spanish => Algorithm::Spanish,
            Language::Swedish => Algorithm::Swedish,
            Language::Tamil => Algorithm::Tamil,
            Language::Turkish => Algorithm::Turkish,
        }
    }
}

/// How the stemmed result is presented.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// The original text with every word replaced by its stem; punctuation,
    /// line breaks, and spacing are preserved byte for byte.
    Text,
    /// Unique stems, one per line, in order of first appearance.
    Stems,
    /// `form -> stem`, one line per unique surface form, first appearance order.
    Mapping,
    /// Tab-separated `STEM / COUNT / FORMS` table plus a summary line.
    Table,
    /// A JSON object with the stem groups and the corpus statistics.
    Json,
}

impl Output {
    /// Parse an output-format name; empty falls back to `text`.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "" | "text" => Ok(Output::Text),
            "stems" => Ok(Output::Stems),
            "mapping" => Ok(Output::Mapping),
            "table" => Ok(Output::Table),
            "json" => Ok(Output::Json),
            other => Err(format!(
                "unknown output '{other}' — expected one of: text, stems, mapping, table, json"
            )),
        }
    }
}

/// Everything the caller can tune, shared by all three surfaces.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub language: Language,
    pub output: Output,
    /// Words shorter than this (in characters) are passed through unstemmed.
    pub min_length: usize,
    /// Lowercase each word before stemming (Snowball algorithms are defined on
    /// lowercase input).
    pub lowercase: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            language: Language::English,
            output: Output::Text,
            min_length: 1,
            lowercase: true,
        }
    }
}

/// One word occurrence: its byte range in the source and its surface form after
/// the optional lowercasing.
struct Token {
    start: usize,
    end: usize,
    form: String,
}

/// A word character: any Unicode letter/digit plus the two apostrophes English
/// contractions use (`don't`, `don’t`) so the stemmer sees the whole word.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '\u{2019}'
}

fn is_apostrophe(c: char) -> bool {
    c == '\'' || c == '\u{2019}'
}

/// Split `text` into word tokens with their byte ranges. Leading/trailing
/// apostrophes are excluded from the token so quotes around a word stay verbatim
/// in `Output::Text`.
fn tokenize(text: &str, lowercase: bool) -> Vec<Token> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if is_word_char(c) {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            push_token(text, s, i, lowercase, &mut out);
        }
    }
    if let Some(s) = start {
        push_token(text, s, text.len(), lowercase, &mut out);
    }
    out
}

fn push_token(text: &str, start: usize, end: usize, lowercase: bool, out: &mut Vec<Token>) {
    let raw = &text[start..end];
    let trimmed = raw.trim_matches(is_apostrophe);
    if trimmed.is_empty() {
        return;
    }
    // Byte offsets of the trimmed slice inside the original span.
    let lead = raw.len() - raw.trim_start_matches(is_apostrophe).len();
    let start = start + lead;
    let end = start + trimmed.len();
    let form = if lowercase {
        trimmed.to_lowercase()
    } else {
        trimmed.to_string()
    };
    out.push(Token { start, end, form });
}

/// A stem and the surface forms that collapsed onto it.
struct Group {
    stem: String,
    count: usize,
    forms: Vec<String>,
}

/// Stem `text` according to `opts` and render the requested output format.
///
/// Errors on empty input, an over-long input, or an out-of-range `min_length`;
/// the message always names the expected value and what was received.
pub fn stem_text(text: &str, opts: Options) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("text is empty — paste at least one word to stem".to_string());
    }
    let chars = text.chars().count();
    if chars > MAX_CHARS {
        return Err(format!(
            "text is {chars} characters; the limit is {MAX_CHARS} — split it into smaller batches"
        ));
    }
    if opts.min_length < 1 || opts.min_length > MAX_MIN_LENGTH {
        return Err(format!(
            "min_length must be between 1 and {MAX_MIN_LENGTH}, got {}",
            opts.min_length
        ));
    }

    let stemmer = Stemmer::create(opts.language.algorithm());
    let tokens = tokenize(text, opts.lowercase);
    if tokens.is_empty() {
        return Err(
            "no words found — the input has no letters or digits to stem".to_string()
        );
    }

    // Stem each distinct surface form once; a real corpus repeats forms a lot.
    let mut stem_of: HashMap<String, String> = HashMap::new();
    for t in &tokens {
        if !stem_of.contains_key(&t.form) {
            let stem = if t.form.chars().count() < opts.min_length {
                t.form.clone()
            } else {
                stemmer.stem(&t.form).into_owned()
            };
            stem_of.insert(t.form.clone(), stem);
        }
    }

    match opts.output {
        Output::Text => {
            let mut out = String::with_capacity(text.len());
            let mut cursor = 0usize;
            for t in &tokens {
                out.push_str(&text[cursor..t.start]);
                out.push_str(stem_for(&stem_of, &t.form));
                cursor = t.end;
            }
            out.push_str(&text[cursor..]);
            Ok(out)
        }
        Output::Stems => {
            let groups = group(&tokens, &stem_of);
            let lines: Vec<&str> = groups.iter().map(|g| g.stem.as_str()).collect();
            Ok(lines.join("\n"))
        }
        Output::Mapping => {
            let forms = unique_forms(&tokens);
            let lines: Vec<String> = forms
                .iter()
                .map(|f| format!("{f} -> {}", stem_for(&stem_of, f)))
                .collect();
            Ok(lines.join("\n"))
        }
        Output::Table => {
            let groups = group(&tokens, &stem_of);
            let stats = Stats::compute(&tokens, &groups);
            let mut ranked: Vec<&Group> = groups.iter().collect();
            ranked.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.stem.cmp(&b.stem)));
            let mut out = String::from("STEM\tCOUNT\tFORMS");
            for g in ranked {
                out.push_str(&format!("\n{}\t{}\t{}", g.stem, g.count, g.forms.join(", ")));
            }
            out.push_str(&format!("\n\n{}", stats.summary()));
            Ok(out)
        }
        Output::Json => {
            let groups = group(&tokens, &stem_of);
            let stats = Stats::compute(&tokens, &groups);
            let value = serde_json::json!({
                "language": opts.language.name(),
                "words": stats.words,
                "unique_forms": stats.unique_forms,
                "unique_stems": stats.unique_stems,
                "compression_ratio": round2(stats.ratio()),
                "stems": groups
                    .iter()
                    .map(|g| serde_json::json!({
                        "stem": g.stem,
                        "count": g.count,
                        "forms": g.forms,
                    }))
                    .collect::<Vec<_>>(),
            });
            serde_json::to_string_pretty(&value).map_err(|e| format!("could not build JSON: {e}"))
        }
    }
}

/// String-argument entry point shared by the chat block, the CLI, and the page.
/// Parses the language/output names first so a typo is reported before any work
/// is done.
pub fn run(
    text: &str,
    language: &str,
    output: &str,
    min_length: u32,
    lowercase: bool,
) -> Result<String, String> {
    let opts = Options {
        language: Language::parse(language)?,
        output: Output::parse(output)?,
        min_length: min_length as usize,
        lowercase,
    };
    stem_text(text, opts)
}

/// Unique surface forms in order of first appearance.
fn unique_forms(tokens: &[Token]) -> Vec<&str> {
    let mut seen: HashMap<&str, ()> = HashMap::new();
    let mut out = Vec::new();
    for t in tokens {
        if seen.insert(t.form.as_str(), ()).is_none() {
            out.push(t.form.as_str());
        }
    }
    out
}

/// The stem recorded for a surface form. Every token's form is inserted before
/// this is called, so the fallback is unreachable in practice.
fn stem_for<'a>(stem_of: &'a HashMap<String, String>, form: &str) -> &'a str {
    stem_of.get(form).map(String::as_str).unwrap_or_default()
}

/// Group occurrences by stem, keeping first-appearance order for both the
/// groups and the forms inside each group.
fn group(tokens: &[Token], stem_of: &HashMap<String, String>) -> Vec<Group> {
    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut groups: Vec<Group> = Vec::new();
    for t in tokens {
        let stem = stem_for(stem_of, &t.form);
        let idx = match index.get(stem) {
            Some(i) => *i,
            None => {
                index.insert(stem, groups.len());
                groups.push(Group {
                    stem: stem.to_string(),
                    count: 0,
                    forms: Vec::new(),
                });
                groups.len() - 1
            }
        };
        let g = &mut groups[idx];
        g.count += 1;
        if !g.forms.iter().any(|f| f == &t.form) {
            g.forms.push(t.form.clone());
        }
    }
    groups
}

/// Corpus statistics reported by the `table` and `json` outputs.
struct Stats {
    words: usize,
    unique_forms: usize,
    unique_stems: usize,
}

impl Stats {
    fn compute(tokens: &[Token], groups: &[Group]) -> Self {
        let unique_forms = unique_forms(tokens).len();
        Stats {
            words: tokens.len(),
            unique_forms,
            unique_stems: groups.len(),
        }
    }

    /// Vocabulary compression: unique stems ÷ unique surface forms. 1.00 means
    /// nothing collapsed; lower means more forms merged onto shared stems.
    fn ratio(&self) -> f64 {
        if self.unique_forms == 0 {
            1.0
        } else {
            self.unique_stems as f64 / self.unique_forms as f64
        }
    }

    fn summary(&self) -> String {
        format!(
            "{} words · {} unique forms · {} unique stems · compression {:.2}",
            self.words,
            self.unique_forms,
            self.unique_stems,
            self.ratio()
        )
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(language: &str, output: &str) -> Options {
        Options {
            language: Language::parse(language).unwrap(),
            output: Output::parse(output).unwrap(),
            ..Options::default()
        }
    }

    #[test]
    fn english_text_keeps_layout() {
        let out = stem_text(
            "The runners are running quickly.",
            opts("english", "text"),
        )
        .unwrap();
        assert_eq!(out, "the runner are run quick.");
    }

    #[test]
    fn german_stems_inflections() {
        // Snowball German folds the umlaut and strips the plural endings, so all
        // three inflections collapse onto one stem.
        let out = stem_text("Häuser Häusern Haus", opts("german", "text")).unwrap();
        assert_eq!(out, "haus haus haus");
    }

    #[test]
    fn spanish_stems_verb_forms() {
        let out = stem_text("corriendo corremos corrió", opts("spanish", "text")).unwrap();
        assert_eq!(out, "corr corr corr");
    }

    #[test]
    fn danish_stems_definite_forms() {
        let out = stem_text("husene huset hus", opts("danish", "text")).unwrap();
        assert_eq!(out, "hus hus hus");
    }

    #[test]
    fn arabic_stems_prefixed_forms() {
        let out = stem_text("الكتاب كتاب", opts("arabic", "text")).unwrap();
        assert_eq!(out, "كتاب كتاب");
    }

    #[test]
    fn russian_stems_case_endings() {
        let out = stem_text("книги книга", opts("russian", "text")).unwrap();
        assert_eq!(out, "книг книг");
    }

    #[test]
    fn stems_output_is_unique_in_first_appearance_order() {
        let out = stem_text("runs running runner jumps", opts("english", "stems")).unwrap();
        assert_eq!(out, "run\nrunner\njump");
    }

    #[test]
    fn mapping_output_lists_every_form() {
        let out = stem_text("Studies studies studying", opts("english", "mapping")).unwrap();
        assert_eq!(out, "studies -> studi\nstudying -> studi");
    }

    #[test]
    fn table_output_ranks_by_count_with_summary() {
        let out = stem_text("cats cat dogs", opts("english", "table")).unwrap();
        assert_eq!(
            out,
            "STEM\tCOUNT\tFORMS\ncat\t2\tcats, cat\ndog\t1\tdogs\n\n\
             3 words · 3 unique forms · 2 unique stems · compression 0.67"
        );
    }

    #[test]
    fn json_output_carries_stats() {
        let out = stem_text("cats cat", opts("english", "json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["language"], "english");
        assert_eq!(v["words"], 2);
        assert_eq!(v["unique_forms"], 2);
        assert_eq!(v["unique_stems"], 1);
        assert_eq!(v["compression_ratio"], 0.5);
        assert_eq!(v["stems"][0]["stem"], "cat");
        assert_eq!(v["stems"][0]["forms"][1], "cat");
    }

    #[test]
    fn min_length_passes_short_words_through() {
        let o = Options {
            min_length: 6,
            ..opts("english", "text")
        };
        // "runs" (4) is left alone; "running" (7) is stemmed.
        assert_eq!(stem_text("runs running", o).unwrap(), "runs run");
    }

    #[test]
    fn lowercase_off_keeps_original_case() {
        let o = Options {
            lowercase: false,
            ..opts("english", "text")
        };
        // Without case folding the two forms stay distinct and each keeps its
        // own capitalisation through the stemmer.
        assert_eq!(stem_text("Running running", o).unwrap(), "Run run");
    }

    #[test]
    fn punctuation_and_contractions_survive() {
        let out = stem_text("\"Don't stop,\" she cried!\n\nReally?", opts("english", "text"))
            .unwrap();
        assert_eq!(out, "\"don't stop,\" she cri!\n\nrealli?");
    }

    #[test]
    fn empty_text_is_an_error() {
        let err = stem_text("   ", opts("english", "text")).unwrap_err();
        assert!(err.contains("text is empty"), "{err}");
    }

    #[test]
    fn text_without_words_is_an_error() {
        let err = stem_text("!!! ... ???", opts("english", "text")).unwrap_err();
        assert!(err.contains("no words found"), "{err}");
    }

    #[test]
    fn unknown_language_is_an_error() {
        let err = Language::parse("klingon").unwrap_err();
        assert!(err.contains("unknown language 'klingon'"), "{err}");
        assert!(err.contains("english"), "{err}");
    }

    #[test]
    fn unknown_output_is_an_error() {
        let err = Output::parse("csv").unwrap_err();
        assert!(err.contains("unknown output 'csv'"), "{err}");
    }

    #[test]
    fn out_of_range_min_length_is_an_error() {
        let o = Options {
            min_length: 99,
            ..opts("english", "text")
        };
        let err = stem_text("running", o).unwrap_err();
        assert!(err.contains("between 1 and 30, got 99"), "{err}");
    }

    #[test]
    fn over_long_text_is_an_error() {
        let big = "word ".repeat(MAX_CHARS / 4);
        let err = stem_text(&big, opts("english", "text")).unwrap_err();
        assert!(err.contains("the limit is 200000"), "{err}");
    }

    #[test]
    fn empty_language_falls_back_to_english() {
        assert_eq!(Language::parse("").unwrap(), Language::English);
        assert_eq!(Output::parse("").unwrap(), Output::Text);
    }

    #[test]
    fn run_takes_string_arguments_from_every_surface() {
        assert_eq!(
            run("The runners are running.", "english", "text", 1, true).unwrap(),
            "the runner are run."
        );
        // min_length 30 is the widest accepted value: nothing gets stemmed.
        assert_eq!(
            run("runners", "english", "text", MAX_MIN_LENGTH as u32, true).unwrap(),
            "runners"
        );
        let err = run("runners", "elvish", "text", 1, true).unwrap_err();
        assert!(err.contains("unknown language 'elvish'"), "{err}");
    }

    #[test]
    fn every_advertised_language_stems_something() {
        for (name, _) in Language::ALL {
            let out = stem_text("kalapok casas running", opts(name, "text"));
            assert!(out.is_ok(), "{name} failed: {out:?}");
        }
    }
}
