//! openapi-to-typescript-types core — extract the schema objects from an OpenAPI
//! document and emit matching TypeScript type declarations. Pure Rust: parse the
//! input (JSON or YAML) into a `serde_json::Value` with object key order
//! preserved, read `components.schemas` (OpenAPI 3.x) or `definitions`
//! (Swagger 2.0), then walk each schema into a TypeScript type expression.
//!
//! Scope is a deterministic, dependency-free subset of a full codegen: it covers
//! the JSON-Schema constructs that the overwhelming majority of `schemas`
//! objects use — `$ref`, `type` (incl. 3.1 type arrays), `enum`, `const`,
//! `nullable`, `required`, `properties`, `additionalProperties`, `items`,
//! `prefixItems`/tuple `items`, `allOf`/`oneOf`/`anyOf`, and `description`
//! (emitted as JSDoc). It does NOT resolve external `$ref`s, generate the
//! `paths`/operations client, or evaluate JSON-Schema validation keywords
//! (`pattern`, `minimum`, …) — those are documented as limits.

use std::collections::{BTreeSet, HashMap};

use serde_json::{Map, Value};

/// Maximum accepted value for the `indent` parameter (spaces per level).
pub const MAX_INDENT: u32 = 8;

/// How to render each top-level object schema.
#[derive(Clone, Copy, PartialEq)]
enum Declaration {
    /// `export interface Name { … }`
    Interface,
    /// `export type Name = { … }`
    Type,
}

/// How to render a schema that carries an `enum` of string values.
#[derive(Clone, Copy, PartialEq)]
enum EnumStyle {
    /// A union of string-literal types.
    Union,
    /// A real TypeScript `enum`.
    Enum,
}

/// How the optionality (`?`) of object properties is decided.
#[derive(Clone, Copy, PartialEq)]
enum OptionalStyle {
    /// Follow the schema's `required` array (the OpenAPI default).
    Spec,
    /// Every property is optional.
    Optional,
    /// Every property is required.
    Required,
}

/// Convert an OpenAPI/Swagger document into TypeScript type declarations.
///
/// * `spec` — the OpenAPI 3.x or Swagger 2.0 document, as JSON or YAML text.
/// * `input_format` — `"auto"` (default), `"json"`, or `"yaml"`.
/// * `declaration` — `"interface"` (default) or `"type"` for object schemas.
/// * `enum_style` — `"union"` (default) or `"enum"` for string-enum schemas.
/// * `optional_style` — `"spec"` (default, honor `required`), `"optional"`
///   (all properties `?`), or `"required"` (no `?`).
/// * `export` — prefix every declaration with `export` (default true).
/// * `readonly` — mark every property `readonly` (default false).
/// * `sort` — alphabetize object properties (default false; otherwise the
///   document's own key order is preserved).
/// * `indent` — spaces per nesting level, `0`–`8` (default 2).
#[allow(clippy::too_many_arguments)]
pub fn convert(
    spec: &str,
    input_format: &str,
    declaration: &str,
    enum_style: &str,
    optional_style: &str,
    export: bool,
    readonly: bool,
    sort: bool,
    indent: u32,
) -> Result<String, String> {
    let doc = parse(spec, input_format)?;

    let schemas = doc
        .get("components")
        .and_then(|c| c.get("schemas"))
        .and_then(|v| v.as_object())
        .or_else(|| doc.get("definitions").and_then(|v| v.as_object()));

    let schemas = match schemas {
        Some(s) if !s.is_empty() => s,
        Some(_) => {
            return Err(
                "the document's schemas object is empty — there are no schemas to convert".into(),
            )
        }
        None => {
            return Err("no schemas found: expected an OpenAPI 3.x 'components.schemas' object or a Swagger 2.0 'definitions' object at the document root".into())
        }
    };

    // Assign each schema key a unique, valid TypeScript identifier up front so
    // `$ref`s resolve to the same name we declare.
    let mut names: HashMap<String, String> = HashMap::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    for key in schemas.keys() {
        let mut id = sanitize_ident(key);
        while used.contains(&id) {
            id.push('_');
        }
        used.insert(id.clone());
        names.insert(key.clone(), id);
    }

    let g = Gen {
        names,
        declaration: match declaration.trim() {
            "type" => Declaration::Type,
            _ => Declaration::Interface,
        },
        enum_style: match enum_style.trim() {
            "enum" => EnumStyle::Enum,
            _ => EnumStyle::Union,
        },
        optional_style: match optional_style.trim() {
            "optional" => OptionalStyle::Optional,
            "required" => OptionalStyle::Required,
            _ => OptionalStyle::Spec,
        },
        export,
        readonly,
        sort,
        indent: " ".repeat(indent.min(MAX_INDENT) as usize),
    };

    let mut blocks: Vec<String> = Vec::with_capacity(schemas.len());
    for (name, schema) in schemas {
        blocks.push(g.emit(name, schema));
    }
    Ok(blocks.join("\n"))
}

