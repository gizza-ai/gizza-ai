//! email-spam-score core — transparent, deterministic spam-likelihood scoring for an email.
//!
//! Every point in the score comes from a NAMED rule with a published weight, so the result is
//! auditable and reproducible: same input, same score, forever. There is no model, no training
//! data, no network. The tool never performs DNS, RBL, SMTP, HTTP, or reputation lookups — when it
//! reports SPF/DKIM/DMARC it is reading the `Authentication-Results` header the receiving gateway
//! already stamped into the message.
//!
//! Scale is 0–100 where HIGHER is spammier (0–29 LOW, 30–59 MEDIUM, 60–79 HIGH, 80–100 CRITICAL).

use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// Hard cap on the pasted message, to keep a browser wasm run bounded.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;

/// Trigger-phrase weight is capped so a single repetitive rule cannot drown out everything else.
const TRIGGER_CAP: i32 = 25;
/// Repeated-punctuation weight cap.
const PUNCT_CAP: i32 = 9;

// ---------------------------------------------------------------------------------------------
// Trigger phrases: (phrase, category, points). Matched case-insensitively on whitespace-collapsed
// text with word boundaries, over the subject + body. Each phrase contributes at most 3 hits.
// ---------------------------------------------------------------------------------------------
const TRIGGERS: &[(&str, &str, i32)] = &[
    // urgency
    ("act now", "urgency", 3),
    ("act immediately", "urgency", 3),
    ("apply now", "urgency", 2),
    ("call now", "urgency", 2),
    ("do not delete", "urgency", 2),
    ("expires today", "urgency", 3),
    ("final notice", "urgency", 3),
    ("hurry", "urgency", 1),
    ("immediate action", "urgency", 3),
    ("instant access", "urgency", 2),
    ("last chance", "urgency", 3),
    ("limited time", "urgency", 2),
    ("once in a lifetime", "urgency", 3),
    ("only today", "urgency", 2),
    ("today only", "urgency", 2),
    ("urgent", "urgency", 2),
    ("while supplies last", "urgency", 2),
    // money
    ("bitcoin", "money", 2),
    ("cash bonus", "money", 3),
    ("cheap", "money", 1),
    ("credit card offer", "money", 3),
    ("crypto investment", "money", 3),
    ("double your income", "money", 4),
    ("earn extra cash", "money", 4),
    ("eliminate debt", "money", 3),
    ("extra income", "money", 3),
    ("financial freedom", "money", 3),
    ("free money", "money", 4),
    ("get paid", "money", 2),
    ("guaranteed income", "money", 4),
    ("lowest price", "money", 2),
    ("make money", "money", 3),
    ("million dollars", "money", 3),
    ("money back", "money", 2),
    ("no credit check", "money", 3),
    ("risk free", "money", 2),
    ("save big money", "money", 3),
    ("wire transfer", "money", 3),
    // marketing hype
    ("100% free", "hype", 4),
    ("100% satisfied", "hype", 3),
    ("amazing", "hype", 1),
    ("buy now", "hype", 2),
    ("click below", "hype", 2),
    ("click here", "hype", 2),
    ("congratulations", "hype", 2),
    ("exclusive deal", "hype", 2),
    ("free gift", "hype", 3),
    ("free trial", "hype", 1),
    ("increase sales", "hype", 2),
    ("increase traffic", "hype", 2),
    ("limited offer", "hype", 3),
    ("no obligation", "hype", 2),
    ("opt in", "hype", 1),
    ("order now", "hype", 2),
    ("satisfaction guaranteed", "hype", 3),
    ("special promotion", "hype", 2),
    ("subscribe now", "hype", 1),
    ("you are a winner", "hype", 4),
    ("you have been selected", "hype", 3),
    // credentials / phishing
    ("account suspended", "credentials", 4),
    ("billing problem", "credentials", 3),
    ("click to verify", "credentials", 4),
    ("confirm your account", "credentials", 4),
    ("log in to confirm", "credentials", 4),
    ("payment failed", "credentials", 3),
    ("reset your password", "credentials", 2),
    ("security alert", "credentials", 2),
    ("unauthorized login", "credentials", 3),
    ("unusual activity", "credentials", 3),
    ("update your payment", "credentials", 4),
    ("validate your account", "credentials", 4),
    ("verify your account", "credentials", 4),
    ("your account will be closed", "credentials", 4),
    ("your password expires", "credentials", 3),
    // pharma / health
    ("anti-aging", "pharma", 2),
    ("diet pill", "pharma", 3),
    ("male enhancement", "pharma", 4),
    ("miracle cure", "pharma", 4),
    ("no prescription", "pharma", 4),
    ("online pharmacy", "pharma", 4),
    ("lose weight", "pharma", 2),
    ("weight loss", "pharma", 2),
    // gambling / adult
    ("adult content", "gambling", 3),
    ("betting", "gambling", 2),
    ("free spins", "gambling", 3),
    ("jackpot", "gambling", 3),
    ("online casino", "gambling", 4),
    ("casino", "gambling", 2),
    ("lottery", "gambling", 3),
    ("you have won", "gambling", 4),
];

/// Link-shortener hosts (registrable form) that hide the real destination.
const SHORTENERS: &[&str] = &[
    "bit.ly",
    "buff.ly",
    "cutt.ly",
    "goo.gl",
    "is.gd",
    "ow.ly",
    "rb.gy",
    "rebrand.ly",
    "shorturl.at",
    "t.co",
    "tiny.cc",
    "tinyurl.com",
    "t.ly",
    "lnkd.in",
];

/// TLDs disproportionately used for throwaway spam/phishing domains.
const SUSPICIOUS_TLDS: &[&str] = &[
    "cf", "click", "country", "ga", "gq", "link", "loan", "ml", "mov", "review", "tk", "top",
    "work", "xyz", "zip",
];

const ZERO_WIDTH: &[char] = &['\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{feff}'];

// ---------------------------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Rule {
    id: &'static str,
    points: i32,
    reason: String,
}

