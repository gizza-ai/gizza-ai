//! Shared ffmpeg bridge types and block implementation.
//!
//! `ExecArgs`, `ExecResult`, `FfmpegError`, `FfmpegService` (trait), and
//! `FfmpegBlock` all live here so that both the browser app (wasm32) and the
//! upcoming native CLI can register the identical block with their own
//! `FfmpegService` implementation.
//!
//! The browser-specific `BrowserFfmpegService` (which uses `#[wasm_bindgen]`
//! against `/js/ffmpeg.js`) stays in the `gizza-ai` app crate because the
//! module path is resolved relative to *that* crate's root.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wafer_block::{
    block::Block,
    context::Context,
    core_types::{ErrorCode, Message, WaferError},
    streams::{input::InputStream, output::OutputStream},
    types::BlockInfo,
};

// ---------------------------------------------------------------------------
// Output-container selection for H.264 re-encodes
// ---------------------------------------------------------------------------

/// Containers a video tool keeps when re-encoding to H.264 + AAC: they can
/// hold those codecs and the tool pages/blocks know their ext→MIME mapping.
/// Everything else switches to mp4 — webm/ogv/gif cannot hold H.264 at all
/// (ffmpeg hard-fails: WebM only allows VP8/VP9/AV1 + Vorbis/Opus), and the
/// rest (avi/ts/flv/…) won't play from a `data:` URL in a browser `<video>`.
const H264_AAC_CONTAINERS: &[&str] = &["mp4", "mov", "m4v", "mkv"];

/// Choose the output container for a tool that re-encodes video to H.264
/// (`-c:v libx264`), based on the *input* filename.
///
/// Returns `(out_ext, transcode_audio)`:
///
/// * `out_ext` — extension for `out.<ext>`. The input's container is kept
///   (canonical lowercase; matched case-insensitively) when it is one of
///   [`H264_AAC_CONTAINERS`]; any other — or a missing/empty — extension
///   becomes `"mp4"`.
/// * `transcode_audio` — `true` when the container was switched (or unknown):
///   the source's audio codec is then not guaranteed valid in the new
///   container (webm's Opus/Vorbis stream-copied into mp4 is broken, or
///   unplayable in Safari), so `-c:a copy` must become an AAC encode.
///   `false` when the input container is kept — whatever audio the input
///   carried is already valid there, so stream-copy stays safe.
pub fn h264_out_ext(in_name: &str) -> (&'static str, bool) {
    let ext = in_name
        .rsplit_once('.')
        .map(|(_, e)| e)
        .filter(|e| !e.is_empty());
    match ext {
        Some(e) => match H264_AAC_CONTAINERS.iter().copied().find(|c| c.eq_ignore_ascii_case(e)) {
            Some(kept) => (kept, false),
            None => ("mp4", true),
        },
        // No usable extension → the source container is unknown; picking mp4
        // and copying an unknown audio codec into it would be the same gamble
        // as the webm case, so re-encode the audio too.
        None => ("mp4", true),
    }
}

/// Containers a *stream-copy* tool (`-c copy`, no re-encode — e.g. video-trim)
/// can emit and that a browser `<video>` can play from a `data:` URL (each has
/// an EXT_MIME entry on the tool page). A stream copy is always valid in the
/// source's OWN container, so these are kept as-is; anything else falls back to
/// mp4 (see [`copy_out_ext`]).
const COPY_CONTAINERS: &[&str] = &["mp4", "mov", "m4v", "mkv", "webm"];

/// Choose the output container for a tool that *stream-copies* its input
/// (`-c copy`, no re-encode), based on the *input* filename.
///
/// A stream copy is always valid back into the source's own container (a VP8/
/// Vorbis WebM copies cleanly into WebM, but hard-fails into mp4), so the input
/// extension is kept (canonical lowercase, matched case-insensitively) when it
/// is one of [`COPY_CONTAINERS`]. Any other — or a missing/empty — extension
/// falls back to `"mp4"` so the tool page still resolves a MIME and renders;
/// if that fallback container can't hold the copied streams, ffmpeg surfaces a
/// clear error (the same behavior the page had before this helper existed).
pub fn copy_out_ext(in_name: &str) -> &'static str {
    let ext = in_name
        .rsplit_once('.')
        .map(|(_, e)| e)
        .filter(|e| !e.is_empty());
    match ext {
        Some(e) => COPY_CONTAINERS
            .iter()
            .copied()
            .find(|c| c.eq_ignore_ascii_case(e))
            .unwrap_or("mp4"),
        None => "mp4",
    }
}

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecArgs {
    pub args: Vec<String>,
    /// `(filename, bytes)` pairs — written to ffmpeg's virtual FS before exec.
    pub inputs: Vec<(String, Vec<u8>)>,
    /// Filename to read back after exec.
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub output: Vec<u8>,
    pub log: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    #[error("bridge serialization: {0}")]
    Serialize(String),
    #[error("bridge call returned malformed response: {0}")]
    Bridge(String),
}

// ---------------------------------------------------------------------------
// FfmpegService trait
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait FfmpegService: wafer_block::MaybeSend + wafer_block::MaybeSync + 'static {
    async fn exec(&self, args: ExecArgs) -> Result<ExecResult, FfmpegError>;
}

