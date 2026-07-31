//! checksum-calculator core — compute CRC-family checksums (CRC-32, CRC-32C,
//! CRC-16, CRC-8) of an input and optionally verify them against an expected
//! value. Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps.
//!
//! The input text is interpreted as UTF-8 (default) or decoded first from
//! hex / base64 (so raw file bytes can be checksummed), and the checksum is
//! rendered as hexadecimal (default) or decimal. Each CRC uses its standard
//! parameters:
//!
//! - **CRC-32** — CRC-32/ISO-HDLC (poly 0x04C11DB7, reflected), the variant used
//!   by zip, gzip, PNG and Ethernet. check("123456789") = 0xCBF43926.
//! - **CRC-32C** — CRC-32/Castagnoli (poly 0x1EDC6F41, reflected), used by iSCSI,
//!   ext4, and SSE4.2 hardware. check("123456789") = 0xE3069283.
//! - **CRC-16** — CRC-16/ARC (poly 0x8005, reflected, init 0x0000), the classic
//!   "CRC-16" / "CRC-16/IBM". check("123456789") = 0xBB3D.
//! - **CRC-8** — CRC-8/SMBUS (poly 0x07, init 0x00, no reflection), the plain
//!   "CRC-8". check("123456789") = 0xF4.
//!
//! No dependency on a CRC crate: the four algorithms are computed bit-by-bit
//! from their canonical parameters, so the whole tool is a few hundred bytes of
//! pure Rust that instantiates on every backend including the chat Service
//! Worker.

use base64::Engine;

/// Which CRC algorithm to apply.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Algorithm {
    /// CRC-32/ISO-HDLC (zip, gzip, PNG, Ethernet).
    Crc32,
    /// CRC-32C / Castagnoli (iSCSI, ext4, SSE4.2).
    Crc32c,
    /// CRC-16/ARC — the classic "CRC-16".
    Crc16,
    /// CRC-8/SMBUS — the plain "CRC-8".
    Crc8,
}

impl Algorithm {
    /// Human-facing label used to prefix the result line.
    pub fn label(self) -> &'static str {
        match self {
            Algorithm::Crc32 => "CRC-32",
            Algorithm::Crc32c => "CRC-32C",
            Algorithm::Crc16 => "CRC-16",
            Algorithm::Crc8 => "CRC-8",
        }
    }

    /// Width of the checksum in bits.
    fn width_bits(self) -> u32 {
        match self {
            Algorithm::Crc32 | Algorithm::Crc32c => 32,
            Algorithm::Crc16 => 16,
            Algorithm::Crc8 => 8,
        }
    }

    /// Compute the checksum value (within `width_bits`) over `data`.
    fn compute(self, data: &[u8]) -> u64 {
        match self {
            // CRC-32/ISO-HDLC: reflected poly 0xEDB88320, init/xorout 0xFFFFFFFF.
            Algorithm::Crc32 => crc_reflected(data, 0xEDB8_8320, 0xFFFF_FFFF, 0xFFFF_FFFF, 32),
            // CRC-32C/Castagnoli: reflected poly 0x82F63B78, init/xorout 0xFFFFFFFF.
            Algorithm::Crc32c => crc_reflected(data, 0x82F6_3B78, 0xFFFF_FFFF, 0xFFFF_FFFF, 32),
            // CRC-16/ARC: reflected poly 0xA001, init 0, xorout 0.
            Algorithm::Crc16 => crc_reflected(data, 0xA001, 0x0000, 0x0000, 16),
            // CRC-8/SMBUS: non-reflected poly 0x07, init 0, xorout 0.
            Algorithm::Crc8 => crc8(data, 0x07, 0x00, 0x00),
        }
    }
}

/// Reflected (input- and output-reflected) CRC. `poly_rev` is the polynomial in
/// its bit-reversed form, `init`/`xorout` are given in the reflected domain.
fn crc_reflected(data: &[u8], poly_rev: u64, init: u64, xorout: u64, width: u32) -> u64 {
    let mask: u64 = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
    let mut crc = init & mask;
    for &byte in data {
        crc ^= byte as u64;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ poly_rev;
            } else {
                crc >>= 1;
            }
        }
    }
    (crc ^ xorout) & mask
}

/// Non-reflected 8-bit CRC (MSB-first).
fn crc8(data: &[u8], poly: u8, init: u8, xorout: u8) -> u64 {
    let mut crc = init;
    for &byte in data {
        crc ^= byte;
        for _ in 0..8 {
            if crc & 0x80 != 0 {
                crc = (crc << 1) ^ poly;
            } else {
                crc <<= 1;
            }
        }
    }
    (crc ^ xorout) as u64
}

