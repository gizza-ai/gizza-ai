//! gizza-ai/pgp-verify core — verify an OpenPGP (PGP/GPG) signature against a
//! message and a signer's public key. Pure-Rust (rPGP): no wafer/wasm-bindgen
//! deps. Shared by the chat-skill block and the web page.
//!
//! Handles both signature shapes (auto-detected from the armor):
//! - **detached** — a standalone `-----BEGIN PGP SIGNATURE-----` block plus the
//!   original, unmodified message bytes (the usual `gpg --detach-sign`).
//! - **clearsign** — an inline `-----BEGIN PGP SIGNED MESSAGE-----` block that
//!   embeds the readable text; the separate `message` argument is ignored.
//!
//! Verification tries the public key's primary key and every subkey, so a key
//! whose signatures come from a subkey still verifies. The result reports
//! validity, the matched signer's key ID + fingerprint, and a clear error
//! string when the signature does not check out.

use pgp::composed::cleartext::CleartextSignedMessage;
use pgp::composed::{Deserializable, SignedPublicKey, StandaloneSignature};
use pgp::types::PublicKeyTrait;
use serde::Serialize;

/// Which signature shape was verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Standalone `-----BEGIN PGP SIGNATURE-----` over the supplied message.
    Detached,
    /// Inline `-----BEGIN PGP SIGNED MESSAGE-----` (text embedded).
    Clearsign,
}

/// Structured verification result, serialized to JSON for every surface.
#[derive(Debug, Clone, Serialize)]
pub struct VerifyResult {
    /// `true` only when the signature checks out against the supplied key.
    pub valid: bool,
    /// `detached` or `clearsign` (which shape the signature was).
    pub mode: Mode,
    /// 16-hex-digit long key ID of the (sub)key that produced the signature, if
    /// recorded in the signature's issuer subpackets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_key_id: Option<String>,
    /// Full hex fingerprint of the public (sub)key that successfully verified
    /// the signature (only present when `valid`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_fingerprint: Option<String>,
    /// The signer's primary User ID from the public key (e.g.
    /// `"Alice <alice@example.com>"`), so the caller can see *who* the key
    /// claims to belong to. Taken from the key, not the signature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_user_id: Option<String>,
    /// When the signature was created, as an RFC 3339 / ISO-8601 UTC timestamp,
    /// if the signature records a creation time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_at: Option<String>,
    /// The hash algorithm used by the signature (e.g. `"SHA256"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_algorithm: Option<String>,
    /// For a clearsigned message, the embedded signed text (so the caller can
    /// see exactly what was signed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_text: Option<String>,
    /// Why verification failed (only present when `valid` is false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl VerifyResult {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("VerifyResult serializes")
    }
}

fn fingerprint_hex(k: &impl PublicKeyTrait) -> String {
    k.fingerprint()
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect()
}

/// The issuer key ID recorded in a signature packet, if any (upper-hex).
fn sig_issuer_key_id(sig: &pgp::packet::Signature) -> Option<String> {
    sig.issuer().first().map(|kid| format!("{kid:X}"))
}

/// The signature's creation time as an RFC 3339 UTC string, if recorded.
fn sig_created_at(sig: &pgp::packet::Signature) -> Option<String> {
    sig.created().map(|dt| dt.to_rfc3339())
}

/// The signature's hash algorithm as a canonical OpenPGP name (e.g. "SHA256").
fn sig_hash_algorithm(sig: &pgp::packet::Signature) -> Option<String> {
    use pgp::crypto::hash::HashAlgorithm as H;
    let name = match sig.hash_alg() {
        H::None => return None,
        H::MD5 => "MD5",
        H::SHA1 => "SHA1",
        H::RIPEMD160 => "RIPEMD160",
        H::SHA2_256 => "SHA256",
        H::SHA2_384 => "SHA384",
        H::SHA2_512 => "SHA512",
        H::SHA2_224 => "SHA224",
        H::SHA3_256 => "SHA3-256",
        H::SHA3_512 => "SHA3-512",
        other => return Some(format!("{other:?}")),
    };
    Some(name.to_string())
}

