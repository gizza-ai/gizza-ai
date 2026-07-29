//! pem-inspect core — parse and pretty-print PEM-encoded X.509 certificates,
//! CSRs, and public/private keys entirely locally (pure Rust). Shared by the
//! chat skill block, the CLI, and the browser page. Nothing is uploaded; private
//! keys are described by TYPE/ALGORITHM only — secret scalars are never printed.

use ::pem::parse_many;
use pkcs8::der::Decode;
use serde::Serialize;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use x509_parser::prelude::*;
use x509_parser::public_key::PublicKey;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Certificate(CertInfo),
    Csr(CsrInfo),
    PublicKey(KeyInfo),
    PrivateKey(KeyInfo),
}

#[derive(Debug, Clone, Serialize)]
pub struct CertInfo {
    pub version: String,
    pub serial: String,
    pub subject: String,
    pub issuer: String,
    pub self_signed: bool,
    pub not_before: String,
    pub not_after: String,
    pub status: String,
    pub days_until_expiry: i64,
    pub is_ca: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subject_alt_names: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub key_usage: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extended_key_usage: Vec<String>,
    pub public_key: PublicKeyInfo,
    pub signature_algorithm: String,
    pub fingerprint_sha256: String,
    pub fingerprint_sha1: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CsrInfo {
    pub subject: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subject_alt_names: Vec<String>,
    pub public_key: PublicKeyInfo,
    pub signature_algorithm: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct KeyInfo {
    pub format: String,
    pub algorithm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_size_bits: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curve: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicKeyInfo {
    pub algorithm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_size_bits: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curve: Option<String>,
}

/// Bit length of a big-endian, possibly zero-padded integer.
fn bit_len(bytes: &[u8]) -> usize {
    let mut i = 0;
    while i < bytes.len() && bytes[i] == 0 {
        i += 1;
    }
    if i == bytes.len() {
        return 0;
    }
    (bytes.len() - i) * 8 - bytes[i].leading_zeros() as usize
}

fn hex_colon(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            out.push(':');
        }
        out.push_str(&format!("{b:02X}"));
    }
    out
}

/// Human name for a signature/public-key algorithm OID, falling back to the OID.
fn oid_name(oid: &str) -> String {
    match oid {
        "1.2.840.113549.1.1.1" => "RSA",
        "1.2.840.113549.1.1.5" => "SHA-1 with RSA",
        "1.2.840.113549.1.1.11" => "SHA-256 with RSA",
        "1.2.840.113549.1.1.12" => "SHA-384 with RSA",
        "1.2.840.113549.1.1.13" => "SHA-512 with RSA",
        "1.2.840.113549.1.1.14" => "SHA-224 with RSA",
        "1.2.840.113549.1.1.10" => "RSASSA-PSS",
        "1.2.840.10045.2.1" => "EC",
        "1.2.840.10045.4.1" => "ECDSA with SHA-1",
        "1.2.840.10045.4.3.1" => "ECDSA with SHA-224",
        "1.2.840.10045.4.3.2" => "ECDSA with SHA-256",
        "1.2.840.10045.4.3.3" => "ECDSA with SHA-384",
        "1.2.840.10045.4.3.4" => "ECDSA with SHA-512",
        "1.3.101.112" => "Ed25519",
        "1.3.101.113" => "Ed448",
        "1.3.101.110" => "X25519",
        "1.3.101.111" => "X448",
        "1.2.840.10040.4.1" => "DSA",
        _ => return oid.to_string(),
    }
    .to_string()
}

/// Friendly name for a named-curve OID.
fn curve_name(oid: &str) -> String {
    match oid {
        "1.2.840.10045.3.1.7" => "P-256 (prime256v1)",
        "1.3.132.0.34" => "P-384 (secp384r1)",
        "1.3.132.0.35" => "P-521 (secp521r1)",
        "1.3.132.0.33" => "P-224 (secp224r1)",
        "1.3.132.0.10" => "secp256k1",
        "1.2.840.10045.3.1.1" => "P-192 (prime192v1)",
        _ => return oid.to_string(),
    }
    .to_string()
}

fn describe_spki(spki: &SubjectPublicKeyInfo) -> PublicKeyInfo {
    let alg_oid = spki.algorithm.algorithm.to_id_string();
    let algorithm = oid_name(&alg_oid);
    let mut key_size_bits = None;
    let mut curve = None;
    match spki.parsed() {
        Ok(PublicKey::RSA(rsa)) => key_size_bits = Some(bit_len(rsa.modulus)),
        Ok(PublicKey::EC(_)) => {
            curve = spki
                .algorithm
                .parameters
                .as_ref()
                .and_then(|p| p.as_oid().ok())
                .map(|o| curve_name(&o.to_id_string()));
        }
        _ => {}
    }
    // Ed25519/Ed448 have fixed sizes and no parsed() key.
    match alg_oid.as_str() {
        "1.3.101.112" | "1.3.101.110" => key_size_bits = Some(256),
        "1.3.101.113" | "1.3.101.111" => key_size_bits = Some(456),
        _ => {}
    }
    PublicKeyInfo {
        algorithm,
        key_size_bits,
        curve,
    }
}

fn general_name_to_string(gn: &GeneralName) -> Option<String> {
    match gn {
        GeneralName::DNSName(s) => Some(format!("DNS:{s}")),
        GeneralName::RFC822Name(s) => Some(format!("email:{s}")),
        GeneralName::URI(s) => Some(format!("URI:{s}")),
        GeneralName::IPAddress(bytes) => {
            let ip = match bytes.len() {
                4 => format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]),
                16 => {
                    let mut parts = Vec::with_capacity(8);
                    for chunk in bytes.chunks(2) {
                        parts.push(format!("{:x}", u16::from_be_bytes([chunk[0], chunk[1]])));
                    }
                    parts.join(":")
                }
                _ => hex_colon(bytes),
            };
            Some(format!("IP:{ip}"))
        }
        GeneralName::DirectoryName(name) => Some(format!("DirName:{name}")),
        _ => None,
    }
}

