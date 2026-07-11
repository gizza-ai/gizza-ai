//! gizza-ai/list-set-diff — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    list_a: String,
    list_b: String,
    #[serde(default)]
    separator: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default = "default_true")]
    trim: bool,
    #[serde(default = "default_true")]
    ignore_blank: bool,
    #[serde(default = "default_true")]
    dedupe: bool,
    #[serde(default)]
    ignore_leading_zeros: bool,
    #[serde(default)]
    sort: String,
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("list_a")
                .required()
                .describe("The first list (\"A\"). Items separated per the `separator` param — one per line by default."),
        )
        .param(
            Param::string("list_b")
                .required()
                .describe("The second list (\"B\"), same format as list_a."),
        )
        .param(
            Param::enumv("separator", ["newline", "comma", "tab", "semicolon", "pipe", "space"])
                .default("newline")
                .describe("How items are split within each list: 'newline' (default, one per line), 'comma', 'tab', 'semicolon', 'pipe', or 'space'."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Compare items case-insensitively (Apple == apple). Default false."),
        )
        .param(
            Param::boolean("trim")
                .default(true)
                .describe("Strip leading/trailing whitespace from each item before comparing. Default true."),
        )
        .param(
            Param::boolean("ignore_blank")
                .default(true)
                .describe("Drop empty items (e.g. blank lines). Default true."),
        )
        .param(
            Param::boolean("dedupe")
                .default(true)
                .describe("Collapse repeated items within each result section (set semantics). Turn off to keep every occurrence. Default true."),
        )
        .param(
            Param::boolean("ignore_leading_zeros")
                .default(false)
                .describe("Treat leading zeros as insignificant so 007 and 7 match — handy for numeric IDs. Default false."),
        )
        .param(
            Param::enumv("sort", ["input", "asc", "desc"])
                .default("input")
                .describe("Ordering of each result section: 'input' (default, first-seen order), 'asc' (A→Z), or 'desc' (Z→A)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/list-set-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compare two lists and show only-in-A, only-in-B, and shared items.",
    skill(
        description = "Compare two lists as sets and report which items are only in list A, only in list B, and shared by both — each with a count, plus a totals line (including the union size). Items are split by `separator` (newline by default, or comma/tab/semicolon/pipe/space). Normalization options: `ignore_case`, `trim` whitespace, `ignore_blank` empty items, `dedupe` repeats within a list (set semantics), and `ignore_leading_zeros` (007 == 7). `sort` orders each section (input/asc/desc). Great for diffing email lists, IDs, SKUs, or any two collections.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "list-set-diff", |a: Args| {
            gizza_ai_list_set_diff_core::diff(
                &a.list_a,
                &a.list_b,
                &a.separator,
                a.ignore_case,
                a.trim,
                a.ignore_blank,
                a.dedupe,
                a.ignore_leading_zeros,
                &a.sort,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "list_a": { "type": "string", "description": "The first list (\"A\"). Items separated per the `separator` param — one per line by default." },
                    "list_b": { "type": "string", "description": "The second list (\"B\"), same format as list_a." },
                    "separator": { "type": "string", "enum": ["newline", "comma", "tab", "semicolon", "pipe", "space"], "default": "newline", "description": "How items are split within each list: 'newline' (default, one per line), 'comma', 'tab', 'semicolon', 'pipe', or 'space'." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Compare items case-insensitively (Apple == apple). Default false." },
                    "trim": { "type": "boolean", "default": true, "description": "Strip leading/trailing whitespace from each item before comparing. Default true." },
                    "ignore_blank": { "type": "boolean", "default": true, "description": "Drop empty items (e.g. blank lines). Default true." },
                    "dedupe": { "type": "boolean", "default": true, "description": "Collapse repeated items within each result section (set semantics). Turn off to keep every occurrence. Default true." },
                    "ignore_leading_zeros": { "type": "boolean", "default": false, "description": "Treat leading zeros as insignificant so 007 and 7 match — handy for numeric IDs. Default false." },
                    "sort": { "type": "string", "enum": ["input", "asc", "desc"], "default": "input", "description": "Ordering of each result section: 'input' (default, first-seen order), 'asc' (A→Z), or 'desc' (Z→A)." }
                },
                "required": ["list_a", "list_b"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
