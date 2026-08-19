//! gizza-ai/topojson-to-geojson — expand a compact TopoJSON topology (shared
//! arcs, delta encoding, quantization transform) into standard GeoJSON.
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_topojson_to_geojson_core::{topojson_to_geojson, Options, Output};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    topojson: String,
    #[serde(default)]
    object: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    include_bbox: bool,
    #[serde(default = "default_precision")]
    precision: i64,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_output() -> String {
    "feature-collection".into()
}
fn default_precision() -> i64 {
    -1
}
fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("topojson")
                .required()
                .describe("The TopoJSON text to expand — a document with \"type\": \"Topology\", an \"objects\" map and an \"arcs\" array, e.g. {\"type\":\"Topology\",\"objects\":{\"l\":{\"type\":\"LineString\",\"arcs\":[0]}},\"arcs\":[[[0,0],[1,1]]]}. Quantized topologies (a \"transform\" with scale/translate and delta-encoded arcs) are decoded automatically."),
        )
        .param(
            Param::string("object")
                .default("")
                .describe("Name of the single entry in the topology's \"objects\" map to expand, e.g. 'countries'. Leave blank (the default) to expand every object and merge them into one collection, in document order. An unknown name is rejected with the list of names the topology actually has."),
        )
        .param(
            Param::enumv("output", ["feature-collection", "geometry-collection"])
                .default("feature-collection")
                .describe("Shape of the result: 'feature-collection' (default) emits an RFC 7946 FeatureCollection whose features keep their properties, id and bbox; 'geometry-collection' emits a bare GeoJSON GeometryCollection of geometries only, dropping properties and id."),
        )
        .param(
            Param::boolean("include_bbox")
                .default(false)
                .describe("Add a 2D \"bbox\": [west, south, east, north] to the top-level result, computed from the coordinates actually emitted (so it stays correct when a single object was selected). Off by default."),
        )
        .param(
            Param::integer("precision")
                .min(-1.0)
                .max(15.0)
                .default(-1)
                .describe("Round every coordinate to this many decimal places (0-15). Quantized topologies decode to values like -179.99999999999997; rounding removes that float noise and shrinks the output. Use -1 (the default) to keep full precision."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per level (1-8). Use 0 to minify to a single compact line. Default 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(a: &Args) -> Options {
    Options {
        object: a.object.clone(),
        output: Output::parse(&a.output),
        include_bbox: a.include_bbox,
        precision: a.precision,
        indent: a.indent as usize,
    }
}

#[cfg(target_arch = "wasm32")]
struct TopoJsonToGeoJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/topojson-to-geojson",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Expand compact TopoJSON with shared arcs into standard GeoJSON",
    skill(
        description = "Convert a compact TopoJSON topology into standard RFC 7946 GeoJSON. Shared arcs are decoded once and stitched back into full coordinate rings and lines (negative arc indexes are traversed backwards), the quantization transform (scale + translate) is applied, and delta-encoded arc positions are accumulated back to absolute coordinates. Handles Point, MultiPoint, LineString, MultiLineString, Polygon, MultiPolygon and GeometryCollection, keeping each feature's properties, id and bbox. Set object to expand one named entry of the topology's objects map, or leave it blank to merge them all. output is 'feature-collection' (default) or 'geometry-collection'; include_bbox adds a computed bounding box; precision rounds coordinates to N decimals (-1 = full); indent is spaces per level (0 minifies). Invalid arc indexes, unknown geometry types, unknown object names and non-Topology input are reported by name. Runs locally.",
        parameters = schema_json()
    ),
)]
impl TopoJsonToGeoJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "topojson-to-geojson", |a: Args| {
            let opts = build_options(&a);
            topojson_to_geojson(&a.topojson, &opts).map_err(SkillError::InvalidArgs)
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
            r#"{
                "type": "object",
                "properties": {
                    "topojson":     { "type": "string", "description": "The TopoJSON text to expand — a document with \"type\": \"Topology\", an \"objects\" map and an \"arcs\" array, e.g. {\"type\":\"Topology\",\"objects\":{\"l\":{\"type\":\"LineString\",\"arcs\":[0]}},\"arcs\":[[[0,0],[1,1]]]}. Quantized topologies (a \"transform\" with scale/translate and delta-encoded arcs) are decoded automatically." },
                    "object":       { "type": "string", "default": "", "description": "Name of the single entry in the topology's \"objects\" map to expand, e.g. 'countries'. Leave blank (the default) to expand every object and merge them into one collection, in document order. An unknown name is rejected with the list of names the topology actually has." },
                    "output":       { "type": "string", "enum": ["feature-collection", "geometry-collection"], "default": "feature-collection", "description": "Shape of the result: 'feature-collection' (default) emits an RFC 7946 FeatureCollection whose features keep their properties, id and bbox; 'geometry-collection' emits a bare GeoJSON GeometryCollection of geometries only, dropping properties and id." },
                    "include_bbox": { "type": "boolean", "default": false, "description": "Add a 2D \"bbox\": [west, south, east, north] to the top-level result, computed from the coordinates actually emitted (so it stays correct when a single object was selected). Off by default." },
                    "precision":    { "type": "integer", "minimum": -1, "maximum": 15, "default": -1, "description": "Round every coordinate to this many decimal places (0-15). Quantized topologies decode to values like -179.99999999999997; rounding removes that float noise and shrinks the output. Use -1 (the default) to keep full precision." },
                    "indent":       { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per level (1-8). Use 0 to minify to a single compact line. Default 2." }
                },
                "required": ["topojson"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn build_options_maps_args() {
        let a = Args {
            topojson: "{}".into(),
            object: "countries".into(),
            output: "geometry-collection".into(),
            include_bbox: true,
            precision: 6,
            indent: 0,
        };
        let o = build_options(&a);
        assert_eq!(o.object, "countries");
        assert_eq!(o.output, Output::GeometryCollection);
        assert!(o.include_bbox);
        assert_eq!(o.precision, 6);
        assert_eq!(o.indent, 0);
    }

    #[test]
    fn defaults_expand_every_object_as_features() {
        let a: Args = serde_json::from_str(r#"{"topojson":"{}"}"#).unwrap();
        let o = build_options(&a);
        assert_eq!(o.object, "");
        assert_eq!(o.output, Output::FeatureCollection);
        assert!(!o.include_bbox);
        assert_eq!(o.precision, -1);
        assert_eq!(o.indent, 2);
    }
}
