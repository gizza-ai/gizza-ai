//! gizza-ai/ed25519-key-pair-generator core — generate an Ed25519 key pair and
//! serialize to standard encodings. No wafer/wasm-bindgen deps. Pure-Rust
//! `ed25519-dalek`; the CSPRNG is `getrandom` (WASI `random_get` on
//! wasm32-wasip1 / JS crypto on the page), so it runs on every backend.
//!
//! Private key: PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`) + raw 32-byte seed.
//! Public key:  SPKI PEM   (`-----BEGIN PUBLIC KEY-----`) + raw 32-byte point.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
use ed25519_dalek::SigningKey;
use serde::Serialize;

/// A generated Ed25519 key pair in several encodings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyPair {
    pub private_pem: String,
    pub public_pem: String,
    /// Raw 32-byte private seed, base64 / hex.
    pub private_base64: String,
    pub private_hex: String,
    /// Raw 32-byte public key, base64 / hex.
    pub public_base64: String,
    pub public_hex: String,
}

/// Generate a fresh Ed25519 key pair.
pub fn generate() -> Result<KeyPair, String> {
    let mut rng = rand::rngs::OsRng;
    let signing = SigningKey::generate(&mut rng);
    let verifying = signing.verifying_key();

    let private_pem = signing
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| format!("failed to encode private key: {e}"))?
        .to_string();
    let public_pem = verifying
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| format!("failed to encode public key: {e}"))?;

    let priv_raw = signing.to_bytes();
    let pub_raw = verifying.to_bytes();

    Ok(KeyPair {
        private_pem,
        public_pem,
        private_base64: B64.encode(priv_raw),
        private_hex: hex::encode(priv_raw),
        public_base64: B64.encode(pub_raw),
        public_hex: hex::encode(pub_raw),
    })
}

/// Human-readable rendering (used by the page).
pub fn render() -> Result<String, String> {
    let kp = generate()?;
    Ok(format!(
        "{}\n{}\nPrivate key (raw, base64): {}\nPrivate key (raw, hex):    {}\nPublic key (raw, base64):  {}\nPublic key (raw, hex):     {}",
        kp.private_pem.trim_end(),
        kp.public_pem.trim_end(),
        kp.private_base64,
        kp.private_hex,
        kp.public_base64,
        kp.public_hex,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::pkcs8::DecodePrivateKey;
    use ed25519_dalek::Verifier;
    use ed25519_dalek::{Signer, VerifyingKey};

    #[test]
    fn generates_valid_pem_pair() {
        let kp = generate().unwrap();
        assert!(kp.private_pem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(kp.private_pem.trim_end().ends_with("-----END PRIVATE KEY-----"));
        assert!(kp.public_pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        // Ed25519 raw keys are 32 bytes → 64 hex chars.
        assert_eq!(kp.private_hex.len(), 64);
        assert_eq!(kp.public_hex.len(), 64);
        assert_eq!(B64.decode(&kp.public_base64).unwrap().len(), 32);
    }

    #[test]
    fn private_pem_reparses_and_public_matches() {
        let kp = generate().unwrap();
        let sk = SigningKey::from_pkcs8_pem(&kp.private_pem).expect("private PEM parses");
        let derived_pub = sk.verifying_key().to_public_key_pem(LineEnding::LF).unwrap();
        assert_eq!(derived_pub, kp.public_pem, "public PEM matches the private key");
    }

    #[test]
    fn key_pair_actually_signs_and_verifies() {
        let kp = generate().unwrap();
        let sk = SigningKey::from_pkcs8_pem(&kp.private_pem).unwrap();
        let pub_raw: [u8; 32] = B64.decode(&kp.public_base64).unwrap().try_into().unwrap();
        let vk = VerifyingKey::from_bytes(&pub_raw).unwrap();
        let sig = sk.sign(b"test message");
        vk.verify(b"test message", &sig).expect("signature verifies under the public key");
    }

    #[test]
    fn two_calls_differ() {
        let a = generate().unwrap();
        let b = generate().unwrap();
        assert_ne!(a.private_hex, b.private_hex, "each generation is fresh");
    }
}
