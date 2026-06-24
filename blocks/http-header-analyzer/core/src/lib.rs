//! http-header-analyzer core — parse a block of pasted HTTP response headers
//! and explain what they do: caching, compression, content negotiation, server
//! hints, cookies, CORS, and — importantly — which recommended **security**
//! headers are MISSING. Pure-Rust, no wafer/wasm-bindgen deps.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Header {
    pub name: String,
    pub value: String,
    /// A plain-English explanation of what this header does.
    pub explanation: String,
    /// One of: "caching", "compression", "content", "security", "cookie",
    /// "cors", "server", "redirect", "other".
    pub category: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// "warning" or "info".
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MissingSecurityHeader {
    pub name: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Analysis {
    /// Status line, if the paste began with one (e.g. "HTTP/2 200"). Optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_line: Option<String>,
    /// Every parsed header in order, annotated.
    pub headers: Vec<Header>,
    pub header_count: usize,
    /// Recommended security headers that were NOT present.
    pub missing_security_headers: Vec<MissingSecurityHeader>,
    /// Additional notes / warnings (e.g. no caching, no compression).
    pub findings: Vec<Finding>,
    /// An overall letter grade (A+ … F) for the response's security headers,
    /// based on how many of the recommended headers are present minus penalties
    /// for weak values (e.g. CSP unsafe-inline, short HSTS, deprecated headers).
    pub security_grade: String,
}

/// Lower-case a header name for matching.
fn key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

/// Explain a single header by its lower-cased name. Returns (explanation, category).
fn explain(lname: &str, value: &str) -> (String, &'static str) {
    let v = value.trim();
    match lname {
        // --- caching ---
        "cache-control" => {
            let mut parts: Vec<String> = Vec::new();
            let lv = v.to_ascii_lowercase();
            if lv.contains("no-store") {
                parts.push("the response must not be stored by any cache".into());
            }
            if lv.contains("no-cache") {
                parts.push("a cache must revalidate with the origin before reuse".into());
            }
            if lv.contains("private") {
                parts.push("only the browser (not shared caches) may store it".into());
            }
            if lv.contains("public") {
                parts.push("any cache may store it".into());
            }
            if lv.contains("immutable") {
                parts.push("the body will not change, so the browser need not revalidate".into());
            }
            if let Some(age) = directive_value(&lv, "max-age") {
                parts.push(format!("it may be reused for {age} seconds"));
            }
            if let Some(age) = directive_value(&lv, "s-maxage") {
                parts.push(format!("shared caches may reuse it for {age} seconds"));
            }
            let detail = if parts.is_empty() {
                "controls how the response may be cached".to_string()
            } else {
                format!("Caching policy: {}.", parts.join("; "))
            };
            (detail, "caching")
        }
        "expires" => (
            format!("Legacy caching: the response is considered stale after {v} (superseded by Cache-Control: max-age)."),
            "caching",
        ),
        "etag" => (
            "A version identifier for the body; the browser sends it back in If-None-Match so the server can answer 304 Not Modified when unchanged.".into(),
            "caching",
        ),
        "last-modified" => (
            "When the resource was last changed; used with If-Modified-Since for conditional requests.".into(),
            "caching",
        ),
        "age" => (
            format!("The response has been sitting in an intermediate cache for {v} seconds."),
            "caching",
        ),
        "vary" => (
            format!("Caches must key separately on these request headers: {v}. A 'Vary: *' or many values can hurt cache hit-rates."),
            "caching",
        ),

        // --- compression / transfer ---
        "content-encoding" => (
            format!("The body is compressed with '{v}' (the browser decompresses it transparently). Good for reducing transfer size."),
            "compression",
        ),
        "transfer-encoding" => (
            format!("Transfer-level encoding: '{v}' (e.g. chunked streaming, or gzip applied hop-by-hop)."),
            "compression",
        ),

        // --- content / negotiation ---
        "content-type" => (
            format!("The media type of the body is '{v}'. Include a charset (e.g. ; charset=utf-8) for text to avoid mojibake."),
            "content",
        ),
        "content-length" => (
            format!("The body is {v} bytes."),
            "content",
        ),
        "content-language" => (
            format!("The natural language of the body is '{v}'."),
            "content",
        ),
        "content-disposition" => (
            format!("Tells the browser how to present the body: '{v}' (e.g. inline, or attachment to force a download)."),
            "content",
        ),
        "accept-ranges" => (
            format!("The server advertises range support ('{v}'), enabling resumable / partial downloads."),
            "content",
        ),
        "link" => (
            "Web Linking header (RFC 8288) — e.g. preload/preconnect hints or pagination (rel=next/prev).".into(),
            "content",
        ),

        // --- security ---
        "strict-transport-security" => (
            format!("HSTS: forces HTTPS for this host. {}", hsts_note(v)),
            "security",
        ),
        "content-security-policy" => (
            "Content-Security-Policy restricts which sources scripts, styles, images, etc. may load from — the strongest defence against XSS and injection.".into(),
            "security",
        ),
        "content-security-policy-report-only" => (
            "CSP in report-only mode: violations are reported but not blocked — useful for testing a policy before enforcing it.".into(),
            "security",
        ),
        "x-content-type-options" => {
            if v.eq_ignore_ascii_case("nosniff") {
                ("Stops the browser from MIME-sniffing the body, so it won't reinterpret a response as a different (possibly executable) type.".into(), "security")
            } else {
                (format!("Expected 'nosniff'; got '{v}'."), "security")
            }
        }
        "x-frame-options" => (
            format!("Clickjacking protection: '{v}' controls whether the page may be framed (DENY / SAMEORIGIN). The modern equivalent is CSP frame-ancestors."),
            "security",
        ),
        "referrer-policy" => (
            format!("Controls how much of the URL is sent in the Referer header on outgoing requests ('{v}')."),
            "security",
        ),
        "permissions-policy" | "feature-policy" => (
            "Enables/disables browser features (camera, geolocation, etc.) for this page and its iframes.".into(),
            "security",
        ),
        "cross-origin-opener-policy" => (
            format!("COOP ('{v}') isolates this page's browsing-context group, a prerequisite for cross-origin isolation (and some powerful APIs)."),
            "security",
        ),
        "cross-origin-embedder-policy" => (
            format!("COEP ('{v}') requires cross-origin resources to opt in (CORP/CORS), enabling cross-origin isolation."),
            "security",
        ),
        "cross-origin-resource-policy" => (
            format!("CORP ('{v}') limits which origins may embed this resource."),
            "security",
        ),

        // --- cookies ---
        "set-cookie" => (
            format!("Sets a cookie. {}", cookie_note(v)),
            "cookie",
        ),

        // --- CORS ---
        "access-control-allow-origin" => {
            let note = if v == "*" {
                " Any origin is allowed; '*' cannot be combined with credentialed requests."
            } else {
                ""
            };
            (format!("CORS: cross-origin reads are permitted from '{v}'.{note}"), "cors")
        }
        "access-control-allow-credentials" => (
            format!("CORS: cookies/credentials are {} on cross-origin requests.", if v.eq_ignore_ascii_case("true") { "allowed" } else { "not allowed" }),
            "cors",
        ),
        "access-control-allow-methods" => (
            format!("CORS: cross-origin requests may use these methods: {v}."),
            "cors",
        ),
        "access-control-allow-headers" => (
            format!("CORS: cross-origin requests may send these headers: {v}."),
            "cors",
        ),
        "access-control-expose-headers" => (
            format!("CORS: scripts may read these response headers cross-origin: {v}."),
            "cors",
        ),
        "access-control-max-age" => (
            format!("CORS: the preflight result may be cached for {v} seconds."),
            "cors",
        ),

        // --- server hints / fingerprinting ---
        "server" => (
            format!("Identifies the server software ('{v}'). Consider trimming it to avoid leaking exact versions to attackers."),
            "server",
        ),
        "x-powered-by" => (
            format!("Reveals a framework/runtime ('{v}'). Often removed to reduce fingerprinting."),
            "server",
        ),
        "via" => (
            format!("The response passed through these proxies/gateways: {v}."),
            "server",
        ),
        "x-cache" => (
            format!("CDN/proxy cache result: '{v}' (e.g. HIT means served from the edge cache)."),
            "server",
        ),
        "date" => (
            format!("The time the response was generated: {v}."),
            "server",
        ),
        "alt-svc" => (
            format!("Advertises alternative endpoints/protocols (e.g. HTTP/3 over QUIC): {v}."),
            "server",
        ),

        // --- redirect ---
        "location" => (
            format!("Redirect / created-resource target: {v}."),
            "redirect",
        ),
        "retry-after" => (
            format!("Asks the client to wait before retrying: {v} (seconds, or an HTTP date). Common with 429 / 503."),
            "redirect",
        ),

        _ => (
            "Non-standard or unrecognised header — passed through without analysis.".into(),
            "other",
        ),
    }
}

/// Pull a `name=value` directive out of a directive list. Cache-Control uses
/// commas, HSTS uses semicolons — split on either so both work.
fn directive_value(lv: &str, name: &str) -> Option<String> {
    for part in lv.split([',', ';']) {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix(name) {
            if let Some(num) = rest.strip_prefix('=') {
                return Some(num.trim().to_string());
            }
        }
    }
    None
}

fn hsts_note(v: &str) -> String {
    let lv = v.to_ascii_lowercase();
    let mut notes: Vec<String> = Vec::new();
    if let Some(age) = directive_value(&lv, "max-age") {
        notes.push(format!("max-age is {age} seconds"));
        if age == "0" {
            notes.push("max-age=0 disables HSTS".into());
        }
    } else {
        notes.push("no max-age — HSTS needs a max-age to take effect".into());
    }
    if lv.contains("includesubdomains") {
        notes.push("applies to subdomains".into());
    }
    if lv.contains("preload") {
        notes.push("eligible for the browser preload list".into());
    }
    format!("({}).", notes.join("; "))
}

fn cookie_note(v: &str) -> String {
    let lv = v.to_ascii_lowercase();
    let mut notes: Vec<String> = Vec::new();
    if !lv.contains("secure") {
        notes.push("missing 'Secure' (cookie may be sent over plain HTTP)".into());
    }
    if !lv.contains("httponly") {
        notes.push("missing 'HttpOnly' (cookie is readable from JavaScript)".into());
    }
    if !lv.contains("samesite") {
        notes.push("no 'SameSite' (defaults vary; set it explicitly to limit CSRF)".into());
    }
    if notes.is_empty() {
        "Has Secure, HttpOnly, and SameSite set.".into()
    } else {
        format!("Hardening gaps: {}.", notes.join("; "))
    }
}

/// The recommended security headers we check for, with advice when absent.
fn recommended_security() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Strict-Transport-Security", "Add HSTS (e.g. max-age=31536000; includeSubDomains) so browsers always use HTTPS for this host."),
        ("Content-Security-Policy", "Add a Content-Security-Policy to restrict script/style/image sources — the strongest XSS defence."),
        ("X-Content-Type-Options", "Add 'X-Content-Type-Options: nosniff' to stop MIME-sniffing."),
        ("X-Frame-Options", "Add 'X-Frame-Options: SAMEORIGIN' (or a CSP frame-ancestors directive) to prevent clickjacking."),
        ("Referrer-Policy", "Add a Referrer-Policy (e.g. strict-origin-when-cross-origin) to limit referrer leakage."),
        ("Permissions-Policy", "Add a Permissions-Policy to disable browser features the page does not use."),
    ]
}

