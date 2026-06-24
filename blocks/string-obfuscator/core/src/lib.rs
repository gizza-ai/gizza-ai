//! string-obfuscator core — masks or visually obfuscates a string for
//! screenshots, demos, and docs. Pure compute, no wafer/wasm-bindgen deps;
//! shared by the chat skill block and the web page.
//!
//! Four modes:
//! - `mask`: keep the first `keep_start` and last `keep_end` characters,
//!   replace every character in between with `mask_char` (default `*`).
//!   The classic "sk-1234••••••••cdef" / "j•••@example.com" redaction.
//! - `rot`: Caesar / ROT-N rotation of ASCII letters by `rot_n` (default 13,
//!   the self-inverse ROT13). Non-letters pass through unchanged.
//! - `leetspeak`: replace common letters with look-alike digits/symbols
//!   (a→4, e→3, i→1, o→0, s→5, t→7, …).
//! - `homoglyph`: replace ASCII letters with Unicode look-alikes (mostly
//!   Cyrillic) so the text looks the same but is a different string — useful
//!   for demoing homograph spoofing or defeating naive copy-paste.

/// A single obfuscation mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Mask,
    Rot,
    Leetspeak,
    Homoglyph,
}

impl Mode {
    /// Parse a mode name; blank defaults to `mask`.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "mask" => Ok(Mode::Mask),
            "rot" => Ok(Mode::Rot),
            "leetspeak" | "leet" => Ok(Mode::Leetspeak),
            "homoglyph" => Ok(Mode::Homoglyph),
            other => Err(format!(
                "invalid mode {other:?}: expected \"mask\", \"rot\", \"leetspeak\", or \"homoglyph\""
            )),
        }
    }
}

/// Maximum ROT shift the API exposes. ROT is mod-26 so any shift maps into
/// 0..=25; the schema still caps the accepted value for cleanliness.
pub const MAX_ROT: u32 = 25;

/// Obfuscate `text` according to `mode`.
///
/// - `mask_char`: the character used to replace masked chars in `mask` mode.
///   Blank → `'*'`. Only the first character is used.
/// - `keep_start` / `keep_end`: how many leading / trailing characters to keep
///   visible in `mask` mode. If they overlap (sum ≥ length) the whole string is
///   kept (nothing to mask).
/// - `rot_n`: the shift for `rot` mode, taken mod 26 (so 13 = ROT13, 0 = no-op).
///
/// Returns `Err` only on an unknown `mode`.
pub fn obfuscate(
    text: &str,
    mode: &str,
    mask_char: &str,
    keep_start: usize,
    keep_end: usize,
    rot_n: u32,
) -> Result<String, String> {
    let mode = Mode::parse(mode)?;
    Ok(match mode {
        Mode::Mask => mask(text, mask_char, keep_start, keep_end),
        Mode::Rot => rot(text, rot_n),
        Mode::Leetspeak => leetspeak(text),
        Mode::Homoglyph => homoglyph(text),
    })
}

/// Keep the first `keep_start` and last `keep_end` chars; mask the middle.
/// Counts in Unicode scalar values (chars), not bytes, so multi-byte input is
/// handled correctly. Whitespace is preserved (kept visible) so the masked
/// shape still reads naturally.
fn mask(text: &str, mask_char: &str, keep_start: usize, keep_end: usize) -> String {
    let fill = mask_char.chars().next().unwrap_or('*');
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    // Overlap or full coverage → nothing left to mask.
    if keep_start.saturating_add(keep_end) >= n {
        return text.to_string();
    }
    let mut out = String::with_capacity(n);
    for (i, &c) in chars.iter().enumerate() {
        if i < keep_start || i >= n - keep_end {
            out.push(c);
        } else if c.is_whitespace() {
            // Keep structure (spaces / newlines) visible.
            out.push(c);
        } else {
            out.push(fill);
        }
    }
    out
}

/// ROT-N: rotate ASCII letters by `n` (mod 26); everything else passes through.
fn rot(text: &str, n: u32) -> String {
    let shift = (n % 26) as u8;
    text.chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a' + shift) % 26) + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A' + shift) % 26) + b'A') as char,
            other => other,
        })
        .collect()
}

