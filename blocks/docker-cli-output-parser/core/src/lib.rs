//! docker-cli-output-parser core — pure compute, shared by the chat skill block
//! and the web page.
//!
//! Turns the *human* table that `docker ps`, `docker images` and `docker stats`
//! print into structured rows (JSON / CSV / TSV / Markdown / aligned text).
//!
//! The hard part is that the table is fixed-width and several headers contain
//! spaces — `CONTAINER ID`, `IMAGE ID`, `CREATED AT`, `MEM USAGE / LIMIT`,
//! `NET I/O`, `BLOCK I/O`, `CPU %`. Splitting a data row on whitespace is
//! therefore wrong (`COMMAND`, `STATUS` and `PORTS` all contain spaces too).
//! Instead the header line is used as a ruler: docker's tab writer pads every
//! column with at least two spaces and never lets a value overflow its column,
//! so the character offset of each header label is the exact start of that
//! column in every data row.
//!
//! No wafer/wasm-bindgen deps — deterministic pure Rust.

use serde_json::{Map, Value};

/// Upper bound on emitted rows (the `limit` param is clamped to `1..=MAX_LIMIT`).
pub const MAX_LIMIT: u32 = 5000;
/// Default row cap when `limit` is 0/unset.
pub const DEFAULT_LIMIT: u32 = 500;

/// One parsed cell. Values keep their type so JSON output is useful directly.
#[derive(Clone, Debug, PartialEq)]
enum Cell {
    Text(String),
    Num(f64),
    Int(i64),
    List(Vec<String>),
    Null,
}

impl Cell {
    /// Flat rendering used by csv/tsv/markdown/table.
    fn display(&self) -> String {
        match self {
            Cell::Text(s) => s.clone(),
            Cell::Num(n) => format_num(*n),
            Cell::Int(n) => n.to_string(),
            Cell::List(v) => v.join(", "),
            Cell::Null => String::new(),
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Cell::Text(s) => Value::String(s.clone()),
            Cell::Num(n) => serde_json::Number::from_f64(*n)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            Cell::Int(n) => Value::Number((*n).into()),
            Cell::List(v) => Value::Array(v.iter().map(|s| Value::String(s.clone())).collect()),
            Cell::Null => Value::Null,
        }
    }
}

