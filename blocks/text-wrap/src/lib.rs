//! gizza-ai/text-wrap — chat skill block on the shared tool abstraction.
//! Hard-wraps text to a column width. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    /// Target column width; the core clamps to MIN_WIDTH..=MAX_WIDTH.
    #[serde(default = "default_width")]
    width: u32,
    /// Re-apply each source line's leading indentation to its continuation
    /// lines (default true).
    #[serde(default = "default_true")]
    preserve_indent: bool,
    /// Hard-split a word longer than the width (default true). When false, an
    /// over-long word stays on its own over-length line.
    #[serde(default = "default_true")]
    break_long_words: bool,
}

fn default_width() -> u32 {
    80
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to hard-wrap. Existing line breaks and blank lines are preserved; each line is reflowed independently."),
        )
        .param(
            // Bounds reference the core clamp so the LLM-facing schema can't
            // drift from what `wrap` actually enforces.
            Param::integer("width")
                .default(80)
                .min(gizza_ai_text_wrap_core::MIN_WIDTH as f64)
                .max(gizza_ai_text_wrap_core::MAX_WIDTH as f64)
                .describe("Maximum line width in characters (default 80). No output line will exceed this."),
        )
        .param(
            Param::boolean("preserve_indent")
                .default(true)
                .describe("When true (default), the leading whitespace of each source line is re-applied to every continuation line it produces, keeping indented blocks and list items aligned."),
        )
        .param(
            Param::boolean("break_long_words")
                .default(true)
                .describe("When true (default), a single word longer than the width is hard-split at the column. When false, such a word is left intact on its own over-length line."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-wrap",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Hard-wrap text to a column width",
    skill(
        description = "Hard-wrap (reflow) text to a fixed column width so no line exceeds the given number of characters. Set width (default 80) to the target column. Existing hard line breaks and blank lines are kept, so paragraph structure survives, and each line is reflowed independently using greedy word wrap (runs of whitespace between words collapse to a single space). preserve_indent (default true) re-applies each source line's leading indentation to its continuation lines so indented blocks and list items stay aligned. break_long_words (default true) hard-splits a word that is longer than the width; set it false to leave such a word intact on its own over-length line. Useful for fitting prose to a terminal/email column, formatting commit messages, or normalizing paragraph width.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-wrap", |a: Args| {
            gizza_ai_text_wrap_core::wrap(&a.text, a.width, a.preserve_indent, a.break_long_words)
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
                    "text": { "type": "string", "description": "The text to hard-wrap. Existing line breaks and blank lines are preserved; each line is reflowed independently." },
                    "width": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 80, "description": "Maximum line width in characters (default 80). No output line will exceed this." },
                    "preserve_indent": { "type": "boolean", "default": true, "description": "When true (default), the leading whitespace of each source line is re-applied to every continuation line it produces, keeping indented blocks and list items aligned." },
                    "break_long_words": { "type": "boolean", "default": true, "description": "When true (default), a single word longer than the width is hard-split at the column. When false, such a word is left intact on its own over-length line." }
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