fn describe_certificate(der: &[u8], now_unix: i64) -> Result<CertInfo, String> {
    let (_, cert) =
        X509Certificate::from_der(der).map_err(|e| format!("not a valid X.509 certificate: {e}"))?;

    let mut subject_alt_names = Vec::new();
    let mut key_usage = Vec::new();
    let mut extended_key_usage = Vec::new();
    let mut is_ca = false;

    for ext in cert.extensions() {
        match ext.parsed_extension() {
            ParsedExtension::SubjectAlternativeName(san) => {
                for gn in &san.general_names {
                    if let Some(s) = general_name_to_string(gn) {
                        subject_alt_names.push(s);
                    }
                }
            }
            ParsedExtension::BasicConstraints(bc) => is_ca = bc.ca,
            ParsedExtension::KeyUsage(ku) => {
                let flags = [
                    (ku.digital_signature(), "digitalSignature"),
                    (ku.non_repudiation(), "nonRepudiation"),
                    (ku.key_encipherment(), "keyEncipherment"),
                    (ku.data_encipherment(), "dataEncipherment"),
                    (ku.key_agreement(), "keyAgreement"),
                    (ku.key_cert_sign(), "keyCertSign"),
                    (ku.crl_sign(), "cRLSign"),
                    (ku.encipher_only(), "encipherOnly"),
                    (ku.decipher_only(), "decipherOnly"),
                ];
                for (set, name) in flags {
                    if set {
                        key_usage.push(name.to_string());
                    }
                }
            }
            ParsedExtension::ExtendedKeyUsage(eku) => {
                let flags = [
                    (eku.any, "any"),
                    (eku.server_auth, "serverAuth"),
                    (eku.client_auth, "clientAuth"),
                    (eku.code_signing, "codeSigning"),
                    (eku.email_protection, "emailProtection"),
                    (eku.time_stamping, "timeStamping"),
                    (eku.ocsp_signing, "OCSPSigning"),
                ];
                for (set, name) in flags {
                    if set {
                        extended_key_usage.push(name.to_string());
                    }
                }
                for oid in &eku.other {
                    extended_key_usage.push(oid.to_id_string());
                }
            }
            _ => {}
        }
    }

    let validity = cert.validity();
    let not_before_ts = validity.not_before.timestamp();
    let not_after_ts = validity.not_after.timestamp();
    let days_until_expiry = (not_after_ts - now_unix) / 86_400;
    let status = if now_unix < not_before_ts {
        "not yet valid".to_string()
    } else if now_unix > not_after_ts {
        "expired".to_string()
    } else {
        "valid".to_string()
    };

    let mut sha256 = Sha256::new();
    sha256.update(der);
    let mut sha1 = Sha1::new();
    sha1.update(der);

    Ok(CertInfo {
        version: format!("v{}", cert.version().0 + 1),
        serial: cert.raw_serial_as_string(),
        subject: cert.subject().to_string(),
        issuer: cert.issuer().to_string(),
        self_signed: cert.subject() == cert.issuer(),
        not_before: validity.not_before.to_string(),
        not_after: validity.not_after.to_string(),
        status,
        days_until_expiry,
        is_ca,
        subject_alt_names,
        key_usage,
        extended_key_usage,
        public_key: describe_spki(cert.public_key()),
        signature_algorithm: oid_name(&cert.signature_algorithm.algorithm.to_id_string()),
        fingerprint_sha256: hex_colon(&sha256.finalize()),
        fingerprint_sha1: hex_colon(&sha1.finalize()),
    })
}

