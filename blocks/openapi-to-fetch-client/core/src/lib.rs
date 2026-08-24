//! openapi-to-fetch-client core — turn the `paths` object of an OpenAPI 3.x /
//! Swagger 2.0 document into a self-contained, dependency-free TypeScript client
//! built on `fetch`.
//!
//! Pure Rust: parse the input (JSON or YAML) into a `serde_json::Value` with key
//! order preserved, walk `paths` → path item → operation, and emit one typed
//! async function per operation (or one class with one method per operation),
//! plus a small inlined runtime (`apiFetch`, `encodeQuery`, `readBody`) so the
//! generated file drops into any project with no npm dependency.
//!
//! Scope is deliberately the OPERATIONS layer: request/response *schema* types
//! are imported from a module you point at (`types_module`), because emitting
//! `components.schemas` declarations is a different job. Local `$ref`s become
//! named type references; external/remote refs fall back to `unknown`.

use std::collections::{BTreeSet, HashMap};

use serde_json::{Map, Value};

/// Maximum accepted value for the `indent` parameter (spaces per level).
pub const MAX_INDENT: u32 = 8;

/// How deep an inline schema is expanded before it collapses to `unknown`.
const MAX_SCHEMA_DEPTH: usize = 12;

/// HTTP methods that count as operations inside a path item, in the order the
/// OpenAPI specification lists them (so output is deterministic).
const METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// Emit one exported function per operation, or one client class.
#[derive(Clone, Copy, PartialEq)]
enum Style {
    Functions,
    Class,
}

/// How an operation's arguments are laid out.
#[derive(Clone, Copy, PartialEq)]
enum ParamStyle {
    /// A single typed request object (plus `options`).
    Object,
    /// Path params, then body, then a query/header object.
    Positional,
}

/// What a call does when the server answers with a non-2xx status.
#[derive(Clone, Copy, PartialEq)]
enum ErrorHandling {
    /// Throw an `ApiError`.
    Throw,
    /// Resolve to an `ApiResult<T>` discriminated union.
    Result,
}

/// Where a function name comes from.
#[derive(Clone, Copy, PartialEq)]
enum Naming {
    /// `operationId` when present, else derived from method + path.
    OperationId,
    /// Always derived from method + path.
    Path,
}

/// One resolved parameter of an operation.
struct Par {
    /// The wire name, exactly as the document spells it.
    name: String,
    /// TypeScript type expression.
    ty: String,
    required: bool,
    description: Option<String>,
}

/// A resolved request body.
struct Body {
    ty: String,
    required: bool,
    content_type: String,
    description: Option<String>,
}

/// One operation, fully resolved and ready to render.
struct Op {
    method: String,
    path: String,
    fn_name: String,
    iface_name: String,
    summary: Option<String>,
    description: Option<String>,
    deprecated: bool,
    path_params: Vec<Par>,
    query_params: Vec<Par>,
    header_params: Vec<Par>,
    cookie_params: Vec<String>,
    form_params: Vec<String>,
    body: Option<Body>,
    response_ty: String,
}

impl Op {
    /// Params that live in the generated request interface for `param_style`.
    fn iface_params(&self, param_style: ParamStyle) -> Vec<&Par> {
        let mut out: Vec<&Par> = Vec::new();
        if param_style == ParamStyle::Object {
            out.extend(self.path_params.iter());
        }
        out.extend(self.query_params.iter());
        out.extend(self.header_params.iter());
        out
    }

    /// True when the generated request interface would have at least one field.
    fn has_iface(&self, param_style: ParamStyle) -> bool {
        !self.iface_params(param_style).is_empty()
            || (param_style == ParamStyle::Object && self.body.is_some())
    }
}

/// Generate the TypeScript client source.
///
/// `spec` is the OpenAPI 3.x / Swagger 2.0 document (JSON or YAML text).
/// Every other argument accepts the empty string to mean "use the default".
#[allow(clippy::too_many_arguments)]
pub fn generate(
    spec: &str,
    input_format: &str,
    style: &str,
    client_name: &str,
    naming: &str,
    param_style: &str,
    error_handling: &str,
    base_url: &str,
    types_module: &str,
    tags: &str,
    jsdoc: bool,
    indent: u32,
) -> Result<String, String> {
    let style = parse_style(style)?;
    let param_style = parse_param_style(param_style)?;
    let error_handling = parse_error_handling(error_handling)?;
    let naming = parse_naming(naming)?;
    let indent = indent.min(MAX_INDENT) as usize;
    let unit = " ".repeat(indent);

    let doc = parse_document(spec, input_format)?;
    let root = doc
        .as_object()
        .ok_or_else(|| "the document root must be an object".to_string())?;

    let swagger2 = root
        .get("swagger")
        .and_then(Value::as_str)
        .is_some_and(|v| v.starts_with('2'));

    let paths = root
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "no `paths` object found — this tool generates a client from the document's \
             operations, so the spec needs at least one path"
                .to_string()
        })?;

    let mut refs: BTreeSet<String> = BTreeSet::new();
    let wanted_tags = parse_tag_filter(tags);
    let mut used_names: HashMap<String, usize> = HashMap::new();
    let mut ops: Vec<Op> = Vec::new();
    let mut saw_any_operation = false;

    for (path, item) in paths {
        let Some(item) = item.as_object() else {
            continue;
        };
        let shared: Vec<&Value> = item
            .get("parameters")
            .and_then(Value::as_array)
            .map(|a| a.iter().collect())
            .unwrap_or_default();

        for method in METHODS {
            let Some(op) = item.get(method).and_then(Value::as_object) else {
                continue;
            };
            saw_any_operation = true;
            if !tag_matches(op, &wanted_tags) {
                continue;
            }
            ops.push(build_op(
                path,
                method,
                op,
                &shared,
                root,
                swagger2,
                naming,
                &mut used_names,
                &mut refs,
            ));
        }
    }

    if ops.is_empty() {
        return Err(if !saw_any_operation {
            "the `paths` object has no operations (no get/put/post/delete/... entries)".to_string()
        } else {
            format!(
                "no operation matched the tag filter `{}` — clear the filter to generate every operation",
                tags.trim()
            )
        });
    }

    let resolved_base = if base_url.trim().is_empty() {
        spec_base_url(root, swagger2)
    } else {
        base_url.trim().trim_end_matches('/').to_string()
    };

    let mut out = String::new();
    out.push_str("// Typed fetch client generated from an OpenAPI document.\n");
    out.push_str("// Self-contained: no runtime dependency, works anywhere `fetch` exists.\n\n");

    emit_type_imports(&mut out, &refs, types_module);
    emit_prelude(
        &mut out,
        &resolved_base,
        error_handling,
        jsdoc,
        &unit,
        style,
        client_name,
    );

    // Request interfaces live at module level for both styles.
    for op in &ops {
        emit_iface(&mut out, op, param_style, jsdoc, &unit);
    }

    match style {
        Style::Functions => {
            for op in &ops {
                emit_operation(
                    &mut out,
                    op,
                    param_style,
                    error_handling,
                    jsdoc,
                    &unit,
                    0,
                    false,
                );
            }
        }
        Style::Class => {
            let name = pascal_ident(client_name, "ApiClient");
            if jsdoc {
                out.push_str("/** Typed client for every operation in the document. */\n");
            }
            out.push_str(&format!("export class {name} {{\n"));
            out.push_str(&format!(
                "{unit}constructor(private readonly options: RequestOptions = {{}}) {{}}\n\n"
            ));
            if jsdoc {
                out.push_str(&format!(
                    "{unit}/** Per-call options layered over the client's own. */\n"
                ));
            }
            out.push_str(&format!(
                "{unit}private merge(options: RequestOptions): RequestOptions {{\n"
            ));
            out.push_str(&format!("{unit}{unit}return {{\n"));
            out.push_str(&format!("{unit}{unit}{unit}...this.options,\n"));
            out.push_str(&format!("{unit}{unit}{unit}...options,\n"));
            out.push_str(&format!(
                "{unit}{unit}{unit}headers: {{ ...(this.options.headers ?? {{}}), ...(options.headers ?? {{}}) }},\n"
            ));
            out.push_str(&format!("{unit}{unit}}};\n"));
            out.push_str(&format!("{unit}}}\n"));
            for op in &ops {
                out.push('\n');
                emit_operation(
                    &mut out,
                    op,
                    param_style,
                    error_handling,
                    jsdoc,
                    &unit,
                    1,
                    true,
                );
            }
            out.push_str("}\n");
        }
    }

    Ok(out.trim_end().to_string() + "\n")
}

