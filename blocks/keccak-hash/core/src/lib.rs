//! keccak-hash core — compute the original **Keccak** digest (Keccak-256 /
//! Keccak-512) of input text.
//!
//! This is the *original* Keccak submission with the `0x01` multi-rate padding,
//! NOT the FIPS-202 SHA-3 standard (which uses `0x06` padding). The two produce
//! DIFFERENT digests for the same input. Keccak-256 is the hash used throughout
//! Ethereum (addresses, transaction/storage hashing, the EVM `KECCAK256`
//! opcode, ABI 4-byte selectors). If you want the NIST standard, use the SHA-3
//! options in the hash-text tool instead.
//!
//! Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps. Uses the RustCrypto `sha3` crate's `Keccak256` /
//! `Keccak512` types (the legacy-padding variants), so it runs on every
//! backend, including the chat Service Worker.

use base64::Engine;

/// Which Keccak variant to apply.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Algorithm {
    /// Keccak-256 (32-byte digest) — the Ethereum hash.
    Keccak256,
    /// Keccak-512 (64-byte digest).
    Keccak512,
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

/// How to render the digest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    /// Lowercase/uppercase hexadecimal.
    Hex,
    /// Standard base64.
    Base64,
}

/// The canonical list of supported algorithm identifiers, in menu order. Kept
/// in one place so the descriptor enum, manifest, and page stay in sync.
pub const ALGORITHMS: &[&str] = &["keccak-256", "keccak-512"];

