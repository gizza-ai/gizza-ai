//! gizza-ai/extract-ip-addresses core — find, validate, and deduplicate IPv4 and
//! IPv6 addresses in text. Candidates are matched with `regex`, then validated by
//! the standard library's `IpAddr` parser (so only real addresses survive) and
//! canonicalized for de-duplication. Pure-Rust.

use std::net::{Ipv4Addr, Ipv6Addr};

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    pub count: usize,
    /// Unique IPv4 addresses, in first-seen order.
    pub ipv4: Vec<String>,
    /// Unique IPv6 addresses (canonical/compressed form), in first-seen order.
    pub ipv6: Vec<String>,
}

thread_local! {
    // IPv4 dotted-quad (validity checked afterwards by Ipv4Addr).
    static V4: Regex = Regex::new(r"\b\d{1,3}(?:\.\d{1,3}){3}\b").unwrap();
    // IPv6 candidate: a run of hex/colon/dot containing at least one ':'.
    // Ipv6Addr::from_str rejects anything that isn't a real address.
    static V6: Regex = Regex::new(r"[0-9A-Fa-f]*:[0-9A-Fa-f:.]*[0-9A-Fa-f]|::").unwrap();
}

/// Find, validate and dedupe all IPv4/IPv6 addresses in `text`.
pub fn extract(text: &str) -> Found {
    let mut ipv4: Vec<String> = Vec::new();
    let mut ipv6: Vec<String> = Vec::new();
    let mut seen4 = std::collections::HashSet::new();
    let mut seen6 = std::collections::HashSet::new();

    V4.with(|re| {
        for m in re.find_iter(text) {
            if let Ok(ip) = m.as_str().parse::<Ipv4Addr>() {
                let s = ip.to_string();
                if seen4.insert(s.clone()) {
                    ipv4.push(s);
                }
            }
        }
    });

    V6.with(|re| {
        for m in re.find_iter(text) {
            let tok = m.as_str();
            // Require at least two colons (a lone "a:b" is not a v6 address and
            // is usually a time / ratio).
            if tok.matches(':').count() < 2 && !tok.contains("::") {
                continue;
            }
            if let Ok(ip) = tok.parse::<Ipv6Addr>() {
                let s = ip.to_string(); // canonical compressed form
                if seen6.insert(s.clone()) {
                    ipv6.push(s);
                }
            }
        }
    });

    Found { count: ipv4.len() + ipv6.len(), ipv4, ipv6 }
}

/// Human-readable rendering (used by the page).
pub fn render(text: &str) -> Result<String, String> {
    let r = extract(text);
    if r.count == 0 {
        return Ok("No IP addresses found.".to_string());
    }
    let mut out = format!("{} unique IP address(es):\n", r.count);
    if !r.ipv4.is_empty() {
        out.push_str(&format!("\nIPv4 ({}):\n", r.ipv4.len()));
        for ip in &r.ipv4 {
            out.push_str(&format!("  {ip}\n"));
        }
    }
    if !r.ipv6.is_empty() {
        out.push_str(&format!("\nIPv6 ({}):\n", r.ipv6.len()));
        for ip in &r.ipv6 {
            out.push_str(&format!("  {ip}\n"));
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_ipv4_and_dedups() {
        let t = "from 192.168.0.1 to 10.0.0.5, again 192.168.0.1 and bad 999.1.1.1";
        let r = extract(t);
        assert_eq!(r.ipv4, vec!["192.168.0.1", "10.0.0.5"]);
        assert!(r.ipv6.is_empty());
    }

    #[test]
    fn ipv4_with_port() {
        let r = extract("server at 203.0.113.7:8080 listening");
        assert_eq!(r.ipv4, vec!["203.0.113.7"]);
    }

    #[test]
    fn finds_ipv6_canonicalized() {
        let r = extract("addr 2001:0db8:0000:0000:0000:0000:0000:0001 and ::1 and fe80::1");
        // 2001:db8::1 (compressed), ::1, fe80::1
        assert!(r.ipv6.contains(&"2001:db8::1".to_string()));
        assert!(r.ipv6.contains(&"::1".to_string()));
        assert!(r.ipv6.contains(&"fe80::1".to_string()));
    }

    #[test]
    fn ignores_times_and_junk() {
        let r = extract("meeting at 12:34:56 today, ratio 3:4, not an ip");
        assert!(r.ipv6.is_empty(), "12:34:56 is not a valid IPv6");
        assert!(r.ipv4.is_empty());
    }

    #[test]
    fn bracketed_ipv6_in_url() {
        let r = extract("http://[2001:db8::a]:443/path");
        assert_eq!(r.ipv6, vec!["2001:db8::a"]);
    }

    #[test]
    fn render_modes() {
        assert!(render("10.0.0.1").unwrap().contains("IPv4 (1)"));
        assert!(render("::1").unwrap().contains("IPv6 (1)"));
        assert!(render("nope").unwrap().contains("No IP"));
    }
}
