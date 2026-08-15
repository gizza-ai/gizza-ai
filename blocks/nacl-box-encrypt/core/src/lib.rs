//! gizza-ai/nacl-box-encrypt core — NaCl `crypto_box` public-key authenticated
//! encryption: X25519 (Curve25519) key agreement plus XSalsa20-Poly1305.
//! Pure compute, shared by the chat skill block and the web page.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use crypto_box::aead::Aead;
use crypto_box::{Nonce, PublicKey, SalsaBox, SecretKey};

pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 24;
pub const TAG_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Encrypt,
    Decrypt,
}

impl Operation {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "encrypt" | "seal" => Ok(Operation::Encrypt),
            "decrypt" | "open" => Ok(Operation::Decrypt),
            other => Err(format!(
                "unknown operation '{other}' (use encrypt or decrypt)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Hex,
    Base64,
    Text,
}

impl Encoding {
    fn parse_in(s: &str, field: &str, allowed: &[Encoding]) -> Result<Self, String> {
        let enc = match s.trim().to_ascii_lowercase().as_str() {
            "" => allowed[0],
            "hex" | "base16" => Encoding::Hex,
            "base64" | "b64" => Encoding::Base64,
            "text" | "utf8" | "utf-8" => Encoding::Text,
            other => {
                return Err(format!(
                    "unknown {field} '{other}' (use {})",
                    names(allowed)
                ))
            }
        };
        if allowed.contains(&enc) {
            Ok(enc)
        } else {
            Err(format!(
                "unsupported {field} '{s}' (use {})",
                names(allowed)
            ))
        }
    }

    pub fn parse_key(s: &str) -> Result<Self, String> {
        Encoding::parse_in(s, "key_encoding", &[Encoding::Hex, Encoding::Base64])
    }
    pub fn parse_nonce(s: &str) -> Result<Self, String> {
        Encoding::parse_in(s, "nonce_encoding", &[Encoding::Hex, Encoding::Base64])
    }
    pub fn parse_data(s: &str, default: Encoding) -> Result<Self, String> {
        let mut allowed = vec![default];
        for enc in [Encoding::Text, Encoding::Hex, Encoding::Base64] {
            if enc != default {
                allowed.push(enc);
            }
        }
        Encoding::parse_in(s, "data_encoding", &allowed)
    }
    pub fn parse_output(s: &str) -> Result<Self, String> {
        Encoding::parse_in(s, "output_encoding", &[Encoding::Base64, Encoding::Hex])
    }

    pub fn decode(self, s: &str) -> Result<Vec<u8>, String> {
        match self {
            Encoding::Text => Ok(s.as_bytes().to_vec()),
            Encoding::Hex => hex::decode(s.trim()).map_err(|e| format!("invalid hex: {e}")),
            Encoding::Base64 => B64
                .decode(s.trim())
                .map_err(|e| format!("invalid base64: {e}")),
        }
    }
    pub fn encode(self, bytes: &[u8]) -> String {
        match self {
            Encoding::Text => String::from_utf8_lossy(bytes).into_owned(),
            Encoding::Hex => hex::encode(bytes),
            Encoding::Base64 => B64.encode(bytes),
        }
    }
}

fn names(allowed: &[Encoding]) -> String {
    allowed
        .iter()
        .map(|e| match e {
            Encoding::Hex => "hex",
            Encoding::Base64 => "base64",
            Encoding::Text => "text",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_key32(input: &str, enc: Encoding, field: &str) -> Result<[u8; KEY_LEN], String> {
    let bytes = enc.decode(input)?;
    if bytes.len() != KEY_LEN {
        return Err(format!(
            "{field} must decode to exactly {KEY_LEN} bytes, got {}",
            bytes.len()
        ));
    }
    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn parse_nonce24(input: &str, enc: Encoding) -> Result<[u8; NONCE_LEN], String> {
    let bytes = enc.decode(input)?;
    if bytes.len() != NONCE_LEN {
        return Err(format!(
            "nonce must decode to exactly {NONCE_LEN} bytes, got {}",
            bytes.len()
        ));
    }
    let mut out = [0u8; NONCE_LEN];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// Encrypt plaintext to `nonce || ciphertext || 16-byte tag`.
pub fn encrypt(
    data: &str,
    recipient_public_key: &str,
    sender_secret_key: &str,
    nonce: &str,
    key_enc: Encoding,
    nonce_enc: Encoding,
    data_enc: Encoding,
    out_enc: Encoding,
) -> Result<String, String> {
    if nonce.trim().is_empty() {
        return Err(format!(
            "a nonce is required for encryption: supply a unique {NONCE_LEN}-byte value"
        ));
    }
    let recipient = PublicKey::from(parse_key32(
        recipient_public_key,
        key_enc,
        "recipient public key",
    )?);
    let sender = SecretKey::from(parse_key32(
        sender_secret_key,
        key_enc,
        "sender secret key",
    )?);
    let nonce_bytes = parse_nonce24(nonce, nonce_enc)?;
    let plaintext = data_enc.decode(data)?;
    let c = SalsaBox::new(&recipient, &sender);
    let boxed = c
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_ref())
        .map_err(|_| "encryption failed".to_string())?;
    let mut combined = Vec::with_capacity(NONCE_LEN + boxed.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&boxed);
    Ok(out_enc.encode(&combined))
}

/// Decrypt a combined `nonce || ciphertext || tag`, or `ciphertext || tag` when
/// a nonce is supplied separately.
pub fn decrypt(
    data: &str,
    recipient_secret_key: &str,
    sender_public_key: &str,
    nonce: &str,
    key_enc: Encoding,
    nonce_enc: Encoding,
    data_enc: Encoding,
    out_enc: Encoding,
) -> Result<String, String> {
    let recipient = SecretKey::from(parse_key32(
        recipient_secret_key,
        key_enc,
        "recipient secret key",
    )?);
    let sender = PublicKey::from(parse_key32(
        sender_public_key,
        key_enc,
        "sender public key",
    )?);
    let blob = data_enc.decode(data)?;
    let (nonce_bytes, ciphertext): ([u8; NONCE_LEN], &[u8]) = if nonce.trim().is_empty() {
        if blob.len() < NONCE_LEN + TAG_LEN {
            return Err(format!("combined ciphertext too short ({} bytes): expected nonce plus at least a {TAG_LEN}-byte tag", blob.len()));
        }
        let mut n = [0u8; NONCE_LEN];
        n.copy_from_slice(&blob[..NONCE_LEN]);
        (n, &blob[NONCE_LEN..])
    } else {
        (parse_nonce24(nonce, nonce_enc)?, &blob[..])
    };
    let c = SalsaBox::new(&sender, &recipient);
    let plaintext = c
        .decrypt(Nonce::from_slice(&nonce_bytes), ciphertext)
        .map_err(|_| "authentication failed: the Poly1305 tag does not verify (wrong key/nonce/sender/recipient or tampered ciphertext)".to_string())?;
    match String::from_utf8(plaintext) {
        Ok(text) => Ok(text),
        Err(e) => Ok(out_enc.encode(e.as_bytes())),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    operation: &str,
    data: &str,
    recipient_key: &str,
    sender_key: &str,
    nonce: &str,
    key_encoding: &str,
    nonce_encoding: &str,
    data_encoding: &str,
    output_encoding: &str,
) -> Result<String, String> {
    let op = Operation::parse(operation)?;
    let key_enc = Encoding::parse_key(key_encoding)?;
    let nonce_enc = Encoding::parse_nonce(nonce_encoding)?;
    let out_enc = Encoding::parse_output(output_encoding)?;
    match op {
        Operation::Encrypt => encrypt(
            data,
            recipient_key,
            sender_key,
            nonce,
            key_enc,
            nonce_enc,
            Encoding::parse_data(data_encoding, Encoding::Text)?,
            out_enc,
        ),
        Operation::Decrypt => decrypt(
            data,
            recipient_key,
            sender_key,
            nonce,
            key_enc,
            nonce_enc,
            Encoding::parse_data(data_encoding, Encoding::Base64)?,
            out_enc,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE_SECRET: &str = "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a";
    const ALICE_PUBLIC: &str = "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a";
    const BOB_SECRET: &str = "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb";
    const BOB_PUBLIC: &str = "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f";
    const NONCE: &str = "000102030405060708090a0b0c0d0e0f1011121314151617";

    #[test]
    fn combined_roundtrip_hex() {
        let ct = encrypt(
            "attack at dawn",
            BOB_PUBLIC,
            ALICE_SECRET,
            NONCE,
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Text,
            Encoding::Hex,
        )
        .unwrap();
        assert!(ct.starts_with(NONCE));
        let pt = decrypt(
            &ct,
            BOB_SECRET,
            ALICE_PUBLIC,
            "",
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Base64,
        )
        .unwrap();
        assert_eq!(pt, "attack at dawn");
    }

    #[test]
    fn combined_roundtrip_base64_default() {
        let ct = run(
            "encrypt",
            "hello box",
            BOB_PUBLIC,
            ALICE_SECRET,
            NONCE,
            "hex",
            "hex",
            "text",
            "base64",
        )
        .unwrap();
        let pt = run(
            "decrypt",
            &ct,
            BOB_SECRET,
            ALICE_PUBLIC,
            "",
            "hex",
            "hex",
            "base64",
            "base64",
        )
        .unwrap();
        assert_eq!(pt, "hello box");
    }

    #[test]
    fn decrypt_with_separate_nonce() {
        let ct = encrypt(
            "separate",
            BOB_PUBLIC,
            ALICE_SECRET,
            NONCE,
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Text,
            Encoding::Hex,
        )
        .unwrap();
        let combined = hex::decode(&ct).unwrap();
        let box_only = hex::encode(&combined[NONCE_LEN..]);
        let pt = decrypt(
            &box_only,
            BOB_SECRET,
            ALICE_PUBLIC,
            NONCE,
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Base64,
        )
        .unwrap();
        assert_eq!(pt, "separate");
    }

    #[test]
    fn tamper_is_rejected() {
        let ct = encrypt(
            "authentic",
            BOB_PUBLIC,
            ALICE_SECRET,
            NONCE,
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Text,
            Encoding::Hex,
        )
        .unwrap();
        let mut bytes = hex::decode(ct).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(decrypt(
            &hex::encode(bytes),
            BOB_SECRET,
            ALICE_PUBLIC,
            "",
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Hex,
            Encoding::Base64
        )
        .is_err());
    }

    #[test]
    fn validates_lengths_and_enums() {
        assert!(
            run("encrypt", "x", BOB_PUBLIC, "abcd", NONCE, "hex", "hex", "text", "base64").is_err()
        );
        assert!(run(
            "encrypt",
            "x",
            BOB_PUBLIC,
            ALICE_SECRET,
            "abcd",
            "hex",
            "hex",
            "text",
            "base64"
        )
        .is_err());
        assert!(run(
            "wrap",
            "x",
            BOB_PUBLIC,
            ALICE_SECRET,
            NONCE,
            "hex",
            "hex",
            "text",
            "base64"
        )
        .is_err());
    }
}
