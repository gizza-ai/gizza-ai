//! ocr-text-cleaner core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Fixes the *mechanical* errors OCR engines produce, deterministically and with
//! no dictionary or model:
//!   1. **ligatures** — expand `ﬁ ﬂ ﬀ ﬃ ﬄ ﬅ ﬆ` to `fi fl ff ffi ffl ft st`.
//!   2. **dehyphenation** — rejoin a word split by a hyphen at a line break
//!      (`hyphen-\nation` → `hyphenation`).
//!   3. **line breaks** — leave them, join soft breaks inside paragraphs, or
//!      collapse every break into one line.
//!   4. **confusables** — fix letter/number confusion by word vs. number context
//!      (`HeIIo` → `Hello`, `l00` → `100`, `| think` → `I think`).
//!   5. **rn→m** — the classic OCR merge error (OPT-IN; aggressive, no dictionary).
//!   6. **spacing** — collapse repeated spaces, strip spaces around punctuation,
//!      insert missing spaces after sentence/clause punctuation, trim line ends.
//!
//! Everything else — accents, CJK, emoji, real words — is preserved.

/// Maximum accepted input size, in bytes. This is a single-pass, browser-local
/// cleaner with no streaming; a generous cap keeps it responsive and bounds
/// memory. Larger jobs should be chunked upstream.
pub const MAX_INPUT_BYTES: usize = 200_000;

/// Entry point shared by the chat block, the CLI, and the web page. Validates
/// the input size, then runs [`clean`] with the requested options. `line_breaks`
/// is a lenient option string (see [`LineBreaks::parse`]).
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    fix_ligatures: bool,
    join_hyphenated: bool,
    line_breaks: &str,
    fix_confusables: bool,
    fix_rn: bool,
    fix_spacing: bool,
) -> Result<String, String> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is too large ({} bytes); maximum is {MAX_INPUT_BYTES} bytes",
            text.len()
        ));
    }
    let opts = Options {
        fix_ligatures,
        join_hyphenated,
        line_breaks: LineBreaks::parse(line_breaks),
        fix_confusables,
        fix_rn,
        fix_spacing,
    };
    Ok(clean(text, opts))
}

/// How embedded line breaks are handled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineBreaks {
    /// Leave line breaks exactly as they are (only `\r\n`/`\r` → `\n`). Default.
    Keep,
    /// Join soft (single) line breaks inside a paragraph into a space, but keep
    /// blank-line paragraph separators. The typical "reflow OCR columns" fix.
    Paragraphs,
    /// Collapse every line break into a single space (one flowing line).
    All,
}

impl LineBreaks {
    /// Parse the option string (blank / unknown → `Keep`).
    pub fn parse(s: &str) -> LineBreaks {
        match s.trim().to_ascii_lowercase().as_str() {
            "paragraphs" | "paragraph" | "reflow" => LineBreaks::Paragraphs,
            "all" | "join" | "single-line" => LineBreaks::All,
            // "keep", "off", "", and anything else → keep.
            _ => LineBreaks::Keep,
        }
    }
}

/// Options controlling which fixes run. All default to the sensible OCR-cleanup
/// preset except `fix_rn`, which is aggressive and opt-in.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub fix_ligatures: bool,
    pub join_hyphenated: bool,
    pub line_breaks: LineBreaks,
    pub fix_confusables: bool,
    pub fix_rn: bool,
    pub fix_spacing: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            fix_ligatures: true,
            join_hyphenated: true,
            line_breaks: LineBreaks::Keep,
            fix_confusables: true,
            fix_rn: false,
            fix_spacing: true,
        }
    }
}

/// Clean OCR `text` according to `opts`, running each enabled step in a fixed order.
pub fn clean(text: &str, opts: Options) -> String {
    // 0. Normalize line endings so later steps only deal with '\n'.
    let mut s = text.replace("\r\n", "\n").replace('\r', "\n");

    // 1. Ligatures (character-level).
    if opts.fix_ligatures {
        s = expand_ligatures(&s);
    }

    // 2. Dehyphenate words split across a line break (before line breaks are merged).
    if opts.join_hyphenated {
        s = join_hyphenated(&s);
    }

    // 3. Line-break handling.
    s = match opts.line_breaks {
        LineBreaks::Keep => s,
        LineBreaks::Paragraphs => merge_paragraph_breaks(&s),
        LineBreaks::All => merge_all_breaks(&s),
    };

    // 4. Confusables (token-by-token, letter vs. number context).
    if opts.fix_confusables {
        s = fix_confusables(&s);
    }

    // 5. rn -> m (aggressive, opt-in).
    if opts.fix_rn {
        s = s.replace("rn", "m").replace("RN", "M");
    }

    // 6. Spacing normalization (last — tidies anything introduced above).
    if opts.fix_spacing {
        s = fix_spacing(&s);
    }

    s
}

