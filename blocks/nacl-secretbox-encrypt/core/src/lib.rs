//! gizza-ai/nacl-secretbox-encrypt core — NaCl `crypto_secretbox` (libsodium
//! `secretbox`): authenticated symmetric encryption with XSalsa20-Poly1305.
//! Pure compute, shared by the chat skill block and the web page.
//!
//! Primitive: XSalsa20 stream cipher + Poly1305 one-time MAC (RustCrypto
//! `crypto_secretbox`, a wasm-safe pure-Rust implementation). Parameters:
//!   * key   — 32 bytes (256-bit)
//!   * nonce — 24 bytes (192-bit); must be unique per message for a given key
//!   * tag   — 16 bytes (Poly1305), appended to the ciphertext
//!
//! The output layout is `nonce || ciphertext || tag`, all encoded together, so a
//! single combined blob is enough to decrypt (the nonce travels with the box).
//! Decryption verifies the Poly1305 tag and fails on any tampering.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use crypto_secretbox::aead::{Aead, KeyInit};
use crypto_secretbox::{Key, Nonce, XSalsa20Poly1305};

/// XSalsa20-Poly1305 key length (bytes).
pub const KEY_LEN: usize = 32;
/// XSalsa20-Poly1305 nonce length (bytes).
pub const NONCE_LEN: usize = 24;
/// Poly1305 authentication tag length (bytes).
pub const TAG_LEN: usize = 16;

/// Encrypt or decrypt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Encrypt,
    Decrypt,
}

impl Operation {
    pub fn parse(s: &str) -> Result<Operation, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "encrypt" | "seal" | "" => Ok(Operation::Encrypt),
            "decrypt" | "open" => Ok(Operation::Decrypt),
            other => Err(format!(
                "unknown operation '{other}' (use encrypt or decrypt)"
            )),
        }
    }
}

/// A binary <-> string encoding. `Text` treats the string as raw UTF-8 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Hex,
    Base64,
    Text,
}

impl Encoding {
    /// Parse an encoding, restricting the accepted set to `allowed` for clear
    /// per-field error messages (`field` names the parameter in errors). The
    /// empty string selects `allowed[0]` (the field's default).
    fn parse_in(s: &str, field: &str, allowed: &[Encoding]) -> Result<Encoding, String> {
        let e = match s.trim().to_ascii_lowercase().as_str() {
            "hex" | "base16" => Encoding::Hex,
            "base64" | "b64" => Encoding::Base64,
            "text" | "utf8" | "utf-8" => Encoding::Text,
            "" => allowed[0],
            other => {
                return Err(format!(
                    "unknown {field} '{other}' (use {})",
                    names(allowed)
                ))
            }
        };
        if allowed.contains(&e) {
            Ok(e)
        } else {
            Err(format!(
                "unsupported {field} '{s}' (use {})",
                names(allowed)
            ))
        }
    }

    /// Parse `key_encoding` (hex | base64 | text; default hex).
    pub fn parse_key(s: &str) -> Result<Encoding, String> {
        Encoding::parse_in(
            s,
            "key_encoding",
            &[Encoding::Hex, Encoding::Base64, Encoding::Text],
        )
    }
    /// Parse `nonce_encoding` (hex | base64; default hex).
    pub fn parse_nonce(s: &str) -> Result<Encoding, String> {
        Encoding::parse_in(s, "nonce_encoding", &[Encoding::Hex, Encoding::Base64])
    }
    /// Parse `data_encoding` (text | hex | base64). The default differs per
    /// operation, so pass the caller's chosen default first.
    pub fn parse_data(s: &str, default: Encoding) -> Result<Encoding, String> {
        // Reorder so `default` is index 0 (used for the empty string).
        let mut allowed = vec![default];
        for e in [Encoding::Text, Encoding::Hex, Encoding::Base64] {
            if e != default {
                allowed.push(e);
            }
        }
        Encoding::parse_in(s, "data_encoding", &allowed)
    }
    /// Parse `output_encoding` (hex | base64; default base64).
    pub fn parse_output(s: &str) -> Result<Encoding, String> {
        Encoding::parse_in(s, "output_encoding", &[Encoding::Base64, Encoding::Hex])
    }

    pub fn decode(self, s: &str) -> Result<Vec<u8>, String> {
        match self {
            Encoding::Text => Ok(s.as_bytes().to_vec()),
            Encoding::Hex => hex::decode(s.trim()).map_err(|e| format!("invalid hex: {e}")),
            Encoding::Base64 => B64
                .decode(s.trim())
                .map_err(|e| format!("invalid base64: {e}")),
        }
    }
    pub fn encode(self, b: &[u8]) -> String {
        match self {
            Encoding::Text => String::from_utf8_lossy(b).into_owned(),
            Encoding::Hex => hex::encode(b),
            Encoding::Base64 => B64.encode(b),
        }
    }
}

