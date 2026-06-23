//! gizza-ai/image-to-ascii — convert an image into ASCII / ANSI character art.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::convert` (decode +
//! resize + luminance ramp) → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report; the F3 no-page
//! file-input pattern, like image-color-picker / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

use gizza_ai_image_to_ascii_core::{convert, Options, Ramp};

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_ramp")]
    ramp: String,
    #[serde(default)]
    charset: String,
    #[serde(default)]
    color: bool,
    #[serde(default)]
    invert: bool,
    #[serde(default)]
    brightness: f64,
}

fn default_width() -> u32 {
    80
}
fn default_ramp() -> String {
    "standard".into()
}

#[derive(Serialize)]
struct Resp {
    /// The rendered character art (newline-separated rows; ANSI escapes if color).
    art: String,
    /// Output dimensions in characters.
    cols: u32,
    rows: u32,
    /// Source image dimensions in pixels.
    width: u32,
    height: u32,
    /// Which ramp was used.
    ramp: String,
    color: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("width")
                .min(1.0)
                .max(400.0)
                .default(80)
                .describe("Output width in characters (1-400). Default 80."),
        )
        .param(
            Param::enumv("ramp", ["standard", "detailed", "blocks"])
                .default("standard")
                .describe(
                    "Character ramp: standard (10-level), detailed (70-level), or blocks (Unicode shading).",
                ),
        )
        .param(
            Param::string("charset").describe(
                "Custom character ramp ordered dark→light (e.g. \" .:-=+*#%@\"). Overrides `ramp` when set; needs at least 2 characters.",
            ),
        )
        .param(
            Param::number("brightness")
                .min(-1.0)
                .max(1.0)
                .default(0.0)
                .describe("Brightness adjustment from -1.0 (darker) to 1.0 (lighter). Default 0."),
        )
        .param(
            Param::boolean("color")
                .default(false)
                .describe("Wrap each character in a 24-bit ANSI truecolor escape (for terminals)."),
        )
        .param(
            Param::boolean("invert")
                .default(false)
                .describe("Invert brightness (for light-on-dark terminals)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageToAscii;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-to-ascii",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an image into ASCII or ANSI character art",
    requires = ["wafer-run/network"],
    skill(
        description = "Convert an image into ASCII or ANSI character art. The image is scaled to `width` characters wide (1-400, default 80; height is derived from the aspect ratio and corrected for tall text cells) and each cell's brightness is mapped through a character ramp from dark to light. Choose a built-in ramp (standard, detailed, or blocks) or supply a custom `charset` ordered dark→light. Use `brightness` (-1.0 to 1.0) to lighten/darken, color=true to wrap each character in a 24-bit ANSI truecolor escape for terminals, or invert=true for light-on-dark terminals. Returns the art plus the output character dimensions and source pixel dimensions. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ImageToAscii {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-to-ascii")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let ramp = Ramp::parse(&args.ramp).map_err(SkillError::InvalidArgs)?;
    let charset = if args.charset.is_empty() {
        None
    } else {
        Some(args.charset.clone())
    };
    let opts = Options {
        cols: args.width,
        ramp,
        charset,
        color: args.color,
        invert: args.invert,
        brightness: args.brightness as f32,
    };
    let art = convert(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let resp = Resp {
        art: art.text,
        cols: art.cols,
        rows: art.rows,
        width: art.src_width,
        height: art.src_height,
        ramp: args.ramp,
        color: args.color,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize image-to-ascii response: {e}")))
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
                    "width": { "type": "integer", "minimum": 1, "maximum": 400, "default": 80, "description": "Output width in characters (1-400). Default 80." },
                    "ramp": { "type": "string", "enum": ["standard", "detailed", "blocks"], "default": "standard", "description": "Character ramp: standard (10-level), detailed (70-level), or blocks (Unicode shading)." },
                    "charset": { "type": "string", "description": "Custom character ramp ordered dark→light (e.g. \" .:-=+*#%@\"). Overrides `ramp` when set; needs at least 2 characters." },
                    "brightness": { "type": "number", "minimum": -1, "maximum": 1, "default": 0.0, "description": "Brightness adjustment from -1.0 (darker) to 1.0 (lighter). Default 0." },
                    "color": { "type": "boolean", "default": false, "description": "Wrap each character in a 24-bit ANSI truecolor escape (for terminals)." },
                    "invert": { "type": "boolean", "default": false, "description": "Invert brightness (for light-on-dark terminals)." }
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