impl Rule {
    fn new(id: &'static str, points: i32, reason: impl Into<String>) -> Self {
        Self {
            id,
            points,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Default, Clone)]
struct Stats {
    words: usize,
    caps_ratio: f64,
    links: usize,
    unique_domains: usize,
    link_density: f64,
    images: usize,
    trigger_hits: usize,
    punct_runs: usize,
}

/// A parsed message: header block (may be empty) plus a body rendered to plain text.
#[derive(Debug, Default)]
struct Message {
    headers: BTreeMap<String, Vec<String>>,
    had_header_block: bool,
    raw_body: String,
    body_is_html: bool,
    text: String,
    subject: String,
}

impl Message {
    fn first(&self, name: &str) -> Option<&str> {
        self.headers
            .get(name)
            .and_then(|v| v.first())
            .map(String::as_str)
    }
    fn has(&self, name: &str) -> bool {
        self.headers.contains_key(name)
    }
}

// ---------------------------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------------------------

/// Score an email's spamminess with transparent, deterministic heuristics.
///
/// * `email` — the message: a raw RFC 5322 message (headers, blank line, body), an HTML body, or
///   plain body text.
/// * `subject` — subject line to use when the input carries no `Subject:` header (may be empty).
/// * `format` — `auto` | `raw` | `html` | `text`.
/// * `report` — `detailed` | `summary` | `json`.
/// * `check_headers` — include the header-anomaly rules (ignored when there is no header block).
pub fn run(
    email: &str,
    subject: &str,
    format: &str,
    report: &str,
    check_headers: bool,
) -> Result<String, String> {
    if email.trim().is_empty() {
        return Err("email is empty — paste a raw message (headers + blank line + body), an HTML body, or plain body text".into());
    }
    if email.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "email is {} bytes, which exceeds the {} byte limit — trim the message or remove inline base64 attachments",
            email.len(),
            MAX_INPUT_BYTES
        ));
    }
    let format = format.trim().to_ascii_lowercase();
    let format = if format.is_empty() {
        "auto".to_string()
    } else {
        format
    };
    let report = report.trim().to_ascii_lowercase();
    let report = if report.is_empty() {
        "detailed".to_string()
    } else {
        report
    };
    if !matches!(format.as_str(), "auto" | "raw" | "html" | "text") {
        return Err(format!(
            "unknown format '{format}' — expected one of: auto, raw, html, text"
        ));
    }
    if !matches!(report.as_str(), "detailed" | "summary" | "json") {
        return Err(format!(
            "unknown report '{report}' — expected one of: detailed, summary, json"
        ));
    }

    let msg = parse_message(email, subject, &format)?;
    let use_headers = check_headers && msg.had_header_block;

    let mut rules: Vec<Rule> = Vec::new();
    let stats = score_content(&msg, &mut rules);
    if use_headers {
        score_headers(&msg, &mut rules);
    }
    score_unsubscribe(&msg, use_headers, &mut rules);

    let raw_total: i32 = rules.iter().map(|r| r.points).sum();
    let score = raw_total.clamp(0, 100);
    let band = band_for(score);

    match report.as_str() {
        "summary" => Ok(render_summary(score, band, &msg, &rules)),
        "json" => Ok(render_json(score, band, &msg, &stats, &rules)),
        _ => Ok(render_detailed(score, band, &msg, &stats, &rules)),
    }
}

fn band_for(score: i32) -> &'static str {
    match score {
        s if s < 30 => "LOW",
        s if s < 60 => "MEDIUM",
        s if s < 80 => "HIGH",
        _ => "CRITICAL",
    }
}

fn verdict_for(band: &str) -> &'static str {
    match band {
        "LOW" => "reads as legitimate to rules-based spam filters",
        "MEDIUM" => "carries some spam signals — worth cleaning up before sending",
        "HIGH" => "likely to be filtered as spam",
        _ => "almost certainly treated as spam",
    }
}

// ---------------------------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------------------------

fn parse_message(email: &str, subject_param: &str, format: &str) -> Result<Message, String> {
    let normalized = email.replace("\r\n", "\n");
    let mut msg = Message::default();

    let (header_block, body) = match format {
        "raw" => {
            let (h, b) = split_headers(&normalized);
            if !looks_like_headers(h) {
                return Err(
                    "format=raw expected an RFC 5322 header block (`Header-Name: value` lines) before the first blank line — pass format=text or format=html for a body-only paste"
                        .into(),
                );
            }
            (Some(h), b)
        }
        "html" | "text" => (None, normalized.as_str()),
        _ => {
            // auto: treat as a raw message only when the first lines really are headers.
            let (h, b) = split_headers(&normalized);
            if looks_like_headers(h) {
                (Some(h), b)
            } else {
                (None, normalized.as_str())
            }
        }
    };

    if let Some(h) = header_block {
        msg.headers = parse_headers(h);
        msg.had_header_block = true;
    }
    msg.raw_body = body.to_string();
    msg.body_is_html = match format {
        "html" => true,
        "text" => false,
        _ => looks_like_html(&msg.raw_body),
    };
    msg.text = if msg.body_is_html {
        html_to_text(&msg.raw_body)
    } else {
        msg.raw_body.clone()
    };
    msg.subject = msg
        .first("subject")
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| subject_param.trim().to_string());

    if msg.text.trim().is_empty() && !msg.had_header_block {
        return Err("no message body found — the input contained no readable text".into());
    }
    Ok(msg)
}

/// Split at the first blank line. Returns (header block, body).
fn split_headers(s: &str) -> (&str, &str) {
    match s.find("\n\n") {
        Some(i) => (&s[..i], &s[i + 2..]),
        None => (s, ""),
    }
}

/// A header block must start with a `Name: value` line and be mostly such lines.
fn looks_like_headers(block: &str) -> bool {
    let mut total = 0usize;
    let mut header_like = 0usize;
    for (i, line) in block.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            continue; // folded continuation
        }
        total += 1;
        if is_header_line(line) {
            header_like += 1;
        } else if i == 0 {
            return false; // first line isn't a header → not a header block
        }
    }
    total > 0 && header_like * 2 > total
}

fn is_header_line(line: &str) -> bool {
    match line.find(':') {
        Some(0) | None => false,
        Some(i) => {
            let name = &line[..i];
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        }
    }
}

/// Parse a header block into lowercased-name → values, unfolding continuation lines.
fn parse_headers(block: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut current: Option<(String, String)> = None;
    for line in block.lines() {
        if (line.starts_with(' ') || line.starts_with('\t')) && current.is_some() {
            if let Some((_, v)) = current.as_mut() {
                v.push(' ');
                v.push_str(line.trim());
            }
            continue;
        }
        if let Some((name, value)) = current.take() {
            out.entry(name).or_default().push(value);
        }
        if let Some(i) = line.find(':') {
            if is_header_line(line) {
                current = Some((
                    line[..i].trim().to_ascii_lowercase(),
                    line[i + 1..].trim().to_string(),
                ));
            }
        }
    }
    if let Some((name, value)) = current {
        out.entry(name).or_default().push(value);
    }
    out
}

fn looks_like_html(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    [
        "<html", "<body", "<div", "<table", "<a href", "<img", "<br", "<p>", "<span", "<td",
    ]
    .iter()
    .any(|t| lower.contains(t))
}