fn names(allowed: &[Encoding]) -> String {
    allowed
        .iter()
        .map(|e| match e {
            Encoding::Hex => "hex",
            Encoding::Base64 => "base64",
            Encoding::Text => "text",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn cipher(key: &[u8]) -> Result<XSalsa20Poly1305, String> {
    if key.len() != KEY_LEN {
        return Err(format!(
            "key must be exactly {KEY_LEN} bytes (256-bit), got {}",
            key.len()
        ));
    }
    Ok(XSalsa20Poly1305::new(Key::from_slice(key)))
}

fn check_nonce(nonce: &[u8]) -> Result<(), String> {
    if nonce.len() != NONCE_LEN {
        return Err(format!(
            "nonce must be exactly {NONCE_LEN} bytes (192-bit), got {}",
            nonce.len()
        ));
    }
    Ok(())
}

/// NaCl `crypto_secretbox`: seal `plaintext` under `key`/`nonce`. Returns the
/// `ciphertext || tag` box (the 16-byte Poly1305 tag is appended). Does NOT
/// prepend the nonce.
pub fn seal(key: &[u8], nonce: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let c = cipher(key)?;
    check_nonce(nonce)?;
    c.encrypt(Nonce::from_slice(nonce), plaintext)
        .map_err(|_| "encryption failed".to_string())
}

/// NaCl `crypto_secretbox_open`: verify + decrypt a `ciphertext || tag` box.
/// Fails if the tag does not authenticate (wrong key/nonce or tampering).
pub fn open(key: &[u8], nonce: &[u8], box_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let c = cipher(key)?;
    check_nonce(nonce)?;
    if box_bytes.len() < TAG_LEN {
        return Err(format!(
            "ciphertext too short ({} bytes): expected at least a {TAG_LEN}-byte tag",
            box_bytes.len()
        ));
    }
    c.decrypt(Nonce::from_slice(nonce), box_bytes).map_err(|_| {
        "authentication failed: the Poly1305 tag does not verify (wrong key/nonce or tampered ciphertext)"
            .to_string()
    })
}

/// Encrypt `data` and return `output_encoding(nonce || ciphertext || tag)`.
///
/// `data` is decoded with `data_enc` (text/hex/base64), the key with `key_enc`,
/// the nonce with `nonce_enc`. A nonce is REQUIRED (deterministic — no randomness
/// so callers can reproduce/test the output). The nonce is prepended to the box
/// so [`decrypt`] can recover it from the combined blob.
pub fn encrypt(
    data: &str,
    key: &str,
    nonce: &str,
    key_enc: Encoding,
    nonce_enc: Encoding,
    data_enc: Encoding,
    out_enc: Encoding,
) -> Result<String, String> {
    if nonce.trim().is_empty() {
        return Err(format!(
            "a nonce is required for encryption: supply a unique {NONCE_LEN}-byte value"
        ));
    }
    let key_bytes = key_enc.decode(key)?;
    let nonce_bytes = nonce_enc.decode(nonce)?;
    let plaintext = data_enc.decode(data)?;
    let box_bytes = seal(&key_bytes, &nonce_bytes, &plaintext)?;
    let mut combined = Vec::with_capacity(nonce_bytes.len() + box_bytes.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&box_bytes);
    Ok(out_enc.encode(&combined))
}

/// Decrypt a box produced by [`encrypt`]. `data` is `data_enc`-decoded
/// (hex/base64). If `nonce` is non-empty it is used and `data` is treated as
/// `ciphertext || tag`; otherwise the leading [`NONCE_LEN`] bytes of the combined
/// blob are taken as the nonce. Returns the recovered plaintext as UTF-8 text
/// when valid, else `output_encoding`-encoded bytes.
pub fn decrypt(
    data: &str,
    key: &str,
    nonce: &str,
    key_enc: Encoding,
    nonce_enc: Encoding,
    data_enc: Encoding,
    out_enc: Encoding,
) -> Result<String, String> {
    let key_bytes = key_enc.decode(key)?;
    let blob = data_enc.decode(data)?;

    let (nonce_bytes, box_bytes): (Vec<u8>, &[u8]) = if nonce.trim().is_empty() {
        if blob.len() < NONCE_LEN + TAG_LEN {
            return Err(format!(
                "combined ciphertext too short ({} bytes): expected a {NONCE_LEN}-byte nonce + box, or pass the nonce separately",
                blob.len()
            ));
        }
        (blob[..NONCE_LEN].to_vec(), &blob[NONCE_LEN..])
    } else {
        (nonce_enc.decode(nonce)?, &blob[..])
    };

    let plaintext = open(&key_bytes, &nonce_bytes, box_bytes)?;
    match String::from_utf8(plaintext) {
        Ok(text) => Ok(text),
        Err(e) => Ok(out_enc.encode(e.as_bytes())),
    }
}

/// High-level tool entrypoint used by the chat/CLI/page layers.
pub fn run(
    operation: &str,
    data: &str,
    key: &str,
    nonce: &str,
    key_encoding: &str,
    nonce_encoding: &str,
    data_encoding: &str,
    output_encoding: &str,
) -> Result<String, String> {
    let op = Operation::parse(operation)?;
    let key_enc = Encoding::parse_key(key_encoding)?;
    let nonce_enc = Encoding::parse_nonce(nonce_encoding)?;
    let out_enc = Encoding::parse_output(output_encoding)?;
    match op {
        Operation::Encrypt => {
            let data_enc = Encoding::parse_data(data_encoding, Encoding::Text)?;
            encrypt(data, key, nonce, key_enc, nonce_enc, data_enc, out_enc)
        }
        Operation::Decrypt => {
            let data_enc = Encoding::parse_data(data_encoding, Encoding::Base64)?;
            decrypt(data, key, nonce, key_enc, nonce_enc, data_enc, out_enc)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k32() -> String {
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff".to_string()
    }
    fn n24() -> String {
        "000102030405060708090a0b0c0d0e0f1011121314151617".to_string()
    }

    #[test]
    fn seal_appends_tag() {
        let key = [7u8; KEY_LEN];
        let nonce = [3u8; NONCE_LEN];
        let box_bytes = seal(&key, &nonce, b"hello").unwrap();
        assert_eq!(box_bytes.len(), 5 + TAG_LEN);
        let pt = open(&key, &nonce, &box_bytes).unwrap();
        assert_eq!(pt, b"hello");
    }

    #[test]
    fn combined_roundtrip_hex() {
        let msg = "The quick brown fox 🦊 jumps over the lazy dog!";
        let ct = encrypt(
            msg,
            &k32(),
            &n24(),
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Text,
            Encoding::Hex,
        )
        .unwrap();
        // combined blob decrypts with the nonce carried inline (no nonce arg).
        let pt = decrypt(
            &ct,
            &k32(),
            "",
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Base64,
        )
        .unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn combined_roundtrip_base64_default() {
        let msg = "attack at dawn";
        let ct = encrypt(
            msg,
            &k32(),
            &n24(),
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Text,
            Encoding::Base64,
        )
        .unwrap();
        let pt = decrypt(
            &ct,
            &k32(),
            "",
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Base64,
            Encoding::Base64,
        )
        .unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn decrypt_with_separate_nonce() {
        // Strip the leading nonce off the combined blob, then pass it separately.
        let msg = "separate nonce path";
        let ct_hex = encrypt(
            msg,
            &k32(),
            &n24(),
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Text,
            Encoding::Hex,
        )
        .unwrap();
        let combined = hex::decode(&ct_hex).unwrap();
        let box_only = hex::encode(&combined[NONCE_LEN..]);
        let pt = decrypt(
            &box_only,
            &k32(),
            &n24(),
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Base64,
        )
        .unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn text_key_and_base64_nonce() {
        let key = "0123456789abcdef0123456789abcdef"; // 32 UTF-8 bytes
        let nonce_b64 = B64.encode([9u8; NONCE_LEN]);
        let ct = encrypt(
            "mixed encodings",
            key,
            &nonce_b64,
            Encoding::Text,
            Encoding::Base64,
            Encoding::Text,
            Encoding::Base64,
        )
        .unwrap();
        let pt = decrypt(
            &ct,
            key,
            "",
            Encoding::Text,
            Encoding::Base64,
            Encoding::Base64,
            Encoding::Base64,
        )
        .unwrap();
        assert_eq!(pt, "mixed encodings");
    }

    #[test]
    fn tamper_is_rejected() {
        let ct_hex = encrypt(
            "authentic",
            &k32(),
            &n24(),
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Text,
            Encoding::Hex,
        )
        .unwrap();
        let mut bytes = hex::decode(&ct_hex).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01; // flip a bit in the tag
        let tampered = hex::encode(&bytes);
        assert!(decrypt(
            &tampered,
            &k32(),
            "",
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Base64
        )
        .is_err());
    }

    #[test]
    fn wrong_key_is_rejected() {
        let ct = encrypt(
            "secret",
            &k32(),
            &n24(),
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Text,
            Encoding::Hex,
        )
        .unwrap();
        let wrong = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        assert!(decrypt(
            &ct,
            wrong,
            "",
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Base64
        )
        .is_err());
    }

    #[test]
    fn encrypt_requires_nonce() {
        assert!(encrypt(
            "x",
            &k32(),
            "",
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Text,
            Encoding::Base64
        )
        .is_err());
    }

    #[test]
    fn length_validation() {
        assert!(seal(b"shortkey", &[0u8; NONCE_LEN], b"x").is_err());
        assert!(seal(&[0u8; KEY_LEN], &[0u8; 8], b"x").is_err());
        assert!(open(&[0u8; KEY_LEN], &[0u8; NONCE_LEN], &[0u8; 4]).is_err());
    }

    #[test]
    fn encoding_parsing() {
        assert_eq!(Encoding::parse_key("").unwrap(), Encoding::Hex);
        assert_eq!(Encoding::parse_output("").unwrap(), Encoding::Base64);
        assert_eq!(
            Encoding::parse_data("", Encoding::Text).unwrap(),
            Encoding::Text
        );
        assert_eq!(
            Encoding::parse_data("", Encoding::Hex).unwrap(),
            Encoding::Hex
        );
        assert!(Encoding::parse_nonce("text").is_err()); // text not allowed for nonce
        assert!(Encoding::parse_output("text").is_err());
        assert!(Encoding::parse_key("octal").is_err());
        assert!(Operation::parse("frobnicate").is_err());
    }
}
