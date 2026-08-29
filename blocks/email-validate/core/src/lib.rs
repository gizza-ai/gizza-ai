//! email-validate core — address syntax plus MX-record deliverability
//! plausibility.
//!
//! Pure compute: this crate never touches the network. It owns everything that
//! can be decided offline —
//!   1. syntax (delegated wholesale to `gizza-ai-email-validator-core`, so the
//!      RFC 5321/5322 rules, typo suggestions and warnings stay in one place);
//!   2. building the DNS-over-HTTPS request URL/headers for a chosen resolver;
//!   3. parsing a DoH JSON answer into sorted MX records;
//!   4. grading the result into a verdict + risk and rendering the report.
//!
//! The block crate (`../src/lib.rs`) supplies step 2's actual HTTP round trip
//! via `wafer-run/network`; everything either side of it is testable here with
//! captured resolver responses.
//!
//! What this deliberately does NOT do: any SMTP handshake. A `pass` means the
//! address is well-formed and its domain has somewhere to deliver to — never
//! that the mailbox exists.

use gizza_ai_email_validator_core::{validate, Validation};

/// DNS record type for a mail exchanger (RFC 1035).
const TYPE_MX: u64 = 15;
/// DNS record type for an IPv4 address.
const TYPE_A: u64 = 1;
/// DNS record type for an IPv6 address.
const TYPE_AAAA: u64 = 28;

/// Default number of MX records reported (large providers publish 5).
pub const DEFAULT_MAX_RECORDS: u32 = 10;
/// Upper bound on `max_records` — past this the output stops being readable.
pub const MAX_MAX_RECORDS: u32 = 50;

/// One mail exchanger from the domain's MX record set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MxRecord {
    /// Preference (a.k.a. priority): lower is tried first.
    pub preference: u32,
    /// Mail host, with the DNS trailing dot stripped.
    pub host: String,
    /// Record TTL in seconds, when the resolver reported one.
    pub ttl: Option<u32>,
    /// Addresses for `host`, filled in only when IP resolution was requested.
    pub ips: Vec<String>,
}

/// What the DNS side of the check produced — or why it didn't happen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DnsOutcome {
    /// False when the address failed syntax checks and no lookup was attempted.
    pub queried: bool,
    /// DNS response code from the resolver (`0` = NOERROR).
    pub rcode: Option<u32>,
    /// Mail exchangers, sorted by preference then host, capped to `max_records`.
    pub mx: Vec<MxRecord>,
    /// Total MX records the resolver returned, before the cap.
    pub mx_total: usize,
    /// RFC 7505 "null MX" (`0 .`): the domain states it accepts no mail at all.
    pub null_mx: bool,
    /// A/AAAA addresses used as the RFC 5321 §5.1 implicit MX (no MX records).
    pub a_fallback: Vec<String>,
    /// Transport/parse failure — the lookup was attempted but produced nothing
    /// usable.
    pub error: Option<String>,
}

/// A DoH JSON response, reduced to the two fields this tool reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsJson {
    /// The DNS RCODE the resolver reported.
    pub status: u32,
    /// The answer section.
    pub answers: Vec<RawAnswer>,
}

/// One entry of a DoH JSON answer section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAnswer {
    /// DNS record type number (15 = MX, 1 = A, 28 = AAAA).
    pub rtype: u64,
    /// TTL in seconds, when present.
    pub ttl: Option<u32>,
    /// Record data in the resolver's presentation form.
    pub data: String,
}

/// Resolvers this tool can talk to. Both speak the same JSON DoH form; only
/// resolvers reachable over HTTPS are usable, so this is an enum rather than a
/// free-text nameserver field.
pub const RESOLVERS: [&str; 2] = ["google", "cloudflare"];

/// Normalize a resolver choice, defaulting a blank value to `google`.
pub fn normalize_resolver(resolver: &str) -> Result<&'static str, String> {
    match resolver.trim().to_ascii_lowercase().as_str() {
        "" | "google" => Ok("google"),
        "cloudflare" => Ok("cloudflare"),
        other => Err(format!(
            "invalid resolver {other:?}: expected 'google' or 'cloudflare'"
        )),
    }
}

