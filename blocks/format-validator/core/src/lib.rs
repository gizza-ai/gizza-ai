//! gizza-ai/format-validator core — decide whether a string is a well-formed
//! email, URL, IPv4/IPv6, phone number, or credit-card number (Luhn), or
//! auto-detect which of those formats it matches.
//!
//! Pure compute, no deps (JSON is hand-built, no serde in core). This is a
//! *syntax/format* validator only — it never performs any I/O (no DNS/MX,
//! reachability, or BIN lookups).
//!
//! `format`:
//!   * `auto` (default) — run every check, report the highest-priority match as
//!     the detected format plus a full per-format table.
//!   * `email | url | ipv4 | ipv6 | ip | phone | credit-card` — validate against
//!     that single format and explain the verdict.
//!
//! `output`: `text` (default, human-readable) or `json` (machine-readable).

use std::net::{Ipv4Addr, Ipv6Addr};

/// The outcome of one format check.
struct Check {
    valid: bool,
    /// Human-readable note — a positive detail when valid, the reason when not.
    note: String,
}

impl Check {
    fn ok(note: impl Into<String>) -> Self {
        Check { valid: true, note: note.into() }
    }
    fn no(note: impl Into<String>) -> Self {
        Check { valid: false, note: note.into() }
    }
}

/// The formats we can auto-detect, in detection-priority order. A value that is
/// well-formed under more than one format (e.g. an all-digit string) resolves to
/// the first entry here.
const DETECT_ORDER: [&str; 6] = ["ipv4", "ipv6", "email", "url", "credit-card", "phone"];

/// Validate `input` against `format`, rendering as `output` (`text` | `json`).
pub fn validate(input: &str, format: &str, output: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(
            "input is empty: provide a value to validate, e.g. 'user@example.com'".into(),
        );
    }

    let fmt_raw = format.trim();
    let fmt = if fmt_raw.is_empty() { "auto" } else { fmt_raw };
    let fmt = fmt.to_ascii_lowercase();

    let out_raw = output.trim();
    let out = if out_raw.is_empty() { "text" } else { out_raw };
    let out = out.to_ascii_lowercase();
    if out != "text" && out != "json" {
        return Err(format!("invalid output {output:?}: expected 'text' or 'json'"));
    }

    match fmt.as_str() {
        "auto" => Ok(render_auto(trimmed, &out)),
        "email" | "url" | "ipv4" | "ipv6" | "ip" | "phone" | "credit-card" => {
            let check = run_check(&fmt, trimmed);
            Ok(render_single(trimmed, &fmt, &check, &out))
        }
        other => Err(format!(
            "invalid format {other:?}: expected one of auto, email, url, ipv4, ipv6, ip, phone, credit-card"
        )),
    }
}

/// Dispatch a single named format check.
fn run_check(format: &str, input: &str) -> Check {
    match format {
        "email" => check_email(input),
        "url" => check_url(input),
        "ipv4" => check_ipv4(input),
        "ipv6" => check_ipv6(input),
        "ip" => check_ip(input),
        "phone" => check_phone(input),
        "credit-card" => check_credit_card(input),
        _ => Check::no("unknown format"),
    }
}

// ---------------------------------------------------------------- email

fn check_email(s: &str) -> Check {
    if s.chars().any(|c| c.is_whitespace()) {
        return Check::no("contains whitespace");
    }
    let at = s.matches('@').count();
    if at == 0 {
        return Check::no("missing '@'");
    }
    if at > 1 {
        return Check::no("more than one '@'");
    }
    let (local, domain) = s.split_once('@').unwrap();
    if local.is_empty() {
        return Check::no("empty local part (before '@')");
    }
    if local.len() > 64 {
        return Check::no("local part exceeds 64 characters");
    }
    if local.starts_with('.') || local.ends_with('.') {
        return Check::no("local part starts or ends with a dot");
    }
    if local.contains("..") {
        return Check::no("local part has consecutive dots");
    }
    const LOCAL_SPECIAL: &str = ".!#$%&'*+/=?^_`{|}~-";
    if !local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || LOCAL_SPECIAL.contains(c))
    {
        return Check::no("local part has an unsupported character");
    }
    match check_domain(domain) {
        Ok(()) => Check::ok(format!("valid email; local '{local}', domain '{domain}'")),
        Err(e) => Check::no(e),
    }
}

