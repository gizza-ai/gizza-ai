//! gizza-ai/base64-to-audio-file — decode a Base64 string (or an audio `data:`
//! URI) back into a real, downloadable audio file.
//!
//! Pure Rust (no ffmpeg, no re-encoding), so it runs on ALL backends including
//! the chat Service Worker. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` returns the standard
//! base64 download envelope like csv-to-pdf-table, so `--out` writes a playable
//! file straight to disk.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_base64_to_audio_file_core::{decode, MAX_DECODED_BYTES};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    data: String,
    filename: String,
    format: String,
    strict: bool,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            data: String::new(),
            filename: "audio".to_string(),
            format: "auto".to_string(),
            strict: true,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The Base64-encoded audio, or a full `data:audio/…;base64,…` URI. Whitespace, line breaks, wrapping quotes, the URL-safe alphabet (-_) and missing = padding are all tolerated. Decodes to at most 32 MiB."),
        )
        .param(
            Param::string("filename")
                .default("audio")
                .describe("Download name without the extension (default 'audio'); the extension follows the resolved format, e.g. audio.wav."),
        )
        .param(
            Param::enumv(
                "format",
                ["auto", "mp3", "wav", "ogg", "flac", "m4a", "aac", "webm", "aiff", "amr", "wma", "midi", "bin"],
            )
            .default("auto")
            .describe("Container of the decoded bytes. 'auto' (default) sniffs it from the magic header; naming one forces that MIME type and extension for headerless payloads, and 'bin' saves the raw bytes."),
        )
        .param(
            Param::boolean("strict")
                .default(true)
                .describe("When sniffing (format=auto), reject bytes that are not a recognized audio container instead of saving them as application/octet-stream. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Base64ToAudioFile;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/base64-to-audio-file",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode Base64 or a data: URI into a downloadable audio file",
    skill(
        description = "Decode a Base64 string — or a full data:audio/…;base64,… URI — back into a real, downloadable audio file with the right MIME type and extension. The container is sniffed from the decoded magic header (WAV, MP3 with or without an ID3 tag, Ogg, FLAC, MP4/M4A, ADTS AAC, WebM, AIFF, AMR, WMA/ASF, MIDI); set format to force one for headerless payloads, or 'bin' to save the raw bytes. Whitespace, line breaks, wrapping quotes, the URL-safe alphabet and missing padding are tolerated. Bytes are never re-encoded — use audio-convert to change codecs. Decodes up to 32 MiB, locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Base64ToAudioFile {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid base64-to-audio-file args: {e}")))?;
    let out = decode(&args.data, &args.filename, &args.format, args.strict)
        .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &out.bytes,
        &out.mime,
        out.filename.clone(),
        out.summary.clone(),
        MAX_DECODED_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// copy, so an accidental descriptor edit can't silently change the
    /// LLM-facing schema (and the page control the manifest renders from it).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data":     { "type": "string", "description": "The Base64-encoded audio, or a full `data:audio/…;base64,…` URI. Whitespace, line breaks, wrapping quotes, the URL-safe alphabet (-_) and missing = padding are all tolerated. Decodes to at most 32 MiB." },
                    "filename": { "type": "string", "default": "audio", "description": "Download name without the extension (default 'audio'); the extension follows the resolved format, e.g. audio.wav." },
                    "format":   { "type": "string", "enum": ["auto", "mp3", "wav", "ogg", "flac", "m4a", "aac", "webm", "aiff", "amr", "wma", "midi", "bin"], "default": "auto", "description": "Container of the decoded bytes. 'auto' (default) sniffs it from the magic header; naming one forces that MIME type and extension for headerless payloads, and 'bin' saves the raw bytes." },
                    "strict":   { "type": "boolean", "default": true, "description": "When sniffing (format=auto), reject bytes that are not a recognized audio container instead of saving them as application/octet-stream. Default true." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The defaults the chat schema advertises are the defaults `Args` applies
    /// when a caller omits them.
    #[test]
    fn args_defaults_match_the_advertised_schema_defaults() {
        let a: Args = serde_json::from_str(r#"{"data":"SGk="}"#).unwrap();
        assert_eq!(a.filename, "audio");
        assert_eq!(a.format, "auto");
        assert!(a.strict);
    }
}
