//! gizza-ai/ed25519-sign-verify core — sign a message with an Ed25519 private
//! key and verify a signature with the matching public key. Pure-Rust
//! `ed25519-dalek`; no wafer/wasm-bindgen deps, shared by the chat block, the
//! CLI, and the web page.
//!
//! Ed25519 (EdDSA over Curve25519, RFC 8032) signatures are **deterministic** —
//! the same key + message always yields the same 64-byte signature, with no RNG
//! and no repeated-nonce risk. Signing therefore needs no getrandom, so the tool
//! runs on every backend.
//!
//! Keys are accepted in the encodings real tooling emits:
//!   * private — PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`), or a raw 32-byte
//!     seed / 64-byte `seed||public` keypair as hex or base64;
//!   * public  — SPKI PEM (`-----BEGIN PUBLIC KEY-----`), or a raw 32-byte point
//!     as hex or base64.
//! Signatures are the raw 64 bytes as hex or base64 (both are auto-detected on
//! input; signing emits both).

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use ed25519_dalek::pkcs8::{DecodePrivateKey, DecodePublicKey};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::Serialize;

/// Which operation to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Sign,
    Verify,
}

impl Operation {
    pub fn parse(s: &str) -> Result<Operation, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sign" | "" => Ok(Operation::Sign),
            "verify" => Ok(Operation::Verify),
            other => Err(format!("unknown operation '{other}' (use sign or verify)")),
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Operation::Sign => "sign",
            Operation::Verify => "verify",
        }
    }
}

/// How the `message` field is encoded before it is signed/verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgEncoding {
    Utf8,
    Hex,
    Base64,
}

impl MsgEncoding {
    pub fn parse(s: &str) -> Result<MsgEncoding, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "utf8" | "text" | "" => Ok(MsgEncoding::Utf8),
            "hex" | "base16" => Ok(MsgEncoding::Hex),
            "base64" | "b64" => Ok(MsgEncoding::Base64),
            other => Err(format!("unknown message encoding '{other}' (use utf8, hex, or base64)")),
        }
    }
    /// Decode the message field into the raw bytes that are signed/verified.
    pub fn decode(&self, message: &str) -> Result<Vec<u8>, String> {
        match self {
            MsgEncoding::Utf8 => Ok(message.as_bytes().to_vec()),
            MsgEncoding::Hex => {
                let t: String = message.chars().filter(|c| !c.is_whitespace()).collect();
                hex::decode(&t).map_err(|e| format!("message is not valid hex: {e}"))
            }
            MsgEncoding::Base64 => {
                decode_base64(message).ok_or_else(|| "message is not valid base64".to_string())
            }
        }
    }
}

/// Result of a sign or verify run. Only the fields relevant to the operation are
/// serialized (the rest are omitted), so `sign` yields the signature + derived
/// public key and `verify` yields `valid`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    pub operation: String,
    /// verify: whether the signature is valid for this message + public key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid: Option<bool>,
    /// sign: the 64-byte signature, hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_hex: Option<String>,
    /// sign: the 64-byte signature, base64.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_base64: Option<String>,
    /// sign: signature length in bytes (always 64 for Ed25519).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_bytes: Option<usize>,
    /// sign: public key derived from the private key, hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key_hex: Option<String>,
    /// sign: public key derived from the private key, base64.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key_base64: Option<String>,
}

fn decode_base64(s: &str) -> Option<Vec<u8>> {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    for eng in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(b) = eng.decode(&t) {
            return Some(b);
        }
    }
    None
}

/// Auto-detect a raw byte string (a key or signature) as hex or base64, requiring
/// the decoded length to be one of `expected`. Whitespace is ignored, so
/// space-separated hex works.
fn decode_raw(s: &str, expected: &[usize], what: &str) -> Result<Vec<u8>, String> {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if t.is_empty() {
        return Err(format!("no {what} provided"));
    }
    if t.len() % 2 == 0 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(b) = hex::decode(&t) {
            if expected.contains(&b.len()) {
                return Ok(b);
            }
        }
    }
    if let Some(b) = decode_base64(&t) {
        if expected.contains(&b.len()) {
            return Ok(b);
        }
    }
    let want = expected
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(" or ");
    Err(format!(
        "{what} must be {want} raw bytes as hex or base64 (or the matching PEM block)"
    ))
}

