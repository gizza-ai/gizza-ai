//! gizza-ai/password-generator — generate strong random passwords or passphrases.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure (getrandom CSPRNG) → all backends incl. the chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_password_generator_core::{generate_passphrase, generate_password};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_length")]
    length: u32,
    #[serde(default = "default_words")]
    words: u32,
    #[serde(default = "default_true")]
    uppercase: bool,
    #[serde(default = "default_true")]
    digits: bool,
    #[serde(default = "default_true")]
    symbols: bool,
    #[serde(default = "default_sep")]
    separator: String,
}
fn default_mode() -> String { "password".to_string() }
fn default_length() -> u32 { 16 }
fn default_words() -> u32 { 4 }
fn default_true() -> bool { true }
fn default_sep() -> String { "-".to_string() }

#[derive(Serialize)]
struct Resp {
    value: String,
    bits: f64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::enumv("mode", ["password", "passphrase"]).default("password").describe("'password' (random characters, default) or 'passphrase' (random words)."))
        .param(Param::integer("length").min(1.0).max(512.0).describe("Password length in characters (password mode, default 16)."))
        .param(Param::integer("words").min(1.0).max(20.0).describe("Number of words (passphrase mode, default 4)."))
        .param(Param::boolean("uppercase").default(true).describe("Include uppercase letters (password mode). Default true. (Lowercase is always included.)"))
        .param(Param::boolean("digits").default(true).describe("Include digits (password mode). Default true."))
        .param(Param::boolean("symbols").default(true).describe("Include symbols (password mode). Default true."))
        .param(Param::string("separator").default("-").describe("Word separator (passphrase mode). Default '-'."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PasswordGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/password-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a strong random password or passphrase",
    skill(
        description = "Generate a strong random password or passphrase locally with a cryptographic RNG. mode='password' (default) builds a random string of `length` characters from lowercase (always) plus uppercase/digits/symbols (toggle each); mode='passphrase' joins `words` random words with `separator`. Returns the generated value and its entropy in bits.",
        parameters = schema_json()
    )
)]
impl PasswordGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "password-generator", |a: Args| {
            let (value, bits) = match a.mode.as_str() {
                "passphrase" => generate_passphrase(a.words as usize, &a.separator),
                "" | "password" => generate_password(a.length as usize, a.uppercase, a.digits, a.symbols),
                other => Err(format!("mode {other:?} not supported (password|passphrase)")),
            }
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
                    "mode":      { "type": "string", "enum": ["password", "passphrase"], "default": "password", "description": "'password' (random characters, default) or 'passphrase' (random words)." },
                    "length":    { "type": "integer", "minimum": 1, "maximum": 512, "description": "Password length in characters (password mode, default 16)." },
                    "words":     { "type": "integer", "minimum": 1, "maximum": 20, "description": "Number of words (passphrase mode, default 4)." },
                    "uppercase": { "type": "boolean", "default": true, "description": "Include uppercase letters (password mode). Default true. (Lowercase is always included.)" },
                    "digits":    { "type": "boolean", "default": true, "description": "Include digits (password mode). Default true." },
                    "symbols":   { "type": "boolean", "default": true, "description": "Include symbols (password mode). Default true." },
                    "separator": { "type": "string", "default": "-", "description": "Word separator (passphrase mode). Default '-'." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
