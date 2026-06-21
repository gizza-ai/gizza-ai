//! gizza-ai/des-cipher core — encrypt or decrypt data with single DES in ECB or
//! CBC mode, with hex/base64 I/O. Pure-Rust (RustCrypto `des` + `cbc`/`ecb`).
//!
//! DES is a legacy 56-bit cipher and is NOT secure — this tool exists for interop
//! with old systems and for decrypting legacy data. For real encryption use
//! `aes-cipher` or the passphrase tools.

use cipher::block_padding::Pkcs7;
use cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit};
use des::Des;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// Mode of operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Cbc,
    Ecb,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "cbc" | "" => Ok(Mode::Cbc),
            "ecb" => Ok(Mode::Ecb),
            other => Err(format!("unknown mode '{other}' (use cbc or ecb)")),
        }
    }
}

/// Binary encoding for key / iv / ciphertext.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Hex,
    Base64,
}

impl Encoding {
    pub fn parse(s: &str) -> Result<Encoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "base64" | "b64" | "" => Ok(Encoding::Base64),
            "hex" => Ok(Encoding::Hex),
            other => Err(format!("unknown format '{other}' (use hex or base64)")),
        }
    }
    fn decode(self, s: &str) -> Result<Vec<u8>, String> {
        let s = s.trim();
        match self {
            Encoding::Hex => hex::decode(s).map_err(|e| format!("invalid hex: {e}")),
            Encoding::Base64 => B64.decode(s).map_err(|e| format!("invalid base64: {e}")),
        }
    }
    fn encode(self, b: &[u8]) -> String {
        match self {
            Encoding::Hex => hex::encode(b),
            Encoding::Base64 => B64.encode(b),
        }
    }
}

type DesCbcEnc = cbc::Encryptor<Des>;
type DesCbcDec = cbc::Decryptor<Des>;
type DesEcbEnc = ecb::Encryptor<Des>;
type DesEcbDec = ecb::Decryptor<Des>;

/// Encrypt `plaintext` (UTF-8). Returns the ciphertext encoded with `fmt`.
pub fn encrypt(plaintext: &str, key_str: &str, iv_str: &str, mode: Mode, fmt: Encoding) -> Result<String, String> {
    let key = fmt.decode(key_str)?;
    if key.len() != 8 {
        return Err(format!("DES key must be 8 bytes, got {}", key.len()));
    }
    let pt = plaintext.as_bytes();
    let ct = match mode {
        Mode::Cbc => {
            let iv = fmt.decode(iv_str)?;
            if iv.len() != 8 {
                return Err("CBC needs an 8-byte iv".into());
            }
            DesCbcEnc::new_from_slices(&key, &iv)
                .map_err(|_| "bad key or iv".to_string())?
                .encrypt_padded_vec_mut::<Pkcs7>(pt)
        }
        Mode::Ecb => DesEcbEnc::new_from_slice(&key)
            .map_err(|_| "bad key".to_string())?
            .encrypt_padded_vec_mut::<Pkcs7>(pt),
    };
    Ok(fmt.encode(&ct))
}

/// Decrypt `ciphertext` (encoded with `fmt`). Returns the recovered UTF-8 text.
pub fn decrypt(ciphertext: &str, key_str: &str, iv_str: &str, mode: Mode, fmt: Encoding) -> Result<String, String> {
    let key = fmt.decode(key_str)?;
    if key.len() != 8 {
        return Err(format!("DES key must be 8 bytes, got {}", key.len()));
    }
    let ct = fmt.decode(ciphertext)?;
    let pt = match mode {
        Mode::Cbc => {
            let iv = fmt.decode(iv_str)?;
            if iv.len() != 8 {
                return Err("CBC needs an 8-byte iv".into());
            }
            DesCbcDec::new_from_slices(&key, &iv)
                .map_err(|_| "bad key or iv".to_string())?
                .decrypt_padded_vec_mut::<Pkcs7>(&ct)
                .map_err(|_| "decryption failed (wrong key/iv or corrupt data)".to_string())?
        }
        Mode::Ecb => DesEcbDec::new_from_slice(&key)
            .map_err(|_| "bad key".to_string())?
            .decrypt_padded_vec_mut::<Pkcs7>(&ct)
            .map_err(|_| "decryption failed (wrong key or corrupt data)".to_string())?,
    };
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "0123456789abcdef"; // 8 bytes hex
    const IV: &str = "fedcba9876543210";

    #[test]
    fn cbc_roundtrip() {
        let ct = encrypt("hello DES 🔓", KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        assert_eq!(pt, "hello DES 🔓");
    }

    #[test]
    fn ecb_roundtrip() {
        // base64 "MTIzNDU2Nzg=" decodes to the 8-byte key "12345678".
        let k = "MTIzNDU2Nzg=";
        let ct = encrypt("block data here", k, "", Mode::Ecb, Encoding::Base64).unwrap();
        let pt = decrypt(&ct, k, "", Mode::Ecb, Encoding::Base64).unwrap();
        assert_eq!(pt, "block data here");
    }

    #[test]
    fn wrong_key_fails() {
        let ct = encrypt("secret message", KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        assert!(decrypt(&ct, "ffffffffffffffff", IV, Mode::Cbc, Encoding::Hex).is_err());
    }

    #[test]
    fn errors() {
        assert!(encrypt("x", "abcd", IV, Mode::Cbc, Encoding::Hex).is_err()); // key not 8 bytes
        assert!(encrypt("x", KEY, "", Mode::Cbc, Encoding::Hex).is_err()); // missing iv
        assert!(Mode::parse("gcm").is_err());
        assert!(Encoding::parse("octal").is_err());
    }
}
