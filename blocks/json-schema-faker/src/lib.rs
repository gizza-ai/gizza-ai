//! gizza-ai/json-schema-faker — generate fake data from a JSON Schema.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn one() -> u32 {
    1
}
fn seed_default() -> u64 {
    1
}
fn pretty_default() -> bool {
    true
}
fn output_default() -> String {
    "json".to_string()
}

#[derive(Deserialize)]
struct Args {
    schema: String,
    #[serde(default = "one")]
    count: u32,
    #[serde(default = "seed_default")]
    seed: u64,
    #[serde(default = "pretty_default")]
    pretty: bool,
    #[serde(default = "output_default")]
    output: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("schema").required().multiline().describe("JSON Schema object to generate data from. Supported subset: object/array/scalar types, properties, required, enum, const, common string formats, and basic length/range/item bounds."))
        .param(Param::integer("count").default(1).min(1.0).max(1000.0).describe("Number of records to generate (1–1000)."))
        .param(Param::integer("seed").default(1).min(0.0).describe("Deterministic seed. Use the same non-zero seed to reproduce output; 0 uses the core fallback seed."))
        .param(Param::boolean("pretty").default(true).describe("Pretty-print JSON output. Ignored for JSON Lines and CSV."))
        .param(Param::enumv("output", ["json", "jsonl", "csv"]).default("json").describe("Output format: JSON array, JSON Lines, or CSV (CSV requires object rows)."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonSchemaFaker;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-schema-faker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate deterministic fake data from a JSON Schema",
    skill(
        description = "Generate N fake records that conform to a supported JSON Schema subset. Handles object properties, arrays, scalar types, enum/const, required fields, common string formats (email, uuid, date, date-time, uri, ipv4), length/range/item bounds, deterministic seeds, and JSON/JSONL/CSV output. Unsupported assertion keywords such as $ref, oneOf/anyOf/allOf, pattern, patternProperties, dependencies, and conditionals are rejected with clear errors instead of being silently ignored.",
        parameters = schema_json()
    ),
)]
impl JsonSchemaFaker {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-schema-faker", |a: Args| {
            gizza_ai_json_schema_faker_core::generate(
                &a.schema, a.count, a.seed, a.pretty, &a.output,
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
                "type":"object",
                "properties":{
                    "schema":{"type":"string","description":"JSON Schema object to generate data from. Supported subset: object/array/scalar types, properties, required, enum, const, common string formats, and basic length/range/item bounds."},
                    "count":{"type":"integer","default":1,"minimum":1,"maximum":1000,"description":"Number of records to generate (1–1000)."},
                    "seed":{"type":"integer","default":1,"minimum":0,"description":"Deterministic seed. Use the same non-zero seed to reproduce output; 0 uses the core fallback seed."},
                    "pretty":{"type":"boolean","default":true,"description":"Pretty-print JSON output. Ignored for JSON Lines and CSV."},
                    "output":{"type":"string","enum":["json","jsonl","csv"],"default":"json","description":"Output format: JSON array, JSON Lines, or CSV (CSV requires object rows)."}
                },
                "required":["schema"],
                "additionalProperties":false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
