//! gizza-ai/ecdsa-secp256k1 core — generate secp256k1 keypairs and sign/verify
//! messages with ECDSA, the signature scheme behind Bitcoin and Ethereum keys.
//! Pure-Rust `k256` (RustCrypto); no wafer/wasm-bindgen deps, shared by the chat
//! block, the CLI, and the web page.
//!
//! * **Signing is deterministic (RFC 6979)** — the nonce is derived from the key
//!   and digest, so the same key + message always yields the same signature, and
//!   no RNG is needed to sign. Signatures are **low-S normalized** (the
//!   Bitcoin/Ethereum canonical form) and include the **recovery id** (and the
//!   Ethereum-style `v = 27 + id`).
//! * **Key formats** — private: raw 32 bytes as hex (`0x` optional) or base64,
//!   or PEM (SEC1 `EC PRIVATE KEY` / PKCS#8 `PRIVATE KEY`); public: SEC1 point,
//!   compressed 33 bytes (`02`/`03`…) or uncompressed 65 bytes (`04`…), as hex
//!   or base64, or SPKI `PUBLIC KEY` PEM.
//! * **Digests** — SHA-256 (default), Keccak-256 (Ethereum), SHA-384, SHA-512,
//!   or `none` (the message *is* the 32-byte digest, e.g. a transaction hash).
//! * **Signature formats** — sign emits compact `r||s` (hex + base64), ASN.1
//!   DER (hex), and the r/s components; verify auto-detects compact or DER.
//!
//! Key generation draws from the OS CSPRNG via `getrandom` (WASI `random_get`
//! under wafer, `crypto.getRandomValues` in the browser build).

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use k256::ecdsa::signature::hazmat::PrehashVerifier;
use k256::ecdsa::{RecoveryId, Signature, SigningKey, VerifyingKey};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::pkcs8::{
    DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding,
};
use k256::{PublicKey, SecretKey};
use serde::Serialize;
use sha2::Digest;

/// Which operation to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Generate,
    Sign,
    Verify,
}

impl Operation {
    pub fn parse(s: &str) -> Result<Operation, String> {
        match s
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', ' '], "")
            .as_str()
        {
            "generate" | "keygen" | "" => Ok(Operation::Generate),
            "sign" => Ok(Operation::Sign),
            "verify" => Ok(Operation::Verify),
            other => Err(format!(
                "unknown operation '{other}' (use generate, sign, or verify)"
            )),
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Operation::Generate => "generate",
            Operation::Sign => "sign",
            Operation::Verify => "verify",
        }
    }
}

/// How the `message` field is decoded into bytes before hashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgEncoding {
    Utf8,
    Hex,
    Base64,
}

impl MsgEncoding {
    pub fn parse(s: &str) -> Result<MsgEncoding, String> {
        match s
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', ' '], "")
            .as_str()
        {
            "utf8" | "text" | "" => Ok(MsgEncoding::Utf8),
            "hex" | "base16" => Ok(MsgEncoding::Hex),
            "base64" | "b64" => Ok(MsgEncoding::Base64),
            other => Err(format!(
                "unknown message encoding '{other}' (use utf8, hex, or base64)"
            )),
        }
    }
    pub fn decode(&self, message: &str) -> Result<Vec<u8>, String> {
        match self {
            MsgEncoding::Utf8 => Ok(message.as_bytes().to_vec()),
            MsgEncoding::Hex => {
                let t: String = message.chars().filter(|c| !c.is_whitespace()).collect();
                let t = t
                    .strip_prefix("0x")
                    .or_else(|| t.strip_prefix("0X"))
                    .unwrap_or(&t);
                hex::decode(t).map_err(|e| format!("message is not valid hex: {e}"))
            }
            MsgEncoding::Base64 => {
                decode_base64(message).ok_or_else(|| "message is not valid base64".to_string())
            }
        }
    }
}

/// The digest applied to the message before ECDSA (ECDSA signs a digest, not the
/// raw message). `None` means the message already *is* the 32-byte digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlg {
    Sha256,
    Keccak256,
    Sha384,
    Sha512,
    None,
}

