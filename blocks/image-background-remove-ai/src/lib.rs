//! gizza-ai/image-background-remove-ai — generic foreground extraction powered
//! by U-2-Netp in the browser.
//!
//! The standalone page owns inference because Transformers.js needs browser
//! WebGPU/WASM APIs and model caching. The WAFER block preserves the shared
//! tool schema for discovery; non-browser surfaces return a structured message
//! until wafer-run exposes a general image-segmentation service.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{Input, SourceFields, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

const MODEL_ID: &str = "BritishWerewolf/U-2-Netp";
const MODEL_REVISION: &str = "7112208dbac3a3642496c8d54e2f0f9bb3dc1dc8";

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn browser_page_response() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "error": "browser_page_required",
        "message": "AI image background removal currently runs on the standalone browser tool page, where the model can use WebGPU or WebAssembly locally."
    }))
    .expect("static browser-page response serializes")
}

#[cfg(target_arch = "wasm32")]
struct ImageBackgroundRemoveAi;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-background-remove-ai",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove an image background locally with a general foreground model",
    skill(
        description = "Remove the background from an image and return a transparent PNG cutout. The standalone browser page runs the pinned U-2-Netp model locally using WebGPU with a WebAssembly fallback; the image is not sent to an inference server. It supports prominent foreground subjects including people, products, animals, and vehicles. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ImageBackgroundRemoveAi {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        if let Err(error) = serde_json::from_slice::<Args>(&body) {
            return GuestResult::error(wafer_block::core_types::WaferError::new(
                wafer_sdk::ErrorCode::InvalidArgument,
                format!("invalid image-background-remove-ai args: {error}"),
            ));
        }
        GuestResult::respond(browser_page_response())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_a_single_image_source() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(schema["properties"]["url"]["type"], "string");
        assert_eq!(schema["properties"]["ref"]["type"], "string");
        assert_eq!(schema["oneOf"].as_array().unwrap().len(), 2);
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn non_page_response_is_structured() {
        let value: serde_json::Value = serde_json::from_slice(&browser_page_response()).unwrap();
        assert_eq!(value["error"], "browser_page_required");
        assert!(value["message"].as_str().unwrap().contains("WebGPU"));
    }

    #[test]
    fn page_metadata_stays_on_the_same_pinned_model() {
        let meta = include_str!("../page/meta.toml");
        assert!(meta.contains(&format!("id = \"{MODEL_ID}\"")));
        assert!(meta.contains(&format!("revision = \"{MODEL_REVISION}\"")));
    }
}
