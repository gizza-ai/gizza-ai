//! smart-quotes-clean core — replace "smart"/typographic characters with plain
//! ASCII equivalents. No wafer/wasm-bindgen deps; shared by the chat skill block
//! and the web page.
//!
//! Targets the characters word processors and the web silently introduce —
//! curly quotes, em/en dashes, the ellipsis glyph, prime marks, guillemets — and
//! the invisible/exotic spaces (non-breaking, thin, zero-width) that break diffs,
//! code, CSV, and plain-text pipelines. Everything else is left untouched, so
//! ordinary Unicode (accents, emoji, CJK) survives.

/// How an em dash (—) and the horizontal bar (―) are rendered in ASCII.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmDash {
    /// Two hyphens — the common Markdown/plain-text convention. The default.
    DoubleHyphen,
    /// A single hyphen.
    Hyphen,
    /// A spaced hyphen ` - `.
    SpacedHyphen,
}

impl EmDash {
    /// Parse the option string (as sent by the chat schema / page select).
    /// Blank or unrecognised falls back to the default `--`.
    pub fn parse(s: &str) -> EmDash {
        match s {
            " - " => EmDash::SpacedHyphen,
            _ => match s.trim() {
            "-" => EmDash::Hyphen,
            // "--", "", and anything else → the default double hyphen.
            _ => EmDash::DoubleHyphen,
            },
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            EmDash::DoubleHyphen => "--",
            EmDash::Hyphen => "-",
            EmDash::SpacedHyphen => " - ",
        }
    }
}

