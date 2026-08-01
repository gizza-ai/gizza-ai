//! gizza-ai/geojson-merge — combine multiple GeoJSON inputs into one
//! FeatureCollection. Chat schema single-sourced from descriptor() (which also
//! drives the CLI + page); handle() delegates to block_utils::run_skill. Pure →
//! all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_geojson_merge_core::merge_geojson;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    geojson: String,
    #[serde(default = "default_indent")]
    indent: i64,
    #[serde(default = "default_precision")]
    precision: i64,
    #[serde(default)]
    renumber: bool,
    #[serde(default)]
    source_property: String,
}

fn default_indent() -> i64 {
    2
}

fn default_precision() -> i64 {
    -1
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("geojson").required().describe(
            "One or more GeoJSON values to combine — FeatureCollections, bare Features, and raw geometries, concatenated (whitespace/newline separated, so line-delimited GeoJSON works too), e.g. '{\"type\":\"Point\",\"coordinates\":[0,0]}\\n{\"type\":\"Point\",\"coordinates\":[1,1]}'. All features are flattened, in order, into one FeatureCollection.",
        ))
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Output indentation in spaces (1-8). Use 0 to minify. Default 2."),
        )
        .param(
            Param::integer("precision")
                .min(-1.0)
                .max(15.0)
                .default(-1)
                .describe("Round every coordinate to this many decimal places to shrink the output (0-15). Use -1 (default) to keep full precision."),
        )
        .param(
            Param::boolean("renumber")
                .default(false)
                .describe("Assign each output feature a fresh sequential integer id (0, 1, 2, …), overwriting existing ids. Fixes id collisions when merging sources. Default false."),
        )
        .param(
            Param::string("source_property")
                .default("")
                .describe("If set to a property name, tag every feature with that property set to its 1-based source-document number, so merged features stay traceable to their origin. Blank (default) adds nothing."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GeojsonMerge;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/geojson-merge",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Combine multiple GeoJSON files or features into one FeatureCollection",
    skill(
        description = "Combine multiple GeoJSON inputs into a single FeatureCollection. Inputs are provided concatenated (whitespace/newline separated, so line-delimited GeoJSON works): FeatureCollections contribute their features, bare Features pass through, and raw geometries are wrapped into Features — all flattened in input order. Options: indent (0 minifies), precision (round coordinates to N decimals; -1 keeps full precision), renumber (assign sequential integer ids), and source_property (tag each feature with its 1-based source-document number). Returns the merged GeoJSON. Runs locally.",
        parameters = schema_json()
    ),
)]
impl GeojsonMerge {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "geojson-merge", |a: Args| {
            merge_geojson(
                &a.geojson,
                a.indent.max(0) as usize,
                a.precision,
                a.renumber,
                &a.source_property,
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
            r#"{
                "type": "object",
                "properties": {
                    "geojson":         { "type": "string", "description": "One or more GeoJSON values to combine — FeatureCollections, bare Features, and raw geometries, concatenated (whitespace/newline separated, so line-delimited GeoJSON works too), e.g. '{\"type\":\"Point\",\"coordinates\":[0,0]}\\n{\"type\":\"Point\",\"coordinates\":[1,1]}'. All features are flattened, in order, into one FeatureCollection." },
                    "indent":          { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Output indentation in spaces (1-8). Use 0 to minify. Default 2." },
                    "precision":       { "type": "integer", "minimum": -1, "maximum": 15, "default": -1, "description": "Round every coordinate to this many decimal places to shrink the output (0-15). Use -1 (default) to keep full precision." },
                    "renumber":        { "type": "boolean", "default": false, "description": "Assign each output feature a fresh sequential integer id (0, 1, 2, …), overwriting existing ids. Fixes id collisions when merging sources. Default false." },
                    "source_property": { "type": "string", "default": "", "description": "If set to a property name, tag every feature with that property set to its 1-based source-document number, so merged features stay traceable to their origin. Blank (default) adds nothing." }
                },
                "required": ["geojson"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
