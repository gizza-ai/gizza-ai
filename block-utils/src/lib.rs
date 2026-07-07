//! Shared helpers for gizza-ai skill blocks.
//!
//! Pulled out of the duplicated copies in `blocks/image-*` and `blocks/video-*`.
//! Each block crate depends on this via a `path = "../../block-utils"` dep.

pub mod ffmpeg;

pub mod descriptor;
pub use descriptor::*;

/// Per-call linear-memory cap (in 64 KiB wasm pages) that gizza's trusted,
/// single-user runtime grants every skill `WasmiBlock`. 1024 pages = 64 MiB.
///
/// wafer-run defaults to 256 pages / 16 MiB, which is enough for the light
/// tools but OOM-traps memory-heavy ones (e.g. the `syntect`+bundled-font
/// `code-screenshot` render needs ~24 MiB). gizza is local/trusted, so both
/// its native CLI and browser runtimes raise the cap to this value at every
/// skill load site; the hosted multi-tenant runtime keeps the 256-page
/// default. Defined here (shared by `gizza-cli` and the browser `gizza-ai`
/// crate) so the value lives in exactly one place.
pub const GIZZA_MAX_WASM_MEMORY_PAGES: u32 = 1024;

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
                WaferError::new(ErrorCode::InvalidArgument, err.to_string())
            }
            SkillError::HttpStatus { .. } => {
                WaferError::new(ErrorCode::Unavailable, err.to_string())
            }
            SkillError::TooLarge { .. } => WaferError::new(ErrorCode::OutOfRange, err.to_string()),
            SkillError::AttachmentNotFound(_) => {
                WaferError::new(ErrorCode::NotFound, err.to_string())
            }
            SkillError::FfmpegExitNonZero { .. } | SkillError::Serialize(_) => {
                WaferError::new(ErrorCode::Internal, err.to_string())
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
// The `SourceFields` newtype below validates "exactly one of url|ref" at
// deserialize time so block `Args` structs can just `#[serde(flatten)]` it
// and skip the per-block `pick_source(...)` validation step.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Source {
    Url(String),
    Ref(String),
}

/// JSON-deserializable wrapper over `Source`. Validates "exactly one of
/// `url` / `ref`" at deserialize time via `try_from`. Block crates flatten
/// this into their `Args`:
///
/// ```ignore
/// #[derive(Deserialize)]
/// struct Args {
///     #[serde(flatten)]
///     source: SourceFields,
///     width: Option<u32>,
/// }
/// ```
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(try_from = "RawSourceFields")]
pub struct SourceFields(pub Source);

impl SourceFields {
    pub fn into_inner(self) -> Source {
        self.0
    }
}

#[derive(serde::Deserialize)]
struct RawSourceFields {
    #[serde(default)]
    url: Option<String>,
    #[serde(default, rename = "ref")]
    ref_id: Option<String>,
}

impl TryFrom<RawSourceFields> for SourceFields {
    type Error = String;
    fn try_from(raw: RawSourceFields) -> Result<Self, String> {
        match (raw.url, raw.ref_id) {
            (Some(u), None) => Ok(SourceFields(Source::Url(u))),
            (None, Some(r)) => Ok(SourceFields(Source::Ref(r))),
            (Some(_), Some(_)) => Err("provide exactly one of `url` or `ref`".into()),
            (None, None) => Err("`url` or `ref` is required".into()),
        }
    }
}

