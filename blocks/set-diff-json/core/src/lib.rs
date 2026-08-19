//! gizza-ai/set-diff-json core — set operations (union, intersection, difference,
//! symmetric difference) over two JSON arrays. Elements are compared as JSON
//! *values* (object key order and `1` vs `1.0` don't matter), or matched on a
//! chosen field with `key` (a name or dot-path) the way lodash's `differenceBy`
//! family does. Pure-Rust; shared by the chat skill block and the web page.

use serde_json::{Map, Value};
use std::collections::HashSet;

/// Maximum elements accepted per input array. Generous enough for record dumps
/// while keeping the single-pass hashing instant.
pub const MAX_ELEMENTS: usize = 50_000;

/// Which set operation to compute over arrays A and B.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operation {
    /// Everything in A, plus the elements of B that A doesn't already have.
    Union,
    /// The elements of A that also appear in B.
    Intersection,
    /// The elements of A that do not appear in B (A − B).
    Difference,
    /// The elements in exactly one of the arrays: (A − B) then (B − A).
    SymmetricDifference,
}

impl Operation {
    /// Parse the `operation` param. Common aliases are accepted; anything else
    /// is an error so a typo never silently computes the wrong set.
    pub fn parse(s: &str) -> Result<Operation, String> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "" | "difference" | "diff" | "a_minus_b" | "subtract" => Ok(Operation::Difference),
            "union" | "or" => Ok(Operation::Union),
            "intersection" | "intersect" | "and" => Ok(Operation::Intersection),
            "symmetric_difference" | "symmetric" | "xor" => Ok(Operation::SymmetricDifference),
            other => Err(format!(
                "invalid operation {other:?}: expected \"union\", \"intersection\", \"difference\", or \"symmetric_difference\""
            )),
        }
    }

    /// The value written to the report's `operation` field.
    fn label(self) -> &'static str {
        match self {
            Operation::Union => "union",
            Operation::Intersection => "intersection",
            Operation::Difference => "difference",
            Operation::SymmetricDifference => "symmetric_difference",
        }
    }
}

/// Shape of the returned JSON.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// An object with `operation`, `matched_by`, `counts`, and `result`.
    Report,
    /// The bare result array, ready to paste into the next step.
    Array,
}

impl Output {
    /// Parse the `output` param. Blank falls back to the full report.
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" => Ok(Output::Report),
            "array" => Ok(Output::Array),
            other => Err(format!(
                "invalid output {other:?}: expected \"report\" or \"array\""
            )),
        }
    }
}

/// Options controlling the set operation.
#[derive(Debug)]
pub struct Options {
    /// Which set operation to compute.
    pub operation: Operation,
    /// Optional field to match elements on: a key name, or a dot-path into
    /// nested objects/arrays (`meta.id`, `tags.0`). Blank compares whole values.
    pub key: String,
    /// Fold ASCII/Unicode case when comparing strings (inside the key value, or
    /// anywhere in the compared value when no key is set).
    pub case_insensitive: bool,
    /// Collapse repeats so the result is a true set. Off keeps every occurrence.
    pub dedupe: bool,
    /// Shape of the returned JSON.
    pub output: Output,
    /// Spaces of indentation per level (clamped 0..=8). `0` minifies to one line.
    pub indent: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            operation: Operation::Difference,
            key: String::new(),
            case_insensitive: false,
            dedupe: true,
            output: Output::Report,
            indent: 2,
        }
    }
}

/// Parse one input array. Accepts a top-level JSON array, or NDJSON / JSON Lines
/// (one JSON value per line) as a convenience for pasted exports.
fn parse_array(text: &str, which: &str) -> Result<Vec<Value>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "array {which} is empty; paste a JSON array like [1, 2, 3]"
        ));
    }
    if trimmed.starts_with('[') {
        let parsed: Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("array {which} is not valid JSON: {e}"))?;
        let items = match parsed {
            Value::Array(items) => items,
            _ => unreachable!("input starting with '[' parses to an array or fails"),
        };
        check_len(items.len(), which)?;
        return Ok(items);
    }
    // Not an array literal: accept NDJSON / JSON Lines so a pasted export works
    // without hand-wrapping it in brackets. Two or more lines are required — a
    // lone `{...}` is far more likely a mis-paste than a one-record export, and
    // silently treating it as a set of one would hide the mistake.
    let lines: Vec<&str> = trimmed
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let not_an_array = format!(
        "array {which} is not a JSON array: it must start with '[', or be NDJSON with one JSON value per line"
    );
    if lines.len() < 2 {
        return Err(not_an_array);
    }
    let mut items = Vec::new();
    for line in lines {
        let v: Value = serde_json::from_str(line).map_err(|_| not_an_array.clone())?;
        items.push(v);
    }
    check_len(items.len(), which)?;
    Ok(items)
}

