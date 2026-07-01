//! gizza-ai/page-weight-analyzer — parse pasted HTML and report a front-end
//! performance snapshot: resource counts, inline / render-blocking scripts and
//! stylesheets, an estimated request count, and an estimated transfer-weight
//! budget.
//!
//! Thin chat-skill wrapper around `gizza-ai-page-weight-analyzer-core`. The chat
//! schema is single-sourced from `descriptor()` (also drives the CLI + page
//! query-params); `handle()` delegates to `block_utils::run_skill`. No host
//! calls — runs entirely inside the WASM sandbox (no network; external
//! sub-resource sizes are estimated, never fetched).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    output: String,
    /// List every external resource URL grouped by type (default false).
    #[serde(default)]
    list_resources: bool,
}

/// Single source for the chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("html")
                .required()
                .describe("The full HTML source of the page to analyze (paste the document markup, e.g. from View Source)."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output format: 'report' (default) is a human-readable text summary; 'json' is a machine-readable object with all counts and the estimate."),
        )
        .param(
            Param::boolean("list_resources")
                .default(false)
                .describe("When true, also list every external resource URL (scripts, stylesheets, images, fonts, iframes, media) grouped by type. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/page-weight-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parses pasted HTML and reports resource counts, render-blocking scripts/styles, and an estimated page-weight budget.",
    skill(
        description = "Parse pasted HTML and report a front-end performance snapshot. Pass the page's HTML source as 'html'. Returns: resource counts (scripts, stylesheets, images, iframes, audio/video, resource hints); how many external scripts are parser-blocking vs async/defer/module; how many inline scripts run synchronously; how many stylesheets are render-blocking (print-only and disabled sheets excluded); inline JS/CSS byte sizes; an estimated network request count; and an estimated transfer-weight budget. The HTML's own byte size is measured exactly; external sub-resource sizes are ESTIMATED from typical median file sizes (no network is used). Set output='json' for a structured object, or list_resources=true to also list every external resource URL. Use it to spot render-blocking resources, over-large pages, and too many requests.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "page-weight-analyzer", |a: Args| {
            gizza_ai_page_weight_analyzer_core::analyze(&a.html, &a.output, a.list_resources)
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
                    "html": { "type": "string", "description": "The full HTML source of the page to analyze (paste the document markup, e.g. from View Source)." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output format: 'report' (default) is a human-readable text summary; 'json' is a machine-readable object with all counts and the estimate." },
                    "list_resources": { "type": "boolean", "default": false, "description": "When true, also list every external resource URL (scripts, stylesheets, images, fonts, iframes, media) grouped by type. Default false." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
