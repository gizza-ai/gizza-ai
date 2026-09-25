//! gizza-ai/treemap-chart — render hierarchical treemaps as deterministic SVG.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_treemap_chart_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_layout")]
    layout: String,
    #[serde(default = "default_path_separator")]
    path_separator: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_tiling")]
    tiling: String,
    #[serde(default)]
    max_depth: u32,
    #[serde(default)]
    top_n: u32,
    #[serde(default = "default_true")]
    show_labels: bool,
    #[serde(default = "default_true")]
    show_values: bool,
    #[serde(default)]
    show_percent: bool,
    #[serde(default = "default_label_position")]
    label_position: String,
    #[serde(default = "default_font_size")]
    font_size: f64,
    #[serde(default = "default_palette")]
    palette: String,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default)]
    background: String,
    #[serde(default = "default_border_width")]
    border_width: f64,
    #[serde(default = "default_corner_radius")]
    corner_radius: f64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    legend: bool,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_layout() -> String { "auto".into() }
fn default_path_separator() -> String { "/".into() }
fn default_sort() -> String { "value_desc".into() }
fn default_tiling() -> String { "squarified".into() }
fn default_true() -> bool { true }
fn default_label_position() -> String { "top".into() }
fn default_font_size() -> f64 { 13.0 }
fn default_palette() -> String { "default".into() }
fn default_color() -> String { "#2563eb".into() }
fn default_border_width() -> f64 { 2.0 }
fn default_corner_radius() -> f64 { 2.0 }
fn default_width() -> u32 { 800 }
fn default_height() -> u32 { 500 }
fn default_theme() -> String { "light".into() }
fn default_output() -> String { "svg".into() }