fn describe_csr(der: &[u8]) -> Result<CsrInfo, String> {
    let (_, csr) = X509CertificationRequest::from_der(der)
        .map_err(|e| format!("not a valid certificate request (CSR): {e}"))?;
    let cri = &csr.certification_request_info;

    let mut subject_alt_names = Vec::new();
    if let Some(exts) = csr.requested_extensions() {
        for ext in exts {
            if let ParsedExtension::SubjectAlternativeName(san) = ext {
                for gn in &san.general_names {
                    if let Some(s) = general_name_to_string(gn) {
                        subject_alt_names.push(s);
                    }
                }
            }
        }
    }

    Ok(CsrInfo {
        subject: cri.subject.to_string(),
        subject_alt_names,
        public_key: describe_spki(&cri.subject_pki),
        signature_algorithm: oid_name(&csr.signature_algorithm.algorithm.to_id_string()),
    })
}

fn describe_spki_key(der: &[u8]) -> Result<KeyInfo, String> {
    let (_, spki) = SubjectPublicKeyInfo::from_der(der)
        .map_err(|e| format!("not a valid SubjectPublicKeyInfo public key: {e}"))?;
    let pk = describe_spki(&spki);
    Ok(KeyInfo {
        format: "SubjectPublicKeyInfo (SPKI)".to_string(),
        algorithm: pk.algorithm,
        key_size_bits: pk.key_size_bits,
        curve: pk.curve,
        note: None,
    })
}

fn describe_rsa_public(der: &[u8]) -> Result<KeyInfo, String> {
    use pkcs1::RsaPublicKey;
    let rsa = RsaPublicKey::from_der(der).map_err(|e| format!("not a valid RSA public key: {e}"))?;
    Ok(KeyInfo {
        format: "PKCS#1 RSAPublicKey".to_string(),
        algorithm: "RSA".to_string(),
        key_size_bits: Some(bit_len(rsa.modulus.as_bytes())),
        curve: None,
        note: None,
    })
}

fn describe_pkcs8_private(der: &[u8]) -> Result<KeyInfo, String> {
    use pkcs8::PrivateKeyInfo;
    let pki =
        PrivateKeyInfo::try_from(der).map_err(|e| format!("not a valid PKCS#8 private key: {e}"))?;
    let alg_oid = pki.algorithm.oid.to_string();
    let algorithm = oid_name(&alg_oid);
    let mut key_size_bits = None;
    let mut curve = None;
    match alg_oid.as_str() {
        // RSA: inner key is a PKCS#1 RSAPrivateKey.
        "1.2.840.113549.1.1.1" => {
            if let Ok(rsa) = pkcs1::RsaPrivateKey::from_der(pki.private_key) {
                key_size_bits = Some(bit_len(rsa.modulus.as_bytes()));
            }
        }
        // EC: curve is in the AlgorithmIdentifier parameters.
        "1.2.840.10045.2.1" => {
            curve = pki
                .algorithm
                .parameters_oid()
                .ok()
                .map(|o| curve_name(&o.to_string()));
        }
        "1.3.101.112" | "1.3.101.110" => key_size_bits = Some(256),
        "1.3.101.113" | "1.3.101.111" => key_size_bits = Some(456),
        _ => {}
    }
    Ok(KeyInfo {
        format: "PKCS#8 PrivateKeyInfo".to_string(),
        algorithm,
        key_size_bits,
        curve,
        note: None,
    })
}

