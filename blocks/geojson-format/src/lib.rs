//! gizza-ai/geojson-format — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_indent")]
    indent: i64,
    #[serde(default = "default_indent_char")]
    indent_char: String,
    #[serde(default = "default_precision")]
    precision: i64,
    #[serde(default = "default_key_order")]
    key_order: String,
    #[serde(default = "default_bbox")]
    bbox: String,
    #[serde(default = "default_winding")]
    winding: String,
    #[serde(default)]
    keep_properties: String,
    #[serde(default)]
    drop_properties: String,
    #[serde(default)]
    drop_empty_properties: bool,
    #[serde(default = "default_true")]
    validate: bool,
}
fn default_indent() -> i64 {
    2
}
fn default_indent_char() -> String {
    "space".into()
}
fn default_precision() -> i64 {
    -1
}
fn default_key_order() -> String {
    "keep".into()
}
fn default_bbox() -> String {
    "keep".into()
}
fn default_winding() -> String {
    "keep".into()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("A single GeoJSON document to format: a FeatureCollection, Feature, or bare geometry object. Paste valid JSON with a GeoJSON type member. The document shape is preserved; use geojson-merge for multiple or line-delimited inputs."))
        .param(Param::integer("indent").min(0.0).max(8.0).default(2).describe("Indent units per nesting level. 2 pretty-prints with two spaces (default); 0 minifies to one line. Values above 8 are rejected."))
        .param(Param::enumv("indent_char", ["space", "tab"]).default("space").describe("Indentation character for pretty output: spaces (default) or tabs. Ignored when indent=0."))
        .param(Param::integer("precision").min(-1.0).max(15.0).default(-1).describe("Decimal places to round coordinates to. -1 keeps full precision (default); 0 rounds to whole degrees; 5 is roughly metre-level at the equator. Rounding is lossy."))
        .param(Param::enumv("key_order", ["keep", "canonical", "alpha"]).default("keep").describe("Object member ordering. keep preserves the input order; canonical writes GeoJSON members in RFC-style order while preserving properties order; alpha sorts every object key alphabetically, including properties."))
        .param(Param::enumv("bbox", ["keep", "add", "features", "strip"]).default("keep").describe("Bounding-box handling: keep existing bbox members, add/recompute the top-level bbox, add feature bboxes plus the top-level bbox, or strip every bbox member."))
        .param(Param::enumv("winding", ["keep", "rfc7946"]).default("keep").describe("Polygon ring orientation. keep leaves rings untouched; rfc7946 rewinds exterior rings counterclockwise and holes clockwise (right-hand rule)."))
        .param(Param::string("keep_properties").describe("Optional comma- or newline-separated feature property names to keep. When set, all other properties are removed from Feature objects."))
        .param(Param::string("drop_properties").describe("Optional comma- or newline-separated feature property names to remove. Applied after keep_properties."))
        .param(Param::boolean("drop_empty_properties").default(false).describe("Remove feature properties whose value is null, an empty string, an empty array, or an empty object."))
        .param(Param::boolean("validate").default(true).describe("Validate the input as RFC 7946 GeoJSON before formatting (default true): geometry types, coordinate ranges, required Feature members, and polygon ring closure. Turn off only to clean up known-nonconforming data."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/geojson-format",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Format, minify, round and canonicalize GeoJSON",
    skill(
        description = "Pretty-print or minify one GeoJSON document while preserving its shape (FeatureCollection, Feature, or bare geometry). Optionally round coordinate precision, reorder keys (keep, canonical GeoJSON order, or alphabetical), recompute or strip bbox members, rewind polygon rings to the RFC 7946 right-hand rule, prune feature properties, drop empty property values, and validate RFC 7946 geometry structure before output. Use indent=0 for minified output, precision=-1 to keep coordinates unchanged, bbox='features' to add per-feature and top-level boxes, and validate=false only for known-nonconforming data you still need to clean up. Runs locally and returns GeoJSON text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "geojson-format", |a: Args| {
            gizza_ai_geojson_format_core::run(
                &a.input,
                a.indent,
                &a.indent_char,
                a.precision,
                &a.key_order,
                &a.bbox,
                &a.winding,
                &a.keep_properties,
                &a.drop_properties,
                a.drop_empty_properties,
                a.validate,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type":"object",
            "properties":{
                "input":{"type":"string","description":"A single GeoJSON document to format: a FeatureCollection, Feature, or bare geometry object. Paste valid JSON with a GeoJSON type member. The document shape is preserved; use geojson-merge for multiple or line-delimited inputs."},
                "indent":{"type":"integer","minimum":0,"maximum":8,"default":2,"description":"Indent units per nesting level. 2 pretty-prints with two spaces (default); 0 minifies to one line. Values above 8 are rejected."},
                "indent_char":{"type":"string","enum":["space","tab"],"default":"space","description":"Indentation character for pretty output: spaces (default) or tabs. Ignored when indent=0."},
                "precision":{"type":"integer","minimum":-1,"maximum":15,"default":-1,"description":"Decimal places to round coordinates to. -1 keeps full precision (default); 0 rounds to whole degrees; 5 is roughly metre-level at the equator. Rounding is lossy."},
                "key_order":{"type":"string","enum":["keep","canonical","alpha"],"default":"keep","description":"Object member ordering. keep preserves the input order; canonical writes GeoJSON members in RFC-style order while preserving properties order; alpha sorts every object key alphabetically, including properties."},
                "bbox":{"type":"string","enum":["keep","add","features","strip"],"default":"keep","description":"Bounding-box handling: keep existing bbox members, add/recompute the top-level bbox, add feature bboxes plus the top-level bbox, or strip every bbox member."},
                "winding":{"type":"string","enum":["keep","rfc7946"],"default":"keep","description":"Polygon ring orientation. keep leaves rings untouched; rfc7946 rewinds exterior rings counterclockwise and holes clockwise (right-hand rule)."},
                "keep_properties":{"type":"string","description":"Optional comma- or newline-separated feature property names to keep. When set, all other properties are removed from Feature objects."},
                "drop_properties":{"type":"string","description":"Optional comma- or newline-separated feature property names to remove. Applied after keep_properties."},
                "drop_empty_properties":{"type":"boolean","default":false,"description":"Remove feature properties whose value is null, an empty string, an empty array, or an empty object."},
                "validate":{"type":"boolean","default":true,"description":"Validate the input as RFC 7946 GeoJSON before formatting (default true): geometry types, coordinate ranges, required Feature members, and polygon ring closure. Turn off only to clean up known-nonconforming data."}
            },
            "required":["input"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
