//! gizza-ai/image-trim — auto-crop uniform borders or whitespace around an
//! image. Returns PNG (or JPEG for JPEG input). Pure-Rust (image crate) — runs
//! on all backends incl. the chat SW. Surfaces: chat + CLI (image input + image
//! bytes output → no page, like normalize-image / image-color-quantize).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_trim_core::trim;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    color: Option<String>,
    #[serde(default = "default_tolerance")]
    tolerance: u64,
    #[serde(default)]
    padding: u64,
    #[serde(default = "default_background_percent")]
    background_percent: u64,
    #[serde(default = "default_format")]
    format: String,
}
fn default_mode() -> String {
    "auto".into()
}
fn default_tolerance() -> u64 {
    16
}
fn default_background_percent() -> u64 {
    100
}
fn default_format() -> String {
    "auto".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("mode", ["auto", "transparent", "color"])
                .default("auto")
                .describe("What counts as the border to trim: auto (default) samples the 4 corner pixels — mostly-transparent corners trim by alpha, otherwise the majority corner color; transparent trims fully/nearly transparent edges; color trims edges matching the color parameter."),
        )
        .param(
            Param::string("color")
                .describe("Border color as hex, #rgb or #rrggbb (e.g. #fff or #ffffff). Required with mode=color; with mode=auto it overrides corner detection; not used with mode=transparent."),
        )
        .param(
            Param::integer("tolerance")
                .min(0.0)
                .max(255.0)
                .default(16)
                .describe("How far a pixel may differ from the border (max per-channel distance, 0-255) and still be trimmed. 0 = exact match only; default 16 absorbs anti-aliasing and JPEG artifacts; large values can eat into the subject."),
        )
        .param(
            Param::integer("padding")
                .min(0.0)
                .max(500.0)
                .default(0)
                .describe("Pixels of the original border to keep on every side around the detected content, clamped to the image edges (0-500, default 0 = tight crop)."),
        )
        .param(
            Param::integer("background_percent")
                .min(50.0)
                .max(100.0)
                .default(100)
                .describe("An edge row/column is trimmed only while at least this percent of its pixels match the border (50-100, default 100 = every pixel). Lower slightly (e.g. 95) to trim borders containing stray noise pixels."),
        )
        .param(
            Param::enumv("format", ["auto", "png", "jpeg"])
                .default("auto")
                .describe("Output format: auto (default) keeps JPEG input as JPEG and everything else as PNG (transparency preserved); png forces PNG; jpeg forces JPEG quality 90 (transparency flattened onto white)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageTrim;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-trim",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Auto-crop uniform borders or whitespace around an image",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Auto-crop uniform borders or whitespace around an image (like ImageMagick -trim): detects the border — transparent padding or a solid color sampled from the 4 corners, or a color you give — and removes matching edge rows/columns. tolerance (0-255, default 16) absorbs anti-aliasing/JPEG noise, padding keeps N border pixels around the content, background_percent trims noisy borders with stray pixels. Returns PNG (JPEG input stays JPEG by default) and reports the pixels removed per side. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl ImageTrim {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-trim")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let (out, r) = trim(
        &bytes,
        &args.mode,
        args.color.as_deref(),
        args.tolerance,
        args.padding,
        args.background_percent,
        &args.format,
    )
    .map_err(SkillError::InvalidArgs)?;
    let (mime, ext) = match r.format {
        "jpeg" => ("image/jpeg", "jpg"),
        _ => ("image/png", "png"),
    };
    let summary = if r.trimmed {
        format!(
            "trimmed {}x{} -> {}x{} (removed left {}, top {}, right {}, bottom {}; background {}; {} bytes {})",
            r.orig_w,
            r.orig_h,
            r.w,
            r.h,
            r.removed_left,
            r.removed_top,
            r.removed_right,
            r.removed_bottom,
            r.background,
            out.len(),
            r.format
        )
    } else {
        format!(
            "no matching border found — image unchanged ({}x{}, background {}, {} bytes {})",
            r.orig_w,
            r.orig_h,
            r.background,
            out.len(),
            r.format
        )
    };
    build_media_envelope(
        &out,
        mime,
        format!("trimmed.{ext}"),
        summary,
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
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode": {
                        "type": "string",
                        "enum": ["auto", "transparent", "color"],
                        "default": "auto",
                        "description": "What counts as the border to trim: auto (default) samples the 4 corner pixels — mostly-transparent corners trim by alpha, otherwise the majority corner color; transparent trims fully/nearly transparent edges; color trims edges matching the color parameter."
                    },
                    "color": {
                        "type": "string",
                        "description": "Border color as hex, #rgb or #rrggbb (e.g. #fff or #ffffff). Required with mode=color; with mode=auto it overrides corner detection; not used with mode=transparent."
                    },
                    "tolerance": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 255,
                        "default": 16,
                        "description": "How far a pixel may differ from the border (max per-channel distance, 0-255) and still be trimmed. 0 = exact match only; default 16 absorbs anti-aliasing and JPEG artifacts; large values can eat into the subject."
                    },
                    "padding": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 500,
                        "default": 0,
                        "description": "Pixels of the original border to keep on every side around the detected content, clamped to the image edges (0-500, default 0 = tight crop)."
                    },
                    "background_percent": {
                        "type": "integer",
                        "minimum": 50,
                        "maximum": 100,
                        "default": 100,
                        "description": "An edge row/column is trimmed only while at least this percent of its pixels match the border (50-100, default 100 = every pixel). Lower slightly (e.g. 95) to trim borders containing stray noise pixels."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["auto", "png", "jpeg"],
                        "default": "auto",
                        "description": "Output format: auto (default) keeps JPEG input as JPEG and everything else as PNG (transparency preserved); png forces PNG; jpeg forces JPEG quality 90 (transparency flattened onto white)."
                    }
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
