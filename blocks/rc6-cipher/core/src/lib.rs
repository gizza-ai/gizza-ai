//! gizza-ai/rc6-cipher core — encrypt or decrypt data with the RC6 block cipher
//! in ECB or CBC mode, with hex/base64 I/O. Pure-Rust: the RC6-32/20/b cipher
//! (the standard 32-bit-word, 20-round AES-finalist parameterisation), ECB/CBC
//! chaining and PKCS#7 padding are all implemented here so the block runs on
//! every backend with no external crypto crate.
//!
//! RC6 (Rivest, Robshaw, Sidney, Yin, 1998) is a 128-bit-block cipher and was an
//! AES finalist. It takes a variable-length key (here 1-255 bytes; 16/24/32 bytes
//! = the classic 128/192/256-bit sizes). RC6 is unbroken but never standardised
//! as AES and is rarely used in modern systems — prefer `aes-cipher` or the
//! passphrase tools for real encryption; use this for interop, learning, or CTFs.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// RC6 block size in bytes (128-bit block = four 32-bit words).
const BLOCK: usize = 16;
/// Word size in bits (RC6-32).
const W: u32 = 32;
/// Number of rounds (RC6-.../20).
const R: usize = 20;
/// log2(W) — the rotation amount baked into the round function.
const LGW: u32 = 5;
/// Magic constants P32, Q32 (odd integers derived from e and the golden ratio).
const P32: u32 = 0xB7E1_5163;
const Q32: u32 = 0x9E37_79B9;

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

/// RC6 key schedule: expand the user key into the round-key array S (2R+4 words).
fn key_schedule(key: &[u8]) -> Result<Vec<u32>, String> {
    if key.is_empty() || key.len() > 255 {
        return Err(format!("RC6 key must be 1-255 bytes, got {}", key.len()));
    }
    // Load the key bytes little-endian into c = ceil(b/4) words (at least 1).
    let c = key.len().div_ceil(4).max(1);
    let mut l = vec![0u32; c];
    for (i, &byte) in key.iter().enumerate() {
        l[i / 4] |= (byte as u32) << (8 * (i % 4));
    }

    // Initialise S with the magic constants.
    let t = 2 * R + 4; // = 44 words for R = 20
    let mut s = vec![0u32; t];
    s[0] = P32;
    for i in 1..t {
        s[i] = s[i - 1].wrapping_add(Q32);
    }

    // Mix the user key into S.
    let (mut a, mut b) = (0u32, 0u32);
    let (mut i, mut j) = (0usize, 0usize);
    let v = 3 * t.max(c);
    for _ in 0..v {
        a = s[i].wrapping_add(a).wrapping_add(b).rotate_left(3);
        s[i] = a;
        let rot = a.wrapping_add(b) & (W - 1);
        b = l[j].wrapping_add(a).wrapping_add(b).rotate_left(rot);
        l[j] = b;
        i = (i + 1) % t;
        j = (j + 1) % c;
    }
    Ok(s)
}

fn load_words(block: &[u8; BLOCK]) -> [u32; 4] {
    let mut w = [0u32; 4];
    for k in 0..4 {
        w[k] = u32::from_le_bytes([block[4 * k], block[4 * k + 1], block[4 * k + 2], block[4 * k + 3]]);
    }
    w
}

fn store_words(w: [u32; 4]) -> [u8; BLOCK] {
    let mut out = [0u8; BLOCK];
    for k in 0..4 {
        out[4 * k..4 * k + 4].copy_from_slice(&w[k].to_le_bytes());
    }
    out
}

/// f(x) = (x * (2x + 1)) <<< lg w — the RC6 quadratic round mixer.
fn f(x: u32) -> u32 {
    x.wrapping_mul(x.wrapping_mul(2).wrapping_add(1)).rotate_left(LGW)
}

