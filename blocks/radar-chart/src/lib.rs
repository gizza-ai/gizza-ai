//! gizza-ai/radar-chart — render radar/spider charts as deterministic SVG.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_radar_chart_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_layout")]
    layout: String,
    #[serde(default = "default_scale")]
    scale: String,
    #[serde(default)]
    scale_min: f64,
    #[serde(default)]
    scale_max: f64,
    #[serde(default = "default_rings")]
    rings: u32,
    #[serde(default = "default_grid_shape")]
    grid_shape: String,
    #[serde(default = "default_true")]
    show_spokes: bool,
    #[serde(default = "default_true")]
    show_axis_labels: bool,
    #[serde(default = "default_true")]
    show_ticks: bool,
    #[serde(default)]
    show_values: bool,
    #[serde(default = "default_fill_opacity")]
    fill_opacity: f64,
    #[serde(default = "default_line_width")]
    line_width: f64,
    #[serde(default = "default_point_radius")]
    point_radius: f64,
    #[serde(default)]
    start_angle: f64,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default = "default_palette")]
    palette: String,
    #[serde(default)]
    colors: String,
    #[serde(default)]
    background: String,
    #[serde(default)]
    title: String,
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

fn default_layout() -> String {
    "auto".into()
}
fn default_scale() -> String {
    "shared".into()
}
fn default_rings() -> u32 {
    5
}
fn default_grid_shape() -> String {
    "polygon".into()
}
fn default_true() -> bool {
    true
}
fn default_fill_opacity() -> f64 {
    0.25
}
fn default_line_width() -> f64 {
    2.0
}
fn default_point_radius() -> f64 {
    3.0
}
fn default_direction() -> String {
    "clockwise".into()
}
fn default_palette() -> String {
    "default".into()
}
fn default_font_size() -> f64 {
    13.0
}
fn default_width() -> u32 {
    700
}
fn default_height() -> u32 {
    560
}
fn default_theme() -> String {
    "light".into()
}
fn default_output() -> String {
    "svg".into()
}

