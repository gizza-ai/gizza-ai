//! json-schema-to-sample core — build the smallest JSON instance that satisfies
//! a JSON Schema, honouring `required`, `default`, `const`, `enum`, `examples`
//! and the common assertion keywords.
//!
//! Pure Rust (serde_json only) → runs on every backend including the chat
//! Service Worker. Fully DETERMINISTIC: the same schema + options always yield
//! byte-identical output (no clock, no RNG), so the result is safe to paste into
//! API docs, fixtures and golden tests.
//!
//! In-model JSON Schema subset (draft-07 / 2019-09 / 2020-12 flavours):
//! - types `object`, `array`, `string`, `integer`, `number`, `boolean`, `null`,
//!   plus union `"type": ["string", "null"]` (first non-null member wins) and
//!   the boolean schemas `true` / `false`;
//! - value sources, in precedence order: `const` → `default` → `examples[0]` →
//!   `example` (OpenAPI 3.0) → `enum[0]` → generated from `type`/`format`;
//! - objects: `properties`, `required`, `additionalProperties` (as a schema);
//! - arrays: `items`, `prefixItems`, draft-07 tuple `items: [...]` +
//!   `additionalItems`, `minItems`, `maxItems`, `uniqueItems`;
//! - strings: `format` (email, uuid, date, date-time, time, duration, uri,
//!   uri-reference, hostname, ipv4, ipv6, byte, json-pointer, regex, …),
//!   `minLength`, `maxLength`;
//! - numbers: `minimum`, `maximum`, `exclusiveMinimum`/`exclusiveMaximum` (both
//!   the draft-04 boolean form and the numeric form), `multipleOf`;
//! - composition: `allOf` (deep merge), `oneOf`/`anyOf` (first branch),
//!   local `$ref` into `$defs` / `definitions` / any JSON pointer in the same
//!   document, with recursion cut off at the first repeat.
//!
//! Deliberately NOT synthesised (documented on the page, never silently faked):
//! `pattern`/`patternProperties` (no regex generator), `not`, `if`/`then`/`else`,
//! `dependentSchemas`/`dependencies`, `propertyNames`, `contains`,
//! `minProperties`/`maxProperties`, and remote (`http…`/file) `$ref`s — a remote
//! `$ref` is a hard error because there is no network fetch in a browser-local
//! block.

use serde_json::{Map, Number, Value};

/// Largest schema accepted, in bytes.
pub const MAX_SCHEMA_BYTES: usize = 1_048_576;
/// Largest `array_items` accepted.
pub const MAX_ARRAY_ITEMS: u32 = 50;
/// Nesting depth after which generation stops and emits `null`.
const MAX_DEPTH: usize = 32;
/// Total generated nodes allowed, guarding against schema blow-up.
const MAX_NODES: usize = 50_000;
/// Largest count/length a schema keyword (`minItems`, `minLength`, …) may ask
/// this tool to materialise.
const MAX_COUNT_KEYWORD: usize = 10_000;

/// Generation options (mirrors the descriptor params).
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Emit optional properties too (default: required-only).
    pub include_optional: bool,
    /// How many entries to put in an unconstrained array.
    pub array_items: u32,
    /// Pretty-print with 2-space indentation.
    pub pretty: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            include_optional: false,
            array_items: 1,
            pretty: true,
        }
    }
}

/// Generate a minimal conforming JSON instance for `schema_text`.
pub fn sample(schema_text: &str, opts: Options) -> Result<String, String> {
    if schema_text.trim().is_empty() {
        return Err("schema is empty: paste a JSON Schema object such as {\"type\":\"object\",\"properties\":{\"id\":{\"type\":\"integer\"}},\"required\":[\"id\"]}".into());
    }
    if schema_text.len() > MAX_SCHEMA_BYTES {
        return Err(format!(
            "schema is {} bytes, over the {} byte limit for this browser-local tool",
            schema_text.len(),
            MAX_SCHEMA_BYTES
        ));
    }
    if opts.array_items > MAX_ARRAY_ITEMS {
        return Err(format!(
            "array_items is {}, over the maximum of {}",
            opts.array_items, MAX_ARRAY_ITEMS
        ));
    }
    let root: Value = serde_json::from_str(schema_text)
        .map_err(|e| format!("schema is not valid JSON: {e}"))?;
    if !matches!(root, Value::Object(_) | Value::Bool(_)) {
        return Err(format!(
            "expected a JSON Schema object, found {}",
            kind_of(&root)
        ));
    }
    let mut generator = Gen {
        root: &root,
        opts,
        nodes: 0,
    };
    let mut refs: Vec<String> = Vec::new();
    let value = generator.generate(&root, 0, &mut refs)?;
    if opts.pretty {
        serde_json::to_string_pretty(&value).map_err(|e| format!("could not serialise sample: {e}"))
    } else {
        serde_json::to_string(&value).map_err(|e| format!("could not serialise sample: {e}"))
    }
}

