//! gizza-ai/pgp-decrypt core — decrypt an ASCII-armored OpenPGP message.
//! Pure-Rust (rPGP): no wafer/wasm-bindgen deps. Shared by the chat-skill block
//! and the web page.
//!
//! Both OpenPGP encryption shapes are handled, and which one is used is decided
//! from the message's own session-key packets rather than from a mode switch:
//!
//! - **public-key** (PKESK packets, the `gpg --encrypt` / `pgp-encrypt` shape) —
//!   needs the recipient's ASCII-armored private key; `passphrase` unlocks that
//!   key when it is protected.
//! - **password** (SKESK packets, the `gpg --symmetric` shape) — needs only the
//!   password the message was encrypted with, supplied in `passphrase`.
//!
//! A message that carries both kinds of session-key packet is decrypted with the
//! private key when one is supplied, and with the password otherwise.
//!
//! Encrypted messages are frequently *also* signed (`gpg --sign --encrypt`).
//! Signature metadata is always reported, and the signature is actually verified
//! when the signer's public key is supplied.

use base64::Engine as _;
use pgp::composed::{Deserializable, Esk, Message, SignedPublicKey, SignedSecretKey};
use pgp::types::PublicKeyTrait;
use serde::Serialize;

/// Largest armored message accepted, in bytes. The wasm sandbox is 64 MiB and
/// decryption holds the ciphertext, the session-decrypted buffer and the
/// rendered output at once, so the input is bounded well below that.
pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

/// How the decrypted bytes are rendered into the JSON result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// UTF-8 text when the plaintext decodes cleanly, base64 otherwise.
    Auto,
    /// Always UTF-8 text; errors when the plaintext is not valid UTF-8.
    Text,
    /// Standard base64 (with padding).
    Base64,
    /// Lowercase hex.
    Hex,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(OutputFormat::Auto),
            "text" | "utf8" | "utf-8" => Ok(OutputFormat::Text),
            "base64" | "b64" => Ok(OutputFormat::Base64),
            "hex" => Ok(OutputFormat::Hex),
            other => Err(format!(
                "unknown output_format '{other}' (use 'auto', 'text', 'base64' or 'hex')"
            )),
        }
    }
}

/// Signature metadata for an encrypted-and-signed message.
#[derive(Debug, Clone, Serialize)]
pub struct SignatureInfo {
    /// `true`/`false` once the signature has been checked against a supplied
    /// public key; `null` when no `public_key` was given (the signature is
    /// reported but not verified).
    pub valid: Option<bool>,
    /// 16-hex-digit long key ID recorded as the signature's issuer, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_key_id: Option<String>,
    /// Fingerprint of the public (sub)key that actually verified the signature
    /// (only present when `valid` is `true`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_fingerprint: Option<String>,
    /// Primary User ID of the supplied public key (name/email), when given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_user_id: Option<String>,
    /// Signature creation time, RFC 3339 UTC.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_at: Option<String>,
    /// Hash algorithm used by the signature (e.g. `"SHA256"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_algorithm: Option<String>,
    /// Why the signature was not verified, or why it failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Structured decryption result, serialized to JSON for every surface.
#[derive(Debug, Clone, Serialize)]
pub struct DecryptResult {
    /// The decrypted payload, rendered per `output_format`.
    pub plaintext: String,
    /// `public-key` (decrypted with a private key) or `password` (symmetric).
    pub encryption: &'static str,
    /// The format `plaintext` is actually in: `text`, `base64` or `hex`.
    pub output_format: &'static str,
    /// Size of the decrypted payload in bytes (before rendering).
    pub bytes: usize,
    /// `true` when the message's literal packet declares binary data (so the
    /// payload is a file, not text).
    pub binary: bool,
    /// `true` when the plaintext was compressed inside the encryption layer.
    pub compressed: bool,
    /// Long key ID of the private (sub)key that unlocked the session key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decrypted_with_key_id: Option<String>,
    /// Long key IDs the message was encrypted to (empty for `password`, and for
    /// messages encrypted with a hidden/wildcard recipient).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recipient_key_ids: Vec<String>,
    /// Present only when the decrypted message carries a signature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureInfo>,
}

impl DecryptResult {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("DecryptResult serializes")
    }
}

