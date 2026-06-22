//! gizza-ai/generate-hotp core — generate counter-based one-time codes
//! (RFC 4226 HOTP) from a base32 secret and an explicit counter value.
//! Pure-Rust (`hmac` + `sha1`/`sha2` + `base32`), deterministic, no clock —
//! HOTP is event/counter-based (unlike TOTP, which derives the counter from
//! the current time). Works identically on every backend.

use hmac::{Hmac, Mac};

/// HMAC hash algorithm for the HOTP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algo {
    Sha1,
    Sha256,
    Sha512,
}

impl Algo {
    pub fn parse(s: &str) -> Result<Algo, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "sha1" | "" => Ok(Algo::Sha1),
            "sha256" => Ok(Algo::Sha256),
            "sha512" => Ok(Algo::Sha512),
            other => Err(format!("unknown algorithm '{other}' (use sha1, sha256, or sha512)")),
        }
    }
}

fn decode_secret(secret: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = secret.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if cleaned.is_empty() {
        return Err("secret is required".into());
    }
    base32::decode(base32::Alphabet::Rfc4648 { padding: false }, &cleaned.to_uppercase())
        .filter(|b| !b.is_empty())
        .ok_or_else(|| "secret is not valid base32".to_string())
}

fn hotp_code(key: &[u8], counter: u64, digits: u32, algo: Algo) -> String {
    let msg = counter.to_be_bytes();
    let digest: Vec<u8> = match algo {
        Algo::Sha1 => {
            let mut m = Hmac::<sha1::Sha1>::new_from_slice(key).expect("hmac key");
            m.update(&msg);
            m.finalize().into_bytes().to_vec()
        }
        Algo::Sha256 => {
            let mut m = Hmac::<sha2::Sha256>::new_from_slice(key).expect("hmac key");
            m.update(&msg);
            m.finalize().into_bytes().to_vec()
        }
        Algo::Sha512 => {
            let mut m = Hmac::<sha2::Sha512>::new_from_slice(key).expect("hmac key");
            m.update(&msg);
            m.finalize().into_bytes().to_vec()
        }
    };
    // Dynamic truncation (RFC 4226 §5.3).
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let bin = ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    let modulo = 10u32.pow(digits);
    format!("{:0width$}", bin % modulo, width = digits as usize)
}

/// A generated HOTP code for a given counter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotp {
    pub code: String,
    pub counter: u64,
}

/// Generate the HOTP code (RFC 4226) for `counter` using the base32 `secret`.
pub fn generate(secret: &str, counter: u64, digits: u32, algo: Algo) -> Result<Hotp, String> {
    if !(6..=8).contains(&digits) {
        return Err("digits must be 6, 7, or 8".into());
    }
    let key = decode_secret(secret)?;
    let code = hotp_code(&key, counter, digits, algo);
    Ok(Hotp { code, counter })
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 4226 Appendix D test vector secret "12345678901234567890" -> base32.
    const RFC_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    #[test]
    fn rfc4226_sha1_vectors() {
        // RFC 4226 Appendix D: 6-digit HOTP values for counters 0..=9 (SHA-1).
        let expected = [
            "755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583",
            "399871", "520489",
        ];
        for (counter, want) in expected.iter().enumerate() {
            assert_eq!(
                generate(RFC_SECRET, counter as u64, 6, Algo::Sha1).unwrap().code,
                *want,
                "counter {counter}"
            );
        }
    }

    #[test]
    fn eight_digit_counter0() {
        // Same dynamic-truncation 31-bit value as the 6-digit vector, 8 digits wide.
        assert_eq!(generate(RFC_SECRET, 0, 8, Algo::Sha1).unwrap().code, "84755224");
    }

    #[test]
    fn accepts_spaced_lowercase_secret() {
        let spaced = "gezd gnbv gy3t qojq gezd gnbv gy3t qojq";
        assert_eq!(generate(spaced, 1, 6, Algo::Sha1).unwrap().code, "287082");
    }

    #[test]
    fn algo_parse() {
        assert_eq!(Algo::parse("SHA-256").unwrap(), Algo::Sha256);
        assert_eq!(Algo::parse("").unwrap(), Algo::Sha1);
        assert!(Algo::parse("md5").is_err());
    }

    #[test]
    fn errors() {
        assert!(generate("", 0, 6, Algo::Sha1).is_err());
        assert!(generate("not base32 @@@", 0, 6, Algo::Sha1).is_err());
        assert!(generate(RFC_SECRET, 0, 5, Algo::Sha1).is_err()); // bad digits
        assert!(generate(RFC_SECRET, 0, 9, Algo::Sha1).is_err()); // bad digits
    }
}
