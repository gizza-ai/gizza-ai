//! gizza-ai/json-transform-rules core — reshape a JSON document into a new
//! structure with a list of declarative match/transform rules. Pure compute,
//! shared by the chat skill block, the CLI, and the web page. No wafer/
//! wasm-bindgen deps and no host/WASI calls, so it runs in the gizza wafer
//! runtime (chat SW), the CLI, and the browser page alike.
//!
//! The model is a **rule list**, not an expression language: every rule names an
//! output `target` path and where its value comes from (a `source` selector or a
//! literal `value`), so a mapping is data you can store, diff and review rather
//! than a one-line program.
//!
//! Rules can be written three ways:
//!   - **shorthand lines** — `fullName = $.user.name`, one per line, `#`/`//`
//!     comments, `-path` to drop a path in merge mode, a quoted/JSON literal RHS
//!     for a constant;
//!   - **a JSON object** — `{ "fullName": "$.user.name", "id": "$.user.id" }`,
//!     key = target, value = selector (or a rule object);
//!   - **a JSON array** of rule objects — the full form, with `default`,
//!     `transform`, `when` and `op`.
//!
//! Selectors are a JSONPath subset: `$`, `.key`, `["key"]`, `[0]`, `[*]`, `.*`
//! and `..key` (recursive descent), plus the meta selectors `#index` / `#count`
//! (the current item's position and the item total under `each`). Targets are
//! dotted/bracketed output paths (`user.profile.email`, `items[0].id`, `tags[]`
//! to append) that create the intermediate objects and arrays they need.

use serde::Serialize;
use serde_json::{Map, Value};

/// Largest accepted JSON document, in bytes.
pub const MAX_INPUT_BYTES: usize = 5_000_000;
/// Largest accepted rules document, in bytes.
pub const MAX_RULES_BYTES: usize = 200_000;
/// Maximum number of rules in one run.
pub const MAX_RULES: usize = 500;
/// Maximum values a single selector may match.
pub const MAX_MATCHES: usize = 100_000;
/// Cap on how far a target index may grow an output array.
pub const MAX_ARRAY_GROW: usize = 10_000;
/// Maximum items an `each` selector may fan out over.
pub const MAX_ITEMS: usize = 50_000;

// ---------------------------------------------------------------------------
// Selectors (the JSONPath subset used by `source`, `when` and `each`)
// ---------------------------------------------------------------------------

/// One parsed selector step.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seg {
    /// `.key` / `["key"]`
    Key(String),
    /// `[0]`
    Index(usize),
    /// `[*]` / `.*` — every member of the current object or array.
    Wildcard,
    /// `..key` — every `key` at any depth, in document order.
    Descend(String),
    /// `..*` — every value at any depth.
    DescendAll,
}

/// Parse a selector into steps. An empty selector (or a bare `$`) selects the
/// document root itself.
fn parse_selector(sel: &str) -> Result<Vec<Seg>, String> {
    let s = sel.trim();
    let bytes: Vec<char> = s.strip_prefix('$').unwrap_or(s).chars().collect();
    let mut segs = Vec::new();
    let mut i = 0usize;
    let mut first = true;
    while i < bytes.len() {
        match bytes[i] {
            '.' if i + 1 < bytes.len() && bytes[i + 1] == '.' => {
                i += 2;
                let name = read_name(&bytes, &mut i);
                if name.is_empty() {
                    return Err(format!(
                        "invalid selector '{sel}': expected a key or '*' after '..'"
                    ));
                }
                segs.push(if name == "*" {
                    Seg::DescendAll
                } else {
                    Seg::Descend(name)
                });
            }
            '.' => {
                i += 1;
                let name = read_name(&bytes, &mut i);
                if name.is_empty() {
                    return Err(format!(
                        "invalid selector '{sel}': expected a key or '*' after '.'"
                    ));
                }
                segs.push(if name == "*" {
                    Seg::Wildcard
                } else {
                    Seg::Key(name)
                });
            }
            '[' => {
                i += 1;
                segs.push(read_bracket(&bytes, &mut i, sel)?);
            }
            _ if first => {
                // A leading bare key: `user.name` behaves like `$.user.name`.
                let name = read_name(&bytes, &mut i);
                if name.is_empty() {
                    return Err(format!(
                        "invalid selector '{sel}': unexpected character '{}'",
                        bytes[i]
                    ));
                }
                segs.push(if name == "*" {
                    Seg::Wildcard
                } else {
                    Seg::Key(name)
                });
            }
            other => {
                return Err(format!(
                    "invalid selector '{sel}': unexpected character '{other}' (expected '.', '..' or '[')"
                ))
            }
        }
        first = false;
    }
    Ok(segs)
}

/// Read a bare key up to the next `.` or `[`.
fn read_name(bytes: &[char], i: &mut usize) -> String {
    let start = *i;
    while *i < bytes.len() && bytes[*i] != '.' && bytes[*i] != '[' {
        *i += 1;
    }
    bytes[start..*i].iter().collect::<String>().trim().to_string()
}

/// Read one `[...]` step; `*i` points just past the `[` and ends past the `]`.
fn read_bracket(bytes: &[char], i: &mut usize, sel: &str) -> Result<Seg, String> {
    // Quoted key: ["a.b"] / ['a.b']
    if *i < bytes.len() && (bytes[*i] == '"' || bytes[*i] == '\'') {
        let quote = bytes[*i];
        *i += 1;
        let start = *i;
        while *i < bytes.len() && bytes[*i] != quote {
            *i += 1;
        }
        if *i >= bytes.len() {
            return Err(format!("invalid selector '{sel}': unterminated quoted key"));
        }
        let key: String = bytes[start..*i].iter().collect();
        *i += 1; // closing quote
        if *i >= bytes.len() || bytes[*i] != ']' {
            return Err(format!("invalid selector '{sel}': expected ']' after a quoted key"));
        }
        *i += 1;
        return Ok(Seg::Key(key));
    }
    let start = *i;
    while *i < bytes.len() && bytes[*i] != ']' {
        *i += 1;
    }
    if *i >= bytes.len() {
        return Err(format!("invalid selector '{sel}': unterminated '[' (expected ']')"));
    }
    let inner: String = bytes[start..*i].iter().collect();
    *i += 1; // ']'
    let inner = inner.trim();
    if inner == "*" {
        return Ok(Seg::Wildcard);
    }
    inner
        .parse::<usize>()
        .map(Seg::Index)
        .map_err(|_| format!("invalid selector '{sel}': expected a number, '*' or a quoted key inside [], got '{inner}'"))
}

