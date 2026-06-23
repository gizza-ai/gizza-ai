//! gizza-ai/gradient-image-generator — create a solid, linear-, or radial-gradient
//! image at a chosen size, as a PNG (raster) or SVG (scalable).
//!
//! Pure-Rust (`image` for PNG; hand-built SVG), so it runs on ALL backends incl.
//! the chat Service Worker. The image is wrapped as an `image/png` or
//! `image/svg+xml` data-URL envelope. Surfaces: chat + CLI (image-bytes output →
//! no page, like the qr-code / chart tools).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_gradient_image_generator_core::{generate, Format, GradientType};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(default = "default_gradient_type")]
    gradient_type: String,
    #[serde(default = "default_colors")]
    colors: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    /// Linear direction in degrees. f64 (wasm BigInt gotcha).
    #[serde(default = "default_angle")]
    angle: f64,
    #[serde(default = "default_format")]
    format: String,
}
fn default_gradient_type() -> String {
    "linear".to_string()
}
fn default_colors() -> String {
    "#2196f3,#9c27b0".to_string()
}
fn default_width() -> u32 {
    1024
}
fn default_height() -> u32 {
    1024
}
fn default_angle() -> f64 {
    90.0
}
fn default_format() -> String {
    "png".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("gradient_type", ["solid", "linear", "radial"]).default("linear").describe(
                "The fill to render: solid (a flat colour — uses the first stop), linear (a straight gradient along the angle direction, default), or radial (a gradient from the centre outward).",
            ),
        )
        .param(
            Param::string("colors").default("#2196f3,#9c27b0").describe(
                "The colour stops, separated by commas, spaces, or newlines. Each is a hex colour (#rgb, #rgba, #rrggbb, or #rrggbbaa — alpha supported). The stops are spread evenly across the gradient. Solid mode uses only the first stop. At least one colour is required.",
            ),
        )
        .param(
            Param::integer("width")
                .default(1024)
                .min(1.0)
                .max(4096.0)
                .describe("Image width in pixels (1-4096)."),
        )
        .param(
            Param::integer("height")
                .default(1024)
                .min(1.0)
                .max(4096.0)
                .describe("Image height in pixels (1-4096)."),
        )
        .param(
            Param::number("angle")
                .default(90.0)
                .min(0.0)
                .max(360.0)
                .describe("For linear gradients: the direction in degrees, measured clockwise (0 = left→right, 90 = top→bottom — the default, 180 = right→left). Ignored for solid and radial."),
        )
        .param(
            Param::enumv("format", ["png", "svg"]).default("png").describe(
                "Output image format: png (raster, sized by width/height, default) or svg (scalable vector; the width/height set the document size).",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GradientGen;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gradient-image-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Create a solid, linear- or radial-gradient image (PNG or SVG)",
    skill(
        description = "Create a solid, linear-gradient, or radial-gradient image at a chosen size, for backgrounds, hero banners, wallpapers, and placeholders. Set gradient_type to solid (a flat colour from the first stop), linear (a straight gradient along the angle direction), or radial (a gradient radiating from the centre). colors is a comma/space/newline-separated list of hex stops (#rgb, #rgba, #rrggbb, or #rrggbbaa — alpha supported) spread evenly across the gradient. width and height set the size in pixels (1-4096). angle sets the linear direction in degrees, clockwise (0 = left→right, 90 = top→bottom; ignored for solid/radial). Choose format png (raster) or svg (scalable). Returns an image. Runs locally — nothing leaves the device.",
        parameters = schema_json()
    ),
)]
impl GradientGen {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("gradient-image-generator")?;
    let gradient_type = GradientType::parse(&args.gradient_type).map_err(SkillError::InvalidArgs)?;
    let format = Format::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    let g = generate(gradient_type, &args.colors, args.width, args.height, args.angle, format)
        .map_err(SkillError::InvalidArgs)?;
    let n = g.bytes.len();
    build_media_envelope(
        &g.bytes,
        g.format.mime(),
        format!("gradient.{}", g.format.ext()),
        format!(
            "{} gradient {}x{} ({} image, {} stops, {n} bytes)",
            g.gradient_type.label(),
            g.width,
            g.height,
            g.format.ext().to_uppercase(),
            g.stop_count
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
                    "gradient_type": { "type": "string", "enum": ["solid", "linear", "radial"], "default": "linear", "description": "The fill to render: solid (a flat colour — uses the first stop), linear (a straight gradient along the angle direction, default), or radial (a gradient from the centre outward)." },
                    "colors": { "type": "string", "default": "#2196f3,#9c27b0", "description": "The colour stops, separated by commas, spaces, or newlines. Each is a hex colour (#rgb, #rgba, #rrggbb, or #rrggbbaa — alpha supported). The stops are spread evenly across the gradient. Solid mode uses only the first stop. At least one colour is required." },
                    "width": { "type": "integer", "default": 1024, "minimum": 1, "maximum": 4096, "description": "Image width in pixels (1-4096)." },
                    "height": { "type": "integer", "default": 1024, "minimum": 1, "maximum": 4096, "description": "Image height in pixels (1-4096)." },
                    "angle": { "type": "number", "default": 90.0, "minimum": 0, "maximum": 360, "description": "For linear gradients: the direction in degrees, measured clockwise (0 = left→right, 90 = top→bottom — the default, 180 = right→left). Ignored for solid and radial." },
                    "format": { "type": "string", "enum": ["png", "svg"], "default": "png", "description": "Output image format: png (raster, sized by width/height, default) or svg (scalable vector; the width/height set the document size)." }
                },
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
