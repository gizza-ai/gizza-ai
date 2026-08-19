//! gizza-ai/multilingual-stemmer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_min_length")]
    min_length: u32,
    #[serde(default = "default_lowercase")]
    lowercase: bool,
}

fn default_language() -> String {
    "english".to_string()
}
fn default_output() -> String {
    "text".to_string()
}
fn default_min_length() -> u32 {
    1
}
fn default_lowercase() -> bool {
    true
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .multiline()
                .describe("Text to stem. Paste words, sentences, search queries or a small corpus; punctuation, spacing and line breaks are preserved in text output. Limit: 200,000 characters."),
        )
        .param(
            Param::enumv("language", [
                "arabic", "danish", "dutch", "english", "finnish", "french", "german", "greek", "hungarian", "italian", "norwegian", "portuguese", "romanian", "russian", "spanish", "swedish", "tamil", "turkish",
            ])
            .default("english")
            .describe("Snowball stemming language to apply. Pick the language of the input text; using the wrong language gives misleading stems. Default: english."),
        )
        .param(
            Param::enumv("output", ["text", "stems", "mapping", "table", "json"])
                .default("text")
                .describe("Output format: text preserves the original layout with each word replaced by its stem; stems lists unique stems; mapping lists surface form -> stem; table counts stems; json returns machine-readable groups and stats. Default: text."),
        )
        .param(
            Param::integer("min_length")
                .default(1)
                .min(1.0)
                .max(30.0)
                .describe("Minimum word length, in Unicode characters, that will be stemmed. Shorter words pass through unchanged. Use this to protect abbreviations or short codes. Default: 1; maximum: 30."),
        )
        .param(
            Param::boolean("lowercase")
                .default(true)
                .describe("Lowercase words before stemming. Snowball algorithms are defined for lowercase input, so this should usually stay on. Turn it off only when case distinctions are meaningful. Default: true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/multilingual-stemmer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Stem text in 18 languages with Snowball algorithms",
    skill(
        description = "Stem words in 18 languages with pure-Rust Snowball algorithms. Pass text as 'input' and choose a 'language' such as english, german, spanish, french, russian, arabic, tamil or turkish. The default 'output=text' preserves punctuation, spacing and line breaks while replacing each word with its stem; 'stems' lists unique stems, 'mapping' returns surface form -> stem pairs, 'table' counts stems, and 'json' returns machine-readable groups and corpus statistics. Optional 'min_length' leaves short words unchanged, and 'lowercase' controls whether words are lowercased before stemming. Useful for search indexing, keyword normalization, deduplicating inflected terms and comparing vocabulary across small corpora. Stemming is not lemmatization: stems may not be dictionary words.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "multilingual-stemmer", |a: Args| {
            gizza_ai_multilingual_stemmer_core::run(
                &a.input,
                &a.language,
                &a.output,
                a.min_length,
                a.lowercase,
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
                    "input": {
                        "type": "string",
                        "description": "Text to stem. Paste words, sentences, search queries or a small corpus; punctuation, spacing and line breaks are preserved in text output. Limit: 200,000 characters."
                    },
                    "language": {
                        "type": "string",
                        "enum": ["arabic", "danish", "dutch", "english", "finnish", "french", "german", "greek", "hungarian", "italian", "norwegian", "portuguese", "romanian", "russian", "spanish", "swedish", "tamil", "turkish"],
                        "default": "english",
                        "description": "Snowball stemming language to apply. Pick the language of the input text; using the wrong language gives misleading stems. Default: english."
                    },
                    "output": {
                        "type": "string",
                        "enum": ["text", "stems", "mapping", "table", "json"],
                        "default": "text",
                        "description": "Output format: text preserves the original layout with each word replaced by its stem; stems lists unique stems; mapping lists surface form -> stem; table counts stems; json returns machine-readable groups and stats. Default: text."
                    },
                    "min_length": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 30,
                        "default": 1,
                        "description": "Minimum word length, in Unicode characters, that will be stemmed. Shorter words pass through unchanged. Use this to protect abbreviations or short codes. Default: 1; maximum: 30."
                    },
                    "lowercase": {
                        "type": "boolean",
                        "default": true,
                        "description": "Lowercase words before stemming. Snowball algorithms are defined for lowercase input, so this should usually stay on. Turn it off only when case distinctions are meaningful. Default: true."
                    }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
