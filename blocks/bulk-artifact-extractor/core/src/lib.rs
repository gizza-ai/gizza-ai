//! bulk-artifact-extractor core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps — deterministic pure Rust.
//!
//! Scans a blob of text for common indicators-of-interest ("artifacts") and
//! reports each hit with its **kind**, exact **value**, **byte offset**, and a
//! short **context** snippet. Detects: email addresses, URLs, IPv4 addresses,
//! bare domains, phone numbers, Bitcoin-like addresses, and Luhn-valid
//! credit-card numbers. Overlapping hits are resolved by specificity (a domain
//! inside an email/URL, or an IP inside a URL, is not reported twice), so the
//! output is a clean, de-duplicated, deterministically ordered list. Renders a
//! Markdown **table** (default) or a **json** array; filter by kind and cap the
//! number of hits.

use regex::Regex;
use std::fmt::Write as _;

/// Default hit cap when `limit` is 0/unset.
const DEFAULT_LIMIT: u32 = 1000;
/// Hard upper bound the `limit` param is clamped to.
pub const MAX_LIMIT: u32 = 20000;
/// Default context characters on each side of a hit.
const DEFAULT_CONTEXT: u32 = 24;
/// Hard upper bound the `context` param is clamped to.
pub const MAX_CONTEXT: u32 = 200;

// ---------------------------------------------------------------------------
// Artifact kinds.
// ---------------------------------------------------------------------------

/// The kind of artifact a hit represents. The array order is the **overlap
/// priority** (earlier = more specific = wins when spans overlap) AND the
/// stable tie-break order for hits at the same offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Url,
    Email,
    CreditCard,
    Ipv4,
    Bitcoin,
    Phone,
    Domain,
}

/// All kinds in priority order — see [`Kind`].
const ALL_KINDS: [Kind; 7] = [
    Kind::Url,
    Kind::Email,
    Kind::CreditCard,
    Kind::Ipv4,
    Kind::Bitcoin,
    Kind::Phone,
    Kind::Domain,
];

impl Kind {
    /// The stable machine label used in output and the `kinds` filter.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Url => "url",
            Kind::Email => "email",
            Kind::CreditCard => "credit_card",
            Kind::Ipv4 => "ipv4",
            Kind::Bitcoin => "bitcoin",
            Kind::Phone => "phone",
            Kind::Domain => "domain",
        }
    }

    /// Priority index (lower wins on overlap / sorts first on ties).
    fn priority(self) -> usize {
        ALL_KINDS.iter().position(|&k| k == self).unwrap()
    }

    fn from_label(s: &str) -> Option<Kind> {
        ALL_KINDS.iter().copied().find(|k| k.label() == s)
    }
}

// ---------------------------------------------------------------------------
// Kind filter.
// ---------------------------------------------------------------------------

/// Which kinds the caller wants reported.
#[derive(Debug, Clone)]
pub enum KindFilter {
    All,
    /// A non-empty subset, in canonical priority order.
    Some(Vec<Kind>),
}

impl KindFilter {
    /// Parse a comma/space-separated list of kind labels (or `all`).
    pub fn parse(s: &str) -> Result<KindFilter, String> {
        let s = s.trim();
        if s.is_empty() || s.eq_ignore_ascii_case("all") {
            return Ok(KindFilter::All);
        }
        let mut want: Vec<Kind> = Vec::new();
        for tok in s.split([',', ' ', '\t', '\n']).filter(|t| !t.is_empty()) {
            let norm = tok.trim().to_ascii_lowercase().replace(['-', ' '], "_");
            let k = match norm.as_str() {
                "all" => return Ok(KindFilter::All),
                "creditcard" | "card" | "cc" => Kind::CreditCard,
                "ip" | "ipv4" | "ipaddress" => Kind::Ipv4,
                "btc" | "bitcoin" => Kind::Bitcoin,
                "tel" | "telephone" | "phone" => Kind::Phone,
                other => Kind::from_label(other).ok_or_else(|| {
                    format!(
                        "unknown kind '{tok}' (use all, or a comma-list of: {})",
                        ALL_KINDS
                            .iter()
                            .map(|k| k.label())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?,
            };
            if !want.contains(&k) {
                want.push(k);
            }
        }
        if want.is_empty() {
            return Ok(KindFilter::All);
        }
        // Canonicalise to priority order for deterministic behaviour.
        want.sort_by_key(|k| k.priority());
        Ok(KindFilter::Some(want))
    }

    fn keeps(&self, k: Kind) -> bool {
        match self {
            KindFilter::All => true,
            KindFilter::Some(v) => v.contains(&k),
        }
    }
}

// ---------------------------------------------------------------------------
// Output shape.
// ---------------------------------------------------------------------------

/// How to render the findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Table,
    Json,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "table" | "markdown" | "md" => Output::Table,
            "json" => Output::Json,
            other => return Err(format!("unknown output '{other}' (use table or json)")),
        })
    }
}

