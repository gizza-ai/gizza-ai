//! gizza-ai/regex-codemod — regex find-and-replace across a pasted multi-file
//! bundle, previewed as a unified diff. Thin wrapper around the core; the chat
//! schema is single-sourced from `descriptor()`; the handler delegates to
//! `run_skill`. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_regex_codemod_core::{run, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    pattern: String,
    #[serde(default)]
    replacement: String,
    #[serde(default)]
    literal: bool,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    multiline: bool,
    #[serde(default)]
    dot_matches_newline: bool,
    #[serde(default)]
    whole_word: bool,
    #[serde(default = "default_true")]
    replace_all: bool,
    #[serde(default = "default_marker")]
    file_marker: String,
    #[serde(default)]
    marker_regex: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_context")]
    context: u32,
    #[serde(default)]
    include_unchanged: bool,
}

fn default_true() -> bool {
    true
}
fn default_marker() -> String {
    "auto".to_string()
}
fn default_output() -> String {
    "diff".to_string()
}
fn default_context() -> u32 {
    3
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The pasted bundle: one or more files concatenated behind header markers, e.g. \"=== src/a.js ===\" followed by that file's contents."))
        .param(Param::string("pattern").required().describe("The regular expression to find (Rust regex syntax, e.g. \\boldName\\b). With literal=true it is matched as plain text instead."))
        .param(Param::string("replacement").default("").describe("Replacement text. $1/${name} insert capture groups; $$ is a literal dollar. With literal=true it is inserted verbatim. Default empty, which deletes each match."))
        .param(Param::boolean("literal").default(false).describe("Match `pattern` as plain text instead of a regular expression, and insert `replacement` verbatim. Default false."))
        .param(Param::boolean("ignore_case").default(false).describe("Match case-insensitively (the regex `i` flag). Default false."))
        .param(Param::boolean("multiline").default(false).describe("Make ^ and $ match at the start and end of every line (the regex `m` flag). Default false."))
        .param(Param::boolean("dot_matches_newline").default(false).describe("Let `.` also match newlines, so a pattern can span lines (the regex `s` flag). Default false."))
        .param(Param::boolean("whole_word").default(false).describe("Require a word boundary on both sides of the match, so \"id\" does not match inside \"idx\". Default false."))
        .param(Param::boolean("replace_all").default(true).describe("Replace every match (the regex `g` flag). When false, only the first match in each file is replaced. Default true."))
        .param(Param::enumv("file_marker", ["auto", "equals", "dashes", "arrow", "comment", "custom", "none"]).default("auto").describe("How file boundaries are detected in the bundle: auto (any of the four below), equals (=== path ===), dashes (--- path ---), arrow (==> path <==), comment (# file: path or // file: path), custom (use marker_regex), or none (treat the whole input as one file). Default auto."))
        .param(Param::string("marker_regex").default("").describe("Custom file-marker pattern, matched against each line, with one capture group for the path, e.g. ^@@@ (\\S+)$. Only used when file_marker=custom."))
        .param(Param::enumv("output", ["diff", "full", "json"]).default("diff").describe("What to return: diff (a git-apply-able unified diff of the changes), full (the whole rewritten bundle) or json (a per-file report with counts and diffs). Default diff."))
        .param(Param::integer("context").default(3).min(0.0).max(100.0).describe("Unchanged context lines shown around each hunk in the unified diff. Default 3."))
        .param(Param::boolean("include_unchanged").default(false).describe("Also report files the pattern did not change — as \"# unchanged: path\" comment lines in diff output, or as entries in json output. Default false."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options(a: &Args) -> Options {
    Options {
        literal: a.literal,
        ignore_case: a.ignore_case,
        multiline: a.multiline,
        dot_matches_newline: a.dot_matches_newline,
        whole_word: a.whole_word,
        replace_all: a.replace_all,
        context: a.context as usize,
        include_unchanged: a.include_unchanged,
    }
}

#[cfg(target_arch = "wasm32")]
struct RegexCodemod;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regex-codemod",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Run a regex find-and-replace across a pasted multi-file bundle and preview the diff",
    skill(
        description = "Apply one regex (or literal) find-and-replace across a pasted multi-file bundle and preview the result. Files are split at header markers (=== path ===, --- path ---, ==> path <==, # file: path, a custom marker_regex, or none for a single file) and each file is rewritten independently. Replacements support $1/${name} capture references, and the i/m/s flags, whole-word matching and first-match-only are all available. Returns a git-apply-able unified diff (default), the fully rewritten bundle, or a JSON report with per-file replacement counts.",
        parameters = schema_json()
    ),
)]
impl RegexCodemod {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "regex-codemod", |a: Args| {
            let opts = options(&a);
            run(
                &a.text,
                &a.pattern,
                &a.replacement,
                &a.file_marker,
                &a.marker_regex,
                &a.output,
                &opts,
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
        // `r##` delimiters: one description contains the `"#` sequence, which would
        // close a plain `r#"…"#` raw string early.
        let authored: serde_json::Value = serde_json::from_str(r##"{
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The pasted bundle: one or more files concatenated behind header markers, e.g. \"=== src/a.js ===\" followed by that file's contents." },
                "pattern": { "type": "string", "description": "The regular expression to find (Rust regex syntax, e.g. \\boldName\\b). With literal=true it is matched as plain text instead." },
                "replacement": { "type": "string", "default": "", "description": "Replacement text. $1/${name} insert capture groups; $$ is a literal dollar. With literal=true it is inserted verbatim. Default empty, which deletes each match." },
                "literal": { "type": "boolean", "default": false, "description": "Match `pattern` as plain text instead of a regular expression, and insert `replacement` verbatim. Default false." },
                "ignore_case": { "type": "boolean", "default": false, "description": "Match case-insensitively (the regex `i` flag). Default false." },
                "multiline": { "type": "boolean", "default": false, "description": "Make ^ and $ match at the start and end of every line (the regex `m` flag). Default false." },
                "dot_matches_newline": { "type": "boolean", "default": false, "description": "Let `.` also match newlines, so a pattern can span lines (the regex `s` flag). Default false." },
                "whole_word": { "type": "boolean", "default": false, "description": "Require a word boundary on both sides of the match, so \"id\" does not match inside \"idx\". Default false." },
                "replace_all": { "type": "boolean", "default": true, "description": "Replace every match (the regex `g` flag). When false, only the first match in each file is replaced. Default true." },
                "file_marker": { "type": "string", "enum": ["auto", "equals", "dashes", "arrow", "comment", "custom", "none"], "default": "auto", "description": "How file boundaries are detected in the bundle: auto (any of the four below), equals (=== path ===), dashes (--- path ---), arrow (==> path <==), comment (# file: path or // file: path), custom (use marker_regex), or none (treat the whole input as one file). Default auto." },
                "marker_regex": { "type": "string", "default": "", "description": "Custom file-marker pattern, matched against each line, with one capture group for the path, e.g. ^@@@ (\\S+)$. Only used when file_marker=custom." },
                "output": { "type": "string", "enum": ["diff", "full", "json"], "default": "diff", "description": "What to return: diff (a git-apply-able unified diff of the changes), full (the whole rewritten bundle) or json (a per-file report with counts and diffs). Default diff." },
                "context": { "type": "integer", "minimum": 0, "maximum": 100, "default": 3, "description": "Unchanged context lines shown around each hunk in the unified diff. Default 3." },
                "include_unchanged": { "type": "boolean", "default": false, "description": "Also report files the pattern did not change — as \"# unchanged: path\" comment lines in diff output, or as entries in json output. Default false." }
            },
            "required": ["text", "pattern"],
            "additionalProperties": false
        }"##).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
