//! utm-link-builder core — pure compute, shared by the chat skill block and the
//! web page. Builds a campaign-tagged URL by appending UTM query parameters
//! (utm_source, utm_medium, utm_campaign, utm_term, utm_content) to a base URL.
//! No external deps; percent-encoding is hand-rolled (application/x-www-form
//! "+ for space" style, the convention Google Analytics/most link builders use).

use serde::Serialize;

/// The UTM parameters this tool manages, in canonical order. Covers the five
/// classic Urchin params plus the four GA4 additions (utm_id and the *_platform /
/// creative_format / marketing_tactic fields) so re-tagging strips every one.
pub const UTM_KEYS: [&str; 9] = [
    "utm_id",
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_source_platform",
    "utm_creative_format",
    "utm_marketing_tactic",
];

#[derive(Debug, Serialize, PartialEq)]
pub struct Built {
    /// The full tagged URL.
    pub url: String,
    /// Just the query string that was applied (without a leading '?').
    pub query: String,
    /// The UTM parameters that were added/updated, in canonical order.
    pub params: Vec<Param>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Param {
    pub key: String,
    pub value: String,
}

/// Inputs for the builder. Empty/whitespace-only optional fields are skipped.
pub struct Utm<'a> {
    pub url: &'a str,
    pub source: &'a str,
    pub medium: &'a str,
    pub campaign: &'a str,
    pub term: &'a str,
    pub content: &'a str,
    /// GA4 Campaign ID (utm_id) — optional.
    pub id: &'a str,
    /// GA4 utm_source_platform — optional.
    pub source_platform: &'a str,
    /// GA4 utm_creative_format — optional.
    pub creative_format: &'a str,
    /// GA4 utm_marketing_tactic — optional.
    pub marketing_tactic: &'a str,
}

/// Build the tagged URL.
///
/// - `url` is required and must be non-empty. If it has no scheme, `https://` is
///   prepended (so `example.com/page` becomes `https://example.com/page`).
/// - `source`, `medium` and `campaign` are the three required UTM params per the
///   GA convention; an empty one is an error.
/// - `term` and `content` are optional and skipped when blank.
/// - `lowercase` lowercases the parameter VALUES (a common normalization to avoid
///   "Email"/"email" splitting a campaign in analytics).
/// - Any existing `utm_*` params already on the URL are removed first, then the new
///   ones appended, so re-tagging a URL is idempotent. Non-UTM query params and the
///   fragment are preserved.
/// - Spaces in values become `+`; other reserved chars are percent-encoded.
pub fn build(input: &Utm, lowercase: bool) -> Result<Built, String> {
    let raw = input.url.trim();
    if raw.is_empty() {
        return Err("url is required".into());
    }

    // Prepend a scheme if none is present (treat `//` or a `scheme:` prefix as present).
    let normalized = if has_scheme(raw) {
        raw.to_string()
    } else if let Some(rest) = raw.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        format!("https://{raw}")
    };

    // Split off the fragment (kept verbatim, must stay after the query).
    let (before_frag, fragment) = match normalized.split_once('#') {
        Some((b, f)) => (b.to_string(), Some(f.to_string())),
        None => (normalized, None),
    };

    // Split base (scheme://host/path) from any existing query string.
    let (base, existing_query) = match before_frag.split_once('?') {
        Some((b, q)) => (b.to_string(), q.to_string()),
        None => (before_frag, String::new()),
    };

    // Keep existing query pairs that are NOT utm_* (preserve order + raw encoding).
    let mut kept_pairs: Vec<String> = Vec::new();
    if !existing_query.is_empty() {
        for pair in existing_query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let key = pair.split('=').next().unwrap_or("");
            let key_norm = key.to_ascii_lowercase();
            if UTM_KEYS.contains(&key_norm.as_str()) {
                continue; // drop existing utm_* — they'll be replaced
            }
            kept_pairs.push(pair.to_string());
        }
    }

    // Collect the requested UTM params in canonical order. utm_id leads (GA4
    // Campaign ID), then the three required classics, then the optional fields.
    let requested = [
        ("utm_id", input.id, false),
        ("utm_source", input.source, true),
        ("utm_medium", input.medium, true),
        ("utm_campaign", input.campaign, true),
        ("utm_term", input.term, false),
        ("utm_content", input.content, false),
        ("utm_source_platform", input.source_platform, false),
        ("utm_creative_format", input.creative_format, false),
        ("utm_marketing_tactic", input.marketing_tactic, false),
    ];

    let mut params: Vec<Param> = Vec::new();
    for (key, val, required) in requested {
        let v = val.trim();
        if v.is_empty() {
            if required {
                return Err(format!("{} is required", &key["utm_".len()..]));
            }
            continue;
        }
        let v = if lowercase { v.to_lowercase() } else { v.to_string() };
        params.push(Param {
            key: key.to_string(),
            value: v,
        });
    }

    // Assemble the new query string: kept non-utm pairs first, then the UTM pairs.
    let mut new_pairs: Vec<String> = kept_pairs;
    for p in &params {
        new_pairs.push(format!("{}={}", p.key, encode_form(&p.value)));
    }
    let query = new_pairs.join("&");

    let mut url = base;
    if !query.is_empty() {
        url.push('?');
        url.push_str(&query);
    }
    if let Some(f) = fragment {
        url.push('#');
        url.push_str(&f);
    }

    Ok(Built { url, query, params })
}

