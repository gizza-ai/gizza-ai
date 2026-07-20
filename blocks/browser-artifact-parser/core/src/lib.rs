//! browser-artifact-parser core — read an uploaded browser **artifact database**
//! (Chrome/Edge, Firefox, or Safari history, cookies, downloads, or cache) and
//! correlate every time-stamped record into one unified, searchable forensic
//! timeline.
//!
//! Pure Rust, no wafer/wasm-bindgen deps → compiles to both `wasm32-wasip1`
//! (the wafer chat block) and native (CLI + host tests). The SQLite file is
//! parsed by reusing `gizza-ai-sqlite-table-to-csv-core::read_table`, which
//! walks the on-disk b-tree pages directly (no SQL engine, no C `libsqlite3`,
//! read-only).
//!
//! A single uploaded file is one artifact database, but several databases hold
//! more than one artifact type — a Chromium `History` file carries both page
//! visits and downloads — so every recognized table found is extracted and the
//! events are merged into one chronological timeline.
//!
//! Supported artifacts (auto-detected by table presence):
//!   - **Chrome/Edge/Chromium** `History`: `urls`+`visits` (page visits) and
//!     `downloads`(+`downloads_url_chains`) (downloads). Times are WebKit
//!     microseconds since 1601-01-01 UTC.
//!   - **Chrome/Edge/Chromium** `Cookies`: `cookies` (creation_utc, WebKit us).
//!   - **Firefox** `places.sqlite`: `moz_places`+`moz_historyvisits` (visits)
//!     and legacy `moz_downloads` (downloads). Times are PRTime microseconds
//!     since 1970-01-01 UTC.
//!   - **Firefox** `cookies.sqlite`: `moz_cookies` (creationTime, PRTime us).
//!   - **Safari** `History.db`: `history_items`+`history_visits` (visits).
//!     Times are CFAbsoluteTime — seconds since 2001-01-01 UTC.
//!   - **Safari/WebKit** `Cache.db`: `cfurl_cache_response` (cache entries).
//!     Time is a textual UTC datetime.
//!
//! Modern Firefox stores downloads as `moz_annos` annotations rather than a
//! table (only the legacy `downloads.sqlite`/`moz_downloads` form is parsed);
//! Chromium and Firefox *disk* caches are custom binary formats, not SQLite, so
//! only Safari's SQLite `Cache.db` is a cache source. An unrecognized SQLite
//! file yields a clear error listing the tables that were found.

use std::cmp::Ordering;
use std::collections::HashMap;

use gizza_ai_sqlite_table_to_csv_core::{list_tables, read_table, Value};
use serde::Serialize;

/// Seconds between the WebKit epoch (1601-01-01) and the Unix epoch (1970-01-01).
const WEBKIT_OFFSET_SECONDS: i64 = 11_644_473_600;
/// Seconds between the Cocoa/CFAbsoluteTime epoch (2001-01-01) and Unix.
const COCOA_OFFSET_SECONDS: i64 = 978_307_200;

/// The kind of event a timeline row represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A page visit (browsing history).
    Visit,
    /// A file download.
    Download,
    /// An HTTP cookie (dated by its creation time).
    Cookie,
    /// A cached network response.
    Cache,
}

impl Kind {
    /// The lowercase label used in output and by the `kind` filter.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Visit => "visit",
            Kind::Download => "download",
            Kind::Cookie => "cookie",
            Kind::Cache => "cache",
        }
    }

    fn parse_label(s: &str) -> Option<Kind> {
        match s {
            "visit" => Some(Kind::Visit),
            "download" => Some(Kind::Download),
            "cookie" => Some(Kind::Cookie),
            "cache" => Some(Kind::Cache),
            _ => None,
        }
    }
}

/// Which event kinds to keep in the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindFilter {
    /// Keep every event kind (default).
    All,
    /// Keep only one kind.
    Only(Kind),
}

impl KindFilter {
    /// Parse a filter name (as used by the descriptor `enumv`).
    pub fn parse(s: &str) -> Result<KindFilter, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "all" | "" => Ok(KindFilter::All),
            "visit" | "visits" => Ok(KindFilter::Only(Kind::Visit)),
            "download" | "downloads" => Ok(KindFilter::Only(Kind::Download)),
            "cookie" | "cookies" => Ok(KindFilter::Only(Kind::Cookie)),
            "cache" => Ok(KindFilter::Only(Kind::Cache)),
            other => Err(format!(
                "unknown kind {other:?} (expected: all, visit, download, cookie, or cache)"
            )),
        }
    }

    fn keeps(self, k: Kind) -> bool {
        match self {
            KindFilter::All => true,
            KindFilter::Only(want) => want == k,
        }
    }
}