/// Parse the input into a JSON value, honoring the requested format.
fn parse(spec: &str, input_format: &str) -> Result<Value, String> {
    let s = spec.trim();
    if s.is_empty() {
        return Err(
            "input is empty — paste an OpenAPI 3.x or Swagger 2.0 document (JSON or YAML)".into(),
        );
    }
    match input_format.trim() {
        "json" => serde_json::from_str(s).map_err(|e| format!("invalid JSON: {e}")),
        "yaml" => serde_yml::from_str(s).map_err(|e| format!("invalid YAML: {e}")),
        _ => serde_json::from_str(s).or_else(|_| {
            serde_yml::from_str(s)
                .map_err(|e| format!("could not parse input as JSON or YAML: {e}"))
        }),
    }
}

struct Gen {
    names: HashMap<String, String>,
    declaration: Declaration,
    enum_style: EnumStyle,
    optional_style: OptionalStyle,
    export: bool,
    readonly: bool,
    sort: bool,
    /// One level of indentation (`indent` spaces).
    indent: String,
}

impl Gen {
    /// Emit one top-level declaration for `name`/`schema`.
    fn emit(&self, name: &str, schema: &Value) -> String {
        let ident = self
            .names
            .get(name)
            .cloned()
            .unwrap_or_else(|| sanitize_ident(name));
        let mut out = String::new();
        if let Some(doc) = top_jsdoc(schema) {
            out.push_str(&doc);
        }
        let exp = if self.export { "export " } else { "" };
        let obj = match schema {
            Value::Object(m) => m,
            other => {
                // A boolean/other schema at the top level — alias it.
                let ty = self.ts_type(other, 0);
                out.push_str(&format!("{exp}type {ident} = {ty};\n"));
                return out;
            }
        };

        // A pure string enum can become a real TypeScript enum.
        if self.enum_style == EnumStyle::Enum && !obj.contains_key("$ref") {
            if let Some(vals) = obj.get("enum").and_then(|v| v.as_array()) {
                if !vals.is_empty() && vals.iter().all(|v| v.is_string()) {
                    let mut lines = Vec::new();
                    for v in vals {
                        let s = v.as_str().unwrap_or_default();
                        lines.push(format!("{}{} = {},", self.indent, enum_member(s), quote(s)));
                    }
                    out.push_str(&format!("{exp}enum {ident} {{\n{}\n}}\n", lines.join("\n")));
                    return out;
                }
            }
        }

        // A plain object schema → interface (when requested) built from an
        // object literal body; anything else → a type alias.
        let is_plain_object = !obj.contains_key("$ref")
            && !obj.contains_key("allOf")
            && !obj.contains_key("oneOf")
            && !obj.contains_key("anyOf")
            && !obj.contains_key("enum")
            && !obj.contains_key("const")
            && (obj.get("type").and_then(|v| v.as_str()) == Some("object")
                || (obj.get("type").is_none() && obj.contains_key("properties")));

        if self.declaration == Declaration::Interface && is_plain_object {
            let body = self.object_type(obj, 0);
            if body.starts_with('{') {
                out.push_str(&format!("{exp}interface {ident} {body}\n"));
                return out;
            }
            out.push_str(&format!("{exp}type {ident} = {body};\n"));
            return out;
        }

        let ty = self.ts_type(schema, 0);
        out.push_str(&format!("{exp}type {ident} = {ty};\n"));
        out
    }

    /// A TypeScript type expression for `schema`, applying `nullable`.
    fn ts_type(&self, schema: &Value, depth: usize) -> String {
        let base = self.ts_type_base(schema, depth);
        if let Value::Object(obj) = schema {
            if obj.get("nullable") == Some(&Value::Bool(true))
                && base != "null"
                && !base.ends_with("| null")
            {
                return format!("{base} | null");
            }
        }
        base
    }

