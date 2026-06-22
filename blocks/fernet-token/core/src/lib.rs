//! fernet-token core — create and read Fernet tokens (spec: AES-128-CBC + HMAC-SHA256).
//!
//! A Fernet key is 32 url-safe-base64 bytes = 16-byte signing key ‖ 16-byte
//! encryption key. A token is url-safe-base64 of:
//!   version(0x80) ‖ timestamp(8B BE seconds) ‖ IV(16B) ‖ ciphertext ‖ HMAC(32B)
//! where ciphertext = AES-128-CBC(PKCS7(plaintext)) and the HMAC-SHA256 covers
//! version‖timestamp‖IV‖ciphertext, keyed by the signing key.
//!
//! The core takes the timestamp and IV explicitly so it is deterministic and
//! testable; each surface supplies its own clock/RNG.

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::engine::general_purpose::{STANDARD, URL_SAFE};
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type HmacSha256 = Hmac<Sha256>;

const VERSION: u8 = 0x80;

/// A parsed Fernet key: 16-byte signing key + 16-byte encryption key.
struct FernetKey {
    signing: [u8; 16],
    encryption: [u8; 16],
}

/// Decode a Fernet key from url-safe (or standard) base64. Must decode to 32 bytes.
fn parse_key(key: &str) -> Result<FernetKey, String> {
    let raw = decode_b64(key.trim()).map_err(|_| "key is not valid base64".to_string())?;
    if raw.len() != 32 {
        return Err(format!(
            "Fernet key must be 32 bytes (got {}). A key is url-safe base64 of 32 random bytes.",
            raw.len()
        ));
    }
    let mut signing = [0u8; 16];
    let mut encryption = [0u8; 16];
    signing.copy_from_slice(&raw[..16]);
    encryption.copy_from_slice(&raw[16..]);
    Ok(FernetKey { signing, encryption })
}

/// Accept either url-safe or standard base64, with or without padding.
fn decode_b64(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    let s = s.trim();
    // Try url-safe first (the Fernet canonical form), then standard.
    URL_SAFE
        .decode(pad(s))
        .or_else(|_| STANDARD.decode(pad(&s.replace('-', "+").replace('_', "/"))))
}

/// Re-pad a possibly-unpadded base64 string to a multiple of 4.
fn pad(s: &str) -> String {
    let mut out = s.to_string();
    while out.len() % 4 != 0 {
        out.push('=');
    }
    out
}

/// Build a Fernet key from 32 raw bytes (url-safe base64, padded form).
pub fn key_from_bytes(raw: &[u8; 32]) -> String {
    URL_SAFE.encode(raw)
}

/// Create a Fernet token. `timestamp` = seconds since the Unix epoch; `iv` = 16
/// random bytes. Returns the url-safe-base64 token (canonical Fernet form).
pub fn encrypt(key: &str, plaintext: &[u8], timestamp: u64, iv: &[u8; 16]) -> Result<String, String> {
    let k = parse_key(key)?;

    // AES-128-CBC with PKCS7 padding.
    let enc = Aes128CbcEnc::new(&k.encryption.into(), iv.into());
    let ciphertext = enc.encrypt_padded_vec_mut::<cipher::block_padding::Pkcs7>(plaintext);

    // Assemble the signed payload: version ‖ timestamp ‖ iv ‖ ciphertext.
    let mut payload = Vec::with_capacity(1 + 8 + 16 + ciphertext.len() + 32);
    payload.push(VERSION);
    payload.extend_from_slice(&timestamp.to_be_bytes());
    payload.extend_from_slice(iv);
    payload.extend_from_slice(&ciphertext);

    // HMAC-SHA256 over everything so far, keyed by the signing key.
    let mut mac = HmacSha256::new_from_slice(&k.signing).expect("hmac key");
    mac.update(&payload);
    let tag = mac.finalize().into_bytes();
    payload.extend_from_slice(&tag);

    Ok(URL_SAFE.encode(&payload))
}

