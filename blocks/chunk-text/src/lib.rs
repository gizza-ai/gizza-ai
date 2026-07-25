//! gizza-ai/chunk-text — chat skill block on the shared tool abstraction.
//! Splits a long document into overlapping chunks by token/character/word count
//! for RAG / embedding pipelines. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill and the pure logic lives in gizza-ai-chunk-text-core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_chunk_size() -> u32 {
    500
}
fn default_overlap() -> u32 {
    50
}
fn default_unit() -> String {
    "tokens".into()
}
fn default_boundary() -> String {
    "word".into()
}
fn default_chars_per_token() -> f64 {
    4.0
}
fn default_format() -> String {
    "json".into()
}

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_chunk_size")]
    chunk_size: u32,
    #[serde(default = "default_overlap")]
    overlap: u32,
    #[serde(default = "default_unit")]
    unit: String,
    #[serde(default = "default_boundary")]
    boundary: String,
    #[serde(default = "default_chars_per_token")]
    chars_per_token: f64,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    trim: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The document text to split into chunks."),
        )
        .param(
            Param::integer("chunk_size")
                .default(500)
                .min(gizza_ai_chunk_text_core::MIN_CHUNK_SIZE as f64)
                .max(gizza_ai_chunk_text_core::MAX_CHUNK_SIZE as f64)
                .describe("Maximum size of each chunk, measured in the chosen unit. Default 500."),
        )
        .param(
            Param::integer("overlap")
                .default(50)
                .min(0.0)
                .describe("How much each chunk overlaps the previous one, in the same unit. Must be less than chunk_size. Default 50 (10% of the default size). Overlap helps preserve context across chunk boundaries for retrieval."),
        )
        .param(
            Param::enumv("unit", ["tokens", "characters", "words"])
                .default("tokens")
                .describe("How chunk_size and overlap are measured. 'tokens' (default) is an estimate of chars/chars_per_token (no real BPE tokenizer runs here — use the token-counter tool for exact counts); 'characters' counts Unicode characters; 'words' counts whitespace-separated words."),
        )
        .param(
            Param::enumv("boundary", ["word", "character", "sentence", "paragraph"])
                .default("word")
                .describe("Where a chunk is allowed to end. 'word' (default) never splits a word; 'character' is a hard cut at the exact size; 'sentence' cuts on sentence ends (. ! ?); 'paragraph' cuts on blank lines. Coarser boundaries fall back to finer ones when a single unit is larger than chunk_size (recursive splitting)."),
        )
        .param(
            Param::number("chars_per_token")
                .default(4.0)
                .min(gizza_ai_chunk_text_core::MIN_CHARS_PER_TOKEN)
                .max(gizza_ai_chunk_text_core::MAX_CHARS_PER_TOKEN)
                .describe("Approximate characters per token used to estimate token counts when unit='tokens'. Default 4.0 (a common English GPT estimate)."),
        )
        .param(
            Param::enumv("format", ["json", "jsonl", "plain"])
                .default("json")
                .describe("Output format. 'json' (default) is a pretty JSON array of records with id, text, chars, tokens, start and end char offsets; 'jsonl' is one compact JSON object per line; 'plain' is the chunk texts separated by a '---' divider."),
        )
        .param(
            Param::boolean("trim")
                .default(false)
                .describe("Trim leading/trailing whitespace from each chunk (whitespace-only chunks are dropped). Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/chunk-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a long document into overlapping chunks by token, character, or word count for RAG and embedding pipelines.",
    skill(
        description = "Split a long document into overlapping chunks for RAG / embedding pipelines. Each chunk is a contiguous substring of the source with character offsets. chunk_size and overlap are measured in unit='tokens' (default, estimated via chars_per_token), 'characters', or 'words'; overlap must be less than chunk_size. boundary controls where chunks end: 'word' (default, never splits a word), 'character' (hard cut), 'sentence', or 'paragraph', with recursive fallback to a finer boundary when a unit is larger than chunk_size. format='json' (default) returns a JSON array of {id, text, chars, tokens, start, end}; 'jsonl' is one object per line; 'plain' joins chunk texts with a '---' divider. Set trim=true to strip surrounding whitespace from each chunk.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "chunk-text", |a: Args| {
            gizza_ai_chunk_text_core::chunk(
                &a.text,
                a.chunk_size as usize,
                a.overlap as usize,
                &a.unit,
                &a.boundary,
                a.chars_per_token,
                &a.format,
                a.trim,
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
    /// schema so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The document text to split into chunks." },
                    "chunk_size": { "type": "integer", "minimum": 1, "maximum": 1000000, "default": 500, "description": "Maximum size of each chunk, measured in the chosen unit. Default 500." },
                    "overlap": { "type": "integer", "minimum": 0, "default": 50, "description": "How much each chunk overlaps the previous one, in the same unit. Must be less than chunk_size. Default 50 (10% of the default size). Overlap helps preserve context across chunk boundaries for retrieval." },
                    "unit": { "type": "string", "enum": ["tokens", "characters", "words"], "default": "tokens", "description": "How chunk_size and overlap are measured. 'tokens' (default) is an estimate of chars/chars_per_token (no real BPE tokenizer runs here — use the token-counter tool for exact counts); 'characters' counts Unicode characters; 'words' counts whitespace-separated words." },
                    "boundary": { "type": "string", "enum": ["word", "character", "sentence", "paragraph"], "default": "word", "description": "Where a chunk is allowed to end. 'word' (default) never splits a word; 'character' is a hard cut at the exact size; 'sentence' cuts on sentence ends (. ! ?); 'paragraph' cuts on blank lines. Coarser boundaries fall back to finer ones when a single unit is larger than chunk_size (recursive splitting)." },
                    "chars_per_token": { "type": "number", "minimum": 1, "maximum": 100, "default": 4.0, "description": "Approximate characters per token used to estimate token counts when unit='tokens'. Default 4.0 (a common English GPT estimate)." },
                    "format": { "type": "string", "enum": ["json", "jsonl", "plain"], "default": "json", "description": "Output format. 'json' (default) is a pretty JSON array of records with id, text, chars, tokens, start and end char offsets; 'jsonl' is one compact JSON object per line; 'plain' is the chunk texts separated by a '---' divider." },
                    "trim": { "type": "boolean", "default": false, "description": "Trim leading/trailing whitespace from each chunk (whitespace-only chunks are dropped). Default false." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
