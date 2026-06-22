//! gizza-ai/candlestick-chart — render OHLC price data as an SVG candlestick chart.
//!
//! Pure-Rust (no deps in core), so it runs on ALL backends including the chat
//! Service Worker. The SVG is wrapped as an `image/svg+xml` data-URL envelope
//! (like blocks/line-series-chart) so the chat UI shows the chart. Surfaces:
//! chat + CLI (no standalone page — a pure-Rust image-bytes output has no page
//! render mode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_candlestick_chart_core::render_svg;
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
    #[serde(default)]
    trendline: bool,
}
fn default_w() -> u32 { 800 }
fn default_h() -> u32 { 400 }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("OHLC price data as CSV, one bar per line. Each row is 4 columns 'open,high,low,close' or 5 columns 'label,open,high,low,close' (the label/date is shown on the first and last candle). A leading non-numeric header row (e.g. 'date,open,high,low,close') is skipped. E.g. '2024-01-01,10,15,8,12\\n2024-01-02,12,16,11,15'."))
        .param(Param::string("title").default("").describe("Optional chart title."))
        .param(Param::integer("width").default(800).min(200.0).describe("SVG width in pixels (default 800)."))
        .param(Param::integer("height").default(400).min(150.0).describe("SVG height in pixels (default 400)."))
        .param(Param::boolean("trendline").default(false).describe("Overlay a dashed line connecting each bar's closing price (default false)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CandlestickChart;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/candlestick-chart",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render OHLC price data as an SVG candlestick chart",
    skill(
        description = "Render OHLC (open/high/low/close) price data as an SVG candlestick chart: a price axis with min/mid/max labels, one candle per row (a high–low wick plus an open–close body, green when close >= open and red when it falls), an optional title, and an optional dashed closing-price trendline. Input is CSV with 4 columns (open,high,low,close) or 5 columns (label/date,open,high,low,close); a header row is skipped automatically.",
        parameters = schema_json()
    )
)]
impl CandlestickChart {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("candlestick-chart")?;
    let svg = render_svg(&args.data, &args.title, args.width, args.height, args.trendline)
        .map_err(SkillError::InvalidArgs)?;
    let title = if args.title.is_empty() { "candlestick".to_string() } else { args.title.clone() };
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        format!("{}.svg", title.replace(['/', '\\', ' '], "-")),
        format!("rendered a candlestick chart ({} bytes SVG)", svg.len()),
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
                    "data":   { "type": "string", "description": "OHLC price data as CSV, one bar per line. Each row is 4 columns 'open,high,low,close' or 5 columns 'label,open,high,low,close' (the label/date is shown on the first and last candle). A leading non-numeric header row (e.g. 'date,open,high,low,close') is skipped. E.g. '2024-01-01,10,15,8,12\\n2024-01-02,12,16,11,15'." },
                    "title":  { "type": "string", "default": "", "description": "Optional chart title." },
                    "width":  { "type": "integer", "minimum": 200, "default": 800, "description": "SVG width in pixels (default 800)." },
                    "height": { "type": "integer", "minimum": 150, "default": 400, "description": "SVG height in pixels (default 400)." },
                    "trendline": { "type": "boolean", "default": false, "description": "Overlay a dashed line connecting each bar's closing price (default false)." }
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
