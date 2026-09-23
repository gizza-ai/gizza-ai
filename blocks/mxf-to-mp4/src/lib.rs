//! gizza-ai/mxf-to-mp4 — fetch a broadcast MXF URL or attachment ref, convert it
//! into a playable MP4 via ffmpeg, and return the media envelope.
//!
//! MXF is not a container relabel job. A blanket `-c copy` writes MXF's PCM audio
//! into MP4 as `ipcm`, which almost nothing decodes (the file plays silent), and
//! broadcast MXF carries audio as discrete MONO tracks rather than one stereo
//! track. So audio is always re-encoded to AAC here, with `audio = "merge"`
//! combining channel-per-track mono audio into one stereo track. The picture is
//! re-encoded to H.264 by default, or stream-copied with `video = "rewrap"` when
//! the essence is already MP4-legal (AVC-Intra / XAVC / H.264) — no generation
//! loss on a broadcast master. See `core` for the full rationale.
//!
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI + page); the handler delegates source-resolution, ffmpeg dispatch,
//! and envelope-building to `block_utils`. The pure `core` argv builder is shared
//! with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, replace_extension, validate_quality_1_100, AssetKind, Input,
    Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, format_to_mime_and_ext, resolve_source};
use gizza_ai_mxf_to_mp4_core::{
    build_argv, parse_audio, parse_video, quality_to_crf, DEFAULT_AUDIO_BITRATE,
    DEFAULT_MERGE_TRACKS, DEFAULT_QUALITY, MAX_AUDIO_BITRATE, MAX_MERGE_TRACKS, MIN_AUDIO_BITRATE,
    MIN_MERGE_TRACKS,
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
    picture: Option<String>,
    #[serde(default)]
    quality: Option<u8>,
    #[serde(default)]
    audio: Option<String>,
    #[serde(default)]
    merge_tracks: Option<u8>,
    #[serde(default)]
    audio_bitrate: Option<u16>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("picture", ["h264", "rewrap"])
                .default("h264")
                .describe(
                    "What happens to the picture. h264 (default) = re-encode with libx264 to \
                     yuv420p 8-bit MP4 — always playable, and the only option for the MPEG-2 \
                     essence in XDCAM HD / IMX MXF or for DNxHD. rewrap = -c:v copy, moving the \
                     picture into the MP4 untouched with no generation loss; works only when the \
                     MXF already holds MP4-legal essence (AVC-Intra, XAVC, plain H.264, HEVC) \
                     and ffmpeg errors out on anything else. Audio is re-encoded either way.",
                ),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .default(75)
                .describe(
                    "Picture quality 1-100 (default 75; higher = better quality, larger file). \
                     Maps to ffmpeg's libx264 CRF: 100 = CRF 18 (visually lossless), 75 = CRF 24, \
                     1 = CRF 40. Only used when picture=h264; ignored for rewrap, which never \
                     re-encodes the picture.",
                ),
        )
        .param(
            Param::enumv("audio", ["stereo", "merge", "all", "none"])
                .default("stereo")
                .describe(
                    "How the MXF's audio tracks become MP4 audio. Always re-encoded to AAC — a \
                     stream copy would write MXF's PCM as ipcm, which browsers and most players \
                     cannot decode, so the file would play silent. stereo (default) = first track \
                     only, as one 2-channel AAC track. merge = combine the first merge_tracks \
                     tracks and downmix to stereo, which is what broadcast MXF needs because it \
                     stores audio as separate MONO tracks (one channel per track). all = keep \
                     every track as its own AAC stream at its native channel count. none = drop \
                     audio, picture-only MP4.",
                ),
        )
        .param(
            Param::integer("merge_tracks")
                .min(MIN_MERGE_TRACKS as f64)
                .max(MAX_MERGE_TRACKS as f64)
                .default(DEFAULT_MERGE_TRACKS as i64)
                .describe(
                    "How many audio tracks to combine when audio=merge (2-16, default 2). Set it \
                     to the number of discrete mono tracks the MXF carries — 2 for an L/R pair, \
                     4 or 8 for a multi-channel broadcast layout; the merged result is downmixed \
                     to stereo. Ignored unless audio=merge. Asking for more tracks than the file \
                     has is an error rather than a silent partial mix.",
                ),
        )
        .param(
            Param::integer("audio_bitrate")
                .min(MIN_AUDIO_BITRATE as f64)
                .max(MAX_AUDIO_BITRATE as f64)
                .default(DEFAULT_AUDIO_BITRATE as i64)
                .describe(
                    "AAC audio bitrate in kbps (32-320, default 192). 192 is transparent for a \
                     stereo delivery copy; drop to 96-128 for speech-only material, raise to 256 \
                     or 320 for music. Ignored when audio=none.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MxfToMp4;

// The #[wafer_block] macro emits a native registration call requiring ::new() on
// the impl; skill-style impls don't have one. Gate the struct + impl to wasm32 so
// the native unit tests can still compile.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mxf-to-mp4",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a broadcast MXF file into a playable MP4.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert a broadcast SMPTE MXF video (XDCAM, P2 AVC-Intra, XAVC, IMX/D-10, DNxHD) into a playable MP4. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). picture 'h264' (default) re-encodes the picture with libx264 at quality 1-100 (default 75, maps to CRF) and always works; picture 'rewrap' stream-copies MP4-legal essence (AVC-Intra/XAVC/H.264/HEVC) with no generation loss and errors on MPEG-2 or DNxHD. Audio is always re-encoded to AAC because copying MXF's PCM would write ipcm, which plays silent: audio 'stereo' (default) keeps the first track, 'merge' combines the first merge_tracks (2-16) channel-per-track mono tracks into one stereo track, 'all' keeps every track, 'none' drops audio. audio_bitrate is 32-320 kbps (default 192). Output is a faststart MP4; the MXF timecode track is dropped.",
        parameters = schema_json()
    ),
)]
impl MxfToMp4 {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (both enums + the three numeric ranges).
    let args: Args = serde_json::from_slice(&body).invalid_args("mxf-to-mp4")?;
    let bad = |e: String| SkillError::InvalidArgs(format!("invalid mxf-to-mp4 args: {e}"));

