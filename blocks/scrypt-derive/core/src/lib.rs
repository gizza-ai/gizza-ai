//! scrypt-derive core — derive a key from a password using the scrypt
//! memory-hard key-derivation function (RFC 7914 / Colin Percival 2009) with
//! configurable cost parameters N (CPU/memory cost), r (block size), p
//! (parallelization), a salt, and output length. Pure-Rust (`scrypt`),
//! deterministic, no I/O — shared by the chat skill block and the web page.

use scrypt::{scrypt, Params};

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

/// `N` (cost) must be a power of two > 1. Returns log2(N) for `Params`.
fn log2_n(n: u32) -> Result<u8, String> {
    if n < 2 || (n & (n - 1)) != 0 {
        return Err(format!(
            "N (cost) must be a power of two greater than 1 (got {n}); e.g. 16384"
        ));
    }
    // n is a power of two ≥ 2, so trailing_zeros() ∈ 1..=31.
    Ok(n.trailing_zeros() as u8)
}

/// Derive `dk_len` bytes from `password` (UTF-8) and `salt` using scrypt(N, r, p).
/// Returns the raw derived-key bytes.
pub fn derive_bytes(
    password: &str,
    salt: &[u8],
    n: u32,
    r: u32,
    p: u32,
    dk_len: usize,
) -> Result<Vec<u8>, String> {
    if r == 0 {
        return Err("r (block size) must be at least 1".into());
    }
    if p == 0 {
        return Err("p (parallelization) must be at least 1".into());
    }
    if dk_len == 0 {
        return Err("output length (bytes) must be at least 1".into());
    }
    if dk_len > 1024 {
        return Err("output length (bytes) must be at most 1024".into());
    }
    let log_n = log2_n(n)?;
    // scrypt safety constraint (RFC 7914 / the `scrypt` crate): N must satisfy
    // log2(N) < r * 16, otherwise N is too large relative to the block size.
    if (log_n as u32) >= r.saturating_mul(16) {
        return Err(format!(
            "N is too large for r={r}: scrypt requires log2(N) < r*16 (log2(N)={log_n}); raise r or lower N"
        ));
    }
    // scrypt also requires r * p < 2^30.
    if r.saturating_mul(p) >= 0x4000_0000 {
        return Err("r * p must be less than 2^30".into());
    }
    // Guard against absurd memory/CPU requests. scrypt memory ≈ 128 * N * r bytes;
    // cap at ~1 GiB so we never OOM the wasm runtime.
    let mem = 128u64
        .checked_mul(n as u64)
        .and_then(|v| v.checked_mul(r as u64))
        .ok_or_else(|| "scrypt parameters overflow".to_string())?;
    if mem > 1u64 << 30 {
        return Err(format!(
            "scrypt parameters too large: 128*N*r = {mem} bytes exceeds the 1 GiB limit; lower N or r"
        ));
    }
    // `Params::len` is unused by `scrypt()` (it derives `output.len()` bytes), but
    // `Params::new` requires len in 10..=64 — pass a fixed valid value; the real
    // output length is governed by the `out` buffer below.
    let params =
        Params::new(log_n, r, p, 32).map_err(|e| format!("invalid scrypt parameters: {e}"))?;
    let mut out = vec![0u8; dk_len];
    scrypt(password.as_bytes(), salt, &params, &mut out)
        .map_err(|e| format!("scrypt failed: {e}"))?;
    Ok(out)
}

/// High-level derive: decode the salt, run scrypt, and encode the result.
/// `salt_encoding` ∈ {utf8, hex, base64}; `out_encoding` ∈ {hex, base64}.
#[allow(clippy::too_many_arguments)]
pub fn derive(
    password: &str,
    salt: &str,
    salt_encoding: &str,
    n: u32,
    r: u32,
    p: u32,
    dk_len: usize,
    out_encoding: &str,
) -> Result<String, String> {
    let enc = Encoding::parse(out_encoding)?;
    let salt_bytes = decode_salt(salt, salt_encoding)?;
    let dk = derive_bytes(password, &salt_bytes, n, r, p, dk_len)?;
    Ok(match enc {
        Encoding::Hex => to_hex(&dk),
        Encoding::Base64 => base64_encode(&dk),
    })
}

/// Decode an expected key given as hex or base64 (auto-detected: a string of only
/// hex digits with even length is treated as hex, otherwise base64).
fn decode_expected(expected: &str) -> Result<Vec<u8>, String> {
    let s: String = expected.split_whitespace().collect();
    let looks_hex = !s.is_empty() && s.len() % 2 == 0 && s.bytes().all(|b| b.is_ascii_hexdigit());
    if looks_hex {
        decode_hex(&s)
    } else {
        base64_decode(expected)
    }
}

