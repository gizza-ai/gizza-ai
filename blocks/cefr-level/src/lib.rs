//! gizza-ai/cefr-level — estimate English text difficulty with a deterministic CEFR heuristic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_cefr_level_core::{run_with_options, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_target")]
    target: String,
    #[serde(default = "default_coverage")]
    coverage: u32,
    #[serde(default = "default_unknown")]
    unknown: String,
    #[serde(default)]
    proper_nouns: bool,
}
fn default_output() -> String {
    "summary".to_string()
}
fn default_target() -> String {
    "B1".to_string()
}
fn default_coverage() -> u32 {
    90
}
fn default_unknown() -> String {
    "estimate".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("English text to estimate, from a sentence to a long reading passage."),
        )
        .param(
            Param::enumv("output", ["summary", "annotated", "table", "json"])
                .default("summary")
                .describe("Output format: summary (readable report), annotated (report plus word list), table (TSV word breakdown), or json (machine-readable)."),
        )
        .param(
            Param::enumv("target", ["A1", "A2", "B1", "B2", "C1", "C2"])
                .default("B1")
                .describe("Target learner level used to flag words above the intended CEFR band."),
        )
        .param(
            Param::integer("coverage")
                .default(90)
                .min(50.0)
                .max(100.0)
                .describe("Vocabulary coverage percentage used for the vocabulary band. Default 90 means the smallest CEFR band covering 90% of recognised running words."),
        )
        .param(
            Param::enumv("unknown", ["estimate", "c1", "c2", "exclude"])
                .default("estimate")
                .describe("How to treat words outside the built-in lexicon: estimate from length/shape, force C1, force C2, or exclude from the band profile."),
        )
        .param(
            Param::boolean("proper_nouns")
                .default(false)
                .describe("When true, capitalised proper-name-like words count toward difficulty; by default they are excluded so names do not inflate the estimate."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cefr-level",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Estimate English text CEFR level with word breakdowns",
    skill(
        description = "Estimate the CEFR reading difficulty of English text locally with a deterministic heuristic. Returns an overall A1-C2 band, decimal sublevel, vocabulary and grammar/sentence signals, per-band profile, and words above a chosen target level. Options choose output format (summary, annotated, table, json), target level, vocabulary coverage threshold, unknown-word handling, and whether proper nouns count.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cefr-level", |a: Args| {
            run_with_options(
                &a.text,
                &Options {
                    output: a.output,
                    target: a.target,
                    coverage: a.coverage,
                    unknown: a.unknown,
                    proper_nouns: a.proper_nouns,
                },
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
                    "text": { "type": "string", "description": "English text to estimate, from a sentence to a long reading passage." },
                    "output": { "type": "string", "enum": ["summary", "annotated", "table", "json"], "default": "summary", "description": "Output format: summary (readable report), annotated (report plus word list), table (TSV word breakdown), or json (machine-readable)." },
                    "target": { "type": "string", "enum": ["A1", "A2", "B1", "B2", "C1", "C2"], "default": "B1", "description": "Target learner level used to flag words above the intended CEFR band." },
                    "coverage": { "type": "integer", "default": 90, "minimum": 50, "maximum": 100, "description": "Vocabulary coverage percentage used for the vocabulary band. Default 90 means the smallest CEFR band covering 90% of recognised running words." },
                    "unknown": { "type": "string", "enum": ["estimate", "c1", "c2", "exclude"], "default": "estimate", "description": "How to treat words outside the built-in lexicon: estimate from length/shape, force C1, force C2, or exclude from the band profile." },
                    "proper_nouns": { "type": "boolean", "default": false, "description": "When true, capitalised proper-name-like words count toward difficulty; by default they are excluded so names do not inflate the estimate." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
