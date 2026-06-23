//! gizza-ai/gost-magma-cipher core — encrypt or decrypt text with the GOST
//! 28147-89 / GOST R 34.12-2015 "Magma" 64-bit block cipher, in ECB or CBC mode,
//! with hex/base64 I/O. Pure-Rust (RustCrypto `magma`). No wafer/wasm-bindgen deps.
//!
//! Magma uses a fixed 256-bit (32-byte) key and a 64-bit (8-byte) block. This is a
//! low-level cipher tool: you supply the raw key and IV yourself. The S-box is the
//! id-tc26-gost-28147-param-Z set standardized in GOST R 34.12-2015 (RFC 8891).

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use cipher::block_padding::Pkcs7;
use cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit};
use magma::Magma;

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

const KEY_LEN: usize = 32; // 256-bit Magma key
const BLOCK_LEN: usize = 8; // 64-bit block / IV

fn check_key(key: &[u8]) -> Result<(), String> {
    if key.len() != KEY_LEN {
        return Err(format!("Magma key must be {KEY_LEN} bytes (256-bit), got {}", key.len()));
    }
    Ok(())
}

fn check_iv(iv: &[u8]) -> Result<(), String> {
    if iv.len() != BLOCK_LEN {
        return Err(format!("IV must be {BLOCK_LEN} bytes, got {}", iv.len()));
    }
    Ok(())
}

type MagmaCbcEnc = cbc::Encryptor<Magma>;
type MagmaCbcDec = cbc::Decryptor<Magma>;
type MagmaEcbEnc = ecb::Encryptor<Magma>;
type MagmaEcbDec = ecb::Decryptor<Magma>;

/// Encrypt `plaintext` (UTF-8 text). Returns the ciphertext encoded with `fmt`.
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
            check_iv(&iv)?;
            MagmaCbcEnc::new_from_slices(&key, &iv)
                .map_err(|_| "bad key or iv length".to_string())?
                .encrypt_padded_vec_mut::<Pkcs7>(pt)
        }
        Mode::Ecb => MagmaEcbEnc::new_from_slice(&key)
            .map_err(|_| "bad key length".to_string())?
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
            check_iv(&iv)?;
            MagmaCbcDec::new_from_slices(&key, &iv)
                .map_err(|_| "bad key or iv length".to_string())?
                .decrypt_padded_vec_mut::<Pkcs7>(&ct)
                .map_err(|_| "decryption failed (wrong key/iv or corrupt data)".to_string())?
        }
        Mode::Ecb => MagmaEcbDec::new_from_slice(&key)
            .map_err(|_| "bad key length".to_string())?
            .decrypt_padded_vec_mut::<Pkcs7>(&ct)
            .map_err(|_| "decryption failed (wrong key or corrupt data)".to_string())?,
    };
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 32-byte key and 8-byte IV, hex-encoded.
    const KEY: &str = "ffeeddccbbaa99887766554433221100f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff";
    const IV8: &str = "1234567890abcdef";

    fn enc(hex_str: &str, fmt: Encoding) -> String {
        let bytes = hex::decode(hex_str).unwrap();
        match fmt {
            Encoding::Hex => hex::encode(bytes),
            Encoding::Base64 => B64.encode(bytes),
        }
    }

    fn roundtrip(mode: Mode, fmt: Encoding) {
        let msg = "The quick brown fox 🦊 jumps over the lazy dog!";
        let key = enc(KEY, fmt);
        let iv = if mode == Mode::Ecb { String::new() } else { enc(IV8, fmt) };
        let ct = encrypt(msg, &key, &iv, mode, fmt).unwrap();
        let pt = decrypt(&ct, &key, &iv, mode, fmt).unwrap();
        assert_eq!(pt, msg, "round-trip failed for {mode:?}");
    }

    #[test]
    fn cbc_roundtrip() {
        roundtrip(Mode::Cbc, Encoding::Hex);
        roundtrip(Mode::Cbc, Encoding::Base64);
    }

    #[test]
    fn ecb_roundtrip() {
        roundtrip(Mode::Ecb, Encoding::Hex);
        roundtrip(Mode::Ecb, Encoding::Base64);
    }

    // GOST R 34.12-2015 / RFC 8891 Magma single-block test vector.
    // Key = ffeeddcc…fcfdfeff, plaintext block = fedcba9876543210
    //   → ciphertext block = 4ee901e5c2d8ca3d.
    #[test]
    fn rfc8891_ecb_block_vector() {
        let ct = encrypt_raw_block(KEY, "fedcba9876543210");
        assert_eq!(ct, "4ee901e5c2d8ca3d");
    }

    fn encrypt_raw_block(key_hex: &str, block_hex: &str) -> String {
        use cipher::generic_array::GenericArray;
        use cipher::{BlockEncrypt, KeyInit};
        let key = hex::decode(key_hex).unwrap();
        let mut block = GenericArray::clone_from_slice(&hex::decode(block_hex).unwrap());
        let c = Magma::new_from_slice(&key).unwrap();
        c.encrypt_block(&mut block);
        hex::encode(block)
    }

    #[test]
    fn wrong_key_fails_cbc() {
        let ct = encrypt("hello magma test", KEY, IV8, Mode::Cbc, Encoding::Hex).unwrap();
        let wrong = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        assert!(decrypt(&ct, wrong, IV8, Mode::Cbc, Encoding::Hex).is_err());
    }

    #[test]
    fn errors() {
        assert!(encrypt("x", "abcd", IV8, Mode::Cbc, Encoding::Hex).is_err()); // bad key len
        assert!(encrypt("x", KEY, "", Mode::Cbc, Encoding::Hex).is_err()); // missing iv
        assert!(encrypt("x", KEY, "abcd", Mode::Cbc, Encoding::Hex).is_err()); // bad iv len
        assert!(Mode::parse("gcm").is_err());
        assert!(Encoding::parse("octal").is_err());
    }
}
