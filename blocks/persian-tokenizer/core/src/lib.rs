//! persian-tokenizer core — split Persian/Farsi text into sentences and words.
//! Pure compute, no wafer/wasm-bindgen deps; shared by the chat skill block, the
//! CLI and the web page.
//!
//! Everything is rule-based (no model, no training data) so the same input
//! always produces the same output on every surface. The parts that make
//! Persian different from English are handled explicitly:
//!
//! * **ZWNJ (نیم‌فاصله, U+200C)** joins the parts of one word — `می‌خوانیم`,
//!   `کتاب‌ها`, `نمی‌شود`. By default a ZWNJ-joined compound stays ONE token;
//!   `split_zwnj` breaks it into its parts instead.
//! * **Persian punctuation** — `،` `؛` `؟` `«` `»` `٪` `٫` `٬` `۔` — is
//!   recognised alongside the ASCII marks, and `؟` `۔` `⸮` end a sentence just
//!   like `?` and `.`.
//! * **Three digit sets** — ASCII `0-9`, Arabic-Indic `٠-٩` and Persian
//!   `۰-۹` — all count as digits, so `۱۳۹۶/۰۶/۱۱` and `3.14` stay one token.
//! * **Arabic vs Persian letter forms** — `ي ك ى ة` are folded to `ی ک ی ه`
//!   and the harakat/tatweel are stripped when `normalize` is on, so the same
//!   word typed on an Arabic keyboard tokenizes identically.

use serde::Serialize;

/// Largest input accepted, in Unicode characters. Comfortably above any paste a
/// browser text box handles; keeps the wasm sandbox's memory bounded.
pub const MAX_CHARS: usize = 200_000;

/// Zero-width non-joiner — the Persian half-space (نیم‌فاصله).
const ZWNJ: char = '\u{200C}';
/// Zero-width joiner.
const ZWJ: char = '\u{200D}';
/// Kashida / tatweel — decorative letter stretching with no meaning.
const TATWEEL: char = '\u{0640}';

/// What to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Word tokens for the whole text. Default.
    Words,
    /// Sentences only.
    Sentences,
    /// Sentences, each with its own word tokens.
    Both,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "words" => Ok(Mode::Words),
            "sentences" => Ok(Mode::Sentences),
            "both" => Ok(Mode::Both),
            other => Err(format!(
                "invalid mode {other:?}: expected \"words\", \"sentences\" or \"both\""
            )),
        }
    }
}

/// How the result is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// One item per line. Default.
    Lines,
    /// One item per line, prefixed `1. `, `2. `.
    Numbered,
    /// Items joined by a single space.
    SpaceSeparated,
    /// Machine-readable JSON with counts.
    Json,
}

impl Format {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "lines" => Ok(Format::Lines),
            "numbered" => Ok(Format::Numbered),
            "space-separated" => Ok(Format::SpaceSeparated),
            "json" => Ok(Format::Json),
            other => Err(format!(
                "invalid format {other:?}: expected \"lines\", \"numbered\", \"space-separated\" or \"json\""
            )),
        }
    }
}

/// What happens to punctuation marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Punctuation {
    /// Each punctuation run becomes its own token. Default.
    Separate,
    /// Punctuation stays attached to the word it touches (whitespace-only split).
    Attach,
    /// Punctuation tokens are dropped.
    Remove,
}

impl Punctuation {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "separate" => Ok(Punctuation::Separate),
            "attach" => Ok(Punctuation::Attach),
            "remove" => Ok(Punctuation::Remove),
            other => Err(format!(
                "invalid punctuation {other:?}: expected \"separate\", \"attach\" or \"remove\""
            )),
        }
    }
}

/// How a line break affects sentence boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Newlines {
    /// Only a blank line (two or more line breaks) ends a sentence. Default.
    Paragraph,
    /// Line breaks are ordinary whitespace; only punctuation ends a sentence.
    Never,
    /// Every line break ends a sentence (lists, subtitles, one item per line).
    Always,
}

impl Newlines {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "paragraph" => Ok(Newlines::Paragraph),
            "never" => Ok(Newlines::Never),
            "always" => Ok(Newlines::Always),
            other => Err(format!(
                "invalid newlines {other:?}: expected \"paragraph\", \"never\" or \"always\""
            )),
        }
    }
}

