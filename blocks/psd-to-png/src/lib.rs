//! gizza-ai/psd-to-png — decode an Adobe Photoshop `.psd` document and render its
//! flattened composite (all visible layers merged) to a viewable PNG or JPEG.
//! Pure-Rust (`psd` + the `image` crate). Surfaces: chat + CLI (file input +
//! image bytes output → no page, like dicom-to-image / blur-image / svg-to-png).
//!
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI); the handler resolves the source bytes, calls the pure `core`
//! renderer, and wraps the result in the shared media envelope.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source, AssetKind};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_psd_to_png_core::{parse_hex_color, render, OutputFormat, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024; // 64 MiB — PSDs run large
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    quality: Option<u32>,
    #[serde(default)]
    background: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("format", ["png", "jpeg"])
                .default("png")
                .describe("Output image format (default: png). PNG keeps the document's transparency; JPEG is smaller but has no alpha."),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .default(90)
                .describe("JPEG quality 1-100 (default 90; ignored for PNG)."),
        )
        .param(
            Param::string("background")
                .default("#ffffff")
                .describe("Background colour for JPEG output as hex (#rgb or #rrggbb, default #ffffff). Transparent areas are flattened onto this colour. Ignored for PNG, which keeps transparency."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PsdToPng;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/psd-to-png",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an Adobe Photoshop PSD to a flattened PNG or JPEG image.",
    requires = ["wafer-run/network"],
    skill(
        description = "Decode an Adobe Photoshop .psd document and render its FLATTENED composite (all visible layers merged, exactly as Photoshop previews it) to a viewable PNG or JPEG. Output is a single image of the whole canvas — per-layer extraction to separate files is not supported. PNG preserves the document's transparency; JPEG has no alpha, so transparent areas are flattened onto the background colour. Params: format (png|jpeg, default png), quality (JPEG 1-100, default 90), background (hex #rrggbb for JPEG flatten, default #ffffff). Renders RGB/RGBA PSDs from the document's stored composite; a file that is not a PSD returns a clear error. Provide the file as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl PsdToPng {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("psd-to-png")?;
    let format = OutputFormat::parse(args.format.as_deref().unwrap_or("png"))
        .map_err(SkillError::InvalidArgs)?;
    let quality = args.quality.unwrap_or(90).clamp(1, 100) as u8;
    let background = parse_hex_color(args.background.as_deref().unwrap_or("#ffffff"))
        .map_err(SkillError::InvalidArgs)?;
    let opts = Options {
        format,
        quality,
        background,
    };

    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
    let out = render(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let filename = format!("psd.{}", format.ext());
    build_media_envelope(
        &out,
        format.mime(),
        filename,
        format!(
            "PSD flattened to {} ({} bytes)",
            format.ext().to_uppercase(),
            out.len(),
        ),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": { "type": "string", "enum": ["png", "jpeg"], "default": "png", "description": "Output image format (default: png). PNG keeps the document's transparency; JPEG is smaller but has no alpha." },
                    "quality": { "type": "integer", "minimum": 1, "maximum": 100, "default": 90, "description": "JPEG quality 1-100 (default 90; ignored for PNG)." },
                    "background": { "type": "string", "default": "#ffffff", "description": "Background colour for JPEG output as hex (#rgb or #rrggbb, default #ffffff). Transparent areas are flattened onto this colour. Ignored for PNG, which keeps transparency." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
