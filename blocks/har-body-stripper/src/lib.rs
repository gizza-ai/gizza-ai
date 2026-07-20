//! gizza-ai/har-body-stripper — strip request/response bodies out of a HAR
//! capture to shrink and de-sensitize it. Chat schema single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_har_body_stripper_core::strip_bodies;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    har: String,
    #[serde(default = "default_strip")]
    strip: String,
    #[serde(default)]
    only_mime: String,
    #[serde(default)]
    min_bytes: u64,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    pretty: bool,
}

fn default_strip() -> String {
    "both".into()
}
fn default_output() -> String {
    "har".into()
}

/// Single source for the chat schema (and CLI). Removes body payloads only
/// (postData text/params, content text/encoding, websocket frame data) —
/// URLs, headers, cookies, timings, and size metadata all survive, so the
/// stripped capture stays analyzable and diff-able (key order is preserved).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("har")
                .required()
                .describe("The HAR (HTTP Archive) capture as JSON text — the { \"log\": { \"entries\": [ … ] } } object that browser DevTools export via Network tab → \"Save all as HAR\". Max 10000 entries."),
        )
        .param(
            Param::enumv("strip", ["both", "request", "response"])
                .default("both")
                .describe("Which bodies to remove: 'both' (default) request AND response payloads, 'request' only uploaded postData (form fields, JSON payloads), 'response' only downloaded content. Websocket frame data follows the side: sent frames count as request, received as response."),
        )
        .param(
            Param::string("only_mime")
                .default("")
                .describe("Comma-separated case-insensitive mimeType substrings, e.g. image/,font/,video/ — only bodies whose recorded mimeType contains one are stripped. Empty (default) strips every body. Bodies with no recorded mimeType are kept when a filter is set."),
        )
        .param(
            Param::integer("min_bytes")
                .default(0)
                .min(0.0)
                .describe("Only strip bodies at least this many bytes, e.g. 10240 to keep small API JSON but drop big blobs. Response bodies measure content.size when recorded (decoded size), else the stored text length; request bodies measure the stored postData length. Default 0 = strip all."),
        )
        .param(
            Param::enumv("output", ["har", "summary"])
                .default("har")
                .describe("'har' (default) returns the stripped capture as JSON, ready to save as a .har file. 'summary' is a dry-run report instead: entries scanned, bodies stripped per side with bytes removed, and the before → after size."),
        )
        .param(
            Param::boolean("pretty")
                .default(false)
                .describe("Pretty-print the output HAR with 2-space indentation. Default false = compact single-line JSON, which shrinks the file the most (DevTools exports are pretty-printed)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/har-body-stripper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip request/response bodies out of a HAR capture",
    skill(
        description = "Remove the request and response bodies from a HAR (HTTP Archive) capture to drastically shrink it and clear payload data before sharing: request postData (form fields, JSON uploads), response content text, and Chrome websocket frame payloads. URLs, headers, cookies, timings, and size metadata are untouched, so the capture stays analyzable and diff-able (key order preserved). Choose the side with strip=both/request/response, limit by mimeType substrings (only_mime=image/,font/) or size (min_bytes), and get the stripped HAR JSON (compact by default, pretty=true to indent) or a dry-run summary (output=summary) with counts, bytes removed, and before/after size. Note: cookies and auth headers are NOT redacted — this tool only removes bodies. Runs locally; max 10000 entries.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "har-body-stripper", |a: Args| {
            strip_bodies(&a.har, &a.strip, &a.only_mime, a.min_bytes, &a.output, a.pretty)
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
                    "har":       { "type": "string", "description": "The HAR (HTTP Archive) capture as JSON text — the { \"log\": { \"entries\": [ … ] } } object that browser DevTools export via Network tab → \"Save all as HAR\". Max 10000 entries." },
                    "strip":     { "type": "string", "enum": ["both", "request", "response"], "default": "both", "description": "Which bodies to remove: 'both' (default) request AND response payloads, 'request' only uploaded postData (form fields, JSON payloads), 'response' only downloaded content. Websocket frame data follows the side: sent frames count as request, received as response." },
                    "only_mime": { "type": "string", "default": "", "description": "Comma-separated case-insensitive mimeType substrings, e.g. image/,font/,video/ — only bodies whose recorded mimeType contains one are stripped. Empty (default) strips every body. Bodies with no recorded mimeType are kept when a filter is set." },
                    "min_bytes": { "type": "integer", "minimum": 0, "default": 0, "description": "Only strip bodies at least this many bytes, e.g. 10240 to keep small API JSON but drop big blobs. Response bodies measure content.size when recorded (decoded size), else the stored text length; request bodies measure the stored postData length. Default 0 = strip all." },
                    "output":    { "type": "string", "enum": ["har", "summary"], "default": "har", "description": "'har' (default) returns the stripped capture as JSON, ready to save as a .har file. 'summary' is a dry-run report instead: entries scanned, bodies stripped per side with bytes removed, and the before → after size." },
                    "pretty":    { "type": "boolean", "default": false, "description": "Pretty-print the output HAR with 2-space indentation. Default false = compact single-line JSON, which shrinks the file the most (DevTools exports are pretty-printed)." }
                },
                "required": ["har"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let rendered: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(rendered, authored, "descriptor schema drifted from the authored copy");
    }
}
