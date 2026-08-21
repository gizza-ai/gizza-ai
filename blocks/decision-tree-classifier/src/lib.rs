//! gizza-ai/decision-tree-classifier — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_decision_tree_classifier_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
#[serde(default)]
struct Args {
    data: String,
    target: String,
    features: String,
    criterion: String,
    splits: String,
    max_depth: u32,
    min_samples_split: u32,
    min_samples_leaf: u32,
    min_gain: f64,
    class_weight: String,
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
            criterion: o.criterion,
            splits: o.splits,
            max_depth: o.max_depth,
            min_samples_split: o.min_samples_split,
            min_samples_leaf: o.min_samples_leaf,
            min_gain: o.min_gain,
            class_weight: o.class_weight,
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
            criterion: a.criterion,
            splits: a.splits,
            max_depth: a.max_depth,
            min_samples_split: a.min_samples_split,
            min_samples_leaf: a.min_samples_leaf,
            min_gain: a.min_gain,
            class_weight: a.class_weight,
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
        .param(Param::string("data").required().describe("CSV, TSV, semicolon, pipe, or whitespace-delimited training table. One row per observation; the first row may be a header. Feature columns may be numeric or categorical."))
        .param(Param::string("target").default("last").describe("Class column to predict: last, first, a 1-based index, or a header name."))
        .param(Param::string("features").default("").describe("Optional comma-separated feature columns by name or 1-based index. Leave empty to use every non-target column."))
        .param(Param::enumv("criterion", ["gini", "entropy", "gain_ratio"]).default("gini").describe("Split quality measure: gini impurity (CART), Shannon information gain (ID3), or the gain ratio that penalizes many-valued features (C4.5)."))
        .param(Param::enumv("splits", ["binary", "multiway"]).default("binary").describe("How categorical features are split: binary one-vs-rest tests, or one branch per distinct value. Numeric features always use a threshold test."))
        .param(Param::integer("max_depth").default(5).min(1.0).max(20.0).describe("Maximum tree depth. Lower values give shorter, more readable rules."))
        .param(Param::integer("min_samples_split").default(2).min(2.0).max(1000.0).describe("Minimum rows a node must hold before it may be split."))
        .param(Param::integer("min_samples_leaf").default(1).min(1.0).max(1000.0).describe("Minimum rows every resulting branch must keep, so tiny unreliable leaves are not created."))
        .param(Param::number("min_gain").default(0.0).min(0.0).max(1.0).describe("Pre-pruning threshold: a split is only kept when its score beats this impurity decrease (or gain ratio)."))
        .param(Param::enumv("class_weight", ["none", "balanced"]).default("none").describe("Weight classes equally (none) or inversely to their frequency (balanced), which helps on imbalanced data."))
        .param(Param::number("test_split").default(0.0).min(0.0).max(0.5).describe("Fraction of rows held out for a deterministic accuracy check, from 0 to 0.5."))
        .param(Param::integer("seed").default(42).min(0.0).describe("Deterministic seed for the hold-out shuffle. Tree fitting itself is exact."))
        .param(Param::string("predict").default("").describe("Optional rows to classify with the fitted tree: one row per line, either the full table layout, just the feature columns, or with a header naming them."))
        .param(Param::enumv("header", ["auto", "yes", "no"]).default("auto").describe("Whether the first row contains column names."))
        .param(Param::integer("decimals").default(4).min(0.0).max(12.0).describe("Decimal places for thresholds, importance, and accuracy."))
        .param(Param::enumv("format", ["text", "json", "csv", "dot"]).default("text").describe("Output format: readable report, JSON, flat CSV, or a Graphviz DOT digraph."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/decision-tree-classifier",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a decision tree from a pasted table and print if/then rules, importance, and predictions.",
    skill(
        description = "Fit a CART, ID3, or C4.5-style decision tree classifier to a pasted table and report human-readable if/then rules, a text tree, feature importance, training accuracy with a confusion matrix, an optional hold-out check, and predictions for new rows. Runs locally in pure Rust/WASM.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "decision-tree-classifier", |a: Args| {
            let data = a.data.clone();
            let opts: Options = a.into();
            gizza_ai_decision_tree_classifier_core::run(&data, &opts)
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
            "criterion",
            "splits",
            "max_depth",
            "min_samples_split",
            "min_samples_leaf",
            "min_gain",
            "class_weight",
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
            props["criterion"]["enum"],
            serde_json::json!(["gini", "entropy", "gain_ratio"])
        );
        assert_eq!(props["splits"]["enum"], serde_json::json!(["binary", "multiway"]));
        assert_eq!(
            props["class_weight"]["enum"],
            serde_json::json!(["none", "balanced"])
        );
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["text", "json", "csv", "dot"])
        );
        assert_eq!(
            props["header"]["enum"],
            serde_json::json!(["auto", "yes", "no"])
        );
        assert_eq!(props["max_depth"]["default"], 5);
        assert_eq!(props["max_depth"]["maximum"], 20);
        assert_eq!(props["min_samples_split"]["minimum"], 2);
        assert_eq!(props["min_gain"]["default"], 0.0);
        assert_eq!(props["test_split"]["maximum"], 0.5);
        assert_eq!(props["decimals"]["default"], 4);
        assert_eq!(v["additionalProperties"], false);
    }
}