/// Strip HTML to readable text: drop script/style contents, drop tags, decode common entities.
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let bytes: Vec<char> = html.chars().collect();
    let lower: String = html.to_ascii_lowercase();
    let lower_chars: Vec<char> = lower.chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == '<' {
            // skip whole script/style elements including their text content
            for tag in ["script", "style"] {
                if starts_with_at(&lower_chars, i + 1, tag) {
                    if let Some(end) = find_at(&lower_chars, i, &format!("</{tag}")) {
                        i = end;
                    }
                    break;
                }
            }
            match find_at(&bytes, i, ">") {
                Some(end) => {
                    out.push(' ');
                    i = end + 1;
                }
                None => break,
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    decode_entities(&out)
}

fn starts_with_at(hay: &[char], at: usize, needle: &str) -> bool {
    let n: Vec<char> = needle.chars().collect();
    at + n.len() <= hay.len() && hay[at..at + n.len()] == n[..]
}

fn find_at(hay: &[char], from: usize, needle: &str) -> Option<usize> {
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || from >= hay.len() {
        return None;
    }
    (from..=hay.len().saturating_sub(n.len())).find(|&i| hay[i..i + n.len()] == n[..])
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

// ---------------------------------------------------------------------------------------------
// URL / address helpers
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Link {
    url: String,
    host: String,
    scheme: String,
    userinfo: bool,
}

fn extract_links(text: &str) -> Vec<Link> {
    let lower = text.to_ascii_lowercase();
    let chars: Vec<char> = text.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let scheme = if starts_with_at(&lower_chars, i, "https://") {
            Some("https")
        } else if starts_with_at(&lower_chars, i, "http://") {
            Some("http")
        } else if starts_with_at(&lower_chars, i, "www.")
            && (i == 0 || !chars[i - 1].is_ascii_alphanumeric())
        {
            Some("www")
        } else {
            None
        };
        let Some(scheme) = scheme else {
            i += 1;
            continue;
        };
        let mut j = i;
        while j < chars.len() && !is_url_terminator(chars[j]) {
            j += 1;
        }
        // trailing punctuation is almost never part of the URL
        while j > i
            && matches!(
                chars[j - 1],
                '.' | ',' | ')' | ']' | ';' | ':' | '!' | '?' | '\''
            )
        {
            j -= 1;
        }
        let url: String = chars[i..j].iter().collect();
        if let Some(link) = to_link(&url, scheme) {
            out.push(link);
        }
        i = j.max(i + 1);
    }
    out
}

fn is_url_terminator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '`' | '\\')
}

fn to_link(url: &str, scheme: &str) -> Option<Link> {
    let rest = match scheme {
        "https" => url.get(8..)?,
        "http" => url.get(7..)?,
        _ => url,
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return None;
    }
    let (userinfo, hostport) = match authority.rsplit_once('@') {
        Some((_, h)) => (true, h),
        None => (false, authority),
    };
    let host = hostport
        .split(':')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    Some(Link {
        url: url.to_string(),
        host,
        scheme: scheme.to_string(),
        userinfo,
    })
}

/// Best-effort registrable domain: the last two labels. Good enough for grouping/comparison here;
/// the page documents that multi-level suffixes (`co.uk`) group at the two-label level.
fn registrable(host: &str) -> String {
    if is_ip_host(host) {
        return host.to_string();
    }
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    if labels.len() <= 2 {
        return labels.join(".");
    }
    labels[labels.len() - 2..].join(".")
}

fn is_ip_host(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    (parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())))
        || (host.contains(':') && host.chars().all(|c| c.is_ascii_hexdigit() || c == ':'))
}

fn tld(host: &str) -> String {
    host.rsplit('.').next().unwrap_or("").to_string()
}

/// Extract addr-spec-looking email addresses from free text.
fn extract_addresses(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    for (i, c) in chars.iter().enumerate() {
        if *c != '@' || i == 0 || i + 1 >= chars.len() {
            continue;
        }
        let mut a = i;
        while a > 0 && is_atext(chars[a - 1]) {
            a -= 1;
        }
        let mut b = i + 1;
        while b < chars.len()
            && (chars[b].is_ascii_alphanumeric() || chars[b] == '.' || chars[b] == '-')
        {
            b += 1;
        }
        if a == i || b == i + 1 {
            continue;
        }
        let local: String = chars[a..i].iter().collect();
        let domain: String = chars[i + 1..b].iter().collect();
        let domain = domain.trim_end_matches('.').to_string();
        if domain.contains('.') && !local.is_empty() {
            out.push(format!("{}@{}", local, domain).to_ascii_lowercase());
        }
    }
    out
}

fn is_atext(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-')
}

/// Pull the addr-spec out of a header value like `Name <a@b.com>` or `a@b.com`.
fn header_address(value: &str) -> Option<String> {
    let inner = match (value.find('<'), value.find('>')) {
        (Some(a), Some(b)) if b > a + 1 => &value[a + 1..b],
        _ => value,
    };
    extract_addresses(inner).into_iter().next()
}

fn address_domain(addr: &str) -> String {
    addr.rsplit('@').next().unwrap_or("").to_ascii_lowercase()
}

// ---------------------------------------------------------------------------------------------
// Content rules
// ---------------------------------------------------------------------------------------------

