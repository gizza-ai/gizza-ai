//! gizza-ai/zero-width-cleaner core — detect and strip invisible zero-width
//! spaces, joiners, byte-order marks and other non-printing formatting
//! characters from text. Pure-Rust, dependency-free.
//!
//! The characters targeted here are *invisible*: they occupy no visible width
//! (or masquerade as an ordinary space) yet still ride along in copied text.
//! They routinely sneak in from web pages, PDFs, word processors, chat apps and
//! "watermarked" AI output, and then break search, diffing, `==` comparisons,
//! code, CSV imports and password fields.
//!
//! Four disjoint groups are handled, each behind its own toggle:
//!
//! * **Zero-width** (`remove_zero_width`) — U+200B ZERO WIDTH SPACE,
//!   U+200C ZERO WIDTH NON-JOINER, U+200D ZERO WIDTH JOINER, U+2060 WORD JOINER,
//!   U+2061–U+2064 invisible math operators, U+180E MONGOLIAN VOWEL SEPARATOR and
//!   U+FEFF ZERO WIDTH NO-BREAK SPACE (the byte-order mark / BOM).
//! * **Bidirectional controls** (`remove_bidi`) — U+061C ARABIC LETTER MARK,
//!   U+200E/U+200F LRM/RLM, U+202A–U+202E embeddings & overrides and
//!   U+2066–U+2069 isolates. These invisible marks can visually reorder text and
//!   are a known spoofing vector ("Trojan Source").
//! * **Soft hyphen** (`remove_soft_hyphen`) — U+00AD, an invisible optional
//!   line-break hint that shows up as a stray character when copied.
//! * **Odd spaces** (`replace_nbsp`) — non-breaking and other Unicode space
//!   separators (U+00A0, U+2000–U+200A, U+202F, U+205F, U+3000, U+1680) are
//!   *replaced with a normal ASCII space* rather than deleted, since they hold a
//!   real word gap.
//!
//! Each removed character (zero-width / bidi / soft-hyphen) is replaced with the
//! `replacement` string ("" by default → the character is simply deleted). Odd
//! spaces always collapse to a single ASCII space when `replace_nbsp` is on.

/// U+200B ZWSP, U+200C ZWNJ, U+200D ZWJ, U+2060 WJ, U+2061–U+2064 invisible math
/// operators, U+180E Mongolian vowel separator, U+FEFF BOM / ZWNBSP.
fn is_zero_width(c: char) -> bool {
    matches!(
        c as u32,
        0x200B | 0x200C | 0x200D | 0x2060 | 0x2061..=0x2064 | 0x180E | 0xFEFF
    )
}

/// Invisible bidirectional formatting controls (also removed by default).
fn is_bidi(c: char) -> bool {
    matches!(
        c as u32,
        0x061C | 0x200E | 0x200F | 0x202A..=0x202E | 0x2066..=0x2069
    )
}

/// U+00AD SOFT HYPHEN — an invisible optional-break hint.
fn is_soft_hyphen(c: char) -> bool {
    c as u32 == 0x00AD
}

/// Non-breaking / unusual Unicode space separators that look like an ordinary
/// space but aren't U+0020. Note U+200B (zero-width space) is NOT here — it has
/// no width and belongs to [`is_zero_width`].
fn is_odd_space(c: char) -> bool {
    matches!(
        c as u32,
        0x00A0 | 0x1680 | 0x2000..=0x200A | 0x202F | 0x205F | 0x3000
    )
}

/// Return true if `c` would be deleted (zero-width / bidi / soft-hyphen) given
/// the toggles. Odd-space replacement is counted separately by [`count_hits`].
fn is_removable(
    c: char,
    remove_zero_width: bool,
    remove_bidi_flag: bool,
    remove_soft_hyphen: bool,
) -> bool {
    (remove_zero_width && is_zero_width(c))
        || (remove_bidi_flag && is_bidi(c))
        || (remove_soft_hyphen && is_soft_hyphen(c))
}

