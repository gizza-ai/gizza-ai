//! gizza-ai/isolation-forest-anomaly — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_isolation_forest_anomaly_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
#[serde(default)]
struct Args {
    input: String,
    features: String,
    method: String,
    trees: u32,
    sample_size: String,
    max_features: f64,
    bootstrap: bool,
    contamination: String,
    threshold: f64,
    missing: String,
    sort: String,
    top: u32,
    only_anomalies: bool,
    header: String,
    delimiter: String,
    decimals: u32,
    seed: u64,
    format: String,
}

impl Default for Args {
    fn default() -> Self {
        let o = Options::default();
        Args {
            input: String::new(),
            features: o.features,
            method: o.method,
            trees: o.trees,
            sample_size: o.sample_size,
            max_features: o.max_features,
            bootstrap: o.bootstrap,
            contamination: o.contamination,
            threshold: o.threshold,
            missing: o.missing,
            sort: o.sort,
            top: o.top,
            only_anomalies: o.only_anomalies,
            header: o.header,
            delimiter: o.delimiter,
            decimals: o.decimals,
            seed: o.seed,
            format: o.format,
        }
    }
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            features: a.features,
            method: a.method,
            trees: a.trees,
            sample_size: a.sample_size,
            max_features: a.max_features,
            bootstrap: a.bootstrap,
            contamination: a.contamination,
            threshold: a.threshold,
            missing: a.missing,
            sort: a.sort,
            top: a.top,
            only_anomalies: a.only_anomalies,
            header: a.header,
            delimiter: a.delimiter,
            decimals: a.decimals,
            seed: a.seed,
            format: a.format,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("CSV, TSV, semicolon, pipe, or whitespace-delimited table. One row per observation; the first row may be a header. Only numeric columns can be scored. Max 20000 rows and 200 columns."))
        .param(Param::string("features").default("").describe("Optional comma-separated feature columns by header name or 1-based index, e.g. 'temp,pressure' or '2,3'. Leave empty to use every fully numeric column."))
        .param(Param::enumv("method", ["standard", "extended"]).default("standard").describe("Isolation variant. standard = axis-parallel cuts, the original Liu/Ting/Zhou algorithm; scale-invariant per column. extended = randomly-oriented hyperplane cuts, which removes the rectangular banding the standard version leaves around correlated columns; features are standardized first because hyperplanes mix columns."))
        .param(Param::integer("trees").default(100).min(1.0).max(1000.0).describe("Number of isolation trees in the forest. More trees give a steadier average path length and smoother scores at a linear cost. Default 100."))
        .param(Param::string("sample_size").default("auto").describe("Rows drawn per tree: auto for min(256, row count) as in the paper, an absolute count such as 128, a fraction such as 0.25, or a percentage such as 25%. Small subsamples are what make isolation forests good at swamping-resistant local anomalies."))
        .param(Param::number("max_features").default(1.0).min(0.01).max(200.0).describe("Feature columns each tree may split on: a fraction of the selected columns when 1 or less (1 = all of them), otherwise an absolute count. Lower values decorrelate the trees on wide tables."))
        .param(Param::boolean("bootstrap").default(false).describe("Draw each tree's subsample with replacement instead of without. Default false (sample without replacement)."))
        .param(Param::string("contamination").default("auto").describe("Expected outlier rate used to place the score cut-off: auto keeps the classic 0.5 score threshold, or give a fraction such as 0.05 or a percentage such as 5% to flag that share of the rows. Must be above 0 and at most 0.5."))
        .param(Param::number("threshold").default(0.0).min(0.0).max(0.9999).describe("Explicit anomaly-score cut-off in (0, 1); rows scoring at or above it are flagged. 0 means derive the cut-off from contamination instead. Overrides contamination when set."))
        .param(Param::enumv("missing", ["drop", "median", "mean", "zero", "error"]).default("drop").describe("What to do with a blank or non-numeric cell in a feature column. drop = skip that row (it is still listed, marked skipped). median / mean = fill from the column's numeric values. zero = fill with 0. error = stop and name the offending cell."))
        .param(Param::enumv("sort", ["input", "score"]).default("input").describe("Row order in the output: input keeps the pasted order, score lists the most anomalous rows first."))
        .param(Param::integer("top").default(0).min(0.0).max(20000.0).describe("Keep only the first N listed rows after sorting. 0 lists every row. Pair with sort=score to get a shortlist of the worst offenders."))
        .param(Param::boolean("only_anomalies").default(false).describe("List only the rows flagged as anomalies, dropping the normal rows from the table. The summary still counts every scored row."))
        .param(Param::enumv("header", ["auto", "yes", "no"]).default("auto").describe("Whether the first row contains column names. auto treats a first row with any non-numeric cell as a header."))
        .param(Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe", "space"]).default("auto").describe("Column delimiter. auto picks whichever of comma, tab, semicolon or pipe appears most on the first line, falling back to runs of whitespace."))
        .param(Param::integer("decimals").default(4).min(0.0).max(12.0).describe("Decimal places for anomaly scores, path lengths and the summary averages."))
        .param(Param::integer("seed").default(42).min(0.0).describe("Seed for the subsampling, the split features and the split values. The same input plus the same seed always gives byte-identical output."))
        .param(Param::enumv("format", ["text", "json", "csv"]).default("text").describe("Output format: a readable report with a scored table, JSON, or the original table with anomaly_score, path_length, is_anomaly and rank columns appended."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/isolation-forest-anomaly",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Score every row of a pasted table with an isolation forest and flag the outliers.",
    skill(
        description = "Fit an isolation forest on the numeric columns of a pasted CSV/TSV table and report a per-row anomaly score, average path length, rank and anomaly flag, plus a summary of the forest settings and the normal/anomalous score split. Supports the standard axis-parallel algorithm and the extended hyperplane variant, subsampling, bootstrapping, per-tree feature sampling, a contamination rate or an explicit score threshold, missing-value policies, and text/JSON/CSV output. Deterministic for a given seed and runs locally in pure Rust/WASM.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "isolation-forest-anomaly", |a: Args| {
            let input = a.input.clone();
            let opts: Options = a.into();
            gizza_ai_isolation_forest_anomaly_core::run(&input, &opts)
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

    /// Drift guard: the chat/CLI schema must keep every param, every
    /// description, and every fixed choice list the page form renders from.
    #[test]
    fn schema_json_has_expected_parameters() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["type"], "object");
        assert_eq!(v["required"], serde_json::json!(["input"]));
        assert_eq!(v["additionalProperties"], false);
        let props = v["properties"].as_object().unwrap();
        for name in [
            "input",
            "features",
            "method",
            "trees",
            "sample_size",
            "max_features",
            "bootstrap",
            "contamination",
            "threshold",
            "missing",
            "sort",
            "top",
            "only_anomalies",
            "header",
            "delimiter",
            "decimals",
            "seed",
            "format",
        ] {
            assert!(props.contains_key(name), "missing {name} in schema");
            assert!(
                props[name]
                    .get("description")
                    .and_then(|d| d.as_str())
                    .is_some_and(|d| !d.is_empty()),
                "missing description for {name}"
            );
        }
        assert_eq!(props.len(), 18, "unexpected parameter count");
    }

    #[test]
    fn enum_params_expose_their_fixed_choices() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &v["properties"];
        assert_eq!(
            props["method"]["enum"],
            serde_json::json!(["standard", "extended"])
        );
        assert_eq!(
            props["missing"]["enum"],
            serde_json::json!(["drop", "median", "mean", "zero", "error"])
        );
        assert_eq!(props["sort"]["enum"], serde_json::json!(["input", "score"]));
        assert_eq!(
            props["header"]["enum"],
            serde_json::json!(["auto", "yes", "no"])
        );
        assert_eq!(
            props["delimiter"]["enum"],
            serde_json::json!(["auto", "comma", "tab", "semicolon", "pipe", "space"])
        );
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["text", "json", "csv"])
        );
        assert_eq!(props["bootstrap"]["type"], "boolean");
        assert_eq!(props["only_anomalies"]["type"], "boolean");
        assert_eq!(props["trees"]["type"], "integer");
        assert_eq!(props["max_features"]["type"], "number");
    }

    /// The schema defaults are what chat sends when a param is omitted, so they
    /// must not drift from `Options::default()`.
    #[test]
    fn schema_defaults_match_core_defaults() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &v["properties"];
        let o = Options::default();
        assert_eq!(props["features"]["default"], o.features);
        assert_eq!(props["method"]["default"], o.method);
        assert_eq!(props["trees"]["default"], o.trees);
        assert_eq!(props["sample_size"]["default"], o.sample_size);
        assert_eq!(props["max_features"]["default"], o.max_features);
        assert_eq!(props["bootstrap"]["default"], o.bootstrap);
        assert_eq!(props["contamination"]["default"], o.contamination);
        assert_eq!(props["threshold"]["default"], o.threshold);
        assert_eq!(props["missing"]["default"], o.missing);
        assert_eq!(props["sort"]["default"], o.sort);
        assert_eq!(props["top"]["default"], o.top);
        assert_eq!(props["only_anomalies"]["default"], o.only_anomalies);
        assert_eq!(props["header"]["default"], o.header);
        assert_eq!(props["delimiter"]["default"], o.delimiter);
        assert_eq!(props["decimals"]["default"], o.decimals);
        assert_eq!(props["seed"]["default"], o.seed);
        assert_eq!(props["format"]["default"], o.format);
    }

    #[test]
    fn numeric_bounds_match_the_core_limits() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &v["properties"];
        assert_eq!(props["trees"]["minimum"], 1);
        assert_eq!(
            props["trees"]["maximum"],
            gizza_ai_isolation_forest_anomaly_core::MAX_TREES
        );
        assert_eq!(props["top"]["minimum"], 0);
        assert_eq!(
            props["top"]["maximum"],
            gizza_ai_isolation_forest_anomaly_core::MAX_ROWS
        );
        assert_eq!(
            props["max_features"]["maximum"],
            gizza_ai_isolation_forest_anomaly_core::MAX_COLS
        );
        assert_eq!(props["threshold"]["minimum"], 0);
        assert_eq!(props["threshold"]["maximum"], 0.9999);
        assert_eq!(props["decimals"]["minimum"], 0);
        assert_eq!(props["decimals"]["maximum"], 12);
        assert_eq!(props["seed"]["minimum"], 0);
    }

    /// Args -> Options must carry every field through; a dropped field would
    /// silently ignore a chat param.
    #[test]
    fn args_defaults_round_trip_into_core_options() {
        let a: Args = serde_json::from_str(r#"{"input":"1,2\n3,4"}"#).unwrap();
        assert_eq!(a.input, "1,2\n3,4");
        let o: Options = a.into();
        let d = Options::default();
        assert_eq!(o.method, d.method);
        assert_eq!(o.trees, d.trees);
        assert_eq!(o.sample_size, d.sample_size);
        assert_eq!(o.contamination, d.contamination);
        assert_eq!(o.delimiter, d.delimiter);
        assert_eq!(o.seed, d.seed);
        assert_eq!(o.format, d.format);

        let a: Args = serde_json::from_str(
            r#"{"input":"x","method":"extended","trees":50,"sample_size":"8","max_features":0.5,
                "bootstrap":true,"contamination":"10%","threshold":0.7,"missing":"median",
                "sort":"score","top":3,"only_anomalies":true,"header":"no","delimiter":"tab",
                "decimals":2,"seed":7,"format":"json","features":"temp"}"#,
        )
        .unwrap();
        let o: Options = a.into();
        assert_eq!(o.features, "temp");
        assert_eq!(o.method, "extended");
        assert_eq!(o.trees, 50);
        assert_eq!(o.sample_size, "8");
        assert_eq!(o.max_features, 0.5);
        assert!(o.bootstrap);
        assert_eq!(o.contamination, "10%");
        assert_eq!(o.threshold, 0.7);
        assert_eq!(o.missing, "median");
        assert_eq!(o.sort, "score");
        assert_eq!(o.top, 3);
        assert!(o.only_anomalies);
        assert_eq!(o.header, "no");
        assert_eq!(o.delimiter, "tab");
        assert_eq!(o.decimals, 2);
        assert_eq!(o.seed, 7);
        assert_eq!(o.format, "json");
    }

    /// Every enum variant the schema advertises must actually be accepted by
    /// core — an unreachable choice in the dropdown is a live bug.
    #[test]
    fn every_advertised_enum_variant_is_accepted_by_core() {
        // All-numeric so the header=yes/no variants stay valid tables too.
        const DATA: &str = "20,101\n21,102\n20,100\n22,101\n21,101\n20,102\n21,100\n90,300";
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        for (param, set) in [
            ("method", "method"),
            ("missing", "missing"),
            ("sort", "sort"),
            ("header", "header"),
            ("format", "format"),
        ] {
            for variant in v["properties"][param]["enum"].as_array().unwrap() {
                let variant = variant.as_str().unwrap().to_string();
                let mut o = Options::default();
                match set {
                    "method" => o.method = variant.clone(),
                    "missing" => o.missing = variant.clone(),
                    "sort" => o.sort = variant.clone(),
                    "header" => o.header = variant.clone(),
                    _ => o.format = variant.clone(),
                }
                // header=yes/no changes the row split but must still parse.
                assert!(
                    gizza_ai_isolation_forest_anomaly_core::run(DATA, &o).is_ok(),
                    "{param}={variant} was advertised but rejected"
                );
            }
        }
        for variant in v["properties"]["delimiter"]["enum"].as_array().unwrap() {
            let o = Options {
                delimiter: variant.as_str().unwrap().into(),
                ..Options::default()
            };
            let data = match variant.as_str().unwrap() {
                "tab" => "1\t1\n2\t1\n1\t2\n50\t50",
                "semicolon" => "1;1\n2;1\n1;2\n50;50",
                "pipe" => "1|1\n2|1\n1|2\n50|50",
                "space" => "1 1\n2 1\n1 2\n50 50",
                _ => "1,1\n2,1\n1,2\n50,50",
            };
            assert!(
                gizza_ai_isolation_forest_anomaly_core::run(data, &o).is_ok(),
                "delimiter={variant} was advertised but rejected"
            );
        }
    }
}
