//! base64url-converter core — transcode between standard Base64 and URL-safe
//! Base64url. This is a pure ALPHABET/padding transform, not a byte
//! encode/decode: it swaps the two characters that differ between the alphabets
//! (`+`/`/` ⇄ `-`/`_`) and applies a padding policy. No wafer/wasm-bindgen deps;
//! shared by the chat skill block and the web page.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

/// Which alphabet the output should use.
#[derive(Clone, Copy)]
enum Direction {
    /// Produce URL-safe Base64url (`+`→`-`, `/`→`_`).
    ToUrl,
    /// Produce standard Base64 (`-`→`+`, `_`→`/`).
    ToStandard,
}

impl Direction {
    /// Parse the `direction` arg. `""`/`"auto"` picks the OPPOSITE of what the
    /// input looks like: a string that already contains a `-` or `_` is treated
    /// as Base64url and converted to standard; anything else (including a bare
    /// token with no `+`/`/` either) is treated as standard and converted to
    /// URL-safe Base64url — the tool's headline use.
    fn parse(s: &str, cleaned: &str) -> Result<Self, String> {
        match s {
            "" | "auto" => {
                if cleaned.contains('-') || cleaned.contains('_') {
                    Ok(Direction::ToStandard)
                } else {
                    Ok(Direction::ToUrl)
                }
            }
            "to-url" => Ok(Direction::ToUrl),
            "to-standard" => Ok(Direction::ToStandard),
            other => Err(format!(
                "invalid direction {other:?}: expected \"auto\", \"to-url\", or \"to-standard\""
            )),
        }
    }

    /// Map a single character into the target alphabet. Characters outside the
    /// two swap pairs pass through unchanged.
    fn map_char(self, c: char) -> char {
        match self {
            Direction::ToUrl => match c {
                '+' => '-',
                '/' => '_',
                x => x,
            },
            Direction::ToStandard => match c {
                '-' => '+',
                '_' => '/',
                x => x,
            },
        }
    }
}

/// How the output handles `=` padding.
#[derive(Clone, Copy)]
enum PadPolicy {
    /// Pad standard output, leave URL-safe output unpadded (the canonical form
    /// of each alphabet).
    Auto,
    /// Always pad the output to a multiple of 4 with `=`.
    Keep,
    /// Never pad — strip all `=` from the output.
    Strip,
}

impl PadPolicy {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "auto" => Ok(PadPolicy::Auto),
            "keep" => Ok(PadPolicy::Keep),
            "strip" => Ok(PadPolicy::Strip),
            other => Err(format!(
                "invalid padding {other:?}: expected \"auto\", \"keep\", or \"strip\""
            )),
        }
    }

    /// Whether the output should carry `=` padding.
    fn should_pad(self, dir: Direction) -> bool {
        match self {
            PadPolicy::Keep => true,
            PadPolicy::Strip => false,
            // Canonical: standard Base64 is padded, Base64url is not.
            PadPolicy::Auto => matches!(dir, Direction::ToStandard),
        }
    }
}

/// Transcode `text` between standard Base64 and URL-safe Base64url.
///
/// - `direction` (`"auto"` | `"to-url"` | `"to-standard"`, blank → `"auto"`):
///   which alphabet to produce. `auto` converts to the opposite of the detected
///   input (see [`Direction::parse`]).
/// - `padding` (`"auto"` | `"keep"` | `"strip"`, blank → `"auto"`): whether the
///   output carries `=` padding. `auto` pads standard output and leaves
///   Base64url output unpadded.
/// - `validate`: when `true`, verify the input actually decodes as Base64
///   (rejecting a malformed length or stray characters) before converting.
///
/// All ASCII whitespace in the input is ignored, so line-wrapped MIME Base64 and
/// copy-pasted tokens convert cleanly. Characters outside the combined Base64 /
/// Base64url alphabet (`A–Z a–z 0–9 + / - _ =`) are always rejected.
///
/// Returns `Err` on an invalid `direction`/`padding`, a stray non-alphabet
/// character, misplaced padding, or (when `validate`) input that is not
/// decodable Base64.
pub fn convert(text: &str, direction: &str, padding: &str, validate: bool) -> Result<String, String> {
    // Ignore all whitespace: MIME Base64 wraps at 76 columns and pasted tokens
    // often carry stray spaces/newlines.
    let cleaned: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();

    // Reject anything that clearly is not Base64 data up front, so errors name
    // the offending character instead of silently passing it through.
    for c in cleaned.chars() {
        if !(c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '-' | '_' | '=')) {
            return Err(format!(
                "invalid character {c:?} in input: Base64 data may only contain \
                 A-Z, a-z, 0-9, and the symbols + / - _ = (whitespace is ignored)"
            ));
        }
    }

    let dir = Direction::parse(direction, &cleaned)?;
    let pad_policy = PadPolicy::parse(padding)?;

    // Normalize: `=` padding only ever appears at the very end. Strip it to a
    // bare core; a `=` anywhere else means the input is malformed.
    let core = cleaned.trim_end_matches('=');
    if core.contains('=') {
        return Err("invalid Base64: '=' padding may only appear at the end of the string".into());
    }

    if validate {
        // Decode-check against the standard alphabet with padding restored, so a
        // wrong length or an out-of-alphabet byte is caught before we hand back
        // a string that only looks converted.
        let std: String = core.chars().map(|c| Direction::ToStandard.map_char(c)).collect();
        let padded = pad_to_multiple_of_4(&std);
        STANDARD
            .decode(padded.as_bytes())
            .map_err(|e| format!("input is not valid Base64: {e}"))?;
    }

    let mapped: String = core.chars().map(|c| dir.map_char(c)).collect();
    let out = if pad_policy.should_pad(dir) {
        pad_to_multiple_of_4(&mapped)
    } else {
        mapped
    };
    Ok(out)
}

