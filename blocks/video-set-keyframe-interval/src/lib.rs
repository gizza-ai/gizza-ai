//! gizza-ai/video-set-keyframe-interval — fetch a video URL or attachment ref and
//! re-encode it with a FIXED keyframe (GOP) cadence — e.g. one keyframe every 2
//! seconds — so seeking is clean and HLS/DASH packagers can cut evenly-sized
//! segments on keyframe boundaries. Output is a progressive H.264 MP4 with
//! `+faststart`.
//!
//! Distinct from `video-fragmented-mp4` (which ALWAYS writes a fragmented MP4)
//! and from `video-compress` / `video-to-h264` / `video-transcode` (which expose
//! no GOP control at all).
//!
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI + page); the handler delegates source-resolution, ffmpeg dispatch,
//! and envelope-building to `block_utils`. The pure `core` argv builder is shared
//! with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, replace_extension, AssetKind, Input, Param, SkillError, SkillResultExt,
    SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, format_to_mime_and_ext, mime_to_ext, resolve_source};
use gizza_ai_video_set_keyframe_interval_core::{
    plan, DEFAULT_INTERVAL, DEFAULT_PRESET, DEFAULT_QUALITY,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 50 * 1024 * 1024; // 50 MiB
const MAX_OUTPUT_BYTES: usize = 60 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    interval: Option<f64>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    scene_cut: bool,
    #[serde(default = "default_true")]
    closed_gop: bool,
    #[serde(default)]
    quality: Option<u32>,
    #[serde(default)]
    preset: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("interval")
                .min(0.1)
                .max(3000.0)
                .default(2.0)
                .describe(
                    "How far apart keyframes are placed, in the unit given by unit. \
                     With unit=seconds (default) the accepted range is 0.1-60 and 2 is the \
                     streaming-standard cadence; with unit=frames it is a GOP size of 1-3000 \
                     frames (e.g. 60 frames = 2 seconds at 30 fps). Smaller = faster seeking \
                     and finer segments but a larger file.",
                ),
        )
        .param(
            Param::enumv("unit", ["seconds", "frames"])
                .default("seconds")
                .describe(
                    "Unit for interval. seconds (default) places keyframes by timestamp \
                     (ffmpeg -force_key_frames), which stays correct at any frame rate \
                     including variable-frame-rate sources. frames sets the classic GOP size \
                     (ffmpeg -g / -keyint_min) and is exact for constant-frame-rate sources.",
                ),
        )
        .param(
            Param::boolean("scene_cut")
                .default(false)
                .describe(
                    "Allow the encoder to add EXTRA keyframes at scene changes. false (default) \
                     passes -sc_threshold 0 so the cadence is strictly fixed — what HLS/DASH \
                     segment alignment needs. true lets scene-cut detection insert additional \
                     keyframes, which looks better on hard cuts but breaks even spacing.",
                ),
        )
        .param(
            Param::boolean("closed_gop")
                .default(true)
                .describe(
                    "Request closed GOPs (ffmpeg -flags +cgop) so no frame references anything \
                     before its keyframe. true (default) is what seeking and bitrate switching \
                     want. false leaves open GOPs, which compress marginally better.",
                ),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(51.0)
                .default(23)
                .describe(
                    "libx264 CRF quality, 1-51 (default 23). LOWER is better quality and a \
                     bigger file: 18 is near-visually-lossless, 23 is the balanced default, \
                     28 is noticeably compressed. Setting a keyframe interval always \
                     re-encodes, so this controls the quality cost. True-lossless CRF 0 is \
                     excluded because it blows past the output size cap.",
                ),
        )
        .param(
            Param::enumv(
                "preset",
                [
                    "ultrafast",
                    "superfast",
                    "veryfast",
                    "faster",
                    "fast",
                    "medium",
                    "slow",
                    "slower",
                    "veryslow",
                ],
            )
            .default("medium")
            .describe(
                "libx264 speed/compression preset (default medium). Slower presets produce a \
                 smaller file at the same CRF but take longer; the keyframe cadence is \
                 identical at every preset.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoSetKeyframeInterval;

// The #[wafer_block] macro emits a native registration call requiring ::new() on
// the impl; skill-style impls don't have one. Gate the struct + impl to wasm32 so
// the native unit tests can still compile.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-set-keyframe-interval",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Re-encode a video with a fixed keyframe (GOP) interval.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Re-encode a video with a FIXED keyframe (GOP) interval — e.g. one keyframe every 2 seconds — so players seek cleanly and HLS/DASH packagers can cut evenly-sized segments on keyframe boundaries. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). interval is the cadence (default 2) in the unit given by unit: seconds (default, 0.1-60, placed by timestamp so it is correct even for variable-frame-rate sources) or frames (a GOP size of 1-3000, i.e. ffmpeg -g/-keyint_min). scene_cut=false (default) disables scene-cut detection so the spacing is strictly even; closed_gop=true (default) requests closed GOPs. quality is libx264 CRF 1-51 (default 23, lower = better) and preset is the libx264 speed preset (default medium). Output is always a re-encoded progressive H.264/AAC MP4 with +faststart — a fixed cadence cannot be applied by stream copy. For a FRAGMENTED MP4 for MSE/DASH playback use video-fragmented-mp4 instead. Note: runs on the standalone page and CLI; chat ffmpeg is unavailable.",
        parameters = schema_json()
    ),
)]
impl VideoSetKeyframeInterval {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Args → validated plan (core owns every range check).
    let args: Args = serde_json::from_slice(&body).invalid_args("video-set-keyframe-interval")?;
    let interval = args.interval.unwrap_or(DEFAULT_INTERVAL);
    let unit = args.unit.as_deref().unwrap_or("seconds").to_string();
    let quality = args.quality.unwrap_or(DEFAULT_QUALITY);
    let preset = args.preset.as_deref().unwrap_or(DEFAULT_PRESET).to_string();

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output is always out.mp4.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(
        interval,
        &unit,
        args.scene_cut,
        args.closed_gop,
        quality,
        &preset,
        &ffmpeg_in,
    )
    .map_err(|e| SkillError::InvalidArgs(format!("invalid video-set-keyframe-interval args: {e}")))?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope.
    let (out_mime, out_ext) =
        format_to_mime_and_ext(AssetKind::Video, "mp4").expect("video/mp4 is a known format");
    let output_size = output.len();
    let filename = replace_extension(&in_filename, out_ext);
    let cadence = if unit == "frames" {
        format!("{} frames", interval.round())
    } else {
        format!("{interval} s")
    };
    let for_llm = format!(
        "re-encoded {in_filename} with a keyframe every {cadence} \
         (H.264 CRF {quality} {preset}, closed_gop={}, scene_cut={}, {output_size} bytes)",
        args.closed_gop, args.scene_cut
    );
    build_media_envelope(
        output.as_slice(),
        out_mime,
        filename,
        for_llm,
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored schema. `to_schema_json` centralizes the `url`/`ref` wording and
    /// emits `additionalProperties: false`; no param is required (all have
    /// defaults), so there is no `required` key — only the media `oneOf`.
    /// NOTE the `N.0` gotcha: number defaults serialize as floats (`2.0`),
    /// integer defaults as integers (`23`).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "interval": { "type": "number", "minimum": 0.1, "maximum": 3000, "default": 2.0, "description": "How far apart keyframes are placed, in the unit given by unit. With unit=seconds (default) the accepted range is 0.1-60 and 2 is the streaming-standard cadence; with unit=frames it is a GOP size of 1-3000 frames (e.g. 60 frames = 2 seconds at 30 fps). Smaller = faster seeking and finer segments but a larger file." },
                    "unit": { "type": "string", "enum": ["seconds", "frames"], "default": "seconds", "description": "Unit for interval. seconds (default) places keyframes by timestamp (ffmpeg -force_key_frames), which stays correct at any frame rate including variable-frame-rate sources. frames sets the classic GOP size (ffmpeg -g / -keyint_min) and is exact for constant-frame-rate sources." },
                    "scene_cut": { "type": "boolean", "default": false, "description": "Allow the encoder to add EXTRA keyframes at scene changes. false (default) passes -sc_threshold 0 so the cadence is strictly fixed — what HLS/DASH segment alignment needs. true lets scene-cut detection insert additional keyframes, which looks better on hard cuts but breaks even spacing." },
                    "closed_gop": { "type": "boolean", "default": true, "description": "Request closed GOPs (ffmpeg -flags +cgop) so no frame references anything before its keyframe. true (default) is what seeking and bitrate switching want. false leaves open GOPs, which compress marginally better." },
                    "quality": { "type": "integer", "minimum": 1, "maximum": 51, "default": 23, "description": "libx264 CRF quality, 1-51 (default 23). LOWER is better quality and a bigger file: 18 is near-visually-lossless, 23 is the balanced default, 28 is noticeably compressed. Setting a keyframe interval always re-encodes, so this controls the quality cost. True-lossless CRF 0 is excluded because it blows past the output size cap." },
                    "preset": { "type": "string", "enum": ["ultrafast", "superfast", "veryfast", "faster", "fast", "medium", "slow", "slower", "veryslow"], "default": "medium", "description": "libx264 speed/compression preset (default medium). Slower presets produce a smaller file at the same CRF but take longer; the keyframe cadence is identical at every preset." }
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
    fn output_filename_swaps_extension_to_mp4() {
        assert_eq!(replace_extension("clip.webm", "mp4"), "clip.mp4");
        assert_eq!(replace_extension("Recording.MOV", "mp4"), "Recording.mp4");
    }

    #[test]
    fn defaults_match_the_descriptor() {
        assert_eq!(DEFAULT_INTERVAL, 2.0);
        assert_eq!(DEFAULT_QUALITY, 23);
        assert_eq!(DEFAULT_PRESET, "medium");
    }
}
