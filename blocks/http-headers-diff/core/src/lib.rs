//! http-headers-diff core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps (only serde_json for the JSON output).
//!
//! Diffs two sets of HTTP headers and reports which headers were **added**,
//! **removed**, or **changed** between them (direction is left → right). Each
//! block is parsed into a case-folded `name → combined-value` map that follows
//! HTTP semantics:
//!
//! - Header **names are case-insensitive** (RFC 9110 §5.1) — they are matched
//!   case-insensitively and displayed in canonical Title-Case.
//! - **Repeated** headers are combined into one value with `, ` (RFC 9110 §5.3),
//!   **except `Set-Cookie`** which is never comma-joined (RFC 6265) — its
//!   occurrences are kept newline-joined so each cookie stays intact.
//! - An optional leading **request/status line** (`GET / HTTP/1.1`, `HTTP/1.1
//!   200 OK`) is detected and skipped — only the header set is diffed.
//! - CRLF or bare-LF endings, obsolete line-folded continuations, and a blank
//!   line ending the header block are all handled.

use std::collections::{BTreeMap, BTreeSet};

/// Canonical HTTP Title-Case for an already-lowercased header name. Most names
/// are per-segment capitalized (`x-frame-options` → `X-Frame-Options`); a few
/// well-known headers use non-standard casing and are special-cased.
fn canonical(lower: &str) -> String {
    match lower {
        "etag" => return "ETag".to_string(),
        "www-authenticate" => return "WWW-Authenticate".to_string(),
        "content-md5" => return "Content-MD5".to_string(),
        "te" => return "TE".to_string(),
        "dnt" => return "DNT".to_string(),
        "x-xss-protection" => return "X-XSS-Protection".to_string(),
        _ => {}
    }
    lower
        .split('-')
        .map(|seg| {
            let mut chars = seg.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Detect a leading start line. Returns true if the first line is a request line
/// (`METHOD target HTTP/x.y`) or a status line (`HTTP/x.y code …`) rather than a
/// header. A header line's first token ends the name at ':', so it can't be one.
fn is_start_line(line: &str) -> bool {
    let l = line.trim();
    let first = l.split(' ').next().unwrap_or("");
    if first.contains(':') {
        return false;
    }
    if first.to_ascii_uppercase().starts_with("HTTP/") {
        return true; // status line
    }
    let parts: Vec<&str> = l.splitn(3, ' ').collect();
    parts.len() == 3 && parts[2].trim().to_ascii_uppercase().starts_with("HTTP/")
}

/// One parsed header block: lower-name → (display name, combined value).
/// A `BTreeMap` gives deterministic, alphabetically-sorted diff output.
struct Parsed {
    map: BTreeMap<String, (String, String)>,
}

/// Combine the values collected for one header name. Repeated headers join with
/// `, ` (RFC 9110), except `Set-Cookie` which joins with a newline so each
/// cookie stays intact (RFC 6265 forbids comma-joining Set-Cookie).
fn combine(lower: &str, vals: &[String]) -> String {
    if lower == "set-cookie" {
        vals.join("\n")
    } else {
        vals.join(", ")
    }
}

/// Parse one raw header block into a case-folded map (last-line-wins ordering is
/// irrelevant; every occurrence is combined). Returns `Err` on a line that is
/// neither a `Name: value` header, a start line, a folded continuation, nor
/// blank. The header block ends at the first blank line (head/body split).
fn parse(text: &str) -> Result<Parsed, String> {
    let trimmed = text.trim_start_matches('\u{feff}');
    let trimmed = trimmed.trim_start_matches(['\r', '\n']);
    if trimmed.trim().is_empty() {
        return Err(
            "input is empty — paste an HTTP header block (one 'Name: value' per line)".into(),
        );
    }

    let lines: Vec<&str> = trimmed
        .split('\n')
        .map(|l| l.trim_end_matches('\r'))
        .collect();

    // Skip an optional leading request/status line.
    let mut idx = 0usize;
    if lines.first().map(|l| is_start_line(l)).unwrap_or(false) {
        idx = 1;
    }

    // Collect first-seen order + every value per lower-cased name.
    let mut order: Vec<String> = Vec::new();
    let mut values: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut display: BTreeMap<String, String> = BTreeMap::new();
    let mut last_key: Option<String> = None;

    while idx < lines.len() {
        let line = lines[idx];
        idx += 1;
        if line.trim().is_empty() {
            break; // blank line ends the header block
        }
        // Obsolete line folding: a continuation line begins with whitespace.
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(k) = &last_key {
                if let Some(vs) = values.get_mut(k) {
                    if let Some(last) = vs.last_mut() {
                        last.push(' ');
                        last.push_str(line.trim());
                        continue;
                    }
                }
            }
            return Err(format!(
                "line '{line}' is a folded continuation but has no preceding header"
            ));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| format!("line '{line}' is not a 'Name: value' header (missing ':')"))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(format!("line '{line}' has an empty header name"));
        }
        let value = value.trim().to_string();
        let lower = name.to_ascii_lowercase();
        if !values.contains_key(&lower) {
            order.push(lower.clone());
            values.insert(lower.clone(), Vec::new());
            display.insert(lower.clone(), name.to_string());
        }
        values.get_mut(&lower).unwrap().push(value);
        last_key = Some(lower);
    }

    if order.is_empty() {
        return Err("no headers found — each header must be on its own 'Name: value' line".into());
    }

    let mut map = BTreeMap::new();
    for lower in &order {
        let disp = canonical(lower);
        let val = combine(lower, &values[lower]);
        map.insert(lower.clone(), (disp, val));
    }
    Ok(Parsed { map })
}

/// Parse the `ignore` list: header names separated by commas, whitespace, or
/// newlines, folded to lowercase for case-insensitive matching.
fn parse_ignore(ignore: &str) -> BTreeSet<String> {
    ignore
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Normalize a value for comparison when `ignore_order` is on: split on `,`,
/// trim each token, sort, and rejoin, so a list-valued header (Vary,
/// Cache-Control, Accept) that only reordered its tokens compares equal.
fn order_insensitive(value: &str) -> String {
    let mut toks: Vec<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();
    toks.sort_unstable();
    toks.join(", ")
}

/// Diff two sets of HTTP headers key-by-key.
///
/// - `left`: the first (old/base) header block.
/// - `right`: the second (new/compared) header block.
/// - `ignore`: header names to exclude from the diff (comma/space/newline
///   separated, case-insensitive) — e.g. `"Date, Age"`.
/// - `ignore_order`: when comparing values, treat a comma-separated list as a
///   set so token reordering isn't reported as a change.
/// - `output`: `"report"` (default) a grouped human summary, or `"json"` a
///   structured object.
///
/// Returns `Err` on an unparseable block or an unknown `output` mode.
pub fn diff(
    left: &str,
    right: &str,
    ignore: &str,
    ignore_order: bool,
    output: &str,
) -> Result<String, String> {
    let out_mode = if output.trim().is_empty() {
        "report"
    } else {
        output.trim()
    };
    if !matches!(out_mode, "report" | "json") {
        return Err(format!(
            "invalid output {out_mode:?}: expected \"report\" or \"json\""
        ));
    }

    let l = parse(left)?;
    let r = parse(right)?;
    let ignored = parse_ignore(ignore);

    let keys: BTreeSet<&String> = l
        .map
        .keys()
        .chain(r.map.keys())
        .filter(|k| !ignored.contains(*k))
        .collect();

    let mut added: Vec<(String, String)> = Vec::new();
    let mut removed: Vec<(String, String)> = Vec::new();
    let mut changed: Vec<(String, String, String)> = Vec::new(); // display, old, new
    let mut unchanged: Vec<String> = Vec::new();

    let same = |a: &str, b: &str| -> bool {
        if ignore_order {
            order_insensitive(a) == order_insensitive(b)
        } else {
            a == b
        }
    };

    for lower in keys {
        match (l.map.get(lower), r.map.get(lower)) {
            (None, Some((disp, val))) => added.push((disp.clone(), val.clone())),
            (Some((disp, val)), None) => removed.push((disp.clone(), val.clone())),
            (Some((disp, lv)), Some((_, rv))) => {
                if same(lv, rv) {
                    unchanged.push(disp.clone());
                } else {
                    changed.push((disp.clone(), lv.clone(), rv.clone()));
                }
            }
            (None, None) => unreachable!("key came from one of the maps"),
        }
    }

    if out_mode == "json" {
        Ok(render_json(&added, &removed, &changed, &unchanged))
    } else {
        Ok(render_report(&added, &removed, &changed, &unchanged))
    }
}

/// Render a header value for the human report, keeping it on one line: a
/// newline-joined Set-Cookie value is shown with ` ⏎ ` between cookies.
fn one_line(value: &str) -> String {
    value.replace('\n', " ⏎ ")
}

fn render_report(
    added: &[(String, String)],
    removed: &[(String, String)],
    changed: &[(String, String, String)],
    unchanged: &[String],
) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Header diff — {} added, {} removed, {} changed, {} unchanged\n\n",
        added.len(),
        removed.len(),
        changed.len(),
        unchanged.len(),
    ));

    s.push_str(&format!("Added ({}):\n", added.len()));
    if added.is_empty() {
        s.push_str("  (none)\n");
    } else {
        for (k, v) in added {
            s.push_str(&format!("  + {k}: {}\n", one_line(v)));
        }
    }
    s.push('\n');

    s.push_str(&format!("Removed ({}):\n", removed.len()));
    if removed.is_empty() {
        s.push_str("  (none)\n");
    } else {
        for (k, v) in removed {
            s.push_str(&format!("  - {k}: {}\n", one_line(v)));
        }
    }
    s.push('\n');

    s.push_str(&format!("Changed ({}):\n", changed.len()));
    if changed.is_empty() {
        s.push_str("  (none)\n");
    } else {
        for (k, old, new) in changed {
            s.push_str(&format!(
                "  ~ {k}: {} -> {}\n",
                one_line(old),
                one_line(new)
            ));
        }
    }
    s.push('\n');

    s.push_str(&format!("Unchanged ({}):\n", unchanged.len()));
    if unchanged.is_empty() {
        s.push_str("  (none)");
    } else {
        s.push_str(&format!("  {}", unchanged.join(", ")));
    }
    s
}

