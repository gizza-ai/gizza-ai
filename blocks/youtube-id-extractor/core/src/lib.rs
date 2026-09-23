//! youtube-id-extractor core — pure compute, shared by the chat skill block and the web page.
//!
//! Turns any YouTube-style link into its canonical 11-character video ID plus the start
//! timestamp encoded in the link. The same path/query rules are applied to EVERY host, so
//! Invidious and Piped front-ends (`yewtu.be`, `piped.video`, `inv.nadeko.net`, …) resolve
//! exactly like `youtube.com` does — competitors hard-code the YouTube hostnames.
//!
//! No network, no clock, no allocation-heavy deps: the whole thing is string parsing, so it
//! behaves identically in chat, in the CLI and in the browser page.

use serde_json::{json, Map, Value};

/// Hard cap on input lines — keeps a pasted log from turning into an unbounded run.
pub const MAX_LINES: usize = 200;

/// How many wrapper URLs (`attribution_link`, `redirect`, `oembed`) we unwrap before giving up.
const MAX_DEPTH: usize = 3;

/// What a single input line resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// The line exactly as the user supplied it (trimmed).
    pub input: String,
    /// `video`, `playlist`, `channel`, `handle`, `user`, `custom`, or `invalid`.
    pub kind: &'static str,
    /// The extracted identifier (video ID, playlist ID, channel ID, handle, or name).
    pub id: String,
    /// Start offset in whole seconds, when the link carries one.
    pub start: Option<u64>,
    /// Playlist the video was linked in, when present.
    pub playlist: Option<String>,
    /// 1-based position within that playlist, when present.
    pub index: Option<u64>,
    /// Why the line could not be resolved (only set when `kind == "invalid"`).
    pub error: Option<String>,
}

impl Record {
    fn invalid(input: &str, why: &str) -> Self {
        Record {
            input: input.to_string(),
            kind: "invalid",
            id: String::new(),
            start: None,
            playlist: None,
            index: None,
            error: Some(why.to_string()),
        }
    }
}

/// Entry point used by every surface.
///
/// * `urls` — one link, ID or handle per line (blank lines ignored).
/// * `format` — `text` | `json` | `csv`.
/// * `timestamp` — `seconds` | `clock` | `both`.
/// * `thumbnail` — `none` | `default` | `mqdefault` | `hqdefault` | `sddefault` | `maxresdefault`.
/// * `canonical` — include the canonical `youtube.com/watch` URL.
/// * `embed` — include the privacy-enhanced `youtube-nocookie.com/embed` URL.
/// * `strict` — fail the whole run if any line cannot be resolved.
pub fn extract(
    urls: &str,
    format: &str,
    timestamp: &str,
    thumbnail: &str,
    canonical: bool,
    embed: bool,
    strict: bool,
) -> Result<String, String> {
    let format = normalize(format, "text");
    let timestamp = normalize(timestamp, "both");
    let thumbnail = normalize(thumbnail, "hqdefault");

    if !matches!(format.as_str(), "text" | "json" | "csv") {
        return Err(format!(
            "unknown format '{format}' — expected text, json or csv"
        ));
    }
    if !matches!(timestamp.as_str(), "seconds" | "clock" | "both") {
        return Err(format!(
            "unknown timestamp '{timestamp}' — expected seconds, clock or both"
        ));
    }
    if !matches!(
        thumbnail.as_str(),
        "none" | "default" | "mqdefault" | "hqdefault" | "sddefault" | "maxresdefault"
    ) {
        return Err(format!(
            "unknown thumbnail '{thumbnail}' — expected none, default, mqdefault, hqdefault, sddefault or maxresdefault"
        ));
    }

    let lines: Vec<&str> = urls
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err("no input — paste at least one YouTube URL, video ID or @handle".into());
    }
    if lines.len() > MAX_LINES {
        return Err(format!(
            "too many lines: {} (max {MAX_LINES}) — split the list and run it in batches",
            lines.len()
        ));
    }

    let records: Vec<Record> = lines.iter().map(|l| parse_one(l)).collect();

    if strict {
        if let Some(bad) = records.iter().find(|r| r.kind == "invalid") {
            return Err(format!(
                "strict mode: could not resolve '{}' — {}",
                bad.input,
                bad.error.as_deref().unwrap_or("no YouTube ID found")
            ));
        }
    }

    let opts = Render {
        timestamp: timestamp.as_str(),
        thumbnail: thumbnail.as_str(),
        canonical,
        embed,
    };
    Ok(match format.as_str() {
        "json" => render_json(&records, &opts),
        "csv" => render_csv(&records, &opts),
        _ => render_text(&records, &opts),
    })
}

