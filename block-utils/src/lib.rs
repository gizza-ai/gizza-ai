//! Shared helpers for gizza-ai skill blocks.
//!
//! Pulled out of the duplicated copies in `blocks/image-*` and `blocks/video-*`.
//! Each block crate depends on this via a `path = "../../block-utils"` dep.

use serde::{Deserialize, Serialize};
use wafer_block::core_types::{Message, WaferError};

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

// ---------------------------------------------------------------------------
// MIME → extension
// ---------------------------------------------------------------------------

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
// Agent-side response envelope (LLM history vs UI render)
// ---------------------------------------------------------------------------

/// Render hint for the chat UI. The agent block strips this off and forwards
/// it to the frontend as `tool_result.for_ui`.
#[derive(Serialize)]
pub struct ForUi {
    pub data_url: String,
    pub mime: String,
    pub filename: String,
}

/// Envelope a skill block emits when it has a renderable artifact: the LLM
/// gets `_for_llm` as a plain-text summary; the UI gets `_for_ui` as a
/// structured render hint.
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
}
