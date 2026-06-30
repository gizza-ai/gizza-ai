//! gizza-ai/percentage-calculator — chat skill block on the shared tool abstraction.
//!
//! Answers the five everyday percentage questions (percent_of, what_percent,
//! change, apply_change, percent_of_total) from plain numbers. The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill` and returns the pretty-printed result
//! JSON from `core::compute_json`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_percentage_calculator_core::Inputs;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    mode: String,
    #[serde(default)]
    percent: Option<f64>,
    #[serde(default)]
    base: Option<f64>,
    #[serde(default)]
    part: Option<f64>,
    #[serde(default)]
    whole: Option<f64>,
    #[serde(default)]
    from: Option<f64>,
    #[serde(default)]
    to: Option<f64>,
    #[serde(default)]
    value: Option<f64>,
    #[serde(default)]
    total: Option<f64>,
}

impl Args {
    fn inputs(&self) -> Inputs {
        Inputs {
            percent: self.percent,
            base: self.base,
            part: self.part,
            whole: self.whole,
            from: self.from,
            to: self.to,
            value: self.value,
            total: self.total,
        }
    }
}

const MODES: [&str; 5] = [
    "percent_of",
    "what_percent",
    "change",
    "apply_change",
    "percent_of_total",
];

/// Single source for the chat schema (and CLI). Pass `mode` plus only the numbers
/// that mode reads; the rest are ignored.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("mode", MODES).required().describe(
                "Which percentage question to answer. percent_of: P% of a base \
                 (percent, base). what_percent: part is what percent of whole \
                 (part, whole). change: percent change from one value to another \
                 (from, to). apply_change: increase or decrease a base by a \
                 percent (base, percent). percent_of_total: a value's share of a \
                 total (value, total).",
            ),
        )
        .param(Param::number("percent").describe(
            "Percentage value, e.g. 15 for 15%. Used by percent_of and apply_change.",
        ))
        .param(Param::number("base").describe(
            "The base amount the percentage acts on. Used by percent_of and apply_change.",
        ))
        .param(Param::number("part").describe(
            "The part amount. Used by what_percent.",
        ))
        .param(Param::number("whole").describe(
            "The whole amount (must be non-zero). Used by what_percent.",
        ))
        .param(Param::number("from").describe(
            "Starting (old) value, must be non-zero. Used by change.",
        ))
        .param(Param::number("to").describe(
            "Ending (new) value. Used by change.",
        ))
        .param(Param::number("value").describe(
            "The value whose share of the total you want. Used by percent_of_total.",
        ))
        .param(Param::number("total").describe(
            "The total (must be non-zero). Used by percent_of_total.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/percentage-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Percentage calculator: percent of, what percent, percent change, apply change, share of total",
    skill(
        description = "Answer everyday percentage questions from plain numbers. Choose a mode: percent_of (what is P% of a base — percent, base), what_percent (a part is what percent of a whole — part, whole), change (percent change from one value to another — from, to), apply_change (increase or decrease a base by a percent — base, percent), or percent_of_total (a value's share of a total — value, total). Pass mode plus only the numbers that mode needs. Returns the canonical mode, the inputs echoed back, the computed measures with unit suffixes, and a human-readable summary.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "percentage-calculator", |a: Args| {
            gizza_ai_percentage_calculator_core::compute_json(&a.mode, &a.inputs())
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

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "enum": ["percent_of","what_percent","change","apply_change","percent_of_total"], "description": "Which percentage question to answer. percent_of: P% of a base (percent, base). what_percent: part is what percent of whole (part, whole). change: percent change from one value to another (from, to). apply_change: increase or decrease a base by a percent (base, percent). percent_of_total: a value's share of a total (value, total)." },
                    "percent": { "type": "number", "description": "Percentage value, e.g. 15 for 15%. Used by percent_of and apply_change." },
                    "base": { "type": "number", "description": "The base amount the percentage acts on. Used by percent_of and apply_change." },
                    "part": { "type": "number", "description": "The part amount. Used by what_percent." },
                    "whole": { "type": "number", "description": "The whole amount (must be non-zero). Used by what_percent." },
                    "from": { "type": "number", "description": "Starting (old) value, must be non-zero. Used by change." },
                    "to": { "type": "number", "description": "Ending (new) value. Used by change." },
                    "value": { "type": "number", "description": "The value whose share of the total you want. Used by percent_of_total." },
                    "total": { "type": "number", "description": "The total (must be non-zero). Used by percent_of_total." }
                },
                "required": ["mode"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
