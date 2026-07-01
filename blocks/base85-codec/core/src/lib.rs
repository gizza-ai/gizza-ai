//! base85-codec core — encode text or hex bytes to Base85 and decode Base85
//! back, in three real-world variants: **Ascii85** (the Adobe / btoa encoding,
//! with the `z` zero-group shortcut and optional `<~ ~>` framing), **Z85** (the
//! ZeroMQ RFC 32 encoding, whose input must be a multiple of four bytes), and
//! **RFC 1924** (the base85 alphabet from RFC 1924, matching Python's
//! `base64.b85encode`). All three share the standard base85 algorithm — four
//! big-endian bytes become five base-85 digits — and differ in their 85-symbol
//! alphabet plus a few per-variant rules. Pure compute, no wafer/wasm-bindgen
//! deps — shared by the chat skill block and the web page.

/// The supported Base85 variants. Each carries its own 85-symbol alphabet and a
/// few encoding rules (zero-group shortcut, framing, length constraints).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    /// Adobe / btoa Ascii85: alphabet `!`..`u` (byte values 33..=117), the `z`
    /// shortcut for an all-zero 4-byte group, optional `<~ ~>` framing, and
    /// partial final groups. The default.
    Ascii85,
    /// ZeroMQ Z85 (RFC 32): its own printable alphabet; input length must be a
    /// multiple of 4 (encoded length a multiple of 5). No shortcuts, no framing.
    Z85,
    /// RFC 1924 base85 alphabet, applied with the standard 4-byte grouping and
    /// partial final groups (matches Python's `base64.b85encode`). No shortcuts,
    /// no framing.
    Rfc1924,
}

