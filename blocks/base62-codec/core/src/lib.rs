//! base62-codec core — encode text, hex bytes, or a decimal number to Base62
//! (the `0-9A-Za-z` alphabet, plus the `0-9a-zA-Z` "inverted" variant) and
//! decode a Base62 string back. Byte encoding is a big-integer base conversion
//! that preserves leading-zero bytes; number encoding converts an
//! arbitrary-precision non-negative integer. Pure compute, no wafer/wasm-bindgen
//! deps — shared by the chat skill block and the web page.

/// The supported Base62 alphabets. Both hold all 62 characters (digits, upper,
/// lower) and differ only in whether uppercase or lowercase comes first.
///
/// - `standard`: `0-9A-Za-z` — digits, then uppercase, then lowercase. This is
///   the GMP / most-common Base62 order and the default.
/// - `inverted`: `0-9a-zA-Z` — digits, then lowercase, then uppercase (used by
///   some short-ID libraries).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Standard,
    Inverted,
}

const STANDARD: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const INVERTED: &[u8; 62] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

impl Variant {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "standard" | "gmp" => Ok(Variant::Standard),
            "inverted" => Ok(Variant::Inverted),
            other => Err(format!(
                "invalid variant {other:?}: expected \"standard\" or \"inverted\""
            )),
        }
    }

    fn alphabet(self) -> &'static [u8; 62] {
        match self {
            Variant::Standard => STANDARD,
            Variant::Inverted => INVERTED,
        }
    }
}

/// How to interpret the input when encoding / how to render the result when
/// decoding.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DataFormat {
    /// UTF-8 text — the default.
    Text,
    /// A hex string (e.g. `48 65 6c` or `0x48656c`), case-insensitive.
    Hex,
    /// A non-negative decimal integer of arbitrary size.
    Number,
}

impl DataFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "text" => Ok(DataFormat::Text),
            "hex" => Ok(DataFormat::Hex),
            "number" | "int" | "integer" => Ok(DataFormat::Number),
            other => Err(format!(
                "invalid data format {other:?}: expected \"text\", \"hex\", or \"number\""
            )),
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

