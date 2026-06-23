//! hex-codec core — encode UTF-8 text (or raw bytes) to a hexadecimal string and
//! decode a hex string back to text. Pure compute, no wafer/wasm-bindgen deps —
//! shared by the chat skill block and the web page.
//!
//! Encoding turns each byte into two hex digits, with optional case, a per-byte
//! delimiter, and an optional per-byte prefix (`0x` / `\x`). Decoding is tolerant:
//! it ignores ASCII whitespace and the common delimiters/prefixes, so a string
//! produced with any of these options round-trips back without the user having to
//! describe how it was formatted.

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

/// An optional marker placed before each byte's two hex digits when encoding.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prefix {
    None,
    /// `0x` before each byte (e.g. `0x480x65`).
    ZeroX,
    /// `\x` before each byte (e.g. `\x48\x65`).
    BackslashX,
}

impl Prefix {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "none" => Ok(Prefix::None),
            "0x" => Ok(Prefix::ZeroX),
            "\\x" | "backslash-x" => Ok(Prefix::BackslashX),
            other => Err(format!(
                "invalid prefix {other:?}: expected \"none\", \"0x\", or \"\\x\""
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Prefix::None => "",
            Prefix::ZeroX => "0x",
            Prefix::BackslashX => "\\x",
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
            "bytes" | "binary" => Ok(OutputFormat::Bytes),
            other => Err(format!(
                "invalid output format {other:?}: expected \"text\" or \"bytes\""
            )),
        }
    }
}

/// Encode raw bytes to a hex string with the chosen formatting.
fn encode_bytes(bytes: &[u8], uppercase: bool, delim: Delimiter, prefix: Prefix) -> String {
    let d = delim.as_str();
    let p = prefix.as_str();
    let mut out = String::with_capacity(bytes.len() * (2 + p.len() + d.len()));
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            out.push_str(d);
        }
        out.push_str(p);
        if uppercase {
            out.push_str(&format!("{b:02X}"));
        } else {
            out.push_str(&format!("{b:02x}"));
        }
    }
    out
}

/// Parse a (possibly delimited/prefixed) hex string into raw bytes. Ignores
/// ASCII whitespace, the common delimiters (`: - ,`), and the `0x` / `\x`
/// per-byte prefixes, so anything this tool can emit round-trips back.
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    // Drop the prefixes first so a `0x`/`\x` marker is removed whole, then keep
    // only hex digits.
    let stripped = s
        .replace("0x", "")
        .replace("0X", "")
        .replace("\\x", "")
        .replace("\\X", "");
    let cleaned: String = stripped.chars().filter(|c| c.is_ascii_hexdigit()).collect();
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

