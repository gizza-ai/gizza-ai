//! gizza-ai/signature-extract — pull a handwritten signature off its paper and
//! return a clean transparent PNG (just the ink), ready to drop into a PDF or
//! e-sign form.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::extract_signature`
//! (decode + luminance knock-out + optional recolor + optional trim via the
//! `image` crate) → transparent-PNG media envelope.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + image bytes output; the page
//! file-input path is ffmpeg-only — like image-document-shadow-remove /
//! image-to-sketch).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_signature_extract_core::{extract_signature, parse_ink};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_threshold")]
    threshold: u32,
    #[serde(default = "default_ink")]
    ink: String,
    #[serde(default = "default_true")]
    smooth: bool,
    #[serde(default = "default_true")]
    trim: bool,
}
fn default_threshold() -> u32 {
    50
}
fn default_ink() -> String {
    "original".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("threshold")
                .min(0.0)
                .max(100.0)
                .default(50)
                .describe("Ink sensitivity, 0-100 (default 50). Pixels darker than this luminance cut-off are kept as ink; the lighter paper is knocked out to transparent. Raise it to catch faint/pencil strokes, lower it if paper texture bleeds through."),
        )
        .param(
            Param::enumv("ink", ["original", "black", "blue", "red"])
                .default("original")
                .describe("Recolour the extracted signature: original (keep the pen's own colour, default), or a solid black / blue / red 'wet ink' shade."),
        )
        .param(
            Param::boolean("smooth")
                .default(true)
                .describe("Anti-alias the stroke edges with a soft alpha ramp (default true) for a natural look; false gives hard 1-bit edges."),
        )
        .param(
            Param::boolean("trim")
                .default(true)
                .describe("Crop the transparent margin down to the signature's bounding box (default true), so the PNG is tight to the ink."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SignatureExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/signature-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract a handwritten signature from a photo as a transparent PNG",
    requires = ["wafer-run/network"],
    skill(
        description = "Extract a handwritten signature from a photo or scan of a signed page and return it as a clean transparent PNG (just the ink, paper knocked out) — ready to drop into a PDF, contract, or e-sign form. threshold (0-100, default 50) is the ink sensitivity: pixels darker than the cut-off are kept, lighter paper is made transparent; raise it for faint strokes. ink recolours the result: 'original' keeps the pen colour (default), or 'black'/'blue'/'red' for a solid wet-ink look. smooth (default true) anti-aliases the edges; trim (default true) crops the transparent margin to the signature. Best on dark ink over light paper. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl SignatureExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("signature-extract")?;
    let ink = parse_ink(&args.ink).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let png = extract_signature(&bytes, args.threshold, ink, args.smooth, args.trim)
        .map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &png,
        "image/png",
        "signature.png".to_string(),
        format!("extracted signature ({} ink, {} bytes transparent PNG)", args.ink, png.len()),
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
                    "url":       { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "threshold": { "type": "integer", "minimum": 0, "maximum": 100, "default": 50, "description": "Ink sensitivity, 0-100 (default 50). Pixels darker than this luminance cut-off are kept as ink; the lighter paper is knocked out to transparent. Raise it to catch faint/pencil strokes, lower it if paper texture bleeds through." },
                    "ink":       { "type": "string", "enum": ["original", "black", "blue", "red"], "default": "original", "description": "Recolour the extracted signature: original (keep the pen's own colour, default), or a solid black / blue / red 'wet ink' shade." },
                    "smooth":    { "type": "boolean", "default": true, "description": "Anti-alias the stroke edges with a soft alpha ramp (default true) for a natural look; false gives hard 1-bit edges." },
                    "trim":      { "type": "boolean", "default": true, "description": "Crop the transparent margin down to the signature's bounding box (default true), so the PNG is tight to the ink." }
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
