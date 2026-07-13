//! gizza-ai/token-counter — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    model: String,
}

/// Single source for the chat schema (and CLI + page). The `model` enum variants
/// come from the core pricing table so they can't drift.
fn descriptor() -> ToolDescriptor {
    // `enumv` needs a fixed-size array; keep this list in the same order as the
    // core MODELS table (a core unit test guards the count).
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .multiline()
                .describe("The text to tokenize. Counted with the chosen model's BPE encoding (exact for OpenAI models; an approximation for Anthropic/Google, whose tokenizers are proprietary)."),
        )
        .param(
            Param::enumv(
                "model",
                [
                    "gpt-5.5", "gpt-5", "gpt-4.1", "gpt-4.1-mini", "gpt-4o", "gpt-4o-mini",
                    "gpt-4-turbo", "gpt-3.5-turbo", "claude-opus-4.8", "claude-sonnet-5",
                    "claude-haiku-4.5", "gemini-3-pro", "gemini-2.5-flash",
                ],
            )
            .default(gizza_ai_token_counter_core::DEFAULT_MODEL)
            .describe("Model whose tokenizer and pricing to use. OpenAI GPT-5.5/5/4.1/4.1-mini/4o/4o-mini use o200k_base and GPT-4-turbo/3.5-turbo use cl100k_base (both exact); Anthropic Claude and Google Gemini token counts are an o200k_base approximation (proprietary tokenizers). Default gpt-4o."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/token-counter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Count LLM tokens for pasted text and estimate the prompt cost and context-window usage for a chosen model.",
    skill(
        description = "Count how many tokens a piece of text is for a chosen LLM, and estimate the prompt (input) cost and context-window usage. Uses real BPE tokenization via tiktoken: OpenAI GPT-5.5/GPT-5/GPT-4.1/GPT-4.1-mini/GPT-4o/GPT-4o-mini use o200k_base and GPT-4-turbo/GPT-3.5-turbo use cl100k_base — those counts are exact. Anthropic Claude (Opus 4.8, Sonnet 5, Haiku 4.5) and Google Gemini (3 Pro, 2.5 Flash) tokenizers are proprietary, so their counts are an o200k_base approximation, labeled (approx). Reports the token count, character count, estimated input cost (tokens x the model's input price / 1M), the output price for reference, and the context-window size with percent used. Pricing is a dated embedded snapshot (an estimate) — verify on the provider's pricing page. Runs entirely in the sandbox; no text leaves the tool.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "token-counter", |a: Args| {
            gizza_ai_token_counter_core::count(&a.text, &a.model).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The descriptor's `model` enum must exactly match the core pricing table,
    /// in order — otherwise the CLI/page could offer a model the core rejects.
    #[test]
    fn model_enum_matches_core_table() {
        let d = descriptor();
        let model_param = d.params.iter().find(|p| p.name == "model").unwrap();
        if let gizza_ai_block_utils::ParamKind::Enum(variants) = &model_param.kind {
            assert_eq!(
                variants.as_slice(),
                gizza_ai_token_counter_core::model_ids().as_slice(),
                "descriptor model enum drifted from core MODELS table"
            );
        } else {
            panic!("model param is not an enum");
        }
    }

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to tokenize. Counted with the chosen model's BPE encoding (exact for OpenAI models; an approximation for Anthropic/Google, whose tokenizers are proprietary)." },
                    "model": { "type": "string", "enum": ["gpt-5.5","gpt-5","gpt-4.1","gpt-4.1-mini","gpt-4o","gpt-4o-mini","gpt-4-turbo","gpt-3.5-turbo","claude-opus-4.8","claude-sonnet-5","claude-haiku-4.5","gemini-3-pro","gemini-2.5-flash"], "default": "gpt-4o", "description": "Model whose tokenizer and pricing to use. OpenAI GPT-5.5/5/4.1/4.1-mini/4o/4o-mini use o200k_base and GPT-4-turbo/3.5-turbo use cl100k_base (both exact); Anthropic Claude and Google Gemini token counts are an o200k_base approximation (proprietary tokenizers). Default gpt-4o." }
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
