//! gizza-ai/image-drop-shadow — add a soft drop shadow behind a
//! transparent-PNG cutout (product shots, stickers, logos) via ffmpeg, on the
//! shared tool abstraction.
//!
//! `Input::Image` emits the url⊕ref oneOf; run() is
//! resolve_source → core::plan → dispatch_ffmpeg → build_media_envelope. The
//! shadow follows the input's ALPHA channel (the CSS `filter: drop-shadow()`
//! model), so a cutout casts a silhouette-shaped shadow rather than a
//! rectangular one — see the core for the filtergraph.
//!
//! By default the canvas is expanded so the shadow can never be clipped;
//! `padding` sets an explicit margin and `clip_to_original` keeps the input's
//! exact dimensions instead. The chat schema is derived from `descriptor()`
//! (the single source shared across chat + CLI + page) and the drift-guard
//! test below proves it matches the authored schema.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, replace_extension, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_drop_shadow_core::{
    parse_background, parse_color, parse_format, plan, Shadow, DEFAULT_BACKGROUND, DEFAULT_BLUR,
    DEFAULT_COLOR, DEFAULT_FORMAT, DEFAULT_OFFSET_X, DEFAULT_OFFSET_Y, DEFAULT_OPACITY,
    DEFAULT_PADDING, FORMATS, MAX_BLUR, MAX_OFFSET, MAX_PADDING,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    offset_x: Option<f64>,
    #[serde(default)]
    offset_y: Option<f64>,
    #[serde(default)]
    blur: Option<f64>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    opacity: Option<f64>,
    #[serde(default)]
    padding: Option<f64>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    clip_to_original: Option<bool>,
    #[serde(default)]
    format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. Param ORDER is load-bearing: the page's
    // meta.toml fields and web/build_argv arguments must line up with it.
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("offset_x")
                .min(-MAX_OFFSET)
                .max(MAX_OFFSET)
                .default(DEFAULT_OFFSET_X)
                .describe(
                    "How far the shadow is shifted horizontally, in pixels. Positive moves it \
                     right (12 is the default), negative moves it left. Example: -10 for light \
                     coming from the right.",
                ),
        )
        .param(
            Param::number("offset_y")
                .min(-MAX_OFFSET)
                .max(MAX_OFFSET)
                .default(DEFAULT_OFFSET_Y)
                .describe(
                    "How far the shadow is shifted vertically, in pixels. Positive moves it down \
                     (16 is the default, like overhead light), negative moves it up. Example: 40 \
                     for an object floating high above a surface.",
                ),
        )
        .param(
            Param::number("blur")
                .min(0.0)
                .max(MAX_BLUR)
                .default(DEFAULT_BLUR)
                .describe(
                    "Shadow blur radius in pixels, the same units CSS drop-shadow uses (the \
                     Gaussian sigma is half of it). 0 gives a hard-edged sticker shadow, 24 (the \
                     default) a soft one, 60+ a wide ambient glow.",
                ),
        )
        .param(Param::string("color").default(DEFAULT_COLOR).describe(
            "Shadow color: hex like #000000 (the default), #333 or 0x1A2B3C, or a name such as \
             black, charcoal, slate, navy or red. Any alpha in the hex is ignored — use the \
             opacity parameter for transparency.",
        ))
        .param(
            Param::number("opacity")
                .min(0.0)
                .max(100.0)
                .default(DEFAULT_OPACITY)
                .describe(
                    "Shadow opacity in percent, 0-100. 35 (the default) reads as a realistic \
                     cast shadow; 0 makes the shadow invisible and 100 makes it fully solid. \
                     Example: 60 for a heavier, contactier shadow.",
                ),
        )
        .param(
            Param::number("padding")
                .min(0.0)
                .max(MAX_PADDING)
                .default(DEFAULT_PADDING)
                .describe(
                    "Transparent margin added to every side, in pixels, so the shadow has room. \
                     0 (the default) means AUTO: enough for the blur reach plus the offset, so \
                     the shadow is never cut off. Set a number to control the margin exactly, \
                     e.g. 80 for a roomy product-shot canvas. Ignored when clip_to_original is \
                     true.",
                ),
        )
        .param(
            Param::string("background")
                .default(DEFAULT_BACKGROUND)
                .describe(
                    "Canvas fill behind the shadow: transparent (the default) keeps the PNG \
                     see-through; a hex value like #FFFFFF or a name like white/beige/slate \
                     fills it with a solid opaque color. JPEG output cannot store transparency, \
                     so it flattens onto white unless you set a color here.",
                ),
        )
        .param(
            Param::boolean("clip_to_original")
                .default(false)
                .describe(
                    "Keep the input's exact width and height instead of growing the canvas: the \
                     shadow is then clipped wherever it falls outside the original frame. \
                     Default false (the canvas grows to fit the shadow). Turn it on when the \
                     output must stay a fixed size, e.g. replacing an existing asset.",
                ),
        )
        .param(Param::enumv("format", FORMATS).default(DEFAULT_FORMAT).describe(
            "Output image format: png (the default, lossless with full transparency), webp \
             (smaller, also keeps transparency), or jpg (smallest, NO transparency — the canvas \
             is flattened onto the background color, white by default).",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-drop-shadow",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add a soft drop shadow behind a transparent-PNG cutout, with adjustable offset, blur, color and opacity.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Add a realistic soft drop shadow behind a transparent-PNG cutout (product shots, stickers, logos). The shadow follows the image's alpha channel, so it takes the shape of the subject, not a rectangle. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call); optional offset_x/offset_y in px (defaults 12/16, negative shifts left/up), blur radius in px (default 24, 0 = hard edge), color (hex or name, default #000000), opacity 0-100 percent (default 35), padding px (default 0 = auto-fit so the shadow is never clipped), background (transparent by default, or a solid color), clip_to_original true to keep the input's exact dimensions, and format png|webp|jpg (default png). An image with no transparency casts a plain rectangular shadow — remove the background first for a shaped one.",
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid image-drop-shadow args: {e}")))?;
    let shadow = Shadow {
        offset_x: args.offset_x.unwrap_or(DEFAULT_OFFSET_X),
        offset_y: args.offset_y.unwrap_or(DEFAULT_OFFSET_Y),
        blur: args.blur.unwrap_or(DEFAULT_BLUR),
        color: parse_color(args.color.as_deref()).map_err(SkillError::InvalidArgs)?,
        opacity: args.opacity.unwrap_or(DEFAULT_OPACITY),
        padding: args.padding.unwrap_or(DEFAULT_PADDING),
        background: parse_background(args.background.as_deref())
            .map_err(SkillError::InvalidArgs)?,
        clip_to_original: args.clip_to_original.unwrap_or(false),
        format: parse_format(args.format.as_deref()).map_err(SkillError::InvalidArgs)?,
    };

    let (bytes, mime, in_display) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) = plan(&in_path, &shadow).map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, in_path, bytes, out_name)?;

    let out_display = replace_extension(&in_display, shadow.format.ext());
    let (r, g, b) = shadow.color;
    let canvas = if shadow.clip_to_original {
        "original size kept"
    } else {
        "canvas expanded to fit"
    };
    let for_llm = format!(
        "added a drop shadow to {in_display} (offset {}x{} px, blur {} px, #{r:02X}{g:02X}{b:02X} \
         at {}% opacity, {canvas}) → {out_display}",
        shadow.offset_x, shadow.offset_y, shadow.blur, shadow.opacity
    );
    build_media_envelope(&output, shadow.format.mime(), out_display, for_llm, MAX_BYTES)
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
                    "offset_x": {
                        "type": "number",
                        "minimum": -500,
                        "maximum": 500,
                        "default": 12.0,
                        "description": "How far the shadow is shifted horizontally, in pixels. Positive moves it right (12 is the default), negative moves it left. Example: -10 for light coming from the right."
                    },
                    "offset_y": {
                        "type": "number",
                        "minimum": -500,
                        "maximum": 500,
                        "default": 16.0,
                        "description": "How far the shadow is shifted vertically, in pixels. Positive moves it down (16 is the default, like overhead light), negative moves it up. Example: 40 for an object floating high above a surface."
                    },
                    "blur": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 400,
                        "default": 24.0,
                        "description": "Shadow blur radius in pixels, the same units CSS drop-shadow uses (the Gaussian sigma is half of it). 0 gives a hard-edged sticker shadow, 24 (the default) a soft one, 60+ a wide ambient glow."
                    },
                    "color": {
                        "type": "string",
                        "default": "#000000",
                        "description": "Shadow color: hex like #000000 (the default), #333 or 0x1A2B3C, or a name such as black, charcoal, slate, navy or red. Any alpha in the hex is ignored — use the opacity parameter for transparency."
                    },
                    "opacity": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                        "default": 35.0,
                        "description": "Shadow opacity in percent, 0-100. 35 (the default) reads as a realistic cast shadow; 0 makes the shadow invisible and 100 makes it fully solid. Example: 60 for a heavier, contactier shadow."
                    },
                    "padding": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 2000,
                        "default": 0.0,
                        "description": "Transparent margin added to every side, in pixels, so the shadow has room. 0 (the default) means AUTO: enough for the blur reach plus the offset, so the shadow is never cut off. Set a number to control the margin exactly, e.g. 80 for a roomy product-shot canvas. Ignored when clip_to_original is true."
                    },
                    "background": {
                        "type": "string",
                        "default": "transparent",
                        "description": "Canvas fill behind the shadow: transparent (the default) keeps the PNG see-through; a hex value like #FFFFFF or a name like white/beige/slate fills it with a solid opaque color. JPEG output cannot store transparency, so it flattens onto white unless you set a color here."
                    },
                    "clip_to_original": {
                        "type": "boolean",
                        "default": false,
                        "description": "Keep the input's exact width and height instead of growing the canvas: the shadow is then clipped wherever it falls outside the original frame. Default false (the canvas grows to fit the shadow). Turn it on when the output must stay a fixed size, e.g. replacing an existing asset."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["png", "webp", "jpg"],
                        "default": "png",
                        "description": "Output image format: png (the default, lossless with full transparency), webp (smaller, also keeps transparency), or jpg (smallest, NO transparency — the canvas is flattened onto the background color, white by default)."
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
    fn descriptor_exposes_every_output_format() {
        let json = schema_json();
        for name in FORMATS {
            assert!(json.contains(name), "schema format enum must list {name}");
        }
    }

    #[test]
    fn every_param_is_described() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for (name, prop) in props {
            let d = prop["description"].as_str().unwrap_or("");
            assert!(d.len() > 20, "param {name} needs a usable description");
        }
    }
}
