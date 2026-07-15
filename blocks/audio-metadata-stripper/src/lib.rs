//! gizza-ai/audio-metadata-stripper — fetch an audio URL or attachment ref and
//! strip every embedded tag (ID3, Vorbis comments, RIFF/ASF INFO), chapters and
//! (by default) the cover-art image, WITHOUT re-encoding. Part of the
//! audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. The `cover_art` choice and
//! the pure argv builder live in `core`, shared verbatim with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (its native
// registration call requires ::new(), which skill-style impls don't have). The
// supporting imports, constants, and `Args` are only used inside that wasm-gated
// impl, so they read as "unused" under native unit tests — `descriptor()` /
// `schema_json()` stay native-compilable so the drift-guard test can run.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, AssetKind, Input, Param, SkillError, SkillResultExt,
    SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_audio_metadata_stripper_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    cover_art: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio).param(
        Param::enumv("cover_art", ["remove", "keep"])
            .default("remove")
            .describe(
                "What to do with the embedded cover-art image: remove (default) drops it along \
                 with every text tag; keep preserves the picture while still stripping all text \
                 tags and chapters.",
            ),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioMetadataStripper;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-metadata-stripper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip metadata, tags and cover art from an audio file",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Remove every embedded tag from an audio file — ID3v1/ID3v2, Vorbis comments, RIFF/ASF INFO, chapters and (by default) the cover-art image — without re-encoding, so the audio stays bit-identical and the container/codec are preserved. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). cover_art defaults to remove; set it to keep to retain the embedded picture while still stripping all text tags. Works on mp3, wav, ogg, flac and m4a.",
        parameters = schema_json()
    ),
)]
impl AudioMetadataStripper {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; the cover_art value is validated inside core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-metadata-stripper")?;
    let cover_art = args.cover_art.as_deref().unwrap_or("remove");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). The output keeps the input
    //    container, so `out.<ext>` uses the input's extension.
    let in_ext = mime_to_ext(&in_mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported audio mime: {in_mime}")))?;
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&ffmpeg_in, cover_art).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope — same container/mime as the input; filename keeps the
    //    original stem (metadata is gone, but it's the same song).
    let output_size = output.len();
    let for_llm = format!("stripped metadata from {in_filename} ({output_size} bytes)");
    build_media_envelope(&output, &in_mime, in_filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently. The `url`/`ref`
    /// property descriptions are centralized in `to_schema_json` (Audio
    /// wording), so the expected JSON uses that shared wording.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "cover_art": { "type": "string", "enum": ["remove", "keep"], "default": "remove", "description": "What to do with the embedded cover-art image: remove (default) drops it along with every text tag; keep preserves the picture while still stripping all text tags and chapters." }
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
}
