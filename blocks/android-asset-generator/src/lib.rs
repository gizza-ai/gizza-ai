//! gizza-ai/android-asset-generator — resize a source image into a full set of
//! Android launcher icons at every density bucket (mdpi → xxxhdpi) plus the
//! 512px Google Play store icon, bundled into one ZIP laid out as an Android
//! `res/mipmap-*` resource tree.
//!
//! Pipeline: resolve the source image (URL or attachment ref) →
//! `core::generate_zip` (pure-Rust image resize + zip) → base64 envelope (the
//! ZIP as a downloadable file).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a ZIP-of-images output fits neither the
//! pure-text nor the media page shape — the "no-page file-input" pattern, like
//! extract-pdf-images / encrypt-file).
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
    #[serde(default = "default_name")]
    name: String,
    #[serde(default)]
    round: bool,
}
fn default_name() -> String {
    "ic_launcher".to_string()
}

/// `Input::Image` emits the `url`⊕`ref` `oneOf`; `name` sets the base icon file
/// name, `round` adds a circular-masked variant per density.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::string("name")
                .default("ic_launcher")
                .describe("Base icon resource name used inside the mipmap folders, e.g. ic_launcher (default). Sanitized to lowercase a-z, 0-9 and underscore; invalid names fall back to ic_launcher."),
        )
        .param(
            Param::boolean("round")
                .default(false)
                .describe("Also emit a circular-masked <name>_round.png in every density bucket (the ic_launcher_round variant used by round launcher masks). Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AndroidAssetGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/android-asset-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a full set of Android launcher icons from one image",
    requires = ["wafer-run/network"],
    skill(
        description = "Resize a source image into a full set of Android launcher icons at every density bucket (mdpi 48px, hdpi 72px, xhdpi 96px, xxhdpi 144px, xxxhdpi 192px) plus the 512px Google Play store icon. Returns a ZIP laid out as an Android res/mipmap-* resource tree (res/mipmap-<density>/<name>.png + play-store-icon.png). For best results give a square image at least 512px. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call). Optional name sets the icon resource name (default ic_launcher); set round=true to also emit a circular-masked <name>_round.png per density.",
        parameters = schema_json()
    ),
)]
impl AndroidAssetGenerator {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("android-asset-generator")?;
    let (bytes, _mime, _in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let (zip, summary) =
        gizza_ai_android_asset_generator_core::generate_zip(&bytes, &args.name, args.round)
            .map_err(SkillError::InvalidArgs)?;

    if zip.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "generated archive is {} bytes, exceeds the {} byte limit",
            zip.len(),
            MAX_OUTPUT_BYTES
        )));
    }

    let filename = "android-icons.zip".to_string();
    let zip_len = zip.len();
    let encoded = B64.encode(&zip);
    let data_url = format!("data:application/zip;base64,{encoded}");

    let (sw, sh) = summary.source_dims;
    let round_note = if args.round {
        format!(" + {}_round.png (circular)", summary.icon_name)
    } else {
        String::new()
    };
    let for_llm = format!(
        "generated {} Android icon asset(s) from a {sw}x{sh} source into {filename} ({zip_len}-byte ZIP): mipmap-mdpi/hdpi/xhdpi/xxhdpi/xxxhdpi ({}.png{round_note}) + play-store-icon.png (512px)",
        summary.assets, summary.icon_name
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
    /// schema (Input::Image url⊕ref oneOf + optional `name` string).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "name": { "type": "string", "default": "ic_launcher", "description": "Base icon resource name used inside the mipmap folders, e.g. ic_launcher (default). Sanitized to lowercase a-z, 0-9 and underscore; invalid names fall back to ic_launcher." },
                    "round": { "type": "boolean", "default": false, "description": "Also emit a circular-masked <name>_round.png in every density bucket (the ic_launcher_round variant used by round launcher masks). Default false." }
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
