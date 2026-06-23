//! hkdf-derive core — HKDF extract-and-expand key derivation (RFC 5869) with a
//! selectable HMAC hash (SHA-1 / SHA-256 / SHA-384 / SHA-512), input key material
//! (IKM), optional salt, optional info (context/application label), and output
//! length. Pure-Rust (`hkdf` + `hmac` + `sha1`/`sha2`), deterministic, no I/O —
//! shared by the chat skill block and the web page.

use hkdf::Hkdf;
use sha1::Sha1;
use sha2::{Sha256, Sha384, Sha512};

/// Supported HMAC hash functions for HKDF.
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
            "sha256" | "hmacsha256" => Ok(Hash::Sha256),
            "sha384" | "hmacsha384" => Ok(Hash::Sha384),
            "sha512" | "hmacsha512" => Ok(Hash::Sha512),
            other => Err(format!(
                "unknown hash '{other}' (use sha1, sha256, sha384, or sha512)"
            )),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Hash::Sha1 => "sha1",
            Hash::Sha256 => "sha256",
            Hash::Sha384 => "sha384",
            Hash::Sha512 => "sha512",
        }
    }
    /// Output size of the underlying hash (HashLen), in bytes.
    pub fn hash_len(self) -> usize {
        match self {
            Hash::Sha1 => 20,
            Hash::Sha256 => 32,
            Hash::Sha384 => 48,
            Hash::Sha512 => 64,
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

/// Decode a string of input bytes (IKM, salt, or info) per `encoding`
/// ("utf8" | "hex" | "base64").
fn decode_bytes(s: &str, encoding: &str, what: &str) -> Result<Vec<u8>, String> {
    match encoding.trim().to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" | "text" | "" => Ok(s.as_bytes().to_vec()),
        "hex" => decode_hex(s).map_err(|e| format!("{what}: {e}")),
        "base64" | "b64" => base64_decode(s).map_err(|e| format!("{what}: {e}")),
        other => Err(format!(
            "unknown encoding '{other}' for {what} (use utf8, hex, or base64)"
        )),
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let s: String = s.split_whitespace().collect();
    if s.len() % 2 != 0 {
        return Err("hex input must have an even number of digits".into());
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

/// HKDF-Extract: PRK = HMAC-Hash(salt, IKM). When `salt` is empty, HKDF uses a
/// string of HashLen zero octets as the salt (RFC 5869 §2.2).
pub fn extract_bytes(salt: &[u8], ikm: &[u8], hash: Hash) -> Vec<u8> {
    match hash {
        Hash::Sha1 => Hkdf::<Sha1>::extract(Some(salt), ikm).0.to_vec(),
        Hash::Sha256 => Hkdf::<Sha256>::extract(Some(salt), ikm).0.to_vec(),
        Hash::Sha384 => Hkdf::<Sha384>::extract(Some(salt), ikm).0.to_vec(),
        Hash::Sha512 => Hkdf::<Sha512>::extract(Some(salt), ikm).0.to_vec(),
    }
}

/// Full HKDF (extract-then-expand): derive `out_len` bytes of output key
/// material (OKM) from `ikm`, `salt`, and `info` (RFC 5869 §2).
pub fn derive_bytes(
    ikm: &[u8],
    salt: &[u8],
    info: &[u8],
    hash: Hash,
    out_len: usize,
) -> Result<Vec<u8>, String> {
    if out_len == 0 {
        return Err("output length (bytes) must be at least 1".into());
    }
    // RFC 5869 §2.3: L <= 255 * HashLen.
    let max = 255 * hash.hash_len();
    if out_len > max {
        return Err(format!(
            "output length (bytes) must be at most {max} for {} (255 * HashLen)",
            hash.label()
        ));
    }
    let mut out = vec![0u8; out_len];
    let res = match hash {
        Hash::Sha1 => Hkdf::<Sha1>::new(Some(salt), ikm).expand(info, &mut out),
        Hash::Sha256 => Hkdf::<Sha256>::new(Some(salt), ikm).expand(info, &mut out),
        Hash::Sha384 => Hkdf::<Sha384>::new(Some(salt), ikm).expand(info, &mut out),
        Hash::Sha512 => Hkdf::<Sha512>::new(Some(salt), ikm).expand(info, &mut out),
    };
    res.map_err(|_| "HKDF expand failed (output too long)".to_string())?;
    Ok(out)
}

/// High-level derive: decode IKM/salt/info, run HKDF, and encode the result.
/// `hash` ∈ {sha1, sha256, sha384, sha512}; `*_encoding` ∈ {utf8, hex, base64};
/// `out_encoding` ∈ {hex, base64}.
#[allow(clippy::too_many_arguments)]
pub fn derive(
    ikm: &str,
    ikm_encoding: &str,
    salt: &str,
    salt_encoding: &str,
    info: &str,
    info_encoding: &str,
    hash: &str,
    out_len: usize,
    out_encoding: &str,
) -> Result<String, String> {
    if ikm.is_empty() {
        return Err("input key material (ikm) is required".into());
    }
    let h = Hash::parse(hash)?;
    let enc = Encoding::parse(out_encoding)?;
    let ikm_b = decode_bytes(ikm, ikm_encoding, "ikm")?;
    if ikm_b.is_empty() {
        return Err("input key material (ikm) is required".into());
    }
    let salt_b = decode_bytes(salt, salt_encoding, "salt")?;
    let info_b = decode_bytes(info, info_encoding, "info")?;
    let okm = derive_bytes(&ikm_b, &salt_b, &info_b, h, out_len)?;
    Ok(match enc {
        Encoding::Hex => to_hex(&okm),
        Encoding::Base64 => base64_encode(&okm),
    })
}

/// Extract-only: compute the pseudorandom key (PRK) and return it encoded.
pub fn extract(
    ikm: &str,
    ikm_encoding: &str,
    salt: &str,
    salt_encoding: &str,
    hash: &str,
    out_encoding: &str,
) -> Result<String, String> {
    if ikm.is_empty() {
        return Err("input key material (ikm) is required".into());
    }
    let h = Hash::parse(hash)?;
    let enc = Encoding::parse(out_encoding)?;
    let ikm_b = decode_bytes(ikm, ikm_encoding, "ikm")?;
    if ikm_b.is_empty() {
        return Err("input key material (ikm) is required".into());
    }
    let salt_b = decode_bytes(salt, salt_encoding, "salt")?;
    let prk = extract_bytes(&salt_b, &ikm_b, h);
    Ok(match enc {
        Encoding::Hex => to_hex(&prk),
        Encoding::Base64 => base64_encode(&prk),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 5869 Appendix A.1 — SHA-256, with salt and info, L=42.
    #[test]
    fn rfc5869_case1_sha256() {
        let ikm = decode_hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b").unwrap();
        let salt = decode_hex("000102030405060708090a0b0c").unwrap();
        let info = decode_hex("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let prk = extract_bytes(&salt, &ikm, Hash::Sha256);
        assert_eq!(
            to_hex(&prk),
            "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
        );
        let okm = derive_bytes(&ikm, &salt, &info, Hash::Sha256, 42).unwrap();
        assert_eq!(
            to_hex(&okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }

    // RFC 5869 Appendix A.3 — SHA-256, empty salt + empty info, L=42.
    #[test]
    fn rfc5869_case3_sha256_empty() {
        let ikm = decode_hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b").unwrap();
        let prk = extract_bytes(&[], &ikm, Hash::Sha256);
        assert_eq!(
            to_hex(&prk),
            "19ef24a32c717b167f33a91d6f648bdf96596776afdb6377ac434c1c293ccb04"
        );
        let okm = derive_bytes(&ikm, &[], &[], Hash::Sha256, 42).unwrap();
        assert_eq!(
            to_hex(&okm),
            "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8"
        );
    }

    // RFC 5869 Appendix A.4 — SHA-1, with salt and info, L=42.
    #[test]
    fn rfc5869_case4_sha1() {
        let ikm = decode_hex("0b0b0b0b0b0b0b0b0b0b0b").unwrap();
        let salt = decode_hex("000102030405060708090a0b0c").unwrap();
        let info = decode_hex("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let prk = extract_bytes(&salt, &ikm, Hash::Sha1);
        assert_eq!(to_hex(&prk), "9b6c18c432a7bf8f0e71c8eb88f4b30baa2ba243");
        let okm = derive_bytes(&ikm, &salt, &info, Hash::Sha1, 42).unwrap();
        assert_eq!(
            to_hex(&okm),
            "085a01ea1b10f36933068b56efa5ad81a4f14b822f5b091568a9cdd4f155fda2c22e422478d305f3f896"
        );
    }

    // RFC 5869 Appendix A.6 — SHA-1, empty salt + empty info, L=42.
    #[test]
    fn rfc5869_case6_sha1_empty() {
        let ikm = decode_hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b").unwrap();
        let okm = derive_bytes(&ikm, &[], &[], Hash::Sha1, 42).unwrap();
        assert_eq!(
            to_hex(&okm),
            "0ac1af7002b3d761d1e55298da9d0506b9ae52057220a306e07b6b87e8df21d0ea00033de03984d34918"
        );
    }

    #[test]
    fn high_level_derive_hex_matches_rfc() {
        // Case A.1 via the high-level string API (hex IKM/salt/info).
        let out = derive(
            "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b",
            "hex",
            "000102030405060708090a0b0c",
            "hex",
            "f0f1f2f3f4f5f6f7f8f9",
            "hex",
            "sha256",
            42,
            "hex",
        )
        .unwrap();
        assert_eq!(
            out,
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }

    #[test]
    fn extract_high_level_matches_rfc() {
        let prk = extract(
            "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b",
            "hex",
            "000102030405060708090a0b0c",
            "hex",
            "sha256",
            "hex",
        )
        .unwrap();
        assert_eq!(
            prk,
            "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
        );
    }

    #[test]
    fn utf8_inputs_and_base64_output() {
        // Deterministic round-trip: utf8 IKM/info, derive twice, compare; check
        // base64 decodes back to the hex output.
        let hexout = derive("secret ikm", "utf8", "", "utf8", "app context", "utf8", "sha256", 16, "hex").unwrap();
        let b64out = derive("secret ikm", "utf8", "", "utf8", "app context", "utf8", "sha256", 16, "base64").unwrap();
        let raw = base64_decode(&b64out).unwrap();
        assert_eq!(to_hex(&raw), hexout);
        assert_eq!(raw.len(), 16);
    }

    #[test]
    fn info_changes_output() {
        let a = derive("ikm", "utf8", "salt", "utf8", "ctx-a", "utf8", "sha256", 32, "hex").unwrap();
        let b = derive("ikm", "utf8", "salt", "utf8", "ctx-b", "utf8", "sha256", 32, "hex").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn empty_salt_allowed() {
        let out = derive("ikm", "utf8", "", "utf8", "", "utf8", "sha256", 32, "hex").unwrap();
        assert_eq!(out.len(), 64);
    }

    #[test]
    fn max_length_enforced() {
        // 255 * 32 = 8160 is OK for sha256; one more is an error.
        assert!(derive_bytes(b"ikm", b"", b"", Hash::Sha256, 8160).is_ok());
        assert!(derive_bytes(b"ikm", b"", b"", Hash::Sha256, 8161).is_err());
    }

    #[test]
    fn errors() {
        assert!(derive("", "utf8", "", "utf8", "", "utf8", "sha256", 32, "hex").is_err()); // empty ikm
        assert!(derive("ikm", "utf8", "", "utf8", "", "utf8", "sha256", 0, "hex").is_err()); // 0 len
        assert!(derive("ikm", "utf8", "", "utf8", "", "utf8", "md5", 32, "hex").is_err()); // bad hash
        assert!(derive("zz", "hex", "", "utf8", "", "utf8", "sha256", 32, "hex").is_err()); // bad hex ikm
        assert!(derive("ikm", "utf8", "", "utf8", "", "utf8", "sha256", 32, "ascii").is_err()); // bad out enc
    }
}
