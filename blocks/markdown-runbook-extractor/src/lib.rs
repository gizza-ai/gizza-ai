//! gizza-ai/markdown-runbook-extractor — chat skill block on the shared tool abstraction.
//! Turns a Markdown runbook (a how-to doc whose steps are fenced code blocks)
//! into ONE runnable script plus an ordered task list. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_runbook_extractor_core::{extract, Language, Options, OutputFormat};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    #[serde(default = "default_auto")]
    language: String,
    #[serde(default = "default_script")]
    output: String,
    #[serde(default)]
    tags: String,
    #[serde(default = "default_true")]
    strip_prompts: bool,
    #[serde(default = "default_true")]
    echo_steps: bool,
    #[serde(default = "default_true")]
    fail_fast: bool,
    #[serde(default = "default_true")]
    skip_marked: bool,
}
fn default_auto() -> String {
    "auto".to_string()
}
fn default_script() -> String {
    "script".to_string()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The Markdown runbook to extract from — paste the whole document. Steps are its fenced code blocks (```bash …```)."),
        )
        .param(
            Param::enumv(
                "language",
                ["auto", "shell", "python", "powershell", "javascript", "any"],
            )
            .default("auto")
            .describe("Which fenced blocks are steps, and which script to emit: 'auto' (default) picks the family with the most blocks; 'shell' matches bash/sh/zsh/console/shell-session; 'python' matches python/py/pycon; 'powershell' matches powershell/pwsh/ps1; 'javascript' matches js/node; 'any' takes every fence that has a language tag (untagged fences are never steps)."),
        )
        .param(
            Param::enumv("output", ["script", "tasks", "json"])
                .default("script")
                .describe("What to return: 'script' (default) — one runnable script whose header comment lists the ordered tasks; 'tasks' — a Markdown checklist of the steps with language, line count and source line; 'json' — {language, count, runnable, steps:[{index,name,language,line,tags,skipped,skip_reason,lines,code}]}."),
        )
        .param(
            Param::string("tags")
                .default("")
                .describe("Optional comma-separated tag filter over the fence info string (```bash#deploy, ```bash deploy slow, ```{.bash .deploy}). A block is kept if it carries ANY listed tag; prefix a tag with '-' to exclude it (e.g. 'deploy,-slow'). Empty (default) keeps every block."),
        )
        .param(
            Param::boolean("strip_prompts")
                .default(true)
                .describe("Turn a pasted terminal session into runnable commands: strip leading prompts ('$ ', '% ', 'PS C:\\>', '>>> ') and drop the un-prompted lines, which are command output. Blocks with no prompt at all are kept verbatim. Default true."),
        )
        .param(
            Param::boolean("echo_steps")
                .default(true)
                .describe("Emit a progress line before each step in script output (echo / print / Write-Host of '==> [i/n] name'), so a long run says where it is. Default true."),
        )
        .param(
            Param::boolean("fail_fast")
                .default(true)
                .describe("Add an abort-on-error header to script output: 'set -euo pipefail' for shell, \"$ErrorActionPreference = 'Stop'\" for PowerShell. No-op for Python/JavaScript, which already abort on an unhandled error. Default true."),
        )
        .param(
            Param::boolean("skip_marked")
                .default(true)
                .describe("Treat blocks tagged skip, no-run, norun, noexec, dont-run, ignore, example or output as documentation: they stay in the task list and appear commented out in the script instead of running. Set false to make them runnable. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-runbook-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a Markdown runbook's named code blocks into one runnable script plus an ordered task list",
    skill(
        description = "Extract the named, executable code blocks of a Markdown runbook into a single runnable script plus an ordered task list. Each fenced block becomes a numbered step; its name comes from the fence info string (```bash \"Install deps\", name=/title=/id=, or a {.bash #id} attribute), else a bold label or the nearest heading above it, else step-N. language selects the family (auto/shell/python/powershell/javascript/any). output='script' (default) returns a runnable script whose header lists the tasks; 'tasks' returns a Markdown checklist; 'json' returns structured steps with source line numbers. strip_prompts turns pasted terminal sessions into commands (drops '$ ' prompts and output lines); tags filters blocks by info-string tag ('deploy,-slow'); skip_marked comments out blocks tagged skip/no-run/example; echo_steps adds progress lines; fail_fast adds 'set -euo pipefail'. Returns the chosen text. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-runbook-extractor", |a: Args| {
            let opts = Options {
                language: Language::parse(&a.language).map_err(SkillError::InvalidArgs)?,
                output: OutputFormat::parse(&a.output).map_err(SkillError::InvalidArgs)?,
                tags: a.tags,
                strip_prompts: a.strip_prompts,
                echo_steps: a.echo_steps,
                fail_fast: a.fail_fast,
                skip_marked: a.skip_marked,
            };
            extract(&a.markdown, &opts).map_err(SkillError::InvalidArgs)
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
                    "markdown": { "type": "string", "description": "The Markdown runbook to extract from — paste the whole document. Steps are its fenced code blocks (```bash …```)." },
                    "language": { "type": "string", "enum": ["auto", "shell", "python", "powershell", "javascript", "any"], "default": "auto", "description": "Which fenced blocks are steps, and which script to emit: 'auto' (default) picks the family with the most blocks; 'shell' matches bash/sh/zsh/console/shell-session; 'python' matches python/py/pycon; 'powershell' matches powershell/pwsh/ps1; 'javascript' matches js/node; 'any' takes every fence that has a language tag (untagged fences are never steps)." },
                    "output": { "type": "string", "enum": ["script", "tasks", "json"], "default": "script", "description": "What to return: 'script' (default) — one runnable script whose header comment lists the ordered tasks; 'tasks' — a Markdown checklist of the steps with language, line count and source line; 'json' — {language, count, runnable, steps:[{index,name,language,line,tags,skipped,skip_reason,lines,code}]}." },
                    "tags": { "type": "string", "default": "", "description": "Optional comma-separated tag filter over the fence info string (```bash#deploy, ```bash deploy slow, ```{.bash .deploy}). A block is kept if it carries ANY listed tag; prefix a tag with '-' to exclude it (e.g. 'deploy,-slow'). Empty (default) keeps every block." },
                    "strip_prompts": { "type": "boolean", "default": true, "description": "Turn a pasted terminal session into runnable commands: strip leading prompts ('$ ', '% ', 'PS C:\\>', '>>> ') and drop the un-prompted lines, which are command output. Blocks with no prompt at all are kept verbatim. Default true." },
                    "echo_steps": { "type": "boolean", "default": true, "description": "Emit a progress line before each step in script output (echo / print / Write-Host of '==> [i/n] name'), so a long run says where it is. Default true." },
                    "fail_fast": { "type": "boolean", "default": true, "description": "Add an abort-on-error header to script output: 'set -euo pipefail' for shell, \"$ErrorActionPreference = 'Stop'\" for PowerShell. No-op for Python/JavaScript, which already abort on an unhandled error. Default true." },
                    "skip_marked": { "type": "boolean", "default": true, "description": "Treat blocks tagged skip, no-run, norun, noexec, dont-run, ignore, example or output as documentation: they stay in the task list and appear commented out in the script instead of running. Set false to make them runnable. Default true." }
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
