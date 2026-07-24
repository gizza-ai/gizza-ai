//! gizza-ai/photo-gps-mapper — extract EXIF GPS coordinates from a batch of
//! photos and return a coordinate list or mapping export.
//!
//! Pipeline: resolve each image source (url/ref) → `core::map_photos` (pure
//! `kamadak-exif`) → JSON report containing GeoJSON/CSV/GPX/KML/list text.
//! Pure Rust → runs on all backends. No standalone page: source-list input is
//! chat + CLI only, matching the batch-image tool pattern.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{resolve_source, respond_ok, AssetKind, Source};
use gizza_ai_block_utils::{Input, Param, SkillError, SourceFields, ToolDescriptor};
#[cfg(target_arch = "wasm32")]
use gizza_ai_photo_gps_mapper_core::{map_photos, InputPhoto, OutputFormat};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_precision")]
    precision: u8,
}

fn default_format() -> String {
    "geojson".to_string()
}

fn default_precision() -> u8 {
    6
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("images", 1)
                .required()
                .describe("One or more photo sources (JPEG/TIFF/PNG/WebP, depending on embedded EXIF support) to scan for GPS coordinates. Each item has exactly one of `url` or `ref`."),
        )
        .param(
            Param::enumv("format", ["geojson", "csv", "gpx", "kml", "list"])
                .default("geojson")
                .describe("Mapping export format: geojson (default FeatureCollection), csv, gpx waypoints, kml placemarks, or a plain list."),
        )
        .param(
            Param::integer("precision")
                .default(6)
                .min(0.0)
                .max(10.0)
                .describe("Decimal places for latitude, longitude, and altitude in the formatted output (0-10, default 6)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PhotoGpsMapper;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/photo-gps-mapper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract GPS coordinates from photos as GeoJSON, CSV, GPX, KML, or a list",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract EXIF GPS coordinates from a batch of photos and return a mapping-friendly report. Provide `images` as a list of one or more sources (each a url or a `ref`). `format` chooses geojson (default FeatureCollection), csv, gpx, kml, or list; `precision` controls decimal places (0-10, default 6). The result includes total photos, how many had GPS, names without GPS, parsed locations with optional altitude/timestamp, and the requested export text. Photos without GPS are reported; the tool errors only when none of the inputs contain coordinates. Interactive map rendering and reverse geocoding are intentionally out of scope.",
        parameters = schema_json()
    ),
)]
impl PhotoGpsMapper {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("photo-gps-mapper")?;
    if args.images.is_empty() {
        return Err(SkillError::InvalidArgs(
            "photo-gps-mapper needs at least 1 image".into(),
        ));
    }
    let format = OutputFormat::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    if args.precision > 10 {
        return Err(SkillError::InvalidArgs(
            "precision must be between 0 and 10".into(),
        ));
    }

    let mut photos = Vec::with_capacity(args.images.len());
    for (i, field) in args.images.into_iter().enumerate() {
        let source = field.into_inner();
        let from_source = source_label(&source);
        let (bytes, _mime, name) = resolve_source(source, AssetKind::Image, MAX_INPUT_BYTES)?;
        let label = if name.contains('.') && !name.trim().is_empty() {
            name
        } else if !from_source.is_empty() {
            from_source
        } else {
            format!("photo {i}")
        };
        photos.push(InputPhoto { label, bytes });
    }

    let report = map_photos(&photos, format, args.precision).map_err(SkillError::InvalidArgs)?;
    respond_ok(&report)
}

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
                        "minItems": 1,
                        "description": "One or more photo sources (JPEG/TIFF/PNG/WebP, depending on embedded EXIF support) to scan for GPS coordinates. Each item has exactly one of `url` or `ref`.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "format": {
                        "type": "string",
                        "enum": ["geojson", "csv", "gpx", "kml", "list"],
                        "default": "geojson",
                        "description": "Mapping export format: geojson (default FeatureCollection), csv, gpx waypoints, kml placemarks, or a plain list."
                    },
                    "precision": {
                        "type": "integer",
                        "default": 6,
                        "minimum": 0,
                        "maximum": 10,
                        "description": "Decimal places for latitude, longitude, and altitude in the formatted output (0-10, default 6)."
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
