//! browser-history-parser core — read an uploaded browser **history database**
//! (Chrome/Edge `History` or Firefox `places.sqlite`) and produce a unified,
//! searchable visit timeline.
//!
//! Pure Rust, no wafer/wasm-bindgen deps → compiles to both `wasm32-wasip1`
//! (the wafer chat block) and native (CLI + host tests). The SQLite file is
//! parsed by reusing `gizza-ai-sqlite-table-to-csv-core::read_table`, which
//! walks the on-disk b-tree pages directly (no SQL engine, no C `libsqlite3`).
//!
//! Supported browsers (auto-detected by table presence):
//!   - **Chrome/Edge** (Chromium): `urls` + `visits`. Visit times are WebKit
//!     microseconds since 1601-01-01 UTC; `transition`'s low byte is the core
//!     page-transition type.
//!   - **Firefox**: `moz_places` + `moz_historyvisits`. Visit times are PRTime
//!     microseconds since 1970-01-01 UTC; `visit_type` is the transition code.
//!
//! Other browsers (Safari, legacy IE, downloads DBs) use different schemas/epochs
//! and are not handled (a clear error lists the tables that were found).

use std::cmp::Ordering;
use std::collections::HashMap;

use gizza_ai_sqlite_table_to_csv_core::{list_tables, read_table, Value};
use serde::Serialize;

/// Seconds between the WebKit epoch (1601-01-01) and the Unix epoch (1970-01-01).
const WEBKIT_OFFSET_SECONDS: i64 = 11_644_473_600;

/// Sort order for the visit timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Most recent visits first (default).
    Newest,
    /// Oldest visits first.
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
    /// Case-insensitive substring; keep only visits whose URL or title contains
    /// it. `None`/empty → all visits.
    pub search: Option<String>,
    /// Timeline sort order.
    pub order: Order,
    /// Cap on returned visits; `0` = all.
    pub limit: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            search: None,
            order: Order::Newest,
            limit: 0,
        }
    }
}

/// One visit in the timeline.
#[derive(Debug, Clone, Serialize)]
pub struct Visit {
    /// Readable UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`), or empty if unknown.
    pub timestamp: String,
    /// Unix time in seconds (`0` if the source timestamp was missing/zero).
    pub unix_seconds: i64,
    /// The visited URL.
    pub url: String,
    /// The page title (empty if the browser stored none).
    pub title: String,
    /// Human transition/visit type (link, typed, bookmark, reload, …).
    pub visit_type: String,
    /// Total recorded visits to this URL (from the URL row's `visit_count`).
    pub visit_count: i64,
}

/// The parsed timeline.
#[derive(Debug, Clone, Serialize)]
pub struct HistoryOutput {
    /// Detected browser family: `"Chrome/Edge"` or `"Firefox"`.
    pub browser: String,
    /// Total visits found in the database (before search/limit).
    pub total_visits: usize,
    /// Visits remaining after the search filter.
    pub matched: usize,
    /// Visits actually returned after the limit.
    pub returned: usize,
    /// Whether the result was truncated by `limit`.
    pub truncated: bool,
    /// The timeline, sorted per `Options::order`.
    pub visits: Vec<Visit>,
}

/// Parse a browser history database into a unified visit timeline.
pub fn parse_history(bytes: &[u8], opts: &Options) -> Result<HistoryOutput, String> {
    // `list_tables` also validates the SQLite header (bad magic → clear error).
    let tables = list_tables(bytes)?;
    let lower: Vec<String> = tables.iter().map(|t| t.to_ascii_lowercase()).collect();
    let has = |name: &str| lower.iter().any(|t| t == name);

    let (browser, mut visits) = if has("urls") && has("visits") {
        ("Chrome/Edge".to_string(), parse_chrome(bytes)?)
    } else if has("moz_places") && has("moz_historyvisits") {
        ("Firefox".to_string(), parse_firefox(bytes)?)
    } else {
        return Err(format!(
            "not a recognized browser history database — expected Chrome/Edge (urls, visits) or \
             Firefox (moz_places, moz_historyvisits) tables; found: {}",
            if tables.is_empty() { "<none>".to_string() } else { tables.join(", ") }
        ));
    };

    let total_visits = visits.len();

    if let Some(q) = opts.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let ql = q.to_lowercase();
        visits.retain(|v| v.url.to_lowercase().contains(&ql) || v.title.to_lowercase().contains(&ql));
    }
    let matched = visits.len();

    // Sort by time; visits with an unknown timestamp (0) always sort last.
    visits.sort_by(|a, b| match (a.unix_seconds == 0, b.unix_seconds == 0) {
        (false, true) => Ordering::Less,
        (true, false) => Ordering::Greater,
        _ => match opts.order {
            Order::Newest => b.unix_seconds.cmp(&a.unix_seconds),
            Order::Oldest => a.unix_seconds.cmp(&b.unix_seconds),
        },
    });

    let truncated = opts.limit != 0 && visits.len() > opts.limit;
    if truncated {
        visits.truncate(opts.limit);
    }
    let returned = visits.len();

    Ok(HistoryOutput {
        browser,
        total_visits,
        matched,
        returned,
        truncated,
        visits,
    })
}

