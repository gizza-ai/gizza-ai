//! gizza-ai/log-to-metrics — chat skill block on the shared tool abstraction.
//! Aggregates already-structured log lines (JSON/NDJSON, logfmt, CSV) into
//! RED-style metrics: counts, throughput rates, error rates and exact latency
//! percentiles, grouped by any field. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill and the pure logic lives in
//! gizza-ai-log-to-metrics-core. No host calls — the whole pipeline runs in the
//! sandbox, and the same log always produces the same numbers.
//!
//! Stated limits (also in the skill description so an LLM can relay them):
//!   * 2,000,000 characters / 200,000 lines / 50,000 distinct groups per run;
//!   * one-shot batch aggregation — one row per group over the whole input, no
//!     time-bucketed series, no streaming, nothing persisted between runs;
//!   * percentiles are exact over the pasted batch (no sketching, no sampling).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_format() -> String {
    "auto".into()
}
fn default_percentiles() -> String {
    "50,95,99".into()
}
fn default_percentile_method() -> String {
    "linear".into()
}
fn default_rate_unit() -> String {
    "auto".into()
}
fn default_limit() -> u32 {
    20
}
fn default_other() -> bool {
    true
}
fn default_sort() -> String {
    "count".into()
}
fn default_output() -> String {
    "table".into()
}
fn default_metric_prefix() -> String {
    "log".into()
}

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    group_by: String,
    #[serde(default)]
    value_field: String,
    #[serde(default = "default_percentiles")]
    percentiles: String,
    #[serde(default = "default_percentile_method")]
    percentile_method: String,
    #[serde(default)]
    time_field: String,
    #[serde(default = "default_rate_unit")]
    rate_unit: String,
    #[serde(default)]
    error_field: String,
    #[serde(default)]
    error_values: String,
    #[serde(default = "default_limit")]
    limit: u32,
    #[serde(default = "default_other")]
    other: bool,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_metric_prefix")]
    metric_prefix: String,
}

