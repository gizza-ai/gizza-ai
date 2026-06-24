//! otpauth-uri core — build a standard `otpauth://` provisioning URI (the Key Uri
//! Format used by Google Authenticator and other authenticator apps) from an issuer,
//! account name, and base32 secret. Pure-Rust, no I/O, no external deps. Percent-
//! encoding is hand-rolled (RFC 3986 unreserved set) so the crate stays dependency-free
//! and instantiates on every backend.

/// OTP type: time-based (TOTP, RFC 6238) or counter-based (HOTP, RFC 4226).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtpType {
    Totp,
    Hotp,
}

impl OtpType {
    pub fn parse(s: &str) -> Result<OtpType, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "totp" | "" => Ok(OtpType::Totp),
            "hotp" => Ok(OtpType::Hotp),
            other => Err(format!("unknown type '{other}' (use totp or hotp)")),
        }
    }
    fn label(self) -> &'static str {
        match self {
            OtpType::Totp => "totp",
            OtpType::Hotp => "hotp",
        }
    }
}

/// HMAC hash algorithm advertised in the URI.
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
    fn label(self) -> &'static str {
        match self {
            Algo::Sha1 => "SHA1",
            Algo::Sha256 => "SHA256",
            Algo::Sha512 => "SHA512",
        }
    }
}

/// Percent-encode `s` keeping the RFC 3986 unreserved set (`A-Z a-z 0-9 - . _ ~`)
/// literal and encoding everything else as `%XX`. Space becomes `%20` (not `+`).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Validate + normalise a base32 secret: strip spaces/dashes, uppercase, and check
/// it only contains RFC 4648 base32 alphabet characters (`A-Z`, `2-7`, optional `=`).
fn normalize_secret(secret: &str) -> Result<String, String> {
    let cleaned: String =
        secret.chars().filter(|c| !c.is_whitespace() && *c != '-').collect::<String>().to_uppercase();
    if cleaned.is_empty() {
        return Err("secret is required".into());
    }
    if !cleaned.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7' | '=')) {
        return Err("secret must be base32 (characters A-Z and 2-7 only)".into());
    }
    Ok(cleaned)
}

/// Parameters for building an otpauth URI.
#[derive(Debug, Clone)]
pub struct OtpAuth {
    pub otp_type: OtpType,
    pub issuer: String,
    pub account: String,
    pub secret: String,
    pub algorithm: Algo,
    pub digits: u32,
    /// TOTP period in seconds (ignored for HOTP).
    pub period: u64,
    /// HOTP initial counter (ignored for TOTP).
    pub counter: u64,
}

/// Build the `otpauth://` provisioning URI.
///
/// Format (Key Uri Format):
/// `otpauth://TYPE/[ISSUER:]ACCOUNT?secret=...&issuer=...&algorithm=...&digits=...&period=...|counter=...`
pub fn build(p: &OtpAuth) -> Result<String, String> {
    if !(6..=8).contains(&p.digits) {
        return Err("digits must be 6, 7, or 8".into());
    }
    if matches!(p.otp_type, OtpType::Totp) && p.period == 0 {
        return Err("period must be at least 1 second".into());
    }
    let account = p.account.trim();
    if account.is_empty() {
        return Err("account name is required".into());
    }
    if account.contains(':') {
        return Err("account name must not contain a colon".into());
    }
    let issuer = p.issuer.trim();
    if issuer.contains(':') {
        return Err("issuer must not contain a colon".into());
    }
    let secret = normalize_secret(&p.secret)?;

    // Label = "Issuer:Account" (issuer prefix recommended), each component encoded.
    let label = if issuer.is_empty() {
        percent_encode(account)
    } else {
        format!("{}:{}", percent_encode(issuer), percent_encode(account))
    };

    let mut query = format!("secret={}", secret);
    if !issuer.is_empty() {
        query.push_str(&format!("&issuer={}", percent_encode(issuer)));
    }
    query.push_str(&format!("&algorithm={}", p.algorithm.label()));
    query.push_str(&format!("&digits={}", p.digits));
    match p.otp_type {
        OtpType::Totp => query.push_str(&format!("&period={}", p.period)),
        OtpType::Hotp => query.push_str(&format!("&counter={}", p.counter)),
    }

    Ok(format!("otpauth://{}/{}?{}", p.otp_type.label(), label, query))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn totp(issuer: &str, account: &str, secret: &str) -> OtpAuth {
        OtpAuth {
            otp_type: OtpType::Totp,
            issuer: issuer.into(),
            account: account.into(),
            secret: secret.into(),
            algorithm: Algo::Sha1,
            digits: 6,
            period: 30,
            counter: 0,
        }
    }

    #[test]
    fn basic_totp_uri() {
        let uri = build(&totp("Example", "alice@google.com", "JBSWY3DPEHPK3PXP")).unwrap();
        assert_eq!(
            uri,
            "otpauth://totp/Example:alice%40google.com?secret=JBSWY3DPEHPK3PXP&issuer=Example&algorithm=SHA1&digits=6&period=30"
        );
    }

    #[test]
    fn no_issuer_label() {
        let uri = build(&totp("", "alice", "JBSWY3DPEHPK3PXP")).unwrap();
        assert_eq!(uri, "otpauth://totp/alice?secret=JBSWY3DPEHPK3PXP&algorithm=SHA1&digits=6&period=30");
    }

    #[test]
    fn issuer_with_space_is_encoded() {
        let uri = build(&totp("ACME Co", "bob", "JBSWY3DPEHPK3PXP")).unwrap();
        assert!(uri.starts_with("otpauth://totp/ACME%20Co:bob?"));
        assert!(uri.contains("&issuer=ACME%20Co"));
    }

    #[test]
    fn hotp_uses_counter() {
        let mut p = totp("Example", "alice", "JBSWY3DPEHPK3PXP");
        p.otp_type = OtpType::Hotp;
        p.counter = 5;
        let uri = build(&p).unwrap();
        assert!(uri.starts_with("otpauth://hotp/"));
        assert!(uri.ends_with("&counter=5"));
        assert!(!uri.contains("period="));
    }

    #[test]
    fn custom_algorithm_digits_period() {
        let mut p = totp("Example", "alice", "JBSWY3DPEHPK3PXP");
        p.algorithm = Algo::Sha256;
        p.digits = 8;
        p.period = 60;
        let uri = build(&p).unwrap();
        assert!(uri.contains("&algorithm=SHA256"));
        assert!(uri.contains("&digits=8"));
        assert!(uri.contains("&period=60"));
    }

    #[test]
    fn secret_normalized() {
        let uri = build(&totp("X", "y", "jbsw y3dp ehpk 3pxp")).unwrap();
        assert!(uri.contains("secret=JBSWY3DPEHPK3PXP"));
    }

    #[test]
    fn errors() {
        assert!(build(&totp("X", "", "JBSWY3DPEHPK3PXP")).is_err()); // empty account
        assert!(build(&totp("X", "y", "")).is_err()); // empty secret
        assert!(build(&totp("X", "y", "not!base32")).is_err()); // bad secret
        assert!(build(&totp("X", "a:b", "JBSWY3DPEHPK3PXP")).is_err()); // colon in account
        let mut p = totp("X", "y", "JBSWY3DPEHPK3PXP");
        p.digits = 5;
        assert!(build(&p).is_err()); // bad digits
        p.digits = 6;
        p.period = 0;
        assert!(build(&p).is_err()); // bad period
        assert!(OtpType::parse("foo").is_err());
        assert!(Algo::parse("md5").is_err());
    }
}