/// Serialize the timeline as pretty JSON.
pub fn render_json(out: &HistoryOutput) -> String {
    serde_json::to_string_pretty(out).unwrap_or_else(|_| "{}".to_string())
}

/// Render the timeline as RFC-4180 CSV (with a `browser` column so exports from
/// several history files merge cleanly in a spreadsheet).
pub fn render_csv(out: &HistoryOutput) -> String {
    let mut s = String::from("browser,timestamp,url,title,visit_type,visit_count\r\n");
    for v in &out.visits {
        let count = v.visit_count.to_string();
        let fields = [
            out.browser.as_str(),
            v.timestamp.as_str(),
            v.url.as_str(),
            v.title.as_str(),
            v.visit_type.as_str(),
            count.as_str(),
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
// Per-browser extraction
// ---------------------------------------------------------------------------

/// A URL row keyed by id: (url, title, visit_count).
type UrlMap = HashMap<i64, (String, String, i64)>;

fn parse_chrome(bytes: &[u8]) -> Result<Vec<Visit>, String> {
    let urls = read_table(bytes, "urls")?;
    let (i_id, i_url) = (
        urls.col_index("id").ok_or("Chrome `urls` table has no `id` column")?,
        urls.col_index("url").ok_or("Chrome `urls` table has no `url` column")?,
    );
    let i_title = urls.col_index("title");
    let i_count = urls.col_index("visit_count");
    let mut map: UrlMap = HashMap::new();
    for row in &urls.rows {
        let Some(id) = row.get(i_id).and_then(Value::as_i64) else {
            continue;
        };
        map.insert(id, (cell_text(row, Some(i_url)), cell_text(row, i_title), cell_i64(row, i_count)));
    }

    let visits = read_table(bytes, "visits")?;
    let v_url = visits.col_index("url").ok_or("Chrome `visits` table has no `url` column")?;
    let v_time = visits
        .col_index("visit_time")
        .ok_or("Chrome `visits` table has no `visit_time` column")?;
    let v_trans = visits.col_index("transition");

    let mut out = Vec::with_capacity(visits.rows.len());
    for row in &visits.rows {
        let uid = row.get(v_url).and_then(Value::as_i64).unwrap_or(i64::MIN);
        let (url, title, visit_count) = map.get(&uid).cloned().unwrap_or_default();
        let unix = webkit_to_unix(cell_i64(row, Some(v_time)));
        let visit_type = chrome_transition(cell_i64(row, v_trans));
        out.push(Visit {
            timestamp: iso8601_utc(unix),
            unix_seconds: unix,
            url,
            title,
            visit_type,
            visit_count,
        });
    }
    Ok(out)
}

fn parse_firefox(bytes: &[u8]) -> Result<Vec<Visit>, String> {
    let places = read_table(bytes, "moz_places")?;
    let (p_id, p_url) = (
        places.col_index("id").ok_or("Firefox `moz_places` table has no `id` column")?,
        places.col_index("url").ok_or("Firefox `moz_places` table has no `url` column")?,
    );
    let p_title = places.col_index("title");
    let p_count = places.col_index("visit_count");
    let mut map: UrlMap = HashMap::new();
    for row in &places.rows {
        let Some(id) = row.get(p_id).and_then(Value::as_i64) else {
            continue;
        };
        map.insert(id, (cell_text(row, Some(p_url)), cell_text(row, p_title), cell_i64(row, p_count)));
    }

    let hv = read_table(bytes, "moz_historyvisits")?;
    let h_place = hv
        .col_index("place_id")
        .ok_or("Firefox `moz_historyvisits` table has no `place_id` column")?;
    let h_date = hv
        .col_index("visit_date")
        .ok_or("Firefox `moz_historyvisits` table has no `visit_date` column")?;
    let h_type = hv.col_index("visit_type");

    let mut out = Vec::with_capacity(hv.rows.len());
    for row in &hv.rows {
        let pid = row.get(h_place).and_then(Value::as_i64).unwrap_or(i64::MIN);
        let (url, title, visit_count) = map.get(&pid).cloned().unwrap_or_default();
        let unix = prtime_to_unix(cell_i64(row, Some(h_date)));
        let visit_type = firefox_visit_type(cell_i64(row, h_type));
        out.push(Visit {
            timestamp: iso8601_utc(unix),
            unix_seconds: unix,
            url,
            title,
            visit_type,
            visit_count,
        });
    }
    Ok(out)
}

fn cell_text(row: &[Value], idx: Option<usize>) -> String {
    idx.and_then(|i| row.get(i)).and_then(Value::as_text).unwrap_or("").to_string()
}

fn cell_i64(row: &[Value], idx: Option<usize>) -> i64 {
    idx.and_then(|i| row.get(i)).and_then(Value::as_i64).unwrap_or(0)
}

/// WebKit/Chrome microseconds (since 1601) → Unix seconds (`0` if absent).
fn webkit_to_unix(micros: i64) -> i64 {
    if micros <= 0 {
        0
    } else {
        micros / 1_000_000 - WEBKIT_OFFSET_SECONDS
    }
}

/// Firefox PRTime microseconds (since 1970) → Unix seconds (`0` if absent).
fn prtime_to_unix(micros: i64) -> i64 {
    if micros <= 0 {
        0
    } else {
        micros / 1_000_000
    }
}

/// Decode a Chrome `transition` value's core page-transition type (low byte).
fn chrome_transition(t: i64) -> String {
    match (t as u64) & 0xff {
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

/// Decode a Firefox `visit_type` code.
fn firefox_visit_type(v: i64) -> String {
    match v {
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
// Date formatting (no chrono — Howard Hinnant's civil-from-days algorithm)
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

#[cfg(test)]
mod tests {
    use super::*;

    // Real SQLite files built by `tests/fixtures/gen_fixtures.py`.
    const CHROME: &[u8] = include_bytes!("../tests/fixtures/chrome_history.db");
    const FIREFOX: &[u8] = include_bytes!("../tests/fixtures/firefox_places.sqlite");
    const OTHER: &[u8] = include_bytes!("../tests/fixtures/other.db");

    #[test]
    fn chrome_timeline_newest_first_with_conversions() {
        let out = parse_history(CHROME, &Options::default()).unwrap();
        assert_eq!(out.browser, "Chrome/Edge");
        assert_eq!(out.total_visits, 3);
        assert_eq!(out.matched, 3);
        assert_eq!(out.returned, 3);
        assert!(!out.truncated);

        // Newest first: RELOAD (2024-02-01) → LINK (2024-01-15) → TYPED (2023-12-25).
        let v0 = &out.visits[0];
        assert_eq!(v0.timestamp, "2024-02-01T12:00:00Z");
        assert_eq!(v0.unix_seconds, 1_706_788_800);
        assert_eq!(v0.url, "https://www.rust-lang.org/");
        assert_eq!(v0.title, "Rust Programming Language");
        assert_eq!(v0.visit_type, "reload");
        assert_eq!(v0.visit_count, 5);

        assert_eq!(out.visits[1].timestamp, "2024-01-15T10:30:00Z");
        assert_eq!(out.visits[1].visit_type, "link");

        // Transition 0x30000001 masks to core type 1 = "typed".
        let v2 = &out.visits[2];
        assert_eq!(v2.timestamp, "2023-12-25T08:00:00Z");
        assert_eq!(v2.visit_type, "typed");
        assert_eq!(v2.url, "https://news.example.com/article");
    }

    #[test]
    fn firefox_timeline_newest_first_with_conversions() {
        let out = parse_history(FIREFOX, &Options::default()).unwrap();
        assert_eq!(out.browser, "Firefox");
        assert_eq!(out.total_visits, 3);

        // Newest first: BOOKMARK (2024-03-11) → LINK (2024-03-10) → TYPED (2022-06-01).
        let v0 = &out.visits[0];
        assert_eq!(v0.timestamp, "2024-03-11T18:45:00Z");
        assert_eq!(v0.unix_seconds, 1_710_182_700);
        assert_eq!(v0.url, "https://www.mozilla.org/");
        assert_eq!(v0.title, "Mozilla");
        assert_eq!(v0.visit_type, "bookmark");
        assert_eq!(v0.visit_count, 3);

        assert_eq!(out.visits[1].timestamp, "2024-03-10T09:15:00Z");
        assert_eq!(out.visits[1].visit_type, "link");

        assert_eq!(out.visits[2].timestamp, "2022-06-01T00:00:00Z");
        assert_eq!(out.visits[2].visit_type, "typed");
        assert_eq!(out.visits[2].url, "https://blog.example.org/post");
        assert_eq!(out.visits[2].visit_count, 1);
    }

    #[test]
    fn search_filters_url_or_title_case_insensitive() {
        let out = parse_history(
            CHROME,
            &Options {
                search: Some("NEWS".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.total_visits, 3);
        assert_eq!(out.matched, 1);
        assert_eq!(out.returned, 1);
        assert_eq!(out.visits[0].url, "https://news.example.com/article");
    }

    #[test]
    fn order_oldest_reverses() {
        let out = parse_history(
            CHROME,
            &Options {
                order: Order::Oldest,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.visits[0].timestamp, "2023-12-25T08:00:00Z");
        assert_eq!(out.visits[2].timestamp, "2024-02-01T12:00:00Z");
    }

    #[test]
    fn limit_truncates_and_flags() {
        let out = parse_history(
            CHROME,
            &Options {
                limit: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.total_visits, 3);
        assert_eq!(out.matched, 3);
        assert_eq!(out.returned, 1);
        assert!(out.truncated);
        assert_eq!(out.visits.len(), 1);
        assert_eq!(out.visits[0].timestamp, "2024-02-01T12:00:00Z");
    }

    #[test]
    fn render_csv_has_header_and_browser_column() {
        let out = parse_history(FIREFOX, &Options::default()).unwrap();
        let csv = render_csv(&out);
        assert!(csv.starts_with("browser,timestamp,url,title,visit_type,visit_count\r\n"));
        assert!(csv.contains("Firefox,2024-03-11T18:45:00Z,https://www.mozilla.org/,Mozilla,bookmark,3\r\n"));
    }

    #[test]
    fn render_json_round_trips() {
        let out = parse_history(CHROME, &Options::default()).unwrap();
        let json = render_json(&out);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["browser"], "Chrome/Edge");
        assert_eq!(v["visits"].as_array().unwrap().len(), 3);
        assert_eq!(v["visits"][0]["visit_type"], "reload");
    }

    #[test]
    fn rejects_non_sqlite_bytes() {
        let err = parse_history(b"this is not a database", &Options::default()).unwrap_err();
        assert!(err.contains("SQLite"), "got: {err}");
    }

    #[test]
    fn rejects_non_history_sqlite() {
        let err = parse_history(OTHER, &Options::default()).unwrap_err();
        assert!(err.contains("not a recognized browser history"), "got: {err}");
        assert!(err.contains("notes"), "should list the found tables; got: {err}");
    }

    #[test]
    fn order_parse_accepts_synonyms() {
        assert_eq!(Order::parse("newest").unwrap(), Order::Newest);
        assert_eq!(Order::parse("OLDEST").unwrap(), Order::Oldest);
        assert_eq!(Order::parse("asc").unwrap(), Order::Oldest);
        assert!(Order::parse("sideways").is_err());
    }

    #[test]
    fn iso_formatting_and_epoch_math() {
        assert_eq!(iso8601_utc(0), "");
        assert_eq!(iso8601_utc(1_706_788_800), "2024-02-01T12:00:00Z");
        assert_eq!(iso8601_utc(1), "1970-01-01T00:00:01Z");
        // A WebKit timestamp corresponding exactly to a known Unix second.
        assert_eq!(webkit_to_unix((1_706_788_800 + WEBKIT_OFFSET_SECONDS) * 1_000_000), 1_706_788_800);
        assert_eq!(prtime_to_unix(1_710_182_700_000_000), 1_710_182_700);
    }
}