/// Timeline sort order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Most recent events first (default).
    Newest,
    /// Oldest events first.
    Oldest,
}

impl Order {
    /// Parse an order name (as used by the descriptor `enumv`).
    pub fn parse(s: &str) -> Result<Order, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "newest" | "desc" | "descending" => Ok(Order::Newest),
            "oldest" | "asc" | "ascending" => Ok(Order::Oldest),
            other => Err(format!("unknown order {other:?} (expected: newest or oldest)")),
        }
    }
}

/// Options controlling the timeline output.
#[derive(Debug, Clone)]
pub struct Options {
    /// Case-insensitive substring; keep only events whose location, name, or
    /// info contains it. `None`/empty → all events.
    pub search: Option<String>,
    /// Restrict to one event kind, or keep all.
    pub kind: KindFilter,
    /// Timeline sort order.
    pub order: Order,
    /// Cap on returned events; `0` = all.
    pub limit: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            search: None,
            kind: KindFilter::All,
            order: Order::Newest,
            limit: 0,
        }
    }
}

/// One event in the unified timeline.
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    /// Readable UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`), or empty if unknown.
    pub timestamp: String,
    /// Unix time in seconds (`0` if the source timestamp was missing/zero).
    pub unix_seconds: i64,
    /// Event kind: `visit`, `download`, `cookie`, or `cache`.
    pub kind: String,
    /// Origin of the record, e.g. `"Chrome/Edge history"` or `"Firefox cookies"`.
    pub source: String,
    /// The URL (visits, downloads, cache) or cookie host.
    pub location: String,
    /// Page title (visits), saved filename (downloads), or cookie name.
    pub name: String,
    /// Extra context: visit type, download size, or cookie expiry.
    pub info: String,
}

/// Per-kind event tallies (before filtering).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Counts {
    pub visits: usize,
    pub downloads: usize,
    pub cookies: usize,
    pub cache: usize,
}

/// The parsed, merged timeline.
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactOutput {
    /// Detected browser families (e.g. `["Chrome/Edge"]`).
    pub browsers: Vec<String>,
    /// Detected artifact sources (e.g. `["Chrome/Edge history", "Chrome/Edge downloads"]`).
    pub artifacts: Vec<String>,
    /// Event counts per kind, before search/kind/limit filtering.
    pub counts: Counts,
    /// Total events extracted (before search/kind/limit).
    pub total_events: usize,
    /// Events remaining after the search + kind filters.
    pub matched: usize,
    /// Events actually returned after the limit.
    pub returned: usize,
    /// Whether the result was truncated by `limit`.
    pub truncated: bool,
    /// The timeline, sorted per `Options::order`.
    pub events: Vec<Event>,
}

