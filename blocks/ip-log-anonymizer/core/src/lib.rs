//! gizza-ai/ip-log-anonymizer core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Finds every IPv4 and IPv6 address in a log/text and anonymizes it IN PLACE,
//! leaving all surrounding characters untouched. Three methods:
//!   - `mask`   — zero the trailing octets (IPv4) / hextets (IPv6), like the
//!                classic Google-Analytics / Matomo IP truncation.
//!   - `hash`   — replace each address with a short salted SHA-256 token, so the
//!                same address maps to the same pseudonym without being reversible
//!                (a secret salt is strongly recommended).
//!   - `redact` — replace each address with a fixed placeholder token.
//!
//! Addresses are matched with `regex`, then validated by the standard library's
//! `IpAddr` parser so only real addresses are touched. IPv6 matches are resolved
//! first, so an IPv4-mapped address (`::ffff:192.0.2.1`) is anonymized once as a
//! whole, not double-replaced.

use std::net::{Ipv4Addr, Ipv6Addr};

use regex::Regex;
use sha2::{Digest, Sha256};

thread_local! {
    // IPv4 dotted-quad; validity (each octet 0-255) is checked afterwards by Ipv4Addr.
    static V4: Regex = Regex::new(r"\b\d{1,3}(?:\.\d{1,3}){3}\b").unwrap();
    // IPv6 candidate: a run of hex/colon/dot containing at least one ':'.
    // Ipv6Addr::from_str rejects anything that isn't a real address.
    static V6: Regex = Regex::new(r"[0-9A-Fa-f]*:[0-9A-Fa-f:.]*[0-9A-Fa-f]|::").unwrap();
}

/// How to anonymize each matched address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Zero the trailing octets / hextets (GA / Matomo style truncation).
    Mask,
    /// Replace with a short salted SHA-256 token (stable pseudonym).
    Hash,
    /// Replace with a fixed placeholder token.
    Redact,
}

impl Mode {
    fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mask" | "" => Ok(Mode::Mask),
            "hash" => Ok(Mode::Hash),
            "redact" => Ok(Mode::Redact),
            other => Err(format!(
                "unknown mode '{other}' — expected one of: mask, hash, redact"
            )),
        }
    }
}

/// True for addresses that are not internet-routable / not personal data
/// (private, loopback, link-local, unspecified). Used by `skip_private`.
fn is_private_v4(ip: &Ipv4Addr) -> bool {
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || ip.octets()[0] == 0
}

fn is_private_v6(ip: &Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || (first & 0xfe00) == 0xfc00 // Unique local fc00::/7
        || (first & 0xffc0) == 0xfe80 // Link-local fe80::/10
}

fn mask_v4(ip: Ipv4Addr, octets: u32) -> Ipv4Addr {
    let n = octets.min(4) as usize;
    let mut o = ip.octets();
    for slot in o.iter_mut().skip(4 - n) {
        *slot = 0;
    }
    Ipv4Addr::from(o)
}

fn mask_v6(ip: Ipv6Addr, groups: u32) -> Ipv6Addr {
    let n = groups.min(8) as usize;
    let mut s = ip.segments();
    for slot in s.iter_mut().skip(8 - n) {
        *slot = 0;
    }
    Ipv6Addr::from(s)
}

/// Salted SHA-256 of the canonical address text, truncated to `len` hex chars.
fn hash_ip(canonical: &str, salt: &str, len: usize) -> String {
    let mut h = Sha256::new();
    h.update(salt.as_bytes());
    h.update(b":");
    h.update(canonical.as_bytes());
    let digest = h.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        hex.push_str(&format!("{b:02x}"));
    }
    hex.truncate(len.clamp(4, 64));
    hex
}

struct Repl {
    start: usize,
    end: usize,
    text: String,
}

