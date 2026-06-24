//! evp-derive core — reproduce OpenSSL's legacy `EVP_BytesToKey` key/IV
//! derivation from a password, salt, and hash. Pure-Rust (`md-5` + `sha1` +
//! `sha2`), deterministic, no I/O — shared by the chat skill block and the web
//! page.
//!
//! Algorithm (OpenSSL `EVP_BytesToKey`, the legacy KDF behind
//! `openssl enc -pass pass:… [-S salt]` without `-pbkdf2`):
//!   D_1 = HASH(password ‖ salt)
//!   D_i = HASH(D_{i-1} ‖ password ‖ salt)
//! with an optional iteration `count` (each block is re-hashed `count-1` extra
//! times: `D_i = HASH^count(...)`), concatenating D_1‖D_2‖… until at least
//! key_len + iv_len bytes are produced. The first key_len bytes are the key, the
//! next iv_len bytes are the IV. The salt, when present, is classically 8 bytes.

use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

/// Supported message-digest algorithms for EVP_BytesToKey.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hash {
    Md5,
    Sha1,
    Sha256,
    Sha512,
}

impl Hash {
    pub fn parse(s: &str) -> Result<Hash, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "md5" | "" => Ok(Hash::Md5),
            "sha1" => Ok(Hash::Sha1),
            "sha256" => Ok(Hash::Sha256),
            "sha512" => Ok(Hash::Sha512),
            other => Err(format!(
                "unknown hash '{other}' (use md5, sha1, sha256, or sha512)"
            )),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Hash::Md5 => "md5",
            Hash::Sha1 => "sha1",
            Hash::Sha256 => "sha256",
            Hash::Sha512 => "sha512",
        }
    }
    /// One pass of the digest over `data`.
    fn digest(self, data: &[u8]) -> Vec<u8> {
        match self {
            Hash::Md5 => Md5::digest(data).to_vec(),
            Hash::Sha1 => Sha1::digest(data).to_vec(),
            Hash::Sha256 => Sha256::digest(data).to_vec(),
            Hash::Sha512 => Sha512::digest(data).to_vec(),
        }
    }
}

/// Output encodings for the derived key/IV bytes.
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

/// The raw EVP_BytesToKey derivation. Returns `key_len + iv_len` bytes (key
/// concatenated with IV). `count` is the iteration count (OpenSSL default 1).
pub fn derive_bytes(
    password: &[u8],
    salt: &[u8],
    hash: Hash,
    key_len: usize,
    iv_len: usize,
    count: u32,
) -> Result<Vec<u8>, String> {
    if count == 0 {
        return Err("iteration count must be at least 1".into());
    }
    let need = key_len + iv_len;
    if need == 0 {
        return Err("key length + IV length must be at least 1 byte".into());
    }
    if need > 1024 {
        return Err("key length + IV length must be at most 1024 bytes".into());
    }
    let mut out: Vec<u8> = Vec::with_capacity(need);
    let mut di: Vec<u8> = Vec::new();
    while out.len() < need {
        // D_i = HASH(D_{i-1} ‖ password ‖ salt)
        let mut buf = Vec::with_capacity(di.len() + password.len() + salt.len());
        buf.extend_from_slice(&di);
        buf.extend_from_slice(password);
        buf.extend_from_slice(salt);
        di = hash.digest(&buf);
        // Apply the extra (count-1) iterations: each re-hashes the digest alone.
        for _ in 1..count {
            di = hash.digest(&di);
        }
        out.extend_from_slice(&di);
    }
    out.truncate(need);
    Ok(out)
}