/// Print a float without a trailing `.0` (docker percentages are 1–2 decimals).
fn format_num(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// How JSON/CSV keys are spelled.
#[derive(Clone, Copy, PartialEq)]
enum KeyStyle {
    Snake,
    Header,
    Docker,
}

/// Parse docker CLI table output into the chosen `output` shape.
///
/// - `input`: the pasted output, header line included.
/// - `kind`: `auto` (default), `ps`, `images` or `stats`.
/// - `output`: `json` (default), `csv`, `tsv`, `markdown` or `table`.
/// - `keys`: `snake` (default), `header` (verbatim column titles) or `docker`
///   (the `--format` template names such as `CPUPerc`).
/// - `parse_values`: split composite columns and type the numbers.
/// - `columns`: comma-separated subset/order of columns to emit (empty = all).
/// - `header`: emit a header row for csv/tsv/markdown/table.
/// - `strict`: fail on truncated rows and on a `kind` that does not match the
///   header, instead of parsing best-effort.
/// - `limit`: max rows (1..=5000, 0 → default 500).
#[allow(clippy::too_many_arguments)]
pub fn parse(
    input: &str,
    kind: &str,
    output: &str,
    keys: &str,
    parse_values: bool,
    columns: &str,
    header: bool,
    strict: bool,
    limit: u32,
) -> Result<String, String> {
    let kind = non_empty(kind, "auto");
    let output = non_empty(output, "json");
    let keys = non_empty(keys, "snake");

    if !matches!(kind, "auto" | "ps" | "images" | "stats") {
        return Err(format!(
            "unknown kind '{kind}': expected one of auto, ps, images, stats"
        ));
    }
    if !matches!(output, "json" | "csv" | "tsv" | "markdown" | "table") {
        return Err(format!(
            "unknown output '{output}': expected one of json, csv, tsv, markdown, table"
        ));
    }
    let style = match keys {
        "snake" => KeyStyle::Snake,
        "header" => KeyStyle::Header,
        "docker" => KeyStyle::Docker,
        other => {
            return Err(format!(
                "unknown keys '{other}': expected one of snake, header, docker"
            ))
        }
    };
    if input.trim().is_empty() {
        return Err("input is empty — paste the output of docker ps, docker images or docker stats, including the header line".into());
    }

    let mut lines = input.lines().skip_while(|l| l.trim().is_empty());
    let header_line = lines.next().unwrap_or("");
    let tabbed = header_line.contains('\t');

    // Column titles + (fixed-width only) their character offsets.
    let (starts, labels): (Vec<usize>, Vec<String>) = if tabbed {
        let labels: Vec<String> = header_line
            .split('\t')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        ((0..labels.len()).collect(), labels)
    } else {
        let starts = column_starts(header_line);
        let labels = slice_row(header_line, &starts);
        (starts, labels)
    };

    if labels.is_empty() || !looks_like_header(&labels) {
        return Err(format!(
            "no docker header row found — the first line must be the column titles, e.g. 'CONTAINER ID   IMAGE   COMMAND   CREATED   STATUS   PORTS   NAMES' (got '{}')",
            truncate(header_line, 60)
        ));
    }

    let detected = detect_kind(&labels);
    if kind != "auto" && detected != kind && detected != "custom" && strict {
        return Err(format!(
            "strict: the header line looks like docker {detected} output, not docker {kind} — set kind to auto or {detected}"
        ));
    }
    let effective_kind = if kind == "auto" { detected } else { kind };

    let cap = if limit == 0 {
        DEFAULT_LIMIT
    } else {
        limit.min(MAX_LIMIT)
    } as usize;

    // Build one row of (canonical snake key, header label, cell) per data line.
    let mut rows: Vec<Vec<(String, Option<String>, Cell)>> = Vec::new();
    let mut total = 0usize;
    for (n, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let raw: Vec<String> = if tabbed {
            let mut v: Vec<String> = line.split('\t').map(|s| s.trim().to_string()).collect();
            if strict && v.len() != labels.len() {
                return Err(format!(
                    "strict: row {} has {} tab-separated fields but the header has {}",
                    n + 2,
                    v.len(),
                    labels.len()
                ));
            }
            v.resize(labels.len(), String::new());
            v.truncate(labels.len());
            v
        } else {
            if strict && line.chars().count() <= *starts.last().unwrap_or(&0) {
                return Err(format!(
                    "strict: row {} is truncated — it has no value for the last column '{}'",
                    n + 2,
                    labels.last().cloned().unwrap_or_default()
                ));
            }
            slice_row(line, &starts)
        };
        total += 1;
        if rows.len() < cap {
            rows.push(build_row(&labels, &raw, effective_kind, parse_values));
        }
    }

    if total == 0 {
        return Err("the header row was found but there are no data rows — paste the full command output (a container/image list with at least one row)".into());
    }

    // Column order comes from the first row; every row shares the same keys.
    let mut order: Vec<(String, Option<String>)> = Vec::new();
    for (snake, label, _) in &rows[0] {
        order.push((snake.clone(), label.clone()));
    }

    // Apply the optional column filter (matched loosely: case/underscore blind).
    if !columns.trim().is_empty() {
        let mut picked: Vec<(String, Option<String>)> = Vec::new();
        for want in columns.split(',') {
            let want = want.trim();
            if want.is_empty() {
                continue;
            }
            let norm = normalize(want);
            let hit = order.iter().find(|(snake, label)| {
                normalize(snake) == norm
                    || label.as_deref().map(|l| normalize(l) == norm).unwrap_or(false)
                    || normalize(&style_key(style, snake, label.as_deref(), effective_kind)) == norm
            });
            match hit {
                Some(c) => {
                    if !picked.iter().any(|p| p.0 == c.0) {
                        picked.push(c.clone());
                    }
                }
                None => {
                    let available: Vec<String> = order
                        .iter()
                        .map(|(s, l)| style_key(style, s, l.as_deref(), effective_kind))
                        .collect();
                    return Err(format!(
                        "unknown column '{want}': available columns are {}",
                        available.join(", ")
                    ));
                }
            }
        }
        if picked.is_empty() {
            return Err("columns selected no columns — list at least one column name, or leave it blank for all".into());
        }
        order = picked;
    }

    // Final key spelling + the aligned cell matrix.
    let out_keys: Vec<String> = order
        .iter()
        .map(|(s, l)| style_key(style, s, l.as_deref(), effective_kind))
        .collect();
    let matrix: Vec<Vec<Cell>> = rows
        .iter()
        .map(|row| {
            order
                .iter()
                .map(|(snake, _)| {
                    row.iter()
                        .find(|(s, _, _)| s == snake)
                        .map(|(_, _, c)| c.clone())
                        .unwrap_or(Cell::Null)
                })
                .collect()
        })
        .collect();

    Ok(match output {
        "json" => render_json(&out_keys, &matrix),
        "csv" => render_delimited(&out_keys, &matrix, ',', header),
        "tsv" => render_delimited(&out_keys, &matrix, '\t', header),
        "markdown" => render_markdown(&out_keys, &matrix, header),
        _ => render_table(&out_keys, &matrix, header),
    })
}

fn non_empty<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    let t = v.trim();
    if t.is_empty() {
        fallback
    } else {
        t
    }
}

fn truncate(s: &str, n: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= n {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(n).collect::<String>())
    }
}

