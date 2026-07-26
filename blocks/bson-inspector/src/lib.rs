//! gizza-ai/bson-inspector — parse a BSON document into a typed tree or into
//! canonical MongoDB Extended JSON v2, without a server.
//!
//! Thin chat-skill wrapper around `gizza-ai-bson-inspector-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
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
    #[serde(default = "default_indent")]
    indent: u64,
    #[serde(default)]
    show_offsets: bool,
}

fn default_indent() -> u64 {
    2
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The BSON document bytes, encoded as a base64 or hex string (e.g. from a mongodump export or a MongoDB driver). A single top-level document is expected."),
        )
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe("How the input bytes are encoded: 'base64' (default; standard or URL-safe, padding optional) or 'hex' (whitespace, ':' and '-' separators ignored)."),
        )
        .param(
            Param::enumv("output", ["tree", "json"])
                .default("tree")
                .describe("Output shape. 'tree' (default) is an indented, typed outline of every element (name, BSON type, value); 'json' is canonical MongoDB Extended JSON v2 (typed wrappers like $oid, $date, $numberLong, $numberDecimal)."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per nesting level (0-8, default 2). In 'json' mode, 0 minifies to a single compact line."),
        )
        .param(
            Param::boolean("show_offsets")
                .default(false)
                .describe("When true, prefix each line of the 'tree' output with the byte offset where that element begins. Ignored in 'json' mode."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BsonInspector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bson-inspector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse BSON bytes into a typed tree or canonical Extended JSON v2.",
    skill(
        description = "Parse a BSON document (binary JSON, as used by MongoDB) into a human-readable form, entirely in the browser/sandbox. Give the document bytes as base64 (default) or hex in 'input' (set input_format='base64' or 'hex'). Choose output='tree' (default) for an indented, typed outline of every element — its field name, BSON type (Double, String, Document, Array, Binary, ObjectId, Boolean, DateTime, Null, Regex, DBPointer, JavaScript, Symbol, JavaScriptWithScope, Int32, Timestamp, Int64, Decimal128, MinKey, MaxKey, Undefined) and value — or output='json' for canonical MongoDB Extended JSON v2 (typed wrappers: $oid, $date, $binary, $numberInt, $numberLong, $numberDouble, $numberDecimal, $timestamp, $regularExpression, $code, $symbol, $dbPointer, $undefined, $minKey, $maxKey). indent sets spaces per level (0-8, default 2; 0 minifies the JSON). show_offsets prefixes each tree line with the element's byte offset. Field order is preserved. Returns a clear error for invalid base64/hex, a wrong document length, a missing terminator, an unknown element type, or a malformed string/element.",
        parameters = schema_json()
    )
)]
impl BsonInspector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bson-inspector", |a: Args| {
            gizza_ai_bson_inspector_core::run(
                &a.input,
                &a.input_format,
                &a.output,
                a.indent as usize,
                a.show_offsets,
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
                    "input": { "type": "string", "description": "The BSON document bytes, encoded as a base64 or hex string (e.g. from a mongodump export or a MongoDB driver). A single top-level document is expected." },
                    "input_format": { "type": "string", "enum": ["base64", "hex"], "default": "base64", "description": "How the input bytes are encoded: 'base64' (default; standard or URL-safe, padding optional) or 'hex' (whitespace, ':' and '-' separators ignored)." },
                    "output": { "type": "string", "enum": ["tree", "json"], "default": "tree", "description": "Output shape. 'tree' (default) is an indented, typed outline of every element (name, BSON type, value); 'json' is canonical MongoDB Extended JSON v2 (typed wrappers like $oid, $date, $numberLong, $numberDecimal)." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per nesting level (0-8, default 2). In 'json' mode, 0 minifies to a single compact line." },
                    "show_offsets": { "type": "boolean", "default": false, "description": "When true, prefix each line of the 'tree' output with the byte offset where that element begins. Ignored in 'json' mode." }
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
