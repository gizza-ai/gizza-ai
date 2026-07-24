//! gizza-ai/sd-png-metadata-reader — read Stable Diffusion generation parameters
//! (prompt, negative prompt, seed, sampler, steps, CFG, size, model/hash) from a
//! PNG's text chunks.
//!
//! Pipeline: resolve the source PNG → `core::read` (pure, byte-level chunk walk +
//! flate2 inflate + A1111 parse) → structured JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image file input + text/JSON output, the no-page
//! file-input pattern, like image-metadata-viewer / image-info).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;

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

#[cfg(target_arch = "wasm32")]
struct SdPngMetadataReader;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sd-png-metadata-reader",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Read Stable Diffusion generation parameters from a PNG's text chunks",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Read the Stable Diffusion generation parameters embedded in a PNG's text chunks (tEXt/iTXt/zTXt, including zlib-compressed ones). Parses the AUTOMATIC1111 'parameters' format into structured fields — positive prompt, negative prompt, steps, sampler, CFG scale, seed, size (split into width/height), model and model hash — and keeps every other settings key (Schedule type, VAE, Clip skip, Denoising strength, Hires upscale/steps/upscaler, Lora hashes, Version, ControlNet, …) in a params map. Also detects the generator (AUTOMATIC1111, ComfyUI, NovelAI, InvokeAI) and returns every raw text chunk (keyword + type + text) so you get both the parsed values and the raw view. Load-by-URL is supported. Returns has_sd_metadata:false (with the raw chunks) for a PNG that carries no SD metadata; errors on non-PNG input. Provide the PNG as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl SdPngMetadataReader {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("sd-png-metadata-reader")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let meta = gizza_ai_sd_png_metadata_reader_core::read(&bytes).map_err(SkillError::InvalidArgs)?;
    serde_json::to_vec(&meta).map_err(|e| {
        SkillError::Serialize(format!("serialize sd-png-metadata-reader response: {e}"))
    })
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
