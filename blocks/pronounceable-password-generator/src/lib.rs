//! gizza-ai/pronounceable-password-generator — build easy-to-say, easy-to-type
//! passwords from consonant/vowel phoneme patterns, then inject digits + symbols.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure (getrandom CSPRNG) → all backends incl. the chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_pronounceable_password_generator_core::generate_pronounceable;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_length")]
    length: u32,
    #[serde(default = "default_true")]
    capitalize: bool,
    #[serde(default = "default_digits")]
    digits: u32,
    #[serde(default = "default_symbols")]
    symbols: u32,
}
fn default_length() -> u32 { 12 }
fn default_true() -> bool { true }
fn default_digits() -> u32 { 2 }
fn default_symbols() -> u32 { 1 }

#[derive(Serialize)]
struct Resp {
    value: String,
    bits: f64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::integer("length").min(4.0).max(64.0).describe("Number of pronounceable letters (alternating consonant/vowel). 4–64, default 12."))
        .param(Param::boolean("capitalize").default(true).describe("Capitalize the first letter. Default true."))
        .param(Param::integer("digits").min(0.0).max(12.0).describe("Random digits appended to the end. 0–12, default 2."))
        .param(Param::integer("symbols").min(0.0).max(12.0).describe("Random symbols (from !@#$%&*?-_+=) appended after the digits. 0–12, default 1."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PronounceablePasswordGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pronounceable-password-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a pronounceable, easy-to-type password",
    skill(
        description = "Generate a pronounceable password locally with a cryptographic RNG. `length` letters strictly alternate consonant/vowel (starting with a consonant) so the core is always speakable (e.g. `bofuka`); `digits` random digits and `symbols` random symbols are then appended, and `capitalize` upper-cases the first letter. Returns the generated value and its entropy in bits.",
        parameters = schema_json()
    )
)]
impl PronounceablePasswordGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pronounceable-password-generator", |a: Args| {
            let (value, bits) = generate_pronounceable(
                a.length as usize,
                a.capitalize,
                a.digits as usize,
                a.symbols as usize,
            )
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { value, bits })
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
                    "length":     { "type": "integer", "minimum": 4, "maximum": 64, "description": "Number of pronounceable letters (alternating consonant/vowel). 4–64, default 12." },
                    "capitalize": { "type": "boolean", "default": true, "description": "Capitalize the first letter. Default true." },
                    "digits":     { "type": "integer", "minimum": 0, "maximum": 12, "description": "Random digits appended to the end. 0–12, default 2." },
                    "symbols":    { "type": "integer", "minimum": 0, "maximum": 12, "description": "Random symbols (from !@#$%&*?-_+=) appended after the digits. 0–12, default 1." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
