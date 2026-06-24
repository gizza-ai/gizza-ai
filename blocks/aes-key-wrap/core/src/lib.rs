//! gizza-ai/aes-key-wrap core — wrap or unwrap cryptographic key material with the
//! AES Key Wrap algorithm. Supports KW (NIST SP 800-38F / RFC 3394, the classic
//! 8-byte-multiple wrap) and KWP (RFC 5649, "with padding", any byte length ≥ 1).
//! Pure-Rust (RustCrypto `aes-kw`). No wafer/wasm-bindgen deps.
//!
//! You supply a Key-Encryption-Key (KEK) — 16/24/32 bytes selecting AES-128/192/256 —
//! and the key material to protect. Wrapping produces ciphertext 8 bytes longer than
//! the (padded) input; unwrapping reverses it and verifies the integrity check.

use aes_kw::{KeyInit, KwAes128, KwAes192, KwAes256, KwpAes128, KwpAes192, KwpAes256};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

const IV_LEN: usize = 8;

/// Key Wrap algorithm variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// RFC 3394 / NIST SP 800-38F KW — input must be a multiple of 8 bytes, ≥ 16 bytes.
    Kw,
    /// RFC 5649 KWP — "with padding"; accepts any input length ≥ 1 byte.
    Kwp,
}

impl Algorithm {
    pub fn parse(s: &str) -> Result<Algorithm, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "kw" | "rfc3394" | "" => Ok(Algorithm::Kw),
            "kwp" | "rfc5649" => Ok(Algorithm::Kwp),
            other => Err(format!("unknown algorithm '{other}' (use kw or kwp)")),
        }
    }
}

/// Binary encoding for the KEK, the key material, and the wrapped output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Hex,
    Base64,
}

impl Encoding {
    pub fn parse(s: &str) -> Result<Encoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "base64" | "b64" | "" => Ok(Encoding::Base64),
            "hex" => Ok(Encoding::Hex),
            other => Err(format!("unknown format '{other}' (use hex or base64)")),
        }
    }
    fn decode(self, s: &str) -> Result<Vec<u8>, String> {
        let s = s.trim();
        match self {
            Encoding::Hex => hex::decode(s).map_err(|e| format!("invalid hex: {e}")),
            Encoding::Base64 => B64.decode(s).map_err(|e| format!("invalid base64: {e}")),
        }
    }
    fn encode(self, b: &[u8]) -> String {
        match self {
            Encoding::Hex => hex::encode(b),
            Encoding::Base64 => B64.encode(b),
        }
    }
}

const INTEGRITY_ERR: &str =
    "unwrap failed: integrity check failed (wrong KEK, algorithm, or corrupted data)";

/// Wrap `key_material` under `kek`. Returns the wrapped blob, encoded with `fmt`.
pub fn wrap(
    key_material: &str,
    kek: &str,
    algorithm: Algorithm,
    fmt: Encoding,
) -> Result<String, String> {
    let kek = fmt.decode(kek)?;
    let data = fmt.decode(key_material)?;

    let out: Vec<u8> = match algorithm {
        Algorithm::Kw => {
            if data.is_empty() || data.len() % IV_LEN != 0 {
                return Err(format!(
                    "KW requires key material that is a non-empty multiple of 8 bytes (got {} bytes); use kwp for arbitrary lengths",
                    data.len()
                ));
            }
            if data.len() < 16 {
                return Err(format!(
                    "KW requires at least 16 bytes of key material (got {}); use kwp for shorter keys",
                    data.len()
                ));
            }
            let mut buf = vec![0u8; data.len() + IV_LEN];
            match kek.len() {
                16 => kw_new::<KwAes128>(&kek)?.wrap_key(&data, &mut buf),
                24 => kw_new::<KwAes192>(&kek)?.wrap_key(&data, &mut buf),
                32 => kw_new::<KwAes256>(&kek)?.wrap_key(&data, &mut buf),
                n => return Err(bad_kek(n)),
            }
            .map_err(|e| format!("wrap failed: {e}"))?;
            buf
        }
        Algorithm::Kwp => {
            if data.is_empty() {
                return Err("nothing to wrap: key material is empty".to_string());
            }
            // KWP output = smallest multiple of 8 that is ≥ 8 longer than input.
            let semiblocks = data.len().div_ceil(IV_LEN);
            let mut buf = vec![0u8; semiblocks * IV_LEN + IV_LEN];
            match kek.len() {
                16 => kw_new::<KwpAes128>(&kek)?.wrap_key(&data, &mut buf),
                24 => kw_new::<KwpAes192>(&kek)?.wrap_key(&data, &mut buf),
                32 => kw_new::<KwpAes256>(&kek)?.wrap_key(&data, &mut buf),
                n => return Err(bad_kek(n)),
            }
            .map_err(|e| format!("wrap failed: {e}"))?;
            buf
        }
    };
    Ok(fmt.encode(&out))
}

