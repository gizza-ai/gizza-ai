//! sha3-hash core — compute the FIPS-202 **SHA-3** digest (SHA3-256 / SHA3-384
//! / SHA3-512) of input text.
//!
//! Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps. Uses the RustCrypto `sha3` crate's `Sha3_256` /
//! `Sha3_384` / `Sha3_512` types — the **standardized** SHA-3 with `0x06`
//! multi-rate padding (NOT the original Keccak with `0x01` padding, which gives
//! different digests; use the keccak-hash tool for that). Runs on every backend,
//! including the chat Service Worker.
//!
//! The input text can be interpreted as plain UTF-8 (default) or decoded first
//! from hex / base64, and the digest is emitted as lowercase/uppercase hex or
//! base64.

use base64::Engine;

/// Which FIPS-202 SHA-3 variant to apply.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Algorithm {
    /// SHA3-256 (32-byte digest) — the default.
    Sha3_256,
    /// SHA3-384 (48-byte digest).
    Sha3_384,
    /// SHA3-512 (64-byte digest).
    Sha3_512,
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
pub const ALGORITHMS: &[&str] = &["sha3-256", "sha3-384", "sha3-512"];

fn parse_algorithm(s: &str) -> Result<Algorithm, String> {
    // Normalize: lowercase and strip '-'/'_' so "sha3-256", "sha3_256",
    // "sha3256", "SHA3-256" all parse the same.
    let lower = s.trim().to_ascii_lowercase();
    let canon: String = lower.chars().filter(|c| *c != '-' && *c != '_').collect();
    match canon.as_str() {
        "" | "sha3256" | "sha3" => Ok(Algorithm::Sha3_256), // sha3-256 is the default
        "sha3384" => Ok(Algorithm::Sha3_384),
        "sha3512" => Ok(Algorithm::Sha3_512),
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

/// Compute the raw SHA-3 digest bytes of `data` with `alg`.
pub fn digest_bytes(data: &[u8], alg: Algorithm) -> Vec<u8> {
    use digest::Digest;
    use sha3::{Sha3_256, Sha3_384, Sha3_512};
    match alg {
        Algorithm::Sha3_256 => Sha3_256::digest(data).to_vec(),
        Algorithm::Sha3_384 => Sha3_384::digest(data).to_vec(),
        Algorithm::Sha3_512 => Sha3_512::digest(data).to_vec(),
    }
}

/// Compute the SHA-3 digest of `text` (interpreted per `input_encoding`) with
/// the selected `algorithm`, rendered per `output_format`. When `output_format`
/// is hex, `uppercase` controls the hex case; it has no effect on base64.
///
/// Defaults (blank strings): algorithm=sha3-256, input_encoding=text,
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

    // FIPS-202 SHA3-256("abc") — the standard NIST test vector, distinct from
    // the original Keccak-256("abc").
    const ABC_SHA3_256: &str =
        "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532";

    #[test]
    fn sha3_256_abc() {
        assert_eq!(hash("abc", "sha3-256", "", "", false).unwrap(), ABC_SHA3_256);
    }

    #[test]
    fn default_is_sha3_256() {
        assert_eq!(
            hash("abc", "", "", "", false).unwrap(),
            hash("abc", "sha3-256", "", "", false).unwrap()
        );
    }

    // SHA3-256 of the empty string — well-known constant.
    #[test]
    fn sha3_256_empty() {
        assert_eq!(
            hash("", "sha3-256", "", "", false).unwrap(),
            "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"
        );
    }

    // SHA-3 differs from the original Keccak: Keccak-256("abc") is
    // 4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45.
    #[test]
    fn sha3_differs_from_keccak() {
        assert_ne!(
            hash("abc", "sha3-256", "", "", false).unwrap(),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
    }

    // SHA3-384("abc") — NIST vector.
    #[test]
    fn sha3_384_abc() {
        assert_eq!(
            hash("abc", "sha3-384", "", "", false).unwrap(),
            "ec01498288516fc926459f58e2c6ad8df9b473cb0fc08c2596da7cf0e49be4b298d88cea927ac7f539f1edf228376d25"
        );
    }

    // SHA3-512("abc") — NIST vector.
    #[test]
    fn sha3_512_abc() {
        assert_eq!(
            hash("abc", "sha3-512", "", "", false).unwrap(),
            "b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0"
        );
    }

    #[test]
    fn base64_output() {
        // SHA3-256("abc") rendered as base64 must match base64 of the raw digest.
        let hex = hash("abc", "sha3-256", "", "hex", false).unwrap();
        let raw = hex::decode(hex).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&raw);
        assert_eq!(hash("abc", "sha3-256", "", "base64", false).unwrap(), b64);
    }

    #[test]
    fn uppercase_hex() {
        assert_eq!(
            hash("abc", "sha3-256", "", "hex", true).unwrap(),
            ABC_SHA3_256.to_uppercase()
        );
    }

    #[test]
    fn hex_input_matches_text() {
        // "abc" as hex is 616263.
        assert_eq!(
            hash("616263", "sha3-256", "hex", "", false).unwrap(),
            hash("abc", "sha3-256", "text", "", false).unwrap()
        );
    }

    #[test]
    fn hex_input_accepts_0x_prefix() {
        assert_eq!(
            hash("0x616263", "sha3-256", "hex", "", false).unwrap(),
            hash("abc", "sha3-256", "text", "", false).unwrap()
        );
    }

    #[test]
    fn base64_input_matches_text() {
        // "abc" as base64 is "YWJj".
        assert_eq!(
            hash("YWJj", "sha3-256", "base64", "", false).unwrap(),
            hash("abc", "sha3-256", "text", "", false).unwrap()
        );
    }

    #[test]
    fn algorithm_aliases() {
        assert_eq!(
            hash("abc", "SHA3_256", "", "", false).unwrap(),
            hash("abc", "sha3-256", "", "", false).unwrap()
        );
        assert_eq!(
            hash("abc", "sha3512", "", "", false).unwrap(),
            hash("abc", "sha3-512", "", "", false).unwrap()
        );
    }

    #[test]
    fn rejects_bad_algorithm() {
        assert!(hash("abc", "sha3-128", "", "", false).is_err());
    }

    #[test]
    fn rejects_bad_hex() {
        assert!(hash("zz", "sha3-256", "hex", "", false).is_err());
    }

    #[test]
    fn rejects_bad_base64() {
        assert!(hash("not base64!!!", "sha3-256", "base64", "", false).is_err());
    }

    #[test]
    fn rejects_bad_input_encoding() {
        assert!(hash("abc", "sha3-256", "rot13", "", false).is_err());
    }

    #[test]
    fn rejects_bad_output_format() {
        assert!(hash("abc", "sha3-256", "", "binary", false).is_err());
    }
}
