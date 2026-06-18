//! gizza-ai/url-encode — percent-encodes / decodes text and URLs.
//!
//! Thin chat-skill wrapper around `gizza-ai-url-encode-core`. Takes
//! `{ "text": "...", "mode": "encode|decode", "target": "component|uri" }`
//! and returns `{ "result": "..." }` or `{ "error": "..." }`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    target: String,
}

#[cfg(target_arch = "wasm32")]
struct UrlEncode;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/url-encode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "URL Encode skill",
    skill(
        description = "Percent-encode or percent-decode text and URLs. Use mode='encode' (default) to make text URL-safe or mode='decode' to reverse it. When encoding, target='component' (default) escapes everything for a single query value or path segment (e.g. 'São Paulo' -> 'S%C3%A3o%20Paulo'); target='uri' encodes a whole URL while preserving its delimiters (: / ? # & = etc.).",
        parameters = r#"{
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The text or URL to encode or decode." },
                "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) percent-encodes, 'decode' reverses it." },
                "target": { "type": "string", "enum": ["component", "uri"], "default": "component", "description": "Encode mode only: 'component' (default) escapes reserved chars for a single value; 'uri' preserves URL delimiters. Ignored when decoding." }
            },
            "required": ["text"],
            "additionalProperties": false
        }"#
    )
)]
impl UrlEncode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => return respond_error(format!("invalid args: {e}")),
        };
        match gizza_ai_url_encode_core::convert(&args.text, &args.mode, &args.target) {
            Ok(v) => GuestResult::respond(
                serde_json::to_vec(&serde_json::json!({ "result": v })).unwrap_or_default(),
            ),
            Err(e) => respond_error(e),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn respond_error(msg: String) -> GuestResult {
    GuestResult::respond(
        serde_json::to_vec(&serde_json::json!({ "error": msg })).unwrap_or_default(),
    )
}
