//! gizza-ai/ip-range-to-cidr — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure compute — no host
//! calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe(
                    "The 'start-end' IP range to convert into CIDR blocks. IPv4 like \
'192.168.1.0-192.168.1.255' or '10.0.0.5-10.0.0.20' (a bare final octet on the right is \
accepted as shorthand, e.g. '192.168.1.10-20'), or IPv6 like '2001:db8::-2001:db8::ffff' \
or '2001:db8::1-2001:db8::5'. A single address with no '-' is treated as a one-address \
range and returns a /32 (IPv4) or /128 (IPv6) host route. Both endpoints must be the same \
family and start must be <= end.",
                ),
        )
        .param(
            Param::enumv("output", ["list", "count"])
                .default("list")
                .describe(
                    "What to return. 'list' (default) gives the minimal set of CIDR blocks that \
exactly covers the range, one per line. 'count' gives just the number of CIDR blocks needed.",
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
    name = "gizza-ai/ip-range-to-cidr",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a start-end IP range into the minimal set of CIDR blocks covering it.",
    skill(
        description = "Convert an arbitrary inclusive 'start-end' IP range into the MINIMAL set \
of CIDR blocks that exactly covers it (no more, no fewer addresses), for both IPv4 and IPv6. \
Set 'input' to a range like '192.168.1.0-192.168.1.255', '10.0.0.5-10.0.0.20', \
'2001:db8::-2001:db8::ffff', or '2001:db8::1-2001:db8::5' (IPv4 ranges also accept a bare final \
octet on the right, e.g. '192.168.1.10-20'). A single address with no '-' yields a single host \
route (/32 for IPv4, /128 for IPv6). An aligned range collapses to one CIDR \
(192.168.1.0-192.168.1.255 -> 192.168.1.0/24); an unaligned range is split into the fewest \
aligned blocks (10.0.0.5-10.0.0.20 -> 10.0.0.5/32, 10.0.0.6/31, 10.0.0.8/29, 10.0.0.16/30, \
10.0.0.20/32). Set 'output' to 'list' (default, one CIDR per line) or 'count' (just how many \
CIDR blocks are needed). This is the inverse of expanding a CIDR into addresses.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ip-range-to-cidr", |a: Args| {
            gizza_ai_ip_range_to_cidr_core::run(&a.input, &a.output)
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
                        "description": "The 'start-end' IP range to convert into CIDR blocks. IPv4 like '192.168.1.0-192.168.1.255' or '10.0.0.5-10.0.0.20' (a bare final octet on the right is accepted as shorthand, e.g. '192.168.1.10-20'), or IPv6 like '2001:db8::-2001:db8::ffff' or '2001:db8::1-2001:db8::5'. A single address with no '-' is treated as a one-address range and returns a /32 (IPv4) or /128 (IPv6) host route. Both endpoints must be the same family and start must be <= end."
                    },
                    "output": {
                        "type": "string",
                        "enum": ["list", "count"],
                        "default": "list",
                        "description": "What to return. 'list' (default) gives the minimal set of CIDR blocks that exactly covers the range, one per line. 'count' gives just the number of CIDR blocks needed."
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
