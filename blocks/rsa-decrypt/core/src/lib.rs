//! gizza-ai/rsa-decrypt core — decrypt RSA ciphertext with a private key using
//! either OAEP (SHA-256/384/512) or PKCS#1 v1.5 padding. Accepts base64 or hex
//! ciphertext and PKCS#8 / PKCS#1 PEM keys (including passphrase-protected
//! PKCS#8). Pure-Rust (`rsa` + `sha2`). No wafer/wasm-bindgen deps.
//!
//! Inverse of the `rsa-encrypt` tool.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::{Oaep, Pkcs1v15Encrypt, RsaPrivateKey};
use sha2::{Sha256, Sha384, Sha512};

/// Padding scheme the ciphertext was produced with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Padding {
    Oaep,
    Pkcs1v15,
}

impl Padding {
    pub fn parse(s: &str) -> Result<Padding, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' ', '.'], "").as_str() {
            "oaep" | "" => Ok(Padding::Oaep),
            "pkcs1v15" | "pkcs1" => Ok(Padding::Pkcs1v15),
            other => Err(format!("unknown padding '{other}' (use 'oaep' or 'pkcs1v15')")),
        }
    }
}

/// Hash used by OAEP (the MGF1 + label digest). Ignored for PKCS#1 v1.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hash {
    Sha256,
    Sha384,
    Sha512,
}

impl Hash {
    pub fn parse(s: &str) -> Result<Hash, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "sha256" | "" => Ok(Hash::Sha256),
            "sha384" => Ok(Hash::Sha384),
            "sha512" => Ok(Hash::Sha512),
            other => Err(format!("unknown hash '{other}' (use 'sha256', 'sha384', or 'sha512')")),
        }
    }
}

/// How the ciphertext is encoded on the way in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherEncoding {
    Auto,
    Base64,
    Hex,
}

impl CipherEncoding {
    pub fn parse(s: &str) -> Result<CipherEncoding, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "auto" | "" => Ok(CipherEncoding::Auto),
            "base64" | "b64" => Ok(CipherEncoding::Base64),
            "hex" | "base16" => Ok(CipherEncoding::Hex),
            other => {
                Err(format!("unknown ciphertext_encoding '{other}' (use 'auto', 'base64', or 'hex')"))
            }
        }
    }
}

/// How the recovered plaintext bytes are rendered on the way out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputEncoding {
    Utf8,
    Hex,
    Base64,
}

impl OutputEncoding {
    pub fn parse(s: &str) -> Result<OutputEncoding, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "utf8" | "text" | "" => Ok(OutputEncoding::Utf8),
            "hex" | "base16" => Ok(OutputEncoding::Hex),
            "base64" | "b64" => Ok(OutputEncoding::Base64),
            other => {
                Err(format!("unknown output_encoding '{other}' (use 'utf8', 'hex', or 'base64')"))
            }
        }
    }
}

/// A successful decryption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decrypted {
    /// The plaintext, rendered per the requested output encoding.
    pub plaintext: String,
    /// Length of the recovered plaintext in BYTES (not characters).
    pub plaintext_bytes: usize,
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace() && *c != ':').collect();
    let cleaned = cleaned.strip_prefix("0x").or_else(|| cleaned.strip_prefix("0X")).map(String::from).unwrap_or(cleaned);
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "expected an even number of hex digits, got {} — is the ciphertext truncated?",
            cleaned.len()
        ));
    }
    let bytes = cleaned.as_bytes();
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = (pair[0] as char)
            .to_digit(16)
            .ok_or_else(|| format!("expected a hex digit, got '{}'", pair[0] as char))?;
        let lo = (pair[1] as char)
            .to_digit(16)
            .ok_or_else(|| format!("expected a hex digit, got '{}'", pair[1] as char))?;
        out.push((hi * 16 + lo) as u8);
    }
    Ok(out)
}

fn looks_like_hex(s: &str) -> bool {
    let cleaned: Vec<char> =
        s.chars().filter(|c| !c.is_whitespace() && *c != ':').collect();
    let cleaned: Vec<char> = if cleaned.starts_with(&['0', 'x']) || cleaned.starts_with(&['0', 'X'])
    {
        cleaned[2..].to_vec()
    } else {
        cleaned
    };
    !cleaned.is_empty() && cleaned.len() % 2 == 0 && cleaned.iter().all(|c| c.is_ascii_hexdigit())
}

