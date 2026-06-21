//! gizza-ai/extract-email-addresses core — pull every email address out of text,
//! deduplicate (case-insensitively), and optionally group by domain. Pure-Rust
//! (`regex`).

use std::collections::BTreeMap;

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Extracted {
    pub count: usize,
    /// Unique addresses (lowercased), in first-seen order.
    pub emails: Vec<String>,
    /// domain → addresses, present only when grouping was requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_domain: Option<BTreeMap<String, Vec<String>>>,
}

thread_local! {
    // Pragmatic email matcher: local-part, '@', domain with a real TLD.
    static EMAIL: Regex =
        Regex::new(r"(?i)\b[a-z0-9._%+\-]+@[a-z0-9](?:[a-z0-9\-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9\-]*[a-z0-9])?)*\.[a-z]{2,}\b").unwrap();
}

/// Extract unique email addresses from `text`.
pub fn extract(text: &str, group_by_domain: bool) -> Extracted {
    let mut seen = std::collections::HashSet::new();
    let mut emails: Vec<String> = Vec::new();
    EMAIL.with(|re| {
        for m in re.find_iter(text) {
            let e = m.as_str().to_ascii_lowercase();
            if seen.insert(e.clone()) {
                emails.push(e);
            }
        }
    });

    let by_domain = if group_by_domain {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for e in &emails {
            if let Some((_, domain)) = e.rsplit_once('@') {
                map.entry(domain.to_string()).or_default().push(e.clone());
            }
        }
        Some(map)
    } else {
        None
    };

    Extracted { count: emails.len(), emails, by_domain }
}

/// Human-readable rendering (used by the page).
pub fn render(text: &str, group_by_domain: bool) -> Result<String, String> {
    let r = extract(text, group_by_domain);
    if r.count == 0 {
        return Ok("No email addresses found.".to_string());
    }
    let mut out = format!("{} unique address(es):\n", r.count);
    if let Some(by) = &r.by_domain {
        for (domain, addrs) in by {
            out.push_str(&format!("\n@{domain} ({}):\n", addrs.len()));
            for a in addrs {
                out.push_str(&format!("  {a}\n"));
            }
        }
    } else {
        for e in &r.emails {
            out.push_str(&format!("{e}\n"));
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_dedups() {
        let t = "Contact a@x.com or A@X.com, also b@y.org. Reach a@x.com again.";
        let r = extract(t, false);
        assert_eq!(r.count, 2);
        assert_eq!(r.emails, vec!["a@x.com", "b@y.org"]);
        assert!(r.by_domain.is_none());
    }

    #[test]
    fn group_by_domain() {
        let t = "alice@corp.com bob@corp.com carol@other.io";
        let r = extract(t, true);
        let by = r.by_domain.unwrap();
        assert_eq!(by["corp.com"], vec!["alice@corp.com", "bob@corp.com"]);
        assert_eq!(by["other.io"], vec!["carol@other.io"]);
    }

    #[test]
    fn handles_plus_and_subdomains() {
        let r = extract("user+tag@mail.example.co.uk", false);
        assert_eq!(r.emails, vec!["user+tag@mail.example.co.uk"]);
    }

    #[test]
    fn ignores_non_emails() {
        let r = extract("not an email: @nope, foo@, @bar.com, plain text", false);
        assert_eq!(r.count, 0);
    }

    #[test]
    fn render_modes() {
        assert!(render("a@x.com", false).unwrap().contains("a@x.com"));
        assert!(render("a@x.com b@x.com", true).unwrap().contains("@x.com (2)"));
        assert!(render("nothing", false).unwrap().contains("No email"));
    }
}