/// What `decrypt` recovers from a token.
#[derive(Debug)]
pub struct Decoded {
    pub plaintext: Vec<u8>,
    /// Token creation time, seconds since the Unix epoch.
    pub timestamp: u64,
}

/// Read (verify + decrypt) a Fernet token.
///
/// - `now` = current time in seconds since the epoch (used only when `ttl` is set).
/// - `ttl` = optional max token age in seconds; `Some(n)` rejects tokens older than
///   `n` seconds (or with a timestamp in the future). `None` skips the TTL check.
pub fn decrypt(key: &str, token: &str, now: u64, ttl: Option<u64>) -> Result<Decoded, String> {
    let k = parse_key(key)?;
    let raw = decode_b64(token).map_err(|_| "token is not valid base64".to_string())?;

    // Minimum: version(1) + timestamp(8) + iv(16) + hmac(32) = 57 bytes.
    if raw.len() < 57 {
        return Err("token too short to be a valid Fernet token".into());
    }
    if raw[0] != VERSION {
        return Err(format!(
            "unsupported token version 0x{:02x} (expected 0x80)",
            raw[0]
        ));
    }

    let hmac_start = raw.len() - 32;
    let (signed, tag) = raw.split_at(hmac_start);

    // Verify the HMAC in constant time before touching the ciphertext.
    let mut mac = HmacSha256::new_from_slice(&k.signing).expect("hmac key");
    mac.update(signed);
    let expected = mac.finalize().into_bytes();
    if expected.ct_eq(tag).unwrap_u8() != 1 {
        return Err("HMAC verification failed: wrong key or the token was tampered with".into());
    }

    let mut ts_bytes = [0u8; 8];
    ts_bytes.copy_from_slice(&signed[1..9]);
    let timestamp = u64::from_be_bytes(ts_bytes);

    if let Some(ttl) = ttl {
        if timestamp > now.saturating_add(60) {
            return Err("token timestamp is in the future (clock skew or invalid token)".into());
        }
        if now.saturating_sub(timestamp) > ttl {
            return Err(format!(
                "token has expired (age {} s exceeds TTL {} s)",
                now.saturating_sub(timestamp),
                ttl
            ));
        }
    }

    let mut iv = [0u8; 16];
    iv.copy_from_slice(&signed[9..25]);
    let ciphertext = &signed[25..];
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return Err("token ciphertext length is invalid".into());
    }

    let dec = Aes128CbcDec::new(&k.encryption.into(), &iv.into());
    let plaintext = dec
        .decrypt_padded_vec_mut::<cipher::block_padding::Pkcs7>(ciphertext)
        .map_err(|_| "decryption failed: invalid padding (wrong key or corrupt token)".to_string())?;

    Ok(Decoded { plaintext, timestamp })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Canonical test vector from the Fernet spec.
    // Secret: cw_0x689RpI-jtRR7oE8h_eQsKImvJapLeSbXpwF4e4=
    // Token (generated at 1985-10-26T08:20:00-07:00 = 499162800):
    //   gAAAAAAdwJ6wAAECAwQFBgcICQoLDA0ODy021cpGVWKZ_eEwCGM4BLLF_5CV9dOPmrhuVUPgJobwOz7JcbmrR64jVmpU4IwqDA==
    // Plaintext: "hello"
    const SPEC_KEY: &str = "cw_0x689RpI-jtRR7oE8h_eQsKImvJapLeSbXpwF4e4=";
    const SPEC_TOKEN: &str = "gAAAAAAdwJ6wAAECAwQFBgcICQoLDA0ODy021cpGVWKZ_eEwCGM4BLLF_5CV9dOPmrhuVUPgJobwOz7JcbmrR64jVmpU4IwqDA==";
    const SPEC_TS: u64 = 499_162_800;

    #[test]
    fn spec_vector_encrypts() {
        let iv: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let tok = encrypt(SPEC_KEY, b"hello", SPEC_TS, &iv).unwrap();
        assert_eq!(tok, SPEC_TOKEN);
    }

    #[test]
    fn spec_vector_decrypts() {
        let d = decrypt(SPEC_KEY, SPEC_TOKEN, SPEC_TS, None).unwrap();
        assert_eq!(d.plaintext, b"hello");
        assert_eq!(d.timestamp, SPEC_TS);
    }

    #[test]
    fn roundtrip() {
        let iv = [7u8; 16];
        let tok = encrypt(SPEC_KEY, b"the quick brown fox", 1_000_000, &iv).unwrap();
        let d = decrypt(SPEC_KEY, &tok, 1_000_000, None).unwrap();
        assert_eq!(d.plaintext, b"the quick brown fox");
    }

    #[test]
    fn empty_plaintext_roundtrips() {
        let iv = [0u8; 16];
        let tok = encrypt(SPEC_KEY, b"", 42, &iv).unwrap();
        let d = decrypt(SPEC_KEY, &tok, 42, None).unwrap();
        assert!(d.plaintext.is_empty());
    }

    #[test]
    fn ttl_within_window_ok() {
        let iv = [1u8; 16];
        let tok = encrypt(SPEC_KEY, b"x", 1000, &iv).unwrap();
        assert!(decrypt(SPEC_KEY, &tok, 1050, Some(60)).is_ok());
    }

    #[test]
    fn ttl_expired_fails() {
        let iv = [1u8; 16];
        let tok = encrypt(SPEC_KEY, b"x", 1000, &iv).unwrap();
        let err = decrypt(SPEC_KEY, &tok, 2000, Some(60)).unwrap_err();
        assert!(err.contains("expired"), "got: {err}");
    }

    #[test]
    fn future_timestamp_fails_with_ttl() {
        let iv = [1u8; 16];
        let tok = encrypt(SPEC_KEY, b"x", 5000, &iv).unwrap();
        let err = decrypt(SPEC_KEY, &tok, 1000, Some(60)).unwrap_err();
        assert!(err.contains("future"), "got: {err}");
    }

    #[test]
    fn wrong_key_fails() {
        let iv = [1u8; 16];
        let tok = encrypt(SPEC_KEY, b"secret", 1000, &iv).unwrap();
        let other = key_from_bytes(&[9u8; 32]);
        let err = decrypt(&other, &tok, 1000, None).unwrap_err();
        assert!(err.contains("HMAC"), "got: {err}");
    }

    #[test]
    fn tampered_token_fails() {
        let iv = [1u8; 16];
        let tok = encrypt(SPEC_KEY, b"secret", 1000, &iv).unwrap();
        let mut bytes = tok.into_bytes();
        // flip a character in the middle (ciphertext region)
        let mid = bytes.len() / 2;
        bytes[mid] = if bytes[mid] == b'A' { b'B' } else { b'A' };
        let bad = String::from_utf8(bytes).unwrap();
        assert!(decrypt(SPEC_KEY, &bad, 1000, None).is_err());
    }

    #[test]
    fn bad_key_length_rejected() {
        let short = URL_SAFE.encode([0u8; 16]);
        let err = encrypt(&short, b"x", 1, &[0u8; 16]).unwrap_err();
        assert!(err.contains("32 bytes"), "got: {err}");
    }

    #[test]
    fn invalid_base64_token_rejected() {
        let err = decrypt(SPEC_KEY, "!!!not base64!!!", 0, None).unwrap_err();
        assert!(err.contains("base64"), "got: {err}");
    }

    #[test]
    fn wrong_version_rejected() {
        // Craft 57 bytes with version 0x79.
        let mut raw = vec![0x79u8];
        raw.extend_from_slice(&[0u8; 56]);
        let tok = URL_SAFE.encode(&raw);
        let err = decrypt(SPEC_KEY, &tok, 0, None).unwrap_err();
        assert!(err.contains("version"), "got: {err}");
    }
}
