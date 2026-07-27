//! key-derive core — one unified key-derivation-function selector. Derives a key
//! of a chosen length from a passphrase or seed via **PBKDF2**, **scrypt**,
//! **Argon2id/i/d**, or **HKDF** (RFC 5869), with each algorithm's own parameters.
//! Pure-Rust (`pbkdf2` + `scrypt` + `argon2` + `hkdf` + `hmac` + `sha1`/`sha2`),
//! deterministic, no I/O — shared by the chat skill block and the web page.
//!
//! Unlike the focused `argon2-hash` block (which emits a PHC hash string for
//! password storage), the Argon2 path here returns **raw key material** of the
//! requested length via `Argon2::hash_password_into`.

use hmac::Hmac;
use sha1::Sha1;
use sha2::{Sha256, Sha384, Sha512};

/// Supported key-derivation functions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Algorithm {
    Pbkdf2,
    Scrypt,
    Argon2,
    Hkdf,
}

impl Algorithm {
    pub fn parse(s: &str) -> Result<Algorithm, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "pbkdf2" | "" => Ok(Algorithm::Pbkdf2),
            "scrypt" => Ok(Algorithm::Scrypt),
            "argon2" | "argon2id" | "argon2i" | "argon2d" => Ok(Algorithm::Argon2),
            "hkdf" => Ok(Algorithm::Hkdf),
            other => Err(format!(
                "unknown algorithm '{other}' (use pbkdf2, scrypt, argon2, or hkdf)"
            )),
        }
    }
}

/// Hash functions shared by PBKDF2 (sha1/256/512) and HKDF (sha1/256/384/512).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hash {
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

impl Hash {
    pub fn parse(s: &str) -> Result<Hash, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "sha1" | "hmacsha1" => Ok(Hash::Sha1),
            "sha256" | "hmacsha256" | "" => Ok(Hash::Sha256),
            "sha384" | "hmacsha384" => Ok(Hash::Sha384),
            "sha512" | "hmacsha512" => Ok(Hash::Sha512),
            other => Err(format!(
                "unknown hash '{other}' (use sha1, sha256, sha384, or sha512)"
            )),
        }
    }
}

/// Argon2 variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Argon2Variant {
    Argon2id,
    Argon2i,
    Argon2d,
}

