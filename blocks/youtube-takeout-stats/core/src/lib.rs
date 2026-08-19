//! youtube-takeout-stats core — turn a Google/YouTube Takeout **watch-history**
//! export into viewing statistics: totals, top channels, top videos, videos per
//! month, and time-of-day / weekday patterns.
//!
//! Pure Rust (`serde_json` + `scraper`, no wafer/wasm-bindgen deps) so the same
//! logic backs the chat block, the CLI, and the browser page.
//!
//! Accepted inputs (auto-detected):
//!   * `watch-history.json` — the JSON export format: a top-level array of
//!     activity records with `header`, `title` ("Watched …"), `titleUrl`,
//!     `subtitles[0].name` (the channel) and an ISO-8601 UTC `time`.
//!   * `watch-history.html` — the HTML export format Takeout still produces when
//!     the export format is left on HTML. Its timestamps are already written in
//!     the exporting account's local time, so `utc_offset` is NOT applied to
//!     them (applying it twice would shift the clock).
//!
//! Everything is deterministic: no clock, no network, no filesystem.

use scraper::{Html, Selector};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Largest accepted input, in bytes (~24 MB of pasted export text).
pub const MAX_INPUT_BYTES: usize = 24_000_000;
/// Largest accepted number of parsed watch records.
pub const MAX_ENTRIES: usize = 500_000;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// Weekday names, Monday-first (index 0 = Monday).
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Output serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Csv,
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => Ok(OutputFormat::Text),
            "csv" => Ok(OutputFormat::Csv),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "invalid output {other:?}: expected one of text, csv, json"
            )),
        }
    }
}

/// Which section of the statistics to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    Overview,
    Channels,
    Videos,
    Months,
    Weekdays,
    Hours,
}

impl Report {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "overview" => Ok(Report::Overview),
            "channels" => Ok(Report::Channels),
            "videos" => Ok(Report::Videos),
            "months" => Ok(Report::Months),
            "weekdays" => Ok(Report::Weekdays),
            "hours" => Ok(Report::Hours),
            other => Err(format!(
                "invalid report {other:?}: expected one of overview, channels, videos, months, weekdays, hours"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Parsed watch record
// ---------------------------------------------------------------------------

/// One watch event pulled out of the export.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Video title with the "Watched " activity prefix removed.
    pub title: String,
    /// Video URL, empty when the export did not carry one.
    pub url: String,
    /// Channel name; `None` for removed/private videos (no channel is exported).
    pub channel: Option<String>,
    /// Seconds since the Unix epoch, as written in the export.
    pub epoch: i64,
    /// True when `epoch` is already local wall-clock time (HTML exports).
    pub local_clock: bool,
    pub is_ad: bool,
    pub is_music: bool,
}

/// Counts of rows the parser recognized but deliberately did not include.
#[derive(Debug, Clone, Default)]
struct Skipped {
    ads: u64,
    music: u64,
    out_of_range: u64,
}

// ---------------------------------------------------------------------------
// Calendar helpers (Howard Hinnant's civil-date algorithms — no clock needed)
// ---------------------------------------------------------------------------

/// Days since 1970-01-01 for a proleptic-Gregorian civil date.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Monday-first weekday index (0 = Monday) for a day number.
fn weekday_index(day: i64) -> usize {
    (day + 3).rem_euclid(7) as usize
}

fn fmt_date(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Parse a `YYYY-MM-DD` date into a day number.
fn parse_ymd(s: &str, field: &str) -> Result<i64, String> {
    let t = s.trim();
    let parts: Vec<&str> = t.split('-').collect();
    let bad = || format!("invalid {field} {t:?}: expected a YYYY-MM-DD date, e.g. 2024-01-31");
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return Err(bad());
    }
    let y: i64 = parts[0].parse().map_err(|_| bad())?;
    let m: i64 = parts[1].parse().map_err(|_| bad())?;
    let d: i64 = parts[2].parse().map_err(|_| bad())?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(bad());
    }
    Ok(days_from_civil(y, m, d))
}

/// Parse an ISO-8601 timestamp (`2024-03-05T18:42:11.123Z`, `…+02:00`, or a bare
/// `YYYY-MM-DD HH:MM:SS`) into seconds since the Unix epoch (UTC).
pub fn parse_iso8601(s: &str) -> Result<i64, String> {
    let t = s.trim();
    let bad =
        || format!("invalid timestamp {t:?}: expected an ISO-8601 time like 2024-03-05T18:42:11Z");
    let b = t.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') {
        return Err(bad());
    }
    let y: i64 = t[0..4].parse().map_err(|_| bad())?;
    let mo: i64 = t[5..7].parse().map_err(|_| bad())?;
    let d: i64 = t[8..10].parse().map_err(|_| bad())?;
    let hh: i64 = t[11..13].parse().map_err(|_| bad())?;
    let mi: i64 = t[14..16].parse().map_err(|_| bad())?;
    let ss: i64 = t[17..19].parse().map_err(|_| bad())?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || hh > 23 || mi > 59 || ss > 60 {
        return Err(bad());
    }
    let mut rest = &t[19..];
    if let Some(r) = rest.strip_prefix('.') {
        let n = r.chars().take_while(|c| c.is_ascii_digit()).count();
        rest = &r[n..];
    }
    let tz_secs = match rest.trim() {
        "" | "Z" | "z" => 0,
        other => {
            let sign = match other.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return Err(bad()),
            };
            let digits: String = other[1..].chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() != 4 {
                return Err(bad());
            }
            let oh: i64 = digits[0..2].parse().map_err(|_| bad())?;
            let om: i64 = digits[2..4].parse().map_err(|_| bad())?;
            sign * (oh * 3600 + om * 60)
        }
    };
    Ok(days_from_civil(y, mo, d) * 86_400 + hh * 3600 + mi * 60 + ss - tz_secs)
}