/// Anonymize every IPv4/IPv6 address in `text`.
///
/// - `mode_s`: "mask" | "hash" | "redact".
/// - `ipv4_octets`: mask mode — trailing IPv4 octets to zero (0-4, clamped).
/// - `ipv6_groups`: mask mode — trailing IPv6 16-bit hextets to zero (0-8, clamped).
/// - `salt`: hash mode — secret mixed in before hashing (recommended, may be empty).
/// - `hash_length`: hash mode — hex characters of the digest to keep (4-64, clamped).
/// - `replacement`: redact mode — the placeholder token to substitute.
/// - `skip_private`: when true, private/loopback/link-local addresses are left as-is.
#[allow(clippy::too_many_arguments)]
pub fn anonymize(
    text: &str,
    mode_s: &str,
    ipv4_octets: u32,
    ipv6_groups: u32,
    salt: &str,
    hash_length: u32,
    replacement: &str,
    skip_private: bool,
) -> Result<String, String> {
    if text.is_empty() {
        return Err("input text is empty".into());
    }
    let mode = Mode::parse(mode_s)?;
    let ipv4_octets = ipv4_octets.min(4);
    let ipv6_groups = ipv6_groups.min(8);
    let hash_length = hash_length.clamp(4, 64) as usize;

    let mut repls: Vec<Repl> = Vec::new();

    // IPv6 first — a v6 match encompasses any embedded IPv4-mapped address, so
    // resolving it first stops the v4 pass from double-replacing the inner quad.
    V6.with(|re| {
        for m in re.find_iter(text) {
            let tok = m.as_str();
            if tok.matches(':').count() < 2 && !tok.contains("::") {
                continue; // a lone "a:b" is a time/ratio, not a v6 address
            }
            if let Ok(ip) = tok.parse::<Ipv6Addr>() {
                if skip_private && is_private_v6(&ip) {
                    continue;
                }
                let out = match mode {
                    Mode::Mask => mask_v6(ip, ipv6_groups).to_string(),
                    Mode::Hash => hash_ip(&ip.to_string(), salt, hash_length),
                    Mode::Redact => replacement.to_string(),
                };
                repls.push(Repl { start: m.start(), end: m.end(), text: out });
            }
        }
    });

    V4.with(|re| {
        for m in re.find_iter(text) {
            if let Ok(ip) = m.as_str().parse::<Ipv4Addr>() {
                // skip an IPv4 that sits inside an already-accepted v6 match
                if repls.iter().any(|r| m.start() < r.end && r.start < m.end()) {
                    continue;
                }
                if skip_private && is_private_v4(&ip) {
                    continue;
                }
                let out = match mode {
                    Mode::Mask => mask_v4(ip, ipv4_octets).to_string(),
                    Mode::Hash => hash_ip(&ip.to_string(), salt, hash_length),
                    Mode::Redact => replacement.to_string(),
                };
                repls.push(Repl { start: m.start(), end: m.end(), text: out });
            }
        }
    });

    repls.sort_by_key(|r| r.start);

    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for r in &repls {
        if r.start < last {
            continue; // paranoia: never emit overlapping replacements
        }
        out.push_str(&text[last..r.start]);
        out.push_str(&r.text);
        last = r.end;
    }
    out.push_str(&text[last..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_zeros_last_octet_by_default() {
        assert_eq!(
            anonymize("hit from 203.0.113.45 ok", "mask", 1, 5, "", 12, "[IP]", false).unwrap(),
            "hit from 203.0.113.0 ok"
        );
    }

    #[test]
    fn mask_two_octets() {
        assert_eq!(
            anonymize("203.0.113.45", "mask", 2, 5, "", 12, "[IP]", false).unwrap(),
            "203.0.0.0"
        );
    }

    #[test]
    fn mask_keeps_port_and_surrounding_text() {
        assert_eq!(
            anonymize("client=203.0.113.45:8080 GET", "mask", 1, 5, "", 12, "[IP]", false).unwrap(),
            "client=203.0.113.0:8080 GET"
        );
    }

    #[test]
    fn mask_ipv6_keeps_leading_groups() {
        // keep the first 3 hextets, zero the last 5 → canonical compressed form.
        assert_eq!(
            anonymize(
                "src 2001:db8:85a3::8a2e:370:7334 end",
                "mask",
                1,
                5,
                "",
                12,
                "[IP]",
                false
            )
            .unwrap(),
            "src 2001:db8:85a3:: end"
        );
    }

    #[test]
    fn redact_replaces_with_token() {
        assert_eq!(
            anonymize("a 192.0.2.1 b 198.51.100.7 c", "redact", 1, 5, "", 12, "[IP]", false)
                .unwrap(),
            "a [IP] b [IP] c"
        );
    }

    #[test]
    fn hash_is_stable_salted_and_truncated() {
        let a = anonymize("x 203.0.113.45 y", "hash", 1, 5, "s3cret", 12, "[IP]", false).unwrap();
        let b = anonymize("x 203.0.113.45 y", "hash", 1, 5, "s3cret", 12, "[IP]", false).unwrap();
        assert_eq!(a, b, "same address + salt → same token");
        // "x <12 hex> y"
        let token = &a[2..a.len() - 2];
        assert_eq!(token.len(), 12);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        // a different salt yields a different token
        let c = anonymize("x 203.0.113.45 y", "hash", 1, 5, "other", 12, "[IP]", false).unwrap();
        assert_ne!(a, c, "salt changes the token");
    }

    #[test]
    fn skip_private_leaves_internal_addresses() {
        assert_eq!(
            anonymize("10.0.0.5 -> 203.0.113.9", "mask", 1, 5, "", 12, "[IP]", true).unwrap(),
            "10.0.0.5 -> 203.0.113.0"
        );
        // without skip_private the private one is masked too
        assert_eq!(
            anonymize("10.0.0.5 -> 203.0.113.9", "mask", 1, 5, "", 12, "[IP]", false).unwrap(),
            "10.0.0.0 -> 203.0.113.0"
        );
    }

    #[test]
    fn ipv4_mapped_v6_is_replaced_once() {
        assert_eq!(
            anonymize("x ::ffff:192.0.2.1 y", "redact", 1, 5, "", 12, "[IP]", false).unwrap(),
            "x [IP] y"
        );
    }

    #[test]
    fn empty_text_errors() {
        assert!(anonymize("", "mask", 1, 5, "", 12, "[IP]", false).is_err());
    }

    #[test]
    fn unknown_mode_errors() {
        let e = anonymize("1.2.3.4", "scramble", 1, 5, "", 12, "[IP]", false).unwrap_err();
        assert!(e.contains("scramble"), "error names the bad mode: {e}");
    }

    #[test]
    fn full_mask_zeros_whole_address() {
        assert_eq!(
            anonymize("1.2.3.4", "mask", 4, 8, "", 12, "[IP]", false).unwrap(),
            "0.0.0.0"
        );
    }
}
