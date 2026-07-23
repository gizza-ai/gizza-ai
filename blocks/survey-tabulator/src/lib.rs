//! gizza-ai/survey-tabulator — chat skill block on the shared tool abstraction.
//!
//! Tabulates a survey-response CSV (first row = question headers, each later row
//! = one respondent) into either a per-question frequency table (`overview`) or
//! a two-way cross-tabulation of one question against another (`crosstab`), with
//! optional chi-square / Cramér's V / p-value association stats. The chat schema
//! is single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    question: String,
    #[serde(default)]
    by: String,
    #[serde(default)]
    percent: String,
    #[serde(default)]
    include_blanks: bool,
    #[serde(default)]
    stats: bool,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    top: i64,
    #[serde(default)]
    delimiter: String,
}

/// Single source for the chat schema (and CLI). `data` is required; every option
/// falls back to the documented default so a bare CSV returns a full overview.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data").required().describe(
                "The survey CSV to tabulate. First row = question headers, each later row = one \
                 respondent's answers. Also accepts tab/semicolon/pipe-separated data (set \
                 delimiter).",
            ),
        )
        .param(
            Param::enumv("mode", ["overview", "crosstab"])
                .default("overview")
                .describe(
                    "overview = a frequency table (count + %) for every question column (or one, \
                     if question is set). crosstab = a two-way table of question (rows) against by \
                     (columns). Default overview.",
                ),
        )
        .param(
            Param::string("question")
                .default("")
                .describe(
                    "Which column to tabulate: a 1-based index or a header name. In overview, \
                     blank means tabulate every column; in crosstab this is the row variable \
                     (required).",
                ),
        )
        .param(
            Param::string("by")
                .default("")
                .describe(
                    "crosstab only: the column variable (table columns), as a 1-based index or \
                     header name. Must differ from question.",
                ),
        )
        .param(
            Param::enumv("percent", ["total", "row", "column", "none"])
                .default("total")
                .describe(
                    "crosstab only: what each cell's percentage is out of — total (grand total), \
                     row, column, or none (counts only). Default total.",
                ),
        )
        .param(
            Param::boolean("include_blanks")
                .default(false)
                .describe(
                    "Count blank/empty answers as their own '(blank)' category instead of \
                     dropping them. Default false.",
                ),
        )
        .param(
            Param::boolean("stats")
                .default(false)
                .describe(
                    "crosstab only: append a chi-square test of independence with degrees of \
                     freedom, Cramér's V effect size, and an upper-tail p-value. Default false.",
                ),
        )
        .param(
            Param::enumv("sort", ["count", "label"])
                .default("count")
                .describe(
                    "Order categories by count (descending, ties by label) or label \
                     (alphabetical). Default count.",
                ),
        )
        .param(
            Param::integer("top")
                .default(0)
                .min(0.0)
                .describe(
                    "overview only: keep only the N most frequent categories per question (0 = \
                     all). Default 0.",
                ),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe(
                    "Field delimiter of the input: a single character or one of comma, tab, \
                     semicolon, pipe. Default comma.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

impl Args {
    fn tabulate(&self) -> Result<String, String> {
        gizza_ai_survey_tabulator_core::tabulate(
            &self.data,
            &self.mode,
            &self.question,
            &self.by,
            &self.percent,
            self.include_blanks,
            self.stats,
            &self.sort,
            self.top,
            &self.delimiter,
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/survey-tabulator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Tabulate survey CSV answers into frequency tables and crosstabs with chi-square stats.",
    skill(
        description = "Tabulate a survey-response CSV into a frequency table or a two-way cross-tabulation. Pass data (CSV: first row = question headers, each later row = one respondent). mode=overview gives a count + percent table for every question column (or one, if question is set); mode=crosstab builds a contingency table of question (rows) against by (columns) with row/column/total percentages, marginal totals, and — with stats=true — a chi-square test, degrees of freedom, Cramér's V, and p-value. Columns are named by 1-based index or header. Options: percent (total/row/column/none), include_blanks, sort (count/label), top (keep the N most frequent categories in overview), delimiter (comma/tab/semicolon/pipe). Returns a monospaced text table.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "survey-tabulator", |a: Args| {
            a.tabulate().map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The survey CSV to tabulate. First row = question headers, each later row = one respondent's answers. Also accepts tab/semicolon/pipe-separated data (set delimiter)." },
                    "mode": { "type": "string", "enum": ["overview","crosstab"], "default": "overview", "description": "overview = a frequency table (count + %) for every question column (or one, if question is set). crosstab = a two-way table of question (rows) against by (columns). Default overview." },
                    "question": { "type": "string", "default": "", "description": "Which column to tabulate: a 1-based index or a header name. In overview, blank means tabulate every column; in crosstab this is the row variable (required)." },
                    "by": { "type": "string", "default": "", "description": "crosstab only: the column variable (table columns), as a 1-based index or header name. Must differ from question." },
                    "percent": { "type": "string", "enum": ["total","row","column","none"], "default": "total", "description": "crosstab only: what each cell's percentage is out of — total (grand total), row, column, or none (counts only). Default total." },
                    "include_blanks": { "type": "boolean", "default": false, "description": "Count blank/empty answers as their own '(blank)' category instead of dropping them. Default false." },
                    "stats": { "type": "boolean", "default": false, "description": "crosstab only: append a chi-square test of independence with degrees of freedom, Cramér's V effect size, and an upper-tail p-value. Default false." },
                    "sort": { "type": "string", "enum": ["count","label"], "default": "count", "description": "Order categories by count (descending, ties by label) or label (alphabetical). Default count." },
                    "top": { "type": "integer", "minimum": 0, "default": 0, "description": "overview only: keep only the N most frequent categories per question (0 = all). Default 0." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field delimiter of the input: a single character or one of comma, tab, semicolon, pipe. Default comma." }
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
