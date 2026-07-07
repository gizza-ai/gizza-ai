//! gizza-ai/pii-tokenize core — replace detected PII in text with **deterministic,
//! format-preserving pseudonyms**. The same input value (under the same secret)
//! always maps to the same token, so the text stays linkable/joinable while being
//! de-identified. Distinct from `redact-pii`, which masks (`***`) or labels
//! (`[EMAIL]`) and therefore destroys linkability.
//!
//! Pure-Rust: `regex` for detection (same categories as redact-pii), `hmac`+`sha2`
//! for the keyed keystream that drives per-character substitution.
//!
//! Substitution rules keep each value's shape and length:
//! - digits → deterministic digits, ASCII letters → deterministic same-case letters,
//!   punctuation/structure (`@ . - + ( ) : space`) kept verbatim;
//! - **credit cards** additionally get a recomputed Luhn check digit, so the token
//!   is still a checksum-valid card number;
//! - **IPv4** octets are each mapped to a valid `0–255` value (dotted-quad shape kept);
//! - **IPv6** hextets are substituted nibble-wise, so every group stays valid hex.

use std::sync::OnceLock;

use hmac::{Hmac, Mac};
use regex::Regex;
use serde::Serialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Built-in key used when the caller supplies no secret. Tokens are still
/// deterministic; supplying a secret makes them stable across sessions and unique
/// to whoever holds that secret.
const DEFAULT_KEY: &[u8] = b"gizza-ai/pii-tokenize/v1";

/// Per-category counts of tokenizations made.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub email: usize,
    pub phone: usize,
    pub ipv4: usize,
    pub ipv6: usize,
    pub credit_card: usize,
    pub ssn: usize,
}

/// Result of a tokenization pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tokenization {
    pub tokenized: String,
    pub counts: Counts,
    pub total: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Email,
    Phone,
    Ipv4,
    Ipv6,
    CreditCard,
    Ssn,
}

struct Patterns {
    email: Regex,
    ssn: Regex,
    ipv4: Regex,
    ipv6: Regex,
    credit_card: Regex,
    phone: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        email: Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap(),
        ssn: Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap(),
        ipv4: Regex::new(
            r"\b(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\b",
        )
        .unwrap(),
        // Full 8-group IPv6 or a `::`-compressed form.
        ipv6: Regex::new(
            r"(?:[A-Fa-f0-9]{1,4}:){7}[A-Fa-f0-9]{1,4}|(?:[A-Fa-f0-9]{1,4}:){1,7}:(?:[A-Fa-f0-9]{1,4}:?){0,6}",
        )
        .unwrap(),
        // 13-19 digit runs allowing space/dash separators (must start and end on
        // a digit); Luhn-checked below.
        credit_card: Regex::new(r"\b\d(?:[ -]?\d){12,18}\b").unwrap(),
        // Optional country code, optional area-code parens, US-style 10 digits.
        phone: Regex::new(r"(?:\+?\d{1,3}[\s.\-]?)?(?:\(\d{3}\)|\d{3})[\s.\-]?\d{3}[\s.\-]?\d{4}")
            .unwrap(),
    })
}

fn luhn_ok(digits: &str) -> bool {
    let ds: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if ds.len() < 13 || ds.len() > 19 {
        return false;
    }
    let mut sum = 0u32;
    for (i, &d) in ds.iter().rev().enumerate() {
        if i % 2 == 1 {
            let dd = d * 2;
            sum += if dd > 9 { dd - 9 } else { dd };
        } else {
            sum += d;
        }
    }
    sum % 10 == 0
}

/// Parse a checkbox/CLI boolean. Kept in `core` so every surface shares one
/// truthiness rule (the page checkbox sends `"true"`/`"false"`).
pub fn parse_bool(s: &str) -> Result<bool, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" => Ok(true),
        "false" | "0" | "off" | "no" => Ok(false),
        other => Err(format!("invalid boolean '{other}' (use true or false)")),
    }
}

