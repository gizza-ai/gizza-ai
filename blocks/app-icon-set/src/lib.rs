//! gizza-ai/app-icon-set — turn one source image into a complete cross-platform
//! app-icon set (iOS Xcode `AppIcon.appiconset` + `Contents.json`, the Android
//! `res/mipmap-*` launcher tree + Play-store icon, and PWA icons + a
//! `manifest.webmanifest`), bundled into one downloadable ZIP.
//!
//! Pipeline: resolve the source image (URL or attachment ref) →
//! `core::generate_zip` (pure-Rust image resize + zip) → base64 envelope (the
//! ZIP as a downloadable file).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a ZIP-of-images output fits neither the
//! pure-text nor the media page shape — the "no-page file-input" pattern, like
//! android-asset-generator / favicon-generator).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB source image
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024; // generated ZIP

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_app_name")]
    app_name: String,
    #[serde(default = "default_background")]
    background: String,
    #[serde(default = "default_true")]
    ios: bool,
    #[serde(default = "default_true")]
    android: bool,
    #[serde(default = "default_true")]
    web: bool,
}
fn default_app_name() -> String {
    "My App".to_string()
}
fn default_background() -> String {
    "#ffffff".to_string()
}
fn default_true() -> bool {
    true
}

/// `Input::Image` emits the `url`⊕`ref` `oneOf`; the rest scope which platform
/// folders are emitted, name the PWA manifest, and set the background used to
/// flatten the opaque iOS marketing icon + pad the maskable PWA icon.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::string("app_name")
                .default("My App")
                .describe("App name written into the PWA manifest.webmanifest (name and short_name). Default \"My App\"."),
        )
        .param(
            Param::string("background")
                .default("#ffffff")
                .describe("Background colour (hex, e.g. #ffffff) used to flatten the opaque iOS 1024px App-Store icon (Apple rejects transparency there) and to pad the maskable PWA icon's safe zone. Default #ffffff (white)."),
        )
        .param(
            Param::boolean("ios")
                .default(true)
                .describe("Emit the iOS Xcode AppIcon.appiconset folder (every iPhone/iPad/App-Store size + Contents.json). Default true."),
        )
        .param(
            Param::boolean("android")
                .default(true)
                .describe("Emit the Android res/mipmap-* launcher tree (mdpi 48 → xxxhdpi 192) plus the 512px Google Play store icon. Default true."),
        )
        .param(
            Param::boolean("web")
                .default(true)
                .describe("Emit the PWA/web icons (192, 512, a padded 512 maskable icon, a 180px apple-touch-icon) and a manifest.webmanifest. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AppIconSet;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/app-icon-set",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a full PWA/iOS/Android app-icon set from one image",
    requires = ["wafer-run/network"],
    skill(
        description = "Resize a single source image into a complete cross-platform app-icon set, returned as one ZIP. Includes: a drop-in iOS Xcode AppIcon.appiconset/ folder with every iPhone/iPad size (20–83.5pt across @1x/@2x/@3x) plus the 1024px App-Store marketing icon and the Contents.json catalog; the Android res/mipmap-* launcher tree (mdpi 48, hdpi 72, xhdpi 96, xxhdpi 144, xxxhdpi 192) plus the 512px Google Play store icon; and PWA/web icons (192, 512, a padded 512 maskable icon, a 180px apple-touch-icon) with a ready manifest.webmanifest. For best results provide a square image at least 1024px. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call). Optional app_name names the PWA manifest; background (hex) flattens the opaque iOS marketing icon and pads the maskable icon; set ios/android/web=false to omit a platform.",
        parameters = schema_json()
    ),
)]
impl AppIconSet {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_app_icon_set_core::{generate_zip, parse_hex_color, Options, Platforms};
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("app-icon-set")?;
    let (bytes, _mime, _in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let opts = Options {
        platforms: Platforms { ios: args.ios, android: args.android, web: args.web },
        app_name: args.app_name,
        background: parse_hex_color(&args.background),
    };

    let (zip, summary) = generate_zip(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    if zip.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "generated archive is {} bytes, exceeds the {} byte limit",
            zip.len(),
            MAX_OUTPUT_BYTES
        )));
    }

    let filename = "app-icons.zip".to_string();
    let zip_len = zip.len();
    let encoded = B64.encode(&zip);
    let data_url = format!("data:application/zip;base64,{encoded}");

    let (sw, sh) = summary.source_dims;
    let for_llm = format!(
        "generated {} app-icon file(s) for {} from a {sw}x{sh} source into {filename} ({zip_len}-byte ZIP)",
        summary.icons,
        summary.platforms.join("+"),
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime: "application/zip".to_string(), filename },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Image url⊕ref oneOf + the platform/name/background params).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "app_name": { "type": "string", "default": "My App", "description": "App name written into the PWA manifest.webmanifest (name and short_name). Default \"My App\"." },
                    "background": { "type": "string", "default": "#ffffff", "description": "Background colour (hex, e.g. #ffffff) used to flatten the opaque iOS 1024px App-Store icon (Apple rejects transparency there) and to pad the maskable PWA icon's safe zone. Default #ffffff (white)." },
                    "ios": { "type": "boolean", "default": true, "description": "Emit the iOS Xcode AppIcon.appiconset folder (every iPhone/iPad/App-Store size + Contents.json). Default true." },
                    "android": { "type": "boolean", "default": true, "description": "Emit the Android res/mipmap-* launcher tree (mdpi 48 → xxxhdpi 192) plus the 512px Google Play store icon. Default true." },
                    "web": { "type": "boolean", "default": true, "description": "Emit the PWA/web icons (192, 512, a padded 512 maskable icon, a 180px apple-touch-icon) and a manifest.webmanifest. Default true." }
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
