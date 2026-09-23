//! gizza-ai/svm-classifier — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_svm_classifier_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
#[serde(default)]
struct Args {
    data: String,
    target: String,
    features: String,
    kernel: String,
    c: f64,
    gamma: String,
    degree: u32,
    coef0: f64,
    scaling: String,
    class_weight: String,
    multiclass: String,
    tol: f64,
    max_iter: u32,
    cv_folds: u32,
    test_split: f64,
    seed: u64,
    predict: String,
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
            kernel: o.kernel,
            c: o.c,
            gamma: o.gamma,
            degree: o.degree,
            coef0: o.coef0,
            scaling: o.scaling,
            class_weight: o.class_weight,
            multiclass: o.multiclass,
            tol: o.tol,
            max_iter: o.max_iter,
            cv_folds: o.cv_folds,
            test_split: o.test_split,
            seed: o.seed,
            predict: o.predict,
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
            kernel: a.kernel,
            c: a.c,
            gamma: a.gamma,
            degree: a.degree,
            coef0: a.coef0,
            scaling: a.scaling,
            class_weight: a.class_weight,
            multiclass: a.multiclass,
            tol: a.tol,
            max_iter: a.max_iter,
            cv_folds: a.cv_folds,
            test_split: a.test_split,
            seed: a.seed,
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
        .param(Param::string("data").required().describe("CSV, TSV, semicolon, pipe, or whitespace-delimited training table. One row per observation; the first row may be a header. Feature columns may be numeric or categorical (categorical columns are one-hot encoded). Max 2000 rows."))
        .param(Param::string("target").default("last").describe("Class column to predict: last, first, a 1-based index, or a header name."))
        .param(Param::string("features").default("").describe("Optional comma-separated feature columns by name or 1-based index. Leave empty to use every non-target column."))
        .param(Param::enumv("kernel", ["linear", "rbf", "poly", "sigmoid"]).default("rbf").describe("Kernel function. linear = dot product, best when the classes are already separable and you want readable weights. rbf = exp(-gamma*||x-y||^2), the general-purpose default. poly = (gamma*x.y + coef0)^degree. sigmoid = tanh(gamma*x.y + coef0)."))
        .param(Param::number("c").default(1.0).min(0.0001).max(10000.0).describe("Regularization cost, the penalty for a margin violation. Default 1. Small values (0.1) give a wide, smooth margin that tolerates mistakes; large values (100) fit the training rows harder and risk overfitting."))
        .param(Param::string("gamma").default("scale").describe("Kernel coefficient for rbf, poly, and sigmoid. Use scale for 1/(n_features * variance of the scaled data), auto for 1/n_features, or an explicit positive number such as 0.5. Larger gamma means a tighter, more local decision boundary."))
        .param(Param::integer("degree").default(3).min(1.0).max(10.0).describe("Polynomial degree, used only when kernel is poly. Default 3."))
        .param(Param::number("coef0").default(0.0).describe("Independent term added inside the poly and sigmoid kernels. Default 0. Ignored by linear and rbf."))
        .param(Param::enumv("scaling", ["standard", "minmax", "none"]).default("standard").describe("Feature scaling applied before training. standard = z-score each column (recommended; a kernel SVM compares raw distances). minmax = rescale each column to 0-1. none = use the raw values."))
        .param(Param::enumv("class_weight", ["none", "balanced"]).default("none").describe("Weight classes equally (none) or scale each class's cost by n_samples/(n_classes*class_count) (balanced), which helps on imbalanced data."))
        .param(Param::enumv("multiclass", ["ovo", "ovr"]).default("ovo").describe("Strategy for three or more classes: ovo trains one model per class pair and votes; ovr trains one model per class against all the others and takes the highest decision value. Ignored for two classes, which need a single model."))
        .param(Param::number("tol").default(0.001).min(0.0000001).max(1.0).describe("Solver stopping tolerance on the KKT violation. Default 0.001. Smaller is more exact and slower."))
        .param(Param::integer("max_iter").default(100000).min(1.0).max(1000000.0).describe("Hard cap on solver iterations. If a sub-model hits the cap before reaching tol, the report says so rather than presenting a partial fit as final."))
        .param(Param::integer("cv_folds").default(0).min(0.0).max(10.0).describe("Stratified k-fold cross-validation over the training rows: 0 turns it off, otherwise 2 to 10. Reports overall and per-fold accuracy."))
        .param(Param::number("test_split").default(0.0).min(0.0).max(0.5).describe("Fraction of rows held out, stratified by class, for an honest accuracy check. 0 to 0.5."))
        .param(Param::integer("seed").default(42).min(0.0).describe("Deterministic seed for the hold-out shuffle and the cross-validation folds. Solving itself is exact."))
        .param(Param::string("predict").default("").describe("Optional rows to classify with the fitted model: one row per line, either the full table layout, just the feature columns in order, or with a header naming them. Reports the class and the decision value."))
        .param(Param::enumv("header", ["auto", "yes", "no"]).default("auto").describe("Whether the first row contains column names."))
        .param(Param::integer("decimals").default(4).min(0.0).max(12.0).describe("Decimal places for the margin, alphas, weights, and metrics."))
        .param(Param::enumv("format", ["text", "json", "csv"]).default("text").describe("Output format: readable report, JSON, or a flat section/key/value CSV."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/svm-classifier",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Train a support vector machine on a pasted table and read the margin, support vectors, and accuracy.",
    skill(
        description = "Train a C-SVC support vector machine (linear, RBF, polynomial, or sigmoid kernel) on a pasted tabular dataset and report the margin width, the support vectors with their alphas, training accuracy with a confusion matrix and per-class precision/recall/F1, an optional hold-out split and k-fold cross-validation, linear weights, and predictions for new rows. Categorical columns are one-hot encoded and features are scaled by default. Runs locally in pure Rust/WASM.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "svm-classifier", |a: Args| {
            let data = a.data.clone();
            let opts: Options = a.into();
            gizza_ai_svm_classifier_core::run(&data, &opts).map_err(SkillError::InvalidArgs)
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
            "kernel",
            "c",
            "gamma",
            "degree",
            "coef0",
            "scaling",
            "class_weight",
            "multiclass",
            "tol",
            "max_iter",
            "cv_folds",
            "test_split",
            "seed",
            "predict",
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
            props["kernel"]["enum"],
            serde_json::json!(["linear", "rbf", "poly", "sigmoid"])
        );
        assert_eq!(
            props["scaling"]["enum"],
            serde_json::json!(["standard", "minmax", "none"])
        );
        assert_eq!(
            props["class_weight"]["enum"],
            serde_json::json!(["none", "balanced"])
        );
        assert_eq!(props["multiclass"]["enum"], serde_json::json!(["ovo", "ovr"]));
        assert_eq!(
            props["header"]["enum"],
            serde_json::json!(["auto", "yes", "no"])
        );
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["text", "json", "csv"])
        );
        assert_eq!(props["kernel"]["default"], "rbf");
        assert_eq!(props["c"]["default"], 1.0);
        assert_eq!(props["c"]["maximum"], 10000.0);
        assert_eq!(props["gamma"]["default"], "scale");
        assert_eq!(props["degree"]["default"], 3);
        assert_eq!(props["degree"]["maximum"], 10);
        assert_eq!(props["coef0"]["default"], 0.0);
        assert_eq!(props["tol"]["default"], 0.001);
        assert_eq!(props["max_iter"]["default"], 100000);
        assert_eq!(props["cv_folds"]["default"], 0);
        assert_eq!(props["cv_folds"]["maximum"], 10);
        assert_eq!(props["test_split"]["maximum"], 0.5);
        assert_eq!(props["seed"]["default"], 42);
        assert_eq!(props["decimals"]["default"], 4);
        assert_eq!(v["additionalProperties"], false);
    }
}
