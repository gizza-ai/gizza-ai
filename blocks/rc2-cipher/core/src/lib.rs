//! gizza-ai/rc2-cipher core — encrypt or decrypt data with the RC2 block cipher
//! (RFC 2268) at a configurable effective key length, in ECB or CBC mode, with
//! hex/base64 I/O. Pure-Rust (RustCrypto `rc2`); ECB/CBC chaining and PKCS#7
//! padding are implemented here so the effective-key-length parameter can be
//! passed straight through to `Rc2::new_with_eff_key_len`.
//!
//! RC2 is a 64-bit-block cipher with a variable key (1–128 bytes) and a separate
//! "effective key bits" (T1) parameter. It is a legacy cipher (used by old PKCS#12,
//! S/MIME and Microsoft formats) and is NOT considered secure for new designs — use
//! it only for interop with old systems or to decrypt legacy data. For real
//! encryption use `aes-cipher` or the passphrase tools.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rc2::cipher::array::Array;
use rc2::cipher::{BlockCipherDecrypt, BlockCipherEncrypt};
use rc2::Rc2;

/// RC2 block size in bytes (64-bit block).
const BLOCK: usize = 8;

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

/// Resolve the effective key length in bits (T1). `0` / unset means "default to
/// the supplied key's bit-length" (RC2's natural default). Valid RC2 range is
/// 1..=1024 bits.
fn resolve_eff_bits(eff_key_bits: u32, key_len: usize) -> Result<usize, String> {
    let bits = if eff_key_bits == 0 { key_len * 8 } else { eff_key_bits as usize };
    if !(1..=1024).contains(&bits) {
        return Err(format!("effective key bits must be 1..=1024, got {bits}"));
    }
    Ok(bits)
}

fn build_cipher(key: &[u8], eff_bits: usize) -> Result<Rc2, String> {
    if key.is_empty() || key.len() > 128 {
        return Err(format!("RC2 key must be 1..=128 bytes, got {}", key.len()));
    }
    Ok(Rc2::new_with_eff_key_len(key, eff_bits))
}

/// PKCS#7-pad `data` to a multiple of BLOCK bytes.
fn pkcs7_pad(data: &[u8]) -> Vec<u8> {
    let pad = BLOCK - (data.len() % BLOCK);
    let mut out = Vec::with_capacity(data.len() + pad);
    out.extend_from_slice(data);
    out.extend(std::iter::repeat(pad as u8).take(pad));
    out
}

/// Strip + validate PKCS#7 padding.
fn pkcs7_unpad(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() || data.len() % BLOCK != 0 {
        return Err("ciphertext length is not a multiple of the 8-byte block".into());
    }
    let pad = *data.last().unwrap() as usize;
    if pad == 0 || pad > BLOCK {
        return Err("invalid padding (wrong key/iv or corrupt data)".into());
    }
    if data[data.len() - pad..].iter().any(|&b| b as usize != pad) {
        return Err("invalid padding (wrong key/iv or corrupt data)".into());
    }
    Ok(data[..data.len() - pad].to_vec())
}

fn enc_block(c: &Rc2, b: &mut [u8; BLOCK]) {
    let mut ga = Array(*b);
    c.encrypt_block(&mut ga);
    b.copy_from_slice(&ga);
}

fn dec_block(c: &Rc2, b: &mut [u8; BLOCK]) {
    let mut ga = Array(*b);
    c.decrypt_block(&mut ga);
    b.copy_from_slice(&ga);
}

/// Raw RC2 over already-padded bytes (no encoding, no padding). Useful for tests
/// against the RFC 2268 single-block vectors. `data` must be a multiple of BLOCK.
pub fn encrypt_blocks(data: &[u8], key: &[u8], eff_bits: usize, mode: Mode, iv: &[u8]) -> Result<Vec<u8>, String> {
    let c = build_cipher(key, eff_bits)?;
    let mut out = Vec::with_capacity(data.len());
    let mut prev = [0u8; BLOCK];
    if mode == Mode::Cbc {
        if iv.len() != BLOCK {
            return Err("CBC needs an 8-byte iv".into());
        }
        prev.copy_from_slice(iv);
    }
    for chunk in data.chunks(BLOCK) {
        let mut block = [0u8; BLOCK];
        block.copy_from_slice(chunk);
        if mode == Mode::Cbc {
            for i in 0..BLOCK {
                block[i] ^= prev[i];
            }
        }
        enc_block(&c, &mut block);
        if mode == Mode::Cbc {
            prev = block;
        }
        out.extend_from_slice(&block);
    }
    Ok(out)
}

