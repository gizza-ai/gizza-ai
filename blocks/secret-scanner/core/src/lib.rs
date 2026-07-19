//! secret-scanner core — pure compute, shared by the chat skill block and the web page.
//!
//! A static secret-detection scanner. It reads text line by line and flags hardcoded
//! secrets: provider API keys/tokens matched by their well-known prefix shape (AWS
//! `AKIA…`, GitHub `ghp_…`, GitLab `glpat-…`, Slack `xox…`, Stripe `sk_live_…`, Google
//! `AIza…`, OpenAI `sk-…`, Twilio, SendGrid, npm, Shopify, Square, Slack webhooks), PEM
//! private-key headers, JWT-shaped strings, and — as a lower-confidence generic pass —
//! keyword assignments (`api_key = "…"`, `password: …`) whose value has high Shannon
//! entropy and doesn't look like a placeholder. It never runs the code, never contacts a
//! provider, and never verifies whether a credential is live — it only pattern-matches, so
//! it can miss obfuscated secrets and can occasionally flag a false positive. No
//! wafer/wasm-bindgen deps.

use regex::Regex;
use serde_json::json;

/// Hard cap on input size (characters). Keeps regex scanning bounded.
pub const MAX_CHARS: usize = 200_000;

/// Severity of a finding. `High` = a value matches a known provider secret shape or is a
/// private-key header; `Medium` = a lower-confidence heuristic hit (generic keyword+entropy
/// assignment, or a JWT-shaped string that may not be a secret).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    High,
    Medium,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::High => "high",
            Severity::Medium => "medium",
        }
    }
    fn tag(self) -> &'static str {
        match self {
            Severity::High => "HIGH",
            Severity::Medium => "MEDIUM",
        }
    }
}

/// Minimum severity to include in the report.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MinSeverity {
    All,
    High,
}

impl MinSeverity {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => Ok(MinSeverity::All),
            "high" => Ok(MinSeverity::High),
            other => Err(format!(
                "invalid min_severity {other:?}: expected \"all\" or \"high\""
            )),
        }
    }
    fn keeps(self, s: Severity) -> bool {
        match self {
            MinSeverity::All => true,
            MinSeverity::High => s == Severity::High,
        }
    }
}

/// Output rendering.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => Ok(Format::Text),
            "json" => Ok(Format::Json),
            other => Err(format!("invalid format {other:?}: expected \"text\" or \"json\"")),
        }
    }
}

/// One detected secret.
pub struct Finding {
    pub line: usize,
    pub column: usize,
    pub severity: Severity,
    pub rule: &'static str,
    pub provider: &'static str,
    /// The matched value, already redacted if requested.
    pub display: String,
    /// Shannon entropy (bits/char) — set only for the generic entropy rule.
    pub entropy: Option<f64>,
}

/// Entry point: scan `text` and render the report in `format`, filtered to `min_severity`,
/// masking matched values when `redact` is true.
pub fn run(text: &str, min_severity: &str, redact: bool, format: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("no text provided".into());
    }
    let n_chars = text.chars().count();
    if n_chars > MAX_CHARS {
        return Err(format!(
            "input too large: {n_chars} characters (max {MAX_CHARS}). Scan a smaller snippet."
        ));
    }
    let min = MinSeverity::parse(min_severity)?;
    let fmt = Format::parse(format)?;

    let lines_scanned = text.lines().count().max(1);
    let findings: Vec<Finding> = scan(text, redact)
        .into_iter()
        .filter(|f| min.keeps(f.severity))
        .collect();

    Ok(match fmt {
        Format::Text => render_text(&findings, lines_scanned),
        Format::Json => render_json(&findings, lines_scanned),
    })
}

/// A named-provider / structural detector: rule id + provider label + severity + regex.
struct Detector {
    rule: &'static str,
    provider: &'static str,
    severity: Severity,
    re: Regex,
}

