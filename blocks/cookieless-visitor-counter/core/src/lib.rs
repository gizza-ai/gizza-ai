//! cookieless-visitor-counter core — pure, no wafer/wasm-bindgen deps.
//!
//! Counts **unique visitors** in a web access log using the daily-salted-hash
//! method popularised by privacy-first analytics: a visitor is identified by
//! `SHA-256(salt ‖ period_key ‖ identity_material)`, never by a cookie. The
//! period key is mixed **into** the hash input, so an ID computed for one day
//! cannot equal the ID for the same person on the next day — the same
//! un-linkability guarantee a server gets by rotating its salt every 24 h,
//! achieved deterministically so the same log always yields the same numbers.
//!
//! Nothing is stored: raw IPs and user-agents are consumed to build the hash
//! and dropped. Only the truncated hex digest ever reaches the output, and only
//! when the caller explicitly asks for `output = ids`.
//!
//! Inherent limitation (stated on the page): logs cannot distinguish people
//! behind a shared NAT, and a changing mobile IP or user-agent splits one
//! person into several. Every log-based visitor counter shares this.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;

/// Hard cap on input lines — keeps a pathological paste from hanging the tab.
pub const MAX_LINES: usize = 200_000;
/// Hard cap on emitted rows for the row-wise outputs (`table`, `ids`).
pub const MAX_ROWS: usize = 5_000;
/// Default number of hex characters kept from the digest.
pub const DEFAULT_HASH_LENGTH: u32 = 12;
pub const MIN_HASH_LENGTH: u32 = 6;
pub const MAX_HASH_LENGTH: u32 = 64;

/// Used when the caller supplies no salt, so runs stay reproducible. A real
/// deployment should pass its own secret salt.
const BUILTIN_SALT: &str = "gizza-cookieless-visitor-counter/v1";
/// Bucket label for lines that parsed but carried no readable timestamp.
const UNDATED: &str = "(undated)";
/// Bucket label used when `period = total`.
const ALL: &str = "all";

/// How to read each log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Sniff the format from the first non-blank line.
    Auto,
    /// Apache/nginx Combined Log Format — user-agent is the last quoted field.
    Combined,
    /// Apache/nginx Common Log Format — no user-agent field at all.
    Common,
    /// One JSON object per line (NDJSON), with aliased ip/ua/time keys.
    Json,
    /// CSV with a header row naming the ip/user-agent/timestamp columns.
    Csv,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Format::Auto,
            "combined" => Format::Combined,
            "common" | "clf" => Format::Common,
            "json" | "ndjson" => Format::Json,
            "csv" => Format::Csv,
            other => {
                return Err(format!(
                    "unknown format '{other}' — expected auto, combined, common, json, or csv"
                ))
            }
        })
    }
}

/// What goes into the visitor hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    /// IP + user-agent (Plausible / GoAccess convention). The default.
    IpUa,
    /// IP only (AWStats convention) — coarser, merges browsers on one machine.
    Ip,
    /// Truncated network (IPv4 → /24, IPv6 → /48) + user-agent (Matomo/GA-style
    /// IP anonymisation applied *before* identification).
    NetworkUa,
}

impl Identity {
    pub fn parse(s: &str) -> Result<Identity, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "ip_ua" => Identity::IpUa,
            "ip" => Identity::Ip,
            "network_ua" => Identity::NetworkUa,
            other => {
                return Err(format!(
                    "unknown identity '{other}' — expected ip_ua, ip, or network_ua"
                ))
            }
        })
    }
}

/// The salt-rotation and bucketing window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Hour,
    Day,
    Month,
    Total,
}

impl Period {
    pub fn parse(s: &str) -> Result<Period, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "hour" => Period::Hour,
            "" | "day" | "daily" => Period::Day,
            "month" => Period::Month,
            "total" | "all" => Period::Total,
            other => {
                return Err(format!(
                    "unknown period '{other}' — expected hour, day, month, or total"
                ))
            }
        })
    }

    fn label(self) -> &'static str {
        match self {
            Period::Hour => "Hour",
            Period::Day => "Date",
            Period::Month => "Month",
            Period::Total => "Period",
        }
    }
}

/// What to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Report,
    Table,
    Json,
    Csv,
    Ids,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" => Output::Report,
            "table" => Output::Table,
            "json" => Output::Json,
            "csv" => Output::Csv,
            "ids" => Output::Ids,
            other => {
                return Err(format!(
                    "unknown output '{other}' — expected report, table, json, csv, or ids"
                ))
            }
        })
    }
}

/// One parsed request: the raw identity material plus its bucket.
struct Hit {
    bucket: String,
    ip: String,
    ua: String,
}

