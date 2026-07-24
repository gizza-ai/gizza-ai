//! gizza-ai/seamless-loop-video — fetch a video URL or attachment ref and make
//! it loop seamlessly by crossfading its tail back into its head, returning ONE
//! shorter, silent H.264 MP4 whose end frame flows into its start frame.
//!
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI + page); the handler delegates source-resolution, ffmpeg dispatch,
//! and envelope-building to `block_utils`. Tool-specific validation (crossfade
//! bounds, quality 1-100) and the pure `core` argv builder stay shared with the
//! page.

// The #[wafer_block] macro emits the impl gated to wasm32 (its native
// registration call needs ::new()). The supporting imports/const/Args are only
// used inside that impl, so they appear "unused" in native unit-test builds;
// descriptor()/schema_json() stay native-compilable for the drift-guard test.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, format_to_mime_and_ext, mime_to_ext, AssetKind,
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_seamless_loop_video_core::{
    build_argv, quality_to_crf, DEFAULT_CROSSFADE, DEFAULT_QUALITY, MAX_CROSSFADE,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    crossfade: Option<f64>,
    #[serde(default)]
    quality: Option<u8>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("crossfade")
                .min(0.1)
                .max(MAX_CROSSFADE)
                .default(DEFAULT_CROSSFADE)
                .describe(
                    "Crossfade (overlap) length in seconds — how long the clip's tail is blended \
                     back into its head to hide the loop join (default 0.5, range 0.1-5). The \
                     output is exactly this many seconds SHORTER than the input, and its first \
                     frame equals the source frame at (duration - crossfade), so repeating it is \
                     seamless. Longer = smoother but eats more of a short clip; keep it well below \
                     the clip's own length.",
                ),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .default(DEFAULT_QUALITY as i64)
                .describe(
                    "Encode quality 1-100 (default 75; higher = better quality, larger file). The \
                     output is always re-encoded to H.264/yuv420p MP4 (the crossfade compositing \
                     rules out a lossless stream copy). Maps to libx264 CRF (100 = visually \
                     lossless CRF 18, 75 ≈ CRF 23, 1 = small CRF 40).",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SeamlessLoopVideo;

// The #[wafer_block] macro emits a native registration call requiring ::new() on
// the impl; skill-style impls don't have one. Gate the struct + impl to wasm32 so
// the native unit tests can still compile.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/seamless-loop-video",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Make a video loop seamlessly by crossfading its tail into its head.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Make a short video clip loop seamlessly: crossfade (alpha-overlay) the clip's tail back into its head so the loop join is invisible. Returns ONE clip that is `crossfade` seconds shorter, re-encoded to H.264/yuv420p MP4, whose first frame equals the source frame at (duration - crossfade) — so repeating it reads as continuous motion. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). crossfade is seconds (default 0.5, range 0.1-5). quality is 1-100 (default 75) mapped to libx264 CRF. The output is SILENT (there is no audio crossfade). Best for SHORT clips — the clip is buffered to reverse it, so very long / high-resolution inputs can exhaust memory. To then repeat the seamless clip N times or fill a duration, chain it into loop-video.",
        parameters = schema_json()
    ),
)]
impl SeamlessLoopVideo {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (crossfade > 0 finite ≤ cap; quality 1-100).
    let args: Args = serde_json::from_slice(&body).invalid_args("seamless-loop-video")?;
    let crossfade = args.crossfade.unwrap_or(DEFAULT_CROSSFADE);
    if !crossfade.is_finite() || crossfade <= 0.0 || crossfade > MAX_CROSSFADE {
        return Err(SkillError::InvalidArgs(format!(
            "invalid seamless-loop-video args: crossfade must be > 0 and <= {MAX_CROSSFADE}s, got {crossfade}"
        )));
    }
    let quality = args.quality.unwrap_or(DEFAULT_QUALITY);
    if !(1..=100).contains(&quality) {
        return Err(SkillError::InvalidArgs(format!(
            "invalid seamless-loop-video args: quality must be 1-100, got {quality}"
        )));
    }
    let crf = quality_to_crf(quality);
    let (out_mime, out_ext) =
        format_to_mime_and_ext(AssetKind::Video, "mp4").expect("video/mp4 is a known format");

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output is always out.mp4.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{out_ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, crossfade, crf);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope.
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-loop", out_ext);
    let for_llm = format!(
        "made {in_filename} loop seamlessly (crossfade {crossfade}s, silent {out_mime}, \
         {output_size} bytes)"
    );
    build_media_envelope(output.as_slice(), out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored schema. `to_schema_json` centralizes the `url`/`ref` wording and
    /// emits `additionalProperties: false`; neither param is required (both have
    /// defaults), so there is no `required` key — only the media `oneOf`.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "crossfade": { "type": "number", "minimum": 0.1, "maximum": 5, "default": 0.5, "description": "Crossfade (overlap) length in seconds — how long the clip's tail is blended back into its head to hide the loop join (default 0.5, range 0.1-5). The output is exactly this many seconds SHORTER than the input, and its first frame equals the source frame at (duration - crossfade), so repeating it is seamless. Longer = smoother but eats more of a short clip; keep it well below the clip's own length." },
                    "quality":   { "type": "integer", "minimum": 1, "maximum": 100, "default": 75, "description": "Encode quality 1-100 (default 75; higher = better quality, larger file). The output is always re-encoded to H.264/yuv420p MP4 (the crossfade compositing rules out a lossless stream copy). Maps to libx264 CRF (100 = visually lossless CRF 18, 75 ≈ CRF 23, 1 = small CRF 40)." }
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
    fn output_filename_uses_loop_suffix_and_mp4() {
        assert_eq!(filename_with_suffix("clip.webm", "-loop", "mp4"), "clip-loop.mp4");
        assert_eq!(filename_with_suffix("Scene.MOV", "-loop", "mp4"), "Scene-loop.mp4");
    }
}
