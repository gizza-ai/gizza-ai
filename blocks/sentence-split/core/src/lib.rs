//! sentence-split core — segment plain text into individual sentences with a
//! deterministic, rule-based English boundary detector. No wafer/wasm-bindgen
//! deps; shared by the chat skill block, the CLI and the web page.
//!
//! The detector is intentionally rule-based (no model, no training data) so the
//! same input always produces the same output on every surface. It handles the
//! cases a naive "split on `.`" gets wrong: titles and abbreviations (`Dr.`,
//! `e.g.`), initials (`J. R. R.`), decimals and version numbers (`3.14`,
//! `1.2.3`), reference prefixes (`No. 5`), list enumerators (`1. Buy milk`),
//! ellipses, terminators followed by closing quotes/brackets, and line breaks.

use serde::Serialize;

/// Largest input accepted, in Unicode characters. Well above any paste a
/// browser text box handles comfortably; keeps the 64 MiB wasm sandbox safe.
pub const MAX_CHARS: usize = 500_000;

/// Largest accepted `min_chars` filter value.
pub const MAX_MIN_CHARS: usize = 10_000;

/// Abbreviations and titles that never end a sentence, whatever follows.
/// Stored lowercase, without the trailing period. Dotted forms (`e.g`) are
/// matched against the same dotted token the scanner reads back from the text.
const NEVER_BOUNDARY: &[&str] = &[
    // Personal / professional titles.
    "mr", "mrs", "ms", "mx", "dr", "prof", "sr", "jr", "st", "mt", "ft", "rev", "hon", "gen",
    "col", "capt", "lt", "sgt", "maj", "adm", "cmdr", "cpl", "pvt", "gov", "pres", "supt", "atty",
    "messrs", "mme", "mlle", "fr", "br", "rep", "sen", "insp", "det", "ofc",
    // Inline Latin connectives — these introduce a continuation, never a new
    // sentence, even when the next word is capitalised ("e.g. Apples").
    "e.g", "i.e", "viz", "cf", "c.f", "vs", "v.s", "al", "et", "ca",
];

/// Abbreviations that only suppress a break when a number follows, so
/// "No. 5" stays joined while "The answer is no. Then we left." still splits.
const NUMBER_PREFIXES: &[&str] = &[
    "no", "nos", "fig", "figs", "vol", "vols", "pp", "ch", "chap", "sec", "art", "ref", "eq",
    "eqs", "para", "pt", "ver", "iss", "jan", "feb", "mar", "apr", "jun", "jul", "aug", "sep",
    "sept", "oct", "nov", "dec",
];

/// How a line break is treated.
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

    fn breaks_on(self, newline_count: usize) -> bool {
        match self {
            Newlines::Paragraph => newline_count >= 2,
            Newlines::Never => false,
            Newlines::Always => newline_count >= 1,
        }
    }
}

/// How the sentence list is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// One sentence per line. Default.
    Lines,
    /// One sentence per line, prefixed `1. `, `2. `, …
    Numbered,
    /// Sentences separated by a blank line.
    BlankLine,
    /// `{ "count": N, "sentences": [{ index, text, words, characters }] }`.
    Json,
}

impl Format {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "lines" => Ok(Format::Lines),
            "numbered" => Ok(Format::Numbered),
            "blank-line" => Ok(Format::BlankLine),
            "json" => Ok(Format::Json),
            other => Err(format!(
                "invalid format {other:?}: expected \"lines\", \"numbered\", \"blank-line\", or \"json\""
            )),
        }
    }
}

/// One detected sentence plus its counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sentence {
    /// 1-based position in the input.
    pub index: usize,
    pub text: String,
    /// Whitespace-separated word count.
    pub words: usize,
    /// Length in Unicode characters.
    pub characters: usize,
}

fn is_terminator(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '…' | '。' | '！' | '？' | '‽')
}

/// Full-width / CJK terminators end a sentence with no following space.
fn is_hard_terminator(c: char) -> bool {
    matches!(c, '。' | '！' | '？')
}

/// Closing punctuation that belongs to the sentence it ends.
fn is_closer(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | '’' | '”' | '»' | '›' | ')' | ']' | '}' | '」' | '』'
    )
}

