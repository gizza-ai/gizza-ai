//! mattermost-export-reader core — turn a Mattermost bulk export (JSONL, the
//! `import.jsonl` produced by `mmctl export create` / the bulk export tool) into
//! a readable per-channel transcript plus a summary.
//!
//! Pure Rust (`serde_json` only), no wafer/wasm-bindgen deps, so it runs on every
//! backend including the chat Service Worker.
//!
//! The export is JSON Lines: one self-contained object per line, each tagged with
//! a `type` field. Recognized types are `version`, `team`, `channel`, `user`,
//! `post`, `direct_channel`, `direct_post` and `emoji`; anything else is ignored
//! so a newer export still reads. Posts carry `create_at` in MILLISECONDS since
//! the Unix epoch, plus optional `replies`, `reactions` and `attachments` arrays.
//!
//! Author resolution comes from the `user` lines (nickname → first + last →
//! username) and channel resolution from the `channel` lines (display name +
//! public/private type); direct messages are labelled by their members.

use std::collections::BTreeMap;

use serde_json::Value;

/// Which sections to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Transcript,
    Stats,
    Both,
}

impl Output {
    pub fn parse(s: &str) -> Output {
        match s.trim().to_ascii_lowercase().as_str() {
            "transcript" | "text" | "messages" => Output::Transcript,
            "stats" | "statistics" | "summary" => Output::Stats,
            _ => Output::Both,
        }
    }

    fn wants_transcript(self) -> bool {
        matches!(self, Output::Transcript | Output::Both)
    }

    fn wants_stats(self) -> bool {
        matches!(self, Output::Stats | Output::Both)
    }
}

/// How the transcript (and, in CSV mode, the summary) is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Markdown,
    Html,
    Csv,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" | "plain" | "txt" | "" => Ok(Format::Text),
            "markdown" | "md" => Ok(Format::Markdown),
            "html" | "htm" => Ok(Format::Html),
            "csv" => Ok(Format::Csv),
            other => Err(format!(
                "unknown format '{other}': expected text, markdown, html or csv"
            )),
        }
    }
}

/// Rendering options (mirror the block descriptor params).
#[derive(Debug, Clone)]
pub struct Options {
    pub output: Output,
    pub format: Format,
    /// Keep only this channel (matched against the channel name or display name,
    /// case-insensitive, a leading `#` is ignored). Direct messages are dropped
    /// while a channel filter is set.
    pub channel: Option<String>,
    /// Keep only posts from this username or display name (case-insensitive).
    pub user_filter: Option<String>,
    /// Inclusive `YYYY-MM-DD` lower bound on the UTC date.
    pub since: Option<String>,
    /// Inclusive `YYYY-MM-DD` upper bound on the UTC date.
    pub until: Option<String>,
    pub include_direct_messages: bool,
    pub include_replies: bool,
    /// Cap on rendered messages after filtering (0 = no limit).
    pub max_messages: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            output: Output::Both,
            format: Format::Text,
            channel: None,
            user_filter: None,
            since: None,
            until: None,
            include_direct_messages: true,
            include_replies: true,
            max_messages: 0,
        }
    }
}

/// One rendered message (a root post or a thread reply).
#[derive(Debug, Clone)]
struct Msg {
    ts: i64,
    channel_sort: String,
    channel_label: String,
    author: String,
    username: String,
    text: String,
    extras: Vec<String>,
    is_reply: bool,
    is_dm: bool,
    words: usize,
}

#[derive(Default)]
struct Meta {
    version: Option<i64>,
    teams: Vec<String>,
    channels: BTreeMap<String, ChannelInfo>,
    users: BTreeMap<String, String>,
    emoji: usize,
    direct_channels: usize,
}

#[derive(Clone)]
struct ChannelInfo {
    name: String,
    display: String,
    private: bool,
}