impl HashAlg {
    pub fn parse(s: &str) -> Result<HashAlg, String> {
        match s
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', ' '], "")
            .as_str()
        {
            "sha256" | "" => Ok(HashAlg::Sha256),
            "keccak256" | "keccak" => Ok(HashAlg::Keccak256),
            "sha384" => Ok(HashAlg::Sha384),
            "sha512" => Ok(HashAlg::Sha512),
            "none" | "prehashed" | "raw" => Ok(HashAlg::None),
            other => Err(format!(
                "unknown hash '{other}' (use sha256, keccak256, sha384, sha512, or none)"
            )),
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            HashAlg::Sha256 => "sha256",
            HashAlg::Keccak256 => "keccak256",
            HashAlg::Sha384 => "sha384",
            HashAlg::Sha512 => "sha512",
            HashAlg::None => "none",
        }
    }
    /// Hash `msg` into the digest that is signed/verified.
    pub fn digest(&self, msg: &[u8]) -> Result<Vec<u8>, String> {
        Ok(match self {
            HashAlg::Sha256 => sha2::Sha256::digest(msg).to_vec(),
            HashAlg::Keccak256 => sha3::Keccak256::digest(msg).to_vec(),
            HashAlg::Sha384 => sha2::Sha384::digest(msg).to_vec(),
            HashAlg::Sha512 => sha2::Sha512::digest(msg).to_vec(),
            HashAlg::None => {
                if msg.len() != 32 {
                    return Err(format!(
                        "with hash=none the message must be the 32-byte digest itself (got {} bytes) — paste the 64-hex-char hash and set message_encoding=hex",
                        msg.len()
                    ));
                }
                msg.to_vec()
            }
        })
    }
}

/// Result of a run. Only the fields relevant to the operation are serialized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    pub operation: String,
    /// sign/verify: which digest was applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// sign/verify: the digest that was signed/checked, hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest_hex: Option<String>,
    /// verify: whether the signature is valid for this message + public key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid: Option<bool>,
    /// verify: how the pasted signature was read (compact or der).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_form: Option<String>,
    /// verify: true when a high-S signature was low-S normalized before checking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_s: Option<bool>,
    /// sign: compact 64-byte r||s signature, hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_compact_hex: Option<String>,
    /// sign: compact 64-byte r||s signature, base64.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_compact_base64: Option<String>,
    /// sign: ASN.1 DER signature, hex (what OpenSSL emits/consumes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_der_hex: Option<String>,
    /// sign: the r component, 32 bytes hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_hex: Option<String>,
    /// sign: the s component (low-S normalized), 32 bytes hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s_hex: Option<String>,
    /// sign: recovery id (0-3) — which candidate public key is the signer's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_id: Option<u8>,
    /// sign: Ethereum-style v = 27 + recovery id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v: Option<u8>,
    /// generate: private key, 32 bytes lower-hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_hex: Option<String>,
    /// generate: private key as PKCS#8 PEM.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_pem: Option<String>,
    /// generate/sign: SEC1 compressed public key (33 bytes), hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key_compressed_hex: Option<String>,
    /// generate/sign: SEC1 uncompressed public key (65 bytes, 04-prefixed), hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key_uncompressed_hex: Option<String>,
    /// generate: public key as SPKI PEM.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key_pem: Option<String>,
}

impl Output {
    fn base(op: Operation) -> Output {
        Output {
            operation: op.name().into(),
            hash: None,
            digest_hex: None,
            valid: None,
            signature_form: None,
            normalized_s: None,
            signature_compact_hex: None,
            signature_compact_base64: None,
            signature_der_hex: None,
            r_hex: None,
            s_hex: None,
            recovery_id: None,
            v: None,
            private_key_hex: None,
            private_key_pem: None,
            public_key_compressed_hex: None,
            public_key_uncompressed_hex: None,
            public_key_pem: None,
        }
    }
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

/// Auto-detect a raw byte string (key or signature) as hex (optional `0x`) or
/// base64, requiring the decoded length to be one of `expected` (empty = any).
fn decode_raw(s: &str, expected: &[usize], what: &str, hint: &str) -> Result<Vec<u8>, String> {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if t.is_empty() {
        return Err(format!("no {what} provided"));
    }
    let h = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(&t);
    if h.len() % 2 == 0 && !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(b) = hex::decode(h) {
            if expected.is_empty() || expected.contains(&b.len()) {
                return Ok(b);
            }
        }
    }
    if let Some(b) = decode_base64(&t) {
        if expected.is_empty() || expected.contains(&b.len()) {
            return Ok(b);
        }
    }
    Err(format!(
        "{what} must be {hint}, as hex (0x optional) or base64"
    ))
}

