//! gizza-ai/image-grayscale — fetch an image URL or attachment ref, convert to
//! grayscale via ffmpeg, return envelope.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. The pure `core::plan` argv
//! builder stays shared with the page. See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_grayscale_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024; // 4 MiB
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// Single-source param descriptor → chat schema (and CLI + page). image-grayscale
/// takes only the image input (no scalar params), so the descriptor is just
/// `Input::Image`. The drift-guard test below proves the derived schema matches
/// the pre-retrofit authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageGrayscale;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-grayscale",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an image to grayscale",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Convert an image to grayscale. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = schema_json()
    ),
)]
impl ImageGrayscale {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (grayscale has no scalar params — just the image source).
    let args: Args = serde_json::from_slice(&body).invalid_args("image-grayscale")?;

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core).
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {mime}")))?;
    let ffmpeg_in = format!("in.{ext}");
    let (argv, ffmpeg_out) = plan(&ffmpeg_in).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope.
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-gray", ext);
    let for_llm = format!("converted {in_filename} to grayscale ({output_size} bytes, {mime})");
    build_media_envelope(&output, &mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. `to_schema_json`
    /// emits `additionalProperties: false` uniformly (image-grayscale's authored
    /// schema lacked it — added below as intentional uniform hardening). The
    /// `url`/`ref` property descriptions are centralized in `to_schema_json`, so
    /// the expected JSON uses that shared wording.
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
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn filename_with_gray_suffix() {
        assert_eq!(
            filename_with_suffix("cat.png", "-gray", "png"),
            "cat-gray.png"
        );
    }

    #[test]
    fn filename_with_gray_suffix_jpg() {
        assert_eq!(
            filename_with_suffix("photo.jpg", "-gray", "jpg"),
            "photo-gray.jpg"
        );
    }
}