fn normalize(v: &str, fallback: &str) -> String {
    let v = v.trim().to_ascii_lowercase();
    if v.is_empty() {
        fallback.to_string()
    } else {
        v
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Resolve one line. Never fails — an unresolvable line becomes an `invalid` record so a
/// bulk run reports every line instead of aborting on the first typo.
pub fn parse_one(raw: &str) -> Record {
    let input = raw.trim().to_string();
    let cleaned = trim_wrappers(&input).to_string();
    let cleaned = cleaned.as_str();
    if cleaned.is_empty() {
        return Record::invalid(&input, "empty line");
    }

    // Bare identifiers, pasted without any URL around them.
    if is_video_id(cleaned) {
        return Record {
            input,
            kind: "video",
            id: cleaned.to_string(),
            start: None,
            playlist: None,
            index: None,
            error: None,
        };
    }
    if let Some(handle) = cleaned.strip_prefix('@') {
        if is_handle(handle) {
            return simple(input, "handle", handle);
        }
    }
    if is_channel_id(cleaned) {
        return simple(input, "channel", cleaned);
    }
    if is_playlist_id(cleaned) {
        return simple(input, "playlist", cleaned);
    }

    match parse_url(cleaned, 0) {
        Some(mut rec) => {
            rec.input = input;
            rec
        }
        None => Record::invalid(
            &input,
            "no YouTube video ID found — expected a watch, youtu.be, shorts, embed, live, Invidious or Piped link, or a bare 11-character ID",
        ),
    }
}

fn simple(input: String, kind: &'static str, id: &str) -> Record {
    Record {
        input,
        kind,
        id: id.to_string(),
        start: None,
        playlist: None,
        index: None,
        error: None,
    }
}

/// Strip the punctuation that survives a copy-paste out of Markdown, chat or prose.
fn trim_wrappers(s: &str) -> &str {
    let s = s.trim_matches(|c| matches!(c, '<' | '>' | '"' | '\'' | '`' | '(' | '[' | ' '));
    s.trim_end_matches(|c| matches!(c, '.' | ',' | ';' | ':' | ')' | ']' | '!' | '?'))
}

fn is_video_id(s: &str) -> bool {
    s.len() == 11 && s.chars().all(is_id_char)
}

fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn is_channel_id(s: &str) -> bool {
    s.len() == 24 && s.starts_with("UC") && s.chars().all(is_id_char)
}

fn is_playlist_id(s: &str) -> bool {
    // Playlist IDs are far longer than a video ID (13+), which is what keeps an 11-character
    // video ID that happens to start with "PL" from being mistaken for a playlist.
    s.len() >= 13
        && s.chars().all(is_id_char)
        && ["PL", "UU", "LL", "FL", "RD", "OL", "TL", "SP"]
            .iter()
            .any(|p| s.starts_with(p))
}

fn is_handle(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 30
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// A URL split into the pieces we care about.
struct Parts {
    host: String,
    segments: Vec<String>,
    query: Vec<(String, String)>,
    fragment: String,
}

fn split_url(raw: &str) -> Option<Parts> {
    let mut rest = raw;
    // Scheme (optional) — `//host/path` and bare `host/path` are both accepted.
    if let Some(i) = rest.find("://") {
        let scheme = &rest[..i].to_ascii_lowercase();
        if !matches!(scheme.as_str(), "http" | "https") {
            return None;
        }
        rest = &rest[i + 3..];
    } else if let Some(stripped) = rest.strip_prefix("//") {
        rest = stripped;
    }

    let (rest, fragment) = match rest.find('#') {
        Some(i) => (&rest[..i], rest[i + 1..].to_string()),
        None => (rest, String::new()),
    };
    let (rest, query) = match rest.find('?') {
        Some(i) => (&rest[..i], rest[i + 1..].to_string()),
        None => (rest, String::new()),
    };

    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    // Drop userinfo and port — neither affects which video a link points at.
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let host = authority
        .split(':')
        .next()
        .unwrap_or(authority)
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }

    let segments: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(percent_decode)
        .collect();

    Some(Parts {
        host,
        segments,
        query: parse_query(&query),
        fragment,
    })
}

fn parse_query(q: &str) -> Vec<(String, String)> {
    q.split(['&', ';'])
        .filter(|p| !p.is_empty())
        .map(|p| match p.find('=') {
            Some(i) => (
                percent_decode(&p[..i]).to_ascii_lowercase(),
                percent_decode(&p[i + 1..]),
            ),
            None => (percent_decode(p).to_ascii_lowercase(), String::new()),
        })
        .collect()
}

fn get<'a>(q: &'a [(String, String)], key: &str) -> Option<&'a str> {
    q.iter()
        .find(|(k, v)| k == key && !v.is_empty())
        .map(|(_, v)| v.as_str())
}

