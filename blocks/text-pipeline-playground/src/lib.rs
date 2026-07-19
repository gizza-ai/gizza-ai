//! gizza-ai/text-pipeline-playground — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Runs entirely inside the WASM
//! sandbox — a SAFE, declarative text pipeline (no arbitrary code execution).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_text_pipeline_playground_core::{run, OnError, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    pipeline: String,
    #[serde(default)]
    regex_mode: bool,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default = "default_limit")]
    limit: f64,
    #[serde(default = "default_on_error")]
    on_error: String,
}

fn default_limit() -> f64 {
    10_000.0
}
fn default_on_error() -> String {
    "stop".to_string()
}

const DEFAULT_PIPELINE: &str = "grep ERROR\nreplace /^\\S+ ERROR /!! /\nsort\nunique";

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to run through the pipeline, processed line by line."),
        )
        .param(
            Param::string("pipeline")
                .required()
                .default(DEFAULT_PIPELINE)
                .describe(
                    "The transform pipeline: ONE operation per line, applied top to bottom. \
Blank lines and lines starting with '#' are ignored. Operations: 'grep PATTERN' keep matching \
lines; 'reject PATTERN' drop matching lines; 'replace /old/new/' regex-replace on each line \
($1 backrefs, any delimiter e.g. replace |a|b|); 'prefix TEXT' / 'suffix TEXT' add text to each \
line; 'lower' / 'upper' change case; 'trim' strip surrounding whitespace; 'sort' (or 'sort -r' \
for descending); 'unique' drop duplicate lines; 'head N' / 'tail N' keep first/last N lines; \
'reverse' flip line order; 'split SEP' split each line into more lines (no SEP = on whitespace); \
'join SEP' merge all lines into one. Example: grep ERROR / sort / unique.",
                ),
        )
        .param(
            Param::boolean("regex_mode")
                .default(false)
                .describe(
                    "When true, 'grep' and 'reject' patterns are treated as regular expressions \
(Rust regex syntax) instead of literal substrings. 'replace' always uses regex. Default false.",
                ),
        )
        .param(
            Param::boolean("case_insensitive")
                .default(false)
                .describe("When true, all grep/reject/replace matching is case-insensitive. Default false."),
        )
        .param(
            Param::integer("limit")
                .default(10000)
                .min(1.0)
                .max(1000000.0)
                .describe(
                    "Maximum number of output lines (safety cap to keep runaway pipelines fast). \
Output is truncated to this many lines. Default 10000.",
                ),
        )
        .param(
            Param::enumv("on_error", ["stop", "skip"])
                .default("stop")
                .describe(
                    "What to do when a pipeline line cannot be parsed. 'stop' (default) returns an \
error naming the bad line; 'skip' ignores the bad line and runs the rest.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options_from(a: &Args) -> Result<Options, SkillError> {
    let limit = a.limit;
    if !(1.0..=1_000_000.0).contains(&limit) || limit.fract() != 0.0 {
        return Err(SkillError::InvalidArgs(format!(
            "limit must be a whole number between 1 and 1000000 (got {limit})"
        )));
    }
    Ok(Options {
        regex_mode: a.regex_mode,
        case_insensitive: a.case_insensitive,
        limit: limit as usize,
        on_error: OnError::parse(&a.on_error).map_err(SkillError::InvalidArgs)?,
    })
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-pipeline-playground",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Run text through a chain of transforms (grep, replace, sort, unique, head…) with a tiny safe DSL.",
    skill(
        description = "Run pasted text through a chain of safe, declarative text transforms — a browser-local, no-code-execution take on tools like Ultimate Plumber. The 'pipeline' is one operation per line, applied top to bottom: grep PATTERN (keep matching lines), reject PATTERN (drop matching lines), replace /old/new/ (regex replace on each line, $1 backrefs, any delimiter), prefix/suffix TEXT, lower, upper, trim, sort (or sort -r), unique, head N, tail N, reverse, split SEP (split each line into more lines; no SEP splits on whitespace), and join SEP (merge all lines into one). Blank lines and #comments in the pipeline are ignored. regex_mode makes grep/reject patterns regular expressions; case_insensitive folds case for all matching; limit caps output lines; on_error = stop|skip controls what happens on a malformed pipeline line. Does NOT execute Python or arbitrary code — only these fixed operations.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-pipeline-playground", |a: Args| {
            let opts = options_from(&a)?;
            run(&a.text, &a.pipeline, &opts).map_err(SkillError::InvalidArgs)
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
                    "text": { "type": "string", "description": "The text to run through the pipeline, processed line by line." },
                    "pipeline": {
                        "type": "string",
                        "default": "grep ERROR\nreplace /^\\S+ ERROR /!! /\nsort\nunique",
                        "description": "The transform pipeline: ONE operation per line, applied top to bottom. Blank lines and lines starting with '#' are ignored. Operations: 'grep PATTERN' keep matching lines; 'reject PATTERN' drop matching lines; 'replace /old/new/' regex-replace on each line ($1 backrefs, any delimiter e.g. replace |a|b|); 'prefix TEXT' / 'suffix TEXT' add text to each line; 'lower' / 'upper' change case; 'trim' strip surrounding whitespace; 'sort' (or 'sort -r' for descending); 'unique' drop duplicate lines; 'head N' / 'tail N' keep first/last N lines; 'reverse' flip line order; 'split SEP' split each line into more lines (no SEP = on whitespace); 'join SEP' merge all lines into one. Example: grep ERROR / sort / unique."
                    },
                    "regex_mode": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, 'grep' and 'reject' patterns are treated as regular expressions (Rust regex syntax) instead of literal substrings. 'replace' always uses regex. Default false."
                    },
                    "case_insensitive": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, all grep/reject/replace matching is case-insensitive. Default false."
                    },
                    "limit": {
                        "type": "integer",
                        "default": 10000,
                        "minimum": 1,
                        "maximum": 1000000,
                        "description": "Maximum number of output lines (safety cap to keep runaway pipelines fast). Output is truncated to this many lines. Default 10000."
                    },
                    "on_error": {
                        "type": "string",
                        "enum": ["stop", "skip"],
                        "default": "stop",
                        "description": "What to do when a pipeline line cannot be parsed. 'stop' (default) returns an error naming the bad line; 'skip' ignores the bad line and runs the rest."
                    }
                },
                "required": ["text", "pipeline"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