/// Expand the common typographic ligatures OCR/PDF text carries.
fn expand_ligatures(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\u{FB00}' => out.push_str("ff"),
            '\u{FB01}' => out.push_str("fi"),
            '\u{FB02}' => out.push_str("fl"),
            '\u{FB03}' => out.push_str("ffi"),
            '\u{FB04}' => out.push_str("ffl"),
            '\u{FB05}' => out.push_str("ft"), // long s-t
            '\u{FB06}' => out.push_str("st"),
            _ => out.push(ch),
        }
    }
    out
}

/// Rejoin a word broken by a hyphen at the end of a line: `exam-\nple` → `example`.
/// If the continuation starts with an uppercase letter it is treated as a real
/// hyphenated compound (`New-\nYork` → `New-York`) — the hyphen is kept but the
/// line break is removed.
fn join_hyphenated(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        // Look for: <letter> ('-'|'‐') <spaces>* '\n' <spaces>* <letter>
        if (ch == '-' || ch == '\u{2010}') && i > 0 && chars[i - 1].is_alphabetic() {
            let mut j = i + 1;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            if j < chars.len() && chars[j] == '\n' {
                let mut k = j + 1;
                while k < chars.len() && (chars[k] == ' ' || chars[k] == '\t') {
                    k += 1;
                }
                if k < chars.len() && chars[k].is_alphabetic() {
                    // Join. Keep the hyphen only if the continuation is uppercase.
                    if chars[k].is_uppercase() {
                        out.push('-');
                    }
                    i = k; // continue from the continuation letter
                    continue;
                }
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// Join soft line breaks inside a paragraph to a space, preserving blank-line
/// paragraph separators.
fn merge_paragraph_breaks(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (idx, para) in split_paragraphs(s).iter().enumerate() {
        if idx > 0 {
            out.push_str("\n\n");
        }
        let joined = para
            .split('\n')
            .map(|l| l.trim_end())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&joined);
    }
    out
}

/// Split on blank lines (empty / whitespace-only) into paragraphs.
fn split_paragraphs(s: &str) -> Vec<String> {
    let mut paras: Vec<String> = Vec::new();
    let mut cur = String::new();
    for line in s.split('\n') {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                paras.push(std::mem::take(&mut cur));
            }
        } else {
            if !cur.is_empty() {
                cur.push('\n');
            }
            cur.push_str(line);
        }
    }
    if !cur.is_empty() {
        paras.push(cur);
    }
    paras
}

/// Collapse every line break into a single space.
fn merge_all_breaks(s: &str) -> String {
    s.split('\n')
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Fix letter/number confusables token by token, using each token's own context
/// (a word vs. a number) to decide direction.
fn fix_confusables(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut token = String::new();
    for ch in s.chars() {
        if is_token_char(ch) {
            token.push(ch);
        } else {
            if !token.is_empty() {
                out.push_str(&fix_token(&token));
                token.clear();
            }
            out.push(ch);
        }
    }
    if !token.is_empty() {
        out.push_str(&fix_token(&token));
    }
    out
}

/// A token for confusable analysis: letters, digits, and the pipe (a common
/// OCR stand-in for `I`/`l`/`1`).
fn is_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '|'
}

/// Is `c` a letter that OCR routinely confuses with a digit?
fn is_confusable_letter(c: char) -> bool {
    matches!(c, 'l' | 'I' | 'O' | 'o')
}

