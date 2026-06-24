//! gizza-ai/chacha20-cipher core — encrypt or decrypt data with the ChaCha20
//! stream cipher and the ChaCha20-Poly1305 AEAD construction (RFC 8439).
//! Pure Rust, no crypto dependencies (only hex/base64 for I/O).
//!
//! Two modes:
//!   * `stream` — raw IETF ChaCha20: a 32-byte key + 12-byte nonce expand into a
//!     keystream XOR'd with the data. Symmetric (the same op encrypts/decrypts),
//!     but UNAUTHENTICATED. The same key+nonce pair must NEVER be reused.
//!   * `aead` — ChaCha20-Poly1305 (RFC 8439 §2.8): authenticated encryption with
//!     a 16-byte Poly1305 tag and optional associated data (AAD). Decryption
//!     verifies the tag before returning the plaintext (tamper-evident).
//!
//! IETF/RFC 8439 parameters: 256-bit key, 96-bit (12-byte) nonce, 32-bit block
//! counter (so this is NOT the original 64-bit-nonce DJB ChaCha20).

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// Binary encoding for the key/nonce (when given as bytes) and the ciphertext/tag.
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

/// Cipher mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Raw ChaCha20 stream cipher (unauthenticated).
    Stream,
    /// ChaCha20-Poly1305 AEAD (authenticated, with optional AAD).
    Aead,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stream" | "chacha20" | "" => Ok(Mode::Stream),
            "aead" | "chacha20-poly1305" | "poly1305" | "chacha20poly1305" => Ok(Mode::Aead),
            other => Err(format!("unknown mode '{other}' (use stream or aead)")),
        }
    }
}

// ---------------------------------------------------------------------------
// ChaCha20 block function (RFC 8439 §2.3)
// ---------------------------------------------------------------------------

#[inline]
fn quarter_round(x: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = (x[d] ^ x[a]).rotate_left(16);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = (x[b] ^ x[c]).rotate_left(12);
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = (x[d] ^ x[a]).rotate_left(8);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = (x[b] ^ x[c]).rotate_left(7);
}

#[inline]
fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Initialise the 16-word ChaCha20 state: constants, 32-byte key, 32-bit counter,
/// 12-byte nonce.
fn init_state(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u32; 16] {
    [
        0x6170_7865,
        0x3320_646e,
        0x7962_2d32,
        0x6b20_6574,
        le32(&key[0..4]),
        le32(&key[4..8]),
        le32(&key[8..12]),
        le32(&key[12..16]),
        le32(&key[16..20]),
        le32(&key[20..24]),
        le32(&key[24..28]),
        le32(&key[28..32]),
        counter,
        le32(&nonce[0..4]),
        le32(&nonce[4..8]),
        le32(&nonce[8..12]),
    ]
}

