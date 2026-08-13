//! sentence-tokenizer core — split plain text into sentences and into word /
//! number / punctuation tokens, each carrying its character span in the source
//! text. No wafer/wasm-bindgen deps; shared by the chat skill block, the CLI
//! and the web page.
//!
//! The pipeline is tokenize-then-segment (the order every offset-preserving
//! tokenizer uses): the scanner walks the input once, emitting tokens with
//! `start`/`end` character offsets, and the segmenter then groups that token
//! stream into sentences. Because segmentation never re-slices the text, every
//! offset in the output indexes the ORIGINAL input exactly.
//!
//! Everything is rule-based and deterministic — no model, no training data —
//! so the same input always produces the same output on every surface.

use serde::Serialize;

/// Largest input accepted, in Unicode characters. Well above any paste a
/// browser text box handles comfortably; keeps the 64 MiB wasm sandbox safe.
pub const MAX_CHARS: usize = 500_000;

/// Abbreviations and titles that never end a sentence, whatever follows.
/// Stored lowercase, without the trailing period. Dotted forms (`e.g`) are
/// matched against the same dotted stem the scanner reads back from the text.
const NEVER_BOUNDARY: &[&str] = &[
    // Personal / professional titles.
    "mr", "mrs", "ms", "mx", "dr", "prof", "sr", "jr", "st", "mt", "rev", "hon", "gen", "col",
    "capt", "lt", "sgt", "maj", "adm", "cmdr", "cpl", "pvt", "gov", "pres", "supt", "atty",
    "messrs", "mme", "mlle", "rep", "sen", "insp", "det", "ofc", "ph", "ph.d", "m.d", "b.a",
    "m.a", "b.sc", "m.sc",
    // Inline Latin connectives — these introduce a continuation, never a new
    // sentence, even when the next word is capitalised ("e.g. Apples").
    "e.g", "i.e", "viz", "cf", "c.f", "vs", "v.s", "al", "et", "ca", "etc", "resp",
    // Common business / address abbreviations.
    "inc", "ltd", "co", "corp", "dept", "est", "approx", "min", "max", "incl", "excl", "apt",
    "ave", "blvd", "rd", "hwy", "sq",
];

/// Abbreviations that only suppress a break when a number follows, so
/// "No. 5" stays joined while "The answer is no. Then we left." still splits.
const NUMBER_PREFIXES: &[&str] = &[
    "no", "nos", "fig", "figs", "vol", "vols", "pp", "ch", "chap", "sec", "art", "ref", "eq",
    "eqs", "para", "pt", "ver", "iss", "ex", "jan", "feb", "mar", "apr", "jun", "jul", "aug",
    "sep", "sept", "oct", "nov", "dec", "mon", "tue", "tues", "wed", "thu", "thur", "thurs",
    "fri", "sat", "sun",
];

/// Characters that can terminate a sentence, including the full-width forms
/// used in Chinese and Japanese text.
const TERMINATORS: &[char] = &['.', '!', '?', '…', '。', '！', '？', '‼', '⁇', '⁈', '⁉'];

/// Closing quotes and brackets that belong to the sentence they follow.
const CLOSERS: &[char] = &['"', '\'', ')', ']', '}', '”', '’', '»', '›'];

/// Opening quotes and brackets that can legitimately start a new sentence.
const OPENERS: &[char] = &['"', '\'', '(', '[', '{', '“', '‘', '«', '‹', '¿', '¡'];

/// Characters classified as punctuation rather than as symbols.
const PUNCT_CHARS: &[char] = &[
    '.', ',', ';', ':', '!', '?', '…', '"', '\'', '`', '(', ')', '[', ']', '{', '}', '«', '»',
    '‹', '›', '„', '“', '”', '‘', '’', '-', '–', '—', '‑', '/', '\\', '¡', '¿', '،', '؛', '。',
    '、', '！', '？', '‼', '⁇', '⁈', '⁉',
];

/// Punctuation runs that collapse into a single token (`...`, `!!!`, `--`).
const RUN_CHARS: &[char] = &['.', '!', '?', '-', '…'];

/// How a line break affects sentence boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Newlines {
    /// Only a blank line (two or more line breaks) ends a sentence. Default.
    Paragraph,
    /// Line breaks are ordinary whitespace; only punctuation ends a sentence.
    Never,
    /// Every line break ends a sentence (line-oriented text: lists, subtitles).
    Always,
}

