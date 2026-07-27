//! gizza-ai/emoji-remover core — strip emoji and pictographic symbols from
//! text. Pure-Rust; the only dependency is `unicode-segmentation` for
//! grapheme clustering.
//!
//! ## How detection works
//!
//! Emoji are matched per **extended grapheme cluster** (UAX #29), not per
//! `char`. That is what makes multi-codepoint emoji come out cleanly and
//! whole:
//!
//! * ZWJ sequences — 👨‍👩‍👧‍👦 (man + ZWJ + woman + ZWJ + girl + ZWJ + boy) is a
//!   single grapheme, removed in one piece (joiners and all).
//! * Regional-indicator flags — 🇬🇧 is two regional indicators that cluster
//!   into one grapheme.
//! * Skin-tone modifiers — 👍🏽 (thumbs-up + Fitzpatrick modifier) is one
//!   grapheme.
//! * Keycap sequences — 1️⃣ (digit + VS16 + U+20E3 enclosing keycap).
//! * Variation selectors — the emoji-presentation selector VS16 (U+FE0F) rides
//!   along inside the cluster and is removed with it.
//!
//! A cluster is treated as emoji when it contains any Extended_Pictographic
//! character, a regional indicator, or the combining enclosing keycap. Whether
//! a lone *text-default* symbol (©, ®, ™, ❤ without VS16, …) counts is
//! controlled by `keep_text_symbols`.

use unicode_segmentation::UnicodeSegmentation;

/// What to leave behind in place of each removed emoji.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Delete the emoji entirely (leave nothing).
    Remove,
    /// Leave a single space for each removed emoji.
    Space,
    /// Leave a caller-supplied placeholder string for each removed emoji.
    Placeholder,
}

impl Mode {
    /// Parse the string form used by the descriptor/CLI/web surfaces.
    pub fn parse(s: &str) -> Option<Mode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "remove" => Some(Mode::Remove),
            "space" => Some(Mode::Space),
            "placeholder" => Some(Mode::Placeholder),
            _ => None,
        }
    }
}

const VS16: char = '\u{FE0F}'; // emoji presentation selector
const KEYCAP: char = '\u{20E3}'; // combining enclosing keycap

/// Regional indicator symbols A–Z (used in pairs to form flags).
fn is_regional_indicator(c: char) -> bool {
    ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}

/// Fitzpatrick skin-tone modifiers.
fn is_skin_tone_modifier(c: char) -> bool {
    ('\u{1F3FB}'..='\u{1F3FF}').contains(&c)
}

/// The BMP characters that default to **emoji** presentation
/// (Unicode `Emoji_Presentation=Yes`). Every code point at U+1F000 or above is
/// emoji-presentation, so this table only needs the scattered BMP members.
fn is_bmp_emoji_presentation(c: char) -> bool {
    matches!(c as u32,
        0x231A..=0x231B | 0x23E9..=0x23EC | 0x23F0 | 0x23F3 | 0x25FD..=0x25FE
        | 0x2614..=0x2615 | 0x2648..=0x2653 | 0x267F | 0x2693 | 0x26A1
        | 0x26AA..=0x26AB | 0x26BD..=0x26BE | 0x26C4..=0x26C5 | 0x26CE | 0x26D4
        | 0x26EA | 0x26F2..=0x26F3 | 0x26F5 | 0x26FA | 0x26FD | 0x2705
        | 0x270A..=0x270B | 0x2728 | 0x274C | 0x274E | 0x2753..=0x2755 | 0x2757
        | 0x2795..=0x2797 | 0x27B0 | 0x27BF | 0x2B1B..=0x2B1C | 0x2B50 | 0x2B55)
}

/// Unicode `Extended_Pictographic` property (emoji + pictographic symbols).
/// Covers the scattered BMP members plus the SMP emoji planes; a handful of
/// reserved code points inside the U+1F000–U+1FAFF blocks are
/// Extended_Pictographic by design (reserved for future emoji), matching the
/// property definition.
fn is_extended_pictographic(c: char) -> bool {
    let u = c as u32;
    matches!(u,
        0x00A9 | 0x00AE | 0x203C | 0x2049 | 0x2122 | 0x2139
        | 0x2194..=0x2199 | 0x21A9..=0x21AA | 0x231A..=0x231B | 0x2328 | 0x2388
        | 0x23CF | 0x23E9..=0x23F3 | 0x23F8..=0x23FA | 0x24C2 | 0x25AA..=0x25AB
        | 0x25B6 | 0x25C0 | 0x25FB..=0x25FE
        | 0x2600..=0x2605 | 0x2607..=0x2612 | 0x2614..=0x2685 | 0x2690..=0x2705
        | 0x2708..=0x2712 | 0x2714 | 0x2716 | 0x271D | 0x2721 | 0x2728
        | 0x2733..=0x2734 | 0x2744 | 0x2747 | 0x274C | 0x274E | 0x2753..=0x2755
        | 0x2757 | 0x2763..=0x2767 | 0x2795..=0x2797 | 0x27A1 | 0x27B0 | 0x27BF
        | 0x2934..=0x2935 | 0x2B05..=0x2B07 | 0x2B1B..=0x2B1C | 0x2B50 | 0x2B55
        | 0x3030 | 0x303D | 0x3297 | 0x3299
        | 0x1F000..=0x1FAFF)
}

