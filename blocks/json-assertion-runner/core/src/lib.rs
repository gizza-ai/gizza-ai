//! gizza-ai/json-assertion-runner core — run a list of declarative assertions against a
//! JSON payload and report pass/fail.
//!
//! Each assertion is a `<jsonpath> <operator> [expected]` rule. Paths are RFC 9535
//! JSONPath, evaluated with `serde_json_path`; regular expressions use the pure-Rust
//! `regex` crate. Nothing here touches wafer, wasm-bindgen, WASI or the network, so the
//! same code runs in the chat block, the `gizza` CLI, and the browser page.
//!
//! Two rule syntaxes are accepted:
//!
//! * a **line DSL** — one assertion per line, `#` starts a comment line:
//!   `$.status equals ok`, `$.items[*].price lte 100`, `$.id matches ^u-\d+$`
//! * a **JSON array** of rule objects:
//!   `[{"name":"status ok","path":"$.status","op":"equals","value":"ok"}]`
//!
//! Multi-match semantics: for value-comparing operators every matched node must satisfy
//! the rule, and a path that matches nothing fails. `exists`/`not_exists` assert on the
//! match count itself, `count` compares how many nodes the path selected (so
//! `$.items[*] count 2` — one node per item), and `length` measures the size of each
//! matched node (`$.items length 2` — one array of two).

use regex::Regex;
use serde_json::{Map, Value};
use serde_json_path::JsonPath;

/// Hard cap on how many assertions one run may contain.
pub const MAX_ASSERTIONS: usize = 200;

// ---------------------------------------------------------------------------
// Operators
// ---------------------------------------------------------------------------

/// The assertion operators the runner understands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Exists,
    NotExists,
    Equals,
    NotEquals,
    Type,
    Range,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
    NotContains,
    Matches,
    Length,
    Count,
    Empty,
    NotEmpty,
}

impl Op {
    /// Parse an operator name. Common aliases and symbol forms are accepted so rules
    /// copied from other assertion vocabularies keep working.
    pub fn parse(s: &str) -> Option<Op> {
        let norm: String = s
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| *c != '_' && *c != '-')
            .collect();
        Some(match norm.as_str() {
            "exists" | "present" | "haspath" => Op::Exists,
            "notexists" | "notpresent" | "absent" | "missing" => Op::NotExists,
            "equals" | "equal" | "eq" | "is" | "=" | "==" => Op::Equals,
            "notequals" | "notequal" | "ne" | "isnot" | "!=" | "<>" => Op::NotEquals,
            "type" | "istype" | "hastype" => Op::Type,
            "range" | "between" | "inrange" => Op::Range,
            "gt" | "greaterthan" | ">" => Op::Gt,
            "gte" | "atleast" | "min" | ">=" => Op::Gte,
            "lt" | "lessthan" | "<" => Op::Lt,
            "lte" | "atmost" | "max" | "<=" => Op::Lte,
            "contains" | "includes" | "hasitem" => Op::Contains,
            "notcontains" | "excludes" => Op::NotContains,
            "matches" | "match" | "regex" | "regexp" => Op::Matches,
            "length" | "len" | "size" | "haslength" => Op::Length,
            "count" | "nodecount" | "matchcount" => Op::Count,
            "empty" | "isempty" => Op::Empty,
            "notempty" | "isnotempty" | "nonempty" => Op::NotEmpty,
            _ => return None,
        })
    }

    /// The canonical spelling used in reports (and in the JSON report's `op` field).
    pub fn canonical(self) -> &'static str {
        match self {
            Op::Exists => "exists",
            Op::NotExists => "not_exists",
            Op::Equals => "equals",
            Op::NotEquals => "not_equals",
            Op::Type => "type",
            Op::Range => "range",
            Op::Gt => "gt",
            Op::Gte => "gte",
            Op::Lt => "lt",
            Op::Lte => "lte",
            Op::Contains => "contains",
            Op::NotContains => "not_contains",
            Op::Matches => "matches",
            Op::Length => "length",
            Op::Count => "count",
            Op::Empty => "empty",
            Op::NotEmpty => "not_empty",
        }
    }

    /// Whether the operator needs an expected value after it.
    fn needs_expected(self) -> bool {
        !matches!(self, Op::Exists | Op::NotExists | Op::Empty | Op::NotEmpty)
    }
}

/// Every operator name the runner accepts, canonical spellings only — used by the
/// descriptor copy and the error message for an unknown operator.
pub const OPERATORS: [&str; 17] = [
    "exists",
    "not_exists",
    "equals",
    "not_equals",
    "type",
    "range",
    "gt",
    "gte",
    "lt",
    "lte",
    "contains",
    "not_contains",
    "matches",
    "length",
    "count",
    "empty",
    "not_empty",
];

const TYPE_NAMES: [&str; 7] = [
    "string", "number", "integer", "boolean", "object", "array", "null",
];

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

/// One parsed assertion.
#[derive(Clone, Debug)]
pub struct Rule {
    /// Optional human label (JSON rules only).
    pub name: Option<String>,
    /// The JSONPath expression the rule addresses.
    pub path: String,
    pub op: Op,
    /// The expected value, if the operator takes one. `range` stores `[min, max]`.
    pub expected: Option<Value>,
}

