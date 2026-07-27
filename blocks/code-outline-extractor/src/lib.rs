//! gizza-ai/code-outline-extractor — produce a structured, nested outline of the
//! functions, classes, and methods in pasted source code.
//!
//! Thin chat-skill wrapper around `gizza-ai-code-outline-extractor-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default)]
    language: String,
    #[serde(default)]
    format: String,
    /// Show each entry's full signature instead of just its name (default false).
    #[serde(default)]
    signatures: bool,
    /// Annotate each entry with its 1-based source line (default true).
    #[serde(default = "default_true")]
    line_numbers: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The source code to outline. Paste one file or a snippet."),
        )
        .param(
            Param::enumv(
                "language",
                [
                    "auto",
                    "python",
                    "javascript",
                    "typescript",
                    "rust",
                    "go",
                    "java",
                    "c",
                    "cpp",
                    "csharp",
                    "php",
                    "swift",
                ],
            )
            .default("auto")
            .describe("Source language. 'auto' (default) guesses from the code (Python by indentation, everything else by braces). Set it explicitly if auto-detect picks wrong."),
        )
        .param(
            Param::enumv("format", ["tree", "markdown", "json"])
                .default("tree")
                .describe("Output shape. 'tree' (default) is an indented text outline; 'markdown' is a nested bullet list (a table of contents); 'json' is a nested array of {kind, name, line, signature, children}."),
        )
        .param(
            Param::boolean("signatures")
                .default(false)
                .describe("When true, show each entry's full header line (e.g. 'def add(a, b)') instead of just its name. Ignored by the json format, which always includes the signature field. Default false."),
        )
        .param(
            Param::boolean("line_numbers")
                .default(true)
                .describe("When true (default), annotate each entry with its 1-based source line. Ignored by the json format, which always includes the line field."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CodeOutlineExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/code-outline-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Outline the functions, classes, and methods in source code.",
    skill(
        description = "Produce a structured, nested outline of the functions, classes, and methods in pasted source code — a symbol tree / table of contents. Supports Python, JavaScript, TypeScript, Rust, Go, Java, C, C++, C#, PHP, and Swift; language='auto' (default) guesses from the code. Methods nest under their class. format='tree' (default) is an indented text outline, 'markdown' a nested bullet list, 'json' a nested array of {kind, name, line, signature, children}. Set signatures=true to show each entry's full header line instead of just its name; line_numbers defaults to true. Uses a dependency-free brace-depth / indentation heuristic (no full language parser), so unusual multi-line signatures may be missed.",
        parameters = schema_json()
    ),
)]
impl CodeOutlineExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "code-outline-extractor", |a: Args| {
            gizza_ai_code_outline_extractor_core::outline(
                &a.code,
                &a.language,
                &a.format,
                a.signatures,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "code": { "type": "string", "description": "The source code to outline. Paste one file or a snippet." },
                    "language": { "type": "string", "enum": ["auto", "python", "javascript", "typescript", "rust", "go", "java", "c", "cpp", "csharp", "php", "swift"], "default": "auto", "description": "Source language. 'auto' (default) guesses from the code (Python by indentation, everything else by braces). Set it explicitly if auto-detect picks wrong." },
                    "format": { "type": "string", "enum": ["tree", "markdown", "json"], "default": "tree", "description": "Output shape. 'tree' (default) is an indented text outline; 'markdown' is a nested bullet list (a table of contents); 'json' is a nested array of {kind, name, line, signature, children}." },
                    "signatures": { "type": "boolean", "default": false, "description": "When true, show each entry's full header line (e.g. 'def add(a, b)') instead of just its name. Ignored by the json format, which always includes the signature field. Default false." },
                    "line_numbers": { "type": "boolean", "default": true, "description": "When true (default), annotate each entry with its 1-based source line. Ignored by the json format, which always includes the line field." }
                },
                "required": ["code"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
