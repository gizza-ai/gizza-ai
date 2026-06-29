//! gizza-ai/sm2-keypair-generate core — generate an SM2 key pair (Chinese
//! national standard GM/T 0003, OSCCA curve sm2p256v1) and serialize it. No
//! wafer/wasm-bindgen deps. Pure-Rust RustCrypto `sm2`; the CSPRNG is
//! `getrandom` (WASI `random_get` on wasm32-wasip1 / JS crypto on the page), so
//! it runs on every backend.
//!
//! Private key: PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`) + raw 32-byte scalar (hex).
//! Public key:  SPKI PEM   (`-----BEGIN PUBLIC KEY-----`) + the curve point as
//!              SEC1 hex, both uncompressed (65 B, `04 || x || y`) and compressed
//!              (33 B, `02|03 || x`).

use serde::Serialize;
use sm2::elliptic_curve::sec1::ToEncodedPoint;
use sm2::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};

/// A generated SM2 key pair in several encodings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyPair {
    /// Private key, PKCS#8 PEM.
    pub private_pem: String,
    /// Public key, SubjectPublicKeyInfo (SPKI) PEM.
    pub public_pem: String,
    /// Raw 32-byte private scalar, lowercase hex (64 chars).
    pub private_scalar_hex: String,
    /// Public point, SEC1 uncompressed hex: `04 || x || y` (130 chars).
    pub public_point_hex: String,
    /// Public point, SEC1 compressed hex: `02|03 || x` (66 chars).
    pub public_point_hex_compressed: String,
    /// Curve identifier (always `sm2p256v1`).
    pub curve: String,
}

/// Generate a fresh SM2 key pair.
pub fn generate() -> Result<KeyPair, String> {
    let mut rng = rand_core::OsRng;
    let secret = sm2::SecretKey::random(&mut rng);
    let public = secret.public_key();

    let private_pem = secret
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| format!("failed to encode private key: {e}"))?
        .to_string();
    let public_pem = public
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| format!("failed to encode public key: {e}"))?;

    let scalar = secret.to_bytes();
    let uncompressed = public.to_encoded_point(false);
    let compressed = public.to_encoded_point(true);

    Ok(KeyPair {
        private_pem,
        public_pem,
        private_scalar_hex: hex::encode(scalar),
        public_point_hex: hex::encode(uncompressed.as_bytes()),
        public_point_hex_compressed: hex::encode(compressed.as_bytes()),
        curve: "sm2p256v1".to_string(),
    })
}

/// Human-readable rendering (used by the CLI / chat fallback).
pub fn render() -> Result<String, String> {
    let kp = generate()?;
    Ok(format!(
        "{}\n{}\nPrivate scalar (hex):             {}\nPublic point (hex, uncompressed): {}\nPublic point (hex, compressed):   {}\nCurve: {}",
        kp.private_pem.trim_end(),
        kp.public_pem.trim_end(),
        kp.private_scalar_hex,
        kp.public_point_hex,
        kp.public_point_hex_compressed,
        kp.curve,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sm2::pkcs8::DecodePrivateKey;

    #[test]
    fn generates_valid_pem_pair() {
        let kp = generate().unwrap();
        assert!(kp.private_pem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(kp
            .private_pem
            .trim_end()
            .ends_with("-----END PRIVATE KEY-----"));
        assert!(kp.public_pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert!(kp
            .public_pem
            .trim_end()
            .ends_with("-----END PUBLIC KEY-----"));
        assert_eq!(kp.curve, "sm2p256v1");
    }

    #[test]
    fn raw_encodings_have_expected_lengths() {
        let kp = generate().unwrap();
        // SM2 is a 256-bit curve: scalar = 32 B → 64 hex chars.
        assert_eq!(kp.private_scalar_hex.len(), 64);
        // Uncompressed point = 1 + 32 + 32 = 65 B → 130 hex chars, leads with 04.
        assert_eq!(kp.public_point_hex.len(), 130);
        assert!(kp.public_point_hex.starts_with("04"));
        // Compressed point = 1 + 32 = 33 B → 66 hex chars, leads with 02 or 03.
        assert_eq!(kp.public_point_hex_compressed.len(), 66);
        assert!(
            kp.public_point_hex_compressed.starts_with("02")
                || kp.public_point_hex_compressed.starts_with("03")
        );
        // All hex parses.
        assert!(hex::decode(&kp.private_scalar_hex).is_ok());
        assert!(hex::decode(&kp.public_point_hex).is_ok());
    }

    #[test]
    fn private_pem_reparses_and_public_matches() {
        let kp = generate().unwrap();
        let sk = sm2::SecretKey::from_pkcs8_pem(&kp.private_pem).expect("private PEM parses");
        let derived_pub = sk.public_key().to_public_key_pem(LineEnding::LF).unwrap();
        assert_eq!(derived_pub, kp.public_pem, "public PEM matches the private key");
        // The scalar in the PEM matches the reported hex scalar.
        assert_eq!(hex::encode(sk.to_bytes()), kp.private_scalar_hex);
    }

    #[test]
    fn two_calls_differ() {
        let a = generate().unwrap();
        let b = generate().unwrap();
        assert_ne!(
            a.private_scalar_hex, b.private_scalar_hex,
            "each generation is fresh"
        );
        assert_ne!(a.public_point_hex, b.public_point_hex);
    }

    #[test]
    fn render_contains_both_pems() {
        let out = render().unwrap();
        assert!(out.contains("-----BEGIN PRIVATE KEY-----"));
        assert!(out.contains("-----BEGIN PUBLIC KEY-----"));
        assert!(out.contains("sm2p256v1"));
    }
}
