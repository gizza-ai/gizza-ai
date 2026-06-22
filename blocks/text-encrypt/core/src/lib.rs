//! gizza-ai/text-encrypt core — encrypt or decrypt text with a passphrase using
//! AES-256-GCM. Pure-Rust. Reuses the proven `encrypt-file` crypto core (the same
//! self-describing blob: magic | salt | nonce | ciphertext+tag, key derived via
//! PBKDF2-HMAC-SHA256), wrapping the binary blob as base64 so the result is a
//! compact, copy-pasteable token.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// Encrypt `text` with `passphrase`; returns a base64 token.
pub fn encrypt_text(text: &str, passphrase: &str) -> Result<String, String> {
    let blob = gizza_ai_encrypt_file_core::encrypt(text.as_bytes(), passphrase)?;
    Ok(B64.encode(blob))
}

/// Decrypt a base64 token produced by [`encrypt_text`] with `passphrase`.
pub fn decrypt_text(token: &str, passphrase: &str) -> Result<String, String> {
    let blob = B64
        .decode(token.trim().as_bytes())
        .map_err(|_| "input is not a valid base64 token".to_string())?;
    let plain = gizza_ai_encrypt_file_core::decrypt(&blob, passphrase)?;
    String::from_utf8(plain).map_err(|_| "decrypted data is not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let token = encrypt_text("hello secret 🤫", "hunter2").unwrap();
        // token is base64 and not the plaintext
        assert!(!token.contains("hello"));
        let back = decrypt_text(&token, "hunter2").unwrap();
        assert_eq!(back, "hello secret 🤫");
    }

    #[test]
    fn wrong_passphrase_fails() {
        let token = encrypt_text("top secret", "correct").unwrap();
        assert!(decrypt_text(&token, "wrong").is_err());
    }

    #[test]
    fn nondeterministic() {
        // Fresh salt+nonce each time -> different tokens for the same input.
        let a = encrypt_text("x", "pw").unwrap();
        let b = encrypt_text("x", "pw").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn errors() {
        assert!(encrypt_text("x", "").is_err()); // empty passphrase
        assert!(decrypt_text("not base64!!!", "pw").is_err());
        assert!(decrypt_text("aGVsbG8=", "pw").is_err()); // valid base64, bad header
    }
}
