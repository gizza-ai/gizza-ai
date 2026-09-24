//! gizza-ai/t-test-calculator — Student's t-tests as a chat skill block on the
//! shared tool abstraction. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI and, via manifest.json, the page form); handle()
//! delegates to block_utils::run_skill, which hands the parsed Args to the
//! shared core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_auto")]
    test: String,
    #[serde(default = "default_auto")]
    format: String,
    #[serde(default = "default_auto")]
    delimiter: String,
    #[serde(default = "default_auto")]
    header: String,
    #[serde(default)]
    mu: f64,
    #[serde(default = "default_tails")]
    tails: String,
    #[serde(default = "default_alpha")]
    alpha: f64,
    #[serde(default = "default_decimals")]
    decimals: u32,
    #[serde(default = "default_output")]
    output: String,
}

fn default_auto() -> String {
    "auto".into()
}
fn default_tails() -> String {
    "two".into()
}
fn default_alpha() -> f64 {
    0.05
}
fn default_decimals() -> u32 {
    4
}
fn default_output() -> String {
    "summary".into()
}

const DATA_DESC: &str = "The observations to test, one row per line, in any of four shapes. One column of numbers for a one-sample test, for example `5.1` / `4.8` / `5.4`. Two columns for a two-sample or paired test, one sample per column, for example `12,10` / `14,11` — blank cells are allowed for unequal sample sizes in an independent-samples test. Long rows pairing a group label with a value, for example `Control,6` / `Drug,13`, with exactly two distinct labels. Summary rows as `name,n,mean,sd`, for example `Control,12,5.2,1.9235`, when you only have published means and standard deviations; convert a standard error with `sd = sem * sqrt(n)`. Comma, tab, semicolon, pipe, and space separated rows all work, a header row is detected automatically, and `#` starts a comment. Up to 200000 values.";
const TEST_DESC: &str = "Which t-test to run. auto (default) picks the one-sample test for a single column and Welch's unequal-variance test for two samples, which is the modern default recommendation. one-sample compares one column's mean against mu. two-sample is the pooled Student test, which assumes the two populations share a variance. welch is the unequal-variance test with Welch-Satterthwaite fractional degrees of freedom. paired tests matched observations: give two equal-length columns and it tests their differences, or one column of pre-computed differences.";
const FORMAT_DESC: &str = "How to read the pasted rows. auto (default) infers the shape: a `name,n,mean,sd` table is read as summary, a two-column table whose label column repeats non-numeric names is read as long, anything else is read as wide (one column per sample). Set wide, long, or summary explicitly when the guess is wrong — most often with numeric group labels like `1,23.5`, which auto reads as two wide columns but you may mean as long.";
const DELIMITER_DESC: &str = "How each row is split into columns. auto (default) picks the separator that appears across the first rows, preferring tab, then comma, then semicolon, then pipe, and falling back to runs of whitespace. Force one when a label itself contains the auto-detected separator — for example choose tab for `Site A, north<TAB>12` rows.";
const HEADER_DESC: &str = "Whether the first row names the columns or groups rather than holding data. auto (default) treats it as a header when its cells are not all numeric, and then uses those names as the sample names. yes always skips it, no always treats it as data — use no when your samples really are labelled with numbers.";
const MU_DESC: &str = "The value the null hypothesis claims. Default 0. For a one-sample test this is the hypothesised population mean, so set it to 100 to test a sample against a published norm of 100. For a two-sample or paired test it is the hypothesised mean difference, so set it to 5 to test whether one group beats the other by more than 5 rather than by any amount at all. It shifts the t statistic, the p-value, and the effect size, while the confidence interval stays centred on the observed estimate.";
const TAILS_DESC: &str = "Which alternative hypothesis to test. two (default) is the two-tailed test: the mean or difference is not equal to mu, and the reported interval is two-sided. right tests only that the estimate is greater than mu, left only that it is less than mu; both halve the p-value when the effect points that way and report a one-sided bound instead of an interval. Pick a one-tailed test only when the direction was chosen before seeing the data.";
const ALPHA_DESC: &str = "Significance level for the test, between 0.0001 and 0.5. Default 0.05. Sets the critical t reported next to the t statistic, decides the reject / fail-to-reject verdict, sets the confidence level of the interval at 100 * (1 - alpha), and is the threshold used by the equal-variance check and the observed-power figure. Use 0.01 for a stricter test, 0.1 for an exploratory one.";
const DECIMALS_DESC: &str = "Decimal places for every number in the text and table output, 0-10. Default 4, which matches how t-tests are usually reported. p-values smaller than the chosen resolution print as `< 0.0001` rather than rounding to 0. Also controls the rounding applied to json output.";
const OUTPUT_DESC: &str = "Output format. summary (default) is a readable report: per-sample n / mean / sd / sem / min / max, the estimate and its standard error, t with its degrees of freedom and exact p-value, the critical t, the reject verdict, the confidence interval, Cohen's d and Hedges' g with their standardiser named, observed power, and the variance-ratio assumption check. table returns the same content as GitHub-flavoured markdown tables you can paste into a document or issue. json returns the whole result as a machine-readable object with every statistic rounded to decimals.";

