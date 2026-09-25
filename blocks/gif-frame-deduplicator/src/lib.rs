//! gizza-ai/gif-frame-deduplicator — drop near-duplicate consecutive frames from
//! an animated GIF and re-encode it with a clip-tuned palette. ffmpeg
//! (`mpdecimate` + `palettegen`/`paletteuse`) media skill on the shared
//! abstraction.
//!
//! The chat schema is derived from `descriptor()` (single source — same shape
//! across chat + CLI + page); the handler delegates source resolution, ffmpeg
//! dispatch and envelope building to `block_utils`. The pure argv builder and
//! param validation live in `core`, shared with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_gif_frame_deduplicator_core::{plan, DEFAULT_THRESHOLD};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024; // 32 MiB in and out

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Similarity threshold in percent; `None` → `DEFAULT_THRESHOLD` (98).
    #[serde(default)]
    threshold: Option<f64>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. The input must be an animated GIF.
    ToolDescriptor::new(Input::Image).param(
        Param::number("threshold")
            .min(0.0)
            .max(100.0)
            .default(98)
            .describe(
                "How similar two consecutive frames must be to count as duplicates, in percent 0-100 (default 98). 100 removes only essentially identical frames; lower values also remove frames that merely look alike; below ~90 the animation starts collapsing.",
            ),
    )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// ffmpeg's input filename for a resolved mime. GIF is the only supported input
/// (`mime_to_ext` has no GIF entry); anything else keeps its real extension so
/// `core::plan` can reject it by name.
fn ffmpeg_in_name(mime: &str) -> String {
    if mime.eq_ignore_ascii_case("image/gif") {
        "in.gif".to_string()
    } else {
        format!("in.{}", mime_to_ext(mime).unwrap_or("bin"))
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gif-frame-deduplicator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove near-duplicate frames from an animated GIF",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Remove near-duplicate consecutive frames from an animated GIF to shrink it. Provide the GIF as either url (HTTP/HTTPS) or ref (id from a prior tool call). Optional threshold (0-100 percent, default 98) sets how similar two frames must be to count as duplicates — 100 drops only essentially identical frames, lower values drop frames that merely look alike. Kept frames stay at their original timestamps, so the animation still runs for the same length of time, and the GIF is re-encoded with a palette built from the surviving frames (palettegen/paletteuse) for quality.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).map_err(|e| {
        SkillError::InvalidArgs(format!("invalid gif-frame-deduplicator args: {e}"))
    })?;
    let threshold = args.threshold.unwrap_or(DEFAULT_THRESHOLD);

    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let input_len = bytes.len();

    let ffmpeg_in = ffmpeg_in_name(&mime);
    let (argv, out_name) = plan(&ffmpeg_in, threshold).map_err(|e| {
        SkillError::InvalidArgs(format!("invalid gif-frame-deduplicator args: {e}"))
    })?;
    let output = dispatch_ffmpeg(argv, ffmpeg_in, bytes, out_name)?;

    let filename = filename_with_suffix(&in_name, "-dedup", "gif");
    let for_llm = format!(
        "removed duplicate frames from {in_name} at a {threshold}% similarity threshold: {input_len} → {} bytes gif",
        output.len()
    );
    build_media_envelope(&output, "image/gif", filename, for_llm, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM-facing contract is stable.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "threshold": { "type": "number", "minimum": 0, "maximum": 100, "default": 98, "description": "How similar two consecutive frames must be to count as duplicates, in percent 0-100 (default 98). 100 removes only essentially identical frames; lower values also remove frames that merely look alike; below ~90 the animation starts collapsing." }
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
    fn gif_mime_maps_to_a_gif_ffmpeg_input() {
        assert_eq!(ffmpeg_in_name("image/gif"), "in.gif");
        assert_eq!(ffmpeg_in_name("IMAGE/GIF"), "in.gif");
        // Non-GIF inputs keep their extension so plan() names the real problem.
        assert_eq!(ffmpeg_in_name("image/png"), "in.png");
        assert!(plan(&ffmpeg_in_name("image/png"), DEFAULT_THRESHOLD).is_err());
        assert!(plan(&ffmpeg_in_name("image/gif"), DEFAULT_THRESHOLD).is_ok());
    }

    #[test]
    fn missing_threshold_falls_back_to_the_default() {
        let args: Args = serde_json::from_str(r#"{ "url": "https://x/a.gif" }"#).unwrap();
        assert_eq!(args.threshold.unwrap_or(DEFAULT_THRESHOLD), 98.0);
        let args: Args =
            serde_json::from_str(r#"{ "url": "https://x/a.gif", "threshold": 92.5 }"#).unwrap();
        assert_eq!(args.threshold, Some(92.5));
    }

    #[test]
    fn output_filename_uses_gif_ext() {
        assert_eq!(
            filename_with_suffix("screen.gif", "-dedup", "gif"),
            "screen-dedup.gif"
        );
    }
}
