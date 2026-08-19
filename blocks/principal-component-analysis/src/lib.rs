//! gizza-ai/principal-component-analysis — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Runs PCA on a
//! pasted numeric matrix and reports the explained-variance table, the component
//! loadings and the projected coordinates (scores). Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    labels: String,
    #[serde(default)]
    components: f64,
    #[serde(default = "default_true")]
    scale: bool,
    #[serde(default = "default_format")]
    format: String,
}
fn default_true() -> bool {
    true
}
fn default_format() -> String {
    "text".into()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "The data matrix: one observation per line, one variable per column, columns separated by commas, tabs, semicolons or spaces. Every row must have the same number of columns and there must be at least 2 columns and 2 rows, e.g. '1,2\\n2,4\\n3,5'. A first row whose values are not all numbers is read as a header of column names. Up to 20000 rows and 100 columns.",
        ))
        .param(Param::string("labels").describe(
            "Optional comma-separated variable names, one per column in data order (e.g. 'height,weight,age'). They name the loading rows and override any header row. Default v1, v2, … .",
        ))
        .param(
            Param::integer("components")
                .min(0.0)
                .max(100.0)
                .default(0)
                .describe(
                    "How many principal components to report, most-variance first. 0 (default) keeps every component. The explained-variance percentages are always computed against the full set.",
                ),
        )
        .param(
            Param::boolean("scale")
                .default(true)
                .describe(
                    "Standardize each column to unit variance before the decomposition (default true), i.e. run PCA on the correlation matrix — the right choice when the variables use different units or scales. Set false to use the covariance matrix and let raw magnitudes dominate. Columns are always mean-centered.",
                ),
        )
        .param(
            Param::enumv("format", ["text", "json", "csv"])
                .default("text")
                .describe(
                    "Output format: 'text' (default) = a formatted report with the explained-variance table, the loadings and the first 20 score rows; 'json' = the full result including every score, the column means and standard deviations; 'csv' = just the projected coordinates as 'row,PC1,PC2,…', ready to plot.",
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
    name = "gizza-ai/principal-component-analysis",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "PCA on a data matrix: explained variance, loadings and scores",
    skill(
        description = "Run principal component analysis (PCA) on a pasted numeric data matrix — one observation per line, one variable per column, split on commas, tabs, semicolons or spaces (a non-numeric first row is read as column names). Columns are mean-centered and, by default (scale=true), standardized to unit variance so the analysis runs on the correlation matrix; set scale=false for covariance PCA. Returns each component's eigenvalue, its proportion and cumulative proportion of the total variance, how many components reach 90/95/99% of the variance, the Kaiser count, the loadings (each variable's weight in each component) and the scores (every observation projected onto the components). Use components to keep only the top N, labels to name the columns, and format='json' or 'csv' to get every score row for plotting. Handles up to 20000 rows and 100 columns; component signs are fixed so the largest loading is positive. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "principal-component-analysis", |a: Args| {
            gizza_ai_principal_component_analysis_core::run(
                &a.data,
                &a.labels,
                a.components,
                a.scale,
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
                    "data": { "type": "string", "description": "The data matrix: one observation per line, one variable per column, columns separated by commas, tabs, semicolons or spaces. Every row must have the same number of columns and there must be at least 2 columns and 2 rows, e.g. '1,2\\n2,4\\n3,5'. A first row whose values are not all numbers is read as a header of column names. Up to 20000 rows and 100 columns." },
                    "labels": { "type": "string", "description": "Optional comma-separated variable names, one per column in data order (e.g. 'height,weight,age'). They name the loading rows and override any header row. Default v1, v2, … ." },
                    "components": { "type": "integer", "minimum": 0, "maximum": 100, "default": 0, "description": "How many principal components to report, most-variance first. 0 (default) keeps every component. The explained-variance percentages are always computed against the full set." },
                    "scale": { "type": "boolean", "default": true, "description": "Standardize each column to unit variance before the decomposition (default true), i.e. run PCA on the correlation matrix — the right choice when the variables use different units or scales. Set false to use the covariance matrix and let raw magnitudes dominate. Columns are always mean-centered." },
                    "format": { "type": "string", "enum": ["text", "json", "csv"], "default": "text", "description": "Output format: 'text' (default) = a formatted report with the explained-variance table, the loadings and the first 20 score rows; 'json' = the full result including every score, the column means and standard deviations; 'csv' = just the projected coordinates as 'row,PC1,PC2,…', ready to plot." }
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
