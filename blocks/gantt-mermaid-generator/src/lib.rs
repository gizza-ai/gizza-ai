//! gizza-ai/gantt-mermaid-generator — turn a task table into Mermaid `gantt`
//! source. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    tasks: String,
    #[serde(default)]
    title: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_date_format")]
    date_format: String,
    #[serde(default)]
    axis_format: String,
    #[serde(default)]
    tick_interval: String,
    #[serde(default)]
    exclude_weekends: bool,
    #[serde(default = "default_weekend")]
    weekend: String,
    #[serde(default)]
    excludes: String,
    #[serde(default = "default_true")]
    today_marker: bool,
    #[serde(default)]
    compact: bool,
    #[serde(default)]
    fence: bool,
}

fn default_delimiter() -> String {
    "auto".into()
}
fn default_date_format() -> String {
    "YYYY-MM-DD".into()
}
fn default_weekend() -> String {
    "saturday".into()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("tasks").required().describe("The task table, one task per line, with up to five columns separated by a pipe, a tab or a comma: `name | start | duration | tags | id`. `start` is a date in the chosen date_format, `after <task name or id>` (comma-separate several), or empty to start when the previous line's task ends. `duration` is a length such as `5d`, `2w`, `36h` or a bare number meaning days, an end date, or `until <task>`. `tags` are any of done, active, crit, milestone. `id` is optional — an id is otherwise derived from the task name, so dependencies can name the task directly. A line starting with `section ` or `## ` opens a section; blank lines and lines starting with #, // or %% are ignored. Example: `Design | 2026-03-02 | 5d | done` then `Build | after Design | 2w | active`. Up to 500 tasks, 100 sections and 2 MB."))
        .param(Param::string("title").default("").describe("Optional chart title rendered above the timeline, e.g. `Q2 launch plan`. Leave blank for no title line."))
        .param(Param::enumv("delimiter", ["auto", "pipe", "comma", "tab"]).default("auto").describe("Column separator for the task table. `auto` (default) looks at the first task line and picks pipe, then tab, then comma. Choose `pipe` when task names contain commas."))
        .param(Param::enumv("date_format", ["YYYY-MM-DD", "YYYY/MM/DD", "DD-MM-YYYY", "MM-DD-YYYY", "DD/MM/YYYY", "MM/DD/YYYY", "YYYY-MM-DD HH:mm", "YYYY-MM-DDTHH:mm:ss", "X"]).default("YYYY-MM-DD").describe("How dates in the task table are written; emitted as Mermaid's `dateFormat` and used to validate every date you type. Default YYYY-MM-DD (2026-03-02). `X` means Unix epoch seconds, and with it every bare number is read as a timestamp, so durations then need a unit such as 5d."))
        .param(Param::string("axis_format").default("").describe("Optional Mermaid `axisFormat`: a strftime pattern for the date labels along the axis, e.g. `%b %d` for `Mar 02`, `%Y-%m-%d`, or `%d/%m`. Leave blank to let Mermaid choose."))
        .param(Param::string("tick_interval").default("").describe("Optional Mermaid `tickInterval` spacing for axis ticks: a count then a unit, e.g. `1week`, `2day`, `6hour`, `1month`. Units are millisecond, second, minute, hour, day, week, month. Leave blank for automatic spacing."))
        .param(Param::boolean("exclude_weekends").default(false).describe("When true, add `weekends` to Mermaid's `excludes` line so task bars skip weekend days. Default false."))
        .param(Param::enumv("weekend", ["saturday", "friday"]).default("saturday").describe("Which day the weekend starts on when weekends are excluded: `saturday` (Sat+Sun, the default) or `friday` (Fri+Sat). Emitted as Mermaid's `weekend` directive, which needs Mermaid 11 or newer."))
        .param(Param::string("excludes").default("").describe("Extra non-working days to skip, comma or space separated: weekday names such as `monday`, the word `weekends`, or specific dates in the chosen date_format, e.g. `2026-04-03, 2026-04-06, monday`. Leave blank to exclude nothing beyond the exclude_weekends toggle."))
        .param(Param::boolean("today_marker").default(true).describe("Keep Mermaid's vertical today line. True (default) leaves it on; false emits `todayMarker off`, which is what you want for a historical or purely illustrative chart."))
        .param(Param::boolean("compact").default(false).describe("When true, emit `displayMode: compact` front matter so Mermaid packs non-overlapping tasks onto shared rows, making long plans much shorter. Needs Mermaid 10 or newer. Default false."))
        .param(Param::boolean("fence").default(false).describe("Wrap the output in a ```mermaid fenced code block, ready to paste into a Markdown file, a README or an issue. Default false, which returns bare Mermaid source."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gantt-mermaid-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a Mermaid gantt chart from a task table with dates, durations and dependencies",
    skill(
        description = "Turn a plain task table into Mermaid `gantt` diagram source. Each line is `name | start | duration | tags | id` (pipe, tab or comma separated): the start is a date, `after <task>` to chain off another task, or blank to follow the previous line; the duration is a length such as 5d, 2w or 36h, a bare number meaning days, an end date, or `until <task>`. Task ids are derived from task names, so dependencies reference tasks by name instead of hand-made ids. `section Name` (or `## Name`) groups the tasks below it, and the done, active, crit and milestone tags map to Mermaid's task styling, with milestones defaulting to a zero-length marker. Chart-level options cover the title, dateFormat (including DD/MM/YYYY and Unix epoch), axisFormat, tickInterval, excluding weekends or specific dates, moving the weekend to Friday, turning the today marker off, compact display mode, and wrapping the result in a ```mermaid Markdown fence. Dates are validated against the chosen format, dependencies must resolve and must be defined earlier, and errors name the offending line. Caps: 500 tasks, 100 sections, 2 MB. Returns plain-text Mermaid source that renders in GitHub, GitLab, Notion or any Mermaid-compatible viewer.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gantt-mermaid-generator", |a: Args| {
            gizza_ai_gantt_mermaid_generator_core::generate(
                &a.tasks,
                &a.title,
                &a.delimiter,
                &a.date_format,
                &a.axis_format,
                &a.tick_interval,
                a.exclude_weekends,
                &a.weekend,
                &a.excludes,
                a.today_marker,
                a.compact,
                a.fence,
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

    /// Drift guard: the derived chat schema must equal the authored one. When a
    /// descriptor change is intentional, REGENERATE this literal.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            "properties": {
                "tasks": { "type": "string", "description": "The task table, one task per line, with up to five columns separated by a pipe, a tab or a comma: `name | start | duration | tags | id`. `start` is a date in the chosen date_format, `after <task name or id>` (comma-separate several), or empty to start when the previous line's task ends. `duration` is a length such as `5d`, `2w`, `36h` or a bare number meaning days, an end date, or `until <task>`. `tags` are any of done, active, crit, milestone. `id` is optional — an id is otherwise derived from the task name, so dependencies can name the task directly. A line starting with `section ` or `## ` opens a section; blank lines and lines starting with #, // or %% are ignored. Example: `Design | 2026-03-02 | 5d | done` then `Build | after Design | 2w | active`. Up to 500 tasks, 100 sections and 2 MB." },
                "title": { "type": "string", "default": "", "description": "Optional chart title rendered above the timeline, e.g. `Q2 launch plan`. Leave blank for no title line." },
                "delimiter": { "type": "string", "enum": ["auto", "pipe", "comma", "tab"], "default": "auto", "description": "Column separator for the task table. `auto` (default) looks at the first task line and picks pipe, then tab, then comma. Choose `pipe` when task names contain commas." },
                "date_format": { "type": "string", "enum": ["YYYY-MM-DD", "YYYY/MM/DD", "DD-MM-YYYY", "MM-DD-YYYY", "DD/MM/YYYY", "MM/DD/YYYY", "YYYY-MM-DD HH:mm", "YYYY-MM-DDTHH:mm:ss", "X"], "default": "YYYY-MM-DD", "description": "How dates in the task table are written; emitted as Mermaid's `dateFormat` and used to validate every date you type. Default YYYY-MM-DD (2026-03-02). `X` means Unix epoch seconds, and with it every bare number is read as a timestamp, so durations then need a unit such as 5d." },
                "axis_format": { "type": "string", "default": "", "description": "Optional Mermaid `axisFormat`: a strftime pattern for the date labels along the axis, e.g. `%b %d` for `Mar 02`, `%Y-%m-%d`, or `%d/%m`. Leave blank to let Mermaid choose." },
                "tick_interval": { "type": "string", "default": "", "description": "Optional Mermaid `tickInterval` spacing for axis ticks: a count then a unit, e.g. `1week`, `2day`, `6hour`, `1month`. Units are millisecond, second, minute, hour, day, week, month. Leave blank for automatic spacing." },
                "exclude_weekends": { "type": "boolean", "default": false, "description": "When true, add `weekends` to Mermaid's `excludes` line so task bars skip weekend days. Default false." },
                "weekend": { "type": "string", "enum": ["saturday", "friday"], "default": "saturday", "description": "Which day the weekend starts on when weekends are excluded: `saturday` (Sat+Sun, the default) or `friday` (Fri+Sat). Emitted as Mermaid's `weekend` directive, which needs Mermaid 11 or newer." },
                "excludes": { "type": "string", "default": "", "description": "Extra non-working days to skip, comma or space separated: weekday names such as `monday`, the word `weekends`, or specific dates in the chosen date_format, e.g. `2026-04-03, 2026-04-06, monday`. Leave blank to exclude nothing beyond the exclude_weekends toggle." },
                "today_marker": { "type": "boolean", "default": true, "description": "Keep Mermaid's vertical today line. True (default) leaves it on; false emits `todayMarker off`, which is what you want for a historical or purely illustrative chart." },
                "compact": { "type": "boolean", "default": false, "description": "When true, emit `displayMode: compact` front matter so Mermaid packs non-overlapping tasks onto shared rows, making long plans much shorter. Needs Mermaid 10 or newer. Default false." },
                "fence": { "type": "boolean", "default": false, "description": "Wrap the output in a ```mermaid fenced code block, ready to paste into a Markdown file, a README or an issue. Default false, which returns bare Mermaid source." }
            },
            "required": ["tasks"],
            "additionalProperties": false
        });
        assert_eq!(derived, authored);
    }

    #[test]
    fn descriptor_params_cover_every_arg_field() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for name in [
            "tasks",
            "title",
            "delimiter",
            "date_format",
            "axis_format",
            "tick_interval",
            "exclude_weekends",
            "weekend",
            "excludes",
            "today_marker",
            "compact",
            "fence",
        ] {
            let p = props.get(name).unwrap_or_else(|| panic!("missing {name}"));
            assert!(
                p["description"].as_str().is_some_and(|d| d.len() > 30),
                "{name} needs a useful describe()"
            );
        }
    }
}
