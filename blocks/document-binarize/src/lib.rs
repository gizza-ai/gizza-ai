//! gizza-ai/document-binarize — threshold an image to pure black & white.
//! Returns a PNG. Pure-Rust (image crate) — runs on all backends incl. the chat
//! SW. Surfaces: chat + CLI (image input + image bytes output → no page, like
//! blur-image / image-grayscale).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_document_binarize_core::{binarize, parse_method};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

const METHOD_DESC: &str = "Binarization method: otsu (global auto threshold), fixed (global threshold param), sauvola or niblack (local adaptive; best for documents). Default sauvola.";
const THRESHOLD_DESC: &str = "Global cutoff 0-255 used only when method=fixed; pixels brighter than it become white. Default 128.";
const WINDOW_DESC: &str = "Neighbourhood size in pixels for the local methods (sauvola/niblack); forced odd internally. Default 25.";
const K_DESC: &str = "Sensitivity for the local methods (sauvola/niblack), -1.0 to 1.0; lower keeps more dark foreground. Default 0.34.";
const INVERT_DESC: &str = "Swap black and white in the output. Default false.";

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_threshold")]
    threshold: u32,
    #[serde(default = "default_window")]
    window: u32,
    #[serde(default = "default_k")]
    k: f64,
    #[serde(default)]
    invert: bool,
}
fn default_method() -> String {
    "sauvola".to_string()
}
fn default_threshold() -> u32 {
    128
}
fn default_window() -> u32 {
    25
}
fn default_k() -> f64 {
    0.34
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("method", ["otsu", "fixed", "sauvola", "niblack"])
                .default("sauvola")
                .describe(METHOD_DESC),
        )
        .param(
            Param::integer("threshold")
                .min(0.0)
                .max(255.0)
                .default(128)
                .describe(THRESHOLD_DESC),
        )
        .param(
            Param::integer("window")
                .min(3.0)
                .max(101.0)
                .default(25)
                .describe(WINDOW_DESC),
        )
        .param(
            Param::number("k")
                .min(-1.0)
                .max(1.0)
                .default(0.34)
                .describe(K_DESC),
        )
        .param(Param::boolean("invert").default(false).describe(INVERT_DESC))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DocumentBinarize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/document-binarize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Threshold an image to pure black and white.",
    requires = ["wafer-run/network"],
    skill(
        description = "Binarize an image to pure black & white — ideal for cleaning up scanned documents. method picks the algorithm: otsu (automatic global), fixed (use the threshold param), sauvola or niblack (local adaptive, best for uneven lighting; default sauvola). Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl DocumentBinarize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("document-binarize")?;
    let method = parse_method(Some(&args.method)).map_err(SkillError::InvalidArgs)?;
    if args.threshold > 255 {
        return Err(SkillError::InvalidArgs(
            "invalid document-binarize args: threshold must be 0-255".into(),
        ));
    }
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = binarize(
        &bytes,
        method,
        args.threshold as u8,
        args.window,
        args.k,
        args.invert,
    )
    .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "binarized.png".to_string(),
        format!(
            "binarized image (method {}, {} bytes PNG)",
            args.method,
            png.len()
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
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "method":    { "type": "string", "enum": ["otsu", "fixed", "sauvola", "niblack"], "default": "sauvola", "description": "Binarization method: otsu (global auto threshold), fixed (global threshold param), sauvola or niblack (local adaptive; best for documents). Default sauvola." },
                    "threshold": { "type": "integer", "minimum": 0, "maximum": 255, "default": 128, "description": "Global cutoff 0-255 used only when method=fixed; pixels brighter than it become white. Default 128." },
                    "window":    { "type": "integer", "minimum": 3, "maximum": 101, "default": 25, "description": "Neighbourhood size in pixels for the local methods (sauvola/niblack); forced odd internally. Default 25." },
                    "k":         { "type": "number", "minimum": -1, "maximum": 1, "default": 0.34, "description": "Sensitivity for the local methods (sauvola/niblack), -1.0 to 1.0; lower keeps more dark foreground. Default 0.34." },
                    "invert":    { "type": "boolean", "default": false, "description": "Swap black and white in the output. Default false." }
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