fn describe_rsa_private(der: &[u8]) -> Result<KeyInfo, String> {
    let rsa = pkcs1::RsaPrivateKey::from_der(der)
        .map_err(|e| format!("not a valid RSA private key: {e}"))?;
    Ok(KeyInfo {
        format: "PKCS#1 RSAPrivateKey".to_string(),
        algorithm: "RSA".to_string(),
        key_size_bits: Some(bit_len(rsa.modulus.as_bytes())),
        curve: None,
        note: None,
    })
}

fn describe_ec_private(der: &[u8]) -> Result<KeyInfo, String> {
    let ec = sec1::EcPrivateKey::from_der(der)
        .map_err(|e| format!("not a valid EC private key: {e}"))?;
    let curve = ec.parameters.and_then(|p| p.named_curve()).map(|o| curve_name(&o.to_string()));
    Ok(KeyInfo {
        format: "SEC1 ECPrivateKey".to_string(),
        algorithm: "EC".to_string(),
        key_size_bits: None,
        curve,
        note: None,
    })
}

/// Parse every PEM block in `input` and describe each. `now_unix` is the current
/// time (Unix seconds) used to compute certificate expiry — each surface supplies
/// its own clock so the core stays deterministic.
pub fn inspect(input: &str, now_unix: i64) -> Result<Vec<Block>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("paste one or more PEM blocks (a '-----BEGIN …-----' block)".into());
    }
    let pems = parse_many(trimmed)
        .map_err(|e| format!("could not read PEM: {e}"))?;
    if pems.is_empty() {
        return Err(
            "no PEM block found — paste a '-----BEGIN CERTIFICATE-----' (or CSR / key) block".into(),
        );
    }

    let mut blocks = Vec::with_capacity(pems.len());
    for p in &pems {
        let der = p.contents();
        let tag = p.tag().to_ascii_uppercase();
        let block = match tag.as_str() {
            "CERTIFICATE" | "TRUSTED CERTIFICATE" | "X509 CERTIFICATE" => {
                Block::Certificate(describe_certificate(der, now_unix)?)
            }
            "CERTIFICATE REQUEST" | "NEW CERTIFICATE REQUEST" => Block::Csr(describe_csr(der)?),
            "PUBLIC KEY" => Block::PublicKey(describe_spki_key(der)?),
            "RSA PUBLIC KEY" => Block::PublicKey(describe_rsa_public(der)?),
            "PRIVATE KEY" => Block::PrivateKey(describe_pkcs8_private(der)?),
            "RSA PRIVATE KEY" => Block::PrivateKey(describe_rsa_private(der)?),
            "EC PRIVATE KEY" => Block::PrivateKey(describe_ec_private(der)?),
            "ENCRYPTED PRIVATE KEY" => Block::PrivateKey(KeyInfo {
                format: "PKCS#8 EncryptedPrivateKeyInfo".to_string(),
                algorithm: "unknown".to_string(),
                key_size_bits: None,
                curve: None,
                note: Some(
                    "encrypted — decrypt with the password first (openssl pkcs8) to inspect".into(),
                ),
            }),
            other => {
                return Err(format!(
                    "unsupported PEM block '{other}' — supported: CERTIFICATE, CERTIFICATE REQUEST, PUBLIC KEY, RSA PUBLIC KEY, PRIVATE KEY, RSA PRIVATE KEY, EC PRIVATE KEY"
                ))
            }
        };
        blocks.push(block);
    }
    Ok(blocks)
}

