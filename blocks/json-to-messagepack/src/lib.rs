//! gizza-ai/json-to-messagepack — chat skill block on the shared tool abstraction.
//!
//! Encodes JSON into MessagePack bytes, rendered as hex/base64/byte-array or
//! diagnostic text. The chat schema is single-sourced from descriptor() (which
//! also drives the CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_key_order")]
    key_order: String,
    #[serde(default)]
    compact_floats: bool,
    #[serde(default = "default_spec")]
    spec: String,
    #[serde(default)]
    group: u32,
}

fn default_output() -> String {
    "hex".to_string()
}
fn default_key_order() -> String {
    "input".to_string()
}
fn default_spec() -> String {
    "new".to_string()
}

const OUTPUTS: [&str; 6] = ["hex", "base64", "bytes", "annotated", "summary", "json"];
const KEY_ORDERS: [&str; 2] = ["input", "sorted"];
const SPECS: [&str; 2] = ["new", "old"];

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("JSON value to serialize into MessagePack. Objects, arrays, strings, numbers, booleans and null are supported; input is capped at 1,000,000 UTF-8 bytes."),
        )
        .param(
            Param::enumv("output", OUTPUTS)
                .default("hex")
                .describe("Rendered output format: hex (default), base64, bytes (decimal byte array), annotated (offset/type breakdown), summary (sizes plus hex/base64), or json (machine-readable summary)."),
        )
        .param(
            Param::enumv("key_order", KEY_ORDERS)
                .default("input")
                .describe("Object key order: input preserves the JSON document order (default); sorted sorts keys by raw UTF-8 bytes for deterministic payloads."),
        )
        .param(
            Param::boolean("compact_floats")
                .default(false)
                .describe("When true, emit float32 for JSON numbers that round-trip exactly as f32; otherwise all non-integer numbers use float64. Default false."),
        )
        .param(
            Param::enumv("spec", SPECS)
                .default("new")
                .describe("MessagePack string header revision: new uses the str8 header for 32–255 byte strings; old avoids str8 for pre-2013 decoders. Default new."),
        )
        .param(
            Param::integer("group")
                .default(0)
                .min(0.0)
                .max(64.0)
                .describe("For hex-bearing outputs, insert a space every N bytes (for example 16 for dump-style grouping). Use 0 for continuous hex. Default 0; max 64."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-to-messagepack",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Serialize JSON into MessagePack and render the bytes as hex, base64 or diagnostics.",
    skill(
        description = "Serialize a JSON value into MessagePack locally. Pass input as any JSON value. output chooses hex, base64, bytes, annotated, summary or json. key_order=input preserves object order while key_order=sorted gives deterministic byte order. compact_floats=true writes float32 when lossless. spec=old avoids the MessagePack str8 header for older decoders. group inserts spaces every N bytes in hex output. Returns the rendered MessagePack bytes or diagnostics.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-to-messagepack", |a: Args| {
            let opts = gizza_ai_json_to_messagepack_core::Options {
                output: a.output,
                key_order: a.key_order,
                compact_floats: a.compact_floats,
                spec: a.spec,
                group: a.group,
            };
            gizza_ai_json_to_messagepack_core::run_with_options(&a.input, &opts)
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
            "type":"object",
            "properties":{
                "input":{"type":"string","description":"JSON value to serialize into MessagePack. Objects, arrays, strings, numbers, booleans and null are supported; input is capped at 1,000,000 UTF-8 bytes."},
                "output":{"type":"string","enum":["hex","base64","bytes","annotated","summary","json"],"default":"hex","description":"Rendered output format: hex (default), base64, bytes (decimal byte array), annotated (offset/type breakdown), summary (sizes plus hex/base64), or json (machine-readable summary)."},
                "key_order":{"type":"string","enum":["input","sorted"],"default":"input","description":"Object key order: input preserves the JSON document order (default); sorted sorts keys by raw UTF-8 bytes for deterministic payloads."},
                "compact_floats":{"type":"boolean","default":false,"description":"When true, emit float32 for JSON numbers that round-trip exactly as f32; otherwise all non-integer numbers use float64. Default false."},
                "spec":{"type":"string","enum":["new","old"],"default":"new","description":"MessagePack string header revision: new uses the str8 header for 32–255 byte strings; old avoids str8 for pre-2013 decoders. Default new."},
                "group":{"type":"integer","minimum":0,"maximum":64,"default":0,"description":"For hex-bearing outputs, insert a space every N bytes (for example 16 for dump-style grouping). Use 0 for continuous hex. Default 0; max 64."}
            },
            "required":["input"],
            "additionalProperties":false
        }"#).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(actual, authored);
    }
}