fn detectors() -> Vec<Detector> {
    let d = |rule, provider, severity, pat: &str| Detector {
        rule,
        provider,
        severity,
        re: Regex::new(pat).unwrap(),
    };
    vec![
        // --- private key headers (structural, HIGH) ---
        d(
            "private-key",
            "Private key (PEM)",
            Severity::High,
            r"-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP |ENCRYPTED )?PRIVATE KEY-----",
        ),
        // --- named provider prefixes (HIGH) ---
        d(
            "aws-access-key-id",
            "AWS Access Key ID",
            Severity::High,
            r"\b(?:AKIA|AGPA|AIDA|AROA|AIPA|ANPA|ANVA|ASIA)[A-Z0-9]{16}\b",
        ),
        d(
            "github-token",
            "GitHub token",
            Severity::High,
            r"\bgh[opsur]_[A-Za-z0-9]{36}\b",
        ),
        d(
            "github-fine-grained-pat",
            "GitHub fine-grained PAT",
            Severity::High,
            r"\bgithub_pat_[A-Za-z0-9_]{80,}\b",
        ),
        d(
            "gitlab-pat",
            "GitLab personal access token",
            Severity::High,
            r"\bglpat-[A-Za-z0-9_-]{20,}\b",
        ),
        d(
            "slack-token",
            "Slack token",
            Severity::High,
            r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b",
        ),
        d(
            "slack-webhook",
            "Slack incoming webhook",
            Severity::High,
            r"https://hooks\.slack\.com/services/T[A-Za-z0-9_]{8,}/B[A-Za-z0-9_]{8,}/[A-Za-z0-9_]{20,}",
        ),
        d(
            "stripe-key",
            "Stripe API key",
            Severity::High,
            r"\b[sr]k_(?:live|test)_[A-Za-z0-9]{20,}\b",
        ),
        d(
            "google-api-key",
            "Google API key",
            Severity::High,
            r"\bAIza[0-9A-Za-z_-]{35}\b",
        ),
        d(
            "openai-key",
            "OpenAI API key",
            Severity::High,
            r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}\b",
        ),
        d(
            "twilio-api-key",
            "Twilio API key SID",
            Severity::High,
            r"\bSK[0-9a-fA-F]{32}\b",
        ),
        d(
            "sendgrid-key",
            "SendGrid API key",
            Severity::High,
            r"\bSG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}\b",
        ),
        d(
            "npm-token",
            "npm access token",
            Severity::High,
            r"\bnpm_[A-Za-z0-9]{36}\b",
        ),
        d(
            "shopify-token",
            "Shopify access token",
            Severity::High,
            r"\bshp(?:at|ca|pa|ss)_[A-Za-z0-9]{32}\b",
        ),
        d(
            "square-token",
            "Square access token",
            Severity::High,
            r"\bsq0(?:atp|csp)-[A-Za-z0-9_-]{22,}\b",
        ),
        // --- JWT (structural, MEDIUM: could be a non-secret token) ---
        d(
            "jwt",
            "JSON Web Token (JWT)",
            Severity::Medium,
            r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b",
        ),
    ]
}

// Generic keyword-assignment detector (MEDIUM). Captures the value (group 1) after a
// secret-ish keyword and `=`/`:`; the value is then entropy-gated + placeholder-filtered.
// A leading `[a-z0-9_]*` lets the keyword sit at the end of a longer identifier
// (`database_secret`, `myApiKey`) — the trailing `\b` still blocks `secretary`/`tokens`.
fn generic_re() -> Regex {
    Regex::new(
        r#"(?i)\b[a-z0-9_]*(?:passwd|password|pwd|secret|token|api[_-]?key|apikey|access[_-]?key|secret[_-]?key|client[_-]?secret|auth[_-]?token|credential)\b\s*[:=]\s*["']?([A-Za-z0-9_\-+/.=]{8,})["']?"#,
    )
    .unwrap()
}

/// Shannon entropy of `s` in bits per character.
pub fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut counts = std::collections::HashMap::new();
    let mut total = 0usize;
    for c in s.chars() {
        *counts.entry(c).or_insert(0usize) += 1;
        total += 1;
    }
    let total = total as f64;
    -counts
        .values()
        .map(|&c| {
            let p = c as f64 / total;
            p * p.log2()
        })
        .sum::<f64>()
}

/// Values that are obviously placeholders, not real secrets.
fn looks_like_placeholder(v: &str) -> bool {
    let lower = v.to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "example", "your", "changeme", "change_me", "placeholder", "insert", "todo", "fixme",
        "redacted", "dummy", "sample", "xxxx", "0000", "1234", "abcd", "test_key", "my_secret",
        "somekey", "s3cr3t", "not_a", "fake",
    ];
    if NEEDLES.iter().any(|n| lower.contains(n)) {
        return true;
    }
    // A single repeated char (e.g. "aaaaaaaa", "********").
    let mut chars = v.chars();
    if let Some(first) = chars.next() {
        if chars.all(|c| c == first) {
            return true;
        }
    }
    false
}

