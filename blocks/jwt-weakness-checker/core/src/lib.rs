//! gizza-ai-jwt-weakness-checker-core — pure-Rust offline security audit of a
//! compact JSON Web Token (JWS). It never contacts a server: everything below
//! runs on the token bytes you paste.
//!
//! What it flags: the `alg: none` / unsecured-token trap, weak or guessable
//! HMAC secrets (dictionary attack over a built-in common-secret list plus any
//! candidates you supply), missing or expired or over-long expiry, missing
//! best-practice claims (`iss`/`aud`), missing `iat`/`typ`, `kid`-injection
//! surface, algorithm-confusion risk, sensitive data in the payload, and an
//! oversized token. Each finding carries a severity; the findings roll up into
//! a 0–100 risk score and a level.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A curated, original list of well-known example / default / weak JWT signing
/// secrets. This is NOT a copied wordlist — it is a small hand-picked set of the
/// secrets that turn up in tutorials, framework defaults, and lazy deployments.
/// Users can extend it via the `wordlist` argument.
pub const COMMON_SECRETS: &[&str] = &[
    "secret",
    "secretkey",
    "secret-key",
    "supersecret",
    "your-256-bit-secret",
    "your_jwt_secret",
    "jwt_secret",
    "jwtsecret",
    "changeme",
    "change-me",
    "password",
    "password1",
    "passw0rd",
    "admin",
    "administrator",
    "root",
    "test",
    "testing",
    "test123",
    "demo",
    "example",
    "default",
    "qwerty",
    "abc123",
    "123456",
    "12345678",
    "1234567890",
    "0000",
    "letmein",
    "welcome",
    "hello",
    "key",
    "mykey",
    "mysecret",
    "my-secret",
    "token",
    "jwt",
    "jsonwebtoken",
    "s3cr3t",
    "s3cret",
    "P@ssw0rd",
    "secretpassword",
    "shhhh",
    "iloveyou",
    "master",
    "dev",
    "development",
    "production",
    "prod",
    "app",
];

/// Severity of a single finding, ordered least → most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    /// Point weight this severity contributes to the risk score.
    fn weight(self) -> u32 {
        match self {
            Severity::Info => 0,
            Severity::Low => 8,
            Severity::Medium => 20,
            Severity::High => 40,
            Severity::Critical => 60,
        }
    }
    /// Lower-case name, matching the serde representation.
    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }
}

/// A single security finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Stable machine id (e.g. `alg_none`).
    pub id: String,
    pub severity: Severity,
    pub title: String,
    /// What was found and why it matters.
    pub detail: String,
    /// Concrete remediation advice.
    pub recommendation: String,
}

/// The full audit result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResult {
    /// The token's declared algorithm (from the header), lower-cased view kept
    /// as the raw string so callers see exactly what was sent.
    pub algorithm: String,
    pub header: Value,
    pub payload: Value,
    /// The weak secret that verified the signature, if the dictionary attack
    /// found one (HMAC tokens only).
    pub cracked_secret: Option<String>,
    /// 0–100, higher = more risk.
    pub risk_score: u32,
    /// low | medium | high | critical
    pub risk_level: String,
    pub findings: Vec<Finding>,
}

impl AuditResult {
    pub fn to_json(&self) -> Value {
        json!({
            "algorithm": self.algorithm,
            "header": self.header,
            "payload": self.payload,
            "cracked_secret": self.cracked_secret,
            "risk_score": self.risk_score,
            "risk_level": self.risk_level,
            "findings": self.findings,
        })
    }
}

fn b64url_decode(label: &str, s: &str) -> Result<Vec<u8>, String> {
    let clean = s.trim().trim_end_matches('=');
    B64URL
        .decode(clean)
        .map_err(|e| format!("{label} segment is not valid base64url: {e}"))
}

/// Split a compact JWT into header, payload, and (optional) signature segments.
fn split_token(token: &str) -> Result<(&str, &str, Option<&str>), String> {
    let t = token.trim();
    if t.is_empty() {
        return Err("No token provided. Paste a compact JWT (header.payload.signature).".into());
    }
    let parts: Vec<&str> = t.split('.').collect();
    if parts.len() == 5 {
        return Err("Encrypted JWTs (JWE) with 5 segments cannot be audited offline. Provide a signed JWS (3 segments) or an unsecured token (2 segments).".into());
    }
    if parts.len() != 2 && parts.len() != 3 {
        return Err(format!(
            "Invalid JWT format: expected 2 or 3 dot-separated segments, found {}.",
            parts.len()
        ));
    }
    let sig = if parts.len() == 3 && !parts[2].is_empty() {
        Some(parts[2])
    } else {
        None
    };
    Ok((parts[0], parts[1], sig))
}

