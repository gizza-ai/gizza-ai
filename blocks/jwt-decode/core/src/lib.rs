//! gizza-ai-jwt-decode-core — pure compute to decode a compact JWT string and validate time-based claims offline.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// One validation check that was performed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

/// The structured result of decoding a token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodeResult {
    pub valid: bool,
    pub signature_present: bool,
    pub error: Option<String>,
    pub header: Value,
    pub payload: Value,
    pub checks: Vec<Check>,
}

impl DecodeResult {
    /// Render as a serde_json object.
    pub fn to_json(&self) -> Value {
        json!({
            "valid": self.valid,
            "signature_present": self.signature_present,
            "error": self.error,
            "header": self.header,
            "payload": self.payload,
            "checks": self.checks,
        })
    }
}

fn b64url_decode(label: &str, s: &str) -> Result<Vec<u8>, String> {
    let clean = s.trim().trim_end_matches('=');
    B64URL
        .decode(clean)
        .map_err(|e| format!("{label} segment is not valid base64url: {e}"))
}

/// Split a compact JWT into its header, payload, and optional signature segments.
fn split_token(token: &str) -> Result<(&str, &str, Option<&str>), String> {
    let t = token.trim();
    let parts: Vec<&str> = t.split('.').collect();
    if parts.len() == 5 {
        return Err("Encrypted JWTs (JWE) with 5 segments are not supported. Only JWS (3 segments) or unsecured JWTs (2 segments) can be decoded offline.".to_string());
    }
    if parts.len() != 2 && parts.len() != 3 {
        return Err(format!(
            "Invalid JWT format: expected 2 or 3 dot-separated segments, found {}",
            parts.len()
        ));
    }
    let header = parts[0];
    let payload = parts[1];
    let signature = if parts.len() == 3 {
        let sig = parts[2];
        if sig.is_empty() {
            None
        } else {
            Some(sig)
        }
    } else {
        None
    };
    Ok((header, payload, signature))
}

/// Pull an integer-valued claim (`exp`/`nbf`/`iat`), accepting JSON numbers.
fn claim_i64(payload: &Value, key: &str) -> Option<i64> {
    let v = payload.get(key)?;
    if let Some(i) = v.as_i64() {
        Some(i)
    } else {
        v.as_f64().map(|f| f as i64)
    }
}

/// Decodes the compact JWT and validates standard claims.
///
/// - `token`: the compact `header.payload.signature` string.
/// - `now`: the current Unix time (seconds since epoch) for time checks.
/// - `leeway`: clock-skew tolerance in seconds.
pub fn decode_jwt(token: &str, now: i64, leeway: i64) -> Result<DecodeResult, String> {
    let (h_b64, p_b64, s_b64) = split_token(token)?;
    
    let header: Value = serde_json::from_slice(&b64url_decode("Header", h_b64)?)
        .map_err(|e| format!("Header is not valid JSON: {e}"))?;
    let payload: Value = serde_json::from_slice(&b64url_decode("Payload", p_b64)?)
        .map_err(|e| format!("Payload is not valid JSON: {e}"))?;

    let signature_present = s_b64.is_some();
    let mut checks = Vec::new();

    // Check 1: Signature Presence
    checks.push(Check {
        name: "signature_present".to_string(),
        ok: true,
        detail: if signature_present {
            "signature segment is present (cryptographic verification skipped)".to_string()
        } else {
            "unsigned token / no signature segment present".to_string()
        },
    });

    // Check 2: Expiration (exp)
    if let Some(exp) = claim_i64(&payload, "exp") {
        let ok = now <= exp + leeway;
        checks.push(Check {
            name: "exp".to_string(),
            ok,
            detail: if ok {
                format!("not expired (exp {exp}, reference time {now})")
            } else {
                format!("EXPIRED (exp {exp}, reference time {now})")
            },
        });
    }

    // Check 3: Not Before (nbf)
    if let Some(nbf) = claim_i64(&payload, "nbf") {
        let ok = now + leeway >= nbf;
        checks.push(Check {
            name: "nbf".to_string(),
            ok,
            detail: if ok {
                format!("active (nbf {nbf}, reference time {now})")
            } else {
                format!("NOT YET ACTIVE (nbf {nbf}, reference time {now})")
            },
        });
    }

    // Check 4: Issued At (iat)
    if let Some(iat) = claim_i64(&payload, "iat") {
        let ok = now + leeway >= iat;
        checks.push(Check {
            name: "iat".to_string(),
            ok,
            detail: if ok {
                format!("valid issued-at time (iat {iat}, reference time {now})")
            } else {
                format!("issued in the future (iat {iat}, reference time {now})")
            },
        });
    }

    let valid = checks.iter().all(|c| c.ok);
    let error = checks.iter().find(|c| !c.ok).map(|c| c.detail.clone());

    Ok(DecodeResult {
        valid,
        signature_present,
        error,
        header,
        payload,
        checks,
    })
}

/// Fallback helper wrapper for run skill interface.
pub fn run(input: &str) -> Result<String, String> {
    let now = 0; // default/mock epoch if now is not specified.
    let leeway = 0;
    let res = decode_jwt(input, now, leeway)?;
    serde_json::to_string_pretty(&res.to_json()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_jwt(header: &str, payload: &str, signature: Option<&str>) -> String {
        let h = B64URL.encode(header.as_bytes());
        let p = B64URL.encode(payload.as_bytes());
        if let Some(s) = signature {
            format!("{h}.{p}.{s}")
        } else {
            format!("{h}.{p}")
        }
    }

    #[test]
    fn test_valid_jwt_decode() {
        let h = r#"{"alg":"HS256","typ":"JWT"}"#;
        let p = r#"{"sub":"1234567890","name":"John Doe","iat":1516239022}"#;
        let t = mock_jwt(h, p, Some("sig-xyz"));

        let res = decode_jwt(&t, 1516239022, 0).unwrap();
        assert!(res.valid);
        assert!(res.signature_present);
        assert_eq!(res.header["alg"], "HS256");
        assert_eq!(res.payload["sub"], "1234567890");
        assert_eq!(res.checks.len(), 2); // signature_present + iat
    }

    #[test]
    fn test_expired_jwt() {
        let h = r#"{"alg":"HS256"}"#;
        let p = r#"{"sub":"user","exp":1000}"#;
        let t = mock_jwt(h, p, Some("sig"));

        let res = decode_jwt(&t, 1050, 0).unwrap();
        assert!(!res.valid);
        assert_eq!(res.checks[1].name, "exp");
        assert!(!res.checks[1].ok);

        // With leeway
        let res_leeway = decode_jwt(&t, 1050, 60).unwrap();
        assert!(res_leeway.valid);
    }
}
