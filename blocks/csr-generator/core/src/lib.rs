//! gizza-ai/csr-generator core — generate a fresh private key and PKCS#10 CSR.
//! No wafer/wasm-bindgen deps. Pure-Rust RustCrypto ECDSA keys and a small
//! PKCS#10 encoder; the CSPRNG is `getrandom`, so it runs on native and WASI.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use p256::ecdsa::signature::Signer;
use p256::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use rand_core::OsRng;
use serde::Serialize;
use std::net::IpAddr;
use yasna::models::ObjectIdentifier;
use yasna::Tag;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    P256,
    P384,
}

impl Algorithm {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "p256" | "ecdsa-p256" | "prime256v1" | "secp256r1" => Ok(Self::P256),
            "p384" | "ecdsa-p384" | "secp384r1" => Ok(Self::P384),
            other => Err(format!(
                "unsupported algorithm '{other}'; expected p256 or p384"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::P256 => "p256",
            Self::P384 => "p384",
        }
    }

    fn signature_oid(self) -> &'static [u64] {
        match self {
            // ecdsa-with-SHA256 / ecdsa-with-SHA384
            Self::P256 => &[1, 2, 840, 10045, 4, 3, 2],
            Self::P384 => &[1, 2, 840, 10045, 4, 3, 3],
        }
    }
}

#[derive(Debug, Clone)]
pub struct CsrRequest {
    pub algorithm: Algorithm,
    pub common_name: String,
    pub organization: String,
    pub organizational_unit: String,
    pub country: String,
    pub state: String,
    pub locality: String,
    pub san_dns: String,
    pub san_ips: String,
    pub san_emails: String,
    pub san_uris: String,
}

#[derive(Debug, Serialize)]
pub struct CsrOutput {
    pub algorithm: String,
    pub subject: String,
    pub subject_alt_names: Vec<String>,
    pub private_key_pem: String,
    pub public_key_pem: String,
    pub csr_pem: String,
}

#[derive(Debug, Clone)]
enum GeneralName {
    Dns(String),
    Ip(IpAddr),
    Email(String),
    Uri(String),
}

pub fn generate(req: CsrRequest) -> Result<CsrOutput, String> {
    let common_name = clean_required(&req.common_name, "common_name")?;
    validate_country(&req.country)?;
    let names = parse_sans(&req, &common_name)?;
    let subject = subject_summary(&common_name, &req);
    let spki_der;
    let private_key_pem;
    let public_key_pem;
    let signature_der;
    let cri_der;

    match req.algorithm {
        Algorithm::P256 => {
            let signing_key = p256::ecdsa::SigningKey::random(&mut OsRng);
            let verifying_key = signing_key.verifying_key();
            spki_der = verifying_key
                .to_public_key_der()
                .map_err(|e| format!("public key DER encoding failed: {e}"))?
                .as_bytes()
                .to_vec();
            private_key_pem = signing_key
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(|e| format!("private key PEM encoding failed: {e}"))?
                .to_string();
            public_key_pem = verifying_key
                .to_public_key_pem(LineEnding::LF)
                .map_err(|e| format!("public key PEM encoding failed: {e}"))?;
            cri_der = certification_request_info(&req, &common_name, &spki_der, &names)?;
            let signature: p256::ecdsa::Signature = signing_key.sign(&cri_der);
            signature_der = signature.to_der().as_bytes().to_vec();
        }
        Algorithm::P384 => {
            let signing_key = p384::ecdsa::SigningKey::random(&mut OsRng);
            let verifying_key = signing_key.verifying_key();
            spki_der = verifying_key
                .to_public_key_der()
                .map_err(|e| format!("public key DER encoding failed: {e}"))?
                .as_bytes()
                .to_vec();
            private_key_pem = signing_key
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(|e| format!("private key PEM encoding failed: {e}"))?
                .to_string();
            public_key_pem = verifying_key
                .to_public_key_pem(LineEnding::LF)
                .map_err(|e| format!("public key PEM encoding failed: {e}"))?;
            cri_der = certification_request_info(&req, &common_name, &spki_der, &names)?;
            let signature: p384::ecdsa::Signature = signing_key.sign(&cri_der);
            signature_der = signature.to_der().as_bytes().to_vec();
        }
    }

    let csr_der = yasna::construct_der(|writer| {
        writer.write_sequence(|writer| {
            writer.next().write_der(&cri_der);
            write_algorithm_identifier(writer.next(), req.algorithm.signature_oid());
            writer.next().write_bitvec_bytes(&signature_der, signature_der.len() * 8);
        });
    });

    Ok(CsrOutput {
        algorithm: req.algorithm.label().to_string(),
        subject,
        subject_alt_names: names.iter().map(general_name_summary).collect(),
        private_key_pem,
        public_key_pem,
        csr_pem: pem_wrap("CERTIFICATE REQUEST", &csr_der),
    })
}

