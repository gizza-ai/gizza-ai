//! Shared helpers for gizza-ai skill blocks.
//!
//! Pulled out of the duplicated copies in `blocks/image-*` and `blocks/video-*`.
//! Each block crate depends on this via a `path = "../../block-utils"` dep.

#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use wafer_block::core_types::{Message, WaferError};
use wafer_sdk::ErrorCode;

// ---------------------------------------------------------------------------
// SkillError — replaces verbose `match { Ok=>.., Err=>return GuestResult::error(..) }`
// chains in skill block handlers. Each variant maps to a wafer ErrorCode at
// `From<SkillError> for WaferError` so callers can write idiomatic `?`-based
// code and wrap the final result with `GuestResult::error(e.into())`.
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    /// User-facing argument-validation failure (bad JSON, missing field,
    /// out-of-range value, conflicting `url`/`ref`, unsupported `format`, etc.).
    #[error("{0}")]
    InvalidArgs(String),

    /// Network response with HTTP status >= 400.
    #[error("HTTP {status} for {url}")]
    HttpStatus { status: u16, url: String },

    /// Returned content-type or attachment mime didn't match the expected class.
    #[error("expected {expected} content-type, got {actual}")]
    UnexpectedMime {
        expected: &'static str,
        actual: String,
    },

    /// Input or output exceeded its byte cap.
    #[error("{kind} too large: {bytes} bytes (cap {cap} bytes)")]
    TooLarge {
        kind: &'static str,
        bytes: usize,
        cap: usize,
    },

    /// `lookup_attachment` returned `Ok(None)`.
    #[error("no attachment found for ref {0:?}")]
    AttachmentNotFound(String),

    /// `ffmpeg-runtime` reported a non-zero exit code.
    #[error("ffmpeg failed (exit {exit}): {snippet}")]
    FfmpegExitNonZero { exit: i32, snippet: String },

    /// `serde_json::to_vec` / `from_slice` failed. Internal error (the input
    /// is host-built, not user-supplied) when serializing; an args parse
    /// failure should use `InvalidArgs` instead.
    #[error("serialize/parse failed: {0}")]
    Serialize(String),

    /// Propagated from any host call (`do_request`, `lookup_attachment`,
    /// `dispatch_ffmpeg_runtime`, etc.). `WaferError` doesn't implement
    /// `std::error::Error`, so we wrap it explicitly rather than via `#[from]`.
    #[error("{0}")]
    Wafer(WaferError),
}

impl From<WaferError> for SkillError {
    fn from(err: WaferError) -> Self {
        SkillError::Wafer(err)
    }
}

impl From<SkillError> for WaferError {
    fn from(err: SkillError) -> Self {
        match err {
            SkillError::Wafer(w) => w,
            SkillError::InvalidArgs(_) | SkillError::UnexpectedMime { .. } => {
                WaferError::new(ErrorCode::INVALID_ARGUMENT, err.to_string())
            }
            SkillError::HttpStatus { .. } => {
                WaferError::new(ErrorCode::UNAVAILABLE, err.to_string())
            }
            SkillError::TooLarge { .. } => WaferError::new(ErrorCode::OUT_OF_RANGE, err.to_string()),
            SkillError::AttachmentNotFound(_) => {
                WaferError::new(ErrorCode::NOT_FOUND, err.to_string())
            }
            SkillError::FfmpegExitNonZero { .. } | SkillError::Serialize(_) => {
                WaferError::new(ErrorCode::INTERNAL, err.to_string())
            }
        }
    }
}

/// Extension to attach a `SkillError::InvalidArgs("invalid <name> args: <e>")`
/// label to any `Result<T, impl Display>`. Used at the top of each block's
/// `run()` to label JSON parse errors with the block name.
pub trait SkillResultExt<T> {
    fn invalid_args(self, block: &str) -> Result<T, SkillError>;
}

impl<T, E: std::fmt::Display> SkillResultExt<T> for Result<T, E> {
    fn invalid_args(self, block: &str) -> Result<T, SkillError> {
        self.map_err(|e| SkillError::InvalidArgs(format!("invalid {block} args: {e}")))
    }
}

// ---------------------------------------------------------------------------
// Source enum — used by every block that accepts either `url` or `ref`.
// ---------------------------------------------------------------------------

pub enum Source {
    Url(String),
    Ref(String),
}

