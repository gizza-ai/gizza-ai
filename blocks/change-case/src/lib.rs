//! gizza-ai/change-case — converts text between letter cases (chat skill block).
//!
//! Thin chat-skill wrapper around `gizza-ai-change-case-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    mode: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to convert."),
        )
        .param(
            Param::enumv(
                "mode",
                [
                    "upper", "lower", "title", "sentence", "capitalize", "swap",
                    "alternate", "camel", "pascal", "snake", "constant", "kebab",
                    "train", "dot", "path",
                ],
            )
            .default("upper")
            .describe(
                "Target case. Letter-case (keeps spacing/punctuation): 'upper' UPPERCASE; \
'lower' lowercase; 'title' Capitalize Each Word; 'sentence' Capitalize each sentence; \
'capitalize' Capitalize only the first letter; 'swap' invert every letter's case; \
'alternate' aLtErNaTe lower/upper. Identifier cases (re-tokenize words): 'camel' \
camelCase; 'pascal' PascalCase; 'snake' snake_case; 'constant' CONSTANT_CASE; 'kebab' \
kebab-case; 'train' Train-Case; 'dot' dot.case; 'path' path/case. Default 'upper'.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/change-case",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert text between letter cases.",
    skill(
        description = "Convert text to a different letter case. Set mode to one of: 'upper' (UPPERCASE), 'lower' (lowercase), 'title' (Capitalize Each Word), 'sentence' (Capitalize the first letter of each sentence), 'capitalize' (Capitalize only the first letter), 'swap' (invert every letter's case), 'alternate' (aLtErNaTe lower/upper), or an identifier case that re-splits the text into words — 'camel' (camelCase), 'pascal' (PascalCase), 'snake' (snake_case), 'constant' (CONSTANT_CASE), 'kebab' (kebab-case), 'train' (Train-Case), 'dot' (dot.case), 'path' (path/case). Default mode is 'upper'. Handles full Unicode (e.g. 'straße' -> 'STRASSE') and existing camelCase/acronyms when producing identifier cases (e.g. 'HTTPServer' -> 'http_server').",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "change-case", |a: Args| {
            gizza_ai_change_case_core::convert(&a.text, &a.mode).map_err(SkillError::InvalidArgs)
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
                    "text": { "type": "string", "description": "The text to convert." },
                    "mode": {
                        "type": "string",
                        "enum": ["upper", "lower", "title", "sentence", "capitalize", "swap", "alternate", "camel", "pascal", "snake", "constant", "kebab", "train", "dot", "path"],
                        "default": "upper",
                        "description": "Target case. Letter-case (keeps spacing/punctuation): 'upper' UPPERCASE; 'lower' lowercase; 'title' Capitalize Each Word; 'sentence' Capitalize each sentence; 'capitalize' Capitalize only the first letter; 'swap' invert every letter's case; 'alternate' aLtErNaTe lower/upper. Identifier cases (re-tokenize words): 'camel' camelCase; 'pascal' PascalCase; 'snake' snake_case; 'constant' CONSTANT_CASE; 'kebab' kebab-case; 'train' Train-Case; 'dot' dot.case; 'path' path/case. Default 'upper'."
                    }
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