fn certification_request_info(
    req: &CsrRequest,
    common_name: &str,
    spki_der: &[u8],
    names: &[GeneralName],
) -> Result<Vec<u8>, String> {
    Ok(yasna::construct_der(|writer| {
        writer.write_sequence(|writer| {
            writer.next().write_u8(0);
            write_subject(writer.next(), req, common_name);
            writer.next().write_der(spki_der);
            writer.next().write_tagged_implicit(Tag::context(0), |writer| {
                writer.write_set(|writer| {
                    if !names.is_empty() {
                        write_extension_request_attribute(writer.next(), names);
                    }
                });
            });
        });
    }))
}

fn write_subject(writer: yasna::DERWriter<'_>, req: &CsrRequest, common_name: &str) {
    let fields = [
        (&[2, 5, 4, 3][..], common_name),
        (&[2, 5, 4, 10][..], req.organization.trim()),
        (&[2, 5, 4, 11][..], req.organizational_unit.trim()),
        (&[2, 5, 4, 6][..], req.country.trim()),
        (&[2, 5, 4, 8][..], req.state.trim()),
        (&[2, 5, 4, 7][..], req.locality.trim()),
    ];
    writer.write_sequence(|writer| {
        for (oid, value) in fields {
            if value.is_empty() {
                continue;
            }
            writer.next().write_set(|writer| {
                writer.next().write_sequence(|writer| {
                    writer.next().write_oid(&ObjectIdentifier::from_slice(oid));
                    if oid == &[2, 5, 4, 6] {
                        writer.next().write_printable_string(value);
                    } else {
                        writer.next().write_utf8_string(value);
                    }
                });
            });
        }
    });
}

fn write_extension_request_attribute(writer: yasna::DERWriter<'_>, names: &[GeneralName]) {
    // pkcs-9-at-extensionRequest: 1.2.840.113549.1.9.14
    writer.write_sequence(|writer| {
        writer
            .next()
            .write_oid(&ObjectIdentifier::from_slice(&[1, 2, 840, 113549, 1, 9, 14]));
        writer.next().write_set(|writer| {
            writer.next().write_der(&extensions_der(names));
        });
    });
}

fn extensions_der(names: &[GeneralName]) -> Vec<u8> {
    yasna::construct_der(|writer| {
        writer.write_sequence(|writer| {
            writer.next().write_sequence(|writer| {
                // subjectAltName: 2.5.29.17. Critical defaults to false, so omit BOOLEAN.
                writer
                    .next()
                    .write_oid(&ObjectIdentifier::from_slice(&[2, 5, 29, 17]));
                writer.next().write_bytes(&general_names_der(names));
            });
        });
    })
}

fn general_names_der(names: &[GeneralName]) -> Vec<u8> {
    yasna::construct_der(|writer| {
        writer.write_sequence(|writer| {
            for name in names {
                match name {
                    GeneralName::Email(value) => writer
                        .next()
                        .write_tagged_implicit(Tag::context(1), |writer| writer.write_ia5_string(value)),
                    GeneralName::Dns(value) => writer
                        .next()
                        .write_tagged_implicit(Tag::context(2), |writer| writer.write_ia5_string(value)),
                    GeneralName::Uri(value) => writer
                        .next()
                        .write_tagged_implicit(Tag::context(6), |writer| writer.write_ia5_string(value)),
                    GeneralName::Ip(ip) => writer
                        .next()
                        .write_tagged_implicit(Tag::context(7), |writer| match ip {
                            IpAddr::V4(v4) => writer.write_bytes(&v4.octets()),
                            IpAddr::V6(v6) => writer.write_bytes(&v6.octets()),
                        }),
                }
            }
        });
    })
}

fn write_algorithm_identifier(writer: yasna::DERWriter<'_>, oid: &[u64]) {
    writer.write_sequence(|writer| {
        writer.next().write_oid(&ObjectIdentifier::from_slice(oid));
    });
}

fn parse_sans(req: &CsrRequest, common_name: &str) -> Result<Vec<GeneralName>, String> {
    let mut out = Vec::new();
    add_ia5_sans(&mut out, "DNS", &req.san_dns, SanKind::Dns)?;
    add_ip_sans(&mut out, &req.san_ips)?;
    add_ia5_sans(&mut out, "email", &req.san_emails, SanKind::Email)?;
    add_ia5_sans(&mut out, "URI", &req.san_uris, SanKind::Uri)?;
    if out.is_empty() && looks_like_dns_name(common_name) {
        validate_ia5(common_name, "common_name as DNS SAN")?;
        out.push(GeneralName::Dns(common_name.to_string()));
    }
    Ok(out)
}

#[derive(Clone, Copy)]
enum SanKind {
    Dns,
    Email,
    Uri,
}

fn add_ia5_sans(
    out: &mut Vec<GeneralName>,
    label: &str,
    raw: &str,
    kind: SanKind,
) -> Result<(), String> {
    for item in split_list(raw) {
        let value = strip_optional_prefix(&item, label);
        validate_ia5(&value, label)?;
        match kind {
            SanKind::Dns => out.push(GeneralName::Dns(value)),
            SanKind::Email => out.push(GeneralName::Email(value)),
            SanKind::Uri => out.push(GeneralName::Uri(value)),
        }
    }
    Ok(())
}