// ---------------------------------------------------------------------------
// A single finding.
// ---------------------------------------------------------------------------

/// One extracted artifact.
#[derive(Debug, Clone)]
pub struct Finding {
    pub kind: Kind,
    pub value: String,
    /// Byte offset of the value's first byte in the original input.
    pub offset: usize,
    /// Short surrounding text (newlines flattened to spaces, ends elided).
    pub context: String,
}

// ---------------------------------------------------------------------------
// Compiled matchers (built once per call).
// ---------------------------------------------------------------------------

struct Matchers {
    email: Regex,
    url: Regex,
    ipv4: Regex,
    domain: Regex,
    phone: Regex,
    // Bitcoin: legacy base58 (P2PKH/P2SH) and bech32 (segwit).
    btc_base58: Regex,
    btc_bech32: Regex,
    // A run of 13-19 digits (optionally space/dash grouped) — Luhn-checked.
    card: Regex,
}

impl Matchers {
    fn new() -> Matchers {
        // The `regex` crate has no look-around, so boundaries lean on `\b` and
        // explicit character classes; trailing punctuation on URLs is trimmed
        // in post-processing.
        Matchers {
            email: Regex::new(r"(?i)\b[a-z0-9._%+\-]+@[a-z0-9](?:[a-z0-9\-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9\-]*[a-z0-9])?)*\.[a-z]{2,24}\b").unwrap(),
            url: Regex::new(r#"(?i)\b(?:https?://|www\.)[^\s<>"'`(){}\[\]|\\^]+"#).unwrap(),
            ipv4: Regex::new(
                r"\b(?:(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9]?[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9]?[0-9])\b",
            )
            .unwrap(),
            // Bare domain: 1+ labels then a 2-24 char alphabetic TLD.
            domain: Regex::new(
                r"(?i)\b(?:[a-z0-9](?:[a-z0-9\-]*[a-z0-9])?\.)+[a-z]{2,24}\b",
            )
            .unwrap(),
            // Grouped digits with at least one separator (avoids matching plain
            // digit runs, which are more likely IDs/cards); digit count is
            // range-checked in post-processing.
            phone: Regex::new(
                r"\+?[0-9](?:[0-9]{0,3})(?:[ .\-][0-9]{1,4}){1,6}|\+?[0-9]{1,3}[ .\-]?\([0-9]{1,4}\)[ .\-]?[0-9]{1,4}(?:[ .\-][0-9]{1,4}){1,4}",
            )
            .unwrap(),
            btc_base58: Regex::new(r"\b[13][a-km-zA-HJ-NP-Z1-9]{25,34}\b").unwrap(),
            btc_bech32: Regex::new(r"\bbc1[ac-hj-np-z02-9]{8,87}\b").unwrap(),
            card: Regex::new(r"\b[0-9](?:[ \-]?[0-9]){12,18}\b").unwrap(),
        }
    }
}

// ---------------------------------------------------------------------------
// Detection.
// ---------------------------------------------------------------------------

/// A candidate span before overlap resolution.
struct Cand {
    start: usize,
    end: usize,
    kind: Kind,
    value: String,
}

/// Trailing punctuation that shouldn't be part of a URL/domain value.
fn trim_trailing(value: &str) -> &str {
    value.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\''])
}

