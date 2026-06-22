//! ioc-extract core — extract indicators of compromise (IOCs) from text.
//! No wafer/wasm-bindgen deps; shared by the chat skill block and the web page.
//!
//! Given an arbitrary blob of text (a log line, an email, a threat report, a
//! pasted alert) this pulls out the indicators an analyst cares about:
//!
//! * IPv4 and IPv6 addresses,
//! * URLs (http/https/ftp),
//! * domain names / hostnames,
//! * email addresses, and
//! * file hashes — MD5 (32 hex), SHA-1 (40 hex), SHA-256 (64 hex), SHA-512 (128 hex).
//!
//! It accepts **defanged** input too: `hxxp[://]evil[.]com`, `1[.]2[.]3[.]4`,
//! `bad[at]evil[dot]com` and the round/curly variants are refanged before
//! matching, so you can paste indicators straight out of a CTI report. The
//! extracted indicators are de-duplicated, sorted, and grouped by type. With
//! `defang = true` the OUTPUT indicators are re-defanged so the result itself is
//! safe to paste back into a ticket.
//!
//! No external crates — the scanners are hand-rolled so the block instantiates
//! cleanly in the wafer (wasm32-wasip1) runtime.

use std::collections::BTreeSet;

/// One category of indicator, in display order.
const CATEGORIES: &[&str] = &[
    "ipv4", "ipv6", "url", "domain", "email", "md5", "sha1", "sha256", "sha512",
];

/// Extract IOCs from `text`.
///
/// * `types` — comma/space separated list of categories to include
///   (`ipv4,ipv6,url,domain,email,md5,sha1,sha256,sha512`, or `all` / empty for
///   every category). Unknown names are an error.
/// * `defang` — when `true`, the indicators in the OUTPUT are re-defanged
///   (`evil[.]com`, `hxxp[://]…`, `bad[at]evil[.]com`) so the result is safe to
///   paste; when `false` (default) they are emitted as-is (refanged).
///
/// Returns a plain-text report grouped by category, or `Err` on an unknown type.
pub fn extract(text: &str, types: &str, defang: bool) -> Result<String, String> {
    let wanted = parse_types(types)?;
    // Refang first so defanged input still matches the strict scanners.
    let refanged = refang(text);

    let mut found: Vec<(&str, BTreeSet<String>)> = Vec::new();
    for cat in CATEGORIES {
        if !wanted.contains(cat) {
            continue;
        }
        let set = scan(&refanged, cat);
        found.push((cat, set));
    }

    let total: usize = found.iter().map(|(_, s)| s.len()).sum();
    if total == 0 {
        return Ok("No indicators of compromise found.".to_string());
    }

    let mut out = String::new();
    for (cat, set) in &found {
        if set.is_empty() {
            continue;
        }
        out.push_str(&format!("{} ({}):\n", label(cat), set.len()));
        for v in set {
            let line = if defang { defang_indicator(v) } else { v.clone() };
            out.push_str("  ");
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }
    Ok(out.trim_end().to_string())
}

/// Human-readable category label.
fn label(cat: &str) -> &'static str {
    match cat {
        "ipv4" => "IPv4 addresses",
        "ipv6" => "IPv6 addresses",
        "url" => "URLs",
        "domain" => "Domains",
        "email" => "Email addresses",
        "md5" => "MD5 hashes",
        "sha1" => "SHA-1 hashes",
        "sha256" => "SHA-256 hashes",
        "sha512" => "SHA-512 hashes",
        _ => "Indicators",
    }
}

/// Parse the `types` selector into a set of canonical category names.
fn parse_types(types: &str) -> Result<BTreeSet<&'static str>, String> {
    let t = types.trim().to_ascii_lowercase();
    if t.is_empty() || t == "all" || t == "*" {
        return Ok(CATEGORIES.iter().copied().collect());
    }
    let mut set = BTreeSet::new();
    for raw in t.split([',', ' ', ';', '\n', '\t']) {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let canon = match raw {
            "ipv4" | "ip" | "ips" | "ipv4s" => "ipv4",
            "ipv6" | "ipv6s" => "ipv6",
            "url" | "urls" | "link" | "links" => "url",
            "domain" | "domains" | "host" | "hosts" | "hostname" | "hostnames" | "fqdn" => "domain",
            "email" | "emails" | "mail" | "e-mail" => "email",
            "md5" | "md5s" => "md5",
            "sha1" | "sha-1" => "sha1",
            "sha256" | "sha-256" => "sha256",
            "sha512" | "sha-512" => "sha512",
            "hash" | "hashes" => {
                set.insert("md5");
                set.insert("sha1");
                set.insert("sha256");
                set.insert("sha512");
                continue;
            }
            other => {
                return Err(format!(
                    "unknown indicator type {other:?}: expected any of \
ipv4, ipv6, url, domain, email, md5, sha1, sha256, sha512, hash, or 'all'"
                ))
            }
        };
        set.insert(canon);
    }
    if set.is_empty() {
        return Ok(CATEGORIES.iter().copied().collect());
    }
    Ok(set)
}

