//! gizza-ai/list-dedupe-merge — chat skill block on the shared tool abstraction.
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
    merge_order: String,
    #[serde(default = "default_true")]
    trim: bool,
    #[serde(default = "default_true")]
    ignore_blank: bool,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    ignore_leading_zeros: bool,
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
            Param::enumv("merge_order", ["append", "interleave"])
                .default("append")
                .describe("How the two lists are combined before de-duplicating: 'append' (default) puts all of A then all of B; 'interleave' alternates A, B, A, B… The first occurrence of a duplicate is the one kept."),
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
            Param::boolean("ignore_case")
                .default(false)
                .describe("Match items case-insensitively (Apple == apple). The kept item shows the first occurrence's original case. Default false."),
        )
        .param(
            Param::enumv("sort", ["input", "asc", "desc"])
                .default("input")
                .describe("Ordering of the merged list: 'input' (default, first-seen order), 'asc' (A→Z), or 'desc' (Z→A)."),
        )
        .param(
            Param::boolean("ignore_leading_zeros")
                .default(false)
                .describe("Treat leading zeros as insignificant so 007 and 7 match — handy for numeric IDs. Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/list-dedupe-merge",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Merge two lists into one deduplicated list and count the overlap.",
    skill(
        description = "Merge two lists into a single de-duplicated list (the set union) and report how many overlapping entries were collapsed. Items are split by `separator` (newline by default, or comma/tab/semicolon/pipe/space). `merge_order` controls how the lists are combined before de-duplicating: 'append' (all of A then all of B) or 'interleave' (A, B, A, B…); the first occurrence of a duplicate is kept. Normalization options: `trim` whitespace, `ignore_blank` empty items, `ignore_case` (Apple == apple), and `ignore_leading_zeros` (007 == 7). `sort` orders the merged list (input/asc/desc). The totals line reports each list's size, the merged size, how many duplicates were removed, and how many entries were shared by both lists. Great for combining email lists, IDs, SKUs, tags, or any two collections into one clean list.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "list-dedupe-merge", |a: Args| {
            gizza_ai_list_dedupe_merge_core::merge(
                &a.list_a,
                &a.list_b,
                &a.separator,
                &a.merge_order,
                a.trim,
                a.ignore_blank,
                a.ignore_case,
                &a.sort,
                a.ignore_leading_zeros,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "list_a": { "type": "string", "description": "The first list (\"A\"). Items separated per the `separator` param — one per line by default." },
                    "list_b": { "type": "string", "description": "The second list (\"B\"), same format as list_a." },
                    "separator": { "type": "string", "enum": ["newline", "comma", "tab", "semicolon", "pipe", "space"], "default": "newline", "description": "How items are split within each list: 'newline' (default, one per line), 'comma', 'tab', 'semicolon', 'pipe', or 'space'." },
                    "merge_order": { "type": "string", "enum": ["append", "interleave"], "default": "append", "description": "How the two lists are combined before de-duplicating: 'append' (default) puts all of A then all of B; 'interleave' alternates A, B, A, B… The first occurrence of a duplicate is the one kept." },
                    "trim": { "type": "boolean", "default": true, "description": "Strip leading/trailing whitespace from each item before comparing. Default true." },
                    "ignore_blank": { "type": "boolean", "default": true, "description": "Drop empty items (e.g. blank lines). Default true." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Match items case-insensitively (Apple == apple). The kept item shows the first occurrence's original case. Default false." },
                    "sort": { "type": "string", "enum": ["input", "asc", "desc"], "default": "input", "description": "Ordering of the merged list: 'input' (default, first-seen order), 'asc' (A→Z), or 'desc' (Z→A)." },
                    "ignore_leading_zeros": { "type": "boolean", "default": false, "description": "Treat leading zeros as insignificant so 007 and 7 match — handy for numeric IDs. Default false." }
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
