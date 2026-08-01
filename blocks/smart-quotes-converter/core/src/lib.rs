//! smart-quotes-converter core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Two directions:
//!  - "educate" straight ASCII quotes (`"` `'`) into curly typographic quotes
//!    (`“ ” ‘ ’`), choosing opening vs closing from the surrounding characters,
//!    the way word processors and SmartyPants do; and
//!  - the reverse — "straighten" curly quotes (and prime marks) back to plain
//!    ASCII, which is what you want before pasting into code, CSV, or JSON.
//!
//! Double quotes and single quotes/apostrophes are independently toggleable, and
//! an optional feet-and-inches mode turns digit-adjacent straight quotes into
//! true prime marks (`6'4"` → `6′4″`). Everything else — accents, CJK, emoji,
//! dashes, ellipses — passes through untouched.

/// Which way to convert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Straight ASCII quotes → curly typographic quotes (the default).
    ToCurly,
    /// Curly quotes (and prime marks) → straight ASCII quotes.
    ToStraight,
}

impl Direction {
    /// Parse the option string (as sent by the chat schema / page select).
    /// Blank or unrecognised falls back to the default `to_curly`.
    pub fn parse(s: &str) -> Direction {
        match s.trim() {
            "to_straight" | "straight" | "straighten" => Direction::ToStraight,
            // "to_curly", "curly", "", and anything else → the default.
            _ => Direction::ToCurly,
        }
    }
}

// Curly / typographic characters.
const LDQUO: char = '\u{201C}'; // “ left double quotation mark
const RDQUO: char = '\u{201D}'; // ” right double quotation mark
const LSQUO: char = '\u{2018}'; // ‘ left single quotation mark
const RSQUO: char = '\u{2019}'; // ’ right single quotation mark (also apostrophe)
const PRIME: char = '\u{2032}'; // ′ prime (feet / minutes)
const DPRIME: char = '\u{2033}'; // ″ double prime (inches / seconds)

/// Options controlling the conversion.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub direction: Direction,
    /// Convert double quotes (`"` ⇄ `“ ”`). Default true.
    pub convert_double: bool,
    /// Convert single quotes and apostrophes (`'` ⇄ `‘ ’`). Default true.
    pub convert_single: bool,
    /// Feet-and-inches mode. In `to_curly`, a straight quote directly after a
    /// digit becomes a prime (`′`/`″`) instead of a curly quote, so `6'4"`
    /// becomes `6′4″`. Default false. (In `to_straight`, prime marks are always
    /// straightened regardless of this flag.)
    pub feet_inches: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            direction: Direction::ToCurly,
            convert_double: true,
            convert_single: true,
            feet_inches: false,
        }
    }
}

/// Largest input accepted, in bytes. Comfortably covers a long document while
/// keeping a single conversion instant in the browser.
pub const MAX_BYTES: usize = 1_000_000;

/// Convert the quotes in `text` per `opts`.
///
/// Returns `Err` only when `text` exceeds [`MAX_BYTES`]; the conversion itself
/// never fails.
pub fn convert(text: &str, opts: Options) -> Result<String, String> {
    if text.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes; the maximum is {} bytes (~1 MB)",
            text.len(),
            MAX_BYTES
        ));
    }
    Ok(match opts.direction {
        Direction::ToCurly => to_curly(text, opts),
        Direction::ToStraight => to_straight(text, opts),
    })
}

/// A straight quote opens (rather than closes) when the character before it is
/// the start of the text, whitespace, or an opening bracket / dash — i.e. there
/// is nothing "word-like" immediately to its left for it to close.
fn opens_after(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) => {
            c.is_whitespace()
                || matches!(
                    c,
                    '(' | '[' | '{' | '<'
                        | '\u{2018}' | '\u{201C}' // ‘ “ already-curly openers
                        | '\u{2013}' | '\u{2014}' // – — en/em dash
                        | '-' | '/' | '\u{00A1}' | '\u{00BF}' // - / ¡ ¿
                )
        }
    }
}

