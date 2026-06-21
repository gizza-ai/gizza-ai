//! gizza-ai/scatter-chart — render x/y data as an SVG scatter plot.
//!
//! Pure-Rust (no deps in core), so it runs on ALL backends including the chat
//! Service Worker. The SVG is wrapped as an `image/svg+xml` data-URL envelope so
//! the chat UI shows the chart. Surfaces: chat + CLI (no standalone page — a
//! pure-Rust image-bytes output has no page mode, like line-series-chart).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_scatter_chart_core::render_svg;
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
fn default_w() -> u32 {
    700
}
fn default_h() -> u32 {
    500
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "Points as a JSON array. Each point is either [x, y] or an object {\"x\":..,\"y\":..,\"category\":\"..\",\"size\":..}. category colours the point (with a legend); size scales the marker. E.g. [[1,2],[3,4]] or [{\"x\":1,\"y\":2,\"category\":\"A\"}].",
        ))
        .param(Param::string("title").default("").describe("Optional chart title."))
        .param(Param::integer("width").default(700).min(200.0).describe("SVG width in pixels (default 700)."))
        .param(Param::integer("height").default(500).min(150.0).describe("SVG height in pixels (default 500)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ScatterChart;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/scatter-chart",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a scatter plot as an SVG chart",
    skill(
        description = "Render a scatter plot from x/y data as an SVG chart (axes with min/mid/max tick labels and gridlines, one circle per point). Points are a JSON array of [x, y] pairs or {x, y, category, size} objects: category colours points and adds a legend; size scales the marker radius. Optional title, width, height. Returns an SVG image.",
        parameters = schema_json()
    ),
)]
impl ScatterChart {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("scatter-chart")?;
    let svg = render_svg(&args.data, &args.title, args.width, args.height)
        .map_err(SkillError::InvalidArgs)?;
    let title = if args.title.is_empty() {
        "scatter".to_string()
    } else {
        args.title.clone()
    };
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        format!("{}.svg", title.replace(['/', '\\', ' '], "-")),
        format!("rendered a scatter plot ({} bytes SVG)", svg.len()),
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
                    "data":   { "type": "string", "description": "Points as a JSON array. Each point is either [x, y] or an object {\"x\":..,\"y\":..,\"category\":\"..\",\"size\":..}. category colours the point (with a legend); size scales the marker. E.g. [[1,2],[3,4]] or [{\"x\":1,\"y\":2,\"category\":\"A\"}]." },
                    "title":  { "type": "string", "default": "", "description": "Optional chart title." },
                    "width":  { "type": "integer", "default": 700, "minimum": 200, "description": "SVG width in pixels (default 700)." },
                    "height": { "type": "integer", "default": 500, "minimum": 150, "description": "SVG height in pixels (default 500)." }
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