/// Normalize an output format, defaulting a blank value to `report`.
pub fn normalize_format(format: &str) -> Result<&'static str, String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "report" => Ok("report"),
        "summary" => Ok("summary"),
        "json" => Ok("json"),
        other => Err(format!(
            "invalid format {other:?}: expected 'report', 'summary', or 'json'"
        )),
    }
}

/// Clamp `max_records` into `1..=MAX_MAX_RECORDS`, defaulting `None`.
pub fn normalize_max_records(max_records: Option<u32>) -> u32 {
    max_records
        .unwrap_or(DEFAULT_MAX_RECORDS)
        .clamp(1, MAX_MAX_RECORDS)
}

/// The DNS-over-HTTPS JSON endpoint for `resolver`, querying `name`/`rtype`.
///
/// `name` is percent-encoded defensively even though a validated domain can
/// only contain DNS-safe characters — the caller may pass a mail host straight
/// out of a resolver answer.
pub fn doh_url(resolver: &str, name: &str, rtype: &str) -> Result<String, String> {
    let base = match normalize_resolver(resolver)? {
        "cloudflare" => "https://cloudflare-dns.com/dns-query",
        _ => "https://dns.google/resolve",
    };
    Ok(format!("{base}?name={}&type={rtype}", encode_query(name)))
}

/// Headers required by `resolver`'s JSON endpoint. Cloudflare only emits JSON
/// when the request asks for it explicitly (otherwise it answers HTTP 400).
pub fn doh_headers(resolver: &str) -> Result<Vec<(String, String)>, String> {
    let accept = match normalize_resolver(resolver)? {
        "cloudflare" => "application/dns-json",
        _ => "application/x-javascript",
    };
    Ok(vec![("accept".to_string(), accept.to_string())])
}

/// Percent-encode everything outside the unreserved set.
fn encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Parse a DoH JSON body into its status + answer section.
pub fn parse_dns_json(body: &str) -> Result<DnsJson, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| {
        format!(
            "resolver did not return JSON ({e}); expected a DNS-over-HTTPS JSON object with a \"Status\" field"
        )
    })?;
    let status = v.get("Status").and_then(|s| s.as_u64()).ok_or_else(|| {
        "resolver response has no numeric \"Status\" field — not a DNS-over-HTTPS JSON answer"
            .to_string()
    })?;
    let answers = v
        .get("Answer")
        .and_then(|a| a.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(RawAnswer {
                        rtype: item.get("type")?.as_u64()?,
                        ttl: item.get("TTL").and_then(|t| t.as_u64()).map(|t| t as u32),
                        data: item.get("data")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(DnsJson {
        status: status as u32,
        answers,
    })
}

/// Extract the MX records from a parsed answer, sorted by preference (lowest
/// first) then host, capped to `max_records`.
///
/// Returns `(records, total_before_cap, null_mx)`. A single `0 .` record is
/// RFC 7505's null MX and is reported as such rather than as a mail host.
pub fn mx_records(dns: &DnsJson, max_records: u32) -> (Vec<MxRecord>, usize, bool) {
    let mut records: Vec<MxRecord> = Vec::new();
    let mut null_mx = false;
    for answer in dns.answers.iter().filter(|a| a.rtype == TYPE_MX) {
        let Some((pref, host)) = parse_mx_data(&answer.data) else {
            continue;
        };
        if host.is_empty() {
            // "0 ." — the domain publishes that it accepts no mail.
            null_mx = true;
            continue;
        }
        records.push(MxRecord {
            preference: pref,
            host,
            ttl: answer.ttl,
            ips: Vec::new(),
        });
    }
    records.sort_by(|a, b| {
        a.preference
            .cmp(&b.preference)
            .then_with(|| a.host.cmp(&b.host))
    });
    let total = records.len();
    records.truncate(max_records as usize);
    (records, total, null_mx)
}

/// Split an MX record's presentation form (`"10 mx.example.com."`) into its
/// preference and its host with the trailing dot stripped. A bare root target
/// (`"."`) yields an empty host, which the caller reads as a null MX.
fn parse_mx_data(data: &str) -> Option<(u32, String)> {
    let mut parts = data.split_whitespace();
    let pref: u32 = parts.next()?.parse().ok()?;
    let host = parts.next()?.trim_end_matches('.').to_ascii_lowercase();
    Some((pref, host))
}

/// Extract A/AAAA addresses from a parsed answer, in resolver order.
pub fn addresses(dns: &DnsJson) -> Vec<String> {
    dns.answers
        .iter()
        .filter(|a| a.rtype == TYPE_A || a.rtype == TYPE_AAAA)
        .map(|a| a.data.clone())
        .collect()
}

/// Human-readable name for a DNS response code (RFC 1035 / RFC 2136).
pub fn rcode_name(rcode: u32) -> &'static str {
    match rcode {
        0 => "NOERROR",
        1 => "FORMERR",
        2 => "SERVFAIL",
        3 => "NXDOMAIN",
        4 => "NOTIMP",
        5 => "REFUSED",
        _ => "unknown",
    }
}

/// The graded outcome of one address check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    /// `pass`, `fail`, or `unknown` (the lookup itself failed).
    pub verdict: &'static str,
    /// `low`, `medium`, or `high`.
    pub risk: &'static str,
    /// One sentence explaining the verdict, safe to show a user verbatim.
    pub reason: String,
}

