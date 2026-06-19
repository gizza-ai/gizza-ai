//! gizza-ai/video-compress — fetch a video URL or attachment ref, shrink its
//! file size with a single-pass CRF re-encode (H.264/AAC), keep the container.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Tool-specific validation
//! (a finite `crf`) and the pure `core` argv builder stay here. See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

// The #[wafer_block] macro emits the impl gated to wasm32 (it generates a native
// registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` remain native-compilable so the drift-guard test below can
// exercise them. See image-resize for the full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_compress_core::{build_argv, clamp_crf};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    /// CRF quality knob; omitted → core default (lower = higher quality/larger).
    #[serde(default)]
    crf: Option<f64>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the pre-retrofit authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video).param(
        Param::number("crf")
            .min(18.0)
            .max(34.0)
            .describe("Quality/size knob (default 28). Lower = higher quality/larger; higher = smaller. Clamped to 18-34."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoCompress;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so the drift-guard + core unit tests still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-compress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shrink a video's file size with a single-pass CRF re-encode, keeping the format",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Shrink a video's file size by re-encoding it at a chosen quality (CRF), keeping the container format. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Single-pass H.264/AAC re-encode — higher crf = smaller file / lower quality. This is a quality knob, not a target-size guarantee (true target-byte-size needs a 2-pass encode).",
        parameters = schema_json()
    ),
)]
impl VideoCompress {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args. `crf` is validated/clamped by core; an explicit non-finite
    //    value is the only thing we reject up front (a clear user error).
    let args: Args = serde_json::from_slice(&body).invalid_args("video-compress")?;
    if let Some(c) = args.crf {
        if !c.is_finite() {
            return Err(SkillError::InvalidArgs(
                "invalid video-compress args: crf must be a finite number".into(),
            ));
        }
    }
    // The page's "unset" sentinel is 0.0; the chat schema omits `crf` instead, so
    // map None → 0.0 to hit core's default path.
    let crf_req = args.crf.unwrap_or(0.0);
    let crf = clamp_crf(crf_req);

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (core keeps the input container extension).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = build_argv(crf_req, &ffmpeg_in);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope. Output mime follows the produced extension (== input ext).
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-compressed", out_ext);
    let for_llm = format!("compressed {in_filename} at crf {crf} ({output_size} bytes {out_mime})");
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

/// Map an output container extension to its video MIME (mirrors `mime_to_ext`).
#[cfg(target_arch = "wasm32")]
fn ext_to_video_mime(ext: &str) -> &'static str {
    match ext {
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "video/mp4",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. `to_schema_json`
    /// now emits `additionalProperties: false` uniformly (video-compress's
    /// authored schema lacked it — added below as intentional uniform hardening)
    /// and centralizes the `url`/`ref` property descriptions, so the expected
    /// JSON uses that shared wording. The `crf` number bounds (18/34, whole
    /// numbers) render as JSON integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "crf": { "type": "number", "minimum": 18, "maximum": 34, "description": "Quality/size knob (default 28). Lower = higher quality/larger; higher = smaller. Clamped to 18-34." }
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
    fn output_filename_uses_compressed_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-compressed", "mp4"),
            "clip-compressed.mp4"
        );
    }
}