    let video_str = args.picture.as_deref().unwrap_or("h264");
    let video = parse_video(video_str).map_err(bad)?;
    let audio_str = args.audio.as_deref().unwrap_or("stereo");
    let audio = parse_audio(audio_str).map_err(bad)?;
    validate_quality_1_100(args.quality, "mxf-to-mp4")?;
    let crf = quality_to_crf(args.quality.unwrap_or(DEFAULT_QUALITY));

    let merge_tracks = args.merge_tracks.unwrap_or(DEFAULT_MERGE_TRACKS);
    if !(MIN_MERGE_TRACKS..=MAX_MERGE_TRACKS).contains(&merge_tracks) {
        return Err(bad(format!(
            "merge_tracks must be {MIN_MERGE_TRACKS}-{MAX_MERGE_TRACKS}, got {merge_tracks}"
        )));
    }
    let audio_bitrate = args.audio_bitrate.unwrap_or(DEFAULT_AUDIO_BITRATE);
    if !(MIN_AUDIO_BITRATE..=MAX_AUDIO_BITRATE).contains(&audio_bitrate) {
        return Err(bad(format!(
            "audio_bitrate must be {MIN_AUDIO_BITRATE}-{MAX_AUDIO_BITRATE} kbps, got {audio_bitrate}"
        )));
    }

    let (out_mime, out_ext) =
        format_to_mime_and_ext(AssetKind::Video, "mp4").expect("video/mp4 is a known format");

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output is always out.mp4.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mxf");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{out_ext}");
    let argv = build_argv(
        &ffmpeg_in,
        &ffmpeg_out,
        video,
        crf,
        audio,
        merge_tracks,
        audio_bitrate,
    );

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope.
    let output_size = output.len();
    let filename = replace_extension(&in_filename, out_ext);
    let for_llm = format!(
        "converted {in_filename} ({in_mime}) to {out_mime} — picture {video_str}, audio \
         {audio_str} ({output_size} bytes)"
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
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":           { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":           { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "picture":       { "type": "string", "enum": ["h264", "rewrap"], "default": "h264", "description": "What happens to the picture. h264 (default) = re-encode with libx264 to yuv420p 8-bit MP4 — always playable, and the only option for the MPEG-2 essence in XDCAM HD / IMX MXF or for DNxHD. rewrap = -c:v copy, moving the picture into the MP4 untouched with no generation loss; works only when the MXF already holds MP4-legal essence (AVC-Intra, XAVC, plain H.264, HEVC) and ffmpeg errors out on anything else. Audio is re-encoded either way." },
                    "quality":       { "type": "integer", "minimum": 1, "maximum": 100, "default": 75, "description": "Picture quality 1-100 (default 75; higher = better quality, larger file). Maps to ffmpeg's libx264 CRF: 100 = CRF 18 (visually lossless), 75 = CRF 24, 1 = CRF 40. Only used when picture=h264; ignored for rewrap, which never re-encodes the picture." },
                    "audio":         { "type": "string", "enum": ["stereo", "merge", "all", "none"], "default": "stereo", "description": "How the MXF's audio tracks become MP4 audio. Always re-encoded to AAC — a stream copy would write MXF's PCM as ipcm, which browsers and most players cannot decode, so the file would play silent. stereo (default) = first track only, as one 2-channel AAC track. merge = combine the first merge_tracks tracks and downmix to stereo, which is what broadcast MXF needs because it stores audio as separate MONO tracks (one channel per track). all = keep every track as its own AAC stream at its native channel count. none = drop audio, picture-only MP4." },
                    "merge_tracks":  { "type": "integer", "minimum": 2, "maximum": 16, "default": 2, "description": "How many audio tracks to combine when audio=merge (2-16, default 2). Set it to the number of discrete mono tracks the MXF carries — 2 for an L/R pair, 4 or 8 for a multi-channel broadcast layout; the merged result is downmixed to stereo. Ignored unless audio=merge. Asking for more tracks than the file has is an error rather than a silent partial mix." },
                    "audio_bitrate": { "type": "integer", "minimum": 32, "maximum": 320, "default": 192, "description": "AAC audio bitrate in kbps (32-320, default 192). 192 is transparent for a stereo delivery copy; drop to 96-128 for speech-only material, raise to 256 or 320 for music. Ignored when audio=none." }
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
        assert_eq!(replace_extension("A001C003.mxf", "mp4"), "A001C003.mp4");
        assert_eq!(replace_extension("MASTER.MXF", "mp4"), "MASTER.mp4");
    }
}