/// Parse a browser artifact database into a unified event timeline.
pub fn parse_artifacts(bytes: &[u8], opts: &Options) -> Result<ArtifactOutput, String> {
    // `list_tables` also validates the SQLite header (bad magic → clear error).
    let tables = list_tables(bytes)?;
    let lower: Vec<String> = tables.iter().map(|t| t.to_ascii_lowercase()).collect();
    let has = |name: &str| lower.iter().any(|t| t == name);

    let mut browsers: Vec<String> = Vec::new();
    let mut artifacts: Vec<String> = Vec::new();
    let mut events: Vec<Event> = Vec::new();

    let note_browser = |browsers: &mut Vec<String>, b: &str| {
        if !browsers.iter().any(|x| x == b) {
            browsers.push(b.to_string());
        }
    };

    // --- Chrome/Edge/Chromium History: visits (+ downloads) -----------------
    if has("urls") && has("visits") {
        note_browser(&mut browsers, "Chrome/Edge");
        artifacts.push("Chrome/Edge history".into());
        extract_chrome_visits(bytes, &mut events)?;
    }
    if has("downloads") && has("downloads_url_chains") {
        note_browser(&mut browsers, "Chrome/Edge");
        artifacts.push("Chrome/Edge downloads".into());
        extract_chrome_downloads(bytes, &mut events)?;
    }
    // --- Chrome/Edge/Chromium Cookies ---------------------------------------
    if has("cookies") {
        note_browser(&mut browsers, "Chrome/Edge");
        artifacts.push("Chrome/Edge cookies".into());
        extract_chrome_cookies(bytes, &mut events)?;
    }
    // --- Firefox places: visits (+ legacy downloads) ------------------------
    if has("moz_places") && has("moz_historyvisits") {
        note_browser(&mut browsers, "Firefox");
        artifacts.push("Firefox history".into());
        extract_firefox_visits(bytes, &mut events)?;
    }
    if has("moz_downloads") {
        note_browser(&mut browsers, "Firefox");
        artifacts.push("Firefox downloads".into());
        extract_firefox_downloads(bytes, &mut events)?;
    }
    // --- Firefox cookies ----------------------------------------------------
    if has("moz_cookies") {
        note_browser(&mut browsers, "Firefox");
        artifacts.push("Firefox cookies".into());
        extract_firefox_cookies(bytes, &mut events)?;
    }
    // --- Safari History.db --------------------------------------------------
    if has("history_items") && has("history_visits") {
        note_browser(&mut browsers, "Safari");
        artifacts.push("Safari history".into());
        extract_safari_visits(bytes, &mut events)?;
    }
    // --- Safari/WebKit Cache.db ---------------------------------------------
    if has("cfurl_cache_response") {
        note_browser(&mut browsers, "Safari");
        artifacts.push("Safari cache".into());
        extract_safari_cache(bytes, &mut events)?;
    }

    if artifacts.is_empty() {
        return Err(format!(
            "not a recognized browser artifact database — expected Chrome/Edge (urls+visits, \
             downloads, cookies), Firefox (moz_places+moz_historyvisits, moz_cookies, \
             moz_downloads), or Safari (history_items+history_visits, cfurl_cache_response) \
             tables; found: {}",
            if tables.is_empty() {
                "<none>".to_string()
            } else {
                tables.join(", ")
            }
        ));
    }

    let counts = Counts {
        visits: events.iter().filter(|e| e.kind == "visit").count(),
        downloads: events.iter().filter(|e| e.kind == "download").count(),
        cookies: events.iter().filter(|e| e.kind == "cookie").count(),
        cache: events.iter().filter(|e| e.kind == "cache").count(),
    };
    let total_events = events.len();

    // Filter by kind + search.
    let needle = opts.search.as_ref().map(|s| s.to_lowercase());
    events.retain(|e| {
        let kind_ok = match Kind::parse_label(&e.kind) {
            Some(k) => opts.kind.keeps(k),
            None => true,
        };
        if !kind_ok {
            return false;
        }
        match &needle {
            Some(n) => {
                e.location.to_lowercase().contains(n)
                    || e.name.to_lowercase().contains(n)
                    || e.info.to_lowercase().contains(n)
            }
            None => true,
        }
    });
    let matched = events.len();

    // Sort by time (then location for a stable tie-break).
    events.sort_by(|a, b| {
        let ord = a.unix_seconds.cmp(&b.unix_seconds);
        let ord = match opts.order {
            Order::Newest => ord.reverse(),
            Order::Oldest => ord,
        };
        match ord {
            Ordering::Equal => a.location.cmp(&b.location),
            other => other,
        }
    });

    let truncated = opts.limit > 0 && events.len() > opts.limit;
    if truncated {
        events.truncate(opts.limit);
    }
    let returned = events.len();

    browsers.sort();
    browsers.dedup();

    Ok(ArtifactOutput {
        browsers,
        artifacts,
        counts,
        total_events,
        matched,
        returned,
        truncated,
        events,
    })
}

// ---------------------------------------------------------------------------
// Extractors — one per artifact source. Each pushes `Event`s onto `out`.
// ---------------------------------------------------------------------------

/// Value → owned text: TEXT verbatim, INTEGER/REAL stringified, else empty.
fn cell_text(v: &Value) -> String {
    match v {
        Value::Text(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Real(f) => f.to_string(),
        _ => String::new(),
    }
}

fn get<'a>(row: &'a [Value], idx: Option<usize>) -> Option<&'a Value> {
    idx.and_then(|i| row.get(i))
}

fn text_at(row: &[Value], idx: Option<usize>) -> String {
    get(row, idx).map(cell_text).unwrap_or_default()
}

fn i64_at(row: &[Value], idx: Option<usize>) -> Option<i64> {
    get(row, idx).and_then(Value::as_i64)
}

fn f64_at(row: &[Value], idx: Option<usize>) -> Option<f64> {
    match get(row, idx) {
        Some(Value::Real(f)) => Some(*f),
        Some(Value::Int(i)) => Some(*i as f64),
        _ => None,
    }
}

