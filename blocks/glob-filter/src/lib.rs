//! gizza-ai/glob-filter — filter a path list by glob / gitignore-style
//! include/exclude patterns. Chat schema single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to run_skill. Pure → all
//! backends, no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_glob_filter_core::{filter, OutputMode, Syntax};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    paths: String,
    #[serde(default)]
    include: String,
    #[serde(default)]
    exclude: String,
    #[serde(default)]
    syntax: String,
    #[serde(default = "default_true")]
    case_sensitive: bool,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("paths")
                .required()
                .describe("The list of paths to test, one per line (e.g. the output of `git ls-files` or `find`)."),
        )
        .param(Param::string("include").describe(
            "Include patterns, one per line. A path is kept only if it matches an include pattern (leave empty to include everything). A line starting with '!' re-includes; the last matching line wins.",
        ))
        .param(Param::string("exclude").describe(
            "Exclude patterns, one per line. A path matching an exclude pattern is dropped (exclude overrides include). A line starting with '!' re-includes; the last matching line wins.",
        ))
        .param(
            Param::enumv("syntax", ["gitignore", "glob"])
                .default("gitignore")
                .describe("Pattern dialect. 'gitignore' (default): git's .gitignore rules — a pattern with no '/' matches at any depth, a leading/embedded '/' anchors to the root, a match also covers a directory's contents, and '#' lines are comments. 'glob': each pattern must match the whole path ('*'/'?' stay within a segment, '**' spans them — use '**/' for any depth)."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(true)
                .describe("Match case-sensitively (default true). Set false to ignore case, e.g. on case-insensitive filesystems."),
        )
        .param(
            Param::enumv("output", ["matched", "unmatched", "annotated"])
                .default("matched")
                .describe("What to return. 'matched' (default): only the kept paths. 'unmatched': only the dropped paths. 'annotated': every path prefixed with ✓ (kept) or ✗ (dropped)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GlobFilter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/glob-filter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Filter a path list by glob and gitignore-style include/exclude patterns",
    skill(
        description = "Filter a list of file paths (one per line) by glob and gitignore-style include/exclude patterns, and preview exactly which paths match. Set syntax='gitignore' (default) for git's .gitignore rules (any-depth matching, '/' anchoring, directory contents, '#' comments) or syntax='glob' for whole-path glob (use '**/' for any depth). include keeps only matching paths; exclude drops matching paths (exclude wins); a pattern line starting with '!' re-includes, last match winning. case_sensitive defaults true. output='matched' (default) returns kept paths, 'unmatched' the dropped ones, 'annotated' every path prefixed ✓/✗. Also returns total/matched/dropped counts. Runs locally.",
        parameters = schema_json()
    ),
)]
impl GlobFilter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "glob-filter", |a: Args| {
            let syntax = Syntax::parse(&a.syntax).map_err(SkillError::InvalidArgs)?;
            let output = OutputMode::parse(&a.output).map_err(SkillError::InvalidArgs)?;
            filter(
                &a.paths,
                &a.include,
                &a.exclude,
                syntax,
                a.case_sensitive,
                output,
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
            r#"{
                "type": "object",
                "properties": {
                    "paths": { "type": "string", "description": "The list of paths to test, one per line (e.g. the output of `git ls-files` or `find`)." },
                    "include": { "type": "string", "description": "Include patterns, one per line. A path is kept only if it matches an include pattern (leave empty to include everything). A line starting with '!' re-includes; the last matching line wins." },
                    "exclude": { "type": "string", "description": "Exclude patterns, one per line. A path matching an exclude pattern is dropped (exclude overrides include). A line starting with '!' re-includes; the last matching line wins." },
                    "syntax": { "type": "string", "enum": ["gitignore", "glob"], "default": "gitignore", "description": "Pattern dialect. 'gitignore' (default): git's .gitignore rules — a pattern with no '/' matches at any depth, a leading/embedded '/' anchors to the root, a match also covers a directory's contents, and '#' lines are comments. 'glob': each pattern must match the whole path ('*'/'?' stay within a segment, '**' spans them — use '**/' for any depth)." },
                    "case_sensitive": { "type": "boolean", "default": true, "description": "Match case-sensitively (default true). Set false to ignore case, e.g. on case-insensitive filesystems." },
                    "output": { "type": "string", "enum": ["matched", "unmatched", "annotated"], "default": "matched", "description": "What to return. 'matched' (default): only the kept paths. 'unmatched': only the dropped paths. 'annotated': every path prefixed with ✓ (kept) or ✗ (dropped)." }
                },
                "required": ["paths"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
