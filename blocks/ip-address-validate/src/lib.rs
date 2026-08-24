//! gizza-ai/ip-address-validate — chat skill block on the shared tool abstraction.
//! Validates IPv4/IPv6 addresses, canonicalizes them (RFC 5952) and classifies
//! them against the IANA special-purpose registries. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    addresses: String,
    #[serde(default = "default_family")]
    family: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_ipv6_form")]
    ipv6_form: String,
    #[serde(default = "default_true")]
    allow_prefix: bool,
    #[serde(default = "default_true")]
    allow_port: bool,
    #[serde(default)]
    allow_leading_zeros: bool,
    #[serde(default)]
    dedupe: bool,
}

fn default_family() -> String {
    "any".to_string()
}
fn default_output() -> String {
    "report".to_string()
}
fn default_ipv6_form() -> String {
    "compressed".to_string()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI). Param order matches
/// `page/meta.toml` and the `web` wrapper's argument order.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("addresses")
                .required()
                .describe("The IP addresses to check, one per line. Blank lines are ignored. Each line may carry an optional '/prefix' length, ':port' (or '[v6]:port'), and '%zone' id — e.g. '192.168.1.1', '2001:0db8::1', '10.0.0.0/8', '[2001:db8::1]:443', 'fe80::1%eth0'. Up to 5000 lines / 500000 bytes per run."),
        )
        .param(
            Param::enumv("family", ["any", "ipv4", "ipv6"])
                .default("any")
                .describe("Which address family to accept. 'any' (default) accepts both. 'ipv4' or 'ipv6' turns an address of the other family into an invalid result with the reason 'family filter is set to …', which is how you assert that a list is single-family."),
        )
        .param(
            Param::enumv("output", ["report", "table", "json", "valid", "invalid", "summary"])
                .default("report")
                .describe("The shape of the result. 'report' (default) is a human-readable labelled block per address. 'table' is CSV with the header line,input,status,version,canonical,expanded,type,scope,routable,prefix,port,zone,embedded_ipv4,integer,hex,reverse_dns,error. 'json' is an array of objects with the same fields. 'valid' is just the canonical addresses, one per line, with zone/prefix/port put back on — the form to paste into a config. 'invalid' is 'line N: input — reason' for the rejected lines only. 'summary' is a CSV of the totals (total, valid, invalid, duplicates, ipv4, ipv6, globally_routable, and a type:<kind> count per category)."),
        )
        .param(
            Param::enumv("ipv6_form", ["compressed", "expanded"])
                .default("compressed")
                .describe("The canonical IPv6 spelling to emit. 'compressed' (default) is the RFC 5952 form — lowercase hex, no leading zeros in a group, and the longest run of zero groups collapsed to '::'. 'expanded' is the full eight groups of four hex digits, e.g. 2001:0db8:0000:0000:0000:0000:0000:0001. Ignored for IPv4."),
        )
        .param(
            Param::boolean("allow_prefix")
                .default(true)
                .describe("Accept a trailing '/prefix' length such as 10.0.0.0/8 or 2001:db8::/32, validating it against the family's range (0-32 for IPv4, 0-128 for IPv6). Default true. Set false to reject CIDR suffixes, for a field that must hold a bare host address."),
        )
        .param(
            Param::boolean("allow_port")
                .default(true)
                .describe("Accept a trailing ':port' such as 192.0.2.7:8080, or the bracketed IPv6 authority form [2001:db8::1]:443, and validate the port is 0-65535. Default true. Set false to reject any port suffix."),
        )
        .param(
            Param::boolean("allow_leading_zeros")
                .default(false)
                .describe("Accept IPv4 octets written with leading zeros, e.g. 192.168.01.1, and read them as DECIMAL. Default false, which rejects them — they are ambiguous because some C-derived parsers (inet_aton, ping) read a leading zero as octal, so 010 means 8 there and 10 here. Turn this on only for input you know is decimal."),
        )
        .param(
            Param::boolean("dedupe")
                .default(false)
                .describe("Drop repeat addresses instead of listing them. Duplicates are matched on the CANONICAL form, so 2001:DB8::1 and 2001:0db8:0000:0000:0000:0000:0000:0001 count as the same address. Default false, which keeps every line and marks the later ones 'duplicate'. The 'summary' counts always cover every input line regardless of this setting."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ip-address-validate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate, canonicalize and classify IPv4 and IPv6 addresses",
    skill(
        description = "Validate IPv4 and IPv6 addresses, rewrite them in canonical form, and classify what each one is. Takes one address per line (up to 5000) and, for every line, reports: valid or invalid with a SPECIFIC reason ('octet 4 (256) is out of range 0-255', \"'::' may appear only once\", 'prefix length 33 is out of range for IPv4 (0-32)'); the canonical text — IPv4 with leading zeros stripped, IPv6 in the RFC 5952 lowercase compressed form or fully expanded on request; the category from the IANA special-purpose registries (private, loopback, link-local, multicast, documentation, shared/CGNAT, unique-local, teredo, 6to4, nat64, ipv4-mapped, benchmarking, reserved, broadcast, unspecified, global) with the exact block and RFC that decided it; whether it is globally routable; the legacy IPv4 class letter; the integer, hex and dotted-binary values; the in-addr.arpa / ip6.arpa reverse-DNS pointer name; and any embedded IPv4 inside a mapped, 6to4 or NAT64 address. Each line may carry a '/prefix' length, a ':port' (including the bracketed '[2001:db8::1]:443' authority form) and a '%zone' id, each individually acceptable or refusable via allow_prefix / allow_port. family='ipv4'|'ipv6' asserts a single-family list. dedupe collapses addresses that share a canonical form. output selects the shape: 'report' (default, labelled text), 'table' (CSV), 'json', 'valid' (canonical addresses only), 'invalid' (rejects and reasons), or 'summary' (CSV of the totals and a count per category). Syntax and classification only — nothing is pinged, resolved or geolocated, and it runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ip-address-validate", |a: Args| {
            gizza_ai_ip_address_validate_core::run(
                &a.addresses,
                &a.family,
                a.allow_prefix,
                a.allow_port,
                a.allow_leading_zeros,
                &a.ipv6_form,
                a.dedupe,
                &a.output,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "addresses": { "type": "string", "description": "The IP addresses to check, one per line. Blank lines are ignored. Each line may carry an optional '/prefix' length, ':port' (or '[v6]:port'), and '%zone' id — e.g. '192.168.1.1', '2001:0db8::1', '10.0.0.0/8', '[2001:db8::1]:443', 'fe80::1%eth0'. Up to 5000 lines / 500000 bytes per run." },
                    "family": {
                        "type": "string",
                        "enum": ["any", "ipv4", "ipv6"],
                        "default": "any",
                        "description": "Which address family to accept. 'any' (default) accepts both. 'ipv4' or 'ipv6' turns an address of the other family into an invalid result with the reason 'family filter is set to …', which is how you assert that a list is single-family."
                    },
                    "output": {
                        "type": "string",
                        "enum": ["report", "table", "json", "valid", "invalid", "summary"],
                        "default": "report",
                        "description": "The shape of the result. 'report' (default) is a human-readable labelled block per address. 'table' is CSV with the header line,input,status,version,canonical,expanded,type,scope,routable,prefix,port,zone,embedded_ipv4,integer,hex,reverse_dns,error. 'json' is an array of objects with the same fields. 'valid' is just the canonical addresses, one per line, with zone/prefix/port put back on — the form to paste into a config. 'invalid' is 'line N: input — reason' for the rejected lines only. 'summary' is a CSV of the totals (total, valid, invalid, duplicates, ipv4, ipv6, globally_routable, and a type:<kind> count per category)."
                    },
                    "ipv6_form": {
                        "type": "string",
                        "enum": ["compressed", "expanded"],
                        "default": "compressed",
                        "description": "The canonical IPv6 spelling to emit. 'compressed' (default) is the RFC 5952 form — lowercase hex, no leading zeros in a group, and the longest run of zero groups collapsed to '::'. 'expanded' is the full eight groups of four hex digits, e.g. 2001:0db8:0000:0000:0000:0000:0000:0001. Ignored for IPv4."
                    },
                    "allow_prefix": { "type": "boolean", "default": true, "description": "Accept a trailing '/prefix' length such as 10.0.0.0/8 or 2001:db8::/32, validating it against the family's range (0-32 for IPv4, 0-128 for IPv6). Default true. Set false to reject CIDR suffixes, for a field that must hold a bare host address." },
                    "allow_port": { "type": "boolean", "default": true, "description": "Accept a trailing ':port' such as 192.0.2.7:8080, or the bracketed IPv6 authority form [2001:db8::1]:443, and validate the port is 0-65535. Default true. Set false to reject any port suffix." },
                    "allow_leading_zeros": { "type": "boolean", "default": false, "description": "Accept IPv4 octets written with leading zeros, e.g. 192.168.01.1, and read them as DECIMAL. Default false, which rejects them — they are ambiguous because some C-derived parsers (inet_aton, ping) read a leading zero as octal, so 010 means 8 there and 10 here. Turn this on only for input you know is decimal." },
                    "dedupe": { "type": "boolean", "default": false, "description": "Drop repeat addresses instead of listing them. Duplicates are matched on the CANONICAL form, so 2001:DB8::1 and 2001:0db8:0000:0000:0000:0000:0000:0001 count as the same address. Default false, which keeps every line and marks the later ones 'duplicate'. The 'summary' counts always cover every input line regardless of this setting." }
                },
                "required": ["addresses"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// Every advertised `output` choice must be a shape the core actually
    /// renders — an enum value the core rejects would be a dead choice on the
    /// page's dropdown.
    #[test]
    fn every_advertised_output_choice_runs() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        for v in schema["properties"]["output"]["enum"].as_array().unwrap() {
            let out = v.as_str().unwrap();
            gizza_ai_ip_address_validate_core::run(
                "192.0.2.1\nnot-an-ip",
                "any",
                true,
                true,
                false,
                "compressed",
                false,
                out,
            )
            .unwrap_or_else(|e| panic!("output={out} should render, got {e}"));
        }
        for v in schema["properties"]["family"]["enum"].as_array().unwrap() {
            let fam = v.as_str().unwrap();
            gizza_ai_ip_address_validate_core::run(
                "192.0.2.1", fam, true, true, false, "compressed", false, "report",
            )
            .unwrap_or_else(|e| panic!("family={fam} should run, got {e}"));
        }
        for v in schema["properties"]["ipv6_form"]["enum"].as_array().unwrap() {
            let form = v.as_str().unwrap();
            gizza_ai_ip_address_validate_core::run(
                "2001:db8::1", "any", true, true, false, form, false, "valid",
            )
            .unwrap_or_else(|e| panic!("ipv6_form={form} should run, got {e}"));
        }
    }

    /// The caps quoted in the `addresses` description must be the caps the core
    /// enforces.
    #[test]
    fn advertised_caps_match_the_core() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let desc = schema["properties"]["addresses"]["description"]
            .as_str()
            .unwrap();
        assert!(
            desc.contains(&gizza_ai_ip_address_validate_core::MAX_ADDRESSES.to_string()),
            "descriptor should quote the real line cap"
        );
        assert!(
            desc.contains(&gizza_ai_ip_address_validate_core::MAX_BYTES.to_string()),
            "descriptor should quote the real byte cap"
        );
    }
}