/// Refang common defanged conventions so the strict scanners match.
fn refang(text: &str) -> String {
    let mut out = text.to_string();
    for (l, r) in [("[", "]"), ("(", ")"), ("{", "}")] {
        out = out.replace(&format!("{l}://{r}"), "://");
        out = out.replace(&format!("{l}:{r}"), ":");
        out = out.replace(&format!("{l}at{r}"), "@");
        out = out.replace(&format!("{l}@{r}"), "@");
        out = out.replace(&format!("{l}dot{r}"), ".");
        out = out.replace(&format!("{l}.{r}"), ".");
    }
    out = out.replace("[://]", "://");
    out = out.replace("hxxps://", "https://");
    out = out.replace("hxxp://", "http://");
    out = out.replace("fxp://", "ftp://");
    out = out.replace("hxxps[:]//", "https://");
    out = out.replace("hXXp", "http");
    out
}

/// Defang a single extracted indicator for safe output.
fn defang_indicator(ind: &str) -> String {
    let mut out = ind.to_string();
    out = out.replace("https://", "hxxps[://]");
    out = out.replace("http://", "hxxp[://]");
    out = out.replace("ftp://", "fxp[://]");
    out = out.replace('@', "[at]");
    out = out.replace('.', "[.]");
    out
}

/// Dispatch to the per-category scanner.
fn scan(text: &str, cat: &str) -> BTreeSet<String> {
    match cat {
        "ipv4" => scan_ipv4(text),
        "ipv6" => scan_ipv6(text),
        "url" => scan_url(text),
        "domain" => scan_domain(text),
        "email" => scan_email(text),
        "md5" => scan_hash(text, 32),
        "sha1" => scan_hash(text, 40),
        "sha256" => scan_hash(text, 64),
        "sha512" => scan_hash(text, 128),
        _ => BTreeSet::new(),
    }
}

/// Tokenize on anything that can't be part of an indicator, keeping `.`, `:`,
/// `/`, `@`, `-`, `_`, `~`, `?`, `=`, `&`, `%`, `#`, `+` so URLs/emails survive.
fn tokens(text: &str) -> Vec<&str> {
    text.split(|c: char| {
        !(c.is_ascii_alphanumeric()
            || matches!(c, '.' | ':' | '/' | '@' | '-' | '_' | '~' | '?' | '=' | '&' | '%' | '#' | '+'))
    })
    .filter(|s| !s.is_empty())
    .collect()
}

/// Trim leading/trailing punctuation that commonly hugs an indicator in prose.
fn trim_edges(s: &str) -> &str {
    s.trim_matches(|c: char| matches!(c, '.' | ',' | ';' | ':' | '"' | '\'' | ')' | '(' | '<' | '>' | '/'))
}

fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| {
        !p.is_empty()
            && p.len() <= 3
            && p.bytes().all(|b| b.is_ascii_digit())
            && p.parse::<u16>().map(|n| n <= 255).unwrap_or(false)
            // reject leading zeros like 01 to avoid version-string false positives
            && !(p.len() > 1 && p.starts_with('0'))
    })
}

fn scan_ipv4(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for tok in tokens(text) {
        let t = trim_edges(tok);
        if is_ipv4(t) {
            set.insert(t.to_string());
        }
    }
    set
}

