//! morse-code core — convert text to International Morse code and back. No
//! wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.
//!
//! Supports the standard International Morse alphabet (A–Z, 0–9, and the common
//! punctuation in ITU-R M.1677-1), case-insensitive on encode, with configurable
//! letter and word separators so the output matches whatever convention you want
//! (the defaults are a single space between letters and ` / ` between words).

/// One (uppercase character, Morse code) entry of the International Morse table.
/// Letters, digits, then the punctuation/prosigns from ITU-R M.1677-1 plus a few
/// in-everyday-use extras (`@`, `&`, `$`, `!`, `_`).
const TABLE: &[(char, &str)] = &[
    // Letters
    ('A', ".-"),
    ('B', "-..."),
    ('C', "-.-."),
    ('D', "-.."),
    ('E', "."),
    ('F', "..-."),
    ('G', "--."),
    ('H', "...."),
    ('I', ".."),
    ('J', ".---"),
    ('K', "-.-"),
    ('L', ".-.."),
    ('M', "--"),
    ('N', "-."),
    ('O', "---"),
    ('P', ".--."),
    ('Q', "--.-"),
    ('R', ".-."),
    ('S', "..."),
    ('T', "-"),
    ('U', "..-"),
    ('V', "...-"),
    ('W', ".--"),
    ('X', "-..-"),
    ('Y', "-.--"),
    ('Z', "--.."),
    // Digits
    ('0', "-----"),
    ('1', ".----"),
    ('2', "..---"),
    ('3', "...--"),
    ('4', "....-"),
    ('5', "....."),
    ('6', "-...."),
    ('7', "--..."),
    ('8', "---.."),
    ('9', "----."),
    // Punctuation (ITU-R M.1677-1 + common extras)
    ('.', ".-.-.-"),
    (',', "--..--"),
    ('?', "..--.."),
    ('\'', ".----."),
    ('!', "-.-.--"),
    ('/', "-..-."),
    ('(', "-.--."),
    (')', "-.--.-"),
    ('&', ".-..."),
    (':', "---..."),
    (';', "-.-.-."),
    ('=', "-...-"),
    ('+', ".-.-."),
    ('-', "-....-"),
    ('_', "..--.-"),
    ('"', ".-..-."),
    ('$', "...-..-"),
    ('@', ".--.-."),
];

/// The direction of conversion.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direction {
    Encode,
    Decode,
}

impl Direction {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            // Blank defaults to encode (text -> morse). Accept friendly aliases.
            "" | "encode" | "text-to-morse" | "text2morse" => Ok(Direction::Encode),
            "decode" | "morse-to-text" | "morse2text" => Ok(Direction::Decode),
            other => Err(format!(
                "invalid direction {other:?}: expected \"encode\" (text to Morse) or \"decode\" (Morse to text)"
            )),
        }
    }
}

/// Look up the Morse code for an uppercase character.
fn char_to_morse(c: char) -> Option<&'static str> {
    TABLE.iter().find(|(ch, _)| *ch == c).map(|(_, code)| *code)
}

/// Look up the character for a Morse code token.
fn morse_to_char(code: &str) -> Option<char> {
    TABLE.iter().find(|(_, m)| *m == code).map(|(ch, _)| *ch)
}

/// Resolve the effective letter / word separators, applying the defaults when a
/// caller passes a blank string. Defaults: a single space between letters, ` / `
/// (space-slash-space) between words — the most common on-air/text convention.
fn separators(letter_sep: &str, word_sep: &str) -> (String, String) {
    let l = if letter_sep.is_empty() {
        " ".to_string()
    } else {
        letter_sep.to_string()
    };
    let w = if word_sep.is_empty() {
        " / ".to_string()
    } else {
        word_sep.to_string()
    };
    (l, w)
}

/// Encode plain `text` into Morse code.
///
/// - Each character is uppercased and looked up in the International Morse table.
/// - Letters within a word are joined by `letter_sep` (default: a single space).
/// - Words (runs separated by ASCII whitespace in the input) are joined by
///   `word_sep` (default: ` / `).
/// - Unsupported characters are replaced by the Morse code for `?` in the output,
///   so the result stays decodable; this is the conventional "unknown" placeholder.
fn encode(text: &str, letter_sep: &str, word_sep: &str) -> Result<String, String> {
    let (letter_sep, word_sep) = separators(letter_sep, word_sep);

    let words: Vec<String> = text
        .split_whitespace()
        .map(|word| {
            word.chars()
                .map(|c| {
                    let upper = c.to_ascii_uppercase();
                    // Fall back to '?' for anything not in the table.
                    char_to_morse(upper).or_else(|| char_to_morse('?')).unwrap()
                })
                .collect::<Vec<_>>()
                .join(&letter_sep)
        })
        .collect();

    Ok(words.join(&word_sep))
}

