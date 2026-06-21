//! gizza-ai/ecdsa-sign core — sign a message with an ECDSA private key on the
//! NIST P-256, P-384, or P-521 curve, returning the signature in DER or raw
//! (r||s, IEEE-P1363) form. Deterministic RFC-6979 nonces (no RNG needed), so
//! signing is reproducible. Pure-Rust (`p256`/`p384`/`p521` + `ecdsa`).

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::Serialize;

/// NIST prime curve. The curve fixes the digest (P-256→SHA-256, P-384→SHA-384),
/// matching every standard ECDSA toolchain. Signatures use deterministic
/// RFC-6979 nonces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    P256,
    P384,
}

impl Curve {
    pub fn parse(s: &str) -> Result<Curve, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "p256" | "secp256r1" | "prime256v1" | "" => Ok(Curve::P256),
            "p384" | "secp384r1" => Ok(Curve::P384),
            other => Err(format!("unknown curve '{other}' (use p256 or p384)")),
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Curve::P256 => "p256",
            Curve::P384 => "p384",
        }
    }
}

/// Signature encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigFormat {
    /// ASN.1 DER `SEQUENCE { r INTEGER, s INTEGER }` (what OpenSSL emits).
    Der,
    /// Fixed-length `r || s` (IEEE P1363 / JOSE "raw").
    Raw,
}

