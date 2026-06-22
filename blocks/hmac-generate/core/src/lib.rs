//! hmac-generate core — compute a keyed-hash message authentication code (HMAC)
//! of a message with a selectable underlying hash algorithm. Pure compute,
//! shared by the chat skill block and the web page. No wafer/wasm-bindgen deps.
//!
//! HMAC (RFC 2104) combines a secret key with a message and a cryptographic
//! hash to produce a tag that proves both integrity and authenticity — unlike a
//! plain hash, you cannot forge or recompute the tag without the key. The
//! message and the key can each be interpreted as plain UTF-8 (default) or
//! decoded first from hex / base64, and the tag is emitted as lowercase/
//! uppercase hex or base64. All hashers are pure-Rust (RustCrypto) so the tool
//! runs on every backend, including the chat Service Worker.

use base64::Engine;
use hmac::{Hmac, Mac};

/// Which underlying hash algorithm to key.
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
}

/// How to interpret an input string (message or key) before use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputEncoding {
    /// Use the UTF-8 bytes of the text as-is (default).
    Text,
    /// Decode the text from hexadecimal first, then use the raw bytes.
    Hex,
    /// Decode the text from base64 (standard alphabet) first, then use the bytes.
    Base64,
}

/// How to render the tag.
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
];

fn parse_algorithm(s: &str) -> Result<Algorithm, String> {
    // Normalize: lowercase, and strip '-'/'_' so "sha3-256", "sha3_256",
    // "SHA-256" etc. all parse to the same algorithm.
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
        other => {
            return Err(format!(
                "invalid algorithm '{other}': expected one of {}",
                ALGORITHMS.join(", ")
            ))
        }
    };
    Ok(alg)
}

