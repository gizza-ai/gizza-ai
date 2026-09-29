//! gizza-ai/caesar-cipher-breaker — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_top")]
    top: u32,
    #[serde(default)]
    shift_digits: bool,
}

fn default_output() -> String {
    "best".into()
}
fn default_language() -> String {
    "english".into()
}
fn default_top() -> u32 {
    5
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("Caesar-shifted ciphertext to crack. The scorer uses A-Z letters only, preserving case, spacing, punctuation, symbols, and (unless shift_digits is true) digits in the recovered plaintext. A few sentences gives the frequency scorer enough evidence; limit 20,000 characters."))
        .param(Param::enumv("output", gizza_ai_caesar_cipher_breaker_core::OUTPUTS).default("best").describe("Output view. best (default) prints the best shift, confidence note, and recovered plaintext. ranked adds the top candidate table. all lists all 26 decryptions. report adds index-of-coincidence and letter-frequency diagnostics."))
        .param(Param::enumv("language", gizza_ai_caesar_cipher_breaker_core::LANGUAGES).default("english").describe("Expected plaintext language used for letter-frequency scoring: english (default), french, german, spanish, italian, or portuguese."))
        .param(Param::integer("top").default(5).min(1.0).max(26.0).describe("How many candidates to show in the ranked view, 1 through 26. Default 5. Ignored by best/all/report except for validation."))
        .param(Param::boolean("shift_digits").default(false).describe("Also rotate digits 0-9 backward by shift modulo 10. Default false because ordinary Caesar ciphers shift letters only."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/caesar-cipher-breaker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Crack Caesar ciphers by frequency analysis",
    skill(
        description = "Crack a Caesar cipher automatically by trying all 26 shifts and scoring each candidate against language letter frequencies. Returns the likely shift, a confidence estimate, and the recovered plaintext, with optional ranked/all/report views, six language profiles, and an option to rotate digits too.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "caesar-cipher-breaker", |a: Args| {
            gizza_ai_caesar_cipher_breaker_core::crack(
                &a.input,
                &a.output,
                &a.language,
                a.top,
                a.shift_digits,
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
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived["required"], serde_json::json!(["input"]));
        assert_eq!(
            derived["properties"]["output"]["enum"],
            serde_json::json!(["best", "ranked", "all", "report"])
        );
        assert_eq!(
            derived["properties"]["language"]["enum"],
            serde_json::json!([
                "english",
                "french",
                "german",
                "spanish",
                "italian",
                "portuguese"
            ])
        );
        assert_eq!(
            derived["properties"]["top"]["default"],
            serde_json::json!(5)
        );
        assert!(derived["properties"]
            .as_object()
            .unwrap()
            .values()
            .all(|p| p
                .get("description")
                .and_then(|d| d.as_str())
                .is_some_and(|s| !s.is_empty())));
    }
}
