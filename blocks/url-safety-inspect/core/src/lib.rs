//! url-safety-inspect core — deterministic, offline phishing-risk heuristics for a
//! single URL. No network, no blocklists, no wafer/wasm-bindgen deps: every signal is
//! derived purely from the URL string's structure, so the same input always yields the
//! same rating. Shared by the chat skill block, the CLI, and the web page.
//!
//! This is a STRUCTURAL heuristic rater, NOT a live threat-intelligence lookup: it can
//! flag a URL that *looks* like phishing and clear one that has no structural red flags,
//! but a clean rating is not proof a URL is safe (see the page FAQ / limits).

/// Severity buckets for a single finding. The numeric weight feeds the composite score
/// and also orders the findings (most severe first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    /// Points this severity contributes to the 0–100 composite risk score.
    fn weight(self) -> u32 {
        match self {
            Severity::High => 30,
            Severity::Medium => 18,
            Severity::Low => 9,
            Severity::Info => 3,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
            Severity::Info => "info",
        }
    }

    /// Sort rank — lower sorts first (High before Low).
    fn rank(self) -> u8 {
        match self {
            Severity::High => 0,
            Severity::Medium => 1,
            Severity::Low => 2,
            Severity::Info => 3,
        }
    }
}

/// One heuristic red flag detected on the URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    /// A short machine-ish check id (e.g. `ip-literal-host`).
    pub check: String,
    /// A human sentence explaining the flag and why it matters.
    pub message: String,
}

/// The overall verdict rendered from the composite score.
fn rating_for(score: u32) -> &'static str {
    match score {
        0 => "MINIMAL",
        1..=19 => "LOW",
        20..=44 => "MEDIUM",
        45..=69 => "HIGH",
        _ => "CRITICAL",
    }
}

// ---- URL structure ---------------------------------------------------------------

/// The few structural pieces the heuristics need. Parsed forgivingly — a malformed URL
/// is exactly what we want to inspect, so we never hard-error on structure.
struct UrlParts {
    scheme: Option<String>,
    /// Everything before the '@' in the authority (raw), if any.
    userinfo: Option<String>,
    /// The host (registered name, IPv4, or IPv6 with brackets stripped), lowercased.
    host: String,
    /// True if the host came from a `[...]` IPv6 literal.
    ipv6_literal: bool,
    /// Explicit port, if a numeric `:port` followed the host.
    port: Option<u32>,
    /// The path + query + fragment tail (everything after the authority).
    tail: String,
}

fn parse_url(raw: &str) -> UrlParts {
    // Scheme: split on the first "://" (the web-URL form with an authority).
    let (scheme, after_scheme) = match raw.find("://") {
        Some(i) => (Some(raw[..i].to_ascii_lowercase()), &raw[i + 3..]),
        None => (None, raw),
    };

    // Authority ends at the first '/', '?' or '#'.
    let auth_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let authority = &after_scheme[..auth_end];
    let tail = after_scheme[auth_end..].to_string();

    // Userinfo: everything before the LAST '@' — structurally the real host follows the
    // final '@'.
    let (userinfo, hostport) = match authority.rfind('@') {
        Some(i) => (Some(authority[..i].to_string()), &authority[i + 1..]),
        None => (None, authority),
    };

    // Host / port, IPv6-aware.
    let (host, ipv6_literal, port) = if let Some(close) = hostport.strip_prefix('[') {
        // [IPv6]:port
        match close.find(']') {
            Some(j) => {
                let h = &close[..j];
                let rest = &close[j + 1..];
                let port = rest.strip_prefix(':').and_then(|p| p.parse::<u32>().ok());
                (h.to_ascii_lowercase(), true, port)
            }
            None => (close.to_ascii_lowercase(), true, None),
        }
    } else {
        match hostport.rfind(':') {
            // Only treat the tail as a port if it is non-empty and all ASCII digits.
            Some(i)
                if !hostport[i + 1..].is_empty()
                    && hostport[i + 1..].bytes().all(|b| b.is_ascii_digit()) =>
            {
                (
                    hostport[..i].to_ascii_lowercase(),
                    false,
                    hostport[i + 1..].parse::<u32>().ok(),
                )
            }
            _ => (hostport.to_ascii_lowercase(), false, None),
        }
    };

    UrlParts {
        scheme,
        userinfo,
        host,
        ipv6_literal,
        port,
        tail,
    }
}

