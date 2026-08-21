//! gizza-ai/jpg-stego-embed — hide a file or message inside a JPEG while the
//! picture stays byte-for-byte the same.
//!
//! Pure-Rust (deflate + AES-GCM + a JPEG container writer in core), so like the
//! other pure tools it runs on ALL backends including the chat Service Worker.
//! Pipeline: resolve the carrier JPEG (URL fetch or attachment ref) → resolve
//! the payload (inline `payload_text`, or `payload_url`/`payload_ref` bytes) →
//! `core::embed` writes a framed container into APP9/COM marker segments or
//! after the EOI marker → base64 `image/jpeg` envelope.
//!
//! The entropy-coded image data is copied verbatim, so the output renders as
//! exactly the same picture and stays a JPEG — unlike pixel-based LSB
//! steganography, which JPEG's lossy compression destroys (see the lsb-embed
//! block, which has to return a PNG for that reason).
//!
//! Surfaces: chat + CLI. No standalone page — a pure-Rust image-bytes output has
//! no page render mode (same as lsb-embed / add-text-to-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{resolve_source, Source};
use gizza_ai_block_utils::{
    build_media_envelope, Input, Param, SkillError, SkillResultExt, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

/// Carrier/output cap. JPEG photos are the input class here, and the container
/// only adds the payload on top, so one cap covers both sides.
const MAX_BYTES: usize = 16 * 1024 * 1024;
/// Payload cap. Smaller than the carrier cap: the hidden bytes ride inside the
/// output, so they have to leave room under `MAX_BYTES` for the picture itself.
const MAX_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    /// The carrier JPEG (`url` ⊕ `ref`).
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    #[serde(default)]
    payload_text: Option<String>,
    #[serde(default)]
    payload_url: Option<String>,
    #[serde(default)]
    payload_ref: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default = "default_compress")]
    compress: bool,
    #[serde(default = "default_method")]
    method: String,
}
fn default_compress() -> bool {
    true
}
fn default_method() -> String {
    "app".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::string("payload_text")
                .describe("The secret text to hide. Give exactly one of payload_text, payload_url or payload_ref."),
        )
        .param(
            Param::string("payload_url")
                .describe("URL (HTTP/HTTPS) of a file to hide. Give exactly one of payload_text, payload_url or payload_ref."),
        )
        .param(
            Param::string("payload_ref")
                .describe("Reference id from a prior tool call for the file to hide. Give exactly one of payload_text, payload_url or payload_ref."),
        )
        .param(
            Param::string("filename")
                .describe("Filename to record alongside the hidden payload, so an extractor can restore it. Defaults to the payload's own name."),
        )
        .param(
            Param::string("password")
                .describe("Optional passphrase. When set, the payload is AES-256-GCM encrypted and the same passphrase is needed to recover it."),
        )
        .param(
            Param::boolean("compress")
                .default(true)
                .describe("Deflate the payload before hiding it (default true). Kept only when it actually shrinks the payload."),
        )
        .param(
            Param::enumv("method", ["app", "comment", "append"])
                .default("app")
                .describe("Where to hide the payload: app = APP9 marker segments (default, stays a clean JPEG), comment = COM segments, append = a blob after the EOI marker."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JpgStegoEmbed;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jpg-stego-embed",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Hide a file or message inside a JPEG without changing the picture",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Hide a secret file or text message inside a JPEG photo. The picture data is copied verbatim, so the output is the same image, still a JPEG, with the payload concealed in container space. Give the payload as exactly one of payload_text (inline text), payload_url (HTTP/HTTPS) or payload_ref (id from a prior tool call). Optionally set filename (recorded for the extractor), password (AES-256-GCM encrypts the payload), compress (deflate first, default true) and method: app = APP9 marker segments (default), comment = COM segments, append = after the EOI marker. Provide the carrier JPEG as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl JpgStegoEmbed {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;
    use gizza_ai_jpg_stego_embed_core as core;

    let args: Args = serde_json::from_slice(&body).invalid_args("jpg-stego-embed")?;

    // Exactly one payload source — the same url⊕ref discipline `SourceFields`
    // applies to the carrier, spelled out here because the payload is a second,
    // optional-by-shape input the descriptor cannot express as a `oneOf`.
    let text = args.payload_text.filter(|s| !s.is_empty());
    let purl = args.payload_url.filter(|s| !s.is_empty());
    let pref = args.payload_ref.filter(|s| !s.is_empty());
    let given = text.is_some() as u8 + purl.is_some() as u8 + pref.is_some() as u8;
    if given != 1 {
        return Err(SkillError::InvalidArgs(
            "give exactly one of payload_text, payload_url or payload_ref".into(),
        ));
    }

    let method = core::parse_method(&args.method).map_err(SkillError::InvalidArgs)?;

    let (carrier, _mime, carrier_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    // The payload is arbitrary bytes (a key file, an archive, a note), so it is
    // resolved as `Any` — no transport-MIME gate.
    let (payload, payload_name) = match (text, purl, pref) {
        (Some(t), _, _) => (t.into_bytes(), String::new()),
        (_, Some(u), _) => {
            let (bytes, _m, name) = resolve_source(Source::Url(u), AssetKind::Any, MAX_PAYLOAD_BYTES)?;
            (bytes, name)
        }
        (_, _, Some(r)) => {
            let (bytes, _m, name) = resolve_source(Source::Ref(r), AssetKind::Any, MAX_PAYLOAD_BYTES)?;
            (bytes, name)
        }
        _ => unreachable!("payload source count checked above"),
    };

    let opts = core::Options {
        method,
        filename: args
            .filename
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(payload_name),
        password: args.password.filter(|s| !s.is_empty()),
        compress: args.compress,
    };

    let report = core::embed(&carrier, &payload, &opts).map_err(SkillError::InvalidArgs)?;

    let stem = carrier_name
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(&carrier_name);
    let out_name = format!("{stem}-stego.jpg");

    let mut notes = String::new();
    if report.compressed {
        notes.push_str(", compressed");
    }
    if report.encrypted {
        notes.push_str(", encrypted");
    }
    let for_llm = format!(
        "hid {} bytes inside {carrier_name} via {} → {}-byte JPEG ({out_name}, +{:.1}%{notes}); the picture is unchanged",
        report.payload_bytes,
        report.method.as_str(),
        report.output_bytes,
        report.growth_percent(),
    );
    build_media_envelope(&report.output, "image/jpeg", out_name, for_llm, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":          { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":          { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "payload_text": { "type": "string", "description": "The secret text to hide. Give exactly one of payload_text, payload_url or payload_ref." },
                    "payload_url":  { "type": "string", "description": "URL (HTTP/HTTPS) of a file to hide. Give exactly one of payload_text, payload_url or payload_ref." },
                    "payload_ref":  { "type": "string", "description": "Reference id from a prior tool call for the file to hide. Give exactly one of payload_text, payload_url or payload_ref." },
                    "filename":     { "type": "string", "description": "Filename to record alongside the hidden payload, so an extractor can restore it. Defaults to the payload's own name." },
                    "password":     { "type": "string", "description": "Optional passphrase. When set, the payload is AES-256-GCM encrypted and the same passphrase is needed to recover it." },
                    "compress":     { "type": "boolean", "default": true, "description": "Deflate the payload before hiding it (default true). Kept only when it actually shrinks the payload." },
                    "method":       { "type": "string", "enum": ["app", "comment", "append"], "default": "app", "description": "Where to hide the payload: app = APP9 marker segments (default, stays a clean JPEG), comment = COM segments, append = a blob after the EOI marker." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The manifest is what the runtime serves to the LLM — it must not drift
    /// from the descriptor either.
    #[test]
    fn manifest_schema_matches_descriptor() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../manifest.json")).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(
            manifest["tool"]["parameters"], derived,
            "manifest.json parameters drifted from descriptor()"
        );
    }

    /// `method` is a fixed choice; the defaults the schema advertises must be the
    /// ones the deserializer actually applies when the LLM omits them.
    #[test]
    fn defaults_match_the_advertised_schema() {
        let args: Args =
            serde_json::from_str(r#"{"url":"https://example.com/a.jpg","payload_text":"hi"}"#)
                .unwrap();
        assert!(args.compress, "compress defaults to true");
        assert_eq!(args.method, "app", "method defaults to app");
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(schema["properties"]["compress"]["default"], true);
        assert_eq!(schema["properties"]["method"]["default"], "app");
    }
}
