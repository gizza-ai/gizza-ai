//! gizza-ai/gpx-to-kml — chat skill block on the shared tool abstraction.
//! Converts a GPX GPS document into KML for Google Earth: tracks/routes become
//! LineStrings, waypoints become Points, with configurable line color/width/
//! opacity, waypoint icon color, and altitude interpretation. The chat schema
//! is single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_gpx_to_kml_core::{convert, AltitudeMode, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    gpx: String,
    #[serde(default = "default_line_color")]
    line_color: String,
    #[serde(default = "default_line_width")]
    line_width: u32,
    #[serde(default = "default_line_opacity")]
    line_opacity: u32,
    #[serde(default = "default_waypoint_color")]
    waypoint_color: String,
    #[serde(default = "default_altitude_mode")]
    altitude_mode: String,
    #[serde(default)]
    document_name: String,
}
fn default_line_color() -> String {
    "#ef4444".to_string()
}
fn default_line_width() -> u32 {
    4
}
fn default_line_opacity() -> u32 {
    80
}
fn default_waypoint_color() -> String {
    "#3b82f6".to_string()
}
fn default_altitude_mode() -> String {
    "clamp_to_ground".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("gpx").required().describe(
            "The GPX document to convert, as XML text. Tracks (<trk>) and routes (<rte>) \
                     become KML LineStrings; waypoints (<wpt>) become Points. Names, descriptions, \
                     elevation, and timestamps are carried over where present.",
        ))
        .param(Param::string("line_color").default("#ef4444").describe(
            "Track/route line color as a CSS hex value (#RRGGBB or #RGB). Converted to \
                     KML's aabbggrr color for the shared LineStyle. Default #ef4444 (red).",
        ))
        .param(
            Param::integer("line_width")
                .default(4)
                .min(1.0)
                .max(20.0)
                .describe(
                    "Track/route line width in pixels (Google Earth pen width). 1–20, default 4.",
                ),
        )
        .param(
            Param::integer("line_opacity")
                .default(80)
                .min(0.0)
                .max(100.0)
                .describe(
                    "Track/route line opacity as a percentage, 0 (fully transparent) to 100 \
                     (opaque). Becomes the alpha byte of the KML line color. Default 80.",
                ),
        )
        .param(Param::string("waypoint_color").default("#3b82f6").describe(
            "Waypoint icon color as a CSS hex value (#RRGGBB or #RGB), applied fully \
                     opaque to the shared IconStyle. Default #3b82f6 (blue).",
        ))
        .param(
            Param::enumv(
                "altitude_mode",
                ["clamp_to_ground", "absolute", "relative_to_ground"],
            )
            .default("clamp_to_ground")
            .describe(
                "How Google Earth interprets each coordinate's altitude. 'clamp_to_ground' \
                     (default) drapes geometry on the terrain and ignores elevation; 'absolute' \
                     reads elevation as metres above sea level; 'relative_to_ground' reads it as \
                     metres above the terrain.",
            ),
        )
        .param(Param::string("document_name").describe(
            "Optional name for the KML <Document>. Falls back to the GPX \
                     <metadata><name>, then a generic label when neither is set.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gpx-to-kml",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a GPX GPS track, route, and waypoints into KML for Google Earth",
    skill(
        description = "Convert a GPX GPS document into a KML 2.2 document that opens directly in Google Earth. Tracks (<trk>/<trkseg>/<trkpt>) become KML LineStrings — a multi-segment track becomes a MultiGeometry of LineStrings — and routes (<rte>/<rtept>) become LineStrings too; waypoints (<wpt>) become Points. Names and descriptions (<name>, <desc>, falling back to <cmt>) are carried onto each Placemark, elevation (<ele>) becomes the third lon,lat,ele coordinate, per-point timestamps become a track <TimeSpan> and a waypoint <TimeStamp>. Styling is emitted as two shared Style blocks the Placemarks reference: line_color (CSS hex, default #ef4444) + line_width (1–20, default 4) + line_opacity (0–100%, default 80) drive a LineStyle, and waypoint_color (CSS hex, default #3b82f6) drives an IconStyle. CSS #RRGGBB + opacity% are converted to KML's aabbggrr byte order (alpha, blue, green, red). altitude_mode (clamp_to_ground/absolute/relative_to_ground, default clamp_to_ground) sets how Google Earth reads each coordinate's altitude. document_name optionally names the KML <Document> (falls back to the GPX <metadata><name>). Errors on empty input or a document with no <trk>/<rte>/<wpt>. Pure and deterministic; runs locally, no network.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gpx-to-kml", |a: Args| {
            let altitude_mode =
                AltitudeMode::parse(&a.altitude_mode).map_err(SkillError::InvalidArgs)?;
            let opt = Options {
                line_color: a.line_color,
                line_width: a.line_width,
                line_opacity: a.line_opacity,
                waypoint_color: a.waypoint_color,
                altitude_mode,
                document_name: Some(a.document_name),
            };
            convert(&a.gpx, &opt).map_err(SkillError::InvalidArgs)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "gpx": { "type": "string", "description": "The GPX document to convert, as XML text. Tracks (<trk>) and routes (<rte>) become KML LineStrings; waypoints (<wpt>) become Points. Names, descriptions, elevation, and timestamps are carried over where present." },
                    "line_color": { "type": "string", "default": "#ef4444", "description": "Track/route line color as a CSS hex value (#RRGGBB or #RGB). Converted to KML's aabbggrr color for the shared LineStyle. Default #ef4444 (red)." },
                    "line_width": { "type": "integer", "minimum": 1, "maximum": 20, "default": 4, "description": "Track/route line width in pixels (Google Earth pen width). 1–20, default 4." },
                    "line_opacity": { "type": "integer", "minimum": 0, "maximum": 100, "default": 80, "description": "Track/route line opacity as a percentage, 0 (fully transparent) to 100 (opaque). Becomes the alpha byte of the KML line color. Default 80." },
                    "waypoint_color": { "type": "string", "default": "#3b82f6", "description": "Waypoint icon color as a CSS hex value (#RRGGBB or #RGB), applied fully opaque to the shared IconStyle. Default #3b82f6 (blue)." },
                    "altitude_mode": { "type": "string", "enum": ["clamp_to_ground", "absolute", "relative_to_ground"], "default": "clamp_to_ground", "description": "How Google Earth interprets each coordinate's altitude. 'clamp_to_ground' (default) drapes geometry on the terrain and ignores elevation; 'absolute' reads elevation as metres above sea level; 'relative_to_ground' reads it as metres above the terrain." },
                    "document_name": { "type": "string", "description": "Optional name for the KML <Document>. Falls back to the GPX <metadata><name>, then a generic label when neither is set." }
                },
                "required": ["gpx"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
