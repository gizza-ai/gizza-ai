//! gizza-ai/gost-kuznyechik-cipher core — encrypt or decrypt text with the GOST R
//! 34.12-2015 "Kuznyechik" 128-bit block cipher (RFC 7801), in CBC, CTR or ECB
//! mode, with hex/base64 I/O. Pure-Rust (RustCrypto). No wafer/wasm-bindgen deps.
//!
//! Kuznyechik uses a fixed 256-bit (32-byte) key and a 128-bit (16-byte) block.
//! This is a low-level cipher tool: you supply the raw key and IV/nonce yourself.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use cipher::block_padding::Pkcs7;
use cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit, StreamCipher};
use kuznyechik::Kuznyechik;

/// Mode of operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Cbc,
    Ctr,
    Cfb,
    Ofb,
    Ecb,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "cbc" | "" => Ok(Mode::Cbc),
            "ctr" => Ok(Mode::Ctr),
            "cfb" => Ok(Mode::Cfb),
            "ofb" => Ok(Mode::Ofb),
            "ecb" => Ok(Mode::Ecb),
            other => Err(format!("unknown mode '{other}' (use cbc, ctr, cfb, ofb, or ecb)")),
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

const KEY_LEN: usize = 32; // 256-bit Kuznyechik key
const BLOCK_LEN: usize = 16; // 128-bit block / IV

fn check_key(key: &[u8]) -> Result<(), String> {
    if key.len() != KEY_LEN {
        return Err(format!("Kuznyechik key must be {KEY_LEN} bytes (256-bit), got {}", key.len()));
    }
    Ok(())
}

fn check_iv(iv: &[u8]) -> Result<(), String> {
    if iv.len() != BLOCK_LEN {
        return Err(format!("IV/nonce must be {BLOCK_LEN} bytes, got {}", iv.len()));
    }
    Ok(())
}

fn cbc_encrypt(key: &[u8], iv: &[u8], pt: &[u8]) -> Result<Vec<u8>, String> {
    let c = cbc::Encryptor::<Kuznyechik>::new_from_slices(key, iv)
        .map_err(|_| "bad key or iv length".to_string())?;
    Ok(c.encrypt_padded_vec_mut::<Pkcs7>(pt))
}

fn cbc_decrypt(key: &[u8], iv: &[u8], ct: &[u8]) -> Result<Vec<u8>, String> {
    let c = cbc::Decryptor::<Kuznyechik>::new_from_slices(key, iv)
        .map_err(|_| "bad key or iv length".to_string())?;
    c.decrypt_padded_vec_mut::<Pkcs7>(ct)
        .map_err(|_| "decryption failed (wrong key/iv or corrupt data)".to_string())
}

fn ecb_encrypt(key: &[u8], pt: &[u8]) -> Result<Vec<u8>, String> {
    let c = ecb::Encryptor::<Kuznyechik>::new_from_slice(key)
        .map_err(|_| "bad key length".to_string())?;
    Ok(c.encrypt_padded_vec_mut::<Pkcs7>(pt))
}

fn ecb_decrypt(key: &[u8], ct: &[u8]) -> Result<Vec<u8>, String> {
    let c = ecb::Decryptor::<Kuznyechik>::new_from_slice(key)
        .map_err(|_| "bad key length".to_string())?;
    c.decrypt_padded_vec_mut::<Pkcs7>(ct)
        .map_err(|_| "decryption failed (wrong key or corrupt data)".to_string())
}

fn ctr_apply(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    // CTR is symmetric: the same operation encrypts and decrypts.
    let mut c = ctr::Ctr128BE::<Kuznyechik>::new_from_slices(key, iv)
        .map_err(|_| "bad key or iv length".to_string())?;
    let mut buf = data.to_vec();
    c.apply_keystream(&mut buf);
    Ok(buf)
}

fn ofb_apply(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    // OFB is symmetric: the same operation encrypts and decrypts.
    let mut c = ofb::Ofb::<Kuznyechik>::new_from_slices(key, iv)
        .map_err(|_| "bad key or iv length".to_string())?;
    let mut buf = data.to_vec();
    c.apply_keystream(&mut buf);
    Ok(buf)
}

fn cfb_encrypt(key: &[u8], iv: &[u8], pt: &[u8]) -> Result<Vec<u8>, String> {
    use cipher::AsyncStreamCipher;
    let c = cfb_mode::Encryptor::<Kuznyechik>::new_from_slices(key, iv)
        .map_err(|_| "bad key or iv length".to_string())?;
    let mut buf = pt.to_vec();
    c.encrypt(&mut buf);
    Ok(buf)
}

