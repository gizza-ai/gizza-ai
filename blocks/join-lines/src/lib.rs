//! gizza-ai/join-lines — chat skill block. Joins the lines of text into one line.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_join_lines_core::join;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default)]
    trim: bool,
    #[serde(default)]
    remove_blank: bool,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
}

fn default_separator() -> String {
    ", ".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The multi-line text whose lines should be joined into one."),
        )
        .param(
            Param::string("separator")
                .default(", ")
                .describe("String placed between lines. Escapes \\t, \\n, \\r and \\\\ are decoded, so use \\t for a tab or an empty string to concatenate with no separator."),
        )
        .param(
            Param::boolean("trim")
                .default(false)
                .describe("Strip leading/trailing whitespace from each line before joining."),
        )
        .param(
            Param::boolean("remove_blank")
                .default(false)
                .describe("Drop blank/whitespace-only lines instead of joining them."),
        )
        .param(
            Param::string("prefix")
                .default("")
                .describe("Text added to the start of every line before joining (e.g. a quote for a CSV list)."),
        )
        .param(
            Param::string("suffix")
                .default("")
                .describe("Text added to the end of every line before joining."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JoinLines;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/join-lines",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Join multiple lines of text into a single line with a chosen separator",
    skill(
        description = "Join the lines of a block of text into one line. separator (default \", \") is placed between lines; backslash escapes \\t, \\n, \\r and \\\\ are decoded, and an empty separator concatenates with nothing between. Set trim to strip surrounding whitespace from each line, remove_blank to drop blank lines, and prefix/suffix to wrap each line (e.g. quotes for a CSV list or parentheses for a SQL IN clause). Returns the joined line plus line counts. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JoinLines {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "join-lines", |a: Args| {
            join(
                &a.text,
                &a.separator,
                a.trim,
                a.remove_blank,
                &a.prefix,
                &a.suffix,
            )
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
                    "text": { "type": "string", "description": "The multi-line text whose lines should be joined into one." },
                    "separator": { "type": "string", "default": ", ", "description": "String placed between lines. Escapes \\t, \\n, \\r and \\\\ are decoded, so use \\t for a tab or an empty string to concatenate with no separator." },
                    "trim": { "type": "boolean", "default": false, "description": "Strip leading/trailing whitespace from each line before joining." },
                    "remove_blank": { "type": "boolean", "default": false, "description": "Drop blank/whitespace-only lines instead of joining them." },
                    "prefix": { "type": "string", "default": "", "description": "Text added to the start of every line before joining (e.g. a quote for a CSV list)." },
                    "suffix": { "type": "string", "default": "", "description": "Text added to the end of every line before joining." }
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
