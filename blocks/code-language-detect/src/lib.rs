//! gizza-ai/code-language-detect — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_code_language_detect_core::{detect_to_string, Options, MAX_TOP_K};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
#[serde(default)]
struct Args {
    code: String,
    filename: String,
    candidates: String,
    common_only: bool,
    top_k: usize,
    explain: bool,
    output: String,
}

impl Default for Args {
    fn default() -> Self {
        let opts = Options::default();
        Args {
            code: String::new(),
            filename: opts.filename,
            candidates: opts.candidates,
            common_only: opts.common_only,
            top_k: opts.top_k,
            explain: opts.explain,
            output: opts.output,
        }
    }
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            filename: a.filename,
            candidates: a.candidates,
            common_only: a.common_only,
            top_k: a.top_k,
            explain: a.explain,
            output: a.output,
        }
    }
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The source-code snippet to identify. Paste at least three to five lines for the most reliable result."),
        )
        .param(
            Param::string("filename")
                .default("")
                .describe("Optional filename hint such as main.rs, Dockerfile, package.json, or styles.css. Extensions and special filenames add a strong score bonus."),
        )
        .param(
            Param::string("candidates")
                .default("")
                .describe("Optional comma-separated allowlist of language ids to consider, such as rust,python,javascript. Leave empty to consider every built-in language."),
        )
        .param(
            Param::boolean("common_only")
                .default(false)
                .describe("Only consider common mainstream languages. Useful for short snippets where niche-language signals can otherwise win."),
        )
        .param(
            Param::integer("top_k")
                .default(3)
                .min(0.0)
                .max(MAX_TOP_K as f64)
                .describe("Number of ranked candidate languages to show. Use 0 to list every language with a positive score."),
        )
        .param(
            Param::boolean("explain")
                .default(true)
                .describe("Include the weighted signals that explain the top detection."),
        )
        .param(
            Param::enumv("output", ["report", "json", "language"])
                .default("report")
                .describe("Output format: readable report, structured JSON, or just the top language id."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/code-language-detect",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect a code snippet's programming language with explainable heuristics.",
    skill(
        description = "Detect the likely programming language of a pasted source-code snippet using deterministic weighted signals, optional filename/shebang hints, a common-language filter, ranked alternatives, confidence, and evidence. Runs locally with no network or ML model.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "code-language-detect", |a: Args| {
            let code = a.code.clone();
            let opts: Options = a.into();
            detect_to_string(&code, &opts).map_err(SkillError::InvalidArgs)
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
                    "code": { "type": "string", "description": "The source-code snippet to identify. Paste at least three to five lines for the most reliable result." },
                    "filename": { "type": "string", "default": "", "description": "Optional filename hint such as main.rs, Dockerfile, package.json, or styles.css. Extensions and special filenames add a strong score bonus." },
                    "candidates": { "type": "string", "default": "", "description": "Optional comma-separated allowlist of language ids to consider, such as rust,python,javascript. Leave empty to consider every built-in language." },
                    "common_only": { "type": "boolean", "default": false, "description": "Only consider common mainstream languages. Useful for short snippets where niche-language signals can otherwise win." },
                    "top_k": { "type": "integer", "default": 3, "minimum": 0, "maximum": 30, "description": "Number of ranked candidate languages to show. Use 0 to list every language with a positive score." },
                    "explain": { "type": "boolean", "default": true, "description": "Include the weighted signals that explain the top detection." },
                    "output": { "type": "string", "enum": ["report", "json", "language"], "default": "report", "description": "Output format: readable report, structured JSON, or just the top language id." }
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
