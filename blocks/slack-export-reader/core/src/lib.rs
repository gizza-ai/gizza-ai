//! gizza-ai/slack-export-reader core — turn a Slack workspace export (a ZIP of
//! `users.json`, `channels.json` and per-channel `YYYY-MM-DD.json` message files)
//! into a readable Markdown or HTML transcript.
//!
//! Pure-Rust (`zip` deflate-only + `serde_json`); no wafer/wasm-bindgen deps, so
//! it runs on every backend including the chat Service Worker.
//!
//! What it does per message: resolves the author id to a display name (from
//! `users.json`), turns the raw `ts` epoch into a `YYYY-MM-DD HH:MM:SS UTC`
//! stamp, and rewrites Slack's `<...>` markup — user/channel mentions
//! (`<@U123>`, `<#C123|general>`), special commands (`<!here>`) and links
//! (`<https://x|label>`) — into readable Markdown or HTML. Optional `channel`
//! and `date` filters narrow the transcript.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use serde::Serialize;
use serde_json::Value;

/// Output transcript format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Html,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" | "" => Ok(Format::Markdown),
            "html" | "htm" => Ok(Format::Html),
            other => Err(format!("unknown format '{other}' (use markdown or html)")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Format::Markdown => "markdown",
            Format::Html => "html",
        }
    }
}

/// Rendering options.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub format: Format,
    /// When set, only this channel is rendered (case-insensitive, a leading `#`
    /// is ignored).
    pub channel: Option<String>,
    /// When set, only messages from this `YYYY-MM-DD` day file are rendered.
    pub date: Option<String>,
}

impl Default for Format {
    fn default() -> Self {
        Format::Markdown
    }
}

/// The rendered transcript plus a few counts for the caller/LLM.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Transcript {
    pub format: String,
    /// Number of channels included in the transcript.
    pub channels: usize,
    /// Number of messages rendered.
    pub messages: usize,
    pub content: String,
}

/// A single day file's messages, keyed for stable ordering.
type DayFiles = BTreeMap<String, Vec<Value>>;

