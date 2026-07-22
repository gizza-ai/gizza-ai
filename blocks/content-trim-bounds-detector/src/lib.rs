//! gizza-ai/content-trim-bounds-detector — find the tight bounding box of the
//! non-background / non-transparent content in an image and report the crop (and
//! per-side trim margins) that would remove the surrounding empty margin. It
//! MEASURES only — the sibling `image-trim` tool actually crops.
//! Pure Rust image analysis; chat + CLI JSON report (no standalone page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_content_trim_bounds_detector_core::{detect, BoundsReport};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_mode")]
    mode: String,
    #[serde(default)]
    color: Option<String>,
    #[serde(default = "d_tolerance")]
    tolerance: f64,
    #[serde(default = "d_background_percent")]
    background_percent: f64,
    #[serde(default = "d_padding")]
    padding: f64,
}

fn d_mode() -> String {
    "auto".into()
}
fn d_tolerance() -> f64 {
    16.0
}
fn d_background_percent() -> f64 {
    100.0
}
fn d_padding() -> f64 {
    0.0
}

#[derive(Serialize)]
struct Resp {
    orig_width: u32,
    orig_height: u32,
    has_content: bool,
    content_x: u32,
    content_y: u32,
    content_width: u32,
    content_height: u32,
    crop_x: u32,
    crop_y: u32,
    crop_width: u32,
    crop_height: u32,
    trim_left: u32,
    trim_top: u32,
    trim_right: u32,
    trim_bottom: u32,
    needs_trim: bool,
    content_fraction: f64,
    background: String,
    note: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("mode", ["auto", "transparent", "color"])
                .default("auto")
                .describe("How to decide what is background: auto (default) reads the alpha channel if the corners are transparent, else votes a solid color from the 4 corners; transparent keys only on the alpha channel; color compares against the color parameter. Pass color in either auto or color mode to force a specific background."),
        )
        .param(
            Param::string("color")
                .describe("Background color as hex (#rgb or #rrggbb, e.g. #fff or #ffffff). Required for mode=color; optional in mode=auto to override the corner vote; must be omitted for mode=transparent."),
        )
        .param(
            Param::integer("tolerance")
                .min(0.0)
                .max(255.0)
                .default(16)
                .describe("Color/alpha match tolerance 0-255 (default 16): a pixel counts as background when its max per-channel distance from the background color (and its distance from fully opaque) is at or below this. 0 = exact match; raise it to absorb JPEG noise or anti-aliased edges."),
        )
        .param(
            Param::integer("background_percent")
                .min(50.0)
                .max(100.0)
                .default(100)
                .describe("Percent of a row/column that must match the background for that edge line to be trimmed, 50-100 (default 100 = every pixel must match). Lower it (e.g. 98) to trim through a few stray non-background pixels in an otherwise empty margin."),
        )
        .param(
            Param::integer("padding")
                .min(0.0)
                .max(500.0)
                .default(0)
                .describe("Pixels of the original margin to keep back around the detected content in the suggested crop, 0-500 (default 0 = tight box). Clamped to the image edges — no synthetic pixels are added."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ContentTrimBoundsDetector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/content-trim-bounds-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect an image's tight content bounding box and report the crop that trims the uniform or transparent margin",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Find the tight bounding box of the real content in an image (everything that is not the uniform background or fully transparent padding) and report the crop that would remove the surrounding empty margin — without producing a cropped image. Params: mode=auto|transparent|color (default auto: alpha if corners are transparent, else a solid color voted from the 4 corners), color = hex background like #fff/#ffffff (required for mode=color, optional override in auto), tolerance = 0-255 color/alpha match slack (default 16), background_percent = 50-100 fraction of an edge line that must match to trim it (default 100), padding = 0-500 px of margin to keep back in the suggested crop (default 0). Returns JSON with orig_width/height, has_content, the tight content_x/y/width/height, the suggested crop_x/y/width/height (after padding), per-side trim_left/top/right/bottom margins, needs_trim, content_fraction (0-1), and the detected background. This tool only MEASURES — pass the reported crop box to image-trim to actually crop. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ContentTrimBoundsDetector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("content-trim-bounds-detector")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let report = detect(
        &bytes,
        &args.mode,
        args.color.as_deref(),
        args.tolerance as u64,
        args.background_percent as u64,
        args.padding as u64,
    )
    .map_err(SkillError::InvalidArgs)?;
    let resp = response(report);
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize content-trim-bounds-detector response: {e}"))
    })
}

