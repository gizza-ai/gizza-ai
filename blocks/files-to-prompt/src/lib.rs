//! gizza-ai/files-to-prompt — bundle pasted files into one LLM-ready digest.
//!
//! Thin chat-skill wrapper around `gizza-ai-files-to-prompt-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    files: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    separator: String,
    #[serde(default)]
    line_numbers: bool,
    /// Prepend a directory tree. Defaults to true (see `default_true`).
    #[serde(default = "default_true")]
    include_tree: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("files")
                .required()
                .multiline()
                .placeholder("=== src/main.rs\nfn main() {}\n\n=== README.md\n# Title")
                .describe(
                    "The files to bundle, concatenated. Start each file with a header line \
                     '<separator> path/to/file.ext' (e.g. '=== src/main.rs'); everything up to \
                     the next header is that file's content. Any text before the first header is \
                     ignored.",
                ),
        )
        .param(
            Param::enumv("format", ["markdown", "xml", "plain"])
                .default("markdown")
                .describe(
                    "Output style. 'markdown' (default): a '## path' heading + a \
                     language-fenced code block per file. 'xml': a single Claude-style \
                     <documents> wrapper with one indexed <document> per file. 'plain': \
                     files-to-prompt's default — 'path', a '---' rule, contents, another '---'.",
                ),
        )
        .param(
            Param::string("separator")
                .default("===")
                .placeholder("===")
                .describe(
                    "The marker that begins each file's header line (default '==='). Change it \
                     (e.g. '>>>') if your file contents contain lines starting with '==='.",
                ),
        )
        .param(
            Param::boolean("line_numbers").default(false).describe(
                "Prefix every content line with its right-aligned line number. Default false.",
            ),
        )
        .param(
            Param::boolean("include_tree")
                .default(true)
                .describe(
                    "Prepend a 'Directory structure:' tree built from the file paths, so the \
                     model sees the layout at a glance. Default true.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FilesToPrompt;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/files-to-prompt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Bundle pasted files into one LLM-ready digest with a directory tree and token estimate.",
    skill(
        description = "Bundle multiple files into a single LLM-ready prompt digest with a directory tree, each file's contents, and a rough token estimate. Paste the files in 'files', each preceded by a header line '<separator> path' (default separator '==='). Choose the output 'format' (markdown fenced blocks, Claude-XML <documents>, or plain files-to-prompt style), toggle 'line_numbers', and toggle the 'include_tree' directory tree. Pure text in, pure text out — no repo crawling or filesystem access.",
        parameters = schema_json()
    )
)]
impl FilesToPrompt {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } and routes
        // errors through GuestResult::error.
        match run_skill(&body, "files-to-prompt", |a: Args| {
            gizza_ai_files_to_prompt_core::build_digest(
                &a.files,
                &a.format,
                &a.separator,
                a.line_numbers,
                a.include_tree,
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
                    "files": { "type": "string", "description": "The files to bundle, concatenated. Start each file with a header line '<separator> path/to/file.ext' (e.g. '=== src/main.rs'); everything up to the next header is that file's content. Any text before the first header is ignored." },
                    "format": { "type": "string", "enum": ["markdown", "xml", "plain"], "default": "markdown", "description": "Output style. 'markdown' (default): a '## path' heading + a language-fenced code block per file. 'xml': a single Claude-style <documents> wrapper with one indexed <document> per file. 'plain': files-to-prompt's default — 'path', a '---' rule, contents, another '---'." },
                    "separator": { "type": "string", "default": "===", "description": "The marker that begins each file's header line (default '==='). Change it (e.g. '>>>') if your file contents contain lines starting with '==='." },
                    "line_numbers": { "type": "boolean", "default": false, "description": "Prefix every content line with its right-aligned line number. Default false." },
                    "include_tree": { "type": "boolean", "default": true, "description": "Prepend a 'Directory structure:' tree built from the file paths, so the model sees the layout at a glance. Default true." }
                },
                "required": ["files"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
