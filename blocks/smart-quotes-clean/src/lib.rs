//! gizza-ai/smart-quotes-clean — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_smart_quotes_clean_core::{clean, EmDash};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    /// ASCII rendering for em dashes (—): "--" (default), "-", or " - ".
    #[serde(default = "default_em_dash")]
    em_dash: String,
    /// Fold exotic Unicode spaces to ASCII and strip zero-width chars. Default true.
    #[serde(default = "default_true")]
    normalize_spaces: bool,
}

fn default_em_dash() -> String {
    "--".to_string()
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
                .describe("The text to clean of smart quotes and other typographic characters."),
        )
        .param(
            Param::enumv("em_dash", ["--", "-", " - "])
                .default("--")
                .describe("ASCII rendering for em dashes (—) and horizontal bars (―): '--' (default), '-', or ' - ' (a spaced hyphen)."),
        )
        .param(
            Param::boolean("normalize_spaces")
                .default(true)
                .describe("When true (default), exotic Unicode spaces (non-breaking, thin, ideographic, …) become a regular space and zero-width characters (incl. the BOM) are removed; set false to leave whitespace untouched."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SmartQuotesClean;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/smart-quotes-clean",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Smart-quotes / typography cleaner skill",
    skill(
        description = "Replace smart (curly) quotes, em/en dashes, the ellipsis glyph, prime marks, guillemets, and other typographic characters with plain ASCII equivalents: “ ” -> \", ‘ ’ -> ', – (en dash) -> -, — (em dash) -> -- (configurable via em_dash), … -> ..., ′ ″ -> ' \". Set em_dash to '-' or ' - ' to change how em dashes render. With normalize_spaces=true (default) it also folds non-breaking/thin/ideographic spaces to a regular space and strips zero-width characters and the BOM. Ordinary Unicode — accents, CJK, emoji — is preserved.",
        parameters = schema_json()
    ),
)]
impl SmartQuotesClean {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "smart-quotes-clean", |a: Args| {
            Ok::<String, SkillError>(clean(
                &a.text,
                EmDash::parse(&a.em_dash),
                a.normalize_spaces,
            ))
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
                    "text": { "type": "string", "description": "The text to clean of smart quotes and other typographic characters." },
                    "em_dash": { "type": "string", "enum": ["--", "-", " - "], "default": "--", "description": "ASCII rendering for em dashes (—) and horizontal bars (―): '--' (default), '-', or ' - ' (a spaced hyphen)." },
                    "normalize_spaces": { "type": "boolean", "default": true, "description": "When true (default), exotic Unicode spaces (non-breaking, thin, ideographic, …) become a regular space and zero-width characters (incl. the BOM) are removed; set false to leave whitespace untouched." }
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
