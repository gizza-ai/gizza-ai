//! gizza-ai/curl-command-parser — parse a `curl` command line into a structured
//! HTTP request (method, URL, query params, headers, body, auth, cookies, flags)
//! or rebuild a clean canonical `curl` command from it. Chat schema single-sourced
//! from descriptor() (which also drives the CLI); handler delegates to run_skill.
//! Pure → all backends (no host calls).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    mode: String,
    /// The curl command to parse or rebuild.
    #[serde(default)]
    command: String,
}

/// Single source for the chat schema (and CLI). `parse` (default) decodes the
/// command into structured JSON; `rebuild` returns a clean canonical command.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("mode", ["parse", "rebuild"])
                .default("parse")
                .describe("Direction: 'parse' (default) decodes the curl command into a structured request (JSON); 'rebuild' returns a clean, canonical curl command reconstructed from it."),
        )
        .param(
            Param::string("command")
                .required()
                .describe("The curl command to process, e.g. \"curl -X POST 'https://api.example.com/v1/items' -H 'Content-Type: application/json' -d '{\\\"name\\\":\\\"gizza\\\"}'\". Quotes, backslash line-continuations, attached short flags (-XPOST) and combined flags (-sSL) are all handled; a leading 'curl' is optional."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/curl-command-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a curl command into a structured request",
    skill(
        description = "Parse a curl command line into a structured HTTP request, or rebuild a clean canonical curl command from it. In parse mode (default) set command='curl ...' and get back JSON with the HTTP method (explicit -X wins, otherwise inferred: POST when a body/form is present, HEAD for -I, else GET), the URL, query parameters split out of the URL (percent-decoded), headers (-H), the effective Content-Type, the request body (-d/--data/--data-ascii with newlines stripped, --data-raw/--data-binary verbatim, --data-urlencode percent-encoded), multipart form fields (-F), cookies (-b, parsed to name/value pairs), HTTP basic auth (-u user:password, decoded), user-agent (-A), referer (-e), and the boolean flags insecure (-k), follow-redirects (-L), compressed, head (-I) and get (-G); @file arguments are reported as file references. In rebuild mode set mode='rebuild' to get a tidy multi-line curl command. A robust shell tokenizer handles single/double quotes, backslash escapes and line continuations. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "curl-command-parser", |a: Args| {
            gizza_ai_curl_command_parser_core::dispatch(&a.mode, &a.command, false)
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
                    "mode": { "type": "string", "enum": ["parse", "rebuild"], "default": "parse", "description": "Direction: 'parse' (default) decodes the curl command into a structured request (JSON); 'rebuild' returns a clean, canonical curl command reconstructed from it." },
                    "command": { "type": "string", "description": "The curl command to process, e.g. \"curl -X POST 'https://api.example.com/v1/items' -H 'Content-Type: application/json' -d '{\\\"name\\\":\\\"gizza\\\"}'\". Quotes, backslash line-continuations, attached short flags (-XPOST) and combined flags (-sSL) are all handled; a leading 'curl' is optional." }
                },
                "required": ["command"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
