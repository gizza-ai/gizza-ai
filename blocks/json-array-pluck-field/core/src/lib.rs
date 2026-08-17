//! json-array-pluck-field core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Pull ONE field out of every object in a JSON array and emit the values as a
//! flat list (the `_.map(rows, 'field')` / `jq '.[].field'` / SQL single-column
//! `SELECT` operation). Distinct from the general query tools: there is no query
//! language to learn — you type a key name or a dotted path.
//!
//! Input shapes accepted:
//! - a top-level JSON array of objects;
//! - a wrapper object holding the array (`{"data": [...]}`) — the first
//!   array-valued property is used unless `root` names one explicitly;
//! - NDJSON / JSON Lines (one JSON value per line);
//! - a single object, treated as a one-element list.
//!
//! Field paths are dotted: `email`, `user.name`, `tags.0`. Bracket indexes
//! (`tags[0]`) and a leading `$.` are normalised away, `*` matches every
//! element of an array or every value of an object, so `orders.*.total` fans one
//! row out to several values, and `**` is a recursive descent (JSONPath `..`) —
//! `**.city` finds every `city` at any depth.

use serde_json::Value;
use std::collections::HashSet;

/// Largest JSON document accepted, in bytes.
pub const MAX_INPUT_BYTES: usize = 5_000_000;
/// Largest field / root path accepted, in bytes.
pub const MAX_PATH_BYTES: usize = 200;
/// Largest number of plucked values returned.
pub const MAX_VALUES: usize = 200_000;

/// How the flat list of values is joined.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    /// One value per line.
    Lines,
    /// Comma-separated, RFC 4180 quoting when a value needs it.
    Csv,
    /// Tab-separated, RFC 4180 quoting when a value needs it.
    Tsv,
    /// A JSON array of the values in their native JSON types.
    Json,
    /// Joined with the caller's `delimiter`.
    Custom,
}

impl Format {
    /// Parse the descriptor's `format` value; anything unknown falls back to lines.
    pub fn parse(s: &str) -> Format {
        match s.trim().to_ascii_lowercase().as_str() {
            "csv" => Format::Csv,
            "tsv" => Format::Tsv,
            "json" => Format::Json,
            "custom" => Format::Custom,
            _ => Format::Lines,
        }
    }
}

/// What to do about a row whose field is absent (or JSON `null`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Missing {
    /// Leave the row out of the list entirely (default).
    Skip,
    /// Emit an empty value, keeping the list aligned with the rows.
    Empty,
    /// Emit the literal `null`.
    Null,
    /// Fail with the index of the first offending row.
    Error,
}

impl Missing {
    /// Parse the descriptor's `missing` value; anything unknown falls back to skip.
    pub fn parse(s: &str) -> Missing {
        match s.trim().to_ascii_lowercase().as_str() {
            "empty" => Missing::Empty,
            "null" => Missing::Null,
            "error" => Missing::Error,
            _ => Missing::Skip,
        }
    }
}

/// What to do about a plucked value that is itself an object or an array.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Complex {
    /// Serialize it back to compact JSON (default).
    Json,
    /// Replace it with the placeholder `{object}` / `[array]`.
    Label,
    /// Leave it out of the list.
    Skip,
}

impl Complex {
    /// Parse the descriptor's `complex_values` value; unknown falls back to json.
    pub fn parse(s: &str) -> Complex {
        match s.trim().to_ascii_lowercase().as_str() {
            "label" => Complex::Label,
            "skip" => Complex::Skip,
            _ => Complex::Json,
        }
    }
}