/// Evaluate a selector against `root`, returning matches in document order.
fn select<'a>(root: &'a Value, segs: &[Seg]) -> Result<Vec<&'a Value>, String> {
    let mut cur: Vec<&Value> = vec![root];
    for seg in segs {
        let mut next: Vec<&Value> = Vec::new();
        for node in &cur {
            match seg {
                Seg::Key(k) => {
                    if let Value::Object(m) = node {
                        if let Some(v) = m.get(k) {
                            next.push(v);
                        }
                    }
                }
                Seg::Index(idx) => {
                    if let Value::Array(a) = node {
                        if let Some(v) = a.get(*idx) {
                            next.push(v);
                        }
                    }
                }
                Seg::Wildcard => match node {
                    Value::Array(a) => next.extend(a.iter()),
                    Value::Object(m) => next.extend(m.values()),
                    _ => {}
                },
                Seg::Descend(k) => collect_descend(node, Some(k), &mut next),
                Seg::DescendAll => collect_descend(node, None, &mut next),
            }
            if next.len() > MAX_MATCHES {
                return Err(format!(
                    "selector matched more than {MAX_MATCHES} values; narrow it down"
                ));
            }
        }
        cur = next;
    }
    Ok(cur)
}

/// Pre-order recursive descent: every value under `key` (or every value at all
/// when `key` is `None`).
fn collect_descend<'a>(node: &'a Value, key: Option<&str>, out: &mut Vec<&'a Value>) {
    match node {
        Value::Object(m) => {
            for (k, v) in m {
                match key {
                    Some(want) if k == want => out.push(v),
                    None => out.push(v),
                    _ => {}
                }
                collect_descend(v, key, out);
            }
        }
        Value::Array(a) => {
            for v in a {
                if key.is_none() {
                    out.push(v);
                }
                collect_descend(v, key, out);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Target paths (where a rule writes)
// ---------------------------------------------------------------------------

/// One parsed output-path step.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TSeg {
    Key(String),
    Index(usize),
    /// `[]` — append a new element to the array at this position.
    Append,
}

/// Parse a target path. An empty path (or a bare `$`) means the whole output.
fn parse_target(path: &str) -> Result<Vec<TSeg>, String> {
    let s = path.trim();
    let bytes: Vec<char> = s.strip_prefix('$').unwrap_or(s).chars().collect();
    let mut segs = Vec::new();
    let mut i = 0usize;
    let mut first = true;
    while i < bytes.len() {
        match bytes[i] {
            '.' => {
                i += 1;
                let name = read_name(&bytes, &mut i);
                if name.is_empty() {
                    return Err(format!("invalid target path '{path}': expected a key after '.'"));
                }
                segs.push(name_to_tseg(&name));
            }
            '[' => {
                i += 1;
                segs.push(read_target_bracket(&bytes, &mut i, path)?);
            }
            _ if first => {
                let name = read_name(&bytes, &mut i);
                if name.is_empty() {
                    return Err(format!(
                        "invalid target path '{path}': unexpected character '{}'",
                        bytes[i]
                    ));
                }
                segs.push(name_to_tseg(&name));
            }
            other => {
                return Err(format!(
                    "invalid target path '{path}': unexpected character '{other}' (expected '.' or '[')"
                ))
            }
        }
        first = false;
    }
    Ok(segs)
}

/// A bare all-digits segment addresses an array index (`items.0.id`).
fn name_to_tseg(name: &str) -> TSeg {
    match name.parse::<usize>() {
        Ok(n) => TSeg::Index(n),
        Err(_) => TSeg::Key(name.to_string()),
    }
}

fn read_target_bracket(bytes: &[char], i: &mut usize, path: &str) -> Result<TSeg, String> {
    if *i < bytes.len() && (bytes[*i] == '"' || bytes[*i] == '\'') {
        let quote = bytes[*i];
        *i += 1;
        let start = *i;
        while *i < bytes.len() && bytes[*i] != quote {
            *i += 1;
        }
        if *i >= bytes.len() {
            return Err(format!("invalid target path '{path}': unterminated quoted key"));
        }
        let key: String = bytes[start..*i].iter().collect();
        *i += 1;
        if *i >= bytes.len() || bytes[*i] != ']' {
            return Err(format!(
                "invalid target path '{path}': expected ']' after a quoted key"
            ));
        }
        *i += 1;
        return Ok(TSeg::Key(key));
    }
    let start = *i;
    while *i < bytes.len() && bytes[*i] != ']' {
        *i += 1;
    }
    if *i >= bytes.len() {
        return Err(format!("invalid target path '{path}': unterminated '[' (expected ']')"));
    }
    let inner: String = bytes[start..*i].iter().collect();
    *i += 1;
    let inner = inner.trim();
    if inner.is_empty() {
        return Ok(TSeg::Append);
    }
    inner.parse::<usize>().map(TSeg::Index).map_err(|_| {
        format!("invalid target path '{path}': expected a number, '' or a quoted key inside [], got '{inner}'")
    })
}

/// Write `val` at `segs`, creating intermediate objects/arrays as needed.
fn set_path(root: &mut Value, segs: &[TSeg], val: Value) -> Result<(), String> {
    let Some((seg, rest)) = segs.split_first() else {
        *root = val;
        return Ok(());
    };
    match seg {
        TSeg::Key(k) => {
            if root.is_null() {
                *root = Value::Object(Map::new());
            }
            let Value::Object(m) = root else {
                return Err(format!(
                    "cannot set key '{k}': that position already holds {}",
                    type_name(root)
                ));
            };
            if !m.contains_key(k) {
                m.insert(k.clone(), Value::Null);
            }
            set_path(m.get_mut(k).expect("just inserted"), rest, val)
        }
        TSeg::Index(idx) => {
            if root.is_null() {
                *root = Value::Array(Vec::new());
            }
            let Value::Array(a) = root else {
                return Err(format!(
                    "cannot set index [{idx}]: that position already holds {}",
                    type_name(root)
                ));
            };
            if *idx >= MAX_ARRAY_GROW {
                return Err(format!(
                    "target index [{idx}] is above the {MAX_ARRAY_GROW} element cap"
                ));
            }
            while a.len() <= *idx {
                a.push(Value::Null);
            }
            set_path(&mut a[*idx], rest, val)
        }
        TSeg::Append => {
            if root.is_null() {
                *root = Value::Array(Vec::new());
            }
            let Value::Array(a) = root else {
                return Err(format!(
                    "cannot append with []: that position already holds {}",
                    type_name(root)
                ));
            };
            if a.len() >= MAX_ARRAY_GROW {
                return Err(format!(
                    "appending with [] passed the {MAX_ARRAY_GROW} element cap"
                ));
            }
            a.push(Value::Null);
            let last = a.len() - 1;
            set_path(&mut a[last], rest, val)
        }
    }
}

/// Read the value at `segs`, if the path exists. `[]` (append) never resolves.
fn get_path<'a>(root: &'a Value, segs: &[TSeg]) -> Option<&'a Value> {
    let mut cur = root;
    for seg in segs {
        cur = match (seg, cur) {
            (TSeg::Key(k), Value::Object(m)) => m.get(k)?,
            (TSeg::Index(i), Value::Array(a)) => a.get(*i)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// Delete the value at `segs`. Returns true when something was removed.
fn remove_path(root: &mut Value, segs: &[TSeg]) -> Result<bool, String> {
    let Some((last, parents)) = segs.split_last() else {
        *root = Value::Null;
        return Ok(true);
    };
    let mut cur = root;
    for seg in parents {
        cur = match (seg, cur) {
            (TSeg::Key(k), Value::Object(m)) => match m.get_mut(k) {
                Some(v) => v,
                None => return Ok(false),
            },
            (TSeg::Index(i), Value::Array(a)) => match a.get_mut(*i) {
                Some(v) => v,
                None => return Ok(false),
            },
            (TSeg::Append, _) => {
                return Err("'[]' (append) cannot be used in a remove path".to_string())
            }
            _ => return Ok(false),
        };
    }
    match (last, cur) {
        (TSeg::Key(k), Value::Object(m)) => Ok(m.shift_remove(k).is_some()),
        (TSeg::Index(i), Value::Array(a)) => {
            if *i < a.len() {
                a.remove(*i);
                Ok(true)
            } else {
                Ok(false)
            }
        }
        (TSeg::Append, _) => Err("'[]' (append) cannot be used in a remove path".to_string()),
        _ => Ok(false),
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

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

/// Per-value / per-list / whole-list transforms a rule can apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transform {
    None,
    Upper,
    Lower,
    Trim,
    Number,
    Text,
    Boolean,
    Length,
    Sort,
    Unique,
    Count,
    Sum,
    Min,
    Max,
    Avg,
    First,
    Last,
    Join,
    Reverse,
    Flatten,
    Keys,
    Values,
}

const TRANSFORM_NAMES: &str = "upper, lower, trim, number, string, boolean, length, sort, reverse, unique, flatten, keys, values, count, sum, min, max, avg, first, last, join";

impl Transform {
    fn parse(s: &str) -> Result<Transform, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" => Transform::None,
            "upper" | "uppercase" => Transform::Upper,
            "lower" | "lowercase" => Transform::Lower,
            "trim" => Transform::Trim,
            "number" | "num" => Transform::Number,
            "string" | "str" | "text" => Transform::Text,
            "boolean" | "bool" => Transform::Boolean,
            "length" | "len" => Transform::Length,
            "sort" => Transform::Sort,
            "reverse" => Transform::Reverse,
            "unique" | "uniq" => Transform::Unique,
            "flatten" | "flat" => Transform::Flatten,
            "keys" => Transform::Keys,
            "values" => Transform::Values,
            "count" => Transform::Count,
            "sum" => Transform::Sum,
            "min" => Transform::Min,
            "max" => Transform::Max,
            "avg" | "average" | "mean" => Transform::Avg,
            "first" => Transform::First,
            "last" => Transform::Last,
            "join" => Transform::Join,
            other => {
                return Err(format!(
                    "unknown transform '{other}' (expected one of: {TRANSFORM_NAMES})"
                ))
            }
        })
    }

    fn name(self) -> &'static str {
        match self {
            Transform::None => "none",
            Transform::Upper => "upper",
            Transform::Lower => "lower",
            Transform::Trim => "trim",
            Transform::Number => "number",
            Transform::Text => "string",
            Transform::Boolean => "boolean",
            Transform::Length => "length",
            Transform::Sort => "sort",
            Transform::Reverse => "reverse",
            Transform::Unique => "unique",
            Transform::Flatten => "flatten",
            Transform::Keys => "keys",
            Transform::Values => "values",
            Transform::Count => "count",
            Transform::Sum => "sum",
            Transform::Min => "min",
            Transform::Max => "max",
            Transform::Avg => "avg",
            Transform::First => "first",
            Transform::Last => "last",
            Transform::Join => "join",
        }
    }
}