struct Gen<'a> {
    root: &'a Value,
    opts: Options,
    nodes: usize,
}

impl Gen<'_> {
    fn generate(
        &mut self,
        schema: &Value,
        depth: usize,
        refs: &mut Vec<String>,
    ) -> Result<Value, String> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(format!(
                "sample grew past {MAX_NODES} values; lower array_items or turn off include_optional"
            ));
        }
        let map = match schema {
            // `true` accepts anything; `null` is the smallest such instance.
            Value::Bool(true) => return Ok(Value::Null),
            Value::Bool(false) => {
                return Err(
                    "schema `false` accepts no instance, so no example can satisfy it".into(),
                )
            }
            Value::Object(m) => m,
            other => {
                return Err(format!(
                    "expected a JSON Schema object or boolean, found {}",
                    kind_of(other)
                ))
            }
        };
        if depth > MAX_DEPTH {
            return Ok(Value::Null);
        }

        // --- composition: $ref, allOf, oneOf/anyOf --------------------------
        if let Some(rv) = map.get("$ref") {
            let target = rv
                .as_str()
                .ok_or_else(|| format!("\"$ref\" must be a string, found {}", kind_of(rv)))?;
            // A repeated $ref means the schema is recursive: stop with null.
            if refs.iter().any(|r| r == target) {
                return Ok(Value::Null);
            }
            let resolved = self.resolve_ref(target)?;
            let merged = merge_schema(&resolved, &Value::Object(without(map, &["$ref"])));
            refs.push(target.to_string());
            let out = self.generate(&merged, depth + 1, refs);
            refs.pop();
            return out;
        }
        if let Some(all_of) = map.get("allOf") {
            let list = all_of
                .as_array()
                .ok_or_else(|| format!("\"allOf\" must be an array, found {}", kind_of(all_of)))?;
            let mut merged = Value::Object(without(map, &["allOf"]));
            for branch in list {
                merged = merge_schema(&merged, branch);
            }
            return self.generate(&merged, depth + 1, refs);
        }
        for key in ["oneOf", "anyOf"] {
            let Some(node) = map.get(key) else { continue };
            let list = node
                .as_array()
                .ok_or_else(|| format!("\"{key}\" must be an array, found {}", kind_of(node)))?;
            let first = list
                .first()
                .ok_or_else(|| format!("\"{key}\" is empty: it needs at least one branch schema"))?;
            let merged = merge_schema(&Value::Object(without(map, &[key])), first);
            return self.generate(&merged, depth + 1, refs);
        }

        // --- explicit values beat anything we could invent -------------------
        if let Some(v) = map.get("const") {
            return Ok(v.clone());
        }
        if let Some(v) = map.get("default") {
            return Ok(v.clone());
        }
        if let Some(Value::Array(examples)) = map.get("examples") {
            if let Some(first) = examples.first() {
                return Ok(first.clone());
            }
        }
        if let Some(v) = map.get("example") {
            return Ok(v.clone());
        }
        if let Some(node) = map.get("enum") {
            let list = node
                .as_array()
                .ok_or_else(|| format!("\"enum\" must be an array, found {}", kind_of(node)))?;
            let first = list
                .first()
                .ok_or_else(|| "\"enum\" is empty: it accepts no value".to_string())?;
            return Ok(first.clone());
        }

        // --- generate from the type -----------------------------------------
        match pick_type(map)?.as_str() {
            "object" => self.gen_object(map, depth, refs),
            "array" => self.gen_array(map, depth, refs),
            "string" => gen_string(map),
            "integer" => gen_number(map, true),
            "number" => gen_number(map, false),
            "boolean" => Ok(Value::Bool(true)),
            "null" => Ok(Value::Null),
            other => Err(format!(
                "unsupported \"type\": {other:?} — expected object, array, string, integer, number, boolean or null"
            )),
        }
    }

    fn gen_object(
        &mut self,
        map: &Map<String, Value>,
        depth: usize,
        refs: &mut Vec<String>,
    ) -> Result<Value, String> {
        let required: Vec<&str> = match map.get("required") {
            Some(Value::Array(list)) => list.iter().filter_map(Value::as_str).collect(),
            Some(other) => {
                return Err(format!(
                    "\"required\" must be an array of property names, found {}",
                    kind_of(other)
                ))
            }
            None => Vec::new(),
        };
        let mut out = Map::new();
        if let Some(node) = map.get("properties") {
            let props = node.as_object().ok_or_else(|| {
                format!("\"properties\" must be an object, found {}", kind_of(node))
            })?;
            for (name, sub) in props {
                let has_default = sub.get("default").is_some();
                if required.contains(&name.as_str()) || self.opts.include_optional || has_default {
                    let value = self.generate(sub, depth + 1, refs)?;
                    out.insert(name.clone(), value);
                }
            }
        }
        // A required property with no schema of its own still has to appear.
        let extra = map.get("additionalProperties");
        for name in &required {
            if out.contains_key(*name) {
                continue;
            }
            let value = match extra {
                Some(s @ Value::Object(_)) => self.generate(s, depth + 1, refs)?,
                _ => Value::Null,
            };
            out.insert((*name).to_string(), value);
        }
        // A free-form object gets one illustrative key, but only when the caller
        // asked for optional content — otherwise `{}` is the minimal instance.
        if out.is_empty() && self.opts.include_optional {
            if let Some(s @ Value::Object(_)) = extra {
                let value = self.generate(s, depth + 1, refs)?;
                out.insert("additionalProp1".to_string(), value);
            }
        }
        Ok(Value::Object(out))
    }

    fn gen_array(
        &mut self,
        map: &Map<String, Value>,
        depth: usize,
        refs: &mut Vec<String>,
    ) -> Result<Value, String> {
        let min_items = usize_of(map, "minItems")?.unwrap_or(0);
        let max_items = usize_of(map, "maxItems")?;
        if let Some(max) = max_items {
            if min_items > max {
                return Err(format!(
                    "conflicting bounds: minItems {min_items} is greater than maxItems {max}"
                ));
            }
        }
        // draft-07 tuples put the per-position schemas in `items`; 2020-12 uses
        // `prefixItems` and keeps `items` for the rest.
        let (prefix, rest): (Vec<&Value>, Option<&Value>) = match map.get("prefixItems") {
            Some(Value::Array(list)) => (list.iter().collect(), map.get("items")),
            Some(other) => {
                return Err(format!(
                    "\"prefixItems\" must be an array, found {}",
                    kind_of(other)
                ))
            }
            None => match map.get("items") {
                Some(Value::Array(list)) => (list.iter().collect(), map.get("additionalItems")),
                other => (Vec::new(), other),
            },
        };

        let mut count = self.opts.array_items as usize;
        if count < min_items {
            count = min_items;
        }
        if count < prefix.len() {
            // Show every tuple position; a partial tuple is legal but useless.
            count = prefix.len();
        }
        if let Some(max) = max_items {
            if count > max {
                count = max;
            }
        }

        let mut items = Vec::with_capacity(count);
        for i in 0..count {
            let schema = if i < prefix.len() {
                Some(prefix[i])
            } else {
                rest
            };
            match schema {
                Some(s) => {
                    let v = self.generate(s, depth + 1, refs)?;
                    items.push(v);
                }
                // No item schema: only pad when minItems demands entries.
                None if i < min_items => items.push(Value::Null),
                None => break,
            }
        }
        if map.get("uniqueItems") == Some(&Value::Bool(true)) {
            make_unique(&mut items);
        }
        Ok(Value::Array(items))
    }

    fn resolve_ref(&self, target: &str) -> Result<Value, String> {
        if target == "#" {
            return Ok(self.root.clone());
        }
        if !target.starts_with("#/") {
            return Err(format!(
                "unsupported $ref {target:?}: only local pointers into this document (\"#/$defs/Name\", \"#/definitions/Name\") are resolved — remote and named-anchor refs are not fetched"
            ));
        }
        let pointer = percent_decode(&target[1..]);
        self.root.pointer(&pointer).cloned().ok_or_else(|| {
            format!("unresolved $ref {target:?}: no such location in this schema document")
        })
    }
}

