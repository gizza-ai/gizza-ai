//! jsonl-stats core — pure compute, shared by the chat skill block and the web page.
//!
//! Profiles a JSON Lines / NDJSON stream: how many records it holds, which keys
//! appear in how many of them (presence + coverage), what JSON types each key's
//! values take, how many distinct scalar values each key has, and min/max/mean
//! for numeric keys plus min/max length for string keys. Nested objects and
//! array elements are reachable with `depth` (`user.id`, `items[].sku`).
//!
//! No I/O and no clock: the same paste always produces the same report.

use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Non-blank input lines accepted per run (matches the NDJSON tool family).
pub const MAX_LINES: usize = 50_000;
/// Deepest nesting level `depth` may ask for.
pub const MAX_DEPTH: i64 = 10;
/// Most sample values that can be shown per key.
pub const MAX_SAMPLES: i64 = 5;
/// Distinct scalar values tracked per key before the count is reported as "N+".
const DISTINCT_CAP: usize = 10_000;
/// Invalid lines listed individually before the report says "and N more".
const INVALID_LISTED: usize = 10;

const TYPE_NAMES: [&str; 6] = ["string", "number", "boolean", "null", "object", "array"];

fn type_index(v: &Value) -> usize {
    match v {
        Value::String(_) => 0,
        Value::Number(_) => 1,
        Value::Bool(_) => 2,
        Value::Null => 3,
        Value::Object(_) => 4,
        Value::Array(_) => 5,
    }
}

struct KeyStat {
    order: usize,
    present: usize,
    counts: [usize; 6],
    distinct: HashSet<String>,
    distinct_overflow: bool,
    has_scalar: bool,
    samples: Vec<String>,
    num_count: usize,
    num_sum: f64,
    num_min: f64,
    num_max: f64,
    str_count: usize,
    str_min_len: usize,
    str_max_len: usize,
}

impl KeyStat {
    fn new(order: usize) -> Self {
        KeyStat {
            order,
            present: 0,
            counts: [0; 6],
            distinct: HashSet::new(),
            distinct_overflow: false,
            has_scalar: false,
            samples: Vec::new(),
            num_count: 0,
            num_sum: 0.0,
            num_min: f64::INFINITY,
            num_max: f64::NEG_INFINITY,
            str_count: 0,
            str_min_len: usize::MAX,
            str_max_len: 0,
        }
    }
}

struct Profile {
    stats: HashMap<String, KeyStat>,
    order: usize,
    samples_wanted: usize,
}

impl Profile {
    /// Record one value at `path`, then descend while `depth_left` allows.
    /// `seen` holds the paths already credited for the CURRENT record, so
    /// presence counts records, not occurrences (an array of 50 objects still
    /// counts as one record carrying `items[].sku`).
    fn visit(&mut self, path: &str, v: &Value, depth_left: usize, seen: &mut HashSet<String>) {
        let order = &mut self.order;
        let samples_wanted = self.samples_wanted;
        let stat = self.stats.entry(path.to_string()).or_insert_with(|| {
            let o = *order;
            *order += 1;
            KeyStat::new(o)
        });
        if !seen.contains(path) {
            seen.insert(path.to_string());
            stat.present += 1;
        }
        stat.counts[type_index(v)] += 1;

        // Distinct values + samples are tracked for SCALARS only: a whole
        // nested object's text is unbounded and rarely repeats anyway.
        match v {
            Value::Object(_) | Value::Array(_) => {}
            _ => {
                stat.has_scalar = true;
                let text = serde_json::to_string(v).unwrap_or_else(|_| "null".to_string());
                if !stat.distinct.contains(&text) {
                    if stat.distinct.len() < DISTINCT_CAP {
                        if stat.samples.len() < samples_wanted {
                            stat.samples.push(text.clone());
                        }
                        stat.distinct.insert(text);
                    } else {
                        stat.distinct_overflow = true;
                    }
                }
            }
        }
        if let Value::Number(n) = v {
            if let Some(x) = n.as_f64() {
                stat.num_count += 1;
                stat.num_sum += x;
                if x < stat.num_min {
                    stat.num_min = x;
                }
                if x > stat.num_max {
                    stat.num_max = x;
                }
            }
        }
        if let Value::String(s) = v {
            let len = s.chars().count();
            stat.str_count += 1;
            stat.str_min_len = stat.str_min_len.min(len);
            stat.str_max_len = stat.str_max_len.max(len);
        }

        if depth_left == 0 {
            return;
        }
        match v {
            Value::Object(map) => {
                for (k, sub) in map {
                    let child = format!("{path}.{k}");
                    self.visit(&child, sub, depth_left - 1, seen);
                }
            }
            Value::Array(items) => {
                let child = format!("{path}[]");
                for sub in items {
                    self.visit(&child, sub, depth_left - 1, seen);
                }
            }
            _ => {}
        }
    }
}

