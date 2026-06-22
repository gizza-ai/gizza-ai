//! sha256-hash core — compute the SHA-256 (SHA-2) digest of input text.
//!
//! Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps. The input text can be interpreted as plain UTF-8
//! text (default) or decoded first from hex / base64, and the digest is
//! emitted as lowercase/uppercase hex or base64.

use base64::Engine;
use sha2::{Digest, Sha256};

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

/// How to render the 32-byte digest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    /// Lowercase/uppercase hexadecimal (64 chars).
    Hex,
    /// Standard base64 (44 chars incl. padding).
    Base64,
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

/// Compute the raw 32-byte SHA-256 digest of `bytes`.
pub fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// Compute the SHA-256 digest of `text` (interpreted per `input_encoding`) and
/// render it per `output_format`. When `output_format` is hex, `uppercase`
/// controls the hex case; it has no effect on base64 output.
///
/// Defaults (blank strings): input_encoding=text, output_format=hex, lowercase.
pub fn hash(
    text: &str,
    input_encoding: &str,
    output_format: &str,
    uppercase: bool,
) -> Result<String, String> {
    let enc = parse_input_encoding(input_encoding)?;
    let fmt = parse_output_format(output_format)?;
    let bytes = decode_input(text, enc)?;
    let digest = sha256_bytes(&bytes);
    Ok(match fmt {
        OutputFormat::Hex => {
            let h = hex::encode(digest);
            if uppercase {
                h.to_uppercase()
            } else {
                h
            }
        }
        OutputFormat::Base64 => base64::engine::general_purpose::STANDARD.encode(digest),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known vector: SHA-256("abc")
    const ABC_HEX: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn hashes_text_abc() {
        assert_eq!(hash("abc", "", "", false).unwrap(), ABC_HEX);
    }

    #[test]
    fn hashes_empty_string() {
        // SHA-256("") is a well-known constant.
        assert_eq!(
            hash("", "text", "hex", false).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn uppercase_hex() {
        assert_eq!(hash("abc", "text", "hex", true).unwrap(), ABC_HEX.to_uppercase());
    }

    #[test]
    fn base64_output() {
        // base64 of the raw digest of "abc"
        assert_eq!(
            hash("abc", "text", "base64", false).unwrap(),
            "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
        );
    }

    #[test]
    fn hex_input_matches_text() {
        // "abc" as hex is 616263; hashing the decoded bytes equals hashing "abc".
        assert_eq!(hash("616263", "hex", "hex", false).unwrap(), ABC_HEX);
    }

    #[test]
    fn base64_input_matches_text() {
        // "abc" base64 is YWJj
        assert_eq!(hash("YWJj", "base64", "hex", false).unwrap(), ABC_HEX);
    }

    #[test]
    fn rejects_bad_hex() {
        assert!(hash("zz", "hex", "hex", false).is_err());
    }

    #[test]
    fn rejects_bad_base64() {
        assert!(hash("not base64!!!", "base64", "hex", false).is_err());
    }

    #[test]
    fn rejects_bad_input_encoding() {
        assert!(hash("abc", "rot13", "hex", false).is_err());
    }

    #[test]
    fn rejects_bad_output_format() {
        assert!(hash("abc", "text", "binary", false).is_err());
    }
}