/// How to interpret the input string before checksumming.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputEncoding {
    /// Checksum the UTF-8 bytes of the text as-is (default).
    Text,
    /// Decode the text from hexadecimal first, then checksum the raw bytes.
    Hex,
    /// Decode the text from standard base64 first, then checksum the bytes.
    Base64,
}

/// How to render the checksum value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    /// Zero-padded hexadecimal (default).
    Hex,
    /// Unsigned decimal integer.
    Decimal,
}

fn parse_algorithm(s: &str) -> Result<Algorithm, String> {
    match s.trim().to_ascii_lowercase().replace('-', "").as_str() {
        "" | "crc32" => Ok(Algorithm::Crc32),
        "crc32c" => Ok(Algorithm::Crc32c),
        "crc16" => Ok(Algorithm::Crc16),
        "crc8" => Ok(Algorithm::Crc8),
        other => Err(format!(
            "invalid algorithm '{other}': expected 'crc32', 'crc32c', 'crc16', or 'crc8'"
        )),
    }
}

fn parse_input_encoding(s: &str) -> Result<InputEncoding, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "text" | "utf8" | "utf-8" => Ok(InputEncoding::Text),
        "hex" => Ok(InputEncoding::Hex),
        "base64" | "b64" => Ok(InputEncoding::Base64),
        other => Err(format!(
            "invalid input_encoding '{other}': expected 'text', 'hex', or 'base64'"
        )),
    }
}

fn parse_output_format(s: &str) -> Result<OutputFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "hex" => Ok(OutputFormat::Hex),
        "decimal" | "dec" => Ok(OutputFormat::Decimal),
        other => Err(format!(
            "invalid output_format '{other}': expected 'hex' or 'decimal'"
        )),
    }
}

/// Decode `text` into raw bytes per `encoding`. Tolerates a leading `0x` and
/// internal whitespace for hex, and whitespace for base64 — matching the rest
/// of the hash/checksum family.
fn decode_input(text: &str, encoding: InputEncoding) -> Result<Vec<u8>, String> {
    match encoding {
        InputEncoding::Text => Ok(text.as_bytes().to_vec()),
        InputEncoding::Hex => {
            let cleaned: String = text.split_whitespace().collect();
            let cleaned = cleaned
                .strip_prefix("0x")
                .or_else(|| cleaned.strip_prefix("0X"))
                .unwrap_or(&cleaned);
            hex::decode(cleaned).map_err(|e| format!("input is not valid hex: {e}"))
        }
        InputEncoding::Base64 => {
            let cleaned: String = text.split_whitespace().collect();
            base64::engine::general_purpose::STANDARD
                .decode(cleaned.as_bytes())
                .map_err(|e| format!("input is not valid base64: {e}"))
        }
    }
}

/// Render `value` (a `width_bits`-wide checksum) per `fmt`.
fn render(value: u64, width_bits: u32, fmt: OutputFormat, uppercase: bool) -> String {
    match fmt {
        OutputFormat::Hex => {
            let digits = (width_bits / 4) as usize;
            let s = format!("{value:0digits$x}");
            if uppercase {
                s.to_uppercase()
            } else {
                s
            }
        }
        OutputFormat::Decimal => value.to_string(),
    }
}

/// Does `expected` describe the same checksum `value` (width `width_bits`)?
/// Accepts hex (with or without a leading `0x`, any case, with or without
/// leading zeros) or a plain decimal integer, ignoring surrounding whitespace.
fn expected_matches(expected: &str, value: u64, width_bits: u32) -> bool {
    let cleaned: String = expected.split_whitespace().collect();
    if cleaned.is_empty() {
        return false;
    }
    let hex_body = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(&cleaned);
    // Interpret as hex numerically (handles leading zeros / case differences).
    if let Ok(n) = u64::from_str_radix(hex_body, 16) {
        if n == value {
            return true;
        }
    }
    // Also accept a plain decimal form (for decimal-output users).
    if let Ok(n) = cleaned.parse::<u64>() {
        if n == value {
            return true;
        }
    }
    // Exact zero-padded hex string match (belt-and-braces; case-insensitive).
    let padded = render(value, width_bits, OutputFormat::Hex, false);
    hex_body.eq_ignore_ascii_case(&padded)
}

