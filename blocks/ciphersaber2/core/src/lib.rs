//! gizza-ai/ciphersaber2 core — encrypt or decrypt data with CipherSaber-2.
//!
//! CipherSaber is a minimalist symmetric cipher built on RC4 (ARCFOUR). Each
//! message is encrypted with a *session key* formed by concatenating the user
//! key with a fresh random **10-byte initialization vector (IV)**, then running
//! RC4's key-scheduling algorithm (KSA). CipherSaber-**2** strengthens the
//! original by repeating the KSA mixing loop `rounds` times (the spec recommends
//! 20) to better diffuse the key — this defends against the FMS key-recovery
//! attack on plain RC4 that breaks CipherSaber-1.
//!
//! The 10-byte IV is prepended to the ciphertext in the clear, so a decryptor
//! who knows the key and `rounds` can reproduce the session key. The IV must be
//! unique per message; reuse leaks plaintext (it is a stream cipher).
//!
//! Wire format: `IV(10 bytes) || RC4(plaintext)`, then encoded as hex or base64.
//! Pure Rust; `getrandom` supplies the CSPRNG (wasi `random_get` on
//! wasm32-wasip1, the JS crypto RNG in the browser with the `js` feature, and
//! the OS RNG natively).
//!
//! WARNING: RC4 is cryptographically broken; CipherSaber is a hobby/learning
//! cipher. Use it for interop, CTFs and education only — for real encryption use
//! `aes-cipher` / `text-encrypt` / `encrypt-file`.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// The CipherSaber IV is always 10 bytes.
pub const IV_LEN: usize = 10;

/// Binary encoding for the ciphertext (and the key, when given as bytes).
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

/// How the user key string is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFormat {
    /// The key is a UTF-8 passphrase used as raw bytes.
    Text,
    /// The key is encoded (hex or base64), matching `format`.
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

/// Run the RC4 keystream over `data`, keyed by `session_key`, with the KSA
/// mixing loop repeated `rounds` times (CipherSaber-2). RC4 is symmetric so the
/// same call both encrypts and decrypts. `session_key` is the user key with the
/// 10-byte IV appended; it must be 1..=256 bytes and `rounds` must be >= 1.
fn rc4_apply(session_key: &[u8], rounds: u32, data: &[u8]) -> Result<Vec<u8>, String> {
    if session_key.is_empty() {
        return Err("key must not be empty".into());
    }
    if session_key.len() > 256 {
        // user key + 10-byte IV must fit in 256.
        return Err(format!(
            "key + IV must be 1..=256 bytes, got {} (use a shorter key)",
            session_key.len()
        ));
    }
    if rounds == 0 {
        return Err("rounds must be >= 1".into());
    }

    // Key-scheduling algorithm (KSA), repeated `rounds` times. The S box and j
    // carry over between rounds — this is the CipherSaber-2 strengthening.
    let mut s: [u8; 256] = [0u8; 256];
    for (i, b) in s.iter_mut().enumerate() {
        *b = i as u8;
    }
    let mut j: usize = 0;
    for _ in 0..rounds {
        for i in 0..256 {
            j = (j + s[i] as usize + session_key[i % session_key.len()] as usize) & 0xff;
            s.swap(i, j);
        }
    }

    // Pseudo-random generation algorithm (PRGA).
    let mut i: usize = 0;
    let mut j: usize = 0;
    let mut next = || -> u8 {
        i = (i + 1) & 0xff;
        j = (j + s[i] as usize) & 0xff;
        s.swap(i, j);
        let t = (s[i] as usize + s[j] as usize) & 0xff;
        s[t]
    };

    let mut out = Vec::with_capacity(data.len());
    for &byte in data {
        out.push(byte ^ next());
    }
    Ok(out)
}

fn resolve_key(key_str: &str, key_format: KeyFormat, fmt: Encoding) -> Result<Vec<u8>, String> {
    let k = match key_format {
        KeyFormat::Text => key_str.as_bytes().to_vec(),
        KeyFormat::Encoded => fmt.decode(key_str)?,
    };
    if k.is_empty() {
        return Err("key must not be empty".into());
    }
    Ok(k)
}

/// Generate a random 10-byte IV using the platform CSPRNG.
fn random_iv() -> Result<[u8; IV_LEN], String> {
    let mut iv = [0u8; IV_LEN];
    getrandom::getrandom(&mut iv).map_err(|e| format!("rng error: {e}"))?;
    Ok(iv)
}

