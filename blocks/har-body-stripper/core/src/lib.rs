//! har-body-stripper core — remove request/response bodies from a HAR
//! (HTTP Archive) capture to shrink and de-sensitize it. Pure Rust
//! (`serde_json` with `preserve_order`, so the stripped capture keeps the
//! original key order and stays diff-able against the source).
//!
//! What is removed: `request.postData.text` + `request.postData.params`,
//! `response.content.text` + `response.content.encoding`, and the `data`
//! payload of Chrome's `_webSocketMessages` frames (`send` counts as the
//! request side, `receive` as the response side). Everything else — URLs,
//! headers, cookies, timings, `content.size`/`mimeType`, `bodySize` — is
//! left untouched, so the capture stays fully analyzable. Cookie/header
//! redaction is deliberately out of scope (that is a redaction tool's job).

use serde_json::Value;

/// Hard cap on `log.entries` per run.
pub const MAX_ENTRIES: usize = 10_000;

#[derive(Default)]
struct SideStats {
    count: usize,
    bytes: u64,
}

/// Strip bodies from a HAR document.
///
/// * `strip` — `both` (default), `request`, or `response`: which side's
///   bodies to remove.
/// * `only_mime` — comma-separated case-insensitive mimeType substrings
///   (e.g. `image/,font/`); only bodies whose recorded mimeType contains one
///   are stripped. Empty = every body. Bodies with no recorded mimeType (and
///   websocket frames, which have none) are kept when a filter is set.
/// * `min_bytes` — only strip bodies at least this large. Response bodies
///   measure `content.size` when recorded (decoded size), else the stored
///   text length; request bodies measure the stored `postData` text/params
///   length; websocket frames measure the `data` length. 0 = strip all.
/// * `output` — `har` (the stripped capture as JSON) or `summary` (a dry-run
///   report: counts, bytes removed, before/after size).
/// * `pretty` — pretty-print the output HAR (2-space indent). Compact
///   (default) shrinks most — DevTools exports are pretty-printed.
pub fn strip_bodies(
    har: &str,
    strip: &str,
    only_mime: &str,
    min_bytes: u64,
    output: &str,
    pretty: bool,
) -> Result<String, String> {
    if har.trim().is_empty() {
        return Err("no HAR input".into());
    }
    let (strip_req, strip_resp) = match strip {
        "both" => (true, true),
        "request" => (true, false),
        "response" => (false, true),
        other => {
            return Err(format!(
                "unknown strip mode '{other}' (expected both, request, or response)"
            ))
        }
    };
    if output != "har" && output != "summary" {
        return Err(format!("unknown output '{output}' (expected har or summary)"));
    }

    let mut root: Value =
        serde_json::from_str(har).map_err(|e| format!("invalid JSON: {e}"))?;

    let filters: Vec<String> = only_mime
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let mut req = SideStats::default();
    let mut resp = SideStats::default();
    let mut ws = SideStats::default();
    let entry_count;

    {
        let entries = root
            .get_mut("log")
            .and_then(|l| l.get_mut("entries"))
            .and_then(|e| e.as_array_mut())
            .ok_or_else(|| {
                "not a HAR capture: expected a top-level { \"log\": { \"entries\": [ … ] } } \
                 object (browser DevTools → Network tab → \"Save all as HAR\")"
                    .to_string()
            })?;
        if entries.len() > MAX_ENTRIES {
            return Err(format!(
                "too many entries: {} (max {MAX_ENTRIES} entries per run)",
                entries.len()
            ));
        }
        entry_count = entries.len();

        for entry in entries.iter_mut() {
            let Some(entry) = entry.as_object_mut() else {
                continue; // forgiving: skip malformed entries, strip the rest
            };

            if strip_req {
                if let Some(post) = entry
                    .get_mut("request")
                    .and_then(|r| r.get_mut("postData"))
                    .and_then(|p| p.as_object_mut())
                {
                    let mime = post.get("mimeType").and_then(|v| v.as_str()).unwrap_or("");
                    if mime_matches(mime, &filters) {
                        let text_len = post
                            .get("text")
                            .and_then(|v| v.as_str())
                            .map(|s| s.len() as u64)
                            .unwrap_or(0);
                        let params_len: u64 = post
                            .get("params")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .map(|p| {
                                        let n = p
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .map_or(0, str::len);
                                        let v = p
                                            .get("value")
                                            .and_then(|v| v.as_str())
                                            .map_or(0, str::len);
                                        (n + v) as u64
                                    })
                                    .sum()
                            })
                            .unwrap_or(0);
                        let total = text_len + params_len;
                        let has_body =
                            post.contains_key("text") || post.contains_key("params");
                        if has_body && total >= min_bytes {
                            post.remove("text");
                            post.remove("params");
                            req.count += 1;
                            req.bytes += total;
                        }
                    }
                }
            }

            if strip_resp {
                if let Some(content) = entry
                    .get_mut("response")
                    .and_then(|r| r.get_mut("content"))
                    .and_then(|c| c.as_object_mut())
                {
                    let mime = content
                        .get("mimeType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if mime_matches(mime, &filters) {
                        let text_len = content
                            .get("text")
                            .and_then(|v| v.as_str())
                            .map(|s| s.len() as u64)
                            .unwrap_or(0);
                        // The threshold measures the decoded size when
                        // recorded (base64-stored bodies differ from their
                        // decoded size); stats count the stored bytes removed.
                        let measured = content
                            .get("size")
                            .and_then(|v| v.as_i64())
                            .filter(|s| *s >= 0)
                            .map(|s| s as u64)
                            .unwrap_or(text_len);
                        if content.contains_key("text") && measured >= min_bytes {
                            content.remove("text");
                            content.remove("encoding");
                            resp.count += 1;
                            resp.bytes += text_len;
                        }
                    }
                }
            }

            // Chrome's websocket frames carry payloads too (a common
            // sanitizer blind spot). They have no mimeType, so a mime filter
            // never matches them; `send` frames follow the request side,
            // `receive` frames the response side.
            if filters.is_empty() {
                if let Some(msgs) = entry
                    .get_mut("_webSocketMessages")
                    .and_then(|v| v.as_array_mut())
                {
                    for msg in msgs.iter_mut() {
                        let Some(m) = msg.as_object_mut() else { continue };
                        let side = m.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        let in_scope = (side == "send" && strip_req)
                            || (side == "receive" && strip_resp);
                        if !in_scope {
                            continue;
                        }
                        let Some(dlen) = m
                            .get("data")
                            .and_then(|v| v.as_str())
                            .map(|s| s.len() as u64)
                        else {
                            continue;
                        };
                        if dlen >= min_bytes {
                            m.remove("data");
                            ws.count += 1;
                            ws.bytes += dlen;
                        }
                    }
                }
            }
        }
    }

    let out = if pretty {
        serde_json::to_string_pretty(&root)
    } else {
        serde_json::to_string(&root)
    }
    .map_err(|e| format!("serialize failed: {e}"))?;

    if output == "har" {
        return Ok(out);
    }

    let in_len = har.len() as u64;
    let out_len = out.len() as u64;
    let delta = if out_len < in_len {
        let pct = (in_len - out_len) as f64 / in_len as f64 * 100.0;
        format!("{pct:.1}% smaller")
    } else if out_len > in_len {
        let pct = (out_len - in_len) as f64 / in_len as f64 * 100.0;
        format!("{pct:.1}% larger")
    } else {
        "unchanged".to_string()
    };

    Ok(format!(
        "HAR body strip summary\n\
         entries scanned: {entry_count}\n\
         {}\n\
         {}\n\
         {}\n\
         size: {} → {} ({delta})\n\
         Run with output=har to get the stripped capture.",
        stat_line("request bodies stripped", &req),
        stat_line("response bodies stripped", &resp),
        stat_line("websocket payloads stripped", &ws),
        human_size(in_len),
        human_size(out_len),
    ))
}

