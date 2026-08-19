//! gizza-ai/stepwise-feature-selection — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Runs forward, backward or
//! bidirectional stepwise OLS regression over a pasted numeric table and reports
//! the selection path, the surviving model and a null-vs-selected-vs-full
//! comparison. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_target")]
    target: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default = "default_criterion")]
    criterion: String,
    #[serde(default = "default_alpha_enter")]
    alpha_enter: f64,
    #[serde(default = "default_alpha_remove")]
    alpha_remove: f64,
    #[serde(default)]
    force: String,
    #[serde(default)]
    labels: String,
    #[serde(default)]
    header: bool,
    #[serde(default = "default_decimals")]
    decimals: u32,
}
fn default_target() -> String {
    "last".into()
}
fn default_direction() -> String {
    "both".into()
}
fn default_criterion() -> String {
    "aic".into()
}
fn default_alpha_enter() -> f64 {
    0.05
}
fn default_alpha_remove() -> f64 {
    0.10
}
fn default_decimals() -> u32 {
    4
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "The data table: one observation per line, columns separated by commas, tabs, semicolons or spaces. Every row needs the same number of columns, at least 2 columns (the target plus one candidate predictor) and at least 3 rows. Blank lines and lines starting with '#' are ignored. Max 20000 rows and 60 columns, e.g. '1,5,5.1\\n2,3,7.9\\n3,8,11.2'.",
        ))
        .param(Param::string("target").default("last").describe(
            "Which column is the target (dependent variable) to explain: 'last' (default, the rightmost column), 'first', a 1-based column number, or a column name from the header row or labels. Every other column becomes a candidate predictor.",
        ))
        .param(
            Param::enumv("direction", ["forward", "backward", "both"])
                .default("both")
                .describe(
                    "Search direction: 'forward' starts from the intercept-only model and only adds predictors; 'backward' starts from all predictors and only removes them; 'both' (default) is bidirectional stepwise, which may add or remove at every step and can drop a term that became redundant.",
                ),
        )
        .param(
            Param::enumv("criterion", ["aic", "bic", "aicc", "pvalue"])
                .default("aic")
                .describe(
                    "What a move is judged on: 'aic' (default), 'bic' (heavier size penalty, so smaller models), 'aicc' (AIC corrected for small samples), or 'pvalue' (enter/drop on the alpha_enter and alpha_remove thresholds instead of an information criterion). AIC/BIC use the scale-free regression form n·ln(RSS/n) + penalty·k, the same convention R's step() reports.",
                ),
        )
        .param(
            Param::number("alpha_enter")
                .min(0.0001)
                .max(0.9999)
                .default(0.05)
                .describe(
                    "p-value a candidate must beat to ENTER the model when criterion='pvalue' (default 0.05). Ignored by the aic/bic/aicc criteria.",
                ),
        )
        .param(
            Param::number("alpha_remove")
                .min(0.0001)
                .max(0.9999)
                .default(0.1)
                .describe(
                    "p-value above which a term is REMOVED from the model when criterion='pvalue' (default 0.1). Must be at least alpha_enter, otherwise a predictor would be dropped the moment it enters. Ignored by the aic/bic/aicc criteria.",
                ),
        )
        .param(Param::string("force").describe(
            "Optional comma-separated predictors that must stay in every candidate model, by column name or 1-based index (e.g. 'ads' or '2'). They are never removed and seed the starting model for forward and bidirectional searches. Default: none forced.",
        ))
        .param(Param::string("labels").describe(
            "Optional comma-separated column names, one per column in data order (e.g. 'ads,price,temp,sales'). They name the predictors in the report and can be used by target and force. Overrides the header row. Default v1, v2, … .",
        ))
        .param(Param::boolean("header").default(false).describe(
            "Treat the first row of data as column names instead of numbers (default false). The labels parameter, when given, overrides these names.",
        ))
        .param(
            Param::integer("decimals")
                .min(0.0)
                .max(10.0)
                .default(4)
                .describe("Decimal places for every number in the report, 0 to 10 (default 4)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/stepwise-feature-selection",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Stepwise regression: pick the predictors that best explain a target by AIC/BIC",
    skill(
        description = "Run forward, backward or bidirectional stepwise ordinary-least-squares regression over a pasted numeric table to work out which predictor columns actually explain the target. One column is the target (target: 'last' default, 'first', a 1-based index, or a name from a header row or labels) and every other column is a candidate predictor; predictors are added or removed one at a time until no single move improves the chosen criterion. Pick direction='forward' (add only), 'backward' (drop only) or 'both' (default, bidirectional) and criterion='aic' (default), 'bic', 'aicc' or 'pvalue' (which uses the alpha_enter / alpha_remove thresholds instead). Use force to pin predictors into every candidate model. Returns the step-by-step selection path with the criterion after each move, the selected model's fitted equation and coefficient table (estimate, standard error, t, p), its R², adjusted R², RMSE, AIC/BIC/AICc and overall F-test, the list of dropped predictors, and an intercept-only vs selected vs all-predictors comparison. Accepts up to 20000 rows and 60 columns; exactly collinear subsets are skipped rather than failing. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "stepwise-feature-selection", |a: Args| {
            gizza_ai_stepwise_feature_selection_core::run(
                &a.data,
                &a.target,
                &a.direction,
                &a.criterion,
                a.alpha_enter,
                a.alpha_remove,
                &a.force,
                &a.labels,
                a.header,
                a.decimals,
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
                    "data": { "type": "string", "description": "The data table: one observation per line, columns separated by commas, tabs, semicolons or spaces. Every row needs the same number of columns, at least 2 columns (the target plus one candidate predictor) and at least 3 rows. Blank lines and lines starting with '#' are ignored. Max 20000 rows and 60 columns, e.g. '1,5,5.1\\n2,3,7.9\\n3,8,11.2'." },
                    "target": { "type": "string", "default": "last", "description": "Which column is the target (dependent variable) to explain: 'last' (default, the rightmost column), 'first', a 1-based column number, or a column name from the header row or labels. Every other column becomes a candidate predictor." },
                    "direction": { "type": "string", "enum": ["forward", "backward", "both"], "default": "both", "description": "Search direction: 'forward' starts from the intercept-only model and only adds predictors; 'backward' starts from all predictors and only removes them; 'both' (default) is bidirectional stepwise, which may add or remove at every step and can drop a term that became redundant." },
                    "criterion": { "type": "string", "enum": ["aic", "bic", "aicc", "pvalue"], "default": "aic", "description": "What a move is judged on: 'aic' (default), 'bic' (heavier size penalty, so smaller models), 'aicc' (AIC corrected for small samples), or 'pvalue' (enter/drop on the alpha_enter and alpha_remove thresholds instead of an information criterion). AIC/BIC use the scale-free regression form n·ln(RSS/n) + penalty·k, the same convention R's step() reports." },
                    "alpha_enter": { "type": "number", "minimum": 0.0001, "maximum": 0.9999, "default": 0.05, "description": "p-value a candidate must beat to ENTER the model when criterion='pvalue' (default 0.05). Ignored by the aic/bic/aicc criteria." },
                    "alpha_remove": { "type": "number", "minimum": 0.0001, "maximum": 0.9999, "default": 0.1, "description": "p-value above which a term is REMOVED from the model when criterion='pvalue' (default 0.1). Must be at least alpha_enter, otherwise a predictor would be dropped the moment it enters. Ignored by the aic/bic/aicc criteria." },
                    "force": { "type": "string", "description": "Optional comma-separated predictors that must stay in every candidate model, by column name or 1-based index (e.g. 'ads' or '2'). They are never removed and seed the starting model for forward and bidirectional searches. Default: none forced." },
                    "labels": { "type": "string", "description": "Optional comma-separated column names, one per column in data order (e.g. 'ads,price,temp,sales'). They name the predictors in the report and can be used by target and force. Overrides the header row. Default v1, v2, … ." },
                    "header": { "type": "boolean", "default": false, "description": "Treat the first row of data as column names instead of numbers (default false). The labels parameter, when given, overrides these names." },
                    "decimals": { "type": "integer", "minimum": 0, "maximum": 10, "default": 4, "description": "Decimal places for every number in the report, 0 to 10 (default 4)." }
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
