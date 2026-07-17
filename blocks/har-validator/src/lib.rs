//! gizza-ai/har-validator — validate a HAR (HTTP Archive) file's structure.
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_har_validator_core::validate;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    har: String,
    #[serde(default = "default_true")]
    check_timings: bool,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("har")
                .required()
                .describe("The HAR (HTTP Archive) file contents as JSON text — a single { \"log\": … } object with entries."),
        )
        .param(
            Param::boolean("check_timings")
                .default(true)
                .describe("Also check that each entry's total time equals the sum of its timing phases (blocked, dns, connect, send, wait, receive); phases of -1 are excluded. Reported as warnings. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/har-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate a HAR (HTTP Archive) file's structure",
    skill(
        description = "Validate a HAR (HTTP Archive) file against the HAR 1.2 spec: checks the log/entries shape, every required request, response, content, cache, and timings field (with the exact JSON path of each problem), and — when check_timings is true (default) — that each entry's total time equals the sum of its timing phases. Reports ALL problems found, not just the first, and returns a readable report for both valid and invalid HARs (only non-JSON input is an error). Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "har-validator", |a: Args| {
            validate(&a.har, a.check_timings).map_err(SkillError::InvalidArgs)
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
                    "har":           { "type": "string", "description": "The HAR (HTTP Archive) file contents as JSON text — a single { \"log\": … } object with entries." },
                    "check_timings": { "type": "boolean", "default": true, "description": "Also check that each entry's total time equals the sum of its timing phases (blocked, dns, connect, send, wait, receive); phases of -1 are excluded. Reported as warnings. Default true." }
                },
                "required": ["har"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
