//! gizza-ai/ecies-encrypt core — ECIES (Elliptic Curve Integrated Encryption
//! Scheme): encrypt a message to a recipient's elliptic-curve PUBLIC key and
//! decrypt it with the matching PRIVATE key. Pure compute, shared by the chat
//! skill block and the web page.
//!
//! The scheme is the hybrid one every ECIES library implements:
//!
//! 1. A fresh single-use ("ephemeral") key pair is generated on the chosen curve.
//! 2. ECDH between the ephemeral private key and the recipient public key
//!    yields a shared point.
//! 3. HKDF-SHA256 turns that into a 32-byte data-encryption key.
//! 4. The message is sealed with AES-256-GCM (or XChaCha20-Poly1305).
//! 5. The ephemeral PUBLIC key travels with the ciphertext so the recipient can
//!    redo step 2 with their private key.
//!
//! Two details differ between implementations, so both are parameters here:
//!
//! * **KDF input.** The `ecies/py` + `ecies/js` family feeds HKDF the
//!   ephemeral public key concatenated with the shared point
//!   (`kdf_input = ephemeral-and-point`, the default). SEC 1 /
//!   WebCrypto-style stacks feed it only the shared point's X coordinate
//!   (`kdf_input = shared-x`).
//! * **Nonce length.** `ecies/py` defaults to a 16-byte AES-GCM nonce; plain
//!   NIST AES-GCM uses 12. XChaCha20-Poly1305 is always 24.
//!
//! Both points always go into HKDF in uncompressed SEC1 form — the
//! `ecies/py` / `ecies/js` default (`is_hkdf_key_compressed = False`).
//! `compressed_ephemeral` only changes how the ephemeral public key is written
//! into the payload, never the derived key.
//!
//! Payload layout (matching the `ecies/py` + `ecies/js` wire format):
//! `ephemeral public key || nonce || 16-byte tag || ciphertext`.

use aes_gcm::aead::consts::{U12, U16};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::aes::Aes256;
use aes_gcm::AesGcm;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::XChaCha20Poly1305;
use hkdf::Hkdf;
use sha2::Sha256;

/// AES-256-GCM with the 16-byte nonce `ecies/py` defaults to.
type Aes256Gcm16 = AesGcm<Aes256, U16>;
/// AES-256-GCM with the 12-byte nonce NIST SP 800-38D recommends.
type Aes256Gcm12 = AesGcm<Aes256, U12>;

/// Poly1305 / GCM tag length in bytes — the same 16 for every cipher here.
pub const TAG_LEN: usize = 16;
/// Derived data-encryption key length in bytes (AES-256 / XChaCha20).
pub const DEK_LEN: usize = 32;
/// Largest accepted input, in bytes (after decoding). Everything is buffered in
/// memory inside a 64 MiB wasm sandbox alongside the ciphertext copy.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

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
                "unknown operation '{other}' (expected encrypt or decrypt)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    Secp256k1,
    P256,
    P384,
}

impl Curve {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "" | "secp256k1" | "k256" => Ok(Curve::Secp256k1),
            "p256" | "p-256" | "secp256r1" | "prime256v1" => Ok(Curve::P256),
            "p384" | "p-384" | "secp384r1" => Ok(Curve::P384),
            other => Err(format!(
                "unknown curve '{other}' (expected secp256k1, p256 or p384)"
            )),
        }
    }

    /// Coordinate size in bytes — how wide X and Y are in a SEC1 point.
    pub fn field_len(self) -> usize {
        match self {
            Curve::Secp256k1 | Curve::P256 => 32,
            Curve::P384 => 48,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Curve::Secp256k1 => "secp256k1",
            Curve::P256 => "p256",
            Curve::P384 => "p384",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cipher {
    Aes256Gcm,
    XChaCha20Poly1305,
}

impl Cipher {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "" | "aes-256-gcm" | "aes256gcm" | "aes-gcm" => Ok(Cipher::Aes256Gcm),
            "xchacha20-poly1305" | "xchacha20poly1305" | "xchacha20" | "xchacha" => {
                Ok(Cipher::XChaCha20Poly1305)
            }
            other => Err(format!(
                "unknown cipher '{other}' (expected aes-256-gcm or xchacha20-poly1305)"
            )),
        }
    }
}

/// What gets fed into HKDF-SHA256 as input keying material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KdfInput {
    /// `ecies/py` + `ecies/js`: uncompressed ephemeral public key ‖ uncompressed shared point.
    EphemeralAndPoint,
    /// SEC 1 / WebCrypto style: only the shared point's X coordinate.
    SharedX,
}

