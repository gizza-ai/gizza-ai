//! gizza-ai/index-of-coincidence — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure compute —
//! no host calls, runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    /// When > 0, also run key-length (period) estimation up to this length.
    #[serde(default)]
    max_period: u32,
    /// When true, include the per-letter frequency table in the report.
    #[serde(default)]
    show_counts: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to analyze. Only the 26 Latin letters A-Z are counted (case-folded); digits, spaces, punctuation, and non-Latin characters are ignored."),
        )
        .param(
            Param::integer("max_period")
                .default(0)
                .min(0.0)
                .max(gizza_ai_index_of_coincidence_core::MAX_PERIOD as f64)
                .describe("When greater than 0, also estimate the polyalphabetic key length (Vigenère period) by splitting the text into 1..=max_period columns and reporting the average column IC for each — the period with the highest value is the likely key length. 0 (default) skips this analysis. Max 40."),
        )
        .param(
            Param::boolean("show_counts")
                .default(false)
                .describe("When true, include a per-letter frequency table (count and percentage for each A-Z letter that appears). Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/index-of-coincidence",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Index of Coincidence skill",
    skill(
        description = "Calculate the Index of Coincidence (IC) of a text — the probability that two randomly drawn letters are the same — to gauge whether it is monoalphabetic, polyalphabetic, or random. Counts only the 26 Latin letters A-Z (case-folded); other characters are ignored. Reports the IC both normalized (English plaintext ~1.73, uniform/random ~1.0) and raw (English ~0.0667, random ~0.0385), plus a plain-language interpretation. Set max_period > 0 to also estimate a Vigenère key length (the period whose average column IC is highest is the likely key length). Set show_counts=true to include a per-letter frequency table.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "index-of-coincidence", |a: Args| {
            gizza_ai_index_of_coincidence_core::report(&a.text, a.max_period, a.show_counts)
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
                    "text": { "type": "string", "description": "The text to analyze. Only the 26 Latin letters A-Z are counted (case-folded); digits, spaces, punctuation, and non-Latin characters are ignored." },
                    "max_period": { "type": "integer", "minimum": 0, "maximum": 40, "default": 0, "description": "When greater than 0, also estimate the polyalphabetic key length (Vigenère period) by splitting the text into 1..=max_period columns and reporting the average column IC for each — the period with the highest value is the likely key length. 0 (default) skips this analysis. Max 40." },
                    "show_counts": { "type": "boolean", "default": false, "description": "When true, include a per-letter frequency table (count and percentage for each A-Z letter that appears). Default false." }
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
