//! gizza-ai/calculator — evaluates math expressions.
//!
//! Thin chat-skill wrapper around `gizza-ai-calculator-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`, returning
//! `{ "result": <number> }`. No host calls — runs entirely inside the WASM
//! sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    expr: String,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("expr").required().describe(
            "Arithmetic expression to evaluate (e.g. '2+2*3', 'sqrt(16)', '3.14 * 2^2').",
        ),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Calculator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Calculator skill",
    skill(
        description = "Evaluate an arithmetic expression (e.g. '2+2*3'). Returns the numeric result.",
        parameters = schema_json()
    )
)]
impl Calculator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } — calculator's
        // existing success shape — and routes errors through GuestResult::error.
        match run_skill(&body, "calculator", |a: Args| {
            gizza_ai_calculator_core::evaluate(&a.expr).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. (to_schema_json
    /// emits `additionalProperties: false` uniformly, which calculator's
    /// authored schema already had — so this is an exact match.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "expr": { "type": "string", "description": "Arithmetic expression to evaluate (e.g. '2+2*3', 'sqrt(16)', '3.14 * 2^2')." }
                },
                "required": ["expr"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
