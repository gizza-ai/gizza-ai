//! twitter-archive-reader core — turn the `tweets.js` file from a Twitter/X data
//! export into a readable transcript plus posting statistics and top tweets.
//!
//! Pure Rust (`serde_json` only), no wafer/wasm-bindgen deps, so it runs on every
//! backend including the chat Service Worker.
//!
//! `tweets.js` is not JSON: it is a JavaScript assignment,
//! `window.YTD.tweets.part0 = [ … ]`, whose right-hand side is an array of
//! `{"tweet": {…}}` envelopes. The wrapper is stripped, a bare JSON array and
//! bare (un-enveloped) tweet objects are accepted too, and every unknown field is
//! ignored so a newer export still reads.
//!
//! Timestamps arrive in Twitter's `created_at` form (`Mon Jan 15 09:30:00 +0000
//! 2024`); ISO-8601 is accepted as a fallback. Everything is rendered in UTC so
//! filtering is reproducible. Engagement counts arrive as JSON strings in the
//! export and as numbers in some third-party dumps — both are read.

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
            "transcript" | "tweets" | "timeline" => Output::Transcript,
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

/// How the transcript and the summary are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Text,
    Html,
    Csv,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" | "" => Ok(Format::Markdown),
            "text" | "plain" | "txt" => Ok(Format::Text),
            "html" | "htm" => Ok(Format::Html),
            "csv" => Ok(Format::Csv),
            other => {
                Err(format!("unknown format '{other}': expected markdown, text, html or csv"))
            }
        }
    }
}

/// Transcript ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Newest,
    Oldest,
    Likes,
    Retweets,
}

impl Sort {
    pub fn parse(s: &str) -> Result<Sort, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "newest" | "recent" | "" => Ok(Sort::Newest),
            "oldest" | "chronological" => Ok(Sort::Oldest),
            "likes" | "favorites" => Ok(Sort::Likes),
            "retweets" | "reposts" => Ok(Sort::Retweets),
            other => {
                Err(format!("unknown sort '{other}': expected newest, oldest, likes or retweets"))
            }
        }
    }
}

/// Rendering options (mirror the block descriptor params).
#[derive(Debug, Clone)]
pub struct Options {
    pub output: Output,
    pub format: Format,
    pub sort: Sort,
    /// Case-insensitive substring kept from the tweet text (after t.co expansion).
    pub search: Option<String>,
    /// Inclusive `YYYY-MM-DD` lower bound on the UTC date.
    pub since: Option<String>,
    /// Inclusive `YYYY-MM-DD` upper bound on the UTC date.
    pub until: Option<String>,
    pub include_replies: bool,
    pub include_retweets: bool,
    /// Rewrite `t.co` links to the `expanded_url` stored in the archive.
    pub expand_urls: bool,
    /// How many most-liked tweets to list in the summary (0 = none).
    pub top_count: usize,
    /// Cap on rendered tweets after filtering and sorting (0 = no limit).
    pub max_tweets: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            output: Output::Both,
            format: Format::Markdown,
            sort: Sort::Newest,
            search: None,
            since: None,
            until: None,
            include_replies: true,
            include_retweets: true,
            expand_urls: true,
            top_count: 5,
            max_tweets: 0,
        }
    }
}

/// What a tweet is, in the transcript and in the counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Original,
    Reply,
    Retweet,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Original => "original",
            Kind::Reply => "reply",
            Kind::Retweet => "retweet",
        }
    }
}

#[derive(Debug, Clone)]
struct Tweet {
    id: String,
    ts: i64,
    text: String,
    likes: u64,
    retweets: u64,
    lang: String,
    source: String,
    kind: Kind,
    reply_to: Option<String>,
    hashtags: Vec<String>,
    mentions: Vec<String>,
    domains: Vec<String>,
    media: Vec<String>,
}

impl Tweet {
    fn permalink(&self) -> String {
        if self.id.is_empty() {
            String::new()
        } else {
            format!("https://twitter.com/i/web/status/{}", self.id)
        }
    }

    /// The transcript body: text plus any media placeholders.
    fn body(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.text.is_empty() {
            parts.push(self.text.clone());
        }
        for m in &self.media {
            parts.push(m.clone());
        }
        if parts.is_empty() {
            parts.push("[no text]".to_string());
        }
        parts.join(" ")
    }
}