/// Count ASCII digits in a string.
fn digit_count(s: &str) -> usize {
    s.bytes().filter(|b| b.is_ascii_digit()).count()
}

/// Luhn checksum over the digits in `s` (non-digits ignored).
fn luhn_valid(s: &str) -> bool {
    let digits: Vec<u32> = s
        .bytes()
        .filter(|b| b.is_ascii_digit())
        .map(|b| (b - b'0') as u32)
        .collect();
    let n = digits.len();
    if !(13..=19).contains(&n) {
        return false;
    }
    let mut sum = 0u32;
    // Double every second digit counting from the right.
    for (i, &d) in digits.iter().rev().enumerate() {
        let v = if i % 2 == 1 {
            let dd = d * 2;
            if dd > 9 {
                dd - 9
            } else {
                dd
            }
        } else {
            d
        };
        sum += v;
    }
    sum % 10 == 0
}

/// Collect every candidate match across all kinds (unfiltered, unresolved).
fn candidates(m: &Matchers, text: &str) -> Vec<Cand> {
    let mut out: Vec<Cand> = Vec::new();

    for mat in m.email.find_iter(text) {
        out.push(Cand {
            start: mat.start(),
            end: mat.end(),
            kind: Kind::Email,
            value: mat.as_str().to_string(),
        });
    }
    for mat in m.url.find_iter(text) {
        let trimmed = trim_trailing(mat.as_str());
        if trimmed.is_empty() {
            continue;
        }
        out.push(Cand {
            start: mat.start(),
            end: mat.start() + trimmed.len(),
            kind: Kind::Url,
            value: trimmed.to_string(),
        });
    }
    for mat in m.ipv4.find_iter(text) {
        out.push(Cand {
            start: mat.start(),
            end: mat.end(),
            kind: Kind::Ipv4,
            value: mat.as_str().to_string(),
        });
    }
    for mat in m.domain.find_iter(text) {
        let trimmed = trim_trailing(mat.as_str());
        if trimmed.is_empty() {
            continue;
        }
        out.push(Cand {
            start: mat.start(),
            end: mat.start() + trimmed.len(),
            kind: Kind::Domain,
            value: trimmed.to_string(),
        });
    }
    for mat in m.phone.find_iter(text) {
        let n = digit_count(mat.as_str());
        if (7..=15).contains(&n) {
            let v = mat.as_str().trim();
            let start = mat.start() + (mat.as_str().len() - mat.as_str().trim_start().len());
            out.push(Cand {
                start,
                end: start + v.len(),
                kind: Kind::Phone,
                value: v.to_string(),
            });
        }
    }
    for mat in m.btc_base58.find_iter(text) {
        out.push(Cand {
            start: mat.start(),
            end: mat.end(),
            kind: Kind::Bitcoin,
            value: mat.as_str().to_string(),
        });
    }
    for mat in m.btc_bech32.find_iter(text) {
        out.push(Cand {
            start: mat.start(),
            end: mat.end(),
            kind: Kind::Bitcoin,
            value: mat.as_str().to_string(),
        });
    }
    for mat in m.card.find_iter(text) {
        if luhn_valid(mat.as_str()) {
            out.push(Cand {
                start: mat.start(),
                end: mat.end(),
                kind: Kind::CreditCard,
                value: mat.as_str().to_string(),
            });
        }
    }

    out
}

