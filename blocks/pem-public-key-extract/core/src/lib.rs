//! pem-public-key-extract core — derive the PUBLIC key (PEM SubjectPublicKeyInfo,
//! the `-----BEGIN PUBLIC KEY-----` SPKI form) from a PRIVATE key.
//!
//! Pure Rust, no wafer/wasm-bindgen deps. Supports RSA, EC (NIST P-256 / P-384)
//! and Ed25519 private keys, supplied either as PEM text (PKCS#8
//! `-----BEGIN PRIVATE KEY-----`, traditional PKCS#1 `-----BEGIN RSA PRIVATE
//! KEY-----`, or SEC1 `-----BEGIN EC PRIVATE KEY-----`) or as raw DER bytes
//! given in hex or base64. The public key is always emitted as a PKCS#8/SPKI
//! `-----BEGIN PUBLIC KEY-----` PEM block, the universally-accepted form.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
use ed25519_dalek::pkcs8::{DecodePrivateKey, EncodePublicKey};
use ed25519_dalek::SigningKey as Ed25519SigningKey;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::RsaPrivateKey;

/// Which key algorithm the input is. `Auto` detects from the PEM label first
/// and otherwise tries each algorithm in turn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyType {
    Auto,
    Rsa,
    Ec,
    Ed25519,
}

/// How raw DER binary input is represented as text (DER cannot be printed raw).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DerFormat {
    Hex,
    Base64,
}

pub fn parse_key_type(s: &str) -> Result<KeyType, String> {
    match s.trim().to_ascii_lowercase().replace(['_', '-'], "").as_str() {
        "auto" | "" => Ok(KeyType::Auto),
        "rsa" => Ok(KeyType::Rsa),
        "ec" | "ecdsa" | "p256" | "p384" => Ok(KeyType::Ec),
        "ed25519" | "eddsa" => Ok(KeyType::Ed25519),
        other => Err(format!(
            "unknown key_type '{other}'. Use 'auto', 'rsa', 'ec' or 'ed25519'."
        )),
    }
}

pub fn parse_der_format(s: &str) -> Result<DerFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "hex" | "" => Ok(DerFormat::Hex),
        "base64" | "b64" => Ok(DerFormat::Base64),
        other => Err(format!("unknown der_format '{other}'. Use 'hex' or 'base64'.")),
    }
}

/// Parse a hex string, ignoring whitespace, `0x` prefix and `:`/`-` separators.
fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != ':' && *c != '-')
        .collect();
    let cleaned = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(&cleaned);
    if cleaned.is_empty() {
        return Err("no hex bytes found in input".into());
    }
    if cleaned.len() % 2 != 0 {
        return Err("hex input has an odd number of digits".into());
    }
    let bytes = cleaned.as_bytes();
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char)
            .to_digit(16)
            .ok_or_else(|| format!("invalid hex digit '{}'", bytes[i] as char))?;
        let lo = (bytes[i + 1] as char)
            .to_digit(16)
            .ok_or_else(|| format!("invalid hex digit '{}'", bytes[i + 1] as char))?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Ok(out)
}

fn from_base64(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("no base64 data found in input".into());
    }
    B64.decode(cleaned.as_bytes())
        .map_err(|e| format!("invalid base64 DER input: {e}"))
}

/// RSA private key (PKCS#8 PEM or PKCS#1 PEM) → SPKI public-key PEM.
fn rsa_pub_pem_from_pem(pem: &str) -> Result<String, String> {
    let key = RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
        .map_err(|e| format!("not a valid RSA private key: {e}"))?;
    key.to_public_key()
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| format!("failed to encode RSA public key: {e}"))
}

fn rsa_pub_pem_from_der(der: &[u8]) -> Result<String, String> {
    let key = RsaPrivateKey::from_pkcs8_der(der)
        .or_else(|_| RsaPrivateKey::from_pkcs1_der(der))
        .map_err(|e| format!("not a valid RSA private key: {e}"))?;
    key.to_public_key()
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| format!("failed to encode RSA public key: {e}"))
}

/// EC private key (PKCS#8 or SEC1 PEM) → SPKI public-key PEM. Tries P-256 then P-384.
fn ec_pub_pem_from_pem(pem: &str) -> Result<String, String> {
    if let Ok(sk) =
        p256::SecretKey::from_pkcs8_pem(pem).or_else(|_| p256::SecretKey::from_sec1_pem(pem))
    {
        return sk
            .public_key()
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| format!("failed to encode P-256 public key: {e}"));
    }
    if let Ok(sk) =
        p384::SecretKey::from_pkcs8_pem(pem).or_else(|_| p384::SecretKey::from_sec1_pem(pem))
    {
        return sk
            .public_key()
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| format!("failed to encode P-384 public key: {e}"));
    }
    Err("not a valid EC private key (tried P-256 and P-384; only these NIST curves are supported)".into())
}

