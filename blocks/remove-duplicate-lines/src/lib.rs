//! gizza-ai/remove-duplicate-lines — remove duplicate lines from text, returning
//! the deduplicated text plus a small summary. Chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_remove_duplicate_lines_core::{dedupe, Keep, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    trim: bool,
    #[serde(default)]
    adjacent_only: bool,
    #[serde(default = "default_keep")]
    keep: String,
    #[serde(default)]
    remove_empty: bool,
}

fn default_keep() -> String {
    "first".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text whose duplicate lines should be removed."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Compare lines case-insensitively when detecting duplicates."),
        )
        .param(
            Param::boolean("trim")
                .default(false)
                .describe("Trim leading/trailing whitespace from each line before comparing, and from the kept output."),
        )
        .param(
            Param::boolean("adjacent_only")
                .default(false)
                .describe("Only collapse runs of consecutive identical lines (like the Unix `uniq` command); a line that reappears later is kept."),
        )
        .param(
            Param::enumv("keep", ["first", "last"])
                .default("first")
                .describe("Which occurrence of a duplicated line to keep: the first (default) or the last."),
        )
        .param(
            Param::boolean("remove_empty")
                .default(false)
                .describe("Also drop empty (or whitespace-only, when trim is on) lines."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RemoveDuplicateLines;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/remove-duplicate-lines",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove duplicate lines from text",
    skill(
        description = "Remove duplicate lines from the text and return the deduplicated text plus total/kept/removed line counts. By default the first occurrence of each line is kept and order is preserved. Set ignore_case=true to compare case-insensitively, trim=true to ignore surrounding whitespace, adjacent_only=true to only collapse consecutive duplicates (like Unix `uniq`), keep=\"last\" to keep the last occurrence instead of the first, and remove_empty=true to also drop blank lines. Runs locally.",
        parameters = schema_json()
    ),
)]
impl RemoveDuplicateLines {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "remove-duplicate-lines", |a: Args| {
            let keep = Keep::parse(&a.keep).map_err(SkillError::InvalidArgs)?;
            let opts = Options {
                ignore_case: a.ignore_case,
                trim: a.trim,
                adjacent_only: a.adjacent_only,
                keep,
                remove_empty: a.remove_empty,
            };
            Ok::<_, SkillError>(dedupe(&a.text, &opts))
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
                    "text": { "type": "string", "description": "The text whose duplicate lines should be removed." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Compare lines case-insensitively when detecting duplicates." },
                    "trim": { "type": "boolean", "default": false, "description": "Trim leading/trailing whitespace from each line before comparing, and from the kept output." },
                    "adjacent_only": { "type": "boolean", "default": false, "description": "Only collapse runs of consecutive identical lines (like the Unix `uniq` command); a line that reappears later is kept." },
                    "keep": { "type": "string", "enum": ["first", "last"], "default": "first", "description": "Which occurrence of a duplicated line to keep: the first (default) or the last." },
                    "remove_empty": { "type": "boolean", "default": false, "description": "Also drop empty (or whitespace-only, when trim is on) lines." }
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