/// Parse a Twitter/X `tweets.js` archive file and render it.
pub fn render(tweets_js: &str, opts: &Options) -> Result<String, String> {
    if tweets_js.trim().is_empty() {
        return Err("tweets.js is empty: paste the contents of the data/tweets.js file from \
                    your Twitter/X archive (it starts with window.YTD.tweets.part0 = [)"
            .to_string());
    }
    let since = parse_date_bound(opts.since.as_deref(), "since")?;
    let until = parse_date_bound(opts.until.as_deref(), "until")?;
    if let (Some(a), Some(b)) = (&since, &until) {
        if a > b {
            return Err(format!("since ({a}) is after until ({b}): swap the two dates"));
        }
    }

    let all = parse_archive(tweets_js, opts.expand_urls)?;
    let total = all.len();

    let needle = opts
        .search
        .as_deref()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    let mut kept: Vec<Tweet> = all
        .into_iter()
        .filter(|t| match t.kind {
            Kind::Reply => opts.include_replies,
            Kind::Retweet => opts.include_retweets,
            Kind::Original => true,
        })
        .filter(|t| {
            let day = date_of(t.ts);
            since.as_deref().map_or(true, |s| day.as_str() >= s)
                && until.as_deref().map_or(true, |u| day.as_str() <= u)
        })
        .filter(|t| match &needle {
            Some(n) => t.text.to_lowercase().contains(n),
            None => true,
        })
        .collect();

    match opts.sort {
        Sort::Newest => kept.sort_by(|a, b| b.ts.cmp(&a.ts).then_with(|| a.id.cmp(&b.id))),
        Sort::Oldest => kept.sort_by(|a, b| a.ts.cmp(&b.ts).then_with(|| a.id.cmp(&b.id))),
        Sort::Likes => kept.sort_by(|a, b| {
            b.likes.cmp(&a.likes).then_with(|| b.ts.cmp(&a.ts)).then_with(|| a.id.cmp(&b.id))
        }),
        Sort::Retweets => kept.sort_by(|a, b| {
            b.retweets.cmp(&a.retweets).then_with(|| b.ts.cmp(&a.ts)).then_with(|| a.id.cmp(&b.id))
        }),
    }

    let matched = kept.len();
    let truncated = opts.max_tweets > 0 && matched > opts.max_tweets;
    if truncated {
        kept.truncate(opts.max_tweets);
    }

    let mut out = String::new();
    if opts.output.wants_stats() {
        out.push_str(&render_stats(
            &kept,
            total,
            matched,
            if truncated { Some(opts.max_tweets) } else { None },
            opts,
        ));
    }
    if opts.output.wants_transcript() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&render_transcript(&kept, opts.format, opts.output == Output::Both));
    }
    Ok(out.trim_end().to_string())
}

// ---------------------------------------------------------------------------
// parsing
// ---------------------------------------------------------------------------

/// Strip the `window.YTD.tweets.part0 = ` JavaScript assignment (any part index,
/// `tweet`/`tweets` key) and any trailing `;`, leaving the JSON value.
fn strip_js_wrapper(raw: &str) -> &str {
    let mut t = raw.trim();
    while let Some(rest) = t.strip_suffix(';') {
        t = rest.trim_end();
    }
    if t.starts_with('[') || t.starts_with('{') {
        return t;
    }
    // `window.YTD.<key>.partN = [ … ]` — everything before the first `=` is the
    // assignment target; the URL-ish text of a tweet can't appear there.
    match t.find('=') {
        Some(i) => t[i + 1..].trim(),
        None => t,
    }
}

fn parse_archive(raw: &str, expand_urls: bool) -> Result<Vec<Tweet>, String> {
    let json = strip_js_wrapper(raw);
    let value: Value = serde_json::from_str(json).map_err(|e| {
        format!(
            "could not parse tweets.js as JSON ({e}). Paste the whole file, including the \
             `window.YTD.tweets.part0 = [` line, or a bare JSON array of tweets. Near: {}",
            snippet(json)
        )
    })?;
    let items = match &value {
        Value::Array(items) => items,
        other => {
            return Err(format!(
                "tweets.js holds a {} , expected a JSON array of tweets (the export writes \
                 `window.YTD.tweets.part0 = [ … ]`)",
                kind_of(other)
            ))
        }
    };

    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if let Some(body) = tweet_body(item) {
            out.push(read_tweet(body, expand_urls));
        }
    }
    if out.is_empty() {
        return Err("no tweets found: the array holds no tweet objects (each entry should be \
                    {\"tweet\": {\"id_str\": …, \"created_at\": …, \"full_text\": …}})"
            .to_string());
    }
    Ok(out)
}

/// The tweet object inside an entry: the `{"tweet": …}` envelope the export
/// writes, or a bare tweet object as some third-party dumps store it.
fn tweet_body(item: &Value) -> Option<&Value> {
    if let Some(inner) = item.get("tweet") {
        if inner.is_object() {
            return Some(inner);
        }
    }
    if item.is_object()
        && (item.get("full_text").is_some()
            || item.get("text").is_some()
            || item.get("id_str").is_some())
    {
        return Some(item);
    }
    None
}

