//! gizza-ai/salsa20-cipher core — encrypt or decrypt data with the Salsa20
//! stream cipher (Daniel J. Bernstein's eSTREAM finalist) given a key and an
//! 8-byte nonce. Pure Rust, no crypto dependencies (only hex/base64 for I/O).
//!
//! Salsa20 is a symmetric stream cipher: it expands a 16- or 32-byte key and an
//! 8-byte nonce into a keystream that is XOR'd with the data, so the **same**
//! operation encrypts and decrypts (given the same key + nonce). This is the
//! original 20-round Salsa20 ("Salsa20/20"). The same key+nonce pair must NEVER
//! be reused for two different messages.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// Binary encoding for the key/nonce (when given as bytes) and the ciphertext.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Hex,
    Base64,
}

impl Encoding {
    pub fn parse(s: &str) -> Result<Encoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "hex" | "" => Ok(Encoding::Hex),
            "base64" | "b64" => Ok(Encoding::Base64),
            other => Err(format!("unknown format '{other}' (use hex or base64)")),
        }
    }
    pub fn decode(self, s: &str) -> Result<Vec<u8>, String> {
        let s = s.trim();
        match self {
            Encoding::Hex => hex::decode(s).map_err(|e| format!("invalid hex: {e}")),
            Encoding::Base64 => B64.decode(s).map_err(|e| format!("invalid base64: {e}")),
        }
    }
    pub fn encode(self, b: &[u8]) -> String {
        match self {
            Encoding::Hex => hex::encode(b),
            Encoding::Base64 => B64.encode(b),
        }
    }
}

/// How the key/nonce string is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFormat {
    /// The key/nonce are UTF-8 passphrases used as raw bytes.
    Text,
    /// The key/nonce are encoded (hex or base64), matching `format`.
    Encoded,
}

impl KeyFormat {
    pub fn parse(s: &str) -> Result<KeyFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" | "utf8" | "utf-8" | "" => Ok(KeyFormat::Text),
            "encoded" | "hex" | "base64" => Ok(KeyFormat::Encoded),
            other => Err(format!("unknown key_format '{other}' (use text or encoded)")),
        }
    }
}

#[inline]
fn rotl(a: u32, b: u32) -> u32 {
    a.rotate_left(b)
}