/// Unwrap `wrapped` under `kek`. Returns the recovered key material, encoded with `fmt`.
/// Fails if the integrity check does not verify (wrong KEK or corrupted blob).
pub fn unwrap(
    wrapped: &str,
    kek: &str,
    algorithm: Algorithm,
    fmt: Encoding,
) -> Result<String, String> {
    let kek = fmt.decode(kek)?;
    let data = fmt.decode(wrapped)?;
    if data.len() < IV_LEN * 2 || data.len() % IV_LEN != 0 {
        return Err(format!(
            "wrapped data must be a multiple of 8 bytes and at least 16 bytes long (got {} bytes)",
            data.len()
        ));
    }

    let out: Vec<u8> = match algorithm {
        Algorithm::Kw => {
            let mut buf = vec![0u8; data.len() - IV_LEN];
            let n = match kek.len() {
                16 => kw_new::<KwAes128>(&kek)?.unwrap_key(&data, &mut buf),
                24 => kw_new::<KwAes192>(&kek)?.unwrap_key(&data, &mut buf),
                32 => kw_new::<KwAes256>(&kek)?.unwrap_key(&data, &mut buf),
                n => return Err(bad_kek(n)),
            }
            .map_err(|_| INTEGRITY_ERR.to_string())?
            .len();
            buf.truncate(n);
            buf
        }
        Algorithm::Kwp => {
            let mut buf = vec![0u8; data.len() - IV_LEN];
            let n = match kek.len() {
                16 => kw_new::<KwpAes128>(&kek)?.unwrap_key(&data, &mut buf),
                24 => kw_new::<KwpAes192>(&kek)?.unwrap_key(&data, &mut buf),
                32 => kw_new::<KwpAes256>(&kek)?.unwrap_key(&data, &mut buf),
                n => return Err(bad_kek(n)),
            }
            .map_err(|_| INTEGRITY_ERR.to_string())?
            .len();
            buf.truncate(n);
            buf
        }
    };
    Ok(fmt.encode(&out))
}

fn kw_new<T: KeyInit>(kek: &[u8]) -> Result<T, String> {
    T::new_from_slice(kek).map_err(|_| bad_kek(kek.len()))
}

fn bad_kek(n: usize) -> String {
    format!("the key-encryption key must be 16, 24, or 32 bytes (AES-128/192/256); got {n} bytes")
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 3394 §4.1 test vector: 128-bit KEK wrapping 128-bit key data.
    const RFC3394_KEK: &str = "000102030405060708090A0B0C0D0E0F";
    const RFC3394_KEY: &str = "00112233445566778899AABBCCDDEEFF";
    const RFC3394_WRAPPED: &str = "1FA68B0A8112B447AEF34BD8FB5A7B829D3E862371D2CFE5";

    #[test]
    fn kw_rfc3394_vector_wrap() {
        let out = wrap(RFC3394_KEY, RFC3394_KEK, Algorithm::Kw, Encoding::Hex).unwrap();
        assert_eq!(out.to_uppercase(), RFC3394_WRAPPED);
    }

    #[test]
    fn kw_rfc3394_vector_unwrap() {
        let out = unwrap(RFC3394_WRAPPED, RFC3394_KEK, Algorithm::Kw, Encoding::Hex).unwrap();
        assert_eq!(out.to_uppercase(), RFC3394_KEY);
    }

    #[test]
    fn kw_roundtrip_256() {
        let kek = "0".repeat(64); // 32 bytes hex
        let key = "deadbeef".repeat(8); // 32 bytes
        let w = wrap(&key, &kek, Algorithm::Kw, Encoding::Hex).unwrap();
        let u = unwrap(&w, &kek, Algorithm::Kw, Encoding::Hex).unwrap();
        assert_eq!(u, key);
    }

    #[test]
    fn kwp_arbitrary_length_roundtrip() {
        // 7 bytes — not a multiple of 8, so KW would reject; KWP pads.
        let kek = "00112233445566778899aabbccddeeff"; // 16 bytes
        let key = "0badc0ffee1337"; // 7 bytes
        let w = wrap(key, kek, Algorithm::Kwp, Encoding::Hex).unwrap();
        let u = unwrap(&w, kek, Algorithm::Kwp, Encoding::Hex).unwrap();
        assert_eq!(u, key);
    }

    #[test]
    fn kwp_single_byte() {
        let kek = "00112233445566778899aabbccddeeff";
        let key = "ab"; // 1 byte
        let w = wrap(key, kek, Algorithm::Kwp, Encoding::Hex).unwrap();
        let u = unwrap(&w, kek, Algorithm::Kwp, Encoding::Hex).unwrap();
        assert_eq!(u, key);
    }

    #[test]
    fn base64_roundtrip() {
        let kek = B64.encode([7u8; 16]);
        let key = B64.encode([42u8; 16]);
        let w = wrap(&key, &kek, Algorithm::Kw, Encoding::Base64).unwrap();
        let u = unwrap(&w, &kek, Algorithm::Kw, Encoding::Base64).unwrap();
        assert_eq!(u, key);
    }

    #[test]
    fn err_bad_kek_length() {
        let e = wrap(RFC3394_KEY, "001122", Algorithm::Kw, Encoding::Hex).unwrap_err();
        assert!(e.contains("16, 24, or 32 bytes"), "{e}");
    }

    #[test]
    fn err_kw_not_multiple_of_8() {
        let e = wrap("0badc0ffee1337", RFC3394_KEK, Algorithm::Kw, Encoding::Hex).unwrap_err();
        assert!(e.contains("multiple of 8"), "{e}");
    }

    #[test]
    fn err_kw_too_short() {
        // 8 bytes: multiple of 8 but < 16.
        let e = wrap("0011223344556677", RFC3394_KEK, Algorithm::Kw, Encoding::Hex).unwrap_err();
        assert!(e.contains("at least 16 bytes"), "{e}");
    }

    #[test]
    fn err_unwrap_wrong_kek() {
        let wrong = "ffeeddccbbaa99887766554433221100";
        let e = unwrap(RFC3394_WRAPPED, wrong, Algorithm::Kw, Encoding::Hex).unwrap_err();
        assert!(e.contains("integrity check failed"), "{e}");
    }

    #[test]
    fn err_bad_algorithm() {
        assert!(Algorithm::parse("nope").is_err());
    }
}