fn score_content(msg: &Message, rules: &mut Vec<Rule>) -> Stats {
    let mut stats = Stats::default();
    let text = &msg.text;
    let subject = &msg.subject;
    let scan = format!("{subject}\n{text}");

    // --- word count + caps ratio -------------------------------------------------------------
    stats.words = text
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count();
    let alpha: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if !alpha.is_empty() {
        let upper = alpha.iter().filter(|c| c.is_uppercase()).count();
        stats.caps_ratio = upper as f64 * 100.0 / alpha.len() as f64;
    }
    if alpha.len() >= 20 {
        if stats.caps_ratio >= 50.0 {
            rules.push(Rule::new(
                "CAPS_RATIO",
                14,
                format!(
                    "{:.1}% of body letters are uppercase (shouting; 50% or more)",
                    stats.caps_ratio
                ),
            ));
        } else if stats.caps_ratio > 30.0 {
            rules.push(Rule::new(
                "CAPS_RATIO",
                8,
                format!(
                    "{:.1}% of body letters are uppercase (over the 30% threshold)",
                    stats.caps_ratio
                ),
            ));
        }
    }

    // --- subject rules -------------------------------------------------------------------------
    let subj_alpha: Vec<char> = subject.chars().filter(|c| c.is_alphabetic()).collect();
    if subj_alpha.len() >= 8 {
        let upper = subj_alpha.iter().filter(|c| c.is_uppercase()).count();
        let ratio = upper as f64 * 100.0 / subj_alpha.len() as f64;
        if ratio > 60.0 {
            rules.push(Rule::new(
                "SUBJ_CAPS",
                6,
                format!("subject is {ratio:.1}% uppercase letters"),
            ));
        }
    }
    let bangs = subject.matches('!').count();
    if bangs >= 2 {
        rules.push(Rule::new(
            "SUBJ_EXCLAIM",
            5,
            format!("subject contains {bangs} exclamation marks"),
        ));
    } else if bangs == 1 {
        rules.push(Rule::new(
            "SUBJ_EXCLAIM",
            3,
            "subject contains an exclamation mark",
        ));
    }

    // --- trigger phrases -----------------------------------------------------------------------
    let hay = normalize_for_match(&scan);
    let mut per_category: BTreeMap<&str, usize> = BTreeMap::new();
    let mut trigger_points = 0i32;
    let mut matched: Vec<(&str, usize)> = Vec::new();
    for (phrase, category, weight) in TRIGGERS {
        let n = count_phrase(&hay, phrase).min(3);
        if n > 0 {
            stats.trigger_hits += n;
            *per_category.entry(category).or_default() += n;
            trigger_points += *weight * n as i32;
            matched.push((phrase, n));
        }
    }
    if stats.trigger_hits > 0 {
        let capped = trigger_points.min(TRIGGER_CAP);
        let cats = per_category
            .iter()
            .map(|(c, n)| format!("{c} x{n}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut top = matched.clone();
        top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let examples = top
            .iter()
            .take(3)
            .map(|(p, _)| format!("\"{p}\""))
            .collect::<Vec<_>>()
            .join(", ");
        rules.push(Rule::new(
            "TRIGGER_PHRASES",
            capped,
            format!(
                "{} spam trigger phrase hit{} ({cats}); e.g. {examples}",
                stats.trigger_hits,
                if stats.trigger_hits == 1 { "" } else { "s" }
            ),
        ));
    }

    // --- repeated punctuation + character runs ------------------------------------------------
    stats.punct_runs = count_punct_runs(text) + count_punct_runs(subject);
    if stats.punct_runs > 0 {
        let pts = (stats.punct_runs as i32 * 3).min(PUNCT_CAP);
        rules.push(Rule::new(
            "EXCESS_PUNCT",
            pts,
            format!(
                "{} run{} of repeated ! or ? punctuation",
                stats.punct_runs,
                if stats.punct_runs == 1 { "" } else { "s" }
            ),
        ));
    }
    if let Some(word) = find_repeated_char_word(&scan) {
        rules.push(Rule::new(
            "REPEATED_CHARS",
            5,
            format!("a letter is repeated 4 or more times inside a word (\"{word}\")"),
        ));
    }

    // --- unicode tricks -------------------------------------------------------------------------
    let zw = scan.chars().filter(|c| ZERO_WIDTH.contains(c)).count();
    if zw > 0 {
        rules.push(Rule::new(
            "ZERO_WIDTH",
            10,
            format!(
                "{zw} zero-width/invisible character(s) — often used to break up filtered words"
            ),
        ));
    }
    if let Some(word) = find_mixed_script_word(&scan) {
        rules.push(Rule::new(
            "MIXED_SCRIPT",
            12,
            format!("a word mixes Latin with lookalike non-Latin letters (\"{word}\") — homoglyph obfuscation"),
        ));
    }

    // --- links ----------------------------------------------------------------------------------
    let mut links = extract_links(&msg.raw_body);
    if msg.body_is_html {
        links.extend(extract_attr_links(&msg.raw_body));
    }
    let mut seen_urls = BTreeSet::new();
    links.retain(|l| seen_urls.insert(l.url.to_ascii_lowercase()));
    stats.links = links.len();
    let domains: BTreeSet<String> = links.iter().map(|l| registrable(&l.host)).collect();
    stats.unique_domains = domains.len();
    if stats.words > 0 {
        stats.link_density = stats.links as f64 * 100.0 / stats.words as f64;
    }

    if stats.links >= 10 {
        rules.push(Rule::new(
            "MANY_LINKS",
            6,
            format!("{} links in the message", stats.links),
        ));
    }
    if stats.words >= 20 && stats.link_density > 5.0 {
        rules.push(Rule::new(
            "LINK_DENSITY",
            8,
            format!(
                "{:.1} links per 100 words (over the 5.0 threshold) — link-heavy relative to the copy",
                stats.link_density
            ),
        ));
    }
    if let Some(l) = links
        .iter()
        .find(|l| SHORTENERS.contains(&registrable(&l.host).as_str()))
    {
        rules.push(Rule::new(
            "LINK_SHORTENER",
            9,
            format!(
                "link uses the URL shortener {} — the real destination is hidden",
                registrable(&l.host)
            ),
        ));
    }
    if let Some(l) = links
        .iter()
        .find(|l| !is_ip_host(&l.host) && SUSPICIOUS_TLDS.contains(&tld(&l.host).as_str()))
    {
        rules.push(Rule::new(
            "SUSPICIOUS_TLD",
            8,
            format!(
                "link host {} uses the .{} TLD, common in throwaway spam domains",
                l.host,
                tld(&l.host)
            ),
        ));
    }
    if links.iter().any(|l| l.scheme == "http") {
        rules.push(Rule::new(
            "INSECURE_LINK",
            4,
            "at least one link uses plain http:// instead of https://",
        ));
    }
    if let Some(l) = links.iter().find(|l| {
        l.userinfo || l.host.starts_with("xn--") || l.host.contains(".xn--") || is_ip_host(&l.host)
    }) {
        let why = if l.userinfo {
            "an @ userinfo section that hides the real host"
        } else if is_ip_host(&l.host) {
            "a bare IP address instead of a hostname"
        } else {
            "a punycode (xn--) hostname that can imitate another domain"
        };
        rules.push(Rule::new(
            "OBFUSCATED_URL",
            12,
            format!("link {} uses {why}", l.url),
        ));
    }

    // --- HTML-only rules -------------------------------------------------------------------------
    if msg.body_is_html {
        let imgs = extract_img_tags(&msg.raw_body);
        stats.images = imgs.len();
        if stats.images > 0 && stats.words < stats.images * 25 {
            rules.push(Rule::new(
                "IMAGE_HEAVY",
                10,
                format!(
                    "{} image(s) but only {} words of text — image-heavy mail is a classic filter evasion",
                    stats.images, stats.words
                ),
            ));
        }
        if imgs.iter().any(|t| is_tracking_pixel(t)) {
            rules.push(Rule::new(
                "TRACKING_PIXEL",
                3,
                "a 1x1 / zero-size tracking pixel image is embedded",
            ));
        }
        if let Some(marker) = find_hidden_text_marker(&msg.raw_body) {
            rules.push(Rule::new(
                "HIDDEN_TEXT",
                9,
                format!("markup hides content from the reader ({marker})"),
            ));
        }
        if let Some((anchor_dom, href_dom)) = find_anchor_mismatch(&msg.raw_body) {
            rules.push(Rule::new(
                "URL_MISMATCH",
                12,
                format!("link text shows {anchor_dom} but the href points at {href_dom}"),
            ));
        }
    }

    // --- addresses + length extremes ------------------------------------------------------------
    let addrs: BTreeSet<String> = extract_addresses(&msg.text).into_iter().collect();
    if addrs.len() >= 3 {
        rules.push(Rule::new(
            "MANY_ADDRESSES",
            5,
            format!(
                "{} distinct email addresses appear in the body",
                addrs.len()
            ),
        ));
    }
    if stats.words > 0 && stats.words < 5 {
        rules.push(Rule::new(
            "VERY_SHORT_BODY",
            4,
            format!(
                "only {} words of body text — too little content to look like real mail",
                stats.words
            ),
        ));
    }
    if stats.words > 3000 {
        rules.push(Rule::new(
            "VERY_LONG_BODY",
            3,
            format!("{} words of body text — unusually long", stats.words),
        ));
    }
    if let Some(amount) = find_big_money(&scan) {
        rules.push(Rule::new(
            "CURRENCY_HYPE",
            6,
            format!("large headline money amount in the copy ({amount})"),
        ));
    }

    stats
}

fn normalize_for_match(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut last_space = false;
    for c in lower.chars() {
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out
}

/// Count word-boundary-respecting occurrences of `needle` in `hay` (both already lowercased).
fn count_phrase(hay: &str, needle: &str) -> usize {
    let hb = hay.as_bytes();
    let nb = needle.as_bytes();
    if nb.is_empty() || nb.len() > hb.len() {
        return 0;
    }
    let mut count = 0usize;
    let mut i = 0usize;
    while i + nb.len() <= hb.len() {
        if &hb[i..i + nb.len()] == nb {
            let before_ok = i == 0 || !is_word_byte(hb[i - 1]);
            let after = i + nb.len();
            let after_ok = after >= hb.len() || !is_word_byte(hb[after]);
            if before_ok && after_ok {
                count += 1;
                i = after;
                continue;
            }
        }
        i += 1;
    }
    count
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn count_punct_runs(s: &str) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let mut runs = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '!' || chars[i] == '?' {
            let mut j = i;
            while j < chars.len() && (chars[j] == '!' || chars[j] == '?') {
                j += 1;
            }
            if j - i >= 2 {
                runs += 1;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    runs
}

fn find_repeated_char_word(s: &str) -> Option<String> {
    for word in s.split(|c: char| !c.is_alphanumeric()) {
        if word.chars().count() < 5 {
            continue;
        }
        let chars: Vec<char> = word.chars().collect();
        let mut run = 1usize;
        for i in 1..chars.len() {
            if chars[i].to_lowercase().eq(chars[i - 1].to_lowercase()) && chars[i].is_alphabetic() {
                run += 1;
                if run >= 4 {
                    return Some(word.to_string());
                }
            } else {
                run = 1;
            }
        }
    }
    None
}

fn find_mixed_script_word(s: &str) -> Option<String> {
    for word in s.split(|c: char| c.is_whitespace()) {
        let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
        if letters.len() < 3 {
            continue;
        }
        let has_latin = letters.iter().any(|c| c.is_ascii_alphabetic());
        let has_confusable = letters.iter().any(|c| {
            let u = *c as u32;
            // Cyrillic and Greek blocks host the common Latin lookalikes.
            (0x0370..=0x03ff).contains(&u) || (0x0400..=0x04ff).contains(&u)
        });
        if has_latin && has_confusable {
            return Some(
                word.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_string(),
            );
        }
    }
    None
}

fn find_big_money(s: &str) -> Option<String> {
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c != '$' && *c != '€' && *c != '£' {
            continue;
        }
        let mut j = i + 1;
        while j < chars.len() && chars[j] == ' ' {
            j += 1;
        }
        let start = j;
        let mut digits = 0usize;
        while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == ',' || chars[j] == '.') {
            if chars[j].is_ascii_digit() {
                digits += 1;
            }
            j += 1;
        }
        if digits >= 4 && start < j {
            let amount: String = chars[i..j].iter().collect();
            return Some(amount.trim_end_matches(['.', ',']).to_string());
        }
    }
    None
}

fn extract_attr_links(html: &str) -> Vec<Link> {
    let mut out = Vec::new();
    for value in extract_attr_values(html, &["href", "src"]) {
        let v = value.trim();
        let lower = v.to_ascii_lowercase();
        let scheme = if lower.starts_with("https://") {
            "https"
        } else if lower.starts_with("http://") {
            "http"
        } else if lower.starts_with("www.") {
            "www"
        } else {
            continue;
        };
        if let Some(l) = to_link(v, scheme) {
            out.push(l);
        }
    }
    out
}

/// Collect the values of the given attributes across the whole document.
fn extract_attr_values(html: &str, names: &[&str]) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();
    let mut out = Vec::new();
    for name in names {
        let pat = format!("{name}=");
        let mut i = 0usize;
        while let Some(pos) = find_at(&lower_chars, i, &pat) {
            let mut j = pos + pat.chars().count();
            if j >= chars.len() {
                break;
            }
            let value = match chars[j] {
                '"' | '\'' => {
                    let quote = chars[j];
                    j += 1;
                    let start = j;
                    while j < chars.len() && chars[j] != quote {
                        j += 1;
                    }
                    chars[start..j.min(chars.len())].iter().collect::<String>()
                }
                _ => {
                    let start = j;
                    while j < chars.len() && !chars[j].is_whitespace() && chars[j] != '>' {
                        j += 1;
                    }
                    chars[start..j].iter().collect::<String>()
                }
            };
            out.push(value);
            i = (j + 1).max(pos + 1);
        }
    }
    out
}

fn extract_img_tags(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(pos) = find_at(&lower_chars, i, "<img") {
        let after = pos + 4;
        if after < lower_chars.len()
            && (lower_chars[after].is_whitespace()
                || lower_chars[after] == '>'
                || lower_chars[after] == '/')
        {
            let end = find_at(&chars, pos, ">").unwrap_or(chars.len() - 1);
            out.push(
                chars[pos..=end.min(chars.len() - 1)]
                    .iter()
                    .collect::<String>(),
            );
            i = end + 1;
        } else {
            i = pos + 1;
        }
    }
    out
}

fn is_tracking_pixel(tag: &str) -> bool {
    let t = tag.to_ascii_lowercase().replace(' ', "");
    [
        "width=\"1\"",
        "width='1'",
        "width=1",
        "height=\"1\"",
        "height='1'",
        "height=1",
        "width=\"0\"",
        "height=\"0\"",
        "width:1px",
        "height:1px",
        "width:0",
        "height:0",
    ]
    .iter()
    .any(|p| t.contains(p))
}

fn find_hidden_text_marker(html: &str) -> Option<String> {
    let t = html.to_ascii_lowercase().replace(' ', "");
    for (needle, label) in [
        ("display:none", "display:none"),
        ("visibility:hidden", "visibility:hidden"),
        ("font-size:0", "font-size:0"),
        ("opacity:0", "opacity:0"),
    ] {
        if t.contains(needle) {
            return Some(label.to_string());
        }
    }
    None
}

/// Find the first anchor whose visible text names a different domain than its href.
fn find_anchor_mismatch(html: &str) -> Option<(String, String)> {
    let lower = html.to_ascii_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();
    let mut i = 0usize;
    while let Some(pos) = find_at(&lower_chars, i, "<a ") {
        let open_end = match find_at(&chars, pos, ">") {
            Some(e) => e,
            None => break,
        };
        let tag: String = chars[pos..=open_end].iter().collect();
        let href = extract_attr_values(&tag, &["href"])
            .into_iter()
            .next()
            .unwrap_or_default();
        let close = find_at(&lower_chars, open_end, "</a").unwrap_or(chars.len());
        let inner: String = chars[(open_end + 1).min(chars.len())..close.min(chars.len())]
            .iter()
            .collect();
        let inner_text = html_to_text(&inner);
        i = close.max(pos + 1);

        let Some(href_link) = to_link_any(&href) else {
            continue;
        };
        let href_dom = registrable(&href_link.host);
        // A domain named in the visible text (as a URL or a bare host).
        let shown = extract_links(&inner_text)
            .into_iter()
            .map(|l| registrable(&l.host))
            .chain(bare_hosts(&inner_text).into_iter().map(|h| registrable(&h)))
            .find(|d| !d.is_empty());
        if let Some(shown) = shown {
            if shown != href_dom {
                return Some((shown, href_dom));
            }
        }
    }
    None
}

fn to_link_any(url: &str) -> Option<Link> {
    let lower = url.trim().to_ascii_lowercase();
    let scheme = if lower.starts_with("https://") {
        "https"
    } else if lower.starts_with("http://") {
        "http"
    } else if lower.starts_with("www.") {
        "www"
    } else {
        return None;
    };
    to_link(url.trim(), scheme)
}

/// Hostname-looking tokens (`login.example.com`) in visible text, with no scheme.
fn bare_hosts(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in
        text.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | ',' | '"' | '<' | '>'))
    {
        let t = token
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_ascii_lowercase();
        if t.contains('@') || !t.contains('.') {
            continue;
        }
        let labels: Vec<&str> = t.split('.').collect();
        if labels.len() >= 2
            && labels
                .iter()
                .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
            && labels
                .last()
                .is_some_and(|l| l.len() >= 2 && l.chars().all(|c| c.is_ascii_alphabetic()))
        {
            out.push(t);
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Header rules
// ---------------------------------------------------------------------------------------------

fn score_headers(msg: &Message, rules: &mut Vec<Rule>) {
    let from_addr = msg.first("from").and_then(header_address);
    let from_dom = from_addr.as_deref().map(address_domain).unwrap_or_default();

    // Authentication-Results (already stamped by the receiving gateway — never looked up here).
    let auth = msg
        .first("authentication-results")
        .map(str::to_ascii_lowercase);
    match auth.as_deref() {
        None => rules.push(Rule::new(
            "AUTH_MISSING",
            6,
            "no Authentication-Results header — SPF/DKIM/DMARC outcomes are unknown",
        )),
        Some(a) => {
            let spf = auth_verdict(a, "spf");
            let dkim = auth_verdict(a, "dkim");
            let dmarc = auth_verdict(a, "dmarc");
            let failed: Vec<&str> = [("spf", spf), ("dkim", dkim), ("dmarc", dmarc)]
                .iter()
                .filter(|(_, v)| matches!(*v, Some("fail") | Some("softfail") | Some("permerror")))
                .map(|(n, _)| *n)
                .collect();
            if !failed.is_empty() {
                rules.push(Rule::new(
                    "AUTH_FAIL",
                    if failed.len() >= 2 { 18 } else { 12 },
                    format!(
                        "{} did not pass in Authentication-Results",
                        failed.join(" and ").to_uppercase()
                    ),
                ));
            } else if spf == Some("pass") && dkim == Some("pass") && dmarc == Some("pass") {
                rules.push(Rule::new("AUTH_PASS", -12, "SPF, DKIM and DMARC all pass"));
            }
        }
    }

    // From vs Return-Path
    if let (Some(rp), false) = (
        msg.first("return-path").and_then(header_address),
        from_dom.is_empty(),
    ) {
        let rp_dom = address_domain(&rp);
        if !rp_dom.is_empty() && registrable(&rp_dom) != registrable(&from_dom) {
            rules.push(Rule::new(
                "FROM_RETURNPATH_MISMATCH",
                10,
                format!("From domain {from_dom} does not match Return-Path domain {rp_dom}"),
            ));
        }
    }

    // Reply-To detour
    if let (Some(rt), false) = (
        msg.first("reply-to").and_then(header_address),
        from_dom.is_empty(),
    ) {
        let rt_dom = address_domain(&rt);
        if !rt_dom.is_empty() && registrable(&rt_dom) != registrable(&from_dom) {
            rules.push(Rule::new(
                "REPLYTO_MISMATCH",
                9,
                format!(
                    "replies go to {rt_dom}, a different domain than the From domain {from_dom}"
                ),
            ));
        }
    }

    // Display-name spoofing
    if let Some(raw_from) = msg.first("from") {
        let display = raw_from
            .split('<')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches('"')
            .to_string();
        if !display.is_empty() && !from_dom.is_empty() {
            let claimed = extract_addresses(&display)
                .into_iter()
                .map(|a| address_domain(&a))
                .chain(bare_hosts(&display))
                .find(|d| !d.is_empty() && registrable(d) != registrable(&from_dom));
            if let Some(claimed) = claimed {
                rules.push(Rule::new(
                    "DISPLAY_NAME_SPOOF",
                    14,
                    format!("the From display name shows {claimed} but the real mailbox is at {from_dom}"),
                ));
            }
        }
    }

    // Message-ID
    match msg.first("message-id") {
        None => rules.push(Rule::new(
            "MISSING_MESSAGE_ID",
            5,
            "no Message-ID header — real mail systems always add one",
        )),
        Some(mid) => {
            let mid_dom = mid
                .trim()
                .trim_matches(['<', '>'])
                .rsplit('@')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            if !mid_dom.is_empty()
                && !from_dom.is_empty()
                && registrable(&mid_dom) != registrable(&from_dom)
            {
                rules.push(Rule::new(
                    "MSGID_DOMAIN_MISMATCH",
                    4,
                    format!("Message-ID domain {mid_dom} differs from the From domain {from_dom} (normal for some ESPs)"),
                ));
            }
        }
    }

    if !msg.has("date") {
        rules.push(Rule::new("MISSING_DATE", 3, "no Date header"));
    }
    if !msg.has("received") {
        rules.push(Rule::new(
            "NO_RECEIVED",
            4,
            "no Received header — the relay path cannot be checked at all",
        ));
    }
    if msg.subject.trim().is_empty() {
        rules.push(Rule::new(
            "SUBJECT_MISSING",
            3,
            "the message has no subject",
        ));
    }
    let to = msg.first("to").unwrap_or("").to_ascii_lowercase();
    if to.is_empty() || to.contains("undisclosed-recipients") {
        rules.push(Rule::new(
            "UNDISCLOSED_RECIPIENTS",
            4,
            "the To header is missing or hides the recipients (bulk-mail pattern)",
        ));
    }
    let precedence = msg.first("precedence").unwrap_or("").to_ascii_lowercase();
    if matches!(precedence.trim(), "bulk" | "junk" | "list") {
        rules.push(Rule::new(
            "PRECEDENCE_BULK",
            2,
            format!("Precedence: {} marks this as bulk mail", precedence.trim()),
        ));
    }
}

fn auth_verdict<'a>(auth: &'a str, mech: &str) -> Option<&'a str> {
    let pat = format!("{mech}=");
    let idx = auth.find(&pat)?;
    let rest = &auth[idx + pat.len()..];
    let word: &str = rest
        .split(|c: char| !c.is_ascii_alphabetic())
        .next()
        .unwrap_or("");
    if word.is_empty() {
        None
    } else {
        Some(word)
    }
}

fn score_unsubscribe(msg: &Message, use_headers: bool, rules: &mut Vec<Rule>) {
    let header = use_headers && msg.has("list-unsubscribe");
    let in_body = normalize_for_match(&msg.text).contains("unsubscribe");
    if header || in_body {
        let where_ = if header {
            "a List-Unsubscribe header"
        } else {
            "an unsubscribe mention in the body"
        };
        rules.push(Rule::new(
            "HAS_UNSUBSCRIBE",
            -5,
            format!("the message provides {where_}"),
        ));
    }
}

// ---------------------------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------------------------

fn parsed_as(msg: &Message) -> &'static str {
    match (msg.had_header_block, msg.body_is_html) {
        (true, true) => "raw email with an HTML body",
        (true, false) => "raw email with a plain-text body",
        (false, true) => "HTML body (no headers)",
        (false, false) => "plain-text body (no headers)",
    }
}

