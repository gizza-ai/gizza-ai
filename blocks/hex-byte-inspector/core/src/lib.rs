//! hex-byte-inspector core — pure, dependency-free byte inspector shared by the
//! chat skill block and the web page.
//!
//! Given a value expressed as hex, base64 or text, it reports the value's byte
//! length, bit length and hex-char count; shows the same bytes converted between
//! hex, base64 and (printable) text; groups the hex for readability; and — when
//! asked — notes which common cryptographic value sizes match that byte length
//! (hash digests, AES/ChaCha keys, Ed25519/secp256k1 keys & signatures, IVs).
//! No I/O, no deps beyond std. Deterministic.

/// How the caller's `input` string should be read into bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InputFormat {
    /// A hexadecimal string (whitespace, `:` `-` `,` `_` and `0x` / `\x`
    /// per-byte prefixes are ignored).
    Hex,
    /// A standard or URL-safe Base64 string (padding optional).
    Base64,
    /// Literal UTF-8 text — its bytes are inspected directly.
    Text,
}

impl InputFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "hex" => Ok(InputFormat::Hex),
            "base64" | "b64" => Ok(InputFormat::Base64),
            "text" | "utf8" | "utf-8" => Ok(InputFormat::Text),
            other => Err(format!(
                "invalid input_format {other:?}: expected \"hex\", \"base64\", or \"text\""
            )),
        }
    }
}

/// Decode the caller's `input` into the raw bytes it represents.
fn decode(input: &str, fmt: InputFormat) -> Result<Vec<u8>, String> {
    match fmt {
        InputFormat::Text => Ok(input.as_bytes().to_vec()),
        InputFormat::Hex => decode_hex(input),
        InputFormat::Base64 => decode_base64(input),
    }
}

/// Tolerant hex decode: strips ASCII whitespace, the `: - , _` delimiters and the
/// `0x` / `\x` per-byte prefixes, then requires an even number of hex digits.
fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let mut cleaned = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b' ' | b'\t' | b'\n' | b'\r' | b':' | b'-' | b',' | b'_' => {
                i += 1;
                continue;
            }
            b'0' if i + 1 < bytes.len() && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') => {
                // `0x` prefix — skip both characters.
                i += 2;
                continue;
            }
            b'\\' if i + 1 < bytes.len() && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') => {
                // `\x` prefix — skip both characters.
                i += 2;
                continue;
            }
            _ => {}
        }
        let ch = c as char;
        if ch.is_ascii_hexdigit() {
            cleaned.push(ch);
            i += 1;
        } else {
            return Err(format!(
                "invalid hex: unexpected character {ch:?} — expected 0-9, a-f, A-F, or a delimiter"
            ));
        }
    }
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "invalid hex: found {} hex digit(s) after cleanup — a byte needs 2, so the count must be even",
            cleaned.len()
        ));
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let cb = cleaned.as_bytes();
    let mut j = 0;
    while j < cb.len() {
        let hi = (cb[j] as char).to_digit(16).unwrap() as u8;
        let lo = (cb[j + 1] as char).to_digit(16).unwrap() as u8;
        out.push((hi << 4) | lo);
        j += 2;
    }
    Ok(out)
}

/// Decode standard or URL-safe Base64, with or without `=` padding. ASCII
/// whitespace is ignored.
fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }
    let mut symbols = Vec::new();
    for &c in input.as_bytes() {
        match c {
            b' ' | b'\t' | b'\n' | b'\r' | b'=' => continue,
            _ => match val(c) {
                Some(v) => symbols.push(v),
                None => {
                    return Err(format!(
                        "invalid base64: unexpected character {:?}",
                        c as char
                    ))
                }
            },
        }
    }
    if symbols.len() % 4 == 1 {
        return Err("invalid base64: length is not a valid base64 sequence".into());
    }
    let mut out = Vec::with_capacity(symbols.len() / 4 * 3);
    for chunk in symbols.chunks(4) {
        let mut buf = 0u32;
        for (k, &s) in chunk.iter().enumerate() {
            buf |= (s as u32) << (18 - 6 * k);
        }
        let n = chunk.len();
        out.push((buf >> 16) as u8);
        if n >= 3 {
            out.push((buf >> 8) as u8);
        }
        if n >= 4 {
            out.push(buf as u8);
        }
    }
    Ok(out)
}