/// High-level derive: decode the salt, run EVP_BytesToKey, split into key + IV,
/// and encode each. `hash` ∈ {md5, sha1, sha256, sha512}; `salt_encoding` ∈
/// {utf8, hex, base64}; `out_encoding` ∈ {hex, base64}.
///
/// Returns `(key, iv)` where `iv` is empty when `iv_len == 0`.
pub fn derive(
    password: &str,
    salt: &str,
    salt_encoding: &str,
    hash: &str,
    key_len: usize,
    iv_len: usize,
    count: u32,
    out_encoding: &str,
) -> Result<(String, String), String> {
    let h = Hash::parse(hash)?;
    let enc = Encoding::parse(out_encoding)?;
    let salt_bytes = decode_salt(salt, salt_encoding)?;
    let derived = derive_bytes(
        password.as_bytes(),
        &salt_bytes,
        h,
        key_len,
        iv_len,
        count,
    )?;
    let key = &derived[..key_len];
    let iv = &derived[key_len..];
    let encode = |b: &[u8]| match enc {
        Encoding::Hex => to_hex(b),
        Encoding::Base64 => base64_encode(b),
    };
    Ok((encode(key), if iv.is_empty() { String::new() } else { encode(iv) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verified against OpenSSL 3.5.6:
    //   openssl enc -aes-256-cbc -md md5 -nosalt -pass pass:hello -P
    // password "hello", no salt, MD5, key 32, IV 16.
    #[test]
    fn openssl_md5_no_salt_aes256() {
        let (key, iv) = derive("hello", "", "utf8", "md5", 32, 16, 1, "hex").unwrap();
        assert_eq!(
            key,
            "5d41402abc4b2a76b9719d911017c59228b46ed3c111e85102909b1cfb50ea0f"
        );
        assert_eq!(iv, "de37085f7eda3711fa2d9fe7e0fc29be");
    }

    // openssl enc -aes-256-cbc -md md5 -S 0102030405060708 -pass pass:hello -P
    // password "hello", 8-byte salt 0102030405060708, MD5, key 32, IV 16.
    #[test]
    fn openssl_md5_salt_aes256() {
        let (key, iv) =
            derive("hello", "0102030405060708", "hex", "md5", 32, 16, 1, "hex").unwrap();
        assert_eq!(
            key,
            "577943ad91305815e2bd7ed2d805eefab293e5d367f40c7dcb692b5f41bb5e08"
        );
        assert_eq!(iv, "f8fd4a50f857a2e9eb6474d5ef9ce67b");
    }

    // openssl enc -aes-256-cbc -md sha256 -nosalt -pass pass:password -P
    // password "password", no salt, SHA-256, key 32, IV 16.
    #[test]
    fn openssl_sha256_no_salt_aes256() {
        let (key, iv) =
            derive("password", "", "utf8", "sha256", 32, 16, 1, "hex").unwrap();
        assert_eq!(
            key,
            "5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8"
        );
        assert_eq!(iv, "3b02902846ffd32e92ff168b3f5d16b0");
    }

    // Single-block MD5 of "password" with no salt == MD5("password").
    #[test]
    fn md5_password_key16() {
        let (key, iv) = derive("password", "", "utf8", "md5", 16, 0, 1, "hex").unwrap();
        assert_eq!(key, "5f4dcc3b5aa765d61d8327deb882cf99");
        assert_eq!(iv, "");
    }

    #[test]
    fn salt_encodings_agree() {
        // "salt" as utf8 == hex 73616c74 == base64 c2FsdA==
        let a = derive("pw", "salt", "utf8", "md5", 16, 16, 1, "hex").unwrap();
        let b = derive("pw", "73616c74", "hex", "md5", 16, 16, 1, "hex").unwrap();
        let c = derive("pw", "c2FsdA==", "base64", "md5", 16, 16, 1, "hex").unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn base64_output_roundtrips() {
        let (key_hex, _) = derive("pw", "salt", "utf8", "sha256", 16, 0, 1, "hex").unwrap();
        let (key_b64, _) = derive("pw", "salt", "utf8", "sha256", 16, 0, 1, "base64").unwrap();
        assert_eq!(to_hex(&base64_decode(&key_b64).unwrap()), key_hex);
    }

    #[test]
    fn iteration_count_changes_output() {
        let a = derive("pw", "salt", "utf8", "md5", 16, 0, 1, "hex").unwrap();
        let b = derive("pw", "salt", "utf8", "md5", 16, 0, 2, "hex").unwrap();
        assert_ne!(a, b);
        // count=2 first block is HASH(HASH(pw‖salt))
        let inner = Hash::Md5.digest(b"pwsalt");
        let outer = Hash::Md5.digest(&inner);
        assert_eq!(b.0, to_hex(&outer[..16]));
    }

    #[test]
    fn errors() {
        assert!(derive("pw", "salt", "utf8", "rc4", 16, 0, 1, "hex").is_err()); // bad hash
        assert!(derive("pw", "salt", "utf8", "md5", 0, 0, 1, "hex").is_err()); // 0 total len
        assert!(derive("pw", "salt", "utf8", "md5", 16, 0, 0, "hex").is_err()); // 0 count
        assert!(derive("pw", "salt", "utf8", "md5", 600, 600, 1, "hex").is_err()); // too big
        assert!(derive("pw", "zz", "hex", "md5", 16, 0, 1, "hex").is_err()); // bad hex salt
        assert!(derive("pw", "salt", "utf8", "md5", 16, 0, 1, "b85").is_err()); // bad encoding
        assert!(derive("pw", "salt", "weird", "md5", 16, 0, 1, "hex").is_err()); // bad salt enc
    }
}
