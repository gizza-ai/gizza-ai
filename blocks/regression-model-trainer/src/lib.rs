//! gizza-ai/regression-model-trainer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_regression_model_trainer_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
#[serde(default)]
struct Args {
    data: String,
    target: String,
    features: String,
    model: String,
    alpha: f64,
    standardize: bool,
    trees: u32,
    max_depth: u32,
    test_split: f64,
    cv_folds: u32,
    seed: u64,
    header: String,
    decimals: u32,
    format: String,
}

impl Default for Args {
    fn default() -> Self {
        let o = Options::default();
        Args {
            data: String::new(),
            target: o.target,
            features: o.features,
            model: o.model,
            alpha: o.alpha,
            standardize: o.standardize,
            trees: o.trees,
            max_depth: o.max_depth,
            test_split: o.test_split,
            cv_folds: o.cv_folds,
            seed: o.seed,
            header: o.header,
            decimals: o.decimals,
            format: o.format,
        }
    }
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            target: a.target,
            features: a.features,
            model: a.model,
            alpha: a.alpha,
            standardize: a.standardize,
            trees: a.trees,
            max_depth: a.max_depth,
            test_split: a.test_split,
            cv_folds: a.cv_folds,
            seed: a.seed,
            header: a.header,
            decimals: a.decimals,
            format: a.format,
        }
    }
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("CSV, TSV, semicolon, pipe, or whitespace-delimited numeric table. Use one row per observation; the first row may be a header."))
        .param(Param::string("target").default("last").describe("Target column to predict: last, first, a 1-based index, or a header name."))
        .param(Param::string("features").default("").describe("Optional comma-separated feature columns by name or 1-based index. Leave empty to use every non-target column."))
        .param(Param::enumv("model", ["linear", "ridge", "random_forest"]).default("linear").describe("Regression algorithm: ordinary least squares, L2-regularized ridge, or a deterministic small random forest."))
        .param(Param::number("alpha").default(1.0).min(0.0).max(1000000000.0).describe("Ridge L2 penalty strength. Used only when model is ridge."))
        .param(Param::boolean("standardize").default(true).describe("Standardize predictors before applying ridge regularization, then report coefficients back in original units."))
        .param(Param::integer("trees").default(100).min(1.0).max(300.0).describe("Number of trees for random_forest."))
        .param(Param::integer("max_depth").default(8).min(1.0).max(20.0).describe("Maximum depth for each random_forest tree."))
        .param(Param::number("test_split").default(0.0).min(0.0).max(0.5).describe("Fraction of rows to hold out for a deterministic test set, from 0 to 0.5."))
        .param(Param::integer("cv_folds").default(0).min(0.0).max(10.0).describe("Cross-validation folds: 0 disables CV, otherwise use 2 through 10."))
        .param(Param::integer("seed").default(42).min(0.0).describe("Deterministic seed for splits, folds, and random forest sampling."))
        .param(Param::enumv("header", ["auto", "yes", "no"]).default("auto").describe("Whether the first row contains column names."))
        .param(Param::integer("decimals").default(4).min(0.0).max(12.0).describe("Decimal places for text and CSV output."))
        .param(Param::enumv("format", ["text", "json", "csv"]).default("text").describe("Output format."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regression-model-trainer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Train linear, ridge, or small random-forest regressions from a pasted numeric table.",
    skill(
        description = "Fit a regression model to a pasted numeric table, predict a target column, and report R², RMSE, MAE, coefficients or feature importance, optional test split, and optional cross-validation. Runs locally in pure Rust/WASM.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "regression-model-trainer", |a: Args| {
            let data = a.data.clone();
            let opts: Options = a.into();
            gizza_ai_regression_model_trainer_core::run(&data, &opts)
                .map_err(SkillError::InvalidArgs)
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
    fn schema_json_has_expected_parameters() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["type"], "object");
        assert_eq!(v["required"], serde_json::json!(["data"]));
        let props = v["properties"].as_object().unwrap();
        for name in [
            "data",
            "target",
            "features",
            "model",
            "alpha",
            "standardize",
            "trees",
            "max_depth",
            "test_split",
            "cv_folds",
            "seed",
            "header",
            "decimals",
            "format",
        ] {
            assert!(props.contains_key(name), "missing {name} in schema");
            assert!(
                props[name].get("description").is_some(),
                "missing description for {name}"
            );
        }
        assert_eq!(
            props["model"]["enum"],
            serde_json::json!(["linear", "ridge", "random_forest"])
        );
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["text", "json", "csv"])
        );
        assert_eq!(
            props["header"]["enum"],
            serde_json::json!(["auto", "yes", "no"])
        );
        assert_eq!(props["trees"]["maximum"], 300);
        assert_eq!(props["test_split"]["maximum"], 0.5);
        assert_eq!(v["additionalProperties"], false);
    }
}
