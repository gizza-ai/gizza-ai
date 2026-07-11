//! gizza-ai/mp4-to-m4a — fetch an MP4 URL or attachment ref, pull out its first
//! audio track and rewrap it into an `.m4a` container via ffmpeg (lossless
//! stream-copy), and return the media envelope.
//!
//! `-vn -map 0:a:0 -c:a copy` drops the video, selects only the first audio
//! stream and copies its already-compressed packets (AAC, ALAC, …) into the M4A
//! container with no re-encode: lossless, near-instant, no quality change. `.m4a`
//! is the audio-only profile of the same MP4/ISO-BMFF container the source uses,
//! so whatever audio codec the MP4 held fits unchanged — there is no transcode
//! fallback (and no params). To actually re-encode (change codec/bitrate, e.g.
//! AAC → MP3), use extract-audio-from-video or audio-convert.
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
use gizza_ai_mp4_to_m4a_core::build_argv;
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
/// only a video source (url⊕ref); the audio remux is parameterless. The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Mp4ToM4a;

// The #[wafer_block] macro emits a native registration call requiring ::new() on
// the impl; skill-style impls don't have one. Gate the struct + impl to wasm32 so
// the native unit tests can still compile.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mp4-to-m4a",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remux the audio track of an MP4 into an M4A container losslessly.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Remux the audio track of an MP4 into an M4A (.m4a) container without re-encoding. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). The first audio stream is stream-copied with -vn -map 0:a:0 -c:a copy: the video is dropped and the already-compressed audio packets (AAC, ALAC, …) are copied across losslessly — near-instant, no quality change. M4A is the audio profile of the same MP4 container, so this always succeeds. To change codec/bitrate (e.g. AAC → MP3) use extract-audio-from-video or audio-convert instead.",
        parameters = schema_json()
    ),
)]
impl Mp4ToM4a {
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
    let args: Args = serde_json::from_slice(&body).invalid_args("mp4-to-m4a")?;
    let (out_mime, out_ext) =
        format_to_mime_and_ext(AssetKind::Audio, "m4a").expect("audio/mp4 is a known format");

    // 2. Resolve source — URL fetch or attachment lookup (input is a video).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output is always out.m4a.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{out_ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope — the extracted track is audio (m4a / audio/mp4).
    let output_size = output.len();
    let filename = replace_extension(&in_filename, out_ext);
    let for_llm = format!(
        "remuxed the audio track of {in_filename} ({in_mime}) into an M4A container {out_mime} ({output_size} bytes, lossless -c:a copy)"
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
    fn output_filename_swaps_extension_to_m4a() {
        assert_eq!(replace_extension("clip.mp4", "m4a"), "clip.m4a");
        assert_eq!(replace_extension("MyMovie.M4V", "m4a"), "MyMovie.m4a");
    }
}