fn decrypt_blocks(data: &[u8], key: &[u8], eff_bits: usize, mode: Mode, iv: &[u8]) -> Result<Vec<u8>, String> {
    let c = build_cipher(key, eff_bits)?;
    let mut out = Vec::with_capacity(data.len());
    let mut prev = [0u8; BLOCK];
    if mode == Mode::Cbc {
        if iv.len() != BLOCK {
            return Err("CBC needs an 8-byte iv".into());
        }
        prev.copy_from_slice(iv);
    }
    for chunk in data.chunks(BLOCK) {
        let mut block = [0u8; BLOCK];
        block.copy_from_slice(chunk);
        let cipher_block = block;
        dec_block(&c, &mut block);
        if mode == Mode::Cbc {
            for i in 0..BLOCK {
                block[i] ^= prev[i];
            }
            prev = cipher_block;
        }
        out.extend_from_slice(&block);
    }
    Ok(out)
}

/// Encrypt `plaintext` (UTF-8). Returns the ciphertext encoded with `fmt`.
pub fn encrypt(
    plaintext: &str,
    key_str: &str,
    iv_str: &str,
    eff_key_bits: u32,
    mode: Mode,
    fmt: Encoding,
) -> Result<String, String> {
    let key = fmt.decode(key_str)?;
    let eff = resolve_eff_bits(eff_key_bits, key.len())?;
    let iv = if mode == Mode::Cbc { fmt.decode(iv_str)? } else { Vec::new() };
    if mode == Mode::Cbc && iv.len() != BLOCK {
        return Err("CBC needs an 8-byte iv".into());
    }
    let padded = pkcs7_pad(plaintext.as_bytes());
    let ct = encrypt_blocks(&padded, &key, eff, mode, &iv)?;
    Ok(fmt.encode(&ct))
}