/// Everything the pluck needs besides the document itself.
#[derive(Clone, Debug)]
pub struct Options {
    /// Dotted path of the field to pull from each row.
    pub field: String,
    /// Dotted path of the array to pluck from; blank auto-detects it.
    pub root: String,
    pub format: Format,
    /// Separator for `Format::Custom`; `\t`, `\n`, `\r`, `\\` escapes are honoured.
    pub delimiter: String,
    /// Force every value to be quoted.
    pub quote: bool,
    pub missing: Missing,
    pub complex: Complex,
    /// Drop repeated values, keeping the first occurrence.
    pub unique: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            field: String::new(),
            root: String::new(),
            format: Format::Lines,
            delimiter: ", ".into(),
            quote: false,
            missing: Missing::Skip,
            complex: Complex::Json,
            unique: false,
        }
    }
}

/// Turn textual escapes (`\t`, `\n`, `\r`, `\\`) in a delimiter into real bytes.
/// Unknown escapes are left verbatim so `\d` stays `\d`.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Strip one matched pair of surrounding quotes, so the JSONPath bracket form
/// `$['user']` resolves to the plain key `user`.
fn unquote_segment(s: &str) -> &str {
    for q in ['\'', '"'] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// Split a dotted path into segments, normalising `a[0]` to `a.0`, dropping a
/// leading `$` and rewriting the JSONPath recursive descent `..` to a `**`
/// segment, so JSONPath muscle memory still works.
fn split_path(path: &str) -> Vec<String> {
    let trimmed = path.trim();
    let p = trimmed.strip_prefix('$').unwrap_or(trimmed);
    let p = p.replace("..", ".**.");
    let normalised: String = {
        let mut out = String::with_capacity(p.len());
        for c in p.chars() {
            match c {
                '[' => out.push('.'),
                ']' => {}
                other => out.push(other),
            }
        }
        out
    };
    normalised
        .split('.')
        .map(|s| unquote_segment(s.trim()).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Follow one path segment from `value`, appending every match to `out`.
fn step<'a>(value: &'a Value, seg: &str, out: &mut Vec<&'a Value>) {
    if seg == "*" {
        match value {
            Value::Array(items) => out.extend(items.iter()),
            Value::Object(map) => out.extend(map.values()),
            _ => {}
        }
        return;
    }
    match value {
        Value::Object(map) => {
            if let Some(v) = map.get(seg) {
                out.push(v);
            }
        }
        Value::Array(items) => {
            if let Ok(i) = seg.parse::<usize>() {
                if let Some(v) = items.get(i) {
                    out.push(v);
                }
            }
        }
        _ => {}
    }
}

/// Collect `value` and every node nested inside it, outermost first — the
/// recursive-descent (`**`, JSONPath `..`) step.
fn descend<'a>(value: &'a Value, out: &mut Vec<&'a Value>) {
    out.push(value);
    match value {
        Value::Array(items) => items.iter().for_each(|c| descend(c, out)),
        Value::Object(map) => map.values().for_each(|c| descend(c, out)),
        _ => {}
    }
}

/// Resolve a dotted path against `root`, returning every matching node.
fn resolve<'a>(root: &'a Value, segments: &[String]) -> Vec<&'a Value> {
    let mut current: Vec<&Value> = vec![root];
    for seg in segments {
        let mut next: Vec<&Value> = Vec::new();
        for v in &current {
            if seg == "**" {
                descend(v, &mut next);
            } else {
                step(v, seg, &mut next);
            }
        }
        if next.is_empty() {
            return Vec::new();
        }
        current = next;
    }
    current
}

/// Parse the document: strict JSON first, then NDJSON / JSON Lines.
fn parse_document(input: &str) -> Result<Value, String> {
    match serde_json::from_str::<Value>(input) {
        Ok(v) => Ok(v),
        Err(strict_err) => {
            let lines: Vec<&str> = input
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect();
            if lines.len() < 2 {
                return Err(format!("invalid JSON input: {strict_err}"));
            }
            let mut rows = Vec::with_capacity(lines.len());
            for (i, line) in lines.iter().enumerate() {
                match serde_json::from_str::<Value>(line) {
                    Ok(v) => rows.push(v),
                    Err(e) => {
                        return Err(format!(
                            "invalid JSON input: {strict_err} (also tried NDJSON: line {} failed: {e})",
                            i + 1
                        ))
                    }
                }
            }
            Ok(Value::Array(rows))
        }
    }
}

