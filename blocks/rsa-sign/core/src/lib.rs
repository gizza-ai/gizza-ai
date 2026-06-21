//! gizza-ai/rsa-sign core — sign a message with an RSA private key using either
//! PKCS#1 v1.5 or PSS, with SHA-256/384/512, returning a base64 signature.
//! Pure-Rust (`rsa` + `sha2`). No wafer/wasm-bindgen deps.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::rand_core::OsRng;
use rsa::signature::{RandomizedSigner, SignatureEncoding, Signer};
use rsa::RsaPrivateKey;
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

fn parse_key(pem: &str) -> Result<RsaPrivateKey, String> {
    if !pem.contains("PRIVATE KEY") {
        return Err(
            "no RSA private key found — paste a PEM '-----BEGIN PRIVATE KEY-----' or '-----BEGIN RSA PRIVATE KEY-----' block"
                .into(),
        );
    }
    // Try PKCS#8 first, then PKCS#1.
    RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
        .map_err(|e| format!("invalid RSA private key: {e}"))
}

/// Sign `message` and return the base64-encoded signature.
pub fn sign(
    message: &str,
    private_key_pem: &str,
    scheme: Scheme,
    hash: Hash,
) -> Result<String, String> {
    let key = parse_key(private_key_pem)?;
    let msg = message.as_bytes();

    let sig_bytes: Vec<u8> = match (scheme, hash) {
        (Scheme::Pkcs1v15, Hash::Sha256) => {
            rsa::pkcs1v15::SigningKey::<Sha256>::new(key).sign(msg).to_vec()
        }
        (Scheme::Pkcs1v15, Hash::Sha384) => {
            rsa::pkcs1v15::SigningKey::<Sha384>::new(key).sign(msg).to_vec()
        }
        (Scheme::Pkcs1v15, Hash::Sha512) => {
            rsa::pkcs1v15::SigningKey::<Sha512>::new(key).sign(msg).to_vec()
        }
        (Scheme::Pss, Hash::Sha256) => rsa::pss::SigningKey::<Sha256>::new(key)
            .sign_with_rng(&mut OsRng, msg)
            .to_vec(),
        (Scheme::Pss, Hash::Sha384) => rsa::pss::SigningKey::<Sha384>::new(key)
            .sign_with_rng(&mut OsRng, msg)
            .to_vec(),
        (Scheme::Pss, Hash::Sha512) => rsa::pss::SigningKey::<Sha512>::new(key)
            .sign_with_rng(&mut OsRng, msg)
            .to_vec(),
    };

    Ok(B64.encode(sig_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs8::EncodePrivateKey;
    use rsa::signature::Verifier;
    use rsa::RsaPublicKey;

    fn test_key_pem() -> (String, RsaPublicKey) {
        let mut rng = OsRng;
        let priv_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub_key = RsaPublicKey::from(&priv_key);
        let pem = priv_key
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap()
            .to_string();
        (pem, pub_key)
    }

    #[test]
    fn pkcs1v15_sha256_verifies() {
        let (pem, pubk) = test_key_pem();
        let b64 = sign("hello", &pem, Scheme::Pkcs1v15, Hash::Sha256).unwrap();
        let sig_bytes = B64.decode(&b64).unwrap();
        let sig = rsa::pkcs1v15::Signature::try_from(sig_bytes.as_slice()).unwrap();
        let vk = rsa::pkcs1v15::VerifyingKey::<Sha256>::new(pubk);
        vk.verify(b"hello", &sig).unwrap();
        assert!(vk.verify(b"goodbye", &sig).is_err());
    }

    #[test]
    fn pss_sha512_verifies() {
        let (pem, pubk) = test_key_pem();
        let b64 = sign("data to sign", &pem, Scheme::Pss, Hash::Sha512).unwrap();
        let sig_bytes = B64.decode(&b64).unwrap();
        let sig = rsa::pss::Signature::try_from(sig_bytes.as_slice()).unwrap();
        let vk = rsa::pss::VerifyingKey::<Sha512>::new(pubk);
        vk.verify(b"data to sign", &sig).unwrap();
    }

    #[test]
    fn deterministic_pkcs1v15() {
        let (pem, _) = test_key_pem();
        let a = sign("x", &pem, Scheme::Pkcs1v15, Hash::Sha256).unwrap();
        let b = sign("x", &pem, Scheme::Pkcs1v15, Hash::Sha256).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn errors() {
        assert!(sign("x", "not a key", Scheme::Pkcs1v15, Hash::Sha256).is_err());
        assert!(Scheme::parse("nope").is_err());
        assert!(Hash::parse("md5").is_err());
        assert_eq!(Scheme::parse("PSS").unwrap(), Scheme::Pss);
        assert_eq!(Hash::parse("sha-384").unwrap(), Hash::Sha384);
    }
}
