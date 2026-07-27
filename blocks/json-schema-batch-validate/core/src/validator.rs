//! A small, dependency-light JSON Schema validator covering the common subset
//! this tool documents. It exists so the block carries **no SIMD-using crate**
//! (the full `jsonschema` crate transitively pulls `uuid-simd`/`vsimd`, which
//! emit wasm SIMD that the wafer runtime — SIMD disabled — refuses to
//! instantiate). Error messages are kept byte-for-byte compatible with the
//! `jsonschema` crate's `Display` output so the page/CLI reports read the same.
//!
//! ## Supported keywords
//! `type` (incl. `integer` vs `number`), `required`, `properties`,
//! `additionalProperties` (bool or subschema), `enum`, `const`,
//! `minimum`/`maximum`/`exclusiveMinimum`/`exclusiveMaximum` (numeric form),
//! `minLength`/`maxLength`, `pattern` (Rust `regex`, ECMA-262-ish, no
//! lookaround/backreferences), `items` (single subschema or tuple form), and
//! internal `$ref` (`#/…` JSON Pointers into the same document, including
//! `$defs`/`definitions`). Boolean subschemas (`true`/`false`) are honored.
//!
//! ## Not supported (validated leniently — treated as annotations, never fail)
//! `format` assertion, `patternProperties`, `dependencies`/`dependentSchemas`,
//! `allOf`/`anyOf`/`oneOf`/`not`, `if`/`then`/`else`, `contains`,
//! `uniqueItems`, `minItems`/`maxItems`, `minProperties`/`maxProperties`,
//! `multipleOf`, `propertyNames`, `unevaluated*`, external/remote `$ref`, and
//! the draft-4 boolean `exclusiveMinimum`/`exclusiveMaximum` form. Validation
//! is draft-agnostic: the reported `draft` reflects selection/detection only.

use crate::RecordError;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

/// A schema compiled once (structure checked, `pattern` regexes built) and then
/// reused to validate every record in the batch.
pub struct CompiledSchema {
    root: Value,
    patterns: HashMap<String, Regex>,
}

impl CompiledSchema {
    /// Structurally validate `schema` and pre-compile its `pattern` regexes.
    /// Returns a human message (without the "not a valid JSON Schema" prefix)
    /// when the schema itself is malformed.
    pub fn build(schema: &Value) -> Result<Self, String> {
        let mut patterns = HashMap::new();
        check_schema(schema, schema, &mut patterns)?;
        Ok(CompiledSchema {
            root: schema.clone(),
            patterns,
        })
    }

    /// Every validation failure `instance` produced, in traversal order. The
    /// caller sorts for deterministic output.
    pub fn errors(&self, instance: &Value) -> Vec<RecordError> {
        let mut errs = Vec::new();
        validate_value(&self.root, instance, "", &self.root, &self.patterns, &mut errs);
        errs
    }
}

const KNOWN_TYPES: [&str; 7] = [
    "null", "boolean", "object", "array", "number", "integer", "string",
];

/// Render a JSON value exactly like `serde_json`'s `Display` (compact JSON),
/// matching how the `jsonschema` crate renders instances/limits in messages.
fn show(v: &Value) -> String {
    v.to_string()
}

/// Escape a property name for use as a JSON Pointer segment (RFC 6901).
fn escape_ptr(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

/// Resolve an internal `$ref` (`#`, or `#/…` JSON Pointer) against the document
/// root. External/remote refs (anything not starting with `#`) return `None`.
fn resolve_ref<'a>(r: &str, root: &'a Value) -> Option<&'a Value> {
    let ptr = if r == "#" {
        ""
    } else {
        r.strip_prefix('#')?
    };
    root.pointer(ptr)
}

/// Is `v` an instance of the single JSON Schema `type` name `t`?
fn is_type(v: &Value, t: &str) -> bool {
    match t {
        "null" => v.is_null(),
        "boolean" => v.is_boolean(),
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "number" => v.is_number(),
        "integer" => is_integer(v),
        _ => false,
    }
}

/// JSON Schema `integer`: an integral number, so `1` and `1.0` both qualify but
/// `1.5` does not.
fn is_integer(v: &Value) -> bool {
    if v.is_i64() || v.is_u64() {
        true
    } else {
        matches!(v.as_f64(), Some(f) if v.is_f64() && f.fract() == 0.0)
    }
}

