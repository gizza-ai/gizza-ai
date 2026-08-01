//! gizza-ai/pie-donut-chart-svg — turn labeled values into a standalone pie or
//! donut chart, rendered as a self-contained SVG string. The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to the pure `core::render`. Pure compute → runs on every backend
//! including the chat Service Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_pie_donut_chart_svg_core::{render, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_chart_type")]
    chart_type: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_donut_hole")]
    donut_hole: f64,
    #[serde(default)]
    start_angle: f64,
    #[serde(default)]
    colors: String,
    #[serde(default)]
    show_labels: bool,
    #[serde(default = "default_true")]
    show_percentages: bool,
    #[serde(default)]
    show_values: bool,
    #[serde(default = "default_legend")]
    legend: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default)]
    title: String,
    #[serde(default = "default_background")]
    background: String,
}

fn default_chart_type() -> String {
    "pie".into()
}
fn default_width() -> u32 {
    640
}
fn default_height() -> u32 {
    400
}
fn default_donut_hole() -> f64 {
    0.55
}
fn default_true() -> bool {
    true
}
fn default_legend() -> String {
    "right".into()
}
fn default_sort() -> String {
    "input".into()
}
fn default_background() -> String {
    "#ffffff".into()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The labeled values to chart, one `label, value` pair per line (also split on `;`). The separator can be `,`, `:` or `=` — e.g. `Chrome, 63` or `Mobile: 55`. A JSON array is also accepted (`[[\"A\",5],[\"B\",3]]` or `[{\"label\":\"A\",\"value\":5}]`). Values must be zero or positive numbers; negatives are rejected."),
        )
        .param(
            Param::enumv("chart_type", ["pie", "donut"])
                .default("pie")
                .describe("Chart to draw: 'pie' (full wedges) or 'donut' (a pie with a round hole in the middle). Default pie."),
        )
        .param(
            Param::integer("width")
                .min(120.0)
                .max(4000.0)
                .default(640)
                .describe("SVG width in pixels (120–4000). Default 640."),
        )
        .param(
            Param::integer("height")
                .min(120.0)
                .max(4000.0)
                .default(400)
                .describe("SVG height in pixels (120–4000). Default 400."),
        )
        .param(
            Param::number("donut_hole")
                .min(0.0)
                .max(0.9)
                .default(0.55)
                .describe("Donut hole size as a fraction of the outer radius (0.0–0.9). Only affects a donut chart; 0.55 is a typical ring. Default 0.55."),
        )
        .param(
            Param::number("start_angle")
                .min(-360.0)
                .max(360.0)
                .default(0.0)
                .describe("Angle in degrees for the first slice's leading edge, measured clockwise from 12 o'clock. 0 starts at the top. Default 0."),
        )
        .param(
            Param::string("colors")
                .default("")
                .describe("Optional comma-separated CSS colors cycled across slices (e.g. `#4e79a7, tomato, #59a14f`). Empty uses a built-in 10-color palette."),
        )
        .param(
            Param::boolean("show_labels")
                .default(false)
                .describe("Draw each slice's label text on the slice itself (in addition to the legend). Default false."),
        )
        .param(
            Param::boolean("show_percentages")
                .default(true)
                .describe("Draw each slice's percentage of the total on the slice, and include it in legend rows. Default true."),
        )
        .param(
            Param::boolean("show_values")
                .default(false)
                .describe("Include each slice's raw value in its legend row. Default false."),
        )
        .param(
            Param::enumv("legend", ["none", "right", "bottom"])
                .default("right")
                .describe("Where to place the legend listing each label with a color swatch: 'none', 'right' of the chart, or 'bottom'. Default right."),
        )
        .param(
            Param::enumv("sort", ["input", "descending", "ascending"])
                .default("input")
                .describe("Slice order: 'input' keeps the given order, 'descending'/'ascending' sort by value. Default input."),
        )
        .param(
            Param::string("title")
                .default("")
                .describe("Optional chart title drawn centered above the chart."),
        )
        .param(
            Param::string("background")
                .default("#ffffff")
                .describe("Background fill — any CSS color, or `none`/`transparent` for no backdrop. Default #ffffff."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options(a: &Args) -> Options {
    Options {
        chart_type: a.chart_type.clone(),
        width: a.width,
        height: a.height,
        donut_hole: a.donut_hole,
        start_angle: a.start_angle,
        colors: a.colors.clone(),
        show_labels: a.show_labels,
        show_percentages: a.show_percentages,
        show_values: a.show_values,
        legend: a.legend.clone(),
        sort: a.sort.clone(),
        title: a.title.clone(),
        background: a.background.clone(),
    }
}

#[cfg(target_arch = "wasm32")]
struct PieDonutChartSvg;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pie-donut-chart-svg",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn labeled values into a pie or donut chart as a standalone SVG",
    skill(
        description = "Generate a pie or donut chart as a self-contained SVG from labeled values. Enter one `label, value` pair per line (the separator can be `,`, `:` or `=`, and entries may also be `;`-separated), or paste a JSON array. Percentages are computed automatically from the sum. Options: chart_type (pie or donut), donut hole size, start angle, a custom color list, on-slice labels and percentages, raw values in the legend, legend placement (none/right/bottom), slice sort (input/descending/ascending), a title, canvas width and height, and a background color. Negative or non-numeric values are rejected with a clear error. The output is a plain SVG string — no plotting library, nothing uploaded.",
        parameters = schema_json()
    ),
)]
impl PieDonutChartSvg {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pie-donut-chart-svg", |a: Args| {
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
                    "data": { "type": "string", "description": "The labeled values to chart, one `label, value` pair per line (also split on `;`). The separator can be `,`, `:` or `=` — e.g. `Chrome, 63` or `Mobile: 55`. A JSON array is also accepted (`[[\"A\",5],[\"B\",3]]` or `[{\"label\":\"A\",\"value\":5}]`). Values must be zero or positive numbers; negatives are rejected." },
                    "chart_type": { "type": "string", "enum": ["pie", "donut"], "default": "pie", "description": "Chart to draw: 'pie' (full wedges) or 'donut' (a pie with a round hole in the middle). Default pie." },
                    "width": { "type": "integer", "minimum": 120, "maximum": 4000, "default": 640, "description": "SVG width in pixels (120–4000). Default 640." },
                    "height": { "type": "integer", "minimum": 120, "maximum": 4000, "default": 400, "description": "SVG height in pixels (120–4000). Default 400." },
                    "donut_hole": { "type": "number", "minimum": 0, "maximum": 0.9, "default": 0.55, "description": "Donut hole size as a fraction of the outer radius (0.0–0.9). Only affects a donut chart; 0.55 is a typical ring. Default 0.55." },
                    "start_angle": { "type": "number", "minimum": -360, "maximum": 360, "default": 0.0, "description": "Angle in degrees for the first slice's leading edge, measured clockwise from 12 o'clock. 0 starts at the top. Default 0." },
                    "colors": { "type": "string", "default": "", "description": "Optional comma-separated CSS colors cycled across slices (e.g. `#4e79a7, tomato, #59a14f`). Empty uses a built-in 10-color palette." },
                    "show_labels": { "type": "boolean", "default": false, "description": "Draw each slice's label text on the slice itself (in addition to the legend). Default false." },
                    "show_percentages": { "type": "boolean", "default": true, "description": "Draw each slice's percentage of the total on the slice, and include it in legend rows. Default true." },
                    "show_values": { "type": "boolean", "default": false, "description": "Include each slice's raw value in its legend row. Default false." },
                    "legend": { "type": "string", "enum": ["none", "right", "bottom"], "default": "right", "description": "Where to place the legend listing each label with a color swatch: 'none', 'right' of the chart, or 'bottom'. Default right." },
                    "sort": { "type": "string", "enum": ["input", "descending", "ascending"], "default": "input", "description": "Slice order: 'input' keeps the given order, 'descending'/'ascending' sort by value. Default input." },
                    "title": { "type": "string", "default": "", "description": "Optional chart title drawn centered above the chart." },
                    "background": { "type": "string", "default": "#ffffff", "description": "Background fill — any CSS color, or `none`/`transparent` for no backdrop. Default #ffffff." }
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
