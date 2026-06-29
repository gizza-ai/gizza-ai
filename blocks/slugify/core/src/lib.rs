//! slugify core — turn a title or phrase into a clean URL-safe slug. No
//! wafer/wasm-bindgen deps; shared by the chat skill block and the web page.
//!
//! Pipeline: transliterate Unicode → ASCII (`deunicode`, so `Crème Brûlée` →
//! `creme-brulee`, `北京` → `bei-jing`), drop apostrophes so contractions join
//! (`Bob's` → `bobs`), lowercase (optional), then collapse every run of
//! remaining non-alphanumeric characters to a single `separator`, trimming it
//! from both ends. An optional `max_length` truncates on a word boundary.

use deunicode::deunicode;

/// Hard cap on `max_length` so a hostile value can't request an absurd slice;
/// 0 means "no limit". 2048 comfortably exceeds any real URL slug.
pub const MAX_LENGTH_CAP: u32 = 2048;

/// Slugify `text`.
///
/// - `separator` (blank → `"-"`): the string placed between words. Must not
///   contain ASCII letters or digits (it has to survive the cleaning pass).
///   Common choices are `"-"` (default) or `"_"`; `""` concatenates words.
/// - `lowercase`: when `true` (the usual case) the slug is lowercased.
/// - `max_length`: when `> 0`, truncate the slug to at most this many
///   characters, cutting on a word boundary (never mid-word) where possible.
///   Clamped to `MAX_LENGTH_CAP`. `0` means no limit.
/// - `per_line`: when `true`, slugify each line of `text` independently and
///   rejoin with `'\n'` (a batch list of titles).
///
/// Returns `Err` only when `separator` is invalid.
pub fn slugify(
    text: &str,
    separator: &str,
    lowercase: bool,
    max_length: u32,
    per_line: bool,
) -> Result<String, String> {
    let sep = if separator.is_empty() { "-" } else { separator };
    validate_separator(sep)?;
    let limit = max_length.min(MAX_LENGTH_CAP);

    let run = |line: &str| slugify_line(line, sep, lowercase, limit);

    if per_line {
        Ok(text.split('\n').map(run).collect::<Vec<_>>().join("\n"))
    } else {
        Ok(run(text))
    }
}

/// A separator may not contain ASCII alphanumerics — otherwise it would be
/// stripped (or merged into words) by the cleaning pass and the slug would be
/// malformed. Anything else (symbols, blank) is allowed.
fn validate_separator(sep: &str) -> Result<(), String> {
    if sep.bytes().any(|b| b.is_ascii_alphanumeric()) {
        return Err(format!(
            "invalid separator {sep:?}: must not contain ASCII letters or digits"
        ));
    }
    Ok(())
}

fn slugify_line(line: &str, sep: &str, lowercase: bool, limit: u32) -> String {
    // 1. Transliterate to ASCII. After this every char is ASCII, so byte
    //    indexing below is safe.
    let mut ascii = deunicode(line);
    if lowercase {
        ascii.make_ascii_lowercase();
    }

    // 2. Build the slug: keep ASCII alphanumerics verbatim, drop apostrophes so
    //    contractions/possessives join (`bob's` → `bobs`), and treat every other
    //    run of characters as a single separator.
    let mut out = String::with_capacity(ascii.len());
    let mut pending_sep = false;
    for ch in ascii.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_sep && !out.is_empty() {
                out.push_str(sep);
            }
            pending_sep = false;
            out.push(ch);
        } else if ch == '\'' {
            // Apostrophe: skip entirely without forcing a word break.
        } else {
            pending_sep = true;
        }
    }

    if limit > 0 {
        truncate_on_word_boundary(out, sep, limit as usize)
    } else {
        out
    }
}

