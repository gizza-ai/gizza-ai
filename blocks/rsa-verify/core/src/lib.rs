//! gizza-ai/rsa-verify core — verify an RSA signature (PKCS#1 v1.5 or PSS) over a
//! message against an RSA public key, with SHA-256/384/512. The signature is
//! base64-encoded. Pure-Rust (`rsa` + `sha2`). No wafer/wasm-bindgen deps.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::signature::Verifier;
use rsa::RsaPublicKey;
use sha2::{Sha256, Sha384, Sha512};

/// Signature scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Pkcs1v15,
    Pss,
}

impl Scheme {
    pub fn parse(s: &str) -> Result<Scheme, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "pkcs1v15" | "pkcs1" | "" => Ok(Scheme::Pkcs1v15),
            "pss" => Ok(Scheme::Pss),
            other => Err(format!("unknown scheme '{other}' (use 'pkcs1v15' or 'pss')")),
        }
    }
}

/// Digest algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hash {
    Sha256,
    Sha384,
    Sha512,
}

impl Hash {
    pub fn parse(s: &str) -> Result<Hash, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "sha256" | "" => Ok(Hash::Sha256),
            "sha384" => Ok(Hash::Sha384),
            "sha512" => Ok(Hash::Sha512),
            other => Err(format!("unknown hash '{other}' (use 'sha256', 'sha384', or 'sha512')")),
        }
    }
}

fn parse_key(pem: &str) -> Result<RsaPublicKey, String> {
    if !pem.contains("PUBLIC KEY") {
        return Err(
            "no RSA public key found — paste a PEM '-----BEGIN PUBLIC KEY-----' (SPKI) or '-----BEGIN RSA PUBLIC KEY-----' (PKCS#1) block"
                .into(),
        );
    }
    // Try SPKI (X.509 SubjectPublicKeyInfo) first, then PKCS#1.
    RsaPublicKey::from_public_key_pem(pem)
        .or_else(|_| RsaPublicKey::from_pkcs1_pem(pem))
        .map_err(|e| format!("invalid RSA public key: {e}"))
}

fn decode_sig(signature_b64: &str) -> Result<Vec<u8>, String> {
    let s: String = signature_b64.split_whitespace().collect();
    if s.is_empty() {
        return Err("empty signature — paste the base64-encoded signature".into());
    }
    B64.decode(s.as_bytes())
        .map_err(|e| format!("signature is not valid base64: {e}"))
}