fn fingerprint_hex(k: &impl PublicKeyTrait) -> String {
    k.fingerprint()
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect()
}

/// The signature's hash algorithm as a canonical OpenPGP name (e.g. "SHA256").
fn sig_hash_algorithm(sig: &pgp::packet::Signature) -> Option<String> {
    use pgp::crypto::hash::HashAlgorithm as H;
    let name = match sig.hash_alg() {
        H::None => return None,
        H::MD5 => "MD5",
        H::SHA1 => "SHA1",
        H::RIPEMD160 => "RIPEMD160",
        H::SHA2_256 => "SHA256",
        H::SHA2_384 => "SHA384",
        H::SHA2_512 => "SHA512",
        H::SHA2_224 => "SHA224",
        H::SHA3_256 => "SHA3-256",
        H::SHA3_512 => "SHA3-512",
        other => return Some(format!("{other:?}")),
    };
    Some(name.to_string())
}

fn parse_secret_key(armored: &str) -> Result<SignedSecretKey, String> {
    if !armored.contains("BEGIN PGP PRIVATE KEY BLOCK") {
        if armored.contains("BEGIN PGP PUBLIC KEY BLOCK") {
            return Err(
                "that is a PUBLIC key — decryption needs the matching ASCII-armored '-----BEGIN PGP PRIVATE KEY BLOCK-----' private key"
                    .into(),
            );
        }
        return Err(
            "no PGP private key found — paste an ASCII-armored '-----BEGIN PGP PRIVATE KEY BLOCK-----' key"
                .into(),
        );
    }
    let (key, _) = SignedSecretKey::from_string(armored)
        .map_err(|e| format!("not a valid PGP private key: {e}"))?;
    key.verify()
        .map_err(|e| format!("private key failed self-verification: {e}"))?;
    Ok(key)
}

fn parse_public_key(armored: &str) -> Result<SignedPublicKey, String> {
    if !armored.contains("BEGIN PGP PUBLIC KEY BLOCK") {
        return Err(
            "public_key is not a PGP public key — paste the signer's ASCII-armored '-----BEGIN PGP PUBLIC KEY BLOCK-----' block (or leave it blank to skip signature verification)"
                .into(),
        );
    }
    let (key, _) = SignedPublicKey::from_string(armored)
        .map_err(|e| format!("not a valid PGP public key: {e}"))?;
    key.verify()
        .map_err(|e| format!("public key failed self-verification: {e}"))?;
    Ok(key)
}

/// Turn an rPGP public-key-decryption failure into an actionable message.
fn explain_key_failure(err: pgp::errors::Error, has_passphrase: bool) -> String {
    if matches!(err, pgp::errors::Error::MissingKey) {
        return "the supplied private key is not one of this message's recipients — check you pasted the key the message was encrypted to (its key ID is listed under recipient_key_ids)".into();
    }
    let detail = err.to_string();
    if detail.contains("failed to decrypt session key") {
        return if has_passphrase {
            "could not unlock the private key — the passphrase looks wrong (watch for a trailing space or a smart quote pasted from a document)".into()
        } else {
            "could not unlock the private key — it looks passphrase-protected, so enter its passphrase".into()
        };
    }
    format!("decryption failed: {detail}")
}

/// Turn an rPGP password-decryption failure into an actionable message.
fn explain_password_failure(err: pgp::errors::Error) -> String {
    format!(
        "decryption failed with that password: {err} — check the password used when the message was created"
    )
}

/// Normalize the CRLF line endings OpenPGP stores text-mode literals in.
fn crlf_to_lf(s: &str) -> String {
    s.replace("\r\n", "\n")
}

/// Which session-key packets a message carries, plus the recipients it names.
struct EskSummary {
    public_key: bool,
    password: bool,
    recipient_key_ids: Vec<String>,
}