/// Deterministic keystream of `n` bytes: `HMAC-SHA256(key, value || counter)`
/// concatenated over increasing counters. Keyed on the exact matched value, so the
/// same value always yields the same stream (linkability).
fn keystream(key: &[u8], value: &str, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n);
    let mut counter: u32 = 0;
    while out.len() < n {
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(value.as_bytes());
        mac.update(&counter.to_be_bytes());
        out.extend_from_slice(&mac.finalize().into_bytes());
        counter += 1;
    }
    out.truncate(n);
    out
}

/// Character-class substitution: digit→digit, a-z→a-z, A-Z→A-Z, everything else
/// kept. `portion` is what gets walked; `seed` keys the keystream (so a substring —
/// e.g. an email local part — is still keyed on the whole value for determinism).
fn substitute(portion: &str, key: &[u8], seed: &str) -> String {
    let n = portion.chars().filter(|c| c.is_ascii_alphanumeric()).count();
    let ks = keystream(key, seed, n.max(1));
    let mut out = String::with_capacity(portion.len());
    let mut i = 0usize;
    for c in portion.chars() {
        if c.is_ascii_digit() {
            let b = ks[i];
            i += 1;
            out.push(char::from(b'0' + (b % 10)));
        } else if c.is_ascii_lowercase() {
            let b = ks[i];
            i += 1;
            out.push(char::from(b'a' + (b % 26)));
        } else if c.is_ascii_uppercase() {
            let b = ks[i];
            i += 1;
            out.push(char::from(b'A' + (b % 26)));
        } else {
            out.push(c);
        }
    }
    out
}

/// Luhn check digit for `payload` (all digits except the trailing check digit,
/// left-to-right). Appended at the rightmost position (Luhn position 1).
fn luhn_check_digit(payload: &[u32]) -> u32 {
    let mut sum = 0u32;
    for (i, &d) in payload.iter().rev().enumerate() {
        // rightmost payload digit is Luhn position 2 (doubled), then alternates.
        if i % 2 == 0 {
            let dd = d * 2;
            sum += if dd > 9 { dd - 9 } else { dd };
        } else {
            sum += d;
        }
    }
    (10 - (sum % 10)) % 10
}

/// Rewrite the last digit of `s` so the whole run passes Luhn.
fn fix_luhn(s: &mut String) {
    let byte_pos: Vec<usize> = s
        .char_indices()
        .filter(|(_, c)| c.is_ascii_digit())
        .map(|(i, _)| i)
        .collect();
    if byte_pos.len() < 2 {
        return;
    }
    let digits: Vec<u32> = byte_pos
        .iter()
        .map(|&i| s[i..].chars().next().unwrap().to_digit(10).unwrap())
        .collect();
    let check = luhn_check_digit(&digits[..digits.len() - 1]);
    let last = *byte_pos.last().unwrap();
    // ASCII digit → single byte, safe in-place replace.
    unsafe {
        s.as_bytes_mut()[last] = b'0' + check as u8;
    }
}

fn tokenize_ipv4(value: &str, key: &[u8]) -> String {
    let parts: Vec<&str> = value.split('.').collect();
    let ks = keystream(key, value, parts.len().max(1));
    parts
        .iter()
        .enumerate()
        .map(|(i, _)| ks[i].to_string()) // ks byte is already 0..=255 → valid octet
        .collect::<Vec<_>>()
        .join(".")
}

fn tokenize_ipv6(value: &str, key: &[u8]) -> String {
    let n = value.chars().filter(|c| c.is_ascii_hexdigit()).count();
    let ks = keystream(key, value, n.max(1));
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(value.len());
    let mut i = 0usize;
    for c in value.chars() {
        if c.is_ascii_hexdigit() {
            let h = HEX[(ks[i] % 16) as usize] as char;
            i += 1;
            out.push(if c.is_ascii_uppercase() {
                h.to_ascii_uppercase()
            } else {
                h
            });
        } else {
            out.push(c); // ':'
        }
    }
    out
}

