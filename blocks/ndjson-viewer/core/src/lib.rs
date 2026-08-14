//! ndjson-viewer core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps, no I/O, no clock: the same paste always
//! produces the same output.
//!
//! Reads newline-delimited JSON (NDJSON / JSON Lines): one complete JSON value
//! per line. Blank lines are skipped, a leading UTF-8 BOM is stripped and CRLF
//! endings are handled. Every line is parsed on its own, so one malformed line
//! never costs you the rest of the stream — it is reported in place, dropped, or
//! turned into an error, depending on `invalid`.
//!
//! What it does with the records:
//! - **renders** them as one indented record per block (`pretty`), one minified
//!   record per line (`compact`), a single JSON array (`array`), or an aligned
//!   text table whose columns are the keys discovered across the records
//!   (`table`);
//! - **searches** them — `search` matches a key name or a scalar value at ANY
//!   depth, so you can find `timeout` without knowing which field holds it;
//! - **filters** them by an explicit dotted `path` (`user.name`, `items.0.id`),
//!   optionally requiring a `value` at that path;
//! - **paginates** them with `skip` + `limit`, and optionally prefixes each
//!   record with its input line number.
//!
//! Deliberately NOT here (they belong to neighbouring blocks): a boolean
//! predicate language and field selection/renaming (`ndjson-filter`), and CSV
//! output (`data-format-converter`).

use regex::{Regex, RegexBuilder};
use serde::Serialize;
use serde_json::{Map, Value};

/// The most non-blank lines one run will read. Past this the browser tab and the
/// wasm heap are the real limit, so it fails with a clear message instead.
pub const MAX_LINES: usize = 50_000;

/// How many distinct top-level keys the `stats` header lists before it truncates.
const MAX_LISTED_KEYS: usize = 20;

// ------------------------------------------------------------------ options