fn extract_chrome_visits(bytes: &[u8], out: &mut Vec<Event>) -> Result<(), String> {
    let urls = read_table(bytes, "urls")?;
    let visits = read_table(bytes, "visits")?;
    let u_id = urls.col_index("id");
    let u_url = urls.col_index("url");
    let u_title = urls.col_index("title");
    let mut url_by_id: HashMap<i64, (String, String)> = HashMap::new();
    for row in &urls.rows {
        if let Some(id) = i64_at(row, u_id) {
            url_by_id.insert(id, (text_at(row, u_url), text_at(row, u_title)));
        }
    }
    let v_url = visits.col_index("url");
    let v_time = visits.col_index("visit_time");
    let v_trans = visits.col_index("transition");
    for row in &visits.rows {
        let uid = i64_at(row, v_url).unwrap_or_default();
        let (url, title) = url_by_id.get(&uid).cloned().unwrap_or_default();
        let unix = webkit_to_unix(i64_at(row, v_time).unwrap_or_default());
        let trans = i64_at(row, v_trans).unwrap_or_default();
        out.push(Event {
            timestamp: iso8601_utc(unix),
            unix_seconds: unix,
            kind: Kind::Visit.label().into(),
            source: "Chrome/Edge history".into(),
            location: url,
            name: title,
            info: format!("type: {}", chrome_transition(trans)),
        });
    }
    Ok(())
}

fn extract_chrome_downloads(bytes: &[u8], out: &mut Vec<Event>) -> Result<(), String> {
    let dl = read_table(bytes, "downloads")?;
    // Final URL for each download is the highest chain_index in downloads_url_chains.
    let chains = read_table(bytes, "downloads_url_chains").ok();
    let mut url_by_id: HashMap<i64, (i64, String)> = HashMap::new();
    if let Some(ch) = &chains {
        let c_id = ch.col_index("id");
        let c_idx = ch.col_index("chain_index");
        let c_url = ch.col_index("url");
        for row in &ch.rows {
            let id = i64_at(row, c_id).unwrap_or_default();
            let idx = i64_at(row, c_idx).unwrap_or_default();
            let url = text_at(row, c_url);
            let e = url_by_id.entry(id).or_insert((i64::MIN, String::new()));
            if idx >= e.0 {
                *e = (idx, url);
            }
        }
    }
    let d_id = dl.col_index("id");
    let d_target = dl.col_index("target_path");
    let d_tab = dl.col_index("tab_url");
    let d_start = dl.col_index("start_time");
    let d_recv = dl.col_index("received_bytes");
    let d_total = dl.col_index("total_bytes");
    for row in &dl.rows {
        let id = i64_at(row, d_id).unwrap_or_default();
        let url = url_by_id
            .get(&id)
            .map(|(_, u)| u.clone())
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| text_at(row, d_tab));
        let unix = webkit_to_unix(i64_at(row, d_start).unwrap_or_default());
        let filename = basename(&text_at(row, d_target));
        let recv = i64_at(row, d_recv).unwrap_or_default();
        let total = i64_at(row, d_total).unwrap_or_default();
        out.push(Event {
            timestamp: iso8601_utc(unix),
            unix_seconds: unix,
            kind: Kind::Download.label().into(),
            source: "Chrome/Edge downloads".into(),
            location: url,
            name: filename,
            info: format!("{recv} of {total} bytes"),
        });
    }
    Ok(())
}

fn extract_chrome_cookies(bytes: &[u8], out: &mut Vec<Event>) -> Result<(), String> {
    let ck = read_table(bytes, "cookies")?;
    let c_host = ck.col_index("host_key");
    let c_name = ck.col_index("name");
    let c_create = ck.col_index("creation_utc");
    let c_expire = ck.col_index("expires_utc");
    for row in &ck.rows {
        let unix = webkit_to_unix(i64_at(row, c_create).unwrap_or_default());
        let expire = webkit_to_unix(i64_at(row, c_expire).unwrap_or_default());
        out.push(Event {
            timestamp: iso8601_utc(unix),
            unix_seconds: unix,
            kind: Kind::Cookie.label().into(),
            source: "Chrome/Edge cookies".into(),
            location: text_at(row, c_host),
            name: text_at(row, c_name),
            info: cookie_expiry_info(expire),
        });
    }
    Ok(())
}

fn extract_firefox_visits(bytes: &[u8], out: &mut Vec<Event>) -> Result<(), String> {
    let places = read_table(bytes, "moz_places")?;
    let visits = read_table(bytes, "moz_historyvisits")?;
    let p_id = places.col_index("id");
    let p_url = places.col_index("url");
    let p_title = places.col_index("title");
    let mut place_by_id: HashMap<i64, (String, String)> = HashMap::new();
    for row in &places.rows {
        if let Some(id) = i64_at(row, p_id) {
            place_by_id.insert(id, (text_at(row, p_url), text_at(row, p_title)));
        }
    }
    let v_place = visits.col_index("place_id");
    let v_date = visits.col_index("visit_date");
    let v_type = visits.col_index("visit_type");
    for row in &visits.rows {
        let pid = i64_at(row, v_place).unwrap_or_default();
        let (url, title) = place_by_id.get(&pid).cloned().unwrap_or_default();
        let unix = prtime_to_unix(i64_at(row, v_date).unwrap_or_default());
        let vtype = i64_at(row, v_type).unwrap_or_default();
        out.push(Event {
            timestamp: iso8601_utc(unix),
            unix_seconds: unix,
            kind: Kind::Visit.label().into(),
            source: "Firefox history".into(),
            location: url,
            name: title,
            info: format!("type: {}", firefox_visit_type(vtype)),
        });
    }
    Ok(())
}

