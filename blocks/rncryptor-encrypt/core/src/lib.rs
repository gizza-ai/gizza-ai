//! gizza-ai/rncryptor-encrypt core — the password-based RNCryptor **v3**
//! container format. Pure compute, shared by the chat skill block, the CLI and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Container layout (all fields concatenated, no separators):
//!
//! ```text
//! version (1 = 0x03) | options (1 = 0x01, password) | encryption salt (8)
//!   | HMAC salt (8) | IV (16) | ciphertext (n*16) | HMAC-SHA256 (32)
//! ```
//!
//! The cryptographic parameters are pinned by the format and are deliberately
//! NOT configurable — changing any of them produces a blob no RNCryptor
//! implementation can open:
//!   * encryption key = PBKDF2-HMAC-SHA1(password, encryption salt, 10 000) → 32 bytes
//!   * HMAC key       = PBKDF2-HMAC-SHA1(password, HMAC salt, 10 000)       → 32 bytes
//!   * ciphertext     = AES-256-CBC with PKCS#7 padding under the encryption key + IV
//!   * HMAC           = HMAC-SHA256 over the whole message *before* the tag
//!
//! Salts and the IV are random per run unless supplied explicitly (hex), which
//! makes a run reproducible against the published spec test vectors.

use aes::Aes256;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use cbc::{Decryptor, Encryptor};
use cipher::block_padding::Pkcs7;
use cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// RNCryptor data-format version this tool reads and writes.
pub const VERSION: u8 = 3;
/// Options byte for the password-based variant (bit 0 = "uses password").
pub const OPTIONS_PASSWORD: u8 = 1;
/// PBKDF2 iteration count fixed by the v3 spec.
pub const PBKDF2_ITERATIONS: u32 = 10_000;
const SALT_LEN: usize = 8;
const IV_LEN: usize = 16;
const KEY_LEN: usize = 32;
const HMAC_LEN: usize = 32;
const HEADER_LEN: usize = 2 + SALT_LEN + SALT_LEN + IV_LEN; // 34
/// Smallest possible container: header + one padded block + HMAC.
const MIN_CONTAINER_LEN: usize = HEADER_LEN + 16 + HMAC_LEN; // 82
/// Cap on decoded input bytes, so a paste can't OOM the 64 MiB wasm sandbox.
pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;

type Aes256CbcEnc = Encryptor<Aes256>;
type Aes256CbcDec = Decryptor<Aes256>;

/// PBKDF2-HMAC-SHA1 with the spec's fixed iteration count → a 32-byte key.
pub fn derive_key(password: &str, salt: &[u8]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    // Generic form avoids pbkdf2's optional `pbkdf2_hmac` convenience feature.
    pbkdf2::pbkdf2::<Hmac<Sha1>>(
        password.as_bytes(),
        salt,
        PBKDF2_ITERATIONS,
        &mut key,
    )
    .expect("HMAC accepts any key length");
    key
}

/// Build a v3 password container from raw bytes and explicit salts/IV.
pub fn encrypt_with(
    plaintext: &[u8],
    password: &str,
    encryption_salt: &[u8],
    hmac_salt: &[u8],
    iv: &[u8],
) -> Result<Vec<u8>, String> {
    if password.is_empty() {
        return Err("password is required (RNCryptor derives both keys from it)".into());
    }
    if plaintext.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is too large: {} bytes, maximum is {} bytes ({} MiB)",
            plaintext.len(),
            MAX_INPUT_BYTES,
            MAX_INPUT_BYTES / (1024 * 1024)
        ));
    }
    check_len("encryption_salt", encryption_salt, SALT_LEN)?;
    check_len("hmac_salt", hmac_salt, SALT_LEN)?;
    check_len("iv", iv, IV_LEN)?;

    let enc_key = derive_key(password, encryption_salt);
    let hmac_key = derive_key(password, hmac_salt);

    let mut out = Vec::with_capacity(HEADER_LEN + plaintext.len() + 16 + HMAC_LEN);
    out.push(VERSION);
    out.push(OPTIONS_PASSWORD);
    out.extend_from_slice(encryption_salt);
    out.extend_from_slice(hmac_salt);
    out.extend_from_slice(iv);

    let ciphertext = Aes256CbcEnc::new(enc_key.as_slice().into(), iv.into())
        .encrypt_padded_vec_mut::<Pkcs7>(plaintext);
    out.extend_from_slice(&ciphertext);

    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&hmac_key)
        .expect("HMAC accepts any key length");
    mac.update(&out);
    out.extend_from_slice(&mac.finalize().into_bytes());
    Ok(out)
}

