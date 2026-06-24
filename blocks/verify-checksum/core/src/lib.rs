//! verify-checksum core — verify that input data matches an expected checksum.
//!
//! Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps. The input is hashed (UTF-8 text by default, or
//! decoded from hex/base64 first) and the resulting digest is compared, in
//! constant-ish fashion, against an expected checksum string. The expected
//! checksum may be hex or base64; whitespace and a leading `0x` are tolerated,
//! and the comparison is case-insensitive for hex.
//!
//! The hashing algorithm can be given explicitly, or left as `auto` (the
//! default), in which case it is inferred from the expected digest's byte
//! length — trying every algorithm of that width and reporting the one that
//! matched. All hashers are pure-Rust (RustCrypto + blake3) so the tool runs on
//! every backend, including the chat Service Worker.

use base64::Engine;

/// Which hash algorithm to apply.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Algorithm {
    Md5,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
    Sha3_256,
    Sha3_512,
    Blake2b512,
    Blake2s256,
    Blake3,
}

impl Algorithm {
    /// Canonical lowercase identifier (matches the descriptor enum values).
    pub fn name(self) -> &'static str {
        match self {
            Algorithm::Md5 => "md5",
            Algorithm::Sha1 => "sha1",
            Algorithm::Sha224 => "sha224",
            Algorithm::Sha256 => "sha256",
            Algorithm::Sha384 => "sha384",
            Algorithm::Sha512 => "sha512",
            Algorithm::Sha3_256 => "sha3-256",
            Algorithm::Sha3_512 => "sha3-512",
            Algorithm::Blake2b512 => "blake2b-512",
            Algorithm::Blake2s256 => "blake2s-256",
            Algorithm::Blake3 => "blake3",
        }
    }

    /// Digest width in bytes.
    pub fn digest_len(self) -> usize {
        match self {
            Algorithm::Md5 => 16,
            Algorithm::Sha1 => 20,
            Algorithm::Sha224 => 28,
            Algorithm::Sha256 | Algorithm::Sha3_256 | Algorithm::Blake2s256 | Algorithm::Blake3 => {
                32
            }
            Algorithm::Sha384 => 48,
            Algorithm::Sha512 | Algorithm::Sha3_512 | Algorithm::Blake2b512 => 64,
        }
    }
}

/// How to interpret the input string before hashing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputEncoding {
    /// Hash the UTF-8 bytes of the text as-is (default).
    Text,
    /// Decode the text from hexadecimal first, then hash the raw bytes.
    Hex,
    /// Decode the text from base64 (standard alphabet) first, then hash the bytes.
    Base64,
}

/// The canonical list of supported algorithm identifiers, in menu order. The
/// extra `auto` selector lives only in the descriptor/parser, not here.
pub const ALGORITHMS: &[Algorithm] = &[
    Algorithm::Md5,
    Algorithm::Sha1,
    Algorithm::Sha224,
    Algorithm::Sha256,
    Algorithm::Sha384,
    Algorithm::Sha512,
    Algorithm::Sha3_256,
    Algorithm::Sha3_512,
    Algorithm::Blake2b512,
    Algorithm::Blake2s256,
    Algorithm::Blake3,
];

/// Parse an explicit algorithm id (not `auto`). Tolerant of case and `-`/`_`.
fn parse_algorithm(s: &str) -> Result<Algorithm, String> {
    let lower = s.trim().to_ascii_lowercase();
    let canon: String = lower.chars().filter(|c| *c != '-' && *c != '_').collect();
    let alg = match canon.as_str() {
        "md5" => Algorithm::Md5,
        "sha1" => Algorithm::Sha1,
        "sha224" => Algorithm::Sha224,
        "sha256" => Algorithm::Sha256,
        "sha384" => Algorithm::Sha384,
        "sha512" => Algorithm::Sha512,
        "sha3256" => Algorithm::Sha3_256,
        "sha3512" => Algorithm::Sha3_512,
        "blake2b512" | "blake2b" => Algorithm::Blake2b512,
        "blake2s256" | "blake2s" => Algorithm::Blake2s256,
        "blake3" => Algorithm::Blake3,
        other => {
            return Err(format!(
                "invalid algorithm '{other}': expected 'auto' or one of md5, sha1, sha224, \
                 sha256, sha384, sha512, sha3-256, sha3-512, blake2b-512, blake2s-256, blake3"
            ))
        }
    };
    Ok(alg)
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

/// Decode `text` into raw bytes per `encoding`.
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

/// Compute the raw digest bytes of `data` with `alg`.
pub fn digest_bytes(data: &[u8], alg: Algorithm) -> Vec<u8> {
    use blake2::{Blake2b512, Blake2s256};
    use digest::Digest;
    use sha2::{Sha224, Sha256, Sha384, Sha512};
    use sha3::{Sha3_256, Sha3_512};
    match alg {
        Algorithm::Md5 => md5::Md5::digest(data).to_vec(),
        Algorithm::Sha1 => sha1::Sha1::digest(data).to_vec(),
        Algorithm::Sha224 => Sha224::digest(data).to_vec(),
        Algorithm::Sha256 => Sha256::digest(data).to_vec(),
        Algorithm::Sha384 => Sha384::digest(data).to_vec(),
        Algorithm::Sha512 => Sha512::digest(data).to_vec(),
        Algorithm::Sha3_256 => Sha3_256::digest(data).to_vec(),
        Algorithm::Sha3_512 => Sha3_512::digest(data).to_vec(),
        Algorithm::Blake2b512 => Blake2b512::digest(data).to_vec(),
        Algorithm::Blake2s256 => Blake2s256::digest(data).to_vec(),
        Algorithm::Blake3 => blake3::hash(data).as_bytes().to_vec(),
    }
}

/// Parse the expected-checksum string into raw digest bytes. The expected
/// value may be hex (optionally `0x`-prefixed, any case, internal whitespace
/// tolerated) or standard base64. Hex is preferred when the string is all
/// hex digits and has even length; otherwise base64 is attempted.
fn parse_expected(expected: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = expected.split_whitespace().collect();
    if cleaned.is_empty() {
        return Err("expected checksum is empty".into());
    }
    let hex_str = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(&cleaned);
    let looks_hex =
        !hex_str.is_empty() && hex_str.len() % 2 == 0 && hex_str.bytes().all(|b| b.is_ascii_hexdigit());
    if looks_hex {
        return hex::decode(hex_str)
            .map_err(|e| format!("expected checksum is not valid hex: {e}"));
    }
    base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|_| {
            "expected checksum is neither valid hex nor valid base64".to_string()
        })
}

