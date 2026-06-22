//! gizza-ai/geojson-to-svg — render GeoJSON features into a standalone SVG map,
//! with no tile servers and no network. Pure-Rust, so it runs on every backend
//! (incl. the chat Service Worker). The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. The SVG markup is returned as text (like dot-to-svg).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_geojson_to_svg_core::{render, Options};
use serde::Deserialize;
use wafer_sdk::*;

fn default_width() -> f64 {
    800.0
}
fn default_fill() -> String {
    "#cfe8ff".into()
}
fn default_stroke() -> String {
    "#1f6feb".into()
}
fn default_stroke_width() -> f64 {
    1.0
}
fn default_point_radius() -> f64 {
    4.0
}
fn default_background() -> String {
    "#ffffff".into()
}
fn default_projection() -> String {
    "mercator".into()
}

#[derive(Deserialize)]
struct Args {
    geojson: String,
    #[serde(default = "default_width")]
    width: f64,
    #[serde(default = "default_fill")]
    fill: String,
    #[serde(default = "default_stroke")]
    stroke: String,
    #[serde(default = "default_stroke_width")]
    stroke_width: f64,
    #[serde(default = "default_point_radius")]
    point_radius: f64,
    #[serde(default = "default_background")]
    background: String,
    #[serde(default = "default_projection")]
    projection: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("geojson").required().describe(
            "GeoJSON to render. Accepts a FeatureCollection, a single Feature, or a bare geometry \
             (Point, MultiPoint, LineString, MultiLineString, Polygon, MultiPolygon, \
             GeometryCollection). Coordinates are [longitude, latitude].",
        ))
        .param(
            Param::number("width")
                .default(800.0)
                .min(16.0)
                .max(4096.0)
                .describe("Output SVG width in pixels; height is derived to preserve aspect ratio (default 800)."),
        )
        .param(
            Param::string("fill")
                .default("#cfe8ff")
                .describe("Fill colour for polygons, any CSS colour (default #cfe8ff)."),
        )
        .param(
            Param::string("stroke")
                .default("#1f6feb")
                .describe("Stroke colour for outlines, lines and point rings (default #1f6feb)."),
        )
        .param(
            Param::number("stroke_width")
                .default(1.0)
                .min(0.0)
                .max(50.0)
                .describe("Stroke width in pixels (default 1)."),
        )
        .param(
            Param::number("point_radius")
                .default(4.0)
                .min(0.1)
                .max(100.0)
                .describe("Radius in pixels for Point/MultiPoint markers (default 4)."),
        )
        .param(
            Param::string("background")
                .default("#ffffff")
                .describe("Background colour, or 'none'/'transparent' for no background (default #ffffff)."),
        )
        .param(
            Param::enumv("projection", ["mercator", "none"])
                .default("mercator")
                .describe(
                    "Map projection: 'mercator' applies a spherical Web-Mercator transform; \
                     'none' plots raw longitude/latitude (default mercator).",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn opts(a: &Args) -> Options {
    Options {
        width: a.width,
        padding: 16.0,
        fill: a.fill.clone(),
        stroke: a.stroke.clone(),
        stroke_width: a.stroke_width,
        point_radius: a.point_radius,
        background: a.background.clone(),
        mercator: a.projection != "none",
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/geojson-to-svg",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render GeoJSON to an SVG map",
    skill(
        description = "Render GeoJSON features into a standalone SVG map — no tile servers, no network. Pass `geojson` as a FeatureCollection, a single Feature, or a bare geometry (Point, LineString, Polygon and their Multi/Collection variants); coordinates are [longitude, latitude]. Polygons are filled, lines stroked, points drawn as circles, with holes cut out. Optional `width`, `fill`, `stroke`, `stroke_width`, `point_radius`, `background` ('none' for transparent) and `projection` ('mercator' Web-Mercator or 'none' for raw lon/lat) control the look; everything is scaled to fit the viewport with the aspect ratio preserved. Returns the SVG markup as text. Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "geojson-to-svg", |a: Args| {
            render(&a.geojson, &opts(&a)).map_err(SkillError::InvalidArgs)
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
                    "geojson": { "type": "string", "description": "GeoJSON to render. Accepts a FeatureCollection, a single Feature, or a bare geometry (Point, MultiPoint, LineString, MultiLineString, Polygon, MultiPolygon, GeometryCollection). Coordinates are [longitude, latitude]." },
                    "width": { "type": "number", "default": 800.0, "minimum": 16, "maximum": 4096, "description": "Output SVG width in pixels; height is derived to preserve aspect ratio (default 800)." },
                    "fill": { "type": "string", "default": "#cfe8ff", "description": "Fill colour for polygons, any CSS colour (default #cfe8ff)." },
                    "stroke": { "type": "string", "default": "#1f6feb", "description": "Stroke colour for outlines, lines and point rings (default #1f6feb)." },
                    "stroke_width": { "type": "number", "default": 1.0, "minimum": 0, "maximum": 50, "description": "Stroke width in pixels (default 1)." },
                    "point_radius": { "type": "number", "default": 4.0, "minimum": 0.1, "maximum": 100, "description": "Radius in pixels for Point/MultiPoint markers (default 4)." },
                    "background": { "type": "string", "default": "#ffffff", "description": "Background colour, or 'none'/'transparent' for no background (default #ffffff)." },
                    "projection": { "type": "string", "enum": ["mercator", "none"], "default": "mercator", "description": "Map projection: 'mercator' applies a spherical Web-Mercator transform; 'none' plots raw longitude/latitude (default mercator)." }
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
