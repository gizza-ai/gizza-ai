//! nt-hash core — compute the NT (NTLM) hash of a password.
//!
//! Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps.
//!
//! The NT hash (also called the NTLM hash, or "NT one-way function" / NTOWF) is
//! defined as `MD4(UTF-16LE(password))`: the password is encoded as a
//! little-endian UTF-16 byte string, then MD4-hashed. The result is a 128-bit
//! (16-byte) digest, conventionally written as 32 lowercase hex chars. It is the
//! value stored in the Windows SAM / NTDS.dit and used by NTLM authentication
//! and pass-the-hash.
//!
//! The NT hash is **unsalted and uncached** — the same password always yields
//! the same hash — and MD4 is fast and cryptographically broken, so NT hashes
//! offer essentially no protection against offline cracking. This tool exists
//! for password-audit, CTF, and NTLM-interop / pass-the-hash testing, NOT for
//! securely storing new passwords (use argon2-hash or bcrypt-hash for that).

use base64::Engine;
use md4::{Digest, Md4};

/// How to render the 16-byte digest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    /// Hexadecimal (32 chars) — the conventional NTLM representation.
    Hex,
    /// Standard base64 (24 chars incl. padding).
    Base64,
}

fn parse_output_format(s: &str) -> Result<OutputFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "hex" => Ok(OutputFormat::Hex),
        "base64" | "b64" => Ok(OutputFormat::Base64),
        other => Err(format!(
            "invalid output_format '{other}': expected 'hex' or 'base64'"
        )),
    }
}

/// Encode `password` as a UTF-16LE byte string (each UTF-16 code unit as two
/// little-endian bytes). Characters outside the BMP are encoded as surrogate
/// pairs, matching the NTLM specification.
pub fn utf16le_bytes(password: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(password.len() * 2);
    for unit in password.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

/// Compute the raw 16-byte NT hash (`MD4(UTF-16LE(password))`).
pub fn nt_hash_bytes(password: &str) -> [u8; 16] {
    let mut hasher = Md4::new();
    hasher.update(utf16le_bytes(password));
    hasher.finalize().into()
}

/// Compute the NT (NTLM) hash of `password` and render it per `output_format`.
/// When `output_format` is hex, `uppercase` controls the hex case; it has no
/// effect on base64 output.
///
/// Defaults (blank strings): output_format=hex, lowercase.
pub fn hash(password: &str, output_format: &str, uppercase: bool) -> Result<String, String> {
    let fmt = parse_output_format(output_format)?;
    let digest = nt_hash_bytes(password);
    Ok(match fmt {
        OutputFormat::Hex => {
            let h = hex::encode(digest);
            if uppercase {
                h.to_uppercase()
            } else {
                h
            }
        }
        OutputFormat::Base64 => base64::engine::general_purpose::STANDARD.encode(digest),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Well-known NTLM test vectors.
    const EMPTY: &str = "31d6cfe0d16ae931b73c59d7e0c089c0";
    const PASSWORD: &str = "8846f7eaee8fb117ad06bdd830b7586c";

    #[test]
    fn hashes_empty_password() {
        assert_eq!(hash("", "", false).unwrap(), EMPTY);
    }

    #[test]
    fn hashes_password() {
        assert_eq!(hash("password", "hex", false).unwrap(), PASSWORD);
    }

    #[test]
    fn hashes_123456() {
        assert_eq!(
            hash("123456", "", false).unwrap(),
            "32ed87bdb5fdc5e9cba88547376818d4"
        );
    }

    #[test]
    fn uppercase_hex() {
        assert_eq!(
            hash("password", "hex", true).unwrap(),
            PASSWORD.to_uppercase()
        );
    }

    #[test]
    fn base64_output() {
        // base64 of the raw 16-byte NT hash of "password".
        assert_eq!(
            hash("password", "base64", false).unwrap(),
            "iEb36u6PsRetBr3YMLdYbA=="
        );
    }

    #[test]
    fn unicode_password() {
        // Non-ASCII characters are encoded as UTF-16LE before hashing; the
        // result is a stable 32-char hex digest (sanity, not a published vector).
        let h = hash("Pässwörd☃", "hex", false).unwrap();
        assert_eq!(h.len(), 32);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn digest_is_16_bytes() {
        assert_eq!(nt_hash_bytes("password").len(), 16);
    }

    #[test]
    fn rejects_bad_output_format() {
        assert!(hash("password", "binary", false).is_err());
    }
}
