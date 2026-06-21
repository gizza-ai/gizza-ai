//! gizza-ai/generate-pgp-key-pair core — generate a new OpenPGP key pair (RSA or
//! Curve25519/ECC) with a user ID and optional passphrase, emitting ASCII-armored
//! public and private keys. Pure-Rust rPGP (`pgp`). The CSPRNG is `getrandom`
//! (WASI `random_get` on wasm32-wasip1), so it runs on every backend.

use pgp::composed::{
    KeyType, SecretKeyParamsBuilder, SignedPublicKey, SignedSecretKey, SubkeyParamsBuilder,
};
use pgp::crypto::ecc_curve::ECCCurve;
use pgp::crypto::hash::HashAlgorithm;
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use pgp::types::{CompressionAlgorithm, PublicKeyTrait};
use serde::Serialize;
use smallvec::smallvec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algo {
    Rsa2048,
    Rsa3072,
    Rsa4096,
    Curve25519,
}

impl Algo {
    pub fn parse(s: &str) -> Result<Algo, String> {
        match s.trim().to_ascii_lowercase().replace([' ', '-', '_'], "").as_str() {
            "rsa2048" | "rsa" | "" => Ok(Algo::Rsa2048),
            "rsa3072" => Ok(Algo::Rsa3072),
            "rsa4096" => Ok(Algo::Rsa4096),
            "curve25519" | "ecc" | "ed25519" | "25519" => Ok(Algo::Curve25519),
            other => Err(format!(
                "unknown algorithm '{other}' (use rsa2048, rsa3072, rsa4096, or curve25519)"
            )),
        }
    }
    fn name(&self) -> &'static str {
        match self {
            Algo::Rsa2048 => "rsa2048",
            Algo::Rsa3072 => "rsa3072",
            Algo::Rsa4096 => "rsa4096",
            Algo::Curve25519 => "curve25519",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyPair {
    pub user_id: String,
    pub algorithm: String,
    pub fingerprint: String,
    pub public_key: String,
    pub private_key: String,
    pub encrypted: bool,
}

fn hex_upper(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02X}"));
    }
    s
}

/// Generate a fresh OpenPGP key pair.
pub fn generate(user_id: &str, passphrase: Option<&str>, algo: Algo) -> Result<KeyPair, String> {
    let uid = user_id.trim();
    if uid.is_empty() {
        return Err("user_id is required (e.g. \"Alice <alice@example.com>\")".into());
    }
    let pass = passphrase.map(|p| p.to_string()).filter(|p| !p.is_empty());

    let (primary_type, subkey_type) = match algo {
        Algo::Rsa2048 => (KeyType::Rsa(2048), KeyType::Rsa(2048)),
        Algo::Rsa3072 => (KeyType::Rsa(3072), KeyType::Rsa(3072)),
        Algo::Rsa4096 => (KeyType::Rsa(4096), KeyType::Rsa(4096)),
        // GnuPG-compatible v4 ECC: EdDSA (legacy) primary + ECDH(Curve25519) subkey.
        Algo::Curve25519 => (KeyType::EdDSALegacy, KeyType::ECDH(ECCCurve::Curve25519)),
    };

    let mut builder = SecretKeyParamsBuilder::default();
    builder
        .key_type(primary_type)
        .can_certify(true)
        .can_sign(true)
        .primary_user_id(uid.to_string())
        .preferred_symmetric_algorithms(smallvec![
            SymmetricKeyAlgorithm::AES256,
            SymmetricKeyAlgorithm::AES192,
            SymmetricKeyAlgorithm::AES128,
        ])
        .preferred_hash_algorithms(smallvec![
            HashAlgorithm::SHA2_256,
            HashAlgorithm::SHA2_384,
            HashAlgorithm::SHA2_512,
        ])
        .preferred_compression_algorithms(smallvec![
            CompressionAlgorithm::ZLIB,
            CompressionAlgorithm::ZIP,
        ]);
    if let Some(p) = &pass {
        builder.passphrase(Some(p.clone()));
    }

    let mut sub = SubkeyParamsBuilder::default();
    sub.key_type(subkey_type).can_encrypt(true);
    if let Some(p) = &pass {
        sub.passphrase(Some(p.clone()));
    }
    let sub = sub.build().map_err(|e| format!("subkey params: {e}"))?;
    builder.subkey(sub);

    let params = builder.build().map_err(|e| format!("key params: {e}"))?;

    let mut rng = rand::rngs::OsRng;
    let secret = params.generate(&mut rng).map_err(|e| format!("key generation failed: {e}"))?;
    let pass_for_sign = pass.clone().unwrap_or_default();
    let signed_secret: SignedSecretKey = secret
        .sign(&mut rng, || pass_for_sign.clone())
        .map_err(|e| format!("self-signing failed: {e}"))?;

    let fingerprint = hex_upper(signed_secret.fingerprint().as_bytes());
    let private_key = signed_secret
        .to_armored_string(None.into())
        .map_err(|e| format!("armor private key: {e}"))?;
    let signed_public: SignedPublicKey = signed_secret.into();
    let public_key = signed_public
        .to_armored_string(None.into())
        .map_err(|e| format!("armor public key: {e}"))?;

    Ok(KeyPair {
        user_id: uid.to_string(),
        algorithm: algo.name().to_string(),
        fingerprint,
        public_key,
        private_key,
        encrypted: pass.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pgp::composed::Deserializable;
    use pgp::types::SecretKeyTrait;

    #[test]
    fn parse_algos() {
        assert_eq!(Algo::parse("curve25519").unwrap(), Algo::Curve25519);
        assert_eq!(Algo::parse("RSA-4096").unwrap(), Algo::Rsa4096);
        assert_eq!(Algo::parse("").unwrap(), Algo::Rsa2048);
        assert!(Algo::parse("dsa").is_err());
    }

    #[test]
    fn empty_user_id_errors() {
        assert!(generate("", None, Algo::Curve25519).is_err());
    }

    #[test]
    fn generates_curve25519_pair() {
        // Curve25519 is fast — good for the test.
        let kp = generate("Test User <test@example.com>", None, Algo::Curve25519).unwrap();
        assert!(kp.public_key.starts_with("-----BEGIN PGP PUBLIC KEY BLOCK-----"));
        assert!(kp.private_key.starts_with("-----BEGIN PGP PRIVATE KEY BLOCK-----"));
        assert_eq!(kp.fingerprint.len(), 40); // v4 SHA-1 fingerprint = 20 bytes
        assert!(!kp.encrypted);

        // The armored keys must round-trip and verify.
        let (sk, _) = SignedSecretKey::from_string(&kp.private_key).unwrap();
        sk.verify().expect("secret key self-sig verifies");
        let (pk, _) = SignedPublicKey::from_string(&kp.public_key).unwrap();
        pk.verify().expect("public key self-sig verifies");
    }

    #[test]
    fn passphrase_protects_and_unlocks() {
        let kp = generate("Bob <bob@x.io>", Some("s3cret"), Algo::Curve25519).unwrap();
        assert!(kp.encrypted);
        let (sk, _) = SignedSecretKey::from_string(&kp.private_key).unwrap();
        sk.verify().unwrap();
        // Unlock with the right passphrase succeeds.
        sk.unlock(|| "s3cret".into(), |_| Ok(())).expect("unlock with correct passphrase");
    }
}
