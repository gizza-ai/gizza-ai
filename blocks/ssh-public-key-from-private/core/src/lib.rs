//! ssh-public-key-from-private core — derive the OpenSSH **public key line**
//! (the `id_*.pub` format: `ssh-rsa AAAA... comment`) from a PRIVATE key.
//!
//! This is the offline equivalent of `ssh-keygen -y -f id_key`. It differs from
//! the `pem-public-key-extract` tool, which emits a PEM SubjectPublicKeyInfo
//! block; here the output is the single-line OpenSSH authorized_keys / `.pub`
//! wire format (RFC 4253 ssh-rsa, RFC 5656 ecdsa-sha2-nistp*, RFC 8709
//! ssh-ed25519), base64-encoded with the algorithm prefix and optional comment.
//!
//! Pure Rust, no wafer/wasm-bindgen deps. Supports RSA, EC (NIST P-256 / P-384)
//! and Ed25519 private keys, supplied either as PEM text (PKCS#8
//! `-----BEGIN PRIVATE KEY-----`, PKCS#1 `-----BEGIN RSA PRIVATE KEY-----`, or
//! SEC1 `-----BEGIN EC PRIVATE KEY-----`) or as raw DER bytes in hex / base64.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use ed25519_dalek::pkcs8::DecodePrivateKey as Ed25519Decode;
use ed25519_dalek::SigningKey as Ed25519SigningKey;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
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

// ---------------------------------------------------------------------------
// SSH wire format (RFC 4251 §5): the public-key blob is a sequence of
// length-prefixed fields. A "string" is a 4-byte big-endian length followed by
// the raw bytes; an "mpint" is the two's-complement big-endian integer, with a
// leading 0x00 byte prepended when the most-significant bit would otherwise be
// set (so positive numbers stay positive).
// ---------------------------------------------------------------------------

struct SshWriter {
    buf: Vec<u8>,
}

impl SshWriter {
    fn new() -> Self {
        SshWriter { buf: Vec::new() }
    }

    /// Append a length-prefixed byte string.
    fn put_string(&mut self, data: &[u8]) {
        self.buf
            .extend_from_slice(&(data.len() as u32).to_be_bytes());
        self.buf.extend_from_slice(data);
    }