/// Compute the `algorithm` checksum of `text` (interpreted per `input_encoding`),
/// render it per `output_format`/`uppercase`, and — when `expected` is non-empty
/// — append a verification verdict (MATCH / MISMATCH).
///
/// Defaults (blank strings / false): algorithm=crc32, input_encoding=text,
/// output_format=hex, lowercase, no verification.
pub fn checksum(
    text: &str,
    algorithm: &str,
    input_encoding: &str,
    output_format: &str,
    uppercase: bool,
    expected: &str,
) -> Result<String, String> {
    let algo = parse_algorithm(algorithm)?;
    let enc = parse_input_encoding(input_encoding)?;
    let fmt = parse_output_format(output_format)?;

    let bytes = decode_input(text, enc)?;
    let value = algo.compute(&bytes);
    let rendered = render(value, algo.width_bits(), fmt, uppercase);

    let mut out = format!("{}: {rendered}", algo.label());

    if !expected.trim().is_empty() {
        let matched = expected_matches(expected, value, algo.width_bits());
        out.push_str(&format!("\nExpected: {}", expected.trim()));
        out.push_str(if matched {
            "\nResult: MATCH"
        } else {
            "\nResult: MISMATCH"
        });
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The canonical "check" value of each CRC is its checksum of the ASCII
    // string "123456789" — published for every CRC variant.
    #[test]
    fn check_vectors() {
        let c = |a: Algorithm| {
            let v = a.compute(b"123456789");
            render(v, a.width_bits(), OutputFormat::Hex, false)
        };
        assert_eq!(c(Algorithm::Crc32), "cbf43926");
        assert_eq!(c(Algorithm::Crc32c), "e3069283");
        assert_eq!(c(Algorithm::Crc16), "bb3d");
        assert_eq!(c(Algorithm::Crc8), "f4");
    }

    #[test]
    fn empty_input_check_vectors() {
        // CRC of the empty message equals init ^ xorout: 0 for every variant
        // here (reflected pair has init == xorout; CRC-8 has init 0).
        assert_eq!(Algorithm::Crc32.compute(b""), 0);
        assert_eq!(Algorithm::Crc32c.compute(b""), 0);
        assert_eq!(Algorithm::Crc16.compute(b""), 0);
        assert_eq!(Algorithm::Crc8.compute(b""), 0);
    }

    #[test]
    fn happy_default_crc32() {
        let out = checksum("123456789", "", "", "", false, "").unwrap();
        assert_eq!(out, "CRC-32: cbf43926");
    }

    #[test]
    fn uppercase_and_decimal() {
        let up = checksum("123456789", "crc32", "text", "hex", true, "").unwrap();
        assert_eq!(up, "CRC-32: CBF43926");
        let dec = checksum("123456789", "crc32", "text", "decimal", false, "").unwrap();
        assert_eq!(dec, "CRC-32: 3421780262");
    }

    #[test]
    fn hex_input_matches_text() {
        // "abc" == hex "616263"; CRC-32 of both must agree.
        let a = checksum("abc", "crc32", "text", "hex", false, "").unwrap();
        let b = checksum("616263", "crc32", "hex", "hex", false, "").unwrap();
        assert_eq!(a, b);
        assert_eq!(a, "CRC-32: 352441c2");
    }

    #[test]
    fn base64_input_matches_text() {
        // base64("abc") == "YWJj"; the CRC-16 of the decoded bytes must equal
        // the CRC-16 of the plain text.
        let via_b64 = checksum("YWJj", "crc16", "base64", "hex", false, "").unwrap();
        let via_text = checksum("abc", "crc16", "text", "hex", false, "").unwrap();
        assert_eq!(via_b64, via_text);
    }

    #[test]
    fn verify_match_forms() {
        // hex, 0x-prefixed, uppercase, and decimal all verify as a MATCH.
        for exp in ["cbf43926", "0xCBF43926", "CBF43926", "3421780262"] {
            let out = checksum("123456789", "crc32", "text", "hex", false, exp).unwrap();
            assert!(out.ends_with("Result: MATCH"), "expected MATCH for {exp}: {out}");
        }
    }

    #[test]
    fn verify_mismatch() {
        let out = checksum("123456789", "crc32", "text", "hex", false, "deadbeef").unwrap();
        assert!(out.contains("Expected: deadbeef"));
        assert!(out.ends_with("Result: MISMATCH"));
    }

    #[test]
    fn err_bad_algorithm() {
        let err = checksum("x", "crc7", "", "", false, "").unwrap_err();
        assert!(err.contains("invalid algorithm"), "{err}");
    }

    #[test]
    fn err_bad_hex_input() {
        let err = checksum("zz", "crc32", "hex", "", false, "").unwrap_err();
        assert!(err.contains("not valid hex"), "{err}");
    }

    #[test]
    fn err_bad_output_format() {
        let err = checksum("x", "crc32", "", "octal", false, "").unwrap_err();
        assert!(err.contains("invalid output_format"), "{err}");
    }
}
