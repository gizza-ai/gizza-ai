//! gizza-ai/splash-screen-generator — build iOS/Android/PWA launch images from
//! one logo and return the whole set as a ZIP.
//!
//! Pure Rust core (`image` + `zip`) so the chat/CLI block can run anywhere the
//! shared image-source resolver can fetch a PNG/JPEG/WebP/GIF/BMP. There is no
//! standalone page: a multi-file ZIP output fits the same no-page file-input
//! pattern as app-icon-set, favicon-generator and android-asset-generator.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_background")]
    background: String,
    #[serde(default)]
    dark_background: Option<String>,
    #[serde(default = "default_logo_scale")]
    logo_scale: f32,
    #[serde(default = "default_orientation")]
    orientation: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_quality")]
    quality: u8,
    #[serde(default = "default_true")]
    ios: bool,
    #[serde(default = "default_true")]
    android: bool,
}

fn default_background() -> String {
    "#ffffff".to_string()
}
fn default_logo_scale() -> f32 {
    0.4
}
fn default_orientation() -> String {
    "portrait".to_string()
}
fn default_format() -> String {
    "png".to_string()
}
fn default_quality() -> u8 {
    82
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI). `Input::Image` emits the source
/// image `url`/`ref` oneOf; params control the generated bundle.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(Param::string("background").default("#ffffff").describe("Opaque splash background colour as #rgb, #rrggbb, #rrggbbaa, rgb(r,g,b), or a basic colour name. Default #ffffff."))
        .param(Param::string("dark_background").describe("Optional second background colour. When set, the ZIP also includes dark-mode iOS launch images and Android -night resources."))
        .param(Param::number("logo_scale").min(0.05).max(0.9).default(0.4).describe("Logo size as a fraction of the canvas's shorter edge, 0.05-0.9. Default 0.4 (40%)."))
        .param(Param::enumv("orientation", ["portrait", "landscape", "both"]).default("portrait").describe("Which screen orientations to emit: portrait, landscape, or both. Default portrait."))
        .param(Param::enumv("format", ["png", "jpeg"]).default("png").describe("Image format for generated launch screens: png (default) or jpeg. ZIP wrapper is always application/zip."))
        .param(Param::integer("quality").min(1.0).max(100.0).default(82).describe("JPEG quality, 1-100. Ignored for PNG output. Default 82."))
        .param(Param::boolean("ios").default(true).describe("Emit iOS/iPadOS PWA launch images plus apple-touch-startup-image.html. Default true."))
        .param(Param::boolean("android").default(true).describe("Emit Android drawable density buckets plus the Android 12+ splash icon. Default true."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/splash-screen-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate iOS, Android, and PWA splash screen image bundles from one logo.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Create a ZIP bundle of launch/splash screen assets from one logo image. Provide the source as an image url or ref (PNG, JPEG, WebP, GIF or BMP). The bundle can include iOS/iPadOS PWA apple-touch-startup-image files with a ready HTML snippet, Android drawable density buckets, and an Android 12+ splash icon. Configure a background colour, optional dark-mode background, logo_scale from 0.05 to 0.9, orientation (portrait, landscape or both), output format (png or jpeg), JPEG quality, and ios/android platform toggles. The source image is capped at 16 MiB and the generated ZIP at 64 MiB.",
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
    use gizza_ai_block_utils::AssetKind;
    use gizza_ai_splash_screen_generator_core::{
        generate_zip, parse_color, Format, Options, Orientation,
    };

    let args: Args = serde_json::from_slice(&body).invalid_args("splash-screen-generator")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let opts = Options {
        background: parse_color(&args.background).map_err(SkillError::InvalidArgs)?,
        dark_background: args
            .dark_background
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(parse_color)
            .transpose()
            .map_err(SkillError::InvalidArgs)?,
        logo_scale: args.logo_scale,
        orientation: Orientation::parse(&args.orientation).map_err(SkillError::InvalidArgs)?,
        format: Format::parse(&args.format).map_err(SkillError::InvalidArgs)?,
        quality: args.quality,
        ios: args.ios,
        android: args.android,
    };

    let (zip, summary) = generate_zip(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let (sw, sh) = summary.source_dims;
    let dark = if summary.dark {
        " including dark-mode variants"
    } else {
        ""
    };
    build_media_envelope(
        &zip,
        "application/zip",
        "splash-screens.zip".to_string(),
        format!(
            "generated {} splash screen image(s) across {} file(s) for {} from a {sw}x{sh} logo{dark}",
            summary.images,
            summary.files,
            summary.platforms.join("+"),
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
                "type":"object",
                "properties":{
                    "url":{"type":"string","description":"Image URL (HTTP/HTTPS). Use either url or ref."},
                    "ref":{"type":"string","description":"Reference id from a prior tool call. Use either url or ref."},
                    "background":{"type":"string","default":"#ffffff","description":"Opaque splash background colour as #rgb, #rrggbb, #rrggbbaa, rgb(r,g,b), or a basic colour name. Default #ffffff."},
                    "dark_background":{"type":"string","description":"Optional second background colour. When set, the ZIP also includes dark-mode iOS launch images and Android -night resources."},
                    "logo_scale":{"type":"number","minimum":0.05,"maximum":0.9,"default":0.4,"description":"Logo size as a fraction of the canvas's shorter edge, 0.05-0.9. Default 0.4 (40%)."},
                    "orientation":{"type":"string","enum":["portrait","landscape","both"],"default":"portrait","description":"Which screen orientations to emit: portrait, landscape, or both. Default portrait."},
                    "format":{"type":"string","enum":["png","jpeg"],"default":"png","description":"Image format for generated launch screens: png (default) or jpeg. ZIP wrapper is always application/zip."},
                    "quality":{"type":"integer","minimum":1,"maximum":100,"default":82,"description":"JPEG quality, 1-100. Ignored for PNG output. Default 82."},
                    "ios":{"type":"boolean","default":true,"description":"Emit iOS/iPadOS PWA launch images plus apple-touch-startup-image.html. Default true."},
                    "android":{"type":"boolean","default":true,"description":"Emit Android drawable density buckets plus the Android 12+ splash icon. Default true."}
                },
                "additionalProperties":false,
                "oneOf":[{"required":["url"]},{"required":["ref"]}]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
