//! gizza-ai/coefficient-of-variation-calculator — chat skill block on the
//! shared tool abstraction. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI and, via manifest.json, the page form); handle()
//! delegates to block_utils::run_skill, which hands the parsed Args to the
//! shared core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

/// Every optional param is an `Option` rather than a serde default so the
/// unwrap in `handle` is the one place a default is written, and `mean` /
/// `std_dev` keep a real "not supplied" state (the core needs to tell a missing
/// summary statistic apart from a zero one).
#[derive(Deserialize)]
struct Args {
    data: String,
    basis: Option<String>,
    grouping: Option<String>,
    delimiter: Option<String>,
    mean: Option<f64>,
    std_dev: Option<f64>,
    exclude_outliers: Option<bool>,
    ignore_non_numeric: Option<bool>,
    decimals: Option<f64>,
    output: Option<String>,
}

const DATA_DESC: &str = "The numbers to analyse, one dataset per line, for example `Machine A: 4.2 5.1 4.8` / `Machine B: 792 800 803`. A leading `Label:` names the dataset and is optional; unlabelled rows become `Dataset 1`, `Dataset 2`, and so on. A single column of one number per line is pooled into one dataset automatically. Comma, tab, semicolon, pipe, and space separated values all work, blank lines are skipped, and `#` starts a comment row. Leave this empty and supply both `mean` and `std_dev` instead to compute the CV from summary statistics you already have. Up to 200000 values across at most 1000 datasets.";
const BASIS_DESC: &str = "Which standard deviation divides the mean. sample (default) uses the sample standard deviation with the n-1 Bessel-corrected divisor, the right choice when your numbers are a sample drawn from a larger population — this is what most spreadsheets and textbooks report. population uses the N divisor, for when the values are the entire population you care about. The sample basis needs at least 2 values per dataset; a single reading only has a defined CV on the population basis.";
const GROUPING_DESC: &str = "How the pasted rows become datasets. auto (default) reads one dataset per line when any row is labelled or holds two or more numbers, and otherwise pools a single column of readings into one dataset. lines always treats every row as its own dataset, so a one-number-per-row list is scored as many one-value datasets. single always pools every number in the input into one dataset, so a multi-row block of readings is treated as one sample.";
const DELIMITER_DESC: &str = "How each row is split into numbers. auto (default) splits on whitespace, commas, semicolons, and pipes all at once, which is safe because a number never contains one of them and needs no detection. Force a single separator when a dataset label itself contains one of the others — for example choose tab for a `Site A, north<TAB>12 14 11` row whose label has a comma in it.";
const MEAN_DESC: &str = "Summary mode: the already-computed mean of the dataset, used only when `data` is empty. Pair it with `std_dev` to get the CV straight from published statistics without the raw numbers. Both must be supplied together; a mean of 0 leaves the CV undefined, and a negative mean is reported with its sign.";
const STD_DEV_DESC: &str = "Summary mode: the already-computed standard deviation of the dataset, used only when `data` is empty. Pair it with `mean`; the CV is then simply this value divided by that mean. Must not be negative. Which basis the number came from is up to you — in summary mode it is taken as given and `basis` is ignored.";
const EXCLUDE_OUTLIERS_DESC: &str = "Drop outliers before computing the statistics. false (default) uses every value. true applies the Tukey 1.5xIQR fence per dataset, removing values below Q1 - 1.5*IQR or above Q3 + 1.5*IQR, and lists exactly which values were removed so the filtering stays auditable. A dataset with fewer than 4 values is left untouched and noted, since the quartiles are not meaningful there.";
const IGNORE_NON_NUMERIC_DESC: &str = "What to do with a cell that is not a number. false (default) is strict: an unparseable token is an error naming the line and the token, so a typo in a hand-typed list is caught rather than silently dropped. true skips non-numeric cells, which is what you want for a paste straight out of a spreadsheet full of `n/a`, blanks, and stray unit suffixes.";
const DECIMALS_DESC: &str = "Decimal places for every number in the output, 0-10. Default 4, which keeps a CV like 0.0056 readable as a ratio and as a percentage. Values are rounded for display and for the json output; the underlying statistics are always computed at full double precision, so the rounding never feeds back into the ranking.";
const OUTPUT_DESC: &str = "Output format. summary (default) is a readable report: per dataset the n, mean, standard deviation, sum of squares, min/max, the CV as a ratio and as a percentage, and a rule-of-thumb relative-spread band, followed by which dataset is most consistent, which is most variable, the ratio between them, and any caveats. table returns the same ranking as a GitHub-flavoured markdown table you can paste into a document or issue. json returns the whole report as a machine-readable object.";

