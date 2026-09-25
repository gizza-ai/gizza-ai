//! gizza-ai/pareto-chart — rank categories and render a Pareto chart as deterministic SVG.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_pareto_chart_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_header")]
    header: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default)]
    max_categories: u32,
    #[serde(default = "default_other_label")]
    other_label: String,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_true")]
    highlight_vital_few: bool,
    #[serde(default = "default_true")]
    show_cumulative: bool,
    #[serde(default)]
    show_values: bool,
    #[serde(default)]
    show_cumulative_labels: bool,
    #[serde(default = "default_decimals")]
    decimals: u32,
    #[serde(default)]
    title: String,
    #[serde(default)]
    category_label: String,
    #[serde(default)]
    value_label: String,
    #[serde(default = "default_percent_label")]
    percent_label: String,
    #[serde(default)]
    label_angle: f64,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default = "default_vital_color")]
    vital_color: String,
    #[serde(default = "default_line_color")]
    line_color: String,
    #[serde(default = "default_threshold_color")]
    threshold_color: String,
    #[serde(default)]
    background: String,
    #[serde(default = "default_bar_width")]
    bar_width: f64,
    #[serde(default = "default_line_width")]
    line_width: f64,
    #[serde(default = "default_point_radius")]
    point_radius: f64,
    #[serde(default = "default_true")]
    grid: bool,
    #[serde(default = "default_true")]
    legend: bool,
    #[serde(default = "default_font_size")]
    font_size: f64,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_delimiter() -> String {
    "auto".into()
}
fn default_header() -> String {
    "auto".into()
}
fn default_sort() -> String {
    "desc".into()
}
fn default_other_label() -> String {
    "Other".into()
}
fn default_threshold() -> f64 {
    80.0
}
fn default_true() -> bool {
    true
}
fn default_decimals() -> u32 {
    1
}
fn default_percent_label() -> String {
    "Cumulative %".into()
}
fn default_color() -> String {
    "#2563eb".into()
}
fn default_vital_color() -> String {
    "#f97316".into()
}
fn default_line_color() -> String {
    "#dc2626".into()
}
fn default_threshold_color() -> String {
    "#94a3b8".into()
}
fn default_bar_width() -> f64 {
    0.8
}
fn default_line_width() -> f64 {
    2.0
}
fn default_point_radius() -> f64 {
    3.5
}
fn default_font_size() -> f64 {
    13.0
}
fn default_width() -> u32 {
    820
}
fn default_height() -> u32 {
    520
}
fn default_theme() -> String {
    "light".into()
}
fn default_output() -> String {
    "svg".into()
}