// ---------------------------------------------------------------------------
// FfmpegBlock
// ---------------------------------------------------------------------------

/// Block implementation for `gizza-ai/ffmpeg-runtime`.
///
/// Dispatches `msg.kind = "ffmpeg.exec"` to a configurable `FfmpegService`
/// (e.g. `BrowserFfmpegService` in the browser app, or a native implementation
/// in the CLI).
pub struct FfmpegBlock {
    service: Arc<dyn FfmpegService>,
}

impl FfmpegBlock {
    pub fn new(service: Arc<dyn FfmpegService>) -> Self {
        Self { service }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl Block for FfmpegBlock {
    fn info(&self) -> BlockInfo {
        BlockInfo::new(
            "gizza-ai/ffmpeg-runtime",
            "0.1.0",
            "ffmpeg@v1",
            "Browser-side ffmpeg bridge: exec accepts CLI args + virtual-FS inputs",
        )
    }

    async fn handle(&self, _ctx: &dyn Context, msg: Message, input: InputStream) -> OutputStream {
        if msg.kind != "ffmpeg.exec" {
            return OutputStream::error(WaferError::new(
                ErrorCode::InvalidArgument,
                format!("FfmpegBlock: unknown msg.kind {}", msg.kind),
            ));
        }

        let body = input.collect_to_bytes().await;
        let args: ExecArgs = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                return OutputStream::error(WaferError::new(
                    ErrorCode::InvalidArgument,
                    format!("FfmpegBlock: invalid request body: {e}"),
                ));
            }
        };

        match self.service.exec(args).await {
            Ok(result) => match serde_json::to_vec(&result) {
                Ok(bytes) => OutputStream::respond(bytes),
                Err(e) => OutputStream::error(WaferError::new(
                    ErrorCode::Internal,
                    format!("FfmpegBlock: serialize result: {e}"),
                )),
            },
            Err(e) => OutputStream::error(WaferError::new(
                ErrorCode::Unavailable,
                format!("ffmpeg exec failed: {e}"),
            )),
        }
    }
}

#[cfg(test)]
mod h264_out_ext_tests {
    use super::h264_out_ext;

    #[test]
    fn h264_capable_containers_are_kept_and_audio_stays_copyable() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            assert_eq!(h264_out_ext(&format!("clip.{ext}")), (ext, false), "{ext}");
        }
    }

    #[test]
    fn extensions_match_case_insensitively_and_normalize_to_lowercase() {
        assert_eq!(h264_out_ext("CLIP.MP4"), ("mp4", false));
        assert_eq!(h264_out_ext("clip.MoV"), ("mov", false));
        assert_eq!(h264_out_ext("clip.MKV"), ("mkv", false));
        assert_eq!(h264_out_ext("clip.WEBM"), ("mp4", true));
    }

    #[test]
    fn incompatible_containers_switch_to_mp4_and_reencode_audio() {
        // webm/ogv/gif can't hold H.264 at all; avi/ts/flv won't play from a
        // data: URL in a browser <video> — all switch to mp4 + AAC.
        for name in ["clip.webm", "clip.ogv", "clip.gif", "clip.avi", "clip.ts", "clip.flv"] {
            assert_eq!(h264_out_ext(name), ("mp4", true), "{name}");
        }
    }

    #[test]
    fn missing_or_empty_extension_is_mp4_with_audio_reencode() {
        assert_eq!(h264_out_ext("noext"), ("mp4", true));
        assert_eq!(h264_out_ext("trailing."), ("mp4", true));
        assert_eq!(h264_out_ext(""), ("mp4", true));
    }

    #[test]
    fn only_the_last_extension_counts() {
        assert_eq!(h264_out_ext("archive.tar.mp4"), ("mp4", false));
        assert_eq!(h264_out_ext("clip.mp4.webm"), ("mp4", true));
    }
}

#[cfg(test)]
mod copy_out_ext_tests {
    use super::copy_out_ext;

    #[test]
    fn stream_copy_keeps_the_source_container() {
        // A stream copy is valid in its own container, so each is kept as-is —
        // including webm (VP8/Vorbis → out.webm), the whole point of the fix.
        for ext in ["mp4", "mov", "m4v", "mkv", "webm"] {
            assert_eq!(copy_out_ext(&format!("clip.{ext}")), ext, "{ext}");
        }
    }

    #[test]
    fn extensions_match_case_insensitively_and_normalize_to_lowercase() {
        assert_eq!(copy_out_ext("CLIP.WEBM"), "webm");
        assert_eq!(copy_out_ext("clip.MP4"), "mp4");
        assert_eq!(copy_out_ext("clip.MoV"), "mov");
    }

    #[test]
    fn unknown_or_missing_extension_falls_back_to_mp4() {
        for name in ["clip.avi", "clip.ogv", "clip.ts", "noext", "trailing.", ""] {
            assert_eq!(copy_out_ext(name), "mp4", "{name}");
        }
    }

    #[test]
    fn only_the_last_extension_counts() {
        assert_eq!(copy_out_ext("archive.tar.webm"), "webm");
        assert_eq!(copy_out_ext("clip.webm.avi"), "mp4");
    }
}