impl Newlines {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "paragraph" => Ok(Newlines::Paragraph),
            "never" => Ok(Newlines::Never),
            "always" => Ok(Newlines::Always),
            other => Err(format!(
                "invalid newlines {other:?}: expected \"paragraph\", \"never\", or \"always\""
            )),
        }
    }

    /// How many line breaks before a token force a sentence break.
    fn threshold(self) -> usize {
        match self {
            Newlines::Always => 1,
            Newlines::Paragraph => 2,
            Newlines::Never => usize::MAX,
        }
    }
}

/// How the token stream is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Full structure: counts, sentence spans, token spans and types.
    Json,
    /// Tab-separated rows, one per token, with a header line.
    Table,
    /// One token per line.
    Lines,
    /// One sentence per line, tokens separated by single spaces.
    Spaces,
    /// One sentence per line, original text.
    Sentences,
}

impl Format {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "json" => Ok(Format::Json),
            "table" => Ok(Format::Table),
            "lines" => Ok(Format::Lines),
            "spaces" => Ok(Format::Spaces),
            "sentences" => Ok(Format::Sentences),
            other => Err(format!(
                "invalid format {other:?}: expected \"json\", \"table\", \"lines\", \"spaces\", or \"sentences\""
            )),
        }
    }
}

/// What a token is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenType {
    /// A word, an abbreviation with its period, or a contraction piece.
    Word,
    /// A number, including internal separators (`1,000.00`, `2018-11-11`).
    Number,
    /// Punctuation: terminators, commas, quotes, brackets, dashes.
    Punct,
    /// Anything else non-alphanumeric: currency, maths, `@`, `#`, emoji.
    Symbol,
    /// A web address kept intact.
    Url,
    /// An e-mail address kept intact.
    Email,
}

impl TokenType {
    pub fn as_str(self) -> &'static str {
        match self {
            TokenType::Word => "word",
            TokenType::Number => "number",
            TokenType::Punct => "punct",
            TokenType::Symbol => "symbol",
            TokenType::Url => "url",
            TokenType::Email => "email",
        }
    }
}

/// Tokenizer / segmenter settings.
#[derive(Debug, Clone)]
pub struct Options {
    pub newlines: Newlines,
    /// Split `don't` into `do` + `n't` and `Anna's` into `Anna` + `'s`.
    pub split_contractions: bool,
    /// Split `state-of-the-art` into its parts plus the hyphens.
    pub split_hyphenated: bool,
    /// Lowercase the emitted text (offsets still point at the original).
    pub lowercase: bool,
    /// Drop punctuation and symbol tokens from the output.
    pub drop_punctuation: bool,
    /// Extra never-split abbreviations, lowercase, without trailing periods.
    pub extra_abbreviations: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            newlines: Newlines::Paragraph,
            split_contractions: true,
            split_hyphenated: false,
            lowercase: false,
            drop_punctuation: false,
            extra_abbreviations: Vec::new(),
        }
    }
}

/// One token with its span in the source text. `start` is inclusive and `end`
/// exclusive, both counted in Unicode characters from the start of the input.
#[derive(Debug, Clone, Serialize)]
pub struct Token {
    /// 1-based position within its sentence.
    pub index: usize,
    pub start: usize,
    pub end: usize,
    #[serde(rename = "type")]
    pub kind: TokenType,
    pub text: String,
}

/// One sentence with its span and its tokens.
#[derive(Debug, Clone, Serialize)]
pub struct Sentence {
    /// 1-based position within the text.
    pub index: usize,
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub tokens: Vec<Token>,
}

/// Totals for the whole input.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Counts {
    pub sentences: usize,
    pub tokens: usize,
    pub words: usize,
    pub numbers: usize,
    pub punctuation: usize,
    pub symbols: usize,
    pub urls: usize,
    pub emails: usize,
    /// Unicode characters in the input text.
    pub characters: usize,
}

/// The complete analysis, serialized as-is by `format = "json"`.
#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    pub counts: Counts,
    pub sentences: Vec<Sentence>,
}

/// The two abbreviation lists, with the caller's extras merged into the first.
struct Abbrevs {
    never: Vec<String>,
    numeric: Vec<String>,
}