/// Grade a syntax result plus a DNS outcome into verdict + risk + reason.
///
/// Ordering matters: syntax outranks DNS (a malformed address is never
/// deliverable), an explicit null MX outranks a missing one, and a lookup that
/// could not complete is reported as `unknown` rather than guessed either way.
pub fn assess(v: &Validation, outcome: &DnsOutcome) -> Assessment {
    if !v.valid {
        return Assessment {
            verdict: "fail",
            risk: "high",
            reason: "the address is not valid syntax, so no mail server was looked up".to_string(),
        };
    }
    if !outcome.queried {
        return Assessment {
            verdict: "unknown",
            risk: "medium",
            reason: "no MX lookup was performed".to_string(),
        };
    }
    if let Some(err) = &outcome.error {
        return Assessment {
            verdict: "unknown",
            risk: "medium",
            reason: format!("the MX lookup could not be completed: {err}"),
        };
    }
    let rcode = outcome.rcode.unwrap_or(0);
    if rcode == 3 {
        return Assessment {
            verdict: "fail",
            risk: "high",
            reason: "the domain does not exist (NXDOMAIN), so mail to it cannot be delivered"
                .to_string(),
        };
    }
    if rcode != 0 {
        return Assessment {
            verdict: "unknown",
            risk: "medium",
            reason: format!(
                "the resolver answered {} ({rcode}) instead of NOERROR, so the domain's mail setup could not be read",
                rcode_name(rcode)
            ),
        };
    }
    if outcome.null_mx {
        return Assessment {
            verdict: "fail",
            risk: "high",
            reason:
                "the domain publishes a null MX record (RFC 7505), which states it accepts no mail"
                    .to_string(),
        };
    }
    if !outcome.mx.is_empty() {
        let (risk, reason) = match &v.suggestion {
            Some(s) => (
                "medium",
                format!(
                    "the domain publishes {} mail exchanger(s), but {s:?} looks like the intended address",
                    outcome.mx_total
                ),
            ),
            None => (
                "low",
                format!(
                    "the domain publishes {} mail exchanger(s), so it can accept mail",
                    outcome.mx_total
                ),
            ),
        };
        return Assessment {
            verdict: "pass",
            risk,
            reason,
        };
    }
    if !outcome.a_fallback.is_empty() {
        return Assessment {
            verdict: "pass",
            risk: "medium",
            reason: format!(
                "the domain has no MX record but does have an address record, so RFC 5321 §5.1 treats {} as an implicit mail exchanger",
                outcome.a_fallback[0]
            ),
        };
    }
    Assessment {
        verdict: "fail",
        risk: "high",
        reason: "the domain has neither an MX record nor an address record, so there is nowhere to deliver mail".to_string(),
    }
}

