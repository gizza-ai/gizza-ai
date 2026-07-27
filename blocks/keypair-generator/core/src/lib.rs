//! gizza-ai/keypair-generator core — generate an X25519 or Ed25519 key pair and
//! serialize it to hex, base64, and PEM. No wafer/wasm-bindgen deps.
//!
//! * **Ed25519** — EdDSA signing keys (SSH, OpenPGP, JWT EdDSA, TLS). Uses
//!   pure-Rust `ed25519-dalek`, whose `pkcs8` feature emits the RFC 8410 PKCS#8
//!   (private) / SPKI (public) PEM directly.
//! * **X25519** — Curve25519 ECDH keys for key exchange / secure channels
//!   (Noise, WireGuard, HPKE, age). Uses pure-Rust `x25519-dalek` for the raw
//!   scalar/point; the PEM is hand-rolled per RFC 8410 (fixed-length short-form
//!   DER for 32-byte Curve25519 keys) and cross-checked against dalek's own
//!   Ed25519 encoder in the unit tests.
//!
//! The CSPRNG is `getrandom` via `OsRng` (WASI `random_get` on wasm32-wasip1),
//! so the block runs on every backend. Key generation is non-deterministic, so
//! there is no standalone page (chat + CLI only).

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
use ed25519_dalek::SigningKey;
use serde::Serialize;
use x25519_dalek::{PublicKey, StaticSecret};

/// A supported key-agreement / signature algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// Curve25519 ECDH — key exchange / secure channels.
    X25519,
    /// Edwards25519 EdDSA — digital signatures.
    Ed25519,
}

impl Algorithm {
    /// Parse an algorithm name (case-insensitive; accepts a couple of aliases).
    pub fn parse(s: &str) -> Result<Algorithm, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "x25519" | "curve25519" => Ok(Algorithm::X25519),
            "ed25519" | "edwards25519" => Ok(Algorithm::Ed25519),
            other => Err(format!(
                "unsupported algorithm {other:?} (expected x25519 or ed25519)"
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Algorithm::X25519 => "x25519",
            Algorithm::Ed25519 => "ed25519",
        }
    }

    /// One-line description of what the key pair is used for.
    pub fn usage(self) -> &'static str {
        match self {
            Algorithm::X25519 => {
                "Curve25519 ECDH key agreement — secure channels (Noise, WireGuard, HPKE, age)"
            }
            Algorithm::Ed25519 => {
                "Edwards25519 EdDSA signatures — signing/verification (SSH, OpenPGP, JWT EdDSA, TLS)"
            }
        }
    }
}

/// A generated key pair in several encodings. Raw keys are the 32-byte scalar
/// (private) and 32-byte point (public); PEM is RFC 8410 PKCS#8 / SPKI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyPair {
    /// The algorithm used: `"x25519"` or `"ed25519"`.
    pub algorithm: String,
    /// One-line description of the key pair's purpose.
    pub usage: String,
    /// Private key, PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`).
    pub private_pem: String,
    /// Public key, SPKI PEM (`-----BEGIN PUBLIC KEY-----`).
    pub public_pem: String,
    /// Raw 32-byte private scalar, base64.
    pub private_base64: String,
    /// Raw 32-byte private scalar, lower-hex.
    pub private_hex: String,
    /// Raw 32-byte public point, base64.
    pub public_base64: String,
    /// Raw 32-byte public point, lower-hex.
    pub public_hex: String,
}

// RFC 8410 algorithm-identifier OIDs, DER-encoded (the 3 content bytes of the
// OBJECT IDENTIFIER — `1.3.101.11x`).
const OID_X25519: [u8; 3] = [0x2b, 0x65, 0x6e]; // 1.3.101.110
const OID_ED25519: [u8; 3] = [0x2b, 0x65, 0x70]; // 1.3.101.112