/// Case/punctuation-blind key comparison (`CONTAINER ID` ≡ `container_id`).
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

// ---------------------------------------------------------------------------
// fixed-width slicing
// ---------------------------------------------------------------------------

/// Character offsets where a column begins: any non-space preceded by ≥2 spaces
/// (docker's tab writer always pads columns by at least two), or the line start.
fn column_starts(line: &str) -> Vec<usize> {
    let chars: Vec<char> = line.chars().collect();
    let mut starts = Vec::new();
    for (i, c) in chars.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let boundary = i == 0
            || (i == 1 && chars[0] == ' ')
            || (i >= 2 && chars[i - 1] == ' ' && chars[i - 2] == ' ');
        if boundary {
            starts.push(i);
        }
    }
    starts
}

/// Slice one line at the column offsets, trimming each cell.
fn slice_row(line: &str, starts: &[usize]) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::with_capacity(starts.len());
    for (i, start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(chars.len());
        let start = (*start).min(chars.len());
        let end = end.min(chars.len()).max(start);
        out.push(chars[start..end].iter().collect::<String>().trim().to_string());
    }
    out
}

/// Docker table headers are uppercase; anything lowercase is a data row.
fn looks_like_header(labels: &[String]) -> bool {
    labels.iter().any(|l| l.chars().any(|c| c.is_ascii_uppercase()))
        && labels
            .iter()
            .all(|l| !l.is_empty() && !l.chars().any(|c| c.is_ascii_lowercase()))
}

/// Guess which docker command produced this header.
fn detect_kind(labels: &[String]) -> &'static str {
    let up: Vec<String> = labels.iter().map(|l| l.to_uppercase()).collect();
    let has = |n: &str| up.iter().any(|l| l == n);
    if has("CPU %") || has("MEM USAGE / LIMIT") || has("MEM %") || has("NET I/O") || has("BLOCK I/O")
    {
        "stats"
    } else if has("REPOSITORY") || has("IMAGE ID") || has("DIGEST") {
        "images"
    } else if has("CONTAINER ID") || has("NAMES") || has("CONTAINER") {
        "ps"
    } else {
        "custom"
    }
}

// ---------------------------------------------------------------------------
// keys
// ---------------------------------------------------------------------------

/// Header label → canonical snake key. Unknown labels fall back to a generic
/// slug (`LOCAL VOLUMES` → `local_volumes`, `CPU %` → `cpu_percent`).
fn snake_key(label: &str) -> String {
    match label.to_uppercase().as_str() {
        "CONTAINER ID" => "container_id",
        "CONTAINER" => "container",
        "IMAGE" => "image",
        "IMAGE ID" => "image_id",
        "COMMAND" => "command",
        "CREATED" => "created",
        "CREATED AT" => "created_at",
        "CREATED SINCE" => "created_since",
        "RUNNING FOR" => "running_for",
        "STATUS" => "status",
        "STATE" => "state",
        "PORTS" => "ports",
        "NAMES" => "names",
        "NAME" => "name",
        "SIZE" => "size",
        "SHARED SIZE" => "shared_size",
        "UNIQUE SIZE" => "unique_size",
        "REPOSITORY" => "repository",
        "TAG" => "tag",
        "DIGEST" => "digest",
        "PLATFORM" => "platform",
        "CONTAINERS" => "containers",
        "LOCAL VOLUMES" => "local_volumes",
        "LABELS" => "labels",
        "MOUNTS" => "mounts",
        "NETWORKS" => "networks",
        "CPU %" => "cpu_percent",
        "MEM %" => "mem_percent",
        "MEM USAGE / LIMIT" => "mem_usage_limit",
        "NET I/O" => "net_io",
        "BLOCK I/O" => "block_io",
        "PIDS" => "pids",
        _ => return generic_snake(label),
    }
    .to_string()
}

fn generic_snake(label: &str) -> String {
    let lowered = label.to_lowercase().replace('%', " percent ");
    let mut out = String::new();
    let mut pending = false;
    for c in lowered.chars() {
        if c.is_ascii_alphanumeric() {
            if pending && !out.is_empty() {
                out.push('_');
            }
            pending = false;
            out.push(c);
        } else {
            pending = true;
        }
    }
    if out.is_empty() {
        "column".to_string()
    } else {
        out
    }
}