impl KdfInput {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "" | "ephemeral-and-point" => Ok(KdfInput::EphemeralAndPoint),
            "shared-x" | "x" | "sec1" => Ok(KdfInput::SharedX),
            other => Err(format!(
                "unknown kdf_input '{other}' (expected ephemeral-and-point or shared-x)"
            )),
        }
    }
}

/// How a binary field is spelled in text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinEncoding {
    Hex,
    Base64,
}

impl BinEncoding {
    fn encode(self, bytes: &[u8]) -> String {
        match self {
            BinEncoding::Hex => hex::encode(bytes),
            BinEncoding::Base64 => B64.encode(bytes),
        }
    }

    pub fn parse_output(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "base64" | "b64" => Ok(BinEncoding::Base64),
            "hex" => Ok(BinEncoding::Hex),
            other => Err(format!(
                "unknown output_encoding '{other}' (expected base64 or hex)"
            )),
        }
    }
}

/// How the `data` field is spelled. `Auto` means "text when encrypting,
/// base64 when decrypting" — the shape each direction is almost always in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataEncoding {
    Auto,
    Text,
    Hex,
    Base64,
}

impl DataEncoding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(DataEncoding::Auto),
            "text" | "utf8" | "utf-8" => Ok(DataEncoding::Text),
            "hex" => Ok(DataEncoding::Hex),
            "base64" | "b64" => Ok(DataEncoding::Base64),
            other => Err(format!(
                "unknown data_encoding '{other}' (expected auto, text, hex or base64)"
            )),
        }
    }

    fn decode(self, s: &str, op: Operation) -> Result<Vec<u8>, String> {
        let resolved = match (self, op) {
            (DataEncoding::Auto, Operation::Encrypt) => DataEncoding::Text,
            (DataEncoding::Auto, Operation::Decrypt) => DataEncoding::Base64,
            (other, _) => other,
        };
        match (resolved, op) {
            (DataEncoding::Text, Operation::Decrypt) => Err(
                "data_encoding=text cannot describe an encrypted payload; use hex or base64 (or leave it on auto)"
                    .to_string(),
            ),
            (DataEncoding::Text, _) => Ok(s.as_bytes().to_vec()),
            (DataEncoding::Hex, _) => decode_hex(s, "data"),
            (DataEncoding::Base64, _) => decode_b64(s, "data"),
            (DataEncoding::Auto, _) => unreachable!("auto resolved above"),
        }
    }
}

/// How a key is spelled. `Auto` sniffs PEM, then hex, then base64.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEncoding {
    Auto,
    Hex,
    Base64,
    Pem,
}

impl KeyEncoding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(KeyEncoding::Auto),
            "hex" => Ok(KeyEncoding::Hex),
            "base64" | "b64" => Ok(KeyEncoding::Base64),
            "pem" => Ok(KeyEncoding::Pem),
            other => Err(format!(
                "unknown key_encoding '{other}' (expected auto, hex, base64 or pem)"
            )),
        }
    }
}

/// A key as the caller supplied it: raw bytes, or a PEM document.
pub enum KeyMaterial {
    Raw(Vec<u8>),
    Pem(String),
}