// ---------------------------------------------------------------------------
// scalar generation
// ---------------------------------------------------------------------------

fn gen_string(map: &Map<String, Value>) -> Result<Value, String> {
    let format = map.get("format").and_then(Value::as_str).unwrap_or("");
    let base = match format {
        "email" | "idn-email" => "user@example.com",
        "uuid" => "3fa85f64-5717-4562-b3fc-2c963f66afa6",
        "date" => "2026-01-01",
        "date-time" => "2026-01-01T12:00:00Z",
        "time" => "12:00:00Z",
        "duration" => "P1D",
        "uri" | "url" | "iri" => "https://example.com/path",
        "uri-reference" | "iri-reference" => "/path",
        "uri-template" => "https://example.com/{id}",
        "hostname" | "idn-hostname" => "example.com",
        "ipv4" => "192.0.2.1",
        "ipv6" => "2001:db8::1",
        "byte" => "c3RyaW5n",
        "json-pointer" | "relative-json-pointer" => "/foo/bar",
        "regex" => "^[a-z]+$",
        _ => "string",
    };
    let mut s = base.to_string();
    // Length assertions win over `format`, which is only an annotation unless a
    // validator opts in; padding/truncating keeps the instance valid.
    if let Some(min) = usize_of(map, "minLength")? {
        while s.chars().count() < min {
            s.push('x');
        }
    }
    if let Some(max) = usize_of(map, "maxLength")? {
        if s.chars().count() > max {
            s = s.chars().take(max).collect();
        }
    }
    Ok(Value::String(s))
}