/// Truncate `slug` to at most `limit` characters. If the cut would fall
/// mid-word and the kept portion still contains a separator, back up to the
/// last separator so the slug ends on a whole word. A trailing separator is
/// always stripped.
fn truncate_on_word_boundary(slug: String, sep: &str, limit: usize) -> String {
    // The slug is pure ASCII, so char count == byte length.
    if slug.len() <= limit {
        return slug;
    }
    let mut cut = &slug[..limit];
    // Whether we sliced through the middle of a word: the char at the cut point
    // continues a word (alphanumeric) and the char just before it is too.
    let next_is_wordy = slug.as_bytes()[limit].is_ascii_alphanumeric();
    let prev_is_wordy = cut
        .bytes()
        .last()
        .map_or(false, |b| b.is_ascii_alphanumeric());
    if next_is_wordy && prev_is_wordy {
        if let Some(idx) = cut.rfind(sep) {
            cut = &cut[..idx];
        }
    }
    cut.trim_end_matches(sep).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slug(s: &str) -> String {
        slugify(s, "-", true, 0, false).unwrap()
    }

    #[test]
    fn basic_phrase() {
        assert_eq!(slug("Hello World"), "hello-world");
    }

    #[test]
    fn collapses_runs_and_trims_edges() {
        assert_eq!(slug("  Hello,   World!!  "), "hello-world");
        assert_eq!(slug("--Already--Sluggish--"), "already-sluggish");
    }

    #[test]
    fn transliterates_unicode_to_ascii() {
        assert_eq!(slug("Crème Brûlée"), "creme-brulee");
        assert_eq!(slug("Münchner Straße"), "munchner-strasse");
        assert_eq!(slug("北京"), "bei-jing");
        assert_eq!(slug("São Paulo"), "sao-paulo");
    }

    #[test]
    fn drops_apostrophes_so_contractions_join() {
        assert_eq!(slug("Bob's Burgers"), "bobs-burgers");
        // A typographic apostrophe deunicodes to a plain one and is also dropped.
        assert_eq!(slug("It’s a Test"), "its-a-test");
    }

    #[test]
    fn keeps_existing_digits_and_dashes() {
        assert_eq!(slug("Top 10 Tips (2024)"), "top-10-tips-2024");
    }

    #[test]
    fn lowercase_can_be_disabled() {
        assert_eq!(
            slugify("Hello World", "-", false, 0, false).unwrap(),
            "Hello-World"
        );
    }

    #[test]
    fn custom_separator_underscore() {
        assert_eq!(
            slugify("Hello World Test", "_", true, 0, false).unwrap(),
            "hello_world_test"
        );
    }

    #[test]
    fn blank_separator_falls_back_to_dash() {
        // An empty separator string defaults to "-" (not "no separator").
        assert_eq!(
            slugify("Hello World", "", true, 0, false).unwrap(),
            "hello-world"
        );
    }

    #[test]
    fn max_length_truncates_on_word_boundary() {
        // "the-quick-brown-fox" capped at 13 → "the-quick" (won't split "brown").
        assert_eq!(
            slugify("The Quick Brown Fox", "-", true, 13, false).unwrap(),
            "the-quick"
        );
        // Trailing separator from the cut is stripped.
        assert_eq!(
            slugify("The Quick Brown Fox", "-", true, 10, false).unwrap(),
            "the-quick"
        );
    }

    #[test]
    fn max_length_hard_cuts_a_single_long_word() {
        // No separator to back up to → hard cut at the limit.
        assert_eq!(
            slugify("Supercalifragilistic", "-", true, 5, false).unwrap(),
            "super"
        );
    }

    #[test]
    fn max_length_keeps_short_slug_intact() {
        assert_eq!(
            slugify("Hi There", "-", true, 100, false).unwrap(),
            "hi-there"
        );
    }

    #[test]
    fn per_line_slugifies_each_line() {
        assert_eq!(
            slugify("Hello World\nFoo & Bar", "-", true, 0, true).unwrap(),
            "hello-world\nfoo-bar"
        );
        // Without per_line the newline is just another separator run.
        assert_eq!(
            slugify("Hello World\nFoo & Bar", "-", true, 0, false).unwrap(),
            "hello-world-foo-bar"
        );
    }

    #[test]
    fn empty_and_symbol_only_input_yield_empty_slug() {
        assert_eq!(slug(""), "");
        assert_eq!(slug("!!! ??? ..."), "");
    }

    #[test]
    fn rejects_alphanumeric_separator() {
        let err = slugify("a b", "x", true, 0, false).unwrap_err();
        assert!(err.contains("invalid separator"), "got: {err}");
        let err = slugify("a b", "-9-", true, 0, false).unwrap_err();
        assert!(err.contains("invalid separator"), "got: {err}");
    }

    #[test]
    fn separator_cap_does_not_panic_on_huge_max_length() {
        // max_length far above the slug length is a no-op, and above the cap is
        // clamped — neither panics.
        assert_eq!(
            slugify("hello world", "-", true, u32::MAX, false).unwrap(),
            "hello-world"
        );
    }
}
