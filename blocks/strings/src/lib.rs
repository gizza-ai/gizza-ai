//! gizza-ai/strings — extract printable string sequences from a binary file
//! (URL/ref), like the Unix `strings` command.
//!
//! Pipeline: resolve the source file (any bytes) → `core::extract` (ASCII /
//! UTF-16 scan) → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file→text report — the F3 no-page file-input
//! pattern, like detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_strings_core::{extract, Encoding};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_min_len")]
    min_len: u32,
    #[serde(default = "default_encoding")]
    encoding: String,
}
fn default_min_len() -> u32 {
    4
}
fn default_encoding() -> String {
    "ascii".to_string()
}

#[derive(Serialize)]
struct Resp {
    strings: Vec<String>,
    count: usize,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::integer("min_len")
                .min(1.0)
                .max(1024.0)
                .describe("Minimum run length to report (default 4), matching `strings -n`."),
        )
        .param(
            Param::enumv("encoding", ["ascii", "utf16", "all"]).default("ascii").describe(
                "Which strings to find: ascii (default), utf16 (UTF-16 LE+BE), or all.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Strings;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/strings",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract printable strings from a binary file",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract printable string sequences from a binary file, like the Unix `strings` command. Finds runs of printable characters at least min_len long (default 4). encoding=ascii (default), utf16 (UTF-16 LE+BE), or all. Returns the list of strings and a count. Provide the file as either url (HTTP/HTTPS) or ref. Runs locally — the file never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Strings {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("strings")?;
    let encoding = Encoding::parse(&args.encoding).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let found = extract(&bytes, args.min_len as usize, encoding);
    let resp = Resp {
        strings: found.strings,
        count: found.count,
        truncated: found.truncated,
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize strings response: {e}")))
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
                    "url":      { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "min_len":  { "type": "integer", "minimum": 1, "maximum": 1024, "description": "Minimum run length to report (default 4), matching `strings -n`." },
                    "encoding": { "type": "string", "enum": ["ascii", "utf16", "all"], "default": "ascii", "description": "Which strings to find: ascii (default), utf16 (UTF-16 LE+BE), or all." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