/// Parse the private key used for signing: PEM (SEC1 `EC PRIVATE KEY` or PKCS#8
/// `PRIVATE KEY`), or the raw 32-byte scalar as hex or base64.
fn parse_signing_key(key: &str) -> Result<SigningKey, String> {
    let t = key.trim();
    if t.is_empty() {
        return Err("signing needs a private key — generate one with operation=generate, or paste yours (32-byte hex, base64, or PEM)".into());
    }
    if t.contains("-----BEGIN") {
        if t.contains("PUBLIC KEY") {
            return Err("signing needs a PRIVATE key — you pasted a public key. Use operation=verify to check a signature with a public key.".into());
        }
        let sk = if t.contains("EC PRIVATE KEY") {
            SecretKey::from_sec1_pem(t).map_err(|e| {
                format!("invalid SEC1 EC private key PEM: {e} (is it a secp256k1 key?)")
            })?
        } else {
            SecretKey::from_pkcs8_pem(t).map_err(|e| {
                format!("invalid PKCS#8 private key PEM: {e} (is it a secp256k1 key?)")
            })?
        };
        return Ok(SigningKey::from(sk));
    }
    let bytes = decode_raw(
        t,
        &[32],
        "private key",
        "the raw 32-byte scalar (64 hex chars)",
    )?;
    SigningKey::from_slice(&bytes).map_err(|e| {
        format!("invalid secp256k1 private key: {e} (must be non-zero and below the curve order)")
    })
}

/// Parse the public key used for verifying: SPKI PEM, or a SEC1 point —
/// compressed 33 bytes or uncompressed 65 bytes — as hex or base64.
fn parse_verifying_key(key: &str) -> Result<VerifyingKey, String> {
    let t = key.trim();
    if t.is_empty() {
        return Err(
            "verifying needs the signer's public key (compressed or uncompressed hex, base64, or PEM)"
                .into(),
        );
    }
    if t.contains("-----BEGIN") {
        if t.contains("PRIVATE KEY") {
            return Err("verifying needs a PUBLIC key — you pasted a private key. Use operation=sign to create a signature with a private key.".into());
        }
        let pk = PublicKey::from_public_key_pem(t)
            .map_err(|e| format!("invalid public key PEM: {e} (is it a secp256k1 SPKI key?)"))?;
        return Ok(VerifyingKey::from(pk));
    }
    let bytes = decode_raw(
        t,
        &[33, 65],
        "public key",
        "a SEC1 point — 33 bytes compressed (02/03…) or 65 bytes uncompressed (04…)",
    )?;
    VerifyingKey::from_sec1_bytes(&bytes)
        .map_err(|e| format!("invalid secp256k1 public key point: {e}"))
}

/// Parse a signature as compact 64-byte `r||s` or ASN.1 DER, hex or base64.
/// Returns the parsed signature and the detected form name.
fn parse_signature(signature: &str) -> Result<(Signature, &'static str), String> {
    let bytes = decode_raw(
        signature,
        &[],
        "signature",
        "compact 64-byte r||s or ASN.1 DER",
    )?;
    if bytes.len() == 64 {
        if let Ok(sig) = Signature::from_slice(&bytes) {
            return Ok((sig, "compact"));
        }
    }
    if bytes.first() == Some(&0x30) {
        return Signature::from_der(&bytes)
            .map(|sig| (sig, "der"))
            .map_err(|e| format!("invalid DER signature: {e}"));
    }
    Err(format!(
        "signature must be the compact 64-byte r||s or an ASN.1 DER blob (got {} bytes)",
        bytes.len()
    ))
}

