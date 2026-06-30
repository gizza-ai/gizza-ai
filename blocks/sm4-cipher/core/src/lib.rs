//! gizza-ai/sm4-cipher core — encrypt or decrypt data with the SM4 block cipher
//! in ECB or CBC mode, with hex/base64 I/O. Pure-Rust (RustCrypto `sm4` +
//! `cbc`/`ecb`).
//!
//! SM4 is the Chinese national standard block cipher (GB/T 32907-2016, also
//! GM/T 0002 / ISO/IEC 18033-3:2010 amendment). It uses a 128-bit (16-byte) key
//! and operates on 128-bit (16-byte) blocks. CBC needs a 16-byte IV.

use cipher::block_padding::Pkcs7;
use cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// SM4 with the standard key schedule.
type Sm4 = sm4::Sm4;

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

type Sm4CbcEnc = cbc::Encryptor<Sm4>;
type Sm4CbcDec = cbc::Decryptor<Sm4>;
type Sm4EcbEnc = ecb::Encryptor<Sm4>;
type Sm4EcbDec = ecb::Decryptor<Sm4>;

/// SM4 uses a fixed 128-bit (16-byte) key.
fn check_key(key: &[u8]) -> Result<(), String> {
    if key.len() != 16 {
        return Err(format!(
            "SM4 key must be exactly 16 bytes (128 bits), got {}",
            key.len()
        ));
    }
    Ok(())
}

/// Encrypt `plaintext` (UTF-8). Returns the ciphertext encoded with `fmt`.
pub fn encrypt(
    plaintext: &str,
    key_str: &str,
    iv_str: &str,
    mode: Mode,
    fmt: Encoding,
) -> Result<String, String> {
    let key = fmt.decode(key_str)?;
    check_key(&key)?;
    let pt = plaintext.as_bytes();
    let ct = match mode {
        Mode::Cbc => {
            let iv = fmt.decode(iv_str)?;
            if iv.len() != 16 {
                return Err("CBC needs a 16-byte iv".into());
            }
            Sm4CbcEnc::new_from_slices(&key, &iv)
                .map_err(|_| "bad key or iv".to_string())?
                .encrypt_padded_vec_mut::<Pkcs7>(pt)
        }
        Mode::Ecb => Sm4EcbEnc::new_from_slice(&key)
            .map_err(|_| "bad key".to_string())?
            .encrypt_padded_vec_mut::<Pkcs7>(pt),
    };
    Ok(fmt.encode(&ct))
}

/// Decrypt `ciphertext` (encoded with `fmt`). Returns the recovered UTF-8 text.
pub fn decrypt(
    ciphertext: &str,
    key_str: &str,
    iv_str: &str,
    mode: Mode,
    fmt: Encoding,
) -> Result<String, String> {
    let key = fmt.decode(key_str)?;
    check_key(&key)?;
    let ct = fmt.decode(ciphertext)?;
    let pt = match mode {
        Mode::Cbc => {
            let iv = fmt.decode(iv_str)?;
            if iv.len() != 16 {
                return Err("CBC needs a 16-byte iv".into());
            }
            Sm4CbcDec::new_from_slices(&key, &iv)
                .map_err(|_| "bad key or iv".to_string())?
                .decrypt_padded_vec_mut::<Pkcs7>(&ct)
                .map_err(|_| "decryption failed (wrong key/iv or corrupt data)".to_string())?
        }
        Mode::Ecb => Sm4EcbDec::new_from_slice(&key)
            .map_err(|_| "bad key".to_string())?
            .decrypt_padded_vec_mut::<Pkcs7>(&ct)
            .map_err(|_| "decryption failed (wrong key or corrupt data)".to_string())?,
    };
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "0123456789abcdeffedcba9876543210"; // 16 bytes hex
    const IV: &str = "00112233445566778899aabbccddeeff"; // 16 bytes hex

    #[test]
    fn cbc_roundtrip() {
        let ct = encrypt("hello SM4 国密 🔐", KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        assert_eq!(pt, "hello SM4 国密 🔐");
    }

    #[test]
    fn ecb_roundtrip() {
        // base64 "MDEyMzQ1Njc4OWFiY2RlZg==" decodes to the 16-byte key "0123456789abcdef".
        let k = "MDEyMzQ1Njc4OWFiY2RlZg==";
        let ct = encrypt("block data here", k, "", Mode::Ecb, Encoding::Base64).unwrap();
        let pt = decrypt(&ct, k, "", Mode::Ecb, Encoding::Base64).unwrap();
        assert_eq!(pt, "block data here");
    }

    #[test]
    fn known_answer_vector() {
        // GB/T 32907-2016 SM4 single-block test vector:
        // key = pt = 0123456789abcdeffedcba9876543210
        //   → ct = 681edf34d206965e86b3e94f536e4246.
        use cipher::generic_array::GenericArray;
        use cipher::{BlockEncrypt, KeyInit as _};
        let key = hex::decode("0123456789abcdeffedcba9876543210").unwrap();
        let cipher = Sm4::new_from_slice(&key).unwrap();
        let mut block =
            GenericArray::clone_from_slice(&hex::decode("0123456789abcdeffedcba9876543210").unwrap());
        cipher.encrypt_block(&mut block);
        assert_eq!(hex::encode(block), "681edf34d206965e86b3e94f536e4246");
    }

    #[test]
    fn wrong_key_fails() {
        let ct = encrypt("secret message", KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        let bad = "ffffffffffffffffffffffffffffffff";
        assert!(decrypt(&ct, bad, IV, Mode::Cbc, Encoding::Hex).is_err());
    }

    #[test]
    fn errors() {
        assert!(encrypt("x", "abcd", IV, Mode::Cbc, Encoding::Hex).is_err()); // key not 16 bytes
        assert!(encrypt("x", KEY, "", Mode::Cbc, Encoding::Hex).is_err()); // missing iv
        assert!(encrypt("x", KEY, "0011", Mode::Cbc, Encoding::Hex).is_err()); // iv not 16 bytes
        assert!(Mode::parse("gcm").is_err());
        assert!(Encoding::parse("octal").is_err());
    }
}