/// Parse a Mattermost bulk export and render it.
pub fn render(export: &str, opts: &Options) -> Result<String, String> {
    if export.trim().is_empty() {
        return Err(
            "export is empty: paste the contents of the bulk export .jsonl file (one JSON object \
             per line)"
                .to_string(),
        );
    }
    let since = parse_date_bound(opts.since.as_deref(), "since")?;
    let until = parse_date_bound(opts.until.as_deref(), "until")?;
    if let (Some(a), Some(b)) = (&since, &until) {
        if a > b {
            return Err(format!("since ({a}) is after until ({b}): swap the two dates"));
        }
    }

    let meta = collect_meta(export)?;
    let (mut msgs, total_posts, total_dms, total_replies, total_reactions, total_attachments) =
        collect_messages(export, &meta, opts)?;

    if total_posts + total_dms == 0 && meta.channels.is_empty() && meta.users.is_empty() {
        return Err(
            "no Mattermost bulk export records found: expected JSON Lines tagged with \"type\" \
             (version, team, channel, user, post, direct_post)"
                .to_string(),
        );
    }

    msgs.sort_by(|a, b| a.ts.cmp(&b.ts).then_with(|| a.channel_sort.cmp(&b.channel_sort)));
    let matched = msgs.len();
    let truncated = opts.max_messages > 0 && matched > opts.max_messages;
    if truncated {
        msgs.truncate(opts.max_messages);
    }

    let stats = Stats {
        version: meta.version,
        teams: meta.teams.clone(),
        channels: meta.channels.len(),
        users: meta.users.len(),
        emoji: meta.emoji,
        direct_channels: meta.direct_channels,
        file_posts: total_posts,
        file_dms: total_dms,
        file_replies: total_replies,
        file_reactions: total_reactions,
        file_attachments: total_attachments,
        matched,
        truncated_to: if truncated { Some(opts.max_messages) } else { None },
        msgs: &msgs,
    };

    let mut out = String::new();
    if opts.output.wants_stats() {
        out.push_str(&render_stats(&stats, opts.format));
    }
    if opts.output.wants_transcript() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&render_transcript(&msgs, opts.format, opts.output == Output::Both));
    }
    Ok(out.trim_end().to_string())
}

// ---------------------------------------------------------------------------
// parsing
// ---------------------------------------------------------------------------

fn each_line(export: &str) -> impl Iterator<Item = (usize, &str)> {
    export
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty())
}

fn parse_line(no: usize, line: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(line)
        .map_err(|e| format!("line {no} is not valid JSON ({e}): {}", snippet(line)))?;
    if !v.is_object() {
        return Err(format!(
            "line {no} is a {} , expected a JSON object with a \"type\" field: {}",
            kind_of(&v),
            snippet(line)
        ));
    }
    if v.get("type").and_then(Value::as_str).is_none() {
        return Err(format!(
            "line {no} has no \"type\" field: every Mattermost bulk export line is tagged (version, \
             team, channel, user, post, direct_post): {}",
            snippet(line)
        ));
    }
    Ok(v)
}

fn kind_of(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "JSON array",
        Value::Object(_) => "JSON object",
    }
}

fn snippet(line: &str) -> String {
    let cut: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        format!("{cut}…")
    } else {
        cut
    }
}

fn collect_meta(export: &str) -> Result<Meta, String> {
    let mut meta = Meta::default();
    for (no, line) in each_line(export) {
        let v = parse_line(no, line)?;
        match v.get("type").and_then(Value::as_str).unwrap_or_default() {
            "version" => meta.version = v.get("version").and_then(Value::as_i64),
            "team" => {
                if let Some(t) = v.get("team") {
                    let label = str_of(t, "display_name")
                        .filter(|s| !s.is_empty())
                        .or_else(|| str_of(t, "name"))
                        .unwrap_or_default();
                    if !label.is_empty() && !meta.teams.contains(&label) {
                        meta.teams.push(label);
                    }
                }
            }
            "channel" => {
                if let Some(c) = v.get("channel") {
                    let name = str_of(c, "name").unwrap_or_default();
                    if name.is_empty() {
                        continue;
                    }
                    let display = str_of(c, "display_name").filter(|s| !s.is_empty());
                    let private = str_of(c, "type").as_deref() == Some("P");
                    meta.channels.insert(
                        name.to_ascii_lowercase(),
                        ChannelInfo {
                            display: display.clone().unwrap_or_else(|| name.clone()),
                            name,
                            private,
                        },
                    );
                }
            }
            "user" => {
                if let Some(u) = v.get("user") {
                    let username = str_of(u, "username").unwrap_or_default();
                    if username.is_empty() {
                        continue;
                    }
                    meta.users.insert(username.to_ascii_lowercase(), display_name(u, &username));
                }
            }
            "emoji" => meta.emoji += 1,
            "direct_channel" => meta.direct_channels += 1,
            _ => {}
        }
    }
    Ok(meta)
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(|s| s.trim().to_string())
}

fn display_name(user: &Value, username: &str) -> String {
    if let Some(nick) = str_of(user, "nickname").filter(|s| !s.is_empty()) {
        return nick;
    }
    let first = str_of(user, "first_name").unwrap_or_default();
    let last = str_of(user, "last_name").unwrap_or_default();
    let full = format!("{first} {last}");
    let full = full.trim();
    if full.is_empty() {
        username.to_string()
    } else {
        full.to_string()
    }
}

type Collected = (Vec<Msg>, usize, usize, usize, usize, usize);