/// Validate an internet host name used in an email/URL. Returns Err(reason).
fn check_domain(domain: &str) -> Result<(), String> {
    if domain.is_empty() {
        return Err("empty domain (after '@')".into());
    }
    if domain.len() > 253 {
        return Err("domain exceeds 253 characters".into());
    }
    if !domain.contains('.') {
        return Err("domain has no dot (needs a TLD, e.g. example.com)".into());
    }
    let labels: Vec<&str> = domain.split('.').collect();
    for label in &labels {
        if label.is_empty() {
            return Err("domain has an empty label (consecutive or trailing dot)".into());
        }
        if label.len() > 63 {
            return Err("a domain label exceeds 63 characters".into());
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err("a domain label starts or ends with a hyphen".into());
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err("a domain label has an unsupported character".into());
        }
    }
    let tld = labels.last().unwrap();
    if tld.len() < 2 {
        return Err("the top-level domain is too short".into());
    }
    if !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("the top-level domain must be alphabetic".into());
    }
    Ok(())
}

// ---------------------------------------------------------------- url

fn check_url(s: &str) -> Check {
    if s.chars().any(|c| c.is_whitespace()) {
        return Check::no("contains whitespace");
    }
    let Some((scheme, rest)) = s.split_once("://") else {
        return Check::no("missing scheme separator '://' (e.g. https://example.com)");
    };
    if scheme.is_empty() {
        return Check::no("missing scheme before '://'");
    }
    let first = scheme.chars().next().unwrap();
    if !first.is_ascii_alphabetic() {
        return Check::no("scheme must start with a letter");
    }
    if !scheme
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
    {
        return Check::no("scheme has an unsupported character");
    }
    // Authority is up to the first '/', '?' or '#'.
    let authority_end = rest
        .find(|c| c == '/' || c == '?' || c == '#')
        .unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty() {
        return Check::no("missing host after '://'");
    }
    // Strip optional userinfo and port to isolate the host.
    let host_port = authority.rsplit('@').next().unwrap();
    let host = match host_port.rfind(':') {
        // A ':' inside a bracketed IPv6 literal is not a port separator.
        Some(i) if !host_port.starts_with('[') => &host_port[..i],
        _ => host_port,
    };
    if host.is_empty() {
        return Check::no("empty host");
    }
    let scheme_lc = scheme.to_ascii_lowercase();
    Check::ok(format!("valid URL; scheme '{scheme_lc}', host '{host}'"))
}

// ---------------------------------------------------------------- ip

fn check_ipv4(s: &str) -> Check {
    match s.parse::<Ipv4Addr>() {
        Ok(a) => {
            let scope = if a.is_private() || a.is_loopback() || a.is_link_local() {
                "private"
            } else {
                "public"
            };
            Check::ok(format!("valid IPv4 address ({scope})"))
        }
        Err(_) => Check::no("not a valid IPv4 address (need four 0-255 octets, e.g. 192.168.1.1)"),
    }
}

fn check_ipv6(s: &str) -> Check {
    match s.parse::<Ipv6Addr>() {
        Ok(a) => {
            let scope = if a.is_loopback() { "loopback" } else { "unicast/other" };
            Check::ok(format!("valid IPv6 address ({scope})"))
        }
        Err(_) => Check::no("not a valid IPv6 address (e.g. 2001:db8::1 or ::1)"),
    }
}

fn check_ip(s: &str) -> Check {
    if s.parse::<Ipv4Addr>().is_ok() {
        return Check::ok("valid IP address (IPv4)");
    }
    if s.parse::<Ipv6Addr>().is_ok() {
        return Check::ok("valid IP address (IPv6)");
    }
    Check::no("not a valid IPv4 or IPv6 address")
}

// ---------------------------------------------------------------- phone

fn check_phone(s: &str) -> Check {
    let mut digits = 0usize;
    for (i, c) in s.chars().enumerate() {
        match c {
            '0'..='9' => digits += 1,
            '+' if i == 0 => {}
            ' ' | '-' | '(' | ')' | '.' | '\t' => {}
            other => {
                return Check::no(format!("unsupported character '{other}' for a phone number"));
            }
        }
    }
    if digits < 7 {
        return Check::no(format!("only {digits} digits (a phone number needs at least 7)"));
    }
    if digits > 15 {
        return Check::no(format!("{digits} digits (E.164 allows at most 15)"));
    }
    let intl = if s.trim_start().starts_with('+') {
        "E.164 international"
    } else {
        "national/local"
    };
    Check::ok(format!("valid phone number ({digits} digits, {intl})"))
}

// ---------------------------------------------------------------- credit card