    /// Append an SSH mpint (big-endian, with a leading 0x00 if the high bit set,
    /// and with leading zero bytes stripped).
    fn put_mpint(&mut self, bytes: &[u8]) {
        // Strip leading zero bytes.
        let mut start = 0;
        while start < bytes.len() && bytes[start] == 0 {
            start += 1;
        }
        let trimmed = &bytes[start..];
        if trimmed.is_empty() {
            // Value is zero → encode as an empty string.
            self.put_string(&[]);
            return;
        }
        if trimmed[0] & 0x80 != 0 {
            // High bit set → prepend 0x00 so it reads as positive.
            let mut v = Vec::with_capacity(trimmed.len() + 1);
            v.push(0);
            v.extend_from_slice(trimmed);
            self.put_string(&v);
        } else {
            self.put_string(trimmed);
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

/// Assemble the final OpenSSH public-key line: `<type> <base64-blob>[ comment]`.
fn ssh_line(key_type: &str, blob: &[u8], comment: &str) -> String {
    let b64 = B64.encode(blob);
    let comment = comment.trim();
    if comment.is_empty() {
        format!("{key_type} {b64}")
    } else {
        format!("{key_type} {b64} {comment}")
    }
}

// --- per-algorithm blob builders ------------------------------------------

fn ssh_rsa_line(key: &RsaPrivateKey, comment: &str) -> String {
    let e = key.e().to_bytes_be();
    let n = key.n().to_bytes_be();
    let mut w = SshWriter::new();
    w.put_string(b"ssh-rsa");
    w.put_mpint(&e);
    w.put_mpint(&n);
    ssh_line("ssh-rsa", &w.into_bytes(), comment)
}

fn ssh_ed25519_line(sk: &Ed25519SigningKey, comment: &str) -> String {
    let pk = sk.verifying_key();
    let pk_bytes = pk.to_bytes(); // 32 bytes
    let mut w = SshWriter::new();
    w.put_string(b"ssh-ed25519");
    w.put_string(&pk_bytes);
    ssh_line("ssh-ed25519", &w.into_bytes(), comment)
}

fn ssh_ecdsa_p256_line(sk: &p256::SecretKey, comment: &str) -> String {
    use p256::elliptic_curve::sec1::ToEncodedPoint;
    let point = sk.public_key().to_encoded_point(false); // uncompressed 0x04|X|Y
    let mut w = SshWriter::new();
    w.put_string(b"ecdsa-sha2-nistp256");
    w.put_string(b"nistp256");
    w.put_string(point.as_bytes());
    ssh_line("ecdsa-sha2-nistp256", &w.into_bytes(), comment)
}

fn ssh_ecdsa_p384_line(sk: &p384::SecretKey, comment: &str) -> String {
    use p384::elliptic_curve::sec1::ToEncodedPoint;
    let point = sk.public_key().to_encoded_point(false);
    let mut w = SshWriter::new();
    w.put_string(b"ecdsa-sha2-nistp384");
    w.put_string(b"nistp384");
    w.put_string(point.as_bytes());
    ssh_line("ecdsa-sha2-nistp384", &w.into_bytes(), comment)
}

// --- parsing entry points --------------------------------------------------

fn rsa_from_pem(pem: &str) -> Result<RsaPrivateKey, String> {
    RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
        .map_err(|e| format!("not a valid RSA private key: {e}"))
}
fn rsa_from_der(der: &[u8]) -> Result<RsaPrivateKey, String> {
    RsaPrivateKey::from_pkcs8_der(der)
        .or_else(|_| RsaPrivateKey::from_pkcs1_der(der))
        .map_err(|e| format!("not a valid RSA private key: {e}"))
}

fn ec_line_from_pem(pem: &str, comment: &str) -> Result<String, String> {
    if let Ok(sk) =
        p256::SecretKey::from_pkcs8_pem(pem).or_else(|_| p256::SecretKey::from_sec1_pem(pem))
    {
        return Ok(ssh_ecdsa_p256_line(&sk, comment));
    }
    if let Ok(sk) =
        p384::SecretKey::from_pkcs8_pem(pem).or_else(|_| p384::SecretKey::from_sec1_pem(pem))
    {
        return Ok(ssh_ecdsa_p384_line(&sk, comment));
    }
    Err("not a valid EC private key (tried P-256 and P-384; only these NIST curves are supported)".into())
}
fn ec_line_from_der(der: &[u8], comment: &str) -> Result<String, String> {
    if let Ok(sk) =
        p256::SecretKey::from_pkcs8_der(der).or_else(|_| p256::SecretKey::from_sec1_der(der))
    {
        return Ok(ssh_ecdsa_p256_line(&sk, comment));
    }
    if let Ok(sk) =
        p384::SecretKey::from_pkcs8_der(der).or_else(|_| p384::SecretKey::from_sec1_der(der))
    {
        return Ok(ssh_ecdsa_p384_line(&sk, comment));
    }
    Err("not a valid EC private key (tried P-256 and P-384; only these NIST curves are supported)".into())
}

fn ed25519_from_pem(pem: &str) -> Result<Ed25519SigningKey, String> {
    Ed25519SigningKey::from_pkcs8_pem(pem)
        .map_err(|e| format!("not a valid Ed25519 private key: {e}"))
}
fn ed25519_from_der(der: &[u8]) -> Result<Ed25519SigningKey, String> {
    Ed25519SigningKey::from_pkcs8_der(der)
        .map_err(|e| format!("not a valid Ed25519 private key: {e}"))
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

fn try_all_pem(pem: &str, comment: &str) -> Result<String, String> {
    if let Ok(k) = rsa_from_pem(pem) {
        return Ok(ssh_rsa_line(&k, comment));
    }
    if let Ok(l) = ec_line_from_pem(pem, comment) {
        return Ok(l);
    }
    if let Ok(k) = ed25519_from_pem(pem) {
        return Ok(ssh_ed25519_line(&k, comment));
    }
    Err("could not parse the input as an RSA, EC (P-256/P-384) or Ed25519 private key. Check that it is a PRIVATE key in PKCS#8, PKCS#1 or SEC1 PEM form.".into())
}

fn try_all_der(der: &[u8], comment: &str) -> Result<String, String> {
    if let Ok(k) = rsa_from_der(der) {
        return Ok(ssh_rsa_line(&k, comment));
    }
    if let Ok(l) = ec_line_from_der(der, comment) {
        return Ok(l);
    }
    if let Ok(k) = ed25519_from_der(der) {
        return Ok(ssh_ed25519_line(&k, comment));
    }
    Err("could not parse the DER input as an RSA, EC (P-256/P-384) or Ed25519 private key. Check that it is the DER of a PRIVATE key (PKCS#8/PKCS#1/SEC1).".into())
}

/// Top-level: derive the OpenSSH public-key line from a private key.
///
/// - `input` is the private key: PEM text, or DER bytes as hex/base64.
/// - `key_type` selects the algorithm or `Auto` to detect/try.
/// - `der_format` interprets non-PEM (binary) input.
/// - `comment` is an optional trailing comment (e.g. `user@host`).
pub fn run_with(
    input: &str,
    key_type: KeyType,
    der_format: DerFormat,
    comment: &str,
) -> Result<String, String> {
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
        if trimmed.to_uppercase().contains("OPENSSH PRIVATE KEY") {
            return Err(
                "OpenSSH-format private keys (-----BEGIN OPENSSH PRIVATE KEY-----) are not supported. Convert it to PEM first with: ssh-keygen -p -m PEM -f id_key".into(),
            );
        }
        let resolved = if key_type == KeyType::Auto {
            key_type_from_pem_label(trimmed).unwrap_or(KeyType::Auto)
        } else {
            key_type
        };
        return match resolved {
            KeyType::Rsa => rsa_from_pem(trimmed).map(|k| ssh_rsa_line(&k, comment)),
            KeyType::Ec => ec_line_from_pem(trimmed, comment),
            KeyType::Ed25519 => ed25519_from_pem(trimmed).map(|k| ssh_ed25519_line(&k, comment)),
            KeyType::Auto => try_all_pem(trimmed, comment),
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
        KeyType::Rsa => rsa_from_der(&der).map(|k| ssh_rsa_line(&k, comment)),
        KeyType::Ec => ec_line_from_der(&der, comment),
        KeyType::Ed25519 => ed25519_from_der(&der).map(|k| ssh_ed25519_line(&k, comment)),
        KeyType::Auto => try_all_der(&der, comment),
    }
}

/// Convenience entry: auto-detect, hex DER, no comment.
pub fn run(input: &str) -> Result<String, String> {
    run_with(input, KeyType::Auto, DerFormat::Hex, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rsa_priv_pem() -> String {
        use rsa::pkcs8::EncodePrivateKey;
        use rsa::pkcs8::LineEnding;
        let mut rng = rand::thread_rng();
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string()
    }

    fn p256_priv_pem() -> String {
        use p256::pkcs8::EncodePrivateKey;
        use p256::pkcs8::LineEnding;
        let sk = p256::SecretKey::random(&mut rand::thread_rng());
        sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string()
    }

    fn p384_priv_pem() -> String {
        use p384::pkcs8::EncodePrivateKey;
        use p384::pkcs8::LineEnding;
        let sk = p384::SecretKey::random(&mut rand::thread_rng());
        sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string()
    }

    fn ed25519_priv_pem() -> String {
        use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
        use ed25519_dalek::pkcs8::EncodePrivateKey;
        let sk = Ed25519SigningKey::generate(&mut rand::rngs::OsRng);
        sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string()
    }

    /// Decode an OpenSSH key line and check the algorithm prefix and that the
    /// base64 blob's first field equals the prefix (the wire format invariant).
    fn check_blob_prefix(line: &str, expected_type: &str) {
        let mut parts = line.split(' ');
        let kt = parts.next().unwrap();
        assert_eq!(kt, expected_type, "key type prefix");
        let blob = B64.decode(parts.next().unwrap()).expect("valid base64 blob");
        // First field is a length-prefixed string == the key type.
        let len = u32::from_be_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
        let inner = std::str::from_utf8(&blob[4..4 + len]).unwrap();
        assert_eq!(inner, expected_type, "embedded key type");
    }

    #[test]
    fn rsa_pem_yields_ssh_rsa() {
        let pem = rsa_priv_pem();
        let out = run(&pem).unwrap();
        check_blob_prefix(&out, "ssh-rsa");
        // explicit key_type matches auto
        let out2 = run_with(&pem, KeyType::Rsa, DerFormat::Hex, "").unwrap();
        assert_eq!(out, out2);
    }

    #[test]
    fn ed25519_pem_yields_ssh_ed25519() {
        let pem = ed25519_priv_pem();
        let out = run(&pem).unwrap();
        check_blob_prefix(&out, "ssh-ed25519");
        // ed25519 blob: 4+11 (type) + 4+32 (key) = 51 bytes
        let blob = B64.decode(out.split(' ').nth(1).unwrap()).unwrap();
        assert_eq!(blob.len(), 4 + 11 + 4 + 32);
    }

    #[test]
    fn p256_pem_yields_ecdsa_nistp256() {
        let pem = p256_priv_pem();
        let out = run(&pem).unwrap();
        check_blob_prefix(&out, "ecdsa-sha2-nistp256");
        assert!(out.contains(' '));
    }

    #[test]
    fn p384_pem_yields_ecdsa_nistp384() {
        let pem = p384_priv_pem();
        let out = run(&pem).unwrap();
        check_blob_prefix(&out, "ecdsa-sha2-nistp384");
    }

    #[test]
    fn comment_is_appended() {
        let pem = ed25519_priv_pem();
        let out = run_with(&pem, KeyType::Auto, DerFormat::Hex, "user@host").unwrap();
        assert!(out.ends_with(" user@host"), "got: {out}");
        // three space-separated fields
        assert_eq!(out.split(' ').count(), 3);
    }

    #[test]
    fn der_base64_input_works() {
        use ed25519_dalek::pkcs8::EncodePrivateKey;
        let sk = Ed25519SigningKey::generate(&mut rand::rngs::OsRng);
        let der = sk.to_pkcs8_der().unwrap();
        let b64 = B64.encode(der.as_bytes());
        let out = run_with(&b64, KeyType::Ed25519, DerFormat::Base64, "").unwrap();
        check_blob_prefix(&out, "ssh-ed25519");
    }

    #[test]
    fn empty_input_errors() {
        assert!(run("   ").is_err());
    }

    #[test]
    fn public_key_input_errors() {
        let err = run("-----BEGIN PUBLIC KEY-----\nAAAA\n-----END PUBLIC KEY-----").unwrap_err();
        assert!(err.to_lowercase().contains("private key"), "got: {err}");
    }

    #[test]
    fn openssh_format_is_rejected_with_hint() {
        let err =
            run("-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----")
                .unwrap_err();
        assert!(err.to_lowercase().contains("openssh"), "got: {err}");
    }

    #[test]
    fn garbage_errors() {
        assert!(run("not a key at all").is_err());
    }

    #[test]
    fn parse_key_type_and_der_format() {
        assert_eq!(parse_key_type("auto").unwrap(), KeyType::Auto);
        assert_eq!(parse_key_type("ED25519").unwrap(), KeyType::Ed25519);
        assert_eq!(parse_key_type("p256").unwrap(), KeyType::Ec);
        assert!(parse_key_type("bogus").is_err());
        assert_eq!(parse_der_format("base64").unwrap(), DerFormat::Base64);
        assert!(parse_der_format("bogus").is_err());
    }
}