fn render_detailed(score: i32, band: &str, msg: &Message, stats: &Stats, rules: &[Rule]) -> String {
    let mut out = String::new();
    out.push_str(&format!("Spam score: {score}/100 ({band})\n"));
    out.push_str(&format!("Verdict: {}\n", verdict_for(band)));
    out.push_str(&format!("Input parsed as: {}\n", parsed_as(msg)));
    out.push('\n');

    out.push_str("Message stats\n");
    for (label, value) in stats_rows(stats) {
        out.push_str(&format!("  {label:<21}{value}\n"));
    }

    let (fired, reducing): (Vec<&Rule>, Vec<&Rule>) = rules.iter().partition(|r| r.points > 0);
    let mut fired = fired;
    fired.sort_by(|a, b| b.points.cmp(&a.points).then(a.id.cmp(b.id)));

    out.push('\n');
    out.push_str(&format!("Rules fired ({})\n", fired.len()));
    if fired.is_empty() {
        out.push_str("  none — no spam heuristic matched this message\n");
    }
    for r in &fired {
        out.push_str(&format!(
            "  {:>4}  {:<24}  {}\n",
            format!("{:+}", r.points),
            r.id,
            r.reason
        ));
    }

    if !reducing.is_empty() {
        out.push('\n');
        out.push_str(&format!("Score-reducing signals ({})\n", reducing.len()));
        for r in &reducing {
            out.push_str(&format!(
                "  {:>4}  {:<24}  {}\n",
                format!("{:+}", r.points),
                r.id,
                r.reason
            ));
        }
    }
    out.trim_end().to_string()
}

