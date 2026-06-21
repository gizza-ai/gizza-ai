//! gizza-ai/media-info — report an audio or video file's container, codec(s),
//! duration, sample rate, channels, bit depth, and overall bitrate from its
//! bytes, using the pure-Rust `symphonia` demuxers (no decoding, no ffmpeg).
//!
//! Pipeline: resolve the media source (URL/ref) → `core::info` (demux + inspect)
//! → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a media-file → text report fits neither the
//! pure-text page nor the ffmpeg file→media page shape — the F3 "no-page
//! file-input" pattern, like detect-file-type / image-info).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct TrackResp {
    index: usize,
    codec: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channels: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bits_per_sample: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_seconds: Option<f64>,
}

#[derive(Serialize)]
struct Resp {
    container: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    overall_bitrate_kbps: Option<u32>,
    track_count: usize,
    tracks: Vec<TrackResp>,
    /// File size in bytes.
    bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf` — the media file arrives via URL
/// fetch or an attachment ref. No other parameters: inspection is automatic.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MediaInfo;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/media-info",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Report an audio or video file's container, codecs, duration, and stream metadata",
    requires = ["wafer-run/network"],
    skill(
        description = "Inspect an audio or video file and report its container format, total duration, overall bitrate, and a per-track summary — codec (AAC/MP3/FLAC/ALAC/Opus/Vorbis/PCM/ADPCM/etc.), stream kind (audio or video), sample rate, channel count and layout (mono/stereo/5.1), and bit depth. Reads the bytes directly without re-encoding. Recognised containers: MP4/M4A/MOV, Matroska/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3, and AAC/ADTS. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl MediaInfo {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("media-info")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let i = gizza_ai_media_info_core::info(&bytes).map_err(SkillError::InvalidArgs)?;

    let tracks = i
        .tracks
        .into_iter()
        .map(|t| TrackResp {
            index: t.index,
            codec: t.codec,
            kind: t.kind,
            sample_rate: t.sample_rate,
            channels: t.channels,
            channel_layout: t.channel_layout,
            bits_per_sample: t.bits_per_sample,
            duration_seconds: t.duration_seconds.map(round3),
        })
        .collect();

    let resp = Resp {
        container: i.container,
        duration_seconds: i.duration_seconds.map(round3),
        duration: i.duration_human,
        overall_bitrate_kbps: i.overall_bitrate_kbps,
        track_count: i.track_count,
        tracks,
        bytes: bytes.len(),
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize media-info response: {e}")))
}

#[cfg(target_arch = "wasm32")]
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
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
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
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
