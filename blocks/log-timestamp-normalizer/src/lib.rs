//! gizza-ai/log-timestamp-normalizer — chat skill block on the shared tool abstraction.
//! Rewrites the timestamps on a block of pasted log lines into one format and
//! timezone and annotates the elapsed time between consecutive events.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    log: String,
    #[serde(default = "default_output_format")]
    output_format: String,
    #[serde(default = "default_zone")]
    output_timezone: String,
    #[serde(default = "default_zone")]
    assume_timezone: String,
    #[serde(default)]
    assume_year: f64,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_output_mode")]
    output_mode: String,
    #[serde(default = "default_delta")]
    delta: bool,
    #[serde(default = "default_delta_format")]
    delta_format: String,
    #[serde(default)]
    gap_threshold_seconds: f64,
    #[serde(default = "default_unmatched")]
    unmatched: String,
    #[serde(default)]
    summary: bool,
}

fn default_output_format() -> String {
    "iso8601".to_string()
}
fn default_zone() -> String {
    "UTC".to_string()
}
fn default_sort() -> String {
    "input".to_string()
}
fn default_output_mode() -> String {
    "replace".to_string()
}
fn default_delta() -> bool {
    true
}
fn default_delta_format() -> String {
    "auto".to_string()
}
fn default_unmatched() -> String {
    "keep".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("log")
                .required()
                .describe("The log text to normalize, one event per line, pasted exactly as the service wrote it. Every line is detected on its own, so one paste can mix formats: ISO-8601/RFC 3339 (2023-12-01T10:15:30.123Z, 2023-12-01 10:15:30), RFC 2822 and HTTP dates (Fri, 01 Dec 2023 10:15:30 +0000), Apache/nginx (10/Oct/2000:13:55:36 -0700), 2023/12/01 10:15:30, syslog (Dec  1 10:15:30, no year and no zone), and epoch values at seconds (1701425730), milliseconds (1701425730123), microseconds, nanoseconds or fractional-second (1701425730.25) precision. Lines carrying no timestamp — stack-trace frames, banners, wrapped payloads — stay attached to the event above them. Up to 50000 lines per run."),
        )
        .param(
            Param::enumv(
                "output_format",
                [
                    "iso8601",
                    "iso8601_ms",
                    "epoch_seconds",
                    "epoch_millis",
                    "rfc2822",
                    "datetime",
                ],
            )
            .default("iso8601")
            .describe("How every normalized timestamp is written. \"iso8601\" (default) is 2023-12-01T10:15:30+00:00; \"iso8601_ms\" adds three fractional digits (2023-12-01T10:15:30.123+00:00); \"epoch_seconds\" (1701425730) and \"epoch_millis\" (1701425730123) are bare numbers and are always UTC, so output_timezone does not move them; \"rfc2822\" is Fri, 1 Dec 2023 10:15:30 +0000; \"datetime\" is a plain 2023-12-01 10:15:30 with no zone marker."),
        )
        .param(
            Param::string("output_timezone")
                .default("UTC")
                .describe("The timezone the normalized timestamps are written in. \"UTC\" by default. Takes \"UTC\", an IANA zone name such as \"Europe/Berlin\", \"America/New_York\" or \"Asia/Tokyo\" (daylight saving applied per timestamp from the bundled IANA database), or a fixed offset such as \"+02:00\", \"-0700\" or \"UTC+5:30\". The epoch_seconds and epoch_millis output formats ignore it, being UTC by definition."),
        )
        .param(
            Param::string("assume_timezone")
                .default("UTC")
                .describe("The timezone to read a timestamp in when it carries no zone of its own — a bare 2023-12-01 10:15:30, an Apache stamp with no offset, a syslog line. \"UTC\" by default, and it takes the same values as output_timezone. Set it to the zone the machine that wrote the log was configured for, or those lines land at the wrong instant. For a named zone the DST rules are applied per timestamp: an hour that repeats resolves to the earlier instant, an hour that never existed moves forward to the first instant that did. Stamps that already carry a zone or offset are unaffected."),
        )
        .param(
            Param::integer("assume_year")
                .default(0)
                .min(0.0)
                .max(2100.0)
                .describe("The year to give syslog-style stamps, which carry a month and day but no year (Dec  1 10:15:30). 0 (default) infers it from the nearest line in the same paste that does have a year, trying that year, the one before and the one after and keeping whichever lands closest in time — so a December-into-January paste does not jump twelve months at the rollover. When the paste has no dated line at all, those lines stay unmatched. Otherwise pass an explicit year between 1970 and 2100."),
        )
        .param(
            Param::enumv("sort", ["input", "oldest", "newest"])
                .default("input")
                .describe("The order the events come out in. \"input\" (default) leaves the log exactly as pasted — non-destructive, and what you want when the interleaving is the thing you are reading. \"oldest\" sorts oldest first and \"newest\" newest first, which is how you merge logs from several services into one timeline. Untimestamped lines travel with the event above them, so a stack trace is never torn off its error, and a leading banner stays at the top."),
        )
        .param(
            Param::enumv("output_mode", ["replace", "prefix", "timestamp"])
                .default("replace")
                .describe("What each output line is made of. \"replace\" (default) rewrites the timestamp where it sits and leaves the rest of the line untouched. \"prefix\" puts the normalized timestamp in front of the original line, so you can still see what the service actually wrote. \"timestamp\" emits only the normalized timestamp and its delta, which is what you want when lining two logs up side by side."),
        )
        .param(
            Param::boolean("delta")
                .default(true)
                .describe("Append the elapsed time since the previous event to each line in parentheses — \"2023-12-01T10:15:35+00:00 WARN cache miss  (+5.123s)\". On by default; the first event reads \"(start)\", and a step that goes backwards in time is signed with a minus. This is how the slow step in a boot or request trace becomes visible. Turn it off when the output is going into another parser."),
        )
        .param(
            Param::enumv("delta_format", ["auto", "seconds", "milliseconds", "hms"])
                .default("auto")
                .describe("How an elapsed time is rendered. \"auto\" (default) picks the readable unit per gap: 850ms, 5.123s, 1m45s, 2h03m10s, 1d04h30m. \"seconds\" always uses seconds (+105.5s) and \"milliseconds\" always milliseconds (+105500ms) — both easy to sort or graph — while \"hms\" uses a clock-style 0:01:45. It also formats the durations in the summary header."),
        )
        .param(
            Param::number("gap_threshold_seconds")
                .default(0.0)
                .min(0.0)
                .describe("Flag any step that took at least this many seconds by appending \" GAP\" to its delta, and count those steps in the summary header. 0 (default) turns gap flagging off. 60 picks out the minute-plus stalls in a boot log; 0.5 is the more useful setting on a request trace. Fractional values are allowed. The GAP marker needs delta on to be visible; the count is reported whenever summary is on."),
        )
        .param(
            Param::enumv("unmatched", ["keep", "drop", "mark"])
                .default("keep")
                .describe("What happens to lines with no detectable timestamp — stack-trace frames, wrapped payloads, banners. \"keep\" (default) passes them through untouched, in place under the event they followed. \"drop\" removes them, leaving exactly one line per detected event. \"mark\" keeps them and appends \"  (no timestamp)\" so it is obvious which lines the detector did not read."),
        )
        .param(
            Param::boolean("summary")
                .default(false)
                .describe("Prepend a header of \"#\" comment lines: how many lines were read and how many carried a timestamp, the mix of source formats detected with a count each, the span from first to last event, the largest gap and the input line it falls on, the number of gaps at or above gap_threshold_seconds, and the output format and zone. Off by default. Every header line starts with \"#\", so the result is still pasteable into a log viewer."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LogTimestampNormalizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/log-timestamp-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Normalize mixed log timestamps to one format and timezone, with the elapsed time between events",
    skill(
        description = "Rewrite the timestamps in a block of log text into one format and one timezone, and annotate how long passed between consecutive events. Paste the log into `log`, one event per line. Detection runs per line, so a single paste can mix formats: ISO-8601/RFC 3339 with or without fractional seconds and with a `T` or a space, RFC 2822 and HTTP dates, Apache/nginx `10/Oct/2000:13:55:36 -0700`, `2023/12/01 10:15:30`, syslog `Dec  1 10:15:30` (no year, no zone), and epoch values at second, millisecond, microsecond, nanosecond or fractional-second precision — the digit count picks the unit, and a more specific format on the same line beats a bare number. `output_format` picks iso8601 (default), iso8601_ms, epoch_seconds, epoch_millis, rfc2822 or datetime, and `output_timezone` picks the zone it is written in (UTC, an IANA name with DST applied per timestamp, or a fixed offset). Stamps that carry no zone are read in `assume_timezone`; syslog stamps carry no year either, so `assume_year` supplies it or, at the default 0, the year is inferred from the nearest dated line in the same paste. `delta` (on by default) appends the elapsed time since the previous event — `(+5.123s)`, `(start)` on the first — formatted by `delta_format` as auto, seconds, milliseconds or h:mm:ss, and `gap_threshold_seconds` marks any step at or above it with GAP. `output_mode` chooses whether each line keeps its original text with the stamp replaced (default), gets the normalized stamp prefixed, or is reduced to the stamp alone; `sort` leaves the paste order alone (default) or reorders oldest- or newest-first, carrying each event's untimestamped follow-on lines with it so a stack trace stays with its error. `unmatched` keeps, drops or marks lines with no timestamp, and `summary` prepends a `#` header with the line counts, the detected format mix, the span, the largest gap and the gap count. There is no clock and no I/O — every instant comes out of the pasted text, so the same input always produces the same output. Up to 50000 lines per run.",
        parameters = schema_json()
    ),
)]
impl LogTimestampNormalizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "log-timestamp-normalizer", |a: Args| {
            gizza_ai_log_timestamp_normalizer_core::run(
                &a.log,
                &a.output_format,
                &a.output_timezone,
                &a.assume_timezone,
                a.assume_year.round() as i64,
                &a.sort,
                &a.output_mode,
                a.delta,
                &a.delta_format,
                a.gap_threshold_seconds,
                &a.unmatched,
                a.summary,
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
            r##"{
                "type": "object",
                "properties": {
                    "log": { "type": "string", "description": "The log text to normalize, one event per line, pasted exactly as the service wrote it. Every line is detected on its own, so one paste can mix formats: ISO-8601/RFC 3339 (2023-12-01T10:15:30.123Z, 2023-12-01 10:15:30), RFC 2822 and HTTP dates (Fri, 01 Dec 2023 10:15:30 +0000), Apache/nginx (10/Oct/2000:13:55:36 -0700), 2023/12/01 10:15:30, syslog (Dec  1 10:15:30, no year and no zone), and epoch values at seconds (1701425730), milliseconds (1701425730123), microseconds, nanoseconds or fractional-second (1701425730.25) precision. Lines carrying no timestamp — stack-trace frames, banners, wrapped payloads — stay attached to the event above them. Up to 50000 lines per run." },
                    "output_format": { "type": "string", "enum": ["iso8601", "iso8601_ms", "epoch_seconds", "epoch_millis", "rfc2822", "datetime"], "default": "iso8601", "description": "How every normalized timestamp is written. \"iso8601\" (default) is 2023-12-01T10:15:30+00:00; \"iso8601_ms\" adds three fractional digits (2023-12-01T10:15:30.123+00:00); \"epoch_seconds\" (1701425730) and \"epoch_millis\" (1701425730123) are bare numbers and are always UTC, so output_timezone does not move them; \"rfc2822\" is Fri, 1 Dec 2023 10:15:30 +0000; \"datetime\" is a plain 2023-12-01 10:15:30 with no zone marker." },
                    "output_timezone": { "type": "string", "default": "UTC", "description": "The timezone the normalized timestamps are written in. \"UTC\" by default. Takes \"UTC\", an IANA zone name such as \"Europe/Berlin\", \"America/New_York\" or \"Asia/Tokyo\" (daylight saving applied per timestamp from the bundled IANA database), or a fixed offset such as \"+02:00\", \"-0700\" or \"UTC+5:30\". The epoch_seconds and epoch_millis output formats ignore it, being UTC by definition." },
                    "assume_timezone": { "type": "string", "default": "UTC", "description": "The timezone to read a timestamp in when it carries no zone of its own — a bare 2023-12-01 10:15:30, an Apache stamp with no offset, a syslog line. \"UTC\" by default, and it takes the same values as output_timezone. Set it to the zone the machine that wrote the log was configured for, or those lines land at the wrong instant. For a named zone the DST rules are applied per timestamp: an hour that repeats resolves to the earlier instant, an hour that never existed moves forward to the first instant that did. Stamps that already carry a zone or offset are unaffected." },
                    "assume_year": { "type": "integer", "default": 0, "minimum": 0, "maximum": 2100, "description": "The year to give syslog-style stamps, which carry a month and day but no year (Dec  1 10:15:30). 0 (default) infers it from the nearest line in the same paste that does have a year, trying that year, the one before and the one after and keeping whichever lands closest in time — so a December-into-January paste does not jump twelve months at the rollover. When the paste has no dated line at all, those lines stay unmatched. Otherwise pass an explicit year between 1970 and 2100." },
                    "sort": { "type": "string", "enum": ["input", "oldest", "newest"], "default": "input", "description": "The order the events come out in. \"input\" (default) leaves the log exactly as pasted — non-destructive, and what you want when the interleaving is the thing you are reading. \"oldest\" sorts oldest first and \"newest\" newest first, which is how you merge logs from several services into one timeline. Untimestamped lines travel with the event above them, so a stack trace is never torn off its error, and a leading banner stays at the top." },
                    "output_mode": { "type": "string", "enum": ["replace", "prefix", "timestamp"], "default": "replace", "description": "What each output line is made of. \"replace\" (default) rewrites the timestamp where it sits and leaves the rest of the line untouched. \"prefix\" puts the normalized timestamp in front of the original line, so you can still see what the service actually wrote. \"timestamp\" emits only the normalized timestamp and its delta, which is what you want when lining two logs up side by side." },
                    "delta": { "type": "boolean", "default": true, "description": "Append the elapsed time since the previous event to each line in parentheses — \"2023-12-01T10:15:35+00:00 WARN cache miss  (+5.123s)\". On by default; the first event reads \"(start)\", and a step that goes backwards in time is signed with a minus. This is how the slow step in a boot or request trace becomes visible. Turn it off when the output is going into another parser." },
                    "delta_format": { "type": "string", "enum": ["auto", "seconds", "milliseconds", "hms"], "default": "auto", "description": "How an elapsed time is rendered. \"auto\" (default) picks the readable unit per gap: 850ms, 5.123s, 1m45s, 2h03m10s, 1d04h30m. \"seconds\" always uses seconds (+105.5s) and \"milliseconds\" always milliseconds (+105500ms) — both easy to sort or graph — while \"hms\" uses a clock-style 0:01:45. It also formats the durations in the summary header." },
                    "gap_threshold_seconds": { "type": "number", "default": 0.0, "minimum": 0, "description": "Flag any step that took at least this many seconds by appending \" GAP\" to its delta, and count those steps in the summary header. 0 (default) turns gap flagging off. 60 picks out the minute-plus stalls in a boot log; 0.5 is the more useful setting on a request trace. Fractional values are allowed. The GAP marker needs delta on to be visible; the count is reported whenever summary is on." },
                    "unmatched": { "type": "string", "enum": ["keep", "drop", "mark"], "default": "keep", "description": "What happens to lines with no detectable timestamp — stack-trace frames, wrapped payloads, banners. \"keep\" (default) passes them through untouched, in place under the event they followed. \"drop\" removes them, leaving exactly one line per detected event. \"mark\" keeps them and appends \"  (no timestamp)\" so it is obvious which lines the detector did not read." },
                    "summary": { "type": "boolean", "default": false, "description": "Prepend a header of \"#\" comment lines: how many lines were read and how many carried a timestamp, the mix of source formats detected with a count each, the span from first to last event, the largest gap and the input line it falls on, the number of gaps at or above gap_threshold_seconds, and the output format and zone. Off by default. Every header line starts with \"#\", so the result is still pasteable into a log viewer." }
                },
                "required": ["log"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
