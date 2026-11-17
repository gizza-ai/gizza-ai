//! gizza-ai/xsalsa20-cipher core — XSalsa20 stream cipher.
//!
//! XSalsa20 is Salsa20 with a 24-byte nonce. The same operation encrypts and
//! decrypts by XORing the plaintext/ciphertext with the keystream.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use salsa20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use salsa20::XSalsa20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Hex,
    Base64,
}

impl Encoding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "hex" => Ok(Self::Hex),
            "base64" | "b64" => Ok(Self::Base64),
            other => Err(format!("unknown format '{other}' (use hex or base64)")),
        }
    }

    pub fn decode(self, input: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::Hex => {
                let cleaned: String = input.chars().filter(|c| !c.is_ascii_whitespace()).collect();
                hex::decode(cleaned).map_err(|e| format!("invalid hex: {e}"))
            }
            Self::Base64 => B64.decode(input.trim()).map_err(|e| format!("invalid base64: {e}")),
        }
    }

    pub fn encode(self, bytes: &[u8]) -> String {
        match self {
            Self::Hex => hex::encode(bytes),
            Self::Base64 => B64.encode(bytes),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFormat {
    Text,
    Encoded,
}

impl KeyFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => Ok(Self::Text),
            "encoded" | "hex" | "base64" => Ok(Self::Encoded),
            other => Err(format!("unknown key_format '{other}' (use text or encoded)")),
        }
    }
}

fn key_or_nonce_bytes(input: &str, key_format: KeyFormat, encoding: Encoding) -> Result<Vec<u8>, String> {
    match key_format {
        KeyFormat::Text => Ok(input.as_bytes().to_vec()),
        KeyFormat::Encoded => encoding.decode(input),
    }
}

pub fn xsalsa20_apply(key: &[u8], nonce: &[u8], counter: u64, data: &[u8]) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err(format!("XSalsa20 key must be exactly 32 bytes, got {}", key.len()));
    }
    if nonce.len() != 24 {
        return Err(format!("XSalsa20 nonce must be exactly 24 bytes, got {}", nonce.len()));
    }
    let mut buf = data.to_vec();
    let mut cipher = XSalsa20::new(key.into(), nonce.into());
    cipher.seek(counter.saturating_mul(64));
    cipher.apply_keystream(&mut buf);
    Ok(buf)
}

pub fn encrypt(
    plaintext: &str,
    key: &str,
    nonce: &str,
    key_format: KeyFormat,
    counter: u64,
    format: Encoding,
) -> Result<String, String> {
    let key = key_or_nonce_bytes(key, key_format, format)?;
    let nonce = key_or_nonce_bytes(nonce, key_format, format)?;
    let ct = xsalsa20_apply(&key, &nonce, counter, plaintext.as_bytes())?;
    Ok(format.encode(&ct))
}

pub fn decrypt(
    ciphertext: &str,
    key: &str,
    nonce: &str,
    key_format: KeyFormat,
    counter: u64,
    format: Encoding,
) -> Result<String, String> {
    let key = key_or_nonce_bytes(key, key_format, format)?;
    let nonce = key_or_nonce_bytes(nonce, key_format, format)?;
    let ct = format.decode(ciphertext)?;
    let pt = xsalsa20_apply(&key, &nonce, counter, &ct)?;
    String::from_utf8(pt).map_err(|_| "decrypted bytes are not valid UTF-8".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const NONCE_HEX: &str = "000102030405060708090a0b0c0d0e0f1011121314151617";

    #[test]
    fn roundtrip_hex() {
        let msg = "hello xsalsa20";
        let ct = encrypt(msg, KEY_HEX, NONCE_HEX, KeyFormat::Encoded, 0, Encoding::Hex).unwrap();
        assert_ne!(ct, hex::encode(msg));
        let pt = decrypt(&ct, KEY_HEX, NONCE_HEX, KeyFormat::Encoded, 0, Encoding::Hex).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn roundtrip_base64() {
        let key = B64.encode([7u8; 32]);
        let nonce = B64.encode([9u8; 24]);
        let ct = encrypt("secret", &key, &nonce, KeyFormat::Encoded, 1, Encoding::Base64).unwrap();
        let pt = decrypt(&ct, &key, &nonce, KeyFormat::Encoded, 1, Encoding::Base64).unwrap();
        assert_eq!(pt, "secret");
    }

    #[test]
    fn text_key_nonce_lengths() {
        let key = "12345678901234567890123456789012";
        let nonce = "123456789012345678901234";
        let ct = encrypt("abc", key, nonce, KeyFormat::Text, 0, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, key, nonce, KeyFormat::Text, 0, Encoding::Hex).unwrap();
        assert_eq!(pt, "abc");
    }

    #[test]
    fn validates_lengths_and_formats() {
        assert!(xsalsa20_apply(&[0u8; 31], &[0u8; 24], 0, b"x").is_err());
        assert!(xsalsa20_apply(&[0u8; 32], &[0u8; 23], 0, b"x").is_err());
        assert!(Encoding::parse("base32").is_err());
        assert!(KeyFormat::parse("pem").is_err());
    }
}
