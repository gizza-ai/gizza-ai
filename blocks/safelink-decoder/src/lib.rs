//! gizza-ai/safelink-decoder — reveal the real URL behind a rewrite wrapper.
//!
//! Thin chat-skill wrapper around `gizza-ai-safelink-decoder-core`. Chat schema
//! single-sourced from `descriptor()`; handler delegates to `run_skill`. Pure
//! string work (no network) — runs entirely in the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_safelink_decoder_core::decode;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    url: String,
    #[serde(default)]
    per_line: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("url").required().describe("The wrapped/rewritten URL to unwrap (or a batch list, one per line with per_line=true)."))
        .param(Param::boolean("per_line").default(false).describe("When true, unwrap each line of the input independently (rejoined with newlines). Default false."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SafelinkDecoder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/safelink-decoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Unwrap SafeLinks / Proofpoint / redirect URLs",
    skill(
        description = "Reveal the original destination behind a URL-rewrite wrapper: Outlook SafeLinks, Proofpoint URLDefense (v2 and v3), Google redirects, and generic ?url=/?q=/?u= redirectors. Follows nested wrappers. Pure string decoding — it does NOT fetch the URL. Set per_line=true to unwrap a batch list of URLs.",
        parameters = schema_json()
    )
)]
impl SafelinkDecoder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "safelink-decoder", |a: Args| {
            decode(&a.url, a.per_line).map_err(SkillError::InvalidArgs)
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
                    "url":      { "type": "string", "description": "The wrapped/rewritten URL to unwrap (or a batch list, one per line with per_line=true)." },
                    "per_line": { "type": "boolean", "default": false, "description": "When true, unwrap each line of the input independently (rejoined with newlines). Default false." }
                },
                "required": ["url"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
