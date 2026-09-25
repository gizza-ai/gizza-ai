//! gizza-ai/hdbscan-cluster — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_hdbscan_cluster_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
#[serde(default)]
struct Args {
    input: String,
    features: String,
    min_cluster_size: u32,
    min_samples: u32,
    metric: String,
    alpha: f64,
    cluster_selection_epsilon: f64,
    selection: String,
    allow_single_cluster: bool,
    max_cluster_size: u32,
    normalize: bool,
    missing: String,
    sort: String,
    top: u32,
    only_noise: bool,
    header: String,
    delimiter: String,
    decimals: u32,
    format: String,
}

impl Default for Args {
    fn default() -> Self {
        let o = Options::default();
        Args {
            input: String::new(),
            features: o.features,
            min_cluster_size: o.min_cluster_size,
            min_samples: o.min_samples,
            metric: o.metric,
            alpha: o.alpha,
            cluster_selection_epsilon: o.cluster_selection_epsilon,
            selection: o.selection,
            allow_single_cluster: o.allow_single_cluster,
            max_cluster_size: o.max_cluster_size,
            normalize: o.normalize,
            missing: o.missing,
            sort: o.sort,
            top: o.top,
            only_noise: o.only_noise,
            header: o.header,
            delimiter: o.delimiter,
            decimals: o.decimals,
            format: o.format,
        }
    }
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            features: a.features,
            min_cluster_size: a.min_cluster_size,
            min_samples: a.min_samples,
            metric: a.metric,
            alpha: a.alpha,
            cluster_selection_epsilon: a.cluster_selection_epsilon,
            selection: a.selection,
            allow_single_cluster: a.allow_single_cluster,
            max_cluster_size: a.max_cluster_size,
            normalize: a.normalize,
            missing: a.missing,
            sort: a.sort,
            top: a.top,
            only_noise: a.only_noise,
            header: a.header,
            delimiter: a.delimiter,
            decimals: a.decimals,
            format: a.format,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("CSV, TSV, semicolon, pipe, or whitespace-delimited table. One row per point; the first row may be a header. Only numeric columns can be clustered. Max 5000 rows and 200 columns."))
        .param(Param::string("features").default("").describe("Optional comma-separated feature columns by header name or 1-based index, e.g. 'x,y' or '2,3'. Leave empty to use every fully numeric column."))
        .param(Param::integer("min_cluster_size").default(5).min(2.0).max(5000.0).describe("Smallest group of points that may be called a cluster. Raising it returns fewer, broader clusters and pushes more points to noise; it is the one parameter most runs need to tune. Default 5."))
        .param(Param::integer("min_samples").default(0).min(0.0).max(5000.0).describe("Neighbourhood size k used for each point's core distance (its local density estimate). 0 mirrors min_cluster_size, the reference default. Higher values make the density estimate more conservative, so more points are labelled noise."))
        .param(Param::enumv("metric", ["euclidean", "manhattan", "chebyshev", "cosine"]).default("euclidean").describe("Distance between rows: 'euclidean' (straight-line, the default), 'manhattan' (sum of absolute differences, steadier on many columns), 'chebyshev' (largest single-column difference), or 'cosine' (angle between rows — use it for direction/profile similarity rather than magnitude)."))
        .param(Param::number("alpha").default(1.0).min(0.01).max(100.0).describe("Robust-single-linkage distance scale. Raw distances are divided by alpha before being combined with the core distances, so values above 1 lean more on local density and tend to split conservatively. Leave at 1.0 unless you are deliberately tuning."))
        .param(Param::number("cluster_selection_epsilon").default(0.0).min(0.0).max(1000000.0).describe("Distance below which neighbouring micro-clusters are merged back together, in the clustering feature space (standardized units when normalize is on). 0 turns it off and returns the plain HDBSCAN* answer. Set it when the result is split finer than you care about."))
        .param(Param::enumv("selection", ["eom", "leaf"]).default("eom").describe("How clusters are picked out of the condensed tree: 'eom' (excess of mass) keeps the most stable clusters and gives few, large groups; 'leaf' takes every leaf of the tree and gives many small, homogeneous groups. Default eom."))
        .param(Param::boolean("allow_single_cluster").default(false).describe("Allow the answer to be one cluster covering the whole dataset. Off by default, which forces at least a split or noise; turn it on when you genuinely expect one dense population plus outliers."))
        .param(Param::integer("max_cluster_size").default(0).min(0.0).max(5000.0).describe("Excess-of-mass size cap: a selected cluster larger than this is rejected in favour of its sub-clusters. 0 means no cap. Ignored by selection=leaf."))
        .param(Param::boolean("normalize").default(true).describe("Standardize every feature to zero mean and unit variance before clustering, so columns on different scales contribute comparably. On by default because pasted tables usually mix units. Turn it off when the raw distances are already meaningful (e.g. coordinates in one unit)."))
        .param(Param::enumv("missing", ["drop", "median", "mean", "zero", "error"]).default("drop").describe("What to do with a blank or non-numeric cell in a feature column. drop = skip that row (it is still listed, marked skipped). median / mean = fill from the column's numeric values. zero = fill with 0. error = stop and name the offending cell."))
        .param(Param::enumv("sort", ["input", "cluster", "outlier"]).default("input").describe("Row order in the output: 'input' keeps the pasted order, 'cluster' groups rows by cluster id with noise last, 'outlier' lists the highest GLOSH outlier scores first."))
        .param(Param::integer("top").default(0).min(0.0).max(5000.0).describe("Keep only the first N listed rows after sorting. 0 lists every row. Pair with sort=outlier to get a shortlist of the most outlying points."))
        .param(Param::boolean("only_noise").default(false).describe("List only the rows labelled noise (-1), dropping the clustered rows from the table. The summary still counts every row."))
        .param(Param::enumv("header", ["auto", "yes", "no"]).default("auto").describe("Whether the first row contains column names. auto treats a first row with any non-numeric cell as a header."))
        .param(Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe", "space"]).default("auto").describe("Column delimiter. auto picks whichever of comma, tab, semicolon or pipe appears most on the first line, falling back to runs of whitespace."))
        .param(Param::integer("decimals").default(4).min(0.0).max(12.0).describe("Decimal places for membership probabilities, outlier scores, persistence and cluster centroids."))
        .param(Param::enumv("format", ["text", "json", "csv"]).default("text").describe("Output format: a readable report with a cluster summary and a per-row table, JSON, or the original table with cluster, probability and outlier_score columns appended."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hdbscan-cluster",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Cluster a pasted table with HDBSCAN, finding the cluster count automatically and labelling noise.",
    skill(
        description = "Run HDBSCAN* over the numeric columns of a pasted CSV/TSV table: it discovers the number of clusters by itself, handles clusters of differing densities, and labels leftover points as noise (-1). Reports a per-row cluster label, membership probability and GLOSH outlier score, plus per-cluster size, persistence, centroid and medoid row. Supports min_cluster_size / min_samples, euclidean, manhattan, chebyshev and cosine metrics, excess-of-mass or leaf cluster selection, cluster_selection_epsilon merging, a max cluster size cap, allow_single_cluster, feature standardization, missing-value policies, and text/JSON/CSV output. Fully deterministic — no seed — and runs locally in pure Rust/WASM.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "hdbscan-cluster", |a: Args| {
            let input = a.input.clone();
            let opts: Options = a.into();
            gizza_ai_hdbscan_cluster_core::run(&input, &opts).map_err(SkillError::InvalidArgs)
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
            "min_cluster_size",
            "min_samples",
            "metric",
            "alpha",
            "cluster_selection_epsilon",
            "selection",
            "allow_single_cluster",
            "max_cluster_size",
            "normalize",
            "missing",
            "sort",
            "top",
            "only_noise",
            "header",
            "delimiter",
            "decimals",
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
        assert_eq!(props.len(), 19, "unexpected parameter count");
    }

    #[test]
    fn enum_params_expose_their_fixed_choices() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &v["properties"];
        assert_eq!(
            props["metric"]["enum"],
            serde_json::json!(["euclidean", "manhattan", "chebyshev", "cosine"])
        );
        assert_eq!(props["selection"]["enum"], serde_json::json!(["eom", "leaf"]));
        assert_eq!(
            props["missing"]["enum"],
            serde_json::json!(["drop", "median", "mean", "zero", "error"])
        );
        assert_eq!(
            props["sort"]["enum"],
            serde_json::json!(["input", "cluster", "outlier"])
        );
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
        assert_eq!(props["allow_single_cluster"]["type"], "boolean");
        assert_eq!(props["normalize"]["type"], "boolean");
        assert_eq!(props["only_noise"]["type"], "boolean");
        assert_eq!(props["min_cluster_size"]["type"], "integer");
        assert_eq!(props["alpha"]["type"], "number");
        assert_eq!(props["cluster_selection_epsilon"]["type"], "number");
    }

    /// The schema defaults are what chat sends when a param is omitted, so they
    /// must not drift from `Options::default()`.
    #[test]
    fn schema_defaults_match_core_defaults() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &v["properties"];
        let o = Options::default();
        assert_eq!(props["features"]["default"], o.features);
        assert_eq!(props["min_cluster_size"]["default"], o.min_cluster_size);
        assert_eq!(props["min_samples"]["default"], o.min_samples);
        assert_eq!(props["metric"]["default"], o.metric);
        assert_eq!(props["alpha"]["default"], o.alpha);
        assert_eq!(
            props["cluster_selection_epsilon"]["default"],
            o.cluster_selection_epsilon
        );
        assert_eq!(props["selection"]["default"], o.selection);
        assert_eq!(
            props["allow_single_cluster"]["default"],
            o.allow_single_cluster
        );
        assert_eq!(props["max_cluster_size"]["default"], o.max_cluster_size);
        assert_eq!(props["normalize"]["default"], o.normalize);
        assert_eq!(props["missing"]["default"], o.missing);
        assert_eq!(props["sort"]["default"], o.sort);
        assert_eq!(props["top"]["default"], o.top);
        assert_eq!(props["only_noise"]["default"], o.only_noise);
        assert_eq!(props["header"]["default"], o.header);
        assert_eq!(props["delimiter"]["default"], o.delimiter);
        assert_eq!(props["decimals"]["default"], o.decimals);
        assert_eq!(props["format"]["default"], o.format);
    }

    #[test]
    fn numeric_bounds_match_the_core_limits() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &v["properties"];
        assert_eq!(props["min_cluster_size"]["minimum"], 2);
        assert_eq!(
            props["min_cluster_size"]["maximum"],
            gizza_ai_hdbscan_cluster_core::MAX_ROWS
        );
        assert_eq!(props["min_samples"]["minimum"], 0);
        assert_eq!(
            props["top"]["maximum"],
            gizza_ai_hdbscan_cluster_core::MAX_ROWS
        );
        assert_eq!(
            props["max_cluster_size"]["maximum"],
            gizza_ai_hdbscan_cluster_core::MAX_ROWS
        );
        assert_eq!(props["alpha"]["minimum"], 0.01);
        assert_eq!(props["cluster_selection_epsilon"]["minimum"], 0.0);
        assert_eq!(props["decimals"]["minimum"], 0);
        assert_eq!(props["decimals"]["maximum"], 12);
    }

    /// Args -> Options must carry every field through; a dropped field would
    /// silently ignore a chat param.
    #[test]
    fn args_defaults_round_trip_into_core_options() {
        let a: Args = serde_json::from_str(r#"{"input":"1,2\n3,4"}"#).unwrap();
        assert_eq!(a.input, "1,2\n3,4");
        let o: Options = a.into();
        let d = Options::default();
        assert_eq!(o.min_cluster_size, d.min_cluster_size);
        assert_eq!(o.min_samples, d.min_samples);
        assert_eq!(o.metric, d.metric);
        assert_eq!(o.alpha, d.alpha);
        assert_eq!(o.selection, d.selection);
        assert_eq!(o.normalize, d.normalize);
        assert_eq!(o.delimiter, d.delimiter);
        assert_eq!(o.format, d.format);

        let a: Args = serde_json::from_str(
            r#"{"input":"x","features":"x,y","min_cluster_size":8,"min_samples":3,
                "metric":"cosine","alpha":1.5,"cluster_selection_epsilon":0.25,
                "selection":"leaf","allow_single_cluster":true,"max_cluster_size":40,
                "normalize":false,"missing":"median","sort":"outlier","top":3,
                "only_noise":true,"header":"no","delimiter":"tab","decimals":2,
                "format":"json"}"#,
        )
        .unwrap();
        let o: Options = a.into();
        assert_eq!(o.features, "x,y");
        assert_eq!(o.min_cluster_size, 8);
        assert_eq!(o.min_samples, 3);
        assert_eq!(o.metric, "cosine");
        assert_eq!(o.alpha, 1.5);
        assert_eq!(o.cluster_selection_epsilon, 0.25);
        assert_eq!(o.selection, "leaf");
        assert!(o.allow_single_cluster);
        assert_eq!(o.max_cluster_size, 40);
        assert!(!o.normalize);
        assert_eq!(o.missing, "median");
        assert_eq!(o.sort, "outlier");
        assert_eq!(o.top, 3);
        assert!(o.only_noise);
        assert_eq!(o.header, "no");
        assert_eq!(o.delimiter, "tab");
        assert_eq!(o.decimals, 2);
        assert_eq!(o.format, "json");
    }

    /// Every enum variant the schema advertises must actually be accepted by
    /// core — an unreachable choice in the dropdown is a live bug.
    #[test]
    fn every_advertised_enum_variant_is_accepted_by_core() {
        // All-numeric so the header=yes/no variants stay valid tables too.
        const DATA: &str = "1,1\n1,2\n2,1\n2,2\n1.5,1.5\n1.2,1.8\n\
                            10,10\n10,11\n11,10\n11,11\n10.5,10.5\n10.2,10.8\n60,-40";
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        for (param, set) in [
            ("metric", "metric"),
            ("selection", "selection"),
            ("missing", "missing"),
            ("sort", "sort"),
            ("header", "header"),
            ("format", "format"),
        ] {
            for variant in v["properties"][param]["enum"].as_array().unwrap() {
                let variant = variant.as_str().unwrap().to_string();
                let mut o = Options::default();
                match set {
                    "metric" => o.metric = variant.clone(),
                    "selection" => o.selection = variant.clone(),
                    "missing" => o.missing = variant.clone(),
                    "sort" => o.sort = variant.clone(),
                    "header" => o.header = variant.clone(),
                    _ => o.format = variant.clone(),
                }
                assert!(
                    gizza_ai_hdbscan_cluster_core::run(DATA, &o).is_ok(),
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
                "tab" => DATA.replace(',', "\t"),
                "semicolon" => DATA.replace(',', ";"),
                "pipe" => DATA.replace(',', "|"),
                "space" => DATA.replace(',', " "),
                _ => DATA.to_string(),
            };
            assert!(
                gizza_ai_hdbscan_cluster_core::run(&data, &o).is_ok(),
                "delimiter={variant} was advertised but rejected"
            );
        }
    }
}
