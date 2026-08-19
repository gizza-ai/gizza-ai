//! gizza-ai/json-dedupe-array core — remove duplicate ELEMENTS from a JSON
//! array. Duplicates are decided either by whole-element structural equality
//! (nested values included; object key ORDER is ignored when comparing but
//! preserved in the output) or by one or more chosen key fields given as
//! comma-separated dot-paths. The first or last occurrence is kept and the
//! original order is preserved; the tool can instead return only the removed
//! duplicates or a counts/groups report.
//!
//! Distinct from `jsonl-deduplicator` (NDJSON, one value per LINE) and from
//! `json-sort` (which reorders object keys). Pure-Rust; shared by the chat
//! skill block and the web page.

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Hard cap on array elements — beyond this the browser tab is the real limit,
/// so fail with a named error instead of an out-of-memory crash.
pub const MAX_ELEMENTS: usize = 200_000;

/// Which occurrence of a duplicated element survives.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keep {
    First,
    Last,
}

impl Keep {
    /// Parse the `keep` param. Blank/unknown falls back to first.
    pub fn parse(s: &str) -> Keep {
        match s.trim().to_ascii_lowercase().as_str() {
            "last" => Keep::Last,
            _ => Keep::First,
        }
    }
}

/// What the tool returns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputKind {
    /// The de-duplicated array (inside its original wrapper, if `root` is set).
    Unique,
    /// Only the elements that were removed, in their original order.
    Duplicates,
    /// A JSON report: total / unique / removed counts plus each duplicate group.
    Report,
}

impl OutputKind {
    /// Parse the `output` param. Blank/unknown falls back to the unique array.
    pub fn parse(s: &str) -> OutputKind {
        match s.trim().to_ascii_lowercase().as_str() {
            "duplicates" => OutputKind::Duplicates,
            "report" => OutputKind::Report,
            _ => OutputKind::Unique,
        }
    }
}

/// Options controlling the de-duplication.
pub struct Options {
    /// Comma-separated field(s) to compare on. Each may be a dot-path
    /// (`user.email`, array index `tags.0`). Empty compares whole elements.
    pub keys: String,
    /// Dot-path to the array when it is nested inside a wrapper object
    /// (`data.items`). Empty means the whole document IS the array.
    pub root: String,
    /// Which occurrence of a duplicate survives.
    pub keep: Keep,
    /// Compare case-insensitively (string values and field names).
    pub ignore_case: bool,
    /// What to return.
    pub output: OutputKind,
    /// Spaces of indentation per level (clamped 0..=8). `0` minifies.
    pub indent: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            keys: String::new(),
            root: String::new(),
            keep: Keep::First,
            ignore_case: false,
            output: OutputKind::Unique,
            indent: 2,
        }
    }
}

/// Split a comma-separated key spec into trimmed, non-empty dot-paths.
fn parse_keys(spec: &str) -> Vec<String> {
    spec.split(',')
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect()
}