fn decode_b64(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    // Tolerate URL-safe alphabets and missing padding, which real-world tools emit.
    let cleaned: String = cleaned.replace('-', "+").replace('_', "/");
    let cleaned = cleaned.trim_end_matches('=').to_string();
    base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(cleaned.as_bytes())
        .map_err(|e| format!("ciphertext is not valid base64: {e}"))
}

/// Decode the ciphertext string into raw bytes per `encoding`.
pub fn decode_ciphertext(ciphertext: &str, encoding: CipherEncoding) -> Result<Vec<u8>, String> {
    if ciphertext.trim().is_empty() {
        return Err("no ciphertext — paste the base64 (or hex) RSA ciphertext to decrypt".into());
    }
    match encoding {
        CipherEncoding::Hex => hex_decode(ciphertext),
        CipherEncoding::Base64 => decode_b64(ciphertext),
        CipherEncoding::Auto => {
            if looks_like_hex(ciphertext) {
                hex_decode(ciphertext)
            } else {
                decode_b64(ciphertext)
            }
        }
    }
}

/// Parse a PEM private key: PKCS#8, PKCS#1, or passphrase-protected PKCS#8.
pub fn parse_key(pem: &str, passphrase: &str) -> Result<RsaPrivateKey, String> {
    if !pem.contains("PRIVATE KEY") {
        return Err(
            "no RSA private key found — paste a PEM '-----BEGIN PRIVATE KEY-----' (PKCS#8), '-----BEGIN RSA PRIVATE KEY-----' (PKCS#1), or '-----BEGIN ENCRYPTED PRIVATE KEY-----' block"
                .into(),
        );
    }
    let encrypted = pem.contains("ENCRYPTED PRIVATE KEY");
    let legacy_encrypted = pem.contains("Proc-Type: 4,ENCRYPTED");

    if encrypted {
        if passphrase.is_empty() {
            return Err(
                "this key is an encrypted PKCS#8 key ('-----BEGIN ENCRYPTED PRIVATE KEY-----') — enter its passphrase to unlock it"
                    .into(),
            );
        }
        return RsaPrivateKey::from_pkcs8_encrypted_pem(pem, passphrase.as_bytes())
            .map_err(|e| format!("could not unlock the encrypted private key (wrong passphrase or unsupported cipher): {e}"));
    }
    if legacy_encrypted {
        return Err(
            "this is a legacy OpenSSL-encrypted PEM key (a 'Proc-Type: 4,ENCRYPTED' header) — convert it first with: openssl pkcs8 -topk8 -in key.pem -out key8.pem"
                .into(),
        );
    }

    // Try PKCS#8 first, then PKCS#1.
    RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
        .map_err(|e| format!("invalid RSA private key: {e}"))
}