/// Print a float the way a person would write it: whole numbers without a
/// trailing `.0`, everything else trimmed to at most 6 decimals.
fn fmt_num(x: f64) -> String {
    if !x.is_finite() {
        return format!("{x}");
    }
    if x == x.trunc() && x.abs() < 1e15 {
        return format!("{}", x as i64);
    }
    let s = format!("{x:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// Coverage as a percentage: whole numbers stay whole, the rest get 1 decimal.
fn fmt_pct(present: usize, records: usize) -> String {
    if records == 0 {
        return "0%".to_string();
    }
    let pct = present as f64 * 100.0 / records as f64;
    if (pct - pct.round()).abs() < 1e-9 {
        format!("{}%", pct.round() as i64)
    } else {
        format!("{pct:.1}%")
    }
}

struct Row {
    key: String,
    present: String,
    coverage: String,
    types: String,
    distinct: String,
    values: String,
    samples: String,
}

/// One profiled key, ready for any of the four output formats.
struct Analysis {
    records: usize,
    lines_read: usize,
    invalid_count: usize,
    invalid: Vec<(usize, String)>,
    record_types: [usize; 6],
    depth: usize,
    keys_total: usize,
    keys: Vec<(String, KeyStat)>,
    show_distinct: bool,
    show_values: bool,
    samples_wanted: usize,
}

fn types_text(counts: &[usize; 6]) -> String {
    let mut present: Vec<usize> = (0..6).filter(|&i| counts[i] > 0).collect();
    present.sort_by(|&a, &b| counts[b].cmp(&counts[a]).then(a.cmp(&b)));
    present
        .iter()
        .map(|&i| format!("{} {}", TYPE_NAMES[i], counts[i]))
        .collect::<Vec<_>>()
        .join(", ")
}

fn distinct_text(stat: &KeyStat) -> String {
    if !stat.has_scalar {
        return "-".to_string();
    }
    if stat.distinct_overflow {
        format!("{DISTINCT_CAP}+")
    } else {
        stat.distinct.len().to_string()
    }
}

fn values_text(stat: &KeyStat) -> String {
    let mut parts = Vec::new();
    if stat.num_count > 0 {
        let mean = stat.num_sum / stat.num_count as f64;
        parts.push(format!(
            "min {}, max {}, mean {}",
            fmt_num(stat.num_min),
            fmt_num(stat.num_max),
            fmt_num(mean)
        ));
    }
    if stat.str_count > 0 {
        if stat.str_min_len == stat.str_max_len {
            parts.push(format!("length {}", stat.str_min_len));
        } else {
            parts.push(format!("length {}-{}", stat.str_min_len, stat.str_max_len));
        }
    }
    parts.join("; ")
}

impl Analysis {
    fn rows(&self) -> Vec<Row> {
        self.keys
            .iter()
            .map(|(key, stat)| Row {
                key: key.clone(),
                present: stat.present.to_string(),
                coverage: fmt_pct(stat.present, self.records),
                types: types_text(&stat.counts),
                distinct: distinct_text(stat),
                values: values_text(stat),
                samples: stat.samples.join(", "),
            })
            .collect()
    }

    fn headers(&self) -> Vec<&'static str> {
        let mut h = vec!["key", "present", "coverage", "types"];
        if self.show_distinct {
            h.push("distinct");
        }
        if self.show_values {
            h.push("values");
        }
        if self.samples_wanted > 0 {
            h.push("samples");
        }
        h
    }

    fn cells(&self, row: &Row) -> Vec<String> {
        let mut c = vec![
            row.key.clone(),
            row.present.clone(),
            row.coverage.clone(),
            row.types.clone(),
        ];
        if self.show_distinct {
            c.push(row.distinct.clone());
        }
        if self.show_values {
            c.push(row.values.clone());
        }
        if self.samples_wanted > 0 {
            c.push(row.samples.clone());
        }
        c
    }

    /// Right-align the count-ish columns; everything else reads left-to-right.
    fn right_aligned(&self) -> Vec<bool> {
        let mut a = vec![false, true, true, false];
        if self.show_distinct {
            a.push(true);
        }
        if self.show_values {
            a.push(false);
        }
        if self.samples_wanted > 0 {
            a.push(false);
        }
        a
    }

    fn summary_line(&self) -> String {
        format!(
            "records: {} · lines read: {} · invalid: {}",
            self.records, self.lines_read, self.invalid_count
        )
    }

    fn record_types_line(&self) -> String {
        format!("record types: {}", types_text(&self.record_types))
    }

    fn keys_line(&self) -> String {
        let shown = self.keys.len();
        if shown == self.keys_total {
            format!("keys: {} (depth {})", self.keys_total, self.depth)
        } else {
            format!(
                "keys: {} (depth {}), showing {}",
                self.keys_total, self.depth, shown
            )
        }
    }
}

fn text_report(a: &Analysis) -> String {
    let mut out = String::new();
    out.push_str(&a.summary_line());
    out.push('\n');
    if a.records > 0 {
        out.push_str(&a.record_types_line());
        out.push('\n');
    }
    out.push_str(&a.keys_line());
    out.push('\n');

    if a.keys.is_empty() {
        out.push('\n');
        out.push_str(
            "no object keys found — records are not JSON objects, or the stream is empty\n",
        );
    } else {
        let headers = a.headers();
        let rows = a.rows();
        let table: Vec<Vec<String>> = rows.iter().map(|r| a.cells(r)).collect();
        let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
        for row in &table {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.chars().count());
            }
        }
        let right = a.right_aligned();
        let line = |cells: &[String]| {
            let mut s = String::new();
            for (i, cell) in cells.iter().enumerate() {
                if i > 0 {
                    s.push_str("  ");
                }
                let pad = widths[i].saturating_sub(cell.chars().count());
                if right[i] {
                    s.push_str(&" ".repeat(pad));
                    s.push_str(cell);
                } else {
                    s.push_str(cell);
                    s.push_str(&" ".repeat(pad));
                }
            }
            format!("{}\n", s.trim_end())
        };
        out.push('\n');
        let head: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
        out.push_str(&line(&head));
        for row in &table {
            out.push_str(&line(row));
        }
    }

    if !a.invalid.is_empty() {
        out.push('\n');
        out.push_str("invalid lines:\n");
        for (line_no, err) in a.invalid.iter().take(INVALID_LISTED) {
            out.push_str(&format!("  line {line_no}: invalid JSON — {err}\n"));
        }
        if a.invalid.len() > INVALID_LISTED {
            out.push_str(&format!(
                "  … and {} more\n",
                a.invalid.len() - INVALID_LISTED
            ));
        }
    }
    out
}

