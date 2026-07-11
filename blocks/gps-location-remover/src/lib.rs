//! gizza-ai/gps-location-remover — remove ONLY the GPS/geolocation tags from a
//! JPEG or PNG photo, leaving camera, lens, and exposure metadata intact.
//!
//! Pipeline: resolve the source image (url/ref) → `core::remove_gps` (pure,
//! `img-parts` + in-place EXIF TIFF surgery) → media envelope with the cleaned
//! image bytes.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker (no ffmpeg
//! dependency). Surfaces: chat + CLI. No standalone page — image-bytes output
//! has no page render mode (the no-page image-output pattern, like strip-exif).
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

/// Single-source param descriptor → chat schema (and CLI). This tool takes only
/// the image input (no scalar params — "GPS only, everything else kept" is the
/// whole behaviour), so the descriptor is just `Input::Image`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GpsLocationRemover;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gps-location-remover",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove only the GPS location tags from a photo, keeping camera data",
    requires = ["wafer-run/network"],
    skill(
        description = "Remove ONLY the GPS/geolocation tags (latitude, longitude, altitude, GPS timestamp, etc.) from a JPEG or PNG photo, while KEEPING all other EXIF metadata — camera make/model, lens, and exposure settings (ISO, aperture, shutter speed, timestamps). Unlike a strip-all-metadata cleaner, camera and exposure data survive. The pixels are NOT re-encoded, so there is no quality loss. Useful for privacy before sharing photos while keeping the technical camera data. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call). JPEG and PNG only; GPS embedded in XMP is not removed.",
        parameters = schema_json()
    ),
)]
impl GpsLocationRemover {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (no scalar params — just the image source).
    let args: Args = serde_json::from_slice(&body).invalid_args("gps-location-remover")?;

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    // 3. Remove GPS only (shared pure core). The core detects the real format
    //    from the magic bytes, so we trust it over the resolved mime.
    let (output, report) =
        gizza_ai_gps_location_remover_core::remove_gps(&input_bytes).map_err(SkillError::InvalidArgs)?;

    // 4. Envelope (same format as the input — pixels are untouched).
    let (_fmt, out_mime, ext) = gizza_ai_gps_location_remover_core::detect_format(&output)
        .ok_or_else(|| SkillError::InvalidArgs("output is not a recognized image".into()))?;
    let filename = filename_with_suffix(&in_filename, "-nogps", ext);
    let for_llm = summary(&in_filename, &report);
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

/// One-line summary for the LLM: what was removed and what was kept.
fn summary(source: &str, report: &gizza_ai_gps_location_remover_core::GpsReport) -> String {
    if report.had_gps {
        format!(
            "removed {} GPS/location tag(s) from {} ({} format); camera and exposure metadata kept, pixels unchanged",
            report.gps_tags_removed, source, report.format
        )
    } else if report.had_exif {
        format!(
            "{} ({} format) had EXIF but no GPS/location tags — nothing to remove; camera metadata untouched",
            source, report.format
        )
    } else {
        format!(
            "{} ({} format) had no EXIF metadata — no GPS tags to remove",
            source, report.format
        )
    }
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
    fn summary_reports_removed_gps_and_kept_camera() {
        let report = gizza_ai_gps_location_remover_core::GpsReport {
            format: "jpeg".into(),
            input_bytes: 12000,
            output_bytes: 12000,
            had_gps: true,
            gps_tags_removed: 5,
            had_exif: true,
        };
        let s = summary("photo.jpg", &report);
        assert!(s.contains("photo.jpg"));
        assert!(s.contains("5 GPS"));
        assert!(s.contains("camera and exposure metadata kept"));
        assert!(s.contains("pixels unchanged"));
    }

    #[test]
    fn summary_handles_no_gps_present() {
        let report = gizza_ai_gps_location_remover_core::GpsReport {
            format: "png".into(),
            input_bytes: 500,
            output_bytes: 500,
            had_gps: false,
            gps_tags_removed: 0,
            had_exif: true,
        };
        let s = summary("shot.png", &report);
        assert!(s.contains("no GPS/location tags"));
    }

    #[test]
    fn clean_filename_keeps_extension() {
        assert_eq!(filename_with_suffix("cat.png", "-nogps", "png"), "cat-nogps.png");
        assert_eq!(filename_with_suffix("photo.jpg", "-nogps", "jpg"), "photo-nogps.jpg");
    }
}
