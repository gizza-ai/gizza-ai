//! gizza-ai/log-merger — pure core.
//!
//! Interleaves multiple pasted log sources into one unified timeline, sorted by
//! each line's parsed timestamp, with every line prefixed by a `[source]` tag.
//!
//! Sources inside the single pasted blob are delimited by header lines:
//!   `--- app.log ---`  ·  `=== app.log ===`  ·  `==> app.log <==` (GNU tail)  ·  `# app.log`
//! or, in `blank` mode, by blank-line-separated blocks (numbered `source1`, …).
//!
//! Timestamps are parsed from anywhere in a line (ISO 8601 / RFC 3339,
//! `YYYY-MM-DD HH:MM:SS[.sss]`, Apache/nginx `10/Oct/2000:13:55:36 -0700`,
//! syslog `Mon DD HH:MM:SS`, and unix epoch seconds/milliseconds). A line with
//! no recognizable timestamp inherits the previous line's timestamp in its
//! source (so wrapped messages / stack-trace continuations stay attached);
//! leading untimestamped lines borrow their source's first timestamp.
//!
//! No system clock is read — only parsing/formatting — so this crate stays
//! instantiable under both wafer (wasm32-wasip1) and wasm-pack
//! (wasm32-unknown-unknown) with no getrandom/js dependency.

use chrono::{DateTime, NaiveDateTime};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

/// Hard cap on total input lines (across all sources).
pub const MAX_LINES: usize = 50_000;

// ---------------------------------------------------------------------------
// Timestamp extraction → epoch seconds (UTC).
// ---------------------------------------------------------------------------

// ISO 8601 / RFC 3339: 2024-01-01T00:00:00(.sss)?(Z|±hh:mm)? — the "T" may be a space.
static RE_ISO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?").unwrap()
});
// Apache/nginx access-log date: 10/Oct/2000:13:55:36 -0700
static RE_APACHE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\d{2}/[A-Za-z]{3}/\d{4}:\d{2}:\d{2}:\d{2} [+-]\d{4}").unwrap());
// Syslog RFC 3164 (no year): Oct 11 22:14:15  /  Oct  1 22:14:15
static RE_SYSLOG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Z][a-z]{2}\s+\d{1,2} \d{2}:\d{2}:\d{2}").unwrap());

fn parse_iso(s: &str) -> Option<i64> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp());
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(ndt.and_utc().timestamp());
        }
    }
    None
}