fn key_material(s: &str, enc: KeyEncoding, field: &str) -> Result<KeyMaterial, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} is required"));
    }
    let looks_pem = trimmed.starts_with("-----BEGIN");
    match enc {
        KeyEncoding::Pem => {
            if !looks_pem {
                return Err(format!(
                    "{field}: key_encoding=pem but the value does not start with -----BEGIN"
                ));
            }
            Ok(KeyMaterial::Pem(trimmed.to_string()))
        }
        KeyEncoding::Hex => Ok(KeyMaterial::Raw(decode_hex(trimmed, field)?)),
        KeyEncoding::Base64 => Ok(KeyMaterial::Raw(decode_b64(trimmed, field)?)),
        KeyEncoding::Auto => {
            if looks_pem {
                Ok(KeyMaterial::Pem(trimmed.to_string()))
            } else if is_hexish(trimmed) {
                Ok(KeyMaterial::Raw(decode_hex(trimmed, field)?))
            } else {
                decode_b64(trimmed, field).map(KeyMaterial::Raw).map_err(|_| {
                    format!("{field} is not a PEM block, hex, or base64 — set key_encoding explicitly")
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Text/byte helpers
// ---------------------------------------------------------------------------

fn strip_ws(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn is_hexish(s: &str) -> bool {
    let c = strip_ws(s);
    let c = c
        .strip_prefix("0x")
        .or_else(|| c.strip_prefix("0X"))
        .unwrap_or(&c);
    !c.is_empty() && c.len() % 2 == 0 && c.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn decode_hex(s: &str, field: &str) -> Result<Vec<u8>, String> {
    let cleaned = strip_ws(s);
    let cleaned = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(&cleaned);
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "{field} is not valid hex: expected an even number of hex digits, got {}",
            cleaned.len()
        ));
    }
    hex::decode(cleaned).map_err(|e| format!("{field} is not valid hex: {e}"))
}

fn decode_b64(s: &str, field: &str) -> Result<Vec<u8>, String> {
    B64.decode(strip_ws(s))
        .map_err(|e| format!("{field} is not valid base64: {e}"))
}

fn hkdf32(ikm: &[u8]) -> [u8; DEK_LEN] {
    let mut okm = [0u8; DEK_LEN];
    // Empty salt + empty info: what ecies/py, ecies/js and WebCrypto stacks use.
    Hkdf::<Sha256>::new(Some(&[]), ikm)
        .expand(&[], &mut okm)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    okm
}

// ---------------------------------------------------------------------------
// Per-curve key handling (one concrete module per curve — the RustCrypto types
// are distinct per curve, so a macro instantiates the identical body three
// times rather than fighting generic trait bounds).
// ---------------------------------------------------------------------------

macro_rules! curve_module {
    ($mod_name:ident, $ck:ident, $field_len:expr) => {
        mod $mod_name {
            use super::{hkdf32, KdfInput, KeyMaterial, DEK_LEN};
            use $ck::elliptic_curve::pkcs8::{DecodePrivateKey, DecodePublicKey};
            use $ck::elliptic_curve::sec1::ToEncodedPoint;
            use $ck::{ProjectivePoint, PublicKey, SecretKey};

            const FIELD: usize = $field_len;

            fn public_from_bytes(b: &[u8], field: &str) -> Result<PublicKey, String> {
                // Some tools print a public key as bare X‖Y with no SEC1 tag.
                let owned;
                let bytes = if b.len() == 2 * FIELD {
                    owned = [&[0x04u8][..], b].concat();
                    &owned[..]
                } else {
                    b
                };
                PublicKey::from_sec1_bytes(bytes).map_err(|_| {
                    format!(
                        "{field} is not a valid public key point on this curve \
                         (expected {} bytes compressed or {} bytes uncompressed SEC1, got {})",
                        1 + FIELD,
                        1 + 2 * FIELD,
                        b.len()
                    )
                })
            }

            fn public_from_pem(s: &str, field: &str) -> Result<PublicKey, String> {
                PublicKey::from_public_key_pem(s).map_err(|e| {
                    format!("{field} is not a readable SubjectPublicKeyInfo PEM public key: {e}")
                })
            }

            fn secret_from_bytes(b: &[u8], field: &str) -> Result<SecretKey, String> {
                if b.len() != FIELD {
                    return Err(format!(
                        "{field} must be a {FIELD}-byte private scalar for this curve, got {} bytes",
                        b.len()
                    ));
                }
                SecretKey::from_slice(b).map_err(|_| {
                    format!("{field} is not a valid private key for this curve (must be non-zero and below the curve order)")
                })
            }

            fn secret_from_pem(s: &str, field: &str) -> Result<SecretKey, String> {
                SecretKey::from_pkcs8_pem(s)
                    .or_else(|_| SecretKey::from_sec1_pem(s))
                    .map_err(|e| format!("{field} is not a readable PKCS#8 or SEC1 PEM private key: {e}"))
            }

            fn public_of(m: &KeyMaterial, field: &str) -> Result<PublicKey, String> {
                match m {
                    KeyMaterial::Raw(b) => public_from_bytes(b, field),
                    KeyMaterial::Pem(s) => public_from_pem(s, field),
                }
            }

            fn secret_of(m: &KeyMaterial, field: &str) -> Result<SecretKey, String> {
                match m {
                    KeyMaterial::Raw(b) => secret_from_bytes(b, field),
                    KeyMaterial::Pem(s) => secret_from_pem(s, field),
                }
            }

            fn encode_public(pk: &PublicKey, compressed: bool) -> Vec<u8> {
                pk.as_affine().to_encoded_point(compressed).as_bytes().to_vec()
            }

            /// ECDH: returns (shared point in uncompressed SEC1 form, shared
            /// point X coordinate).
            fn shared(sk: &SecretKey, pk: &PublicKey) -> Result<(Vec<u8>, Vec<u8>), String> {
                let nz = sk.to_nonzero_scalar();
                let point = (ProjectivePoint::from(*pk.as_affine()) * *nz.as_ref()).to_affine();
                let enc = point.to_encoded_point(false);
                if enc.is_identity() {
                    return Err("ECDH produced the point at infinity — the public key is not usable".to_string());
                }
                let x = enc
                    .x()
                    .ok_or_else(|| "ECDH result has no X coordinate".to_string())?
                    .to_vec();
                Ok((enc.as_bytes().to_vec(), x))
            }

            fn derive(eph_pub: &PublicKey, point: &[u8], x: Vec<u8>, kdf: KdfInput) -> [u8; DEK_LEN] {
                match kdf {
                    KdfInput::EphemeralAndPoint => {
                        // ecies/py `encapsulate`: derive_key(sender_point ‖ shared_point),
                        // both uncompressed (is_hkdf_key_compressed = False).
                        let mut ikm = encode_public(eph_pub, false);
                        ikm.extend_from_slice(point);
                        hkdf32(&ikm)
                    }
                    KdfInput::SharedX => hkdf32(&x),
                }
            }

            /// Encrypt side: returns (ephemeral public key bytes for the payload,
            /// derived 32-byte data-encryption key).
            pub fn kem_encrypt(
                recipient: &KeyMaterial,
                ephemeral: Option<&KeyMaterial>,
                compressed: bool,
                kdf: KdfInput,
            ) -> Result<(Vec<u8>, [u8; DEK_LEN]), String> {
                let recipient_pk = public_of(recipient, "key")?;
                let eph_sk = match ephemeral {
                    Some(m) => secret_of(m, "ephemeral_key")?,
                    None => SecretKey::random(&mut rand::rngs::OsRng),
                };
                let eph_pk = eph_sk.public_key();
                let (point, x) = shared(&eph_sk, &recipient_pk)?;
                let dek = derive(&eph_pk, &point, x, kdf);
                Ok((encode_public(&eph_pk, compressed), dek))
            }

            /// Decrypt side: reads the ephemeral public key off the front of the
            /// payload. Returns (derived key, bytes consumed).
            pub fn kem_decrypt(
                recipient: &KeyMaterial,
                payload: &[u8],
                kdf: KdfInput,
            ) -> Result<([u8; DEK_LEN], usize), String> {
                let tag = *payload.first().ok_or_else(|| "data is empty".to_string())?;
                let len = match tag {
                    0x02 | 0x03 => 1 + FIELD,
                    0x04 => 1 + 2 * FIELD,
                    other => {
                        return Err(format!(
                            "data does not start with an ephemeral public key: expected a SEC1 tag byte 0x02, 0x03 or 0x04, got 0x{other:02x}"
                        ))
                    }
                };
                if payload.len() < len {
                    return Err(format!(
                        "data is too short: the ephemeral public key alone needs {len} bytes, the payload has {}",
                        payload.len()
                    ));
                }
                let eph_pk = public_from_bytes(&payload[..len], "data (ephemeral public key)")?;
                let sk = secret_of(recipient, "key")?;
                let (point, x) = shared(&sk, &eph_pk)?;
                Ok((derive(&eph_pk, &point, x, kdf), len))
            }
        }
    };
}

curve_module!(c_secp256k1, k256, 32);
curve_module!(c_p256, p256, 32);
curve_module!(c_p384, p384, 48);

fn kem_encrypt(
    curve: Curve,
    recipient: &KeyMaterial,
    ephemeral: Option<&KeyMaterial>,
    compressed: bool,
    kdf: KdfInput,
) -> Result<(Vec<u8>, [u8; DEK_LEN]), String> {
    match curve {
        Curve::Secp256k1 => c_secp256k1::kem_encrypt(recipient, ephemeral, compressed, kdf),
        Curve::P256 => c_p256::kem_encrypt(recipient, ephemeral, compressed, kdf),
        Curve::P384 => c_p384::kem_encrypt(recipient, ephemeral, compressed, kdf),
    }
}

fn kem_decrypt(
    curve: Curve,
    recipient: &KeyMaterial,
    payload: &[u8],
    kdf: KdfInput,
) -> Result<([u8; DEK_LEN], usize), String> {
    match curve {
        Curve::Secp256k1 => c_secp256k1::kem_decrypt(recipient, payload, kdf),
        Curve::P256 => c_p256::kem_decrypt(recipient, payload, kdf),
        Curve::P384 => c_p384::kem_decrypt(recipient, payload, kdf),
    }
}

// ---------------------------------------------------------------------------
// Symmetric layer
// ---------------------------------------------------------------------------

fn nonce_len(cipher: Cipher, requested: &str) -> Result<usize, String> {
    match cipher {
        Cipher::XChaCha20Poly1305 => Ok(24),
        Cipher::Aes256Gcm => match requested.trim() {
            "" | "16" => Ok(16),
            "12" => Ok(12),
            other => Err(format!(
                "unknown nonce_length '{other}' (expected 16 or 12 for aes-256-gcm; xchacha20-poly1305 always uses 24)"
            )),
        },
    }
}

/// Seal `plaintext`, returning `(ciphertext_body, tag)`.
fn seal(
    cipher: Cipher,
    key: &[u8; DEK_LEN],
    nonce: &[u8],
    plaintext: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut out = match (cipher, nonce.len()) {
        (Cipher::Aes256Gcm, 16) => Aes256Gcm16::new(key.into())
            .encrypt(nonce.into(), plaintext)
            .map_err(|_| "AES-256-GCM encryption failed".to_string())?,
        (Cipher::Aes256Gcm, 12) => Aes256Gcm12::new(key.into())
            .encrypt(nonce.into(), plaintext)
            .map_err(|_| "AES-256-GCM encryption failed".to_string())?,
        (Cipher::XChaCha20Poly1305, 24) => XChaCha20Poly1305::new(key.into())
            .encrypt(nonce.into(), plaintext)
            .map_err(|_| "XChaCha20-Poly1305 encryption failed".to_string())?,
        (_, n) => {
            return Err(format!(
                "unsupported nonce length {n} for the selected cipher"
            ))
        }
    };
    // The AEAD APIs append the tag; ECIES payloads carry it in front of the
    // ciphertext, so split the two apart here.
    let tag = out.split_off(out.len() - TAG_LEN);
    Ok((out, tag))
}

fn open(
    cipher: Cipher,
    key: &[u8; DEK_LEN],
    nonce: &[u8],
    body: &[u8],
    tag: &[u8],
) -> Result<Vec<u8>, String> {
    let joined = [body, tag].concat();
    let failed = "decryption failed: the authentication tag did not verify (wrong key, curve, cipher, nonce_length, kdf_input, or altered ciphertext)";
    match (cipher, nonce.len()) {
        (Cipher::Aes256Gcm, 16) => Aes256Gcm16::new(key.into())
            .decrypt(nonce.into(), joined.as_slice())
            .map_err(|_| failed.to_string()),
        (Cipher::Aes256Gcm, 12) => Aes256Gcm12::new(key.into())
            .decrypt(nonce.into(), joined.as_slice())
            .map_err(|_| failed.to_string()),
        (Cipher::XChaCha20Poly1305, 24) => XChaCha20Poly1305::new(key.into())
            .decrypt(nonce.into(), joined.as_slice())
            .map_err(|_| failed.to_string()),
        (_, n) => Err(format!(
            "unsupported nonce length {n} for the selected cipher"
        )),
    }
}

fn random_bytes(n: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut v = vec![0u8; n];
    rand::rngs::OsRng.fill_bytes(&mut v);
    v
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run one ECIES operation. Returns the encoded payload (encrypt) or the
/// recovered plaintext (decrypt).
#[allow(clippy::too_many_arguments)]
pub fn run(
    operation: &str,
    data: &str,
    key: &str,
    curve: &str,
    cipher: &str,
    nonce_length: &str,
    nonce: &str,
    compressed_ephemeral: bool,
    kdf_input: &str,
    ephemeral_key: &str,
    key_encoding: &str,
    data_encoding: &str,
    output_encoding: &str,
) -> Result<String, String> {
    let op = Operation::parse(operation)?;
    let curve = Curve::parse(curve)?;
    let cipher = Cipher::parse(cipher)?;
    let kdf = KdfInput::parse(kdf_input)?;
    let key_enc = KeyEncoding::parse(key_encoding)?;
    let data_enc = DataEncoding::parse(data_encoding)?;
    let out_enc = BinEncoding::parse_output(output_encoding)?;
    let nlen = nonce_len(cipher, nonce_length)?;

    if data.trim().is_empty() && op == Operation::Decrypt {
        return Err("data is required: paste the ECIES payload to decrypt".to_string());
    }
    let primary_key = key_material(key, key_enc, "key")?;
    let bytes = data_enc.decode(data, op)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "data is {} bytes after decoding, over the {MAX_INPUT_BYTES}-byte limit",
            bytes.len()
        ));
    }

    match op {
        Operation::Encrypt => {
            let eph_material = if ephemeral_key.trim().is_empty() {
                None
            } else {
                Some(key_material(ephemeral_key, key_enc, "ephemeral_key")?)
            };
            let (eph_pub, dek) = kem_encrypt(
                curve,
                &primary_key,
                eph_material.as_ref(),
                compressed_ephemeral,
                kdf,
            )?;
            let nonce_bytes = if nonce.trim().is_empty() {
                random_bytes(nlen)
            } else {
                let n = if is_hexish(nonce) {
                    decode_hex(nonce, "nonce")?
                } else {
                    decode_b64(nonce, "nonce")?
                };
                if n.len() != nlen {
                    return Err(format!(
                        "nonce must be {nlen} bytes for the selected cipher, got {}",
                        n.len()
                    ));
                }
                n
            };
            let (body, tag) = seal(cipher, &dek, &nonce_bytes, &bytes)?;
            let mut payload = eph_pub;
            payload.extend_from_slice(&nonce_bytes);
            payload.extend_from_slice(&tag);
            payload.extend_from_slice(&body);
            Ok(out_enc.encode(&payload))
        }
        Operation::Decrypt => {
            let (dek, consumed) = kem_decrypt(curve, &primary_key, &bytes, kdf)?;
            let rest = &bytes[consumed..];
            if rest.len() < nlen + TAG_LEN {
                return Err(format!(
                    "data is too short: after the {consumed}-byte ephemeral public key it needs at least {} more bytes (a {nlen}-byte nonce and a {TAG_LEN}-byte tag), but only {} remain",
                    nlen + TAG_LEN,
                    rest.len()
                ));
            }
            let (nonce_bytes, rest) = rest.split_at(nlen);
            let (tag, body) = rest.split_at(TAG_LEN);
            let plain = open(cipher, &dek, nonce_bytes, body, tag)?;
            Ok(match String::from_utf8(plain.clone()) {
                Ok(s) => s,
                Err(_) => out_enc.encode(&plain),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — vectors cross-computed with an independent Python implementation
// (pure-python EC point multiplication + `cryptography`'s HKDF/AES-GCM), i.e.
// the same wire format `ecies/py` and `ecies/js` produce.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Recipient private key = 0x1111…11, ephemeral private key = 0x2222…22.
    const RECIPIENT_SK: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const EPHEMERAL_SK: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const NONCE16: &str = "000102030405060708090a0b0c0d0e0f";
    const NONCE12: &str = "000102030405060708090a0b";
    const NONCE24: &str = "000102030405060708090a0b0c0d0e0f1011121314151617";
    const K256_PUB: &str = "044f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa385b6b1b8ead809ca67454d9683fcf2ba03456d6fe2c4abe2b07f0fbdbb2f1c1";

    #[allow(clippy::too_many_arguments)]
    fn enc(
        data: &str,
        key: &str,
        curve: &str,
        cipher: &str,
        nlen: &str,
        nonce: &str,
        compressed: bool,
        kdf: &str,
        out: &str,
    ) -> Result<String, String> {
        run(
            "encrypt",
            data,
            key,
            curve,
            cipher,
            nlen,
            nonce,
            compressed,
            kdf,
            EPHEMERAL_SK,
            "auto",
            "auto",
            out,
        )
    }

    #[test]
    fn secp256k1_default_matches_reference_vector() {
        let got = enc(
            "attack at dawn",
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "ephemeral-and-point",
            "hex",
        )
        .unwrap();
        assert_eq!(
            got,
            "04466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f276728176c3c6431f8eeda4538dc37c865e2784f3a9e77d044f33e407797e1278a000102030405060708090a0b0c0d0e0f52a1afe24dfe7e9300a0def3bdde58599c7c7ab7575286def1cdb7518a35"
        );
    }

    #[test]
    fn secp256k1_default_base64_round_trips_with_private_key() {
        let payload = enc(
            "attack at dawn",
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "ephemeral-and-point",
            "base64",
        )
        .unwrap();
        assert_eq!(payload, "BEZtf8rlY+XLCaDRhwu1gDRIBGF4eaFJSc8iKF8brj8nZygXbDxkMfju2kU43DfIZeJ4Tzqed9BE8z5Ad5fhJ4oAAQIDBAUGBwgJCgsMDQ4PUqGv4k3+fpMAoN7zvd5YWZx8erdXUobe8c23UYo1");
        let back = run(
            "decrypt",
            &payload,
            RECIPIENT_SK,
            "secp256k1",
            "aes-256-gcm",
            "16",
            "",
            false,
            "ephemeral-and-point",
            "",
            "auto",
            "auto",
            "base64",
        )
        .unwrap();
        assert_eq!(back, "attack at dawn");
    }

    #[test]
    fn compressed_ephemeral_key_with_12_byte_nonce_matches_reference() {
        let got = enc(
            "attack at dawn",
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "12",
            NONCE12,
            true,
            "ephemeral-and-point",
            "hex",
        )
        .unwrap();
        assert_eq!(
            got,
            "02466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f27000102030405060708090a0b0aaf7a5c677ffbc9f59a76871def2e224ee3f5b759efcda23132f89b7513"
        );
        // The 33-byte compressed ephemeral key is self-describing on decrypt.
        let back = run(
            "decrypt",
            &got,
            RECIPIENT_SK,
            "secp256k1",
            "aes-256-gcm",
            "12",
            "",
            false,
            "ephemeral-and-point",
            "",
            "auto",
            "hex",
            "base64",
        )
        .unwrap();
        assert_eq!(back, "attack at dawn");
    }

    #[test]
    fn shared_x_kdf_matches_reference() {
        let got = enc(
            "attack at dawn",
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "shared-x",
            "hex",
        )
        .unwrap();
        assert!(
            got.ends_with("efe2b1a87f99d0b000673bd997df78fad82f9124a2486ce75c5b0af69fb8"),
            "got {got}"
        );
        let back = run(
            "decrypt",
            &got,
            RECIPIENT_SK,
            "secp256k1",
            "aes-256-gcm",
            "16",
            "",
            false,
            "shared-x",
            "",
            "auto",
            "hex",
            "base64",
        )
        .unwrap();
        assert_eq!(back, "attack at dawn");
    }

    #[test]
    fn xchacha20_poly1305_matches_reference() {
        let got = enc(
            "attack at dawn",
            K256_PUB,
            "secp256k1",
            "xchacha20-poly1305",
            "",
            NONCE24,
            false,
            "ephemeral-and-point",
            "hex",
        )
        .unwrap();
        assert!(
            got.ends_with("000102030405060708090a0b0c0d0e0f1011121314151617de5cb2798da05958615ed78ecf256e16b337732030c32a47997ebce7a2c6"),
            "got {got}"
        );
        let back = run(
            "decrypt",
            &got,
            RECIPIENT_SK,
            "secp256k1",
            "xchacha20-poly1305",
            "",
            "",
            false,
            "ephemeral-and-point",
            "",
            "auto",
            "hex",
            "base64",
        )
        .unwrap();
        assert_eq!(back, "attack at dawn");
    }

    #[test]
    fn p256_and_p384_match_reference_vectors() {
        let p256_pub = "040217e617f0b6443928278f96999e69a23a4f2c152bdf6d6cdf66e5b80282d4ed194a7debcb97712d2dda3ca85aa8765a56f45fc758599652f2897c65306e5794";
        let got = enc(
            "attack at dawn",
            p256_pub,
            "p256",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "ephemeral-and-point",
            "hex",
        )
        .unwrap();
        assert_eq!(got, "04d65a93977caa3d1b081852ff57a79e465f1660577304baead505dd3a48589cf350185e895372df6221ea3a137557e473fddb6755f05bd507c3c533fce9c91285000102030405060708090a0b0c0d0e0f7b5453166439ff640acbb43b2836bb092dad7c9a3d738b90d993726818f9");

        let p384_pub = "04d9c665fa834b94b621852f1bc508fb26f9d5cbcdddec8f42705b9560c65e87cdbf93c22b8b694e0f274318b5a3dd566376928da6b87bb9a4b7ff4a45dc03e052b9a9bca6dd10778c826e3f6437ed616bfc86c256d2f224ea70e385273604e4b7";
        let p384_eph = "222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222";
        let got384 = run(
            "encrypt",
            "attack at dawn",
            p384_pub,
            "p384",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "ephemeral-and-point",
            p384_eph,
            "auto",
            "auto",
            "hex",
        )
        .unwrap();
        assert_eq!(got384, "044f2bda7fd2105f8467e21f45223ad58863ffa4c084832d9f6c64ffc47fdd519727ab53cb71f9c40de24b64acde61f02fc7dce130b612fa5dbcac94573a2354fd005d8e9caefdc5fde48304474708bbd82f77e1fd2c630bea236f6f8dccc1678e000102030405060708090a0b0c0d0e0f0b28bec4698520eefbe176758a25b2bb4cbdef785c3e3b5c836ea044df03");
    }

    #[test]
    fn random_ephemeral_key_and_nonce_round_trip() {
        let payload = run(
            "encrypt",
            "hello ecies",
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "16",
            "",
            false,
            "ephemeral-and-point",
            "",
            "auto",
            "auto",
            "base64",
        )
        .unwrap();
        let second = run(
            "encrypt",
            "hello ecies",
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "16",
            "",
            false,
            "ephemeral-and-point",
            "",
            "auto",
            "auto",
            "base64",
        )
        .unwrap();
        assert_ne!(payload, second, "each run must use a fresh ephemeral key");
        let back = run(
            "decrypt",
            &payload,
            RECIPIENT_SK,
            "secp256k1",
            "aes-256-gcm",
            "16",
            "",
            false,
            "ephemeral-and-point",
            "",
            "auto",
            "auto",
            "base64",
        )
        .unwrap();
        assert_eq!(back, "hello ecies");
    }

    #[test]
    fn bare_xy_public_key_without_sec1_tag_is_accepted() {
        let bare = &K256_PUB[2..];
        let tagged = enc(
            "attack at dawn",
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "ephemeral-and-point",
            "hex",
        )
        .unwrap();
        let untagged = enc(
            "attack at dawn",
            bare,
            "secp256k1",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "ephemeral-and-point",
            "hex",
        )
        .unwrap();
        assert_eq!(tagged, untagged);
    }

    #[test]
    fn tampered_ciphertext_fails_authentication() {
        let payload = enc(
            "attack at dawn",
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "ephemeral-and-point",
            "hex",
        )
        .unwrap();
        let mut bytes = hex::decode(&payload).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;
        let err = run(
            "decrypt",
            &hex::encode(&bytes),
            RECIPIENT_SK,
            "secp256k1",
            "aes-256-gcm",
            "16",
            "",
            false,
            "ephemeral-and-point",
            "",
            "auto",
            "hex",
            "base64",
        )
        .unwrap_err();
        assert!(
            err.contains("authentication tag did not verify"),
            "got {err}"
        );
    }

    #[test]
    fn wrong_private_key_is_rejected() {
        let payload = enc(
            "attack at dawn",
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "ephemeral-and-point",
            "base64",
        )
        .unwrap();
        let err = run(
            "decrypt",
            &payload,
            "3333333333333333333333333333333333333333333333333333333333333333",
            "secp256k1",
            "aes-256-gcm",
            "16",
            "",
            false,
            "ephemeral-and-point",
            "",
            "auto",
            "auto",
            "base64",
        )
        .unwrap_err();
        assert!(
            err.contains("authentication tag did not verify"),
            "got {err}"
        );
    }

    #[test]
    fn errors_name_what_was_expected() {
        let e = run(
            "encrypt",
            "hi",
            "not-a-key!!",
            "secp256k1",
            "",
            "16",
            "",
            false,
            "",
            "",
            "auto",
            "auto",
            "base64",
        )
        .unwrap_err();
        assert!(e.contains("not a PEM block, hex, or base64"), "got {e}");

        let e = run(
            "encrypt",
            "hi",
            "0405",
            "secp256k1",
            "",
            "16",
            "",
            false,
            "",
            "",
            "hex",
            "auto",
            "base64",
        )
        .unwrap_err();
        assert!(e.contains("valid public key point"), "got {e}");

        let e = run(
            "encrypt",
            "hi",
            K256_PUB,
            "curve25519",
            "",
            "16",
            "",
            false,
            "",
            "",
            "auto",
            "auto",
            "base64",
        )
        .unwrap_err();
        assert!(e.contains("unknown curve 'curve25519'"), "got {e}");

        let e = run(
            "encrypt",
            "hi",
            K256_PUB,
            "secp256k1",
            "",
            "13",
            "",
            false,
            "",
            "",
            "auto",
            "auto",
            "base64",
        )
        .unwrap_err();
        assert!(e.contains("unknown nonce_length '13'"), "got {e}");

        let e = run(
            "encrypt",
            "hi",
            K256_PUB,
            "secp256k1",
            "",
            "16",
            "00",
            false,
            "",
            "",
            "auto",
            "auto",
            "base64",
        )
        .unwrap_err();
        assert!(e.contains("nonce must be 16 bytes"), "got {e}");

        let e = run(
            "decrypt",
            "AAEC",
            RECIPIENT_SK,
            "secp256k1",
            "",
            "16",
            "",
            false,
            "",
            "",
            "auto",
            "auto",
            "base64",
        )
        .unwrap_err();
        assert!(e.contains("SEC1 tag byte"), "got {e}");

        let e = run(
            "decrypt",
            "hello",
            RECIPIENT_SK,
            "secp256k1",
            "",
            "16",
            "",
            false,
            "",
            "",
            "auto",
            "text",
            "base64",
        )
        .unwrap_err();
        assert!(
            e.contains("data_encoding=text cannot describe an encrypted payload"),
            "got {e}"
        );
    }

    #[test]
    fn non_utf8_plaintext_comes_back_in_the_output_encoding() {
        let payload = run(
            "encrypt",
            "ff00ff",
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "ephemeral-and-point",
            EPHEMERAL_SK,
            "auto",
            "hex",
            "base64",
        )
        .unwrap();
        let back = run(
            "decrypt",
            &payload,
            RECIPIENT_SK,
            "secp256k1",
            "aes-256-gcm",
            "16",
            "",
            false,
            "ephemeral-and-point",
            "",
            "auto",
            "auto",
            "hex",
        )
        .unwrap();
        assert_eq!(back, "ff00ff");
    }

    #[test]
    fn oversized_input_is_rejected_at_the_boundary() {
        let ok = "a".repeat(MAX_INPUT_BYTES);
        assert!(enc(
            &ok,
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "ephemeral-and-point",
            "base64"
        )
        .is_ok());
        let too_big = "a".repeat(MAX_INPUT_BYTES + 1);
        let err = enc(
            &too_big,
            K256_PUB,
            "secp256k1",
            "aes-256-gcm",
            "16",
            NONCE16,
            false,
            "ephemeral-and-point",
            "base64",
        )
        .unwrap_err();
        assert!(err.contains("over the"), "got {err}");
    }
}
