//! gizza-ai/worklog-summarizer — summarize timestamped worklogs by project, tag, day, or entry.
//! Thin wrapper around the pure core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_worklog_summarizer_core::{summarize, GroupBy, Options, OutputFormat, SortBy, Units};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    log: String,
    #[serde(default = "default_group_by")]
    group_by: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_units")]
    units: String,
    #[serde(default)]
    round: u32,
    #[serde(default)]
    max_entry: u32,
    #[serde(default)]
    end_time: String,
    #[serde(default)]
    from: String,
    #[serde(default)]
    to: String,
    #[serde(default)]
    filter: String,
    #[serde(default = "default_project")]
    default_project: String,
    #[serde(default = "default_sort")]
    sort: String,
}

fn default_group_by() -> String {
    "all".into()
}
fn default_output() -> String {
    "summary".into()
}
fn default_units() -> String {
    "hm".into()
}
fn default_project() -> String {
    "(untagged)".into()
}
fn default_sort() -> String {
    "duration".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("log").required().describe("Timestamped worklog text. Each non-comment line should start with a time such as '09:00 @acme writing', '2024-01-15 09:00 @acme writing', '[2024-01-15 09:00] writing', or '9:00am writing'. Duration is inferred until the next timestamp. Maximum 5000000 bytes."))
        .param(Param::enumv("group_by", ["all", "project", "tag", "day", "entry"]).default("all").describe("Which totals to show: all (projects, tags and days), project, tag, day, or entry for the original timestamped rows. Default all."))
        .param(Param::enumv("output", ["summary", "table", "csv", "json"]).default("summary").describe("Report format: summary (readable text with percentages and ASCII bars), table (tab-separated rows), csv, or json. Default summary."))
        .param(Param::enumv("units", ["hm", "decimal", "minutes"]).default("hm").describe("How to render durations: hm (for example 1h 30m), decimal hours, or raw minutes. Default hm."))
        .param(Param::integer("round").min(0.0).max(1440.0).default(0).describe("Round each entry to this many minutes before totaling (0 = no rounding). Common billing increments are 5, 6, 10, 15, 30 or 60."))
        .param(Param::integer("max_entry").min(0.0).max(1440.0).default(0).describe("Cap any single entry at this many minutes before totals are computed (0 = no cap). Useful when a forgotten stop would span a long break."))
        .param(Param::string("end_time").default("").describe("Optional time used to close the final still-running entry, such as 17:30 or 5:30pm. Blank leaves the final entry marked open with zero added time."))
        .param(Param::string("from").default("").describe("Inclusive start date filter in YYYY-MM-DD form. Blank keeps the beginning of the log; undated entries are excluded when a date filter is used."))
        .param(Param::string("to").default("").describe("Inclusive end date filter in YYYY-MM-DD form. Blank keeps the end of the log; undated entries are excluded when a date filter is used."))
        .param(Param::string("filter").default("").describe("Comma-separated project or tag filters, with optional trailing * prefix matching. Examples: '@acme', '+dev', 'ac*'. Blank includes every project and tag."))
        .param(Param::string("default_project").default("(untagged)").describe("Project label for entries with no @project, +tag or #tag token. Default '(untagged)'."))
        .param(Param::enumv("sort", ["duration", "name", "time"]).default("duration").describe("Sort grouped rows by duration descending, name, or first timestamp. Default duration."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WorklogSummarizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/worklog-summarizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Summarize timestamped worklogs by project, tag, day or entry",
    skill(
        description = "Parse a pasted timestamped worklog or doing log and total time by project, tag, day, or entry. Durations are inferred from consecutive timestamps; stop markers such as done/end/stop close the previous entry without adding their own time. Recognizes dated and undated lines, bracketed timestamps, ISO T timestamps, 24-hour times, and 12-hour am/pm times. Inline @project, +tag, and #tag tokens become grouping keys. Options cover output format (summary, table, csv, json), duration units (h/m, decimal hours, minutes), per-entry rounding, max-entry caps, final end_time, from/to date filtering, project/tag filtering, default project label, and row sorting. Fully local and deterministic — no AI model, no upload.",
        parameters = schema_json()
    ),
)]
impl WorklogSummarizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "worklog-summarizer", |a: Args| {
            let opts = Options {
                group_by: GroupBy::parse(&a.group_by).map_err(SkillError::InvalidArgs)?,
                output: OutputFormat::parse(&a.output).map_err(SkillError::InvalidArgs)?,
                units: Units::parse(&a.units).map_err(SkillError::InvalidArgs)?,
                round: a.round as i64,
                max_entry: a.max_entry as i64,
                end_time: a.end_time,
                from: a.from,
                to: a.to,
                filter: a.filter,
                default_project: a.default_project,
                sort: SortBy::parse(&a.sort).map_err(SkillError::InvalidArgs)?,
            };
            summarize(&a.log, &opts).map_err(SkillError::InvalidArgs)
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
                    "log": { "type": "string", "description": "Timestamped worklog text. Each non-comment line should start with a time such as '09:00 @acme writing', '2024-01-15 09:00 @acme writing', '[2024-01-15 09:00] writing', or '9:00am writing'. Duration is inferred until the next timestamp. Maximum 5000000 bytes." },
                    "group_by": { "type": "string", "enum": ["all", "project", "tag", "day", "entry"], "default": "all", "description": "Which totals to show: all (projects, tags and days), project, tag, day, or entry for the original timestamped rows. Default all." },
                    "output": { "type": "string", "enum": ["summary", "table", "csv", "json"], "default": "summary", "description": "Report format: summary (readable text with percentages and ASCII bars), table (tab-separated rows), csv, or json. Default summary." },
                    "units": { "type": "string", "enum": ["hm", "decimal", "minutes"], "default": "hm", "description": "How to render durations: hm (for example 1h 30m), decimal hours, or raw minutes. Default hm." },
                    "round": { "type": "integer", "minimum": 0, "maximum": 1440, "default": 0, "description": "Round each entry to this many minutes before totaling (0 = no rounding). Common billing increments are 5, 6, 10, 15, 30 or 60." },
                    "max_entry": { "type": "integer", "minimum": 0, "maximum": 1440, "default": 0, "description": "Cap any single entry at this many minutes before totals are computed (0 = no cap). Useful when a forgotten stop would span a long break." },
                    "end_time": { "type": "string", "default": "", "description": "Optional time used to close the final still-running entry, such as 17:30 or 5:30pm. Blank leaves the final entry marked open with zero added time." },
                    "from": { "type": "string", "default": "", "description": "Inclusive start date filter in YYYY-MM-DD form. Blank keeps the beginning of the log; undated entries are excluded when a date filter is used." },
                    "to": { "type": "string", "default": "", "description": "Inclusive end date filter in YYYY-MM-DD form. Blank keeps the end of the log; undated entries are excluded when a date filter is used." },
                    "filter": { "type": "string", "default": "", "description": "Comma-separated project or tag filters, with optional trailing * prefix matching. Examples: '@acme', '+dev', 'ac*'. Blank includes every project and tag." },
                    "default_project": { "type": "string", "default": "(untagged)", "description": "Project label for entries with no @project, +tag or #tag token. Default '(untagged)'." },
                    "sort": { "type": "string", "enum": ["duration", "name", "time"], "default": "duration", "description": "Sort grouped rows by duration descending, name, or first timestamp. Default duration." }
                },
                "required": ["log"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