/// Verify that `signature_b64` (base64) is a valid signature over `message` under
/// `public_key_pem`, using the given `scheme` and `hash`. Returns `Ok(true)` if the
/// signature is valid, `Ok(false)` if it is well-formed but does not verify, and
/// `Err` only for malformed inputs (bad key, bad base64, etc.).
pub fn verify(
    message: &str,
    signature_b64: &str,
    public_key_pem: &str,
    scheme: Scheme,
    hash: Hash,
) -> Result<bool, String> {
    let key = parse_key(public_key_pem)?;
    let sig_bytes = decode_sig(signature_b64)?;
    let msg = message.as_bytes();

    let ok = match scheme {
        Scheme::Pkcs1v15 => {
            let sig = rsa::pkcs1v15::Signature::try_from(sig_bytes.as_slice())
                .map_err(|e| format!("malformed PKCS#1 v1.5 signature: {e}"))?;
            match hash {
                Hash::Sha256 => rsa::pkcs1v15::VerifyingKey::<Sha256>::new(key)
                    .verify(msg, &sig)
                    .is_ok(),
                Hash::Sha384 => rsa::pkcs1v15::VerifyingKey::<Sha384>::new(key)
                    .verify(msg, &sig)
                    .is_ok(),
                Hash::Sha512 => rsa::pkcs1v15::VerifyingKey::<Sha512>::new(key)
                    .verify(msg, &sig)
                    .is_ok(),
            }
        }
        Scheme::Pss => {
            let sig = rsa::pss::Signature::try_from(sig_bytes.as_slice())
                .map_err(|e| format!("malformed PSS signature: {e}"))?;
            match hash {
                Hash::Sha256 => rsa::pss::VerifyingKey::<Sha256>::new(key)
                    .verify(msg, &sig)
                    .is_ok(),
                Hash::Sha384 => rsa::pss::VerifyingKey::<Sha384>::new(key)
                    .verify(msg, &sig)
                    .is_ok(),
                Hash::Sha512 => rsa::pss::VerifyingKey::<Sha512>::new(key)
                    .verify(msg, &sig)
                    .is_ok(),
            }
        }
    };

    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs8::{EncodePublicKey, LineEnding};
    use rsa::rand_core::OsRng;
    use rsa::signature::{RandomizedSigner, SignatureEncoding, Signer};
    use rsa::RsaPrivateKey;

    fn test_keys() -> (RsaPrivateKey, String) {
        let mut rng = OsRng;
        let priv_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub_pem = RsaPublicKey::from(&priv_key)
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        (priv_key, pub_pem)
    }

    #[test]
    fn pkcs1v15_sha256_roundtrip() {
        let (priv_key, pub_pem) = test_keys();
        let sig = rsa::pkcs1v15::SigningKey::<Sha256>::new(priv_key)
            .sign(b"hello")
            .to_vec();
        let b64 = B64.encode(&sig);
        assert!(verify("hello", &b64, &pub_pem, Scheme::Pkcs1v15, Hash::Sha256).unwrap());
        // Wrong message -> false, not error.
        assert!(!verify("goodbye", &b64, &pub_pem, Scheme::Pkcs1v15, Hash::Sha256).unwrap());
        // Wrong hash -> false.
        assert!(!verify("hello", &b64, &pub_pem, Scheme::Pkcs1v15, Hash::Sha512).unwrap());
    }

    #[test]
    fn pss_sha512_roundtrip() {
        let (priv_key, pub_pem) = test_keys();
        let mut rng = OsRng;
        let sig = rsa::pss::SigningKey::<Sha512>::new(priv_key)
            .sign_with_rng(&mut rng, b"data to sign")
            .to_vec();
        let b64 = B64.encode(&sig);
        assert!(verify("data to sign", &b64, &pub_pem, Scheme::Pss, Hash::Sha512).unwrap());
        assert!(!verify("tampered", &b64, &pub_pem, Scheme::Pss, Hash::Sha512).unwrap());
        // PSS signature checked under PKCS#1 v1.5 -> invalid (false).
        assert!(!verify("data to sign", &b64, &pub_pem, Scheme::Pkcs1v15, Hash::Sha512).unwrap());
    }

    #[test]
    fn whitespace_in_signature_is_tolerated() {
        let (priv_key, pub_pem) = test_keys();
        let sig = rsa::pkcs1v15::SigningKey::<Sha256>::new(priv_key)
            .sign(b"x")
            .to_vec();
        let b64 = B64.encode(&sig);
        let wrapped = format!("{}\n{}", &b64[..20], &b64[20..]);
        assert!(verify("x", &wrapped, &pub_pem, Scheme::Pkcs1v15, Hash::Sha256).unwrap());
    }

    #[test]
    fn errors() {
        let (_priv, pub_pem) = test_keys();
        assert!(verify("x", "AAAA", "not a key", Scheme::Pkcs1v15, Hash::Sha256).is_err());
        assert!(verify("x", "!!!notbase64!!!", &pub_pem, Scheme::Pkcs1v15, Hash::Sha256).is_err());
        assert!(verify("x", "", &pub_pem, Scheme::Pkcs1v15, Hash::Sha256).is_err());
        assert!(Scheme::parse("nope").is_err());
        assert!(Hash::parse("md5").is_err());
        assert_eq!(Scheme::parse("PSS").unwrap(), Scheme::Pss);
        assert_eq!(Hash::parse("sha-384").unwrap(), Hash::Sha384);
    }
}
