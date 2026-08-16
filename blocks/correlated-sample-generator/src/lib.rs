//! gizza-ai/correlated-sample-generator — draw multivariate-normal samples that
//! match a supplied mean vector and covariance (or correlation) matrix, by
//! transforming independent normals with a Cholesky or symmetric-square-root
//! factor. Chat schema single-sourced from descriptor(); handle() delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_correlated_sample_generator_core::generate;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    covariance: String,
    #[serde(default)]
    mean: String,
    #[serde(default = "default_samples")]
    samples: usize,
    #[serde(default = "default_matrix_kind")]
    matrix_kind: String,
    #[serde(default)]
    sd: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default)]
    empirical: bool,
    #[serde(default = "default_tol")]
    tol: f64,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_decimals")]
    decimals: usize,
    #[serde(default)]
    labels: String,
    #[serde(default = "default_true")]
    header: bool,
}

fn default_samples() -> usize {
    100
}
fn default_matrix_kind() -> String {
    "covariance".into()
}
fn default_method() -> String {
    "cholesky".into()
}
fn default_seed() -> u64 {
    42
}
fn default_tol() -> f64 {
    1e-8
}
fn default_output() -> String {
    "csv".into()
}
fn default_decimals() -> usize {
    4
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("covariance").required().describe(
            "The square target matrix, one row per variable. Accepts rows separated by newlines or semicolons and entries separated by commas, tabs or spaces, plus JSON: \"1, 0.8; 0.8, 1\" and [[1,0.8],[0.8,1]] are the same 2-variable matrix. It must be symmetric. By default it is read as a COVARIANCE matrix (variances on the diagonal); set matrix_kind to 'correlation' to supply correlations with 1 on the diagonal instead. Three shorthands build a standard correlation structure instead of typing every entry: iid(4) for 4 uncorrelated variables, cs(4, 0.3) for compound symmetry with 0.3 between every pair, and ar1(4, 0.7) for an autoregressive structure where correlation is 0.7^|i-j|. Up to 50 variables.",
        ))
        .param(Param::string("mean").default("").describe(
            "The mean of each variable, comma- or space-separated, in the same order as the matrix rows — for example \"10, -4\". Must have one value per variable. Leave empty for a mean of 0 everywhere.",
        ))
        .param(
            Param::integer("samples")
                .default(100)
                .min(1.0)
                .max(100000.0)
                .describe("How many draws (rows) to generate, 1-100000. Total output is also capped at 200000 numbers, so 2 variables allow 100000 draws and 4 variables allow 50000. Default 100."),
        )
        .param(
            Param::enumv("matrix_kind", ["covariance", "correlation"])
                .default("covariance")
                .describe("How to read the matrix. 'covariance' (default) takes variances on the diagonal and covariances off it. 'correlation' takes correlations in -1..1 with 1 on the diagonal, and is combined with sd to build the covariance matrix — use this when you know the correlations and standard deviations rather than covariances."),
        )
        .param(Param::string("sd").default("").describe(
            "Standard deviation of each variable, comma-separated, used only when matrix_kind is 'correlation' — for example \"2, 3\" turns correlation 0.5 into covariance 0.5*2*3 = 3. One value per variable; leave empty for a standard deviation of 1 everywhere (which makes the correlation matrix the covariance matrix).",
        ))
        .param(
            Param::enumv("method", ["cholesky", "eigen"])
                .default("cholesky")
                .describe("How the matrix is factorised into A with A*A^T equal to the covariance. 'cholesky' (default) is the classic fast lower-triangular factor and requires a strictly positive-definite matrix. 'eigen' uses the symmetric square root from an eigendecomposition, which is slower but also handles positive-semi-definite and numerically singular matrices such as a correlation of exactly 1 or -1. Both give samples from the same distribution, but different draws for the same seed."),
        )
        .param(
            Param::integer("seed")
                .default(42)
                .min(0.0)
                .describe("Seed for the built-in deterministic generator. The same seed, matrix and settings always return exactly the same draws — change it for a different sample. Default 42."),
        )
        .param(
            Param::boolean("empirical")
                .default(false)
                .describe("When true, rescale the draws so the SAMPLE mean and SAMPLE covariance equal the targets exactly instead of only in expectation (the behaviour of R's mvrnorm with empirical=TRUE). Useful for fixed test data with known statistics; leave false for an honest random sample that shows natural sampling variation. Needs more draws than variables. Default false."),
        )
        .param(
            Param::number("tol")
                .default(1e-8)
                .min(0.0)
                .describe("Numerical tolerance, relative to the size of the matrix entries, used for the symmetry check and for how far an eigenvalue may fall below zero before the matrix is rejected as invalid. Raise it if a matrix that is valid in theory is rejected because of rounding in the values you pasted. Default 0.00000001."),
        )
        .param(
            Param::enumv("output", ["csv", "tsv", "json", "stats"])
                .default("csv")
                .describe("Result format. 'csv' (default) and 'tsv' emit one row per draw with an optional header line. 'json' returns {count, dimensions, labels, method, seed, empirical, target, achieved, samples} including the sample mean/covariance/correlation actually achieved. 'stats' skips the rows and returns only a readable comparison of the target against the sample mean, covariance and correlation — the quickest way to check that a draw came out right."),
        )
        .param(
            Param::integer("decimals")
                .default(4)
                .min(0.0)
                .max(12.0)
                .describe("Digits after the decimal point in the output, 0-12. Default 4."),
        )
        .param(Param::string("labels").default("").describe(
            "Column names, comma-separated, in matrix-row order — for example \"height, weight\". One per variable. Leave empty to use X1, X2, X3 and so on.",
        ))
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Include the column-name row at the top of csv and tsv output. Set false to get bare numbers for a script. Ignored by json and stats. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/correlated-sample-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate multivariate-normal samples matching a mean vector and covariance matrix",
    skill(
        description = "Generate random samples that follow a multivariate normal distribution with a given mean vector and covariance matrix, so the columns carry the correlations you asked for. Independent standard normals are transformed by a factor A of the matrix (A*A^T = covariance): method='cholesky' uses the classic lower-triangular Cholesky factor and needs a positive-definite matrix, while method='eigen' uses the symmetric square root and also handles positive-semi-definite or singular matrices such as a correlation of exactly 1. The matrix is accepted as newline-, semicolon- or JSON-delimited rows, read as covariances by default or as a correlation matrix plus sd values when matrix_kind='correlation'. mean sets the per-variable means, labels the column names, samples the number of draws. Randomness is a seeded deterministic generator, so the same seed reproduces the same data exactly; empirical=true additionally rescales the draws so the sample mean and covariance match the targets exactly rather than approximately, like R's mvrnorm(empirical=TRUE). output returns csv, tsv, a json document that also reports the achieved mean/covariance/correlation, or a stats-only comparison of target against sample. Symmetry, definiteness and shape are validated with explicit errors. Capped at 50 variables, 100000 draws and 200000 numbers. Runs locally and never uploads anything.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "correlated-sample-generator", |a: Args| {
            generate(
                &a.covariance,
                &a.mean,
                a.samples,
                &a.matrix_kind,
                &a.sd,
                &a.method,
                a.seed,
                a.empirical,
                a.tol,
                &a.output,
                a.decimals,
                &a.labels,
                a.header,
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

    /// Drift guard: the authored chat/CLI schema must stay identical to the one
    /// derived from descriptor(). Update BOTH sides together, on purpose.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "covariance": { "type": "string", "description": "The square target matrix, one row per variable. Accepts rows separated by newlines or semicolons and entries separated by commas, tabs or spaces, plus JSON: \"1, 0.8; 0.8, 1\" and [[1,0.8],[0.8,1]] are the same 2-variable matrix. It must be symmetric. By default it is read as a COVARIANCE matrix (variances on the diagonal); set matrix_kind to 'correlation' to supply correlations with 1 on the diagonal instead. Three shorthands build a standard correlation structure instead of typing every entry: iid(4) for 4 uncorrelated variables, cs(4, 0.3) for compound symmetry with 0.3 between every pair, and ar1(4, 0.7) for an autoregressive structure where correlation is 0.7^|i-j|. Up to 50 variables." },
                    "mean": { "type": "string", "default": "", "description": "The mean of each variable, comma- or space-separated, in the same order as the matrix rows — for example \"10, -4\". Must have one value per variable. Leave empty for a mean of 0 everywhere." },
                    "samples": { "type": "integer", "default": 100, "minimum": 1, "maximum": 100000, "description": "How many draws (rows) to generate, 1-100000. Total output is also capped at 200000 numbers, so 2 variables allow 100000 draws and 4 variables allow 50000. Default 100." },
                    "matrix_kind": { "type": "string", "enum": ["covariance", "correlation"], "default": "covariance", "description": "How to read the matrix. 'covariance' (default) takes variances on the diagonal and covariances off it. 'correlation' takes correlations in -1..1 with 1 on the diagonal, and is combined with sd to build the covariance matrix — use this when you know the correlations and standard deviations rather than covariances." },
                    "sd": { "type": "string", "default": "", "description": "Standard deviation of each variable, comma-separated, used only when matrix_kind is 'correlation' — for example \"2, 3\" turns correlation 0.5 into covariance 0.5*2*3 = 3. One value per variable; leave empty for a standard deviation of 1 everywhere (which makes the correlation matrix the covariance matrix)." },
                    "method": { "type": "string", "enum": ["cholesky", "eigen"], "default": "cholesky", "description": "How the matrix is factorised into A with A*A^T equal to the covariance. 'cholesky' (default) is the classic fast lower-triangular factor and requires a strictly positive-definite matrix. 'eigen' uses the symmetric square root from an eigendecomposition, which is slower but also handles positive-semi-definite and numerically singular matrices such as a correlation of exactly 1 or -1. Both give samples from the same distribution, but different draws for the same seed." },
                    "seed": { "type": "integer", "default": 42, "minimum": 0, "description": "Seed for the built-in deterministic generator. The same seed, matrix and settings always return exactly the same draws — change it for a different sample. Default 42." },
                    "empirical": { "type": "boolean", "default": false, "description": "When true, rescale the draws so the SAMPLE mean and SAMPLE covariance equal the targets exactly instead of only in expectation (the behaviour of R's mvrnorm with empirical=TRUE). Useful for fixed test data with known statistics; leave false for an honest random sample that shows natural sampling variation. Needs more draws than variables. Default false." },
                    "tol": { "type": "number", "default": 1e-8, "minimum": 0, "description": "Numerical tolerance, relative to the size of the matrix entries, used for the symmetry check and for how far an eigenvalue may fall below zero before the matrix is rejected as invalid. Raise it if a matrix that is valid in theory is rejected because of rounding in the values you pasted. Default 0.00000001." },
                    "output": { "type": "string", "enum": ["csv", "tsv", "json", "stats"], "default": "csv", "description": "Result format. 'csv' (default) and 'tsv' emit one row per draw with an optional header line. 'json' returns {count, dimensions, labels, method, seed, empirical, target, achieved, samples} including the sample mean/covariance/correlation actually achieved. 'stats' skips the rows and returns only a readable comparison of the target against the sample mean, covariance and correlation — the quickest way to check that a draw came out right." },
                    "decimals": { "type": "integer", "default": 4, "minimum": 0, "maximum": 12, "description": "Digits after the decimal point in the output, 0-12. Default 4." },
                    "labels": { "type": "string", "default": "", "description": "Column names, comma-separated, in matrix-row order — for example \"height, weight\". One per variable. Leave empty to use X1, X2, X3 and so on." },
                    "header": { "type": "boolean", "default": true, "description": "Include the column-name row at the top of csv and tsv output. Set false to get bare numbers for a script. Ignored by json and stats. Default true." }
                },
                "required": ["covariance"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(authored, derived);
    }
}