/// snake key → the `docker --format` template name, where one exists.
fn docker_key(snake: &str, kind: &str) -> Option<&'static str> {
    Some(match snake {
        "container_id" | "image_id" => "ID",
        "container" => "Container",
        "image" => "Image",
        "command" => "Command",
        "created" => match kind {
            "ps" => "RunningFor",
            "images" => "CreatedSince",
            _ => "CreatedAt",
        },
        "created_at" => "CreatedAt",
        "created_since" => "CreatedSince",
        "running_for" => "RunningFor",
        "status" => "Status",
        "state" => "State",
        "ports" => "Ports",
        "names" => "Names",
        "name" => "Name",
        "size" => "Size",
        "shared_size" => "SharedSize",
        "unique_size" => "UniqueSize",
        "repository" => "Repository",
        "tag" => "Tag",
        "digest" => "Digest",
        "platform" => "Platform",
        "containers" => "Containers",
        "local_volumes" => "LocalVolumes",
        "labels" => "Labels",
        "mounts" => "Mounts",
        "networks" => "Networks",
        "cpu_percent" => "CPUPerc",
        "mem_percent" => "MemPerc",
        "mem_usage_limit" => "MemUsage",
        "net_io" => "NetIO",
        "block_io" => "BlockIO",
        "pids" => "PIDs",
        _ => return None,
    })
}

fn camel(snake: &str) -> String {
    snake
        .split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Spell one column in the requested key style. Derived columns (no original
/// header label) get an uppercased/camel-cased form of their snake name.
fn style_key(style: KeyStyle, snake: &str, label: Option<&str>, kind: &str) -> String {
    match style {
        KeyStyle::Snake => snake.to_string(),
        KeyStyle::Header => match label {
            Some(l) => l.to_string(),
            None => snake.replace('_', " ").to_uppercase(),
        },
        KeyStyle::Docker => docker_key(snake, kind)
            .map(|s| s.to_string())
            .unwrap_or_else(|| camel(snake)),
    }
}

// ---------------------------------------------------------------------------
// value parsing
// ---------------------------------------------------------------------------

/// `133MB` / `1.09GiB` / `0B` → bytes. SI suffixes are 1000-based, `*iB`
/// suffixes 1024-based, exactly as the docker CLI prints them.
pub fn parse_bytes(value: &str) -> Option<i64> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    let split = v
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+'))
        .map(|(i, _)| i)
        .unwrap_or(v.len());
    let (num, unit) = v.split_at(split);
    let num: f64 = num.trim().parse().ok()?;
    let unit = unit.trim().to_ascii_lowercase();
    let mult: f64 = match unit.as_str() {
        "" | "b" => 1.0,
        "kb" => 1e3,
        "mb" => 1e6,
        "gb" => 1e9,
        "tb" => 1e12,
        "pb" => 1e15,
        "kib" => 1024.0,
        "mib" => 1024f64.powi(2),
        "gib" => 1024f64.powi(3),
        "tib" => 1024f64.powi(4),
        "pib" => 1024f64.powi(5),
        _ => return None,
    };
    Some((num * mult).round() as i64)
}

/// `25.63%` → 25.63.
fn parse_percent(value: &str) -> Option<f64> {
    value.trim().trim_end_matches('%').trim().parse::<f64>().ok()
}

/// docker prints `--` for a value it cannot report (a restarting container).
fn is_unavailable(v: &str) -> bool {
    let t = v.trim();
    t == "--" || t == "--/--"
}

/// Turn one raw line into typed, possibly split, cells.
fn build_row(
    labels: &[String],
    raw: &[String],
    kind: &str,
    parse_values: bool,
) -> Vec<(String, Option<String>, Cell)> {
    let mut out: Vec<(String, Option<String>, Cell)> = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        let key = snake_key(label);
        let value = raw.get(i).cloned().unwrap_or_default();
        if !parse_values {
            out.push((key, Some(label.clone()), Cell::Text(value)));
            continue;
        }
        let unavailable = is_unavailable(&value);
        match key.as_str() {
            // Composite pairs: `1.05GiB / 7.667GiB`, `1.3kB / 0B`.
            "mem_usage_limit" | "net_io" | "block_io" => {
                let (a, b) = match key.as_str() {
                    "mem_usage_limit" => ("mem_usage", "mem_limit"),
                    "net_io" => ("net_input", "net_output"),
                    _ => ("block_input", "block_output"),
                };
                let parts: Vec<&str> = value.split('/').map(|p| p.trim()).collect();
                let left = parts.first().copied().unwrap_or("");
                let right = parts.get(1).copied().unwrap_or("");
                for (name, part) in [(a, left), (b, right)] {
                    let empty = part.is_empty() || unavailable || is_unavailable(part);
                    out.push((
                        name.to_string(),
                        None,
                        if empty {
                            Cell::Null
                        } else {
                            Cell::Text(part.to_string())
                        },
                    ));
                    out.push((
                        format!("{name}_bytes"),
                        None,
                        match parse_bytes(part) {
                            Some(b) if !empty => Cell::Int(b),
                            _ => Cell::Null,
                        },
                    ));
                }
            }
            "cpu_percent" | "mem_percent" => out.push((
                key,
                Some(label.clone()),
                match parse_percent(&value) {
                    Some(p) if !unavailable => Cell::Num(p),
                    _ => Cell::Null,
                },
            )),
            "pids" | "containers" => out.push((
                key,
                Some(label.clone()),
                match value.trim().parse::<i64>() {
                    Ok(n) if !unavailable => Cell::Int(n),
                    _ => Cell::Null,
                },
            )),
            // `docker ps --size` prints `0B (virtual 133MB)`.
            "size" | "shared_size" | "unique_size" => {
                let (head, virt) = split_virtual(&value);
                out.push((key.clone(), Some(label.clone()), Cell::Text(value.clone())));
                out.push((
                    format!("{key}_bytes"),
                    None,
                    match parse_bytes(head) {
                        Some(b) => Cell::Int(b),
                        None => Cell::Null,
                    },
                ));
                if let Some(v) = virt {
                    out.push(("virtual_size".to_string(), None, Cell::Text(v.to_string())));
                    out.push((
                        "virtual_size_bytes".to_string(),
                        None,
                        match parse_bytes(v) {
                            Some(b) => Cell::Int(b),
                            None => Cell::Null,
                        },
                    ));
                }
            }
            // `0.0.0.0:8080->80/tcp, :::8080->80/tcp` → a list.
            "ports" | "names" | "mounts" | "networks" => {
                let sep = if key == "ports" { ", " } else { "," };
                let list: Vec<String> = value
                    .split(sep)
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect();
                out.push((key, Some(label.clone()), Cell::List(list)));
            }
            // docker quotes COMMAND and truncates it with an ellipsis.
            "command" => {
                let unquoted = value.trim().trim_matches('"').to_string();
                out.push((key, Some(label.clone()), Cell::Text(unquoted)));
            }
            _ => out.push((
                key,
                Some(label.clone()),
                if unavailable {
                    Cell::Null
                } else {
                    Cell::Text(value)
                },
            )),
        }
    }
    let _ = kind;
    out
}