/// Minimal percent-decoder (`+` means space, as in a query string).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    (Some(h), Some(l)) => {
                        out.push(h * 16 + l);
                        i += 3;
                    }
                    _ => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn parse_url(raw: &str, depth: usize) -> Option<Record> {
    if depth > MAX_DEPTH {
        return None;
    }
    let parts = split_url(raw)?;
    let q = &parts.query;

    // Wrapper links — unwrap and re-parse the URL they carry.
    let first = parts.segments.first().map(String::as_str).unwrap_or("");
    let wrapped = match first {
        "attribution_link" => get(q, "u"),
        "redirect" => get(q, "q").or_else(|| get(q, "url")),
        "oembed" => get(q, "url"),
        _ => None,
    };
    if let Some(target) = wrapped {
        let absolute = if target.starts_with('/') {
            format!("https://www.youtube.com{target}")
        } else {
            target.to_string()
        };
        return parse_url(&absolute, depth + 1);
    }

    let short_host = is_short_host(&parts.host);
    let start = timestamp_of(q, &parts.fragment);
    let playlist = get(q, "list").map(str::to_string);
    let index = get(q, "index").and_then(|v| v.parse::<u64>().ok());

    // The video ID, found in whichever place this URL shape puts it.
    let mut id: Option<String> = None;
    match first {
        "watch" | "watch_videos" => {
            id = get(q, "v").or_else(|| get(q, "vi")).map(str::to_string);
        }
        "embed" | "e" | "v" | "vi" | "shorts" | "live" | "w" | "video" => {
            let seg = parts.segments.get(1).cloned();
            // `/embed/videoseries?list=…` is a playlist embed, not a video.
            if seg.as_deref() != Some("videoseries") {
                id = seg;
            }
        }
        "playlist" => {
            if let Some(list) = get(q, "list") {
                return Some(simple(String::new(), "playlist", list));
            }
        }
        "channel" => {
            if let Some(seg) = parts.segments.get(1) {
                return Some(simple(String::new(), "channel", seg));
            }
        }
        "user" => {
            if let Some(seg) = parts.segments.get(1) {
                return Some(simple(String::new(), "user", seg));
            }
        }
        "c" => {
            if let Some(seg) = parts.segments.get(1) {
                return Some(simple(String::new(), "custom", seg));
            }
        }
        _ => {}
    }
    if id.is_none() {
        if let Some(handle) = first.strip_prefix('@') {
            if is_handle(handle) {
                return Some(simple(String::new(), "handle", handle));
            }
        }
    }
    // Query-first fallback: covers `?v=` on any Invidious/Piped path shape.
    if id.is_none() {
        id = get(q, "v").or_else(|| get(q, "vi")).map(str::to_string);
    }
    // Bare-path form: `youtu.be/<id>` and the Invidious/Piped clones that mirror it.
    if id.is_none() && parts.segments.len() == 1 && (short_host || !is_youtube_host(&parts.host)) {
        id = Some(parts.segments[0].clone());
    }

    // No usable video ID? A playlist-only link (`/embed/videoseries?list=…`, a watch URL whose
    // `v` is malformed) still carries something worth reporting.
    if !id.as_deref().map(is_video_id).unwrap_or(false) {
        if let Some(list) = playlist.filter(|l| is_playlist_id(l)) {
            return Some(simple(String::new(), "playlist", &list));
        }
        return None;
    }
    let id = id?;

    Some(Record {
        input: String::new(),
        kind: "video",
        id,
        start,
        playlist,
        index,
        error: None,
    })
}