fn collect_messages(export: &str, meta: &Meta, opts: &Options) -> Result<Collected, String> {
    let mut msgs = Vec::new();
    let (mut posts, mut dms, mut replies, mut reactions, mut attachments) = (0, 0, 0, 0, 0);
    let channel_want = opts
        .channel
        .as_deref()
        .map(|c| c.trim().trim_start_matches('#').to_ascii_lowercase())
        .filter(|c| !c.is_empty());
    let user_want = opts
        .user_filter
        .as_deref()
        .map(|u| u.trim().trim_start_matches('@').to_ascii_lowercase())
        .filter(|u| !u.is_empty());

    for (no, line) in each_line(export) {
        let v = parse_line(no, line)?;
        let ty = v.get("type").and_then(Value::as_str).unwrap_or_default();
        let (body, is_dm) = match ty {
            "post" => (v.get("post"), false),
            "direct_post" => (v.get("direct_post"), true),
            _ => continue,
        };
        let Some(body) = body else {
            return Err(format!("line {no} is typed \"{ty}\" but has no \"{ty}\" object"));
        };

        let (label, sort) = if is_dm {
            dm_label(body, meta)
        } else {
            let name = str_of(body, "channel").unwrap_or_default();
            channel_label(&name, meta)
        };
        if is_dm {
            dms += 1;
        } else {
            posts += 1;
        }
        let root_replies = body.get("replies").and_then(Value::as_array);
        replies += root_replies.map(|r| r.len()).unwrap_or(0);
        reactions += count_array(body, "reactions");
        attachments += count_array(body, "attachments");

        let keep_channel = match (&channel_want, is_dm) {
            (Some(_), true) => false,
            (Some(want), false) => {
                let key = str_of(body, "channel").unwrap_or_default().to_ascii_lowercase();
                let display = meta
                    .channels
                    .get(&key)
                    .map(|c| c.display.to_ascii_lowercase())
                    .unwrap_or_default();
                key == *want || display == *want
            }
            (None, true) => opts.include_direct_messages,
            (None, false) => true,
        };

        let mut units: Vec<(&Value, bool)> = vec![(body, false)];
        if opts.include_replies {
            if let Some(list) = root_replies {
                for r in list {
                    reactions += count_array(r, "reactions");
                    attachments += count_array(r, "attachments");
                    units.push((r, true));
                }
            }
        } else if let Some(list) = root_replies {
            for r in list {
                reactions += count_array(r, "reactions");
                attachments += count_array(r, "attachments");
            }
        }
        if !keep_channel {
            continue;
        }

        for (unit, is_reply) in units {
            let username = str_of(unit, "user").unwrap_or_else(|| "unknown".to_string());
            let author = meta
                .users
                .get(&username.to_ascii_lowercase())
                .cloned()
                .unwrap_or_else(|| username.clone());
            if let Some(want) = &user_want {
                if username.to_ascii_lowercase() != *want && author.to_ascii_lowercase() != *want {
                    continue;
                }
            }
            let ts = unit.get("create_at").and_then(Value::as_i64).unwrap_or(0);
            let day = date_of(ts);
            if let Some(s) = &opts.since {
                if day.as_str() < s.as_str() {
                    continue;
                }
            }
            if let Some(u) = &opts.until {
                if day.as_str() > u.as_str() {
                    continue;
                }
            }
            let text = str_of(unit, "message").unwrap_or_default();
            let mut extras = Vec::new();
            for path in unit
                .get("attachments")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|a| str_of(a, "path"))
            {
                extras.push(format!("[attachment: {path}]"));
            }
            if let Some(r) = reaction_summary(unit) {
                extras.push(r);
            }
            if text.is_empty() && extras.is_empty() {
                extras.push("[empty message]".to_string());
            }
            let words = text.split_whitespace().count();
            msgs.push(Msg {
                ts,
                channel_sort: sort.clone(),
                channel_label: label.clone(),
                author,
                username,
                text,
                extras,
                is_reply,
                is_dm,
                words,
            });
        }
    }
    Ok((msgs, posts, dms, replies, reactions, attachments))
}

fn count_array(v: &Value, key: &str) -> usize {
    v.get(key).and_then(Value::as_array).map(|a| a.len()).unwrap_or(0)
}

