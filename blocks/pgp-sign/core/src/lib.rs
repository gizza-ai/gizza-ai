//! gizza-ai/pgp-sign core — create an OpenPGP signature over a message with a
//! private key. Pure-Rust (rPGP): no wafer/wasm-bindgen deps.
//!
//! Two output modes:
//! - `clearsign` → an inline `-----BEGIN PGP SIGNED MESSAGE-----` block (the text
//!   stays readable, with the signature appended).
//! - `detached` → a standalone `-----BEGIN PGP SIGNATURE-----` block that
//!   verifies against the original, unmodified message bytes.
//!
//! Signing uses the key's primary key (the usual signing key for GPG/Proton keys,
//! which are sign+certify). The key may be passphrase-protected.

use chrono::SubsecRound;

use pgp::composed::cleartext::CleartextSignedMessage;
use pgp::composed::{Deserializable, SignedSecretKey, StandaloneSignature};
use pgp::packet::{SignatureConfig, SignatureType, Subpacket, SubpacketData};
use pgp::types::{KeyVersion, PublicKeyTrait, SecretKeyTrait};
use rand::rngs::OsRng;

/// Signature output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Inline clear-signed message (PGP SIGNED MESSAGE).
    Clearsign,
    /// Detached signature (PGP SIGNATURE) over the raw message.
    Detached,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "clearsign" | "clear" | "cleartext" => Ok(Mode::Clearsign),
            "detached" | "detach" => Ok(Mode::Detached),
            other => Err(format!("unknown mode '{other}' (use 'detached' or 'clearsign')")),
        }
    }
}

fn parse_secret_key(armored: &str) -> Result<SignedSecretKey, String> {
    if !armored.contains("BEGIN PGP PRIVATE KEY BLOCK") {
        return Err(
            "no PGP private key found — paste an ASCII-armored '-----BEGIN PGP PRIVATE KEY BLOCK-----' key"
                .into(),
        );
    }
    let (key, _) = SignedSecretKey::from_string(armored)
        .map_err(|e| format!("not a valid PGP private key: {e}"))?;
    key.verify()
        .map_err(|e| format!("private key failed self-verification: {e}"))?;
    Ok(key)
}

/// Sign `message` with the armored `private_key`. `passphrase` unlocks the key if
/// it is encrypted (use "" for an unprotected key).
pub fn run(
    message: &str,
    private_key: &str,
    mode: Mode,
    passphrase: &str,
) -> Result<String, String> {
    let key = parse_secret_key(private_key)?;
    let pw = passphrase.to_string();

    match mode {
        Mode::Clearsign => {
            let signed = CleartextSignedMessage::sign(OsRng, message, &key, || pw.clone())
                .map_err(|e| format!("signing failed: {e}"))?;
            signed
                .to_armored_string(Default::default())
                .map_err(|e| format!("failed to ASCII-armor the signed message: {e}"))
        }
        Mode::Detached => {
            let algorithm = key.algorithm();
            let hash_algorithm = key.hash_alg();
            let mut config = match key.version() {
                KeyVersion::V4 => {
                    SignatureConfig::v4(SignatureType::Binary, algorithm, hash_algorithm)
                }
                KeyVersion::V6 => {
                    SignatureConfig::v6(OsRng, SignatureType::Binary, algorithm, hash_algorithm)
                        .map_err(|e| format!("signature config error: {e}"))?
                }
                v => return Err(format!("unsupported key version {v:?}")),
            };
            config.hashed_subpackets = vec![
                Subpacket::regular(SubpacketData::IssuerFingerprint(key.fingerprint())),
                Subpacket::regular(SubpacketData::SignatureCreationTime(
                    chrono::Utc::now().trunc_subsecs(0),
                )),
            ];
            config.unhashed_subpackets =
                vec![Subpacket::regular(SubpacketData::Issuer(key.key_id()))];

            let sig = config
                .sign(&key, || pw.clone(), message.as_bytes())
                .map_err(|e| format!("signing failed: {e}"))?;
            StandaloneSignature::new(sig)
                .to_armored_string(Default::default())
                .map_err(|e| format!("failed to ASCII-armor the signature: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pgp::composed::{KeyType, SecretKeyParamsBuilder};
    use pgp::types::PublicKeyTrait;
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

    fn armored_secret(sk: &SignedSecretKey) -> String {
        sk.to_armored_string(Default::default()).unwrap()
    }

    #[test]
    fn detached_signature_verifies() {
        let sk = gen_secret();
        let armored = run("sign me", &armored_secret(&sk), Mode::Detached, "").unwrap();
        assert!(armored.contains("-----BEGIN PGP SIGNATURE-----"));

        let (sig, _) = StandaloneSignature::from_string(&armored).unwrap();
        // Verifies against the original bytes with the public key.
        sig.verify(&sk.public_key(), b"sign me").unwrap();
        // And fails against tampered data.
        assert!(sig.verify(&sk.public_key(), b"sign you").is_err());
    }

    #[test]
    fn clearsign_roundtrips_and_keeps_text() {
        let sk = gen_secret();
        let armored = run("hello world", &armored_secret(&sk), Mode::Clearsign, "").unwrap();
        assert!(armored.contains("-----BEGIN PGP SIGNED MESSAGE-----"));
        assert!(armored.contains("hello world"));

        let (csm, _) = CleartextSignedMessage::from_string(&armored).unwrap();
        csm.verify(&sk.public_key()).unwrap();
    }

    #[test]
    fn errors_clearly() {
        let sk = gen_secret();
        assert!(run("x", "", Mode::Detached, "").is_err());
        assert!(run("x", "not a key", Mode::Detached, "").is_err());
        // a public key is not a private key
        let pubk = sk.public_key().sign(OsRng, &sk.primary_key, || String::new()).unwrap();
        let pub_armored = pubk.to_armored_string(Default::default()).unwrap();
        assert!(run("x", &pub_armored, Mode::Detached, "").is_err());
    }

    #[test]
    fn mode_parse() {
        assert_eq!(Mode::parse("detached").unwrap(), Mode::Detached);
        assert_eq!(Mode::parse("CLEARSIGN").unwrap(), Mode::Clearsign);
        assert!(Mode::parse("nope").is_err());
    }
}
