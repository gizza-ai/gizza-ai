//! gizza-ai/cidr-calculator — chat skill block on the shared tool abstraction.
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
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input").required().describe(
                "The CIDR block to analyze, as ADDRESS/PREFIX. IPv4 like '192.168.1.0/24', \
'10.0.0.0/8', or a non-aligned host such as '192.168.1.130/26' (normalized to its network \
'192.168.1.128'). IPv6 like '2001:db8::/48' or '2001:db8::/64'. Prefix is 0-32 for IPv4 \
and 0-128 for IPv6.",
            ),
        )
        .param(
            Param::enumv("format", ["text", "json"]).default("text").describe(
                "Output format. 'text' (default) is an aligned human-readable report. 'json' is \
a machine-readable object with the same fields (network, broadcast, netmask, wildcard, \
first_host, last_host, total_addresses, usable_hosts, is_private for IPv4; network, \
first_address, last_address, total_addresses for IPv6).",
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
    name = "gizza-ai/cidr-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute network address, broadcast, netmask, wildcard, host range and counts for a CIDR.",
    skill(
        description = "Calculate the subnet properties of an IPv4 or IPv6 CIDR block. Set 'input' \
to a CIDR like '192.168.1.0/24', '10.0.0.0/8', '192.168.1.130/26', '2001:db8::/48', or \
'2001:db8::/64'. For IPv4 it returns the network address, broadcast address, netmask, wildcard \
mask, usable host range (first-last assignable host), total address count, usable host count, \
and whether the block is private (RFC 1918/loopback/link-local/CGNAT) or public. A non-aligned \
base is normalized to its network address (192.168.1.130/26 -> 192.168.1.128). /31 is treated \
as a point-to-point link (both addresses usable, RFC 3021) and /32 as a single host. For IPv6 \
it returns the network address, prefix length, first and last address, and the total address \
count (exact even for a /0 = 2^128). Set 'format' to 'text' (default, aligned report) or 'json' \
(machine-readable object).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cidr-calculator", |a: Args| {
            gizza_ai_cidr_calculator_core::calculate(&a.input, &a.format)
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
                        "description": "The CIDR block to analyze, as ADDRESS/PREFIX. IPv4 like '192.168.1.0/24', '10.0.0.0/8', or a non-aligned host such as '192.168.1.130/26' (normalized to its network '192.168.1.128'). IPv6 like '2001:db8::/48' or '2001:db8::/64'. Prefix is 0-32 for IPv4 and 0-128 for IPv6."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "json"],
                        "default": "text",
                        "description": "Output format. 'text' (default) is an aligned human-readable report. 'json' is a machine-readable object with the same fields (network, broadcast, netmask, wildcard, first_host, last_host, total_addresses, usable_hosts, is_private for IPv4; network, first_address, last_address, total_addresses for IPv6)."
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