/// Every knob the tokenizer takes, so the surfaces share one shape.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub mode: Mode,
    pub format: Format,
    pub punctuation: Punctuation,
    /// Break a ZWNJ-joined compound into its parts (`می‌خوانیم` → `می` + `خوانیم`).
    pub split_zwnj: bool,
    /// Fold Arabic letter forms to Persian and strip harakat/tatweel first.
    pub normalize: bool,
    /// Keep URLs, emails, @mentions, #hashtags and separator-bearing numbers whole.
    pub keep_entities: bool,
    pub newlines: Newlines,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Words,
            format: Format::Lines,
            punctuation: Punctuation::Separate,
            split_zwnj: false,
            normalize: true,
            keep_entities: true,
            newlines: Newlines::Paragraph,
        }
    }
}

/// One segmented sentence with its word tokens.
#[derive(Debug, Clone, Serialize)]
pub struct Sentence {
    /// 1-based position in the input.
    pub index: usize,
    /// The sentence text, whitespace-collapsed and trimmed.
    pub text: String,
    /// The sentence's word tokens under the current options.
    pub tokens: Vec<String>,
    /// Character length of `text`.
    pub characters: usize,
}

/// Full tokenization of an input.
#[derive(Debug, Clone)]
pub struct Tokenized {
    pub sentences: Vec<Sentence>,
}

impl Tokenized {
    /// Every token, in reading order, across all sentences.
    pub fn tokens(&self) -> Vec<&str> {
        self.sentences
            .iter()
            .flat_map(|s| s.tokens.iter().map(String::as_str))
            .collect()
    }

    pub fn token_count(&self) -> usize {
        self.sentences.iter().map(|s| s.tokens.len()).sum()
    }
}

/// Tokenize `text` under `opts`. Errors on empty/over-long input only — the
/// enum parsing happens in [`run`].
pub fn tokenize(text: &str, opts: Options) -> Result<Tokenized, String> {
    let count = text.chars().count();
    if count > MAX_CHARS {
        return Err(format!(
            "text is too long: {count} characters, maximum {MAX_CHARS}"
        ));
    }
    let cleaned = clean(text, opts.normalize);
    if cleaned.trim().is_empty() {
        return Err("text is empty: expected Persian text to tokenize, e.g. 'ما کتاب می‌خوانیم.'"
            .to_string());
    }

    let sentences: Vec<Sentence> = split_sentences(&cleaned, opts.newlines)
        .into_iter()
        .enumerate()
        .map(|(i, text)| {
            let tokens = tokenize_words(&text, opts);
            Sentence {
                index: i + 1,
                characters: text.chars().count(),
                text,
                tokens,
            }
        })
        .collect();

    let out = Tokenized { sentences };
    if out.token_count() == 0 {
        return Err(
            "no tokens found: the input has no words or numbers left after the punctuation and \
             normalization options were applied"
                .to_string(),
        );
    }
    Ok(out)
}

/// Parse the string-shaped surface arguments, tokenize, and render.
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    mode: &str,
    format: &str,
    punctuation: &str,
    split_zwnj: bool,
    normalize: bool,
    keep_entities: bool,
    newlines: &str,
) -> Result<String, String> {
    let opts = Options {
        mode: Mode::parse(mode)?,
        format: Format::parse(format)?,
        punctuation: Punctuation::parse(punctuation)?,
        split_zwnj,
        normalize,
        keep_entities,
        newlines: Newlines::parse(newlines)?,
    };
    Ok(render(&tokenize(text, opts)?, opts))
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

/// Drop invisible formatting characters (always) and, when `normalize` is on,
/// fold Arabic letter forms to Persian and strip harakat + tatweel.
fn clean(text: &str, normalize: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            // Bidi/format controls and the soft hyphen are never tokens.
            '\u{200E}' | '\u{200F}' | '\u{061C}' | '\u{FEFF}' | '\u{00AD}' | '\u{2066}'
            | '\u{2067}' | '\u{2068}' | '\u{2069}' | '\u{202A}'..='\u{202E}' => continue,
            _ => {}
        }
        if normalize {
            if is_harakat(c) || c == TATWEEL {
                continue;
            }
            out.push(fold_letter(c));
        } else {
            out.push(c);
        }
    }
    out
}

