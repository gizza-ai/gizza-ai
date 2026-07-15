//! gizza-ai/duplicate-image-finder — scan a batch of images and report exact +
//! near-duplicate pairs above a similarity threshold, with a suggested
//! keep/delete list.
//!
//! Pipeline: resolve each image source (URL/ref) → pure
//! `core::find_duplicates` (decode + dHash + pairwise Hamming distance +
//! keep/delete grouping via the `image` crate) → JSON report. `Input::None` +
//! a required `images` source_list (like image-collage / images-to-pdf).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (array input + JSON report output).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{resolve_source, respond_ok, AssetKind, Source};
use gizza_ai_block_utils::{Input, Param, SkillError, SourceFields, ToolDescriptor};
#[cfg(target_arch = "wasm32")]
use gizza_ai_duplicate_image_finder_core::{find_duplicates, InputImage};
use serde::Deserialize;
use wafer_sdk::*;

/// Each image is capped at 8 MiB on the wire (matches image-collage).
const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
    #[serde(default = "default_threshold")]
    threshold: f64,
}
fn default_threshold() -> f64 {
    90.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("images", 2)
                .required()
                .describe("Two or more image sources (PNG/JPEG/WebP/GIF/BMP) to scan for duplicates. Each item has exactly one of `url` or `ref`."),
        )
        .param(
            Param::number("threshold")
                .default(90.0)
                .min(0.0)
                .max(100.0)
                .describe("Similarity cutoff, 0-100 percent. A pair is reported when its perceptual similarity is at or above this value. 100 keeps only visually identical images; 90 (default) catches resizes, re-compression and minor edits; lower toward 80 for looser matches. Byte-identical files are always reported as exact regardless of this value."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DuplicateImageFinder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/duplicate-image-finder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find exact and near-duplicate images in a batch",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Scan a batch of images and report exact and near-duplicate pairs above a similarity threshold, with a suggested keep/delete list. Provide `images` as a list of 2+ sources (each a url or a `ref`; PNG/JPEG/WebP/GIF/BMP). `threshold` is a 0-100 similarity percent (default 90): a pair is reported when its perceptual similarity is at or above it; 100 keeps only visually identical images, lower catches looser matches. Byte-identical files are always flagged as exact. The result lists each image (dimensions, size, format, perceptual hash), the duplicate pairs (similarity %, Hamming distance, exact vs near), and grouped clusters whose suggested keep is the highest-resolution copy. Similarity uses a grayscale structural hash (dHash), so it is invariant to brightness, contrast, resizing and color; images that share a layout but differ in color, and flat/solid-color images, can be grouped — review before deleting.",
        parameters = schema_json()
    ),
)]
impl DuplicateImageFinder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::SkillResultExt;

    let args: Args = serde_json::from_slice(&body).invalid_args("duplicate-image-finder")?;
    if args.images.len() < 2 {
        return Err(SkillError::InvalidArgs(
            "duplicate-image-finder needs at least 2 images to compare".into(),
        ));
    }

    let mut inputs: Vec<InputImage> = Vec::with_capacity(args.images.len());
    for (i, field) in args.images.into_iter().enumerate() {
        let source = field.into_inner();
        let from_source = source_label(&source);
        let (bytes, _mime, name) = resolve_source(source, AssetKind::Image, MAX_INPUT_BYTES)?;
        // Prefer the resolved filename when it carries an extension (a real
        // name); otherwise fall back to a label derived from the URL/ref, then
        // the index. This keeps the keep/delete list identifiable.
        let label = if name.contains('.') && !name.trim().is_empty() {
            name
        } else if !from_source.is_empty() {
            from_source
        } else {
            format!("image {i}")
        };
        inputs.push(InputImage { label, bytes });
    }

    let report = find_duplicates(&inputs, args.threshold).map_err(SkillError::InvalidArgs)?;
    respond_ok(&report)
}

/// A human-facing label for a source, used to identify which file to delete:
/// the ref id, a URL's filename segment, or the host+path (scheme stripped).
#[cfg(target_arch = "wasm32")]
fn source_label(source: &Source) -> String {
    match source {
        Source::Ref(id) => id.clone(),
        Source::Url(u) => {
            let no_query = u.split(['?', '#']).next().unwrap_or(u);
            let seg = no_query.rsplit('/').next().unwrap_or("");
            if seg.contains('.') {
                seg.to_string()
            } else {
                let short = u
                    .strip_prefix("https://")
                    .or_else(|| u.strip_prefix("http://"))
                    .unwrap_or(u);
                short.chars().take(80).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "images": {
                        "type": "array",
                        "minItems": 2,
                        "description": "Two or more image sources (PNG/JPEG/WebP/GIF/BMP) to scan for duplicates. Each item has exactly one of `url` or `ref`.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "threshold": {
                        "type": "number",
                        "default": 90.0,
                        "minimum": 0,
                        "maximum": 100,
                        "description": "Similarity cutoff, 0-100 percent. A pair is reported when its perceptual similarity is at or above this value. 100 keeps only visually identical images; 90 (default) catches resizes, re-compression and minor edits; lower toward 80 for looser matches. Byte-identical files are always reported as exact regardless of this value."
                    }
                },
                "required": ["images"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
