//! gizza-ai/gpx-to-geojson — chat skill block on the shared tool abstraction.
//! Converts GPX/KML GPS tracks and waypoints into GeoJSON, and converts
//! GeoJSON back into GPX. Input format (GPX/KML XML vs. GeoJSON) is
//! auto-detected from the content — no separate format-selector param. The
//! chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → runs on all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_gpx_to_geojson_core::{convert, OutputFormat, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_output_format")]
    output_format: String,
    #[serde(default = "default_true")]
    include_styles: bool,
}
fn default_output_format() -> String {
    "geojson".to_string()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe(
                    "The document to convert: GPX or KML XML text, or a GeoJSON document \
                     (FeatureCollection, Feature, or bare geometry). The format is auto-detected \
                     from the content — no separate format selector needed.",
                ),
        )
        .param(
            Param::enumv("output_format", ["geojson", "gpx"])
                .default("geojson")
                .describe(
                    "Target format. 'geojson' (default) converts GPX/KML input into a GeoJSON \
                     FeatureCollection. 'gpx' converts GeoJSON input back into a GPX document \
                     (Point -> wpt, LineString/MultiLineString -> trk, Polygon's exterior ring -> \
                     trk).",
                ),
        )
        .param(
            Param::boolean("include_styles")
                .default(true)
                .describe(
                    "KML input only: fold each Placemark's Style/styleUrl/StyleMap color and \
                     width into simplestyle-spec properties (stroke, stroke-width, \
                     stroke-opacity, fill, fill-opacity, marker-color). Ignored for GPX input and \
                     for output_format=gpx. Default true.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gpx-to-geojson",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert GPX/KML GPS tracks and waypoints into GeoJSON, and back",
    skill(
        description = "Convert GPX or KML GPS data into GeoJSON, or GeoJSON back into GPX — the input format is auto-detected from the content. GPX -> GeoJSON: <trk>/<trkseg>/<trkpt> becomes a LineString (or MultiLineString for a multi-segment track), <rte>/<rtept> becomes a LineString, <wpt> becomes a Point; standard tags (name, cmt, desc, link, keywords, sym, type) become properties, elevation becomes the position's third coordinate, and per-point timestamps become a coordTimes property. KML -> GeoJSON: a Placemark's Point/LineString/Polygon becomes the matching geometry, MultiGeometry becomes a GeometryCollection; name/description become properties, ExtendedData/SimpleData become arbitrary properties, TimeSpan/TimeStamp become begin/end/time properties, and (when include_styles=true, the default) each Placemark's inline or shared Style/styleUrl/StyleMap color and line width are resolved into simplestyle-spec properties (stroke, stroke-width, stroke-opacity, fill, fill-opacity, marker-color). Set output_format=gpx to convert GeoJSON back into GPX: Point becomes a wpt, LineString/MultiLineString becomes a trk (one trkseg per line, restoring per-point <time> from a coordTimes property when present), and a Polygon's exterior ring becomes a trk (GPX has no polygon primitive, so interior rings/holes are dropped). Runs fully locally, no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gpx-to-geojson", |a: Args| {
            let output_format = OutputFormat::parse(&a.output_format).map_err(SkillError::InvalidArgs)?;
            let opt = Options { output_format, include_styles: a.include_styles };
            convert(&a.input, &opt).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "The document to convert: GPX or KML XML text, or a GeoJSON document (FeatureCollection, Feature, or bare geometry). The format is auto-detected from the content — no separate format selector needed." },
                    "output_format": { "type": "string", "enum": ["geojson", "gpx"], "default": "geojson", "description": "Target format. 'geojson' (default) converts GPX/KML input into a GeoJSON FeatureCollection. 'gpx' converts GeoJSON input back into a GPX document (Point -> wpt, LineString/MultiLineString -> trk, Polygon's exterior ring -> trk)." },
                    "include_styles": { "type": "boolean", "default": true, "description": "KML input only: fold each Placemark's Style/styleUrl/StyleMap color and width into simplestyle-spec properties (stroke, stroke-width, stroke-opacity, fill, fill-opacity, marker-color). Ignored for GPX input and for output_format=gpx. Default true." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
