//! gizza-ai/rabbit-cipher core — encrypt or decrypt data with the Rabbit stream
//! cipher (RFC 4503 / eSTREAM), a fast 128-bit-key, 64-bit-IV stream cipher.
//!
//! Rabbit is a symmetric stream cipher: it generates a keystream that is XORed
//! with the data, so the **same** operation encrypts and decrypts. To recover a
//! message you must use the *same key, IV and encoding*.
//!
//! This is a from-scratch, dependency-free implementation of RFC 4503 (the eSTREAM
//! Rabbit cipher) — it validates against the official RFC test vectors below. Pure
//! Rust, no dependencies beyond hex/base64 for I/O, so it instantiates on every
//! gizza backend (chat block, page wasm, CLI).

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// Binary encoding for the key, the IV and the ciphertext.
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

// ----------------------------------------------------------------------------
// Rabbit cipher core (RFC 4503).
// ----------------------------------------------------------------------------

/// Internal Rabbit state: eight 32-bit state vars, eight 32-bit counters, one
/// carry bit.
struct Rabbit {
    x: [u32; 8],
    c: [u32; 8],
    carry: u32,
}

const A: [u32; 8] = [
    0x4D34_D34D,
    0xD34D_34D3,
    0x34D3_4D34,
    0x4D34_D34D,
    0xD34D_34D3,
    0x34D3_4D34,
    0x4D34_D34D,
    0xD34D_34D3,
];

#[inline]
fn g_func(u: u32, v: u32) -> u32 {
    // square = (u + v)^2 over 64 bits, then fold high/low 32-bit halves.
    let uv = u.wrapping_add(v) as u64;
    let sq = uv.wrapping_mul(uv);
    ((sq >> 32) ^ sq) as u32
}

impl Rabbit {
    fn next_state(&mut self) {
        // Counter system update.
        let old = self.c;
        let mut carry = self.carry;
        for i in 0..8 {
            let t = (old[i] as u64) + (A[i] as u64) + (carry as u64);
            carry = (t >> 32) as u32;
            self.c[i] = t as u32;
        }
        self.carry = carry;

        // Next-state function.
        let mut g = [0u32; 8];
        for i in 0..8 {
            g[i] = g_func(self.x[i], self.c[i]);
        }
        self.x[0] = g[0]
            .wrapping_add(g[7].rotate_left(16))
            .wrapping_add(g[6].rotate_left(16));
        self.x[1] = g[1].wrapping_add(g[0].rotate_left(8)).wrapping_add(g[7]);
        self.x[2] = g[2]
            .wrapping_add(g[1].rotate_left(16))
            .wrapping_add(g[0].rotate_left(16));
        self.x[3] = g[3].wrapping_add(g[2].rotate_left(8)).wrapping_add(g[1]);
        self.x[4] = g[4]
            .wrapping_add(g[3].rotate_left(16))
            .wrapping_add(g[2].rotate_left(16));
        self.x[5] = g[5].wrapping_add(g[4].rotate_left(8)).wrapping_add(g[3]);
        self.x[6] = g[6]
            .wrapping_add(g[5].rotate_left(16))
            .wrapping_add(g[4].rotate_left(16));
        self.x[7] = g[7].wrapping_add(g[6].rotate_left(8)).wrapping_add(g[5]);
    }

    /// Key setup from a 16-byte (128-bit) key. The key is interpreted as a
    /// 128-bit integer in big-endian (MSB-first) byte order, exactly as written
    /// in a hex key string and in RFC 4503's test vectors; K0 = K[15..0] is the
    /// least-significant 16-bit subkey.
    fn new(key: &[u8; 16]) -> Rabbit {
        // K[0..8] are 16-bit subkeys; K0 is the least-significant word.
        let mut k = [0u16; 8];
        for j in 0..8 {
            // Byte 0 is the most significant; subkey j spans bytes 14-2j..16-2j.
            let hi = key[15 - (2 * j + 1)];
            let lo = key[15 - (2 * j)];
            k[j] = u16::from_be_bytes([hi, lo]);
        }

        let mut x = [0u32; 8];
        let mut c = [0u32; 8];
        for j in 0..8 {
            if j % 2 == 0 {
                x[j] = ((k[(j + 1) % 8] as u32) << 16) | (k[j] as u32);
                c[j] = ((k[(j + 4) % 8] as u32) << 16) | (k[(j + 5) % 8] as u32);
            } else {
                x[j] = ((k[(j + 5) % 8] as u32) << 16) | (k[(j + 4) % 8] as u32);
                c[j] = ((k[j] as u32) << 16) | (k[(j + 1) % 8] as u32);
            }
        }

        let mut r = Rabbit { x, c, carry: 0 };
        // Iterate the system four times.
        for _ in 0..4 {
            r.next_state();
        }
        // Reinitialise the counters.
        for j in 0..8 {
            r.c[j] ^= r.x[(j + 4) % 8];
        }
        r
    }

