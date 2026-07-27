//! gizza-ai/http-headers-diff — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The first (old/base) HTTP header block.
    left: String,
    /// The second (new/compared) HTTP header block.
    right: String,
    /// Header names to exclude from the diff (comma/space separated).
    #[serde(default)]
    ignore: String,
    /// Compare comma-list values as a set, ignoring token order (default false).
    #[serde(default)]
    ignore_order: bool,
    /// Output form: report | json.
    #[serde(default)]
    output: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("left")
                .required()
                .describe("The first (old/base) set of HTTP headers, one 'Name: value' per line. An optional leading request or status line (e.g. 'GET / HTTP/1.1' or 'HTTP/1.1 200 OK') is skipped, not diffed. Header names are matched case-insensitively; repeated headers are combined (Set-Cookie kept per-line); CRLF, folded continuation lines, and a blank line ending the block are accepted."),
        )
        .param(
            Param::string("right")
                .required()
                .describe("The second (new/compared) set of HTTP headers, in the same format as `left`. Headers only in `right` are reported as added, headers only in `left` as removed, and headers in both with a different value as changed (old -> new)."),
        )
        .param(
            Param::string("ignore")
                .default("")
                .describe("Header names to exclude from the diff, separated by commas, spaces or newlines and matched case-insensitively — e.g. 'Date, Age, Report-To' to drop volatile/noise headers. Default empty (compare all headers)."),
        )
        .param(
            Param::boolean("ignore_order")
                .default(false)
                .describe("For list-valued headers (Vary, Cache-Control, Accept, …), compare the comma-separated tokens as a set so a pure reordering isn't reported as a change. Default false (exact string compare)."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output form: 'report' (default) a grouped human summary with Added/Removed/Changed/Unchanged sections and counts; 'json' a structured object { summary, added, removed, changed, unchanged }."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/http-headers-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Diff two sets of HTTP headers: added, removed and changed header values.",
    skill(
        description = "Compare two sets of HTTP headers and report what changed. Pass the first (old/base) header block in `left` and the second (new/compared) block in `right`, each as one 'Name: value' per line; an optional leading request/status line (GET / HTTP/1.1, HTTP/1.1 200 OK) is skipped. Following HTTP semantics, header names are matched case-insensitively (shown in canonical Title-Case) and repeated headers are combined into one value (Set-Cookie kept per-line, never comma-joined). The tool reports headers added (only in `right`), removed (only in `left`), changed (in both with a different value, shown old -> new), and unchanged, with a count summary. Set `ignore` to a comma/space list of header names to exclude (e.g. volatile Date/Age). Set `ignore_order` to treat comma-list values (Vary, Cache-Control) as a set so reordering isn't flagged. `output` selects 'report' (default, grouped human summary) or 'json' (structured). Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "http-headers-diff", |a: Args| {
            gizza_ai_http_headers_diff_core::diff(
                &a.left,
                &a.right,
                &a.ignore,
                a.ignore_order,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "left": { "type": "string", "description": "The first (old/base) set of HTTP headers, one 'Name: value' per line. An optional leading request or status line (e.g. 'GET / HTTP/1.1' or 'HTTP/1.1 200 OK') is skipped, not diffed. Header names are matched case-insensitively; repeated headers are combined (Set-Cookie kept per-line); CRLF, folded continuation lines, and a blank line ending the block are accepted." },
                    "right": { "type": "string", "description": "The second (new/compared) set of HTTP headers, in the same format as `left`. Headers only in `right` are reported as added, headers only in `left` as removed, and headers in both with a different value as changed (old -> new)." },
                    "ignore": { "type": "string", "default": "", "description": "Header names to exclude from the diff, separated by commas, spaces or newlines and matched case-insensitively — e.g. 'Date, Age, Report-To' to drop volatile/noise headers. Default empty (compare all headers)." },
                    "ignore_order": { "type": "boolean", "default": false, "description": "For list-valued headers (Vary, Cache-Control, Accept, …), compare the comma-separated tokens as a set so a pure reordering isn't reported as a change. Default false (exact string compare)." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output form: 'report' (default) a grouped human summary with Added/Removed/Changed/Unchanged sections and counts; 'json' a structured object { summary, added, removed, changed, unchanged }." }
                },
                "required": ["left", "right"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
