//! gizza-ai/address-parse — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    address: String,
    #[serde(default)]
    country: Option<String>,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("address")
                .required()
                .describe("The freeform postal address to parse, e.g. '123 Main St, Springfield, IL 62704, USA'. Comma-separated or multi-line input both work; surrounding whitespace is trimmed."),
        )
        .param(
            Param::enumv(
                "country",
                ["auto", "US", "GB", "CA", "AU", "DE", "FR", "IN", "NL", "ES", "IT", "BR", "JP"],
            )
            .default("auto")
            .describe("Country hint as an ISO 3166-1 alpha-2 code. Biases postcode and region detection and fills the country field when the address text omits it. 'auto' (default) detects the country from the text."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/address-parse",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a freeform postal address into structured fields",
    skill(
        description = "Parse a freeform postal address into structured fields and return JSON. Extracts the house number, street, unit/secondary designator (Apt/Suite/#), city, region (US state, Canadian province or Australian state — with its code where known), postal code (US ZIP incl. ZIP+4, UK, Canadian, and numeric formats, normalized), and country (canonical name + ISO 3166-1 alpha-2 code). Accepts comma-separated or multi-line input. An optional 'country' hint (ISO alpha-2, or 'auto') biases postcode/region detection and fills the country when the text omits it. Rule-based heuristic tuned for common formats — not a statistical model; unusual orderings may parse imperfectly. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "address-parse", |a: Args| {
            let hint = a.country.as_deref().unwrap_or("auto");
            gizza_ai_address_parse_core::run(&a.address, hint).map_err(SkillError::InvalidArgs)
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
                    "address": { "type": "string", "description": "The freeform postal address to parse, e.g. '123 Main St, Springfield, IL 62704, USA'. Comma-separated or multi-line input both work; surrounding whitespace is trimmed." },
                    "country": {
                        "type": "string",
                        "enum": ["auto", "US", "GB", "CA", "AU", "DE", "FR", "IN", "NL", "ES", "IT", "BR", "JP"],
                        "default": "auto",
                        "description": "Country hint as an ISO 3166-1 alpha-2 code. Biases postcode and region detection and fills the country field when the address text omits it. 'auto' (default) detects the country from the text."
                    }
                },
                "required": ["address"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
