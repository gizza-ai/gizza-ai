//! gizza-ai/mars-spline-regression — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_mars_spline_regression_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

fn d_target() -> String { "last".into() }
fn d_max_terms() -> u32 { 21 }
fn d_max_degree() -> u32 { 1 }
fn d_penalty() -> f64 { 3.0 }
fn d_true() -> bool { true }
fn d_zero() -> u32 { 0 }
fn d_minspan() -> u32 { 0 }
fn d_endspan() -> u32 { 0 }
fn d_thresh() -> f64 { 0.001 }
fn d_header() -> String { "auto".into() }
fn d_decimals() -> u32 { 4 }
fn d_format() -> String { "text".into() }

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "d_target")]
    target: String,
    #[serde(default)]
    features: String,
    #[serde(default = "d_max_terms")]
    max_terms: u32,
    #[serde(default = "d_max_degree")]
    max_degree: u32,
    #[serde(default = "d_penalty")]
    penalty: f64,
    #[serde(default = "d_true")]
    prune: bool,
    #[serde(default = "d_zero")]
    nprune: u32,
    #[serde(default = "d_minspan")]
    minspan: u32,
    #[serde(default = "d_endspan")]
    endspan: u32,
    #[serde(default = "d_thresh")]
    thresh: f64,
    #[serde(default = "d_true")]
    allow_linear: bool,
    #[serde(default)]
    predict: String,
    #[serde(default = "d_header")]
    header: String,
    #[serde(default = "d_decimals")]
    decimals: u32,
    #[serde(default = "d_format")]
    format: String,
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            target: a.target,
            features: a.features,
            max_terms: a.max_terms,
            max_degree: a.max_degree,
            penalty: a.penalty,
            prune: a.prune,
            nprune: a.nprune,
            minspan: a.minspan,
            endspan: a.endspan,
            thresh: a.thresh,
            allow_linear: a.allow_linear,
            predict: a.predict,
            header: a.header,
            decimals: a.decimals,
            format: a.format,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().multiline().describe("Numeric CSV, TSV, semicolon, pipe, or whitespace table. Header row is auto-detected by default. The target column defaults to the last column."))
        .param(Param::string("target").default("last").describe("Target column to predict: a header name, 1-based column number, or 'last'. Default last."))
        .param(Param::string("features").describe("Optional comma-separated feature columns by name or 1-based number. Blank means all columns except the target."))
        .param(Param::integer("max_terms").default(21).min(3.0).max(101.0).describe("Maximum number of basis terms grown during the forward pass. Default 21."))
        .param(Param::integer("max_degree").default(1).min(1.0).max(3.0).describe("Maximum interaction degree for hinge terms. 1 = additive piecewise-linear model; 2 or 3 allow interactions. Default 1."))
        .param(Param::number("penalty").default(3.0).min(0.0).max(10.0).describe("GCV complexity penalty per term, similar to the earth package penalty. Default 3.0."))
        .param(Param::boolean("prune").default(true).describe("Run the backward pruning pass and keep the sub-model with the best GCV score. Default true."))
        .param(Param::integer("nprune").default(0).min(0.0).max(101.0).describe("Optional cap on kept terms after pruning. 0 lets GCV choose. Default 0."))
        .param(Param::integer("minspan").default(0).min(0.0).max(1000.0).describe("Minimum number of rows between candidate knots. 0 chooses an automatic span. Default 0."))
        .param(Param::integer("endspan").default(0).min(0.0).max(1000.0).describe("Rows to protect at each edge from becoming knots. 0 chooses an automatic span. Default 0."))
        .param(Param::number("thresh").default(0.001).min(0.0).max(1.0).describe("Minimum relative RSS improvement needed to add another pair of hinge terms. Default 0.001."))
        .param(Param::boolean("allow_linear").default(true).describe("Allow plain linear terms as candidates as well as hinge pairs. Default true."))
        .param(Param::string("predict").multiline().describe("Optional new feature rows to score, using the same delimiter and feature order as the training data. Do not include the target column."))
        .param(Param::enumv("header", ["auto", "yes", "no"]).default("auto").describe("Whether the training data has a header row. Default auto."))
        .param(Param::integer("decimals").default(4).min(0.0).max(12.0).describe("Decimal places for text and CSV output. Default 4."))
        .param(Param::enumv("format", ["text", "csv", "json"]).default("text").describe("Output format: text report, CSV tables, or JSON model object. Default text."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mars-spline-regression",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fit a MARS hinge-spline regression model to numeric tables",
    skill(
        description = "Fit a deterministic Multivariate Adaptive Regression Splines (MARS) model to pasted numeric data. The tool grows hinge functions, prunes them by GCV, reports the explicit piecewise-linear equation, fit statistics, variable importance, knots, terms, and optional predictions for new rows. Inputs are local CSV/TSV/semicolon/pipe/whitespace tables; no network or machine-learning runtime is used.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mars-spline-regression", |a: Args| {
            let data = a.data.clone();
            let opts: Options = a.into();
            gizza_ai_mars_spline_regression_core::run(&data, &opts).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
