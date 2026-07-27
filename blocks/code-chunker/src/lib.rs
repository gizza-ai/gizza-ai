//! gizza-ai/code-chunker — chat skill block on the shared tool abstraction.
//! Splits source code into function-/class-aligned chunks for embedding or an
//! LLM context window. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to block_utils::run_skill and
//! the pure logic lives in gizza-ai-code-chunker-core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_language() -> String {
    "auto".into()
}
fn default_max_lines() -> u32 {
    gizza_ai_code_chunker_core::DEFAULT_MAX_LINES
}
fn default_format() -> String {
    "json".into()
}

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_max_lines")]
    max_lines: u32,
    #[serde(default = "default_format")]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The source code to split into function-/class-aligned chunks."),
        )
        .param(
            Param::enumv(
                "language",
                [
                    "auto",
                    "python",
                    "javascript",
                    "typescript",
                    "rust",
                    "go",
                    "java",
                    "c",
                    "cpp",
                    "csharp",
                    "php",
                    "swift",
                ],
            )
            .default("auto")
            .describe("Language of the code, which selects the boundary strategy. 'auto' (default) guesses: Python (indentation) vs a brace language (bracket balancing). Brace languages share one strategy; Rust is handled specially so lifetimes aren't mistaken for strings."),
        )
        .param(
            Param::integer("max_lines")
                .default(gizza_ai_code_chunker_core::DEFAULT_MAX_LINES as i64)
                .min(gizza_ai_code_chunker_core::MIN_MAX_LINES as f64)
                .max(gizza_ai_code_chunker_core::MAX_MAX_LINES as f64)
                .describe("Target maximum lines per chunk. Consecutive top-level constructs are packed together up to this many lines; a single construct larger than this is emitted whole (never split mid-definition) and flagged 'oversize'. Default 60."),
        )
        .param(
            Param::enumv("format", ["json", "jsonl", "text"])
                .default("json")
                .describe("Output format. 'json' (default) is a pretty JSON array of {index, start_line, end_line, line_count, kind, name, oversize, text}; 'jsonl' is one compact object per line; 'text' is each chunk under a header banner."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/code-chunker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split source code into function- and class-aligned chunks for embedding or an LLM context window.",
    skill(
        description = "Split source code into function-/class-aligned chunks suitable for embedding or an LLM context window (RAG over code). Boundaries come from a dependency-free heuristic: bracket balancing for brace languages (JS/TS/Rust/Go/Java/C/C++/C#/PHP/Swift, ignoring brackets in strings and comments) and indentation for Python; leading comments/decorators fold into the construct they precede. language='auto' (default) guesses the language. Consecutive constructs are packed into chunks up to max_lines (default 60); a construct larger than that is kept whole and flagged oversize (definitions are never split). format='json' (default) returns an array of {index, start_line, end_line, line_count, kind, name, oversize, text}; 'jsonl' is one object per line; 'text' banners each chunk. Chunks do not overlap.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "code-chunker", |a: Args| {
            gizza_ai_code_chunker_core::chunk(&a.code, &a.language, a.max_lines, &a.format)
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
    /// schema so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "code": { "type": "string", "description": "The source code to split into function-/class-aligned chunks." },
                    "language": { "type": "string", "enum": ["auto", "python", "javascript", "typescript", "rust", "go", "java", "c", "cpp", "csharp", "php", "swift"], "default": "auto", "description": "Language of the code, which selects the boundary strategy. 'auto' (default) guesses: Python (indentation) vs a brace language (bracket balancing). Brace languages share one strategy; Rust is handled specially so lifetimes aren't mistaken for strings." },
                    "max_lines": { "type": "integer", "minimum": 1, "maximum": 100000, "default": 60, "description": "Target maximum lines per chunk. Consecutive top-level constructs are packed together up to this many lines; a single construct larger than this is emitted whole (never split mid-definition) and flagged 'oversize'. Default 60." },
                    "format": { "type": "string", "enum": ["json", "jsonl", "text"], "default": "json", "description": "Output format. 'json' (default) is a pretty JSON array of {index, start_line, end_line, line_count, kind, name, oversize, text}; 'jsonl' is one compact object per line; 'text' is each chunk under a header banner." }
                },
                "required": ["code"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
