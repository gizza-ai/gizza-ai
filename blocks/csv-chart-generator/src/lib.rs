//! gizza-ai/csv-chart-generator — turn chosen columns of a CSV into a bar, line,
//! scatter, or histogram chart, rendered as a standalone SVG string. The chat
//! schema is single-sourced from `descriptor()` (which also drives the CLI);
//! `handle()` delegates to the pure `core::render`. Pure compute → runs on every
//! backend including the chat Service Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_chart_generator_core::{render, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_chart_type")]
    chart_type: String,
    #[serde(default)]
    x_column: String,
    #[serde(default)]
    y_column: String,
    #[serde(default)]
    title: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default = "default_bins")]
    bins: u32,
}

fn default_chart_type() -> String {
    "bar".into()
}
fn default_width() -> u32 {
    800
}
fn default_height() -> u32 {
    400
}
fn default_color() -> String {
    "#2563eb".into()
}
fn default_bins() -> u32 {
    10
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The CSV text to chart. Include a header row when you can (e.g. `month,revenue`); a first row that is not entirely numeric is treated as headers. Quoted fields with embedded commas are handled."),
        )
        .param(
            Param::enumv("chart_type", ["bar", "line", "scatter", "histogram"])
                .default("bar")
                .describe("Chart to draw: 'bar' (categorical X, numeric Y), 'line' (numeric or ordered X, numeric Y, points joined), 'scatter' (numeric X vs numeric Y), or 'histogram' (bin one numeric column into buckets). Default bar."),
        )
        .param(
            Param::string("x_column")
                .default("")
                .describe("Column for the X axis of bar/line/scatter charts: a header name (case-insensitive) or a 1-based column index. Not used for histograms."),
        )
        .param(
            Param::string("y_column")
                .required()
                .describe("The numeric value column (Y axis) for bar/line/scatter. For a histogram this is the single numeric column whose values are binned. A header name (case-insensitive) or a 1-based column index."),
        )
        .param(
            Param::string("title")
                .default("")
                .describe("Optional chart title drawn centered above the plot."),
        )
        .param(
            Param::integer("width")
                .min(200.0)
                .max(4000.0)
                .default(800)
                .describe("SVG width in pixels (200–4000). Default 800."),
        )
        .param(
            Param::integer("height")
                .min(150.0)
                .max(4000.0)
                .default(400)
                .describe("SVG height in pixels (150–4000). Default 400."),
        )
        .param(
            Param::string("color")
                .default("#2563eb")
                .describe("Series color for bars, the line, or the points — any CSS color (e.g. `#2563eb`, `tomato`, `rgb(20,120,200)`). Default #2563eb."),
        )
        .param(
            Param::integer("bins")
                .min(2.0)
                .max(100.0)
                .default(10)
                .describe("Number of buckets for a histogram (2–100). Ignored for other chart types. Default 10."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options(a: &Args) -> Options {
    Options {
        chart_type: a.chart_type.clone(),
        x_column: a.x_column.clone(),
        y_column: a.y_column.clone(),
        title: a.title.clone(),
        width: a.width,
        height: a.height,
        color: a.color.clone(),
        bins: a.bins,
    }
}

#[cfg(target_arch = "wasm32")]
struct CsvChartGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-chart-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn CSV columns into a bar, line, scatter, or histogram chart as an SVG",
    skill(
        description = "Chart columns of a CSV as a standalone SVG. Paste CSV text, name the columns to plot, and get a bar, line, scatter, or histogram chart. Bar charts take a categorical X column and a numeric Y column; line and scatter take numeric X and Y (line sorts by X and joins the points); a histogram bins one numeric column (given as y_column) into 2–100 buckets. Columns are chosen by header name (case-insensitive) or 1-based index. Options: title, width and height in pixels, series color (any CSS color), and bins for histograms. The output is a self-contained SVG string — no plotting library, nothing uploaded.",
        parameters = schema_json()
    ),
)]
impl CsvChartGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-chart-generator", |a: Args| {
            render(&a.data, &options(&a)).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// copy, so an accidental descriptor edit can't silently change the
    /// LLM-facing schema (and the page controls the manifest renders from it).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The CSV text to chart. Include a header row when you can (e.g. `month,revenue`); a first row that is not entirely numeric is treated as headers. Quoted fields with embedded commas are handled." },
                    "chart_type": { "type": "string", "enum": ["bar", "line", "scatter", "histogram"], "default": "bar", "description": "Chart to draw: 'bar' (categorical X, numeric Y), 'line' (numeric or ordered X, numeric Y, points joined), 'scatter' (numeric X vs numeric Y), or 'histogram' (bin one numeric column into buckets). Default bar." },
                    "x_column": { "type": "string", "default": "", "description": "Column for the X axis of bar/line/scatter charts: a header name (case-insensitive) or a 1-based column index. Not used for histograms." },
                    "y_column": { "type": "string", "description": "The numeric value column (Y axis) for bar/line/scatter. For a histogram this is the single numeric column whose values are binned. A header name (case-insensitive) or a 1-based column index." },
                    "title": { "type": "string", "default": "", "description": "Optional chart title drawn centered above the plot." },
                    "width": { "type": "integer", "minimum": 200, "maximum": 4000, "default": 800, "description": "SVG width in pixels (200–4000). Default 800." },
                    "height": { "type": "integer", "minimum": 150, "maximum": 4000, "default": 400, "description": "SVG height in pixels (150–4000). Default 400." },
                    "color": { "type": "string", "default": "#2563eb", "description": "Series color for bars, the line, or the points — any CSS color (e.g. `#2563eb`, `tomato`, `rgb(20,120,200)`). Default #2563eb." },
                    "bins": { "type": "integer", "minimum": 2, "maximum": 100, "default": 10, "description": "Number of buckets for a histogram (2–100). Ignored for other chart types. Default 10." }
                },
                "required": ["data", "y_column"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