fn summarize_esk(msg: &Message) -> Result<EskSummary, String> {
    let esk = match msg {
        Message::Encrypted { esk, .. } => esk,
        Message::Signed { .. } => {
            return Err(
                "this PGP message is signed but NOT encrypted — there is nothing to decrypt; use the PGP verify tool to check its signature"
                    .into(),
            )
        }
        Message::Literal(_) | Message::Compressed(_) => {
            return Err(
                "this PGP message is not encrypted (it holds plain literal data) — pgp-decrypt expects a message produced by encrypting to a key or to a password"
                    .into(),
            )
        }
    };

    let mut summary = EskSummary {
        public_key: false,
        password: false,
        recipient_key_ids: Vec::new(),
    };
    for e in esk {
        match e {
            Esk::PublicKeyEncryptedSessionKey(p) => {
                summary.public_key = true;
                if let Ok(id) = p.id() {
                    let hex = format!("{id:X}");
                    // A wildcard recipient is all-zero and names nobody.
                    if hex.chars().any(|c| c != '0') && !summary.recipient_key_ids.contains(&hex) {
                        summary.recipient_key_ids.push(hex);
                    }
                }
            }
            Esk::SymKeyEncryptedSessionKey(_) => summary.password = true,
        }
    }
    if !summary.public_key && !summary.password {
        return Err(
            "this encrypted PGP message carries no session-key packet, so it cannot be decrypted with a key or a password"
                .into(),
        );
    }
    Ok(summary)
}

/// Collect signature metadata and, when a public key is supplied, verify it.
fn inspect_signature(msg: &Message, public_key: &str) -> Result<SignatureInfo, String> {
    let signature = match msg {
        Message::Signed { signature, .. } => signature,
        _ => unreachable!("only called for a signed message"),
    };
    let signer_key_id = signature.issuer().first().map(|kid| format!("{kid:X}"));
    let signed_at = signature.created().map(|dt| dt.to_rfc3339());
    let hash_algorithm = sig_hash_algorithm(signature);

    if public_key.trim().is_empty() {
        return Ok(SignatureInfo {
            valid: None,
            signer_key_id,
            signer_fingerprint: None,
            signer_user_id: None,
            signed_at,
            hash_algorithm,
            note: Some(
                "the message is signed but no public_key was supplied, so the signature was not verified"
                    .into(),
            ),
        });
    }

    let key = parse_public_key(public_key)?;
    let signer_user_id = key
        .details
        .users
        .first()
        .map(|u| u.id.id().to_string())
        .filter(|s| !s.is_empty());

    // PublicKeyTrait is not dyn-compatible, so try the primary key and then each
    // subkey directly (they are distinct concrete types).
    let mut matched: Option<String> = None;
    if msg.verify(&key.primary_key).is_ok() {
        matched = Some(fingerprint_hex(&key.primary_key));
    } else {
        for sub in &key.public_subkeys {
            if msg.verify(&sub.key).is_ok() {
                matched = Some(fingerprint_hex(&sub.key));
                break;
            }
        }
    }

    Ok(match matched {
        Some(fpr) => SignatureInfo {
            valid: Some(true),
            signer_key_id,
            signer_fingerprint: Some(fpr),
            signer_user_id,
            signed_at,
            hash_algorithm,
            note: None,
        },
        None => SignatureInfo {
            valid: Some(false),
            signer_key_id,
            signer_fingerprint: None,
            signer_user_id,
            signed_at,
            hash_algorithm,
            note: Some(
                "the signature does not verify against this public key — the message may have been altered, or it was signed by a different key"
                    .into(),
            ),
        },
    })
}

