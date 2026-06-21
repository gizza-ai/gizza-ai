//! gizza-ai/aes-cipher core — encrypt or decrypt text with AES in CBC, CTR, GCM,
//! or ECB mode, across 128/192/256-bit keys (chosen by the key length), with
//! hex/base64 I/O. Pure-Rust (RustCrypto). No wafer/wasm-bindgen deps.
//!
//! This is a low-level cipher tool: you supply the raw key and IV/nonce yourself.
//! For passphrase-based encryption with a safe random salt+nonce, use
//! `text-encrypt` / `encrypt-file` instead.

use aes::{Aes128, Aes192, Aes256};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use cipher::block_padding::Pkcs7;
use cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit, StreamCipher};

/// Cipher mode of operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Cbc,
    Ctr,
    Gcm,
    Ecb,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "cbc" | "" => Ok(Mode::Cbc),
            "ctr" => Ok(Mode::Ctr),
            "gcm" => Ok(Mode::Gcm),
            "ecb" => Ok(Mode::Ecb),
            other => Err(format!("unknown mode '{other}' (use cbc, ctr, gcm, or ecb)")),
        }
    }
    fn needs_iv(self) -> bool {
        !matches!(self, Mode::Ecb)
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

// --- CBC / ECB (block, PKCS7) and CTR (stream) via macros over key size ---

fn cbc_encrypt(key: &[u8], iv: &[u8], pt: &[u8]) -> Result<Vec<u8>, String> {
    macro_rules! go {
        ($a:ty) => {{
            let c = cbc::Encryptor::<$a>::new_from_slices(key, iv)
                .map_err(|_| "bad key or iv length".to_string())?;
            Ok(c.encrypt_padded_vec_mut::<Pkcs7>(pt))
        }};
    }
    match key.len() {
        16 => go!(Aes128),
        24 => go!(Aes192),
        32 => go!(Aes256),
        n => Err(format!("AES key must be 16/24/32 bytes, got {n}")),
    }
}

fn cbc_decrypt(key: &[u8], iv: &[u8], ct: &[u8]) -> Result<Vec<u8>, String> {
    macro_rules! go {
        ($a:ty) => {{
            let c = cbc::Decryptor::<$a>::new_from_slices(key, iv)
                .map_err(|_| "bad key or iv length".to_string())?;
            c.decrypt_padded_vec_mut::<Pkcs7>(ct)
                .map_err(|_| "decryption failed (wrong key/iv or corrupt data)".to_string())
        }};
    }
    match key.len() {
        16 => go!(Aes128),
        24 => go!(Aes192),
        32 => go!(Aes256),
        n => Err(format!("AES key must be 16/24/32 bytes, got {n}")),
    }
}

fn ecb_encrypt(key: &[u8], pt: &[u8]) -> Result<Vec<u8>, String> {
    macro_rules! go {
        ($a:ty) => {{
            let c = ecb::Encryptor::<$a>::new_from_slice(key)
                .map_err(|_| "bad key length".to_string())?;
            Ok(c.encrypt_padded_vec_mut::<Pkcs7>(pt))
        }};
    }
    match key.len() {
        16 => go!(Aes128),
        24 => go!(Aes192),
        32 => go!(Aes256),
        n => Err(format!("AES key must be 16/24/32 bytes, got {n}")),
    }
}

fn ecb_decrypt(key: &[u8], ct: &[u8]) -> Result<Vec<u8>, String> {
    macro_rules! go {
        ($a:ty) => {{
            let c = ecb::Decryptor::<$a>::new_from_slice(key)
                .map_err(|_| "bad key length".to_string())?;
            c.decrypt_padded_vec_mut::<Pkcs7>(ct)
                .map_err(|_| "decryption failed (wrong key or corrupt data)".to_string())
        }};
    }
    match key.len() {
        16 => go!(Aes128),
        24 => go!(Aes192),
        32 => go!(Aes256),
        n => Err(format!("AES key must be 16/24/32 bytes, got {n}")),
    }
}

fn ctr_apply(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    // CTR is symmetric: the same operation encrypts and decrypts.
    macro_rules! go {
        ($a:ty) => {{
            let mut c = ctr::Ctr128BE::<$a>::new_from_slices(key, iv)
                .map_err(|_| "bad key or iv length".to_string())?;
            let mut buf = data.to_vec();
            c.apply_keystream(&mut buf);
            Ok(buf)
        }};
    }
    match key.len() {
        16 => go!(Aes128),
        24 => go!(Aes192),
        32 => go!(Aes256),
        n => Err(format!("AES key must be 16/24/32 bytes, got {n}")),
    }
}

fn gcm_encrypt(key: &[u8], nonce: &[u8], pt: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::Aead;
    use aes_gcm::{AesGcm, Nonce};
    use cipher::consts::U12;
    if nonce.len() != 12 {
        return Err("GCM nonce must be 12 bytes".into());
    }
    let n = Nonce::from_slice(nonce);
    macro_rules! go {
        ($a:ty) => {{
            let c = <AesGcm<$a, U12> as aes_gcm::KeyInit>::new_from_slice(key)
                .map_err(|_| "bad key length".to_string())?;
            c.encrypt(n, pt).map_err(|_| "encryption failed".to_string())
        }};
    }
    match key.len() {
        16 => go!(Aes128),
        24 => go!(Aes192),
        32 => go!(Aes256),
        n => Err(format!("AES key must be 16/24/32 bytes, got {n}")),
    }
}

fn gcm_decrypt(key: &[u8], nonce: &[u8], ct: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::Aead;
    use aes_gcm::{AesGcm, Nonce};
    use cipher::consts::U12;
    if nonce.len() != 12 {
        return Err("GCM nonce must be 12 bytes".into());
    }
    let n = Nonce::from_slice(nonce);
    macro_rules! go {
        ($a:ty) => {{
            let c = <AesGcm<$a, U12> as aes_gcm::KeyInit>::new_from_slice(key)
                .map_err(|_| "bad key length".to_string())?;
            c.decrypt(n, ct)
                .map_err(|_| "decryption failed (wrong key/nonce or tampered data)".to_string())
        }};
    }
    match key.len() {
        16 => go!(Aes128),
        24 => go!(Aes192),
        32 => go!(Aes256),
        n => Err(format!("AES key must be 16/24/32 bytes, got {n}")),
    }
}

/// Encrypt `plaintext` (UTF-8 text). Returns the ciphertext encoded with `fmt`.
/// For GCM the 16-byte authentication tag is appended to the ciphertext.
pub fn encrypt(
    plaintext: &str,
    key_str: &str,
    iv_str: &str,
    mode: Mode,
    fmt: Encoding,
) -> Result<String, String> {
    let key = fmt.decode(key_str)?;
    let iv = if mode.needs_iv() {
        let iv = fmt.decode(iv_str)?;
        if iv.is_empty() {
            return Err("this mode requires an iv/nonce".into());
        }
        iv
    } else {
        Vec::new()
    };
    let pt = plaintext.as_bytes();
    let ct = match mode {
        Mode::Cbc => cbc_encrypt(&key, &iv, pt)?,
        Mode::Ctr => ctr_apply(&key, &iv, pt)?,
        Mode::Ecb => ecb_encrypt(&key, pt)?,
        Mode::Gcm => gcm_encrypt(&key, &iv, pt)?,
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
    let iv = if mode.needs_iv() {
        let iv = fmt.decode(iv_str)?;
        if iv.is_empty() {
            return Err("this mode requires an iv/nonce".into());
        }
        iv
    } else {
        Vec::new()
    };
    let ct = fmt.decode(ciphertext)?;
    let pt = match mode {
        Mode::Cbc => cbc_decrypt(&key, &iv, &ct)?,
        Mode::Ctr => ctr_apply(&key, &iv, &ct)?,
        Mode::Ecb => ecb_decrypt(&key, &ct)?,
        Mode::Gcm => gcm_decrypt(&key, &iv, &ct)?,
    };
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 16/24/32-byte keys and a 16-byte IV / 12-byte nonce, hex-encoded.
    const K128: &str = "000102030405060708090a0b0c0d0e0f";
    const K192: &str = "000102030405060708090a0b0c0d0e0f1011121314151617";
    const K256: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const IV16: &str = "0f0e0d0c0b0a09080706050403020100";
    const NONCE12: &str = "0f0e0d0c0b0a090807060504";

    fn roundtrip(mode: Mode, key: &str, iv: &str) {
        let msg = "The quick brown fox 🦊 jumps!";
        let ct = encrypt(msg, key, iv, mode, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, key, iv, mode, Encoding::Hex).unwrap();
        assert_eq!(pt, msg, "round-trip failed for {mode:?}");
    }

    #[test]
    fn cbc_roundtrip_all_key_sizes() {
        roundtrip(Mode::Cbc, K128, IV16);
        roundtrip(Mode::Cbc, K192, IV16);
        roundtrip(Mode::Cbc, K256, IV16);
    }

    #[test]
    fn ctr_roundtrip() {
        roundtrip(Mode::Ctr, K128, IV16);
        roundtrip(Mode::Ctr, K256, IV16);
    }

    #[test]
    fn gcm_roundtrip_all_key_sizes() {
        roundtrip(Mode::Gcm, K128, NONCE12);
        roundtrip(Mode::Gcm, K192, NONCE12);
        roundtrip(Mode::Gcm, K256, NONCE12);
    }

    #[test]
    fn ecb_roundtrip() {
        let msg = "block aligned or not";
        let ct = encrypt(msg, K128, "", Mode::Ecb, Encoding::Base64).unwrap();
        let pt = decrypt(&ct, K128, "", Mode::Ecb, Encoding::Base64).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn gcm_detects_tampering() {
        let ct = encrypt("secret", K128, NONCE12, Mode::Gcm, Encoding::Hex).unwrap();
        // Flip a hex nibble.
        let mut bytes = hex::decode(&ct).unwrap();
        bytes[0] ^= 0xff;
        let tampered = hex::encode(bytes);
        assert!(decrypt(&tampered, K128, NONCE12, Mode::Gcm, Encoding::Hex).is_err());
    }

    #[test]
    fn wrong_key_fails_cbc() {
        let ct = encrypt("hello world test", K128, IV16, Mode::Cbc, Encoding::Hex).unwrap();
        let wrong = "ffffffffffffffffffffffffffffffff";
        // CBC with a wrong key usually fails PKCS7 unpadding.
        assert!(decrypt(&ct, wrong, IV16, Mode::Cbc, Encoding::Hex).is_err());
    }

    #[test]
    fn errors() {
        assert!(encrypt("x", "abcd", IV16, Mode::Cbc, Encoding::Hex).is_err()); // bad key len
        assert!(encrypt("x", K128, "", Mode::Cbc, Encoding::Hex).is_err()); // missing iv
        assert!(Mode::parse("xts").is_err());
        assert!(Encoding::parse("octal").is_err());
    }
}