// ---- Heuristic tables ------------------------------------------------------------

/// Free / cheap / heavily-abused TLDs that show up disproportionately in phishing.
const SUSPICIOUS_TLDS: &[&str] = &[
    "tk", "ml", "ga", "cf", "gq", "xyz", "top", "work", "click", "link", "gdn", "loan",
    "download", "review", "country", "kim", "science", "party", "date", "racing", "win",
    "bid", "stream", "cricket", "accountant", "faith", "zip", "mov", "cam", "rest",
    "buzz", "monster", "quest", "sbs", "lol",
];

/// TLDs commonly used to typo-squat a well-known one (`.cm`/`.co`/`.om` ~ `.com`).
const LOOKALIKE_TLDS: &[&str] = &["cm", "co", "om", "ne", "ogr", "orgg", "comm"];

/// Well-known URL-shortener hosts — not malicious per se, but they hide the real
/// destination, which matters for a safety read.
const SHORTENER_HOSTS: &[&str] = &[
    "bit.ly", "tinyurl.com", "t.co", "goo.gl", "ow.ly", "is.gd", "buff.ly", "rebrand.ly",
    "cutt.ly", "t.ly", "rb.gy", "shorturl.at", "tiny.cc", "bit.do", "soo.gd", "x.co",
];

/// Words that phishing URLs overuse to look legitimate/urgent.
const DECEPTIVE_KEYWORDS: &[&str] = &[
    "login", "signin", "secure", "account", "verify", "verification", "update",
    "confirm", "banking", "wallet", "password", "webscr", "appleid", "security",
    "suspended", "recover", "unlock", "authenticate",
];

fn is_ipv4(host: &str) -> bool {
    let mut parts = 0;
    for octet in host.split('.') {
        parts += 1;
        if octet.is_empty() || octet.len() > 3 || !octet.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        if octet.parse::<u32>().map(|n| n > 255).unwrap_or(true) {
            return false;
        }
    }
    parts == 4
}

// ---- Public API ------------------------------------------------------------------