/// Classify one grapheme cluster.
///
/// Returns `true` when the whole cluster should be treated as an emoji /
/// pictographic run and removed.
fn cluster_is_emoji(cluster: &str, keep_text_symbols: bool) -> bool {
    let mut has_vs16 = false;
    let mut has_keycap = false;
    let mut has_regional = false;
    let mut has_emoji_default = false; // pictographic with default emoji presentation
    let mut has_text_default = false; // pictographic with default text presentation

    for c in cluster.chars() {
        match c {
            VS16 => has_vs16 = true,
            KEYCAP => has_keycap = true,
            _ if is_regional_indicator(c) => has_regional = true,
            _ if is_skin_tone_modifier(c) => has_emoji_default = true,
            _ if is_extended_pictographic(c) => {
                if (c as u32) >= 0x1F000 || is_bmp_emoji_presentation(c) {
                    has_emoji_default = true;
                } else {
                    has_text_default = true;
                }
            }
            _ => {}
        }
    }

    // Keycaps and regional-indicator flags are always emoji.
    if has_keycap || has_regional {
        return true;
    }
    // Anything with a default emoji presentation (or an emoji-styled symbol) goes.
    if has_emoji_default {
        return true;
    }
    // A lone text-default symbol (©, ®, ™, ❤ without VS16, …).
    if has_text_default {
        // Kept only when the caller asked to keep text symbols AND the symbol
        // is not explicitly emoji-styled with VS16.
        if keep_text_symbols && !has_vs16 {
            return false;
        }
        return true;
    }
    false
}

/// Collapse each run of whitespace to a single character, preserving paragraph
/// breaks: a run that contains a line break collapses to one `\n`, otherwise to
/// one space. Leading/trailing whitespace is trimmed.
fn collapse_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run: Option<bool> = None; // Some(has_newline) while inside a ws run
    for c in text.chars() {
        if c.is_whitespace() {
            let nl = c == '\n' || c == '\r';
            run = Some(run.map_or(nl, |had| had || nl));
        } else {
            if let Some(had_nl) = run.take() {
                out.push(if had_nl { '\n' } else { ' ' });
            }
            out.push(c);
        }
    }
    // A trailing whitespace run is dropped by the loop; trim leading whitespace
    // that came from removed emoji at the start or a leading whitespace run.
    out.trim().to_string()
}

/// Strip emoji and pictographic symbols from `text`.
///
/// * `mode` — what to leave behind for each removed emoji.
/// * `placeholder` — the string used when `mode == Mode::Placeholder`.
/// * `collapse_whitespace` — collapse runs of whitespace left behind (and
///   throughout the text) into a single space / newline and trim the ends.
/// * `keep_text_symbols` — keep pictographic symbols that default to text
///   presentation (©, ®, ™, ❤ without an emoji variation selector, …) instead
///   of removing them.
pub fn remove_emoji(
    text: &str,
    mode: Mode,
    placeholder: &str,
    collapse_whitespace: bool,
    keep_text_symbols: bool,
) -> String {
    let replacement = match mode {
        Mode::Remove => "",
        Mode::Space => " ",
        Mode::Placeholder => placeholder,
    };

    let mut out = String::with_capacity(text.len());
    for g in text.graphemes(true) {
        if cluster_is_emoji(g, keep_text_symbols) {
            out.push_str(replacement);
        } else {
            out.push_str(g);
        }
    }

    if collapse_whitespace {
        collapse_ws(&out)
    } else {
        out
    }
}

