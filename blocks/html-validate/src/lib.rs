//! gizza-ai/html-validate — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_html_validate_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("html")
                .required()
                .describe("The HTML document or snippet to validate. Paste the full markup; a `<`/`>` inside <script>, <style> or a quoted attribute is handled correctly."),
        )
        .param(
            Param::enumv("format", ["report", "json"])
                .default("report")
                .describe("Output: 'report' (human-readable list of issues with line:column, default) or 'json' (a machine-readable {valid, errors, warnings, elements, issues[]} object)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-validate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate HTML and report syntax errors, unclosed tags, and nesting issues",
    skill(
        description = "Validate an HTML document or snippet and report every syntax error, unclosed tag, and nesting issue, each with a 1-based line and column. Detects unterminated tags and comments, tags with no name, elements opened but never closed, overlapping/misnested tags (e.g. <b><i></b>), and stray closing tags with no matching open. Understands void elements (br, img, hr, …), self-closing tags, quoted attributes, and the raw contents of <script>/<style>/<textarea>/<pre>. Set format='report' (default, human-readable) or 'json' (a machine-readable {valid, errors, warnings, elements, issues} summary). Runs entirely locally — nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-validate", |a: Args| {
            run(&a.html, &a.format).map_err(SkillError::InvalidArgs)
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
                    "html":   { "type": "string", "description": "The HTML document or snippet to validate. Paste the full markup; a `<`/`>` inside <script>, <style> or a quoted attribute is handled correctly." },
                    "format": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output: 'report' (human-readable list of issues with line:column, default) or 'json' (a machine-readable {valid, errors, warnings, elements, issues[]} object)." }
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
