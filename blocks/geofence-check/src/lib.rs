//! gizza-ai/geofence-check — chat skill block on the shared tool abstraction.
//! Test whether latitude/longitude points fall inside a polygon (point-in-polygon).
//! The chat schema is single-sourced from descriptor() (which also drives the CLI
//! and, via manifest.json, the page form); handle() delegates to run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_geofence_check_core::check;
use serde::Deserialize;
use wafer_sdk::*;

fn default_coord_order() -> String {
    "lat_lon".into()
}
fn default_boundary() -> String {
    "inside".into()
}
fn default_output() -> String {
    "text".into()
}

#[derive(Deserialize)]
struct Args {
    polygon: String,
    points: String,
    #[serde(default = "default_coord_order")]
    coord_order: String,
    #[serde(default = "default_boundary")]
    boundary: String,
    #[serde(default = "default_output")]
    output: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("polygon").required().describe(
            "The boundary polygon. Accepts GeoJSON — a Polygon or MultiPolygon (interior \
             rings act as holes), or a Feature/FeatureCollection/GeometryCollection wrapping \
             them (all polygons are unioned) — where coordinates are [longitude, latitude]. \
             Or a simple ring as `lat,lon` lines (one vertex per line) or a JSON array of \
             [lat,lon] pairs; the ring is auto-closed and its coordinate order follows \
             coord_order. Needs at least 3 vertices.",
        ))
        .param(Param::string("points").required().describe(
            "The points to test, one or many. Accepts CSV lines `lat,lon` (an optional third \
             field is a label), a JSON array of [lat,lon] pairs or {lat,lon,label} objects, or \
             GeoJSON Point/MultiPoint/Feature/FeatureCollection ([longitude, latitude]). CSV and \
             JSON-pair coordinate order follows coord_order; GeoJSON is always [lon, lat]. \
             Blank lines and lines starting with # are ignored; a non-numeric first line is \
             treated as a header.",
        ))
        .param(
            Param::enumv("coord_order", ["lat_lon", "lon_lat"])
                .default("lat_lon")
                .describe(
                    "Coordinate order for the non-GeoJSON forms (CSV lines and JSON pairs): \
                     'lat_lon' (default, e.g. `51.5,-0.12`) or 'lon_lat'. GeoJSON always uses \
                     [longitude, latitude] per RFC 7946 and ignores this.",
                ),
        )
        .param(
            Param::enumv("boundary", ["inside", "outside", "boundary"])
                .default("inside")
                .describe(
                    "How a point that lies exactly on an edge or vertex is classified: 'inside' \
                     (default) counts it as inside, 'outside' counts it as outside, 'boundary' \
                     reports it with its own 'boundary' status.",
                ),
        )
        .param(
            Param::enumv("output", ["text", "csv", "json"])
                .default("text")
                .describe(
                    "Output format: 'text' (default) a summary line plus one line per point; \
                     'csv' a table with columns point, latitude, longitude[, label], status; \
                     'json' an object with a summary and a points array.",
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
    name = "gizza-ai/geofence-check",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Test whether latitude/longitude points fall inside a polygon",
    skill(
        description = "Test whether latitude/longitude points fall inside a polygon (point-in-polygon / geofence). Pass `polygon` as GeoJSON (Polygon or MultiPolygon with holes, or a Feature/FeatureCollection wrapping them; coordinates are [longitude, latitude]) or a simple ring as `lat,lon` lines / a JSON array of [lat,lon] pairs. Pass `points` as CSV `lat,lon` lines (an optional third field is a label), a JSON array of [lat,lon] pairs or {lat,lon,label} objects, or GeoJSON Point/MultiPoint features. `coord_order` ('lat_lon' default / 'lon_lat') applies to the non-GeoJSON forms; GeoJSON is always [lon, lat]. `boundary` chooses how on-edge points are classified: 'inside' (default), 'outside', or 'boundary' (its own status). `output` is 'text' (default), 'csv', or 'json'. Uses even-odd ray casting; holes and multipolygons are honored. Coordinates are decimal degrees (lat -90..90, lon -180..180). Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "geofence-check", |a: Args| {
            check(&a.polygon, &a.points, &a.coord_order, &a.boundary, &a.output)
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "polygon": { "type": "string", "description": "The boundary polygon. Accepts GeoJSON — a Polygon or MultiPolygon (interior rings act as holes), or a Feature/FeatureCollection/GeometryCollection wrapping them (all polygons are unioned) — where coordinates are [longitude, latitude]. Or a simple ring as `lat,lon` lines (one vertex per line) or a JSON array of [lat,lon] pairs; the ring is auto-closed and its coordinate order follows coord_order. Needs at least 3 vertices." },
                    "points": { "type": "string", "description": "The points to test, one or many. Accepts CSV lines `lat,lon` (an optional third field is a label), a JSON array of [lat,lon] pairs or {lat,lon,label} objects, or GeoJSON Point/MultiPoint/Feature/FeatureCollection ([longitude, latitude]). CSV and JSON-pair coordinate order follows coord_order; GeoJSON is always [lon, lat]. Blank lines and lines starting with # are ignored; a non-numeric first line is treated as a header." },
                    "coord_order": { "type": "string", "enum": ["lat_lon", "lon_lat"], "default": "lat_lon", "description": "Coordinate order for the non-GeoJSON forms (CSV lines and JSON pairs): 'lat_lon' (default, e.g. `51.5,-0.12`) or 'lon_lat'. GeoJSON always uses [longitude, latitude] per RFC 7946 and ignores this." },
                    "boundary": { "type": "string", "enum": ["inside", "outside", "boundary"], "default": "inside", "description": "How a point that lies exactly on an edge or vertex is classified: 'inside' (default) counts it as inside, 'outside' counts it as outside, 'boundary' reports it with its own 'boundary' status." },
                    "output": { "type": "string", "enum": ["text", "csv", "json"], "default": "text", "description": "Output format: 'text' (default) a summary line plus one line per point; 'csv' a table with columns point, latitude, longitude[, label], status; 'json' an object with a summary and a points array." }
                },
                "required": ["polygon", "points"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
