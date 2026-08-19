//! gizza-ai/jsonl-stats — chat skill block on the shared tool abstraction.
//! The descriptor is the single source for the chat schema, CLI, and generated
//! page controls; handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_depth")]
    depth: i64,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default)]
    max_keys: i64,
    #[serde(default = "default_samples")]
    samples: i64,
    #[serde(default = "default_true")]
    value_stats: bool,
    #[serde(default = "default_true")]
    distinct: bool,
    #[serde(default = "default_invalid")]
    invalid: String,
}

fn default_depth() -> i64 {
    1
}
fn default_format() -> String {
    "text".to_string()
}
fn default_sort() -> String {
    "frequency".to_string()
}
fn default_samples() -> i64 {
    2
}
fn default_true() -> bool {
    true
}
fn default_invalid() -> String {
    "report".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("JSON Lines / NDJSON text: one complete JSON value per non-blank line. Object records are profiled by key; invalid lines can be reported, skipped, or treated as errors."),
        )
        .param(
            Param::integer("depth")
                .min(1.0)
                .max(10.0)
                .default(1)
                .describe("Nested key depth to profile. 1 reports top-level keys; larger values add dotted object paths and [] array-element paths such as user.id or items[].sku."),
        )
        .param(
            Param::enumv("format", ["text", "json", "markdown", "csv"])
                .default("text")
                .describe("Output format: aligned text report, structured JSON, Markdown table, or CSV."),
        )
        .param(
            Param::enumv("sort", ["frequency", "name", "first-seen"])
                .default("frequency")
                .describe("Sort reported keys by record coverage frequency, alphabetic key name, or first-seen order in the file."),
        )
        .param(
            Param::integer("max_keys")
                .min(0.0)
                .default(0)
                .describe("Maximum keys to include in the report. 0 reports every profiled key after sorting."),
        )
        .param(
            Param::integer("samples")
                .min(0.0)
                .max(5.0)
                .default(2)
                .describe("Number of first distinct scalar sample values to show per key, from 0 to 5."),
        )
        .param(
            Param::boolean("value_stats")
                .default(true)
                .describe("Include numeric min/max/mean and string length min/max summaries where applicable."),
        )
        .param(
            Param::boolean("distinct")
                .default(true)
                .describe("Include an approximate distinct scalar value count per key, capped and shown as N+ after the internal tracking limit."),
        )
        .param(
            Param::enumv("invalid", ["report", "skip", "error"])
                .default("report")
                .describe("Invalid-line handling: report counts and examples, skip invalid lines silently except for the count, or stop at the first parse error."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonlStats;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jsonl-stats",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Summarize JSON Lines key coverage, types, and value distributions.",
    skill(
        description = "Profile JSON Lines / NDJSON text. Counts records, reports per-key presence frequency and coverage, value-type distribution, optional distinct scalar counts, sample values, numeric min/max/mean, string length ranges, nested dotted paths, and invalid-line handling. Outputs text, JSON, Markdown, or CSV.",
        parameters = schema_json()
    ),
)]
impl JsonlStats {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jsonl-stats", |a: Args| {
            gizza_ai_jsonl_stats_core::run(
                &a.input,
                a.depth,
                &a.format,
                &a.sort,
                a.max_keys,
                a.samples,
                a.value_stats,
                a.distinct,
                &a.invalid,
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
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").unwrap();
        assert_eq!(schema.get("type").unwrap(), "object");
        assert_eq!(schema.get("additionalProperties").unwrap(), false);
        assert_eq!(
            schema.get("required").unwrap(),
            &serde_json::json!(["input"])
        );
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["text", "json", "markdown", "csv"])
        );
        assert_eq!(
            props["sort"]["enum"],
            serde_json::json!(["frequency", "name", "first-seen"])
        );
        assert_eq!(
            props["invalid"]["enum"],
            serde_json::json!(["report", "skip", "error"])
        );
        assert_eq!(props["depth"]["default"], 1);
        assert_eq!(props["samples"]["default"], 2);
        assert_eq!(props["value_stats"]["default"], true);
        assert_eq!(props["distinct"]["default"], true);
        for key in [
            "input",
            "depth",
            "format",
            "sort",
            "max_keys",
            "samples",
            "value_stats",
            "distinct",
            "invalid",
        ] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing .describe() for {key}"
            );
        }
    }
}
