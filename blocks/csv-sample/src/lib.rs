//! gizza-ai/csv-sample — take a random, systematic, top-N, bottom-N, or
//! stratified sample of a CSV's rows. Chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_sample_core::sample;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_n")]
    n: u64,
    #[serde(default)]
    percent: f64,
    #[serde(default)]
    stratify_column: String,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_delim")]
    delimiter: String,
}
fn default_method() -> String { "random".into() }
fn default_n() -> u64 { 10 }
fn default_seed() -> u64 { 42 }
fn default_true() -> bool { true }
fn default_delim() -> String { "comma".into() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to sample rows from."))
        .param(Param::enumv("method", ["random", "systematic", "top", "bottom", "stratified"]).default("random").describe("Sampling method: random (uniform), systematic (every k-th after a seeded start), top (first N), bottom (last N), or stratified (proportional across the values of stratify_column). Default random."))
        .param(Param::integer("n").default(10).min(1.0).describe("Number of data rows to sample. Ignored when percent > 0. Capped at the row count. Default 10."))
        .param(Param::number("percent").default(0.0).min(0.0).max(100.0).describe("Sample this percentage (0–100) of data rows instead of n; 0 means use n. Default 0."))
        .param(Param::string("stratify_column").default("").describe("For method=stratified: the column to stratify on — a header name (header=true) or a 1-based index. Each distinct value forms a stratum sampled in proportion to its size."))
        .param(Param::integer("seed").default(42).min(0.0).describe("Seed for the reproducible PRNG used by random/systematic/stratified sampling. Change it for a different draw. Default 42."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header (always kept, and used to resolve stratify_column names). Default true."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Field delimiter of the input, also used for the output. Default comma."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct CsvSample;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-sample",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sample rows from a CSV (random, systematic, top/bottom-N, or stratified)",
    skill(
        description = "Take a sample of a CSV's data rows. `method` is random (uniform), systematic (every k-th after a seeded start), top (first N), bottom (last N), or stratified (proportional across the distinct values of `stratify_column`). Size is `n` rows, or a `percent` (0–100) of the rows when percent > 0. Random draws are reproducible via `seed`. The header row is preserved and the input delimiter is used for the output.",
        parameters = schema_json()
    ),
)]
impl CsvSample {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-sample", |a: Args| {
            sample(
                &a.data,
                &a.method,
                a.n as usize,
                a.percent,
                &a.stratify_column,
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
                    "data":            { "type": "string", "description": "The CSV text to sample rows from." },
                    "method":          { "type": "string", "enum": ["random", "systematic", "top", "bottom", "stratified"], "default": "random", "description": "Sampling method: random (uniform), systematic (every k-th after a seeded start), top (first N), bottom (last N), or stratified (proportional across the values of stratify_column). Default random." },
                    "n":               { "type": "integer", "default": 10, "minimum": 1, "description": "Number of data rows to sample. Ignored when percent > 0. Capped at the row count. Default 10." },
                    "percent":         { "type": "number", "default": 0.0, "minimum": 0, "maximum": 100, "description": "Sample this percentage (0–100) of data rows instead of n; 0 means use n. Default 0." },
                    "stratify_column": { "type": "string", "default": "", "description": "For method=stratified: the column to stratify on — a header name (header=true) or a 1-based index. Each distinct value forms a stratum sampled in proportion to its size." },
                    "seed":            { "type": "integer", "default": 42, "minimum": 0, "description": "Seed for the reproducible PRNG used by random/systematic/stratified sampling. Change it for a different draw. Default 42." },
                    "header":          { "type": "boolean", "default": true, "description": "Treat the first row as a header (always kept, and used to resolve stratify_column names). Default true." },
                    "delimiter":       { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field delimiter of the input, also used for the output. Default comma." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