fn is_youtube_host(host: &str) -> bool {
    let base = host.trim_start_matches("www.");
    matches!(base, "youtube.com" | "youtube-nocookie.com" | "youtu.be")
        || host.ends_with(".youtube.com")
        || host.ends_with(".youtube-nocookie.com")
        || host.ends_with(".youtu.be")
}

fn is_short_host(host: &str) -> bool {
    host == "youtu.be" || host.ends_with(".youtu.be")
}

fn timestamp_of(q: &[(String, String)], fragment: &str) -> Option<u64> {
    for key in ["t", "start", "time_continue", "begin"] {
        if let Some(v) = get(q, key) {
            if let Some(secs) = parse_time(v) {
                return Some(secs);
            }
        }
    }
    if fragment.is_empty() {
        return None;
    }
    // `#t=90`, `#t=1m30s`, or a bare `#90s`.
    let frag = parse_query(fragment);
    for key in ["t", "start"] {
        if let Some(v) = get(&frag, key) {
            if let Some(secs) = parse_time(v) {
                return Some(secs);
            }
        }
    }
    if !fragment.contains('=') {
        return parse_time(fragment);
    }
    None
}

/// Accepts `90`, `90s`, `1m30s`, `1h2m3s`, `1:30` and `1:02:03`.
pub fn parse_time(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() > 3 {
            return None;
        }
        let mut total: u64 = 0;
        for p in parts {
            let n: u64 = p.parse().ok()?;
            total = total.checked_mul(60)?.checked_add(n)?;
        }
        return Some(total);
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        return s.parse().ok();
    }
    let mut total: u64 = 0;
    let mut digits = String::new();
    let mut saw_unit = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        let mul: u64 = match c.to_ascii_lowercase() {
            'h' => 3600,
            'm' => 60,
            's' => 1,
            _ => return None,
        };
        let n: u64 = digits.parse().ok()?;
        digits.clear();
        total = total.checked_add(n.checked_mul(mul)?)?;
        saw_unit = true;
    }
    if !digits.is_empty() {
        total = total.checked_add(digits.parse::<u64>().ok()?)?;
        saw_unit = true;
    }
    if saw_unit {
        Some(total)
    } else {
        None
    }
}

/// `90` → `1:30`; `3723` → `1:02:03`.
pub fn clock(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

// ---------------------------------------------------------------------------
// Derived links
// ---------------------------------------------------------------------------

struct Render<'a> {
    timestamp: &'a str,
    thumbnail: &'a str,
    canonical: bool,
    embed: bool,
}