fn read_tweet(t: &Value, expand_urls: bool) -> Tweet {
    let id = str_of(t, "id_str").or_else(|| str_of(t, "id")).unwrap_or_default();
    let ts = parse_created_at(&str_of(t, "created_at").unwrap_or_default());
    let raw_text = str_of(t, "full_text").or_else(|| str_of(t, "text")).unwrap_or_default();

    let entities = t.get("entities");
    let mut text = raw_text.clone();
    let mut domains: Vec<String> = Vec::new();
    for u in array_of(entities, "urls") {
        let short = str_of(u, "url").unwrap_or_default();
        let long = str_of(u, "expanded_url")
            .filter(|s| !s.is_empty())
            .or_else(|| str_of(u, "display_url"))
            .unwrap_or_default();
        if long.is_empty() {
            continue;
        }
        if let Some(d) = domain_of(&long) {
            if !domains.contains(&d) {
                domains.push(d);
            }
        }
        if expand_urls && !short.is_empty() {
            text = text.replace(&short, &long);
        }
    }

    // Media lives under extended_entities (all attachments) or entities (the
    // first one). Its `url` is the redundant t.co link already in the text.
    let mut media: Vec<String> = Vec::new();
    let media_items = {
        let ext = array_of(t.get("extended_entities"), "media");
        if ext.is_empty() {
            array_of(entities, "media")
        } else {
            ext
        }
    };
    for m in media_items {
        let short = str_of(m, "url").unwrap_or_default();
        if expand_urls && !short.is_empty() {
            text = text.replace(&short, "");
        }
        let kind = str_of(m, "type").unwrap_or_else(|| "media".to_string());
        let url = str_of(m, "media_url_https")
            .or_else(|| str_of(m, "media_url"))
            .or_else(|| str_of(m, "expanded_url"))
            .unwrap_or_default();
        let placeholder = if url.is_empty() {
            format!("[{kind}]")
        } else {
            format!("[{kind}: {url}]")
        };
        if !media.contains(&placeholder) {
            media.push(placeholder);
        }
    }

    let text = decode_entities(&squeeze(&text));

    let mut hashtags: Vec<String> = Vec::new();
    for h in array_of(entities, "hashtags") {
        if let Some(tag) = str_of(h, "text").filter(|s| !s.is_empty()) {
            let tag = format!("#{}", tag.to_lowercase());
            if !hashtags.contains(&tag) {
                hashtags.push(tag);
            }
        }
    }
    let mut mentions: Vec<String> = Vec::new();
    for m in array_of(entities, "user_mentions") {
        if let Some(name) = str_of(m, "screen_name").filter(|s| !s.is_empty()) {
            let name = format!("@{}", name.to_lowercase());
            if !mentions.contains(&name) {
                mentions.push(name);
            }
        }
    }

    let reply_to = str_of(t, "in_reply_to_screen_name").filter(|s| !s.is_empty());
    let is_reply = reply_to.is_some()
        || t.get("in_reply_to_status_id_str").map(|v| !v.is_null()).unwrap_or(false)
        || t.get("in_reply_to_user_id_str").map(|v| !v.is_null()).unwrap_or(false);
    let is_retweet = t.get("retweeted_status").map(|v| !v.is_null()).unwrap_or(false)
        || raw_text.starts_with("RT @");
    let kind = if is_retweet {
        Kind::Retweet
    } else if is_reply {
        Kind::Reply
    } else {
        Kind::Original
    };

    Tweet {
        id,
        ts,
        text,
        likes: count_of(t, "favorite_count"),
        retweets: count_of(t, "retweet_count"),
        lang: str_of(t, "lang").filter(|s| !s.is_empty()).unwrap_or_else(|| "und".to_string()),
        source: decode_entities(&strip_tags(&str_of(t, "source").unwrap_or_default())),
        kind,
        reply_to,
        hashtags,
        mentions,
        domains,
        media,
    }
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(|s| s.trim().to_string())
}

fn array_of<'a>(v: Option<&'a Value>, key: &str) -> Vec<&'a Value> {
    v.and_then(|v| v.get(key))
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