/// Verify and open a v3 password container, returning the plaintext bytes.
pub fn decrypt_container(container: &[u8], password: &str) -> Result<Vec<u8>, String> {
    if password.is_empty() {
        return Err("password is required (RNCryptor derives both keys from it)".into());
    }
    if container.len() < MIN_CONTAINER_LEN {
        return Err(format!(
            "container is too short: {} bytes, expected at least {MIN_CONTAINER_LEN} bytes \
             (2-byte header + 8-byte encryption salt + 8-byte HMAC salt + 16-byte IV + \
             one 16-byte block + 32-byte HMAC)",
            container.len()
        ));
    }
    if container[0] != VERSION {
        return Err(format!(
            "unsupported RNCryptor version {}: this tool reads and writes version {VERSION} only",
            container[0]
        ));
    }
    if container[1] != OPTIONS_PASSWORD {
        return Err(format!(
            "options byte is 0x{:02x}: this tool handles the password-based variant (0x01) only, \
             not key-based containers",
            container[1]
        ));
    }
    let body_len = container.len() - HMAC_LEN;
    let ciphertext = &container[HEADER_LEN..body_len];
    if ciphertext.len() % 16 != 0 {
        return Err(format!(
            "ciphertext length {} is not a multiple of the 16-byte AES block size — \
             the container is truncated or corrupted",
            ciphertext.len()
        ));
    }
    let encryption_salt = &container[2..2 + SALT_LEN];
    let hmac_salt = &container[2 + SALT_LEN..2 + 2 * SALT_LEN];
    let iv = &container[2 + 2 * SALT_LEN..HEADER_LEN];

    let hmac_key = derive_key(password, hmac_salt);
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&hmac_key)
        .expect("HMAC accepts any key length");
    mac.update(&container[..body_len]);
    let expected = mac.finalize().into_bytes();
    // Constant-time compare: never leak how much of the tag matched.
    if expected.ct_eq(&container[body_len..]).unwrap_u8() != 1 {
        return Err(
            "HMAC verification failed: wrong password, or the container was modified in transit"
                .into(),
        );
    }

    let enc_key = derive_key(password, encryption_salt);
    Aes256CbcDec::new(enc_key.as_slice().into(), iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
        .map_err(|_| "PKCS#7 padding is invalid after decryption — the container is corrupted".into())
}

fn check_len(what: &str, got: &[u8], want: usize) -> Result<(), String> {
    if got.len() != want {
        return Err(format!(
            "{what} must be exactly {want} bytes ({} hex characters), got {}",
            want * 2,
            got.len()
        ));
    }
    Ok(())
}

fn decode_hex(what: &str, s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s
        .trim()
        .trim_start_matches("0x")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    hex::decode(&cleaned).map_err(|e| format!("{what} is not valid hex: {e}"))
}

fn decode_base64(what: &str, s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    B64.decode(cleaned.as_bytes())
        .map_err(|e| format!("{what} is not valid base64: {e}"))
}

fn random_bytes(n: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).map_err(|e| format!("secure random number generator unavailable: {e}"))?;
    Ok(buf)
}

/// `explicit` empty → fresh random bytes; otherwise parse it as hex.
fn salt_or_random(what: &str, explicit: &str, len: usize) -> Result<Vec<u8>, String> {
    if explicit.trim().is_empty() {
        return random_bytes(len);
    }
    let bytes = decode_hex(what, explicit)?;
    check_len(what, &bytes, len)?;
    Ok(bytes)
}

fn looks_like_hex(s: &str) -> bool {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    !cleaned.is_empty()
        && cleaned.len() % 2 == 0
        && cleaned.chars().all(|c| c.is_ascii_hexdigit())
}

/// Decode `data` for encryption according to `data_encoding`.
fn decode_input(data: &str, data_encoding: &str) -> Result<Vec<u8>, String> {
    match data_encoding {
        "" | "text" => Ok(data.as_bytes().to_vec()),
        "hex" => decode_hex("data", data),
        "base64" => decode_base64("data", data),
        other => Err(format!(
            "unknown data_encoding {other:?}: expected text, hex, or base64"
        )),
    }
}

/// Decode a container for decryption. `text` means auto-detect hex vs base64.
fn decode_container_text(data: &str, data_encoding: &str) -> Result<Vec<u8>, String> {
    match data_encoding {
        "hex" => decode_hex("data", data),
        "base64" => decode_base64("data", data),
        "" | "text" => {
            if looks_like_hex(data) {
                decode_hex("data", data)
            } else {
                decode_base64("data", data)
            }
        }
        other => Err(format!(
            "unknown data_encoding {other:?}: expected text, hex, or base64"
        )),
    }
}