/// Pick a `Source` from the two optional fields. Exactly one of `url` /
/// `ref_id` must be set.
pub fn pick_source(url: Option<&str>, ref_id: Option<&str>) -> Result<Source, String> {
    match (url, ref_id) {
        (Some(u), None) => Ok(Source::Url(u.to_string())),
        (None, Some(r)) => Ok(Source::Ref(r.to_string())),
        (Some(_), Some(_)) => Err("provide exactly one of `url` or `ref`".into()),
        (None, None) => Err("`url` or `ref` is required".into()),
    }
}

// ---------------------------------------------------------------------------
// Filename derivation
// ---------------------------------------------------------------------------

/// Best-effort filename from the URL's last path segment.
///
/// Percent-decodes the segment, strips control characters and the Unicode
/// replacement char, and falls back to `default` (e.g. `"image"`, `"video"`)
/// when the result is empty. No `url` crate dependency — we do the minimum
/// manually to keep the wasm payload small.
pub fn derive_filename(url: &str, default: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let path: String = after_scheme.split('/').skip(1).collect::<Vec<_>>().join("/");
    let path = path.split('?').next().unwrap_or("");
    let path = path.split('#').next().unwrap_or("");
    let last = path.rsplit('/').next().unwrap_or("");
    let decoded = percent_decode(last);
    let cleaned: String = decoded
        .chars()
        .filter(|c| !c.is_control() && *c != '\u{FFFD}')
        .collect();
    if cleaned.is_empty() {
        default.to_string()
    } else {
        cleaned
    }
}

/// Inline percent-decoder for ASCII URLs. Skips invalid escapes silently.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Swap a filename's last `.ext` for `new_ext`. Falls back to appending if
/// `filename` has no extension.
pub fn replace_extension(filename: &str, new_ext: &str) -> String {
    let base = filename
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(filename);
    format!("{base}.{new_ext}")
}

// ---------------------------------------------------------------------------
// AssetKind — image vs video, controls MIME prefix, expected-label, kind-label,
// and default filename used by `fetch_from_url` / `load_from_attachment`.
// Pulled out of the duplicated fetch helpers in blocks/image-* and blocks/video-*.
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AssetKind {
    Image,
    Video,
}

// Methods are only consumed by the wasm-gated fetch/load functions below
// (plus tests). On host non-test builds, they look unused — silence the lint
// rather than peppering each method with cfg attributes.
#[allow(dead_code)]
impl AssetKind {
    pub(crate) fn mime_prefix(self) -> &'static str {
        match self {
            Self::Image => "image/",
            Self::Video => "video/",
        }
    }

    pub(crate) fn expected_url_label(self) -> &'static str {
        match self {
            Self::Image => "image/*",
            Self::Video => "video/*",
        }
    }

    pub(crate) fn expected_attachment_label(self) -> &'static str {
        match self {
            Self::Image => "image/* attachment",
            Self::Video => "video/* attachment",
        }
    }

    pub(crate) fn too_large_label(self) -> &'static str {
        match self {
            Self::Image => "input image",
            Self::Video => "input video",
        }
    }

    pub(crate) fn default_filename(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }
}

// ---------------------------------------------------------------------------
// Fetch / load — bytes from URL or attachment, validated to a given AssetKind.
// Both return (bytes, mime, filename).
// ---------------------------------------------------------------------------

/// GET `url`, validate `Content-Type` matches `kind`, enforce `max_bytes` against
/// both the `Content-Length` header (if present) and the body. Returns the bytes,
/// the normalized lowercase MIME (parameters stripped), and a filename derived
/// from the URL's last path segment.
///
/// wasm-only because `wafer_sdk::clients::network` is wasm-gated in the SDK.
#[cfg(target_arch = "wasm32")]
pub fn fetch_from_url(
    url: &str,
    kind: AssetKind,
    max_bytes: usize,
) -> Result<(Vec<u8>, String, String), SkillError> {
    let net = wafer_sdk::clients::network::do_request("GET", url, &HashMap::new(), None)?;
    if net.status_code >= 400 {
        return Err(SkillError::HttpStatus {
            status: net.status_code,
            url: url.to_string(),
        });
    }
    let raw_mime = net
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .and_then(|(_, vs)| vs.first().cloned())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let mime: String = raw_mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if !mime.starts_with(kind.mime_prefix()) {
        return Err(SkillError::UnexpectedMime {
            expected: kind.expected_url_label(),
            actual: mime,
        });
    }
    if let Some(cl) = net
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, vs)| vs.first())
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        if cl > max_bytes {
            return Err(SkillError::TooLarge {
                kind: kind.too_large_label(),
                bytes: cl,
                cap: max_bytes,
            });
        }
    }
    if net.body.len() > max_bytes {
        return Err(SkillError::TooLarge {
            kind: kind.too_large_label(),
            bytes: net.body.len(),
            cap: max_bytes,
        });
    }
    let filename = derive_filename(url, kind.default_filename());
    Ok((net.body, mime, filename))
}

