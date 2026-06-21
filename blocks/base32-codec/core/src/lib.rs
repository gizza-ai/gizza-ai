//! base32-codec core — encode text/bytes to RFC 4648 Base32 (and the hex,
//! Crockford, and z-base-32 variants) and decode Base32 back to the original
//! data. Pure compute, no wafer/wasm-bindgen deps — shared by the chat skill
//! block and the web page.

use base32::Alphabet;

/// The supported Base32 alphabets.
///
/// - `standard`: RFC 4648 §6 — uppercase `A–Z` + `2–7` (the default).
/// - `hex`: RFC 4648 §7 "base32hex" — `0–9` + `A–V`, sorts in the same order as
///   the underlying bytes.
/// - `crockford`: Douglas Crockford's Base32 — `0–9` + `A–Z` excluding the
///   ambiguous `I L O U`, case-insensitive on decode. Never padded.
/// - `zbase32`: z-base-32 — a human-oriented permuted alphabet. Never padded.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Standard,
    Hex,
    Crockford,
    Zbase32,
}

impl Variant {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "standard" | "rfc4648" => Ok(Variant::Standard),
            "hex" | "base32hex" => Ok(Variant::Hex),
            "crockford" => Ok(Variant::Crockford),
            "zbase32" | "z-base-32" => Ok(Variant::Zbase32),
            other => Err(format!(
                "invalid variant {other:?}: expected \"standard\", \"hex\", \"crockford\", or \"zbase32\""
            )),
        }
    }

    /// Map to a `base32::Alphabet`. `lowercase` and `padding` only affect the
    /// RFC 4648 alphabets (standard/hex); Crockford and z-base-32 have a fixed
    /// case and are never padded.
    fn alphabet(self, lowercase: bool, padding: bool) -> Alphabet {
        match self {
            Variant::Standard => {
                if lowercase {
                    Alphabet::Rfc4648Lower { padding }
                } else {
                    Alphabet::Rfc4648 { padding }
                }
            }
            Variant::Hex => {
                if lowercase {
                    Alphabet::Rfc4648HexLower { padding }
                } else {
                    Alphabet::Rfc4648Hex { padding }
                }
            }
            Variant::Crockford => Alphabet::Crockford,
            Variant::Zbase32 => Alphabet::Z,
        }
    }
}

/// How to interpret the bytes being encoded / how to render the decoded bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DataFormat {
    /// UTF-8 text — the default.
    Text,
    /// A hex string (e.g. `48 65 6c` or `48656c`), case-insensitive.
    Hex,
}