/// One parsed rule.
#[derive(Debug, Clone)]
struct Rule {
    target: String,
    tsegs: Vec<TSeg>,
    source: Option<Vec<Seg>>,
    source_text: String,
    value: Option<Value>,
    fallback: Option<Value>,
    transform: Transform,
    separator: String,
    remove: bool,
    /// `op = "default"` / the `target ?= selector` shorthand: write only when the
    /// target path is still absent (or null).
    only_if_absent: bool,
    when: Option<Vec<Seg>>,
    when_text: String,
}

const RULE_KEYS: &str =
    "target, source, value, default, transform, separator, when, op";

fn parse_rules(text: &str) -> Result<Vec<Rule>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("no rules given: add at least one rule, e.g. name = $.user.name".to_string());
    }
    let rules = if t.starts_with('[') || t.starts_with('{') {
        let doc: Value = serde_json::from_str(t)
            .map_err(|e| format!("invalid rules JSON: {e}"))?;
        match doc {
            Value::Array(items) => items
                .into_iter()
                .enumerate()
                .map(|(i, item)| match item {
                    Value::Object(m) => rule_from_object(None, &m, i + 1),
                    other => Err(format!(
                        "rule {} must be an object like {{\"target\": \"id\", \"source\": \"$.user.id\"}}, got {}",
                        i + 1,
                        type_name(&other)
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?,
            Value::Object(m) => {
                if m.contains_key("target") {
                    vec![rule_from_object(None, &m, 1)?]
                } else {
                    m.iter()
                        .enumerate()
                        .map(|(i, (k, v))| match v {
                            Value::String(s) => {
                                let (target, only_if_absent) = split_optional(k);
                                shorthand_rule(target, s, i + 1, only_if_absent)
                            }
                            Value::Object(inner) => rule_from_object(Some(k), inner, i + 1),
                            other => Err(format!(
                                "rule '{k}' must be a selector string or a rule object, got {}",
                                type_name(other)
                            )),
                        })
                        .collect::<Result<Vec<_>, _>>()?
                }
            }
            other => {
                return Err(format!(
                    "rules must be a JSON array of rule objects, a target → selector object, or 'target = selector' lines, got {}",
                    type_name(&other)
                ))
            }
        }
    } else {
        parse_shorthand(t)?
    };
    if rules.is_empty() {
        return Err("no rules given: add at least one rule, e.g. name = $.user.name".to_string());
    }
    if rules.len() > MAX_RULES {
        return Err(format!(
            "{} rules given; the maximum is {MAX_RULES}",
            rules.len()
        ));
    }
    Ok(rules)
}

/// `target = selector` / `target = "constant"` / `-target` lines.
fn parse_shorthand(text: &str) -> Result<Vec<Rule>, String> {
    let mut out = Vec::new();
    for (ln, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if let Some(path) = line.strip_prefix('-') {
            let path = path.trim();
            if path.is_empty() {
                return Err(format!("line {}: '-' needs a path to remove, e.g. -user.ssn", ln + 1));
            }
            out.push(Rule {
                target: path.to_string(),
                tsegs: parse_target(path).map_err(|e| format!("line {}: {e}", ln + 1))?,
                source: None,
                source_text: String::new(),
                value: None,
                fallback: None,
                transform: Transform::None,
                separator: ",".to_string(),
                remove: true,
                only_if_absent: false,
                when: None,
                when_text: String::new(),
            });
            continue;
        }
        let Some((lhs, rhs)) = line.split_once('=') else {
            return Err(format!(
                "line {}: expected 'target = selector' (or '-path' to remove), got '{line}'",
                ln + 1
            ));
        };
        let (target, only_if_absent) = split_optional(lhs);
        let rhs = rhs.trim();
        if target.is_empty() {
            return Err(format!("line {}: missing the target path before '='", ln + 1));
        }
        if rhs.is_empty() {
            return Err(format!(
                "line {}: expected a selector or constant after '=', e.g. {target} = $.user.name",
                ln + 1
            ));
        }
        out.push(shorthand_rule(target, rhs, ln + 1, only_if_absent)?);
    }
    Ok(out)
}

/// A target may carry a trailing `?` (`tier ?= "free"`, or the key `"tier?"` in
/// the object form) meaning "write only when this path is still absent".
fn split_optional(target: &str) -> (&str, bool) {
    match target.trim().strip_suffix('?') {
        Some(t) => (t.trim(), true),
        None => (target.trim(), false),
    }
}

/// Build a rule from a `target` + a right-hand side that is either a JSON
/// literal (constant) or a selector.
fn shorthand_rule(
    target: &str,
    rhs: &str,
    n: usize,
    only_if_absent: bool,
) -> Result<Rule, String> {
    let rhs_t = rhs.trim();
    let literal = if rhs_t.starts_with('"') || rhs_t.starts_with('[') || rhs_t.starts_with('{') {
        serde_json::from_str::<Value>(rhs_t).ok()
    } else if matches!(rhs_t, "true" | "false" | "null") || rhs_t.parse::<f64>().is_ok() {
        serde_json::from_str::<Value>(rhs_t).ok()
    } else {
        None
    };
    let tsegs = parse_target(target).map_err(|e| format!("rule {n}: {e}"))?;
    let (source, source_text, value) = match literal {
        Some(v) => (None, String::new(), Some(v)),
        None => (
            Some(parse_selector(rhs_t).map_err(|e| format!("rule {n}: {e}"))?),
            rhs_t.to_string(),
            None,
        ),
    };
    Ok(Rule {
        target: target.to_string(),
        tsegs,
        source,
        source_text,
        value,
        fallback: None,
        transform: Transform::None,
        separator: ",".to_string(),
        remove: false,
        only_if_absent,
        when: None,
        when_text: String::new(),
    })
}

fn rule_from_object(
    outer_target: Option<&str>,
    m: &Map<String, Value>,
    n: usize,
) -> Result<Rule, String> {
    for k in m.keys() {
        if !matches!(
            k.as_str(),
            "target" | "source" | "value" | "default" | "transform" | "separator" | "when" | "op"
        ) {
            return Err(format!(
                "rule {n}: unknown field '{k}' (expected one of: {RULE_KEYS})"
            ));
        }
    }
    let (target, only_if_absent) = match (outer_target, m.get("target")) {
        (Some(t), _) => {
            let (t, opt) = split_optional(t);
            (t.to_string(), opt)
        }
        (None, Some(Value::String(t))) => {
            let (t, opt) = split_optional(t);
            (t.to_string(), opt)
        }
        (None, Some(other)) => {
            return Err(format!(
                "rule {n}: 'target' must be a string path, got {}",
                type_name(other)
            ))
        }
        (None, None) => {
            return Err(format!(
                "rule {n}: missing 'target' (the output path this rule writes)"
            ))
        }
    };
    let op = match m.get("op") {
        None => "set".to_string(),
        Some(Value::String(s)) => s.trim().to_ascii_lowercase(),
        Some(other) => {
            return Err(format!(
                "rule {n}: 'op' must be \"set\" or \"remove\", got {}",
                type_name(other)
            ))
        }
    };
    let remove = match op.as_str() {
        "set" => false,
        "remove" | "delete" => true,
        other => {
            return Err(format!(
                "rule {n}: unknown op '{other}' (expected \"set\" or \"remove\")"
            ))
        }
    };
    let source_text = match m.get("source") {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => {
            return Err(format!(
                "rule {n}: 'source' must be a selector string, got {}",
                type_name(other)
            ))
        }
    };
    let value = m.get("value").cloned();
    if value.is_some() && !source_text.trim().is_empty() {
        return Err(format!(
            "rule {n} ('{target}'): use either 'source' or 'value', not both"
        ));
    }
    if value.is_none() && source_text.trim().is_empty() && !remove {
        return Err(format!(
            "rule {n} ('{target}'): needs a 'source' selector or a literal 'value'"
        ));
    }
    let transform = match m.get("transform") {
        None => Transform::None,
        Some(Value::String(s)) => Transform::parse(s).map_err(|e| format!("rule {n}: {e}"))?,
        Some(other) => {
            return Err(format!(
                "rule {n}: 'transform' must be a string, got {}",
                type_name(other)
            ))
        }
    };
    let separator = match m.get("separator") {
        None => ",".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => {
            return Err(format!(
                "rule {n}: 'separator' must be a string, got {}",
                type_name(other)
            ))
        }
    };
    let when_text = match m.get("when") {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => {
            return Err(format!(
                "rule {n}: 'when' must be a selector string, got {}",
                type_name(other)
            ))
        }
    };
    Ok(Rule {
        tsegs: parse_target(&target).map_err(|e| format!("rule {n}: {e}"))?,
        source: if source_text.trim().is_empty() {
            None
        } else {
            Some(parse_selector(&source_text).map_err(|e| format!("rule {n}: {e}"))?)
        },
        source_text,
        value,
        fallback: m.get("default").cloned(),
        transform,
        separator,
        remove,
        only_if_absent,
        when: if when_text.trim().is_empty() {
            None
        } else {
            Some(parse_selector(&when_text).map_err(|e| format!("rule {n}: {e}"))?)
        },
        when_text,
        target,
    })
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// How the output document starts out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Start from `{}` and write only what the rules produce.
    Build,
    /// Start from a copy of the input and let the rules override/remove parts.
    Merge,
}

/// What to do when a rule's selector matches nothing and it has no `default`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnMissing {
    Skip,
    Null,
    Fail,
}

/// How multi-value matches are shaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArrayMode {
    /// One match → the value itself; several → an array.
    Auto,
    /// Always an array (an empty one when nothing matched).
    Always,
    /// Only the first match.
    First,
}

fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "build" => Ok(Mode::Build),
        "merge" => Ok(Mode::Merge),
        other => Err(format!("unknown mode '{other}' (expected build or merge)")),
    }
}

