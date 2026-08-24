//! gizza-ai/fiscal-quarter-mapper — label a CSV date column with fiscal quarters.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_auto")]
    column: String,
    #[serde(default = "default_start_month")]
    fiscal_start_month: String,
    #[serde(default = "default_naming")]
    fiscal_year_naming: String,
    #[serde(default = "default_quarter_label")]
    quarter_label: String,
    #[serde(default = "default_year_label")]
    fiscal_year_label: String,
    #[serde(default = "default_true")]
    add_fiscal_year: bool,
    #[serde(default)]
    add_quarter_dates: bool,
    #[serde(default)]
    add_fiscal_month: bool,
    #[serde(default)]
    add_quarter_position: bool,
    #[serde(default = "default_auto")]
    date_order: String,
    #[serde(default = "default_on_error")]
    on_error: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_auto")]
    delimiter: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_auto() -> String {
    "auto".into()
}
fn default_start_month() -> String {
    "january".into()
}
fn default_naming() -> String {
    "end".into()
}
fn default_quarter_label() -> String {
    "q-fy".into()
}
fn default_year_label() -> String {
    "fy-yyyy".into()
}
fn default_on_error() -> String {
    "blank".into()
}
fn default_output() -> String {
    "csv".into()
}
fn default_true() -> bool {
    true
}

