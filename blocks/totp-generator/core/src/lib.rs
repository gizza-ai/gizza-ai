//! gizza-ai/totp-generator core — generate time-based one-time codes (RFC 6238
//! TOTP / RFC 4226 HOTP) from a base32 secret. Pure-Rust (`hmac` + `sha1`/`sha2`
//! + `base32`). Time is passed in explicitly so the function is deterministic and
//! works on every backend (the chat block / CLI supply the real clock; the page
//! supplies `Date.now()`).

use hmac::{Hmac, Mac};

/// HMAC hash algorithm for the TOTP.
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

fn hotp(key: &[u8], counter: u64, digits: u32, algo: Algo) -> String {
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
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let bin = ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    let modulo = 10u32.pow(digits);
    format!("{:0width$}", bin % modulo, width = digits as usize)
}

/// A generated code and how long (seconds) it remains valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Totp {
    pub code: String,
    pub seconds_remaining: u64,
    pub period: u64,
}

/// Generate the TOTP code valid at `unix_time` (seconds since the epoch).
pub fn generate(
    secret: &str,
    unix_time: u64,
    digits: u32,
    period: u64,
    algo: Algo,
) -> Result<Totp, String> {
    if !(6..=8).contains(&digits) {
        return Err("digits must be 6, 7, or 8".into());
    }
    if period == 0 {
        return Err("period must be at least 1 second".into());
    }
    let key = decode_secret(secret)?;
    let counter = unix_time / period;
    let code = hotp(&key, counter, digits, algo);
    let seconds_remaining = period - (unix_time % period);
    Ok(Totp { code, seconds_remaining, period })
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 6238 test vector secret "12345678901234567890" -> base32.
    const RFC_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    #[test]
    fn rfc6238_sha1_vectors() {
        // From RFC 6238 Appendix B (SHA-1, 8 digits, period 30).
        assert_eq!(generate(RFC_SECRET, 59, 8, 30, Algo::Sha1).unwrap().code, "94287082");
        assert_eq!(
            generate(RFC_SECRET, 1111111109, 8, 30, Algo::Sha1).unwrap().code,
            "07081804"
        );
        assert_eq!(
            generate(RFC_SECRET, 1111111111, 8, 30, Algo::Sha1).unwrap().code,
            "14050471"
        );
    }

    #[test]
    fn six_digit_default() {
        let t = generate(RFC_SECRET, 59, 6, 30, Algo::Sha1).unwrap();
        // 8-digit code "94287082" truncates to last 6: "287082"
        assert_eq!(t.code, "287082");
        assert_eq!(t.code.len(), 6);
    }

    #[test]
    fn seconds_remaining() {
        // at t=59, counter window [30,60), remaining = 60-59 = 1
        assert_eq!(generate(RFC_SECRET, 59, 6, 30, Algo::Sha1).unwrap().seconds_remaining, 1);
        // at t=30, remaining = 30
        assert_eq!(generate(RFC_SECRET, 30, 6, 30, Algo::Sha1).unwrap().seconds_remaining, 30);
    }

    #[test]
    fn accepts_spaced_lowercase_secret() {
        let spaced = "gezd gnbv gy3t qojq gezd gnbv gy3t qojq";
        assert_eq!(generate(spaced, 59, 8, 30, Algo::Sha1).unwrap().code, "94287082");
    }

    #[test]
    fn errors() {
        assert!(generate("", 0, 6, 30, Algo::Sha1).is_err());
        assert!(generate("not base32 @@@", 0, 6, 30, Algo::Sha1).is_err());
        assert!(generate(RFC_SECRET, 0, 5, 30, Algo::Sha1).is_err()); // bad digits
        assert!(generate(RFC_SECRET, 0, 6, 0, Algo::Sha1).is_err()); // bad period
        assert!(Algo::parse("md5").is_err());
    }
}