/// Validate `email`'s syntax without any lookup. Re-exported so the block crate
/// can decide whether a DNS round trip is worth making.
pub fn syntax(email: &str) -> Validation {
    validate(email)
}

/// Render the full result in `format` (`report`, `summary` or `json`).
///
/// `resolver` is echoed so the output records which resolver answered.
pub fn render(
    email: &str,
    resolver: &str,
    outcome: &DnsOutcome,
    format: &str,
) -> Result<String, String> {
    let fmt = normalize_format(format)?;
    let resolver = normalize_resolver(resolver)?;
    let v = syntax(email);
    let a = assess(&v, outcome);

    match fmt {
        "summary" => Ok(render_summary(&v, outcome, &a)),
        "json" => Ok(render_json(&v, resolver, outcome, &a)),
        _ => Ok(render_report(&v, resolver, outcome, &a)),
    }
}

/// The caveat printed on every human-readable result: this tool never probes a
/// mailbox, and users should not read `pass` as "this mailbox exists".
pub const SMTP_CAVEAT: &str = "No SMTP handshake was made: a pass means the address is well-formed and its domain has somewhere to deliver mail, not that the mailbox exists.";

fn render_summary(v: &Validation, outcome: &DnsOutcome, a: &Assessment) -> String {
    let primary = outcome
        .mx
        .first()
        .map(|r| r.host.clone())
        .or_else(|| outcome.a_fallback.first().cloned())
        .unwrap_or_else(|| "none".to_string());
    format!(
        "{}: {} (syntax: {}, mx: {}, primary: {primary}, risk: {})",
        a.verdict,
        v.normalized,
        if v.valid { "valid" } else { "invalid" },
        outcome.mx_total,
        a.risk,
    )
}

fn render_report(v: &Validation, resolver: &str, outcome: &DnsOutcome, a: &Assessment) -> String {
    let mut out = String::new();
    out.push_str(&format!("Email: {}\n", v.normalized));
    out.push_str(&format!(
        "Syntax: {}\n",
        if v.valid { "valid" } else { "invalid" }
    ));
    if let Some(local) = &v.local {
        out.push_str(&format!("Local part: {local}\n"));
    }
    if let Some(domain) = &v.domain {
        out.push_str(&format!("Domain: {domain}\n"));
    }

    if !outcome.queried {
        out.push_str("MX lookup: skipped (the address is not valid syntax)\n");
    } else {
        out.push_str(&format!(
            "Resolver: {resolver} (DNS over HTTPS)\nDNS status: {}\n",
            match &outcome.error {
                Some(e) => format!("lookup failed — {e}"),
                None => {
                    let rcode = outcome.rcode.unwrap_or(0);
                    format!("{} ({rcode})", rcode_name(rcode))
                }
            }
        ));
        if outcome.null_mx {
            out.push_str("MX records: null MX (0 .) — the domain accepts no mail\n");
        } else if outcome.mx.is_empty() {
            out.push_str("MX records: none\n");
        } else {
            out.push_str(&format!(
                "MX records: {} (showing {})\n",
                outcome.mx_total,
                outcome.mx.len()
            ));
            for r in &outcome.mx {
                let ttl = match r.ttl {
                    Some(t) => format!(" ttl {t}s"),
                    None => String::new(),
                };
                let ips = if r.ips.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", r.ips.join(", "))
                };
                out.push_str(&format!("  {:<5} {}{ttl}{ips}\n", r.preference, r.host));
            }
            out.push_str(&format!(
                "Primary mail host: {} (preference {}) — lower preference is tried first, the rest are fallbacks\n",
                outcome.mx[0].host, outcome.mx[0].preference
            ));
        }
        if !outcome.a_fallback.is_empty() {
            out.push_str(&format!(
                "Implicit MX (RFC 5321 §5.1): {}\n",
                outcome.a_fallback.join(", ")
            ));
        }
    }

    out.push_str(&format!("Deliverability: {}\n", a.reason));
    out.push_str(&format!("Risk: {}\n", a.risk));
    out.push_str(&format!("Verdict: {}\n", a.verdict));

    if !v.errors.is_empty() {
        out.push_str("Errors:\n");
        for e in &v.errors {
            out.push_str(&format!("- {e}\n"));
        }
    }
    if !v.warnings.is_empty() {
        out.push_str("Warnings:\n");
        for w in &v.warnings {
            out.push_str(&format!("- {w}\n"));
        }
    }
    if let Some(s) = &v.suggestion {
        out.push_str(&format!("Suggestion: {s}\n"));
    }
    out.push_str(SMTP_CAVEAT);
    out
}