fn response(r: BoundsReport) -> Resp {
    let note = if !r.has_content {
        format!(
            "The whole image is background ({}); there is no content to keep — nothing to crop.",
            r.background
        )
    } else if !r.needs_trim {
        "The content already fills the image within tolerance; there is no margin to trim.".to_string()
    } else {
        format!(
            "Trim {} left, {} top, {} right, {} bottom — or crop to {}x{}+{}+{} — to remove the {} margin; pass that box to image-trim to actually crop.",
            r.trim_left,
            r.trim_top,
            r.trim_right,
            r.trim_bottom,
            r.crop_width,
            r.crop_height,
            r.crop_x,
            r.crop_y,
            r.background
        )
    };
    Resp {
        orig_width: r.orig_w,
        orig_height: r.orig_h,
        has_content: r.has_content,
        content_x: r.content_x,
        content_y: r.content_y,
        content_width: r.content_width,
        content_height: r.content_height,
        crop_x: r.crop_x,
        crop_y: r.crop_y,
        crop_width: r.crop_width,
        crop_height: r.crop_height,
        trim_left: r.trim_left,
        trim_top: r.trim_top,
        trim_right: r.trim_right,
        trim_bottom: r.trim_bottom,
        needs_trim: r.needs_trim,
        content_fraction: r.content_fraction,
        background: r.background,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_args() -> Args {
        serde_json::from_str(r#"{"url":"https://example.com/logo.png"}"#).unwrap()
    }

    #[test]
    fn defaults_applied() {
        let a = default_args();
        assert_eq!(a.mode, "auto");
        assert!(a.color.is_none());
        assert_eq!(a.tolerance, 16.0);
        assert_eq!(a.background_percent, 100.0);
        assert_eq!(a.padding, 0.0);
    }

    #[test]
    fn response_note_points_at_image_trim() {
        let r = BoundsReport {
            orig_w: 100,
            orig_h: 80,
            has_content: true,
            content_x: 10,
            content_y: 8,
            content_width: 60,
            content_height: 40,
            crop_x: 10,
            crop_y: 8,
            crop_width: 60,
            crop_height: 40,
            trim_left: 10,
            trim_top: 8,
            trim_right: 30,
            trim_bottom: 32,
            needs_trim: true,
            content_fraction: 0.3,
            background: "#ffffff".into(),
        };
        let resp = response(r);
        assert!(resp.needs_trim);
        assert!(resp.note.contains("image-trim"));
        assert!(resp.note.contains("60x40+10+8"));
    }

    #[test]
    fn response_note_when_no_content() {
        let r = BoundsReport {
            orig_w: 6,
            orig_h: 6,
            has_content: false,
            content_x: 0,
            content_y: 0,
            content_width: 6,
            content_height: 6,
            crop_x: 0,
            crop_y: 0,
            crop_width: 6,
            crop_height: 6,
            trim_left: 0,
            trim_top: 0,
            trim_right: 0,
            trim_bottom: 0,
            needs_trim: false,
            content_fraction: 0.0,
            background: "transparent".into(),
        };
        let resp = response(r);
        assert!(!resp.has_content);
        assert!(resp.note.contains("no content"));
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode": { "type": "string", "enum": ["auto", "transparent", "color"], "default": "auto", "description": "How to decide what is background: auto (default) reads the alpha channel if the corners are transparent, else votes a solid color from the 4 corners; transparent keys only on the alpha channel; color compares against the color parameter. Pass color in either auto or color mode to force a specific background." },
                    "color": { "type": "string", "description": "Background color as hex (#rgb or #rrggbb, e.g. #fff or #ffffff). Required for mode=color; optional in mode=auto to override the corner vote; must be omitted for mode=transparent." },
                    "tolerance": { "type": "integer", "minimum": 0, "maximum": 255, "default": 16, "description": "Color/alpha match tolerance 0-255 (default 16): a pixel counts as background when its max per-channel distance from the background color (and its distance from fully opaque) is at or below this. 0 = exact match; raise it to absorb JPEG noise or anti-aliased edges." },
                    "background_percent": { "type": "integer", "minimum": 50, "maximum": 100, "default": 100, "description": "Percent of a row/column that must match the background for that edge line to be trimmed, 50-100 (default 100 = every pixel must match). Lower it (e.g. 98) to trim through a few stray non-background pixels in an otherwise empty margin." },
                    "padding": { "type": "integer", "minimum": 0, "maximum": 500, "default": 0, "description": "Pixels of the original margin to keep back around the detected content in the suggested crop, 0-500 (default 0 = tight box). Clamped to the image edges — no synthetic pixels are added." }
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
