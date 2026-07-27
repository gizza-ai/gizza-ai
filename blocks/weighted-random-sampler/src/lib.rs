//! gizza-ai/weighted-random-sampler — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_weighted_random_sampler_core::sample;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_format")]
    format: String,
    weight_field: String,
    #[serde(default = "default_n")]
    n: u64,
    #[serde(default)]
    replacement: bool,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_delim")]
    delimiter: String,
}
fn default_format() -> String {
    "csv".into()
}
fn default_n() -> u64 {
    2
}
fn default_seed() -> u64 {
    42
}
fn default_true() -> bool {
    true
}
fn default_delim() -> String {
    "comma".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The dataset to sample from — CSV text (with header by default) or a JSON array of objects, matching `format`."))
        .param(Param::enumv("format", ["csv", "json"]).default("csv").describe("Input/output format: csv (delimited rows) or json (an array of objects). The result is emitted in the same format. Default csv."))
        .param(Param::string("weight_field").required().describe("Field holding each row's weight. For CSV: a header name (header=true) or a 1-based column index. For JSON: the object key. Weights must be non-negative numbers; higher weight = more likely to be drawn."))
        .param(Param::integer("n").default(2).min(1.0).describe("Number of rows to draw. Without replacement it is capped at the count of positive-weight rows (error if larger); with replacement it may exceed the row count. Default 2."))
        .param(Param::boolean("replacement").default(false).describe("Sample WITH replacement (a row may be drawn more than once) when true; WITHOUT replacement (each row at most once, output in original order) when false. Default false."))
        .param(Param::integer("seed").default(42).min(0.0).describe("Seed for the reproducible PRNG. The same seed + inputs always yields the same draw; change it for a different sample. Default 42."))
        .param(Param::boolean("header").default(true).describe("CSV only: treat the first row as a header (kept in the output and used to resolve a named weight_field). Ignored for JSON. Default true."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("CSV only: field delimiter of the input, also used for the output. Ignored for JSON. Default comma."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WeightedRandomSampler;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/weighted-random-sampler",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Draw a weighted random sample of rows from a CSV or JSON dataset, with or without replacement",
    skill(
        description = "Draw a weighted random sample of `n` rows from a CSV or JSON-array dataset. Each row is selected with probability proportional to a numeric `weight_field` (CSV header name / 1-based index, or JSON object key). `replacement=true` allows a row to be drawn more than once and lets `n` exceed the row count; `replacement=false` (default) draws each row at most once (capped at the positive-weight rows) and preserves the input order. Draws are reproducible via `seed`. The output is emitted in the same `format` as the input; for CSV the header and `delimiter` are preserved.",
        parameters = schema_json()
    ),
)]
impl WeightedRandomSampler {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "weighted-random-sampler", |a: Args| {
            sample(
                &a.data,
                &a.format,
                &a.weight_field,
                a.n as usize,
                a.replacement,
                a.seed,
                a.header,
                &a.delimiter,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data":         { "type": "string", "description": "The dataset to sample from — CSV text (with header by default) or a JSON array of objects, matching `format`." },
                    "format":       { "type": "string", "enum": ["csv", "json"], "default": "csv", "description": "Input/output format: csv (delimited rows) or json (an array of objects). The result is emitted in the same format. Default csv." },
                    "weight_field": { "type": "string", "description": "Field holding each row's weight. For CSV: a header name (header=true) or a 1-based column index. For JSON: the object key. Weights must be non-negative numbers; higher weight = more likely to be drawn." },
                    "n":            { "type": "integer", "default": 2, "minimum": 1, "description": "Number of rows to draw. Without replacement it is capped at the count of positive-weight rows (error if larger); with replacement it may exceed the row count. Default 2." },
                    "replacement":  { "type": "boolean", "default": false, "description": "Sample WITH replacement (a row may be drawn more than once) when true; WITHOUT replacement (each row at most once, output in original order) when false. Default false." },
                    "seed":         { "type": "integer", "default": 42, "minimum": 0, "description": "Seed for the reproducible PRNG. The same seed + inputs always yields the same draw; change it for a different sample. Default 42." },
                    "header":       { "type": "boolean", "default": true, "description": "CSV only: treat the first row as a header (kept in the output and used to resolve a named weight_field). Ignored for JSON. Default true." },
                    "delimiter":    { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "CSV only: field delimiter of the input, also used for the output. Ignored for JSON. Default comma." }
                },
                "required": ["data", "weight_field"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