/// Find the first recognizable timestamp anywhere in `line` → epoch seconds.
fn line_ts(line: &str) -> Option<i64> {
    // 1. ISO 8601 / RFC 3339 (also matches a bracketed `[2024-01-01 00:00:00]`).
    if let Some(m) = RE_ISO.find(line) {
        if let Some(ts) = parse_iso(m.as_str()) {
            return Some(ts);
        }
    }
    // 2. Apache/nginx access-log date.
    if let Some(m) = RE_APACHE.find(line) {
        if let Ok(dt) = DateTime::parse_from_str(m.as_str(), "%d/%b/%Y:%H:%M:%S %z") {
            return Some(dt.timestamp());
        }
    }
    // 3. Syslog date without a year → pin to a fixed reference year (1970) so
    //    relative ordering still works.
    if let Some(m) = RE_SYSLOG.find(line) {
        let norm = m.as_str().split_whitespace().collect::<Vec<_>>().join(" ");
        if let Ok(ndt) = NaiveDateTime::parse_from_str(&format!("1970 {norm}"), "%Y %b %e %H:%M:%S")
        {
            return Some(ndt.and_utc().timestamp());
        }
    }
    // 4. Bare unix epoch as the FIRST token (seconds or milliseconds).
    let tok = line.trim_start().split_whitespace().next().unwrap_or("");
    if tok.len() >= 10 && tok.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(n) = tok.parse::<i64>() {
            return Some(if tok.len() >= 13 { n / 1000 } else { n });
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Source splitting.
// ---------------------------------------------------------------------------

/// If `line` is a source-header line, return the source name it introduces.
/// Recognized: `--- name ---`, `=== name ===`, GNU tail `==> name <==`, and
/// markdown `# name` (one to six leading `#`).
fn header_name(line: &str) -> Option<String> {
    let t = line.trim();
    if t.len() < 2 {
        return None;
    }
    // GNU `tail file1 file2` banner: ==> name <==
    if t.starts_with("==>") && t.ends_with("<==") && t.len() >= 6 {
        let inner = t[3..t.len() - 3].trim();
        if !inner.is_empty() {
            return Some(inner.to_string());
        }
    }
    // Markdown header: one-or-more '#' then whitespace then a name.
    if t.starts_with('#') {
        let name = t.trim_start_matches('#');
        // Require a space after the hashes so a bare "#tag" line isn't a header.
        if name.starts_with(char::is_whitespace) {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    // Fenced: --- name --- / === name === (≥3 of the same fence char both ends).
    for fence in ['-', '='] {
        let f3: String = std::iter::repeat(fence).take(3).collect();
        if t.starts_with(&f3) && t.ends_with(&f3) && t.len() >= 6 {
            let inner = t.trim_matches(fence).trim();
            if !inner.is_empty() {
                return Some(inner.to_string());
            }
        }
    }
    None
}

/// Split the raw blob into `(source_name, lines)` groups.
fn split_sources(logs: &str, blank_mode: bool) -> Vec<(String, Vec<String>)> {
    let lines: Vec<&str> = logs.lines().collect();

    if blank_mode {
        // Blank-line-separated blocks → source1, source2, …
        let mut out: Vec<(String, Vec<String>)> = Vec::new();
        let mut cur: Vec<String> = Vec::new();
        for &line in &lines {
            if line.trim().is_empty() {
                if !cur.is_empty() {
                    out.push((format!("source{}", out.len() + 1), std::mem::take(&mut cur)));
                }
            } else {
                cur.push(line.to_string());
            }
        }
        if !cur.is_empty() {
            out.push((format!("source{}", out.len() + 1), cur));
        }
        return out;
    }

    // Header mode. Content before the first header (if any) → "log".
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut cur_name: Option<String> = None;
    let mut cur: Vec<String> = Vec::new();
    for &line in &lines {
        if let Some(name) = header_name(line) {
            if cur_name.is_some() || !cur.is_empty() {
                let n = cur_name.take().unwrap_or_else(|| "log".to_string());
                out.push((n, std::mem::take(&mut cur)));
            }
            cur_name = Some(name);
            cur.clear();
        } else {
            cur.push(line.to_string());
        }
    }
    if cur_name.is_some() || !cur.is_empty() {
        let n = cur_name.take().unwrap_or_else(|| "log".to_string());
        out.push((n, cur));
    }
    out
}

// ---------------------------------------------------------------------------
// Merge.
// ---------------------------------------------------------------------------

struct Entry {
    ts: i64,
    source_idx: usize,
    line_idx: usize,
    name_idx: usize,
    text: String,
}

/// Interleave the pasted `logs` into one timeline.
///
/// - `source_mode`: `""`/`"header"` (delimit by header lines) or `"blank"`
///   (blank-line-separated blocks).
/// - `order`: `""`/`"asc"` (oldest first) or `"desc"` (newest first).
/// - `dedupe`: drop later lines that repeat an already-emitted `(timestamp, text)`.
/// - `align`: pad each `[source]` tag to a common width so messages line up.
pub fn merge(
    logs: &str,
    source_mode: &str,
    order: &str,
    dedupe: bool,
    align: bool,
) -> Result<String, String> {
    let blank_mode = match source_mode.trim() {
        "" | "header" => false,
        "blank" => true,
        other => {
            return Err(format!(
                "unknown source_mode '{other}' — expected 'header' or 'blank'"
            ))
        }
    };
    let descending = match order.trim() {
        "" | "asc" => false,
        "desc" => true,
        other => return Err(format!("unknown order '{other}' — expected 'asc' or 'desc'")),
    };

    // Line-count cap (across the whole paste).
    let total_lines = logs.lines().count();
    if total_lines > MAX_LINES {
        return Err(format!(
            "input has {total_lines} lines; the maximum is {MAX_LINES}. Merge fewer or shorter logs."
        ));
    }

    let sources = split_sources(logs, blank_mode);
    // Distinct source names (in first-seen order) for tag alignment width.
    let names: Vec<String> = sources.iter().map(|(n, _)| n.clone()).collect();

    let mut entries: Vec<Entry> = Vec::new();
    let mut any_ts = false;
    let mut any_line = false;

    for (source_idx, (_name, lines)) in sources.iter().enumerate() {
        // Pass 1: parse each line's own timestamp; find the source's first one.
        let per_line: Vec<Option<i64>> = lines.iter().map(|l| line_ts(l)).collect();
        let first_ts = per_line.iter().flatten().next().copied();
        if first_ts.is_some() {
            any_ts = true;
        }
        // Pass 2: carry forward; leading untimestamped lines borrow first_ts.
        // A source with no timestamp at all sorts to the very start (i64::MIN).
        let mut last: Option<i64> = None;
        for (line_idx, line) in lines.iter().enumerate() {
            any_line = true;
            let ts = match per_line[line_idx] {
                Some(t) => {
                    last = Some(t);
                    t
                }
                None => last.or(first_ts).unwrap_or(i64::MIN),
            };
            entries.push(Entry {
                ts,
                source_idx,
                line_idx,
                name_idx: source_idx,
                text: line.clone(),
            });
        }
    }

    if !any_line {
        return Err("No log lines to merge — paste one or more log sources.".to_string());
    }
    if !any_ts {
        return Err(
            "No parseable timestamps found. log-merger orders lines by their timestamp — \
             each source needs at least one line with a recognizable timestamp \
             (ISO 8601, YYYY-MM-DD HH:MM:SS, syslog 'Mon DD HH:MM:SS', Apache date, or unix epoch)."
                .to_string(),
        );
    }

    // Stable sort: (ts, source_idx, line_idx). Ties keep input order in both
    // directions (source, then line) — only the timestamp key flips for desc.
    entries.sort_by(|a, b| {
        let key = if descending {
            b.ts.cmp(&a.ts)
        } else {
            a.ts.cmp(&b.ts)
        };
        key.then(a.source_idx.cmp(&b.source_idx))
            .then(a.line_idx.cmp(&b.line_idx))
    });

    // Tag width for alignment = widest emitted `[name]`.
    let tag_width = if align {
        names.iter().map(|n| n.chars().count() + 2).max().unwrap_or(0)
    } else {
        0
    };

    let mut seen: HashSet<(i64, String)> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for e in &entries {
        if dedupe && !seen.insert((e.ts, e.text.trim().to_string())) {
            continue;
        }
        let tag = format!("[{}]", names[e.name_idx]);
        if align {
            out.push(format!("{tag:<tag_width$} {}", e.text));
        } else {
            out.push(format!("{tag} {}", e.text));
        }
    }

    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_two_sources_by_timestamp() {
        let logs = "--- app ---\n\
                    2024-01-01T00:00:02Z started\n\
                    2024-01-01T00:00:05Z ready\n\
                    --- db ---\n\
                    2024-01-01T00:00:01Z connect\n\
                    2024-01-01T00:00:04Z query";
        let got = merge(logs, "header", "asc", false, true).unwrap();
        assert_eq!(
            got,
            "[db]  2024-01-01T00:00:01Z connect\n\
             [app] 2024-01-01T00:00:02Z started\n\
             [db]  2024-01-01T00:00:04Z query\n\
             [app] 2024-01-01T00:00:05Z ready"
        );
    }

    #[test]
    fn descending_order_newest_first() {
        let logs = "=== a ===\n2024-01-01 00:00:01 one\n=== b ===\n2024-01-01 00:00:02 two";
        let got = merge(logs, "header", "desc", false, false).unwrap();
        assert_eq!(got, "[b] 2024-01-01 00:00:02 two\n[a] 2024-01-01 00:00:01 one");
    }

    #[test]
    fn tail_style_headers_and_epoch() {
        // GNU tail banners + a mix of epoch seconds / milliseconds.
        let logs = "==> a.log <==\n1704067201 alpha\n==> b.log <==\n1704067200000 beta";
        let got = merge(logs, "header", "asc", false, false).unwrap();
        // 1704067200000 ms = 1704067200 s (earlier) → beta first.
        assert_eq!(got, "[b.log] 1704067200000 beta\n[a.log] 1704067201 alpha");
    }

    #[test]
    fn continuation_lines_inherit_previous_timestamp() {
        // The untimestamped stack-trace line stays attached to its entry, not
        // floated to the top.
        let logs = "--- svc ---\n\
                    2024-01-01T00:00:01Z boot\n\
                    2024-01-01T00:00:03Z ERROR boom\n    at frame.rs:9\n\
                    --- other ---\n\
                    2024-01-01T00:00:02Z tick";
        let got = merge(logs, "header", "asc", false, false).unwrap();
        assert_eq!(
            got,
            "[svc] 2024-01-01T00:00:01Z boot\n\
             [other] 2024-01-01T00:00:02Z tick\n\
             [svc] 2024-01-01T00:00:03Z ERROR boom\n\
             [svc]     at frame.rs:9"
        );
    }

    #[test]
    fn dedupe_drops_repeated_timestamped_lines() {
        let logs = "--- a ---\n2024-01-01T00:00:01Z dup\n--- b ---\n2024-01-01T00:00:01Z dup";
        let got = merge(logs, "header", "asc", true, false).unwrap();
        assert_eq!(got, "[a] 2024-01-01T00:00:01Z dup");
    }

    #[test]
    fn blank_mode_numbers_sources() {
        let logs = "2024-01-01T00:00:02Z b\n\n2024-01-01T00:00:01Z a";
        let got = merge(logs, "blank", "asc", false, false).unwrap();
        assert_eq!(
            got,
            "[source2] 2024-01-01T00:00:01Z a\n[source1] 2024-01-01T00:00:02Z b"
        );
    }

    #[test]
    fn syslog_and_apache_timestamps_parse() {
        let logs = "--- sys ---\nOct 11 22:14:15 host sshd: ok\n--- acc ---\n\
                    127.0.0.1 - - [11/Oct/1970:22:14:14 +0000] \"GET / HTTP/1.1\" 200 5";
        let got = merge(logs, "header", "asc", false, false).unwrap();
        // Apache line is one second earlier → first.
        assert!(got.starts_with("[acc] 127.0.0.1"), "got: {got}");
        assert!(got.contains("[sys] Oct 11 22:14:15"));
    }

    #[test]
    fn error_when_no_timestamps() {
        let err = merge("--- a ---\njust text\nmore text", "header", "asc", false, false)
            .unwrap_err();
        assert!(err.contains("No parseable timestamps"), "got: {err}");
    }

    #[test]
    fn error_when_over_line_cap() {
        let big = "2024-01-01T00:00:00Z x\n".repeat(MAX_LINES + 1);
        let err = merge(&big, "header", "asc", false, false).unwrap_err();
        assert!(err.contains("maximum is"), "got: {err}");
    }

    #[test]
    fn error_on_bad_enum() {
        assert!(merge("x", "sideways", "asc", false, false)
            .unwrap_err()
            .contains("source_mode"));
        assert!(merge("x", "header", "sideways", false, false)
            .unwrap_err()
            .contains("order"));
    }
}
