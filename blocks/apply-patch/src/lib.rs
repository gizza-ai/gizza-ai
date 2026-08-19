//! gizza-ai/apply-patch — chat skill block on the shared tool abstraction.
//!
//! Applies a pasted unified diff to a pasted source file and returns the patched
//! text, with `patch(1)`-style offset search, a fuzz factor, whitespace-insensitive
//! matching, reverse (unapply) mode, and first-class conflict handling. The chat
//! schema is single-sourced from `descriptor()` (which also drives the CLI + page);
//! `handle()` delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_fuzz() -> f64 {
    2.0
}

#[derive(Deserialize)]
struct Args {
    source: String,
    patch: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    reverse: bool,
    #[serde(default = "default_fuzz")]
    fuzz: f64,
    #[serde(default)]
    ignore_whitespace: bool,
    #[serde(default)]
    on_conflict: String,
    #[serde(default)]
    file: String,
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("source")
                .required()
                .describe("The original file text the patch applies to — paste the file as it is right now, before the change. Its line endings (LF or CRLF) and final-newline state are preserved in the result. Max 1000000 bytes (1 MB). Pass an empty string when the patch creates a new file."),
        )
        .param(
            Param::string("patch")
                .required()
                .describe("The unified diff to apply — the output of `git diff`, `git show`, `git format-patch`, `diff -u`, or `svn diff`, including its `@@` hunk headers. `---`/`+++`/`diff --git` file headers are optional; wrong `@@` line counts are recounted from the body. Max 1000000 bytes (1 MB)."),
        )
        .param(
            Param::enumv("output", ["patched", "report", "json", "rejects"])
                .default("patched")
                .describe("What to return. 'patched' (default) is the patched file text. 'report' is a dry run like `git apply --check`: one line per hunk with the line it landed on, the offset from its header, the fuzz used, or why it failed. 'json' is the same data as machine-readable fields plus the patched text. 'rejects' returns only the hunks that did NOT apply, as a standalone patch you can fix by hand. 'report' and 'json' never fail on a conflict — they describe it."),
        )
        .param(
            Param::boolean("reverse")
                .default(false)
                .describe("Apply the patch backwards, like `git apply -R` / `patch -R`: '+' and '-' swap roles so the change is undone. Use this to recover the original file from a patched one, or to revert a commit's diff. Default false."),
        )
        .param(
            Param::integer("fuzz")
                .default(2)
                .min(0.0)
                .max(3.0)
                .describe("How many CONTEXT lines may be dropped from each end of a hunk when an exact match fails — the `patch -F` fuzz factor. 0 requires every context line to match; 2 (default, same as `patch(1)`) tolerates a couple of drifted surrounding lines; 3 is the maximum. Fuzz never drops a '+' or '-' line and never reduces a hunk to zero context, so it cannot turn a real conflict into a blind insertion."),
        )
        .param(
            Param::boolean("ignore_whitespace")
                .default(false)
                .describe("Match context and deleted lines with runs of whitespace collapsed and the ends trimmed, like `git apply --ignore-whitespace` — use it when the source was reindented (tabs vs spaces) since the patch was made. Matching only: context lines are re-emitted with the SOURCE file's own whitespace, byte for byte. Default false."),
        )
        .param(
            Param::enumv("on_conflict", ["fail", "skip"])
                .default("fail")
                .describe("What to do when a hunk does not match, for output='patched'. 'fail' (default) refuses the whole patch and names the hunk, the line it expected and the line it found, so you never get a half-applied file by accident. 'skip' applies every hunk that matches and leaves the rest out, like `patch --reject` — pair it with output='rejects' to get the leftovers as a patch."),
        )
        .param(
            Param::string("file")
                .default("")
                .describe("Which file's hunks to apply when the patch touches more than one — a full path ('src/main.rs'), a bare filename ('main.rs'), or any substring of the path. Blank (default) works when the patch touches exactly one file; a multi-file patch with no filter reports the paths it found instead of guessing."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ApplyPatch;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/apply-patch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Apply a unified diff to a pasted file and return the patched text, with conflict detection.",
    skill(
        description = "Apply a unified diff / git patch to a pasted source file and return the patched text. Pass source (the file as it is now) and patch (the diff). output='patched' (default) returns the patched file; 'report' is a dry run listing every hunk with the line it landed on, its offset, the fuzz used, or why it failed; 'json' returns that as data plus the patched text; 'rejects' returns the failed hunks as a standalone patch. reverse=true unapplies the patch (git apply -R) to recover the original. fuzz (0-3, default 2) is how many context lines may be dropped from each end of a hunk when the exact context drifted; ignore_whitespace=true matches context ignoring indentation changes while keeping the source's own whitespace in the output. on_conflict='fail' (default) refuses the patch and names the expected vs found line; 'skip' applies what matches. file picks one path out of a multi-file patch. Hunks may land away from the line their @@ header claims (offset search); wrong @@ counts are recounted; CRLF, the source's final-newline state and '\\ No newline at end of file' are preserved. 1 MB cap per input. Binary, rename-only, and combined (@@@) diff entries are reported as unsupported, not silently dropped.",
        parameters = schema_json()
    ),
)]
impl ApplyPatch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "apply-patch", |a: Args| {
            if !a.fuzz.is_finite() || a.fuzz < 0.0 || a.fuzz > 3.0 {
                return Err(SkillError::InvalidArgs(format!(
                    "fuzz must be 0-3, got {}",
                    a.fuzz
                )));
            }
            gizza_ai_apply_patch_core::apply_patch(
                &a.source,
                &a.patch,
                &a.output,
                a.reverse,
                a.fuzz as u32,
                a.ignore_whitespace,
                &a.on_conflict,
                &a.file,
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
                    "source": { "type": "string", "description": "The original file text the patch applies to — paste the file as it is right now, before the change. Its line endings (LF or CRLF) and final-newline state are preserved in the result. Max 1000000 bytes (1 MB). Pass an empty string when the patch creates a new file." },
                    "patch": { "type": "string", "description": "The unified diff to apply — the output of `git diff`, `git show`, `git format-patch`, `diff -u`, or `svn diff`, including its `@@` hunk headers. `---`/`+++`/`diff --git` file headers are optional; wrong `@@` line counts are recounted from the body. Max 1000000 bytes (1 MB)." },
                    "output": { "type": "string", "enum": ["patched", "report", "json", "rejects"], "default": "patched", "description": "What to return. 'patched' (default) is the patched file text. 'report' is a dry run like `git apply --check`: one line per hunk with the line it landed on, the offset from its header, the fuzz used, or why it failed. 'json' is the same data as machine-readable fields plus the patched text. 'rejects' returns only the hunks that did NOT apply, as a standalone patch you can fix by hand. 'report' and 'json' never fail on a conflict — they describe it." },
                    "reverse": { "type": "boolean", "default": false, "description": "Apply the patch backwards, like `git apply -R` / `patch -R`: '+' and '-' swap roles so the change is undone. Use this to recover the original file from a patched one, or to revert a commit's diff. Default false." },
                    "fuzz": { "type": "integer", "default": 2, "minimum": 0, "maximum": 3, "description": "How many CONTEXT lines may be dropped from each end of a hunk when an exact match fails — the `patch -F` fuzz factor. 0 requires every context line to match; 2 (default, same as `patch(1)`) tolerates a couple of drifted surrounding lines; 3 is the maximum. Fuzz never drops a '+' or '-' line and never reduces a hunk to zero context, so it cannot turn a real conflict into a blind insertion." },
                    "ignore_whitespace": { "type": "boolean", "default": false, "description": "Match context and deleted lines with runs of whitespace collapsed and the ends trimmed, like `git apply --ignore-whitespace` — use it when the source was reindented (tabs vs spaces) since the patch was made. Matching only: context lines are re-emitted with the SOURCE file's own whitespace, byte for byte. Default false." },
                    "on_conflict": { "type": "string", "enum": ["fail", "skip"], "default": "fail", "description": "What to do when a hunk does not match, for output='patched'. 'fail' (default) refuses the whole patch and names the hunk, the line it expected and the line it found, so you never get a half-applied file by accident. 'skip' applies every hunk that matches and leaves the rest out, like `patch --reject` — pair it with output='rejects' to get the leftovers as a patch." },
                    "file": { "type": "string", "default": "", "description": "Which file's hunks to apply when the patch touches more than one — a full path ('src/main.rs'), a bare filename ('main.rs'), or any substring of the path. Blank (default) works when the patch touches exactly one file; a multi-file patch with no filter reports the paths it found instead of guessing." }
                },
                "required": ["source", "patch"],
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
            [
                "source",
                "patch",
                "output",
                "reverse",
                "fuzz",
                "ignore_whitespace",
                "on_conflict",
                "file"
            ]
        );
    }

    /// The advertised 1 MB cap and 0-3 fuzz range are the core's real limits.
    #[test]
    fn advertised_limits_match_the_core() {
        assert_eq!(gizza_ai_apply_patch_core::MAX_INPUT_BYTES, 1_000_000);
        assert_eq!(gizza_ai_apply_patch_core::MAX_FUZZ, 3);
    }
}