/// Replace smart quotes, em/en dashes, the ellipsis glyph, and other common
/// typographic characters in `text` with plain ASCII equivalents.
///
/// - `em_dash`: how em dashes (—) and horizontal bars (―) are rendered.
/// - `normalize_spaces`: when `true`, exotic Unicode spaces (non-breaking, thin,
///   ideographic, …) become a regular ASCII space and zero-width characters are
///   removed; when `false` they are left as-is.
///
/// All other characters — including ordinary accented Latin, CJK, and emoji —
/// pass through unchanged.
pub fn clean(text: &str, em_dash: EmDash, normalize_spaces: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            // ── Double quotes ──────────────────────────────────────────────
            '\u{201C}' // “ left double quotation mark
            | '\u{201D}' // ” right double quotation mark
            | '\u{201E}' // „ double low-9
            | '\u{201F}' // ‟ double high-reversed-9
            | '\u{275D}' // ❝ heavy double turned comma
            | '\u{275E}' // ❞ heavy double comma
            | '\u{2033}' // ″ double prime
            | '\u{301D}' // 〝 reversed double prime quotation mark
            | '\u{301E}' // 〞 double prime quotation mark
            | '\u{00AB}' // « left guillemet
            | '\u{00BB}' // » right guillemet
            => out.push('"'),

            // ── Single quotes / apostrophes ────────────────────────────────
            '\u{2018}' // ‘ left single quotation mark
            | '\u{2019}' // ’ right single quotation mark
            | '\u{201A}' // ‚ single low-9
            | '\u{201B}' // ‛ single high-reversed-9
            | '\u{275B}' // ❛ heavy single turned comma
            | '\u{275C}' // ❜ heavy single comma
            | '\u{2032}' // ′ prime
            | '\u{02B9}' // ʹ modifier letter prime
            | '\u{02BC}' // ʼ modifier letter apostrophe
            | '\u{2039}' // ‹ single left guillemet
            | '\u{203A}' // › single right guillemet
            => out.push('\''),

            // ── Dashes / minus ─────────────────────────────────────────────
            '\u{2010}' // ‐ hyphen
            | '\u{2011}' // ‑ non-breaking hyphen
            | '\u{2012}' // ‒ figure dash
            | '\u{2013}' // – en dash
            | '\u{2212}' // − minus sign
            => out.push('-'),
            '\u{2014}' // — em dash
            | '\u{2015}' // ― horizontal bar
            => out.push_str(em_dash.as_str()),

            // ── Ellipsis ───────────────────────────────────────────────────
            '\u{2026}' => out.push_str("..."), // … horizontal ellipsis

            // ── Spaces (optional) ──────────────────────────────────────────
            '\u{00A0}' // no-break space
            | '\u{1680}' // ogham space mark
            | '\u{2000}'..='\u{200A}' // en/em/thin/hair/… spaces
            | '\u{202F}' // narrow no-break space
            | '\u{205F}' // medium mathematical space
            | '\u{3000}' // ideographic space
                if normalize_spaces =>
            {
                out.push(' ')
            }
            // Zero-width characters: drop entirely when normalizing.
            '\u{200B}' // zero-width space
            | '\u{200C}' // zero-width non-joiner
            | '\u{200D}' // zero-width joiner
            | '\u{2060}' // word joiner
            | '\u{FEFF}' // zero-width no-break space / BOM
                if normalize_spaces => {}

            // Everything else is preserved verbatim.
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> String {
        clean(s, EmDash::DoubleHyphen, true)
    }

    #[test]
    fn straightens_double_and_single_quotes() {
        assert_eq!(c("\u{201C}Hello,\u{201D} she said."), "\"Hello,\" she said.");
        assert_eq!(c("It\u{2019}s a \u{2018}test\u{2019}"), "It's a 'test'");
    }

    #[test]
    fn em_dash_options() {
        let s = "wait\u{2014}what";
        assert_eq!(clean(s, EmDash::DoubleHyphen, true), "wait--what");
        assert_eq!(clean(s, EmDash::Hyphen, true), "wait-what");
        assert_eq!(clean(s, EmDash::SpacedHyphen, true), "wait - what");
    }

    #[test]
    fn en_dash_and_minus_become_hyphen() {
        assert_eq!(c("2010\u{2013}2020"), "2010-2020");
        assert_eq!(c("5 \u{2212} 3"), "5 - 3");
        assert_eq!(c("non\u{2011}breaking"), "non-breaking");
    }

    #[test]
    fn ellipsis_glyph_becomes_three_dots() {
        assert_eq!(c("Wait\u{2026}"), "Wait...");
    }

    #[test]
    fn prime_marks_and_guillemets() {
        assert_eq!(c("5\u{2032}6\u{2033}"), "5'6\"");
        assert_eq!(c("\u{00AB}bonjour\u{00BB}"), "\"bonjour\"");
        assert_eq!(c("\u{2039}x\u{203A}"), "'x'");
    }

    #[test]
    fn normalize_spaces_folds_exotic_spaces_and_drops_zero_width() {
        // non-breaking + thin space → regular space; zero-width space removed.
        assert_eq!(c("a\u{00A0}b\u{2009}c\u{200B}d"), "a b cd");
        // BOM at the start is stripped.
        assert_eq!(c("\u{FEFF}hello"), "hello");
    }

    #[test]
    fn spaces_preserved_when_normalize_disabled() {
        let s = "a\u{00A0}b\u{200B}c";
        assert_eq!(clean(s, EmDash::DoubleHyphen, false), s);
    }

    #[test]
    fn ordinary_unicode_is_left_untouched() {
        // Accents, CJK, and emoji are NOT transliterated — only typographic
        // punctuation/spaces are touched.
        assert_eq!(c("Crème café 北京 🚀"), "Crème café 北京 🚀");
    }

    #[test]
    fn plain_ascii_is_unchanged() {
        let s = "Plain \"ASCII\" text -- with dots... and 'quotes'.";
        assert_eq!(c(s), s);
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert_eq!(c(""), "");
    }

    #[test]
    fn em_dash_parse_falls_back_to_default() {
        assert_eq!(EmDash::parse("--"), EmDash::DoubleHyphen);
        assert_eq!(EmDash::parse("-"), EmDash::Hyphen);
        assert_eq!(EmDash::parse(" - "), EmDash::SpacedHyphen);
        assert_eq!(EmDash::parse(""), EmDash::DoubleHyphen);
        assert_eq!(EmDash::parse("garbage"), EmDash::DoubleHyphen);
    }
}
