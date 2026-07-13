//! phishing-header-inspector core — deterministic, offline heuristics for raw email headers.
//!
//! The inspector never performs DNS, HTTP, or reputation lookups. It parses the header block the
//! user pasted, compares sender identity headers, summarizes authentication results, and scores
//! structural phishing/spoofing indicators that are visible from the message headers alone.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Severity {
    Info,
    Low,
    Medium,
    High,
}

impl Severity {
    fn label(&self) -> &'static str {
        match self {
            Severity::Info => "INFO",
            Severity::Low => "LOW",
            Severity::Medium => "MEDIUM",
            Severity::High => "HIGH",
        }
    }

    fn score(&self) -> u32 {
        match self {
            Severity::Info => 0,
            Severity::Low => 10,
            Severity::Medium => 25,
            Severity::High => 40,
        }
    }
}

#[derive(Debug, Clone)]
struct Finding {
    severity: Severity,
    message: String,
}

impl Finding {
    fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
        }
    }
}

#[derive(Debug, Default)]
struct ParsedHeaders {
    values: BTreeMap<String, Vec<String>>,
}

impl ParsedHeaders {
    fn get_first(&self, name: &str) -> Option<&str> {
        self.values
            .get(&name.to_ascii_lowercase())
            .and_then(|v| v.first())
            .map(String::as_str)
    }

