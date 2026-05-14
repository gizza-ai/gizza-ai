//! gizza-ai/imagine — text-to-image skill block.
//!
//! Dispatches to the `wafer-run/image` service block via the typed SDK
//! client. The browser-side `BrowserImageService` (registered in
//! `gizza-ai/src/lib.rs`) provides the actual inference via
//! transformers.js running Janus-Pro-1B on WebGPU.
//!
//! Pipeline: parse {prompt} → call wafer-sdk::clients::image::generate →
//! take the first generated image → base64-encode → emit envelope
//! `{_for_llm, _for_ui}` JSON. The agent block recognises the envelope
//! and bifurcates LLM history (sees `_for_llm`) from UI rendering (sees
//! `_for_ui` with the data: URL), matching `gizza-ai/image-fetch`.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi};
use serde::Deserialize;
use wafer_sdk::clients::image::{ImageParams, ImageRequest};
use wafer_sdk::*;

const BACKEND_ID: &str = "browser";
const MODEL_ID: &str = "onnx-community/Janus-Pro-1B-ONNX";

#[derive(Deserialize)]
struct Args {
    prompt: String,
}

struct Imagine;

#[wafer_block(
    name = "gizza-ai/imagine",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an image from a text prompt",
    capabilities(callable_blocks = ["wafer-run/image"]),
    skill(
        description = "Generate an image from a text prompt. Renders inline in the chat. \
                       Requires WebGPU with shader-f16. Output is a PNG.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "Description of the image to generate." }
            },
            "required": ["prompt"],
            "additionalProperties": false
        }"#
    ),
)]
impl Imagine {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid imagine args: {e}"),
                ));
            }
        };

        let trimmed = args.prompt.trim();
        if trimmed.is_empty() {
            return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                "imagine: prompt must not be empty".to_string(),
            ));
        }

        let req = ImageRequest {
            backend_id: BACKEND_ID.to_string(),
            model: MODEL_ID.to_string(),
            prompt: trimmed.to_string(),
            params: ImageParams::default(),
            extra: serde_json::Value::Null,
        };

        let resp = match wafer_sdk::clients::image::generate(&req) {
            Ok(r) => r,
            Err(e) => return GuestResult::error(e),
        };

        let image = match resp.images.into_iter().next() {
            Some(img) => img,
            None => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INTERNAL,
                    "imagine: backend returned no images".to_string(),
                ));
            }
        };

        let mime = if image.mime_type.is_empty() {
            "image/png".to_string()
        } else {
            image.mime_type
        };
        let byte_len = image.bytes.len();
        let encoded = B64.encode(&image.bytes);
        let data_url = format!("data:{mime};base64,{encoded}");

        let env = Envelope {
            for_llm: format!("generated {byte_len}-byte {mime} for prompt: {trimmed}"),
            for_ui: ForUi {
                data_url,
                mime,
                filename: "imagine.png".to_string(),
            },
        };

        match serde_json::to_vec(&env) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(WaferError::new(
                ErrorCode::INTERNAL,
                format!("serialize envelope: {e}"),
            )),
        }
    }
}
