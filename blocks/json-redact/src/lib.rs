//! gizza-ai/json-redact — detect and mask secrets (tokens, API keys, passwords,
//! emails, private keys) in a JSON document while preserving its structure. Thin
//! wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_redact_core::{parse_extra_keys, redact_json, Options, Style};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_style")]
    style: String,
    #[serde(default = "default_placeholder")]
    placeholder: String,
    #[serde(default = "default_true")]
    detect_values: bool,
    #[serde(default)]
    extra_keys: String,
}

fn default_style() -> String {
    "redacted".to_string()
}
fn default_placeholder() -> String {
    "[REDACTED]".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON document to scan and redact. Must be valid JSON (object, array or value)."),
        )
        .param(
            Param::enumv("style", ["redacted", "mask", "null", "empty", "preserve-length"])
                .default("redacted")
                .describe(
                    "How to replace each detected secret: redacted (default) inserts the placeholder text; mask inserts '***'; null replaces with JSON null; empty replaces with \"\"; preserve-length replaces a string with '*' repeated to its original length.",
                ),
        )
        .param(
            Param::string("placeholder")
                .default("[REDACTED]")
                .describe("Replacement text used when style=redacted. Default '[REDACTED]'."),
        )
        .param(
            Param::boolean("detect_values")
                .default(true)
                .describe(
                    "Also scan string VALUES for secret patterns (JWTs, AWS/OpenAI/GitHub/Stripe/Google/Slack keys, PEM private keys, emails), not just sensitive key names. Default true.",
                ),
        )
        .param(
            Param::string("extra_keys")
                .default("")
                .describe(
                    "Comma-separated extra key-name markers to always redact, e.g. 'nickname,phone'. Matched case-insensitively as substrings of the normalized key. Default empty.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonRedact;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-redact",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect and mask secrets in a JSON document",
    skill(
        description = "Detect and mask secrets — API keys, tokens, passwords, private keys and emails — inside a JSON document before sharing it, keeping the JSON structure and key order intact. Detects both sensitive KEY names (password, secret, api_key, token, private_key, authorization, email, ssn…) and, when detect_values is on (default), secret-looking string VALUES (JWTs, AWS AKIA/OpenAI sk-/GitHub gh?_/Stripe/Google AIza/Slack keys, PEM private-key blocks, emails). style controls the replacement (redacted→placeholder, mask→'***', null, empty, preserve-length). placeholder sets the redacted text; extra_keys adds your own field-name markers. Returns the redacted JSON plus a count and the JSON path of every redacted value. Runs locally — the document never leaves the device.",
        parameters = schema_json()
    ),
)]
impl JsonRedact {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-redact", |a: Args| {
            let style = Style::parse(&a.style).map_err(SkillError::InvalidArgs)?;
            let opts = Options {
                style,
                placeholder: &a.placeholder,
                detect_values: a.detect_values,
                extra_keys: parse_extra_keys(&a.extra_keys),
            };
            redact_json(&a.json, &opts).map_err(SkillError::InvalidArgs)
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
                    "json": { "type": "string", "description": "The JSON document to scan and redact. Must be valid JSON (object, array or value)." },
                    "style": { "type": "string", "enum": ["redacted", "mask", "null", "empty", "preserve-length"], "default": "redacted", "description": "How to replace each detected secret: redacted (default) inserts the placeholder text; mask inserts '***'; null replaces with JSON null; empty replaces with \"\"; preserve-length replaces a string with '*' repeated to its original length." },
                    "placeholder": { "type": "string", "default": "[REDACTED]", "description": "Replacement text used when style=redacted. Default '[REDACTED]'." },
                    "detect_values": { "type": "boolean", "default": true, "description": "Also scan string VALUES for secret patterns (JWTs, AWS/OpenAI/GitHub/Stripe/Google/Slack keys, PEM private keys, emails), not just sensitive key names. Default true." },
                    "extra_keys": { "type": "string", "default": "", "description": "Comma-separated extra key-name markers to always redact, e.g. 'nickname,phone'. Matched case-insensitively as substrings of the normalized key. Default empty." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
