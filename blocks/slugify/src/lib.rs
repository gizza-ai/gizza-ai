//! gizza-ai/slugify — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    separator: String,
    /// Lowercase the slug. Defaults to true when omitted.
    #[serde(default = "default_true")]
    lowercase: bool,
    /// Truncate to at most this many chars on a word boundary; 0 = no limit.
    #[serde(default)]
    max_length: u32,
    /// Slugify each line independently (default false).
    #[serde(default)]
    per_line: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The title or phrase to convert into a slug."),
        )
        .param(
            Param::string("separator")
                .default("-")
                .describe("Word separator placed between words. Usually '-' (default) or '_'. Must not contain ASCII letters or digits."),
        )
        .param(
            Param::boolean("lowercase")
                .default(true)
                .describe("Lowercase the slug. Default true; set false to preserve the original case."),
        )
        .param(
            // Bounds reference the core cap so the LLM-facing schema can't drift
            // from what `slugify` actually enforces.
            Param::integer("max_length")
                .default(0)
                .min(0.0)
                .max(gizza_ai_slugify_core::MAX_LENGTH_CAP as f64)
                .describe("Truncate the slug to at most this many characters, cutting on a word boundary where possible. 0 (default) means no limit."),
        )
        .param(
            Param::boolean("per_line")
                .default(false)
                .describe("When true, slugify each line of the input independently (rejoined with newlines) — for a batch list of titles. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Slugify;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/slugify",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a title or phrase into a clean URL-safe slug.",
    skill(
        description = "Convert a title or phrase into a clean, URL-safe slug: Unicode is transliterated to ASCII (e.g. 'Crème Brûlée' -> 'creme-brulee', '北京' -> 'bei-jing'), apostrophes are dropped so contractions join ('Bob's' -> 'bobs'), the text is lowercased, and every run of spaces and punctuation is collapsed to a single separator with no leading/trailing separators. Set separator to '_' (or another non-alphanumeric string) to change the word separator, lowercase=false to keep the original case, max_length>0 to truncate on a word boundary, or per_line=true to slugify a batch list of titles line by line.",
        parameters = schema_json()
    ),
)]
impl Slugify {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "slugify", |a: Args| {
            gizza_ai_slugify_core::slugify(
                &a.text,
                &a.separator,
                a.lowercase,
                a.max_length,
                a.per_line,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The title or phrase to convert into a slug." },
                    "separator": { "type": "string", "default": "-", "description": "Word separator placed between words. Usually '-' (default) or '_'. Must not contain ASCII letters or digits." },
                    "lowercase": { "type": "boolean", "default": true, "description": "Lowercase the slug. Default true; set false to preserve the original case." },
                    "max_length": { "type": "integer", "minimum": 0, "maximum": 2048, "default": 0, "description": "Truncate the slug to at most this many characters, cutting on a word boundary where possible. 0 (default) means no limit." },
                    "per_line": { "type": "boolean", "default": false, "description": "When true, slugify each line of the input independently (rejoined with newlines) — for a batch list of titles. Default false." }
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
