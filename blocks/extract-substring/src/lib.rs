//! gizza-ai/extract-substring — pull out a portion of text by index or between
//! delimiters. Chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_extract_substring_core::{extract, Mode};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    start: i64,
    #[serde(default)]
    end: Option<i64>,
    #[serde(default)]
    start_delim: String,
    #[serde(default)]
    end_delim: String,
}

fn default_mode() -> String {
    "index".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The source text to extract from."))
        .param(
            Param::enumv("mode", ["index", "delimiters"])
                .default("index")
                .describe("How to extract: 'index' (by character position) or 'delimiters' (everything between two markers)."),
        )
        .param(
            Param::integer("start")
                .default(0)
                .describe("index mode: 0-based start character (inclusive). Negative counts from the end. Default 0."),
        )
        .param(
            Param::integer("end")
                .describe("index mode: end character (exclusive). Negative counts from the end. Omit for end-of-string."),
        )
        .param(
            Param::string("start_delim")
                .describe("delimiters mode: the opening delimiter (required in delimiters mode)."),
        )
        .param(
            Param::string("end_delim")
                .describe("delimiters mode: the closing delimiter (required in delimiters mode)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractSubstring;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-substring",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract a portion of text by index or between delimiters",
    skill(
        description = "Pull out a portion of text. mode='index' returns the substring from a 0-based start character (inclusive) to an end character (exclusive); both may be negative to count from the end, and end may be omitted for the rest of the string (indices are character-based and clamped). mode='delimiters' returns every non-overlapping substring found between a start_delim and end_delim. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ExtractSubstring {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "extract-substring", |a: Args| {
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            extract(&a.text, mode, a.start, a.end, &a.start_delim, &a.end_delim)
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
                    "text": { "type": "string", "description": "The source text to extract from." },
                    "mode": { "type": "string", "enum": ["index", "delimiters"], "default": "index", "description": "How to extract: 'index' (by character position) or 'delimiters' (everything between two markers)." },
                    "start": { "type": "integer", "default": 0, "description": "index mode: 0-based start character (inclusive). Negative counts from the end. Default 0." },
                    "end": { "type": "integer", "description": "index mode: end character (exclusive). Negative counts from the end. Omit for end-of-string." },
                    "start_delim": { "type": "string", "description": "delimiters mode: the opening delimiter (required in delimiters mode)." },
                    "end_delim": { "type": "string", "description": "delimiters mode: the closing delimiter (required in delimiters mode)." }
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