fn to_options(a: &Args) -> Options {
    Options {
        layout: a.layout.clone(),
        scale: a.scale.clone(),
        scale_min: a.scale_min,
        scale_max: a.scale_max,
        rings: a.rings,
        grid_shape: a.grid_shape.clone(),
        show_spokes: a.show_spokes,
        show_axis_labels: a.show_axis_labels,
        show_ticks: a.show_ticks,
        show_values: a.show_values,
        fill_opacity: a.fill_opacity,
        line_width: a.line_width,
        point_radius: a.point_radius,
        start_angle: a.start_angle,
        direction: a.direction.clone(),
        palette: a.palette.clone(),
        colors: a.colors.clone(),
        background: a.background.clone(),
        title: a.title.clone(),
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
        .param(Param::string("data").required().describe("The table to plot, one row per line. Wide form is a header row of axis names then one row of numbers per series, for example `product,Camera,Battery,Speed` then `Phone A,8,7,9`. Long form is `series,axis,value` triples. A plain `axis,value` list draws one series. Comma, tab and semicolon separated rows all work, a space-separated row is read as `axis value`, `#` starts a comment, and values may carry thousands separators, a currency mark, or a trailing %. At least 3 axes are required. Example: `product,Camera,Battery,Speed,Price`\\n`Phone A,8,7,9,6`\\n`Phone B,6,9,7,8`."))
        .param(Param::enumv("layout", ["auto", "wide", "long", "single"]).default("auto").describe("How to read the pasted table: auto (default) detects the shape, wide is a header row of axis names plus one numeric row per series, long is `series,axis,value` triples, single is a two-column `axis,value` list drawn as one series."))
        .param(Param::enumv("scale", ["shared", "per_axis", "percent"]).default("shared").describe("How values map to the radius: shared (default) puts every axis on one scale so magnitudes stay comparable, per_axis rescales each axis to its own maximum so mixed units such as revenue and a 1-5 rating can share a chart, percent pins the domain to 0-100 for scores that are already percentages."))
        .param(Param::number("scale_min").min(-1000000000.0).max(1000000000.0).default(0.0).describe("The value at the centre of the chart. Default 0, which is what makes radar areas honest; raise it to zoom into a narrow band such as 60-100. Values below it are drawn at the centre."))
        .param(Param::number("scale_max").min(0.0).max(1000000000.0).default(0.0).describe("The value at the outer ring. Default 0 means derive it from the data and round up to a clean number such as 10, 25, or 500. Ignored when scale is per_axis (each axis derives its own maximum)."))
        .param(Param::integer("rings").min(0.0).max(10.0).default(5).describe("Number of concentric grid rings, 0-10. Default 5. Use 0 for just the outer boundary and no tick labels; 4-5 reads best."))
        .param(Param::enumv("grid_shape", ["polygon", "circle"]).default("polygon").describe("Grid style: polygon (default) draws straight-edged web rings through the axis points, circle draws smooth concentric circles."))
        .param(Param::boolean("show_spokes").default(true).describe("Draw the radial line from the centre out to each axis. Default true; turn it off for a cleaner web when you have many axes."))
        .param(Param::boolean("show_axis_labels").default(true).describe("Draw each axis name around the outside of the chart. Default true. Turning it off frees the reserved margin and grows the plotted radius."))
        .param(Param::boolean("show_ticks").default(true).describe("Draw the scale value next to each grid ring. Default true. Shows percentages instead of values when scale is per_axis, since each axis then has its own maximum."))
        .param(Param::boolean("show_values").default(false).describe("Print each series' number just outside its vertex on every axis. Default false; useful for 1-2 series, crowded beyond that."))
        .param(Param::number("fill_opacity").min(0.0).max(1.0).default(0.25).describe("Opacity of each series' filled polygon, 0-1. Default 0.25, which keeps 2-4 overlapping shapes readable. Use 0 for outline-only charts."))
        .param(Param::number("line_width").min(0.0).max(8.0).default(2.0).describe("Stroke width of each series outline in pixels, 0-8. Default 2. Use 0 with a higher fill_opacity for a flat filled look."))
        .param(Param::number("point_radius").min(0.0).max(12.0).default(3.0).describe("Radius of the marker drawn at each vertex in pixels, 0-12. Default 3; use 0 to hide markers. Markers carry a hover tooltip with the series, axis, and exact value."))
        .param(Param::number("start_angle").min(-360.0).max(360.0).default(0.0).describe("Rotate the whole chart, in degrees, -360 to 360. Default 0 puts the first axis straight up; 45 rotates it an eighth-turn clockwise."))
        .param(Param::enumv("direction", ["clockwise", "counterclockwise"]).default("clockwise").describe("Order the axes are placed around the circle from the starting angle: clockwise (default) or counterclockwise."))
        .param(Param::enumv("palette", ["default", "pastel", "dusk", "earth", "ocean", "mono"]).default("default").describe("Series colour scheme: default (bright, high contrast), pastel, dusk (deep), earth, ocean, or mono (one hue lightened per series, based on the first entry of colors). Overridden per series by colors."))
        .param(Param::string("colors").default("").describe("Comma-separated colour overrides, one per series and cycled if short, for example `#2563eb,#f97316` or `tomato,steelblue`. Default empty uses the palette. With palette=mono the first entry is the base hue."))
        .param(Param::string("background").default("").describe("Chart background colour, for example #ffffff or transparent. Default empty uses the theme background (white for light, dark slate for dark)."))
        .param(Param::string("title").default("").describe("Optional chart title centred above the web, for example `Phone comparison`."))
        .param(Param::boolean("legend").default(true).describe("Draw a legend strip under the chart naming each series with its colour swatch. Default true because radar charts are comparison charts."))
        .param(Param::number("font_size").min(6.0).max(48.0).default(13.0).describe("Base font size in pixels for axis captions and the legend, 6-48. Default 13. Tick and value labels are drawn slightly smaller."))
        .param(Param::integer("width").min(320.0).max(2400.0).default(700).describe("SVG width in pixels, 320-2400. Default 700."))
        .param(Param::integer("height").min(240.0).max(1800.0).default(560).describe("SVG height in pixels, 240-1800. Default 560. Radar charts read best close to square."))
        .param(Param::enumv("theme", ["light", "dark"]).default("light").describe("Chart theme: light (default) or dark. Sets the background, grid, and text colours; override the background alone with the background parameter."))
        .param(Param::enumv("output", ["svg", "summary", "json"]).default("svg").describe("Output format: svg (default, self-contained markup), summary (aligned text table of every series and axis with its value and scaled percentage, plus per-series means), or json (machine-readable axis domains, angles, and vertex coordinates)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RadarChart;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/radar-chart",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render radar/spider charts comparing entities across numeric axes as deterministic SVG",
    skill(
        description = "Render a radar (spider) chart from a pasted table: a wide `series,Axis1,Axis2,…` matrix, long `series,axis,value` triples, or a single-series `axis,value` list. Normalizes values onto a shared, per-axis, or 0-100 percentage radial scale and returns self-contained SVG by default, or a summary table / JSON vertex geometry. Options control layout detection, scale mode, scale minimum and maximum, ring count, polygon or circle grid, spokes, axis captions, tick labels, per-vertex value labels, fill opacity, line width, point radius, start angle, direction, colour palette, explicit series colours, background, title, legend, font size, canvas size, and theme. Requires at least 3 axes. Everything runs locally in pure Rust; no plotting service, fonts, or external data are used.",
        parameters = schema_json()
    ),
)]
impl RadarChart {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "radar-chart", |a: Args| {
            let opts = to_options(&a);
            gizza_ai_radar_chart_core::render(&a.data, &opts).map_err(SkillError::InvalidArgs)
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
        assert_eq!(props.len(), 26, "parameter count changed");
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
        let a: Args = serde_json::from_str(r#"{"data":"A,1\nB,2\nC,3"}"#).unwrap();
        let o = to_options(&a);
        assert_eq!(o.layout, "auto");
        assert_eq!(o.scale, "shared");
        assert_eq!(o.scale_min, 0.0);
        assert_eq!(o.scale_max, 0.0);
        assert_eq!(o.rings, 5);
        assert_eq!(o.grid_shape, "polygon");
        assert!(o.show_spokes);
        assert!(o.show_axis_labels);
        assert!(o.show_ticks);
        assert!(!o.show_values);
        assert_eq!(o.fill_opacity, 0.25);
        assert_eq!(o.line_width, 2.0);
        assert_eq!(o.point_radius, 3.0);
        assert_eq!(o.start_angle, 0.0);
        assert_eq!(o.direction, "clockwise");
        assert_eq!(o.palette, "default");
        assert_eq!(o.colors, "");
        assert_eq!(o.background, "");
        assert_eq!(o.title, "");
        assert!(o.legend);
        assert_eq!(o.font_size, 13.0);
        assert_eq!(o.width, 700);
        assert_eq!(o.height, 560);
        assert_eq!(o.theme, "light");
        assert_eq!(o.output, "svg");
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
                    "description": "The table to plot, one row per line. Wide form is a header row of axis names then one row of numbers per series, for example `product,Camera,Battery,Speed` then `Phone A,8,7,9`. Long form is `series,axis,value` triples. A plain `axis,value` list draws one series. Comma, tab and semicolon separated rows all work, a space-separated row is read as `axis value`, `#` starts a comment, and values may carry thousands separators, a currency mark, or a trailing %. At least 3 axes are required. Example: `product,Camera,Battery,Speed,Price`\\n`Phone A,8,7,9,6`\\n`Phone B,6,9,7,8`."
                },
                "layout": {
                    "type": "string",
                    "enum": ["auto", "wide", "long", "single"],
                    "default": "auto",
                    "description": "How to read the pasted table: auto (default) detects the shape, wide is a header row of axis names plus one numeric row per series, long is `series,axis,value` triples, single is a two-column `axis,value` list drawn as one series."
                },
                "scale": {
                    "type": "string",
                    "enum": ["shared", "per_axis", "percent"],
                    "default": "shared",
                    "description": "How values map to the radius: shared (default) puts every axis on one scale so magnitudes stay comparable, per_axis rescales each axis to its own maximum so mixed units such as revenue and a 1-5 rating can share a chart, percent pins the domain to 0-100 for scores that are already percentages."
                },
                "scale_min": {
                    "type": "number",
                    "minimum": -1000000000,
                    "maximum": 1000000000,
                    "default": 0.0,
                    "description": "The value at the centre of the chart. Default 0, which is what makes radar areas honest; raise it to zoom into a narrow band such as 60-100. Values below it are drawn at the centre."
                },
                "scale_max": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 1000000000,
                    "default": 0.0,
                    "description": "The value at the outer ring. Default 0 means derive it from the data and round up to a clean number such as 10, 25, or 500. Ignored when scale is per_axis (each axis derives its own maximum)."
                },
                "rings": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 10,
                    "default": 5,
                    "description": "Number of concentric grid rings, 0-10. Default 5. Use 0 for just the outer boundary and no tick labels; 4-5 reads best."
                },
                "grid_shape": {
                    "type": "string",
                    "enum": ["polygon", "circle"],
                    "default": "polygon",
                    "description": "Grid style: polygon (default) draws straight-edged web rings through the axis points, circle draws smooth concentric circles."
                },
                "show_spokes": {
                    "type": "boolean",
                    "default": true,
                    "description": "Draw the radial line from the centre out to each axis. Default true; turn it off for a cleaner web when you have many axes."
                },
                "show_axis_labels": {
                    "type": "boolean",
                    "default": true,
                    "description": "Draw each axis name around the outside of the chart. Default true. Turning it off frees the reserved margin and grows the plotted radius."
                },
                "show_ticks": {
                    "type": "boolean",
                    "default": true,
                    "description": "Draw the scale value next to each grid ring. Default true. Shows percentages instead of values when scale is per_axis, since each axis then has its own maximum."
                },
                "show_values": {
                    "type": "boolean",
                    "default": false,
                    "description": "Print each series' number just outside its vertex on every axis. Default false; useful for 1-2 series, crowded beyond that."
                },
                "fill_opacity": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 1,
                    "default": 0.25,
                    "description": "Opacity of each series' filled polygon, 0-1. Default 0.25, which keeps 2-4 overlapping shapes readable. Use 0 for outline-only charts."
                },
                "line_width": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 8,
                    "default": 2.0,
                    "description": "Stroke width of each series outline in pixels, 0-8. Default 2. Use 0 with a higher fill_opacity for a flat filled look."
                },
                "point_radius": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 12,
                    "default": 3.0,
                    "description": "Radius of the marker drawn at each vertex in pixels, 0-12. Default 3; use 0 to hide markers. Markers carry a hover tooltip with the series, axis, and exact value."
                },
                "start_angle": {
                    "type": "number",
                    "minimum": -360,
                    "maximum": 360,
                    "default": 0.0,
                    "description": "Rotate the whole chart, in degrees, -360 to 360. Default 0 puts the first axis straight up; 45 rotates it an eighth-turn clockwise."
                },
                "direction": {
                    "type": "string",
                    "enum": ["clockwise", "counterclockwise"],
                    "default": "clockwise",
                    "description": "Order the axes are placed around the circle from the starting angle: clockwise (default) or counterclockwise."
                },
                "palette": {
                    "type": "string",
                    "enum": ["default", "pastel", "dusk", "earth", "ocean", "mono"],
                    "default": "default",
                    "description": "Series colour scheme: default (bright, high contrast), pastel, dusk (deep), earth, ocean, or mono (one hue lightened per series, based on the first entry of colors). Overridden per series by colors."
                },
                "colors": {
                    "type": "string",
                    "default": "",
                    "description": "Comma-separated colour overrides, one per series and cycled if short, for example `#2563eb,#f97316` or `tomato,steelblue`. Default empty uses the palette. With palette=mono the first entry is the base hue."
                },
                "background": {
                    "type": "string",
                    "default": "",
                    "description": "Chart background colour, for example #ffffff or transparent. Default empty uses the theme background (white for light, dark slate for dark)."
                },
                "title": {
                    "type": "string",
                    "default": "",
                    "description": "Optional chart title centred above the web, for example `Phone comparison`."
                },
                "legend": {
                    "type": "boolean",
                    "default": true,
                    "description": "Draw a legend strip under the chart naming each series with its colour swatch. Default true because radar charts are comparison charts."
                },
                "font_size": {
                    "type": "number",
                    "minimum": 6,
                    "maximum": 48,
                    "default": 13.0,
                    "description": "Base font size in pixels for axis captions and the legend, 6-48. Default 13. Tick and value labels are drawn slightly smaller."
                },
                "width": {
                    "type": "integer",
                    "minimum": 320,
                    "maximum": 2400,
                    "default": 700,
                    "description": "SVG width in pixels, 320-2400. Default 700."
                },
                "height": {
                    "type": "integer",
                    "minimum": 240,
                    "maximum": 1800,
                    "default": 560,
                    "description": "SVG height in pixels, 240-1800. Default 560. Radar charts read best close to square."
                },
                "theme": {
                    "type": "string",
                    "enum": ["light", "dark"],
                    "default": "light",
                    "description": "Chart theme: light (default) or dark. Sets the background, grid, and text colours; override the background alone with the background parameter."
                },
                "output": {
                    "type": "string",
                    "enum": ["svg", "summary", "json"],
                    "default": "svg",
                    "description": "Output format: svg (default, self-contained markup), summary (aligned text table of every series and axis with its value and scaled percentage, plus per-series means), or json (machine-readable axis domains, angles, and vertex coordinates)."
                }
            }
        });
        assert_eq!(
            actual, authored,
            "descriptor drifted from the authored schema"
        );
    }
}