/// Fix confusables within a single token.
fn fix_token(tok: &str) -> String {
    let chars: Vec<char> = tok.chars().collect();
    let letters = chars.iter().filter(|c| c.is_ascii_alphabetic()).count();
    let digits = chars.iter().filter(|c| c.is_ascii_digit()).count();
    // "Hard" letters are unambiguous — a token whose only letters are
    // digit-confusable (l/I/O/o) alongside digits is really a number (l00 → 100).
    let hard_letters = chars
        .iter()
        .filter(|c| c.is_ascii_alphabetic() && !is_confusable_letter(**c))
        .count();

    // Pure pipe run (e.g. a stray "|") with no letters/digits → treat as "I".
    if letters == 0 && digits == 0 {
        return tok.replace('|', "I");
    }

    if hard_letters > 0 || digits == 0 {
        // WORD context.
        let all_upper = chars
            .iter()
            .filter(|c| c.is_ascii_alphabetic())
            .all(|c| c.is_ascii_uppercase());
        let mut out = String::with_capacity(tok.len());
        for (idx, &c) in chars.iter().enumerate() {
            let prev_letter = idx > 0 && chars[idx - 1].is_ascii_alphabetic();
            let next_letter = idx + 1 < chars.len() && chars[idx + 1].is_ascii_alphabetic();
            let replacement = match c {
                // Digit-in-word only when flanked by letters on BOTH sides — this
                // protects alphanumeric codes (COVID19, MP3, B12) at word edges.
                '0' if prev_letter && next_letter => Some(if all_upper { 'O' } else { 'o' }),
                '1' if prev_letter && next_letter => Some(if all_upper { 'I' } else { 'l' }),
                // A pipe inside a word → the letter it was misread from.
                '|' => Some(if all_upper { 'I' } else { 'l' }),
                // A word-interior uppercase I in a mixed-case word is almost always
                // a misread lowercase l (HeIIo → Hello). Leaving it strictly
                // interior spares the pronoun "I", capitalized starts (Italy), and
                // all-caps acronyms/Roman numerals (IBM, III).
                'I' if !all_upper && idx > 0 && idx + 1 < chars.len() => Some('l'),
                _ => None,
            };
            out.push(replacement.unwrap_or(c));
        }
        out
    } else {
        // NUMBER context (more digits than letters).
        let mut out = String::with_capacity(tok.len());
        for &c in &chars {
            let replacement = match c {
                'O' | 'o' => Some('0'),
                'l' | 'I' | '|' => Some('1'),
                _ => None,
            };
            out.push(replacement.unwrap_or(c));
        }
        out
    }
}