fn parse_algorithm(s: &str) -> Result<Algorithm, String> {
    // Normalize: lowercase and strip '-'/'_' so "keccak-256", "keccak_256",
    // "keccak256", "KECCAK-256" all parse the same.
    let lower = s.trim().to_ascii_lowercase();
    let canon: String = lower.chars().filter(|c| *c != '-' && *c != '_').collect();
    match canon.as_str() {
        "" | "keccak256" | "keccak" => Ok(Algorithm::Keccak256), // keccak-256 is the default
        "keccak512" => Ok(Algorithm::Keccak512),
        other => Err(format!(
            "invalid algorithm '{other}': expected one of {}",
            ALGORITHMS.join(", ")
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
        "base64" | "b64" => Ok(OutputFormat::Base64),
        other => Err(format!(
            "invalid output_format '{other}': expected 'hex' or 'base64'"
        )),
    }
}

/// Decode `text` into raw bytes per `encoding`.
fn decode_input(text: &str, encoding: InputEncoding) -> Result<Vec<u8>, String> {
    match encoding {
        InputEncoding::Text => Ok(text.as_bytes().to_vec()),
        InputEncoding::Hex => {
            // Tolerate an optional 0x prefix and internal whitespace.
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

/// Compute the raw Keccak digest bytes of `data` with `alg`.
pub fn digest_bytes(data: &[u8], alg: Algorithm) -> Vec<u8> {
    use digest::Digest;
    use sha3::{Keccak256, Keccak512};
    match alg {
        Algorithm::Keccak256 => Keccak256::digest(data).to_vec(),
        Algorithm::Keccak512 => Keccak512::digest(data).to_vec(),
    }
}

/// Compute the Keccak digest of `text` (interpreted per `input_encoding`) with
/// the selected `algorithm`, rendered per `output_format`. When `output_format`
/// is hex, `uppercase` controls the hex case; it has no effect on base64.
///
/// Defaults (blank strings): algorithm=keccak-256, input_encoding=text,
/// output_format=hex, lowercase.
pub fn hash(
    text: &str,
    algorithm: &str,
    input_encoding: &str,
    output_format: &str,
    uppercase: bool,
) -> Result<String, String> {
    let alg = parse_algorithm(algorithm)?;
    let enc = parse_input_encoding(input_encoding)?;
    let fmt = parse_output_format(output_format)?;
    let bytes = decode_input(text, enc)?;
    let digest = digest_bytes(&bytes, alg);
    Ok(match fmt {
        OutputFormat::Hex => {
            let h = hex::encode(&digest);
            if uppercase {
                h.to_uppercase()
            } else {
                h
            }
        }
        OutputFormat::Base64 => base64::engine::general_purpose::STANDARD.encode(&digest),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Keccak-256 of the empty string — the well-known Ethereum "empty" hash.
    #[test]
    fn keccak256_empty() {
        assert_eq!(
            hash("", "keccak-256", "", "", false).unwrap(),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
    }

    // Keccak-256("abc") — distinct from SHA3-256("abc").
    #[test]
    fn keccak256_abc() {
        assert_eq!(
            hash("abc", "keccak-256", "", "", false).unwrap(),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
    }

    // Default (blank algorithm) is keccak-256.
    #[test]
    fn default_is_keccak256() {
        assert_eq!(
            hash("abc", "", "", "", false).unwrap(),
            hash("abc", "keccak-256", "", "", false).unwrap()
        );
    }

    // Keccak differs from FIPS SHA-3: SHA3-256("abc") is
    // 3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532.
    #[test]
    fn keccak_differs_from_sha3() {
        assert_ne!(
            hash("abc", "keccak-256", "", "", false).unwrap(),
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"
        );
    }

    // Keccak-512 of the empty string.
    #[test]
    fn keccak512_empty() {
        assert_eq!(
            hash("", "keccak-512", "", "", false).unwrap(),
            "0eab42de4c3ceb9235fc91acffe746b29c29a8c366b7c60e4e67c466f36a4304c00fa9caf9d87976ba469bcbe06713b435f091ef2769fb160cdab33d3670680e"
        );
    }

    // "Hello, World!" → a commonly cited Keccak-256 vector.
    #[test]
    fn keccak256_hello_world() {
        assert_eq!(
            hash("Hello, World!", "keccak-256", "", "", false).unwrap(),
            "acaf3289d7b601cbd114fb36c4d29c85bbfd5e133f14cb355c3fd8d99367964f"
        );
    }

    #[test]
    fn base64_output() {
        // Keccak-256("") rendered as base64 must match base64 of the raw digest.
        let hex = hash("", "keccak-256", "", "hex", false).unwrap();
        let raw = hex::decode(hex).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&raw);
        assert_eq!(hash("", "keccak-256", "", "base64", false).unwrap(), b64);
    }

    #[test]
    fn uppercase_hex() {
        let lo = hash("abc", "keccak-256", "", "hex", false).unwrap();
        assert_eq!(
            hash("abc", "keccak-256", "", "hex", true).unwrap(),
            lo.to_uppercase()
        );
    }

    #[test]
    fn hex_input_matches_text() {
        // "abc" as hex is 616263.
        assert_eq!(
            hash("616263", "keccak-256", "hex", "", false).unwrap(),
            hash("abc", "keccak-256", "text", "", false).unwrap()
        );
    }

    #[test]
    fn hex_input_accepts_0x_prefix() {
        assert_eq!(
            hash("0x616263", "keccak-256", "hex", "", false).unwrap(),
            hash("abc", "keccak-256", "text", "", false).unwrap()
        );
    }

    #[test]
    fn base64_input_matches_text() {
        // "abc" as base64 is "YWJj".
        assert_eq!(
            hash("YWJj", "keccak-256", "base64", "", false).unwrap(),
            hash("abc", "keccak-256", "text", "", false).unwrap()
        );
    }

    #[test]
    fn algorithm_aliases() {
        let a = hash("abc", "KECCAK_256", "", "", false).unwrap();
        let b = hash("abc", "keccak-256", "", "", false).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            hash("abc", "keccak512", "", "", false).unwrap(),
            hash("abc", "keccak-512", "", "", false).unwrap()
        );
    }

    #[test]
    fn invalid_algorithm_errors() {
        let e = hash("abc", "sha256", "", "", false).unwrap_err();
        assert!(e.contains("invalid algorithm"), "{e}");
    }

    #[test]
    fn invalid_input_encoding_errors() {
        let e = hash("abc", "keccak-256", "rot13", "", false).unwrap_err();
        assert!(e.contains("invalid input_encoding"), "{e}");
    }

    #[test]
    fn bad_hex_input_errors() {
        let e = hash("zz", "keccak-256", "hex", "", false).unwrap_err();
        assert!(e.contains("not valid hex"), "{e}");
    }
}