/// `0B (virtual 133MB)` → (`0B`, Some(`133MB`)).
fn split_virtual(value: &str) -> (&str, Option<&str>) {
    match value.find("(virtual ") {
        Some(i) => {
            let head = value[..i].trim();
            let rest = &value[i + "(virtual ".len()..];
            let virt = rest.trim_end().trim_end_matches(')').trim();
            (head, Some(virt))
        }
        None => (value.trim(), None),
    }
}

// ---------------------------------------------------------------------------
// renderers
// ---------------------------------------------------------------------------

fn render_json(keys: &[String], rows: &[Vec<Cell>]) -> String {
    let arr: Vec<Value> = rows
        .iter()
        .map(|row| {
            let mut obj = Map::new();
            for (k, cell) in keys.iter().zip(row.iter()) {
                obj.insert(k.clone(), cell.to_json());
            }
            Value::Object(obj)
        })
        .collect();
    serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or_else(|_| "[]".into())
}

/// RFC-4180-style CSV (`,`) or TSV (`\t`).
fn render_delimited(keys: &[String], rows: &[Vec<Cell>], delim: char, header: bool) -> String {
    let esc = |s: &str| -> String {
        if s.contains(delim) || s.contains('"') || s.contains('\n') || s.contains('\r') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    let push = |fields: Vec<String>, out: &mut String| {
        out.push_str(
            &fields
                .iter()
                .map(|f| esc(f))
                .collect::<Vec<_>>()
                .join(&delim.to_string()),
        );
        out.push('\n');
    };
    if header {
        push(keys.to_vec(), &mut out);
    }
    for row in rows {
        push(row.iter().map(|c| c.display()).collect(), &mut out);
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn render_markdown(keys: &[String], rows: &[Vec<Cell>], header: bool) -> String {
    let cell = |s: &str| s.replace('|', "\\|");
    let widths = widths(keys, rows, &cell);
    let mut out = String::new();
    if header {
        out.push_str(&format!("| {} |\n", pad_join(keys, &widths, &cell)));
        let sep: Vec<String> = widths.iter().map(|w| "-".repeat((*w).max(3))).collect();
        out.push_str(&format!("| {} |\n", sep.join(" | ")));
    }
    for row in rows {
        let vals: Vec<String> = row.iter().map(|c| c.display()).collect();
        out.push_str(&format!("| {} |\n", pad_join(&vals, &widths, &cell)));
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Aligned plain text, the same shape docker itself prints (3-space gutter).
fn render_table(keys: &[String], rows: &[Vec<Cell>], header: bool) -> String {
    let cell = |s: &str| s.to_string();
    let widths = widths(keys, rows, &cell);
    let mut out = String::new();
    let line = |vals: &[String], widths: &[usize]| -> String {
        let mut s = String::new();
        for (i, v) in vals.iter().enumerate() {
            if i > 0 {
                s.push_str("   ");
            }
            s.push_str(v);
            if i + 1 < vals.len() {
                s.push_str(&" ".repeat(widths[i].saturating_sub(v.chars().count())));
            }
        }
        s.trim_end().to_string()
    };
    if header {
        out.push_str(&line(keys, &widths));
        out.push('\n');
    }
    for row in rows {
        let vals: Vec<String> = row.iter().map(|c| c.display()).collect();
        out.push_str(&line(&vals, &widths));
        out.push('\n');
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn widths(keys: &[String], rows: &[Vec<Cell>], cell: &dyn Fn(&str) -> String) -> Vec<usize> {
    let mut widths: Vec<usize> = keys.iter().map(|k| cell(k).chars().count()).collect();
    for row in rows {
        for (i, c) in row.iter().enumerate() {
            let w = cell(&c.display()).chars().count();
            if i < widths.len() && w > widths[i] {
                widths[i] = w;
            }
        }
    }
    widths
}

fn pad_join(vals: &[String], widths: &[usize], cell: &dyn Fn(&str) -> String) -> String {
    vals.iter()
        .enumerate()
        .map(|(i, v)| {
            let c = cell(v);
            let w = widths.get(i).copied().unwrap_or(0);
            format!("{}{}", c, " ".repeat(w.saturating_sub(c.chars().count())))
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PS: &str = concat!(
        "CONTAINER ID   IMAGE          COMMAND                  CREATED         STATUS         PORTS                    NAMES\n",
        "9f21a1b2c3d4   nginx:1.25     \"/docker-entrypoint.…\"   3 minutes ago   Up 3 minutes   0.0.0.0:8080->80/tcp     web\n",
        "0011deadbeef   postgres:16    \"docker-entrypoint.s…\"   2 hours ago     Up 2 hours                              db\n",
    );

    const IMAGES: &str = concat!(
        "REPOSITORY   TAG       IMAGE ID       CREATED        SIZE\n",
        "nginx        1.25      a1b2c3d4e5f6   2 weeks ago    187MB\n",
        "postgres     16        112233445566   3 months ago   432MB\n",
    );

    const STATS: &str = concat!(
        "CONTAINER ID   NAME   CPU %   MEM USAGE / LIMIT     MEM %     NET I/O           BLOCK I/O     PIDS\n",
        "9f21a1b2c3d4   web    0.07%   12.05MiB / 7.667GiB   0.15%     1.31kB / 0B       0B / 8.19kB   5\n",
        "0011deadbeef   db     1.25%   64.5MiB / 7.667GiB    0.82%     18.4kB / 12.2kB   4.1MB / 0B    12\n",
    );

    fn run(input: &str, kind: &str, output: &str) -> String {
        parse(input, kind, output, "snake", true, "", true, false, 0).unwrap()
    }

    // ---- happy paths ----

    #[test]
    fn ps_auto_detects_and_splits_headers_with_spaces() {
        let out = run(PS, "auto", "json");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["container_id"], "9f21a1b2c3d4");
        assert_eq!(v[0]["image"], "nginx:1.25");
        // COMMAND is quoted and contains spaces; CREATED/STATUS do too.
        assert_eq!(v[0]["command"], "/docker-entrypoint.…");
        assert_eq!(v[0]["created"], "3 minutes ago");
        assert_eq!(v[0]["status"], "Up 3 minutes");
        assert_eq!(v[0]["ports"][0], "0.0.0.0:8080->80/tcp");
        assert_eq!(v[0]["names"][0], "web");
        // An empty PORTS cell stays an empty list, and NAMES still lands.
        assert_eq!(v[1]["ports"], serde_json::json!([]));
        assert_eq!(v[1]["names"][0], "db");
        assert_eq!(v[1]["status"], "Up 2 hours");
    }

    #[test]
    fn ps_to_csv_is_exact() {
        let out = parse(PS, "ps", "csv", "snake", false, "container_id,image,names", true, false, 0)
            .unwrap();
        assert_eq!(
            out,
            "container_id,image,names\n\
             9f21a1b2c3d4,nginx:1.25,web\n\
             0011deadbeef,postgres:16,db"
        );
    }

    #[test]
    fn images_size_gains_a_byte_count() {
        let out = run(IMAGES, "auto", "json");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["repository"], "nginx");
        assert_eq!(v[0]["tag"], "1.25");
        assert_eq!(v[0]["image_id"], "a1b2c3d4e5f6");
        assert_eq!(v[0]["size"], "187MB");
        assert_eq!(v[0]["size_bytes"], 187_000_000i64);
        assert_eq!(v[1]["size_bytes"], 432_000_000i64);
    }

    #[test]
    fn stats_splits_composite_columns_and_types_numbers() {
        let out = run(STATS, "auto", "json");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["name"], "web");
        assert_eq!(v[0]["cpu_percent"], 0.07);
        assert_eq!(v[0]["mem_usage"], "12.05MiB");
        assert_eq!(v[0]["mem_limit"], "7.667GiB");
        assert_eq!(v[0]["mem_usage_bytes"], 12_635_341i64);
        assert_eq!(v[0]["mem_percent"], 0.15);
        assert_eq!(v[0]["net_input"], "1.31kB");
        assert_eq!(v[0]["net_output"], "0B");
        assert_eq!(v[0]["net_input_bytes"], 1310i64);
        assert_eq!(v[0]["net_output_bytes"], 0i64);
        assert_eq!(v[0]["block_output_bytes"], 8190i64);
        assert_eq!(v[0]["pids"], 5i64);
        assert_eq!(v[1]["block_input_bytes"], 4_100_000i64);
    }

    #[test]
    fn stats_unavailable_values_become_null() {
        let input = concat!(
            "CONTAINER ID   NAME   CPU %   MEM USAGE / LIMIT   MEM %   NET I/O   BLOCK I/O   PIDS\n",
            "9f21a1b2c3d4   web    --      --                  --      --        --          --\n",
        );
        let v: Value = serde_json::from_str(&run(input, "stats", "json")).unwrap();
        assert!(v[0]["cpu_percent"].is_null());
        assert!(v[0]["mem_usage"].is_null());
        assert!(v[0]["mem_limit_bytes"].is_null());
        assert!(v[0]["pids"].is_null());
    }

    #[test]
    fn ps_size_column_splits_the_virtual_size() {
        let input = concat!(
            "CONTAINER ID   IMAGE        COMMAND   CREATED       STATUS       PORTS   NAMES   SIZE\n",
            "9f21a1b2c3d4   nginx:1.25   \"nginx\"   2 hours ago   Up 2 hours           web     1.09kB (virtual 187MB)\n",
        );
        let v: Value = serde_json::from_str(&run(input, "ps", "json")).unwrap();
        assert_eq!(v[0]["size"], "1.09kB (virtual 187MB)");
        assert_eq!(v[0]["size_bytes"], 1090i64);
        assert_eq!(v[0]["virtual_size"], "187MB");
        assert_eq!(v[0]["virtual_size_bytes"], 187_000_000i64);
    }

    #[test]
    fn key_styles_rename_columns() {
        let header = parse(STATS, "auto", "json", "header", false, "", true, false, 0).unwrap();
        let v: Value = serde_json::from_str(&header).unwrap();
        assert_eq!(v[0]["MEM USAGE / LIMIT"], "12.05MiB / 7.667GiB");
        assert_eq!(v[0]["CPU %"], "0.07%");

        let docker = parse(STATS, "auto", "json", "docker", false, "", true, false, 0).unwrap();
        let v: Value = serde_json::from_str(&docker).unwrap();
        assert_eq!(v[0]["CPUPerc"], "0.07%");
        assert_eq!(v[0]["MemUsage"], "12.05MiB / 7.667GiB");
        assert_eq!(v[0]["NetIO"], "1.31kB / 0B");
        assert_eq!(v[0]["PIDs"], "5");

        // docker ps' CREATED column is the .RunningFor template field.
        let ps = parse(PS, "ps", "json", "docker", false, "", true, false, 0).unwrap();
        let v: Value = serde_json::from_str(&ps).unwrap();
        assert_eq!(v[0]["RunningFor"], "3 minutes ago");
        assert_eq!(v[0]["ID"], "9f21a1b2c3d4");
    }

    #[test]
    fn raw_mode_keeps_every_value_as_text() {
        let out = parse(STATS, "stats", "csv", "snake", false, "", true, false, 0).unwrap();
        let first = out.lines().next().unwrap();
        assert_eq!(
            first,
            "container_id,name,cpu_percent,mem_usage_limit,mem_percent,net_io,block_io,pids"
        );
        assert!(out.contains("12.05MiB / 7.667GiB"), "{out}");
        assert!(out.contains("1.31kB / 0B"), "{out}");
    }

    #[test]
    fn markdown_and_table_render_aligned() {
        let md = parse(IMAGES, "images", "markdown", "snake", false, "repository,tag", true, false, 0)
            .unwrap();
        assert_eq!(
            md,
            "| repository | tag  |\n\
             | ---------- | ---- |\n\
             | nginx      | 1.25 |\n\
             | postgres   | 16   |"
        );

        let tbl = parse(IMAGES, "images", "table", "header", false, "REPOSITORY,SIZE", true, false, 0)
            .unwrap();
        assert_eq!(
            tbl,
            "REPOSITORY   SIZE\n\
             nginx        187MB\n\
             postgres     432MB"
        );
    }

    #[test]
    fn header_row_can_be_suppressed() {
        let out = parse(IMAGES, "images", "csv", "snake", false, "repository", false, false, 0)
            .unwrap();
        assert_eq!(out, "nginx\npostgres");
    }

    #[test]
    fn tab_separated_output_is_supported() {
        let input = "CONTAINER ID\tNAMES\tSTATUS\n9f21a1b2c3d4\tweb\tUp 3 minutes\n";
        let v: Value = serde_json::from_str(&run(input, "auto", "json")).unwrap();
        assert_eq!(v[0]["container_id"], "9f21a1b2c3d4");
        assert_eq!(v[0]["status"], "Up 3 minutes");
    }

    #[test]
    fn custom_format_headers_still_parse() {
        let input = "NAME    LOCAL VOLUMES   STATE\nweb     2               running\n";
        let v: Value = serde_json::from_str(&run(input, "auto", "json")).unwrap();
        assert_eq!(v[0]["name"], "web");
        assert_eq!(v[0]["local_volumes"], "2");
        assert_eq!(v[0]["state"], "running");
    }

    #[test]
    fn limit_caps_rows() {
        let out = parse(IMAGES, "images", "csv", "snake", false, "repository", false, false, 1)
            .unwrap();
        assert_eq!(out, "nginx");
    }

    #[test]
    fn columns_reorder_and_accept_any_spelling() {
        let out = parse(PS, "ps", "csv", "snake", false, "NAMES, container id", true, false, 0)
            .unwrap();
        assert_eq!(out, "names,container_id\nweb,9f21a1b2c3d4\ndb,0011deadbeef");
    }

    #[test]
    fn byte_parsing_covers_si_and_binary_suffixes() {
        assert_eq!(parse_bytes("0B"), Some(0));
        assert_eq!(parse_bytes("1.09kB"), Some(1090));
        assert_eq!(parse_bytes("187MB"), Some(187_000_000));
        assert_eq!(parse_bytes("1.5GB"), Some(1_500_000_000));
        assert_eq!(parse_bytes("1KiB"), Some(1024));
        assert_eq!(parse_bytes("7.667GiB"), Some(8_232_378_565));
        assert_eq!(parse_bytes("nonsense"), None);
    }

    // ---- error paths ----

    #[test]
    fn error_on_empty_input() {
        let err = parse("   ", "auto", "json", "snake", true, "", true, false, 0).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn error_when_the_header_line_is_missing() {
        let err = parse(
            "9f21a1b2c3d4   nginx:1.25   web\n",
            "ps",
            "json",
            "snake",
            true,
            "",
            true,
            false,
            0,
        )
        .unwrap_err();
        assert!(err.contains("no docker header row found"), "{err}");
    }

    #[test]
    fn error_when_there_are_no_data_rows() {
        let err = parse(
            "CONTAINER ID   IMAGE   NAMES\n",
            "ps",
            "json",
            "snake",
            true,
            "",
            true,
            false,
            0,
        )
        .unwrap_err();
        assert!(err.contains("no data rows"), "{err}");
    }

    #[test]
    fn error_on_unknown_kind_output_and_keys() {
        let e1 = parse(PS, "swarm", "json", "snake", true, "", true, false, 0).unwrap_err();
        assert!(e1.contains("unknown kind 'swarm'"), "{e1}");
        let e2 = parse(PS, "ps", "yaml", "snake", true, "", true, false, 0).unwrap_err();
        assert!(e2.contains("unknown output 'yaml'"), "{e2}");
        let e3 = parse(PS, "ps", "json", "camel", true, "", true, false, 0).unwrap_err();
        assert!(e3.contains("unknown keys 'camel'"), "{e3}");
    }

    #[test]
    fn error_on_unknown_column() {
        let err = parse(PS, "ps", "csv", "snake", false, "nope", true, false, 0).unwrap_err();
        assert!(err.contains("unknown column 'nope'"), "{err}");
        assert!(err.contains("container_id"), "{err}");
    }

    #[test]
    fn strict_rejects_a_mismatched_kind() {
        let err = parse(IMAGES, "stats", "json", "snake", true, "", true, true, 0).unwrap_err();
        assert!(err.contains("looks like docker images output"), "{err}");
        // Lenient mode parses it anyway.
        assert!(parse(IMAGES, "stats", "json", "snake", true, "", true, false, 0).is_ok());
    }

    #[test]
    fn strict_rejects_a_truncated_row() {
        let input = concat!(
            "REPOSITORY   TAG    IMAGE ID       CREATED       SIZE\n",
            "nginx        1.25   a1b2c3d4e5f6\n",
        );
        let err = parse(input, "images", "json", "snake", true, "", true, true, 0).unwrap_err();
        assert!(err.contains("row 2 is truncated"), "{err}");
        // Lenient mode fills the missing cells instead.
        let v: Value =
            serde_json::from_str(&parse(input, "images", "json", "snake", false, "", true, false, 0).unwrap())
                .unwrap();
        assert_eq!(v[0]["size"], "");
    }
}