// ---------------------------------------------------------------------------
// input parsing
// ---------------------------------------------------------------------------

fn parse_document(spec: &str, input_format: &str) -> Result<Value, String> {
    if spec.trim().is_empty() {
        return Err("spec is empty — paste an OpenAPI 3.x or Swagger 2.0 document".to_string());
    }
    match input_format.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => serde_json::from_str::<Value>(spec).or_else(|json_err| {
            serde_yml::from_str::<Value>(spec).map_err(|yaml_err| {
                format!("could not parse the document as JSON ({json_err}) or YAML ({yaml_err})")
            })
        }),
        "json" => serde_json::from_str::<Value>(spec).map_err(|e| format!("invalid JSON: {e}")),
        "yaml" | "yml" => {
            serde_yml::from_str::<Value>(spec).map_err(|e| format!("invalid YAML: {e}"))
        }
        other => Err(format!(
            "unknown input_format `{other}` — use auto, json, or yaml"
        )),
    }
}

fn parse_style(v: &str) -> Result<Style, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "functions" => Ok(Style::Functions),
        "class" => Ok(Style::Class),
        other => Err(format!("unknown style `{other}` — use functions or class")),
    }
}

fn parse_param_style(v: &str) -> Result<ParamStyle, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "object" => Ok(ParamStyle::Object),
        "positional" => Ok(ParamStyle::Positional),
        other => Err(format!(
            "unknown param_style `{other}` — use object or positional"
        )),
    }
}

fn parse_error_handling(v: &str) -> Result<ErrorHandling, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "throw" => Ok(ErrorHandling::Throw),
        "result" => Ok(ErrorHandling::Result),
        other => Err(format!(
            "unknown error_handling `{other}` — use throw or result"
        )),
    }
}

fn parse_naming(v: &str) -> Result<Naming, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "operation_id" => Ok(Naming::OperationId),
        "path" => Ok(Naming::Path),
        other => Err(format!(
            "unknown naming `{other}` — use operation_id or path"
        )),
    }
}

fn parse_tag_filter(tags: &str) -> Vec<String> {
    tags.split(',')
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

fn tag_matches(op: &Map<String, Value>, wanted: &[String]) -> bool {
    if wanted.is_empty() {
        return true;
    }
    op.get("tags")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .any(|t| wanted.iter().any(|w| w == &t.to_ascii_lowercase()))
        })
        .unwrap_or(false)
}

/// The base URL declared by the document itself, with `{variables}` filled in
/// from their declared defaults.
fn spec_base_url(root: &Map<String, Value>, swagger2: bool) -> String {
    if swagger2 {
        let scheme = root
            .get("schemes")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .unwrap_or("https");
        let host = root.get("host").and_then(Value::as_str).unwrap_or("");
        let base = root.get("basePath").and_then(Value::as_str).unwrap_or("");
        if host.is_empty() {
            return base.trim_end_matches('/').to_string();
        }
        return format!("{scheme}://{host}{base}")
            .trim_end_matches('/')
            .to_string();
    }
    let Some(server) = root
        .get("servers")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(Value::as_object)
    else {
        return String::new();
    };
    let url = server.get("url").and_then(Value::as_str).unwrap_or("");
    let mut resolved = url.to_string();
    if let Some(vars) = server.get("variables").and_then(Value::as_object) {
        for (name, spec) in vars {
            if let Some(default) = spec.get("default").and_then(Value::as_str) {
                resolved = resolved.replace(&format!("{{{name}}}"), default);
            }
        }
    }
    resolved.trim_end_matches('/').to_string()
}