/// The tidy link to share — a `youtube.com/watch` URL that keeps the playlist and start offset.
pub fn canonical_url(r: &Record) -> Option<String> {
    match r.kind {
        "video" => {
            let mut url = format!("https://www.youtube.com/watch?v={}", r.id);
            if let Some(list) = &r.playlist {
                url.push_str(&format!("&list={list}"));
            }
            if let Some(start) = r.start {
                url.push_str(&format!("&t={start}s"));
            }
            Some(url)
        }
        "playlist" => Some(format!("https://www.youtube.com/playlist?list={}", r.id)),
        "channel" => Some(format!("https://www.youtube.com/channel/{}", r.id)),
        "handle" => Some(format!("https://www.youtube.com/@{}", r.id)),
        "user" => Some(format!("https://www.youtube.com/user/{}", r.id)),
        "custom" => Some(format!("https://www.youtube.com/c/{}", r.id)),
        _ => None,
    }
}

/// Privacy-enhanced embed URL (`youtube-nocookie.com`), carrying playlist and start offset.
pub fn embed_url(r: &Record) -> Option<String> {
    match r.kind {
        "video" => {
            let mut url = format!("https://www.youtube-nocookie.com/embed/{}", r.id);
            let mut sep = '?';
            if let Some(list) = &r.playlist {
                url.push_str(&format!("{sep}list={list}"));
                sep = '&';
            }
            if let Some(start) = r.start {
                url.push_str(&format!("{sep}start={start}"));
            }
            Some(url)
        }
        "playlist" => Some(format!(
            "https://www.youtube-nocookie.com/embed/videoseries?list={}",
            r.id
        )),
        _ => None,
    }
}

/// Thumbnail URL for the chosen variant (video records only).
pub fn thumbnail_url(r: &Record, variant: &str) -> Option<String> {
    if r.kind != "video" || variant == "none" {
        return None;
    }
    Some(format!("https://i.ytimg.com/vi/{}/{variant}.jpg", r.id))
}

