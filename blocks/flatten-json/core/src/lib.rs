//! flatten-json core — pure compute, shared by the chat skill block, the CLI,
//! and the web page. No wafer/wasm-bindgen deps and no host/WASI calls, so it
//! runs in the gizza wafer runtime, the CLI, and the browser alike.
//!
//! Flattens a nested JSON document into a single-level map of path -> value and
//! rebuilds ("unflattens") that map back into the nested document.
//!
//! Path notation (lodash / `dot-object` / `flat` style, NOT RFC 9535 JSONPath):
//!   - object keys are joined with `separator` (default `.`): `user.address.city`
//!   - array elements use `array_notation`:
//!       * `bracket`   (default) → `items[0].sku` — unambiguous, because a bare
//!         digit segment can then only mean an object key literally named "0"
//!       * `separator`           → `items.0.sku` — an all-digit segment rebuilds
//!         an array on the way back
//!
//! Both directions round-trip: `unflatten(flatten(doc)) == doc` for every option
//! combination that isn't explicitly lossy (`key_case`, `max_depth` with
//! `preserve_empty = false`).

use serde_json::{Map, Value};

/// Largest source document accepted, in bytes.
pub const MAX_JSON_BYTES: usize = 5_000_000;
/// Largest number of flattened keys produced (or consumed) before giving up.
pub const MAX_KEYS: usize = 200_000;
/// Largest nesting depth walked before giving up.
pub const MAX_DEPTH: usize = 100;
/// Largest array index accepted while unflattening (guards against a single
/// `a[999999999]` key allocating a huge array).
pub const MAX_ARRAY_INDEX: usize = 100_000;
/// Largest separator accepted, in characters.
pub const MAX_SEPARATOR_CHARS: usize = 8;
/// Largest indent accepted, in spaces.
pub const MAX_INDENT: usize = 8;

// --------------------------------------------------------------- options ----

/// Which way to convert.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    /// Nested document → flat path map.
    Flatten,
    /// Flat path map → nested document.
    Unflatten,
    /// Decide from the input's shape.
    Auto,
}

/// How array elements appear in a path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArrayNotation {
    /// `items[0]`
    Bracket,
    /// `items.0`
    Separator,
}

/// Case transform applied to the generated path (flatten only).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyCase {
    Preserve,
    Upper,
    Lower,
}

/// Shape of the flattened result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    /// A JSON object of path -> value.
    Json,
    /// `path=value` lines.
    Pairs,
    /// A two-column `key,value` CSV with a header row.
    Csv,
    /// The paths only, one per line.
    Paths,
}

/// Everything the conversion needs beyond the document itself.
#[derive(Clone, Debug)]
pub struct Options {
    pub direction: Direction,
    pub separator: String,
    pub array_notation: ArrayNotation,
    /// Levels to flatten; 0 = unlimited. Containers at the cap are emitted as
    /// nested JSON values.
    pub max_depth: usize,
    /// When false, arrays are kept whole as JSON values instead of indexed.
    pub flatten_arrays: bool,
    /// When true, an empty object/array is emitted as a `{}` / `[]` leaf so the
    /// key survives the round-trip; when false the key is dropped.
    pub preserve_empty: bool,
    pub key_case: KeyCase,
    pub output: OutputFormat,
    pub pretty: bool,
    pub indent: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            direction: Direction::Flatten,
            separator: ".".to_string(),
            array_notation: ArrayNotation::Bracket,
            max_depth: 0,
            flatten_arrays: true,
            preserve_empty: true,
            key_case: KeyCase::Preserve,
            output: OutputFormat::Json,
            pretty: true,
            indent: 2,
        }
    }
}

fn parse_direction(s: &str) -> Result<Direction, String> {
    match s.trim() {
        "" | "flatten" => Ok(Direction::Flatten),
        "unflatten" => Ok(Direction::Unflatten),
        "auto" => Ok(Direction::Auto),
        other => Err(format!(
            "direction must be 'flatten', 'unflatten' or 'auto', got '{other}'"
        )),
    }
}

fn parse_array_notation(s: &str) -> Result<ArrayNotation, String> {
    match s.trim() {
        "" | "bracket" => Ok(ArrayNotation::Bracket),
        "separator" => Ok(ArrayNotation::Separator),
        other => Err(format!(
            "array_notation must be 'bracket' (items[0]) or 'separator' (items.0), got '{other}'"
        )),
    }
}

