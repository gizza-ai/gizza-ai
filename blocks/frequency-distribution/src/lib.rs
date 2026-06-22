//! gizza-ai/frequency-distribution — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_frequency_distribution_core::{distribute, InputKind, Mode};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_input_kind")]
    input_kind: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_n")]
    n: u32,
    #[serde(default = "default_true")]
    case_sensitive: bool,
}
fn default_input_kind() -> String {
    "text".to_string()
}
fn default_mode() -> String {
    "char".to_string()
}
fn default_n() -> u32 {
    2
}
fn default_true() -> bool {
    true
}

fn parse_kind(s: &str) -> Result<InputKind, SkillError> {
    match s {
        "text" => Ok(InputKind::Text),
        "hex" => Ok(InputKind::Hex),
        other => Err(SkillError::InvalidArgs(format!(
            "input_kind must be 'text' or 'hex', got '{other}'"
        ))),
    }
}

fn parse_mode(s: &str, n: u32) -> Result<Mode, SkillError> {
    match s {
        "char" => Ok(Mode::Char),
        "byte" => Ok(Mode::Byte),
        "ngram" => Ok(Mode::Ngram(n as usize)),
        other => Err(SkillError::InvalidArgs(format!(
            "mode must be 'char', 'byte', or 'ngram', got '{other}'"
        ))),
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The text (or hex string, if input_kind=hex) to analyze."),
        )
        .param(
            Param::enumv("input_kind", ["text", "hex"])
                .default("text")
                .describe("How to read 'input': 'text' (default) literally, or 'hex' to decode a hex string into bytes first."),
        )
        .param(
            Param::enumv("mode", ["char", "byte", "ngram"])
                .default("char")
                .describe("What to tally: 'char' (Unicode characters, default), 'byte' (raw bytes), or 'ngram' (overlapping n-character windows)."),
        )
        .param(
            Param::integer("n")
                .min(1.0)
                .default(2)
                .describe("Window size for mode=ngram (e.g. 2 = bigrams). Ignored for char/byte modes."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(true)
                .describe("When true (default), 'A' and 'a' count separately; false groups them (char/ngram modes only — byte mode is always raw)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FrequencyDistribution;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/frequency-distribution",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a character, byte, or n-gram frequency table",
    skill(
        description = "Build a frequency table over the symbols of an input, ranked most to least frequent with counts and percentages. mode='char' tallies Unicode characters (default), 'byte' tallies raw bytes (shown as 0xNN), 'ngram' tallies overlapping n-character windows (set n, default 2). input_kind='text' reads input literally (default); 'hex' decodes a hex string into bytes first. case_sensitive (default true) controls whether 'A'/'a' are grouped (char/ngram only). Returns each distinct symbol with its count and percent of the total, plus the number of distinct symbols and the grand total. Runs locally.",
        parameters = schema_json()
    ),
)]
impl FrequencyDistribution {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "frequency-distribution", |a: Args| {
            let kind = parse_kind(&a.input_kind)?;
            let mode = parse_mode(&a.mode, a.n)?;
            distribute(&a.input, kind, mode, a.case_sensitive).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "The text (or hex string, if input_kind=hex) to analyze." },
                    "input_kind": { "type": "string", "enum": ["text", "hex"], "default": "text", "description": "How to read 'input': 'text' (default) literally, or 'hex' to decode a hex string into bytes first." },
                    "mode": { "type": "string", "enum": ["char", "byte", "ngram"], "default": "char", "description": "What to tally: 'char' (Unicode characters, default), 'byte' (raw bytes), or 'ngram' (overlapping n-character windows)." },
                    "n": { "type": "integer", "minimum": 1, "default": 2, "description": "Window size for mode=ngram (e.g. 2 = bigrams). Ignored for char/byte modes." },
                    "case_sensitive": { "type": "boolean", "default": true, "description": "When true (default), 'A' and 'a' count separately; false groups them (char/ngram modes only — byte mode is always raw)." }
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
