//! email-phishing-link-scanner core — scan every link in a pasted email and rate each one
//! for phishing risk. Pure compute, shared by the chat skill block, the CLI, and the web page.
//! No wafer/wasm-bindgen deps, no network, no blocklists, no model.
//!
//! Three things distinguish this from a single-URL rater:
//!
//! 1. It takes a WHOLE email (raw RFC 5322, an HTML body, or plain text), pulls out every
//!    `<a href>` and every bare URL, and reports each link separately with its own findings.
//! 2. It compares each link's VISIBLE TEXT against its actual target, so the classic
//!    "shows paypal.com, goes to 10.0.0.9" trick is caught per link.
//! 3. It compares each link's domain against a brand list (built in + your own), folding
//!    homoglyphs, digit-for-letter swaps and punycode, so `paypa1-secure.com` and
//!    `xn--pypal-4ve.com` are recognised as impersonations of `paypal.com`.
//!
//! Everything is deterministic: the same message always yields the same report.

use serde::Serialize;

/// Hard cap on the pasted message, to keep a browser wasm run bounded.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
/// Upper bound accepted for the `max_links` parameter.
pub const MAX_LINKS_LIMIT: i64 = 1000;

// ---------------------------------------------------------------------------------------------
// Curated lists. Snapshots, deliberately not exhaustive — see the page's "Limits" section.
// ---------------------------------------------------------------------------------------------

/// Domains that phishing impersonates most often. Lookalike matching runs against these plus
/// anything the caller passes in `brands` plus the message's own sender domain.
const BRAND_DOMAINS: &[&str] = &[
    "adobe.com",
    "airbnb.com",
    "amazon.com",
    "americanexpress.com",
    "apple.com",
    "bankofamerica.com",
    "barclays.co.uk",
    "binance.com",
    "booking.com",
    "chase.com",
    "citibank.com",
    "coinbase.com",
    "discord.com",
    "docusign.com",
    "dropbox.com",
    "facebook.com",
    "fedex.com",
    "github.com",
    "gitlab.com",
    "gmail.com",
    "google.com",
    "hsbc.com",
    "instagram.com",
    "linkedin.com",
    "mailchimp.com",
    "metamask.io",
    "microsoft.com",
    "netflix.com",
    "office365.com",
    "onedrive.com",
    "outlook.com",
    "paypal.com",
    "revolut.com",
    "roblox.com",
    "santander.com",
    "sharepoint.com",
    "shopify.com",
    "slack.com",
    "spotify.com",
    "steampowered.com",
    "stripe.com",
    "telegram.org",
    "tiktok.com",
    "twitter.com",
    "uber.com",
    "usps.com",
    "walmart.com",
    "wellsfargo.com",
    "whatsapp.com",
    "wise.com",
    "zoom.us",
];

/// Registrable domains of link shorteners — the real destination is hidden behind them.
const SHORTENER_HOSTS: &[&str] = &[
    "adf.ly",
    "bit.ly",
    "bl.ink",
    "buff.ly",
    "clck.ru",
    "cutt.ly",
    "goo.gl",
    "is.gd",
    "j.mp",
    "lnkd.in",
    "ow.ly",
    "rb.gy",
    "rebrand.ly",
    "s.id",
    "shorturl.at",
    "t.co",
    "t.ly",
    "tiny.cc",
    "tinyurl.com",
    "u.to",
    "urlz.fr",
    "v.gd",
    "x.co",
];

/// TLDs disproportionately used by throwaway phishing hosts, plus typo-squats of common TLDs.
const SUSPICIOUS_TLDS: &[&str] = &[
    "bar", "buzz", "cam", "cf", "click", "cm", "country", "cyou", "ga", "gdn", "gq", "icu", "kim",
    "link", "loan", "men", "ml", "monster", "mov", "om", "party", "quest", "rest", "review", "sbs",
    "surf", "tk", "top", "work", "xyz", "zip",
];

/// Hosts that wrap a real destination inside a query parameter.
const WRAPPER_HOST_MARKERS: &[&str] = &[
    "clicktime.symantec.com",
    "linkprotect.cudasvc.com",
    "protect-us.mimecast.com",
    "protect.mimecast.com",
    "safelinks.protection.outlook.com",
    "urldefense.com",
    "urldefense.proofpoint.com",
];

/// Query parameters that commonly carry a wrapped destination URL.
const REDIRECT_PARAMS: &[&str] = &[
    "dest",
    "destination",
    "goto",
    "next",
    "out",
    "q",
    "r",
    "redirect",
    "redirect_url",
    "return",
    "rurl",
    "target",
    "u",
    "url",
];

/// Words that show up in hosts/paths built to fake a login or verification prompt.
const CREDENTIAL_KEYWORDS: &[&str] = &[
    "account", "billing", "confirm", "invoice", "login", "password", "recover", "secure", "signin",
    "suspend", "unlock", "update", "verify", "wallet",
];

/// Multi-part public suffixes we recognise, so `example.co.uk` is one registrable domain.
const MULTI_TLDS: &[&str] = &[
    "co.jp", "co.kr", "co.nz", "co.uk", "co.za", "com.au", "com.br", "com.mx", "com.sg", "com.tr",
    "gov.uk", "net.au", "org.uk",
];

/// Schemes that should never appear behind a link in an email.
const DANGEROUS_SCHEMES: &[&str] = &["data", "file", "javascript", "vbscript"];

// ---------------------------------------------------------------------------------------------
// Severity / findings
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    /// Points this severity contributes to a link's 0-100 score.
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

    /// Sort rank — lower sorts first (High before Info).
    fn rank(self) -> u8 {
        match self {
            Severity::High => 0,
            Severity::Medium => 1,
            Severity::Low => 2,
            Severity::Info => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: &'static str,
    pub check: String,
    pub message: String,
    #[serde(skip)]
    sev: SeverityRepr,
}

/// Serde-invisible copy of the severity, kept for sorting/scoring.
#[derive(Debug, Clone, Copy)]
struct SeverityRepr(Severity);

impl Default for SeverityRepr {
    fn default() -> Self {
        SeverityRepr(Severity::Info)
    }
}

fn finding(sev: Severity, check: &str, message: String) -> Finding {
    Finding {
        severity: sev.label(),
        check: check.to_string(),
        message,
        sev: SeverityRepr(sev),
    }
}

/// 0-100 score → rating band. Same bands as the sibling URL/header inspectors.
fn rating_for(score: u32) -> &'static str {
    match score {
        0 => "MINIMAL",
        1..=19 => "LOW",
        20..=44 => "MEDIUM",
        45..=69 => "HIGH",
        _ => "CRITICAL",
    }
}