/// Curated crawler tokens, matched case-insensitively as substrings. Declared
/// user-agent only — no reverse DNS or IP-range verification (stated on page).
const BOT_TOKENS: &[&str] = &[
    "bot", "crawl", "spider", "slurp", "googlebot", "bingbot", "yandex", "baiduspider",
    "duckduckbot", "gptbot", "oai-searchbot", "chatgpt-user", "claudebot", "claude-web",
    "anthropic-ai", "perplexitybot", "ccbot", "bytespider", "amazonbot", "applebot",
    "google-extended", "meta-externalagent", "ahrefs", "semrush", "mj12", "dotbot", "petalbot",
    "screaming frog", "curl/", "wget", "python-requests", "python-urllib", "go-http-client",
    "okhttp", "java/", "libwww-perl", "axios/", "node-fetch", "headlesschrome", "phantomjs",
    "puppeteer", "playwright", "uptimerobot", "pingdom", "statuscake", "site24x7", "newrelic",
    "facebookexternalhit", "twitterbot", "slackbot", "discordbot", "telegrambot", "whatsapp",
    "linkedinbot", "embedly", "scrapy", "httpclient", "zgrab", "masscan",
];

fn is_bot(ua: &str) -> bool {
    let ua = ua.trim();
    if ua.is_empty() || ua == "-" {
        // A hit with no declared user-agent is a script far more often than a
        // browser; treated as a bot when exclusion is on.
        return true;
    }
    let lower = ua.to_ascii_lowercase();
    BOT_TOKENS.iter().any(|t| lower.contains(t))
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// Normalise a timestamp to `(YYYY-MM-DD, HH)`, or `None` if unreadable.
///
/// Handles the CLF form `10/Aug/2026:13:55:36 +0000` and the ISO-8601 family
/// `2026-08-10T13:55:36Z` / `2026-08-10 13:55:36` (and a bare `2026-08-10`).
fn parse_stamp(raw: &str) -> Option<(String, String)> {
    let s = raw.trim().trim_start_matches('[').trim_end_matches(']').trim();
    let bytes = s.as_bytes();

    // ISO 8601: YYYY-MM-DD[(T| )HH:...]
    if bytes.len() >= 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
    {
        let date = s[0..10].to_string();
        let hour = if bytes.len() >= 13
            && (bytes[10] == b'T' || bytes[10] == b' ')
            && bytes[11..13].iter().all(u8::is_ascii_digit)
        {
            s[11..13].to_string()
        } else {
            "00".to_string()
        };
        return Some((date, hour));
    }

    // CLF: DD/Mon/YYYY:HH:MM:SS +ZZZZ
    if bytes.len() >= 11 && bytes[2] == b'/' && bytes[6] == b'/' {
        let day = &s[0..2];
        let mon = s[3..6].to_ascii_lowercase();
        let year = &s[7..11];
        if day.bytes().all(|b| b.is_ascii_digit()) && year.bytes().all(|b| b.is_ascii_digit()) {
            let m = MONTHS.iter().position(|x| *x == mon)? + 1;
            let hour = if bytes.len() >= 14 && bytes[11] == b':' {
                s[12..14].to_string()
            } else {
                "00".to_string()
            };
            if !hour.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            return Some((format!("{year}-{m:02}-{day}"), hour));
        }
    }
    None
}

fn bucket_of(stamp: Option<(String, String)>, period: Period) -> String {
    match period {
        Period::Total => ALL.to_string(),
        _ => match stamp {
            None => UNDATED.to_string(),
            Some((date, hour)) => match period {
                Period::Hour => format!("{date} {hour}:00"),
                Period::Day => date,
                Period::Month => date[0..7].to_string(),
                Period::Total => unreachable!(),
            },
        },
    }
}

/// Split a line on double quotes → the unquoted head plus each quoted field.
fn quoted_fields(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(a) = rest.find('"') {
        rest = &rest[a + 1..];
        match rest.find('"') {
            Some(b) => {
                out.push(&rest[..b]);
                rest = &rest[b + 1..];
            }
            None => break,
        }
    }
    out
}

/// Parse one Apache/nginx access-log line: leading IP, bracketed timestamp,
/// and (Combined only) the last quoted field as the user-agent.
fn parse_clf(line: &str, with_ua: bool) -> Option<(String, Option<(String, String)>, String)> {
    let ip = line.split_whitespace().next()?.to_string();
    if !looks_like_ip(&ip) {
        return None;
    }
    let stamp = line
        .find('[')
        .and_then(|a| line[a..].find(']').map(|b| &line[a + 1..a + b]))
        .and_then(parse_stamp);
    let ua = if with_ua {
        quoted_fields(line).last().map(|s| s.to_string()).unwrap_or_default()
    } else {
        String::new()
    };
    Some((ip, stamp, ua))
}

/// Cheap shape check — enough to reject a prose line, not a full validator.
fn looks_like_ip(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let v4 = s.split('.').count() == 4 && s.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    let v6 = s.contains(':') && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b':');
    v4 || v6
}

