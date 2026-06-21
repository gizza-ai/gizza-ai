//! gizza-ai/chi-square-test — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_chi_square_test_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    observed: String,
    #[serde(default)]
    expected: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    yates: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("mode", ["goodness-of-fit", "contingency"])
                .default("goodness-of-fit")
                .describe("Which test to run: 'goodness-of-fit' compares one observed frequency vector against an expected distribution; 'contingency' tests independence in an r×c table of observed counts."),
        )
        .param(
            Param::string("observed")
                .required()
                .describe("Observed counts. For goodness-of-fit: one row of numbers separated by spaces/commas/semicolons. For contingency: a table with one row per line, cells separated by spaces/commas/tabs (newlines between rows)."),
        )
        .param(
            Param::string("expected")
                .describe("Goodness-of-fit only (optional): the expected counts or ratios (same length as observed). Omit for an equal-frequency (uniform) null. Ratios like '9 3 3 1' are rescaled to the observed total. Ignored for contingency tests."),
        )
        .param(
            Param::boolean("yates")
                .default(false)
                .describe("Contingency only: apply Yates' continuity correction (only affects 2×2 tables). Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/chi-square-test",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pearson's chi-square test (goodness-of-fit or contingency table)",
    skill(
        description = "Run Pearson's chi-square test. Mode 'goodness-of-fit' compares an observed frequency vector against an expected distribution (uniform by default, or explicit expected counts/ratios); mode 'contingency' tests independence in an r×c table of observed counts. Returns the chi-square statistic, degrees of freedom, p-value, expected counts, Cramér's V effect size (contingency), and a low-expected-count warning. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "chi-square-test", |a: Args| {
            let yates = if a.yates { "true" } else { "false" };
            run(&a.observed, &a.expected, &a.mode, yates).map_err(SkillError::InvalidArgs)
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
                    "mode": {
                        "type": "string",
                        "enum": ["goodness-of-fit", "contingency"],
                        "default": "goodness-of-fit",
                        "description": "Which test to run: 'goodness-of-fit' compares one observed frequency vector against an expected distribution; 'contingency' tests independence in an r×c table of observed counts."
                    },
                    "observed": {
                        "type": "string",
                        "description": "Observed counts. For goodness-of-fit: one row of numbers separated by spaces/commas/semicolons. For contingency: a table with one row per line, cells separated by spaces/commas/tabs (newlines between rows)."
                    },
                    "expected": {
                        "type": "string",
                        "description": "Goodness-of-fit only (optional): the expected counts or ratios (same length as observed). Omit for an equal-frequency (uniform) null. Ratios like '9 3 3 1' are rescaled to the observed total. Ignored for contingency tests."
                    },
                    "yates": {
                        "type": "boolean",
                        "default": false,
                        "description": "Contingency only: apply Yates' continuity correction (only affects 2×2 tables). Default false."
                    }
                },
                "required": ["observed"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