fn parse_on_missing(s: &str) -> Result<OnMissing, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "skip" => Ok(OnMissing::Skip),
        "null" => Ok(OnMissing::Null),
        "error" | "fail" => Ok(OnMissing::Fail),
        other => Err(format!(
            "unknown on_missing '{other}' (expected skip, null or error)"
        )),
    }
}

fn parse_array_mode(s: &str) -> Result<ArrayMode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(ArrayMode::Auto),
        "always" => Ok(ArrayMode::Always),
        "first" => Ok(ArrayMode::First),
        other => Err(format!(
            "unknown array_mode '{other}' (expected auto, always or first)"
        )),
    }
}

// ---------------------------------------------------------------------------
// Transforms
// ---------------------------------------------------------------------------

fn scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn to_f64(v: &Value, what: &str, target: &str) -> Result<f64, String> {
    match v {
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| format!("transform '{what}' hit a number it cannot read at target '{target}'")),
        Value::String(s) => s.trim().parse::<f64>().map_err(|_| {
            format!("transform '{what}' expected a number at target '{target}', got the string \"{s}\"")
        }),
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        other => Err(format!(
            "transform '{what}' expected a number at target '{target}', got {}",
            type_name(other)
        )),
    }
}

fn num_value(f: f64) -> Value {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 9e15 {
        Value::from(f as i64)
    } else {
        serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "" | "false" | "0" | "no" | "off"
        ),
        Value::Array(a) => !a.is_empty(),
        Value::Object(m) => !m.is_empty(),
    }
}

fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        _ => scalar_to_string(a).cmp(&scalar_to_string(b)),
    }
}

/// A lone match that is itself an array *is* the list a list/aggregate transform
/// should work on, so `total = $.prices | sum` behaves like `$.prices[*] | sum`.
/// The returned flag says the result has to be re-wrapped into one array value.
fn unwrap_single_array(mut vals: Vec<Value>) -> (Vec<Value>, bool) {
    if vals.len() == 1 && vals[0].is_array() {
        if let Value::Array(a) = vals.remove(0) {
            return (a, true);
        }
    }
    (vals, false)
}

/// Undo `unwrap_single_array` shaping: a list that came out of a single array
/// match goes back in as one array value.
fn rewrap(items: Vec<Value>, wrapped: bool) -> Vec<Value> {
    if wrapped {
        vec![Value::Array(items)]
    } else {
        items
    }
}

/// Apply `t` to the list of matched values. Returns `None` when the transform
/// collapses to "nothing matched" (`first`/`last`/`min`/`max`/`avg` on an empty
/// list), which then falls through to `default` / `on_missing`.
fn apply_transform(
    t: Transform,
    vals: Vec<Value>,
    sep: &str,
    target: &str,
) -> Result<Option<Vec<Value>>, String> {
    let one = |v: Value| Ok(Some(vec![v]));
    match t {
        Transform::None => Ok(Some(vals)),
        Transform::Upper | Transform::Lower | Transform::Trim => {
            let mut out = Vec::with_capacity(vals.len());
            for v in vals {
                if v.is_array() || v.is_object() {
                    return Err(format!(
                        "transform '{}' expected a string at target '{target}', got {}",
                        t.name(),
                        type_name(&v)
                    ));
                }
                let s = scalar_to_string(&v);
                out.push(Value::String(match t {
                    Transform::Upper => s.to_uppercase(),
                    Transform::Lower => s.to_lowercase(),
                    _ => s.trim().to_string(),
                }));
            }
            Ok(Some(out))
        }
        Transform::Number => {
            let mut out = Vec::with_capacity(vals.len());
            for v in vals {
                out.push(num_value(to_f64(&v, "number", target)?));
            }
            Ok(Some(out))
        }
        Transform::Text => Ok(Some(
            vals.into_iter()
                .map(|v| Value::String(scalar_to_string(&v)))
                .collect(),
        )),
        Transform::Boolean => Ok(Some(
            vals.into_iter().map(|v| Value::Bool(truthy(&v))).collect(),
        )),
        Transform::Length => {
            let mut out = Vec::with_capacity(vals.len());
            for v in vals {
                let n = match &v {
                    Value::String(s) => s.chars().count(),
                    Value::Array(a) => a.len(),
                    Value::Object(m) => m.len(),
                    other => {
                        return Err(format!(
                            "transform 'length' expected a string, array or object at target '{target}', got {}",
                            type_name(other)
                        ))
                    }
                };
                out.push(Value::from(n));
            }
            Ok(Some(out))
        }
        Transform::Sort => {
            let (mut items, wrapped) = unwrap_single_array(vals);
            items.sort_by(compare_values);
            Ok(Some(rewrap(items, wrapped)))
        }
        Transform::Reverse => {
            let (mut items, wrapped) = unwrap_single_array(vals);
            items.reverse();
            Ok(Some(rewrap(items, wrapped)))
        }
        Transform::Unique => {
            let (items, wrapped) = unwrap_single_array(vals);
            let mut seen: Vec<String> = Vec::new();
            let mut out = Vec::new();
            for v in items {
                let k = v.to_string();
                if !seen.contains(&k) {
                    seen.push(k);
                    out.push(v);
                }
            }
            Ok(Some(rewrap(out, wrapped)))
        }
        Transform::Flatten => {
            let (items, wrapped) = unwrap_single_array(vals);
            let mut out: Vec<Value> = Vec::with_capacity(items.len());
            for v in items {
                match v {
                    Value::Array(inner) => out.extend(inner),
                    other => out.push(other),
                }
                if out.len() > MAX_MATCHES {
                    return Err(format!(
                        "transform 'flatten' produced more than {MAX_MATCHES} values at target '{target}'"
                    ));
                }
            }
            Ok(Some(rewrap(out, wrapped)))
        }
        Transform::Keys | Transform::Values => {
            let want_keys = t == Transform::Keys;
            let mut out = Vec::with_capacity(vals.len());
            for v in vals {
                out.push(match (&v, want_keys) {
                    (Value::Object(m), true) => {
                        Value::Array(m.keys().map(|k| Value::String(k.clone())).collect())
                    }
                    (Value::Object(m), false) => Value::Array(m.values().cloned().collect()),
                    // An array's "keys" are its indexes; its "values" are itself.
                    (Value::Array(a), true) => {
                        Value::Array((0..a.len()).map(Value::from).collect())
                    }
                    (Value::Array(a), false) => Value::Array(a.clone()),
                    (other, _) => {
                        return Err(format!(
                            "transform '{}' expected an object or array at target '{target}', got {}",
                            t.name(),
                            type_name(other)
                        ))
                    }
                });
            }
            Ok(Some(out))
        }
        Transform::Count => {
            let (items, _) = unwrap_single_array(vals);
            one(Value::from(items.len()))
        }
        Transform::Sum => {
            let (items, _) = unwrap_single_array(vals);
            let mut acc = 0.0;
            for v in &items {
                acc += to_f64(v, "sum", target)?;
            }
            one(num_value(acc))
        }
        Transform::Avg => {
            let (items, _) = unwrap_single_array(vals);
            if items.is_empty() {
                return Ok(None);
            }
            let mut acc = 0.0;
            for v in &items {
                acc += to_f64(v, "avg", target)?;
            }
            one(num_value(acc / items.len() as f64))
        }
        Transform::Min | Transform::Max => {
            let (items, _) = unwrap_single_array(vals);
            if items.is_empty() {
                return Ok(None);
            }
            let name = t.name();
            let mut best = to_f64(&items[0], name, target)?;
            for v in &items[1..] {
                let f = to_f64(v, name, target)?;
                if (t == Transform::Min && f < best) || (t == Transform::Max && f > best) {
                    best = f;
                }
            }
            one(num_value(best))
        }
        Transform::First => {
            let (items, _) = unwrap_single_array(vals);
            Ok(items.into_iter().next().map(|v| vec![v]))
        }
        Transform::Last => {
            let (items, _) = unwrap_single_array(vals);
            Ok(items.into_iter().next_back().map(|v| vec![v]))
        }
        Transform::Join => {
            let (items, _) = unwrap_single_array(vals);
            one(Value::String(
                items
                    .iter()
                    .map(scalar_to_string)
                    .collect::<Vec<_>>()
                    .join(sep),
            ))
        }
    }
}

