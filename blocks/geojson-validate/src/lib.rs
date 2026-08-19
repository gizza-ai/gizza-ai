//! gizza-ai/geojson-validate — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_true")]
    strict_winding: bool,
    #[serde(default = "default_true")]
    allow_bbox: bool,
    #[serde(default = "default_true")]
    warn_problematic: bool,
    #[serde(default = "default_max_precision")]
    max_precision: i64,
    #[serde(default = "default_max_issues")]
    max_issues: i64,
}
fn default_output() -> String {
    "text".into()
}
fn default_true() -> bool {
    true
}
fn default_max_precision() -> i64 {
    6
}
fn default_max_issues() -> i64 {
    50
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("A single GeoJSON document to check: a FeatureCollection, Feature, or bare geometry object. Paste the raw text — invalid JSON is reported as a finding rather than an error, so broken files are fine to submit."))
        .param(Param::enumv("output", ["text", "json"]).default("text").describe("Report format. text is a readable verdict plus numbered issues and a summary (default); json returns {valid, error_count, warning_count, errors[], warnings[], summary} with a stable kebab-case rule id and JSON path per issue, for scripts and CI."))
        .param(Param::boolean("strict_winding").default(true).describe("Treat RFC 7946 right-hand-rule violations (exterior rings must wind counterclockwise, holes clockwise) as errors that make the document invalid. Set false to report them as warnings instead — the RFC says SHOULD, and older GeoJSON commonly winds the other way."))
        .param(Param::boolean("allow_bbox").default(true).describe("Accept bbox members, checking their length (4 or 6 values), numeric type, ranges and [west, south, east, north] order. Set false to also flag every bbox member itself, for consumers that reject them."))
        .param(Param::boolean("warn_problematic").default(true).describe("Report the 'valid but problematic' family as warnings: duplicate consecutive nodes, zero-area rings, zero-length lines, single-element MultiPoint/MultiLineString/MultiPolygon and GeometryCollections, nested GeometryCollections, null geometries, bare geometries, legacy crs members, antimeridian crossings and excess coordinate precision. Set false for a spec-only check."))
        .param(Param::integer("max_precision").min(-1.0).max(15.0).default(6).describe("Warn when a coordinate carries more decimal places than this. 6 is roughly 0.1 m at the equator and the usual recommendation (default); -1 turns the precision check off. Needs warn_problematic on."))
        .param(Param::integer("max_issues").min(1.0).max(1000.0).default(50).describe("How many issues to LIST per severity before truncating with an 'and N more' line. Counts in the verdict always stay complete. Default 50; raise it to see every issue in a badly broken file."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/geojson-validate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate GeoJSON and report structural, winding and coordinate errors",
    skill(
        description = "Check one GeoJSON document against RFC 7946 and report EVERY problem it finds, not just the first: missing or wrong type/properties/geometry/coordinates members, unknown geometry types, non-numeric or out-of-range coordinates (longitude -180..180, latitude -90..90), positions that are too short or too long, LineStrings under two positions, polygon rings that are unclosed or have fewer than three distinct nodes, exterior/interior ring winding against the right-hand rule, and malformed bbox members. Each issue carries a stable rule id and a JSON path such as features[3].geometry.coordinates[0][2]. A second family of warnings covers valid-but-problematic data (duplicate nodes, single-element multi-geometries, nested GeometryCollections, null or bare geometries, legacy crs members, antimeridian jumps, excess coordinate precision). Returns a VALID/INVALID verdict, the numbered issues, and a summary counting features, geometry types, positions, rings and the coordinate bounds — as text (default) or as JSON for scripting. Invalid JSON is reported as a finding, so broken files are safe to paste. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "geojson-validate", |a: Args| {
            gizza_ai_geojson_validate_core::run(
                &a.input,
                &a.output,
                a.strict_winding,
                a.allow_bbox,
                a.warn_problematic,
                a.max_precision,
                a.max_issues,
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

    /// Drift guard: the chat/CLI schema must stay exactly what the page,
    /// manifest and docs were written against. Regenerate deliberately.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(AUTHORED).unwrap();
        let live: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(live, authored);
    }

    const AUTHORED: &str = r#"
        {
            "additionalProperties": false,
            "properties": {
                "allow_bbox": {
                    "default": true,
                    "description": "Accept bbox members, checking their length (4 or 6 values), numeric type, ranges and [west, south, east, north] order. Set false to also flag every bbox member itself, for consumers that reject them.",
                    "type": "boolean"
                },
                "input": {
                    "description": "A single GeoJSON document to check: a FeatureCollection, Feature, or bare geometry object. Paste the raw text — invalid JSON is reported as a finding rather than an error, so broken files are fine to submit.",
                    "type": "string"
                },
                "max_issues": {
                    "default": 50,
                    "description": "How many issues to LIST per severity before truncating with an 'and N more' line. Counts in the verdict always stay complete. Default 50; raise it to see every issue in a badly broken file.",
                    "maximum": 1000,
                    "minimum": 1,
                    "type": "integer"
                },
                "max_precision": {
                    "default": 6,
                    "description": "Warn when a coordinate carries more decimal places than this. 6 is roughly 0.1 m at the equator and the usual recommendation (default); -1 turns the precision check off. Needs warn_problematic on.",
                    "maximum": 15,
                    "minimum": -1,
                    "type": "integer"
                },
                "output": {
                    "default": "text",
                    "description": "Report format. text is a readable verdict plus numbered issues and a summary (default); json returns {valid, error_count, warning_count, errors[], warnings[], summary} with a stable kebab-case rule id and JSON path per issue, for scripts and CI.",
                    "enum": [
                        "text",
                        "json"
                    ],
                    "type": "string"
                },
                "strict_winding": {
                    "default": true,
                    "description": "Treat RFC 7946 right-hand-rule violations (exterior rings must wind counterclockwise, holes clockwise) as errors that make the document invalid. Set false to report them as warnings instead — the RFC says SHOULD, and older GeoJSON commonly winds the other way.",
                    "type": "boolean"
                },
                "warn_problematic": {
                    "default": true,
                    "description": "Report the 'valid but problematic' family as warnings: duplicate consecutive nodes, zero-area rings, zero-length lines, single-element MultiPoint/MultiLineString/MultiPolygon and GeometryCollections, nested GeometryCollections, null geometries, bare geometries, legacy crs members, antimeridian crossings and excess coordinate precision. Set false for a spec-only check.",
                    "type": "boolean"
                }
            },
            "required": [
                "input"
            ],
            "type": "object"
        }
    "#;
}