fn parse_key_case(s: &str) -> Result<KeyCase, String> {
    match s.trim() {
        "" | "preserve" => Ok(KeyCase::Preserve),
        "upper" => Ok(KeyCase::Upper),
        "lower" => Ok(KeyCase::Lower),
        other => Err(format!(
            "key_case must be 'preserve', 'upper' or 'lower', got '{other}'"
        )),
    }
}

fn parse_output(s: &str) -> Result<OutputFormat, String> {
    match s.trim() {
        "" | "json" => Ok(OutputFormat::Json),
        "pairs" => Ok(OutputFormat::Pairs),
        "csv" => Ok(OutputFormat::Csv),
        "paths" => Ok(OutputFormat::Paths),
        other => Err(format!(
            "output must be 'json', 'pairs', 'csv' or 'paths', got '{other}'"
        )),
    }
}

/// String-argument entry point used by the block, the CLI, and the web export.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    json: &str,
    direction: &str,
    separator: &str,
    array_notation: &str,
    max_depth: usize,
    flatten_arrays: bool,
    preserve_empty: bool,
    key_case: &str,
    output: &str,
    pretty: bool,
    indent: usize,
) -> Result<String, String> {
    let opts = Options {
        direction: parse_direction(direction)?,
        separator: if separator.is_empty() {
            ".".to_string()
        } else {
            separator.to_string()
        },
        array_notation: parse_array_notation(array_notation)?,
        max_depth,
        flatten_arrays,
        preserve_empty,
        key_case: parse_key_case(key_case)?,
        output: parse_output(output)?,
        pretty,
        indent,
    };
    run(json, &opts)
}

// ------------------------------------------------------------- top level ----

/// Convert `json` according to `opts`.
pub fn run(json: &str, opts: &Options) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("no JSON provided — paste a JSON object or array to convert".to_string());
    }
    if json.len() > MAX_JSON_BYTES {
        return Err(format!(
            "input is {} bytes, over the {} byte ({} MB) limit",
            json.len(),
            MAX_JSON_BYTES,
            MAX_JSON_BYTES / 1_000_000
        ));
    }
    let sep_chars = opts.separator.chars().count();
    if sep_chars == 0 || sep_chars > MAX_SEPARATOR_CHARS {
        return Err(format!(
            "separator must be 1-{MAX_SEPARATOR_CHARS} characters, got {sep_chars}"
        ));
    }
    if opts.separator.contains('[') || opts.separator.contains(']') {
        return Err(format!(
            "separator must not contain '[' or ']' (they delimit array indices), got '{}'",
            opts.separator
        ));
    }
    if opts.max_depth > MAX_DEPTH {
        return Err(format!(
            "max_depth must be 0 (unlimited) to {MAX_DEPTH}, got {}",
            opts.max_depth
        ));
    }
    if opts.indent > MAX_INDENT {
        return Err(format!(
            "indent must be 0-{MAX_INDENT} spaces, got {}",
            opts.indent
        ));
    }

    let value: Value = serde_json::from_str(json).map_err(|e| {
        format!(
            "invalid JSON at line {}, column {}: {}",
            e.line(),
            e.column(),
            e
        )
    })?;

    let direction = match opts.direction {
        Direction::Flatten => Direction::Flatten,
        Direction::Unflatten => Direction::Unflatten,
        Direction::Auto => detect(&value, opts),
    };

    match direction {
        Direction::Unflatten => {
            if opts.output != OutputFormat::Json {
                return Err(format!(
                    "output='{}' only applies when flattening — unflatten always returns nested JSON, so set output=json",
                    format_name(opts.output)
                ));
            }
            let nested = unflatten(&value, opts)?;
            Ok(serialize(&nested, opts.pretty, opts.indent))
        }
        _ => {
            let flat = flatten(&value, opts)?;
            Ok(render(&flat, opts))
        }
    }
}

fn format_name(f: OutputFormat) -> &'static str {
    match f {
        OutputFormat::Json => "json",
        OutputFormat::Pairs => "pairs",
        OutputFormat::Csv => "csv",
        OutputFormat::Paths => "paths",
    }
}

