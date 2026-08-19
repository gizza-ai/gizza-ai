//! gizza-ai/histogram-chart — bin a list of numbers and render an SVG histogram.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_histogram_chart_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_bin_method")]
    bin_method: String,
    #[serde(default = "default_bins")]
    bins: u32,
    #[serde(default)]
    bin_width: f64,
    #[serde(default)]
    range_min: String,
    #[serde(default)]
    range_max: String,
    #[serde(default = "default_normalize")]
    normalize: String,
    #[serde(default)]
    right_closed: bool,
    #[serde(default)]
    show_values: bool,
    #[serde(default)]
    show_mean: bool,
    #[serde(default)]
    show_median: bool,
    #[serde(default)]
    normal_curve: bool,
    #[serde(default)]
    rug: bool,
    #[serde(default = "default_true")]
    grid: bool,
    #[serde(default = "default_orientation")]
    orientation: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    x_label: String,
    #[serde(default)]
    y_label: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default = "default_opacity")]
    opacity: f64,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_precision")]
    precision: u32,
    #[serde(default = "default_output")]
    output: String,
}

fn default_bin_method() -> String { "auto".into() }
fn default_bins() -> u32 { 10 }
fn default_normalize() -> String { "count".into() }
fn default_true() -> bool { true }
fn default_orientation() -> String { "vertical".into() }
fn default_width() -> u32 { 800 }
fn default_height() -> u32 { 480 }
fn default_color() -> String { "#2563eb".into() }
fn default_opacity() -> f64 { 0.9 }
fn default_theme() -> String { "light".into() }
fn default_precision() -> u32 { 4 }
fn default_output() -> String { "svg".into() }

