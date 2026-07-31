//! gizza-ai/diff-viewer — render a pasted **unified diff** as a readable view.
//!
//! Input is an already-computed unified diff (the output of `git diff`,
//! `diff -u`, or a `.patch` file) — NOT two texts to compare (that is the
//! `text-diff` tool). The block parses the patch into files → hunks → lines and
//! re-renders it as a clean `inline` diff, a text `side-by-side` (old | new)
//! layout, a `git --stat`-style `stats` summary, or a structured `json` report.
//! An optional `ignore_whitespace` flag folds whitespace-only changes into
//! context. The chat schema is single-sourced from `descriptor()` (shared with
//! the CLI); `handle()` delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

/// Default view when the caller omits `view`. Mirrors the descriptor default so
/// chat/CLI/page all behave the same when it is unset.
const DEFAULT_VIEW: &str = "inline";

#[derive(Deserialize)]
struct Args {
    diff: String,
    #[serde(default = "default_view")]
    view: String,
    #[serde(default)]
    ignore_whitespace: bool,
}

fn default_view() -> String {
    DEFAULT_VIEW.to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("diff")
                .required()
                .describe("The unified diff to render — paste the output of `git diff`, `diff -u`, or a `.patch` file. Handles multi-file patches, new/deleted/renamed/binary files, and hunk section headings. This is a viewer for an already-computed diff, not a comparator of two texts."),
        )
        .param(
            Param::enumv("view", ["inline", "side-by-side", "stats", "json"])
                .default("inline")
                .describe("Output layout. 'inline' (default) is a clean unified diff with a change summary; 'side-by-side' lays the old and new text in two columns; 'stats' is a `git diff --stat`-style per-file bar graph with totals; 'json' is a structured report (files, hunks, per-line numbers) for programmatic use."),
        )
        .param(
            Param::boolean("ignore_whitespace")
                .default(false)
                .describe("Fold whitespace-only changes into unchanged context so they stop counting toward the add/remove totals. A deleted line paired with an added line that is identical after whitespace normalization becomes a single context line. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/diff-viewer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a pasted unified diff as a clean inline, side-by-side, or stats view.",
    skill(
        description = "Render a pasted unified diff as a readable view. Input is an already-computed unified diff — the output of `git diff`, `diff -u`, or a `.patch` file (this is a viewer, not a text comparator; use text-diff to compare two texts). Pass the patch in 'diff'. view='inline' (default) returns a clean unified diff with a `N files changed, A insertions(+), D deletions(-)` summary; view='side-by-side' lays old and new text in two columns; view='stats' returns a `git diff --stat`-style per-file bar graph plus totals; view='json' returns a structured report (files → hunks → lines, each with old/new line numbers). Handles multi-file patches, new/deleted/renamed/binary files, and hunk section headings; ignore_whitespace=true folds whitespace-only changes into context. Errors when the input contains no recognizable diff.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "diff-viewer", |a: Args| {
            gizza_ai_diff_viewer_core::run(&a.diff, &a.view, a.ignore_whitespace)
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
                    "diff": { "type": "string", "description": "The unified diff to render — paste the output of `git diff`, `diff -u`, or a `.patch` file. Handles multi-file patches, new/deleted/renamed/binary files, and hunk section headings. This is a viewer for an already-computed diff, not a comparator of two texts." },
                    "view": { "type": "string", "enum": ["inline", "side-by-side", "stats", "json"], "default": "inline", "description": "Output layout. 'inline' (default) is a clean unified diff with a change summary; 'side-by-side' lays the old and new text in two columns; 'stats' is a `git diff --stat`-style per-file bar graph with totals; 'json' is a structured report (files, hunks, per-line numbers) for programmatic use." },
                    "ignore_whitespace": { "type": "boolean", "default": false, "description": "Fold whitespace-only changes into unchanged context so they stop counting toward the add/remove totals. A deleted line paired with an added line that is identical after whitespace normalization becomes a single context line. Default false." }
                },
                "required": ["diff"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
