//! gizza-ai/mock-data-generator — turn a compact shorthand schema (your own
//! field names + types, with array counts and nested objects) into realistic
//! mock JSON.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. The chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Deterministic given a non-zero
//! `seed`; with the default seed=0 the chat surface derives a varying seed from
//! the wall clock so repeated calls aren't identical.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    schema: String,
    #[serde(default = "default_count")]
    count: u32,
    #[serde(default)]
    seed: u64,
    #[serde(default = "default_true")]
    pretty: bool,
}

fn default_count() -> u32 {
    1
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("schema").required().describe(
                "Shorthand schema: comma- or newline-separated `field:type` pairs. Types: string, int, float, bool, uuid, name, first_name, last_name, username, email, phone, date, datetime, url, domain, word, words, sentence, paragraph, city, state, country, zip, street, color, ipv4, mac, lat, lng. Parametric: enum(a|b|c), int(lo..hi), float(lo..hi), words(n). Append [n] for an array of that field (e.g. tags:string[3]). Nest objects with braces (e.g. user:{name:name, email:email}). Example: id:int(1..1000), user:{name:name, email:email}, roles:enum(admin|user)[2].",
            ),
        )
        .param(
            Param::integer("count")
                .default(1)
                .min(1.0)
                .max(gizza_ai_mock_data_generator_core::MAX_COUNT as f64)
                .describe("Number of records to generate, 1-1000. >1 wraps the output in a JSON array; 1 (default) emits a single object."),
        )
        .param(
            Param::integer("seed")
                .default(0)
                .min(0.0)
                .describe("Reproducibility seed. A non-zero value always reproduces the same data; 0 (default) varies per call (the chat surface seeds from the clock)."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Pretty-print the JSON with indentation (default true). False emits compact single-line JSON."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mock-data-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate mock JSON from a shorthand field schema",
    skill(
        description = "Generate realistic mock JSON from a compact shorthand schema you define: comma- or newline-separated `field:type` pairs, where you choose the field names and a type for each. Supported types: string, int, float, bool, uuid, name, first_name, last_name, username, email, phone, date, datetime, url, domain, word, words, sentence, paragraph, city, state, country, zip, street, color, ipv4, mac, lat, lng; parametric enum(a|b|c), int(lo..hi), float(lo..hi), words(n). Append [n] to a field for an array of n values (tags:string[3]) and nest objects with braces (user:{name:name, email:email}). Set count for the number of records (1-1000; >1 returns a JSON array). All data is synthetic placeholder data for testing/mocking, never real people. Pass a non-zero seed to reproduce the exact same data; pretty toggles indentation.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mock-data-generator", |a: Args| {
            // Chat: derive a varying seed from the wall clock when seed=0 so
            // repeated calls aren't identical; a non-zero seed reproduces.
            let seed = if a.seed == 0 {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0)
            } else {
                a.seed
            };
            gizza_ai_mock_data_generator_core::generate(&a.schema, a.count, seed, a.pretty)
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
                    "schema": { "type": "string", "description": "Shorthand schema: comma- or newline-separated `field:type` pairs. Types: string, int, float, bool, uuid, name, first_name, last_name, username, email, phone, date, datetime, url, domain, word, words, sentence, paragraph, city, state, country, zip, street, color, ipv4, mac, lat, lng. Parametric: enum(a|b|c), int(lo..hi), float(lo..hi), words(n). Append [n] for an array of that field (e.g. tags:string[3]). Nest objects with braces (e.g. user:{name:name, email:email}). Example: id:int(1..1000), user:{name:name, email:email}, roles:enum(admin|user)[2]." },
                    "count": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 1, "description": "Number of records to generate, 1-1000. >1 wraps the output in a JSON array; 1 (default) emits a single object." },
                    "seed": { "type": "integer", "minimum": 0, "default": 0, "description": "Reproducibility seed. A non-zero value always reproduces the same data; 0 (default) varies per call (the chat surface seeds from the clock)." },
                    "pretty": { "type": "boolean", "default": true, "description": "Pretty-print the JSON with indentation (default true). False emits compact single-line JSON." }
                },
                "required": ["schema"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
