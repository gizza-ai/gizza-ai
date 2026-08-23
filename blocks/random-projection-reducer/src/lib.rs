//! gizza-ai/random-projection-reducer — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Reduces a wide
//! numeric table to a few columns with a Johnson–Lindenstrauss random projection
//! and reports how well pairwise distances survived. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_components")]
    components: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default)]
    density: f64,
    #[serde(default = "default_eps")]
    eps: f64,
    #[serde(default = "default_seed")]
    seed: f64,
    #[serde(default = "default_format")]
    format: String,
}
fn default_components() -> String {
    "auto".into()
}
fn default_method() -> String {
    "gaussian".into()
}
fn default_eps() -> f64 {
    0.1
}
fn default_seed() -> f64 {
    42.0
}
fn default_format() -> String {
    "text".into()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "The data matrix: one observation per line, one variable per column, columns separated by commas, tabs, semicolons, pipes or spaces. Every row must have the same number of columns, and there must be at least 2 rows and 2 columns, e.g. '1,2,3,4\\n8,7,6,5\\n2,4,6,8'. A first row whose values are not all numbers is read as a header of column names. Up to 2000 rows, 1000 columns and 200000 cells.",
        ))
        .param(Param::string("components").default("auto").describe(
            "Target number of dimensions k. 'auto' (the default) derives it from the Johnson–Lindenstrauss bound at the given eps, clamped to the number of input columns. A whole number sets k directly (e.g. 3). A percentage keeps that share of the input width (e.g. '25%' of 8 columns is 2). Maximum 256.",
        ))
        .param(
            Param::enumv("method", ["gaussian", "sparse", "achlioptas", "rademacher"])
                .default("gaussian")
                .describe(
                    "Which random matrix to project with. 'gaussian' (default) is dense with entries drawn from N(0, 1/k) — the classic choice. 'sparse' draws ±sqrt(1/(density·k)) with the rest zero, at density 1/sqrt(columns) by default, which is much faster on wide data. 'achlioptas' is the same family at the fixed density 1/3, i.e. sqrt(3/k)·{-1, 0, +1}. 'rademacher' is a dense ±sqrt(1/k) sign matrix. All four are scaled so distances are preserved in expectation.",
                ),
        )
        .param(
            Param::number("density")
                .min(0.0)
                .max(1.0)
                .default(0.0)
                .describe(
                    "Fraction of non-zero entries in the random matrix, for method 'sparse' or 'achlioptas' only. 0 (the default) uses each method's own default: 1/sqrt(columns) for 'sparse' and 1/3 for 'achlioptas'. Lower values are sparser and faster but noisier. Setting it for a dense method ('gaussian', 'rademacher') is an error.",
                ),
        )
        .param(
            Param::number("eps")
                .min(0.01)
                .max(0.99)
                .default(0.1)
                .describe(
                    "Distance-distortion tolerance, as a fraction — 0.1 (the default) means ±10%. It sets the target dimension when components='auto' (k = 4·ln(rows) / (eps²/2 − eps³/3)) and is the threshold the report counts row pairs against. Smaller eps means a tighter embedding and more dimensions.",
                ),
        )
        .param(
            Param::integer("seed")
                .min(0.0)
                .max(4294967295.0)
                .default(42)
                .describe(
                    "Seed for the random matrix, 0 to 4294967295 (default 42). The generator is a fixed portable integer stream, so the same seed reproduces exactly the same projection here, in the CLI and in the browser. Change it to draw a different projection of the same data.",
                ),
        )
        .param(
            Param::enumv("format", ["text", "json", "csv", "matrix"])
                .default("text")
                .describe(
                    "Output format: 'text' (default) = a report with the settings, the distance-preservation diagnostics, Johnson–Lindenstrauss guidance and the first 20 projected rows; 'json' = the full structured result including every projected row and the projection matrix; 'csv' = just the projected rows as 'row,RP1,RP2,…', ready to plot; 'matrix' = the k × columns projection matrix itself as CSV, so the same projection can be reapplied to new rows.",
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
    name = "gizza-ai/random-projection-reducer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reduce a wide numeric table to fewer dimensions with a Johnson–Lindenstrauss random projection",
    skill(
        description = "Reduce a high-dimensional numeric table to fewer columns with a fast Johnson–Lindenstrauss random projection. Paste a matrix — one observation per line, one variable per column, split on commas, tabs, semicolons, pipes or spaces (a non-numeric first row is read as column names) — and it is multiplied by a randomly drawn matrix scaled so pairwise distances are preserved in expectation. Choose the matrix family with method: gaussian (dense N(0, 1/k)), sparse (±sqrt(1/(density·k)) at density 1/sqrt(columns)), achlioptas (density 1/3) or rademacher (dense ±1 signs). Set components to a number, to a percentage of the input width like '25%', or leave it 'auto' to derive k from the Johnson–Lindenstrauss bound at eps. The report states the settings used, measures how much pairwise row distances actually moved (mean, median and maximum distortion, the mean ratio, and how many sampled pairs stayed inside ±eps), tabulates the JL minimum dimension for several eps values, and prints the projected rows. Use format='csv' for every projected row, 'json' for the full result, or 'matrix' to get the projection matrix itself. Unlike PCA it needs no eigen-decomposition, so it scales to very wide data, but it optimises distances rather than variance. Handles up to 2000 rows, 1000 columns and 200000 cells, with k up to 256; the seed makes every run reproducible. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "random-projection-reducer", |a: Args| {
            gizza_ai_random_projection_reducer_core::run(
                &a.data,
                &a.components,
                &a.method,
                a.density,
                a.eps,
                a.seed,
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
                    "data": { "type": "string", "description": "The data matrix: one observation per line, one variable per column, columns separated by commas, tabs, semicolons, pipes or spaces. Every row must have the same number of columns, and there must be at least 2 rows and 2 columns, e.g. '1,2,3,4\\n8,7,6,5\\n2,4,6,8'. A first row whose values are not all numbers is read as a header of column names. Up to 2000 rows, 1000 columns and 200000 cells." },
                    "components": { "type": "string", "default": "auto", "description": "Target number of dimensions k. 'auto' (the default) derives it from the Johnson–Lindenstrauss bound at the given eps, clamped to the number of input columns. A whole number sets k directly (e.g. 3). A percentage keeps that share of the input width (e.g. '25%' of 8 columns is 2). Maximum 256." },
                    "method": { "type": "string", "enum": ["gaussian", "sparse", "achlioptas", "rademacher"], "default": "gaussian", "description": "Which random matrix to project with. 'gaussian' (default) is dense with entries drawn from N(0, 1/k) — the classic choice. 'sparse' draws ±sqrt(1/(density·k)) with the rest zero, at density 1/sqrt(columns) by default, which is much faster on wide data. 'achlioptas' is the same family at the fixed density 1/3, i.e. sqrt(3/k)·{-1, 0, +1}. 'rademacher' is a dense ±sqrt(1/k) sign matrix. All four are scaled so distances are preserved in expectation." },
                    "density": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.0, "description": "Fraction of non-zero entries in the random matrix, for method 'sparse' or 'achlioptas' only. 0 (the default) uses each method's own default: 1/sqrt(columns) for 'sparse' and 1/3 for 'achlioptas'. Lower values are sparser and faster but noisier. Setting it for a dense method ('gaussian', 'rademacher') is an error." },
                    "eps": { "type": "number", "minimum": 0.01, "maximum": 0.99, "default": 0.1, "description": "Distance-distortion tolerance, as a fraction — 0.1 (the default) means ±10%. It sets the target dimension when components='auto' (k = 4·ln(rows) / (eps²/2 − eps³/3)) and is the threshold the report counts row pairs against. Smaller eps means a tighter embedding and more dimensions." },
                    "seed": { "type": "integer", "minimum": 0, "maximum": 4294967295, "default": 42, "description": "Seed for the random matrix, 0 to 4294967295 (default 42). The generator is a fixed portable integer stream, so the same seed reproduces exactly the same projection here, in the CLI and in the browser. Change it to draw a different projection of the same data." },
                    "format": { "type": "string", "enum": ["text", "json", "csv", "matrix"], "default": "text", "description": "Output format: 'text' (default) = a report with the settings, the distance-preservation diagnostics, Johnson–Lindenstrauss guidance and the first 20 projected rows; 'json' = the full structured result including every projected row and the projection matrix; 'csv' = just the projected rows as 'row,RP1,RP2,…', ready to plot; 'matrix' = the k × columns projection matrix itself as CSV, so the same projection can be reapplied to new rows." }
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