/// Strip a leading HTTP status line if present, returning (Option<status>, rest).
fn split_status_line(lines: &[&str]) -> (Option<String>, usize) {
    if let Some(first) = lines.first() {
        let t = first.trim();
        // A status line starts with "HTTP/" and has no ':' before the first space.
        if t.to_ascii_uppercase().starts_with("HTTP/") && !t.split(' ').next().unwrap_or("").contains(':') {
            return (Some(t.to_string()), 1);
        }
    }
    (None, 0)
}

/// Analyze a block of pasted HTTP response headers.
pub fn analyze(input: &str) -> Result<Analysis, String> {
    let trimmed = input.trim_start_matches('\u{feff}');
    if trimmed.trim().is_empty() {
        return Err("input is empty — paste one or more HTTP response headers (Name: value per line)".into());
    }

    // Normalise: split on LF, drop trailing CR. A blank line ends the header block.
    let raw_lines: Vec<&str> = trimmed.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    let (status_line, mut idx) = split_status_line(&raw_lines);

    let mut headers: Vec<Header> = Vec::new();
    while idx < raw_lines.len() {
        let line = raw_lines[idx];
        idx += 1;
        if line.trim().is_empty() {
            // blank line terminates the header section
            break;
        }
        // Obsolete line folding: a continuation line begins with whitespace.
        if (line.starts_with(' ') || line.starts_with('\t')) && !headers.is_empty() {
            let last = headers.last_mut().unwrap();
            last.value.push(' ');
            last.value.push_str(line.trim());
            let (ex, cat) = explain(&key(&last.name), &last.value);
            last.explanation = ex;
            last.category = cat.to_string();
            continue;
        }
        let (name, value) = match line.split_once(':') {
            Some((n, v)) => (n.trim().to_string(), v.trim().to_string()),
            None => {
                return Err(format!(
                    "line '{line}' is not a 'Name: value' header (missing ':')"
                ));
            }
        };
        if name.is_empty() {
            return Err(format!("line '{line}' has an empty header name"));
        }
        let (ex, cat) = explain(&key(&name), &value);
        headers.push(Header {
            name,
            value,
            explanation: ex,
            category: cat.to_string(),
        });
    }

    if headers.is_empty() {
        return Err("no headers found — each header must be on its own 'Name: value' line".into());
    }

    let present: std::collections::HashSet<String> =
        headers.iter().map(|h| key(&h.name)).collect();

    // Missing recommended security headers.
    let mut missing = Vec::new();
    for (name, advice) in recommended_security() {
        if !present.contains(&key(name)) {
            missing.push(MissingSecurityHeader {
                name: name.to_string(),
                recommendation: advice.to_string(),
            });
        }
    }

    // Extra findings.
    let mut findings = Vec::new();
    let has_caching = present.contains("cache-control")
        || present.contains("expires")
        || present.contains("etag")
        || present.contains("last-modified");
    if !has_caching {
        findings.push(Finding {
            severity: "info".into(),
            message: "No caching headers (Cache-Control / ETag / Last-Modified) — every request will re-download this resource.".into(),
        });
    }
    if !present.contains("content-encoding") {
        findings.push(Finding {
            severity: "info".into(),
            message: "No Content-Encoding — the body is sent uncompressed. Enable gzip/br for text responses to cut transfer size.".into(),
        });
    }
    if let Some(ct) = headers.iter().find(|h| key(&h.name) == "content-type") {
        let lv = ct.value.to_ascii_lowercase();
        let is_text = lv.starts_with("text/")
            || lv.contains("json")
            || lv.contains("javascript")
            || lv.contains("xml")
            || lv.contains("html");
        if is_text && !lv.contains("charset") {
            findings.push(Finding {
                severity: "warning".into(),
                message: format!("Content-Type '{}' has no charset — add '; charset=utf-8' to avoid encoding ambiguity.", ct.value),
            });
        }
    }
    // Fingerprinting / information-disclosure headers to remove.
    let disclosure: Vec<&str> = [
        "server",
        "x-powered-by",
        "x-aspnet-version",
        "x-aspnetmvc-version",
    ]
    .iter()
    .filter(|h| present.contains(**h))
    .copied()
    .collect();
    if !disclosure.is_empty() {
        findings.push(Finding {
            severity: "info".into(),
            message: format!(
                "These headers reveal software/versions and aid fingerprinting — consider removing or trimming them: {}.",
                disclosure.join(", ")
            ),
        });
    }

    // --- Value-quality checks on PRESENT security headers (penalties feed the grade) ---
    let val = |name: &str| -> Option<String> {
        headers
            .iter()
            .find(|h| key(&h.name) == name)
            .map(|h| h.value.to_ascii_lowercase())
    };
    let mut weak_penalty = 0u32;

    if let Some(csp) = val("content-security-policy") {
        if csp.contains("unsafe-inline") {
            weak_penalty += 1;
            findings.push(Finding {
                severity: "warning".into(),
                message: "Content-Security-Policy allows 'unsafe-inline', which lets inline scripts/styles run and largely defeats CSP's XSS protection.".into(),
            });
        }
        if csp.contains("unsafe-eval") {
            weak_penalty += 1;
            findings.push(Finding {
                severity: "warning".into(),
                message: "Content-Security-Policy allows 'unsafe-eval', permitting eval()-style code execution — avoid it where possible.".into(),
            });
        }
    }
    if let Some(hsts) = val("strict-transport-security") {
        if let Some(age) = directive_value(&hsts, "max-age") {
            if let Ok(secs) = age.parse::<u64>() {
                if secs == 0 {
                    weak_penalty += 1;
                    findings.push(Finding {
                        severity: "warning".into(),
                        message: "Strict-Transport-Security has max-age=0, which disables HSTS.".into(),
                    });
                } else if secs < 31_536_000 {
                    weak_penalty += 1;
                    findings.push(Finding {
                        severity: "info".into(),
                        message: format!("HSTS max-age is {secs}s; use at least 31536000 (1 year) — and includeSubDomains; preload — to qualify for the browser preload list."),
                    });
                }
            }
        }
    }
    if let Some(rp) = val("referrer-policy") {
        if rp.contains("unsafe-url") || rp == "no-referrer-when-downgrade" {
            weak_penalty += 1;
            findings.push(Finding {
                severity: "info".into(),
                message: format!("Referrer-Policy '{rp}' is weak — prefer 'strict-origin-when-cross-origin' or 'no-referrer'."),
            });
        }
    }
    if let Some(xfo) = val("x-frame-options") {
        if !xfo.contains("deny") && !xfo.contains("sameorigin") {
            weak_penalty += 1;
            findings.push(Finding {
                severity: "warning".into(),
                message: format!("X-Frame-Options '{xfo}' is not DENY or SAMEORIGIN; the ALLOW-FROM form is obsolete — use a CSP frame-ancestors directive instead."),
            });
        }
    }
    if present.contains("x-xss-protection") {
        findings.push(Finding {
            severity: "info".into(),
            message: "X-XSS-Protection is deprecated and can introduce vulnerabilities — set it to '0' or remove it and rely on Content-Security-Policy.".into(),
        });
    }

    if !missing.is_empty() {
        findings.push(Finding {
            severity: "warning".into(),
            message: format!("{} recommended security header(s) are missing — see missing_security_headers.", missing.len()),
        });
    }

    // Overall grade: start from how many of the recommended security headers are
    // present, then dock for weak values. (Like securityheaders.com's letter grade.)
    let recommended = recommended_security();
    let present_count = recommended.len() - missing.len();
    let security_grade =
        grade(present_count, recommended.len(), weak_penalty).to_string();

    let header_count = headers.len();
    Ok(Analysis {
        status_line,
        headers,
        header_count,
        missing_security_headers: missing,
        findings,
        security_grade,
    })
}

