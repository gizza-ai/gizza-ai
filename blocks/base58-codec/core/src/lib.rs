//! base58-codec core — encode bytes to Base58 (the Bitcoin/IPFS alphabet, plus
//! the Ripple and Flickr variants) and decode a Base58 string back to bytes,
//! preserving leading-zero bytes. Pure compute, no wafer/wasm-bindgen deps —
//! shared by the chat skill block and the web page.

/// The supported Base58 alphabets (all 58 characters, differing only in order).
///
/// - `bitcoin`: the original Satoshi alphabet, also used by IPFS, Monero, and
///   most "base58" tooling — `123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz`
///   (the default).
/// - `ripple`: the XRP Ledger alphabet — `rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz`.
/// - `flickr`: Flickr's short-URL alphabet — lowercase before uppercase,
///   `123456789abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Bitcoin,
    Ripple,
    Flickr,
}

const BITCOIN: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const RIPPLE: &[u8; 58] = b"rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz";
const FLICKR: &[u8; 58] = b"123456789abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ";

impl Variant {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "bitcoin" | "btc" | "ipfs" => Ok(Variant::Bitcoin),
            "ripple" | "xrp" => Ok(Variant::Ripple),
            "flickr" => Ok(Variant::Flickr),
            other => Err(format!(
                "invalid variant {other:?}: expected \"bitcoin\", \"ripple\", or \"flickr\""
            )),
        }
    }

    fn alphabet(self) -> &'static [u8; 58] {
        match self {
            Variant::Bitcoin => BITCOIN,
            Variant::Ripple => RIPPLE,
            Variant::Flickr => FLICKR,
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

/// Encode raw bytes to a Base58 string using `alphabet`, preserving each
/// leading zero byte as a leading "1" (the alphabet's first symbol).
fn encode_bytes(bytes: &[u8], alphabet: &[u8; 58]) -> String {
    // Count leading zero bytes — each maps to one leading "zero" symbol.
    let zeros = bytes.iter().take_while(|&&b| b == 0).count();

    // Big-integer base conversion via repeated division (base 256 -> base 58).
    // `digits` holds base-58 digits, least-significant first.
    let mut digits: Vec<u8> = Vec::with_capacity(bytes.len() * 138 / 100 + 1);
    for &byte in &bytes[zeros..] {
        let mut carry = byte as u32;
        for d in digits.iter_mut() {
            carry += (*d as u32) << 8;
            *d = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
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
    String::from_utf8(out).expect("base58 alphabet is ASCII")
}

/// Decode a Base58 string back to raw bytes using `alphabet`. Whitespace is
/// trimmed; any character not in the alphabet is an error. Leading "zero"
/// symbols become leading zero bytes.
fn decode_str(s: &str, alphabet: &[u8; 58]) -> Result<Vec<u8>, String> {
    // Build a reverse lookup: alphabet symbol -> digit value (0..58).
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

    // Big-integer base conversion (base 58 -> base 256).
    let mut bytes: Vec<u8> = Vec::with_capacity(trimmed.len());
    for c in trimmed.chars() {
        let value = if (c as u32) < 128 {
            lookup[c as usize]
        } else {
            255
        };
        if value == 255 {
            return Err(format!(
                "invalid Base58 character {c:?}: not in the selected alphabet"
            ));
        }
        let mut carry = value as u32;
        for b in bytes.iter_mut() {
            carry += (*b as u32) * 58;
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

/// Encode `input` to Base58 or decode a Base58 string back.
///
/// - `mode` (`"encode"` | `"decode"`, blank → `"encode"`): direction.
/// - `variant` (`"bitcoin"` | `"ripple"` | `"flickr"`, blank → `"bitcoin"`):
///   the Base58 alphabet.
/// - `format` (`"text"` | `"hex"`, blank → `"text"`): when encoding, how to read
///   `input` (UTF-8 text or a hex byte string); when decoding, how to render the
///   recovered bytes (UTF-8 text or hex).
///
/// Base58 has no padding and preserves leading-zero bytes (rendered as the
/// alphabet's first symbol). Returns `Err` on an invalid `mode`/`variant`/`format`,
/// malformed hex input, an undecodable Base58 string, or decoded bytes that
/// aren't valid UTF-8 (when `format = "text"`).
pub fn convert(input: &str, mode: &str, variant: &str, format: &str) -> Result<String, String> {
    let variant = Variant::parse(variant)?;
    let fmt = DataFormat::parse(format)?;
    let alphabet = variant.alphabet();

    match mode {
        "" | "encode" => {
            let bytes = fmt.to_bytes(input)?;
            Ok(encode_bytes(&bytes, alphabet))
        }
        "decode" => {
            let bytes = decode_str(input, alphabet)?;
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

    // Bitcoin-alphabet vectors (verified against the reference implementation).
    #[test]
    fn bitcoin_text_vectors() {
        let cases = [
            ("", ""),
            ("Hello World!", "2NEpo7TZRRrLZSi2U"),
            (
                "The quick brown fox jumps over the lazy dog.",
                "USm3fpXnKG5EUBx2ndxBDMPVciP5hGey2Jh4NDv6gmeo1LkMeiKrLJUUBk6Z",
            ),
        ];
        for (plain, encoded) in cases {
            assert_eq!(
                convert(plain, "encode", "bitcoin", "text").unwrap(),
                encoded,
                "encode {plain:?}"
            );
            assert_eq!(
                convert(encoded, "decode", "bitcoin", "text").unwrap(),
                plain,
                "decode {encoded:?}"
            );
        }
    }

    // Hex vectors from the original base58 test suite.
    #[test]
    fn bitcoin_hex_vectors() {
        let cases = [
            ("61", "2g"),
            ("626262", "a3gV"),
            ("636363", "aPEr"),
            ("516b6fcd0f", "ABnLTmg"),
            ("ecac89cad93923c02321", "EJDM8drfXA6uyA"),
        ];
        for (hex, encoded) in cases {
            assert_eq!(
                convert(hex, "encode", "bitcoin", "hex").unwrap(),
                encoded,
                "encode {hex:?}"
            );
            assert_eq!(
                convert(encoded, "decode", "bitcoin", "hex").unwrap(),
                hex,
                "decode {encoded:?}"
            );
        }
    }

    #[test]
    fn preserves_leading_zero_bytes() {
        // Each leading 0x00 byte must become a leading '1'.
        assert_eq!(convert("00", "encode", "bitcoin", "hex").unwrap(), "1");
        assert_eq!(convert("0000", "encode", "bitcoin", "hex").unwrap(), "11");
        // 0x0000287fb4cd round-trips with two leading 1s.
        let enc = convert("0000287fb4cd", "encode", "bitcoin", "hex").unwrap();
        assert!(enc.starts_with("11"), "got {enc}");
        assert_eq!(
            convert(&enc, "decode", "bitcoin", "hex").unwrap(),
            "0000287fb4cd"
        );
    }

    #[test]
    fn empty_round_trips() {
        assert_eq!(convert("", "encode", "bitcoin", "text").unwrap(), "");
        assert_eq!(convert("", "decode", "bitcoin", "text").unwrap(), "");
    }

    #[test]
    fn ripple_alphabet_differs_from_bitcoin() {
        // Same bytes, different alphabet => different string.
        let btc = convert("Hello", "encode", "bitcoin", "text").unwrap();
        let xrp = convert("Hello", "encode", "ripple", "text").unwrap();
        assert_ne!(btc, xrp);
        assert_eq!(convert(&xrp, "decode", "ripple", "text").unwrap(), "Hello");
    }

    #[test]
    fn flickr_alphabet_round_trips() {
        let enc = convert("flickr photo id", "encode", "flickr", "text").unwrap();
        assert_eq!(
            convert(&enc, "decode", "flickr", "text").unwrap(),
            "flickr photo id"
        );
    }

    #[test]
    fn flickr_alphabet_differs_from_bitcoin() {
        // Flickr puts lowercase before uppercase, so high digit values map to
        // different symbols than Bitcoin.
        let f = convert("ffeedd", "encode", "flickr", "hex").unwrap();
        let b = convert("ffeedd", "encode", "bitcoin", "hex").unwrap();
        assert_ne!(f, b);
    }

    #[test]
    fn hex_input_tolerates_whitespace_and_prefix() {
        assert_eq!(convert("0x 61", "encode", "bitcoin", "hex").unwrap(), "2g");
        assert_eq!(
            convert("62 62 62", "encode", "bitcoin", "hex").unwrap(),
            "a3gV"
        );
    }

    #[test]
    fn decode_trims_surrounding_whitespace() {
        assert_eq!(convert("  2g \n", "decode", "bitcoin", "hex").unwrap(), "61");
    }

    #[test]
    fn defaults_blank_args_to_encode_bitcoin_text() {
        assert_eq!(
            convert("Hello World!", "", "", "").unwrap(),
            "2NEpo7TZRRrLZSi2U"
        );
    }

    #[test]
    fn decode_non_utf8_to_text_errors_but_hex_works() {
        // Encode a lone 0xFF byte, then decode it back.
        let enc = convert("ff", "encode", "bitcoin", "hex").unwrap();
        let err = convert(&enc, "decode", "bitcoin", "text").unwrap_err();
        assert!(err.contains("UTF-8"), "got: {err}");
        assert_eq!(convert(&enc, "decode", "bitcoin", "hex").unwrap(), "ff");
    }

    #[test]
    fn rejects_unknown_mode_variant_format() {
        assert!(convert("x", "zap", "bitcoin", "text")
            .unwrap_err()
            .contains("invalid mode"));
        assert!(convert("x", "encode", "base999", "text")
            .unwrap_err()
            .contains("invalid variant"));
        assert!(convert("x", "encode", "bitcoin", "binary")
            .unwrap_err()
            .contains("invalid data format"));
    }

    #[test]
    fn rejects_bad_hex_input() {
        assert!(convert("zz", "encode", "bitcoin", "hex")
            .unwrap_err()
            .contains("invalid hex"));
        assert!(convert("abc", "encode", "bitcoin", "hex")
            .unwrap_err()
            .contains("odd number"));
    }

    #[test]
    fn rejects_undecodable_base58() {
        // '0', 'O', 'I', 'l' are excluded from the Bitcoin alphabet.
        let err = convert("0OIl", "decode", "bitcoin", "text").unwrap_err();
        assert!(err.contains("invalid Base58 character"), "got: {err}");
    }
}
