//! gizza-ai/google-takeout-photo-merger — merge Google Takeout JSON sidecar
//! metadata (date taken, GPS position, caption) back into each photo's EXIF.
//!
//! Pipeline: resolve the album ZIP (url/ref) → `core::merge_zip` (pure: `zip` +
//! `serde_json` + the shipped exif-edit write engine) → a ZIP envelope with the
//! rewritten photos, or a plain-text plan when `dry_run` is on.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker.
//! Surfaces: chat + CLI. No standalone page — a ZIP result fits neither the
//! text page nor the ffmpeg media page shape (the same no-page file-input
//! pattern as archive-extractor / app-icon-set).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    respond_ok, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_google_takeout_photo_merger_core as core;
use serde::Deserialize;
use wafer_sdk::*;

/// Matches archive-extractor's input ceiling. A whole multi-GB Takeout export
/// has to be uploaded an album at a time; the descriptor says so.
const MAX_BYTES: usize = 64 * 1024 * 1024;
/// Cap on the ZIP handed back through the envelope's data URL.
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
/// Per-file rows included in the chat/CLI summary before it is elided.
const MAX_REPORT_ROWS: usize = 40;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    fields: Option<String>,
    overwrite: Option<bool>,
    gps_source: Option<String>,
    date_source: Option<String>,
    fix_extension: Option<bool>,
    set_file_times: Option<bool>,
    keep_sidecars: Option<bool>,
    dry_run: Option<bool>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::string("fields")
                .default("date,gps,description")
                .describe(
                    "Comma-separated metadata groups to merge: date (writes DateTimeOriginal, \
                     DateTimeDigitized and DateTime from the sidecar's photoTakenTime), gps \
                     (latitude, longitude and altitude), description (the caption you typed in \
                     Google Photos, written to ImageDescription). Pass 'all' for every group. \
                     Example: date,gps.",
                ),
        )
        .param(
            Param::boolean("overwrite")
                .default(false)
                .describe(
                    "Replace EXIF fields the photo already carries. Off by default: a real camera \
                     timestamp or GPS fix is more trustworthy than Google's copy, so only missing \
                     fields are filled in. Turn it on when Google Photos holds a location or date \
                     you corrected by hand.",
                ),
        )
        .param(
            Param::enumv("gps_source", ["auto", "geo_data", "geo_data_exif"])
                .default("auto")
                .describe(
                    "Which block of the sidecar the position comes from. auto = prefer geoData \
                     (what Google Photos shows, including a location you edited) and fall back to \
                     geoDataExif; geo_data = only geoData; geo_data_exif = only geoDataExif (the \
                     position the camera originally recorded). An all-zero block counts as 'no \
                     location' and is never written.",
                ),
        )
        .param(
            Param::enumv("date_source", ["auto", "photo_taken", "creation"])
                .default("auto")
                .describe(
                    "Which sidecar timestamp becomes the date taken. auto = photoTakenTime, \
                     falling back to creationTime; photo_taken = only photoTakenTime (when the \
                     shot was taken); creation = only creationTime (when it was uploaded to \
                     Google Photos). Sidecar timestamps are Unix epochs, so they are written as \
                     UTC.",
                ),
        )
        .param(
            Param::boolean("fix_extension")
                .default(true)
                .describe(
                    "Rename files whose contents do not match their extension — Takeout exports \
                     PNGs named .jpg, which trips up other importers. The new name is chosen from \
                     the file's magic bytes. Set false to keep Google's original names.",
                ),
        )
        .param(
            Param::boolean("set_file_times")
                .default(true)
                .describe(
                    "Stamp each file's timestamp in the result ZIP with its capture time, so the \
                     photos sort correctly after extraction even in apps that ignore EXIF. Also \
                     applied to videos, whose containers cannot take EXIF. Dates before 1980 keep \
                     the default stamp (a ZIP cannot store them).",
                ),
        )
        .param(
            Param::boolean("keep_sidecars")
                .default(false)
                .describe(
                    "Keep the .json sidecars in the result. By default a sidecar whose data was \
                     merged is dropped (it is now redundant); a .json that was never paired with \
                     a photo is always kept, since it may be album metadata rather than a sidecar.",
                ),
        )
        .param(
            Param::boolean("dry_run")
                .default(false)
                .describe(
                    "Preview only: report what would be written to each photo and return no ZIP. \
                     Useful for checking the sidecar pairing on a big album before committing.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GoogleTakeoutPhotoMerger;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/google-takeout-photo-merger",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Merge Google Takeout JSON sidecar metadata back into your photos' EXIF",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Merge Google Takeout's JSON sidecar metadata back into each photo's EXIF. \
                       Upload one Takeout album as a ZIP (url or ref); every photo is paired with \
                       its .json sidecar and the date taken, GPS position and caption are written \
                       into the image, and the merged album comes back as a single ZIP. Pairing \
                       handles every naming form Google uses: NAME.jpg.supplemental-metadata.json, \
                       the legacy NAME.jpg.json and NAME.json, sidecar names Google truncated, \
                       duplicate counters (NAME(1).jpg pairs with NAME.jpg(1).json) and -edited \
                       copies, which reuse the original's sidecar. By default only fields the \
                       photo is MISSING are filled in, so real camera metadata is never clobbered \
                       (set overwrite=true to replace). EXIF can only be written into JPEG and \
                       PNG; HEIC, MP4, MOV, GIF and other containers are copied through unchanged \
                       and listed in the report, and their file timestamps are still corrected. \
                       Sidecar timestamps are Unix epochs and are written as UTC. Use dry_run=true \
                       to preview the plan without producing a ZIP. Input cap 64 MiB — upload one \
                       album at a time; if an export is split across several ZIPs, extract them \
                       into one folder and re-zip so each photo sits beside its sidecar.",
        parameters = schema_json()
    ),
)]
impl GoogleTakeoutPhotoMerger {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// Derive the result ZIP's filename from the uploaded one.
fn output_zip_name(in_filename: &str) -> String {
    let stem = in_filename
        .strip_suffix(".zip")
        .or_else(|| in_filename.strip_suffix(".ZIP"))
        .unwrap_or(in_filename);
    if stem.is_empty() {
        "photos-merged.zip".to_string()
    } else {
        format!("{stem}-merged.zip")
    }
}

/// Turn the wire args into validated core options.
fn to_options(args: &Args) -> Result<core::Options, SkillError> {
    let d = core::Options::default();
    Ok(core::Options {
        fields: match &args.fields {
            Some(f) => core::parse_fields(f).map_err(SkillError::InvalidArgs)?,
            None => d.fields,
        },
        overwrite: args.overwrite.unwrap_or(d.overwrite),
        gps_source: match &args.gps_source {
            Some(s) => core::parse_gps_source(s).map_err(SkillError::InvalidArgs)?,
            None => d.gps_source,
        },
        date_source: match &args.date_source {
            Some(s) => core::parse_date_source(s).map_err(SkillError::InvalidArgs)?,
            None => d.date_source,
        },
        fix_extension: args.fix_extension.unwrap_or(d.fix_extension),
        set_file_times: args.set_file_times.unwrap_or(d.set_file_times),
        keep_sidecars: args.keep_sidecars.unwrap_or(d.keep_sidecars),
        dry_run: args.dry_run.unwrap_or(d.dry_run),
    })
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args =
        serde_json::from_slice(&body).invalid_args("google-takeout-photo-merger")?;
    let opts = to_options(&args)?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let (zip, report) = core::merge_zip(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let summary = core::render_report(&report, MAX_REPORT_ROWS);

    // A dry run has no artifact — hand back the plan as plain text.
    let Some(zip) = zip else {
        return respond_ok(&format!("{in_filename}: dry run — no ZIP produced.\n{summary}"));
    };

    if zip.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::TooLarge {
            kind: "output",
            bytes: zip.len(),
            cap: MAX_OUTPUT_BYTES,
        });
    }

