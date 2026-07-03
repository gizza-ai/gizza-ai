//! gizza-ai/image-vignette — add a soft darkened (or lightened) vignette
//! around the edges of an image via ffmpeg, on the shared tool abstraction.
//!
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. Users pass a friendly `strength`
//! 0–100; the core maps it onto the ffmpeg `vignette` filter's angle (radians,
//! max PI/2) — raw radians are never exposed. The chat schema is derived from
//! `descriptor()` (single source — shared across chat + CLI + page) and the
//! drift-guard test below proves it matches the authored schema.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, AssetKind, Input, Param, SkillError, SourceFields,
    ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_vignette_core::{
    mode_name, parse_mode, DEFAULT_CENTER_PCT, DEFAULT_MODE, DEFAULT_STRENGTH, MODES,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    strength: Option<f64>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    center_x: Option<f64>,
    #[serde(default)]
    center_y: Option<f64>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. `strength` is the friendly 0–100 knob the
    // core maps onto the ffmpeg vignette angle; `mode` picks dark vs light
    // edges; the center is a percentage so it works for any resolution.
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("strength")
                .min(0.0)
                .max(100.0)
                .default(DEFAULT_STRENGTH)
                .describe(
                    "Vignette strength, 0-100. 0 leaves the image unchanged, 40 (the default) is \
                     a classic soft vignette, 100 fades the corners fully to black (or white in \
                     lighten mode). Example: 80 for a dramatic look.",
                ),
        )
        .param(Param::enumv("mode", MODES).default(DEFAULT_MODE).describe(
            "darken (default) darkens the edges like a classic lens vignette; lighten brightens \
             them instead for a faded, hazy border.",
        ))
        .param(
            Param::number("center_x")
                .min(0.0)
                .max(100.0)
                .default(DEFAULT_CENTER_PCT)
                .describe(
                    "Horizontal center of the vignette as a percent of the image width: 0 = left \
                     edge, 50 (default) = middle, 100 = right edge.",
                ),
        )
        .param(
            Param::number("center_y")
                .min(0.0)
                .max(100.0)
                .default(DEFAULT_CENTER_PCT)
                .describe(
                    "Vertical center of the vignette as a percent of the image height: 0 = top \
                     edge, 50 (default) = middle, 100 = bottom edge.",
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
    name = "gizza-ai/image-vignette",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add a soft darkened or lightened vignette with adjustable strength around an image's edges.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Add a soft darkened (or lightened) vignette around the edges of an image. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call); optional strength 0-100 (default 40), mode darken|lighten (default darken), and center_x/center_y as percent of the image size (default 50/50, the middle).",
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid image-vignette args: {e}")))?;
    let strength = args.strength.unwrap_or(DEFAULT_STRENGTH);
    let mode = parse_mode(args.mode.as_deref()).map_err(SkillError::InvalidArgs)?;
    let center_x = args.center_x.unwrap_or(DEFAULT_CENTER_PCT);
    let center_y = args.center_y.unwrap_or(DEFAULT_CENTER_PCT);
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) =
        gizza_ai_image_vignette_core::plan(&in_path, strength, mode, center_x, center_y)
            .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, in_path, bytes, out_name)?;
    let for_llm = format!(
        "applied a {} vignette (strength {strength}) to {in_name}",
        mode_name(mode)
    );
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
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "strength": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                        "default": 40.0,
                        "description": "Vignette strength, 0-100. 0 leaves the image unchanged, 40 (the default) is a classic soft vignette, 100 fades the corners fully to black (or white in lighten mode). Example: 80 for a dramatic look."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["darken", "lighten"],
                        "default": "darken",
                        "description": "darken (default) darkens the edges like a classic lens vignette; lighten brightens them instead for a faded, hazy border."
                    },
                    "center_x": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                        "default": 50.0,
                        "description": "Horizontal center of the vignette as a percent of the image width: 0 = left edge, 50 (default) = middle, 100 = right edge."
                    },
                    "center_y": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                        "default": 50.0,
                        "description": "Vertical center of the vignette as a percent of the image height: 0 = top edge, 50 (default) = middle, 100 = bottom edge."
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
    fn descriptor_exposes_every_mode() {
        let json = schema_json();
        for name in MODES {
            assert!(json.contains(name), "schema enum must list {name}");
        }
    }
}
