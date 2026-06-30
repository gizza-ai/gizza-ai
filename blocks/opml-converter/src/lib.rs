//! gizza-ai/opml-converter — convert OPML subscription files between OPML, JSON,
//! and CSV in any direction. Chat schema single-sourced from descriptor() (which
//! also drives the CLI); handle() delegates to block_utils::run_skill. Pure →
//! runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_opml_converter_core::{convert, Format};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    from: String,
    to: String,
    #[serde(default = "default_true")]
    pretty: bool,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The OPML, JSON, or CSV data to convert. OPML is the XML outline format used by podcast/RSS subscription exports; JSON is the faithful tree {\"title\":…,\"outlines\":[…]}; CSV is the flat feed list with a category column."),
        )
        .param(
            Param::enumv("from", ["opml", "json", "csv"])
                .required()
                .default("opml")
                .describe("Source format of the input."),
        )
        .param(
            Param::enumv("to", ["opml", "json", "csv"])
                .required()
                .default("json")
                .describe("Target format to convert to."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Indent the OPML/JSON output. Set false for compact single-line output (ignored for CSV). Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct OpmlConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/opml-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert OPML subscription files to/from JSON and CSV",
    skill(
        description = "Convert OPML subscription files (podcast/RSS feed exports) between OPML, JSON, and CSV in any direction. Specify from and to (opml/json/csv). OPML ⇄ JSON faithfully round-trips the whole outline tree (each <outline> becomes a JSON object of its attributes — text, type, xmlUrl, htmlUrl — plus a nested \"outlines\" array). CSV is the flat feed list: one row per feed with a category column recording the folder path, so folders round-trip back to nested OPML/JSON. pretty indents OPML/JSON output (default true). Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl OpmlConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "opml-converter", |a: Args| {
            let from = Format::parse(&a.from).map_err(SkillError::InvalidArgs)?;
            let to = Format::parse(&a.to).map_err(SkillError::InvalidArgs)?;
            convert(&a.input, from, to, a.pretty).map_err(SkillError::InvalidArgs)
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
                    "input":  { "type": "string", "description": "The OPML, JSON, or CSV data to convert. OPML is the XML outline format used by podcast/RSS subscription exports; JSON is the faithful tree {\"title\":…,\"outlines\":[…]}; CSV is the flat feed list with a category column." },
                    "from":   { "type": "string", "enum": ["opml", "json", "csv"], "default": "opml", "description": "Source format of the input." },
                    "to":     { "type": "string", "enum": ["opml", "json", "csv"], "default": "json", "description": "Target format to convert to." },
                    "pretty": { "type": "boolean", "default": true, "description": "Indent the OPML/JSON output. Set false for compact single-line output (ignored for CSV). Default true." }
                },
                "required": ["input", "from", "to"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