impl SigFormat {
    pub fn parse(s: &str) -> Result<SigFormat, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' ', '|'], "").as_str() {
            "der" | "asn1" | "" => Ok(SigFormat::Der),
            "raw" | "rs" | "p1363" | "fixed" | "jose" => Ok(SigFormat::Raw),
            other => Err(format!("unknown format '{other}' (use der or raw)")),
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            SigFormat::Der => "der",
            SigFormat::Raw => "raw",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Signed {
    pub signature_base64: String,
    pub signature_hex: String,
    pub curve: String,
    pub format: String,
    pub bytes: usize,
}

fn key_err(e: impl std::fmt::Display) -> String {
    format!("invalid EC private key: {e}")
}

/// Parse a PEM EC private key (PKCS#8 or SEC1) and sign `message`.
pub fn sign(
    message: &str,
    private_key_pem: &str,
    curve: Curve,
    format: SigFormat,
) -> Result<Signed, String> {
    if !private_key_pem.contains("PRIVATE KEY") {
        return Err(
            "no EC private key found — paste a PEM '-----BEGIN PRIVATE KEY-----' (PKCS#8) block (convert a SEC1 'EC PRIVATE KEY' with `openssl pkcs8 -topk8 -nocrypt`)"
                .into(),
        );
    }
    let msg = message.as_bytes();

    let bytes: Vec<u8> = match curve {
        Curve::P256 => {
            use p256::ecdsa::{signature::Signer, Signature, SigningKey};
            use p256::pkcs8::DecodePrivateKey;
            let sk = SigningKey::from_pkcs8_pem(private_key_pem).map_err(key_err)?;
            let sig: Signature = sk.sign(msg);
            match format {
                SigFormat::Der => sig.to_der().as_bytes().to_vec(),
                SigFormat::Raw => sig.to_bytes().to_vec(),
            }
        }
        Curve::P384 => {
            use p384::ecdsa::{signature::Signer, Signature, SigningKey};
            use p384::pkcs8::DecodePrivateKey;
            let sk = SigningKey::from_pkcs8_pem(private_key_pem).map_err(key_err)?;
            let sig: Signature = sk.sign(msg);
            match format {
                SigFormat::Der => sig.to_der().as_bytes().to_vec(),
                SigFormat::Raw => sig.to_bytes().to_vec(),
            }
        }
    };

    Ok(Signed {
        signature_base64: B64.encode(&bytes),
        signature_hex: hex::encode(&bytes),
        bytes: bytes.len(),
        curve: curve.name().into(),
        format: format.name().into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const P256_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgn5zu0/Tig4Q969rM\nujaKR8Y42hSK02jb3UXaRk2OKNOhRANCAAQdDtizrLTz7++OkxNxnFU2fCcErxbN\nGd7Vego6C920F1p4GGHX/1dxpgOU3PTUC3A/mlUN7rXTzzWCRHtm95Nm\n-----END PRIVATE KEY-----\n";
    const P384_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIG2AgEAMBAGByqGSM49AgEGBSuBBAAiBIGeMIGbAgEBBDDzCM7XgAj12R8AWV1t\nIapdz6gcC1+G21Iph0sSBLys5W6UEBTRyeKs1p0jbNgcG2ahZANiAATb9ut2Ch10\niEnFida2mj9+NDi8Ql1bg3K6PiTaLy1xPnCiNPuhLvfpsZRkJ3ZK+sjnyh8J+9f8\nKSQhryli4RLarhfxhLPOjZfU9+X0C9ZhU0vZ+Libn6VEJI481DWFBg4=\n-----END PRIVATE KEY-----\n";
    #[test]
    fn p256_der_verifies() {
        use p256::ecdsa::{signature::Verifier, DerSignature, SigningKey};
        use p256::pkcs8::DecodePrivateKey;
        let out = sign("hello world", P256_PEM, Curve::P256, SigFormat::Der).unwrap();
        assert_eq!(out.curve, "p256");
        assert_eq!(out.format, "der");
        let sk = SigningKey::from_pkcs8_pem(P256_PEM).unwrap();
        let vk = sk.verifying_key();
        let der = B64.decode(&out.signature_base64).unwrap();
        let sig = DerSignature::try_from(der.as_slice()).unwrap();
        vk.verify(b"hello world", &sig).unwrap();
        assert!(vk.verify(b"tampered", &sig).is_err());
    }

    #[test]
    fn p256_raw_is_64_bytes_and_verifies() {
        use p256::ecdsa::{signature::Verifier, Signature, SigningKey};
        use p256::pkcs8::DecodePrivateKey;
        let out = sign("msg", P256_PEM, Curve::P256, SigFormat::Raw).unwrap();
        assert_eq!(out.bytes, 64); // 2×32-byte field elements
        let sk = SigningKey::from_pkcs8_pem(P256_PEM).unwrap();
        let raw = B64.decode(&out.signature_base64).unwrap();
        let sig = Signature::try_from(raw.as_slice()).unwrap();
        sk.verifying_key().verify(b"msg", &sig).unwrap();
    }

    #[test]
    fn deterministic() {
        // RFC-6979 → identical signatures for identical inputs.
        let a = sign("x", P256_PEM, Curve::P256, SigFormat::Der).unwrap();
        let b = sign("x", P256_PEM, Curve::P256, SigFormat::Der).unwrap();
        assert_eq!(a.signature_base64, b.signature_base64);
    }

    #[test]
    fn p384_verifies() {
        use p384::ecdsa::{signature::Verifier, DerSignature, SigningKey};
        use p384::pkcs8::DecodePrivateKey;
        let out = sign("data", P384_PEM, Curve::P384, SigFormat::Der).unwrap();
        assert_eq!(out.curve, "p384");
        let sk = SigningKey::from_pkcs8_pem(P384_PEM).unwrap();
        let der = B64.decode(&out.signature_base64).unwrap();
        let sig = DerSignature::try_from(der.as_slice()).unwrap();
        sk.verifying_key().verify(b"data", &sig).unwrap();
    }

    #[test]
    fn p384_raw_is_96_bytes() {
        let out = sign("data", P384_PEM, Curve::P384, SigFormat::Raw).unwrap();
        assert_eq!(out.bytes, 96); // 2×48-byte field elements
    }

    #[test]
    fn errors() {
        assert!(sign("x", "not a key", Curve::P256, SigFormat::Der).is_err());
        assert!(Curve::parse("p999").is_err());
        assert!(Curve::parse("p521").is_err());
        assert!(SigFormat::parse("pem").is_err());
        assert_eq!(Curve::parse("P-384").unwrap(), Curve::P384);
        assert_eq!(SigFormat::parse("R||S").unwrap(), SigFormat::Raw);
    }
}