fn gen_number(map: &Map<String, Value>, integer: bool) -> Result<Value, String> {
    let step = 1.0_f64;
    // draft-04 spells exclusivity as a boolean flag next to minimum/maximum;
    // draft-06 and later put the bound in exclusiveMinimum/exclusiveMaximum.
    let excl_min_flag = map.get("exclusiveMinimum") == Some(&Value::Bool(true));
    let excl_max_flag = map.get("exclusiveMaximum") == Some(&Value::Bool(true));

    let mut lower: Option<f64> = None;
    if let Some(min) = f64_of(map, "minimum")? {
        lower = Some(if excl_min_flag { min + step } else { min });
    }
    if let Some(min) = f64_of(map, "exclusiveMinimum")? {
        let bound = min + step;
        lower = Some(lower.map_or(bound, |l: f64| l.max(bound)));
    }
    let mut upper: Option<f64> = None;
    if let Some(max) = f64_of(map, "maximum")? {
        upper = Some(if excl_max_flag { max - step } else { max });
    }
    if let Some(max) = f64_of(map, "exclusiveMaximum")? {
        let bound = max - step;
        upper = Some(upper.map_or(bound, |u: f64| u.min(bound)));
    }
    if let (Some(low), Some(high)) = (lower, upper) {
        if low > high {
            return Err(format!(
                "conflicting bounds: no value satisfies both the lower bound {low} and the upper bound {high}"
            ));
        }
    }

    let mut v = 0.0_f64;
    if let Some(low) = lower {
        if low > v {
            v = low;
        }
    }
    if let Some(high) = upper {
        if v > high {
            v = high;
        }
    }
    if integer {
        v = v.ceil();
        if let Some(high) = upper {
            if v > high {
                v = high.floor();
            }
        }
    }
    if let Some(multiple) = f64_of(map, "multipleOf")? {
        if multiple <= 0.0 {
            return Err(format!(
                "\"multipleOf\" must be greater than 0, found {multiple}"
            ));
        }
        v = (v / multiple).ceil() * multiple;
        if let Some(high) = upper {
            if v > high {
                v = (high / multiple).floor() * multiple;
            }
        }
    }
    if integer {
        Ok(Value::Number(Number::from(v as i64)))
    } else {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| format!("numeric bounds produced a non-finite value: {v}"))
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn pick_type(map: &Map<String, Value>) -> Result<String, String> {
    match map.get("type") {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(Value::Array(list)) => {
            let mut fallback: Option<String> = None;
            for entry in list {
                let s = entry.as_str().ok_or_else(|| {
                    format!("\"type\" list entries must be strings, found {}", kind_of(entry))
                })?;
                if s != "null" {
                    return Ok(s.to_string());
                }
                fallback = Some(s.to_string());
            }
            fallback.ok_or_else(|| "\"type\" is an empty list: it accepts no value".to_string())
        }
        Some(other) => Err(format!(
            "\"type\" must be a string or a list of strings, found {}",
            kind_of(other)
        )),
        None => {
            // No `type`: infer one from the structural keywords that are present.
            if ["properties", "required", "additionalProperties", "patternProperties"]
                .iter()
                .any(|k| map.contains_key(*k))
            {
                Ok("object".into())
            } else if ["items", "prefixItems", "minItems", "maxItems"]
                .iter()
                .any(|k| map.contains_key(*k))
            {
                Ok("array".into())
            } else {
                Ok("null".into())
            }
        }
    }
}

fn merge_schema(base: &Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Object(b), Value::Object(o)) => {
            let mut out = b.clone();
            for (key, value) in o {
                if key == "required" {
                    if let (Some(Value::Array(existing)), Value::Array(added)) =
                        (out.get(key), value)
                    {
                        let mut merged = existing.clone();
                        for entry in added {
                            if !merged.contains(entry) {
                                merged.push(entry.clone());
                            }
                        }
                        out.insert(key.clone(), Value::Array(merged));
                        continue;
                    }
                }
                let merged = match (out.get(key), value) {
                    (Some(existing @ Value::Object(_)), Value::Object(_)) => {
                        merge_schema(existing, value)
                    }
                    _ => value.clone(),
                };
                out.insert(key.clone(), merged);
            }
            Value::Object(out)
        }
        _ => overlay.clone(),
    }
}