fn reaction_summary(unit: &Value) -> Option<String> {
    let list = unit.get("reactions").and_then(Value::as_array)?;
    if list.is_empty() {
        return None;
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for r in list {
        let name = str_of(r, "emoji_name").unwrap_or_else(|| "?".to_string());
        *counts.entry(name).or_insert(0) += 1;
    }
    let parts: Vec<String> = counts
        .into_iter()
        .map(|(n, c)| if c > 1 { format!(":{n}: x{c}") } else { format!(":{n}:") })
        .collect();
    Some(format!("[reactions: {}]", parts.join(", ")))
}

fn channel_label(name: &str, meta: &Meta) -> (String, String) {
    let key = name.to_ascii_lowercase();
    match meta.channels.get(&key) {
        Some(info) => {
            let mut label = format!("#{}", info.name);
            if !info.display.is_empty() && info.display != info.name {
                label.push_str(&format!(" ({})", info.display));
            }
            if info.private {
                label.push_str(" [private]");
            }
            (label, format!("0{key}"))
        }
        None => {
            let shown = if name.is_empty() { "unknown".to_string() } else { name.to_string() };
            (format!("#{shown}"), format!("0{}", shown.to_ascii_lowercase()))
        }
    }
}

fn dm_label(body: &Value, meta: &Meta) -> (String, String) {
    let mut members: Vec<String> = body
        .get("channel_members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|u| {
            meta.users.get(&u.to_ascii_lowercase()).cloned().unwrap_or_else(|| u.to_string())
        })
        .collect();
    members.sort();
    members.dedup();
    let joined = if members.is_empty() { "unknown".to_string() } else { members.join(", ") };
    let label = if members.len() > 2 {
        format!("Group message: {joined}")
    } else {
        format!("Direct message: {joined}")
    };
    (label, format!("1{}", joined.to_ascii_lowercase()))
}

// ---------------------------------------------------------------------------
// dates
// ---------------------------------------------------------------------------

fn parse_date_bound(raw: Option<&str>, field: &str) -> Result<Option<String>, String> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let bytes = raw.as_bytes();
    let shaped = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
    if !shaped {
        return Err(format!("{field} must be a date in YYYY-MM-DD form, got '{raw}'"));
    }
    let month: u32 = raw[5..7].parse().unwrap_or(0);
    let day: u32 = raw[8..10].parse().unwrap_or(0);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(format!("{field} '{raw}' is not a real calendar date (month 01-12, day 01-31)"));
    }
    Ok(Some(raw.to_string()))
}

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's civil_from_days.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn split_ms(ms: i64) -> (i64, i64) {
    let secs = ms.div_euclid(1000);
    (secs.div_euclid(86_400), secs.rem_euclid(86_400))
}

fn date_of(ms: i64) -> String {
    let (days, _) = split_ms(ms);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn fmt_ts(ms: i64, iso: bool) -> String {
    let (days, sod) = split_ms(ms);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (sod / 3600, (sod % 3600) / 60, sod % 60);
    if iso {
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    } else {
        format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02} UTC")
    }
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

struct Stats<'a> {
    version: Option<i64>,
    teams: Vec<String>,
    channels: usize,
    users: usize,
    emoji: usize,
    direct_channels: usize,
    file_posts: usize,
    file_dms: usize,
    file_replies: usize,
    file_reactions: usize,
    file_attachments: usize,
    matched: usize,
    truncated_to: Option<usize>,
    msgs: &'a [Msg],
}

struct Tally {
    per_channel: Vec<(String, usize)>,
    per_author: Vec<(String, usize, usize)>,
    words: usize,
    replies: usize,
    first: Option<i64>,
    last: Option<i64>,
}

fn tally(msgs: &[Msg]) -> Tally {
    let mut chan: BTreeMap<String, usize> = BTreeMap::new();
    let mut auth: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let (mut words, mut replies) = (0, 0);
    let (mut first, mut last) = (None, None);
    for m in msgs {
        *chan.entry(m.channel_label.clone()).or_insert(0) += 1;
        let key = if m.author == m.username {
            m.username.clone()
        } else {
            format!("{} (@{})", m.author, m.username)
        };
        let e = auth.entry(key).or_insert((0, 0));
        e.0 += 1;
        e.1 += m.words;
        words += m.words;
        if m.is_reply {
            replies += 1;
        }
        first = Some(first.map_or(m.ts, |f: i64| f.min(m.ts)));
        last = Some(last.map_or(m.ts, |l: i64| l.max(m.ts)));
    }
    let mut per_channel: Vec<(String, usize)> = chan.into_iter().collect();
    per_channel.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut per_author: Vec<(String, usize, usize)> =
        auth.into_iter().map(|(k, (c, w))| (k, c, w)).collect();
    per_author.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Tally { per_channel, per_author, words, replies, first, last }
}

fn pct(part: usize, whole: usize) -> String {
    if whole == 0 {
        "0.00%".to_string()
    } else {
        format!("{:.2}%", part as f64 * 100.0 / whole as f64)
    }
}