/// Resolve overlapping candidates by kind priority: process most-specific kinds
/// first, accept a candidate only if its span doesn't overlap an already-accepted
/// one. Ties within a kind go to the earliest, then longest span.
fn resolve(mut cands: Vec<Cand>) -> Vec<Finding> {
    cands.sort_by(|a, b| {
        a.kind
            .priority()
            .cmp(&b.kind.priority())
            .then(a.start.cmp(&b.start))
            .then(b.end.cmp(&a.end))
    });

    let mut accepted: Vec<(usize, usize)> = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();
    for c in cands {
        let overlaps = accepted.iter().any(|&(s, e)| c.start < e && s < c.end);
        if overlaps {
            continue;
        }
        accepted.push((c.start, c.end));
        findings.push(Finding {
            kind: c.kind,
            value: c.value,
            offset: c.start,
            context: String::new(),
        });
    }
    findings
}

/// Build a short context snippet around `[start, end)` in `text`, `ctx` chars on
/// each side, on char boundaries, with newlines flattened and elision markers.
fn context_snippet(text: &str, start: usize, end: usize, ctx: usize) -> String {
    if ctx == 0 {
        return String::new();
    }
    // Walk `ctx` chars left of `start`.
    let mut lo = start;
    let mut left = 0;
    while lo > 0 && left < ctx {
        lo -= 1;
        while !text.is_char_boundary(lo) {
            lo -= 1;
        }
        left += 1;
    }
    // Walk `ctx` chars right of `end`.
    let mut hi = end;
    let mut right = 0;
    while hi < text.len() && right < ctx {
        hi += 1;
        while hi < text.len() && !text.is_char_boundary(hi) {
            hi += 1;
        }
        right += 1;
    }
    let mut s = String::new();
    if lo > 0 {
        s.push('…');
    }
    s.push_str(&text[lo..hi]);
    if hi < text.len() {
        s.push('…');
    }
    // Flatten whitespace runs so a snippet stays on one line.
    let flat: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    flat
}

// ---------------------------------------------------------------------------
// Public entry point.
// ---------------------------------------------------------------------------

/// Scan `text` for artifacts and render them.
///
/// - `kinds`:   `all` (default) or a comma-list of `email,url,ipv4,domain,phone,bitcoin,credit_card`.
/// - `output`:  `table` (default) | `json`.
/// - `context`: context characters on each side of a hit (0 → default 24; max 200).
/// - `limit`:   cap the number of findings (0 → default 1000; hard max 20000).
pub fn extract(
    text: &str,
    kinds: &str,
    output: &str,
    context: u32,
    limit: u32,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("input is empty — paste some text to scan for artifacts".into());
    }
    let filter = KindFilter::parse(kinds)?;
    let out = Output::parse(output)?;
    let ctx = if context == 0 {
        DEFAULT_CONTEXT
    } else {
        context.min(MAX_CONTEXT)
    } as usize;
    let limit = if limit == 0 {
        DEFAULT_LIMIT
    } else {
        limit.clamp(1, MAX_LIMIT)
    } as usize;

    let m = Matchers::new();
    let mut findings = resolve(candidates(&m, text));

    // Deterministic output order: by offset, then kind priority.
    findings.sort_by(|a, b| {
        a.offset
            .cmp(&b.offset)
            .then(a.kind.priority().cmp(&b.kind.priority()))
    });

    // Apply the kind filter, fill context, and cap.
    let mut kept: Vec<Finding> = Vec::new();
    for mut f in findings.into_iter().filter(|f| filter.keeps(f.kind)) {
        if kept.len() >= limit {
            break;
        }
        f.context = context_snippet(text, f.offset, f.offset + f.value.len(), ctx);
        kept.push(f);
    }

    match out {
        Output::Table => Ok(render_table(&kept)),
        Output::Json => render_json(&kept),
    }
}

// ---------------------------------------------------------------------------
// Rendering.
// ---------------------------------------------------------------------------

fn caption(kept: &[Finding]) -> String {
    let mut counts: Vec<(Kind, usize)> = ALL_KINDS
        .iter()
        .map(|&k| (k, kept.iter().filter(|f| f.kind == k).count()))
        .filter(|&(_, n)| n > 0)
        .collect();
    counts.sort_by_key(|(k, _)| k.priority());
    let breakdown: Vec<String> = counts
        .iter()
        .map(|(k, n)| format!("{} {}", k.label(), n))
        .collect();
    let noun = if kept.len() == 1 {
        "artifact"
    } else {
        "artifacts"
    };
    if breakdown.is_empty() {
        format!("Bulk artifact extractor · {} {noun}", kept.len())
    } else {
        format!(
            "Bulk artifact extractor · {} {noun} · {}",
            kept.len(),
            breakdown.join(" · ")
        )
    }
}

