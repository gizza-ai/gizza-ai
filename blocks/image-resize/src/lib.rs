//! gizza-ai/image-resize — fetch an image URL or attachment ref, resize via ffmpeg, return envelope.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Tool-specific validation
//! (width/height required, fit=cover needs both) and the pure `core` argv
//! builders stay here. See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_resize_core::{build_argv, parse_fit, Fit};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024; // 4 MiB
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    fit: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the pre-retrofit authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("width")
                .min(1.0)
                .describe("Target width in pixels."),
        )
        .param(
            Param::integer("height")
                .min(1.0)
                .describe("Target height in pixels."),
        )
        .param(
            Param::enumv("fit", ["contain", "cover", "stretch"])
                .describe("Resize mode (default: contain)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn summary(source: &str, w: Option<u32>, h: Option<u32>, output_size: usize, mime: &str) -> String {
    match (w, h) {
        (Some(w), Some(h)) => format!("resized {source} to {w}x{h} ({output_size} {mime})"),
        (Some(w), None) => format!("resized {source} to {w} wide ({output_size} {mime})"),
        (None, Some(h)) => format!("resized {source} to {h} tall ({output_size} {mime})"),
        (None, None) => format!("resized {source} ({output_size} {mime})"),
    }
}

#[cfg(target_arch = "wasm32")]
struct ImageResize;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-resize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Resize an image fetched by URL or from a prior tool call ref",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Resize an image. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = schema_json()
    ),
)]
impl ImageResize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (tool-specific — width/height required, fit=cover needs both).
    let args: Args = serde_json::from_slice(&body).invalid_args("image-resize")?;
    if args.width.is_none() && args.height.is_none() {
        return Err(SkillError::InvalidArgs(
            "invalid image-resize args: at least one of width/height required".into(),
        ));
    }
    if args.width == Some(0) || args.height == Some(0) {
        return Err(SkillError::InvalidArgs(
            "invalid image-resize args: width/height must be > 0".into(),
        ));
    }
    let fit = parse_fit(args.fit.as_deref()).invalid_args("image-resize")?;
    if fit == Fit::Cover && (args.width.is_none() || args.height.is_none()) {
        return Err(SkillError::InvalidArgs(
            "invalid image-resize args: fit=cover requires both width and height".into(),
        ));
    }

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core).
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {mime}")))?;
    let ffmpeg_in = format!("in.{ext}");
    let ffmpeg_out = format!("out.{ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.width, args.height, fit);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope.
    let output_size = output.len();
    let dim_suffix = match (args.width, args.height) {
        (Some(w), Some(h)) => format!("-{w}x{h}"),
        _ => "-resized".to_string(),
    };
    let filename = filename_with_suffix(&in_filename, &dim_suffix, ext);
    let for_llm = summary(&in_filename, args.width, args.height, output_size, &mime);
    build_media_envelope(&output, &mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. `to_schema_json`
    /// now emits `additionalProperties: false` uniformly (image-resize's authored
    /// schema lacked it — added below as intentional uniform hardening). The
    /// `url`/`ref` property descriptions are centralized in `to_schema_json`, so
    /// the expected JSON uses that shared wording.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "width":  { "type": "integer", "minimum": 1, "description": "Target width in pixels." },
                    "height": { "type": "integer", "minimum": 1, "description": "Target height in pixels." },
                    "fit":    { "type": "string", "enum": ["contain", "cover", "stretch"], "description": "Resize mode (default: contain)." }
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
    fn output_filename_uses_dim_suffix_when_both_given() {
        assert_eq!(
            filename_with_suffix("cat.png", "-640x480", "png"),
            "cat-640x480.png"
        );
    }

    #[test]
    fn output_filename_uses_resized_suffix_when_one_dim() {
        assert_eq!(
            filename_with_suffix("cat.png", "-resized", "png"),
            "cat-resized.png"
        );
    }
}