/// Parse the private key used for signing: PKCS#8 PEM, or a raw 32-byte seed /
/// 64-byte `seed||public` keypair as hex or base64.
fn parse_signing_key(key: &str) -> Result<SigningKey, String> {
    if key.contains("-----BEGIN") {
        if !key.contains("PRIVATE KEY") {
            return Err(
                "signing needs a private key — paste a '-----BEGIN PRIVATE KEY-----' (PKCS#8) block, not a public key".into(),
            );
        }
        return SigningKey::from_pkcs8_pem(key)
            .map_err(|e| format!("invalid Ed25519 private key PEM: {e}"));
    }
    let bytes = decode_raw(key, &[32, 64], "private key")?;
    match bytes.len() {
        32 => {
            let seed: [u8; 32] = bytes.try_into().unwrap();
            Ok(SigningKey::from_bytes(&seed))
        }
        64 => {
            let kp: [u8; 64] = bytes.try_into().unwrap();
            SigningKey::from_keypair_bytes(&kp)
                .map_err(|e| format!("invalid 64-byte Ed25519 keypair (seed||public mismatch): {e}"))
        }
        _ => unreachable!(),
    }
}

/// Parse the public key used for verifying: SPKI PEM, or a raw 32-byte point as
/// hex or base64.
fn parse_verifying_key(key: &str) -> Result<VerifyingKey, String> {
    if key.contains("-----BEGIN") {
        if !key.contains("PUBLIC KEY") {
            return Err(
                "verifying needs a public key — paste a '-----BEGIN PUBLIC KEY-----' (SPKI) block, not a private key".into(),
            );
        }
        return VerifyingKey::from_public_key_pem(key)
            .map_err(|e| format!("invalid Ed25519 public key PEM: {e}"));
    }
    let bytes = decode_raw(key, &[32], "public key")?;
    let point: [u8; 32] = bytes.try_into().unwrap();
    VerifyingKey::from_bytes(&point).map_err(|e| format!("invalid Ed25519 public key: {e}"))
}

/// Sign `message` (already decoded to raw bytes) with a private `key`.
pub fn sign(message: &[u8], key: &str) -> Result<Output, String> {
    let sk = parse_signing_key(key)?;
    let sig = sk.sign(message);
    let sig_bytes = sig.to_bytes();
    let vk = sk.verifying_key().to_bytes();
    Ok(Output {
        operation: "sign".into(),
        valid: None,
        signature_hex: Some(hex::encode(sig_bytes)),
        signature_base64: Some(STANDARD.encode(sig_bytes)),
        signature_bytes: Some(sig_bytes.len()),
        public_key_hex: Some(hex::encode(vk)),
        public_key_base64: Some(STANDARD.encode(vk)),
    })
}

/// Verify that `signature` is a valid Ed25519 signature over `message` under the
/// public `key`. Returns `valid=false` (not an error) when the signature simply
/// doesn't match; errors only on malformed keys/signatures.
pub fn verify(message: &[u8], key: &str, signature: &str) -> Result<Output, String> {
    let vk = parse_verifying_key(key)?;
    let sig_bytes = decode_raw(signature, &[64], "signature")?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|e| format!("invalid signature: {e}"))?;
    let valid = vk.verify(message, &sig).is_ok();
    Ok(Output {
        operation: "verify".into(),
        valid: Some(valid),
        signature_hex: None,
        signature_base64: None,
        signature_bytes: None,
        public_key_hex: None,
        public_key_base64: None,
    })
}