fn to_options(a: &Args) -> Options {
    Options {
        delimiter: a.delimiter.clone(),
        header: a.header.clone(),
        sort: a.sort.clone(),
        max_categories: a.max_categories,
        other_label: a.other_label.clone(),
        threshold: a.threshold,
        highlight_vital_few: a.highlight_vital_few,
        show_cumulative: a.show_cumulative,
        show_values: a.show_values,
        show_cumulative_labels: a.show_cumulative_labels,
        decimals: a.decimals,
        title: a.title.clone(),
        category_label: a.category_label.clone(),
        value_label: a.value_label.clone(),
        percent_label: a.percent_label.clone(),
        label_angle: a.label_angle,
        color: a.color.clone(),
        vital_color: a.vital_color.clone(),
        line_color: a.line_color.clone(),
        threshold_color: a.threshold_color.clone(),
        background: a.background.clone(),
        bar_width: a.bar_width,
        line_width: a.line_width,
        point_radius: a.point_radius,
        grid: a.grid,
        legend: a.legend,
        font_size: a.font_size,
        width: a.width,
        height: a.height,
        theme: a.theme.clone(),
        output: a.output.clone(),
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The categories to rank, one `label,value` row per line, for example `Late delivery,45`. Comma, tab, semicolon, pipe, and whitespace separated rows all work; quoted CSV fields are honoured, `#` starts a comment, a header row is detected automatically, duplicate labels are summed, and values may carry thousands separators, a currency mark, or a trailing %. Values must be zero or positive. Up to 10000 rows and 500 categories. Example: `Late delivery,45`\\n`Wrong item,30`\\n`Damaged,15`\\n`Billing error,7`\\n`Rude staff,3`."))
        .param(Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe", "whitespace"]).default("auto").describe("How each row is split. auto (default) picks the separator that appears on every row, preferring tab, semicolon, pipe, then comma, and falls back to whitespace. Choose whitespace to read `Late delivery 45` rows, where everything before the last space is the label."))
        .param(Param::enumv("header", ["auto", "yes", "no"]).default("auto").describe("Whether the first row is a header. auto (default) treats it as a header when its value column is not a number, and then reuses its two column names as the category and value axis titles. yes always skips it, no always plots it."))
        .param(Param::enumv("sort", ["desc", "asc", "input"]).default("desc").describe("Bar order: desc (default) is the classic Pareto ranking, biggest contributor first, which is what makes the cumulative line meaningful; asc puts the smallest first for tail reviews; input keeps your pasted order to audit an already-ranked table. Ties keep input order."))
        .param(Param::integer("max_categories").min(0.0).max(200.0).default(0).describe("Keep only the first N bars after sorting and roll the rest into one bucket, 0-200. Default 0 keeps every category. Set 8-12 when a long tail squashes the chart; the bucket is named by other_label and still counts toward the total and the cumulative line."))
        .param(Param::string("other_label").default("Other").describe("Name of the bucket that absorbs the tail when max_categories is set, for example `Other` or `All other causes`. Default `Other`. Ignored when max_categories is 0."))
        .param(Param::number("threshold").min(0.0).max(100.0).default(80.0).describe("Cumulative-percentage cutoff that defines the vital few, 0-100. Default 80 for the classic 80/20 rule. Drawn as a dashed horizontal reference line on the percentage axis; set 0 to hide the line and the vital-few marking entirely."))
        .param(Param::boolean("highlight_vital_few").default(true).describe("Paint every bar up to and including the one that crosses the threshold in vital_color instead of color, so the vital few separate from the trivial many at a glance. Default true. Has no effect when threshold is 0."))
        .param(Param::boolean("show_cumulative").default(true).describe("Draw the cumulative-percentage line, its markers, and the right-hand 0-100% axis. Default true — it is what makes this a Pareto chart rather than a sorted bar chart. Turn it off for a plain ranked bar chart."))
        .param(Param::boolean("show_values").default(false).describe("Print each bar's value just above the bar. Default false; useful for 5-10 bars, crowded beyond that."))
        .param(Param::boolean("show_cumulative_labels").default(false).describe("Print the running cumulative percentage above each point on the line. Default false. Turn it on when the exact crossing percentage matters more than the shape."))
        .param(Param::integer("decimals").min(0.0).max(6.0).default(1).describe("Decimal places for percentages and axis values, 0-6. Default 1, so 83.3%. Use 0 for whole-number counts and percentages."))
        .param(Param::string("title").default("").describe("Optional chart title drawn centred above the plot, for example `Q3 customer complaints`. Also printed above the summary table."))
        .param(Param::string("category_label").default("").describe("Title for the horizontal category axis, for example `Complaint reason`. Default empty reuses the pasted header row's first column name, if there was one."))
        .param(Param::string("value_label").default("").describe("Title for the left value axis, for example `Complaints` or `Cost (USD)`. Default empty reuses the pasted header row's second column name, if there was one; it also names the value column in summary output and the bar entry in the legend."))
        .param(Param::string("percent_label").default("Cumulative %").describe("Title for the right-hand percentage axis. Default `Cumulative %`. Set it empty to drop the axis title and keep just the 0-100% ticks."))
        .param(Param::number("label_angle").min(0.0).max(90.0).default(0.0).describe("Rotate the category labels counterclockwise by this many degrees, 0-90. Default 0 (horizontal). Use 45 or 90 when long labels would otherwise overlap; the bottom margin grows to fit them."))
        .param(Param::string("color").default("#2563eb").describe("Bar fill colour as CSS colour text, for example #2563eb, #f00, or steelblue. Default #2563eb. Applies to the trivial-many bars when highlight_vital_few is on, and to every bar when it is off."))
        .param(Param::string("vital_color").default("#f97316").describe("Fill colour for the vital-few bars, those up to the threshold crossing, for example #f97316 or tomato. Default #f97316. Used only when highlight_vital_few is on and threshold is above 0."))
        .param(Param::string("line_color").default("#dc2626").describe("Colour of the cumulative-percentage line, its markers, and its labels, for example #dc2626 or black. Default #dc2626."))
        .param(Param::string("threshold_color").default("#94a3b8").describe("Colour of the dashed threshold reference line, for example #94a3b8 or grey. Default #94a3b8. Ignored when threshold is 0."))
        .param(Param::string("background").default("").describe("Chart background colour, for example #ffffff or transparent. Default empty uses the theme background: white for light, dark slate for dark."))
        .param(Param::number("bar_width").min(0.05).max(1.0).default(0.8).describe("Bar width as a fraction of the slot each category gets, 0.05-1. Default 0.8, which leaves a clear gap between bars. Use 1 for a touching histogram-style look."))
        .param(Param::number("line_width").min(0.0).max(8.0).default(2.0).describe("Stroke width of the cumulative line in pixels, 0-8. Default 2. Use 0 to keep only the point markers."))
        .param(Param::number("point_radius").min(0.0).max(12.0).default(3.5).describe("Radius of the marker drawn at each cumulative point in pixels, 0-12. Default 3.5; use 0 to hide the markers. Markers carry a hover tooltip with the category and its cumulative percentage."))
        .param(Param::boolean("grid").default(true).describe("Draw horizontal grid lines across the plot at each axis tick. Default true; both axes share the same five gridlines, so 20% steps line up with the value ticks."))
        .param(Param::boolean("legend").default(true).describe("Draw a legend strip under the chart naming the bars, the vital-few colour, the cumulative line, and the threshold line. Default true."))
        .param(Param::number("font_size").min(6.0).max(48.0).default(13.0).describe("Base font size in pixels for labels and the legend, 6-48. Default 13. Axis ticks and value labels are drawn slightly smaller; the title slightly larger."))
        .param(Param::integer("width").min(320.0).max(2400.0).default(820).describe("SVG width in pixels, 320-2400. Default 820. Widen it when you have many categories so the bars stay readable."))
        .param(Param::integer("height").min(240.0).max(1800.0).default(520).describe("SVG height in pixels, 240-1800. Default 520."))
        .param(Param::enumv("theme", ["light", "dark"]).default("light").describe("Chart theme: light (default) or dark. Sets the background, grid, axis, and text colours; override the background alone with the background parameter."))
        .param(Param::enumv("output", ["svg", "summary", "json"]).default("svg").describe("Output format: svg (default, self-contained markup you can paste into a document), summary (aligned text table with each category's value, percent of total, running total, cumulative percent, and vital-few mark, plus a one-line vital-few verdict), or json (machine-readable rows plus total, crossing index, crossing label, and vital-few count)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ParetoChart;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pareto-chart",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rank categories and render a Pareto chart: sorted bars plus a cumulative-percentage line as deterministic SVG",
    skill(
        description = "Render a Pareto chart from pasted `label,value` rows: bars sorted largest-first on a value axis, a cumulative-percentage line on a second 0-100% axis, and a dashed threshold line (80% by default) that separates the vital few from the trivial many. Detects the delimiter and an optional header row, sums duplicate labels, tolerates thousands separators, currency marks, and trailing percent signs, and can roll a long tail into one `Other` bar. Returns self-contained SVG by default, or a summary table / JSON with each category's value, percent of total, running total, cumulative percent, vital-few mark, and the exact threshold-crossing category. Options control the delimiter, header handling, sort order, tail bucketing, threshold, vital-few highlighting, value and cumulative labels, decimals, title and all three axis titles, category label rotation, bar/vital/line/threshold/background colours, bar width, line width, point radius, grid, legend, font size, canvas size, theme, and output format. Everything runs locally in pure Rust; no plotting service, fonts, or external data are used.",
        parameters = schema_json()
    ),
)]
impl ParetoChart {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pareto-chart", |a: Args| {
            let opts = to_options(&a);
            gizza_ai_pareto_chart_core::render(&a.data, &opts).map_err(SkillError::InvalidArgs)
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
    fn dump_schema() {
        println!("SCHEMA_BEGIN{}SCHEMA_END", schema_json());
    }

    #[test]
    fn every_param_is_described() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().expect("object schema");
        assert_eq!(props.len(), 32, "parameter count changed");
        for (name, spec) in props {
            let desc = spec["description"].as_str().unwrap_or("");
            assert!(desc.len() > 20, "param {name} needs a real description");
        }
        assert_eq!(
            schema["required"].as_array().unwrap(),
            &vec![serde_json::json!("data")]
        );
    }

    #[test]
    fn args_defaults_match_the_descriptor() {
        let a: Args = serde_json::from_str(r#"{"data":"A,3\nB,1"}"#).unwrap();
        let o = to_options(&a);
        let d = Options::default();
        assert_eq!(o.delimiter, d.delimiter);
        assert_eq!(o.header, d.header);
        assert_eq!(o.sort, d.sort);
        assert_eq!(o.max_categories, d.max_categories);
        assert_eq!(o.other_label, d.other_label);
        assert_eq!(o.threshold, d.threshold);
        assert_eq!(o.highlight_vital_few, d.highlight_vital_few);
        assert_eq!(o.show_cumulative, d.show_cumulative);
        assert_eq!(o.show_values, d.show_values);
        assert_eq!(o.show_cumulative_labels, d.show_cumulative_labels);
        assert_eq!(o.decimals, d.decimals);
        assert_eq!(o.title, d.title);
        assert_eq!(o.category_label, d.category_label);
        assert_eq!(o.value_label, d.value_label);
        assert_eq!(o.percent_label, d.percent_label);
        assert_eq!(o.label_angle, d.label_angle);
        assert_eq!(o.color, d.color);
        assert_eq!(o.vital_color, d.vital_color);
        assert_eq!(o.line_color, d.line_color);
        assert_eq!(o.threshold_color, d.threshold_color);
        assert_eq!(o.background, d.background);
        assert_eq!(o.bar_width, d.bar_width);
        assert_eq!(o.line_width, d.line_width);
        assert_eq!(o.point_radius, d.point_radius);
        assert_eq!(o.grid, d.grid);
        assert_eq!(o.legend, d.legend);
        assert_eq!(o.font_size, d.font_size);
        assert_eq!(o.width, d.width);
        assert_eq!(o.height, d.height);
        assert_eq!(o.theme, d.theme);
        assert_eq!(o.output, d.output);
    }

    /// Drift guard: the chat/CLI/page schema is generated from `descriptor()`, so any
    /// change to a param name, type, enum, bound, or default must be mirrored here.
    #[test]
    fn schema_matches_the_authored_contract() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["data"],
            "properties": {
                "data": {
                    "type": "string",
                    "description": "The categories to rank, one `label,value` row per line, for example `Late delivery,45`. Comma, tab, semicolon, pipe, and whitespace separated rows all work; quoted CSV fields are honoured, `#` starts a comment, a header row is detected automatically, duplicate labels are summed, and values may carry thousands separators, a currency mark, or a trailing %. Values must be zero or positive. Up to 10000 rows and 500 categories. Example: `Late delivery,45`\\n`Wrong item,30`\\n`Damaged,15`\\n`Billing error,7`\\n`Rude staff,3`."
                },
                "delimiter": {
                    "type": "string",
                    "enum": ["auto", "comma", "tab", "semicolon", "pipe", "whitespace"],
                    "default": "auto",
                    "description": "How each row is split. auto (default) picks the separator that appears on every row, preferring tab, semicolon, pipe, then comma, and falls back to whitespace. Choose whitespace to read `Late delivery 45` rows, where everything before the last space is the label."
                },
                "header": {
                    "type": "string",
                    "enum": ["auto", "yes", "no"],
                    "default": "auto",
                    "description": "Whether the first row is a header. auto (default) treats it as a header when its value column is not a number, and then reuses its two column names as the category and value axis titles. yes always skips it, no always plots it."
                },
                "sort": {
                    "type": "string",
                    "enum": ["desc", "asc", "input"],
                    "default": "desc",
                    "description": "Bar order: desc (default) is the classic Pareto ranking, biggest contributor first, which is what makes the cumulative line meaningful; asc puts the smallest first for tail reviews; input keeps your pasted order to audit an already-ranked table. Ties keep input order."
                },
                "max_categories": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 200,
                    "default": 0,
                    "description": "Keep only the first N bars after sorting and roll the rest into one bucket, 0-200. Default 0 keeps every category. Set 8-12 when a long tail squashes the chart; the bucket is named by other_label and still counts toward the total and the cumulative line."
                },
                "other_label": {
                    "type": "string",
                    "default": "Other",
                    "description": "Name of the bucket that absorbs the tail when max_categories is set, for example `Other` or `All other causes`. Default `Other`. Ignored when max_categories is 0."
                },
                "threshold": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 100,
                    "default": 80.0,
                    "description": "Cumulative-percentage cutoff that defines the vital few, 0-100. Default 80 for the classic 80/20 rule. Drawn as a dashed horizontal reference line on the percentage axis; set 0 to hide the line and the vital-few marking entirely."
                },
                "highlight_vital_few": {
                    "type": "boolean",
                    "default": true,
                    "description": "Paint every bar up to and including the one that crosses the threshold in vital_color instead of color, so the vital few separate from the trivial many at a glance. Default true. Has no effect when threshold is 0."
                },
                "show_cumulative": {
                    "type": "boolean",
                    "default": true,
                    "description": "Draw the cumulative-percentage line, its markers, and the right-hand 0-100% axis. Default true — it is what makes this a Pareto chart rather than a sorted bar chart. Turn it off for a plain ranked bar chart."
                },
                "show_values": {
                    "type": "boolean",
                    "default": false,
                    "description": "Print each bar's value just above the bar. Default false; useful for 5-10 bars, crowded beyond that."
                },
                "show_cumulative_labels": {
                    "type": "boolean",
                    "default": false,
                    "description": "Print the running cumulative percentage above each point on the line. Default false. Turn it on when the exact crossing percentage matters more than the shape."
                },
                "decimals": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 6,
                    "default": 1,
                    "description": "Decimal places for percentages and axis values, 0-6. Default 1, so 83.3%. Use 0 for whole-number counts and percentages."
                },
                "title": {
                    "type": "string",
                    "default": "",
                    "description": "Optional chart title drawn centred above the plot, for example `Q3 customer complaints`. Also printed above the summary table."
                },
                "category_label": {
                    "type": "string",
                    "default": "",
                    "description": "Title for the horizontal category axis, for example `Complaint reason`. Default empty reuses the pasted header row's first column name, if there was one."
                },
                "value_label": {
                    "type": "string",
                    "default": "",
                    "description": "Title for the left value axis, for example `Complaints` or `Cost (USD)`. Default empty reuses the pasted header row's second column name, if there was one; it also names the value column in summary output and the bar entry in the legend."
                },
                "percent_label": {
                    "type": "string",
                    "default": "Cumulative %",
                    "description": "Title for the right-hand percentage axis. Default `Cumulative %`. Set it empty to drop the axis title and keep just the 0-100% ticks."
                },
                "label_angle": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 90,
                    "default": 0.0,
                    "description": "Rotate the category labels counterclockwise by this many degrees, 0-90. Default 0 (horizontal). Use 45 or 90 when long labels would otherwise overlap; the bottom margin grows to fit them."
                },
                "color": {
                    "type": "string",
                    "default": "#2563eb",
                    "description": "Bar fill colour as CSS colour text, for example #2563eb, #f00, or steelblue. Default #2563eb. Applies to the trivial-many bars when highlight_vital_few is on, and to every bar when it is off."
                },
                "vital_color": {
                    "type": "string",
                    "default": "#f97316",
                    "description": "Fill colour for the vital-few bars, those up to the threshold crossing, for example #f97316 or tomato. Default #f97316. Used only when highlight_vital_few is on and threshold is above 0."
                },
                "line_color": {
                    "type": "string",
                    "default": "#dc2626",
                    "description": "Colour of the cumulative-percentage line, its markers, and its labels, for example #dc2626 or black. Default #dc2626."
                },
                "threshold_color": {
                    "type": "string",
                    "default": "#94a3b8",
                    "description": "Colour of the dashed threshold reference line, for example #94a3b8 or grey. Default #94a3b8. Ignored when threshold is 0."
                },
                "background": {
                    "type": "string",
                    "default": "",
                    "description": "Chart background colour, for example #ffffff or transparent. Default empty uses the theme background: white for light, dark slate for dark."
                },
                "bar_width": {
                    "type": "number",
                    "minimum": 0.05,
                    "maximum": 1,
                    "default": 0.8,
                    "description": "Bar width as a fraction of the slot each category gets, 0.05-1. Default 0.8, which leaves a clear gap between bars. Use 1 for a touching histogram-style look."
                },
                "line_width": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 8,
                    "default": 2.0,
                    "description": "Stroke width of the cumulative line in pixels, 0-8. Default 2. Use 0 to keep only the point markers."
                },
                "point_radius": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 12,
                    "default": 3.5,
                    "description": "Radius of the marker drawn at each cumulative point in pixels, 0-12. Default 3.5; use 0 to hide the markers. Markers carry a hover tooltip with the category and its cumulative percentage."
                },
                "grid": {
                    "type": "boolean",
                    "default": true,
                    "description": "Draw horizontal grid lines across the plot at each axis tick. Default true; both axes share the same five gridlines, so 20% steps line up with the value ticks."
                },
                "legend": {
                    "type": "boolean",
                    "default": true,
                    "description": "Draw a legend strip under the chart naming the bars, the vital-few colour, the cumulative line, and the threshold line. Default true."
                },
                "font_size": {
                    "type": "number",
                    "minimum": 6,
                    "maximum": 48,
                    "default": 13.0,
                    "description": "Base font size in pixels for labels and the legend, 6-48. Default 13. Axis ticks and value labels are drawn slightly smaller; the title slightly larger."
                },
                "width": {
                    "type": "integer",
                    "minimum": 320,
                    "maximum": 2400,
                    "default": 820,
                    "description": "SVG width in pixels, 320-2400. Default 820. Widen it when you have many categories so the bars stay readable."
                },
                "height": {
                    "type": "integer",
                    "minimum": 240,
                    "maximum": 1800,
                    "default": 520,
                    "description": "SVG height in pixels, 240-1800. Default 520."
                },
                "theme": {
                    "type": "string",
                    "enum": ["light", "dark"],
                    "default": "light",
                    "description": "Chart theme: light (default) or dark. Sets the background, grid, axis, and text colours; override the background alone with the background parameter."
                },
                "output": {
                    "type": "string",
                    "enum": ["svg", "summary", "json"],
                    "default": "svg",
                    "description": "Output format: svg (default, self-contained markup you can paste into a document), summary (aligned text table with each category's value, percent of total, running total, cumulative percent, and vital-few mark, plus a one-line vital-few verdict), or json (machine-readable rows plus total, crossing index, crossing label, and vital-few count)."
                }
            }
        });
        assert_eq!(
            actual, authored,
            "descriptor drifted from the authored schema"
        );
    }
}