// ---------------------------------------------------------------------------------------------
// URL parsing (dependency-free; only the structure is inspected, nothing is fetched)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct ParsedUrl {
    scheme: String,
    userinfo: Option<String>,
    host: String,
    port: Option<u16>,
    tail: String,
    ipv6_literal: bool,
}

fn parse_url(raw: &str) -> ParsedUrl {
    let s = raw.trim();
    let mut p = ParsedUrl::default();

    // Split the scheme. `://` marks a hierarchical URL; a bare `scheme:` marks an opaque one
    // (javascript:, data:, mailto:) which has no authority at all.
    let rest = if let Some(i) = s.find("://") {
        let sc = &s[..i];
        if !sc.is_empty() && sc.chars().all(is_scheme_char) {
            p.scheme = sc.to_ascii_lowercase();
            &s[i + 3..]
        } else {
            s
        }
    } else if let Some(i) = s.find(':') {
        let sc = &s[..i];
        if !sc.is_empty() && sc.chars().all(is_scheme_char) {
            p.scheme = sc.to_ascii_lowercase();
            p.tail = s[i + 1..].to_string();
            return p;
        } else {
            s
        }
    } else {
        s
    };

    let cut = rest
        .find(['/', '?', '#'])
        .unwrap_or(rest.len());
    let authority = &rest[..cut];
    p.tail = rest[cut..].to_string();

    let hostport = match authority.rfind('@') {
        Some(i) => {
            p.userinfo = Some(authority[..i].to_string());
            &authority[i + 1..]
        }
        None => authority,
    };

    if let Some(end) = hostport.strip_prefix('[').and_then(|r| r.find(']')) {
        p.ipv6_literal = true;
        p.host = hostport[1..=end].to_ascii_lowercase();
        if let Some(rest) = hostport[end + 2..].strip_prefix(':') {
            p.port = rest.parse().ok();
        }
    } else if let Some((h, port)) = hostport.rsplit_once(':') {
        if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() {
            p.host = h.to_ascii_lowercase();
            p.port = port.parse().ok();
        } else {
            p.host = hostport.to_ascii_lowercase();
        }
    } else {
        p.host = hostport.to_ascii_lowercase();
    }

    p.host = p.host.trim_end_matches('.').to_string();
    p
}

fn is_scheme_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.'
}

fn is_ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.chars().all(|c| c.is_ascii_digit())
                && p.parse::<u16>().map(|n| n <= 255).unwrap_or(false)
        })
}

/// The registrable domain (`a.b.example.co.uk` → `example.co.uk`).
fn registrable(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    if labels.len() >= 3 {
        let last_two = format!("{}.{}", labels[labels.len() - 2], labels[labels.len() - 1]);
        if MULTI_TLDS.contains(&last_two.as_str()) {
            return format!("{}.{}", labels[labels.len() - 3], last_two);
        }
    }
    if labels.len() >= 2 {
        format!("{}.{}", labels[labels.len() - 2], labels[labels.len() - 1])
    } else {
        host.to_string()
    }
}

/// The registrable domain's name and suffix (`example.co.uk` → `("example", "co.uk")`).
fn split_registrable(reg: &str) -> (String, String) {
    match reg.split_once('.') {
        Some((name, tld)) => (name.to_string(), tld.to_string()),
        None => (reg.to_string(), String::new()),
    }
}

// ---------------------------------------------------------------------------------------------
// Percent-decoding + punycode (both needed before a host can be compared to a brand)
// ---------------------------------------------------------------------------------------------

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

const PUNY_BASE: u32 = 36;
const PUNY_TMIN: u32 = 1;
const PUNY_TMAX: u32 = 26;
const PUNY_SKEW: u32 = 38;
const PUNY_DAMP: u32 = 700;

fn puny_adapt(mut delta: u32, numpoints: u32, first: bool) -> u32 {
    delta = if first { delta / PUNY_DAMP } else { delta / 2 };
    delta += delta / numpoints;
    let mut k = 0;
    while delta > ((PUNY_BASE - PUNY_TMIN) * PUNY_TMAX) / 2 {
        delta /= PUNY_BASE - PUNY_TMIN;
        k += PUNY_BASE;
    }
    k + (((PUNY_BASE - PUNY_TMIN + 1) * delta) / (delta + PUNY_SKEW))
}

/// Decode the payload of an `xn--` label (RFC 3492). Returns None on malformed input.
fn punycode_decode(input: &str) -> Option<String> {
    let (basic, encoded) = match input.rfind('-') {
        Some(i) => (&input[..i], &input[i + 1..]),
        None => ("", input),
    };
    let mut output: Vec<char> = Vec::new();
    for c in basic.chars() {
        if !c.is_ascii() {
            return None;
        }
        output.push(c);
    }
    let mut n: u32 = 128;
    let mut i: u32 = 0;
    let mut bias: u32 = 72;
    let mut chars = encoded.chars().peekable();
    while chars.peek().is_some() {
        let oldi = i;
        let mut w: u32 = 1;
        let mut k = PUNY_BASE;
        loop {
            let c = chars.next()?;
            let digit = match c {
                'a'..='z' => c as u32 - 'a' as u32,
                'A'..='Z' => c as u32 - 'A' as u32,
                '0'..='9' => c as u32 - '0' as u32 + 26,
                _ => return None,
            };
            i = i.checked_add(digit.checked_mul(w)?)?;
            let t = if k <= bias {
                PUNY_TMIN
            } else if k >= bias + PUNY_TMAX {
                PUNY_TMAX
            } else {
                k - bias
            };
            if digit < t {
                break;
            }
            w = w.checked_mul(PUNY_BASE - t)?;
            k += PUNY_BASE;
        }
        let len = output.len() as u32 + 1;
        bias = puny_adapt(i - oldi, len, oldi == 0);
        n = n.checked_add(i / len)?;
        i %= len;
        let ch = char::from_u32(n)?;
        if i as usize > output.len() {
            return None;
        }
        output.insert(i as usize, ch);
        i += 1;
    }
    Some(output.into_iter().collect())
}