fn cfb_decrypt(key: &[u8], iv: &[u8], ct: &[u8]) -> Result<Vec<u8>, String> {
    use cipher::AsyncStreamCipher;
    let c = cfb_mode::Decryptor::<Kuznyechik>::new_from_slices(key, iv)
        .map_err(|_| "bad key or iv length".to_string())?;
    let mut buf = ct.to_vec();
    c.decrypt(&mut buf);
    Ok(buf)
}

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
    let iv = if mode.needs_iv() {
        let iv = fmt.decode(iv_str)?;
        check_iv(&iv)?;
        iv
    } else {
        Vec::new()
    };
    let pt = plaintext.as_bytes();
    let ct = match mode {
        Mode::Cbc => cbc_encrypt(&key, &iv, pt)?,
        Mode::Ctr => ctr_apply(&key, &iv, pt)?,
        Mode::Cfb => cfb_encrypt(&key, &iv, pt)?,
        Mode::Ofb => ofb_apply(&key, &iv, pt)?,
        Mode::Ecb => ecb_encrypt(&key, pt)?,
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
    let iv = if mode.needs_iv() {
        let iv = fmt.decode(iv_str)?;
        check_iv(&iv)?;
        iv
    } else {
        Vec::new()
    };
    let ct = fmt.decode(ciphertext)?;
    let pt = match mode {
        Mode::Cbc => cbc_decrypt(&key, &iv, &ct)?,
        Mode::Ctr => ctr_apply(&key, &iv, &ct)?,
        Mode::Cfb => cfb_decrypt(&key, &iv, &ct)?,
        Mode::Ofb => ofb_apply(&key, &iv, &ct)?,
        Mode::Ecb => ecb_decrypt(&key, &ct)?,
    };
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 32-byte key and 16-byte IV, hex-encoded.
    const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const IV16: &str = "0f0e0d0c0b0a09080706050403020100";

    // Re-encode a hex string into the test format so keys are the right length.
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
        let iv = if mode == Mode::Ecb { String::new() } else { enc(IV16, fmt) };
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
    fn ctr_roundtrip() {
        roundtrip(Mode::Ctr, Encoding::Hex);
        roundtrip(Mode::Ctr, Encoding::Base64);
    }

    #[test]
    fn cfb_roundtrip() {
        roundtrip(Mode::Cfb, Encoding::Hex);
        roundtrip(Mode::Cfb, Encoding::Base64);
    }

    #[test]
    fn ofb_roundtrip() {
        roundtrip(Mode::Ofb, Encoding::Hex);
        roundtrip(Mode::Ofb, Encoding::Base64);
    }

    #[test]
    fn ecb_roundtrip() {
        roundtrip(Mode::Ecb, Encoding::Hex);
        roundtrip(Mode::Ecb, Encoding::Base64);
    }

    // RFC 7801 / GOST R 34.13-2015 Appendix A.1 ECB test vector for a single block.
    // Key A.1, plaintext block 1 → ciphertext block 1.
    #[test]
    fn rfc7801_ecb_block_vector() {
        let key = "8899aabbccddeeff0011223344556677fedcba98765432100123456789abcdef";
        let pt_block = "1122334455667700ffeeddccbbaa9988"; // 16-byte hex, exactly one block
        // We can't get the bare block out of PKCS7 ECB (it pads to 2 blocks),
        // so verify the first 16 bytes of the ciphertext equal the RFC vector.
        let ct = encrypt_raw_block(key, pt_block);
        assert_eq!(ct, "7f679d90bebc24305a468d42b9d4edcd");
    }

    // Helper: ECB-encrypt exactly one raw block (no padding) for the test vector.
    fn encrypt_raw_block(key_hex: &str, block_hex: &str) -> String {
        use cipher::generic_array::GenericArray;
        use cipher::{BlockEncrypt, KeyInit};
        let key = hex::decode(key_hex).unwrap();
        let mut block = GenericArray::clone_from_slice(&hex::decode(block_hex).unwrap());
        let c = Kuznyechik::new_from_slice(&key).unwrap();
        c.encrypt_block(&mut block);
        hex::encode(block)
    }

    #[test]
    fn wrong_key_fails_cbc() {
        let ct = encrypt("hello world test", KEY, IV16, Mode::Cbc, Encoding::Hex).unwrap();
        let wrong = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        assert!(decrypt(&ct, wrong, IV16, Mode::Cbc, Encoding::Hex).is_err());
    }

    #[test]
    fn errors() {
        assert!(encrypt("x", "abcd", IV16, Mode::Cbc, Encoding::Hex).is_err()); // bad key len
        assert!(encrypt("x", KEY, "", Mode::Cbc, Encoding::Hex).is_err()); // missing iv
        assert!(encrypt("x", KEY, "abcd", Mode::Ctr, Encoding::Hex).is_err()); // bad iv len
        assert!(Mode::parse("gcm").is_err());
        assert!(Encoding::parse("octal").is_err());
    }
}