/// The ChaCha20 block function: 20 rounds (10 column+diagonal double-rounds) →
/// 64 keystream bytes.
fn chacha20_block(state: &[u32; 16], out: &mut [u8; 64]) {
    let mut x = *state;
    for _ in 0..10 {
        // column rounds
        quarter_round(&mut x, 0, 4, 8, 12);
        quarter_round(&mut x, 1, 5, 9, 13);
        quarter_round(&mut x, 2, 6, 10, 14);
        quarter_round(&mut x, 3, 7, 11, 15);
        // diagonal rounds
        quarter_round(&mut x, 0, 5, 10, 15);
        quarter_round(&mut x, 1, 6, 11, 12);
        quarter_round(&mut x, 2, 7, 8, 13);
        quarter_round(&mut x, 3, 4, 9, 14);
    }
    for i in 0..16 {
        let v = x[i].wrapping_add(state[i]);
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}

/// Apply the ChaCha20 keystream to `data`, starting at block `counter`.
/// ChaCha20 is symmetric, so this both encrypts and decrypts.
///
/// The key must be 32 bytes and the nonce exactly 12 bytes (IETF/RFC 8439).
pub fn chacha20_apply(key: &[u8], nonce: &[u8], counter: u32, data: &[u8]) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err(format!("ChaCha20 key must be exactly 32 bytes, got {}", key.len()));
    }
    if nonce.len() != 12 {
        return Err(format!("ChaCha20 nonce must be exactly 12 bytes, got {}", nonce.len()));
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(key);
    let mut n = [0u8; 12];
    n.copy_from_slice(nonce);

    let mut out = Vec::with_capacity(data.len());
    let mut block = [0u8; 64];
    let mut blk = counter;
    for chunk in data.chunks(64) {
        let state = init_state(&k, blk, &n);
        chacha20_block(&state, &mut block);
        for (i, &b) in chunk.iter().enumerate() {
            out.push(b ^ block[i]);
        }
        blk = blk.wrapping_add(1);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Poly1305 (RFC 8439 §2.5) — one-time MAC over a 32-byte one-time key (r || s).
// 5 x 26-bit limb implementation (the "donna" formulation).
// ---------------------------------------------------------------------------

fn poly1305(key: &[u8; 32], msg: &[u8]) -> [u8; 16] {
    // Clamp r and split into 5 x 26-bit limbs.
    let t0 = u32::from_le_bytes([key[0], key[1], key[2], key[3]]);
    let t1 = u32::from_le_bytes([key[4], key[5], key[6], key[7]]);
    let t2 = u32::from_le_bytes([key[8], key[9], key[10], key[11]]);
    let t3 = u32::from_le_bytes([key[12], key[13], key[14], key[15]]);

    let r0 = (t0 & 0x3ff_ffff) as u64;
    let r1 = (((t0 >> 26) | (t1 << 6)) & 0x3ff_ff03) as u64;
    let r2 = (((t1 >> 20) | (t2 << 12)) & 0x3ffc_0ff) as u64;
    let r3 = (((t2 >> 14) | (t3 << 18)) & 0x3f0_3fff) as u64;
    let r4 = ((t3 >> 8) & 0x00f_ffff) as u64;

    let s1 = r1 * 5;
    let s2 = r2 * 5;
    let s3 = r3 * 5;
    let s4 = r4 * 5;

    let mut h0: u64 = 0;
    let mut h1: u64 = 0;
    let mut h2: u64 = 0;
    let mut h3: u64 = 0;
    let mut h4: u64 = 0;

    let mut i = 0;
    while i < msg.len() {
        let n = core::cmp::min(16, msg.len() - i);
        let mut blk = [0u8; 17];
        blk[..n].copy_from_slice(&msg[i..i + n]);
        blk[n] = 1; // append the high bit (0x01) right after the message bytes

        let m0 = u32::from_le_bytes([blk[0], blk[1], blk[2], blk[3]]);
        let m1 = u32::from_le_bytes([blk[4], blk[5], blk[6], blk[7]]);
        let m2 = u32::from_le_bytes([blk[8], blk[9], blk[10], blk[11]]);
        let m3 = u32::from_le_bytes([blk[12], blk[13], blk[14], blk[15]]);

        h0 += (m0 & 0x3ff_ffff) as u64;
        h1 += (((m0 >> 26) | (m1 << 6)) & 0x3ff_ffff) as u64;
        h2 += (((m1 >> 20) | (m2 << 12)) & 0x3ff_ffff) as u64;
        h3 += (((m2 >> 14) | (m3 << 18)) & 0x3ff_ffff) as u64;
        h4 += ((m3 >> 8) as u64) | ((blk[16] as u64) << 24);

        // h *= r  (mod 2^130 - 5)
        let d0 = (h0 as u128) * (r0 as u128)
            + (h1 as u128) * (s4 as u128)
            + (h2 as u128) * (s3 as u128)
            + (h3 as u128) * (s2 as u128)
            + (h4 as u128) * (s1 as u128);
        let mut d1 = (h0 as u128) * (r1 as u128)
            + (h1 as u128) * (r0 as u128)
            + (h2 as u128) * (s4 as u128)
            + (h3 as u128) * (s3 as u128)
            + (h4 as u128) * (s2 as u128);
        let mut d2 = (h0 as u128) * (r2 as u128)
            + (h1 as u128) * (r1 as u128)
            + (h2 as u128) * (r0 as u128)
            + (h3 as u128) * (s4 as u128)
            + (h4 as u128) * (s3 as u128);
        let mut d3 = (h0 as u128) * (r3 as u128)
            + (h1 as u128) * (r2 as u128)
            + (h2 as u128) * (r1 as u128)
            + (h3 as u128) * (r0 as u128)
            + (h4 as u128) * (s4 as u128);
        let mut d4 = (h0 as u128) * (r4 as u128)
            + (h1 as u128) * (r3 as u128)
            + (h2 as u128) * (r2 as u128)
            + (h3 as u128) * (r1 as u128)
            + (h4 as u128) * (r0 as u128);

        // Partial carry reduction.
        let mut c: u64;
        h0 = (d0 as u64) & 0x3ff_ffff;
        c = (d0 >> 26) as u64;
        d1 += c as u128;
        h1 = (d1 as u64) & 0x3ff_ffff;
        c = (d1 >> 26) as u64;
        d2 += c as u128;
        h2 = (d2 as u64) & 0x3ff_ffff;
        c = (d2 >> 26) as u64;
        d3 += c as u128;
        h3 = (d3 as u64) & 0x3ff_ffff;
        c = (d3 >> 26) as u64;
        d4 += c as u128;
        h4 = (d4 as u64) & 0x3ff_ffff;
        c = (d4 >> 26) as u64;
        h0 += c * 5;
        c = h0 >> 26;
        h0 &= 0x3ff_ffff;
        h1 += c;

        i += 16;
    }

    // Final carry reduction.
    let mut c = h1 >> 26;
    h1 &= 0x3ff_ffff;
    h2 += c;
    c = h2 >> 26;
    h2 &= 0x3ff_ffff;
    h3 += c;
    c = h3 >> 26;
    h3 &= 0x3ff_ffff;
    h4 += c;
    c = h4 >> 26;
    h4 &= 0x3ff_ffff;
    h0 += c * 5;
    c = h0 >> 26;
    h0 &= 0x3ff_ffff;
    h1 += c;

    // Compute h + (-p) = h + 5 and conditionally use it if h >= p.
    let mut g0 = h0.wrapping_add(5);
    c = g0 >> 26;
    g0 &= 0x3ff_ffff;
    let mut g1 = h1.wrapping_add(c);
    c = g1 >> 26;
    g1 &= 0x3ff_ffff;
    let mut g2 = h2.wrapping_add(c);
    c = g2 >> 26;
    g2 &= 0x3ff_ffff;
    let mut g3 = h3.wrapping_add(c);
    c = g3 >> 26;
    g3 &= 0x3ff_ffff;
    let g4 = h4.wrapping_add(c).wrapping_sub(1 << 26);

    // If g4's borrow bit (bit 63) is clear, h >= p → select g; else select h.
    let mask = (g4 >> 63).wrapping_sub(1); // all-ones if no borrow (select g)
    let nmask = !mask;
    h0 = (h0 & nmask) | (g0 & mask);
    h1 = (h1 & nmask) | (g1 & mask);
    h2 = (h2 & nmask) | (g2 & mask);
    h3 = (h3 & nmask) | (g3 & mask);
    h4 = (h4 & nmask) | (g4 & mask);

    // Serialize h (130 bits) into a 128-bit little-endian integer, then add s.
    let mut h = (h0 | (h1 << 26)) as u128;
    h |= ((h2 | (h3 << 26)) as u128) << 52;
    h = h.wrapping_add((h4 as u128) << 104);

    let s = u128::from_le_bytes([
        key[16], key[17], key[18], key[19], key[20], key[21], key[22], key[23], key[24], key[25],
        key[26], key[27], key[28], key[29], key[30], key[31],
    ]);
    let tag = h.wrapping_add(s);
    tag.to_le_bytes()
}

/// Derive the Poly1305 one-time key for ChaCha20-Poly1305: ChaCha20 block 0's
/// first 32 bytes of keystream (RFC 8439 §2.6).
fn poly1305_key_gen(key: &[u8; 32], nonce: &[u8; 12]) -> [u8; 32] {
    let state = init_state(key, 0, nonce);
    let mut block = [0u8; 64];
    chacha20_block(&state, &mut block);
    let mut otk = [0u8; 32];
    otk.copy_from_slice(&block[..32]);
    otk
}

#[inline]
fn pad16(len: usize) -> usize {
    if len % 16 == 0 {
        0
    } else {
        16 - (len % 16)
    }
}

/// Build the Poly1305 MAC input for ChaCha20-Poly1305 (RFC 8439 §2.8):
/// AAD || pad16(AAD) || ciphertext || pad16(ciphertext) || len(AAD)_le64 ||
/// len(ciphertext)_le64.
fn aead_mac_data(aad: &[u8], ct: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(aad.len() + ct.len() + 32);
    m.extend_from_slice(aad);
    m.extend(std::iter::repeat(0u8).take(pad16(aad.len())));
    m.extend_from_slice(ct);
    m.extend(std::iter::repeat(0u8).take(pad16(ct.len())));
    m.extend_from_slice(&(aad.len() as u64).to_le_bytes());
    m.extend_from_slice(&(ct.len() as u64).to_le_bytes());
    m
}

/// ChaCha20-Poly1305 AEAD encryption (RFC 8439 §2.8).
/// Returns `(ciphertext, tag)`. The keystream uses block counter starting at 1.
pub fn chacha20_poly1305_encrypt(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<(Vec<u8>, [u8; 16]), String> {
    if key.len() != 32 {
        return Err(format!("ChaCha20 key must be exactly 32 bytes, got {}", key.len()));
    }
    if nonce.len() != 12 {
        return Err(format!("ChaCha20 nonce must be exactly 12 bytes, got {}", nonce.len()));
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(key);
    let mut n = [0u8; 12];
    n.copy_from_slice(nonce);

    let otk = poly1305_key_gen(&k, &n);
    let ct = chacha20_apply(&k, &n, 1, plaintext)?;
    let mac_data = aead_mac_data(aad, &ct);
    let tag = poly1305(&otk, &mac_data);
    Ok((ct, tag))
}

/// Constant-time 16-byte tag comparison.
fn tags_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// ChaCha20-Poly1305 AEAD decryption (RFC 8439 §2.8). Verifies the tag before
/// returning the plaintext. Errors if authentication fails.
pub fn chacha20_poly1305_decrypt(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8],
) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err(format!("ChaCha20 key must be exactly 32 bytes, got {}", key.len()));
    }
    if nonce.len() != 12 {
        return Err(format!("ChaCha20 nonce must be exactly 12 bytes, got {}", nonce.len()));
    }
    if tag.len() != 16 {
        return Err(format!("Poly1305 tag must be exactly 16 bytes, got {}", tag.len()));
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(key);
    let mut n = [0u8; 12];
    n.copy_from_slice(nonce);

    let otk = poly1305_key_gen(&k, &n);
    let mac_data = aead_mac_data(aad, ciphertext);
    let expected = poly1305(&otk, &mac_data);
    if !tags_equal(&expected, tag) {
        return Err("authentication failed: the Poly1305 tag does not match (wrong key/nonce/AAD or tampered ciphertext)".to_string());
    }
    chacha20_apply(&k, &n, 1, ciphertext)
}

fn resolve_bytes(s: &str, key_format: KeyFormat, fmt: Encoding) -> Result<Vec<u8>, String> {
    match key_format {
        KeyFormat::Text => Ok(s.as_bytes().to_vec()),
        KeyFormat::Encoded => fmt.decode(s),
    }
}

/// Encrypt UTF-8 `plaintext`.
///
/// * `Mode::Stream` → returns the ciphertext encoded with `fmt`.
/// * `Mode::Aead` → returns `encode(ciphertext || 16-byte Poly1305 tag)`.
pub fn encrypt(
    plaintext: &str,
    key_str: &str,
    nonce_str: &str,
    aad_str: &str,
    key_format: KeyFormat,
    mode: Mode,
    counter: u32,
    fmt: Encoding,
) -> Result<String, String> {
    let key = resolve_bytes(key_str, key_format, fmt)?;
    let nonce = resolve_bytes(nonce_str, key_format, fmt)?;
    match mode {
        Mode::Stream => {
            let ct = chacha20_apply(&key, &nonce, counter, plaintext.as_bytes())?;
            Ok(fmt.encode(&ct))
        }
        Mode::Aead => {
            let aad = aad_str.as_bytes();
            let (mut ct, tag) = chacha20_poly1305_encrypt(&key, &nonce, aad, plaintext.as_bytes())?;
            ct.extend_from_slice(&tag);
            Ok(fmt.encode(&ct))
        }
    }
}

/// Decrypt `ciphertext` (encoded with `fmt`). Returns the recovered UTF-8 text.
///
/// * `Mode::Stream` → XORs the keystream back.
/// * `Mode::Aead` → expects `encode(ciphertext || 16-byte tag)`; verifies the tag.
pub fn decrypt(
    ciphertext: &str,
    key_str: &str,
    nonce_str: &str,
    aad_str: &str,
    key_format: KeyFormat,
    mode: Mode,
    counter: u32,
    fmt: Encoding,
) -> Result<String, String> {
    let key = resolve_bytes(key_str, key_format, fmt)?;
    let nonce = resolve_bytes(nonce_str, key_format, fmt)?;
    let raw = fmt.decode(ciphertext)?;
    let pt = match mode {
        Mode::Stream => chacha20_apply(&key, &nonce, counter, &raw)?,
        Mode::Aead => {
            if raw.len() < 16 {
                return Err(format!(
                    "AEAD ciphertext too short ({} bytes): expected ciphertext + a 16-byte tag",
                    raw.len()
                ));
            }
            let (ct, tag) = raw.split_at(raw.len() - 16);
            let aad = aad_str.as_bytes();
            chacha20_poly1305_decrypt(&key, &nonce, aad, ct, tag)?
        }
    };
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 8439 §2.4.2 — ChaCha20 encryption test vector. key = 00..1f, nonce =
    // 00 00 00 00 00 00 00 4a 00 00 00 00, initial counter = 1.
    #[test]
    fn rfc8439_chacha20_encryption_vector() {
        let key: Vec<u8> = (0u8..32).collect();
        let nonce = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4a, 0x00, 0x00, 0x00, 0x00,
        ];
        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let ct = chacha20_apply(&key, &nonce, 1, plaintext).unwrap();
        let expected = "6e2e359a2568f98041ba0728dd0d6981e97e7aec1d4360c20a27afccfd9fae0bf91b65c5524733ab8f593dabcd62b3571639d624e65152ab8f530c359f0861d807ca0dbf500d6a6156a38e088a22b65e52bc514d16ccf806818ce91ab77937365af90bbf74a35be6b40b8eedf2785e42874d";
        assert_eq!(hex::encode(&ct), expected);
    }

    // RFC 8439 §2.5.2 — Poly1305 MAC test vector.
    #[test]
    fn rfc8439_poly1305_vector() {
        let key = hex::decode("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b").unwrap();
        let mut k = [0u8; 32];
        k.copy_from_slice(&key);
        let msg = b"Cryptographic Forum Research Group";
        let tag = poly1305(&k, msg);
        assert_eq!(hex::encode(tag), "a8061dc1305136c6c22b8baf0c0127a9");
    }

    // RFC 8439 §2.8.2 — AEAD ChaCha20-Poly1305 test vector.
    #[test]
    fn rfc8439_aead_vector() {
        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let aad = hex::decode("50515253c0c1c2c3c4c5c6c7").unwrap();
        let key = hex::decode("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f").unwrap();
        let nonce = hex::decode("070000004041424344454647").unwrap();
        let (ct, tag) = chacha20_poly1305_encrypt(&key, &nonce, &aad, plaintext).unwrap();
        let expected_ct = "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d63dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b3692ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc3ff4def08e4b7a9de576d26586cec64b6116";
        assert_eq!(hex::encode(&ct), expected_ct);
        assert_eq!(hex::encode(tag), "1ae10b594f09e26a7e902ecbd0600691");
    }

    #[test]
    fn stream_roundtrip_text_key() {
        let msg = "The quick brown fox 🦊 jumps over the lazy dog!";
        let key = "0123456789abcdef0123456789abcdef"; // 32 bytes
        let nonce = "0123456789ab"; // 12 bytes
        let ct = encrypt(msg, key, nonce, "", KeyFormat::Text, Mode::Stream, 0, Encoding::Base64).unwrap();
        let pt = decrypt(&ct, key, nonce, "", KeyFormat::Text, Mode::Stream, 0, Encoding::Base64).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn aead_roundtrip_with_aad() {
        let msg = "secret message spanning\nmultiple lines ".repeat(4);
        let key = "0123456789abcdef0123456789abcdef";
        let nonce = "ABCDEFGHIJKL";
        let aad = "header-v1";
        let ct = encrypt(&msg, key, nonce, aad, KeyFormat::Text, Mode::Aead, 0, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, key, nonce, aad, KeyFormat::Text, Mode::Aead, 0, Encoding::Hex).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn aead_roundtrip_encoded_key() {
        let key = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"; // 32-byte hex
        let nonce = "000000000000000000000001"; // 12-byte hex
        let msg = "encoded-key roundtrip";
        let ct = encrypt(msg, key, nonce, "", KeyFormat::Encoded, Mode::Aead, 0, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, key, nonce, "", KeyFormat::Encoded, Mode::Aead, 0, Encoding::Hex).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn aead_tamper_detected() {
        let key = "0123456789abcdef0123456789abcdef";
        let nonce = "0123456789ab";
        let ct = encrypt("authentic", key, nonce, "", KeyFormat::Text, Mode::Aead, 0, Encoding::Hex).unwrap();
        // flip one hex nibble in the ciphertext
        let mut bytes: Vec<u8> = ct.bytes().collect();
        bytes[0] = if bytes[0] == b'0' { b'1' } else { b'0' };
        let tampered = String::from_utf8(bytes).unwrap();
        assert!(decrypt(&tampered, key, nonce, "", KeyFormat::Text, Mode::Aead, 0, Encoding::Hex).is_err());
    }

    #[test]
    fn aead_wrong_aad_detected() {
        let key = "0123456789abcdef0123456789abcdef";
        let nonce = "0123456789ab";
        let ct = encrypt("data", key, nonce, "aad-A", KeyFormat::Text, Mode::Aead, 0, Encoding::Hex).unwrap();
        assert!(decrypt(&ct, key, nonce, "aad-B", KeyFormat::Text, Mode::Aead, 0, Encoding::Hex).is_err());
    }

    #[test]
    fn stream_counter_changes_output() {
        let key = [9u8; 32];
        let nonce = [3u8; 12];
        let a = chacha20_apply(&key, &nonce, 0, &[0u8; 16]).unwrap();
        let b = chacha20_apply(&key, &nonce, 5, &[0u8; 16]).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn stream_multi_block_continuity() {
        let key = [7u8; 32];
        let nonce = [1u8; 12];
        let data = vec![0xABu8; 128];
        let ct = chacha20_apply(&key, &nonce, 0, &data).unwrap();
        let second = chacha20_apply(&key, &nonce, 1, &data[64..]).unwrap();
        assert_eq!(&ct[64..], &second[..]);
    }

    #[test]
    fn errors() {
        assert!(chacha20_apply(b"shortkey", &[0u8; 12], 0, b"x").is_err()); // bad key len
        assert!(chacha20_apply(&[0u8; 32], &[0u8; 8], 0, b"x").is_err()); // bad nonce len
        assert!(Encoding::parse("octal").is_err());
        assert!(KeyFormat::parse("rot13").is_err());
        assert!(Mode::parse("ctr").is_err());
        // AEAD ciphertext too short (no tag)
        assert!(decrypt("00", &"00".repeat(32), &"00".repeat(12), "", KeyFormat::Encoded, Mode::Aead, 0, Encoding::Hex).is_err());
        // bad hex ciphertext
        assert!(decrypt("zzz", &"0".repeat(64), "000000000000000000000000", "", KeyFormat::Encoded, Mode::Stream, 0, Encoding::Hex).is_err());
    }
}
