//! bech32-codec core — encode a human-readable prefix (HRP) plus data to a
//! checksummed **Bech32** (BIP 173) or **Bech32m** (BIP 350) string, and decode
//! / validate Bech32 strings back to their prefix and data. Bech32 is the
//! encoding behind SegWit Bitcoin addresses, Lightning invoices, Nostr keys,
//! Cosmos addresses, and more: an HRP, a `1` separator, a base-32 data part, and
//! a 6-character BCH checksum that detects typos.
//!
//! This crate implements the full BIP 173 / BIP 350 reference algorithm with no
//! external dependencies (so it instantiates cleanly under wafer/wasm) and is
//! shared verbatim by the chat skill block, the CLI, and the web page.

/// The Bech32 data-part alphabet (base-32). Index = 5-bit value 0..=31.
const CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

/// Generator coefficients for the BCH checksum polymod (BIP 173).
const GEN: [u32; 5] = [0x3b6a_57b2, 0x2650_8e6d, 0x1ea1_19fa, 0x3d42_33dd, 0x2a14_62b3];

/// The two checksum "constants" that `polymod` must equal for a valid string.
const BECH32_CONST: u32 = 1;
const BECH32M_CONST: u32 = 0x2bc8_30a3;

/// Which checksum variant a string uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Variant {
    /// BIP 173 Bech32 (checksum constant 1). Used by SegWit v0 addresses.
    Bech32,
    /// BIP 350 Bech32m (checksum constant 0x2bc830a3). Used by SegWit v1+
    /// (Taproot) addresses and other newer schemes.
    Bech32m,
}

impl Variant {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "bech32" => Ok(Variant::Bech32),
            "bech32m" => Ok(Variant::Bech32m),
            other => Err(format!(
                "invalid variant {other:?}: expected \"bech32\" or \"bech32m\""
            )),
        }
    }

    fn checksum_const(self) -> u32 {
        match self {
            Variant::Bech32 => BECH32_CONST,
            Variant::Bech32m => BECH32M_CONST,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Variant::Bech32 => "bech32",
            Variant::Bech32m => "bech32m",
        }
    }
}

/// How the data payload is read on encode / rendered on decode.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DataFormat {
    /// UTF-8 text.
    Text,
    /// A hex byte string — the default (Bech32 payloads are usually binary).
    Hex,
}

impl DataFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "hex" => Ok(DataFormat::Hex),
            "text" => Ok(DataFormat::Text),
            other => Err(format!(
                "invalid data format {other:?}: expected \"hex\" or \"text\""
            )),
        }
    }
}

/// The BCH checksum polymod over a sequence of 5-bit values (BIP 173).
fn polymod(values: &[u8]) -> u32 {
    let mut chk: u32 = 1;
    for &v in values {
        let top = (chk >> 25) as u8;
        chk = ((chk & 0x1ff_ffff) << 5) ^ (v as u32);
        for (i, g) in GEN.iter().enumerate() {
            if (top >> i) & 1 == 1 {
                chk ^= g;
            }
        }
    }
    chk
}

/// Expand the HRP into the value sequence the checksum is computed over: the
/// high 3 bits of every char, a `0` separator, then the low 5 bits of every char.
fn hrp_expand(hrp: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(hrp.len() * 2 + 1);
    v.extend(hrp.iter().map(|c| c >> 5));
    v.push(0);
    v.extend(hrp.iter().map(|c| c & 31));
    v
}

/// Compute the 6-symbol checksum (as 5-bit values) for an HRP + data part.
fn create_checksum(hrp: &[u8], data: &[u8], variant: Variant) -> Vec<u8> {
    let mut values = hrp_expand(hrp);
    values.extend_from_slice(data);
    values.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    let m = polymod(&values) ^ variant.checksum_const();
    (0..6).map(|i| ((m >> (5 * (5 - i))) & 31) as u8).collect()
}

/// Verify the checksum of an HRP + full data-with-checksum part, returning the
/// variant whose constant matches, or `None` if neither does.
fn verify_checksum(hrp: &[u8], data: &[u8]) -> Option<Variant> {
    let mut values = hrp_expand(hrp);
    values.extend_from_slice(data);
    match polymod(&values) {
        BECH32_CONST => Some(Variant::Bech32),
        BECH32M_CONST => Some(Variant::Bech32m),
        _ => None,
    }
}

