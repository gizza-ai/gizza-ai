//! gizza-ai/base64-validator — check whether a string is well-formed Base64
//! (RFC 4648 §4) or Base64url (§5) and, when it isn't, report exactly what is
//! wrong. Thin chat-skill wrapper around `gizza-ai-base64-validator-core`; the
//! chat schema is single-sourced from `descriptor()` (shared shape across chat
//! + CLI) and the handler delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    variant: String,
    #[serde(default)]
    padding: String,
    #[serde(default = "default_true")]
    ignore_whitespace: bool,
    #[serde(default)]
    max_line_length: i64,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The Base64 or Base64url string to check. A wrapping pair of quotes and a leading 'data:<mime>;base64,' URI prefix are detected and skipped, so pasted JSON values and data URIs work."),
        )
        .param(
            Param::enumv("variant", ["auto", "standard", "url-safe"])
                .default("auto")
                .describe("Which alphabet to enforce. 'auto' (default) accepts either and reports which one the string uses, flagging a string that mixes them. 'standard' requires RFC 4648 §4 ('+' and '/'); 'url-safe' requires §5 ('-' and '_')."),
        )
        .param(
            Param::enumv("padding", ["optional", "required", "forbidden"])
                .default("optional")
                .describe("What to expect of the trailing '=' padding. 'optional' (default) accepts a padded or unpadded string but still rejects the wrong number of '='; 'required' is strict RFC 4648 (length must be a multiple of 4); 'forbidden' expects an unpadded string, as used for JWT segments."),
        )
        .param(
            Param::boolean("ignore_whitespace")
                .default(true)
                .describe("Whether spaces, tabs, and line breaks inside the data are ignored (default true), as they are in MIME and PEM. When false, each whitespace character is reported as invalid with its position."),
        )
        .param(
            Param::integer("max_line_length")
                .default(0)
                .min(0.0)
                .max(998.0)
                .describe("Maximum characters per line, checked against the pasted text. 0 (default) skips the check; 76 is MIME, 64 is PEM."),
        )
        .param(
            Param::enumv("output", ["text", "json"])
                .default("text")
                .describe("Report format: a readable 'text' report (default) or a 'json' object with valid/alphabet/problems/warnings fields for scripting."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Base64Validator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/base64-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check whether a string is valid Base64 and explain what is wrong.",
    skill(
        description = "Validate a Base64 or Base64url string and explain precisely why it is invalid, instead of only returning pass/fail. Reports every character outside the alphabet with its 1-based position and line/column, padding that is misplaced, too long, or the wrong count for the length, a length that cannot be Base64, lines over a length limit, and whether the string mixes the standard ('+', '/') and URL-safe ('-', '_') alphabets. For a valid string it reports the detected alphabet, the exact decoded byte count, whether the payload is text or a sniffed binary type (PNG, PDF, ZIP, …), and warns when unused trailing bits make it non-canonical (RFC 4648 §3.5). When the problems are mechanically fixable it suggests a corrected string. An invalid string is a normal result, not an error. Use variant to require a specific alphabet, padding to require or forbid trailing '=', ignore_whitespace=false for a strict check, max_line_length=76 for MIME or 64 for PEM, and output='json' for a machine-readable report.",
        parameters = schema_json()
    ),
)]
impl Base64Validator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } and routes
        // errors through GuestResult::error.
        match run_skill(&body, "base64-validator", |a: Args| {
            gizza_ai_base64_validator_core::validate(
                &a.input,
                &a.variant,
                &a.padding,
                a.ignore_whitespace,
                a.max_line_length,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The Base64 or Base64url string to check. A wrapping pair of quotes and a leading 'data:<mime>;base64,' URI prefix are detected and skipped, so pasted JSON values and data URIs work." },
                    "variant": { "type": "string", "enum": ["auto", "standard", "url-safe"], "default": "auto", "description": "Which alphabet to enforce. 'auto' (default) accepts either and reports which one the string uses, flagging a string that mixes them. 'standard' requires RFC 4648 §4 ('+' and '/'); 'url-safe' requires §5 ('-' and '_')." },
                    "padding": { "type": "string", "enum": ["optional", "required", "forbidden"], "default": "optional", "description": "What to expect of the trailing '=' padding. 'optional' (default) accepts a padded or unpadded string but still rejects the wrong number of '='; 'required' is strict RFC 4648 (length must be a multiple of 4); 'forbidden' expects an unpadded string, as used for JWT segments." },
                    "ignore_whitespace": { "type": "boolean", "default": true, "description": "Whether spaces, tabs, and line breaks inside the data are ignored (default true), as they are in MIME and PEM. When false, each whitespace character is reported as invalid with its position." },
                    "max_line_length": { "type": "integer", "default": 0, "minimum": 0, "maximum": 998, "description": "Maximum characters per line, checked against the pasted text. 0 (default) skips the check; 76 is MIME, 64 is PEM." },
                    "output": { "type": "string", "enum": ["text", "json"], "default": "text", "description": "Report format: a readable 'text' report (default) or a 'json' object with valid/alphabet/problems/warnings fields for scripting." }
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
