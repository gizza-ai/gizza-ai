//! gizza-ai/irc-log-parser — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    log: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    date: String,
    #[serde(default = "default_time_format")]
    time_format: String,
    #[serde(default = "default_include")]
    include: String,
    #[serde(default)]
    nicks: String,
    #[serde(default)]
    channel: String,
    #[serde(default = "default_true")]
    strip_formatting: bool,
    #[serde(default)]
    include_raw: bool,
    #[serde(default)]
    limit: i64,
}

fn default_format() -> String {
    "auto".into()
}
fn default_output() -> String {
    "timeline".into()
}
fn default_time_format() -> String {
    "iso".into()
}
fn default_include() -> String {
    "all".into()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and the CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("log")
                .required()
                .describe("The raw IRC log text. Paste the lines straight out of the client, e.g. '21:07 <alice> hey' (irssi), '2024-01-05 21:07:33<TAB>alice<TAB>hey' (WeeChat), '[21:07:33] *** Joins: alice (~a@host)' (ZNC/mIRC) or 'Jan 05 21:07:33 <alice> hey' (HexChat). Maximum 5000000 bytes."),
        )
        .param(
            Param::enumv("format", ["auto", "weechat", "irssi", "bracket", "hexchat", "iso", "plain"])
                .default("auto")
                .describe("Timestamp grammar of the log: auto (detect it from the first 200 lines), weechat (date TAB nick TAB text), irssi (bare '21:07' or '21:07:33'), bracket ('[21:07:33]' — mIRC, ZNC, EnergyMech, Textual), hexchat ('Jan 05 21:07:33'), iso ('2024-01-05 21:07:33'), or plain (no timestamps). Event wording from every client is understood in all modes. Default auto."),
        )
        .param(
            Param::enumv("output", ["timeline", "json", "ndjson", "csv", "markdown"])
                .default("timeline")
                .describe("Result shape: timeline (readable one line per event), json (array of records), ndjson (one compact record per line for jq or streaming), csv (spreadsheet columns line,time,type,nick,host,channel,arg,text), or markdown (a table). Default timeline."),
        )
        .param(
            Param::string("date")
                .default("")
                .describe("Base calendar date as YYYY-MM-DD for logs that only record a time of day, e.g. '2024-01-05'. Irssi '--- Log opened' and '--- Day changed' markers in the log override it from that point on. Empty by default, which leaves such records with a time only."),
        )
        .param(
            Param::enumv("time_format", ["iso", "24h", "12h", "original", "none"])
                .default("iso")
                .describe("How each timestamp is written out: iso ('2024-01-05T21:07:33', falling back to '21:07:33' when no date is known), 24h ('21:07:33'), 12h ('9:07:33 PM'), original (exactly as it appeared in the log), or none (drop timestamps). Default iso."),
        )
        .param(
            Param::enumv("include", ["all", "messages", "events"])
                .default("all")
                .describe("Which lines to keep: all, messages (message, action and notice lines only), or events (join, part, quit, kick, nick change, mode and topic only). Default all."),
        )
        .param(
            Param::string("nicks")
                .default("")
                .describe("Comma-separated nicks to keep, matched case-insensitively — e.g. 'alice, bob*'. A trailing * matches by prefix, so 'bob*' keeps bob and bobby. Lines with no nick (server notices, log markers) are dropped when this is set. Empty by default, which keeps everyone."),
        )
        .param(
            Param::string("channel")
                .default("")
                .describe("Channel name to record on lines that do not name one themselves, e.g. '#gizza'. Must start with # & or !. Empty by default, which leaves the channel column blank on those lines."),
        )
        .param(
            Param::boolean("strip_formatting")
                .default(true)
                .describe("When true (default), remove mIRC formatting codes (bold, italic, underline, reverse, reset and ^C colour pairs) and ANSI escape sequences from the text. Turn it off to keep the control characters byte-for-byte."),
        )
        .param(
            Param::boolean("include_raw")
                .default(false)
                .describe("Add the original untouched log line as a 'raw' field (json, ndjson) or column (csv). Off by default to keep the output compact."),
        )
        .param(
            Param::integer("limit")
                .min(0.0)
                .max(200000.0)
                .default(0)
                .describe("Maximum number of records to return, applied after the include and nicks filters. 0 (the default) means no limit; the ceiling is 200000."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/irc-log-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse raw IRC, WeeChat or irssi logs into a structured, readable timeline",
    skill(
        description = "Turn a raw IRC client log into a structured, readable timeline or a machine-readable export. It reads the timestamp grammars of the common clients — WeeChat (date TAB nick TAB text), irssi (bare '21:07'), mIRC/ZNC/EnergyMech ('[21:07:33]'), HexChat/XChat ('Jan 05 21:07:33'), plain ISO datetimes, and logs with no timestamps at all — auto-detecting by default, and understands each client's event wording regardless of which grammar was used: '-!- alice [~a@host] has joined #chan', '*** Joins: alice (~a@host)', '* Parts: alice (~a@host) (Leaving)', 'alice was kicked from #chan by bob', 'alice is now known as bobby', 'mode/#chan [+o bob] by alice' and the topic phrasings. Every line becomes a typed record — message, action, notice, join, part, quit, kick, nick, mode, topic, meta or unknown — with the same eight fields: line, time, type, nick, host, channel, arg (the new nick, the kicker, or the mode string) and text. Render them as a readable timeline, a JSON array, NDJSON, CSV or a Markdown table; normalize timestamps to ISO, 24-hour, 12-hour, the original text or nothing; supply a base date for time-only logs (irssi day-change markers roll it forward automatically); attach a channel name to lines that lack one; keep only messages or only events; filter to particular nicks with a trailing * for prefix matching; strip mIRC colour and formatting codes; optionally keep the original raw line; and cap the number of records. Fully local and deterministic — no AI model, no network, nothing uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "irc-log-parser", |a: Args| {
            gizza_ai_irc_log_parser_core::run(
                &a.log,
                &a.format,
                &a.output,
                &a.date,
                &a.time_format,
                &a.include,
                &a.nicks,
                &a.channel,
                a.strip_formatting,
                a.include_raw,
                a.limit,
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
                    "log": { "type": "string", "description": "The raw IRC log text. Paste the lines straight out of the client, e.g. '21:07 <alice> hey' (irssi), '2024-01-05 21:07:33<TAB>alice<TAB>hey' (WeeChat), '[21:07:33] *** Joins: alice (~a@host)' (ZNC/mIRC) or 'Jan 05 21:07:33 <alice> hey' (HexChat). Maximum 5000000 bytes." },
                    "format": { "type": "string", "enum": ["auto", "weechat", "irssi", "bracket", "hexchat", "iso", "plain"], "default": "auto", "description": "Timestamp grammar of the log: auto (detect it from the first 200 lines), weechat (date TAB nick TAB text), irssi (bare '21:07' or '21:07:33'), bracket ('[21:07:33]' — mIRC, ZNC, EnergyMech, Textual), hexchat ('Jan 05 21:07:33'), iso ('2024-01-05 21:07:33'), or plain (no timestamps). Event wording from every client is understood in all modes. Default auto." },
                    "output": { "type": "string", "enum": ["timeline", "json", "ndjson", "csv", "markdown"], "default": "timeline", "description": "Result shape: timeline (readable one line per event), json (array of records), ndjson (one compact record per line for jq or streaming), csv (spreadsheet columns line,time,type,nick,host,channel,arg,text), or markdown (a table). Default timeline." },
                    "date": { "type": "string", "default": "", "description": "Base calendar date as YYYY-MM-DD for logs that only record a time of day, e.g. '2024-01-05'. Irssi '--- Log opened' and '--- Day changed' markers in the log override it from that point on. Empty by default, which leaves such records with a time only." },
                    "time_format": { "type": "string", "enum": ["iso", "24h", "12h", "original", "none"], "default": "iso", "description": "How each timestamp is written out: iso ('2024-01-05T21:07:33', falling back to '21:07:33' when no date is known), 24h ('21:07:33'), 12h ('9:07:33 PM'), original (exactly as it appeared in the log), or none (drop timestamps). Default iso." },
                    "include": { "type": "string", "enum": ["all", "messages", "events"], "default": "all", "description": "Which lines to keep: all, messages (message, action and notice lines only), or events (join, part, quit, kick, nick change, mode and topic only). Default all." },
                    "nicks": { "type": "string", "default": "", "description": "Comma-separated nicks to keep, matched case-insensitively — e.g. 'alice, bob*'. A trailing * matches by prefix, so 'bob*' keeps bob and bobby. Lines with no nick (server notices, log markers) are dropped when this is set. Empty by default, which keeps everyone." },
                    "channel": { "type": "string", "default": "", "description": "Channel name to record on lines that do not name one themselves, e.g. '#gizza'. Must start with # & or !. Empty by default, which leaves the channel column blank on those lines." },
                    "strip_formatting": { "type": "boolean", "default": true, "description": "When true (default), remove mIRC formatting codes (bold, italic, underline, reverse, reset and ^C colour pairs) and ANSI escape sequences from the text. Turn it off to keep the control characters byte-for-byte." },
                    "include_raw": { "type": "boolean", "default": false, "description": "Add the original untouched log line as a 'raw' field (json, ndjson) or column (csv). Off by default to keep the output compact." },
                    "limit": { "type": "integer", "minimum": 0, "maximum": 200000, "default": 0, "description": "Maximum number of records to return, applied after the include and nicks filters. 0 (the default) means no limit; the ceiling is 200000." }
                },
                "required": ["log"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
