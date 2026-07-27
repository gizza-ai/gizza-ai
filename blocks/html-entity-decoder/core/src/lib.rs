//! html-entity-decoder core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Decodes HTML character references back to their literal characters:
//!   * named entities across the full HTML5 set (`&amp;` `&copy;` `&mdash;`
//!     `&hellip;` `&nbsp;` …) via the `entities` crate's WHATWG table,
//!   * decimal numeric references (`&#169;`),
//!   * hexadecimal numeric references (`&#xA9;` / `&#XA9;`),
//!   * WHATWG legacy entities that omit the trailing `;` (`&copy` → ©), matched
//!     by longest known prefix,
//! applying the WHATWG numeric rules: the Windows-1252 remap for code points
//! 0x80–0x9F (so `&#151;` → em dash, `&#128;` → €) and U+FFFD for null,
//! surrogate, or out-of-range code points.
//!
//! Unknown / malformed references are handled by the `unknown` mode: `keep`
//! (default) leaves them exactly as written, `error` reports the offender.

use std::collections::HashMap;
use std::sync::OnceLock;

/// What to do when a reference can't be decoded (unknown named entity or a
/// malformed numeric reference like `&#;`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Unknown {
    /// Leave the text exactly as written (WHATWG-style lenient default).
    Keep,
    /// Return an error naming the offending reference.
    Error,
}

impl Unknown {
    pub fn parse(s: &str) -> Result<Unknown, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "keep" | "leave" | "lenient" => Ok(Unknown::Keep),
            "error" | "strict" | "fail" => Ok(Unknown::Error),
            other => Err(format!(
                "unknown value '{other}' for 'unknown' (use 'keep' or 'error')"
            )),
        }
    }
}

/// Longest ASCII-alnum entity name we'll consider (the longest real name,
/// `CounterClockwiseContourIntegral`, is 31 chars).
const MAX_NAME: usize = 32;

/// Lookup keyed by the entity name WITHOUT the leading `&`, e.g. `"copy;"` and
/// (for legacy entities) `"copy"`. Built once from the `entities` crate table.
fn entity_map() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::with_capacity(entities::ENTITIES.len());
        for e in entities::ENTITIES.iter() {
            if let Some(name) = e.entity.strip_prefix('&') {
                m.insert(name, e.characters);
            }
        }
        m
    })
}

/// WHATWG numeric-reference remap for the C1 range 0x80–0x9F (these code points
/// are interpreted as Windows-1252 bytes, matching every browser). Any code not
/// listed passes through unchanged.
fn remap_c1(code: u32) -> Option<char> {
    let mapped = match code {
        0x80 => 0x20AC, // €
        0x82 => 0x201A,
        0x83 => 0x0192,
        0x84 => 0x201E,
        0x85 => 0x2026, // …
        0x86 => 0x2020,
        0x87 => 0x2021,
        0x88 => 0x02C6,
        0x89 => 0x2030,
        0x8A => 0x0160,
        0x8B => 0x2039,
        0x8C => 0x0152, // Œ
        0x8E => 0x017D,
        0x91 => 0x2018, // ‘
        0x92 => 0x2019, // ’
        0x93 => 0x201C, // “
        0x94 => 0x201D, // ”
        0x95 => 0x2022, // •
        0x96 => 0x2013, // –
        0x97 => 0x2014, // —
        0x98 => 0x02DC,
        0x99 => 0x2122, // ™
        0x9A => 0x0161,
        0x9B => 0x203A,
        0x9C => 0x0153, // œ
        0x9E => 0x017E,
        0x9F => 0x0178, // Ÿ
        _ => return None,
    };
    char::from_u32(mapped)
}

/// Map a numeric code point to its character per the WHATWG rules.
fn numeric_char(code: u32) -> char {
    if code == 0 || code > 0x10_FFFF || (0xD800..=0xDFFF).contains(&code) {
        return '\u{FFFD}';
    }
    if let Some(c) = remap_c1(code) {
        return c;
    }
    char::from_u32(code).unwrap_or('\u{FFFD}')
}