fn parse_input_encoding(s: &str, field: &str) -> Result<InputEncoding, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "text" | "utf8" | "utf-8" => Ok(InputEncoding::Text),
        "hex" => Ok(InputEncoding::Hex),
        "base64" | "b64" => Ok(InputEncoding::Base64),
        other => Err(format!(
            "invalid {field} '{other}': expected 'text', 'hex', or 'base64'"
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

/// Decode `text` into raw bytes per `encoding`. `field` names the value for
/// error messages (e.g. "message" / "key").
fn decode_input(text: &str, encoding: InputEncoding, field: &str) -> Result<Vec<u8>, String> {
    match encoding {
        InputEncoding::Text => Ok(text.as_bytes().to_vec()),
        InputEncoding::Hex => {
            // Tolerate an optional 0x prefix and internal whitespace (matches
            // keccak-hash / hash-text so the hash family accepts the same input).
            let cleaned: String = text.split_whitespace().collect();
            let cleaned = cleaned
                .strip_prefix("0x")
                .or_else(|| cleaned.strip_prefix("0X"))
                .unwrap_or(&cleaned);
            hex::decode(cleaned).map_err(|e| format!("{field} is not valid hex: {e}"))
        }
        InputEncoding::Base64 => {
            let cleaned: String = text.split_whitespace().collect();
            base64::engine::general_purpose::STANDARD
                .decode(cleaned.as_bytes())
                .map_err(|e| format!("{field} is not valid base64: {e}"))
        }
    }
}

/// Compute the raw HMAC tag of `message` keyed by `key` with `alg`.
fn hmac_bytes(key: &[u8], message: &[u8], alg: Algorithm) -> Vec<u8> {
    use sha2::{Sha224, Sha256, Sha384, Sha512};
    use sha3::{Sha3_256, Sha3_512};
    fn mac<D>(key: &[u8], message: &[u8]) -> Vec<u8>
    where
        D: digest::core_api::CoreProxy,
        D::Core: Sync
            + Send
            + Clone
            + digest::core_api::BufferKindUser<BufferKind = digest::block_buffer::Eager>
            + digest::core_api::FixedOutputCore
            + digest::HashMarker
            + Default,
        <D::Core as digest::core_api::BlockSizeUser>::BlockSize:
            digest::typenum::IsLess<digest::consts::U256>,
        digest::typenum::Le<
            <D::Core as digest::core_api::BlockSizeUser>::BlockSize,
            digest::consts::U256,
        >: digest::typenum::NonZero,
    {
        // HMAC accepts a key of any length (it is internally padded/hashed to the
        // block size), so new_from_slice never fails here.
        let mut m = Hmac::<D>::new_from_slice(key).expect("hmac accepts any key length");
        m.update(message);
        m.finalize().into_bytes().to_vec()
    }
    match alg {
        Algorithm::Md5 => mac::<md5::Md5>(key, message),
        Algorithm::Sha1 => mac::<sha1::Sha1>(key, message),
        Algorithm::Sha224 => mac::<Sha224>(key, message),
        Algorithm::Sha256 => mac::<Sha256>(key, message),
        Algorithm::Sha384 => mac::<Sha384>(key, message),
        Algorithm::Sha512 => mac::<Sha512>(key, message),
        Algorithm::Sha3_256 => mac::<Sha3_256>(key, message),
        Algorithm::Sha3_512 => mac::<Sha3_512>(key, message),
    }
}

/// Compute the HMAC of `message` (interpreted per `message_encoding`) keyed by
/// `key` (interpreted per `key_encoding`) with the selected `algorithm`,
/// rendered per `output_format`. When `output_format` is hex, `uppercase`
/// controls the hex case; it has no effect on base64 output.
///
/// Defaults (blank strings): algorithm=sha256, message_encoding=text,
/// key_encoding=text, output_format=hex, lowercase.
#[allow(clippy::too_many_arguments)]
pub fn hmac(
    message: &str,
    key: &str,
    algorithm: &str,
    message_encoding: &str,
    key_encoding: &str,
    output_format: &str,
    uppercase: bool,
) -> Result<String, String> {
    let alg = parse_algorithm(algorithm)?;
    let msg_enc = parse_input_encoding(message_encoding, "message_encoding")?;
    let key_enc = parse_input_encoding(key_encoding, "key_encoding")?;
    let fmt = parse_output_format(output_format)?;
    let msg_bytes = decode_input(message, msg_enc, "message")?;
    let key_bytes = decode_input(key, key_enc, "key")?;
    let tag = hmac_bytes(&key_bytes, &msg_bytes, alg);
    Ok(match fmt {
        OutputFormat::Hex => {
            let h = hex::encode(&tag);
            if uppercase {
                h.to_uppercase()
            } else {
                h
            }
        }
        OutputFormat::Base64 => base64::engine::general_purpose::STANDARD.encode(&tag),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 4231 test case 2: key = "Jefe", data = "what do ya want for nothing?".
    const RFC4231_KEY: &str = "Jefe";
    const RFC4231_MSG: &str = "what do ya want for nothing?";

    #[test]
    fn hmac_sha256_rfc4231_tc2() {
        assert_eq!(
            hmac(RFC4231_MSG, RFC4231_KEY, "sha256", "", "", "", false).unwrap(),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn hmac_sha224_rfc4231_tc2() {
        assert_eq!(
            hmac(RFC4231_MSG, RFC4231_KEY, "sha224", "", "", "", false).unwrap(),
            "a30e01098bc6dbbf45690f3a7e9e6d0f8bbea2a39e6148008fd05e44"
        );
    }

    #[test]
    fn hmac_sha384_rfc4231_tc2() {
        assert_eq!(
            hmac(RFC4231_MSG, RFC4231_KEY, "sha384", "", "", "", false).unwrap(),
            "af45d2e376484031617f78d2b58a6b1b9c7ef464f5a01b47e42ec3736322445e8e2240ca5e69e2c78b3239ecfab21649"
        );
    }

    #[test]
    fn hmac_sha512_rfc4231_tc2() {
        assert_eq!(
            hmac(RFC4231_MSG, RFC4231_KEY, "sha512", "", "", "", false).unwrap(),
            "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737"
        );
    }

    #[test]
    fn hmac_default_is_sha256() {
        // Blank algorithm defaults to sha256.
        assert_eq!(
            hmac(RFC4231_MSG, RFC4231_KEY, "", "", "", "", false).unwrap(),
            hmac(RFC4231_MSG, RFC4231_KEY, "sha256", "", "", "", false).unwrap()
        );
    }

    // RFC 2202 HMAC-MD5 test case 2 (same key/data as RFC4231 tc2).
    #[test]
    fn hmac_md5_rfc2202_tc2() {
        assert_eq!(
            hmac(RFC4231_MSG, RFC4231_KEY, "md5", "", "", "", false).unwrap(),
            "750c783e6ab0b503eaa86e310a5db738"
        );
    }

    // RFC 2202 HMAC-SHA1 test case 2.
    #[test]
    fn hmac_sha1_rfc2202_tc2() {
        assert_eq!(
            hmac(RFC4231_MSG, RFC4231_KEY, "sha1", "", "", "", false).unwrap(),
            "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
        );
    }

    // HMAC-SHA3-256 (key "Jefe", same message).
    #[test]
    fn hmac_sha3_256_jefe() {
        assert_eq!(
            hmac(RFC4231_MSG, RFC4231_KEY, "sha3-256", "", "", "", false).unwrap(),
            "c7d4072e788877ae3596bbb0da73b887c9171f93095b294ae857fbe2645e1ba5"
        );
    }

    #[test]
    fn hmac_sha3_512_jefe() {
        assert_eq!(
            hmac(RFC4231_MSG, RFC4231_KEY, "sha3-512", "", "", "", false).unwrap(),
            "5a4bfeab6166427c7a3647b747292b8384537cdb89afb3bf5665e4c5e709350b287baec921fd7ca0ee7a0c31d022a95e1fc92ba9d77df883960275beb4e62024"
        );
    }

    #[test]
    fn base64_output() {
        // HMAC-SHA256(Jefe, msg) in base64.
        assert_eq!(
            hmac(RFC4231_MSG, RFC4231_KEY, "sha256", "", "", "base64", false).unwrap(),
            "W9zBRr9gdU5qBCQmCJV1x1oAPwidJzmDnexYuWTsOEM="
        );
    }

    #[test]
    fn uppercase_hex() {
        assert_eq!(
            hmac(RFC4231_MSG, RFC4231_KEY, "sha256", "", "", "hex", true).unwrap(),
            "5BDCC146BF60754E6A042426089575C75A003F089D2739839DEC58B964EC3843"
        );
    }

    #[test]
    fn hex_key_matches_text_key() {
        // "Jefe" as hex is 4a656665.
        assert_eq!(
            hmac(RFC4231_MSG, "4a656665", "sha256", "", "hex", "", false).unwrap(),
            hmac(RFC4231_MSG, "Jefe", "sha256", "", "text", "", false).unwrap()
        );
    }

    #[test]
    fn hex_accepts_0x_prefix() {
        // A leading 0x on hex message/key is tolerated, matching keccak-hash / hash-text.
        let plain = hmac("4869205468657265", "4a656665", "sha256", "hex", "hex", "", false).unwrap();
        assert_eq!(
            hmac("0x4869205468657265", "0x4a656665", "sha256", "hex", "hex", "", false).unwrap(),
            plain
        );
    }

    #[test]
    fn hex_message_matches_text_message() {
        // RFC 4231 tc1: key = 0x0b * 20, data = "Hi There".
        let hi_there_hex = "4869205468657265"; // "Hi There"
        let key_hex = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";
        assert_eq!(
            hmac(hi_there_hex, key_hex, "sha256", "hex", "hex", "", false).unwrap(),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn invalid_algorithm_errors() {
        let e = hmac("a", "k", "sha999", "", "", "", false).unwrap_err();
        assert!(e.contains("invalid algorithm"), "{e}");
    }

    #[test]
    fn invalid_message_encoding_errors() {
        let e = hmac("a", "k", "sha256", "rot13", "", "", false).unwrap_err();
        assert!(e.contains("invalid message_encoding"), "{e}");
    }

    #[test]
    fn invalid_key_encoding_errors() {
        let e = hmac("a", "k", "sha256", "", "rot13", "", false).unwrap_err();
        assert!(e.contains("invalid key_encoding"), "{e}");
    }

    #[test]
    fn bad_hex_key_errors() {
        let e = hmac("a", "zz", "sha256", "", "hex", "", false).unwrap_err();
        assert!(e.contains("key is not valid hex"), "{e}");
    }

    #[test]
    fn empty_key_and_message() {
        // HMAC-SHA256("", "") is a well-known vector.
        assert_eq!(
            hmac("", "", "sha256", "", "", "", false).unwrap(),
            "b613679a0814d9ec772f95d778c35fc5ff1697c493715653c6c712144292c5ad"
        );
    }
}
