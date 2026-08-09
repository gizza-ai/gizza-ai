//! gizza-ai/boxplot-chart — render box-and-whisker charts as deterministic SVG.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_boxplot_chart_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_layout")]
    layout: String,
    #[serde(default)]
    group_column: String,
    #[serde(default)]
    value_column: String,
    #[serde(default = "default_quartile_method")]
    quartile_method: String,
    #[serde(default = "default_whiskers")]
    whiskers: String,
    #[serde(default = "default_iqr_multiplier")]
    iqr_multiplier: f64,
    #[serde(default = "default_percentile")]
    percentile: f64,
    #[serde(default = "default_points")]
    points: String,
    #[serde(default = "default_true")]
    show_mean: bool,
    #[serde(default)]
    notched: bool,
    #[serde(default = "default_orientation")]
    orientation: String,
    #[serde(default = "default_true")]
    grid: bool,
    #[serde(default)]
    title: String,
    #[serde(default)]
    value_label: String,
    #[serde(default)]
    group_label: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_layout() -> String { "auto".into() }
fn default_quartile_method() -> String { "linear".into() }
fn default_whiskers() -> String { "tukey".into() }
fn default_iqr_multiplier() -> f64 { 1.5 }
fn default_percentile() -> f64 { 5.0 }
fn default_points() -> String { "outliers".into() }
fn default_true() -> bool { true }
fn default_orientation() -> String { "vertical".into() }
fn default_width() -> u32 { 800 }
fn default_height() -> u32 { 480 }
fn default_color() -> String { "#2563eb".into() }
fn default_theme() -> String { "light".into() }
fn default_output() -> String { "svg".into() }