/// The export writes engagement counts as JSON strings; some dumps use numbers.
fn count_of(v: &Value, key: &str) -> u64 {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
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

fn snippet(s: &str) -> String {
    let cut: String = s.trim().chars().take(60).collect();
    if s.trim().chars().count() > 60 {
        format!("{cut}…")
    } else {
        cut
    }
}

fn domain_of(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split('/').next().unwrap_or("").split('?').next().unwrap_or("");
    let host = host.split('@').next_back().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host).trim().to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// The archive stores tweet text HTML-escaped (`&amp;`, `&lt;`, `&gt;`).
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let replaced = [
            ("&amp;", "&"),
            ("&lt;", "<"),
            ("&gt;", ">"),
            ("&quot;", "\""),
            ("&apos;", "'"),
            ("&#39;", "'"),
            ("&nbsp;", " "),
        ]
        .iter()
        .find(|(pat, _)| tail.starts_with(pat));
        match replaced {
            Some((pat, sub)) => {
                out.push_str(sub);
                rest = &tail[pat.len()..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Collapse the runs of blank space left behind by removing a media t.co link.
fn squeeze(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_space = false;
    for c in s.chars() {
        if c == ' ' || c == '\t' {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            last_space = c == '\n';
            out.push(c);
        }
    }
    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// dates
// ---------------------------------------------------------------------------

const MONTHS: [&str; 12] =
    ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// `Mon Jan 15 09:30:00 +0000 2024` (the export's form), with ISO-8601
/// (`2024-01-15T09:30:00Z`) accepted as a fallback. Unparseable → the epoch.
fn parse_created_at(raw: &str) -> i64 {
    let raw = raw.trim();
    if raw.is_empty() {
        return 0;
    }
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() >= 6 {
        if let Some(month) =
            MONTHS.iter().position(|m| parts[1].to_ascii_lowercase().starts_with(m))
        {
            let day: u32 = parts[2].parse().unwrap_or(1);
            let (h, mi, s) = parse_hms(parts[3]);
            let year: i64 = parts[5].parse().unwrap_or(1970);
            let offset = parse_offset(parts[4]);
            return days_from_civil(year, month as u32 + 1, day) * 86_400
                + h * 3600
                + mi * 60
                + s
                - offset;
        }
    }
    parse_iso(raw).unwrap_or(0)
}

fn parse_hms(s: &str) -> (i64, i64, i64) {
    let mut it = s.split(':');
    let h = it.next().unwrap_or("0").parse().unwrap_or(0);
    let m = it.next().unwrap_or("0").parse().unwrap_or(0);
    let sec = it.next().unwrap_or("0").split('.').next().unwrap_or("0").parse().unwrap_or(0);
    (h, m, sec)
}

fn parse_offset(s: &str) -> i64 {
    let sign = match s.chars().next() {
        Some('-') => -1,
        Some('+') => 1,
        _ => return 0,
    };
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 4 {
        return 0;
    }
    let hh: i64 = digits[..2].parse().unwrap_or(0);
    let mm: i64 = digits[2..4].parse().unwrap_or(0);
    sign * (hh * 3600 + mm * 60)
}

fn parse_iso(raw: &str) -> Option<i64> {
    let bytes = raw.as_bytes();
    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year: i64 = raw[..4].parse().ok()?;
    let month: u32 = raw[5..7].parse().ok()?;
    let day: u32 = raw[8..10].parse().ok()?;
    let (h, m, s) = if raw.len() >= 19 { parse_hms(&raw[11..19]) } else { (0, 0, 0) };
    Some(days_from_civil(year, month, day) * 86_400 + h * 3600 + m * 60 + s)
}

/// (year, month, day) → days since 1970-01-01. Howard Hinnant's days_from_civil.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
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

fn date_of(secs: i64) -> String {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

fn year_of(secs: i64) -> i64 {
    civil_from_days(secs.div_euclid(86_400)).0
}

fn fmt_ts(secs: i64, iso: bool) -> String {
    let (days, sod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (sod / 3600, (sod % 3600) / 60, sod % 60);
    if iso {
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    } else {
        format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02} UTC")
    }
}

fn parse_date_bound(raw: Option<&str>, field: &str) -> Result<Option<String>, String> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let bytes = raw.as_bytes();
    let shaped = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes.iter().enumerate().all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
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

// ---------------------------------------------------------------------------
// statistics
// ---------------------------------------------------------------------------

/// A three-column table in the summary. One shape renders in all four formats.
struct Section {
    /// The row tag used in CSV output.
    key: &'static str,
    title: String,
    headers: [&'static str; 3],
    rows: Vec<[String; 3]>,
}

const TOP_ROWS: usize = 10;

fn pct(part: usize, whole: usize) -> String {
    if whole == 0 {
        "0.00%".to_string()
    } else {
        format!("{:.2}%", part as f64 * 100.0 / whole as f64)
    }
}

fn avg(total: u64, whole: usize) -> String {
    if whole == 0 {
        "0.00".to_string()
    } else {
        format!("{:.2}", total as f64 / whole as f64)
    }
}

/// Count → rows sorted by count desc, label asc, capped at `TOP_ROWS`.
fn ranked(counts: BTreeMap<String, usize>, shown: usize) -> (Vec<[String; 3]>, usize) {
    let total = counts.len();
    let mut items: Vec<(String, usize)> = counts.into_iter().collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items.truncate(TOP_ROWS);
    (
        items
            .into_iter()
            .map(|(label, n)| [label, n.to_string(), pct(n, shown)])
            .collect(),
        total,
    )
}

fn section(
    key: &'static str,
    name: &str,
    unit: &'static str,
    counts: BTreeMap<String, usize>,
    shown: usize,
) -> Option<Section> {
    let (rows, total) = ranked(counts, shown);
    if rows.is_empty() {
        return None;
    }
    let title = if total > rows.len() {
        format!("Top {name} (top {} of {total})", rows.len())
    } else {
        format!("Top {name}")
    };
    Some(Section { key, title, headers: [name_header(name), unit, "Share"], rows })
}

fn name_header(name: &str) -> &'static str {
    match name {
        "hashtags" => "Hashtag",
        "mentions" => "Mention",
        "link domains" => "Domain",
        "sources" => "Posted from",
        "languages" => "Language",
        _ => "Item",
    }
}

fn summary_rows(
    kept: &[Tweet],
    total: usize,
    matched: usize,
    truncated_to: Option<usize>,
    opts: &Options,
) -> Vec<(String, String)> {
    let shown = kept.len();
    let originals = kept.iter().filter(|t| t.kind == Kind::Original).count();
    let replies = kept.iter().filter(|t| t.kind == Kind::Reply).count();
    let retweets = kept.iter().filter(|t| t.kind == Kind::Retweet).count();
    let likes: u64 = kept.iter().map(|t| t.likes).sum();
    let rts: u64 = kept.iter().map(|t| t.retweets).sum();
    let words: usize = kept.iter().map(|t| t.text.split_whitespace().count()).sum();

    let mut rows = vec![
        ("Tweets in file".to_string(), total.to_string()),
        ("Tweets after filters".to_string(), matched.to_string()),
        (
            "Tweets shown".to_string(),
            format!(
                "{shown} ({originals} original, {replies} replies, {retweets} retweets)"
            ),
        ),
        ("Likes received".to_string(), format!("{likes} (avg {} per tweet)", avg(likes, shown))),
        ("Retweets received".to_string(), format!("{rts} (avg {} per tweet)", avg(rts, shown))),
        ("Words shown".to_string(), words.to_string()),
    ];
    if let (Some(first), Some(last)) =
        (kept.iter().map(|t| t.ts).min(), kept.iter().map(|t| t.ts).max())
    {
        rows.push(("Date range".to_string(), format!("{} to {}", date_of(first), date_of(last))));
    }
    if let Some(cap) = truncated_to {
        rows.push(("Truncated to".to_string(), format!("{cap} tweets")));
    }
    if !opts.expand_urls {
        rows.push(("Link expansion".to_string(), "off (t.co links kept as-is)".to_string()));
    }
    rows
}

fn stat_sections(kept: &[Tweet], opts: &Options) -> Vec<Section> {
    let shown = kept.len();
    let mut sections = Vec::new();

    let mut years: BTreeMap<String, usize> = BTreeMap::new();
    let (mut tags, mut mentions, mut domains, mut sources, mut langs) = (
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    for t in kept {
        *years.entry(year_of(t.ts).to_string()).or_insert(0) += 1;
        for h in &t.hashtags {
            *tags.entry(h.clone()).or_insert(0) += 1;
        }
        for m in &t.mentions {
            *mentions.entry(m.clone()).or_insert(0) += 1;
        }
        for d in &t.domains {
            *domains.entry(d.clone()).or_insert(0) += 1;
        }
        if !t.source.is_empty() {
            *sources.entry(t.source.clone()).or_insert(0) += 1;
        }
        *langs.entry(t.lang.clone()).or_insert(0) += 1;
    }

    if !years.is_empty() {
        let rows: Vec<[String; 3]> = years
            .into_iter()
            .rev()
            .map(|(y, n)| [y, n.to_string(), pct(n, shown)])
            .collect();
        sections.push(Section {
            key: "year",
            title: "Tweets per year".to_string(),
            headers: ["Year", "Tweets", "Share"],
            rows,
        });
    }
    sections.extend(section("hashtag", "hashtags", "Tweets", tags, shown));
    sections.extend(section("mention", "mentions", "Tweets", mentions, shown));
    sections.extend(section("domain", "link domains", "Tweets", domains, shown));
    sections.extend(section("source", "sources", "Tweets", sources, shown));
    sections.extend(section("language", "languages", "Tweets", langs, shown));

    if opts.top_count > 0 && !kept.is_empty() {
        let mut top: Vec<&Tweet> = kept.iter().collect();
        top.sort_by(|a, b| {
            b.likes.cmp(&a.likes).then_with(|| b.retweets.cmp(&a.retweets)).then_with(|| {
                b.ts.cmp(&a.ts)
            })
        });
        top.truncate(opts.top_count);
        let rows: Vec<[String; 3]> = top
            .into_iter()
            .map(|t| {
                [
                    format!("{} — {}", date_of(t.ts), ellipsis(&t.body(), 80)),
                    format!("{} likes, {} retweets", t.likes, t.retweets),
                    t.permalink(),
                ]
            })
            .collect();
        sections.push(Section {
            key: "top",
            title: format!("Top {} by likes", rows.len()),
            headers: ["Tweet", "Engagement", "Link"],
            rows,
        });
    }
    sections
}

fn ellipsis(s: &str, max: usize) -> String {
    let flat = s.replace('\n', " ");
    if flat.chars().count() <= max {
        flat
    } else {
        format!("{}…", flat.chars().take(max).collect::<String>().trim_end())
    }
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn render_stats(
    kept: &[Tweet],
    total: usize,
    matched: usize,
    truncated_to: Option<usize>,
    opts: &Options,
) -> String {
    let rows = summary_rows(kept, total, matched, truncated_to, opts);
    let sections = stat_sections(kept, opts);
    match opts.format {
        Format::Csv => {
            let mut out = String::from("section,label,value,share\n");
            for (k, v) in rows {
                out.push_str(&format!("summary,{},{},\n", csv_cell(&k), csv_cell(&v)));
            }
            for s in sections {
                for r in s.rows {
                    out.push_str(&format!(
                        "{},{},{},{}\n",
                        s.key,
                        csv_cell(&r[0]),
                        csv_cell(&r[1]),
                        csv_cell(&r[2])
                    ));
                }
            }
            out
        }
        Format::Html => {
            let mut out = String::from("<h2>Summary</h2>\n<ul>\n");
            for (k, v) in rows {
                out.push_str(&format!("  <li>{}: {}</li>\n", esc(&k), esc(&v)));
            }
            out.push_str("</ul>\n");
            for s in sections {
                out.push_str(&format!("<h3>{}</h3>\n<table>\n<thead><tr>", esc(&s.title)));
                for h in s.headers {
                    out.push_str(&format!("<th>{}</th>", esc(h)));
                }
                out.push_str("</tr></thead>\n<tbody>\n");
                for r in s.rows {
                    out.push_str("<tr>");
                    for c in &r {
                        out.push_str(&format!("<td>{}</td>", esc(c)));
                    }
                    out.push_str("</tr>\n");
                }
                out.push_str("</tbody>\n</table>\n");
            }
            out
        }
        Format::Markdown => {
            let mut out = String::from("## Summary\n\n");
            for (k, v) in rows {
                out.push_str(&format!("- **{k}**: {v}\n"));
            }
            for s in sections {
                out.push_str(&format!("\n### {}\n\n", s.title));
                out.push_str(&format!("| {} | {} | {} |\n", s.headers[0], s.headers[1], s.headers[2]));
                out.push_str("| --- | ---: | --- |\n");
                for r in s.rows {
                    out.push_str(&format!("| {} | {} | {} |\n", md_cell(&r[0]), r[1], md_cell(&r[2])));
                }
            }
            out
        }
        Format::Text => {
            let mut out = String::new();
            for (k, v) in rows {
                out.push_str(&format!("{k}: {v}\n"));
            }
            for s in sections {
                out.push_str(&format!("\n{}:\n", s.title));
                let w0 = s
                    .rows
                    .iter()
                    .map(|r| r[0].chars().count())
                    .chain(std::iter::once(s.headers[0].chars().count()))
                    .max()
                    .unwrap_or(0)
                    .min(84);
                let w1 = s
                    .rows
                    .iter()
                    .map(|r| r[1].chars().count())
                    .chain(std::iter::once(s.headers[1].chars().count()))
                    .max()
                    .unwrap_or(0);
                for r in s.rows {
                    out.push_str(&format!(
                        "  {:<w0$}  {:>w1$}  {}\n",
                        r[0],
                        r[1],
                        r[2],
                        w0 = w0,
                        w1 = w1
                    ));
                }
            }
            out
        }
    }
}

fn meta_line(t: &Tweet) -> String {
    let mut parts = vec![t.kind.label().to_string()];
    if let Some(who) = &t.reply_to {
        parts.push(format!("to @{who}"));
    }
    parts.push(format!("{} likes", t.likes));
    parts.push(format!("{} retweets", t.retweets));
    if !t.lang.is_empty() {
        parts.push(t.lang.clone());
    }
    if !t.source.is_empty() {
        parts.push(t.source.clone());
    }
    parts.join(" · ")
}

fn render_transcript(kept: &[Tweet], format: Format, titled: bool) -> String {
    if kept.is_empty() {
        return match format {
            Format::Csv => {
                "date,id,kind,likes,retweets,language,source,text,permalink\n".to_string()
            }
            Format::Html => "<p>No tweets matched the filters.</p>\n".to_string(),
            _ => "No tweets matched the filters.\n".to_string(),
        };
    }
    match format {
        Format::Csv => {
            let mut out = String::from("date,id,kind,likes,retweets,language,source,text,permalink\n");
            for t in kept {
                out.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{}\n",
                    fmt_ts(t.ts, true),
                    csv_cell(&t.id),
                    t.kind.label(),
                    t.likes,
                    t.retweets,
                    csv_cell(&t.lang),
                    csv_cell(&t.source),
                    csv_cell(&t.body()),
                    csv_cell(&t.permalink())
                ));
            }
            out
        }
        Format::Html => {
            let mut out = String::new();
            if titled {
                out.push_str("<h2>Transcript</h2>\n");
            }
            for t in kept {
                out.push_str(&format!(
                    "<article>\n  <p><time>{}</time> — {}</p>\n  <p>{}</p>\n  <p><a \
                     href=\"{}\">{}</a></p>\n</article>\n",
                    esc(&fmt_ts(t.ts, false)),
                    esc(&meta_line(t)),
                    esc(&t.body()).replace('\n', "<br>"),
                    esc(&t.permalink()),
                    esc(&t.permalink())
                ));
            }
            out
        }
        Format::Markdown => {
            let mut out = String::new();
            if titled {
                out.push_str("## Transcript\n");
            }
            for t in kept {
                out.push_str(&format!(
                    "\n### {}\n\n{}\n\n*{} · [permalink]({})*\n",
                    fmt_ts(t.ts, false),
                    t.body(),
                    meta_line(t),
                    t.permalink()
                ));
            }
            out
        }
        Format::Text => {
            let mut out = String::new();
            if titled {
                out.push_str("=== Transcript ===\n");
            }
            for t in kept {
                out.push_str(&format!(
                    "\n[{}] {}\n{}\n{}\n",
                    fmt_ts(t.ts, false),
                    meta_line(t),
                    t.body(),
                    t.permalink()
                ));
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

/// A markdown table cell can't hold a raw `|` or a newline.
fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
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

    const SAMPLE: &str = r#"window.YTD.tweets.part0 = [
  {"tweet":{"id_str":"1746900000000000001","created_at":"Mon Jan 15 09:30:00 +0000 2024","full_text":"Shipped the new parser today &amp; it is fast https://t.co/abc123 #rust","favorite_count":"42","retweet_count":"7","lang":"en","source":"<a href=\"https://x.com\" rel=\"nofollow\">Twitter Web App</a>","entities":{"hashtags":[{"text":"rust"}],"user_mentions":[],"urls":[{"url":"https://t.co/abc123","expanded_url":"https://example.com/parser","display_url":"example.com/parser"}]}}},
  {"tweet":{"id_str":"1746900000000000002","created_at":"Mon Jan 15 11:00:00 +0000 2024","full_text":"@bob good point, will fix","favorite_count":"3","retweet_count":"0","lang":"en","source":"<a href=\"https://x.com\" rel=\"nofollow\">Twitter for iPhone</a>","in_reply_to_status_id_str":"1746800000000000009","in_reply_to_screen_name":"bob","entities":{"hashtags":[],"user_mentions":[{"screen_name":"bob"}],"urls":[]}}},
  {"tweet":{"id_str":"1746900000000000003","created_at":"Tue Jan 16 08:15:00 +0000 2024","full_text":"RT @carol: release notes are up #rust","favorite_count":"0","retweet_count":"12","lang":"en","source":"<a href=\"https://x.com\" rel=\"nofollow\">Twitter Web App</a>","entities":{"hashtags":[{"text":"rust"}],"user_mentions":[{"screen_name":"carol"}],"urls":[]}}}
]"#;

    #[test]
    fn renders_markdown_summary_and_transcript() {
        let out = render(SAMPLE, &Options::default()).unwrap();
        assert!(out.starts_with("## Summary"), "{out}");
        assert!(out.contains("- **Tweets in file**: 3"), "{out}");
        assert!(
            out.contains("- **Tweets shown**: 3 (1 original, 1 replies, 1 retweets)"),
            "{out}"
        );
        assert!(out.contains("- **Likes received**: 45 (avg 15.00 per tweet)"), "{out}");
        assert!(out.contains("- **Date range**: 2024-01-15 to 2024-01-16"), "{out}");
        assert!(out.contains("| #rust | 2 | 66.67% |"), "{out}");
        assert!(out.contains("| Twitter Web App | 2 | 66.67% |"), "{out}");
        assert!(out.contains("### Top 3 by likes"), "{out}");
        // Newest first.
        assert!(out.contains("## Transcript\n\n### 2024-01-16 08:15:00 UTC"), "{out}");
        assert!(
            out.contains("*reply · to @bob · 3 likes · 0 retweets · en · Twitter for iPhone · \
                          [permalink](https://twitter.com/i/web/status/1746900000000000002)*"),
            "{out}"
        );
    }

    #[test]
    fn expands_tco_links_and_decodes_entities() {
        let out = render(SAMPLE, &Options { output: Output::Transcript, ..Options::default() })
            .unwrap();
        assert!(out.contains("Shipped the new parser today & it is fast \
                              https://example.com/parser #rust"), "{out}");
        assert!(!out.contains("t.co/abc123"), "{out}");
    }

    #[test]
    fn expansion_can_be_switched_off() {
        let opts =
            Options { output: Output::Transcript, expand_urls: false, ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("https://t.co/abc123"), "{out}");
        assert!(!out.contains("https://example.com/parser"), "{out}");
    }

    #[test]
    fn accepts_a_bare_array_and_bare_tweet_objects() {
        let bare = r#"[{"id_str":"9","created_at":"Mon Jan 15 09:30:00 +0000 2024","full_text":"hello","favorite_count":1,"retweet_count":0,"lang":"en"}]"#;
        let out =
            render(bare, &Options { output: Output::Transcript, ..Options::default() }).unwrap();
        assert!(out.contains("hello"), "{out}");
        assert!(out.contains("https://twitter.com/i/web/status/9"), "{out}");
    }

    #[test]
    fn strips_any_part_index_and_key() {
        let js = r#"window.YTD.tweet.part17 = [{"tweet":{"id_str":"5","created_at":"Mon Jan 15 09:30:00 +0000 2024","full_text":"wrapped"}}];"#;
        let out =
            render(js, &Options { output: Output::Transcript, ..Options::default() }).unwrap();
        assert!(out.contains("wrapped"), "{out}");
    }

    #[test]
    fn classifies_replies_and_retweets_and_can_drop_them() {
        let opts = Options {
            output: Output::Transcript,
            include_replies: false,
            include_retweets: false,
            ..Options::default()
        };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("Shipped the new parser"), "{out}");
        assert!(!out.contains("good point, will fix"), "{out}");
        assert!(!out.contains("release notes are up"), "{out}");
    }

    #[test]
    fn search_filters_case_insensitively() {
        let opts = Options {
            output: Output::Transcript,
            search: Some("RELEASE".into()),
            ..Options::default()
        };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("release notes are up"), "{out}");
        assert!(!out.contains("Shipped the new parser"), "{out}");
    }

    #[test]
    fn date_bounds_filter_tweets() {
        let opts =
            Options { output: Output::Transcript, since: Some("2024-01-16".into()), ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("release notes are up"), "{out}");
        assert!(!out.contains("Shipped the new parser"), "{out}");

        let opts =
            Options { output: Output::Transcript, until: Some("2024-01-15".into()), ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("Shipped the new parser"), "{out}");
        assert!(!out.contains("release notes are up"), "{out}");
    }

    #[test]
    fn every_sort_order_picks_the_right_first_tweet() {
        let first = |sort: Sort| {
            let opts = Options {
                output: Output::Transcript,
                format: Format::Csv,
                sort,
                ..Options::default()
            };
            let out = render(SAMPLE, &opts).unwrap();
            out.lines().nth(1).unwrap().to_string()
        };
        assert!(first(Sort::Newest).starts_with("2024-01-16T08:15:00Z"), "{}", first(Sort::Newest));
        assert!(first(Sort::Oldest).starts_with("2024-01-15T09:30:00Z"), "{}", first(Sort::Oldest));
        assert!(first(Sort::Likes).contains(",42,"), "{}", first(Sort::Likes));
        assert!(first(Sort::Retweets).contains(",12,"), "{}", first(Sort::Retweets));
    }

    #[test]
    fn max_tweets_caps_at_its_boundary() {
        let opts = Options { max_tweets: 2, ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("- **Tweets after filters**: 3"), "{out}");
        assert!(out.contains("- **Truncated to**: 2 tweets"), "{out}");
        assert!(!out.contains("Shipped the new parser"), "{out}");

        let opts = Options { max_tweets: 3, ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(!out.contains("Truncated to"), "{out}");
        assert!(out.contains("Shipped the new parser"), "{out}");
    }

    #[test]
    fn csv_transcript_has_a_header_and_quotes_commas() {
        let opts =
            Options { output: Output::Transcript, format: Format::Csv, ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        let mut lines = out.lines();
        assert_eq!(lines.next().unwrap(), "date,id,kind,likes,retweets,language,source,text,permalink");
        assert_eq!(
            lines.next().unwrap(),
            "2024-01-16T08:15:00Z,1746900000000000003,retweet,0,12,en,Twitter Web App,RT @carol: \
             release notes are up #rust,https://twitter.com/i/web/status/1746900000000000003"
        );
        assert!(out.contains("\"@bob good point, will fix\""), "{out}");
    }

    #[test]
    fn csv_stats_tags_each_row_with_its_section() {
        let opts = Options { output: Output::Stats, format: Format::Csv, ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.starts_with("section,label,value,share\n"), "{out}");
        assert!(out.contains("summary,Tweets in file,3,\n"), "{out}");
        assert!(out.contains("year,2024,3,100.00%\n"), "{out}");
        assert!(out.contains("hashtag,#rust,2,66.67%\n"), "{out}");
        assert!(out.contains("language,en,3,100.00%\n"), "{out}");
        assert!(out.contains("mention,@bob,1,33.33%\n"), "{out}");
        assert!(out.contains("domain,example.com,1,33.33%\n"), "{out}");
    }

    #[test]
    fn html_output_escapes_the_tweet_text() {
        let js = r#"[{"tweet":{"id_str":"1","created_at":"Mon Jan 15 09:30:00 +0000 2024","full_text":"use &lt;b&gt; &amp; escape","lang":"en"}}]"#;
        let opts =
            Options { output: Output::Transcript, format: Format::Html, ..Options::default() };
        let out = render(js, &opts).unwrap();
        assert!(out.contains("<p>use &lt;b&gt; &amp; escape</p>"), "{out}");
        assert!(out.contains("<a href=\"https://twitter.com/i/web/status/1\">"), "{out}");
    }

    #[test]
    fn text_output_lists_media_placeholders() {
        let js = r#"[{"tweet":{"id_str":"1","created_at":"Mon Jan 15 09:30:00 +0000 2024","full_text":"look at this https://t.co/pic1","lang":"en","extended_entities":{"media":[{"url":"https://t.co/pic1","type":"photo","media_url_https":"https://pbs.twimg.com/media/a.jpg"}]}}}]"#;
        let opts =
            Options { output: Output::Transcript, format: Format::Text, ..Options::default() };
        let out = render(js, &opts).unwrap();
        assert!(out.contains("look at this [photo: https://pbs.twimg.com/media/a.jpg]"), "{out}");
    }

    #[test]
    fn top_tweets_can_be_switched_off() {
        let opts = Options { output: Output::Stats, top_count: 0, ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(!out.contains("by likes"), "{out}");

        let opts = Options { output: Output::Stats, top_count: 1, ..Options::default() };
        let out = render(SAMPLE, &opts).unwrap();
        assert!(out.contains("### Top 1 by likes"), "{out}");
        assert!(out.contains("42 likes, 7 retweets"), "{out}");
    }

    #[test]
    fn filters_that_match_nothing_are_not_an_error() {
        let opts = Options {
            output: Output::Transcript,
            search: Some("nothing here".into()),
            ..Options::default()
        };
        assert_eq!(render(SAMPLE, &opts).unwrap(), "No tweets matched the filters.");
    }

    #[test]
    fn rejects_empty_input() {
        let err = render("   \n ", &Options::default()).unwrap_err();
        assert!(err.contains("tweets.js is empty"), "{err}");
    }

    #[test]
    fn rejects_non_json_input() {
        let err = render("this is not an archive", &Options::default()).unwrap_err();
        assert!(err.contains("could not parse tweets.js as JSON"), "{err}");
    }

    #[test]
    fn rejects_json_that_is_not_an_array() {
        let err = render("{\"tweets\": 3}", &Options::default()).unwrap_err();
        assert!(err.contains("holds a JSON object , expected a JSON array of tweets"), "{err}");
    }

    #[test]
    fn rejects_an_array_without_tweets() {
        let err = render("[{\"follower\":{\"accountId\":\"1\"}}]", &Options::default()).unwrap_err();
        assert!(err.contains("no tweets found"), "{err}");
    }

    #[test]
    fn rejects_reversed_date_bounds_and_bad_dates() {
        let opts = Options {
            since: Some("2024-02-01".into()),
            until: Some("2024-01-01".into()),
            ..Options::default()
        };
        assert!(render(SAMPLE, &opts).unwrap_err().contains("is after until"));

        let opts = Options { since: Some("15/01/2024".into()), ..Options::default() };
        assert_eq!(
            render(SAMPLE, &opts).unwrap_err(),
            "since must be a date in YYYY-MM-DD form, got '15/01/2024'"
        );
    }

    #[test]
    fn format_and_sort_parse_reject_unknown_values() {
        assert_eq!(Format::parse("MD").unwrap(), Format::Markdown);
        assert!(Format::parse("pdf")
            .unwrap_err()
            .contains("expected markdown, text, html or csv"));
        assert_eq!(Sort::parse("Likes").unwrap(), Sort::Likes);
        assert!(Sort::parse("random")
            .unwrap_err()
            .contains("expected newest, oldest, likes or retweets"));
    }

    #[test]
    fn timestamps_parse_across_forms_and_offsets() {
        assert_eq!(parse_created_at("Mon Jan 15 09:30:00 +0000 2024"), 1_705_311_000);
        assert_eq!(parse_created_at("Mon Jan 15 10:30:00 +0100 2024"), 1_705_311_000);
        assert_eq!(parse_created_at("2024-01-15T09:30:00Z"), 1_705_311_000);
        assert_eq!(fmt_ts(1_705_311_000, true), "2024-01-15T09:30:00Z");
        assert_eq!(date_of(-86_400), "1969-12-31");
    }
}
