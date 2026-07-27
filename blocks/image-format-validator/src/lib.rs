//! gizza-ai/image-format-validator — verify an image's REAL format against a
//! claimed one, check decode integrity, and report dimensions / colour depth.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::validate` (magic-byte
//! detection + full decode + claim matching) → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image file input + text verdict — the F3
//! no-page file-input pattern, like image-info / detect-file-type; the standalone
//! page file-input path is ffmpeg-only).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Format the file claims to be, or "auto" to fall back to the filename.
    #[serde(default)]
    claimed_format: Option<String>,
}

#[derive(Serialize)]
struct Resp {
    #[serde(flatten)]
    report: gizza_ai_image_format_validator_core::Report,
    /// Source filename, when one was available.
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

/// `Input::Image` emits the `url`⊕`ref` `oneOf`; the one control is the claimed
/// format to check against (a fixed-choice enum the chat UI renders as a select).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::enumv(
            "claimed_format",
            ["auto", "png", "jpeg", "gif", "webp", "bmp", "tiff", "ico"],
        )
        .default("auto")
        .describe(
            "Format the file claims to be, to check the bytes against (spoof/mismatch \
             detection). Use 'auto' (default) to fall back to the filename extension.",
        ),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageFormatValidator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-format-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Verify an image's real format against its claimed type and check for corruption",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Validate an image without uploading it: detect the TRUE format from its magic bytes (PNG/JPEG/GIF/WebP/BMP/TIFF/ICO), fully decode it to catch corrupt or truncated files, and report dimensions, megapixels, aspect ratio, colour type, bit depth, channel count, alpha channel, and file size. Optionally pass a claimed_format (or rely on the filename extension) to flag spoofed/mislabelled files where the extension disagrees with the real bytes. Returns a structured verdict (valid, matches_claim, corruption) and never throws on bad input. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ImageFormatValidator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-format-validator")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let filename_opt = (!filename.is_empty()).then(|| filename.clone());
    let report = gizza_ai_image_format_validator_core::validate(
        &bytes,
        args.claimed_format.as_deref(),
        filename_opt.as_deref(),
    )
    .map_err(SkillError::InvalidArgs)?;

    let resp = Resp {
        report,
        filename: filename_opt,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize image-format-validator response: {e}")))
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
                    "claimed_format": {
                        "type": "string",
                        "enum": ["auto", "png", "jpeg", "gif", "webp", "bmp", "tiff", "ico"],
                        "default": "auto",
                        "description": "Format the file claims to be, to check the bytes against (spoof/mismatch detection). Use 'auto' (default) to fall back to the filename extension."
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
