//! gizza-ai/spline-smoother — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page query-params); handle() delegates to block_utils::run_skill. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_spline_smoother_core::{smooth, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// Numeric series: one y per row, x,y rows, JSON numbers, JSON [x,y] pairs, or JSON {x,y} objects.
    input: String,
    /// How the smoothing level is selected: auto, smoothing, lambda, or df.
    #[serde(default = "default_mode")]
    mode: String,
    /// Scale-free p in [0,1] for mode=smoothing; 0 is a straight line, 1 interpolates.
    #[serde(default = "default_smoothing")]
    smoothing: f64,
    /// Raw penalty λ for mode=lambda.
    #[serde(default = "default_lambda")]
    lambda: f64,
    /// Target effective degrees of freedom for mode=df.
    #[serde(default = "default_df")]
    df: f64,
    /// Automatic selection score: generalized cross-validation or leave-one-out CV.
    #[serde(default = "default_criterion")]
    criterion: String,
    /// Optional per-observation positive weights.
    #[serde(default)]
    weights: String,
    /// Optional x values to predict at.
    #[serde(default)]
    predict_at: String,
    /// Optional count for an evenly spaced fitted curve.
    #[serde(default)]
    resample: usize,
    /// Include piecewise cubic coefficients in JSON/CSV output.
    #[serde(default)]
    coefficients: bool,
    /// Output format: json, csv, or svg.
    #[serde(default = "default_output")]
    output: String,
}

fn default_mode() -> String {
    "auto".into()
}
fn default_smoothing() -> f64 {
    0.99
}
fn default_lambda() -> f64 {
    1.0
}
fn default_df() -> f64 {
    5.0
}
fn default_criterion() -> String {
    "gcv".into()
}
fn default_output() -> String {
    "json".into()
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            mode: a.mode,
            smoothing: a.smoothing,
            lambda: a.lambda,
            df: a.df,
            criterion: a.criterion,
            weights: a.weights,
            predict_at: a.predict_at,
            resample: a.resample,
            coefficients: a.coefficients,
            output: a.output,
        }
    }
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Numeric series to smooth. Accepts one y value per row, x,y rows with an optional header, a one-line y list, a JSON array of numbers, JSON [x,y] pairs, or JSON objects with x and y."),
        )
        .param(
            Param::enumv("mode", ["auto", "smoothing", "lambda", "df"])
                .default("auto")
                .describe("How to choose the smoothing penalty: auto selects by criterion, smoothing uses p in [0,1], lambda uses a raw penalty, and df targets effective degrees of freedom."),
        )
        .param(
            Param::number("smoothing")
                .default(0.99)
                .min(0.0)
                .max(1.0)
                .describe("Scale-free smoothing p for mode=smoothing. 0 gives the weighted least-squares straight line; 1 interpolates the distinct input points. Default 0.99."),
        )
        .param(
            Param::number("lambda")
                .default(1.0)
                .min(0.0)
                .describe("Raw non-negative smoothing penalty λ for mode=lambda, in the input x units. Default 1."),
        )
        .param(
            Param::number("df")
                .default(5.0)
                .min(2.0)
                .max(10000.0)
                .describe("Target effective degrees of freedom for mode=df. Must be between 2 and the number of distinct x values. Default 5."),
        )
        .param(
            Param::enumv("criterion", ["gcv", "cv"])
                .default("gcv")
                .describe("Score used by mode=auto: generalized cross-validation (gcv) or exact leave-one-out cross-validation (cv). Default gcv."),
        )
        .param(
            Param::string("weights")
                .default("")
                .describe("Optional positive weights, one per original input point, separated by commas, spaces, tabs, or semicolons. Leave empty for equal weights."),
        )
        .param(
            Param::string("predict_at")
                .default("")
                .describe("Optional x values where the fitted spline should be evaluated, separated by commas, spaces, tabs, or semicolons."),
        )
        .param(
            Param::integer("resample")
                .default(0)
                .min(0.0)
                .max(5000.0)
                .describe("Number of evenly spaced fitted-curve samples to include. 0 disables the curve; values above 0 must be at least 2. Default 0."),
        )
        .param(
            Param::boolean("coefficients")
                .default(false)
                .describe("Include natural-cubic piece coefficients for each interval in JSON/CSV output. Default false."),
        )
        .param(
            Param::enumv("output", ["json", "csv", "svg"])
                .default("json")
                .describe("Output format: JSON report, CSV tables, or a self-contained SVG chart. Default json."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/spline-smoother",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fit cubic smoothing splines to noisy numeric series.",
    skill(
        description = "Fit a cubic smoothing spline to a noisy numeric series. Paste one y value per row, x,y rows, or JSON data; choose automatic GCV/CV smoothing, a p value, a raw lambda, or a target effective degrees of freedom. Returns JSON, CSV, or an SVG chart with fitted values, residuals, optional predictions, resampled curve points, and optional piecewise cubic coefficients.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "spline-smoother", |a: Args| {
            let input = a.input.clone();
            let opts: Options = a.into();
            smooth(&input, &opts).map_err(SkillError::InvalidArgs)
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
    fn schema_json_exposes_authored_controls() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["required"], serde_json::json!(["input"]));
        let p = &v["properties"];
        assert_eq!(
            p["mode"]["enum"],
            serde_json::json!(["auto", "smoothing", "lambda", "df"])
        );
        assert_eq!(p["criterion"]["enum"], serde_json::json!(["gcv", "cv"]));
        assert_eq!(
            p["output"]["enum"],
            serde_json::json!(["json", "csv", "svg"])
        );
        assert_eq!(p["smoothing"]["minimum"], 0.0);
        assert_eq!(p["smoothing"]["maximum"], 1.0);
        assert_eq!(p["coefficients"]["default"], false);
        assert!(p["input"]["description"]
            .as_str()
            .unwrap()
            .contains("x,y rows"));
    }
}
