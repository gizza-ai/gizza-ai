//! gizza-ai/percentile-rank-calculator — chat skill block on the shared tool abstraction.
//! Reports where one or more values fall inside a reference dataset: percentile rank
//! (four standard tie-handling methods), below/equal/above counts, quartile, z-score,
//! and an optional dataset summary. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_percentile_rank_calculator_core::{decimals_from, report, Method, Options};
#[cfg(test)]
use gizza_ai_percentile_rank_calculator_core::{MAX_DATA_POINTS, MAX_VALUES};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    values: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default = "default_include_stats")]
    include_stats: bool,
}

fn default_method() -> String {
    "weak".into()
}
fn default_decimals() -> i64 {
    2
}
fn default_include_stats() -> bool {
    true
}

fn run_tool(a: Args) -> Result<String, String> {
    let opts = Options {
        method: Method::parse(&a.method)?,
        decimals: decimals_from(a.decimals)?,
        include_stats: a.include_stats,
    };
    report(&a.data, &a.values, &opts)
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The reference dataset: the numbers the target values are compared against, separated by commas, spaces, semicolons, or newlines (for example '6, 12, 13, 17, 17, 18'). Order does not matter — the list is sorted internally. Up to 10000 numbers per run."),
        )
        .param(
            Param::string("values")
                .required()
                .describe("The value or values whose percentile rank you want, separated by commas, spaces, semicolons, or newlines (for example '25' or '25, 30'). A value need not appear in the dataset; anything below the minimum ranks at 0 and anything above the maximum ranks at 100. Up to 100 values per run."),
        )
        .param(
            Param::enumv("method", ["weak", "strict", "mean", "rank"])
                .default("weak")
                .describe("How tied values are handled, matching SciPy percentileofscore: 'weak' (default) is count(<= value)/N x 100, the formula most online percentile-rank calculators use; 'strict' is count(< value)/N x 100; 'mean' is the midpoint of strict and weak, so ties split evenly; 'rank' is the average ranking of tied values. All four agree when the value is not tied with any dataset entry."),
        )
        .param(
            Param::integer("decimals")
                .default(2)
                .min(0.0)
                .max(6.0)
                .describe("Decimal places used when rounding the percentile ranks, z-scores, and summary statistics, 0 to 6. Trailing zeros are trimmed, so 64.70 prints as 64.7. Default 2."),
        )
        .param(
            Param::boolean("include_stats")
                .default(true)
                .describe("Append a dataset summary line with n, min, max, range, mean, median, sample standard deviation, Q1, Q3 and IQR. Set false for just the ranked values. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/percentile-rank-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Percentile rank of one or more values within a reference dataset.",
    skill(
        description = "Find the percentile rank of one or more values inside a reference dataset — how a test score, salary, response time or measurement compares with the rest of the numbers. Give the dataset and the value(s) to locate; the tool returns each value's percentile rank plus how many dataset entries fall below, equal and above it, its quartile and its z-score, and an optional dataset summary (n, min, max, range, mean, median, sample standard deviation, Q1, Q3, IQR). Four standard tie-handling methods are available (weak, strict, mean, rank) matching SciPy percentileofscore. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "percentile-rank-calculator", |a: Args| {
            run_tool(a).map_err(SkillError::InvalidArgs)
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The reference dataset: the numbers the target values are compared against, separated by commas, spaces, semicolons, or newlines (for example '6, 12, 13, 17, 17, 18'). Order does not matter — the list is sorted internally. Up to 10000 numbers per run." },
                    "values": { "type": "string", "description": "The value or values whose percentile rank you want, separated by commas, spaces, semicolons, or newlines (for example '25' or '25, 30'). A value need not appear in the dataset; anything below the minimum ranks at 0 and anything above the maximum ranks at 100. Up to 100 values per run." },
                    "method": { "type": "string", "enum": ["weak", "strict", "mean", "rank"], "default": "weak", "description": "How tied values are handled, matching SciPy percentileofscore: 'weak' (default) is count(<= value)/N x 100, the formula most online percentile-rank calculators use; 'strict' is count(< value)/N x 100; 'mean' is the midpoint of strict and weak, so ties split evenly; 'rank' is the average ranking of tied values. All four agree when the value is not tied with any dataset entry." },
                    "decimals": { "type": "integer", "default": 2, "minimum": 0, "maximum": 6, "description": "Decimal places used when rounding the percentile ranks, z-scores, and summary statistics, 0 to 6. Trailing zeros are trimmed, so 64.70 prints as 64.7. Default 2." },
                    "include_stats": { "type": "boolean", "default": true, "description": "Append a dataset summary line with n, min, max, range, mean, median, sample standard deviation, Q1, Q3 and IQR. Set false for just the ranked values. Default true." }
                },
                "required": ["data", "values"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn descriptor_describes_every_param() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = derived["properties"].as_object().unwrap();
        assert_eq!(props.len(), 5);
        for (name, prop) in props {
            let d = prop["description"].as_str().unwrap_or("");
            assert!(d.len() > 20, "param '{name}' needs a real .describe()");
        }
    }

    #[test]
    fn run_tool_defaults_match_the_page_defaults() {
        let got = run_tool(Args {
            data: "10, 20, 30, 40".into(),
            values: "30".into(),
            method: default_method(),
            decimals: default_decimals(),
            include_stats: default_include_stats(),
        })
        .unwrap();
        assert!(got.contains("30 → 75"), "got {got}");
        assert!(got.contains("Dataset summary"), "got {got}");
    }

    #[test]
    fn run_tool_rejects_an_unknown_method() {
        let err = run_tool(Args {
            data: "1,2,3".into(),
            values: "2".into(),
            method: "median".into(),
            decimals: 2,
            include_stats: true,
        })
        .unwrap_err();
        assert!(err.contains("Unknown method"), "got {err}");
    }

    #[test]
    fn caps_are_reflected_in_the_param_docs() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let data = derived["properties"]["data"]["description"]
            .as_str()
            .unwrap();
        assert!(data.contains(&MAX_DATA_POINTS.to_string()));
        let values = derived["properties"]["values"]["description"]
            .as_str()
            .unwrap();
        assert!(values.contains(&MAX_VALUES.to_string()));
    }
}