/// Decode `morse` back into plain text.
///
/// The input is split into words on `word_sep`, then each word is split into
/// tokens on `letter_sep`; every token is looked up in the Morse table. Words are
/// rejoined with a single space. An unrecognised Morse token is an error (Morse is
/// ambiguous, so silently dropping tokens would corrupt the message). A `_` is
/// accepted as a dash alias for convenience.
fn decode(morse: &str, letter_sep: &str, word_sep: &str) -> Result<String, String> {
    let (letter_sep, word_sep) = separators(letter_sep, word_sep);

    // Normalise common dash aliases so "_" works as a dash.
    let normalized = morse.replace('_', "-");

    let mut out_words: Vec<String> = Vec::new();
    for word in split_nonempty(&normalized, &word_sep) {
        let mut letters = String::new();
        for token in split_nonempty(word, &letter_sep) {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            match morse_to_char(token) {
                Some(c) => letters.push(c),
                None => {
                    return Err(format!(
                        "invalid Morse token {token:?}: not a recognised code (use '.' and '-', letters separated by {letter_sep:?}, words by {word_sep:?})"
                    ))
                }
            }
        }
        if !letters.is_empty() {
            out_words.push(letters);
        }
    }

    Ok(out_words.join(" "))
}

/// Split `s` on `sep`, dropping empty pieces. When `sep` is a single space, splits
/// on any whitespace (so runs of spaces collapse — the natural reading of Morse).
fn split_nonempty<'a>(s: &'a str, sep: &str) -> Vec<&'a str> {
    if sep == " " {
        s.split_whitespace().collect()
    } else {
        s.split(sep).filter(|p| !p.trim().is_empty()).collect()
    }
}

/// Convert `text` to or from Morse code.
///
/// - `direction` (`"encode"` | `"decode"`, blank → `"encode"`): `encode` turns
///   plain text into Morse; `decode` turns Morse back into text.
/// - `letter_sep` (blank → a single space): the separator between letters/symbols.
/// - `word_sep` (blank → ` / `): the separator between words.
///
/// Returns `Err` on an invalid `direction` or an unrecognised Morse token on decode.
pub fn convert(
    text: &str,
    direction: &str,
    letter_sep: &str,
    word_sep: &str,
) -> Result<String, String> {
    match Direction::parse(direction)? {
        Direction::Encode => encode(text, letter_sep, word_sep),
        Direction::Decode => decode(text, letter_sep, word_sep),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_sos_default() {
        assert_eq!(convert("SOS", "encode", "", "").unwrap(), "... --- ...");
    }

    #[test]
    fn encode_defaults_when_blank_direction() {
        // Blank direction defaults to encode.
        assert_eq!(convert("E", "", "", "").unwrap(), ".");
    }

    #[test]
    fn encode_is_case_insensitive() {
        assert_eq!(
            convert("Hello", "encode", "", "").unwrap(),
            ".... . .-.. .-.. ---"
        );
    }

    #[test]
    fn encode_multiple_words_uses_word_separator() {
        assert_eq!(
            convert("HI BYE", "encode", "", "").unwrap(),
            ".... .. / -... -.-- ."
        );
    }

    #[test]
    fn encode_digits_and_punctuation() {
        assert_eq!(
            convert("AB12.", "encode", "", "").unwrap(),
            ".- -... .---- ..--- .-.-.-"
        );
    }

    #[test]
    fn encode_unknown_char_becomes_question_mark_code() {
        // '€' isn't in the table → replaced by the code for '?'.
        assert_eq!(convert("A€", "encode", "", "").unwrap(), ".- ..--..");
    }

    #[test]
    fn decode_sos_default() {
        assert_eq!(convert("... --- ...", "decode", "", "").unwrap(), "SOS");
    }

    #[test]
    fn decode_multiple_words() {
        assert_eq!(
            convert(".... .. / -... -.-- .", "decode", "", "").unwrap(),
            "HI BYE"
        );
    }

    #[test]
    fn decode_accepts_underscore_as_dash() {
        // "_" is normalised to "-", so "_..._" style dashes decode.
        assert_eq!(convert(".._ / ._", "decode", "", "").unwrap(), "U A");
    }

    #[test]
    fn decode_rejects_invalid_token() {
        let err = convert("........", "decode", "", "").unwrap_err();
        assert!(err.contains("invalid Morse token"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_direction() {
        let err = convert("x", "sideways", "", "").unwrap_err();
        assert!(err.contains("invalid direction"), "got: {err}");
    }

    #[test]
    fn custom_separators_encode() {
        // Pipe between letters, double-slash between words.
        assert_eq!(
            convert("HI YO", "encode", "|", "//").unwrap(),
            "....|..//-.--|---"
        );
    }

    #[test]
    fn custom_separators_decode() {
        assert_eq!(
            convert("....|..//-.--|---", "decode", "|", "//").unwrap(),
            "HI YO"
        );
    }

    #[test]
    fn round_trip_encode_then_decode() {
        let original = "THE QUICK BROWN FOX 123";
        let morse = convert(original, "encode", "", "").unwrap();
        let back = convert(&morse, "decode", "", "").unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn round_trip_with_punctuation() {
        let original = "HELLO, WORLD!";
        let morse = convert(original, "encode", "", "").unwrap();
        let back = convert(&morse, "decode", "", "").unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn round_trip_custom_separators() {
        let original = "GIZZA AI";
        let morse = convert(original, "encode", "/", "|").unwrap();
        let back = convert(&morse, "decode", "/", "|").unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn empty_input_encodes_to_empty() {
        assert_eq!(convert("", "encode", "", "").unwrap(), "");
        assert_eq!(convert("   ", "encode", "", "").unwrap(), "");
    }

    #[test]
    fn empty_input_decodes_to_empty() {
        assert_eq!(convert("", "decode", "", "").unwrap(), "");
    }
}
