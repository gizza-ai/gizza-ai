//! gizza-ai/list-converter — reformat a list between comma/newline/bulleted/
//! numbered/quoted/space forms with optional sort/dedupe. Thin wrapper; chat
//! schema single-sourced from descriptor(); handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_list_converter_core::{convert, parse_in_sep, parse_out_format};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_separator: String,
    #[serde(default = "default_out")]
    output_format: String,
    #[serde(default)]
    sort: bool,
    #[serde(default)]
    dedupe: bool,
}
fn default_out() -> String {
    "newline".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The list text to reformat."))
        .param(Param::enumv("input_separator", ["auto", "comma", "newline", "semicolon", "space"]).default("auto").describe("How to split the input. 'auto' (default): newlines if present, else commas, else semicolons."))
        .param(Param::enumv("output_format", ["comma", "newline", "bulleted", "numbered", "quoted", "space"]).default("newline").describe("Output layout: comma (a, b), newline (one per line), bulleted (- a), numbered (1. a), quoted (\"a\", \"b\"), or space. Default newline."))
        .param(Param::boolean("sort").default(false).describe("Sort items alphabetically (case-insensitive). Default false."))
        .param(Param::boolean("dedupe").default(false).describe("Remove duplicate items (keeping the first). Default false."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ListConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/list-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reformat a list (comma/newline/bullets/numbered/quoted)",
    skill(
        description = "Reformat a list between forms: comma-separated, newline (one per line), bulleted, numbered, quoted (\"a\", \"b\" for code arrays), or space-separated. input_separator controls splitting ('auto' detects newlines/commas/semicolons); output_format picks the layout. Optionally sort (case-insensitive) and/or dedupe. Items are trimmed and blanks dropped.",
        parameters = schema_json()
    )
)]
impl ListConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "list-converter", |a: Args| {
            let insep = parse_in_sep(&a.input_separator).map_err(SkillError::InvalidArgs)?;
            let outf = parse_out_format(&a.output_format).map_err(SkillError::InvalidArgs)?;
            convert(&a.input, insep, outf, a.sort, a.dedupe).map_err(SkillError::InvalidArgs)
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
                    "input":            { "type": "string", "description": "The list text to reformat." },
                    "input_separator":  { "type": "string", "enum": ["auto", "comma", "newline", "semicolon", "space"], "default": "auto", "description": "How to split the input. 'auto' (default): newlines if present, else commas, else semicolons." },
                    "output_format":    { "type": "string", "enum": ["comma", "newline", "bulleted", "numbered", "quoted", "space"], "default": "newline", "description": "Output layout: comma (a, b), newline (one per line), bulleted (- a), numbered (1. a), quoted (\"a\", \"b\"), or space. Default newline." },
                    "sort":             { "type": "boolean", "default": false, "description": "Sort items alphabetically (case-insensitive). Default false." },
                    "dedupe":           { "type": "boolean", "default": false, "description": "Remove duplicate items (keeping the first). Default false." }
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
