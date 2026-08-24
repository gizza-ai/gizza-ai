//! gizza-ai/geojson-coords-to-csv — chat skill block on the shared tool abstraction.
//! Explode every coordinate in a GeoJSON document into a flat CSV, one row per
//! position. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI and, via manifest.json, the page form); handle() delegates to
//! block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_geojson_coords_to_csv_core::convert_str;
use serde::Deserialize;
use wafer_sdk::*;

fn default_order() -> String {
    "lonlat".into()
}
fn default_columns() -> String {
    "coords".into()
}
fn default_shapes() -> String {
    "all".into()
}
fn default_elevation() -> String {
    "auto".into()
}
fn default_precision() -> f64 {
    -1.0
}
fn default_dedupe() -> String {
    "none".into()
}
fn default_delimiter() -> String {
    "comma".into()
}
fn default_header() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    geojson: String,
    #[serde(default = "default_order")]
    order: String,
    #[serde(default = "default_columns")]
    columns: String,
    #[serde(default = "default_shapes")]
    shapes: String,
    #[serde(default = "default_elevation")]
    elevation: String,
    #[serde(default = "default_precision")]
    precision: f64,
    #[serde(default = "default_dedupe")]
    dedupe: String,
    #[serde(default)]
    properties: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_header")]
    header: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("geojson").required().describe(
            "GeoJSON to explode into coordinates. Accepts a FeatureCollection, a single Feature, a \
             bare geometry (Point, MultiPoint, LineString, MultiLineString, Polygon, MultiPolygon, \
             GeometryCollection) or a top-level array of any of those. Every position in the \
             document becomes ONE CSV row, in document order — a 500-vertex polygon gives 500 \
             rows. Positions are [longitude, latitude] or [longitude, latitude, elevation].",
        ))
        .param(
            Param::enumv("order", ["lonlat", "latlon"])
                .default("lonlat")
                .describe(
                    "Axis order of the two coordinate columns: 'lonlat' (default) writes \
                     longitude,latitude — GeoJSON's own storage order; 'latlon' writes \
                     latitude,longitude, which is what most map UIs and spreadsheets expect. The \
                     header row follows the choice.",
                ),
        )
        .param(
            Param::enumv("columns", ["coords", "indexed", "feature", "full"])
                .default("coords")
                .describe(
                    "Context columns written before the coordinates: 'coords' (default) just the \
                     coordinate columns; 'indexed' adds a 1-based `index`; 'feature' adds \
                     `index`, `feature_index`, `feature_id` (the Feature's `id`, blank when \
                     absent) and `geometry_type`; 'full' additionally adds `part` (1-based part \
                     within the feature), `ring` (1 = a polygon's outer ring, 2+ = a hole, blank \
                     for non-polygons), `position` (1-based position within its ring or part) and \
                     `ring_close` (true on the closing vertex that repeats its ring's first).",
                ),
        )
        .param(
            Param::enumv("shapes", ["all", "points", "lines", "polygons"])
                .default("all")
                .describe(
                    "Which geometries contribute coordinates: 'all' (default); 'points' only \
                     Point/MultiPoint; 'lines' only LineString/MultiLineString; 'polygons' only \
                     Polygon/MultiPolygon. GeometryCollections are always walked and their \
                     members filtered individually.",
                ),
        )
        .param(
            Param::enumv("elevation", ["auto", "always", "never"])
                .default("auto")
                .describe(
                    "When the third `elevation` (Z) column is written: 'auto' (default) only if \
                     at least one coordinate carries a Z value; 'always' writes the column with \
                     blanks for 2-D coordinates; 'never' drops it.",
                ),
        )
        .param(
            Param::integer("precision")
                .default(-1)
                .min(-1.0)
                .max(15.0)
                .describe(
                    "Decimal places for the coordinate columns. -1 (default) keeps each number \
                     exactly as written in the document; 0-15 rounds and pads to that many places \
                     (6 ≈ 0.1 m, 5 ≈ 1 m). Property columns are never reformatted.",
                ),
        )
        .param(
            Param::enumv("dedupe", ["none", "ring-close", "adjacent", "all"])
                .default("none")
                .describe(
                    "Drop repeated coordinates. The levels are cumulative: 'none' (default) emits \
                     every position as stored; 'ring-close' drops each polygon ring's closing \
                     position when it repeats the ring's first; 'adjacent' also drops a position \
                     identical to the one emitted just before it in the same ring or part; 'all' \
                     emits each distinct coordinate once across the whole document.",
                ),
        )
        .param(Param::boolean("properties").default(false).describe(
            "Append the features' `properties` as extra columns (the union of keys, in \
             first-seen order), repeating each feature's values on every one of its coordinate \
             rows. Nested objects/arrays are written as compact JSON. Default false.",
        ))
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"])
                .default("comma")
                .describe(
                    "Output field separator: 'comma' (default), 'semicolon', 'tab' or 'pipe'. \
                     Fields are RFC-4180 escaped (quoted when they contain the delimiter, a quote \
                     or a newline).",
                ),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Emit a header row of column names. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/geojson-coords-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract every GeoJSON coordinate into a flat lon,lat CSV",
    skill(
        description = "Extract every coordinate pair in a GeoJSON document into a flat CSV, ONE ROW PER COORDINATE (unlike a feature-level converter, a 500-vertex polygon becomes 500 rows). Pass `geojson` as a FeatureCollection, a single Feature, a bare geometry (Point, MultiPoint, LineString, MultiLineString, Polygon, MultiPolygon, GeometryCollection) or a top-level array of any of those. Columns are `longitude,latitude` plus `elevation` when a coordinate carries a Z value; set `order`='latlon' to swap the axis order. `columns` adds context: 'indexed' (an `index`), 'feature' (`feature_index`, `feature_id`, `geometry_type`) or 'full' (also `part`, `ring`, `position`, `ring_close`). `shapes` limits extraction to points, lines or polygons; `precision` rounds to 0-15 decimals (-1 keeps the source value); `dedupe` drops repeated positions ('ring-close', 'adjacent' or 'all'); `properties`=true appends the feature properties as columns; `delimiter` picks comma/semicolon/tab/pipe and `header`=false drops the header row. Up to 200000 coordinate rows. Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "geojson-coords-to-csv", |a: Args| {
            convert_str(
                &a.geojson,
                &a.order,
                &a.columns,
                &a.shapes,
                &a.elevation,
                a.precision as i32,
                &a.dedupe,
                a.properties,
                &a.delimiter,
                a.header,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "geojson": { "type": "string", "description": "GeoJSON to explode into coordinates. Accepts a FeatureCollection, a single Feature, a bare geometry (Point, MultiPoint, LineString, MultiLineString, Polygon, MultiPolygon, GeometryCollection) or a top-level array of any of those. Every position in the document becomes ONE CSV row, in document order — a 500-vertex polygon gives 500 rows. Positions are [longitude, latitude] or [longitude, latitude, elevation]." },
                    "order": { "type": "string", "enum": ["lonlat", "latlon"], "default": "lonlat", "description": "Axis order of the two coordinate columns: 'lonlat' (default) writes longitude,latitude — GeoJSON's own storage order; 'latlon' writes latitude,longitude, which is what most map UIs and spreadsheets expect. The header row follows the choice." },
                    "columns": { "type": "string", "enum": ["coords", "indexed", "feature", "full"], "default": "coords", "description": "Context columns written before the coordinates: 'coords' (default) just the coordinate columns; 'indexed' adds a 1-based `index`; 'feature' adds `index`, `feature_index`, `feature_id` (the Feature's `id`, blank when absent) and `geometry_type`; 'full' additionally adds `part` (1-based part within the feature), `ring` (1 = a polygon's outer ring, 2+ = a hole, blank for non-polygons), `position` (1-based position within its ring or part) and `ring_close` (true on the closing vertex that repeats its ring's first)." },
                    "shapes": { "type": "string", "enum": ["all", "points", "lines", "polygons"], "default": "all", "description": "Which geometries contribute coordinates: 'all' (default); 'points' only Point/MultiPoint; 'lines' only LineString/MultiLineString; 'polygons' only Polygon/MultiPolygon. GeometryCollections are always walked and their members filtered individually." },
                    "elevation": { "type": "string", "enum": ["auto", "always", "never"], "default": "auto", "description": "When the third `elevation` (Z) column is written: 'auto' (default) only if at least one coordinate carries a Z value; 'always' writes the column with blanks for 2-D coordinates; 'never' drops it." },
                    "precision": { "type": "integer", "default": -1, "minimum": -1, "maximum": 15, "description": "Decimal places for the coordinate columns. -1 (default) keeps each number exactly as written in the document; 0-15 rounds and pads to that many places (6 ≈ 0.1 m, 5 ≈ 1 m). Property columns are never reformatted." },
                    "dedupe": { "type": "string", "enum": ["none", "ring-close", "adjacent", "all"], "default": "none", "description": "Drop repeated coordinates. The levels are cumulative: 'none' (default) emits every position as stored; 'ring-close' drops each polygon ring's closing position when it repeats the ring's first; 'adjacent' also drops a position identical to the one emitted just before it in the same ring or part; 'all' emits each distinct coordinate once across the whole document." },
                    "properties": { "type": "boolean", "default": false, "description": "Append the features' `properties` as extra columns (the union of keys, in first-seen order), repeating each feature's values on every one of its coordinate rows. Nested objects/arrays are written as compact JSON. Default false." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "Output field separator: 'comma' (default), 'semicolon', 'tab' or 'pipe'. Fields are RFC-4180 escaped (quoted when they contain the delimiter, a quote or a newline)." },
                    "header": { "type": "boolean", "default": true, "description": "Emit a header row of column names. Default true." }
                },
                "required": ["geojson"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