/// Arabic letter forms that Persian writes differently.
fn fold_letter(c: char) -> char {
    match c {
        'ي' | 'ى' | 'ﻯ' | 'ﻱ' => 'ی',
        'ك' | 'ﻙ' | 'ﻚ' => 'ک',
        'ة' => 'ه',
        // Arabic-Indic digits → Persian digits (both stay "digits" either way).
        '\u{0660}'..='\u{0669}' => char::from_u32(c as u32 + 0x06F0 - 0x0660).unwrap_or(c),
        _ => c,
    }
}

/// Arabic combining marks: harakat/tashkeel and Quranic annotation marks.
fn is_harakat(c: char) -> bool {
    matches!(c, '\u{064B}'..='\u{065F}' | '\u{0670}' | '\u{06D6}'..='\u{06ED}')
}

// ---------------------------------------------------------------------------
// Character classes
// ---------------------------------------------------------------------------

fn is_digit_char(c: char) -> bool {
    matches!(c, '0'..='9' | '\u{0660}'..='\u{0669}' | '\u{06F0}'..='\u{06F9}')
}

/// A letter, or a combining mark that belongs to the letter before it.
fn is_word_char(c: char) -> bool {
    (c.is_alphabetic() && !is_digit_char(c)) || is_harakat(c) || c == TATWEEL
}

/// Separators that stay inside a number when digits sit on both sides:
/// `3.14`, `1,000`, `۱۳۹۶/۰۶/۱۱`, `12:30`, `۳٫۵` (Persian decimal), `۱٬۰۰۰`.
fn is_number_separator(c: char) -> bool {
    matches!(c, '.' | ',' | '/' | ':' | '-' | '\u{066B}' | '\u{066C}' | '،')
}

/// Characters that end a sentence, ASCII and Persian/Arabic alike.
fn is_terminator(c: char) -> bool {
    matches!(
        c,
        '.' | '!' | '?' | '\u{061F}' | '\u{06D4}' | '\u{2E2E}' | '\u{2026}' | '\u{FF01}'
            | '\u{FF1F}' | '\u{FF0E}' | '\u{3002}'
    )
}

/// Closing marks that belong to the sentence they follow.
fn is_closer(c: char) -> bool {
    matches!(c, '»' | '"' | '\'' | '’' | '”' | ')' | ']' | '}' | '›')
}

/// Invisible joiners handled by the word scanner, never tokens on their own.
fn is_joiner(c: char) -> bool {
    c == ZWNJ || c == ZWJ
}

// ---------------------------------------------------------------------------
// Sentence segmentation
// ---------------------------------------------------------------------------

fn split_sentences(text: &str, newlines: Newlines) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];

        if c == '\n' || c == '\r' {
            let mut j = i;
            let mut breaks = 0usize;
            while j < chars.len() && matches!(chars[j], '\n' | '\r' | ' ' | '\t') {
                if chars[j] == '\n' {
                    breaks += 1;
                }
                j += 1;
            }
            let boundary = match newlines {
                Newlines::Always => true,
                Newlines::Paragraph => breaks >= 2,
                Newlines::Never => false,
            };
            if boundary {
                push_sentence(&mut out, &chars[start..i]);
                start = j;
            }
            i = j;
            continue;
        }

        if is_terminator(c) {
            // Absorb a run of terminators (`؟؟`, `...`, `!!!`) plus any closing
            // quote/bracket that belongs to the same sentence.
            let mut end = i;
            while end < chars.len() && is_terminator(chars[end]) {
                end += 1;
            }
            while end < chars.len() && is_closer(chars[end]) {
                end += 1;
            }
            // A terminator only ends a sentence when whitespace or the end of
            // the text follows it — so `3.14`, `www.example.com` and
            // `user@host.ir` stay intact, exactly like a real boundary needs.
            let followed_by_break = end >= chars.len() || chars[end].is_whitespace();
            if followed_by_break && !is_initial(&chars, i) {
                push_sentence(&mut out, &chars[start..end]);
                start = end;
            }
            i = end.max(i + 1);
            continue;
        }

        i += 1;
    }
    push_sentence(&mut out, &chars[start..]);
    out
}