/// RFC 8410 §7 OneAsymmetricKey (PKCS#8) DER for a 32-byte Curve25519 private
/// key. All lengths are fixed short-form, so the structure is a byte template:
/// `SEQUENCE { INTEGER 0, SEQUENCE { OID }, OCTET STRING { OCTET STRING { key } } }`.
fn pkcs8_der(oid: &[u8; 3], raw: &[u8; 32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(48);
    v.extend_from_slice(&[0x30, 0x2e]); // SEQUENCE, len 46
    v.extend_from_slice(&[0x02, 0x01, 0x00]); // INTEGER version 0
    v.extend_from_slice(&[0x30, 0x05, 0x06, 0x03]); // SEQUENCE { OID (3 bytes) }
    v.extend_from_slice(oid);
    v.extend_from_slice(&[0x04, 0x22, 0x04, 0x20]); // OCTET STRING { OCTET STRING(32) }
    v.extend_from_slice(raw);
    v
}

/// RFC 8410 §4 SubjectPublicKeyInfo (SPKI) DER for a 32-byte Curve25519 public
/// key: `SEQUENCE { SEQUENCE { OID }, BIT STRING { key } }`.
fn spki_der(oid: &[u8; 3], raw: &[u8; 32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(44);
    v.extend_from_slice(&[0x30, 0x2a]); // SEQUENCE, len 42
    v.extend_from_slice(&[0x30, 0x05, 0x06, 0x03]); // SEQUENCE { OID (3 bytes) }
    v.extend_from_slice(oid);
    v.extend_from_slice(&[0x03, 0x21, 0x00]); // BIT STRING(33): 0 unused bits + 32 key bytes
    v.extend_from_slice(raw);
    v
}

/// Wrap DER in a PEM armor with 64-char base64 lines (RFC 7468 / OpenSSL style).
fn pem_wrap(label: &str, der: &[u8]) -> String {
    let b64 = B64.encode(der);
    let mut s = String::with_capacity(b64.len() + 64);
    s.push_str("-----BEGIN ");
    s.push_str(label);
    s.push_str("-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        s.push_str(std::str::from_utf8(chunk).expect("base64 is ASCII"));
        s.push('\n');
    }
    s.push_str("-----END ");
    s.push_str(label);
    s.push_str("-----\n");
    s
}

/// Generate a fresh key pair for `algorithm`, drawing from the OS CSPRNG.
pub fn generate(algorithm: Algorithm) -> Result<KeyPair, String> {
    let (private_pem, public_pem, priv_raw, pub_raw) = match algorithm {
        Algorithm::Ed25519 => {
            let signing = SigningKey::generate(&mut rand::rngs::OsRng);
            let verifying = signing.verifying_key();
            let private_pem = signing
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(|e| format!("failed to encode Ed25519 private key: {e}"))?
                .to_string();
            let public_pem = verifying
                .to_public_key_pem(LineEnding::LF)
                .map_err(|e| format!("failed to encode Ed25519 public key: {e}"))?;
            (private_pem, public_pem, signing.to_bytes(), verifying.to_bytes())
        }
        Algorithm::X25519 => {
            let secret = StaticSecret::random_from_rng(rand::rngs::OsRng);
            let public = PublicKey::from(&secret);
            let priv_raw = secret.to_bytes();
            let pub_raw = public.to_bytes();
            let private_pem = pem_wrap("PRIVATE KEY", &pkcs8_der(&OID_X25519, &priv_raw));
            let public_pem = pem_wrap("PUBLIC KEY", &spki_der(&OID_X25519, &pub_raw));
            (private_pem, public_pem, priv_raw, pub_raw)
        }
    };

    Ok(KeyPair {
        algorithm: algorithm.label().to_string(),
        usage: algorithm.usage().to_string(),
        private_pem,
        public_pem,
        private_base64: B64.encode(priv_raw),
        private_hex: hex::encode(priv_raw),
        public_base64: B64.encode(pub_raw),
        public_hex: hex::encode(pub_raw),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::pkcs8::{DecodePrivateKey, DecodePublicKey};
    use ed25519_dalek::{Signer, Verifier, VerifyingKey};

    #[test]
    fn parse_accepts_names_and_aliases() {
        assert_eq!(Algorithm::parse("x25519").unwrap(), Algorithm::X25519);
        assert_eq!(Algorithm::parse("  Ed25519 ").unwrap(), Algorithm::Ed25519);
        assert_eq!(Algorithm::parse("curve25519").unwrap(), Algorithm::X25519);
        assert!(Algorithm::parse("rsa").is_err());
    }

    #[test]
    fn ed25519_generates_valid_pem_pair() {
        let kp = generate(Algorithm::Ed25519).unwrap();
        assert_eq!(kp.algorithm, "ed25519");
        assert!(kp.private_pem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(kp.private_pem.trim_end().ends_with("-----END PRIVATE KEY-----"));
        assert!(kp.public_pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        // 32-byte raw keys → 64 hex chars, 32 decoded base64 bytes.
        assert_eq!(kp.private_hex.len(), 64);
        assert_eq!(kp.public_hex.len(), 64);
        assert_eq!(B64.decode(&kp.private_base64).unwrap().len(), 32);
        assert_eq!(B64.decode(&kp.public_base64).unwrap().len(), 32);
    }

    #[test]
    fn ed25519_private_pem_reparses_and_public_matches() {
        let kp = generate(Algorithm::Ed25519).unwrap();
        let sk = SigningKey::from_pkcs8_pem(&kp.private_pem).expect("private PEM parses");
        let derived_pub = sk.verifying_key().to_public_key_pem(LineEnding::LF).unwrap();
        assert_eq!(derived_pub, kp.public_pem, "public PEM matches the private key");
    }

    #[test]
    fn ed25519_key_pair_signs_and_verifies() {
        let kp = generate(Algorithm::Ed25519).unwrap();
        let sk = SigningKey::from_pkcs8_pem(&kp.private_pem).unwrap();
        let pub_raw: [u8; 32] = B64.decode(&kp.public_base64).unwrap().try_into().unwrap();
        let vk = VerifyingKey::from_bytes(&pub_raw).unwrap();
        let sig = sk.sign(b"gizza test message");
        vk.verify(b"gizza test message", &sig)
            .expect("signature verifies under the public key");
    }

    /// The hand-rolled DER templates must be exactly the RFC 8410 short-form
    /// byte layout (the RFC's own examples use this v1 form — version 0, no
    /// attached public key). Only the 3-byte OID differs between algorithms.
    #[test]
    fn der_headers_match_rfc8410() {
        let key = [0xABu8; 32];

        let mut want_pkcs8_x = vec![
            0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x04, 0x22,
            0x04, 0x20,
        ];
        want_pkcs8_x.extend_from_slice(&key);
        assert_eq!(pkcs8_der(&OID_X25519, &key), want_pkcs8_x);

        let mut want_spki_x = vec![
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x03, 0x21, 0x00,
        ];
        want_spki_x.extend_from_slice(&key);
        assert_eq!(spki_der(&OID_X25519, &key), want_spki_x);

        // Ed25519 differs only in the trailing OID byte (0x70 vs 0x6e).
        assert_eq!(pkcs8_der(&OID_ED25519, &key)[11], 0x70);
        assert_eq!(spki_der(&OID_ED25519, &key)[8], 0x70);
    }

    /// Cross-check the hand-rolled encoding against a real RFC 8410 parser:
    /// ed25519-dalek must re-parse our hand-rolled PKCS#8/SPKI PEM back to the
    /// same key. If the byte template is valid for Ed25519, it is valid for
    /// X25519 (which differs only in the 3-byte OID).
    #[test]
    fn hand_rolled_pem_reparses_via_dalek() {
        let signing = SigningKey::generate(&mut rand::rngs::OsRng);
        let verifying = signing.verifying_key();

        let ours_priv = pem_wrap("PRIVATE KEY", &pkcs8_der(&OID_ED25519, &signing.to_bytes()));
        let ours_pub = pem_wrap("PUBLIC KEY", &spki_der(&OID_ED25519, &verifying.to_bytes()));

        let reparsed_sk = SigningKey::from_pkcs8_pem(&ours_priv).expect("hand-rolled PKCS#8 parses");
        assert_eq!(reparsed_sk.to_bytes(), signing.to_bytes());
        let reparsed_vk =
            VerifyingKey::from_public_key_pem(&ours_pub).expect("hand-rolled SPKI parses");
        assert_eq!(reparsed_vk.to_bytes(), verifying.to_bytes());
    }

    #[test]
    fn x25519_generates_valid_pem_pair() {
        let kp = generate(Algorithm::X25519).unwrap();
        assert_eq!(kp.algorithm, "x25519");
        assert!(kp.private_pem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(kp.public_pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert!(kp.public_pem.trim_end().ends_with("-----END PUBLIC KEY-----"));
        assert_eq!(kp.private_hex.len(), 64);
        assert_eq!(kp.public_hex.len(), 64);
    }

    /// The public key in the PEM/hex must be the ECDH point derived from the
    /// private scalar — re-derive it from the raw private bytes and compare.
    #[test]
    fn x25519_public_matches_private() {
        let kp = generate(Algorithm::X25519).unwrap();
        let priv_raw: [u8; 32] = B64.decode(&kp.private_base64).unwrap().try_into().unwrap();
        let secret = StaticSecret::from(priv_raw);
        let derived_pub = PublicKey::from(&secret).to_bytes();
        assert_eq!(hex::encode(derived_pub), kp.public_hex, "public matches private");
    }

    /// Two X25519 peers derive the same shared secret — proves the generated
    /// keys are usable for real Diffie-Hellman key agreement.
    #[test]
    fn x25519_keys_agree_on_shared_secret() {
        let alice = generate(Algorithm::X25519).unwrap();
        let bob = generate(Algorithm::X25519).unwrap();
        let a_priv: [u8; 32] = B64.decode(&alice.private_base64).unwrap().try_into().unwrap();
        let b_priv: [u8; 32] = B64.decode(&bob.private_base64).unwrap().try_into().unwrap();
        let a_pub: [u8; 32] = hex::decode(&alice.public_hex).unwrap().try_into().unwrap();
        let b_pub: [u8; 32] = hex::decode(&bob.public_hex).unwrap().try_into().unwrap();

        let a_secret = StaticSecret::from(a_priv).diffie_hellman(&PublicKey::from(b_pub));
        let b_secret = StaticSecret::from(b_priv).diffie_hellman(&PublicKey::from(a_pub));
        assert_eq!(a_secret.to_bytes(), b_secret.to_bytes(), "ECDH agrees");
    }

    #[test]
    fn two_calls_differ() {
        let a = generate(Algorithm::Ed25519).unwrap();
        let b = generate(Algorithm::Ed25519).unwrap();
        assert_ne!(a.private_hex, b.private_hex, "each generation is fresh");
    }
}