/// Decode every `xn--` label in a host so homoglyph comparison sees the real characters.
fn unicode_host(host: &str) -> String {
    host.split('.')
        .map(|label| match label.strip_prefix("xn--") {
            Some(payload) => punycode_decode(payload).unwrap_or_else(|| label.to_string()),
            None => label.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

// ---------------------------------------------------------------------------------------------
// Confusable folding + edit distance
// ---------------------------------------------------------------------------------------------

/// Fold a domain name to its "skeleton": lowercase Latin with digits, common Cyrillic/Greek
/// lookalikes and multi-character tricks mapped onto the letter they imitate, and separators
/// dropped. `paypa1`, `pаypal` (Cyrillic а) and `pay-pal` all fold to `paypal`.
fn skeleton(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    for c in name.to_lowercase().chars() {
        let mapped = match c {
            '0' | 'о' | 'ο' | 'ө' | 'σ' => 'o',
            '1' | 'l' | 'ӏ' | 'ι' | '|' => 'l',
            '3' | 'е' | 'ε' | 'э' => 'e',
            '4' | '@' | 'а' | 'α' => 'a',
            '5' | '$' | 'ѕ' => 's',
            '7' => 't',
            '8' => 'b',
            '9' => 'g',
            '2' => 'z',
            '6' => 'g',
            'с' => 'c',
            'р' | 'ρ' => 'p',
            'у' | 'υ' => 'y',
            'х' | 'χ' => 'x',
            'і' | 'ї' => 'i',
            'ԁ' => 'd',
            'һ' => 'h',
            'ԛ' => 'q',
            'ԝ' => 'w',
            'п' => 'n',
            'т' | 'τ' => 't',
            'κ' => 'k',
            'ν' => 'v',
            'β' => 'b',
            'μ' => 'u',
            '-' | '_' | '.' | ' ' => continue,
            other => other,
        };
        s.push(mapped);
    }
    // Multi-character imitations, applied after the per-character fold.
    s = s.replace("rn", "m").replace("vv", "w").replace("cl", "d");
    s
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

// ---------------------------------------------------------------------------------------------
// Brands
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Brand {
    registrable: String,
    name: String,
    skeleton: String,
}

fn build_brands(extra: &str, sender_domain: Option<&str>) -> Vec<Brand> {
    let mut out: Vec<Brand> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let push = |raw: &str, out: &mut Vec<Brand>, seen: &mut Vec<String>| {
        let cleaned = raw
            .trim()
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or("")
            .trim_start_matches("www.")
            .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-')
            .to_ascii_lowercase();
        if cleaned.is_empty() || !cleaned.contains('.') {
            return;
        }
        let reg = registrable(&cleaned);
        if seen.contains(&reg) {
            return;
        }
        let (name, _) = split_registrable(&reg);
        seen.push(reg.clone());
        out.push(Brand {
            skeleton: skeleton(&name),
            registrable: reg,
            name,
        });
    };
    for b in BRAND_DOMAINS {
        push(b, &mut out, &mut seen);
    }
    for token in extra.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
        push(token, &mut out, &mut seen);
    }
    if let Some(s) = sender_domain {
        push(s, &mut out, &mut seen);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Message parsing + link extraction
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyKind {
    Html,
    Text,
}

struct Message {
    has_headers: bool,
    body: String,
    body_kind: BodyKind,
    sender_domain: Option<String>,
}

fn looks_like_header_block(input: &str) -> bool {
    let mut saw_header = false;
    for line in input.lines() {
        if line.trim().is_empty() {
            return saw_header;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        match line.split_once(':') {
            Some((name, _))
                if !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-') =>
            {
                saw_header = true;
            }
            _ => return false,
        }
    }
    false
}

fn looks_like_html(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    ["<a ", "<a\n", "<a\t", "<html", "<body", "<div", "<table", "<p>", "<br"]
        .iter()
        .any(|m| lower.contains(m))
}

fn header_value(headers: &str, want: &str) -> Option<String> {
    let want = want.to_ascii_lowercase();
    let mut current: Option<String> = None;
    for line in headers.lines() {
        if (line.starts_with(' ') || line.starts_with('\t')) && current.is_some() {
            let cur = current.as_mut().unwrap();
            cur.push(' ');
            cur.push_str(line.trim());
            continue;
        }
        if let Some(v) = current.take() {
            return Some(v);
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().to_ascii_lowercase() == want {
                current = Some(value.trim().to_string());
            }
        }
    }
    current
}

/// Pull the domain out of a `From:` header value (`"Name" <a@b.example>` → `b.example`).
fn address_domain(value: &str) -> Option<String> {
    let candidate = match (value.rfind('<'), value.rfind('>')) {
        (Some(a), Some(b)) if b > a => value[a + 1..b].to_string(),
        _ => value.to_string(),
    };
    let at = candidate.rfind('@')?;
    let dom: String = candidate[at + 1..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
        .collect();
    let dom = dom.trim_matches('.').to_ascii_lowercase();
    if dom.contains('.') {
        Some(dom)
    } else {
        None
    }
}

fn parse_message(input: &str, format: &str) -> Message {
    let normalized = input.replace("\r\n", "\n");
    let treat_as_raw = match format {
        "raw" => true,
        "html" | "text" => false,
        _ => looks_like_header_block(&normalized),
    };

    let (headers, body) = if treat_as_raw {
        match normalized.find("\n\n") {
            Some(i) => (&normalized[..i], normalized[i + 2..].to_string()),
            None => (&normalized[..], String::new()),
        }
    } else {
        ("", normalized.clone())
    };

    let body_kind = match format {
        "html" => BodyKind::Html,
        "text" => BodyKind::Text,
        _ => {
            if looks_like_html(&body) {
                BodyKind::Html
            } else {
                BodyKind::Text
            }
        }
    };

    Message {
        has_headers: treat_as_raw && !headers.is_empty(),
        sender_domain: header_value(headers, "from").and_then(|v| address_domain(&v)),
        body,
        body_kind,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum LinkSource {
    Anchor,
    Text,
}

#[derive(Debug, Clone)]
struct RawLink {
    url: String,
    display: String,
    source: LinkSource,
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

/// Strip inner markup from an anchor's inner HTML and collapse whitespace.
fn anchor_text(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut in_tag = false;
    for c in inner.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn attr_value(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(attr) {
        let at = from + rel;
        // Must be preceded by whitespace so `data-href` doesn't match `href`.
        let ok_before = at == 0
            || lower[..at]
                .chars()
                .next_back()
                .map(|c| c.is_whitespace())
                .unwrap_or(false);
        let rest = &tag[at + attr.len()..];
        let trimmed = rest.trim_start();
        if ok_before && trimmed.starts_with('=') {
            let v = trimmed[1..].trim_start();
            let value = if let Some(stripped) = v.strip_prefix('"') {
                stripped.split('"').next().unwrap_or("").to_string()
            } else if let Some(stripped) = v.strip_prefix('\'') {
                stripped.split('\'').next().unwrap_or("").to_string()
            } else {
                v.split(|c: char| c.is_whitespace() || c == '>')
                    .next()
                    .unwrap_or("")
                    .to_string()
            };
            return Some(decode_entities(value.trim()));
        }
        from = at + attr.len();
    }
    None
}

/// Extract `<a href>` links plus the byte span each anchor occupies.
fn extract_anchors(html: &str) -> (Vec<RawLink>, Vec<(usize, usize)>) {
    let lower = html.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut links = Vec::new();
    let mut spans = Vec::new();
    let mut i = 0;
    while let Some(rel) = lower[i..].find("<a") {
        let start = i + rel;
        let after = start + 2;
        if after >= bytes.len() || !(bytes[after] as char).is_whitespace() && bytes[after] != b'>' {
            i = after;
            continue;
        }
        let gt = match lower[start..].find('>') {
            Some(g) => start + g,
            None => break,
        };
        let tag = &html[start..=gt];
        let close_rel = lower[gt + 1..].find("</a");
        let (inner, end) = match close_rel {
            Some(c) => {
                let close = gt + 1 + c;
                let close_end = lower[close..]
                    .find('>')
                    .map(|g| close + g + 1)
                    .unwrap_or(lower.len());
                (&html[gt + 1..close], close_end)
            }
            None => ("", gt + 1),
        };
        if let Some(href) = attr_value(tag, "href") {
            if !href.trim().is_empty() && !href.trim().starts_with('#') {
                links.push(RawLink {
                    url: href.trim().to_string(),
                    display: anchor_text(inner),
                    source: LinkSource::Anchor,
                });
            }
        }
        spans.push((start, end));
        i = end;
    }
    (links, spans)
}

/// Extract bare `http(s)://…` URLs from text, trimming prose punctuation.
fn extract_bare_urls(text: &str) -> Vec<RawLink> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lower.len() {
        let next = match lower[i..].find("http") {
            Some(r) => i + r,
            None => break,
        };
        let rest = &lower[next..];
        if !(rest.starts_with("http://") || rest.starts_with("https://")) {
            i = next + 4;
            continue;
        }
        let end = text[next..]
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '`' | '|' | '\\')
            })
            .map(|r| next + r)
            .unwrap_or(text.len());
        let raw = text[next..end].trim_end_matches(|c: char| {
            matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '\u{2019}')
        });
        if !raw.is_empty() {
            out.push(RawLink {
                url: raw.to_string(),
                display: String::new(),
                source: LinkSource::Text,
            });
        }
        i = end.max(next + 1);
    }
    out
}

fn collect_links(msg: &Message) -> Vec<RawLink> {
    let mut links = Vec::new();
    let mut masked = msg.body.clone();
    if msg.body_kind == BodyKind::Html {
        let (anchors, spans) = extract_anchors(&msg.body);
        links.extend(anchors);
        // Blank out anchor spans so their hrefs aren't re-found as bare URLs.
        let mut buf = String::with_capacity(msg.body.len());
        let mut cursor = 0;
        for (s, e) in spans {
            if s < cursor || e > msg.body.len() {
                continue;
            }
            buf.push_str(&msg.body[cursor..s]);
            buf.push_str(&" ".repeat(e - s));
            cursor = e;
        }
        buf.push_str(&msg.body[cursor..]);
        masked = buf;
    }
    links.extend(extract_bare_urls(&masked));

    // Collapse exact (target, display) repeats — a template that repeats one link 40 times
    // should not produce 40 identical rows.
    let mut seen: Vec<(String, String)> = Vec::new();
    links.retain(|l| {
        let key = (l.url.clone(), l.display.clone());
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    links
}

// ---------------------------------------------------------------------------------------------
// Redirect wrappers
// ---------------------------------------------------------------------------------------------

/// If the URL wraps another http(s) URL in a query parameter, return (param, destination).
fn unwrap_redirect(url: &str) -> Option<(String, String)> {
    let query = url.split_once('?')?.1;
    let query = query.split('#').next().unwrap_or(query);
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        let key = k.to_ascii_lowercase();
        if !REDIRECT_PARAMS.contains(&key.as_str()) {
            continue;
        }
        let decoded = percent_decode(v);
        let lower = decoded.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Some((key, decoded));
        }
    }
    None
}

fn wrapper_host(host: &str) -> Option<&'static str> {
    WRAPPER_HOST_MARKERS
        .iter()
        .copied()
        .find(|m| host == *m || host.ends_with(&format!(".{m}")))
}

// ---------------------------------------------------------------------------------------------
// Display-text vs target
// ---------------------------------------------------------------------------------------------

/// If the visible link text names a domain, return its registrable form.
fn domain_in_display(display: &str) -> Option<String> {
    for token in display.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '<' | '>' | ',' | '"')) {
        let t = token.trim_matches(|c: char| matches!(c, '.' | ':' | ';' | '!' | '?' | '\'' | '“' | '”'));
        if t.is_empty() || t.contains('@') {
            continue;
        }
        let p = parse_url(t);
        let host = if p.host.is_empty() { t } else { &p.host };
        let host = host.trim_start_matches("www.").to_ascii_lowercase();
        if !host.contains('.') || host.starts_with('.') || host.ends_with('.') {
            continue;
        }
        let tld = host.rsplit('.').next().unwrap_or("");
        if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
            continue;
        }
        if host.split('.').any(|l| l.is_empty()) {
            continue;
        }
        return Some(registrable(&host));
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Per-link analysis
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ScannedLink {
    pub index: usize,
    pub url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub display_text: String,
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unwrapped_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    pub score: u32,
    pub rating: &'static str,
    pub findings: Vec<Finding>,
}

/// Compare a host against the brand list and return at most one lookalike finding.
fn lookalike_finding(host: &str, brands: &[Brand]) -> Option<Finding> {
    if host.is_empty() || is_ipv4(host) {
        return None;
    }
    let decoded = unicode_host(host);
    let reg = registrable(&decoded);
    let (name, tld) = split_registrable(&reg);
    let name_skel = skeleton(&name);

    // An exact brand match wins outright — a real brand link is never a lookalike.
    if brands.iter().any(|b| b.registrable == reg) {
        return None;
    }

    let sub_labels: Vec<String> = {
        let host_labels: Vec<&str> = decoded.split('.').collect();
        let reg_labels = reg.split('.').count();
        host_labels[..host_labels.len().saturating_sub(reg_labels)]
            .iter()
            .flat_map(|l| l.split('-'))
            .map(|l| skeleton(l))
            .collect()
    };
    let name_segments: Vec<String> = name.split('-').map(skeleton).collect();

    // Ordered best-first; the first rule that fires is the one reported.
    for b in brands {
        if b.name.chars().count() < 4 {
            continue;
        }
        // 1. Homoglyph / digit-swap: folds to the same skeleton but isn't the same string.
        if name_skel == b.skeleton {
            let via = if decoded != host {
                format!(" (punycode {host} decodes to {decoded})")
            } else {
                String::new()
            };
            return Some(finding(
                Severity::High,
                "lookalike-domain",
                format!(
                    "'{reg}' is not '{}' but folds to the same name once lookalike characters are normalised{via} — a homoglyph/typo impersonation.",
                    b.registrable
                ),
            ));
        }
        // 2. The brand name sits in a subdomain while the real domain is someone else's.
        if sub_labels.iter().any(|l| *l == b.skeleton) {
            return Some(finding(
                Severity::High,
                "brand-in-subdomain",
                format!(
                    "'{}' appears in the subdomain, but the domain the browser actually connects to is '{reg}'.",
                    b.name
                ),
            ));
        }
        // 3. Combosquat: brand name as a whole hyphen-separated segment of the domain.
        if name_segments.len() > 1 && name_segments.iter().any(|s| *s == b.skeleton) {
            return Some(finding(
                Severity::High,
                "combosquat-domain",
                format!(
                    "'{reg}' bolts extra words onto the brand name '{}' — a combosquat domain unrelated to '{}'.",
                    b.name, b.registrable
                ),
            ));
        }
    }
    for b in brands {
        let blen = b.skeleton.chars().count();
        if blen < 4 || name_skel.chars().count() < 3 {
            continue;
        }
        // 4. Edit distance on the folded names.
        let allowed = if blen >= 9 { 2 } else { 1 };
        if levenshtein(&name_skel, &b.skeleton) <= allowed {
            return Some(finding(
                Severity::High,
                "typosquat-domain",
                format!(
                    "'{reg}' is one small edit away from '{}' — a typosquat of that brand.",
                    b.registrable
                ),
            ));
        }
    }
    for b in brands {
        // 5. Same brand name, different TLD.
        if !tld.is_empty() && skeleton(&name) == b.skeleton && reg != b.registrable {
            return Some(finding(
                Severity::Medium,
                "brand-different-tld",
                format!(
                    "'{reg}' uses the brand name '{}' under a different suffix (.{tld}) than '{}'. Legitimate country sites do this too — check the suffix.",
                    b.name, b.registrable
                ),
            ));
        }
    }
    None
}

fn analyze_link(index: usize, raw: &RawLink, brands: &[Brand]) -> ScannedLink {
    let mut findings: Vec<Finding> = Vec::new();
    let parsed = parse_url(&raw.url);

    // Opaque, non-navigational schemes never belong behind an email link.
    if DANGEROUS_SCHEMES.contains(&parsed.scheme.as_str()) {
        findings.push(finding(
            Severity::High,
            "dangerous-scheme",
            format!(
                "Link uses the '{}:' scheme — it runs or embeds content instead of opening a web page.",
                parsed.scheme
            ),
        ));
        let score = findings.iter().map(|f| f.sev.0.weight()).sum::<u32>().min(100);
        return ScannedLink {
            index,
            url: raw.url.clone(),
            display_text: raw.display.clone(),
            source: match raw.source {
                LinkSource::Anchor => "anchor",
                LinkSource::Text => "text",
            },
            unwrapped_target: None,
            host: None,
            score,
            rating: rating_for(score),
            findings,
        };
    }

    // A wrapped destination is what the recipient really lands on, so structural checks run
    // against it. Only one level is unwrapped (see the page's Limits section).
    let mut effective_url = raw.url.clone();
    let mut unwrapped: Option<String> = None;
    if let Some(marker) = wrapper_host(&parsed.host) {
        match unwrap_redirect(&raw.url) {
            Some((param, dest)) => {
                findings.push(finding(
                    Severity::Medium,
                    "redirect-wrapper",
                    format!(
                        "Link is a '{marker}' redirect wrapper; the real destination sits in its '{param}' parameter: {dest}"
                    ),
                ));
                effective_url = dest.clone();
                unwrapped = Some(dest);
            }
            None => findings.push(finding(
                Severity::Medium,
                "redirect-wrapper",
                format!(
                    "Link goes through the '{marker}' redirect wrapper, so the real destination is not visible in the URL."
                ),
            )),
        }
    } else if let Some((param, dest)) = unwrap_redirect(&raw.url) {
        findings.push(finding(
            Severity::Medium,
            "redirect-wrapper",
            format!(
                "Link is an open redirect through '{}': the '{param}' parameter carries the real destination {dest}",
                parsed.host
            ),
        ));
        effective_url = dest.clone();
        unwrapped = Some(dest);
    }

    let p = if unwrapped.is_some() {
        parse_url(&effective_url)
    } else {
        parsed
    };
    let host = p.host.clone();
    let host_is_ip = is_ipv4(&host);
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();

    // Display text vs the real target.
    if !raw.display.is_empty() && !host.is_empty() {
        if let Some(shown) = domain_in_display(&raw.display) {
            let actual = registrable(&host);
            if shown != actual {
                findings.push(finding(
                    Severity::High,
                    "display-target-mismatch",
                    format!(
                        "Link text shows '{shown}' but the link actually goes to '{}'.",
                        if actual.is_empty() { host.clone() } else { actual }
                    ),
                ));
            }
        }
    }

    if host_is_ip || p.ipv6_literal {
        findings.push(finding(
            Severity::High,
            "ip-literal-host",
            format!("Target host is a bare IP address ({host}), not a registered domain name."),
        ));
    }

    if let Some(ui) = &p.userinfo {
        findings.push(finding(
            Severity::High,
            "userinfo-authority",
            format!(
                "Authority contains '@': the browser connects to '{host}' and ignores the '{ui}' shown before the '@'."
            ),
        ));
    }

    if !host_is_ip {
        if let Some(f) = lookalike_finding(&host, brands) {
            findings.push(f);
        }
        if let Some(l) = labels.iter().find(|l| l.starts_with("xn--")) {
            let decoded = unicode_host(&host);
            findings.push(finding(
                Severity::Medium,
                "punycode-label",
                format!(
                    "Host contains the punycode label '{l}' — it renders as '{decoded}', which can imitate a familiar name."
                ),
            ));
        }
        if host.contains('%') {
            findings.push(finding(
                Severity::Medium,
                "percent-encoded-host",
                format!("Host '{host}' is percent-encoded — encoding a hostname hides its real characters."),
            ));
        }
        let reg = registrable(&host);
        if SHORTENER_HOSTS.contains(&reg.as_str()) {
            findings.push(finding(
                Severity::Medium,
                "url-shortener",
                format!("'{reg}' is a link shortener — the real destination is hidden until the link is followed."),
            ));
        }
        if let Some(tld) = labels.last() {
            if SUSPICIOUS_TLDS.contains(tld) {
                findings.push(finding(
                    Severity::Medium,
                    "suspicious-tld",
                    format!(".{tld} is a free or heavily abused suffix, or a typo of a common one."),
                ));
            }
        }
        if labels.len() >= 5 {
            findings.push(finding(
                Severity::Low,
                "deep-subdomains",
                format!(
                    "Host has {} labels ({host}) — deep nesting is used to bury a lookalike name in front of the real domain.",
                    labels.len()
                ),
            ));
        }
        let hyphens = host.matches('-').count();
        if hyphens >= 3 {
            findings.push(finding(
                Severity::Low,
                "hyphenated-host",
                format!("Host stacks {hyphens} hyphens ({host}) — typical of brand-impersonation domains."),
            ));
        }
    }

    if p.scheme == "http" {
        findings.push(finding(
            Severity::Low,
            "insecure-http",
            "Link uses plain http:// — the connection is unencrypted and the site proves no identity.".to_string(),
        ));
    }

    if let Some(port) = p.port {
        if port != 80 && port != 443 {
            findings.push(finding(
                Severity::Low,
                "non-standard-port",
                format!("Link specifies port {port} — normal web traffic uses 80 or 443."),
            ));
        }
    }

    let hay = format!("{} {}", host, p.tail).to_ascii_lowercase();
    let hits: Vec<&str> = CREDENTIAL_KEYWORDS
        .iter()
        .copied()
        .filter(|k| hay.contains(k))
        .collect();
    if !hits.is_empty() {
        findings.push(finding(
            Severity::Low,
            "credential-keywords",
            format!(
                "Contains credential/urgency words ({}) — common on pages that fake a login or verification prompt.",
                hits.join(", ")
            ),
        ));
    }

    if effective_url.chars().count() > 120 {
        findings.push(finding(
            Severity::Info,
            "excessive-length",
            format!(
                "Link is {} characters long — padding helps hide the real destination.",
                effective_url.chars().count()
            ),
        ));
    }

    if !host_is_ip && labels.len() >= 2 {
        let name: String = labels[..labels.len() - 1].concat();
        let alnum = name.chars().filter(|c| c.is_ascii_alphanumeric()).count();
        let digits = name.chars().filter(|c| c.is_ascii_digit()).count();
        if alnum >= 6 && digits * 100 >= alnum * 40 {
            findings.push(finding(
                Severity::Info,
                "digit-heavy-host",
                format!(
                    "Host name is {}% digits — a hallmark of throwaway, algorithmically generated domains.",
                    digits * 100 / alnum
                ),
            ));
        }
    }

    findings.sort_by(|a, b| {
        a.sev
            .0
            .rank()
            .cmp(&b.sev.0.rank())
            .then_with(|| a.check.cmp(&b.check))
    });
    let score = findings.iter().map(|f| f.sev.0.weight()).sum::<u32>().min(100);

    ScannedLink {
        index,
        url: raw.url.clone(),
        display_text: raw.display.clone(),
        source: match raw.source {
            LinkSource::Anchor => "anchor",
            LinkSource::Text => "text",
        },
        unwrapped_target: unwrapped,
        host: if host.is_empty() { None } else { Some(host) },
        score,
        rating: rating_for(score),
        findings,
    }
}

// ---------------------------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub rating: &'static str,
    pub score: u32,
    pub parsed_as: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_domain: Option<String>,
    pub links_found: usize,
    pub links_scanned: usize,
    pub links_flagged: usize,
    pub truncated: bool,
    pub links: Vec<ScannedLink>,
}

/// Scan an email and return the report as a structure (used by every surface).
pub fn scan(
    email: &str,
    brands: &str,
    format: &str,
    max_links: i64,
) -> Result<Report, String> {
    if email.trim().is_empty() {
        return Err("email is empty — paste the message (or just its links) to scan".into());
    }
    if email.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "message is {} bytes, over the {} byte limit",
            email.len(),
            MAX_INPUT_BYTES
        ));
    }
    if !matches!(format, "auto" | "raw" | "html" | "text") {
        return Err(format!(
            "unknown format '{format}' — use auto, raw, html, or text"
        ));
    }
    if max_links < 1 || max_links > MAX_LINKS_LIMIT {
        return Err(format!(
            "max_links must be between 1 and {MAX_LINKS_LIMIT}, got {max_links}"
        ));
    }

    let msg = parse_message(email, format);
    let brand_list = build_brands(brands, msg.sender_domain.as_deref());
    let raw_links = collect_links(&msg);
    let links_found = raw_links.len();
    let truncated = links_found > max_links as usize;

    let links: Vec<ScannedLink> = raw_links
        .iter()
        .take(max_links as usize)
        .enumerate()
        .map(|(i, l)| analyze_link(i + 1, l, &brand_list))
        .collect();

    let links_flagged = links.iter().filter(|l| !l.findings.is_empty()).count();
    let score = links.iter().map(|l| l.score).max().unwrap_or(0);

    let parsed_as = match (msg.has_headers, msg.body_kind) {
        (true, BodyKind::Html) => "raw email with an HTML body",
        (true, BodyKind::Text) => "raw email with a plain-text body",
        (false, BodyKind::Html) => "HTML body (no headers)",
        (false, BodyKind::Text) => "plain text (no headers)",
    }
    .to_string();

    Ok(Report {
        rating: rating_for(score),
        score,
        parsed_as,
        sender_domain: msg.sender_domain,
        links_found,
        links_scanned: links.len(),
        links_flagged,
        truncated,
        links,
    })
}