fn mime_matches(mime: &str, filters: &[String]) -> bool {
    if filters.is_empty() {
        return true;
    }
    let m = mime.to_ascii_lowercase();
    filters.iter().any(|f| m.contains(f.as_str()))
}

fn stat_line(label: &str, s: &SideStats) -> String {
    if s.count == 0 {
        format!("{label}: 0")
    } else {
        format!("{label}: {} ({})", s.count, human_size(s.bytes))
    }
}

/// `512 B`, `12.1 KB`, `4.3 MB`, `1.2 GB` (1024-based, one decimal).
fn human_size(b: u64) -> String {
    const KB: f64 = 1024.0;
    let bf = b as f64;
    if bf < KB {
        format!("{b} B")
    } else if bf < KB * KB {
        format!("{:.1} KB", bf / KB)
    } else if bf < KB * KB * KB {
        format!("{:.1} MB", bf / (KB * KB))
    } else {
        format!("{:.1} GB", bf / (KB * KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small two-entry HAR: a POST with a form body + JSON response and
    /// websocket frames, then a GET with a base64 image response.
    fn sample() -> String {
        r#"{"log":{"version":"1.2","creator":{"name":"t","version":"1"},"entries":[
            {"request":{"method":"POST","url":"https://x.test/login",
                "postData":{"mimeType":"application/x-www-form-urlencoded","text":"user=alice&pass=hunter2"}},
             "response":{"status":200,"content":{"size":16,"mimeType":"application/json","text":"{\"token\":\"abc\"}"}},
             "_webSocketMessages":[
                {"type":"send","time":1.0,"opcode":1,"data":"hello-ws-token"},
                {"type":"receive","time":2.0,"opcode":1,"data":"ws-reply"}]},
            {"request":{"method":"GET","url":"https://x.test/logo.png"},
             "response":{"status":200,"content":{"size":9000,"mimeType":"image/png","encoding":"base64","text":"aGVsbG8hIQ=="}}}
        ]}}"#
            .to_string()
    }

    #[test]
    fn strips_both_sides_by_default() {
        let out = strip_bodies(&sample(), "both", "", 0, "har", false).unwrap();
        assert!(!out.contains("hunter2"), "request body must be gone");
        assert!(!out.contains("abc"), "response body must be gone");
        assert!(!out.contains("aGVsbG8hIQ=="), "base64 body must be gone");
        assert!(!out.contains("hello-ws-token") && !out.contains("ws-reply"));
        assert!(!out.contains("\"encoding\""), "encoding goes with the body");
        // Metadata survives so the capture stays analyzable.
        assert!(out.contains("\"size\":9000"));
        assert!(out.contains("\"mimeType\":\"image/png\""));
        assert!(out.contains("\"mimeType\":\"application/x-www-form-urlencoded\""));
        assert!(out.contains("\"opcode\":1"), "ws frame metadata kept");
        assert!(out.contains("https://x.test/login"));
    }

    #[test]
    fn request_only_keeps_response_bodies() {
        let out = strip_bodies(&sample(), "request", "", 0, "har", false).unwrap();
        assert!(!out.contains("hunter2"));
        assert!(out.contains("aGVsbG8hIQ=="), "response bodies must survive");
        assert!(!out.contains("hello-ws-token"), "send frame follows request side");
        assert!(out.contains("ws-reply"), "receive frame survives");
    }

    #[test]
    fn response_only_keeps_request_bodies() {
        let out = strip_bodies(&sample(), "response", "", 0, "har", false).unwrap();
        assert!(out.contains("hunter2"), "request bodies must survive");
        assert!(!out.contains("aGVsbG8hIQ=="));
        assert!(out.contains("hello-ws-token"));
        assert!(!out.contains("ws-reply"));
    }

    #[test]
    fn mime_filter_limits_stripping() {
        let out = strip_bodies(&sample(), "both", "image/,font/", 0, "har", false).unwrap();
        assert!(!out.contains("aGVsbG8hIQ=="), "image body stripped");
        assert!(out.contains("hunter2"), "form body kept (mime not matched)");
        assert!(out.contains("abc"), "json body kept");
        assert!(out.contains("hello-ws-token"), "ws frames kept under a mime filter");
    }

    #[test]
    fn min_bytes_keeps_small_bodies() {
        // Response 1 has decoded size 16, response 2 has 9000; the request
        // body is 23 stored bytes; ws frames are 14 and 8.
        let out = strip_bodies(&sample(), "both", "", 100, "har", false).unwrap();
        assert!(out.contains("abc"), "16-byte body kept under min_bytes=100");
        assert!(out.contains("hunter2"), "23-byte request body kept");
        assert!(out.contains("hello-ws-token"), "small ws frame kept");
        assert!(!out.contains("aGVsbG8hIQ=="), "9000-byte (decoded) body stripped");
    }

    #[test]
    fn summary_reports_counts_and_sizes() {
        let har = sample();
        let out = strip_bodies(&har, "both", "", 0, "summary", false).unwrap();
        assert!(out.starts_with("HAR body strip summary\n"));
        assert!(out.contains("entries scanned: 2"));
        // request: one postData text of 23 bytes
        assert!(out.contains("request bodies stripped: 1 (23 B)"), "got:\n{out}");
        // responses: 15 stored + 12 stored base64 = 27 B over 2 bodies
        assert!(out.contains("response bodies stripped: 2 (27 B)"), "got:\n{out}");
        // ws: 14 + 8 = 22 B over 2 frames
        assert!(out.contains("websocket payloads stripped: 2 (22 B)"), "got:\n{out}");
        assert!(out.contains("% smaller"), "got:\n{out}");
        assert!(out.trim_end().ends_with("Run with output=har to get the stripped capture."));
    }

    #[test]
    fn pretty_and_compact_serialization() {
        let compact = strip_bodies(&sample(), "both", "", 0, "har", false).unwrap();
        assert!(!compact.contains('\n'), "compact output is one line");
        let pretty = strip_bodies(&sample(), "both", "", 0, "har", true).unwrap();
        assert!(pretty.contains("\n  \"log\""), "pretty output is 2-space indented");
    }

    #[test]
    fn key_order_is_preserved() {
        let har = r#"{"log":{"version":"1.2","creator":{"name":"t","version":"1"},"pages":[],"entries":[]}}"#;
        let out = strip_bodies(har, "both", "", 0, "har", false).unwrap();
        let v = out.find("\"version\"").unwrap();
        let c = out.find("\"creator\"").unwrap();
        let p = out.find("\"pages\"").unwrap();
        let e = out.find("\"entries\"").unwrap();
        assert!(v < c && c < p && p < e, "original key order must survive: {out}");
    }

    #[test]
    fn rejects_non_json() {
        let err = strip_bodies("not json", "both", "", 0, "har", false).unwrap_err();
        assert!(err.starts_with("invalid JSON:"), "{err}");
    }

    #[test]
    fn rejects_non_har_json() {
        let err = strip_bodies(r#"{"foo":1}"#, "both", "", 0, "har", false).unwrap_err();
        assert!(err.starts_with("not a HAR capture:"), "{err}");
    }

    #[test]
    fn rejects_empty_and_bad_modes() {
        assert_eq!(
            strip_bodies("  ", "both", "", 0, "har", false).unwrap_err(),
            "no HAR input"
        );
        let err = strip_bodies(&sample(), "everything", "", 0, "har", false).unwrap_err();
        assert!(err.contains("unknown strip mode 'everything'"), "{err}");
        let err = strip_bodies(&sample(), "both", "", 0, "csv", false).unwrap_err();
        assert!(err.contains("unknown output 'csv'"), "{err}");
    }

    #[test]
    fn entry_cap_boundary_at_and_over() {
        let entry = r#"{"request":{"method":"GET","url":"https://x.test/"},"response":{"status":200,"content":{"size":1,"mimeType":"text/plain","text":"x"}}}"#;
        let at = format!(
            r#"{{"log":{{"entries":[{}]}}}}"#,
            vec![entry; MAX_ENTRIES].join(",")
        );
        let out = strip_bodies(&at, "both", "", 0, "summary", false).unwrap();
        assert!(out.contains(&format!("entries scanned: {MAX_ENTRIES}")));
        assert!(out.contains(&format!("response bodies stripped: {MAX_ENTRIES} ")));

        let over = format!(
            r#"{{"log":{{"entries":[{}]}}}}"#,
            vec![entry; MAX_ENTRIES + 1].join(",")
        );
        let err = strip_bodies(&over, "both", "", 0, "summary", false).unwrap_err();
        assert_eq!(
            err,
            format!(
                "too many entries: {} (max {MAX_ENTRIES} entries per run)",
                MAX_ENTRIES + 1
            )
        );
    }

    #[test]
    fn missing_mime_kept_when_filter_set() {
        let har = r#"{"log":{"entries":[{"response":{"status":200,"content":{"size":4,"text":"body"}}}]}}"#;
        let out = strip_bodies(har, "both", "text/", 0, "har", false).unwrap();
        assert!(out.contains("\"text\":\"body\""), "no-mime body kept under a filter");
        let out = strip_bodies(har, "both", "", 0, "har", false).unwrap();
        assert!(!out.contains("\"text\":\"body\""), "no-mime body stripped without a filter");
    }
}
