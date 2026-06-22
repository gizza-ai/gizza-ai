//! byte-take core — extract a contiguous byte range (start offset + length)
//! from a buffer and return those bytes. The input can be interpreted as UTF-8
//! text, a hex byte string, or Base64, and the extracted slice rendered back in
//! any of the same formats. Pure compute, no wafer/wasm-bindgen deps — shared by
//! the chat skill block and the web page.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// How to interpret the input string / how to render the extracted bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    /// UTF-8 text — the default.
    Text,
    /// A hex string (e.g. `48 65 6c` or `0x48656c`), case-insensitive.
    Hex,
    /// Standard Base64 (RFC 4648), padding optional on decode.
    Base64,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "text" => Ok(Format::Text),
            "hex" => Ok(Format::Hex),
            "base64" | "b64" => Ok(Format::Base64),
            other => Err(format!(
                "invalid format {other:?}: expected \"text\", \"hex\", or \"base64\""
            )),
        }
    }

    /// Turn the user-supplied source string into the raw bytes to operate on.
    fn to_bytes(self, s: &str) -> Result<Vec<u8>, String> {
        match self {
            Format::Text => Ok(s.as_bytes().to_vec()),
            Format::Hex => parse_hex(s),
            Format::Base64 => {
                let cleaned: String = s.chars().filter(|c| !c.is_ascii_whitespace()).collect();
                B64.decode(cleaned.as_bytes())
                    .or_else(|_| {
                        base64::engine::general_purpose::STANDARD_NO_PAD
                            .decode(cleaned.trim_end_matches('=').as_bytes())
                    })
                    .map_err(|e| format!("invalid Base64 input: {e}"))
            }
        }
    }

    /// Render the extracted raw bytes back to a string in this format.
    fn from_bytes(self, bytes: &[u8]) -> Result<String, String> {
        match self {
            Format::Text => String::from_utf8(bytes.to_vec()).map_err(|_| {
                "the extracted bytes are not valid UTF-8 — set the output to 'hex' or 'base64' to view them".into()
            }),
            Format::Hex => Ok(to_hex(bytes)),
            Format::Base64 => Ok(B64.encode(bytes)),
        }
    }
}

/// Parse a hex string into bytes, ignoring ASCII whitespace and an optional
/// `0x` prefix. Rejects odd length or non-hex digits.
fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "hex input has an odd number of digits ({}); each byte needs two",
            cleaned.len()
        ));
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| format!("invalid hex byte {:?}", &cleaned[i..i + 2]))
        })
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Extract a contiguous byte range from `input` and return those bytes.
///
/// - `input`: the data, interpreted per `in_format`.
/// - `start`: 0-based byte offset of the first byte to keep. A negative value
///   counts from the end of the buffer (`-1` is the last byte). Clamped to the
///   buffer bounds.
/// - `length`: how many bytes to extract starting at `start`. `0` extracts
///   nothing (empty result); a length running past the end of the buffer is
///   clamped. Must not be negative.
/// - `in_format` / `out_format`: how to read the input and render the slice
///   (`"text"`, `"hex"`, or `"base64"`).
pub fn take_bytes(
    input: &str,
    start: i64,
    length: i64,
    in_format: &str,
    out_format: &str,
) -> Result<String, String> {
    let in_fmt = Format::parse(in_format)?;
    let out_fmt = Format::parse(out_format)?;
    if length < 0 {
        return Err(format!("length must be zero or positive, got {length}"));
    }
    let bytes = in_fmt.to_bytes(input)?;
    let len = bytes.len() as i64;

    // Resolve a possibly-negative start to an absolute offset, then clamp to
    // [0, len].
    let abs_start = if start < 0 { len + start } else { start };
    let s = abs_start.clamp(0, len) as usize;
    let e = (abs_start + length).clamp(0, len) as usize;

    out_fmt.from_bytes(&bytes[s..e])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_middle_of_text() {
        // "Hello, World" -> take ", Wo" (offset 5, len 4)
        assert_eq!(
            take_bytes("Hello, World", 5, 4, "text", "text").unwrap(),
            ", Wo"
        );
    }

    #[test]
    fn takes_nothing_when_length_zero() {
        assert_eq!(take_bytes("abcdef", 2, 0, "text", "text").unwrap(), "");
    }

    #[test]
    fn takes_from_start() {
        assert_eq!(take_bytes("abcdef", 0, 3, "text", "text").unwrap(), "abc");
    }

    #[test]
    fn negative_start_counts_from_end() {
        // start -2 on a 6-byte buffer == offset 4; take 2 -> "ef"
        assert_eq!(take_bytes("abcdef", -2, 2, "text", "text").unwrap(), "ef");
    }

    #[test]
    fn length_past_end_is_clamped() {
        assert_eq!(take_bytes("abcdef", 4, 100, "text", "text").unwrap(), "ef");
    }

    #[test]
    fn start_past_end_takes_nothing() {
        assert_eq!(take_bytes("abc", 10, 2, "text", "text").unwrap(), "");
    }

    #[test]
    fn hex_in_hex_out() {
        // 0x00112233 take offset 1 len 2 -> 0x1122
        assert_eq!(take_bytes("00112233", 1, 2, "hex", "hex").unwrap(), "1122");
    }

    #[test]
    fn hex_input_with_0x_and_whitespace() {
        assert_eq!(
            take_bytes("0x00 11 22 33", 0, 1, "hex", "hex").unwrap(),
            "00"
        );
    }

    #[test]
    fn text_in_hex_out() {
        // "ABC" = 0x414243, take offset 1 len 1 -> 0x42
        assert_eq!(take_bytes("ABC", 1, 1, "text", "hex").unwrap(), "42");
    }

    #[test]
    fn base64_roundtrip() {
        // "Hello" base64 = SGVsbG8=. Take bytes 1..4 ("ell") -> ZWxs
        assert_eq!(
            take_bytes("SGVsbG8=", 1, 3, "base64", "base64").unwrap(),
            "ZWxs"
        );
    }

    #[test]
    fn base64_in_text_out() {
        assert_eq!(
            take_bytes("SGVsbG8=", 1, 3, "base64", "text").unwrap(),
            "ell"
        );
    }

    #[test]
    fn negative_length_errors() {
        assert!(take_bytes("abc", 0, -1, "text", "text").is_err());
    }

    #[test]
    fn invalid_utf8_slice_text_errors() {
        // "é" = 0xC3 0xA9. Input hex, output text: take just the lead byte -> 0xC3 (invalid UTF-8).
        assert!(take_bytes("c3a9", 0, 1, "hex", "text").is_err());
    }

    #[test]
    fn invalid_format_errors() {
        assert!(take_bytes("abc", 0, 1, "weird", "text").is_err());
    }
}
