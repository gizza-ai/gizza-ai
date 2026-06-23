//! gizza-ai/function-grapher — plot one or more y = f(x) functions over an
//! x-range as an SVG graph (axes, gridlines, labels, a colored curve per
//! function, and a legend).
//!
//! Pure-Rust (core depends only on `meval`), so it runs on ALL backends
//! including the chat Service Worker. The SVG is wrapped as an `image/svg+xml`
//! data-URL envelope (like blocks/line-series-chart) so the chat UI shows the
//! graph. Surfaces: chat + CLI (no standalone page — a pure-Rust image-bytes
//! output has no page render mode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_function_grapher_core::render_svg;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    functions: String,
    #[serde(default = "default_xmin")]
    xmin: f64,
    #[serde(default = "default_xmax")]
    xmax: f64,
    #[serde(default = "default_samples")]
    samples: u32,
    #[serde(default = "default_nan")]
    ymin: f64,
    #[serde(default = "default_nan")]
    ymax: f64,
    #[serde(default)]
    title: String,
    #[serde(default = "default_w")]
    width: u32,
    #[serde(default = "default_h")]
    height: u32,
}
fn default_xmin() -> f64 { -10.0 }
fn default_xmax() -> f64 { 10.0 }
fn default_samples() -> u32 { 400 }
fn default_nan() -> f64 { f64::NAN }
fn default_w() -> u32 { 800 }
fn default_h() -> u32 { 500 }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("functions").required().describe("One or more y=f(x) expressions to plot. Separate multiple functions by newlines or ';'. Each may carry an optional 'name: ' or 'name=' prefix for the legend (a 'y=' prefix is treated as unlabeled). Supports + - * / ^, parentheses, functions (sin, cos, tan, sqrt, abs, ln, log10, exp, …) and constants (pi, e). E.g. 'sin(x); parabola: x^2'."))
        .param(Param::number("xmin").default(-10.0).describe("Left edge of the x domain (default -10)."))
        .param(Param::number("xmax").default(10.0).describe("Right edge of the x domain (default 10). Must be greater than xmin."))
        .param(Param::integer("samples").default(400).min(2.0).max(4000.0).describe("Number of points sampled per curve (default 400; clamped to 2..4000)."))
        .param(Param::number("ymin").describe("Optional fixed lower edge of the y-axis. Omit (with ymax) to autoscale the y-axis to the data."))
        .param(Param::number("ymax").describe("Optional fixed upper edge of the y-axis. Must be greater than ymin. Set both ymin and ymax to crop the view; omit both to autoscale."))
        .param(Param::string("title").default("").describe("Optional graph title."))
        .param(Param::integer("width").default(800).min(200.0).describe("SVG width in pixels (default 800)."))
        .param(Param::integer("height").default(500).min(150.0).describe("SVG height in pixels (default 500)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FunctionGrapher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/function-grapher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Plot y=f(x) functions over a range as an SVG graph",
    skill(
        description = "Plot one or more mathematical functions y=f(x) over an x-range and return an SVG graph with axes, gridlines, value labels, a colored curve per function, and a legend. Separate multiple functions by newlines or ';' and optionally name each with a 'name:' prefix. The y-axis autoscales to the data by default, or set both ymin and ymax to fix the y-window (the curve is cropped to it); x=0 and y=0 axes are emphasized when in range; discontinuities (e.g. 1/x) break the curve. Supports + - * / ^, parentheses, the usual functions (sin, cos, sqrt, abs, ln, exp, …) and constants (pi, e). Optional title, xmin/xmax, samples, ymin/ymax, width, height.",
        parameters = schema_json()
    )
)]
impl FunctionGrapher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("function-grapher")?;
    let svg = render_svg(
        &args.functions,
        args.xmin,
        args.xmax,
        args.samples,
        args.ymin,
        args.ymax,
        &args.title,
        args.width,
        args.height,
    )
    .map_err(SkillError::InvalidArgs)?;
    let name = if args.title.is_empty() { "graph".to_string() } else { args.title.clone() };
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        format!("{}.svg", name.replace(['/', '\\', ' '], "-")),
        format!("plotted a function graph ({} bytes SVG)", svg.len()),
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
                    "functions": { "type": "string", "description": "One or more y=f(x) expressions to plot. Separate multiple functions by newlines or ';'. Each may carry an optional 'name: ' or 'name=' prefix for the legend (a 'y=' prefix is treated as unlabeled). Supports + - * / ^, parentheses, functions (sin, cos, tan, sqrt, abs, ln, log10, exp, …) and constants (pi, e). E.g. 'sin(x); parabola: x^2'." },
                    "xmin":      { "type": "number", "default": -10.0, "description": "Left edge of the x domain (default -10)." },
                    "xmax":      { "type": "number", "default": 10.0, "description": "Right edge of the x domain (default 10). Must be greater than xmin." },
                    "samples":   { "type": "integer", "minimum": 2, "maximum": 4000, "default": 400, "description": "Number of points sampled per curve (default 400; clamped to 2..4000)." },
                    "ymin":      { "type": "number", "description": "Optional fixed lower edge of the y-axis. Omit (with ymax) to autoscale the y-axis to the data." },
                    "ymax":      { "type": "number", "description": "Optional fixed upper edge of the y-axis. Must be greater than ymin. Set both ymin and ymax to crop the view; omit both to autoscale." },
                    "title":     { "type": "string", "default": "", "description": "Optional graph title." },
                    "width":     { "type": "integer", "minimum": 200, "default": 800, "description": "SVG width in pixels (default 800)." },
                    "height":    { "type": "integer", "minimum": 150, "default": 500, "description": "SVG height in pixels (default 500)." }
                },
                "required": ["functions"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