/// Normalize spacing: collapse repeated spaces/tabs, strip spaces around
/// punctuation, insert missing spaces after clause/sentence punctuation, and
/// trim trailing whitespace on each line.
fn fix_spacing(s: &str) -> String {
    s.split('\n')
        .map(fix_spacing_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn fix_spacing_line(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());

    for (idx, &c) in chars.iter().enumerate() {
        if c == ' ' || c == '\t' {
            // Collapse runs of spaces/tabs.
            if matches!(out.last(), Some(' ')) {
                continue;
            }
            // Drop a space that sits before closing punctuation.
            if let Some(&n) = chars[idx + 1..].iter().find(|&&x| x != ' ' && x != '\t') {
                if matches!(n, ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '%') {
                    continue;
                }
            }
            // Drop a space that sits right after opening punctuation.
            if matches!(out.last(), Some('(') | Some('[') | Some('{')) {
                continue;
            }
            out.push(' ');
        } else {
            // Capture the previously emitted char BEFORE pushing — earlier steps
            // may have dropped a space, so the original `chars[idx-1]` is stale.
            let prev_out = out.last().copied();
            out.push(c);
            // Insert a missing space after punctuation when the next char is glued
            // on and the guards confirm a real boundary.
            if idx + 1 < chars.len() {
                let n = chars[idx + 1];
                if n != ' ' && n != '\t' && needs_space_after(prev_out, c, n) {
                    out.push(' ');
                }
            }
        }
    }

    // Trim trailing whitespace.
    while matches!(out.last(), Some(' ') | Some('\t')) {
        out.pop();
    }
    out.into_iter().collect()
}

/// Should a space be inserted after punctuation `c`, given the char actually
/// emitted before it (`prev`) and the upcoming char (`next`)?
fn needs_space_after(prev: Option<char>, c: char, next: char) -> bool {
    match c {
        // Sentence end: `word.Next` → `word. Next`. Guard against decimals
        // (3.14), abbreviations/initials (U.S.A), and ellipses (...).
        '.' | '!' | '?' => {
            prev.map(|p| p.is_ascii_lowercase()).unwrap_or(false) && next.is_ascii_uppercase()
        }
        // Clause punctuation: `Thanks,next` → `Thanks, next`. Guard against
        // thousands separators / times (3,000 / 12:30).
        ',' | ';' => {
            prev.map(|p| p.is_ascii_alphabetic()).unwrap_or(false) && next.is_ascii_alphabetic()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Options {
        Options {
            fix_ligatures: true,
            join_hyphenated: true,
            line_breaks: LineBreaks::Paragraphs,
            fix_confusables: true,
            fix_rn: false,
            fix_spacing: true,
        }
    }

    #[test]
    fn happy_full_pipeline() {
        // ligature + confusable + spacing + dehyphenation in one pass.
        let input = "The ﬁnal  report says HeIIo ,world .Next exam-\nple here.";
        let got = clean(input, all());
        assert_eq!(got, "The final report says Hello, world. Next example here.");
    }

    #[test]
    fn ligatures_expand() {
        let o = Options { line_breaks: LineBreaks::Keep, ..Default::default() };
        assert_eq!(clean("ofﬁce ﬂour ﬀ ﬃx ﬄy", o), "office flour ff ffix ffly");
    }

    #[test]
    fn confusable_word_and_number_context() {
        let o = Options { fix_spacing: false, ..Default::default() };
        assert_eq!(clean("HeIIo l00 items", o), "Hello 100 items");
        // pipe as the pronoun I, and l0 as 10 in number context
        assert_eq!(clean("| saw l0 cats", o), "I saw 10 cats");
    }

    #[test]
    fn confusable_preserves_codes_and_pronoun() {
        let o = Options { fix_spacing: false, line_breaks: LineBreaks::Keep, ..Default::default() };
        // digit flanked by a digit is not "in-word" → codes survive.
        assert_eq!(clean("COVID19 MP3 H2O", o), "COVID19 MP3 H2O");
        // the pronoun I and acronyms stay.
        assert_eq!(clean("I like IBM", o), "I like IBM");
    }

    #[test]
    fn dehyphenation_joins_and_keeps_compounds() {
        let o = Options {
            line_breaks: LineBreaks::Keep,
            fix_spacing: false,
            fix_confusables: false,
            ..Default::default()
        };
        assert_eq!(clean("hyphen-\nation", o), "hyphenation");
        // uppercase continuation → real compound, hyphen kept, break removed.
        assert_eq!(clean("New-\nYork", o), "New-York");
    }

    #[test]
    fn line_breaks_paragraphs_vs_all() {
        let text = "line one\nline two\n\nnew para";
        let para = Options {
            fix_confusables: false,
            fix_spacing: false,
            join_hyphenated: false,
            fix_ligatures: false,
            fix_rn: false,
            line_breaks: LineBreaks::Paragraphs,
        };
        assert_eq!(clean(text, para), "line one line two\n\nnew para");
        let joinall = Options { line_breaks: LineBreaks::All, ..para };
        assert_eq!(clean(text, joinall), "line one line two new para");
        let keep = Options { line_breaks: LineBreaks::Keep, ..para };
        assert_eq!(clean(text, keep), text);
    }

    #[test]
    fn spacing_rules() {
        let o = Options {
            fix_confusables: false,
            fix_ligatures: false,
            join_hyphenated: false,
            fix_rn: false,
            line_breaks: LineBreaks::Keep,
            fix_spacing: true,
        };
        assert_eq!(clean("word ,  next .Done", o), "word, next. Done");
        assert_eq!(clean("Thanks,next please", o), "Thanks, next please");
        // decimals, thousands, times, urls are NOT touched.
        assert_eq!(
            clean("3.14 and 3,000 at 12:30 see a.com", o),
            "3.14 and 3,000 at 12:30 see a.com"
        );
        // trailing spaces trimmed.
        assert_eq!(clean("trail   ", o), "trail");
    }

    #[test]
    fn fix_rn_is_opt_in() {
        let off = Options {
            fix_rn: false,
            line_breaks: LineBreaks::Keep,
            fix_confusables: false,
            fix_spacing: false,
            ..Default::default()
        };
        assert_eq!(clean("modem", off), "modem");
        let on = Options { fix_rn: true, ..off };
        // aggressive: rn -> m everywhere (documented).
        assert_eq!(clean("modern rnodem", on), "modem modem");
    }

    #[test]
    fn empty_input() {
        assert_eq!(clean("", all()), "");
    }

    #[test]
    fn unicode_preserved() {
        let o = Options { line_breaks: LineBreaks::Keep, ..Default::default() };
        assert_eq!(clean("café 北京 🚀", o), "café 北京 🚀");
    }

    #[test]
    fn run_happy_wires_options() {
        // ligature + spacing + confusable through the public entry point.
        let out = run(
            "The ﬁnal  report says HeIIo .Next",
            true,
            true,
            "keep",
            true,
            false,
            true,
        )
        .unwrap();
        assert_eq!(out, "The final report says Hello. Next");
    }

    #[test]
    fn run_reflow_and_rn_opt_in() {
        // paragraphs reflow + opt-in rn->m.
        let out = run("colu-\nrnn one\ncolurnn two", false, true, "paragraphs", false, true, false)
            .unwrap();
        assert_eq!(out, "column one column two");
    }

    #[test]
    fn run_rejects_oversize_input() {
        let big = "a".repeat(MAX_INPUT_BYTES + 1);
        let err = run(&big, true, true, "keep", true, false, true).unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn run_accepts_input_at_the_limit() {
        let at = "a".repeat(MAX_INPUT_BYTES);
        assert!(run(&at, false, false, "keep", false, false, false).is_ok());
    }

    #[test]
    fn linebreaks_parse() {
        assert_eq!(LineBreaks::parse("paragraphs"), LineBreaks::Paragraphs);
        assert_eq!(LineBreaks::parse("ALL"), LineBreaks::All);
        assert_eq!(LineBreaks::parse(""), LineBreaks::Keep);
        assert_eq!(LineBreaks::parse("garbage"), LineBreaks::Keep);
    }
}
