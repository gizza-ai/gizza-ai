//! pbkdf2-derive core — derive a key from a password using PBKDF2 (RFC 2898 /
//! RFC 8018) with a selectable HMAC hash (SHA-1 / SHA-256 / SHA-512), iteration
//! count, salt, and output length. Pure-Rust (`pbkdf2` + `hmac` + `sha1`/`sha2`),
//! deterministic, no I/O — shared by the chat skill block and the web page.

use hmac::Hmac;
use sha1::Sha1;
use sha2::{Sha256, Sha512};

/// Supported HMAC pseudorandom functions for PBKDF2.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prf {
    Sha1,
    Sha256,
    Sha512,
}

impl Prf {
    pub fn parse(s: &str) -> Result<Prf, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "sha1" | "hmacsha1" => Ok(Prf::Sha1),
            "sha256" | "hmacsha256" => Ok(Prf::Sha256),
            "sha512" | "hmacsha512" => Ok(Prf::Sha512),
            other => Err(format!(
                "unknown hash '{other}' (use sha1, sha256, or sha512)"
            )),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Prf::Sha1 => "sha1",
            Prf::Sha256 => "sha256",
            Prf::Sha512 => "sha512",
        }
    }
}

/// Output encodings for the derived key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Encoding {
    Hex,
    Base64,
}

impl Encoding {
    pub fn parse(s: &str) -> Result<Encoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "hex" | "" => Ok(Encoding::Hex),
            "base64" | "b64" => Ok(Encoding::Base64),
            other => Err(format!("unknown encoding '{other}' (use hex or base64)")),
        }
    }
}