/// Auto-detect: only a one-level object whose keys already look like paths is
/// treated as flat input. Anything else (nested values, no path-shaped key) is
/// flattened, which is the safe no-op for an already-flat document.
fn detect(value: &Value, opts: &Options) -> Direction {
    let Value::Object(map) = value else {
        return Direction::Flatten;
    };
    let all_leaves = map.values().all(|v| match v {
        Value::Object(m) => m.is_empty(),
        Value::Array(a) => a.is_empty(),
        _ => true,
    });
    if !all_leaves {
        return Direction::Flatten;
    }
    let path_shaped = map
        .keys()
        .any(|k| k.contains(opts.separator.as_str()) || has_bracket_index(k));
    if path_shaped {
        Direction::Unflatten
    } else {
        Direction::Flatten
    }
}

/// True when the key carries at least one `[<digits>]` group.
fn has_bracket_index(key: &str) -> bool {
    let bytes = key.as_bytes();
    let mut i = 0;
    while let Some(open) = bytes[i..].iter().position(|&b| b == b'[') {
        let start = i + open + 1;
        if let Some(close) = bytes[start..].iter().position(|&b| b == b']') {
            let inner = &key[start..start + close];
            if !inner.is_empty() && inner.bytes().all(|b| b.is_ascii_digit()) {
                return true;
            }
            i = start + close + 1;
        } else {
            return false;
        }
        if i >= bytes.len() {
            break;
        }
    }
    false
}

// --------------------------------------------------------------- flatten ----

/// Flatten `value` into an ordered path -> value map.
pub fn flatten(value: &Value, opts: &Options) -> Result<Map<String, Value>, String> {
    match value {
        Value::Object(_) | Value::Array(_) => {}
        other => {
            return Err(format!(
                "expected a JSON object or array to flatten, got {}",
                type_name(other)
            ))
        }
    }
    let mut out = Map::new();
    walk(String::new(), value, 0, opts, &mut out)?;
    Ok(out)
}

fn walk(
    path: String,
    value: &Value,
    depth: usize,
    opts: &Options,
    out: &mut Map<String, Value>,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "document nests deeper than {MAX_DEPTH} levels at '{path}'"
        ));
    }
    // At the depth cap, containers are emitted whole rather than expanded.
    let capped = opts.max_depth > 0 && depth >= opts.max_depth;
    match value {
        Value::Object(map) if !capped => {
            if map.is_empty() {
                if opts.preserve_empty {
                    emit(path, Value::Object(Map::new()), opts, out)?;
                }
                return Ok(());
            }
            for (key, child) in map {
                walk(join(&path, key, opts), child, depth + 1, opts, out)?;
            }
            Ok(())
        }
        Value::Array(items) if !capped && opts.flatten_arrays => {
            if items.is_empty() {
                if opts.preserve_empty {
                    emit(path, Value::Array(Vec::new()), opts, out)?;
                }
                return Ok(());
            }
            for (i, child) in items.iter().enumerate() {
                walk(join_index(&path, i, opts), child, depth + 1, opts, out)?;
            }
            Ok(())
        }
        other => {
            debug_assert!(
                !path.is_empty(),
                "the root container is never emitted as a leaf"
            );
            emit(path, other.clone(), opts, out)
        }
    }
}

fn emit(
    path: String,
    value: Value,
    opts: &Options,
    out: &mut Map<String, Value>,
) -> Result<(), String> {
    let key = match opts.key_case {
        KeyCase::Preserve => path,
        KeyCase::Upper => path.to_uppercase(),
        KeyCase::Lower => path.to_lowercase(),
    };
    if out.contains_key(&key) {
        return Err(format!(
            "two different paths flatten to the same key '{key}' — a source key already contains \
             the separator '{}' (or key_case merged them); pick another separator or \
             key_case=preserve",
            opts.separator
        ));
    }
    if out.len() >= MAX_KEYS {
        return Err(format!("document flattens to more than {MAX_KEYS} keys"));
    }
    out.insert(key, value);
    Ok(())
}

fn join(prefix: &str, key: &str, opts: &Options) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}{}{key}", opts.separator)
    }
}