/// General power-of-two base conversion (BIP 173 `convertbits`). Packs `from`-bit
/// input values into `to`-bit output values, MSB-first. When `pad` is true a
/// trailing partial group is zero-padded (encode, 8→5); when false, leftover bits
/// must be zero and fewer than `from` or the input is rejected (decode, 5→8).
fn convert_bits(data: &[u8], from: u32, to: u32, pad: bool) -> Option<Vec<u8>> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut out = Vec::with_capacity(data.len() * from as usize / to as usize + 1);
    let maxv: u32 = (1 << to) - 1;
    let max_acc: u32 = (1 << (from + to - 1)) - 1;
    for &value in data {
        let v = value as u32;
        if (v >> from) != 0 {
            return None; // value has more than `from` bits
        }
        acc = ((acc << from) | v) & max_acc;
        bits += from;
        while bits >= to {
            bits -= to;
            out.push(((acc >> bits) & maxv) as u8);
        }
    }
    if pad {
        if bits > 0 {
            out.push(((acc << (to - bits)) & maxv) as u8);
        }
    } else if bits >= from || ((acc << (to - bits)) & maxv) != 0 {
        return None; // non-zero padding bits
    }
    Some(out)
}

/// Validate an HRP for encoding and return its lowercase bytes. The HRP must be
/// 1..=83 characters, each in the printable US-ASCII range 33..=126.
fn normalize_hrp(hrp: &str) -> Result<Vec<u8>, String> {
    let hrp = hrp.trim();
    if hrp.is_empty() {
        return Err("a human-readable prefix (HRP) is required to encode, e.g. \"bc\" for a Bitcoin mainnet address".into());
    }
    if hrp.len() > 83 {
        return Err(format!(
            "HRP is too long ({} chars); Bech32 allows at most 83",
            hrp.len()
        ));
    }
    let mut out = Vec::with_capacity(hrp.len());
    for c in hrp.chars() {
        if !c.is_ascii() || (c as u32) < 33 || (c as u32) > 126 {
            return Err(format!(
                "invalid HRP character {c:?}: must be printable US-ASCII (33..=126)"
            ));
        }
        out.push(c.to_ascii_lowercase() as u8);
    }
    Ok(out)
}

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

/// Encode an HRP plus a data part (already 5-bit values) into a Bech32 string.
fn encode_words(hrp: &[u8], data: &[u8], variant: Variant) -> String {
    let checksum = create_checksum(hrp, data, variant);
    let mut s = Vec::with_capacity(hrp.len() + 1 + data.len() + 6);
    s.extend_from_slice(hrp);
    s.push(b'1');
    for &d in data.iter().chain(checksum.iter()) {
        s.push(CHARSET[d as usize]);
    }
    // HRP is validated ASCII and CHARSET is ASCII, so this is valid UTF-8.
    String::from_utf8(s).expect("bech32 output is ASCII")
}

/// Decode a Bech32/Bech32m string into (hrp, data-5bit-values, variant),
/// validating case, structure, alphabet, and checksum.
fn decode_words(s: &str) -> Result<(Vec<u8>, Vec<u8>, Variant), String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty input: nothing to decode".into());
    }
    // No mixed case allowed (BIP 173): the whole string is either all-lower or
    // all-upper (ignoring digits/punctuation).
    let has_lower = s.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
    if has_lower && has_upper {
        return Err("mixed-case Bech32 string is not allowed; use all lowercase or all uppercase".into());
    }
    // Every character must be printable ASCII in 33..=126.
    for c in s.chars() {
        if !c.is_ascii() || (c as u32) < 33 || (c as u32) > 126 {
            return Err(format!("invalid character {c:?}: Bech32 strings are printable US-ASCII"));
        }
    }
    let lower = s.to_ascii_lowercase();
    let sep = lower
        .rfind('1')
        .ok_or("missing the '1' separator between the prefix and the data part")?;
    if sep == 0 {
        return Err("the human-readable prefix (before '1') must not be empty".into());
    }
    let (hrp, rest) = lower.split_at(sep);
    let data_part = &rest[1..]; // skip the '1'
    if data_part.len() < 6 {
        return Err(format!(
            "the data part is too short ({} chars); it must hold at least the 6-character checksum",
            data_part.len()
        ));
    }
    if hrp.len() > 83 {
        return Err(format!("HRP is too long ({} chars); Bech32 allows at most 83", hrp.len()));
    }
    // Map each data character to its 5-bit value.
    let mut values = Vec::with_capacity(data_part.len());
    for c in data_part.bytes() {
        match CHARSET.iter().position(|&x| x == c) {
            Some(v) => values.push(v as u8),
            None => {
                return Err(format!(
                    "invalid data character {:?}: not in the Bech32 alphabet (excludes 1, b, i, o)",
                    c as char
                ))
            }
        }
    }
    let hrp_bytes = hrp.as_bytes().to_vec();
    let variant = verify_checksum(&hrp_bytes, &values)
        .ok_or("checksum mismatch: the string is not a valid Bech32 or Bech32m encoding (a typo or altered character)")?;
    let data5 = values[..values.len() - 6].to_vec();
    Ok((hrp_bytes, data5, variant))
}

