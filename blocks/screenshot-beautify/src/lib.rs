//! gizza-ai/screenshot-beautify — wrap a raw screenshot in a share-ready frame:
//! padding, rounded corners, a soft drop shadow, an optional macOS window title
//! bar with traffic-light dots, and a solid or linear-gradient backdrop. Pads out
//! to an aspect ratio without cropping. Returns a PNG. Pure Rust (image crate) —
//! runs on all backends incl. the chat SW. Surfaces: chat + CLI (image input +
//! image bytes output → no page, like image-border-frame / image-round-avatar).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_screenshot_beautify_core::{
    beautify, parse_aspect, parse_background, parse_color, parse_titlebar, Options,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_padding")]
    padding: u64,
    #[serde(default = "d_corner_radius")]
    corner_radius: u64,
    #[serde(default = "d_shadow")]
    shadow: bool,
    #[serde(default = "d_shadow_blur")]
    shadow_blur: u64,
    #[serde(default = "d_shadow_opacity")]
    shadow_opacity: f64,
    #[serde(default = "d_background")]
    background: String,
    #[serde(default = "d_bg_color")]
    bg_color: String,
    #[serde(default = "d_bg_color2")]
    bg_color2: String,
    #[serde(default = "d_gradient_angle")]
    gradient_angle: f64,
    #[serde(default = "d_aspect")]
    aspect: String,
    #[serde(default = "d_titlebar")]
    titlebar: String,
}