fn check_credit_card(s: &str) -> Check {
    let mut digits: Vec<u8> = Vec::new();
    for c in s.chars() {
        match c {
            '0'..='9' => digits.push(c as u8 - b'0'),
            ' ' | '-' | '\t' => {}
            other => {
                return Check::no(format!("unsupported character '{other}' for a card number"));
            }
        }
    }
    let n = digits.len();
    if n < 12 {
        return Check::no(format!("only {n} digits (card numbers are 12-19 digits)"));
    }
    if n > 19 {
        return Check::no(format!("{n} digits (card numbers are 12-19 digits)"));
    }
    if !luhn_ok(&digits) {
        return Check::no("fails the Luhn checksum");
    }
    let brand = detect_brand(&digits).unwrap_or_else(|| "unknown".to_string());
    Check::ok(format!("valid card number; Luhn OK; brand {brand}; {n} digits"))
}

/// Luhn (mod-10) check over decimal digits (rightmost is position 0).
fn luhn_ok(digits: &[u8]) -> bool {
    let mut sum = 0u32;
    for (i, &d) in digits.iter().rev().enumerate() {
        let mut v = d as u32;
        if i % 2 == 1 {
            v *= 2;
            if v > 9 {
                v -= 9;
            }
        }
        sum += v;
    }
    sum % 10 == 0
}

/// Best-effort payment-card brand by length + prefix (mirrors luhn-validate).
fn detect_brand(d: &[u8]) -> Option<String> {
    let n = d.len();
    let s: String = d.iter().map(|x| (x + b'0') as char).collect();
    let p = |k: usize| -> u32 { s[..k.min(s.len())].parse().unwrap_or(0) };
    let two = p(2);
    let four = p(4);
    let brand = if d.first() == Some(&4) && (n == 13 || n == 16 || n == 19) {
        "Visa"
    } else if (two == 34 || two == 37) && n == 15 {
        "American Express"
    } else if ((51..=55).contains(&two) || (2221..=2720).contains(&four)) && n == 16 {
        "Mastercard"
    } else if (four == 6011 || two == 65 || (644..=649).contains(&p(3))) && n == 16 {
        "Discover"
    } else if (3528..=3589).contains(&four) && n == 16 {
        "JCB"
    } else if (two == 36 || two == 38 || (300..=305).contains(&p(3))) && (n == 14 || n == 16) {
        "Diners Club"
    } else {
        return None;
    };
    Some(brand.to_string())
}

// ---------------------------------------------------------------- rendering

fn render_single(input: &str, format: &str, check: &Check, output: &str) -> String {
    if output == "json" {
        return format!(
            "{{\n  \"input\": \"{input}\",\n  \"format\": \"{format}\",\n  \"valid\": {valid},\n  \"detail\": \"{note}\"\n}}",
            input = json_escape(input),
            format = format,
            valid = check.valid,
            note = json_escape(&check.note),
        );
    }
    let verdict = if check.valid { "yes" } else { "no" };
    format!(
        "Input:    {input}\n\
         Format:   {format}\n\
         Valid:    {verdict}\n\
         Detail:   {note}",
        input = input,
        format = format,
        verdict = verdict,
        note = check.note,
    )
}

fn render_auto(input: &str, output: &str) -> String {
    // Run every detectable format; keep the checks in a stable display order.
    let checks: Vec<(&str, Check)> = [
        ("email", check_email(input)),
        ("url", check_url(input)),
        ("ipv4", check_ipv4(input)),
        ("ipv6", check_ipv6(input)),
        ("phone", check_phone(input)),
        ("credit-card", check_credit_card(input)),
    ]
    .into_iter()
    .collect();

    // Detected = first valid format in detection-priority order.
    let detected = DETECT_ORDER
        .iter()
        .find(|f| checks.iter().any(|(name, c)| name == *f && c.valid))
        .copied();

    if output == "json" {
        let mut items = Vec::new();
        for (name, c) in &checks {
            items.push(format!(
                "    {{ \"format\": \"{name}\", \"valid\": {valid}, \"detail\": \"{note}\" }}",
                name = name,
                valid = c.valid,
                note = json_escape(&c.note),
            ));
        }
        let detected_json = match detected {
            Some(d) => format!("\"{d}\""),
            None => "null".to_string(),
        };
        return format!(
            "{{\n  \"input\": \"{input}\",\n  \"detected\": {detected},\n  \"checks\": [\n{body}\n  ]\n}}",
            input = json_escape(input),
            detected = detected_json,
            body = items.join(",\n"),
        );
    }

    let detected_line = match detected {
        Some(d) => d.to_string(),
        None => "none (no supported format matched)".to_string(),
    };
    let mut lines = String::new();
    lines.push_str(&format!("Input:      {input}\n"));
    lines.push_str(&format!("Detected:   {detected_line}\n\n"));
    lines.push_str("Checks:\n");
    // Display order: detection priority, so the winner sorts to the top.
    for f in DETECT_ORDER {
        let (_, c) = checks.iter().find(|(name, _)| *name == f).unwrap();
        let mark = if c.valid { "valid  " } else { "invalid" };
        lines.push_str(&format!("  {f:<12} {mark}  {note}\n", f = f, mark = mark, note = c.note));
    }
    lines.trim_end().to_string()
}

