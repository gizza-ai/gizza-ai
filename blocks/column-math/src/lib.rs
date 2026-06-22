//! gizza-ai/column-math — element-wise arithmetic between two numeric columns.
//!
//! Thin chat-skill wrapper around `gizza-ai-column-math-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    a: String,
    b: String,
    #[serde(default)]
    operation: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("a")
                .required()
                .describe("Column A — the numbers, separated by commas, spaces, or newlines (one per line)."),
        )
        .param(
            Param::string("b")
                .required()
                .describe("Column B — the second set of numbers, same length as A, separated by commas, spaces, or newlines."),
        )
        .param(
            Param::enumv("operation", ["add", "subtract", "multiply", "divide"])
                .default("add")
                .describe("Element-wise operation to apply between A and B, row by row: 'add' (default) computes A+B, 'subtract' A-B, 'multiply' A*B, 'divide' A/B (a zero divisor is an error)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ColumnMath;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/column-math",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Element-wise math between two numeric columns.",
    skill(
        description = "Perform element-wise arithmetic between two equal-length numeric columns. Pass column A in 'a' and column B in 'b' (numbers separated by commas, spaces, or newlines), and choose operation='add' (default, A+B), 'subtract' (A-B), 'multiply' (A*B), or 'divide' (A/B). The columns must have the same number of values; a zero divisor on divide is an error. Returns the per-row results as a newline-separated list. Use it for spreadsheet-style column math — summing two columns, computing differences, scaling, or ratios.",
        parameters = schema_json()
    ),
)]
impl ColumnMath {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "column-math", |a: Args| {
            gizza_ai_column_math_core::compute(&a.a, &a.b, &a.operation)
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
                    "a": { "type": "string", "description": "Column A — the numbers, separated by commas, spaces, or newlines (one per line)." },
                    "b": { "type": "string", "description": "Column B — the second set of numbers, same length as A, separated by commas, spaces, or newlines." },
                    "operation": { "type": "string", "enum": ["add", "subtract", "multiply", "divide"], "default": "add", "description": "Element-wise operation to apply between A and B, row by row: 'add' (default) computes A+B, 'subtract' A-B, 'multiply' A*B, 'divide' A/B (a zero divisor is an error)." }
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
