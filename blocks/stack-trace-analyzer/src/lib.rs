//! gizza-ai/stack-trace-analyzer — chat skill block on the shared tool abstraction.
//!
//! Parses a raw stack trace from Java/Kotlin, Python, JavaScript/TypeScript,
//! Go, Ruby, C#/.NET, Rust, or PHP into a structured exception chain, names the
//! root cause and the first frame of the caller's own code, and marks every
//! frame `user` or `framework`. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI + page); `handle()` delegates to
//! `block_utils::run_skill`. No host calls — runs entirely inside the WASM
//! sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    trace: String,
    #[serde(default)]
    language: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    user_packages: String,
    #[serde(default)]
    hide_framework: bool,
    #[serde(default)]
    reverse: bool,
    /// 0 → the core default (100); the core clamps to 1..=MAX_LIMIT.
    #[serde(default)]
    limit: u32,
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("trace")
                .required()
                .describe("The raw stack trace text, exactly as printed. Include the error/exception header line and the frame lines, e.g. a Java \"Exception in thread ...\" block, a Python \"Traceback (most recent call last):\" block, or a Node.js \"TypeError: ...\" block."),
        )
        .param(
            Param::enumv(
                "language",
                ["auto", "java", "python", "javascript", "go", "ruby", "csharp", "rust", "php"],
            )
            .default("auto")
            .describe("Trace language. 'auto' (default) scores marker lines and picks the best match; or force 'java' (also Kotlin/Scala/Groovy), 'python', 'javascript' (also TypeScript/Node), 'go', 'ruby', 'csharp' (.NET), 'rust', or 'php'. Set it explicitly for short or truncated traces."),
        )
        .param(
            Param::enumv("output", ["report", "table", "json"])
                .default("report")
                .describe("Output shape. 'report' (default) is a readable analysis: language, reported exception, root cause, first user frame, then each exception with its frames ('*' marks your code). 'table' is a Markdown table per exception. 'json' is the full structured result (chain, frames, counts)."),
        )
        .param(
            Param::string("user_packages")
                .default("")
                .describe("Comma-separated prefixes that identify your own code, e.g. 'com.example,myapp' or 'src/'. Matched against each frame's function and file path. When set this becomes an allow-list: only matching frames count as user code. Blank (default) uses the built-in per-language framework rules (java.*, node_modules, site-packages, /gems/, /vendor/, /rustc/, System.*, ...)."),
        )
        .param(
            Param::boolean("hide_framework")
                .default(false)
                .describe("When true, omit framework/standard-library frames from the output and report how many were hidden. Default false (all frames are shown, marked user or framework)."),
        )
        .param(
            Param::boolean("reverse")
                .default(false)
                .describe("Frame order. Default false lists frames innermost first (the throw/panic site first) for every language — Python tracebacks are reversed to match. Set true to list them outermost first, i.e. in call order from the entry point."),
        )
        .param(
            // Bounds reference the core clamp (MAX_LIMIT) so the schema can't
            // drift from what `analyze` actually enforces.
            Param::integer("limit")
                .default(100)
                .min(1.0)
                .max(gizza_ai_stack_trace_analyzer_core::MAX_LIMIT as f64)
                .describe("Maximum frames to show per exception (1-2000, default 100). Applied after hide_framework; anything beyond it is reported as a count. Raise it for deep recursion traces."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct StackTraceAnalyzer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/stack-trace-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a stack trace into frames, find the root cause, and mark your code vs framework code.",
    skill(
        description = "Analyse a raw stack trace from Java/Kotlin, Python, JavaScript/TypeScript, Go, Ruby, C#/.NET, Rust, or PHP. language='auto' (default) detects it. Returns the reported exception, the ROOT CAUSE at the end of the Caused-by / __cause__ / inner-exception chain, the FIRST FRAME of the user's own code, and every frame split into function, file, line and column with a user/framework classification. Order is normalised across languages: frames innermost (throw site) first, the chain reported-exception first. output='report' (default) is readable prose, 'table' a Markdown table, 'json' the full structure. Set user_packages to a comma-separated prefix list to override which frames count as the caller's code; hide_framework=true drops library frames; reverse=true lists frames in call order; limit caps frames per exception (default 100).",
        parameters = schema_json()
    ),
)]
impl StackTraceAnalyzer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "stack-trace-analyzer", |a: Args| {
            gizza_ai_stack_trace_analyzer_core::analyze(
                &a.trace,
                &a.language,
                &a.output,
                &a.user_packages,
                a.hide_framework,
                a.reverse,
                a.limit,
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
                    "trace": { "type": "string", "description": "The raw stack trace text, exactly as printed. Include the error/exception header line and the frame lines, e.g. a Java \"Exception in thread ...\" block, a Python \"Traceback (most recent call last):\" block, or a Node.js \"TypeError: ...\" block." },
                    "language": { "type": "string", "enum": ["auto", "java", "python", "javascript", "go", "ruby", "csharp", "rust", "php"], "default": "auto", "description": "Trace language. 'auto' (default) scores marker lines and picks the best match; or force 'java' (also Kotlin/Scala/Groovy), 'python', 'javascript' (also TypeScript/Node), 'go', 'ruby', 'csharp' (.NET), 'rust', or 'php'. Set it explicitly for short or truncated traces." },
                    "output": { "type": "string", "enum": ["report", "table", "json"], "default": "report", "description": "Output shape. 'report' (default) is a readable analysis: language, reported exception, root cause, first user frame, then each exception with its frames ('*' marks your code). 'table' is a Markdown table per exception. 'json' is the full structured result (chain, frames, counts)." },
                    "user_packages": { "type": "string", "default": "", "description": "Comma-separated prefixes that identify your own code, e.g. 'com.example,myapp' or 'src/'. Matched against each frame's function and file path. When set this becomes an allow-list: only matching frames count as user code. Blank (default) uses the built-in per-language framework rules (java.*, node_modules, site-packages, /gems/, /vendor/, /rustc/, System.*, ...)." },
                    "hide_framework": { "type": "boolean", "default": false, "description": "When true, omit framework/standard-library frames from the output and report how many were hidden. Default false (all frames are shown, marked user or framework)." },
                    "reverse": { "type": "boolean", "default": false, "description": "Frame order. Default false lists frames innermost first (the throw/panic site first) for every language — Python tracebacks are reversed to match. Set true to list them outermost first, i.e. in call order from the entry point." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 2000, "default": 100, "description": "Maximum frames to show per exception (1-2000, default 100). Applied after hide_framework; anything beyond it is reported as a count. Raise it for deep recursion traces." }
                },
                "required": ["trace"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