fn type_matches(inst: &Value, t: &Value) -> bool {
    match t {
        Value::String(s) => is_type(inst, s),
        Value::Array(arr) => arr
            .iter()
            .any(|x| x.as_str().map_or(false, |s| is_type(inst, s))),
        // Malformed `type` is rejected at build time; be permissive here.
        _ => true,
    }
}

fn type_message(inst: &Value, t: &Value) -> String {
    match t {
        Value::String(s) => format!(r#"{} is not of type "{}""#, show(inst), s),
        Value::Array(arr) => {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|x| x.as_str())
                .map(|s| format!("\"{s}\""))
                .collect();
            format!("{} is not of types {}", show(inst), parts.join(", "))
        }
        _ => String::new(),
    }
}

fn additional_props_msg(props: &[&String]) -> String {
    let quoted: Vec<String> = props.iter().map(|p| format!("'{p}'")).collect();
    let tail = if props.len() == 1 {
        " was unexpected)"
    } else {
        " were unexpected)"
    };
    format!(
        "Additional properties are not allowed ({}{}",
        quoted.join(", "),
        tail
    )
}

/// Structurally validate a (sub)schema and compile any `pattern` regexes it (or
/// its known nested subschemas) contains.
fn check_schema(
    schema: &Value,
    root: &Value,
    patterns: &mut HashMap<String, Regex>,
) -> Result<(), String> {
    let map = match schema {
        Value::Bool(_) => return Ok(()),
        Value::Object(map) => map,
        _ => return Err("a JSON Schema must be an object or a boolean".to_string()),
    };

    if let Some(t) = map.get("type") {
        check_type_decl(t)?;
    }
    if let Some(r) = map.get("required") {
        let arr = r
            .as_array()
            .ok_or("`required` must be an array of property-name strings")?;
        if arr.iter().any(|it| !it.is_string()) {
            return Err("`required` must contain only strings".to_string());
        }
    }
    if let Some(p) = map.get("properties") {
        let obj = p.as_object().ok_or("`properties` must be an object")?;
        for sub in obj.values() {
            check_schema(sub, root, patterns)?;
        }
    }
    if let Some(ap) = map.get("additionalProperties") {
        match ap {
            Value::Bool(_) => {}
            Value::Object(_) => check_schema(ap, root, patterns)?,
            _ => return Err("`additionalProperties` must be a boolean or a schema object".to_string()),
        }
    }
    if let Some(items) = map.get("items") {
        match items {
            Value::Bool(_) | Value::Object(_) => check_schema(items, root, patterns)?,
            Value::Array(arr) => {
                for s in arr {
                    check_schema(s, root, patterns)?;
                }
            }
            _ => return Err("`items` must be a schema or an array of schemas".to_string()),
        }
    }
    if let Some(e) = map.get("enum") {
        if !e.is_array() {
            return Err("`enum` must be an array".to_string());
        }
    }
    for k in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
        if let Some(v) = map.get(k) {
            // draft-4 uses boolean exclusiveMinimum/Maximum — accepted silently
            // (its combined semantics are unsupported), everything else must be
            // a number.
            let ok = v.is_number()
                || ((k == "exclusiveMinimum" || k == "exclusiveMaximum") && v.is_boolean());
            if !ok {
                return Err(format!("`{k}` must be a number"));
            }
        }
    }
    for k in ["minLength", "maxLength"] {
        if let Some(v) = map.get(k) {
            if v.as_u64().is_none() {
                return Err(format!("`{k}` must be a non-negative integer"));
            }
        }
    }
    if let Some(pat) = map.get("pattern") {
        let s = pat.as_str().ok_or("`pattern` must be a string")?;
        if !patterns.contains_key(s) {
            let re = Regex::new(s)
                .map_err(|e| format!("`pattern` is not a valid regular expression: {e}"))?;
            patterns.insert(s.to_string(), re);
        }
    }
    if let Some(r) = map.get("$ref") {
        let s = r.as_str().ok_or("`$ref` must be a string")?;
        resolve_ref(s, root)
            .ok_or_else(|| format!("`$ref` could not be resolved (internal refs only): {s}"))?;
    }
    for defk in ["definitions", "$defs"] {
        if let Some(Value::Object(obj)) = map.get(defk) {
            for sub in obj.values() {
                check_schema(sub, root, patterns)?;
            }
        }
    }
    Ok(())
}

