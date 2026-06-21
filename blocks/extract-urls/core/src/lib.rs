//! gizza-ai/extract-urls core — extract all http(s) URLs from text, deduplicate
//! (first-seen order), and optionally split each into scheme/host/port/path/
//! query/fragment components. Pure-Rust (`regex` + `url`).

use regex::Regex;
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Components {
    pub url: String,
    pub scheme: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    pub count: usize,
    /// Unique URLs in first-seen order.
    pub urls: Vec<String>,
    /// Per-URL component split, present only when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<Components>>,
}

thread_local! {
    static URLRE: Regex = Regex::new(r#"(?i)\bhttps?://[^\s<>"'`\)\]\}]+"#).unwrap();
}

/// Trim trailing punctuation that commonly abuts a URL in prose.
fn trim_trailing(u: &str) -> &str {
    u.trim_end_matches(|c| matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '\u{2019}'))
}

fn split(u: &str) -> Option<Components> {
    let parsed = Url::parse(u).ok()?;
    Some(Components {
        url: u.to_string(),
        scheme: parsed.scheme().to_string(),
        host: parsed.host_str().map(|h| h.to_string()),
        port: parsed.port(),
        path: parsed.path().to_string(),
        query: parsed.query().map(|q| q.to_string()),
        fragment: parsed.fragment().map(|f| f.to_string()),
    })
}

/// Extract, validate, and dedupe URLs from `text`.
pub fn extract(text: &str, split_components: bool) -> Found {
    let mut seen = std::collections::HashSet::new();
    let mut urls: Vec<String> = Vec::new();
    URLRE.with(|re| {
        for m in re.find_iter(text) {
            let candidate = trim_trailing(m.as_str());
            if Url::parse(candidate).is_err() {
                continue;
            }
            if seen.insert(candidate.to_string()) {
                urls.push(candidate.to_string());
            }
        }
    });

    let components = if split_components {
        Some(urls.iter().filter_map(|u| split(u)).collect())
    } else {
        None
    };

    Found { count: urls.len(), urls, components }
}

/// Human-readable rendering (used by the page).
pub fn render(text: &str, split_components: bool) -> Result<String, String> {
    let r = extract(text, split_components);
    if r.count == 0 {
        return Ok("No URLs found.".to_string());
    }
    let mut out = format!("{} unique URL(s):\n", r.count);
    if let Some(comps) = &r.components {
        for c in comps {
            out.push_str(&format!("\n{}\n", c.url));
            out.push_str(&format!("  scheme: {}\n", c.scheme));
            if let Some(h) = &c.host {
                out.push_str(&format!("  host:   {h}\n"));
            }
            if let Some(p) = c.port {
                out.push_str(&format!("  port:   {p}\n"));
            }
            out.push_str(&format!("  path:   {}\n", c.path));
            if let Some(q) = &c.query {
                out.push_str(&format!("  query:  {q}\n"));
            }
            if let Some(f) = &c.fragment {
                out.push_str(&format!("  fragment: {f}\n"));
            }
        }
    } else {
        for u in &r.urls {
            out.push_str(&format!("{u}\n"));
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_dedups() {
        let t = "see https://a.com/x and http://b.org. Also https://a.com/x again.";
        let r = extract(t, false);
        assert_eq!(r.urls, vec!["https://a.com/x", "http://b.org"]);
        assert!(r.components.is_none());
    }

    #[test]
    fn trims_trailing_punctuation() {
        let r = extract("visit https://example.com/page.", false);
        assert_eq!(r.urls, vec!["https://example.com/page"]);
    }

    #[test]
    fn does_not_grab_brackets() {
        let r = extract("(https://example.com/a) [http://b.io/c]", false);
        assert_eq!(r.urls, vec!["https://example.com/a", "http://b.io/c"]);
    }

    #[test]
    fn splits_components() {
        let r = extract("https://user@host.com:8443/p/q?a=1&b=2#frag", true);
        let c = &r.components.unwrap()[0];
        assert_eq!(c.scheme, "https");
        assert_eq!(c.host.as_deref(), Some("host.com"));
        assert_eq!(c.port, Some(8443));
        assert_eq!(c.path, "/p/q");
        assert_eq!(c.query.as_deref(), Some("a=1&b=2"));
        assert_eq!(c.fragment.as_deref(), Some("frag"));
    }

    #[test]
    fn ignores_non_http() {
        // ftp:// and bare words aren't matched (http/https only).
        let r = extract("ftp://x.com and just example.com text", false);
        assert!(r.urls.is_empty());
    }

    #[test]
    fn render_modes() {
        assert!(render("https://a.com", false).unwrap().contains("https://a.com"));
        assert!(render("https://a.com/p?x=1", true).unwrap().contains("query:  x=1"));
        assert!(render("nope", false).unwrap().contains("No URLs"));
    }
}