    fn get_all(&self, name: &str) -> Vec<&str> {
        self.values
            .get(&name.to_ascii_lowercase())
            .map(|v| v.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    fn count(&self, name: &str) -> usize {
        self.values
            .get(&name.to_ascii_lowercase())
            .map(Vec::len)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
struct AuthResults {
    spf: Option<String>,
    dkim: Option<String>,
    dmarc: Option<String>,
}

/// Inspect a pasted email header block and return a human-readable phishing risk report.
pub fn run(headers: &str, report_mode: &str, check_received: bool) -> Result<String, String> {
    let parsed = parse_headers(headers)?;
    let mode = match report_mode.trim().to_ascii_lowercase().as_str() {
        "summary" => "summary",
        "detailed" | "detail" | "" => "detailed",
        other => return Err(format!("report_mode must be 'summary' or 'detailed', got '{other}'")),
    };

    let from = parsed.get_first("from").unwrap_or("");
    let return_path = parsed.get_first("return-path").unwrap_or("");
    let reply_to = parsed.get_first("reply-to").unwrap_or("");
    let message_id = parsed.get_first("message-id").unwrap_or("");

    let from_addr = extract_email(from);
    let return_addr = extract_email(return_path);
    let reply_addr = extract_email(reply_to);
    let from_domain = from_addr.as_deref().and_then(domain_of);
    let return_domain = return_addr.as_deref().and_then(domain_of);
    let reply_domain = reply_addr.as_deref().and_then(domain_of);
    let auth = auth_results(&parsed);

    let mut findings = Vec::new();

    if from.trim().is_empty() {
        findings.push(Finding::new(Severity::High, "Missing From header."));
    } else if from_addr.is_none() {
        findings.push(Finding::new(
            Severity::Medium,
            "From header does not contain a parseable email address.",
        ));
    }

    match (&from_domain, &return_domain) {
        (Some(f), Some(r)) if f != r => findings.push(Finding::new(
            Severity::High,
            format!("From domain {f} differs from Return-Path domain {r}."),
        )),
        (Some(_), None) if !return_path.trim().is_empty() && return_path.trim() != "<>" => {
            findings.push(Finding::new(
                Severity::Medium,
                "Return-Path is present but its mailbox domain could not be parsed.",
            ));
        }
        (Some(_), None) => findings.push(Finding::new(
            Severity::Low,
            "Return-Path is missing or empty, so bounce-path alignment cannot be checked.",
        )),
        _ => {}
    }

    if let (Some(f), Some(r)) = (&from_domain, &reply_domain) {
        if f != r {
            findings.push(Finding::new(
                Severity::Medium,
                format!("Reply-To domain {r} differs from From domain {f}."),
            ));
        }
    }

    if let Some(display_domain) = display_name_domain(from) {
        if let Some(f) = &from_domain {
            if &display_domain != f {
                findings.push(Finding::new(
                    Severity::High,
                    format!(
                        "Display name mentions {display_domain} but the From address uses {f}."
                    ),
                ));
            }
        }
    }

    if parsed.get_all("authentication-results").is_empty()
        && parsed.get_all("received-spf").is_empty()
        && parsed.get_all("dkim-signature").is_empty()
    {
        findings.push(Finding::new(
            Severity::Medium,
            "No Authentication-Results, Received-SPF, or DKIM-Signature headers were found.",
        ));
    }

    add_auth_finding("SPF", &auth.spf, &mut findings);
    add_auth_finding("DKIM", &auth.dkim, &mut findings);
    add_auth_finding("DMARC", &auth.dmarc, &mut findings);

    if parsed.count("dkim-signature") > 0 && auth.dkim.is_none() {
        findings.push(Finding::new(
            Severity::Info,
            "DKIM-Signature header is present, but no dkim= result was found in Authentication-Results.",
        ));
    }

    if check_received {
        let received_count = parsed.count("received");
        if received_count == 0 {
            findings.push(Finding::new(
                Severity::Medium,
                "No Received headers were found; the delivery path cannot be traced.",
            ));
        } else if received_count == 1 {
            findings.push(Finding::new(
                Severity::Low,
                "Only one Received hop is present; forwarded or relayed mail usually has more context.",
            ));
        } else {
            findings.push(Finding::new(
                Severity::Info,
                format!("Received chain contains {received_count} hops."),
            ));
        }

        let private_hops = parsed
            .get_all("received")
            .iter()
            .filter(|v| contains_private_ip(v))
            .count();
        if private_hops > 0 {
            findings.push(Finding::new(
                Severity::Low,
                format!("Received chain includes {private_hops} private/internal IP reference(s)."),
            ));
        }
    }

    if !message_id.trim().is_empty() {
        if let (Some(mid_domain), Some(f)) = (message_id_domain(message_id), &from_domain) {
            if &mid_domain != f {
                findings.push(Finding::new(
                    Severity::Low,
                    format!("Message-ID domain {mid_domain} differs from From domain {f}."),
                ));
            }
        }
    }

    findings.sort_by_key(|f| match f.severity {
        Severity::High => 0,
        Severity::Medium => 1,
        Severity::Low => 2,
        Severity::Info => 3,
    });

    let risk_score = findings.iter().map(|f| f.severity.score()).sum::<u32>().min(100);
    let risk = match risk_score {
        0 => "MINIMAL",
        1..=34 => "LOW",
        35..=59 => "MEDIUM",
        60..=84 => "HIGH",
        _ => "CRITICAL",
    };

    let mut out = String::new();
    out.push_str(&format!("Risk: {risk} ({risk_score}/100)\n"));
    out.push_str(&format!(
        "Summary: {} high, {} medium, {} low findings\n",
        findings.iter().filter(|f| f.severity == Severity::High).count(),
        findings
            .iter()
            .filter(|f| f.severity == Severity::Medium)
            .count(),
        findings.iter().filter(|f| f.severity == Severity::Low).count()
    ));
    out.push_str(&format!(
        "From domain: {}\n",
        from_domain.as_deref().unwrap_or("unknown")
    ));
    out.push_str(&format!(
        "Return-Path domain: {}\n",
        return_domain.as_deref().unwrap_or("unknown")
    ));
    out.push_str(&format!(
        "Reply-To domain: {}\n",
        reply_domain.as_deref().unwrap_or("not present")
    ));
    out.push_str(&format!(
        "Authentication: SPF {}; DKIM {}; DMARC {}\n",
        auth.spf.as_deref().unwrap_or("not found"),
        auth.dkim.as_deref().unwrap_or("not found"),
        auth.dmarc.as_deref().unwrap_or("not found")
    ));
    out.push_str(&format!("Received hops: {}\n", parsed.count("received")));

    if mode == "summary" {
        out.push_str("Top findings:\n");
        for finding in findings.iter().take(3) {
            out.push_str(&format!("- {}: {}\n", finding.severity.label(), finding.message));
        }
        if findings.is_empty() {
            out.push_str("- INFO: No structural red flags were found in the pasted headers.\n");
        }
        return Ok(out.trim_end().to_string());
    }

    out.push_str("Findings:\n");
    if findings.is_empty() {
        out.push_str("- INFO: No structural red flags were found in the pasted headers.\n");
    } else {
        for finding in &findings {
            out.push_str(&format!("- {}: {}\n", finding.severity.label(), finding.message));
        }
    }
    out.push_str("Recommended checks:\n");
    out.push_str("- Treat this as a triage signal, not a verdict; review the message body and links separately.\n");
    out.push_str("- Compare the visible sender with a known-good address before replying or opening attachments.\n");
    out.push_str("- For production incident response, confirm SPF/DKIM/DMARC with your mail gateway logs or DNS.\n");

    Ok(out.trim_end().to_string())
}

fn parse_headers(raw: &str) -> Result<ParsedHeaders, String> {
    if raw.trim().is_empty() {
        return Err("expected raw email headers, got an empty string".into());
    }

    let mut unfolded = Vec::<String>::new();
    for line in raw.lines() {
        let trimmed_end = line.trim_end_matches(['\r', '\n']);
        if trimmed_end.trim().is_empty() {
            break;
        }
        if trimmed_end.starts_with(' ') || trimmed_end.starts_with('\t') {
            if let Some(last) = unfolded.last_mut() {
                last.push(' ');
                last.push_str(trimmed_end.trim());
            }
        } else {
            unfolded.push(trimmed_end.to_string());
        }
    }

    let mut parsed = ParsedHeaders::default();
    for line in unfolded {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let key = name.trim().to_ascii_lowercase();
        if key.is_empty()
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            continue;
        }
        parsed
            .values
            .entry(key)
            .or_default()
            .push(value.trim().to_string());
    }

    if parsed.values.is_empty() {
        return Err("expected RFC 5322-style header lines such as 'From: user@example.com'".into());
    }
    Ok(parsed)
}

fn extract_email(value: &str) -> Option<String> {
    if let (Some(start), Some(end)) = (value.find('<'), value.rfind('>')) {
        if start < end {
            let inner = value[start + 1..end].trim();
            if inner.contains('@') {
                return Some(clean_addr(inner));
            }
        }
    }

    value
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .find(|part| part.contains('@'))
        .map(clean_addr)
}

fn clean_addr(addr: &str) -> String {
    addr.trim_matches(|c: char| {
        matches!(c, '<' | '>' | '"' | '\'' | '(' | ')' | '[' | ']' | ',' | ';')
    })
    .trim()
    .to_ascii_lowercase()
}

fn domain_of(addr: &str) -> Option<String> {
    addr.rsplit_once('@').and_then(|(_, domain)| {
        let d = domain
            .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '.')
            .trim_end_matches('.')
            .to_ascii_lowercase();
        if d.contains('.') && !d.contains(' ') {
            Some(d)
        } else {
            None
        }
    })
}

fn display_name_domain(from: &str) -> Option<String> {
    let before_angle = from.split('<').next().unwrap_or(from);
    extract_email(before_angle).and_then(|addr| domain_of(&addr))
}

fn message_id_domain(message_id: &str) -> Option<String> {
    let inner = message_id
        .trim()
        .trim_matches('<')
        .trim_matches('>')
        .trim_matches('"');
    domain_of(inner)
}

fn auth_results(headers: &ParsedHeaders) -> AuthResults {
    let mut combined = headers.get_all("authentication-results").join(" ").to_ascii_lowercase();
    if let Some(received_spf) = headers.get_first("received-spf") {
        combined.push(' ');
        combined.push_str(&format!("spf={}", first_token(received_spf).to_ascii_lowercase()));
    }
    AuthResults {
        spf: find_auth_value(&combined, "spf"),
        dkim: find_auth_value(&combined, "dkim"),
        dmarc: find_auth_value(&combined, "dmarc"),
    }
}

fn first_token(value: &str) -> &str {
    value.split_whitespace().next().unwrap_or("")
}

fn find_auth_value(text: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=");
    let idx = text.find(&needle)? + needle.len();
    let value = text[idx..]
        .split(|c: char| c.is_whitespace() || c == ';' || c == ')' || c == '(')
        .next()
        .unwrap_or("")
        .trim_matches(|c: char| c == ',' || c == '.');
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn add_auth_finding(label: &str, value: &Option<String>, findings: &mut Vec<Finding>) {
    match value.as_deref() {
        Some("pass") => findings.push(Finding::new(
            Severity::Info,
            format!("{label} authentication passed."),
        )),
        Some("fail") | Some("hardfail") => findings.push(Finding::new(
            Severity::High,
            format!("{label} authentication failed."),
        )),
        Some("softfail") | Some("temperror") | Some("permerror") => findings.push(Finding::new(
            Severity::Medium,
            format!("{label} result is {}.", value.as_deref().unwrap()),
        )),
        Some("neutral") | Some("none") => findings.push(Finding::new(
            Severity::Low,
            format!("{label} result is {}.", value.as_deref().unwrap()),
        )),
        Some(other) => findings.push(Finding::new(
            Severity::Info,
            format!("{label} result is {other}."),
        )),
        None => findings.push(Finding::new(
            Severity::Low,
            format!("{label} result was not found."),
        )),
    }
}

fn contains_private_ip(value: &str) -> bool {
    let v = value.to_ascii_lowercase();
    v.contains("[10.")
        || v.contains("[192.168.")
        || v.contains("[172.16.")
        || v.contains("[172.17.")
        || v.contains("[172.18.")
        || v.contains("[172.19.")
        || v.contains("[172.2")
        || v.contains("[172.30.")
        || v.contains("[172.31.")
        || v.contains("localhost")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPOOFED: &str = "From: \"alerts@paypal.com\" <notice@evil.example>\nReturn-Path: <bounce@mailer.bad>\nReply-To: help@bad.example\nAuthentication-Results: mx.example; spf=fail smtp.mailfrom=mailer.bad; dkim=none; dmarc=fail header.from=evil.example\nReceived: from [10.0.0.4] by mx.example; Tue, 1 Jan 2026 00:00:00 +0000\nReceived: from unknown by relay.example; Tue, 1 Jan 2026 00:00:01 +0000\nMessage-ID: <abc@mailer.bad>";

    const CLEAN: &str = "From: Example Alerts <alerts@example.com>\nReturn-Path: <bounce@example.com>\nAuthentication-Results: mx.example; spf=pass smtp.mailfrom=example.com; dkim=pass header.d=example.com; dmarc=pass header.from=example.com\nReceived: from mail.example.com by mx.example; Tue, 1 Jan 2026 00:00:00 +0000\nReceived: from app.example.com by mail.example.com; Tue, 1 Jan 2026 00:00:01 +0000\nMessage-ID: <abc@example.com>";

    #[test]
    fn flags_sender_mismatch_and_failed_authentication() {
        let report = run(SPOOFED, "detailed", true).unwrap();
        assert!(report.contains("Risk: CRITICAL (100/100)"));
        assert!(report.contains("From domain evil.example differs from Return-Path domain mailer.bad."));
        assert!(report.contains("Display name mentions paypal.com but the From address uses evil.example."));
        assert!(report.contains("SPF authentication failed."));
        assert!(report.contains("DMARC authentication failed."));
    }

    #[test]
    fn summarizes_clean_authenticated_headers() {
        let report = run(CLEAN, "summary", true).unwrap();
        assert!(report.contains("Risk: MINIMAL (0/100)"));
        assert!(report.contains("Authentication: SPF pass; DKIM pass; DMARC pass"));
        assert!(report.contains("Received hops: 2"));
    }

    #[test]
    fn rejects_non_header_input() {
        let err = run("this is just body text", "detailed", true).unwrap_err();
        assert!(err.contains("expected RFC 5322-style header lines"));
    }

    #[test]
    fn validates_report_mode() {
        let err = run(CLEAN, "verbose", true).unwrap_err();
        assert!(err.contains("report_mode must be"));
    }
}
