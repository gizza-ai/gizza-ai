//! gizza-ai/reservoir-sampler — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_reservoir_sampler_core::sample;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_k")]
    k: u32,
    #[serde(default = "default_algorithm")]
    algorithm: String,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default = "default_true")]
    skip_empty: bool,
    #[serde(default)]
    header: bool,
    #[serde(default = "default_order")]
    order: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    stats: bool,
}
fn default_k() -> u32 {
    10
}
fn default_algorithm() -> String {
    "l".into()
}
fn default_seed() -> u64 {
    42
}
fn default_true() -> bool {
    true
}
fn default_order() -> String {
    "input".into()
}
fn default_format() -> String {
    "lines".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The dataset to sample from, one record per line. Records are read as a stream in a single pass, so the memory used depends on `k`, not on how long the dataset is."))
        .param(Param::integer("k").default(10).min(1.0).max(1000000.0).describe("Sample size — how many records to keep in the reservoir. If the dataset holds fewer records than this, every record is returned. Default 10."))
        .param(Param::enumv("algorithm", ["l", "r"]).default("l").describe("Reservoir algorithm: 'l' (Algorithm L — skips ahead to the next replacement, far fewer random draws) or 'r' (Algorithm R — the classic one-draw-per-record version). Both give a uniform sample without replacement; they pick different records for the same seed. Default l."))
        .param(Param::integer("seed").default(42).min(0.0).describe("Seed for the reproducible PRNG. The same seed, dataset, and options always yield the same sample; change it to draw again. Default 42."))
        .param(Param::boolean("skip_empty").default(true).describe("Ignore blank and whitespace-only lines so they can never be drawn or counted. Default true."))
        .param(Param::boolean("header").default(false).describe("Treat the first line as a header row: it is excluded from the draw and echoed at the top of the result (lines/numbered output only). Default false."))
        .param(Param::enumv("order", ["input", "reservoir"]).default("input").describe("Order of the sampled records: 'input' keeps their original order in the dataset, 'reservoir' returns them in the shuffled order the reservoir ended up in (handy for picking ranked winners). Default input."))
        .param(Param::enumv("format", ["lines", "numbered", "json"]).default("lines").describe("Output format: 'lines' (the records alone), 'numbered' (original line number, a tab, then the record), or 'json' (an array of {line, text} objects). Default lines."))
        .param(Param::boolean("stats").default(false).describe("Report the one-pass statistics — records scanned, records sampled, the inclusion probability k/N, the algorithm, and the seed. Appended as a '#' comment line for lines/numbered output, or wrapped around the array as an object for json. Default false."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/reservoir-sampler",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Draw a uniform random sample of fixed size from a dataset in one pass",
    skill(
        description = "Draw a uniform random sample of `k` records from a line-oriented dataset using reservoir sampling — one pass, memory proportional to the sample size rather than the dataset. `algorithm` picks Algorithm L (skip-based, the default) or Algorithm R (the classic per-record version); both sample without replacement, so no record is drawn twice. `seed` makes the draw reproducible. Blank lines are skipped by default (`skip_empty`), and `header=true` keeps the first line out of the draw and echoes it above the sample. `order` returns the records in their original dataset order or in reservoir (draw) order, `format` emits plain lines, line-numbered lines, or JSON, and `stats=true` reports how many records were scanned and the inclusion probability. If the dataset is smaller than `k`, every record is returned.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "reservoir-sampler", |a: Args| {
            sample(
                &a.data,
                a.k,
                &a.algorithm,
                a.seed,
                a.skip_empty,
                a.header,
                &a.order,
                &a.format,
                a.stats,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The dataset to sample from, one record per line. Records are read as a stream in a single pass, so the memory used depends on `k`, not on how long the dataset is." },
                    "k": { "type": "integer", "minimum": 1, "maximum": 1000000, "default": 10, "description": "Sample size — how many records to keep in the reservoir. If the dataset holds fewer records than this, every record is returned. Default 10." },
                    "algorithm": { "type": "string", "enum": ["l", "r"], "default": "l", "description": "Reservoir algorithm: 'l' (Algorithm L — skips ahead to the next replacement, far fewer random draws) or 'r' (Algorithm R — the classic one-draw-per-record version). Both give a uniform sample without replacement; they pick different records for the same seed. Default l." },
                    "seed": { "type": "integer", "minimum": 0, "default": 42, "description": "Seed for the reproducible PRNG. The same seed, dataset, and options always yield the same sample; change it to draw again. Default 42." },
                    "skip_empty": { "type": "boolean", "default": true, "description": "Ignore blank and whitespace-only lines so they can never be drawn or counted. Default true." },
                    "header": { "type": "boolean", "default": false, "description": "Treat the first line as a header row: it is excluded from the draw and echoed at the top of the result (lines/numbered output only). Default false." },
                    "order": { "type": "string", "enum": ["input", "reservoir"], "default": "input", "description": "Order of the sampled records: 'input' keeps their original order in the dataset, 'reservoir' returns them in the shuffled order the reservoir ended up in (handy for picking ranked winners). Default input." },
                    "format": { "type": "string", "enum": ["lines", "numbered", "json"], "default": "lines", "description": "Output format: 'lines' (the records alone), 'numbered' (original line number, a tab, then the record), or 'json' (an array of {line, text} objects). Default lines." },
                    "stats": { "type": "boolean", "default": false, "description": "Report the one-pass statistics — records scanned, records sampled, the inclusion probability k/N, the algorithm, and the seed. Appended as a '#' comment line for lines/numbered output, or wrapped around the array as an object for json. Default false." }
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