/// Strip invisible characters from `text`.
///
/// * `remove_zero_width` — delete zero-width spaces/joiners, word joiner,
///   invisible math operators and the BOM.
/// * `remove_bidi` — delete bidirectional formatting controls.
/// * `remove_soft_hyphen` — delete U+00AD soft hyphens.
/// * `replace_nbsp` — replace non-breaking & other odd Unicode spaces with a
///   normal ASCII space (they carry a real word gap, so they are not deleted).
/// * `replacement` — string substituted for each *deleted* character
///   (zero-width / bidi / soft-hyphen); "" deletes it outright.
pub fn clean(
    text: &str,
    remove_zero_width: bool,
    remove_bidi: bool,
    remove_soft_hyphen: bool,
    replace_nbsp: bool,
    replacement: &str,
) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if is_removable(c, remove_zero_width, remove_bidi, remove_soft_hyphen) {
            out.push_str(replacement);
        } else if replace_nbsp && is_odd_space(c) {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// Count how many characters in `text` the cleaner would touch (delete or, for
/// odd spaces, replace) given the toggles. Useful for a "detected N invisible
/// characters" report.
pub fn count_hits(
    text: &str,
    remove_zero_width: bool,
    remove_bidi: bool,
    remove_soft_hyphen: bool,
    replace_nbsp: bool,
) -> usize {
    text.chars()
        .filter(|&c| {
            is_removable(c, remove_zero_width, remove_bidi, remove_soft_hyphen)
                || (replace_nbsp && is_odd_space(c))
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Default flags mirror the descriptor defaults: remove zero-width, bidi and
    // soft hyphen; leave odd spaces alone.
    fn clean_default(t: &str) -> String {
        clean(t, true, true, true, false, "")
    }

    #[test]
    fn strips_zero_width_space_and_joiners() {
        // ZWSP, ZWNJ, ZWJ, WORD JOINER between letters — all deleted by default.
        let t = "a\u{200B}b\u{200C}c\u{200D}d\u{2060}e";
        assert_eq!(clean_default(t), "abcde");
    }

    #[test]
    fn strips_bom() {
        // A leading BOM (U+FEFF) is the classic "invisible first character".
        assert_eq!(clean_default("\u{FEFF}hello"), "hello");
    }

    #[test]
    fn strips_bidi_controls() {
        // RLO override + PDF (a Trojan-Source style sequence) removed.
        let t = "user\u{202E}resu\u{202C}";
        assert_eq!(clean_default(t), "userresu");
    }

    #[test]
    fn strips_soft_hyphen() {
        assert_eq!(clean_default("sig\u{00AD}nal"), "signal");
    }

    #[test]
    fn keeps_ordinary_text_and_emoji() {
        // Visible characters, accents, ordinary spaces and emoji survive.
        let t = "café 🚀 — a normal line";
        assert_eq!(clean_default(t), t);
    }

    #[test]
    fn zwj_emoji_preserved_when_zero_width_off() {
        // Family emoji uses ZWJ (U+200D); with the toggle off it is untouched.
        let family = "👨\u{200D}👩\u{200D}👧";
        assert_eq!(clean(family, false, true, true, false, ""), family);
        // With zero-width removal on, the ZWJ is stripped (documented trade-off).
        assert_eq!(clean(family, true, true, true, false, ""), "👨👩👧");
    }

    #[test]
    fn replace_nbsp_normalizes_spaces_but_keeps_gap() {
        let t = "a\u{00A0}b\u{2009}c\u{3000}d";
        // Off: odd spaces are left as-is.
        assert_eq!(clean(t, true, true, true, false, ""), t);
        // On: each odd space becomes a single ASCII space (word gaps kept).
        assert_eq!(clean(t, true, true, true, true, ""), "a b c d");
    }

    #[test]
    fn replacement_string_substitutes_deleted_chars() {
        let t = "a\u{200B}b\u{FEFF}c";
        assert_eq!(clean(t, true, true, true, false, "_"), "a_b_c");
        assert_eq!(clean(t, true, true, true, false, "[?]"), "a[?]b[?]c");
    }

    #[test]
    fn toggles_are_independent() {
        let t = "a\u{200B}b\u{202E}c\u{00AD}d";
        // Only zero-width removed; bidi + soft hyphen kept.
        assert_eq!(clean(t, true, false, false, false, ""), "ab\u{202E}c\u{00AD}d");
        // Only soft hyphen removed.
        assert_eq!(clean(t, false, false, true, false, ""), "a\u{200B}b\u{202E}cd");
    }

    #[test]
    fn count_hits_reports_detected_characters() {
        let t = "a\u{200B}b\u{202E}c\u{00AD}d\u{00A0}e";
        // Defaults (no nbsp): ZWSP + RLO + soft hyphen = 3.
        assert_eq!(count_hits(t, true, true, true, false), 3);
        // With nbsp replacement: + the non-breaking space = 4.
        assert_eq!(count_hits(t, true, true, true, true), 4);
        // Nothing enabled → nothing detected.
        assert_eq!(count_hits(t, false, false, false, false), 0);
    }

    #[test]
    fn empty_input() {
        assert_eq!(clean_default(""), "");
        assert_eq!(count_hits("", true, true, true, true), 0);
    }
}