fn to_options(a: &Args) -> Options {
    Options {
        layout: a.layout.clone(),
        group_column: a.group_column.clone(),
        value_column: a.value_column.clone(),
        quartile_method: a.quartile_method.clone(),
        whiskers: a.whiskers.clone(),
        iqr_multiplier: a.iqr_multiplier,
        percentile: a.percentile,
        points: a.points.clone(),
        show_mean: a.show_mean,
        notched: a.notched,
        orientation: a.orientation.clone(),
        grid: a.grid,
        title: a.title.clone(),
        value_label: a.value_label.clone(),
        group_label: a.group_label.clone(),
        width: a.width,
        height: a.height,
        color: a.color.clone(),
        theme: a.theme.clone(),
        output: a.output.clone(),
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("Values to plot. Paste a one-column list, a tidy table such as `group,value`, or a wide table with one numeric column per group. CSV, tab, semicolon, and whitespace-separated values are accepted. Required."))
        .param(Param::enumv("layout", ["auto", "values", "long", "wide"]).default("auto").describe("Input layout: auto (default) detects a one-column list, group/value table, or wide columns; values forces one series; long uses group_column/value_column; wide treats each numeric column as a separate box."))
        .param(Param::string("group_column").default("").describe("For layout=long, the group/category column, by header name or 1-based index. Default empty uses the first non-value-looking column."))
        .param(Param::string("value_column").default("").describe("For layout=long, the numeric value column, by header name or 1-based index. Default empty uses the first numeric-looking column after the group column."))
        .param(Param::enumv("quartile_method", ["linear", "inclusive", "exclusive"]).default("linear").describe("Quartile calculation: linear (default, interpolated percentile), inclusive (median included in each half), or exclusive (median excluded from halves)."))
        .param(Param::enumv("whiskers", ["tukey", "minmax", "percentile"]).default("tukey").describe("Whisker rule: tukey (default, last point inside Q1/Q3 ± k*IQR), minmax (full data range), or percentile (lower percentile and 100-percentile)."))
        .param(Param::number("iqr_multiplier").min(0.0).max(10.0).default(1.5).describe("Tukey fence multiplier k for whiskers=tukey. Default 1.5; larger values mark fewer outliers."))
        .param(Param::number("percentile").min(0.0).max(49.0).default(5.0).describe("Lower percentile for whiskers=percentile; upper whisker uses 100 minus this value. Default 5."))
        .param(Param::enumv("points", ["outliers", "all", "none"]).default("outliers").describe("Point markers to draw: outliers (default), all observations, or none."))
        .param(Param::boolean("show_mean").default(true).describe("Draw a mean marker on each box. Default true."))
        .param(Param::boolean("notched").default(false).describe("Draw notched boxes using an approximate median confidence notch. Default false."))
        .param(Param::enumv("orientation", ["vertical", "horizontal"]).default("vertical").describe("Chart orientation: vertical (default) or horizontal."))
        .param(Param::boolean("grid").default(true).describe("Draw background value grid lines. Default true."))
        .param(Param::string("title").default("").describe("Optional chart title shown above the plot."))
        .param(Param::string("value_label").default("").describe("Optional value-axis label, such as `Latency (ms)`."))
        .param(Param::string("group_label").default("").describe("Optional group-axis label, such as `Region`."))
        .param(Param::integer("width").min(320.0).max(2400.0).default(800).describe("SVG width in pixels, 320-2400. Default 800."))
        .param(Param::integer("height").min(240.0).max(1800.0).default(480).describe("SVG height in pixels, 240-1800. Default 480."))
        .param(Param::string("color").default("#2563eb").describe("Primary box colour as CSS color text, for example #2563eb or tomato. Default #2563eb."))
        .param(Param::enumv("theme", ["light", "dark"]).default("light").describe("Chart theme: light (default) or dark."))
        .param(Param::enumv("output", ["svg", "summary", "json"]).default("svg").describe("Output format: svg (default), summary (five-number table), or json (machine-readable group stats)."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct BoxplotChart;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/boxplot-chart",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render grouped box-and-whisker charts from pasted values",
    skill(
        description = "Render a deterministic box-and-whisker chart from pasted numeric data. Accepts a one-column list, a tidy group/value table, or a wide table with one numeric column per group. Computes min, Q1, median, Q3, max, IQR, mean, standard deviation, Tukey/percentile/min-max whiskers, and outliers, then returns SVG by default or a summary/JSON stats report. Options control layout, group/value columns, quartile method, whisker rule, IQR multiplier, percentile whiskers, outlier/all/no point markers, mean marker, notches, orientation, grid, title, axis labels, SVG size, colour, theme, and output format. Everything runs locally with pure Rust; no plotting service or external data is used.",
        parameters = schema_json()
    ),
)]
impl BoxplotChart {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "boxplot-chart", |a: Args| {
            let opts = to_options(&a);
            gizza_ai_boxplot_chart_core::render(&a.data, &opts).map_err(SkillError::InvalidArgs)
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
            r##"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "Values to plot. Paste a one-column list, a tidy table such as `group,value`, or a wide table with one numeric column per group. CSV, tab, semicolon, and whitespace-separated values are accepted. Required." },
                    "layout": { "type": "string", "enum": ["auto", "values", "long", "wide"], "default": "auto", "description": "Input layout: auto (default) detects a one-column list, group/value table, or wide columns; values forces one series; long uses group_column/value_column; wide treats each numeric column as a separate box." },
                    "group_column": { "type": "string", "default": "", "description": "For layout=long, the group/category column, by header name or 1-based index. Default empty uses the first non-value-looking column." },
                    "value_column": { "type": "string", "default": "", "description": "For layout=long, the numeric value column, by header name or 1-based index. Default empty uses the first numeric-looking column after the group column." },
                    "quartile_method": { "type": "string", "enum": ["linear", "inclusive", "exclusive"], "default": "linear", "description": "Quartile calculation: linear (default, interpolated percentile), inclusive (median included in each half), or exclusive (median excluded from halves)." },
                    "whiskers": { "type": "string", "enum": ["tukey", "minmax", "percentile"], "default": "tukey", "description": "Whisker rule: tukey (default, last point inside Q1/Q3 \u00b1 k*IQR), minmax (full data range), or percentile (lower percentile and 100-percentile)." },
                    "iqr_multiplier": { "type": "number", "default": 1.5, "minimum": 0, "maximum": 10, "description": "Tukey fence multiplier k for whiskers=tukey. Default 1.5; larger values mark fewer outliers." },
                    "percentile": { "type": "number", "default": 5.0, "minimum": 0, "maximum": 49, "description": "Lower percentile for whiskers=percentile; upper whisker uses 100 minus this value. Default 5." },
                    "points": { "type": "string", "enum": ["outliers", "all", "none"], "default": "outliers", "description": "Point markers to draw: outliers (default), all observations, or none." },
                    "show_mean": { "type": "boolean", "default": true, "description": "Draw a mean marker on each box. Default true." },
                    "notched": { "type": "boolean", "default": false, "description": "Draw notched boxes using an approximate median confidence notch. Default false." },
                    "orientation": { "type": "string", "enum": ["vertical", "horizontal"], "default": "vertical", "description": "Chart orientation: vertical (default) or horizontal." },
                    "grid": { "type": "boolean", "default": true, "description": "Draw background value grid lines. Default true." },
                    "title": { "type": "string", "default": "", "description": "Optional chart title shown above the plot." },
                    "value_label": { "type": "string", "default": "", "description": "Optional value-axis label, such as `Latency (ms)`." },
                    "group_label": { "type": "string", "default": "", "description": "Optional group-axis label, such as `Region`." },
                    "width": { "type": "integer", "default": 800, "minimum": 320, "maximum": 2400, "description": "SVG width in pixels, 320-2400. Default 800." },
                    "height": { "type": "integer", "default": 480, "minimum": 240, "maximum": 1800, "description": "SVG height in pixels, 240-1800. Default 480." },
                    "color": { "type": "string", "default": "#2563eb", "description": "Primary box colour as CSS color text, for example #2563eb or tomato. Default #2563eb." },
                    "theme": { "type": "string", "enum": ["light", "dark"], "default": "light", "description": "Chart theme: light (default) or dark." },
                    "output": { "type": "string", "enum": ["svg", "summary", "json"], "default": "svg", "description": "Output format: svg (default), summary (five-number table), or json (machine-readable group stats)." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn every_param_is_described() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().expect("object schema");
        assert_eq!(props.len(), 21, "parameter count changed");
        for (name, spec) in props {
            let desc = spec["description"].as_str().unwrap_or("");
            assert!(desc.len() > 20, "param {name} needs a real description");
        }
    }

    #[test]
    fn args_defaults_match_the_descriptor() {
        let a: Args = serde_json::from_str(r#"{"data":"1,2,3"}"#).unwrap();
        let o = to_options(&a);
        assert_eq!(o.layout, "auto");
        assert_eq!(o.quartile_method, "linear");
        assert_eq!(o.whiskers, "tukey");
        assert_eq!(o.iqr_multiplier, 1.5);
        assert_eq!(o.percentile, 5.0);
        assert_eq!(o.points, "outliers");
        assert!(o.show_mean);
        assert!(!o.notched);
        assert_eq!(o.orientation, "vertical");
        assert!(o.grid);
        assert_eq!(o.width, 800);
        assert_eq!(o.height, 480);
        assert_eq!(o.color, "#2563eb");
        assert_eq!(o.theme, "light");
        assert_eq!(o.output, "svg");
    }
}