impl Rule {
    /// `$.items[*].price lte 100` — the canonical one-line rendering used in reports,
    /// identical whether the rule came from the DSL or the JSON array.
    pub fn display(&self) -> String {
        let mut s = String::new();
        if let Some(n) = &self.name {
            s.push('[');
            s.push_str(n);
            s.push_str("] ");
        }
        s.push_str(&self.path);
        s.push(' ');
        s.push_str(self.op.canonical());
        if let Some(v) = &self.expected {
            s.push(' ');
            s.push_str(&expected_display(self.op, v));
        }
        s
    }
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn compact(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "<unserializable>".into())
}

fn expected_display(op: Op, v: &Value) -> String {
    match op {
        Op::Range => match range_bounds(v) {
            Some((lo, hi)) => format!("{}..{}", fmt_num(lo), fmt_num(hi)),
            None => compact(v),
        },
        Op::Type => v
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| compact(v)),
        Op::Matches => v
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| compact(v)),
        _ => compact(v),
    }
}

fn range_bounds(v: &Value) -> Option<(f64, f64)> {
    let a = v.as_array()?;
    if a.len() != 2 {
        return None;
    }
    Some((a[0].as_f64()?, a[1].as_f64()?))
}

fn num_value(n: f64) -> Result<Value, String> {
    serde_json::Number::from_f64(n)
        .map(Value::Number)
        .ok_or_else(|| format!("{n} is not a finite number"))
}

/// Interpret a bare DSL token as JSON when it parses as JSON, else as a plain string.
/// `ok` → `"ok"`, `12.5` → `12.5`, `true` → `true`, `"12"` → `"12"`.
fn token_to_value(tok: &str) -> Value {
    match serde_json::from_str::<Value>(tok) {
        Ok(v) => v,
        Err(_) => Value::String(tok.to_string()),
    }
}

fn parse_range_token(tok: &str, line_no: usize) -> Result<Value, String> {
    let (lo, hi) = tok
        .split_once("..")
        .ok_or_else(|| err_at(line_no, "range expects `min..max`, e.g. `0..100`"))?;
    let lo: f64 = lo
        .trim()
        .parse()
        .map_err(|_| err_at(line_no, "range lower bound must be a number"))?;
    let hi: f64 = hi
        .trim()
        .parse()
        .map_err(|_| err_at(line_no, "range upper bound must be a number"))?;
    if hi < lo {
        return Err(err_at(
            line_no,
            "range upper bound is below the lower bound",
        ));
    }
    Ok(Value::Array(vec![num_value(lo)?, num_value(hi)?]))
}

fn err_at(line_no: usize, msg: &str) -> String {
    if line_no == 0 {
        msg.to_string()
    } else {
        format!("assertion line {line_no}: {msg}")
    }
}

fn unknown_op(name: &str, line_no: usize) -> String {
    err_at(
        line_no,
        &format!(
            "unknown operator '{name}'. Supported operators: {}",
            OPERATORS.join(", ")
        ),
    )
}

fn validate_expected(rule: &Rule, line_no: usize) -> Result<(), String> {
    match (rule.op, rule.expected.as_ref()) {
        (op, None) if op.needs_expected() => Err(err_at(
            line_no,
            &format!("operator '{}' needs an expected value", op.canonical()),
        )),
        (Op::Type, Some(v)) => {
            let t = v.as_str().unwrap_or("");
            if TYPE_NAMES.contains(&t) {
                Ok(())
            } else {
                Err(err_at(
                    line_no,
                    &format!("type expects one of {}, got '{}'", TYPE_NAMES.join(", "), t),
                ))
            }
        }
        (Op::Range, Some(v)) => {
            if range_bounds(v).is_some() {
                Ok(())
            } else {
                Err(err_at(
                    line_no,
                    "range expects two numeric bounds, e.g. `0..100`",
                ))
            }
        }
        (Op::Gt | Op::Gte | Op::Lt | Op::Lte, Some(v)) if v.as_f64().is_none() => Err(err_at(
            line_no,
            &format!("operator '{}' expects a number", rule.op.canonical()),
        )),
        (Op::Length | Op::Count, Some(v)) => match v.as_u64() {
            Some(_) => Ok(()),
            None => Err(err_at(
                line_no,
                &format!(
                    "operator '{}' expects a whole number (0 or more)",
                    rule.op.canonical()
                ),
            )),
        },
        (Op::Matches, Some(v)) => {
            let pat = v.as_str().unwrap_or("");
            Regex::new(pat)
                .map(|_| ())
                .map_err(|e| err_at(line_no, &format!("invalid regular expression: {e}")))
        }
        _ => Ok(()),
    }
}

