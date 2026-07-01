//! binary-codec core — encode UTF-8 text (or raw bytes) to a per-byte binary
//! bit string and decode a binary string back to text. Pure compute, no
//! wafer/wasm-bindgen deps — shared by the chat skill block and the web page.
//!
//! Encoding turns each byte into eight `0`/`1` bits, with an optional per-byte
//! delimiter and an optional per-byte `0b` prefix. Decoding is tolerant: it
//! ignores ASCII whitespace, the common delimiters, and the `0b` prefix, so a
//! string produced with any of these options round-trips back without the user
//! having to describe how it was formatted.

/// The separator inserted between bytes when encoding (and stripped on decode).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Delimiter {
    None,
    Space,
    Colon,
    Dash,
    Comma,
    Newline,
}

impl Delimiter {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "none" => Ok(Delimiter::None),
            "space" => Ok(Delimiter::Space),
            "colon" => Ok(Delimiter::Colon),
            "dash" | "hyphen" => Ok(Delimiter::Dash),
            "comma" => Ok(Delimiter::Comma),
            "newline" => Ok(Delimiter::Newline),
            other => Err(format!(
                "invalid delimiter {other:?}: expected \"none\", \"space\", \"colon\", \"dash\", \"comma\", or \"newline\""
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Delimiter::None => "",
            Delimiter::Space => " ",
            Delimiter::Colon => ":",
            Delimiter::Dash => "-",
            Delimiter::Comma => ",",
            Delimiter::Newline => "\n",
        }
    }
}

/// An optional marker placed before each byte's eight bits when encoding.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prefix {
    None,
    /// `0b` before each byte (e.g. `0b01001000 0b01101001`).
    ZeroB,
}

impl Prefix {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "none" => Ok(Prefix::None),
            "0b" => Ok(Prefix::ZeroB),
            other => Err(format!(
                "invalid prefix {other:?}: expected \"none\" or \"0b\""
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Prefix::None => "",
            Prefix::ZeroB => "0b",
        }
    }
}

/// How to render the recovered bytes when decoding.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    /// UTF-8 text — the default.
    Text,
    /// A plain lowercase hex byte string — for binary that isn't valid UTF-8.
    Bytes,
}

impl OutputFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "text" => Ok(OutputFormat::Text),
            "bytes" | "hex" => Ok(OutputFormat::Bytes),
            other => Err(format!(
                "invalid output format {other:?}: expected \"text\" or \"bytes\""
            )),
        }
    }
}

/// Encode raw bytes to a binary bit string with the chosen formatting.
fn encode_bytes(bytes: &[u8], delim: Delimiter, prefix: Prefix) -> String {
    let d = delim.as_str();
    let p = prefix.as_str();
    let mut out = String::with_capacity(bytes.len() * (8 + p.len() + d.len()));
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            out.push_str(d);
        }
        out.push_str(p);
        out.push_str(&format!("{b:08b}"));
    }
    out
}

/// Encode raw bytes to a plain lowercase hex string (used for the `bytes`
/// decode-output format so non-UTF-8 data is still viewable).
fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Parse a (possibly delimited/prefixed) binary string into raw bytes. Ignores
/// ASCII whitespace, the common delimiters (`: - ,`), and the `0b` per-byte
/// prefix, then reads the remaining `0`/`1` digits in groups of eight, so
/// anything this tool can emit round-trips back.
fn decode_binary(s: &str) -> Result<Vec<u8>, String> {
    // Drop the prefix first so a `0b` marker is removed whole, then keep only
    // binary digits.
    let stripped = s.replace("0b", "").replace("0B", "");
    let cleaned: String = stripped.chars().filter(|c| *c == '0' || *c == '1').collect();
    if cleaned.len() % 8 != 0 {
        return Err(format!(
            "binary input has {} bits, which is not a multiple of 8; each byte needs eight bits",
            cleaned.len()
        ));
    }
    (0..cleaned.len())
        .step_by(8)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 8], 2)
                .map_err(|_| format!("invalid binary byte {:?}", &cleaned[i..i + 8]))
        })
        .collect()
}

