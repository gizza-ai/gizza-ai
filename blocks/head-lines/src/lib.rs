//! gizza-ai/head-lines — chat skill block on the shared tool abstraction.
//! Outputs the first N lines of text (the `head -n` operation), with optional
//! leading-line skipping and line numbering. The chat schema is single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. No host calls — runs entirely in the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    /// Number of leading lines to keep; the core clamps to 1..=1_000_000 (0 → 1).
    #[serde(default)]
    count: u32,
    /// Drop this many lines from the start before taking `count`.
    #[serde(default)]
    skip: u32,
    /// Prefix each kept line with its 1-based line number and a tab.
    #[serde(default)]
    number: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to take the first lines from (split on newlines)."),
        )
        .param(
            // Bounds reference the core clamp (MAX_COUNT) so the LLM-facing
            // schema can't drift from what `head` actually enforces.
            Param::integer("count")
                .default(10)
                .min(1.0)
                .max(gizza_ai_head_lines_core::MAX_COUNT as f64)
                .describe("How many leading lines to keep (like `head -n`). Default 10."),
        )
        .param(
            Param::integer("skip")
                .default(0)
                .min(0.0)
                .describe("Drop this many lines from the start before taking `count` (like `tail -n +K`). Default 0."),
        )
        .param(
            Param::boolean("number")
                .default(false)
                .describe("When true, prefix each kept line with its 1-based original line number and a tab (like `cat -n`). Default false."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/head-lines",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Output the first N lines of text (head -n).",
    skill(
        description = "Output the first N lines of text — the `head -n` / 'show the top rows' operation. Set count to how many leading lines to keep (default 10). Use skip to drop that many lines from the start before taking count (like `tail -n +K`, e.g. skip=1 to ignore a header row). Set number=true to prefix each kept line with its 1-based original line number and a tab (like `cat -n`). Lines split on newlines; a trailing newline and any Windows CRLF are preserved.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "head-lines", |a: Args| {
            gizza_ai_head_lines_core::head(&a.text, a.count, a.skip, a.number)
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
                    "text": { "type": "string", "description": "The text to take the first lines from (split on newlines)." },
                    "count": { "type": "integer", "minimum": 1, "maximum": 1000000, "default": 10, "description": "How many leading lines to keep (like `head -n`). Default 10." },
                    "skip": { "type": "integer", "minimum": 0, "default": 0, "description": "Drop this many lines from the start before taking `count` (like `tail -n +K`). Default 0." },
                    "number": { "type": "boolean", "default": false, "description": "When true, prefix each kept line with its 1-based original line number and a tab (like `cat -n`). Default false." }
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