/// Validate an IPv6 address (full, compressed `::`, and IPv4-mapped tails).
fn is_ipv6(s: &str) -> bool {
    // Must contain a colon and only hex/colon/. (for v4-mapped) characters.
    if !s.contains(':') {
        return false;
    }
    if s.matches("::").count() > 1 {
        return false;
    }
    let has_compress = s.contains("::");
    // Split off an optional embedded IPv4 tail (e.g. ::ffff:1.2.3.4).
    let groups: Vec<&str> = s.split(':').collect();
    if groups.len() < 3 || groups.len() > 9 {
        return false;
    }
    let mut hextet_count = 0usize;
    let mut last_is_v4 = false;
    for (i, g) in groups.iter().enumerate() {
        if g.is_empty() {
            // empty group is only allowed as part of a `::`
            continue;
        }
        if i == groups.len() - 1 && g.contains('.') {
            if !is_ipv4(g) {
                return false;
            }
            hextet_count += 2;
            last_is_v4 = true;
            continue;
        }
        if g.len() > 4 || !g.bytes().all(|b| b.is_ascii_hexdigit()) {
            return false;
        }
        hextet_count += 1;
    }
    let _ = last_is_v4;
    if has_compress {
        hextet_count <= 7 && hextet_count >= 1
    } else {
        hextet_count == 8
    }
}

fn scan_ipv6(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for tok in tokens(text) {
        // strip a bracketed/zone form like [::1] or fe80::1%eth0
        let t = tok.trim_matches(|c: char| matches!(c, '[' | ']'));
        let t = t.split('%').next().unwrap_or(t);
        let t = trim_edges(t);
        if is_ipv6(t) {
            set.insert(t.to_ascii_lowercase());
        }
    }
    set
}

fn scan_url(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for tok in tokens(text) {
        let t = trim_edges(tok);
        let lower = t.to_ascii_lowercase();
        if (lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("ftp://"))
            && t.len() > 8
            // must have a host after the scheme
            && t[t.find("://").map(|i| i + 3).unwrap_or(t.len())..].len() >= 3
        {
            set.insert(t.to_string());
        }
    }
    set
}

