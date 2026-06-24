//! gizza-ai/string-obfuscator — mask or visually obfuscate a string for
//! screenshots, demos, and docs. Thin chat-skill wrapper around
//! `gizza-ai-string-obfuscator-core`. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    mask_char: String,
    /// How many leading characters to keep visible in `mask` mode (default 0).
    #[serde(default)]
    keep_start: u32,
    /// How many trailing characters to keep visible in `mask` mode (default 0).
    #[serde(default)]
    keep_end: u32,
    /// ROT shift for `rot` mode; the core takes it mod 26 (default 13 = ROT13).
    #[serde(default = "default_rot")]
    rot_n: u32,
}

fn default_rot() -> u32 {
    13
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to mask or obfuscate."),
        )
        .param(
            Param::enumv("mode", ["mask", "rot", "leetspeak", "homoglyph"])
                .default("mask")
                .describe(
                    "How to transform the text. 'mask' (default) keeps the first keep_start and last keep_end characters and replaces the middle with mask_char — for redacting secrets/PII in screenshots. 'rot' is a Caesar/ROT-N letter rotation (rot_n, default 13 = ROT13). 'leetspeak' swaps letters for look-alike digits (a→4, e→3, o→0…). 'homoglyph' swaps ASCII letters for identical-looking Unicode (Cyrillic) so the text looks the same but is a different string.",
                ),
        )
        .param(
            Param::string("mask_char")
                .default("*")
                .describe("The character used to replace masked characters in 'mask' mode. Only the first character is used. Default '*'."),
        )
        .param(
            Param::integer("keep_start")
                .default(0)
                .min(0.0)
                .describe("In 'mask' mode, how many leading characters to keep visible. Default 0."),
        )
        .param(
            Param::integer("keep_end")
                .default(0)
                .min(0.0)
                .describe("In 'mask' mode, how many trailing characters to keep visible. Default 0."),
        )
        .param(
            Param::integer("rot_n")
                .default(13)
                .min(0.0)
                .max(gizza_ai_string_obfuscator_core::MAX_ROT as f64)
                .describe("In 'rot' mode, the shift applied to each letter, 0-25 (taken mod 26). Default 13 (ROT13, which is self-inverse)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct StringObfuscator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/string-obfuscator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Mask or obfuscate a string for screenshots and demos",
    skill(
        description = "Mask or visually obfuscate a string for screenshots, demos, and docs. mode='mask' (default) redacts the middle of a value while keeping keep_start leading and keep_end trailing characters visible, replacing the rest with mask_char (default '*') — e.g. an API key 'sk-1234…cdef'. mode='rot' applies a Caesar/ROT-N rotation (rot_n, default 13 = the self-inverse ROT13). mode='leetspeak' swaps letters for look-alike digits/symbols (a→4, e→3, i→1, o→0, s→5, t→7). mode='homoglyph' swaps ASCII letters for identical-looking Unicode (mostly Cyrillic) so the text looks the same but is a distinct string. Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl StringObfuscator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "string-obfuscator", |a: Args| {
            gizza_ai_string_obfuscator_core::obfuscate(
                &a.text,
                &a.mode,
                &a.mask_char,
                a.keep_start as usize,
                a.keep_end as usize,
                a.rot_n,
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
                    "text": { "type": "string", "description": "The text to mask or obfuscate." },
                    "mode": { "type": "string", "enum": ["mask", "rot", "leetspeak", "homoglyph"], "default": "mask", "description": "How to transform the text. 'mask' (default) keeps the first keep_start and last keep_end characters and replaces the middle with mask_char — for redacting secrets/PII in screenshots. 'rot' is a Caesar/ROT-N letter rotation (rot_n, default 13 = ROT13). 'leetspeak' swaps letters for look-alike digits (a→4, e→3, o→0…). 'homoglyph' swaps ASCII letters for identical-looking Unicode (Cyrillic) so the text looks the same but is a different string." },
                    "mask_char": { "type": "string", "default": "*", "description": "The character used to replace masked characters in 'mask' mode. Only the first character is used. Default '*'." },
                    "keep_start": { "type": "integer", "minimum": 0, "default": 0, "description": "In 'mask' mode, how many leading characters to keep visible. Default 0." },
                    "keep_end": { "type": "integer", "minimum": 0, "default": 0, "description": "In 'mask' mode, how many trailing characters to keep visible. Default 0." },
                    "rot_n": { "type": "integer", "minimum": 0, "maximum": 25, "default": 13, "description": "In 'rot' mode, the shift applied to each letter, 0-25 (taken mod 26). Default 13 (ROT13, which is self-inverse)." }
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