/// Outcome of a verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verification {
    /// Whether the computed digest matched the expected checksum.
    pub matched: bool,
    /// The algorithm that produced the match, or — when no match — the single
    /// algorithm that was requested, or `None` when `auto` tried several and
    /// none matched.
    pub algorithm: Option<Algorithm>,
    /// The expected digest as lowercase hex.
    pub expected_hex: String,
    /// The actual digest as lowercase hex. For an `auto` run with no match this
    /// is the digest of the first candidate algorithm (so the user sees a value
    /// of the right width to compare).
    pub actual_hex: String,
    /// Human-readable summary line.
    pub summary: String,
}

/// Verify that `text` (interpreted per `input_encoding`) hashes to `expected`.
///
/// `algorithm` is an explicit id (e.g. `sha256`) or `auto`/blank to infer it
/// from the expected digest's byte length. Returns a [`Verification`].
pub fn verify(
    text: &str,
    expected: &str,
    algorithm: &str,
    input_encoding: &str,
) -> Result<Verification, String> {
    let enc = parse_input_encoding(input_encoding)?;
    let data = decode_input(text, enc)?;
    let expected_bytes = parse_expected(expected)?;
    let expected_hex = hex::encode(&expected_bytes);

    let alg_sel = algorithm.trim().to_ascii_lowercase();
    let auto = alg_sel.is_empty() || alg_sel == "auto";

    if !auto {
        // Explicit algorithm: compute it and compare.
        let alg = parse_algorithm(algorithm)?;
        let actual = digest_bytes(&data, alg);
        let actual_hex = hex::encode(&actual);
        let matched = constant_eq(&actual, &expected_bytes);
        let summary = if matched {
            format!("MATCH — input matches the expected {} checksum.", alg.name())
        } else {
            format!(
                "MISMATCH — input does NOT match the expected {} checksum.",
                alg.name()
            )
        };
        return Ok(Verification {
            matched,
            algorithm: Some(alg),
            expected_hex,
            actual_hex,
            summary,
        });
    }

    // Auto mode: candidates are every algorithm whose digest width equals the
    // expected length. Try each; report the first that matches.
    let candidates: Vec<Algorithm> = ALGORITHMS
        .iter()
        .copied()
        .filter(|a| a.digest_len() == expected_bytes.len())
        .collect();
    if candidates.is_empty() {
        return Err(format!(
            "could not auto-detect the algorithm: no supported algorithm produces a \
             {}-byte ({}-hex-char) digest. Pick an algorithm explicitly.",
            expected_bytes.len(),
            expected_bytes.len() * 2
        ));
    }

    let first = candidates[0];
    let mut first_actual = Vec::new();
    for alg in &candidates {
        let actual = digest_bytes(&data, *alg);
        if alg == &first {
            first_actual = actual.clone();
        }
        if constant_eq(&actual, &expected_bytes) {
            let actual_hex = hex::encode(&actual);
            let summary = format!(
                "MATCH — input matches the expected checksum (auto-detected algorithm: {}).",
                alg.name()
            );
            return Ok(Verification {
                matched: true,
                algorithm: Some(*alg),
                expected_hex,
                actual_hex,
                summary,
            });
        }
    }

    // No candidate matched.
    let tried: Vec<&str> = candidates.iter().map(|a| a.name()).collect();
    let summary = format!(
        "MISMATCH — input does not match the expected checksum (tried {}-byte algorithm(s): {}).",
        expected_bytes.len(),
        tried.join(", ")
    );
    Ok(Verification {
        matched: false,
        algorithm: None,
        expected_hex,
        actual_hex: hex::encode(&first_actual),
        summary,
    })
}

