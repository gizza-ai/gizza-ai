//! gizza-ai/jwt-claims-diff core — decode two compact JWTs offline and report
//! which claims were added, removed or changed between them.
//!
//! Both tokens are decoded WITHOUT a verification key (signatures are never
//! checked — that is `jwt-verify`'s job). Claims are compared at the top level:
//! each claim name present in either token is classified as `added` (only in
//! the second token), `removed` (only in the first) or `changed` (present in
//! both with a different value). Registered time claims (`exp`, `nbf`, `iat`)
//! carry human-readable UTC annotations, and when both tokens expire the report
//! includes the expiry delta between them. Pure-Rust; no I/O.

use chrono::DateTime;
use gizza_ai_jwt_decode_core::decode_jwt;
use serde::Serialize;
use serde_json::{json, Map, Value};

/// One claim-level difference between the two tokens.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Change {
    /// The claim name, e.g. `sub`, `exp`, `roles`.
    pub claim: String,
    /// `"added"` (only in the second/right token), `"removed"` (only in the
    /// first/left token) or `"changed"` (present in both but different).
    pub kind: String,
    /// The value in the first (left) token, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<Value>,
    /// The value in the second (right) token, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<Value>,
    /// Human-readable UTC for a numeric time claim's old value (exp/nbf/iat).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_time: Option<String>,
    /// Human-readable UTC for a numeric time claim's new value (exp/nbf/iat).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct Summary {
    added: usize,
    removed: usize,
    changed: usize,
    unchanged: usize,
}

#[derive(Debug, Clone, Serialize)]
struct DiffReport {
    equal: bool,
    /// Fraction of unchanged claims over the union of all compared claim names,
    /// as a percentage rounded to one decimal place (100.0 = identical claims).
    similarity: f64,
    summary: Summary,
    payload: Vec<Change>,
    #[serde(skip_serializing_if = "Option::is_none")]
    header: Option<Vec<Change>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiry: Option<Value>,
}

/// Registered time claims whose numeric seconds-since-epoch value is annotated.
const TIME_CLAIMS: [&str; 3] = ["exp", "nbf", "iat"];

/// Render a Unix timestamp (seconds) as `YYYY-MM-DD HH:MM:SS UTC`, or `None`
/// when the value is out of range.
fn human_time(secs: i64) -> Option<String> {
    DateTime::from_timestamp(secs, 0).map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
}

/// Extract an integer-valued claim, accepting JSON integers or whole floats.
fn claim_secs(v: &Value) -> Option<i64> {
    if let Some(i) = v.as_i64() {
        Some(i)
    } else {
        v.as_f64().map(|f| f as i64)
    }
}

/// Annotate a change for a time claim with human-readable UTC on each side.
fn annotate_times(claim: &str, mut c: Change) -> Change {
    if TIME_CLAIMS.contains(&claim) {
        if let Some(v) = &c.old {
            c.old_time = claim_secs(v).and_then(human_time);
        }
        if let Some(v) = &c.new {
            c.new_time = claim_secs(v).and_then(human_time);
        }
    }
    c
}

/// Diff the top-level claims of two decoded JSON objects. Returns the ordered
/// change list plus the count of unchanged claims (for the similarity metric).
fn diff_claims(a: &Map<String, Value>, b: &Map<String, Value>) -> (Vec<Change>, usize) {
    let mut changes = Vec::new();
    let mut unchanged = 0usize;

    // Removed / changed claims, in the first token's order.
    for (k, av) in a {
        match b.get(k) {
            None => changes.push(annotate_times(
                k,
                Change {
                    claim: k.clone(),
                    kind: "removed".into(),
                    old: Some(av.clone()),
                    new: None,
                    old_time: None,
                    new_time: None,
                },
            )),
            Some(bv) => {
                if av == bv {
                    unchanged += 1;
                } else {
                    changes.push(annotate_times(
                        k,
                        Change {
                            claim: k.clone(),
                            kind: "changed".into(),
                            old: Some(av.clone()),
                            new: Some(bv.clone()),
                            old_time: None,
                            new_time: None,
                        },
                    ));
                }
            }
        }
    }

    // Added claims (present in the second token only).
    for (k, bv) in b {
        if !a.contains_key(k) {
            changes.push(annotate_times(
                k,
                Change {
                    claim: k.clone(),
                    kind: "added".into(),
                    old: None,
                    new: Some(bv.clone()),
                    old_time: None,
                    new_time: None,
                },
            ));
        }
    }

    (changes, unchanged)
}

/// A decoded JWT reduced to its header + payload objects.
fn decode_parts(
    token: &str,
    label: &str,
) -> Result<(Map<String, Value>, Map<String, Value>), String> {
    let res =
        decode_jwt(token, 0, 0).map_err(|e| format!("the {label} JWT could not be decoded: {e}"))?;
    let as_obj = |v: Value, part: &str| -> Result<Map<String, Value>, String> {
        match v {
            Value::Object(m) => Ok(m),
            _ => Err(format!("the {label} JWT {part} is not a JSON object")),
        }
    };
    Ok((as_obj(res.header, "header")?, as_obj(res.payload, "payload")?))
}

