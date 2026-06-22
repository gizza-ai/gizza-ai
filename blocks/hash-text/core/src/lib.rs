//! hash-text core — compute a cryptographic hash of input text with a
//! selectable algorithm. Pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! The input text can be interpreted as plain UTF-8 (default) or decoded first
//! from hex / base64, and the digest is emitted as lowercase/uppercase hex or
//! base64. All hashers are pure-Rust (RustCrypto + blake3) so the tool runs on
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
pub const ALGORITHMS: &[&str] = &[
    "md5", "sha1", "sha224", "sha256", "sha384", "sha512", "sha3-256", "sha3-512",
    "blake2b-512", "blake2s-256", "blake3",
];

fn parse_algorithm(s: &str) -> Result<Algorithm, String> {
    // Normalize: lowercase, and strip '-'/'_' so "sha3-256", "sha3_256",
    // "sha3256", "SHA-256" etc. all parse to the same algorithm.
    let lower = s.trim().to_ascii_lowercase();
    let canon: String = lower.chars().filter(|c| *c != '-' && *c != '_').collect();
    let alg = match canon.as_str() {
        "" | "sha256" => Algorithm::Sha256, // sha256 is the default
        "md5" => Algorithm::Md5,
        "sha1" => Algorithm::Sha1,
        "sha224" => Algorithm::Sha224,
        "sha384" => Algorithm::Sha384,
        "sha512" => Algorithm::Sha512,
        "sha3256" => Algorithm::Sha3_256,
        "sha3512" => Algorithm::Sha3_512,
        "blake2b512" | "blake2b" => Algorithm::Blake2b512,
        "blake2s256" | "blake2s" => Algorithm::Blake2s256,
        "blake3" => Algorithm::Blake3,
        other => {
            return Err(format!(
                "invalid algorithm '{other}': expected one of {}",
                ALGORITHMS.join(", ")
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
            let cleaned: String = text.split_whitespace().collect();
            hex::decode(&cleaned).map_err(|e| format!("input is not valid hex: {e}"))
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

/// Compute the digest of `text` (interpreted per `input_encoding`) with the
/// selected `algorithm`, rendered per `output_format`. When `output_format` is
/// hex, `uppercase` controls the hex case; it has no effect on base64 output.
///
/// Defaults (blank strings): algorithm=sha256, input_encoding=text,
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

    // Known vectors for "abc".
    #[test]
    fn md5_abc() {
        assert_eq!(
            hash("abc", "md5", "", "", false).unwrap(),
            "900150983cd24fb0d6963f7d28e17f72"
        );
    }

    #[test]
    fn sha1_abc() {
        assert_eq!(
            hash("abc", "sha1", "", "", false).unwrap(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn sha256_default_abc() {
        // Blank algorithm defaults to sha256.
        assert_eq!(
            hash("abc", "", "", "", false).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha224_abc() {
        assert_eq!(
            hash("abc", "sha224", "", "", false).unwrap(),
            "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7"
        );
    }

    #[test]
    fn sha384_abc() {
        assert_eq!(
            hash("abc", "sha384", "", "", false).unwrap(),
            "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7"
        );
    }

    #[test]
    fn sha512_abc() {
        assert_eq!(
            hash("abc", "sha512", "", "", false).unwrap(),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
    }

    #[test]
    fn sha3_256_abc() {
        assert_eq!(
            hash("abc", "sha3-256", "", "", false).unwrap(),
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"
        );
    }

    #[test]
    fn sha3_512_abc() {
        assert_eq!(
            hash("abc", "sha3-512", "", "", false).unwrap(),
            "b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0"
        );
    }

    #[test]
    fn blake2b_512_abc() {
        assert_eq!(
            hash("abc", "blake2b-512", "", "", false).unwrap(),
            "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923"
        );
    }

    #[test]
    fn blake2s_256_abc() {
        assert_eq!(
            hash("abc", "blake2s-256", "", "", false).unwrap(),
            "508c5e8c327c14e2e1a72ba34eeb452f37458b209ed63a294d999b4c86675982"
        );
    }

    #[test]
    fn blake3_abc() {
        assert_eq!(
            hash("abc", "blake3", "", "", false).unwrap(),
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
        );
    }

    #[test]
    fn base64_output() {
        // SHA-256("abc") in base64.
        assert_eq!(
            hash("abc", "sha256", "", "base64", false).unwrap(),
            "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
        );
    }

    #[test]
    fn uppercase_hex() {
        assert_eq!(
            hash("abc", "sha256", "", "hex", true).unwrap(),
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        );
    }

    #[test]
    fn hex_input_matches_text() {
        // "abc" as hex is 616263.
        assert_eq!(
            hash("616263", "sha256", "hex", "", false).unwrap(),
            hash("abc", "sha256", "text", "", false).unwrap()
        );
    }

    #[test]
    fn base64_input_matches_text() {
        // "abc" as base64 is "YWJj".
        assert_eq!(
            hash("YWJj", "sha256", "base64", "", false).unwrap(),
            hash("abc", "sha256", "text", "", false).unwrap()
        );
    }

    #[test]
    fn empty_string_sha256() {
        assert_eq!(
            hash("", "sha256", "", "", false).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn algorithm_aliases() {
        // Mixed case and underscore variants normalize to the same algorithm.
        let a = hash("abc", "SHA3_256", "", "", false).unwrap();
        let b = hash("abc", "sha3-256", "", "", false).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            hash("abc", "blake2b", "", "", false).unwrap(),
            hash("abc", "blake2b-512", "", "", false).unwrap()
        );
    }

    #[test]
    fn invalid_algorithm_errors() {
        let e = hash("abc", "sha999", "", "", false).unwrap_err();
        assert!(e.contains("invalid algorithm"), "{e}");
    }

    #[test]
    fn invalid_input_encoding_errors() {
        let e = hash("abc", "sha256", "rot13", "", false).unwrap_err();
        assert!(e.contains("invalid input_encoding"), "{e}");
    }

    #[test]
    fn bad_hex_input_errors() {
        let e = hash("zz", "sha256", "hex", "", false).unwrap_err();
        assert!(e.contains("not valid hex"), "{e}");
    }
}
