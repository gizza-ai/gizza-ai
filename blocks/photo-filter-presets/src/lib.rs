//! gizza-ai/photo-filter-presets — apply a named photographic filter preset
//! (sepia, vintage, warm, cool, noir, grayscale, vivid, invert, fade) to an
//! image via ffmpeg, on the shared tool abstraction.
//!
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. The chat schema is derived from
//! `descriptor()` (single source — shared across chat + CLI + page) and the
//! drift-guard test below proves it matches the authored schema. See
//! blocks/image-convert/src/lib.rs for a full example.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, AssetKind, Input, Param, SkillError, SourceFields,
    ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_photo_filter_presets_core::{parse_preset, preset_name, DEFAULT_PRESET, PRESETS};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    preset: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. `preset` is an enum with a sepia default.
    ToolDescriptor::new(Input::Image).param(
        Param::enumv("preset", PRESETS)
            .default(DEFAULT_PRESET)
            .describe(
                "Photographic filter preset to apply. One of sepia, vintage, warm, cool, noir, \
                 grayscale, vivid, invert, fade. Default: sepia.",
            ),
    )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/photo-filter-presets",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Apply a photo filter preset to an image",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Apply a named photographic filter preset (sepia, vintage, warm, cool, noir, grayscale, vivid, invert, fade) to an image. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call), and optional preset (default sepia).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid photo-filter-presets args: {e}")))?;
    let preset = parse_preset(args.preset.as_deref()).map_err(SkillError::InvalidArgs)?;
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) =
        gizza_ai_photo_filter_presets_core::plan(&in_path, preset).map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, in_path, bytes, out_name)?;
    let for_llm = format!("applied {} filter to {in_name}", preset_name(preset));
    build_media_envelope(&output, &mime, in_name, for_llm, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-guard: the descriptor-derived chat schema must match the authored
    /// schema below, so the LLM-facing tool definition never silently changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "preset": {
                        "type": "string",
                        "enum": ["sepia", "vintage", "warm", "cool", "noir", "grayscale", "vivid", "invert", "fade"],
                        "default": "sepia",
                        "description": "Photographic filter preset to apply. One of sepia, vintage, warm, cool, noir, grayscale, vivid, invert, fade. Default: sepia."
                    }
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

    #[test]
    fn descriptor_exposes_every_preset() {
        let json = schema_json();
        for name in PRESETS {
            assert!(json.contains(name), "schema enum must list {name}");
        }
    }
}
