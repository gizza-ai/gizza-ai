//! gizza-ai/http-header-parser — parse a raw HTTP request or response header
//! block into a structured, case-normalized map. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_http_header_parser_core::run as parse_headers;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    headers: String,
    #[serde(default = "default_case")]
    case: String,
    #[serde(default = "default_duplicates")]
    duplicates: String,
}

fn default_case() -> String {
    "canonical".to_string()
}
fn default_duplicates() -> String {
    "combine".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("headers")
                .required()
                .describe("The raw HTTP header block to parse — one 'Name: value' per line. A leading request line (GET /p HTTP/1.1) or status line (HTTP/1.1 200 OK) is optional and captured separately. CRLF or bare-LF line endings and obsolete folded continuation lines are accepted; parsing stops at the first blank line."),
        )
        .param(
            Param::enumv("case", ["canonical", "lower", "upper", "original"])
                .default("canonical")
                .describe("How to re-case header names in the map (names are always folded case-insensitively): canonical HTTP Title-Case like Content-Type (default), lower, upper, or original (keep the first-seen casing)."),
        )
        .param(
            Param::enumv("duplicates", ["combine", "list", "first", "last"])
                .default("combine")
                .describe("How to fold repeated headers: combine joins values with ', ' per RFC 7230 while keeping Set-Cookie as a list (default); list keeps every occurrence as a JSON array; first or last keeps only that occurrence."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/http-header-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a raw HTTP header block into a case-normalized map",
    skill(
        description = "Parse a raw HTTP request or response header block into a structured, case-normalized JSON map of name → value. Accepts a bare header block or one led by a request/status line (captured separately). Header names are folded case-insensitively and re-cased via `case`: canonical Title-Case like Content-Type (default), lower, upper, or original. Repeated headers are folded via `duplicates`: combine joins values with ', ' per RFC 7230 while keeping Set-Cookie as a list (default), list keeps every occurrence as an array, or first/last keeps one. Tolerates CRLF or bare-LF endings and obsolete folded continuation lines, stops at the first blank line, and reports the detected kind, header count, line count, and which names were duplicated. Returns JSON. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "http-header-parser", |a: Args| {
            parse_headers(&a.headers, &a.case, &a.duplicates).map_err(SkillError::InvalidArgs)
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
                    "headers": { "type": "string", "description": "The raw HTTP header block to parse — one 'Name: value' per line. A leading request line (GET /p HTTP/1.1) or status line (HTTP/1.1 200 OK) is optional and captured separately. CRLF or bare-LF line endings and obsolete folded continuation lines are accepted; parsing stops at the first blank line." },
                    "case": { "type": "string", "enum": ["canonical", "lower", "upper", "original"], "default": "canonical", "description": "How to re-case header names in the map (names are always folded case-insensitively): canonical HTTP Title-Case like Content-Type (default), lower, upper, or original (keep the first-seen casing)." },
                    "duplicates": { "type": "string", "enum": ["combine", "list", "first", "last"], "default": "combine", "description": "How to fold repeated headers: combine joins values with ', ' per RFC 7230 while keeping Set-Cookie as a list (default); list keeps every occurrence as a JSON array; first or last keeps only that occurrence." }
                },
                "required": ["headers"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
