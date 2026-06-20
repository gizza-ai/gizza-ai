//! gizza-ai/line-series-chart — render numeric series as an SVG line chart.
//!
//! Pure-Rust (no deps in core), so it runs on ALL backends including the chat
//! Service Worker. The SVG is wrapped as an `image/svg+xml` data-URL envelope
//! (like blocks/vectorize) so the chat UI shows the chart. Surfaces: chat + CLI
//! (no standalone page — a pure-Rust image-bytes output has no page mode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_line_series_chart_core::render_svg;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    data: String,
    #[serde(default)]
    title: String,
    #[serde(default = "default_w")]
    width: u32,
    #[serde(default = "default_h")]
    height: u32,
}
fn default_w() -> u32 { 800 }
fn default_h() -> u32 { 400 }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("Numeric series to plot. Separate series by newlines or ';'; within a series separate values by commas/spaces (x is the value's index). E.g. '1,2,3,2,5' or '1,2,3\\n3,2,1'."))
        .param(Param::string("title").default("").describe("Optional chart title."))
        .param(Param::integer("width").default(800).min(200.0).describe("SVG width in pixels (default 800)."))
        .param(Param::integer("height").default(400).min(150.0).describe("SVG height in pixels (default 400)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LineSeriesChart;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/line-series-chart",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render numeric series as an SVG line chart",
    skill(
        description = "Render one or more numeric series as an SVG line/time-series chart (axes, y min/mid/max labels, a colored line per series, and a legend for multiple series). Separate series by newlines or ';'; within a series separate values by commas/spaces. Optional title, width, height.",
        parameters = schema_json()
    )
)]
impl LineSeriesChart {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("line-series-chart")?;
    let svg = render_svg(&args.data, &args.title, args.width, args.height)
        .map_err(SkillError::InvalidArgs)?;
    let title = if args.title.is_empty() { "chart".to_string() } else { args.title.clone() };
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        format!("{}.svg", title.replace(['/', '\\', ' '], "-")),
        format!("rendered a line chart ({} bytes SVG)", svg.len()),
        MAX_OUTPUT_BYTES,
    )
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
                    "data":   { "type": "string", "description": "Numeric series to plot. Separate series by newlines or ';'; within a series separate values by commas/spaces (x is the value's index). E.g. '1,2,3,2,5' or '1,2,3\\n3,2,1'." },
                    "title":  { "type": "string", "default": "", "description": "Optional chart title." },
                    "width":  { "type": "integer", "minimum": 200, "default": 800, "description": "SVG width in pixels (default 800)." },
                    "height": { "type": "integer", "minimum": 150, "default": 400, "description": "SVG height in pixels (default 400)." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
