//! Certificate-chain validation core.
//!
//! Accepts one or more PEM encoded X.509 certificates in leaf-to-root order and
//! checks the local properties a browser/debugging workflow usually needs before
//! attempting network trust: issuer/subject ordering, certificate signatures, CA
//! flags on issuers, and current validity windows. No network fetching or system
//! trust store is used.

use rsa::pkcs1v15::{Signature as RsaSignature, VerifyingKey};
use rsa::signature::Verifier;
use rsa::{BigUint, RsaPublicKey};
use sha2::{Sha256, Sha384, Sha512};
use x509_parser::certificate::X509Certificate;
use x509_parser::pem::Pem;
use x509_parser::public_key::PublicKey;
use x509_parser::time::ASN1Time;

#[derive(Debug, Clone)]
struct CertInfo {
    subject: String,
    issuer: String,
    serial: String,
    not_before: String,
    not_after: String,
    is_ca: bool,
    valid_now: bool,
}

/// Validate a PEM certificate chain in leaf -> intermediate -> root order.
pub fn run(input: &str) -> Result<String, String> {
    let now = current_unix_timestamp()?;
    run_at(input, now)
}

/// Validate a PEM certificate chain at a supplied Unix timestamp.
pub fn run_at(input: &str, now_unix: i64) -> Result<String, String> {
    let pems = parse_pems(input)?;
    let certs = parse_certs(&pems)?;
    validate_chain(&certs, now_unix)?;

    let infos = certs
        .iter()
        .map(|cert| cert_info(cert, now_unix))
        .collect::<Vec<_>>();
    Ok(render_report(&infos, certs.len()))
}

#[cfg(not(target_arch = "wasm32"))]
fn current_unix_timestamp() -> Result<i64, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .map_err(|e| format!("system clock is before Unix epoch: {e}"))
}

#[cfg(target_arch = "wasm32")]
fn current_unix_timestamp() -> Result<i64, String> {
    Err("current time is unavailable in this WebAssembly environment".into())
}

fn parse_pems(input: &str) -> Result<Vec<Pem>, String> {
    if input.trim().is_empty() {
        return Err("paste one or more PEM certificates".into());
    }

    let mut pems = Vec::new();
    for pem in Pem::iter_from_buffer(input.as_bytes()) {
        let pem = pem.map_err(|e| format!("invalid PEM block: {e}"))?;
        if pem.label == "CERTIFICATE" {
            pems.push(pem);
        }
    }

    if pems.is_empty() {
        return Err("no CERTIFICATE PEM blocks found".into());
    }
    Ok(pems)
}

fn parse_certs<'a>(pems: &'a [Pem]) -> Result<Vec<X509Certificate<'a>>, String> {
    pems.iter()
        .enumerate()
        .map(|(idx, pem)| {
            pem.parse_x509()
                .map_err(|e| format!("certificate #{} is not valid X.509 DER: {e}", idx + 1))
        })
        .collect()
}