/// Parse a Takeout HTML date such as `Mar 5, 2024, 6:42:11 PM PST` into seconds
/// since the epoch, interpreted as **local wall-clock** time (the zone
/// abbreviation is informational and is not converted).
fn parse_takeout_html_date(s: &str) -> Option<i64> {
    let norm: String = s
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let t = norm.split_whitespace().collect::<Vec<_>>().join(" ");
    let parts: Vec<&str> = t.split(", ").collect();
    if parts.len() < 3 {
        return None;
    }
    let mut head = parts[0].split(' ');
    let mon_name = head.next()?;
    let mon = MONTHS.iter().position(|m| *m == mon_name)? as i64 + 1;
    let day: i64 = head.next()?.parse().ok()?;
    let year: i64 = parts[1].trim().parse().ok()?;
    let mut tail = parts[2].split(' ');
    let clock = tail.next()?;
    let mut hms = clock.split(':');
    let mut hour: i64 = hms.next()?.parse().ok()?;
    let min: i64 = hms.next()?.parse().ok()?;
    let sec: i64 = hms.next().unwrap_or("0").parse().ok()?;
    match tail.next() {
        Some("PM") => {
            if hour < 12 {
                hour += 12;
            }
        }
        Some("AM") => {
            if hour == 12 {
                hour = 0;
            }
        }
        _ => {}
    }
    if !(1..=31).contains(&day) || hour > 23 || min > 59 || sec > 60 {
        return None;
    }
    Some(days_from_civil(year, mon, day) * 86_400 + hour * 3600 + min * 60 + sec)
}

// ---------------------------------------------------------------------------
// Export parsing
// ---------------------------------------------------------------------------

fn strip_watched_prefix(title: &str) -> String {
    let t = title.trim();
    for p in ["Watched ", "Watched\u{a0}"] {
        if let Some(r) = t.strip_prefix(p) {
            return r.trim().to_string();
        }
    }
    t.to_string()
}

fn looks_unavailable(title: &str) -> bool {
    let t = title.to_ascii_lowercase();
    t.contains("has been removed") || t.starts_with("private video") || t == "a private video"
}