impl Abbrevs {
    fn build(extra: &[String]) -> Self {
        let mut never: Vec<String> = NEVER_BOUNDARY.iter().map(|s| s.to_string()).collect();
        never.extend(extra.iter().cloned());
        Abbrevs {
            never,
            numeric: NUMBER_PREFIXES.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn is_never(&self, stem: &str) -> bool {
        self.never.iter().any(|a| a == stem)
    }

    fn is_numeric(&self, stem: &str) -> bool {
        self.numeric.iter().any(|a| a == stem)
    }

    /// Whether a period directly after `stem` belongs to the word token.
    fn absorbs_dot(&self, stem: &str) -> bool {
        self.is_never(stem) || self.is_numeric(stem)
    }
}

/// A scanned token, before the text is materialised.
#[derive(Debug, Clone, Copy)]
struct Raw {
    start: usize,
    end: usize,
    kind: TokenType,
    /// Line breaks between the previous token and this one.
    breaks_before: usize,
}

/// Parse a free-text abbreviation list (`Corp., Ltd; Inc`) into lowercase stems.
pub fn parse_abbreviations(list: &str) -> Vec<String> {
    list.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(|s| s.trim().trim_end_matches('.').to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Tokenize + segment `text`. The main entry point for tests and for `run`.
pub fn analyze(text: &str, opts: &Options) -> Result<Analysis, String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() > MAX_CHARS {
        return Err(format!(
            "text is {} characters; the maximum is {MAX_CHARS}",
            chars.len()
        ));
    }
    if chars.iter().all(|c| c.is_whitespace()) {
        return Err("text is empty: paste some text to tokenize".into());
    }

    let abbrevs = Abbrevs::build(&opts.extra_abbreviations);
    let raw = tokenize(&chars, opts, &abbrevs);
    let groups = segment(&raw, &chars, opts, &abbrevs);

    let mut counts = Counts {
        characters: chars.len(),
        ..Counts::default()
    };
    let mut sentences: Vec<Sentence> = Vec::new();
    for group in groups {
        let span_start = raw[group[0]].start;
        let span_end = raw[*group.last().expect("group is never empty")].end;
        let mut tokens: Vec<Token> = Vec::new();
        for &ti in &group {
            let t = raw[ti];
            if opts.drop_punctuation && matches!(t.kind, TokenType::Punct | TokenType::Symbol) {
                continue;
            }
            let mut text = slice(&chars, t.start, t.end);
            if opts.lowercase {
                text = text.to_lowercase();
            }
            tokens.push(Token {
                index: tokens.len() + 1,
                start: t.start,
                end: t.end,
                kind: t.kind,
                text,
            });
        }
        if tokens.is_empty() {
            continue;
        }
        for t in &tokens {
            counts.tokens += 1;
            match t.kind {
                TokenType::Word => counts.words += 1,
                TokenType::Number => counts.numbers += 1,
                TokenType::Punct => counts.punctuation += 1,
                TokenType::Symbol => counts.symbols += 1,
                TokenType::Url => counts.urls += 1,
                TokenType::Email => counts.emails += 1,
            }
        }
        let mut stext = fold_whitespace(&slice(&chars, span_start, span_end));
        if opts.lowercase {
            stext = stext.to_lowercase();
        }
        sentences.push(Sentence {
            index: sentences.len() + 1,
            start: span_start,
            end: span_end,
            text: stext,
            tokens,
        });
    }

    if sentences.is_empty() {
        return Err(
            "no tokens found: the text has nothing left after the current filters".into(),
        );
    }
    counts.sentences = sentences.len();
    Ok(Analysis { counts, sentences })
}

/// Render an analysis in the requested format.
pub fn render(analysis: &Analysis, format: Format) -> String {
    match format {
        Format::Json => {
            serde_json::to_string_pretty(analysis).unwrap_or_else(|e| format!("{{\"error\":{e}}}"))
        }
        Format::Table => {
            let mut out = String::from("sentence\ttoken\tstart\tend\ttype\ttext");
            for s in &analysis.sentences {
                for t in &s.tokens {
                    out.push('\n');
                    out.push_str(&format!(
                        "{}\t{}\t{}\t{}\t{}\t{}",
                        s.index,
                        t.index,
                        t.start,
                        t.end,
                        t.kind.as_str(),
                        t.text
                    ));
                }
            }
            out
        }
        Format::Lines => analysis
            .sentences
            .iter()
            .flat_map(|s| s.tokens.iter().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n"),
        Format::Spaces => analysis
            .sentences
            .iter()
            .map(|s| {
                s.tokens
                    .iter()
                    .map(|t| t.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Format::Sentences => analysis
            .sentences
            .iter()
            .map(|s| s.text.clone())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// Tokenize, segment and render in one call — the signature every surface uses.
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    format: &str,
    newlines: &str,
    split_contractions: bool,
    split_hyphenated: bool,
    lowercase: bool,
    drop_punctuation: bool,
    extra_abbreviations: &str,
) -> Result<String, String> {
    let format = Format::parse(format)?;
    let opts = Options {
        newlines: Newlines::parse(newlines)?,
        split_contractions,
        split_hyphenated,
        lowercase,
        drop_punctuation,
        extra_abbreviations: parse_abbreviations(extra_abbreviations),
    };
    let analysis = analyze(text, &opts)?;
    Ok(render(&analysis, format))
}

// ---------------------------------------------------------------------------
// Scanner
// ---------------------------------------------------------------------------

fn tokenize(chars: &[char], opts: &Options, abbrevs: &Abbrevs) -> Vec<Raw> {
    let n = chars.len();
    let mut out: Vec<Raw> = Vec::new();
    let mut i = 0usize;
    let mut breaks = 0usize;
    while i < n {
        let c = chars[i];
        if c.is_whitespace() {
            if c == '\n' {
                breaks += 1;
            }
            i += 1;
            continue;
        }
        let (end, kind) = if let Some(e) = scan_url(chars, i) {
            (e, TokenType::Url)
        } else if let Some(e) = scan_email(chars, i) {
            (e, TokenType::Email)
        } else if c.is_alphabetic() || c == '_' {
            (scan_word(chars, i, abbrevs), TokenType::Word)
        } else if c.is_numeric() {
            (scan_number(chars, i), TokenType::Number)
        } else {
            scan_symbolic(chars, i)
        };
        push_pieces(&mut out, chars, i, end, kind, breaks, opts);
        breaks = 0;
        i = end;
    }
    out
}

/// Scan a word: letters plus internal apostrophes, hyphens and the periods of
/// abbreviations (`Dr.`) and dotted acronyms (`U.S.A.`).
fn scan_word(chars: &[char], start: usize, abbrevs: &Abbrevs) -> usize {
    let n = chars.len();
    let mut j = start;
    while j < n && (chars[j].is_alphanumeric() || chars[j] == '_') {
        j += 1;
    }
    loop {
        let Some(&c) = chars.get(j) else { break };
        let next_alnum = chars.get(j + 1).is_some_and(|x| x.is_alphanumeric());
        match c {
            '\'' | '’' if chars.get(j + 1).is_some_and(|x| x.is_alphabetic()) => {
                j += 1;
                while j < n && chars[j].is_alphanumeric() {
                    j += 1;
                }
            }
            '-' | '‑' if next_alnum => {
                j += 1;
                while j < n && chars[j].is_alphanumeric() {
                    j += 1;
                }
            }
            '.' if dot_is_internal(&slice(chars, start, j), abbrevs) => {
                j += 1;
                if chars.get(j).is_some_and(|x| x.is_alphanumeric()) {
                    while j < n && chars[j].is_alphanumeric() {
                        j += 1;
                    }
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    j
}

/// Whether the period after `stem` belongs to the word rather than ending the
/// sentence: a known abbreviation, an initial (`J.`) or an acronym (`U.S.`).
fn dot_is_internal(stem: &str, abbrevs: &Abbrevs) -> bool {
    if stem.is_empty() {
        return false;
    }
    let low = stem.to_lowercase();
    if abbrevs.absorbs_dot(&low) {
        return true;
    }
    let parts: Vec<&str> = low.split('.').filter(|p| !p.is_empty()).collect();
    !parts.is_empty()
        && parts
            .iter()
            .all(|p| p.chars().count() == 1 && p.chars().all(|c| c.is_alphabetic()))
}

/// Scan a number, keeping internal separators (`1,000.00`, `2018-11-11`,
/// `12:30`, `3/4`) and a trailing unit or ordinal suffix (`3rd`, `5km`).
fn scan_number(chars: &[char], start: usize) -> usize {
    let n = chars.len();
    let mut j = start;
    while j < n && chars[j].is_numeric() {
        j += 1;
    }
    loop {
        let Some(&c) = chars.get(j) else { break };
        let next_num = chars.get(j + 1).is_some_and(|x| x.is_numeric());
        if matches!(c, ',' | '.' | ':' | '/' | '-') && next_num {
            j += 2;
            while j < n && chars[j].is_numeric() {
                j += 1;
            }
        } else {
            break;
        }
    }
    while j < n && chars[j].is_alphabetic() {
        j += 1;
    }
    j
}

/// Scan punctuation (collapsing `...`, `!!!`, `--` into one token) or a symbol.
fn scan_symbolic(chars: &[char], start: usize) -> (usize, TokenType) {
    let n = chars.len();
    let c = chars[start];
    if PUNCT_CHARS.contains(&c) {
        let mut j = start;
        if RUN_CHARS.contains(&c) {
            while j < n && chars[j] == c {
                j += 1;
            }
        } else {
            j = start + 1;
        }
        (j, TokenType::Punct)
    } else {
        (start + 1, TokenType::Symbol)
    }
}

fn starts_with_ci(chars: &[char], at: usize, prefix: &str) -> bool {
    let p: Vec<char> = prefix.chars().collect();
    if at + p.len() > chars.len() {
        return false;
    }
    (0..p.len()).all(|k| chars[at + k].to_ascii_lowercase() == p[k])
}

/// Scan a web address so its dots and slashes never split a sentence.
fn scan_url(chars: &[char], start: usize) -> Option<usize> {
    let prefix_len = ["https://", "http://", "ftp://", "www."]
        .iter()
        .find(|p| starts_with_ci(chars, start, p))
        .map(|p| p.chars().count())?;
    let n = chars.len();
    let mut j = start;
    while j < n && !chars[j].is_whitespace() && !matches!(chars[j], '"' | '<' | '>' | '«' | '»') {
        j += 1;
    }
    // A trailing `.`, `,` or `)` is far more likely to be sentence punctuation.
    while j > start
        && matches!(
            chars[j - 1],
            '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '\'' | '…'
        )
    {
        j -= 1;
    }
    if j <= start + prefix_len {
        return None;
    }
    Some(j)
}

/// Scan an e-mail address so its dots never split a sentence.
fn scan_email(chars: &[char], start: usize) -> Option<usize> {
    let n = chars.len();
    if !chars.get(start).is_some_and(|c| c.is_alphanumeric()) {
        return None;
    }
    let mut j = start;
    while j < n && (chars[j].is_alphanumeric() || matches!(chars[j], '.' | '_' | '%' | '+' | '-')) {
        j += 1;
    }
    if chars.get(j) != Some(&'@') || j == start {
        return None;
    }
    let domain_start = j + 1;
    j = domain_start;
    while j < n && (chars[j].is_alphanumeric() || matches!(chars[j], '.' | '-')) {
        j += 1;
    }
    while j > domain_start && matches!(chars[j - 1], '.' | '-') {
        j -= 1;
    }
    let domain = slice(chars, domain_start, j);
    let tld = domain.rsplit('.').next().unwrap_or("");
    if !domain.contains('.') || tld.chars().count() < 2 || !tld.chars().all(|c| c.is_alphabetic()) {
        return None;
    }
    Some(j)
}

/// Emit a scanned span, applying the hyphen / contraction splitting options.
/// Only the first emitted piece carries the preceding line-break count.
fn push_pieces(
    out: &mut Vec<Raw>,
    chars: &[char],
    start: usize,
    end: usize,
    kind: TokenType,
    breaks: usize,
    opts: &Options,
) {
    let mut pieces: Vec<(usize, usize, TokenType)> = Vec::new();
    if kind == TokenType::Word {
        let mut segments: Vec<(usize, usize)> = Vec::new();
        if opts.split_hyphenated {
            let mut s = start;
            for k in start..end {
                if matches!(chars[k], '-' | '‑') {
                    if k > s {
                        segments.push((s, k));
                    }
                    segments.push((k, k + 1));
                    s = k + 1;
                }
            }
            if s < end {
                segments.push((s, end));
            }
        } else {
            segments.push((start, end));
        }
        for (s, e) in segments {
            if e == s + 1 && matches!(chars[s], '-' | '‑') {
                pieces.push((s, e, TokenType::Punct));
                continue;
            }
            match opts.split_contractions.then(|| contraction_split(chars, s, e)) {
                Some(Some(p)) => {
                    pieces.push((s, p, TokenType::Word));
                    pieces.push((p, e, TokenType::Word));
                }
                _ => pieces.push((s, e, TokenType::Word)),
            }
        }
    } else {
        pieces.push((start, end, kind));
    }
    for (idx, (s, e, k)) in pieces.into_iter().enumerate() {
        out.push(Raw {
            start: s,
            end: e,
            kind: k,
            breaks_before: if idx == 0 { breaks } else { 0 },
        });
    }
}

/// Where to cut a contraction, treebank style: `don't` → `do` + `n't`,
/// `Anna's` → `Anna` + `'s`. Returns `None` for `o'clock` and friends.
fn contraction_split(chars: &[char], start: usize, end: usize) -> Option<usize> {
    let positions: Vec<usize> = (start..end)
        .filter(|&k| matches!(chars[k], '\'' | '’'))
        .collect();
    if positions.len() != 1 {
        return None;
    }
    let p = positions[0];
    if p == start || p + 1 >= end {
        return None;
    }
    let suffix = slice(chars, p + 1, end).to_lowercase();
    if suffix == "t" && matches!(chars[p - 1], 'n' | 'N') && p - 1 > start {
        return Some(p - 1);
    }
    if matches!(suffix.as_str(), "s" | "ll" | "re" | "ve" | "m" | "d" | "em") {
        return Some(p);
    }
    None
}

// ---------------------------------------------------------------------------
// Segmenter
// ---------------------------------------------------------------------------

/// Group the token stream into sentences, returning token indices per sentence.
fn segment(toks: &[Raw], chars: &[char], opts: &Options, abbrevs: &Abbrevs) -> Vec<Vec<usize>> {
    let threshold = opts.newlines.threshold();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut i = 0usize;
    while i < toks.len() {
        if !cur.is_empty() && toks[i].breaks_before >= threshold {
            groups.push(std::mem::take(&mut cur));
        }
        cur.push(i);
        let text = slice(chars, toks[i].start, toks[i].end);
        let first = text.chars().next().unwrap_or(' ');
        let mut cut = false;
        if toks[i].kind == TokenType::Punct && TERMINATORS.contains(&first) {
            // A lone period after a short leading number is a list marker
            // ("1. Buy milk"), not the end of a sentence. "Leading" means the
            // number opens the sentence OR opens its own line — the second case
            // is what keeps a multi-line list from breaking at every item when
            // line breaks are not boundaries.
            let enumerator = text == "."
                && i > 0
                && toks[i - 1].kind == TokenType::Number
                && toks[i - 1].end - toks[i - 1].start <= 3
                && (cur.len() == 2 || toks[i - 1].breaks_before >= 1);
            if !enumerator {
                let mut j = i;
                while j + 1 < toks.len() && is_closer(chars, &toks[j + 1]) {
                    j += 1;
                    cur.push(j);
                }
                let continues = toks
                    .get(j + 1)
                    .is_some_and(|nx| continues_sentence(chars, nx));
                cut = !continues;
                i = j;
            }
        } else if toks[i].kind == TokenType::Word && text.ends_with('.') {
            cut = word_dot_boundary(&text, toks.get(i + 1), chars, abbrevs);
        }
        if cut {
            groups.push(std::mem::take(&mut cur));
        }
        i += 1;
    }
    if !cur.is_empty() {
        groups.push(cur);
    }
    groups
}

fn is_closer(chars: &[char], t: &Raw) -> bool {
    t.kind == TokenType::Punct
        && t.end - t.start == 1
        && CLOSERS.contains(&chars[t.start])
}

/// After a terminator, a lowercase word or a comma means the sentence goes on
/// ("the U.S. dollar", `"Stop!" he said.`).
fn continues_sentence(chars: &[char], t: &Raw) -> bool {
    let c = chars[t.start];
    if c.is_alphabetic() && c.is_lowercase() {
        return true;
    }
    t.kind == TokenType::Punct && matches!(c, ',' | ';' | ':')
}

/// A capital letter, a digit or an opening quote starts a new sentence.
fn starts_sentence(chars: &[char], t: &Raw) -> bool {
    let c = chars[t.start];
    c.is_uppercase() || c.is_numeric() || OPENERS.contains(&c)
}

/// Decide whether a word token that ends in a period ends the sentence.
fn word_dot_boundary(text: &str, next: Option<&Raw>, chars: &[char], abbrevs: &Abbrevs) -> bool {
    let stem = text.trim_end_matches('.').to_lowercase();
    if stem.is_empty() || abbrevs.is_never(&stem) {
        return false;
    }
    if abbrevs.is_numeric(&stem) && next.is_some_and(|n| n.kind == TokenType::Number) {
        return false;
    }
    // A single initial ("J. R. R. Tolkien") never ends a sentence.
    let parts: Vec<&str> = stem.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() == 1 && parts[0].chars().count() == 1 {
        return false;
    }
    match next {
        None => true,
        Some(n) => starts_sentence(chars, n),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn slice(chars: &[char], start: usize, end: usize) -> String {
    chars[start..end].iter().collect()
}

/// Collapse runs of whitespace (including line breaks) into single spaces so a
/// sentence stays on one line. Offsets always refer to the untouched source.
fn fold_whitespace(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(text: &str, opts: &Options) -> Vec<(String, &'static str, usize, usize)> {
        analyze(text, opts)
            .unwrap()
            .sentences
            .into_iter()
            .flat_map(|s| s.tokens)
            .map(|t| (t.text, t.kind.as_str(), t.start, t.end))
            .collect()
    }

    #[test]
    fn splits_sentences_and_tokens_with_offsets() {
        let a = analyze("Dr. Green paid $99.99. It works.", &Options::default()).unwrap();
        assert_eq!(a.sentences.len(), 2);
        assert_eq!(a.sentences[0].text, "Dr. Green paid $99.99.");
        assert_eq!((a.sentences[0].start, a.sentences[0].end), (0, 22));
        assert_eq!((a.sentences[1].start, a.sentences[1].end), (23, 32));
        let first: Vec<(&str, &str, usize, usize)> = a.sentences[0]
            .tokens
            .iter()
            .map(|t| (t.text.as_str(), t.kind.as_str(), t.start, t.end))
            .collect();
        assert_eq!(
            first,
            vec![
                ("Dr.", "word", 0, 3),
                ("Green", "word", 4, 9),
                ("paid", "word", 10, 14),
                ("$", "symbol", 15, 16),
                ("99.99", "number", 16, 21),
                (".", "punct", 21, 22),
            ]
        );
        // Offsets index the original text exactly.
        let src: Vec<char> = "Dr. Green paid $99.99. It works.".chars().collect();
        assert_eq!(slice(&src, 16, 21), "99.99");
    }

    #[test]
    fn keeps_abbreviations_initials_decimals_and_list_markers_together() {
        let a = analyze(
            "J. R. R. Tolkien met Mrs. Hall, e.g. on Mar. 3. Release 1.2.3 shipped.",
            &Options::default(),
        )
        .unwrap();
        assert_eq!(a.counts.sentences, 2);
        assert!(a.sentences[0].text.starts_with("J. R. R. Tolkien"));
        assert_eq!(a.sentences[1].text, "Release 1.2.3 shipped.");

        let a = analyze("1. Buy milk\n2. Walk the dog", &Options::default()).unwrap();
        assert_eq!(a.counts.sentences, 1, "paragraph mode joins the two lines");
        let a = analyze(
            "1. Buy milk\n2. Walk the dog",
            &Options {
                newlines: Newlines::Always,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(a.sentences.len(), 2);
        assert_eq!(a.sentences[0].text, "1. Buy milk");
    }

    #[test]
    fn splits_contractions_treebank_style_and_can_keep_them() {
        let t = tokens("They don't like Anna's cat.", &Options::default());
        let texts: Vec<&str> = t.iter().map(|(s, _, _, _)| s.as_str()).collect();
        assert_eq!(
            texts,
            vec!["They", "do", "n't", "like", "Anna", "'s", "cat", "."]
        );
        // Split pieces keep exact source offsets.
        assert_eq!(t[1].2..t[1].3, 5..7);
        assert_eq!(t[2].2..t[2].3, 7..10);

        let kept = tokens(
            "They don't like Anna's cat.",
            &Options {
                split_contractions: false,
                ..Options::default()
            },
        );
        let texts: Vec<&str> = kept.iter().map(|(s, _, _, _)| s.as_str()).collect();
        assert_eq!(texts, vec!["They", "don't", "like", "Anna's", "cat", "."]);
    }

    #[test]
    fn hyphenated_words_are_one_token_unless_split_is_asked_for() {
        let joined = tokens("A state-of-the-art design.", &Options::default());
        assert_eq!(joined[1].0, "state-of-the-art");
        assert_eq!(joined[1].1, "word");

        let split = tokens(
            "A state-of-the-art design.",
            &Options {
                split_hyphenated: true,
                ..Options::default()
            },
        );
        let texts: Vec<&str> = split.iter().map(|(s, _, _, _)| s.as_str()).collect();
        assert_eq!(
            texts,
            vec!["A", "state", "-", "of", "-", "the", "-", "art", "design", "."]
        );
    }

    #[test]
    fn keeps_numbers_urls_and_emails_intact() {
        let t = tokens(
            "Order 1,000.00 on 2018-11-11 via https://example.com/a.b or mail ops@example.com.",
            &Options::default(),
        );
        let by_type: Vec<(&str, &str)> = t
            .iter()
            .map(|(s, k, _, _)| (s.as_str(), *k))
            .filter(|(_, k)| matches!(*k, "number" | "url" | "email"))
            .collect();
        assert_eq!(
            by_type,
            vec![
                ("1,000.00", "number"),
                ("2018-11-11", "number"),
                ("https://example.com/a.b", "url"),
                ("ops@example.com", "email"),
            ]
        );
    }

    #[test]
    fn quotes_and_ellipses_stay_with_their_sentence() {
        let a = analyze("\"Stop!\" he said. Wait... really?", &Options::default()).unwrap();
        assert_eq!(a.sentences.len(), 2);
        assert_eq!(a.sentences[0].text, "\"Stop!\" he said.");
        let ell: Vec<&str> = a.sentences[1]
            .tokens
            .iter()
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(ell, vec!["Wait", "...", "really", "?"]);
    }

    #[test]
    fn newline_modes_change_boundaries() {
        let text = "first line\nsecond line\n\nnew paragraph";
        let count = |nl| analyze(text, &Options { newlines: nl, ..Options::default() }).unwrap().counts.sentences;
        assert_eq!(count(Newlines::Paragraph), 2);
        assert_eq!(count(Newlines::Always), 3);
        assert_eq!(count(Newlines::Never), 1);
    }

    #[test]
    fn extra_abbreviations_and_filters_apply() {
        let default_run = analyze("Ship it to Acme Corp. Then invoice.", &Options::default()).unwrap();
        assert_eq!(default_run.counts.sentences, 1, "corp. is built in");

        let a = analyze(
            "Ship it to Acme Blarg. Then invoice.",
            &Options {
                extra_abbreviations: parse_abbreviations("Blarg."),
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(a.counts.sentences, 1);

        let clean = analyze(
            "Hello, World! Bye.",
            &Options {
                drop_punctuation: true,
                lowercase: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(render(&clean, Format::Lines), "hello\nworld\nbye");
        assert_eq!(clean.counts.punctuation, 0);
        assert_eq!(clean.counts.words, 3);
    }

    #[test]
    fn renders_every_format() {
        let a = analyze("Hi there. Bye!", &Options::default()).unwrap();
        assert_eq!(render(&a, Format::Lines), "Hi\nthere\n.\nBye\n!");
        assert_eq!(render(&a, Format::Spaces), "Hi there .\nBye !");
        assert_eq!(render(&a, Format::Sentences), "Hi there.\nBye!");
        assert_eq!(
            render(&a, Format::Table),
            "sentence\ttoken\tstart\tend\ttype\ttext\n\
             1\t1\t0\t2\tword\tHi\n\
             1\t2\t3\t8\tword\tthere\n\
             1\t3\t8\t9\tpunct\t.\n\
             2\t1\t10\t13\tword\tBye\n\
             2\t2\t13\t14\tpunct\t!"
        );
        let json = render(&a, Format::Json);
        assert!(json.contains("\"sentences\": 2"));
        assert!(json.contains("\"type\": \"word\""));
    }

    #[test]
    fn parse_rejects_unknown_choices() {
        assert!(Format::parse("yaml").unwrap_err().contains("invalid format"));
        assert!(Newlines::parse("sometimes")
            .unwrap_err()
            .contains("invalid newlines"));
    }

    #[test]
    fn empty_and_oversized_input_are_errors() {
        let err = analyze("   \n  ", &Options::default()).unwrap_err();
        assert!(err.contains("text is empty"), "{err}");

        let big = "a ".repeat(MAX_CHARS);
        let err = analyze(&big, &Options::default()).unwrap_err();
        assert!(err.contains("the maximum is 500000"), "{err}");
    }

    #[test]
    fn filtering_everything_away_is_an_error() {
        let err = run("...", "lines", "paragraph", true, false, false, true, "").unwrap_err();
        assert!(err.contains("no tokens found"), "{err}");
    }

    #[test]
    fn run_wires_the_options_together() {
        let out = run(
            "Dr. Green paid $99.99. It works.",
            "sentences",
            "paragraph",
            true,
            false,
            false,
            false,
            "",
        )
        .unwrap();
        assert_eq!(out, "Dr. Green paid $99.99.\nIt works.");
    }

    #[test]
    fn full_width_terminators_and_cjk() {
        let a = analyze("这是第一句。这是第二句！", &Options::default()).unwrap();
        assert_eq!(a.counts.sentences, 2);
        assert_eq!(a.sentences[0].text, "这是第一句。");
    }
}