/// Build the `expiry` block comparing the two `exp` claims, when both are
/// present and numeric.
fn expiry_delta(left: &Map<String, Value>, right: &Map<String, Value>) -> Option<Value> {
    let le = left.get("exp").and_then(claim_secs)?;
    let re = right.get("exp").and_then(claim_secs)?;
    let delta = re - le;
    let note = if delta == 0 {
        "both tokens expire at the same time".to_string()
    } else if delta > 0 {
        format!("the second token expires {} seconds later than the first", delta)
    } else {
        format!("the second token expires {} seconds earlier than the first", -delta)
    };
    Some(json!({
        "left_exp": le,
        "left_exp_utc": human_time(le),
        "right_exp": re,
        "right_exp_utc": human_time(re),
        "delta_seconds": delta,
        "note": note,
    }))
}

/// Decode two JWTs and diff their claims. Returns a JSON report string.
///
/// - `left` / `right`: the compact `header.payload.signature` tokens.
/// - `include_header`: also diff the JOSE header parameters.
/// - `indent`: output indentation in spaces (0 = minified).
pub fn diff_jwts(
    left: &str,
    right: &str,
    include_header: bool,
    indent: usize,
) -> Result<String, String> {
    if left.trim().is_empty() {
        return Err("the first (left) JWT is empty".into());
    }
    if right.trim().is_empty() {
        return Err("the second (right) JWT is empty".into());
    }

    let (lhdr, lpay) = decode_parts(left, "first (left)")?;
    let (rhdr, rpay) = decode_parts(right, "second (right)")?;

    let (payload_changes, payload_unchanged) = diff_claims(&lpay, &rpay);

    // Similarity + counts span payload (and header when included).
    let mut unchanged = payload_unchanged;
    let mut all_changes_for_counts: Vec<&Change> = payload_changes.iter().collect();

    let header_changes = if include_header {
        let (hc, hu) = diff_claims(&lhdr, &rhdr);
        unchanged += hu;
        Some(hc)
    } else {
        None
    };
    if let Some(hc) = &header_changes {
        all_changes_for_counts.extend(hc.iter());
    }

    let added = all_changes_for_counts
        .iter()
        .filter(|c| c.kind == "added")
        .count();
    let removed = all_changes_for_counts
        .iter()
        .filter(|c| c.kind == "removed")
        .count();
    let changed = all_changes_for_counts
        .iter()
        .filter(|c| c.kind == "changed")
        .count();

    // Union of compared claim names = unchanged + one entry per change.
    let union = unchanged + added + removed + changed;
    let similarity = if union == 0 {
        100.0
    } else {
        ((unchanged as f64 / union as f64) * 1000.0).round() / 10.0
    };

    let report = DiffReport {
        equal: all_changes_for_counts.is_empty(),
        similarity,
        summary: Summary {
            added,
            removed,
            changed,
            unchanged,
        },
        payload: payload_changes,
        header: header_changes,
        expiry: expiry_delta(&lpay, &rpay),
    };

    write_pretty(&report, indent)
}