/// Encode `input` to a binary bit string, or decode a binary string back.
///
/// - `mode` (`"encode"` | `"decode"`, blank → `"encode"`): direction.
/// - `format` (`"text"` | `"bytes"`, blank → `"text"`): on **decode**, how to
///   render the recovered bytes — `"text"` is UTF-8 (errors if the bytes aren't
///   valid UTF-8) and `"bytes"` shows them as a plain lowercase hex byte string.
///   Ignored on encode (encode always reads the input as UTF-8 text).
/// - `delimiter` (`"none"` | `"space"` | `"colon"` | `"dash"` | `"comma"` |
///   `"newline"`, blank → `"none"` here; the descriptor defaults the field to
///   `"space"`): the separator between bytes when encoding. Decoding ignores any
///   of these.
/// - `prefix` (`"none"` | `"0b"`, blank → `"none"`): a marker before each byte
///   when encoding. Decoding strips `0b` automatically.
///
/// Returns `Err` on an invalid `mode`/`format`/`delimiter`/`prefix`, a malformed
/// binary string on decode, or decoded bytes that aren't valid UTF-8 (when
/// `format = "text"`).
pub fn convert(
    input: &str,
    mode: &str,
    format: &str,
    delimiter: &str,
    prefix: &str,
) -> Result<String, String> {
    let delim = Delimiter::parse(delimiter)?;
    let prefix = Prefix::parse(prefix)?;
    let fmt = OutputFormat::parse(format)?;

    match mode {
        "" | "encode" => Ok(encode_bytes(input.as_bytes(), delim, prefix)),
        "decode" => {
            let bytes = decode_binary(input)?;
            match fmt {
                OutputFormat::Text => String::from_utf8(bytes).map_err(|_| {
                    "decoded bytes are not valid UTF-8 — set output format 'bytes' to view the raw hex"
                        .into()
                }),
                OutputFormat::Bytes => Ok(to_hex(&bytes)),
            }
        }
        other => Err(format!(
            "invalid mode {other:?}: expected \"encode\" or \"decode\""
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_basic() {
        // "Hi" == 0x48 0x69 == 01001000 01101001.
        assert_eq!(convert("Hi", "encode", "", "none", "").unwrap(), "0100100001101001");
        assert_eq!(convert("Hi", "encode", "", "space", "").unwrap(), "01001000 01101001");
    }

    #[test]
    fn encode_blank_delimiter_is_none() {
        // Core treats "" as none; the descriptor defaults the field to "space".
        assert_eq!(convert("A", "encode", "", "", "").unwrap(), "01000001");
    }

    #[test]
    fn encode_with_delimiters() {
        assert_eq!(convert("Hi", "encode", "", "colon", "").unwrap(), "01001000:01101001");
        assert_eq!(convert("Hi", "encode", "", "dash", "").unwrap(), "01001000-01101001");
        assert_eq!(convert("Hi", "encode", "", "comma", "").unwrap(), "01001000,01101001");
        assert_eq!(convert("Hi", "encode", "", "newline", "").unwrap(), "01001000\n01101001");
    }

    #[test]
    fn encode_with_prefix() {
        assert_eq!(convert("Hi", "encode", "", "space", "0b").unwrap(), "0b01001000 0b01101001");
        assert_eq!(convert("A", "encode", "", "none", "0b").unwrap(), "0b01000001");
    }

    #[test]
    fn decode_plain() {
        assert_eq!(convert("0100100001101001", "decode", "", "", "").unwrap(), "Hi");
        assert_eq!(convert("01001000 01101001", "decode", "", "", "").unwrap(), "Hi");
    }

    #[test]
    fn decode_ignores_delimiters_and_prefixes() {
        assert_eq!(convert("01001000:01101001", "decode", "", "", "").unwrap(), "Hi");
        assert_eq!(convert("01001000-01101001", "decode", "", "", "").unwrap(), "Hi");
        assert_eq!(convert("0b01001000 0b01101001", "decode", "", "", "").unwrap(), "Hi");
        assert_eq!(convert("01001000\n01101001", "decode", "", "", "").unwrap(), "Hi");
    }

    #[test]
    fn round_trip_all_options() {
        let enc = convert("Hello, world!", "encode", "", "space", "0b").unwrap();
        assert_eq!(convert(&enc, "decode", "", "", "").unwrap(), "Hello, world!");
    }

    #[test]
    fn decode_to_bytes_format_for_non_utf8() {
        // 0xff == 11111111 is never valid UTF-8 on its own.
        let err = convert("11111111", "decode", "text", "", "").unwrap_err();
        assert!(err.contains("UTF-8"), "got: {err}");
        assert_eq!(convert("11111111", "decode", "bytes", "", "").unwrap(), "ff");
        // 0xde 0xad 0xbe 0xef.
        assert_eq!(
            convert("11011110 10101101 10111110 11101111", "decode", "bytes", "", "").unwrap(),
            "deadbeef"
        );
    }

    #[test]
    fn encode_empty_input() {
        assert_eq!(convert("", "encode", "", "space", "").unwrap(), "");
        assert_eq!(convert("", "decode", "", "", "").unwrap(), "");
    }

    #[test]
    fn unicode_encodes_utf8_bytes() {
        // 'é' == U+00E9 == 0xC3 0xA9 == 11000011 10101001 in UTF-8.
        assert_eq!(convert("é", "encode", "", "space", "").unwrap(), "11000011 10101001");
        assert_eq!(convert("11000011 10101001", "decode", "", "", "").unwrap(), "é");
    }

    #[test]
    fn rejects_non_multiple_of_eight() {
        let err = convert("0100100", "decode", "", "", "").unwrap_err();
        assert!(err.contains("multiple of 8"), "got: {err}");
    }

    #[test]
    fn non_binary_chars_are_filtered() {
        // Letters/other digits are dropped; "x01001000y" -> "01001000" -> "H".
        assert_eq!(convert("x01001000y", "decode", "", "", "").unwrap(), "H");
    }

    #[test]
    fn rejects_unknown_mode_format_delimiter_prefix() {
        assert!(convert("x", "zap", "", "", "")
            .unwrap_err()
            .contains("invalid mode"));
        assert!(convert("x", "decode", "weird", "", "")
            .unwrap_err()
            .contains("invalid output format"));
        assert!(convert("x", "encode", "", "pipe", "")
            .unwrap_err()
            .contains("invalid delimiter"));
        assert!(convert("x", "encode", "", "", "0x")
            .unwrap_err()
            .contains("invalid prefix"));
    }
}