/// Generate a fresh secp256k1 keypair from the OS CSPRNG.
pub fn generate() -> Result<Output, String> {
    let sk = SecretKey::random(&mut rand::rngs::OsRng);
    let pk = sk.public_key();
    let mut out = Output::base(Operation::Generate);
    out.private_key_hex = Some(hex::encode(sk.to_bytes()));
    out.private_key_pem = Some(
        sk.to_pkcs8_pem(LineEnding::LF)
            .map_err(|e| format!("PEM encoding failed: {e}"))?
            .to_string(),
    );
    fill_public(&mut out, &VerifyingKey::from(pk));
    out.public_key_pem = Some(
        pk.to_public_key_pem(LineEnding::LF)
            .map_err(|e| format!("PEM encoding failed: {e}"))?,
    );
    Ok(out)
}

fn fill_public(out: &mut Output, vk: &VerifyingKey) {
    out.public_key_compressed_hex = Some(hex::encode(vk.to_encoded_point(true).as_bytes()));
    out.public_key_uncompressed_hex = Some(hex::encode(vk.to_encoded_point(false).as_bytes()));
}

/// Sign the digest of `message` with a private `key` (deterministic RFC 6979,
/// low-S normalized, with recovery id).
pub fn sign(msg: &[u8], hash: HashAlg, key: &str) -> Result<Output, String> {
    let sk = parse_signing_key(key)?;
    let digest = hash.digest(msg)?;
    let (sig, recid) = sk
        .sign_prehash_recoverable(&digest)
        .map_err(|e| format!("signing failed: {e}"))?;
    // k256 signs low-S already; normalize defensively and keep the recovery id
    // consistent (normalizing s flips the candidate key's parity bit).
    let (sig, recid) = match sig.normalize_s() {
        Some(norm) => (
            norm,
            RecoveryId::from_byte(recid.to_byte() ^ 1).expect("recovery id stays in range"),
        ),
        None => (sig, recid),
    };
    let compact = sig.to_bytes();
    let mut out = Output::base(Operation::Sign);
    out.hash = Some(hash.name().into());
    out.digest_hex = Some(hex::encode(&digest));
    out.signature_compact_hex = Some(hex::encode(compact));
    out.signature_compact_base64 = Some(STANDARD.encode(compact));
    out.signature_der_hex = Some(hex::encode(sig.to_der().as_bytes()));
    out.r_hex = Some(hex::encode(&compact[..32]));
    out.s_hex = Some(hex::encode(&compact[32..]));
    out.recovery_id = Some(recid.to_byte());
    out.v = Some(27 + recid.to_byte());
    fill_public(&mut out, sk.verifying_key());
    Ok(out)
}

/// Verify `signature` over the digest of `message` under the public `key`.
/// Returns `valid=false` (not an error) when the signature simply doesn't
/// match; errors only on malformed inputs. High-S signatures are low-S
/// normalized before checking (OpenSSL may emit high-S).
pub fn verify(msg: &[u8], hash: HashAlg, key: &str, signature: &str) -> Result<Output, String> {
    let vk = parse_verifying_key(key)?;
    let digest = hash.digest(msg)?;
    let (sig, form) = parse_signature(signature)?;
    let (sig, normalized) = match sig.normalize_s() {
        Some(norm) => (norm, true),
        None => (sig, false),
    };
    let valid = vk.verify_prehash(&digest, &sig).is_ok();
    let mut out = Output::base(Operation::Verify);
    out.hash = Some(hash.name().into());
    out.digest_hex = Some(hex::encode(&digest));
    out.valid = Some(valid);
    out.signature_form = Some(form.into());
    out.normalized_s = Some(normalized);
    Ok(out)
}