fn tokenize_value(value: &str, key: &[u8], kind: Kind, preserve_email_domain: bool) -> String {
    match kind {
        Kind::Ipv4 => tokenize_ipv4(value, key),
        Kind::Ipv6 => tokenize_ipv6(value, key),
        Kind::Email if preserve_email_domain => {
            if let Some(at) = value.rfind('@') {
                let local = &value[..at];
                let domain = &value[at..]; // includes '@'
                format!("{}{}", substitute(local, key, value), domain)
            } else {
                substitute(value, key, value)
            }
        }
        Kind::CreditCard => {
            let mut s = substitute(value, key, value);
            fix_luhn(&mut s);
            s
        }
        _ => substitute(value, key, value),
    }
}

/// Tokenize PII in `text`.
///
/// - `secret` keys the deterministic mapping; empty uses a built-in default key.
/// - `preserve_email_domain` keeps the part after `@` in emails (only the local
///   part is pseudonymized) when true; when false the whole address is substituted.
pub fn tokenize(text: &str, secret: &str, preserve_email_domain: bool) -> Tokenization {
    let key: &[u8] = if secret.is_empty() {
        DEFAULT_KEY
    } else {
        secret.as_bytes()
    };
    let p = patterns();
    let mut spans: Vec<(usize, usize, Kind)> = Vec::new();
    for m in p.email.find_iter(text) {
        spans.push((m.start(), m.end(), Kind::Email));
    }
    for m in p.ssn.find_iter(text) {
        spans.push((m.start(), m.end(), Kind::Ssn));
    }
    for m in p.ipv4.find_iter(text) {
        spans.push((m.start(), m.end(), Kind::Ipv4));
    }
    for m in p.ipv6.find_iter(text) {
        if text[m.start()..m.end()].matches(':').count() >= 2 {
            spans.push((m.start(), m.end(), Kind::Ipv6));
        }
    }
    for m in p.credit_card.find_iter(text) {
        if luhn_ok(&text[m.start()..m.end()]) {
            spans.push((m.start(), m.end(), Kind::CreditCard));
        }
    }
    for m in p.phone.find_iter(text) {
        spans.push((m.start(), m.end(), Kind::Phone));
    }

    // De-overlap: earliest start first, longest on ties.
    spans.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut accepted: Vec<(usize, usize, Kind)> = Vec::new();
    let mut last_end = 0usize;
    for (s, e, k) in spans {
        if s >= last_end {
            accepted.push((s, e, k));
            last_end = e;
        }
    }

    let mut counts = Counts::default();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for (s, e, k) in &accepted {
        out.push_str(&text[cursor..*s]);
        out.push_str(&tokenize_value(&text[*s..*e], key, *k, preserve_email_domain));
        cursor = *e;
        match k {
            Kind::Email => counts.email += 1,
            Kind::Phone => counts.phone += 1,
            Kind::Ipv4 => counts.ipv4 += 1,
            Kind::Ipv6 => counts.ipv6 += 1,
            Kind::CreditCard => counts.credit_card += 1,
            Kind::Ssn => counts.ssn += 1,
        }
    }
    out.push_str(&text[cursor..]);

    let total = accepted.len();
    Tokenization {
        tokenized: out,
        counts,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_emailish(s: &str) -> bool {
        // one '@', a dot after it, only email chars.
        let at = match s.find('@') {
            Some(i) => i,
            None => return false,
        };
        s.matches('@').count() == 1
            && s[at..].contains('.')
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "._%+-@".contains(c))
    }

    #[test]
    fn email_is_tokenized_deterministically_and_linkably() {
        let text = "ping ada@example.com and ada@example.com later";
        let a = tokenize(text, "", true);
        assert_eq!(a.counts.email, 2);
        assert_eq!(a.total, 2);
        // original value must be gone…
        assert!(!a.tokenized.contains("ada@example.com"));
        // …the two occurrences must map to the SAME token (linkability)…
        let toks: Vec<&str> = a
            .tokenized
            .split_whitespace()
            .filter(|w| w.contains('@'))
            .collect();
        assert_eq!(toks.len(), 2);
        assert_eq!(toks[0], toks[1]);
        // …the token is still email-shaped and keeps the domain (preserve=true).
        assert!(is_emailish(toks[0]), "not email-shaped: {}", toks[0]);
        assert!(toks[0].ends_with("@example.com"));
        // determinism across calls
        let b = tokenize(text, "", true);
        assert_eq!(a.tokenized, b.tokenized);
    }

    #[test]
    fn preserve_email_domain_toggle() {
        let kept = tokenize("x john.doe@acme.com y", "s3cret", true);
        assert!(kept.tokenized.contains("@acme.com"));
        let dropped = tokenize("x john.doe@acme.com y", "s3cret", false);
        assert!(!dropped.tokenized.contains("@acme.com"));
        assert!(dropped.tokenized.contains('@')); // still email-shaped
        assert!(!dropped.tokenized.contains("john.doe"));
    }

    #[test]
    fn different_secret_gives_different_token() {
        let a = tokenize("ada@example.com", "key-a", true);
        let b = tokenize("ada@example.com", "key-b", true);
        assert_ne!(a.tokenized, b.tokenized);
    }

    #[test]
    fn credit_card_token_stays_luhn_valid() {
        let r = tokenize("card 4111 1111 1111 1111 end", "", true);
        assert_eq!(r.counts.credit_card, 1);
        assert!(!r.tokenized.contains("4111 1111 1111 1111"));
        // extract the token and confirm it is a valid Luhn card.
        let tok = r
            .tokenized
            .strip_prefix("card ")
            .and_then(|s| s.strip_suffix(" end"))
            .unwrap();
        assert!(luhn_ok(tok), "token not Luhn-valid: {tok}");
        // format preserved: same length + same separator positions.
        assert_eq!(tok.len(), "4111 1111 1111 1111".len());
    }

    #[test]
    fn ipv4_octets_stay_in_range() {
        let r = tokenize("host 192.168.0.1 up", "", true);
        assert_eq!(r.counts.ipv4, 1);
        assert!(!r.tokenized.contains("192.168.0.1"));
        let tok = r
            .tokenized
            .strip_prefix("host ")
            .and_then(|s| s.strip_suffix(" up"))
            .unwrap();
        let octets: Vec<u32> = tok.split('.').map(|o| o.parse().unwrap()).collect();
        assert_eq!(octets.len(), 4);
        assert!(octets.iter().all(|&o| o <= 255));
    }

    #[test]
    fn ipv6_stays_valid_hex() {
        let r = tokenize("addr 2001:0db8:85a3:0000:0000:8a2e:0370:7334 x", "", true);
        assert_eq!(r.counts.ipv6, 1);
        let tok = r
            .tokenized
            .strip_prefix("addr ")
            .and_then(|s| s.strip_suffix(" x"))
            .unwrap();
        assert!(tok.chars().all(|c| c.is_ascii_hexdigit() || c == ':'));
        // eight groups, each 1..=4 hex digits
        assert_eq!(tok.split(':').count(), 8);
    }

    #[test]
    fn ssn_and_phone_shape_preserved() {
        let r = tokenize("ssn 123-45-6789 call (415) 555-0132", "", true);
        assert_eq!(r.counts.ssn, 1);
        assert_eq!(r.counts.phone, 1);
        assert!(!r.tokenized.contains("123-45-6789"));
        assert!(!r.tokenized.contains("(415) 555-0132"));
        // dash + paren structure kept
        assert!(Regex::new(r"\d{3}-\d{2}-\d{4}")
            .unwrap()
            .is_match(&r.tokenized));
        assert!(r.tokenized.contains('(') && r.tokenized.contains(')'));
    }

    #[test]
    fn clean_text_unchanged() {
        let r = tokenize("nothing to see here", "", true);
        assert_eq!(r.tokenized, "nothing to see here");
        assert_eq!(r.total, 0);
    }

    #[test]
    fn parse_bool_happy_and_error() {
        assert_eq!(parse_bool("true"), Ok(true));
        assert_eq!(parse_bool("FALSE"), Ok(false));
        assert!(parse_bool("maybe").is_err());
    }
}
