//! cmac-generate core — compute an AES-CMAC (Cipher-based Message Authentication
//! Code, NIST SP 800-38B / RFC 4493) over a message with a 128/192/256-bit key.
//! Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps.
//!
//! CMAC is a block-cipher MAC: it keys AES with a secret key, derives two
//! subkeys, and runs a CBC-MAC-style chain over the (padded) message to produce
//! a 16-byte tag that proves both integrity and authenticity. Unlike a plain
//! hash, the tag cannot be recomputed or forged without the key. The AES variant
//! (AES-128/192/256) is selected by the KEY LENGTH (16/24/32 bytes). The message
//! and key can each be read as UTF-8 text (default) or decoded first from hex /
//! base64, and the tag is emitted as lowercase/uppercase hex or base64. The AES
//! implementation is pure-Rust (RustCrypto) so the tool runs on every backend,
//! including the chat Service Worker.

use base64::Engine;
use cmac::{Cmac, Mac};

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
            // the hash/hmac family so the crypto tools accept the same input).
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

/// Compute the raw AES-CMAC tag of `message` keyed by `key`. The AES variant is
/// chosen by the key length: 16 bytes → AES-128, 24 → AES-192, 32 → AES-256.
fn cmac_bytes(key: &[u8], message: &[u8]) -> Result<Vec<u8>, String> {
    macro_rules! mac {
        ($cipher:ty) => {{
            let mut m = <Cmac<$cipher> as Mac>::new_from_slice(key)
                .expect("key length already validated for this cipher");
            m.update(message);
            m.finalize().into_bytes().to_vec()
        }};
    }
    match key.len() {
        16 => Ok(mac!(aes::Aes128)),
        24 => Ok(mac!(aes::Aes192)),
        32 => Ok(mac!(aes::Aes256)),
        n => Err(format!(
            "AES-CMAC requires a 16-, 24-, or 32-byte key (AES-128/192/256); got {n} byte(s). \
             A text key is used as-is — for a binary key set key_encoding to 'hex' or 'base64'."
        )),
    }
}

