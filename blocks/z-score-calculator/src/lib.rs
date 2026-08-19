//! gizza-ai/z-score-calculator — standard scores against a known normal
//! distribution: raw→z with percentile and tail probabilities, z→raw, the
//! inverse normal (p→critical z), the area between two bounds, and standardizing
//! a pasted dataset. Thin wrapper; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_z_score_calculator_core::summary;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    values: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    mean: f64,
    #[serde(default = "default_std_dev")]
    std_dev: f64,
    #[serde(default = "default_n")]
    n: i64,
    #[serde(default)]
    sample: bool,
    #[serde(default = "default_decimals")]
    decimals: i64,
}

fn default_mode() -> String {
    "score".to_string()
}
fn default_std_dev() -> f64 {
    1.0
}
fn default_n() -> i64 {
    1
}
fn default_decimals() -> i64 {
    6
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("values").required().describe(
                "The numbers to work on, separated by spaces, commas, semicolons or newlines (e.g. '130' or '85, 100, 115'). What they mean depends on mode: raw scores for score and dataset, z-scores for raw, left-tail probabilities in (0,1) for critical, and exactly two bounds for between. Maximum 10000 values.",
            ),
        )
        .param(
            Param::enumv("mode", ["score", "raw", "critical", "between", "dataset"])
                .default("score")
                .describe(
                    "Which direction to compute. score (default): raw score -> z = (x - mean) / std_dev, plus percentile and tail probabilities. raw: z-score -> x = mean + z * std_dev. critical: left-tail probability -> the z with that area below it (e.g. 0.975 -> 1.959964). between: exactly two bounds -> the area between them. dataset: derive the mean and standard deviation from the pasted numbers themselves, then standardize them.",
                ),
        )
        .param(
            Param::number("mean").default(0.0).describe(
                "The population mean (mu) of the reference distribution, e.g. 100 for IQ. Default 0. Ignored in dataset mode, which derives it from the values.",
            ),
        )
        .param(
            Param::number("std_dev").default(1.0).min(0.0).describe(
                "The population standard deviation (sigma) of the reference distribution, e.g. 15 for IQ. Must be greater than 0. Default 1, so the defaults describe the standard normal curve. Ignored in dataset mode, which derives it from the values.",
            ),
        )
        .param(
            Param::integer("n")
                .default(1)
                .min(1.0)
                .max(1000000.0)
                .describe(
                    "Sample size behind a sample mean. At the default 1 you get the ordinary z-score. Above 1 the divisor becomes the standard error std_dev / sqrt(n), which is the form used to test a sample mean against a known population mean (e.g. mean=100, std_dev=15, n=9 gives a standard error of 5).",
                ),
        )
        .param(
            Param::boolean("sample").default(false).describe(
                "In dataset mode, derive the SAMPLE standard deviation (divide by N-1) instead of the population one (divide by N). Needs at least 2 values. Default false. Ignored in every other mode.",
            ),
        )
        .param(
            Param::integer("decimals")
                .default(6)
                .min(0.0)
                .max(12.0)
                .describe(
                    "Decimal places for every number in the output, 0 to 12. Default 6. Probabilities too small to survive the rounding (deep tails such as P(Z > 6)) keep that many significant digits instead of rounding to zero.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Shared argument validation for the whole-integer params, so chat, CLI and the
/// page all reject the same things with the same wording.
fn run_args(a: Args) -> Result<String, String> {
    if a.n < 1 {
        return Err(format!("sample size n must be at least 1 (got {})", a.n));
    }
    if !(0..=12).contains(&a.decimals) {
        return Err(format!(
            "decimals must be between 0 and 12 (got {})",
            a.decimals
        ));
    }
    summary(
        &a.values,
        &a.mode,
        a.mean,
        a.std_dev,
        a.n as u32,
        a.sample,
        a.decimals as u32,
    )
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/z-score-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Z-scores, percentiles and normal-curve probabilities for a known mean and standard deviation",
    skill(
        description = "Calculate standard scores (z-scores) against a normal distribution with a known mean and standard deviation, and read the probabilities off the curve. mode=score (default) turns one or many raw scores into z = (x - mean) / std_dev and reports the percentile, the left tail P(X < x), the right tail P(X > x) and the two-tailed p-value. mode=raw inverts it (x = mean + z * std_dev), mode=critical turns a left-tail probability into its critical z (0.975 -> 1.959964), mode=between returns the area between exactly two bounds, and mode=dataset derives the mean and standard deviation from the pasted numbers before standardizing them. Set n above 1 to use the standard error std_dev / sqrt(n) for a sample mean, and decimals to control precision. Values are separated by spaces, commas, semicolons or newlines. Runs locally, no data leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "z-score-calculator", |a: Args| {
            run_args(a).map_err(SkillError::InvalidArgs)
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
                    "values": { "type": "string", "description": "The numbers to work on, separated by spaces, commas, semicolons or newlines (e.g. '130' or '85, 100, 115'). What they mean depends on mode: raw scores for score and dataset, z-scores for raw, left-tail probabilities in (0,1) for critical, and exactly two bounds for between. Maximum 10000 values." },
                    "mode": { "type": "string", "enum": ["score", "raw", "critical", "between", "dataset"], "default": "score", "description": "Which direction to compute. score (default): raw score -> z = (x - mean) / std_dev, plus percentile and tail probabilities. raw: z-score -> x = mean + z * std_dev. critical: left-tail probability -> the z with that area below it (e.g. 0.975 -> 1.959964). between: exactly two bounds -> the area between them. dataset: derive the mean and standard deviation from the pasted numbers themselves, then standardize them." },
                    "mean": { "type": "number", "default": 0.0, "description": "The population mean (mu) of the reference distribution, e.g. 100 for IQ. Default 0. Ignored in dataset mode, which derives it from the values." },
                    "std_dev": { "type": "number", "default": 1.0, "minimum": 0, "description": "The population standard deviation (sigma) of the reference distribution, e.g. 15 for IQ. Must be greater than 0. Default 1, so the defaults describe the standard normal curve. Ignored in dataset mode, which derives it from the values." },
                    "n": { "type": "integer", "default": 1, "minimum": 1, "maximum": 1000000, "description": "Sample size behind a sample mean. At the default 1 you get the ordinary z-score. Above 1 the divisor becomes the standard error std_dev / sqrt(n), which is the form used to test a sample mean against a known population mean (e.g. mean=100, std_dev=15, n=9 gives a standard error of 5)." },
                    "sample": { "type": "boolean", "default": false, "description": "In dataset mode, derive the SAMPLE standard deviation (divide by N-1) instead of the population one (divide by N). Needs at least 2 values. Default false. Ignored in every other mode." },
                    "decimals": { "type": "integer", "default": 6, "minimum": 0, "maximum": 12, "description": "Decimal places for every number in the output, 0 to 12. Default 6. Probabilities too small to survive the rounding (deep tails such as P(Z > 6)) keep that many significant digits instead of rounding to zero." }
                },
                "required": ["values"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn defaults_describe_the_standard_normal_curve() {
        let a: Args = serde_json::from_str(r#"{"values":"1.96"}"#).unwrap();
        let out = run_args(a).unwrap();
        assert!(out.contains("z = 1.96"), "{out}");
        assert!(out.contains("Mean (μ) = 0"), "{out}");
    }

    #[test]
    fn out_of_range_decimals_is_rejected() {
        let a: Args = serde_json::from_str(r#"{"values":"1","decimals":99}"#).unwrap();
        let e = run_args(a).unwrap_err();
        assert!(e.contains("decimals must be between 0 and 12"), "{e}");
    }

    #[test]
    fn zero_sample_size_is_rejected() {
        let a: Args = serde_json::from_str(r#"{"values":"1","n":0}"#).unwrap();
        let e = run_args(a).unwrap_err();
        assert!(e.contains("at least 1"), "{e}");
    }
}