fn summary_rows(s: &Stats) -> Vec<(String, String)> {
    let t = tally(s.msgs);
    let mut rows = vec![
        (
            "Bulk export format version".to_string(),
            s.version.map(|v| v.to_string()).unwrap_or_else(|| "not declared".to_string()),
        ),
        (
            "Teams".to_string(),
            if s.teams.is_empty() {
                "0".to_string()
            } else {
                format!("{} ({})", s.teams.len(), s.teams.join(", "))
            },
        ),
        ("Channels in export".to_string(), s.channels.to_string()),
        ("Direct conversations in export".to_string(), s.direct_channels.to_string()),
        ("Users in export".to_string(), s.users.to_string()),
        ("Custom emoji in export".to_string(), s.emoji.to_string()),
        (
            "Messages in export".to_string(),
            format!(
                "{} ({} channel posts, {} direct messages, {} thread replies)",
                s.file_posts + s.file_dms + s.file_replies,
                s.file_posts,
                s.file_dms,
                s.file_replies
            ),
        ),
        ("Reactions in export".to_string(), s.file_reactions.to_string()),
        ("Attachments in export".to_string(), s.file_attachments.to_string()),
        ("Messages after filters".to_string(), s.matched.to_string()),
        ("Thread replies shown".to_string(), t.replies.to_string()),
        ("Words shown".to_string(), t.words.to_string()),
    ];
    if let (Some(f), Some(l)) = (t.first, t.last) {
        rows.push(("Date range".to_string(), format!("{} to {}", date_of(f), date_of(l))));
    }
    if let Some(cap) = s.truncated_to {
        rows.push(("Truncated to".to_string(), format!("{cap} messages")));
    }
    rows
}

fn render_stats(s: &Stats, format: Format) -> String {
    let t = tally(s.msgs);
    let rows = summary_rows(s);
    match format {
        Format::Csv => {
            let mut out = String::from("section,key,value\n");
            for (k, v) in rows {
                out.push_str(&format!("summary,{},{}\n", csv_cell(&k), csv_cell(&v)));
            }
            for (label, n) in &t.per_channel {
                out.push_str(&format!(
                    "channel,{},{}\n",
                    csv_cell(label),
                    csv_cell(&format!("{n} ({})", pct(*n, s.matched)))
                ));
            }
            for (who, n, w) in &t.per_author {
                out.push_str(&format!(
                    "author,{},{}\n",
                    csv_cell(who),
                    csv_cell(&format!("{n} ({}, {w} words)", pct(*n, s.matched)))
                ));
            }
            out
        }
        Format::Html => {
            let mut out = String::from("<h2>Summary</h2>\n<ul>\n");
            for (k, v) in rows {
                out.push_str(&format!("  <li>{}: {}</li>\n", esc(&k), esc(&v)));
            }
            out.push_str("</ul>\n");
            if !t.per_channel.is_empty() {
                out.push_str("<h3>Messages per channel</h3>\n<ul>\n");
                for (label, n) in &t.per_channel {
                    out.push_str(&format!(
                        "  <li>{} — {n} ({})</li>\n",
                        esc(label),
                        pct(*n, s.matched)
                    ));
                }
                out.push_str("</ul>\n");
            }
            if !t.per_author.is_empty() {
                out.push_str("<h3>Messages per author</h3>\n<ul>\n");
                for (who, n, w) in &t.per_author {
                    out.push_str(&format!(
                        "  <li>{} — {n} ({}, {w} words)</li>\n",
                        esc(who),
                        pct(*n, s.matched)
                    ));
                }
                out.push_str("</ul>\n");
            }
            out
        }
        Format::Markdown => {
            let mut out = String::from("## Summary\n\n");
            for (k, v) in rows {
                out.push_str(&format!("- **{k}**: {v}\n"));
            }
            if !t.per_channel.is_empty() {
                out.push_str("\n### Messages per channel\n\n| Channel | Messages | Share |\n| --- | ---: | ---: |\n");
                for (label, n) in &t.per_channel {
                    out.push_str(&format!("| {label} | {n} | {} |\n", pct(*n, s.matched)));
                }
            }
            if !t.per_author.is_empty() {
                out.push_str("\n### Messages per author\n\n| Author | Messages | Share | Words |\n| --- | ---: | ---: | ---: |\n");
                for (who, n, w) in &t.per_author {
                    out.push_str(&format!("| {who} | {n} | {} | {w} |\n", pct(*n, s.matched)));
                }
            }
            out
        }
        Format::Text => {
            let mut out = String::new();
            for (k, v) in rows {
                out.push_str(&format!("{k}: {v}\n"));
            }
            if !t.per_channel.is_empty() {
                out.push_str("\nMessages per channel:\n");
                for (label, n) in &t.per_channel {
                    out.push_str(&format!("{n:>7}  {:>7}  {label}\n", pct(*n, s.matched)));
                }
            }
            if !t.per_author.is_empty() {
                out.push_str("\nMessages per author:\n");
                for (who, n, w) in &t.per_author {
                    out.push_str(&format!(
                        "{n:>7}  {:>7}  {who}  ({w} words)\n",
                        pct(*n, s.matched)
                    ));
                }
            }
            out
        }
    }
}

