//! gizza-ai/format-validator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input").required().describe(
                "The single value to validate, e.g. 'user@example.com', 'https://example.com/path', \
'192.168.1.1', '2001:db8::1', '+1 (415) 555-2671', or '4111 1111 1111 1111'. Whitespace at the \
ends is trimmed. One value per call.",
            ),
        )
        .param(
            Param::enumv(
                "format",
                ["auto", "email", "url", "ipv4", "ipv6", "ip", "phone", "credit-card"],
            )
            .default("auto")
            .describe(
                "Which format to check against. 'auto' (default) tests every format, reports the \
detected format (priority ipv4, ipv6, email, url, credit-card, phone) and a per-format table. \
Otherwise validate against exactly one: 'email' (RFC-style syntax, no DNS/MX probe), 'url' \
(needs scheme://host; reports scheme + host), 'ipv4', 'ipv6', 'ip' (either family), 'phone' \
(7-15 digits, optional leading +, E.164-style), or 'credit-card' (Luhn check + brand, 12-19 \
digits).",
            ),
        )
        .param(
            Param::enumv("output", ["text", "json"]).default("text").describe(
                "Output format. 'text' (default) is a human-readable report. 'json' is a \
machine-readable object: for a single format {input, format, valid, detail}; for auto \
{input, detected, checks:[{format, valid, detail}]}.",
            ),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/format-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate whether a string is a well-formed email, URL, IPv4/IPv6, phone, or credit card (Luhn), or auto-detect the format.",
    skill(
        description = "Check whether 'input' is a well-formed email address, URL, IPv4 or IPv6 \
address, phone number, or credit-card number (with a Luhn checksum). Set 'format' to 'auto' \
(default) to test every format and report which one it is (detection priority ipv4, ipv6, \
email, url, credit-card, phone) plus a per-format pass/fail table; or set it to one of 'email', \
'url', 'ipv4', 'ipv6', 'ip', 'phone', 'credit-card' to validate against just that format and \
explain the verdict. This is a pure syntax/format check that runs entirely in the sandbox: it \
never performs DNS/MX lookups, reachability probes, or BIN lookups. Credit-card checks run the \
Luhn algorithm and report the brand (Visa, Mastercard, American Express, Discover, JCB, Diners \
Club). Set 'output' to 'text' (default) or 'json'.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "format-validator", |a: Args| {
            gizza_ai_format_validator_core::validate(&a.input, &a.format, &a.output)
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The single value to validate, e.g. 'user@example.com', 'https://example.com/path', '192.168.1.1', '2001:db8::1', '+1 (415) 555-2671', or '4111 1111 1111 1111'. Whitespace at the ends is trimmed. One value per call." },
                    "format": { "type": "string", "enum": ["auto", "email", "url", "ipv4", "ipv6", "ip", "phone", "credit-card"], "default": "auto", "description": "Which format to check against. 'auto' (default) tests every format, reports the detected format (priority ipv4, ipv6, email, url, credit-card, phone) and a per-format table. Otherwise validate against exactly one: 'email' (RFC-style syntax, no DNS/MX probe), 'url' (needs scheme://host; reports scheme + host), 'ipv4', 'ipv6', 'ip' (either family), 'phone' (7-15 digits, optional leading +, E.164-style), or 'credit-card' (Luhn check + brand, 12-19 digits)." },
                    "output": { "type": "string", "enum": ["text", "json"], "default": "text", "description": "Output format. 'text' (default) is a human-readable report. 'json' is a machine-readable object: for a single format {input, format, valid, detail}; for auto {input, detected, checks:[{format, valid, detail}]}." }
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