fn stats_rows(stats: &Stats) -> Vec<(&'static str, String)> {
    vec![
        ("Words", stats.words.to_string()),
        ("Uppercase ratio", format!("{:.1}%", stats.caps_ratio)),
        (
            "Links",
            format!("{} ({} unique domains)", stats.links, stats.unique_domains),
        ),
        (
            "Link density",
            format!("{:.1} per 100 words", stats.link_density),
        ),
        ("Images", stats.images.to_string()),
        ("Trigger phrase hits", stats.trigger_hits.to_string()),
        ("Punctuation runs", stats.punct_runs.to_string()),
    ]
}

fn render_summary(score: i32, band: &str, msg: &Message, rules: &[Rule]) -> String {
    let mut fired: Vec<&Rule> = rules.iter().filter(|r| r.points > 0).collect();
    fired.sort_by(|a, b| b.points.cmp(&a.points).then(a.id.cmp(b.id)));
    let top = if fired.is_empty() {
        "none".to_string()
    } else {
        fired
            .iter()
            .take(3)
            .map(|r| format!("{} ({:+})", r.id, r.points))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "Spam score: {score}/100 ({band})\nVerdict: {}\nInput parsed as: {}\nTop signals: {top}",
        verdict_for(band),
        parsed_as(msg)
    )
}

