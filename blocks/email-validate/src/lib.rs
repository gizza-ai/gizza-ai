//! gizza-ai/email-validate — validate email syntax and check MX deliverability
//! plausibility via DNS-over-HTTPS.
//!
//! The block is intentionally honest: it uses the existing offline
//! `email-validator` rules for syntax, then performs HTTP-only DoH queries through
//! `wafer-run/network`. It never opens a raw DNS socket and never performs an SMTP
//! mailbox probe, so `pass` means "the domain has somewhere to deliver mail",
//! not "this mailbox exists".

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_email_validate_core::{
    addresses, doh_headers, doh_url, mx_records, normalize_max_records, parse_dns_json, render,
    syntax, DnsOutcome, DEFAULT_MAX_RECORDS, MAX_MAX_RECORDS,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Debug)]
struct Args {
    email: String,
    #[serde(default)]
    resolver: Option<String>,
    #[serde(default)]
    fallback_a: Option<bool>,
    #[serde(default)]
    resolve_ips: Option<bool>,
    #[serde(default)]
    max_records: Option<u32>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source parameter descriptor → chat schema, CLI, and manifest sync.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("email")
                .required()
                .placeholder("ada@gmail.com")
                .describe("Email address to validate before looking up its domain's MX records."),
        )
        .param(
            Param::enumv("resolver", ["google", "cloudflare"])
                .default("google")
                .describe("DNS-over-HTTPS resolver to query for MX records."),
        )
        .param(
            Param::boolean("fallback_a")
                .default(true)
                .describe("When a domain has no MX, also query A/AAAA records and apply RFC 5321's implicit-MX fallback."),
        )
        .param(
            Param::boolean("resolve_ips")
                .default(false)
                .describe("Also resolve A/AAAA addresses for each reported MX host (one extra lookup per host)."),
        )
        .param(
            Param::integer("max_records")
                .default(DEFAULT_MAX_RECORDS)
                .min(1.0)
                .max(MAX_MAX_RECORDS as f64)
                .describe("Maximum MX records to list after sorting by preference (1-50, default 10)."),
        )
        .param(
            Param::enumv("format", ["report", "summary", "json"])
                .default("report")
                .describe("Output format: human report, one-line summary, or machine-readable JSON."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EmailValidate;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/email-validate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate email syntax and check MX deliverability over DNS-over-HTTPS",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Validate one email address, then use DNS-over-HTTPS to check whether its domain publishes MX records (or an RFC 5321 implicit A/AAAA fallback). Reports MX priority, TTL, optional host IPs, risk, and an explicit no-SMTP caveat.",
        parameters = schema_json()
    ),
)]
impl EmailValidate {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("email-validate")?;
    let resolver = args.resolver.as_deref().unwrap_or("google");
    let format = args.format.as_deref().unwrap_or("report");
    let fallback_a = args.fallback_a.unwrap_or(true);
    let resolve_ips = args.resolve_ips.unwrap_or(false);
    let max_records = normalize_max_records(args.max_records);

    let validation = syntax(&args.email);
    let mut outcome = DnsOutcome::default();

    if validation.valid {
        outcome.queried = true;
        let domain = validation
            .domain
            .as_deref()
            .ok_or_else(|| SkillError::InvalidArgs("valid email had no domain".to_string()))?;
        outcome = query_mx(domain, resolver, max_records)?;

        if outcome.error.is_none()
            && outcome.rcode == Some(0)
            && outcome.mx.is_empty()
            && !outcome.null_mx
            && fallback_a
        {
            outcome.a_fallback = query_addresses(domain, resolver)?;
        }
        if outcome.error.is_none() && resolve_ips {
            for rec in &mut outcome.mx {
                rec.ips = query_addresses(&rec.host, resolver)?;
            }
        }
    }

    let report =
        render(&args.email, resolver, &outcome, format).map_err(SkillError::InvalidArgs)?;
    Ok(report.into_bytes())
}

#[cfg(target_arch = "wasm32")]
fn query_mx(domain: &str, resolver: &str, max_records: u32) -> Result<DnsOutcome, SkillError> {
    let url = doh_url(resolver, domain, "MX").map_err(SkillError::InvalidArgs)?;
    let headers: HashMap<String, String> = doh_headers(resolver)
        .map_err(SkillError::InvalidArgs)?
        .into_iter()
        .collect();
    let resp = wafer_sdk::clients::network::do_request("GET", &url, &headers, None)?;
    if resp.status_code >= 400 {
        return Ok(DnsOutcome {
            queried: true,
            error: Some(format!("HTTP {} for {url}", resp.status_code)),
            ..Default::default()
        });
    }
    let body = String::from_utf8_lossy(&resp.body);
    let dns = match parse_dns_json(&body) {
        Ok(dns) => dns,
        Err(e) => {
            return Ok(DnsOutcome {
                queried: true,
                error: Some(e),
                ..Default::default()
            })
        }
    };
    let (mx, mx_total, null_mx) = mx_records(&dns, max_records);
    Ok(DnsOutcome {
        queried: true,
        rcode: Some(dns.status),
        mx,
        mx_total,
        null_mx,
        ..Default::default()
    })
}

#[cfg(target_arch = "wasm32")]
fn query_addresses(name: &str, resolver: &str) -> Result<Vec<String>, SkillError> {
    let mut ips = Vec::new();
    for qtype in ["A", "AAAA"] {
        let url = doh_url(resolver, name, qtype).map_err(SkillError::InvalidArgs)?;
        let headers: HashMap<String, String> = doh_headers(resolver)
            .map_err(SkillError::InvalidArgs)?
            .into_iter()
            .collect();
        let resp = wafer_sdk::clients::network::do_request("GET", &url, &headers, None)?;
        if resp.status_code >= 400 {
            continue;
        }
        let body = String::from_utf8_lossy(&resp.body);
        if let Ok(dns) = parse_dns_json(&body) {
            if dns.status == 0 {
                ips.extend(addresses(&dns));
            }
        }
    }
    ips.sort();
    ips.dedup();
    Ok(ips)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
              "type":"object",
              "properties":{
                "email":{"type":"string","description":"Email address to validate before looking up its domain's MX records."},
                "resolver":{"type":"string","enum":["google","cloudflare"],"default":"google","description":"DNS-over-HTTPS resolver to query for MX records."},
                "fallback_a":{"type":"boolean","default":true,"description":"When a domain has no MX, also query A/AAAA records and apply RFC 5321's implicit-MX fallback."},
                "resolve_ips":{"type":"boolean","default":false,"description":"Also resolve A/AAAA addresses for each reported MX host (one extra lookup per host)."},
                "max_records":{"type":"integer","minimum":1,"maximum":50,"default":10,"description":"Maximum MX records to list after sorting by preference (1-50, default 10)."},
                "format":{"type":"string","enum":["report","summary","json"],"default":"report","description":"Output format: human report, one-line summary, or machine-readable JSON."}
              },
              "required":["email"],
              "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
