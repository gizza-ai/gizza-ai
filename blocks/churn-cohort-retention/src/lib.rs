//! gizza-ai/churn-cohort-retention — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}
fn default_periods() -> f64 {
    6.0
}

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    signups: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    signup_date: String,
    #[serde(default)]
    granularity: String,
    #[serde(default = "default_periods")]
    periods: f64,
    #[serde(default)]
    metric: String,
    #[serde(default)]
    values: String,
    #[serde(default)]
    as_of: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("Activity/event log as CSV/table text: one row per user action, with a user-id column and an activity-date column (e.g. \"user,date\\nu1,2024-01-05\"). Rows may use comma, tab, semicolon, or pipe."),
        )
        .param(
            Param::string("signups")
                .describe("Optional signup/users table as CSV/table text: one row per user, with the same user-id column and a signup-date column. When omitted, each user's cohort is their FIRST activity date instead."),
        )
        .param(
            Param::string("user")
                .describe("User-id column: a header name or a 1-based column index. Defaults to the first column. The same name is looked up in the signup table; if it is not there, the signup table's first column is used."),
        )
        .param(
            Param::string("date")
                .describe("Activity-date column in the activity data: a header name or a 1-based column index. Defaults to the second column."),
        )
        .param(
            Param::string("signup_date")
                .describe("Signup-date column in the signup table: a header name or a 1-based column index. Defaults to the second column. Ignored when no signup table is given."),
        )
        .param(
            Param::enumv("granularity", ["month", "week", "day"])
                .default("month")
                .describe("Cohort/period size: month (default), week (weeks start Monday), or day. Cohorts are labelled YYYY-MM for months and by the period's first date for weeks/days."),
        )
        .param(
            Param::integer("periods")
                .default(6)
                .min(1.0)
                .max(36.0)
                .describe("How many follow-up periods to report after signup, 1 to 36. Columns are P0 (the signup period) through P<periods>. Default 6."),
        )
        .param(
            Param::enumv("metric", ["retention", "churn"])
                .default("retention")
                .describe("retention (default) = active users divided by the cohort size. churn = users lost since the PREVIOUS period, divided by that period's active users; a negative churn rate means users came back."),
        )
        .param(
            Param::enumv("values", ["percent", "count", "both"])
                .default("percent")
                .describe("What each cell shows: percent (default), count (active users, or users lost for churn), or both (\"12 (60%)\")."),
        )
        .param(
            Param::string("as_of")
                .describe("Analysis date as YYYY-MM-DD. Periods a cohort has not aged into by this date are shown as '-' (unobservable) instead of 0. Defaults to the latest date found in the data."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("When true (default), the first row of each table is treated as column names so `user`/`date`/`signup_date` can reference them by name. Turn it off for headerless data and use 1-based column indexes."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"])
                .default("comma")
                .describe("Input delimiter for both tables: comma (default), tab, semicolon, or pipe."),
        )
        .param(
            Param::enumv("format", ["table", "csv", "json"])
                .default("table")
                .describe("Output format: table (aligned cohort grid plus a text curve, default), csv (the grid alone, for a spreadsheet), or json (structured per-cohort counts and percentages)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/churn-cohort-retention",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build cohort retention and churn tables and curves from signup and activity data.",
    skill(
        description = "Build a cohort retention or churn analysis from raw user data. Paste an activity/event log (a user-id column and an activity-date column) as `data`, and optionally a signup/users table as `signups`; users are grouped into monthly, weekly, or daily cohorts by their signup date, or by their first activity when no signup table is given. Returns a cohort-by-period grid (P0 = the signup period) with each cohort's size, the users active in each period, a weighted average row across the cohorts that can observe each period, and a text retention curve. Use metric='churn' for period-over-period loss instead of retention, values='count' or 'both' to show user counts, granularity to change the period size, periods to set how many follow-up periods to report, and as_of to set the analysis date (cells a cohort has not aged into show '-' rather than a misleading 0). Dates must be ISO-8601 (YYYY-MM-DD, or a timestamp) or a Unix epoch; ambiguous day/month-first dates are rejected. format='csv' returns the grid for a spreadsheet, format='json' returns structured numbers.",
        parameters = schema_json()
    )
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "churn-cohort-retention", |a: Args| {
            gizza_ai_churn_cohort_retention_core::analyze(
                &a.data,
                &a.signups,
                &a.user,
                &a.date,
                &a.signup_date,
                &a.granularity,
                a.periods,
                &a.metric,
                &a.values,
                &a.as_of,
                a.header,
                &a.delimiter,
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
                    "data": { "type": "string", "description": "Activity/event log as CSV/table text: one row per user action, with a user-id column and an activity-date column (e.g. \"user,date\\nu1,2024-01-05\"). Rows may use comma, tab, semicolon, or pipe." },
                    "signups": { "type": "string", "description": "Optional signup/users table as CSV/table text: one row per user, with the same user-id column and a signup-date column. When omitted, each user's cohort is their FIRST activity date instead." },
                    "user": { "type": "string", "description": "User-id column: a header name or a 1-based column index. Defaults to the first column. The same name is looked up in the signup table; if it is not there, the signup table's first column is used." },
                    "date": { "type": "string", "description": "Activity-date column in the activity data: a header name or a 1-based column index. Defaults to the second column." },
                    "signup_date": { "type": "string", "description": "Signup-date column in the signup table: a header name or a 1-based column index. Defaults to the second column. Ignored when no signup table is given." },
                    "granularity": { "type": "string", "enum": ["month", "week", "day"], "default": "month", "description": "Cohort/period size: month (default), week (weeks start Monday), or day. Cohorts are labelled YYYY-MM for months and by the period's first date for weeks/days." },
                    "periods": { "type": "integer", "default": 6, "minimum": 1, "maximum": 36, "description": "How many follow-up periods to report after signup, 1 to 36. Columns are P0 (the signup period) through P<periods>. Default 6." },
                    "metric": { "type": "string", "enum": ["retention", "churn"], "default": "retention", "description": "retention (default) = active users divided by the cohort size. churn = users lost since the PREVIOUS period, divided by that period's active users; a negative churn rate means users came back." },
                    "values": { "type": "string", "enum": ["percent", "count", "both"], "default": "percent", "description": "What each cell shows: percent (default), count (active users, or users lost for churn), or both (\"12 (60%)\")." },
                    "as_of": { "type": "string", "description": "Analysis date as YYYY-MM-DD. Periods a cohort has not aged into by this date are shown as '-' (unobservable) instead of 0. Defaults to the latest date found in the data." },
                    "header": { "type": "boolean", "default": true, "description": "When true (default), the first row of each table is treated as column names so `user`/`date`/`signup_date` can reference them by name. Turn it off for headerless data and use 1-based column indexes." },
                    "delimiter": { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Input delimiter for both tables: comma (default), tab, semicolon, or pipe." },
                    "format": { "type": "string", "enum": ["table", "csv", "json"], "default": "table", "description": "Output format: table (aligned cohort grid plus a text curve, default), csv (the grid alone, for a spreadsheet), or json (structured per-cohort counts and percentages)." }
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