fn encode_output(bytes: &[u8], output_encoding: &str) -> Result<String, String> {
    match output_encoding {
        "" | "base64" => Ok(B64.encode(bytes)),
        "hex" => Ok(hex::encode(bytes)),
        other => Err(format!(
            "unknown output_encoding {other:?}: expected base64 or hex"
        )),
    }
}

/// The single entry point shared by every surface.
///
/// * `operation` — `encrypt` (default) or `decrypt`
/// * `data` — plaintext to seal, or a container to open
/// * `password` — the passphrase both keys are derived from
/// * `data_encoding` — `text` | `hex` | `base64` (for decrypt, `text` auto-detects)
/// * `output_encoding` — `base64` (default) | `hex`
/// * `encryption_salt` / `hmac_salt` / `iv` — optional hex overrides for reproducible output
#[allow(clippy::too_many_arguments)]
pub fn run(
    operation: &str,
    data: &str,
    password: &str,
    data_encoding: &str,
    output_encoding: &str,
    encryption_salt: &str,
    hmac_salt: &str,
    iv: &str,
) -> Result<String, String> {
    match operation {
        "" | "encrypt" => {
            let plaintext = decode_input(data, data_encoding)?;
            let enc_salt = salt_or_random("encryption_salt", encryption_salt, SALT_LEN)?;
            let mac_salt = salt_or_random("hmac_salt", hmac_salt, SALT_LEN)?;
            let iv_bytes = salt_or_random("iv", iv, IV_LEN)?;
            let container = encrypt_with(&plaintext, password, &enc_salt, &mac_salt, &iv_bytes)?;
            encode_output(&container, output_encoding)
        }
        "decrypt" => {
            let container = decode_container_text(data, data_encoding)?;
            if container.len() > MAX_INPUT_BYTES {
                return Err(format!(
                    "container is too large: {} bytes, maximum is {} bytes ({} MiB)",
                    container.len(),
                    MAX_INPUT_BYTES,
                    MAX_INPUT_BYTES / (1024 * 1024)
                ));
            }
            let plaintext = decrypt_container(&container, password)?;
            if output_encoding == "hex" {
                return encode_output(&plaintext, output_encoding);
            }
            match String::from_utf8(plaintext.clone()) {
                Ok(text) if !text.chars().any(char::is_control) => Ok(text),
                Ok(_) | Err(_) => encode_output(&plaintext, output_encoding),
            }
        }
        other => Err(format!(
            "unknown operation {other:?}: expected encrypt or decrypt"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> Vec<u8> {
        hex::decode(s).unwrap()
    }

    /// Spec vector: all fields zero, one-byte password, empty plaintext.
    #[test]
    fn spec_vector_all_zero_fields() {
        let out = encrypt_with(
            &[],
            "a",
            &h("0000000000000000"),
            &h("0000000000000000"),
            &h("00000000000000000000000000000000"),
        )
        .unwrap();
        assert_eq!(
            hex::encode(&out),
            concat!(
                "0301",
                "0000000000000000",
                "0000000000000000",
                "00000000000000000000000000000000",
                "b3039be31cd7ece5e754f5c8da170036",
                "66313ae8a89ddcf8e3cb41fdc130b2329dbe07d6f4d32c34e050c8bd7e933b12",
            )
        );
    }

    /// Spec vector: one plaintext byte.
    #[test]
    fn spec_vector_one_byte() {
        let out = encrypt_with(
            &h("01"),
            "thepassword",
            &h("0001020304050607"),
            &h("0102030405060708"),
            &h("02030405060708090a0b0c0d0e0f0001"),
        )
        .unwrap();
        assert_eq!(
            hex::encode(&out),
            concat!(
                "0301",
                "0001020304050607",
                "0102030405060708",
                "02030405060708090a0b0c0d0e0f0001",
                "a1f8730e0bf480eb7b70f690abf21e02",
                "9514164ad3c474a51b30c7eaa1ca545b7de3de5b010acbad0a9a13857df696a8",
            )
        );
    }

    /// Spec vector: exactly one AES block of plaintext (16 bytes in → two
    /// blocks out, because PKCS#7 always appends a full block of padding).
    #[test]
    fn spec_vector_exactly_one_block() {
        let out = encrypt_with(
            &h("0123456789abcdef0123456789abcdef"),
            "thepassword",
            &h("0102030405060700"),
            &h("0203040506070801"),
            &h("030405060708090a0b0c0d0e0f000102"),
        )
        .unwrap();
        assert_eq!(
            hex::encode(&out),
            concat!(
                "0301",
                "0102030405060700",
                "0203040506070801",
                "030405060708090a0b0c0d0e0f000102",
                "c9314e428d33fcce5336854d698ea0fdf49547a3a0709c754649ac4113f5dedb",
                "10b5dd8f5ee8c7e8d370b77c5d67b7cfd09a72141bfbe2797de8d503e174b5a6",
            )
        );
    }

    /// Spec vector: multibyte (non-ASCII) password.
    #[test]
    fn spec_vector_multibyte_password() {
        let out = encrypt_with(
            &h("23456789abcdef0123456701"),
            "中文密码",
            &h("0304050607000102"),
            &h("0405060708010203"),
            &h("05060708090a0b0c0d0e0f0001020304"),
        )
        .unwrap();
        assert_eq!(
            hex::encode(&out),
            concat!(
                "0301",
                "0304050607000102",
                "0405060708010203",
                "05060708090a0b0c0d0e0f0001020304",
                "8a9e08bdec1c4bfe13e81fb85f009ab3",
                "ddb91387e809c4ad86d9e8a6014557716657bd317d4bb6a7644615b3de402341",
            )
        );
    }

    /// Spec KDF vectors: PBKDF2-HMAC-SHA1, 10 000 iterations, 32-byte key.
    #[test]
    fn spec_kdf_vectors() {
        assert_eq!(
            hex::encode(derive_key("a", &h("0102030405060708"))),
            "fc632b0ca6b23eff9a9dc3e0e585167f5a328916ed19f83558be3ba9828797cd"
        );
        assert_eq!(
            hex::encode(derive_key("thepassword", &h("0203040506070801"))),
            "0ea84f5252310dc3e3a7607c33bfd1eb580805fb68293005da21037ccf499626"
        );
        assert_eq!(
            hex::encode(derive_key("中文密码", &h("0506070801020304"))),
            "d2fc3237d4a69668ca83d969c2cda1ac6c3684792b6644b1a90b2052007215dd"
        );
    }

    #[test]
    fn round_trip_random_salts_through_run() {
        let container = run("encrypt", "attack at dawn", "hunter2", "text", "base64", "", "", "").unwrap();
        let back = run("decrypt", &container, "hunter2", "base64", "base64", "", "", "").unwrap();
        assert_eq!(back, "attack at dawn");
    }

    #[test]
    fn random_salts_differ_between_runs() {
        let a = run("encrypt", "same input", "pw", "text", "base64", "", "", "").unwrap();
        let b = run("encrypt", "same input", "pw", "text", "base64", "", "", "").unwrap();
        assert_ne!(a, b, "each run must use fresh random salts and IV");
    }

    #[test]
    fn decrypt_auto_detects_hex_and_base64() {
        let hex_container = run(
            "encrypt", "auto detect me", "pw", "text", "hex", "0001020304050607", "0102030405060708",
            "02030405060708090a0b0c0d0e0f0001",
        )
        .unwrap();
        assert_eq!(
            run("decrypt", &hex_container, "pw", "text", "base64", "", "", "").unwrap(),
            "auto detect me"
        );
        let b64_container = run(
            "encrypt", "auto detect me", "pw", "text", "base64", "0001020304050607", "0102030405060708",
            "02030405060708090a0b0c0d0e0f0001",
        )
        .unwrap();
        assert_eq!(
            run("decrypt", &b64_container, "pw", "text", "base64", "", "", "").unwrap(),
            "auto detect me"
        );
    }

    #[test]
    fn hex_input_encoding_is_binary_safe() {
        let container = run("encrypt", "00ff10", "pw", "hex", "base64", "", "", "").unwrap();
        // Non-UTF-8 plaintext comes back in the chosen output encoding.
        let back = run("decrypt", &container, "pw", "base64", "hex", "", "", "").unwrap();
        assert_eq!(back, "00ff10");
    }

    #[test]
    fn wrong_password_fails_hmac() {
        let container = run("encrypt", "secret", "right", "text", "base64", "", "", "").unwrap();
        let err = run("decrypt", &container, "wrong", "base64", "base64", "", "", "").unwrap_err();
        assert!(err.contains("HMAC verification failed"), "got {err}");
    }

    #[test]
    fn tampered_container_fails_hmac() {
        let container = run(
            "encrypt", "secret", "pw", "text", "hex", "0001020304050607", "0102030405060708",
            "02030405060708090a0b0c0d0e0f0001",
        )
        .unwrap();
        let mut bytes = hex::decode(&container).unwrap();
        let last = bytes.len() - 40;
        bytes[last] ^= 0x01;
        let err = run("decrypt", &hex::encode(&bytes), "pw", "hex", "base64", "", "", "").unwrap_err();
        assert!(err.contains("HMAC verification failed"), "got {err}");
    }

    #[test]
    fn empty_password_is_rejected() {
        let err = run("encrypt", "data", "", "text", "base64", "", "", "").unwrap_err();
        assert!(err.contains("password is required"), "got {err}");
    }

    #[test]
    fn bad_salt_length_is_rejected() {
        let err = run("encrypt", "data", "pw", "text", "base64", "0102", "", "").unwrap_err();
        assert!(
            err.contains("encryption_salt must be exactly 8 bytes"),
            "got {err}"
        );
    }

    #[test]
    fn bad_iv_length_is_rejected() {
        let err = run("encrypt", "data", "pw", "text", "base64", "", "", "0102030405060708").unwrap_err();
        assert!(err.contains("iv must be exactly 16 bytes"), "got {err}");
    }

    #[test]
    fn non_hex_salt_is_rejected() {
        let err = run("encrypt", "data", "pw", "text", "base64", "zzzzzzzzzzzzzzzz", "", "").unwrap_err();
        assert!(err.contains("encryption_salt is not valid hex"), "got {err}");
    }

    #[test]
    fn short_container_is_rejected() {
        let err = run("decrypt", "AAAA", "pw", "base64", "base64", "", "", "").unwrap_err();
        assert!(err.contains("container is too short"), "got {err}");
    }

    #[test]
    fn wrong_version_byte_is_rejected() {
        let container = run("encrypt", "hi", "pw", "text", "hex", "", "", "").unwrap();
        let mut bytes = hex::decode(&container).unwrap();
        bytes[0] = 2;
        let err = run("decrypt", &hex::encode(&bytes), "pw", "hex", "base64", "", "", "").unwrap_err();
        assert!(err.contains("unsupported RNCryptor version 2"), "got {err}");
    }

    #[test]
    fn key_based_options_byte_is_rejected() {
        let container = run("encrypt", "hi", "pw", "text", "hex", "", "", "").unwrap();
        let mut bytes = hex::decode(&container).unwrap();
        bytes[1] = 0;
        let err = run("decrypt", &hex::encode(&bytes), "pw", "hex", "base64", "", "", "").unwrap_err();
        assert!(err.contains("password-based variant"), "got {err}");
    }

    #[test]
    fn unknown_operation_is_rejected() {
        let err = run("sign", "data", "pw", "text", "base64", "", "", "").unwrap_err();
        assert!(err.contains("unknown operation"), "got {err}");
    }

    /// The published container from `spec_vector_one_byte` must open with the
    /// same password — interop is verified in both directions, not just on write.
    #[test]
    fn spec_vector_container_decrypts() {
        let container = concat!(
            "0301",
            "0001020304050607",
            "0102030405060708",
            "02030405060708090a0b0c0d0e0f0001",
            "a1f8730e0bf480eb7b70f690abf21e02",
            "9514164ad3c474a51b30c7eaa1ca545b7de3de5b010acbad0a9a13857df696a8",
        );
        assert_eq!(
            run("decrypt", container, "thepassword", "hex", "hex", "", "", "").unwrap(),
            "01"
        );
    }

    #[test]
    fn input_over_the_cap_is_rejected() {
        let too_big = "a".repeat(MAX_INPUT_BYTES + 1);
        let err = run("encrypt", &too_big, "pw", "text", "base64", "", "", "").unwrap_err();
        assert!(err.contains("input is too large"), "got {err}");
        // One byte under the cap still encrypts.
        let ok = "a".repeat(MAX_INPUT_BYTES);
        assert!(run("encrypt", &ok, "pw", "text", "base64", "", "", "").is_ok());
    }

    #[test]
    fn unknown_encodings_are_rejected() {
        let err = run("encrypt", "data", "pw", "base32", "base64", "", "", "").unwrap_err();
        assert!(err.contains("unknown data_encoding"), "got {err}");
        let err = run("encrypt", "data", "pw", "text", "base32", "", "", "").unwrap_err();
        assert!(err.contains("unknown output_encoding"), "got {err}");
    }
}
