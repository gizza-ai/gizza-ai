//! gizza-ai/urdu-romanizer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_scheme")]
    scheme: String,
    #[serde(default = "default_short_vowels")]
    short_vowels: String,
    #[serde(default = "default_common_words")]
    common_words: bool,
    #[serde(default = "default_digits")]
    digits: String,
    #[serde(default = "default_punctuation")]
    punctuation: String,
    #[serde(default = "default_capitalization")]
    capitalization: String,
}

fn default_scheme() -> String {
    "informal".to_string()
}
fn default_short_vowels() -> String {
    "insert-a".to_string()
}
fn default_common_words() -> bool {
    true
}
fn default_digits() -> String {
    "latin".to_string()
}
fn default_punctuation() -> String {
    "latin".to_string()
}
fn default_capitalization() -> String {
    "sentence".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("Urdu script text to transliterate into Roman (Latin) Urdu. Paragraphs, punctuation, emoji and existing Latin text are preserved."))
        .param(Param::enumv("scheme", ["informal", "ala-lc", "iso15919"]).default("informal").describe("Romanization scheme. informal is plain ASCII Roman Urdu; ala-lc and iso15919 keep more Arabic-letter distinctions with diacritics."))
        .param(Param::enumv("short_vowels", ["insert-a", "marks-only", "omit"]).default("insert-a").describe("How to handle unwritten short vowels: insert-a adds a pronounceable default between consonants, marks-only honours only typed vowel marks, omit drops all short-vowel marks."))
        .param(Param::boolean("common_words").default(true).describe("Use a small built-in list of common Urdu words for conventional informal spellings such as ہے → hai and پاکستان → pakistan. Applies only to the informal scheme."))
        .param(Param::enumv("digits", ["latin", "keep"]).default("latin").describe("Convert Urdu and Arabic-Indic digits to ASCII 0-9, or keep digit characters exactly as typed."))
        .param(Param::enumv("punctuation", ["latin", "keep"]).default("latin").describe("Convert Urdu punctuation such as ۔ ، ؟ ؛ ٪ to Latin punctuation, or preserve the original punctuation characters."))
        .param(Param::enumv("capitalization", ["none", "sentence", "title"]).default("sentence").describe("Capitalization for Roman output: lowercase/no change, sentence-case after punctuation and line breaks, or title-case each word."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/urdu-romanizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Transliterate Urdu script into Roman Urdu",
    skill(
        description = "Transliterate Urdu script into Roman (Latin) Urdu locally. Choose informal ASCII Roman Urdu, ALA-LC or ISO 15919 schemes; control short-vowel handling, common-word spellings, digit conversion, punctuation conversion and capitalization. The converter is deterministic and preserves line breaks, emoji and existing Latin text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "urdu-romanizer", |a: Args| {
            gizza_ai_urdu_romanizer_core::run(
                &a.input,
                &a.scheme,
                &a.short_vowels,
                a.common_words,
                &a.digits,
                &a.punctuation,
                &a.capitalization,
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
    fn descriptor_documents_every_param() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert_eq!(props.len(), 7);
        for (name, spec) in props {
            assert!(
                spec["description"].as_str().unwrap_or_default().len() > 40,
                "{name} needs a useful description"
            );
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["input"],
            "properties": {
                "input": { "type": "string", "description": "Urdu script text to transliterate into Roman (Latin) Urdu. Paragraphs, punctuation, emoji and existing Latin text are preserved." },
                "scheme": { "type": "string", "enum": ["informal", "ala-lc", "iso15919"], "default": "informal", "description": "Romanization scheme. informal is plain ASCII Roman Urdu; ala-lc and iso15919 keep more Arabic-letter distinctions with diacritics." },
                "short_vowels": { "type": "string", "enum": ["insert-a", "marks-only", "omit"], "default": "insert-a", "description": "How to handle unwritten short vowels: insert-a adds a pronounceable default between consonants, marks-only honours only typed vowel marks, omit drops all short-vowel marks." },
                "common_words": { "type": "boolean", "default": true, "description": "Use a small built-in list of common Urdu words for conventional informal spellings such as ہے → hai and پاکستان → pakistan. Applies only to the informal scheme." },
                "digits": { "type": "string", "enum": ["latin", "keep"], "default": "latin", "description": "Convert Urdu and Arabic-Indic digits to ASCII 0-9, or keep digit characters exactly as typed." },
                "punctuation": { "type": "string", "enum": ["latin", "keep"], "default": "latin", "description": "Convert Urdu punctuation such as ۔ ، ؟ ؛ ٪ to Latin punctuation, or preserve the original punctuation characters." },
                "capitalization": { "type": "string", "enum": ["none", "sentence", "title"], "default": "sentence", "description": "Capitalization for Roman output: lowercase/no change, sentence-case after punctuation and line breaks, or title-case each word." }
            }
        });
        assert_eq!(derived, authored);
    }
}
