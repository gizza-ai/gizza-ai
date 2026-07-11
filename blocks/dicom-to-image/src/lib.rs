//! gizza-ai/dicom-to-image — decode an uncompressed grayscale DICOM (.dcm) file
//! and render one frame to a viewable PNG or JPEG, with automatic or manual
//! window/level (contrast). Pure-Rust (hand-written DICOM parser + the `image`
//! crate). Surfaces: chat + CLI (file input + image bytes output → no page, like
//! blur-image / svg-to-png).
//!
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI); the handler resolves the source bytes, calls the pure `core`
//! renderer, and wraps the result in the shared media envelope.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source, AssetKind};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_dicom_to_image_core::{render, OutputFormat, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 48 * 1024 * 1024; // 48 MiB — DICOM slices run large
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
    invert: Option<bool>,
    #[serde(default)]
    frame: Option<u32>,
    #[serde(default)]
    window_center: Option<f64>,
    #[serde(default)]
    window_width: Option<f64>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("format", ["png", "jpeg"])
                .default("png")
                .describe("Output image format (default: png). PNG is lossless; JPEG is smaller."),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .default(90)
                .describe("JPEG quality 1-100 (default 90; ignored for PNG)."),
        )
        .param(
            Param::boolean("invert")
                .default(false)
                .describe("Invert the grayscale (default false). MONOCHROME1/2 photometric inversion is already applied automatically; this toggles on top of it."),
        )
        .param(
            Param::integer("frame")
                .min(1.0)
                .default(1)
                .describe("1-based frame to render for multi-frame images (default 1)."),
        )
        .param(
            Param::number("window_center")
                .describe("Manual window centre (level). Overrides the file's embedded WindowCenter. Modality presets: brain ~40, lung ~-600, bone ~300, soft-tissue ~50. Omit to use the embedded value or auto contrast."),
        )
        .param(
            Param::number("window_width")
                .describe("Manual window width (contrast). Overrides the file's embedded WindowWidth. Modality presets: brain ~80, lung ~1500, bone ~1500, soft-tissue ~400. Omit to use the embedded value or auto contrast."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DicomToImage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dicom-to-image",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an uncompressed grayscale DICOM (.dcm) scan to a PNG or JPEG image.",
    requires = ["wafer-run/network"],
    skill(
        description = "Decode an uncompressed grayscale DICOM (.dcm) medical image (CT/MR/X-ray/ultrasound) and render one frame to a viewable PNG or JPEG. Reads Rows/Columns/BitsAllocated/PixelRepresentation/PhotometricInterpretation and the pixel data; applies rescale slope/intercept and window centre/width (embedded WindowCenter/WindowWidth, or your manual window_center/window_width, else auto contrast from the pixel range); honours MONOCHROME1/MONOCHROME2. Params: format (png|jpeg, default png), quality (JPEG 1-100, default 90), invert (bool), frame (1-based), window_center, window_width. Supports Implicit and Explicit VR Little Endian, 8/16-bit signed or unsigned. Compressed (JPEG/JPEG2000/JPEG-LS/RLE) or colour DICOM return a clear error. Provide the file as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl DicomToImage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("dicom-to-image")?;
    let format = OutputFormat::parse(args.format.as_deref().unwrap_or("png"))
        .map_err(SkillError::InvalidArgs)?;
    let quality = args.quality.unwrap_or(90).clamp(1, 100) as u8;
    let frame = args.frame.unwrap_or(1);
    if frame < 1 {
        return Err(SkillError::InvalidArgs(
            "frame must be >= 1 (frames are 1-based)".to_string(),
        ));
    }
    let opts = Options {
        format,
        quality,
        invert: args.invert.unwrap_or(false),
        frame,
        window_center: args.window_center,
        window_width: args.window_width,
    };

    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
    let out = render(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let filename = format!("dicom.{}", format.ext());
    build_media_envelope(
        &out,
        format.mime(),
        filename,
        format!(
            "DICOM rendered to {} ({} bytes, frame {})",
            format.ext().to_uppercase(),
            out.len(),
            frame
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
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": { "type": "string", "enum": ["png", "jpeg"], "default": "png", "description": "Output image format (default: png). PNG is lossless; JPEG is smaller." },
                    "quality": { "type": "integer", "minimum": 1, "maximum": 100, "default": 90, "description": "JPEG quality 1-100 (default 90; ignored for PNG)." },
                    "invert": { "type": "boolean", "default": false, "description": "Invert the grayscale (default false). MONOCHROME1/2 photometric inversion is already applied automatically; this toggles on top of it." },
                    "frame": { "type": "integer", "minimum": 1, "default": 1, "description": "1-based frame to render for multi-frame images (default 1)." },
                    "window_center": { "type": "number", "description": "Manual window centre (level). Overrides the file's embedded WindowCenter. Modality presets: brain ~40, lung ~-600, bone ~300, soft-tissue ~50. Omit to use the embedded value or auto contrast." },
                    "window_width": { "type": "number", "description": "Manual window width (contrast). Overrides the file's embedded WindowWidth. Modality presets: brain ~80, lung ~1500, bone ~1500, soft-tissue ~400. Omit to use the embedded value or auto contrast." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