fn render_json(score: i32, band: &str, msg: &Message, stats: &Stats, rules: &[Rule]) -> String {
    let mut fired: Vec<&Rule> = rules.iter().collect();
    fired.sort_by(|a, b| b.points.cmp(&a.points).then(a.id.cmp(b.id)));
    let items = fired
        .iter()
        .map(|r| {
            format!(
                "    {{ \"id\": \"{}\", \"points\": {}, \"reason\": \"{}\" }}",
                r.id,
                r.points,
                json_escape(&r.reason)
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\n  \"score\": {score},\n  \"max_score\": 100,\n  \"band\": \"{band}\",\n  \"verdict\": \"{}\",\n  \"parsed_as\": \"{}\",\n  \"stats\": {{\n    \"words\": {},\n    \"uppercase_ratio_pct\": {:.1},\n    \"links\": {},\n    \"unique_link_domains\": {},\n    \"link_density_per_100_words\": {:.1},\n    \"images\": {},\n    \"trigger_phrase_hits\": {},\n    \"punctuation_runs\": {}\n  }},\n  \"rules\": [{}{}{}]\n}}",
        json_escape(verdict_for(band)),
        json_escape(parsed_as(msg)),
        stats.words,
        stats.caps_ratio,
        stats.links,
        stats.unique_domains,
        stats.link_density,
        stats.images,
        stats.trigger_hits,
        stats.punct_runs,
        if items.is_empty() { "" } else { "\n" },
        items,
        if items.is_empty() { "" } else { "\n  " },
    )
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const CLEAN_RAW: &str = "From: Ada Lovelace <ada@example.com>\nTo: team@example.com\nReturn-Path: <ada@example.com>\nDate: Tue, 1 Jan 2026 09:00:00 +0000\nMessage-ID: <a1@example.com>\nSubject: Notes from Tuesday's planning session\nReceived: from mail.example.com by mx.example.com\nAuthentication-Results: mx.example.com; spf=pass; dkim=pass; dmarc=pass\n\nHi team,\n\nHere are the notes from our planning session. The rollout stays on the schedule we agreed, and the migration document is linked from the shared drive at https://docs.example.com/plan for anyone who needs the detail.\n\nThanks,\nAda\n";

    const SPAMMY_RAW: &str = "From: \"security@yourbank.com\" <notice@grabber.top>\nTo: undisclosed-recipients:;\nReturn-Path: <bounce@mailer.example>\nReply-To: help@other.example\nSubject: URGENT!! VERIFY YOUR ACCOUNT NOW!!\nAuthentication-Results: mx.example.com; spf=fail; dkim=none; dmarc=fail\n\nACT NOW! YOUR ACCOUNT SUSPENDED. CLICK HERE TO VERIFY YOUR ACCOUNT AND CLAIM YOUR FREE GIFT OF $50,000 CASH!!! FINAL NOTICE, IMMEDIATE ACTION REQUIRED!!!\nhttp://bit.ly/x1 http://192.0.2.9/login http://grabber.top/go\n";

    #[test]
    fn happy_path_clean_message_scores_low() {
        let out = run(CLEAN_RAW, "", "auto", "detailed", true).unwrap();
        assert!(out.starts_with("Spam score: 0/100 (LOW)"), "got:\n{out}");
        assert!(
            out.contains("AUTH_PASS"),
            "clean mail should earn the AUTH_PASS credit:\n{out}"
        );
        assert!(
            out.contains("Input parsed as: raw email with a plain-text body"),
            "{out}"
        );
    }

    #[test]
    fn happy_path_spammy_message_scores_critical() {
        let out = run(SPAMMY_RAW, "", "auto", "detailed", true).unwrap();
        let first = out.lines().next().unwrap();
        assert!(
            first.ends_with("(CRITICAL)"),
            "expected CRITICAL band, got: {first}\n{out}"
        );
        for id in [
            "TRIGGER_PHRASES",
            "CAPS_RATIO",
            "EXCESS_PUNCT",
            "LINK_SHORTENER",
            "SUSPICIOUS_TLD",
            "OBFUSCATED_URL",
            "AUTH_FAIL",
            "DISPLAY_NAME_SPOOF",
            "FROM_RETURNPATH_MISMATCH",
            "REPLYTO_MISMATCH",
            "UNDISCLOSED_RECIPIENTS",
        ] {
            assert!(out.contains(id), "expected rule {id} to fire:\n{out}");
        }
    }

    #[test]
    fn error_on_empty_input() {
        let err = run("   \n ", "", "auto", "detailed", true).unwrap_err();
        assert!(err.contains("email is empty"), "{err}");
    }

    #[test]
    fn error_on_unknown_format_and_report() {
        assert!(run("hello there", "", "eml", "detailed", true)
            .unwrap_err()
            .contains("unknown format 'eml'"));
        assert!(run("hello there", "", "auto", "verbose", true)
            .unwrap_err()
            .contains("unknown report 'verbose'"));
    }

    #[test]
    fn error_when_raw_format_gets_a_body_only_paste() {
        let err = run("Buy now, this is just a body.", "", "raw", "detailed", true).unwrap_err();
        assert!(err.contains("RFC 5322 header block"), "{err}");
    }

    #[test]
    fn error_when_input_too_large() {
        let big = "a ".repeat(MAX_INPUT_BYTES);
        let err = run(&big, "", "text", "detailed", true).unwrap_err();
        assert!(err.contains("exceeds the"), "{err}");
    }

    #[test]
    fn check_headers_false_drops_header_rules() {
        let with = run(SPAMMY_RAW, "", "auto", "detailed", true).unwrap();
        let without = run(SPAMMY_RAW, "", "auto", "detailed", false).unwrap();
        assert!(with.contains("AUTH_FAIL"));
        assert!(!without.contains("AUTH_FAIL"), "{without}");
        assert!(!without.contains("DISPLAY_NAME_SPOOF"), "{without}");
        // content rules survive
        assert!(without.contains("TRIGGER_PHRASES"), "{without}");
    }

    #[test]
    fn subject_param_is_used_when_no_subject_header() {
        let out = run(
            "Please review the attached notes when you get a chance.",
            "WIN A FREE IPHONE NOW",
            "text",
            "detailed",
            true,
        )
        .unwrap();
        assert!(out.contains("SUBJ_CAPS"), "{out}");
    }

    #[test]
    fn summary_report_lists_top_signals() {
        let out = run(SPAMMY_RAW, "", "auto", "summary", true).unwrap();
        assert_eq!(out.lines().count(), 4, "{out}");
        assert!(
            out.lines().nth(3).unwrap().starts_with("Top signals: "),
            "{out}"
        );
    }

    #[test]
    fn json_report_is_valid_shape() {
        let out = run(CLEAN_RAW, "", "auto", "json", true).unwrap();
        assert!(
            out.starts_with("{\n  \"score\": 0,\n  \"max_score\": 100,\n  \"band\": \"LOW\","),
            "{out}"
        );
        assert!(out.contains("\"unique_link_domains\": 1"), "{out}");
        assert!(
            out.contains("\"id\": \"HAS_UNSUBSCRIBE\"") || out.contains("\"id\": \"AUTH_PASS\""),
            "{out}"
        );
        assert!(out.trim_end().ends_with('}'));
    }

    #[test]
    fn html_body_rules_fire() {
        let html = "<html><body><p>Hello, this newsletter has a little text.</p>\
             <img src=\"https://cdn.example.com/a.png\"><img src=\"https://cdn.example.com/b.png\">\
             <img src=\"https://track.example.com/p.gif\" width=\"1\" height=\"1\">\
             <div style=\"display:none\">hidden keywords</div>\
             <a href=\"https://evil.example/login\">https://secure.mybank.com/login</a></body></html>";
        let out = run(html, "Newsletter", "html", "detailed", true).unwrap();
        for id in [
            "IMAGE_HEAVY",
            "TRACKING_PIXEL",
            "HIDDEN_TEXT",
            "URL_MISMATCH",
        ] {
            assert!(out.contains(id), "expected {id}:\n{out}");
        }
        assert!(
            out.contains("Input parsed as: HTML body (no headers)"),
            "{out}"
        );
    }

    #[test]
    fn zero_width_and_homoglyph_tricks_are_caught() {
        let body = "Cl\u{200b}ick here to cl\u{0430}im your prize today, it is completely genuine.";
        let out = run(body, "Prize", "text", "detailed", true).unwrap();
        assert!(out.contains("ZERO_WIDTH"), "{out}");
        assert!(out.contains("MIXED_SCRIPT"), "{out}");
    }

    #[test]
    fn trigger_phrases_respect_word_boundaries() {
        // "cheap" must not match inside "cheaply"? it does not: boundary check on both sides.
        assert_eq!(count_phrase("cheaply made goods", "cheap"), 0);
        assert_eq!(count_phrase("very cheap goods", "cheap"), 1);
        assert_eq!(
            count_phrase("act now, act now, act now, act now", "act now"),
            4
        );
    }

    #[test]
    fn score_is_clamped_to_the_0_100_range() {
        let out = run(SPAMMY_RAW, "", "auto", "json", true).unwrap();
        let score_line = out.lines().nth(1).unwrap();
        let n: i32 = score_line
            .trim()
            .trim_start_matches("\"score\": ")
            .trim_end_matches(',')
            .parse()
            .unwrap();
        assert!((0..=100).contains(&n), "{score_line}");
    }

    #[test]
    fn folded_headers_are_unfolded() {
        let raw = "From: Ada <ada@example.com>\nSubject: A very long subject line\n that was folded across two lines\nDate: Tue, 1 Jan 2026 09:00:00 +0000\nMessage-ID: <x@example.com>\nReceived: from a by b\nTo: b@example.com\nAuthentication-Results: mx; spf=pass; dkim=pass; dmarc=pass\n\nA short but perfectly ordinary body paragraph goes right here.\n";
        let out = run(raw, "", "raw", "json", true).unwrap();
        assert!(!out.contains("SUBJECT_MISSING"), "{out}");
    }

    #[test]
    fn registrable_and_ip_hosts() {
        assert_eq!(registrable("mail.example.co.uk"), "co.uk");
        assert_eq!(registrable("example.com"), "example.com");
        assert_eq!(registrable("192.0.2.9"), "192.0.2.9");
        assert!(is_ip_host("192.0.2.9"));
        assert!(!is_ip_host("example.com"));
    }
}
