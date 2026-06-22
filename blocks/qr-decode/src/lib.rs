//! gizza-ai/qr-decode — read and decode the data in a QR code image (URL/ref).
//!
//! Pipeline: resolve the image source (URL/ref) → `core::run` (image + rqrr) →
//! flat JSON the LLM reads directly (the decoded text of each QR code found).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text output — the F3 no-page
//! file-input pattern, like image-info / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct Resp {
    /// Decoded text of each QR code found, in detection order.
    decoded: Vec<String>,
    /// Number of QR codes decoded.
    count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct QrDecode;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/qr-decode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode the data in a QR code image",
    requires = ["wafer-run/network"],
    skill(
        description = "Read and decode the data contained in a QR code image (PNG/JPEG/GIF/BMP/WebP). Returns the decoded text of every QR code found in the image, in detection order. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call). Runs locally — the image never leaves the device.",
        parameters = schema_json()
    ),
)]
impl QrDecode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("qr-decode")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let decoded = gizza_ai_qr_decode_core::run(&bytes).map_err(SkillError::InvalidArgs)?;
    let resp = Resp {
        count: decoded.len(),
        decoded,
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize qr-decode response: {e}")))
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
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
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
