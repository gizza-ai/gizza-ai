//! log-to-table core — pure compute, shared by the chat skill block and the web page.
//!
//! Parses semi-structured log lines into a table / CSV / TSV / JSON using a
//! regex template whose **named capture groups** become the columns
//! (`(?P<name>...)`). A set of presets supplies a ready-made regex for common
//! formats (Apache common & combined access logs, syslog RFC 3164, log4j-style
//! lines). Non-matching lines can be skipped, kept (raw), or treated as an
//! error. No wafer/wasm-bindgen deps — deterministic pure Rust.

use regex::Regex;

/// Upper bound on emitted rows (the `limit` param is clamped to `1..=MAX_LIMIT`).
pub const MAX_LIMIT: u32 = 5000;
/// Default row cap when `limit` is 0/unset.
pub const DEFAULT_LIMIT: u32 = 500;
/// Column name used for a kept non-matching line (`on_nomatch = "keep"`).
const UNMATCHED_COL: &str = "unparsed";

/// The named regex templates behind the `preset` param. Each expands to a Rust
/// `regex` string with `(?P<name>...)` groups defining the columns.
fn preset_pattern(preset: &str) -> Result<&'static str, String> {
    Ok(match preset {
        // Apache/nginx Common Log Format (CLF).
        "common" => {
            r#"^(?P<host>\S+) \S+ (?P<user>\S+) \[(?P<time>[^\]]+)\] "(?P<method>\S+) (?P<path>\S+) (?P<protocol>[^"]*)" (?P<status>\d{3}|-) (?P<size>\d+|-)"#
        }
        // Combined Log Format (CLF + referer + user-agent).
        "combined" => {
            r#"^(?P<host>\S+) \S+ (?P<user>\S+) \[(?P<time>[^\]]+)\] "(?P<method>\S+) (?P<path>\S+) (?P<protocol>[^"]*)" (?P<status>\d{3}|-) (?P<size>\d+|-) "(?P<referer>[^"]*)" "(?P<agent>[^"]*)""#
        }
        // Syslog RFC 3164: `Mon DD HH:MM:SS host tag: message`.
        "syslog" => {
            r#"^(?P<time>\w{3}\s+\d+ \d{2}:\d{2}:\d{2}) (?P<host>\S+) (?P<tag>[^:\[]+)(?:\[(?P<pid>\d+)\])?: (?P<message>.*)$"#
        }
        // log4j / generic app log: `2024-01-01 12:00:00 LEVEL logger - message`.
        "log4j" => {
            r#"^(?P<time>\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?) +(?P<level>[A-Z]+) +(?P<logger>\S+) +- +(?P<message>.*)$"#
        }
        other => return Err(format!(
            "unknown preset '{other}': expected one of custom, common, combined, syslog, log4j"
        )),
    })
}

/// Ordered, deduped list of named capture groups in a compiled regex.
fn column_names(re: &Regex) -> Vec<String> {
    let mut cols = Vec::new();
    for name in re.capture_names().flatten() {
        if !cols.iter().any(|c| c == name) {
            cols.push(name.to_string());
        }
    }
    cols
}

