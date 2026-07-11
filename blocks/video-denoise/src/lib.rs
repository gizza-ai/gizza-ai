//! gizza-ai/video-denoise — fetch a video URL or attachment ref and reduce
//! visual noise/grain in its PICTURE with a temporal-spatial denoise filter
//! (`hqdn3d`, default, or `nlmeans`). The picture is rewritten, so the video is
//! re-encoded to H.264 (`-crf 20`); audio is kept as-is (stream-copied), and
//! re-encoded to AAC only when the container must switch to MP4 (e.g. webm).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); source-resolution, ffmpeg dispatch, and
//! envelope-building are delegated to `block_utils`. Strength/method validation
//! and the pure argv builder live in `core`.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg can't load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_denoise_core::{plan, resolve_strength};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Denoise intensity 1–100; omitted → core default (30).
    #[serde(default)]
    strength: Option<f64>,
    #[serde(default)]
    method: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("strength")
                .min(1.0)
                .max(100.0)
                .describe("How aggressively to reduce noise/grain, 1–100 (higher removes more but risks softening/smearing fine detail). Default 30. Clamped to 1–100."),
        )
        .param(
            Param::enumv("method", ["hqdn3d", "nlmeans"])
                .default("hqdn3d")
                .describe("Denoiser: hqdn3d (temporal + spatial 3D, fast, default) or nlmeans (non-local means, spatial only, slower but preserves detail better on heavy noise)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
fn ext_to_video_mime(ext: &str) -> &'static str {
    match ext {
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "video/mp4",
    }
}

#[cfg(target_arch = "wasm32")]
struct VideoDenoise;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-denoise",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reduce visual noise/grain in a video",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Reduce visual noise/grain in a video's picture with a temporal-spatial denoise filter. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call), plus strength (1–100, higher removes more but risks softening detail; default 30) and method (hqdn3d = temporal+spatial 3D, fast, default; or nlmeans = non-local means, spatial only, slower, keeps detail on heavy noise). The picture is re-encoded to H.264 (crf 20); audio is kept as-is. mp4/mov/m4v/mkv inputs keep their container; other inputs (e.g. webm) are converted to MP4. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoDenoise {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args. Reject an explicit non-finite strength up front; unset /
    //    out-of-range is resolved by core.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-denoise")?;
    if let Some(s) = args.strength {
        if !s.is_finite() {
            return Err(SkillError::InvalidArgs(
                "invalid video-denoise args: strength must be a finite number".into(),
            ));
        }
    }
    let method = args.method.as_deref().unwrap_or("hqdn3d");
    // The page's "unset" sentinel is 0.0; the chat schema omits `strength`
    // instead, so map None → 0.0 to hit core's default path.
    let strength_req = args.strength.unwrap_or(0.0);
    let strength = resolve_strength(strength_req);

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates strength/method).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(strength_req, method, &ffmpeg_in).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-denoised", out_ext);
    let for_llm = format!(
        "denoised {in_filename} ({method}, strength {strength}) ({output_size} bytes {out_mime})"
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + strength/method), so any future
    /// change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":      { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "strength": { "type": "number", "minimum": 1, "maximum": 100, "description": "How aggressively to reduce noise/grain, 1–100 (higher removes more but risks softening/smearing fine detail). Default 30. Clamped to 1–100." },
                    "method":   { "type": "string", "enum": ["hqdn3d", "nlmeans"], "default": "hqdn3d", "description": "Denoiser: hqdn3d (temporal + spatial 3D, fast, default) or nlmeans (non-local means, spatial only, slower but preserves detail better on heavy noise)." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_denoised_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-denoised", "mp4"),
            "clip-denoised.mp4"
        );
    }
}