/// Length-independent byte comparison that does not short-circuit on the first
/// differing byte (avoids leaking match progress via timing).
fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Render a [`Verification`] as a multi-line text report (used by the page +
/// CLI/chat string surface).
pub fn verify_report(
    text: &str,
    expected: &str,
    algorithm: &str,
    input_encoding: &str,
) -> Result<String, String> {
    let v = verify(text, expected, algorithm, input_encoding)?;
    let alg_line = match v.algorithm {
        Some(a) => a.name().to_string(),
        None => "none matched".to_string(),
    };
    Ok(format!(
        "{}\nstatus: {}\nalgorithm: {}\nexpected: {}\nactual:   {}",
        v.summary,
        if v.matched { "MATCH" } else { "MISMATCH" },
        alg_line,
        v.expected_hex,
        v.actual_hex,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // SHA-256("abc") = ba7816bf...
    const ABC_SHA256: &str =
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const ABC_MD5: &str = "900150983cd24fb0d6963f7d28e17f72";

    #[test]
    fn explicit_sha256_match() {
        let v = verify("abc", ABC_SHA256, "sha256", "").unwrap();
        assert!(v.matched);
        assert_eq!(v.algorithm, Some(Algorithm::Sha256));
    }

    #[test]
    fn explicit_sha256_mismatch() {
        let v = verify("abcd", ABC_SHA256, "sha256", "").unwrap();
        assert!(!v.matched);
        assert_eq!(v.algorithm, Some(Algorithm::Sha256));
    }

    #[test]
    fn auto_detects_md5_by_length() {
        let v = verify("abc", ABC_MD5, "auto", "").unwrap();
        assert!(v.matched);
        assert_eq!(v.algorithm, Some(Algorithm::Md5));
    }

    #[test]
    fn auto_detects_among_32byte_family() {
        // BLAKE3("abc") is 32 bytes — auto must try the whole 32-byte family
        // and land on blake3, not stop at sha256.
        let abc_blake3 = hex::encode(digest_bytes(b"abc", Algorithm::Blake3));
        let v = verify("abc", &abc_blake3, "auto", "").unwrap();
        assert!(v.matched);
        assert_eq!(v.algorithm, Some(Algorithm::Blake3));
    }

    #[test]
    fn auto_blank_is_auto() {
        let v = verify("abc", ABC_SHA256, "", "").unwrap();
        assert!(v.matched);
        assert_eq!(v.algorithm, Some(Algorithm::Sha256));
    }

    #[test]
    fn auto_no_match_reports_mismatch_not_error() {
        let v = verify("not-the-input", ABC_SHA256, "auto", "").unwrap();
        assert!(!v.matched);
        assert_eq!(v.algorithm, None);
    }

    #[test]
    fn expected_accepts_uppercase_and_0x_and_spaces() {
        let spaced = format!("0x{}", ABC_SHA256.to_uppercase());
        let v = verify("abc", &spaced, "sha256", "").unwrap();
        assert!(v.matched);
    }

    #[test]
    fn expected_accepts_base64() {
        // base64 of the SHA-256("abc") digest bytes.
        let b64 = base64::engine::general_purpose::STANDARD
            .encode(digest_bytes(b"abc", Algorithm::Sha256));
        let v = verify("abc", &b64, "sha256", "").unwrap();
        assert!(v.matched);
    }

    #[test]
    fn hex_input_encoding_matches_text() {
        // "abc" == hex 616263
        let v = verify("616263", ABC_SHA256, "sha256", "hex").unwrap();
        assert!(v.matched);
    }

    #[test]
    fn unknown_length_errors() {
        // 10 hex chars = 5 bytes, no algorithm of that width.
        let err = verify("abc", "0123456789", "auto", "").unwrap_err();
        assert!(err.contains("auto-detect"));
    }

    #[test]
    fn bad_algorithm_errors() {
        let err = verify("abc", ABC_SHA256, "sha999", "").unwrap_err();
        assert!(err.contains("invalid algorithm"));
    }

    #[test]
    fn bad_expected_errors() {
        let err = verify("abc", "!!!not-hex-not-b64@@@", "sha256", "").unwrap_err();
        assert!(err.contains("expected checksum"));
    }

    #[test]
    fn report_contains_status() {
        let r = verify_report("abc", ABC_SHA256, "sha256", "").unwrap();
        assert!(r.contains("status: MATCH"));
        assert!(r.contains("algorithm: sha256"));
    }
}