/// True when the period at `dot` follows a lone letter — an initial such as
/// `J. R. R.` or `ا.` — which never ends a sentence.
fn is_initial(chars: &[char], dot: usize) -> bool {
    if chars[dot] != '.' || dot == 0 {
        return false;
    }
    if !chars[dot - 1].is_alphabetic() {
        return false;
    }
    dot < 2 || !chars[dot - 2].is_alphabetic()
}

/// Collapse whitespace runs to one space, trim, and skip empties.
fn push_sentence(out: &mut Vec<String>, chars: &[char]) {
    let mut s = String::with_capacity(chars.len());
    let mut pending_space = false;
    for &c in chars {
        if c.is_whitespace() {
            pending_space = !s.is_empty();
            continue;
        }
        if pending_space {
            s.push(' ');
            pending_space = false;
        }
        s.push(c);
    }
    if !s.is_empty() {
        out.push(s);
    }
}

// ---------------------------------------------------------------------------
// Word tokenization
// ---------------------------------------------------------------------------

fn tokenize_words(text: &str, opts: Options) -> Vec<String> {
    if opts.punctuation == Punctuation::Attach {
        // "Attach" means the only separator is whitespace: every mark stays
        // glued to the word it was typed against.
        return text
            .split_whitespace()
            .flat_map(|chunk| {
                if opts.split_zwnj {
                    chunk
                        .split(ZWNJ)
                        .filter(|p| !p.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                } else {
                    vec![chunk.to_string()]
                }
            })
            .collect();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() || is_joiner(c) {
            i += 1;
            continue;
        }

        if opts.keep_entities {
            if let Some(end) = match_entity(&chars, i) {
                out.push(chars[i..end].iter().collect());
                i = end;
                continue;
            }
        }

        if is_digit_char(c) {
            let mut j = i + 1;
            while j < chars.len() {
                if is_digit_char(chars[j]) {
                    j += 1;
                } else if opts.keep_entities
                    && is_number_separator(chars[j])
                    && j + 1 < chars.len()
                    && is_digit_char(chars[j + 1])
                {
                    j += 2;
                } else {
                    break;
                }
            }
            out.push(chars[i..j].iter().collect());
            i = j;
            continue;
        }

        if is_word_char(c) {
            let mut j = i + 1;
            while j < chars.len() {
                let d = chars[j];
                if is_word_char(d) || d == ZWJ {
                    j += 1;
                } else if d == ZWNJ && j + 1 < chars.len() && is_word_char(chars[j + 1]) {
                    if opts.split_zwnj {
                        break;
                    }
                    j += 1;
                } else if matches!(d, '\'' | '’')
                    && j + 1 < chars.len()
                    && is_word_char(chars[j + 1])
                {
                    j += 1;
                } else {
                    break;
                }
            }
            out.push(chars[i..j].iter().collect());
            i = j;
            continue;
        }

        // Punctuation or symbol: a run of the SAME character is one mark
        // (`...`, `؟؟`, `!!!`).
        let mut j = i + 1;
        while j < chars.len() && chars[j] == c {
            j += 1;
        }
        if opts.punctuation == Punctuation::Separate {
            out.push(chars[i..j].iter().collect());
        }
        i = j;
    }
    out
}

/// Match a URL, @mention, #hashtag or email starting at `i`; returns the
/// exclusive end index of the whole entity.
fn match_entity(chars: &[char], i: usize) -> Option<usize> {
    // URL — http(s):// or a bare www. host.
    let head: String = chars[i..chars.len().min(i + 8)]
        .iter()
        .collect::<String>()
        .to_lowercase();
    if head.starts_with("http://") || head.starts_with("https://") || head.starts_with("www.") {
        let mut j = i;
        while j < chars.len() && !chars[j].is_whitespace() {
            j += 1;
        }
        // A trailing sentence mark belongs to the sentence, not the URL.
        while j > i
            && matches!(
                chars[j - 1],
                '.' | ',' | ';' | ':' | '!' | '?' | '\u{061F}' | '\u{060C}' | '\u{061B}' | ')'
                    | ']' | '»' | '"' | '\''
            )
        {
            j -= 1;
        }
        if j > i {
            return Some(j);
        }
    }

    // @mention / #hashtag.
    if matches!(chars[i], '@' | '#')
        && i + 1 < chars.len()
        && (is_word_char(chars[i + 1]) || is_digit_char(chars[i + 1]))
    {
        let mut j = i + 1;
        while j < chars.len()
            && (is_word_char(chars[j]) || is_digit_char(chars[j]) || chars[j] == '_' || is_joiner(chars[j]))
        {
            j += 1;
        }
        return Some(j);
    }

    // Email — local@domain.tld.
    if is_word_char(chars[i]) || is_digit_char(chars[i]) {
        let mut j = i;
        while j < chars.len()
            && (is_word_char(chars[j])
                || is_digit_char(chars[j])
                || matches!(chars[j], '.' | '_' | '%' | '+' | '-'))
        {
            j += 1;
        }
        if j < chars.len() && chars[j] == '@' {
            let at = j;
            let mut k = at + 1;
            let mut has_dot = false;
            while k < chars.len()
                && (is_word_char(chars[k]) || is_digit_char(chars[k]) || matches!(chars[k], '.' | '-'))
            {
                if chars[k] == '.' {
                    has_dot = true;
                }
                k += 1;
            }
            while k > at + 1 && matches!(chars[k - 1], '.' | '-') {
                k -= 1;
            }
            if has_dot && k > at + 1 {
                return Some(k);
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct WordsJson<'a> {
    mode: &'static str,
    sentence_count: usize,
    token_count: usize,
    tokens: Vec<&'a str>,
}

#[derive(Serialize)]
struct SentencesJson<'a> {
    mode: &'static str,
    sentence_count: usize,
    token_count: usize,
    sentences: Vec<&'a Sentence>,
}

fn render(t: &Tokenized, opts: Options) -> String {
    match opts.mode {
        Mode::Words => {
            let tokens = t.tokens();
            match opts.format {
                Format::Lines => tokens.join("\n"),
                Format::Numbered => tokens
                    .iter()
                    .enumerate()
                    .map(|(i, tok)| format!("{}. {}", i + 1, tok))
                    .collect::<Vec<_>>()
                    .join("\n"),
                Format::SpaceSeparated => tokens.join(" "),
                Format::Json => json(&WordsJson {
                    mode: "words",
                    sentence_count: t.sentences.len(),
                    token_count: tokens.len(),
                    tokens,
                }),
            }
        }
        Mode::Sentences => match opts.format {
            Format::Lines => t
                .sentences
                .iter()
                .map(|s| s.text.clone())
                .collect::<Vec<_>>()
                .join("\n"),
            Format::Numbered => t
                .sentences
                .iter()
                .map(|s| format!("{}. {}", s.index, s.text))
                .collect::<Vec<_>>()
                .join("\n"),
            Format::SpaceSeparated => t
                .sentences
                .iter()
                .map(|s| s.text.clone())
                .collect::<Vec<_>>()
                .join(" "),
            Format::Json => json(&SentencesJson {
                mode: "sentences",
                sentence_count: t.sentences.len(),
                token_count: t.token_count(),
                sentences: t.sentences.iter().collect(),
            }),
        },
        Mode::Both => match opts.format {
            Format::Lines => t
                .sentences
                .iter()
                .map(|s| {
                    let mut block = s.text.clone();
                    for tok in &s.tokens {
                        block.push_str("\n  ");
                        block.push_str(tok);
                    }
                    block
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
            Format::Numbered => t
                .sentences
                .iter()
                .map(|s| {
                    let mut block = format!("{}. {}", s.index, s.text);
                    for (k, tok) in s.tokens.iter().enumerate() {
                        block.push_str(&format!("\n  {}.{}. {}", s.index, k + 1, tok));
                    }
                    block
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
            Format::SpaceSeparated => t
                .sentences
                .iter()
                .map(|s| s.tokens.join(" "))
                .collect::<Vec<_>>()
                .join("\n"),
            Format::Json => json(&SentencesJson {
                mode: "both",
                sentence_count: t.sentences.len(),
                token_count: t.token_count(),
                sentences: t.sentences.iter().collect(),
            }),
        },
    }
}

fn json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        tokenize(text, Options::default())
            .unwrap()
            .tokens()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn tokenizes_zwnj_compound_as_one_word() {
        assert_eq!(words("ما کتاب می‌خوانیم."), ["ما", "کتاب", "می‌خوانیم", "."]);
    }

    #[test]
    fn splits_zwnj_compound_on_request() {
        let opts = Options {
            split_zwnj: true,
            ..Options::default()
        };
        let t = tokenize("ما کتاب می‌خوانیم.", opts).unwrap();
        assert_eq!(t.tokens(), ["ما", "کتاب", "می", "خوانیم", "."]);
    }

    #[test]
    fn splits_sentences_on_persian_question_mark() {
        let opts = Options {
            mode: Mode::Sentences,
            ..Options::default()
        };
        let t = tokenize("حال شما چطور است؟ من خوبم. ممنون!", opts).unwrap();
        let texts: Vec<&str> = t.sentences.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["حال شما چطور است؟", "من خوبم.", "ممنون!"]);
    }

    #[test]
    fn keeps_numbers_urls_and_emails_whole() {
        assert_eq!(
            words("قیمت 1,250.75 است https://example.com/a.b و info@example.com"),
            [
                "قیمت",
                "1,250.75",
                "است",
                "https://example.com/a.b",
                "و",
                "info@example.com"
            ]
        );
    }

    #[test]
    fn persian_date_stays_one_token() {
        assert_eq!(words("تاریخ ۱۳۹۶/۰۶/۱۱ بود"), ["تاریخ", "۱۳۹۶/۰۶/۱۱", "بود"]);
    }

    #[test]
    fn splits_numbers_at_separators_without_entities() {
        let opts = Options {
            keep_entities: false,
            ..Options::default()
        };
        let t = tokenize("تاریخ ۱۳۹۶/۰۶/۱۱ بود", opts).unwrap();
        assert_eq!(t.tokens(), ["تاریخ", "۱۳۹۶", "/", "۰۶", "/", "۱۱", "بود"]);
    }

    #[test]
    fn normalizes_arabic_letters_and_strips_harakat() {
        // Arabic ي/ك plus a fatha — folded to the Persian ی/ک forms.
        assert_eq!(words("كتابي\u{064E} خوب"), ["کتابی", "خوب"]);
    }

    #[test]
    fn keeps_arabic_letters_when_normalize_is_off() {
        let opts = Options {
            normalize: false,
            ..Options::default()
        };
        let t = tokenize("كتابي خوب", opts).unwrap();
        assert_eq!(t.tokens(), ["كتابي", "خوب"]);
    }

    #[test]
    fn punctuation_modes() {
        let attach = Options {
            punctuation: Punctuation::Attach,
            ..Options::default()
        };
        assert_eq!(
            tokenize("سلام، دنیا!", attach).unwrap().tokens(),
            ["سلام،", "دنیا!"]
        );
        let remove = Options {
            punctuation: Punctuation::Remove,
            ..Options::default()
        };
        assert_eq!(
            tokenize("سلام، دنیا!", remove).unwrap().tokens(),
            ["سلام", "دنیا"]
        );
        assert_eq!(words("سلام، دنیا!"), ["سلام", "،", "دنیا", "!"]);
    }

    #[test]
    fn repeated_marks_group_into_one_token() {
        assert_eq!(words("واقعاً؟؟ بله..."), ["واقعا", "؟؟", "بله", "..."]);
    }

    #[test]
    fn decimal_point_does_not_end_a_sentence() {
        let opts = Options {
            mode: Mode::Sentences,
            ..Options::default()
        };
        let t = tokenize("عدد پی 3.14 است. تمام.", opts).unwrap();
        assert_eq!(t.sentences.len(), 2);
        assert_eq!(t.sentences[0].text, "عدد پی 3.14 است.");
    }

    #[test]
    fn newlines_always_breaks_every_line() {
        let opts = Options {
            mode: Mode::Sentences,
            newlines: Newlines::Always,
            ..Options::default()
        };
        let t = tokenize("خرید شیر\nپیاده‌روی\nرزرو بلیت", opts).unwrap();
        assert_eq!(t.sentences.len(), 3);
        assert_eq!(t.sentences[1].text, "پیاده‌روی");
    }

    #[test]
    fn newlines_never_joins_wrapped_lines() {
        let opts = Options {
            mode: Mode::Sentences,
            newlines: Newlines::Never,
            ..Options::default()
        };
        let t = tokenize("این یک جمله\nطولانی است.", opts).unwrap();
        assert_eq!(t.sentences.len(), 1);
        assert_eq!(t.sentences[0].text, "این یک جمله طولانی است.");
    }

    #[test]
    fn renders_every_format() {
        assert_eq!(
            run("سلام دنیا", "words", "lines", "separate", false, true, true, "paragraph").unwrap(),
            "سلام\nدنیا"
        );
        assert_eq!(
            run("سلام دنیا", "words", "numbered", "separate", false, true, true, "paragraph")
                .unwrap(),
            "1. سلام\n2. دنیا"
        );
        assert_eq!(
            run(
                "سلام دنیا",
                "words",
                "space-separated",
                "separate",
                false,
                true,
                true,
                "paragraph"
            )
            .unwrap(),
            "سلام دنیا"
        );
        assert_eq!(
            run("سلام دنیا", "words", "json", "separate", false, true, true, "paragraph").unwrap(),
            "{\"mode\":\"words\",\"sentence_count\":1,\"token_count\":2,\"tokens\":[\"سلام\",\"دنیا\"]}"
        );
    }

    #[test]
    fn both_mode_lists_tokens_under_each_sentence() {
        let out = run(
            "سلام دنیا. خوبی؟",
            "both",
            "lines",
            "separate",
            false,
            true,
            true,
            "paragraph",
        )
        .unwrap();
        assert_eq!(out, "سلام دنیا.\n  سلام\n  دنیا\n  .\n\nخوبی؟\n  خوبی\n  ؟");
    }

    #[test]
    fn both_mode_space_separated_is_one_line_per_sentence() {
        let out = run(
            "سلام دنیا. خوبی؟",
            "both",
            "space-separated",
            "remove",
            false,
            true,
            true,
            "paragraph",
        )
        .unwrap();
        assert_eq!(out, "سلام دنیا\nخوبی");
    }

    #[test]
    fn rejects_empty_input() {
        let err = run("   ", "words", "lines", "separate", false, true, true, "paragraph")
            .unwrap_err();
        assert!(err.starts_with("text is empty"), "{err}");
    }

    #[test]
    fn rejects_input_over_the_cap() {
        let big = "ا".repeat(MAX_CHARS + 1);
        let err = run(&big, "words", "lines", "separate", false, true, true, "paragraph")
            .unwrap_err();
        assert_eq!(
            err,
            format!("text is too long: {} characters, maximum {MAX_CHARS}", MAX_CHARS + 1)
        );
    }

    #[test]
    fn accepts_input_exactly_at_the_cap() {
        let big = "ا".repeat(MAX_CHARS);
        let out = run(&big, "words", "lines", "separate", false, true, true, "paragraph").unwrap();
        assert_eq!(out.chars().count(), MAX_CHARS);
    }

    #[test]
    fn rejects_unknown_option_values() {
        let err = run("سلام", "letters", "lines", "separate", false, true, true, "paragraph")
            .unwrap_err();
        assert_eq!(
            err,
            "invalid mode \"letters\": expected \"words\", \"sentences\" or \"both\""
        );
        let err = run("سلام", "words", "csv", "separate", false, true, true, "paragraph")
            .unwrap_err();
        assert!(err.starts_with("invalid format \"csv\""), "{err}");
        let err = run("سلام", "words", "lines", "keep", false, true, true, "paragraph")
            .unwrap_err();
        assert!(err.starts_with("invalid punctuation \"keep\""), "{err}");
        let err = run("سلام", "words", "lines", "separate", false, true, true, "sometimes")
            .unwrap_err();
        assert!(err.starts_with("invalid newlines \"sometimes\""), "{err}");
    }

    #[test]
    fn errors_when_everything_is_filtered_out() {
        let err = run("!!! ??? ...", "words", "lines", "remove", false, true, true, "paragraph")
            .unwrap_err();
        assert!(err.starts_with("no tokens found"), "{err}");
    }

    #[test]
    fn hashtags_and_mentions_survive_as_one_token() {
        assert_eq!(
            words("سلام @ali_r و #تهران_زیبا"),
            ["سلام", "@ali_r", "و", "#تهران_زیبا"]
        );
    }
}
