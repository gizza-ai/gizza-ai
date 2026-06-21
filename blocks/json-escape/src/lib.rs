//! gizza-ai/json-escape — escape or unescape a string for use inside JSON. Chat
//! schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_escape_core::{process, Mode};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    quotes: bool,
}

fn default_mode() -> String {
    "escape".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The string to escape or unescape."))
        .param(
            Param::enumv("mode", ["escape", "unescape"])
                .default("escape")
                .describe("escape = make the text safe inside a JSON string; unescape = turn a JSON-escaped string back to raw text."),
        )
        .param(
            Param::boolean("quotes")
                .default(false)
                .describe("Escape mode only: also wrap the result in surrounding double quotes."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonEscape;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-escape",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Escape or unescape a string for JSON",
    skill(
        description = "Escape or unescape special characters in a string for safe use inside JSON. mode=escape (default) turns raw text into a JSON-safe string body (escaping quotes, backslashes, newlines, tabs, and control chars as \\uXXXX); set quotes=true to also wrap it in double quotes. mode=unescape reverses it, accepting input with or without surrounding quotes. Uses a spec-correct JSON codec. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonEscape {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-escape", |a: Args| {
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            process(&a.text, mode, a.quotes).map_err(SkillError::InvalidArgs)
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
                    "text":   { "type": "string", "description": "The string to escape or unescape." },
                    "mode":   { "type": "string", "enum": ["escape", "unescape"], "default": "escape", "description": "escape = make the text safe inside a JSON string; unescape = turn a JSON-escaped string back to raw text." },
                    "quotes": { "type": "boolean", "default": false, "description": "Escape mode only: also wrap the result in surrounding double quotes." }
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