/// Decrypt `ciphertext` (encoded with `fmt`). Returns the recovered UTF-8 text.
pub fn decrypt(
    ciphertext: &str,
    key_str: &str,
    iv_str: &str,
    eff_key_bits: u32,
    mode: Mode,
    fmt: Encoding,
) -> Result<String, String> {
    let key = fmt.decode(key_str)?;
    let eff = resolve_eff_bits(eff_key_bits, key.len())?;
    let ct = fmt.decode(ciphertext)?;
    if ct.is_empty() || ct.len() % BLOCK != 0 {
        return Err("ciphertext length is not a multiple of the 8-byte block".into());
    }
    let iv = if mode == Mode::Cbc { fmt.decode(iv_str)? } else { Vec::new() };
    if mode == Mode::Cbc && iv.len() != BLOCK {
        return Err("CBC needs an 8-byte iv".into());
    }
    let padded = decrypt_blocks(&ct, &key, eff, mode, &iv)?;
    let pt = pkcs7_unpad(&padded)?;
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 2268 §5 test vectors. (key hex, T1 bits, plaintext hex, ciphertext hex)
    const RFC_VECTORS: &[(&str, usize, &str, &str)] = &[
        ("0000000000000000", 63, "0000000000000000", "ebb773f993278eff"),
        ("ffffffffffffffff", 64, "ffffffffffffffff", "278b27e42e2f0d49"),
        ("3000000000000000", 64, "1000000000000001", "30649edf9be7d2c2"),
        ("88", 64, "0000000000000000", "61a8a244adacccf0"),
        ("88bca90e90875a", 64, "0000000000000000", "6ccf4308974c267f"),
        (
            "88bca90e90875a7f0f79c384627bafb2",
            64,
            "0000000000000000",
            "1a807d272bbe5db1",
        ),
        (
            "88bca90e90875a7f0f79c384627bafb2",
            128,
            "0000000000000000",
            "2269552ab0f85ca6",
        ),
        (
            "88bca90e90875a7f0f79c384627bafb216f80a6f85920584c42fceb0be255daf1e",
            129,
            "0000000000000000",
            "5b78d3a43dfff1f1",
        ),
    ];

    #[test]
    fn rfc2268_vectors_encrypt() {
        for (i, (key, t1, pt, ct)) in RFC_VECTORS.iter().enumerate() {
            let key = hex::decode(key).unwrap();
            let pt = hex::decode(pt).unwrap();
            let want = hex::decode(ct).unwrap();
            let got = encrypt_blocks(&pt, &key, *t1, Mode::Ecb, &[]).unwrap();
            assert_eq!(got, want, "RFC vector {} encrypt", i + 1);
        }
    }

    #[test]
    fn rfc2268_vectors_decrypt() {
        for (i, (key, t1, pt, ct)) in RFC_VECTORS.iter().enumerate() {
            let key = hex::decode(key).unwrap();
            let want = hex::decode(pt).unwrap();
            let ct = hex::decode(ct).unwrap();
            let got = decrypt_blocks(&ct, &key, *t1, Mode::Ecb, &[]).unwrap();
            assert_eq!(got, want, "RFC vector {} decrypt", i + 1);
        }
    }

    const KEY: &str = "0123456789abcdef"; // 8 bytes hex
    const IV: &str = "fedcba9876543210";

    #[test]
    fn cbc_roundtrip() {
        let ct = encrypt("hello RC2 🔐", KEY, IV, 0, Mode::Cbc, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, KEY, IV, 0, Mode::Cbc, Encoding::Hex).unwrap();
        assert_eq!(pt, "hello RC2 🔐");
    }

    #[test]
    fn ecb_roundtrip_base64() {
        // base64 "MTIzNDU2Nzg=" decodes to the 8-byte key "12345678".
        let k = "MTIzNDU2Nzg=";
        let ct = encrypt("block data here", k, "", 0, Mode::Ecb, Encoding::Base64).unwrap();
        let pt = decrypt(&ct, k, "", 0, Mode::Ecb, Encoding::Base64).unwrap();
        assert_eq!(pt, "block data here");
    }

    #[test]
    fn eff_key_bits_changes_output() {
        let a = encrypt("same plaintext!!", KEY, IV, 40, Mode::Cbc, Encoding::Hex).unwrap();
        let b = encrypt("same plaintext!!", KEY, IV, 128, Mode::Cbc, Encoding::Hex).unwrap();
        assert_ne!(a, b, "different effective key bits must produce different ciphertext");
        // each still round-trips with its own setting
        assert_eq!(decrypt(&a, KEY, IV, 40, Mode::Cbc, Encoding::Hex).unwrap(), "same plaintext!!");
        assert_eq!(decrypt(&b, KEY, IV, 128, Mode::Cbc, Encoding::Hex).unwrap(), "same plaintext!!");
    }

    #[test]
    fn explicit_eff_bits_roundtrip_ecb() {
        let ct = encrypt("interop test 123", KEY, "", 40, Mode::Ecb, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, KEY, "", 40, Mode::Ecb, Encoding::Hex).unwrap();
        assert_eq!(pt, "interop test 123");
    }

    #[test]
    fn wrong_key_fails() {
        let ct = encrypt("secret message", KEY, IV, 0, Mode::Cbc, Encoding::Hex).unwrap();
        assert!(decrypt(&ct, "ffffffffffffffff", IV, 0, Mode::Cbc, Encoding::Hex).is_err());
    }

    #[test]
    fn errors() {
        assert!(encrypt("x", "", IV, 0, Mode::Cbc, Encoding::Hex).is_err()); // empty key
        assert!(encrypt("x", KEY, "", 0, Mode::Cbc, Encoding::Hex).is_err()); // missing iv for cbc
        assert!(encrypt("x", KEY, IV, 2000, Mode::Cbc, Encoding::Hex).is_err()); // eff bits out of range
        assert!(decrypt("ab", KEY, IV, 0, Mode::Cbc, Encoding::Hex).is_err()); // not a block multiple
        assert!(Mode::parse("gcm").is_err());
        assert!(Encoding::parse("octal").is_err());
    }
}
