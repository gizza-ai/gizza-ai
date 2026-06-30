//! unicode-to-text core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! A forgiving, zero-config decoder: it scans the input and turns every common
//! Unicode escape notation back into the actual character, leaving any text that
//! isn't an escape untouched. Recognized notations (auto-detected together, in a
//! single pass — you never pick a "target"):
//!   * `\uXXXX`            — 4-hex JS/JSON/Java escape (surrogate pairs combined)
//!   * `\u{X..}`           — braced Rust/ES6 escape, 1-6 hex
//!   * `\xXX`              — 2-hex byte/Latin-1 escape
//!   * `\U00XXXXXX`        — 8-hex Python escape
//!   * `U+XXXX` / `u+xxxx` — Unicode code-point notation, 1-6 hex
//!   * `&#DDDD;`           — HTML decimal numeric character reference
//!   * `&#xHHHH;` / `&#X…;` — HTML hexadecimal numeric character reference
//!
//! Invalid code points (e.g. a lone surrogate) decode to the Unicode replacement
//! character U+FFFD. Anything that merely looks like the start of an escape but
//! doesn't complete a valid one is passed through verbatim.

/// Decode every recognized Unicode escape sequence in `text` back into the
/// characters it represents. Returns an error only for empty input.
pub fn decode(text: &str) -> Result<String, String> {
    if text.is_empty() {
        return Err("input text is empty".into());
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let n = chars.len();
    while i < n {
        let c = chars[i];
        match c {
            '\\' if i + 1 < n => {
                if let Some((decoded, consumed)) = decode_backslash(&chars, i) {
                    out.push_str(&decoded);
                    i += consumed;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            'u' | 'U' if i + 1 < n && chars[i + 1] == '+' => {
                if let Some((cp, consumed)) = parse_hex_run(&chars, i + 2, 6) {
                    out.push(from_cp(cp));
                    i += 2 + consumed;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            '&' if i + 1 < n && chars[i + 1] == '#' => {
                if let Some((cp, consumed)) = parse_html_numeric(&chars, i) {
                    out.push(from_cp(cp));
                    i += consumed;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(out)
}

/// Map a (possibly invalid) code point to a char, falling back to U+FFFD.
fn from_cp(cp: u32) -> char {
    char::from_u32(cp).unwrap_or('\u{FFFD}')
}

/// Parse up to `max` hex digits starting at `start`. Returns the value and the
/// number of hex chars consumed, or None if there isn't at least one hex digit.
fn parse_hex_run(chars: &[char], start: usize, max: usize) -> Option<(u32, usize)> {
    let mut val: u32 = 0;
    let mut count = 0;
    while start + count < chars.len() && count < max {
        match chars[start + count].to_digit(16) {
            Some(d) => {
                val = val.checked_mul(16)?.checked_add(d)?;
                count += 1;
            }
            None => break,
        }
    }
    if count == 0 {
        None
    } else {
        Some((val, count))
    }
}

/// Parse exactly `len` hex digits starting at `start`.
fn parse_hex_fixed(chars: &[char], start: usize, len: usize) -> Option<u32> {
    if start + len > chars.len() {
        return None;
    }
    let mut val: u32 = 0;
    for k in 0..len {
        val = val.checked_mul(16)?.checked_add(chars[start + k].to_digit(16)?)?;
    }
    Some(val)
}

/// Decode a backslash escape at index `i` (where `chars[i] == '\\'`). Returns the
/// decoded text and the total chars consumed (including the backslash), or None.
fn decode_backslash(chars: &[char], i: usize) -> Option<(String, usize)> {
    let next = chars[i + 1];
    match next {
        'u' => {
            // \u{X..}
            if i + 2 < chars.len() && chars[i + 2] == '{' {
                let mut j = i + 3;
                let mut val: u32 = 0;
                let mut count = 0;
                while j < chars.len() && chars[j] != '}' {
                    val = val.checked_mul(16)?.checked_add(chars[j].to_digit(16)?)?;
                    count += 1;
                    j += 1;
                }
                if count == 0 || j >= chars.len() {
                    return None; // empty braces or unterminated
                }
                // j points at the closing '}'
                return Some((from_cp(val).to_string(), j - i + 1));
            }
            // \uXXXX (exactly 4 hex), with surrogate-pair combination
            let hi = parse_hex_fixed(chars, i + 2, 4)?;
            if (0xD800..=0xDBFF).contains(&hi) {
                // Look for a following \uXXXX low surrogate to combine.
                let lo_start = i + 6;
                if lo_start + 1 < chars.len()
                    && chars[lo_start] == '\\'
                    && chars[lo_start + 1] == 'u'
                {
                    if let Some(lo) = parse_hex_fixed(chars, lo_start + 2, 4) {
                        if (0xDC00..=0xDFFF).contains(&lo) {
                            let cp = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                            return Some((from_cp(cp).to_string(), 12));
                        }
                    }
                }
            }
            Some((from_cp(hi).to_string(), 6))
        }
        'x' => {
            // \xXX (exactly 2 hex)
            let v = parse_hex_fixed(chars, i + 2, 2)?;
            Some((from_cp(v).to_string(), 4))
        }
        'U' => {
            // \U00XXXXXX (exactly 8 hex, Python wide escape)
            let v = parse_hex_fixed(chars, i + 2, 8)?;
            Some((from_cp(v).to_string(), 10))
        }
        _ => None,
    }
}

/// Parse an HTML numeric character reference at index `i` (`chars[i] == '&'`,
/// `chars[i+1] == '#'`). Returns the code point and total chars consumed
/// (including the trailing `;`), or None if malformed/unterminated.
fn parse_html_numeric(chars: &[char], i: usize) -> Option<(u32, usize)> {
    let mut j = i + 2;
    let hex = j < chars.len() && (chars[j] == 'x' || chars[j] == 'X');
    if hex {
        j += 1;
    }
    let base: u32 = if hex { 16 } else { 10 };
    let mut acc: u32 = 0;
    let mut count = 0;
    while j < chars.len() && chars[j] != ';' {
        let d = chars[j].to_digit(base)?;
        acc = acc.checked_mul(base)?.checked_add(d)?;
        count += 1;
        j += 1;
    }
    if count == 0 || j >= chars.len() || chars[j] != ';' {
        return None; // no digits or unterminated
    }
    // j points at the closing ';'
    Some((acc, j - i + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_basic_u_escape() {
        // \uXXXX 4-hex escape (JS/JSON/Java).
        assert_eq!(decode(r"caf\u00e9").unwrap(), "café");
        assert_eq!(decode(r"\u0041\u0042").unwrap(), "AB");
    }

    #[test]
    fn decodes_braced_escape() {
        assert_eq!(decode(r"\u{1F600}").unwrap(), "😀");
        assert_eq!(decode(r"\u{41}").unwrap(), "A");
    }

    #[test]
    fn combines_surrogate_pair() {
        assert_eq!(decode(r"\uD83D\uDE00").unwrap(), "😀");
    }

    #[test]
    fn decodes_u_plus_notation() {
        assert_eq!(decode("U+0041U+1F600").unwrap(), "A😀");
        assert_eq!(decode("u+00e9").unwrap(), "é");
    }

    #[test]
    fn decodes_html_numeric() {
        assert_eq!(decode("&#65;&#x42;").unwrap(), "AB");
        assert_eq!(decode("&#128512;").unwrap(), "😀");
        assert_eq!(decode("&#X1F600;").unwrap(), "😀");
    }

    #[test]
    fn decodes_x_and_wide_escapes() {
        assert_eq!(decode(r"\x41").unwrap(), "A");
        assert_eq!(decode(r"\U0001F600").unwrap(), "😀");
    }

    #[test]
    fn mixed_notations_and_passthrough() {
        let input = r"AB U+0043 &#68; plain";
        assert_eq!(decode(input).unwrap(), "AB C D plain");
    }

    #[test]
    fn passes_through_plain_text() {
        assert_eq!(decode("just text, no escapes").unwrap(), "just text, no escapes");
    }

    #[test]
    fn leaves_incomplete_escapes_verbatim() {
        // Not enough hex digits / unterminated -> pass through unchanged.
        assert_eq!(decode(r"\uZZZZ").unwrap(), r"\uZZZZ");
        assert_eq!(decode(r"\u{").unwrap(), r"\u{");
        assert_eq!(decode("&#;").unwrap(), "&#;");
        assert_eq!(decode("&#12").unwrap(), "&#12");
        assert_eq!(decode("U+").unwrap(), "U+");
    }

    #[test]
    fn lone_surrogate_becomes_replacement() {
        assert_eq!(decode(r"\uD83D").unwrap(), "\u{FFFD}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(decode("").is_err());
    }

    #[test]
    fn does_not_touch_backslash_n() {
        // \n is a string escape, not a Unicode escape — left for string-escaper.
        assert_eq!(decode(r"line\nbreak").unwrap(), r"line\nbreak");
    }
}