/// Encode `input` to a hex string, or decode a hex string back.
///
/// - `mode` (`"encode"` | `"decode"`, blank → `"encode"`): direction.
/// - `format` (`"text"` | `"bytes"`, blank → `"text"`): on **decode**, how to
///   render the recovered bytes — `"text"` is UTF-8 (errors if the bytes aren't
///   valid UTF-8) and `"bytes"` shows them as a plain lowercase hex byte string.
///   Ignored on encode (encode always reads the input as UTF-8 text).
/// - `delimiter` (`"none"` | `"space"` | `"colon"` | `"dash"` | `"comma"` |
///   `"newline"`, blank → `"none"`): the separator between bytes when encoding.
///   Decoding ignores any of these.
/// - `uppercase`: emit uppercase `A–F` hex digits when encoding (default false).
/// - `prefix` (`"none"` | `"0x"` | `"\x"`, blank → `"none"`): a marker before each
///   byte when encoding. Decoding strips `0x` and `\x` automatically.
///
/// Returns `Err` on an invalid `mode`/`format`/`delimiter`/`prefix`, a malformed
/// hex string on decode, or decoded bytes that aren't valid UTF-8 (when
/// `format = "text"`).
pub fn convert(
    input: &str,
    mode: &str,
    format: &str,
    delimiter: &str,
    uppercase: bool,
    prefix: &str,
) -> Result<String, String> {
    let delim = Delimiter::parse(delimiter)?;
    let prefix = Prefix::parse(prefix)?;
    let fmt = OutputFormat::parse(format)?;

    match mode {
        "" | "encode" => Ok(encode_bytes(input.as_bytes(), uppercase, delim, prefix)),
        "decode" => {
            let bytes = decode_hex(input)?;
            match fmt {
                OutputFormat::Text => String::from_utf8(bytes).map_err(|_| {
                    "decoded bytes are not valid UTF-8 — set output format 'bytes' to view the raw hex"
                        .into()
                }),
                OutputFormat::Bytes => {
                    Ok(encode_bytes(&bytes, false, Delimiter::None, Prefix::None))
                }
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
    fn encode_basic_lowercase() {
        // "Hi" == 0x48 0x69.
        assert_eq!(convert("Hi", "encode", "", "", false, "").unwrap(), "4869");
        assert_eq!(
            convert("foobar", "encode", "", "", false, "").unwrap(),
            "666f6f626172"
        );
    }

    #[test]
    fn encode_uppercase() {
        assert_eq!(
            convert("foobar", "encode", "", "", true, "").unwrap(),
            "666F6F626172"
        );
    }

    #[test]
    fn encode_with_delimiters() {
        assert_eq!(convert("Hi", "encode", "", "space", false, "").unwrap(), "48 69");
        assert_eq!(convert("Hi", "encode", "", "colon", false, "").unwrap(), "48:69");
        assert_eq!(convert("Hi", "encode", "", "dash", false, "").unwrap(), "48-69");
        assert_eq!(convert("Hi", "encode", "", "comma", false, "").unwrap(), "48,69");
        assert_eq!(convert("Hi", "encode", "", "newline", false, "").unwrap(), "48\n69");
    }

    #[test]
    fn encode_with_prefix() {
        assert_eq!(convert("Hi", "encode", "", "", false, "0x").unwrap(), "0x480x69");
        assert_eq!(
            convert("Hi", "encode", "", "space", false, "0x").unwrap(),
            "0x48 0x69"
        );
        assert_eq!(
            convert("Hi", "encode", "", "space", false, "\\x").unwrap(),
            "\\x48 \\x69"
        );
    }

    #[test]
    fn decode_plain() {
        assert_eq!(convert("4869", "decode", "", "", false, "").unwrap(), "Hi");
        assert_eq!(
            convert("666f6f626172", "decode", "", "", false, "").unwrap(),
            "foobar"
        );
    }

    #[test]
    fn decode_is_case_insensitive() {
        assert_eq!(convert("666F6F", "decode", "", "", false, "").unwrap(), "foo");
    }

    #[test]
    fn decode_ignores_delimiters_and_prefixes() {
        assert_eq!(convert("48 69", "decode", "", "", false, "").unwrap(), "Hi");
        assert_eq!(convert("48:69", "decode", "", "", false, "").unwrap(), "Hi");
        assert_eq!(convert("48-69", "decode", "", "", false, "").unwrap(), "Hi");
        assert_eq!(convert("0x48 0x69", "decode", "", "", false, "").unwrap(), "Hi");
        assert_eq!(convert("\\x48\\x69", "decode", "", "", false, "").unwrap(), "Hi");
        assert_eq!(convert("48\n69", "decode", "", "", false, "").unwrap(), "Hi");
    }

    #[test]
    fn round_trip_all_options() {
        let enc = convert("Hello, world!", "encode", "", "colon", true, "").unwrap();
        assert_eq!(enc, "48:65:6C:6C:6F:2C:20:77:6F:72:6C:64:21");
        assert_eq!(
            convert(&enc, "decode", "", "", false, "").unwrap(),
            "Hello, world!"
        );
    }

    #[test]
    fn decode_to_bytes_format_for_non_utf8() {
        // 0xff is never valid UTF-8 on its own.
        let err = convert("ff", "decode", "text", "", false, "").unwrap_err();
        assert!(err.contains("UTF-8"), "got: {err}");
        assert_eq!(convert("ff", "decode", "bytes", "", false, "").unwrap(), "ff");
        // Bytes output is always plain lowercase regardless of how it was given.
        assert_eq!(
            convert("DE AD BE EF", "decode", "bytes", "", false, "").unwrap(),
            "deadbeef"
        );
    }

    #[test]
    fn encode_empty_input() {
        assert_eq!(convert("", "encode", "", "space", false, "").unwrap(), "");
        assert_eq!(convert("", "decode", "", "", false, "").unwrap(), "");
    }

    #[test]
    fn unicode_encodes_utf8_bytes() {
        // 'é' == U+00E9 == 0xC3 0xA9 in UTF-8.
        assert_eq!(convert("é", "encode", "", "", false, "").unwrap(), "c3a9");
        assert_eq!(convert("c3a9", "decode", "", "", false, "").unwrap(), "é");
    }

    #[test]
    fn rejects_odd_length_hex() {
        let err = convert("abc", "decode", "", "", false, "").unwrap_err();
        assert!(err.contains("odd number"), "got: {err}");
    }

    #[test]
    fn non_hex_chars_are_filtered() {
        // Non-hex chars are dropped; "g4869" -> "4869" -> "Hi".
        assert_eq!(convert("g4869", "decode", "", "", false, "").unwrap(), "Hi");
        // 'z','z' both filtered -> empty -> empty string.
        assert_eq!(convert("zz", "decode", "", "", false, "").unwrap(), "");
    }

    #[test]
    fn rejects_unknown_mode_format_delimiter_prefix() {
        assert!(convert("x", "zap", "", "", false, "")
            .unwrap_err()
            .contains("invalid mode"));
        assert!(convert("x", "decode", "weird", "", false, "")
            .unwrap_err()
            .contains("invalid output format"));
        assert!(convert("x", "encode", "", "pipe", false, "")
            .unwrap_err()
            .contains("invalid delimiter"));
        assert!(convert("x", "encode", "", "", false, "%x")
            .unwrap_err()
            .contains("invalid prefix"));
    }
}