fn col_of(line: &str, byte_start: usize) -> usize {
    line.get(..byte_start)
        .map(|p| p.chars().count() + 1)
        .unwrap_or(1)
}

/// Mask a matched secret, keeping only a short non-secret prefix.
fn mask(value: &str) -> String {
    let keep = if value.starts_with("-----BEGIN") {
        // Show the whole header line; there's no secret body in the header itself.
        return value.to_string();
    } else {
        4.min(value.chars().count())
    };
    let prefix: String = value.chars().take(keep).collect();
    format!("{prefix}…[redacted]")
}

fn scan(text: &str, redact: bool) -> Vec<Finding> {
    let dets = detectors();
    let generic = generic_re();
    let mut findings = Vec::new();

    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        // Byte ranges already claimed on this line (higher-priority matches win).
        let mut claimed: Vec<(usize, usize)> = Vec::new();

        for det in &dets {
            for m in det.re.find_iter(raw) {
                if overlaps(&claimed, m.start(), m.end()) {
                    continue;
                }
                claimed.push((m.start(), m.end()));
                let raw_val = m.as_str();
                findings.push(Finding {
                    line: line_no,
                    column: col_of(raw, m.start()),
                    severity: det.severity,
                    rule: det.rule,
                    provider: det.provider,
                    display: if redact { mask(raw_val) } else { raw_val.to_string() },
                    entropy: None,
                });
            }
        }

        // Generic keyword+entropy pass (lower priority; skip spans already claimed).
        for caps in generic.captures_iter(raw) {
            let val = caps.get(1).unwrap();
            if overlaps(&claimed, val.start(), val.end()) {
                continue;
            }
            let value = val.as_str();
            if looks_like_placeholder(value) {
                continue;
            }
            let ent = shannon_entropy(value);
            if ent < 3.0 {
                continue;
            }
            claimed.push((val.start(), val.end()));
            findings.push(Finding {
                line: line_no,
                column: col_of(raw, val.start()),
                severity: Severity::Medium,
                rule: "generic-secret-assignment",
                provider: "Generic secret assignment",
                display: if redact { mask(value) } else { value.to_string() },
                entropy: Some(ent),
            });
        }
    }

    findings.sort_by(|a, b| a.line.cmp(&b.line).then(a.column.cmp(&b.column)));
    findings
}

fn overlaps(claimed: &[(usize, usize)], start: usize, end: usize) -> bool {
    claimed.iter().any(|&(s, e)| start < e && s < end)
}

fn counts(findings: &[Finding]) -> (usize, usize) {
    let high = findings
        .iter()
        .filter(|f| f.severity == Severity::High)
        .count();
    let medium = findings.len() - high;
    (high, medium)
}

fn render_text(findings: &[Finding], lines_scanned: usize) -> String {
    if findings.is_empty() {
        return format!(
            "No hardcoded secrets found in {lines_scanned} line(s) scanned. \
             (Static heuristic — not a guarantee.)"
        );
    }
    let (high, medium) = counts(findings);
    let mut out = String::new();
    out.push_str(&format!(
        "{} finding(s) ({high} high, {medium} medium) in {lines_scanned} line(s) scanned\n\n",
        findings.len()
    ));
    for f in findings {
        out.push_str(&format!(
            "line {}, col {}  {}  {}  {}\n  {}\n\n",
            f.line,
            f.column,
            f.severity.tag(),
            f.rule,
            f.provider,
            f.display
        ));
    }
    out.push_str(
        "Recommendation: remove hardcoded secrets from source, rotate anything real that was \
         exposed, and load credentials from environment variables or a secrets manager. A clean \
         result means nothing matched, not that the code is secret-free.",
    );
    out
}