/// Standard Base64 encode with `=` padding.
fn encode_base64(data: &[u8]) -> String {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHA[(n >> 18 & 63) as usize] as char);
        out.push(ALPHA[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() >= 2 {
            ALPHA[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() >= 3 {
            ALPHA[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Lowercase/uppercase continuous hex, then grouped `group_size` bytes per space-
/// separated group (0 = no grouping / continuous).
fn hex_string(data: &[u8], uppercase: bool, group_size: usize) -> String {
    let mut flat = String::with_capacity(data.len() * 2);
    for &b in data {
        if uppercase {
            flat.push_str(&format!("{b:02X}"));
        } else {
            flat.push_str(&format!("{b:02x}"));
        }
    }
    if group_size == 0 || data.len() <= group_size {
        return flat;
    }
    // Regroup the flat hex into `group_size`-byte (2*group_size-char) chunks.
    let per = group_size * 2;
    let chars: Vec<char> = flat.chars().collect();
    let mut out = String::with_capacity(flat.len() + flat.len() / per);
    for (i, ch) in chars.iter().enumerate() {
        if i != 0 && i % per == 0 {
            out.push(' ');
        }
        out.push(*ch);
    }
    out
}

/// A printable text rendering of the bytes, or a note if they aren't printable.
fn text_repr(data: &[u8]) -> String {
    match std::str::from_utf8(data) {
        Ok(s) if !s.chars().any(|c| c.is_control() && c != '\t' && c != '\n' && c != '\r') => {
            format!("{s:?}")
        }
        _ => format!("(not printable as text — {} raw bytes)", data.len()),
    }
}

/// Common cryptographic values whose canonical size is `n` bytes, most-common
/// first. Empty when nothing standard matches that exact length.
fn crypto_matches(n: usize) -> &'static [&'static str] {
    match n {
        8 => &["DES key (56-bit effective) or 64-bit block", "CRC-64 / 64-bit token"],
        12 => &["AES-GCM / ChaCha20-Poly1305 nonce (IV)"],
        16 => &[
            "MD5 or MD4 digest",
            "AES-128 key",
            "UUID / GUID (128-bit)",
            "AES-CBC / CTR block or IV",
        ],
        20 => &["SHA-1 or RIPEMD-160 digest", "HMAC-SHA-1 output"],
        24 => &["AES-192 key", "Triple-DES (3-key) key"],
        28 => &["SHA-224 or SHA3-224 digest"],
        32 => &[
            "SHA-256 / SHA3-256 / BLAKE2s digest",
            "AES-256 or ChaCha20 key",
            "Ed25519 public or private key",
            "secp256k1 or NIST P-256 private key",
        ],
        33 => &["Compressed secp256k1 / P-256 public key (0x02 / 0x03 prefix)"],
        48 => &["SHA-384 or SHA3-384 digest", "NIST P-384 private key"],
        64 => &[
            "SHA-512 / SHA3-512 / BLAKE2b digest",
            "Ed25519 signature",
            "HMAC-SHA-512 output",
        ],
        65 => &["Uncompressed secp256k1 / P-256 public key (0x04 prefix)"],
        _ => &[],
    }
}

/// Inspect `input` (read per `input_format`) and return a human-readable report.
///
/// - `group_size`: bytes per space-separated group in the hex view (0 = continuous),
///   clamped to 0..=64.
/// - `uppercase`: uppercase A–F hex digits in the output.
/// - `interpret`: when true, append the "Matches" block of common crypto value
///   sizes for the byte length.
pub fn inspect(
    input: &str,
    input_format: &str,
    group_size: i64,
    uppercase: bool,
    interpret: bool,
) -> Result<String, String> {
    let fmt = InputFormat::parse(input_format)?;
    let data = decode(input, fmt)?;
    let group = group_size.clamp(0, 64) as usize;

    let n = data.len();
    let bits = n * 8;
    let hex_chars = n * 2;

    let mut out = String::new();
    out.push_str(&format!("Bytes:     {n}\n"));
    out.push_str(&format!("Bits:      {bits}\n"));
    out.push_str(&format!("Hex chars: {hex_chars}\n"));
    out.push('\n');
    out.push_str(&format!("Hex:    {}\n", hex_string(&data, uppercase, group)));
    out.push_str(&format!("Base64: {}\n", encode_base64(&data)));
    out.push_str(&format!("Text:   {}\n", text_repr(&data)));

    if interpret {
        out.push('\n');
        let matches = crypto_matches(n);
        if matches.is_empty() {
            out.push_str(&format!(
                "Matches: no common cryptographic value has a {n}-byte ({bits}-bit) size.\n"
            ));
        } else {
            out.push_str(&format!("Matches (common {n}-byte / {bits}-bit values):\n"));
            for m in matches {
                out.push_str(&format!("  - {m}\n"));
            }
        }
    }

    // Trim the trailing newline so surfaces compare cleanly.
    while out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_sha256_hex() {
        // 32 bytes of 0xAB → 256-bit, flagged as a SHA-256 / AES-256 size.
        let hex = "ab".repeat(32);
        let r = inspect(&hex, "hex", 4, false, true).unwrap();
        assert!(r.contains("Bytes:     32"));
        assert!(r.contains("Bits:      256"));
        assert!(r.contains("Hex chars: 64"));
        // grouped 4 bytes/8 chars per group
        assert!(r.contains("Hex:    abababab abababab"));
        assert!(r.contains("SHA-256 / SHA3-256 / BLAKE2s digest"));
        assert!(r.contains("AES-256 or ChaCha20 key"));
    }

    #[test]
    fn tolerant_hex_paste_and_text_roundtrip() {
        // "Hi" with mixed delimiters/prefixes decodes to 2 bytes and prints as text.
        let r = inspect("0x48 0x69", "hex", 0, false, false).unwrap();
        assert!(r.contains("Bytes:     2"));
        assert!(r.contains("Hex:    4869"));
        assert!(r.contains(r#"Text:   "Hi""#));
        // no Matches block when interpret=false
        assert!(!r.contains("Matches"));
    }

    #[test]
    fn text_input_utf8_byte_length() {
        // 'é' is 2 UTF-8 bytes (c3 a9).
        let r = inspect("é", "text", 4, true, false).unwrap();
        assert!(r.contains("Bytes:     2"));
        assert!(r.contains("Hex:    C3A9"));
        assert!(r.contains("Base64: w6k="));
    }

    #[test]
    fn base64_input_decodes() {
        // "SGk=" is base64 for "Hi".
        let r = inspect("SGk=", "base64", 0, false, false).unwrap();
        assert!(r.contains("Bytes:     2"));
        assert!(r.contains(r#"Text:   "Hi""#));
    }

    #[test]
    fn unusual_length_reports_no_match() {
        let r = inspect("aabbcc", "hex", 4, false, true).unwrap();
        assert!(r.contains("Bytes:     3"));
        assert!(r.contains("no common cryptographic value has a 3-byte"));
    }

    #[test]
    fn odd_hex_errors() {
        let e = inspect("abc", "hex", 4, false, false).unwrap_err();
        assert!(e.contains("even"), "got: {e}");
    }

    #[test]
    fn bad_hex_char_errors() {
        let e = inspect("zz", "hex", 4, false, false).unwrap_err();
        assert!(e.contains("invalid hex"), "got: {e}");
    }

    #[test]
    fn bad_format_errors() {
        let e = inspect("aa", "octal", 4, false, false).unwrap_err();
        assert!(e.contains("invalid input_format"), "got: {e}");
    }

    #[test]
    fn non_printable_bytes_noted() {
        let r = inspect("00ff", "hex", 0, false, false).unwrap();
        assert!(r.contains("not printable as text"));
    }
}