/// Parse `logs` into the chosen `output` shape.
///
/// - `preset`: `custom` (default) uses `pattern`; otherwise a built-in regex
///   (`common`, `combined`, `syslog`, `log4j`).
/// - `pattern`: a Rust regex with `(?P<name>...)` groups (required for `custom`).
/// - `output`: `table` (Markdown, default), `csv`, `tsv`, or `json`.
/// - `header`: include a header row (table/csv/tsv). Default true.
/// - `on_nomatch`: `skip` (default), `keep` (raw line in an `unparsed` column),
///   or `error` (fail on the first non-matching line).
/// - `limit`: max rows (1..=5000, 0 → default 500).
#[allow(clippy::too_many_arguments)]
pub fn parse(
    logs: &str,
    preset: &str,
    pattern: &str,
    output: &str,
    header: bool,
    on_nomatch: &str,
    limit: u32,
) -> Result<String, String> {
    let preset = if preset.trim().is_empty() { "custom" } else { preset.trim() };
    let output = if output.trim().is_empty() { "table" } else { output.trim() };
    let on_nomatch = if on_nomatch.trim().is_empty() { "skip" } else { on_nomatch.trim() };
    if !matches!(output, "table" | "csv" | "tsv" | "json") {
        return Err(format!(
            "unknown output '{output}': expected one of table, csv, tsv, json"
        ));
    }
    if !matches!(on_nomatch, "skip" | "keep" | "error") {
        return Err(format!(
            "unknown on_nomatch '{on_nomatch}': expected one of skip, keep, error"
        ));
    }

    // Resolve the pattern from the preset or the user-supplied template.
    let pattern_str: String = if preset == "custom" {
        let p = pattern.trim();
        if p.is_empty() {
            return Err("a regex pattern is required when preset is 'custom' — use (?P<name>...) named groups to define columns, or pick a preset".into());
        }
        p.to_string()
    } else {
        preset_pattern(preset)?.to_string()
    };

    let re = Regex::new(&pattern_str)
        .map_err(|e| format!("invalid regex '{pattern_str}': {e}"))?;

    let mut columns = column_names(&re);
    if columns.is_empty() {
        return Err("the regex has no named capture groups — add at least one (?P<name>...) group to define a column".into());
    }
    let keep_raw = on_nomatch == "keep";
    if keep_raw && !columns.iter().any(|c| c == UNMATCHED_COL) {
        columns.push(UNMATCHED_COL.to_string());
    }

    let cap = if limit == 0 { DEFAULT_LIMIT } else { limit.min(MAX_LIMIT) } as usize;

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut matched = 0usize;
    let mut skipped = 0usize;
    for (i, line) in logs.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if rows.len() >= cap {
            break;
        }
        match re.captures(line) {
            Some(caps) => {
                matched += 1;
                let row: Vec<String> = columns
                    .iter()
                    .map(|c| {
                        if c == UNMATCHED_COL {
                            String::new()
                        } else {
                            caps.name(c).map(|m| m.as_str().to_string()).unwrap_or_default()
                        }
                    })
                    .collect();
                rows.push(row);
            }
            None => match on_nomatch {
                "error" => {
                    return Err(format!(
                        "line {} did not match the pattern: {}",
                        i + 1,
                        line
                    ));
                }
                "keep" => {
                    let mut row = vec![String::new(); columns.len()];
                    if let Some(idx) = columns.iter().position(|c| c == UNMATCHED_COL) {
                        row[idx] = line.to_string();
                    }
                    rows.push(row);
                    skipped += 1;
                }
                _ => {
                    skipped += 1;
                }
            },
        }
    }

    if matched == 0 && !keep_raw {
        return Err(
            "no lines matched the pattern — check the regex against your log format, or pick a preset".into(),
        );
    }

    Ok(match output {
        "json" => render_json(&columns, &rows),
        "csv" => render_delimited(&columns, &rows, ',', header),
        "tsv" => render_delimited(&columns, &rows, '\t', header),
        _ => render_table(&columns, &rows, header, matched, skipped),
    })
}

/// JSON array of one object per row (column name → value), order preserved.
fn render_json(columns: &[String], rows: &[Vec<String>]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (c, v) in columns.iter().zip(row.iter()) {
                obj.insert(c.clone(), serde_json::Value::String(v.clone()));
            }
            serde_json::Value::Object(obj)
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_else(|_| "[]".into())
}