fn extract_firefox_downloads(bytes: &[u8], out: &mut Vec<Event>) -> Result<(), String> {
    let dl = read_table(bytes, "moz_downloads")?;
    let d_name = dl.col_index("name");
    let d_source = dl.col_index("source");
    let d_target = dl.col_index("target");
    let d_start = dl.col_index("startTime");
    for row in &dl.rows {
        let unix = prtime_to_unix(i64_at(row, d_start).unwrap_or_default());
        let name = {
            let n = text_at(row, d_name);
            if n.is_empty() {
                basename(&text_at(row, d_target))
            } else {
                n
            }
        };
        out.push(Event {
            timestamp: iso8601_utc(unix),
            unix_seconds: unix,
            kind: Kind::Download.label().into(),
            source: "Firefox downloads".into(),
            location: text_at(row, d_source),
            name,
            info: String::new(),
        });
    }
    Ok(())
}

fn extract_firefox_cookies(bytes: &[u8], out: &mut Vec<Event>) -> Result<(), String> {
    let ck = read_table(bytes, "moz_cookies")?;
    let c_host = ck.col_index("host");
    let c_name = ck.col_index("name");
    let c_create = ck.col_index("creationTime");
    let c_expiry = ck.col_index("expiry");
    for row in &ck.rows {
        let unix = prtime_to_unix(i64_at(row, c_create).unwrap_or_default());
        // Firefox `expiry` is already SECONDS since the Unix epoch.
        let expire = i64_at(row, c_expiry).unwrap_or_default();
        out.push(Event {
            timestamp: iso8601_utc(unix),
            unix_seconds: unix,
            kind: Kind::Cookie.label().into(),
            source: "Firefox cookies".into(),
            location: text_at(row, c_host),
            name: text_at(row, c_name),
            info: cookie_expiry_info(expire),
        });
    }
    Ok(())
}

fn extract_safari_visits(bytes: &[u8], out: &mut Vec<Event>) -> Result<(), String> {
    let items = read_table(bytes, "history_items")?;
    let visits = read_table(bytes, "history_visits")?;
    let i_id = items.col_index("id");
    let i_url = items.col_index("url");
    let mut url_by_id: HashMap<i64, String> = HashMap::new();
    for row in &items.rows {
        if let Some(id) = i64_at(row, i_id) {
            url_by_id.insert(id, text_at(row, i_url));
        }
    }
    let v_item = visits.col_index("history_item");
    let v_time = visits.col_index("visit_time");
    let v_title = visits.col_index("title");
    for row in &visits.rows {
        let iid = i64_at(row, v_item).unwrap_or_default();
        let url = url_by_id.get(&iid).cloned().unwrap_or_default();
        let unix = cocoa_to_unix(f64_at(row, v_time).unwrap_or_default());
        out.push(Event {
            timestamp: iso8601_utc(unix),
            unix_seconds: unix,
            kind: Kind::Visit.label().into(),
            source: "Safari history".into(),
            location: url,
            name: text_at(row, v_title),
            info: String::new(),
        });
    }
    Ok(())
}

fn extract_safari_cache(bytes: &[u8], out: &mut Vec<Event>) -> Result<(), String> {
    let cache = read_table(bytes, "cfurl_cache_response")?;
    let c_key = cache.col_index("request_key");
    let c_ts = cache.col_index("time_stamp");
    for row in &cache.rows {
        let ts = text_at(row, c_ts);
        let unix = parse_sql_datetime(&ts).unwrap_or_default();
        out.push(Event {
            timestamp: iso8601_utc(unix),
            unix_seconds: unix,
            kind: Kind::Cache.label().into(),
            source: "Safari cache".into(),
            location: text_at(row, c_key),
            name: String::new(),
            info: String::new(),
        });
    }
    Ok(())
}

/// Human-readable cookie expiry (session cookies have expiry 0).
fn cookie_expiry_info(expire_unix: i64) -> String {
    if expire_unix <= 0 {
        "expires: session".to_string()
    } else {
        format!("expires: {}", iso8601_utc(expire_unix))
    }
}