/// Append `=` until the length is a multiple of 4 (Base64's group size).
fn pad_to_multiple_of_4(s: &str) -> String {
    let rem = s.len() % 4;
    if rem == 0 {
        s.to_string()
    } else {
        format!("{s}{}", "=".repeat(4 - rem))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_to_url_swaps_chars_and_strips_padding() {
        // '/' → '_', padding dropped for canonical Base64url.
        assert_eq!(
            convert("c3ViamVjdHM/X2Q9MQ==", "to-url", "auto", false).unwrap(),
            "c3ViamVjdHM_X2Q9MQ"
        );
        // Both swap chars at once.
        assert_eq!(convert("+/+/", "to-url", "auto", false).unwrap(), "-_-_");
    }

    #[test]
    fn url_to_standard_swaps_chars_and_pads() {
        assert_eq!(
            convert("c3ViamVjdHM_X2Q9MQ", "to-standard", "auto", false).unwrap(),
            "c3ViamVjdHM/X2Q9MQ=="
        );
        assert_eq!(convert("-_-_", "to-standard", "auto", false).unwrap(), "+/+/");
    }

    #[test]
    fn auto_detects_url_input_and_converts_to_standard() {
        // Contains '-'/'_' → treated as Base64url → standard (padded).
        assert_eq!(
            convert("FPucA9l-", "auto", "auto", false).unwrap(),
            "FPucA9l+"
        );
    }

    #[test]
    fn auto_defaults_ambiguous_input_to_url() {
        // No '-'/'_' markers → treated as standard → URL-safe (unpadded).
        assert_eq!(convert("+/+/", "", "", false).unwrap(), "-_-_");
        // A bare token with no special chars is left alone but unpadded.
        assert_eq!(
            convert("eyJhbGciOiJIUzI1NiJ9", "auto", "auto", false).unwrap(),
            "eyJhbGciOiJIUzI1NiJ9"
        );
    }

    #[test]
    fn padding_keep_and_strip_override_auto() {
        // keep: pad even URL-safe output.
        assert_eq!(
            convert("c3ViamVjdHM/X2Q9MQ==", "to-url", "keep", false).unwrap(),
            "c3ViamVjdHM_X2Q9MQ=="
        );
        // strip: unpad even standard output.
        assert_eq!(
            convert("c3ViamVjdHM_X2Q9MQ", "to-standard", "strip", false).unwrap(),
            "c3ViamVjdHM/X2Q9MQ"
        );
    }

    #[test]
    fn whitespace_is_ignored() {
        assert_eq!(
            convert("c3ViamVj\ndHM/X2\tQ9MQ==", "to-url", "auto", false).unwrap(),
            "c3ViamVjdHM_X2Q9MQ"
        );
    }

    #[test]
    fn validate_accepts_valid_and_rejects_bad_length() {
        // Valid Base64 passes validation and still converts.
        assert_eq!(
            convert("eyJhbGciOiJIUzI1NiJ9", "to-url", "auto", true).unwrap(),
            "eyJhbGciOiJIUzI1NiJ9"
        );
        // Length ≡ 1 (mod 4) can never be valid Base64.
        let err = convert("abcde", "to-url", "auto", true).unwrap_err();
        assert!(err.contains("not valid Base64"), "got: {err}");
    }

    #[test]
    fn rejects_stray_non_alphabet_character() {
        let err = convert("abc$", "to-url", "auto", false).unwrap_err();
        assert!(err.contains("invalid character"), "got: {err}");
    }

    #[test]
    fn rejects_misplaced_padding() {
        let err = convert("ab=cd", "to-url", "auto", false).unwrap_err();
        assert!(err.contains("padding may only appear"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_direction_and_padding() {
        assert!(convert("ab", "sideways", "auto", false)
            .unwrap_err()
            .contains("invalid direction"));
        assert!(convert("ab", "auto", "loose", false)
            .unwrap_err()
            .contains("invalid padding"));
    }
}
