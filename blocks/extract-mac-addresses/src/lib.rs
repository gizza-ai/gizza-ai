//! gizza-ai/extract-mac-addresses — find MAC (EUI-48/EUI-64) addresses in text
//! written in any common notation and normalize them to one chosen format.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_extract_mac_addresses_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    format: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text or log to scan for MAC addresses."),
        )
        .param(
            Param::enumv("format", ["colon", "hyphen", "cisco", "bare"])
                .default("colon")
                .describe("Output notation each found address is normalized to: 'colon' (default) e.g. 00:1a:2b:3c:4d:5e; 'hyphen' 00-1a-2b-3c-4d-5e; 'cisco' dotted-quad 001a.2b3c.4d5e; 'bare' 001a2b3c4d5e."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractMacAddresses;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-mac-addresses",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find MAC addresses in text and normalize them",
    skill(
        description = "Scan text or a log for MAC addresses (EUI-48 and EUI-64) written in any common notation — colon (00:1A:2B:3C:4D:5E), hyphen (00-1A-2B-3C-4D-5E), Cisco dotted-quad (001a.2b3c.4d5e), or bare hex (001A2B3C4D5E). Every address is normalized to the chosen output format (set format='colon' default, 'hyphen', 'cisco', or 'bare'), deduplicated by its underlying bytes (so the same address written two ways counts once), and returned in first-seen order as a macs list plus a count. A 32-char hash or other long hex run is ignored. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ExtractMacAddresses {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "extract-mac-addresses", |a: Args| {
            extract(&a.text, &a.format).map_err(SkillError::InvalidArgs)
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
                    "text": { "type": "string", "description": "The text or log to scan for MAC addresses." },
                    "format": { "type": "string", "enum": ["colon", "hyphen", "cisco", "bare"], "default": "colon", "description": "Output notation each found address is normalized to: 'colon' (default) e.g. 00:1a:2b:3c:4d:5e; 'hyphen' 00-1a-2b-3c-4d-5e; 'cisco' dotted-quad 001a.2b3c.4d5e; 'bare' 001a2b3c4d5e." }
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