/// Encode or decode a Bech32 / Bech32m string.
///
/// - `mode` (`"encode"` | `"decode"`, blank → `"encode"`): direction.
/// - `input`: on encode, the data payload (hex or text per `format`); on decode,
///   the Bech32 string to validate and unpack.
/// - `hrp` (encode only): the human-readable prefix, e.g. `bc`, `tb`, `lnbc`,
///   `npub`. Lower-cased in the output. Ignored on decode (read from the string).
/// - `variant` (`"bech32"` | `"bech32m"`, blank → `"bech32"`): which checksum to
///   write on encode. On decode the variant is auto-detected and reported.
/// - `format` (`"hex"` | `"text"`, blank → `"hex"`): how to read the payload on
///   encode, or render it on decode. `hex` suits binary data; `text` is UTF-8.
///
/// Encode returns the Bech32 string. Decode returns a short report with the HRP,
/// detected variant, and the decoded data.
pub fn convert(
    input: &str,
    mode: &str,
    hrp: &str,
    variant: &str,
    format: &str,
) -> Result<String, String> {
    let fmt = DataFormat::parse(format)?;
    match mode.trim().to_ascii_lowercase().as_str() {
        "" | "encode" => {
            let variant = Variant::parse(variant)?;
            let hrp = normalize_hrp(hrp)?;
            let bytes = match fmt {
                DataFormat::Text => input.as_bytes().to_vec(),
                DataFormat::Hex => parse_hex(input)?,
            };
            let data5 = convert_bits(&bytes, 8, 5, true)
                .ok_or("failed to convert the data to 5-bit groups")?;
            Ok(encode_words(&hrp, &data5, variant))
        }
        "decode" => {
            let (hrp, data5, variant) = decode_words(input)?;
            let bytes = convert_bits(&data5, 5, 8, false).ok_or(
                "the data part has invalid padding bits and does not decode to whole bytes",
            )?;
            let rendered = match fmt {
                DataFormat::Hex => to_hex(&bytes),
                DataFormat::Text => String::from_utf8(bytes).map_err(|_| {
                    "decoded bytes are not valid UTF-8 — set format 'hex' to view the raw bytes"
                        .to_string()
                })?,
            };
            let hrp_str = String::from_utf8(hrp).expect("hrp is ASCII");
            Ok(format!(
                "hrp: {hrp_str}\nvariant: {}\ndata: {rendered}",
                variant.label()
            ))
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
    fn empty_data_known_vectors() {
        // BIP 173 / BIP 350 canonical empty-data checksums.
        assert_eq!(encode_words(b"a", &[], Variant::Bech32), "a12uel5l");
        assert_eq!(encode_words(b"a", &[], Variant::Bech32m), "a1lqfn3a");
        assert_eq!(encode_words(b"?", &[], Variant::Bech32), "?1ezyfcl");
        assert_eq!(encode_words(b"?", &[], Variant::Bech32m), "?1v759aa");
    }

    #[test]
    fn bip173_full_charset_vector_roundtrips() {
        // "abcdef1qpzry9x8gf2tvdw0s3jn54khce6mua7lmqqqxw" carries data values 0..=31.
        let s = "abcdef1qpzry9x8gf2tvdw0s3jn54khce6mua7lmqqqxw";
        let (hrp, data5, variant) = decode_words(s).unwrap();
        assert_eq!(hrp, b"abcdef");
        assert_eq!(variant, Variant::Bech32);
        assert_eq!(data5, (0u8..32).collect::<Vec<_>>());
        // Re-encoding the same words reproduces the exact string.
        assert_eq!(encode_words(&hrp, &data5, variant), s);
    }

    #[test]
    fn decode_detects_bech32m() {
        // BIP 350 valid Bech32m string.
        let (hrp, _data, variant) = decode_words("A1LQFN3A").unwrap();
        assert_eq!(hrp, b"a");
        assert_eq!(variant, Variant::Bech32m);
    }

    #[test]
    fn convert_encode_hex_roundtrips() {
        // Arbitrary 20-byte payload → bech32 → decode back to the same hex.
        let hex = "751e76e8199196d454941c45d1b3a323f1433bd6";
        let enc = convert(hex, "encode", "bc", "bech32", "hex").unwrap();
        assert!(enc.starts_with("bc1"), "got {enc}");
        let dec = convert(&enc, "decode", "", "", "hex").unwrap();
        assert!(dec.contains("hrp: bc"), "got {dec}");
        assert!(dec.contains("variant: bech32"), "got {dec}");
        assert!(dec.contains(&format!("data: {hex}")), "got {dec}");
    }

    #[test]
    fn convert_encode_text_roundtrips() {
        let enc = convert("hello", "encode", "test", "bech32m", "text").unwrap();
        let dec = convert(&enc, "decode", "", "", "text").unwrap();
        assert!(dec.contains("hrp: test"));
        assert!(dec.contains("variant: bech32m"));
        assert!(dec.contains("data: hello"));
    }

    #[test]
    fn defaults_blank_args_encode_bech32_hex() {
        // blank mode/variant/format → encode, bech32, hex.
        let enc = convert("00", "", "bc", "", "").unwrap();
        assert!(enc.starts_with("bc1"), "got {enc}");
        // 1 byte 0x00 → convertbits(8,5) → [0,0] → "qq".
        assert_eq!(enc, encode_words(b"bc", &[0, 0], Variant::Bech32));
    }

    #[test]
    fn hex_input_tolerates_whitespace_and_prefix() {
        let a = convert("0x 75 1e", "encode", "bc", "bech32", "hex").unwrap();
        let b = convert("751e", "encode", "bc", "bech32", "hex").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn decode_rejects_bad_checksum() {
        // Flip one character of a valid string.
        let err = convert("a12uel5m", "decode", "", "", "hex").unwrap_err();
        assert!(err.contains("checksum"), "got: {err}");
    }

    #[test]
    fn decode_rejects_mixed_case() {
        let err = convert("A12Uel5l", "decode", "", "", "hex").unwrap_err();
        assert!(err.contains("mixed-case"), "got: {err}");
    }

    #[test]
    fn decode_rejects_missing_separator() {
        let err = convert("qpzry9x8", "decode", "", "", "hex").unwrap_err();
        // "qpzry9x8" has no '1' at all.
        assert!(err.contains("separator"), "got: {err}");
    }

    #[test]
    fn decode_rejects_invalid_data_char() {
        // 'b' is excluded from the Bech32 alphabet.
        let err = convert("bc1bqqqqqqqqq", "decode", "", "", "hex").unwrap_err();
        assert!(
            err.contains("invalid data character") || err.contains("checksum"),
            "got: {err}"
        );
    }

    #[test]
    fn encode_requires_hrp() {
        let err = convert("00", "encode", "", "bech32", "hex").unwrap_err();
        assert!(err.contains("HRP") || err.contains("prefix"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_mode_variant_format() {
        assert!(convert("x", "zap", "bc", "bech32", "hex")
            .unwrap_err()
            .contains("invalid mode"));
        assert!(convert("00", "encode", "bc", "base99", "hex")
            .unwrap_err()
            .contains("invalid variant"));
        assert!(convert("00", "encode", "bc", "bech32", "binary")
            .unwrap_err()
            .contains("invalid data format"));
    }

    #[test]
    fn rejects_bad_hex() {
        assert!(convert("zz", "encode", "bc", "bech32", "hex")
            .unwrap_err()
            .contains("invalid hex"));
        assert!(convert("abc", "encode", "bc", "bech32", "hex")
            .unwrap_err()
            .contains("odd number"));
    }

    #[test]
    fn uppercase_decode_roundtrips_to_same_words() {
        // Upper and lower forms decode identically.
        let lower = decode_words("a12uel5l").unwrap();
        let upper = decode_words("A12UEL5L").unwrap();
        assert_eq!(lower.0, upper.0);
        assert_eq!(lower.1, upper.1);
        assert_eq!(lower.2, upper.2);
    }
}
