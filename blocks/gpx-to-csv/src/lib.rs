//! gizza-ai/gpx-to-csv — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page + query-params); handle() delegates to block_utils::run_skill.
//! Pure compute — no host calls, runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_gpx_to_csv_core::convert_str;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    gpx: String,
    #[serde(default)]
    points: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    time_format: String,
    #[serde(default)]
    speed: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("gpx")
                .required()
                .describe("The GPX document to convert. Paste the whole file contents — an XML document with <trkpt> (track), <rtept> (route), and/or <wpt> (waypoint) elements carrying lat/lon attributes."),
        )
        .param(
            Param::enumv("points", ["all", "track", "route", "waypoint"])
                .default("all")
                .describe("Which point types to include as rows. all (default) emits track, route, and waypoint points; or restrict to one of track, route, waypoint."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"])
                .default("comma")
                .describe("Output CSV field separator. Default comma. Use semicolon for comma-decimal locales, tab for TSV, or pipe."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Emit the column-name header row. Default true; set false for a headerless CSV."),
        )
        .param(
            Param::enumv("time_format", ["iso", "unix", "none"])
                .default("iso")
                .describe("How the time column is written. iso (default) keeps the original ISO-8601 text; unix converts each timestamp to epoch seconds; none drops the time column entirely."),
        )
        .param(
            Param::boolean("speed")
                .default(false)
                .describe("Add a speed_kmh column with ground speed derived from consecutive timestamps (haversine distance ÷ Δt) along each track segment / route. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GpxToCsv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gpx-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a GPX track, route, or waypoints into a clean CSV table.",
    skill(
        description = "Convert a GPX document into a CSV table with one row per point — columns point_type, name, latitude, longitude, elevation_m, and time, plus a speed_kmh column (opt-in) and any Garmin-style sensor extension columns (heart_rate_bpm, cadence_rpm, power_w, temperature_c) that are present. points selects track/route/waypoint/all; delimiter picks the separator (comma/semicolon/tab/pipe); header toggles the header row; time_format writes the time column as iso/unix or drops it (none); speed adds derived ground speed. Coordinates and elevations are preserved verbatim; fields are RFC-4180 escaped. Paste the GPX contents in the 'gpx' field. Runs entirely in-browser; nothing is uploaded.",
        parameters = schema_json()
    )
)]
impl GpxToCsv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gpx-to-csv", |a: Args| {
            convert_str(
                &a.gpx,
                &a.points,
                &a.delimiter,
                a.header,
                &a.time_format,
                a.speed,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. (Regenerate this literal when the descriptor changes.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "gpx": { "type": "string", "description": "The GPX document to convert. Paste the whole file contents — an XML document with <trkpt> (track), <rtept> (route), and/or <wpt> (waypoint) elements carrying lat/lon attributes." },
                    "points": { "type": "string", "enum": ["all", "track", "route", "waypoint"], "default": "all", "description": "Which point types to include as rows. all (default) emits track, route, and waypoint points; or restrict to one of track, route, waypoint." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "Output CSV field separator. Default comma. Use semicolon for comma-decimal locales, tab for TSV, or pipe." },
                    "header": { "type": "boolean", "default": true, "description": "Emit the column-name header row. Default true; set false for a headerless CSV." },
                    "time_format": { "type": "string", "enum": ["iso", "unix", "none"], "default": "iso", "description": "How the time column is written. iso (default) keeps the original ISO-8601 text; unix converts each timestamp to epoch seconds; none drops the time column entirely." },
                    "speed": { "type": "boolean", "default": false, "description": "Add a speed_kmh column with ground speed derived from consecutive timestamps (haversine distance ÷ Δt) along each track segment / route. Default false." }
                },
                "required": ["gpx"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
