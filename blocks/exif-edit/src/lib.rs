//! gizza-ai/exif-edit — write, edit, or selectively strip individual EXIF
//! fields (date taken, GPS position, artist, copyright, camera info, software)
//! on a JPEG or PNG photo, without re-encoding the pixels.
//!
//! Pipeline: resolve the source image (url/ref) → `core::edit` (pure,
//! img-parts + kamadak-exif) → media envelope with the edited image bytes.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page — image-bytes output has no page render mode
//! (the no-page image-output pattern, like strip-exif / rotate-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_exif_edit_core::{parse_date, parse_remove, EditReport, Edits};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    date_taken: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    altitude: Option<f64>,
    artist: Option<String>,
    copyright: Option<String>,
    description: Option<String>,
    make: Option<String>,
    model: Option<String>,
    software: Option<String>,
    remove: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(Param::string("date_taken").describe(
            "New 'date taken' — writes DateTimeOriginal, DateTimeDigitized and DateTime together. \
             Accepts YYYY-MM-DD HH:MM:SS, EXIF's YYYY:MM:DD HH:MM:SS, ISO YYYY-MM-DDTHH:MM:SS, \
             or a bare YYYY-MM-DD (midnight). Example: 2024-06-01 14:30:00.",
        ))
        .param(
            Param::number("latitude")
                .min(-90.0)
                .max(90.0)
                .describe(
                    "GPS latitude in decimal degrees (-90..90, negative = south). Must be given \
                     together with longitude. Example: 48.8584 (Eiffel Tower).",
                ),
        )
        .param(
            Param::number("longitude")
                .min(-180.0)
                .max(180.0)
                .describe(
                    "GPS longitude in decimal degrees (-180..180, negative = west). Must be given \
                     together with latitude. Example: 2.2945.",
                ),
        )
        .param(
            Param::number("altitude")
                .min(-11000.0)
                .max(20000.0)
                .describe(
                    "GPS altitude in meters; negative = below sea level. Can be set alone or with \
                     latitude/longitude. Example: 35.",
                ),
        )
        .param(Param::string("artist").describe(
            "Creator name written to the EXIF Artist tag. Example: Jane Doe.",
        ))
        .param(Param::string("copyright").describe(
            "Copyright notice written to the EXIF Copyright tag. Example: (c) 2026 Jane Doe.",
        ))
        .param(Param::string("description").describe(
            "Caption written to the EXIF ImageDescription tag. Example: Sunset over the harbor.",
        ))
        .param(Param::string("make").describe(
            "Camera make written to the EXIF Make tag. Example: Canon.",
        ))
        .param(Param::string("model").describe(
            "Camera model written to the EXIF Model tag. Example: EOS R5.",
        ))
        .param(Param::string("software").describe(
            "Software/app name written to the EXIF Software tag. Example: darktable 4.6.",
        ))
        .param(Param::string("remove").describe(
            "Comma-separated metadata groups to remove while keeping everything else: gps (all GPS \
             tags), date (DateTime*/SubSec*/OffsetTime*), artist, copyright, description (incl. \
             UserComment + Windows XP* text tags), software, camera (Make/Model/Lens*), serials \
             (body+lens serial numbers, owner name, unique id), xmp (whole XMP packet), iptc \
             (whole IPTC/Photoshop block). Example: gps,serials.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Convert the wire args (minus the source) into validated core `Edits`.
fn to_edits(args: &Args) -> Result<Edits, SkillError> {
    let edits = Edits {
        date_taken: args
            .date_taken
            .as_deref()
            .map(parse_date)
            .transpose()
            .map_err(SkillError::InvalidArgs)?,
        latitude: args.latitude,
        longitude: args.longitude,
        altitude: args.altitude,
        artist: args.artist.clone(),
        copyright: args.copyright.clone(),
        description: args.description.clone(),
        make: args.make.clone(),
        model: args.model.clone(),
        software: args.software.clone(),
        remove: args
            .remove
            .as_deref()
            .map(parse_remove)
            .transpose()
            .map_err(SkillError::InvalidArgs)?
            .unwrap_or_default(),
    };
    gizza_ai_exif_edit_core::validate(&edits).map_err(SkillError::InvalidArgs)?;
    Ok(edits)
}

#[cfg(target_arch = "wasm32")]
struct ExifEdit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/exif-edit",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Edit or strip individual EXIF fields (date, GPS, copyright) on a photo",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Write, edit, or selectively remove individual EXIF metadata fields on a JPEG or PNG photo without re-encoding the pixels (no quality loss). Set the date taken (DateTimeOriginal/Digitized/DateTime), GPS position (decimal-degree latitude+longitude, optional altitude), artist, copyright, description, camera make/model, or software; and/or remove field groups (gps, date, artist, copyright, description, software, camera, serials, xmp, iptc) while keeping the rest. At least one set or remove is required. When EXIF fields change, the EXIF block is rebuilt: the embedded thumbnail and MakerNote are dropped (reported). Non-ASCII text values (e.g. \u{a9}, accents) are written as UTF-8 bytes like common EXIF tooling does; strict spec-ASCII readers may mis-render them. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call). JPEG and PNG only (TIFF/WebP/HEIC are not supported). To wipe ALL metadata instead, use strip-exif.",
        parameters = schema_json()
    ),
)]
impl ExifEdit {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("exif-edit")?;
    // Validate the edits BEFORE fetching the image — bad params fail fast.
    let edits = to_edits(&args)?;

