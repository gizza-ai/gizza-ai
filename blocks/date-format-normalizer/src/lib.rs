//! gizza-ai/date-format-normalizer — chat skill block on the shared tool abstraction.
//! Finds every date string in a block of text and rewrites them all into one
//! chosen format. Chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_output_format")]
    output_format: String,
    #[serde(default)]
    custom_format: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default = "default_month_style")]
    month_style: String,
    #[serde(default = "default_year_style")]
    year_style: String,
    #[serde(default = "default_true")]
    leading_zeros: bool,
    #[serde(default = "default_input_order")]
    input_order: String,
    #[serde(default = "default_pivot")]
    two_digit_year_pivot: f64,
    #[serde(default = "default_true")]
    keep_time: bool,
    #[serde(default = "default_time_style")]
    time_style: String,
    #[serde(default = "default_timezone")]
    output_timezone: String,
    #[serde(default)]
    detect_timestamps: bool,
    #[serde(default = "default_output_mode")]
    output_mode: String,
}

fn default_output_format() -> String {
    "iso".to_string()
}
fn default_separator() -> String {
    "dash".to_string()
}
fn default_month_style() -> String {
    "full".to_string()
}
fn default_year_style() -> String {
    "four".to_string()
}
fn default_input_order() -> String {
    "auto".to_string()
}
fn default_time_style() -> String {
    "24h".to_string()
}
fn default_timezone() -> String {
    "source".to_string()
}
fn default_output_mode() -> String {
    "text".to_string()
}
fn default_true() -> bool {
    true
}
fn default_pivot() -> f64 {
    68.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text whose dates you want rewritten — prose, notes, a pasted table, an exported record. Every date is detected where it sits and replaced in place, so the words around it are untouched. One paste can mix formats: ISO 8601 (2024-01-05, 2024-01-05T14:30:00Z), slash/dot/dash numbers (01/05/2024, 5.1.2024, 5-1-24), year-first numbers (2024/01/05), and month names with or without a weekday and an ordinal (January 5, 2024; Jan. 5th, 2024; 5 Jan 2024; Friday, 5 January 2024 14:30 +0100). A clock time written next to a date is picked up with it, including \"at 2:30 PM\" and a trailing Z or +01:00. Strings that only look like dates (2024-02-30, 13/13/2024, a version number) are left exactly as written. Up to 1000000 bytes per run."),
        )
        .param(
            Param::enumv(
                "output_format",
                [
                    "iso",
                    "ymd",
                    "dmy",
                    "mdy",
                    "month_day_year",
                    "day_month_year",
                    "rfc2822",
                    "unix_seconds",
                    "unix_millis",
                    "custom",
                ],
            )
            .default("iso")
            .describe("The one format every detected date is rewritten into. \"iso\" (default) is ISO 8601 — 2024-01-05, or 2024-01-05T14:30:00Z when a time came with it — and ignores the separator, year_style and time_style knobs because ISO fixes them. \"ymd\", \"dmy\" and \"mdy\" are numeric in that field order (2024-01-05, 05-01-2024, 01-05-2024) and honour separator, year_style and leading_zeros. \"month_day_year\" is January 5, 2024 and \"day_month_year\" is 5 January 2024, both following month_style. \"rfc2822\" is the email/HTTP date Fri, 5 Jan 2024 14:30:00 +0000. \"unix_seconds\" (1704412800) and \"unix_millis\" are bare numbers. \"custom\" renders the chrono/strftime pattern you pass in custom_format."),
        )
        .param(
            Param::string("custom_format")
                .default("")
                .describe("The strftime pattern used when output_format is \"custom\" — for example \"%d.%m.%Y\" (05.01.2024), \"%B %-d, %Y\" (January 5, 2024), \"%Y%m%d\" (20240105) or \"%Y-%m-%dT%H:%M:%S%z\". Useful fields: %Y four-digit year, %y two-digit year, %m month, %d day, %-m and %-d drop the leading zero, %B full month name, %b short month name, %A weekday, %H:%M:%S 24-hour clock, %I:%M %p 12-hour clock, %j day of the year, %z the UTC offset. Ignored unless output_format is \"custom\"; an invalid pattern is reported rather than silently half-rendered."),
        )
        .param(
            Param::enumv("separator", ["dash", "slash", "dot", "none", "space"])
                .default("dash")
                .describe("The character between the numeric fields of the ymd, dmy and mdy formats. \"dash\" (default) gives 05-01-2024, \"slash\" 05/01/2024, \"dot\" 05.01.2024 (the usual European written form), \"space\" 05 01 2024, and \"none\" 05012024 — which with ymd and leading_zeros is the 20240105 stamp filenames and folder names sort correctly by. The iso, month-name, rfc2822, unix and custom formats set their own punctuation and ignore this."),
        )
        .param(
            Param::enumv("month_style", ["full", "short"])
                .default("full")
                .describe("How the month is spelled in the month_day_year and day_month_year formats. \"full\" (default) writes it out — January 5, 2024. \"short\" uses the three-letter abbreviation — Jan 5, 2024 — which is what fits in a narrow column or a chart axis. The numeric formats are unaffected."),
        )
        .param(
            Param::enumv("year_style", ["four", "two"])
                .default("four")
                .describe("Whether the year is written in full or shortened. \"four\" (default) writes 2024. \"two\" writes 24, matching the compact 05/01/24 style still used on forms and labels. It applies to the ymd, dmy, mdy and month-name formats; iso, rfc2822, unix and custom always write the year their own way. Round-tripping through \"two\" loses the century, so keep \"four\" for anything a machine will read back."),
        )
        .param(
            Param::boolean("leading_zeros")
                .default(true)
                .describe("Pad single-digit days and months with a zero in the numeric formats: 05/01/2024 with it on (the default), 5/1/2024 with it off. Turn it off for the way people write dates by hand and for spreadsheet cells that should not be read as text. Month-name formats never pad the day (January 5, 2024, not January 05, 2024), and iso, rfc2822 and unix always pad."),
        )
        .param(
            Param::enumv("input_order", ["auto", "day_first", "month_first"])
                .default("auto")
                .describe("How to read a numeric date whose first two fields are both 12 or less — the 03/04/2024 problem, which is 3 April in most of the world and 4 March in the US. \"auto\" (default) reads the whole text first and lets the dates that can only be one thing decide for the rest: one 15/04/2024 in the paste settles it as day-first, one 04/15/2024 as month-first. With no such date, or with the text contradicting itself, it falls back to month-first and says so in the report output mode. \"day_first\" and \"month_first\" skip the inference and force the reading — use them when you know where the data came from. Dates that can only be read one way (a field above 12) always keep that reading."),
        )
        .param(
            Param::integer("two_digit_year_pivot")
                .default(68)
                .min(0.0)
                .max(99.0)
                .describe("Which century a two-digit year belongs to. A year at or below this number becomes 20xx and anything above becomes 19xx, so at the default 68 the year 24 is 2024 and 70 is 1970 (the POSIX convention). Raise it to 99 to force every two-digit year into the 2000s, or lower it when the data is full of twentieth-century birthdays. Only affects dates written with a two-digit year, such as 5-1-24."),
        )
        .param(
            Param::boolean("keep_time")
                .default(true)
                .describe("Carry a clock time into the output when the source date had one — 2024-01-05T14:30:00Z stays a date and time. On by default. Turn it off to reduce everything to a bare date, which is what you want when the times are noise or when the values are going into a column that must hold dates only. Dates with no time are unaffected either way."),
        )
        .param(
            Param::enumv("time_style", ["24h", "12h"])
                .default("24h")
                .describe("The clock the time is written on in the ymd, dmy, mdy and month-name formats. \"24h\" (default) gives 14:30; \"12h\" gives 2:30 PM. Seconds are shown only when the source had them. ISO 8601 and RFC 2822 are always 24-hour by definition, and the unix formats have no clock at all, so this does not touch them."),
        )
        .param(
            Param::string("output_timezone")
                .default("source")
                .describe("Where to place the dates that carry an explicit UTC offset — an ISO stamp ending in Z or +01:00, an RFC 2822 date, a detected epoch value. \"source\" (default) leaves each one on the offset it was written with. Pass \"UTC\", an IANA zone name such as \"Europe/Berlin\", \"America/New_York\" or \"Asia/Tokyo\" (daylight saving applied per date from the bundled IANA database), or a fixed offset such as \"+02:00\", \"-0700\" or \"UTC+5:30\" to move them, which can shift the calendar day itself. Dates written without any zone are left exactly where they are — there is nothing to convert them from."),
        )
        .param(
            Param::boolean("detect_timestamps")
                .default(false)
                .describe("Also treat bare 10-digit and 13-digit numbers as unix timestamps in seconds and milliseconds, so 1704465000 becomes 2024-01-05T14:30:00Z. Off by default on purpose: most long numbers in real text are order ids, phone numbers or account references, not dates. When on, only values between 1973 and 2100 are accepted, which filters out the obvious non-dates. Turn it on for log exports and API payloads where epoch values are genuinely mixed into the text."),
        )
        .param(
            Param::enumv("output_mode", ["text", "list", "report"])
                .default("text")
                .describe("What comes back. \"text\" (default) is the original text with every date rewritten in place and nothing else changed — paste it straight back where it came from. \"list\" is just the normalized dates, one per line, in the order they appear, ready for a spreadsheet column. \"report\" is an audit trail: a \"#\" header with how many dates were found, the mix of source forms detected and which day/month order was chosen and why, then one tab-separated line per date giving its line and column, the original string, the rewritten value, and whether it was ambiguous."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DateFormatNormalizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/date-format-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rewrite every date in a block of text into one chosen format, resolving day/month order from the text itself",
    skill(
        description = "Find every date string in a block of text and rewrite them all into one format, leaving the surrounding words untouched. Paste the text into `text`. Detection runs per occurrence, so one paste can mix ISO 8601 (2024-01-05, 2024-01-05T14:30:00Z), slash/dot/dash numbers (01/05/2024, 5.1.2024, 5-1-24), year-first numbers (2024/01/05) and month names with optional weekday and ordinal (January 5, 2024; Jan. 5th, 2024; 5 Jan 2024; Friday, 5 January 2024 14:30 +0100); a clock time written beside a date travels with it, including \"at 2:30 PM\" and a trailing Z or +01:00. `output_format` picks iso (default), ymd, dmy, mdy, month_day_year, day_month_year, rfc2822, unix_seconds, unix_millis or custom (a strftime pattern in `custom_format`), shaped further by `separator`, `month_style`, `year_style`, `leading_zeros`, `keep_time` and `time_style`. The 03/04/2024 ambiguity is settled by `input_order`: at the default \"auto\" the whole text is read first and any date that can only be one thing (a field above 12) decides the reading for the rest, falling back to month-first when nothing settles it; \"day_first\" and \"month_first\" force it. `two_digit_year_pivot` decides the century for years like 24 or 70. `output_timezone` moves the dates that carry an explicit offset into UTC, an IANA zone or a fixed offset — zone-less dates are left alone. `detect_timestamps` (off by default) additionally reads bare 10- and 13-digit epoch numbers as dates. `output_mode` returns the rewritten text (default), just the normalized values one per line, or a report with line/column, original, result and an ambiguity flag. Strings that only look like dates (2024-02-30, 13/13/2024, version numbers) are never guessed at. No clock and no I/O — the same input always produces the same output. Up to 1000000 bytes per run.",
        parameters = schema_json()
    ),
)]
impl DateFormatNormalizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "date-format-normalizer", |a: Args| {
            gizza_ai_date_format_normalizer_core::run(
                &a.text,
                &a.output_format,
                &a.custom_format,
                &a.separator,
                &a.month_style,
                &a.year_style,
                a.leading_zeros,
                &a.input_order,
                a.two_digit_year_pivot.round() as i64,
                a.keep_time,
                &a.time_style,
                &a.output_timezone,
                a.detect_timestamps,
                &a.output_mode,
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
                    "text": { "type": "string", "description": "The text whose dates you want rewritten — prose, notes, a pasted table, an exported record. Every date is detected where it sits and replaced in place, so the words around it are untouched. One paste can mix formats: ISO 8601 (2024-01-05, 2024-01-05T14:30:00Z), slash/dot/dash numbers (01/05/2024, 5.1.2024, 5-1-24), year-first numbers (2024/01/05), and month names with or without a weekday and an ordinal (January 5, 2024; Jan. 5th, 2024; 5 Jan 2024; Friday, 5 January 2024 14:30 +0100). A clock time written next to a date is picked up with it, including \"at 2:30 PM\" and a trailing Z or +01:00. Strings that only look like dates (2024-02-30, 13/13/2024, a version number) are left exactly as written. Up to 1000000 bytes per run." },
                    "output_format": { "type": "string", "enum": ["iso", "ymd", "dmy", "mdy", "month_day_year", "day_month_year", "rfc2822", "unix_seconds", "unix_millis", "custom"], "default": "iso", "description": "The one format every detected date is rewritten into. \"iso\" (default) is ISO 8601 — 2024-01-05, or 2024-01-05T14:30:00Z when a time came with it — and ignores the separator, year_style and time_style knobs because ISO fixes them. \"ymd\", \"dmy\" and \"mdy\" are numeric in that field order (2024-01-05, 05-01-2024, 01-05-2024) and honour separator, year_style and leading_zeros. \"month_day_year\" is January 5, 2024 and \"day_month_year\" is 5 January 2024, both following month_style. \"rfc2822\" is the email/HTTP date Fri, 5 Jan 2024 14:30:00 +0000. \"unix_seconds\" (1704412800) and \"unix_millis\" are bare numbers. \"custom\" renders the chrono/strftime pattern you pass in custom_format." },
                    "custom_format": { "type": "string", "default": "", "description": "The strftime pattern used when output_format is \"custom\" — for example \"%d.%m.%Y\" (05.01.2024), \"%B %-d, %Y\" (January 5, 2024), \"%Y%m%d\" (20240105) or \"%Y-%m-%dT%H:%M:%S%z\". Useful fields: %Y four-digit year, %y two-digit year, %m month, %d day, %-m and %-d drop the leading zero, %B full month name, %b short month name, %A weekday, %H:%M:%S 24-hour clock, %I:%M %p 12-hour clock, %j day of the year, %z the UTC offset. Ignored unless output_format is \"custom\"; an invalid pattern is reported rather than silently half-rendered." },
                    "separator": { "type": "string", "enum": ["dash", "slash", "dot", "none", "space"], "default": "dash", "description": "The character between the numeric fields of the ymd, dmy and mdy formats. \"dash\" (default) gives 05-01-2024, \"slash\" 05/01/2024, \"dot\" 05.01.2024 (the usual European written form), \"space\" 05 01 2024, and \"none\" 05012024 — which with ymd and leading_zeros is the 20240105 stamp filenames and folder names sort correctly by. The iso, month-name, rfc2822, unix and custom formats set their own punctuation and ignore this." },
                    "month_style": { "type": "string", "enum": ["full", "short"], "default": "full", "description": "How the month is spelled in the month_day_year and day_month_year formats. \"full\" (default) writes it out — January 5, 2024. \"short\" uses the three-letter abbreviation — Jan 5, 2024 — which is what fits in a narrow column or a chart axis. The numeric formats are unaffected." },
                    "year_style": { "type": "string", "enum": ["four", "two"], "default": "four", "description": "Whether the year is written in full or shortened. \"four\" (default) writes 2024. \"two\" writes 24, matching the compact 05/01/24 style still used on forms and labels. It applies to the ymd, dmy, mdy and month-name formats; iso, rfc2822, unix and custom always write the year their own way. Round-tripping through \"two\" loses the century, so keep \"four\" for anything a machine will read back." },
                    "leading_zeros": { "type": "boolean", "default": true, "description": "Pad single-digit days and months with a zero in the numeric formats: 05/01/2024 with it on (the default), 5/1/2024 with it off. Turn it off for the way people write dates by hand and for spreadsheet cells that should not be read as text. Month-name formats never pad the day (January 5, 2024, not January 05, 2024), and iso, rfc2822 and unix always pad." },
                    "input_order": { "type": "string", "enum": ["auto", "day_first", "month_first"], "default": "auto", "description": "How to read a numeric date whose first two fields are both 12 or less — the 03/04/2024 problem, which is 3 April in most of the world and 4 March in the US. \"auto\" (default) reads the whole text first and lets the dates that can only be one thing decide for the rest: one 15/04/2024 in the paste settles it as day-first, one 04/15/2024 as month-first. With no such date, or with the text contradicting itself, it falls back to month-first and says so in the report output mode. \"day_first\" and \"month_first\" skip the inference and force the reading — use them when you know where the data came from. Dates that can only be read one way (a field above 12) always keep that reading." },
                    "two_digit_year_pivot": { "type": "integer", "default": 68, "minimum": 0, "maximum": 99, "description": "Which century a two-digit year belongs to. A year at or below this number becomes 20xx and anything above becomes 19xx, so at the default 68 the year 24 is 2024 and 70 is 1970 (the POSIX convention). Raise it to 99 to force every two-digit year into the 2000s, or lower it when the data is full of twentieth-century birthdays. Only affects dates written with a two-digit year, such as 5-1-24." },
                    "keep_time": { "type": "boolean", "default": true, "description": "Carry a clock time into the output when the source date had one — 2024-01-05T14:30:00Z stays a date and time. On by default. Turn it off to reduce everything to a bare date, which is what you want when the times are noise or when the values are going into a column that must hold dates only. Dates with no time are unaffected either way." },
                    "time_style": { "type": "string", "enum": ["24h", "12h"], "default": "24h", "description": "The clock the time is written on in the ymd, dmy, mdy and month-name formats. \"24h\" (default) gives 14:30; \"12h\" gives 2:30 PM. Seconds are shown only when the source had them. ISO 8601 and RFC 2822 are always 24-hour by definition, and the unix formats have no clock at all, so this does not touch them." },
                    "output_timezone": { "type": "string", "default": "source", "description": "Where to place the dates that carry an explicit UTC offset — an ISO stamp ending in Z or +01:00, an RFC 2822 date, a detected epoch value. \"source\" (default) leaves each one on the offset it was written with. Pass \"UTC\", an IANA zone name such as \"Europe/Berlin\", \"America/New_York\" or \"Asia/Tokyo\" (daylight saving applied per date from the bundled IANA database), or a fixed offset such as \"+02:00\", \"-0700\" or \"UTC+5:30\" to move them, which can shift the calendar day itself. Dates written without any zone are left exactly where they are — there is nothing to convert them from." },
                    "detect_timestamps": { "type": "boolean", "default": false, "description": "Also treat bare 10-digit and 13-digit numbers as unix timestamps in seconds and milliseconds, so 1704465000 becomes 2024-01-05T14:30:00Z. Off by default on purpose: most long numbers in real text are order ids, phone numbers or account references, not dates. When on, only values between 1973 and 2100 are accepted, which filters out the obvious non-dates. Turn it on for log exports and API payloads where epoch values are genuinely mixed into the text." },
                    "output_mode": { "type": "string", "enum": ["text", "list", "report"], "default": "text", "description": "What comes back. \"text\" (default) is the original text with every date rewritten in place and nothing else changed — paste it straight back where it came from. \"list\" is just the normalized dates, one per line, in the order they appear, ready for a spreadsheet column. \"report\" is an audit trail: a \"#\" header with how many dates were found, the mix of source forms detected and which day/month order was chosen and why, then one tab-separated line per date giving its line and column, the original string, the rewritten value, and whether it was ambiguous." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