/// Parse the line DSL: one `<path> <op> [expected]` assertion per line, blank lines and
/// `#` comment lines skipped.
fn parse_dsl(src: &str) -> Result<Vec<Rule>, String> {
    let mut rules = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.splitn(3, char::is_whitespace);
        let path = it.next().unwrap_or("").trim().to_string();
        let op_tok = it.next().unwrap_or("").trim();
        let rest = it.next().unwrap_or("").trim();
        if path.is_empty() || op_tok.is_empty() {
            return Err(err_at(
                line_no,
                "expected `<jsonpath> <operator> [expected]`, e.g. `$.status equals ok`",
            ));
        }
        let op = Op::parse(op_tok).ok_or_else(|| unknown_op(op_tok, line_no))?;
        let expected = if !op.needs_expected() {
            if !rest.is_empty() {
                return Err(err_at(
                    line_no,
                    &format!(
                        "operator '{}' takes no expected value (got '{rest}')",
                        op.canonical()
                    ),
                ));
            }
            None
        } else if rest.is_empty() {
            None
        } else if op == Op::Range {
            Some(parse_range_token(rest, line_no)?)
        } else if op == Op::Type || op == Op::Matches {
            // Never JSON-decode these: a type name is a bare word and a regex is not JSON.
            // A quoted form is still honoured so `matches "^a.*"` behaves as expected.
            Some(match serde_json::from_str::<Value>(rest) {
                Ok(Value::String(s)) => Value::String(s),
                _ => Value::String(rest.to_string()),
            })
        } else {
            Some(token_to_value(rest))
        };
        let rule = Rule {
            name: None,
            path,
            op,
            expected,
        };
        validate_expected(&rule, line_no)?;
        rules.push(rule);
    }
    Ok(rules)
}

fn obj_str<'a>(o: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|k| o.get(*k))
}

/// Parse the JSON rule array (a single rule object is also accepted).
fn parse_json_rules(src: &str) -> Result<Vec<Rule>, String> {
    let doc: Value =
        serde_json::from_str(src).map_err(|e| format!("assertions are not valid JSON: {e}"))?;
    let items: Vec<Value> = match doc {
        Value::Array(a) => a,
        Value::Object(_) => vec![doc],
        other => {
            return Err(format!(
                "assertions must be a JSON array of rule objects, got {}",
                type_name(&other)
            ))
        }
    };
    let mut rules = Vec::with_capacity(items.len());
    for (i, item) in items.into_iter().enumerate() {
        let idx = i + 1;
        let at = |msg: &str| format!("assertion {idx}: {msg}");
        let o = item
            .as_object()
            .ok_or_else(|| at("each assertion must be a JSON object"))?;
        let path = obj_str(o, &["path", "jsonpath", "json_path"])
            .and_then(|v| v.as_str())
            .ok_or_else(|| at("missing a string \"path\" (the JSONPath expression)"))?
            .trim()
            .to_string();
        if path.is_empty() {
            return Err(at("\"path\" is empty"));
        }
        let op_name = obj_str(o, &["op", "operator", "assert"])
            .and_then(|v| v.as_str())
            .ok_or_else(|| at("missing a string \"op\" (the operator)"))?;
        let op = Op::parse(op_name).ok_or_else(|| {
            at(&format!(
                "unknown operator '{op_name}'. Supported operators: {}",
                OPERATORS.join(", ")
            ))
        })?;
        let name = obj_str(o, &["name", "label", "description"])
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let expected = if op == Op::Range {
            match (o.get("min"), o.get("max")) {
                (Some(lo), Some(hi)) => {
                    let lo = lo.as_f64().ok_or_else(|| at("\"min\" must be a number"))?;
                    let hi = hi.as_f64().ok_or_else(|| at("\"max\" must be a number"))?;
                    if hi < lo {
                        return Err(at("\"max\" is below \"min\""));
                    }
                    Some(Value::Array(vec![num_value(lo)?, num_value(hi)?]))
                }
                _ => match obj_str(o, &["value", "expected"]) {
                    Some(Value::String(s)) => Some(parse_range_token(s, 0).map_err(|e| at(&e))?),
                    Some(v @ Value::Array(_)) => Some(v.clone()),
                    _ => {
                        return Err(at(
                            "range needs \"min\" and \"max\", or \"value\" as [min, max] or \"min..max\"",
                        ))
                    }
                },
            }
        } else {
            obj_str(o, &["value", "expected"]).cloned()
        };

        let rule = Rule {
            name,
            path,
            op,
            expected,
        };
        validate_expected(&rule, 0).map_err(|e| at(&e))?;
        rules.push(rule);
    }
    Ok(rules)
}

