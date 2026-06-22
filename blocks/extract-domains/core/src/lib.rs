//! gizza-ai/extract-domains core — extract hostnames and registrable domains
//! (eTLD+1) from arbitrary text. Handles bare hostnames, http(s)/ftp URLs, and
//! email addresses. Deduplicates in first-seen order. Pure-Rust (`regex` + the
//! built-in Public Suffix List in `psl`), so it runs on every backend.

use regex::Regex;
use serde::Serialize;

/// Which domains to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The full hostname as it appears (e.g. `www.blog.example.co.uk`).
    Hostname,
    /// The registrable domain / eTLD+1 (e.g. `example.co.uk`).
    Registrable,
    /// Both lists.
    Both,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "hostname" | "hostnames" | "host" => Ok(Mode::Hostname),
            "registrable" | "root" | "etld1" | "etld+1" | "apex" => Ok(Mode::Registrable),
            "both" | "" => Ok(Mode::Both),
            other => Err(format!(
                "unknown mode '{other}' (expected hostname, registrable, or both)"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    /// Number of unique hostnames found.
    pub hostname_count: usize,
    /// Unique hostnames in first-seen order (present unless mode = registrable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostnames: Option<Vec<String>>,
    /// Number of unique registrable domains found.
    pub registrable_count: usize,
    /// Unique registrable domains (eTLD+1) in first-seen order (present unless
    /// mode = hostname).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registrable_domains: Option<Vec<String>>,
}

thread_local! {
    // Hostname-ish candidates: a label sequence with at least one dot, optionally
    // preceded by a scheme (`https://`, `ftp://`) or an email local-part (`@`).
    // We capture broadly then validate each candidate via the PSL.
    static HOSTRE: Regex = Regex::new(
        r#"(?ix)
        (?: [a-z][a-z0-9+.\-]* :// )?     # optional scheme
        (?: [^\s/@:]+ @ )?                # optional user@ / email local-part
        (
          (?: [a-z0-9] (?: [a-z0-9\-]{0,61} [a-z0-9] )? \. )+   # one or more labels + dot
          [a-z] (?: [a-z0-9\-]{0,61} [a-z0-9] )?                # TLD label (must start alpha)
        )
        "#,
    )
    .unwrap();
}

fn push_unique(seen: &mut std::collections::HashSet<String>, list: &mut Vec<String>, v: String) {
    if seen.insert(v.clone()) {
        list.push(v);
    }
}

/// Extract hostnames + registrable domains from `text`, in first-seen order.
pub fn extract(text: &str, mode: Mode) -> Found {
    extract_opts(text, mode, false)
}

/// Extract with options. When `sort` is true, both lists are returned in
/// case-insensitive alphabetical order instead of first-seen order.
pub fn extract_opts(text: &str, mode: Mode, sort: bool) -> Found {
    let mut host_seen = std::collections::HashSet::new();
    let mut hostnames: Vec<String> = Vec::new();
    let mut reg_seen = std::collections::HashSet::new();
    let mut registrable: Vec<String> = Vec::new();

    HOSTRE.with(|re| {
        for caps in re.captures_iter(text) {
            let raw = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            // Normalize: lowercase, strip a trailing dot (FQDN root).
            let host = raw.trim_end_matches('.').to_ascii_lowercase();
            if host.is_empty() {
                continue;
            }
            // Validate via the Public Suffix List. `psl` applies an implicit `*`
            // rule, so it returns Some even for bogus single-label TLDs — we only
            // accept domains whose suffix is a KNOWN (ICANN or private) entry. This
            // rejects IP-address fragments, version numbers like `3.14`, and made-up
            // TLDs while still accepting multi-level suffixes like `co.uk`.
            let suffix = psl::suffix(host.as_bytes());
            if suffix.map(|s| s.typ().is_none()).unwrap_or(true) {
                continue;
            }
            let dom = match psl::domain_str(&host) {
                Some(d) => d.to_string(),
                None => continue,
            };
            push_unique(&mut host_seen, &mut hostnames, host);
            push_unique(&mut reg_seen, &mut registrable, dom);
        }
    });

    if sort {
        hostnames.sort();
        registrable.sort();
    }

    let want_host = matches!(mode, Mode::Hostname | Mode::Both);
    let want_reg = matches!(mode, Mode::Registrable | Mode::Both);

    Found {
        hostname_count: hostnames.len(),
        hostnames: if want_host { Some(hostnames) } else { None },
        registrable_count: registrable.len(),
        registrable_domains: if want_reg { Some(registrable) } else { None },
    }
}

/// Human-readable rendering (used by the page).
pub fn render(text: &str, mode: &str, sort: bool) -> Result<String, String> {
    let m = Mode::parse(mode)?;
    let r = extract_opts(text, m, sort);
    let mut out = String::new();

    if let Some(hosts) = &r.hostnames {
        if hosts.is_empty() {
            out.push_str("No hostnames found.\n");
        } else {
            out.push_str(&format!("{} unique hostname(s):\n", r.hostname_count));
            for h in hosts {
                out.push_str(&format!("  {h}\n"));
            }
        }
    }
    if let Some(reg) = &r.registrable_domains {
        if r.hostnames.is_some() {
            out.push('\n');
        }
        if reg.is_empty() {
            out.push_str("No registrable domains found.\n");
        } else {
            out.push_str(&format!("{} unique registrable domain(s):\n", r.registrable_count));
            for d in reg {
                out.push_str(&format!("  {d}\n"));
            }
        }
    }

    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_from_urls_emails_and_bare() {
        let t = "visit https://www.example.com/path, mail jo@mail.example.co.uk, see sub.test.org.";
        let r = extract(t, Mode::Both);
        assert_eq!(
            r.hostnames.as_ref().unwrap(),
            &vec![
                "www.example.com".to_string(),
                "mail.example.co.uk".to_string(),
                "sub.test.org".to_string()
            ]
        );
        assert_eq!(
            r.registrable_domains.as_ref().unwrap(),
            &vec![
                "example.com".to_string(),
                "example.co.uk".to_string(),
                "test.org".to_string()
            ]
        );
    }

    #[test]
    fn dedupes_first_seen() {
        let t = "a.example.com and b.example.com and a.example.com again";
        let r = extract(t, Mode::Both);
        assert_eq!(
            r.hostnames.as_ref().unwrap(),
            &vec!["a.example.com".to_string(), "b.example.com".to_string()]
        );
        // Both hosts share one registrable domain.
        assert_eq!(r.registrable_domains.as_ref().unwrap(), &vec!["example.com".to_string()]);
        assert_eq!(r.registrable_count, 1);
    }

    #[test]
    fn handles_multi_level_suffix() {
        let r = extract("shop.foo.bar.gov.uk", Mode::Registrable);
        assert_eq!(r.registrable_domains.as_ref().unwrap(), &vec!["bar.gov.uk".to_string()]);
        assert!(r.hostnames.is_none());
    }

    #[test]
    fn rejects_invalid_tld_and_numbers() {
        // 1.2.3.4 is an IP, foo.invalidtld is not a real suffix.
        let r = extract("1.2.3.4 and foo.invalidtldxyz and version 3.14 here", Mode::Both);
        assert!(r.registrable_domains.as_ref().unwrap().is_empty());
        assert_eq!(r.hostname_count, 0);
    }

    #[test]
    fn case_insensitive_and_trailing_dot() {
        let r = extract("HTTPS://WWW.Example.COM. done", Mode::Both);
        assert_eq!(r.hostnames.as_ref().unwrap(), &vec!["www.example.com".to_string()]);
        assert_eq!(r.registrable_domains.as_ref().unwrap(), &vec!["example.com".to_string()]);
    }

    #[test]
    fn mode_filters_output() {
        let hr = extract("a.example.com", Mode::Hostname);
        assert!(hr.hostnames.is_some() && hr.registrable_domains.is_none());
        let rr = extract("a.example.com", Mode::Registrable);
        assert!(rr.hostnames.is_none() && rr.registrable_domains.is_some());
    }

    #[test]
    fn sorts_alphabetically_when_requested() {
        let t = "zebra.com apple.com mango.com";
        let unsorted = extract_opts(t, Mode::Registrable, false);
        assert_eq!(
            unsorted.registrable_domains.as_ref().unwrap(),
            &vec!["zebra.com".to_string(), "apple.com".to_string(), "mango.com".to_string()]
        );
        let sorted = extract_opts(t, Mode::Registrable, true);
        assert_eq!(
            sorted.registrable_domains.as_ref().unwrap(),
            &vec!["apple.com".to_string(), "mango.com".to_string(), "zebra.com".to_string()]
        );
    }

    #[test]
    fn render_and_parse_errors() {
        let out = render("https://example.com mail@test.org", "both", false).unwrap();
        assert!(out.contains("example.com"));
        assert!(out.contains("test.org"));
        assert!(render("x", "nonsense", false).is_err());
        assert!(render("nothing here", "hostname", false).unwrap().contains("No hostnames"));
    }
}