/// Count how many grapheme clusters in `text` would be removed as emoji.
pub fn count_emoji(text: &str, keep_text_symbols: bool) -> usize {
    text.graphemes(true)
        .filter(|g| cluster_is_emoji(g, keep_text_symbols))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(t: &str) -> String {
        remove_emoji(t, Mode::Remove, "", false, false)
    }

    #[test]
    fn removes_basic_emoji() {
        assert_eq!(strip("Hello 👋 World 🚀"), "Hello  World ");
    }

    #[test]
    fn keeps_plain_text_and_digits() {
        // Digits, '#', '*' are Emoji=Yes but NOT Extended_Pictographic — keep them.
        let t = "Order #5 costs $3.50 — 100% ok.";
        assert_eq!(strip(t), t);
    }

    #[test]
    fn removes_zwj_family_as_one() {
        // 👨‍👩‍👧‍👦 is a single ZWJ grapheme; nothing (no stray joiners) is left.
        assert_eq!(strip("family 👨‍👩‍👧‍👦 done"), "family  done");
    }

    #[test]
    fn removes_flag_regional_indicators() {
        assert_eq!(strip("go 🇬🇧 team"), "go  team");
    }

    #[test]
    fn removes_skin_tone_modifier() {
        assert_eq!(strip("nice 👍🏽!"), "nice !");
    }

    #[test]
    fn removes_keycap_sequence() {
        assert_eq!(strip("press 1️⃣ now"), "press  now");
        // A bare digit stays.
        assert_eq!(strip("press 1 now"), "press 1 now");
    }

    #[test]
    fn removes_emoji_styled_heart_but_keep_option_preserves_text_heart() {
        // ❤️ (with VS16) is always removed.
        assert_eq!(strip("I ❤️ NY"), "I  NY");
        // Bare ❤ is removed by default…
        assert_eq!(strip("I ❤ NY"), "I  NY");
        // …but kept when keep_text_symbols is on.
        assert_eq!(
            remove_emoji("I ❤ NY", Mode::Remove, "", false, true),
            "I ❤ NY"
        );
        // Even with keep_text_symbols, a VS16-styled heart still goes.
        assert_eq!(
            remove_emoji("I ❤️ NY", Mode::Remove, "", false, true),
            "I  NY"
        );
    }

    #[test]
    fn keep_text_symbols_preserves_copyright_family() {
        assert_eq!(
            remove_emoji("© 2026 ACME™ ®", Mode::Remove, "", false, true),
            "© 2026 ACME™ ®"
        );
        // Default strips them (trailing ® removed, leaving a trailing space).
        assert_eq!(strip("© 2026 ACME™ ®"), " 2026 ACME ");
    }

    #[test]
    fn space_mode_leaves_one_space_each() {
        assert_eq!(
            remove_emoji("a👋b🚀c", Mode::Space, "", false, false),
            "a b c"
        );
    }

    #[test]
    fn placeholder_mode_substitutes() {
        assert_eq!(
            remove_emoji("hi 👋 there", Mode::Placeholder, "[emoji]", false, false),
            "hi [emoji] there"
        );
    }

    #[test]
    fn collapse_whitespace_tidies_gaps() {
        // Removing the emoji leaves a double space; collapse fixes it.
        assert_eq!(
            remove_emoji("Hello 👋 World", Mode::Remove, "", true, false),
            "Hello World"
        );
        // Leading/trailing emoji + trim.
        assert_eq!(
            remove_emoji("🚀 launch 🚀", Mode::Remove, "", true, false),
            "launch"
        );
        // Paragraph break is preserved as a single newline.
        assert_eq!(
            remove_emoji("a 🚀\n\nb 🚀", Mode::Remove, "", true, false),
            "a\nb"
        );
    }

    #[test]
    fn preserves_accents_and_cjk() {
        let t = "café — 東京 — naïve";
        assert_eq!(strip(t), t);
    }

    #[test]
    fn count_matches() {
        assert_eq!(count_emoji("a👋b🚀c🇬🇧", false), 3);
        assert_eq!(count_emoji("no emoji here", false), 0);
        // Bare text heart counts by default, not when kept.
        assert_eq!(count_emoji("❤", false), 1);
        assert_eq!(count_emoji("❤", true), 0);
    }

    #[test]
    fn empty_input() {
        assert_eq!(strip(""), "");
        assert_eq!(count_emoji("", false), 0);
    }

    #[test]
    fn mode_parse() {
        assert_eq!(Mode::parse("remove"), Some(Mode::Remove));
        assert_eq!(Mode::parse(" Space "), Some(Mode::Space));
        assert_eq!(Mode::parse("PLACEHOLDER"), Some(Mode::Placeholder));
        assert_eq!(Mode::parse("nope"), None);
    }
}
