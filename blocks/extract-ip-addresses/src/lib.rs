//! gizza-ai/extract-ip-addresses — find, validate, and dedupe IPv4/IPv6 in text.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_extract_ip_addresses_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("text")
            .required()
            .describe("The text or log to scan for IP addresses."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractIpAddresses;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-ip-addresses",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find IPv4 and IPv6 addresses in text",
    skill(
        description = "Scan text or a log for IP addresses. Finds and validates both IPv4 (dotted-quad, octets checked) and IPv6 (including compressed :: and bracketed forms), deduplicates them (IPv6 canonicalized to its compressed form), and returns them in first-seen order as separate ipv4 and ipv6 lists plus a total count. Ports and surrounding punctuation are handled. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ExtractIpAddresses {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "extract-ip-addresses", |a: Args| {
            Ok::<_, SkillError>(extract(&a.text))
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
                    "text": { "type": "string", "description": "The text or log to scan for IP addresses." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