    /// IV setup from an 8-byte (64-bit) IV. Like the key, the IV is interpreted
    /// MSB-first (big-endian), as written in a hex IV string and in RFC 4503's
    /// vectors. i0 = IV[31..0], i2 = IV[63..32].
    fn iv_setup(&mut self, iv: &[u8; 8]) {
        let i0 = u32::from_be_bytes([iv[4], iv[5], iv[6], iv[7]]); // IV[31..0]
        let i2 = u32::from_be_bytes([iv[0], iv[1], iv[2], iv[3]]); // IV[63..32]
        let i1 = (i2 & 0xFFFF_0000) | (i0 >> 16); // IV[63..48] || IV[31..16]
        let i3 = (i2 << 16) | (i0 & 0x0000_FFFF); // IV[47..32] || IV[15..0]

        self.c[0] ^= i0;
        self.c[1] ^= i1;
        self.c[2] ^= i2;
        self.c[3] ^= i3;
        self.c[4] ^= i0;
        self.c[5] ^= i1;
        self.c[6] ^= i2;
        self.c[7] ^= i3;

        for _ in 0..4 {
            self.next_state();
        }
    }

    /// Extract the next 16-byte keystream block (128 bits), little-endian.
    fn next_block(&mut self) -> [u8; 16] {
        self.next_state();
        let s0 = self.x[0] ^ (self.x[5] >> 16) ^ (self.x[3] << 16);
        let s1 = self.x[2] ^ (self.x[7] >> 16) ^ (self.x[5] << 16);
        let s2 = self.x[4] ^ (self.x[1] >> 16) ^ (self.x[7] << 16);
        let s3 = self.x[6] ^ (self.x[3] >> 16) ^ (self.x[1] << 16);
        let mut out = [0u8; 16];
        out[0..4].copy_from_slice(&s0.to_le_bytes());
        out[4..8].copy_from_slice(&s1.to_le_bytes());
        out[8..12].copy_from_slice(&s2.to_le_bytes());
        out[12..16].copy_from_slice(&s3.to_le_bytes());
        out
    }
}

/// Apply the Rabbit keystream to `data` with the given 16-byte key and optional
/// 8-byte IV. Rabbit is symmetric, so this both encrypts and decrypts.
pub fn rabbit_apply(key: &[u8], iv: Option<&[u8]>, data: &[u8]) -> Result<Vec<u8>, String> {
    if key.len() != 16 {
        return Err(format!(
            "Rabbit key must be exactly 16 bytes (128 bits), got {}",
            key.len()
        ));
    }
    let key16: [u8; 16] = key.try_into().unwrap();
    let mut r = Rabbit::new(&key16);
    if let Some(iv) = iv {
        if iv.len() != 8 {
            return Err(format!(
                "Rabbit IV must be exactly 8 bytes (64 bits), got {}",
                iv.len()
            ));
        }
        let iv8: [u8; 8] = iv.try_into().unwrap();
        r.iv_setup(&iv8);
    }

    let mut out = Vec::with_capacity(data.len());
    let mut block = r.next_block();
    let mut idx = 0usize;
    for &byte in data {
        if idx == 16 {
            block = r.next_block();
            idx = 0;
        }
        out.push(byte ^ block[idx]);
        idx += 1;
    }
    Ok(out)
}

/// How the key / IV strings are interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFormat {
    /// The value is a UTF-8 passphrase used as raw bytes.
    Text,
    /// The value is encoded (hex or base64), matching `format`.
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

fn resolve_bytes(s: &str, key_format: KeyFormat, fmt: Encoding) -> Result<Vec<u8>, String> {
    match key_format {
        KeyFormat::Text => Ok(s.as_bytes().to_vec()),
        KeyFormat::Encoded => fmt.decode(s),
    }
}

fn resolve_iv(
    iv_str: &str,
    key_format: KeyFormat,
    fmt: Encoding,
) -> Result<Option<Vec<u8>>, String> {
    if iv_str.trim().is_empty() {
        return Ok(None);
    }
    resolve_bytes(iv_str, key_format, fmt).map(Some)
}

/// Encrypt UTF-8 `plaintext`. Returns the ciphertext encoded with `fmt`.
pub fn encrypt(
    plaintext: &str,
    key_str: &str,
    iv_str: &str,
    key_format: KeyFormat,
    fmt: Encoding,
) -> Result<String, String> {
    let key = resolve_bytes(key_str, key_format, fmt)?;
    let iv = resolve_iv(iv_str, key_format, fmt)?;
    let ct = rabbit_apply(&key, iv.as_deref(), plaintext.as_bytes())?;
    Ok(fmt.encode(&ct))
}