#[derive(Clone, Copy, PartialEq, Debug)]
enum View {
    Pretty,
    Compact,
    Array,
    Table,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SearchIn {
    Any,
    Keys,
    Values,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum MatchMode {
    Contains,
    Exact,
    Regex,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Invalid {
    Report,
    Skip,
    Error,
}

fn parse_view(s: &str) -> Result<View, String> {
    match s.trim() {
        "" | "pretty" => Ok(View::Pretty),
        "compact" | "minify" | "minified" => Ok(View::Compact),
        "array" => Ok(View::Array),
        "table" => Ok(View::Table),
        other => Err(format!(
            "view must be one of pretty, compact, array, table — got \"{other}\""
        )),
    }
}

fn parse_search_in(s: &str) -> Result<SearchIn, String> {
    match s.trim() {
        "" | "any" => Ok(SearchIn::Any),
        "keys" => Ok(SearchIn::Keys),
        "values" => Ok(SearchIn::Values),
        other => Err(format!(
            "search_in must be one of any, keys, values — got \"{other}\""
        )),
    }
}

fn parse_match_mode(s: &str) -> Result<MatchMode, String> {
    match s.trim() {
        "" | "contains" => Ok(MatchMode::Contains),
        "exact" => Ok(MatchMode::Exact),
        "regex" => Ok(MatchMode::Regex),
        other => Err(format!(
            "match_mode must be one of contains, exact, regex — got \"{other}\""
        )),
    }
}

fn parse_invalid(s: &str) -> Result<Invalid, String> {
    match s.trim() {
        "" | "report" => Ok(Invalid::Report),
        "skip" => Ok(Invalid::Skip),
        "error" => Ok(Invalid::Error),
        other => Err(format!(
            "invalid must be one of report, skip, error — got \"{other}\""
        )),
    }
}

// ------------------------------------------------------------------ matching

/// One compiled needle, reused across every record and every nested value.
struct Matcher {
    needle: String,
    lowered: String,
    mode: MatchMode,
    case_sensitive: bool,
    re: Option<Regex>,
}

impl Matcher {
    fn new(needle: &str, mode: MatchMode, case_sensitive: bool) -> Result<Self, String> {
        let re = if mode == MatchMode::Regex {
            Some(
                RegexBuilder::new(needle)
                    .case_insensitive(!case_sensitive)
                    .build()
                    .map_err(|e| format!("\"{needle}\" is not a valid regular expression: {e}"))?,
            )
        } else {
            None
        };
        Ok(Self {
            needle: needle.to_string(),
            lowered: needle.to_lowercase(),
            mode,
            case_sensitive,
            re,
        })
    }

    fn is_match(&self, hay: &str) -> bool {
        match self.mode {
            MatchMode::Regex => self.re.as_ref().is_some_and(|r| r.is_match(hay)),
            MatchMode::Contains => {
                if self.case_sensitive {
                    hay.contains(&self.needle)
                } else {
                    hay.to_lowercase().contains(&self.lowered)
                }
            }
            MatchMode::Exact => {
                if self.case_sensitive {
                    hay == self.needle
                } else {
                    hay.to_lowercase() == self.lowered
                }
            }
        }
    }
}

/// The text a value is compared/searched as: strings by their contents, every
/// other scalar by its JSON form, containers by their compact JSON.
fn scalar_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Does this needle hit any key name and/or scalar value anywhere in the record?
fn matches_anywhere(v: &Value, m: &Matcher, si: SearchIn) -> bool {
    match v {
        Value::Object(map) => map.iter().any(|(k, val)| {
            (matches!(si, SearchIn::Any | SearchIn::Keys) && m.is_match(k))
                || matches_anywhere(val, m, si)
        }),
        Value::Array(items) => items.iter().any(|it| matches_anywhere(it, m, si)),
        scalar => {
            matches!(si, SearchIn::Any | SearchIn::Values) && m.is_match(&scalar_text(scalar))
        }
    }
}

/// Walk a dotted path — object keys, and numeric segments indexing into arrays.
fn lookup<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for seg in path.split('.') {
        if seg.is_empty() {
            return None;
        }
        cur = match cur {
            Value::Object(map) => map.get(seg)?,
            Value::Array(items) => items.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

// ------------------------------------------------------------------ rendering

fn pretty_json(v: &Value, indent: usize) -> String {
    if indent == 0 {
        return v.to_string();
    }
    let pad = " ".repeat(indent);
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(pad.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    v.serialize(&mut ser)
        .expect("re-serializing an already-parsed JSON value cannot fail");
    String::from_utf8(buf).expect("serde_json always emits UTF-8")
}

/// Recursively reorder every object's keys alphabetically.
fn sort_value(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            let mut out = Map::new();
            for (k, val) in entries {
                out.insert(k.clone(), sort_value(val));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(sort_value).collect()),
        other => other.clone(),
    }
}

/// Keep table rows on one line each.
fn cell_text(v: &Value) -> String {
    scalar_text(v)
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// serde_json reports "line 1 column N" for a single line, which reads oddly
/// when the caller already knows the real line number.
fn clean_err(e: &serde_json::Error) -> String {
    let text = e.to_string();
    const AT: &str = " at line 1 column ";
    match text.find(AT) {
        Some(i) => format!("{} at column {}", &text[..i], &text[i + AT.len()..]),
        None => text,
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

// ------------------------------------------------------------------ run

struct Record {
    line: usize,
    index: usize,
    value: Value,
}

/// One item of interleaved output: a kept record (already sorted/rendered) or a
/// malformed input line.
enum Item<'a> {
    Rec {
        line: usize,
        index: usize,
        value: &'a Value,
    },
    Bad(usize, &'a str),
}

impl Item<'_> {
    fn line(&self) -> usize {
        match self {
            Item::Rec { line, .. } => *line,
            Item::Bad(line, _) => *line,
        }
    }
}

/// Render an NDJSON stream. See the module docs for what each option means.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    view: &str,
    indent: i64,
    search: &str,
    search_in: &str,
    case_sensitive: bool,
    path: &str,
    value: &str,
    match_mode: &str,
    sort_keys: bool,
    skip: i64,
    limit: i64,
    invalid: &str,
    line_numbers: bool,
    stats: bool,
) -> Result<String, String> {
    let view = parse_view(view)?;
    let search_in = parse_search_in(search_in)?;
    let match_mode = parse_match_mode(match_mode)?;
    let invalid = parse_invalid(invalid)?;

    if !(0..=8).contains(&indent) {
        return Err(format!(
            "indent must be between 0 (minified) and 8 spaces — got {indent}"
        ));
    }
    let indent = indent as usize;
    if skip < 0 {
        return Err(format!("skip must be 0 or more records — got {skip}"));
    }
    if limit < 0 {
        return Err(format!(
            "limit must be 0 (show every matching record) or more — got {limit}"
        ));
    }
    let skip = skip as usize;
    let limit = limit as usize;

    let data = data.strip_prefix('\u{feff}').unwrap_or(data);
    let path = path.trim();

    let line_count = data.lines().filter(|l| !l.trim().is_empty()).count();
    if line_count == 0 {
        return Err("no NDJSON to view: paste one complete JSON value per line, for example \
                    {\"id\":1,\"status\":\"ok\"} on the first line and {\"id\":2,\"status\":\"error\"} \
                    on the second"
            .to_string());
    }
    if line_count > MAX_LINES {
        return Err(format!(
            "input has {line_count} non-blank lines, which is over the {MAX_LINES}-line limit for \
             one run — split the stream and view it in parts"
        ));
    }

    // ---- parse every line on its own
    let mut records: Vec<Record> = Vec::new();
    let mut bad: Vec<(usize, String)> = Vec::new();
    let mut invalid_count = 0usize;
    for (i, raw) in data.lines().enumerate() {
        let line_no = i + 1;
        let text = raw.trim();
        if text.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(text) {
            Ok(v) => records.push(Record {
                line: line_no,
                index: records.len() + 1,
                value: v,
            }),
            Err(e) => {
                invalid_count += 1;
                match invalid {
                    Invalid::Error => {
                        return Err(format!(
                            "invalid JSON on line {line_no}: {} — every line must be one complete \
                             JSON value; use invalid=report to mark bad lines in place or \
                             invalid=skip to drop them",
                            clean_err(&e)
                        ))
                    }
                    Invalid::Report => bad.push((line_no, clean_err(&e))),
                    Invalid::Skip => {}
                }
            }
        }
    }

    // ---- filter
    let search_matcher = if search.is_empty() {
        None
    } else {
        Some(Matcher::new(search, match_mode, case_sensitive)?)
    };
    let value_matcher = if value.is_empty() {
        None
    } else {
        Some(Matcher::new(value, match_mode, case_sensitive)?)
    };

    let matched: Vec<&Record> = records
        .iter()
        .filter(|r| {
            if let Some(m) = &search_matcher {
                if !matches_anywhere(&r.value, m, search_in) {
                    return false;
                }
            }
            if !path.is_empty() {
                match lookup(&r.value, path) {
                    None => return false,
                    Some(found) => {
                        if let Some(m) = &value_matcher {
                            if !m.is_match(&scalar_text(found)) {
                                return false;
                            }
                        }
                    }
                }
            } else if let Some(m) = &value_matcher {
                if !matches_anywhere(&r.value, m, SearchIn::Values) {
                    return false;
                }
            }
            true
        })
        .collect();

    let shown: Vec<&Record> = matched
        .iter()
        .copied()
        .skip(skip)
        .take(if limit == 0 { usize::MAX } else { limit })
        .collect();

    // ---- render
    let rendered: Vec<Value> = shown
        .iter()
        .map(|r| {
            if sort_keys {
                sort_value(&r.value)
            } else {
                r.value.clone()
            }
        })
        .collect();

    let markers: Vec<String> = bad
        .iter()
        .map(|(line, err)| format!("# line {line}: invalid JSON — {err}"))
        .collect();

    let empty_note = if matched.is_empty() {
        Some("# no records matched the filters".to_string())
    } else if shown.is_empty() {
        Some(format!(
            "# no records to show — {} matched, but skip={skip} is past the last one",
            plural(matched.len(), "record", "records")
        ))
    } else {
        None
    };

    let mut body: Vec<String> = Vec::new();
    match view {
        View::Pretty | View::Compact => {
            // Malformed lines stay where they were in the input, so the output
            // reads in the same order as the paste.
            let mut items: Vec<Item> = shown
                .iter()
                .zip(rendered.iter())
                .map(|(r, value)| Item::Rec {
                    line: r.line,
                    index: r.index,
                    value,
                })
                .collect();
            items.extend(bad.iter().map(|(line, err)| Item::Bad(*line, err.as_str())));
            items.sort_by_key(|it| it.line());

            for it in &items {
                match it {
                    Item::Bad(line, err) => {
                        body.push(format!("# line {line}: invalid JSON — {err}"))
                    }
                    Item::Rec { line, index, value } => {
                        if view == View::Pretty {
                            let json = pretty_json(value, indent);
                            body.push(if line_numbers {
                                format!("# record {index} (line {line})\n{json}")
                            } else {
                                json
                            });
                        } else if line_numbers {
                            body.push(format!("{line}: {value}"));
                        } else {
                            body.push(value.to_string());
                        }
                    }
                }
            }
            if let Some(note) = &empty_note {
                body.push(note.clone());
            }
        }
        View::Array => {
            body.push(pretty_json(&Value::Array(rendered.clone()), indent));
            body.extend(markers.iter().cloned());
        }
        View::Table => {
            if rendered.is_empty() {
                body.push(
                    empty_note
                        .clone()
                        .unwrap_or_else(|| "# no records matched the filters".to_string()),
                );
            } else {
                body.push(render_table(&shown, &rendered, line_numbers));
            }
            body.extend(markers.iter().cloned());
        }
    }

    let mut out: Vec<String> = Vec::new();
    if stats {
        out.push(format!(
            "# {} read · {} · {}",
            plural(line_count, "non-blank line", "non-blank lines"),
            plural(records.len(), "record", "records"),
            plural(invalid_count, "invalid line", "invalid lines"),
        ));
        out.push(format!(
            "# {} matched · {} shown (skip {skip}, limit {})",
            matched.len(),
            shown.len(),
            if limit == 0 {
                "none".to_string()
            } else {
                limit.to_string()
            },
        ));
        out.push(format!("# top-level keys: {}", key_inventory(&records)));
    }

    let separator = if view == View::Pretty { "\n\n" } else { "\n" };
    let body_text = body.join(separator);
    if out.is_empty() {
        Ok(body_text)
    } else {
        out.push(body_text);
        Ok(out.join("\n"))
    }
}

/// Top-level key names in first-seen order, each with how many records carry it.
fn key_inventory(records: &[Record]) -> String {
    let mut order: Vec<String> = Vec::new();
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut non_objects = 0usize;
    for r in records {
        match &r.value {
            Value::Object(map) => {
                for k in map.keys() {
                    if !counts.contains_key(k) {
                        order.push(k.clone());
                    }
                    *counts.entry(k.clone()).or_insert(0) += 1;
                }
            }
            _ => non_objects += 1,
        }
    }
    if order.is_empty() {
        return format!(
            "none — {} carry no object keys",
            plural(non_objects, "record", "records")
        );
    }
    let listed: Vec<String> = order
        .iter()
        .take(MAX_LISTED_KEYS)
        .map(|k| format!("{k} ({})", counts[k]))
        .collect();
    let mut text = listed.join(", ");
    if order.len() > MAX_LISTED_KEYS {
        text.push_str(&format!(", … and {} more", order.len() - MAX_LISTED_KEYS));
    }
    if non_objects > 0 {
        text.push_str(&format!(
            " (plus {} with no object keys)",
            plural(non_objects, "record", "records")
        ));
    }
    text
}

/// An aligned text table: one column per key discovered across the shown
/// records, in first-seen order. Records that are not objects land in a
/// trailing `(value)` column.
fn render_table(shown: &[&Record], rendered: &[Value], line_numbers: bool) -> String {
    let mut columns: Vec<String> = Vec::new();
    let mut needs_value_column = false;
    for v in rendered {
        match v {
            Value::Object(map) => {
                for k in map.keys() {
                    if !columns.iter().any(|c| c == k) {
                        columns.push(k.clone());
                    }
                }
            }
            _ => needs_value_column = true,
        }
    }
    if needs_value_column {
        columns.push("(value)".to_string());
    }

    let mut header: Vec<String> = Vec::new();
    if line_numbers {
        header.push("line".to_string());
    }
    header.extend(columns.iter().cloned());

    let mut rows: Vec<Vec<String>> = Vec::new();
    for (rec, v) in shown.iter().zip(rendered.iter()) {
        let mut row: Vec<String> = Vec::new();
        if line_numbers {
            row.push(rec.line.to_string());
        }
        for col in &columns {
            let cell = match v {
                Value::Object(map) => map.get(col).map(cell_text).unwrap_or_default(),
                other => {
                    if col == "(value)" {
                        cell_text(other)
                    } else {
                        String::new()
                    }
                }
            };
            row.push(cell);
        }
        rows.push(row);
    }

    let widths: Vec<usize> = header
        .iter()
        .enumerate()
        .map(|(i, h)| {
            rows.iter()
                .map(|r| r[i].chars().count())
                .max()
                .unwrap_or(0)
                .max(h.chars().count())
        })
        .collect();

    let pad = |cells: &[String]| -> String {
        let line: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{c}{}", " ".repeat(widths[i] - c.chars().count())))
            .collect();
        line.join(" | ").trim_end().to_string()
    };

    let rule: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    let mut lines = vec![pad(&header), rule.join("-+-")];
    for row in &rows {
        lines.push(pad(row));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = r#"{"id":1,"status":"ok","latency_ms":12}
{"id":2,"status":"error","latency_ms":940,"err":{"code":"timeout"}}
{"id":3,"status":"ok","latency_ms":31}"#;

    fn view(data: &str, view: &str) -> String {
        run(
            data, view, 2, "", "any", false, "", "", "contains", false, 0, 0, "report", false,
            false,
        )
        .unwrap()
    }

    #[test]
    fn pretty_indents_each_record() {
        let out = view(r#"{"id":1,"status":"ok"}"#, "pretty");
        assert_eq!(out, "{\n  \"id\": 1,\n  \"status\": \"ok\"\n}");
    }

    #[test]
    fn pretty_keeps_input_key_order_and_separates_records() {
        let out = view("{\"b\":1,\"a\":2}\n{\"c\":3}", "pretty");
        assert_eq!(out, "{\n  \"b\": 1,\n  \"a\": 2\n}\n\n{\n  \"c\": 3\n}");
    }

    #[test]
    fn compact_minifies_one_record_per_line() {
        let out = view("{ \"id\" : 1 }\n\n{ \"id\" : 2 }\n", "compact");
        assert_eq!(out, "{\"id\":1}\n{\"id\":2}");
    }

    #[test]
    fn array_collects_records() {
        let out = view("{\"a\":1}\n{\"a\":2}", "array");
        assert_eq!(out, "[\n  {\n    \"a\": 1\n  },\n  {\n    \"a\": 2\n  }\n]");
    }

    #[test]
    fn indent_zero_minifies_the_array() {
        let out = run(
            "{\"a\":1}\n{\"a\":2}",
            "array",
            0,
            "",
            "any",
            false,
            "",
            "",
            "contains",
            false,
            0,
            0,
            "report",
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "[{\"a\":1},{\"a\":2}]");
    }

    #[test]
    fn table_unions_keys_in_first_seen_order() {
        let out = view("{\"a\":1,\"b\":2}\n{\"a\":3,\"c\":true}", "table");
        assert_eq!(out, "a | b | c\n--+---+-----\n1 | 2 |\n3 |   | true");
    }

    #[test]
    fn table_renders_nested_values_as_compact_json() {
        let out = view("{\"id\":1,\"tags\":[\"a\",\"b\"]}", "table");
        assert_eq!(out, "id | tags\n---+----------\n1  | [\"a\",\"b\"]");
    }

    #[test]
    fn table_puts_non_objects_in_a_value_column() {
        let out = view("{\"a\":1}\n\"plain string\"", "table");
        assert_eq!(out, "a | (value)\n--+-------------\n1 |\n  | plain string");
    }

    #[test]
    fn search_matches_a_nested_value_without_knowing_the_path() {
        let out = run(
            DATA, "compact", 2, "timeout", "any", false, "", "", "contains", false, 0, 0, "report",
            false, false,
        )
        .unwrap();
        assert_eq!(
            out,
            "{\"id\":2,\"status\":\"error\",\"latency_ms\":940,\"err\":{\"code\":\"timeout\"}}"
        );
    }

    #[test]
    fn search_in_keys_ignores_values() {
        let keys = run(
            DATA, "compact", 2, "err", "keys", false, "", "", "contains", false, 0, 0, "report",
            false, false,
        )
        .unwrap();
        assert_eq!(keys.lines().count(), 1, "only record 2 has an err key");
        let values = run(
            DATA, "compact", 2, "err", "values", false, "", "", "contains", false, 0, 0, "report",
            false, false,
        )
        .unwrap();
        assert_eq!(
            values.lines().count(),
            1,
            "only record 2 has value \"error\""
        );
        assert!(values.contains("\"status\":\"error\""));
    }

    #[test]
    fn search_is_case_insensitive_by_default_and_exact_when_asked() {
        let insensitive = run(
            DATA, "compact", 2, "TIMEOUT", "any", false, "", "", "contains", false, 0, 0, "report",
            false, false,
        )
        .unwrap();
        assert!(insensitive.contains("timeout"));
        let sensitive = run(
            DATA, "compact", 2, "TIMEOUT", "any", true, "", "", "contains", false, 0, 0, "report",
            false, false,
        )
        .unwrap();
        assert_eq!(sensitive, "# no records matched the filters");
    }

    #[test]
    fn regex_search_matches() {
        let out = run(
            DATA, "compact", 2, "^ok$", "values", false, "", "", "regex", false, 0, 0, "report",
            false, false,
        )
        .unwrap();
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn path_filters_on_existence_and_value() {
        let exists = run(
            DATA, "compact", 2, "", "any", false, "err.code", "", "contains", false, 0, 0,
            "report", false, false,
        )
        .unwrap();
        assert!(exists.contains("\"id\":2"));
        assert_eq!(exists.lines().count(), 1);

        let equal = run(
            DATA, "compact", 2, "", "any", false, "status", "ok", "exact", false, 0, 0, "report",
            false, false,
        )
        .unwrap();
        assert_eq!(equal.lines().count(), 2);
    }

    #[test]
    fn path_indexes_into_arrays() {
        let out = run(
            "{\"items\":[{\"id\":\"a\"},{\"id\":\"b\"}]}\n{\"items\":[]}",
            "compact",
            2,
            "",
            "any",
            false,
            "items.1.id",
            "b",
            "exact",
            false,
            0,
            0,
            "report",
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "{\"items\":[{\"id\":\"a\"},{\"id\":\"b\"}]}");
    }

    #[test]
    fn skip_and_limit_paginate() {
        let out = run(
            DATA, "compact", 2, "", "any", false, "", "", "contains", false, 1, 1, "report", false,
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "{\"id\":2,\"status\":\"error\",\"latency_ms\":940,\"err\":{\"code\":\"timeout\"}}"
        );
    }

    #[test]
    fn skip_past_the_end_says_so() {
        let out = run(
            DATA, "compact", 2, "", "any", false, "", "", "contains", false, 9, 0, "report", false,
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "# no records to show — 3 records matched, but skip=9 is past the last one"
        );
    }

    #[test]
    fn sort_keys_reorders_every_level() {
        let out = run(
            "{\"b\":{\"z\":1,\"y\":2},\"a\":3}",
            "compact",
            2,
            "",
            "any",
            false,
            "",
            "",
            "contains",
            true,
            0,
            0,
            "report",
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "{\"a\":3,\"b\":{\"y\":2,\"z\":1}}");
    }

    #[test]
    fn line_numbers_label_records() {
        let pretty = run(
            "{\"a\":1}\n\n{\"a\":2}",
            "pretty",
            0,
            "",
            "any",
            false,
            "",
            "",
            "contains",
            false,
            0,
            0,
            "report",
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            pretty,
            "# record 1 (line 1)\n{\"a\":1}\n\n# record 2 (line 3)\n{\"a\":2}"
        );
        let compact = run(
            "{\"a\":1}\n\n{\"a\":2}",
            "compact",
            2,
            "",
            "any",
            false,
            "",
            "",
            "contains",
            false,
            0,
            0,
            "report",
            true,
            false,
        )
        .unwrap();
        assert_eq!(compact, "1: {\"a\":1}\n3: {\"a\":2}");
    }

    #[test]
    fn invalid_lines_are_reported_in_place_by_default() {
        let out = view("{\"a\":1}\nnot json\n{\"a\":2}", "compact");
        assert_eq!(
            out,
            "{\"a\":1}\n# line 2: invalid JSON — expected ident at column 2\n{\"a\":2}"
        );
    }

    #[test]
    fn invalid_lines_can_be_skipped() {
        let out = run(
            "{\"a\":1}\nnot json\n{\"a\":2}",
            "compact",
            2,
            "",
            "any",
            false,
            "",
            "",
            "contains",
            false,
            0,
            0,
            "skip",
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "{\"a\":1}\n{\"a\":2}");
    }

    #[test]
    fn invalid_lines_can_be_a_hard_error() {
        let err = run(
            "{\"a\":1}\n{oops}",
            "compact",
            2,
            "",
            "any",
            false,
            "",
            "",
            "contains",
            false,
            0,
            0,
            "error",
            false,
            false,
        )
        .unwrap_err();
        assert!(err.starts_with("invalid JSON on line 2:"), "got {err}");
    }

    #[test]
    fn stats_header_counts_and_lists_keys() {
        let out = run(
            "{\"id\":1}\nbroken\n{\"id\":2,\"ok\":true}",
            "compact",
            2,
            "",
            "any",
            false,
            "",
            "",
            "contains",
            false,
            0,
            0,
            "skip",
            false,
            true,
        )
        .unwrap();
        let header: Vec<&str> = out.lines().take(3).collect();
        assert_eq!(
            header[0],
            "# 3 non-blank lines read · 2 records · 1 invalid line"
        );
        assert_eq!(header[1], "# 2 matched · 2 shown (skip 0, limit none)");
        assert_eq!(header[2], "# top-level keys: id (2), ok (1)");
    }

    #[test]
    fn empty_input_errors() {
        let err = run(
            "   \n\n", "pretty", 2, "", "any", false, "", "", "contains", false, 0, 0, "report",
            false, false,
        )
        .unwrap_err();
        assert!(err.starts_with("no NDJSON to view:"), "got {err}");
    }

    #[test]
    fn over_the_line_cap_errors() {
        let big = "{\"a\":1}\n".repeat(MAX_LINES + 1);
        let err = run(
            &big, "compact", 2, "", "any", false, "", "", "contains", false, 0, 0, "report", false,
            false,
        )
        .unwrap_err();
        assert!(err.contains("over the 50000-line limit"), "got {err}");
    }

    #[test]
    fn unknown_option_values_error_with_the_choices() {
        let err = run(
            DATA, "cards", 2, "", "any", false, "", "", "contains", false, 0, 0, "report", false,
            false,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "view must be one of pretty, compact, array, table — got \"cards\""
        );
    }

    #[test]
    fn a_broken_regex_says_so() {
        let err = run(
            DATA,
            "compact",
            2,
            "[unclosed",
            "any",
            false,
            "",
            "",
            "regex",
            false,
            0,
            0,
            "report",
            false,
            false,
        )
        .unwrap_err();
        assert!(
            err.starts_with("\"[unclosed\" is not a valid regular expression:"),
            "got {err}"
        );
    }

    #[test]
    fn out_of_range_indent_errors() {
        let err = run(
            DATA, "pretty", 9, "", "any", false, "", "", "contains", false, 0, 0, "report", false,
            false,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "indent must be between 0 (minified) and 8 spaces — got 9"
        );
    }
}
