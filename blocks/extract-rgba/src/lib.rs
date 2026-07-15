//! gizza-ai/extract-rgba — decode an image and export its raw per-pixel RGBA8
//! values as text, CSV, or JSON for inspection or scripting.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::extract` (decode +
//! render) → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report; the page file-input
//! path is ffmpeg-only — the F3 no-page file-input pattern, like image-info).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    format: String,
    /// Cap the number of pixels emitted (0 = no cap). f64 to dodge the wasm
    /// BigInt gotcha; clamped to a non-negative integer.
    #[serde(default)]
    max_pixels: f64,
}

#[derive(Serialize)]
struct Resp {
    format: String,
    width: u32,
    height: u32,
    total_pixels: u64,
    emitted_pixels: u64,
    truncated: bool,
    /// The rendered RGBA values in the requested format.
    data: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("format", ["json", "text", "csv"]).default("json").describe(
                "Output format for the per-pixel RGBA values: json (default; {width,height,pixels:[[r,g,b,a],...]}), text (one `r,g,b,a` line per pixel), or csv (`x,y,r,g,b,a` rows with a header). Pixels are row-major (left to right, top to bottom).",
            ),
        )
        .param(
            Param::integer("max_pixels").min(0.0).default(0).describe(
                "Maximum number of pixels to emit, capping huge images (0 = no cap, emit every pixel).",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractRgba;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-rgba",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Export an image's raw per-pixel RGBA values as text, CSV, or JSON",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Decode an image and export its raw per-pixel RGBA values (0-255 red, green, blue, alpha) as text, CSV, or JSON for inspection or scripting. Pixels are emitted in row-major order (left to right, top to bottom). Use `format` to choose text/csv/json and `max_pixels` to cap the output for large images. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ExtractRgba {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("extract-rgba")?;
    let fmt = gizza_ai_extract_rgba_core::Format::parse(&args.format)
        .map_err(SkillError::InvalidArgs)?;
    let max_pixels = if args.max_pixels.is_finite() && args.max_pixels > 0.0 {
        args.max_pixels as u64
    } else {
        0
    };

    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let e = gizza_ai_extract_rgba_core::extract(&bytes, fmt, max_pixels)
        .map_err(SkillError::InvalidArgs)?;

    let format_name = match fmt {
        gizza_ai_extract_rgba_core::Format::Json => "json",
        gizza_ai_extract_rgba_core::Format::Text => "text",
        gizza_ai_extract_rgba_core::Format::Csv => "csv",
    };

    let resp = Resp {
        format: format_name.to_string(),
        width: e.width,
        height: e.height,
        total_pixels: e.total_pixels,
        emitted_pixels: e.emitted_pixels,
        truncated: e.truncated,
        data: e.data,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize extract-rgba response: {e}")))
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
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": { "type": "string", "enum": ["json", "text", "csv"], "default": "json", "description": "Output format for the per-pixel RGBA values: json (default; {width,height,pixels:[[r,g,b,a],...]}), text (one `r,g,b,a` line per pixel), or csv (`x,y,r,g,b,a` rows with a header). Pixels are row-major (left to right, top to bottom)." },
                    "max_pixels": { "type": "integer", "minimum": 0, "default": 0, "description": "Maximum number of pixels to emit, capping huge images (0 = no cap, emit every pixel)." }
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