fn render_detailed(r: &Report, only_flagged: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Phishing link scan: {} (score {}/100)\n",
        r.rating, r.score
    ));
    out.push_str(&format!(
        "Links: {} scanned, {} flagged\n",
        r.links_scanned, r.links_flagged
    ));
    out.push_str(&format!("Input parsed as: {}\n", r.parsed_as));
    if let Some(s) = &r.sender_domain {
        out.push_str(&format!("Sender domain: {s}\n"));
    }
    if r.truncated {
        out.push_str(&format!(
            "Note: {} links found, only the first {} were scanned (raise max_links)\n",
            r.links_found, r.links_scanned
        ));
    }
    if r.links_scanned == 0 {
        out.push_str("\nNo links were found in this message.\n");
        return out;
    }

    let shown: Vec<&ScannedLink> = r
        .links
        .iter()
        .filter(|l| !only_flagged || !l.findings.is_empty())
        .collect();
    if shown.is_empty() {
        out.push_str("\nNo link raised a finding.\n");
        return out;
    }

    for l in shown {
        out.push_str(&format!(
            "\nLink {} — {} (score {}/100)\n",
            l.index, l.rating, l.score
        ));
        out.push_str(&format!("  Target:   {}\n", l.url));
        if !l.display_text.is_empty() {
            out.push_str(&format!("  Shown as: \"{}\"\n", l.display_text));
        }
        if let Some(t) = &l.unwrapped_target {
            out.push_str(&format!("  Unwraps to: {t}\n"));
        }
        if l.findings.is_empty() {
            out.push_str("  No findings.\n");
        }
        for f in &l.findings {
            out.push_str(&format!(
                "  [{}] {}: {}\n",
                f.severity, f.check, f.message
            ));
        }
    }
    out
}