/// Guard the per-array element cap.
fn check_len(len: usize, which: &str) -> Result<(), String> {
    if len > MAX_ELEMENTS {
        return Err(format!(
            "array {which} has {len} elements; the maximum is {MAX_ELEMENTS}"
        ));
    }
    Ok(())
}

/// Walk a dot-path (`meta.id`, `tags.0`) into a value. Returns `None` when any
/// segment is absent, so the caller can report a precise missing-key error.
fn lookup<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = value;
    for seg in path.split('.') {
        cur = match cur {
            Value::Object(map) => map.get(seg)?,
            Value::Array(items) => items.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// Build the comparison key for a value: a canonical JSON rendering with object
/// keys sorted (so key order never matters), numbers normalized through `f64`
/// (so `1` and `1.0` match), and strings optionally case-folded.
fn canonical(value: &Value, case_insensitive: bool) -> String {
    fn norm(value: &Value, ci: bool) -> Value {
        match value {
            Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                let mut out = Map::new();
                for k in keys {
                    out.insert(k.clone(), norm(&map[k], ci));
                }
                Value::Object(out)
            }
            Value::Array(items) => Value::Array(items.iter().map(|v| norm(v, ci)).collect()),
            Value::String(s) => Value::String(if ci { s.to_lowercase() } else { s.clone() }),
            Value::Number(n) => match n.as_f64() {
                // Render every number through f64 so 1, 1.0 and 1e0 agree.
                Some(f) => Value::String(format!("#{f}")),
                None => Value::String(format!("#{n}")),
            },
            other => other.clone(),
        }
    }
    norm(value, case_insensitive).to_string()
}

/// Compute the match key for one element, erroring clearly when `key` is set but
/// the element doesn't carry it.
fn element_key(value: &Value, index: usize, which: &str, opts: &Options) -> Result<String, String> {
    if opts.key.is_empty() {
        return Ok(canonical(value, opts.case_insensitive));
    }
    match lookup(value, &opts.key) {
        Some(found) => Ok(canonical(found, opts.case_insensitive)),
        None => Err(format!(
            "array {which} element {index} has no {:?} field; every element must contain the key field (leave `key` blank to compare whole values)",
            opts.key
        )),
    }
}

/// The match keys of one array, in element order.
fn keys_of(items: &[Value], which: &str, opts: &Options) -> Result<Vec<String>, String> {
    items
        .iter()
        .enumerate()
        .map(|(i, v)| element_key(v, i, which, opts))
        .collect()
}

/// Collect elements from `items` whose key passes `keep`, honouring `dedupe`.
fn collect(
    items: &[Value],
    keys: &[String],
    dedupe: bool,
    keep: impl Fn(&str) -> bool,
    seen: &mut HashSet<String>,
    out: &mut Vec<Value>,
) {
    for (item, key) in items.iter().zip(keys) {
        if !keep(key) {
            continue;
        }
        if dedupe && !seen.insert(key.clone()) {
            continue;
        }
        out.push(item.clone());
    }
}

/// Serialize with the requested indentation (`0` minifies).
fn render(value: &Value, indent: usize) -> String {
    if indent == 0 {
        return value.to_string();
    }
    let pad = vec![b' '; indent.min(8)];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    serde::Serialize::serialize(value, &mut ser).expect("serializing a Value cannot fail");
    String::from_utf8(buf).expect("serde_json emits UTF-8")
}

/// Run a set operation over two JSON arrays and return the rendered JSON.
///
/// Elements are compared as whole JSON values (object key order and integer /
/// float spelling are irrelevant) unless `key` names a field — then elements are
/// matched on that field's value, like `differenceBy`. Result elements always
/// come from array A where an operation can draw from either side, and the first
/// occurrence of a repeated key wins.
pub fn set_diff(array_a: &str, array_b: &str, opts: &Options) -> Result<String, String> {
    let a = parse_array(array_a, "A")?;
    let b = parse_array(array_b, "B")?;
    let a_keys = keys_of(&a, "A", opts)?;
    let b_keys = keys_of(&b, "B", opts)?;

    let a_set: HashSet<&str> = a_keys.iter().map(String::as_str).collect();
    let b_set: HashSet<&str> = b_keys.iter().map(String::as_str).collect();

    let mut seen: HashSet<String> = HashSet::new();
    let mut result: Vec<Value> = Vec::new();
    match opts.operation {
        Operation::Union => {
            collect(&a, &a_keys, opts.dedupe, |_| true, &mut seen, &mut result);
            collect(
                &b,
                &b_keys,
                opts.dedupe,
                |k| !a_set.contains(k),
                &mut seen,
                &mut result,
            );
        }
        Operation::Intersection => collect(
            &a,
            &a_keys,
            opts.dedupe,
            |k| b_set.contains(k),
            &mut seen,
            &mut result,
        ),
        Operation::Difference => collect(
            &a,
            &a_keys,
            opts.dedupe,
            |k| !b_set.contains(k),
            &mut seen,
            &mut result,
        ),
        Operation::SymmetricDifference => {
            collect(
                &a,
                &a_keys,
                opts.dedupe,
                |k| !b_set.contains(k),
                &mut seen,
                &mut result,
            );
            collect(
                &b,
                &b_keys,
                opts.dedupe,
                |k| !a_set.contains(k),
                &mut seen,
                &mut result,
            );
        }
    }

    let result = Value::Array(result);
    if opts.output == Output::Array {
        return Ok(render(&result, opts.indent));
    }

    // Counts describe the *sets* (distinct keys), so they stay meaningful whether
    // or not `dedupe` is on for the result itself.
    let only_a = a_set.difference(&b_set).count();
    let only_b = b_set.difference(&a_set).count();
    let both = a_set.intersection(&b_set).count();
    let result_len = result.as_array().map_or(0, Vec::len);
    let mut counts = Map::new();
    counts.insert("a".into(), a.len().into());
    counts.insert("b".into(), b.len().into());
    counts.insert("a_unique".into(), a_set.len().into());
    counts.insert("b_unique".into(), b_set.len().into());
    counts.insert("only_in_a".into(), only_a.into());
    counts.insert("only_in_b".into(), only_b.into());
    counts.insert("in_both".into(), both.into());
    counts.insert("union".into(), (only_a + only_b + both).into());
    counts.insert("result".into(), result_len.into());

    let mut report = Map::new();
    report.insert("operation".into(), opts.operation.label().into());
    report.insert(
        "matched_by".into(),
        if opts.key.is_empty() {
            Value::String("value".into())
        } else {
            Value::String(opts.key.clone())
        },
    );
    report.insert("counts".into(), Value::Object(counts));
    report.insert("result".into(), result);
    Ok(render(&Value::Object(report), opts.indent))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(operation: Operation) -> Options {
        Options {
            operation,
            output: Output::Array,
            indent: 0,
            ..Default::default()
        }
    }

    #[test]
    fn difference_of_scalar_arrays() {
        let out = set_diff("[1,2,3]", "[2,4]", &opts(Operation::Difference)).unwrap();
        assert_eq!(out, "[1,3]");
    }

    #[test]
    fn union_keeps_a_order_then_new_b_items() {
        let out = set_diff("[1,2]", "[2,3]", &opts(Operation::Union)).unwrap();
        assert_eq!(out, "[1,2,3]");
    }

    #[test]
    fn intersection_and_symmetric_difference() {
        assert_eq!(
            set_diff("[1,2,3]", "[3,2,9]", &opts(Operation::Intersection)).unwrap(),
            "[2,3]"
        );
        assert_eq!(
            set_diff("[1,2,3]", "[3,2,9]", &opts(Operation::SymmetricDifference)).unwrap(),
            "[1,9]"
        );
    }

    #[test]
    fn objects_match_regardless_of_key_order_and_number_spelling() {
        let out = set_diff(
            r#"[{"a":1,"b":2},{"a":3}]"#,
            r#"[{"b":2,"a":1.0}]"#,
            &opts(Operation::Difference),
        )
        .unwrap();
        assert_eq!(out, r#"[{"a":3}]"#);
    }

    #[test]
    fn key_matching_ignores_other_fields() {
        let o = Options {
            key: "id".into(),
            ..opts(Operation::Difference)
        };
        let out = set_diff(
            r#"[{"id":1,"name":"Ada"},{"id":2,"name":"Bo"}]"#,
            r#"[{"id":2,"name":"CHANGED"}]"#,
            &o,
        )
        .unwrap();
        assert_eq!(out, r#"[{"id":1,"name":"Ada"}]"#);
    }

    #[test]
    fn nested_dot_path_key() {
        let o = Options {
            key: "meta.sku".into(),
            ..opts(Operation::Intersection)
        };
        let out = set_diff(
            r#"[{"meta":{"sku":"A"}},{"meta":{"sku":"B"}}]"#,
            r#"[{"meta":{"sku":"B"}}]"#,
            &o,
        )
        .unwrap();
        assert_eq!(out, r#"[{"meta":{"sku":"B"}}]"#);
    }

    #[test]
    fn case_insensitive_matching() {
        let o = Options {
            case_insensitive: true,
            ..opts(Operation::Intersection)
        };
        assert_eq!(
            set_diff(r#"["Apple","Pear"]"#, r#"["APPLE"]"#, &o).unwrap(),
            r#"["Apple"]"#
        );
        // Case-sensitive by default: nothing matches.
        assert_eq!(
            set_diff(
                r#"["Apple","Pear"]"#,
                r#"["APPLE"]"#,
                &opts(Operation::Intersection)
            )
            .unwrap(),
            "[]"
        );
    }

    #[test]
    fn dedupe_off_keeps_every_occurrence() {
        let o = Options {
            dedupe: false,
            ..opts(Operation::Difference)
        };
        assert_eq!(set_diff("[1,1,2]", "[2]", &o).unwrap(), "[1,1]");
        assert_eq!(
            set_diff("[1,1,2]", "[2]", &opts(Operation::Difference)).unwrap(),
            "[1]"
        );
    }

    #[test]
    fn ndjson_input_is_accepted() {
        let out = set_diff(
            "{\"id\":1}\n{\"id\":2}",
            r#"[{"id":2}]"#,
            &Options {
                key: "id".into(),
                ..opts(Operation::Difference)
            },
        )
        .unwrap();
        assert_eq!(out, r#"[{"id":1}]"#);
    }

    #[test]
    fn report_output_carries_counts() {
        let out = set_diff("[1,2,3]", "[3,4]", &Options::default()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["operation"], "difference");
        assert_eq!(v["matched_by"], "value");
        assert_eq!(v["counts"]["only_in_a"], 2);
        assert_eq!(v["counts"]["only_in_b"], 1);
        assert_eq!(v["counts"]["in_both"], 1);
        assert_eq!(v["counts"]["union"], 4);
        assert_eq!(v["counts"]["result"], 2);
        assert_eq!(v["result"], serde_json::json!([1, 2]));
        assert!(out.contains("\n  \"operation\""), "indent 2 by default");
    }

    #[test]
    fn rejects_non_array_input() {
        let err = set_diff(r#"{"a":1}"#, "[1]", &Options::default()).unwrap_err();
        assert!(err.contains("array A is not a JSON array"), "{err}");
    }

    #[test]
    fn rejects_malformed_json_array() {
        let err = set_diff("[1,2", "[1]", &Options::default()).unwrap_err();
        assert!(err.contains("array A is not valid JSON"), "{err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = set_diff("[1]", "   ", &Options::default()).unwrap_err();
        assert!(err.contains("array B is empty"), "{err}");
    }

    #[test]
    fn rejects_element_missing_the_key_field() {
        let o = Options {
            key: "id".into(),
            ..Options::default()
        };
        let err = set_diff(r#"[{"id":1},{"name":"Bo"}]"#, r#"[{"id":1}]"#, &o).unwrap_err();
        assert!(
            err.contains("array A element 1 has no \"id\" field"),
            "{err}"
        );
    }

    #[test]
    fn rejects_unknown_operation() {
        let err = Operation::parse("sideways").unwrap_err();
        assert!(err.contains("invalid operation"), "{err}");
    }

    #[test]
    fn rejects_oversized_array() {
        let big = format!(
            "[{}]",
            (0..=MAX_ELEMENTS)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        let err = set_diff(&big, "[1]", &Options::default()).unwrap_err();
        assert!(err.contains("the maximum is 50000"), "{err}");
    }
}