fn render_json(findings: &[Finding], lines_scanned: usize) -> String {
    let (high, medium) = counts(findings);
    let items: Vec<_> = findings
        .iter()
        .map(|f| {
            let mut o = json!({
                "line": f.line,
                "column": f.column,
                "severity": f.severity.label(),
                "rule": f.rule,
                "provider": f.provider,
                "match": f.display,
            });
            if let Some(e) = f.entropy {
                o["entropy"] = json!((e * 100.0).round() / 100.0);
            }
            o
        })
        .collect();
    let v = json!({
        "summary": {
            "findings": findings.len(),
            "high": high,
            "medium": medium,
            "lines_scanned": lines_scanned,
        },
        "findings": items,
    });
    serde_json::to_string_pretty(&v).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_aws_access_key() {
        let out = run(
            "aws_key = AKIAIOSFODNN7EXAMPLE",
            "all",
            true,
            "text",
        )
        .unwrap();
        assert!(out.contains("aws-access-key-id"), "{out}");
        assert!(out.contains("HIGH"), "{out}");
        assert!(out.contains("AKIA…[redacted]"), "{out}");
        // Redaction hides the full key.
        assert!(!out.contains("AKIAIOSFODNN7EXAMPLE"), "{out}");
    }

    #[test]
    fn redact_off_shows_full_value() {
        let out = run("token = ghp_1234567890abcdefghijklmnopqrstuvwxyz", "all", false, "text").unwrap();
        assert!(out.contains("ghp_1234567890abcdefghijklmnopqrstuvwxyz"), "{out}");
        assert!(out.contains("github-token"), "{out}");
    }

    #[test]
    fn flags_private_key_header() {
        let out = run("-----BEGIN RSA PRIVATE KEY-----", "all", true, "text").unwrap();
        assert!(out.contains("private-key"), "{out}");
        assert!(out.contains("HIGH"), "{out}");
    }

    #[test]
    fn flags_generic_keyword_entropy() {
        // A high-entropy value assigned to a secret-ish keyword → medium.
        let out = run(
            "database_secret = \"f4Kd9xQ2pLm7Zt1Rv8Nw3Bc6Yh0Ge5J\"",
            "all",
            true,
            "text",
        )
        .unwrap();
        assert!(out.contains("generic-secret-assignment"), "{out}");
        assert!(out.contains("MEDIUM"), "{out}");
    }

    #[test]
    fn placeholder_value_not_flagged() {
        let out = run("api_key = \"your_api_key_here\"", "all", true, "text").unwrap();
        assert!(out.contains("No hardcoded secrets found"), "{out}");
    }

    #[test]
    fn low_entropy_value_not_flagged() {
        // Assigned but too low-entropy / short to be a real secret.
        let out = run("password = \"aaaaaaaa\"", "all", true, "text").unwrap();
        assert!(out.contains("No hardcoded secrets found"), "{out}");
    }

    #[test]
    fn jwt_is_medium_and_hidden_by_high_filter() {
        let jwt = "token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let all = run(jwt, "all", true, "text").unwrap();
        assert!(all.contains("jwt"), "{all}");
        assert!(all.contains("MEDIUM"), "{all}");
        let high = run(jwt, "high", true, "text").unwrap();
        assert!(high.contains("No hardcoded secrets found"), "{high}");
    }

    #[test]
    fn named_wins_over_generic_on_same_line() {
        // `api_key = "<AWS key>"` must report the AWS rule once, not also generic.
        let out = run(
            "api_key = \"AKIAIOSFODNN7EXAMPLE\"",
            "all",
            true,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["findings"], 1, "{out}");
        assert_eq!(v["findings"][0]["rule"], "aws-access-key-id", "{out}");
    }

    #[test]
    fn json_output_shape() {
        let out = run("key = AKIAIOSFODNN7EXAMPLE", "all", true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["findings"], 1);
        assert_eq!(v["summary"]["high"], 1);
        assert_eq!(v["findings"][0]["severity"], "high");
        assert_eq!(v["findings"][0]["rule"], "aws-access-key-id");
        assert_eq!(v["findings"][0]["line"], 1);
        assert_eq!(v["findings"][0]["provider"], "AWS Access Key ID");
    }

    #[test]
    fn clean_input_reports_none() {
        let out = run("let x = 42; // just some code", "all", true, "text").unwrap();
        assert!(out.contains("No hardcoded secrets found"), "{out}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(run("   ", "all", true, "text").is_err());
    }

    #[test]
    fn over_cap_errors() {
        let big = "a".repeat(MAX_CHARS + 1);
        assert!(run(&big, "all", true, "text").is_err());
        // At the cap it succeeds (no findings in a wall of 'a').
        let at = "a".repeat(MAX_CHARS);
        assert!(run(&at, "all", true, "text").is_ok());
    }

    #[test]
    fn invalid_enum_errors() {
        assert!(run("SELECT 1", "all", true, "yaml").is_err());
        assert!(run("SELECT 1", "critical", true, "text").is_err());
    }

    #[test]
    fn entropy_is_reasonable() {
        // Uniform-ish high-entropy string > repeated char.
        assert!(shannon_entropy("f4Kd9xQ2pLm7Zt1Rv8Nw3Bc6Yh0Ge5J") > 4.0);
        assert!(shannon_entropy("aaaaaaaa") < 0.5);
    }
}