fn to_curly(text: &str, opts: Options) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        let prev = if i == 0 { None } else { Some(chars[i - 1]) };
        let next = chars.get(i + 1).copied();
        match c {
            '"' if opts.convert_double => {
                if opts.feet_inches && prev.map(|p| p.is_ascii_digit()).unwrap_or(false) {
                    out.push(DPRIME); // inches: 6'4" → …″
                } else if opens_after(prev) {
                    out.push(LDQUO);
                } else {
                    out.push(RDQUO);
                }
            }
            '\'' if opts.convert_single => {
                let prev_digit = prev.map(|p| p.is_ascii_digit()).unwrap_or(false);
                if opts.feet_inches && prev_digit {
                    out.push(PRIME); // feet: 6' → 6′
                } else if prev
                    .map(|p| p.is_alphanumeric())
                    .unwrap_or(false)
                {
                    // After a letter/digit → apostrophe or closing quote (it’s, dogs’).
                    out.push(RSQUO);
                } else if opens_after(prev) {
                    // Boundary position. A digit right after (’89) is an elided
                    // year → apostrophe; otherwise an opening single quote.
                    if next.map(|n| n.is_ascii_digit()).unwrap_or(false) {
                        out.push(RSQUO);
                    } else {
                        out.push(LSQUO);
                    }
                } else {
                    out.push(RSQUO);
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn to_straight(text: &str, opts: Options) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let replaced = match ch {
            LDQUO | RDQUO | DPRIME if opts.convert_double => Some('"'),
            LSQUO | RSQUO | PRIME if opts.convert_single => Some('\''),
            _ => None,
        };
        match replaced {
            Some(r) => out.push(r),
            None => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curly(text: &str) -> String {
        convert(text, Options::default()).unwrap()
    }

    #[test]
    fn double_quotes_open_and_close() {
        assert_eq!(curly("\"Hello,\" she said."), "\u{201C}Hello,\u{201D} she said.");
    }

    #[test]
    fn apostrophe_and_single_quotes() {
        assert_eq!(curly("It's a 'test'"), "It\u{2019}s a \u{2018}test\u{2019}");
    }

    #[test]
    fn elided_year_is_apostrophe_not_open_quote() {
        assert_eq!(curly("Back in '89"), "Back in \u{2019}89");
    }

    #[test]
    fn possessive_plural_apostrophe() {
        assert_eq!(curly("the dogs' bowls"), "the dogs\u{2019} bowls");
    }

    #[test]
    fn feet_inches_off_by_default_uses_curly() {
        // Default: no primes; digit-adjacent quotes still curl.
        assert_eq!(curly("6'4\""), "6\u{2019}4\u{201D}");
    }

    #[test]
    fn feet_inches_mode_uses_primes() {
        let opts = Options {
            feet_inches: true,
            ..Options::default()
        };
        assert_eq!(convert("6'4\"", opts).unwrap(), "6\u{2032}4\u{2033}");
    }

    #[test]
    fn straighten_reverses_curly_and_primes() {
        let opts = Options {
            direction: Direction::ToStraight,
            ..Options::default()
        };
        assert_eq!(
            convert("\u{201C}It\u{2019}s 6\u{2032}4\u{2033}\u{201D}", opts).unwrap(),
            "\"It's 6'4\"\""
        );
    }

    #[test]
    fn toggle_leaves_the_other_kind_untouched() {
        // Only single quotes convert; double quotes stay straight.
        let opts = Options {
            convert_double: false,
            ..Options::default()
        };
        assert_eq!(convert("\"It's\"", opts).unwrap(), "\"It\u{2019}s\"");
    }

    #[test]
    fn non_quote_unicode_passes_through() {
        // Accents, CJK, emoji, dashes, and ellipses are untouched.
        assert_eq!(curly("café 北京 🙂 — …"), "café 北京 🙂 — …");
    }

    #[test]
    fn empty_input_is_empty() {
        assert_eq!(curly(""), "");
    }

    #[test]
    fn at_the_cap_succeeds() {
        let text = "a".repeat(MAX_BYTES);
        assert!(convert(&text, Options::default()).is_ok());
    }

    #[test]
    fn over_the_cap_errors() {
        let text = "a".repeat(MAX_BYTES + 1);
        let err = convert(&text, Options::default()).unwrap_err();
        assert!(err.contains("maximum"), "unexpected error: {err}");
    }
}
