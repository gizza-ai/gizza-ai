//! gizza-ai/mac-address-format — reformat one or many MAC (EUI-48 / EUI-64)
//! addresses between the four common notations with a chosen hex case.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_mac_address_format_core::reformat;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    case: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("One or more MAC addresses to reformat, in any notation (colon, hyphen, Cisco dotted, or bare). Separate multiple addresses with whitespace, commas, or newlines."),
        )
        .param(
            Param::enumv("format", ["colon", "hyphen", "cisco", "bare"])
                .default("colon")
                .describe("Output notation: 'colon' (default) e.g. 00:1a:2b:3c:4d:5e; 'hyphen' 00-1a-2b-3c-4d-5e; 'cisco' dotted-quad 001a.2b3c.4d5e; 'bare' 001a2b3c4d5e."),
        )
        .param(
            Param::enumv("case", ["lower", "upper"])
                .default("lower")
                .describe("Hex case of the output: 'lower' (default) or 'upper'."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mac-address-format",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reformat MAC addresses between colon, hyphen, Cisco, and bare styles",
    skill(
        description = "Reformat one or many MAC addresses (EUI-48 and EUI-64) between the four common notations — colon (00:1A:2B:3C:4D:5E), hyphen (00-1A-2B-3C-4D-5E), Cisco dotted-quad (001a.2b3c.4d5e), and bare hex (001A2B3C4D5E) — with a chosen hex case. The input may be in any of those notations; set format='colon' (default), 'hyphen', 'cisco', or 'bare' and case='lower' (default) or 'upper'. Separate multiple addresses with whitespace, commas, or newlines; order is preserved and duplicates are kept (use extract-mac-addresses to pull MACs out of free text and dedupe). Every address is validated, so a token that isn't a 12- or 16-hex-digit MAC is reported as an error. Returns a macs list plus a count. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mac-address-format", |a: Args| {
            reformat(&a.input, &a.format, &a.case).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "One or more MAC addresses to reformat, in any notation (colon, hyphen, Cisco dotted, or bare). Separate multiple addresses with whitespace, commas, or newlines." },
                    "format": { "type": "string", "enum": ["colon", "hyphen", "cisco", "bare"], "default": "colon", "description": "Output notation: 'colon' (default) e.g. 00:1a:2b:3c:4d:5e; 'hyphen' 00-1a-2b-3c-4d-5e; 'cisco' dotted-quad 001a.2b3c.4d5e; 'bare' 001a2b3c4d5e." },
                    "case": { "type": "string", "enum": ["lower", "upper"], "default": "lower", "description": "Hex case of the output: 'lower' (default) or 'upper'." }
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
