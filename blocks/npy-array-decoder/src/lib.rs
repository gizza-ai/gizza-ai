//! gizza-ai/npy-array-decoder — read a NumPy `.npy` file and report its dtype,
//! shape and values without NumPy, Python or a server.
//!
//! Thin chat-skill wrapper around `gizza-ai-npy-array-decoder-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — the
//! whole decoder is std-only Rust running inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    output: String,
    #[serde(default = "default_limit")]
    limit: u64,
    #[serde(default = "default_delimiter")]
    delimiter: String,
}

fn default_limit() -> u64 {
    gizza_ai_npy_array_decoder_core::DEFAULT_LIMIT as u64
}

fn default_delimiter() -> String {
    ",".to_string()
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The contents of a .npy file, as a base64 or hex string (e.g. `base64 -w0 array.npy`). A `data:...;base64,` prefix is accepted and ignored. Maximum 8 MiB of decoded bytes."),
        )
        .param(
            Param::enumv("input_format", ["auto", "base64", "hex"])
                .default("auto")
                .describe("How the bytes are encoded: 'auto' (default) tells base64 and hex apart from the file's own magic bytes, 'base64' (standard or URL-safe, padding optional) or 'hex' (whitespace, ':' and '-' separators ignored)."),
        )
        .param(
            Param::enumv("output", ["summary", "json", "csv", "header"])
                .default("summary")
                .describe("What to return. 'summary' (default) is a readable report — dtype, shape, order, byte layout and the values; 'json' is a JSON object with the same metadata plus a nested `data` array; 'csv' is just the values, one row per last-axis slice; 'header' is the metadata only, as JSON, with no values."),
        )
        .param(
            Param::integer("limit")
                .min(0.0)
                .max(gizza_ai_npy_array_decoder_core::MAX_LIMIT as f64)
                .default(gizza_ai_npy_array_decoder_core::DEFAULT_LIMIT as i64)
                .describe("Maximum number of values to render (1-100000; 0 uses the default 1000). Larger arrays are truncated in row-major order — 'json'/'summary' then emit a flat list and flag it with truncated=true, and 'csv' emits whole rows only. Ignored by 'header'."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator for 'csv' output: a single character such as \",\", \";\" or \"|\", or the word \"tab\". Defaults to a comma. Ignored by the other output modes."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct NpyArrayDecoder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/npy-array-decoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a NumPy .npy file: dtype, shape and values as JSON or CSV.",
    skill(
        description = "Decode a NumPy .npy array file without NumPy, Python or a server. Paste the file bytes as base64 or hex in 'input' (input_format='auto' tells them apart). Reads .npy format versions 1.0, 2.0 and 3.0, reports the dtype, shape, C/Fortran order and byte layout, and emits the values. Supported dtypes: bool, int8/16/32/64, uint8/16/32/64, float16/32/64, complex64/128, fixed-width bytes (S<n>) and fixed-width text (U<n>), little- or big-endian; Fortran-ordered data is re-indexed to row-major. Choose output='summary' (default, a readable report), 'json' (metadata plus a nested data array), 'csv' (values only, one row per last-axis slice, separator set by 'delimiter'), or 'header' (metadata only). 'limit' caps how many values are rendered (default 1000, max 100000); larger arrays are truncated and flagged. Object/pickled arrays, structured (record) dtypes, datetime64/timedelta64 and .npz archives are rejected with an explanation. Errors name what was expected: a bad magic number, an unsupported version, a malformed header dict, an unsupported dtype, or data shorter than the declared shape.",
        parameters = schema_json()
    )
)]
impl NpyArrayDecoder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "npy-array-decoder", |a: Args| {
            gizza_ai_npy_array_decoder_core::run(
                &a.input,
                &a.input_format,
                &a.output,
                a.limit as usize,
                &a.delimiter,
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
                    "input": { "type": "string", "description": "The contents of a .npy file, as a base64 or hex string (e.g. `base64 -w0 array.npy`). A `data:...;base64,` prefix is accepted and ignored. Maximum 8 MiB of decoded bytes." },
                    "input_format": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How the bytes are encoded: 'auto' (default) tells base64 and hex apart from the file's own magic bytes, 'base64' (standard or URL-safe, padding optional) or 'hex' (whitespace, ':' and '-' separators ignored)." },
                    "output": { "type": "string", "enum": ["summary", "json", "csv", "header"], "default": "summary", "description": "What to return. 'summary' (default) is a readable report — dtype, shape, order, byte layout and the values; 'json' is a JSON object with the same metadata plus a nested `data` array; 'csv' is just the values, one row per last-axis slice; 'header' is the metadata only, as JSON, with no values." },
                    "limit": { "type": "integer", "minimum": 0, "maximum": 100000, "default": 1000, "description": "Maximum number of values to render (1-100000; 0 uses the default 1000). Larger arrays are truncated in row-major order — 'json'/'summary' then emit a flat list and flag it with truncated=true, and 'csv' emits whole rows only. Ignored by 'header'." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator for 'csv' output: a single character such as \",\", \";\" or \"|\", or the word \"tab\". Defaults to a comma. Ignored by the other output modes." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_apply() {
        let a: Args = serde_json::from_str(r#"{"input":"k05VTVBZ"}"#).unwrap();
        assert_eq!(a.input_format, "");
        assert_eq!(a.output, "");
        assert_eq!(a.limit, 1000);
        assert_eq!(a.delimiter, ",");
    }
}