/// Decode the salt according to `salt_encoding` ("utf8" | "hex" | "base64").
fn decode_salt(salt: &str, salt_encoding: &str) -> Result<Vec<u8>, String> {
    match salt_encoding.trim().to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" | "text" | "" => Ok(salt.as_bytes().to_vec()),
        "hex" => decode_hex(salt),
        "base64" | "b64" => base64_decode(salt),
        other => Err(format!(
            "unknown salt_encoding '{other}' (use utf8, hex, or base64)"
        )),
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let s: String = s.split_whitespace().collect();
    if s.len() % 2 != 0 {
        return Err("hex salt must have an even number of digits".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| format!("invalid hex byte '{}'", &s[i..i + 2]))
        })
        .collect()
}

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(B64[((n >> 18) & 63) as usize] as char);
        out.push(B64[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            B64[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let trimmed = s.trim_end_matches('=');
    let mut out = Vec::with_capacity(trimmed.len() / 4 * 3 + 3);
    let bytes = trimmed.as_bytes();
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        let mut bits = 0;
        for &c in chunk {
            let v = val(c).ok_or_else(|| format!("invalid base64 character '{}'", c as char))?;
            n = (n << 6) | v;
            bits += 6;
        }
        // Pad to a multiple of 8 from the top.
        n <<= 24 - bits;
        let nbytes = bits / 8;
        for i in 0..nbytes {
            out.push((n >> (16 - i * 8)) as u8);
        }
    }
    Ok(out)
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Derive `dk_len` bytes from `password` (UTF-8) and `salt` using PBKDF2-HMAC.
/// Returns the raw derived-key bytes.
pub fn derive_bytes(
    password: &str,
    salt: &[u8],
    iterations: u32,
    prf: Prf,
    dk_len: usize,
) -> Result<Vec<u8>, String> {
    if iterations == 0 {
        return Err("iterations must be at least 1".into());
    }
    if dk_len == 0 {
        return Err("output length (bytes) must be at least 1".into());
    }
    if dk_len > 1024 {
        return Err("output length (bytes) must be at most 1024".into());
    }
    let mut out = vec![0u8; dk_len];
    let res = match prf {
        Prf::Sha1 => pbkdf2::pbkdf2::<Hmac<Sha1>>(password.as_bytes(), salt, iterations, &mut out),
        Prf::Sha256 => {
            pbkdf2::pbkdf2::<Hmac<Sha256>>(password.as_bytes(), salt, iterations, &mut out)
        }
        Prf::Sha512 => {
            pbkdf2::pbkdf2::<Hmac<Sha512>>(password.as_bytes(), salt, iterations, &mut out)
        }
    };
    res.map_err(|e| format!("PBKDF2 failed: {e}"))?;
    Ok(out)
}

/// High-level derive: decode the salt, run PBKDF2, and encode the result.
/// `hash` ∈ {sha1, sha256, sha512}; `salt_encoding` ∈ {utf8, hex, base64};
/// `out_encoding` ∈ {hex, base64}.
pub fn derive(
    password: &str,
    salt: &str,
    salt_encoding: &str,
    iterations: u32,
    hash: &str,
    dk_len: usize,
    out_encoding: &str,
) -> Result<String, String> {
    if password.is_empty() {
        return Err("password is required".into());
    }
    let prf = Prf::parse(hash)?;
    let enc = Encoding::parse(out_encoding)?;
    let salt_bytes = decode_salt(salt, salt_encoding)?;
    let dk = derive_bytes(password, &salt_bytes, iterations, prf, dk_len)?;
    Ok(match enc {
        Encoding::Hex => to_hex(&dk),
        Encoding::Base64 => base64_encode(&dk),
    })
}

/// Decode an expected key given as hex or base64 (auto-detected: a string of only
/// hex digits with even length is treated as hex, otherwise base64).
fn decode_expected(expected: &str) -> Result<Vec<u8>, String> {
    let s: String = expected.split_whitespace().collect();
    let looks_hex = !s.is_empty()
        && s.len() % 2 == 0
        && s.bytes().all(|b| b.is_ascii_hexdigit());
    if looks_hex {
        decode_hex(&s)
    } else {
        base64_decode(expected)
    }
}

/// Constant-time-ish byte comparison (length + value). Both inputs are
/// non-secret-length derived keys, so this is a defensive equality check.
fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Verify that `password` + params re-derive the `expected` key.
/// `expected` may be hex or base64 (auto-detected); the derived-key length is
/// taken from the expected key's byte length (so `dk_len` is implied).
pub fn verify(
    password: &str,
    salt: &str,
    salt_encoding: &str,
    iterations: u32,
    hash: &str,
    expected: &str,
) -> Result<bool, String> {
    if password.is_empty() {
        return Err("password is required".into());
    }
    let want = decode_expected(expected)?;
    if want.is_empty() {
        return Err("expected key is required for verify".into());
    }
    let prf = Prf::parse(hash)?;
    let salt_bytes = decode_salt(salt, salt_encoding)?;
    let got = derive_bytes(password, &salt_bytes, iterations, prf, want.len())?;
    Ok(bytes_eq(&got, &want))
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 6070 PBKDF2-HMAC-SHA1 test vector:
    // P="password", S="salt", c=1, dkLen=20.
    #[test]
    fn rfc6070_sha1_c1() {
        let dk = derive_bytes("password", b"salt", 1, Prf::Sha1, 20).unwrap();
        assert_eq!(to_hex(&dk), "0c60c80f961f0e71f3a9b524af6012062fe037a6");
    }

    // RFC 6070 PBKDF2-HMAC-SHA1: P="password", S="salt", c=2, dkLen=20.
    #[test]
    fn rfc6070_sha1_c2() {
        let dk = derive_bytes("password", b"salt", 2, Prf::Sha1, 20).unwrap();
        assert_eq!(to_hex(&dk), "ea6c014dc72d6f8ccd1ed92ace1d41f0d8de8957");
    }

    // RFC 6070: P="passwordPASSWORDpassword",
    // S="saltSALTsaltSALTsaltSALTsaltSALTsalt", c=4096, dkLen=25.
    #[test]
    fn rfc6070_sha1_long() {
        let dk = derive_bytes(
            "passwordPASSWORDpassword",
            b"saltSALTsaltSALTsaltSALTsaltSALTsalt",
            4096,
            Prf::Sha1,
            25,
        )
        .unwrap();
        assert_eq!(
            to_hex(&dk),
            "3d2eec4fe41c849b80c8d83662c0e44a8b291a964cf2f07038"
        );
    }

    // RFC 7914 §11 PBKDF2-HMAC-SHA256: P="passwd", S="salt", c=1, dkLen=64.
    #[test]
    fn rfc7914_sha256_c1() {
        let dk = derive_bytes("passwd", b"salt", 1, Prf::Sha256, 64).unwrap();
        assert_eq!(
            to_hex(&dk),
            "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc\
             49ca9cccf179b645991664b39d77ef317c71b845b1e30bd509112041d3a19783"
        );
    }

    #[test]
    fn sha512_derives() {
        // Just exercise the SHA-512 path + deterministic re-derivation.
        let a = derive_bytes("pw", b"NaCl", 4096, Prf::Sha512, 32).unwrap();
        let b = derive_bytes("pw", b"NaCl", 4096, Prf::Sha512, 32).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn high_level_hex_matches_rfc() {
        let out = derive("password", "salt", "utf8", 1, "sha1", 20, "hex").unwrap();
        assert_eq!(out, "0c60c80f961f0e71f3a9b524af6012062fe037a6");
    }

    #[test]
    fn base64_output_roundtrips() {
        let out = derive("password", "salt", "utf8", 1, "sha256", 16, "base64").unwrap();
        let raw = base64_decode(&out).unwrap();
        assert_eq!(to_hex(&raw), derive("password", "salt", "utf8", 1, "sha256", 16, "hex").unwrap());
    }

    #[test]
    fn hex_salt_decodes() {
        // salt "salt" == hex 73616c74
        let a = derive("password", "73616c74", "hex", 1, "sha1", 20, "hex").unwrap();
        let b = derive("password", "salt", "utf8", 1, "sha1", 20, "hex").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn base64_salt_decodes() {
        // "salt" base64 == "c2FsdA=="
        let a = derive("password", "c2FsdA==", "base64", 1, "sha1", 20, "hex").unwrap();
        let b = derive("password", "salt", "utf8", 1, "sha1", 20, "hex").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn empty_salt_allowed() {
        let out = derive("password", "", "utf8", 1, "sha256", 16, "hex").unwrap();
        assert_eq!(out.len(), 32);
    }

    #[test]
    fn verify_roundtrip_hex() {
        // RFC 6070 SHA-1 c=1 key, given as hex.
        let key = "0c60c80f961f0e71f3a9b524af6012062fe037a6";
        assert!(verify("password", "salt", "utf8", 1, "sha1", key).unwrap());
        assert!(!verify("Password", "salt", "utf8", 1, "sha1", key).unwrap());
        assert!(!verify("password", "salt", "utf8", 2, "sha1", key).unwrap()); // wrong iters
    }

    #[test]
    fn verify_roundtrip_base64() {
        let key = derive("passwd", "salt", "utf8", 1, "sha256", 16, "base64").unwrap();
        assert!(verify("passwd", "salt", "utf8", 1, "sha256", &key).unwrap());
        assert!(!verify("passwd", "salt", "utf8", 1, "sha256", "AAAA").unwrap());
    }

    #[test]
    fn verify_errors() {
        assert!(verify("", "salt", "utf8", 1, "sha256", "deadbeef").is_err()); // empty pw
        assert!(verify("pw", "salt", "utf8", 1, "sha256", "").is_err()); // empty expected
    }

    #[test]
    fn errors() {
        assert!(derive("", "salt", "utf8", 1, "sha256", 16, "hex").is_err()); // empty pw
        assert!(derive("pw", "salt", "utf8", 0, "sha256", 16, "hex").is_err()); // 0 iters
        assert!(derive("pw", "salt", "utf8", 1, "sha256", 0, "hex").is_err()); // 0 len
        assert!(derive("pw", "salt", "utf8", 1, "md5", 16, "hex").is_err()); // bad hash
        assert!(derive("pw", "salt", "utf8", 1, "sha256", 2000, "hex").is_err()); // len too big
        assert!(derive("pw", "zz", "hex", 1, "sha256", 16, "hex").is_err()); // bad hex salt
    }
}