/// Parse the assertion source in the requested format (`auto`, `dsl`, or `json`).
pub fn parse_rules(src: &str, rules_format: &str) -> Result<Vec<Rule>, String> {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return Err("no assertions provided — add at least one rule".into());
    }
    let rules = match rules_format.trim() {
        "" | "auto" => {
            if trimmed.starts_with('[') || trimmed.starts_with('{') {
                parse_json_rules(trimmed)?
            } else {
                parse_dsl(trimmed)?
            }
        }
        "json" => parse_json_rules(trimmed)?,
        "dsl" => parse_dsl(trimmed)?,
        other => {
            return Err(format!(
                "unknown rules_format '{other}' — use auto, dsl, or json"
            ))
        }
    };
    if rules.is_empty() {
        return Err("no assertions provided — every line was blank or a comment".into());
    }
    if rules.len() > MAX_ASSERTIONS {
        return Err(format!(
            "too many assertions: {} (maximum {MAX_ASSERTIONS})",
            rules.len()
        ));
    }
    Ok(rules)
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn values_equal(a: &Value, b: &Value, case_sensitive: bool) -> bool {
    match (a, b) {
        (Value::Number(_), Value::Number(_)) => match (a.as_f64(), b.as_f64()) {
            (Some(x), Some(y)) => x == y,
            _ => a == b,
        },
        (Value::String(x), Value::String(y)) if !case_sensitive => x.eq_ignore_ascii_case(y),
        (Value::Array(x), Value::Array(y)) if !case_sensitive => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(p, q)| values_equal(p, q, case_sensitive))
        }
        _ => a == b,
    }
}

/// Length of a value for `length`/`empty`: array items, object keys, or string chars.
fn value_len(v: &Value) -> Option<usize> {
    match v {
        Value::Array(a) => Some(a.len()),
        Value::Object(o) => Some(o.len()),
        Value::String(s) => Some(s.chars().count()),
        _ => None,
    }
}

/// The outcome of one assertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    Skip,
}

impl Status {
    fn label(&self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Skip => "SKIP",
        }
    }
}

/// One assertion's result.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub status: Status,
    /// Number of nodes the JSONPath matched.
    pub matches: usize,
    /// Why it failed (or was skipped).
    pub message: Option<String>,
    /// Normalized path of the node that failed, when a specific node is at fault.
    pub location: Option<String>,
    /// The offending node's value, when a specific node is at fault.
    pub actual: Option<Value>,
}

fn pass(matches: usize) -> Outcome {
    Outcome {
        status: Status::Pass,
        matches,
        message: None,
        location: None,
        actual: None,
    }
}

fn fail(matches: usize, message: String) -> Outcome {
    Outcome {
        status: Status::Fail,
        matches,
        message: Some(message),
        location: None,
        actual: None,
    }
}

fn fail_at(matches: usize, loc: String, actual: &Value, message: String) -> Outcome {
    Outcome {
        status: Status::Fail,
        matches,
        message: Some(message),
        location: Some(loc),
        actual: Some(actual.clone()),
    }
}

/// Check one matched node against the rule. `Ok(())` passes; `Err(msg)` explains the miss.
fn check_node(rule: &Rule, node: &Value, case_sensitive: bool) -> Result<(), String> {
    let expected = rule.expected.as_ref();
    match rule.op {
        Op::Exists | Op::NotExists | Op::Count => Ok(()),
        Op::Equals => {
            let e = expected.unwrap();
            if values_equal(node, e, case_sensitive) {
                Ok(())
            } else {
                Err(format!("expected {}", compact(e)))
            }
        }
        Op::NotEquals => {
            let e = expected.unwrap();
            if values_equal(node, e, case_sensitive) {
                Err(format!("expected any value other than {}", compact(e)))
            } else {
                Ok(())
            }
        }
        Op::Type => {
            let want = expected.and_then(|v| v.as_str()).unwrap_or("");
            let ok = match want {
                "integer" => {
                    node.is_i64()
                        || node.is_u64()
                        || node.as_f64().is_some_and(|f| f.fract() == 0.0)
                }
                other => type_name(node) == other,
            };
            if ok {
                Ok(())
            } else {
                Err(format!("expected type {want}"))
            }
        }
        Op::Range | Op::Gt | Op::Gte | Op::Lt | Op::Lte => {
            let Some(n) = node.as_f64() else {
                return Err(format!(
                    "expected a number to compare, got {}",
                    type_name(node)
                ));
            };
            let e = expected.unwrap();
            if rule.op == Op::Range {
                let (lo, hi) = range_bounds(e).unwrap();
                if n >= lo && n <= hi {
                    Ok(())
                } else {
                    Err(format!("expected within {}..{}", fmt_num(lo), fmt_num(hi)))
                }
            } else {
                let t = e.as_f64().unwrap();
                let (ok, sym) = match rule.op {
                    Op::Gt => (n > t, ">"),
                    Op::Gte => (n >= t, ">="),
                    Op::Lt => (n < t, "<"),
                    _ => (n <= t, "<="),
                };
                if ok {
                    Ok(())
                } else {
                    Err(format!("expected {sym} {}", fmt_num(t)))
                }
            }
        }
        Op::Contains | Op::NotContains => {
            let e = expected.unwrap();
            let found = match node {
                Value::Array(items) => items.iter().any(|it| values_equal(it, e, case_sensitive)),
                Value::String(s) => {
                    let needle = match e {
                        Value::String(x) => x.clone(),
                        other => compact(other),
                    };
                    if case_sensitive {
                        s.contains(&needle)
                    } else {
                        s.to_lowercase().contains(&needle.to_lowercase())
                    }
                }
                Value::Object(o) => match e.as_str() {
                    Some(k) if case_sensitive => o.contains_key(k),
                    Some(k) => o.keys().any(|kk| kk.eq_ignore_ascii_case(k)),
                    None => {
                        return Err(
                            "contains on an object expects a string key as the expected value"
                                .into(),
                        )
                    }
                },
                other => {
                    return Err(format!(
                        "expected an array, string, or object to search, got {}",
                        type_name(other)
                    ))
                }
            };
            match (rule.op, found) {
                (Op::Contains, true) | (Op::NotContains, false) => Ok(()),
                (Op::Contains, false) => Err(format!("expected to contain {}", compact(e))),
                _ => Err(format!("expected not to contain {}", compact(e))),
            }
        }
        Op::Matches => {
            let pat = expected.and_then(|v| v.as_str()).unwrap_or("");
            let re = if case_sensitive {
                Regex::new(pat)
            } else {
                Regex::new(&format!("(?i){pat}"))
            }
            .map_err(|e| format!("invalid regular expression: {e}"))?;
            let hay = match node {
                Value::String(s) => s.clone(),
                Value::Number(_) | Value::Bool(_) => compact(node),
                other => {
                    return Err(format!(
                        "expected a string, number, or boolean to match, got {}",
                        type_name(other)
                    ))
                }
            };
            if re.is_match(&hay) {
                Ok(())
            } else {
                Err(format!("expected to match /{pat}/"))
            }
        }
        Op::Length => {
            let want = expected.and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            match value_len(node) {
                Some(len) if len == want => Ok(()),
                Some(len) => Err(format!("expected length {want}, got {len}")),
                None => Err(format!(
                    "expected an array, object, or string to measure, got {}",
                    type_name(node)
                )),
            }
        }
        Op::Empty | Op::NotEmpty => {
            let is_empty = match node {
                Value::Null => true,
                other => match value_len(other) {
                    Some(len) => len == 0,
                    None => {
                        return Err(format!(
                            "expected an array, object, string, or null, got {}",
                            type_name(other)
                        ))
                    }
                },
            };
            match (rule.op, is_empty) {
                (Op::Empty, true) | (Op::NotEmpty, false) => Ok(()),
                (Op::Empty, false) => Err("expected an empty value".into()),
                _ => Err("expected a non-empty value".into()),
            }
        }
    }
}

