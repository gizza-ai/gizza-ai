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
//!
//! WebGPU graceful failure: when the page lacks WebGPU entirely,
//! `t2i-engine.js::requireWebGpuAdapter` throws with the recognisable
//! marker [`WEBGPU_UNAVAILABLE_MARKER`]. We surface that as a successful
//! flat-shape `{ "message": ... }` skill response so the chat renders a
//! plain assistant bubble explaining the limitation, instead of a raw
//! `transformers: webgpu-unavailable: ...` error chip.

// The #[wafer_block] macro emits wasm-only registration; supporting imports
// and the Args type are only used inside that impl.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, SkillError, SkillResultExt};
use serde::Deserialize;
#[cfg(target_arch = "wasm32")]
use wafer_sdk::clients::image::{ImageParams, ImageRequest};
use wafer_sdk::*;

const BACKEND_ID: &str = "browser";
const MODEL_ID: &str = "onnx-community/Janus-Pro-1B-ONNX";

/// Marker emitted by `t2i-engine.js::requireWebGpuAdapter` (in solobase's
/// `solobase-browser` crate). When this substring appears in an
/// `ImageService::generate` error, the page has no WebGPU adapter at all
/// and no amount of retrying or fallback dtype will help. Keep in sync
/// with `WEBGPU_UNAVAILABLE_MARKER` in `t2i-engine.js`.
const WEBGPU_UNAVAILABLE_MARKER: &str = "webgpu-unavailable";

const WEBGPU_UNAVAILABLE_USER_MESSAGE: &str =
    "I can't generate images here — your browser doesn't expose WebGPU. \
     On Chrome try enabling `chrome://flags/#enable-unsafe-webgpu` and \
     restart; otherwise this needs a browser and device with WebGPU support.";

#[derive(Deserialize)]
struct Args {
    prompt: String,
}

/// If `err` came from `requireWebGpuAdapter` (recognised via
/// [`WEBGPU_UNAVAILABLE_MARKER`]), return the user-facing explanation
/// the chat should render. Otherwise return `None` and let the error
/// propagate normally.
fn webgpu_unavailable_message(err: &str) -> Option<&'static str> {
    if err.contains(WEBGPU_UNAVAILABLE_MARKER) {
        Some(WEBGPU_UNAVAILABLE_USER_MESSAGE)
    } else {
        None
    }
}

/// Normalize and validate a prompt — trim surrounding whitespace and
/// reject empty / whitespace-only input. The error is the user-facing
/// `SkillError::InvalidArgs` so the agent can surface it directly.
fn validate_prompt(prompt: &str) -> Result<String, SkillError> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Err(SkillError::InvalidArgs(
            "imagine: prompt must not be empty".into(),
        ));
    }
    Ok(trimmed.to_string())
}

#[cfg(target_arch = "wasm32")]
struct Imagine;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/imagine",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an image from a text prompt",
    capabilities(callable_blocks = ["wafer-run/image"]),
    skill(
        description = "Generate an image from a text prompt. Renders inline in the chat. \
                       Requires WebGPU in the browser (uses shader-f16 when available, \
                       falls back to fp32 otherwise). Output is a PNG.",
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
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("imagine")?;
    let trimmed = validate_prompt(&args.prompt)?;

    let req = ImageRequest {
        backend_id: BACKEND_ID.to_string(),
        model: MODEL_ID.to_string(),
        prompt: trimmed.clone(),
        params: ImageParams::default(),
        extra: serde_json::Value::Null,
    };
    let resp = match wafer_sdk::clients::image::generate(&req) {
        Ok(r) => r,
        Err(e) => {
            if let Some(msg) = webgpu_unavailable_message(&e.message) {
                let body = serde_json::json!({ "message": msg });
                return serde_json::to_vec(&body)
                    .map_err(|e| SkillError::Serialize(format!("serialize message: {e}")));
            }
            return Err(SkillError::Wafer(e));
        }
    };

    let image = resp
        .images
        .into_iter()
        .next()
        .ok_or_else(|| SkillError::Serialize("imagine: backend returned no images".into()))?;

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
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_prompt_trims_surrounding_whitespace() {
        assert_eq!(validate_prompt("  a cat  ").unwrap(), "a cat");
        assert_eq!(validate_prompt("\ta cat\n").unwrap(), "a cat");
    }

    #[test]
    fn validate_prompt_rejects_empty() {
        let err = validate_prompt("").unwrap_err();
        assert!(matches!(err, SkillError::InvalidArgs(ref s) if s.contains("must not be empty")));
    }

    #[test]
    fn validate_prompt_rejects_whitespace_only() {
        for s in ["   ", "\t\t", "\n\n", " \t \n "] {
            assert!(
                matches!(validate_prompt(s), Err(SkillError::InvalidArgs(_))),
                "expected InvalidArgs for {s:?}"
            );
        }
    }

    #[test]
    fn webgpu_message_matches_marker_in_wrapped_errors() {
        // BrowserImageService wraps backend errors as
        // `ImageError::BackendError("transformers: <msg>")`, so the marker
        // appears as a substring rather than a prefix.
        let wire = "transformers: webgpu-unavailable: navigator.gpu is undefined";
        let msg = webgpu_unavailable_message(wire).expect("marker should match");
        assert!(msg.contains("WebGPU"));
        assert!(msg.contains("chrome://flags"));
    }

    #[test]
    fn webgpu_message_ignores_unrelated_errors() {
        assert!(webgpu_unavailable_message("transformers: out of memory").is_none());
        assert!(webgpu_unavailable_message("model not loaded; call load_model first").is_none());
        assert!(webgpu_unavailable_message("").is_none());
    }
}