/// Constant-time-ish byte comparison (length + value).
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
    n: u32,
    r: u32,
    p: u32,
    expected: &str,
) -> Result<bool, String> {
    let want = decode_expected(expected)?;
    if want.is_empty() {
        return Err("expected key is required for verify".into());
    }
    let salt_bytes = decode_salt(salt, salt_encoding)?;
    let got = derive_bytes(password, &salt_bytes, n, r, p, want.len())?;
    Ok(bytes_eq(&got, &want))
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 7914 §12, test vector 1: P="", S="", N=16, r=1, p=1, dkLen=64.
    #[test]
    fn rfc7914_vector1() {
        let dk = derive_bytes("", b"", 16, 1, 1, 64).unwrap();
        assert_eq!(
            to_hex(&dk),
            "77d6576238657b203b19ca42c18a0497f16b4844e3074ae8dfdffa3fede2144\
             2fcd0069ded0948f8326a753a0fc81f17e8d3e0fb2e0d3628cf35e20c38d18906"
        );
    }

    // RFC 7914 §12, test vector 3: P="pleaseletmein", S="SodiumChloride",
    // N=16384, r=8, p=1, dkLen=64.
    #[test]
    fn rfc7914_vector3() {
        let dk = derive_bytes("pleaseletmein", b"SodiumChloride", 16384, 8, 1, 64).unwrap();
        assert_eq!(
            to_hex(&dk),
            "7023bdcb3afd7348461c06cd81fd38ebfda8fbba904f8e3ea9b543f6545da1f2\
             d5432955613f0fcf62d49705242a9af9e61e85dc0d651e40dfcf017b45575887"
        );
    }

    #[test]
    fn high_level_hex_matches_rfc() {
        let out = derive("", "", "utf8", 16, 1, 1, 64, "hex").unwrap();
        assert_eq!(
            out,
            "77d6576238657b203b19ca42c18a0497f16b4844e3074ae8dfdffa3fede2144\
             2fcd0069ded0948f8326a753a0fc81f17e8d3e0fb2e0d3628cf35e20c38d18906"
        );
    }

    #[test]
    fn deterministic() {
        let a = derive_bytes("pw", b"salt", 1024, 8, 1, 32).unwrap();
        let b = derive_bytes("pw", b"salt", 1024, 8, 1, 32).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn base64_output_roundtrips() {
        let out = derive("pw", "salt", "utf8", 1024, 8, 1, 16, "base64").unwrap();
        let raw = base64_decode(&out).unwrap();
        assert_eq!(
            to_hex(&raw),
            derive("pw", "salt", "utf8", 1024, 8, 1, 16, "hex").unwrap()
        );
    }

    #[test]
    fn hex_salt_decodes() {
        // salt "salt" == hex 73616c74
        let a = derive("pw", "73616c74", "hex", 1024, 8, 1, 32, "hex").unwrap();
        let b = derive("pw", "salt", "utf8", 1024, 8, 1, 32, "hex").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn base64_salt_decodes() {
        // "salt" base64 == "c2FsdA=="
        let a = derive("pw", "c2FsdA==", "base64", 1024, 8, 1, 32, "hex").unwrap();
        let b = derive("pw", "salt", "utf8", 1024, 8, 1, 32, "hex").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn empty_password_and_salt_allowed() {
        let out = derive("", "", "utf8", 16, 1, 1, 32, "hex").unwrap();
        assert_eq!(out.len(), 64);
    }

    #[test]
    fn verify_roundtrip_hex() {
        let key = "77d6576238657b203b19ca42c18a0497f16b4844e3074ae8dfdffa3fede2144\
                   2fcd0069ded0948f8326a753a0fc81f17e8d3e0fb2e0d3628cf35e20c38d18906";
        assert!(verify("", "", "utf8", 16, 1, 1, key).unwrap());
        assert!(!verify("x", "", "utf8", 16, 1, 1, key).unwrap()); // wrong pw
        assert!(!verify("", "", "utf8", 32, 1, 1, key).unwrap()); // wrong N
    }

    #[test]
    fn verify_roundtrip_base64() {
        let key = derive("pw", "salt", "utf8", 1024, 8, 1, 16, "base64").unwrap();
        assert!(verify("pw", "salt", "utf8", 1024, 8, 1, &key).unwrap());
        assert!(!verify("pw", "salt", "utf8", 1024, 8, 1, "AAAA").unwrap());
    }

    #[test]
    fn errors() {
        assert!(derive("pw", "salt", "utf8", 15, 8, 1, 32, "hex").is_err()); // N not power of two
        assert!(derive("pw", "salt", "utf8", 1, 8, 1, 32, "hex").is_err()); // N must be > 1
        assert!(derive("pw", "salt", "utf8", 1024, 0, 1, 32, "hex").is_err()); // r = 0
        assert!(derive("pw", "salt", "utf8", 1024, 8, 0, 32, "hex").is_err()); // p = 0
        assert!(derive("pw", "salt", "utf8", 1024, 8, 1, 0, "hex").is_err()); // len = 0
        assert!(derive("pw", "salt", "utf8", 1024, 8, 1, 2000, "hex").is_err()); // len too big
        assert!(derive("pw", "zz", "hex", 1024, 8, 1, 32, "hex").is_err()); // bad hex salt
        assert!(derive("pw", "salt", "utf8", 1024, 8, 1, 32, "md5").is_err()); // bad out encoding
        // Memory guard: N=2^24, r=8 → 128*N*r ≈ 16 GiB.
        assert!(derive("pw", "salt", "utf8", 16777216, 8, 1, 32, "hex").is_err());
    }

    #[test]
    fn verify_errors() {
        assert!(verify("pw", "salt", "utf8", 1024, 8, 1, "").is_err()); // empty expected
    }
}
