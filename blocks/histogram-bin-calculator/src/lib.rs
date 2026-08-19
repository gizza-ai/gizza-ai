//! gizza-ai/histogram-bin-calculator — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    numbers: String,
    #[serde(default = "default_auto")]
    rule: String,
    #[serde(default = "default_bins")]
    bins: u32,
    #[serde(default)]
    range_min: String,
    #[serde(default)]
    range_max: String,
    #[serde(default)]
    nice_edges: bool,
    #[serde(default)]
    right_closed: bool,
    #[serde(default = "default_precision")]
    precision: u32,
    #[serde(default = "default_report")]
    output: String,
    #[serde(default)]
    cumulative: bool,
    #[serde(default)]
    density: bool,
    #[serde(default = "default_true")]
    chart: bool,
}

fn default_auto() -> String {
    "auto".to_string()
}
fn default_report() -> String {
    "report".to_string()
}
fn default_bins() -> u32 {
    10
}
fn default_precision() -> u32 {
    4
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("numbers").required().describe("The dataset to bin, one number per line (commas, semicolons, tabs, pipes and spaces also work). Plain decimals and scientific notation only, e.g. '12', '-3.5', '1.2e3'. At least 2 values, at most 100,000."))
        .param(Param::enumv("rule", ["auto", "sturges", "scott", "freedman_diaconis", "rice", "sqrt", "manual"]).default("auto").describe("Which bin-count rule to APPLY (all of them are reported side by side either way). 'auto' (default) takes the finer of Sturges and Freedman-Diaconis, like numpy; 'sturges' is k = ceil(log2(n)) + 1; 'scott' is h = 3.49 x sd x n^(-1/3); 'freedman_diaconis' is h = 2 x IQR x n^(-1/3) and resists outliers; 'rice' is k = ceil(2 x n^(1/3)); 'sqrt' is k = ceil(sqrt(n)); 'manual' uses your own 'bins' count."))
        .param(Param::integer("bins").default(10).min(1.0).max(1000.0).describe("Number of bins to use when rule='manual' (ignored otherwise). 1-1000, default 10."))
        .param(Param::string("range_min").describe("Lower edge of the first bin. Blank (default) uses the smallest value in the data. Values below it are reported as excluded, not counted."))
        .param(Param::string("range_max").describe("Upper edge of the last bin. Blank (default) uses the largest value in the data. Values above it are reported as excluded, not counted."))
        .param(Param::boolean("nice_edges").default(false).describe("Round the bin width up to a human-friendly step (1, 2, 2.5 or 5 x a power of ten) and, when range_min is blank, snap the first edge down to a multiple of it — so edges read 0, 10, 20 instead of 1, 10.75, 20.5. Default false (exact rule-derived edges)."))
        .param(Param::boolean("right_closed").default(false).describe("Use right-closed intervals '(a, b]' so a value on an edge falls in the LOWER bin. Default false = left-closed '[a, b)', the numpy/matplotlib convention, where the last bin also includes the maximum."))
        .param(Param::integer("precision").default(4).min(0.0).max(12.0).describe("Decimal places used when printing bin edges, widths and statistics. Default 4."))
        .param(Param::enumv("output", ["report", "table", "csv", "json"]).default("report").describe("Output shape: 'report' (default) is the readable rule comparison plus a bin table with ASCII bars; 'table' is the bin table alone as TSV; 'csv' is the same table comma-separated; 'json' is the full structured report (summary, every rule, bins)."))
        .param(Param::boolean("cumulative").default(false).describe("Add cumulative count and cumulative percentage columns to the bin table (default false)."))
        .param(Param::boolean("density").default(false).describe("Add a density column, count / (n x bin width) — the bar height of a unit-area histogram (default false)."))
        .param(Param::boolean("chart").default(true).describe("Draw an ASCII bar per bin in the 'report' output so the distribution shape is visible (default true; ignored by table/csv/json)."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/histogram-bin-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Recommend histogram bin counts and widths with Sturges, Scott, and Freedman-Diaconis",
    skill(
        description = "Work out how many bins a histogram of a dataset should have. Reports the bin count and bin width recommended by Sturges (k = ceil(log2 n) + 1), Scott (h = 3.49 x sd x n^(-1/3)), Freedman-Diaconis (h = 2 x IQR x n^(-1/3)), Rice, and the square-root rule side by side, then applies the rule you pick and shows the resulting bin edges, counts, percentages, optional cumulative totals and density, plus n/min/max/mean/median/sd/quartiles/IQR/skewness/kurtosis and an outlier count. Options: rule auto|sturges|scott|freedman_diaconis|rice|sqrt|manual, bins 1-1000 (for manual), range_min/range_max to fix the histogram range, nice_edges true|false for rounded edges, right_closed true|false for (a,b] instead of [a,b), precision 0-12, output report|table|csv|json, cumulative/density/chart true|false. Paste plain decimals or scientific notation; at least 2 and at most 100,000 values. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "histogram-bin-calculator", |a: Args| {
            gizza_ai_histogram_bin_calculator_core::run(
                &a.numbers,
                &a.rule,
                a.bins,
                &a.range_min,
                &a.range_max,
                a.nice_edges,
                a.right_closed,
                a.precision,
                &a.output,
                a.cumulative,
                a.density,
                a.chart,
            )
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
    fn schema_json_matches_authored_chat_schema() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["type"], "object");
        assert_eq!(v["properties"]["numbers"]["type"], "string");
        assert_eq!(v["required"], serde_json::json!(["numbers"]));

        assert_eq!(
            v["properties"]["rule"]["enum"],
            serde_json::json!([
                "auto",
                "sturges",
                "scott",
                "freedman_diaconis",
                "rice",
                "sqrt",
                "manual"
            ])
        );
        assert_eq!(v["properties"]["rule"]["default"], "auto");

        assert_eq!(v["properties"]["bins"]["type"], "integer");
        assert_eq!(v["properties"]["bins"]["default"], 10);
        assert_eq!(v["properties"]["bins"]["minimum"], 1.0);
        assert_eq!(v["properties"]["bins"]["maximum"], 1000.0);

        assert_eq!(v["properties"]["range_min"]["type"], "string");
        assert_eq!(v["properties"]["range_max"]["type"], "string");

        assert_eq!(v["properties"]["nice_edges"]["type"], "boolean");
        assert_eq!(v["properties"]["nice_edges"]["default"], false);
        assert_eq!(v["properties"]["right_closed"]["default"], false);

        assert_eq!(v["properties"]["precision"]["default"], 4);
        assert_eq!(v["properties"]["precision"]["minimum"], 0.0);
        assert_eq!(v["properties"]["precision"]["maximum"], 12.0);

        assert_eq!(
            v["properties"]["output"]["enum"],
            serde_json::json!(["report", "table", "csv", "json"])
        );
        assert_eq!(v["properties"]["output"]["default"], "report");

        assert_eq!(v["properties"]["cumulative"]["default"], false);
        assert_eq!(v["properties"]["density"]["default"], false);
        assert_eq!(v["properties"]["chart"]["default"], true);

        // Drift guard: the page form and the CLI both read this property set.
        let props = v["properties"].as_object().unwrap();
        let mut names: Vec<&str> = props.keys().map(|k| k.as_str()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            vec![
                "bins",
                "chart",
                "cumulative",
                "density",
                "nice_edges",
                "numbers",
                "output",
                "precision",
                "range_max",
                "range_min",
                "right_closed",
                "rule",
            ]
        );
        // Every param carries a description an LLM/CLI user can act on.
        for (name, p) in props {
            let d = p["description"].as_str().unwrap_or("");
            assert!(d.len() > 20, "param {name} needs a real description");
        }
    }

    #[test]
    fn defaults_deserialize_from_a_bare_numbers_arg() {
        let a: Args = serde_json::from_str(r#"{"numbers":"1 2 3 4"}"#).unwrap();
        assert_eq!(a.rule, "auto");
        assert_eq!(a.bins, 10);
        assert_eq!(a.precision, 4);
        assert_eq!(a.output, "report");
        assert!(a.chart);
        assert!(!a.cumulative);
        assert!(!a.nice_edges);
        assert!(a.range_min.is_empty());
    }
}
