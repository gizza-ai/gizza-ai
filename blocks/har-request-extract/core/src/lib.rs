//! har-request-extract core — pull the request list (method, URL, status,
//! type, size, timing) out of a HAR (HTTP Archive) capture. Pure Rust
//! (serde/serde_json only). Deliberately FORGIVING about per-entry shape —
//! unlike a validator, an extractor should still list what it can from a
//! sloppy exporter's file; only non-JSON / non-HAR input is an error.

use serde::Serialize;
use serde_json::Value;

/// One extracted request row. `index` is the 1-based position in the
/// ORIGINAL capture, so rows stay cross-referenceable after filtering/sorting.
struct Entry {
    index: usize,
    method: String,
    url: String,
    status: i64,
    status_text: String,
    /// `response.content.mimeType` with any `; charset=…` parameters stripped.
    mime: String,
    /// Wire size in bytes: `response.bodySize` when ≥ 0, else Chrome's
    /// `response._transferSize`, else decoded `content.size`.
    size: Option<i64>,
    /// Total request time in ms (`entry.time`); negative/-1 means unknown.
    time: Option<f64>,
    started: String,
}

/// JSON output row — field order here is the emitted key order.
#[derive(Serialize)]
struct JsonRow<'a> {
    index: usize,
    method: &'a str,
    url: &'a str,
    status: i64,
    status_text: &'a str,
    mime_type: Option<&'a str>,
    size_bytes: Option<i64>,
    time_ms: Option<f64>,
    started: Option<&'a str>,
}

/// Extract the request list from a HAR document.
///
/// * `format` — `table` (aligned text + summary line), `csv`, `json`, `urls`.
/// * `status` — `all`, `2xx`, `3xx`, `4xx`, `5xx`, or `errors`
///   (4xx + 5xx + failed requests recorded with status 0).
/// * `method` — case-insensitive exact HTTP-method filter; empty = all.
/// * `url_contains` — case-insensitive URL substring filter; empty = all.
/// * `sort` — `order` (capture order), `slowest` (time desc), `largest`
///   (size desc). Sorting is stable; unknown values sort last.
pub fn extract(
    har: &str,
    format: &str,
    status: &str,
    method: &str,
    url_contains: &str,
    sort: &str,
) -> Result<String, String> {
    if har.trim().is_empty() {
        return Err("no HAR input".into());
    }
    let format = format.trim();
    if !matches!(format, "table" | "csv" | "json" | "urls") {
        return Err(format!(
            "unknown format \"{format}\" (use table, csv, json, or urls)"
        ));
    }
    let status = status.trim();
    if !matches!(status, "all" | "2xx" | "3xx" | "4xx" | "5xx" | "errors") {
        return Err(format!(
            "unknown status filter \"{status}\" (use all, 2xx, 3xx, 4xx, 5xx, or errors)"
        ));
    }
    let sort = sort.trim();
    if !matches!(sort, "order" | "slowest" | "largest") {
        return Err(format!(
            "unknown sort \"{sort}\" (use order, slowest, or largest)"
        ));
    }

    let root: Value = serde_json::from_str(har).map_err(|e| format!("invalid JSON: {e}"))?;
    let log = root
        .get("log")
        .and_then(Value::as_object)
        .ok_or("not a HAR file: missing top-level \"log\" object (a HAR is { \"log\": … })")?;
    let entries = log
        .get("entries")
        .and_then(Value::as_array)
        .ok_or("not a HAR file: \"log.entries\" is missing or not an array")?;

    let all: Vec<Entry> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| parse_entry(i + 1, e))
        .collect();
    let total = all.len();

    let method = method.trim();
    let needle = url_contains.trim().to_lowercase();
    let mut rows: Vec<&Entry> = all
        .iter()
        .filter(|e| {
            status_matches(status, e.status)
                && (method.is_empty() || e.method.eq_ignore_ascii_case(method))
                && (needle.is_empty() || e.url.to_lowercase().contains(&needle))
        })
        .collect();

    match sort {
        "slowest" => {
            rows.sort_by(|a, b| b.time.unwrap_or(-1.0).total_cmp(&a.time.unwrap_or(-1.0)))
        }
        "largest" => rows.sort_by(|a, b| b.size.unwrap_or(-1).cmp(&a.size.unwrap_or(-1))),
        _ => {} // "order" — keep capture order
    }

    Ok(match format {
        "csv" => render_csv(&rows),
        "json" => render_json(&rows)?,
        "urls" => rows
            .iter()
            .map(|e| e.url.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => render_table(&rows, total),
    })
}