/// Look up attachment `id`, validate its mime matches `kind`, enforce `max_bytes`.
/// Returns the bytes, the attachment's mime, and its filename (falling back to
/// `kind`'s default if the attachment carries none).
///
/// wasm-only because `wafer_sdk::lookup_attachment` is wasm-gated in the SDK.
#[cfg(target_arch = "wasm32")]
pub fn load_from_attachment(
    id: &str,
    kind: AssetKind,
    max_bytes: usize,
) -> Result<(Vec<u8>, String, String), SkillError> {
    let att = wafer_sdk::lookup_attachment(id)
        .map_err(|e| SkillError::Serialize(e.to_string()))?
        .ok_or_else(|| SkillError::AttachmentNotFound(id.to_string()))?;
    if !att.mime.starts_with(kind.mime_prefix()) {
        return Err(SkillError::UnexpectedMime {
            expected: kind.expected_attachment_label(),
            actual: att.mime,
        });
    }
    if att.bytes.len() > max_bytes {
        return Err(SkillError::TooLarge {
            kind: kind.too_large_label(),
            bytes: att.bytes.len(),
            cap: max_bytes,
        });
    }
    let filename = att
        .filename
        .unwrap_or_else(|| kind.default_filename().to_string());
    Ok((att.bytes, att.mime, filename))
}

// ---------------------------------------------------------------------------
// MIME → extension
// ---------------------------------------------------------------------------

/// Map a top-level MIME class to a default display filename: `image/*` →
/// `"image"`, `video/*` → `"video"`, everything else → `"file"`. Used when
/// a user upload arrives without a filename of its own.
pub fn default_filename_for_mime(mime: &str) -> &'static str {
    if mime.starts_with("image/") {
        "image"
    } else if mime.starts_with("video/") {
        "video"
    } else {
        "file"
    }
}

/// Reject a quality value outside `1..=100`, returning a `SkillError::InvalidArgs`
/// labeled with the block name. `None` (i.e. quality omitted) is accepted.
pub fn validate_quality_1_100(quality: Option<u8>, block: &str) -> Result<(), SkillError> {
    if let Some(q) = quality {
        if !(1..=100).contains(&q) {
            return Err(SkillError::InvalidArgs(format!(
                "invalid {block} args: quality must be 1-100, got {q}"
            )));
        }
    }
    Ok(())
}

