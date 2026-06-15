//! gizza-ai/calculator — evaluates math expressions.
//!
//! Thin chat-skill wrapper around `gizza-ai-calculator-core`. Takes
//! `{ "expr": "..." }`, returns `{ "result": <number> }` or `{ "error": "..." }`.
//! No host calls — runs entirely inside the WASM sandbox.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    expr: String,
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
        parameters = r#"{
            "type": "object",
            "properties": {
                "expr": { "type": "string", "description": "Arithmetic expression to evaluate (e.g. '2+2*3', 'sqrt(16)', '3.14 * 2^2')." }
            },
            "required": ["expr"],
            "additionalProperties": false
        }"#
    ),
)]
impl Calculator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => return respond_error(format!("invalid args: {e}")),
        };
        match gizza_ai_calculator_core::evaluate(&args.expr) {
            Ok(v) => {
                let body = serde_json::json!({ "result": v });
                GuestResult::respond(serde_json::to_vec(&body).unwrap_or_default())
            }
            Err(e) => respond_error(e),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn respond_error(msg: String) -> GuestResult {
    let body = serde_json::json!({ "error": msg });
    GuestResult::respond(serde_json::to_vec(&body).unwrap_or_default())
}
