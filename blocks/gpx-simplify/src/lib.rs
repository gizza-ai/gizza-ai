//! gizza-ai/gpx-simplify — simplify a GPX track with Douglas-Peucker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_tolerance")]
    tolerance_meters: f64,
    #[serde(default)]
    keep_every: u64,
    #[serde(default = "default_decimals")]
    decimals: u32,
    #[serde(default)]
    output: String,
}

fn default_tolerance() -> f64 {
    10.0
}
fn default_decimals() -> u32 {
    6
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The GPX document to simplify. The tool reads <trkpt lat=\"…\" lon=\"…\"> track points, preserves endpoint points plus any points needed to stay within the tolerance, and carries each kept point's <ele> and <time> children into a clean GPX 1.1 output."))
        .param(Param::number("tolerance_meters").default(10).min(0.0).max(1000.0).describe("Douglas-Peucker tolerance in meters. Higher values remove more intermediate points while allowing more shape deviation. 0 keeps every bend; 10 m is a practical default for hiking/running tracks."))
        .param(Param::integer("keep_every").default(0).min(0.0).describe("Optional safety sampling interval: keep every Nth source point in addition to the Douglas-Peucker result. Use 0 (default) to rely only on the tolerance."))
        .param(Param::integer("decimals").default(6).min(0.0).max(8.0).describe("Decimal places for output coordinates, 0–8. Six decimals is roughly 11 cm at the equator and is usually enough for GPX tracks."))
        .param(Param::enumv("output", ["gpx", "summary"]).default("gpx").describe("Return the simplified GPX document (default) or a short summary with original point count, kept point count, removal percentage, tolerance and approximate path lengths."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GpxSimplify;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gpx-simplify",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Simplify GPX track points with Douglas-Peucker while preserving route shape.",
    skill(
        description = "Reduce the point count of a GPX track using the Douglas-Peucker algorithm at a chosen tolerance in meters. Paste GPX XML; the tool reads <trkpt lat/lon> points, keeps endpoints and shape-critical bends, optionally keeps every Nth point, preserves kept points' elevation and time children, and returns a clean GPX 1.1 document or a summary. Use tolerance_meters to trade detail for size, decimals to control coordinate precision, and output='summary' to preview point reduction before exporting GPX.",
        parameters = schema_json()
    ),
)]
impl GpxSimplify {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gpx-simplify", |a: Args| {
            gizza_ai_gpx_simplify_core::simplify(
                &a.input,
                a.tolerance_meters,
                a.keep_every as usize,
                a.decimals,
                &a.output,
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
                "input":{"type":"string","description":"The GPX document to simplify. The tool reads <trkpt lat=\"…\" lon=\"…\"> track points, preserves endpoint points plus any points needed to stay within the tolerance, and carries each kept point's <ele> and <time> children into a clean GPX 1.1 output."},
                "tolerance_meters":{"type":"number","default":10,"minimum":0,"maximum":1000,"description":"Douglas-Peucker tolerance in meters. Higher values remove more intermediate points while allowing more shape deviation. 0 keeps every bend; 10 m is a practical default for hiking/running tracks."},
                "keep_every":{"type":"integer","default":0,"minimum":0,"description":"Optional safety sampling interval: keep every Nth source point in addition to the Douglas-Peucker result. Use 0 (default) to rely only on the tolerance."},
                "decimals":{"type":"integer","default":6,"minimum":0,"maximum":8,"description":"Decimal places for output coordinates, 0–8. Six decimals is roughly 11 cm at the equator and is usually enough for GPX tracks."},
                "output":{"type":"string","enum":["gpx","summary"],"default":"gpx","description":"Return the simplified GPX document (default) or a short summary with original point count, kept point count, removal percentage, tolerance and approximate path lengths."}
            },
            "required":["input"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }
}