/// Single source for the chat schema (and CLI). `data` is the pasted log; every
/// other param either selects a field to aggregate on (group_by, value_field,
/// time_field, error_field) or controls the maths and the rendering.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .multiline()
                .describe("The structured log lines to aggregate, newline-separated. One JSON object per line (NDJSON), logfmt 'key=value' pairs, or a CSV/TSV block whose first row is the header. Nested JSON objects flatten to dotted paths, so {\"http\":{\"status\":500}} is addressable as 'http.status'. Up to 2000000 characters and 200000 lines per run; blank lines are ignored and lines that fail to parse are counted, not fatal."),
        )
        .param(
            Param::enumv("format", ["auto", "json", "logfmt", "csv"])
                .default("auto")
                .describe("How to read each line. 'auto' (default) sniffs the first few non-blank lines: a leading '{' means JSON, parseable 'key=value' pairs mean logfmt, otherwise CSV. 'json' = one JSON object per line (NDJSON); 'logfmt' = whitespace-separated key=value pairs with optional double-quoted values; 'csv' = a header row plus delimited rows, delimiter auto-detected between comma, tab and semicolon. Set it explicitly when a mixed or unusual log confuses the sniffer."),
        )
        .param(
            Param::string("group_by")
                .default("")
                .describe("Comma-separated field names to group by, up to 5 — e.g. 'route' or 'service,status'. Names are matched case-insensitively and may be dotted paths into nested JSON. A row whose field is absent, null or empty is grouped as '(missing)' rather than dropped. Leave blank (the default) to fold every line into a single '(all)' row, which is the fastest way to get overall totals and percentiles."),
        )
        .param(
            Param::string("value_field")
                .default("")
                .describe("The numeric field to summarise per group — typically a latency such as 'duration_ms', or a size such as 'bytes'. When set, the output gains min, avg, the requested percentiles, max and sum. Plain numbers are used as-is; a value carrying a duration suffix (ns, us, microseconds, ms, s, m, h) is normalised to milliseconds, so '1.5s' counts as 1500. Values that are missing or non-numeric are counted and reported, never silently treated as zero. Leave blank (the default) for counts only."),
        )
        .param(
            Param::string("percentiles")
                .default("50,95,99")
                .describe("Comma-separated percentiles to compute over value_field, 0-100, up to 10 of them (default '50,95,99'). Decimals are allowed ('99.9', '99.99') and a leading 'p' is accepted ('p95'). Ignored when value_field is blank. Percentiles are exact over the pasted batch — every value is kept and sorted, so there is no sketching error and repeated runs give identical numbers."),
        )
        .param(
            Param::enumv("percentile_method", ["linear", "nearest"])
                .default("linear")
                .describe("How a percentile is read off the sorted values. 'linear' (default) interpolates between the two neighbouring order statistics (the numpy/R type-7 definition), so p50 of [1,2,3,4] is 2.5. 'nearest' uses nearest-rank — the smallest value at or above the requested rank, so p50 of [1,2,3,4] is 2 — which is what most exporters and log-search tools report and always returns a value that actually occurred in the log."),
        )
        .param(
            Param::string("time_field")
                .default("")
                .describe("The timestamp field used to measure the log's own time span, which is what turns counts into a rate. Leave blank (the default) to auto-detect the first field named timestamp, @timestamp, time, ts, datetime, date, event_time, eventtime or _time that holds a parseable value. Set 'none' to skip timestamps entirely and drop the rate column. Understood formats: epoch seconds/millis/micros/nanos (auto-scaled), ISO-8601 / RFC-3339 with an optional Z or +HH:MM offset, and the Apache/nginx '10/Oct/2000:13:55:36 -0700' form. A rate needs at least two distinct timestamps."),
        )
        .param(
            Param::enumv("rate_unit", ["auto", "second", "minute", "hour"])
                .default("auto")
                .describe("Denominator of the rate column. 'auto' (default) picks from the span itself: per second when the log covers under a minute, per minute when it covers under an hour, per hour beyond that. 'second', 'minute' and 'hour' force the unit so two runs stay comparable. The rate is count divided by the span between the earliest and latest timestamp in the input — a batch measure of the window the log covers, not a sliding window."),
        )
        .param(
            Param::string("error_field")
                .default("")
                .describe("The field inspected to decide whether a line is an error — usually 'status', 'level' or 'severity'. When set, the output gains an errors count and an error percent per group. Leave blank (the default) to skip error counting entirely."),
        )
        .param(
            Param::string("error_values")
                .default("")
                .describe("Comma-separated rules deciding which error_field values count as errors. Blank (the default) uses the built-in set: 5* (any HTTP 5xx), error, err, fatal, critical, crit, panic, emerg, alert. A plain word matches case-insensitively and exactly; a trailing '*' is a prefix match ('4*'); and '>=500', '>499', '<=299', '<400' compare numerically. Ignored when error_field is blank."),
        )
        .param(
            Param::integer("limit")
                .min(1.0)
                .max(1000.0)
                .default(20)
                .describe("How many groups to show after sorting, 1-1000 (default 20). The full group count is always reported alongside, so a truncated table is never mistaken for the whole log, and the totals row always covers every parsed line whether or not it was shown."),
        )
        .param(
            Param::boolean("other")
                .default(true)
                .describe("When true (the default), the groups beyond limit are merged into a single '(other)' row instead of vanishing. Its percentiles are recomputed from the merged values rather than averaged, so they stay exact. Turn it off for a clean top-N list."),
        )
        .param(
            Param::enumv("sort", ["count", "group", "sum", "avg", "max", "errors", "p_top"])
                .default("count")
                .describe("How groups are ranked before limit is applied. 'count' (default) = most lines first; 'group' = alphabetical by the group key; 'sum', 'avg' and 'max' = largest total, mean or maximum of value_field first; 'errors' = most errors first; 'p_top' = largest value of the highest requested percentile first, which is the usual way to find the slowest endpoints. Ties break by group name so the order is stable."),
        )
        .param(
            Param::enumv("output", ["table", "json", "csv", "prometheus"])
                .default("table")
                .describe("Rendering of the result. 'table' (default) = a summary line (lines, parsed, unparsed, format, groups, span, value coverage) followed by an aligned Markdown table; 'json' = a full report {format, lines, parsed, unparsed, group_by, groups_found, groups_shown, time_field, span_seconds, rate_unit, value_field, value_parsed, value_missing, value_non_numeric, percentiles, percentile_method, error_field, error_values, totals, groups[], other}; 'csv' = the same table as comma-separated rows with a header, ready for a spreadsheet; 'prometheus' = text exposition with a lines counter, a rate gauge, an errors counter and a summary carrying one quantile series per requested percentile."),
        )
        .param(
            Param::string("metric_prefix")
                .default("log")
                .describe("Metric name prefix used by output='prometheus' (default 'log', giving log_lines_total, log_errors_total and so on). Characters that Prometheus disallows are replaced with underscores, and a blank or otherwise unusable prefix falls back to 'log'. Ignored by the other output formats."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/log-to-metrics",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Aggregate structured logs into per-group counts, rates, error rates and latency percentiles.",
    skill(
        description = "Turn a batch of already-structured log lines into RED-style metrics: request count and share of total, throughput rate, error count and error rate, and exact latency percentiles, grouped by any field or combination of fields. Reads NDJSON (one JSON object per line, nested objects flattened to dotted paths such as http.status), logfmt key=value pairs, or CSV/TSV with a header row — format='auto' sniffs it. Set group_by to a comma list of up to 5 fields ('route' or 'service,status'; blank folds everything into one '(all)' row, missing values group as '(missing)'), value_field to the numeric column to summarise (duration suffixes like 250ms or 1.5s normalise to milliseconds; min/avg/max/sum plus the percentiles listed in percentiles, default '50,95,99', computed exactly with percentile_method 'linear' (numpy/R-7) or 'nearest' (nearest-rank)), and error_field plus error_values ('5*' prefixes, '>=500' comparisons, or the built-in error/fatal/critical set) for error rates. Rates come from the log's own time span via time_field (auto-detected from timestamp/@timestamp/time/ts/…, or 'none' to skip; epoch, ISO-8601 and Apache/nginx timestamps understood) with rate_unit 'auto'/'second'/'minute'/'hour'. Rank with sort ('count', 'group', 'sum', 'avg', 'max', 'errors', 'p_top' for the slowest groups), cap with limit (default 20) and keep the remainder as an '(other)' row with other=true. output='table' (default) is a summary line plus an aligned table; 'json' is a full report with totals; 'csv' is spreadsheet-ready; 'prometheus' is text exposition (counters, a rate gauge and a summary with quantile labels) named from metric_prefix. Deterministic and stateless: one pass in input order, exact percentiles over the batch (no sketching or sampling), same input always gives the same numbers, nothing persisted between runs. One-shot batch aggregation only — one row per group over the whole input, not a time-bucketed series, no streaming or live tailing, and no query language for filtering before aggregation. Up to 2000000 characters, 200000 lines and 50000 distinct groups per run.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "log-to-metrics", |a: Args| {
            gizza_ai_log_to_metrics_core::aggregate(
                &a.data,
                &a.format,
                &a.group_by,
                &a.value_field,
                &a.percentiles,
                &a.percentile_method,
                &a.time_field,
                &a.rate_unit,
                &a.error_field,
                &a.error_values,
                a.limit,
                a.other,
                &a.sort,
                &a.output,
                &a.metric_prefix,
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
        let authored: serde_json::Value = serde_json::from_str(AUTHORED).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    const AUTHORED: &str = r##"{
        "type": "object",
        "properties": {
            "data": {
                "type": "string",
                "description": "The structured log lines to aggregate, newline-separated. One JSON object per line (NDJSON), logfmt 'key=value' pairs, or a CSV/TSV block whose first row is the header. Nested JSON objects flatten to dotted paths, so {\"http\":{\"status\":500}} is addressable as 'http.status'. Up to 2000000 characters and 200000 lines per run; blank lines are ignored and lines that fail to parse are counted, not fatal."
            },
            "format": {
                "type": "string",
                "enum": ["auto", "json", "logfmt", "csv"],
                "default": "auto",
                "description": "How to read each line. 'auto' (default) sniffs the first few non-blank lines: a leading '{' means JSON, parseable 'key=value' pairs mean logfmt, otherwise CSV. 'json' = one JSON object per line (NDJSON); 'logfmt' = whitespace-separated key=value pairs with optional double-quoted values; 'csv' = a header row plus delimited rows, delimiter auto-detected between comma, tab and semicolon. Set it explicitly when a mixed or unusual log confuses the sniffer."
            },
            "group_by": {
                "type": "string",
                "default": "",
                "description": "Comma-separated field names to group by, up to 5 — e.g. 'route' or 'service,status'. Names are matched case-insensitively and may be dotted paths into nested JSON. A row whose field is absent, null or empty is grouped as '(missing)' rather than dropped. Leave blank (the default) to fold every line into a single '(all)' row, which is the fastest way to get overall totals and percentiles."
            },
            "value_field": {
                "type": "string",
                "default": "",
                "description": "The numeric field to summarise per group — typically a latency such as 'duration_ms', or a size such as 'bytes'. When set, the output gains min, avg, the requested percentiles, max and sum. Plain numbers are used as-is; a value carrying a duration suffix (ns, us, microseconds, ms, s, m, h) is normalised to milliseconds, so '1.5s' counts as 1500. Values that are missing or non-numeric are counted and reported, never silently treated as zero. Leave blank (the default) for counts only."
            },
            "percentiles": {
                "type": "string",
                "default": "50,95,99",
                "description": "Comma-separated percentiles to compute over value_field, 0-100, up to 10 of them (default '50,95,99'). Decimals are allowed ('99.9', '99.99') and a leading 'p' is accepted ('p95'). Ignored when value_field is blank. Percentiles are exact over the pasted batch — every value is kept and sorted, so there is no sketching error and repeated runs give identical numbers."
            },
            "percentile_method": {
                "type": "string",
                "enum": ["linear", "nearest"],
                "default": "linear",
                "description": "How a percentile is read off the sorted values. 'linear' (default) interpolates between the two neighbouring order statistics (the numpy/R type-7 definition), so p50 of [1,2,3,4] is 2.5. 'nearest' uses nearest-rank — the smallest value at or above the requested rank, so p50 of [1,2,3,4] is 2 — which is what most exporters and log-search tools report and always returns a value that actually occurred in the log."
            },
            "time_field": {
                "type": "string",
                "default": "",
                "description": "The timestamp field used to measure the log's own time span, which is what turns counts into a rate. Leave blank (the default) to auto-detect the first field named timestamp, @timestamp, time, ts, datetime, date, event_time, eventtime or _time that holds a parseable value. Set 'none' to skip timestamps entirely and drop the rate column. Understood formats: epoch seconds/millis/micros/nanos (auto-scaled), ISO-8601 / RFC-3339 with an optional Z or +HH:MM offset, and the Apache/nginx '10/Oct/2000:13:55:36 -0700' form. A rate needs at least two distinct timestamps."
            },
            "rate_unit": {
                "type": "string",
                "enum": ["auto", "second", "minute", "hour"],
                "default": "auto",
                "description": "Denominator of the rate column. 'auto' (default) picks from the span itself: per second when the log covers under a minute, per minute when it covers under an hour, per hour beyond that. 'second', 'minute' and 'hour' force the unit so two runs stay comparable. The rate is count divided by the span between the earliest and latest timestamp in the input — a batch measure of the window the log covers, not a sliding window."
            },
            "error_field": {
                "type": "string",
                "default": "",
                "description": "The field inspected to decide whether a line is an error — usually 'status', 'level' or 'severity'. When set, the output gains an errors count and an error percent per group. Leave blank (the default) to skip error counting entirely."
            },
            "error_values": {
                "type": "string",
                "default": "",
                "description": "Comma-separated rules deciding which error_field values count as errors. Blank (the default) uses the built-in set: 5* (any HTTP 5xx), error, err, fatal, critical, crit, panic, emerg, alert. A plain word matches case-insensitively and exactly; a trailing '*' is a prefix match ('4*'); and '>=500', '>499', '<=299', '<400' compare numerically. Ignored when error_field is blank."
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "maximum": 1000,
                "default": 20,
                "description": "How many groups to show after sorting, 1-1000 (default 20). The full group count is always reported alongside, so a truncated table is never mistaken for the whole log, and the totals row always covers every parsed line whether or not it was shown."
            },
            "other": {
                "type": "boolean",
                "default": true,
                "description": "When true (the default), the groups beyond limit are merged into a single '(other)' row instead of vanishing. Its percentiles are recomputed from the merged values rather than averaged, so they stay exact. Turn it off for a clean top-N list."
            },
            "sort": {
                "type": "string",
                "enum": ["count", "group", "sum", "avg", "max", "errors", "p_top"],
                "default": "count",
                "description": "How groups are ranked before limit is applied. 'count' (default) = most lines first; 'group' = alphabetical by the group key; 'sum', 'avg' and 'max' = largest total, mean or maximum of value_field first; 'errors' = most errors first; 'p_top' = largest value of the highest requested percentile first, which is the usual way to find the slowest endpoints. Ties break by group name so the order is stable."
            },
            "output": {
                "type": "string",
                "enum": ["table", "json", "csv", "prometheus"],
                "default": "table",
                "description": "Rendering of the result. 'table' (default) = a summary line (lines, parsed, unparsed, format, groups, span, value coverage) followed by an aligned Markdown table; 'json' = a full report {format, lines, parsed, unparsed, group_by, groups_found, groups_shown, time_field, span_seconds, rate_unit, value_field, value_parsed, value_missing, value_non_numeric, percentiles, percentile_method, error_field, error_values, totals, groups[], other}; 'csv' = the same table as comma-separated rows with a header, ready for a spreadsheet; 'prometheus' = text exposition with a lines counter, a rate gauge, an errors counter and a summary carrying one quantile series per requested percentile."
            },
            "metric_prefix": {
                "type": "string",
                "default": "log",
                "description": "Metric name prefix used by output='prometheus' (default 'log', giving log_lines_total, log_errors_total and so on). Characters that Prometheus disallows are replaced with underscores, and a blank or otherwise unusable prefix falls back to 'log'. Ignored by the other output formats."
            }
        },
        "required": ["data"],
        "additionalProperties": false
    }"##;
}
