//! gizza-ai/encrypt-file core — pure-Rust passphrase file encryption shared by
//! the chat skill block and the CLI. No wafer/wasm-bindgen deps.
//!
//! AES-256-GCM (authenticated) with a key derived from the passphrase via
//! PBKDF2-HMAC-SHA256. The output is a self-describing blob so decryption needs
//! only the passphrase:
//!
//!   magic "GZAE1" (5) | salt (16) | nonce (12) | ciphertext+tag
//!
//! A fresh random salt + nonce are generated per encryption (so the same input
//! never produces the same blob). Decryption fails cleanly on a wrong passphrase
//! or tampered blob (the GCM tag won't verify).

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};

const MAGIC: &[u8; 5] = b"GZAE1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const PBKDF2_ITERS: u32 = 200_000;
const HEADER_LEN: usize = 5 + SALT_LEN + NONCE_LEN;

fn derive_key(passphrase: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    // Generic form avoids the optional `pbkdf2_hmac` convenience feature.
    pbkdf2::pbkdf2::<hmac::Hmac<sha2::Sha256>>(passphrase.as_bytes(), salt, PBKDF2_ITERS, &mut key)
        .expect("HMAC accepts any key length");
    key
}

/// Encrypt `data` with `passphrase`; returns the self-describing blob.
pub fn encrypt(data: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    if passphrase.is_empty() {
        return Err("passphrase is required".into());
    }
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut salt).map_err(|e| format!("rng error: {e}"))?;
    getrandom::getrandom(&mut nonce).map_err(|e| format!("rng error: {e}"))?;

    let key = derive_key(passphrase, &salt);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), data)
        .map_err(|_| "encryption failed".to_string())?;

    let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt a blob produced by [`encrypt`] with `passphrase`.
pub fn decrypt(blob: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    if passphrase.is_empty() {
        return Err("passphrase is required".into());
    }
    if blob.len() < HEADER_LEN || &blob[..5] != MAGIC {
        return Err("not a recognized encrypted file (bad header)".into());
    }
    let salt = &blob[5..5 + SALT_LEN];
    let nonce = &blob[5 + SALT_LEN..HEADER_LEN];
    let ciphertext = &blob[HEADER_LEN..];

    let key = derive_key(passphrase, salt);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| "decryption failed — wrong passphrase or corrupted file".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_recovers_plaintext() {
        let data = b"hello gizza, this is a secret \x00\x01\x02 blob";
        let blob = encrypt(data, "correct horse battery staple").unwrap();
        assert_eq!(&blob[..5], MAGIC);
        assert_ne!(&blob[HEADER_LEN..], &data[..], "ciphertext differs from plaintext");
        let back = decrypt(&blob, "correct horse battery staple").unwrap();
        assert_eq!(back, data);
    }

    #[test]
    fn wrong_passphrase_fails() {
        let blob = encrypt(b"secret", "right").unwrap();
        assert!(decrypt(&blob, "wrong").is_err());
    }

    #[test]
    fn tampered_blob_fails() {
        let mut blob = encrypt(b"secret", "pw").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xff;
        assert!(decrypt(&blob, "pw").is_err());
    }

    #[test]
    fn fresh_salt_nonce_per_encrypt() {
        let a = encrypt(b"x", "pw").unwrap();
        let b = encrypt(b"x", "pw").unwrap();
        assert_ne!(a, b, "same input must not produce the same blob");
    }

    #[test]
    fn bad_header_rejected() {
        assert!(decrypt(b"not an encrypted file at all", "pw").is_err());
    }

    #[test]
    fn empty_passphrase_rejected() {
        assert!(encrypt(b"x", "").is_err());
        assert!(decrypt(b"GZAE1________________________xx", "").is_err());
    }
}