fn join_index(prefix: &str, index: usize, opts: &Options) -> String {
    match opts.array_notation {
        ArrayNotation::Bracket => format!("{prefix}[{index}]"),
        ArrayNotation::Separator => join(prefix, &index.to_string(), opts),
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

// ------------------------------------------------------------- unflatten ----

/// One step of a parsed path.
#[derive(Debug, PartialEq, Eq)]
enum Segment {
    Key(String),
    Index(usize),
}

/// Rebuild the nested document from a flat path -> value object.
pub fn unflatten(value: &Value, opts: &Options) -> Result<Value, String> {
    let Value::Object(map) = value else {
        return Err(format!(
            "unflatten expects a flat JSON object of path -> value, got {}",
            type_name(value)
        ));
    };
    if map.len() > MAX_KEYS {
        return Err(format!("input has more than {MAX_KEYS} keys"));
    }
    let mut root = Value::Null;
    for (key, leaf) in map {
        let segments = parse_path(key, opts)?;
        insert(&mut root, &segments, 0, leaf.clone(), key)?;
    }
    if root.is_null() {
        // An empty `{}` input rebuilds to an empty object, not null.
        root = Value::Object(Map::new());
    }
    Ok(root)
}

/// Split a flattened key into segments: separator-joined pieces, each of which
/// may carry trailing `[<digits>]` groups.
fn parse_path(key: &str, opts: &Options) -> Result<Vec<Segment>, String> {
    if key.is_empty() {
        return Err("empty key: every key must be a path like 'user.name'".to_string());
    }
    let mut segments = Vec::new();
    for piece in key.split(opts.separator.as_str()) {
        push_piece(piece, key, opts, &mut segments)?;
    }
    if segments.is_empty() {
        return Err(format!("key '{key}' has no path segments"));
    }
    if segments.len() > MAX_DEPTH {
        return Err(format!(
            "key '{key}' has more than {MAX_DEPTH} path segments"
        ));
    }
    Ok(segments)
}

fn push_piece(
    piece: &str,
    key: &str,
    opts: &Options,
    out: &mut Vec<Segment>,
) -> Result<(), String> {
    let (name, rest) = match piece.find('[') {
        Some(i) => (&piece[..i], &piece[i..]),
        None => (piece, ""),
    };
    if !name.is_empty() {
        out.push(name_segment(name, opts));
    } else if rest.is_empty() {
        return Err(format!(
            "key '{key}' has an empty segment — two separators ('{}') in a row, or a leading/\
             trailing separator",
            opts.separator
        ));
    }
    let mut tail = rest;
    while !tail.is_empty() {
        if !tail.starts_with('[') {
            return Err(format!(
                "key '{key}': expected '[' after an array index, got '{tail}'"
            ));
        }
        let close = tail.find(']').ok_or_else(|| {
            format!("key '{key}': array index '{tail}' is missing its closing ']'")
        })?;
        let inner = &tail[1..close];
        if inner.is_empty() || !inner.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!(
                "key '{key}': array index '[{inner}]' must be digits, e.g. '[0]'"
            ));
        }
        let index: usize = inner
            .parse()
            .map_err(|_| format!("key '{key}': array index '[{inner}]' is too large"))?;
        if index > MAX_ARRAY_INDEX {
            return Err(format!(
                "key '{key}': array index {index} is over the {MAX_ARRAY_INDEX} limit"
            ));
        }
        out.push(Segment::Index(index));
        tail = &tail[close + 1..];
    }
    Ok(())
}

/// With `array_notation = separator`, an all-digit segment rebuilds an array;
/// with `bracket` it stays an object key, because bracket paths spell array
/// indices out explicitly.
fn name_segment(name: &str, opts: &Options) -> Segment {
    if opts.array_notation == ArrayNotation::Separator
        && !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_digit())
    {
        if let Ok(i) = name.parse::<usize>() {
            if i <= MAX_ARRAY_INDEX {
                return Segment::Index(i);
            }
        }
    }
    Segment::Key(name.to_string())
}

