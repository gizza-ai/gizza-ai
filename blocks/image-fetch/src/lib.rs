//! gizza-ai/image-fetch — fetches an image URL and returns a renderable envelope.
//!
//! Pipeline: parse {url} → call wafer-run/network GET → validate status, mime,
//! body size → base64-encode → emit envelope `{_for_llm, _for_ui}` JSON. The
//! agent block recognises the envelope and bifurcates LLM history (gets
//! `_for_llm`) from UI rendering (gets `_for_ui` with the data: URL).

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

#[derive(Deserialize)]
struct Args {
    url: String,
}

#[derive(Serialize)]
struct ForUi {
    data_url: String,
    mime: String,
    filename: String,
}

#[derive(Serialize)]
struct Envelope {
    #[serde(rename = "_for_llm")]
    for_llm: String,
    #[serde(rename = "_for_ui")]
    for_ui: ForUi,
}

struct ImageFetch;

#[wafer_block(
    name = "gizza-ai/image-fetch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fetch an image URL and return it for inline display",
    capabilities(callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Fetch an image from a URL and render it inline.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "HTTP/HTTPS URL of the image to fetch." }
            },
            "required": ["url"],
            "additionalProperties": false
        }"#
    ),
)]
impl ImageFetch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // Skill input parsing: LLM tool-call args (JSON wire format).
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid image-fetch args: {e}"),
                ));
            }
        };

        // 1. Fetch.
        let resp = match wafer_sdk::clients::network::do_request(
            "GET",
            &args.url,
            &HashMap::new(),
            None,
        ) {
            Ok(r) => r,
            Err(e) => return GuestResult::error(e),
        };

        // 2. HTTP status check.
        if resp.status_code >= 400 {
            return GuestResult::error(WaferError::new(
                ErrorCode::UNAVAILABLE,
                format!("HTTP {} for {}", resp.status_code, args.url),
            ));
        }

        // 3. Content-type check (case-insensitive header lookup, first value, strip ; params).
        let raw_mime = resp
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
        if !mime.starts_with("image/") {
            return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                format!("expected image/* content-type, got {mime}"),
            ));
        }

        // 4a. Content-Length pre-check: reject when the server advertises a size
        //     that already exceeds the cap. With the new binary transport this
        //     is no longer load-bearing for OOM avoidance (the wire format no
        //     longer inflates binary data ~6x), but it remains as a defensive
        //     UX guard — refuse to download huge images we'd reject anyway.
        if let Some(cl) = resp
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, vs)| vs.first())
            .and_then(|v| v.trim().parse::<usize>().ok())
        {
            if cl > MAX_BYTES {
                return GuestResult::error(WaferError::new(
                    ErrorCode::OUT_OF_RANGE,
                    format!("image too large: {cl} bytes (cap {MAX_BYTES} bytes)"),
                ));
            }
        }

        // 4b. Body size check (catches cases where Content-Length is absent).
        if resp.body.len() > MAX_BYTES {
            return GuestResult::error(WaferError::new(
                ErrorCode::OUT_OF_RANGE,
                format!(
                    "image too large: {} bytes (cap {} bytes)",
                    resp.body.len(),
                    MAX_BYTES
                ),
            ));
        }

        // 5. Encode + build data URL.
        let body_len = resp.body.len();
        let encoded = B64.encode(&resp.body);
        let data_url = format!("data:{mime};base64,{encoded}");

        // 6. Derive filename from URL last path segment.
        let filename = derive_filename(&args.url);

        // 7. Build envelope.
        let env = Envelope {
            for_llm: format!("fetched {body_len}-byte {mime} from {}", args.url),
            for_ui: ForUi {
                data_url,
                mime,
                filename,
            },
        };
        // Skill output emission: LLM tool-call envelope (JSON wire format).
        match serde_json::to_vec(&env) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(WaferError::new(
                ErrorCode::INTERNAL,
                format!("serialize envelope: {e}"),
            )),
        }
    }
}

/// Best-effort: take the URL's last path segment, percent-decode it, strip
/// non-printable / control characters, fall back to "image" if empty.
/// No `url` crate dependency — we do the minimum manually to keep the wasm
/// payload small.
fn derive_filename(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let path = after_scheme.split('/').skip(1).collect::<Vec<_>>().join("/");
    let path = path.split('?').next().unwrap_or("");
    let path = path.split('#').next().unwrap_or("");
    let last = path.rsplit('/').next().unwrap_or("");
    let decoded = percent_decode(last);
    let cleaned: String = decoded
        .chars()
        .filter(|c| !c.is_control() && *c != '\u{FFFD}')
        .collect();
    if cleaned.is_empty() {
        "image".to_string()
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
