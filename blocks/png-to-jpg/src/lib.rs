//! gizza-ai/png-to-jpg — flatten transparency onto a chosen background color
//! and encode JPEG, via ffmpeg on the shared tool abstraction.
//!
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. JPEG has no alpha channel, so the
//! core builds a split + flood-fill + overlay filtergraph that composites the
//! image onto the chosen `background` color (default white) before the mjpeg
//! encode — the same result a browser shows when rendering the PNG over that
//! color. The chat schema is derived from `descriptor()` (single source across
//! chat + CLI + page) and the drift-guard test below pins it.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, replace_extension, validate_quality_1_100, AssetKind,
    Input, Param, SkillError, SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_png_to_jpg_core::{plan, DEFAULT_BACKGROUND, DEFAULT_QUALITY};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    quality: Option<u8>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. background is the fill for transparent
    // areas (JPEG has no alpha); quality maps to mjpeg -q:v like image-convert.
    ToolDescriptor::new(Input::Image)
        .param(Param::string("background").default(DEFAULT_BACKGROUND).describe(
            "Background color that fills transparent areas (JPEG has no transparency): a CSS \
             color name (white, black, navy, …) or hex like #FFFFFF / #FFF. Default #ffffff \
             (white) — what browsers show a transparent PNG on.",
        ))
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .default(85)
                .describe(
                    "JPEG quality 1-100 (default 85). Higher keeps more detail but makes a \
                     bigger file; 100 is near-lossless, 60-80 is a good size/quality trade.",
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
    name = "gizza-ai/png-to-jpg",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a PNG (or any image) to JPG, flattening transparency onto a chosen background color.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert a PNG (or any image) to JPG. JPEG has no alpha channel, so transparent areas are flattened onto a chosen background color first — fully transparent pixels become that color and semi-transparent pixels blend onto it. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call); optional background (CSS color name or hex like #FFFFFF / #FFF, default #ffffff white) and quality 1-100 (default 85). Animated inputs convert their first frame.",
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid png-to-jpg args: {e}")))?;
    // Reject 0/out-of-range explicitly (core treats 0.0 as "unset" for the
    // page's cleared-field convention, but a typed 0 from chat/CLI is an error).
    validate_quality_1_100(args.quality, "png-to-jpg")?;
    let quality = args.quality.unwrap_or(DEFAULT_QUALITY);

    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) = plan(&in_path, args.background.as_deref(), quality as f64)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid png-to-jpg args: {e}")))?;
    let output = dispatch_ffmpeg(argv, in_path, bytes, out_name)?;

    let out_display = replace_extension(&in_name, "jpg");
    let bg = args
        .background
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_BACKGROUND);
    let for_llm = format!(
        "converted {in_name} to JPEG at quality {quality}, flattening transparency onto {bg}; \
         output {out_display} ({} bytes)",
        output.len()
    );
    build_media_envelope(&output, "image/jpeg", out_display, for_llm, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-guard: the descriptor-derived chat schema must match the authored
    /// schema below, so the LLM-facing tool definition never silently changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "background": {
                        "type": "string",
                        "default": "#ffffff",
                        "description": "Background color that fills transparent areas (JPEG has no transparency): a CSS color name (white, black, navy, …) or hex like #FFFFFF / #FFF. Default #ffffff (white) — what browsers show a transparent PNG on."
                    },
                    "quality": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 85,
                        "description": "JPEG quality 1-100 (default 85). Higher keeps more detail but makes a bigger file; 100 is near-lossless, 60-80 is a good size/quality trade."
                    }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_swaps_extension_to_jpg() {
        assert_eq!(replace_extension("logo.png", "jpg"), "logo.jpg");
        assert_eq!(replace_extension("sticker.webp", "jpg"), "sticker.jpg");
    }
}
