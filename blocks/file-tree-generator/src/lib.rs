//! gizza-ai/file-tree-generator — turn file paths or an indented outline into an
//! ASCII tree diagram for READMEs and docs.
//!
//! Thin chat-skill wrapper around `gizza-ai-file-tree-generator-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host calls
//! — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    root: String,
    /// Plain-ASCII connectors instead of Unicode box-drawing. Default false.
    #[serde(default)]
    ascii: bool,
    /// Append "/" to directory names. Default true.
    #[serde(default = "default_true")]
    trailing_slash: bool,
    /// Sort entries (directories first, then alphabetical). Default false.
    #[serde(default)]
    sort: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The file list or outline, one entry per line. In 'paths' mode each line is a slash-separated path like 'src/main.rs' (a trailing '/' marks a directory); lines sharing a prefix merge into one tree. In 'outline' mode each line's leading whitespace (spaces or tabs, a tab = 4 columns) sets the nesting. Blank lines are ignored."),
        )
        .param(
            Param::enumv("mode", ["paths", "outline"])
                .default("paths")
                .describe("How to read the input. 'paths' (default) merges slash-separated paths on shared prefixes; 'outline' nests by leading indentation."),
        )
        .param(
            Param::string("root")
                .default(".")
                .describe("Label for the top line of the tree (the root). Default '.'."),
        )
        .param(
            Param::boolean("ascii")
                .default(false)
                .describe("Use plain-ASCII connectors ('|--', '`--') instead of Unicode box-drawing ('├──', '└──'). Default false."),
        )
        .param(
            Param::boolean("trailing_slash")
                .default(true)
                .describe("Append '/' to directory names. Default true."),
        )
        .param(
            Param::boolean("sort")
                .default(false)
                .describe("Sort each directory's entries (directories first, then files, each alphabetical). Default false preserves input order."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FileTreeGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/file-tree-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn file paths or an indented outline into an ASCII tree diagram.",
    skill(
        description = "Turn a list of file paths or an indented outline into an ASCII tree diagram for READMEs and docs. In mode='paths' (default) each line is a slash-separated path like 'src/main.rs' and lines sharing a prefix merge into one tree (a trailing '/' marks a directory). In mode='outline' each line's leading whitespace sets the nesting. Output uses Unicode box-drawing connectors (├──, └──, │) by default, or plain ASCII (|--, `--) when ascii=true. 'root' labels the top line (default '.'). trailing_slash adds '/' to directories (default true). sort orders entries directories-first then alphabetically (default false keeps input order).",
        parameters = schema_json()
    ),
)]
impl FileTreeGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "file-tree-generator", |a: Args| {
            gizza_ai_file_tree_generator_core::generate(
                &a.input,
                &a.mode,
                &a.root,
                a.ascii,
                a.trailing_slash,
                a.sort,
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
                    "input": { "type": "string", "description": "The file list or outline, one entry per line. In 'paths' mode each line is a slash-separated path like 'src/main.rs' (a trailing '/' marks a directory); lines sharing a prefix merge into one tree. In 'outline' mode each line's leading whitespace (spaces or tabs, a tab = 4 columns) sets the nesting. Blank lines are ignored." },
                    "mode": { "type": "string", "enum": ["paths", "outline"], "default": "paths", "description": "How to read the input. 'paths' (default) merges slash-separated paths on shared prefixes; 'outline' nests by leading indentation." },
                    "root": { "type": "string", "default": ".", "description": "Label for the top line of the tree (the root). Default '.'." },
                    "ascii": { "type": "boolean", "default": false, "description": "Use plain-ASCII connectors ('|--', '`--') instead of Unicode box-drawing ('├──', '└──'). Default false." },
                    "trailing_slash": { "type": "boolean", "default": true, "description": "Append '/' to directory names. Default true." },
                    "sort": { "type": "boolean", "default": false, "description": "Sort each directory's entries (directories first, then files, each alphabetical). Default false preserves input order." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