/// Compute the AES-CMAC of `message` (interpreted per `message_encoding`) keyed
/// by `key` (interpreted per `key_encoding`), rendered per `output_format`. When
/// `output_format` is hex, `uppercase` controls the hex case; it has no effect
/// on base64 output. The AES variant follows the key length (16/24/32 bytes).
///
/// Defaults (blank strings): message_encoding=text, key_encoding=text,
/// output_format=hex, lowercase.
pub fn cmac(
    message: &str,
    key: &str,
    message_encoding: &str,
    key_encoding: &str,
    output_format: &str,
    uppercase: bool,
) -> Result<String, String> {
    let msg_enc = parse_input_encoding(message_encoding, "message_encoding")?;
    let key_enc = parse_input_encoding(key_encoding, "key_encoding")?;
    let fmt = parse_output_format(output_format)?;
    let msg_bytes = decode_input(message, msg_enc, "message")?;
    let key_bytes = decode_input(key, key_enc, "key")?;
    let tag = cmac_bytes(&key_bytes, &msg_bytes)?;
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

    // NIST SP 800-38B / RFC 4493 AES-128 key.
    const KEY128: &str = "2b7e151628aed2a6abf7158809cf4f3c";
    // RFC 4493 example messages (hex).
    const MLEN16: &str = "6bc1bee22e409f96e93d7e117393172a";
    const MLEN40: &str =
        "6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e5130c81c46a35ce411";
    const MLEN64: &str = "6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e5130c81c46a35ce411e5fbc1191a0a52eff69f2445df4f9b17ad2b417be66c3710";

    // RFC 4493 / NIST SP 800-38B AES-128-CMAC test vectors.
    #[test]
    fn aes128_empty_message() {
        // Mlen = 0
        assert_eq!(
            cmac("", KEY128, "", "hex", "", false).unwrap(),
            "bb1d6929e95937287fa37d129b756746"
        );
    }

    #[test]
    fn aes128_16_byte_message() {
        // Mlen = 128 (one full block)
        assert_eq!(
            cmac(MLEN16, KEY128, "hex", "hex", "", false).unwrap(),
            "070a16b46b4d4144f79bdd9dd04a287c"
        );
    }

    #[test]
    fn aes128_40_byte_message() {
        // Mlen = 320 (partial block → padded)
        assert_eq!(
            cmac(MLEN40, KEY128, "hex", "hex", "", false).unwrap(),
            "dfa66747de9ae63030ca32611497c827"
        );
    }

    #[test]
    fn aes128_64_byte_message() {
        // Mlen = 512 (exact multiple of block size)
        assert_eq!(
            cmac(MLEN64, KEY128, "hex", "hex", "", false).unwrap(),
            "51f0bebf7e3b9d92fc49741779363cfe"
        );
    }

    // NIST SP 800-38B AES-192-CMAC, empty message.
    #[test]
    fn aes192_empty_message() {
        let key192 = "8e73b0f7da0e6452c810f32b809079e562f8ead2522c6b7b";
        assert_eq!(
            cmac("", key192, "", "hex", "", false).unwrap(),
            "d17ddf46adaacde531cac483de7a9367"
        );
    }

    // NIST SP 800-38B AES-256-CMAC, empty message.
    #[test]
    fn aes256_empty_message() {
        let key256 =
            "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4";
        assert_eq!(
            cmac("", key256, "", "hex", "", false).unwrap(),
            "028962f61b7bf89efc6b551f4667d983"
        );
    }

    #[test]
    fn aes256_16_byte_message() {
        let key256 =
            "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4";
        assert_eq!(
            cmac(MLEN16, key256, "hex", "hex", "", false).unwrap(),
            "28a7023f452e8f82bd4bf28d8c37c35c"
        );
    }

    #[test]
    fn base64_output() {
        // AES-128-CMAC of the empty message in base64.
        assert_eq!(
            cmac("", KEY128, "", "hex", "base64", false).unwrap(),
            "ux1pKelZNyh/o30Sm3VnRg=="
        );
    }

    #[test]
    fn uppercase_hex() {
        assert_eq!(
            cmac("", KEY128, "", "hex", "hex", true).unwrap(),
            "BB1D6929E95937287FA37D129B756746"
        );
    }

    #[test]
    fn text_key_used_as_bytes() {
        // A 16-char ASCII key is a valid AES-128 key (16 bytes).
        let tag = cmac("hello", "0123456789abcdef", "", "", "", false).unwrap();
        assert_eq!(tag.len(), 32); // 16 bytes hex
    }

    #[test]
    fn base64_key_matches_hex_key() {
        // KEY128 hex == its base64 form; same tag.
        let b64_key = base64::engine::general_purpose::STANDARD
            .encode(hex::decode(KEY128).unwrap());
        assert_eq!(
            cmac(MLEN16, &b64_key, "hex", "base64", "", false).unwrap(),
            cmac(MLEN16, KEY128, "hex", "hex", "", false).unwrap()
        );
    }

    #[test]
    fn hex_accepts_0x_prefix() {
        let plain = cmac(MLEN16, KEY128, "hex", "hex", "", false).unwrap();
        assert_eq!(
            cmac(&format!("0x{MLEN16}"), &format!("0x{KEY128}"), "hex", "hex", "", false).unwrap(),
            plain
        );
    }

    #[test]
    fn wrong_key_length_errors() {
        // 5-byte text key is not a valid AES key length.
        let e = cmac("a", "short", "", "", "", false).unwrap_err();
        assert!(e.contains("16-, 24-, or 32-byte key"), "{e}");
    }

    #[test]
    fn invalid_message_encoding_errors() {
        let e = cmac("a", KEY128, "rot13", "hex", "", false).unwrap_err();
        assert!(e.contains("invalid message_encoding"), "{e}");
    }

    #[test]
    fn invalid_key_encoding_errors() {
        let e = cmac("a", KEY128, "", "rot13", "", false).unwrap_err();
        assert!(e.contains("invalid key_encoding"), "{e}");
    }

    #[test]
    fn invalid_output_format_errors() {
        let e = cmac("a", KEY128, "", "hex", "rot13", false).unwrap_err();
        assert!(e.contains("invalid output_format"), "{e}");
    }

    #[test]
    fn bad_hex_key_errors() {
        let e = cmac("a", "zz", "", "hex", "", false).unwrap_err();
        assert!(e.contains("key is not valid hex"), "{e}");
    }
}