fn render_summary(r: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Phishing link scan: {} (score {}/100)\n",
        r.rating, r.score
    ));
    out.push_str(&format!(
        "Links: {} scanned, {} flagged\n",
        r.links_scanned, r.links_flagged
    ));
    if r.links_flagged == 0 {
        out.push_str("\nNo link raised a finding.\n");
        return out;
    }
    out.push('\n');
    for l in r.links.iter().filter(|l| !l.findings.is_empty()) {
        let top = &l.findings[0];
        out.push_str(&format!(
            "{} ({}/100) {} — {}\n",
            l.rating,
            l.score,
            l.unwrapped_target.as_deref().unwrap_or(&l.url),
            top.message
        ));
    }
    out
}

/// Scan an email and render the report for the requested output style.
pub fn run(
    email: &str,
    brands: &str,
    format: &str,
    report: &str,
    only_flagged: bool,
    max_links: i64,
) -> Result<String, String> {
    if !matches!(report, "detailed" | "summary" | "json") {
        return Err(format!(
            "unknown report '{report}' — use detailed, summary, or json"
        ));
    }
    let mut r = scan(email, brands, format, max_links)?;
    match report {
        "summary" => Ok(render_summary(&r)),
        "json" => {
            if only_flagged {
                r.links.retain(|l| !l.findings.is_empty());
            }
            serde_json::to_string_pretty(&r).map_err(|e| e.to_string())
        }
        _ => Ok(render_detailed(&r, only_flagged)),
    }
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const PHISH: &str = "From: \"PayPal Security\" <alerts@paypa1-secure.com>\nTo: victim@example.com\nSubject: Urgent: verify your account\n\n<html><body>\n<p><a href=\"http://192.168.10.4/login\">https://www.paypal.com/signin</a></p>\n<p><a href=\"https://bit.ly/3abcXYZ\">Update billing</a></p>\n<p><a href=\"https://www.paypal.com/help\">Help centre</a></p>\n</body></html>";

    #[test]
    fn happy_path_flags_the_phishing_links_and_clears_the_real_one() {
        let r = scan(PHISH, "", "auto", 200).unwrap();
        assert_eq!(r.links_scanned, 3, "three anchors");
        assert_eq!(r.parsed_as, "raw email with an HTML body");
        assert_eq!(r.sender_domain.as_deref(), Some("paypa1-secure.com"));

        let checks: Vec<&str> = r.links[0].findings.iter().map(|f| f.check.as_str()).collect();
        assert!(checks.contains(&"display-target-mismatch"), "{checks:?}");
        assert!(checks.contains(&"ip-literal-host"), "{checks:?}");
        assert_eq!(r.links[0].rating, "CRITICAL");

        let checks: Vec<&str> = r.links[1].findings.iter().map(|f| f.check.as_str()).collect();
        assert!(checks.contains(&"url-shortener"), "{checks:?}");

        // The genuine paypal.com link must stay clean.
        assert!(r.links[2].findings.is_empty(), "{:?}", r.links[2].findings);
        assert_eq!(r.links[2].rating, "MINIMAL");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = scan("   \n ", "", "auto", 200).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn bad_format_and_bad_report_and_bad_cap_are_errors() {
        assert!(scan("http://x.example", "", "xml", 200)
            .unwrap_err()
            .contains("unknown format"));
        assert!(run("http://x.example", "", "auto", "csv", false, 200)
            .unwrap_err()
            .contains("unknown report"));
        assert!(scan("http://x.example", "", "auto", 0)
            .unwrap_err()
            .contains("max_links"));
        assert!(scan("http://x.example", "", "auto", MAX_LINKS_LIMIT + 1)
            .unwrap_err()
            .contains("max_links"));
    }

    #[test]
    fn oversized_input_is_rejected_at_the_boundary() {
        let filler = "x".repeat(MAX_INPUT_BYTES);
        assert!(scan(&filler, "", "text", 10).is_ok(), "exactly at the cap is fine");
        let over = "x".repeat(MAX_INPUT_BYTES + 1);
        assert!(scan(&over, "", "text", 10).unwrap_err().contains("over the"));
    }

    #[test]
    fn typosquat_homoglyph_and_combosquat_domains_are_caught() {
        // digit-for-letter
        let r = scan("https://paypa1.com/verify", "", "text", 200).unwrap();
        assert_eq!(r.links[0].findings[0].check, "lookalike-domain");
        // one-edit typo
        let r = scan("https://micosoft.com/login", "", "text", 200).unwrap();
        assert!(r.links[0]
            .findings
            .iter()
            .any(|f| f.check == "typosquat-domain"));
        // brand bolted onto other words
        let r = scan("https://paypal-secure-billing.com/x", "", "text", 200).unwrap();
        assert!(r.links[0]
            .findings
            .iter()
            .any(|f| f.check == "combosquat-domain"));
        // brand pushed into a subdomain of somebody else's domain
        let r = scan("https://apple.com.account-check.tk/", "", "text", 200).unwrap();
        assert!(r.links[0]
            .findings
            .iter()
            .any(|f| f.check == "brand-in-subdomain"));
    }

    #[test]
    fn punycode_host_decodes_and_matches_the_brand_it_imitates() {
        // xn--pypal-4ve.com decodes to a Cyrillic-а spelling of paypal.com.
        let decoded = unicode_host("xn--pypal-4ve.com");
        assert_ne!(decoded, "xn--pypal-4ve.com", "punycode must decode");
        let r = scan("https://xn--pypal-4ve.com/verify", "", "text", 200).unwrap();
        let checks: Vec<&str> = r.links[0].findings.iter().map(|f| f.check.as_str()).collect();
        assert!(checks.contains(&"punycode-label"), "{checks:?}");
        assert!(checks.contains(&"lookalike-domain"), "{checks:?}");
    }

    #[test]
    fn custom_brands_extend_the_lookalike_check() {
        let clean = scan("https://acmecorp-login.example/", "", "text", 200).unwrap();
        assert!(
            !clean.links[0]
                .findings
                .iter()
                .any(|f| f.check.contains("squat")),
            "unknown brand is not flagged by default"
        );
        let with_brand =
            scan("https://acmecorp-login.example/", "acmecorp.com", "text", 200).unwrap();
        assert!(with_brand.links[0]
            .findings
            .iter()
            .any(|f| f.check == "combosquat-domain"));
    }

    #[test]
    fn redirect_wrappers_are_unwrapped_and_the_destination_is_scanned() {
        let r = scan(
            "https://eu.safelinks.protection.outlook.com/?url=http%3A%2F%2F203.0.113.9%2Fsignin&data=x",
            "",
            "text",
            200,
        )
        .unwrap();
        assert_eq!(
            r.links[0].unwrapped_target.as_deref(),
            Some("http://203.0.113.9/signin")
        );
        let checks: Vec<&str> = r.links[0].findings.iter().map(|f| f.check.as_str()).collect();
        assert!(checks.contains(&"redirect-wrapper"), "{checks:?}");
        assert!(checks.contains(&"ip-literal-host"), "{checks:?}");
    }

    #[test]
    fn dangerous_schemes_and_userinfo_tricks_are_flagged() {
        let r = scan("<a href=\"javascript:steal()\">Click</a>", "", "html", 200).unwrap();
        assert_eq!(r.links[0].findings[0].check, "dangerous-scheme");

        let r = scan("http://paypal.com@203.0.113.5/login", "", "text", 200).unwrap();
        let checks: Vec<&str> = r.links[0].findings.iter().map(|f| f.check.as_str()).collect();
        assert!(checks.contains(&"userinfo-authority"), "{checks:?}");
        assert!(checks.contains(&"ip-literal-host"), "{checks:?}");
    }

    #[test]
    fn plain_text_bare_urls_are_extracted_and_deduped() {
        let r = scan(
            "Hi,\nsee https://example.com/a and https://example.com/a again, plus https://example.com/b.",
            "",
            "text",
            200,
        )
        .unwrap();
        assert_eq!(r.links_scanned, 2, "{:?}", r.links);
        assert_eq!(r.links[0].url, "https://example.com/a");
        assert_eq!(r.links[1].url, "https://example.com/b");
    }

    #[test]
    fn anchor_hrefs_are_not_double_counted_as_bare_urls() {
        let r = scan(
            "<p><a href=\"https://example.com/a\">Read <b>more</b></a> or https://example.com/b</p>",
            "",
            "html",
            200,
        )
        .unwrap();
        assert_eq!(r.links_scanned, 2);
        assert_eq!(r.links[0].display_text, "Read more");
        assert_eq!(r.links[1].source, "text");
    }

    #[test]
    fn max_links_truncates_and_says_so() {
        let body = (0..5)
            .map(|i| format!("https://example{i}.com/x"))
            .collect::<Vec<_>>()
            .join(" ");
        let r = scan(&body, "", "text", 3).unwrap();
        assert_eq!(r.links_found, 5);
        assert_eq!(r.links_scanned, 3);
        assert!(r.truncated);
        assert!(render_detailed(&r, false).contains("only the first 3 were scanned"));
    }

    #[test]
    fn only_flagged_hides_clean_links_in_every_report_style() {
        let out = run(PHISH, "", "auto", "detailed", true, 200).unwrap();
        assert!(!out.contains("https://www.paypal.com/help"), "{out}");
        assert!(out.contains("192.168.10.4"), "{out}");
        let json = run(PHISH, "", "auto", "json", true, 200).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["links"].as_array().unwrap().len(), 2);
        assert_eq!(v["links_flagged"], 2);
    }

    #[test]
    fn json_report_is_machine_readable() {
        let json = run(PHISH, "", "auto", "json", false, 200).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["rating"], "CRITICAL");
        assert_eq!(v["links_scanned"], 3);
        assert_eq!(v["links"][0]["findings"][0]["severity"], "high");
        assert_eq!(v["sender_domain"], "paypa1-secure.com");
    }

    #[test]
    fn summary_report_lists_only_flagged_links() {
        let out = run(PHISH, "", "auto", "summary", false, 200).unwrap();
        assert!(out.starts_with("Phishing link scan: CRITICAL"), "{out}");
        assert!(!out.contains("paypal.com/help"), "{out}");
    }

    #[test]
    fn a_clean_newsletter_scores_minimal() {
        let msg = "From: News <news@example.com>\nSubject: Weekly notes\n\n<p>Read the <a href=\"https://example.com/weekly\">weekly notes</a>.</p>";
        let r = scan(msg, "", "auto", 200).unwrap();
        assert_eq!(r.rating, "MINIMAL");
        assert_eq!(r.links_flagged, 0);
        assert!(render_summary(&r).contains("No link raised a finding."));
    }

    #[test]
    fn format_override_forces_plain_text_reading() {
        let msg = "From: a@b.example\nSubject: x\n\nhttps://example.com/1";
        assert!(scan(msg, "", "raw", 200).unwrap().sender_domain.is_some());
        // Read as text, the header block is just body copy — no sender, and the
        // address is not mistaken for a link.
        let as_text = scan(msg, "", "text", 200).unwrap();
        assert!(as_text.sender_domain.is_none());
        assert_eq!(as_text.links_scanned, 1);
    }

    #[test]
    fn levenshtein_and_skeleton_behave() {
        assert_eq!(levenshtein("paypal", "paypal"), 0);
        assert_eq!(levenshtein("paypal", "papal"), 1);
        assert_eq!(skeleton("Pay-Pa1"), "paypal");
        assert_eq!(skeleton("MICR0S0FT"), "microsoft");
    }
}