/// Decode every HTML character reference in `text`.
///
/// `unknown` selects how unrecognized references are handled: `"keep"` (default)
/// or `"error"`. Returns the decoded string, or an error string when `error`
/// mode meets a reference it can't decode.
pub fn decode(text: &str, unknown: &str) -> Result<String, String> {
    let mode = Unknown::parse(unknown)?;
    let map = entity_map();
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '&' {
            out.push(c);
            continue;
        }

        // Numeric reference: &# followed by decimal, or &#x / &#X followed by hex.
        if chars.peek() == Some(&'#') {
            chars.next(); // consume '#'
            let hex = matches!(chars.peek(), Some('x') | Some('X'));
            if hex {
                chars.next(); // consume 'x'/'X'
            }
            let mut digits = String::new();
            while let Some(&n) = chars.peek() {
                let ok = if hex {
                    n.is_ascii_hexdigit()
                } else {
                    n.is_ascii_digit()
                };
                if ok && digits.len() < 8 {
                    digits.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            if digits.is_empty() {
                // Malformed: "&#" / "&#x" with no digits.
                if mode == Unknown::Error {
                    return Err(format!(
                        "malformed numeric reference '&#{}' (no digits)",
                        if hex { "x" } else { "" }
                    ));
                }
                out.push('&');
                out.push('#');
                if hex {
                    out.push('x');
                }
                continue;
            }
            // A trailing ';' is part of the reference when present (optional per WHATWG).
            if chars.peek() == Some(&';') {
                chars.next();
            }
            let code = u32::from_str_radix(&digits, if hex { 16 } else { 10 })
                .unwrap_or(0x11_0000); // overlong digit run → out-of-range → U+FFFD
            out.push(numeric_char(code));
            continue;
        }

        // Named reference: gather the ASCII-alphanumeric name run.
        let mut name = String::new();
        while let Some(&n) = chars.peek() {
            if n.is_ascii_alphanumeric() && name.len() < MAX_NAME {
                name.push(n);
                chars.next();
            } else {
                break;
            }
        }
        if name.is_empty() {
            // Bare '&' (e.g. "Tom & Jerry") — emit literally.
            out.push('&');
            continue;
        }

        if chars.peek() == Some(&';') {
            // Well-formed "&name;": require an exact match.
            let key = format!("{name};");
            if let Some(&decoded) = map.get(key.as_str()) {
                chars.next(); // consume ';'
                out.push_str(decoded);
                continue;
            }
            // Unknown named entity.
            if mode == Unknown::Error {
                return Err(format!("unknown HTML entity '&{name};'"));
            }
            // Keep: leave "&name" as-is; the ';' is emitted by the next iteration.
            out.push('&');
            out.push_str(&name);
            continue;
        }

        // No trailing ';': only the WHATWG legacy set decodes, matched by the
        // longest known prefix (e.g. "&copytext" → "©text").
        let mut matched: Option<(usize, &'static str)> = None;
        for len in (1..=name.len()).rev() {
            if let Some(&decoded) = map.get(&name[..len]) {
                matched = Some((len, decoded));
                break;
            }
        }
        if let Some((len, decoded)) = matched {
            out.push_str(decoded);
            out.push_str(&name[len..]);
            continue;
        }
        if mode == Unknown::Error {
            return Err(format!("unknown HTML entity '&{name}'"));
        }
        out.push('&');
        out.push_str(&name);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_named_numeric_and_hex() {
        assert_eq!(
            decode("Fish &amp; Chips &mdash; &#169; 2026 &#xA9;", "keep").unwrap(),
            "Fish & Chips — © 2026 ©"
        );
    }

    #[test]
    fn decodes_common_named_entities() {
        assert_eq!(
            decode("&copy;&nbsp;&hellip;&trade;&rsquo;", "keep").unwrap(),
            "\u{00A9}\u{00A0}\u{2026}\u{2122}\u{2019}"
        );
    }

    #[test]
    fn hex_case_insensitive() {
        assert_eq!(decode("&#Xe9; &#xe9;", "keep").unwrap(), "é é");
    }

    #[test]
    fn c1_windows1252_remap() {
        // &#151; is an em dash (—), &#128; is the euro sign (€) under WHATWG.
        assert_eq!(decode("&#151;&#128;", "keep").unwrap(), "—€");
    }

    #[test]
    fn invalid_codepoints_become_replacement() {
        // null, a surrogate, and out-of-range all map to U+FFFD.
        assert_eq!(
            decode("&#0;&#xD800;&#x110000;", "keep").unwrap(),
            "\u{FFFD}\u{FFFD}\u{FFFD}"
        );
    }

    #[test]
    fn legacy_entity_without_semicolon() {
        // Legacy set decodes without ';'; the trailing text is preserved.
        assert_eq!(decode("&copyright", "keep").unwrap(), "\u{00A9}right");
        // A non-legacy name still needs its ';'.
        assert_eq!(decode("&mdash text", "keep").unwrap(), "&mdash text");
    }

    #[test]
    fn bare_ampersand_and_numeric_without_semicolon() {
        assert_eq!(decode("Tom & Jerry", "keep").unwrap(), "Tom & Jerry");
        assert_eq!(decode("&#169 plain", "keep").unwrap(), "© plain");
    }

    #[test]
    fn keep_mode_leaves_unknown_untouched() {
        assert_eq!(
            decode("a &notareal; b &#; c", "keep").unwrap(),
            "a &notareal; b &#; c"
        );
    }

    #[test]
    fn error_mode_rejects_unknown_named_entity() {
        let err = decode("hello &bogus; world", "error").unwrap_err();
        assert!(err.contains("&bogus;"), "got: {err}");
    }

    #[test]
    fn error_mode_rejects_malformed_numeric() {
        assert!(decode("bad &#; ref", "error").is_err());
    }

    #[test]
    fn rejects_unknown_mode_value() {
        assert!(decode("x", "sideways").is_err());
    }
}