fn without(map: &Map<String, Value>, keys: &[&str]) -> Map<String, Value> {
    let mut out = map.clone();
    for key in keys {
        out.remove(*key);
    }
    out
}

fn make_unique(items: &mut Vec<Value>) {
    for i in 1..items.len() {
        let mut attempt = 0;
        while items[..i].contains(&items[i]) {
            attempt += 1;
            items[i] = match &items[i] {
                Value::String(s) => Value::String(format!("{s}-{}", i + attempt - 1)),
                Value::Number(n) if n.is_i64() => {
                    Value::Number(Number::from(n.as_i64().unwrap_or(0) + 1))
                }
                Value::Number(n) => Number::from_f64(n.as_f64().unwrap_or(0.0) + 1.0)
                    .map(Value::Number)
                    .unwrap_or(Value::Null),
                other => {
                    let mut wrapped = other.clone();
                    if let Value::Object(map) = &mut wrapped {
                        map.insert("_index".into(), Value::Number(Number::from(i as i64)));
                    }
                    wrapped
                }
            };
            if attempt > 64 {
                break;
            }
        }
    }
    items.dedup();
}

fn usize_of(map: &Map<String, Value>, key: &str) -> Result<Option<usize>, String> {
    match map.get(key) {
        None => Ok(None),
        Some(Value::Number(n)) => {
            let v = n
                .as_f64()
                .ok_or_else(|| format!("\"{key}\" is not a usable number"))?;
            if v < 0.0 {
                return Err(format!("\"{key}\" must be zero or greater, found {v}"));
            }
            if v > MAX_COUNT_KEYWORD as f64 {
                return Err(format!(
                    "\"{key}\" is {v}, over the {MAX_COUNT_KEYWORD} limit this tool will materialise"
                ));
            }
            Ok(Some(v as usize))
        }
        Some(other) => Err(format!(
            "\"{key}\" must be a number, found {}",
            kind_of(other)
        )),
    }
}