/// Pick a `Source` from the two optional fields. Exactly one of `url` /
/// `ref_id` must be set.
///
/// Prefer flattening `SourceFields` into your `Args` struct — this function
/// is kept for ad-hoc callers and tests.
pub fn pick_source(url: Option<&str>, ref_id: Option<&str>) -> Result<Source, SkillError> {
    match (url, ref_id) {
        (Some(u), None) => Ok(Source::Url(u.to_string())),
        (None, Some(r)) => Ok(Source::Ref(r.to_string())),
        (Some(_), Some(_)) => Err(SkillError::InvalidArgs(
            "provide exactly one of `url` or `ref`".into(),
        )),
        (None, None) => Err(SkillError::InvalidArgs("`url` or `ref` is required".into())),
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
    let path: String = after_scheme
        .split('/')
        .skip(1)
        .collect::<Vec<_>>()
        .join("/");
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

/// Strip a filename's last extension, append `suffix`, then add `new_ext`.
///
/// ```text
/// filename_with_suffix("cat.png", "-resized", "jpg")  → "cat-resized.jpg"
/// filename_with_suffix("cat",     "-resized", "jpg")  → "cat-resized.jpg"
/// filename_with_suffix("a.b.mp4", "-trimmed", "mp4")  → "a.b-trimmed.mp4"
/// ```
pub fn filename_with_suffix(input: &str, suffix: &str, new_ext: &str) -> String {
    let stem = match input.rsplit_once('.') {
        Some((s, _)) => s,
        None => input,
    };
    format!("{stem}{suffix}.{new_ext}")
}

/// Map a `(kind, format-string)` pair to `(mime, extension)`.
///
/// Returns `None` for unrecognised format strings. The format strings are the
/// user-facing API values each block accepts (e.g. `"jpeg"` not `"jpg"`).
///
/// Supported:
/// - `AssetKind::Image`: `"jpeg"` → `("image/jpeg", "jpg")`, `"png"`, `"webp"`
/// - `AssetKind::Video`: `"mp4"` → `("video/mp4", "mp4")`, `"webm"`
/// - `AssetKind::Audio`: `"mp3"` → `("audio/mpeg", "mp3")`, `"wav"`, `"ogg"`,
///   `"flac"`, `"m4a"` (`audio/mp4`)
pub fn format_to_mime_and_ext(kind: AssetKind, fmt: &str) -> Option<(&'static str, &'static str)> {
    match (kind, fmt) {
        (AssetKind::Image, "jpeg") => Some(("image/jpeg", "jpg")),
        (AssetKind::Image, "png") => Some(("image/png", "png")),
        (AssetKind::Image, "webp") => Some(("image/webp", "webp")),
        (AssetKind::Video, "mp4") => Some(("video/mp4", "mp4")),
        (AssetKind::Video, "webm") => Some(("video/webm", "webm")),
        (AssetKind::Audio, "mp3") => Some(("audio/mpeg", "mp3")),
        (AssetKind::Audio, "wav") => Some(("audio/wav", "wav")),
        (AssetKind::Audio, "ogg") => Some(("audio/ogg", "ogg")),
        (AssetKind::Audio, "flac") => Some(("audio/flac", "flac")),
        (AssetKind::Audio, "m4a") => Some(("audio/mp4", "m4a")),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// AssetKind — image / video / document / any, controls MIME acceptance,
// expected-label, kind-label, and default filename used by `fetch_from_url` /
// `load_from_attachment`. Pulled out of the duplicated fetch helpers in
// blocks/image-* and blocks/video-*.
//
// Acceptance is expressed as a single `accepts_mime(mime)` predicate rather than
// a `mime_prefix` string, so a kind can match a MIME *family* (`image/`,
// `video/`, `application/`) or accept everything (`Any`) without callers having
// to know which matching mode applies. `Document` accepts the whole
// `application/` class (pdf, ooxml, xls, ods, octet-stream, zip, …); the precise
// file-type validation is left to the consuming parser (e.g. `lopdf`/`calamine`
// reject non-matching bytes), so the fetch-time check stays permissive within
// that class.
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AssetKind {
    Image,
    Video,
    /// An audio file — accepts the `audio/*` MIME class. Pairs with
    /// [`crate::descriptor::Input::Audio`] for the audio-only ffmpeg family.
    Audio,
    /// A binary document — accepts the `application/*` MIME class (PDF, OOXML
    /// `.xlsx`/`.docx`, legacy `.xls`, OpenDocument `.ods`, `application/zip`
    /// containers, and the generic `application/octet-stream` many static hosts
    /// serve binaries as). The real format check happens when the bytes are
    /// parsed downstream, not by the transport MIME.
    Document,
    /// Any bytes — no MIME validation at all. For tools that accept arbitrary
    /// binary input and fully validate it themselves downstream.
    Any,
}

// Methods are only consumed by the wasm-gated fetch/load functions below
// (plus tests). On host non-test builds, they look unused — silence the lint
// rather than peppering each method with cfg attributes.
#[allow(dead_code)]
impl AssetKind {
    /// Whether `mime` (already normalized: lowercase, parameters stripped) is
    /// acceptable for this kind. `Image`/`Video`/`Document` match on the
    /// `image/`/`video/`/`application/` prefix; `Any` accepts everything.
    pub(crate) fn accepts_mime(self, mime: &str) -> bool {
        match self {
            Self::Image => mime.starts_with("image/"),
            Self::Video => mime.starts_with("video/"),
            Self::Audio => mime.starts_with("audio/"),
            Self::Document => mime.starts_with("application/"),
            Self::Any => true,
        }
    }

    pub(crate) fn expected_url_label(self) -> &'static str {
        match self {
            Self::Image => "image/*",
            Self::Video => "video/*",
            Self::Audio => "audio/*",
            Self::Document => "application/*",
            Self::Any => "any",
        }
    }

    pub(crate) fn expected_attachment_label(self) -> &'static str {
        match self {
            Self::Image => "image/* attachment",
            Self::Video => "video/* attachment",
            Self::Audio => "audio/* attachment",
            Self::Document => "application/* attachment",
            Self::Any => "any attachment",
        }
    }

    pub(crate) fn too_large_label(self) -> &'static str {
        match self {
            Self::Image => "input image",
            Self::Video => "input video",
            Self::Audio => "input audio",
            Self::Document => "input document",
            Self::Any => "input file",
        }
    }

    pub(crate) fn default_filename(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Document => "document",
            Self::Any => "file",
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
    if !kind.accepts_mime(&mime) {
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
    let att_mime: String = att
        .mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if !kind.accepts_mime(&att_mime) {
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
/// `"image"`, `video/*` → `"video"`, `audio/*` → `"audio"`, everything else →
/// `"file"`. Used when a user upload arrives without a filename of its own.
pub fn default_filename_for_mime(mime: &str) -> &'static str {
    if mime.starts_with("image/") {
        "image"
    } else if mime.starts_with("video/") {
        "video"
    } else if mime.starts_with("audio/") {
        "audio"
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
/// Knows the image, video, and audio formats every gizza-ai block accepts.
pub fn mime_to_ext(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        "video/quicktime" => Some("mov"),
        "video/x-matroska" => Some("mkv"),
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav"),
        "audio/ogg" => Some("ogg"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/mp4" | "audio/x-m4a" => Some("m4a"),
        "audio/aac" => Some("aac"),
        "audio/webm" => Some("weba"),
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

// ---------------------------------------------------------------------------
// Text-shape skill helper. Returns Result<Vec<u8>, SkillError>; the tool's
// wasm `handle()` wraps Ok => GuestResult::respond, Err => GuestResult::error.
// ---------------------------------------------------------------------------

/// Serialize a success payload as `{ "result": <value> }`.
pub fn respond_ok<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, SkillError> {
    serde_json::to_vec(&serde_json::json!({ "result": value }))
        .map_err(|e| SkillError::Serialize(format!("serialize result: {e}")))
}

/// Run a text-shape skill: parse `A` from `body` (errors labeled
/// `invalid <block> args: …`), call `f`, and shape `{ "result": … }`.
pub fn run_skill<A, T, F>(body: &[u8], block: &str, f: F) -> Result<Vec<u8>, SkillError>
where
    A: serde::de::DeserializeOwned,
    T: serde::Serialize,
    F: FnOnce(A) -> Result<T, SkillError>,
{
    let args: A = serde_json::from_slice(body).invalid_args(block)?;
    let out = f(args)?;
    respond_ok(&out)
}

// ---------------------------------------------------------------------------
// Media helpers. `build_media_envelope` is pure (native-testable); the source
// resolver and ffmpeg dispatcher call host imports and are wasm-only.
// ---------------------------------------------------------------------------

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

/// Encode `out_bytes` (enforcing `max_out`) as a `data:` URL and wrap it in the
/// standard image/video `Envelope` (`for_llm` summary + `for_ui` data URL).
pub fn build_media_envelope(
    out_bytes: &[u8],
    mime: &str,
    filename: String,
    for_llm: String,
    max_out: usize,
) -> Result<Vec<u8>, SkillError> {
    if out_bytes.len() > max_out {
        return Err(SkillError::TooLarge {
            kind: "output",
            bytes: out_bytes.len(),
            cap: max_out,
        });
    }
    let data_url = format!("data:{mime};base64,{}", B64.encode(out_bytes));
    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: mime.to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

/// Base64-encode bytes (standard alphabet). Used by ffmpeg-page `build_argv`s
/// that ship extra virtual-FS files (a bundled font, a text file) to the page
/// driver via [`ArgvPlanWithInputs`] — the browser ffmpeg FS starts empty, so
/// a drawtext tool must hand it the font + text bytes alongside the media.
pub fn encode_b64(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

/// The color names ffmpeg's own parser accepts (`ffmpeg -colors`, the standard
/// CSS/X11 table), lower-cased. Validating against the same table means a bad
/// color fails in the tool with a guiding message instead of deep inside
/// ffmpeg — and the strict charset keeps a filtergraph string injection-free.
pub const FFMPEG_COLOR_NAMES: &[&str] = &[
    "aliceblue", "antiquewhite", "aqua", "aquamarine", "azure", "beige", "bisque", "black",
    "blanchedalmond", "blue", "blueviolet", "brown", "burlywood", "cadetblue", "chartreuse",
    "chocolate", "coral", "cornflowerblue", "cornsilk", "crimson", "cyan", "darkblue", "darkcyan",
    "darkgoldenrod", "darkgray", "darkgreen", "darkkhaki", "darkmagenta", "darkolivegreen",
    "darkorange", "darkorchid", "darkred", "darksalmon", "darkseagreen", "darkslateblue",
    "darkslategray", "darkturquoise", "darkviolet", "deeppink", "deepskyblue", "dimgray",
    "dodgerblue", "firebrick", "floralwhite", "forestgreen", "fuchsia", "gainsboro", "ghostwhite",
    "gold", "goldenrod", "gray", "green", "greenyellow", "honeydew", "hotpink", "indianred",
    "indigo", "ivory", "khaki", "lavender", "lavenderblush", "lawngreen", "lemonchiffon",
    "lightblue", "lightcoral", "lightcyan", "lightgoldenrodyellow", "lightgreen", "lightgrey",
    "lightpink", "lightsalmon", "lightseagreen", "lightskyblue", "lightslategray",
    "lightsteelblue", "lightyellow", "lime", "limegreen", "linen", "magenta", "maroon",
    "mediumaquamarine", "mediumblue", "mediumorchid", "mediumpurple", "mediumseagreen",
    "mediumslateblue", "mediumspringgreen", "mediumturquoise", "mediumvioletred", "midnightblue",
    "mintcream", "mistyrose", "moccasin", "navajowhite", "navy", "oldlace", "olive", "olivedrab",
    "orange", "orangered", "orchid", "palegoldenrod", "palegreen", "paleturquoise",
    "palevioletred", "papayawhip", "peachpuff", "peru", "pink", "plum", "powderblue", "purple",
    "red", "rosybrown", "royalblue", "saddlebrown", "salmon", "sandybrown", "seagreen",
    "seashell", "sienna", "silver", "skyblue", "slateblue", "slategray", "snow", "springgreen",
    "steelblue", "tan", "teal", "thistle", "tomato", "turquoise", "violet", "wheat", "white",
    "whitesmoke", "yellow", "yellowgreen",
];

/// Normalize a user-facing color into a form ffmpeg accepts verbatim and that
/// is safe to inject into a filtergraph: a lower-cased name from
/// [`FFMPEG_COLOR_NAMES`], or `#RRGGBB` / `#RGB` / `0xRRGGBB` / bare 6-digit hex
/// → `0xRRGGBB`. Errors (never defaults) on empty or unrecognized input — the
/// caller applies its own default before calling so the meaning of "blank" is
/// the caller's, not baked in here.
pub fn normalize_ffmpeg_color(color: &str) -> Result<String, String> {
    let t = color.trim();
    if t.is_empty() {
        return Err("color must not be empty".to_string());
    }
    let hex = t
        .strip_prefix('#')
        .or_else(|| t.strip_prefix("0x"))
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(format!("0x{}", hex.to_ascii_uppercase()));
    }
    if t.starts_with('#') && hex.len() == 3 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let doubled: String = hex.chars().flat_map(|c| [c, c]).collect();
        return Ok(format!("0x{}", doubled.to_ascii_uppercase()));
    }
    let lower = t.to_ascii_lowercase();
    if FFMPEG_COLOR_NAMES.contains(&lower.as_str()) {
        return Ok(lower);
    }
    Err(format!(
        "color {t:?} not recognized — use a CSS color name (black, white, navy, …) or hex like #1A2B3C"
    ))
}

#[cfg(test)]
mod color_tests {
    use super::normalize_ffmpeg_color;

    #[test]
    fn names_hex_and_short_hex_normalize() {
        assert_eq!(normalize_ffmpeg_color("black").unwrap(), "black");
        assert_eq!(normalize_ffmpeg_color(" White ").unwrap(), "white");
        assert_eq!(normalize_ffmpeg_color("DarkSlateGray").unwrap(), "darkslategray");
        assert_eq!(normalize_ffmpeg_color("#ff0000").unwrap(), "0xFF0000");
        assert_eq!(normalize_ffmpeg_color("1a2b3c").unwrap(), "0x1A2B3C");
        assert_eq!(normalize_ffmpeg_color("0xabcdef").unwrap(), "0xABCDEF");
        assert_eq!(normalize_ffmpeg_color("#abc").unwrap(), "0xAABBCC");
    }

    #[test]
    fn empty_and_unknown_error() {
        assert!(normalize_ffmpeg_color("").is_err());
        assert!(normalize_ffmpeg_color("   ").is_err());
        assert!(normalize_ffmpeg_color("notacolor").is_err());
        assert!(normalize_ffmpeg_color("#12").is_err());
        // bare 3-hex (no '#') is ambiguous with a name → rejected, as before.
        assert!(normalize_ffmpeg_color("abc").is_err());
    }
}

/// Resolve a `Source` to `(bytes, mime, filename)` — the `url` fetch vs `ref`
/// attachment branch every media tool repeats.
#[cfg(target_arch = "wasm32")]
pub fn resolve_source(
    source: Source,
    kind: AssetKind,
    max_in: usize,
) -> Result<(Vec<u8>, String, String), SkillError> {
    match source {
        Source::Url(u) => fetch_from_url(&u, kind, max_in),
        Source::Ref(id) => load_from_attachment(&id, kind, max_in),
    }
}

/// Run one ffmpeg-runtime call and return the output bytes, mapping a non-zero
/// exit to `SkillError::FfmpegExitNonZero` (200-char log snippet).
#[cfg(target_arch = "wasm32")]
pub fn dispatch_ffmpeg(
    argv: Vec<String>,
    in_name: String,
    in_bytes: Vec<u8>,
    out_name: String,
) -> Result<Vec<u8>, SkillError> {
    dispatch_ffmpeg_inputs(argv, vec![(in_name, in_bytes)], out_name)
}

/// Like [`dispatch_ffmpeg`] but writes MULTIPLE virtual-FS files before exec —
/// e.g. the media input plus a bundled font and a text file for a drawtext
/// tool. `inputs` is `(filename, bytes)` pairs; the native CLI service writes
/// each to its temp working dir and the browser bridge writes each into
/// ffmpeg's FS, so `-vf drawtext=fontfile=font.ttf:textfile=title.txt` resolves
/// identically on both surfaces.
#[cfg(target_arch = "wasm32")]
pub fn dispatch_ffmpeg_inputs(
    argv: Vec<String>,
    inputs: Vec<(String, Vec<u8>)>,
    out_name: String,
) -> Result<Vec<u8>, SkillError> {
    let req = FfmpegReq {
        args: argv,
        inputs,
        output: out_name,
    };
    let req_body = serde_json::to_vec(&req)
        .map_err(|e| SkillError::Serialize(format!("serialize ffmpeg request: {e}")))?;
    let resp_bytes = dispatch_ffmpeg_runtime(&req_body)?;
    let ff: FfmpegResp = serde_json::from_slice(&resp_bytes)
        .map_err(|e| SkillError::Serialize(format!("malformed ffmpeg response: {e}")))?;
    if ff.exit_code != 0 {
        return Err(SkillError::FfmpegExitNonZero {
            exit: ff.exit_code,
            snippet: ff.log.chars().take(200).collect(),
        });
    }
    Ok(ff.output)
}

/// The result an ffmpeg page tool's `build_argv` returns to the JS page driver:
/// the ffmpeg argument vector plus the output filename. Shared so every web
/// wrapper stops redefining an identical local `struct Plan`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArgvPlan {
    pub argv: Vec<String>,
    pub out_name: String,
}

/// Like [`ArgvPlan`] but also carries extra virtual-FS files the page ffmpeg
/// driver must write before exec, beyond the single uploaded media input. Each
/// entry is `(filename, base64-encoded bytes)`. Used by drawtext-style tools
/// (e.g. `video-title-card`) that ship a bundled font and the overlay text as a
/// `textfile` — so the text needs no filtergraph escaping and the font is
/// present in the browser ffmpeg's otherwise-empty FS. The chat/CLI surfaces
/// write the same files via [`dispatch_ffmpeg_inputs`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArgvPlanWithInputs {
    pub argv: Vec<String>,
    pub out_name: String,
    pub inputs: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_filename_uses_last_path_segment() {
        assert_eq!(
            derive_filename("https://x.test/path/cat.png", "image"),
            "cat.png"
        );
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
        assert_eq!(
            derive_filename("https://x.test/a\x01b.png", "image"),
            "ab.png"
        );
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
    fn source_fields_deserializes_url() {
        let v: SourceFields = serde_json::from_str(r#"{"url":"u"}"#).unwrap();
        assert!(matches!(v.0, Source::Url(ref u) if u == "u"));
    }

    #[test]
    fn source_fields_deserializes_ref() {
        let v: SourceFields = serde_json::from_str(r#"{"ref":"call_1"}"#).unwrap();
        assert!(matches!(v.0, Source::Ref(ref r) if r == "call_1"));
    }

    #[test]
    fn source_fields_rejects_both() {
        let err = serde_json::from_str::<SourceFields>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn source_fields_rejects_neither() {
        let err = serde_json::from_str::<SourceFields>(r#"{}"#).unwrap_err();
        assert!(err.to_string().contains("required"));
    }

    #[test]
    fn source_fields_flattens_alongside_other_fields() {
        #[derive(serde::Deserialize)]
        struct A {
            #[serde(flatten)]
            source: SourceFields,
            #[serde(default)]
            width: Option<u32>,
        }
        let a: A = serde_json::from_str(r#"{"url":"u","width":200}"#).unwrap();
        assert!(matches!(a.source.0, Source::Url(ref u) if u == "u"));
        assert_eq!(a.width, Some(200));
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
    fn filename_with_suffix_replaces_extension() {
        assert_eq!(
            filename_with_suffix("cat.png", "-resized", "jpg"),
            "cat-resized.jpg"
        );
    }

    #[test]
    fn filename_with_suffix_no_extension() {
        assert_eq!(
            filename_with_suffix("cat", "-resized", "jpg"),
            "cat-resized.jpg"
        );
    }

    #[test]
    fn filename_with_suffix_multiple_dots() {
        // Only the LAST dot-segment is treated as the extension
        assert_eq!(
            filename_with_suffix("video.tmp.mp4", "-trimmed", "mp4"),
            "video.tmp-trimmed.mp4"
        );
    }

    #[test]
    fn format_to_mime_and_ext_image() {
        assert_eq!(
            format_to_mime_and_ext(AssetKind::Image, "jpeg"),
            Some(("image/jpeg", "jpg"))
        );
        assert_eq!(
            format_to_mime_and_ext(AssetKind::Image, "png"),
            Some(("image/png", "png"))
        );
        assert_eq!(
            format_to_mime_and_ext(AssetKind::Image, "webp"),
            Some(("image/webp", "webp"))
        );
        assert_eq!(format_to_mime_and_ext(AssetKind::Image, "bogus"), None);
    }

    #[test]
    fn format_to_mime_and_ext_video() {
        assert_eq!(
            format_to_mime_and_ext(AssetKind::Video, "mp4"),
            Some(("video/mp4", "mp4"))
        );
        assert_eq!(
            format_to_mime_and_ext(AssetKind::Video, "webm"),
            Some(("video/webm", "webm"))
        );
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
        assert_eq!(we.code, ErrorCode::InvalidArgument);
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
        assert_eq!(we.code, ErrorCode::OutOfRange);
    }

    #[test]
    fn skill_error_attachment_not_found_maps_to_not_found() {
        let we: WaferError = SkillError::AttachmentNotFound("call_1".into()).into();
        assert_eq!(we.code, ErrorCode::NotFound);
    }

    #[test]
    fn skill_error_wafer_passes_through() {
        let inner = WaferError::new(ErrorCode::Unavailable, "underlying");
        let we: WaferError = SkillError::Wafer(inner).into();
        assert_eq!(we.code, ErrorCode::Unavailable);
        assert_eq!(we.message, "underlying");
    }

    #[test]
    fn asset_kind_image_labels() {
        let k = AssetKind::Image;
        assert!(k.accepts_mime("image/png"));
        assert!(k.accepts_mime("image/jpeg"));
        assert!(!k.accepts_mime("video/mp4"));
        assert!(!k.accepts_mime("application/pdf"));
        assert_eq!(k.expected_url_label(), "image/*");
        assert_eq!(k.expected_attachment_label(), "image/* attachment");
        assert_eq!(k.too_large_label(), "input image");
        assert_eq!(k.default_filename(), "image");
    }

    #[test]
    fn asset_kind_video_labels() {
        let k = AssetKind::Video;
        assert!(k.accepts_mime("video/mp4"));
        assert!(k.accepts_mime("video/webm"));
        assert!(!k.accepts_mime("image/png"));
        assert!(!k.accepts_mime("application/pdf"));
        assert_eq!(k.expected_url_label(), "video/*");
        assert_eq!(k.expected_attachment_label(), "video/* attachment");
        assert_eq!(k.too_large_label(), "input video");
        assert_eq!(k.default_filename(), "video");
    }

    #[test]
    fn asset_kind_document_accepts_application_class() {
        let k = AssetKind::Document;
        // PDF (merge-pdf, pdf-extract-text).
        assert!(k.accepts_mime("application/pdf"));
        // OOXML spreadsheet + legacy xls + OpenDocument (xlsx-to-csv).
        assert!(k.accepts_mime("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"));
        assert!(k.accepts_mime("application/vnd.ms-excel"));
        assert!(k.accepts_mime("application/vnd.oasis.opendocument.spreadsheet"));
        // Generic binary types many static hosts serve documents as — the
        // parser does the real format validation downstream.
        assert!(k.accepts_mime("application/octet-stream"));
        assert!(k.accepts_mime("application/zip"));
        // But not non-application MIMEs (e.g. a GitHub raw redirect HTML page).
        assert!(!k.accepts_mime("text/html"));
        assert!(!k.accepts_mime("image/png"));
        assert_eq!(k.expected_url_label(), "application/*");
        assert_eq!(k.expected_attachment_label(), "application/* attachment");
        assert_eq!(k.too_large_label(), "input document");
        assert_eq!(k.default_filename(), "document");
    }

    #[test]
    fn asset_kind_audio_accepts_audio_class() {
        let k = AssetKind::Audio;
        assert!(k.accepts_mime("audio/mpeg"));
        assert!(k.accepts_mime("audio/wav"));
        assert!(k.accepts_mime("audio/ogg"));
        assert!(!k.accepts_mime("video/mp4"));
        assert!(!k.accepts_mime("application/octet-stream"));
        assert_eq!(k.expected_url_label(), "audio/*");
        assert_eq!(k.expected_attachment_label(), "audio/* attachment");
        assert_eq!(k.too_large_label(), "input audio");
        assert_eq!(k.default_filename(), "audio");
    }

    #[test]
    fn audio_formats_map_to_mime_and_ext() {
        assert_eq!(
            format_to_mime_and_ext(AssetKind::Audio, "mp3"),
            Some(("audio/mpeg", "mp3"))
        );
        assert_eq!(
            format_to_mime_and_ext(AssetKind::Audio, "wav"),
            Some(("audio/wav", "wav"))
        );
        assert_eq!(
            format_to_mime_and_ext(AssetKind::Audio, "m4a"),
            Some(("audio/mp4", "m4a"))
        );
        assert_eq!(format_to_mime_and_ext(AssetKind::Audio, "bogus"), None);
    }

    #[test]
    fn asset_kind_any_accepts_everything() {
        let k = AssetKind::Any;
        assert!(k.accepts_mime("text/html"));
        assert!(k.accepts_mime("application/octet-stream"));
        assert!(k.accepts_mime("image/png"));
        assert!(k.accepts_mime(""));
        assert_eq!(k.expected_url_label(), "any");
        assert_eq!(k.expected_attachment_label(), "any attachment");
        assert_eq!(k.too_large_label(), "input file");
        assert_eq!(k.default_filename(), "file");
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

    #[derive(serde::Deserialize)]
    struct EchoArgs {
        text: String,
    }
    #[derive(serde::Serialize)]
    struct EchoOut {
        echo: String,
    }

    #[test]
    fn run_skill_shapes_result_on_ok() {
        let body = br#"{"text":"hi"}"#;
        let out = run_skill(body, "echo", |a: EchoArgs| {
            Ok::<_, SkillError>(EchoOut { echo: a.text })
        })
        .expect("ok path");
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["result"]["echo"], "hi");
    }

    #[test]
    fn run_skill_labels_bad_json_as_invalid_args() {
        let body = br#"{ not json"#;
        let err = run_skill(body, "echo", |a: EchoArgs| {
            Ok::<_, SkillError>(EchoOut { echo: a.text })
        })
        .expect_err("bad json must error");
        assert!(matches!(err, SkillError::InvalidArgs(_)));
        assert!(err.to_string().contains("invalid echo args"));
    }

    #[test]
    fn run_skill_propagates_inner_error() {
        let body = br#"{"text":"x"}"#;
        let err = run_skill(body, "echo", |_a: EchoArgs| {
            Err::<EchoOut, _>(SkillError::InvalidArgs("nope".into()))
        })
        .expect_err("inner error propagates");
        assert!(matches!(err, SkillError::InvalidArgs(_)));
    }

    #[test]
    fn build_media_envelope_emits_data_url_and_caps_size() {
        let bytes = b"\x89PNG\r\n\x1a\n";
        let out = build_media_envelope(
            bytes,
            "image/png",
            "cat-resized.png".into(),
            "resized cat".into(),
            1024,
        )
        .expect("under cap");
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        // Envelope serde-renames to _for_llm / _for_ui.
        assert_eq!(v["_for_llm"], "resized cat");
        assert_eq!(v["_for_ui"]["mime"], "image/png");
        assert_eq!(v["_for_ui"]["filename"], "cat-resized.png");
        let data_url = v["_for_ui"]["data_url"].as_str().unwrap();
        assert!(data_url.starts_with("data:image/png;base64,"));

        let err = build_media_envelope(bytes, "image/png", "x.png".into(), "x".into(), 4)
            .expect_err("over cap");
        assert!(matches!(err, SkillError::TooLarge { .. }));
    }

    #[test]
    fn argv_plan_serializes_to_argv_and_out_name() {
        let plan = ArgvPlan {
            argv: vec!["-i".into(), "in.png".into()],
            out_name: "out.png".into(),
        };
        let v = serde_json::to_value(&plan).unwrap();
        assert_eq!(v["argv"], serde_json::json!(["-i", "in.png"]));
        assert_eq!(v["out_name"], "out.png");
    }
}
