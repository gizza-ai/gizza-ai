//! byte-drop core — remove a byte range (start offset + length) from a buffer
//! and return the remaining bytes. The input can be interpreted as UTF-8 text,
//! a hex byte string, or Base64, and the remainder rendered back in any of the
//! same formats. Pure compute, no wafer/wasm-bindgen deps — shared by the chat
//! skill block and the web page.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// How to interpret the input string / how to render the remaining bytes.
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

    /// Render the remaining raw bytes back to a string in this format.
    fn from_bytes(self, bytes: &[u8]) -> Result<String, String> {
        match self {
            Format::Text => String::from_utf8(bytes.to_vec()).map_err(|_| {
                "the remaining bytes are not valid UTF-8 — set the output to 'hex' or 'base64' to view them".into()
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

/// Remove a contiguous byte range from `input` and return the remaining bytes.
///
/// - `input`: the data, interpreted per `in_format`.
/// - `start`: 0-based byte offset of the first byte to drop. A negative value
///   counts from the end of the buffer (`-1` is the last byte). Clamped to the
///   buffer bounds.
/// - `length`: how many bytes to remove starting at `start`. `0` drops nothing;
///   a length running past the end of the buffer is clamped. Must not be
///   negative.
/// - `in_format` / `out_format`: how to read the input and render the remainder
///   (`"text"`, `"hex"`, or `"base64"`).
pub fn drop_bytes(
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

    let mut out = Vec::with_capacity(bytes.len() - (e - s));
    out.extend_from_slice(&bytes[..s]);
    out.extend_from_slice(&bytes[e..]);
    out_fmt.from_bytes(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_middle_of_text() {
        // "Hello, World" -> drop ", Wo" (offset 5, len 4) -> "Hellorld"
        assert_eq!(
            drop_bytes("Hello, World", 5, 4, "text", "text").unwrap(),
            "Hellorld"
        );
    }

    #[test]
    fn drops_nothing_when_length_zero() {
        assert_eq!(drop_bytes("abcdef", 2, 0, "text", "text").unwrap(), "abcdef");
    }

    #[test]
    fn drops_from_start() {
        assert_eq!(drop_bytes("abcdef", 0, 3, "text", "text").unwrap(), "def");
    }

    #[test]
    fn negative_start_counts_from_end() {
        // start -2 on a 6-byte buffer == offset 4; drop 2 -> "abcd"
        assert_eq!(drop_bytes("abcdef", -2, 2, "text", "text").unwrap(), "abcd");
    }

    #[test]
    fn length_past_end_is_clamped() {
        assert_eq!(drop_bytes("abcdef", 4, 100, "text", "text").unwrap(), "abcd");
    }

    #[test]
    fn start_past_end_drops_nothing() {
        assert_eq!(drop_bytes("abc", 10, 2, "text", "text").unwrap(), "abc");
    }

    #[test]
    fn hex_in_hex_out() {
        // 0x00112233 drop offset 1 len 2 (0x1122) -> 0x0033
        assert_eq!(drop_bytes("00112233", 1, 2, "hex", "hex").unwrap(), "0033");
    }

    #[test]
    fn hex_input_with_0x_and_whitespace() {
        assert_eq!(
            drop_bytes("0x00 11 22 33", 0, 1, "hex", "hex").unwrap(),
            "112233"
        );
    }

    #[test]
    fn text_in_hex_out() {
        // "ABC" = 0x414243, drop offset 1 len 1 -> 0x4143
        assert_eq!(drop_bytes("ABC", 1, 1, "text", "hex").unwrap(), "4143");
    }

    #[test]
    fn base64_roundtrip() {
        // "Hello" base64 = SGVsbG8=. Drop first byte 'H' (offset 0 len 1) -> "ello" -> ZWxsbw==
        assert_eq!(
            drop_bytes("SGVsbG8=", 0, 1, "base64", "base64").unwrap(),
            "ZWxsbw=="
        );
    }

    #[test]
    fn base64_in_text_out() {
        assert_eq!(
            drop_bytes("SGVsbG8=", 0, 1, "base64", "text").unwrap(),
            "ello"
        );
    }

    #[test]
    fn negative_length_errors() {
        assert!(drop_bytes("abc", 0, -1, "text", "text").is_err());
    }

    #[test]
    fn invalid_utf8_remainder_text_errors() {
        // "é" = 0xC3 0xA9. Input hex, output text: drop the lead byte -> 0xA9 alone (invalid UTF-8).
        assert!(drop_bytes("c3a9", 0, 1, "hex", "text").is_err());
    }

    #[test]
    fn odd_hex_errors() {
        assert!(drop_bytes("abc", 0, 1, "hex", "hex").is_err());
    }

    #[test]
    fn invalid_format_errors() {
        assert!(drop_bytes("abc", 0, 1, "yaml", "text").is_err());
    }

    #[test]
    fn empty_input_is_ok() {
        assert_eq!(drop_bytes("", 0, 5, "text", "text").unwrap(), "");
    }
}
