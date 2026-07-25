//! gizza-ai/monte-carlo-sim — chat skill block on the shared tool abstraction.
//! Run a Monte Carlo simulation of a user-defined model: declare uncertain input
//! variables as probability distributions, combine them in a formula, sample N
//! trials, and report the outcome distribution (mean/std-dev/min/max),
//! percentiles, an optional probability of meeting a target, and a text
//! histogram. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to run_skill. Pure + deterministic
//! (seeded SplitMix64 PRNG, no `getrandom`) → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_monte_carlo_sim_core::report;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    variables: String,
    model: String,
    #[serde(default = "default_trials")]
    trials: i64,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default)]
    threshold: Option<f64>,
    #[serde(default = "default_direction")]
    threshold_direction: String,
    #[serde(default = "default_bins")]
    histogram_bins: i64,
}

fn default_trials() -> i64 {
    10_000
}
fn default_seed() -> u64 {
    42
}
fn default_direction() -> String {
    "at-or-above".to_string()
}
fn default_bins() -> i64 {
    20
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("variables")
                .required()
                .describe("Uncertain input variables, one 'name = distribution(args)' per line. Supported distributions: normal(mean, std_dev), uniform(min, max), triangular(min, mode, max), lognormal(mu, sigma), constant(value). Blank lines and '#' comments are ignored. E.g. 'revenue = normal(1000, 200)\\ncost = uniform(400, 700)'."),
        )
        .param(
            Param::string("model")
                .required()
                .describe("The output formula combining the variables, evaluated once per trial. Supports + - * / ^, parentheses, functions (sin, cos, sqrt, abs, ln, exp, min, max, …) and constants (pi, e). Every name must be a declared variable. E.g. 'revenue - cost'."),
        )
        .param(
            Param::integer("trials")
                .default(10_000)
                .min(100.0)
                .max(1_000_000.0)
                .describe("Number of random trials to sample (default 10000; 100–1000000). More trials tighten the percentile estimates."),
        )
        .param(
            Param::integer("seed")
                .default(42)
                .min(0.0)
                .describe("Random seed for the deterministic PRNG (default 42). The same seed + inputs always produce identical results; change it to draw a different sample."),
        )
        .param(
            Param::number("threshold")
                .describe("Optional target value. When set, the report adds the probability that the outcome meets this target (P80/P95-style confidence). Omit to skip."),
        )
        .param(
            Param::enumv("threshold_direction", ["at-or-above", "at-or-below"])
                .default("at-or-above")
                .describe("Which side of the threshold counts as a success: 'at-or-above' = P(outcome >= threshold), 'at-or-below' = P(outcome <= threshold). Only used when threshold is set."),
        )
        .param(
            Param::integer("histogram_bins")
                .default(20)
                .min(0.0)
                .max(40.0)
                .describe("Number of rows in the text histogram of the outcome distribution (default 20; 0 hides the histogram; max 40)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/monte-carlo-sim",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Monte Carlo simulation of a user-defined probabilistic model",
    skill(
        description = "Run a Monte Carlo simulation of a user-defined model. Declare uncertain input variables as probability distributions (one 'name = distribution(args)' per line — normal, uniform, triangular, lognormal, constant), combine them in a model formula (e.g. 'revenue - cost'), and sample `trials` outcomes (default 10000). Returns the outcome distribution (mean, std dev, min, max), percentiles P5–P99 (read as confidence levels, e.g. P90), an optional probability of meeting a `threshold` target (at-or-above / at-or-below), and a text histogram (`histogram_bins` rows, 0 to hide). Deterministic given a `seed`. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "monte-carlo-sim", |a: Args| {
            report(
                &a.variables,
                &a.model,
                a.trials,
                a.seed,
                a.threshold,
                &a.threshold_direction,
                a.histogram_bins,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "variables": { "type": "string", "description": "Uncertain input variables, one 'name = distribution(args)' per line. Supported distributions: normal(mean, std_dev), uniform(min, max), triangular(min, mode, max), lognormal(mu, sigma), constant(value). Blank lines and '#' comments are ignored. E.g. 'revenue = normal(1000, 200)\\ncost = uniform(400, 700)'." },
                    "model": { "type": "string", "description": "The output formula combining the variables, evaluated once per trial. Supports + - * / ^, parentheses, functions (sin, cos, sqrt, abs, ln, exp, min, max, …) and constants (pi, e). Every name must be a declared variable. E.g. 'revenue - cost'." },
                    "trials": { "type": "integer", "minimum": 100, "maximum": 1000000, "default": 10000, "description": "Number of random trials to sample (default 10000; 100–1000000). More trials tighten the percentile estimates." },
                    "seed": { "type": "integer", "minimum": 0, "default": 42, "description": "Random seed for the deterministic PRNG (default 42). The same seed + inputs always produce identical results; change it to draw a different sample." },
                    "threshold": { "type": "number", "description": "Optional target value. When set, the report adds the probability that the outcome meets this target (P80/P95-style confidence). Omit to skip." },
                    "threshold_direction": { "type": "string", "enum": ["at-or-above", "at-or-below"], "default": "at-or-above", "description": "Which side of the threshold counts as a success: 'at-or-above' = P(outcome >= threshold), 'at-or-below' = P(outcome <= threshold). Only used when threshold is set." },
                    "histogram_bins": { "type": "integer", "minimum": 0, "maximum": 40, "default": 20, "description": "Number of rows in the text histogram of the outcome distribution (default 20; 0 hides the histogram; max 40)." }
                },
                "required": ["variables", "model"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