/// The Salsa20/20 block function, applied to 16 words → 64 keystream bytes.
fn salsa20_block(input: &[u32; 16], out: &mut [u8; 64]) {
    let mut x = *input;
    // 20 rounds = 10 double-rounds.
    for _ in 0..10 {
        // column round
        x[4] ^= rotl(x[0].wrapping_add(x[12]), 7);
        x[8] ^= rotl(x[4].wrapping_add(x[0]), 9);
        x[12] ^= rotl(x[8].wrapping_add(x[4]), 13);
        x[0] ^= rotl(x[12].wrapping_add(x[8]), 18);

        x[9] ^= rotl(x[5].wrapping_add(x[1]), 7);
        x[13] ^= rotl(x[9].wrapping_add(x[5]), 9);
        x[1] ^= rotl(x[13].wrapping_add(x[9]), 13);
        x[5] ^= rotl(x[1].wrapping_add(x[13]), 18);

        x[14] ^= rotl(x[10].wrapping_add(x[6]), 7);
        x[2] ^= rotl(x[14].wrapping_add(x[10]), 9);
        x[6] ^= rotl(x[2].wrapping_add(x[14]), 13);
        x[10] ^= rotl(x[6].wrapping_add(x[2]), 18);

        x[3] ^= rotl(x[15].wrapping_add(x[11]), 7);
        x[7] ^= rotl(x[3].wrapping_add(x[15]), 9);
        x[11] ^= rotl(x[7].wrapping_add(x[3]), 13);
        x[15] ^= rotl(x[11].wrapping_add(x[7]), 18);

        // row round
        x[1] ^= rotl(x[0].wrapping_add(x[3]), 7);
        x[2] ^= rotl(x[1].wrapping_add(x[0]), 9);
        x[3] ^= rotl(x[2].wrapping_add(x[1]), 13);
        x[0] ^= rotl(x[3].wrapping_add(x[2]), 18);

        x[6] ^= rotl(x[5].wrapping_add(x[4]), 7);
        x[7] ^= rotl(x[6].wrapping_add(x[5]), 9);
        x[4] ^= rotl(x[7].wrapping_add(x[6]), 13);
        x[5] ^= rotl(x[4].wrapping_add(x[7]), 18);

        x[11] ^= rotl(x[10].wrapping_add(x[9]), 7);
        x[8] ^= rotl(x[11].wrapping_add(x[10]), 9);
        x[9] ^= rotl(x[8].wrapping_add(x[11]), 13);
        x[10] ^= rotl(x[9].wrapping_add(x[8]), 18);

        x[12] ^= rotl(x[15].wrapping_add(x[14]), 7);
        x[13] ^= rotl(x[12].wrapping_add(x[15]), 9);
        x[14] ^= rotl(x[13].wrapping_add(x[12]), 13);
        x[15] ^= rotl(x[14].wrapping_add(x[13]), 18);
    }
    for i in 0..16 {
        let v = x[i].wrapping_add(input[i]);
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}

#[inline]
fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Initialise the 16-word Salsa20 state for a given key (16 or 32 bytes), 8-byte
/// nonce, and 64-bit block counter.
fn init_state(key: &[u8], nonce: &[u8; 8], counter: u64) -> [u32; 16] {
    // sigma = "expand 32-byte k", tau = "expand 16-byte k"
    const SIGMA: [&[u8; 4]; 4] = [b"expa", b"nd 3", b"2-by", b"te k"];
    const TAU: [&[u8; 4]; 4] = [b"expa", b"nd 1", b"6-by", b"te k"];

    let mut s = [0u32; 16];
    let (k0, k1, consts) = if key.len() == 32 {
        (&key[0..16], &key[16..32], SIGMA)
    } else {
        // 16-byte key is used for both halves (tau constants).
        (&key[0..16], &key[0..16], TAU)
    };

    s[0] = le32(consts[0]);
    s[1] = le32(&k0[0..4]);
    s[2] = le32(&k0[4..8]);
    s[3] = le32(&k0[8..12]);
    s[4] = le32(&k0[12..16]);
    s[5] = le32(consts[1]);
    s[6] = le32(&nonce[0..4]);
    s[7] = le32(&nonce[4..8]);
    s[8] = (counter & 0xffff_ffff) as u32;
    s[9] = (counter >> 32) as u32;
    s[10] = le32(consts[2]);
    s[11] = le32(&k1[0..4]);
    s[12] = le32(&k1[4..8]);
    s[13] = le32(&k1[8..12]);
    s[14] = le32(&k1[12..16]);
    s[15] = le32(consts[3]);
    s
}

/// Apply the Salsa20/20 keystream to `data`, starting at block `counter`.
/// Salsa20 is symmetric, so this both encrypts and decrypts.
///
/// The key must be 16 or 32 bytes and the nonce exactly 8 bytes.
pub fn salsa20_apply(key: &[u8], nonce: &[u8], counter: u64, data: &[u8]) -> Result<Vec<u8>, String> {
    if key.len() != 16 && key.len() != 32 {
        return Err(format!("Salsa20 key must be 16 or 32 bytes, got {}", key.len()));
    }
    if nonce.len() != 8 {
        return Err(format!("Salsa20 nonce must be exactly 8 bytes, got {}", nonce.len()));
    }
    let mut n = [0u8; 8];
    n.copy_from_slice(nonce);

    let mut out = Vec::with_capacity(data.len());
    let mut block = [0u8; 64];
    let mut blk = counter;
    for chunk in data.chunks(64) {
        let state = init_state(key, &n, blk);
        salsa20_block(&state, &mut block);
        for (i, &b) in chunk.iter().enumerate() {
            out.push(b ^ block[i]);
        }
        blk = blk.wrapping_add(1);
    }
    Ok(out)
}

fn resolve_bytes(s: &str, key_format: KeyFormat, fmt: Encoding) -> Result<Vec<u8>, String> {
    match key_format {
        KeyFormat::Text => Ok(s.as_bytes().to_vec()),
        KeyFormat::Encoded => fmt.decode(s),
    }
}

/// Encrypt UTF-8 `plaintext`. Returns the ciphertext encoded with `fmt`.
pub fn encrypt(
    plaintext: &str,
    key_str: &str,
    nonce_str: &str,
    key_format: KeyFormat,
    counter: u64,
    fmt: Encoding,
) -> Result<String, String> {
    let key = resolve_bytes(key_str, key_format, fmt)?;
    let nonce = resolve_bytes(nonce_str, key_format, fmt)?;
    let ct = salsa20_apply(&key, &nonce, counter, plaintext.as_bytes())?;
    Ok(fmt.encode(&ct))
}

/// Decrypt `ciphertext` (encoded with `fmt`). Returns the recovered UTF-8 text.
pub fn decrypt(
    ciphertext: &str,
    key_str: &str,
    nonce_str: &str,
    key_format: KeyFormat,
    counter: u64,
    fmt: Encoding,
) -> Result<String, String> {
    let key = resolve_bytes(key_str, key_format, fmt)?;
    let nonce = resolve_bytes(nonce_str, key_format, fmt)?;
    let ct = fmt.decode(ciphertext)?;
    let pt = salsa20_apply(&key, &nonce, counter, &ct)?;
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Official Salsa20/20 test vector (ECRYPT eSTREAM, 256-bit, Set 1, vector 0):
    // 256-bit key 0x80,0,...,0; nonce all zero; first 64 keystream bytes.
    #[test]
    fn estream_set1_vector0_keystream() {
        let mut key = [0u8; 32];
        key[0] = 0x80;
        let nonce = [0u8; 8];
        // keystream = encrypt of 64 zero bytes
        let ks = salsa20_apply(&key, &nonce, 0, &[0u8; 64]).unwrap();
        assert_eq!(
            hex::encode_upper(&ks[..64]),
            "E3BE8FDD8BECA2E3EA8EF9475B29A6E7\
             003951E1097A5C38D23B7A5FAD9F6844\
             B22C97559E2723C7CBBD3FE4FC8D9A07\
             44652A83E72A9C461876AF4D7EF1A117"
        );
    }

    // 128-bit key vector (ECRYPT eSTREAM, 128-bit, Set 1, vector 0):
    // key 0x80,0,...,0 (16 bytes); nonce all zero.
    #[test]
    fn estream_128bit_set1_vector0_keystream() {
        let mut key = [0u8; 16];
        key[0] = 0x80;
        let nonce = [0u8; 8];
        let ks = salsa20_apply(&key, &nonce, 0, &[0u8; 64]).unwrap();
        assert_eq!(
            hex::encode_upper(&ks[..64]),
            "4DFA5E481DA23EA09A31022050859936\
             DA52FCEE218005164F267CB65F5CFD7F\
             2B4F97E0FF16924A52DF269515110A07\
             F9E460BC65EF95DA58F740B7D1DBB0AA"
        );
    }

    #[test]
    fn encrypt_decrypt_roundtrip_text_key() {
        let msg = "The quick brown fox 🦊 jumps!";
        let ct = encrypt(msg, "0123456789abcdef", "deadbeef", KeyFormat::Text, 0, Encoding::Base64)
            .unwrap();
        let pt = decrypt(&ct, "0123456789abcdef", "deadbeef", KeyFormat::Text, 0, Encoding::Base64)
            .unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn roundtrip_encoded_key() {
        // 32-byte hex key + 8-byte hex nonce.
        let key = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        let nonce = "0011223344556677";
        let msg = "spans multiple\nblocks ".repeat(8);
        let ct = encrypt(&msg, key, nonce, KeyFormat::Encoded, 0, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, key, nonce, KeyFormat::Encoded, 0, Encoding::Hex).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn counter_changes_output() {
        let mut key = [0u8; 32];
        key[0] = 1;
        let nonce = [0u8; 8];
        let a = salsa20_apply(&key, &nonce, 0, &[0u8; 16]).unwrap();
        let b = salsa20_apply(&key, &nonce, 5, &[0u8; 16]).unwrap();
        assert_ne!(a, b, "different block counters must produce different keystream");
    }

    #[test]
    fn multi_block_continuity() {
        // A 128-byte message uses two keystream blocks; XORing the second 64
        // bytes alone with counter=1 must match.
        let mut key = [7u8; 32];
        key[3] = 9;
        let nonce = [1u8; 8];
        let data = vec![0xABu8; 128];
        let ct = salsa20_apply(&key, &nonce, 0, &data).unwrap();
        let second = salsa20_apply(&key, &nonce, 1, &data[64..]).unwrap();
        assert_eq!(&ct[64..], &second[..]);
    }

    #[test]
    fn wrong_nonce_does_not_recover() {
        let key = "0123456789abcdef";
        let ct = encrypt("hello world", key, "11111111", KeyFormat::Text, 0, Encoding::Hex).unwrap();
        match decrypt(&ct, key, "22222222", KeyFormat::Text, 0, Encoding::Hex) {
            Ok(pt) => assert_ne!(pt, "hello world"),
            Err(_) => {}
        }
    }

    #[test]
    fn errors() {
        assert!(salsa20_apply(b"shortkey", &[0u8; 8], 0, b"x").is_err()); // bad key len
        assert!(salsa20_apply(&[0u8; 32], &[0u8; 7], 0, b"x").is_err()); // bad nonce len
        assert!(Encoding::parse("octal").is_err());
        assert!(KeyFormat::parse("rot13").is_err());
        // bad hex ciphertext
        assert!(decrypt("zzz", &"0".repeat(64), "0011223344556677", KeyFormat::Encoded, 0, Encoding::Hex).is_err());
    }
}