/// The signer's primary User ID string from the public key (the first UID on
/// the key, e.g. "Alice <alice@example.com>"), if present.
fn primary_user_id(key: &SignedPublicKey) -> Option<String> {
    key.details
        .users
        .first()
        .map(|u| u.id.id().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_public_key(armored: &str) -> Result<SignedPublicKey, String> {
    if !armored.contains("BEGIN PGP PUBLIC KEY BLOCK") {
        return Err(
            "no PGP public key found — paste the signer's ASCII-armored '-----BEGIN PGP PUBLIC KEY BLOCK-----' key"
                .into(),
        );
    }
    let (key, _) = SignedPublicKey::from_string(armored)
        .map_err(|e| format!("not a valid PGP public key: {e}"))?;
    key.verify()
        .map_err(|e| format!("public key failed self-verification: {e}"))?;
    Ok(key)
}

/// Verify a PGP signature.
///
/// - `message`: the original signed bytes — required for a detached signature,
///   ignored for a clearsigned block (the text is embedded).
/// - `signature`: the ASCII-armored signature. A `PGP SIGNED MESSAGE` block is
///   treated as clearsign; a `PGP SIGNATURE` block as detached.
/// - `public_key`: the signer's ASCII-armored public key.
///
/// Returns a [`VerifyResult`]. A signature that simply does not match returns
/// `Ok(VerifyResult { valid: false, .. })`; only malformed inputs return `Err`.
pub fn run(message: &str, signature: &str, public_key: &str) -> Result<VerifyResult, String> {
    let key = parse_public_key(public_key)?;
    let trimmed = signature.trim_start();

    if trimmed.contains("BEGIN PGP SIGNED MESSAGE") {
        verify_clearsign(signature, &key)
    } else if trimmed.contains("BEGIN PGP SIGNATURE") {
        verify_detached(message, signature, &key)
    } else {
        Err("no PGP signature found — paste a '-----BEGIN PGP SIGNATURE-----' (detached) or '-----BEGIN PGP SIGNED MESSAGE-----' (clearsign) block".into())
    }
}

fn verify_detached(
    message: &str,
    signature: &str,
    key: &SignedPublicKey,
) -> Result<VerifyResult, String> {
    let (sig, _) = StandaloneSignature::from_string(signature)
        .map_err(|e| format!("not a valid detached PGP signature: {e}"))?;
    let key_id = sig_issuer_key_id(&sig.signature);
    let signed_at = sig_created_at(&sig.signature);
    let hash_algorithm = sig_hash_algorithm(&sig.signature);
    let user_id = primary_user_id(key);
    let data = message.as_bytes();

    // PublicKeyTrait is not dyn-compatible, so iterate the primary key then each
    // subkey directly (they are distinct concrete types) and take the first that
    // verifies.
    let mut matched: Option<String> = None;
    if sig.signature.verify(&key.primary_key, data).is_ok() {
        matched = Some(fingerprint_hex(&key.primary_key));
    } else {
        for sub in &key.public_subkeys {
            if sig.signature.verify(&sub.key, data).is_ok() {
                matched = Some(fingerprint_hex(&sub.key));
                break;
            }
        }
    }
    Ok(match matched {
        Some(fpr) => VerifyResult {
            valid: true,
            mode: Mode::Detached,
            signer_key_id: key_id,
            signer_fingerprint: Some(fpr),
            signer_user_id: user_id,
            signed_at,
            hash_algorithm,
            signed_text: None,
            error: None,
        },
        None => VerifyResult {
            valid: false,
            mode: Mode::Detached,
            signer_key_id: key_id,
            signer_fingerprint: None,
            signer_user_id: user_id,
            signed_at,
            hash_algorithm,
            signed_text: None,
            error: Some(
                "signature does not verify against this public key — the message may have been altered, or it was signed by a different key"
                    .into(),
            ),
        },
    })
}

fn verify_clearsign(signature: &str, key: &SignedPublicKey) -> Result<VerifyResult, String> {
    let (csm, _) = CleartextSignedMessage::from_string(signature)
        .map_err(|e| format!("not a valid clearsigned PGP message: {e}"))?;
    let signed_text = csm.signed_text();
    let first_sig = csm.signatures().first();
    let key_id = first_sig.and_then(|s| sig_issuer_key_id(&s.signature));
    let signed_at = first_sig.and_then(|s| sig_created_at(&s.signature));
    let hash_algorithm = first_sig.and_then(|s| sig_hash_algorithm(&s.signature));
    let user_id = primary_user_id(key);

    let mut matched: Option<String> = None;
    if csm.verify(&key.primary_key).is_ok() {
        matched = Some(fingerprint_hex(&key.primary_key));
    } else {
        for sub in &key.public_subkeys {
            if csm.verify(&sub.key).is_ok() {
                matched = Some(fingerprint_hex(&sub.key));
                break;
            }
        }
    }
    Ok(match matched {
        Some(fpr) => VerifyResult {
            valid: true,
            mode: Mode::Clearsign,
            signer_key_id: key_id,
            signer_fingerprint: Some(fpr),
            signer_user_id: user_id,
            signed_at,
            hash_algorithm,
            signed_text: Some(signed_text),
            error: None,
        },
        None => VerifyResult {
            valid: false,
            mode: Mode::Clearsign,
            signer_key_id: key_id,
            signer_fingerprint: None,
            signer_user_id: user_id,
            signed_at,
            hash_algorithm,
            signed_text: Some(signed_text),
            error: Some(
                "signature does not verify against this public key — the signed text may have been altered, or it was signed by a different key"
                    .into(),
            ),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pgp::composed::cleartext::CleartextSignedMessage;
    use pgp::composed::{KeyType, SecretKeyParamsBuilder, SignedSecretKey, StandaloneSignature};
    use pgp::packet::{SignatureConfig, SignatureType, Subpacket, SubpacketData};
    use pgp::types::{PublicKeyTrait, SecretKeyTrait};
    use chrono::SubsecRound;
    use rand::rngs::OsRng;
    use smallvec::smallvec;

    fn gen_secret() -> SignedSecretKey {
        let mut params = SecretKeyParamsBuilder::default();
        params
            .key_type(KeyType::Rsa(2048))
            .can_sign(true)
            .can_certify(true)
            .primary_user_id("Test <test@example.com>".into())
            .preferred_symmetric_algorithms(smallvec![
                pgp::crypto::sym::SymmetricKeyAlgorithm::AES256
            ]);
        let params = params.build().unwrap();
        let sk = params.generate(OsRng).unwrap();
        sk.sign(OsRng, || String::new()).unwrap()
    }

    fn pub_armored(sk: &SignedSecretKey) -> String {
        sk.public_key()
            .sign(OsRng, &sk.primary_key, || String::new())
            .unwrap()
            .to_armored_string(Default::default())
            .unwrap()
    }

    fn detached_sig(sk: &SignedSecretKey, msg: &str) -> String {
        let mut config =
            SignatureConfig::v4(SignatureType::Binary, sk.algorithm(), sk.hash_alg());
        config.hashed_subpackets = vec![
            Subpacket::regular(SubpacketData::IssuerFingerprint(sk.fingerprint())),
            Subpacket::regular(SubpacketData::SignatureCreationTime(
                chrono::Utc::now().trunc_subsecs(0),
            )),
        ];
        config.unhashed_subpackets =
            vec![Subpacket::regular(SubpacketData::Issuer(sk.key_id()))];
        let sig = config.sign(&sk, || String::new(), msg.as_bytes()).unwrap();
        StandaloneSignature::new(sig)
            .to_armored_string(Default::default())
            .unwrap()
    }

    fn clearsign(sk: &SignedSecretKey, msg: &str) -> String {
        CleartextSignedMessage::sign(OsRng, msg, &sk, || String::new())
            .unwrap()
            .to_armored_string(Default::default())
            .unwrap()
    }

    #[test]
    fn detached_valid() {
        let sk = gen_secret();
        let sig = detached_sig(&sk, "hello world");
        let res = run("hello world", &sig, &pub_armored(&sk)).unwrap();
        assert!(res.valid, "{:?}", res.error);
        assert_eq!(res.mode, Mode::Detached);
        assert!(res.signer_fingerprint.is_some());
        assert!(res.signer_key_id.is_some());
        // The key's UID and the signature's metadata are surfaced.
        assert_eq!(res.signer_user_id.as_deref(), Some("Test <test@example.com>"));
        assert!(res.signed_at.is_some(), "signature creation time reported");
        assert_eq!(res.hash_algorithm.as_deref(), Some("SHA256"));
    }

    #[test]
    fn detached_rejects_tampered_message() {
        let sk = gen_secret();
        let sig = detached_sig(&sk, "hello world");
        let res = run("hello WORLD", &sig, &pub_armored(&sk)).unwrap();
        assert!(!res.valid);
        assert!(res.error.unwrap().contains("does not verify"));
    }

    #[test]
    fn detached_rejects_wrong_key() {
        let sk = gen_secret();
        let other = gen_secret();
        let sig = detached_sig(&sk, "hello world");
        let res = run("hello world", &sig, &pub_armored(&other)).unwrap();
        assert!(!res.valid);
    }

    #[test]
    fn clearsign_valid_and_exposes_text() {
        let sk = gen_secret();
        let block = clearsign(&sk, "the embedded message");
        let res = run("ignored for clearsign", &block, &pub_armored(&sk)).unwrap();
        assert!(res.valid, "{:?}", res.error);
        assert_eq!(res.mode, Mode::Clearsign);
        assert_eq!(res.signed_text.as_deref(), Some("the embedded message"));
    }

    #[test]
    fn clearsign_rejects_wrong_key() {
        let sk = gen_secret();
        let other = gen_secret();
        let block = clearsign(&sk, "the embedded message");
        let res = run("", &block, &pub_armored(&other)).unwrap();
        assert!(!res.valid);
    }

    #[test]
    fn rejects_garbage_signature() {
        let sk = gen_secret();
        let err = run("x", "not a signature", &pub_armored(&sk)).unwrap_err();
        assert!(err.contains("no PGP signature"), "{err}");
    }

    #[test]
    fn rejects_missing_public_key() {
        let sk = gen_secret();
        let sig = detached_sig(&sk, "x");
        let err = run("x", &sig, "not a key").unwrap_err();
        assert!(err.contains("no PGP public key"), "{err}");
    }

    #[test]
    fn result_serializes_to_expected_json_shape() {
        let sk = gen_secret();
        let sig = detached_sig(&sk, "hi");
        let res = run("hi", &sig, &pub_armored(&sk)).unwrap();
        let v = res.to_json();
        assert_eq!(v["valid"], serde_json::Value::Bool(true));
        assert_eq!(v["mode"], "detached");
        assert!(v["signer_fingerprint"].is_string());
        assert!(v["signer_user_id"].is_string());
        assert!(v["hash_algorithm"].is_string());
        assert!(v.get("error").is_none());
    }
}