fn check_type_decl(t: &Value) -> Result<(), String> {
    let known = |s: &str| KNOWN_TYPES.contains(&s);
    match t {
        Value::String(s) => {
            if known(s) {
                Ok(())
            } else {
                Err(format!("`type` has unknown value \"{s}\""))
            }
        }
        Value::Array(arr) => {
            for it in arr {
                let s = it
                    .as_str()
                    .ok_or("`type` array must contain only type-name strings")?;
                if !known(s) {
                    return Err(format!("`type` has unknown value \"{s}\""));
                }
            }
            Ok(())
        }
        _ => Err("`type` must be a string or an array of strings".to_string()),
    }
}

/// Validate `instance` against `schema`, pushing each failure onto `errors`.
fn validate_value(
    schema: &Value,
    instance: &Value,
    path: &str,
    root: &Value,
    patterns: &HashMap<String, Regex>,
    errors: &mut Vec<RecordError>,
) {
    let map = match schema {
        Value::Bool(true) => return,
        Value::Bool(false) => {
            errors.push(RecordError {
                path: path.to_string(),
                keyword: String::new(),
                message: format!("False schema does not allow {}", show(instance)),
            });
            return;
        }
        Value::Object(map) => map,
        _ => return,
    };

    // `$ref` short-circuits sibling keywords (draft-4/7 behavior).
    if let Some(r) = map.get("$ref").and_then(Value::as_str) {
        if let Some(target) = resolve_ref(r, root) {
            validate_value(target, instance, path, root, patterns, errors);
        }
        return;
    }

    if let Some(t) = map.get("type") {
        if !type_matches(instance, t) {
            errors.push(RecordError {
                path: path.to_string(),
                keyword: "type".to_string(),
                message: type_message(instance, t),
            });
        }
    }

    if let Some(opts) = map.get("enum").and_then(Value::as_array) {
        if !opts.iter().any(|o| o == instance) {
            errors.push(RecordError {
                path: path.to_string(),
                keyword: "enum".to_string(),
                message: format!(
                    "{} is not one of {}",
                    show(instance),
                    show(map.get("enum").unwrap())
                ),
            });
        }
    }

    if let Some(c) = map.get("const") {
        if c != instance {
            errors.push(RecordError {
                path: path.to_string(),
                keyword: "const".to_string(),
                message: format!("{} was expected", show(c)),
            });
        }
    }

    // Numeric bounds — only meaningful for numbers.
    if let Some(num) = instance.as_f64() {
        if let Some(lim) = map.get("minimum") {
            if matches!(lim.as_f64(), Some(l) if num < l) {
                errors.push(bound_err(path, "minimum", instance, lim, "is less than the minimum of"));
            }
        }
        if let Some(lim) = map.get("maximum") {
            if matches!(lim.as_f64(), Some(l) if num > l) {
                errors.push(bound_err(path, "maximum", instance, lim, "is greater than the maximum of"));
            }
        }
        if let Some(lim) = map.get("exclusiveMinimum") {
            if matches!(lim.as_f64(), Some(l) if num <= l) {
                errors.push(bound_err(
                    path,
                    "exclusiveMinimum",
                    instance,
                    lim,
                    "is less than or equal to the minimum of",
                ));
            }
        }
        if let Some(lim) = map.get("exclusiveMaximum") {
            if matches!(lim.as_f64(), Some(l) if num >= l) {
                errors.push(bound_err(
                    path,
                    "exclusiveMaximum",
                    instance,
                    lim,
                    "is greater than or equal to the maximum of",
                ));
            }
        }
    }

    // String length + pattern — only meaningful for strings.
    if let Some(s) = instance.as_str() {
        let len = s.chars().count() as u64;
        if let Some(lim) = map.get("minLength").and_then(Value::as_u64) {
            if len < lim {
                errors.push(RecordError {
                    path: path.to_string(),
                    keyword: "minLength".to_string(),
                    message: format!(
                        "{} is shorter than {} character{}",
                        show(instance),
                        lim,
                        if lim == 1 { "" } else { "s" }
                    ),
                });
            }
        }
        if let Some(lim) = map.get("maxLength").and_then(Value::as_u64) {
            if len > lim {
                errors.push(RecordError {
                    path: path.to_string(),
                    keyword: "maxLength".to_string(),
                    message: format!(
                        "{} is longer than {} character{}",
                        show(instance),
                        lim,
                        if lim == 1 { "" } else { "s" }
                    ),
                });
            }
        }
        if let Some(pat) = map.get("pattern").and_then(Value::as_str) {
            if let Some(re) = patterns.get(pat) {
                if !re.is_match(s) {
                    errors.push(RecordError {
                        path: path.to_string(),
                        keyword: "pattern".to_string(),
                        message: format!(r#"{} does not match "{}""#, show(instance), pat),
                    });
                }
            }
        }
    }

    // Object keywords — only meaningful for objects.
    if let Value::Object(obj) = instance {
        if let Some(reqs) = map.get("required").and_then(Value::as_array) {
            for rq in reqs {
                if let Some(name) = rq.as_str() {
                    if !obj.contains_key(name) {
                        errors.push(RecordError {
                            path: path.to_string(),
                            keyword: "required".to_string(),
                            message: format!(
                                "{} is a required property",
                                Value::String(name.to_string())
                            ),
                        });
                    }
                }
            }
        }

        if let Some(props) = map.get("properties").and_then(Value::as_object) {
            for (k, sub) in props {
                if let Some(val) = obj.get(k) {
                    let cp = format!("{}/{}", path, escape_ptr(k));
                    validate_value(sub, val, &cp, root, patterns, errors);
                }
            }
        }

        if let Some(ap) = map.get("additionalProperties") {
            let known: std::collections::HashSet<&str> = map
                .get("properties")
                .and_then(Value::as_object)
                .map(|p| p.keys().map(String::as_str).collect())
                .unwrap_or_default();
            match ap {
                Value::Bool(false) => {
                    let unexpected: Vec<&String> =
                        obj.keys().filter(|k| !known.contains(k.as_str())).collect();
                    if !unexpected.is_empty() {
                        errors.push(RecordError {
                            path: path.to_string(),
                            keyword: "additionalProperties".to_string(),
                            message: additional_props_msg(&unexpected),
                        });
                    }
                }
                Value::Object(_) => {
                    for (k, val) in obj {
                        if !known.contains(k.as_str()) {
                            let cp = format!("{}/{}", path, escape_ptr(k));
                            validate_value(ap, val, &cp, root, patterns, errors);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Array `items`.
    if let (Some(items), Value::Array(arr)) = (map.get("items"), instance) {
        match items {
            Value::Array(schemas) => {
                for (i, el) in arr.iter().enumerate() {
                    if let Some(sub) = schemas.get(i) {
                        let cp = format!("{path}/{i}");
                        validate_value(sub, el, &cp, root, patterns, errors);
                    }
                }
            }
            _ => {
                for (i, el) in arr.iter().enumerate() {
                    let cp = format!("{path}/{i}");
                    validate_value(items, el, &cp, root, patterns, errors);
                }
            }
        }
    }
}

fn bound_err(path: &str, keyword: &str, instance: &Value, limit: &Value, phrase: &str) -> RecordError {
    RecordError {
        path: path.to_string(),
        keyword: keyword.to_string(),
        message: format!("{} {} {}", show(instance), phrase, show(limit)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn errs(schema: Value, instance: Value) -> Vec<RecordError> {
        CompiledSchema::build(&schema).unwrap().errors(&instance)
    }

    #[test]
    fn type_message_matches_jsonschema() {
        let e = errs(json!({"type": "integer"}), json!("2"));
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].keyword, "type");
        assert_eq!(e[0].message, r#""2" is not of type "integer""#);
    }

    #[test]
    fn required_message_and_root_path() {
        let e = errs(json!({"required": ["id"]}), json!({"name": "x"}));
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].path, "");
        assert_eq!(e[0].keyword, "required");
        assert_eq!(e[0].message, r#""id" is a required property"#);
    }

    #[test]
    fn nested_property_path_and_maximum() {
        let schema = json!({"properties": {"age": {"type": "integer", "maximum": 120}}});
        let e = errs(schema, json!({"age": 200}));
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].path, "/age");
        assert_eq!(e[0].keyword, "maximum");
        assert_eq!(e[0].message, "200 is greater than the maximum of 120");
    }

    #[test]
    fn minimum_and_exclusive() {
        assert_eq!(
            errs(json!({"minimum": 0}), json!(-1))[0].message,
            "-1 is less than the minimum of 0"
        );
        assert_eq!(
            errs(json!({"exclusiveMinimum": 0}), json!(0))[0].message,
            "0 is less than or equal to the minimum of 0"
        );
        assert_eq!(
            errs(json!({"exclusiveMaximum": 10}), json!(10))[0].message,
            "10 is greater than or equal to the maximum of 10"
        );
    }

    #[test]
    fn integer_accepts_whole_float() {
        assert!(errs(json!({"type": "integer"}), json!(1.0)).is_empty());
        assert_eq!(errs(json!({"type": "integer"}), json!(1.5)).len(), 1);
    }

    #[test]
    fn enum_and_const() {
        let e = errs(json!({"enum": ["a", "b"]}), json!("c"));
        assert_eq!(e[0].keyword, "enum");
        assert_eq!(e[0].message, r#""c" is not one of ["a","b"]"#);
        let e = errs(json!({"const": 5}), json!(6));
        assert_eq!(e[0].keyword, "const");
        assert_eq!(e[0].message, "5 was expected");
    }

    #[test]
    fn pattern_and_length() {
        let e = errs(json!({"pattern": "^a"}), json!("xyz"));
        assert_eq!(e[0].keyword, "pattern");
        assert_eq!(e[0].message, r#""xyz" does not match "^a""#);
        assert_eq!(
            errs(json!({"minLength": 3}), json!("hi"))[0].message,
            r#""hi" is shorter than 3 characters"#
        );
        assert_eq!(
            errs(json!({"maxLength": 1}), json!("hi"))[0].message,
            r#""hi" is longer than 1 character"#
        );
    }

    #[test]
    fn additional_properties_false() {
        let schema = json!({"properties": {"a": {}}, "additionalProperties": false});
        let e = errs(schema, json!({"a": 1, "b": 2, "c": 3}));
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].keyword, "additionalProperties");
        assert_eq!(
            e[0].message,
            "Additional properties are not allowed ('b', 'c' were unexpected)"
        );
    }

    #[test]
    fn additional_properties_schema() {
        let schema = json!({"additionalProperties": {"type": "string"}});
        let e = errs(schema, json!({"a": 1}));
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].path, "/a");
        assert_eq!(e[0].keyword, "type");
    }

    #[test]
    fn items_single_and_tuple() {
        let e = errs(json!({"items": {"type": "integer"}}), json!([1, "x", 3]));
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].path, "/1");
        let tuple = json!({"items": [{"type": "integer"}, {"type": "string"}]});
        let e = errs(tuple, json!([1, 2]));
        assert_eq!(e[0].path, "/1");
        assert_eq!(e[0].keyword, "type");
    }

    #[test]
    fn internal_ref_resolves() {
        let schema = json!({
            "$defs": {"pos": {"type": "integer", "minimum": 0}},
            "properties": {"n": {"$ref": "#/$defs/pos"}}
        });
        assert!(errs(schema.clone(), json!({"n": 5})).is_empty());
        let e = errs(schema, json!({"n": -1}));
        assert_eq!(e[0].path, "/n");
        assert_eq!(e[0].keyword, "minimum");
    }

    #[test]
    fn escaped_pointer_segment() {
        let schema = json!({"properties": {"a/b": {"type": "integer"}}});
        let e = errs(schema, json!({"a/b": "x"}));
        assert_eq!(e[0].path, "/a~1b");
    }

    #[test]
    fn bad_type_decl_rejected() {
        assert!(CompiledSchema::build(&json!({"type": 5})).is_err());
        assert!(CompiledSchema::build(&json!({"type": "widget"})).is_err());
        assert!(CompiledSchema::build(&json!({"pattern": "("}).clone()).is_err());
        assert!(CompiledSchema::build(&json!({"$ref": "#/nope"})).is_err());
    }

    #[test]
    fn unknown_keywords_are_annotations() {
        // format/title/etc. must not cause errors or failures.
        let schema = json!({"type": "string", "format": "email", "title": "x"});
        assert!(CompiledSchema::build(&schema).is_ok());
        assert!(errs(json!({"format": "email"}), json!("not-an-email")).is_empty());
    }
}