/// Transforms that collapse the match list to a single value.
fn is_aggregate(t: Transform) -> bool {
    matches!(
        t,
        Transform::Count
            | Transform::Sum
            | Transform::Min
            | Transform::Max
            | Transform::Avg
            | Transform::First
            | Transform::Last
            | Transform::Join
    )
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

/// Per-rule outcome, used by `output = "report"`.
struct RuleReport {
    matches: usize,
    applied: usize,
    skipped_by_when: usize,
    filled_default: usize,
    missing: usize,
}

/// Reshape `json` with `rules`.
///
/// - `each`: optional selector; when set the rules run once per matched item and
///   the result is an array of the per-item outputs.
/// - `mode`: `build` (start from `{}`) or `merge` (start from the input).
/// - `on_missing`: `skip` / `null` / `error` when a selector matches nothing.
/// - `array_mode`: `auto` / `always` / `first` shaping of multi-value matches.
/// - `pretty` + `indent`: output formatting (indent 0–8 spaces).
/// - `output`: `json` (the reshaped document) or `report` (a plain-text rule
///   trace showing what each rule matched).
#[allow(clippy::too_many_arguments)]
pub fn transform(
    json: &str,
    rules: &str,
    each: &str,
    mode: &str,
    on_missing: &str,
    array_mode: &str,
    pretty: bool,
    indent: usize,
    output: &str,
) -> Result<String, String> {
    if json.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input JSON is {} bytes; the maximum is {MAX_INPUT_BYTES}",
            json.len()
        ));
    }
    if rules.len() > MAX_RULES_BYTES {
        return Err(format!(
            "rules are {} bytes; the maximum is {MAX_RULES_BYTES}",
            rules.len()
        ));
    }
    if json.trim().is_empty() {
        return Err("no JSON given: paste the document you want to reshape".to_string());
    }
    let doc: Value =
        serde_json::from_str(json).map_err(|e| format!("invalid JSON input: {e}"))?;
    let rules = parse_rules(rules)?;
    let mode = parse_mode(mode)?;
    let on_missing = parse_on_missing(on_missing)?;
    let array_mode = parse_array_mode(array_mode)?;
    let output = match output.trim().to_ascii_lowercase().as_str() {
        "" | "json" => "json",
        "report" => "report",
        other => {
            return Err(format!(
                "unknown output '{other}' (expected json or report)"
            ))
        }
    };
    if indent > 8 {
        return Err(format!("indent is {indent}; the maximum is 8 spaces"));
    }

    // The item roots the rules run against.
    let each = each.trim();
    let items: Vec<&Value> = if each.is_empty() {
        vec![&doc]
    } else {
        let segs = parse_selector(each).map_err(|e| format!("each: {e}"))?;
        let found = select(&doc, &segs).map_err(|e| format!("each: {e}"))?;
        if found.len() > MAX_ITEMS {
            return Err(format!(
                "'each' matched {} items; the maximum is {MAX_ITEMS}",
                found.len()
            ));
        }
        found
    };

    let mut reports: Vec<RuleReport> = rules
        .iter()
        .map(|_| RuleReport {
            matches: 0,
            applied: 0,
            skipped_by_when: 0,
            filled_default: 0,
            missing: 0,
        })
        .collect();

    let mut outs: Vec<Value> = Vec::with_capacity(items.len());
    for item in &items {
        let mut out = match mode {
            Mode::Build => Value::Object(Map::new()),
            Mode::Merge => (*item).clone(),
        };
        for (i, rule) in rules.iter().enumerate() {
            apply_rule(rule, item, &mut out, on_missing, array_mode, &mut reports[i])?;
        }
        outs.push(out);
    }

    let result = if each.is_empty() {
        outs.into_iter().next().unwrap_or(Value::Null)
    } else {
        Value::Array(outs)
    };

    if output == "report" {
        return Ok(render_report(&rules, &reports, items.len(), each));
    }
    serialize(&result, pretty, indent)
}

fn apply_rule(
    rule: &Rule,
    item: &Value,
    out: &mut Value,
    on_missing: OnMissing,
    array_mode: ArrayMode,
    report: &mut RuleReport,
) -> Result<(), String> {
    if let Some(when) = &rule.when {
        let hits = select(item, when)?;
        if !hits.iter().any(|v| truthy(v)) {
            report.skipped_by_when += 1;
            return Ok(());
        }
    }
    if rule.only_if_absent && !matches!(get_path(out, &rule.tsegs), None | Some(Value::Null)) {
        // `target ?= …` — the path already carries a value, so leave it alone.
        report.skipped_by_when += 1;
        return Ok(());
    }
    if rule.remove {
        if remove_path(out, &rule.tsegs)? {
            report.applied += 1;
        } else {
            report.missing += 1;
        }
        return Ok(());
    }

    let matched: Vec<Value> = match (&rule.value, &rule.source) {
        (Some(v), _) => vec![v.clone()],
        (None, Some(segs)) => select(item, segs)?
            .into_iter()
            .cloned()
            .collect::<Vec<Value>>(),
        (None, None) => {
            return Err(format!(
                "rule for target '{}' needs a 'source' selector or a literal 'value'",
                rule.target
            ))
        }
    };
    report.matches += matched.len();

    let transformed = apply_transform(rule.transform, matched, &rule.separator, &rule.target)?;
    let value = match transformed {
        Some(vals) if !vals.is_empty() => Some(shape(vals, array_mode, rule.transform)),
        _ => {
            // Nothing matched (or the aggregate collapsed to nothing).
            if let Some(d) = &rule.fallback {
                report.filled_default += 1;
                Some(d.clone())
            } else {
                match array_mode {
                    ArrayMode::Always => Some(Value::Array(Vec::new())),
                    _ => match on_missing {
                        OnMissing::Skip => {
                            report.missing += 1;
                            None
                        }
                        OnMissing::Null => Some(Value::Null),
                        OnMissing::Fail => {
                            return Err(format!(
                                "no match for target '{}': selector '{}' matched nothing (set a 'default', or use on_missing=skip/null)",
                                rule.target, rule.source_text
                            ))
                        }
                    },
                }
            }
        }
    };
    if let Some(v) = value {
        set_path(out, &rule.tsegs, v)
            .map_err(|e| format!("target '{}': {e}", rule.target))?;
        report.applied += 1;
    }
    Ok(())
}

