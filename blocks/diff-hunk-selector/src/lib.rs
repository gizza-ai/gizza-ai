//! gizza-ai/diff-hunk-selector — chat skill block on the shared tool abstraction.
//!
//! Numbers every hunk in a pasted unified / `git diff` patch (globally, 1-based)
//! and lists them, filters them into a smaller patch that still applies, splits
//! them into one standalone patch per hunk, or reports the inventory as JSON.
//! The chat schema is single-sourced from `descriptor()` (which also drives the
//! CLI + page); `handle()` delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    diff: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    hunks: String,
    #[serde(default)]
    invert: bool,
    #[serde(default)]
    files: String,
    #[serde(default)]
    lines: String,
    #[serde(default = "default_true")]
    renumber: bool,
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("diff")
                .required()
                .describe("The unified diff / patch text — the output of `git diff`, `git show`, `git format-patch`, or `diff -u`. Hunks are numbered globally from 1 in the order they appear. Max 1000000 bytes (1 MB)."),
        )
        .param(
            Param::enumv("output", ["list", "patch", "split", "json"])
                .default("list")
                .describe("What to return. 'list' (default) is the numbered hunk inventory — file path, @@ header and +/- counts per hunk, plus per-file totals. 'patch' emits the selected hunks as one smaller, still-applicable patch with the file headers kept. 'split' emits one complete standalone patch per selected hunk under a labelled separator. 'json' is the machine-readable inventory plus the current selection."),
        )
        .param(
            Param::string("hunks")
                .default("all")
                .describe("Which hunk numbers to select, using the numbers shown by output=list. 'all' (default) selects every hunk; otherwise a comma list of numbers and ranges — '2', '1,3-5', open-ended '4-' (hunk 4 to the end) or '-2' (up to hunk 2)."),
        )
        .param(
            Param::boolean("invert")
                .default(false)
                .describe("When true, keep the hunks the 'hunks' selection does NOT name — e.g. hunks='2' invert=true drops hunk 2 and keeps the rest. Default false. The 'files' and 'lines' filters are not inverted."),
        )
        .param(
            Param::string("files")
                .default("")
                .describe("Comma-separated file globs to restrict the selection to, e.g. 'src/*.rs, *.toml'. A '!' prefix excludes ('!*.lock'), and an exclude always wins. '*' matches path separators too; a pattern with no '/' also matches the file's basename. Blank (default) keeps every file."),
        )
        .param(
            Param::string("lines")
                .default("")
                .describe("Keep only hunks touching these ORIGINAL-file line numbers, using the same span grammar as 'hunks' — '40-120', '200-', '-50', or a comma list. Blank (default) applies no line filter."),
        )
        .param(
            Param::boolean("renumber")
                .default(true)
                .describe("When true (default), the new-side start of each kept hunk is shifted back by the net line delta of the dropped hunks before it in the same file, so the emitted patch applies cleanly to the original file. Set false to keep the original @@ headers verbatim (useful when the dropped hunks will be applied separately)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DiffHunkSelector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/diff-hunk-selector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "List, filter, and split the hunks of a unified diff into a smaller patch that still applies.",
    skill(
        description = "Pick hunks out of a pasted unified / git diff. output='list' (default) returns the globally numbered hunk inventory — file, @@ header, +/- counts and per-file totals — so you can see what is in a patch. Then select with hunks ('all', '2', '1,3-5', '4-', '-2'), optionally inverted with invert=true, narrowed to files with comma-separated globs ('src/*.rs', '!*.lock' excludes) or to hunks touching original-file lines with lines ('40-120'). output='patch' emits the selection as one smaller patch with the file headers preserved; output='split' emits one standalone patch per selected hunk; output='json' returns the inventory and selection as data. renumber (default true) shifts kept hunks' new-side starts by the dropped hunks' net delta so the result still applies. Input cap 1 MB; binary/rename-only file entries are listed but carry no hunks.",
        parameters = schema_json()
    ),
)]
impl DiffHunkSelector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "diff-hunk-selector", |a: Args| {
            gizza_ai_diff_hunk_selector_core::select_hunks(
                &a.diff, &a.output, &a.hunks, a.invert, &a.files, &a.lines, a.renumber,
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
                    "diff": { "type": "string", "description": "The unified diff / patch text — the output of `git diff`, `git show`, `git format-patch`, or `diff -u`. Hunks are numbered globally from 1 in the order they appear. Max 1000000 bytes (1 MB)." },
                    "output": { "type": "string", "enum": ["list", "patch", "split", "json"], "default": "list", "description": "What to return. 'list' (default) is the numbered hunk inventory — file path, @@ header and +/- counts per hunk, plus per-file totals. 'patch' emits the selected hunks as one smaller, still-applicable patch with the file headers kept. 'split' emits one complete standalone patch per selected hunk under a labelled separator. 'json' is the machine-readable inventory plus the current selection." },
                    "hunks": { "type": "string", "default": "all", "description": "Which hunk numbers to select, using the numbers shown by output=list. 'all' (default) selects every hunk; otherwise a comma list of numbers and ranges — '2', '1,3-5', open-ended '4-' (hunk 4 to the end) or '-2' (up to hunk 2)." },
                    "invert": { "type": "boolean", "default": false, "description": "When true, keep the hunks the 'hunks' selection does NOT name — e.g. hunks='2' invert=true drops hunk 2 and keeps the rest. Default false. The 'files' and 'lines' filters are not inverted." },
                    "files": { "type": "string", "default": "", "description": "Comma-separated file globs to restrict the selection to, e.g. 'src/*.rs, *.toml'. A '!' prefix excludes ('!*.lock'), and an exclude always wins. '*' matches path separators too; a pattern with no '/' also matches the file's basename. Blank (default) keeps every file." },
                    "lines": { "type": "string", "default": "", "description": "Keep only hunks touching these ORIGINAL-file line numbers, using the same span grammar as 'hunks' — '40-120', '200-', '-50', or a comma list. Blank (default) applies no line filter." },
                    "renumber": { "type": "boolean", "default": true, "description": "When true (default), the new-side start of each kept hunk is shifted back by the net line delta of the dropped hunks before it in the same file, so the emitted patch applies cleanly to the original file. Set false to keep the original @@ headers verbatim (useful when the dropped hunks will be applied separately)." }
                },
                "required": ["diff"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The page passes fields positionally to the wasm export, so the page form's
    /// [[input]] order must stay in lockstep with the descriptor's param order.
    #[test]
    fn descriptor_param_order_matches_the_page_form() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let names: Vec<&str> = derived["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        assert_eq!(
            names,
            ["diff", "output", "hunks", "invert", "files", "lines", "renumber"]
        );
    }

    /// The advertised 1 MB cap in the `diff` description is the core's real cap.
    #[test]
    fn advertised_cap_matches_the_core() {
        assert_eq!(
            gizza_ai_diff_hunk_selector_core::MAX_INPUT_BYTES,
            1_000_000
        );
    }
}