/// Pick the array of rows out of the parsed document.
fn locate_rows(doc: &Value, root: &str) -> Result<Vec<Value>, String> {
    if !root.trim().is_empty() {
        let segs = split_path(root);
        let found = resolve(doc, &segs);
        let node = found
            .first()
            .ok_or_else(|| format!("root path '{root}' matched nothing in the document"))?;
        return match node {
            Value::Array(items) => Ok(items.clone()),
            Value::Object(_) => Ok(vec![(*node).clone()]),
            other => Err(format!(
                "root path '{root}' points at {}, expected an array of objects",
                type_name(other)
            )),
        };
    }
    match doc {
        Value::Array(items) => Ok(items.clone()),
        Value::Object(map) => {
            for (_k, v) in map.iter() {
                if v.is_array() {
                    return Ok(v.as_array().unwrap().clone());
                }
            }
            // No array anywhere: treat the lone object as a one-row list.
            Ok(vec![doc.clone()])
        }
        other => Err(format!(
            "expected a JSON array of objects (or an object containing one), got {}",
            type_name(other)
        )),
    }
}

/// Human-readable JSON type name for error messages.
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

/// Render one plucked value as plain (unquoted) text.
fn plain(v: &Value, complex: Complex) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some("null".to_string()),
        Value::Array(_) | Value::Object(_) => match complex {
            Complex::Skip => None,
            Complex::Label => Some(if v.is_array() {
                "[array]".to_string()
            } else {
                "{object}".to_string()
            }),
            Complex::Json => Some(v.to_string()),
        },
    }
}

/// RFC 4180 field: quote when the value needs it, or always when forced.
fn csv_field(s: &str, delim: char, force: bool) -> String {
    let needs = force
        || s.contains(delim)
        || s.contains('"')
        || s.contains('\n')
        || s.contains('\r');
    if !needs {
        return s.to_string();
    }
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Pluck `opts.field` from every row of the JSON in `input` and join the values.
pub fn pluck(input: &str, opts: &Options) -> Result<String, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, over the {MAX_INPUT_BYTES}-byte limit",
            input.len()
        ));
    }
    if input.trim().is_empty() {
        return Err("input is empty: paste a JSON array of objects".into());
    }
    if opts.field.trim().is_empty() {
        return Err("field is empty: name the key to pull, e.g. 'email' or 'user.name'".into());
    }
    if opts.field.len() > MAX_PATH_BYTES {
        return Err(format!(
            "field path is {} bytes, over the {MAX_PATH_BYTES}-byte limit",
            opts.field.len()
        ));
    }
    if opts.root.len() > MAX_PATH_BYTES {
        return Err(format!(
            "root path is {} bytes, over the {MAX_PATH_BYTES}-byte limit",
            opts.root.len()
        ));
    }

    let doc = parse_document(input)?;
    let rows = locate_rows(&doc, &opts.root)?;
    let segments = split_path(&opts.field);
    if segments.is_empty() {
        return Err(format!(
            "field '{}' has no path segments: use a key like 'email' or 'user.name'",
            opts.field
        ));
    }

    // Collected as JSON values so `format=json` can keep native types.
    let mut values: Vec<Value> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let matches: Vec<&Value> = resolve(row, &segments)
            .into_iter()
            .filter(|v| !v.is_null())
            .collect();
        if matches.is_empty() {
            match opts.missing {
                Missing::Skip => continue,
                Missing::Empty => values.push(Value::String(String::new())),
                Missing::Null => values.push(Value::Null),
                Missing::Error => {
                    return Err(format!(
                        "field '{}' is missing (or null) on element {i}",
                        opts.field
                    ))
                }
            }
            continue;
        }
        for m in matches {
            if opts.complex == Complex::Skip && (m.is_array() || m.is_object()) {
                continue;
            }
            values.push(m.clone());
        }
        if values.len() > MAX_VALUES {
            return Err(format!(
                "more than {MAX_VALUES} values matched; narrow the field path or split the input"
            ));
        }
    }

    if opts.unique {
        let mut seen: HashSet<String> = HashSet::new();
        let mut kept: Vec<Value> = Vec::new();
        for v in values {
            if seen.insert(v.to_string()) {
                kept.push(v);
            }
        }
        values = kept;
    }

    Ok(render(&values, opts))
}

