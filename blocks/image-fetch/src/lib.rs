//! gizza-ai/image-fetch — fetches an image URL and returns a renderable envelope.
//!
//! Pipeline: parse {url} → call wafer-run/network GET → validate status, mime,
//! body size → base64-encode → emit envelope `{_for_llm, _for_ui}` JSON. The
//! agent block recognises the envelope and bifurcates LLM history (gets
//! `_for_llm`) from UI rendering (gets `_for_ui` with the data: URL).

// The #[wafer_block] macro emits wasm-only registration; supporting imports
// and the Args type are only used inside that impl.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{derive_filename, Envelope, ForUi, SkillError, SkillResultExt};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

#[derive(Deserialize)]
struct Args {
    url: String,
}

/// Pull the lowercased MIME class out of an HTTP `Content-Type` header
/// value (e.g. `"image/png; charset=binary"` → `"image/png"`). Falls back
/// to `application/octet-stream` when absent / malformed.
fn parse_content_type_mime(headers: &HashMap<String, Vec<String>>) -> String {
    let raw = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .and_then(|(_, vs)| vs.first().cloned())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    raw.split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase()
}

#[cfg(target_arch = "wasm32")]
struct ImageFetch;

#[cfg(target_arch = "wasm32")]
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
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-fetch")?;

    let resp = wafer_sdk::clients::network::do_request("GET", &args.url, &HashMap::new(), None)?;
    if resp.status_code >= 400 {
        return Err(SkillError::HttpStatus {
            status: resp.status_code,
            url: args.url,
        });
    }

    let mime = parse_content_type_mime(&resp.headers);
    if !mime.starts_with("image/") {
        return Err(SkillError::UnexpectedMime {
            expected: "image/*",
            actual: mime,
        });
    }

    // Content-Length pre-check: defensive UX guard — refuse to surface huge
    // images we'd reject anyway. Binary transport no longer inflates 6x, so
    // this is no longer load-bearing for OOM avoidance.
    if let Some(cl) = resp
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, vs)| vs.first())
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        if cl > MAX_BYTES {
            return Err(SkillError::TooLarge {
                kind: "image",
                bytes: cl,
                cap: MAX_BYTES,
            });
        }
    }
    if resp.body.len() > MAX_BYTES {
        return Err(SkillError::TooLarge {
            kind: "image",
            bytes: resp.body.len(),
            cap: MAX_BYTES,
        });
    }

    let body_len = resp.body.len();
    let encoded = B64.encode(&resp.body);
    let data_url = format!("data:{mime};base64,{encoded}");
    let filename = derive_filename(&args.url, "image");

    let env = Envelope {
        for_llm: format!("fetched {body_len}-byte {mime} from {}", args.url),
        for_ui: ForUi {
            data_url,
            mime,
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_type_strips_params_and_lowercases() {
        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            vec!["IMAGE/PNG; charset=binary".to_string()],
        );
        assert_eq!(parse_content_type_mime(&headers), "image/png");
    }

    #[test]
    fn parse_content_type_handles_case_insensitive_header_name() {
        let mut headers = HashMap::new();
        headers.insert(
            "content-type".to_string(),
            vec!["image/jpeg".to_string()],
        );
        assert_eq!(parse_content_type_mime(&headers), "image/jpeg");
    }

    #[test]
    fn parse_content_type_falls_back_when_header_missing() {
        let headers: HashMap<String, Vec<String>> = HashMap::new();
        assert_eq!(parse_content_type_mime(&headers), "application/octet-stream");
    }
}
