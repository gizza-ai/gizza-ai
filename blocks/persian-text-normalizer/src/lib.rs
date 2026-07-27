//! gizza-ai/persian-text-normalizer — Persian/Farsi text normalizer (chat skill block).
//!
//! Thin chat-skill wrapper around `gizza-ai-persian-text-normalizer-core`. The
//! chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI); the handler delegates to `block_utils::run_skill`. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, SkillError};
use gizza_ai_block_utils::{Input, Param, ToolDescriptor};
use gizza_ai_persian_text_normalizer_core::{normalize, Digits, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_true")]
    characters: bool,
    #[serde(default = "default_digits")]
    digits: String,
    #[serde(default = "default_true")]
    half_space: bool,
    #[serde(default = "default_true")]
    punctuation_spacing: bool,
    #[serde(default)]
    persian_punctuation: bool,
    #[serde(default)]
    remove_diacritics: bool,
    #[serde(default = "default_true")]
    whitespace: bool,
}

fn default_true() -> bool {
    true
}
fn default_digits() -> String {
    "persian".to_string()
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The Persian/Farsi text to normalize."),
        )
        .param(Param::boolean("characters").default(true).describe(
            "When true (default), replace Arabic letters with their Persian equivalents: \
ك -> ک, ي -> ی, ى (alef maksura) -> ی, and ة (teh marbuta) -> ه.",
        ))
        .param(
            Param::enumv("digits", ["persian", "english", "keep"])
                .default("persian")
                .describe(
                    "Digit script. 'persian' (default) converts ASCII (0-9) and Arabic-Indic \
(٠-٩) digits to Persian (۰-۹); 'english' converts Persian and Arabic-Indic digits to ASCII; \
'keep' leaves digits untouched.",
                ),
        )
        .param(Param::boolean("half_space").default(true).describe(
            "When true (default), fix half-spaces (ZWNJ / نیم‌فاصله): tidy spacing around \
existing ZWNJ characters and attach common affixes with a half-space — verb prefixes می/نمی and \
plural/comparative suffixes ها/های/هایی/تر/تری/ترین.",
        ))
        .param(
            Param::boolean("punctuation_spacing")
                .default(true)
                .describe(
                    "When true (default), correct punctuation spacing: remove spaces before . ، ؛ \
؟ ! : and leave exactly one space after. Decimal, time, and thousands separators (3.14, 10:30, \
1,000) are left intact.",
                ),
        )
        .param(
            Param::boolean("persian_punctuation")
                .default(false)
                .describe(
                    "When true, convert Latin punctuation to Persian equivalents: comma -> ، , \
semicolon -> ؛ , and question mark -> ؟. A comma between digits (thousands separator) is kept. \
Default false.",
                ),
        )
        .param(Param::boolean("remove_diacritics").default(false).describe(
            "When true, remove Arabic diacritics (harakat / تشکیل, e.g. fatha, damma, \
kasra, shadda, sukun) and the kashida (tatweel ـ). Default false.",
        ))
        .param(Param::boolean("whitespace").default(true).describe(
            "When true (default), collapse repeated spaces/tabs to one space, normalize \
line endings to \\n, drop blank surrounding lines, and trim the ends.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options_from(a: &Args) -> Result<Options, SkillError> {
    Ok(Options {
        characters: a.characters,
        digits: Digits::parse(&a.digits).map_err(SkillError::InvalidArgs)?,
        half_space: a.half_space,
        punctuation_spacing: a.punctuation_spacing,
        persian_punctuation: a.persian_punctuation,
        remove_diacritics: a.remove_diacritics,
        whitespace: a.whitespace,
    })
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/persian-text-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Normalize Persian/Farsi text: Arabic-to-Persian characters, digits, half-spaces (ZWNJ), punctuation spacing.",
    skill(
        description = "Normalize Persian/Farsi text in one configurable pass. Replace Arabic letters with Persian equivalents (characters = true: ك->ک, ي->ی, ى->ی, ة->ه), convert digits (digits = persian/english/keep; default persian maps 0-9 and ٠-٩ to ۰-۹), fix half-spaces / ZWNJ (half_space = true attaches می/نمی prefixes and ها/تر/ترین suffixes with a نیم‌فاصله and tidies stray spacing around existing ZWNJ), correct punctuation spacing (punctuation_spacing = true: no space before . ، ؛ ؟ ! :, one space after, sparing decimal/time/thousands separators), optionally convert Latin punctuation to Persian (persian_punctuation = true: , -> ،, ; -> ؛, ? -> ؟), optionally strip Arabic diacritics/kashida (remove_diacritics = true), and clean whitespace (whitespace = true collapses runs, normalizes line endings, trims). Steps run in the order characters -> remove diacritics -> digits -> Persian punctuation -> half-space -> punctuation spacing -> whitespace.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "persian-text-normalizer", |a: Args| {
            let opts = options_from(&a)?;
            Ok::<String, SkillError>(normalize(&a.text, opts))
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
                    "text": { "type": "string", "description": "The Persian/Farsi text to normalize." },
                    "characters": { "type": "boolean", "default": true, "description": "When true (default), replace Arabic letters with their Persian equivalents: ك -> ک, ي -> ی, ى (alef maksura) -> ی, and ة (teh marbuta) -> ه." },
                    "digits": { "type": "string", "enum": ["persian", "english", "keep"], "default": "persian", "description": "Digit script. 'persian' (default) converts ASCII (0-9) and Arabic-Indic (٠-٩) digits to Persian (۰-۹); 'english' converts Persian and Arabic-Indic digits to ASCII; 'keep' leaves digits untouched." },
                    "half_space": { "type": "boolean", "default": true, "description": "When true (default), fix half-spaces (ZWNJ / نیم‌فاصله): tidy spacing around existing ZWNJ characters and attach common affixes with a half-space — verb prefixes می/نمی and plural/comparative suffixes ها/های/هایی/تر/تری/ترین." },
                    "punctuation_spacing": { "type": "boolean", "default": true, "description": "When true (default), correct punctuation spacing: remove spaces before . ، ؛ ؟ ! : and leave exactly one space after. Decimal, time, and thousands separators (3.14, 10:30, 1,000) are left intact." },
                    "persian_punctuation": { "type": "boolean", "default": false, "description": "When true, convert Latin punctuation to Persian equivalents: comma -> ، , semicolon -> ؛ , and question mark -> ؟. A comma between digits (thousands separator) is kept. Default false." },
                    "remove_diacritics": { "type": "boolean", "default": false, "description": "When true, remove Arabic diacritics (harakat / تشکیل, e.g. fatha, damma, kasra, shadda, sukun) and the kashida (tatweel ـ). Default false." },
                    "whitespace": { "type": "boolean", "default": true, "description": "When true (default), collapse repeated spaces/tabs to one space, normalize line endings to \\n, drop blank surrounding lines, and trim the ends." }
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