/// Encrypt UTF-8 `plaintext` with CipherSaber-2. A fresh random IV is generated
/// unless `iv` is supplied (`Some` is used for deterministic tests / interop).
/// Returns `encode(IV || RC4(plaintext))`.
pub fn encrypt(
    plaintext: &str,
    key_str: &str,
    key_format: KeyFormat,
    rounds: u32,
    iv: Option<[u8; IV_LEN]>,
    fmt: Encoding,
) -> Result<String, String> {
    let user_key = resolve_key(key_str, key_format, fmt)?;
    let iv = match iv {
        Some(iv) => iv,
        None => random_iv()?,
    };
    let mut session_key = user_key;
    session_key.extend_from_slice(&iv);
    let ct = rc4_apply(&session_key, rounds, plaintext.as_bytes())?;
    let mut out = Vec::with_capacity(IV_LEN + ct.len());
    out.extend_from_slice(&iv);
    out.extend_from_slice(&ct);
    Ok(fmt.encode(&out))
}

/// Decrypt CipherSaber-2 `ciphertext` (encoded `IV || RC4(plaintext)`). Reads
/// the IV from the first 10 bytes, rebuilds the session key and recovers the
/// UTF-8 plaintext.
pub fn decrypt(
    ciphertext: &str,
    key_str: &str,
    key_format: KeyFormat,
    rounds: u32,
    fmt: Encoding,
) -> Result<String, String> {
    let user_key = resolve_key(key_str, key_format, fmt)?;
    let raw = fmt.decode(ciphertext)?;
    if raw.len() < IV_LEN {
        return Err(format!(
            "ciphertext too short: need at least {IV_LEN} bytes for the IV, got {}",
            raw.len()
        ));
    }
    let (iv, body) = raw.split_at(IV_LEN);
    let mut session_key = user_key;
    session_key.extend_from_slice(iv);
    let pt = rc4_apply(&session_key, rounds, body)?;
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Deterministic encrypt with a fixed IV so the wire bytes are reproducible.
    #[test]
    fn fixed_iv_roundtrip() {
        let iv = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let msg = "Attack at dawn";
        let ct = encrypt(msg, "asecretkey", KeyFormat::Text, 20, Some(iv), Encoding::Hex).unwrap();
        // first 10 bytes (20 hex chars) are the IV, in the clear.
        assert_eq!(&ct[..20], &hex::encode(iv));
        let pt = decrypt(&ct, "asecretkey", KeyFormat::Text, 20, Encoding::Hex).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn random_iv_roundtrip_text_key() {
        let msg = "The quick brown fox 🦊 jumps!";
        let ct = encrypt(msg, "s3cr3t", KeyFormat::Text, 20, None, Encoding::Base64).unwrap();
        let pt = decrypt(&ct, "s3cr3t", KeyFormat::Text, 20, Encoding::Base64).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn random_iv_differs_each_call() {
        // Same key + plaintext must produce different ciphertext (different IV).
        let a = encrypt("hello", "k", KeyFormat::Text, 20, None, Encoding::Hex).unwrap();
        let b = encrypt("hello", "k", KeyFormat::Text, 20, None, Encoding::Hex).unwrap();
        assert_ne!(a, b, "fresh IV must randomize the ciphertext");
    }

    #[test]
    fn rounds_must_match() {
        let iv = [0u8; IV_LEN];
        let ct = encrypt("payload", "key", KeyFormat::Text, 20, Some(iv), Encoding::Hex).unwrap();
        // decrypting with the wrong round count must not recover the plaintext.
        let got = decrypt(&ct, "key", KeyFormat::Text, 1, Encoding::Hex);
        match got {
            Ok(pt) => assert_ne!(pt, "payload"),
            Err(_) => {}
        }
    }

    #[test]
    fn encoded_key_roundtrip() {
        let msg = "encoded key path";
        let ct = encrypt(msg, "0badc0de", KeyFormat::Encoded, 30, Some([9u8; IV_LEN]), Encoding::Hex)
            .unwrap();
        let pt = decrypt(&ct, "0badc0de", KeyFormat::Encoded, 30, Encoding::Hex).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn wrong_key_does_not_recover() {
        let ct =
            encrypt("hello world", "right-key", KeyFormat::Text, 20, Some([2u8; IV_LEN]), Encoding::Hex)
                .unwrap();
        match decrypt(&ct, "wrong-key", KeyFormat::Text, 20, Encoding::Hex) {
            Ok(pt) => assert_ne!(pt, "hello world"),
            Err(_) => {}
        }
    }

    #[test]
    fn errors() {
        // empty key
        assert!(encrypt("x", "", KeyFormat::Text, 20, Some([0u8; IV_LEN]), Encoding::Hex).is_err());
        // rounds = 0
        assert!(encrypt("x", "k", KeyFormat::Text, 0, Some([0u8; IV_LEN]), Encoding::Hex).is_err());
        // bad encoding parse
        assert!(Encoding::parse("octal").is_err());
        assert!(KeyFormat::parse("rot13").is_err());
        // ciphertext shorter than the IV
        assert!(decrypt("00", "k", KeyFormat::Text, 20, Encoding::Hex).is_err());
        // invalid hex
        assert!(decrypt("zzz", "k", KeyFormat::Text, 20, Encoding::Hex).is_err());
    }
}