/// RFC-4180-style CSV (`,`) or TSV (`\t`). Fields are quoted when they contain
/// the delimiter, a quote, or a newline; embedded quotes are doubled.
fn render_delimited(columns: &[String], rows: &[Vec<String>], delim: char, header: bool) -> String {
    let mut out = String::new();
    let esc = |s: &str| -> String {
        let needs = s.contains(delim) || s.contains('"') || s.contains('\n') || s.contains('\r');
        if needs {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let join = |fields: &[String]| -> String {
        fields.iter().map(|f| esc(f)).collect::<Vec<_>>().join(&delim.to_string())
    };
    if header {
        out.push_str(&join(columns));
        out.push('\n');
    }
    for row in rows {
        out.push_str(&join(row));
        out.push('\n');
    }
    // Drop the trailing newline for a clean, exact output.
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Aligned Markdown table with a stats caption.
fn render_table(
    columns: &[String],
    rows: &[Vec<String>],
    header: bool,
    matched: usize,
    skipped: usize,
) -> String {
    // Escape pipes so cell content can't break the Markdown table.
    let cell = |s: &str| s.replace('|', "\\|");
    let mut widths: Vec<usize> = columns.iter().map(|c| cell(c).chars().count()).collect();
    for row in rows {
        for (i, v) in row.iter().enumerate() {
            let w = cell(v).chars().count();
            if w > widths[i] {
                widths[i] = w;
            }
        }
    }
    let pad = |s: &str, w: usize| -> String {
        let c = cell(s);
        let len = c.chars().count();
        format!("{}{}", c, " ".repeat(w.saturating_sub(len)))
    };
    let mut out = String::new();
    if header {
        let head: Vec<String> = columns.iter().enumerate().map(|(i, c)| pad(c, widths[i])).collect();
        out.push_str(&format!("| {} |\n", head.join(" | ")));
        let sep: Vec<String> = widths.iter().map(|w| "-".repeat((*w).max(3))).collect();
        out.push_str(&format!("| {} |\n", sep.join(" | ")));
    }
    for row in rows {
        let cells: Vec<String> = row.iter().enumerate().map(|(i, v)| pad(v, widths[i])).collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    let mut caption = format!("\n_{} row(s)", matched + skipped);
    if skipped > 0 {
        caption.push_str(&format!(", {skipped} unmatched"));
    }
    caption.push_str("_");
    out.push_str(&caption);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_preset_to_csv() {
        let logs = "127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] \"GET /index.html HTTP/1.0\" 200 2326 \"-\" \"Mozilla/5.0\"";
        let out = parse(logs, "combined", "", "csv", true, "skip", 0).unwrap();
        assert_eq!(
            out,
            "host,user,time,method,path,protocol,status,size,referer,agent\n\
             127.0.0.1,-,10/Oct/2000:13:55:36 -0700,GET,/index.html,HTTP/1.0,200,2326,-,Mozilla/5.0"
        );
    }

    #[test]
    fn custom_named_groups_to_json() {
        let logs = "ERROR 42 boom\nINFO 7 ok";
        let pat = r"^(?P<level>\w+) (?P<code>\d+) (?P<msg>.*)$";
        let out = parse(logs, "custom", pat, "json", true, "skip", 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["level"], "ERROR");
        assert_eq!(v[0]["code"], "42");
        assert_eq!(v[1]["msg"], "ok");
    }

    #[test]
    fn table_output_has_header_and_caption() {
        let logs = "a 1\nb 2";
        let pat = r"^(?P<name>\w+) (?P<n>\d+)$";
        let out = parse(logs, "custom", pat, "table", true, "skip", 0).unwrap();
        assert!(out.contains("| name | n |"));
        assert!(out.contains("| ---- | --- |"));
        assert!(out.contains("_2 row(s)_"));
    }

    #[test]
    fn nomatch_skip_counts_but_omits() {
        let logs = "ok 1\ngarbage line\nok 2";
        let pat = r"^(?P<w>\w+) (?P<n>\d+)$";
        let out = parse(logs, "custom", pat, "csv", false, "skip", 0).unwrap();
        assert_eq!(out, "ok,1\nok,2");
    }

    #[test]
    fn nomatch_keep_adds_unparsed_column() {
        let logs = "ok 1\ngarbage";
        let pat = r"^(?P<w>\w+) (?P<n>\d+)$";
        let out = parse(logs, "custom", pat, "csv", true, "keep", 0).unwrap();
        assert_eq!(out, "w,n,unparsed\nok,1,\n,,garbage");
    }

    #[test]
    fn csv_quotes_fields_with_delimiters() {
        let logs = "GET /a,b 200";
        let pat = r"^(?P<method>\w+) (?P<path>\S+) (?P<status>\d+)$";
        let out = parse(logs, "custom", pat, "csv", false, "skip", 0).unwrap();
        assert_eq!(out, "GET,\"/a,b\",200");
    }

    #[test]
    fn limit_caps_rows() {
        let logs = "x 1\nx 2\nx 3\nx 4";
        let pat = r"^(?P<w>\w+) (?P<n>\d+)$";
        let out = parse(logs, "custom", pat, "csv", false, "skip", 2).unwrap();
        assert_eq!(out, "x,1\nx,2");
    }

    // ---- error paths ----

    #[test]
    fn error_on_empty_pattern_for_custom() {
        let err = parse("a 1", "custom", "", "csv", true, "skip", 0).unwrap_err();
        assert!(err.contains("regex pattern is required"), "{err}");
    }

    #[test]
    fn error_on_no_named_groups() {
        let err = parse("a 1", "custom", r"^\w+ \d+$", "csv", true, "skip", 0).unwrap_err();
        assert!(err.contains("no named capture groups"), "{err}");
    }

    #[test]
    fn error_on_invalid_regex() {
        let err = parse("a", "custom", r"(?P<x>", "csv", true, "skip", 0).unwrap_err();
        assert!(err.contains("invalid regex"), "{err}");
    }

    #[test]
    fn error_on_unknown_preset() {
        let err = parse("a 1", "bogus", "", "csv", true, "skip", 0).unwrap_err();
        assert!(err.contains("unknown preset"), "{err}");
    }

    #[test]
    fn error_on_bad_output() {
        let err = parse("a 1", "common", "", "xml", true, "skip", 0).unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }

    #[test]
    fn error_mode_fails_on_first_nomatch() {
        let logs = "ok 1\nnope";
        let pat = r"^(?P<w>\w+) (?P<n>\d+)$";
        let err = parse(logs, "custom", pat, "csv", true, "error", 0).unwrap_err();
        assert!(err.contains("line 2 did not match"), "{err}");
    }

    #[test]
    fn error_when_nothing_matches() {
        let logs = "zzz\nyyy";
        let pat = r"^(?P<w>\w+) (?P<n>\d+)$";
        let err = parse(logs, "custom", pat, "csv", true, "skip", 0).unwrap_err();
        assert!(err.contains("no lines matched"), "{err}");
    }

    #[test]
    fn syslog_preset_parses() {
        let logs = "Oct 11 22:14:15 myhost sshd[1234]: Accepted password for root";
        let out = parse(logs, "syslog", "", "json", true, "skip", 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["host"], "myhost");
        assert_eq!(v[0]["tag"], "sshd");
        assert_eq!(v[0]["pid"], "1234");
        assert_eq!(v[0]["message"], "Accepted password for root");
    }
}