const COLS: [&str; 4] = ["kind", "value", "offset", "context"];

fn md_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', " ")
}

fn render_table(kept: &[Finding]) -> String {
    let cap = caption(kept);
    if kept.is_empty() {
        return format!("{cap}\n\n(no artifacts found)");
    }
    let mut out = String::new();
    out.push_str(&cap);
    out.push_str("\n\n| ");
    out.push_str(&COLS.join(" | "));
    out.push_str(" |\n| ");
    out.push_str(&vec!["---"; COLS.len()].join(" | "));
    out.push_str(" |\n");
    for f in kept {
        let _ = write!(
            out,
            "| {} | {} | {} | {} |\n",
            f.kind.label(),
            md_escape(&f.value),
            f.offset,
            md_escape(&f.context),
        );
    }
    out.pop();
    out
}

fn render_json(kept: &[Finding]) -> Result<String, String> {
    let mut arr = Vec::with_capacity(kept.len());
    for f in kept {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "kind".into(),
            serde_json::Value::String(f.kind.label().to_string()),
        );
        obj.insert("value".into(), serde_json::Value::String(f.value.clone()));
        obj.insert("offset".into(), serde_json::Value::Number(f.offset.into()));
        obj.insert(
            "context".into(),
            serde_json::Value::String(f.context.clone()),
        );
        arr.push(serde_json::Value::Object(obj));
    }
    serde_json::to_string_pretty(&serde_json::Value::Array(arr))
        .map_err(|e| format!("failed to serialize JSON: {e}"))
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // A blob touching every kind. 4111111111111111 is a Luhn-valid test Visa;
    // 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa is the genesis-block address.
    const BLOB: &str = "Contact alice@example.com or visit https://data.example.org/path?q=1. \
Server 203.0.113.7 hosts admin.internal.net. Call +1 415-555-0132 today. \
Card 4111 1111 1111 1111 on file. Tip: bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq or 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa.";

    fn kinds_in(json: &str) -> Vec<String> {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        v.as_array()
            .unwrap()
            .iter()
            .map(|o| o["kind"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn json_finds_every_kind_once() {
        let out = extract(BLOB, "all", "json", 0, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
        let kinds = kinds_in(&out);
        for want in ["email", "url", "ipv4", "domain", "phone", "credit_card"] {
            assert!(
                kinds.iter().any(|k| k == want),
                "missing {want} in {kinds:?}"
            );
        }
        // Two bitcoin addresses (base58 + bech32).
        assert_eq!(
            kinds.iter().filter(|k| *k == "bitcoin").count(),
            2,
            "{kinds:?}"
        );
    }

    #[test]
    fn findings_are_offset_sorted_and_values_slice_the_input() {
        let out = extract(BLOB, "all", "json", 0, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = v.as_array().unwrap();
        let mut last = 0usize;
        for o in arr {
            let off = o["offset"].as_u64().unwrap() as usize;
            let val = o["value"].as_str().unwrap();
            assert!(off >= last, "offsets not ascending: {arr:?}");
            last = off;
            // The reported byte offset must actually point at the value.
            assert_eq!(
                &BLOB[off..off + val.len()],
                val,
                "offset mismatch for {val}"
            );
        }
    }

    #[test]
    fn email_finds_the_address_and_suppresses_its_domain() {
        let out = extract("write to bob@sub.example.com please", "all", "json", 0, 0).unwrap();
        let kinds = kinds_in(&out);
        assert_eq!(
            kinds,
            vec!["email"],
            "domain inside the email must not double-report: {kinds:?}"
        );
    }

    #[test]
    fn url_suppresses_its_host_domain_and_ip() {
        let out = extract("see http://192.168.0.1/admin now", "all", "json", 0, 0).unwrap();
        let kinds = kinds_in(&out);
        assert_eq!(
            kinds,
            vec!["url"],
            "IP inside the URL must not double-report: {kinds:?}"
        );
    }

    #[test]
    fn bare_domain_is_detected_when_standalone() {
        let out = extract("the site example.co.uk is up", "domain", "json", 0, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["kind"], "domain");
        assert_eq!(v[0]["value"], "example.co.uk");
    }

    #[test]
    fn credit_card_requires_luhn() {
        // Valid Luhn 4111... is kept; the invalid neighbour is not a card.
        let ok = extract("pay 4111111111111111", "credit_card", "json", 0, 0).unwrap();
        assert_eq!(kinds_in(&ok), vec!["credit_card"]);
        let bad = extract("id 4111111111111112", "credit_card", "json", 0, 0).unwrap();
        assert_eq!(
            kinds_in(&bad).len(),
            0,
            "non-Luhn run must not be a card: {bad}"
        );
    }

    #[test]
    fn ipv4_rejects_out_of_range_octets() {
        let out = extract("bad 999.1.1.1 good 10.0.0.255", "ipv4", "json", 0, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let vals: Vec<&str> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["value"].as_str().unwrap())
            .collect();
        assert_eq!(vals, vec!["10.0.0.255"], "{vals:?}");
    }

    #[test]
    fn phone_needs_a_separator_and_digit_count() {
        let out = extract(
            "ring +44 20 7946 0958 or 020 7946 0958",
            "phone",
            "json",
            0,
            0,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let vals: Vec<&str> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["value"].as_str().unwrap())
            .collect();
        assert!(vals.iter().any(|s| s.contains("7946")), "{vals:?}");
    }

    #[test]
    fn table_has_caption_header_and_rows() {
        let out = extract("mail me at a@b.io", "all", "table", 0, 0).unwrap();
        assert!(
            out.starts_with("Bulk artifact extractor · 1 artifact · email 1"),
            "{out}"
        );
        assert!(out.contains("| kind | value | offset | context |"), "{out}");
        assert!(out.contains("| email | a@b.io | 11 |"), "{out}");
    }

    #[test]
    fn context_window_is_bounded_and_flattened() {
        let out = extract(
            "xxxxxxxxxxxxxxxxxxxx foo@bar.io yyyyyyyyyyyyyyyyyyyy",
            "email",
            "json",
            5,
            0,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let ctx = v[0]["context"].as_str().unwrap();
        assert!(ctx.contains("foo@bar.io"), "{ctx}");
        assert!(ctx.starts_with('…') && ctx.ends_with('…'), "{ctx}");
        assert!(!ctx.contains('\n'));
    }

    #[test]
    fn kinds_filter_keeps_only_requested() {
        let out = extract(BLOB, "email,ipv4", "json", 0, 0).unwrap();
        let kinds = kinds_in(&out);
        assert!(
            kinds.iter().all(|k| k == "email" || k == "ipv4"),
            "{kinds:?}"
        );
        assert!(kinds.contains(&"email".to_string()) && kinds.contains(&"ipv4".to_string()));
    }

    #[test]
    fn limit_caps_findings() {
        let out = extract("a@x.io b@y.io c@z.io", "email", "json", 0, 2).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
    }

    #[test]
    fn empty_input_errors() {
        let err = extract("   \n ", "all", "json", 0, 0).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn unknown_kind_errors() {
        let err = extract("x", "email,hash", "json", 0, 0).unwrap_err();
        assert!(err.contains("unknown kind"), "{err}");
    }

    #[test]
    fn unknown_output_errors() {
        let err = extract("x", "all", "xml", 0, 0).unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }

    #[test]
    fn no_artifacts_reports_cleanly() {
        let out = extract("just some plain words here", "all", "table", 0, 0).unwrap();
        assert!(out.contains("(no artifacts found)"), "{out}");
    }
}