/// Extract a YouTube video id from a watch/shorts/youtu.be URL.
fn video_id(url: &str) -> Option<String> {
    if let Some(i) = url.find("v=") {
        let id: String = url[i + 2..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if !id.is_empty() {
            return Some(id);
        }
    }
    for marker in ["/shorts/", "youtu.be/", "/live/"] {
        if let Some(i) = url.find(marker) {
            let id: String = url[i + marker.len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if !id.is_empty() {
                return Some(id);
            }
        }
    }
    None
}

fn parse_json_export(input: &str) -> Result<Vec<Entry>, String> {
    let root: Value = serde_json::from_str(input)
        .map_err(|e| format!("invalid JSON in the watch-history export: {e}"))?;
    let records: Vec<&Value> = match &root {
        Value::Array(a) => a.iter().collect(),
        Value::Object(_) => vec![&root],
        _ => {
            return Err(
                "unexpected JSON: watch-history.json is a top-level array of activity records"
                    .into(),
            )
        }
    };
    if records.len() > MAX_ENTRIES {
        return Err(format!(
            "export holds {} records; the maximum is {MAX_ENTRIES}",
            records.len()
        ));
    }
    let mut out = Vec::with_capacity(records.len());
    for rec in records {
        let obj = match rec.as_object() {
            Some(o) => o,
            None => continue,
        };
        let header = obj.get("header").and_then(Value::as_str).unwrap_or("");
        let raw_title = obj.get("title").and_then(Value::as_str).unwrap_or("");
        let url = obj.get("titleUrl").and_then(Value::as_str).unwrap_or("");
        let is_watch = raw_title.starts_with("Watched")
            || url.contains("watch?v=")
            || url.contains("/shorts/")
            || url.contains("youtu.be/");
        if !is_watch {
            continue;
        }
        let time = match obj.get("time").and_then(Value::as_str) {
            Some(t) => t,
            None => continue,
        };
        let epoch = parse_iso8601(time)?;
        let channel = obj
            .get("subtitles")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(|s| s.get("name"))
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let is_ad = obj
            .get("details")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter().any(|d| {
                    d.get("name")
                        .and_then(Value::as_str)
                        .map(|n| n.contains("Google Ads"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        let products_music = obj
            .get("products")
            .and_then(Value::as_array)
            .map(|a| a.iter().any(|p| p.as_str() == Some("YouTube Music")))
            .unwrap_or(false);
        let is_music = header.eq_ignore_ascii_case("YouTube Music")
            || url.contains("music.youtube.com")
            || products_music;
        let title = strip_watched_prefix(raw_title);
        out.push(Entry {
            channel: if looks_unavailable(&title) {
                None
            } else {
                channel
            },
            title,
            url: url.to_string(),
            epoch,
            local_clock: false,
            is_ad,
            is_music,
        });
    }
    Ok(out)
}

fn parse_html_export(input: &str) -> Result<Vec<Entry>, String> {
    let doc = Html::parse_document(input);
    let outer = Selector::parse("div.outer-cell").unwrap();
    let body = Selector::parse("div.content-cell").unwrap();
    let header = Selector::parse("div.header-cell").unwrap();
    let link = Selector::parse("a").unwrap();

    let mut out = Vec::new();
    for cell in doc.select(&outer) {
        let product = cell
            .select(&header)
            .next()
            .map(|h| h.text().collect::<String>())
            .unwrap_or_default();
        let cells: Vec<_> = cell.select(&body).collect();
        let main = match cells.first() {
            Some(c) => *c,
            None => continue,
        };
        let main_text = main.text().collect::<String>();
        if !main_text.trim_start().starts_with("Watched") {
            continue;
        }
        let links: Vec<(String, String)> = main
            .select(&link)
            .map(|a| {
                (
                    a.value().attr("href").unwrap_or("").to_string(),
                    a.text().collect::<String>().trim().to_string(),
                )
            })
            .collect();
        // The trailing bare text node carries the timestamp.
        let epoch = match main.text().filter_map(parse_takeout_html_date).last() {
            Some(e) => e,
            None => continue,
        };
        let (url, title) = match links.first() {
            Some((u, t)) if !t.is_empty() => (u.clone(), t.clone()),
            _ => (String::new(), strip_watched_prefix(&main_text)),
        };
        let channel = links
            .iter()
            .find(|(u, _)| u.contains("/channel/") || u.contains("/@") || u.contains("/user/"))
            .map(|(_, t)| t.clone())
            .filter(|s| !s.is_empty());
        let extra: String = cells
            .iter()
            .skip(1)
            .map(|c| c.text().collect::<String>())
            .collect();
        let is_ad = extra.contains("Google Ads");
        let is_music = product.contains("YouTube Music") || url.contains("music.youtube.com");
        out.push(Entry {
            channel: if looks_unavailable(&title) {
                None
            } else {
                channel
            },
            title,
            url,
            epoch,
            local_clock: true,
            is_ad,
            is_music,
        });
        if out.len() > MAX_ENTRIES {
            return Err(format!(
                "export holds more than {MAX_ENTRIES} records; split it into smaller files"
            ));
        }
    }
    Ok(out)
}

/// Auto-detect the export format and parse it into watch entries.
pub fn parse_export(input: &str) -> Result<(Vec<Entry>, &'static str), String> {
    let t = input.trim_start();
    if t.is_empty() {
        return Err(
            "input is empty: paste the contents of watch-history.json or watch-history.html".into(),
        );
    }
    if t.starts_with('[') || t.starts_with('{') {
        Ok((parse_json_export(input)?, "watch-history.json"))
    } else if t.starts_with('<') || t.to_ascii_lowercase().contains("<html") {
        Ok((parse_html_export(input)?, "watch-history.html"))
    } else {
        Err(format!(
            "unrecognized export: expected watch-history.json (starts with '[') or watch-history.html (starts with '<'), got {:?}",
            t.chars().take(24).collect::<String>()
        ))
    }
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

struct Counted {
    day: i64,
    hour: usize,
    channel: Option<String>,
    key: String,
    title: String,
    url: String,
}

struct Stats {
    source: &'static str,
    total: u64,
    unique_videos: usize,
    unique_channels: usize,
    first_day: i64,
    last_day: i64,
    active_days: usize,
    per_day: Vec<(i64, u64)>,
    channels: Vec<(String, u64)>,
    videos: Vec<(String, String, String, u64)>,
    months: Vec<(String, u64)>,
    weekdays: [u64; 7],
    hours: [u64; 24],
    streak_len: usize,
    streak_start: i64,
    streak_end: i64,
    unavailable: u64,
    skipped: Skipped,
}

#[allow(clippy::too_many_arguments)]
fn compute(
    entries: Vec<Entry>,
    source: &'static str,
    offset_secs: i64,
    include_ads: bool,
    include_music: bool,
    from_day: Option<i64>,
    to_day: Option<i64>,
) -> Result<Stats, String> {
    let mut skipped = Skipped::default();
    let mut counted: Vec<Counted> = Vec::new();
    let mut unavailable = 0u64;

    for e in entries {
        if e.is_ad && !include_ads {
            skipped.ads += 1;
            continue;
        }
        if e.is_music && !include_music {
            skipped.music += 1;
            continue;
        }
        let local = if e.local_clock {
            e.epoch
        } else {
            e.epoch + offset_secs
        };
        let day = local.div_euclid(86_400);
        if from_day.map(|f| day < f).unwrap_or(false) || to_day.map(|t| day > t).unwrap_or(false) {
            skipped.out_of_range += 1;
            continue;
        }
        let hour = (local.rem_euclid(86_400) / 3600) as usize;
        if e.channel.is_none() {
            unavailable += 1;
        }
        let key = video_id(&e.url).unwrap_or_else(|| e.title.clone());
        counted.push(Counted {
            day,
            hour,
            channel: e.channel,
            key,
            title: e.title,
            url: e.url,
        });
    }

    if counted.is_empty() {
        return Err(
            "no watch records matched: the export held no \"Watched …\" entries in range (check the date filters, or enable ads / YouTube Music)"
                .into(),
        );
    }

    let total = counted.len() as u64;
    let mut day_counts: HashMap<i64, u64> = HashMap::new();
    let mut channel_counts: HashMap<String, u64> = HashMap::new();
    let mut video_counts: HashMap<String, (String, String, u64)> = HashMap::new();
    let mut month_counts: HashMap<(i64, i64), u64> = HashMap::new();
    let mut weekdays = [0u64; 7];
    let mut hours = [0u64; 24];

    for c in &counted {
        *day_counts.entry(c.day).or_insert(0) += 1;
        weekdays[weekday_index(c.day)] += 1;
        hours[c.hour] += 1;
        let (y, m, _) = civil_from_days(c.day);
        *month_counts.entry((y, m)).or_insert(0) += 1;
        if let Some(ch) = &c.channel {
            *channel_counts.entry(ch.clone()).or_insert(0) += 1;
        }
        let slot = video_counts
            .entry(c.key.clone())
            .or_insert_with(|| (c.title.clone(), c.url.clone(), 0));
        slot.2 += 1;
    }

    let first_day = *day_counts.keys().min().unwrap();
    let last_day = *day_counts.keys().max().unwrap();

    let mut per_day: Vec<(i64, u64)> = day_counts.into_iter().collect();
    per_day.sort_by(|a, b| a.0.cmp(&b.0));

    let mut channels: Vec<(String, u64)> = channel_counts.into_iter().collect();
    channels.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let unique_channels = channels.len();

    let mut videos: Vec<(String, String, String, u64)> = video_counts
        .into_iter()
        .map(|(k, (title, url, n))| (k, title, url, n))
        .collect();
    let unique_videos = videos.len();
    videos.sort_by(|a, b| b.3.cmp(&a.3).then(a.1.cmp(&b.1)).then(a.0.cmp(&b.0)));

    // Channel lookup for the top-videos table.
    let mut video_channel: HashMap<String, String> = HashMap::new();
    for c in &counted {
        if let Some(ch) = &c.channel {
            video_channel.entry(c.key.clone()).or_insert(ch.clone());
        }
    }
    let videos: Vec<(String, String, String, u64)> = videos
        .into_iter()
        .map(|(k, title, url, n)| {
            let ch = video_channel.get(&k).cloned().unwrap_or_default();
            (title, ch, url, n)
        })
        .collect();

    // Months, with gap months filled in so a trend line reads correctly.
    let (fy, fm, _) = civil_from_days(first_day);
    let (ly, lm, _) = civil_from_days(last_day);
    let mut months = Vec::new();
    let (mut y, mut m) = (fy, fm);
    while (y, m) <= (ly, lm) {
        months.push((
            format!("{y:04}-{m:02}"),
            *month_counts.get(&(y, m)).unwrap_or(&0),
        ));
        m += 1;
        if m > 12 {
            m = 1;
            y += 1;
        }
        if months.len() > 12_000 {
            return Err(
                "export spans an implausible number of months; check the timestamps".into(),
            );
        }
    }

    // Longest run of consecutive active days.
    let (mut streak_len, mut streak_end) = (1usize, per_day[0].0);
    let mut cur_len = 1usize;
    for w in per_day.windows(2) {
        cur_len = if w[1].0 == w[0].0 + 1 { cur_len + 1 } else { 1 };
        if cur_len > streak_len {
            streak_len = cur_len;
            streak_end = w[1].0;
        }
    }
    let streak_start = streak_end - streak_len as i64 + 1;

    Ok(Stats {
        source,
        total,
        unique_videos,
        unique_channels,
        first_day,
        last_day,
        active_days: per_day.len(),
        per_day,
        channels,
        videos,
        months,
        weekdays,
        hours,
        streak_len,
        streak_start,
        streak_end,
        unavailable,
        skipped,
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Append a table row with any trailing padding removed (an empty bar must not
/// leave invisible whitespace at the end of a line).
fn push_row(out: &mut String, line: String) {
    out.push_str(line.trim_end());
    out.push('\n');
}

fn bar(n: u64, max: u64) -> String {
    if n == 0 || max == 0 {
        return String::new();
    }
    let w = ((n as f64 / max as f64) * 20.0).round().max(1.0) as usize;
    "#".repeat(w)
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let head: String = s.chars().take(n.saturating_sub(1)).collect();
    format!("{head}…")
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn csv_row(cells: &[String]) -> String {
    cells
        .iter()
        .map(|c| csv_field(c))
        .collect::<Vec<_>>()
        .join(",")
}

fn peak_hour(s: &Stats) -> usize {
    let mut best = 0usize;
    for h in 1..24 {
        if s.hours[h] > s.hours[best] {
            best = h;
        }
    }
    best
}

fn busiest_weekday(s: &Stats) -> usize {
    let mut best = 0usize;
    for d in 1..7 {
        if s.weekdays[d] > s.weekdays[best] {
            best = d;
        }
    }
    best
}

fn busiest_day(s: &Stats) -> (i64, u64) {
    let mut best = s.per_day[0];
    for &(d, n) in &s.per_day {
        if n > best.1 {
            best = (d, n);
        }
    }
    best
}

fn render_text(s: &Stats, report: Report, top: usize) -> String {
    match report {
        Report::Overview => {
            let span = s.last_day - s.first_day + 1;
            let ph = peak_hour(s);
            let (bday, bcount) = busiest_day(s);
            let mut out = String::new();
            out.push_str(&format!(
                "YouTube watch history — {} videos watched\n",
                s.total
            ));
            out.push_str(&format!(
                "Range: {} → {} ({span} days, {} active)\n",
                fmt_date(s.first_day),
                fmt_date(s.last_day),
                s.active_days
            ));
            out.push_str(&format!(
                "Averages: {:.2} per active day, {:.2} per day\n",
                s.total as f64 / s.active_days as f64,
                s.total as f64 / span as f64
            ));
            out.push_str(&format!(
                "Unique videos: {} · Unique channels: {}\n",
                s.unique_videos, s.unique_channels
            ));
            out.push_str(&format!(
                "Peak hour: {ph:02}:00 · Busiest weekday: {} · Busiest day: {} ({bcount})\n",
                WEEKDAYS[busiest_weekday(s)],
                fmt_date(bday)
            ));
            out.push_str(&format!(
                "Longest streak: {} days ({} → {})\n",
                s.streak_len,
                fmt_date(s.streak_start),
                fmt_date(s.streak_end)
            ));
            if s.unavailable > 0 {
                out.push_str(&format!(
                    "Unavailable (removed or private): {}\n",
                    s.unavailable
                ));
            }
            let sk = &s.skipped;
            if sk.ads + sk.music + sk.out_of_range > 0 {
                let mut bits = Vec::new();
                if sk.ads > 0 {
                    bits.push(format!(
                        "{} ad{}",
                        sk.ads,
                        if sk.ads == 1 { "" } else { "s" }
                    ));
                }
                if sk.music > 0 {
                    bits.push(format!("{} YouTube Music", sk.music));
                }
                if sk.out_of_range > 0 {
                    bits.push(format!("{} outside the date range", sk.out_of_range));
                }
                out.push_str(&format!("Skipped: {}\n", bits.join(", ")));
            }
            out.push('\n');
            out.push_str(&render_text(s, Report::Channels, top));
            out.push_str("\n\n");
            out.push_str(&render_text(s, Report::Videos, top));
            out.push_str("\n\n");
            out.push_str(&render_text(s, Report::Months, top));
            out.push_str("\n\n");
            out.push_str(&render_text(s, Report::Weekdays, top));
            out.push_str("\n\n");
            out.push_str(&render_text(s, Report::Hours, top));
            out
        }
        Report::Channels => {
            if s.channels.is_empty() {
                return "Top channels\n  (no channel information in this export)".to_string();
            }
            let shown: Vec<_> = s.channels.iter().take(top).collect();
            let max = shown[0].1;
            let w = shown
                .iter()
                .map(|(n, _)| trunc(n, 34).chars().count())
                .max()
                .unwrap_or(8);
            let mut out = format!("Top channels ({} of {})\n", shown.len(), s.channels.len());
            for (i, (name, n)) in shown.iter().enumerate() {
                push_row(
                    &mut out,
                    format!(
                        "{:>3}. {:<w$}  {:>5}  {:>5.1}%  {}",
                        i + 1,
                        trunc(name, 34),
                        n,
                        *n as f64 * 100.0 / s.total as f64,
                        bar(*n, max),
                        w = w
                    ),
                );
            }
            out.trim_end().to_string()
        }
        Report::Videos => {
            let shown: Vec<_> = s.videos.iter().take(top).collect();
            let mut out = format!("Top videos ({} of {})\n", shown.len(), s.videos.len());
            for (i, (title, channel, _url, n)) in shown.iter().enumerate() {
                let who = if channel.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", trunc(channel, 28))
                };
                push_row(
                    &mut out,
                    format!("{:>3}. {:>3}×  {}{}", i + 1, n, trunc(title, 54), who),
                );
            }
            out.trim_end().to_string()
        }
        Report::Months => {
            let max = s.months.iter().map(|(_, n)| *n).max().unwrap_or(0);
            let mut out = String::from("Videos per month\n");
            for (m, n) in &s.months {
                push_row(&mut out, format!("  {m}  {n:>6}  {}", bar(*n, max)));
            }
            out.trim_end().to_string()
        }
        Report::Weekdays => {
            let max = s.weekdays.iter().copied().max().unwrap_or(0);
            let mut out = String::from("Videos per weekday\n");
            for (i, name) in WEEKDAYS.iter().enumerate() {
                push_row(
                    &mut out,
                    format!(
                        "  {name}  {:>6}  {}",
                        s.weekdays[i],
                        bar(s.weekdays[i], max)
                    ),
                );
            }
            out.trim_end().to_string()
        }
        Report::Hours => {
            let max = s.hours.iter().copied().max().unwrap_or(0);
            let mut out = String::from("Videos per hour\n");
            for (h, n) in s.hours.iter().enumerate() {
                push_row(&mut out, format!("  {h:02}  {n:>6}  {}", bar(*n, max)));
            }
            out.trim_end().to_string()
        }
    }
}

fn render_csv(s: &Stats, report: Report, top: usize) -> String {
    let mut rows: Vec<String> = Vec::new();
    match report {
        Report::Overview => {
            let span = s.last_day - s.first_day + 1;
            let (bday, bcount) = busiest_day(s);
            rows.push("metric,value".into());
            let push = |rows: &mut Vec<String>, k: &str, v: String| {
                rows.push(csv_row(&[k.to_string(), v]));
            };
            push(&mut rows, "source_format", s.source.to_string());
            push(&mut rows, "total_videos", s.total.to_string());
            push(&mut rows, "unique_videos", s.unique_videos.to_string());
            push(&mut rows, "unique_channels", s.unique_channels.to_string());
            push(&mut rows, "first_watch", fmt_date(s.first_day));
            push(&mut rows, "last_watch", fmt_date(s.last_day));
            push(&mut rows, "span_days", span.to_string());
            push(&mut rows, "active_days", s.active_days.to_string());
            push(
                &mut rows,
                "avg_per_active_day",
                format!("{:.2}", s.total as f64 / s.active_days as f64),
            );
            push(
                &mut rows,
                "avg_per_day",
                format!("{:.2}", s.total as f64 / span as f64),
            );
            push(&mut rows, "peak_hour", format!("{:02}", peak_hour(s)));
            push(
                &mut rows,
                "busiest_weekday",
                WEEKDAYS[busiest_weekday(s)].to_string(),
            );
            push(&mut rows, "busiest_day", fmt_date(bday));
            push(&mut rows, "busiest_day_videos", bcount.to_string());
            push(&mut rows, "longest_streak_days", s.streak_len.to_string());
            push(&mut rows, "longest_streak_start", fmt_date(s.streak_start));
            push(&mut rows, "longest_streak_end", fmt_date(s.streak_end));
            push(&mut rows, "unavailable_videos", s.unavailable.to_string());
            push(&mut rows, "skipped_ads", s.skipped.ads.to_string());
            push(&mut rows, "skipped_music", s.skipped.music.to_string());
            push(
                &mut rows,
                "skipped_out_of_range",
                s.skipped.out_of_range.to_string(),
            );
        }
        Report::Channels => {
            rows.push("rank,channel,videos,share_percent".into());
            for (i, (name, n)) in s.channels.iter().take(top).enumerate() {
                rows.push(csv_row(&[
                    (i + 1).to_string(),
                    name.clone(),
                    n.to_string(),
                    format!("{:.1}", *n as f64 * 100.0 / s.total as f64),
                ]));
            }
        }
        Report::Videos => {
            rows.push("rank,title,channel,videos,url".into());
            for (i, (title, channel, url, n)) in s.videos.iter().take(top).enumerate() {
                rows.push(csv_row(&[
                    (i + 1).to_string(),
                    title.clone(),
                    channel.clone(),
                    n.to_string(),
                    url.clone(),
                ]));
            }
        }
        Report::Months => {
            rows.push("month,videos".into());
            for (m, n) in &s.months {
                rows.push(format!("{m},{n}"));
            }
        }
        Report::Weekdays => {
            rows.push("weekday,videos".into());
            for (i, name) in WEEKDAYS.iter().enumerate() {
                rows.push(format!("{name},{}", s.weekdays[i]));
            }
        }
        Report::Hours => {
            rows.push("hour,videos".into());
            for (h, n) in s.hours.iter().enumerate() {
                rows.push(format!("{h:02},{n}"));
            }
        }
    }
    rows.join("\n")
}

fn channels_json(s: &Stats, top: usize) -> Value {
    Value::Array(
        s.channels
            .iter()
            .take(top)
            .enumerate()
            .map(|(i, (name, n))| {
                json!({
                    "rank": i + 1,
                    "channel": name,
                    "videos": n,
                    "share_percent": (*n as f64 * 1000.0 / s.total as f64).round() / 10.0,
                })
            })
            .collect(),
    )
}

fn videos_json(s: &Stats, top: usize) -> Value {
    Value::Array(
        s.videos
            .iter()
            .take(top)
            .enumerate()
            .map(|(i, (title, channel, url, n))| {
                json!({
                    "rank": i + 1,
                    "title": title,
                    "channel": channel,
                    "url": url,
                    "videos": n,
                })
            })
            .collect(),
    )
}

fn render_json(s: &Stats, report: Report, top: usize) -> String {
    let months = Value::Array(
        s.months
            .iter()
            .map(|(m, n)| json!({ "month": m, "videos": n }))
            .collect(),
    );
    let weekdays = Value::Array(
        WEEKDAYS
            .iter()
            .enumerate()
            .map(|(i, name)| json!({ "weekday": name, "videos": s.weekdays[i] }))
            .collect(),
    );
    let hours = Value::Array(
        s.hours
            .iter()
            .enumerate()
            .map(|(h, n)| json!({ "hour": format!("{h:02}"), "videos": n }))
            .collect(),
    );
    let value = match report {
        Report::Overview => {
            let span = s.last_day - s.first_day + 1;
            let (bday, bcount) = busiest_day(s);
            json!({
                "source_format": s.source,
                "totals": {
                    "videos": s.total,
                    "unique_videos": s.unique_videos,
                    "unique_channels": s.unique_channels,
                    "first_watch": fmt_date(s.first_day),
                    "last_watch": fmt_date(s.last_day),
                    "span_days": span,
                    "active_days": s.active_days,
                    "avg_per_active_day": (s.total as f64 * 100.0 / s.active_days as f64).round() / 100.0,
                    "avg_per_day": (s.total as f64 * 100.0 / span as f64).round() / 100.0,
                    "unavailable_videos": s.unavailable,
                },
                "patterns": {
                    "peak_hour": format!("{:02}", peak_hour(s)),
                    "busiest_weekday": WEEKDAYS[busiest_weekday(s)],
                    "busiest_day": fmt_date(bday),
                    "busiest_day_videos": bcount,
                    "longest_streak_days": s.streak_len,
                    "longest_streak_start": fmt_date(s.streak_start),
                    "longest_streak_end": fmt_date(s.streak_end),
                },
                "skipped": {
                    "ads": s.skipped.ads,
                    "youtube_music": s.skipped.music,
                    "out_of_range": s.skipped.out_of_range,
                },
                "top_channels": channels_json(s, top),
                "top_videos": videos_json(s, top),
                "months": months,
                "weekdays": weekdays,
                "hours": hours,
            })
        }
        Report::Channels => json!({ "channels": channels_json(s, top) }),
        Report::Videos => json!({ "videos": videos_json(s, top) }),
        Report::Months => json!({ "months": months }),
        Report::Weekdays => json!({ "weekdays": weekdays }),
        Report::Hours => json!({ "hours": hours }),
    };
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Parse a Takeout watch-history export and render the requested statistics.
///
/// * `input` — the contents of `watch-history.json` or `watch-history.html`.
/// * `output` — `text` | `csv` | `json`.
/// * `report` — `overview` | `channels` | `videos` | `months` | `weekdays` | `hours`.
/// * `top` — how many rows the channel/video rankings keep (1–100).
/// * `utc_offset` — hours added to the JSON export's UTC timestamps before
///   bucketing into local days/hours. Not applied to HTML exports, whose
///   timestamps are already local.
/// * `include_ads` / `include_music` — keep ad impressions / YouTube Music rows.
/// * `start_date` / `end_date` — inclusive `YYYY-MM-DD` bounds; empty = unbounded.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    output: &str,
    report: &str,
    top: f64,
    utc_offset: f64,
    include_ads: bool,
    include_music: bool,
    start_date: &str,
    end_date: &str,
) -> Result<String, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the maximum is {MAX_INPUT_BYTES} bytes (~24 MB)",
            input.len()
        ));
    }
    let fmt = OutputFormat::parse(output)?;
    let rep = Report::parse(report)?;
    if !(1.0..=100.0).contains(&top) {
        return Err(format!(
            "invalid top {top}: expected a whole number between 1 and 100"
        ));
    }
    let top = top.round() as usize;
    if !(-14.0..=14.0).contains(&utc_offset) {
        return Err(format!(
            "invalid utc_offset {utc_offset}: expected hours between -14 and 14"
        ));
    }
    let offset_secs = (utc_offset * 3600.0).round() as i64;
    let from_day = if start_date.trim().is_empty() {
        None
    } else {
        Some(parse_ymd(start_date, "start_date")?)
    };
    let to_day = if end_date.trim().is_empty() {
        None
    } else {
        Some(parse_ymd(end_date, "end_date")?)
    };
    if let (Some(f), Some(t)) = (from_day, to_day) {
        if f > t {
            return Err(format!(
                "start_date {} is after end_date {}",
                start_date.trim(),
                end_date.trim()
            ));
        }
    }

    let (entries, source) = parse_export(input)?;
    if entries.is_empty() {
        return Err(
            "no watch records found: the export held no \"Watched …\" activity rows (is this the search-history or subscriptions file?)"
                .into(),
        );
    }
    let stats = compute(
        entries,
        source,
        offset_secs,
        include_ads,
        include_music,
        from_day,
        to_day,
    )?;

    Ok(match fmt {
        OutputFormat::Text => render_text(&stats, rep, top),
        OutputFormat::Csv => render_csv(&stats, rep, top),
        OutputFormat::Json => render_json(&stats, rep, top),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
      {"header":"YouTube","title":"Watched Never Gonna Give You Up","titleUrl":"https://www.youtube.com/watch?v=dQw4w9WgXcQ","subtitles":[{"name":"Rick Astley"}],"time":"2024-01-01T18:10:00Z","products":["YouTube"]},
      {"header":"YouTube","title":"Watched Never Gonna Give You Up","titleUrl":"https://www.youtube.com/watch?v=dQw4w9WgXcQ","subtitles":[{"name":"Rick Astley"}],"time":"2024-01-01T18:40:00Z","products":["YouTube"]},
      {"header":"YouTube","title":"Watched Rust in 100 Seconds","titleUrl":"https://www.youtube.com/watch?v=5C_HPTJg5ek","subtitles":[{"name":"Fireship"}],"time":"2024-01-02T09:05:00Z","products":["YouTube"]},
      {"header":"YouTube","title":"Watched an ad","titleUrl":"https://www.youtube.com/watch?v=aaaaaaaaaaa","subtitles":[{"name":"Advertiser"}],"time":"2024-01-02T09:06:00Z","details":[{"name":"From Google Ads"}],"products":["YouTube"]},
      {"header":"YouTube Music","title":"Watched Some Song","titleUrl":"https://music.youtube.com/watch?v=bbbbbbbbbbb","subtitles":[{"name":"Some Artist"}],"time":"2024-02-03T22:00:00Z","products":["YouTube Music"]},
      {"header":"YouTube","title":"Watched a video that has been removed","time":"2024-03-04T07:30:00Z","products":["YouTube"]}
    ]"#;

    fn stats_text(report: &str) -> String {
        run(SAMPLE, "text", report, 10.0, 0.0, false, false, "", "").unwrap()
    }

    #[test]
    fn happy_path_overview_counts_videos_and_channels() {
        let out = stats_text("overview");
        assert!(
            out.starts_with("YouTube watch history — 4 videos watched\n"),
            "unexpected header: {out}"
        );
        assert!(
            out.contains("Range: 2024-01-01 → 2024-03-04 (64 days, 3 active)"),
            "{out}"
        );
        assert!(
            out.contains("Unique videos: 3 · Unique channels: 2"),
            "{out}"
        );
        assert!(out.contains("Unavailable (removed or private): 1"), "{out}");
        assert!(out.contains("Skipped: 1 ad, 1 YouTube Music"), "{out}");
        assert!(out.contains("Rick Astley"), "{out}");
    }

    #[test]
    fn channels_csv_is_exact() {
        let out = run(SAMPLE, "csv", "channels", 10.0, 0.0, false, false, "", "").unwrap();
        assert_eq!(
            out,
            "rank,channel,videos,share_percent\n1,Rick Astley,2,50.0\n2,Fireship,1,25.0"
        );
    }

    #[test]
    fn months_csv_fills_gap_months() {
        let out = run(SAMPLE, "csv", "months", 10.0, 0.0, false, false, "", "").unwrap();
        assert_eq!(out, "month,videos\n2024-01,3\n2024-02,0\n2024-03,1");
    }

    #[test]
    fn top_limits_the_ranking() {
        let out = run(SAMPLE, "csv", "channels", 1.0, 0.0, false, false, "", "").unwrap();
        assert_eq!(
            out,
            "rank,channel,videos,share_percent\n1,Rick Astley,2,50.0"
        );
    }

    #[test]
    fn include_music_and_ads_add_rows() {
        let out = run(SAMPLE, "csv", "overview", 10.0, 0.0, true, true, "", "").unwrap();
        assert!(out.contains("total_videos,6"), "{out}");
        assert!(out.contains("skipped_ads,0"), "{out}");
        assert!(out.contains("skipped_music,0"), "{out}");
    }

    #[test]
    fn utc_offset_shifts_the_local_day_and_hour() {
        // 18:10Z on 2024-01-01 becomes 13:10 local at -5.
        let out = run(SAMPLE, "csv", "hours", 10.0, -5.0, false, false, "", "").unwrap();
        assert!(out.contains("\n13,2"), "{out}");
        // 09:05Z on 2024-01-02 becomes 04:05 local.
        assert!(out.contains("\n04,1"), "{out}");
    }

    #[test]
    fn date_range_filters_records() {
        let out = run(
            SAMPLE,
            "csv",
            "overview",
            10.0,
            0.0,
            false,
            false,
            "2024-01-02",
            "2024-01-31",
        )
        .unwrap();
        assert!(out.contains("total_videos,1"), "{out}");
        assert!(out.contains("skipped_out_of_range,3"), "{out}");
    }

    #[test]
    fn weekday_bucketing_is_correct() {
        // 2024-01-01 was a Monday, 2024-01-02 a Tuesday, 2024-03-04 a Monday.
        let out = run(SAMPLE, "csv", "weekdays", 10.0, 0.0, false, false, "", "").unwrap();
        assert_eq!(
            out,
            "weekday,videos\nMon,3\nTue,1\nWed,0\nThu,0\nFri,0\nSat,0\nSun,0"
        );
    }

    #[test]
    fn json_overview_has_every_section() {
        let out = run(SAMPLE, "json", "overview", 5.0, 0.0, false, false, "", "").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["source_format"], "watch-history.json");
        assert_eq!(v["totals"]["videos"], 4);
        assert_eq!(v["patterns"]["busiest_weekday"], "Mon");
        assert_eq!(v["top_channels"][0]["channel"], "Rick Astley");
        assert_eq!(v["months"].as_array().unwrap().len(), 3);
        assert_eq!(v["hours"].as_array().unwrap().len(), 24);
    }

    #[test]
    fn top_videos_counts_repeat_views() {
        let out = run(SAMPLE, "json", "videos", 10.0, 0.0, false, false, "", "").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["videos"][0]["title"], "Never Gonna Give You Up");
        assert_eq!(v["videos"][0]["videos"], 2);
        assert_eq!(v["videos"][0]["channel"], "Rick Astley");
    }

    #[test]
    fn html_export_is_parsed_with_local_timestamps() {
        let html = r#"<html><body>
          <div class="outer-cell">
            <div class="header-cell"><p class="mdl-typography--title">YouTube<br></p></div>
            <div class="content-cell mdl-typography--body-1">Watched&nbsp;<a href="https://www.youtube.com/watch?v=dQw4w9WgXcQ">Never Gonna Give You Up</a><br><a href="https://www.youtube.com/channel/UCabc">Rick Astley</a><br>Jan 1, 2024, 6:10:00 PM PST</div>
          </div>
          <div class="outer-cell">
            <div class="header-cell"><p class="mdl-typography--title">YouTube<br></p></div>
            <div class="content-cell mdl-typography--body-1">Watched&nbsp;<a href="https://www.youtube.com/watch?v=5C_HPTJg5ek">Rust in 100 Seconds</a><br><a href="https://www.youtube.com/channel/UCdef">Fireship</a><br>Jan 2, 2024, 9:05:00 AM PST</div>
          </div>
        </body></html>"#;
        // utc_offset is deliberately non-zero: HTML timestamps must ignore it.
        let out = run(html, "csv", "overview", 10.0, -5.0, false, false, "", "").unwrap();
        assert!(out.contains("source_format,watch-history.html"), "{out}");
        assert!(out.contains("total_videos,2"), "{out}");
        assert!(out.contains("first_watch,2024-01-01"), "{out}");
        assert!(out.contains("peak_hour,09"), "{out}");
        let ch = run(html, "csv", "channels", 10.0, 0.0, false, false, "", "").unwrap();
        assert_eq!(
            ch,
            "rank,channel,videos,share_percent\n1,Fireship,1,50.0\n2,Rick Astley,1,50.0"
        );
    }

    #[test]
    fn longest_streak_is_the_longest_consecutive_run() {
        let json = r#"[
          {"header":"YouTube","title":"Watched A","titleUrl":"https://www.youtube.com/watch?v=a","subtitles":[{"name":"C"}],"time":"2024-01-01T10:00:00Z"},
          {"header":"YouTube","title":"Watched B","titleUrl":"https://www.youtube.com/watch?v=b","subtitles":[{"name":"C"}],"time":"2024-01-05T10:00:00Z"},
          {"header":"YouTube","title":"Watched C","titleUrl":"https://www.youtube.com/watch?v=c","subtitles":[{"name":"C"}],"time":"2024-01-06T10:00:00Z"},
          {"header":"YouTube","title":"Watched D","titleUrl":"https://www.youtube.com/watch?v=d","subtitles":[{"name":"C"}],"time":"2024-01-07T10:00:00Z"}
        ]"#;
        let out = run(json, "csv", "overview", 10.0, 0.0, false, false, "", "").unwrap();
        assert!(out.contains("longest_streak_days,3"), "{out}");
        assert!(out.contains("longest_streak_start,2024-01-05"), "{out}");
        assert!(out.contains("longest_streak_end,2024-01-07"), "{out}");
    }

    #[test]
    fn timestamps_with_offsets_and_fractions_parse() {
        assert_eq!(
            parse_iso8601("2024-01-01T00:00:00Z").unwrap(),
            1_704_067_200
        );
        assert_eq!(
            parse_iso8601("2024-01-01T00:00:00.123Z").unwrap(),
            1_704_067_200
        );
        assert_eq!(
            parse_iso8601("2024-01-01T02:00:00+02:00").unwrap(),
            1_704_067_200
        );
    }

    // ---- error paths ----

    #[test]
    fn invalid_json_is_reported() {
        let err = run("{bad}", "text", "overview", 10.0, 0.0, false, false, "", "").unwrap_err();
        assert!(err.contains("invalid JSON"), "{err}");
    }

    #[test]
    fn unrecognized_input_is_reported() {
        let err = run("hello", "text", "overview", 10.0, 0.0, false, false, "", "").unwrap_err();
        assert!(err.contains("unrecognized export"), "{err}");
    }

    #[test]
    fn empty_input_is_reported() {
        let err = run("   ", "text", "overview", 10.0, 0.0, false, false, "", "").unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn bad_output_and_report_are_reported() {
        let err = run(SAMPLE, "xml", "overview", 10.0, 0.0, false, false, "", "").unwrap_err();
        assert!(err.contains("invalid output"), "{err}");
        let err = run(SAMPLE, "text", "creators", 10.0, 0.0, false, false, "", "").unwrap_err();
        assert!(err.contains("invalid report"), "{err}");
    }

    #[test]
    fn top_out_of_range_is_reported() {
        let err = run(SAMPLE, "text", "channels", 101.0, 0.0, false, false, "", "").unwrap_err();
        assert!(err.contains("between 1 and 100"), "{err}");
        assert!(run(SAMPLE, "text", "channels", 100.0, 0.0, false, false, "", "").is_ok());
    }

    #[test]
    fn bad_dates_are_reported() {
        let err = run(
            SAMPLE, "text", "overview", 10.0, 0.0, false, false, "01/2024", "",
        )
        .unwrap_err();
        assert!(err.contains("invalid start_date"), "{err}");
        let err = run(
            SAMPLE,
            "text",
            "overview",
            10.0,
            0.0,
            false,
            false,
            "2024-06-01",
            "2024-01-01",
        )
        .unwrap_err();
        assert!(err.contains("is after end_date"), "{err}");
    }

    #[test]
    fn a_range_that_excludes_everything_is_reported() {
        let err = run(
            SAMPLE,
            "text",
            "overview",
            10.0,
            0.0,
            false,
            false,
            "2030-01-01",
            "2030-12-31",
        )
        .unwrap_err();
        assert!(err.contains("no watch records matched"), "{err}");
    }

    #[test]
    fn a_non_watch_export_is_reported() {
        let search = r#"[{"header":"YouTube","title":"Searched for rust lang","titleUrl":"https://www.youtube.com/results?search_query=rust","time":"2024-01-01T10:00:00Z"}]"#;
        let err = run(search, "text", "overview", 10.0, 0.0, false, false, "", "").unwrap_err();
        assert!(err.contains("no watch records found"), "{err}");
    }

    #[test]
    fn oversized_input_is_reported() {
        let big = "[".to_string() + &" ".repeat(MAX_INPUT_BYTES);
        let err = run(&big, "text", "overview", 10.0, 0.0, false, false, "", "").unwrap_err();
        assert!(err.contains("the maximum is"), "{err}");
    }
}