/// Replace common letters with look-alike digits/symbols.
fn leetspeak(text: &str) -> String {
    text.chars()
        .map(|c| match c.to_ascii_lowercase() {
            'a' => '4',
            'e' => '3',
            'i' => '1',
            'o' => '0',
            's' => '5',
            't' => '7',
            'b' => '8',
            'g' => '9',
            'l' => '1',
            _ => c,
        })
        .collect()
}

/// Replace ASCII letters with confusable Unicode look-alikes. The result looks
/// (almost) identical but is a distinct byte sequence — the canonical
/// "homograph" trick. Only letters with good single-codepoint confusables are
/// substituted; the rest pass through.
fn homoglyph(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'a' => 'а', // CYRILLIC SMALL A
            'c' => 'с', // CYRILLIC SMALL ES
            'e' => 'е', // CYRILLIC SMALL IE
            'i' => 'і', // CYRILLIC SMALL BYELORUSSIAN-UKRAINIAN I
            'j' => 'ј', // CYRILLIC SMALL JE
            'o' => 'о', // CYRILLIC SMALL O
            'p' => 'р', // CYRILLIC SMALL ER
            's' => 'ѕ', // CYRILLIC SMALL DZE
            'x' => 'х', // CYRILLIC SMALL HA
            'y' => 'у', // CYRILLIC SMALL U
            'A' => 'А',
            'B' => 'В',
            'C' => 'С',
            'E' => 'Е',
            'H' => 'Н',
            'K' => 'К',
            'M' => 'М',
            'O' => 'О',
            'P' => 'Р',
            'T' => 'Т',
            'X' => 'Х',
            'Y' => 'У',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_keeps_ends_masks_middle() {
        assert_eq!(
            obfuscate("sk-1234567890abcdef", "mask", "*", 5, 4, 0).unwrap(),
            "sk-12**********cdef"
        );
    }

    #[test]
    fn mask_defaults_to_star_when_char_blank() {
        assert_eq!(
            obfuscate("password", "mask", "", 1, 1, 0).unwrap(),
            "p******d"
        );
    }

    #[test]
    fn mask_preserves_whitespace_structure() {
        // Spaces stay visible so the masked shape still reads.
        assert_eq!(
            obfuscate("john doe", "mask", "•", 0, 0, 0).unwrap(),
            "•••• •••"
        );
    }

    #[test]
    fn mask_full_overlap_keeps_everything() {
        // keep_start + keep_end >= len → nothing to mask.
        assert_eq!(obfuscate("abc", "mask", "*", 2, 2, 0).unwrap(), "abc");
    }

    #[test]
    fn mask_counts_unicode_chars_not_bytes() {
        // "café" is 4 chars; keep 1 + 1, mask the middle two.
        assert_eq!(obfuscate("café", "mask", "*", 1, 1, 0).unwrap(), "c**é");
    }

    #[test]
    fn rot13_is_self_inverse() {
        let once = obfuscate("Hello, World!", "rot", "", 0, 0, 13).unwrap();
        assert_eq!(once, "Uryyb, Jbeyq!");
        let twice = obfuscate(&once, "rot", "", 0, 0, 13).unwrap();
        assert_eq!(twice, "Hello, World!");
    }

    #[test]
    fn rot_custom_shift_and_wrap() {
        // ROT1: z wraps to a, Z wraps to A.
        assert_eq!(obfuscate("xyzXYZ", "rot", "", 0, 0, 1).unwrap(), "yzaYZA");
        // Shift mod 26: 27 behaves as 1.
        assert_eq!(obfuscate("a", "rot", "", 0, 0, 27).unwrap(), "b");
    }

    #[test]
    fn leetspeak_substitutes_lookalikes() {
        assert_eq!(
            obfuscate("Leetspeak", "leetspeak", "", 0, 0, 0).unwrap(),
            "13375p34k"
        );
        // "leet" alias is accepted.
        assert_eq!(obfuscate("test", "leet", "", 0, 0, 0).unwrap(), "7357");
    }

    #[test]
    fn homoglyph_changes_bytes_but_looks_similar() {
        let out = obfuscate("paypal", "homoglyph", "", 0, 0, 0).unwrap();
        // Visually similar but a different string (Cyrillic substitutions).
        assert_ne!(out, "paypal");
        assert_eq!(out.chars().count(), "paypal".chars().count());
    }

    #[test]
    fn rejects_unknown_mode() {
        let err = obfuscate("x", "scramble", "", 0, 0, 0).unwrap_err();
        assert!(err.contains("invalid mode"), "got: {err}");
    }
}