/// Render the Slack export `zip_bytes` into a transcript per `opts`.
pub fn render(zip_bytes: &[u8], opts: &Options) -> Result<Transcript, String> {
    let reader = Cursor::new(zip_bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| format!("not a valid zip archive: {e}"))?;

    // Slack nests everything under an optional top-level export folder; detect
    // and strip it so paths line up whether or not the zip was re-wrapped.
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();
    let prefix = common_prefix(&names);

    // Read the two index files and the per-channel day files.
    let mut users_raw: Option<Vec<u8>> = None;
    let mut channels_raw: Option<Vec<u8>> = None;
    // channel name -> (date -> messages)
    let mut channels: BTreeMap<String, DayFiles> = BTreeMap::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("failed to read zip entry {i}: {e}"))?;
        if file.is_dir() {
            continue;
        }
        let full = file.name().to_string();
        let rel = full.strip_prefix(&prefix).unwrap_or(&full);
        let parts: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();

        let mut read_bytes = || -> Result<Vec<u8>, String> {
            let mut buf = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buf)
                .map_err(|e| format!("failed to read '{full}': {e}"))?;
            Ok(buf)
        };

        match parts.as_slice() {
            [name] if *name == "users.json" => users_raw = Some(read_bytes()?),
            [name] if *name == "channels.json" => channels_raw = Some(read_bytes()?),
            [channel, day] if day.ends_with(".json") && is_date_file(day) => {
                let date = day.trim_end_matches(".json").to_string();
                let msgs: Vec<Value> = serde_json::from_slice(&read_bytes()?)
                    .map_err(|e| format!("'{full}' is not a valid Slack message file: {e}"))?;
                channels
                    .entry((*channel).to_string())
                    .or_default()
                    .insert(date, msgs);
            }
            // Ignore other bookkeeping files (integration_logs.json, etc.).
            _ => {}
        }
    }

    if users_raw.is_none() && channels_raw.is_none() && channels.is_empty() {
        return Err(
            "this ZIP does not look like a Slack export (no users.json/channels.json or \
             per-channel YYYY-MM-DD.json message files found)"
                .to_string(),
        );
    }

    let users = users_raw
        .as_deref()
        .map(build_user_map)
        .transpose()?
        .unwrap_or_default();
    let chan_names = channels_raw
        .as_deref()
        .map(build_channel_map)
        .transpose()?
        .unwrap_or_default();

    // Apply the optional channel filter.
    let want_channel = opts.channel.as_deref().map(normalize_channel);
    let want_date = opts.date.as_deref().map(|d| d.trim().to_string());

    let mut out = String::new();
    let mut total_channels = 0usize;
    let mut total_messages = 0usize;

    if opts.format == Format::Html {
        out.push_str(HTML_HEAD);
    }

    for (channel, days) in &channels {
        if let Some(want) = &want_channel {
            if &normalize_channel(channel) != want {
                continue;
            }
        }

        let mut channel_body = String::new();
        let mut channel_msgs = 0usize;
        for (date, msgs) in days {
            if let Some(want) = &want_date {
                if date != want {
                    continue;
                }
            }
            for msg in msgs {
                if let Some(rendered) =
                    render_message(msg, date, &users, &chan_names, opts.format)
                {
                    channel_body.push_str(&rendered);
                    channel_msgs += 1;
                }
            }
        }

        if channel_msgs == 0 {
            continue;
        }
        total_channels += 1;
        total_messages += channel_msgs;

        match opts.format {
            Format::Markdown => {
                out.push_str(&format!("## #{channel}\n\n{channel_body}"));
            }
            Format::Html => {
                out.push_str(&format!(
                    "<section class=\"channel\">\n<h2>#{}</h2>\n{}</section>\n",
                    escape_html(channel),
                    channel_body
                ));
            }
        }
    }

    if opts.format == Format::Html {
        out.push_str(HTML_FOOT);
    }

    if total_messages == 0 {
        let mut hint = String::from("no messages matched");
        if let Some(c) = &opts.channel {
            hint.push_str(&format!(" channel '{c}'"));
        }
        if let Some(d) = &opts.date {
            hint.push_str(&format!(" date '{d}'"));
        }
        return Err(hint);
    }

    Ok(Transcript {
        format: opts.format.as_str().to_string(),
        channels: total_channels,
        messages: total_messages,
        content: out.trim_end().to_string(),
    })
}

const HTML_HEAD: &str = "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
<title>Slack transcript</title>\n<style>\nbody{font:15px/1.5 -apple-system,BlinkMacSystemFont,\
\"Segoe UI\",Roboto,sans-serif;max-width:760px;margin:2rem auto;padding:0 1rem;color:#1d1c1d}\n\
h2{border-bottom:1px solid #e0e0e0;padding-bottom:.3rem;margin-top:2rem}\n.msg{margin:.75rem 0}\n\
.author{font-weight:600}\n.ts{color:#616061;font-size:.8em;margin-left:.4rem}\n\
.text{white-space:pre-wrap;margin:.15rem 0 0}\na{color:#1264a3}\n</style>\n</head>\n<body>\n";
const HTML_FOOT: &str = "</body>\n</html>\n";

/// Render one message; returns `None` for entries with no displayable text.
fn render_message(
    msg: &Value,
    date: &str,
    users: &BTreeMap<String, String>,
    channels: &BTreeMap<String, String>,
    format: Format,
) -> Option<String> {
    // Only "message" entries (or entries with text) carry transcript content.
    if let Some(t) = msg.get("type").and_then(Value::as_str) {
        if t != "message" {
            return None;
        }
    }
    let raw_text = msg.get("text").and_then(Value::as_str).unwrap_or("");
    if raw_text.trim().is_empty() {
        return None;
    }

    let author = author_name(msg, users);
    let ts = message_time(msg, date);
    let body = format_text(raw_text, users, channels, format);

    Some(match format {
        Format::Markdown => format!("**{author}**  _{ts}_\n{body}\n\n"),
        Format::Html => format!(
            "<div class=\"msg\"><span class=\"author\">{}</span><span class=\"ts\">{}</span>\
             <div class=\"text\">{}</div></div>\n",
            escape_html(&author),
            escape_html(&ts),
            body
        ),
    })
}

/// Best display name for a message's author.
fn author_name(msg: &Value, users: &BTreeMap<String, String>) -> String {
    if let Some(uid) = msg.get("user").and_then(Value::as_str) {
        if let Some(name) = users.get(uid) {
            return name.clone();
        }
        // Fall back to an inline user_profile if the id wasn't in users.json.
        if let Some(p) = msg.get("user_profile") {
            if let Some(n) = profile_name(p) {
                return n;
            }
        }
        return uid.to_string();
    }
    if let Some(name) = msg.get("username").and_then(Value::as_str) {
        return name.to_string();
    }
    if let Some(bot) = msg.get("bot_id").and_then(Value::as_str) {
        return format!("bot:{bot}");
    }
    "unknown".to_string()
}

/// `YYYY-MM-DD HH:MM:SS UTC` from the message `ts`, falling back to the day file
/// name when `ts` is missing/unparseable.
fn message_time(msg: &Value, date: &str) -> String {
    let ts = msg
        .get("ts")
        .and_then(Value::as_str)
        .and_then(|s| s.split('.').next())
        .and_then(|s| s.parse::<i64>().ok());
    match ts {
        Some(secs) => format_epoch(secs),
        None => date.to_string(),
    }
}

/// Format a UTC unix timestamp as `YYYY-MM-DD HH:MM:SS UTC` (no chrono dep).
pub fn format_epoch(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02} UTC")
}