/// Split a dot-path into its non-empty segments.
fn parse_path(spec: &str) -> Vec<String> {
    spec.split('.')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Read a dot-path inside a value (object fields; numeric segments index
/// arrays). `None` if any segment is absent or descends into a scalar.
fn extract<'a>(value: &'a Value, path: &str, ignore_case: bool) -> Option<&'a Value> {
    let mut cur = value;
    for seg in path.split('.') {
        match cur {
            Value::Object(map) => {
                cur = if ignore_case {
                    map.iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(seg))
                        .map(|(_, v)| v)?
                } else {
                    map.get(seg)?
                };
            }
            Value::Array(arr) => {
                let idx: usize = seg.parse().ok()?;
                cur = arr.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(cur)
}

/// Human name for a JSON value's type, for error messages.
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

/// Canonical, comparison-only rendering of a value.
///
/// Object keys are sorted so `{"a":1,"b":2}` and `{"b":2,"a":1}` compare equal
/// (the OUTPUT keeps the author's key order — this string is never emitted).
/// Numbers normalise so `1`, `1.0` and `1e0` compare equal, while integers too
/// large for exact `f64` keep their exact form.
fn canonical(v: &Value) -> String {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys
                .iter()
                .map(|k| {
                    let val = map.get(*k).unwrap_or(&Value::Null);
                    format!("{}:{}", Value::String((*k).clone()), canonical(val))
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Array(arr) => {
            let parts: Vec<String> = arr.iter().map(canonical).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                format!("#{i}")
            } else if let Some(u) = n.as_u64() {
                format!("#{u}")
            } else if let Some(f) = n.as_f64() {
                // Whole floats inside the exactly-representable range render
                // like the equivalent integer so 2 == 2.0.
                if f.fract() == 0.0 && f.abs() < 9.007_199_254_740_992e15 {
                    format!("#{}", f as i64)
                } else {
                    format!("#{f}")
                }
            } else {
                format!("#{n}")
            }
        }
        other => other.to_string(),
    }
}

/// The comparison signature of one element: the whole element when no keys are
/// configured, otherwise the chosen fields joined in order. A field that is
/// absent uses a sentinel distinct from an explicit JSON `null`.
fn signature(elem: &Value, keys: &[String], ignore_case: bool) -> String {
    let sig = if keys.is_empty() {
        canonical(elem)
    } else {
        let parts: Vec<String> = keys
            .iter()
            .map(|path| match extract(elem, path, ignore_case) {
                Some(v) => canonical(v),
                // \u{0} cannot appear in parsed JSON text, so no real value
                // can collide with the "field absent" marker.
                None => "\u{0}absent".to_string(),
            })
            .collect();
        // \u{1} likewise cannot appear in a canonical rendering.
        parts.join("\u{1}")
    };
    if ignore_case {
        sig.to_lowercase()
    } else {
        sig
    }
}

/// One set of elements that share a signature, in first-occurrence order.
struct Group {
    /// 0-based positions of every element in the group, ascending.
    indexes: Vec<usize>,
}

/// Group elements by signature, preserving first-occurrence order.
fn group_elements(elems: &[Value], keys: &[String], ignore_case: bool) -> Vec<Group> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut groups: Vec<Group> = Vec::new();
    for (i, elem) in elems.iter().enumerate() {
        let sig = signature(elem, keys, ignore_case);
        match seen.get(&sig) {
            Some(&g) => groups[g].indexes.push(i),
            None => {
                seen.insert(sig, groups.len());
                groups.push(Group { indexes: vec![i] });
            }
        }
    }
    groups
}

/// Serialize a value with `indent` spaces per level (`0` = minified).
fn serialize(value: &Value, indent: usize) -> Result<String, String> {
    let n = indent.min(8);
    if n == 0 {
        return serde_json::to_string(value).map_err(|e| format!("serialize failed: {e}"));
    }
    let pad = vec![b' '; n];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value
        .serialize(&mut ser)
        .map_err(|e| format!("serialize failed: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))
}

/// Walk `path` inside `doc` and return the slot holding the target array.
fn resolve_slot<'a>(doc: &'a mut Value, path: &[String], root: &str) -> Result<&'a mut Value, String> {
    let mut cur = doc;
    for (depth, seg) in path.iter().enumerate() {
        let walked = path[..depth].join(".");
        let here = if walked.is_empty() {
            "the top level".to_string()
        } else {
            format!("'{walked}'")
        };
        cur = match cur {
            Value::Object(map) => map.get_mut(seg).ok_or_else(|| {
                format!("no field '{seg}' at {here} — check the 'root' path '{root}'")
            })?,
            Value::Array(arr) => {
                let idx: usize = seg.parse().map_err(|_| {
                    format!("'{seg}' in the 'root' path '{root}' is not an array index — {here} is an array, so use a number")
                })?;
                let len = arr.len();
                arr.get_mut(idx).ok_or_else(|| {
                    format!("index {idx} is out of range at {here} — the array has {len} element(s)")
                })?
            }
            other => {
                return Err(format!(
                    "cannot look up '{seg}': {here} is {} — check the 'root' path '{root}'",
                    type_name(other)
                ))
            }
        };
    }
    Ok(cur)
}

/// De-duplicate the target array in `json` and render the requested output.
pub fn dedupe(json: &str, opts: &Options) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("no JSON input — paste a JSON array such as [1, 1, 2]".into());
    }
    let mut doc: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let path = parse_path(&opts.root);

    // Take ownership of the array so elements can be moved into the output.
    let elems: Vec<Value> = {
        let slot = resolve_slot(&mut doc, &path, &opts.root)?;
        match slot {
            Value::Array(arr) => std::mem::take(arr),
            other => {
                let where_ = if opts.root.trim().is_empty() {
                    "the top level of the input".to_string()
                } else {
                    format!("'{}'", opts.root.trim())
                };
                let hint = if opts.root.trim().is_empty() {
                    " — set 'root' to the dot-path of the array if it is nested inside an object, e.g. 'data.items'"
                } else {
                    ""
                };
                return Err(format!(
                    "expected a JSON array at {where_}, got {}{hint}",
                    type_name(other)
                ));
            }
        }
    };

    if elems.len() > MAX_ELEMENTS {
        return Err(format!(
            "array has {} elements — the limit is {MAX_ELEMENTS}; split the input into smaller batches",
            elems.len()
        ));
    }

    let keys = parse_keys(&opts.keys);
    let groups = group_elements(&elems, &keys, opts.ignore_case);

    // The surviving position of each group, and a per-index keep flag.
    let mut keep_flags = vec![false; elems.len()];
    for g in &groups {
        let kept = match opts.keep {
            Keep::First => g.indexes[0],
            Keep::Last => *g.indexes.last().unwrap_or(&0),
        };
        keep_flags[kept] = true;
    }

    match opts.output {
        OutputKind::Unique => {
            let kept: Vec<Value> = elems
                .into_iter()
                .zip(keep_flags.iter())
                .filter_map(|(v, &k)| if k { Some(v) } else { None })
                .collect();
            let slot = resolve_slot(&mut doc, &path, &opts.root)?;
            *slot = Value::Array(kept);
            serialize(&doc, opts.indent)
        }
        OutputKind::Duplicates => {
            let dropped: Vec<Value> = elems
                .into_iter()
                .zip(keep_flags.iter())
                .filter_map(|(v, &k)| if k { None } else { Some(v) })
                .collect();
            serialize(&Value::Array(dropped), opts.indent)
        }
        OutputKind::Report => {
            let total = elems.len();
            let unique = groups.len();
            let removed = total - unique;
            let mut dup_groups: Vec<Value> = Vec::new();
            for g in &groups {
                if g.indexes.len() < 2 {
                    continue;
                }
                let kept_index = match opts.keep {
                    Keep::First => g.indexes[0],
                    Keep::Last => *g.indexes.last().unwrap_or(&0),
                };
                let mut entry = Map::new();
                entry.insert("count".into(), Value::from(g.indexes.len()));
                entry.insert(
                    "indexes".into(),
                    Value::Array(g.indexes.iter().map(|&i| Value::from(i)).collect()),
                );
                entry.insert("kept_index".into(), Value::from(kept_index));
                entry.insert("value".into(), elems[kept_index].clone());
                dup_groups.push(Value::Object(entry));
            }
            let mut report = Map::new();
            report.insert("total".into(), Value::from(total));
            report.insert("unique".into(), Value::from(unique));
            report.insert("removed".into(), Value::from(removed));
            report.insert("duplicate_groups".into(), Value::Array(dup_groups));
            serialize(&Value::Object(report), opts.indent)
        }
    }
}