fn render_json(v: &Validation, resolver: &str, outcome: &DnsOutcome, a: &Assessment) -> String {
    let records: Vec<serde_json::Value> = outcome
        .mx
        .iter()
        .map(|r| {
            serde_json::json!({
                "preference": r.preference,
                "host": r.host,
                "ttl": r.ttl,
                "ips": r.ips,
            })
        })
        .collect();
    serde_json::json!({
        "email": v.normalized,
        "valid_syntax": v.valid,
        "local": v.local,
        "domain": v.domain,
        "resolver": resolver,
        "queried": outcome.queried,
        "dns_status": outcome.rcode.map(rcode_name),
        "dns_rcode": outcome.rcode,
        "lookup_error": outcome.error,
        "null_mx": outcome.null_mx,
        "mx_count": outcome.mx_total,
        "mx": records,
        "implicit_mx": outcome.a_fallback,
        "verdict": a.verdict,
        "risk": a.risk,
        "reason": a.reason,
        "errors": v.errors,
        "warnings": v.warnings,
        "suggestion": v.suggestion,
        "smtp_probe": false,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured verbatim from a `dns.google/resolve?name=gmail.com&type=MX`
    /// answer so the parser is exercised against the real wire shape without a
    /// live lookup.
    const GMAIL_MX_JSON: &str = r#"{"Status":0,"TC":false,"RD":true,"RA":true,"AD":false,"CD":false,
      "Question":[{"name":"gmail.com.","type":15}],
      "Answer":[
        {"name":"gmail.com.","type":15,"TTL":1411,"data":"5 gmail-smtp-in.l.google.com."},
        {"name":"gmail.com.","type":15,"TTL":1411,"data":"40 alt4.gmail-smtp-in.l.google.com."},
        {"name":"gmail.com.","type":15,"TTL":1411,"data":"30 alt3.gmail-smtp-in.l.google.com."},
        {"name":"gmail.com.","type":15,"TTL":1411,"data":"20 alt2.gmail-smtp-in.l.google.com."},
        {"name":"gmail.com.","type":15,"TTL":1411,"data":"10 alt1.gmail-smtp-in.l.google.com."}
      ]}"#;

    const NULL_MX_JSON: &str =
        r#"{"Status":0,"Answer":[{"name":"example.com.","type":15,"TTL":300,"data":"0 ."}]}"#;

    const NXDOMAIN_JSON: &str = r#"{"Status":3,"Authority":[{"name":"com.","type":6,"TTL":900,"data":"a.gtld-servers.net. nstld.verisign-grs.com. 1 1800 900 604800 86400"}]}"#;

    const A_ONLY_JSON: &str =
        r#"{"Status":0,"Answer":[{"name":"h.example.","type":1,"TTL":60,"data":"192.0.2.7"}]}"#;

    fn gmail_outcome() -> DnsOutcome {
        let dns = parse_dns_json(GMAIL_MX_JSON).unwrap();
        let (mx, total, null_mx) = mx_records(&dns, DEFAULT_MAX_RECORDS);
        DnsOutcome {
            queried: true,
            rcode: Some(dns.status),
            mx,
            mx_total: total,
            null_mx,
            ..Default::default()
        }
    }

    // ---- parsing -----------------------------------------------------------

    #[test]
    fn parses_mx_answer_sorted_by_preference() {
        let dns = parse_dns_json(GMAIL_MX_JSON).unwrap();
        assert_eq!(dns.status, 0);
        let (mx, total, null_mx) = mx_records(&dns, DEFAULT_MAX_RECORDS);
        assert!(!null_mx);
        assert_eq!(total, 5);
        assert_eq!(mx[0].preference, 5);
        assert_eq!(mx[0].host, "gmail-smtp-in.l.google.com");
        assert_eq!(mx[0].ttl, Some(1411));
        assert!(mx[0].ips.is_empty());
        let prefs: Vec<u32> = mx.iter().map(|r| r.preference).collect();
        assert_eq!(prefs, vec![5, 10, 20, 30, 40], "sorted ascending");
    }

    #[test]
    fn caps_records_but_reports_the_true_total() {
        let dns = parse_dns_json(GMAIL_MX_JSON).unwrap();
        let (mx, total, _) = mx_records(&dns, 2);
        assert_eq!(mx.len(), 2, "capped");
        assert_eq!(total, 5, "total is pre-cap");
        assert_eq!(mx[1].preference, 10, "the cap keeps the lowest preferences");
    }

    #[test]
    fn detects_rfc7505_null_mx() {
        let dns = parse_dns_json(NULL_MX_JSON).unwrap();
        let (mx, total, null_mx) = mx_records(&dns, DEFAULT_MAX_RECORDS);
        assert!(null_mx);
        assert!(mx.is_empty());
        assert_eq!(total, 0);
    }

    #[test]
    fn nxdomain_answer_has_no_records() {
        let dns = parse_dns_json(NXDOMAIN_JSON).unwrap();
        assert_eq!(dns.status, 3);
        assert_eq!(rcode_name(dns.status), "NXDOMAIN");
        let (mx, total, null_mx) = mx_records(&dns, DEFAULT_MAX_RECORDS);
        assert!(mx.is_empty() && total == 0 && !null_mx);
    }

    #[test]
    fn extracts_addresses_for_the_implicit_mx_fallback() {
        let dns = parse_dns_json(A_ONLY_JSON).unwrap();
        assert_eq!(addresses(&dns), vec!["192.0.2.7".to_string()]);
        let (mx, _, _) = mx_records(&dns, DEFAULT_MAX_RECORDS);
        assert!(mx.is_empty(), "an A record is not an MX record");
    }

    #[test]
    fn rejects_a_non_json_resolver_body() {
        let err = parse_dns_json("<html>rate limited</html>").unwrap_err();
        assert!(err.contains("did not return JSON"), "{err}");
    }

    #[test]
    fn rejects_json_without_a_status_field() {
        let err = parse_dns_json(r#"{"Answer":[]}"#).unwrap_err();
        assert!(err.contains("\"Status\""), "{err}");
    }

    #[test]
    fn ignores_unparseable_mx_data_instead_of_failing() {
        let dns = parse_dns_json(r#"{"Status":0,"Answer":[{"type":15,"TTL":1,"data":"garbage"}]}"#)
            .unwrap();
        let (mx, total, _) = mx_records(&dns, DEFAULT_MAX_RECORDS);
        assert!(mx.is_empty() && total == 0);
    }

    // ---- request building --------------------------------------------------

    #[test]
    fn builds_resolver_urls_and_headers() {
        assert_eq!(
            doh_url("google", "gmail.com", "MX").unwrap(),
            "https://dns.google/resolve?name=gmail.com&type=MX"
        );
        assert_eq!(
            doh_url("Cloudflare", "gmail.com", "MX").unwrap(),
            "https://cloudflare-dns.com/dns-query?name=gmail.com&type=MX"
        );
        assert_eq!(
            doh_headers("cloudflare").unwrap(),
            vec![("accept".to_string(), "application/dns-json".to_string())],
            "cloudflare answers HTTP 400 without this accept header"
        );
    }

    #[test]
    fn percent_encodes_the_query_name() {
        assert_eq!(
            doh_url("google", "a b&type=A", "MX").unwrap(),
            "https://dns.google/resolve?name=a%20b%26type%3DA&type=MX"
        );
    }

    #[test]
    fn rejects_an_unknown_resolver() {
        let err = normalize_resolver("quad9").unwrap_err();
        assert!(err.contains("invalid resolver"), "{err}");
        assert!(err.contains("'google' or 'cloudflare'"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_format() {
        let err = normalize_format("xml").unwrap_err();
        assert!(err.contains("invalid format"), "{err}");
        assert!(err.contains("'report', 'summary', or 'json'"), "{err}");
    }

    #[test]
    fn clamps_max_records() {
        assert_eq!(normalize_max_records(None), DEFAULT_MAX_RECORDS);
        assert_eq!(normalize_max_records(Some(0)), 1);
        assert_eq!(normalize_max_records(Some(999)), MAX_MAX_RECORDS);
        assert_eq!(normalize_max_records(Some(3)), 3);
    }

    // ---- grading -----------------------------------------------------------

    #[test]
    fn a_domain_with_mx_records_passes_at_low_risk() {
        let a = assess(&syntax("ada@gmail.com"), &gmail_outcome());
        assert_eq!((a.verdict, a.risk), ("pass", "low"));
        assert!(a.reason.contains("5 mail exchanger"), "{}", a.reason);
    }

    #[test]
    fn a_typo_domain_with_mx_records_passes_at_medium_risk() {
        // Syntax is fine but the domain looks misspelled — deliverable, yet
        // worth a human look.
        let a = assess(&syntax("ada@gmial.com"), &gmail_outcome());
        assert_eq!((a.verdict, a.risk), ("pass", "medium"));
        assert!(a.reason.contains("ada@gmail.com"), "{}", a.reason);
    }

    #[test]
    fn bad_syntax_fails_without_a_lookup() {
        let a = assess(&syntax("not-an-address"), &DnsOutcome::default());
        assert_eq!((a.verdict, a.risk), ("fail", "high"));
        assert!(a.reason.contains("not valid syntax"), "{}", a.reason);
    }

    #[test]
    fn null_mx_fails() {
        let dns = parse_dns_json(NULL_MX_JSON).unwrap();
        let (mx, total, null_mx) = mx_records(&dns, DEFAULT_MAX_RECORDS);
        let a = assess(
            &syntax("bob@example.com"),
            &DnsOutcome {
                queried: true,
                rcode: Some(0),
                mx,
                mx_total: total,
                null_mx,
                ..Default::default()
            },
        );
        assert_eq!((a.verdict, a.risk), ("fail", "high"));
        assert!(a.reason.contains("RFC 7505"), "{}", a.reason);
    }

    #[test]
    fn nxdomain_fails() {
        let a = assess(
            &syntax("bob@nope.example"),
            &DnsOutcome {
                queried: true,
                rcode: Some(3),
                ..Default::default()
            },
        );
        assert_eq!((a.verdict, a.risk), ("fail", "high"));
        assert!(a.reason.contains("NXDOMAIN"), "{}", a.reason);
    }

    #[test]
    fn servfail_is_unknown_not_a_verdict() {
        let a = assess(
            &syntax("bob@example.com"),
            &DnsOutcome {
                queried: true,
                rcode: Some(2),
                ..Default::default()
            },
        );
        assert_eq!((a.verdict, a.risk), ("unknown", "medium"));
        assert!(a.reason.contains("SERVFAIL"), "{}", a.reason);
    }

    #[test]
    fn a_record_fallback_passes_at_medium_risk() {
        let a = assess(
            &syntax("bob@example.com"),
            &DnsOutcome {
                queried: true,
                rcode: Some(0),
                a_fallback: vec!["192.0.2.7".to_string()],
                ..Default::default()
            },
        );
        assert_eq!((a.verdict, a.risk), ("pass", "medium"));
        assert!(a.reason.contains("implicit mail exchanger"), "{}", a.reason);
    }

    #[test]
    fn no_mx_and_no_address_record_fails() {
        let a = assess(
            &syntax("bob@example.com"),
            &DnsOutcome {
                queried: true,
                rcode: Some(0),
                ..Default::default()
            },
        );
        assert_eq!((a.verdict, a.risk), ("fail", "high"));
        assert!(a.reason.contains("nowhere to deliver"), "{}", a.reason);
    }

    #[test]
    fn a_failed_lookup_is_unknown() {
        let a = assess(
            &syntax("bob@example.com"),
            &DnsOutcome {
                queried: true,
                error: Some("HTTP 429 for https://dns.google/resolve".to_string()),
                ..Default::default()
            },
        );
        assert_eq!((a.verdict, a.risk), ("unknown", "medium"));
        assert!(a.reason.contains("HTTP 429"), "{}", a.reason);
    }

    // ---- rendering ---------------------------------------------------------

    #[test]
    fn report_lists_records_and_states_the_caveat() {
        let out = render("Ada <ada@gmail.com>", "google", &gmail_outcome(), "report").unwrap();
        assert!(out.contains("Email: ada@gmail.com"), "{out}");
        assert!(out.contains("Syntax: valid"), "{out}");
        assert!(out.contains("Domain: gmail.com"), "{out}");
        assert!(out.contains("Resolver: google (DNS over HTTPS)"), "{out}");
        assert!(out.contains("DNS status: NOERROR (0)"), "{out}");
        assert!(out.contains("MX records: 5 (showing 5)"), "{out}");
        assert!(
            out.contains("  5     gmail-smtp-in.l.google.com ttl 1411s"),
            "{out}"
        );
        assert!(
            out.contains("Primary mail host: gmail-smtp-in.l.google.com (preference 5)"),
            "{out}"
        );
        assert!(out.contains("Verdict: pass"), "{out}");
        assert!(out.contains("Risk: low"), "{out}");
        assert!(out.ends_with(SMTP_CAVEAT), "{out}");
    }

    #[test]
    fn report_shows_resolved_ips_when_present() {
        let mut outcome = gmail_outcome();
        outcome.mx[0].ips = vec!["192.0.2.7".to_string(), "2001:db8::1".to_string()];
        let out = render("ada@gmail.com", "google", &outcome, "report").unwrap();
        assert!(
            out.contains("gmail-smtp-in.l.google.com ttl 1411s [192.0.2.7, 2001:db8::1]"),
            "{out}"
        );
    }

    #[test]
    fn report_for_bad_syntax_says_the_lookup_was_skipped() {
        let out = render("not-an-address", "google", &DnsOutcome::default(), "report").unwrap();
        assert!(out.contains("Syntax: invalid"), "{out}");
        assert!(
            out.contains("MX lookup: skipped (the address is not valid syntax)"),
            "{out}"
        );
        assert!(out.contains("missing '@'"), "{out}");
        assert!(out.contains("Verdict: fail"), "{out}");
    }

    #[test]
    fn summary_is_one_line() {
        let out = render("ada@gmail.com", "google", &gmail_outcome(), "summary").unwrap();
        assert_eq!(
            out,
            "pass: ada@gmail.com (syntax: valid, mx: 5, primary: gmail-smtp-in.l.google.com, risk: low)"
        );
    }

    #[test]
    fn json_is_machine_readable() {
        let out = render("ada@gmail.com", "google", &gmail_outcome(), "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["email"], "ada@gmail.com");
        assert_eq!(v["valid_syntax"], true);
        assert_eq!(v["domain"], "gmail.com");
        assert_eq!(v["mx_count"], 5);
        assert_eq!(v["mx"][0]["host"], "gmail-smtp-in.l.google.com");
        assert_eq!(v["mx"][0]["preference"], 5);
        assert_eq!(v["mx"][0]["ttl"], 1411);
        assert_eq!(v["dns_status"], "NOERROR");
        assert_eq!(v["verdict"], "pass");
        assert_eq!(v["risk"], "low");
        assert_eq!(v["smtp_probe"], false);
    }

    #[test]
    fn render_rejects_an_unknown_format() {
        let err = render("ada@gmail.com", "google", &gmail_outcome(), "yaml").unwrap_err();
        assert!(err.contains("invalid format"), "{err}");
    }
}