/// Days since 1970-01-01 -> (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Rewrite Slack markup into readable Markdown/HTML. Slack HTML-encodes literal
/// `&<>` in text, so `<...>` runs are always control tokens.
fn format_text(
    raw: &str,
    users: &BTreeMap<String, String>,
    channels: &BTreeMap<String, String>,
    format: Format,
) -> String {
    let mut out = String::new();
    let bytes = raw.as_bytes();
    let mut i = 0;
    let mut literal = String::new();

    let flush = |literal: &mut String, out: &mut String| {
        if literal.is_empty() {
            return;
        }
        let decoded = unescape_entities(literal);
        match format {
            Format::Markdown => out.push_str(&decoded),
            Format::Html => out.push_str(&escape_html(&decoded)),
        }
        literal.clear();
    };

    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(end) = raw[i + 1..].find('>') {
                let token = &raw[i + 1..i + 1 + end];
                flush(&mut literal, &mut out);
                out.push_str(&render_token(token, users, channels, format));
                i = i + 1 + end + 1;
                continue;
            }
        }
        // Copy this char (handle multi-byte utf-8 via char boundary).
        let ch_len = utf8_len(bytes[i]);
        literal.push_str(&raw[i..i + ch_len]);
        i += ch_len;
    }
    flush(&mut literal, &mut out);
    out
}

/// Render one `<...>` token (mention/channel/command/link) for the format.
fn render_token(
    token: &str,
    users: &BTreeMap<String, String>,
    channels: &BTreeMap<String, String>,
    format: Format,
) -> String {
    let (left, label) = match token.split_once('|') {
        Some((l, r)) => (l, Some(r)),
        None => (token, None),
    };

    let mention = |name: &str| match format {
        Format::Markdown => format!("@{name}"),
        Format::Html => format!("<b>@{}</b>", escape_html(name)),
    };

    if let Some(uid) = left.strip_prefix('@') {
        let name = label
            .map(str::to_string)
            .or_else(|| users.get(uid).cloned())
            .unwrap_or_else(|| uid.to_string());
        return mention(&name);
    }
    if let Some(cid) = left.strip_prefix('#') {
        let name = label
            .map(str::to_string)
            .or_else(|| channels.get(cid).cloned())
            .unwrap_or_else(|| cid.to_string());
        return match format {
            Format::Markdown => format!("#{name}"),
            Format::Html => format!("<b>#{}</b>", escape_html(&name)),
        };
    }
    if let Some(cmd) = left.strip_prefix('!') {
        // <!here>, <!channel>, <!everyone>, <!subteam^ID|@group>, <!date^...>.
        let name = label.unwrap_or_else(|| cmd.split(['^']).next().unwrap_or(cmd));
        return mention(name);
    }

    // Otherwise it's a link: <url> or <url|label>.
    let url = left;
    let text = label.unwrap_or(url);
    match format {
        Format::Markdown => {
            if text == url {
                url.to_string()
            } else {
                format!("[{}]({})", unescape_entities(text), url)
            }
        }
        Format::Html => format!(
            "<a href=\"{}\">{}</a>",
            escape_html(url),
            escape_html(&unescape_entities(text))
        ),
    }
}