/// Entry point shared by every surface: decode the message and dispatch.
pub fn process(
    op: Operation,
    message: &str,
    encoding: MsgEncoding,
    hash: HashAlg,
    key: &str,
    signature: &str,
) -> Result<Output, String> {
    match op {
        Operation::Generate => generate(),
        Operation::Sign | Operation::Verify => {
            if message.trim().is_empty() {
                return Err(format!(
                    "nothing to {} — enter a message (or, for a pre-hashed digest, set hash=none and paste the 32-byte hash as hex)",
                    op.name()
                ));
            }
            let msg = encoding.decode(message)?;
            if op == Operation::Sign {
                sign(&msg, hash, key)
            } else {
                if signature.trim().is_empty() {
                    return Err(
                        "verifying needs a signature (compact r||s or DER, as hex or base64)"
                            .into(),
                    );
                }
                verify(&msg, hash, key, signature)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The widely-published RFC 6979 / secp256k1 known-answer vector:
    /// private key = 1, message "Satoshi Nakamoto", SHA-256.
    const KEY_ONE: &str = "0000000000000000000000000000000000000000000000000000000000000001";
    /// secp256k1 generator point G (the public key for private key 1), compressed.
    const PUB_G_COMPRESSED: &str =
        "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    const PUB_G_UNCOMPRESSED: &str = "0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8";
    const KAT_MSG: &str = "Satoshi Nakamoto";
    const KAT_R: &str = "934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d8";
    const KAT_S: &str = "2442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5";
    /// sha256("Satoshi Nakamoto") — for the hash=none path (checked with sha256sum).
    const KAT_DIGEST: &str = "a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e";

    // PEM forms of private key 1, generated + cross-checked with OpenSSL.
    const SEC1_PEM: &str = "-----BEGIN EC PRIVATE KEY-----\nMHQCAQEEIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABoAcGBSuBBAAK\noUQDQgAEeb5mfvncu6xVoGKVzocLBwKb/NstzijZWfKBWxb4F5hIOtp3JqPEZV2k\n+/wOEQio/Re0SKaFVBmcR9CP+xDUuA==\n-----END EC PRIVATE KEY-----\n";
    const PKCS8_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIGEAgEAMBAGByqGSM49AgEGBSuBBAAKBG0wawIBAQQgAAAAAAAAAAAAAAAAAAAA\nAAAAAAAAAAAAAAAAAAAAAAGhRANCAAR5vmZ++dy7rFWgYpXOhwsHApv82y3OKNlZ\n8oFbFvgXmEg62ncmo8RlXaT7/A4RCKj9F7RIpoVUGZxH0I/7ENS4\n-----END PRIVATE KEY-----\n";
    const SPKI_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEeb5mfvncu6xVoGKVzocLBwKb/NstzijZ\nWfKBWxb4F5hIOtp3JqPEZV2k+/wOEQio/Re0SKaFVBmcR9CP+xDUuA==\n-----END PUBLIC KEY-----\n";

    fn kat_sign() -> Output {
        process(
            Operation::Sign,
            KAT_MSG,
            MsgEncoding::Utf8,
            HashAlg::Sha256,
            KEY_ONE,
            "",
        )
        .unwrap()
    }

    #[test]
    fn signs_rfc6979_kat_exactly() {
        let out = kat_sign();
        assert_eq!(out.r_hex.as_deref(), Some(KAT_R));
        assert_eq!(out.s_hex.as_deref(), Some(KAT_S));
        assert_eq!(
            out.signature_compact_hex.as_deref().unwrap(),
            format!("{KAT_R}{KAT_S}")
        );
        assert_eq!(out.digest_hex.as_deref(), Some(KAT_DIGEST));
        assert_eq!(
            out.public_key_compressed_hex.as_deref(),
            Some(PUB_G_COMPRESSED)
        );
        assert_eq!(
            out.public_key_uncompressed_hex.as_deref(),
            Some(PUB_G_UNCOMPRESSED)
        );
    }

    #[test]
    fn second_rfc6979_kat() {
        // Same key, the Blade Runner message — another published vector.
        let out = process(
            Operation::Sign,
            "All those moments will be lost in time, like tears in rain. Time to die...",
            MsgEncoding::Utf8,
            HashAlg::Sha256,
            KEY_ONE,
            "",
        )
        .unwrap();
        assert_eq!(
            out.r_hex.as_deref(),
            Some("8600dbd41e348fe5c9465ab92d23e3db8b98b873beecd930736488696438cb6b")
        );
        assert_eq!(
            out.s_hex.as_deref(),
            Some("547fe64427496db33bf66019dacbf0039c04199abb0122918601db38a72cfc21")
        );
    }

    #[test]
    fn signing_is_deterministic_and_low_s() {
        let a = kat_sign();
        let b = kat_sign();
        assert_eq!(a.signature_compact_hex, b.signature_compact_hex);
        // low-S: s < n/2 (n/2 starts 0x7fff…), so the first byte is <= 0x7f.
        let s = a.s_hex.unwrap();
        assert!(
            u8::from_str_radix(&s[..2], 16).unwrap() <= 0x7f,
            "s is high: {s}"
        );
    }

    #[test]
    fn hash_none_prehashed_matches_sha256_path() {
        let out = process(
            Operation::Sign,
            KAT_DIGEST,
            MsgEncoding::Hex,
            HashAlg::None,
            KEY_ONE,
            "",
        )
        .unwrap();
        assert_eq!(
            out.signature_compact_hex.as_deref().unwrap(),
            format!("{KAT_R}{KAT_S}")
        );
    }

    #[test]
    fn hash_none_wrong_length_errors() {
        let long = format!("{KAT_DIGEST}00");
        for digest in ["a0dc", long.as_str()] {
            let err = process(
                Operation::Sign,
                digest,
                MsgEncoding::Hex,
                HashAlg::None,
                KEY_ONE,
                "",
            )
            .unwrap_err();
            assert!(err.contains("32-byte digest"), "got: {err}");
        }
    }

    #[test]
    fn keccak256_digest_matches_published_vector() {
        // keccak256("abc") — the published Keccak-256 vector.
        let out = process(
            Operation::Sign,
            "abc",
            MsgEncoding::Utf8,
            HashAlg::Keccak256,
            KEY_ONE,
            "",
        )
        .unwrap();
        assert_eq!(
            out.digest_hex.as_deref(),
            Some("4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45")
        );
    }

    #[test]
    fn verifies_kat_compact_and_der() {
        let signed = kat_sign();
        let compact = signed.signature_compact_hex.unwrap();
        let der = signed.signature_der_hex.unwrap();
        for (sig, form) in [(&compact, "compact"), (&der, "der")] {
            let out = process(
                Operation::Verify,
                KAT_MSG,
                MsgEncoding::Utf8,
                HashAlg::Sha256,
                PUB_G_COMPRESSED,
                sig,
            )
            .unwrap();
            assert_eq!(out.valid, Some(true), "form {form}");
            assert_eq!(out.signature_form.as_deref(), Some(form));
            assert_eq!(out.normalized_s, Some(false));
        }
    }

    #[test]
    fn verify_wrong_message_is_valid_false() {
        let sig = kat_sign().signature_compact_hex.unwrap();
        let out = process(
            Operation::Verify,
            "Satoshi Nakamoto!",
            MsgEncoding::Utf8,
            HashAlg::Sha256,
            PUB_G_UNCOMPRESSED,
            &sig,
        )
        .unwrap();
        assert_eq!(out.valid, Some(false));
    }

    #[test]
    fn verify_normalizes_high_s() {
        // Rebuild the KAT signature with s' = n - s (the high-S twin) — it must
        // still verify, with normalized_s reported.
        let sig = Signature::from_slice(&hex::decode(format!("{KAT_R}{KAT_S}")).unwrap()).unwrap();
        let s = sig.s();
        let high = Signature::from_scalars(*sig.r(), -*s.as_ref()).unwrap();
        let out = process(
            Operation::Verify,
            KAT_MSG,
            MsgEncoding::Utf8,
            HashAlg::Sha256,
            PUB_G_COMPRESSED,
            &hex::encode(high.to_bytes()),
        )
        .unwrap();
        assert_eq!(out.valid, Some(true));
        assert_eq!(out.normalized_s, Some(true));
    }

    #[test]
    fn recovery_id_recovers_signer_key() {
        let sk = parse_signing_key(KEY_ONE).unwrap();
        for (msg, hash) in [
            ("Satoshi Nakamoto", HashAlg::Sha256),
            ("hello ethereum", HashAlg::Keccak256),
            ("long digests too", HashAlg::Sha512),
        ] {
            let out = sign(msg.as_bytes(), hash, KEY_ONE).unwrap();
            let digest = hex::decode(out.digest_hex.unwrap()).unwrap();
            let sig =
                Signature::from_slice(&hex::decode(out.signature_compact_hex.unwrap()).unwrap())
                    .unwrap();
            let recid = RecoveryId::from_byte(out.recovery_id.unwrap()).unwrap();
            let recovered = VerifyingKey::recover_from_prehash(&digest, &sig, recid).unwrap();
            assert_eq!(&recovered, sk.verifying_key(), "msg {msg:?}");
            assert_eq!(out.v, Some(27 + recid.to_byte()));
        }
    }

    #[test]
    fn all_hashes_round_trip() {
        for hash in [
            HashAlg::Sha256,
            HashAlg::Keccak256,
            HashAlg::Sha384,
            HashAlg::Sha512,
        ] {
            let signed = sign(b"round trip", hash, KEY_ONE).unwrap();
            let out = verify(
                b"round trip",
                hash,
                PUB_G_COMPRESSED,
                signed.signature_compact_hex.as_deref().unwrap(),
            )
            .unwrap();
            assert_eq!(out.valid, Some(true), "hash {}", hash.name());
        }
    }

    #[test]
    fn pem_keys_and_alternate_encodings_accepted() {
        let expect = kat_sign().signature_compact_hex.unwrap();
        // SEC1 + PKCS#8 private PEM, 0x-hex, and base64 all sign identically.
        let hex0x = format!("0x{KEY_ONE}");
        let b64_key = STANDARD.encode(hex::decode(KEY_ONE).unwrap());
        for key in [SEC1_PEM, PKCS8_PEM, hex0x.as_str(), b64_key.as_str()] {
            let out = process(
                Operation::Sign,
                KAT_MSG,
                MsgEncoding::Utf8,
                HashAlg::Sha256,
                key,
                "",
            )
            .unwrap();
            assert_eq!(
                out.signature_compact_hex.as_deref(),
                Some(expect.as_str()),
                "key form failed: {key:.40}"
            );
        }
        // SPKI PEM + base64 compact signature verify.
        let sig_b64 = STANDARD.encode(hex::decode(&expect).unwrap());
        let out = process(
            Operation::Verify,
            KAT_MSG,
            MsgEncoding::Utf8,
            HashAlg::Sha256,
            SPKI_PEM,
            &sig_b64,
        )
        .unwrap();
        assert_eq!(out.valid, Some(true));
    }

    #[test]
    fn generate_produces_valid_working_keypair() {
        let a = generate().unwrap();
        let b = generate().unwrap();
        let priv_hex = a.private_key_hex.clone().unwrap();
        assert_eq!(priv_hex.len(), 64);
        assert_ne!(
            a.private_key_hex, b.private_key_hex,
            "two generations must differ"
        );
        let comp = a.public_key_compressed_hex.clone().unwrap();
        assert_eq!(comp.len(), 66);
        assert!(comp.starts_with("02") || comp.starts_with("03"));
        let uncomp = a.public_key_uncompressed_hex.clone().unwrap();
        assert_eq!(uncomp.len(), 130);
        assert!(uncomp.starts_with("04"));
        assert!(a
            .private_key_pem
            .as_deref()
            .unwrap()
            .contains("BEGIN PRIVATE KEY"));
        assert!(a
            .public_key_pem
            .as_deref()
            .unwrap()
            .contains("BEGIN PUBLIC KEY"));
        // The generated pair actually signs + verifies.
        let signed = sign(b"self test", HashAlg::Sha256, &priv_hex).unwrap();
        let out = verify(
            b"self test",
            HashAlg::Sha256,
            &comp,
            signed.signature_compact_hex.as_deref().unwrap(),
        )
        .unwrap();
        assert_eq!(out.valid, Some(true));
        // And the generated PEMs parse back.
        let signed_pem = sign(
            b"self test",
            HashAlg::Sha256,
            a.private_key_pem.as_deref().unwrap(),
        )
        .unwrap();
        assert_eq!(
            signed_pem.signature_compact_hex,
            signed.signature_compact_hex
        );
        let out = verify(
            b"self test",
            HashAlg::Sha256,
            a.public_key_pem.as_deref().unwrap(),
            signed.signature_compact_hex.as_deref().unwrap(),
        )
        .unwrap();
        assert_eq!(out.valid, Some(true));
    }

    #[test]
    fn wrong_key_material_errors() {
        // Public key where a private key is needed.
        let err = sign(b"x", HashAlg::Sha256, SPKI_PEM).unwrap_err();
        assert!(err.contains("PRIVATE"), "got: {err}");
        // Private key where a public key is needed.
        let err = verify(b"x", HashAlg::Sha256, PKCS8_PEM, "00").unwrap_err();
        assert!(err.contains("PUBLIC"), "got: {err}");
        // Zero private key is invalid.
        let err = sign(b"x", HashAlg::Sha256, &"00".repeat(32)).unwrap_err();
        assert!(err.contains("invalid secp256k1 private key"), "got: {err}");
        // Wrong-length raw keys.
        let err = sign(b"x", HashAlg::Sha256, "00ff").unwrap_err();
        assert!(err.contains("32-byte"), "got: {err}");
        let err = verify(b"x", HashAlg::Sha256, "00ff", "00").unwrap_err();
        assert!(err.contains("compressed"), "got: {err}");
        // A P-256 SPKI PEM is not a secp256k1 key.
        let p256_pem = "-----BEGIN PUBLIC KEY-----\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEHRC8N1nCP5uxRz5voMthkKBmvhNU\nZ0MMpBl5U8T1NwJs6/8jQVXYFHqGVUEHF+RNXhkJTHwbLAP450b47DIWaA==\n-----END PUBLIC KEY-----";
        let err = verify(b"x", HashAlg::Sha256, p256_pem, "00").unwrap_err();
        assert!(err.contains("public key PEM"), "got: {err}");
    }

    #[test]
    fn empty_message_and_missing_signature_error() {
        let err = process(
            Operation::Sign,
            "  ",
            MsgEncoding::Utf8,
            HashAlg::Sha256,
            KEY_ONE,
            "",
        )
        .unwrap_err();
        assert!(err.contains("nothing to sign"), "got: {err}");
        let err = process(
            Operation::Verify,
            "x",
            MsgEncoding::Utf8,
            HashAlg::Sha256,
            PUB_G_COMPRESSED,
            " ",
        )
        .unwrap_err();
        assert!(err.contains("needs a signature"), "got: {err}");
    }

    #[test]
    fn message_encodings_decode() {
        // hex + base64 message encodings feed the same bytes → same signature.
        let utf8 = process(
            Operation::Sign,
            "hi",
            MsgEncoding::Utf8,
            HashAlg::Sha256,
            KEY_ONE,
            "",
        )
        .unwrap();
        let hexm = process(
            Operation::Sign,
            "6869",
            MsgEncoding::Hex,
            HashAlg::Sha256,
            KEY_ONE,
            "",
        )
        .unwrap();
        let b64m = process(
            Operation::Sign,
            "aGk=",
            MsgEncoding::Base64,
            HashAlg::Sha256,
            KEY_ONE,
            "",
        )
        .unwrap();
        assert_eq!(utf8.signature_compact_hex, hexm.signature_compact_hex);
        assert_eq!(utf8.signature_compact_hex, b64m.signature_compact_hex);
        let err = process(
            Operation::Sign,
            "zz",
            MsgEncoding::Hex,
            HashAlg::Sha256,
            KEY_ONE,
            "",
        )
        .unwrap_err();
        assert!(err.contains("not valid hex"), "got: {err}");
    }

    #[test]
    fn enum_parsers() {
        assert_eq!(Operation::parse("").unwrap(), Operation::Generate);
        assert_eq!(Operation::parse("Sign").unwrap(), Operation::Sign);
        assert!(Operation::parse("what").is_err());
        assert_eq!(HashAlg::parse("KECCAK-256").unwrap(), HashAlg::Keccak256);
        assert_eq!(HashAlg::parse("").unwrap(), HashAlg::Sha256);
        assert!(HashAlg::parse("md5").is_err());
        assert_eq!(MsgEncoding::parse("text").unwrap(), MsgEncoding::Utf8);
        assert!(MsgEncoding::parse("ebcdic").is_err());
    }
}
