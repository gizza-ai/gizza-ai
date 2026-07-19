//! gizza-ai/device-mockup-framer — frame a screenshot inside a clean device
//! mockup (phone, tablet, laptop, or browser window) with vector-style bezels,
//! a solid / gradient / transparent backdrop, an optional soft drop shadow, and
//! padding. Returns a PNG. Pure Rust (image + fontdue) — runs on all backends
//! incl. the chat SW. Surfaces: chat + CLI (image input + image bytes output →
//! no page, like screenshot-beautify / image-border-frame).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_device_mockup_framer_core::{
    frame, parse_background, parse_color, parse_device, parse_frame_color, Options,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_device")]
    device: String,
    #[serde(default = "d_frame_color")]
    frame_color: String,
    #[serde(default = "d_background")]
    background: String,
    #[serde(default = "d_bg_color")]
    bg_color: String,
    #[serde(default = "d_bg_color2")]
    bg_color2: String,
    #[serde(default = "d_gradient_angle")]
    gradient_angle: f64,
    #[serde(default = "d_padding")]
    padding: u64,
    #[serde(default = "d_shadow")]
    shadow: bool,
    #[serde(default = "d_shadow_blur")]
    shadow_blur: u64,
    #[serde(default = "d_shadow_opacity")]
    shadow_opacity: f64,
    #[serde(default = "d_browser_url")]
    browser_url: String,
}

