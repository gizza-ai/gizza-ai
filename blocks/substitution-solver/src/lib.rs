//! gizza-ai/substitution-solver — chat skill block on the shared tool abstraction.
//! Solves and assists monoalphabetic substitution cryptograms locally.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_mode() -> String {
    "solve".to_string()
}
fn default_effort() -> String {
    "standard".to_string()
}
fn default_keep_layout() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    key: String,
    #[serde(default)]
    cribs: String,
    #[serde(default = "default_effort")]
    effort: String,
    #[serde(default = "default_keep_layout")]
    keep_layout: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("Ciphertext or plaintext to process. ASCII letters A-Z are substituted; punctuation, digits, spacing, and letter case are preserved when keep_layout=true. Maximum 100000 characters."),
        )
        .param(
            Param::enumv("mode", ["solve", "decode", "encode", "analyze"])
                .default("solve")
                .describe("Operation to run: solve automatically searches for a monoalphabetic key, decode applies a cipher-to-plain key, encode applies the inverse key, and analyze reports frequencies plus a starting key."),
        )
        .param(
            Param::string("key")
                .default("")
                .describe("Optional 26-character cipher-to-plaintext alphabet for decode/encode, in cipher-letter order A-Z. Use '?' for unknown letters. Example Atbash key: zyxwvutsrqponmlkjihgfedcba. Leave blank for solve/analyze."),
        )
        .param(
            Param::string("cribs")
                .default("")
                .describe("Optional comma/semicolon/newline-separated known mappings for solve, such as X=e or QVW=the. Cribs lock cipher letters to plaintext letters before hill-climbing."),
        )
        .param(
            Param::enumv("effort", ["quick", "standard", "thorough"])
                .default("standard")
                .describe("Search effort for mode=solve. quick uses 3 deterministic restarts, standard uses 15, and thorough uses 50. Higher effort can improve hard ciphers but takes longer."),
        )
        .param(
            Param::boolean("keep_layout")
                .default(true)
                .describe("Preserve original spacing, punctuation, and case in the output. When false, output letters only in grouped five-letter blocks."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/substitution-solver",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Solve and analyze monoalphabetic substitution ciphers",
    skill(
        description = "Solve, decode, encode, or analyze monoalphabetic substitution ciphers and cryptograms. mode='solve' (default) hill-climbs a cipher-to-plain alphabet against English letter statistics with effort='quick'|'standard'|'thorough' and optional cribs such as X=e or QVW=the. mode='decode' and mode='encode' use a 26-letter cipher-to-plaintext key (use ? for unknown letters). mode='analyze' reports frequency, repeated bigrams, index of coincidence, and a frequency-matched starting key. Runs locally; best for English simple-substitution puzzles, not Vigenere, Playfair, homophonic, or modern encryption.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "substitution-solver", |a: Args| {
            gizza_ai_substitution_solver_core::run(
                &a.text,
                &a.mode,
                &a.key,
                &a.cribs,
                &a.effort,
                a.keep_layout,
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
                    "text": { "type": "string", "description": "Ciphertext or plaintext to process. ASCII letters A-Z are substituted; punctuation, digits, spacing, and letter case are preserved when keep_layout=true. Maximum 100000 characters." },
                    "mode": { "type": "string", "enum": ["solve", "decode", "encode", "analyze"], "default": "solve", "description": "Operation to run: solve automatically searches for a monoalphabetic key, decode applies a cipher-to-plain key, encode applies the inverse key, and analyze reports frequencies plus a starting key." },
                    "key": { "type": "string", "default": "", "description": "Optional 26-character cipher-to-plaintext alphabet for decode/encode, in cipher-letter order A-Z. Use '?' for unknown letters. Example Atbash key: zyxwvutsrqponmlkjihgfedcba. Leave blank for solve/analyze." },
                    "cribs": { "type": "string", "default": "", "description": "Optional comma/semicolon/newline-separated known mappings for solve, such as X=e or QVW=the. Cribs lock cipher letters to plaintext letters before hill-climbing." },
                    "effort": { "type": "string", "enum": ["quick", "standard", "thorough"], "default": "standard", "description": "Search effort for mode=solve. quick uses 3 deterministic restarts, standard uses 15, and thorough uses 50. Higher effort can improve hard ciphers but takes longer." },
                    "keep_layout": { "type": "boolean", "default": true, "description": "Preserve original spacing, punctuation, and case in the output. When false, output letters only in grouped five-letter blocks." }
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