fn f64_of(map: &Map<String, Value>, key: &str) -> Result<Option<f64>, String> {
    match map.get(key) {
        None => Ok(None),
        Some(Value::Bool(_)) => Ok(None), // draft-04 exclusive* flag, handled separately
        Some(Value::Number(n)) => Ok(n.as_f64()),
        Some(other) => Err(format!(
            "\"{key}\" must be a number, found {}",
            kind_of(other)
        )),
    }
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compact(schema: &str, opts: Options) -> String {
        sample(
            schema,
            Options {
                pretty: false,
                ..opts
            },
        )
        .expect("sample")
    }

    #[test]
    fn required_only_by_default() {
        let schema = r#"{
            "type":"object",
            "properties":{
                "id":{"type":"integer"},
                "name":{"type":"string"},
                "nickname":{"type":"string"}
            },
            "required":["id","name"]
        }"#;
        assert_eq!(
            compact(schema, Options::default()),
            r#"{"id":0,"name":"string"}"#
        );
    }

    #[test]
    fn include_optional_adds_every_property() {
        let schema = r#"{
            "type":"object",
            "properties":{"id":{"type":"integer"},"nickname":{"type":"string"}},
            "required":["id"]
        }"#;
        assert_eq!(
            compact(
                schema,
                Options {
                    include_optional: true,
                    ..Options::default()
                }
            ),
            r#"{"id":0,"nickname":"string"}"#
        );
    }

    #[test]
    fn defaults_are_used_and_pull_in_optional_properties() {
        let schema = r#"{
            "type":"object",
            "properties":{
                "id":{"type":"integer"},
                "role":{"type":"string","default":"viewer"},
                "other":{"type":"string"}
            },
            "required":["id"]
        }"#;
        assert_eq!(
            compact(schema, Options::default()),
            r#"{"id":0,"role":"viewer"}"#
        );
    }

    #[test]
    fn value_precedence_const_default_examples_enum() {
        assert_eq!(
            compact(r#"{"const":"fixed","default":"d","enum":["e"]}"#, Options::default()),
            r#""fixed""#
        );
        assert_eq!(
            compact(r#"{"default":"d","examples":["x"],"enum":["e"]}"#, Options::default()),
            r#""d""#
        );
        assert_eq!(
            compact(r#"{"examples":["x"],"enum":["e"]}"#, Options::default()),
            r#""x""#
        );
        assert_eq!(
            compact(r#"{"example":"o","enum":["e"]}"#, Options::default()),
            r#""o""#
        );
        assert_eq!(
            compact(r#"{"type":"string","enum":["red","green"]}"#, Options::default()),
            r#""red""#
        );
    }

    #[test]
    fn local_ref_into_defs_is_resolved() {
        let schema = r##"{
            "type":"object",
            "properties":{"user":{"$ref":"#/$defs/User"}},
            "required":["user"],
            "$defs":{"User":{"type":"object","properties":{"id":{"type":"integer"}},"required":["id"]}}
        }"##;
        assert_eq!(compact(schema, Options::default()), r#"{"user":{"id":0}}"#);
    }

    #[test]
    fn draft07_definitions_ref_is_resolved() {
        let schema = r##"{
            "type":"object",
            "properties":{"pet":{"$ref":"#/definitions/Pet"}},
            "required":["pet"],
            "definitions":{"Pet":{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}}
        }"##;
        assert_eq!(
            compact(schema, Options::default()),
            r#"{"pet":{"name":"string"}}"#
        );
    }

    #[test]
    fn recursive_ref_stops_at_null() {
        let schema = r##"{
            "$ref":"#/$defs/Node",
            "$defs":{"Node":{"type":"object","properties":{"next":{"$ref":"#/$defs/Node"}},"required":["next"]}}
        }"##;
        assert_eq!(compact(schema, Options::default()), r#"{"next":null}"#);
    }

    #[test]
    fn all_of_branches_are_merged() {
        let schema = r#"{
            "allOf":[
                {"type":"object","properties":{"a":{"type":"string"}},"required":["a"]},
                {"type":"object","properties":{"b":{"type":"integer"}},"required":["b"]}
            ]
        }"#;
        assert_eq!(compact(schema, Options::default()), r#"{"a":"string","b":0}"#);
    }

    #[test]
    fn one_of_takes_the_first_branch() {
        let schema = r#"{"oneOf":[{"type":"integer","minimum":5},{"type":"string"}]}"#;
        assert_eq!(compact(schema, Options::default()), "5");
    }

    #[test]
    fn arrays_honour_item_count_and_bounds() {
        let schema = r#"{"type":"array","items":{"type":"string"}}"#;
        assert_eq!(
            compact(
                schema,
                Options {
                    array_items: 2,
                    ..Options::default()
                }
            ),
            r#"["string","string"]"#
        );
        let bounded = r#"{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3}"#;
        assert_eq!(compact(bounded, Options::default()), "[0,0,0]");
    }

    #[test]
    fn tuple_positions_are_all_emitted() {
        let schema =
            r#"{"type":"array","prefixItems":[{"type":"string"},{"type":"integer"}]}"#;
        assert_eq!(compact(schema, Options::default()), r#"["string",0]"#);
    }

    #[test]
    fn unique_items_are_made_distinct() {
        let schema =
            r#"{"type":"array","items":{"type":"string"},"minItems":2,"uniqueItems":true}"#;
        assert_eq!(
            compact(schema, Options::default()),
            r#"["string","string-1"]"#
        );
    }

    #[test]
    fn string_formats_and_length_bounds() {
        assert_eq!(
            compact(r#"{"type":"string","format":"email"}"#, Options::default()),
            r#""user@example.com""#
        );
        assert_eq!(
            compact(r#"{"type":"string","format":"date"}"#, Options::default()),
            r#""2026-01-01""#
        );
        assert_eq!(
            compact(r#"{"type":"string","minLength":8}"#, Options::default()),
            r#""stringxx""#
        );
        assert_eq!(
            compact(r#"{"type":"string","maxLength":3}"#, Options::default()),
            r#""str""#
        );
    }

    #[test]
    fn numeric_bounds_and_multiple_of() {
        assert_eq!(
            compact(r#"{"type":"integer","minimum":10}"#, Options::default()),
            "10"
        );
        assert_eq!(
            compact(r#"{"type":"integer","exclusiveMinimum":10}"#, Options::default()),
            "11"
        );
        assert_eq!(
            compact(
                r#"{"type":"integer","minimum":10,"exclusiveMinimum":true}"#,
                Options::default()
            ),
            "11"
        );
        assert_eq!(
            compact(r#"{"type":"integer","maximum":-5}"#, Options::default()),
            "-5"
        );
        assert_eq!(
            compact(r#"{"type":"integer","minimum":3,"multipleOf":5}"#, Options::default()),
            "5"
        );
        assert_eq!(
            compact(r#"{"type":"number","minimum":1.5}"#, Options::default()),
            "1.5"
        );
    }

    #[test]
    fn nullable_union_picks_the_real_type() {
        assert_eq!(
            compact(r#"{"type":["null","integer"]}"#, Options::default()),
            "0"
        );
    }

    #[test]
    fn pretty_output_is_indented() {
        let out = sample(
            r#"{"type":"object","properties":{"id":{"type":"integer"}},"required":["id"]}"#,
            Options::default(),
        )
        .unwrap();
        assert_eq!(out, "{\n  \"id\": 0\n}");
    }

    #[test]
    fn free_form_object_stays_empty_unless_optional_asked_for() {
        let schema = r#"{"type":"object","additionalProperties":{"type":"string"}}"#;
        assert_eq!(compact(schema, Options::default()), "{}");
        assert_eq!(
            compact(
                schema,
                Options {
                    include_optional: true,
                    ..Options::default()
                }
            ),
            r#"{"additionalProp1":"string"}"#
        );
    }

    #[test]
    fn invalid_json_is_reported() {
        let err = sample("{oops", Options::default()).unwrap_err();
        assert!(err.contains("not valid JSON"), "{err}");
    }

    #[test]
    fn empty_schema_is_reported() {
        let err = sample("   ", Options::default()).unwrap_err();
        assert!(err.contains("schema is empty"), "{err}");
    }

    #[test]
    fn remote_ref_is_rejected_with_a_clear_message() {
        let err = sample(
            r#"{"$ref":"https://example.com/schema.json#/User"}"#,
            Options::default(),
        )
        .unwrap_err();
        assert!(err.contains("only local pointers"), "{err}");
    }

    #[test]
    fn unresolved_local_ref_is_reported() {
        let err = sample(r##"{"$ref":"#/$defs/Missing"}"##, Options::default()).unwrap_err();
        assert!(err.contains("unresolved $ref"), "{err}");
    }

    #[test]
    fn false_schema_is_rejected() {
        let err = sample("false", Options::default()).unwrap_err();
        assert!(err.contains("accepts no instance"), "{err}");
    }

    #[test]
    fn non_schema_root_is_rejected() {
        let err = sample("[1,2,3]", Options::default()).unwrap_err();
        assert!(err.contains("expected a JSON Schema object"), "{err}");
    }

    #[test]
    fn conflicting_numeric_bounds_are_reported() {
        let err = sample(
            r#"{"type":"integer","minimum":10,"maximum":5}"#,
            Options::default(),
        )
        .unwrap_err();
        assert!(err.contains("conflicting bounds"), "{err}");
    }

    #[test]
    fn array_items_over_the_cap_is_rejected() {
        let err = sample(
            r#"{"type":"array","items":{"type":"string"}}"#,
            Options {
                array_items: 999,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("over the maximum"), "{err}");
    }
}