fn validate_chain(certs: &[X509Certificate<'_>], now_unix: i64) -> Result<(), String> {
    let now =
        ASN1Time::from_timestamp(now_unix).map_err(|e| format!("invalid current time: {e}"))?;

    for (idx, cert) in certs.iter().enumerate() {
        if !cert.validity().is_valid_at(now) {
            return Err(format!(
                "certificate #{} is not currently valid: not before {}, not after {}",
                idx + 1,
                cert.validity().not_before,
                cert.validity().not_after
            ));
        }
    }

    for (idx, pair) in certs.windows(2).enumerate() {
        let child = &pair[0];
        let issuer = &pair[1];
        if child.issuer() != issuer.subject() {
            return Err(format!(
                "issuer/subject mismatch between certificate #{} and #{}: child issuer is '{}', next subject is '{}'",
                idx + 1,
                idx + 2,
                child.issuer(),
                issuer.subject()
            ));
        }
        if !issuer.is_ca() {
            return Err(format!(
                "certificate #{} signs certificate #{} but does not have basicConstraints CA:true",
                idx + 2,
                idx + 1
            ));
        }
        verify_cert_signature(child, issuer)
            .map_err(|e| format!("signature check failed for certificate #{}: {e}", idx + 1))?;
    }

    if let Some(root) = certs.last() {
        if root.issuer() == root.subject() {
            verify_cert_signature(root, root)
                .map_err(|e| format!("self-signed root signature check failed: {e}"))?;
        }
    }

    Ok(())
}

fn verify_cert_signature(
    cert: &X509Certificate<'_>,
    issuer: &X509Certificate<'_>,
) -> Result<(), String> {
    let oid = cert.signature_algorithm.algorithm.to_id_string();
    let signature = RsaSignature::try_from(cert.signature_value.data.as_ref())
        .map_err(|e| format!("invalid RSA signature bytes: {e}"))?;
    let public_key = match issuer.public_key().parsed().map_err(|e| e.to_string())? {
        PublicKey::RSA(key) => RsaPublicKey::new(
            BigUint::from_bytes_be(strip_leading_zero(key.modulus)),
            BigUint::from_bytes_be(strip_leading_zero(key.exponent)),
        )
        .map_err(|e| format!("invalid issuer RSA public key: {e}"))?,
        _ => return Err("unsupported issuer public key type; RSA signatures are supported".into()),
    };

    match oid.as_str() {
        "1.2.840.113549.1.1.11" => VerifyingKey::<Sha256>::new(public_key)
            .verify(cert.tbs_certificate.as_ref(), &signature),
        "1.2.840.113549.1.1.12" => VerifyingKey::<Sha384>::new(public_key)
            .verify(cert.tbs_certificate.as_ref(), &signature),
        "1.2.840.113549.1.1.13" => VerifyingKey::<Sha512>::new(public_key)
            .verify(cert.tbs_certificate.as_ref(), &signature),
        other => return Err(format!("unsupported signature algorithm OID {other}")),
    }
    .map_err(|e| e.to_string())
}

fn strip_leading_zero(bytes: &[u8]) -> &[u8] {
    if bytes.len() > 1 && bytes[0] == 0 {
        &bytes[1..]
    } else {
        bytes
    }
}

fn cert_info(cert: &X509Certificate<'_>, now_unix: i64) -> CertInfo {
    let now = ASN1Time::from_timestamp(now_unix).expect("validated timestamp");
    CertInfo {
        subject: cert.subject().to_string(),
        issuer: cert.issuer().to_string(),
        serial: cert.raw_serial_as_string(),
        not_before: cert.validity().not_before.to_string(),
        not_after: cert.validity().not_after.to_string(),
        is_ca: cert.is_ca(),
        valid_now: cert.validity().is_valid_at(now),
    }
}

fn render_report(infos: &[CertInfo], count: usize) -> String {
    let mut out = String::new();
    out.push_str("Certificate chain: VALID\n");
    out.push_str(&format!("Certificates checked: {count}\n"));
    out.push_str("Ordering: leaf-to-root issuer/subject chain matches\n");
    out.push_str("Signatures: verified against the next certificate public key\n");
    out.push_str("Validity: every certificate is currently within its notBefore/notAfter window\n");
    out.push_str("Trust: not checked against browser or OS root stores\n\n");

    for (idx, info) in infos.iter().enumerate() {
        let role = if idx == 0 {
            "leaf"
        } else if idx + 1 == infos.len() {
            "root"
        } else {
            "intermediate"
        };
        out.push_str(&format!(
            "#{} ({role})\n  Subject: {}\n  Issuer: {}\n  Serial: {}\n  Valid: {} to {}\n  CA: {}\n  Valid now: {}\n",
            idx + 1,
            info.subject,
            info.issuer,
            info.serial,
            info.not_before,
            info.not_after,
            if info.is_ca { "yes" } else { "no" },
            if info.valid_now { "yes" } else { "no" }
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHAIN_RSA: &str = include_str!("../tests/fixtures/chain-rsa.pem");
    const ROOT_RSA: &str = include_str!("../tests/fixtures/root-rsa.pem");
    const INT_RSA: &str = include_str!("../tests/fixtures/int-rsa.pem");
    const LEAF_RSA: &str = include_str!("../tests/fixtures/leaf-rsa.pem");
    const LEAF_EXPIRED: &str = include_str!("../tests/fixtures/leaf-expired-rsa.pem");

    #[test]
    fn validates_leaf_to_root_chain() {
        let out = run(CHAIN_RSA).unwrap();
        assert!(out.contains("Certificate chain: VALID"));
        assert!(out.contains("Certificates checked: 3"));
        assert!(out.contains("Signatures: verified"));
    }

    #[test]
    fn rejects_wrong_order() {
        let wrong = format!("{ROOT_RSA}\n{INT_RSA}\n{LEAF_RSA}");
        let err = run(&wrong).unwrap_err();
        assert!(err.contains("issuer/subject mismatch"));
    }

    #[test]
    fn rejects_expired_leaf() {
        let expired = format!("{LEAF_EXPIRED}\n{INT_RSA}\n{ROOT_RSA}");
        let err = run(&expired).unwrap_err();
        assert!(err.contains("not currently valid"));
    }

    #[test]
    fn rejects_missing_certificate_blocks() {
        let err = run("hello").unwrap_err();
        assert!(err.contains("no CERTIFICATE PEM blocks"));
    }
}