fn to_options(a: &Args) -> Options {
    Options {
        layout: a.layout.clone(),
        path_separator: a.path_separator.clone(),
        sort: a.sort.clone(),
        tiling: a.tiling.clone(),
        max_depth: a.max_depth,
        top_n: a.top_n,
        show_labels: a.show_labels,
        show_values: a.show_values,
        show_percent: a.show_percent,
        label_position: a.label_position.clone(),
        font_size: a.font_size,
        palette: a.palette.clone(),
        color: a.color.clone(),
        background: a.background.clone(),
        border_width: a.border_width,
        corner_radius: a.corner_radius,
        title: a.title.clone(),
        legend: a.legend,
        width: a.width,
        height: a.height,
        theme: a.theme.clone(),
        output: a.output.clone(),
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("Rows to plot, one per line. Use `label,value` for a flat treemap, `parent/child,value` for hierarchical paths, or `group,label,value` for a grouped table; the last column is always the number. Comma, tab, semicolon, and single-space separated rows all work, a leading header row is skipped, `#` starts a comment, and values may carry thousands separators or a currency mark. Required."))
        .param(Param::enumv("layout", ["auto", "flat", "path", "grouped"]).default("auto").describe("How to read each row: auto (default) picks grouped for 3+ columns, path when the first column contains the path separator, else flat; flat forces one level of `label,value`; path splits the first column on path_separator; grouped treats every column except the last as a nesting level."))
        .param(Param::string("path_separator").default("/").describe("Separator that splits hierarchy levels when layout is path or auto, for example `/` (default), `>`, or `::`. Ignored for flat and grouped layouts."))
        .param(Param::enumv("sort", ["value_desc", "value_asc", "input", "label"]).default("value_desc").describe("Order tiles within each parent: value_desc (default, largest first, best for squarified layout), value_asc, input (order first seen in the data), or label (alphabetical)."))
        .param(Param::enumv("tiling", ["squarified", "slice_dice", "binary"]).default("squarified").describe("Tiling algorithm: squarified (default, near-square tiles that are easiest to compare), slice_dice (strict stripes that alternate direction per level and preserve order), or binary (recursive halving, a middle ground)."))
        .param(Param::integer("max_depth").min(0.0).max(12.0).default(0).describe("Maximum hierarchy levels to keep, 0-12. Deeper paths are aggregated into their ancestor at this depth. Default 0 keeps every level (hard cap 12)."))
        .param(Param::integer("top_n").min(0.0).max(500.0).default(0).describe("Keep only the largest N children at each level and fold the remainder into a single `Other` tile, 0-500. Default 0 keeps everything; 10-15 is a readable tile count."))
        .param(Param::boolean("show_labels").default(true).describe("Draw the name of each tile and group when it fits. Default true."))
        .param(Param::boolean("show_values").default(true).describe("Draw each tile's numeric value under its name when it fits. Default true."))
        .param(Param::boolean("show_percent").default(false).describe("Draw each tile's share of the grand total as a percentage. Default false; combines with show_values as `1,250 (23.4%)`."))
        .param(Param::enumv("label_position", ["top", "center", "bottom"]).default("top").describe("Vertical placement of tile text: top (default, left aligned), center (also horizontally centred), or bottom."))
        .param(Param::number("font_size").min(6.0).max(48.0).default(13.0).describe("Label font size in pixels, 6-48. Default 13. Larger values hide labels on small tiles because text is only drawn when it fits."))
        .param(Param::enumv("palette", ["default", "pastel", "dusk", "earth", "ocean", "mono"]).default("default").describe("Tile colour scheme: default (bright), pastel, dusk (deep), earth, ocean, or mono (single-hue ramp built from the color parameter, darkest tile = largest value). Non-mono palettes give each top-level branch its own hue and lighten nested children."))
        .param(Param::string("color").default("#2563eb").describe("Base colour for palette=mono, as CSS colour text such as #2563eb, #f00, or tomato. Default #2563eb. Named colours are used as-is without the lightness ramp."))
        .param(Param::string("background").default("").describe("Chart background colour, for example #ffffff or transparent. Default empty uses the theme background (white for light, dark slate for dark)."))
        .param(Param::number("border_width").min(0.0).max(12.0).default(2.0).describe("Gap/stroke width between tiles in pixels, 0-12. Default 2; use 0 for a seamless mosaic."))
        .param(Param::number("corner_radius").min(0.0).max(24.0).default(2.0).describe("Rounded corner radius of each tile in pixels, 0-24. Default 2; use 0 for square corners."))
        .param(Param::string("title").default("").describe("Optional chart title drawn above the tiles, for example `Storage by folder`."))
        .param(Param::boolean("legend").default(false).describe("Draw a legend strip under the chart naming each top-level branch with its share. Default false."))
        .param(Param::integer("width").min(320.0).max(2400.0).default(800).describe("SVG width in pixels, 320-2400. Default 800."))
        .param(Param::integer("height").min(240.0).max(1800.0).default(500).describe("SVG height in pixels, 240-1800. Default 500."))
        .param(Param::enumv("theme", ["light", "dark"]).default("light").describe("Chart theme: light (default) or dark. Sets the background and text colours; override the background alone with the background parameter."))
        .param(Param::enumv("output", ["svg", "summary", "json"]).default("svg").describe("Output format: svg (default, self-contained markup), summary (aligned text table of every tile with value, share, and depth), or json (machine-readable nodes with values, shares, and tile rectangles)."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct TreemapChart;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/treemap-chart",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render hierarchical treemaps sized by value as deterministic SVG",
    skill(
        description = "Render a hierarchical treemap from pasted rows: `label,value` for a flat chart, `parent/child,value` for paths, or `group,label,value` for a grouped table. Aggregates duplicate paths, sorts tiles, tiles the canvas with a squarified, slice-and-dice, or binary algorithm, and returns self-contained SVG by default or a summary table / JSON node list with values, shares, and tile rectangles. Options control layout detection, path separator, sort order, tiling algorithm, depth cap, top-N grouping into an Other tile, label/value/percentage display, label position, font size, colour palette, mono base colour, background, tile gap, corner radius, title, legend, canvas size, and theme. Everything runs locally in pure Rust; no plotting service, fonts, or external data are used.",
        parameters = schema_json()
    ),
)]
impl TreemapChart {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "treemap-chart", |a: Args| {
            let opts = to_options(&a);
            gizza_ai_treemap_chart_core::render(&a.data, &opts).map_err(SkillError::InvalidArgs)
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
        assert_eq!(props.len(), 23, "parameter count changed");
        for (name, spec) in props {
            let desc = spec["description"].as_str().unwrap_or("");
            assert!(desc.len() > 20, "param {name} needs a real description");
        }
    }

    #[test]
    fn args_defaults_match_the_descriptor() {
        let a: Args = serde_json::from_str(r#"{"data":"A,1"}"#).unwrap();
        let o = to_options(&a);
        assert_eq!(o.layout, "auto");
        assert_eq!(o.path_separator, "/");
        assert_eq!(o.sort, "value_desc");
        assert_eq!(o.tiling, "squarified");
        assert_eq!(o.max_depth, 0);
        assert_eq!(o.top_n, 0);
        assert!(o.show_labels);
        assert!(o.show_values);
        assert!(!o.show_percent);
        assert_eq!(o.label_position, "top");
        assert_eq!(o.font_size, 13.0);
        assert_eq!(o.palette, "default");
        assert_eq!(o.color, "#2563eb");
        assert_eq!(o.background, "");
        assert_eq!(o.border_width, 2.0);
        assert_eq!(o.corner_radius, 2.0);
        assert_eq!(o.title, "");
        assert!(!o.legend);
        assert_eq!(o.width, 800);
        assert_eq!(o.height, 500);
        assert_eq!(o.theme, "light");
        assert_eq!(o.output, "svg");
    }
}
