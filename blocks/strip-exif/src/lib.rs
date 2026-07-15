//! gizza-ai/strip-exif — remove all metadata (EXIF, GPS, XMP, comments) from a
//! JPEG or PNG and return a clean copy, **without re-encoding the pixels**.
//!
//! Pipeline: resolve the source image (url/ref) → `core::strip` (pure,
//! `img-parts`) → media envelope with the cleaned image bytes.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker (no ffmpeg
//! dependency). Surfaces: chat + CLI. No standalone page — image-bytes output
//! has no page render mode (the no-page image-output pattern, like image-collage).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, AssetKind, Input, SkillError, SkillResultExt,
    SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// Single-source param descriptor → chat schema (and CLI). strip-exif takes only
/// the image input (no scalar params), so the descriptor is just `Input::Image`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct StripExif;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/strip-exif",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove EXIF/GPS/XMP metadata from a JPEG or PNG",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Remove all metadata (EXIF, GPS location, XMP, IPTC, comments) from a JPEG or PNG image and return a clean copy. The pixels are NOT re-encoded — the image is byte-for-byte identical apart from the stripped metadata, so there is no quality loss. Useful for privacy before sharing photos. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call). JPEG and PNG only.",
        parameters = schema_json()
    ),
)]
impl StripExif {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (strip-exif has no scalar params — just the image source).
    let args: Args = serde_json::from_slice(&body).invalid_args("strip-exif")?;

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    // 3. Strip metadata (shared pure core). The core detects the real format from
    //    the magic bytes, so we trust it over the resolved mime.
    let (output, report) =
        gizza_ai_strip_exif_core::strip(&input_bytes).map_err(SkillError::InvalidArgs)?;

    // 4. Envelope (same format as the input — pixels are untouched).
    let (_fmt, out_mime, ext) = gizza_ai_strip_exif_core::detect_format(&output)
        .ok_or_else(|| SkillError::InvalidArgs("output is not a recognized image".into()))?;
    let filename = filename_with_suffix(&in_filename, "-clean", ext);
    let for_llm = summary(&in_filename, &report);
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

/// One-line summary for the LLM: what was stripped and the byte delta.
fn summary(source: &str, report: &gizza_ai_strip_exif_core::StripReport) -> String {
    let exif = if report.had_exif {
        " (including EXIF/GPS)"
    } else {
        ""
    };
    format!(
        "stripped {} metadata segment(s){} from {} ({} → {} bytes, {} format); pixels unchanged",
        report.segments_removed, exif, source, report.input_bytes, report.output_bytes, report.format
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
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn summary_reports_exif_and_delta() {
        let report = gizza_ai_strip_exif_core::StripReport {
            format: "jpeg".into(),
            input_bytes: 12000,
            output_bytes: 9000,
            removed_bytes: 3000,
            segments_removed: 2,
            had_exif: true,
        };
        let s = summary("photo.jpg", &report);
        assert!(s.contains("photo.jpg"));
        assert!(s.contains("EXIF/GPS"));
        assert!(s.contains("12000"));
        assert!(s.contains("9000"));
        assert!(s.contains("pixels unchanged"));
    }

    #[test]
    fn clean_filename_keeps_extension() {
        assert_eq!(filename_with_suffix("cat.png", "-clean", "png"), "cat-clean.png");
        assert_eq!(filename_with_suffix("photo.jpg", "-clean", "jpg"), "photo-clean.jpg");
    }
}