fn render_json(
    added: &[(String, String)],
    removed: &[(String, String)],
    changed: &[(String, String, String)],
    unchanged: &[String],
) -> String {
    use serde_json::{Map, Value};

    let mut added_obj = Map::new();
    for (k, v) in added {
        added_obj.insert(k.clone(), Value::String(v.clone()));
    }
    let mut removed_obj = Map::new();
    for (k, v) in removed {
        removed_obj.insert(k.clone(), Value::String(v.clone()));
    }
    let mut changed_obj = Map::new();
    for (k, old, new) in changed {
        let mut pair = Map::new();
        pair.insert("old".into(), Value::String(old.clone()));
        pair.insert("new".into(), Value::String(new.clone()));
        changed_obj.insert(k.clone(), Value::Object(pair));
    }
    let unchanged_arr: Vec<Value> = unchanged.iter().map(|k| Value::String(k.clone())).collect();

    let mut summary = Map::new();
    summary.insert("added".into(), Value::from(added.len()));
    summary.insert("removed".into(), Value::from(removed.len()));
    summary.insert("changed".into(), Value::from(changed.len()));
    summary.insert("unchanged".into(), Value::from(unchanged.len()));

    let mut root = Map::new();
    root.insert("summary".into(), Value::Object(summary));
    root.insert("added".into(), Value::Object(added_obj));
    root.insert("removed".into(), Value::Object(removed_obj));
    root.insert("changed".into(), Value::Object(changed_obj));
    root.insert("unchanged".into(), Value::Array(unchanged_arr));

    serde_json::to_string_pretty(&Value::Object(root)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_report_added_removed_changed() {
        let left = "Content-Type: text/html\nServer: nginx\nX-Frame-Options: DENY\n";
        let right = "Content-Type: application/json\nServer: nginx\nCache-Control: no-cache\n";
        let out = diff(left, right, "", false, "report").unwrap();
        assert_eq!(
            out,
            "Header diff — 1 added, 1 removed, 1 changed, 1 unchanged\n\n\
             Added (1):\n  + Cache-Control: no-cache\n\n\
             Removed (1):\n  - X-Frame-Options: DENY\n\n\
             Changed (1):\n  ~ Content-Type: text/html -> application/json\n\n\
             Unchanged (1):\n  Server"
        );
    }

    #[test]
    fn header_names_match_case_insensitively() {
        let left = "content-type: text/html\n";
        let right = "Content-Type: text/html\n";
        let out = diff(left, right, "", false, "report").unwrap();
        // Same header despite different casing → unchanged, displayed canonical.
        assert!(
            out.contains("0 added, 0 removed, 0 changed, 1 unchanged"),
            "{out}"
        );
        assert!(out.contains("Unchanged (1):\n  Content-Type"), "{out}");
    }

    #[test]
    fn repeated_headers_are_combined_before_diff() {
        let left = "Vary: Accept\nVary: Accept-Encoding\n";
        let right = "Vary: Accept, Accept-Encoding\n";
        let out = diff(left, right, "", false, "report").unwrap();
        // "Accept" + "Accept-Encoding" combine to the same string → unchanged.
        assert!(out.contains("0 changed, 1 unchanged"), "{out}");
    }

    #[test]
    fn set_cookie_is_not_comma_joined() {
        let left = "Set-Cookie: a=1\nSet-Cookie: b=2\n";
        let right = "Set-Cookie: a=1\n";
        let out = diff(left, right, "", false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // left combined "a=1\nb=2" vs right "a=1" → changed, newline-joined (not "a=1, b=2").
        assert_eq!(v["changed"]["Set-Cookie"]["old"], "a=1\nb=2");
        assert_eq!(v["changed"]["Set-Cookie"]["new"], "a=1");
    }

    #[test]
    fn leading_start_line_is_skipped() {
        let left = "HTTP/1.1 200 OK\nContent-Type: text/html\n";
        let right = "HTTP/2 301 Moved Permanently\nContent-Type: text/html\n";
        let out = diff(left, right, "", false, "report").unwrap();
        // Status lines differ but are not diffed; the one header is unchanged.
        assert!(
            out.contains("0 added, 0 removed, 0 changed, 1 unchanged"),
            "{out}"
        );
    }

    #[test]
    fn request_start_line_is_skipped() {
        let left = "GET / HTTP/1.1\nHost: a.com\n";
        let out = diff(left, "Host: a.com\n", "", false, "report").unwrap();
        assert!(out.contains("1 unchanged"), "{out}");
    }

    #[test]
    fn ignore_list_drops_named_headers() {
        let left = "Date: Mon\nContent-Type: text/html\n";
        let right = "Date: Tue\nContent-Type: text/html\n";
        let noisy = diff(left, right, "", false, "report").unwrap();
        assert!(noisy.contains("1 changed"), "{noisy}");
        // Date is noise — ignore it (case-insensitive).
        let clean = diff(left, right, "date", false, "report").unwrap();
        assert!(
            clean.contains("0 added, 0 removed, 0 changed, 1 unchanged"),
            "{clean}"
        );
        assert!(!clean.contains("Date"), "{clean}");
    }

    #[test]
    fn ignore_order_treats_list_reorder_as_unchanged() {
        let left = "Cache-Control: no-cache, no-store\n";
        let right = "Cache-Control: no-store, no-cache\n";
        let strict = diff(left, right, "", false, "report").unwrap();
        assert!(strict.contains("1 changed"), "{strict}");
        let loose = diff(left, right, "", true, "report").unwrap();
        assert!(loose.contains("0 changed, 1 unchanged"), "{loose}");
    }

    #[test]
    fn folds_continuation_lines() {
        let left = "X-Long: part1\n  part2\n";
        let right = "X-Long: part1 part2\n";
        let out = diff(left, right, "", false, "report").unwrap();
        assert!(out.contains("1 unchanged"), "{out}");
    }

    #[test]
    fn stops_at_blank_line() {
        let left = "Server: nginx\n\nthis is body, not a header\n";
        let out = diff(left, "Server: nginx\n", "", false, "report").unwrap();
        assert!(
            out.contains("0 added, 0 removed, 0 changed, 1 unchanged"),
            "{out}"
        );
    }

    #[test]
    fn tolerates_crlf() {
        let left = "Host: x\r\nAccept: y\r\n";
        let right = "Host: x\r\nAccept: z\r\n";
        let out = diff(left, right, "", false, "report").unwrap();
        assert!(out.contains("~ Accept: y -> z"), "{out}");
    }

    #[test]
    fn json_output_shape() {
        let left = "A: 1\nB: 2\n";
        let right = "A: 9\nC: 3\n";
        let out = diff(left, right, "", false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["added"], 1);
        assert_eq!(v["summary"]["removed"], 1);
        assert_eq!(v["summary"]["changed"], 1);
        assert_eq!(v["added"]["C"], "3");
        assert_eq!(v["removed"]["B"], "2");
        assert_eq!(v["changed"]["A"]["old"], "1");
        assert_eq!(v["changed"]["A"]["new"], "9");
    }

    #[test]
    fn invalid_output_mode_errors() {
        let err = diff("A: 1", "A: 2", "", false, "yaml").unwrap_err();
        assert!(err.contains("invalid output"), "{err}");
    }

    #[test]
    fn empty_block_errors() {
        assert!(diff("", "A: 1", "", false, "report").is_err());
        assert!(diff("A: 1", "   \r\n\r\n", "", false, "report").is_err());
    }

    #[test]
    fn line_without_colon_errors() {
        let err = diff("this has no colon\n", "A: 1", "", false, "report").unwrap_err();
        assert!(err.contains("missing ':'"), "{err}");
    }

    #[test]
    fn empty_name_errors() {
        assert!(diff(": value\n", "A: 1", "", false, "report").is_err());
    }
}
