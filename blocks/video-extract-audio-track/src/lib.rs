//! gizza-ai/video-extract-audio-track — fetch a video URL or attachment ref, pull
//! one audio track out of it and rewrap it into a chosen container via ffmpeg
//! (lossless stream-copy), and return the media envelope.
//!
//! `-vn -map 0:a:<track> -c:a copy` drops the video, selects a single audio
//! stream (0 = first, so language/commentary tracks are reachable), and copies
//! its already-compressed packets (AAC, Opus, ALAC, …) into the chosen container
//! with no re-encode: lossless, near-instant, no quality change. The default MKA
//! (Matroska audio) container accepts any codec so it never errors; M4A fits
//! AAC/ALAC and OGG fits Vorbis/Opus. To actually re-encode (change codec/bitrate,
//! e.g. AAC → MP3), use extract-audio-from-video or audio-convert instead.
//!
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI + page); the handler delegates source-resolution, ffmpeg dispatch,
//! and envelope-building to `block_utils`. The pure `core` argv builder is shared
//! with the standalone web page. Input is a video, output is audio, so the page is
//! `format="audio"`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, replace_extension, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_extract_audio_track_core::{parse_container, plan};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_container")]
    container: String,
    #[serde(default)]
    track: Option<u32>,
}
fn default_container() -> String {
    "mka".to_string()
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("container", ["mka", "m4a", "ogg"])
                .default("mka")
                .describe(
                    "Output container the audio stream is copied into (no re-encode): \
                     mka (Matroska audio — accepts any codec, the safe default), \
                     m4a (fits AAC/ALAC from MP4/MOV), or ogg (fits Vorbis/Opus from \
                     WebM/OGG). Pick one that matches the source codec. Default mka.",
                ),
        )
        .param(
            Param::integer("track")
                .min(0.0)
                .default(0)
                .describe(
                    "Which audio stream to extract on multi-track files (0 = the first \
                     stream). Maps ffmpeg -map 0:a:<track>. Default 0.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoExtractAudioTrack;

// The #[wafer_block] macro emits a native registration call requiring ::new() on
// the impl; skill-style impls don't have one. Gate the struct + impl to wasm32 so
// the native unit tests can still compile.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-extract-audio-track",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract a video's audio track losslessly (stream-copy, no re-encode)",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Extract (demux) one audio track from a video and save it in its original codec WITHOUT re-encoding — lossless, near-instant, no quality change. Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call). Set container='mka' (default, Matroska — accepts any codec), 'm4a' (AAC/ALAC) or 'ogg' (Vorbis/Opus), and optionally track (which audio stream, 0 = first, default 0). Runs -vn -map 0:a:<track> -c:a copy: drops the video and stream-copies the chosen audio. To change codec/bitrate (e.g. AAC → MP3) use extract-audio-from-video or audio-convert instead.",
        parameters = schema_json()
    ),
)]
impl VideoExtractAudioTrack {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (container enum + optional track index).
    let args: Args = serde_json::from_slice(&body).invalid_args("video-extract-audio-track")?;
    let container = parse_container(&args.container).map_err(|e| {
        SkillError::InvalidArgs(format!("invalid video-extract-audio-track args: {e}"))
    })?;
    let track = args.track.unwrap_or(0);

    // 2. Resolve source — URL fetch or attachment lookup (input is a video).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output uses the container ext.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&args.container, track, &ffmpeg_in)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-extract-audio-track args: {e}")))?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope — the extracted track is audio (mka / m4a / ogg).
    let output_size = output.len();
    let filename = replace_extension(&in_filename, container.ext());
    let for_llm = format!(
        "extracted audio track {track} from {in_filename} ({in_mime}) into a {} container {} ({output_size} bytes, lossless -c:a copy)",
        container.ext(),
        container.mime()
    );
    build_media_envelope(
        output.as_slice(),
        container.mime(),
        filename,
        for_llm,
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + container enum + optional track).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "container": { "type": "string", "enum": ["mka", "m4a", "ogg"], "default": "mka", "description": "Output container the audio stream is copied into (no re-encode): mka (Matroska audio — accepts any codec, the safe default), m4a (fits AAC/ALAC from MP4/MOV), or ogg (fits Vorbis/Opus from WebM/OGG). Pick one that matches the source codec. Default mka." },
                    "track":     { "type": "integer", "minimum": 0, "default": 0, "description": "Which audio stream to extract on multi-track files (0 = the first stream). Maps ffmpeg -map 0:a:<track>. Default 0." }
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
    fn output_filename_swaps_to_container_ext() {
        assert_eq!(replace_extension("clip.mp4", "mka"), "clip.mka");
        assert_eq!(replace_extension("movie.mkv", "ogg"), "movie.ogg");
        assert_eq!(replace_extension("show.mov", "m4a"), "show.m4a");
    }
}
