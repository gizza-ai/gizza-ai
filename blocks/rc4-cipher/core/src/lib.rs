//! gizza-ai/rc4-cipher core — encrypt or decrypt data with the RC4 stream cipher,
//! with an optional drop-N (RC4-drop / RC4-drop[n]) to discard the weak initial
//! keystream bytes. Pure Rust, no dependencies beyond hex/base64 for I/O.
//!
//! RC4 is a symmetric stream cipher: the same operation encrypts and decrypts
//! (plaintext XOR keystream). It is **cryptographically broken** and must not be
//! used to protect real secrets — this tool is for interop, CTFs, legacy systems
//! and learning. For real encryption use `aes-cipher` / `text-encrypt`.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// Binary encoding for the key (when given as bytes) and the ciphertext.
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

/// How the key string is interpreted.
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

/// Apply the RC4 keystream to `data`, dropping the first `drop` keystream bytes
/// (RC4-drop[drop]). RC4 is symmetric, so this both encrypts and decrypts.
///
/// The key must be 1..=256 bytes. Returns an error otherwise.
pub fn rc4_apply(key: &[u8], drop: usize, data: &[u8]) -> Result<Vec<u8>, String> {
    if key.is_empty() {
        return Err("key must not be empty".into());
    }
    if key.len() > 256 {
        return Err(format!("RC4 key must be 1..=256 bytes, got {}", key.len()));
    }

    // Key-scheduling algorithm (KSA).
    let mut s: [u8; 256] = [0u8; 256];
    for (i, b) in s.iter_mut().enumerate() {
        *b = i as u8;
    }
    let mut j: usize = 0;
    for i in 0..256 {
        j = (j + s[i] as usize + key[i % key.len()] as usize) & 0xff;
        s.swap(i, j);
    }

    // Pseudo-random generation algorithm (PRGA), discarding `drop` bytes first.
    let mut i: usize = 0;
    let mut j: usize = 0;
    let mut next = || -> u8 {
        i = (i + 1) & 0xff;
        j = (j + s[i] as usize) & 0xff;
        s.swap(i, j);
        let t = (s[i] as usize + s[j] as usize) & 0xff;
        s[t]
    };

    for _ in 0..drop {
        next();
    }

    let mut out = Vec::with_capacity(data.len());
    for &byte in data {
        out.push(byte ^ next());
    }
    Ok(out)
}

fn resolve_key(key_str: &str, key_format: KeyFormat, fmt: Encoding) -> Result<Vec<u8>, String> {
    match key_format {
        KeyFormat::Text => Ok(key_str.as_bytes().to_vec()),
        KeyFormat::Encoded => fmt.decode(key_str),
    }
}

/// Encrypt UTF-8 `plaintext`. Returns the ciphertext encoded with `fmt`.
pub fn encrypt(
    plaintext: &str,
    key_str: &str,
    key_format: KeyFormat,
    drop: usize,
    fmt: Encoding,
) -> Result<String, String> {
    let key = resolve_key(key_str, key_format, fmt)?;
    let ct = rc4_apply(&key, drop, plaintext.as_bytes())?;
    Ok(fmt.encode(&ct))
}

/// Decrypt `ciphertext` (encoded with `fmt`). Returns the recovered UTF-8 text.
pub fn decrypt(
    ciphertext: &str,
    key_str: &str,
    key_format: KeyFormat,
    drop: usize,
    fmt: Encoding,
) -> Result<String, String> {
    let key = resolve_key(key_str, key_format, fmt)?;
    let ct = fmt.decode(ciphertext)?;
    let pt = rc4_apply(&key, drop, &ct)?;
    String::from_utf8(pt).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Classic RC4 test vectors (Wikipedia): key bytes XOR'd with ASCII plaintext.
    #[test]
    fn vector_key_plaintext() {
        let out = rc4_apply(b"Key", 0, b"Plaintext").unwrap();
        assert_eq!(hex::encode_upper(&out), "BBF316E8D940AF0AD3");
    }

    #[test]
    fn vector_wiki_pedia() {
        let out = rc4_apply(b"Wiki", 0, b"pedia").unwrap();
        assert_eq!(hex::encode_upper(&out), "1021BF0420");
    }

    #[test]
    fn vector_secret_attack() {
        let out = rc4_apply(b"Secret", 0, b"Attack at dawn").unwrap();
        assert_eq!(hex::encode_upper(&out), "45A01F645FC35B383552544B9BF5");
    }

    #[test]
    fn encrypt_decrypt_roundtrip_text_key() {
        let msg = "The quick brown fox 🦊 jumps!";
        let ct = encrypt(msg, "s3cr3t", KeyFormat::Text, 0, Encoding::Base64).unwrap();
        let pt = decrypt(&ct, "s3cr3t", KeyFormat::Text, 0, Encoding::Base64).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn roundtrip_with_drop() {
        let msg = "drop the weak prefix";
        let ct = encrypt(msg, "0badc0de", KeyFormat::Encoded, 768, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, "0badc0de", KeyFormat::Encoded, 768, Encoding::Hex).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn drop_changes_output() {
        let a = rc4_apply(b"Key", 0, b"hello").unwrap();
        let b = rc4_apply(b"Key", 256, b"hello").unwrap();
        assert_ne!(a, b, "dropping keystream bytes must change the output");
    }

    #[test]
    fn wrong_key_does_not_recover() {
        let ct = encrypt("hello world", "right-key", KeyFormat::Text, 0, Encoding::Hex).unwrap();
        match decrypt(&ct, "wrong-key", KeyFormat::Text, 0, Encoding::Hex) {
            Ok(pt) => assert_ne!(pt, "hello world"),
            Err(_) => {}
        }
    }

    #[test]
    fn errors() {
        assert!(rc4_apply(b"", 0, b"x").is_err()); // empty key
        assert!(rc4_apply(&[0u8; 257], 0, b"x").is_err()); // key too long
        assert!(Encoding::parse("octal").is_err());
        assert!(KeyFormat::parse("rot13").is_err());
        assert!(decrypt("zzz", "k", KeyFormat::Text, 0, Encoding::Hex).is_err()); // bad hex
    }
}
