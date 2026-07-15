//! gizza-ai/mp4-to-mkv — fetch an MP4 URL or attachment ref, rewrap it into a
//! Matroska (.mkv) container via ffmpeg (lossless stream-copy), and return the
//! media envelope.
//!
//! `-map 0 -c copy` copies every stream (video + all audio + subtitles + data)
//! into the MKV container with no re-encode: lossless, near-instant, no quality
//! change. MKV is a superset container that accepts every codec MP4 can hold, so
//! the remux always succeeds — there is no transcode fallback (and no params).
//! The point of MKV is to be able to later add soft subtitles or extra audio
//! tracks. To actually re-encode (change codec/quality), use video-transcode.
//!
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI + page); the handler delegates source-resolution, ffmpeg dispatch,
//! and envelope-building to `block_utils`. The pure `core` argv builder is shared
//! with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, replace_extension, AssetKind, Input, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, format_to_mime_and_ext, resolve_source};
use gizza_ai_mp4_to_mkv_core::build_argv;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// Single-source param descriptor → chat schema (and CLI + page). The tool takes
/// only a video source (url⊕ref); the remux is parameterless. The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Mp4ToMkv;

// The #[wafer_block] macro emits a native registration call requiring ::new() on
// the impl; skill-style impls don't have one. Gate the struct + impl to wasm32 so
// the native unit tests can still compile.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mp4-to-mkv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remux an MP4 video into a Matroska (.mkv) container losslessly.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Remux an MP4 video into a Matroska (.mkv) container without re-encoding. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Every stream (video, all audio tracks, subtitles, data) is stream-copied with -map 0 -c copy: lossless, near-instant, no quality change. MKV holds every codec MP4 can, so this always succeeds — moving to MKV lets you later add soft subtitles or extra audio tracks. To change codec/quality, use video-transcode instead.",
        parameters = schema_json()
    ),
)]
impl Mp4ToMkv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (only a video source; no params).
    let args: Args = serde_json::from_slice(&body).invalid_args("mp4-to-mkv")?;
    let (out_mime, out_ext) = format_to_mime_and_ext(AssetKind::Video, "mkv")
        .expect("video/x-matroska is a known format");

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output is always out.mkv.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{out_ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope.
    let output_size = output.len();
    let filename = replace_extension(&in_filename, out_ext);
    let for_llm = format!(
        "remuxed {in_filename} ({in_mime}) into a Matroska container {out_mime} ({output_size} bytes, lossless -c copy)"
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
    /// emits `additionalProperties: false`. With no params there is no
    /// `required` key — only the url/ref properties and the media `oneOf`.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
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
    fn output_filename_swaps_extension_to_mkv() {
        assert_eq!(replace_extension("clip.mp4", "mkv"), "clip.mkv");
        assert_eq!(replace_extension("MyMovie.M4V", "mkv"), "MyMovie.mkv");
    }
}