fn evaluate(rule: &Rule, doc: &Value, case_sensitive: bool) -> Result<Outcome, String> {
    let jp = JsonPath::parse(&rule.path)
        .map_err(|e| format!("invalid JSONPath '{}': {e}", rule.path))?;
    let nodes = jp.query_located(doc);
    let count = nodes.len();

    match rule.op {
        Op::Exists => {
            return Ok(if count > 0 {
                pass(count)
            } else {
                fail(count, "path matched no nodes".into())
            })
        }
        Op::NotExists => {
            return Ok(match nodes.iter().next() {
                None => pass(0),
                Some(n) => fail_at(
                    count,
                    n.location().to_string(),
                    n.node(),
                    "expected no match".into(),
                ),
            })
        }
        Op::Count => {
            let want = rule.expected.as_ref().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if count == want {
                return Ok(pass(count));
            }
            // `$.items count 2` selects ONE array, not two nodes — the classic mix-up
            // between "how many nodes matched" and "how big is the matched node".
            let hint = match nodes.iter().next().map(|n| n.node()) {
                Some(node) if count == 1 && value_len(node).is_some() => format!(
                    " — the path selected a single {}; use `length` to assert its size",
                    type_name(node)
                ),
                _ => String::new(),
            };
            return Ok(fail(
                count,
                format!("expected count {want}, got {count} matched node(s){hint}"),
            ));
        }
        _ => {}
    }

    if count == 0 {
        return Ok(fail(0, "path matched no nodes".into()));
    }
    for n in nodes.iter() {
        if let Err(msg) = check_node(rule, n.node(), case_sensitive) {
            return Ok(fail_at(count, n.location().to_string(), n.node(), msg));
        }
    }
    Ok(pass(count))
}

/// One assertion plus its outcome.
#[derive(Clone, Debug)]
pub struct Checked {
    pub rule: Rule,
    pub outcome: Outcome,
}

/// The whole run.
#[derive(Clone, Debug)]
pub struct Report {
    pub passed: bool,
    pub results: Vec<Checked>,
}

impl Report {
    pub fn passed_count(&self) -> usize {
        self.count_of(Status::Pass)
    }
    pub fn failed_count(&self) -> usize {
        self.count_of(Status::Fail)
    }
    pub fn skipped_count(&self) -> usize {
        self.count_of(Status::Skip)
    }
    fn count_of(&self, s: Status) -> usize {
        self.results
            .iter()
            .filter(|c| c.outcome.status == s)
            .count()
    }
}

