//! gizza-ai/least-squares-regression — fit linear or polynomial least-squares
//! models to pasted `(x, y)` data. Thin chat-skill wrapper around the pure core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    y_values: String,
    #[serde(default = "default_degree")]
    degree: i64,
    #[serde(default = "default_header")]
    header: String,
    #[serde(default = "default_intercept")]
    intercept: bool,
    #[serde(default)]
    predict_x: String,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default = "default_format")]
    format: String,
}

fn default_degree() -> i64 { 1 }
fn default_header() -> String { "auto".to_string() }
fn default_intercept() -> bool { true }
fn default_decimals() -> i64 { 4 }
fn default_format() -> String { "text".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data").required().describe(
                "Regression data. Paste either two columns as one x,y pair per line (CSV, semicolon, or whitespace separated), or paste only x values here and put y values in the y_values field. A first non-numeric row is treated as x/y labels when header=auto. Maximum 20,000 points.",
            ),
        )
        .param(
            Param::string("y_values").default("").describe(
                "Optional separate y-value list. Leave empty when data already has two columns. When set, data is parsed as the x-value list and this field as the y-value list; both lists must have the same length.",
            ),
        )
        .param(
            Param::integer("degree").default(1).min(1.0).max(10.0).describe(
                "Polynomial degree to fit, 1 to 10. degree=1 is ordinary straight-line least squares; degree=2 fits a quadratic; higher degrees use the same QR solver but need more distinct x values.",
            ),
        )
        .param(
            Param::enumv("header", ["auto", "yes", "no"]).default("auto").describe(
                "How to treat a leading label row/token. auto (default) consumes the first row/list item only if it is non-numeric; yes always consumes it as labels; no parses every row as data.",
            ),
        )
        .param(
            Param::boolean("intercept").default(true).describe(
                "Fit an intercept/constant term (default true). Turn this off only when the model must pass through the origin; the reported R² then uses the uncentered total sum of squares.",
            ),
        )
        .param(
            Param::string("predict_x").default("").describe(
                "Optional x values to evaluate with the fitted model, separated by commas, semicolons, spaces, or newlines. Predictions are included in text, CSV, and JSON output.",
            ),
        )
        .param(
            Param::integer("decimals").default(4).min(0.0).max(12.0).describe(
                "Decimal places for coefficients and statistics, 0 to 12. Default 4.",
            ),
        )
        .param(
            Param::enumv("format", ["text", "csv", "json"]).default("text").describe(
                "Output format: text summary (equation, model metrics, coefficients, residual spread, predictions), csv tables, or structured json. Default text.",
            ),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/least-squares-regression",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fit linear or polynomial least-squares regression models to x,y data",
    skill(
        description = "Fit a one-variable ordinary least-squares model to pasted x,y points. Supports linear and polynomial fits (degree 1-10), optional separate x and y lists, automatic or forced header labels, optional through-the-origin fitting, predictions at new x values, and text/csv/json output. Reports the fitted equation, coefficients with standard errors, R², adjusted R², Pearson r for straight-line fits, RMSE, residual standard error, residual spread, per-point fitted values/residuals, and predictions. Uses a deterministic Householder QR solver with column scaling rather than normal equations, so it remains stable in WASM and returns explicit rank/degree errors instead of misleading coefficients.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "least-squares-regression", |a: Args| {
            gizza_ai_least_squares_regression_core::run(
                &a.data,
                &a.y_values,
                a.degree,
                &a.header,
                a.intercept,
                &a.predict_x,
                a.decimals,
                &a.format,
            ).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "Regression data. Paste either two columns as one x,y pair per line (CSV, semicolon, or whitespace separated), or paste only x values here and put y values in the y_values field. A first non-numeric row is treated as x/y labels when header=auto. Maximum 20,000 points." },
                    "y_values": { "type": "string", "default": "", "description": "Optional separate y-value list. Leave empty when data already has two columns. When set, data is parsed as the x-value list and this field as the y-value list; both lists must have the same length." },
                    "degree": { "type": "integer", "default": 1, "minimum": 1, "maximum": 10, "description": "Polynomial degree to fit, 1 to 10. degree=1 is ordinary straight-line least squares; degree=2 fits a quadratic; higher degrees use the same QR solver but need more distinct x values." },
                    "header": { "type": "string", "enum": ["auto", "yes", "no"], "default": "auto", "description": "How to treat a leading label row/token. auto (default) consumes the first row/list item only if it is non-numeric; yes always consumes it as labels; no parses every row as data." },
                    "intercept": { "type": "boolean", "default": true, "description": "Fit an intercept/constant term (default true). Turn this off only when the model must pass through the origin; the reported R² then uses the uncentered total sum of squares." },
                    "predict_x": { "type": "string", "default": "", "description": "Optional x values to evaluate with the fitted model, separated by commas, semicolons, spaces, or newlines. Predictions are included in text, CSV, and JSON output." },
                    "decimals": { "type": "integer", "default": 4, "minimum": 0, "maximum": 12, "description": "Decimal places for coefficients and statistics, 0 to 12. Default 4." },
                    "format": { "type": "string", "enum": ["text", "csv", "json"], "default": "text", "description": "Output format: text summary (equation, model metrics, coefficients, residual spread, predictions), csv tables, or structured json. Default text." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
