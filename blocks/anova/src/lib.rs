//! gizza-ai/anova — one-way ANOVA as a chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI and, via manifest.json, the page form); handle() delegates to
//! block_utils::run_skill, which hands the parsed Args to the shared core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_auto")]
    format: String,
    #[serde(default = "default_auto")]
    delimiter: String,
    #[serde(default = "default_auto")]
    header: String,
    #[serde(default = "default_alpha")]
    alpha: f64,
    #[serde(default = "default_decimals")]
    decimals: u32,
    #[serde(default = "default_posthoc")]
    posthoc: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_auto() -> String {
    "auto".into()
}
fn default_alpha() -> f64 {
    0.05
}
fn default_decimals() -> u32 {
    4
}
fn default_posthoc() -> String {
    "none".into()
}
fn default_output() -> String {
    "summary".into()
}

const DATA_DESC: &str = "The observations to compare, one row per line, in any of three shapes. Long: a group label and a value per row, for example `Control,6` / `Control,8` / `Drug A,13`. Wide: one column per group, one row per observation, for example `6,8,13` / `8,12,9` — blank cells are allowed for unequal group sizes. Summary: per-group statistics as `name,n,mean,sd`, for example `Control,5,5.2,1.9235`, when you only have published means and standard deviations. Comma, tab, semicolon, pipe, and space separated rows all work, a header row is detected automatically, and `#` starts a comment. Needs at least 2 groups, at least 2 total observations more than there are groups, and some within-group variation. Up to 200000 values and 1000 groups.";
const FORMAT_DESC: &str = "How to read the pasted rows. auto (default) infers the shape: a `name,n,mean,sd` table is read as summary, a two-column table whose first column repeats non-numeric labels is read as long, anything else is read as wide. Set long, wide, or summary explicitly when the guess is wrong — most often with numeric group labels like `1,23.5`, which auto reads as wide but you may mean as long.";
const DELIMITER_DESC: &str = "How each row is split into columns. auto (default) picks the separator that appears consistently across the rows, preferring tab, then semicolon, then pipe, then comma, and falling back to runs of whitespace. Force one when a label itself contains the auto-detected separator — for example choose tab for `Site A, north<TAB>12` rows.";
const HEADER_DESC: &str = "Whether the first row names the columns or groups rather than holding data. auto (default) treats it as a header when its cells are not all numeric, and then uses those names as the group names in wide format. yes always skips it, no always treats it as data — use no when your groups really are labelled with numbers.";
const ALPHA_DESC: &str = "Significance level for the test, between 0.0001 and 0.5. Default 0.05. Sets the critical F reported next to the F statistic, decides the reject / fail-to-reject verdict, and is the family-wise error rate used for the post-hoc confidence intervals and adjusted p-values. Use 0.01 for a stricter test, 0.1 for an exploratory one.";
const DECIMALS_DESC: &str = "Decimal places for every number in the text and table output, 0-10. Default 4, which matches how ANOVA tables are usually reported. p-values smaller than the chosen resolution print as `< 0.0001` rather than rounding to 0. Also controls the rounding applied to json output.";
const POSTHOC_DESC: &str = "Pairwise follow-up test run when the omnibus F says at least one mean differs, since F alone does not say which pair. none (default) reports only the ANOVA table. tukey is Tukey's HSD, the usual choice for all pairwise comparisons with equal variances, and reports a q statistic with family-wise confidence intervals. lsd is Fisher's LSD, unadjusted t-tests with the pooled error term — the most powerful and the least protected. bonferroni multiplies each p-value by the number of pairs, simple and conservative. holm is the step-down Bonferroni: same protection, uniformly more power.";
const OUTPUT_DESC: &str = "Output format. summary (default) is a readable report: per-group n / mean / sd / sem, the full ANOVA table, F with its p-value and critical value, the reject verdict, eta-squared and omega-squared effect sizes, Levene's homogeneity-of-variance test, Welch's unequal-variance ANOVA, and any post-hoc pairs. table returns the same content as GitHub-flavoured markdown tables you can paste into a document or issue. json returns the whole result as a machine-readable object with every statistic rounded to decimals.";