fn start_text(start: u64, mode: &str) -> String {
    match mode {
        "seconds" => format!("{start}s"),
        "clock" => clock(start),
        _ => format!("{start}s ({})", clock(start)),
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_text(records: &[Record], o: &Render) -> String {
    let mut blocks: Vec<String> = Vec::with_capacity(records.len() + 1);
    for r in records {
        let mut lines = vec![format!("input: {}", r.input), format!("kind: {}", r.kind)];
        if r.kind == "invalid" {
            lines.push(format!(
                "error: {}",
                r.error.as_deref().unwrap_or("no YouTube ID found")
            ));
            blocks.push(lines.join("\n"));
            continue;
        }
        lines.push(format!("id: {}", r.id));
        if let Some(start) = r.start {
            lines.push(format!("start: {}", start_text(start, o.timestamp)));
        }
        if let Some(list) = &r.playlist {
            lines.push(format!("playlist: {list}"));
        }
        if let Some(i) = r.index {
            lines.push(format!("index: {i}"));
        }
        if o.canonical {
            if let Some(u) = canonical_url(r) {
                lines.push(format!("canonical: {u}"));
            }
        }
        if let Some(u) = thumbnail_url(r, o.thumbnail) {
            lines.push(format!("thumbnail: {u}"));
        }
        if o.embed {
            if let Some(u) = embed_url(r) {
                lines.push(format!("embed: {u}"));
            }
        }
        blocks.push(lines.join("\n"));
    }
    let mut out = blocks.join("\n\n");
    if records.len() > 1 {
        let bad = records.iter().filter(|r| r.kind == "invalid").count();
        out.push_str(&format!(
            "\n\nsummary: {} inputs, {} resolved, {bad} unresolved",
            records.len(),
            records.len() - bad
        ));
    }
    out
}

fn render_json(records: &[Record], o: &Render) -> String {
    let items: Vec<Value> = records
        .iter()
        .map(|r| {
            let mut m = Map::new();
            m.insert("input".into(), json!(r.input));
            m.insert("kind".into(), json!(r.kind));
            if r.kind == "invalid" {
                m.insert(
                    "error".into(),
                    json!(r.error.as_deref().unwrap_or("no YouTube ID found")),
                );
                return Value::Object(m);
            }
            m.insert("id".into(), json!(r.id));
            if let Some(start) = r.start {
                if o.timestamp != "clock" {
                    m.insert("start_seconds".into(), json!(start));
                }
                if o.timestamp != "seconds" {
                    m.insert("start_clock".into(), json!(clock(start)));
                }
            }
            if let Some(list) = &r.playlist {
                m.insert("playlist".into(), json!(list));
            }
            if let Some(i) = r.index {
                m.insert("index".into(), json!(i));
            }
            if o.canonical {
                if let Some(u) = canonical_url(r) {
                    m.insert("canonical".into(), json!(u));
                }
            }
            if let Some(u) = thumbnail_url(r, o.thumbnail) {
                m.insert("thumbnail".into(), json!(u));
            }
            if o.embed {
                if let Some(u) = embed_url(r) {
                    m.insert("embed".into(), json!(u));
                }
            }
            Value::Object(m)
        })
        .collect();
    serde_json::to_string_pretty(&Value::Array(items)).unwrap_or_else(|e| e.to_string())
}

fn render_csv(records: &[Record], o: &Render) -> String {
    let mut headers: Vec<&str> = vec!["input", "kind", "id"];
    if o.timestamp != "clock" {
        headers.push("start_seconds");
    }
    if o.timestamp != "seconds" {
        headers.push("start_clock");
    }
    headers.push("playlist");
    headers.push("index");
    if o.canonical {
        headers.push("canonical");
    }
    if o.thumbnail != "none" {
        headers.push("thumbnail");
    }
    if o.embed {
        headers.push("embed");
    }
    headers.push("error");

    let mut out = headers.join(",");
    for r in records {
        let mut row: Vec<String> = vec![r.input.clone(), r.kind.to_string(), r.id.clone()];
        if o.timestamp != "clock" {
            row.push(r.start.map(|s| s.to_string()).unwrap_or_default());
        }
        if o.timestamp != "seconds" {
            row.push(r.start.map(clock).unwrap_or_default());
        }
        row.push(r.playlist.clone().unwrap_or_default());
        row.push(r.index.map(|i| i.to_string()).unwrap_or_default());
        if o.canonical {
            row.push(canonical_url(r).unwrap_or_default());
        }
        if o.thumbnail != "none" {
            row.push(thumbnail_url(r, o.thumbnail).unwrap_or_default());
        }
        if o.embed {
            row.push(embed_url(r).unwrap_or_default());
        }
        row.push(r.error.clone().unwrap_or_default());
        out.push('\n');
        out.push_str(
            &row.iter()
                .map(|c| csv_cell(c))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    out
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id_of(url: &str) -> String {
        parse_one(url).id
    }

    #[test]
    fn extracts_id_and_timestamp_from_short_link() {
        let out = extract(
            "https://youtu.be/dQw4w9WgXcQ?t=90",
            "text",
            "both",
            "hqdefault",
            true,
            false,
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "input: https://youtu.be/dQw4w9WgXcQ?t=90\n\
             kind: video\n\
             id: dQw4w9WgXcQ\n\
             start: 90s (1:30)\n\
             canonical: https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=90s\n\
             thumbnail: https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"
        );
    }

    #[test]
    fn rejects_input_with_no_youtube_id() {
        let err = extract(
            "https://example.com/video/42",
            "text",
            "both",
            "hqdefault",
            true,
            false,
            true,
        )
        .unwrap_err();
        assert!(err.contains("strict mode"), "{err}");
    }

    #[test]
    fn empty_input_is_an_error() {
        assert!(extract("   \n\n", "text", "both", "hqdefault", true, false, false).is_err());
    }

    #[test]
    fn unknown_enum_values_are_rejected() {
        assert!(extract("dQw4w9WgXcQ", "xml", "both", "hqdefault", true, false, false).is_err());
        assert!(extract("dQw4w9WgXcQ", "text", "ticks", "hqdefault", true, false, false).is_err());
        assert!(extract("dQw4w9WgXcQ", "text", "both", "huge", true, false, false).is_err());
    }

    #[test]
    fn handles_every_youtube_url_shape() {
        for url in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ&feature=share",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://www.youtube.com/live/dQw4w9WgXcQ",
            "https://www.youtube.com/embed/dQw4w9WgXcQ",
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
            "https://www.youtube.com/v/dQw4w9WgXcQ",
            "https://www.youtube.com/e/dQw4w9WgXcQ",
            "http://youtube.com/watch?v=dQw4w9WgXcQ",
            "youtube.com/watch?v=dQw4w9WgXcQ",
            "//youtu.be/dQw4w9WgXcQ",
            "dQw4w9WgXcQ",
            "<https://youtu.be/dQw4w9WgXcQ>",
            "https://www.youtube.com/watch?app=desktop&v=dQw4w9WgXcQ",
        ] {
            assert_eq!(id_of(url), "dQw4w9WgXcQ", "failed on {url}");
        }
    }

    #[test]
    fn handles_invidious_and_piped_front_ends() {
        for url in [
            "https://yewtu.be/watch?v=dQw4w9WgXcQ",
            "https://inv.nadeko.net/dQw4w9WgXcQ",
            "https://piped.video/watch?v=dQw4w9WgXcQ",
            "https://piped.kavin.rocks/watch?v=dQw4w9WgXcQ&t=12",
            "https://invidious.example.org/embed/dQw4w9WgXcQ",
        ] {
            assert_eq!(id_of(url), "dQw4w9WgXcQ", "failed on {url}");
        }
    }

    #[test]
    fn unwraps_attribution_and_redirect_links() {
        assert_eq!(
            id_of("https://www.youtube.com/attribution_link?a=abc&u=%2Fwatch%3Fv%3DdQw4w9WgXcQ%26feature%3Dshare"),
            "dQw4w9WgXcQ"
        );
        assert_eq!(
            id_of("https://www.youtube.com/redirect?q=https%3A%2F%2Fyoutu.be%2FdQw4w9WgXcQ"),
            "dQw4w9WgXcQ"
        );
        assert_eq!(
            id_of("https://www.youtube.com/oembed?url=https%3A%2F%2Fyoutu.be%2FdQw4w9WgXcQ&format=json"),
            "dQw4w9WgXcQ"
        );
    }

    #[test]
    fn reads_every_timestamp_form() {
        assert_eq!(parse_one("https://youtu.be/dQw4w9WgXcQ?t=90").start, Some(90));
        assert_eq!(
            parse_one("https://youtu.be/dQw4w9WgXcQ?t=1m30s").start,
            Some(90)
        );
        assert_eq!(
            parse_one("https://youtu.be/dQw4w9WgXcQ?t=1h2m3s").start,
            Some(3723)
        );
        assert_eq!(
            parse_one("https://www.youtube.com/watch?v=dQw4w9WgXcQ&start=45").start,
            Some(45)
        );
        assert_eq!(
            parse_one("https://www.youtube.com/watch?v=dQw4w9WgXcQ#t=1:30").start,
            Some(90)
        );
        assert_eq!(parse_time("1:02:03"), Some(3723));
        assert_eq!(parse_time("90s"), Some(90));
        assert_eq!(parse_time("later"), None);
        assert_eq!(clock(90), "1:30");
        assert_eq!(clock(3723), "1:02:03");
    }

    #[test]
    fn captures_playlist_and_index() {
        let r = parse_one(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLbpi6ZahtOH6Blw3RGYpWkSByi_T7Rygb&index=4",
        );
        assert_eq!(r.kind, "video");
        assert_eq!(
            r.playlist.as_deref(),
            Some("PLbpi6ZahtOH6Blw3RGYpWkSByi_T7Rygb")
        );
        assert_eq!(r.index, Some(4));
        assert_eq!(
            canonical_url(&r).unwrap(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLbpi6ZahtOH6Blw3RGYpWkSByi_T7Rygb"
        );
    }

    #[test]
    fn recognises_playlist_channel_and_handle_links() {
        assert_eq!(
            parse_one("https://www.youtube.com/playlist?list=PLbpi6ZahtOH6Blw3RGYpWkSByi_T7Rygb").kind,
            "playlist"
        );
        let ch = parse_one("https://www.youtube.com/channel/UCuAXFkgsw1L7xaCfnd5JJOw");
        assert_eq!(ch.kind, "channel");
        assert_eq!(ch.id, "UCuAXFkgsw1L7xaCfnd5JJOw");
        assert_eq!(parse_one("https://www.youtube.com/@RickAstleyYT").kind, "handle");
        assert_eq!(parse_one("@RickAstleyYT").id, "RickAstleyYT");
        assert_eq!(parse_one("https://www.youtube.com/user/RickAstleyVEVO").kind, "user");
        assert_eq!(parse_one("https://www.youtube.com/c/RickAstley").kind, "custom");
        assert_eq!(
            parse_one("https://www.youtube.com/embed/videoseries?list=PLbpi6ZahtOH6Blw3RGYpWkSByi_T7Rygb").kind,
            "playlist"
        );
    }

    #[test]
    fn eleven_char_id_starting_with_pl_is_not_a_playlist() {
        assert_eq!(parse_one("PLdQw4w9WgX").kind, "video");
    }

    #[test]
    fn invalid_lines_are_reported_not_fatal() {
        let out = extract(
            "https://youtu.be/dQw4w9WgXcQ\nnot a link\n",
            "text",
            "seconds",
            "none",
            false,
            false,
            false,
        )
        .unwrap();
        assert!(out.contains("kind: invalid"), "{out}");
        assert!(out.ends_with("summary: 2 inputs, 1 resolved, 1 unresolved"), "{out}");
    }

    #[test]
    fn json_and_csv_outputs() {
        let json = extract(
            "https://youtu.be/dQw4w9WgXcQ?t=90",
            "json",
            "both",
            "none",
            true,
            true,
            false,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v[0]["id"], "dQw4w9WgXcQ");
        assert_eq!(v[0]["start_seconds"], 90);
        assert_eq!(v[0]["start_clock"], "1:30");
        assert_eq!(
            v[0]["embed"],
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?start=90"
        );
        assert!(v[0].get("thumbnail").is_none());

        let csv = extract(
            "https://youtu.be/dQw4w9WgXcQ?t=90",
            "csv",
            "seconds",
            "hqdefault",
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(
            csv,
            "input,kind,id,start_seconds,playlist,index,thumbnail,error\n\
             https://youtu.be/dQw4w9WgXcQ?t=90,video,dQw4w9WgXcQ,90,,,https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg,"
        );
    }

    #[test]
    fn thumbnail_variants_and_line_cap() {
        let r = parse_one("dQw4w9WgXcQ");
        for v in ["default", "mqdefault", "hqdefault", "sddefault", "maxresdefault"] {
            assert_eq!(
                thumbnail_url(&r, v).unwrap(),
                format!("https://i.ytimg.com/vi/dQw4w9WgXcQ/{v}.jpg")
            );
        }
        assert!(thumbnail_url(&r, "none").is_none());

        let many = "dQw4w9WgXcQ\n".repeat(MAX_LINES);
        assert!(extract(&many, "text", "both", "none", false, false, false).is_ok());
        let too_many = "dQw4w9WgXcQ\n".repeat(MAX_LINES + 1);
        let err = extract(&too_many, "text", "both", "none", false, false, false).unwrap_err();
        assert!(err.contains("too many lines: 201"), "{err}");
    }
}
