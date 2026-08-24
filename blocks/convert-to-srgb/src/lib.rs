//! gizza-ai/convert-to-srgb — convert an image with an embedded ICC colour
//! profile to plain sRGB PNG. Chat + CLI only: generic pure-WASM image file pages
//! are not available, matching other pure Rust image-source tools with binary
//! image output.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 24 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 48 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/convert-to-srgb",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an embedded-ICC image to plain sRGB PNG.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Convert a JPEG, PNG, WebP, GIF, or BMP image that carries an embedded ICC colour profile (Display P3, Adobe RGB, scanner/printer profiles, etc.) into a plain sRGB PNG for consistent web display. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call). The pixels are colour-managed through the embedded profile; the output is an unprofiled sRGB PNG. Images without an embedded ICC profile are rejected so a no-op is not mistaken for a conversion.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("convert-to-srgb")?;
    let (input, _mime, name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let (png, report) = gizza_ai_convert_to_srgb_core::convert_to_srgb_png(&input)
        .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        filename_with_suffix_local(&name, "-srgb", "png"),
        report.summary(),
        MAX_OUTPUT_BYTES,
    )
}

fn filename_with_suffix_local(name: &str, suffix: &str, ext: &str) -> String {
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
    format!("{stem}{suffix}.{ext}")
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
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_gets_srgb_suffix() {
        assert_eq!(
            filename_with_suffix_local("p3-photo.jpg", "-srgb", "png"),
            "p3-photo-srgb.png"
        );
        assert_eq!(
            filename_with_suffix_local("upload", "-srgb", "png"),
            "upload-srgb.png"
        );
    }
}
