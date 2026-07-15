//! gizza-ai/image-border-frame — add a solid border/frame of a chosen color and
//! thickness around an image. Returns a PNG. Pure Rust (image crate) — runs on
//! all backends incl. the chat SW. Surfaces: chat + CLI (image input + image
//! bytes output → no page, like normalize-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_image_border_frame_core::{add_border, parse_color};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_thickness")]
    thickness: u64,
    #[serde(default = "default_color")]
    color: String,
}
fn default_thickness() -> u64 {
    20
}
fn default_color() -> String {
    "#000000".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("thickness")
                .min(0.0)
                .max(2000.0)
                .default(20)
                .describe("Border thickness in pixels on every side (default 20)."),
        )
        .param(
            Param::string("color")
                .default("#000000")
                .describe("Border color as #rgb, #rrggbb, or #rrggbbaa (default black)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageBorderFrame;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-border-frame",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add a colored border/frame to an image",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Add a solid border/frame of a chosen color and thickness around an image. thickness is the border width in pixels on every side (default 20); color is #rgb/#rrggbb/#rrggbbaa (default black). The output is larger than the input by 2×thickness in each dimension. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageBorderFrame {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-border-frame")?;
    let color = parse_color(&args.color).map_err(SkillError::InvalidArgs)?;
    let thickness = args.thickness.min(2000) as u32;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = add_border(&bytes, thickness, color).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "framed.png".to_string(),
        format!("added a {thickness}px {} border ({} bytes PNG)", args.color, png.len()),
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
                    "url":       { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "thickness": { "type": "integer", "minimum": 0, "maximum": 2000, "default": 20, "description": "Border thickness in pixels on every side (default 20)." },
                    "color":     { "type": "string", "default": "#000000", "description": "Border color as #rgb, #rrggbb, or #rrggbbaa (default black)." }
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
