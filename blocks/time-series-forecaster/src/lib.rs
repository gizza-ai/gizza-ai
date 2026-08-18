//! gizza-ai/time-series-forecaster — forecast a univariate time series with exponential
//! smoothing (SES, Holt, damped trend, Holt-Winters) plus prediction intervals.
//! Thin chat-skill wrapper around the pure core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_time_series_forecaster_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_model")]
    model: String,
    #[serde(default = "default_horizon")]
    horizon: f64,
    #[serde(default)]
    season_length: f64,
    #[serde(default)]
    alpha: f64,
    #[serde(default)]
    beta: f64,
    #[serde(default)]
    gamma: f64,
    #[serde(default)]
    phi: f64,
    #[serde(default = "default_confidence")]
    confidence: String,
    #[serde(default)]
    show_fitted: bool,
    #[serde(default = "default_header")]
    header: String,
    #[serde(default = "default_decimals")]
    decimals: f64,
    #[serde(default = "default_format")]
    format: String,
}

fn default_model() -> String { "auto".to_string() }
fn default_horizon() -> f64 { 6.0 }
fn default_confidence() -> String { "95".to_string() }
fn default_header() -> String { "auto".to_string() }
fn default_decimals() -> f64 { 3.0 }
fn default_format() -> String { "text".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data").required().describe(
                "The historical series, oldest observation first. Paste one value per line, an optional label before the value (\"Jan,120\"), or the whole series on one comma/semicolon/space separated line. Values may carry currency symbols, percent signs or accounting-style (12.5) negatives. Minimum 3 observations, maximum 10000.",
            ),
        )
        .param(
            Param::enumv(
                "model",
                [
                    "auto",
                    "simple",
                    "holt",
                    "damped",
                    "holt-winters-additive",
                    "holt-winters-multiplicative",
                ],
            )
            .default("auto")
            .describe(
                "Exponential-smoothing model. auto (default) fits every applicable candidate and picks the best AICc; simple = simple exponential smoothing (level only); holt = additive linear trend; damped = trend damped by phi so long horizons flatten; holt-winters-additive = trend plus a constant-size seasonal cycle; holt-winters-multiplicative = trend plus a seasonal cycle that grows with the level (needs strictly positive values). Both Holt-Winters models require season_length of 2 or more.",
            ),
        )
        .param(
            Param::integer("horizon").default(6).min(1.0).max(240.0).describe(
                "How many future periods to forecast, 1 to 240. Default 6.",
            ),
        )
        .param(
            Param::integer("season_length").default(0).min(0.0).max(366.0).describe(
                "Observations per seasonal cycle: 12 for monthly data with a yearly cycle, 4 for quarterly, 7 for daily data with a weekly cycle. Use 0 (default) for a non-seasonal series. Seasonal models need at least two full cycles of history.",
            ),
        )
        .param(
            Param::number("alpha").default(0.0).min(0.0).max(1.0).describe(
                "Level smoothing weight between 0 and 1. Leave at 0 (default) to fit it automatically by minimising the sum of squared one-step-ahead errors. Higher values react faster to recent observations.",
            ),
        )
        .param(
            Param::number("beta").default(0.0).min(0.0).max(1.0).describe(
                "Trend smoothing weight between 0 and 1, used by holt, damped and both Holt-Winters models. Leave at 0 (default) to fit it automatically. Ignored by the simple model.",
            ),
        )
        .param(
            Param::number("gamma").default(0.0).min(0.0).max(1.0).describe(
                "Seasonal smoothing weight between 0 and 1, used by the Holt-Winters models. Leave at 0 (default) to fit it automatically. Ignored by non-seasonal models.",
            ),
        )
        .param(
            Param::number("phi").default(0.0).min(0.0).max(1.0).describe(
                "Trend damping factor between 0 and 1 for model=damped; values near 1 keep the trend, lower values flatten it quickly. Leave at 0 (default) to fit it automatically in the range 0.6 to 0.99. Ignored by other models.",
            ),
        )
        .param(
            Param::enumv("confidence", ["80", "90", "95", "99"]).default("95").describe(
                "Prediction-interval confidence level in percent: 80, 90, 95 (default) or 99. Wider levels give wider bands around each forecast.",
            ),
        )
        .param(
            Param::boolean("show_fitted").default(false).describe(
                "Include the per-period table of actual values, one-step-ahead fitted values and residuals (default false). Useful for checking how well the model tracked history.",
            ),
        )
        .param(
            Param::enumv("header", ["auto", "yes", "no"]).default("auto").describe(
                "How to treat a leading label row. auto (default) drops the first row only when its value field is not numeric; yes always drops it; no parses every row as data.",
            ),
        )
        .param(
            Param::integer("decimals").default(3).min(0.0).max(10.0).describe(
                "Decimal places for forecasts and accuracy metrics, 0 to 10. Default 3.",
            ),
        )
        .param(
            Param::enumv("format", ["text", "csv", "json"]).default("text").describe(
                "Output format: text report (model, smoothing parameters, accuracy metrics, forecast table with prediction intervals), csv sections, or structured json. Default text.",
            ),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

fn to_options(a: &Args) -> Options {
    Options {
        model: a.model.clone(),
        horizon: a.horizon.round() as i64,
        season_length: a.season_length.round() as i64,
        alpha: a.alpha,
        beta: a.beta,
        gamma: a.gamma,
        phi: a.phi,
        confidence: a.confidence.clone(),
        show_fitted: a.show_fitted,
        header: a.header.clone(),
        decimals: a.decimals.round() as i64,
        format: a.format.clone(),
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/time-series-forecaster",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Forecast a time series with exponential smoothing and prediction intervals",
    skill(
        description = "Forecast future values of a univariate time series using the exponential-smoothing (ETS) family: simple exponential smoothing, Holt's linear trend, damped trend, and Holt-Winters additive or multiplicative seasonality. Smoothing weights left at 0 are fitted automatically by a deterministic grid search that minimises the sum of squared one-step-ahead errors, and model=auto picks the best candidate by AICc. Returns the chosen model, its alpha/beta/gamma/phi weights, in-sample accuracy (MAE, RMSE, MAPE, MASE, residual sigma, AICc), and a forecast table with 80/90/95/99 percent prediction intervals, optionally alongside per-period fitted values and residuals, as text, CSV, or JSON. Accepts labelled rows, single-line series, currency and percent symbols; up to 10000 observations and a 240-period horizon.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "time-series-forecaster", |a: Args| {
            gizza_ai_time_series_forecaster_core::run(&a.data, &to_options(&a))
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
                    "data": { "type": "string", "description": "The historical series, oldest observation first. Paste one value per line, an optional label before the value (\"Jan,120\"), or the whole series on one comma/semicolon/space separated line. Values may carry currency symbols, percent signs or accounting-style (12.5) negatives. Minimum 3 observations, maximum 10000." },
                    "model": { "type": "string", "enum": ["auto", "simple", "holt", "damped", "holt-winters-additive", "holt-winters-multiplicative"], "default": "auto", "description": "Exponential-smoothing model. auto (default) fits every applicable candidate and picks the best AICc; simple = simple exponential smoothing (level only); holt = additive linear trend; damped = trend damped by phi so long horizons flatten; holt-winters-additive = trend plus a constant-size seasonal cycle; holt-winters-multiplicative = trend plus a seasonal cycle that grows with the level (needs strictly positive values). Both Holt-Winters models require season_length of 2 or more." },
                    "horizon": { "type": "integer", "default": 6, "minimum": 1, "maximum": 240, "description": "How many future periods to forecast, 1 to 240. Default 6." },
                    "season_length": { "type": "integer", "default": 0, "minimum": 0, "maximum": 366, "description": "Observations per seasonal cycle: 12 for monthly data with a yearly cycle, 4 for quarterly, 7 for daily data with a weekly cycle. Use 0 (default) for a non-seasonal series. Seasonal models need at least two full cycles of history." },
                    "alpha": { "type": "number", "default": 0.0, "minimum": 0, "maximum": 1, "description": "Level smoothing weight between 0 and 1. Leave at 0 (default) to fit it automatically by minimising the sum of squared one-step-ahead errors. Higher values react faster to recent observations." },
                    "beta": { "type": "number", "default": 0.0, "minimum": 0, "maximum": 1, "description": "Trend smoothing weight between 0 and 1, used by holt, damped and both Holt-Winters models. Leave at 0 (default) to fit it automatically. Ignored by the simple model." },
                    "gamma": { "type": "number", "default": 0.0, "minimum": 0, "maximum": 1, "description": "Seasonal smoothing weight between 0 and 1, used by the Holt-Winters models. Leave at 0 (default) to fit it automatically. Ignored by non-seasonal models." },
                    "phi": { "type": "number", "default": 0.0, "minimum": 0, "maximum": 1, "description": "Trend damping factor between 0 and 1 for model=damped; values near 1 keep the trend, lower values flatten it quickly. Leave at 0 (default) to fit it automatically in the range 0.6 to 0.99. Ignored by other models." },
                    "confidence": { "type": "string", "enum": ["80", "90", "95", "99"], "default": "95", "description": "Prediction-interval confidence level in percent: 80, 90, 95 (default) or 99. Wider levels give wider bands around each forecast." },
                    "show_fitted": { "type": "boolean", "default": false, "description": "Include the per-period table of actual values, one-step-ahead fitted values and residuals (default false). Useful for checking how well the model tracked history." },
                    "header": { "type": "string", "enum": ["auto", "yes", "no"], "default": "auto", "description": "How to treat a leading label row. auto (default) drops the first row only when its value field is not numeric; yes always drops it; no parses every row as data." },
                    "decimals": { "type": "integer", "default": 3, "minimum": 0, "maximum": 10, "description": "Decimal places for forecasts and accuracy metrics, 0 to 10. Default 3." },
                    "format": { "type": "string", "enum": ["text", "csv", "json"], "default": "text", "description": "Output format: text report (model, smoothing parameters, accuracy metrics, forecast table with prediction intervals), csv sections, or structured json. Default text." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