/// Single source for the chat schema, the CLI, and (via
/// `scripts/sync-tool-manifest.py`) the page form's controls.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(DATA_DESC))
        .param(
            Param::enumv("basis", ["sample", "population"])
                .default("sample")
                .describe(BASIS_DESC),
        )
        .param(
            Param::enumv("grouping", ["auto", "single", "lines"])
                .default("auto")
                .describe(GROUPING_DESC),
        )
        .param(
            Param::enumv(
                "delimiter",
                ["auto", "comma", "tab", "semicolon", "space", "pipe"],
            )
            .default("auto")
            .describe(DELIMITER_DESC),
        )
        .param(Param::number("mean").describe(MEAN_DESC))
        .param(Param::number("std_dev").describe(STD_DEV_DESC))
        .param(
            Param::boolean("exclude_outliers")
                .default(false)
                .describe(EXCLUDE_OUTLIERS_DESC),
        )
        .param(
            Param::boolean("ignore_non_numeric")
                .default(false)
                .describe(IGNORE_NON_NUMERIC_DESC),
        )
        .param(
            Param::number("decimals")
                .min(0.0)
                .max(10.0)
                .default(4)
                .describe(DECIMALS_DESC),
        )
        .param(
            Param::enumv("output", ["summary", "table", "json"])
                .default("summary")
                .describe(OUTPUT_DESC),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/coefficient-of-variation-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Calculate and compare coefficients of variation",
    skill(
        description = "Calculate the coefficient of variation (standard deviation divided by the mean) for one or more datasets and rank them by relative dispersion. Because the CV is unitless it compares how consistent datasets are even when they are measured on wildly different scales, which a raw standard deviation cannot do. Paste one dataset per line with an optional `Label:` prefix, or a single column of readings, and get per dataset the n, mean, standard deviation on either the sample (n-1) or population (N) basis, sum of squares, min and max, the CV as both a ratio and a percentage, and a rule-of-thumb relative-spread band — plus which dataset is the most consistent, which is the most variable, and the ratio between them. If you only have published statistics, leave the data empty and supply mean and std_dev to get the CV from those instead. Options control the grouping, the delimiter, an optional Tukey 1.5xIQR outlier filter that reports exactly what it removed, whether non-numeric cells are an error or skipped, the decimal precision, and whether to return a readable summary, a markdown table, or JSON. The CV is undefined at a mean of zero and unstable near it, and the tool says so rather than printing a misleading number. Everything is computed locally in pure Rust; no data leaves the machine.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "coefficient-of-variation-calculator", |a: Args| {
            gizza_ai_coefficient_of_variation_calculator_core::run(
                &a.data,
                a.basis.as_deref().unwrap_or("sample"),
                a.grouping.as_deref().unwrap_or("auto"),
                a.delimiter.as_deref().unwrap_or("auto"),
                a.mean,
                a.std_dev,
                a.exclude_outliers.unwrap_or(false),
                a.ignore_non_numeric.unwrap_or(false),
                a.decimals.unwrap_or(4.0),
                a.output.as_deref().unwrap_or("summary"),
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
    fn every_param_is_described() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().expect("object schema");
        assert_eq!(props.len(), 10, "parameter count changed");
        for (name, spec) in props {
            let desc = spec["description"].as_str().unwrap_or("");
            assert!(desc.len() > 20, "param {name} needs a real description");
        }
        assert_eq!(
            schema["required"].as_array().unwrap(),
            &vec![serde_json::json!("data")]
        );
    }

    /// The descriptor's declared defaults are what chat/the CLI leave out, so
    /// the `unwrap_or` fallbacks in `handle` must agree with them. Both sides
    /// are read here from the one omit-everything payload.
    #[test]
    fn handle_defaults_match_the_descriptor() {
        let a: Args = serde_json::from_str(r#"{"data":"2 4 4 4 5 5 7 9"}"#).unwrap();
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &schema["properties"];
        assert_eq!(
            a.basis.as_deref().unwrap_or("sample"),
            props["basis"]["default"]
        );
        assert_eq!(
            a.grouping.as_deref().unwrap_or("auto"),
            props["grouping"]["default"]
        );
        assert_eq!(
            a.delimiter.as_deref().unwrap_or("auto"),
            props["delimiter"]["default"]
        );
        assert_eq!(
            a.exclude_outliers.unwrap_or(false),
            props["exclude_outliers"]["default"]
        );
        assert_eq!(
            a.ignore_non_numeric.unwrap_or(false),
            props["ignore_non_numeric"]["default"]
        );
        assert_eq!(
            a.decimals.unwrap_or(4.0),
            props["decimals"]["default"].as_f64().unwrap()
        );
        assert_eq!(
            a.output.as_deref().unwrap_or("summary"),
            props["output"]["default"]
        );
        // `mean` / `std_dev` have no default — absent must stay absent, not 0.
        assert!(a.mean.is_none() && a.std_dev.is_none());
        assert!(props["mean"].get("default").is_none());
        assert!(props["std_dev"].get("default").is_none());
    }

    /// The defaulted Args reach the core and produce the documented headline,
    /// so a default drifting out of the core's accepted set is caught.
    #[test]
    fn defaulted_args_run_through_the_core() {
        let a: Args =
            serde_json::from_str(r#"{"data":"Kittens: 4.2 5.1 4.8\nOxen: 792 800 803"}"#).unwrap();
        let out = gizza_ai_coefficient_of_variation_calculator_core::run(
            &a.data,
            a.basis.as_deref().unwrap_or("sample"),
            a.grouping.as_deref().unwrap_or("auto"),
            a.delimiter.as_deref().unwrap_or("auto"),
            a.mean,
            a.std_dev,
            a.exclude_outliers.unwrap_or(false),
            a.ignore_non_numeric.unwrap_or(false),
            a.decimals.unwrap_or(4.0),
            a.output.as_deref().unwrap_or("summary"),
        )
        .unwrap();
        assert!(out.contains("Rank 1 — Oxen"), "{out}");
        assert!(out.contains("Most consistent  : Oxen"), "{out}");
    }

    /// Every enum variant the descriptor advertises must be one the core
    /// accepts — an advertised-but-rejected option is a broken dropdown.
    #[test]
    fn every_advertised_enum_variant_is_accepted_by_the_core() {
        const DATA: &str = "Kittens: 4.2 5.1 4.8\nOxen: 792 800 803";
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let variants = |p: &str| -> Vec<String> {
            schema["properties"][p]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        };
        let run = gizza_ai_coefficient_of_variation_calculator_core::run;
        for v in variants("basis") {
            run(
                DATA, &v, "auto", "auto", None, None, false, false, 4.0, "summary",
            )
            .unwrap_or_else(|e| panic!("basis={v}: {e}"));
        }
        for v in variants("grouping") {
            run(
                DATA, "sample", &v, "auto", None, None, false, false, 4.0, "summary",
            )
            .unwrap_or_else(|e| panic!("grouping={v}: {e}"));
        }
        for v in variants("delimiter") {
            let data = match v.as_str() {
                "comma" => "1,2,3,4",
                "tab" => "1\t2\t3\t4",
                "semicolon" => "1;2;3;4",
                "pipe" => "1|2|3|4",
                "space" => "1 2 3 4",
                _ => DATA,
            };
            run(
                data, "sample", "auto", &v, None, None, false, false, 4.0, "summary",
            )
            .unwrap_or_else(|e| panic!("delimiter={v}: {e}"));
        }
        for v in variants("output") {
            run(
                DATA, "sample", "auto", "auto", None, None, false, false, 4.0, &v,
            )
            .unwrap_or_else(|e| panic!("output={v}: {e}"));
        }
    }

    /// Drift guard: the chat/CLI/page schema is generated from `descriptor()`,
    /// so any change to a param name, type, enum, bound, or default must be
    /// mirrored here.
    #[test]
    fn schema_matches_the_authored_contract() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["data"],
            "properties": {
                "data": { "type": "string", "description": DATA_DESC },
                "basis": {
                    "type": "string",
                    "enum": ["sample", "population"],
                    "default": "sample",
                    "description": BASIS_DESC
                },
                "grouping": {
                    "type": "string",
                    "enum": ["auto", "single", "lines"],
                    "default": "auto",
                    "description": GROUPING_DESC
                },
                "delimiter": {
                    "type": "string",
                    "enum": ["auto", "comma", "tab", "semicolon", "space", "pipe"],
                    "default": "auto",
                    "description": DELIMITER_DESC
                },
                "mean": { "type": "number", "description": MEAN_DESC },
                "std_dev": { "type": "number", "description": STD_DEV_DESC },
                "exclude_outliers": {
                    "type": "boolean",
                    "default": false,
                    "description": EXCLUDE_OUTLIERS_DESC
                },
                "ignore_non_numeric": {
                    "type": "boolean",
                    "default": false,
                    "description": IGNORE_NON_NUMERIC_DESC
                },
                "decimals": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 10,
                    "default": 4,
                    "description": DECIMALS_DESC
                },
                "output": {
                    "type": "string",
                    "enum": ["summary", "table", "json"],
                    "default": "summary",
                    "description": OUTPUT_DESC
                }
            }
        });
        assert_eq!(actual, authored);
    }
}