/// Web/page entry: build [`Options`] from the raw string fields (order matches
/// the descriptor / meta.toml) and return the rendered text.
pub fn run(
    json: &str,
    keys: &str,
    root: &str,
    keep: &str,
    ignore_case: bool,
    output: &str,
    indent: &str,
) -> Result<String, String> {
    let opts = Options {
        keys: keys.to_string(),
        root: root.to_string(),
        keep: Keep::parse(keep),
        ignore_case,
        output: OutputKind::parse(output),
        indent: indent.trim().parse().unwrap_or(2),
    };
    dedupe(json, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(keys: &str) -> Options {
        Options {
            keys: keys.into(),
            indent: 0,
            ..Default::default()
        }
    }

    #[test]
    fn removes_duplicate_scalars_keeping_order() {
        let out = dedupe("[3, 1, 3, 2, 1]", &opts("")).unwrap();
        assert_eq!(out, "[3,1,2]");
    }

    #[test]
    fn removes_structurally_equal_objects() {
        let out = dedupe(
            r#"[{"id":1,"n":"a"},{"id":2,"n":"b"},{"id":1,"n":"a"}]"#,
            &opts(""),
        )
        .unwrap();
        assert_eq!(out, r#"[{"id":1,"n":"a"},{"id":2,"n":"b"}]"#);
    }

    #[test]
    fn key_order_is_ignored_when_comparing_but_preserved_in_output() {
        // Same object written with the keys in a different order = duplicate,
        // yet the surviving element keeps the author's key order.
        let out = dedupe(r#"[{"b":2,"a":1},{"a":1,"b":2}]"#, &opts("")).unwrap();
        assert_eq!(out, r#"[{"b":2,"a":1}]"#);
    }

    #[test]
    fn nested_values_compare_deeply() {
        let out = dedupe(
            r#"[{"u":{"tags":[1,2]}},{"u":{"tags":[1,2]}},{"u":{"tags":[2,1]}}]"#,
            &opts(""),
        )
        .unwrap();
        assert_eq!(out, r#"[{"u":{"tags":[1,2]}},{"u":{"tags":[2,1]}}]"#);
    }

    #[test]
    fn integer_and_whole_float_compare_equal() {
        let out = dedupe("[2, 2.0, 2.5]", &opts("")).unwrap();
        assert_eq!(out, "[2,2.5]");
    }

    #[test]
    fn dedupes_by_single_key() {
        let out = dedupe(
            r#"[{"id":1,"n":"a"},{"id":1,"n":"b"},{"id":2,"n":"c"}]"#,
            &opts("id"),
        )
        .unwrap();
        assert_eq!(out, r#"[{"id":1,"n":"a"},{"id":2,"n":"c"}]"#);
    }

    #[test]
    fn dedupes_by_multiple_keys() {
        let out = dedupe(
            r#"[{"a":1,"b":1},{"a":1,"b":2},{"a":1,"b":1,"c":9}]"#,
            &opts("a,b"),
        )
        .unwrap();
        assert_eq!(out, r#"[{"a":1,"b":1},{"a":1,"b":2}]"#);
    }

    #[test]
    fn dedupes_by_nested_dot_path() {
        let out = dedupe(
            r#"[{"u":{"email":"a@x.com"},"v":1},{"u":{"email":"a@x.com"},"v":2}]"#,
            &opts("u.email"),
        )
        .unwrap();
        assert_eq!(out, r#"[{"u":{"email":"a@x.com"},"v":1}]"#);
    }

    #[test]
    fn key_path_can_index_an_array() {
        let out = dedupe(r#"[{"t":["x","p"]},{"t":["x","q"]},{"t":["y"]}]"#, &opts("t.0")).unwrap();
        assert_eq!(out, r#"[{"t":["x","p"]},{"t":["y"]}]"#);
    }

    #[test]
    fn absent_key_differs_from_explicit_null() {
        // {"id":null} and {} must NOT collapse into each other.
        let out = dedupe(r#"[{"id":null},{},{"id":null}]"#, &opts("id")).unwrap();
        assert_eq!(out, r#"[{"id":null},{}]"#);
    }

    #[test]
    fn keep_last_keeps_the_final_occurrence_and_its_position() {
        let o = Options {
            keys: "id".into(),
            keep: Keep::Last,
            indent: 0,
            ..Default::default()
        };
        let out = dedupe(
            r#"[{"id":1,"n":"a"},{"id":2,"n":"b"},{"id":1,"n":"z"}]"#,
            &o,
        )
        .unwrap();
        assert_eq!(out, r#"[{"id":2,"n":"b"},{"id":1,"n":"z"}]"#);
    }

    #[test]
    fn ignore_case_collapses_differently_cased_values() {
        let o = Options {
            keys: "email".into(),
            ignore_case: true,
            indent: 0,
            ..Default::default()
        };
        let out = dedupe(
            r#"[{"email":"A@X.com"},{"email":"a@x.com"},{"email":"b@x.com"}]"#,
            &o,
        )
        .unwrap();
        assert_eq!(out, r#"[{"email":"A@X.com"},{"email":"b@x.com"}]"#);
    }

    #[test]
    fn ignore_case_also_matches_field_names() {
        let o = Options {
            keys: "id".into(),
            ignore_case: true,
            indent: 0,
            ..Default::default()
        };
        let out = dedupe(r#"[{"ID":"A"},{"id":"a"},{"Id":"B"}]"#, &o).unwrap();
        assert_eq!(out, r#"[{"ID":"A"},{"Id":"B"}]"#);
    }

    #[test]
    fn case_sensitive_by_default() {
        let out = dedupe(r#"["Ada","ada"]"#, &opts("")).unwrap();
        assert_eq!(out, r#"["Ada","ada"]"#);
    }

    #[test]
    fn root_path_dedupes_a_nested_array_and_keeps_the_wrapper() {
        let o = Options {
            root: "data.items".into(),
            indent: 0,
            ..Default::default()
        };
        let out = dedupe(r#"{"ok":true,"data":{"items":[1,1,2]}}"#, &o).unwrap();
        assert_eq!(out, r#"{"ok":true,"data":{"items":[1,2]}}"#);
    }

    #[test]
    fn duplicates_output_lists_only_removed_elements() {
        let o = Options {
            keys: "id".into(),
            output: OutputKind::Duplicates,
            indent: 0,
            ..Default::default()
        };
        let out = dedupe(
            r#"[{"id":1,"n":"a"},{"id":1,"n":"b"},{"id":2,"n":"c"}]"#,
            &o,
        )
        .unwrap();
        assert_eq!(out, r#"[{"id":1,"n":"b"}]"#);
    }

    #[test]
    fn report_output_counts_and_groups() {
        let o = Options {
            output: OutputKind::Report,
            indent: 0,
            ..Default::default()
        };
        let out = dedupe("[1, 2, 1, 1]", &o).unwrap();
        assert_eq!(
            out,
            r#"{"total":4,"unique":2,"removed":2,"duplicate_groups":[{"count":3,"indexes":[0,2,3],"kept_index":0,"value":1}]}"#
        );
    }

    #[test]
    fn report_kept_index_follows_keep_last() {
        let o = Options {
            keep: Keep::Last,
            output: OutputKind::Report,
            indent: 0,
            ..Default::default()
        };
        let out = dedupe("[1, 2, 1]", &o).unwrap();
        assert!(out.contains(r#""kept_index":2"#), "got {out}");
    }

    #[test]
    fn pretty_indent_two_by_default() {
        let out = dedupe("[1,1,2]", &Options::default()).unwrap();
        assert_eq!(out, "[\n  1,\n  2\n]");
    }

    #[test]
    fn indent_clamped_to_eight() {
        let o = Options {
            indent: 99,
            ..Default::default()
        };
        let out = dedupe("[1,1]", &o).unwrap();
        assert_eq!(out, "[\n        1\n]");
    }

    #[test]
    fn empty_array_round_trips() {
        assert_eq!(dedupe("[]", &opts("")).unwrap(), "[]");
    }

    #[test]
    fn already_unique_array_is_unchanged() {
        let out = dedupe(r#"[1,"1",true,null]"#, &opts("")).unwrap();
        assert_eq!(out, r#"[1,"1",true,null]"#);
    }

    #[test]
    fn accepts_exactly_the_element_cap() {
        let body: Vec<String> = (0..MAX_ELEMENTS).map(|i| i.to_string()).collect();
        let json = format!("[{}]", body.join(","));
        let out = dedupe(&json, &opts("")).unwrap();
        assert_eq!(out.matches(',').count(), MAX_ELEMENTS - 1);
    }

    #[test]
    fn rejects_one_element_over_the_cap() {
        let body: Vec<String> = (0..=MAX_ELEMENTS).map(|i| i.to_string()).collect();
        let json = format!("[{}]", body.join(","));
        let e = dedupe(&json, &opts("")).unwrap_err();
        assert!(e.contains("the limit is 200000"), "got {e}");
    }

    #[test]
    fn rejects_empty_input() {
        let e = dedupe("   ", &opts("")).unwrap_err();
        assert!(e.contains("no JSON input"), "got {e}");
    }

    #[test]
    fn rejects_invalid_json() {
        let e = dedupe("[1, 2,]", &opts("")).unwrap_err();
        assert!(e.contains("invalid JSON"), "got {e}");
    }

    #[test]
    fn rejects_non_array_top_level_with_a_root_hint() {
        let e = dedupe(r#"{"items":[1,1]}"#, &opts("")).unwrap_err();
        assert!(e.contains("expected a JSON array"), "got {e}");
        assert!(e.contains("'root'"), "got {e}");
    }

    #[test]
    fn rejects_missing_root_field() {
        let o = Options {
            root: "data.rows".into(),
            ..Default::default()
        };
        let e = dedupe(r#"{"data":{"items":[1]}}"#, &o).unwrap_err();
        assert!(e.contains("no field 'rows'"), "got {e}");
    }

    #[test]
    fn rejects_root_pointing_at_a_non_array() {
        let o = Options {
            root: "data".into(),
            ..Default::default()
        };
        let e = dedupe(r#"{"data":{"items":[1]}}"#, &o).unwrap_err();
        assert!(e.contains("expected a JSON array at 'data'"), "got {e}");
    }

    #[test]
    fn run_wires_string_fields() {
        let out = run("[1,1,2]", "", "", "first", false, "unique", "0").unwrap();
        assert_eq!(out, "[1,2]");
    }
}
