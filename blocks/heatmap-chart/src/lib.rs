//! gizza-ai/heatmap-chart — render a numeric grid as a color-coded SVG heatmap.
//! Pure-Rust (no deps in core), runs on all backends incl. the chat SW. The SVG
//! is wrapped as image/svg+xml via build_media_envelope (like correlation-heatmap
//! / line-series-chart). Surfaces: chat + CLI (no page mode for image-bytes out).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_heatmap_chart_core::render_svg;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    data: String,
    #[serde(default = "default_true")]
    show_values: bool,
    #[serde(default)]
    col_labels: String,
    #[serde(default)]
    row_labels: String,
    #[serde(default)]
    title: String,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The grid: rows of comma/space-separated numbers (one row per line). All rows must have the same number of columns."))
        .param(Param::boolean("show_values").default(true).describe("Print each cell's numeric value inside the cell. Default true."))
        .param(Param::string("col_labels").default("").describe("Optional comma-separated column headings (defaults to c1..cN)."))
        .param(Param::string("row_labels").default("").describe("Optional comma-separated row headings (defaults to r1..rN)."))
        .param(Param::string("title").default("").describe("Optional chart title."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HeatmapChart;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/heatmap-chart",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a numeric grid as an SVG heatmap",
    skill(
        description = "Render a numeric matrix/grid as a color-coded SVG heatmap with a sequential blue→yellow→red colormap, an optional value in each cell, row/column labels, and a min→max color legend. `data` is rows of comma/space-separated numbers (one row per line, all rows equal width). Set show_values=false to hide cell numbers; col_labels/row_labels optionally name the axes.",
        parameters = schema_json()
    )
)]
impl HeatmapChart {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("heatmap-chart")?;
    let svg = render_svg(
        &args.data,
        args.show_values,
        &args.col_labels,
        &args.row_labels,
        &args.title,
    )
    .map_err(SkillError::InvalidArgs)?;
    let name = if args.title.is_empty() {
        "heatmap".to_string()
    } else {
        args.title.replace(['/', '\\', ' '], "-")
    };
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        format!("{name}.svg"),
        format!("rendered a heatmap ({} bytes SVG)", svg.len()),
        MAX_OUTPUT_BYTES,
    )
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
                    "data":        { "type": "string", "description": "The grid: rows of comma/space-separated numbers (one row per line). All rows must have the same number of columns." },
                    "show_values": { "type": "boolean", "default": true, "description": "Print each cell's numeric value inside the cell. Default true." },
                    "col_labels":  { "type": "string", "default": "", "description": "Optional comma-separated column headings (defaults to c1..cN)." },
                    "row_labels":  { "type": "string", "default": "", "description": "Optional comma-separated row headings (defaults to r1..rN)." },
                    "title":       { "type": "string", "default": "", "description": "Optional chart title." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