/// A label/host is valid if it's dotted, the TLD is alphabetic and >=2 chars,
/// and every label is alnum/hyphen (not starting/ending with a hyphen).
fn is_domain(s: &str) -> bool {
    if s.len() > 253 || !s.contains('.') {
        return false;
    }
    // exclude things that are clearly IPs
    if is_ipv4(s) {
        return false;
    }
    let labels: Vec<&str> = s.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    let tld = labels[labels.len() - 1];
    if tld.len() < 2 || !tld.bytes().all(|b| b.is_ascii_alphabetic()) {
        return false;
    }
    labels.iter().all(|l| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

fn scan_domain(text: &str) -> BTreeSet<String> {
    // Domains embedded in URLs / emails are excluded so the categories don't
    // overlap; only bare hostnames are reported here.
    let urls = scan_url(text);
    let emails = scan_email(text);
    let mut set = BTreeSet::new();
    'outer: for tok in tokens(text) {
        let t = trim_edges(tok);
        // skip if it's part of a URL or email token
        if t.contains("://") || t.contains('@') {
            continue;
        }
        // strip a path if any (host/path) — only keep the host part for matching
        let host = t.split('/').next().unwrap_or(t);
        if !is_domain(host) {
            continue;
        }
        // ensure it's not already captured inside a URL/email's host
        for u in &urls {
            if u.to_ascii_lowercase().contains(&host.to_ascii_lowercase()) {
                continue 'outer;
            }
        }
        for e in &emails {
            if e.to_ascii_lowercase().ends_with(&host.to_ascii_lowercase()) {
                continue 'outer;
            }
        }
        set.insert(host.to_ascii_lowercase());
    }
    set
}

fn is_email(s: &str) -> bool {
    let at = s.find('@');
    let at = match at {
        Some(i) if s.matches('@').count() == 1 => i,
        _ => return false,
    };
    let (local, domain) = (&s[..at], &s[at + 1..]);
    if local.is_empty() || local.len() > 64 {
        return false;
    }
    if !local
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+' | b'%'))
    {
        return false;
    }
    is_domain(domain)
}

fn scan_email(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for tok in tokens(text) {
        let t = trim_edges(tok);
        if is_email(t) {
            set.insert(t.to_ascii_lowercase());
        }
    }
    set
}

/// Hashes are bare hex strings of an exact length. Tokenize on hex boundaries.
fn scan_hash(text: &str, len: usize) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for tok in text.split(|c: char| !c.is_ascii_hexdigit()) {
        if tok.len() == len {
            set.insert(tok.to_ascii_lowercase());
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_mixed_iocs() {
        let text = "Attacker 203.0.113.5 hit http://evil.example.com from bad@phish.net \
md5 d41d8cd98f00b204e9800998ecf8427e";
        let out = extract(text, "all", false).unwrap();
        assert!(out.contains("203.0.113.5"), "{out}");
        assert!(out.contains("http://evil.example.com"), "{out}");
        assert!(out.contains("bad@phish.net"), "{out}");
        assert!(out.contains("d41d8cd98f00b204e9800998ecf8427e"), "{out}");
    }

    #[test]
    fn refangs_defanged_input() {
        let text = "C2 at hxxp[://]evil[.]com and 10[.]0[.]0[.]1 and bad[at]evil[.]com";
        let out = extract(text, "all", false).unwrap();
        assert!(out.contains("http://evil.com"), "{out}");
        assert!(out.contains("10.0.0.1"), "{out}");
        assert!(out.contains("bad@evil.com"), "{out}");
    }

    #[test]
    fn defang_output() {
        let out = extract("visit http://evil.com", "url", true).unwrap();
        assert!(out.contains("hxxp[://]evil[.]com"), "{out}");
        assert!(!out.contains("http://evil.com"), "{out}");
    }

    #[test]
    fn type_filter_restricts() {
        let text = "1.2.3.4 and evil.com and a@b.com";
        let out = extract(text, "ipv4", false).unwrap();
        assert!(out.contains("1.2.3.4"), "{out}");
        assert!(!out.contains("evil.com"), "{out}");
        assert!(!out.contains("a@b.com"), "{out}");
    }

    #[test]
    fn dedup_and_sort() {
        let out = extract("8.8.8.8 8.8.8.8 1.1.1.1", "ipv4", false).unwrap();
        // 1.1.1.1 sorts before 8.8.8.8 and each appears once
        let idx1 = out.find("1.1.1.1").unwrap();
        let idx8 = out.find("8.8.8.8").unwrap();
        assert!(idx1 < idx8, "{out}");
        assert_eq!(out.matches("8.8.8.8").count(), 1, "{out}");
    }

    #[test]
    fn ipv6_full_and_compressed() {
        let out = extract("hosts 2001:0db8:0000:0000:0000:ff00:0042:8329 and ::1 and fe80::1", "ipv6", false).unwrap();
        assert!(out.contains("2001:0db8:0000:0000:0000:ff00:0042:8329"), "{out}");
        assert!(out.contains("::1"), "{out}");
        assert!(out.contains("fe80::1"), "{out}");
    }

    #[test]
    fn hashes_by_length() {
        let md5 = "d41d8cd98f00b204e9800998ecf8427e"; // 32
        let sha1 = "da39a3ee5e6b4b0d3255bfef95601890afd80709"; // 40
        let sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"; // 64
        let text = format!("{md5} {sha1} {sha256}");
        let out = extract(&text, "hash", false).unwrap();
        assert!(out.contains("MD5"), "{out}");
        assert!(out.contains(md5), "{out}");
        assert!(out.contains(sha1), "{out}");
        assert!(out.contains(sha256), "{out}");
    }

    #[test]
    fn no_false_positive_version_string() {
        // 1.2.3 (3 octets) is not an IPv4; leading-zero octet rejected
        let out = extract("version 1.2.3 build 01.02.03.04", "ipv4", false).unwrap();
        assert!(out.contains("No indicators"), "{out}");
    }

    #[test]
    fn domain_excludes_url_and_email_hosts() {
        let text = "http://a.com b.com c@d.com";
        let out = extract(text, "domain", false).unwrap();
        assert!(out.contains("b.com"), "{out}");
        assert!(!out.contains("a.com"), "{out}");
        assert!(!out.contains("d.com"), "{out}");
    }

    #[test]
    fn empty_text_reports_none() {
        let out = extract("nothing here at all", "all", false).unwrap();
        assert_eq!(out, "No indicators of compromise found.");
    }

    #[test]
    fn rejects_unknown_type() {
        let e = extract("x", "frobnicate", false).unwrap_err();
        assert!(e.contains("unknown indicator type"), "{e}");
    }

    #[test]
    fn type_aliases_and_case() {
        let out = extract("1.2.3.4", "IPs", false).unwrap();
        assert!(out.contains("1.2.3.4"), "{out}");
    }
}