fn to_options(a: &Args) -> Options {
    Options {
        bin_method: a.bin_method.clone(),
        bins: a.bins,
        bin_width: a.bin_width,
        range_min: a.range_min.clone(),
        range_max: a.range_max.clone(),
        normalize: a.normalize.clone(),
        right_closed: a.right_closed,
        show_values: a.show_values,
        show_mean: a.show_mean,
        show_median: a.show_median,
        normal_curve: a.normal_curve,
        rug: a.rug,
        grid: a.grid,
        orientation: a.orientation.clone(),
        title: a.title.clone(),
        x_label: a.x_label.clone(),
        y_label: a.y_label.clone(),
        width: a.width,
        height: a.height,
        color: a.color.clone(),
        opacity: a.opacity,
        theme: a.theme.clone(),
        precision: a.precision,
        output: a.output.clone(),
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The numbers to bin. Paste one value per line or separate them with commas, tabs, semicolons, or spaces; a leading all-text header row is ignored. Plain decimals and scientific notation (1.2e3) both work. Needs at least 2 and at most 100000 values. Required."))
        .param(Param::enumv("bin_method", ["auto", "sturges", "scott", "freedman_diaconis", "rice", "doane", "sqrt", "count", "width"]).default("auto").describe("How the bin edges are chosen. auto (default) takes the finer of Freedman-Diaconis and Sturges; sturges, scott, freedman_diaconis, rice, doane, and sqrt apply that named rule; count uses the explicit `bins` value; width uses the explicit `bin_width` value."))
        .param(Param::integer("bins").min(1.0).max(500.0).default(10).describe("Number of bins to use when bin_method=count, 1-500. Default 10. Ignored by every other bin_method."))
        .param(Param::number("bin_width").min(0.0).default(0.0).describe("Exact width of each bin when bin_method=width, for example 5 for 0-5, 5-10, 10-15. Must be positive in that mode and must not split the range into more than 500 bins. Default 0 (unused)."))
        .param(Param::string("range_min").default("").describe("Lower edge of the plotted range instead of the data minimum, for example 0. Values below it are excluded and counted separately. Default empty uses the data minimum."))
        .param(Param::string("range_max").default("").describe("Upper edge of the plotted range instead of the data maximum, for example 100. Values above it are excluded and counted separately. Default empty uses the data maximum."))
        .param(Param::enumv("normalize", ["count", "relative", "percent", "density", "cumulative_count", "cumulative_percent"]).default("count").describe("What each bar measures: count (default, raw frequency), relative (fraction of n), percent (percent of n), density (bars integrate to 1), cumulative_count, or cumulative_percent."))
        .param(Param::boolean("right_closed").default(false).describe("Use right-closed intervals (a, b] so a value on a bin edge falls in the lower bin. Default false gives left-closed [a, b) intervals, with the last bin closing on the maximum."))
        .param(Param::boolean("show_values").default(false).describe("Print each bar's value above (or beside) the bar. Default false."))
        .param(Param::boolean("show_mean").default(false).describe("Draw a dashed marker line at the mean, labelled with its value. Default false."))
        .param(Param::boolean("show_median").default(false).describe("Draw a dotted marker line at the median, labelled with its value. Default false."))
        .param(Param::boolean("normal_curve").default(false).describe("Overlay the normal (Gaussian) curve for the data's mean and standard deviation, scaled to the chosen normalize mode. Skipped for the cumulative modes, where it has no meaning. Default false."))
        .param(Param::boolean("rug").default(false).describe("Draw a rug plot: one short tick per observation along the value axis. Default false."))
        .param(Param::boolean("grid").default(true).describe("Draw background grid lines along the measurement axis. Default true."))
        .param(Param::enumv("orientation", ["vertical", "horizontal"]).default("vertical").describe("Bar direction: vertical (default, bars grow upward) or horizontal (bars grow rightward)."))
        .param(Param::string("title").default("").describe("Optional chart title drawn above the plot, for example `Response times`."))
        .param(Param::string("x_label").default("").describe("Optional label for the value axis, for example `Latency (ms)`. Default empty shows `Value`."))
        .param(Param::string("y_label").default("").describe("Optional label for the measurement axis, for example `Requests`. Default empty names the normalize mode, such as `Count` or `Density`."))
        .param(Param::integer("width").min(320.0).max(2400.0).default(800).describe("SVG width in pixels, 320-2400. Default 800."))
        .param(Param::integer("height").min(240.0).max(1800.0).default(480).describe("SVG height in pixels, 240-1800. Default 480."))
        .param(Param::string("color").default("#2563eb").describe("Bar fill colour as CSS color text, for example #2563eb, #f00, or tomato. Default #2563eb."))
        .param(Param::number("opacity").min(0.05).max(1.0).default(0.9).describe("Bar fill opacity from 0.05 to 1. Default 0.9; lower values let grid lines and an overlaid curve show through."))
        .param(Param::enumv("theme", ["light", "dark"]).default("light").describe("Chart theme: light (default, white background) or dark (slate background)."))
        .param(Param::integer("precision").min(0.0).max(12.0).default(4).describe("Decimal places used for bin edges, labels, and reported statistics, 0-12. Default 4."))
        .param(Param::enumv("output", ["svg", "table", "csv", "json"]).default("svg").describe("Output format: svg (default, the chart markup), table (text frequency table plus summary stats), csv (machine-readable frequency table), or json (bins plus stats)."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct HistogramChart;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/histogram-chart",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Bin a list of numbers and render an SVG histogram",
    skill(
        description = "Bin a pasted list of numbers and render the distribution as a deterministic SVG histogram. Bin edges come from an automatic rule (Freedman-Diaconis, Sturges, Scott, Rice, Doane, or square-root) or from an explicit bin count or exact bin width, over the data range or a range you fix with range_min/range_max. Bars can measure raw counts, relative frequency, percent, density, or a cumulative count/percent, with optional value labels, mean and median markers, a normal-curve overlay, and a rug plot. Options control interval closure, orientation, grid lines, title, axis labels, SVG size, bar colour, opacity, theme, and decimal precision. Switch output to table, csv, or json for the frequency table and summary statistics (n, min, max, mean, median, sd, quartiles) instead of the chart. Everything runs locally in pure Rust; no plotting service or external data is used.",
        parameters = schema_json()
    ),
)]
impl HistogramChart {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "histogram-chart", |a: Args| {
            let opts = to_options(&a);
            gizza_ai_histogram_chart_core::render(&a.data, &opts).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The numbers to bin. Paste one value per line or separate them with commas, tabs, semicolons, or spaces; a leading all-text header row is ignored. Plain decimals and scientific notation (1.2e3) both work. Needs at least 2 and at most 100000 values. Required." },
                    "bin_method": { "type": "string", "enum": ["auto", "sturges", "scott", "freedman_diaconis", "rice", "doane", "sqrt", "count", "width"], "default": "auto", "description": "How the bin edges are chosen. auto (default) takes the finer of Freedman-Diaconis and Sturges; sturges, scott, freedman_diaconis, rice, doane, and sqrt apply that named rule; count uses the explicit `bins` value; width uses the explicit `bin_width` value." },
                    "bins": { "type": "integer", "default": 10, "minimum": 1, "maximum": 500, "description": "Number of bins to use when bin_method=count, 1-500. Default 10. Ignored by every other bin_method." },
                    "bin_width": { "type": "number", "default": 0.0, "minimum": 0, "description": "Exact width of each bin when bin_method=width, for example 5 for 0-5, 5-10, 10-15. Must be positive in that mode and must not split the range into more than 500 bins. Default 0 (unused)." },
                    "range_min": { "type": "string", "default": "", "description": "Lower edge of the plotted range instead of the data minimum, for example 0. Values below it are excluded and counted separately. Default empty uses the data minimum." },
                    "range_max": { "type": "string", "default": "", "description": "Upper edge of the plotted range instead of the data maximum, for example 100. Values above it are excluded and counted separately. Default empty uses the data maximum." },
                    "normalize": { "type": "string", "enum": ["count", "relative", "percent", "density", "cumulative_count", "cumulative_percent"], "default": "count", "description": "What each bar measures: count (default, raw frequency), relative (fraction of n), percent (percent of n), density (bars integrate to 1), cumulative_count, or cumulative_percent." },
                    "right_closed": { "type": "boolean", "default": false, "description": "Use right-closed intervals (a, b] so a value on a bin edge falls in the lower bin. Default false gives left-closed [a, b) intervals, with the last bin closing on the maximum." },
                    "show_values": { "type": "boolean", "default": false, "description": "Print each bar's value above (or beside) the bar. Default false." },
                    "show_mean": { "type": "boolean", "default": false, "description": "Draw a dashed marker line at the mean, labelled with its value. Default false." },
                    "show_median": { "type": "boolean", "default": false, "description": "Draw a dotted marker line at the median, labelled with its value. Default false." },
                    "normal_curve": { "type": "boolean", "default": false, "description": "Overlay the normal (Gaussian) curve for the data's mean and standard deviation, scaled to the chosen normalize mode. Skipped for the cumulative modes, where it has no meaning. Default false." },
                    "rug": { "type": "boolean", "default": false, "description": "Draw a rug plot: one short tick per observation along the value axis. Default false." },
                    "grid": { "type": "boolean", "default": true, "description": "Draw background grid lines along the measurement axis. Default true." },
                    "orientation": { "type": "string", "enum": ["vertical", "horizontal"], "default": "vertical", "description": "Bar direction: vertical (default, bars grow upward) or horizontal (bars grow rightward)." },
                    "title": { "type": "string", "default": "", "description": "Optional chart title drawn above the plot, for example `Response times`." },
                    "x_label": { "type": "string", "default": "", "description": "Optional label for the value axis, for example `Latency (ms)`. Default empty shows `Value`." },
                    "y_label": { "type": "string", "default": "", "description": "Optional label for the measurement axis, for example `Requests`. Default empty names the normalize mode, such as `Count` or `Density`." },
                    "width": { "type": "integer", "default": 800, "minimum": 320, "maximum": 2400, "description": "SVG width in pixels, 320-2400. Default 800." },
                    "height": { "type": "integer", "default": 480, "minimum": 240, "maximum": 1800, "description": "SVG height in pixels, 240-1800. Default 480." },
                    "color": { "type": "string", "default": "#2563eb", "description": "Bar fill colour as CSS color text, for example #2563eb, #f00, or tomato. Default #2563eb." },
                    "opacity": { "type": "number", "default": 0.9, "minimum": 0.05, "maximum": 1, "description": "Bar fill opacity from 0.05 to 1. Default 0.9; lower values let grid lines and an overlaid curve show through." },
                    "theme": { "type": "string", "enum": ["light", "dark"], "default": "light", "description": "Chart theme: light (default, white background) or dark (slate background)." },
                    "precision": { "type": "integer", "default": 4, "minimum": 0, "maximum": 12, "description": "Decimal places used for bin edges, labels, and reported statistics, 0-12. Default 4." },
                    "output": { "type": "string", "enum": ["svg", "table", "csv", "json"], "default": "svg", "description": "Output format: svg (default, the chart markup), table (text frequency table plus summary stats), csv (machine-readable frequency table), or json (bins plus stats)." }
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
        assert_eq!(props.len(), 25, "parameter count changed");
        for (name, spec) in props {
            let desc = spec["description"].as_str().unwrap_or("");
            assert!(desc.len() > 20, "param {name} needs a real description");
        }
    }

    #[test]
    fn args_defaults_match_the_descriptor() {
        let a: Args = serde_json::from_str(r#"{"data":"1,2,3"}"#).unwrap();
        let o = to_options(&a);
        assert_eq!(o.bin_method, "auto");
        assert_eq!(o.bins, 10);
        assert_eq!(o.bin_width, 0.0);
        assert_eq!(o.normalize, "count");
        assert!(!o.right_closed);
        assert!(!o.show_values);
        assert!(!o.show_mean);
        assert!(!o.show_median);
        assert!(!o.normal_curve);
        assert!(!o.rug);
        assert!(o.grid);
        assert_eq!(o.orientation, "vertical");
        assert_eq!(o.width, 800);
        assert_eq!(o.height, 480);
        assert_eq!(o.color, "#2563eb");
        assert_eq!(o.opacity, 0.9);
        assert_eq!(o.theme, "light");
        assert_eq!(o.precision, 4);
        assert_eq!(o.output, "svg");
    }

    #[test]
    fn descriptor_defaults_match_the_core_defaults() {
        let a: Args = serde_json::from_str(r#"{"data":"1,2,3"}"#).unwrap();
        let from_args = to_options(&a);
        let core_default = Options::default();
        assert_eq!(from_args.bin_method, core_default.bin_method);
        assert_eq!(from_args.bins, core_default.bins);
        assert_eq!(from_args.normalize, core_default.normalize);
        assert_eq!(from_args.grid, core_default.grid);
        assert_eq!(from_args.orientation, core_default.orientation);
        assert_eq!(from_args.color, core_default.color);
        assert_eq!(from_args.opacity, core_default.opacity);
        assert_eq!(from_args.output, core_default.output);
    }
}
