//! gizza-ai/image-to-ico — turn one source image (PNG/JPEG/WebP/GIF/BMP) into a
//! single multi-resolution Windows `.ico` favicon, returned as a downloadable
//! file.
//!
//! Pipeline: resolve the image source (URL/ref) → pure `core::to_ico` (decode +
//! resize each requested size + ICO encode via the `image` crate) → media
//! envelope (`image/x-icon`) the page/CLI can download.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a binary `.ico` output has no page render
//! mode — same F3 pattern as image-collage / favicon-generator).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_to_ico_core::{parse_color, parse_fit, parse_sizes, to_ico, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_sizes")]
    sizes: String,
    #[serde(default = "default_fit")]
    fit: String,
    #[serde(default = "default_bg")]
    background: String,
}
fn default_sizes() -> String {
    "16,32,48,64,128,256".to_string()
}
fn default_fit() -> String {
    "contain".to_string()
}
fn default_bg() -> String {
    "#00000000".to_string()
}

/// `Input::Image` emits the `url`⊕`ref` `oneOf` for the source picture.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::string("sizes")
                .default("16,32,48,64,128,256")
                .describe("Comma-separated square resolutions in pixels embedded in the .ico (each 1..=256). Default covers the common Windows + browser favicon set."),
        )
        .param(
            Param::enumv("fit", ["contain", "cover"])
                .default("contain")
                .describe("How to fit a non-square source: contain (pad to a square with the background color, keeps the whole image) or cover (scale-to-fill then center-crop)."),
        )
        .param(
            Param::string("background")
                .default("#00000000")
                .describe("Background fill for padded areas when fit=contain, as #rgb, #rrggbb, or #rrggbbaa (default transparent)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageToIco;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-to-ico",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a PNG/JPG image into a multi-resolution .ico favicon",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Convert one source image (PNG/JPEG/WebP/GIF/BMP) into a single multi-resolution Windows .ico favicon, returned as a downloadable file. The .ico embeds a square bitmap at each requested resolution so the browser/OS can pick the best fit. sizes is a comma-separated list of square resolutions in pixels, each 1..=256 (default 16,32,48,64,128,256). fit controls how a non-square source is squared: contain (pad with the background color) or cover (scale-to-fill then center-crop). background sets the pad color when fit=contain (#rrggbbaa, default transparent). Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ImageToIco {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("image-to-ico")?;

    let sizes = parse_sizes(&args.sizes).map_err(SkillError::InvalidArgs)?;
    let fit = parse_fit(&args.fit).map_err(SkillError::InvalidArgs)?;
    let background = parse_color(&args.background).map_err(SkillError::InvalidArgs)?;

    let (bytes, _mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let opts = Options {
        fit,
        background,
        sizes,
    };

    let ico = to_ico(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let out_name = ico_name(&in_name);
    build_media_envelope(
        &ico,
        "image/x-icon",
        out_name,
        format!("generated a multi-resolution .ico favicon ({} bytes)", ico.len()),
        MAX_OUTPUT_BYTES,
    )
}

/// Derive the output filename: swap the source extension for `.ico`.
#[cfg(target_arch = "wasm32")]
fn ico_name(in_name: &str) -> String {
    let stem = in_name.rsplit_once('.').map(|(s, _)| s).unwrap_or(in_name);
    let stem = if stem.is_empty() { "favicon" } else { stem };
    format!("{stem}.ico")
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
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "sizes":      { "type": "string", "default": "16,32,48,64,128,256", "description": "Comma-separated square resolutions in pixels embedded in the .ico (each 1..=256). Default covers the common Windows + browser favicon set." },
                    "fit":        { "type": "string", "enum": ["contain", "cover"], "default": "contain", "description": "How to fit a non-square source: contain (pad to a square with the background color, keeps the whole image) or cover (scale-to-fill then center-crop)." },
                    "background": { "type": "string", "default": "#00000000", "description": "Background fill for padded areas when fit=contain, as #rgb, #rrggbb, or #rrggbbaa (default transparent)." }
                },
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