/// Decrypt `ciphertext` with `private_key_pem` and render the plaintext.
pub fn decrypt(
    ciphertext: &str,
    private_key_pem: &str,
    passphrase: &str,
    padding: Padding,
    hash: Hash,
    cipher_encoding: CipherEncoding,
    output_encoding: OutputEncoding,
) -> Result<Decrypted, String> {
    let key = parse_key(private_key_pem, passphrase)?;
    let ct = decode_ciphertext(ciphertext, cipher_encoding)?;

    let key_size = rsa::traits::PublicKeyParts::size(&key);
    if ct.len() != key_size {
        return Err(format!(
            "ciphertext is {} bytes but this {}-bit key expects exactly {} bytes — check that the ciphertext is complete and was encrypted to this key",
            ct.len(),
            key_size * 8,
            key_size
        ));
    }

    let plain: Vec<u8> = match padding {
        Padding::Pkcs1v15 => key.decrypt(Pkcs1v15Encrypt, &ct).map_err(|e| {
            format!("decryption failed (wrong key, or the ciphertext was not PKCS#1 v1.5 padded): {e}")
        })?,
        Padding::Oaep => {
            let pad = match hash {
                Hash::Sha256 => Oaep::new::<Sha256>(),
                Hash::Sha384 => Oaep::new::<Sha384>(),
                Hash::Sha512 => Oaep::new::<Sha512>(),
            };
            key.decrypt(pad, &ct).map_err(|e| {
                format!("decryption failed (wrong key, wrong padding, or a different OAEP hash than the one selected): {e}")
            })?
        }
    };

    let plaintext_bytes = plain.len();
    let plaintext = match output_encoding {
        OutputEncoding::Hex => hex_encode(&plain),
        OutputEncoding::Base64 => B64.encode(&plain),
        OutputEncoding::Utf8 => String::from_utf8(plain).map_err(|_| {
            "the decrypted bytes are not valid UTF-8 text — set output_encoding to 'hex' or 'base64' to see the raw bytes"
                .to_string()
        })?,
    };

    Ok(Decrypted { plaintext, plaintext_bytes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs8::LineEnding;
    use rsa::rand_core::OsRng;
    use rsa::RsaPublicKey;

    /// A fixed throwaway RSA-2048 key so tests are fast and deterministic.
    const TEST_KEY: &str = include_str!("../tests/test-key.pem");

    fn key() -> RsaPrivateKey {
        parse_key(TEST_KEY, "").unwrap()
    }

    fn encrypt_oaep(msg: &[u8], hash: Hash) -> String {
        let pk = RsaPublicKey::from(&key());
        let pad = match hash {
            Hash::Sha256 => Oaep::new::<Sha256>(),
            Hash::Sha384 => Oaep::new::<Sha384>(),
            Hash::Sha512 => Oaep::new::<Sha512>(),
        };
        B64.encode(pk.encrypt(&mut OsRng, pad, msg).unwrap())
    }

    #[test]
    fn oaep_sha256_roundtrip() {
        let ct = encrypt_oaep(b"hello world", Hash::Sha256);
        let got = decrypt(
            &ct,
            TEST_KEY,
            "",
            Padding::Oaep,
            Hash::Sha256,
            CipherEncoding::Auto,
            OutputEncoding::Utf8,
        )
        .unwrap();
        assert_eq!(got.plaintext, "hello world");
        assert_eq!(got.plaintext_bytes, 11);
    }

    #[test]
    fn oaep_sha512_roundtrip() {
        let ct = encrypt_oaep(b"longer hash", Hash::Sha512);
        let got = decrypt(
            &ct,
            TEST_KEY,
            "",
            Padding::Oaep,
            Hash::Sha512,
            CipherEncoding::Auto,
            OutputEncoding::Utf8,
        )
        .unwrap();
        assert_eq!(got.plaintext, "longer hash");
    }

    #[test]
    fn pkcs1v15_roundtrip() {
        let pk = RsaPublicKey::from(&key());
        let ct = B64.encode(pk.encrypt(&mut OsRng, Pkcs1v15Encrypt, b"secret payload").unwrap());
        let got = decrypt(
            &ct,
            TEST_KEY,
            "",
            Padding::Pkcs1v15,
            Hash::Sha256,
            CipherEncoding::Auto,
            OutputEncoding::Utf8,
        )
        .unwrap();
        assert_eq!(got.plaintext, "secret payload");
    }

    #[test]
    fn hex_ciphertext_and_hex_output() {
        let pk = RsaPublicKey::from(&key());
        let raw = pk.encrypt(&mut OsRng, Oaep::new::<Sha256>(), &[0x00, 0xff, 0x10]).unwrap();
        let hex = hex_encode(&raw);
        let got = decrypt(
            &hex,
            TEST_KEY,
            "",
            Padding::Oaep,
            Hash::Sha256,
            CipherEncoding::Auto,
            OutputEncoding::Hex,
        )
        .unwrap();
        assert_eq!(got.plaintext, "00ff10");
        assert_eq!(got.plaintext_bytes, 3);
        // Base64 rendering of the same non-UTF-8 bytes.
        let got_b64 = decrypt(
            &hex,
            TEST_KEY,
            "",
            Padding::Oaep,
            Hash::Sha256,
            CipherEncoding::Hex,
            OutputEncoding::Base64,
        )
        .unwrap();
        assert_eq!(got_b64.plaintext, B64.encode([0x00u8, 0xff, 0x10]));
    }

    #[test]
    fn non_utf8_plaintext_errors_with_guidance() {
        let pk = RsaPublicKey::from(&key());
        let ct = B64.encode(pk.encrypt(&mut OsRng, Oaep::new::<Sha256>(), &[0xff, 0xfe]).unwrap());
        let err = decrypt(
            &ct,
            TEST_KEY,
            "",
            Padding::Oaep,
            Hash::Sha256,
            CipherEncoding::Auto,
            OutputEncoding::Utf8,
        )
        .unwrap_err();
        assert!(err.contains("not valid UTF-8"), "{err}");
    }

    #[test]
    fn url_safe_unpadded_base64_is_accepted() {
        let pk = RsaPublicKey::from(&key());
        let raw = pk.encrypt(&mut OsRng, Oaep::new::<Sha256>(), b"urlsafe").unwrap();
        let url = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&raw);
        let got = decrypt(
            &url,
            TEST_KEY,
            "",
            Padding::Oaep,
            Hash::Sha256,
            CipherEncoding::Base64,
            OutputEncoding::Utf8,
        )
        .unwrap();
        assert_eq!(got.plaintext, "urlsafe");
    }

    #[test]
    fn wrong_hash_fails_cleanly() {
        let ct = encrypt_oaep(b"hello", Hash::Sha256);
        let err = decrypt(
            &ct,
            TEST_KEY,
            "",
            Padding::Oaep,
            Hash::Sha512,
            CipherEncoding::Auto,
            OutputEncoding::Utf8,
        )
        .unwrap_err();
        assert!(err.contains("decryption failed"), "{err}");
    }

    #[test]
    fn wrong_length_ciphertext_is_explained() {
        let err = decrypt(
            &B64.encode([1u8, 2, 3]),
            TEST_KEY,
            "",
            Padding::Oaep,
            Hash::Sha256,
            CipherEncoding::Base64,
            OutputEncoding::Utf8,
        )
        .unwrap_err();
        assert!(err.contains("expects exactly 256 bytes"), "{err}");
    }

    #[test]
    fn encrypted_pkcs8_key_needs_a_passphrase() {
        // Same key as TEST_KEY, wrapped by `openssl pkcs8 -topk8` (PBES2:
        // PBKDF2-HMAC-SHA256 + AES-256-CBC) with the passphrase "hunter2".
        let enc_pem = include_str!("../tests/test-key-encrypted.pem");
        let ct = encrypt_oaep(b"locked key", Hash::Sha256);

        let got = decrypt(
            &ct,
            enc_pem,
            "hunter2",
            Padding::Oaep,
            Hash::Sha256,
            CipherEncoding::Auto,
            OutputEncoding::Utf8,
        )
        .unwrap();
        assert_eq!(got.plaintext, "locked key");

        let missing = decrypt(
            &ct,
            enc_pem,
            "",
            Padding::Oaep,
            Hash::Sha256,
            CipherEncoding::Auto,
            OutputEncoding::Utf8,
        )
        .unwrap_err();
        assert!(missing.contains("passphrase"), "{missing}");

        let wrong = decrypt(
            &ct,
            enc_pem,
            "nope",
            Padding::Oaep,
            Hash::Sha256,
            CipherEncoding::Auto,
            OutputEncoding::Utf8,
        )
        .unwrap_err();
        assert!(wrong.contains("could not unlock"), "{wrong}");
    }

    #[test]
    fn pkcs1_pem_keys_are_accepted() {
        let k = key();
        let pkcs1 = rsa::pkcs1::EncodeRsaPrivateKey::to_pkcs1_pem(&k, LineEnding::LF).unwrap();
        let ct = encrypt_oaep(b"pkcs1 key", Hash::Sha256);
        let got = decrypt(
            &ct,
            &pkcs1,
            "",
            Padding::Oaep,
            Hash::Sha256,
            CipherEncoding::Auto,
            OutputEncoding::Utf8,
        )
        .unwrap();
        assert_eq!(got.plaintext, "pkcs1 key");
    }

    #[test]
    fn errors() {
        assert!(decrypt(
            "AAAA",
            "not a key",
            "",
            Padding::Oaep,
            Hash::Sha256,
            CipherEncoding::Auto,
            OutputEncoding::Utf8
        )
        .unwrap_err()
        .contains("no RSA private key found"));
        assert!(decode_ciphertext("   ", CipherEncoding::Auto).unwrap_err().contains("no ciphertext"));
        assert!(decode_ciphertext("zz!!", CipherEncoding::Base64).is_err());
        assert!(decode_ciphertext("abc", CipherEncoding::Hex).unwrap_err().contains("even number"));
        assert!(Padding::parse("nope").is_err());
        assert!(Hash::parse("md5").is_err());
        assert!(CipherEncoding::parse("rot13").is_err());
        assert!(OutputEncoding::parse("binary").is_err());
        assert_eq!(Padding::parse("PKCS1-v1.5").unwrap(), Padding::Pkcs1v15);
        assert_eq!(Hash::parse("sha-384").unwrap(), Hash::Sha384);
        assert_eq!(CipherEncoding::parse("HEX").unwrap(), CipherEncoding::Hex);
        assert_eq!(OutputEncoding::parse("UTF-8").unwrap(), OutputEncoding::Utf8);
    }
}