/// True if `s` starts with a `scheme:` (RFC-3986 scheme = ALPHA *( ALPHA / DIGIT / + / - / . )).
fn has_scheme(s: &str) -> bool {
    let mut chars = s.char_indices();
    match chars.next() {
        Some((_, c)) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    for (i, c) in chars {
        if c == ':' {
            return i > 0;
        }
        if !(c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
            return false;
        }
    }
    false
}

/// application/x-www-form-urlencoded value encoding: space → `+`, unreserved kept,
/// everything else percent-encoded (uppercase hex).
fn encode_form(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(hex(b >> 4));
                out.push(hex(b & 0xf));
            }
        }
    }
    out
}

fn hex(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'A' + (n - 10)) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utm<'a>(
        url: &'a str,
        source: &'a str,
        medium: &'a str,
        campaign: &'a str,
        term: &'a str,
        content: &'a str,
    ) -> Utm<'a> {
        Utm {
            url,
            source,
            medium,
            campaign,
            term,
            content,
            id: "",
            source_platform: "",
            creative_format: "",
            marketing_tactic: "",
        }
    }

    #[test]
    fn basic_three_params() {
        let b = build(
            &utm("https://example.com/page", "google", "cpc", "spring sale", "", ""),
            false,
        )
        .unwrap();
        assert_eq!(
            b.url,
            "https://example.com/page?utm_source=google&utm_medium=cpc&utm_campaign=spring+sale"
        );
        assert_eq!(b.params.len(), 3);
        assert_eq!(b.params[2].value, "spring sale");
    }

    #[test]
    fn adds_scheme_when_missing() {
        let b = build(&utm("example.com", "fb", "social", "launch", "", ""), false).unwrap();
        assert!(b.url.starts_with("https://example.com?"));
    }

    #[test]
    fn preserves_existing_query_and_fragment() {
        let b = build(
            &utm("https://x.io/p?id=7#section", "nl", "email", "june", "", ""),
            false,
        )
        .unwrap();
        assert_eq!(
            b.url,
            "https://x.io/p?id=7&utm_source=nl&utm_medium=email&utm_campaign=june#section"
        );
    }

    #[test]
    fn replaces_existing_utm_params_idempotent() {
        let once = build(
            &utm("https://x.io/p?utm_source=old&utm_medium=banner", "new", "cpc", "c", "", ""),
            false,
        )
        .unwrap();
        assert_eq!(
            once.url,
            "https://x.io/p?utm_source=new&utm_medium=cpc&utm_campaign=c"
        );
        // re-tagging the already-tagged url yields the same result
        let twice = build(&utm(&once.url, "new", "cpc", "c", "", ""), false).unwrap();
        assert_eq!(twice.url, once.url);
    }

    #[test]
    fn optional_term_and_content_and_encoding() {
        let b = build(
            &utm(
                "https://x.io",
                "google",
                "cpc",
                "promo",
                "shoes & socks",
                "ad/1",
            ),
            false,
        )
        .unwrap();
        assert_eq!(
            b.url,
            "https://x.io?utm_source=google&utm_medium=cpc&utm_campaign=promo&utm_term=shoes+%26+socks&utm_content=ad%2F1"
        );
        assert_eq!(b.params.len(), 5);
    }

    #[test]
    fn lowercase_normalizes_values() {
        let b = build(&utm("https://x.io", "Google", "CPC", "Spring", "", ""), true).unwrap();
        assert_eq!(
            b.url,
            "https://x.io?utm_source=google&utm_medium=cpc&utm_campaign=spring"
        );
    }

    #[test]
    fn ga4_params_appear_with_utm_id_first() {
        let b = build(
            &Utm {
                url: "https://x.io",
                source: "google",
                medium: "cpc",
                campaign: "promo",
                term: "",
                content: "",
                id: "abc.123",
                source_platform: "google ads",
                creative_format: "display",
                marketing_tactic: "remarketing",
            },
            false,
        )
        .unwrap();
        assert_eq!(
            b.url,
            "https://x.io?utm_id=abc.123&utm_source=google&utm_medium=cpc&utm_campaign=promo&utm_source_platform=google+ads&utm_creative_format=display&utm_marketing_tactic=remarketing"
        );
        // utm_id is first in the param list.
        assert_eq!(b.params[0].key, "utm_id");
    }

    #[test]
    fn empty_url_errors() {
        let e = build(&utm("  ", "a", "b", "c", "", ""), false).unwrap_err();
        assert!(e.contains("url is required"));
    }

    #[test]
    fn missing_required_source_errors() {
        let e = build(&utm("https://x.io", "", "b", "c", "", ""), false).unwrap_err();
        assert!(e.contains("source is required"));
    }

    #[test]
    fn missing_required_campaign_errors() {
        let e = build(&utm("https://x.io", "a", "b", "", "", ""), false).unwrap_err();
        assert!(e.contains("campaign is required"));
    }
}