/// Parse a non-negative decimal integer (arbitrary precision) into its minimal
/// big-endian byte representation. An empty string, `+`, or any non-digit is an
/// error. The value `0` maps to an empty vector (no significant bytes).
fn decimal_to_bytes(s: &str) -> Result<Vec<u8>, String> {
    let trimmed = s.trim();
    let digits = trimmed.strip_prefix('+').unwrap_or(trimmed);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "invalid number {s:?}: expected a non-negative integer (digits 0-9 only)"
        ));
    }
    // Big-endian base-256 accumulation: bytes = bytes * 10 + digit.
    let mut bytes: Vec<u8> = Vec::new();
    for ch in digits.bytes() {
        let mut carry = (ch - b'0') as u32;
        for byte in bytes.iter_mut().rev() {
            let v = (*byte as u32) * 10 + carry;
            *byte = (v & 0xff) as u8;
            carry = v >> 8;
        }
        while carry > 0 {
            bytes.insert(0, (carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    // Drop any leading zero bytes so the representation is minimal.
    let first = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
    Ok(bytes[first..].to_vec())
}

/// Render big-endian bytes as a non-negative decimal integer string. Leading
/// zero bytes do not affect the value; an empty slice is `"0"`.
fn bytes_to_decimal(bytes: &[u8]) -> String {
    // Big-endian decimal-digit accumulation: digits = digits * 256 + byte.
    let mut digits: Vec<u8> = Vec::new();
    for &byte in bytes {
        let mut carry = byte as u32;
        for d in digits.iter_mut().rev() {
            let v = (*d as u32) * 256 + carry;
            *d = (v % 10) as u8;
            carry = v / 10;
        }
        while carry > 0 {
            digits.insert(0, (carry % 10) as u8);
            carry /= 10;
        }
    }
    if digits.is_empty() {
        "0".to_string()
    } else {
        digits.iter().map(|&d| (b'0' + d) as char).collect()
    }
}

/// Encode raw bytes to a Base62 string using `alphabet`, preserving each
/// leading zero byte as a leading `0` (the alphabet's first symbol).
fn encode_bytes(bytes: &[u8], alphabet: &[u8; 62]) -> String {
    // Count leading zero bytes — each maps to one leading "zero" symbol.
    let zeros = bytes.iter().take_while(|&&b| b == 0).count();

    // Big-integer base conversion via repeated division (base 256 -> base 62).
    // `digits` holds base-62 digits, least-significant first.
    let mut digits: Vec<u8> = Vec::with_capacity(bytes.len() * 137 / 100 + 1);
    for &byte in &bytes[zeros..] {
        let mut carry = byte as u32;
        for d in digits.iter_mut() {
            carry += (*d as u32) << 8;
            *d = (carry % 62) as u8;
            carry /= 62;
        }
        while carry > 0 {
            digits.push((carry % 62) as u8);
            carry /= 62;
        }
    }

    let mut out = Vec::with_capacity(zeros + digits.len());
    for _ in 0..zeros {
        out.push(alphabet[0]);
    }
    for &d in digits.iter().rev() {
        out.push(alphabet[d as usize]);
    }
    // All bytes are ASCII from the alphabet, so this is always valid UTF-8.
    String::from_utf8(out).expect("base62 alphabet is ASCII")
}

/// Decode a Base62 string back to raw bytes using `alphabet`. Whitespace is
/// trimmed; any character not in the alphabet is an error. Leading "zero"
/// symbols become leading zero bytes.
fn decode_str(s: &str, alphabet: &[u8; 62]) -> Result<Vec<u8>, String> {
    // Build a reverse lookup: alphabet symbol -> digit value (0..62).
    let mut lookup = [255u8; 128];
    for (i, &c) in alphabet.iter().enumerate() {
        lookup[c as usize] = i as u8;
    }

    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let zero_sym = alphabet[0];
    let zeros = trimmed.bytes().take_while(|&b| b == zero_sym).count();

    // Big-integer base conversion (base 62 -> base 256).
    let mut bytes: Vec<u8> = Vec::with_capacity(trimmed.len());
    for c in trimmed.chars() {
        let value = if (c as u32) < 128 {
            lookup[c as usize]
        } else {
            255
        };
        if value == 255 {
            return Err(format!(
                "invalid Base62 character {c:?}: not in the selected alphabet"
            ));
        }
        let mut carry = value as u32;
        for b in bytes.iter_mut() {
            carry += (*b as u32) * 62;
            *b = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }

    let mut out = Vec::with_capacity(zeros + bytes.len());
    for _ in 0..zeros {
        out.push(0u8);
    }
    out.extend(bytes.iter().rev());
    Ok(out)
}

/// Encode `input` to Base62 or decode a Base62 string back.
///
/// - `mode` (`"encode"` | `"decode"`, blank → `"encode"`): direction.
/// - `variant` (`"standard"` | `"inverted"`, blank → `"standard"`): the Base62
///   alphabet. `standard` is `0-9A-Za-z`; `inverted` is `0-9a-zA-Z`.
/// - `format` (`"text"` | `"hex"` | `"number"`, blank → `"text"`): how to read
///   `input` when encoding, or render the result when decoding. `text` is UTF-8;
///   `hex` is a hex byte string; `number` is a non-negative decimal integer.
///
/// Byte encoding (`text`/`hex`) preserves leading-zero bytes as leading `0`
/// symbols; `number` encoding converts an arbitrary-precision integer with no
/// leading-zero padding (and `0` encodes to `"0"`). Returns `Err` on an invalid
/// `mode`/`variant`/`format`, malformed hex or decimal input, an undecodable
/// Base62 string, or decoded bytes that aren't valid UTF-8 (when `format = "text"`).
pub fn convert(input: &str, mode: &str, variant: &str, format: &str) -> Result<String, String> {
    let variant = Variant::parse(variant)?;
    let fmt = DataFormat::parse(format)?;
    let alphabet = variant.alphabet();

    match mode {
        "" | "encode" => match fmt {
            DataFormat::Text => Ok(encode_bytes(input.as_bytes(), alphabet)),
            DataFormat::Hex => Ok(encode_bytes(&parse_hex(input)?, alphabet)),
            DataFormat::Number => {
                let bytes = decimal_to_bytes(input)?;
                if bytes.is_empty() {
                    // The value is zero — render the single zero symbol.
                    Ok((alphabet[0] as char).to_string())
                } else {
                    Ok(encode_bytes(&bytes, alphabet))
                }
            }
        },
        "decode" => {
            let bytes = decode_str(input, alphabet)?;
            match fmt {
                DataFormat::Text => String::from_utf8(bytes).map_err(|_| {
                    "decoded bytes are not valid UTF-8 — set format 'hex' to view the raw bytes, or 'number' if it's an integer".into()
                }),
                DataFormat::Hex => Ok(to_hex(&bytes)),
                DataFormat::Number => Ok(bytes_to_decimal(&bytes)),
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
    fn standard_text_vectors() {
        let cases = [
            ("", ""),
            ("foo", "SAPP"),
            ("Hello World!", "T8dgcjRGkZ3aysdN"),
            (
                "The quick brown fox jumps over the lazy dog.",
                "XGPLPeg6g0VCuZCSfg8LKEZOBVFQ79VvzR8f2OlDiCg5SwBJDmeq6nysKtS",
            ),
        ];
        for (plain, encoded) in cases {
            assert_eq!(
                convert(plain, "encode", "standard", "text").unwrap(),
                encoded,
                "encode {plain:?}"
            );
            assert_eq!(
                convert(encoded, "decode", "standard", "text").unwrap(),
                plain,
                "decode {encoded:?}"
            );
        }
    }

    #[test]
    fn standard_hex_vectors() {
        let cases = [
            ("61", "1Z"),
            ("626262", "R3LO"),
            ("516b6fcd0f", "69hruW7"),
            ("ff", "47"),
        ];
        for (hex, encoded) in cases {
            assert_eq!(
                convert(hex, "encode", "standard", "hex").unwrap(),
                encoded,
                "encode {hex:?}"
            );
            assert_eq!(
                convert(encoded, "decode", "standard", "hex").unwrap(),
                hex,
                "decode {encoded:?}"
            );
        }
    }

    #[test]
    fn number_vectors() {
        let cases = [
            ("0", "0"),
            ("1", "1"),
            ("61", "z"),
            ("62", "10"),
            ("63", "11"),
            ("12345", "3D7"),
            ("999999", "4C91"),
            ("4294967295", "4gfFC3"),
            // Well beyond u128 range — proves arbitrary precision.
            (
                "340282366920938463463374607431768211456",
                "7n42DGM5Tflk9n8mt7Fhc8",
            ),
        ];
        for (num, encoded) in cases {
            assert_eq!(
                convert(num, "encode", "standard", "number").unwrap(),
                encoded,
                "encode number {num:?}"
            );
            assert_eq!(
                convert(encoded, "decode", "standard", "number").unwrap(),
                num,
                "decode number {encoded:?}"
            );
        }
    }

    #[test]
    fn preserves_leading_zero_bytes() {
        // Each leading 0x00 byte must become a leading '0' symbol.
        assert_eq!(convert("00", "encode", "standard", "hex").unwrap(), "0");
        assert_eq!(convert("0000", "encode", "standard", "hex").unwrap(), "00");
        let enc = convert("0000287fb4cd", "encode", "standard", "hex").unwrap();
        assert_eq!(enc, "00jyw3x");
        assert_eq!(
            convert(&enc, "decode", "standard", "hex").unwrap(),
            "0000287fb4cd"
        );
    }

    #[test]
    fn number_ignores_leading_zero_symbols_on_decode() {
        // Leading zero symbols do not change the integer value.
        assert_eq!(
            convert("003D7", "decode", "standard", "number").unwrap(),
            "12345"
        );
    }

    #[test]
    fn empty_round_trips() {
        assert_eq!(convert("", "encode", "standard", "text").unwrap(), "");
        assert_eq!(convert("", "decode", "standard", "text").unwrap(), "");
    }

    #[test]
    fn inverted_alphabet_differs_and_round_trips() {
        // Same bytes, different alphabet => lowercase-shifted output.
        assert_eq!(
            convert("foo", "encode", "standard", "text").unwrap(),
            "SAPP"
        );
        assert_eq!(
            convert("foo", "encode", "inverted", "text").unwrap(),
            "sapp"
        );
        assert_eq!(
            convert("sapp", "decode", "inverted", "text").unwrap(),
            "foo"
        );
        assert_eq!(
            convert("12345", "encode", "inverted", "number").unwrap(),
            "3d7"
        );
    }

    #[test]
    fn defaults_blank_args_to_encode_standard_text() {
        assert_eq!(
            convert("Hello World!", "", "", "").unwrap(),
            "T8dgcjRGkZ3aysdN"
        );
    }

    #[test]
    fn hex_input_tolerates_whitespace_and_prefix() {
        assert_eq!(convert("0x 61", "encode", "standard", "hex").unwrap(), "1Z");
        assert_eq!(
            convert("62 62 62", "encode", "standard", "hex").unwrap(),
            "R3LO"
        );
    }

    #[test]
    fn decode_trims_surrounding_whitespace() {
        assert_eq!(
            convert("  1Z \n", "decode", "standard", "hex").unwrap(),
            "61"
        );
    }

    #[test]
    fn decode_non_utf8_to_text_errors_but_hex_works() {
        let enc = convert("ff", "encode", "standard", "hex").unwrap();
        let err = convert(&enc, "decode", "standard", "text").unwrap_err();
        assert!(err.contains("UTF-8"), "got: {err}");
        assert_eq!(convert(&enc, "decode", "standard", "hex").unwrap(), "ff");
    }

    #[test]
    fn rejects_unknown_mode_variant_format() {
        assert!(convert("x", "zap", "standard", "text")
            .unwrap_err()
            .contains("invalid mode"));
        assert!(convert("x", "encode", "base999", "text")
            .unwrap_err()
            .contains("invalid variant"));
        assert!(convert("x", "encode", "standard", "binary")
            .unwrap_err()
            .contains("invalid data format"));
    }

    #[test]
    fn rejects_bad_hex_and_number_input() {
        assert!(convert("zz", "encode", "standard", "hex")
            .unwrap_err()
            .contains("invalid hex"));
        assert!(convert("abc", "encode", "standard", "hex")
            .unwrap_err()
            .contains("odd number"));
        assert!(convert("-5", "encode", "standard", "number")
            .unwrap_err()
            .contains("invalid number"));
        assert!(convert("12.5", "encode", "standard", "number")
            .unwrap_err()
            .contains("invalid number"));
    }

    #[test]
    fn rejects_undecodable_base62() {
        // '+' and '/' are not part of the Base62 alphabet.
        let err = convert("ab+cd", "decode", "standard", "text").unwrap_err();
        assert!(err.contains("invalid Base62 character"), "got: {err}");
    }
}