fn json_report(a: &Analysis) -> String {
    let mut root = serde_json::Map::new();
    root.insert("records".into(), Value::from(a.records));
    root.insert("lines_read".into(), Value::from(a.lines_read));
    root.insert("invalid_lines".into(), Value::from(a.invalid_count));
    let mut rt = serde_json::Map::new();
    for i in 0..6 {
        if a.record_types[i] > 0 {
            rt.insert(TYPE_NAMES[i].into(), Value::from(a.record_types[i]));
        }
    }
    root.insert("record_types".into(), Value::Object(rt));
    root.insert("depth".into(), Value::from(a.depth));
    root.insert("keys_total".into(), Value::from(a.keys_total));
    root.insert("keys_shown".into(), Value::from(a.keys.len()));

    let mut keys = Vec::new();
    for (key, stat) in &a.keys {
        let mut k = serde_json::Map::new();
        k.insert("key".into(), Value::from(key.clone()));
        k.insert("present".into(), Value::from(stat.present));
        let coverage = if a.records == 0 {
            0.0
        } else {
            (stat.present as f64 / a.records as f64 * 1e6).round() / 1e6
        };
        k.insert(
            "coverage".into(),
            serde_json::Number::from_f64(coverage)
                .map(Value::Number)
                .unwrap_or(Value::Null),
        );
        let mut types = serde_json::Map::new();
        for i in 0..6 {
            if stat.counts[i] > 0 {
                types.insert(TYPE_NAMES[i].into(), Value::from(stat.counts[i]));
            }
        }
        k.insert("types".into(), Value::Object(types));
        if a.show_distinct {
            if stat.has_scalar {
                k.insert("distinct".into(), Value::from(stat.distinct.len()));
                if stat.distinct_overflow {
                    k.insert("distinct_capped".into(), Value::Bool(true));
                }
            } else {
                k.insert("distinct".into(), Value::Null);
            }
        }
        if a.show_values {
            if stat.num_count > 0 {
                let mean = stat.num_sum / stat.num_count as f64;
                for (name, x) in [("min", stat.num_min), ("max", stat.num_max), ("mean", mean)] {
                    k.insert(
                        name.into(),
                        serde_json::Number::from_f64(x)
                            .map(Value::Number)
                            .unwrap_or(Value::Null),
                    );
                }
            }
            if stat.str_count > 0 {
                k.insert("min_length".into(), Value::from(stat.str_min_len));
                k.insert("max_length".into(), Value::from(stat.str_max_len));
            }
        }
        if a.samples_wanted > 0 {
            k.insert(
                "samples".into(),
                Value::Array(
                    stat.samples
                        .iter()
                        .map(|s| Value::from(s.clone()))
                        .collect(),
                ),
            );
        }
        keys.push(Value::Object(k));
    }
    root.insert("keys".into(), Value::Array(keys));

    if !a.invalid.is_empty() {
        root.insert(
            "invalid".into(),
            Value::Array(
                a.invalid
                    .iter()
                    .map(|(line, err)| {
                        let mut m = serde_json::Map::new();
                        m.insert("line".into(), Value::from(*line));
                        m.insert("error".into(), Value::from(err.clone()));
                        Value::Object(m)
                    })
                    .collect(),
            ),
        );
    }
    let mut s =
        serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_else(|_| "{}".to_string());
    s.push('\n');
    s
}

