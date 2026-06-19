//! gizza-ai/image-fetch — fetch an image by URL or attachment ref and return it
//! inline. No transform — the fetched bytes ARE the output image.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI); the handler delegates source-resolution and
//! envelope-building to `block_utils`. Unlike `image-resize`, image-fetch does
//! NOT call ffmpeg — it fetches the image and envelopes the bytes verbatim. See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports
// and the Args type are only used inside the wasm32-gated impl, so they appear
// "unused" when running native unit tests. `descriptor()` / `schema_json()`
// remain native-compilable so the drift-guard test below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    build_media_envelope, AssetKind, Input, SkillError, SkillResultExt, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
}

/// Single-source param descriptor → chat schema (and CLI). image-fetch takes
/// only an image source (`url`⊕`ref`) and no scalar params. The drift-guard
/// test below proves the derived schema matches the pre-retrofit authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageFetch;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-fetch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fetch an image by URL or a prior tool call ref and return it for inline display",
    requires = ["wafer-run/network"],
    skill(
        description = "Fetch an image and render it inline. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = schema_json()
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
    // 1. Validate args — exactly one of url/ref (enforced by SourceFields).
    let args: Args = serde_json::from_slice(&body).invalid_args("image-fetch")?;

    // 2. Resolve source — URL fetch or attachment lookup. The fetched bytes ARE
    //    the output image; there is no transform step.
    let (bytes, mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    // 3. Envelope the bytes verbatim.
    let for_llm = format!("fetched {} {} ({})", bytes.len(), mime, filename);
    build_media_envelope(&bytes, &mime, filename, for_llm, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. The pre-retrofit
    /// schema only carried a single required `url`; retrofitting onto the shared
    /// `Input::Image` abstraction adds the `ref` alternative + the `url`⊕`ref`
    /// `oneOf` (so the tool can also consume a prior tool call's image) and
    /// `additionalProperties: false` (uniform hardening). The `url`/`ref`
    /// property descriptions are centralized in `to_schema_json`, so the
    /// expected JSON uses that shared wording.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