/// Parse the user-supplied extra abbreviation list. Entries may be separated by
/// commas, semicolons or whitespace and may carry a trailing period.
fn parse_extra(extra: &str) -> Vec<String> {
    extra
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(|t| t.trim().trim_end_matches('.').to_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

/// The alphanumeric/dotted token immediately before `end`, plus its start index.
fn token_before(chars: &[char], end: usize, sentence_start: usize) -> (String, usize) {
    let mut s = end;
    while s > sentence_start && (chars[s - 1].is_alphanumeric() || chars[s - 1] == '.') {
        s -= 1;
    }
    (chars[s..end].iter().collect(), s)
}

/// Segment `text` into sentences.
///
/// - `newlines`: `"paragraph"` (default — a blank line always ends a sentence),
///   `"never"` (line breaks are plain whitespace) or `"always"` (every line
///   break ends a sentence).
/// - `trim`: trim each sentence and fold a line break inside a sentence to a
///   single space. When off, the original whitespace is preserved verbatim.
/// - `min_chars`: drop sentences shorter than this many characters (0 = keep all).
/// - `extra_abbreviations`: additional abbreviations that never end a sentence.
///
/// Returns `Err` when the input is empty, over [`MAX_CHARS`], when `min_chars`
/// is over [`MAX_MIN_CHARS`], on an invalid `newlines` value, or when the filter
/// leaves nothing behind.
pub fn split_text(
    text: &str,
    newlines: &str,
    trim: bool,
    min_chars: usize,
    extra_abbreviations: &str,
) -> Result<Vec<Sentence>, String> {
    let mode = Newlines::parse(newlines)?;
    if min_chars > MAX_MIN_CHARS {
        return Err(format!(
            "min_chars is {min_chars}: the maximum is {MAX_MIN_CHARS}"
        ));
    }
    if text.trim().is_empty() {
        return Err("text is empty: paste the text you want to split into sentences".to_string());
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() > MAX_CHARS {
        return Err(format!(
            "text is {} characters: the maximum is {MAX_CHARS}",
            chars.len()
        ));
    }
    let extra = parse_extra(extra_abbreviations);

    let n = chars.len();
    let mut raw: Vec<String> = Vec::new();
    let mut buf = String::new();
    let mut sentence_start = 0usize;
    let mut i = 0usize;

    while i < n {
        let ch = chars[i];

        // --- whitespace run: may itself be a sentence break -----------------
        if ch.is_whitespace() {
            let mut j = i;
            let mut newline_count = 0usize;
            while j < n && chars[j].is_whitespace() {
                if chars[j] == '\n' {
                    newline_count += 1;
                }
                j += 1;
            }
            if mode.breaks_on(newline_count) {
                raw.push(std::mem::take(&mut buf));
                sentence_start = j;
            } else if newline_count > 0 && trim {
                if !buf.trim().is_empty() {
                    buf.push(' ');
                }
            } else {
                buf.extend(&chars[i..j]);
            }
            i = j;
            continue;
        }

        // --- terminator run --------------------------------------------------
        if is_terminator(ch) {
            let mut j = i;
            let mut hard = false;
            while j < n && is_terminator(chars[j]) {
                hard |= is_hard_terminator(chars[j]);
                j += 1;
            }
            let single_period = j == i + 1 && ch == '.';
            let mut k = j;
            while k < n && is_closer(chars[k]) {
                k += 1;
            }
            buf.extend(&chars[i..k]);

            // Look ahead past whitespace to the next visible character.
            let mut m = k;
            let mut newline_count = 0usize;
            while m < n && chars[m].is_whitespace() {
                if chars[m] == '\n' {
                    newline_count += 1;
                }
                m += 1;
            }
            let forced = mode.breaks_on(newline_count);
            let boundary = if forced || m >= n || hard {
                true
            } else if m == k {
                // Nothing separates the terminator from the next character:
                // "3.14", "example.com", "10:30a.m"-style runs stay joined.
                false
            } else {
                is_boundary(&chars, i, m, single_period, sentence_start, &extra)
            };

            if boundary {
                raw.push(std::mem::take(&mut buf));
                sentence_start = m;
                i = m;
            } else {
                // Leave the whitespace to the whitespace branch so folding
                // rules live in exactly one place.
                i = k;
            }
            continue;
        }

        buf.push(ch);
        i += 1;
    }
    raw.push(buf);

    let mut sentences = Vec::new();
    for s in raw {
        let text = if trim { s.trim().to_string() } else { s };
        if text.trim().is_empty() {
            continue;
        }
        let characters = text.chars().count();
        if characters < min_chars {
            continue;
        }
        sentences.push(Sentence {
            index: sentences.len() + 1,
            words: text.split_whitespace().count(),
            characters,
            text,
        });
    }

    if sentences.is_empty() {
        return Err(format!(
            "no sentences left: every detected sentence was shorter than min_chars ({min_chars})"
        ));
    }
    Ok(sentences)
}

/// Decide whether a terminator run ending just before whitespace is a real
/// sentence boundary. `term` is the index of the run's first terminator, `next`
/// the index of the following visible character.
fn is_boundary(
    chars: &[char],
    term: usize,
    next: usize,
    single_period: bool,
    sentence_start: usize,
    extra: &[String],
) -> bool {
    let next_ch = chars[next];

    if single_period {
        let (token, token_start) = token_before(chars, term, sentence_start);
        let lower = token.to_lowercase();

        if !token.is_empty() {
            // "1. Buy milk" — a bare number opening a line is a list marker.
            let at_start = chars[sentence_start..token_start]
                .iter()
                .all(|c| c.is_whitespace());
            if at_start && token.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }
            // A lone initial: "J. R. R. Tolkien".
            if token.chars().count() == 1 && token.chars().all(|c| c.is_alphabetic()) {
                return false;
            }
            if NEVER_BOUNDARY.contains(&lower.as_str()) || extra.iter().any(|e| *e == lower) {
                return false;
            }
            if NUMBER_PREFIXES.contains(&lower.as_str()) && next_ch.is_ascii_digit() {
                return false;
            }
        }
    }

    // A lowercase word after any terminator means the sentence continues:
    // covers "etc. and so on", `"Stop!" he said.` and "Wait... really?".
    !next_ch.is_lowercase()
}

/// Segment `text` and render the result in the requested `format`.
///
/// `format` is `"lines"` (default), `"numbered"`, `"blank-line"` or `"json"`.
/// The remaining arguments are passed straight to [`split_text`].
pub fn run(
    text: &str,
    format: &str,
    newlines: &str,
    trim: bool,
    min_chars: usize,
    extra_abbreviations: &str,
) -> Result<String, String> {
    let format = Format::parse(format)?;
    let sentences = split_text(text, newlines, trim, min_chars, extra_abbreviations)?;
    Ok(render(&sentences, format))
}

/// Render already-detected sentences.
pub fn render(sentences: &[Sentence], format: Format) -> String {
    match format {
        Format::Lines => sentences
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        Format::Numbered => sentences
            .iter()
            .map(|s| format!("{}. {}", s.index, s.text))
            .collect::<Vec<_>>()
            .join("\n"),
        Format::BlankLine => sentences
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n"),
        Format::Json => {
            let value = serde_json::json!({
                "count": sentences.len(),
                "sentences": sentences,
            });
            serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        run(text, "lines", "paragraph", true, 0, "")
            .unwrap()
            .lines()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn splits_plain_prose() {
        assert_eq!(
            lines("The cat sat. The dog barked! Did it? Yes."),
            vec![
                "The cat sat.",
                "The dog barked!",
                "Did it?",
                "Yes."
            ]
        );
    }

    #[test]
    fn keeps_titles_initials_and_inline_latin_together() {
        assert_eq!(
            lines("Dr. Green met Mrs. Hall. J. R. R. Tolkien wrote it, e.g. Chapter 2."),
            vec![
                "Dr. Green met Mrs. Hall.",
                "J. R. R. Tolkien wrote it, e.g. Chapter 2.",
            ]
        );
    }

    #[test]
    fn keeps_decimals_versions_and_reference_numbers_together() {
        assert_eq!(
            lines("Pi is 3.14 and it cost $99.99. Release 1.2.3 shipped. It arrived on Mar. 3. See No. 5 for details."),
            vec![
                "Pi is 3.14 and it cost $99.99.",
                "Release 1.2.3 shipped.",
                "It arrived on Mar. 3.",
                "See No. 5 for details.",
            ]
        );
    }

    #[test]
    fn dotted_abbreviations_can_still_end_a_sentence() {
        assert_eq!(
            lines("The talk starts at 5 p.m. The doors open earlier."),
            vec!["The talk starts at 5 p.m.", "The doors open earlier."]
        );
    }

    #[test]
    fn ellipses_and_quotes_do_not_split_mid_thought() {
        assert_eq!(
            lines("\"Stop!\" he said. Wait... really? Fine."),
            vec!["\"Stop!\" he said.", "Wait... really?", "Fine."]
        );
    }

    #[test]
    fn list_enumerators_are_not_boundaries() {
        assert_eq!(
            lines("1. Buy milk. 2. Walk the dog."),
            vec!["1. Buy milk.", "2. Walk the dog."]
        );
    }

    #[test]
    fn newline_modes_change_line_break_handling() {
        let text = "First line\nsecond line\n\nNew paragraph";
        assert_eq!(
            run(text, "lines", "paragraph", true, 0, "").unwrap(),
            "First line second line\nNew paragraph"
        );
        assert_eq!(
            run(text, "lines", "never", true, 0, "").unwrap(),
            "First line second line New paragraph"
        );
        assert_eq!(
            run(text, "lines", "always", true, 0, "").unwrap(),
            "First line\nsecond line\nNew paragraph"
        );
    }

    #[test]
    fn unterminated_trailing_text_is_still_a_sentence() {
        assert_eq!(lines("Done. No final period"), vec!["Done.", "No final period"]);
    }

    #[test]
    fn extra_abbreviations_suppress_a_break() {
        assert_eq!(
            lines("Ship it to Acme Corp. Then invoice them."),
            vec!["Ship it to Acme Corp.", "Then invoice them."]
        );
        assert_eq!(
            run(
                "Ship it to Acme Corp. Then invoice them.",
                "lines",
                "paragraph",
                true,
                0,
                "Corp, Ltd."
            )
            .unwrap(),
            "Ship it to Acme Corp. Then invoice them."
        );
    }

    #[test]
    fn min_chars_drops_short_fragments() {
        assert_eq!(
            run("Yes. This one is long enough to keep.", "lines", "paragraph", true, 10, "")
                .unwrap(),
            "This one is long enough to keep."
        );
    }

    #[test]
    fn numbered_and_blank_line_formats() {
        assert_eq!(
            run("One. Two.", "numbered", "paragraph", true, 0, "").unwrap(),
            "1. One.\n2. Two."
        );
        assert_eq!(
            run("One. Two.", "blank-line", "paragraph", true, 0, "").unwrap(),
            "One.\n\nTwo."
        );
    }

    #[test]
    fn json_format_reports_counts() {
        let out = run("Dr. Green sat. It rained.", "json", "paragraph", true, 0, "").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["sentences"][0]["text"], "Dr. Green sat.");
        assert_eq!(v["sentences"][0]["words"], 3);
        assert_eq!(v["sentences"][1]["index"], 2);
        assert_eq!(v["sentences"][1]["characters"], 10);
    }

    #[test]
    fn cjk_terminators_split_without_spaces() {
        assert_eq!(lines("これはペンです。それは本です。"), vec!["これはペンです。", "それは本です。"]);
    }

    #[test]
    fn untrimmed_output_preserves_original_spacing() {
        let out = run("  One.   Two.  ", "json", "paragraph", false, 0, "").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["sentences"][0]["text"], "  One.");
        assert_eq!(v["sentences"][1]["text"], "Two.");
    }

    // --- error paths --------------------------------------------------------

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   \n ", "lines", "paragraph", true, 0, "").unwrap_err();
        assert!(err.contains("text is empty"), "{err}");
    }

    #[test]
    fn invalid_format_and_newlines_are_errors() {
        let err = run("Hi there.", "csv", "paragraph", true, 0, "").unwrap_err();
        assert!(err.contains("invalid format"), "{err}");
        let err = run("Hi there.", "lines", "sometimes", true, 0, "").unwrap_err();
        assert!(err.contains("invalid newlines"), "{err}");
    }

    #[test]
    fn over_long_input_and_filter_are_errors() {
        let long = "a ".repeat(MAX_CHARS);
        let err = run(&long, "lines", "paragraph", true, 0, "").unwrap_err();
        assert!(err.contains("the maximum is"), "{err}");
        let err = run("Hi.", "lines", "paragraph", true, 50, "").unwrap_err();
        assert!(err.contains("no sentences left"), "{err}");
        let err = run("Hi.", "lines", "paragraph", true, MAX_MIN_CHARS + 1, "").unwrap_err();
        assert!(err.contains("min_chars"), "{err}");
    }
}