fn markdown_report(a: &Analysis) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "**{} records** · {} lines read · {} invalid · {}\n",
        a.records,
        a.lines_read,
        a.invalid_count,
        a.keys_line()
    ));
    if a.keys.is_empty() {
        out.push_str(
            "\nNo object keys found — records are not JSON objects, or the stream is empty.\n",
        );
        return out;
    }
    let headers = a.headers();
    let right = a.right_aligned();
    out.push('\n');
    out.push_str(&format!("| {} |\n", headers.join(" | ")));
    let seps: Vec<&str> = right
        .iter()
        .map(|&r| if r { "---:" } else { "---" })
        .collect();
    out.push_str(&format!("| {} |\n", seps.join(" | ")));
    for row in a.rows() {
        let cells: Vec<String> = a
            .cells(&row)
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let c = c.replace('|', "\\|");
                if i == 0 {
                    format!("`{c}`")
                } else {
                    c
                }
            })
            .collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    out
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn csv_report(a: &Analysis) -> String {
    let mut out = String::new();
    out.push_str(
        &a.headers()
            .iter()
            .map(|h| csv_cell(h))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');
    for row in a.rows() {
        out.push_str(
            &a.cells(&row)
                .iter()
                .map(|c| csv_cell(c))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

/// Profile a JSON Lines / NDJSON stream.
///
/// * `data` — the stream, one complete JSON value per line.
/// * `depth` — 1 profiles top-level keys, higher walks into nested objects and
///   array elements (`user.id`, `items[].sku`). 1–10.
/// * `format` — `text` | `json` | `markdown` | `csv`.
/// * `sort` — `frequency` | `name` | `first-seen`.
/// * `max_keys` — cap on reported keys, 0 = every key.
/// * `samples` — first N distinct scalar values per key, 0–5.
/// * `value_stats` — numeric min/max/mean and string min/max length.
/// * `distinct` — distinct scalar value count per key.
/// * `invalid` — `report` (count + list), `skip` (count only) or `error`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    depth: i64,
    format: &str,
    sort: &str,
    max_keys: i64,
    samples: i64,
    value_stats: bool,
    distinct: bool,
    invalid: &str,
) -> Result<String, String> {
    let text = data.strip_prefix('\u{feff}').unwrap_or(data);
    if text.trim().is_empty() {
        return Err("data is empty: paste JSON Lines (NDJSON) text — one complete JSON value per line, e.g. {\"id\":1,\"status\":\"ok\"}".to_string());
    }
    if !(1..=MAX_DEPTH).contains(&depth) {
        return Err(format!(
            "depth must be a whole number between 1 (top-level keys only) and {MAX_DEPTH}, got {depth}"
        ));
    }
    if !matches!(format, "text" | "json" | "markdown" | "csv") {
        return Err(format!(
            "format must be one of text, json, markdown, csv — got \"{format}\""
        ));
    }
    if !matches!(sort, "frequency" | "name" | "first-seen") {
        return Err(format!(
            "sort must be one of frequency, name, first-seen — got \"{sort}\""
        ));
    }
    if max_keys < 0 {
        return Err(format!(
            "max_keys must be 0 (report every key) or a positive whole number, got {max_keys}"
        ));
    }
    if !(0..=MAX_SAMPLES).contains(&samples) {
        return Err(format!(
            "samples must be a whole number between 0 (none) and {MAX_SAMPLES}, got {samples}"
        ));
    }
    if !matches!(invalid, "report" | "skip" | "error") {
        return Err(format!(
            "invalid must be one of report, skip, error — got \"{invalid}\""
        ));
    }

    let mut profile = Profile {
        stats: HashMap::new(),
        order: 0,
        samples_wanted: samples as usize,
    };
    let mut records = 0usize;
    let mut lines_read = 0usize;
    let mut invalid_count = 0usize;
    let mut invalid_list: Vec<(usize, String)> = Vec::new();
    let mut record_types = [0usize; 6];

    for (idx, raw) in text.split('\n').enumerate() {
        let line = raw.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }
        lines_read += 1;
        if lines_read > MAX_LINES {
            return Err(format!(
                "too many lines: more than {MAX_LINES} non-blank lines — split the file or filter it down first"
            ));
        }
        let line_no = idx + 1;
        let parsed: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                if invalid == "error" {
                    return Err(format!("line {line_no}: invalid JSON — {e}"));
                }
                invalid_count += 1;
                if invalid == "report" {
                    invalid_list.push((line_no, e.to_string()));
                }
                continue;
            }
        };
        records += 1;
        record_types[type_index(&parsed)] += 1;
        if let Value::Object(map) = &parsed {
            let mut seen = HashSet::new();
            for (k, v) in map {
                profile.visit(k, v, (depth - 1) as usize, &mut seen);
            }
        }
    }

    let keys_total = profile.stats.len();
    let mut keys: Vec<(String, KeyStat)> = profile.stats.into_iter().collect();
    match sort {
        "name" => keys.sort_by(|a, b| a.0.cmp(&b.0)),
        "first-seen" => keys.sort_by_key(|(_, s)| s.order),
        _ => keys.sort_by(|a, b| {
            b.1.present
                .cmp(&a.1.present)
                .then(a.1.order.cmp(&b.1.order))
        }),
    }
    if max_keys > 0 {
        keys.truncate(max_keys as usize);
    }

    let analysis = Analysis {
        records,
        lines_read,
        invalid_count,
        invalid: invalid_list,
        record_types,
        depth: depth as usize,
        keys_total,
        keys,
        show_distinct: distinct,
        show_values: value_stats,
        samples_wanted: samples as usize,
    };

    Ok(match format {
        "json" => json_report(&analysis),
        "markdown" => markdown_report(&analysis),
        "csv" => csv_report(&analysis),
        _ => text_report(&analysis),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "{\"id\":1,\"status\":\"ok\",\"latency_ms\":12}\n{\"id\":2,\"status\":\"error\",\"latency_ms\":940,\"err\":{\"code\":\"timeout\"}}\n{\"id\":3,\"status\":\"ok\",\"latency_ms\":31}\n{\"id\":4,\"status\":\"ok\"}";

    fn text(data: &str) -> String {
        run(data, 1, "text", "frequency", 0, 0, true, true, "report").unwrap()
    }

    #[test]
    fn happy_path_text_report() {
        let out = text(SAMPLE);
        assert_eq!(
            out,
            "records: 4 · lines read: 4 · invalid: 0\n\
             record types: object 4\n\
             keys: 4 (depth 1)\n\
             \n\
             key         present  coverage  types     distinct  values\n\
             id                4      100%  number 4         4  min 1, max 4, mean 2.5\n\
             status            4      100%  string 4         2  length 2-5\n\
             latency_ms        3       75%  number 3         3  min 12, max 940, mean 327.666667\n\
             err               1       25%  object 1         -\n"
        );
    }

    #[test]
    fn coverage_counts_records_not_occurrences() {
        let out = text("{\"a\":1}\n{\"b\":2}\n{\"a\":3}\n{\"a\":4}");
        assert!(out.contains("a          3       75%"), "{out}");
        assert!(out.contains("b          1       25%"), "{out}");
    }

    #[test]
    fn depth_walks_nested_objects_and_arrays() {
        let out = run(
            "{\"user\":{\"id\":7},\"items\":[{\"sku\":\"a\"},{\"sku\":\"b\"}]}",
            3,
            "text",
            "name",
            0,
            0,
            false,
            false,
            "report",
        )
        .unwrap();
        assert!(out.contains("items[].sku"), "{out}");
        assert!(out.contains("user.id"), "{out}");
        // The array holds two elements but only one record carries the path.
        assert!(
            out.contains("items[].sku        1      100%  string 2"),
            "{out}"
        );
    }

    #[test]
    fn depth_one_stays_top_level() {
        let out = text("{\"user\":{\"id\":7}}");
        assert!(!out.contains("user.id"), "{out}");
        assert!(out.contains("user"), "{out}");
    }

    #[test]
    fn mixed_types_are_all_counted() {
        let out = text("{\"v\":1}\n{\"v\":null}\n{\"v\":\"x\"}\n{\"v\":true}");
        assert!(
            out.contains("string 1, number 1, boolean 1, null 1"),
            "{out}"
        );
    }

    #[test]
    fn json_format_is_machine_readable() {
        let out = run(SAMPLE, 1, "json", "frequency", 0, 2, true, true, "report").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["records"], 4);
        assert_eq!(v["keys_total"], 4);
        assert_eq!(v["keys"][0]["key"], "id");
        assert_eq!(v["keys"][0]["types"]["number"], 4);
        assert_eq!(v["keys"][0]["max"], 4.0);
        assert_eq!(v["keys"][0]["samples"][0], "1");
        assert_eq!(v["keys"][1]["min_length"], 2);
    }

    #[test]
    fn markdown_and_csv_render_the_same_rows() {
        let md = run(
            "{\"a\":1}\n{\"a\":2}",
            1,
            "markdown",
            "name",
            0,
            0,
            false,
            true,
            "skip",
        )
        .unwrap();
        assert!(md.contains("| `a` | 2 | 100% | number 2 | 2 |"), "{md}");
        let csv = run(
            "{\"a\":1}\n{\"a\":2}",
            1,
            "csv",
            "name",
            0,
            0,
            false,
            true,
            "skip",
        )
        .unwrap();
        assert_eq!(
            csv,
            "key,present,coverage,types,distinct\na,2,100%,number 2,2\n"
        );
    }

    #[test]
    fn max_keys_truncates_but_reports_the_total() {
        let out = text("{\"a\":1,\"b\":2,\"c\":3}\n{\"a\":1,\"b\":2}\n{\"a\":1}");
        assert!(out.contains("keys: 3 (depth 1)"), "{out}");
        let capped = run(
            "{\"a\":1,\"b\":2,\"c\":3}\n{\"a\":1,\"b\":2}\n{\"a\":1}",
            1,
            "text",
            "frequency",
            2,
            0,
            true,
            true,
            "report",
        )
        .unwrap();
        assert!(capped.contains("keys: 3 (depth 1), showing 2"), "{capped}");
        assert!(!capped.contains("\nc "), "{capped}");
    }

    #[test]
    fn invalid_lines_are_reported_with_their_line_number() {
        let out = text("{\"a\":1}\nnot json\n{\"a\":2}");
        assert!(
            out.contains("records: 2 · lines read: 3 · invalid: 1"),
            "{out}"
        );
        assert!(out.contains("  line 2: invalid JSON —"), "{out}");
    }

    #[test]
    fn invalid_skip_counts_without_listing() {
        let out = run(
            "{\"a\":1}\nnot json",
            1,
            "text",
            "frequency",
            0,
            0,
            true,
            true,
            "skip",
        )
        .unwrap();
        assert!(out.contains("invalid: 1"), "{out}");
        assert!(!out.contains("invalid lines:"), "{out}");
    }

    #[test]
    fn invalid_error_stops_at_the_first_bad_line() {
        let err = run(
            "{\"a\":1}\nnope",
            1,
            "text",
            "frequency",
            0,
            0,
            true,
            true,
            "error",
        )
        .unwrap_err();
        assert!(err.starts_with("line 2: invalid JSON — "), "{err}");
    }

    #[test]
    fn non_object_records_are_counted_but_have_no_keys() {
        let out = text("[1,2]\n\"hello\"\n42");
        assert!(
            out.contains("record types: string 1, number 1, array 1"),
            "{out}"
        );
        assert!(out.contains("no object keys found"), "{out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run(
            "   \n  ",
            1,
            "text",
            "frequency",
            0,
            0,
            true,
            true,
            "report",
        )
        .unwrap_err();
        assert!(err.starts_with("data is empty:"), "{err}");
    }

    #[test]
    fn out_of_range_and_unknown_choices_are_errors() {
        assert!(
            run("{}", 0, "text", "frequency", 0, 0, true, true, "report")
                .unwrap_err()
                .contains("depth must be")
        );
        assert!(
            run("{}", 11, "text", "frequency", 0, 0, true, true, "report")
                .unwrap_err()
                .contains("depth must be")
        );
        assert!(
            run("{}", 1, "yaml", "frequency", 0, 0, true, true, "report")
                .unwrap_err()
                .contains("format must be one of text, json, markdown, csv")
        );
        assert!(run("{}", 1, "text", "random", 0, 0, true, true, "report")
            .unwrap_err()
            .contains("sort must be one of"));
        assert!(
            run("{}", 1, "text", "frequency", -1, 0, true, true, "report")
                .unwrap_err()
                .contains("max_keys must be")
        );
        assert!(
            run("{}", 1, "text", "frequency", 0, 9, true, true, "report")
                .unwrap_err()
                .contains("samples must be")
        );
        assert!(run("{}", 1, "text", "frequency", 0, 0, true, true, "loud")
            .unwrap_err()
            .contains("invalid must be one of"));
    }

    #[test]
    fn bom_crlf_and_blank_lines_are_tolerated() {
        let out = text("\u{feff}{\"a\":1}\r\n\r\n{\"a\":2}\r\n");
        assert!(
            out.contains("records: 2 · lines read: 2 · invalid: 0"),
            "{out}"
        );
    }

    #[test]
    fn sort_orders_differ() {
        let data = "{\"z\":1,\"a\":1}\n{\"a\":2}";
        let by_freq = run(data, 1, "csv", "frequency", 0, 0, false, false, "skip").unwrap();
        assert!(
            by_freq.starts_with("key,present,coverage,types\na,"),
            "{by_freq}"
        );
        let by_name = run(data, 1, "csv", "name", 0, 0, false, false, "skip").unwrap();
        assert!(by_name.contains("\na,2,"), "{by_name}");
        let by_seen = run(data, 1, "csv", "first-seen", 0, 0, false, false, "skip").unwrap();
        assert!(
            by_seen.starts_with("key,present,coverage,types\nz,"),
            "{by_seen}"
        );
    }

    #[test]
    fn samples_show_the_first_distinct_values() {
        let out = run(
            "{\"s\":\"a\"}\n{\"s\":\"a\"}\n{\"s\":\"b\"}",
            1,
            "text",
            "frequency",
            0,
            3,
            false,
            true,
            "report",
        )
        .unwrap();
        assert!(out.contains("\"a\", \"b\""), "{out}");
    }
}