    let (input_bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let (output, report) =
        gizza_ai_exif_edit_core::edit(&input_bytes, &edits).map_err(SkillError::InvalidArgs)?;

    let (_fmt, out_mime, ext) = gizza_ai_exif_edit_core::detect_format(&output)
        .ok_or_else(|| SkillError::InvalidArgs("output is not a recognized image".into()))?;
    let filename = filename_with_suffix(&in_filename, "-edited", ext);
    let for_llm = summary(&in_filename, &report);
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

/// One-line summary for the LLM: what was set/removed and the caveats.
fn summary(source: &str, report: &EditReport) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !report.fields_set.is_empty() {
        parts.push(format!("set {}", report.fields_set.join(", ")));
    }
    if !report.fields_removed.is_empty() {
        parts.push(format!(
            "removed {} field(s) ({})",
            report.fields_removed.len(),
            report.fields_removed.join(", ")
        ));
    }
    if report.segments_removed > 0 {
        parts.push(format!(
            "removed {} XMP/IPTC segment(s)",
            report.segments_removed
        ));
    }
    if parts.is_empty() {
        parts.push("no matching metadata found to change".into());
    }
    let mut caveats: Vec<&str> = Vec::new();
    if report.thumbnail_dropped {
        caveats.push("embedded thumbnail dropped");
    }
    if report.makernote_dropped {
        caveats.push("MakerNote dropped (not rewritable)");
    }
    let caveat_str = if caveats.is_empty() {
        String::new()
    } else {
        format!("; {}", caveats.join(", "))
    };
    format!(
        "{} on {} ({} → {} bytes, {} format){}; pixels unchanged",
        parts.join("; "),
        source,
        report.input_bytes,
        report.output_bytes,
        report.format,
        caveat_str
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
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "date_taken": { "type": "string", "description": "New 'date taken' — writes DateTimeOriginal, DateTimeDigitized and DateTime together. Accepts YYYY-MM-DD HH:MM:SS, EXIF's YYYY:MM:DD HH:MM:SS, ISO YYYY-MM-DDTHH:MM:SS, or a bare YYYY-MM-DD (midnight). Example: 2024-06-01 14:30:00." },
                    "latitude": { "type": "number", "minimum": -90, "maximum": 90, "description": "GPS latitude in decimal degrees (-90..90, negative = south). Must be given together with longitude. Example: 48.8584 (Eiffel Tower)." },
                    "longitude": { "type": "number", "minimum": -180, "maximum": 180, "description": "GPS longitude in decimal degrees (-180..180, negative = west). Must be given together with latitude. Example: 2.2945." },
                    "altitude": { "type": "number", "minimum": -11000, "maximum": 20000, "description": "GPS altitude in meters; negative = below sea level. Can be set alone or with latitude/longitude. Example: 35." },
                    "artist": { "type": "string", "description": "Creator name written to the EXIF Artist tag. Example: Jane Doe." },
                    "copyright": { "type": "string", "description": "Copyright notice written to the EXIF Copyright tag. Example: (c) 2026 Jane Doe." },
                    "description": { "type": "string", "description": "Caption written to the EXIF ImageDescription tag. Example: Sunset over the harbor." },
                    "make": { "type": "string", "description": "Camera make written to the EXIF Make tag. Example: Canon." },
                    "model": { "type": "string", "description": "Camera model written to the EXIF Model tag. Example: EOS R5." },
                    "software": { "type": "string", "description": "Software/app name written to the EXIF Software tag. Example: darktable 4.6." },
                    "remove": { "type": "string", "description": "Comma-separated metadata groups to remove while keeping everything else: gps (all GPS tags), date (DateTime*/SubSec*/OffsetTime*), artist, copyright, description (incl. UserComment + Windows XP* text tags), software, camera (Make/Model/Lens*), serials (body+lens serial numbers, owner name, unique id), xmp (whole XMP packet), iptc (whole IPTC/Photoshop block). Example: gps,serials." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// Build Args through the real serde path (SourceFields has no Default).
    fn args(json: &str) -> Args {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn to_edits_parses_date_and_remove() {
        let a = args(
            r#"{"url":"https://example.com/x.jpg","date_taken":"2024-06-01","remove":"GPS, serials"}"#,
        );
        let edits = to_edits(&a).unwrap();
        assert_eq!(edits.date_taken.as_deref(), Some("2024:06:01 00:00:00"));
        assert_eq!(edits.remove, vec!["gps", "serials"]);
    }

    #[test]
    fn to_edits_rejects_noop_and_bad_values() {
        let noop = args(r#"{"url":"https://example.com/x.jpg"}"#);
        assert!(to_edits(&noop).is_err(), "no edit given");
        let bad_date = args(r#"{"url":"https://example.com/x.jpg","date_taken":"not a date"}"#);
        assert!(to_edits(&bad_date).is_err());
        let lat_only = args(r#"{"url":"https://example.com/x.jpg","latitude":48.85}"#);
        assert!(to_edits(&lat_only).is_err(), "latitude without longitude");
        let bad_remove = args(r#"{"url":"https://example.com/x.jpg","remove":"everything"}"#);
        assert!(to_edits(&bad_remove).is_err(), "unknown remove group");
    }

    #[test]
    fn summary_reports_sets_removals_and_caveats() {
        let report = EditReport {
            format: "jpeg".into(),
            input_bytes: 12000,
            output_bytes: 12100,
            fields_set: vec!["DateTimeOriginal".into()],
            fields_removed: vec!["GPSLatitude".into(), "GPSLongitude".into()],
            segments_removed: 1,
            had_exif: true,
            thumbnail_dropped: true,
            makernote_dropped: false,
        };
        let s = summary("photo.jpg", &report);
        assert!(s.contains("set DateTimeOriginal"));
        assert!(s.contains("removed 2 field(s)"));
        assert!(s.contains("XMP/IPTC segment"));
        assert!(s.contains("embedded thumbnail dropped"));
        assert!(!s.contains("MakerNote"));
        assert!(s.contains("pixels unchanged"));
    }

    #[test]
    fn edited_filename_keeps_extension() {
        assert_eq!(
            filename_with_suffix("cat.png", "-edited", "png"),
            "cat-edited.png"
        );
        assert_eq!(
            filename_with_suffix("photo.jpg", "-edited", "jpg"),
            "photo-edited.jpg"
        );
    }
}
