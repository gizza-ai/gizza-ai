//! gizza-ai/truncate-text core — shorten text to a maximum number of
//! characters or words, appending an ellipsis when text is actually cut.
//!
//! Pure-Rust, dependency-free.
//!
//! Two units:
//! - `characters` (default): the result, *including* the ellipsis, is kept at or
//!   below `length` Unicode characters (when `count_ellipsis` is true). When
//!   `break_words` is false, the cut is backed up to the last whitespace so a
//!   word is never split mid-way; trailing whitespace before the ellipsis is
//!   trimmed.
//! - `words`: the first `length` whitespace-separated words are kept and the
//!   ellipsis is appended.
//!
//! If the text already fits within `length` units, it is returned unchanged with
//! NO ellipsis.

/// Minimum accepted length.
pub const MIN_LENGTH: u32 = 1;
/// Maximum accepted length (guards against absurd inputs).
pub const MAX_LENGTH: u32 = 1_000_000;

/// How `length` is measured.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    Characters,
    Words,
}

impl Unit {
    /// Parse the unit string (case-insensitive). Accepts a few friendly aliases.
    pub fn parse(s: &str) -> Result<Unit, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "characters" | "character" | "chars" | "char" | "c" => Ok(Unit::Characters),
            "words" | "word" | "w" => Ok(Unit::Words),
            other => Err(format!(
                "unit must be \"characters\" or \"words\" (got {other:?})"
            )),
        }
    }
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Truncate by characters.
fn truncate_chars(
    text: &str,
    length: usize,
    ellipsis: &str,
    count_ellipsis: bool,
    break_words: bool,
) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= length {
        return text.to_string();
    }

    // Number of source characters we may keep. When the ellipsis counts toward
    // the budget, reserve room for it (but never below 0).
    let ell_len = char_len(ellipsis);
    let budget = if count_ellipsis {
        length.saturating_sub(ell_len)
    } else {
        length
    };
    let cut = budget.min(chars.len());

    let mut kept: String = chars[..cut].iter().collect();

    if !break_words {
        // Back up to the last whitespace so we don't split a word. If there is
        // no whitespace in the kept slice (or backing up would leave nothing),
        // fall back to the hard cut rather than returning an empty string.
        if let Some(pos) = kept.rfind(char::is_whitespace) {
            let candidate = &kept[..pos];
            if !candidate.trim_end().is_empty() {
                kept.truncate(pos);
            }
        }
    }

    let mut out = kept.trim_end().to_string();
    out.push_str(ellipsis);
    out
}

/// Truncate by words.
fn truncate_words(text: &str, length: usize, ellipsis: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= length {
        return text.to_string();
    }
    let mut out = words[..length].join(" ");
    out.push_str(ellipsis);
    out
}

/// Shorten `text` to at most `length` units (`Unit::Characters` or
/// `Unit::Words`), appending `ellipsis` only when text is actually cut.
///
/// Errors if `length` is outside `MIN_LENGTH..=MAX_LENGTH`.
pub fn truncate(
    text: &str,
    length: u32,
    unit: Unit,
    ellipsis: &str,
    count_ellipsis: bool,
    break_words: bool,
) -> Result<String, String> {
    if !(MIN_LENGTH..=MAX_LENGTH).contains(&length) {
        return Err(format!(
            "length must be between {MIN_LENGTH} and {MAX_LENGTH} (got {length})"
        ));
    }
    let length = length as usize;
    let out = match unit {
        Unit::Characters => truncate_chars(text, length, ellipsis, count_ellipsis, break_words),
        Unit::Words => truncate_words(text, length, ellipsis),
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorter_text_is_unchanged() {
        let got = truncate("hello", 20, Unit::Characters, "…", true, false).unwrap();
        assert_eq!(got, "hello");
        // exactly at the limit: still unchanged, no ellipsis.
        let got = truncate("hello", 5, Unit::Characters, "…", true, false).unwrap();
        assert_eq!(got, "hello");
    }

    #[test]
    fn truncates_by_chars_at_word_boundary() {
        let t = "the quick brown fox";
        // length 12, ellipsis counts (1 char) -> budget 11 -> "the quick b"
        // back up to last space -> "the quick" -> + "…"
        let got = truncate(t, 12, Unit::Characters, "…", true, false).unwrap();
        assert_eq!(got, "the quick…");
        assert!(got.chars().count() <= 12);
    }

    #[test]
    fn truncates_by_chars_breaking_words() {
        let t = "the quick brown fox";
        let got = truncate(t, 12, Unit::Characters, "…", true, true).unwrap();
        // budget 11 chars, hard cut: "the quick b" + "…"
        assert_eq!(got, "the quick b…");
        assert!(got.chars().count() <= 12);
    }

    #[test]
    fn ellipsis_excluded_from_budget() {
        let t = "abcdefghij";
        // length 5, count_ellipsis false -> keep 5 chars + ellipsis
        let got = truncate(t, 5, Unit::Characters, "...", false, true).unwrap();
        assert_eq!(got, "abcde...");
    }

    #[test]
    fn custom_ellipsis_words() {
        let t = "one two three four";
        let got = truncate(t, 2, Unit::Words, " [more]", true, false).unwrap();
        assert_eq!(got, "one two [more]");
    }

    #[test]
    fn truncates_by_words() {
        let t = "the quick brown fox jumps";
        let got = truncate(t, 3, Unit::Words, "…", true, false).unwrap();
        assert_eq!(got, "the quick brown…");
    }

    #[test]
    fn words_fewer_than_limit_unchanged() {
        let t = "a b c";
        let got = truncate(t, 10, Unit::Words, "…", true, false).unwrap();
        assert_eq!(got, "a b c");
    }

    #[test]
    fn no_whitespace_falls_back_to_hard_cut() {
        let t = "supercalifragilisticexpialidocious";
        // no spaces, break_words=false -> still must cut hard
        let got = truncate(t, 10, Unit::Characters, "…", true, false).unwrap();
        assert_eq!(got, "supercali…");
        assert!(got.chars().count() <= 10);
    }

    #[test]
    fn unicode_counted_by_chars() {
        let t = "ééééé ààààà";
        let got = truncate(t, 6, Unit::Characters, "…", true, true).unwrap();
        // budget 5 chars -> "ééééé" + "…"
        assert_eq!(got, "ééééé…");
        assert_eq!(got.chars().count(), 6);
    }

    #[test]
    fn unit_parse() {
        assert_eq!(Unit::parse("Words").unwrap(), Unit::Words);
        assert_eq!(Unit::parse("chars").unwrap(), Unit::Characters);
        assert!(Unit::parse("lines").is_err());
    }

    #[test]
    fn rejects_bad_length() {
        assert!(truncate("x", 0, Unit::Characters, "…", true, false).is_err());
        assert!(truncate("x", MAX_LENGTH + 1, Unit::Characters, "…", true, false).is_err());
    }

    #[test]
    fn empty_input() {
        assert_eq!(
            truncate("", 80, Unit::Characters, "…", true, false).unwrap(),
            ""
        );
        assert_eq!(truncate("", 5, Unit::Words, "…", true, false).unwrap(), "");
    }
}
