//! gizza-ai/chunk-list — split list items into fixed-size batches.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    items: String,
    #[serde(default)]
    input_separator: String,
    #[serde(default)]
    custom_separator: String,
    #[serde(default = "default_chunk_size")]
    chunk_size: f64,
    #[serde(default)]
    output: String,
    #[serde(default = "default_true")]
    label_chunks: bool,
}

fn default_chunk_size() -> f64 {
    10.0
}
fn default_true() -> bool {
    true
}

fn parse_chunk_size(n: f64) -> Result<usize, SkillError> {
    if !n.is_finite() || n.fract() != 0.0 || n < 0.0 {
        return Err(SkillError::InvalidArgs(
            "chunk_size must be a whole number".to_string(),
        ));
    }
    Ok(n as usize)
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("items").required().describe("List text to split into batches. Paste one item per line, comma/semicolon/tab/pipe-separated items, or use custom_separator for another delimiter. Items are trimmed and blank entries are dropped."))
        .param(Param::enumv("input_separator", ["auto", "comma", "newline", "semicolon", "tab", "pipe", "custom"]).default("auto").describe("How to split the input. 'auto' (default) splits on newlines, commas, semicolons, tabs, and pipes; explicit choices split on exactly that separator; 'custom' uses custom_separator."))
        .param(Param::string("custom_separator").default("").describe("Literal separator used when input_separator is 'custom'. Supports typed escapes for newline, tab, carriage return, and backslash."))
        .param(Param::integer("chunk_size").default(10).min(1.0).max(gizza_ai_chunk_list_core::MAX_CHUNK_SIZE as f64).describe("Number of items per chunk. Must be at least 1; the final chunk contains any remainder."))
        .param(Param::enumv("output", ["plain", "json", "csv", "markdown"]).default("plain").describe("Output format: plain labelled batches, JSON arrays/records, one CSV row per chunk, or Markdown bullet lists."))
        .param(Param::boolean("label_chunks").default(true).describe("Include chunk numbers and item counts in plain/JSON/CSV/Markdown output (default true). Turn off for bare batches."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ChunkList;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/chunk-list",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split list items into fixed-size chunks for batching.",
    skill(
        description = "Split a pasted list of lines or comma/semicolon/tab/pipe/custom-separated items into fixed-size chunks for batching API calls, imports, SQL IN lists, prompts, or review queues. Items are trimmed, blank entries are dropped, chunk_size controls how many items go in each batch, and the output can be plain text, JSON, CSV, or Markdown with optional chunk labels.",
        parameters = schema_json()
    ),
)]
impl ChunkList {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "chunk-list", |a: Args| {
            gizza_ai_chunk_list_core::chunk_list(
                &a.items,
                &a.input_separator,
                &a.custom_separator,
                parse_chunk_size(a.chunk_size)?,
                &a.output,
                a.label_chunks,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type": "object",
            "properties": {
                "items": { "type": "string", "description": "List text to split into batches. Paste one item per line, comma/semicolon/tab/pipe-separated items, or use custom_separator for another delimiter. Items are trimmed and blank entries are dropped." },
                "input_separator": { "type": "string", "enum": ["auto", "comma", "newline", "semicolon", "tab", "pipe", "custom"], "default": "auto", "description": "How to split the input. 'auto' (default) splits on newlines, commas, semicolons, tabs, and pipes; explicit choices split on exactly that separator; 'custom' uses custom_separator." },
                "custom_separator": { "type": "string", "default": "", "description": "Literal separator used when input_separator is 'custom'. Supports typed escapes for newline, tab, carriage return, and backslash." },
                "chunk_size": { "type": "integer", "minimum": 1, "maximum": 1000000, "default": 10, "description": "Number of items per chunk. Must be at least 1; the final chunk contains any remainder." },
                "output": { "type": "string", "enum": ["plain", "json", "csv", "markdown"], "default": "plain", "description": "Output format: plain labelled batches, JSON arrays/records, one CSV row per chunk, or Markdown bullet lists." },
                "label_chunks": { "type": "boolean", "default": true, "description": "Include chunk numbers and item counts in plain/JSON/CSV/Markdown output (default true). Turn off for bare batches." }
            },
            "required": ["items"],
            "additionalProperties": false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