/// Single source for chat/CLI/page parameters.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe(
            "CSV/TSV text with a date column, e.g. \"invoice,closed\\nA-1,2025-10-14\". Include the header row when header=true. Capped at 5,000,000 bytes.",
        ))
        .param(Param::string("column").default("auto").describe(
            "Which column holds the dates: a header name (\"closed\"), a 0-based index (\"1\"), or \"auto\" (default) to pick the first column where at least 60% of non-blank cells parse as a date. With header=false only an index works.",
        ))
        .param(
            Param::enumv(
                "fiscal_start_month",
                [
                    "january",
                    "february",
                    "march",
                    "april",
                    "may",
                    "june",
                    "july",
                    "august",
                    "september",
                    "october",
                    "november",
                    "december",
                ],
            )
            .default("january")
            .describe(
                "Calendar month the fiscal year starts in. january = calendar year (default), april = UK/India, july = Australia/NZ, october = US federal.",
            ),
        )
        .param(Param::enumv("fiscal_year_naming", ["end", "start"]).default("end").describe(
            "Which calendar year names the fiscal year. \"end\" (default) matches the US federal government and pandas: Oct 2025 is FY2026. \"start\" matches the common spreadsheet recipe: Oct 2025 is FY2025. The two conventions differ by one, so pick deliberately.",
        ))
        .param(
            Param::enumv("quarter_label", ["q-fy", "fy-q", "yyyy-qn", "yyyyqn", "qn", "n"])
                .default("q-fy")
                .describe(
                    "Format of the fiscal_quarter column: q-fy = \"Q1 FY2026\" (default), fy-q = \"FY2026-Q1\", yyyy-qn = \"2026-Q1\", yyyyqn = \"2026Q1\" (pandas style), qn = \"Q1\", n = \"1\".",
                ),
        )
        .param(
            Param::enumv("fiscal_year_label", ["fy-yyyy", "yyyy", "fy-yy", "range", "range-short"])
                .default("fy-yyyy")
                .describe(
                    "Format of the fiscal_year column (and of the year embedded in q-fy/fy-q quarter labels): fy-yyyy = \"FY2026\" (default), yyyy = \"2026\", fy-yy = \"FY26\", range = \"2025-2026\", range-short = \"2025-26\". Ranges collapse to one year when the fiscal year starts in january.",
                ),
        )
        .param(Param::boolean("add_fiscal_year").default(true).describe(
            "Append a fiscal_year column next to fiscal_quarter. Default true.",
        ))
        .param(Param::boolean("add_quarter_dates").default(false).describe(
            "Append fiscal_quarter_start and fiscal_quarter_end columns as ISO YYYY-MM-DD dates. Default false.",
        ))
        .param(Param::boolean("add_fiscal_month").default(false).describe(
            "Append a fiscal_month column numbered 1-12 from the fiscal start month (october start: October = 1). Default false.",
        ))
        .param(Param::boolean("add_quarter_position").default(false).describe(
            "Append day_of_quarter and days_in_quarter columns, measured from the row's own date (quarters run 90-92 days). Default false.",
        ))
        .param(Param::enumv("date_order", ["auto", "day-first", "month-first"]).default("auto").describe(
            "How to read an all-numeric value like 03/04/2024. \"auto\" (default) settles the whole column from rows that can only be read one way (a day above 12) and falls back to month-first; day-first forces DD/MM/YYYY; month-first forces MM/DD/YYYY. ISO YYYY-MM-DD is unaffected.",
        ))
        .param(Param::enumv("on_error", ["blank", "drop", "error"]).default("blank").describe(
            "What to do with a cell that cannot be read as a date: blank (default) leaves the added columns empty, drop removes the row, error stops and names the offending row and line.",
        ))
        .param(Param::boolean("header").default(true).describe(
            "Treat the first row as column names. Default true. Turn off for a bare list of dates with no header row.",
        ))
        .param(Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"]).default("auto").describe(
            "Input/output delimiter. auto (default) sniffs comma, tab, semicolon, or pipe from the first non-blank line; choose one to force it. The output uses the same delimiter as the input.",
        ))
        .param(Param::enumv("output", ["csv", "report", "json"]).default("csv").describe(
            "csv (default) returns the input with the fiscal columns appended; report returns a plain-text audit (column picked, date order and the evidence for it, rows per quarter, unreadable values); json returns that audit plus the CSV as a JSON object.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FiscalQuarterMapper;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fiscal-quarter-mapper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Label a CSV date column with fiscal quarters and fiscal years",
    skill(
        description = "Append fiscal-quarter and fiscal-year label columns to a CSV/TSV table whose date column can start its fiscal year in any month (january = calendar, april = UK/India, july = Australia, october = US federal). The fiscal year can be named by the calendar year it ends in (US federal / pandas) or the one it begins in (the common spreadsheet recipe), and labels render as Q1 FY2026, FY2026-Q1, 2026-Q1, 2026Q1, Q1, or 1. Optional columns add the quarter's start and end dates, the fiscal month 1-12, and the row's day-of-quarter out of days-in-quarter. It reads ISO, US, European, written-month, compact YYYYMMDD, month-precision and timestamped values, settles ambiguous DD/MM vs MM/DD column-wide from rows that can only be read one way, and can blank, drop, or raise on unreadable cells. Output is the rewritten CSV, a plain-text audit report, or JSON carrying both.",
        parameters = schema_json()
    ),
)]
impl FiscalQuarterMapper {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fiscal-quarter-mapper", |a: Args| {
            run_args(a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

fn run_args(a: Args) -> Result<String, String> {
    gizza_ai_fiscal_quarter_mapper_core::run(
        &a.input,
        &a.column,
        &a.fiscal_start_month,
        &a.fiscal_year_naming,
        &a.quarter_label,
        &a.fiscal_year_label,
        a.add_fiscal_year,
        a.add_quarter_dates,
        a.add_fiscal_month,
        a.add_quarter_position,
        &a.date_order,
        &a.on_error,
        a.header,
        &a.delimiter,
        &a.output,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(input: &str) -> Args {
        Args {
            input: input.into(),
            column: default_auto(),
            fiscal_start_month: default_start_month(),
            fiscal_year_naming: default_naming(),
            quarter_label: default_quarter_label(),
            fiscal_year_label: default_year_label(),
            add_fiscal_year: true,
            add_quarter_dates: false,
            add_fiscal_month: false,
            add_quarter_position: false,
            date_order: default_auto(),
            on_error: default_on_error(),
            header: true,
            delimiter: default_auto(),
            output: default_output(),
        }
    }

    #[test]
    fn run_args_uses_defaults() {
        let out = run_args(args("invoice,closed\nA-1,2025-10-14\n")).unwrap();
        assert_eq!(
            out,
            "invoice,closed,fiscal_quarter,fiscal_year\nA-1,2025-10-14,Q4 FY2025,FY2025\n"
        );
    }

    #[test]
    fn run_args_honours_a_non_default_fiscal_start() {
        let mut a = args("invoice,closed\nA-1,2025-10-14\n");
        a.fiscal_start_month = "october".into();
        let out = run_args(a).unwrap();
        assert_eq!(
            out,
            "invoice,closed,fiscal_quarter,fiscal_year\nA-1,2025-10-14,Q1 FY2026,FY2026\n"
        );
    }

    #[test]
    fn run_args_surfaces_a_core_error() {
        let mut a = args("invoice,closed\nA-1,2025-10-14\n");
        a.column = "when".into();
        let err = run_args(a).unwrap_err();
        assert_eq!(err, "column 'when' not found in header (invoice, closed)");
    }

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored manifest schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "CSV/TSV text with a date column, e.g. \"invoice,closed\\nA-1,2025-10-14\". Include the header row when header=true. Capped at 5,000,000 bytes." },
                    "column": { "type": "string", "default": "auto", "description": "Which column holds the dates: a header name (\"closed\"), a 0-based index (\"1\"), or \"auto\" (default) to pick the first column where at least 60% of non-blank cells parse as a date. With header=false only an index works." },
                    "fiscal_start_month": { "type": "string", "enum": ["january","february","march","april","may","june","july","august","september","october","november","december"], "default": "january", "description": "Calendar month the fiscal year starts in. january = calendar year (default), april = UK/India, july = Australia/NZ, october = US federal." },
                    "fiscal_year_naming": { "type": "string", "enum": ["end","start"], "default": "end", "description": "Which calendar year names the fiscal year. \"end\" (default) matches the US federal government and pandas: Oct 2025 is FY2026. \"start\" matches the common spreadsheet recipe: Oct 2025 is FY2025. The two conventions differ by one, so pick deliberately." },
                    "quarter_label": { "type": "string", "enum": ["q-fy","fy-q","yyyy-qn","yyyyqn","qn","n"], "default": "q-fy", "description": "Format of the fiscal_quarter column: q-fy = \"Q1 FY2026\" (default), fy-q = \"FY2026-Q1\", yyyy-qn = \"2026-Q1\", yyyyqn = \"2026Q1\" (pandas style), qn = \"Q1\", n = \"1\"." },
                    "fiscal_year_label": { "type": "string", "enum": ["fy-yyyy","yyyy","fy-yy","range","range-short"], "default": "fy-yyyy", "description": "Format of the fiscal_year column (and of the year embedded in q-fy/fy-q quarter labels): fy-yyyy = \"FY2026\" (default), yyyy = \"2026\", fy-yy = \"FY26\", range = \"2025-2026\", range-short = \"2025-26\". Ranges collapse to one year when the fiscal year starts in january." },
                    "add_fiscal_year": { "type": "boolean", "default": true, "description": "Append a fiscal_year column next to fiscal_quarter. Default true." },
                    "add_quarter_dates": { "type": "boolean", "default": false, "description": "Append fiscal_quarter_start and fiscal_quarter_end columns as ISO YYYY-MM-DD dates. Default false." },
                    "add_fiscal_month": { "type": "boolean", "default": false, "description": "Append a fiscal_month column numbered 1-12 from the fiscal start month (october start: October = 1). Default false." },
                    "add_quarter_position": { "type": "boolean", "default": false, "description": "Append day_of_quarter and days_in_quarter columns, measured from the row's own date (quarters run 90-92 days). Default false." },
                    "date_order": { "type": "string", "enum": ["auto","day-first","month-first"], "default": "auto", "description": "How to read an all-numeric value like 03/04/2024. \"auto\" (default) settles the whole column from rows that can only be read one way (a day above 12) and falls back to month-first; day-first forces DD/MM/YYYY; month-first forces MM/DD/YYYY. ISO YYYY-MM-DD is unaffected." },
                    "on_error": { "type": "string", "enum": ["blank","drop","error"], "default": "blank", "description": "What to do with a cell that cannot be read as a date: blank (default) leaves the added columns empty, drop removes the row, error stops and names the offending row and line." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as column names. Default true. Turn off for a bare list of dates with no header row." },
                    "delimiter": { "type": "string", "enum": ["auto","comma","tab","semicolon","pipe"], "default": "auto", "description": "Input/output delimiter. auto (default) sniffs comma, tab, semicolon, or pipe from the first non-blank line; choose one to force it. The output uses the same delimiter as the input." },
                    "output": { "type": "string", "enum": ["csv","report","json"], "default": "csv", "description": "csv (default) returns the input with the fiscal columns appended; report returns a plain-text audit (column picked, date order and the evidence for it, rows per quarter, unreadable values); json returns that audit plus the CSV as a JSON object." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
