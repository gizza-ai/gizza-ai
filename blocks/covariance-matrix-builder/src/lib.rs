//! gizza-ai/covariance-matrix-builder — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Builds the
//! covariance matrix of a pasted dataset, plus the correlation, centered and
//! standardized variants of the same data. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    labels: String,
    #[serde(default = "default_auto")]
    delimiter: String,
    #[serde(default = "default_auto")]
    header: String,
    #[serde(default = "default_matrix")]
    matrix: String,
    #[serde(default = "default_denominator")]
    denominator: String,
    #[serde(default)]
    weights: String,
    #[serde(default = "default_decimals")]
    decimals: f64,
    #[serde(default = "default_true")]
    stats: bool,
    #[serde(default = "default_format")]
    format: String,
}
fn default_auto() -> String {
    "auto".into()
}
fn default_matrix() -> String {
    "covariance".into()
}
fn default_denominator() -> String {
    "sample".into()
}
fn default_decimals() -> f64 {
    6.0
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
            "The dataset: one observation per line, one variable per column, columns separated by commas, tabs, semicolons, pipes or spaces, e.g. '1,2\\n2,4\\n3,5'. Every row must have the same number of columns and at least 2 data rows are required. A first row whose cells are not all numbers is read as a header of variable names. Up to 20000 rows and 100 columns.",
        ))
        .param(Param::string("labels").describe(
            "Optional comma-separated variable names, one per column in data order (e.g. 'height,weight,age'). They label the matrix rows and columns and override any header row. Default v1, v2, … .",
        ))
        .param(
            Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "space", "pipe"])
                .default("auto")
                .describe(
                    "How each line is split into columns. 'auto' (default) splits on runs of commas, tabs, semicolons, pipes or whitespace, which handles a spreadsheet copy-paste unchanged; pick an explicit delimiter when a value could be ambiguous or when an empty cell must be reported rather than skipped.",
                ),
        )
        .param(
            Param::enumv("header", ["auto", "yes", "no"])
                .default("auto")
                .describe(
                    "Whether the first row holds variable names. 'auto' (default) treats it as a header only when its cells are not all numeric; 'yes' forces it to be names even if numeric; 'no' forces it to be data.",
                ),
        )
        .param(
            Param::enumv(
                "matrix",
                ["covariance", "correlation", "centered", "standardized"],
            )
            .default("covariance")
            .describe(
                "Which matrix to return: 'covariance' (default) = the symmetric k×k matrix whose diagonal is each variable's variance and whose off-diagonal entries are the covariances; 'correlation' = that matrix standardized by the column standard deviations (Pearson r, diagonal 1, range −1…1); 'centered' = the n×k data with each column's mean subtracted; 'standardized' = the n×k data as z-scores, whose covariance matrix is the correlation matrix.",
            ),
        )
        .param(
            Param::enumv("denominator", ["sample", "population"])
                .default("sample")
                .describe(
                    "Divisor for the covariance sums: 'sample' (default) uses n − 1, Bessel's correction, matching numpy.cov, R's cov() and Excel COVARIANCE.S; 'population' uses n, matching COVARIANCE.P, and is right only when the rows are the entire population. With weights the divisor is Σw − 1 or Σw. The choice cancels out of a correlation matrix.",
                ),
        )
        .param(Param::string("weights").describe(
            "Optional frequency weights, one non-negative number per data row, separated by commas or whitespace (e.g. '2,1,1' makes the first row count twice, exactly as if it were pasted twice). Default: every row weighted 1. Weighted runs use Σw in place of n everywhere, matching numpy.cov(fweights=…).",
        ))
        .param(
            Param::integer("decimals")
                .min(0.0)
                .max(12.0)
                .default(6)
                .describe(
                    "Decimal places for every reported number, 0 to 12 (default 6). Only the output is rounded — the arithmetic itself runs at full f64 precision.",
                ),
        )
        .param(Param::boolean("stats").default(true).describe(
            "Append a per-variable summary of n, mean, variance and standard deviation below the matrix (default true); the variances are the covariance matrix's diagonal. Applies to the 'text' and 'markdown' formats — 'csv' is the bare matrix and 'json' always carries those figures.",
        ))
        .param(
            Param::enumv("format", ["text", "markdown", "csv", "json"])
                .default("text")
                .describe(
                    "Output format: 'text' (default) = an aligned, labelled table; 'markdown' = a pipe table to paste into docs; 'csv' = the matrix with a label row and label column, ready for a spreadsheet; 'json' = the full result including the variable names, means, variances, standard deviations, the total variance (trace) and the matrix itself.",
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
    name = "gizza-ai/covariance-matrix-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a covariance, correlation, centered or z-score matrix from pasted data",
    skill(
        description = "Build the covariance matrix of a pasted multivariate dataset — one observation per line, one variable per column, split on commas, tabs, semicolons, pipes or spaces (a non-numeric first row is read as variable names). Returns the symmetric matrix of variances (the diagonal) and covariances (off-diagonal), using the sample n − 1 denominator by default or the population n denominator on request, with optional per-row frequency weights. Set matrix='correlation' for the same matrix standardized to Pearson correlations, 'centered' for the mean-subtracted data, or 'standardized' for z-scores. Reports the column means, variances, standard deviations and the total variance (trace), with decimals controlling the rounding and format='text', 'markdown', 'csv' or 'json' controlling the rendering. Handles up to 20000 observations and 100 variables. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "covariance-matrix-builder", |a: Args| {
            gizza_ai_covariance_matrix_builder_core::run(
                &a.data,
                &a.labels,
                &a.delimiter,
                &a.header,
                &a.matrix,
                &a.denominator,
                &a.weights,
                a.decimals,
                a.stats,
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
                    "data": { "type": "string", "description": "The dataset: one observation per line, one variable per column, columns separated by commas, tabs, semicolons, pipes or spaces, e.g. '1,2\\n2,4\\n3,5'. Every row must have the same number of columns and at least 2 data rows are required. A first row whose cells are not all numbers is read as a header of variable names. Up to 20000 rows and 100 columns." },
                    "labels": { "type": "string", "description": "Optional comma-separated variable names, one per column in data order (e.g. 'height,weight,age'). They label the matrix rows and columns and override any header row. Default v1, v2, … ." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "tab", "semicolon", "space", "pipe"], "default": "auto", "description": "How each line is split into columns. 'auto' (default) splits on runs of commas, tabs, semicolons, pipes or whitespace, which handles a spreadsheet copy-paste unchanged; pick an explicit delimiter when a value could be ambiguous or when an empty cell must be reported rather than skipped." },
                    "header": { "type": "string", "enum": ["auto", "yes", "no"], "default": "auto", "description": "Whether the first row holds variable names. 'auto' (default) treats it as a header only when its cells are not all numeric; 'yes' forces it to be names even if numeric; 'no' forces it to be data." },
                    "matrix": { "type": "string", "enum": ["covariance", "correlation", "centered", "standardized"], "default": "covariance", "description": "Which matrix to return: 'covariance' (default) = the symmetric k×k matrix whose diagonal is each variable's variance and whose off-diagonal entries are the covariances; 'correlation' = that matrix standardized by the column standard deviations (Pearson r, diagonal 1, range −1…1); 'centered' = the n×k data with each column's mean subtracted; 'standardized' = the n×k data as z-scores, whose covariance matrix is the correlation matrix." },
                    "denominator": { "type": "string", "enum": ["sample", "population"], "default": "sample", "description": "Divisor for the covariance sums: 'sample' (default) uses n − 1, Bessel's correction, matching numpy.cov, R's cov() and Excel COVARIANCE.S; 'population' uses n, matching COVARIANCE.P, and is right only when the rows are the entire population. With weights the divisor is Σw − 1 or Σw. The choice cancels out of a correlation matrix." },
                    "weights": { "type": "string", "description": "Optional frequency weights, one non-negative number per data row, separated by commas or whitespace (e.g. '2,1,1' makes the first row count twice, exactly as if it were pasted twice). Default: every row weighted 1. Weighted runs use Σw in place of n everywhere, matching numpy.cov(fweights=…)." },
                    "decimals": { "type": "integer", "minimum": 0, "maximum": 12, "default": 6, "description": "Decimal places for every reported number, 0 to 12 (default 6). Only the output is rounded — the arithmetic itself runs at full f64 precision." },
                    "stats": { "type": "boolean", "default": true, "description": "Append a per-variable summary of n, mean, variance and standard deviation below the matrix (default true); the variances are the covariance matrix's diagonal. Applies to the 'text' and 'markdown' formats — 'csv' is the bare matrix and 'json' always carries those figures." },
                    "format": { "type": "string", "enum": ["text", "markdown", "csv", "json"], "default": "text", "description": "Output format: 'text' (default) = an aligned, labelled table; 'markdown' = a pipe table to paste into docs; 'csv' = the matrix with a label row and label column, ready for a spreadsheet; 'json' = the full result including the variable names, means, variances, standard deviations, the total variance (trace) and the matrix itself." }
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