fn insert(
    node: &mut Value,
    segments: &[Segment],
    at: usize,
    leaf: Value,
    key: &str,
) -> Result<(), String> {
    let segment = &segments[at];
    let last = at + 1 == segments.len();

    // Materialize the container this segment needs.
    match segment {
        Segment::Key(_) => {
            if node.is_null() {
                *node = Value::Object(Map::new());
            }
            if !node.is_object() {
                return Err(conflict(key, segments, at, node));
            }
        }
        Segment::Index(_) => {
            if node.is_null() {
                *node = Value::Array(Vec::new());
            }
            if !node.is_array() {
                return Err(conflict(key, segments, at, node));
            }
        }
    }

    match segment {
        Segment::Key(name) => {
            let map = node.as_object_mut().expect("checked above");
            if last {
                if map.contains_key(name) {
                    return Err(format!(
                        "conflicting paths: key '{key}' would overwrite the value already built \
                         at '{}'",
                        render_path(segments)
                    ));
                }
                map.insert(name.clone(), leaf);
                return Ok(());
            }
            let child = map.entry(name.clone()).or_insert(Value::Null);
            insert(child, segments, at + 1, leaf, key)
        }
        Segment::Index(i) => {
            let items = node.as_array_mut().expect("checked above");
            if *i >= items.len() {
                if *i > MAX_ARRAY_INDEX {
                    return Err(format!(
                        "key '{key}': array index {i} is over the {MAX_ARRAY_INDEX} limit"
                    ));
                }
                items.resize(*i + 1, Value::Null);
            }
            if last {
                if !items[*i].is_null() {
                    return Err(format!(
                        "conflicting paths: key '{key}' would overwrite the value already built \
                         at '{}'",
                        render_path(segments)
                    ));
                }
                items[*i] = leaf;
                return Ok(());
            }
            insert(&mut items[*i], segments, at + 1, leaf, key)
        }
    }
}

fn conflict(key: &str, segments: &[Segment], at: usize, existing: &Value) -> String {
    let prefix = render_path(&segments[..at]);
    let wanted = match &segments[at] {
        Segment::Key(_) => "an object",
        Segment::Index(_) => "an array",
    };
    format!(
        "conflicting paths: '{prefix}' is already {} but key '{key}' needs it to be {wanted}",
        type_name(existing)
    )
}

fn render_path(segments: &[Segment]) -> String {
    let mut out = String::new();
    for seg in segments {
        match seg {
            Segment::Key(k) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(k);
            }
            Segment::Index(i) => out.push_str(&format!("[{i}]")),
        }
    }
    if out.is_empty() {
        "(root)".to_string()
    } else {
        out
    }
}

// ---------------------------------------------------------------- render ----

fn render(flat: &Map<String, Value>, opts: &Options) -> String {
    match opts.output {
        OutputFormat::Json => serialize(&Value::Object(flat.clone()), opts.pretty, opts.indent),
        OutputFormat::Paths => flat.keys().cloned().collect::<Vec<_>>().join("\n"),
        OutputFormat::Pairs => flat
            .iter()
            .map(|(k, v)| format!("{k}={}", scalar_text(v)))
            .collect::<Vec<_>>()
            .join("\n"),
        OutputFormat::Csv => {
            let mut lines = vec!["key,value".to_string()];
            for (k, v) in flat {
                lines.push(format!("{},{}", csv_field(k), csv_field(&scalar_text(v))));
            }
            lines.join("\n")
        }
    }
}