/// Last path segment of a file path (handles both `/` and `\` separators).
fn basename(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(path)
        .to_string()
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Pretty-printed JSON of the full output.
pub fn render_json(out: &ArtifactOutput) -> String {
    serde_json::to_string_pretty(out).unwrap_or_else(|_| "{}".to_string())
}

/// RFC-4180 CSV — one row per event, with a `source` column so exports from
/// several artifact files merge cleanly.
pub fn render_csv(out: &ArtifactOutput) -> String {
    let mut s = String::from("timestamp,unix_seconds,kind,source,location,name,info\r\n");
    for e in &out.events {
        let unix = e.unix_seconds.to_string();
        let fields = [
            e.timestamp.as_str(),
            unix.as_str(),
            e.kind.as_str(),
            e.source.as_str(),
            e.location.as_str(),
            e.name.as_str(),
            e.info.as_str(),
        ];
        let mut first = true;
        for f in fields {
            if !first {
                s.push(',');
            }
            first = false;
            s.push_str(&escape_csv(f));
        }
        s.push_str("\r\n");
    }
    s
}

fn escape_csv(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

// ---------------------------------------------------------------------------
// Transition / visit-type decoding
// ---------------------------------------------------------------------------

/// Chromium `transition`'s low byte is the core page-transition type.
fn chrome_transition(transition: i64) -> String {
    match (transition & 0xff) as u8 {
        0 => "link",
        1 => "typed",
        2 => "bookmark",
        3 => "auto subframe",
        4 => "manual subframe",
        5 => "generated",
        6 => "start page",
        7 => "form submit",
        8 => "reload",
        9 => "keyword",
        10 => "keyword generated",
        n => return format!("other ({n})"),
    }
    .to_string()
}

/// Firefox `visit_type` code.
fn firefox_visit_type(vtype: i64) -> String {
    match vtype {
        1 => "link",
        2 => "typed",
        3 => "bookmark",
        4 => "embed",
        5 => "redirect (permanent)",
        6 => "redirect (temporary)",
        7 => "download",
        8 => "framed link",
        9 => "reload",
        n => return format!("other ({n})"),
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Epoch conversions → Unix seconds
// ---------------------------------------------------------------------------

/// WebKit microseconds since 1601 → Unix seconds (`0`/negative → 0).
fn webkit_to_unix(us: i64) -> i64 {
    if us <= 0 {
        return 0;
    }
    us / 1_000_000 - WEBKIT_OFFSET_SECONDS
}

/// PRTime microseconds since 1970 → Unix seconds (`0`/negative → 0).
fn prtime_to_unix(us: i64) -> i64 {
    if us <= 0 {
        return 0;
    }
    us / 1_000_000
}

/// CFAbsoluteTime seconds since 2001 → Unix seconds (non-positive → 0).
fn cocoa_to_unix(secs: f64) -> i64 {
    if secs <= 0.0 || !secs.is_finite() {
        return 0;
    }
    secs as i64 + COCOA_OFFSET_SECONDS
}

/// Parse a `YYYY-MM-DD HH:MM:SS` (or `...THH:MM:SS`) UTC datetime → Unix seconds.
/// Returns `None` if the string doesn't match.
fn parse_sql_datetime(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let num = |a: usize, b: usize| -> Option<i64> { s.get(a..b)?.parse::<i64>().ok() };
    let y = num(0, 4)?;
    let mo = num(5, 7)?;
    let d = num(8, 10)?;
    let h = num(11, 13)?;
    let mi = num(14, 16)?;
    let sec = num(17, 19)?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let days = days_from_civil(y, mo as u32, d as u32);
    Some(days * 86_400 + h * 3600 + mi * 60 + sec)
}

// ---------------------------------------------------------------------------
// Date math (no chrono — Howard Hinnant's civil algorithms)
// ---------------------------------------------------------------------------

/// Format Unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC). Empty string for `0`.
fn iso8601_utc(unix_seconds: i64) -> String {
    if unix_seconds == 0 {
        return String::new();
    }
    let days = unix_seconds.div_euclid(86_400);
    let secs = unix_seconds.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Days since 1970-01-01 → (year, month [1-12], day [1-31]). UTC/proleptic
/// Gregorian; valid across the whole i64 range.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (year, month [1-12], day [1-31]) → days since 1970-01-01. Inverse of
/// [`civil_from_days`].
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + (d as i64 - 1); // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real SQLite files built by `tests/fixtures/gen_fixtures.py`.
    const CHROME_HISTORY: &[u8] = include_bytes!("../tests/fixtures/chrome_history.db");
    const CHROME_COOKIES: &[u8] = include_bytes!("../tests/fixtures/chrome_cookies.db");
    const FIREFOX_PLACES: &[u8] = include_bytes!("../tests/fixtures/firefox_places.sqlite");
    const FIREFOX_COOKIES: &[u8] = include_bytes!("../tests/fixtures/firefox_cookies.sqlite");
    const SAFARI_HISTORY: &[u8] = include_bytes!("../tests/fixtures/safari_history.db");
    const SAFARI_CACHE: &[u8] = include_bytes!("../tests/fixtures/safari_cache.db");
    const OTHER: &[u8] = include_bytes!("../tests/fixtures/other.db");

    #[test]
    fn chrome_history_merges_visits_and_downloads_newest_first() {
        let out = parse_artifacts(CHROME_HISTORY, &Options::default()).unwrap();
        assert_eq!(out.browsers, vec!["Chrome/Edge"]);
        assert_eq!(
            out.artifacts,
            vec!["Chrome/Edge history", "Chrome/Edge downloads"]
        );
        assert_eq!(out.counts.visits, 2);
        assert_eq!(out.counts.downloads, 1);
        assert_eq!(out.total_events, 3);
        assert_eq!(out.returned, 3);

        // Newest first: download 2024-01-20 → visit 2024-01-15 → visit 2023-12-25.
        let e0 = &out.events[0];
        assert_eq!(e0.kind, "download");
        assert_eq!(e0.timestamp, "2024-01-20T09:05:00Z");
        assert_eq!(e0.unix_seconds, 1_705_741_500);
        assert_eq!(e0.name, "rustup-init");
        // Final URL from the highest chain_index, not the redirect entry.
        assert_eq!(e0.location, "https://static.rust-lang.org/rustup/rustup-init");
        assert_eq!(e0.info, "5242880 of 5242880 bytes");

        assert_eq!(out.events[1].kind, "visit");
        assert_eq!(out.events[1].timestamp, "2024-01-15T10:30:00Z");
        assert_eq!(out.events[1].location, "https://www.rust-lang.org/");
        assert_eq!(out.events[1].name, "Rust Programming Language");
        assert_eq!(out.events[1].info, "type: link");

        assert_eq!(out.events[2].timestamp, "2023-12-25T08:00:00Z");
        assert_eq!(out.events[2].info, "type: typed");
    }

    #[test]
    fn chrome_cookies_dated_by_creation() {
        let out = parse_artifacts(CHROME_COOKIES, &Options::default()).unwrap();
        assert_eq!(out.artifacts, vec!["Chrome/Edge cookies"]);
        assert_eq!(out.counts.cookies, 2);
        // Newest first: github cookie 2024-05-01, then example 2023-11-20.
        let e0 = &out.events[0];
        assert_eq!(e0.kind, "cookie");
        assert_eq!(e0.timestamp, "2024-05-01T08:00:00Z");
        assert_eq!(e0.location, ".github.com");
        assert_eq!(e0.name, "logged_in");
        assert_eq!(e0.info, "expires: 2025-05-01T08:00:00Z");
    }

    #[test]
    fn firefox_places_visits() {
        let out = parse_artifacts(FIREFOX_PLACES, &Options::default()).unwrap();
        assert_eq!(out.browsers, vec!["Firefox"]);
        assert_eq!(out.counts.visits, 2);
        assert_eq!(out.events[0].timestamp, "2024-03-10T09:15:00Z");
        assert_eq!(out.events[0].source, "Firefox history");
        assert_eq!(out.events[0].info, "type: link");
        assert_eq!(out.events[1].timestamp, "2022-06-01T00:00:00Z");
        assert_eq!(out.events[1].info, "type: typed");
    }

    #[test]
    fn firefox_cookies_prtime_creation() {
        let out = parse_artifacts(FIREFOX_COOKIES, &Options::default()).unwrap();
        assert_eq!(out.counts.cookies, 2);
        // Newest first: mozilla 2024-04-10, then wiki 2024-01-05.
        assert_eq!(out.events[0].timestamp, "2024-04-10T06:00:00Z");
        assert_eq!(out.events[0].source, "Firefox cookies");
        assert_eq!(out.events[0].location, ".mozilla.org");
        assert_eq!(out.events[0].name, "pref");
    }

    #[test]
    fn safari_history_cfabsolute_time() {
        let out = parse_artifacts(SAFARI_HISTORY, &Options::default()).unwrap();
        assert_eq!(out.browsers, vec!["Safari"]);
        assert_eq!(out.counts.visits, 2);
        // Newest first: docs 2024-06-03, then apple 2024-06-02.
        assert_eq!(out.events[0].timestamp, "2024-06-03T10:00:00Z");
        assert_eq!(out.events[0].location, "https://developer.example.com/docs");
        assert_eq!(out.events[0].name, "Docs");
        assert_eq!(out.events[1].timestamp, "2024-06-02T16:20:00Z");
        assert_eq!(out.events[1].location, "https://www.apple.com/");
    }

    #[test]
    fn safari_cache_text_timestamp() {
        let out = parse_artifacts(SAFARI_CACHE, &Options::default()).unwrap();
        assert_eq!(out.artifacts, vec!["Safari cache"]);
        assert_eq!(out.counts.cache, 2);
        // Newest first: logo 08:16:00, then app.js 08:15:30.
        assert_eq!(out.events[0].kind, "cache");
        assert_eq!(out.events[0].timestamp, "2024-07-01T08:16:00Z");
        assert_eq!(out.events[0].location, "https://img.example.net/logo.png");
        assert_eq!(out.events[1].timestamp, "2024-07-01T08:15:30Z");
        assert_eq!(out.events[1].unix_seconds, 1_719_821_730);
    }

    #[test]
    fn oldest_order_reverses() {
        let out = parse_artifacts(
            CHROME_HISTORY,
            &Options {
                order: Order::Oldest,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.events[0].timestamp, "2023-12-25T08:00:00Z");
        assert_eq!(out.events.last().unwrap().timestamp, "2024-01-20T09:05:00Z");
    }

    #[test]
    fn kind_filter_keeps_only_downloads() {
        let out = parse_artifacts(
            CHROME_HISTORY,
            &Options {
                kind: KindFilter::Only(Kind::Download),
                ..Default::default()
            },
        )
        .unwrap();
        // total_events counts everything; matched/returned are download-only.
        assert_eq!(out.total_events, 3);
        assert_eq!(out.matched, 1);
        assert_eq!(out.returned, 1);
        assert!(out.events.iter().all(|e| e.kind == "download"));
    }

    #[test]
    fn search_filters_by_substring() {
        let out = parse_artifacts(
            CHROME_HISTORY,
            &Options {
                search: Some("rustup".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.matched, 1);
        assert_eq!(out.events[0].kind, "download");
    }

    #[test]
    fn limit_truncates_and_flags() {
        let out = parse_artifacts(
            CHROME_HISTORY,
            &Options {
                limit: 2,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.returned, 2);
        assert!(out.truncated);
        assert_eq!(out.matched, 3);
    }

    #[test]
    fn non_browser_db_is_rejected() {
        let err = parse_artifacts(OTHER, &Options::default()).unwrap_err();
        assert!(err.contains("not a recognized browser artifact database"));
        assert!(err.contains("notes"));
    }

    #[test]
    fn corrupt_input_is_rejected() {
        let err = parse_artifacts(b"not a sqlite file at all", &Options::default()).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn csv_has_header_and_rows() {
        let out = parse_artifacts(CHROME_HISTORY, &Options::default()).unwrap();
        let csv = render_csv(&out);
        assert!(csv.starts_with("timestamp,unix_seconds,kind,source,location,name,info\r\n"));
        assert_eq!(csv.matches("\r\n").count(), 4); // header + 3 events
        assert!(csv.contains("download"));
    }

    #[test]
    fn kind_filter_parses_aliases() {
        assert_eq!(KindFilter::parse("all").unwrap(), KindFilter::All);
        assert_eq!(
            KindFilter::parse("Cookies").unwrap(),
            KindFilter::Only(Kind::Cookie)
        );
        assert!(KindFilter::parse("bookmarks").is_err());
    }

    #[test]
    fn epoch_and_datetime_math() {
        assert_eq!(iso8601_utc(0), "");
        assert_eq!(iso8601_utc(1_706_788_800), "2024-02-01T12:00:00Z");
        assert_eq!(
            webkit_to_unix((1_706_788_800 + WEBKIT_OFFSET_SECONDS) * 1_000_000),
            1_706_788_800
        );
        assert_eq!(prtime_to_unix(1_710_182_700_000_000), 1_710_182_700);
        assert_eq!(cocoa_to_unix(0.0), 0);
        assert_eq!(cocoa_to_unix(739_800_000.0), 739_800_000 + COCOA_OFFSET_SECONDS);
        assert_eq!(parse_sql_datetime("2024-07-01 08:15:30"), Some(1_719_821_730));
        assert_eq!(parse_sql_datetime("not a date"), None);
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2024, 7, 1) * 86_400, 1_719_792_000);
    }
}