    let out_name = output_zip_name(&in_filename);
    let zip_len = zip.len();
    let data_url = format!("data:application/zip;base64,{}", B64.encode(&zip));
    let env = Envelope {
        for_llm: format!("{in_filename} → {out_name} ({zip_len}-byte ZIP)\n{summary}"),
        for_ui: ForUi {
            data_url,
            mime: "application/zip".to_string(),
            filename: out_name,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_zip_name_suffixes_the_album() {
        assert_eq!(output_zip_name("Photos from 2024.zip"), "Photos from 2024-merged.zip");
        assert_eq!(output_zip_name("takeout"), "takeout-merged.zip");
        assert_eq!(output_zip_name(""), "photos-merged.zip");
    }

    #[test]
    fn to_options_defaults_match_the_core_defaults() {
        let args: Args = serde_json::from_str(r#"{"url":"https://example.com/a.zip"}"#).unwrap();
        assert_eq!(to_options(&args).unwrap(), core::Options::default());
    }

    #[test]
    fn to_options_parses_every_advertised_value() {
        let args: Args = serde_json::from_str(
            r#"{"url":"https://example.com/a.zip","fields":"gps,date","overwrite":true,
                "gps_source":"geo_data_exif","date_source":"creation","fix_extension":false,
                "set_file_times":false,"keep_sidecars":true,"dry_run":true}"#,
        )
        .unwrap();
        let o = to_options(&args).unwrap();
        assert_eq!(o.fields, vec!["gps", "date"]);
        assert!(o.overwrite && o.keep_sidecars && o.dry_run);
        assert!(!o.fix_extension && !o.set_file_times);
        assert_eq!(o.gps_source, core::GpsSource::GeoDataExif);
        assert_eq!(o.date_source, core::DateSource::Creation);
    }

    #[test]
    fn to_options_rejects_a_bad_enum() {
        let args: Args = serde_json::from_str(
            r#"{"url":"https://example.com/a.zip","gps_source":"exif"}"#,
        )
        .unwrap();
        let err = to_options(&args).unwrap_err();
        assert!(format!("{err:?}").contains("unknown gps_source"), "{err:?}");
    }

    /// Drift guard: the descriptor-derived chat schema must match this
    /// authored schema exactly (Input::File url⊕ref oneOf + every param).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "fields": {
                        "type": "string",
                        "default": "date,gps,description",
                        "description": "Comma-separated metadata groups to merge: date (writes DateTimeOriginal, DateTimeDigitized and DateTime from the sidecar's photoTakenTime), gps (latitude, longitude and altitude), description (the caption you typed in Google Photos, written to ImageDescription). Pass 'all' for every group. Example: date,gps."
                    },
                    "overwrite": {
                        "type": "boolean",
                        "default": false,
                        "description": "Replace EXIF fields the photo already carries. Off by default: a real camera timestamp or GPS fix is more trustworthy than Google's copy, so only missing fields are filled in. Turn it on when Google Photos holds a location or date you corrected by hand."
                    },
                    "gps_source": {
                        "type": "string",
                        "enum": ["auto", "geo_data", "geo_data_exif"],
                        "default": "auto",
                        "description": "Which block of the sidecar the position comes from. auto = prefer geoData (what Google Photos shows, including a location you edited) and fall back to geoDataExif; geo_data = only geoData; geo_data_exif = only geoDataExif (the position the camera originally recorded). An all-zero block counts as 'no location' and is never written."
                    },
                    "date_source": {
                        "type": "string",
                        "enum": ["auto", "photo_taken", "creation"],
                        "default": "auto",
                        "description": "Which sidecar timestamp becomes the date taken. auto = photoTakenTime, falling back to creationTime; photo_taken = only photoTakenTime (when the shot was taken); creation = only creationTime (when it was uploaded to Google Photos). Sidecar timestamps are Unix epochs, so they are written as UTC."
                    },
                    "fix_extension": {
                        "type": "boolean",
                        "default": true,
                        "description": "Rename files whose contents do not match their extension — Takeout exports PNGs named .jpg, which trips up other importers. The new name is chosen from the file's magic bytes. Set false to keep Google's original names."
                    },
                    "set_file_times": {
                        "type": "boolean",
                        "default": true,
                        "description": "Stamp each file's timestamp in the result ZIP with its capture time, so the photos sort correctly after extraction even in apps that ignore EXIF. Also applied to videos, whose containers cannot take EXIF. Dates before 1980 keep the default stamp (a ZIP cannot store them)."
                    },
                    "keep_sidecars": {
                        "type": "boolean",
                        "default": false,
                        "description": "Keep the .json sidecars in the result. By default a sidecar whose data was merged is dropped (it is now redundant); a .json that was never paired with a photo is always kept, since it may be album metadata rather than a sidecar."
                    },
                    "dry_run": {
                        "type": "boolean",
                        "default": false,
                        "description": "Preview only: report what would be written to each photo and return no ZIP. Useful for checking the sidecar pairing on a big album before committing."
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