/// Build a user id -> display name map from `users.json`.
fn build_user_map(bytes: &[u8]) -> Result<BTreeMap<String, String>, String> {
    let arr: Vec<Value> =
        serde_json::from_slice(bytes).map_err(|e| format!("users.json is not valid JSON: {e}"))?;
    let mut map = BTreeMap::new();
    for u in arr {
        let Some(id) = u.get("id").and_then(Value::as_str) else {
            continue;
        };
        let name = u
            .get("profile")
            .and_then(profile_name)
            .or_else(|| non_empty(u.get("real_name")))
            .or_else(|| non_empty(u.get("name")))
            .unwrap_or_else(|| id.to_string());
        map.insert(id.to_string(), name);
    }
    Ok(map)
}

/// Build a channel id -> name map from `channels.json`.
fn build_channel_map(bytes: &[u8]) -> Result<BTreeMap<String, String>, String> {
    let arr: Vec<Value> = serde_json::from_slice(bytes)
        .map_err(|e| format!("channels.json is not valid JSON: {e}"))?;
    let mut map = BTreeMap::new();
    for c in arr {
        if let (Some(id), Some(name)) = (
            c.get("id").and_then(Value::as_str),
            c.get("name").and_then(Value::as_str),
        ) {
            map.insert(id.to_string(), name.to_string());
        }
    }
    Ok(map)
}

/// Best name from a profile object: display_name -> real_name.
fn profile_name(p: &Value) -> Option<String> {
    non_empty(p.get("display_name")).or_else(|| non_empty(p.get("real_name")))
}

fn non_empty(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn normalize_channel(s: &str) -> String {
    s.trim().trim_start_matches('#').to_ascii_lowercase()
}

fn is_date_file(name: &str) -> bool {
    let stem = name.trim_end_matches(".json");
    let b = stem.as_bytes();
    // YYYY-MM-DD
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..10].iter().all(u8::is_ascii_digit)
}

/// Longest common directory prefix shared by every path (used to strip a single
/// wrapping export folder). Returns "" when paths are already at the root.
fn common_prefix(names: &[String]) -> String {
    // Only strip a prefix if EVERY entry starts with the same first segment.
    let first_seg = |s: &str| s.split('/').next().unwrap_or("").to_string();
    let mut seg: Option<String> = None;
    for n in names {
        // A root-level file (no '/') means there is no single wrapping folder.
        if !n.contains('/') {
            return String::new();
        }
        let s = first_seg(n);
        match &seg {
            None => seg = Some(s),
            Some(existing) if *existing != s => return String::new(),
            _ => {}
        }
    }
    match seg {
        Some(s) if is_wrapping_folder(&s, names) => format!("{s}/"),
        _ => String::new(),
    }
}

/// A wrapping folder is one that itself contains `users.json`/`channels.json` or
/// channel subfolders — not a real channel named like a folder.
fn is_wrapping_folder(seg: &str, names: &[String]) -> bool {
    let p = format!("{seg}/");
    names.iter().any(|n| {
        n == &format!("{p}users.json") || n == &format!("{p}channels.json")
    })
}