fn grouped(msgs: &[Msg]) -> Vec<(String, Vec<&Msg>)> {
    let mut order: Vec<(String, String)> = Vec::new();
    let mut buckets: BTreeMap<String, Vec<&Msg>> = BTreeMap::new();
    for m in msgs {
        if !buckets.contains_key(&m.channel_sort) {
            order.push((m.channel_sort.clone(), m.channel_label.clone()));
        }
        buckets.entry(m.channel_sort.clone()).or_default().push(m);
    }
    order.sort();
    order
        .into_iter()
        .map(|(sort, label)| (label, buckets.remove(&sort).unwrap_or_default()))
        .collect()
}

fn body_of(m: &Msg) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if !m.text.is_empty() {
        parts.push(&m.text);
    }
    for e in &m.extras {
        parts.push(e);
    }
    parts.join(" ")
}

fn render_transcript(msgs: &[Msg], format: Format, titled: bool) -> String {
    if msgs.is_empty() {
        return match format {
            Format::Csv => "timestamp,channel,author,username,kind,message\n".to_string(),
            Format::Html => "<p>No messages matched the filters.</p>\n".to_string(),
            _ => "No messages matched the filters.\n".to_string(),
        };
    }
    let sections = grouped(msgs);
    match format {
        Format::Csv => {
            let mut out = String::from("timestamp,channel,author,username,kind,message\n");
            for m in msgs {
                out.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    fmt_ts(m.ts, true),
                    csv_cell(&m.channel_label),
                    csv_cell(&m.author),
                    csv_cell(&m.username),
                    if m.is_reply {
                        "reply"
                    } else if m.is_dm {
                        "direct"
                    } else {
                        "post"
                    },
                    csv_cell(&body_of(m))
                ));
            }
            out
        }
        Format::Html => {
            let mut out = String::new();
            if titled {
                out.push_str("<h2>Transcript</h2>\n");
            }
            for (label, list) in sections {
                out.push_str(&format!("<h3>{}</h3>\n", esc(&label)));
                for m in list {
                    let cls = if m.is_reply { " class=\"reply\"" } else { "" };
                    let prefix = if m.is_reply { "&#8627; " } else { "" };
                    out.push_str(&format!(
                        "<p{cls}>{prefix}<time>{}</time> <strong>{}</strong>: {}</p>\n",
                        esc(&fmt_ts(m.ts, false)),
                        esc(&m.author),
                        esc(&body_of(m)).replace('\n', "<br>")
                    ));
                }
            }
            out
        }
        Format::Markdown => {
            let mut out = String::new();
            if titled {
                out.push_str("## Transcript\n");
            }
            for (label, list) in sections {
                out.push_str(&format!("\n### {label}\n\n"));
                for m in list {
                    let indent = if m.is_reply { "  " } else { "" };
                    let bullet = if m.is_reply { "- ↳ " } else { "- " };
                    let body = body_of(m).replace('\n', &format!("\n{indent}  "));
                    out.push_str(&format!(
                        "{indent}{bullet}**{}** ({}): {}\n",
                        m.author,
                        fmt_ts(m.ts, false),
                        body
                    ));
                }
            }
            out
        }
        Format::Text => {
            let mut out = String::new();
            if titled {
                out.push_str("=== Transcript ===\n");
            }
            for (label, list) in sections {
                out.push_str(&format!("\n--- {label} ---\n\n"));
                for m in list {
                    let indent = if m.is_reply { "    ↳ " } else { "" };
                    let cont = if m.is_reply { "\n      " } else { "\n    " };
                    let body = body_of(m).replace('\n', cont);
                    out.push_str(&format!(
                        "{indent}[{}] {}: {}\n",
                        fmt_ts(m.ts, false),
                        m.author,
                        body
                    ));
                }
            }
            out
        }
    }
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn esc(s: &str) -> String {
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

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"type":"version","version":1}
{"type":"team","team":{"name":"core","display_name":"Core Team","type":"O"}}
{"type":"channel","channel":{"team":"core","name":"town-square","display_name":"Town Square","type":"O"}}
{"type":"channel","channel":{"team":"core","name":"release","display_name":"Release Room","type":"P"}}
{"type":"user","user":{"username":"alice","email":"alice@example.com","first_name":"Alice","last_name":"Anderson"}}
{"type":"user","user":{"username":"bob","email":"bob@example.com","nickname":"Bobby"}}
{"type":"post","post":{"team":"core","channel":"town-square","user":"alice","message":"Standup in five","create_at":1705311000000,"reactions":[{"user":"bob","emoji_name":"thumbsup","create_at":1705311060000}],"replies":[{"user":"bob","message":"On my way","create_at":1705311120000}]}}
{"type":"post","post":{"team":"core","channel":"release","user":"bob","message":"Cutting 9.4 today","create_at":1705397400000,"attachments":[{"path":"files/checklist.pdf"}]}}
{"type":"direct_channel","direct_channel":{"members":["alice","bob"]}}
{"type":"direct_post","direct_post":{"channel_members":["alice","bob"],"user":"bob","message":"thanks for the review","create_at":1705400000000}}"#;

    #[test]
    fn renders_text_transcript_and_stats() {
        let out = render(SAMPLE, &Options::default()).unwrap();
        assert!(out.contains("Bulk export format version: 1"), "{out}");
        assert!(out.contains("Teams: 1 (Core Team)"), "{out}");
        assert!(out.contains("Messages in export: 4 (2 channel posts, 1 direct messages, 1 thread replies)"), "{out}");
        assert!(out.contains("--- #town-square (Town Square) ---"), "{out}");
        assert!(out.contains("--- #release (Release Room) [private] ---"), "{out}");
        assert!(out.contains("[2024-01-15 09:30:00 UTC] Alice Anderson: Standup in five [reactions: :thumbsup:]"), "{out}");
        assert!(out.contains("↳ [2024-01-15 09:32:00 UTC] Bobby: On my way"), "{out}");
        assert!(out.contains("[attachment: files/checklist.pdf]"), "{out}");
        assert!(out.contains("--- Direct message: Alice Anderson, Bobby ---"), "{out}");
        assert!(out.contains("Alice Anderson (@alice)"), "{out}");
    }

    #[test]
    fn rejects_a_non_json_line() {
        let err = render("{\"type\":\"version\",\"version\":1}\nnot json here", &Options::default())
            .unwrap_err();
        assert!(err.starts_with("line 2 is not valid JSON"), "{err}");
        assert!(err.contains("not json here"), "{err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = render("   \n  ", &Options::default()).unwrap_err();
        assert!(err.contains("export is empty"), "{err}");
    }

    #[test]
    fn rejects_a_line_without_a_type() {
        let err = render("{\"post\":{\"message\":\"hi\"}}", &Options::default()).unwrap_err();
        assert!(err.contains("line 1 has no \"type\" field"), "{err}");
    }

    #[test]
    fn rejects_input_that_is_not_a_mattermost_export() {
        let err = render("{\"type\":\"note\",\"body\":\"hello\"}", &Options::default()).unwrap_err();
        assert!(err.contains("no Mattermost bulk export records found"), "{err}");
    }

    #[test]
    fn rejects_a_bad_date_bound() {
        let opts = Options { since: Some("15/01/2024".into()), ..Options::default() };
        let err = render(SAMPLE, &opts).unwrap_err();
        assert_eq!(err, "since must be a date in YYYY-MM-DD form, got '15/01/2024'");
    }

    #[test]
    fn rejects_reversed_date_bounds() {
        let opts = Options {
            since: Some("2024-02-01".into()),
            until: Some("2024-01-01".into()),
            ..Options::default()
        };
        assert!(render(SAMPLE, &opts).unwrap_err().contains("is after until"));
    }

    #[test]
    fn channel_filter_keeps_one_channel_and_drops_dms() {
        let opts = Options {
            output: Output::Transcript,
            channel: Some("#town-square".into()),
            ..Options::default()
        };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("Standup in five"), "{out}");
        assert!(!out.contains("Cutting 9.4"), "{out}");
        assert!(!out.contains("thanks for the review"), "{out}");
    }

    #[test]
    fn channel_filter_also_matches_the_display_name() {
        let opts = Options {
            output: Output::Transcript,
            channel: Some("Release Room".into()),
            ..Options::default()
        };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("Cutting 9.4 today"), "{out}");
        assert!(!out.contains("Standup in five"), "{out}");
    }

    #[test]
    fn user_filter_matches_username_or_display_name() {
        let by_username = Options {
            output: Output::Transcript,
            user_filter: Some("bob".into()),
            ..Options::default()
        };
        let out = render(SAMPLE, &by_username).unwrap();
        assert!(out.contains("On my way"), "{out}");
        assert!(!out.contains("Standup in five"), "{out}");

        let by_display = Options {
            output: Output::Transcript,
            user_filter: Some("Alice Anderson".into()),
            ..Options::default()
        };
        let out = render(SAMPLE, &by_display).unwrap();
        assert!(out.contains("Standup in five"), "{out}");
        assert!(!out.contains("On my way"), "{out}");
    }

    #[test]
    fn date_bounds_filter_messages() {
        let opts = Options {
            output: Output::Transcript,
            since: Some("2024-01-16".into()),
            ..Options::default()
        };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(!out.contains("Standup in five"), "{out}");
        assert!(out.contains("Cutting 9.4 today"), "{out}");

        let opts = Options {
            output: Output::Transcript,
            until: Some("2024-01-15".into()),
            ..Options::default()
        };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("Standup in five"), "{out}");
        assert!(!out.contains("Cutting 9.4 today"), "{out}");
    }

    #[test]
    fn replies_and_dms_can_be_switched_off() {
        let opts = Options {
            output: Output::Transcript,
            include_replies: false,
            include_direct_messages: false,
            ..Options::default()
        };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(!out.contains("On my way"), "{out}");
        assert!(!out.contains("thanks for the review"), "{out}");
        assert!(out.contains("Standup in five"), "{out}");
    }

    #[test]
    fn max_messages_truncates_chronologically() {
        let opts = Options { max_messages: 2, ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("Truncated to: 2 messages"), "{out}");
        assert!(out.contains("Standup in five"), "{out}");
        assert!(out.contains("On my way"), "{out}");
        assert!(!out.contains("Cutting 9.4 today"), "{out}");
    }

    #[test]
    fn csv_format_quotes_and_labels_rows() {
        let opts =
            Options { output: Output::Transcript, format: Format::Csv, ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        let mut lines = out.lines();
        assert_eq!(lines.next().unwrap(), "timestamp,channel,author,username,kind,message");
        assert_eq!(
            lines.next().unwrap(),
            "2024-01-15T09:30:00Z,#town-square (Town Square),Alice Anderson,alice,post,\
             Standup in five [reactions: :thumbsup:]"
        );
        assert_eq!(
            lines.next().unwrap(),
            "2024-01-15T09:32:00Z,#town-square (Town Square),Bobby,bob,reply,On my way"
        );
        // A DM label carries a comma, so it must be quoted.
        assert!(out.contains(",\"Direct message: Alice Anderson, Bobby\","), "{out}");
    }

    #[test]
    fn html_format_escapes_message_text() {
        let export = r#"{"type":"version","version":1}
{"type":"channel","channel":{"team":"core","name":"dev","display_name":"dev","type":"O"}}
{"type":"post","post":{"team":"core","channel":"dev","user":"alice","message":"use <b> & escape","create_at":1705311000000}}"#;
        let opts =
            Options { output: Output::Transcript, format: Format::Html, ..Options::default() };
        let out = render(export, &opts).unwrap();
        assert!(out.contains("<h3>#dev</h3>"), "{out}");
        assert!(out.contains("use &lt;b&gt; &amp; escape"), "{out}");
    }

    #[test]
    fn markdown_format_nests_replies() {
        let opts =
            Options { output: Output::Transcript, format: Format::Markdown, ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("### #town-square (Town Square)"), "{out}");
        assert!(out.contains("- **Alice Anderson** (2024-01-15 09:30:00 UTC): Standup in five"), "{out}");
        assert!(out.contains("  - ↳ **Bobby** (2024-01-15 09:32:00 UTC): On my way"), "{out}");
    }

    #[test]
    fn filters_that_match_nothing_are_not_an_error() {
        let opts = Options {
            output: Output::Transcript,
            user_filter: Some("nobody".into()),
            ..Options::default()
        };
        assert_eq!(render(SAMPLE, &opts).unwrap(), "No messages matched the filters.");
    }

    #[test]
    fn unknown_users_and_channels_fall_back_to_their_raw_ids() {
        let export = r#"{"type":"version","version":1}
{"type":"post","post":{"team":"core","channel":"random","user":"carol","message":"hi","create_at":1705311000000}}"#;
        let out = render(export, &Options { output: Output::Transcript, ..Options::default() })
            .unwrap();
        assert!(out.contains("--- #random ---"), "{out}");
        assert!(out.contains("carol: hi"), "{out}");
    }

    #[test]
    fn empty_messages_and_group_dms_are_labelled() {
        let export = r#"{"type":"version","version":1}
{"type":"direct_post","direct_post":{"channel_members":["alice","bob","carol"],"user":"alice","message":"","attachments":[],"create_at":1705311000000}}"#;
        let out = render(export, &Options { output: Output::Transcript, ..Options::default() })
            .unwrap();
        assert!(out.contains("--- Group message: alice, bob, carol ---"), "{out}");
        assert!(out.contains("[empty message]"), "{out}");
    }

    #[test]
    fn timestamps_round_trip_across_epochs() {
        assert_eq!(fmt_ts(0, false), "1970-01-01 00:00:00 UTC");
        assert_eq!(fmt_ts(1705311000000, true), "2024-01-15T09:30:00Z");
        assert_eq!(date_of(-86_400_000), "1969-12-31");
    }

    #[test]
    fn format_parse_rejects_unknown_values() {
        assert_eq!(Format::parse("MD").unwrap(), Format::Markdown);
        assert!(Format::parse("pdf").unwrap_err().contains("expected text, markdown, html or csv"));
    }
}