    fn ts_type_base(&self, schema: &Value, depth: usize) -> String {
        let obj = match schema {
            Value::Object(m) => m,
            Value::Bool(true) => return "unknown".into(),
            Value::Bool(false) => return "never".into(),
            _ => return "unknown".into(),
        };

        if let Some(r) = obj.get("$ref").and_then(|v| v.as_str()) {
            return self.ref_name(r);
        }
        if let Some(c) = obj.get("const") {
            return literal(c);
        }
        if let Some(all) = obj.get("allOf").and_then(|v| v.as_array()) {
            let parts: Vec<String> = all
                .iter()
                .map(|s| paren_if_compound(self.ts_type(s, depth)))
                .collect();
            return if parts.is_empty() {
                "unknown".into()
            } else {
                parts.join(" & ")
            };
        }
        for key in ["oneOf", "anyOf"] {
            if let Some(arr) = obj.get(key).and_then(|v| v.as_array()) {
                let mut parts: Vec<String> = arr.iter().map(|s| self.ts_type(s, depth)).collect();
                if parts.is_empty() {
                    parts.push("unknown".into());
                }
                parts.dedup();
                return parts.join(" | ");
            }
        }
        if let Some(en) = obj.get("enum").and_then(|v| v.as_array()) {
            if !en.is_empty() {
                let mut parts: Vec<String> = en.iter().map(literal).collect();
                parts.dedup();
                return parts.join(" | ");
            }
        }

        match obj.get("type") {
            Some(Value::String(t)) => self.base_for(t, obj, depth),
            Some(Value::Array(ts)) => {
                let mut parts: Vec<String> = ts
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|t| self.base_for(t, obj, depth))
                    .collect();
                if parts.is_empty() {
                    parts.push("unknown".into());
                }
                parts.dedup();
                parts.join(" | ")
            }
            _ => {
                if obj.contains_key("properties") || obj.contains_key("additionalProperties") {
                    self.object_type(obj, depth)
                } else {
                    "unknown".into()
                }
            }
        }
    }

    fn base_for(&self, t: &str, obj: &Map<String, Value>, depth: usize) -> String {
        match t {
            "string" => "string".into(),
            "integer" | "number" => "number".into(),
            "boolean" => "boolean".into(),
            "null" => "null".into(),
            "array" => self.array_type(obj, depth),
            "object" => self.object_type(obj, depth),
            _ => "unknown".into(),
        }
    }

    fn array_type(&self, obj: &Map<String, Value>, depth: usize) -> String {
        if let Some(prefix) = obj.get("prefixItems").and_then(|v| v.as_array()) {
            let parts: Vec<String> = prefix.iter().map(|s| self.ts_type(s, depth)).collect();
            return format!("[{}]", parts.join(", "));
        }
        match obj.get("items") {
            Some(Value::Array(tuple)) => {
                let parts: Vec<String> = tuple.iter().map(|s| self.ts_type(s, depth)).collect();
                format!("[{}]", parts.join(", "))
            }
            Some(items) => {
                let inner = self.ts_type(items, depth);
                if is_compound(&inner) {
                    format!("({inner})[]")
                } else {
                    format!("{inner}[]")
                }
            }
            None => "unknown[]".into(),
        }
    }

    /// An object literal (`{ … }`) or a `Record<…>` when there are no named
    /// properties.
    fn object_type(&self, obj: &Map<String, Value>, depth: usize) -> String {
        let props = obj.get("properties").and_then(|v| v.as_object());
        let addl = obj.get("additionalProperties");
        let has_props = props.map(|p| !p.is_empty()).unwrap_or(false);
        let ro = if self.readonly { "readonly " } else { "" };

        if !has_props {
            return match addl {
                Some(v @ Value::Object(_)) => {
                    let t = self.ts_type(v, depth);
                    if self.readonly {
                        format!("{{ readonly [key: string]: {t} }}")
                    } else {
                        format!("Record<string, {t}>")
                    }
                }
                Some(Value::Bool(false)) => "Record<string, never>".into(),
                _ => "Record<string, unknown>".into(),
            };
        }

        let props = props.unwrap();
        let required: BTreeSet<&str> = obj
            .get("required")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        let pad = self.indent.repeat(depth + 1);
        let close = self.indent.repeat(depth);
        let mut lines: Vec<String> = Vec::new();

        let mut keys: Vec<&String> = props.keys().collect();
        if self.sort {
            keys.sort();
        }
        for k in keys {
            let sub = &props[k];
            if let Some(doc) = prop_jsdoc(sub, &pad) {
                lines.push(doc);
            }
            let optional = match self.optional_style {
                OptionalStyle::Spec => !required.contains(k.as_str()),
                OptionalStyle::Optional => true,
                OptionalStyle::Required => false,
            };
            let key = if is_valid_prop(k) { k.clone() } else { quote(k) };
            let q = if optional { "?" } else { "" };
            let ty = self.ts_type(sub, depth + 1);
            lines.push(format!("{pad}{ro}{key}{q}: {ty};"));
        }

        match addl {
            Some(Value::Bool(true)) => lines.push(format!("{pad}{ro}[key: string]: unknown;")),
            Some(v @ Value::Object(_)) => {
                let t = self.ts_type(v, depth + 1);
                lines.push(format!("{pad}{ro}[key: string]: {t};"));
            }
            _ => {}
        }

        format!("{{\n{}\n{close}}}", lines.join("\n"))
    }

    /// The declared identifier for a local `$ref`.
    fn ref_name(&self, r: &str) -> String {
        let seg = r.rsplit('/').next().unwrap_or(r);
        // Handle JSON Pointer escapes (~1 = '/', ~0 = '~').
        let seg = seg.replace("~1", "/").replace("~0", "~");
        self.names
            .get(&seg)
            .cloned()
            .unwrap_or_else(|| sanitize_ident(&seg))
    }
}