/// Inspect a single URL and return the structured findings + composite score (0–100)
/// + the rating string. Returns `Err` only when the input is empty/blank — any other
/// string is inspectable.
pub fn inspect(raw: &str) -> Result<(Vec<Finding>, u32, &'static str), String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err("no URL provided — pass the URL to inspect".into());
    }

    let p = parse_url(url);
    let mut findings: Vec<Finding> = Vec::new();

    let host_is_ip = is_ipv4(&p.host);

    // 1. IP-literal host — legitimate brands use domain names, not bare IPs.
    if host_is_ip || p.ipv6_literal {
        findings.push(Finding {
            severity: Severity::High,
            check: "ip-literal-host".into(),
            message: format!(
                "Host is an IP literal ({}), not a domain name — legitimate sites almost always use a registered domain.",
                p.host
            ),
        });
    }

    // 2. '@' in the authority — the browser uses the host AFTER the '@'; text before it
    //    (often a familiar brand) is decoy userinfo.
    if let Some(ui) = &p.userinfo {
        findings.push(Finding {
            severity: Severity::High,
            check: "userinfo-at-sign".into(),
            message: format!(
                "Authority contains '@': the browser connects to '{}', while the '{}' shown before the '@' is ignored — a classic disguise.",
                p.host, ui
            ),
        });
    }

    // Domain labels (skip for IP hosts).
    let labels: Vec<&str> = if host_is_ip || p.ipv6_literal || p.host.is_empty() {
        Vec::new()
    } else {
        p.host.split('.').filter(|s| !s.is_empty()).collect()
    };

    // 3. Excessive subdomains — labels beyond the registrable domain (last two).
    if labels.len() >= 5 {
        let sub = labels.len() - 2;
        findings.push(Finding {
            severity: Severity::Medium,
            check: "excessive-subdomains".into(),
            message: format!(
                "Host has {} subdomain levels ({}) — deep subdomain nesting is often used to bury a lookalike brand name.",
                sub, p.host
            ),
        });
    }

    // 4. Punycode (`xn--`) — encodes non-ASCII/homograph characters that can mimic a
    //    real domain (e.g. a Cyrillic 'а').
    if labels.iter().any(|l| l.starts_with("xn--")) {
        findings.push(Finding {
            severity: Severity::High,
            check: "punycode-label".into(),
            message: format!(
                "Host contains a punycode label (xn--…) in '{}' — this can render as a homograph that visually mimics a trusted domain.",
                p.host
            ),
        });
    }

    // 5. TLD reputation — suspicious/abused, or a lookalike of a common TLD.
    if let Some(tld) = labels.last() {
        if SUSPICIOUS_TLDS.contains(tld) {
            findings.push(Finding {
                severity: Severity::Medium,
                check: "suspicious-tld".into(),
                message: format!(
                    "TLD '.{}' is a free/low-cost domain heavily abused for phishing.",
                    tld
                ),
            });
        } else if LOOKALIKE_TLDS.contains(tld) {
            findings.push(Finding {
                severity: Severity::Medium,
                check: "lookalike-tld".into(),
                message: format!(
                    "TLD '.{}' closely resembles a common TLD (e.g. .com/.net/.org) — a frequent typo-squatting trick.",
                    tld
                ),
            });
        }
    }

    // 6. Percent-encoding inside the host — hosts should never be percent-encoded; it is
    //    used to obscure the true destination.
    if p.host.contains('%') {
        findings.push(Finding {
            severity: Severity::High,
            check: "percent-encoded-host".into(),
            message: "Host contains percent-encoding (%XX) — hostnames are never legitimately percent-encoded; this hides the real destination.".into(),
        });
    }

    // 7. No HTTPS.
    if let Some(scheme) = &p.scheme {
        if scheme == "http" {
            findings.push(Finding {
                severity: Severity::Low,
                check: "no-https".into(),
                message: "URL uses plain http:// — traffic is unencrypted and the site presents no TLS identity.".into(),
            });
        }
    }

    // 8. URL-shortener host — hides the real destination.
    let registrable = if labels.len() >= 2 {
        format!("{}.{}", labels[labels.len() - 2], labels[labels.len() - 1])
    } else {
        p.host.clone()
    };
    if SHORTENER_HOSTS.contains(&registrable.as_str()) {
        findings.push(Finding {
            severity: Severity::Info,
            check: "url-shortener".into(),
            message: format!(
                "'{}' is a URL shortener — the real destination is hidden until the link is followed.",
                registrable
            ),
        });
    }

    // 9. Deceptive keywords in the host or path.
    let hay = format!("{} {}", p.host, p.tail).to_ascii_lowercase();
    let hits: Vec<&str> = DECEPTIVE_KEYWORDS
        .iter()
        .copied()
        .filter(|k| hay.contains(k))
        .collect();
    if !hits.is_empty() {
        findings.push(Finding {
            severity: Severity::Low,
            check: "deceptive-keywords".into(),
            message: format!(
                "Contains urgency/credential keywords ({}) — common in pages that fake a login or verification prompt.",
                hits.join(", ")
            ),
        });
    }

    // 10. Many hyphens in the host — brand-impersonating hosts stack hyphens
    //     (e.g. secure-login-apple-verify).
    let hyphens = p.host.matches('-').count();
    if hyphens >= 3 {
        findings.push(Finding {
            severity: Severity::Low,
            check: "hyphenated-host".into(),
            message: format!(
                "Host has {} hyphens ({}) — long hyphen chains are typical of brand-impersonation domains.",
                hyphens, p.host
            ),
        });
    }

    // 11. Non-standard port.
    if let Some(port) = p.port {
        if port != 80 && port != 443 {
            findings.push(Finding {
                severity: Severity::Low,
                check: "non-standard-port".into(),
                message: format!(
                    "URL specifies a non-standard port ({}) — most legitimate web traffic uses 80 or 443.",
                    port
                ),
            });
        }
    }

    // 12. Excessive overall length.
    if url.chars().count() > 100 {
        findings.push(Finding {
            severity: Severity::Info,
            check: "excessive-length".into(),
            message: format!(
                "URL is {} characters long — very long URLs help hide the true destination behind padding.",
                url.chars().count()
            ),
        });
    }

    // 13. Digit-heavy registered-name host — random-looking algorithmically generated
    //     domains skew numeric.
    if !host_is_ip && !p.ipv6_literal && !labels.is_empty() {
        let name: String = labels[..labels.len().saturating_sub(1)].concat();
        let alnum = name.chars().filter(|c| c.is_ascii_alphanumeric()).count();
        let digits = name.chars().filter(|c| c.is_ascii_digit()).count();
        if alnum >= 6 && digits * 100 >= alnum * 40 {
            findings.push(Finding {
                severity: Severity::Info,
                check: "digit-heavy-host".into(),
                message: format!(
                    "Host name is {}% digits — a hallmark of algorithmically generated throwaway domains.",
                    digits * 100 / alnum
                ),
            });
        }
    }

    // Order: most severe first, then by check id for stable output.
    findings.sort_by(|a, b| {
        a.severity
            .rank()
            .cmp(&b.severity.rank())
            .then_with(|| a.check.cmp(&b.check))
    });

    let score: u32 = findings
        .iter()
        .map(|f| f.severity.weight())
        .sum::<u32>()
        .min(100);
    let rating = rating_for(score);
    Ok((findings, score, rating))
}