/// Shape a non-empty match list per `array_mode`.
fn shape(mut vals: Vec<Value>, array_mode: ArrayMode, t: Transform) -> Value {
    match array_mode {
        ArrayMode::First => vals.remove(0),
        ArrayMode::Always => Value::Array(vals),
        ArrayMode::Auto => {
            if vals.len() == 1 || is_aggregate(t) {
                vals.remove(0)
            } else {
                Value::Array(vals)
            }
        }
    }
}

fn serialize(v: &Value, pretty: bool, indent: usize) -> Result<String, String> {
    if !pretty {
        return serde_json::to_string(v).map_err(|e| format!("could not serialize result: {e}"));
    }
    let pad = vec![b' '; indent.min(8)];
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    v.serialize(&mut ser)
        .map_err(|e| format!("could not serialize result: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("could not serialize result: {e}"))
}

fn render_report(
    rules: &[Rule],
    reports: &[RuleReport],
    items: usize,
    each: &str,
) -> String {
    let mut s = String::new();
    if each.is_empty() {
        s.push_str(&format!("{} rule(s) applied to 1 document\n\n", rules.len()));
    } else {
        s.push_str(&format!(
            "{} rule(s) applied to {items} item(s) selected by '{each}'\n\n",
            rules.len()
        ));
    }
    for (i, (rule, rep)) in rules.iter().zip(reports).enumerate() {
        let from = if rule.remove {
            "remove".to_string()
        } else if rule.value.is_some() {
            "constant".to_string()
        } else {
            rule.source_text.clone()
        };
        s.push_str(&format!("{}. {} <- {}", i + 1, rule.target, from));
        if rule.transform != Transform::None {
            s.push_str(&format!(" | {}", rule.transform.name()));
        }
        if !rule.when_text.is_empty() {
            s.push_str(&format!(" | when {}", rule.when_text));
        }
        s.push('\n');
        s.push_str(&format!(
            "   matched {} value(s), written {} time(s)",
            rep.matches, rep.applied
        ));
        if rep.filled_default > 0 {
            s.push_str(&format!(", default used {}x", rep.filled_default));
        }
        if rep.skipped_by_when > 0 {
            s.push_str(&format!(", skipped by 'when' {}x", rep.skipped_by_when));
        }
        if rep.missing > 0 {
            s.push_str(&format!(", no match {}x", rep.missing));
        }
        s.push('\n');
    }
    let unmatched: Vec<&str> = rules
        .iter()
        .zip(reports)
        .filter(|(r, rep)| !r.remove && rep.applied == 0)
        .map(|(r, _)| r.target.as_str())
        .collect();
    s.push('\n');
    if unmatched.is_empty() {
        s.push_str("every rule wrote at least one value.\n");
    } else {
        s.push_str(&format!(
            "rules that never wrote a value: {}\n",
            unmatched.join(", ")
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(json: &str, rules: &str) -> Result<String, String> {
        transform(json, rules, "", "build", "skip", "auto", false, 2, "json")
    }

    #[test]
    fn shorthand_rules_reshape_a_document() {
        let out = run(
            r#"{"user":{"id":7,"name":"Ada","contact":{"email":"ada@example.com"}}}"#,
            "id = $.user.id\nprofile.email = $.user.contact.email\nsource = \"import\"",
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"id":7,"profile":{"email":"ada@example.com"},"source":"import"}"#
        );
    }

    #[test]
    fn object_form_maps_target_to_selector() {
        let out = run(
            r#"{"a":{"b":1}}"#,
            r#"{"x": "$.a.b", "y": {"source": "$.a.missing", "default": 0}}"#,
        )
        .unwrap();
        assert_eq!(out, r#"{"x":1,"y":0}"#);
    }

    #[test]
    fn wildcard_matches_collapse_to_an_array() {
        let out = run(
            r#"{"items":[{"sku":"a"},{"sku":"b"}]}"#,
            "skus = $.items[*].sku",
        )
        .unwrap();
        assert_eq!(out, r#"{"skus":["a","b"]}"#);
    }

    #[test]
    fn recursive_descent_and_aggregates() {
        let out = run(
            r#"{"cart":{"lines":[{"price":10},{"price":32}],"extra":{"price":8}}}"#,
            r#"[{"target":"total","source":"$..price","transform":"sum"},
                {"target":"lines","source":"$..price","transform":"count"},
                {"target":"biggest","source":"$..price","transform":"max"}]"#,
        )
        .unwrap();
        assert_eq!(out, r#"{"total":50,"lines":3,"biggest":32}"#);
    }

    #[test]
    fn each_runs_the_rules_per_item() {
        let out = transform(
            r#"{"automobiles":[{"maker":"Honda","model":"Jazz","year":2010},
                               {"maker":"Ford","model":"Ka","year":2015}]}"#,
            "text = $.model\nyear = $.year",
            "$.automobiles[*]",
            "build",
            "skip",
            "auto",
            false,
            2,
            "json",
        )
        .unwrap();
        assert_eq!(
            out,
            r#"[{"text":"Jazz","year":2010},{"text":"Ka","year":2015}]"#
        );
    }

    #[test]
    fn merge_mode_overrides_and_removes() {
        let out = transform(
            r#"{"name":"ada","ssn":"000-00-0000","tier":"free"}"#,
            "name = $.name\n-ssn",
            "",
            "merge",
            "skip",
            "auto",
            false,
            2,
            "json",
        )
        .unwrap();
        assert_eq!(out, r#"{"name":"ada","tier":"free"}"#);
    }

    #[test]
    fn transforms_map_over_each_match() {
        let out = run(
            r#"{"tags":["  Alpha ","beta","Alpha"]}"#,
            r#"[{"target":"tags","source":"$.tags[*]","transform":"trim"},
                {"target":"csv","source":"$.tags[*]","transform":"join","separator":" | "},
                {"target":"n","source":"$.tags[*]","transform":"length"}]"#,
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"tags":["Alpha","beta","Alpha"],"csv":"  Alpha  | beta | Alpha","n":[8,4,5]}"#
        );
    }

    #[test]
    fn list_transforms_reshape_a_single_array_match() {
        let out = run(
            r#"{"scores":[3,1,2],"nested":[[1,2],[3]],"tags":["a","b","a"]}"#,
            r#"[{"target":"sorted","source":"$.scores","transform":"sort"},
                {"target":"backwards","source":"$.scores","transform":"reverse"},
                {"target":"flat","source":"$.nested","transform":"flatten"},
                {"target":"uniq","source":"$.tags","transform":"unique"},
                {"target":"total","source":"$.scores","transform":"sum"},
                {"target":"csv","source":"$.tags","transform":"join","separator":"-"}]"#,
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"sorted":[1,2,3],"backwards":[2,1,3],"flat":[1,2,3],"uniq":["a","b"],"total":6,"csv":"a-b-a"}"#
        );
    }

    #[test]
    fn keys_and_values_expand_an_object() {
        let out = run(
            r#"{"prices":{"eur":10,"usd":12}}"#,
            r#"[{"target":"currencies","source":"$.prices","transform":"keys"},
                {"target":"amounts","source":"$.prices","transform":"values"}]"#,
        )
        .unwrap();
        assert_eq!(out, r#"{"currencies":["eur","usd"],"amounts":[10,12]}"#);
    }

    #[test]
    fn keys_on_a_scalar_errors() {
        let err = run(
            r#"{"a":1}"#,
            r#"[{"target":"k","source":"$.a","transform":"keys"}]"#,
        )
        .unwrap_err();
        assert!(err.contains("expected an object or array"), "got: {err}");
    }

    #[test]
    fn optional_target_only_fills_a_gap() {
        // `?=` writes only where the merged document has nothing yet.
        let out = transform(
            r#"{"tier":"pro","region":null}"#,
            "tier ?= \"free\"\nregion ?= \"eu\"\ncountry ?= \"NL\"",
            "",
            "merge",
            "skip",
            "auto",
            false,
            2,
            "json",
        )
        .unwrap();
        assert_eq!(out, r#"{"tier":"pro","region":"eu","country":"NL"}"#);
    }

    #[test]
    fn when_gates_a_rule() {
        let rules = r#"[{"target":"discount","value":0.1,"when":"$.vip"}]"#;
        let on = run(r#"{"vip":true}"#, rules).unwrap();
        let off = run(r#"{"vip":false}"#, rules).unwrap();
        assert_eq!(on, r#"{"discount":0.1}"#);
        assert_eq!(off, "{}");
    }

    #[test]
    fn array_mode_always_and_first() {
        let json = r#"{"items":[{"sku":"a"},{"sku":"b"}]}"#;
        let always = transform(
            json, "skus = $.items[*].sku\nnone = $.nope", "", "build", "skip", "always", false, 2,
            "json",
        )
        .unwrap();
        assert_eq!(always, r#"{"skus":["a","b"],"none":[]}"#);
        let first = transform(
            json, "sku = $.items[*].sku", "", "build", "skip", "first", false, 2, "json",
        )
        .unwrap();
        assert_eq!(first, r#"{"sku":"a"}"#);
    }

    #[test]
    fn on_missing_null_and_error() {
        let nulled = transform(
            r#"{"a":1}"#, "b = $.b", "", "build", "null", "auto", false, 2, "json",
        )
        .unwrap();
        assert_eq!(nulled, r#"{"b":null}"#);
        let err = transform(
            r#"{"a":1}"#, "b = $.b", "", "build", "error", "auto", false, 2, "json",
        )
        .unwrap_err();
        assert!(err.contains("no match for target 'b'"), "got: {err}");
    }

    #[test]
    fn nested_and_indexed_targets() {
        let out = run(
            r#"{"a":1,"b":2}"#,
            "out.list[0] = $.a\nout.list[] = $.b\nout[\"odd key\"] = \"ok\"",
        )
        .unwrap();
        assert_eq!(out, r#"{"out":{"list":[1,2],"odd key":"ok"}}"#);
    }

    #[test]
    fn pretty_output_honors_indent() {
        let out = transform(
            r#"{"a":1}"#, "x = $.a", "", "build", "skip", "auto", true, 4, "json",
        )
        .unwrap();
        assert_eq!(out, "{\n    \"x\": 1\n}");
    }

    #[test]
    fn report_lists_every_rule() {
        let out = transform(
            r#"{"a":1}"#,
            "x = $.a\ny = $.nope",
            "",
            "build",
            "skip",
            "auto",
            true,
            2,
            "report",
        )
        .unwrap();
        assert!(out.contains("1. x <- $.a"), "got: {out}");
        assert!(out.contains("rules that never wrote a value: y"), "got: {out}");
    }

    // --- error paths -------------------------------------------------------

    #[test]
    fn invalid_json_input_errors() {
        let err = run("{not json", "x = $.a").unwrap_err();
        assert!(err.starts_with("invalid JSON input:"), "got: {err}");
    }

    #[test]
    fn a_line_without_an_equals_errors() {
        let err = run(r#"{"a":1}"#, "just some text").unwrap_err();
        assert!(err.contains("expected 'target = selector'"), "got: {err}");
    }

    #[test]
    fn unknown_rule_field_errors() {
        let err = run(r#"{"a":1}"#, r#"[{"target":"x","src":"$.a"}]"#).unwrap_err();
        assert!(err.contains("unknown field 'src'"), "got: {err}");
    }

    #[test]
    fn unknown_transform_errors() {
        let err = run(r#"{"a":1}"#, r#"[{"target":"x","source":"$.a","transform":"shout"}]"#)
            .unwrap_err();
        assert!(err.contains("unknown transform 'shout'"), "got: {err}");
    }

    #[test]
    fn bad_selector_errors() {
        let err = run(r#"{"a":1}"#, "x = $.a[").unwrap_err();
        assert!(err.contains("unterminated '['"), "got: {err}");
    }

    #[test]
    fn sum_over_non_numeric_errors() {
        let err = run(
            r#"{"a":["x"]}"#,
            r#"[{"target":"t","source":"$.a[*]","transform":"sum"}]"#,
        )
        .unwrap_err();
        assert!(err.contains("expected a number"), "got: {err}");
    }

    #[test]
    fn empty_rules_error() {
        let err = run(r#"{"a":1}"#, "   ").unwrap_err();
        assert!(err.contains("no rules given"), "got: {err}");
    }

    #[test]
    fn source_and_value_together_error() {
        let err = run(r#"{"a":1}"#, r#"[{"target":"x","source":"$.a","value":2}]"#).unwrap_err();
        assert!(err.contains("not both"), "got: {err}");
    }

    #[test]
    fn target_index_cap_is_enforced() {
        let err = run(r#"{"a":1}"#, "x[999999] = $.a").unwrap_err();
        assert!(err.contains("element cap"), "got: {err}");
    }

    #[test]
    fn indent_above_the_cap_errors() {
        let err = transform(
            r#"{"a":1}"#, "x = $.a", "", "build", "skip", "auto", true, 9, "json",
        )
        .unwrap_err();
        assert!(err.contains("maximum is 8"), "got: {err}");
    }
}
