//! gizza-ai/geojson-wkt — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_from")]
    from: String,
    #[serde(default = "default_to")]
    to: String,
    #[serde(default = "default_multi")]
    multi: String,
    #[serde(default)]
    srid: i64,
    #[serde(default = "default_precision")]
    precision: i64,
    #[serde(default = "default_wkb_encoding")]
    wkb_encoding: String,
    #[serde(default = "default_wkb_endian")]
    wkb_endian: String,
    #[serde(default = "default_true")]
    pretty: bool,
}

fn default_from() -> String {
    "auto".into()
}
fn default_to() -> String {
    "wkt".into()
}
fn default_multi() -> String {
    "collection".into()
}
fn default_precision() -> i64 {
    -1
}
fn default_wkb_encoding() -> String {
    "hex".into()
}
fn default_wkb_endian() -> String {
    "little".into()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("GeoJSON, WKT/EWKT, or WKB/EWKB text to convert. GeoJSON may be a Geometry, Feature, FeatureCollection, GeometryCollection, or an array of those; WKB may be hex or base64, one geometry per line."))
        .param(Param::enumv("from", ["auto", "geojson", "wkt", "wkb"]).default("auto").describe("Input format. Leave as auto to detect GeoJSON from JSON syntax, WKT/EWKT from geometry keywords or SRID=, and WKB/EWKB from hex/base64 bytes."))
        .param(Param::enumv("to", ["wkt", "geojson", "wkb"]).default("wkt").describe("Output format. WKT emits WKT or EWKT when an SRID is present; geojson emits GeoJSON geometry JSON; wkb emits WKB/EWKB as hex or base64 text."))
        .param(Param::enumv("multi", ["collection", "lines"]).default("collection").describe("How to emit several input geometries. collection wraps them into one GeometryCollection; lines writes one converted geometry per output line."))
        .param(Param::integer("srid").default(0).min(0.0).max(999999.0).describe("SRID metadata to write to EWKT/EWKB output. Use 0 to omit a new SRID and preserve any SRID already present in EWKT/EWKB input. Coordinates are not reprojected."))
        .param(Param::integer("precision").default(-1).min(-1.0).max(15.0).describe("Decimal places to round coordinates to before output. Use -1 to keep Rust's shortest round-trip float formatting."))
        .param(Param::enumv("wkb_encoding", ["hex", "base64"]).default("hex").describe("Text encoding for WKB/EWKB output, and the preferred interpretation for non-hex WKB input."))
        .param(Param::enumv("wkb_endian", ["little", "big"]).default("little").describe("Byte order for WKB/EWKB output: little is NDR/PostGIS-style, big is XDR."))
        .param(Param::boolean("pretty").default(true).describe("Pretty-print GeoJSON output with indentation. Disable for compact one-line JSON; WKT and WKB output are unaffected."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, String> {
    gizza_ai_geojson_wkt_core::convert(
        &a.input,
        &a.from,
        &a.to,
        &a.multi,
        a.srid,
        a.precision,
        &a.wkb_encoding,
        &a.wkb_endian,
        a.pretty,
    )
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/geojson-wkt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert geometries between GeoJSON, WKT/EWKT, and WKB/EWKB.",
    skill(
        description = "Convert geometry text between GeoJSON, WKT/EWKT, and WKB/EWKB. Accepts GeoJSON Geometry, Feature, FeatureCollection, GeometryCollection or arrays; WKT and EWKT with SRID= prefixes; and WKB/EWKB as hex or base64 text. Supports all seven OGC simple-feature geometry types, 2D/Z/M/ZM coordinates, EMPTY geometries, multi-feature collection-vs-lines output, optional coordinate rounding, WKB byte order, and SRID metadata. Coordinates are never reprojected.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "geojson-wkt", |a: Args| {
            run_args(a).map_err(SkillError::InvalidArgs)
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
                "type":"object",
                "properties":{
                    "input":{"type":"string","description":"GeoJSON, WKT/EWKT, or WKB/EWKB text to convert. GeoJSON may be a Geometry, Feature, FeatureCollection, GeometryCollection, or an array of those; WKB may be hex or base64, one geometry per line."},
                    "from":{"type":"string","enum":["auto","geojson","wkt","wkb"],"default":"auto","description":"Input format. Leave as auto to detect GeoJSON from JSON syntax, WKT/EWKT from geometry keywords or SRID=, and WKB/EWKB from hex/base64 bytes."},
                    "to":{"type":"string","enum":["wkt","geojson","wkb"],"default":"wkt","description":"Output format. WKT emits WKT or EWKT when an SRID is present; geojson emits GeoJSON geometry JSON; wkb emits WKB/EWKB as hex or base64 text."},
                    "multi":{"type":"string","enum":["collection","lines"],"default":"collection","description":"How to emit several input geometries. collection wraps them into one GeometryCollection; lines writes one converted geometry per output line."},
                    "srid":{"type":"integer","minimum":0,"maximum":999999,"default":0,"description":"SRID metadata to write to EWKT/EWKB output. Use 0 to omit a new SRID and preserve any SRID already present in EWKT/EWKB input. Coordinates are not reprojected."},
                    "precision":{"type":"integer","minimum":-1,"maximum":15,"default":-1,"description":"Decimal places to round coordinates to before output. Use -1 to keep Rust's shortest round-trip float formatting."},
                    "wkb_encoding":{"type":"string","enum":["hex","base64"],"default":"hex","description":"Text encoding for WKB/EWKB output, and the preferred interpretation for non-hex WKB input."},
                    "wkb_endian":{"type":"string","enum":["little","big"],"default":"little","description":"Byte order for WKB/EWKB output: little is NDR/PostGIS-style, big is XDR."},
                    "pretty":{"type":"boolean","default":true,"description":"Pretty-print GeoJSON output with indentation. Disable for compact one-line JSON; WKT and WKB output are unaffected."}
                },
                "required":["input"],
                "additionalProperties":false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