// Ascii85 uses a contiguous run of ASCII, so it's generated (value v -> 33 + v)
// rather than spelled out. Z85 and RFC 1924 have irregular symbol orders.
const Z85_ALPHABET: &[u8; 85] =
    b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ.-:+=^!/*?&<>()[]{}@%$#";
const RFC1924_ALPHABET: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";

impl Variant {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "ascii85" | "adobe" | "btoa" => Ok(Variant::Ascii85),
            "z85" | "zeromq" => Ok(Variant::Z85),
            "rfc1924" | "rfc-1924" | "ipv6" => Ok(Variant::Rfc1924),
            other => Err(format!(
                "invalid variant {other:?}: expected \"ascii85\", \"z85\", or \"rfc1924\""
            )),
        }
    }

    /// Map a base-85 digit value (0..85) to its output symbol for this variant.
    fn symbol(self, value: u8) -> u8 {
        match self {
            Variant::Ascii85 => 33 + value, // '!' + value  =>  '!'..'u'
            Variant::Z85 => Z85_ALPHABET[value as usize],
            Variant::Rfc1924 => RFC1924_ALPHABET[value as usize],
        }
    }

    /// Reverse lookup: ASCII byte -> base-85 digit value, or 255 if not in the
    /// alphabet. A 128-entry table covers every printable base85 symbol.
    fn lookup(self) -> [u8; 128] {
        let mut table = [255u8; 128];
        match self {
            Variant::Ascii85 => {
                for v in 0u8..85 {
                    table[(33 + v) as usize] = v;
                }
            }
            Variant::Z85 => {
                for (v, &c) in Z85_ALPHABET.iter().enumerate() {
                    table[c as usize] = v as u8;
                }
            }
            Variant::Rfc1924 => {
                for (v, &c) in RFC1924_ALPHABET.iter().enumerate() {
                    table[c as usize] = v as u8;
                }
            }
        }
        table
    }

    /// The `z` all-zero-group shortcut is an Ascii85-only feature.
    fn zero_shortcut(self) -> bool {
        matches!(self, Variant::Ascii85)
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

/// Encode raw bytes to a Base85 string with the standard algorithm: process
/// four big-endian bytes at a time into five base-85 digits. A partial final
/// group of `r` bytes (1..=3) emits `r + 1` symbols. Ascii85 collapses an
/// all-zero complete group to `z`.
fn encode_bytes(bytes: &[u8], variant: Variant) -> String {
    let mut out = Vec::with_capacity(bytes.len() * 5 / 4 + 5);
    let mut i = 0;
    while i < bytes.len() {
        let group_len = (bytes.len() - i).min(4);
        // Assemble the 4-byte big-endian value, padding a partial group with 0.
        let mut n: u32 = 0;
        for j in 0..4 {
            let b = if j < group_len { bytes[i + j] } else { 0 };
            n = (n << 8) | b as u32;
        }
        if group_len == 4 && variant.zero_shortcut() && n == 0 {
            out.push(b'z');
        } else {
            // Five base-85 digits, most-significant first.
            let mut digits = [0u8; 5];
            let mut m = n;
            for k in (0..5).rev() {
                digits[k] = (m % 85) as u8;
                m /= 85;
            }
            for &d in digits.iter().take(group_len + 1) {
                out.push(variant.symbol(d));
            }
        }
        i += 4;
    }
    // Every symbol is printable ASCII, so this is always valid UTF-8.
    String::from_utf8(out).expect("base85 alphabets are ASCII")
}

/// Decode a Base85 string back to raw bytes. Whitespace is ignored. Ascii85
/// auto-strips `<~ … ~>` framing and expands each `z` to four zero bytes. A
/// partial final group of `r` symbols (2..=4) is padded with the maximum digit
/// and yields `r - 1` bytes; a lone trailing symbol is an error.
fn decode_str(s: &str, variant: Variant) -> Result<Vec<u8>, String> {
    let lookup = variant.lookup();
    let mut trimmed = s.trim();
    // Ascii85 (Adobe) framing: <~ … ~>. Strip either delimiter if present.
    if variant == Variant::Ascii85 {
        trimmed = trimmed.strip_prefix("<~").unwrap_or(trimmed);
        trimmed = trimmed.strip_suffix("~>").unwrap_or(trimmed);
    }

    let mut out: Vec<u8> = Vec::with_capacity(trimmed.len());
    let mut group: [u8; 5] = [0; 5];
    let mut count = 0usize;

    for c in trimmed.chars() {
        if c.is_whitespace() {
            continue;
        }
        if variant.zero_shortcut() && c == 'z' {
            if count != 0 {
                return Err("'z' zero shortcut must stand alone, not inside a group".into());
            }
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        let value = if (c as u32) < 128 {
            lookup[c as usize]
        } else {
            255
        };
        if value == 255 {
            return Err(format!(
                "invalid Base85 character {c:?}: not in the selected variant's alphabet"
            ));
        }
        group[count] = value;
        count += 1;
        if count == 5 {
            push_group(&mut out, &group, 5)?;
            count = 0;
        }
    }

    match count {
        0 => {}
        1 => {
            return Err(
                "a single trailing Base85 character does not form a valid group".into(),
            )
        }
        r => {
            // Pad the partial group with the maximum digit (84) and keep r-1 bytes.
            for slot in group.iter_mut().take(5).skip(r) {
                *slot = 84;
            }
            push_group(&mut out, &group, r)?;
        }
    }
    Ok(out)
}

/// Convert a full 5-digit base-85 group to 4 big-endian bytes, keeping only the
/// first `keep_len - 1` of them (a full group keeps all 4). Rejects a group
/// whose value overflows 32 bits (an impossible Base85 sequence).
fn push_group(out: &mut Vec<u8>, group: &[u8; 5], keep_len: usize) -> Result<(), String> {
    let mut n: u64 = 0;
    for &d in group.iter() {
        n = n * 85 + d as u64;
    }
    if n > u32::MAX as u64 {
        return Err("invalid Base85 group: value exceeds 32 bits".into());
    }
    let bytes = (n as u32).to_be_bytes();
    let keep = if keep_len == 5 { 4 } else { keep_len - 1 };
    out.extend_from_slice(&bytes[..keep]);
    Ok(())
}

/// Encode `input` to Base85 or decode a Base85 string back.
///
/// - `mode` (`"encode"` | `"decode"`, blank → `"encode"`): direction.
/// - `variant` (`"ascii85"` | `"z85"` | `"rfc1924"`, blank → `"ascii85"`): which
///   Base85 flavour to use. `ascii85` is the Adobe/btoa encoding (`z` shortcut +
///   optional framing); `z85` is ZeroMQ's (input length must be a multiple of
///   4); `rfc1924` is the RFC 1924 alphabet with the standard grouping.
/// - `format` (`"text"` | `"hex"`, blank → `"text"`): how to read `input` when
///   encoding, or render the result when decoding. `text` is UTF-8; `hex` is a
///   hex byte string.
/// - `adobe_frame` (`bool`, default `false`): Ascii85 only — wrap encoded output
///   in Adobe `<~ … ~>` delimiters. Ignored for other variants; decoding always
///   strips framing if present.
///
/// Returns `Err` on an invalid `mode`/`variant`/`format`, malformed hex, a Z85
/// input whose length isn't a multiple of 4 (encode) or 5 (decode), an
/// undecodable Base85 string, or decoded bytes that aren't valid UTF-8 (when
/// `format = "text"`).
pub fn convert(
    input: &str,
    mode: &str,
    variant: &str,
    format: &str,
    adobe_frame: bool,
) -> Result<String, String> {
    let variant = Variant::parse(variant)?;
    let fmt = DataFormat::parse(format)?;

    match mode {
        "" | "encode" => {
            let bytes = match fmt {
                DataFormat::Text => input.as_bytes().to_vec(),
                DataFormat::Hex => parse_hex(input)?,
            };
            if variant == Variant::Z85 && bytes.len() % 4 != 0 {
                return Err(format!(
                    "Z85 input must be a multiple of 4 bytes; got {} bytes. Pad the data or choose the ascii85/rfc1924 variant, which allow any length.",
                    bytes.len()
                ));
            }
            let encoded = encode_bytes(&bytes, variant);
            if adobe_frame && variant == Variant::Ascii85 {
                Ok(format!("<~{encoded}~>"))
            } else {
                Ok(encoded)
            }
        }
        "decode" => {
            if variant == Variant::Z85 {
                // Count only the alphabet symbols (whitespace ignored) for the
                // multiple-of-5 check.
                let symbols = input.chars().filter(|c| !c.is_whitespace()).count();
                if symbols % 5 != 0 {
                    return Err(format!(
                        "Z85 input must be a multiple of 5 characters; got {symbols}. Check for missing characters, or choose the ascii85 variant."
                    ));
                }
            }
            let bytes = decode_str(input, variant)?;
            match fmt {
                DataFormat::Text => String::from_utf8(bytes).map_err(|_| {
                    "decoded bytes are not valid UTF-8 — set format 'hex' to view the raw bytes".into()
                }),
                DataFormat::Hex => Ok(to_hex(&bytes)),
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

    // Convenience wrappers for the common (no-framing) call.
    fn enc(input: &str, variant: &str, format: &str) -> String {
        convert(input, "encode", variant, format, false).unwrap()
    }
    fn dec(input: &str, variant: &str, format: &str) -> String {
        convert(input, "decode", variant, format, false).unwrap()
    }

    #[test]
    fn ascii85_text_vectors() {
        // Cross-checked against Python base64.a85encode (z-shortcut on).
        let cases = [
            ("", ""),
            ("Man ", "9jqo^"),
            ("Hello World!", "87cURD]i,\"Ebo80"),
            ("sure.", "F*2M7/c"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(enc(plain, "ascii85", "text"), encoded, "encode {plain:?}");
            assert_eq!(dec(encoded, "ascii85", "text"), plain, "decode {encoded:?}");
        }
    }

    #[test]
    fn ascii85_z_shortcut() {
        // A complete all-zero group collapses to a single 'z'.
        assert_eq!(enc("00000000", "ascii85", "hex"), "z");
        assert_eq!(dec("z", "ascii85", "hex"), "00000000");
        // z followed by a real group (cross-checked with Python: b"\0\0\0\0abcd").
        let enc4 = enc("0000000061626364", "ascii85", "hex");
        assert_eq!(enc4, "z@:E_W");
        assert_eq!(dec(&enc4, "ascii85", "hex"), "0000000061626364");
    }

    #[test]
    fn ascii85_adobe_framing_roundtrips() {
        let framed = convert("Hello World!", "encode", "ascii85", "text", true).unwrap();
        assert_eq!(framed, "<~87cURD]i,\"Ebo80~>");
        // Decoding strips the framing regardless of the flag.
        assert_eq!(dec(&framed, "ascii85", "text"), "Hello World!");
    }

    #[test]
    fn rfc1924_vectors() {
        // Cross-checked against Python base64.b85encode.
        let cases = [
            ("", ""),
            ("Man ", "O<`^z"),
            ("Hello World!", "NM&qnZy;B1a%^NF"),
            ("abcd", "VPa!s"),
            ("abcde", "VPa!sWd"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(enc(plain, "rfc1924", "text"), encoded, "encode {plain:?}");
            assert_eq!(dec(encoded, "rfc1924", "text"), plain, "decode {encoded:?}");
        }
        // RFC 1924 has no zero shortcut: 4 zero bytes -> "00000".
        assert_eq!(enc("00000000", "rfc1924", "hex"), "00000");
        assert_eq!(dec("00000", "rfc1924", "hex"), "00000000");
    }

    #[test]
    fn z85_rfc32_test_vector() {
        // The canonical Z85 vector from ZeroMQ RFC 32.
        let hello = "864fd26fb559f75b";
        assert_eq!(enc(hello, "z85", "hex"), "HelloWorld");
        assert_eq!(dec("HelloWorld", "z85", "hex"), hello);
        // Text of length 4 round-trips.
        assert_eq!(enc("Man ", "z85", "text"), "o<}]Z");
        assert_eq!(dec("o<}]Z", "z85", "text"), "Man ");
    }

    #[test]
    fn z85_requires_multiple_of_four() {
        let err = convert("abc", "encode", "z85", "text", false).unwrap_err();
        assert!(err.contains("multiple of 4"), "got: {err}");
        let err = convert("abcdef", "decode", "z85", "text", false).unwrap_err();
        assert!(err.contains("multiple of 5"), "got: {err}");
    }

    #[test]
    fn partial_groups_ascii85() {
        // 1..=3 trailing bytes emit 2..=4 symbols and round-trip.
        for s in ["a", "ab", "abc", "abcd", "abcde", "abcdef"] {
            let e = enc(s, "ascii85", "text");
            assert_eq!(dec(&e, "ascii85", "text"), s, "roundtrip {s:?} -> {e:?}");
        }
    }

    #[test]
    fn defaults_blank_args_to_encode_ascii85_text() {
        assert_eq!(
            convert("Hello World!", "", "", "", false).unwrap(),
            "87cURD]i,\"Ebo80"
        );
    }

    #[test]
    fn hex_input_tolerates_whitespace_and_prefix() {
        // "Man " = 4d 61 6e 20
        assert_eq!(enc("0x 4d 61 6e 20", "ascii85", "hex"), "9jqo^");
    }

    #[test]
    fn decode_ignores_internal_whitespace() {
        assert_eq!(dec("87cURD]i,\"Ebo80", "ascii85", "text"), "Hello World!");
        assert_eq!(dec("87cURD]i,\n\"Ebo80", "ascii85", "text"), "Hello World!");
    }

    #[test]
    fn decode_non_utf8_to_text_errors_but_hex_works() {
        let e = enc("fffefd", "ascii85", "hex");
        let err = convert(&e, "decode", "ascii85", "text", false).unwrap_err();
        assert!(err.contains("UTF-8"), "got: {err}");
        assert_eq!(dec(&e, "ascii85", "hex"), "fffefd");
    }

    #[test]
    fn rejects_unknown_mode_variant_format() {
        assert!(convert("x", "zap", "ascii85", "text", false)
            .unwrap_err()
            .contains("invalid mode"));
        assert!(convert("x", "encode", "base999", "text", false)
            .unwrap_err()
            .contains("invalid variant"));
        assert!(convert("x", "encode", "ascii85", "binary", false)
            .unwrap_err()
            .contains("invalid data format"));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(convert("zz", "encode", "ascii85", "hex", false)
            .unwrap_err()
            .contains("invalid hex"));
        assert!(convert("abc", "encode", "ascii85", "hex", false)
            .unwrap_err()
            .contains("odd number"));
        // A character outside the variant alphabet on decode.
        let err = convert("~~~~~", "decode", "z85", "text", false).unwrap_err();
        assert!(err.contains("invalid Base85 character"), "got: {err}");
        // A lone trailing symbol is not a valid group.
        let err = convert("87cURD]i,\"Ebo80X", "decode", "ascii85", "hex", false).unwrap_err();
        assert!(err.contains("single trailing"), "got: {err}");
    }
}