/// Parse the payload + rules and run every assertion.
pub fn run_assertions(
    payload: &str,
    assertions: &str,
    rules_format: &str,
    case_sensitive: bool,
    stop_on_first_failure: bool,
) -> Result<Report, String> {
    if payload.trim().is_empty() {
        return Err("the JSON payload is empty".into());
    }
    let doc: Value =
        serde_json::from_str(payload).map_err(|e| format!("invalid JSON payload: {e}"))?;
    let rules = parse_rules(assertions, rules_format)?;

    let mut results = Vec::with_capacity(rules.len());
    let mut stopped = false;
    for rule in rules {
        if stopped {
            results.push(Checked {
                rule,
                outcome: Outcome {
                    status: Status::Skip,
                    matches: 0,
                    message: Some("skipped after an earlier failure".into()),
                    location: None,
                    actual: None,
                },
            });
            continue;
        }
        let outcome = evaluate(&rule, &doc, case_sensitive)?;
        if outcome.status == Status::Fail && stop_on_first_failure {
            stopped = true;
        }
        results.push(Checked { rule, outcome });
    }
    let passed = results.iter().all(|c| c.outcome.status == Status::Pass);
    Ok(Report { passed, results })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Human-readable pass/fail report.
pub fn text_report(report: &Report) -> String {
    let total = report.results.len();
    let passed = report.passed_count();
    let failed = report.failed_count();
    let skipped = report.skipped_count();

    let mut out = String::new();
    out.push_str(if report.passed { "PASS" } else { "FAIL" });
    out.push_str(&format!(
        " — {passed} of {total} assertions passed, {failed} failed"
    ));
    if skipped > 0 {
        out.push_str(&format!(", {skipped} skipped"));
    }
    out.push_str("\n\n");

    for (i, c) in report.results.iter().enumerate() {
        out.push_str(&format!(
            "{}  #{}  {}\n",
            c.outcome.status.label(),
            i + 1,
            c.rule.display()
        ));
        if c.outcome.status == Status::Pass {
            continue;
        }
        if let Some(msg) = &c.outcome.message {
            let detail = match (&c.outcome.location, &c.outcome.actual) {
                (Some(loc), Some(actual)) => {
                    format!("at {loc}: {msg}, got {}", compact(actual))
                }
                _ => msg.clone(),
            };
            out.push_str(&format!("          {detail}\n"));
        }
    }
    out
}

/// Machine-readable report object.
pub fn json_report(report: &Report) -> Value {
    let assertions: Vec<Value> = report
        .results
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut o = Map::new();
            o.insert("index".into(), Value::from(i as u64 + 1));
            o.insert(
                "name".into(),
                c.rule
                    .name
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            o.insert("path".into(), Value::String(c.rule.path.clone()));
            o.insert("op".into(), Value::String(c.rule.op.canonical().into()));
            o.insert(
                "expected".into(),
                c.rule.expected.clone().unwrap_or(Value::Null),
            );
            o.insert(
                "status".into(),
                Value::String(c.outcome.status.label().to_ascii_lowercase()),
            );
            o.insert("matches".into(), Value::from(c.outcome.matches as u64));
            o.insert(
                "message".into(),
                c.outcome
                    .message
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            o.insert(
                "location".into(),
                c.outcome
                    .location
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            o.insert(
                "actual".into(),
                c.outcome.actual.clone().unwrap_or(Value::Null),
            );
            Value::Object(o)
        })
        .collect();

    let mut root = Map::new();
    root.insert("passed".into(), Value::Bool(report.passed));
    root.insert("total".into(), Value::from(report.results.len() as u64));
    root.insert(
        "passed_count".into(),
        Value::from(report.passed_count() as u64),
    );
    root.insert(
        "failed_count".into(),
        Value::from(report.failed_count() as u64),
    );
    root.insert(
        "skipped_count".into(),
        Value::from(report.skipped_count() as u64),
    );
    root.insert("assertions".into(), Value::Array(assertions));
    Value::Object(root)
}

/// Run the assertions and render the report as `text` (default) or `json`.
#[allow(clippy::too_many_arguments)]
pub fn render(
    payload: &str,
    assertions: &str,
    rules_format: &str,
    case_sensitive: bool,
    stop_on_first_failure: bool,
    output: &str,
) -> Result<String, String> {
    let report = run_assertions(
        payload,
        assertions,
        rules_format,
        case_sensitive,
        stop_on_first_failure,
    )?;
    match output.trim() {
        "" | "text" => Ok(text_report(&report)),
        "json" => serde_json::to_string_pretty(&json_report(&report))
            .map_err(|e| format!("serialize error: {e}")),
        other => Err(format!("unknown output '{other}' — use text or json")),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = r#"{
        "status": "ok",
        "user": {"id": "u-42", "name": "Ada", "roles": ["admin", "dev"], "nickname": null},
        "items": [
            {"sku": "A1", "price": 9.5},
            {"sku": "B2", "price": 250}
        ],
        "meta": {}
    }"#;

    fn run(rules: &str) -> Report {
        run_assertions(PAYLOAD, rules, "auto", true, false).expect("run")
    }

    #[test]
    fn happy_path_all_assertions_pass() {
        let r = run("# a comment line\n\
             $.status equals ok\n\
             $.user.id matches ^u-\\d+$\n\
             $.user.roles contains admin\n\
             $.user.roles length 2\n\
             $.items length 2\n\
             $.items[*] count 2\n\
             $.missing count 0\n\
             $.items[*].price gt 0\n\
             $.items[0].price range 0..100\n\
             $.user.name type string\n\
             $.user.nickname empty\n\
             $.meta empty\n\
             $.user.roles not_empty\n\
             $.missing not_exists\n\
             $.status exists\n\
             $.status not_equals nope\n\
             $.user.roles not_contains guest\n\
             $.items[1].price lte 250\n\
             $.items[0].price lt 10\n\
             $.items[1].price gte 250\n");
        assert!(r.passed, "{}", text_report(&r));
        assert_eq!(r.passed_count(), 20);
        assert_eq!(r.failed_count(), 0);
    }

    #[test]
    fn count_counts_matched_nodes_and_hints_at_length() {
        // One array node matched — NOT two, however tempting `$.items count 2` looks.
        let r = run("$.items count 2");
        assert_eq!(r.results[0].outcome.status, Status::Fail);
        assert_eq!(
            r.results[0].outcome.message.as_deref(),
            Some("expected count 2, got 1 matched node(s) — the path selected a single array; use `length` to assert its size")
        );
        // A string is measurable too, so it gets the same nudge towards `length`.
        let r = run("$.status count 2");
        assert_eq!(
            r.results[0].outcome.message.as_deref(),
            Some("expected count 2, got 1 matched node(s) — the path selected a single string; use `length` to assert its size")
        );
        // No hint for a number, which has no size to assert.
        let r = run("$.items[0].price count 2");
        assert_eq!(
            r.results[0].outcome.message.as_deref(),
            Some("expected count 2, got 1 matched node(s)")
        );
        assert!(run("$.status count 1").passed);
    }

    #[test]
    fn failing_assertion_reports_location_and_actual() {
        let r = run("$.items[*].price lte 100");
        assert!(!r.passed);
        let o = &r.results[0].outcome;
        assert_eq!(o.status, Status::Fail);
        assert_eq!(o.matches, 2);
        assert_eq!(o.location.as_deref(), Some("$['items'][1]['price']"));
        assert_eq!(o.actual, Some(Value::from(250)));
        let text = text_report(&r);
        assert!(
            text.starts_with("FAIL — 0 of 1 assertions passed, 1 failed"),
            "{text}"
        );
        assert!(
            text.contains("at $['items'][1]['price']: expected <= 100, got 250"),
            "{text}"
        );
    }

    #[test]
    fn path_with_no_match_fails_value_operators() {
        let r = run("$.nope equals 1");
        assert_eq!(r.results[0].outcome.status, Status::Fail);
        assert_eq!(
            r.results[0].outcome.message.as_deref(),
            Some("path matched no nodes")
        );
    }

    #[test]
    fn integer_type_is_distinct_from_number() {
        let r = run("$.items[1].price type integer\n$.items[0].price type integer");
        assert_eq!(r.results[0].outcome.status, Status::Pass);
        assert_eq!(r.results[1].outcome.status, Status::Fail);
    }

    #[test]
    fn case_insensitive_affects_equals_contains_and_matches() {
        let strict = run_assertions(
            PAYLOAD,
            "$.user.name equals ADA\n$.user.roles contains ADMIN\n$.user.name matches ^ada$",
            "auto",
            true,
            false,
        )
        .unwrap();
        assert_eq!(strict.failed_count(), 3);
        let loose = run_assertions(
            PAYLOAD,
            "$.user.name equals ADA\n$.user.roles contains ADMIN\n$.user.name matches ^ada$",
            "auto",
            false,
            false,
        )
        .unwrap();
        assert_eq!(loose.passed_count(), 3);
    }

    #[test]
    fn stop_on_first_failure_skips_the_rest() {
        let r = run_assertions(
            PAYLOAD,
            "$.status equals nope\n$.status exists\n$.status exists",
            "auto",
            true,
            true,
        )
        .unwrap();
        assert_eq!(r.failed_count(), 1);
        assert_eq!(r.skipped_count(), 2);
        let text = text_report(&r);
        assert!(
            text.contains("0 of 3 assertions passed, 1 failed, 2 skipped"),
            "{text}"
        );
        assert!(text.contains("SKIP  #3"), "{text}");
    }

    #[test]
    fn json_rules_carry_names_and_range_bounds() {
        let r = run_assertions(
            PAYLOAD,
            r#"[
                {"name":"status ok","path":"$.status","op":"equals","value":"ok"},
                {"path":"$.items[*].price","op":"range","min":0,"max":100}
            ]"#,
            "json",
            true,
            false,
        )
        .unwrap();
        assert_eq!(r.results[0].rule.name.as_deref(), Some("status ok"));
        assert_eq!(r.results[0].outcome.status, Status::Pass);
        assert_eq!(r.results[1].outcome.status, Status::Fail);
        assert_eq!(r.results[1].rule.display(), "$.items[*].price range 0..100");
        let text = text_report(&r);
        assert!(
            text.contains("PASS  #1  [status ok] $.status equals \"ok\""),
            "{text}"
        );
    }

    #[test]
    fn json_report_shape() {
        let r = run("$.status equals ok\n$.status equals nope");
        let v = json_report(&r);
        assert_eq!(v["passed"], Value::Bool(false));
        assert_eq!(v["total"], Value::from(2));
        assert_eq!(v["passed_count"], Value::from(1));
        assert_eq!(v["failed_count"], Value::from(1));
        assert_eq!(v["skipped_count"], Value::from(0));
        assert_eq!(v["assertions"][0]["status"], Value::from("pass"));
        assert_eq!(v["assertions"][1]["status"], Value::from("fail"));
        assert_eq!(v["assertions"][1]["location"], Value::from("$['status']"));
        assert_eq!(v["assertions"][1]["actual"], Value::from("ok"));
    }

    #[test]
    fn operator_aliases_and_symbols_parse() {
        let r = run("$.items[0].price >= 9.5\n$.status == ok\n$.status != no\n$.user.roles len 2");
        assert!(r.passed, "{}", text_report(&r));
    }

    #[test]
    fn err_invalid_json_payload() {
        let e = render("{oops", "$.a exists", "auto", true, false, "text").unwrap_err();
        assert!(e.starts_with("invalid JSON payload:"), "{e}");
    }

    #[test]
    fn err_unknown_operator_lists_supported_ones() {
        let e = render(PAYLOAD, "$.status blorp 1", "auto", true, false, "text").unwrap_err();
        assert!(
            e.contains("assertion line 1: unknown operator 'blorp'"),
            "{e}"
        );
        assert!(e.contains("not_exists"), "{e}");
    }

    #[test]
    fn err_bad_jsonpath() {
        let e = render(PAYLOAD, "$.[ exists", "auto", true, false, "text").unwrap_err();
        assert!(e.starts_with("invalid JSONPath"), "{e}");
    }

    #[test]
    fn err_malformed_rule_line() {
        let e = render(PAYLOAD, "$.status", "auto", true, false, "text").unwrap_err();
        assert!(
            e.contains("expected `<jsonpath> <operator> [expected]`"),
            "{e}"
        );
    }

    #[test]
    fn err_missing_expected_value() {
        let e = render(PAYLOAD, "$.status equals", "auto", true, false, "text").unwrap_err();
        assert!(
            e.contains("operator 'equals' needs an expected value"),
            "{e}"
        );
    }

    #[test]
    fn err_bad_type_name_and_bad_regex() {
        let e = render(PAYLOAD, "$.status type strung", "auto", true, false, "text").unwrap_err();
        assert!(e.contains("type expects one of"), "{e}");
        let e = render(PAYLOAD, "$.status matches (", "auto", true, false, "text").unwrap_err();
        assert!(e.contains("invalid regular expression"), "{e}");
    }

    #[test]
    fn err_empty_inputs_and_unknown_enums() {
        assert!(render("", "$.a exists", "auto", true, false, "text")
            .unwrap_err()
            .contains("payload is empty"));
        assert!(render(PAYLOAD, "   ", "auto", true, false, "text")
            .unwrap_err()
            .contains("no assertions provided"));
        assert!(
            render(PAYLOAD, "$.status exists", "yaml", true, false, "text")
                .unwrap_err()
                .contains("unknown rules_format")
        );
        assert!(
            render(PAYLOAD, "$.status exists", "auto", true, false, "xml")
                .unwrap_err()
                .contains("unknown output")
        );
    }

    #[test]
    fn err_too_many_assertions_at_the_boundary() {
        let ok = "$.status exists\n".repeat(MAX_ASSERTIONS);
        assert_eq!(
            run_assertions(PAYLOAD, &ok, "dsl", true, false)
                .unwrap()
                .results
                .len(),
            MAX_ASSERTIONS
        );
        let over = "$.status exists\n".repeat(MAX_ASSERTIONS + 1);
        let e = run_assertions(PAYLOAD, &over, "dsl", true, false).unwrap_err();
        assert_eq!(e, "too many assertions: 201 (maximum 200)");
    }

    #[test]
    fn err_json_rules_validation() {
        let e = render(PAYLOAD, r#"[{"op":"exists"}]"#, "json", true, false, "text").unwrap_err();
        assert!(e.contains("assertion 1: missing a string \"path\""), "{e}");
        let e = render(PAYLOAD, r#"[{"path":"$.a"}]"#, "json", true, false, "text").unwrap_err();
        assert!(e.contains("missing a string \"op\""), "{e}");
        let e = render(PAYLOAD, "[1,2]", "json", true, false, "text").unwrap_err();
        assert!(e.contains("each assertion must be a JSON object"), "{e}");
    }

    #[test]
    fn contains_works_on_strings_and_object_keys() {
        let r = run("$.user.id contains u-\n$.user contains roles\n$.user not_contains missing");
        assert!(r.passed, "{}", text_report(&r));
    }

    #[test]
    fn quoted_dsl_tokens_stay_strings() {
        let r = run("$.user.id equals \"u-42\"");
        assert_eq!(r.results[0].outcome.status, Status::Pass);
        assert_eq!(r.results[0].rule.display(), "$.user.id equals \"u-42\"");
    }
}
