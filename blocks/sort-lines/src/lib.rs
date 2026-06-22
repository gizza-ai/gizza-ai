//! gizza-ai/sort-lines — chat skill block. Sorts the lines of a block of text.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_sort_lines_core::{sort, Method, Order};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_order")]
    order: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    trim: bool,
    #[serde(default)]
    unique: bool,
    #[serde(default)]
    remove_blank: bool,
}

fn default_method() -> String {
    "alpha".to_string()
}
fn default_order() -> String {
    "asc".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text whose lines should be sorted."),
        )
        .param(
            Param::enumv("method", ["alpha", "numeric", "natural", "length"])
                .default("alpha")
                .describe("alpha = lexicographic; numeric = by the leading number on each line; natural = digit runs compared as numbers (file2 before file10); length = by line length."),
        )
        .param(
            Param::enumv("order", ["asc", "desc"])
                .default("asc")
                .describe("asc = ascending (A→Z, smallest first); desc = descending."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Compare case-insensitively."),
        )
        .param(
            Param::boolean("trim")
                .default(false)
                .describe("Ignore leading/trailing whitespace when comparing lines."),
        )
        .param(
            Param::boolean("unique")
                .default(false)
                .describe("Remove duplicate lines after sorting (respects ignore_case/trim)."),
        )
        .param(
            Param::boolean("remove_blank")
                .default(false)
                .describe("Drop blank/whitespace-only lines before sorting."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SortLines;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sort-lines",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sort the lines of text alphabetically, numerically, or by length",
    skill(
        description = "Sort the lines of a block of text. method=alpha (default) sorts lexicographically; numeric sorts by the leading number on each line; natural compares digit runs as numbers (so file2 comes before file10); length sorts by line length. order=asc (default) or desc. Set ignore_case to compare case-insensitively, trim to ignore surrounding whitespace, unique to drop duplicate lines, and remove_blank to discard blank lines. Returns the sorted text plus line counts. Runs locally.",
        parameters = schema_json()
    ),
)]
impl SortLines {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sort-lines", |a: Args| {
            let method = Method::parse(&a.method).map_err(SkillError::InvalidArgs)?;
            let order = Order::parse(&a.order).map_err(SkillError::InvalidArgs)?;
            sort(
                &a.text,
                method,
                order,
                a.ignore_case,
                a.trim,
                a.unique,
                a.remove_blank,
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
                    "text": { "type": "string", "description": "The text whose lines should be sorted." },
                    "method": { "type": "string", "enum": ["alpha", "numeric", "natural", "length"], "default": "alpha", "description": "alpha = lexicographic; numeric = by the leading number on each line; natural = digit runs compared as numbers (file2 before file10); length = by line length." },
                    "order": { "type": "string", "enum": ["asc", "desc"], "default": "asc", "description": "asc = ascending (A→Z, smallest first); desc = descending." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Compare case-insensitively." },
                    "trim": { "type": "boolean", "default": false, "description": "Ignore leading/trailing whitespace when comparing lines." },
                    "unique": { "type": "boolean", "default": false, "description": "Remove duplicate lines after sorting (respects ignore_case/trim)." },
                    "remove_blank": { "type": "boolean", "default": false, "description": "Drop blank/whitespace-only lines before sorting." }
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