fn ec_pub_pem_from_der(der: &[u8]) -> Result<String, String> {
    if let Ok(sk) =
        p256::SecretKey::from_pkcs8_der(der).or_else(|_| p256::SecretKey::from_sec1_der(der))
    {
        return sk
            .public_key()
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| format!("failed to encode P-256 public key: {e}"));
    }
    if let Ok(sk) =
        p384::SecretKey::from_pkcs8_der(der).or_else(|_| p384::SecretKey::from_sec1_der(der))
    {
        return sk
            .public_key()
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| format!("failed to encode P-384 public key: {e}"));
    }
    Err("not a valid EC private key (tried P-256 and P-384; only these NIST curves are supported)".into())
}

/// Ed25519 private key (PKCS#8 PEM) → SPKI public-key PEM.
fn ed25519_pub_pem_from_pem(pem: &str) -> Result<String, String> {
    let sk = Ed25519SigningKey::from_pkcs8_pem(pem)
        .map_err(|e| format!("not a valid Ed25519 private key: {e}"))?;
    sk.verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| format!("failed to encode Ed25519 public key: {e}"))
}

fn ed25519_pub_pem_from_der(der: &[u8]) -> Result<String, String> {
    let sk = Ed25519SigningKey::from_pkcs8_der(der)
        .map_err(|e| format!("not a valid Ed25519 private key: {e}"))?;
    sk.verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| format!("failed to encode Ed25519 public key: {e}"))
}

/// Sniff a PEM label to guess the key algorithm. Returns None for the generic
/// PKCS#8 `PRIVATE KEY` label (which is algorithm-agnostic).
fn key_type_from_pem_label(pem: &str) -> Option<KeyType> {
    let upper = pem.to_uppercase();
    if upper.contains("RSA PRIVATE KEY") {
        Some(KeyType::Rsa)
    } else if upper.contains("EC PRIVATE KEY") {
        Some(KeyType::Ec)
    } else {
        None
    }
}

/// Try every algorithm on a PEM input, returning the first success.
fn try_all_pem(pem: &str) -> Result<String, String> {
    if let Ok(p) = rsa_pub_pem_from_pem(pem) {
        return Ok(p);
    }
    if let Ok(p) = ec_pub_pem_from_pem(pem) {
        return Ok(p);
    }
    if let Ok(p) = ed25519_pub_pem_from_pem(pem) {
        return Ok(p);
    }
    Err("could not parse the input as an RSA, EC (P-256/P-384) or Ed25519 private key. Check that it is a PRIVATE key in PKCS#8, PKCS#1 or SEC1 PEM form.".into())
}

fn try_all_der(der: &[u8]) -> Result<String, String> {
    if let Ok(p) = rsa_pub_pem_from_der(der) {
        return Ok(p);
    }
    if let Ok(p) = ec_pub_pem_from_der(der) {
        return Ok(p);
    }
    if let Ok(p) = ed25519_pub_pem_from_der(der) {
        return Ok(p);
    }
    Err("could not parse the DER input as an RSA, EC (P-256/P-384) or Ed25519 private key. Check that it is the DER of a PRIVATE key (PKCS#8/PKCS#1/SEC1).".into())
}