/// Single source for the chat schema, the CLI, and (via
/// `scripts/sync-tool-manifest.py`) the page form's controls.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(DATA_DESC))
        .param(
            Param::enumv("format", ["auto", "long", "wide", "summary"])
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
            Param::enumv("posthoc", ["none", "tukey", "lsd", "bonferroni", "holm"])
                .default("none")
                .describe(POSTHOC_DESC),
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
    name = "gizza-ai/anova",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Run a one-way ANOVA on pasted groups: full ANOVA table, F and p, effect sizes, Levene and Welch checks, and post-hoc pairs",
    skill(
        description = "Run a one-way analysis of variance on pasted data and report whether the group means differ. Accepts long `group,value` rows, wide one-column-per-group tables with unequal group sizes, or per-group `name,n,mean,sd` summary statistics, and auto-detects the shape, the delimiter, and a header row. Returns per-group descriptive statistics, the between/within/total sums of squares, degrees of freedom and mean squares, the F statistic with its exact right-tail p-value and the critical F at your alpha, a reject / fail-to-reject verdict, eta-squared and omega-squared effect sizes, Levene's (Brown-Forsythe) homogeneity-of-variance test, Welch's unequal-variance ANOVA, and optional pairwise post-hoc comparisons (Tukey HSD, Fisher's LSD, Bonferroni, or Holm) with adjusted p-values and confidence intervals. Options control the input format, delimiter, header handling, significance level, decimal precision, post-hoc method, and whether to return a readable summary, markdown tables, or JSON. All statistics, including the F and studentized-range distributions, are computed locally in pure Rust; no data leaves the machine and no statistics service is called.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "anova", |a: Args| {
            gizza_ai_anova_core::run(
                &a.data,
                &a.format,
                &a.delimiter,
                &a.header,
                a.alpha,
                a.decimals as f64,
                &a.posthoc,
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
        assert_eq!(props.len(), 8, "parameter count changed");
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
        let a: Args = serde_json::from_str(r#"{"data":"5,5,8\n6,7,11\n9,9,13"}"#).unwrap();
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &schema["properties"];
        assert_eq!(a.format, props["format"]["default"]);
        assert_eq!(a.delimiter, props["delimiter"]["default"]);
        assert_eq!(a.header, props["header"]["default"]);
        assert_eq!(a.alpha, props["alpha"]["default"].as_f64().unwrap());
        assert_eq!(
            u64::from(a.decimals),
            props["decimals"]["default"].as_u64().unwrap()
        );
        assert_eq!(a.posthoc, props["posthoc"]["default"]);
        assert_eq!(a.output, props["output"]["default"]);
    }

    /// The defaulted Args reach the core and produce the documented headline
    /// line, so a default drifting out of the core's accepted set is caught.
    #[test]
    fn defaulted_args_run_through_the_core() {
        let a: Args =
            serde_json::from_str(r#"{"data":"5,5,8\n6,7,11\n9,9,13\n9,10,13\n11,11,14"}"#).unwrap();
        let out = gizza_ai_anova_core::run(
            &a.data,
            &a.format,
            &a.delimiter,
            &a.header,
            a.alpha,
            a.decimals as f64,
            &a.posthoc,
            &a.output,
        )
        .unwrap();
        assert!(out.contains("F(2, 12) = 3.7371, p = 0.0547"), "{out}");
    }

    /// Every enum variant the descriptor advertises must be one the core
    /// accepts — an advertised-but-rejected option is a broken dropdown.
    #[test]
    fn every_advertised_enum_variant_is_accepted_by_the_core() {
        const WIDE: &str = "5,5,8\n6,7,11\n9,9,13\n9,10,13\n11,11,14";
        const LONG: &str = "Control,5\nControl,6\nControl,9\nDrug,8\nDrug,11\nDrug,13";
        const SUMMARY: &str = "Control,5,8.0,2.4495\nDrug A,5,8.4,2.4083\nDrug B,5,11.8,2.3875";
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let variants = |p: &str| -> Vec<String> {
            schema["properties"][p]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        };
        for v in variants("format") {
            let data = match v.as_str() {
                "long" => LONG,
                "summary" => SUMMARY,
                _ => WIDE,
            };
            gizza_ai_anova_core::run(data, &v, "auto", "auto", 0.05, 4.0, "none", "summary")
                .unwrap_or_else(|e| panic!("format={v}: {e}"));
        }
        for v in variants("delimiter") {
            let data = match v.as_str() {
                "tab" => "5\t5\t8\n6\t7\t11\n9\t9\t13\n9\t10\t13\n11\t11\t14",
                "semicolon" => "5;5;8\n6;7;11\n9;9;13\n9;10;13\n11;11;14",
                "pipe" => "5|5|8\n6|7|11\n9|9|13\n9|10|13\n11|11|14",
                "space" => "5 5 8\n6 7 11\n9 9 13\n9 10 13\n11 11 14",
                _ => WIDE,
            };
            gizza_ai_anova_core::run(data, "wide", &v, "no", 0.05, 4.0, "none", "summary")
                .unwrap_or_else(|e| panic!("delimiter={v}: {e}"));
        }
        for v in variants("header") {
            gizza_ai_anova_core::run(WIDE, "wide", "comma", &v, 0.05, 4.0, "none", "summary")
                .unwrap_or_else(|e| panic!("header={v}: {e}"));
        }
        for v in variants("posthoc") {
            gizza_ai_anova_core::run(WIDE, "auto", "auto", "auto", 0.05, 4.0, &v, "summary")
                .unwrap_or_else(|e| panic!("posthoc={v}: {e}"));
        }
        for v in variants("output") {
            gizza_ai_anova_core::run(WIDE, "auto", "auto", "auto", 0.05, 4.0, "none", &v)
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
                "format": {
                    "type": "string",
                    "enum": ["auto", "long", "wide", "summary"],
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
                "posthoc": {
                    "type": "string",
                    "enum": ["none", "tukey", "lsd", "bonferroni", "holm"],
                    "default": "none",
                    "description": POSTHOC_DESC
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