fn parse_entry(index: usize, e: &Value) -> Entry {
    let req = e.get("request");
    let resp = e.get("response");
    let mime = resp
        .and_then(|r| r.get("content"))
        .and_then(|c| c.get("mimeType"))
        .and_then(Value::as_str)
        .map(|m| m.split(';').next().unwrap_or("").trim().to_string())
        .unwrap_or_default();
    Entry {
        index,
        method: req
            .and_then(|r| r.get("method"))
            .and_then(Value::as_str)
            .unwrap_or("-")
            .to_string(),
        url: req
            .and_then(|r| r.get("url"))
            .and_then(Value::as_str)
            .unwrap_or("-")
            .to_string(),
        status: resp
            .and_then(|r| r.get("status"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        status_text: resp
            .and_then(|r| r.get("statusText"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        mime,
        size: pick_size(resp),
        time: e.get("time").and_then(Value::as_f64).filter(|t| *t >= 0.0),
        started: e
            .get("startedDateTime")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    }
}

/// Wire size preference: spec `bodySize` (≥ 0), then Chrome's `_transferSize`,
/// then decoded `content.size`. `-1` means "unknown" in HAR and is skipped.
fn pick_size(resp: Option<&Value>) -> Option<i64> {
    let r = resp?;
    for key in ["bodySize", "_transferSize"] {
        if let Some(n) = r.get(key).and_then(Value::as_i64) {
            if n >= 0 {
                return Some(n);
            }
        }
    }
    r.get("content")
        .and_then(|c| c.get("size"))
        .and_then(Value::as_i64)
        .filter(|n| *n >= 0)
}

fn status_matches(class: &str, status: i64) -> bool {
    match class {
        "2xx" => (200..300).contains(&status),
        "3xx" => (300..400).contains(&status),
        "4xx" => (400..500).contains(&status),
        "5xx" => (500..600).contains(&status),
        "errors" => status == 0 || (400..600).contains(&status),
        _ => true, // "all"
    }
}

/// `512 B`, `12.1 KB`, `4.3 MB`, `1.2 GB` (1024-based, one decimal).
fn human_bytes(n: i64) -> String {
    const KB: f64 = 1024.0;
    let f = n as f64;
    if f < KB {
        format!("{n} B")
    } else if f < KB * KB {
        format!("{:.1} KB", f / KB)
    } else if f < KB * KB * KB {
        format!("{:.1} MB", f / (KB * KB))
    } else {
        format!("{:.1} GB", f / (KB * KB * KB))
    }
}

fn fmt_time(t: f64) -> String {
    format!("{} ms", t.round() as i64)
}

fn render_table(rows: &[&Entry], total: usize) -> String {
    let transferred: i64 = rows.iter().filter_map(|e| e.size).sum();
    let noun = if total == 1 { "request" } else { "requests" };
    let mut out = format!(
        "{} of {} {} · {} transferred\n",
        rows.len(),
        total,
        noun,
        human_bytes(transferred)
    );
    if rows.is_empty() {
        out.push_str("\nNo requests match the filters.");
        return out;
    }

    const HEADERS: [&str; 7] = ["#", "METHOD", "STATUS", "TYPE", "SIZE", "TIME", "URL"];
    let cells: Vec<[String; 7]> = rows
        .iter()
        .map(|e| {
            [
                e.index.to_string(),
                e.method.clone(),
                e.status.to_string(),
                if e.mime.is_empty() { "-".into() } else { e.mime.clone() },
                e.size.map(human_bytes).unwrap_or_else(|| "-".into()),
                e.time.map(fmt_time).unwrap_or_else(|| "-".into()),
                e.url.clone(),
            ]
        })
        .collect();

    let mut widths = [0usize; 7];
    for (i, h) in HEADERS.iter().enumerate() {
        widths[i] = h.chars().count();
    }
    for row in &cells {
        for (i, c) in row.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }

    let render_row = |row: &[String]| -> String {
        let mut line = String::new();
        for (i, c) in row.iter().enumerate() {
            if i == row.len() - 1 {
                line.push_str(c); // last column (URL): no trailing padding
            } else {
                let pad = widths[i] - c.chars().count();
                line.push_str(c);
                line.extend(std::iter::repeat(' ').take(pad + 2));
            }
        }
        line
    };
    out.push('\n');
    out.push_str(&render_row(&HEADERS.map(String::from)));
    for row in &cells {
        out.push('\n');
        out.push_str(&render_row(row));
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(rows: &[&Entry]) -> String {
    let mut out =
        String::from("index,method,url,status,status_text,mime_type,size_bytes,time_ms,started");
    for e in rows {
        out.push('\n');
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}",
            e.index,
            csv_field(&e.method),
            csv_field(&e.url),
            e.status,
            csv_field(&e.status_text),
            csv_field(&e.mime),
            e.size.map(|n| n.to_string()).unwrap_or_default(),
            e.time.map(|t| format!("{t}")).unwrap_or_default(),
            csv_field(&e.started),
        ));
    }
    out
}

fn render_json(rows: &[&Entry]) -> Result<String, String> {
    let arr: Vec<JsonRow> = rows
        .iter()
        .map(|e| JsonRow {
            index: e.index,
            method: &e.method,
            url: &e.url,
            status: e.status,
            status_text: &e.status_text,
            mime_type: (!e.mime.is_empty()).then_some(e.mime.as_str()),
            size_bytes: e.size,
            time_ms: e.time,
            started: (!e.started.is_empty()).then_some(e.started.as_str()),
        })
        .collect();
    serde_json::to_string_pretty(&arr).map_err(|e| format!("JSON encode error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three-entry capture: an HTML page, a slow big JSON API call, a 404 image.
    fn sample() -> String {
        r#"{
          "log": {
            "version": "1.2",
            "creator": { "name": "WebInspector", "version": "537.36" },
            "entries": [
              {
                "startedDateTime": "2024-01-01T00:00:00.000Z",
                "time": 102.5,
                "request": { "method": "GET", "url": "https://example.com/", "headers": [] },
                "response": {
                  "status": 200, "statusText": "OK", "headers": [],
                  "content": { "size": 5120, "mimeType": "text/html; charset=utf-8" },
                  "bodySize": 2048
                }
              },
              {
                "startedDateTime": "2024-01-01T00:00:01.000Z",
                "time": 812,
                "request": { "method": "POST", "url": "https://example.com/api/search", "headers": [] },
                "response": {
                  "status": 200, "statusText": "OK", "headers": [],
                  "content": { "size": 20480, "mimeType": "application/json" },
                  "bodySize": -1, "_transferSize": 10240
                }
              },
              {
                "startedDateTime": "2024-01-01T00:00:02.000Z",
                "time": 54,
                "request": { "method": "GET", "url": "https://cdn.example.com/logo.png", "headers": [] },
                "response": {
                  "status": 404, "statusText": "Not Found", "headers": [],
                  "content": { "size": 0, "mimeType": "image/png" },
                  "bodySize": 512
                }
              }
            ]
          }
        }"#
        .to_string()
    }

    #[test]
    fn table_lists_all_requests_with_summary() {
        let out = extract(&sample(), "table", "all", "", "", "order").unwrap();
        assert_eq!(
            out,
            "3 of 3 requests · 12.5 KB transferred\n\
             \n\
             #  METHOD  STATUS  TYPE              SIZE     TIME    URL\n\
             1  GET     200     text/html         2.0 KB   103 ms  https://example.com/\n\
             2  POST    200     application/json  10.0 KB  812 ms  https://example.com/api/search\n\
             3  GET     404     image/png         512 B    54 ms   https://cdn.example.com/logo.png"
        );
    }

    #[test]
    fn status_and_method_and_url_filters_compose() {
        let out = extract(&sample(), "table", "2xx", "", "", "order").unwrap();
        assert!(out.starts_with("2 of 3 requests"));
        assert!(!out.contains("logo.png"));

        let out = extract(&sample(), "table", "errors", "", "", "order").unwrap();
        assert!(out.starts_with("1 of 3 requests"));
        assert!(out.contains("logo.png"));

        let out = extract(&sample(), "table", "all", "post", "", "order").unwrap();
        assert!(out.starts_with("1 of 3 requests"));
        assert!(out.contains("/api/search"));

        let out = extract(&sample(), "table", "all", "", "CDN.example", "order").unwrap();
        assert!(out.starts_with("1 of 3 requests"));
        assert!(out.contains("logo.png"));

        let out = extract(&sample(), "table", "4xx", "POST", "", "order").unwrap();
        assert!(out.starts_with("0 of 3 requests"));
        assert!(out.contains("No requests match the filters."));

        assert!(extract(&sample(), "table", "3xx", "", "", "order")
            .unwrap()
            .starts_with("0 of 3 requests"));
        assert!(extract(&sample(), "table", "5xx", "", "", "order")
            .unwrap()
            .starts_with("0 of 3 requests"));
    }

    #[test]
    fn sort_slowest_and_largest_keep_capture_index() {
        let out = extract(&sample(), "urls", "all", "", "", "slowest").unwrap();
        assert_eq!(
            out,
            "https://example.com/api/search\nhttps://example.com/\nhttps://cdn.example.com/logo.png"
        );
        let out = extract(&sample(), "csv", "all", "", "", "largest").unwrap();
        let indices: Vec<&str> = out
            .lines()
            .skip(1)
            .map(|l| l.split(',').next().unwrap())
            .collect();
        assert_eq!(indices, ["2", "1", "3"], "capture index survives sorting");
    }

    #[test]
    fn csv_has_header_and_escapes_fields() {
        let har = r#"{ "log": { "entries": [ {
            "startedDateTime": "2024-01-01T00:00:00.000Z",
            "time": 5,
            "request": { "method": "GET", "url": "https://example.com/?q=a,b" },
            "response": { "status": 200, "statusText": "OK, kind of",
                          "content": { "mimeType": "text/plain" }, "bodySize": 3 }
        } ] } }"#;
        let out = extract(har, "csv", "all", "", "", "order").unwrap();
        assert_eq!(
            out,
            "index,method,url,status,status_text,mime_type,size_bytes,time_ms,started\n\
             1,GET,\"https://example.com/?q=a,b\",200,\"OK, kind of\",text/plain,3,5,2024-01-01T00:00:00.000Z"
        );
    }

    #[test]
    fn json_rows_carry_typed_fields_and_nulls() {
        let out = extract(&sample(), "json", "4xx", "", "", "order").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 1);
        assert_eq!(v[0]["index"], 3);
        assert_eq!(v[0]["method"], "GET");
        assert_eq!(v[0]["status"], 404);
        assert_eq!(v[0]["mime_type"], "image/png");
        assert_eq!(v[0]["size_bytes"], 512);
        assert_eq!(v[0]["time_ms"], 54.0);
        // A bare entry with nothing but a URL still extracts, with nulls.
        let bare = r#"{ "log": { "entries": [ { "request": { "url": "https://x.test/" } } ] } }"#;
        let out = extract(bare, "json", "all", "", "", "order").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["status"], 0);
        assert!(v[0]["size_bytes"].is_null());
        assert!(v[0]["time_ms"].is_null());
        assert!(v[0]["started"].is_null());
    }

    #[test]
    fn errors_on_bad_input() {
        assert_eq!(
            extract("", "table", "all", "", "", "order").unwrap_err(),
            "no HAR input"
        );
        assert!(extract("not json", "table", "all", "", "", "order")
            .unwrap_err()
            .starts_with("invalid JSON:"));
        assert!(extract("{\"notlog\":1}", "table", "all", "", "", "order")
            .unwrap_err()
            .contains("missing top-level \"log\""));
        assert!(extract("{\"log\":{}}", "table", "all", "", "", "order")
            .unwrap_err()
            .contains("log.entries"));
        assert!(extract(&sample(), "yaml", "all", "", "", "order")
            .unwrap_err()
            .starts_with("unknown format"));
        assert!(extract(&sample(), "table", "6xx", "", "", "order")
            .unwrap_err()
            .starts_with("unknown status filter"));
        assert!(extract(&sample(), "table", "all", "", "", "biggest")
            .unwrap_err()
            .starts_with("unknown sort"));
    }
}
