//! gizza-ai/image-perspective-warp — apply a free four-corner perspective transform
//! to an image (keystone / skew correction, or deliberate perspective distortion).
//!
//! The chat schema is derived from `descriptor()` (single source across chat + CLI +
//! page). Corners are ffmpeg coordinate expressions: pixel numbers plus `W`/`H`
//! frame constants for the far right/bottom edges.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_perspective_warp_core::{parse_interp, parse_mode, plan, Corners};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024; // 8 MiB

fn d_zero() -> String {
    "0".into()
}
fn d_w() -> String {
    "W".into()
}
fn d_h() -> String {
    "H".into()
}

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_zero")]
    tl_x: String,
    #[serde(default = "d_zero")]
    tl_y: String,
    #[serde(default = "d_w")]
    tr_x: String,
    #[serde(default = "d_zero")]
    tr_y: String,
    #[serde(default = "d_zero")]
    bl_x: String,
    #[serde(default = "d_h")]
    bl_y: String,
    #[serde(default = "d_w")]
    br_x: String,
    #[serde(default = "d_h")]
    br_y: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    interpolation: Option<String>,
}

fn corner(name: &'static str, def: &'static str, which: &str) -> Param {
    Param::string(name).default(def).describe(&format!(
        "{which} corner coordinate for ffmpeg perspective. Use a pixel number, or W/H for the image width/height edge. Default {def}."
    ))
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(corner("tl_x", "0", "X of the top-left"))
        .param(corner("tl_y", "0", "Y of the top-left"))
        .param(corner("tr_x", "W", "X of the top-right"))
        .param(corner("tr_y", "0", "Y of the top-right"))
        .param(corner("bl_x", "0", "X of the bottom-left"))
        .param(corner("bl_y", "H", "Y of the bottom-left"))
        .param(corner("br_x", "W", "X of the bottom-right"))
        .param(corner("br_y", "H", "Y of the bottom-right"))
        .param(
            Param::enumv("mode", ["correct", "distort"]).describe(
                "correct = the four corners mark a rectangle in the photo and are stretched to fill the frame (keystone/skew correction); distort = the image corners are pushed to the four points (add a tilt). Default: correct.",
            ),
        )
        .param(
            Param::enumv("interpolation", ["linear", "cubic"]).describe(
                "Pixel interpolation: linear (faster) or cubic (smoother edges). Default: linear.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-perspective-warp",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Apply a four-corner perspective transform to an image.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Apply a free four-corner perspective transform to an image (keystone / skew correction, or deliberate perspective distortion). Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call). Corners are pixel coordinates or W/H frame constants.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("image-perspective-warp")?;
    let corners: Corners<'_> = [
        &args.tl_x, &args.tl_y, &args.tr_x, &args.tr_y, &args.bl_x, &args.bl_y, &args.br_x,
        &args.br_y,
    ];
    let interp =
        parse_interp(args.interpolation.as_deref()).invalid_args("image-perspective-warp")?;
    let mode = parse_mode(args.mode.as_deref()).invalid_args("image-perspective-warp")?;

    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {mime}")))?;
    let ffmpeg_in = format!("in.{ext}");
    let (argv, out_name) =
        plan(&corners, interp, mode, &ffmpeg_in).invalid_args("image-perspective-warp")?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, bytes, out_name)?;

    let output_size = output.len();
    let filename = filename_with_suffix(&in_name, "-warped", ext);
    let for_llm = format!("perspective-warped {in_name} ({output_size} bytes, {mime})");
    build_media_envelope(&output, &mime, filename, for_llm, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored schema below, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "tl_x": { "type": "string", "default": "0", "description": "X of the top-left corner coordinate for ffmpeg perspective. Use a pixel number, or W/H for the image width/height edge. Default 0." },
                    "tl_y": { "type": "string", "default": "0", "description": "Y of the top-left corner coordinate for ffmpeg perspective. Use a pixel number, or W/H for the image width/height edge. Default 0." },
                    "tr_x": { "type": "string", "default": "W", "description": "X of the top-right corner coordinate for ffmpeg perspective. Use a pixel number, or W/H for the image width/height edge. Default W." },
                    "tr_y": { "type": "string", "default": "0", "description": "Y of the top-right corner coordinate for ffmpeg perspective. Use a pixel number, or W/H for the image width/height edge. Default 0." },
                    "bl_x": { "type": "string", "default": "0", "description": "X of the bottom-left corner coordinate for ffmpeg perspective. Use a pixel number, or W/H for the image width/height edge. Default 0." },
                    "bl_y": { "type": "string", "default": "H", "description": "Y of the bottom-left corner coordinate for ffmpeg perspective. Use a pixel number, or W/H for the image width/height edge. Default H." },
                    "br_x": { "type": "string", "default": "W", "description": "X of the bottom-right corner coordinate for ffmpeg perspective. Use a pixel number, or W/H for the image width/height edge. Default W." },
                    "br_y": { "type": "string", "default": "H", "description": "Y of the bottom-right corner coordinate for ffmpeg perspective. Use a pixel number, or W/H for the image width/height edge. Default H." },
                    "mode": { "type": "string", "enum": ["correct", "distort"], "description": "correct = the four corners mark a rectangle in the photo and are stretched to fill the frame (keystone/skew correction); distort = the image corners are pushed to the four points (add a tilt). Default: correct." },
                    "interpolation": { "type": "string", "enum": ["linear", "cubic"], "description": "Pixel interpolation: linear (faster) or cubic (smoother edges). Default: linear." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_warped_suffix() {
        assert_eq!(
            filename_with_suffix("scan.jpg", "-warped", "jpg"),
            "scan-warped.jpg"
        );
    }
}
