//! gizza-ai/gzip-size-estimator — reports the raw and gzipped byte size of text.
//!
//! Thin chat-skill wrapper around `gizza-ai-gzip-size-estimator-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`, returning the
//! human-readable size report. No host calls — runs entirely inside the WASM
//! sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    /// gzip deflate level 0–9; the core clamps the range (default 6).
    #[serde(default = "default_level")]
    level: u32,
}

fn default_level() -> u32 {
    gizza_ai_gzip_size_estimator_core::DEFAULT_LEVEL
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The code or text to measure. Its UTF-8 bytes are gzipped to estimate the transfer size."),
        )
        .param(
            // Bounds reference the core clamp so the LLM-facing schema can't
            // drift from what `estimate` actually enforces.
            Param::integer("level")
                .default(gizza_ai_gzip_size_estimator_core::DEFAULT_LEVEL as i64)
                .min(gizza_ai_gzip_size_estimator_core::MIN_LEVEL as f64)
                .max(gizza_ai_gzip_size_estimator_core::MAX_LEVEL as f64)
                .describe("gzip deflate level, 0 (none) to 9 (best). Default 6 matches the gzip/zlib default; servers usually use 6. Higher levels shrink a bit more at more CPU."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gzip-size-estimator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Estimate the raw and gzipped byte size of code or text.",
    skill(
        description = "Report the raw, gzipped, and brotli byte size of pasted code or text to estimate its over-the-wire transfer weight (e.g. a JS/CSS bundle served with gzip or brotli Content-Encoding). Returns the raw size, the gzipped size, bytes saved, the percent reduction, the compression ratio, and a brotli (Content-Encoding: br) comparison. Set level (0-9, default 6) to match a server's gzip level.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "gzip-size-estimator", |a: Args| {
            gizza_ai_gzip_size_estimator_core::estimate(&a.input, a.level)
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
                    "input": { "type": "string", "description": "The code or text to measure. Its UTF-8 bytes are gzipped to estimate the transfer size." },
                    "level": { "type": "integer", "minimum": 0, "maximum": 9, "default": 6, "description": "gzip deflate level, 0 (none) to 9 (best). Default 6 matches the gzip/zlib default; servers usually use 6. Higher levels shrink a bit more at more CPU." }
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
