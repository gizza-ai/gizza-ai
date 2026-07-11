//! gizza-ai/context-trimmer — chat skill block on the shared tool abstraction.
//! Trims text to fit an approximate LLM token budget, keeping the head, tail,
//! middle, or both ends. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to block_utils::run_skill.
//! Pure compute.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_context_trimmer_core::Keep;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    /// Target token budget; the core clamps to MIN_TOKENS..=MAX_TOKENS.
    #[serde(default = "default_max_tokens")]
    max_tokens: u32,
    /// Approximate characters per token (default 4.0).
    #[serde(default = "default_cpt")]
    chars_per_token: f64,
    /// "head" (default), "tail", "middle", or "head_tail".
    #[serde(default = "default_keep")]
    keep: String,
    /// Marker inserted where text is removed (default "…").
    #[serde(default = "default_marker")]
    marker: String,
    /// For keep=head_tail, the fraction of the budget given to the head.
    #[serde(default = "default_head_ratio")]
    head_ratio: f64,
    /// Allow cutting in the middle of a word (default false).
    #[serde(default = "default_false")]
    break_words: bool,
}

fn default_max_tokens() -> u32 {
    512
}
fn default_cpt() -> f64 {
    4.0
}
fn default_keep() -> String {
    "head".to_string()
}
fn default_marker() -> String {
    "…".to_string()
}
fn default_head_ratio() -> f64 {
    0.5
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
                .describe("The text to trim. Returned unchanged (no marker) when it already fits the token budget."),
        )
        .param(
            // Bounds reference the core clamp so the LLM-facing schema can't
            // drift from what `trim` actually enforces.
            Param::integer("max_tokens")
                .default(512)
                .min(gizza_ai_context_trimmer_core::MIN_TOKENS as f64)
                .max(gizza_ai_context_trimmer_core::MAX_TOKENS as f64)
                .describe("Target token budget (default 512). Tokens are estimated, not counted by a real tokenizer — see chars_per_token."),
        )
        .param(
            Param::number("chars_per_token")
                .default(4.0)
                .min(gizza_ai_context_trimmer_core::MIN_CPT)
                .max(gizza_ai_context_trimmer_core::MAX_CPT)
                .describe("Approximate characters per token used to estimate tokens (default 4.0, OpenAI's English rule of thumb). Lower it (~3) for code or non-English text to trim more conservatively."),
        )
        .param(
            Param::enumv("keep", ["head", "tail", "middle", "head_tail"])
                .default("head")
                .describe("Which part to keep: head (beginning, default), tail (end), middle (centre, both ends dropped), or head_tail (keep the beginning AND the end, drop the middle)."),
        )
        .param(
            Param::string("marker")
                .default("…")
                .describe("Marker inserted where text is removed (default \"…\"). Its length counts toward the budget so the result still fits. Set to an empty string for a hard cut with no marker."),
        )
        .param(
            Param::number("head_ratio")
                .default(0.5)
                .min(0.0)
                .max(1.0)
                .describe("For keep=head_tail, the fraction of the budget given to the head (default 0.5 = an even split; 1.0 = all head, 0.0 = all tail). Ignored for other strategies."),
        )
        .param(
            Param::boolean("break_words")
                .default(false)
                .describe("When false (default), each cut is backed up to a whitespace boundary so a word is never split. When true, the text is cut exactly at the character limit."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/context-trimmer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Trim text to fit an approximate LLM token budget, keeping the head, tail, middle, or both ends.",
    skill(
        description = "Trim or truncate text to fit an approximate LLM token budget, keeping the part you choose. Token counts are ESTIMATED as characters ÷ chars_per_token (default 4.0) — there is no real tokenizer in the browser. Set max_tokens (default 512) to the budget and keep to \"head\" (beginning, default), \"tail\" (end), \"middle\" (centre, both ends dropped), or \"head_tail\" (keep the beginning AND the end, drop the middle — head_ratio splits the budget). marker is inserted where text is removed (default \"…\"; set empty for a hard cut) and counts toward the budget so the result still fits. break_words=false (default) keeps cuts on whitespace so no word is split. Text that already fits is returned unchanged. Useful for fitting prompts, documents, logs, or chat history into a context window.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "context-trimmer", |a: Args| {
            let keep = Keep::parse(&a.keep).map_err(SkillError::InvalidArgs)?;
            gizza_ai_context_trimmer_core::trim(
                &a.text,
                a.max_tokens,
                a.chars_per_token,
                keep,
                &a.marker,
                a.head_ratio,
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
                    "text": { "type": "string", "description": "The text to trim. Returned unchanged (no marker) when it already fits the token budget." },
                    "max_tokens": { "type": "integer", "minimum": 1, "maximum": 1000000, "default": 512, "description": "Target token budget (default 512). Tokens are estimated, not counted by a real tokenizer — see chars_per_token." },
                    "chars_per_token": { "type": "number", "minimum": 1, "maximum": 20, "default": 4.0, "description": "Approximate characters per token used to estimate tokens (default 4.0, OpenAI's English rule of thumb). Lower it (~3) for code or non-English text to trim more conservatively." },
                    "keep": { "type": "string", "enum": ["head", "tail", "middle", "head_tail"], "default": "head", "description": "Which part to keep: head (beginning, default), tail (end), middle (centre, both ends dropped), or head_tail (keep the beginning AND the end, drop the middle)." },
                    "marker": { "type": "string", "default": "…", "description": "Marker inserted where text is removed (default \"…\"). Its length counts toward the budget so the result still fits. Set to an empty string for a hard cut with no marker." },
                    "head_ratio": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.5, "description": "For keep=head_tail, the fraction of the budget given to the head (default 0.5 = an even split; 1.0 = all head, 0.0 = all tail). Ignored for other strategies." },
                    "break_words": { "type": "boolean", "default": false, "description": "When false (default), each cut is backed up to a whitespace boundary so a word is never split. When true, the text is cut exactly at the character limit." }
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