/// Truncate an address to its network: IPv4 → /24, IPv6 → /48.
fn to_network(ip: &str) -> String {
    if ip.contains(':') {
        let groups: Vec<&str> = ip.split(':').collect();
        let kept: Vec<&str> = groups.into_iter().take(3).collect();
        format!("{}::/48", kept.join(":"))
    } else {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() == 4 {
            format!("{}.{}.{}.0/24", parts[0], parts[1], parts[2])
        } else {
            ip.to_string()
        }
    }
}

/// Pull a string value for the first matching key out of a flat JSON object.
fn json_field(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<String> {
    for k in keys {
        for (key, val) in obj.iter() {
            if key.eq_ignore_ascii_case(k) {
                return match val {
                    serde_json::Value::String(s) => Some(s.clone()),
                    serde_json::Value::Null => None,
                    other => Some(other.to_string()),
                };
            }
        }
    }
    None
}

const IP_KEYS: &[&str] = &[
    "ip", "remote_addr", "client_ip", "remote_ip", "c_ip", "clientip", "src_ip", "address",
];
const UA_KEYS: &[&str] = &[
    "user_agent", "useragent", "ua", "http_user_agent", "agent", "user-agent",
];
const TIME_KEYS: &[&str] = &[
    "time", "timestamp", "time_local", "@timestamp", "date", "datetime", "time_iso8601",
];

/// Split one CSV record, honouring double-quoted fields with `""` escapes.
fn csv_fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).collect()
}

fn sniff(line: &str) -> Format {
    let t = line.trim_start();
    if t.starts_with('{') {
        return Format::Json;
    }
    if t.contains('[') && t.contains(']') && looks_like_ip(t.split_whitespace().next().unwrap_or(""))
    {
        // Combined carries ≥2 quoted fields (request + referer + UA); Common has 1.
        return if quoted_fields(t).len() >= 2 {
            Format::Combined
        } else {
            Format::Common
        };
    }
    if t.contains(',') {
        return Format::Csv;
    }
    Format::Combined
}

/// A per-period tally: distinct visitor IDs plus the raw request count.
#[derive(Default)]
struct Bucket {
    visitors: HashSet<String>,
    pageviews: u64,
}

struct Counted {
    /// Period label → tally, kept in sorted order for stable output.
    buckets: BTreeMap<String, Bucket>,
    /// Distinct IDs over the WHOLE log at the chosen granularity's material,
    /// i.e. what a single un-rotated salt would have produced.
    overall: HashSet<String>,
    /// (bucket, visitor_id) per parsed request, for `output = ids`.
    rows: Vec<(String, String)>,
    parsed: u64,
    skipped: u64,
    bots: u64,
    detected: Format,
}

/// Count unique visitors in `input`.
///
/// Returns the rendered output for `output`, or a human-readable error naming
/// what was expected.
#[allow(clippy::too_many_arguments)]
pub fn count(
    input: &str,
    format: &str,
    identity: &str,
    period: &str,
    salt: &str,
    exclude_bots: bool,
    hash_length: u32,
    output: &str,
) -> Result<String, String> {
    let format = Format::parse(format)?;
    let identity = Identity::parse(identity)?;
    let period = Period::parse(period)?;
    let output = Output::parse(output)?;
    let hash_length = if hash_length == 0 {
        DEFAULT_HASH_LENGTH
    } else {
        hash_length
    };
    if !(MIN_HASH_LENGTH..=MAX_HASH_LENGTH).contains(&hash_length) {
        return Err(format!(
            "hash_length must be between {MIN_HASH_LENGTH} and {MAX_HASH_LENGTH} — got {hash_length}"
        ));
    }
    if input.trim().is_empty() {
        return Err("no log supplied — paste at least one access-log line".to_string());
    }

    let counted = tally(input, format, identity, period, salt, exclude_bots, hash_length)?;

    if counted.parsed == 0 {
        return Err(format!(
            "no readable requests found in {} line(s) — expected an IP as the first field \
             (Combined/Common log), a JSON object per line, or a CSV with an ip column",
            counted.skipped
        ));
    }

    Ok(match output {
        Output::Report => render_report(&counted, period, identity, exclude_bots),
        Output::Table => render_table(&counted, period),
        Output::Json => render_json(&counted, period, identity, exclude_bots),
        Output::Csv => render_csv(&counted, period),
        Output::Ids => render_ids(&counted, period),
    })
}