/// Compute a letter grade from how many recommended security headers are present
/// (out of `total`) minus a penalty for weak header values.
fn grade(present: usize, total: usize, weak_penalty: u32) -> &'static str {
    // Effective score in [0, total]: each present header is worth 1, each weak
    // value costs ~half a header.
    let raw = present as f64 - (weak_penalty as f64) * 0.5;
    let pct = (raw.max(0.0) / total as f64) * 100.0;
    if present == total && weak_penalty == 0 {
        "A+"
    } else if pct >= 90.0 {
        "A"
    } else if pct >= 75.0 {
        "B"
    } else if pct >= 55.0 {
        "C"
    } else if pct >= 35.0 {
        "D"
    } else if pct >= 15.0 {
        "E"
    } else {
        "F"
    }
}

/// Analyze and return pretty JSON (chat / programmatic surface).
pub fn run(input: &str) -> Result<String, String> {
    let a = analyze(input)?;
    serde_json::to_string_pretty(&a).map_err(|e| e.to_string())
}

/// Human-readable rendering (used by the page).
pub fn render(input: &str) -> Result<String, String> {
    let a = analyze(input)?;
    let mut out = String::new();
    out.push_str(&format!("Security grade: {}\n\n", a.security_grade));
    if let Some(s) = &a.status_line {
        out.push_str(&format!("Status line: {s}\n\n"));
    }
    out.push_str(&format!("Headers ({}):\n", a.header_count));
    for h in &a.headers {
        out.push_str(&format!("\n  {}: {}\n", h.name, h.value));
        out.push_str(&format!("    [{}] {}\n", h.category, h.explanation));
    }

    out.push_str(&format!(
        "\nMissing recommended security headers ({}):\n",
        a.missing_security_headers.len()
    ));
    if a.missing_security_headers.is_empty() {
        out.push_str("  (none — all checked security headers are present)\n");
    } else {
        for m in &a.missing_security_headers {
            out.push_str(&format!("  - {}: {}\n", m.name, m.recommendation));
        }
    }

    if !a.findings.is_empty() {
        out.push_str(&format!("\nNotes ({}):\n", a.findings.len()));
        for f in &a.findings {
            out.push_str(&format!("  [{}] {}\n", f.severity, f.message));
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADERS: &str = "HTTP/2 200\nContent-Type: text/html; charset=utf-8\nCache-Control: public, max-age=3600\nContent-Encoding: gzip\nStrict-Transport-Security: max-age=31536000; includeSubDomains\nServer: nginx/1.25.0\n";

    #[test]
    fn parses_status_line() {
        let a = analyze(HEADERS).unwrap();
        assert_eq!(a.status_line.as_deref(), Some("HTTP/2 200"));
        assert_eq!(a.header_count, 5);
    }

    #[test]
    fn explains_cache_control() {
        let a = analyze("Cache-Control: public, max-age=3600\n").unwrap();
        let h = &a.headers[0];
        assert_eq!(h.category, "caching");
        assert!(h.explanation.contains("3600 seconds"));
        assert!(h.explanation.to_lowercase().contains("any cache may store"));
    }

    #[test]
    fn explains_compression() {
        let a = analyze("Content-Encoding: br\n").unwrap();
        assert_eq!(a.headers[0].category, "compression");
        assert!(a.headers[0].explanation.contains("br"));
    }

    #[test]
    fn flags_missing_security_headers() {
        let a = analyze("Content-Type: text/html; charset=utf-8\n").unwrap();
        let names: Vec<&str> = a
            .missing_security_headers
            .iter()
            .map(|m| m.name.as_str())
            .collect();
        assert!(names.contains(&"Content-Security-Policy"));
        assert!(names.contains(&"Strict-Transport-Security"));
        assert!(names.contains(&"X-Content-Type-Options"));
    }

    #[test]
    fn present_security_header_not_flagged_missing() {
        let a = analyze(HEADERS).unwrap();
        let names: Vec<&str> = a
            .missing_security_headers
            .iter()
            .map(|m| m.name.as_str())
            .collect();
        // HSTS is present in HEADERS, so it must not be in the missing list.
        assert!(!names.contains(&"Strict-Transport-Security"));
        // CSP is absent, so it must be flagged.
        assert!(names.contains(&"Content-Security-Policy"));
    }

    #[test]
    fn warns_on_missing_charset() {
        let a = analyze("Content-Type: text/html\n").unwrap();
        assert!(a
            .findings
            .iter()
            .any(|f| f.message.contains("charset") && f.severity == "warning"));
    }

    #[test]
    fn flags_cookie_hardening_gaps() {
        let a = analyze("Set-Cookie: id=abc; Path=/\n").unwrap();
        let h = &a.headers[0];
        assert_eq!(h.category, "cookie");
        assert!(h.explanation.contains("HttpOnly"));
        assert!(h.explanation.contains("Secure"));
        assert!(h.explanation.contains("SameSite"));
    }

    #[test]
    fn hsts_without_max_age_noted() {
        let a = analyze("Strict-Transport-Security: includeSubDomains\n").unwrap();
        assert!(a.headers[0].explanation.contains("no max-age"));
    }

    #[test]
    fn hsts_max_age_parsed_cleanly() {
        // HSTS uses ';' separators — max-age must not absorb later directives.
        let a = analyze("Strict-Transport-Security: max-age=31536000; includeSubDomains\n").unwrap();
        assert!(a.headers[0].explanation.contains("max-age is 31536000 seconds"));
        assert!(a.headers[0].explanation.contains("applies to subdomains"));
    }

    #[test]
    fn folds_continuation_lines() {
        let a = analyze("Content-Security-Policy: default-src 'self';\n  script-src 'self'\n").unwrap();
        assert_eq!(a.header_count, 1);
        assert!(a.headers[0].value.contains("script-src"));
    }

    #[test]
    fn stops_at_blank_line() {
        let a = analyze("Server: nginx\n\nthis is body, not a header\n").unwrap();
        assert_eq!(a.header_count, 1);
        assert_eq!(a.headers[0].name, "Server");
    }

    #[test]
    fn no_caching_finding() {
        let a = analyze("Content-Type: text/plain; charset=utf-8\n").unwrap();
        assert!(a.findings.iter().any(|f| f.message.contains("No caching")));
    }

    #[test]
    fn run_emits_json() {
        let j = run(HEADERS).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["status_line"], "HTTP/2 200");
        assert_eq!(v["headers"][0]["name"], "Content-Type");
        assert!(v["missing_security_headers"].is_array());
        assert!(v["security_grade"].is_string());
    }

    const ALL_SECURE: &str = "Strict-Transport-Security: max-age=63072000; includeSubDomains; preload\nContent-Security-Policy: default-src 'self'\nX-Content-Type-Options: nosniff\nX-Frame-Options: DENY\nReferrer-Policy: strict-origin-when-cross-origin\nPermissions-Policy: geolocation=()\n";

    #[test]
    fn grades_a_plus_when_all_present_and_strong() {
        let a = analyze(ALL_SECURE).unwrap();
        assert!(a.missing_security_headers.is_empty());
        assert_eq!(a.security_grade, "A+");
    }

    #[test]
    fn grades_f_when_none_present() {
        let a = analyze("Content-Type: text/html; charset=utf-8\n").unwrap();
        assert_eq!(a.security_grade, "F");
    }

    #[test]
    fn flags_csp_unsafe_inline_and_drops_grade() {
        let a = analyze(
            "Strict-Transport-Security: max-age=63072000\nContent-Security-Policy: default-src 'self'; script-src 'unsafe-inline'\nX-Content-Type-Options: nosniff\nX-Frame-Options: DENY\nReferrer-Policy: strict-origin-when-cross-origin\nPermissions-Policy: geolocation=()\n",
        )
        .unwrap();
        assert!(a.findings.iter().any(|f| f.message.contains("unsafe-inline")));
        // All present but one weak value → not A+.
        assert_ne!(a.security_grade, "A+");
    }

    #[test]
    fn flags_short_hsts_max_age() {
        let a = analyze("Strict-Transport-Security: max-age=300\n").unwrap();
        assert!(a
            .findings
            .iter()
            .any(|f| f.message.contains("preload") || f.message.contains("31536000")));
    }

    #[test]
    fn flags_deprecated_xss_protection() {
        let a = analyze("X-XSS-Protection: 1; mode=block\n").unwrap();
        assert!(a.findings.iter().any(|f| f.message.contains("deprecated")));
    }

    #[test]
    fn flags_obsolete_x_frame_options_value() {
        let a = analyze("X-Frame-Options: ALLOW-FROM https://example.com\n").unwrap();
        assert!(a
            .findings
            .iter()
            .any(|f| f.message.contains("frame-ancestors")));
    }

    #[test]
    fn lists_aspnet_version_as_disclosure() {
        let a = analyze("X-AspNet-Version: 4.0.30319\n").unwrap();
        assert!(a
            .findings
            .iter()
            .any(|f| f.message.to_lowercase().contains("fingerprint")
                && f.message.contains("x-aspnet-version")));
    }

    #[test]
    fn render_human_readable() {
        let r = render(HEADERS).unwrap();
        assert!(r.contains("Security grade:"));
        assert!(r.contains("Status line: HTTP/2 200"));
        assert!(r.contains("Cache-Control"));
        assert!(r.contains("Missing recommended security headers"));
    }

    #[test]
    fn errors_on_empty() {
        assert!(analyze("").is_err());
        assert!(analyze("   \n\n").is_err());
    }

    #[test]
    fn errors_on_non_header_line() {
        assert!(analyze("this has no colon\n").is_err());
    }
}
