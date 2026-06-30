//! gizza-ai/rsa-encrypt core — encrypt a small payload to an RSA public key using
//! either OAEP (with SHA-256/384/512) or PKCS#1 v1.5 padding, returning base64
//! ciphertext. Pure-Rust (`rsa` + `sha2`). No wafer/wasm-bindgen deps.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::rand_core::OsRng;
use rsa::{Oaep, Pkcs1v15Encrypt, RsaPublicKey};
use sha2::{Sha256, Sha384, Sha512};

/// Padding scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Padding {
    Oaep,
    Pkcs1v15,
}

impl Padding {
    pub fn parse(s: &str) -> Result<Padding, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' ', '.'], "").as_str() {
            "oaep" | "" => Ok(Padding::Oaep),
            "pkcs1v15" | "pkcs1" => Ok(Padding::Pkcs1v15),
            other => Err(format!("unknown padding '{other}' (use 'oaep' or 'pkcs1v15')")),
        }
    }
}

/// Hash used by OAEP (the MGF1 + label digest). Ignored for PKCS#1 v1.5.
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
    // Try SPKI (PKCS#8) first, then PKCS#1.
    RsaPublicKey::from_public_key_pem(pem)
        .or_else(|_| RsaPublicKey::from_pkcs1_pem(pem))
        .map_err(|e| format!("invalid RSA public key: {e}"))
}

/// Encrypt `message` to `public_key_pem` and return the base64-encoded ciphertext.
pub fn encrypt(
    message: &str,
    public_key_pem: &str,
    padding: Padding,
    hash: Hash,
) -> Result<String, String> {
    let key = parse_key(public_key_pem)?;
    let data = message.as_bytes();
    let mut rng = OsRng;

    let ct: Vec<u8> = match padding {
        Padding::Pkcs1v15 => key
            .encrypt(&mut rng, Pkcs1v15Encrypt, data)
            .map_err(|e| format!("encryption failed (message too long for this key/padding?): {e}"))?,
        Padding::Oaep => {
            let pad = match hash {
                Hash::Sha256 => Oaep::new::<Sha256>(),
                Hash::Sha384 => Oaep::new::<Sha384>(),
                Hash::Sha512 => Oaep::new::<Sha512>(),
            };
            key.encrypt(&mut rng, pad, data)
                .map_err(|e| format!("encryption failed (message too long for this key/padding?): {e}"))?
        }
    };

    Ok(B64.encode(ct))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs8::{EncodePublicKey, LineEnding};
    use rsa::RsaPrivateKey;

    fn test_keypair() -> (String, RsaPrivateKey) {
        let mut rng = OsRng;
        let priv_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub_key = RsaPublicKey::from(&priv_key);
        let pem = pub_key.to_public_key_pem(LineEnding::LF).unwrap();
        (pem, priv_key)
    }

    #[test]
    fn oaep_sha256_roundtrips() {
        let (pem, priv_key) = test_keypair();
        let b64 = encrypt("hello world", &pem, Padding::Oaep, Hash::Sha256).unwrap();
        let ct = B64.decode(&b64).unwrap();
        let pt = priv_key.decrypt(Oaep::new::<Sha256>(), &ct).unwrap();
        assert_eq!(pt, b"hello world");
    }

    #[test]
    fn pkcs1v15_roundtrips() {
        let (pem, priv_key) = test_keypair();
        let b64 = encrypt("secret payload", &pem, Padding::Pkcs1v15, Hash::Sha256).unwrap();
        let ct = B64.decode(&b64).unwrap();
        let pt = priv_key.decrypt(Pkcs1v15Encrypt, &ct).unwrap();
        assert_eq!(pt, b"secret payload");
    }

    #[test]
    fn oaep_sha512_roundtrips() {
        let (pem, priv_key) = test_keypair();
        let b64 = encrypt("longer hash", &pem, Padding::Oaep, Hash::Sha512).unwrap();
        let ct = B64.decode(&b64).unwrap();
        let pt = priv_key.decrypt(Oaep::new::<Sha512>(), &ct).unwrap();
        assert_eq!(pt, b"longer hash");
    }

    #[test]
    fn randomized_each_call() {
        let (pem, _) = test_keypair();
        let a = encrypt("x", &pem, Padding::Oaep, Hash::Sha256).unwrap();
        let b = encrypt("x", &pem, Padding::Oaep, Hash::Sha256).unwrap();
        assert_ne!(a, b, "RSA encryption is randomized");
    }

    #[test]
    fn too_long_errors() {
        // OAEP-SHA256 max for a 2048-bit key is 256 - 2*32 - 2 = 190 bytes.
        let (pem, _) = test_keypair();
        let big = "a".repeat(300);
        assert!(encrypt(&big, &pem, Padding::Oaep, Hash::Sha256).is_err());
    }

    #[test]
    fn errors() {
        assert!(encrypt("x", "not a key", Padding::Oaep, Hash::Sha256).is_err());
        assert!(Padding::parse("nope").is_err());
        assert!(Hash::parse("md5").is_err());
        assert_eq!(Padding::parse("PKCS1-v1.5").unwrap(), Padding::Pkcs1v15);
        assert_eq!(Padding::parse("OAEP").unwrap(), Padding::Oaep);
        assert_eq!(Hash::parse("sha-384").unwrap(), Hash::Sha384);
    }
}
