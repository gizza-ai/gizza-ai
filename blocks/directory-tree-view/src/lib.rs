//! gizza-ai/directory-tree-view — turn a pasted file listing (path + byte size
//! per line) into a size-annotated indented tree, like `tree -s -h --du`.
//!
//! Thin chat-skill wrapper around `gizza-ai-directory-tree-view-core`. The chat
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
    format: String,
    #[serde(default)]
    units: String,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    root: String,
    /// Plain-ASCII connectors instead of Unicode box-drawing. Default false.
    #[serde(default)]
    ascii: bool,
    /// List directories before files within each folder. Default true.
    #[serde(default = "default_true")]
    dirs_first: bool,
    /// Append "/" to directory names. Default true.
    #[serde(default = "default_true")]
    trailing_slash: bool,
    /// Show per-directory (files, dirs) counts. Default true.
    #[serde(default = "default_true")]
    show_counts: bool,
    /// Maximum tree depth to print (0 = unlimited). Default 0.
    #[serde(default)]
    depth: i64,
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
                .describe("The file listing, one entry per line, each line carrying a slash-separated path and a byte size. Accepts `du -ab` / `find -printf '%s\\t%p\\n'` output (size then a tab then the path), a `path,size` CSV export, or a mix. Sizes may use suffixes (4K, 1.5M, 2MiB, 500KB). Directory sizes are rolled up from their contents. Blank lines are ignored."),
        )
        .param(
            Param::enumv("format", ["auto", "size-first", "path-first"])
                .default("auto")
                .describe("How each line is split into a path and a size. 'auto' (default) detects per line: a comma → path,size CSV, a tab or leading number → size-first. 'size-first' forces size-then-path (du/find). 'path-first' forces path-then-size (CSV)."),
        )
        .param(
            Param::enumv("units", ["human", "si", "bytes"])
                .default("human")
                .describe("Size unit style. 'human' (default) uses 1024-based K/M/G (like tree -h); 'si' uses 1000-based k/M/G (like tree --si); 'bytes' prints raw bytes with thousands separators (like tree -s)."),
        )
        .param(
            Param::enumv("sort", ["name", "size-desc", "input"])
                .default("name")
                .describe("Order of entries within each directory. 'name' (default) is case-insensitive alphabetical; 'size-desc' is largest cumulative size first; 'input' preserves the order paths first appeared."),
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
            Param::boolean("dirs_first")
                .default(true)
                .describe("List directories before files within each folder (like tree --dirsfirst). Default true."),
        )
        .param(
            Param::boolean("trailing_slash")
                .default(true)
                .describe("Append '/' to directory names. Default true."),
        )
        .param(
            Param::boolean("show_counts")
                .default(true)
                .describe("Annotate each directory with its (files, dirs) counts. Default true. The final report line always shows the totals."),
        )
        .param(
            Param::integer("depth")
                .min(0.0)
                .default(0)
                .describe("Maximum tree depth to print (0 = unlimited, like tree -L). Deeper entries are hidden but still counted in their ancestors' rolled-up sizes. Default 0."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DirectoryTreeView;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/directory-tree-view",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a pasted file listing into a size-annotated directory tree.",
    skill(
        description = "Turn a pasted file listing (a slash-separated path and a byte size per line) into a size-annotated indented tree, like `tree -s -h --du`. Accepts `du -ab` / `find -printf '%s\\t%p\\n'` output, a `path,size` CSV export, or a mix (format=auto by default; size-first / path-first force a layout). Directory sizes roll up cumulatively from their contents and each directory can show its (files, dirs) counts (show_counts, default true), ending with a totals report line. units picks the size style: human (1024-based K/M/G, default), si (1000-based), or bytes (raw with separators). sort orders each folder by name (default), size-desc, or input order; dirs_first lists directories first (default true). root labels the top line (default '.'); depth caps the printed levels (0 = unlimited); ascii swaps box-drawing connectors for plain ASCII; trailing_slash adds '/' to directories (default true).",
        parameters = schema_json()
    ),
)]
impl DirectoryTreeView {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "directory-tree-view", |a: Args| {
            gizza_ai_directory_tree_view_core::build(
                &a.input,
                &a.format,
                &a.units,
                &a.sort,
                &a.root,
                a.ascii,
                a.dirs_first,
                a.trailing_slash,
                a.show_counts,
                a.depth,
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
                    "input": { "type": "string", "description": "The file listing, one entry per line, each line carrying a slash-separated path and a byte size. Accepts `du -ab` / `find -printf '%s\\t%p\\n'` output (size then a tab then the path), a `path,size` CSV export, or a mix. Sizes may use suffixes (4K, 1.5M, 2MiB, 500KB). Directory sizes are rolled up from their contents. Blank lines are ignored." },
                    "format": { "type": "string", "enum": ["auto", "size-first", "path-first"], "default": "auto", "description": "How each line is split into a path and a size. 'auto' (default) detects per line: a comma → path,size CSV, a tab or leading number → size-first. 'size-first' forces size-then-path (du/find). 'path-first' forces path-then-size (CSV)." },
                    "units": { "type": "string", "enum": ["human", "si", "bytes"], "default": "human", "description": "Size unit style. 'human' (default) uses 1024-based K/M/G (like tree -h); 'si' uses 1000-based k/M/G (like tree --si); 'bytes' prints raw bytes with thousands separators (like tree -s)." },
                    "sort": { "type": "string", "enum": ["name", "size-desc", "input"], "default": "name", "description": "Order of entries within each directory. 'name' (default) is case-insensitive alphabetical; 'size-desc' is largest cumulative size first; 'input' preserves the order paths first appeared." },
                    "root": { "type": "string", "default": ".", "description": "Label for the top line of the tree (the root). Default '.'." },
                    "ascii": { "type": "boolean", "default": false, "description": "Use plain-ASCII connectors ('|--', '`--') instead of Unicode box-drawing ('├──', '└──'). Default false." },
                    "dirs_first": { "type": "boolean", "default": true, "description": "List directories before files within each folder (like tree --dirsfirst). Default true." },
                    "trailing_slash": { "type": "boolean", "default": true, "description": "Append '/' to directory names. Default true." },
                    "show_counts": { "type": "boolean", "default": true, "description": "Annotate each directory with its (files, dirs) counts. Default true. The final report line always shows the totals." },
                    "depth": { "type": "integer", "minimum": 0, "default": 0, "description": "Maximum tree depth to print (0 = unlimited, like tree -L). Deeper entries are hidden but still counted in their ancestors' rolled-up sizes. Default 0." }
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