fn enc_block(s: &[u32], block: &mut [u8; BLOCK]) {
    let [mut a, mut b, mut c, mut d] = load_words(block);
    b = b.wrapping_add(s[0]);
    d = d.wrapping_add(s[1]);
    for i in 1..=R {
        let t = f(b);
        let u = f(d);
        a = (a ^ t).rotate_left(u & (W - 1)).wrapping_add(s[2 * i]);
        c = (c ^ u).rotate_left(t & (W - 1)).wrapping_add(s[2 * i + 1]);
        let (na, nb, nc, nd) = (b, c, d, a);
        a = na;
        b = nb;
        c = nc;
        d = nd;
    }
    a = a.wrapping_add(s[2 * R + 2]);
    c = c.wrapping_add(s[2 * R + 3]);
    *block = store_words([a, b, c, d]);
}

fn dec_block(s: &[u32], block: &mut [u8; BLOCK]) {
    let [mut a, mut b, mut c, mut d] = load_words(block);
    c = c.wrapping_sub(s[2 * R + 3]);
    a = a.wrapping_sub(s[2 * R + 2]);
    for i in (1..=R).rev() {
        let (na, nb, nc, nd) = (d, a, b, c);
        a = na;
        b = nb;
        c = nc;
        d = nd;
        let u = f(d);
        let t = f(b);
        c = c.wrapping_sub(s[2 * i + 1]).rotate_right(t & (W - 1)) ^ u;
        a = a.wrapping_sub(s[2 * i]).rotate_right(u & (W - 1)) ^ t;
    }
    d = d.wrapping_sub(s[1]);
    b = b.wrapping_sub(s[0]);
    *block = store_words([a, b, c, d]);
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
        return Err("ciphertext length is not a multiple of the 16-byte block".into());
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

/// Raw RC6 over already-padded bytes (no encoding, no PKCS#7). Useful for the
/// published single-block test vectors. `data` must be a multiple of BLOCK.
pub fn encrypt_blocks(data: &[u8], key: &[u8], mode: Mode, iv: &[u8]) -> Result<Vec<u8>, String> {
    let s = key_schedule(key)?;
    let mut out = Vec::with_capacity(data.len());
    let mut prev = [0u8; BLOCK];
    if mode == Mode::Cbc {
        if iv.len() != BLOCK {
            return Err("CBC needs a 16-byte iv".into());
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
        enc_block(&s, &mut block);
        if mode == Mode::Cbc {
            prev = block;
        }
        out.extend_from_slice(&block);
    }
    Ok(out)
}

fn decrypt_blocks(data: &[u8], key: &[u8], mode: Mode, iv: &[u8]) -> Result<Vec<u8>, String> {
    let s = key_schedule(key)?;
    let mut out = Vec::with_capacity(data.len());
    let mut prev = [0u8; BLOCK];
    if mode == Mode::Cbc {
        if iv.len() != BLOCK {
            return Err("CBC needs a 16-byte iv".into());
        }
        prev.copy_from_slice(iv);
    }
    for chunk in data.chunks(BLOCK) {
        let mut block = [0u8; BLOCK];
        block.copy_from_slice(chunk);
        let cipher_block = block;
        dec_block(&s, &mut block);
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
pub fn encrypt(plaintext: &str, key_str: &str, iv_str: &str, mode: Mode, fmt: Encoding) -> Result<String, String> {
    let key = fmt.decode(key_str)?;
    let iv = if mode == Mode::Cbc { fmt.decode(iv_str)? } else { Vec::new() };
    if mode == Mode::Cbc && iv.len() != BLOCK {
        return Err("CBC needs a 16-byte iv".into());
    }
    let padded = pkcs7_pad(plaintext.as_bytes());
    let ct = encrypt_blocks(&padded, &key, mode, &iv)?;
    Ok(fmt.encode(&ct))
}

/// Decrypt `ciphertext` (encoded with `fmt`). Returns the recovered UTF-8 text.
pub fn decrypt(ciphertext: &str, key_str: &str, iv_str: &str, mode: Mode, fmt: Encoding) -> Result<String, String> {
    let key = fmt.decode(key_str)?;
    let ct = fmt.decode(ciphertext)?;
    if ct.is_empty() || ct.len() % BLOCK != 0 {
        return Err("ciphertext length is not a multiple of the 16-byte block".into());
    }
    let iv = if mode == Mode::Cbc { fmt.decode(iv_str)? } else { Vec::new() };
    if mode == Mode::Cbc && iv.len() != BLOCK {
        return Err("CBC needs a 16-byte iv".into());
    }
    let padded = decrypt_blocks(&ct, &key, mode, &iv)?;
    let pt = pkcs7_unpad(&padded)?;
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Official 128-bit-key RC6 specification test vectors ("The RC6 Block Cipher",
    // App. B), as raw byte streams (hex). RC6 loads each 4-byte group little-endian.
    // (key, plaintext, ciphertext).
    const VECTORS: &[(&str, &str, &str)] = &[
        (
            "00000000000000000000000000000000",
            "00000000000000000000000000000000",
            "8fc3a53656b1f778c129df4e9848a41e",
        ),
        (
            "0123456789abcdef0112233445566778",
            "02132435465768798a9bacbdcedfe0f1",
            "524e192f4715c6231f51f6367ea43f18",
        ),
    ];

    #[test]
    fn rc6_spec_vectors_encrypt() {
        for (i, (key, pt, ct)) in VECTORS.iter().enumerate() {
            let key = hex::decode(key).unwrap();
            let pt = hex::decode(pt).unwrap();
            let want = hex::decode(ct).unwrap();
            let got = encrypt_blocks(&pt, &key, Mode::Ecb, &[]).unwrap();
            assert_eq!(got, want, "RC6 spec vector {} encrypt", i + 1);
        }
    }

    #[test]
    fn rc6_spec_vectors_decrypt() {
        for (i, (key, pt, ct)) in VECTORS.iter().enumerate() {
            let key = hex::decode(key).unwrap();
            let want = hex::decode(pt).unwrap();
            let ct = hex::decode(ct).unwrap();
            let got = decrypt_blocks(&ct, &key, Mode::Ecb, &[]).unwrap();
            assert_eq!(got, want, "RC6 spec vector {} decrypt", i + 1);
        }
    }

    const KEY: &str = "0123456789abcdef0123456789abcdef"; // 16 bytes hex
    const IV: &str = "fedcba9876543210fedcba9876543210"; // 16 bytes hex

    #[test]
    fn cbc_roundtrip() {
        let ct = encrypt("hello RC6 🔐 cipher!", KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        assert_eq!(pt, "hello RC6 🔐 cipher!");
    }

    #[test]
    fn ecb_roundtrip_base64() {
        // base64 "MDEyMzQ1Njc4OWFiY2RlZg==" decodes to the 16-byte key "0123456789abcdef".
        let k = "MDEyMzQ1Njc4OWFiY2RlZg==";
        let ct = encrypt("block data here!", k, "", Mode::Ecb, Encoding::Base64).unwrap();
        let pt = decrypt(&ct, k, "", Mode::Ecb, Encoding::Base64).unwrap();
        assert_eq!(pt, "block data here!");
    }

    #[test]
    fn key_sizes_192_256_roundtrip() {
        let k24 = "0123456789abcdef0123456789abcdef0123456789abcdef"; // 24 bytes
        let k32 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"; // 32 bytes
        for k in [k24, k32] {
            let ct = encrypt("interop test 12345", k, IV, Mode::Cbc, Encoding::Hex).unwrap();
            assert_eq!(decrypt(&ct, k, IV, Mode::Cbc, Encoding::Hex).unwrap(), "interop test 12345");
        }
    }

    #[test]
    fn wrong_key_fails() {
        let ct = encrypt("secret message here", KEY, IV, Mode::Cbc, Encoding::Hex).unwrap();
        let bad = "00000000000000000000000000000000";
        assert!(decrypt(&ct, bad, IV, Mode::Cbc, Encoding::Hex).is_err());
    }

    #[test]
    fn errors() {
        assert!(encrypt("x", "", IV, Mode::Cbc, Encoding::Hex).is_err()); // empty key
        assert!(encrypt("x", KEY, "", Mode::Cbc, Encoding::Hex).is_err()); // missing iv for cbc
        assert!(encrypt("x", KEY, "abcd", Mode::Cbc, Encoding::Hex).is_err()); // short iv
        assert!(decrypt("abcd", KEY, IV, Mode::Cbc, Encoding::Hex).is_err()); // not a block multiple
        assert!(Mode::parse("gcm").is_err());
        assert!(Encoding::parse("octal").is_err());
    }
}
