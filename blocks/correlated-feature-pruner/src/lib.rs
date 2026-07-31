//! gizza-ai/correlated-feature-pruner — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    threshold: f64,
    #[serde(default)]
    method: String,
    #[serde(default)]
    labels: String,
    #[serde(default)]
    header: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("Rows of numbers (CSV-like), one observation per line; each column is a numeric feature. Values may be comma-, space-, or tab-separated. Needs at least 2 columns and 2 rows, e.g. '1,2,5\\n2,4,3\\n3,6,9'."),
        )
        .param(
            Param::number("threshold")
                .default(0.9)
                .min(0.0)
                .max(1.0)
                .describe("Absolute-correlation cutoff in 0..1 (default 0.9). One column of every pair whose |correlation| is strictly greater than this is dropped. Lower it (e.g. 0.8) to prune more aggressively; raise it (0.95) to keep more. 0 falls back to the 0.9 default."),
        )
        .param(
            Param::enumv("method", ["pearson", "spearman", "kendall"])
                .default("pearson")
                .describe("Correlation method: 'pearson' (linear, default), 'spearman' (rank/monotonic), or 'kendall' (rank concordance, tie-corrected tau-b)."),
        )
        .param(
            Param::string("labels")
                .default("")
                .describe("Optional comma-separated column names (defaults to v1..vN). Overrides the header row if both are given."),
        )
        .param(
            Param::boolean("header")
                .default(false)
                .describe("When true, treat the first row as column names instead of data. Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/correlated-feature-pruner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Drop collinear numeric columns above a correlation threshold",
    skill(
        description = "Reduce multicollinearity in a numeric dataset by dropping redundant columns. Computes the pairwise correlation matrix of the columns (features) in `data` — rows of numbers, one observation per line — and greedily removes one column from every pair whose ABSOLUTE correlation exceeds `threshold` (default 0.9), keeping the first-seen column of each correlated group. Choose method='pearson' (linear, default), 'spearman' (monotonic), or 'kendall' (tie-corrected tau-b). Set header=true to read column names from the first row, or pass names via `labels`. Returns a summary of which columns were kept vs. dropped (with the correlation value and the keeper each dropped column was redundant with) and the pruned dataset as CSV.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "correlated-feature-pruner", |a: Args| {
            // serde fills an omitted `threshold` with 0.0; treat that as the
            // documented 0.9 default (0 is a degenerate all-prune setting).
            let threshold = if a.threshold == 0.0 { 0.9 } else { a.threshold };
            gizza_ai_correlated_feature_pruner_core::prune_report(
                &a.data,
                threshold,
                &a.method,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "Rows of numbers (CSV-like), one observation per line; each column is a numeric feature. Values may be comma-, space-, or tab-separated. Needs at least 2 columns and 2 rows, e.g. '1,2,5\\n2,4,3\\n3,6,9'." },
                    "threshold": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.9, "description": "Absolute-correlation cutoff in 0..1 (default 0.9). One column of every pair whose |correlation| is strictly greater than this is dropped. Lower it (e.g. 0.8) to prune more aggressively; raise it (0.95) to keep more. 0 falls back to the 0.9 default." },
                    "method": { "type": "string", "enum": ["pearson", "spearman", "kendall"], "default": "pearson", "description": "Correlation method: 'pearson' (linear, default), 'spearman' (rank/monotonic), or 'kendall' (rank concordance, tie-corrected tau-b)." },
                    "labels": { "type": "string", "default": "", "description": "Optional comma-separated column names (defaults to v1..vN). Overrides the header row if both are given." },
                    "header": { "type": "boolean", "default": false, "description": "When true, treat the first row as column names instead of data. Default false." }
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