fn utf8_len(b: u8) -> usize {
    match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

fn unescape_entities(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for (name, data) in files {
                w.start_file(*name, opts).unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    fn sample_export() -> Vec<u8> {
        let users = br#"[
            {"id":"U1","name":"alice","real_name":"Alice A","profile":{"display_name":"alice","real_name":"Alice A"}},
            {"id":"U2","name":"bob","real_name":"Bob B","profile":{"display_name":"","real_name":"Bob B"}}
        ]"#;
        let channels = br#"[{"id":"C1","name":"general"}]"#;
        // ts 1609459200 = 2021-01-01 00:00:00 UTC
        let day = br#"[
            {"type":"message","user":"U1","ts":"1609459200.000100","text":"Hi <@U2> welcome to <#C1|general>! See <https://gizza.ai|the site> &amp; docs."},
            {"type":"message","user":"U2","ts":"1609459260.000200","text":"Thanks <!here>"},
            {"type":"message","subtype":"channel_join","user":"U2","ts":"1609459201.000000","text":""}
        ]"#;
        make_zip(&[
            ("users.json", users),
            ("channels.json", channels),
            ("general/2021-01-01.json", day),
        ])
    }

    #[test]
    fn renders_markdown_transcript() {
        let zip = sample_export();
        let t = render(&zip, &Options { format: Format::Markdown, ..Default::default() }).unwrap();
        assert_eq!(t.format, "markdown");
        assert_eq!(t.channels, 1);
        assert_eq!(t.messages, 2); // the empty channel_join message is skipped
        assert!(t.content.contains("## #general"));
        assert!(t.content.contains("**alice**"));
        assert!(t.content.contains("2021-01-01 00:00:00 UTC"));
        // mention resolved to a display name, channel ref, link + entity decoded
        assert!(t.content.contains("@Bob B"), "user mention resolved: {}", t.content);
        assert!(t.content.contains("#general"));
        assert!(t.content.contains("[the site](https://gizza.ai)"));
        assert!(t.content.contains("& docs"));
        assert!(t.content.contains("@here"));
    }

    #[test]
    fn renders_html_transcript_with_escaping() {
        let zip = sample_export();
        let t = render(&zip, &Options { format: Format::Html, ..Default::default() }).unwrap();
        assert_eq!(t.format, "html");
        assert!(t.content.starts_with("<!doctype html>"));
        assert!(t.content.contains("<h2>#general</h2>"));
        assert!(t.content.contains("<a href=\"https://gizza.ai\">the site</a>"));
        assert!(t.content.contains("&amp; docs"));
    }

    #[test]
    fn channel_and_date_filters() {
        let zip = sample_export();
        // Matching filters (case-insensitive, leading # tolerated).
        let t = render(
            &zip,
            &Options {
                format: Format::Markdown,
                channel: Some("#General".to_string()),
                date: Some("2021-01-01".to_string()),
            },
        )
        .unwrap();
        assert_eq!(t.messages, 2);
        // Non-matching date -> a clear error, not empty output.
        let err = render(
            &zip,
            &Options { format: Format::Markdown, date: Some("2020-01-01".to_string()), ..Default::default() },
        )
        .unwrap_err();
        assert!(err.contains("no messages matched"), "{err}");
    }

    #[test]
    fn strips_wrapping_export_folder() {
        let users = br#"[{"id":"U1","name":"alice","profile":{"display_name":"alice"}}]"#;
        let channels = br#"[{"id":"C1","name":"general"}]"#;
        let day = br#"[{"type":"message","user":"U1","ts":"1609459200.0","text":"hello"}]"#;
        let zip = make_zip(&[
            ("MyExport/users.json", users),
            ("MyExport/channels.json", channels),
            ("MyExport/general/2021-01-01.json", day),
        ]);
        let t = render(&zip, &Options::default()).unwrap();
        assert_eq!(t.channels, 1);
        assert!(t.content.contains("## #general"));
        assert!(t.content.contains("**alice**"));
    }

    #[test]
    fn errors_on_non_zip() {
        let err = render(b"not a zip at all", &Options::default()).unwrap_err();
        assert!(err.contains("not a valid zip"), "{err}");
    }

    #[test]
    fn errors_on_non_slack_zip() {
        let zip = make_zip(&[("readme.txt", b"hello"), ("data/notes.txt", b"x")]);
        let err = render(&zip, &Options::default()).unwrap_err();
        assert!(err.contains("does not look like a Slack export"), "{err}");
    }

    #[test]
    fn epoch_formats_utc() {
        assert_eq!(format_epoch(1_609_459_200), "2021-01-01 00:00:00 UTC");
        assert_eq!(format_epoch(0), "1970-01-01 00:00:00 UTC");
    }
}
