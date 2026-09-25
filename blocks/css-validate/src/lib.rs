//! gizza-ai/css-validate — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use serde_json::Value;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    css: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    unknown_properties: String,
    #[serde(default)]
    vendor_prefixes: String,
    #[serde(default)]
    stats: Value,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("css")
                .required()
                .describe("CSS stylesheet or snippet to validate. Paste rules such as `.card { color: red; }`; comments, strings, selectors, at-rules, declarations, custom properties, and calc()/var() expressions are scanned locally."),
        )
        .param(
            Param::enumv("format", ["report", "json"])
                .default("report")
                .describe("Output format: `report` for a human-readable issue list with line and column, or `json` for machine-readable validity, counts, issues, and stats."),
        )
        .param(
            Param::enumv("severity", ["all", "error", "warning"])
                .default("all")
                .describe("Filter displayed issues by severity while preserving the true validity summary: `all`, `error`, or `warning`."),
        )
        .param(
            Param::enumv("unknown_properties", ["warn", "error", "ignore"])
                .default("warn")
                .describe("How to handle properties not in the curated modern CSS property list: warn by default, fail as errors, or ignore."),
        )
        .param(
            Param::enumv("vendor_prefixes", ["ignore", "warn", "error"])
                .default("ignore")
                .describe("How to handle vendor-prefixed properties such as `-webkit-transform`: ignore, warn, or treat as errors."),
        )
        .param(
            Param::boolean("stats")
                .default(true)
                .describe("Include rule/declaration counts, unique property count, custom-property count, at-rule count, and the most frequent properties."),
        )
}
#[cfg(not(target_arch = "wasm32"))]
fn schema_json() -> String {
    descriptor().to_schema_json()
}

// Wafer calls this from the block's `__wafer_info` export while booting the
// wasm artifact. Keep the wasm path allocation-light and fully static; the
// native drift test below proves it matches the descriptor-derived schema.
#[cfg(target_arch = "wasm32")]
fn schema_json() -> String {
    r#"{"type":"object","properties":{"css":{"type":"string","description":"CSS stylesheet or snippet to validate. Paste rules such as `.card { color: red; }`; comments, strings, selectors, at-rules, declarations, custom properties, and calc()/var() expressions are scanned locally."},"format":{"type":"string","enum":["report","json"],"default":"report","description":"Output format: `report` for a human-readable issue list with line and column, or `json` for machine-readable validity, counts, issues, and stats."},"severity":{"type":"string","enum":["all","error","warning"],"default":"all","description":"Filter displayed issues by severity while preserving the true validity summary: `all`, `error`, or `warning`."},"unknown_properties":{"type":"string","enum":["warn","error","ignore"],"default":"warn","description":"How to handle properties not in the curated modern CSS property list: warn by default, fail as errors, or ignore."},"vendor_prefixes":{"type":"string","enum":["ignore","warn","error"],"default":"ignore","description":"How to handle vendor-prefixed properties such as `-webkit-transform`: ignore, warn, or treat as errors."},"stats":{"type":"boolean","default":true,"description":"Include rule/declaration counts, unique property count, custom-property count, at-rule count, and the most frequent properties."}},"required":["css"],"additionalProperties":false}"#.to_string()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/css-validate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate CSS syntax and flag malformed rules, unknown properties, and warnings",
    skill(
        description = "Validate a CSS stylesheet or snippet and report malformed rules, unbalanced braces/comments/strings, selector mistakes, declaration syntax errors, unknown properties, vendor prefixes, malformed colors/calc()/var() usage, and summary stats. Every issue includes 1-based line and column. Set format='report' for readable output or format='json' for machine-readable results. Runs locally with no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "css-validate", |a: Args| {
            let stats = match a.stats {
                Value::Null => "true".to_string(),
                Value::Bool(v) => v.to_string(),
                Value::String(v) => v,
                other => other.to_string(),
            };
            gizza_ai_css_validate_core::run(
                &a.css,
                &a.format,
                &a.severity,
                &a.unknown_properties,
                &a.vendor_prefixes,
                &stats,
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "css": { "type": "string", "description": "CSS stylesheet or snippet to validate. Paste rules such as `.card { color: red; }`; comments, strings, selectors, at-rules, declarations, custom properties, and calc()/var() expressions are scanned locally." },
                    "format": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output format: `report` for a human-readable issue list with line and column, or `json` for machine-readable validity, counts, issues, and stats." },
                    "severity": { "type": "string", "enum": ["all", "error", "warning"], "default": "all", "description": "Filter displayed issues by severity while preserving the true validity summary: `all`, `error`, or `warning`." },
                    "unknown_properties": { "type": "string", "enum": ["warn", "error", "ignore"], "default": "warn", "description": "How to handle properties not in the curated modern CSS property list: warn by default, fail as errors, or ignore." },
                    "vendor_prefixes": { "type": "string", "enum": ["ignore", "warn", "error"], "default": "ignore", "description": "How to handle vendor-prefixed properties such as `-webkit-transform`: ignore, warn, or treat as errors." },
                    "stats": { "type": "boolean", "default": true, "description": "Include rule/declaration counts, unique property count, custom-property count, at-rule count, and the most frequent properties." }
                },
                "required": ["css"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