fn add_ip_sans(out: &mut Vec<GeneralName>, raw: &str) -> Result<(), String> {
    for item in split_list(raw) {
        let value = strip_optional_prefix(&item, "IP");
        let ip: IpAddr = value
            .parse()
            .map_err(|_| format!("invalid IP SAN '{value}'; expected IPv4 or IPv6 address"))?;
        out.push(GeneralName::Ip(ip));
    }
    Ok(())
}

fn split_list(raw: &str) -> Vec<String> {
    raw.split(|c| c == ',' || c == '\n' || c == ';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn strip_optional_prefix(value: &str, label: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let wanted = format!("{}:", label.to_ascii_lowercase());
    if lower.starts_with(&wanted) {
        value[wanted.len()..].trim().to_string()
    } else {
        value.trim().to_string()
    }
}

fn validate_ia5(value: &str, field: &str) -> Result<(), String> {
    if value.is_ascii() {
        Ok(())
    } else {
        Err(format!(
            "{field} value '{value}' must contain only valid IA5/ASCII characters"
        ))
    }
}

fn clean_required(value: &str, field: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!("{field} is required"))
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_country(country: &str) -> Result<(), String> {
    let country = country.trim();
    if country.is_empty() {
        return Ok(());
    }
    if country.len() == 2 && country.chars().all(|c| c.is_ascii_alphabetic()) {
        Ok(())
    } else {
        Err("country must be a two-letter ISO code such as US or GB".to_string())
    }
}

fn looks_like_dns_name(value: &str) -> bool {
    value.contains('.') && !value.contains('@') && value.parse::<IpAddr>().is_err()
}

fn subject_summary(common_name: &str, req: &CsrRequest) -> String {
    let mut parts = vec![format!("CN={common_name}")];
    for (label, value) in [
        ("O", req.organization.as_str()),
        ("OU", req.organizational_unit.as_str()),
        ("C", req.country.as_str()),
        ("ST", req.state.as_str()),
        ("L", req.locality.as_str()),
    ] {
        let value = value.trim();
        if !value.is_empty() {
            parts.push(format!("{label}={value}"));
        }
    }
    parts.join(", ")
}

fn general_name_summary(name: &GeneralName) -> String {
    match name {
        GeneralName::Dns(value) => format!("DNS:{value}"),
        GeneralName::Ip(value) => format!("IP:{value}"),
        GeneralName::Email(value) => format!("email:{value}"),
        GeneralName::Uri(value) => format!("URI:{value}"),
    }
}

fn pem_wrap(label: &str, der: &[u8]) -> String {
    let encoded = STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for chunk in encoded.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).expect("base64 is utf8"));
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_req() -> CsrRequest {
        CsrRequest {
            algorithm: Algorithm::P256,
            common_name: "example.com".to_string(),
            organization: "Example Org".to_string(),
            organizational_unit: "Platform".to_string(),
            country: "US".to_string(),
            state: "CA".to_string(),
            locality: "San Francisco".to_string(),
            san_dns: "example.com,www.example.com".to_string(),
            san_ips: "192.0.2.10".to_string(),
            san_emails: "admin@example.com".to_string(),
            san_uris: "https://example.com/device/1".to_string(),
        }
    }

    #[test]
    fn generates_parseable_csr_and_key_shape() {
        let out = generate(base_req()).unwrap();
        assert_eq!(out.algorithm, "p256");
        assert!(out
            .private_key_pem
            .starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(out.public_key_pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert!(out
            .csr_pem
            .starts_with("-----BEGIN CERTIFICATE REQUEST-----"));
        assert!(out.subject.contains("CN=example.com"));
        assert!(out
            .subject_alt_names
            .contains(&"DNS:www.example.com".to_string()));
        assert!(out
            .subject_alt_names
            .contains(&"IP:192.0.2.10".to_string()));
        assert!(out.csr_pem.contains("-----END CERTIFICATE REQUEST-----"));
    }

    #[test]
    fn p384_generates() {
        let mut req = base_req();
        req.algorithm = Algorithm::P384;
        let out = generate(req).unwrap();
        assert_eq!(out.algorithm, "p384");
        assert!(out.csr_pem.contains("CERTIFICATE REQUEST"));
    }

    #[test]
    fn rejects_bad_country() {
        let mut req = base_req();
        req.country = "United States".to_string();
        let err = generate(req).unwrap_err();
        assert!(err.contains("two-letter ISO"), "{err}");
    }

    #[test]
    fn rejects_bad_ip_san() {
        let mut req = base_req();
        req.san_ips = "not-an-ip".to_string();
        let err = generate(req).unwrap_err();
        assert!(err.contains("invalid IP SAN"), "{err}");
    }
}