/// Entry point shared by every surface: decode the message and dispatch.
pub fn process(
    op: Operation,
    message: &str,
    encoding: MsgEncoding,
    key: &str,
    signature: &str,
) -> Result<Output, String> {
    let msg = encoding.decode(message)?;
    match op {
        Operation::Sign => sign(&msg, key),
        Operation::Verify => {
            if signature.trim().is_empty() {
                return Err("verifying needs a signature (hex or base64) to check".into());
            }
            verify(&msg, key, signature)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Deterministic Ed25519 test vector (RFC 8032, TEST 2: 1-byte message 0x72).
    const SEED_HEX: &str = "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb";
    const PUB_HEX: &str = "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c";
    const SIG_HEX: &str = "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da\
085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00";

    #[test]
    fn signs_rfc8032_vector() {
        // message is the single byte 0x72, given as hex.
        let out = process(Operation::Sign, "72", MsgEncoding::Hex, SEED_HEX, "").unwrap();
        assert_eq!(out.signature_hex.as_deref(), Some(SIG_HEX));
        assert_eq!(out.public_key_hex.as_deref(), Some(PUB_HEX));
        assert_eq!(out.signature_bytes, Some(64));
    }

    #[test]
    fn signing_is_deterministic() {
        let a = sign(b"hello", SEED_HEX).unwrap();
        let b = sign(b"hello", SEED_HEX).unwrap();
        assert_eq!(a.signature_hex, b.signature_hex);
    }

    #[test]
    fn verifies_good_signature() {
        let out = process(Operation::Verify, "72", MsgEncoding::Hex, PUB_HEX, SIG_HEX).unwrap();
        assert_eq!(out.valid, Some(true));
    }

    #[test]
    fn rejects_wrong_message() {
        // Valid signature, but over the wrong message → valid=false, not an error.
        let out = process(Operation::Verify, "73", MsgEncoding::Hex, PUB_HEX, SIG_HEX).unwrap();
        assert_eq!(out.valid, Some(false));
    }

    #[test]
    fn base64_signature_and_key_accepted() {
        let sig_b64 = STANDARD.encode(hex::decode(SIG_HEX).unwrap());
        let pub_b64 = STANDARD.encode(hex::decode(PUB_HEX).unwrap());
        let out = process(Operation::Verify, "72", MsgEncoding::Hex, &pub_b64, &sig_b64).unwrap();
        assert_eq!(out.valid, Some(true));
    }

    #[test]
    fn round_trip_utf8_message() {
        let s = sign(b"gizza", SEED_HEX).unwrap();
        let sig = s.signature_hex.unwrap();
        let v = verify(b"gizza", PUB_HEX, &sig).unwrap();
        assert_eq!(v.valid, Some(true));
    }

    #[test]
    fn pem_round_trip() {
        // A PKCS#8 PEM for SEED_HEX (built via ed25519-dalek), signs then verifies.
        let seed: [u8; 32] = hex::decode(SEED_HEX).unwrap().try_into().unwrap();
        let sk = SigningKey::from_bytes(&seed);
        use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
        use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
        let priv_pem = sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let pub_pem = sk.verifying_key().to_public_key_pem(LineEnding::LF).unwrap();
        let s = process(Operation::Sign, "hi", MsgEncoding::Utf8, &priv_pem, "").unwrap();
        let sig = s.signature_hex.unwrap();
        let v = process(Operation::Verify, "hi", MsgEncoding::Utf8, &pub_pem, &sig).unwrap();
        assert_eq!(v.valid, Some(true));
    }

    #[test]
    fn public_key_pem_for_signing_is_rejected() {
        let err =
            sign(b"x", "-----BEGIN PUBLIC KEY-----\nAAAA\n-----END PUBLIC KEY-----").unwrap_err();
        assert!(err.contains("private key"), "got: {err}");
    }

    #[test]
    fn bad_key_length_errors() {
        let err = sign(b"x", "00ff").unwrap_err();
        assert!(err.contains("32 or 64"), "got: {err}");
    }

    #[test]
    fn verify_without_signature_errors() {
        let err = process(Operation::Verify, "x", MsgEncoding::Utf8, PUB_HEX, "  ").unwrap_err();
        assert!(err.contains("signature"), "got: {err}");
    }

    #[test]
    fn keypair_64_byte_accepted() {
        let seed = hex::decode(SEED_HEX).unwrap();
        let pk = hex::decode(PUB_HEX).unwrap();
        let mut kp = seed.clone();
        kp.extend_from_slice(&pk);
        let out = sign(b"hello", &hex::encode(&kp)).unwrap();
        let expect = sign(b"hello", SEED_HEX).unwrap();
        assert_eq!(out.signature_hex, expect.signature_hex);
    }
}
