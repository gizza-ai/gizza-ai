//! gizza-ai/unified-diff-reverse — chat skill block on the shared tool abstraction.
//!
//! Takes one pasted unified / `git diff` patch and returns the INVERTED patch —
//! the revert of that change, byte-for-byte what `git diff -R` would emit. It
//! never applies anything: applying a patch (forward or reversed) is
//! `apply-patch`, and picking hunks out of one is `diff-hunk-selector`.
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
    file: String,
    #[serde(default)]
    index_lines: String,
    #[serde(default = "default_true")]
    swap_paths: bool,
    #[serde(default)]
    on_binary: String,
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("diff")
                .required()
                .describe("The unified diff / patch text to invert — the output of `git diff`, `git show`, `git format-patch`, `diff -u` or `svn diff`. Commit messages and mail headers around the patch are ignored. Max 1000000 bytes (1 MB)."),
        )
        .param(
            Param::enumv("output", ["patch", "summary", "json"])
                .default("patch")
                .describe("What to return. 'patch' (default) is the reversed patch text itself, ready to save or pipe into `git apply`. 'summary' is a human-readable report — per-file hunk counts and each reversed @@ header next to the original one. 'json' is the same report as data, with the reversed patch under the 'patch' key."),
        )
        .param(
            Param::string("file")
                .default("")
                .describe("Restrict a multi-file patch to one file, matched against the path as it appears after `+++ b/`. An exact path, a path suffix on a '/' boundary, or a bare basename all match. Blank (default) reverses every file in the patch."),
        )
        .param(
            Param::enumv("index_lines", ["swap", "keep", "drop"])
                .default("swap")
                .describe("What to do with git's `index <old>..<new>` blob-hash lines. 'swap' (default) exchanges the two hashes the way `git diff -R` does. 'keep' leaves the line untouched. 'drop' removes it entirely — the safe choice when the hashes no longer exist in the target repository, since `git apply` then cannot try a blob-hash fallback."),
        )
        .param(
            Param::boolean("swap_paths")
                .default(true)
                .describe("When true (default), the `--- a/…` and `+++ b/…` path lines swap sides (and so do the two halves of the `diff --git` line), which is what makes a rename or a new-file patch revert correctly. Set false to keep the original header paths verbatim — only useful when a downstream consumer keys off the forward path."),
        )
        .param(
            Param::enumv("on_binary", ["fail", "skip", "keep"])
                .default("fail")
                .describe("What to do with a `GIT binary patch` / `Binary files … differ` file section, which carries no reverse delta and therefore cannot be inverted. 'fail' (default) refuses and names the file. 'skip' drops those file sections and warns. 'keep' passes them through UNCHANGED and warns — the result will not revert those files."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct UnifiedDiffReverse;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/unified-diff-reverse",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Invert a pasted unified diff into the reverse patch that undoes the change.",
    skill(
        description = "Reverse a pasted unified / git diff: every + line becomes -, every - becomes +, each `@@ -a,b +c,d @@` header becomes `@@ -c,d +a,b @@`, and git's extended headers are inverted the way `git diff -R` inverts them — `new file mode` becomes `deleted file mode`, old/new modes swap, rename/copy from/to swap, the two `index` blob hashes swap, and the `---`/`+++` path lines swap. output='patch' (default) returns the reverse patch text; 'summary' reports per-file hunk counts with each reversed header next to its original; 'json' returns that report plus the patch as data. file restricts a multi-file patch to one path; index_lines chooses swap (default) / keep / drop for `index` lines; swap_paths (default true) controls the path-line swap; on_binary chooses fail (default) / skip / keep for binary sections, which carry no reverse delta. CRLF line endings, the final-newline state and `\\ No newline at end of file` markers are preserved; input cap 1 MB. This emits a patch and applies nothing — use apply-patch to apply one (its reverse flag does the reverse-apply) and diff-hunk-selector to pick hunks. Combined merge diffs with @@@ headers are rejected.",
        parameters = schema_json()
    ),
)]
impl UnifiedDiffReverse {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "unified-diff-reverse", |a: Args| {
            gizza_ai_unified_diff_reverse_core::reverse_diff(
                &a.diff,
                &a.output,
                &a.file,
                &a.index_lines,
                a.swap_paths,
                &a.on_binary,
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
                    "diff": { "type": "string", "description": "The unified diff / patch text to invert — the output of `git diff`, `git show`, `git format-patch`, `diff -u` or `svn diff`. Commit messages and mail headers around the patch are ignored. Max 1000000 bytes (1 MB)." },
                    "output": { "type": "string", "enum": ["patch", "summary", "json"], "default": "patch", "description": "What to return. 'patch' (default) is the reversed patch text itself, ready to save or pipe into `git apply`. 'summary' is a human-readable report — per-file hunk counts and each reversed @@ header next to the original one. 'json' is the same report as data, with the reversed patch under the 'patch' key." },
                    "file": { "type": "string", "default": "", "description": "Restrict a multi-file patch to one file, matched against the path as it appears after `+++ b/`. An exact path, a path suffix on a '/' boundary, or a bare basename all match. Blank (default) reverses every file in the patch." },
                    "index_lines": { "type": "string", "enum": ["swap", "keep", "drop"], "default": "swap", "description": "What to do with git's `index <old>..<new>` blob-hash lines. 'swap' (default) exchanges the two hashes the way `git diff -R` does. 'keep' leaves the line untouched. 'drop' removes it entirely — the safe choice when the hashes no longer exist in the target repository, since `git apply` then cannot try a blob-hash fallback." },
                    "swap_paths": { "type": "boolean", "default": true, "description": "When true (default), the `--- a/…` and `+++ b/…` path lines swap sides (and so do the two halves of the `diff --git` line), which is what makes a rename or a new-file patch revert correctly. Set false to keep the original header paths verbatim — only useful when a downstream consumer keys off the forward path." },
                    "on_binary": { "type": "string", "enum": ["fail", "skip", "keep"], "default": "fail", "description": "What to do with a `GIT binary patch` / `Binary files … differ` file section, which carries no reverse delta and therefore cannot be inverted. 'fail' (default) refuses and names the file. 'skip' drops those file sections and warns. 'keep' passes them through UNCHANGED and warns — the result will not revert those files." }
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
            ["diff", "output", "file", "index_lines", "swap_paths", "on_binary"]
        );
    }

    /// The advertised 1 MB cap in the `diff` description is the core's real cap.
    #[test]
    fn advertised_cap_matches_the_core() {
        assert_eq!(
            gizza_ai_unified_diff_reverse_core::MAX_INPUT_BYTES,
            1_000_000
        );
    }
}
