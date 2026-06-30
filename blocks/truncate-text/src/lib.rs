//! gizza-ai/truncate-text — chat skill block on the shared tool abstraction.
//! Shortens text to a chosen number of characters or words, appending an
//! ellipsis. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure compute.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_truncate_text_core::Unit;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    /// Maximum number of units (characters or words) to keep; the core clamps to
    /// MIN_LENGTH..=MAX_LENGTH.
    #[serde(default = "default_length")]
    length: u32,
    /// "characters" (default) or "words".
    #[serde(default = "default_unit")]
    unit: String,
    /// Marker appended when text is cut (default "…").
    #[serde(default = "default_ellipsis")]
    ellipsis: String,
    /// Count the ellipsis toward the character budget (default true). Ignored for
    /// word truncation.
    #[serde(default = "default_true")]
    count_ellipsis: bool,
    /// Allow cutting in the middle of a word (default false). Ignored for word
    /// truncation.
    #[serde(default = "default_false")]
    break_words: bool,
}

fn default_length() -> u32 {
    100
}
fn default_unit() -> String {
    "characters".to_string()
}
fn default_ellipsis() -> String {
    "…".to_string()
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to shorten. Returned unchanged (no ellipsis) when it already fits within the limit."),
        )
        .param(
            // Bounds reference the core clamp so the LLM-facing schema can't drift
            // from what `truncate` actually enforces.
            Param::integer("length")
                .default(100)
                .min(gizza_ai_truncate_text_core::MIN_LENGTH as f64)
                .max(gizza_ai_truncate_text_core::MAX_LENGTH as f64)
                .describe("Maximum number of units to keep (default 100), measured per `unit`."),
        )
        .param(
            Param::enumv("unit", ["characters", "words"])
                .default("characters")
                .describe("Measure the limit in characters (default) or whole words."),
        )
        .param(
            Param::string("ellipsis")
                .default("…")
                .describe("Marker appended when text is actually cut (default \"…\"). Set to an empty string for a hard cut with no marker."),
        )
        .param(
            Param::boolean("count_ellipsis")
                .default(true)
                .describe("When true (default), the ellipsis length counts toward the character budget so the whole result fits within `length`. Ignored when unit is words."),
        )
        .param(
            Param::boolean("break_words")
                .default(false)
                .describe("When false (default), the cut is backed up to the last whitespace so a word is never split mid-way. When true, the text is cut exactly at the character limit. Ignored when unit is words."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/truncate-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Truncate Text skill",
    skill(
        description = "Shorten text to a maximum number of characters or words, appending an ellipsis only when the text is actually cut (text that already fits is returned unchanged). Set length (default 100) to the limit and unit to \"characters\" (default) or \"words\". For character truncation, break_words=false (default) backs the cut up to the last whitespace so a word is never split, and count_ellipsis=true (default) keeps the whole result — ellipsis included — within length. ellipsis is the marker appended when cut (default \"…\"; set empty for a hard cut). Useful for building previews, snippets, meta descriptions, table cells, or summaries of a fixed length.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "truncate-text", |a: Args| {
            let unit = Unit::parse(&a.unit).map_err(SkillError::InvalidArgs)?;
            gizza_ai_truncate_text_core::truncate(
                &a.text,
                a.length,
                unit,
                &a.ellipsis,
                a.count_ellipsis,
                a.break_words,
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
                    "text": { "type": "string", "description": "The text to shorten. Returned unchanged (no ellipsis) when it already fits within the limit." },
                    "length": { "type": "integer", "minimum": 1, "maximum": 1000000, "default": 100, "description": "Maximum number of units to keep (default 100), measured per `unit`." },
                    "unit": { "type": "string", "enum": ["characters", "words"], "default": "characters", "description": "Measure the limit in characters (default) or whole words." },
                    "ellipsis": { "type": "string", "default": "…", "description": "Marker appended when text is actually cut (default \"…\"). Set to an empty string for a hard cut with no marker." },
                    "count_ellipsis": { "type": "boolean", "default": true, "description": "When true (default), the ellipsis length counts toward the character budget so the whole result fits within `length`. Ignored when unit is words." },
                    "break_words": { "type": "boolean", "default": false, "description": "When false (default), the cut is backed up to the last whitespace so a word is never split mid-way. When true, the text is cut exactly at the character limit. Ignored when unit is words." }
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