/// Inspect PEM input and return pretty-printed JSON (one object per block).
pub fn run(input: &str, now_unix: i64) -> Result<String, String> {
    let blocks = inspect(input, now_unix)?;
    serde_json::to_string_pretty(&blocks).map_err(|e| format!("serialize result: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A self-signed leaf certificate (CN=pem-inspect.example, RSA-2048), fixed
    // validity 2026-07-29 .. 2046-07-24, generated offline for these tests.
    const CERT_PEM: &str = include_str!("../tests/fixtures/cert.pem");
    const KEY_PEM: &str = include_str!("../tests/fixtures/key.pem");
    const CSR_PEM: &str = include_str!("../tests/fixtures/csr.pem");
    const PUB_PEM: &str = include_str!("../tests/fixtures/pub.pem");

    // 2027-01-01T00:00:00Z — inside the fixture's validity window.
    const NOW: i64 = 1_798_761_600;

    #[test]
    fn parses_certificate_fields() {
        let blocks = inspect(CERT_PEM, NOW).unwrap();
        assert_eq!(blocks.len(), 1);
        let Block::Certificate(c) = &blocks[0] else {
            panic!("expected certificate");
        };
        assert_eq!(c.version, "v3");
        assert!(c.subject.contains("pem-inspect.example"));
        assert_eq!(c.status, "valid");
        assert!(c.self_signed);
        assert_eq!(c.public_key.algorithm, "RSA");
        assert_eq!(c.public_key.key_size_bits, Some(2048));
        assert!(c.signature_algorithm.contains("RSA"));
        assert_eq!(c.fingerprint_sha256.len(), 32 * 3 - 1);
        assert!(c
            .subject_alt_names
            .iter()
            .any(|s| s.contains("pem-inspect.example")));
    }

    #[test]
    fn reports_expiry_relative_to_now() {
        let blocks = inspect(CERT_PEM, NOW).unwrap();
        let Block::Certificate(c) = &blocks[0] else {
            panic!()
        };
        assert!(c.days_until_expiry > 0);
        // Far future → expired.
        let future = inspect(CERT_PEM, 2_524_608_000).unwrap();
        let Block::Certificate(c2) = &future[0] else {
            panic!()
        };
        assert_eq!(c2.status, "expired");
        assert!(c2.days_until_expiry < 0);
    }

    #[test]
    fn parses_pkcs8_private_key_metadata_only() {
        let blocks = inspect(KEY_PEM, NOW).unwrap();
        let Block::PrivateKey(k) = &blocks[0] else {
            panic!("expected private key");
        };
        assert_eq!(k.algorithm, "RSA");
        assert_eq!(k.key_size_bits, Some(2048));
        // The JSON must never leak private material.
        let json = run(KEY_PEM, NOW).unwrap();
        assert!(!json.contains("privateExponent"));
        assert!(json.contains("RSA"));
    }

    #[test]
    fn parses_csr() {
        let blocks = inspect(CSR_PEM, NOW).unwrap();
        let Block::Csr(c) = &blocks[0] else {
            panic!("expected CSR");
        };
        assert!(c.subject.contains("pem-inspect.example"));
        assert_eq!(c.public_key.algorithm, "RSA");
    }

    #[test]
    fn parses_spki_public_key() {
        let blocks = inspect(PUB_PEM, NOW).unwrap();
        let Block::PublicKey(k) = &blocks[0] else {
            panic!("expected public key");
        };
        assert_eq!(k.algorithm, "RSA");
        assert_eq!(k.key_size_bits, Some(2048));
    }

    #[test]
    fn parses_multiple_blocks_in_one_input() {
        let combined = format!("{CERT_PEM}\n{PUB_PEM}");
        let blocks = inspect(&combined, NOW).unwrap();
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn rejects_empty_input() {
        let err = inspect("   ", NOW).unwrap_err();
        assert!(err.contains("paste"));
    }

    #[test]
    fn rejects_non_pem_input() {
        let err = inspect("just some text", NOW).unwrap_err();
        assert!(err.to_lowercase().contains("pem"));
    }

    #[test]
    fn rejects_corrupt_certificate() {
        let bad = "-----BEGIN CERTIFICATE-----\nMIIBmm\n-----END CERTIFICATE-----\n";
        let err = inspect(bad, NOW).unwrap_err();
        assert!(err.contains("certificate") || err.contains("PEM"));
    }
}