/// Decrypt an ASCII-armored OpenPGP message.
///
/// - `message`: the `-----BEGIN PGP MESSAGE-----` block to decrypt.
/// - `private_key`: the recipient's ASCII-armored private key. Required for a
///   public-key-encrypted message; ignored for a password-encrypted one.
/// - `passphrase`: unlocks a protected private key, **or** is the password a
///   symmetrically-encrypted message was made with. Empty when neither applies.
/// - `public_key`: the signer's ASCII-armored public key. Optional; supplying it
///   verifies the signature of an encrypted-and-signed message.
/// - `output_format`: how the decrypted bytes are rendered.
pub fn run(
    message: &str,
    private_key: &str,
    passphrase: &str,
    public_key: &str,
    output_format: OutputFormat,
) -> Result<DecryptResult, String> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err(
            "no PGP message supplied — paste an ASCII-armored '-----BEGIN PGP MESSAGE-----' block"
                .into(),
        );
    }
    if message.len() > MAX_MESSAGE_BYTES {
        return Err(format!(
            "the armored message is {} bytes, over the {} byte limit — decrypt large payloads with a desktop OpenPGP client",
            message.len(),
            MAX_MESSAGE_BYTES
        ));
    }
    if !trimmed.contains("BEGIN PGP MESSAGE") {
        if trimmed.contains("BEGIN PGP SIGNED MESSAGE") {
            return Err(
                "that is a clearsigned message, not an encrypted one — its text is already readable; use the PGP verify tool to check its signature"
                    .into(),
            );
        }
        if trimmed.contains("BEGIN PGP PRIVATE KEY BLOCK")
            || trimmed.contains("BEGIN PGP PUBLIC KEY BLOCK")
        {
            return Err(
                "that is a PGP key block, not an encrypted message — paste the '-----BEGIN PGP MESSAGE-----' block into the message field (the PGP key info tool inspects keys)"
                    .into(),
            );
        }
        return Err(
            "no PGP message found — paste the whole ASCII-armored block, from '-----BEGIN PGP MESSAGE-----' through '-----END PGP MESSAGE-----'"
                .into(),
        );
    }

    let (msg, _headers) = Message::from_string(trimmed)
        .map_err(|e| format!("not a valid ASCII-armored PGP message: {e} — check the block was copied whole and its lines were not re-wrapped"))?;

    let esk = summarize_esk(&msg)?;
    let have_key = !private_key.trim().is_empty();
    let have_pw = !passphrase.is_empty();

    let (decrypted, encryption, decrypted_with_key_id) = if esk.public_key && have_key {
        let key = parse_secret_key(private_key)?;
        let pw = passphrase.to_string();
        let (m, ids) = msg
            .decrypt(move || pw.clone(), &[&key])
            .map_err(|e| explain_key_failure(e, have_pw))?;
        let used = ids.first().map(|k| format!("{k:X}"));
        (m, "public-key", used)
    } else if esk.password && have_pw {
        let pw = passphrase.to_string();
        let m = msg
            .decrypt_with_password(move || pw.clone())
            .map_err(explain_password_failure)?;
        (m, "password", None)
    } else if esk.public_key && esk.password {
        return Err(
            "this message can be opened with a private key or with a password — paste the private key, or type the password in the passphrase field"
                .into(),
        );
    } else if esk.public_key {
        return Err(format!(
            "this message is encrypted to a public key — paste the matching ASCII-armored '-----BEGIN PGP PRIVATE KEY BLOCK-----' private key{}",
            if esk.recipient_key_ids.is_empty() {
                String::new()
            } else {
                format!(" (recipient key ID: {})", esk.recipient_key_ids.join(", "))
            }
        ));
    } else {
        return Err(
            "this message is password-encrypted (the 'gpg --symmetric' shape) — type the password it was encrypted with in the passphrase field"
                .into(),
        );
    };

    let compressed = matches!(decrypted, Message::Compressed(_));
    let decrypted = if compressed {
        decrypted
            .decompress()
            .map_err(|e| format!("decrypted, but the payload could not be decompressed: {e}"))?
    } else {
        decrypted
    };

    let signature = if matches!(decrypted, Message::Signed { .. }) {
        Some(inspect_signature(&decrypted, public_key)?)
    } else {
        None
    };

    let literal = decrypted.get_literal().ok_or_else(|| {
        "decryption succeeded but the message held no literal data packet, so there is nothing to show"
            .to_string()
    })?;
    let binary = literal.is_binary();
    let data = literal.data().to_vec();
    let bytes = data.len();

    let as_text = std::str::from_utf8(&data).ok().map(|s| {
        if binary {
            s.to_string()
        } else {
            crlf_to_lf(s)
        }
    });

    let (format_label, plaintext) = match output_format {
        OutputFormat::Text => match as_text {
            Some(t) => ("text", t),
            None => {
                return Err(format!(
                    "the decrypted payload is {bytes} bytes of binary data, not UTF-8 text — set output_format to 'base64' or 'hex' to see it"
                ))
            }
        },
        OutputFormat::Auto => match as_text {
            Some(t) => ("text", t),
            None => (
                "base64",
                base64::engine::general_purpose::STANDARD.encode(&data),
            ),
        },
        OutputFormat::Base64 => (
            "base64",
            base64::engine::general_purpose::STANDARD.encode(&data),
        ),
        OutputFormat::Hex => ("hex", hex::encode(&data)),
    };

    Ok(DecryptResult {
        plaintext,
        encryption,
        output_format: format_label,
        bytes,
        binary,
        compressed,
        decrypted_with_key_id,
        recipient_key_ids: esk.recipient_key_ids,
        signature,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pgp::composed::{KeyType, SecretKeyParamsBuilder, SubkeyParamsBuilder};
    use pgp::crypto::sym::SymmetricKeyAlgorithm;
    use pgp::types::{CompressionAlgorithm, SecretKeyTrait, StringToKey};
    use rand::rngs::OsRng;
    use smallvec::smallvec;

    /// RSA-2048 primary (sign+certify) + RSA-2048 encryption subkey — the
    /// standard GPG layout, and the shape `pgp-encrypt` targets.
    fn gen_secret(passphrase: Option<String>) -> pgp::SignedSecretKey {
        let mut subkey = SubkeyParamsBuilder::default();
        subkey
            .key_type(KeyType::Rsa(2048))
            .can_encrypt(true)
            .passphrase(passphrase.clone());
        let subkey = subkey.build().unwrap();
        let mut params = SecretKeyParamsBuilder::default();
        params
            .key_type(KeyType::Rsa(2048))
            .can_sign(true)
            .can_certify(true)
            .primary_user_id("Test <test@example.com>".into())
            .passphrase(passphrase.clone())
            .preferred_symmetric_algorithms(smallvec![SymmetricKeyAlgorithm::AES256])
            .subkey(subkey);
        let params = params.build().unwrap();
        let sk = params.generate(OsRng).unwrap();
        let pw = passphrase.unwrap_or_default();
        sk.sign(OsRng, || pw.clone()).unwrap()
    }

    fn armored_secret(sk: &pgp::SignedSecretKey) -> String {
        sk.to_armored_string(Default::default()).unwrap()
    }

    fn armored_public(sk: &pgp::SignedSecretKey, pw: &str) -> String {
        sk.public_key()
            .sign(OsRng, &sk.primary_key, || pw.to_string())
            .unwrap()
            .to_armored_string(Default::default())
            .unwrap()
    }

    /// The encryption subkey messages are encrypted to, as `pgp-encrypt` picks
    /// it. Derived from the secret subkey so no passphrase is needed
    /// (`PublicKey::public_subkeys` is private, unlike `SignedPublicKey`'s).
    fn encryption_subkey(sk: &pgp::SignedSecretKey) -> pgp::packet::PublicSubkey {
        sk.secret_subkeys
            .iter()
            .map(|s| s.key.public_key())
            .find(|k| k.is_encryption_key())
            .expect("encryption subkey")
    }

    /// Encrypt `data` to `sk`'s encryption subkey, exactly as `pgp-encrypt` does.
    fn encrypt_to(sk: &pgp::SignedSecretKey, data: &[u8]) -> String {
        let sub = encryption_subkey(sk);
        Message::new_literal_bytes("", data)
            .encrypt_to_keys_seipdv1(&mut OsRng, SymmetricKeyAlgorithm::AES256, &[&sub])
            .unwrap()
            .to_armored_string(Default::default())
            .unwrap()
    }

    fn encrypt_with_password(data: &[u8], pw: &str) -> String {
        let pw = pw.to_string();
        let s2k = StringToKey::new_default(OsRng);
        Message::new_literal_bytes("", data)
            .encrypt_with_password_seipdv1(&mut OsRng, s2k, SymmetricKeyAlgorithm::AES256, || {
                pw.clone()
            })
            .unwrap()
            .to_armored_string(Default::default())
            .unwrap()
    }

    // --- happy paths -----------------------------------------------------

    #[test]
    fn decrypts_a_public_key_encrypted_message() {
        let sk = gen_secret(None);
        let armored = encrypt_to(&sk, b"attack at dawn");
        let res = run(&armored, &armored_secret(&sk), "", "", OutputFormat::Auto).unwrap();
        assert_eq!(res.plaintext, "attack at dawn");
        assert_eq!(res.encryption, "public-key");
        assert_eq!(res.output_format, "text");
        assert_eq!(res.bytes, 14);
        assert!(res.decrypted_with_key_id.is_some());
        assert_eq!(res.recipient_key_ids.len(), 1);
        assert!(res.signature.is_none());
    }

    #[test]
    fn decrypts_a_passphrase_protected_key() {
        let sk = gen_secret(Some("hunter2".into()));
        let armored = encrypt_to(&sk, b"protected key payload");
        let res = run(
            &armored,
            &armored_secret(&sk),
            "hunter2",
            "",
            OutputFormat::Auto,
        )
        .unwrap();
        assert_eq!(res.plaintext, "protected key payload");
    }

    #[test]
    fn decrypts_a_password_encrypted_message() {
        let armored = encrypt_with_password(b"symmetric secret", "correct horse");
        let res = run(&armored, "", "correct horse", "", OutputFormat::Auto).unwrap();
        assert_eq!(res.plaintext, "symmetric secret");
        assert_eq!(res.encryption, "password");
        assert!(res.recipient_key_ids.is_empty());
        assert!(res.decrypted_with_key_id.is_none());
    }

    #[test]
    fn decrypts_a_compressed_and_signed_message_and_verifies_it() {
        let sk = gen_secret(None);
        let sub = encryption_subkey(&sk);
        let armored = Message::new_literal_bytes("", b"signed and sealed")
            .sign(OsRng, &sk, String::new, sk.hash_alg())
            .unwrap()
            .compress(CompressionAlgorithm::ZLIB)
            .unwrap()
            .encrypt_to_keys_seipdv1(&mut OsRng, SymmetricKeyAlgorithm::AES256, &[&sub])
            .unwrap()
            .to_armored_string(Default::default())
            .unwrap();

        let res = run(
            &armored,
            &armored_secret(&sk),
            "",
            &armored_public(&sk, ""),
            OutputFormat::Auto,
        )
        .unwrap();
        assert_eq!(res.plaintext, "signed and sealed");
        assert!(res.compressed, "the payload was zlib-compressed");
        let sig = res.signature.expect("signature reported");
        assert_eq!(sig.valid, Some(true), "note: {:?}", sig.note);
        assert_eq!(sig.signer_user_id.as_deref(), Some("Test <test@example.com>"));
        assert!(sig.signer_fingerprint.is_some());
        assert_eq!(sig.hash_algorithm.as_deref(), Some("SHA256"));
    }

    #[test]
    fn reports_a_signature_without_a_public_key_but_does_not_verify_it() {
        let sk = gen_secret(None);
        let sub = encryption_subkey(&sk);
        let armored = Message::new_literal_bytes("", b"unchecked")
            .sign(OsRng, &sk, String::new, sk.hash_alg())
            .unwrap()
            .encrypt_to_keys_seipdv1(&mut OsRng, SymmetricKeyAlgorithm::AES256, &[&sub])
            .unwrap()
            .to_armored_string(Default::default())
            .unwrap();
        let res = run(&armored, &armored_secret(&sk), "", "", OutputFormat::Auto).unwrap();
        let sig = res.signature.expect("signature reported");
        assert_eq!(sig.valid, None);
        assert!(sig.note.unwrap().contains("not verified"));
    }

    #[test]
    fn detects_a_signature_made_by_a_different_key() {
        let sk = gen_secret(None);
        let other = gen_secret(None);
        let sub = encryption_subkey(&sk);
        let armored = Message::new_literal_bytes("", b"whose signature?")
            .sign(OsRng, &sk, String::new, sk.hash_alg())
            .unwrap()
            .encrypt_to_keys_seipdv1(&mut OsRng, SymmetricKeyAlgorithm::AES256, &[&sub])
            .unwrap()
            .to_armored_string(Default::default())
            .unwrap();
        let res = run(
            &armored,
            &armored_secret(&sk),
            "",
            &armored_public(&other, ""),
            OutputFormat::Auto,
        )
        .unwrap();
        assert_eq!(res.signature.unwrap().valid, Some(false));
    }

    // --- output formats --------------------------------------------------

    #[test]
    fn binary_payloads_fall_back_to_base64_and_render_as_hex_on_request() {
        let sk = gen_secret(None);
        let blob = [0xffu8, 0x00, 0xfe, 0x01];
        let armored = encrypt_to(&sk, &blob);

        let auto = run(&armored, &armored_secret(&sk), "", "", OutputFormat::Auto).unwrap();
        assert_eq!(auto.output_format, "base64");
        assert_eq!(auto.plaintext, "/wD+AQ==");
        assert!(auto.binary, "the literal packet declares binary data");
        assert_eq!(auto.bytes, 4);

        let as_hex = run(&armored, &armored_secret(&sk), "", "", OutputFormat::Hex).unwrap();
        assert_eq!(as_hex.plaintext, "ff00fe01");
        assert_eq!(as_hex.output_format, "hex");

        let forced_text = run(&armored, &armored_secret(&sk), "", "", OutputFormat::Text);
        assert!(forced_text.unwrap_err().contains("not UTF-8 text"));
    }

    #[test]
    fn base64_is_available_for_text_payloads_too() {
        let sk = gen_secret(None);
        let armored = encrypt_to(&sk, b"hi");
        let res = run(&armored, &armored_secret(&sk), "", "", OutputFormat::Base64).unwrap();
        assert_eq!(res.plaintext, "aGk=");
    }

    #[test]
    fn output_format_parsing_is_forgiving_but_bounded() {
        assert_eq!(OutputFormat::parse("").unwrap(), OutputFormat::Auto);
        assert_eq!(OutputFormat::parse(" AUTO ").unwrap(), OutputFormat::Auto);
        assert_eq!(OutputFormat::parse("utf-8").unwrap(), OutputFormat::Text);
        assert_eq!(OutputFormat::parse("Base64").unwrap(), OutputFormat::Base64);
        assert_eq!(OutputFormat::parse("hex").unwrap(), OutputFormat::Hex);
        assert!(OutputFormat::parse("yaml").unwrap_err().contains("unknown output_format"));
    }

    #[test]
    fn text_mode_literals_come_back_with_lf_line_endings() {
        let sk = gen_secret(None);
        let sub = encryption_subkey(&sk);
        // `new_literal` stores UTF-8 text mode, normalized to CRLF on the wire.
        let armored = Message::new_literal("", "line one\nline two")
            .encrypt_to_keys_seipdv1(&mut OsRng, SymmetricKeyAlgorithm::AES256, &[&sub])
            .unwrap()
            .to_armored_string(Default::default())
            .unwrap();
        let res = run(&armored, &armored_secret(&sk), "", "", OutputFormat::Auto).unwrap();
        assert!(!res.binary);
        assert_eq!(res.plaintext, "line one\nline two");
    }

    // --- error paths -----------------------------------------------------

    #[test]
    fn rejects_an_empty_or_non_pgp_message() {
        assert!(run("", "", "", "", OutputFormat::Auto)
            .unwrap_err()
            .contains("no PGP message supplied"));
        assert!(run("just some text", "", "", "", OutputFormat::Auto)
            .unwrap_err()
            .contains("no PGP message found"));
    }

    #[test]
    fn points_a_clearsigned_block_at_the_verify_tool() {
        let block = "-----BEGIN PGP SIGNED MESSAGE-----\nHash: SHA256\n\nhi\n";
        let err = run(block, "", "", "", OutputFormat::Auto).unwrap_err();
        assert!(err.contains("clearsigned"), "{err}");
    }

    #[test]
    fn asks_for_a_private_key_when_none_was_given() {
        let sk = gen_secret(None);
        let armored = encrypt_to(&sk, b"locked");
        let err = run(&armored, "", "", "", OutputFormat::Auto).unwrap_err();
        assert!(err.contains("encrypted to a public key"), "{err}");
        assert!(err.contains("recipient key ID"), "{err}");
    }

    #[test]
    fn asks_for_a_password_when_the_message_is_symmetric() {
        let armored = encrypt_with_password(b"x", "pw");
        let err = run(&armored, "", "", "", OutputFormat::Auto).unwrap_err();
        assert!(err.contains("password-encrypted"), "{err}");
    }

    #[test]
    fn rejects_a_public_key_pasted_into_the_private_key_field() {
        let sk = gen_secret(None);
        let armored = encrypt_to(&sk, b"x");
        let err = run(&armored, &armored_public(&sk, ""), "", "", OutputFormat::Auto).unwrap_err();
        assert!(err.contains("that is a PUBLIC key"), "{err}");
    }

    #[test]
    fn reports_the_wrong_private_key_as_a_recipient_mismatch() {
        let sk = gen_secret(None);
        let other = gen_secret(None);
        let armored = encrypt_to(&sk, b"not for you");
        let err = run(&armored, &armored_secret(&other), "", "", OutputFormat::Auto).unwrap_err();
        assert!(err.contains("not one of this message's recipients"), "{err}");
    }

    #[test]
    fn reports_a_wrong_key_passphrase() {
        let sk = gen_secret(Some("hunter2".into()));
        let armored = encrypt_to(&sk, b"secret");
        let err = run(
            &armored,
            &armored_secret(&sk),
            "hunter3",
            "",
            OutputFormat::Auto,
        )
        .unwrap_err();
        assert!(err.contains("passphrase looks wrong"), "{err}");
    }

    #[test]
    fn reports_a_missing_key_passphrase() {
        let sk = gen_secret(Some("hunter2".into()));
        let armored = encrypt_to(&sk, b"secret");
        let err = run(&armored, &armored_secret(&sk), "", "", OutputFormat::Auto).unwrap_err();
        assert!(err.contains("passphrase-protected"), "{err}");
    }

    #[test]
    fn reports_a_wrong_symmetric_password() {
        let armored = encrypt_with_password(b"secret", "right");
        let err = run(&armored, "", "wrong", "", OutputFormat::Auto).unwrap_err();
        assert!(err.contains("decryption failed with that password"), "{err}");
    }

    #[test]
    fn rejects_an_oversized_message() {
        let huge = format!(
            "-----BEGIN PGP MESSAGE-----\n{}\n-----END PGP MESSAGE-----",
            "A".repeat(MAX_MESSAGE_BYTES)
        );
        let err = run(&huge, "", "", "", OutputFormat::Auto).unwrap_err();
        assert!(err.contains("over the"), "{err}");
    }

    #[test]
    fn rejects_a_bad_public_key_for_signature_checking() {
        let sk = gen_secret(None);
        let sub = encryption_subkey(&sk);
        let armored = Message::new_literal_bytes("", b"signed")
            .sign(OsRng, &sk, String::new, sk.hash_alg())
            .unwrap()
            .encrypt_to_keys_seipdv1(&mut OsRng, SymmetricKeyAlgorithm::AES256, &[&sub])
            .unwrap()
            .to_armored_string(Default::default())
            .unwrap();
        let err = run(
            &armored,
            &armored_secret(&sk),
            "",
            "definitely not a key",
            OutputFormat::Auto,
        )
        .unwrap_err();
        assert!(err.contains("public_key is not a PGP public key"), "{err}");
    }

    #[test]
    fn result_serializes_to_the_expected_json_shape() {
        let sk = gen_secret(None);
        let armored = encrypt_to(&sk, b"json shape");
        let v = run(&armored, &armored_secret(&sk), "", "", OutputFormat::Auto)
            .unwrap()
            .to_json();
        assert_eq!(v["plaintext"], "json shape");
        assert_eq!(v["encryption"], "public-key");
        assert_eq!(v["output_format"], "text");
        assert_eq!(v["bytes"], 10);
        assert_eq!(v["compressed"], serde_json::Value::Bool(false));
        assert!(v["recipient_key_ids"].is_array());
        assert!(v.get("signature").is_none(), "unsigned message omits it");
    }
}
