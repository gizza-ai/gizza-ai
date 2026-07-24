//! gizza-ai/totp-secret-setup core — generate a fresh random TOTP secret with a
//! CSPRNG, base32-encode it (RFC 4648, no padding), and assemble the standard
//! `otpauth://` enrollment URI understood by every authenticator app. Pure-Rust
//! (`getrandom` + `base32`, URI assembled by the shared `otpauth-uri` core) so it
//! runs on ALL backends incl. the chat Service Worker. The secret is generated
//! locally and never sent anywhere. Randomness via getrandom (WASI `random_get`
//! on wasm32-wasip1 — instantiates in the wafer runtime; the page's
//! wasm32-unknown-unknown build uses getrandom's `js` backend).

use gizza_ai_otpauth_uri_core::{build, Algo, OtpAuth, OtpType};
use serde::Deserialize;

/// A freshly generated TOTP enrollment.
pub struct Setup {
    /// The generated shared secret, base32-encoded (RFC 4648, no padding).
    pub secret: String,
    /// The `otpauth://` enrollment URI carrying the secret + parameters.
    pub uri: String,
    /// Secret strength in bits (secret_bytes × 8).
    pub bits: usize,
    /// Algorithm label (SHA1 / SHA256 / SHA512) for display.
    pub algorithm: String,
    pub digits: u32,
    pub period: u64,
}

fn algo_label(a: Algo) -> String {
    match a {
        Algo::Sha1 => "SHA1",
        Algo::Sha256 => "SHA256",
        Algo::Sha512 => "SHA512",
    }
    .to_string()
}

/// Draw `secret_bytes` cryptographically random bytes and base32-encode them
/// (RFC 4648, no padding — the form authenticator apps expect).
fn random_secret(secret_bytes: usize) -> Result<String, String> {
    let mut buf = vec![0u8; secret_bytes];
    getrandom::getrandom(&mut buf).map_err(|e| format!("RNG error: {e}"))?;
    Ok(base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &buf))
}

/// Generate a fresh TOTP secret and its `otpauth://` enrollment URI.
///
/// `secret_bytes` is the raw entropy length in bytes (10–64; default 20 = 160
/// bits, the RFC 4226 recommendation for SHA1). `issuer` is optional but
/// recommended; `account` is required (it names the entry in the app).
/// `algorithm` is one of sha1/sha256/sha512. The secret is generated locally with
/// a CSPRNG and never leaves the device.
pub fn generate(
    issuer: &str,
    account: &str,
    secret_bytes: usize,
    digits: u32,
    period: u64,
    algorithm: &str,
) -> Result<Setup, String> {
    if !(10..=64).contains(&secret_bytes) {
        return Err("secret_bytes must be between 10 and 64".into());
    }
    let algo = Algo::parse(algorithm)?;
    let secret = random_secret(secret_bytes)?;
    // `build` validates digits (6–8), period (≥1), and the account/issuer, and
    // percent-encodes the label — reuse it so the URI matches otpauth-uri exactly.
    let uri = build(&OtpAuth {
        otp_type: OtpType::Totp,
        issuer: issuer.to_string(),
        account: account.to_string(),
        secret: secret.clone(),
        algorithm: algo,
        digits,
        period,
        counter: 0,
    })?;
    Ok(Setup {
        secret,
        uri,
        bits: secret_bytes * 8,
        algorithm: algo_label(algo),
        digits,
        period,
    })
}

#[derive(Deserialize)]
struct WebArgs {
    #[serde(default)]
    issuer: String,
    account: String,
    #[serde(default = "default_secret_bytes")]
    secret_bytes: u32,
    #[serde(default = "default_digits")]
    digits: u32,
    #[serde(default = "default_period")]
    period: u64,
    #[serde(default = "default_algorithm")]
    algorithm: String,
}

fn default_secret_bytes() -> u32 { 20 }
fn default_digits() -> u32 { 6 }
fn default_period() -> u64 { 30 }
fn default_algorithm() -> String { "sha1".to_string() }

/// Browser/page entry point: parse JSON form input, generate a fresh secret, and
/// return a copy-friendly text report.
pub fn run(input: &str) -> Result<String, String> {
    let args: WebArgs = serde_json::from_str(input).map_err(|e| format!("invalid JSON input: {e}"))?;
    let setup = generate(
        &args.issuer,
        &args.account,
        args.secret_bytes as usize,
        args.digits,
        args.period,
        &args.algorithm,
    )?;
    Ok(format!(
        "Secret: {secret}\nBits: {bits}\nAlgorithm: {algorithm}\nDigits: {digits}\nPeriod: {period} seconds\nURI: {uri}",
        secret = setup.secret,
        bits = setup.bits,
        algorithm = setup.algorithm,
        digits = setup.digits,
        period = setup.period,
        uri = setup.uri,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_valid_secret_and_uri() {
        let s = generate("GitHub", "alice@example.com", 20, 6, 30, "sha1").unwrap();
        // 20 bytes = 160 bits → exactly 32 base32 chars, no padding.
        assert_eq!(s.secret.len(), 32);
        assert!(s.secret.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7')));
        assert_eq!(s.bits, 160);
        assert_eq!(s.algorithm, "SHA1");
        assert!(s.uri.starts_with("otpauth://totp/GitHub:alice%40example.com?"));
        assert!(s.uri.contains(&format!("secret={}", s.secret)));
        assert!(s.uri.contains("issuer=GitHub"));
        assert!(s.uri.contains("algorithm=SHA1"));
        assert!(s.uri.contains("digits=6"));
        assert!(s.uri.contains("period=30"));
    }

    #[test]
    fn secret_length_scales_with_bytes() {
        let s = generate("X", "a", 32, 8, 30, "sha256").unwrap();
        assert_eq!(s.bits, 256);
        assert_eq!(s.algorithm, "SHA256");
        // 32 bytes = 256 bits → ceil(256/5) = 52 base32 chars (no padding).
        assert_eq!(s.secret.len(), 52);
        assert!(s.uri.contains("digits=8"));
    }

    #[test]
    fn two_calls_produce_different_secrets() {
        let a = generate("X", "a", 20, 6, 30, "sha1").unwrap();
        let b = generate("X", "a", 20, 6, 30, "sha1").unwrap();
        assert_ne!(a.secret, b.secret, "each run must be independently random");
    }

    #[test]
    fn rejects_out_of_range_secret_bytes() {
        assert!(generate("X", "a", 4, 6, 30, "sha1").is_err());
        assert!(generate("X", "a", 128, 6, 30, "sha1").is_err());
    }

    #[test]
    fn rejects_missing_account() {
        assert!(generate("GitHub", "   ", 20, 6, 30, "sha1").is_err());
    }

    #[test]
    fn rejects_unknown_algorithm() {
        assert!(generate("X", "a", 20, 6, 30, "md5").is_err());
    }

    #[test]
    fn web_run_formats_copyable_secret_and_uri() {
        let out = run(r#"{"issuer":"Example","account":"alice@example.com","secret_bytes":10,"digits":8,"algorithm":"sha256"}"#).unwrap();
        assert!(out.contains("Secret: "));
        assert!(out.contains("Bits: 80"));
        assert!(out.contains("Algorithm: SHA256"));
        assert!(out.contains("Digits: 8"));
        assert!(out.contains("URI: otpauth://totp/Example:alice%40example.com?"));
    }
}
