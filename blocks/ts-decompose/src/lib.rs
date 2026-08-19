//! gizza-ai/ts-decompose — split a time series into trend, seasonal, and residual parts.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ts_decompose_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_model")]
    model: String,
    #[serde(default)]
    period: u32,
    #[serde(default)]
    seasonal_window: u32,
    #[serde(default)]
    trend_window: u32,
    #[serde(default)]
    robust: bool,
    #[serde(default = "default_true")]
    two_sided: bool,
    #[serde(default = "default_true")]
    extrapolate_trend: bool,
    #[serde(default = "default_true")]
    trend_overlay: bool,
    #[serde(default)]
    show_adjusted: bool,
    #[serde(default = "default_residual_style")]
    residual_style: String,
    #[serde(default = "default_true")]
    grid: bool,
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
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_precision")]
    precision: u32,
    #[serde(default = "default_output")]
    output: String,
}

fn default_method() -> String { "stl".into() }
fn default_model() -> String { "additive".into() }
fn default_true() -> bool { true }
fn default_residual_style() -> String { "bar".into() }
fn default_width() -> u32 { 900 }
fn default_height() -> u32 { 720 }
fn default_color() -> String { "#2563eb".into() }
fn default_theme() -> String { "light".into() }
fn default_precision() -> u32 { 4 }
fn default_output() -> String { "svg".into() }