impl Argon2Variant {
    pub fn parse(s: &str) -> Result<Argon2Variant, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "argon2id" | "id" | "" => Ok(Argon2Variant::Argon2id),
            "argon2i" | "i" => Ok(Argon2Variant::Argon2i),
            "argon2d" | "d" => Ok(Argon2Variant::Argon2d),
            other => Err(format!(
                "unknown argon2_variant '{other}' (use argon2id, argon2i, or argon2d)"
            )),
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

const MAX_LEN: usize = 1024;

/// Decode a byte string according to `encoding` ("utf8" | "hex" | "base64").
fn decode_bytes(s: &str, encoding: &str, what: &str) -> Result<Vec<u8>, String> {
    match encoding.trim().to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" | "text" | "" => Ok(s.as_bytes().to_vec()),
        "hex" => decode_hex(s).map_err(|e| format!("{what}: {e}")),
        "base64" | "b64" => base64_decode(s).map_err(|e| format!("{what}: {e}")),
        other => Err(format!(
            "unknown {what}_encoding '{other}' (use utf8, hex, or base64)"
        )),
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let s: String = s.split_whitespace().collect();
    if s.len() % 2 != 0 {
        return Err("hex must have an even number of digits".into());
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

fn check_len(dk_len: usize) -> Result<(), String> {
    if dk_len == 0 {
        return Err("output length (bytes) must be at least 1".into());
    }
    if dk_len > MAX_LEN {
        return Err(format!("output length (bytes) must be at most {MAX_LEN}"));
    }
    Ok(())
}

/// Per-algorithm parameter bundle (decoded). Only the fields for the selected
/// algorithm are read, so callers can leave the rest at their defaults.
#[derive(Clone, Debug)]
pub struct Params {
    // PBKDF2 + HKDF
    pub hash: Hash,
    // PBKDF2
    pub iterations: u32,
    // scrypt
    pub n: u32,
    pub r: u32,
    pub p: u32,
    // Argon2
    pub memory_kib: u32,
    pub time_cost: u32,
    pub parallelism: u32,
    pub variant: Argon2Variant,
    // HKDF
    pub info: Vec<u8>,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            hash: Hash::Sha256,
            iterations: 100_000,
            n: 16384,
            r: 8,
            p: 1,
            memory_kib: 19456,
            time_cost: 2,
            parallelism: 1,
            variant: Argon2Variant::Argon2id,
            info: Vec::new(),
        }
    }
}

/// Derive `dk_len` raw bytes from `secret` (already-decoded input key material /
/// passphrase bytes) and `salt` using `algorithm` with `params`.
pub fn derive_bytes(
    algorithm: Algorithm,
    secret: &[u8],
    salt: &[u8],
    params: &Params,
    dk_len: usize,
) -> Result<Vec<u8>, String> {
    check_len(dk_len)?;
    let mut out = vec![0u8; dk_len];
    match algorithm {
        Algorithm::Pbkdf2 => {
            if params.iterations == 0 {
                return Err("iterations must be at least 1".into());
            }
            let res = match params.hash {
                Hash::Sha1 => {
                    pbkdf2::pbkdf2::<Hmac<Sha1>>(secret, salt, params.iterations, &mut out)
                }
                Hash::Sha256 => {
                    pbkdf2::pbkdf2::<Hmac<Sha256>>(secret, salt, params.iterations, &mut out)
                }
                Hash::Sha384 => {
                    pbkdf2::pbkdf2::<Hmac<Sha384>>(secret, salt, params.iterations, &mut out)
                }
                Hash::Sha512 => {
                    pbkdf2::pbkdf2::<Hmac<Sha512>>(secret, salt, params.iterations, &mut out)
                }
            };
            res.map_err(|e| format!("PBKDF2 failed: {e}"))?;
        }
        Algorithm::Scrypt => derive_scrypt(secret, salt, params, &mut out)?,
        Algorithm::Argon2 => derive_argon2(secret, salt, params, &mut out)?,
        Algorithm::Hkdf => derive_hkdf(secret, salt, params, &mut out)?,
    }
    Ok(out)
}

fn log2_n(n: u32) -> Result<u8, String> {
    if n < 2 || (n & (n - 1)) != 0 {
        return Err(format!(
            "scrypt N must be a power of two greater than 1 (got {n})"
        ));
    }
    Ok(n.trailing_zeros() as u8)
}

fn derive_scrypt(secret: &[u8], salt: &[u8], p: &Params, out: &mut [u8]) -> Result<(), String> {
    if p.r == 0 {
        return Err("scrypt r (block size) must be at least 1".into());
    }
    if p.p == 0 {
        return Err("scrypt p (parallelization) must be at least 1".into());
    }
    let log_n = log2_n(p.n)?;
    if (log_n as u32) >= p.r.saturating_mul(16) {
        return Err(format!(
            "scrypt N is too large for r={}: requires log2(N) < r*16 (log2(N)={log_n})",
            p.r
        ));
    }
    if p.r.saturating_mul(p.p) >= 0x4000_0000 {
        return Err("scrypt r * p must be less than 2^30".into());
    }
    let mem = 128u64
        .checked_mul(p.n as u64)
        .and_then(|v| v.checked_mul(p.r as u64))
        .ok_or_else(|| "scrypt parameters overflow".to_string())?;
    if mem > 1u64 << 30 {
        return Err(format!(
            "scrypt parameters too large: 128*N*r = {mem} bytes exceeds the 1 GiB limit"
        ));
    }
    // `Params::len` is unused by `scrypt()` (output length comes from `out`), but
    // `Params::new` requires len in 10..=64 — pass a fixed valid value.
    let sp = scrypt::Params::new(log_n, p.r, p.p, 32)
        .map_err(|e| format!("invalid scrypt parameters: {e}"))?;
    scrypt::scrypt(secret, salt, &sp, out).map_err(|e| format!("scrypt failed: {e}"))
}

fn derive_argon2(secret: &[u8], salt: &[u8], p: &Params, out: &mut [u8]) -> Result<(), String> {
    use argon2::{Algorithm as A2, Argon2, Params as A2Params, Version};
    // Argon2 requires a salt of at least 8 bytes.
    if salt.len() < 8 {
        return Err("Argon2 requires a salt of at least 8 bytes".into());
    }
    let alg = match p.variant {
        Argon2Variant::Argon2id => A2::Argon2id,
        Argon2Variant::Argon2i => A2::Argon2i,
        Argon2Variant::Argon2d => A2::Argon2d,
    };
    let ap = A2Params::new(p.memory_kib, p.time_cost, p.parallelism, Some(out.len()))
        .map_err(|e| format!("invalid Argon2 parameters: {e}"))?;
    let argon = Argon2::new(alg, Version::V0x13, ap);
    argon
        .hash_password_into(secret, salt, out)
        .map_err(|e| format!("Argon2 failed: {e}"))
}

fn derive_hkdf(ikm: &[u8], salt: &[u8], p: &Params, out: &mut [u8]) -> Result<(), String> {
    use hkdf::Hkdf;
    let salt_opt = if salt.is_empty() { None } else { Some(salt) };
    let res = match p.hash {
        Hash::Sha1 => Hkdf::<Sha1>::new(salt_opt, ikm).expand(&p.info, out),
        Hash::Sha256 => Hkdf::<Sha256>::new(salt_opt, ikm).expand(&p.info, out),
        Hash::Sha384 => Hkdf::<Sha384>::new(salt_opt, ikm).expand(&p.info, out),
        Hash::Sha512 => Hkdf::<Sha512>::new(salt_opt, ikm).expand(&p.info, out),
    };
    res.map_err(|_| "HKDF expand failed (requested length exceeds 255 * HashLen)".to_string())
}

/// String-form parameter bundle for [`derive`]. Mirrors the descriptor fields so the
/// block handler and the web wrapper can pass raw strings/numbers through.
#[derive(Clone, Debug)]
pub struct DeriveParams {
    pub hash: String,
    pub iterations: u32,
    pub n: u32,
    pub r: u32,
    pub p: u32,
    pub memory_kib: u32,
    pub time_cost: u32,
    pub parallelism: u32,
    pub argon2_variant: String,
    pub info: String,
    pub info_encoding: String,
}

impl Default for DeriveParams {
    fn default() -> DeriveParams {
        DeriveParams {
            hash: "sha256".into(),
            iterations: 100_000,
            n: 16384,
            r: 8,
            p: 1,
            memory_kib: 19456,
            time_cost: 2,
            parallelism: 1,
            argon2_variant: "argon2id".into(),
            info: String::new(),
            info_encoding: "utf8".into(),
        }
    }
}

/// High-level derive: parse the algorithm/encodings, decode the secret + salt +
/// info, run the KDF, and encode the result.
#[allow(clippy::too_many_arguments)]
pub fn derive(
    algorithm: &str,
    secret: &str,
    input_encoding: &str,
    salt: &str,
    salt_encoding: &str,
    dk_len: usize,
    out_encoding: &str,
    params: DeriveParams,
) -> Result<String, String> {
    if secret.is_empty() {
        return Err("secret (passphrase or seed) is required".into());
    }
    let alg = Algorithm::parse(algorithm)?;
    let enc = Encoding::parse(out_encoding)?;
    let secret_bytes = decode_bytes(secret, input_encoding, "input")?;
    let salt_bytes = decode_bytes(salt, salt_encoding, "salt")?;

    let p = Params {
        hash: Hash::parse(&params.hash)?,
        iterations: params.iterations,
        n: params.n,
        r: params.r,
        p: params.p,
        memory_kib: params.memory_kib,
        time_cost: params.time_cost,
        parallelism: params.parallelism,
        variant: Argon2Variant::parse(&params.argon2_variant)?,
        info: decode_bytes(&params.info, &params.info_encoding, "info")?,
    };

    let dk = derive_bytes(alg, &secret_bytes, &salt_bytes, &p, dk_len)?;
    Ok(match enc {
        Encoding::Hex => to_hex(&dk),
        Encoding::Base64 => base64_encode(&dk),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dp() -> DeriveParams {
        DeriveParams::default()
    }

    // --- PBKDF2: RFC 6070 HMAC-SHA1, P="password" S="salt" c=1 dkLen=20 ---
    #[test]
    fn pbkdf2_rfc6070_sha1() {
        let out = derive(
            "pbkdf2",
            "password",
            "utf8",
            "salt",
            "utf8",
            20,
            "hex",
            DeriveParams {
                hash: "sha1".into(),
                iterations: 1,
                ..dp()
            },
        )
        .unwrap();
        assert_eq!(out, "0c60c80f961f0e71f3a9b524af6012062fe037a6");
    }

    // --- PBKDF2: RFC 7914 HMAC-SHA256, P="passwd" S="salt" c=1 dkLen=64 ---
    #[test]
    fn pbkdf2_rfc7914_sha256() {
        let out = derive(
            "pbkdf2",
            "passwd",
            "utf8",
            "salt",
            "utf8",
            64,
            "hex",
            DeriveParams {
                iterations: 1,
                ..dp()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc\
             49ca9cccf179b645991664b39d77ef317c71b845b1e30bd509112041d3a19783"
        );
    }

    // --- scrypt: RFC 7914 §12 vector, P="password" S="NaCl" N=1024 r=8 p=16 ---
    #[test]
    fn scrypt_rfc7914() {
        let out = derive(
            "scrypt",
            "password",
            "utf8",
            "NaCl",
            "utf8",
            64,
            "hex",
            DeriveParams {
                n: 1024,
                r: 8,
                p: 16,
                ..dp()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            "fdbabe1c9d3472007856e7190d01e9fe7c6ad7cbc8237830e77376634b373162\
             2eaf30d92e22a3886ff109279d9830dac727afb94a83ee6d8360cbdfa2cc0640"
        );
    }

    // --- HKDF: RFC 5869 test case 1 (SHA-256) ---
    #[test]
    fn hkdf_rfc5869_case1() {
        // IKM = 0x0b*22, salt = 000102...0c, info = f0f1...f9, L = 42.
        let out = derive(
            "hkdf",
            "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b",
            "hex",
            "000102030405060708090a0b0c",
            "hex",
            42,
            "hex",
            DeriveParams {
                hash: "sha256".into(),
                info: "f0f1f2f3f4f5f6f7f8f9".into(),
                info_encoding: "hex".into(),
                ..dp()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf\
             34007208d5b887185865"
        );
    }

    // --- Argon2id raw key material (the distinguishing capability) ---
    #[test]
    fn argon2id_raw_bytes_deterministic() {
        let a = derive(
            "argon2",
            "password",
            "utf8",
            "somesalt",
            "utf8",
            32,
            "hex",
            DeriveParams {
                memory_kib: 4096,
                time_cost: 3,
                parallelism: 1,
                ..dp()
            },
        )
        .unwrap();
        let b = derive(
            "argon2",
            "password",
            "utf8",
            "somesalt",
            "utf8",
            32,
            "hex",
            DeriveParams {
                memory_kib: 4096,
                time_cost: 3,
                parallelism: 1,
                ..dp()
            },
        )
        .unwrap();
        assert_eq!(a, b, "deterministic for identical inputs");
        assert_eq!(a.len(), 64, "32 bytes -> 64 hex chars");
        // Not a PHC string — raw key material only.
        assert!(!a.starts_with('$'));
    }

    #[test]
    fn base64_output_roundtrips() {
        let hex = derive(
            "pbkdf2",
            "pw",
            "utf8",
            "salt",
            "utf8",
            16,
            "hex",
            DeriveParams {
                iterations: 10,
                ..dp()
            },
        )
        .unwrap();
        let b64 = derive(
            "pbkdf2",
            "pw",
            "utf8",
            "salt",
            "utf8",
            16,
            "base64",
            DeriveParams {
                iterations: 10,
                ..dp()
            },
        )
        .unwrap();
        assert_eq!(to_hex(&base64_decode(&b64).unwrap()), hex);
    }

    #[test]
    fn hex_and_utf8_salt_agree() {
        // "salt" == hex 73616c74
        let a = derive(
            "pbkdf2",
            "pw",
            "utf8",
            "73616c74",
            "hex",
            16,
            "hex",
            DeriveParams {
                iterations: 5,
                ..dp()
            },
        )
        .unwrap();
        let b = derive(
            "pbkdf2",
            "pw",
            "utf8",
            "salt",
            "utf8",
            16,
            "hex",
            DeriveParams {
                iterations: 5,
                ..dp()
            },
        )
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn errors() {
        // empty secret
        assert!(derive("pbkdf2", "", "utf8", "salt", "utf8", 16, "hex", dp()).is_err());
        // bad algorithm
        assert!(derive("bcrypt", "pw", "utf8", "salt", "utf8", 16, "hex", dp()).is_err());
        // zero length
        assert!(derive("pbkdf2", "pw", "utf8", "salt", "utf8", 0, "hex", dp()).is_err());
        // length too big
        assert!(derive("pbkdf2", "pw", "utf8", "salt", "utf8", 2000, "hex", dp()).is_err());
        // scrypt N not a power of two
        assert!(derive(
            "scrypt",
            "pw",
            "utf8",
            "salt",
            "utf8",
            16,
            "hex",
            DeriveParams { n: 1000, ..dp() }
        )
        .is_err());
        // Argon2 salt too short
        assert!(derive("argon2", "pw", "utf8", "short", "utf8", 16, "hex", dp()).is_err());
        // bad hash
        assert!(derive(
            "pbkdf2",
            "pw",
            "utf8",
            "salt",
            "utf8",
            16,
            "hex",
            DeriveParams {
                hash: "md5".into(),
                ..dp()
            }
        )
        .is_err());
        // bad output encoding
        assert!(derive(
            "pbkdf2",
            "pw",
            "utf8",
            "salt",
            "utf8",
            16,
            "octal",
            DeriveParams {
                iterations: 1,
                ..dp()
            }
        )
        .is_err());
        // bad hex salt
        assert!(derive("pbkdf2", "pw", "utf8", "zz", "hex", 16, "hex", dp()).is_err());
    }
}
