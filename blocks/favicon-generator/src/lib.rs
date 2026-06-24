//! gizza-ai/favicon-generator — turn one source image into a complete favicon
//! bundle (multi-resolution `favicon.ico` + square PNGs 16–512 px +
//! `apple-touch-icon.png` + `site.webmanifest`), returned as a single ZIP.
//!
//! Pipeline: resolve the image source (URL/ref) → pure `core::generate` (decode +
//! resize each size + ICO encode + manifest + zip via the `image`/`zip` crates) →
//! ZIP media envelope the page/CLI can download.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a binary ZIP/bundle output has no page render
//! mode — same F3 pattern as image-collage / create-zip).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_favicon_generator_core::{generate_files, parse_color, parse_fit, zip_bundle, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

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
    #[serde(default = "default_name")]
    name: String,
}
fn default_sizes() -> String {
    "16,32,48,64,96,128,192,256,512".to_string()
}
fn default_fit() -> String {
    "contain".to_string()
}
fn default_bg() -> String {
    "#00000000".to_string()
}
fn default_name() -> String {
    "My App".to_string()
}

/// `Input::Image` emits the `url`⊕`ref` `oneOf` for the source picture.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::string("sizes")
                .default("16,32,48,64,96,128,192,256,512")
                .describe("Comma-separated square PNG sizes in pixels (1..=1024). Default covers browser + PWA + Android Chrome."),
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
        .param(
            Param::string("name")
                .default("My App")
                .describe("App name written into the generated site.webmanifest."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn parse_sizes(s: &str) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    for tok in s.split(',') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        let n: u32 = t.parse().map_err(|_| format!("invalid size '{t}'"))?;
        out.push(n);
    }
    if out.is_empty() {
        return Err("provide at least one size".into());
    }
    Ok(out)
}

#[cfg(target_arch = "wasm32")]
struct FaviconGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/favicon-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a complete favicon bundle (ICO + PNGs + web manifest) from one image",
    requires = ["wafer-run/network"],
    skill(
        description = "Turn one source image into a complete favicon bundle, returned as a ZIP. The bundle contains a multi-resolution favicon.ico (16/32/48 px), square PNG icons at each requested size, an apple-touch-icon.png (180 px) for iOS, and a site.webmanifest referencing the PNGs. sizes is a comma-separated list of square PNG sizes in pixels (default 16,32,48,64,96,128,192,256,512). fit controls how a non-square source is squared: contain (pad with the background color) or cover (scale-to-fill then center-crop). background sets the pad color (#rrggbbaa, default transparent). name sets the app name in the manifest. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call); PNG/JPEG/WebP/GIF/BMP input.",
        parameters = schema_json()
    ),
)]
impl FaviconGenerator {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("favicon-generator")?;

    let png_sizes = parse_sizes(&args.sizes).map_err(SkillError::InvalidArgs)?;
    let fit = parse_fit(&args.fit).map_err(SkillError::InvalidArgs)?;
    let background = parse_color(&args.background).map_err(SkillError::InvalidArgs)?;

    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let opts = Options {
        fit,
        background,
        png_sizes,
        app_name: args.name,
    };

    let files = generate_files(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let count = files.len();
    let zip = zip_bundle(&files).map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &zip,
        "application/zip",
        "favicon-bundle.zip".to_string(),
        format!(
            "generated a favicon bundle of {count} files ({} bytes ZIP): favicon.ico, square PNGs, apple-touch-icon.png, and site.webmanifest",
            zip.len()
        ),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sizes_works() {
        assert_eq!(parse_sizes("16, 32 ,48").unwrap(), vec![16, 32, 48]);
        assert!(parse_sizes("  ").is_err());
        assert!(parse_sizes("16,abc").is_err());
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "sizes":      { "type": "string", "default": "16,32,48,64,96,128,192,256,512", "description": "Comma-separated square PNG sizes in pixels (1..=1024). Default covers browser + PWA + Android Chrome." },
                    "fit":        { "type": "string", "enum": ["contain", "cover"], "default": "contain", "description": "How to fit a non-square source: contain (pad to a square with the background color, keeps the whole image) or cover (scale-to-fill then center-crop)." },
                    "background": { "type": "string", "default": "#00000000", "description": "Background fill for padded areas when fit=contain, as #rgb, #rrggbb, or #rrggbbaa (default transparent)." },
                    "name":       { "type": "string", "default": "My App", "description": "App name written into the generated site.webmanifest." }
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
