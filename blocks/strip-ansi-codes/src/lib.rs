//! gizza-ai/strip-ansi-codes — removes ANSI escape & colour codes from terminal
//! output. Thin chat-skill wrapper around `gizza-ai-strip-ansi-codes-core`; the
//! chat schema is single-sourced from `descriptor()` (shared with the CLI) and
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    scope: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The terminal text (with ANSI escape codes) to clean."),
        )
        .param(
            Param::enumv("scope", ["all", "color"])
                .default("all")
                .describe("What to remove. 'all' (default) strips every ANSI escape sequence — colours, cursor/erase control, OSC titles & hyperlinks — leaving plain text. 'color' strips only SGR colour & style codes (e.g. \\x1b[1;31m) and keeps cursor/erase control intact."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct StripAnsiCodes;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/strip-ansi-codes",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove ANSI escape & color codes from terminal output",
    skill(
        description = "Remove ANSI escape and color codes from terminal output, leaving clean readable text. Pass the captured terminal/log text as 'text'. scope='all' (default) strips every escape sequence — SGR colours/styles, cursor & erase control, and OSC strings like window titles and \x1b]8 hyperlinks. scope='color' strips only the colour/style SGR codes (e.g. \x1b[1;31m) and keeps cursor/erase positioning intact. Unicode (accents, emoji) and newlines are preserved.",
        parameters = schema_json()
    ),
)]
impl StripAnsiCodes {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … }.
        match run_skill(&body, "strip-ansi-codes", |a: Args| {
            gizza_ai_strip_ansi_codes_core::strip(&a.text, &a.scope).map_err(SkillError::InvalidArgs)
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
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The terminal text (with ANSI escape codes) to clean." },
                    "scope": { "type": "string", "enum": ["all", "color"], "default": "all", "description": "What to remove. 'all' (default) strips every ANSI escape sequence — colours, cursor/erase control, OSC titles & hyperlinks — leaving plain text. 'color' strips only SGR colour & style codes (e.g. \\x1b[1;31m) and keeps cursor/erase control intact." }
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