/// Single source for the chat schema, the CLI, and (via
/// `scripts/sync-tool-manifest.py`) the page form's controls.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(DATA_DESC))
        .param(
            Param::enumv(
                "test",
                ["auto", "one-sample", "two-sample", "welch", "paired"],
            )
            .default("auto")
            .describe(TEST_DESC),
        )
        .param(
            Param::enumv("format", ["auto", "wide", "long", "summary"])
                .default("auto")
                .describe(FORMAT_DESC),
        )
        .param(
            Param::enumv(
                "delimiter",
                ["auto", "comma", "tab", "semicolon", "pipe", "space"],
            )
            .default("auto")
            .describe(DELIMITER_DESC),
        )
        .param(
            Param::enumv("header", ["auto", "yes", "no"])
                .default("auto")
                .describe(HEADER_DESC),
        )
        .param(Param::number("mu").default(0.0).describe(MU_DESC))
        .param(
            Param::enumv("tails", ["two", "left", "right"])
                .default("two")
                .describe(TAILS_DESC),
        )
        .param(
            Param::number("alpha")
                .min(0.0001)
                .max(0.5)
                .default(0.05)
                .describe(ALPHA_DESC),
        )
        .param(
            Param::integer("decimals")
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
    name = "gizza-ai/t-test-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Run one-sample, paired, pooled or Welch t-tests on pasted data: t, df, exact p, critical t, confidence interval, Cohen's d and power",
    skill(
        description = "Run a Student's t-test on pasted data and report whether the means differ. Handles the one-sample test, the pooled two-sample (Student) test, Welch's unequal-variance test, and the paired test, choosing one automatically when you do not. Accepts one column of values, two side-by-side columns, long `group,value` rows, or published `name,n,mean,sd` summary statistics, and auto-detects the shape, the delimiter, and a header row. Returns per-sample descriptive statistics, the estimate and its standard error, the t statistic with its degrees of freedom (fractional Welch-Satterthwaite where applicable), the exact two-, left- or right-tailed p-value, the critical t at your alpha, a reject / fail-to-reject verdict, the confidence interval for the mean or the mean difference, Cohen's d and Hedges' g with an approximate interval and the standardiser named, observed power from the noncentral t distribution, and a variance-ratio check that says whether the pooled or the Welch test fits. Options control the test, input format, delimiter, header handling, the hypothesised mean or difference, the number of tails, the significance level, decimal precision, and whether to return a readable summary, markdown tables, or JSON. Every statistic, including the incomplete beta and the noncentral t distribution, is computed locally in pure Rust; no data leaves the machine and no statistics service is called.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "t-test-calculator", |a: Args| {
            gizza_ai_t_test_calculator_core::run(
                &a.data,
                &a.test,
                &a.format,
                &a.delimiter,
                &a.header,
                a.mu,
                &a.tails,
                a.alpha,
                a.decimals as f64,
                &a.output,
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
    fn dump_schema() {
        println!("SCHEMA_BEGIN{}SCHEMA_END", schema_json());
    }

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
    /// serde's `#[serde(default = ...)]` fallbacks must agree with them.
    #[test]
    fn args_defaults_match_the_descriptor() {
        let a: Args = serde_json::from_str(r#"{"data":"1,4\n2,5\n3,6\n4,7\n5,8"}"#).unwrap();
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &schema["properties"];
        assert_eq!(a.test, props["test"]["default"]);
        assert_eq!(a.format, props["format"]["default"]);
        assert_eq!(a.delimiter, props["delimiter"]["default"]);
        assert_eq!(a.header, props["header"]["default"]);
        assert_eq!(a.mu, props["mu"]["default"].as_f64().unwrap());
        assert_eq!(a.tails, props["tails"]["default"]);
        assert_eq!(a.alpha, props["alpha"]["default"].as_f64().unwrap());
        assert_eq!(
            u64::from(a.decimals),
            props["decimals"]["default"].as_u64().unwrap()
        );
        assert_eq!(a.output, props["output"]["default"]);
    }

    /// The defaulted Args reach the core and produce the documented headline
    /// line, so a default drifting out of the core's accepted set is caught.
    #[test]
    fn defaulted_args_run_through_the_core() {
        let a: Args = serde_json::from_str(r#"{"data":"1,4\n2,5\n3,6\n4,7\n5,8"}"#).unwrap();
        let out = gizza_ai_t_test_calculator_core::run(
            &a.data,
            &a.test,
            &a.format,
            &a.delimiter,
            &a.header,
            a.mu,
            &a.tails,
            a.alpha,
            a.decimals as f64,
            &a.output,
        )
        .unwrap();
        assert!(out.contains("Welch's unequal-variance t-test"), "{out}");
        assert!(out.contains("t(8) = -3.0000"), "{out}");
    }

    /// Every enum variant the descriptor advertises must be one the core
    /// accepts — an advertised-but-rejected option is a broken dropdown.
    #[test]
    fn every_advertised_enum_variant_is_accepted_by_the_core() {
        const WIDE: &str = "1,4\n2,5\n3,6\n4,7\n5,8";
        const ONE: &str = "1\n2\n3\n4\n5";
        const LONG: &str = "Control,5\nControl,6\nControl,9\nDrug,8\nDrug,11\nDrug,13";
        const SUMMARY: &str = "Control,5,3.0,1.5811\nDrug,5,6.0,1.5811";
        // The wide columns differ by a constant, which is degenerate for the
        // paired test (sd of the differences is 0), so pairs get their own data.
        const PAIRS: &str = "12,10\n14,11\n11,10\n15,12\n13,12\n16,13";
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let variants = |p: &str| -> Vec<String> {
            schema["properties"][p]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        };
        for v in variants("test") {
            let data = match v.as_str() {
                "one-sample" => ONE,
                "paired" => PAIRS,
                _ => WIDE,
            };
            gizza_ai_t_test_calculator_core::run(
                data, &v, "auto", "auto", "auto", 0.0, "two", 0.05, 4.0, "summary",
            )
            .unwrap_or_else(|e| panic!("test={v}: {e}"));
        }
        for v in variants("format") {
            let data = match v.as_str() {
                "long" => LONG,
                "summary" => SUMMARY,
                _ => WIDE,
            };
            gizza_ai_t_test_calculator_core::run(
                data, "auto", &v, "auto", "auto", 0.0, "two", 0.05, 4.0, "summary",
            )
            .unwrap_or_else(|e| panic!("format={v}: {e}"));
        }
        for v in variants("delimiter") {
            let data = match v.as_str() {
                "tab" => "1\t4\n2\t5\n3\t6\n4\t7\n5\t8",
                "semicolon" => "1;4\n2;5\n3;6\n4;7\n5;8",
                "pipe" => "1|4\n2|5\n3|6\n4|7\n5|8",
                "space" => "1 4\n2 5\n3 6\n4 7\n5 8",
                _ => WIDE,
            };
            gizza_ai_t_test_calculator_core::run(
                data, "welch", "wide", &v, "no", 0.0, "two", 0.05, 4.0, "summary",
            )
            .unwrap_or_else(|e| panic!("delimiter={v}: {e}"));
        }
        for v in variants("header") {
            gizza_ai_t_test_calculator_core::run(
                WIDE, "welch", "wide", "comma", &v, 0.0, "two", 0.05, 4.0, "summary",
            )
            .unwrap_or_else(|e| panic!("header={v}: {e}"));
        }
        for v in variants("tails") {
            gizza_ai_t_test_calculator_core::run(
                WIDE, "auto", "auto", "auto", "auto", 0.0, &v, 0.05, 4.0, "summary",
            )
            .unwrap_or_else(|e| panic!("tails={v}: {e}"));
        }
        for v in variants("output") {
            gizza_ai_t_test_calculator_core::run(
                WIDE, "auto", "auto", "auto", "auto", 0.0, "two", 0.05, 4.0, &v,
            )
            .unwrap_or_else(|e| panic!("output={v}: {e}"));
        }
    }

    /// Drift guard: the chat/CLI/page schema is generated from `descriptor()`, so any
    /// change to a param name, type, enum, bound, or default must be mirrored here.
    #[test]
    fn schema_matches_the_authored_contract() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["data"],
            "properties": {
                "data": { "type": "string", "description": DATA_DESC },
                "test": {
                    "type": "string",
                    "enum": ["auto", "one-sample", "two-sample", "welch", "paired"],
                    "default": "auto",
                    "description": TEST_DESC
                },
                "format": {
                    "type": "string",
                    "enum": ["auto", "wide", "long", "summary"],
                    "default": "auto",
                    "description": FORMAT_DESC
                },
                "delimiter": {
                    "type": "string",
                    "enum": ["auto", "comma", "tab", "semicolon", "pipe", "space"],
                    "default": "auto",
                    "description": DELIMITER_DESC
                },
                "header": {
                    "type": "string",
                    "enum": ["auto", "yes", "no"],
                    "default": "auto",
                    "description": HEADER_DESC
                },
                "mu": {
                    "type": "number",
                    "default": 0.0,
                    "description": MU_DESC
                },
                "tails": {
                    "type": "string",
                    "enum": ["two", "left", "right"],
                    "default": "two",
                    "description": TAILS_DESC
                },
                "alpha": {
                    "type": "number",
                    "minimum": 0.0001,
                    "maximum": 0.5,
                    "default": 0.05,
                    "description": ALPHA_DESC
                },
                "decimals": {
                    "type": "integer",
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
