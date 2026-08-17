//! gizza-ai/path-extractor — extract file paths from logs, stack traces, and prose.
//! Thin chat-skill wrapper; descriptor() is the single source for the chat schema
//! and CLI, while the core crate owns the scanner.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_path_style")]
    path_style: String,
    #[serde(default = "default_true")]
    require_separator: bool,
    #[serde(default)]
    keep_line_numbers: bool,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    extensions: String,
    #[serde(default = "default_extension_mode")]
    extension_mode: String,
    #[serde(default = "default_true")]
    dedupe: bool,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_true() -> bool {
    true
}
fn default_path_style() -> String {
    "any".to_string()
}
fn default_output() -> String {
    "path".to_string()
}
fn default_extension_mode() -> String {
    "include".to_string()
}
fn default_sort() -> String {
    "first-seen".to_string()
}
fn default_format() -> String {
    "list".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("Log, stack trace, git status output, compiler output, or prose to scan for file paths. Handles POSIX paths, Windows drive paths, UNC paths, quoted paths, and common path:line[:column] locators. Limit: 1,000,000 bytes."),
        )
        .param(
            Param::enumv("path_style", ["any", "posix", "windows"])
                .default("any")
                .describe("Which path flavour to keep: 'any' (default), 'posix' for forward-slash paths, or 'windows' for drive-letter/UNC/backslash paths. Bare filenames matched with require_separator=false survive both posix and windows filters."),
        )
        .param(
            Param::boolean("require_separator")
                .default(true)
                .describe("Require a directory separator (/ or \\) in each match (default true). Turn off to also match bare extension-bearing filenames like main.rs and Cargo.toml; this is higher recall but can pick up more ambiguous tokens."),
        )
        .param(
            Param::boolean("keep_line_numbers")
                .default(false)
                .describe("Keep trailing source locators in the returned path text when present, such as src/main.rs:42 or src/main.rs:42:9 (default false strips them). JSON output still exposes line and column fields when they were found."),
        )
        .param(
            Param::enumv("output", ["path", "filename", "directory"])
                .default("path")
                .describe("Return the whole path (default), only the final filename, or only the containing directory."),
        )
        .param(
            Param::string("extensions")
                .default("")
                .describe("Optional extension filter as a comma/space/semicolon-separated list, with or without dots (for example 'rs, toml, md'). Leave blank to keep all extensions."),
        )
        .param(
            Param::enumv("extension_mode", ["include", "exclude"])
                .default("include")
                .describe("How to apply the extensions list: 'include' keeps only listed extensions (default), while 'exclude' drops listed extensions. Ignored when extensions is blank."),
        )
        .param(
            Param::boolean("dedupe")
                .default(true)
                .describe("Deduplicate repeated paths and count occurrences (default true). Turn off to keep every occurrence in first-seen order."),
        )
        .param(
            Param::enumv("sort", ["first-seen", "asc", "desc"])
                .default("first-seen")
                .describe("Result ordering: keep first-seen order (default), sort ascending A to Z, or sort descending Z to A. Sorting is case-insensitive with a stable path tie-breaker."),
        )
        .param(
            Param::enumv("format", ["list", "csv", "json"])
                .default("list")
                .describe("Output format: newline list (default), CSV with occurrence counts, or JSON with count and match metadata."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PathExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/path-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract and deduplicate file paths from logs or prose",
    skill(
        description = "Extract file paths from arbitrary pasted text such as build logs, stack traces, git status output, compiler messages, and prose. Detects POSIX absolute/relative paths, Windows drive-letter paths, UNC paths, tilde paths, quoted paths with spaces when anchored, and common path:LINE[:COL] or path(LINE,COL) locators. Options filter by path style, include or strip line numbers, return whole paths/filenames/directories, include or exclude extensions, deduplicate with occurrence counts, sort first-seen/A-Z/Z-A, and render as a newline list, CSV, or JSON. The matcher is shape-based only: it never stats the filesystem, never opens files, ignores URLs/dates/prose, and runs locally with a 1 MB input cap.",
        parameters = schema_json()
    ),
)]
impl PathExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "path-extractor", |a: Args| {
            gizza_ai_path_extractor_core::run(
                &a.text,
                &a.path_style,
                a.require_separator,
                a.keep_line_numbers,
                &a.output,
                &a.extensions,
                &a.extension_mode,
                a.dedupe,
                &a.sort,
                &a.format,
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
              "type": "object",
              "properties": {
                "text": {"type":"string","description":"Log, stack trace, git status output, compiler output, or prose to scan for file paths. Handles POSIX paths, Windows drive paths, UNC paths, quoted paths, and common path:line[:column] locators. Limit: 1,000,000 bytes."},
                "path_style": {"type":"string","enum":["any","posix","windows"],"default":"any","description":"Which path flavour to keep: 'any' (default), 'posix' for forward-slash paths, or 'windows' for drive-letter/UNC/backslash paths. Bare filenames matched with require_separator=false survive both posix and windows filters."},
                "require_separator": {"type":"boolean","default":true,"description":"Require a directory separator (/ or \\) in each match (default true). Turn off to also match bare extension-bearing filenames like main.rs and Cargo.toml; this is higher recall but can pick up more ambiguous tokens."},
                "keep_line_numbers": {"type":"boolean","default":false,"description":"Keep trailing source locators in the returned path text when present, such as src/main.rs:42 or src/main.rs:42:9 (default false strips them). JSON output still exposes line and column fields when they were found."},
                "output": {"type":"string","enum":["path","filename","directory"],"default":"path","description":"Return the whole path (default), only the final filename, or only the containing directory."},
                "extensions": {"type":"string","default":"","description":"Optional extension filter as a comma/space/semicolon-separated list, with or without dots (for example 'rs, toml, md'). Leave blank to keep all extensions."},
                "extension_mode": {"type":"string","enum":["include","exclude"],"default":"include","description":"How to apply the extensions list: 'include' keeps only listed extensions (default), while 'exclude' drops listed extensions. Ignored when extensions is blank."},
                "dedupe": {"type":"boolean","default":true,"description":"Deduplicate repeated paths and count occurrences (default true). Turn off to keep every occurrence in first-seen order."},
                "sort": {"type":"string","enum":["first-seen","asc","desc"],"default":"first-seen","description":"Result ordering: keep first-seen order (default), sort ascending A to Z, or sort descending Z to A. Sorting is case-insensitive with a stable path tie-breaker."},
                "format": {"type":"string","enum":["list","csv","json"],"default":"list","description":"Output format: newline list (default), CSV with occurrence counts, or JSON with count and match metadata."}
              },
              "required": ["text"],
              "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing schema drift");
    }

    #[test]
    fn args_defaults_extract_a_plain_list() {
        let a: Args = serde_json::from_str(r#"{"text":"src/lib.rs src/lib.rs"}"#).unwrap();
        assert_eq!(a.path_style, "any");
        assert!(a.require_separator);
        assert!(a.dedupe);
        assert_eq!(a.format, "list");
        let out = gizza_ai_path_extractor_core::run(
            &a.text,
            &a.path_style,
            a.require_separator,
            a.keep_line_numbers,
            &a.output,
            &a.extensions,
            &a.extension_mode,
            a.dedupe,
            &a.sort,
            &a.format,
        )
        .unwrap();
        assert_eq!(out, "src/lib.rs");
    }
}