/// Map a MIME type to a file extension for ffmpeg's virtual filesystem.
/// Knows the image and video formats every gizza-ai block accepts.
pub fn mime_to_ext(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        "video/quicktime" => Some("mov"),
        "video/x-matroska" => Some("mkv"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// ffmpeg-runtime wire types + dispatch
// ---------------------------------------------------------------------------

/// Request envelope for `gizza-ai/ffmpeg-runtime` (consumer-controlled JSON
/// protocol — not a wafer-run service).
#[derive(Serialize)]
pub struct FfmpegReq {
    pub args: Vec<String>,
    pub inputs: Vec<(String, Vec<u8>)>,
    pub output: String,
}

#[derive(Deserialize)]
pub struct FfmpegResp {
    pub exit_code: i32,
    pub output: Vec<u8>,
    pub log: String,
}

/// Dispatch a request to `gizza-ai/ffmpeg-runtime` via the raw streaming ABI.
///
/// The runtime uses a consumer-controlled JSON wire format (FfmpegReq/Resp via
/// serde_json), so we hand it an opaque `Vec<u8>` payload and accept opaque
/// chunks back. The transport is the binary-transport streaming ABI; only the
/// encoding inside the chunks is JSON.
pub fn dispatch_ffmpeg_runtime(payload: &[u8]) -> Result<Vec<u8>, WaferError> {
    let msg = Message::new("ffmpeg.exec");
    let mut call = wafer_sdk::stream::CallStream::open("gizza-ai/ffmpeg-runtime", &msg)?;
    call.write_chunk(payload)?;
    let mut resp = call.finish()?;
    let mut out = Vec::new();
    while let Some(chunk) = resp.next_chunk()? {
        out.extend(chunk);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Response shapes — Envelope vs flat
// ---------------------------------------------------------------------------
//
// A skill block's response goes through `agent::dispatch::parse_skill_response`
// (see `src/blocks/agent/dispatch.rs`). The agent splits the response into
// two halves:
//   - `for_llm` — the text the LLM sees in chat history.
//   - `for_ui`  — an optional structured render hint the frontend uses to
//                 render an inline image/video.
//
// Two response shapes are valid, and the choice is mechanical:
//
//   1. **Envelope** — use this when the block produces a renderable
//      artifact (image, video, audio, anything the UI should display
//      inline). The block emits a JSON object with two keys:
//        - `"_for_llm"`: a short text summary (e.g. "generated 50KB PNG
//           for prompt: a cat"). The LLM never sees the raw bytes, only
//           this summary.
//        - `"_for_ui"`:  `{data_url, mime, filename}` for inline render.
//      Implemented by the `Envelope` struct below. Used by every block
//      in `blocks/image-*` and `blocks/video-*`, plus `imagine` and
//      `image-fetch`.
//
//   2. **Flat / plain JSON** — use this when the response is structured
//      data the LLM should read directly: HTTP body text, calculator
//      result, ffmpeg log, current time, etc. Each block declares its
//      own `#[derive(Serialize)] struct Resp { … }` (or builds a
//      `serde_json::json!({…})` inline for trivial cases) and returns
//      its bytes. The agent treats the whole body as `for_llm` and sets
//      `for_ui = None`.
//      Used by `web-fetch`, `ffmpeg`, `calculator`, `clock`.
//
// The agent decision is "envelope iff the body parses as a JSON object
// with a string `_for_llm` field" — see
// `agent::dispatch::parse_skill_response`. Adding a `_for_llm` key
// without a `_for_ui` is also valid Envelope use (the UI just won't
// render anything), but in practice every renderable artifact has both.
// Do not invent a third shape — extend Envelope (add fields under
// `_for_ui` and update `ForUi`) or stay flat.

/// Render hint for the chat UI. The agent block strips this off and forwards
/// it to the frontend as `tool_result.for_ui`.
#[derive(Serialize)]
pub struct ForUi {
    pub data_url: String,
    pub mime: String,
    pub filename: String,
}

/// Response shape for skill blocks that produce a renderable artifact.
///
/// See the module-level "Response shapes" notes above for when to use this
/// vs a plain `#[derive(Serialize)]` struct.
///
/// - `_for_llm`: text summary the LLM sees (never the raw bytes).
/// - `_for_ui`:  structured render hint for the frontend (data URL + MIME
///   + filename).
#[derive(Serialize)]
pub struct Envelope {
    #[serde(rename = "_for_llm")]
    pub for_llm: String,
    #[serde(rename = "_for_ui")]
    pub for_ui: ForUi,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_filename_uses_last_path_segment() {
        assert_eq!(derive_filename("https://x.test/path/cat.png", "image"), "cat.png");
    }

    #[test]
    fn derive_filename_strips_query_and_fragment() {
        assert_eq!(
            derive_filename("https://x.test/cat.png?v=2#fragment", "image"),
            "cat.png"
        );
    }

    #[test]
    fn derive_filename_percent_decodes() {
        assert_eq!(
            derive_filename("https://x.test/my%20cat.png", "image"),
            "my cat.png"
        );
    }

    #[test]
    fn derive_filename_falls_back_to_default_when_empty() {
        assert_eq!(derive_filename("https://x.test/", "image"), "image");
        assert_eq!(derive_filename("https://x.test/", "video"), "video");
    }

    #[test]
    fn derive_filename_strips_control_chars() {
        assert_eq!(derive_filename("https://x.test/a\x01b.png", "image"), "ab.png");
    }

    #[test]
    fn pick_source_url_only() {
        assert!(matches!(
            pick_source(Some("u"), None),
            Ok(Source::Url(ref u)) if u == "u"
        ));
    }

    #[test]
    fn pick_source_ref_only() {
        assert!(matches!(
            pick_source(None, Some("call_1")),
            Ok(Source::Ref(ref r)) if r == "call_1"
        ));
    }

    #[test]
    fn pick_source_rejects_both() {
        assert!(pick_source(Some("u"), Some("r")).is_err());
    }

    #[test]
    fn pick_source_rejects_neither() {
        assert!(pick_source(None, None).is_err());
    }

    #[test]
    fn mime_to_ext_images() {
        assert_eq!(mime_to_ext("image/png"), Some("png"));
        assert_eq!(mime_to_ext("image/jpeg"), Some("jpg"));
        assert_eq!(mime_to_ext("image/webp"), Some("webp"));
    }

    #[test]
    fn mime_to_ext_videos() {
        assert_eq!(mime_to_ext("video/mp4"), Some("mp4"));
        assert_eq!(mime_to_ext("video/webm"), Some("webm"));
        assert_eq!(mime_to_ext("video/quicktime"), Some("mov"));
        assert_eq!(mime_to_ext("video/x-matroska"), Some("mkv"));
    }

    #[test]
    fn mime_to_ext_unknown() {
        assert_eq!(mime_to_ext("application/pdf"), None);
    }

    #[test]
    fn replace_extension_swaps_last_segment() {
        assert_eq!(replace_extension("cat.png", "jpg"), "cat.jpg");
        assert_eq!(replace_extension("video.foo.mp4", "webm"), "video.foo.webm");
    }

    #[test]
    fn replace_extension_appends_when_no_dot() {
        assert_eq!(replace_extension("cat", "png"), "cat.png");
    }

    #[test]
    fn default_filename_for_mime_classes() {
        assert_eq!(default_filename_for_mime("image/png"), "image");
        assert_eq!(default_filename_for_mime("image/jpeg"), "image");
        assert_eq!(default_filename_for_mime("video/mp4"), "video");
        assert_eq!(default_filename_for_mime("video/webm"), "video");
        assert_eq!(default_filename_for_mime("application/pdf"), "file");
        assert_eq!(default_filename_for_mime(""), "file");
    }

    #[test]
    fn validate_quality_1_100_accepts_none_and_valid() {
        assert!(validate_quality_1_100(None, "x").is_ok());
        assert!(validate_quality_1_100(Some(1), "x").is_ok());
        assert!(validate_quality_1_100(Some(100), "x").is_ok());
        assert!(validate_quality_1_100(Some(50), "x").is_ok());
    }

    #[test]
    fn validate_quality_1_100_rejects_zero_and_over_100() {
        let err = validate_quality_1_100(Some(0), "image-convert").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("invalid image-convert args"));
        assert!(msg.contains("got 0"));
        let err = validate_quality_1_100(Some(101), "video-transcode").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("invalid video-transcode args"));
        assert!(msg.contains("got 101"));
    }

    #[test]
    fn skill_error_invalid_args_maps_to_invalid_argument() {
        let we: WaferError = SkillError::InvalidArgs("bad".into()).into();
        assert_eq!(we.code, ErrorCode::INVALID_ARGUMENT);
        assert_eq!(we.message, "bad");
    }

    #[test]
    fn skill_error_too_large_maps_to_out_of_range() {
        let we: WaferError = SkillError::TooLarge {
            kind: "input",
            bytes: 1,
            cap: 2,
        }
        .into();
        assert_eq!(we.code, ErrorCode::OUT_OF_RANGE);
    }

    #[test]
    fn skill_error_attachment_not_found_maps_to_not_found() {
        let we: WaferError = SkillError::AttachmentNotFound("call_1".into()).into();
        assert_eq!(we.code, ErrorCode::NOT_FOUND);
    }

    #[test]
    fn skill_error_wafer_passes_through() {
        let inner = WaferError::new(ErrorCode::UNAVAILABLE, "underlying");
        let we: WaferError = SkillError::Wafer(inner).into();
        assert_eq!(we.code, ErrorCode::UNAVAILABLE);
        assert_eq!(we.message, "underlying");
    }

    #[test]
    fn asset_kind_image_labels() {
        let k = AssetKind::Image;
        assert_eq!(k.mime_prefix(), "image/");
        assert_eq!(k.expected_url_label(), "image/*");
        assert_eq!(k.expected_attachment_label(), "image/* attachment");
        assert_eq!(k.too_large_label(), "input image");
        assert_eq!(k.default_filename(), "image");
    }

    #[test]
    fn asset_kind_video_labels() {
        let k = AssetKind::Video;
        assert_eq!(k.mime_prefix(), "video/");
        assert_eq!(k.expected_url_label(), "video/*");
        assert_eq!(k.expected_attachment_label(), "video/* attachment");
        assert_eq!(k.too_large_label(), "input video");
        assert_eq!(k.default_filename(), "video");
    }

    #[test]
    fn invalid_args_helper_prefixes_with_block_name() {
        let r: Result<(), _> = Err("bad json");
        let err = r.invalid_args("image-resize").unwrap_err();
        assert!(matches!(
            err,
            SkillError::InvalidArgs(ref s) if s == "invalid image-resize args: bad json"
        ));
    }
}
