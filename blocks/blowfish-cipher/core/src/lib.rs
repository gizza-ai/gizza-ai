//! gizza-ai/blowfish-cipher core — encrypt or decrypt data with Blowfish in ECB
//! or CBC mode, with hex/base64 I/O. Pure-Rust (RustCrypto `blowfish` +
//! `cbc`/`ecb`).
//!
//! Blowfish is a 64-bit-block cipher with a variable key length (4–56 bytes).
//! It is a legacy algorithm: its small block size makes it vulnerable to
//! birthday/Sweet32 attacks on large data — for new designs prefer AES
//! (`aes-cipher`). This tool exists for interop with old systems and for
//! decrypting legacy data.

use cipher::block_padding::Pkcs7;
use cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// Blowfish with the standard big-endian key schedule.
type Blowfish = blowfish::Blowfish;

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

type BfCbcEnc = cbc::Encryptor<Blowfish>;
type BfCbcDec = cbc::Decryptor<Blowfish>;
type BfEcbEnc = ecb::Encryptor<Blowfish>;
type BfEcbDec = ecb::Decryptor<Blowfish>;

/// Blowfish accepts keys from 4 to 56 bytes (32–448 bits).
fn check_key(key: &[u8]) -> Result<(), String> {
    if key.len() < 4 || key.len() > 56 {
        return Err(format!(
            "Blowfish key must be 4–56 bytes (32–448 bits), got {}",
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
            if iv.len() != 8 {
                return Err("CBC needs an 8-byte iv".into());
            }
            BfCbcEnc::new_from_slices(&key, &iv)
                .map_err(|_| "bad key or iv".to_string())?
                .encrypt_padded_vec_mut::<Pkcs7>(pt)
        }
        Mode::Ecb => BfEcbEnc::new_from_slice(&key)
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
            if iv.len() != 8 {
                return Err("CBC needs an 8-byte iv".into());
            }
            BfCbcDec::new_from_slices(&key, &iv)
                .map_err(|_| "bad key or iv".to_string())?
                .decrypt_padded_vec_mut::<Pkcs7>(&ct)
                .map_err(|_| "decryption failed (wrong key/iv or corrupt data)".to_string())?
        }
        Mode::Ecb => BfEcbDec::new_from_slice(&key)
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
        let ct = encrypt("hello Blowfish 🐡", KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        assert_eq!(pt, "hello Blowfish 🐡");
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
    fn variable_key_length() {
        // 4-byte (minimum) and 56-byte (maximum) keys both work.
        let short = "01020304"; // 4 bytes hex
        let ct = encrypt("short key ok", short, IV, Mode::Cbc, Encoding::Hex).unwrap();
        assert_eq!(
            decrypt(&ct, short, IV, Mode::Cbc, Encoding::Hex).unwrap(),
            "short key ok"
        );

        let long = "ab".repeat(56); // 56 bytes hex
        let ct = encrypt("long key ok", &long, IV, Mode::Cbc, Encoding::Hex).unwrap();
        assert_eq!(
            decrypt(&ct, &long, IV, Mode::Cbc, Encoding::Hex).unwrap(),
            "long key ok"
        );
    }

    #[test]
    fn known_answer_vector() {
        // Schneier's Blowfish ECB test vector: key=0000000000000000,
        // pt=0000000000000000 → ct=4EF997456198DD78.
        use cipher::generic_array::GenericArray;
        use cipher::{BlockEncrypt, KeyInit as _};
        let key = [0u8; 8];
        let cipher = Blowfish::new_from_slice(&key).unwrap();
        let mut block = GenericArray::clone_from_slice(&[0u8; 8]);
        cipher.encrypt_block(&mut block);
        assert_eq!(hex::encode(block), "4ef997456198dd78");
    }

    #[test]
    fn wrong_key_fails() {
        let ct = encrypt("secret message", KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        assert!(decrypt(&ct, "ffffffffffffffff", IV, Mode::Cbc, Encoding::Hex).is_err());
    }

    #[test]
    fn errors() {
        assert!(encrypt("x", "ab", IV, Mode::Cbc, Encoding::Hex).is_err()); // key < 4 bytes (2)
        assert!(encrypt("x", KEY, "", Mode::Cbc, Encoding::Hex).is_err()); // missing iv
        assert!(Mode::parse("gcm").is_err());
        assert!(Encoding::parse("octal").is_err());
    }
}