fn d_padding() -> u64 {
    64
}
fn d_corner_radius() -> u64 {
    16
}
fn d_shadow() -> bool {
    true
}
fn d_shadow_blur() -> u64 {
    24
}
fn d_shadow_opacity() -> f64 {
    0.35
}
fn d_background() -> String {
    "gradient".into()
}
fn d_bg_color() -> String {
    "#4f46e5".into()
}
fn d_bg_color2() -> String {
    "#9333ea".into()
}
fn d_gradient_angle() -> f64 {
    135.0
}
fn d_aspect() -> String {
    "auto".into()
}
fn d_titlebar() -> String {
    "none".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("padding")
                .min(0.0)
                .max(512.0)
                .default(64)
                .describe("Padding in pixels around the shot on every side (default 64)."),
        )
        .param(
            Param::integer("corner_radius")
                .min(0.0)
                .max(512.0)
                .default(16)
                .describe("Rounded-corner radius on the shot in pixels; 0 = square (default 16)."),
        )
        .param(
            Param::boolean("shadow")
                .default(true)
                .describe("Draw a soft drop shadow behind the shot (default true)."),
        )
        .param(
            Param::integer("shadow_blur")
                .min(0.0)
                .max(200.0)
                .default(24)
                .describe("Drop-shadow blur radius in pixels (default 24)."),
        )
        .param(
            Param::number("shadow_opacity")
                .min(0.0)
                .max(1.0)
                .default(0.35)
                .describe("Drop-shadow opacity from 0 (invisible) to 1 (solid) (default 0.35)."),
        )
        .param(
            Param::enumv("background", ["gradient", "solid"])
                .default("gradient")
                .describe("Backdrop style: gradient (bg_color→bg_color2) or solid (bg_color)."),
        )
        .param(
            Param::string("bg_color")
                .default("#4f46e5")
                .describe("Primary backdrop color as #rgb/#rrggbb/#rrggbbaa (default #4f46e5 indigo). For a named gradient preset use two colors, e.g. Sunset #ff512f→#dd2476."),
        )
        .param(
            Param::string("bg_color2")
                .default("#9333ea")
                .describe("Second gradient color as #rgb/#rrggbb/#rrggbbaa (default #9333ea violet); ignored when background=solid."),
        )
        .param(
            Param::number("gradient_angle")
                .min(0.0)
                .max(360.0)
                .default(135.0)
                .describe("Gradient direction in degrees, 0=left→right, 90=top→bottom (default 135)."),
        )
        .param(
            Param::enumv("aspect", ["auto", "16:9", "4:3", "1:1", "9:16"])
                .default("auto")
                .describe("Pad out to this aspect ratio (never crops); auto keeps the padded size (default auto)."),
        )
        .param(
            Param::enumv("titlebar", ["none", "light", "dark"])
                .default("none")
                .describe("macOS-style window title bar with traffic-light dots atop the shot (default none)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ScreenshotBeautify;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/screenshot-beautify",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Wrap a screenshot in a padded, rounded, shadowed frame with a gradient backdrop",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Wrap a raw screenshot in a share-ready frame. Adds padding around the shot, rounded corners, an optional soft drop shadow, an optional macOS window title bar (traffic-light dots), and a solid or linear-gradient backdrop. Pads out to an aspect ratio (auto/16:9/4:3/1:1/9:16) without ever cropping. Params: padding (px, default 64); corner_radius (px, default 16, 0=square); shadow (bool, default true) + shadow_blur (px) + shadow_opacity (0-1); background=gradient|solid; bg_color/bg_color2 (#rgb/#rrggbb/#rrggbbaa) + gradient_angle (degrees); titlebar=none|light|dark. Express any named gradient preset via the two colors + angle. Returns a PNG (alpha needed for the shadow/rounded corners). Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ScreenshotBeautify {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("screenshot-beautify")?;
    let opts = build_options(&args).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = beautify(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "beautified.png".to_string(),
        format!("beautified the screenshot ({} bytes PNG)", png.len()),
        MAX_OUTPUT_BYTES,
    )
}

/// Validate + clamp the raw args into a core `Options`. Shared shape so the
/// (wasm-only) `run` path and the native unit test agree.
fn build_options(args: &Args) -> Result<Options, String> {
    Ok(Options {
        padding: args.padding.min(512) as u32,
        corner_radius: args.corner_radius.min(512) as u32,
        shadow: args.shadow,
        shadow_blur: args.shadow_blur.min(200) as u32,
        shadow_opacity: args.shadow_opacity.clamp(0.0, 1.0) as f32,
        background: parse_background(&args.background)?,
        bg_color: parse_color(&args.bg_color)?,
        bg_color2: parse_color(&args.bg_color2)?,
        gradient_angle: args.gradient_angle.rem_euclid(360.0) as f32,
        aspect: parse_aspect(&args.aspect)?,
        titlebar: parse_titlebar(&args.titlebar)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_args() -> Args {
        Args {
            source: serde_json::from_str(r#"{"url":"https://example.com/a.png"}"#).unwrap(),
            padding: d_padding(),
            corner_radius: d_corner_radius(),
            shadow: d_shadow(),
            shadow_blur: d_shadow_blur(),
            shadow_opacity: d_shadow_opacity(),
            background: d_background(),
            bg_color: d_bg_color(),
            bg_color2: d_bg_color2(),
            gradient_angle: d_gradient_angle(),
            aspect: d_aspect(),
            titlebar: d_titlebar(),
        }
    }

    #[test]
    fn build_options_defaults_and_clamps() {
        let mut a = default_args();
        a.padding = 100_000;
        a.shadow_opacity = 5.0;
        a.gradient_angle = 495.0; // wraps to 135
        let o = build_options(&a).unwrap();
        assert_eq!(o.padding, 512, "padding clamps to max");
        assert_eq!(o.shadow_opacity, 1.0, "opacity clamps to 1");
        assert!((o.gradient_angle - 135.0).abs() < 1e-3, "angle wraps mod 360");
        assert_eq!(o.corner_radius, 16);
        assert!(o.shadow);
    }

    #[test]
    fn build_options_rejects_bad_enum_and_color() {
        let mut a = default_args();
        a.background = "plaid".into();
        assert!(build_options(&a).is_err());
        let mut a = default_args();
        a.bg_color = "notacolor".into();
        assert!(build_options(&a).is_err());
        let mut a = default_args();
        a.aspect = "3:2".into();
        assert!(build_options(&a).is_err());
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":            { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":            { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "padding":        { "type": "integer", "minimum": 0, "maximum": 512, "default": 64, "description": "Padding in pixels around the shot on every side (default 64)." },
                    "corner_radius":  { "type": "integer", "minimum": 0, "maximum": 512, "default": 16, "description": "Rounded-corner radius on the shot in pixels; 0 = square (default 16)." },
                    "shadow":         { "type": "boolean", "default": true, "description": "Draw a soft drop shadow behind the shot (default true)." },
                    "shadow_blur":    { "type": "integer", "minimum": 0, "maximum": 200, "default": 24, "description": "Drop-shadow blur radius in pixels (default 24)." },
                    "shadow_opacity": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.35, "description": "Drop-shadow opacity from 0 (invisible) to 1 (solid) (default 0.35)." },
                    "background":     { "type": "string", "enum": ["gradient", "solid"], "default": "gradient", "description": "Backdrop style: gradient (bg_color→bg_color2) or solid (bg_color)." },
                    "bg_color":       { "type": "string", "default": "#4f46e5", "description": "Primary backdrop color as #rgb/#rrggbb/#rrggbbaa (default #4f46e5 indigo). For a named gradient preset use two colors, e.g. Sunset #ff512f→#dd2476." },
                    "bg_color2":      { "type": "string", "default": "#9333ea", "description": "Second gradient color as #rgb/#rrggbb/#rrggbbaa (default #9333ea violet); ignored when background=solid." },
                    "gradient_angle": { "type": "number", "minimum": 0, "maximum": 360, "default": 135.0, "description": "Gradient direction in degrees, 0=left→right, 90=top→bottom (default 135)." },
                    "aspect":         { "type": "string", "enum": ["auto", "16:9", "4:3", "1:1", "9:16"], "default": "auto", "description": "Pad out to this aspect ratio (never crops); auto keeps the padded size (default auto)." },
                    "titlebar":       { "type": "string", "enum": ["none", "light", "dark"], "default": "none", "description": "macOS-style window title bar with traffic-light dots atop the shot (default none)." }
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