/// Inspect a URL and render a human-readable multi-line report (used by every surface).
pub fn run(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    let (findings, score, rating) = inspect(url)?;

    let mut out = String::new();
    out.push_str(&format!("Phishing risk: {} (score {}/100)\n", rating, score));
    out.push_str(&format!("URL: {}\n\n", url));

    if findings.is_empty() {
        out.push_str("Findings (0): none — no structural red flags detected in this URL.");
    } else {
        out.push_str(&format!("Findings ({}):\n", findings.len()));
        let lines: Vec<String> = findings
            .iter()
            .map(|f| format!("  [{}] {}", f.severity.label(), f.message))
            .collect();
        out.push_str(&lines.join("\n"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_clean_url_is_minimal() {
        let (findings, score, rating) = inspect("https://www.example.com/pricing").unwrap();
        assert!(findings.is_empty(), "unexpected findings: {:?}", findings);
        assert_eq!(score, 0);
        assert_eq!(rating, "MINIMAL");
    }

    #[test]
    fn ip_literal_and_userinfo_are_critical() {
        let (findings, score, rating) = inspect("http://paypal.com@192.168.0.1/login").unwrap();
        let checks: Vec<&str> = findings.iter().map(|f| f.check.as_str()).collect();
        assert!(checks.contains(&"ip-literal-host"));
        assert!(checks.contains(&"userinfo-at-sign"));
        assert!(checks.contains(&"no-https"));
        // two High (30+30) + Low no-https (9) + Low deceptive 'login' (9) = 78
        assert!(score >= 60, "score was {}", score);
        assert_eq!(rating, "CRITICAL");
    }

    #[test]
    fn punycode_and_suspicious_tld_flagged() {
        let (findings, _score, _rating) = inspect("https://xn--pple-43d.tk/").unwrap();
        let checks: Vec<&str> = findings.iter().map(|f| f.check.as_str()).collect();
        assert!(checks.contains(&"punycode-label"));
        assert!(checks.contains(&"suspicious-tld"));
    }

    #[test]
    fn excessive_subdomains_flagged() {
        let (findings, _, _) = inspect("https://login.secure.account.example.com/").unwrap();
        let checks: Vec<&str> = findings.iter().map(|f| f.check.as_str()).collect();
        assert!(checks.contains(&"excessive-subdomains"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(run("   ").is_err());
    }

    #[test]
    fn report_format_is_stable() {
        let out = run("https://www.example.com/").unwrap();
        assert_eq!(
            out,
            "Phishing risk: MINIMAL (score 0/100)\nURL: https://www.example.com/\n\nFindings (0): none — no structural red flags detected in this URL."
        );
    }

    #[test]
    fn ipv6_literal_and_port_detected() {
        let (findings, _, _) = inspect("http://[2001:db8::1]:8443/").unwrap();
        let checks: Vec<&str> = findings.iter().map(|f| f.check.as_str()).collect();
        assert!(checks.contains(&"ip-literal-host"));
        assert!(checks.contains(&"non-standard-port"));
    }
}