#[allow(clippy::too_many_arguments)]
fn tally(
    input: &str,
    format: Format,
    identity: Identity,
    period: Period,
    salt: &str,
    exclude_bots: bool,
    hash_length: u32,
) -> Result<Counted, String> {
    let salt = if salt.trim().is_empty() {
        BUILTIN_SALT
    } else {
        salt.trim()
    };

    let lines: Vec<&str> = input.lines().collect();
    if lines.len() > MAX_LINES {
        return Err(format!(
            "log has {} lines — the limit is {MAX_LINES}; split it and run the parts separately",
            lines.len()
        ));
    }

    let first = lines.iter().find(|l| !l.trim().is_empty()).copied().unwrap_or("");
    let detected = match format {
        Format::Auto => sniff(first),
        other => other,
    };

    // CSV needs its header row resolved to column indices up front.
    let mut csv_cols: Option<(usize, Option<usize>, Option<usize>)> = None;
    let mut body_start = 0usize;
    if detected == Format::Csv {
        let header_idx = lines
            .iter()
            .position(|l| !l.trim().is_empty())
            .ok_or_else(|| "CSV input is empty".to_string())?;
        let header = csv_fields(lines[header_idx]);
        let find = |names: &[&str]| {
            header
                .iter()
                .position(|h| names.iter().any(|n| h.eq_ignore_ascii_case(n)))
        };
        let ip_col = find(IP_KEYS).ok_or_else(|| {
            format!(
                "CSV header has no IP column — expected one of {} but found: {}",
                IP_KEYS.join(", "),
                header.join(", ")
            )
        })?;
        csv_cols = Some((ip_col, find(UA_KEYS), find(TIME_KEYS)));
        body_start = header_idx + 1;
    }

    let mut c = Counted {
        buckets: BTreeMap::new(),
        overall: HashSet::new(),
        rows: Vec::new(),
        parsed: 0,
        skipped: 0,
        bots: 0,
        detected,
    };

    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if detected == Format::Csv && i < body_start {
            continue;
        }
        let hit = match detected {
            Format::Combined | Format::Auto => parse_clf(line, true),
            Format::Common => parse_clf(line, false),
            Format::Json => serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|v| v.as_object().cloned())
                .and_then(|obj| {
                    let ip = json_field(&obj, IP_KEYS)?;
                    let ua = json_field(&obj, UA_KEYS).unwrap_or_default();
                    let stamp = json_field(&obj, TIME_KEYS).and_then(|t| parse_stamp(&t));
                    Some((ip, stamp, ua))
                }),
            Format::Csv => {
                let (ip_col, ua_col, time_col) = csv_cols.expect("csv columns resolved above");
                let f = csv_fields(line);
                f.get(ip_col).filter(|s| !s.is_empty()).map(|ip| {
                    let ua = ua_col.and_then(|k| f.get(k)).cloned().unwrap_or_default();
                    let stamp = time_col.and_then(|k| f.get(k)).and_then(|t| parse_stamp(t));
                    (ip.clone(), stamp, ua)
                })
            }
        };

        let Some((ip, stamp, ua)) = hit else {
            c.skipped += 1;
            continue;
        };

        if exclude_bots && is_bot(&ua) {
            c.bots += 1;
            continue;
        }

        let bucket = bucket_of(stamp, period);
        let hit = Hit { bucket, ip, ua };
        let id = visitor_id(&hit, identity, salt, hash_length);

        c.parsed += 1;
        c.overall.insert(visitor_id_unrotated(&hit, identity, salt, hash_length));
        let entry = c.buckets.entry(hit.bucket.clone()).or_default();
        entry.pageviews += 1;
        entry.visitors.insert(id.clone());
        if c.rows.len() < MAX_ROWS {
            c.rows.push((hit.bucket, id));
        }
    }

    Ok(c)
}

/// The identity material for a hit, under the chosen identity mode.
fn material(hit: &Hit, identity: Identity) -> String {
    match identity {
        Identity::IpUa => format!("{}\u{1f}{}", hit.ip, hit.ua),
        Identity::Ip => hit.ip.clone(),
        Identity::NetworkUa => format!("{}\u{1f}{}", to_network(&hit.ip), hit.ua),
    }
}

fn digest(parts: &[&str], hash_length: u32) -> String {
    let mut h = Sha256::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            h.update([0x1e]); // record separator — no field can forge a boundary
        }
        h.update(p.as_bytes());
    }
    let hex = format!("{:x}", h.finalize());
    hex[..hash_length as usize].to_string()
}