/// Join the collected values per the chosen format.
fn render(values: &[Value], opts: &Options) -> String {
    if opts.format == Format::Json {
        let mapped: Vec<Value> = values
            .iter()
            .map(|v| match (v, opts.complex) {
                (Value::Array(_), Complex::Label) => Value::String("[array]".into()),
                (Value::Object(_), Complex::Label) => Value::String("{object}".into()),
                _ => v.clone(),
            })
            .collect();
        return serde_json::to_string(&mapped).unwrap_or_else(|_| "[]".into());
    }

    let plains: Vec<String> = values
        .iter()
        .filter_map(|v| plain(v, opts.complex))
        .collect();

    match opts.format {
        Format::Csv | Format::Tsv => {
            let d = if opts.format == Format::Csv { ',' } else { '\t' };
            plains
                .iter()
                .map(|s| csv_field(s, d, opts.quote))
                .collect::<Vec<_>>()
                .join(&d.to_string())
        }
        Format::Custom => {
            let d = unescape(&opts.delimiter);
            quoted(&plains, opts.quote).join(&d)
        }
        // Lines (and Json, handled above)
        _ => quoted(&plains, opts.quote).join("\n"),
    }
}

/// Apply JSON-string quoting to every value when `quote` is on.
fn quoted(plains: &[String], quote: bool) -> Vec<String> {
    if !quote {
        return plains.to_vec();
    }
    plains
        .iter()
        .map(|s| Value::String(s.clone()).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(field: &str) -> Options {
        Options {
            field: field.into(),
            ..Options::default()
        }
    }

    #[test]
    fn plucks_a_top_level_key_one_per_line() {
        let json = r#"[{"name":"ada","age":36},{"name":"linus","age":54}]"#;
        assert_eq!(pluck(json, &opts("name")).unwrap(), "ada\nlinus");
    }

    #[test]
    fn plucks_a_dotted_path_and_an_array_index() {
        let json = r#"[{"user":{"name":"ada"},"tags":["x","y"]},{"user":{"name":"grace"},"tags":["z"]}]"#;
        assert_eq!(pluck(json, &opts("user.name")).unwrap(), "ada\ngrace");
        assert_eq!(pluck(json, &opts("tags.0")).unwrap(), "x\nz");
        // bracket syntax normalises to the same thing
        assert_eq!(pluck(json, &opts("tags[0]")).unwrap(), "x\nz");
        // a leading $. is tolerated
        assert_eq!(pluck(json, &opts("$.user.name")).unwrap(), "ada\ngrace");
        assert_eq!(pluck(json, &opts("$['user'].name")).unwrap(), "ada\ngrace");
    }

    #[test]
    fn recursive_descent_finds_a_key_at_any_depth() {
        let json = r#"[{"city":"Oslo","to":{"city":"Bergen"}},{"legs":[{"stop":{"city":"Kyoto"}}]}]"#;
        // ** matches this node and every node inside it, so a single path reaches
        // all three depths without knowing the shape.
        assert_eq!(
            pluck(json, &opts("**.city")).unwrap(),
            "Oslo\nBergen\nKyoto"
        );
        // JSONPath's `..` spelling is rewritten to the same `**` segment.
        assert_eq!(pluck(json, &opts("$..city")).unwrap(), "Oslo\nBergen\nKyoto");
        assert_eq!(pluck(json, &opts("..city")).unwrap(), "Oslo\nBergen\nKyoto");
    }

    #[test]
    fn wildcard_fans_one_row_out_to_several_values() {
        let json = r#"[{"orders":[{"total":10},{"total":5}]},{"orders":[{"total":7}]}]"#;
        assert_eq!(pluck(json, &opts("orders.*.total")).unwrap(), "10\n5\n7");
    }

    #[test]
    fn finds_the_array_inside_a_wrapper_object() {
        let json = r#"{"page":1,"data":[{"id":"a"},{"id":"b"}]}"#;
        assert_eq!(pluck(json, &opts("id")).unwrap(), "a\nb");
    }

    #[test]
    fn root_path_selects_a_specific_array() {
        let json = r#"{"a":[{"id":1}],"b":[{"id":2},{"id":3}]}"#;
        let o = Options {
            root: "b".into(),
            ..opts("id")
        };
        assert_eq!(pluck(json, &o).unwrap(), "2\n3");
    }

    #[test]
    fn accepts_ndjson_lines() {
        let json = "{\"sku\":\"A1\"}\n{\"sku\":\"B2\"}\n{\"sku\":\"C3\"}";
        assert_eq!(pluck(json, &opts("sku")).unwrap(), "A1\nB2\nC3");
    }

    #[test]
    fn a_single_object_is_a_one_row_list() {
        assert_eq!(pluck(r#"{"id":"solo"}"#, &opts("id")).unwrap(), "solo");
    }

    #[test]
    fn formats_join_the_values_differently() {
        let json = r#"[{"n":"a"},{"n":"b"}]"#;
        let csv = Options {
            format: Format::Csv,
            ..opts("n")
        };
        assert_eq!(pluck(json, &csv).unwrap(), "a,b");
        let tsv = Options {
            format: Format::Tsv,
            ..opts("n")
        };
        assert_eq!(pluck(json, &tsv).unwrap(), "a\tb");
        let js = Options {
            format: Format::Json,
            ..opts("n")
        };
        assert_eq!(pluck(json, &js).unwrap(), r#"["a","b"]"#);
        let custom = Options {
            format: Format::Custom,
            delimiter: " | ".into(),
            ..opts("n")
        };
        assert_eq!(pluck(json, &custom).unwrap(), "a | b");
        let escaped = Options {
            format: Format::Custom,
            delimiter: "\\t".into(),
            ..opts("n")
        };
        assert_eq!(pluck(json, &escaped).unwrap(), "a\tb");
    }

    #[test]
    fn json_format_keeps_native_types() {
        let json = r#"[{"n":1},{"n":true},{"n":"x"}]"#;
        let o = Options {
            format: Format::Json,
            ..opts("n")
        };
        assert_eq!(pluck(json, &o).unwrap(), r#"[1,true,"x"]"#);
    }

    #[test]
    fn quote_wraps_values_per_format() {
        let json = r#"[{"n":"a,b"},{"n":"c"}]"#;
        let lines = Options { quote: true, ..opts("n") };
        assert_eq!(pluck(json, &lines).unwrap(), "\"a,b\"\n\"c\"");
        let csv = Options {
            format: Format::Csv,
            quote: true,
            ..opts("n")
        };
        assert_eq!(pluck(json, &csv).unwrap(), "\"a,b\",\"c\"");
    }

    #[test]
    fn csv_auto_quotes_only_values_that_need_it() {
        let json = r#"[{"n":"a,b"},{"n":"c"}]"#;
        let csv = Options {
            format: Format::Csv,
            ..opts("n")
        };
        assert_eq!(pluck(json, &csv).unwrap(), "\"a,b\",c");
    }

    #[test]
    fn missing_modes_control_absent_and_null_fields() {
        let json = r#"[{"e":"a@x"},{},{"e":null},{"e":"b@x"}]"#;
        assert_eq!(pluck(json, &opts("e")).unwrap(), "a@x\nb@x");
        let empty = Options {
            missing: Missing::Empty,
            ..opts("e")
        };
        assert_eq!(pluck(json, &empty).unwrap(), "a@x\n\n\nb@x");
        let null = Options {
            missing: Missing::Null,
            ..opts("e")
        };
        assert_eq!(pluck(json, &null).unwrap(), "a@x\nnull\nnull\nb@x");
        let err = Options {
            missing: Missing::Error,
            ..opts("e")
        };
        assert!(pluck(json, &err).unwrap_err().contains("element 1"));
    }

    #[test]
    fn complex_values_render_three_ways() {
        let json = r#"[{"v":{"a":1}},{"v":[1,2]},{"v":"plain"}]"#;
        assert_eq!(
            pluck(json, &opts("v")).unwrap(),
            "{\"a\":1}\n[1,2]\nplain"
        );
        let label = Options {
            complex: Complex::Label,
            ..opts("v")
        };
        assert_eq!(pluck(json, &label).unwrap(), "{object}\n[array]\nplain");
        let skip = Options {
            complex: Complex::Skip,
            ..opts("v")
        };
        assert_eq!(pluck(json, &skip).unwrap(), "plain");
    }

    #[test]
    fn unique_drops_repeats_keeping_first_order() {
        let json = r#"[{"c":"eu"},{"c":"us"},{"c":"eu"},{"c":"jp"}]"#;
        let o = Options {
            unique: true,
            ..opts("c")
        };
        assert_eq!(pluck(json, &o).unwrap(), "eu\nus\njp");
    }

    #[test]
    fn empty_result_is_an_empty_string_or_empty_json_array() {
        let json = r#"[{"a":1}]"#;
        assert_eq!(pluck(json, &opts("nope")).unwrap(), "");
        let js = Options {
            format: Format::Json,
            ..opts("nope")
        };
        assert_eq!(pluck(json, &js).unwrap(), "[]");
    }

    #[test]
    fn rejects_invalid_json_with_a_reason() {
        let err = pluck("{not json", &opts("a")).unwrap_err();
        assert!(err.starts_with("invalid JSON input:"), "got {err}");
    }

    #[test]
    fn rejects_a_missing_field_name() {
        let err = pluck(r#"[{"a":1}]"#, &opts("  ")).unwrap_err();
        assert!(err.contains("field is empty"), "got {err}");
    }

    #[test]
    fn rejects_a_root_path_that_is_not_an_array() {
        let o = Options {
            root: "page".into(),
            ..opts("id")
        };
        let err = pluck(r#"{"page":1,"data":[{"id":"a"}]}"#, &o).unwrap_err();
        assert!(err.contains("expected an array of objects"), "got {err}");
    }

    #[test]
    fn rejects_a_scalar_document() {
        let err = pluck("42", &opts("a")).unwrap_err();
        assert!(err.contains("expected a JSON array of objects"), "got {err}");
    }

    #[test]
    fn input_byte_cap_is_exact() {
        // A padded string value takes the document to exactly the cap, then one over.
        let head = r#"[{"n":""#;
        let tail = r#""}]"#;
        let pad = MAX_INPUT_BYTES - head.len() - tail.len();
        let at = format!("{head}{}{tail}", "x".repeat(pad));
        assert_eq!(at.len(), MAX_INPUT_BYTES);
        assert!(pluck(&at, &opts("n")).is_ok());

        let over = format!("{head}{}{tail}", "x".repeat(pad + 1));
        assert_eq!(over.len(), MAX_INPUT_BYTES + 1);
        assert!(pluck(&over, &opts("n"))
            .unwrap_err()
            .contains("over the 5000000-byte limit"));
    }

    #[test]
    fn field_path_cap_is_exact() {
        let json = r#"[{"a":1}]"#;
        let at = "a".repeat(MAX_PATH_BYTES);
        assert!(pluck(json, &opts(&at)).is_ok());
        let over = "a".repeat(MAX_PATH_BYTES + 1);
        assert!(pluck(json, &opts(&over))
            .unwrap_err()
            .contains("over the 200-byte limit"));
    }
}
