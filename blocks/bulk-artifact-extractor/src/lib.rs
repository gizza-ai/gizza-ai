//! gizza-ai/bulk-artifact-extractor — chat skill block on the shared tool abstraction.
//!
//! Scans a blob of text for indicators-of-interest — email addresses, URLs, IPv4
//! addresses, bare domains, phone numbers, Bitcoin-like addresses, and
//! Luhn-valid credit-card numbers — and reports each hit with its kind, value,
//! byte offset, and a short context snippet, as a Markdown table or JSON array.
//! The chat schema is single-sourced from `descriptor()` (which also drives the
//! CLI + page); `handle()` delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    kinds: String,
    #[serde(default)]
    output: String,
    /// 0 → the core default (24); the core clamps to 0..=MAX_CONTEXT.
    #[serde(default)]
    context: u32,
    /// 0 → the core default (1000); the core clamps to 1..=MAX_LIMIT.
    #[serde(default)]
    limit: u32,
}

/// Single source for the chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text/blob to scan. Any UTF-8 text — a log slice, an email dump, a document paste. Artifacts are found anywhere in the text, not just line-by-line."),
        )
        .param(
            Param::string("kinds")
                .default("all")
                .describe("Which artifact kinds to report: 'all' (default) or a comma-separated subset of email, url, ipv4, domain, phone, bitcoin, credit_card. Overlaps are always resolved before filtering, so a domain inside an email/URL or an IP inside a URL is never double-reported."),
        )
        .param(
            Param::enumv("output", ["table", "json"])
                .default("table")
                .describe("Output shape. 'table' (default) is a Markdown table with one row per finding (kind, value, offset, context); 'json' is an array of {kind, value, offset, context} objects for piping into a script."),
        )
        .param(
            // Bounds reference the core clamp so the schema can't drift from
            // what `extract` actually enforces.
            Param::integer("context")
                .default(24)
                .min(0.0)
                .max(gizza_ai_bulk_artifact_extractor_core::MAX_CONTEXT as f64)
                .describe("How many characters of surrounding context to show on each side of a finding (0-200). Newlines are flattened and long ends elided with '…'. Default 24."),
        )
        .param(
            Param::integer("limit")
                .default(1000)
                .min(1.0)
                .max(gizza_ai_bulk_artifact_extractor_core::MAX_LIMIT as f64)
                .describe("Maximum number of findings to return (1-20000). Applied after the kind filter, in ascending byte-offset order. Default 1000."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BulkArtifactExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bulk-artifact-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract emails, URLs, IPs, domains, phones, Bitcoin addresses, and Luhn-valid cards with offsets",
    skill(
        description = "Scan a blob of text for common indicators-of-interest and report each hit with its kind, exact value, byte offset, and a short context snippet. Detects email addresses, URLs, IPv4 addresses, bare domains, phone numbers, Bitcoin-like addresses (base58 + bech32), and Luhn-valid credit-card numbers. Overlapping hits are resolved by specificity — a domain inside an email/URL, or an IP inside a URL, is reported once as the more specific kind. kinds filters to a subset (default all); output='table' (default) is a Markdown table or 'json' an array; context sets the snippet width (default 24); limit caps the findings (default 1000). Deterministic, offset-ordered output; runs entirely in the sandbox.",
        parameters = schema_json()
    ),
)]
impl BulkArtifactExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "bulk-artifact-extractor", |a: Args| {
            gizza_ai_bulk_artifact_extractor_core::extract(
                &a.text, &a.kinds, &a.output, a.context, a.limit,
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
                    "text": { "type": "string", "description": "The text/blob to scan. Any UTF-8 text — a log slice, an email dump, a document paste. Artifacts are found anywhere in the text, not just line-by-line." },
                    "kinds": { "type": "string", "default": "all", "description": "Which artifact kinds to report: 'all' (default) or a comma-separated subset of email, url, ipv4, domain, phone, bitcoin, credit_card. Overlaps are always resolved before filtering, so a domain inside an email/URL or an IP inside a URL is never double-reported." },
                    "output": { "type": "string", "enum": ["table", "json"], "default": "table", "description": "Output shape. 'table' (default) is a Markdown table with one row per finding (kind, value, offset, context); 'json' is an array of {kind, value, offset, context} objects for piping into a script." },
                    "context": { "type": "integer", "minimum": 0, "maximum": 200, "default": 24, "description": "How many characters of surrounding context to show on each side of a finding (0-200). Newlines are flattened and long ends elided with '…'. Default 24." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 20000, "default": 1000, "description": "Maximum number of findings to return (1-20000). Applied after the kind filter, in ascending byte-offset order. Default 1000." }
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
