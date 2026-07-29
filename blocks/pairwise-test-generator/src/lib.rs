//! gizza-ai/pairwise-test-generator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_pairwise_test_generator_core::{generate, OutputFormat};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    parameters: String,
    #[serde(default = "default_format")]
    output_format: String,
    #[serde(default = "default_true")]
    include_index: bool,
}
fn default_format() -> String {
    "markdown".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("parameters").required().describe(
                "The parameter model: one parameter per line as 'Name: value1, value2, …'. Example: 'Browser: Chrome, Firefox, Safari' then 'OS: Windows, macOS, Linux'. Blank lines and lines starting with '#' are ignored. Needs at least 2 parameters; up to 20 parameters with up to 30 values each.",
            ),
        )
        .param(
            Param::enumv("output_format", ["markdown", "csv", "json", "ascii"])
                .default("markdown")
                .describe(
                    "Output format: markdown (default, GitHub pipe table), csv (spreadsheet import), json (array of objects), or ascii (plain box-drawn grid).",
                ),
        )
        .param(
            Param::boolean("include_index")
                .default(true)
                .describe("Prepend a '#' column numbering each generated test case (default true)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pairwise-test-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a minimal all-pairs (pairwise) test set from a parameter model",
    skill(
        description = "Generate a minimal all-pairs (pairwise) combinatorial test set that covers every value pair across parameters. Input `parameters` is a plain-text model, one parameter per line as 'Name: value1, value2, …' (blank lines and '#' comments ignored); needs at least 2 parameters, up to 20 with up to 30 values each. A deterministic greedy algorithm produces far fewer test cases than the full Cartesian product while still exercising every pair of values from any two parameters. output_format=markdown (default) | csv | json | ascii; include_index prepends a numbered '#' column. Pairwise only (no higher-strength t-way or constraints). Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pairwise-test-generator", |a: Args| {
            let fmt = OutputFormat::parse(&a.output_format).map_err(SkillError::InvalidArgs)?;
            generate(&a.parameters, fmt, a.include_index).map_err(SkillError::InvalidArgs)
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
                    "parameters": { "type": "string", "description": "The parameter model: one parameter per line as 'Name: value1, value2, …'. Example: 'Browser: Chrome, Firefox, Safari' then 'OS: Windows, macOS, Linux'. Blank lines and lines starting with '#' are ignored. Needs at least 2 parameters; up to 20 parameters with up to 30 values each." },
                    "output_format": { "type": "string", "enum": ["markdown", "csv", "json", "ascii"], "default": "markdown", "description": "Output format: markdown (default, GitHub pipe table), csv (spreadsheet import), json (array of objects), or ascii (plain box-drawn grid)." },
                    "include_index": { "type": "boolean", "default": true, "description": "Prepend a '#' column numbering each generated test case (default true)." }
                },
                "required": ["parameters"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
