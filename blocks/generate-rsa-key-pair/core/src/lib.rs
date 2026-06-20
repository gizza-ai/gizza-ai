//! gizza-ai/generate-rsa-key-pair core — generate an RSA key pair and serialize
//! to PEM. No wafer/wasm-bindgen deps. Pure-Rust `rsa` crate; the CSPRNG is
//! `getrandom` (WASI `random_get` on wasm32-wasip1), so it runs on every backend.
//!
//! Private key: PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`).
//! Public key:  SPKI PEM   (`-----BEGIN PUBLIC KEY-----`).

use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::{RsaPrivateKey, RsaPublicKey};

/// A generated key pair, both in PEM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPair {
    pub private_pem: String,
    pub public_pem: String,
    pub bits: usize,
}

/// Allowed key sizes (bits).
pub fn validate_bits(bits: usize) -> Result<(), String> {
    match bits {
        2048 | 3072 | 4096 => Ok(()),
        other => Err(format!("key size {other} not supported (2048, 3072, or 4096)")),
    }
}

/// Generate an RSA key pair of `bits` size and return both keys in PEM.
pub fn generate(bits: usize) -> Result<KeyPair, String> {
    validate_bits(bits)?;
    let mut rng = rand::rngs::OsRng;
    let private = RsaPrivateKey::new(&mut rng, bits)
        .map_err(|e| format!("RSA key generation failed: {e}"))?;
    let public = RsaPublicKey::from(&private);

    let private_pem = private
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| format!("failed to encode private key: {e}"))?
        .to_string();
    let public_pem = public
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| format!("failed to encode public key: {e}"))?;

    Ok(KeyPair { private_pem, public_pem, bits })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs8::DecodePrivateKey;
    use rsa::traits::PublicKeyParts;

    #[test]
    fn validate_bits_accepts_known_and_rejects_others() {
        assert!(validate_bits(2048).is_ok());
        assert!(validate_bits(3072).is_ok());
        assert!(validate_bits(4096).is_ok());
        assert!(validate_bits(1024).is_err());
        assert!(validate_bits(512).is_err());
    }

    #[test]
    fn generates_valid_2048_pem_pair() {
        // 2048 keeps the test fast while exercising the full path.
        let kp = generate(2048).unwrap();
        assert!(kp.private_pem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(kp.private_pem.trim_end().ends_with("-----END PRIVATE KEY-----"));
        assert!(kp.public_pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert_eq!(kp.bits, 2048);

        // The private PEM must re-parse, and its size must be 2048 bits.
        let parsed = RsaPrivateKey::from_pkcs8_pem(&kp.private_pem).expect("private PEM parses");
        assert_eq!(parsed.size() * 8, 2048, "modulus size should be 2048 bits");

        // The public PEM must correspond to the private key.
        let derived = RsaPublicKey::from(&parsed)
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        assert_eq!(derived, kp.public_pem, "public PEM matches the private key");
    }

    #[test]
    fn two_calls_differ() {
        let a = generate(2048).unwrap();
        let b = generate(2048).unwrap();
        assert_ne!(a.private_pem, b.private_pem, "each generation is fresh");
    }
}
