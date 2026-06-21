//! gizza-ai/unzip — extract the files from a .zip archive (URL/ref) and return
//! them individually.
//!
//! Pipeline: resolve the source file → `core::extract` (pure-Rust zip) → flat
//! JSON the LLM reads directly (each entry with its path, size, and inline text
//! or base64 content).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file→JSON, the F3 no-page file-input pattern,
//! like extract-tar / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_unzip_core::{extract, DEFAULT_CONTENT_BUDGET};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct Resp {
    entries: Vec<gizza_ai_unzip_core::Entry>,
    count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Unzip;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/unzip",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract the files from a .zip archive",
    requires = ["wafer-run/network"],
    skill(
        description = "Extract the files from a .zip archive and return them individually. Each regular file is listed with its path and size, and its content inline — as text when it is UTF-8, otherwise as base64. (Large files past an internal budget are listed without content.) Provide the zip as either url (HTTP/HTTPS) or ref. Runs locally — the archive never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Unzip {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("unzip")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let ex = extract(&bytes, DEFAULT_CONTENT_BUDGET).map_err(SkillError::InvalidArgs)?;
    let resp = Resp {
        entries: ex.entries,
        count: ex.count,
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize unzip response: {e}")))
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
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
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
