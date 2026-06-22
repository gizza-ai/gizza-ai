//! gizza-ai/generate-ecdsa-key-pair core — generate an ECDSA key pair on a NIST
//! prime curve (P-256, P-384, or P-521) and serialize it. No wafer/wasm-bindgen
//! deps. Pure-Rust `p256`/`p384`/`p521`; the CSPRNG is `getrandom` (WASI
//! `random_get` on wasm32-wasip1), so it runs on every backend.
//!
//! Private key: PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`).
//! Public key:  SPKI PEM   (`-----BEGIN PUBLIC KEY-----`).
//! Optional JWK (RFC 7517/7518) for both keys (EC `kty`, base64url coords).
//!
//! Only random key *generation* is used (no RFC-6979 signing), so P-521 — whose
//! deterministic signer is unavailable — is fully supported here.

use p256::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};

/// NIST prime curve. The curve fixes the key size and JWK `crv` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    P256,
    P384,
    P521,
}

impl Curve {
    pub fn parse(s: &str) -> Result<Curve, String> {
        match s
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', ' '], "")
            .as_str()
        {
            "p256" | "secp256r1" | "prime256v1" | "" => Ok(Curve::P256),
            "p384" | "secp384r1" => Ok(Curve::P384),
            "p521" | "secp521r1" => Ok(Curve::P521),
            other => Err(format!("unknown curve '{other}' (use p256, p384, or p521)")),
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Curve::P256 => "p256",
            Curve::P384 => "p384",
            Curve::P521 => "p521",
        }
    }
}

/// A generated key pair, with PEM always and JWK on request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPair {
    pub private_pem: String,
    pub public_pem: String,
    /// JWK private key (JSON string) — `None` unless `jwk` requested.
    pub private_jwk: Option<String>,
    /// JWK public key (JSON string) — `None` unless `jwk` requested.
    pub public_jwk: Option<String>,
    pub curve: String,
}

/// Generate an ECDSA key pair on `curve`. When `jwk` is true, also emit RFC 7517
/// JSON Web Keys for both the private and public key.
pub fn generate(curve: Curve, jwk: bool) -> Result<KeyPair, String> {
    let mut rng = rand_core::OsRng;
    let pub_enc = |x: p256::pkcs8::spki::Error| format!("failed to encode public key: {x}");

    match curve {
        Curve::P256 => {
            let sk = p256::SecretKey::random(&mut rng);
            let pk = sk.public_key();
            let private_pem = sk
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(|e| format!("failed to encode private key: {e}"))?
                .to_string();
            let public_pem = pk.to_public_key_pem(LineEnding::LF).map_err(pub_enc)?;
            let (private_jwk, public_jwk) = if jwk {
                (Some(sk.to_jwk_string().to_string()), Some(pk.to_jwk_string()))
            } else {
                (None, None)
            };
            Ok(KeyPair { private_pem, public_pem, private_jwk, public_jwk, curve: curve.name().into() })
        }
        Curve::P384 => {
            let sk = p384::SecretKey::random(&mut rng);
            let pk = sk.public_key();
            let private_pem = sk
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(|e| format!("failed to encode private key: {e}"))?
                .to_string();
            let public_pem = pk.to_public_key_pem(LineEnding::LF).map_err(pub_enc)?;
            let (private_jwk, public_jwk) = if jwk {
                (Some(sk.to_jwk_string().to_string()), Some(pk.to_jwk_string()))
            } else {
                (None, None)
            };
            Ok(KeyPair { private_pem, public_pem, private_jwk, public_jwk, curve: curve.name().into() })
        }
        Curve::P521 => {
            let sk = p521::SecretKey::random(&mut rng);
            let pk = sk.public_key();
            let private_pem = sk
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(|e| format!("failed to encode private key: {e}"))?
                .to_string();
            let public_pem = pk.to_public_key_pem(LineEnding::LF).map_err(pub_enc)?;
            let (private_jwk, public_jwk) = if jwk {
                (Some(sk.to_jwk_string().to_string()), Some(pk.to_jwk_string()))
            } else {
                (None, None)
            };
            Ok(KeyPair { private_pem, public_pem, private_jwk, public_jwk, curve: curve.name().into() })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::pkcs8::DecodePrivateKey;

    #[test]
    fn parse_accepts_aliases_and_rejects_unknown() {
        assert_eq!(Curve::parse("").unwrap(), Curve::P256);
        assert_eq!(Curve::parse("P-256").unwrap(), Curve::P256);
        assert_eq!(Curve::parse("prime256v1").unwrap(), Curve::P256);
        assert_eq!(Curve::parse("secp384r1").unwrap(), Curve::P384);
        assert_eq!(Curve::parse("P-521").unwrap(), Curve::P521);
        assert!(Curve::parse("p999").is_err());
        assert!(Curve::parse("ed25519").is_err());
    }

    #[test]
    fn generates_valid_p256_pem_pair() {
        let kp = generate(Curve::P256, false).unwrap();
        assert!(kp.private_pem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(kp.private_pem.trim_end().ends_with("-----END PRIVATE KEY-----"));
        assert!(kp.public_pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert_eq!(kp.curve, "p256");
        assert!(kp.private_jwk.is_none());
        assert!(kp.public_jwk.is_none());

        // Private PEM must re-parse and its public key must match.
        let parsed = p256::SecretKey::from_pkcs8_pem(&kp.private_pem).expect("private PEM parses");
        let derived = parsed
            .public_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        assert_eq!(derived, kp.public_pem, "public PEM matches the private key");
    }

    #[test]
    fn p384_and_p521_generate() {
        let a = generate(Curve::P384, false).unwrap();
        assert_eq!(a.curve, "p384");
        assert!(p384::SecretKey::from_pkcs8_pem(&a.private_pem).is_ok());
        let b = generate(Curve::P521, false).unwrap();
        assert_eq!(b.curve, "p521");
        assert!(p521::SecretKey::from_pkcs8_pem(&b.private_pem).is_ok());
    }

    #[test]
    fn jwk_emitted_when_requested() {
        let kp = generate(Curve::P256, true).unwrap();
        let priv_jwk = kp.private_jwk.expect("private jwk present");
        let pub_jwk = kp.public_jwk.expect("public jwk present");
        // EC JWK: kty=EC, crv=P-256, x/y coords; private adds d.
        assert!(priv_jwk.contains("\"kty\":\"EC\""));
        assert!(priv_jwk.contains("\"crv\":\"P-256\""));
        assert!(priv_jwk.contains("\"d\":"), "private JWK carries d");
        assert!(pub_jwk.contains("\"crv\":\"P-256\""));
        assert!(!pub_jwk.contains("\"d\":"), "public JWK must not leak d");
    }

    #[test]
    fn two_calls_differ() {
        let a = generate(Curve::P256, false).unwrap();
        let b = generate(Curve::P256, false).unwrap();
        assert_ne!(a.private_pem, b.private_pem, "each generation is fresh");
    }
}
