//! gizza-ai/ip-range-expand — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure compute — no host
//! calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

use gizza_ai_ip_range_expand_core::DEFAULT_LIMIT;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    output: String,
    #[serde(default = "default_limit")]
    limit: u64,
}

fn default_limit() -> u64 {
    DEFAULT_LIMIT
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe(
                    "The CIDR block or IP range to expand. A CIDR like '192.168.1.0/24' or \
'2001:db8::/126', or a 'start-end' range like '192.168.1.10-192.168.1.20', \
'10.0.0.1-10.0.0.5', or '2001:db8::1-2001:db8::5'. IPv4 ranges also accept a bare \
last octet on the right (e.g. '192.168.1.10-20'). Works for both IPv4 and IPv6.",
                ),
        )
        .param(
            Param::enumv("output", ["list", "count"])
                .default("list")
                .describe(
                    "What to return. 'list' (default) gives every address in the range, one \
per line. 'count' gives just the total number of addresses (exact and unbounded, so it \
works even for an IPv6 /64 or /0).",
                ),
        )
        .param(
            Param::integer("limit")
                .min(1.0)
                .default(65536)
                .describe(
                    "For output='list', the maximum number of addresses to enumerate. If the \
range is larger the call errors with its actual size instead of truncating. Ignored for \
output='count'. Default 65536 (a /16).",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ip-range-expand",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Expand a CIDR block or IP range into the full address list or a count.",
    skill(
        description = "Expand a CIDR block or a start-end IP range into the full list of \
addresses (or just the count), for both IPv4 and IPv6. Set 'input' to a CIDR like \
'192.168.1.0/24' or '2001:db8::/126', or to a range like '192.168.1.10-192.168.1.20', \
'10.0.0.1-10.0.0.5', or '2001:db8::1-2001:db8::5' (IPv4 ranges also accept a bare final \
octet, e.g. '192.168.1.10-20'). A CIDR expands the WHOLE block, including the network and \
broadcast addresses, and a non-aligned base is normalized to its network address \
(192.168.1.130/29 -> 192.168.1.128). Set 'output' to 'list' (default, one address per line) \
or 'count' (total number of addresses; exact even for an IPv6 /64). 'limit' caps how many \
addresses 'list' will enumerate (default 65536); a larger range errors with its size rather \
than truncating, so use output='count' for huge ranges.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ip-range-expand", |a: Args| {
            gizza_ai_ip_range_expand_core::expand(&a.input, &a.output, a.limit)
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
                    "input": {
                        "type": "string",
                        "description": "The CIDR block or IP range to expand. A CIDR like '192.168.1.0/24' or '2001:db8::/126', or a 'start-end' range like '192.168.1.10-192.168.1.20', '10.0.0.1-10.0.0.5', or '2001:db8::1-2001:db8::5'. IPv4 ranges also accept a bare last octet on the right (e.g. '192.168.1.10-20'). Works for both IPv4 and IPv6."
                    },
                    "output": {
                        "type": "string",
                        "enum": ["list", "count"],
                        "default": "list",
                        "description": "What to return. 'list' (default) gives every address in the range, one per line. 'count' gives just the total number of addresses (exact and unbounded, so it works even for an IPv6 /64 or /0)."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "default": 65536,
                        "description": "For output='list', the maximum number of addresses to enumerate. If the range is larger the call errors with its actual size instead of truncating. Ignored for output='count'. Default 65536 (a /16)."
                    }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