fn d_device() -> String {
    "phone".into()
}
fn d_frame_color() -> String {
    "black".into()
}
fn d_background() -> String {
    "gradient".into()
}
fn d_bg_color() -> String {
    "#6366f1".into()
}
fn d_bg_color2() -> String {
    "#a855f7".into()
}
fn d_gradient_angle() -> f64 {
    135.0
}
fn d_padding() -> u64 {
    64
}
fn d_shadow() -> bool {
    true
}
fn d_shadow_blur() -> u64 {
    40
}
fn d_shadow_opacity() -> f64 {
    0.35
}
fn d_browser_url() -> String {
    "example.com".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("device", ["phone", "tablet", "laptop", "browser"])
                .default("phone")
                .describe("Device shell to frame the shot in: phone, tablet, laptop, or browser window (default phone)."),
        )
        .param(
            Param::enumv("frame_color", ["black", "white", "silver"])
                .default("black")
                .describe("Device body/bezel color; for the browser it also sets a dark (black) or light (white/silver) chrome (default black)."),
        )
        .param(
            Param::enumv("background", ["gradient", "solid", "transparent"])
                .default("gradient")
                .describe("Backdrop behind the device: gradient (bg_color→bg_color2), solid (bg_color), or transparent (default gradient)."),
        )
        .param(
            Param::string("bg_color")
                .default("#6366f1")
                .describe("Primary backdrop color as #rgb/#rrggbb/#rrggbbaa (default #6366f1 indigo); ignored when background=transparent."),
        )
        .param(
            Param::string("bg_color2")
                .default("#a855f7")
                .describe("Second gradient color as #rgb/#rrggbb/#rrggbbaa (default #a855f7 violet); used only when background=gradient."),
        )
        .param(
            Param::number("gradient_angle")
                .min(0.0)
                .max(360.0)
                .default(135.0)
                .describe("Gradient direction in degrees, 0=left→right, 90=top→bottom (default 135)."),
        )
        .param(
            Param::integer("padding")
                .min(0.0)
                .max(512.0)
                .default(64)
                .describe("Padding in pixels around the device on every side (default 64)."),
        )
        .param(
            Param::boolean("shadow")
                .default(true)
                .describe("Draw a soft drop shadow behind the device (default true)."),
        )
        .param(
            Param::integer("shadow_blur")
                .min(0.0)
                .max(200.0)
                .default(40)
                .describe("Drop-shadow blur radius in pixels (default 40)."),
        )
        .param(
            Param::number("shadow_opacity")
                .min(0.0)
                .max(1.0)
                .default(0.35)
                .describe("Drop-shadow opacity from 0 (invisible) to 1 (solid) (default 0.35)."),
        )
        .param(
            Param::string("browser_url")
                .default("example.com")
                .describe("Address-bar text shown in the browser window's URL bar (default example.com); ignored for phone/tablet/laptop."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DeviceMockupFramer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/device-mockup-framer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Frame a screenshot inside a clean phone, tablet, laptop, or browser-window mockup",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Frame a screenshot inside a clean device mockup drawn with vector-style bezels, then drop it on a backdrop with an optional soft shadow. device=phone|tablet|laptop|browser. frame_color=black|white|silver (for browser this also picks dark vs light chrome). background=gradient|solid|transparent with bg_color/bg_color2 (#rgb/#rrggbb/#rrggbbaa) + gradient_angle (degrees). padding (px); shadow (bool) + shadow_blur (px) + shadow_opacity (0-1). browser_url sets the address-bar text in browser mode. Returns a PNG (alpha for the bezels/shadow/transparent backdrop). Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl DeviceMockupFramer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("device-mockup-framer")?;
    let opts = build_options(&args).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = frame(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "device-mockup.png".to_string(),
        format!("framed the screenshot in a device mockup ({} bytes PNG)", png.len()),
        MAX_OUTPUT_BYTES,
    )
}

/// Validate + clamp the raw args into a core `Options`. Shared shape so the
/// (wasm-only) `run` path and the native unit test agree.
fn build_options(args: &Args) -> Result<Options, String> {
    Ok(Options {
        device: parse_device(&args.device)?,
        frame_color: parse_frame_color(&args.frame_color)?,
        background: parse_background(&args.background)?,
        bg_color: parse_color(&args.bg_color)?,
        bg_color2: parse_color(&args.bg_color2)?,
        gradient_angle: args.gradient_angle.rem_euclid(360.0) as f32,
        padding: args.padding.min(512) as u32,
        shadow: args.shadow,
        shadow_blur: args.shadow_blur.min(200) as u32,
        shadow_opacity: args.shadow_opacity.clamp(0.0, 1.0) as f32,
        browser_url: args.browser_url.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_args() -> Args {
        Args {
            source: serde_json::from_str(r#"{"url":"https://example.com/a.png"}"#).unwrap(),
            device: d_device(),
            frame_color: d_frame_color(),
            background: d_background(),
            bg_color: d_bg_color(),
            bg_color2: d_bg_color2(),
            gradient_angle: d_gradient_angle(),
            padding: d_padding(),
            shadow: d_shadow(),
            shadow_blur: d_shadow_blur(),
            shadow_opacity: d_shadow_opacity(),
            browser_url: d_browser_url(),
        }
    }

    #[test]
    fn build_options_defaults_and_clamps() {
        let mut a = default_args();
        a.padding = 100_000;
        a.shadow_opacity = 5.0;
        a.gradient_angle = 495.0; // wraps to 135
        a.shadow_blur = 100_000;
        let o = build_options(&a).unwrap();
        assert_eq!(o.padding, 512, "padding clamps to max");
        assert_eq!(o.shadow_opacity, 1.0, "opacity clamps to 1");
        assert_eq!(o.shadow_blur, 200, "blur clamps to max");
        assert!((o.gradient_angle - 135.0).abs() < 1e-3, "angle wraps mod 360");
        assert!(o.shadow);
    }

    #[test]
    fn build_options_rejects_bad_enum_and_color() {
        let mut a = default_args();
        a.device = "watch".into();
        assert!(build_options(&a).is_err());
        let mut a = default_args();
        a.frame_color = "gold".into();
        assert!(build_options(&a).is_err());
        let mut a = default_args();
        a.background = "plaid".into();
        assert!(build_options(&a).is_err());
        let mut a = default_args();
        a.bg_color = "notacolor".into();
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
                    "device":         { "type": "string", "enum": ["phone", "tablet", "laptop", "browser"], "default": "phone", "description": "Device shell to frame the shot in: phone, tablet, laptop, or browser window (default phone)." },
                    "frame_color":    { "type": "string", "enum": ["black", "white", "silver"], "default": "black", "description": "Device body/bezel color; for the browser it also sets a dark (black) or light (white/silver) chrome (default black)." },
                    "background":     { "type": "string", "enum": ["gradient", "solid", "transparent"], "default": "gradient", "description": "Backdrop behind the device: gradient (bg_color→bg_color2), solid (bg_color), or transparent (default gradient)." },
                    "bg_color":       { "type": "string", "default": "#6366f1", "description": "Primary backdrop color as #rgb/#rrggbb/#rrggbbaa (default #6366f1 indigo); ignored when background=transparent." },
                    "bg_color2":      { "type": "string", "default": "#a855f7", "description": "Second gradient color as #rgb/#rrggbb/#rrggbbaa (default #a855f7 violet); used only when background=gradient." },
                    "gradient_angle": { "type": "number", "minimum": 0, "maximum": 360, "default": 135.0, "description": "Gradient direction in degrees, 0=left→right, 90=top→bottom (default 135)." },
                    "padding":        { "type": "integer", "minimum": 0, "maximum": 512, "default": 64, "description": "Padding in pixels around the device on every side (default 64)." },
                    "shadow":         { "type": "boolean", "default": true, "description": "Draw a soft drop shadow behind the device (default true)." },
                    "shadow_blur":    { "type": "integer", "minimum": 0, "maximum": 200, "default": 40, "description": "Drop-shadow blur radius in pixels (default 40)." },
                    "shadow_opacity": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.35, "description": "Drop-shadow opacity from 0 (invisible) to 1 (solid) (default 0.35)." },
                    "browser_url":    { "type": "string", "default": "example.com", "description": "Address-bar text shown in the browser window's URL bar (default example.com); ignored for phone/tablet/laptop." }
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
