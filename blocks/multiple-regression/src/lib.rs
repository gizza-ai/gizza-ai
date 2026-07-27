//! gizza-ai/multiple-regression — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Fits an ordinary
//! least-squares multiple linear regression on a pasted data matrix and reports
//! the coefficient table (estimate / std error / t / p / CI), R², adjusted R²,
//! residual standard error and the overall F-test. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_response")]
    response: String,
    #[serde(default)]
    labels: String,
    #[serde(default = "default_true")]
    intercept: bool,
    #[serde(default = "default_conf")]
    conf_level: f64,
    #[serde(default = "default_format")]
    format: String,
}
fn default_response() -> String {
    "last".into()
}
fn default_true() -> bool {
    true
}
fn default_conf() -> f64 {
    0.95
}
fn default_format() -> String {
    "text".into()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "The data matrix: one observation per line, columns separated by commas, tabs, semicolons or spaces. Every row must have the same number of columns and at least two columns (one or more predictors plus the response), e.g. '1,6\\n2,8\\n3,11'.",
        ))
        .param(
            Param::string("response")
                .default("last")
                .describe(
                    "Which column is the response (dependent) variable Y: 'last' (default, the rightmost column), 'first', or a 1-based column number. Every other column is treated as a predictor.",
                ),
        )
        .param(Param::string("labels").describe(
            "Optional comma-separated column names, one per column in data order (e.g. 'sqft,rooms,price'). They name the coefficient rows and the response. Default v1, v2, … .",
        ))
        .param(
            Param::boolean("intercept")
                .default(true)
                .describe(
                    "Fit a constant (Intercept) term (default true). Set false to force the regression line through the origin (zero Y-intercept).",
                ),
        )
        .param(
            Param::number("conf_level")
                .min(0.5)
                .max(0.9999)
                .default(0.95)
                .describe(
                    "Confidence level for the coefficient confidence intervals, between 0.5 and 0.9999 (default 0.95). The two-tailed significance level is α = 1 − conf_level.",
                ),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe(
                    "Output format: 'text' (default) = a formatted regression summary with the equation, coefficient table and model statistics; 'json' = the full result as JSON, additionally including the per-observation fitted values and residuals.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/multiple-regression",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "OLS multiple linear regression: coefficients, R², p-values, F-test",
    skill(
        description = "Fit an ordinary least-squares multiple linear regression on a pasted data matrix (one observation per line; columns split on commas, tabs, semicolons or spaces). Choose which column is the response with response ('last' default, 'first', or a 1-based index) — every other column is a predictor — and optionally name the columns with labels. Returns the fitted equation and a coefficient table (estimate, standard error, t-statistic, two-tailed p-value and confidence interval at conf_level) plus R², adjusted R², residual standard error with its degrees of freedom, and the overall F-test with its p-value. Set intercept=false to force a zero Y-intercept. format='json' additionally returns the per-observation fitted values and residuals. Predictors must be numeric and not perfectly collinear. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "multiple-regression", |a: Args| {
            gizza_ai_multiple_regression_core::run(
                &a.data,
                &a.response,
                &a.labels,
                a.intercept,
                a.conf_level,
                &a.format,
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The data matrix: one observation per line, columns separated by commas, tabs, semicolons or spaces. Every row must have the same number of columns and at least two columns (one or more predictors plus the response), e.g. '1,6\\n2,8\\n3,11'." },
                    "response": { "type": "string", "default": "last", "description": "Which column is the response (dependent) variable Y: 'last' (default, the rightmost column), 'first', or a 1-based column number. Every other column is treated as a predictor." },
                    "labels": { "type": "string", "description": "Optional comma-separated column names, one per column in data order (e.g. 'sqft,rooms,price'). They name the coefficient rows and the response. Default v1, v2, … ." },
                    "intercept": { "type": "boolean", "default": true, "description": "Fit a constant (Intercept) term (default true). Set false to force the regression line through the origin (zero Y-intercept)." },
                    "conf_level": { "type": "number", "minimum": 0.5, "maximum": 0.9999, "default": 0.95, "description": "Confidence level for the coefficient confidence intervals, between 0.5 and 0.9999 (default 0.95). The two-tailed significance level is α = 1 − conf_level." },
                    "format": { "type": "string", "enum": ["text", "json"], "default": "text", "description": "Output format: 'text' (default) = a formatted regression summary with the equation, coefficient table and model statistics; 'json' = the full result as JSON, additionally including the per-observation fitted values and residuals." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