/// Strings print bare (so `db.host=localhost`, not `db.host="localhost"`);
/// every other value prints as compact JSON.
fn scalar_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn csv_field(s: &str) -> String {
    let needs_quotes = s.contains(',')
        || s.contains('"')
        || s.contains('\n')
        || s.contains('\r')
        || s.starts_with(' ')
        || s.ends_with(' ');
    if needs_quotes {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn serialize(value: &Value, pretty: bool, indent: usize) -> String {
    if !pretty || indent == 0 {
        return value.to_string();
    }
    let spaces = " ".repeat(indent);
    let formatter = serde_json::ser::PrettyFormatter::with_indent(spaces.as_bytes());
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    match serde::Serialize::serialize(value, &mut ser) {
        Ok(()) => String::from_utf8(buf).unwrap_or_else(|_| value.to_string()),
        Err(_) => value.to_string(),
    }
}

// ----------------------------------------------------------------- tests ----

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn flattens_nested_objects_and_arrays() {
        let out = run(
            r#"{"user":{"name":"Ada","tags":["a","b"]},"ok":true}"#,
            &Options {
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"user.name":"Ada","user.tags[0]":"a","user.tags[1]":"b","ok":true}"#
        );
    }

    #[test]
    fn separator_notation_indexes_arrays_with_the_separator() {
        let out = run(
            r#"{"a":[1,2]}"#,
            &Options {
                array_notation: ArrayNotation::Separator,
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, r#"{"a.0":1,"a.1":2}"#);
    }

    #[test]
    fn custom_separator_is_used() {
        let out = run(
            r#"{"db":{"host":"local"}}"#,
            &Options {
                separator: "_".into(),
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, r#"{"db_host":"local"}"#);
    }

    #[test]
    fn empty_containers_are_preserved_by_default_and_droppable() {
        let kept = run(
            r#"{"a":{},"b":[],"c":1}"#,
            &Options {
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(kept, r#"{"a":{},"b":[],"c":1}"#);
        let dropped = run(
            r#"{"a":{},"b":[],"c":1}"#,
            &Options {
                preserve_empty: false,
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(dropped, r#"{"c":1}"#);
    }

    #[test]
    fn flatten_arrays_false_keeps_arrays_whole() {
        let out = run(
            r#"{"a":{"list":[1,{"x":2}]}}"#,
            &Options {
                flatten_arrays: false,
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, r#"{"a.list":[1,{"x":2}]}"#);
    }

    #[test]
    fn max_depth_leaves_deeper_values_nested() {
        let out = run(
            r#"{"a":{"b":{"c":1}}}"#,
            &Options {
                max_depth: 2,
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, r#"{"a.b":{"c":1}}"#);
    }

    #[test]
    fn key_case_upper_matches_env_style_keys() {
        let out = run(
            r#"{"db":{"host":"local"}}"#,
            &Options {
                separator: "_".into(),
                key_case: KeyCase::Upper,
                output: OutputFormat::Pairs,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, "DB_HOST=local");
    }

    #[test]
    fn csv_output_quotes_commas_and_quotes() {
        let out = run(
            r#"{"a":"x,y","b":"say \"hi\"","n":3}"#,
            &Options {
                output: OutputFormat::Csv,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, "key,value\na,\"x,y\"\nb,\"say \"\"hi\"\"\"\nn,3");
    }

    #[test]
    fn paths_output_lists_paths_only() {
        let out = run(
            r#"{"a":{"b":1},"c":[7]}"#,
            &Options {
                output: OutputFormat::Paths,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, "a.b\nc[0]");
    }

    #[test]
    fn unflatten_rebuilds_objects_and_arrays() {
        let out = run(
            r#"{"user.name":"Ada","user.tags[0]":"a","user.tags[1]":"b"}"#,
            &Options {
                direction: Direction::Unflatten,
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, r#"{"user":{"name":"Ada","tags":["a","b"]}}"#);
    }

    #[test]
    fn unflatten_separator_notation_digits_become_arrays() {
        let out = run(
            r#"{"a.0":1,"a.1":2}"#,
            &Options {
                direction: Direction::Unflatten,
                array_notation: ArrayNotation::Separator,
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, r#"{"a":[1,2]}"#);
    }

    #[test]
    fn unflatten_bracket_notation_keeps_digit_keys_as_object_keys() {
        let out = run(
            r#"{"a.0":1}"#,
            &Options {
                direction: Direction::Unflatten,
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, r#"{"a":{"0":1}}"#);
    }

    #[test]
    fn round_trip_is_lossless() {
        let src = r#"{"a":{"b":[1,{"c":null},[],{}],"d":"x"},"e":false}"#;
        let flat = run(
            src,
            &Options {
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        let back = run(
            &flat,
            &Options {
                direction: Direction::Unflatten,
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&back).unwrap(),
            serde_json::from_str::<Value>(src).unwrap()
        );
    }

    #[test]
    fn auto_detects_flat_input_and_nested_input() {
        let unflattened = run(
            r#"{"a.b":1}"#,
            &Options {
                direction: Direction::Auto,
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(unflattened, r#"{"a":{"b":1}}"#);
        let flattened = run(
            r#"{"a":{"b":1}}"#,
            &Options {
                direction: Direction::Auto,
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(flattened, r#"{"a.b":1}"#);
    }

    #[test]
    fn root_array_flattens_with_indexed_keys() {
        let out = run(
            r#"[{"a":1},{"a":2}]"#,
            &Options {
                pretty: false,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, r#"{"[0].a":1,"[1].a":2}"#);
    }

    #[test]
    fn pretty_indent_is_configurable() {
        let out = run(
            r#"{"a":{"b":1}}"#,
            &Options {
                indent: 4,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, "{\n    \"a.b\": 1\n}");
    }

    // ---- errors ----

    #[test]
    fn invalid_json_reports_line_and_column() {
        let err = run("{\"a\": }", &opts()).unwrap_err();
        assert!(err.starts_with("invalid JSON at line 1, column"), "{err}");
    }

    #[test]
    fn scalar_input_is_rejected() {
        let err = run("42", &opts()).unwrap_err();
        assert_eq!(
            err,
            "expected a JSON object or array to flatten, got a number"
        );
    }

    #[test]
    fn separator_collision_is_reported() {
        let err = run(r#"{"a.b":1,"a":{"b":2}}"#, &opts()).unwrap_err();
        assert!(
            err.contains("two different paths flatten to the same key 'a.b'"),
            "{err}"
        );
    }

    #[test]
    fn unflatten_path_conflict_is_reported() {
        let err = run(
            r#"{"a":1,"a.b":2}"#,
            &Options {
                direction: Direction::Unflatten,
                ..opts()
            },
        )
        .unwrap_err();
        assert_eq!(
            err,
            "conflicting paths: 'a' is already a number but key 'a.b' needs it to be an object"
        );
    }

    #[test]
    fn unflatten_rejects_non_object_input() {
        let err = run(
            "[1,2]",
            &Options {
                direction: Direction::Unflatten,
                ..opts()
            },
        )
        .unwrap_err();
        assert_eq!(
            err,
            "unflatten expects a flat JSON object of path -> value, got an array"
        );
    }

    #[test]
    fn unflatten_rejects_non_json_output_format() {
        let err = run(
            r#"{"a.b":1}"#,
            &Options {
                direction: Direction::Unflatten,
                output: OutputFormat::Csv,
                ..opts()
            },
        )
        .unwrap_err();
        assert!(
            err.starts_with("output='csv' only applies when flattening"),
            "{err}"
        );
    }

    #[test]
    fn bad_array_index_is_reported() {
        let err = run(
            r#"{"a[x]":1}"#,
            &Options {
                direction: Direction::Unflatten,
                ..opts()
            },
        )
        .unwrap_err();
        assert_eq!(
            err,
            "key 'a[x]': array index '[x]' must be digits, e.g. '[0]'"
        );
    }

    #[test]
    fn empty_segment_is_reported() {
        let err = run(
            r#"{"a..b":1}"#,
            &Options {
                direction: Direction::Unflatten,
                ..opts()
            },
        )
        .unwrap_err();
        assert!(err.contains("has an empty segment"), "{err}");
    }

    #[test]
    fn empty_input_is_reported() {
        assert_eq!(
            run("   ", &opts()).unwrap_err(),
            "no JSON provided — paste a JSON object or array to convert"
        );
    }

    #[test]
    fn bad_enum_values_are_reported() {
        let err = convert(
            "{}", "sideways", ".", "bracket", 0, true, true, "preserve", "json", true, 2,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "direction must be 'flatten', 'unflatten' or 'auto', got 'sideways'"
        );
        let err = convert(
            "{}", "flatten", ".", "curly", 0, true, true, "preserve", "json", true, 2,
        )
        .unwrap_err();
        assert!(err.starts_with("array_notation must be 'bracket'"), "{err}");
    }

    #[test]
    fn separator_bounds_are_enforced() {
        let err = convert(
            "{}",
            "flatten",
            "123456789",
            "bracket",
            0,
            true,
            true,
            "preserve",
            "json",
            true,
            2,
        )
        .unwrap_err();
        assert_eq!(err, "separator must be 1-8 characters, got 9");
        let err = convert(
            "{}", "flatten", "[", "bracket", 0, true, true, "preserve", "json", true, 2,
        )
        .unwrap_err();
        assert!(err.contains("must not contain '[' or ']'"), "{err}");
    }

    #[test]
    fn max_depth_and_indent_bounds_are_enforced() {
        let err = convert(
            "{}", "flatten", ".", "bracket", 500, true, true, "preserve", "json", true, 2,
        )
        .unwrap_err();
        assert_eq!(err, "max_depth must be 0 (unlimited) to 100, got 500");
        let err = convert(
            "{}", "flatten", ".", "bracket", 0, true, true, "preserve", "json", true, 99,
        )
        .unwrap_err();
        assert_eq!(err, "indent must be 0-8 spaces, got 99");
    }

    #[test]
    fn convert_string_entry_point_flattens() {
        let out = convert(
            r#"{"a":{"b":1}}"#,
            "flatten",
            ".",
            "bracket",
            0,
            true,
            true,
            "preserve",
            "json",
            false,
            2,
        )
        .unwrap();
        assert_eq!(out, r#"{"a.b":1}"#);
    }
}
