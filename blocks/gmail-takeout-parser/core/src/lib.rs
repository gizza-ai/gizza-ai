//! gizza-ai/gmail-takeout-parser core — turn a Google Takeout Gmail **mbox**
//! export into a clean table of messages (one row each) as CSV or JSON, keeping
//! the Gmail labels (`X-Gmail-Labels`). Pure-Rust (`mail-parser`), wasm-safe, no
//! wafer/wasm-bindgen deps.

use mail_parser::{Address, MessageParser};
use serde::Serialize;

/// Output serialization for the parsed table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Csv,
    Json,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "csv" | "" => Ok(Format::Csv),
            "json" => Ok(Format::Json),
            other => Err(format!("unknown format \"{other}\" — use \"csv\" or \"json\"")),
        }
    }
}

/// One parsed message row. Field order is also the CSV column order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    pub date: String,
    pub from: String,
    pub to: String,
    pub cc: String,
    pub subject: String,
    pub labels: String,
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

/// Options controlling the body/snippet column.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub format: Format,
    /// Include a plain-text body snippet column.
    pub include_body: bool,
    /// Cap the snippet at this many characters (`0` = no cap / full body).
    pub snippet_chars: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options { format: Format::Csv, include_body: false, snippet_chars: 200 }
    }
}

/// Split a raw mbox blob into individual message blocks.
///
/// mbox delimits messages with a `From ` "postmark" line at column 0 (the space
/// after `From` distinguishes it from a `From:` header). Google Takeout uses
/// this classic mbox form. The postmark line itself is dropped — only the RFC
/// 5322 message (headers + body) is returned. A blob with no postmark at all is
/// treated as a single message so a lone `.eml` still parses.
pub fn split_messages(raw: &str) -> Vec<String> {
    let mut msgs: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    for line in raw.split_inclusive('\n') {
        if line.starts_with("From ") {
            if started && !cur.trim().is_empty() {
                msgs.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
            started = true;
            continue; // drop the postmark line
        }
        if started {
            cur.push_str(line);
        }
    }
    if started && !cur.trim().is_empty() {
        msgs.push(cur);
    }
    if msgs.is_empty() && !raw.trim().is_empty() {
        msgs.push(raw.to_string());
    }
    msgs
}

/// Extract a single unstructured header value from a raw message, unfolding RFC
/// 5322 continuation lines. Case-insensitive on the header name. Used for
/// `X-Gmail-Labels`, which is Gmail-specific and not modeled by `mail-parser`.
fn raw_header(raw: &str, name: &str) -> Option<String> {
    let end = raw
        .find("\r\n\r\n")
        .or_else(|| raw.find("\n\n"))
        .unwrap_or(raw.len());
    let headers = &raw[..end];
    let target = name.to_ascii_lowercase();
    let mut acc: Option<String> = None;
    for line in headers.lines() {
        let is_continuation = line.starts_with(' ') || line.starts_with('\t');
        if is_continuation {
            if let Some(a) = acc.as_mut() {
                a.push(' ');
                a.push_str(line.trim());
            }
            continue;
        }
        // A new header line: if we were collecting the target, we're done.
        if acc.is_some() {
            break;
        }
        if let Some(idx) = line.find(':') {
            if line[..idx].trim().eq_ignore_ascii_case(&target) {
                acc = Some(line[idx + 1..].trim().to_string());
            }
        }
    }
    acc.filter(|s| !s.is_empty())
}

/// Format an address list as `Name <addr>, addr2, …`.
fn fmt_addresses(addr: Option<&Address>) -> String {
    match addr {
        None => String::new(),
        Some(a) => a
            .iter()
            .filter_map(|m| match (m.name(), m.address()) {
                (Some(n), Some(ad)) => Some(format!("{n} <{ad}>")),
                (None, Some(ad)) => Some(ad.to_string()),
                (Some(n), None) => Some(n.to_string()),
                (None, None) => None,
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// Collapse all runs of whitespace to a single space and trim.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse a raw mbox blob into rows.
pub fn parse(raw: &str, opts: Options) -> Result<Vec<Row>, String> {
    if raw.trim().is_empty() {
        return Err("input is empty — paste the contents of a Google Takeout Gmail .mbox file".into());
    }
    let mut rows = Vec::new();
    for block in split_messages(raw) {
        let msg = match MessageParser::default().parse(block.as_bytes()) {
            Some(m) => m,
            None => continue, // skip a block that isn't a parseable message
        };
        let snippet = if opts.include_body {
            let body = msg.body_text(0).map(|s| collapse_ws(&s)).unwrap_or_default();
            let capped = if opts.snippet_chars == 0 {
                body
            } else {
                body.chars().take(opts.snippet_chars).collect()
            };
            Some(capped)
        } else {
            None
        };
        rows.push(Row {
            date: msg.date().map(|d| d.to_rfc3339()).unwrap_or_default(),
            from: fmt_addresses(msg.from()),
            to: fmt_addresses(msg.to()),
            cc: fmt_addresses(msg.cc()),
            subject: msg.subject().map(|s| s.to_string()).unwrap_or_default(),
            labels: raw_header(&block, "X-Gmail-Labels").unwrap_or_default(),
            message_id: msg.message_id().map(|s| s.to_string()).unwrap_or_default(),
            snippet,
        });
    }
    if rows.is_empty() {
        return Err("no parseable messages found — is this a Gmail Takeout .mbox export?".into());
    }
    Ok(rows)
}

/// Escape one field for RFC 4180 CSV: quote it when it contains a comma, quote,
/// CR or LF, doubling any embedded quote.
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Render rows to the requested format as a single string.
pub fn render(raw: &str, opts: Options) -> Result<String, String> {
    let rows = parse(raw, opts)?;
    match opts.format {
        Format::Json => {
            serde_json::to_string_pretty(&rows).map_err(|e| format!("failed to serialize JSON: {e}"))
        }
        Format::Csv => {
            let mut header = vec!["date", "from", "to", "cc", "subject", "labels", "message_id"];
            if opts.include_body {
                header.push("snippet");
            }
            let mut out = String::new();
            out.push_str(&header.join(","));
            out.push_str("\r\n");
            for r in &rows {
                let mut cells = vec![
                    csv_field(&r.date),
                    csv_field(&r.from),
                    csv_field(&r.to),
                    csv_field(&r.cc),
                    csv_field(&r.subject),
                    csv_field(&r.labels),
                    csv_field(&r.message_id),
                ];
                if opts.include_body {
                    cells.push(csv_field(r.snippet.as_deref().unwrap_or("")));
                }
                out.push_str(&cells.join(","));
                out.push_str("\r\n");
            }
            Ok(out.trim_end_matches("\r\n").to_string())
        }
    }
}

/// One-call entry shared by the chat/CLI block and the web wrapper: parse a raw
/// mbox blob and render it in the requested format. `format` is `"csv"` (default,
/// also for an empty string) or `"json"`; `include_body` adds the snippet column;
/// `snippet_chars` caps that snippet (`0` = full body).
pub fn run(
    input: &str,
    format: &str,
    include_body: bool,
    snippet_chars: usize,
) -> Result<String, String> {
    let opts = Options {
        format: Format::parse(format)?,
        include_body,
        snippet_chars,
    };
    render(input, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two-message Gmail Takeout mbox (postmark + X-Gmail-Labels per message).
    const MBOX: &str = "From 1521178313854490905@xxx Mon Sep 03 10:00:00 +0000 2018\r\nX-Gmail-Labels: Inbox,Important,Work\r\nFrom: Alice Example <alice@example.com>\r\nTo: Bob <bob@example.org>\r\nSubject: Project kickoff\r\nDate: Mon, 3 Sep 2018 10:00:00 +0000\r\nMessage-ID: <k1@example.com>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHi Bob, let's start the project on Monday.\r\nThanks, Alice\r\nFrom 1611178313854490906@xxx Tue Sep 04 09:30:00 +0000 2018\r\nX-Gmail-Labels: Sent\r\nFrom: Bob <bob@example.org>\r\nTo: alice@example.com, carol@example.net\r\nCc: dave@example.com\r\nSubject: Re: Project kickoff\r\nDate: Tue, 4 Sep 2018 09:30:00 +0000\r\nMessage-ID: <k2@example.com>\r\nContent-Type: text/plain\r\n\r\nSounds good.\r\n";

    #[test]
    fn splits_two_messages() {
        assert_eq!(split_messages(MBOX).len(), 2);
    }

    #[test]
    fn parses_rows_with_labels() {
        let rows = parse(MBOX, Options::default()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].subject, "Project kickoff");
        assert_eq!(rows[0].from, "Alice Example <alice@example.com>");
        assert_eq!(rows[0].to, "Bob <bob@example.org>");
        assert_eq!(rows[0].labels, "Inbox,Important,Work");
        assert_eq!(rows[0].message_id, "k1@example.com");
        assert!(rows[0].date.starts_with("2018-09-03"));
        assert_eq!(rows[1].labels, "Sent");
        assert_eq!(rows[1].to, "alice@example.com, carol@example.net");
        assert_eq!(rows[1].cc, "dave@example.com");
        assert!(rows[0].snippet.is_none());
    }

    #[test]
    fn csv_output_has_header_and_quoting() {
        let out = render(MBOX, Options::default()).unwrap();
        let lines: Vec<&str> = out.split("\r\n").collect();
        assert_eq!(lines[0], "date,from,to,cc,subject,labels,message_id");
        assert_eq!(lines.len(), 3); // header + 2 rows
        // The "Inbox,Important,Work" label list contains commas → must be quoted.
        assert!(lines[1].contains("\"Inbox,Important,Work\""));
    }

    #[test]
    fn json_output_is_an_array() {
        let opts = Options { format: Format::Json, ..Options::default() };
        let out = render(MBOX, opts).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[0]["labels"], "Inbox,Important,Work");
        assert!(v[0].get("snippet").is_none());
    }

    #[test]
    fn body_snippet_included_and_capped() {
        let opts = Options { include_body: true, snippet_chars: 10, ..Options::default() };
        let rows = parse(MBOX, opts).unwrap();
        let snip = rows[0].snippet.as_ref().unwrap();
        assert_eq!(snip.chars().count(), 10);
        assert_eq!(snip, "Hi Bob, le");
        // CSV gains the snippet column.
        let csv = render(MBOX, opts).unwrap();
        assert!(csv.lines().next().unwrap().ends_with("message_id,snippet"));
    }

    #[test]
    fn single_message_without_postmark_still_parses() {
        let eml = "From: a@x.com\r\nSubject: Lonely\r\n\r\nbody\r\n";
        let rows = parse(eml, Options::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].subject, "Lonely");
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse("", Options::default()).is_err());
        assert!(parse("   \n  ", Options::default()).is_err());
    }

    #[test]
    fn run_entry_defaults_to_csv() {
        // Empty format string → CSV; no body column by default.
        let out = run(MBOX, "", false, 200).unwrap();
        assert_eq!(out.split("\r\n").next().unwrap(), "date,from,to,cc,subject,labels,message_id");
        // JSON path with a capped body snippet.
        let js = run(MBOX, "json", true, 5).unwrap();
        let v: serde_json::Value = serde_json::from_str(&js).unwrap();
        assert_eq!(v[0]["snippet"], "Hi Bo");
    }

    #[test]
    fn format_parse_rejects_garbage() {
        assert!(Format::parse("xml").is_err());
        assert_eq!(Format::parse("").unwrap(), Format::Csv);
        assert_eq!(Format::parse("JSON").unwrap(), Format::Json);
    }
}
