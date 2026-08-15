//! gizza-ai/stopword-filter — strip stop words out of text using a built-in
//! multilingual list, a custom list, or both. Thin wrapper around the core; the
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_stopword_filter_core::filter;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default)]
    custom_words: String,
    #[serde(default)]
    keep_words: String,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default)]
    remove_punctuation: bool,
    #[serde(default = "default_output")]
    output: String,
}
fn default_language() -> String {
    "english".to_string()
}
fn default_output() -> String {
    "text".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to filter. Up to 200,000 characters."),
        )
        .param(
            Param::enumv(
                "language",
                [
                    "english",
                    "spanish",
                    "french",
                    "german",
                    "italian",
                    "portuguese",
                    "dutch",
                    "russian",
                    "none",
                ],
            )
            .default("english")
            .describe("Which built-in stop-word list to remove. 'none' skips the built-in list so only custom_words is removed."),
        )
        .param(
            Param::string("custom_words")
                .default("")
                .describe("Extra words to remove on top of the built-in list, separated by commas, semicolons, or whitespace."),
        )
        .param(
            Param::string("keep_words")
                .default("")
                .describe("Words that must never be removed even when a list contains them (same separators as custom_words). Useful for terms like 'not' or 'no'."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("When false (default), 'The' and 'the' both match a list entry; true removes only exact-case matches."),
        )
        .param(
            Param::boolean("remove_punctuation")
                .default(false)
                .describe("When true, punctuation is stripped as well, leaving a bare token stream. Line breaks are always kept."),
        )
        .param(
            Param::enumv("output", ["text", "removed", "stats"])
                .default("text")
                .describe("Which view to return: 'text' (default) is the cleaned text, 'removed' lists each removed stop word with its count, 'stats' is a word-count summary."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct StopwordFilter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/stopword-filter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove stop words from text",
    skill(
        description = "Remove stop words (the, and, of, …) from text using a built-in list for English, Spanish, French, German, Italian, Portuguese, Dutch, or Russian — or set language='none' and supply your own list in `custom_words`. Matching is whole-word, so 'the' never touches 'theatre', and contractions stay one token. `keep_words` protects words that must survive. `output` picks the view: 'text' (cleaned text), 'removed' (each removed word with its count), or 'stats' (counts summary). Set remove_punctuation=true for a bare token stream.",
        parameters = schema_json()
    ),
)]
impl StopwordFilter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "stopword-filter", |a: Args| {
            filter(
                &a.text,
                &a.language,
                &a.custom_words,
                &a.keep_words,
                a.case_sensitive,
                a.remove_punctuation,
                &a.output,
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
                    "text": { "type": "string", "description": "The text to filter. Up to 200,000 characters." },
                    "language": {
                        "type": "string",
                        "enum": ["english", "spanish", "french", "german", "italian", "portuguese", "dutch", "russian", "none"],
                        "default": "english",
                        "description": "Which built-in stop-word list to remove. 'none' skips the built-in list so only custom_words is removed."
                    },
                    "custom_words": { "type": "string", "default": "", "description": "Extra words to remove on top of the built-in list, separated by commas, semicolons, or whitespace." },
                    "keep_words": { "type": "string", "default": "", "description": "Words that must never be removed even when a list contains them (same separators as custom_words). Useful for terms like 'not' or 'no'." },
                    "case_sensitive": { "type": "boolean", "default": false, "description": "When false (default), 'The' and 'the' both match a list entry; true removes only exact-case matches." },
                    "remove_punctuation": { "type": "boolean", "default": false, "description": "When true, punctuation is stripped as well, leaving a bare token stream. Line breaks are always kept." },
                    "output": {
                        "type": "string",
                        "enum": ["text", "removed", "stats"],
                        "default": "text",
                        "description": "Which view to return: 'text' (default) is the cleaned text, 'removed' lists each removed stop word with its count, 'stats' is a word-count summary."
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

    #[test]
    fn descriptor_language_enum_matches_the_core_list() {
        let d = descriptor();
        let p = d.params.iter().find(|p| p.name == "language").unwrap();
        let variants = match &p.kind {
            gizza_ai_block_utils::ParamKind::Enum(v) => v.clone(),
            other => panic!("language should be an enum, got {other:?}"),
        };
        assert_eq!(
            variants,
            gizza_ai_stopword_filter_core::LANGUAGES
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn descriptor_output_enum_matches_the_core_list() {
        let d = descriptor();
        let p = d.params.iter().find(|p| p.name == "output").unwrap();
        let variants = match &p.kind {
            gizza_ai_block_utils::ParamKind::Enum(v) => v.clone(),
            other => panic!("output should be an enum, got {other:?}"),
        };
        assert_eq!(
            variants,
            gizza_ai_stopword_filter_core::OUTPUTS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn every_param_is_described() {
        for p in descriptor().params {
            assert!(!p.description.is_empty(), "{} needs a description", p.name);
        }
    }
}