/// `SHA-256(salt ‖ period_key ‖ identity)` — the rotating ID. Because the
/// bucket label is inside the hash, the same person gets an unrelated ID in
/// every period, exactly as a server-side daily salt rotation would give.
fn visitor_id(hit: &Hit, identity: Identity, salt: &str, hash_length: u32) -> String {
    digest(&[salt, &hit.bucket, &material(hit, identity)], hash_length)
}

/// The same hash WITHOUT the period key — used only to report how many
/// distinct people the log holds overall, which daily uniques never sum to.
fn visitor_id_unrotated(hit: &Hit, identity: Identity, salt: &str, hash_length: u32) -> String {
    digest(&[salt, &material(hit, identity)], hash_length)
}

fn identity_label(identity: Identity) -> &'static str {
    match identity {
        Identity::IpUa => "IP + user-agent",
        Identity::Ip => "IP only",
        Identity::NetworkUa => "IP network (/24, /48) + user-agent",
    }
}

fn format_label(f: Format) -> &'static str {
    match f {
        Format::Auto => "auto",
        Format::Combined => "Combined log",
        Format::Common => "Common log",
        Format::Json => "JSON lines",
        Format::Csv => "CSV",
    }
}

fn per_visitor(pageviews: u64, visitors: usize) -> String {
    if visitors == 0 {
        "0.00".to_string()
    } else {
        format!("{:.2}", pageviews as f64 / visitors as f64)
    }
}

fn totals(c: &Counted) -> (usize, u64) {
    let daily_sum: usize = c.buckets.values().map(|b| b.visitors.len()).sum();
    let pageviews: u64 = c.buckets.values().map(|b| b.pageviews).sum();
    (daily_sum, pageviews)
}

fn render_report(c: &Counted, period: Period, identity: Identity, exclude_bots: bool) -> String {
    let (sum_uniques, pageviews) = totals(c);
    let mut s = String::new();
    s.push_str("Cookieless visitor count\n========================\n");
    let _ = writeln!(s, "Method:    daily-salted-hash (SHA-256), no cookies, no PII stored");
    let _ = writeln!(s, "Identity:  {}", identity_label(identity));
    let _ = writeln!(s, "Bucket:    {}", period_word(period));
    let _ = writeln!(s, "Format:    {}", format_label(c.detected));
    s.push('\n');

    let label = period.label();
    let width = c
        .buckets
        .keys()
        .map(|k| k.len())
        .chain(std::iter::once(label.len()))
        .max()
        .unwrap_or(4);
    let _ = writeln!(s, "{label:<width$}  Visitors  Pageviews  Views/visitor");
    let _ = writeln!(s, "{}  --------  ---------  -------------", "-".repeat(width));
    for (k, b) in &c.buckets {
        let _ = writeln!(
            s,
            "{k:<width$}  {:>8}  {:>9}  {:>13}",
            b.visitors.len(),
            b.pageviews,
            per_visitor(b.pageviews, b.visitors.len())
        );
    }

    s.push('\n');
    let _ = writeln!(s, "Total pageviews:        {pageviews}");
    let _ = writeln!(s, "Sum of {} uniques:  {sum_uniques}", period_word(period));
    let _ = writeln!(
        s,
        "Distinct visitors:      {} (across the whole log)",
        c.overall.len()
    );
    let _ = writeln!(s, "Requests parsed:        {}", c.parsed);
    if exclude_bots {
        let _ = writeln!(s, "Bot hits excluded:      {}", c.bots);
    }
    if c.skipped > 0 {
        let _ = writeln!(s, "Unreadable lines:       {}", c.skipped);
    }
    if sum_uniques != c.overall.len() {
        s.push('\n');
        let _ = writeln!(
            s,
            "Note: per-{} IDs are un-linkable across periods by design, so the",
            period_word(period)
        );
        let _ = writeln!(
            s,
            "sum above double-counts anyone who returned. {} people are distinct.",
            c.overall.len()
        );
    }
    s.trim_end().to_string()
}

fn period_word(period: Period) -> &'static str {
    match period {
        Period::Hour => "hourly",
        Period::Day => "daily",
        Period::Month => "monthly",
        Period::Total => "whole-log",
    }
}

fn render_table(c: &Counted, period: Period) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "| {} | Visitors | Pageviews | Views/visitor |", period.label());
    s.push_str("| --- | --- | --- | --- |\n");
    for (k, b) in c.buckets.iter().take(MAX_ROWS) {
        let _ = writeln!(
            s,
            "| {k} | {} | {} | {} |",
            b.visitors.len(),
            b.pageviews,
            per_visitor(b.pageviews, b.visitors.len())
        );
    }
    s.trim_end().to_string()
}