// ---------------------------------------------------------------------------
// operation resolution
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn build_op(
    path: &str,
    method: &str,
    op: &Map<String, Value>,
    shared: &[&Value],
    root: &Map<String, Value>,
    swagger2: bool,
    naming: Naming,
    used_names: &mut HashMap<String, usize>,
    refs: &mut BTreeSet<String>,
) -> Op {
    let mut path_params = Vec::new();
    let mut query_params = Vec::new();
    let mut header_params = Vec::new();
    let mut cookie_params = Vec::new();
    let mut form_params = Vec::new();
    let mut body: Option<Body> = None;

    // Operation-level parameters override path-level ones with the same name+in.
    let own: Vec<&Value> = op
        .get("parameters")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let mut all: Vec<&Value> = Vec::new();
    for p in shared.iter().chain(own.iter()) {
        let Some(obj) = p.as_object() else { continue };
        let key = (
            obj.get("name").and_then(Value::as_str).unwrap_or_default(),
            obj.get("in").and_then(Value::as_str).unwrap_or_default(),
        );
        if let Some(slot) = all.iter_mut().find(|e| {
            let o = e.as_object().unwrap();
            (
                o.get("name").and_then(Value::as_str).unwrap_or_default(),
                o.get("in").and_then(Value::as_str).unwrap_or_default(),
            ) == key
        }) {
            *slot = p;
        } else {
            all.push(p);
        }
    }

    for p in all {
        let Some(obj) = p.as_object() else { continue };
        let name = obj.get("name").and_then(Value::as_str).unwrap_or_default();
        let location = obj.get("in").and_then(Value::as_str).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        if location == "body" {
            // Swagger 2.0 body parameter.
            let ty = obj
                .get("schema")
                .map(|s| ts_type(s, refs, 0))
                .unwrap_or_else(|| "unknown".to_string());
            body = Some(Body {
                ty,
                required: obj
                    .get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                content_type: swagger2_content_type(op, root),
                description: description_of(obj),
            });
            continue;
        }
        if location == "formData" {
            form_params.push(name.to_string());
            continue;
        }
        if location == "cookie" {
            cookie_params.push(name.to_string());
            continue;
        }
        let schema = obj.get("schema").unwrap_or(p);
        let par = Par {
            name: name.to_string(),
            ty: ts_type(schema, refs, 0),
            required: obj
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || location == "path",
            description: description_of(obj),
        };
        match location {
            "path" => path_params.push(par),
            "header" => header_params.push(par),
            // Anything else (including a missing `in`) is treated as a query param.
            _ => query_params.push(par),
        }
    }

    // OpenAPI 3.x request body.
    if body.is_none() {
        if let Some(rb) = op.get("requestBody").and_then(Value::as_object) {
            if let Some((content_type, schema)) = pick_content(rb.get("content")) {
                body = Some(Body {
                    ty: schema
                        .get("schema")
                        .map(|s| ts_type(s, refs, 0))
                        .unwrap_or_else(|| "unknown".to_string()),
                    required: rb.get("required").and_then(Value::as_bool).unwrap_or(false),
                    content_type,
                    description: description_of(rb),
                });
            }
        }
    }

    let response_ty = success_type(op, swagger2, refs);

    let base_name = match naming {
        Naming::OperationId => op
            .get("operationId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(camel_ident)
            .unwrap_or_else(|| derive_name(method, path)),
        Naming::Path => derive_name(method, path),
    };
    let fn_name = unique_name(base_name, used_names);
    let iface_name = format!("{}Request", capitalize(&fn_name));

    Op {
        method: method.to_ascii_uppercase(),
        path: path.to_string(),
        fn_name,
        iface_name,
        summary: op
            .get("summary")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        description: description_of(op),
        deprecated: op
            .get("deprecated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        path_params,
        query_params,
        header_params,
        cookie_params,
        form_params,
        body,
        response_ty,
    }
}

fn description_of(obj: &Map<String, Value>) -> Option<String> {
    obj.get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Prefer `application/json`, else the first declared content type.
fn pick_content(content: Option<&Value>) -> Option<(String, &Map<String, Value>)> {
    let map = content?.as_object()?;
    if let Some(json) = map.get("application/json").and_then(Value::as_object) {
        return Some(("application/json".to_string(), json));
    }
    for (ct, body) in map {
        if let Some(obj) = body.as_object() {
            return Some((ct.clone(), obj));
        }
    }
    None
}

fn swagger2_content_type(op: &Map<String, Value>, root: &Map<String, Value>) -> String {
    let consumes = op
        .get("consumes")
        .or_else(|| root.get("consumes"))
        .and_then(Value::as_array);
    let Some(list) = consumes else {
        return "application/json".to_string();
    };
    if list
        .iter()
        .filter_map(Value::as_str)
        .any(|c| c == "application/json")
    {
        return "application/json".to_string();
    }
    list.iter()
        .filter_map(Value::as_str)
        .next()
        .unwrap_or("application/json")
        .to_string()
}

/// The TypeScript type of the lowest 2xx response (then `default`), or `void`.
fn success_type(op: &Map<String, Value>, swagger2: bool, refs: &mut BTreeSet<String>) -> String {
    let Some(responses) = op.get("responses").and_then(Value::as_object) else {
        return "void".to_string();
    };
    let mut codes: Vec<&String> = responses
        .keys()
        .filter(|k| k.starts_with('2') && k.len() == 3)
        .collect();
    codes.sort();
    let chosen = codes
        .first()
        .copied()
        .or_else(|| responses.keys().find(|k| k.as_str() == "default"));
    let Some(code) = chosen else {
        return "void".to_string();
    };
    let Some(resp) = responses.get(code).and_then(Value::as_object) else {
        return "void".to_string();
    };
    let schema = if swagger2 {
        resp.get("schema")
    } else {
        pick_content(resp.get("content")).and_then(|(_, body)| body.get("schema"))
    };
    match schema {
        Some(s) => ts_type(s, refs, 0),
        None => "void".to_string(),
    }
}

// ---------------------------------------------------------------------------
// schema → TypeScript type expression
// ---------------------------------------------------------------------------

fn ts_type(schema: &Value, refs: &mut BTreeSet<String>, depth: usize) -> String {
    if depth > MAX_SCHEMA_DEPTH {
        return "unknown".to_string();
    }
    let Some(obj) = schema.as_object() else {
        return "unknown".to_string();
    };

    if let Some(reference) = obj.get("$ref").and_then(Value::as_str) {
        return match local_ref_name(reference) {
            Some(name) => {
                refs.insert(name.clone());
                name
            }
            // External / remote documents are not fetched — see the stated limits.
            None => "unknown".to_string(),
        };
    }

    let nullable = obj
        .get("nullable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let wrap = |t: String| {
        if nullable {
            format!("{} | null", parenthesize(&t))
        } else {
            t
        }
    };

    for (key, joiner) in [("allOf", " & "), ("oneOf", " | "), ("anyOf", " | ")] {
        if let Some(list) = obj.get(key).and_then(Value::as_array) {
            let parts: Vec<String> = list
                .iter()
                .map(|s| parenthesize(&ts_type(s, refs, depth + 1)))
                .collect();
            if !parts.is_empty() {
                return wrap(parts.join(joiner));
            }
        }
    }

    if let Some(values) = obj.get("enum").and_then(Value::as_array) {
        if !values.is_empty() {
            let parts: Vec<String> = values.iter().map(literal_type).collect();
            return wrap(parts.join(" | "));
        }
    }
    if let Some(constant) = obj.get("const") {
        return wrap(literal_type(constant));
    }

    // OpenAPI 3.1 allows `type: ["string", "null"]`.
    if let Some(list) = obj.get("type").and_then(Value::as_array) {
        let parts: Vec<String> = list
            .iter()
            .filter_map(Value::as_str)
            .map(|t| primitive(t, obj, refs, depth))
            .collect();
        if !parts.is_empty() {
            return wrap(parts.join(" | "));
        }
    }

    match obj.get("type").and_then(Value::as_str) {
        Some(t) => wrap(primitive(t, obj, refs, depth)),
        None if obj.contains_key("properties") || obj.contains_key("additionalProperties") => {
            wrap(object_type(obj, refs, depth))
        }
        None => wrap("unknown".to_string()),
    }
}

fn primitive(
    t: &str,
    obj: &Map<String, Value>,
    refs: &mut BTreeSet<String>,
    depth: usize,
) -> String {
    match t {
        "string" => "string".to_string(),
        "integer" | "number" => "number".to_string(),
        "boolean" => "boolean".to_string(),
        "null" => "null".to_string(),
        "array" => {
            let items = obj
                .get("items")
                .map(|i| ts_type(i, refs, depth + 1))
                .unwrap_or_else(|| "unknown".to_string());
            format!("{}[]", parenthesize(&items))
        }
        "object" => object_type(obj, refs, depth),
        _ => "unknown".to_string(),
    }
}

fn object_type(obj: &Map<String, Value>, refs: &mut BTreeSet<String>, depth: usize) -> String {
    let required: BTreeSet<&str> = obj
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut fields: Vec<String> = Vec::new();
    if let Some(props) = obj.get("properties").and_then(Value::as_object) {
        for (name, schema) in props {
            let opt = if required.contains(name.as_str()) {
                ""
            } else {
                "?"
            };
            fields.push(format!(
                "{}{}: {}",
                obj_key(name),
                opt,
                ts_type(schema, refs, depth + 1)
            ));
        }
    }
    match obj.get("additionalProperties") {
        Some(Value::Bool(true)) => fields.push("[key: string]: unknown".to_string()),
        Some(v @ Value::Object(_)) => {
            fields.push(format!("[key: string]: {}", ts_type(v, refs, depth + 1)))
        }
        _ => {}
    }
    if fields.is_empty() {
        return "Record<string, unknown>".to_string();
    }
    format!("{{ {} }}", fields.join("; "))
}

fn literal_type(v: &Value) -> String {
    match v {
        Value::String(s) => format!("\"{}\"", escape_double_quoted(s)),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Wrap a union/intersection so it composes correctly (`(A | B)[]`).
fn parenthesize(t: &str) -> String {
    if t.contains(" | ") || t.contains(" & ") {
        format!("({t})")
    } else {
        t.to_string()
    }
}

/// `#/components/schemas/Pet` → `Pet`; external refs → `None`.
fn local_ref_name(reference: &str) -> Option<String> {
    let rest = reference.strip_prefix("#/")?;
    let name = rest.rsplit('/').next()?;
    let ident = pascal_ident(name, "");
    if ident.is_empty() {
        None
    } else {
        Some(ident)
    }
}

// ---------------------------------------------------------------------------
// emitters
// ---------------------------------------------------------------------------

fn emit_type_imports(out: &mut String, refs: &BTreeSet<String>, types_module: &str) {
    if refs.is_empty() {
        return;
    }
    let module = types_module.trim();
    if module.is_empty() {
        out.push_str(
            "// No types module configured: these are placeholders. Replace them with the real\n\
             // schema types (a dedicated OpenAPI → TypeScript types generator emits them).\n",
        );
        for name in refs {
            out.push_str(&format!("export type {name} = unknown;\n"));
        }
        out.push('\n');
        return;
    }
    let names: Vec<&str> = refs.iter().map(String::as_str).collect();
    out.push_str(&format!(
        "import type {{ {} }} from \"{}\";\n\n",
        names.join(", "),
        escape_double_quoted(module)
    ));
}

fn emit_prelude(
    out: &mut String,
    base: &str,
    error_handling: ErrorHandling,
    jsdoc: bool,
    unit: &str,
    style: Style,
    _client_name: &str,
) {
    let _ = style;
    if jsdoc {
        out.push_str(
            "/** Prefix for every request URL. Override per call with `options.baseUrl`. */\n",
        );
    }
    out.push_str(&format!(
        "export const baseUrl = \"{}\";\n\n",
        escape_double_quoted(base)
    ));

    if jsdoc {
        out.push_str("/** Per-call knobs; every generated call accepts these. */\n");
    }
    out.push_str("export interface RequestOptions {\n");
    if jsdoc {
        out.push_str(&format!(
            "{unit}/** Replaces the module-level `baseUrl`. */\n"
        ));
    }
    out.push_str(&format!("{unit}baseUrl?: string;\n"));
    if jsdoc {
        out.push_str(&format!(
            "{unit}/** Merged over the generated headers — use this for auth. */\n"
        ));
    }
    out.push_str(&format!("{unit}headers?: Record<string, string>;\n"));
    if jsdoc {
        out.push_str(&format!(
            "{unit}/** Forwarded to `fetch` for cancellation. */\n"
        ));
    }
    out.push_str(&format!("{unit}signal?: AbortSignal;\n"));
    if jsdoc {
        out.push_str(&format!(
            "{unit}/** Swap in a custom `fetch` (test double, retry wrapper, server polyfill). */\n"
        ));
    }
    out.push_str(&format!("{unit}fetch?: typeof fetch;\n"));
    out.push_str("}\n\n");

    match error_handling {
        ErrorHandling::Throw => {
            if jsdoc {
                out.push_str("/** Thrown when the server answers with a non-2xx status. */\n");
            }
            out.push_str("export class ApiError extends Error {\n");
            out.push_str(&format!("{unit}readonly status: number;\n"));
            out.push_str(&format!("{unit}readonly response: Response;\n"));
            out.push_str(&format!("{unit}readonly data: unknown;\n\n"));
            out.push_str(&format!(
                "{unit}constructor(response: Response, data: unknown) {{\n"
            ));
            out.push_str(&format!(
                "{unit}{unit}super(`HTTP ${{response.status}} ${{response.statusText}}`.trim());\n"
            ));
            out.push_str(&format!("{unit}{unit}this.name = \"ApiError\";\n"));
            out.push_str(&format!("{unit}{unit}this.status = response.status;\n"));
            out.push_str(&format!("{unit}{unit}this.response = response;\n"));
            out.push_str(&format!("{unit}{unit}this.data = data;\n"));
            out.push_str(&format!("{unit}}}\n"));
            out.push_str("}\n\n");
        }
        ErrorHandling::Result => {
            if jsdoc {
                out.push_str(
                    "/** Every call resolves to this: `data` on 2xx, `error` on anything else. */\n",
                );
            }
            out.push_str("export type ApiResult<T> =\n");
            out.push_str(&format!(
                "{unit}| {{ data: T; error?: undefined; response: Response }}\n"
            ));
            out.push_str(&format!(
                "{unit}| {{ data?: undefined; error: unknown; response: Response }};\n\n"
            ));
        }
    }

    // Query serializer.
    out.push_str("function encodeQuery(query: Record<string, unknown>): string {\n");
    out.push_str(&format!("{unit}const search = new URLSearchParams();\n"));
    out.push_str(&format!(
        "{unit}for (const [key, value] of Object.entries(query)) {{\n"
    ));
    out.push_str(&format!(
        "{unit}{unit}if (value === undefined || value === null) continue;\n"
    ));
    out.push_str(&format!("{unit}{unit}if (Array.isArray(value)) {{\n"));
    out.push_str(&format!(
        "{unit}{unit}{unit}for (const item of value) search.append(key, String(item));\n"
    ));
    out.push_str(&format!("{unit}{unit}}} else {{\n"));
    out.push_str(&format!(
        "{unit}{unit}{unit}search.append(key, String(value));\n"
    ));
    out.push_str(&format!("{unit}{unit}}}\n"));
    out.push_str(&format!("{unit}}}\n"));
    out.push_str(&format!("{unit}const encoded = search.toString();\n"));
    out.push_str(&format!("{unit}return encoded ? `?${{encoded}}` : \"\";\n"));
    out.push_str("}\n\n");

    // Header serializer.
    out.push_str(
        "function encodeHeaders(headers: Record<string, unknown>): Record<string, string> {\n",
    );
    out.push_str(&format!(
        "{unit}const out: Record<string, string> = {{}};\n"
    ));
    out.push_str(&format!(
        "{unit}for (const [key, value] of Object.entries(headers)) {{\n"
    ));
    out.push_str(&format!(
        "{unit}{unit}if (value === undefined || value === null) continue;\n"
    ));
    out.push_str(&format!("{unit}{unit}out[key] = String(value);\n"));
    out.push_str(&format!("{unit}}}\n"));
    out.push_str(&format!("{unit}return out;\n"));
    out.push_str("}\n\n");

    // Response body reader.
    out.push_str("async function readBody(response: Response): Promise<unknown> {\n");
    out.push_str(&format!(
        "{unit}if (response.status === 204 || response.status === 205) return undefined;\n"
    ));
    out.push_str(&format!(
        "{unit}const contentType = response.headers.get(\"content-type\") ?? \"\";\n"
    ));
    out.push_str(&format!(
        "{unit}if (contentType.includes(\"json\")) return response.json();\n"
    ));
    out.push_str(&format!("{unit}const text = await response.text();\n"));
    out.push_str(&format!("{unit}return text === \"\" ? undefined : text;\n"));
    out.push_str("}\n\n");

    // The one request primitive every operation delegates to.
    let ret = match error_handling {
        ErrorHandling::Throw => "Promise<T>",
        ErrorHandling::Result => "Promise<ApiResult<T>>",
    };
    out.push_str("async function apiFetch<T>(\n");
    out.push_str(&format!("{unit}method: string,\n"));
    out.push_str(&format!("{unit}path: string,\n"));
    out.push_str(&format!("{unit}query: Record<string, unknown>,\n"));
    out.push_str(&format!("{unit}headerParams: Record<string, unknown>,\n"));
    out.push_str(&format!(
        "{unit}body: {{ value: unknown; contentType: string }} | undefined,\n"
    ));
    out.push_str(&format!("{unit}options: RequestOptions,\n"));
    out.push_str(&format!("): {ret} {{\n"));
    out.push_str(&format!(
        "{unit}const headers: Record<string, string> = {{ accept: \"application/json\", ...encodeHeaders(headerParams) }};\n"
    ));
    out.push_str(&format!("{unit}let payload: BodyInit | undefined;\n"));
    out.push_str(&format!("{unit}if (body !== undefined) {{\n"));
    out.push_str(&format!(
        "{unit}{unit}headers[\"content-type\"] = body.contentType;\n"
    ));
    out.push_str(&format!(
        "{unit}{unit}payload = body.contentType.includes(\"json\")\n"
    ));
    out.push_str(&format!("{unit}{unit}{unit}? JSON.stringify(body.value)\n"));
    out.push_str(&format!("{unit}{unit}{unit}: (body.value as BodyInit);\n"));
    out.push_str(&format!("{unit}}}\n"));
    out.push_str(&format!(
        "{unit}Object.assign(headers, options.headers ?? {{}});\n"
    ));
    out.push_str(&format!("{unit}const doFetch = options.fetch ?? fetch;\n"));
    out.push_str(&format!(
        "{unit}const response = await doFetch(`${{options.baseUrl ?? baseUrl}}${{path}}${{encodeQuery(query)}}`, {{\n"
    ));
    out.push_str(&format!("{unit}{unit}method,\n"));
    out.push_str(&format!("{unit}{unit}headers,\n"));
    out.push_str(&format!("{unit}{unit}body: payload,\n"));
    out.push_str(&format!("{unit}{unit}signal: options.signal,\n"));
    out.push_str(&format!("{unit}}});\n"));
    out.push_str(&format!("{unit}const data = await readBody(response);\n"));
    match error_handling {
        ErrorHandling::Throw => {
            out.push_str(&format!(
                "{unit}if (!response.ok) throw new ApiError(response, data);\n"
            ));
            out.push_str(&format!("{unit}return data as T;\n"));
        }
        ErrorHandling::Result => {
            out.push_str(&format!(
                "{unit}if (!response.ok) return {{ error: data, response }};\n"
            ));
            out.push_str(&format!("{unit}return {{ data: data as T, response }};\n"));
        }
    }
    out.push_str("}\n\n");
}

fn emit_iface(out: &mut String, op: &Op, param_style: ParamStyle, jsdoc: bool, unit: &str) {
    if !op.has_iface(param_style) {
        return;
    }
    if jsdoc {
        out.push_str(&format!(
            "/** Arguments for `{}` (`{} {}`). */\n",
            op.fn_name, op.method, op.path
        ));
    }
    out.push_str(&format!("export interface {} {{\n", op.iface_name));
    for par in op.iface_params(param_style) {
        if jsdoc {
            if let Some(doc) = &par.description {
                out.push_str(&format!("{unit}/** {} */\n", one_line(doc)));
            }
        }
        out.push_str(&format!(
            "{unit}{}{}: {};\n",
            obj_key(&par.name),
            if par.required { "" } else { "?" },
            par.ty
        ));
    }
    if param_style == ParamStyle::Object {
        if let Some(body) = &op.body {
            if jsdoc {
                let doc = body
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("Request body ({}).", body.content_type));
                out.push_str(&format!("{unit}/** {} */\n", one_line(&doc)));
            }
            out.push_str(&format!(
                "{unit}body{}: {};\n",
                if body.required { "" } else { "?" },
                body.ty
            ));
        }
    }
    out.push_str("}\n\n");
}

/// One argument of a generated function.
struct Arg {
    name: String,
    ty: String,
    required: bool,
    default: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn emit_operation(
    out: &mut String,
    op: &Op,
    param_style: ParamStyle,
    error_handling: ErrorHandling,
    jsdoc: bool,
    unit: &str,
    level: usize,
    method_form: bool,
) {
    let pad = unit.repeat(level);
    let body_pad = unit.repeat(level + 1);

    if jsdoc {
        let mut lines: Vec<String> = Vec::new();
        if let Some(s) = &op.summary {
            lines.push(one_line(s));
        }
        if let Some(d) = &op.description {
            if op.summary.as_deref() != Some(d.as_str()) {
                if !lines.is_empty() {
                    lines.push(String::new());
                }
                lines.extend(d.lines().map(|l| l.trim_end().to_string()));
            }
        }
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("`{} {}`", op.method, op.path));
        if op.deprecated {
            lines.push(String::new());
            lines.push("@deprecated".to_string());
        }
        out.push_str(&format!("{pad}/**\n"));
        for line in lines {
            if line.is_empty() {
                out.push_str(&format!("{pad} *\n"));
            } else {
                out.push_str(&format!("{pad} * {line}\n"));
            }
        }
        out.push_str(&format!("{pad} */\n"));
    }

    // ---- signature -------------------------------------------------------
    let mut args: Vec<Arg> = Vec::new();
    if param_style == ParamStyle::Positional {
        for par in &op.path_params {
            args.push(Arg {
                name: safe_ident(&par.name),
                ty: par.ty.clone(),
                required: true,
                default: None,
            });
        }
        if let Some(body) = &op.body {
            args.push(Arg {
                name: "body".to_string(),
                ty: body.ty.clone(),
                required: body.required,
                default: None,
            });
        }
    }
    if op.has_iface(param_style) {
        let required = op.iface_params(param_style).iter().any(|p| p.required)
            || (param_style == ParamStyle::Object && op.body.as_ref().is_some_and(|b| b.required));
        args.push(Arg {
            name: "params".to_string(),
            ty: op.iface_name.clone(),
            required,
            default: if required {
                None
            } else {
                Some("{}".to_string())
            },
        });
    }
    args.push(Arg {
        name: "options".to_string(),
        ty: "RequestOptions".to_string(),
        required: false,
        default: Some("{}".to_string()),
    });

    let rendered: Vec<String> = args
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let later_required = args[i + 1..].iter().any(|b| b.required);
            if a.required {
                format!("{}: {}", a.name, a.ty)
            } else if later_required {
                format!("{}: {} | undefined", a.name, a.ty)
            } else if let Some(d) = &a.default {
                format!("{}: {} = {}", a.name, a.ty, d)
            } else {
                format!("{}?: {}", a.name, a.ty)
            }
        })
        .collect();

    let ret = match error_handling {
        ErrorHandling::Throw => op.response_ty.clone(),
        ErrorHandling::Result => format!("ApiResult<{}>", op.response_ty),
    };
    let head = if method_form {
        format!("async {}", op.fn_name)
    } else {
        format!("export async function {}", op.fn_name)
    };
    out.push_str(&format!(
        "{pad}{head}({}): Promise<{ret}> {{\n",
        rendered.join(", ")
    ));

    // ---- body ------------------------------------------------------------
    for name in &op.cookie_params {
        out.push_str(&format!(
            "{body_pad}// cookie parameter `{name}` is not sent: scripts cannot set the Cookie header.\n"
        ));
    }
    if !op.form_params.is_empty() {
        out.push_str(&format!(
            "{body_pad}// formData parameters ({}) are not assembled — pass a prepared body via `options`.\n",
            op.form_params
                .iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let path_expr = path_template(&op.path, |name| match param_style {
        ParamStyle::Object => format!(
            "${{encodeURIComponent(String({}))}}",
            prop_access("params", name)
        ),
        ParamStyle::Positional => {
            format!("${{encodeURIComponent(String({}))}}", safe_ident(name))
        }
    });

    let query_obj = record_literal(&op.query_params);
    let header_obj = record_literal(&op.header_params);
    let body_arg = match (&op.body, param_style) {
        (None, _) => "undefined".to_string(),
        (Some(b), ParamStyle::Object) => format!(
            "{{ value: params.body, contentType: \"{}\" }}",
            escape_double_quoted(&b.content_type)
        ),
        (Some(b), ParamStyle::Positional) => format!(
            "{{ value: body, contentType: \"{}\" }}",
            escape_double_quoted(&b.content_type)
        ),
    };
    let options_arg = if method_form {
        "this.merge(options)"
    } else {
        "options"
    };

    out.push_str(&format!(
        "{body_pad}return apiFetch(\"{}\", `{}`, {}, {}, {}, {});\n",
        op.method, path_expr, query_obj, header_obj, body_arg, options_arg
    ));
    out.push_str(&format!("{pad}}}\n"));
    if !method_form {
        out.push('\n');
    }
}

/// `{ limit: params.limit, "x-page": params["x-page"] }`
fn record_literal(params: &[Par]) -> String {
    if params.is_empty() {
        return "{}".to_string();
    }
    let fields: Vec<String> = params
        .iter()
        .map(|p| format!("{}: {}", obj_key(&p.name), prop_access("params", &p.name)))
        .collect();
    format!("{{ {} }}", fields.join(", "))
}

/// Build a template-literal body, substituting `{name}` placeholders.
fn path_template(path: &str, mut expr_for: impl FnMut(&str) -> String) -> String {
    let mut out = String::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            let mut closed = false;
            for inner in chars.by_ref() {
                if inner == '}' {
                    closed = true;
                    break;
                }
                name.push(inner);
            }
            if closed && !name.is_empty() {
                out.push_str(&expr_for(name.trim()));
            } else {
                out.push_str("\\{");
                out.push_str(&name);
            }
            continue;
        }
        match c {
            '\\' => out.push_str("\\\\"),
            '`' => out.push_str("\\`"),
            '$' => out.push_str("\\$"),
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// identifiers + escaping
// ---------------------------------------------------------------------------

/// `params.limit` or `params["x-page"]` for names that are not identifiers.
fn prop_access(base: &str, name: &str) -> String {
    if is_ident(name) {
        format!("{base}.{name}")
    } else {
        format!("{base}[\"{}\"]", escape_double_quoted(name))
    }
}

/// An object-literal / interface key, quoted only when it has to be.
fn obj_key(name: &str) -> String {
    if is_ident(name) {
        name.to_string()
    } else {
        format!("\"{}\"", escape_double_quoted(name))
    }
}

fn is_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

fn escape_double_quoted(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' => "\\\\".to_string(),
            '"' => "\\\"".to_string(),
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            _ => c.to_string(),
        })
        .collect()
}

/// Collapse a doc string onto one line and neutralise `*/`.
fn one_line(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("*/", "*\\/")
}

/// Split an arbitrary name into word chunks (handles snake, kebab, camel).
fn words(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut prev_lower_or_digit = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && prev_lower_or_digit && !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            current.push(c);
            prev_lower_or_digit = c.is_ascii_lowercase() || c.is_ascii_digit();
        } else {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            prev_lower_or_digit = false;
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// `list_pets` / `Pets_List` / `listPets` → `listPets`.
fn camel_ident(s: &str) -> String {
    let parts = words(s);
    if parts.is_empty() {
        return "operation".to_string();
    }
    let mut out = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            let mut chars = part.chars();
            let first = chars.next().unwrap().to_ascii_lowercase();
            out.push(first);
            out.push_str(chars.as_str());
        } else {
            out.push_str(&capitalize(part));
        }
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "op");
    }
    out
}

/// PascalCase identifier, falling back to `fallback` when nothing survives.
fn pascal_ident(s: &str, fallback: &str) -> String {
    let parts = words(s);
    if parts.is_empty() {
        return fallback.to_string();
    }
    let mut out: String = parts.iter().map(|p| capitalize(p)).collect();
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// A safe positional argument name.
fn safe_ident(s: &str) -> String {
    let ident = camel_ident(s);
    if ident.is_empty() {
        "value".to_string()
    } else {
        ident
    }
}

/// `getPets`, `getPetsByPetId` — derived from method + path.
fn derive_name(method: &str, path: &str) -> String {
    let mut parts: Vec<String> = vec![method.to_string()];
    for segment in path.split('/') {
        if segment.is_empty() {
            continue;
        }
        if let Some(inner) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            parts.push("by".to_string());
            parts.extend(words(inner));
        } else {
            parts.extend(words(segment));
        }
    }
    camel_ident(&parts.join("_"))
}

/// Guarantee unique function names across the document.
fn unique_name(base: String, used: &mut HashMap<String, usize>) -> String {
    let count = used.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}{}", *count)
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const PETSTORE: &str = r##"{
      "openapi": "3.0.3",
      "servers": [{ "url": "https://api.example.com/v1" }],
      "paths": {
        "/pets": {
          "get": {
            "operationId": "listPets",
            "summary": "List all pets",
            "tags": ["pets"],
            "parameters": [
              { "name": "limit", "in": "query", "description": "How many to return",
                "schema": { "type": "integer" } }
            ],
            "responses": {
              "200": { "content": { "application/json": {
                "schema": { "type": "array", "items": { "$ref": "#/components/schemas/Pet" } } } } }
            }
          },
          "post": {
            "operationId": "createPet",
            "tags": ["pets"],
            "requestBody": {
              "required": true,
              "content": { "application/json": { "schema": { "$ref": "#/components/schemas/NewPet" } } }
            },
            "responses": { "201": { "content": { "application/json": {
              "schema": { "$ref": "#/components/schemas/Pet" } } } } }
          }
        },
        "/pets/{petId}": {
          "get": {
            "operationId": "showPetById",
            "tags": ["admin"],
            "parameters": [
              { "name": "petId", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": { "200": { "content": { "application/json": {
              "schema": { "$ref": "#/components/schemas/Pet" } } } } }
          }
        }
      }
    }"##;

    fn gen(spec: &str) -> String {
        generate(spec, "auto", "", "", "", "", "", "", "./types", "", true, 2).unwrap()
    }

    #[test]
    fn happy_path_generates_typed_functions() {
        let out = gen(PETSTORE);
        assert!(out.contains("export const baseUrl = \"https://api.example.com/v1\";"));
        assert!(out.contains("import type { NewPet, Pet } from \"./types\";"));
        assert!(out.contains("export interface ListPetsRequest {"));
        assert!(out.contains("  limit?: number;"));
        assert!(out.contains(
            "export async function listPets(params: ListPetsRequest = {}, options: RequestOptions = {}): Promise<Pet[]> {"
        ));
        assert!(out.contains(
            "return apiFetch(\"GET\", `/pets`, { limit: params.limit }, {}, undefined, options);"
        ));
        // Path params are substituted and encoded.
        assert!(out.contains("`/pets/${encodeURIComponent(String(params.petId))}`"));
        // A required body lands in the request interface.
        assert!(out.contains("  body: NewPet;"));
        assert!(out.contains("{ value: params.body, contentType: \"application/json\" }"));
        // Default error handling throws.
        assert!(out.contains("export class ApiError extends Error {"));
        assert!(!out.contains("ApiResult"));
    }

    #[test]
    fn empty_spec_is_an_error() {
        let err = generate(
            "   ", "auto", "", "", "", "", "", "", "./types", "", true, 2,
        )
        .unwrap_err();
        assert!(err.contains("spec is empty"), "{err}");
    }

    #[test]
    fn document_without_paths_is_an_error() {
        let err = generate(
            r#"{"openapi":"3.0.0","components":{"schemas":{}}}"#,
            "auto",
            "",
            "",
            "",
            "",
            "",
            "",
            "./types",
            "",
            true,
            2,
        )
        .unwrap_err();
        assert!(err.contains("no `paths` object"), "{err}");
    }

    #[test]
    fn unparseable_input_is_an_error() {
        let err = generate(
            "{ nope", "json", "", "", "", "", "", "", "./types", "", true, 2,
        )
        .unwrap_err();
        assert!(err.contains("invalid JSON"), "{err}");
    }

    #[test]
    fn unknown_enum_value_is_an_error() {
        let err = generate(
            PETSTORE,
            "auto",
            "",
            "",
            "",
            "",
            "sometimes",
            "",
            "./types",
            "",
            true,
            2,
        )
        .unwrap_err();
        assert!(err.contains("unknown error_handling"), "{err}");
    }

    #[test]
    fn yaml_input_and_swagger_two_base_url() {
        let spec = r##"
swagger: "2.0"
host: petstore.example.com
basePath: /api
schemes: [https]
paths:
  /pets:
    post:
      operationId: addPet
      parameters:
        - name: body
          in: body
          required: true
          schema:
            $ref: "#/definitions/Pet"
      responses:
        "200":
          schema:
            $ref: "#/definitions/Pet"
"##;
        let out = gen(spec);
        assert!(out.contains("export const baseUrl = \"https://petstore.example.com/api\";"));
        assert!(out.contains("import type { Pet } from \"./types\";"));
        assert!(out.contains("  body: Pet;"));
        assert!(out.contains("): Promise<Pet> {"));
    }

    #[test]
    fn result_error_handling_returns_a_union() {
        let out = generate(
            PETSTORE, "auto", "", "", "", "", "result", "", "./types", "", true, 2,
        )
        .unwrap();
        assert!(out.contains("export type ApiResult<T> ="));
        assert!(out.contains("): Promise<ApiResult<Pet[]>> {"));
        assert!(out.contains("if (!response.ok) return { error: data, response };"));
        assert!(!out.contains("class ApiError"));
    }

    #[test]
    fn positional_param_style_lifts_path_params_out() {
        let out = generate(
            PETSTORE,
            "auto",
            "",
            "",
            "",
            "positional",
            "",
            "",
            "./types",
            "",
            true,
            2,
        )
        .unwrap();
        assert!(out.contains(
            "export async function showPetById(petId: string, options: RequestOptions = {}): Promise<Pet> {"
        ));
        assert!(out.contains("`/pets/${encodeURIComponent(String(petId))}`"));
        // The body becomes its own positional argument.
        assert!(out.contains(
            "export async function createPet(body: NewPet, options: RequestOptions = {}): Promise<Pet> {"
        ));
        assert!(out.contains("{ value: body, contentType: \"application/json\" }"));
    }

    #[test]
    fn class_style_emits_one_client_with_methods() {
        let out = generate(
            PETSTORE, "auto", "class", "PetStore", "", "", "", "", "./types", "", true, 2,
        )
        .unwrap();
        assert!(out.contains("export class PetStore {"));
        assert!(out.contains("  constructor(private readonly options: RequestOptions = {}) {}"));
        assert!(out.contains("  async listPets(params: ListPetsRequest = {}, options: RequestOptions = {}): Promise<Pet[]> {"));
        assert!(out.contains("this.merge(options)"));
        assert!(!out.contains("export async function"));
    }

    #[test]
    fn tag_filter_and_path_naming() {
        let out = generate(
            PETSTORE, "auto", "", "", "path", "", "", "", "./types", "admin", true, 2,
        )
        .unwrap();
        assert!(out.contains("export async function getPetsByPetId("));
        assert!(!out.contains("listPets"));
    }

    #[test]
    fn unmatched_tag_filter_is_an_error() {
        let err = generate(
            PETSTORE, "auto", "", "", "", "", "", "", "./types", "ghosts", true, 2,
        )
        .unwrap_err();
        assert!(err.contains("no operation matched the tag filter"), "{err}");
    }

    #[test]
    fn blank_types_module_emits_placeholder_aliases() {
        let out = generate(PETSTORE, "auto", "", "", "", "", "", "", "", "", true, 2).unwrap();
        assert!(out.contains("export type Pet = unknown;"));
        assert!(out.contains("export type NewPet = unknown;"));
        assert!(!out.contains("import type"));
    }

    #[test]
    fn base_url_override_and_jsdoc_off() {
        let out = generate(
            PETSTORE,
            "auto",
            "",
            "",
            "",
            "",
            "",
            "https://staging.example.com/",
            "./types",
            "",
            false,
            2,
        )
        .unwrap();
        assert!(out.contains("export const baseUrl = \"https://staging.example.com\";"));
        assert!(!out.contains("List all pets"));
        assert!(!out.contains("/**"));
    }

    #[test]
    fn indent_is_configurable_and_clamped() {
        let out = generate(
            PETSTORE, "auto", "", "", "", "", "", "", "./types", "", true, 4,
        )
        .unwrap();
        assert!(out.contains("\n    limit?: number;"));
        let flat = generate(
            PETSTORE, "auto", "", "", "", "", "", "", "./types", "", true, 99,
        )
        .unwrap();
        assert!(flat.contains("\n        limit?: number;"), "clamped to 8");
    }

    #[test]
    fn odd_param_names_query_and_headers_are_quoted() {
        let spec = r##"{
          "openapi": "3.1.0",
          "paths": { "/search": { "get": {
            "operationId": "search",
            "parameters": [
              { "name": "x-api-key", "in": "header", "required": true, "schema": { "type": "string" } },
              { "name": "sort[]", "in": "query", "schema": { "type": "array", "items": { "type": "string" } } },
              { "name": "session", "in": "cookie", "schema": { "type": "string" } }
            ],
            "responses": { "200": { "content": { "application/json": {
              "schema": { "type": "object", "properties": { "hits": { "type": "integer" } }, "required": ["hits"] } } } } }
          } } }
        }"##;
        let out = gen(spec);
        assert!(out.contains("\"x-api-key\": string;"));
        assert!(out.contains("\"sort[]\"?: string[];"));
        assert!(out.contains("{ \"sort[]\": params[\"sort[]\"] }"));
        assert!(out.contains("{ \"x-api-key\": params[\"x-api-key\"] }"));
        assert!(out.contains("// cookie parameter `session` is not sent"));
        assert!(out.contains("): Promise<{ hits: number }> {"));
        // A required param means no `= {}` default.
        assert!(out.contains("export async function search(params: SearchRequest, options: RequestOptions = {}): Promise<"));
    }

    #[test]
    fn shared_path_parameters_and_duplicate_names() {
        let spec = r##"{
          "openapi": "3.0.0",
          "paths": {
            "/a/{id}": {
              "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }],
              "get": { "responses": { "204": {} } },
              "delete": { "responses": { "204": {} } }
            }
          }
        }"##;
        let out = gen(spec);
        assert!(out.contains("export async function getAById("));
        assert!(out.contains("export async function deleteAById("));
        assert!(out.contains("  id: number;"));
        assert!(out.contains("): Promise<void> {"));
    }

    #[test]
    fn operation_ids_that_collide_get_suffixed() {
        let spec = r##"{
          "openapi": "3.0.0",
          "paths": {
            "/a": { "get": { "operationId": "ping", "responses": { "200": {} } } },
            "/b": { "get": { "operationId": "Ping", "responses": { "200": {} } } }
          }
        }"##;
        let out = gen(spec);
        assert!(out.contains("export async function ping("));
        assert!(out.contains("export async function ping2("));
    }

    #[test]
    fn non_json_body_is_passed_through_with_its_content_type() {
        let spec = r##"{
          "openapi": "3.0.0",
          "paths": { "/upload": { "post": {
            "operationId": "upload",
            "requestBody": { "required": true, "content": { "application/octet-stream": {
              "schema": { "type": "string", "format": "binary" } } } },
            "responses": { "200": {} }
          } } }
        }"##;
        let out = gen(spec);
        assert!(out.contains("contentType: \"application/octet-stream\""));
    }

    #[test]
    fn output_is_deterministic() {
        assert_eq!(gen(PETSTORE), gen(PETSTORE));
    }
}
