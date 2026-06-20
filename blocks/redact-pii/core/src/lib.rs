//! gizza-ai/redact-pii core — detect and mask personally-identifiable information
//! (PII) in text: email addresses, phone numbers, IPv4/IPv6 addresses, credit-card
//! numbers (Luhn-validated) and US SSN-like numbers. Pure-Rust (`regex`).
//!
//! Matches from all categories are collected, de-overlapped (earliest start wins,
//! longest on ties), then replaced left-to-right so indices stay valid.

use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

/// How to replace a detected value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Replace with a typed token, e.g. `[EMAIL]`.
    Label,
    /// Replace each character with `*`.
    Mask,
}

impl Style {
    pub fn parse(s: &str) -> Result<Style, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "label" | "" => Ok(Style::Label),
            "mask" => Ok(Style::Mask),
            other => Err(format!("unknown style '{other}' (use 'label' or 'mask')")),
        }
    }
}

/// Per-category counts of redactions made.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub email: usize,
    pub phone: usize,
    pub ipv4: usize,
    pub ipv6: usize,
    pub credit_card: usize,
    pub ssn: usize,
}

/// Result of a redaction pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Redaction {
    pub redacted: String,
    pub counts: Counts,
    pub total: usize,
}

#[derive(Clone, Copy)]
enum Kind {
    Email,
    Phone,
    Ipv4,
    Ipv6,
    CreditCard,
    Ssn,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Email => "[EMAIL]",
            Kind::Phone => "[PHONE]",
            Kind::Ipv4 => "[IP]",
            Kind::Ipv6 => "[IP]",
            Kind::CreditCard => "[CREDIT_CARD]",
            Kind::Ssn => "[SSN]",
        }
    }
}

struct Patterns {
    email: Regex,
    ssn: Regex,
    ipv4: Regex,
    ipv6: Regex,
    credit_card: Regex,
    phone: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        email: Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap(),
        ssn: Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap(),
        ipv4: Regex::new(
            r"\b(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\b",
        )
        .unwrap(),
        // Full 8-group IPv6 or a `::`-compressed form.
        ipv6: Regex::new(
            r"(?:[A-Fa-f0-9]{1,4}:){7}[A-Fa-f0-9]{1,4}|(?:[A-Fa-f0-9]{1,4}:){1,7}:(?:[A-Fa-f0-9]{1,4}:?){0,6}",
        )
        .unwrap(),
        // 13-19 digit runs allowing space/dash separators (must start and end on
        // a digit); Luhn-checked below.
        credit_card: Regex::new(r"\b\d(?:[ -]?\d){12,18}\b").unwrap(),
        // Optional country code, optional area-code parens, US-style 10 digits.
        phone: Regex::new(
            r"(?:\+?\d{1,3}[\s.\-]?)?(?:\(\d{3}\)|\d{3})[\s.\-]?\d{3}[\s.\-]?\d{4}",
        )
        .unwrap(),
    })
}

fn luhn_ok(digits: &str) -> bool {
    let ds: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if ds.len() < 13 || ds.len() > 19 {
        return false;
    }
    let mut sum = 0u32;
    for (i, &d) in ds.iter().rev().enumerate() {
        if i % 2 == 1 {
            let dd = d * 2;
            sum += if dd > 9 { dd - 9 } else { dd };
        } else {
            sum += d;
        }
    }
    sum % 10 == 0
}

/// Redact PII in `text` using the given `style`.
pub fn redact(text: &str, style: Style) -> Redaction {
    let p = patterns();
    // (start, end, kind)
    let mut spans: Vec<(usize, usize, Kind)> = Vec::new();
    for m in p.email.find_iter(text) {
        spans.push((m.start(), m.end(), Kind::Email));
    }
    for m in p.ssn.find_iter(text) {
        spans.push((m.start(), m.end(), Kind::Ssn));
    }
    for m in p.ipv4.find_iter(text) {
        spans.push((m.start(), m.end(), Kind::Ipv4));
    }
    for m in p.ipv6.find_iter(text) {
        // Require at least two colons so we don't catch e.g. "a:b".
        if text[m.start()..m.end()].matches(':').count() >= 2 {
            spans.push((m.start(), m.end(), Kind::Ipv6));
        }
    }
    for m in p.credit_card.find_iter(text) {
        if luhn_ok(&text[m.start()..m.end()]) {
            spans.push((m.start(), m.end(), Kind::CreditCard));
        }
    }
    for m in p.phone.find_iter(text) {
        spans.push((m.start(), m.end(), Kind::Phone));
    }

    // De-overlap: earliest start first, longest on ties.
    spans.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut accepted: Vec<(usize, usize, Kind)> = Vec::new();
    let mut last_end = 0usize;
    for (s, e, k) in spans {
        if s >= last_end {
            accepted.push((s, e, k));
            last_end = e;
        }
    }

    let mut counts = Counts::default();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for (s, e, k) in &accepted {
        out.push_str(&text[cursor..*s]);
        match style {
            Style::Label => out.push_str(k.label()),
            Style::Mask => out.extend(std::iter::repeat('*').take(text[*s..*e].chars().count())),
        }
        cursor = *e;
        match k {
            Kind::Email => counts.email += 1,
            Kind::Phone => counts.phone += 1,
            Kind::Ipv4 => counts.ipv4 += 1,
            Kind::Ipv6 => counts.ipv6 += 1,
            Kind::CreditCard => counts.credit_card += 1,
            Kind::Ssn => counts.ssn += 1,
        }
    }
    out.push_str(&text[cursor..]);

    let total = accepted.len();
    Redaction {
        redacted: out,
        counts,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_email_and_labels() {
        let r = redact("contact me at ada@example.com please", Style::Label);
        assert_eq!(r.redacted, "contact me at [EMAIL] please");
        assert_eq!(r.counts.email, 1);
        assert_eq!(r.total, 1);
    }

    #[test]
    fn masks_style() {
        let r = redact("ada@example.com", Style::Mask);
        assert_eq!(r.redacted, "*".repeat("ada@example.com".len()));
        assert_eq!(r.counts.email, 1);
    }

    #[test]
    fn redacts_ssn_and_ipv4() {
        let r = redact("ssn 123-45-6789 host 192.168.0.1", Style::Label);
        assert_eq!(r.redacted, "ssn [SSN] host [IP]");
        assert_eq!(r.counts.ssn, 1);
        assert_eq!(r.counts.ipv4, 1);
    }

    #[test]
    fn credit_card_requires_luhn() {
        // Valid Visa test number (Luhn-valid).
        let good = redact("card 4111 1111 1111 1111 end", Style::Label);
        assert_eq!(good.counts.credit_card, 1);
        assert_eq!(good.redacted, "card [CREDIT_CARD] end");
        // Same length but Luhn-invalid -> not treated as a card.
        let bad = redact("num 4111 1111 1111 1112 end", Style::Label);
        assert_eq!(bad.counts.credit_card, 0);
    }

    #[test]
    fn redacts_phone() {
        let r = redact("call (415) 555-0132 today", Style::Label);
        assert_eq!(r.counts.phone, 1);
        assert_eq!(r.redacted, "call [PHONE] today");
    }

    #[test]
    fn ipv6() {
        let r = redact("addr 2001:0db8:85a3:0000:0000:8a2e:0370:7334 here", Style::Label);
        assert_eq!(r.counts.ipv6, 1);
        assert_eq!(r.redacted, "addr [IP] here");
    }

    #[test]
    fn clean_text_unchanged() {
        let r = redact("nothing to see here", Style::Label);
        assert_eq!(r.redacted, "nothing to see here");
        assert_eq!(r.total, 0);
    }
}