fn render_csv(c: &Counted, period: Period) -> String {
    let mut s = String::new();
    let head = match period {
        Period::Hour => "hour",
        Period::Day => "date",
        Period::Month => "month",
        Period::Total => "period",
    };
    let _ = writeln!(s, "{head},visitors,pageviews,views_per_visitor");
    for (k, b) in c.buckets.iter().take(MAX_ROWS) {
        let _ = writeln!(
            s,
            "{k},{},{},{}",
            b.visitors.len(),
            b.pageviews,
            per_visitor(b.pageviews, b.visitors.len())
        );
    }
    s.trim_end().to_string()
}

fn render_json(c: &Counted, period: Period, identity: Identity, exclude_bots: bool) -> String {
    let (sum_uniques, pageviews) = totals(c);
    let periods: Vec<serde_json::Value> = c
        .buckets
        .iter()
        .map(|(k, b)| {
            serde_json::json!({
                "period": k,
                "visitors": b.visitors.len(),
                "pageviews": b.pageviews,
                "views_per_visitor": per_visitor(b.pageviews, b.visitors.len()).parse::<f64>().unwrap_or(0.0),
            })
        })
        .collect();
    let v = serde_json::json!({
        "method": "daily-salted-hash",
        "algorithm": "sha256",
        "identity": identity_label(identity),
        "bucket": period_word(period),
        "format": format_label(c.detected),
        "periods": periods,
        "totals": {
            "pageviews": pageviews,
            "sum_of_period_uniques": sum_uniques,
            "distinct_visitors": c.overall.len(),
            "requests_parsed": c.parsed,
            "bot_hits_excluded": if exclude_bots { c.bots } else { 0 },
            "unreadable_lines": c.skipped,
        }
    });
    serde_json::to_string_pretty(&v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn render_ids(c: &Counted, period: Period) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "| # | {} | Visitor ID |", period.label());
    s.push_str("| --- | --- | --- |\n");
    for (i, (bucket, id)) in c.rows.iter().enumerate() {
        let _ = writeln!(s, "| {} | {bucket} | {id} |", i + 1);
    }
    if c.parsed as usize > c.rows.len() {
        let _ = writeln!(
            s,
            "\n{} more request(s) not shown (row cap {MAX_ROWS}).",
            c.parsed as usize - c.rows.len()
        );
    }
    s.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = concat!(
        r#"1.1.1.1 - - [06/Aug/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 12 "-" "Mozilla/5.0 Chrome/125.0""#,
        "\n",
        r#"1.1.1.1 - - [06/Aug/2026:10:05:00 +0000] "GET /a HTTP/1.1" 200 12 "-" "Mozilla/5.0 Chrome/125.0""#,
        "\n",
        r#"2.2.2.2 - - [06/Aug/2026:11:00:00 +0000] "GET / HTTP/1.1" 200 12 "-" "Mozilla/5.0 Safari/604.1""#,
        "\n",
        r#"1.1.1.1 - - [07/Aug/2026:09:00:00 +0000] "GET / HTTP/1.1" 200 12 "-" "Mozilla/5.0 Chrome/125.0""#,
    );

    fn json_of(out: &str) -> serde_json::Value {
        serde_json::from_str(out).expect("valid json")
    }

    #[test]
    fn counts_daily_uniques_and_pageviews() {
        let out = count(LOG, "auto", "ip_ua", "day", "", true, 12, "json").unwrap();
        let v = json_of(&out);
        assert_eq!(v["periods"][0]["period"], "2026-08-06");
        assert_eq!(v["periods"][0]["visitors"], 2);
        assert_eq!(v["periods"][0]["pageviews"], 3);
        assert_eq!(v["periods"][1]["period"], "2026-08-07");
        assert_eq!(v["periods"][1]["visitors"], 1);
        // 1.1.1.1 returned on day two: daily uniques sum to 3, distinct is 2.
        assert_eq!(v["totals"]["sum_of_period_uniques"], 3);
        assert_eq!(v["totals"]["distinct_visitors"], 2);
        assert_eq!(v["totals"]["pageviews"], 4);
    }

    #[test]
    fn ids_rotate_across_days_for_the_same_visitor() {
        let out = count(LOG, "combined", "ip_ua", "day", "s3cret", true, 16, "ids").unwrap();
        let ids: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with("| ") && l.contains("2026-08-"))
            .map(|l| l.rsplit('|').nth(1).unwrap().trim())
            .collect();
        assert_eq!(ids.len(), 4);
        // Same visitor, same day → identical ID.
        assert_eq!(ids[0], ids[1]);
        // Same visitor, next day → a different ID (the rotation guarantee).
        assert_ne!(ids[0], ids[3]);
        assert!(ids.iter().all(|i| i.len() == 16));
    }

    #[test]
    fn salt_changes_every_id() {
        let a = count(LOG, "combined", "ip_ua", "day", "salt-a", true, 12, "ids").unwrap();
        let b = count(LOG, "combined", "ip_ua", "day", "salt-b", true, 12, "ids").unwrap();
        assert_ne!(a, b, "a different salt must yield different visitor IDs");
    }

    #[test]
    fn ip_only_identity_merges_user_agents() {
        let log = concat!(
            r#"9.9.9.9 - - [06/Aug/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 1 "-" "Mozilla/5.0 Chrome/125.0""#,
            "\n",
            r#"9.9.9.9 - - [06/Aug/2026:10:01:00 +0000] "GET / HTTP/1.1" 200 1 "-" "Mozilla/5.0 Firefox/130.0""#,
        );
        let ip_ua = json_of(&count(log, "combined", "ip_ua", "day", "", true, 12, "json").unwrap());
        let ip = json_of(&count(log, "combined", "ip", "day", "", true, 12, "json").unwrap());
        assert_eq!(ip_ua["periods"][0]["visitors"], 2);
        assert_eq!(ip["periods"][0]["visitors"], 1);
    }

    #[test]
    fn network_identity_merges_a_24() {
        let log = concat!(
            r#"9.9.9.1 - - [06/Aug/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 1 "-" "UA/1""#,
            "\n",
            r#"9.9.9.2 - - [06/Aug/2026:10:01:00 +0000] "GET / HTTP/1.1" 200 1 "-" "UA/1""#,
        );
        let v = json_of(&count(log, "combined", "network_ua", "day", "", true, 12, "json").unwrap());
        assert_eq!(v["periods"][0]["visitors"], 1, "same /24 + same UA is one visitor");
    }

    #[test]
    fn excludes_bots_by_default_and_keeps_them_when_off() {
        let log = concat!(
            r#"1.1.1.1 - - [06/Aug/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 1 "-" "Mozilla/5.0 Chrome/125.0""#,
            "\n",
            r#"66.249.66.1 - - [06/Aug/2026:10:01:00 +0000] "GET / HTTP/1.1" 200 1 "-" "Googlebot/2.1""#,
        );
        let on = json_of(&count(log, "combined", "ip_ua", "day", "", true, 12, "json").unwrap());
        let off = json_of(&count(log, "combined", "ip_ua", "day", "", false, 12, "json").unwrap());
        assert_eq!(on["periods"][0]["visitors"], 1);
        assert_eq!(on["totals"]["bot_hits_excluded"], 1);
        assert_eq!(off["periods"][0]["visitors"], 2);
    }

    #[test]
    fn hourly_and_monthly_buckets() {
        let hourly = json_of(&count(LOG, "combined", "ip_ua", "hour", "", true, 12, "json").unwrap());
        assert_eq!(hourly["periods"][0]["period"], "2026-08-06 10:00");
        assert_eq!(hourly["periods"][0]["visitors"], 1);
        let monthly =
            json_of(&count(LOG, "combined", "ip_ua", "month", "", true, 12, "json").unwrap());
        assert_eq!(monthly["periods"][0]["period"], "2026-08");
        assert_eq!(monthly["periods"][0]["visitors"], 2);
    }

    #[test]
    fn total_period_collapses_to_one_bucket() {
        let v = json_of(&count(LOG, "combined", "ip_ua", "total", "", true, 12, "json").unwrap());
        assert_eq!(v["periods"][0]["period"], "all");
        assert_eq!(v["periods"][0]["visitors"], 2);
        assert_eq!(v["totals"]["distinct_visitors"], 2);
    }

    #[test]
    fn parses_json_lines() {
        let log = concat!(
            r#"{"remote_addr":"1.1.1.1","time_local":"2026-08-06T10:00:00Z","http_user_agent":"Mozilla/5.0 Chrome/125.0"}"#,
            "\n",
            r#"{"remote_addr":"2.2.2.2","time_local":"2026-08-06T10:01:00Z","http_user_agent":"Mozilla/5.0 Safari/604.1"}"#,
        );
        let v = json_of(&count(log, "auto", "ip_ua", "day", "", true, 12, "json").unwrap());
        assert_eq!(v["format"], "JSON lines");
        assert_eq!(v["periods"][0]["visitors"], 2);
    }

    #[test]
    fn parses_csv_with_aliased_headers() {
        let log = "timestamp,ip,user_agent\n2026-08-06 10:00:00,1.1.1.1,Mozilla/5.0 Chrome/125.0\n2026-08-06 10:01:00,1.1.1.1,Mozilla/5.0 Chrome/125.0";
        let v = json_of(&count(log, "csv", "ip_ua", "day", "", true, 12, "json").unwrap());
        assert_eq!(v["periods"][0]["visitors"], 1);
        assert_eq!(v["periods"][0]["pageviews"], 2);
    }

    #[test]
    fn common_log_has_no_user_agent_field() {
        let log = concat!(
            r#"1.1.1.1 - - [06/Aug/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 12"#,
            "\n",
            r#"1.1.1.1 - - [06/Aug/2026:10:01:00 +0000] "GET /a HTTP/1.1" 200 12"#,
        );
        // Bots off: a Common line has no UA, which the bot rule would drop.
        let v = json_of(&count(log, "common", "ip_ua", "day", "", false, 12, "json").unwrap());
        assert_eq!(v["periods"][0]["visitors"], 1);
        assert_eq!(v["periods"][0]["pageviews"], 2);
    }

    #[test]
    fn undated_lines_land_in_their_own_bucket() {
        let log = "{\"ip\":\"1.1.1.1\",\"user_agent\":\"Mozilla/5.0 Chrome/125.0\"}";
        let v = json_of(&count(log, "json", "ip_ua", "day", "", true, 12, "json").unwrap());
        assert_eq!(v["periods"][0]["period"], "(undated)");
        assert_eq!(v["periods"][0]["visitors"], 1);
    }

    #[test]
    fn report_renders_the_headline_numbers() {
        let out = count(LOG, "combined", "ip_ua", "day", "", true, 12, "report").unwrap();
        assert!(out.contains("Cookieless visitor count"));
        assert!(out.contains("2026-08-06"));
        assert!(out.contains("Total pageviews:        4"));
        assert!(out.contains("Distinct visitors:      2"));
    }

    #[test]
    fn err_on_empty_input() {
        let e = count("", "auto", "ip_ua", "day", "", true, 12, "report").unwrap_err();
        assert!(e.contains("no log supplied"), "got: {e}");
    }

    #[test]
    fn err_on_unreadable_input() {
        let e = count("hello world\nnot a log", "combined", "ip_ua", "day", "", true, 12, "report")
            .unwrap_err();
        assert!(e.contains("no readable requests"), "got: {e}");
    }

    #[test]
    fn err_on_unknown_enum_values() {
        assert!(count(LOG, "nope", "ip_ua", "day", "", true, 12, "report")
            .unwrap_err()
            .contains("unknown format 'nope'"));
        assert!(count(LOG, "auto", "nope", "day", "", true, 12, "report")
            .unwrap_err()
            .contains("unknown identity 'nope'"));
        assert!(count(LOG, "auto", "ip_ua", "nope", "", true, 12, "report")
            .unwrap_err()
            .contains("unknown period 'nope'"));
        assert!(count(LOG, "auto", "ip_ua", "day", "", true, 12, "nope")
            .unwrap_err()
            .contains("unknown output 'nope'"));
    }

    #[test]
    fn err_on_out_of_range_hash_length() {
        let e = count(LOG, "auto", "ip_ua", "day", "", true, 99, "report").unwrap_err();
        assert!(e.contains("hash_length must be between 6 and 64"), "got: {e}");
    }

    #[test]
    fn err_on_csv_without_an_ip_column() {
        let e = count("when,who\n2026-08-06,bob", "csv", "ip_ua", "day", "", true, 12, "report")
            .unwrap_err();
        assert!(e.contains("no IP column"), "got: {e}");
    }

    #[test]
    fn table_and_csv_outputs_match_the_counts() {
        let t = count(LOG, "combined", "ip_ua", "day", "", true, 12, "table").unwrap();
        assert!(t.starts_with("| Date | Visitors | Pageviews | Views/visitor |"));
        assert!(t.contains("| 2026-08-06 | 2 | 3 | 1.50 |"));
        let c = count(LOG, "combined", "ip_ua", "day", "", true, 12, "csv").unwrap();
        assert_eq!(c.lines().next().unwrap(), "date,visitors,pageviews,views_per_visitor");
        assert!(c.contains("2026-08-06,2,3,1.50"));
    }

    #[test]
    fn ipv6_networks_truncate_to_48() {
        assert_eq!(to_network("2001:db8:1234:5678::1"), "2001:db8:1234::/48");
        assert_eq!(to_network("203.0.113.42"), "203.0.113.0/24");
    }
}
