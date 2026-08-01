//! gizza-ai/ast-diff — a structural (AST-level) diff for Rust source.
//!
//! Thin chat-skill wrapper around `gizza-ai-ast-diff-core`. Both sources are
//! parsed into a `syn` AST and re-emitted in one canonical form, so pure
//! reformatting / whitespace / comment changes never show up as differences.
//! The chat schema is single-sourced from `descriptor()` (shared with the CLI);
//! `handle()` delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

/// Default unified-diff context when the caller omits `context`. Mirrors the
/// descriptor default so chat/CLI/page all behave the same when it's unset.
const DEFAULT_CONTEXT: u32 = 3;

#[derive(Deserialize)]
struct Args {
    a: String,
    b: String,
    #[serde(default)]
    mode: String,
    /// Unified-diff context lines; `None` (omitted) → `DEFAULT_CONTEXT`.
    #[serde(default)]
    context: Option<u32>,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("a")
                .required()
                .describe("First Rust source (the 'before' / original). A whole file or a paste-able snippet — must be syntactically valid Rust."),
        )
        .param(
            Param::string("b")
                .required()
                .describe("Second Rust source (the 'after' / modified). Same rules as 'a'."),
        )
        .param(
            Param::enumv("mode", ["unified", "summary", "canonical-a", "canonical-b"])
                .default("unified")
                .describe("Output style. 'unified' (default) is a git-style unified diff of the two canonical forms; 'summary' is a one-line verdict (identical / formatting-only / N added, M removed); 'canonical-a' and 'canonical-b' return one source re-formatted into canonical form without diffing."),
        )
        .param(
            Param::integer("context")
                .default(DEFAULT_CONTEXT as i64)
                .min(0.0)
                .max(100.0)
                .describe("Unified-diff mode only: number of unchanged context lines kept around each change, 0-100. Default 3."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AstDiff;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ast-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Structural AST diff of two Rust sources — ignores reformatting.",
    skill(
        description = "Compute a structural (AST-level) diff between two Rust source files. Both inputs are parsed into an abstract syntax tree and re-emitted in one canonical form, so changes to formatting, indentation, whitespace, or line comments never appear as differences — only changes the compiler would see (renamed items, changed expressions, added/removed code) survive. Pass 'a' (the original source) and 'b' (the modified source). mode='unified' (default) returns a git-style unified diff; mode='summary' returns a one-line verdict (identical / formatting-only / N added, M removed); mode='canonical-a'/'canonical-b' return one source re-formatted without diffing. Inputs must be valid Rust; the error names the line/column on a parse failure.",
        parameters = schema_json()
    )
)]
impl AstDiff {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ast-diff", |a: Args| {
            let context = a.context.unwrap_or(DEFAULT_CONTEXT) as usize;
            gizza_ai_ast_diff_core::diff(&a.a, &a.b, &a.mode, context)
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
                    "a": { "type": "string", "description": "First Rust source (the 'before' / original). A whole file or a paste-able snippet — must be syntactically valid Rust." },
                    "b": { "type": "string", "description": "Second Rust source (the 'after' / modified). Same rules as 'a'." },
                    "mode": { "type": "string", "enum": ["unified", "summary", "canonical-a", "canonical-b"], "default": "unified", "description": "Output style. 'unified' (default) is a git-style unified diff of the two canonical forms; 'summary' is a one-line verdict (identical / formatting-only / N added, M removed); 'canonical-a' and 'canonical-b' return one source re-formatted into canonical form without diffing." },
                    "context": { "type": "integer", "minimum": 0, "maximum": 100, "default": 3, "description": "Unified-diff mode only: number of unchanged context lines kept around each change, 0-100. Default 3." }
                },
                "required": ["a", "b"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