/// Top-level: derive the public key (SPKI PEM) from a private key.
///
/// - `input` is the private key: PEM text, or DER bytes as hex/base64.
/// - `key_type` selects the algorithm or `Auto` to detect/try.
/// - `der_format` interprets non-PEM (binary) input.
pub fn run(input: &str, key_type: KeyType, der_format: DerFormat) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("no input — paste a PRIVATE key (PEM, or DER as hex/base64)".into());
    }

    let is_pem = trimmed.contains("-----BEGIN");

    if is_pem {
        if !trimmed.to_uppercase().contains("PRIVATE KEY") {
            return Err(
                "input is a PEM block but not a PRIVATE key (no '...PRIVATE KEY' label). This tool derives the public key FROM a private key.".into(),
            );
        }
        let resolved = if key_type == KeyType::Auto {
            key_type_from_pem_label(trimmed).unwrap_or(KeyType::Auto)
        } else {
            key_type
        };
        return match resolved {
            KeyType::Rsa => rsa_pub_pem_from_pem(trimmed),
            KeyType::Ec => ec_pub_pem_from_pem(trimmed),
            KeyType::Ed25519 => ed25519_pub_pem_from_pem(trimmed),
            KeyType::Auto => try_all_pem(trimmed),
        };
    }

    // Non-PEM → treat as DER bytes in hex/base64.
    let der = match der_format {
        DerFormat::Hex => from_hex(trimmed)?,
        DerFormat::Base64 => from_base64(trimmed)?,
    };
    if der.is_empty() {
        return Err("DER input decoded to zero bytes".into());
    }
    match key_type {
        KeyType::Rsa => rsa_pub_pem_from_der(&der),
        KeyType::Ec => ec_pub_pem_from_der(&der),
        KeyType::Ed25519 => ed25519_pub_pem_from_der(&der),
        KeyType::Auto => try_all_der(&der),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs8::EncodePrivateKey;

    fn rsa_priv_pem() -> String {
        let mut rng = rand::thread_rng();
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string()
    }

    fn p256_priv_pem() -> String {
        use p256::pkcs8::EncodePrivateKey;
        let sk = p256::SecretKey::random(&mut rand::thread_rng());
        sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string()
    }

    fn ed25519_priv_pem() -> String {
        use ed25519_dalek::pkcs8::EncodePrivateKey;
        let sk = Ed25519SigningKey::generate(&mut rand::rngs::OsRng);
        sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string()
    }

    #[test]
    fn rsa_pem_extracts_spki_public_key() {
        let pem = rsa_priv_pem();
        let out = run(&pem, KeyType::Auto, DerFormat::Hex).unwrap();
        assert!(out.starts_with("-----BEGIN PUBLIC KEY-----"), "got: {out}");
        assert!(out.contains("-----END PUBLIC KEY-----"));
        let out2 = run(&pem, KeyType::Rsa, DerFormat::Hex).unwrap();
        assert_eq!(out, out2);
    }

    #[test]
    fn ec_p256_pem_extracts_public_key() {
        let pem = p256_priv_pem();
        let out = run(&pem, KeyType::Auto, DerFormat::Hex).unwrap();
        assert!(out.starts_with("-----BEGIN PUBLIC KEY-----"), "got: {out}");
        let out2 = run(&pem, KeyType::Ec, DerFormat::Hex).unwrap();
        assert_eq!(out, out2);
    }

    #[test]
    fn ed25519_pem_extracts_public_key() {
        let pem = ed25519_priv_pem();
        let out = run(&pem, KeyType::Auto, DerFormat::Hex).unwrap();
        assert!(out.starts_with("-----BEGIN PUBLIC KEY-----"), "got: {out}");
        let out2 = run(&pem, KeyType::Ed25519, DerFormat::Hex).unwrap();
        assert_eq!(out, out2);
    }

    #[test]
    fn der_hex_roundtrips() {
        use ed25519_dalek::pkcs8::EncodePrivateKey;
        let sk = Ed25519SigningKey::generate(&mut rand::rngs::OsRng);
        let der = sk.to_pkcs8_der().unwrap();
        let hex: String = der.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
        let out = run(&hex, KeyType::Auto, DerFormat::Hex).unwrap();
        assert!(out.starts_with("-----BEGIN PUBLIC KEY-----"));
        let pem = sk.to_pkcs8_pem(LineEnding::LF).unwrap();
        let from_pem = run(&pem, KeyType::Ed25519, DerFormat::Hex).unwrap();
        assert_eq!(out, from_pem);
    }

    #[test]
    fn der_base64_works() {
        use ed25519_dalek::pkcs8::EncodePrivateKey;
        let sk = Ed25519SigningKey::generate(&mut rand::rngs::OsRng);
        let der = sk.to_pkcs8_der().unwrap();
        let b64 = B64.encode(der.as_bytes());
        let out = run(&b64, KeyType::Auto, DerFormat::Base64).unwrap();
        assert!(out.starts_with("-----BEGIN PUBLIC KEY-----"));
    }

    #[test]
    fn rejects_public_key_input() {
        let pem = "-----BEGIN PUBLIC KEY-----\nAAAA\n-----END PUBLIC KEY-----";
        let err = run(pem, KeyType::Auto, DerFormat::Hex).unwrap_err();
        assert!(err.to_lowercase().contains("private"), "got: {err}");
    }

    #[test]
    fn rejects_empty() {
        assert!(run("   ", KeyType::Auto, DerFormat::Hex).is_err());
    }

    #[test]
    fn rejects_garbage_pem() {
        let pem = "-----BEGIN PRIVATE KEY-----\nbm90YWtleQ==\n-----END PRIVATE KEY-----";
        assert!(run(pem, KeyType::Auto, DerFormat::Hex).is_err());
    }

    #[test]
    fn wrong_type_hint_fails_cleanly() {
        let pem = ed25519_priv_pem();
        let err = run(&pem, KeyType::Rsa, DerFormat::Hex).unwrap_err();
        assert!(err.contains("RSA"));
    }

    #[test]
    fn parse_helpers() {
        assert_eq!(parse_key_type("RSA").unwrap(), KeyType::Rsa);
        assert_eq!(parse_key_type("p384").unwrap(), KeyType::Ec);
        assert_eq!(parse_key_type("ed25519").unwrap(), KeyType::Ed25519);
        assert_eq!(parse_key_type("").unwrap(), KeyType::Auto);
        assert!(parse_key_type("dsa").is_err());
        assert_eq!(parse_der_format("base64").unwrap(), DerFormat::Base64);
        assert_eq!(parse_der_format("hex").unwrap(), DerFormat::Hex);
        assert!(parse_der_format("oct").is_err());
    }

    #[test]
    fn label_sniff() {
        assert_eq!(
            key_type_from_pem_label("-----BEGIN RSA PRIVATE KEY-----"),
            Some(KeyType::Rsa)
        );
        assert_eq!(
            key_type_from_pem_label("-----BEGIN EC PRIVATE KEY-----"),
            Some(KeyType::Ec)
        );
        assert_eq!(key_type_from_pem_label("-----BEGIN PRIVATE KEY-----"), None);
    }
}
