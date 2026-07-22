//! gizza-ai/import-graph-extractor — chat skill block on the shared tool abstraction.
//! Paste one or more source files and get the import/require/use dependency graph.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill and returns the core's rendered graph.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    language: String,
    #[serde(default)]
    format: String,
    #[serde(default = "default_true")]
    include_external: bool,
    #[serde(default = "default_true")]
    detect_cycles: bool,
}

/// Single source for the chat schema (and the CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe(
                    "Source code to analyze. Paste one or more files; separate multiple files with a header line like `=== src/app.js ===` or `--- src/app.js ---` (the path sets each file's name and, in auto mode, its language from the extension). With no headers the whole input is treated as one file and its name is inferred from `language`.",
                ),
        )
        .param(
            Param::enumv("language", ["auto", "javascript", "python", "rust", "go"])
                .default("auto")
                .describe(
                    "Source language. 'auto' (default) detects each file from its header extension, or sniffs a single headerless file. Force 'javascript' (JS/TS/JSX/TSX), 'python', 'rust', or 'go' to treat every pasted file as that language.",
                ),
        )
        .param(
            Param::enumv("format", ["text", "json", "dot", "mermaid"])
                .default("text")
                .describe(
                    "Output format. 'text' (default) is a human-readable report; 'json' is a structured graph object; 'dot' is Graphviz DOT; 'mermaid' is a Mermaid flowchart. Paste dot/mermaid into any renderer for a picture.",
                ),
        )
        .param(
            Param::boolean("include_external")
                .default(true)
                .describe(
                    "Include third-party packages and standard-library modules (e.g. react, lodash, os, fmt) in the report and graph. Default true; set false to focus only on file-to-file edges.",
                ),
        )
        .param(
            Param::boolean("detect_cycles")
                .default(true)
                .describe(
                    "Detect and report circular dependencies (import cycles) between the pasted files. Default true.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/import-graph-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract the import/dependency graph from pasted source files.",
    skill(
        description = "Extract the import/require/use dependency graph from pasted source code. Paste one or more files, separated by `=== path ===` (or `--- path ---`) header lines; supports JavaScript/TypeScript (import, dynamic import, export-from, require), Python (import, from, relative imports), Rust (use, mod, extern crate), and Go (import). Resolves file-to-file edges for JS/TS relative paths and Python dotted modules, and reports external dependencies, dependents (who imports each file), orphan and leaf files, and circular dependencies. Choose format=text (default), json, dot (Graphviz), or mermaid; toggle include_external and detect_cycles.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "import-graph-extractor", |a: Args| {
            gizza_ai_import_graph_extractor_core::run(
                &a.input,
                &a.language,
                &a.format,
                a.include_external,
                a.detect_cycles,
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
    /// schema so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Source code to analyze. Paste one or more files; separate multiple files with a header line like `=== src/app.js ===` or `--- src/app.js ---` (the path sets each file's name and, in auto mode, its language from the extension). With no headers the whole input is treated as one file and its name is inferred from `language`." },
                    "language": { "type": "string", "enum": ["auto", "javascript", "python", "rust", "go"], "default": "auto", "description": "Source language. 'auto' (default) detects each file from its header extension, or sniffs a single headerless file. Force 'javascript' (JS/TS/JSX/TSX), 'python', 'rust', or 'go' to treat every pasted file as that language." },
                    "format": { "type": "string", "enum": ["text", "json", "dot", "mermaid"], "default": "text", "description": "Output format. 'text' (default) is a human-readable report; 'json' is a structured graph object; 'dot' is Graphviz DOT; 'mermaid' is a Mermaid flowchart. Paste dot/mermaid into any renderer for a picture." },
                    "include_external": { "type": "boolean", "default": true, "description": "Include third-party packages and standard-library modules (e.g. react, lodash, os, fmt) in the report and graph. Default true; set false to focus only on file-to-file edges." },
                    "detect_cycles": { "type": "boolean", "default": true, "description": "Detect and report circular dependencies (import cycles) between the pasted files. Default true." }
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