/// Minimal JSON string escaping for the hand-built objects.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_valid() {
        let out = validate("user.name+tag@example.co.uk", "email", "text").unwrap();
        assert!(out.contains("Valid:    yes"), "{out}");
        assert!(out.contains("domain 'example.co.uk'"), "{out}");
    }

    #[test]
    fn email_invalid_no_at() {
        let out = validate("not-an-email", "email", "text").unwrap();
        assert!(out.contains("Valid:    no"), "{out}");
        assert!(out.contains("missing '@'"), "{out}");
    }

    #[test]
    fn url_valid_reports_scheme_and_host() {
        let out = validate("https://sub.example.com:8080/path?q=1", "url", "text").unwrap();
        assert!(out.contains("Valid:    yes"), "{out}");
        assert!(out.contains("scheme 'https'"), "{out}");
        assert!(out.contains("host 'sub.example.com'"), "{out}");
    }

    #[test]
    fn url_invalid_missing_scheme() {
        let out = validate("example.com/path", "url", "text").unwrap();
        assert!(out.contains("Valid:    no"), "{out}");
    }

    #[test]
    fn ipv4_and_ipv6() {
        let v4 = validate("192.168.1.1", "ipv4", "text").unwrap();
        assert!(v4.contains("Valid:    yes"), "{v4}");
        assert!(v4.contains("private"), "{v4}");
        assert!(validate("999.1.1.1", "ipv4", "text").unwrap().contains("Valid:    no"));
        assert!(validate("2001:db8::1", "ipv6", "text").unwrap().contains("Valid:    yes"));
        assert!(validate("::1", "ip", "text").unwrap().contains("Valid:    yes"));
    }

    #[test]
    fn phone_checks() {
        let ok = validate("+1 (415) 555-2671", "phone", "text").unwrap();
        assert!(ok.contains("Valid:    yes"), "{ok}");
        assert!(ok.contains("E.164"), "{ok}");
        assert!(validate("12", "phone", "text").unwrap().contains("Valid:    no"));
        assert!(validate("abc123456789", "phone", "text").unwrap().contains("Valid:    no"));
    }

    #[test]
    fn credit_card_luhn_and_brand() {
        let out = validate("4111 1111 1111 1111", "credit-card", "text").unwrap();
        assert!(out.contains("Valid:    yes"), "{out}");
        assert!(out.contains("Visa"), "{out}");
        // Same number, one digit changed → Luhn fails.
        let bad = validate("4111 1111 1111 1112", "credit-card", "text").unwrap();
        assert!(bad.contains("Valid:    no"), "{bad}");
        assert!(bad.contains("Luhn"), "{bad}");
    }

    #[test]
    fn auto_detects_email() {
        let out = validate("user@example.com", "auto", "text").unwrap();
        assert!(out.contains("Detected:   email"), "{out}");
        assert!(out.contains("email        valid"), "{out}");
    }

    #[test]
    fn auto_detects_ipv4_over_phone() {
        // A dotted IP could also parse as a phone digit-string; ipv4 wins.
        let out = validate("10.0.0.1", "auto", "text").unwrap();
        assert!(out.contains("Detected:   ipv4"), "{out}");
    }

    #[test]
    fn auto_none() {
        let out = validate("!!!", "auto", "text").unwrap();
        assert!(out.contains("Detected:   none"), "{out}");
    }

    #[test]
    fn json_single_is_valid_json() {
        let out = validate("4111 1111 1111 1111", "credit-card", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], serde_json::json!(true));
        assert_eq!(v["format"], serde_json::json!("credit-card"));
    }

    #[test]
    fn json_auto_is_valid_json() {
        let out = validate("user@example.com", "auto", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["detected"], serde_json::json!("email"));
        assert_eq!(v["checks"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn empty_and_bad_format_error() {
        assert!(validate("   ", "auto", "text").is_err());
        assert!(validate("x", "bogus", "text").is_err());
        assert!(validate("x", "email", "bogus").is_err());
    }
}