fn to_options(a: &Args) -> Options {
    Options {
        method: a.method.clone(),
        model: a.model.clone(),
        period: a.period,
        seasonal_window: a.seasonal_window,
        trend_window: a.trend_window,
        robust: a.robust,
        two_sided: a.two_sided,
        extrapolate_trend: a.extrapolate_trend,
        trend_overlay: a.trend_overlay,
        show_adjusted: a.show_adjusted,
        residual_style: a.residual_style.clone(),
        grid: a.grid,
        title: a.title.clone(),
        x_label: a.x_label.clone(),
        y_label: a.y_label.clone(),
        width: a.width,
        height: a.height,
        color: a.color.clone(),
        theme: a.theme.clone(),
        precision: a.precision,
        output: a.output.clone(),
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The time series, in observation order and evenly spaced. Paste one value per line, or `label,value` rows such as `2024-01,112` where the label becomes the x-axis tick text, or a single comma/space separated line. A leading all-text header row is ignored. Needs at least 4 values, at most 10000, and at least 2 full seasonal cycles. Required."))
        .param(Param::enumv("method", ["stl", "classical"]).default("stl").describe("Decomposition engine. stl (default) is seasonal-trend decomposition by loess: the seasonal shape may evolve and outliers can be down-weighted. classical uses a centred moving-average trend and one fixed seasonal index per cycle position, the textbook method."))
        .param(Param::enumv("model", ["additive", "multiplicative"]).default("additive").describe("How the components combine. additive (default) gives observed = trend + seasonal + residual, for seasonal swings of a roughly constant size. multiplicative gives observed = trend x seasonal x residual, for swings that grow with the level; it decomposes the logarithms and needs every value to be greater than 0."))
        .param(Param::integer("period").min(0.0).max(1000.0).default(0).describe("Number of observations in one seasonal cycle: 12 for monthly data with a yearly cycle, 4 for quarterly, 7 for daily data with a weekly cycle, 24 for hourly data with a daily cycle. Default 0 detects the period from the strongest autocorrelation peak; set it explicitly when you know it."))
        .param(Param::integer("seasonal_window").min(0.0).max(1000.0).default(0).describe("STL only: length of the seasonal smoother, in observations. Default 0 is the periodic smoother, which holds one fixed seasonal shape for the whole series. A number (odd; an even one is nudged up by 1) lets the shape drift — smaller means it changes faster, 7 is the usual starting point."))
        .param(Param::integer("trend_window").min(0.0).max(4000.0).default(0).describe("STL only: length of the trend smoother, in observations, always odd. Default 0 uses the standard rule of about 1.5 times the period. Larger values give a smoother, stiffer trend that pushes more variation into the residual."))
        .param(Param::boolean("robust").default(false).describe("STL only: run the robust outer loop, which down-weights outliers so one bad reading lands in the residual instead of bending the trend and seasonal components. Slower. Default false."))
        .param(Param::boolean("two_sided").default(true).describe("Classical only: use a centred moving average (default true), which is symmetric but leaves half a cycle undefined at each end. Set false for a trailing moving average that only uses past values, so the trend reaches the final observation but lags it."))
        .param(Param::boolean("extrapolate_trend").default(true).describe("Classical only: fill the moving average's blank ends by extending a least-squares line fitted to the nearest cycle of trend values, so the chart and table have no gaps. Default true; set false to leave those points empty, as statistics packages do by default."))
        .param(Param::boolean("trend_overlay").default(true).describe("Draw the trend as a dashed line over the observed series in the top panel, as well as in its own panel. Default true."))
        .param(Param::boolean("show_adjusted").default(false).describe("Overlay the seasonally adjusted series (observed with the seasonal component removed) on the top panel. It is always reported as a column in the table, csv, and json output. Default false."))
        .param(Param::enumv("residual_style", ["bar", "line"]).default("bar").describe("How the residual panel is drawn: bar (default) draws one bar per observation from the zero line, which makes individual outliers obvious; line draws a connected line."))
        .param(Param::boolean("grid").default(true).describe("Draw horizontal grid lines in every panel. Default true."))
        .param(Param::string("title").default("").describe("Optional chart title drawn above the panels, for example `Monthly sales`."))
        .param(Param::string("x_label").default("").describe("Optional label for the time axis under the bottom panel, for example `Month`. Default empty draws no label."))
        .param(Param::string("y_label").default("").describe("Optional label for the value axis, drawn rotated on the left, for example `Units sold`. Default empty shows `Value`."))
        .param(Param::integer("width").min(360.0).max(2400.0).default(900).describe("SVG width in pixels, 360-2400. Default 900."))
        .param(Param::integer("height").min(320.0).max(2400.0).default(720).describe("SVG height in pixels, 320-2400, split across the four stacked panels. Default 720."))
        .param(Param::string("color").default("#2563eb").describe("Series colour as CSS color text, for example #2563eb, #f00, or teal. Used for all four panels; the trend overlay uses a contrasting accent. Default #2563eb."))
        .param(Param::enumv("theme", ["light", "dark"]).default("light").describe("Chart theme: light (default, white background) or dark (slate background)."))
        .param(Param::integer("precision").min(0.0).max(12.0).default(4).describe("Decimal places used for component values in the table, csv, and json output, 0-12. Default 4."))
        .param(Param::enumv("output", ["svg", "table", "csv", "json"]).default("svg").describe("Output format: svg (default, the four-panel chart markup), table (aligned components table plus seasonal indices and strength diagnostics), csv (one row per observation), or json (diagnostics plus every component value)."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct TsDecompose;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ts-decompose",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a time series into trend, seasonal, and residual components and plot them",
    skill(
        description = "Decompose an evenly spaced time series into trend, seasonal, and residual components and render them as a four-panel SVG (observed, trend, seasonal, residual). Two engines: STL, seasonal-trend decomposition by loess, where the seasonal shape can evolve and a robust mode down-weights outliers; and the classical centred-moving-average method with one fixed seasonal index per cycle position, including a trailing-average option and least-squares fill for the blank ends. Both support an additive model (observed = trend + seasonal + residual) and a multiplicative one (observed = trend x seasonal x residual, decomposed in log space, positive values only). The seasonal period can be given directly (12 monthly, 4 quarterly, 7 daily-with-weekly-cycle, 24 hourly-with-daily-cycle) or detected automatically from the autocorrelation. Input is one value per line, `label,value` rows, or a single separated line. Options control the STL seasonal and trend smoother lengths, the trend and seasonally-adjusted overlays, residual bars or line, grid, title, axis labels, size, colour, theme, and decimal precision. Switch output to table, csv, or json for every component value plus the seasonal indices and the strength-of-trend and strength-of-seasonality diagnostics. Everything runs locally in pure Rust; no plotting service or external data is used.",
        parameters = schema_json()
    ),
)]
impl TsDecompose {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ts-decompose", |a: Args| {
            let opts = to_options(&a);
            gizza_ai_ts_decompose_core::render(&a.data, &opts).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The time series, in observation order and evenly spaced. Paste one value per line, or `label,value` rows such as `2024-01,112` where the label becomes the x-axis tick text, or a single comma/space separated line. A leading all-text header row is ignored. Needs at least 4 values, at most 10000, and at least 2 full seasonal cycles. Required." },
                    "method": { "type": "string", "enum": ["stl", "classical"], "default": "stl", "description": "Decomposition engine. stl (default) is seasonal-trend decomposition by loess: the seasonal shape may evolve and outliers can be down-weighted. classical uses a centred moving-average trend and one fixed seasonal index per cycle position, the textbook method." },
                    "model": { "type": "string", "enum": ["additive", "multiplicative"], "default": "additive", "description": "How the components combine. additive (default) gives observed = trend + seasonal + residual, for seasonal swings of a roughly constant size. multiplicative gives observed = trend x seasonal x residual, for swings that grow with the level; it decomposes the logarithms and needs every value to be greater than 0." },
                    "period": { "type": "integer", "default": 0, "minimum": 0, "maximum": 1000, "description": "Number of observations in one seasonal cycle: 12 for monthly data with a yearly cycle, 4 for quarterly, 7 for daily data with a weekly cycle, 24 for hourly data with a daily cycle. Default 0 detects the period from the strongest autocorrelation peak; set it explicitly when you know it." },
                    "seasonal_window": { "type": "integer", "default": 0, "minimum": 0, "maximum": 1000, "description": "STL only: length of the seasonal smoother, in observations. Default 0 is the periodic smoother, which holds one fixed seasonal shape for the whole series. A number (odd; an even one is nudged up by 1) lets the shape drift — smaller means it changes faster, 7 is the usual starting point." },
                    "trend_window": { "type": "integer", "default": 0, "minimum": 0, "maximum": 4000, "description": "STL only: length of the trend smoother, in observations, always odd. Default 0 uses the standard rule of about 1.5 times the period. Larger values give a smoother, stiffer trend that pushes more variation into the residual." },
                    "robust": { "type": "boolean", "default": false, "description": "STL only: run the robust outer loop, which down-weights outliers so one bad reading lands in the residual instead of bending the trend and seasonal components. Slower. Default false." },
                    "two_sided": { "type": "boolean", "default": true, "description": "Classical only: use a centred moving average (default true), which is symmetric but leaves half a cycle undefined at each end. Set false for a trailing moving average that only uses past values, so the trend reaches the final observation but lags it." },
                    "extrapolate_trend": { "type": "boolean", "default": true, "description": "Classical only: fill the moving average's blank ends by extending a least-squares line fitted to the nearest cycle of trend values, so the chart and table have no gaps. Default true; set false to leave those points empty, as statistics packages do by default." },
                    "trend_overlay": { "type": "boolean", "default": true, "description": "Draw the trend as a dashed line over the observed series in the top panel, as well as in its own panel. Default true." },
                    "show_adjusted": { "type": "boolean", "default": false, "description": "Overlay the seasonally adjusted series (observed with the seasonal component removed) on the top panel. It is always reported as a column in the table, csv, and json output. Default false." },
                    "residual_style": { "type": "string", "enum": ["bar", "line"], "default": "bar", "description": "How the residual panel is drawn: bar (default) draws one bar per observation from the zero line, which makes individual outliers obvious; line draws a connected line." },
                    "grid": { "type": "boolean", "default": true, "description": "Draw horizontal grid lines in every panel. Default true." },
                    "title": { "type": "string", "default": "", "description": "Optional chart title drawn above the panels, for example `Monthly sales`." },
                    "x_label": { "type": "string", "default": "", "description": "Optional label for the time axis under the bottom panel, for example `Month`. Default empty draws no label." },
                    "y_label": { "type": "string", "default": "", "description": "Optional label for the value axis, drawn rotated on the left, for example `Units sold`. Default empty shows `Value`." },
                    "width": { "type": "integer", "default": 900, "minimum": 360, "maximum": 2400, "description": "SVG width in pixels, 360-2400. Default 900." },
                    "height": { "type": "integer", "default": 720, "minimum": 320, "maximum": 2400, "description": "SVG height in pixels, 320-2400, split across the four stacked panels. Default 720." },
                    "color": { "type": "string", "default": "#2563eb", "description": "Series colour as CSS color text, for example #2563eb, #f00, or teal. Used for all four panels; the trend overlay uses a contrasting accent. Default #2563eb." },
                    "theme": { "type": "string", "enum": ["light", "dark"], "default": "light", "description": "Chart theme: light (default, white background) or dark (slate background)." },
                    "precision": { "type": "integer", "default": 4, "minimum": 0, "maximum": 12, "description": "Decimal places used for component values in the table, csv, and json output, 0-12. Default 4." },
                    "output": { "type": "string", "enum": ["svg", "table", "csv", "json"], "default": "svg", "description": "Output format: svg (default, the four-panel chart markup), table (aligned components table plus seasonal indices and strength diagnostics), csv (one row per observation), or json (diagnostics plus every component value)." }
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
        assert_eq!(props.len(), 22, "parameter count changed");
        for (name, spec) in props {
            let desc = spec["description"].as_str().unwrap_or("");
            assert!(desc.len() > 20, "param {name} needs a real description");
        }
    }

    #[test]
    fn args_defaults_match_the_descriptor() {
        let a: Args = serde_json::from_str(r#"{"data":"1,2,3,4"}"#).unwrap();
        let o = to_options(&a);
        assert_eq!(o.method, "stl");
        assert_eq!(o.model, "additive");
        assert_eq!(o.period, 0);
        assert_eq!(o.seasonal_window, 0);
        assert_eq!(o.trend_window, 0);
        assert!(!o.robust);
        assert!(o.two_sided);
        assert!(o.extrapolate_trend);
        assert!(o.trend_overlay);
        assert!(!o.show_adjusted);
        assert_eq!(o.residual_style, "bar");
        assert!(o.grid);
        assert_eq!(o.width, 900);
        assert_eq!(o.height, 720);
        assert_eq!(o.color, "#2563eb");
        assert_eq!(o.theme, "light");
        assert_eq!(o.precision, 4);
        assert_eq!(o.output, "svg");
    }

    #[test]
    fn descriptor_defaults_match_the_core_defaults() {
        let a: Args = serde_json::from_str(r#"{"data":"1,2,3,4"}"#).unwrap();
        let from_args = to_options(&a);
        let core_default = Options::default();
        assert_eq!(from_args.method, core_default.method);
        assert_eq!(from_args.model, core_default.model);
        assert_eq!(from_args.period, core_default.period);
        assert_eq!(from_args.seasonal_window, core_default.seasonal_window);
        assert_eq!(from_args.robust, core_default.robust);
        assert_eq!(from_args.two_sided, core_default.two_sided);
        assert_eq!(from_args.extrapolate_trend, core_default.extrapolate_trend);
        assert_eq!(from_args.trend_overlay, core_default.trend_overlay);
        assert_eq!(from_args.residual_style, core_default.residual_style);
        assert_eq!(from_args.grid, core_default.grid);
        assert_eq!(from_args.width, core_default.width);
        assert_eq!(from_args.height, core_default.height);
        assert_eq!(from_args.color, core_default.color);
        assert_eq!(from_args.theme, core_default.theme);
        assert_eq!(from_args.precision, core_default.precision);
        assert_eq!(from_args.output, core_default.output);
    }

    #[test]
    fn the_block_renders_a_real_decomposition() {
        let season = [4.0, 2.0, -1.0, -3.0, -5.0, -2.0, 1.0, 3.0, 6.0, 2.0, -3.0, -4.0];
        let data = (0..48)
            .map(|i| format!("{}", 100.0 + 0.5 * i as f64 + season[i % 12]))
            .collect::<Vec<_>>()
            .join("\n");
        let a: Args =
            serde_json::from_str(&format!(r#"{{"data":{},"period":12,"output":"json"}}"#, serde_json::to_string(&data).unwrap()))
                .unwrap();
        let out = gizza_ai_ts_decompose_core::render(&a.data, &to_options(&a)).unwrap();
        assert!(out.contains("\"period\":12"));
        assert!(out.contains("\"method\":\"stl\""));
    }
}
