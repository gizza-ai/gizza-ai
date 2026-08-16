//! gizza-ai/video-vfr-to-cfr — fetch a video URL or attachment ref and convert
//! it from variable frame rate (VFR) to constant frame rate (CFR), the timing
//! editors and NLEs expect, via ffmpeg's `-fps_mode cfr`. Optionally re-locks
//! the audio to the rebuilt timeline so it stops drifting out of sync.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. The pure argv construction
//! lives in `core` and is shared verbatim with the standalone page.

// The #[wafer_block] macro emits the impl gated to wasm32; the supporting
// imports/constants/Args type are only used inside that gate, so they appear
// "unused" under native unit tests. `descriptor()`/`schema_json()` stay
// native-compilable so the drift-guard test can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_vfr_to_cfr_core::{plan, DEFAULT_QUALITY, DEFAULT_RESYNC_AUDIO};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Target constant frame rate; omitted → `auto` (keep the source's rate).
    #[serde(default)]
    fps: Option<String>,
    /// Encode quality preset; omitted → `balanced`.
    #[serde(default)]
    quality: Option<String>,
    /// Re-lock the audio to the rebuilt timeline; omitted → true.
    #[serde(default)]
    resync_audio: Option<bool>,
}

/// Single-source param descriptor → chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv(
                "fps",
                ["auto", "23.976", "24", "25", "29.97", "30", "50", "59.94", "60"],
            )
            .default("auto")
            .describe(
                "Constant frame rate to lock the video to. Default auto = keep the source's own nominal rate and only make the timing even (safest for phone/screen recordings). Pick an explicit rate to also retime: 23.976/24 film, 25/50 PAL, 29.97/30 and 59.94/60 NTSC/web.",
            ),
        )
        .param(
            Param::enumv("quality", ["high", "balanced", "small"])
                .default("balanced")
                .describe(
                    "Encode quality for the required H.264 re-encode: high (crf 18, near-transparent, largest), balanced (crf 20, default), small (crf 24, smallest file).",
                ),
        )
        .param(
            Param::boolean("resync_audio")
                .default(true)
                .describe(
                    "Re-lock the audio track to the rebuilt video timeline (aresample async, anchored at the first frame) so it stops drifting out of sync. Default on; turn off to stream-copy the original audio bit-for-bit.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoVfrToCfr;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-vfr-to-cfr",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a variable-frame-rate video to constant frame rate",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert a variable-frame-rate (VFR) video — phone footage, screen or game capture — to constant frame rate (CFR) so it edits and stays in sync. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call), plus optional fps (auto|23.976|24|25|29.97|30|50|59.94|60, default auto = keep the source's nominal rate), quality (high|balanced|small, default balanced), and resync_audio (default true, re-locks the audio to the rebuilt timeline). ffmpeg regenerates the frame timestamps on an even grid (-fps_mode cfr), duplicating or dropping frames where the source's timing was irregular. The video is re-encoded to H.264 yuv420p; mp4/mov/m4v/mkv inputs keep their container, anything else (e.g. webm) becomes MP4.",
        parameters = schema_json()
    ),
)]
impl VideoVfrToCfr {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; unset fields fall back to the core defaults.
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-vfr-to-cfr args: {e}")))?;
    let fps = args.fps.unwrap_or_default();
    let quality = args
        .quality
        .unwrap_or_else(|| DEFAULT_QUALITY.to_string());
    let resync_audio = args.resync_audio.unwrap_or(DEFAULT_RESYNC_AUDIO);

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (core keeps the input container when it can hold
    //    H.264/AAC, otherwise switches to mp4).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&fps, &quality, resync_audio, &ffmpeg_in).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope. Output mime follows the produced extension.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-cfr", out_ext);
    let rate = if fps.trim().is_empty() || fps.trim().eq_ignore_ascii_case("auto") {
        "the source's nominal rate".to_string()
    } else {
        format!("{} fps", fps.trim())
    };
    let for_llm = format!(
        "converted {in_filename} to constant frame rate at {rate} ({output_size} bytes {out_mime})"
    );
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

    /// Drift-guard: the descriptor-derived chat schema must match this authored
    /// shape so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "fps": {
                        "type": "string",
                        "enum": ["auto", "23.976", "24", "25", "29.97", "30", "50", "59.94", "60"],
                        "default": "auto",
                        "description": "Constant frame rate to lock the video to. Default auto = keep the source's own nominal rate and only make the timing even (safest for phone/screen recordings). Pick an explicit rate to also retime: 23.976/24 film, 25/50 PAL, 29.97/30 and 59.94/60 NTSC/web."
                    },
                    "quality": {
                        "type": "string",
                        "enum": ["high", "balanced", "small"],
                        "default": "balanced",
                        "description": "Encode quality for the required H.264 re-encode: high (crf 18, near-transparent, largest), balanced (crf 20, default), small (crf 24, smallest file)."
                    },
                    "resync_audio": {
                        "type": "boolean",
                        "default": true,
                        "description": "Re-lock the audio track to the rebuilt video timeline (aresample async, anchored at the first frame) so it stops drifting out of sync. Default on; turn off to stream-copy the original audio bit-for-bit."
                    }
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
    fn output_filename_uses_cfr_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-cfr", "mp4"),
            "clip-cfr.mp4"
        );
        // A container switch is reflected in the extension.
        assert_eq!(
            filename_with_suffix("capture.webm", "-cfr", "mp4"),
            "capture-cfr.mp4"
        );
    }

    #[test]
    fn descriptor_defaults_match_core_defaults() {
        // The schema's advertised defaults must be what core actually applies.
        assert_eq!(plan("auto", DEFAULT_QUALITY, DEFAULT_RESYNC_AUDIO, "in.mp4"), plan("", "", true, "in.mp4"));
    }
}
