//! gizza-ai/markdown-lint — chat skill block on the shared tool abstraction.
//! Finds (and optionally auto-fixes) common Markdown style/consistency issues.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_lint_core::run as lint_run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    #[serde(default = "default_mode")]
    mode: String,
}

fn default_mode() -> String {
    "check".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The Markdown text to lint."),
        )
        .param(
            Param::enumv("mode", ["check", "fix"]).default("check").describe(
                "check = report every style/consistency issue (line:col rule message); \
                 fix = return the auto-fixed Markdown.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MarkdownLint;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-lint",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Lint and auto-fix Markdown style and consistency issues",
    skill(
        description = "Lint Markdown for common style and consistency issues and optionally auto-fix them. Checks: heading levels increment by one / no skipped levels (MD001), inconsistent unordered list markers (MD004), trailing whitespace (MD009), hard tabs (MD010), multiple consecutive blank lines (MD012), missing space after ATX heading hashes (MD018), multiple spaces after heading hashes (MD019), heading not preceded by a blank line (MD022), multiple top-level H1 headings (MD025), trailing punctuation in headings (MD026), fenced code block missing a language (MD040), and missing final newline (MD047). Fenced code blocks are respected so prose rules don't fire on code. mode='check' returns a report of every issue (line:col rule message); mode='fix' returns the corrected Markdown. Runs locally.",
        parameters = schema_json()
    ),
)]
impl MarkdownLint {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-lint", |a: Args| {
            lint_run(&a.markdown, &a.mode).map_err(SkillError::InvalidArgs)
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
                    "markdown": { "type": "string", "description": "The Markdown text to lint." },
                    "mode": {
                        "type": "string",
                        "enum": ["check", "fix"],
                        "default": "check",
                        "description": "check = report every style/consistency issue (line:col rule message); fix = return the auto-fixed Markdown."
                    }
                },
                "required": ["markdown"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
