//! gizza-ai/diff-extract-versions — chat skill block on the shared tool abstraction.
//! Reconstructs the before-text and after-text described by a pasted unified
//! diff, without needing the original file. The chat schema is single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    diff: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    file: String,
    #[serde(default = "default_gaps")]
    gaps: String,
    #[serde(default)]
    line_numbers: bool,
}

fn default_output() -> String {
    "both".to_string()
}
fn default_gaps() -> String {
    "marker".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("diff")
                .required()
                .describe("The unified diff to reconstruct from: the output of `git diff`, `git show`, `git format-patch`, `diff -u`, or a pasted .patch file. Mail preambles, `index`/mode lines and signature blocks are ignored. Context (`diff -c`) and normal `diff` output are not unified diffs and are rejected. Max 1 MB."),
        )
        .param(
            Param::enumv("output", ["both", "before", "after", "json"])
                .default("both")
                .describe("What to return. both (default) prints labelled BEFORE and AFTER sections; before prints only the original text; after prints only the patched text; json returns a report with both texts plus paths, status, hunk/added/removed counts, completeness and the missing line ranges."),
        )
        .param(
            Param::string("file")
                .default("")
                .describe("Which path to reconstruct from a multi-file diff. Accepts an exact path, a bare filename, a substring, or a `*`/`?` glob such as `src/*.rs`. Leave empty to reconstruct every file in the diff."),
        )
        .param(
            Param::enumv("gaps", ["marker", "omit", "error"])
                .default("marker")
                .describe("What to do about line ranges the diff never carried (a 3-line-context diff omits most of the file). marker (default) inserts a counted `[... N lines not in the diff ...]` placeholder; omit splices the hunks together; error refuses to return a partial reconstruction."),
        )
        .param(
            Param::boolean("line_numbers")
                .default(false)
                .describe("When true, prefix every emitted line with its line number in that version, so gaps and hunk positions line up with the real file. Default false, which returns copy-paste-ready text."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/diff-extract-versions",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rebuild the before and after file versions from a unified diff",
    skill(
        description = "Reconstruct the original (before) and patched (after) file text from a unified diff alone — no original file needed. Paste the output of `git diff`, `git show`, `git format-patch`, `diff -u`, or a .patch file. output=both (default) returns labelled BEFORE/AFTER sections, before/after returns one side as plain text, and json returns a report with both texts plus paths, status (modified/added/deleted/renamed/binary), hunk and added/removed counts, completeness and the missing line ranges. file picks one path out of a multi-file diff (exact path, bare filename, substring, or a * / ? glob). Because a diff only carries the context lines around each hunk, anything between hunks or after the last hunk is unknowable: gaps=marker (default) marks those ranges with a counted placeholder, omit splices hunks together, error refuses a partial result. Wrong `@@` counts are recounted, `\\ No newline at end of file` is honoured per side, and CRLF survives. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "diff-extract-versions", |a: Args| {
            gizza_ai_diff_extract_versions_core::extract_versions(
                &a.diff,
                &a.output,
                &a.file,
                &a.gaps,
                a.line_numbers,
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
            r##"{
                "type": "object",
                "properties": {
                    "diff": { "type": "string", "description": "The unified diff to reconstruct from: the output of `git diff`, `git show`, `git format-patch`, `diff -u`, or a pasted .patch file. Mail preambles, `index`/mode lines and signature blocks are ignored. Context (`diff -c`) and normal `diff` output are not unified diffs and are rejected. Max 1 MB." },
                    "output": { "type": "string", "enum": ["both", "before", "after", "json"], "default": "both", "description": "What to return. both (default) prints labelled BEFORE and AFTER sections; before prints only the original text; after prints only the patched text; json returns a report with both texts plus paths, status, hunk/added/removed counts, completeness and the missing line ranges." },
                    "file": { "type": "string", "default": "", "description": "Which path to reconstruct from a multi-file diff. Accepts an exact path, a bare filename, a substring, or a `*`/`?` glob such as `src/*.rs`. Leave empty to reconstruct every file in the diff." },
                    "gaps": { "type": "string", "enum": ["marker", "omit", "error"], "default": "marker", "description": "What to do about line ranges the diff never carried (a 3-line-context diff omits most of the file). marker (default) inserts a counted `[... N lines not in the diff ...]` placeholder; omit splices the hunks together; error refuses to return a partial reconstruction." },
                    "line_numbers": { "type": "boolean", "default": false, "description": "When true, prefix every emitted line with its line number in that version, so gaps and hunk positions line up with the real file. Default false, which returns copy-paste-ready text." }
                },
                "required": ["diff"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
