//! gizza-ai/image-color-picker — report the color at a pixel coordinate in an
//! image (hex, RGB(A), and nearest named color).
//!
//! Pipeline: resolve the image source (URL/ref) → `core::pick` (decode + pixel
//! lookup) → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report; the page file-input
//! path is ffmpeg-only — the F3 no-page file-input pattern, like detect-file-type).
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
    x: u32,
    y: u32,
}

#[derive(Serialize)]
struct Resp {
    hex: String,
    hex_rgba: String,
    rgb: String,
    rgba: String,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    /// Nearest common color name (approximate).
    nearest_name: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(Param::integer("x").required().min(0.0).describe("Pixel X coordinate (0 = left edge)."))
        .param(Param::integer("y").required().min(0.0).describe("Pixel Y coordinate (0 = top edge)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageColorPicker;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-color-picker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Report the color at a pixel in an image",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Report the color at a given pixel (x, y) in an image: hex (#rrggbb and #rrggbbaa), rgb()/rgba() strings, the individual r/g/b/a channels (0-255), and the nearest common color name, plus the image dimensions. Coordinates are 0-based from the top-left. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ImageColorPicker {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-color-picker")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let p = gizza_ai_image_color_picker_core::pick(&bytes, args.x, args.y)
        .map_err(SkillError::InvalidArgs)?;

    let resp = Resp {
        hex: gizza_ai_image_color_picker_core::hex(&p),
        hex_rgba: gizza_ai_image_color_picker_core::hex_rgba(&p),
        rgb: format!("rgb({}, {}, {})", p.r, p.g, p.b),
        rgba: format!("rgba({}, {}, {}, {:.3})", p.r, p.g, p.b, p.a as f64 / 255.0),
        r: p.r,
        g: p.g,
        b: p.b,
        a: p.a,
        nearest_name: gizza_ai_image_color_picker_core::nearest_name(p.r, p.g, p.b).to_string(),
        x: args.x,
        y: args.y,
        width: p.width,
        height: p.height,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize image-color-picker response: {e}")))
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
                    "x":   { "type": "integer", "minimum": 0, "description": "Pixel X coordinate (0 = left edge)." },
                    "y":   { "type": "integer", "minimum": 0, "description": "Pixel Y coordinate (0 = top edge)." }
                },
                "additionalProperties": false,
                "required": ["x", "y"],
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