fn write_pretty<T: Serialize>(value: &T, indent: usize) -> Result<String, String> {
    if indent == 0 {
        return serde_json::to_string(value).map_err(|e| format!("serialize: {e}"));
    }
    let pad = vec![b' '; indent.min(8)];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value
        .serialize(&mut ser)
        .map_err(|e| format!("serialize: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn b64url(s: &str) -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
        use base64::Engine;
        B64.encode(s.as_bytes())
    }

    /// Build a compact JWT from raw header + payload JSON strings.
    fn jwt(header: &str, payload: &str) -> String {
        format!("{}.{}.sig", b64url(header), b64url(payload))
    }

    fn report(left: &str, right: &str, include_header: bool) -> Value {
        serde_json::from_str(&diff_jwts(left, right, include_header, 0).unwrap()).unwrap()
    }

    #[test]
    fn identical_tokens_are_equal() {
        let a = jwt(r#"{"alg":"HS256"}"#, r#"{"sub":"1","role":"user"}"#);
        let r = report(&a, &a, true);
        assert_eq!(r["equal"], true);
        assert_eq!(r["similarity"], 100.0);
        assert_eq!(r["summary"]["unchanged"], 3); // alg + sub + role
        assert_eq!(r["payload"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn detects_added_removed_changed_payload_claims() {
        let l = jwt(r#"{"alg":"HS256"}"#, r#"{"sub":"1","role":"user","gone":true}"#);
        let ri = jwt(r#"{"alg":"HS256"}"#, r#"{"sub":"1","role":"admin","added":9}"#);
        let r = report(&l, &ri, false); // payload only
        assert_eq!(r["equal"], false);
        assert_eq!(r["summary"]["added"], 1);
        assert_eq!(r["summary"]["removed"], 1);
        assert_eq!(r["summary"]["changed"], 1);
        assert_eq!(r["summary"]["unchanged"], 1); // sub
        let changes = r["payload"].as_array().unwrap();
        assert!(changes.iter().any(|c| c["claim"] == "role"
            && c["kind"] == "changed"
            && c["old"] == "user"
            && c["new"] == "admin"));
        assert!(changes
            .iter()
            .any(|c| c["claim"] == "gone" && c["kind"] == "removed" && c["old"] == true));
        assert!(changes
            .iter()
            .any(|c| c["claim"] == "added" && c["kind"] == "added" && c["new"] == 9));
        // header not compared when include_header=false
        assert!(r.get("header").is_none() || r["header"].is_null());
    }

    #[test]
    fn header_alg_change_is_reported_when_included() {
        let l = jwt(r#"{"alg":"HS256","typ":"JWT"}"#, r#"{"sub":"1"}"#);
        let ri = jwt(r#"{"alg":"none","typ":"JWT"}"#, r#"{"sub":"1"}"#);
        let r = report(&l, &ri, true);
        let hdr = r["header"].as_array().unwrap();
        assert!(hdr.iter().any(|c| c["claim"] == "alg"
            && c["kind"] == "changed"
            && c["old"] == "HS256"
            && c["new"] == "none"));
        assert_eq!(r["summary"]["changed"], 1);
    }

    #[test]
    fn time_claims_get_human_readable_annotations() {
        // 1516239022 = 2018-01-18 01:30:22 UTC
        let l = jwt(r#"{"alg":"HS256"}"#, r#"{"exp":1516239022}"#);
        let ri = jwt(r#"{"alg":"HS256"}"#, r#"{"exp":1600000000}"#);
        let r = report(&l, &ri, false);
        let c = &r["payload"][0];
        assert_eq!(c["claim"], "exp");
        assert_eq!(c["kind"], "changed");
        assert_eq!(c["old_time"], "2018-01-18 01:30:22 UTC");
        assert!(c["new_time"].as_str().unwrap().ends_with("UTC"));
    }

    #[test]
    fn expiry_delta_reported_when_both_have_exp() {
        let l = jwt(r#"{"alg":"HS256"}"#, r#"{"exp":1000}"#);
        let ri = jwt(r#"{"alg":"HS256"}"#, r#"{"exp":1600}"#);
        let r = report(&l, &ri, false);
        assert_eq!(r["expiry"]["delta_seconds"], 600);
        assert_eq!(r["expiry"]["left_exp"], 1000);
        assert_eq!(r["expiry"]["right_exp"], 1600);
        assert!(r["expiry"]["note"].as_str().unwrap().contains("later"));
    }

    #[test]
    fn no_expiry_block_without_both_exp() {
        let l = jwt(r#"{"alg":"HS256"}"#, r#"{"sub":"1"}"#);
        let ri = jwt(r#"{"alg":"HS256"}"#, r#"{"exp":1600}"#);
        let r = report(&l, &ri, false);
        assert!(r.get("expiry").is_none() || r["expiry"].is_null());
    }

    #[test]
    fn similarity_is_fraction_of_unchanged() {
        // payload only: sub unchanged, role changed → 1 of 2 unique = 50.0
        let l = jwt(r#"{"alg":"HS256"}"#, r#"{"sub":"1","role":"user"}"#);
        let ri = jwt(r#"{"alg":"HS256"}"#, r#"{"sub":"1","role":"admin"}"#);
        let r = report(&l, &ri, false);
        assert_eq!(r["similarity"], 50.0);
    }

    #[test]
    fn indent_produces_pretty_output() {
        let a = jwt(r#"{"alg":"HS256"}"#, r#"{"sub":"1"}"#);
        let l = jwt(r#"{"alg":"HS256"}"#, r#"{"sub":"2"}"#);
        let out = diff_jwts(&a, &l, false, 2).unwrap();
        assert!(out.contains('\n'));
        assert!(out.contains("\"equal\": false"));
    }

    #[test]
    fn errors_on_empty_and_invalid() {
        let good = jwt(r#"{"alg":"HS256"}"#, r#"{"sub":"1"}"#);
        assert!(diff_jwts("", &good, true, 0).is_err());
        assert!(diff_jwts(&good, "", true, 0).is_err());
        assert!(diff_jwts("not-a-jwt", &good, true, 0).is_err());
        assert!(diff_jwts(&good, "a.b", true, 0).is_err());
    }

    #[test]
    fn nested_claim_change_reports_whole_claim() {
        let l = jwt(r#"{"alg":"HS256"}"#, r#"{"scope":["read"]}"#);
        let ri = jwt(r#"{"alg":"HS256"}"#, r#"{"scope":["read","write"]}"#);
        let r = report(&l, &ri, false);
        let c = &r["payload"][0];
        assert_eq!(c["claim"], "scope");
        assert_eq!(c["kind"], "changed");
        assert_eq!(c["old"], json!(["read"]));
        assert_eq!(c["new"], json!(["read", "write"]));
    }
}