/// Decrypt `ciphertext` (encoded with `fmt`). Returns the recovered UTF-8 text.
pub fn decrypt(
    ciphertext: &str,
    key_str: &str,
    iv_str: &str,
    key_format: KeyFormat,
    fmt: Encoding,
) -> Result<String, String> {
    let key = resolve_bytes(key_str, key_format, fmt)?;
    let iv = resolve_iv(iv_str, key_format, fmt)?;
    let ct = fmt.decode(ciphertext)?;
    let pt = rabbit_apply(&key, iv.as_deref(), &ct)?;
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 4503 Appendix A.1 — keystream with all-zero key, no IV.
    // The RFC prints S MSB-first; this implementation outputs little-endian, so
    // each expected block below is the RFC value with its bytes reversed.
    #[test]
    fn rfc_vector_zero_key_no_iv() {
        let key = [0u8; 16];
        let mut r = Rabbit::new(&key);
        let b0 = r.next_block();
        let b1 = r.next_block();
        let b2 = r.next_block();
        assert_eq!(hex::encode_upper(b0), "02F74A1C26456BF5ECD6A536F05457B1");
        assert_eq!(hex::encode_upper(b1), "A78AC689476C697B390C9CC515D8E888");
        assert_eq!(hex::encode_upper(b2), "96D6731688D168DA51D40C70C3A116F4");
    }

    // RFC 4503 Appendix A.1 — a non-zero key, no IV. The key hex is fed exactly
    // as the RFC prints it (MSB-first); expected S blocks are byte-reversed.
    #[test]
    fn rfc_vector_nonzero_key_no_iv() {
        let key = hex::decode("912813292E3D36FE3BFC62F1DC51C3AC").unwrap();
        let key16: [u8; 16] = key.try_into().unwrap();
        let mut r = Rabbit::new(&key16);
        let b0 = r.next_block();
        let b1 = r.next_block();
        let b2 = r.next_block();
        assert_eq!(hex::encode_upper(b0), "9C51E28784C37FE9A127F63EC8F32D3D");
        assert_eq!(hex::encode_upper(b1), "19FC5485AA53BF96885B40F461CD76F5");
        assert_eq!(hex::encode_upper(b2), "5E4C4D20203BE58A5043DBFB737454E5");
    }

    // RFC 4503 Appendix A.2 — IV setup test vectors (zero key, varying IV). IV hex
    // is fed exactly as printed (MSB-first); expected S blocks are byte-reversed.
    #[test]
    fn rfc_vector_iv_setup() {
        let key = [0u8; 16];

        let mut r = Rabbit::new(&key);
        r.iv_setup(&[0u8; 8]);
        assert_eq!(
            hex::encode_upper(r.next_block()),
            "EDB70567375DCD7CD89554F85E27A7C6"
        );

        let mut r = Rabbit::new(&key);
        let iv = hex::decode("C373F575C1267E59").unwrap();
        let iv8: [u8; 8] = iv.try_into().unwrap();
        r.iv_setup(&iv8);
        assert_eq!(
            hex::encode_upper(r.next_block()),
            "6D7D012292CCDCE0E2120058B94ECD1F"
        );

        let mut r = Rabbit::new(&key);
        let iv = hex::decode("A6EB561AD2F41727").unwrap();
        let iv8: [u8; 8] = iv.try_into().unwrap();
        r.iv_setup(&iv8);
        assert_eq!(
            hex::encode_upper(r.next_block()),
            "4D1051A123AFB670BF8D8505C8D85A44"
        );
    }

    #[test]
    fn encrypt_decrypt_roundtrip_text_key() {
        let msg = "The quick brown fox 🦊 jumps!";
        let ct = encrypt(msg, "sixteen-byte-key", "", KeyFormat::Text, Encoding::Base64).unwrap();
        let pt = decrypt(&ct, "sixteen-byte-key", "", KeyFormat::Text, Encoding::Base64).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn roundtrip_with_iv() {
        let msg = "rabbit with an IV";
        let key = "00112233445566778899aabbccddeeff";
        let iv = "0123456789abcdef";
        let ct = encrypt(msg, key, iv, KeyFormat::Encoded, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, key, iv, KeyFormat::Encoded, Encoding::Hex).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn iv_changes_output() {
        let key = b"sixteen-byte-key";
        let a = rabbit_apply(key, None, b"hello world data").unwrap();
        let b = rabbit_apply(key, Some(&[1u8; 8]), b"hello world data").unwrap();
        assert_ne!(a, b, "an IV must change the keystream");
    }

    #[test]
    fn spans_multiple_keystream_blocks() {
        // > 16 bytes forces a second keystream block; round-trip must still work.
        let msg = "this message is definitely longer than sixteen bytes of data";
        let ct = encrypt(msg, "sixteen-byte-key", "", KeyFormat::Text, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, "sixteen-byte-key", "", KeyFormat::Text, Encoding::Hex).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn wrong_key_does_not_recover() {
        let ct =
            encrypt("hello world", "right-key-16byte", "", KeyFormat::Text, Encoding::Hex).unwrap();
        match decrypt(&ct, "wrong-key-16byte", "", KeyFormat::Text, Encoding::Hex) {
            Ok(pt) => assert_ne!(pt, "hello world"),
            Err(_) => {}
        }
    }

    #[test]
    fn errors() {
        assert!(rabbit_apply(b"short", None, b"x").is_err()); // key not 16 bytes
        assert!(rabbit_apply(&[0u8; 16], Some(b"shrt"), b"x").is_err()); // IV not 8 bytes
        assert!(Encoding::parse("octal").is_err());
        assert!(KeyFormat::parse("rot13").is_err());
        // bad hex ciphertext
        assert!(decrypt("zzz", "0123456789abcdef", "", KeyFormat::Encoded, Encoding::Hex).is_err());
    }
}
