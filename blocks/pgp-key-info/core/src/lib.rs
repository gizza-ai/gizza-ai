//! gizza-ai/pgp-key-info core — inspect ASCII-armored OpenPGP keys without
//! uploading them. Pure Rust (rPGP), shared by chat, CLI, and the browser page.

use pgp::composed::{Deserializable, SignedPublicKey, SignedSecretKey};
use pgp::types::PublicKeyTrait;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct KeyInfo {
    pub key_type: String,
    pub fingerprint: String,
    pub key_id: String,
    pub algorithm: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub user_ids: Vec<String>,
    pub subkeys: Vec<SubkeyInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubkeyInfo {
    pub fingerprint: String,
    pub key_id: String,
    pub algorithm: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub capabilities: Vec<String>,
}

fn hex_upper(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02X}"));
    }
    out
}

fn key_id_hex(k: &impl PublicKeyTrait) -> String {
    format!("{:X}", k.key_id())
}

fn fingerprint_hex(k: &impl PublicKeyTrait) -> String {
    hex_upper(k.fingerprint().as_bytes())
}

fn created_at(k: &impl PublicKeyTrait) -> String {
    k.created_at().to_rfc3339()
}

fn expires_at(k: &impl PublicKeyTrait) -> Option<String> {
    k.expiration()
        .map(|days| (*k.created_at() + chrono::Duration::days(days as i64)).to_rfc3339())
}

fn capabilities(k: &impl PublicKeyTrait) -> Vec<String> {
    let mut caps = Vec::new();
    if k.is_signing_key() {
        caps.push("sign".to_string());
    }
    if k.is_encryption_key() {
        caps.push("encrypt".to_string());
    }
    caps
}

fn user_ids(key: &SignedPublicKey) -> Vec<String> {
    key.details
        .users
        .iter()
        .map(|u| u.id.id().to_string())
        .filter(|s| !s.trim().is_empty())
        .collect()
}

fn describe_public(key: &SignedPublicKey, key_type: &str) -> Result<KeyInfo, String> {
    key.verify()
        .map_err(|e| format!("key failed self-verification: {e}"))?;

    let subkeys = key
        .public_subkeys
        .iter()
        .map(|sub| SubkeyInfo {
            fingerprint: fingerprint_hex(&sub.key),
            key_id: key_id_hex(&sub.key),
            algorithm: format!("{:?}", sub.key.algorithm()),
            created_at: created_at(&sub.key),
            expires_at: expires_at(&sub.key),
            capabilities: capabilities(&sub.key),
        })
        .collect();

    Ok(KeyInfo {
        key_type: key_type.to_string(),
        fingerprint: fingerprint_hex(&key.primary_key),
        key_id: key_id_hex(&key.primary_key),
        algorithm: format!("{:?}", key.primary_key.algorithm()),
        created_at: created_at(&key.primary_key),
        expires_at: expires_at(&key.primary_key),
        user_ids: user_ids(key),
        subkeys,
    })
}

/// Inspect an ASCII-armored OpenPGP public or private key.
pub fn inspect(key: &str) -> Result<KeyInfo, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("paste an ASCII-armored PGP public or private key".into());
    }

    let info = if trimmed.contains("BEGIN PGP PUBLIC KEY BLOCK") {
        let (key, _) = SignedPublicKey::from_string(trimmed)
            .map_err(|e| format!("not a valid PGP public key: {e}"))?;
        describe_public(&key, "public")?
    } else if trimmed.contains("BEGIN PGP PRIVATE KEY BLOCK") {
        let (secret, _) = SignedSecretKey::from_string(trimmed)
            .map_err(|e| format!("not a valid PGP private key: {e}"))?;
        secret
            .verify()
            .map_err(|e| format!("private key failed self-verification: {e}"))?;
        let public: SignedPublicKey = secret.into();
        describe_public(&public, "private")?
    } else {
        return Err("no PGP key found — paste a full '-----BEGIN PGP PUBLIC KEY BLOCK-----' or '-----BEGIN PGP PRIVATE KEY BLOCK-----' block".into());
    };

    Ok(info)
}

/// Inspect an ASCII-armored OpenPGP public or private key and return pretty JSON.
pub fn run(key: &str) -> Result<String, String> {
    let info = inspect(key)?;
    serde_json::to_string_pretty(&info).map_err(|e| format!("serialize key info: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pgp::composed::{KeyType, SecretKeyParamsBuilder, SubkeyParamsBuilder};
    use pgp::crypto::ecc_curve::ECCCurve;
    use pgp::crypto::hash::HashAlgorithm;
    use pgp::crypto::sym::SymmetricKeyAlgorithm;
    use pgp::types::CompressionAlgorithm;
    use smallvec::smallvec;

    fn public_key() -> String {
        let mut sub = SubkeyParamsBuilder::default();
        sub.key_type(KeyType::ECDH(ECCCurve::Curve25519))
            .can_encrypt(true);
        let sub = sub.build().unwrap();

        let mut params = SecretKeyParamsBuilder::default();
        params
            .key_type(KeyType::EdDSALegacy)
            .can_certify(true)
            .can_sign(true)
            .primary_user_id("Test <t@example.com>".into())
            .preferred_symmetric_algorithms(smallvec![SymmetricKeyAlgorithm::AES256])
            .preferred_hash_algorithms(smallvec![HashAlgorithm::SHA2_256])
            .preferred_compression_algorithms(smallvec![CompressionAlgorithm::ZLIB])
            .subkey(sub);
        let params = params.build().unwrap();
        let mut rng = rand::rngs::OsRng;
        let secret = params.generate(&mut rng).unwrap();
        let signed = secret.sign(&mut rng, || String::new()).unwrap();
        let public: SignedPublicKey = signed.into();
        public.to_armored_string(None.into()).unwrap()
    }

    #[test]
    fn reports_public_key_metadata() {
        let json = run(&public_key()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["key_type"], "public");
        assert_eq!(v["user_ids"][0], "Test <t@example.com>");
        assert!(v["fingerprint"].as_str().unwrap().len() >= 40);
        assert!(v["key_id"].is_string());
        assert!(v["algorithm"].is_string());
        assert!(v["subkeys"].as_array().unwrap().len() >= 1);
    }

    #[test]
    fn rejects_non_key_input() {
        let err = run("not a key").unwrap_err();
        assert!(err.contains("no PGP key"));
    }
}