/// Pull an integer-valued claim, accepting JSON numbers or numeric strings.
fn claim_i64(payload: &Value, key: &str) -> Option<i64> {
    match payload.get(key) {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut m = <Hmac<Sha256>>::new_from_slice(key).expect("HMAC accepts any key length");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}
fn hmac_sha384(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha384;
    let mut m = <Hmac<Sha384>>::new_from_slice(key).expect("HMAC accepts any key length");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}
fn hmac_sha512(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha512;
    let mut m = <Hmac<Sha512>>::new_from_slice(key).expect("HMAC accepts any key length");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

/// Constant-length-independent equality is not needed here (offline audit), so a
/// plain compare is fine.
fn hmac_matches(alg: &str, key: &[u8], signing_input: &[u8], sig: &[u8]) -> bool {
    let computed = match alg {
        "hs256" => hmac_sha256(key, signing_input),
        "hs384" => hmac_sha384(key, signing_input),
        "hs512" => hmac_sha512(key, signing_input),
        _ => return false,
    };
    computed == sig
}

/// Parse the `wordlist` argument: candidates separated by newlines or commas,
/// trimmed, empties dropped.
fn parse_wordlist(raw: &str) -> Vec<String> {
    raw.split(['\n', ','])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Run the full audit.
///
/// * `token` — the compact JWT.
/// * `now` — reference time (seconds since epoch) for exp/nbf/iat checks.
/// * `leeway` — clock-skew tolerance in seconds.
/// * `max_exp_days` — lifetime above which an `exp` is flagged as over-long.
/// * `wordlist` — extra candidate HMAC secrets (newline/comma separated).
pub fn audit(
    token: &str,
    now: i64,
    leeway: i64,
    max_exp_days: f64,
    wordlist: &str,
) -> Result<AuditResult, String> {
    let (h_b64, p_b64, sig_b64) = split_token(token)?;

    let header: Value = serde_json::from_slice(&b64url_decode("Header", h_b64)?)
        .map_err(|e| format!("Header is not valid JSON: {e}"))?;
    let payload: Value = serde_json::from_slice(&b64url_decode("Payload", p_b64)?)
        .map_err(|e| format!("Payload is not valid JSON: {e}"))?;

    let alg_raw = header
        .get("alg")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let alg = alg_raw.to_ascii_lowercase();

    let mut findings: Vec<Finding> = Vec::new();
    let mut cracked_secret: Option<String> = None;

    // ---- Algorithm checks ------------------------------------------------
    if alg.is_empty() {
        findings.push(Finding {
            id: "alg_missing".into(),
            severity: Severity::Medium,
            title: "Header has no 'alg' field".into(),
            detail: "The JOSE header does not declare a signing algorithm. Verifiers cannot know how to validate the signature and may fall back to insecure defaults.".into(),
            recommendation: "Always set an explicit strong algorithm (e.g. HS256, RS256, ES256).".into(),
        });
    } else if alg == "none" {
        findings.push(Finding {
            id: "alg_none".into(),
            severity: Severity::Critical,
            title: "Unsecured token: alg is 'none'".into(),
            detail: "The token declares alg:none, meaning it carries no cryptographic signature. Any party can forge a token with arbitrary claims and it will be accepted by a verifier that honours the 'none' algorithm.".into(),
            recommendation: "Reject alg:none server-side and pin the expected algorithm. Never trust the header's alg to decide how to verify.".into(),
        });
    } else if !matches!(
        alg.as_str(),
        "hs256"
            | "hs384"
            | "hs512"
            | "rs256"
            | "rs384"
            | "rs512"
            | "es256"
            | "es384"
            | "es512"
            | "ps256"
            | "ps384"
            | "ps512"
            | "eddsa"
    ) {
        findings.push(Finding {
            id: "alg_unknown".into(),
            severity: Severity::Medium,
            title: format!("Unrecognized algorithm '{alg_raw}'"),
            detail: "The declared algorithm is not a standard JWA signing algorithm. Non-standard algorithms are frequently a sign of a misconfiguration or a downgrade attempt.".into(),
            recommendation: "Use a vetted algorithm (HS256/384/512, RS256/384/512, ES256/384/512, PS256/384/512, or EdDSA).".into(),
        });
    }

    // Algorithm-confusion note for asymmetric algorithms.
    if matches!(
        alg.as_str(),
        "rs256" | "rs384" | "rs512" | "es256" | "es384" | "es512" | "ps256" | "ps384" | "ps512"
    ) {
        findings.push(Finding {
            id: "alg_confusion_surface".into(),
            severity: Severity::Info,
            title: "Asymmetric algorithm — watch for key-confusion".into(),
            detail: "This token uses an asymmetric signature. If the verifier picks the algorithm from the header instead of pinning it, an attacker can re-sign the token with HS256 using the PUBLIC key as the HMAC secret (algorithm-confusion attack).".into(),
            recommendation: "Pin the expected algorithm on the server and load only the matching key type; never derive the algorithm from the untrusted header.".into(),
        });
    }

    // Missing signature segment on a non-none token.
    if sig_b64.is_none() && alg != "none" {
        findings.push(Finding {
            id: "signature_absent".into(),
            severity: Severity::High,
            title: "No signature segment present".into(),
            detail: "The token has no signature segment even though its algorithm is not 'none'. It is effectively unsigned and trivial to tamper with.".into(),
            recommendation: "Ensure tokens are signed and that verifiers require a non-empty signature.".into(),
        });
    }

    // ---- Weak-secret dictionary attack (HMAC tokens only) ---------------
    if matches!(alg.as_str(), "hs256" | "hs384" | "hs512") {
        if let Some(sig) = sig_b64 {
            let signing_input = format!("{h_b64}.{p_b64}");
            let sig_bytes = b64url_decode("Signature", sig)?;
            let extra = parse_wordlist(wordlist);
            // Built-in list first, then user candidates.
            let candidates = COMMON_SECRETS
                .iter()
                .map(|s| s.to_string())
                .chain(extra.into_iter());
            for cand in candidates {
                if hmac_matches(&alg, cand.as_bytes(), signing_input.as_bytes(), &sig_bytes) {
                    cracked_secret = Some(cand);
                    break;
                }
            }
            if let Some(secret) = &cracked_secret {
                findings.push(Finding {
                    id: "weak_secret".into(),
                    severity: Severity::Critical,
                    title: "Weak HMAC secret — signature cracked".into(),
                    detail: format!(
                        "The token's HMAC signature was reproduced with a common/guessable secret ({} chars). Anyone who guesses this secret can forge valid tokens.",
                        secret.chars().count()
                    ),
                    recommendation: "Rotate to a high-entropy random secret of at least 32 bytes (256 bits) and store it outside source control.".into(),
                });
            }
        }
    }

    // ---- Time-claim checks ----------------------------------------------
    match claim_i64(&payload, "exp") {
        None => findings.push(Finding {
            id: "exp_missing".into(),
            severity: Severity::High,
            title: "No expiry (missing 'exp')".into(),
            detail: "The payload has no exp claim, so the token never expires. A leaked token stays valid forever.".into(),
            recommendation: "Add an exp claim with a short lifetime (minutes to hours for access tokens).".into(),
        }),
        Some(exp) => {
            if now > exp + leeway {
                findings.push(Finding {
                    id: "expired".into(),
                    severity: Severity::High,
                    title: "Token has expired".into(),
                    detail: format!("exp is {exp}; the reference time is {now}. The token is past its expiry."),
                    recommendation: "Expired tokens must be rejected. If this is a live token, issue a fresh one.".into(),
                });
            } else {
                // Over-long lifetime, measured from iat when present, else from now.
                let start = claim_i64(&payload, "iat").unwrap_or(now);
                let lifetime_secs = exp - start;
                let max_secs = (max_exp_days * 86_400.0) as i64;
                if lifetime_secs > max_secs && max_secs > 0 {
                    let days = lifetime_secs as f64 / 86_400.0;
                    findings.push(Finding {
                        id: "exp_too_long".into(),
                        severity: Severity::Medium,
                        title: "Excessively long lifetime".into(),
                        detail: format!(
                            "The token is valid for about {days:.1} days, above the {max_exp_days:.0}-day threshold. Long-lived tokens widen the window of abuse if leaked."
                        ),
                        recommendation: "Shorten the lifetime and use refresh tokens for long sessions.".into(),
                    });
                }
            }
        }
    }

    if let Some(nbf) = claim_i64(&payload, "nbf") {
        if now + leeway < nbf {
            findings.push(Finding {
                id: "not_yet_valid".into(),
                severity: Severity::Low,
                title: "Token is not yet valid (nbf in the future)".into(),
                detail: format!("nbf is {nbf}; the reference time is {now}. The token is not active yet."),
                recommendation: "Confirm the clocks on the issuer and verifier are in sync.".into(),
            });
        }
    }

    if claim_i64(&payload, "iat").is_none() {
        findings.push(Finding {
            id: "iat_missing".into(),
            severity: Severity::Info,
            title: "No issued-at time (missing 'iat')".into(),
            detail: "The payload has no iat claim, so the issue time is unknown. iat helps detect replayed or unreasonably old tokens.".into(),
            recommendation: "Add an iat claim when issuing tokens.".into(),
        });
    }

    // ---- Best-practice claim checks -------------------------------------
    if payload.get("iss").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
        findings.push(Finding {
            id: "iss_missing".into(),
            severity: Severity::Low,
            title: "No issuer claim ('iss')".into(),
            detail: "Without an iss claim, verifiers cannot confirm which service issued the token, making cross-service token confusion easier.".into(),
            recommendation: "Set and validate the iss claim.".into(),
        });
    }
    let aud_present = match payload.get("aud") {
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        _ => false,
    };
    if !aud_present {
        findings.push(Finding {
            id: "aud_missing".into(),
            severity: Severity::Low,
            title: "No audience claim ('aud')".into(),
            detail: "Without an aud claim, a token issued for one service can be replayed against another that shares the signing key.".into(),
            recommendation: "Set and validate the aud claim against the expected recipient.".into(),
        });
    }

    // ---- Header hygiene --------------------------------------------------
    if header.get("typ").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
        findings.push(Finding {
            id: "typ_missing".into(),
            severity: Severity::Info,
            title: "No 'typ' header".into(),
            detail: "The header omits typ. Setting typ (e.g. \"JWT\") helps prevent token-type confusion when multiple JOSE object types are in play.".into(),
            recommendation: "Set the typ header to the expected token type.".into(),
        });
    }
    if header.get("kid").is_some() {
        findings.push(Finding {
            id: "kid_present".into(),
            severity: Severity::Info,
            title: "Key-ID header ('kid') present".into(),
            detail: "A kid header selects the verification key. If the value flows into a file path, SQL query, or URL without validation, it becomes an injection / SSRF / path-traversal surface.".into(),
            recommendation: "Treat kid as untrusted input: look it up against an allow-list, never interpolate it into paths or queries.".into(),
        });
    }

    // ---- Sensitive data in payload --------------------------------------
    let sensitive_keys = [
        "password", "passwd", "pwd", "secret", "api_key", "apikey", "access_key",
        "private_key", "privatekey", "ssn", "credit_card", "creditcard", "card_number",
        "cvv", "pin", "token", "refresh_token",
    ];
    if let Some(obj) = payload.as_object() {
        let mut hits: Vec<String> = obj
            .keys()
            .filter(|k| sensitive_keys.contains(&k.to_ascii_lowercase().as_str()))
            .cloned()
            .collect();
        hits.sort();
        if !hits.is_empty() {
            findings.push(Finding {
                id: "sensitive_data".into(),
                severity: Severity::Medium,
                title: "Possible sensitive data in payload".into(),
                detail: format!(
                    "The payload contains claim(s) that look sensitive: {}. A JWT payload is only base64url-encoded, not encrypted — anyone holding the token can read it.",
                    hits.join(", ")
                ),
                recommendation: "Never put secrets or PII in a JWT payload. Store only opaque identifiers.".into(),
            });
        }
    }

    // ---- Oversized token -------------------------------------------------
    let token_len = token.trim().len();
    if token_len > 4096 {
        findings.push(Finding {
            id: "oversized_token".into(),
            severity: Severity::Low,
            title: "Oversized token (> 4 KB)".into(),
            detail: format!("The token is {token_len} bytes. Large tokens can exceed HTTP header/cookie limits and are often a sign of over-stuffed claims."),
            recommendation: "Trim the payload to the minimum required claims.".into(),
        });
    }

    // ---- Score + level ---------------------------------------------------
    let raw: u32 = findings.iter().map(|f| f.severity.weight()).sum();
    let risk_score = raw.min(100);
    let risk_level = if findings.iter().any(|f| f.severity == Severity::Critical) || risk_score >= 60
    {
        "critical"
    } else if risk_score >= 40 {
        "high"
    } else if risk_score >= 20 {
        "medium"
    } else {
        "low"
    }
    .to_string();

    // Order findings by severity (most severe first) for a readable report.
    findings.sort_by(|a, b| (b.severity as u8).cmp(&(a.severity as u8)));

    Ok(AuditResult {
        algorithm: if alg_raw.is_empty() {
            "(none declared)".into()
        } else {
            alg_raw
        },
        header,
        payload,
        cracked_secret,
        risk_score,
        risk_level,
        findings,
    })
}

/// Args accepted by the thin `run` wrapper (used by the fallback skill path).
#[derive(Deserialize)]
struct RunArgs {
    token: String,
    #[serde(default)]
    now: i64,
    #[serde(default = "default_leeway")]
    leeway: i64,
    #[serde(default = "default_max_exp_days")]
    max_exp_days: f64,
    #[serde(default)]
    wordlist: String,
}
fn default_leeway() -> i64 {
    0
}
fn default_max_exp_days() -> f64 {
    30.0
}

/// JSON-in / JSON-out wrapper so a caller without the typed API can drive the
/// audit. `input` is a JSON object; `now == 0` means "unknown clock".
pub fn run(input: &str) -> Result<String, String> {
    let a: RunArgs = serde_json::from_str(input)
        .map_err(|e| format!("Expected a JSON object with at least a \"token\" field: {e}"))?;
    let res = audit(&a.token, a.now, a.leeway, a.max_exp_days, &a.wordlist)?;
    serde_json::to_string_pretty(&res.to_json()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A token signed HS256 with the secret "secret":
    // header {"alg":"HS256","typ":"JWT"}, payload {"sub":"1234567890","name":"John Doe","iat":1516239022}
    const HS256_SECRET_TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.XbPfbIHMI6arZ3Y922BhjWgQzWXcXNrz0ogtVhfEd2o";

    /// HS256 signed with a strong 32-byte secret; iss/aud/exp/iat/kid all set,
    /// 15-minute lifetime. Nothing here should crack or expire at `now` below.
    const HARDENED_TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6IjIwMjYtMDgifQ.eyJpc3MiOiJodHRwczovL2F1dGguZXhhbXBsZS5jb20iLCJhdWQiOiJhcGkuZXhhbXBsZS5jb20iLCJzdWIiOiJ1LTQ3MTEiLCJpYXQiOjE3NTU4MjA4MDAsImV4cCI6MTc1NTgyMTcwMH0.L1wLdwpf6brJ6jAh7tUYzY9Yvt2JeXaC-ho7A69B_5g";

    /// HS256 signed with a company-specific secret that is NOT in the built-in
    /// list — only a user-supplied wordlist can crack it.
    const CUSTOM_SECRET_TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1MiIsImlhdCI6MTc1NTgyMDgwMCwiZXhwIjoxNzU1ODI0NDAwfQ.yogMTd2k2p_etjJEOssDDdCCjcB1HymsYNVMQzBDzok";

    #[test]
    fn cracks_weak_secret_and_flags_no_exp() {
        let r = audit(HS256_SECRET_TOKEN, 1_600_000_000, 0, 30.0, "").unwrap();
        assert_eq!(r.cracked_secret.as_deref(), Some("secret"));
        assert_eq!(r.risk_level, "critical");
        assert!(r.findings.iter().any(|f| f.id == "weak_secret"));
        assert!(r.findings.iter().any(|f| f.id == "exp_missing"));
        // Most severe first.
        assert_eq!(r.findings[0].severity, Severity::Critical);
    }

    #[test]
    fn user_wordlist_extends_dictionary() {
        // Not crackable with the built-in list alone…
        let r = audit(CUSTOM_SECRET_TOKEN, 1_755_822_000, 0, 30.0, "").unwrap();
        assert!(r.cracked_secret.is_none());
        // …but a supplied candidate list finds it. Both separators are accepted.
        let r = audit(
            CUSTOM_SECRET_TOKEN,
            1_755_822_000,
            0,
            30.0,
            "acme-prod-2026, acme-staging-2026\nacme-dev-2026",
        )
        .unwrap();
        assert_eq!(r.cracked_secret.as_deref(), Some("acme-staging-2026"));
        assert!(r.findings.iter().any(|f| f.id == "weak_secret"));
    }

    #[test]
    fn detects_alg_none() {
        // {"alg":"none","typ":"JWT"} . {"sub":"admin"} . (empty sig)
        let tok = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOiJhZG1pbiJ9.";
        let r = audit(tok, 1_600_000_000, 0, 30.0, "").unwrap();
        assert_eq!(r.algorithm, "none");
        assert!(r.findings.iter().any(|f| f.id == "alg_none"));
        assert_eq!(r.risk_level, "critical");
        assert!(r.cracked_secret.is_none());
    }

    #[test]
    fn expired_token_flagged() {
        // HS256 signed with "changeme"; exp 1600003600, audited well after.
        let tok = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJzdmMtcmVwb3J0IiwiaWF0IjoxNjAwMDAwMDAwLCJleHAiOjE2MDAwMDM2MDB9.XOZClwf6Av_fz629qASlW3wtKQqqDIOX0hx3w-tHFfw";
        let r = audit(tok, 1_700_000_000, 0, 30.0, "").unwrap();
        assert!(r.findings.iter().any(|f| f.id == "expired"));
        // An expired token is NOT also reported as over-long.
        assert!(!r.findings.iter().any(|f| f.id == "exp_too_long"));
    }

    #[test]
    fn leeway_suppresses_a_just_expired_token() {
        let tok = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJzdmMtcmVwb3J0IiwiaWF0IjoxNjAwMDAwMDAwLCJleHAiOjE2MDAwMDM2MDB9.XOZClwf6Av_fz629qASlW3wtKQqqDIOX0hx3w-tHFfw";
        // 30 s past exp, with 60 s of leeway → not flagged expired.
        let r = audit(tok, 1_600_003_630, 60, 30.0, "").unwrap();
        assert!(!r.findings.iter().any(|f| f.id == "expired"));
    }

    #[test]
    fn hardened_token_is_low_risk() {
        // Strong secret, short lifetime, iss/aud/iat/typ all present.
        let r = audit(HARDENED_TOKEN, 1_755_821_000, 0, 30.0, "").unwrap();
        assert!(r.cracked_secret.is_none());
        assert_eq!(r.risk_level, "low");
        assert_eq!(r.risk_score, 0);
        // Only the informational kid note remains.
        assert!(r.findings.iter().all(|f| f.severity == Severity::Info));
        assert!(r.findings.iter().any(|f| f.id == "kid_present"));
    }

    #[test]
    fn over_long_lifetime_flagged_and_threshold_is_configurable() {
        // iat 1755820800 → exp 1787356800 ≈ 365 days.
        let tok = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJhIiwiYXVkIjoiYiIsInN1YiI6InUxIiwiaWF0IjoxNzU1ODIwODAwLCJleHAiOjE3ODczNTY4MDAsInBhc3N3b3JkIjoicmVkYWN0ZWQifQ.M2ZffVmOkA0TjzwMOzBS_Lyk76exZGwsWHffFe8IOyg";
        let r = audit(tok, 1_755_821_000, 0, 30.0, "").unwrap();
        assert!(r.findings.iter().any(|f| f.id == "exp_too_long"));
        // Raising the threshold above the actual lifetime clears the finding…
        let r = audit(tok, 1_755_821_000, 0, 365.0, "").unwrap();
        assert!(!r.findings.iter().any(|f| f.id == "exp_too_long"));
        // …and 0 disables the check entirely.
        let r = audit(tok, 1_755_821_000, 0, 0.0, "").unwrap();
        assert!(!r.findings.iter().any(|f| f.id == "exp_too_long"));
    }

    #[test]
    fn sensitive_claim_flagged() {
        let tok = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJhIiwiYXVkIjoiYiIsInN1YiI6InUxIiwiaWF0IjoxNzU1ODIwODAwLCJleHAiOjE3ODczNTY4MDAsInBhc3N3b3JkIjoicmVkYWN0ZWQifQ.M2ZffVmOkA0TjzwMOzBS_Lyk76exZGwsWHffFe8IOyg";
        let r = audit(tok, 1_755_821_000, 0, 365.0, "").unwrap();
        let f = r
            .findings
            .iter()
            .find(|f| f.id == "sensitive_data")
            .expect("sensitive_data finding");
        assert!(f.detail.contains("password"));
        assert_eq!(f.severity, Severity::Medium);
    }

    #[test]
    fn error_on_garbage() {
        assert!(audit("not-a-jwt", 0, 0, 30.0, "").is_err());
        assert!(audit("", 0, 0, 30.0, "").is_err());
    }

    #[test]
    fn run_wrapper_json_roundtrip() {
        let input = format!(r#"{{"token":"{HS256_SECRET_TOKEN}","now":1600000000}}"#);
        let out = run(&input).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["cracked_secret"], "secret");
        assert_eq!(v["risk_level"], "critical");
    }
}