impl DataFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "text" => Ok(DataFormat::Text),
            "hex" => Ok(DataFormat::Hex),
            other => Err(format!(
                "invalid data format {other:?}: expected \"text\" or \"hex\""
            )),
        }
    }

    /// Turn the user-supplied source string into the raw bytes to encode.
    fn to_bytes(self, s: &str) -> Result<Vec<u8>, String> {
        match self {
            DataFormat::Text => Ok(s.as_bytes().to_vec()),
            DataFormat::Hex => parse_hex(s),
        }
    }

    /// Render decoded raw bytes back to a string in this format.
    fn from_bytes(self, bytes: &[u8]) -> Result<String, String> {
        match self {
            DataFormat::Text => String::from_utf8(bytes.to_vec()).map_err(|_| {
                "decoded bytes are not valid UTF-8 — set output 'hex' to view the raw bytes".into()
            }),
            DataFormat::Hex => Ok(to_hex(bytes)),
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

/// Encode `input` to Base32 or decode a Base32 string back.
///
/// - `mode` (`"encode"` | `"decode"`, blank → `"encode"`): direction.
/// - `variant` (`"standard"` | `"hex"` | `"crockford"` | `"zbase32"`, blank →
///   `"standard"`): the Base32 alphabet.
/// - `format` (`"text"` | `"hex"`, blank → `"text"`): when encoding, how to read
///   `input` (UTF-8 text or a hex byte string); when decoding, how to render the
///   recovered bytes (UTF-8 text or hex).
/// - `lowercase`: emit a lowercase alphabet when encoding standard/hex (ignored
///   for Crockford/z-base-32 and on decode, which is case-insensitive).
/// - `padding`: append `=` padding when encoding standard/hex (RFC 4648).
///   Crockford and z-base-32 are never padded.
///
/// Returns `Err` on an invalid `mode`/`variant`/`format`, malformed hex input,
/// an undecodable Base32 string, or decoded bytes that aren't valid UTF-8 (when
/// `format = "text"`).
pub fn convert(
    input: &str,
    mode: &str,
    variant: &str,
    format: &str,
    lowercase: bool,
    padding: bool,
) -> Result<String, String> {
    let variant = Variant::parse(variant)?;
    let fmt = DataFormat::parse(format)?;

    match mode {
        "" | "encode" => {
            let bytes = fmt.to_bytes(input)?;
            Ok(base32::encode(variant.alphabet(lowercase, padding), &bytes))
        }
        "decode" => {
            // Strip surrounding whitespace. The decoder accepts the unpadded
            // form for the padded alphabets too, so try a padded decode first
            // and fall back to unpadded if it fails.
            let trimmed = input.trim();
            let bytes = base32::decode(variant.alphabet(lowercase, true), trimmed)
                .or_else(|| base32::decode(variant.alphabet(lowercase, false), trimmed))
                .ok_or_else(|| {
                    "could not decode: the input is not a valid Base32 string for this variant".to_string()
                })?;
            fmt.from_bytes(&bytes)
        }
        other => Err(format!(
            "invalid mode {other:?}: expected \"encode\" or \"decode\""
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 4648 §10 standard Base32 test vectors.
    #[test]
    fn rfc4648_standard_vectors() {
        let cases = [
            ("", ""),
            ("f", "MY======"),
            ("fo", "MZXQ===="),
            ("foo", "MZXW6==="),
            ("foob", "MZXW6YQ="),
            ("fooba", "MZXW6YTB"),
            ("foobar", "MZXW6YTBOI======"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(
                convert(plain, "encode", "standard", "text", false, true).unwrap(),
                encoded,
                "encode {plain:?}"
            );
            assert_eq!(
                convert(encoded, "decode", "standard", "text", false, true).unwrap(),
                plain,
                "decode {encoded:?}"
            );
        }
    }

    // RFC 4648 §10 base32hex test vectors.
    #[test]
    fn rfc4648_hex_vectors() {
        let cases = [
            ("f", "CO======"),
            ("fo", "CPNG===="),
            ("foo", "CPNMU==="),
            ("foob", "CPNMUOG="),
            ("fooba", "CPNMUOJ1"),
            ("foobar", "CPNMUOJ1E8======"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(
                convert(plain, "encode", "hex", "text", false, true).unwrap(),
                encoded
            );
            assert_eq!(
                convert(encoded, "decode", "hex", "text", false, true).unwrap(),
                plain
            );
        }
    }

    #[test]
    fn encode_unpadded_omits_equals() {
        assert_eq!(
            convert("foo", "encode", "standard", "text", false, false).unwrap(),
            "MZXW6"
        );
        // Decode tolerates the unpadded form.
        assert_eq!(
            convert("MZXW6", "decode", "standard", "text", false, false).unwrap(),
            "foo"
        );
    }

    #[test]
    fn decode_accepts_padded_input_even_when_padding_off() {
        // The padded string still decodes when the requested padding is false.
        assert_eq!(
            convert("MZXW6===", "decode", "standard", "text", false, false).unwrap(),
            "foo"
        );
    }

    #[test]
    fn lowercase_encode() {
        assert_eq!(
            convert("foobar", "encode", "standard", "text", true, false).unwrap(),
            "mzxw6ytboi"
        );
        // Decode is case-insensitive — uppercase recovers the same data.
        assert_eq!(
            convert("MZXW6YTBOI", "decode", "standard", "text", false, false).unwrap(),
            "foobar"
        );
    }

    #[test]
    fn hex_input_encodes_raw_bytes() {
        // 0x666f6f == "foo".
        assert_eq!(
            convert("66 6f 6f", "encode", "standard", "hex", false, true).unwrap(),
            "MZXW6==="
        );
        assert_eq!(
            convert("0x666f6f", "encode", "standard", "hex", false, true).unwrap(),
            "MZXW6==="
        );
    }

    #[test]
    fn decode_to_hex_output() {
        assert_eq!(
            convert("MZXW6===", "decode", "standard", "hex", false, true).unwrap(),
            "666f6f"
        );
    }

    #[test]
    fn decode_non_utf8_to_text_errors_but_hex_works() {
        // 0xFF is a lone byte, never valid UTF-8. Encode it first.
        let b32 = convert("ff", "encode", "standard", "hex", false, true).unwrap();
        let err = convert(&b32, "decode", "standard", "text", false, true).unwrap_err();
        assert!(err.contains("UTF-8"), "got: {err}");
        assert_eq!(
            convert(&b32, "decode", "standard", "hex", false, true).unwrap(),
            "ff"
        );
    }

    #[test]
    fn crockford_round_trips_and_is_never_padded() {
        let enc = convert("hello", "encode", "crockford", "text", false, true).unwrap();
        // Padding flag is ignored for Crockford — no '=' ever.
        assert!(!enc.contains('='), "crockford should not pad: {enc}");
        assert_eq!(
            convert(&enc, "decode", "crockford", "text", false, false).unwrap(),
            "hello"
        );
    }

    #[test]
    fn zbase32_round_trips() {
        let enc = convert("hello world", "encode", "zbase32", "text", false, false).unwrap();
        assert!(!enc.contains('='));
        assert_eq!(
            convert(&enc, "decode", "zbase32", "text", false, false).unwrap(),
            "hello world"
        );
    }

    #[test]
    fn defaults_blank_args_to_encode_standard_text() {
        assert_eq!(convert("foobar", "", "", "", false, true).unwrap(), "MZXW6YTBOI======");
    }

    #[test]
    fn rejects_unknown_mode_variant_format() {
        assert!(convert("x", "zap", "standard", "text", false, true)
            .unwrap_err()
            .contains("invalid mode"));
        assert!(convert("x", "encode", "base999", "text", false, true)
            .unwrap_err()
            .contains("invalid variant"));
        assert!(convert("x", "encode", "standard", "binary", false, true)
            .unwrap_err()
            .contains("invalid data format"));
    }

    #[test]
    fn rejects_bad_hex_input() {
        assert!(convert("zz", "encode", "standard", "hex", false, true)
            .unwrap_err()
            .contains("invalid hex"));
        assert!(convert("abc", "encode", "standard", "hex", false, true)
            .unwrap_err()
            .contains("odd number"));
    }

    #[test]
    fn rejects_undecodable_base32() {
        // '1', '8', '9', '0' are not in the RFC 4648 standard alphabet.
        let err = convert("1111", "decode", "standard", "text", false, false).unwrap_err();
        assert!(err.contains("not a valid Base32"), "got: {err}");
    }
}