/// Whether `s` is a compound type (union/intersection) that needs parentheses
/// when used as an array element type.
fn is_compound(s: &str) -> bool {
    s.contains(" | ") || s.contains(" & ")
}

fn paren_if_compound(s: String) -> String {
    if is_compound(&s) {
        format!("({s})")
    } else {
        s
    }
}

/// A TypeScript literal type for a JSON scalar (used by `enum`/`const`).
fn literal(v: &Value) -> String {
    match v {
        Value::String(s) => quote(s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
        _ => "unknown".into(),
    }
}

/// A JSON-quoted (double-quote) string, safely escaped.
fn quote(s: &str) -> String {
    Value::String(s.to_string()).to_string()
}

/// Turn a schema key into a valid TypeScript identifier.
fn sanitize_ident(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
            if i == 0 && c.is_ascii_digit() {
                out.push('_');
            }
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// A valid enum member name for a string value (falls back to sanitizing).
fn enum_member(s: &str) -> String {
    let id = sanitize_ident(s);
    if id.is_empty() {
        "_".into()
    } else {
        id
    }
}

/// Whether `s` can appear as a bare (unquoted) object property key.
fn is_valid_prop(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// A single-line JSDoc comment for a schema `description`, at column 0.
fn top_jsdoc(schema: &Value) -> Option<String> {
    let d = schema.get("description")?.as_str()?;
    let d = d.replace('\n', " ");
    let d = d.trim();
    if d.is_empty() {
        return None;
    }
    Some(format!("/** {d} */\n"))
}

/// A single-line JSDoc comment for a property `description`, indented by `pad`.
fn prop_jsdoc(schema: &Value, pad: &str) -> Option<String> {
    let d = schema.get("description")?.as_str()?;
    let d = d.replace('\n', " ");
    let d = d.trim();
    if d.is_empty() {
        return None;
    }
    Some(format!("{pad}/** {d} */"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = r##"{
      "openapi": "3.0.3",
      "components": {
        "schemas": {
          "Status": { "type": "string", "enum": ["active", "banned"] },
          "User": {
            "type": "object",
            "required": ["id", "name"],
            "properties": {
              "id": { "type": "integer" },
              "name": { "type": "string" },
              "status": { "$ref": "#/components/schemas/Status" },
              "tags": { "type": "array", "items": { "type": "string" } }
            }
          }
        }
      }
    }"##;

    #[test]
    fn happy_path_interfaces_and_union() {
        let out =
            convert(SPEC, "auto", "interface", "union", "spec", true, false, false, 2).unwrap();
        let expected = "export type Status = \"active\" | \"banned\";\n\
\n\
export interface User {\n\
\x20\x20id: number;\n\
\x20\x20name: string;\n\
\x20\x20status?: Status;\n\
\x20\x20tags?: string[];\n\
}\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn yaml_input_and_real_enum() {
        let yaml = "definitions:\n  Color:\n    type: string\n    enum: [red, green]\n";
        let out =
            convert(yaml, "yaml", "interface", "enum", "spec", true, false, false, 2).unwrap();
        assert_eq!(
            out,
            "export enum Color {\n  red = \"red\",\n  green = \"green\",\n}\n"
        );
    }

    #[test]
    fn nullable_type_alias_and_readonly() {
        let spec = r#"{
          "components": { "schemas": {
            "Box": {
              "type": "object",
              "required": ["w"],
              "properties": {
                "w": { "type": "number", "nullable": true },
                "note": { "type": "string" }
              }
            }
          } }
        }"#;
        let out =
            convert(spec, "auto", "type", "union", "required", false, true, false, 2).unwrap();
        assert_eq!(
            out,
            "type Box = {\n  readonly w: number | null;\n  readonly note: string;\n};\n"
        );
    }

    #[test]
    fn error_on_missing_schemas() {
        let err = convert(
            "{\"openapi\":\"3.0.0\"}",
            "json",
            "interface",
            "union",
            "spec",
            true,
            false,
            false,
            2,
        )
        .unwrap_err();
        assert!(err.contains("no schemas found"), "got: {err}");
    }

    #[test]
    fn error_on_invalid_input() {
        let err = convert(
            "::: not json or yaml :::",
            "json",
            "interface",
            "union",
            "spec",
            true,
            false,
            false,
            2,
        )
        .unwrap_err();
        assert!(err.contains("invalid JSON"), "got: {err}");
    }
}
