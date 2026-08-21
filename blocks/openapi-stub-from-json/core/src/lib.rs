//! openapi-stub-from-json core — turn a sample request body and/or response body
//! into a ready-to-paste OpenAPI **3.1** path-and-operation stub. Pure Rust
//! (serde_json with preserved key order + serde_yml); no I/O, no network, and
//! byte-identical output for a given input.
//!
//! Pipeline: parse the sample bodies → infer a JSON Schema for each (arrays are
//! MERGED across every element, so keys missing from some elements drop out of
//! `required` and mixed scalars become a type array) → optionally hoist nested
//! objects into named `components.schemas` entries → assemble `paths` → method →
//! `parameters` / `requestBody` / `responses` (+ optional `securitySchemes` and
//! generic 400/500 error responses) → serialize as YAML or JSON.
//!
//! Scope is a deterministic *stub*, not a finished specification: a sample can
//! only reveal what it contains. Nulls hide the underlying type, empty arrays
//! hide their item type, and no sample can reveal enums, validation constraints,
//! headers, or auth scopes — those are documented limits, not silent guesses.

use serde_json::{json, Map, Value};

/// Per-sample input cap (1 MB each for request and response).
pub const MAX_INPUT_BYTES: usize = 1_000_000;
/// Maximum JSON nesting the inferrer will walk before giving up.
pub const MAX_DEPTH: usize = 64;
/// The OpenAPI version this tool emits.
pub const OPENAPI_VERSION: &str = "3.1.0";

/// HTTP methods that can carry an operation, in the order OpenAPI lists them.
pub static METHODS: [&str; 7] = ["get", "post", "put", "patch", "delete", "head", "options"];
/// Accepted `security` choices.
pub static SECURITY_KINDS: [&str; 4] = ["none", "bearer", "basic", "apiKey"];
/// Accepted `format` choices.
pub static OUTPUT_FORMATS: [&str; 2] = ["yaml", "json"];

/// Everything that shapes the generated stub. Defaults match the descriptor.
#[derive(Debug, Clone)]
pub struct Options {
    /// HTTP method for the operation (lowercased on use).
    pub method: String,
    /// Request path, e.g. `/users/{id}`. Must start with `/`.
    pub path: String,
    /// `operationId`; blank derives one from the method + path.
    pub operation_id: String,
    /// Operation tag; blank uses the first literal path segment.
    pub tag: String,
    /// Status code the sample response body belongs to.
    pub status: u16,
    /// Media type for both request and response bodies.
    pub content_type: String,
    /// `info.title`; blank becomes `Sample API`.
    pub title: String,
    /// `info.version`; blank becomes `1.0.0`.
    pub api_version: String,
    /// Single `servers[0].url`; blank omits `servers` entirely.
    pub server_url: String,
    /// Sample query string (`limit=10&active=true`) → typed query parameters.
    pub query: String,
    /// `none` | `bearer` | `basic` | `apiKey`.
    pub security: String,
    /// `yaml` (default) or `json`.
    pub format: String,
    /// Put body schemas in `components.schemas` and `$ref` them.
    pub components: bool,
    /// Also hoist nested objects into their own component schemas.
    pub extract_nested: bool,
    /// Emit a `required` list on inferred objects.
    pub required_props: bool,
    /// Detect string `format`s (email, uri, date-time, date, uuid, ipv4).
    pub detect_formats: bool,
    /// Carry the samples through as media-type `example`s.
    pub include_examples: bool,
    /// Add generic 400 + 500 responses backed by an `Error` schema.
    pub include_error_responses: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            method: "post".into(),
            path: "/items".into(),
            operation_id: String::new(),
            tag: String::new(),
            status: 200,
            content_type: "application/json".into(),
            title: String::new(),
            api_version: String::new(),
            server_url: String::new(),
            query: String::new(),
            security: "none".into(),
            format: "yaml".into(),
            components: true,
            extract_nested: false,
            required_props: true,
            detect_formats: true,
            include_examples: true,
            include_error_responses: false,
        }
    }
}

/// Generate the stub. `request_json` and/or `response_json` may be blank. Both
/// may be blank only for a no-content response such as 204.
pub fn generate(request_json: &str, response_json: &str, o: &Options) -> Result<String, String> {
    let req_src = request_json.trim();
    let resp_src = response_json.trim();
    if req_src.is_empty() && resp_src.is_empty() && o.status != 204 {
        return Err(
            "provide a sample request body, a sample response body, or use status=204 for a no-content operation (both were blank)"
                .into(),
        );
    }
    if req_src.len() > MAX_INPUT_BYTES || resp_src.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "sample body too large: the limit is {} bytes per body",
            MAX_INPUT_BYTES
        ));
    }

    let format = normalize_choice(&o.format, &OUTPUT_FORMATS, "format")?;
    let method_lc = o.method.trim().to_ascii_lowercase();
    let method = normalize_choice(&method_lc, &METHODS, "method")?;
    let security = normalize_choice(&o.security, &SECURITY_KINDS, "security")?;

    let path = o.path.trim();
    if !path.starts_with('/') {
        return Err(format!("path must start with '/' (got '{path}')"));
    }
    if path.chars().any(char::is_whitespace) {
        return Err("path must not contain spaces".into());
    }
    if !(100..=599).contains(&o.status) {
        return Err(format!(
            "status must be an HTTP status code between 100 and 599 (got {})",
            o.status
        ));
    }
    let content_type = o.content_type.trim();
    if content_type.is_empty() {
        return Err("content_type must not be blank (e.g. application/json)".into());
    }

    let req_value = parse_sample(req_src, "request_json")?;
    let resp_value = parse_sample(resp_src, "response_json")?;

    // Nested extraction only makes sense once bodies live in components.
    let use_components = o.components || o.extract_nested;

    let base = component_base(&o.operation_id, path);
    let mut schemas: Map<String, Value> = Map::new();

    let mut req_schema = match &req_value {
        Some(v) => Some(infer(v, o, 0)?),
        None => None,
    };
    let mut resp_schema = match &resp_value {
        Some(v) => Some(infer(v, o, 0)?),
        None => None,
    };

    if o.extract_nested {
        if let Some(s) = req_schema.as_mut() {
            hoist_children(s, &mut schemas);
        }
        if let Some(s) = resp_schema.as_mut() {
            hoist_children(s, &mut schemas);
        }
    }

    // Body schemas become components (and `$ref`s) unless inlining was asked for.
    let req_ref = req_schema.map(|s| {
        if use_components {
            let name = register(&format!("{base}Request"), s, &mut schemas);
            schema_ref(&name)
        } else {
            s
        }
    });
    let resp_ref = resp_schema.map(|s| {
        if use_components {
            let name = register(&format!("{base}Response"), s, &mut schemas);
            schema_ref(&name)
        } else {
            s
        }
    });

    let mut operation = Map::new();
    operation.insert(
        "operationId".into(),
        Value::String(if o.operation_id.trim().is_empty() {
            derive_operation_id(method, path)
        } else {
            o.operation_id.trim().to_string()
        }),
    );
    operation.insert(
        "summary".into(),
        Value::String(derive_summary(method, path)),
    );
    operation.insert(
        "tags".into(),
        json!([if o.tag.trim().is_empty() {
            derive_tag(path)
        } else {
            o.tag.trim().to_string()
        }]),
    );

    let mut parameters: Vec<Value> = path_parameters(path);
    parameters.extend(query_parameters(&o.query, o)?);
    if !parameters.is_empty() {
        operation.insert("parameters".into(), Value::Array(parameters));
    }

    if let Some(schema) = req_ref {
        let mut body = Map::new();
        body.insert("required".into(), Value::Bool(true));
        body.insert(
            "content".into(),
            single(content_type, media_type(schema, req_value.as_ref(), o)),
        );
        operation.insert("requestBody".into(), Value::Object(body));
    }

    let mut responses = Map::new();
    let mut ok = Map::new();
    ok.insert(
        "description".into(),
        Value::String(reason_phrase(o.status).to_string()),
    );
    if let Some(schema) = resp_ref {
        ok.insert(
            "content".into(),
            single(content_type, media_type(schema, resp_value.as_ref(), o)),
        );
    }
    responses.insert(o.status.to_string(), Value::Object(ok));

    if o.include_error_responses {
        let error_schema = error_schema(o);
        let error_ref = if use_components {
            let name = register("Error", error_schema, &mut schemas);
            schema_ref(&name)
        } else {
            error_schema
        };
        for code in [400u16, 500u16] {
            let mut resp = Map::new();
            resp.insert(
                "description".into(),
                Value::String(reason_phrase(code).to_string()),
            );
            resp.insert(
                "content".into(),
                single(content_type, json!({ "schema": error_ref.clone() })),
            );
            responses.insert(code.to_string(), Value::Object(resp));
        }
    }
    operation.insert("responses".into(), Value::Object(responses));

    let scheme = security_scheme(security);
    if let Some((name, _)) = &scheme {
        operation.insert(
            "security".into(),
            Value::Array(vec![single(name, Value::Array(Vec::new()))]),
        );
    }

    let mut doc = Map::new();
    doc.insert("openapi".into(), Value::String(OPENAPI_VERSION.into()));
    doc.insert(
        "info".into(),
        json!({
            "title": blank_or(&o.title, "Sample API"),
            "version": blank_or(&o.api_version, "1.0.0"),
        }),
    );
    if !o.server_url.trim().is_empty() {
        doc.insert("servers".into(), json!([{ "url": o.server_url.trim() }]));
    }
    doc.insert(
        "paths".into(),
        single(path, single(method, Value::Object(operation))),
    );

    let mut components = Map::new();
    if !schemas.is_empty() {
        components.insert("schemas".into(), Value::Object(schemas));
    }
    if let Some((name, def)) = scheme {
        components.insert("securitySchemes".into(), single(&name, def));
    }
    if !components.is_empty() {
        doc.insert("components".into(), Value::Object(components));
    }

    let doc = Value::Object(doc);
    match format {
        "json" => {
            serde_json::to_string_pretty(&doc).map_err(|e| format!("JSON encode failed: {e}"))
        }
        _ => serde_yml::to_string(&doc).map_err(|e| format!("YAML encode failed: {e}")),
    }
}

/* ---------------------------------------------------------------- inference */

/// Infer an OpenAPI 3.1 (JSON Schema 2020-12) schema for one sample value.
fn infer(v: &Value, o: &Options, depth: usize) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "sample nests deeper than {MAX_DEPTH} levels — flatten it or trim the sample"
        ));
    }
    Ok(match v {
        Value::Null => json!({ "type": "null" }),
        Value::Bool(_) => json!({ "type": "boolean" }),
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                json!({ "type": "integer" })
            } else {
                json!({ "type": "number" })
            }
        }
        Value::String(s) => {
            let mut m = Map::new();
            m.insert("type".into(), Value::String("string".into()));
            if o.detect_formats {
                if let Some(f) = detect_format(s) {
                    m.insert("format".into(), Value::String(f.into()));
                }
            }
            Value::Object(m)
        }
        Value::Array(items) => {
            let mut merged: Option<Value> = None;
            for item in items {
                let s = infer(item, o, depth + 1)?;
                merged = Some(match merged {
                    None => s,
                    Some(prev) => merge(prev, s),
                });
            }
            let mut m = Map::new();
            m.insert("type".into(), Value::String("array".into()));
            // An empty array reveals nothing about its items — `{}` says "any".
            m.insert("items".into(), merged.unwrap_or_else(|| json!({})));
            Value::Object(m)
        }
        Value::Object(fields) => {
            let mut props = Map::new();
            let mut required = Vec::new();
            for (k, val) in fields {
                props.insert(k.clone(), infer(val, o, depth + 1)?);
                required.push(Value::String(k.clone()));
            }
            let mut m = Map::new();
            m.insert("type".into(), Value::String("object".into()));
            m.insert("properties".into(), Value::Object(props));
            if o.required_props && !required.is_empty() {
                m.insert("required".into(), Value::Array(required));
            }
            Value::Object(m)
        }
    })
}

/// Merge two inferred schemas (used across the elements of one array).
fn merge(a: Value, b: Value) -> Value {
    let (Value::Object(a), Value::Object(b)) = (a, b) else {
        return json!({});
    };
    // Anything merged with the "any" schema stays "any".
    if a.is_empty() || b.is_empty() {
        return json!({});
    }

    let mut types = type_list(&a);
    for t in type_list(&b) {
        if !types.contains(&t) {
            types.push(t);
        }
    }
    // JSON Schema `number` already admits integers.
    if types.iter().any(|t| t == "number") {
        types.retain(|t| t != "integer");
    }

    let mut out = Map::new();
    out.insert(
        "type".into(),
        if types.len() == 1 {
            Value::String(types[0].clone())
        } else {
            Value::Array(types.iter().map(|t| Value::String(t.clone())).collect())
        },
    );

    // A `format` only survives when both sides agree on it.
    if let (Some(fa), Some(fb)) = (a.get("format"), b.get("format")) {
        if fa == fb {
            out.insert("format".into(), fa.clone());
        }
    }

    match (a.get("items"), b.get("items")) {
        (Some(ia), Some(ib)) => {
            out.insert("items".into(), merge(ia.clone(), ib.clone()));
        }
        (Some(i), None) | (None, Some(i)) => {
            out.insert("items".into(), i.clone());
        }
        (None, None) => {}
    }

    let pa = a.get("properties").and_then(Value::as_object);
    let pb = b.get("properties").and_then(Value::as_object);
    if pa.is_some() || pb.is_some() {
        let empty = Map::new();
        let pa = pa.unwrap_or(&empty);
        let pb = pb.unwrap_or(&empty);
        let mut props = Map::new();
        for (k, v) in pa {
            props.insert(
                k.clone(),
                match pb.get(k) {
                    Some(other) => merge(v.clone(), other.clone()),
                    None => v.clone(),
                },
            );
        }
        for (k, v) in pb {
            if !props.contains_key(k) {
                props.insert(k.clone(), v.clone());
            }
        }
        out.insert("properties".into(), Value::Object(props));

        // Only keys present in EVERY merged sample stay required.
        let ra = required_set(&a);
        let rb = required_set(&b);
        let kept: Vec<Value> = ra
            .iter()
            .filter(|k| rb.contains(*k))
            .map(|k| Value::String(k.clone()))
            .collect();
        if !kept.is_empty() {
            out.insert("required".into(), Value::Array(kept));
        }
    }

    Value::Object(out)
}

fn type_list(m: &Map<String, Value>) -> Vec<String> {
    match m.get("type") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str())
            .map(String::from)
            .collect(),
        _ => Vec::new(),
    }
}

fn required_set(m: &Map<String, Value>) -> Vec<String> {
    m.get("required")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Pattern-based `format` detection — deliberately conservative.
fn detect_format(s: &str) -> Option<&'static str> {
    if s.is_empty() || s.len() > 2048 {
        return None;
    }
    if is_uuid(s) {
        return Some("uuid");
    }
    if is_date_time(s) {
        return Some("date-time");
    }
    if is_date(s) {
        return Some("date");
    }
    if is_email(s) {
        return Some("email");
    }
    if is_ipv4(s) {
        return Some("ipv4");
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return Some("uri");
    }
    None
}

fn is_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && [8usize, 13, 18, 23].iter().all(|i| b[*i] == b'-')
        && b.iter()
            .enumerate()
            .all(|(i, c)| [8, 13, 18, 23].contains(&i) || c.is_ascii_hexdigit())
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

fn is_date_time(s: &str) -> bool {
    // RFC 3339: a full date, a `T`/space separator, then at least hh:mm:ss.
    s.len() >= 19 && is_date(&s[..10]) && matches!(s.as_bytes()[10], b'T' | b't' | b' ') && {
        let t = s.as_bytes();
        t[13] == b':' && t[16] == b':' && t[11..13].iter().all(u8::is_ascii_digit)
    }
}

fn is_email(s: &str) -> bool {
    let mut parts = s.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !s.chars().any(char::is_whitespace)
}

fn is_ipv4(s: &str) -> bool {
    let octets: Vec<&str> = s.split('.').collect();
    octets.len() == 4
        && octets.iter().all(|o| {
            !o.is_empty() && o.len() <= 3 && o.chars().all(|c| c.is_ascii_digit()) && {
                o.parse::<u16>().map(|n| n <= 255).unwrap_or(false)
            }
        })
}

/* --------------------------------------------------------------- components */

fn schema_ref(name: &str) -> Value {
    json!({ "$ref": format!("#/components/schemas/{name}") })
}

/// Insert `schema` under `name`, reusing an identical existing entry and
/// suffixing (`Foo2`, `Foo3`, …) on a genuine collision. Returns the name used.
fn register(name: &str, schema: Value, schemas: &mut Map<String, Value>) -> String {
    let mut candidate = name.to_string();
    let mut n = 2;
    loop {
        match schemas.get(&candidate) {
            None => {
                schemas.insert(candidate.clone(), schema);
                return candidate;
            }
            Some(existing) if *existing == schema => return candidate,
            Some(_) => {
                candidate = format!("{name}{n}");
                n += 1;
            }
        }
    }
}

/// Hoist every nested object below `schema` into its own component, deepest
/// first, so hoisted components themselves contain `$ref`s.
fn hoist_children(schema: &mut Value, schemas: &mut Map<String, Value>) {
    if let Some(items) = schema.get_mut("items") {
        hoist_children(items, schemas);
    }
    let Some(props) = schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .map(|p| p.keys().cloned().collect::<Vec<_>>())
    else {
        return;
    };
    for key in props {
        let Some(child) = schema
            .get_mut("properties")
            .and_then(Value::as_object_mut)
            .and_then(|p| p.get_mut(&key))
        else {
            continue;
        };
        hoist_children(child, schemas);

        if is_object_schema(child) {
            let name = register(&pascal(&singular(&key)), child.take(), schemas);
            *child = schema_ref(&name);
        } else if child.get("type").and_then(Value::as_str) == Some("array") {
            if let Some(items) = child.get_mut("items") {
                if is_object_schema(items) {
                    let name = register(&pascal(&singular(&key)), items.take(), schemas);
                    *items = schema_ref(&name);
                }
            }
        }
    }
}

fn is_object_schema(v: &Value) -> bool {
    v.get("type").and_then(Value::as_str) == Some("object")
        && v.get("properties")
            .and_then(Value::as_object)
            .map(|p| !p.is_empty())
            .unwrap_or(false)
}

fn error_schema(o: &Options) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), Value::String("object".into()));
    m.insert(
        "properties".into(),
        json!({
            "error": { "type": "string" },
            "message": { "type": "string" },
        }),
    );
    if o.required_props {
        m.insert("required".into(), json!(["error"]));
    }
    Value::Object(m)
}

fn security_scheme(kind: &str) -> Option<(String, Value)> {
    match kind {
        "bearer" => Some((
            "BearerAuth".into(),
            json!({ "type": "http", "scheme": "bearer", "bearerFormat": "JWT" }),
        )),
        "basic" => Some((
            "BasicAuth".into(),
            json!({ "type": "http", "scheme": "basic" }),
        )),
        "apiKey" => Some((
            "ApiKeyAuth".into(),
            json!({ "type": "apiKey", "in": "header", "name": "X-API-Key" }),
        )),
        _ => None,
    }
}

/* --------------------------------------------------------------- parameters */

/// Every `{braced}` path segment becomes a required path parameter.
fn path_parameters(path: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start + 1..].find('}') else {
            break;
        };
        let name = &rest[start + 1..start + 1 + len];
        if !name.is_empty() && !out.iter().any(|p: &Value| p["name"] == json!(name)) {
            out.push(json!({
                "name": name,
                "in": "path",
                "required": true,
                "description": format!("Path parameter `{name}`."),
                "schema": { "type": "string" },
            }));
        }
        rest = &rest[start + 1 + len + 1..];
    }
    out
}

/// Parse a sample query string into typed, optional query parameters.
fn query_parameters(query: &str, o: &Options) -> Result<Vec<Value>, String> {
    let q = query.trim().trim_start_matches('?');
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let mut out: Vec<Value> = Vec::new();
    for pair in q.split('&').filter(|p| !p.is_empty()) {
        let (name, value) = match pair.split_once('=') {
            Some((n, v)) => (n.trim(), v.trim()),
            None => (pair.trim(), ""),
        };
        if name.is_empty() {
            return Err(format!("query parameter without a name in '{pair}'"));
        }
        if out.iter().any(|p| p["name"] == json!(name)) {
            continue;
        }
        let mut param = Map::new();
        param.insert("name".into(), Value::String(name.into()));
        param.insert("in".into(), Value::String("query".into()));
        param.insert("required".into(), Value::Bool(false));
        param.insert(
            "description".into(),
            Value::String(format!("Query parameter `{name}`.")),
        );
        param.insert("schema".into(), infer(&scalar_from_text(value), o, 0)?);
        if o.include_examples && !value.is_empty() {
            param.insert("example".into(), scalar_from_text(value));
        }
        out.push(Value::Object(param));
    }
    Ok(out)
}

/// Interpret a raw query-string value as a bool / integer / number / string.
fn scalar_from_text(s: &str) -> Value {
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => {
            if let Ok(i) = s.parse::<i64>() {
                json!(i)
            } else if let Ok(f) = s.parse::<f64>() {
                // Reject inf/NaN, which have no JSON representation.
                if f.is_finite() {
                    json!(f)
                } else {
                    Value::String(s.into())
                }
            } else {
                Value::String(s.into())
            }
        }
    }
}

/* ------------------------------------------------------------------ helpers */

/// A one-entry JSON object — used wherever the key is a runtime string.
fn single(key: &str, value: Value) -> Value {
    let mut m = Map::new();
    m.insert(key.to_string(), value);
    Value::Object(m)
}

fn media_type(schema: Value, sample: Option<&Value>, o: &Options) -> Value {
    let mut m = Map::new();
    m.insert("schema".into(), schema);
    if o.include_examples {
        if let Some(v) = sample {
            m.insert("example".into(), v.clone());
        }
    }
    Value::Object(m)
}

fn parse_sample(src: &str, field: &str) -> Result<Option<Value>, String> {
    if src.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(src)
        .map(Some)
        .map_err(|e| format!("{field} is not valid JSON: {e}"))
}

fn normalize_choice<'a>(
    value: &str,
    allowed: &'a [&'a str],
    field: &str,
) -> Result<&'a str, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(allowed[0]);
    }
    allowed
        .iter()
        .find(|a| **a == v)
        .copied()
        .ok_or_else(|| format!("{field} must be one of {} (got '{v}')", allowed.join(", ")))
}

fn blank_or(value: &str, fallback: &str) -> String {
    let v = value.trim();
    if v.is_empty() {
        fallback.to_string()
    } else {
        v.to_string()
    }
}

/// Literal (non-`{braced}`) path segments.
fn literal_segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|s| !s.is_empty() && !s.starts_with('{'))
        .collect()
}

fn derive_tag(path: &str) -> String {
    literal_segments(path)
        .first()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "default".into())
}

/// `post` + `/users/{id}/orders` → `postUsersByIdOrders`.
fn derive_operation_id(method: &str, path: &str) -> String {
    let mut id = method.to_string();
    for seg in path.split('/').filter(|s| !s.is_empty()) {
        if let Some(inner) = seg.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            id.push_str("By");
            id.push_str(&pascal(inner));
        } else {
            id.push_str(&pascal(seg));
        }
    }
    id
}

fn derive_summary(method: &str, path: &str) -> String {
    let verb = match method {
        "get" => "Retrieve",
        "post" => "Create",
        "put" => "Replace",
        "patch" => "Update",
        "delete" => "Delete",
        "head" => "Check",
        _ => "Describe",
    };
    let resource = literal_segments(path)
        .last()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "resource".into());
    format!("{verb} {resource}")
}

/// Base name for the generated component schemas.
fn component_base(operation_id: &str, path: &str) -> String {
    let from_id = pascal(operation_id.trim());
    if !from_id.is_empty() {
        return from_id;
    }
    let joined: String = literal_segments(path)
        .iter()
        .map(|s| pascal(s))
        .collect::<Vec<_>>()
        .join("");
    if joined.is_empty() {
        "Root".into()
    } else {
        joined
    }
}

/// PascalCase an identifier-ish string, dropping separators.
fn pascal(s: &str) -> String {
    let mut out = String::new();
    let mut upper_next = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            if upper_next {
                out.extend(c.to_uppercase());
                upper_next = false;
            } else {
                out.push(c);
            }
        } else {
            upper_next = true;
        }
    }
    out
}

/// Crude English singularization, used only for nested component names.
fn singular(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    if let Some(stem) = lower.strip_suffix("ies") {
        if !stem.is_empty() {
            return format!("{stem}y");
        }
    }
    for suffix in ["ses", "xes", "zes", "ches", "shes"] {
        if let Some(stem) = lower.strip_suffix(suffix) {
            if !stem.is_empty() {
                return format!("{stem}{}", &suffix[..suffix.len() - 2]);
            }
        }
    }
    if lower.ends_with('s') && !lower.ends_with("ss") && lower.len() > 1 {
        return lower[..lower.len() - 1].to_string();
    }
    s.to_string()
}

/// Standard reason phrase for a status code; falls back to the class name.
fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        410 => "Gone",
        422 => "Unprocessable Content",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        s if s < 200 => "Informational",
        s if s < 300 => "Success",
        s if s < 400 => "Redirection",
        s if s < 500 => "Client Error",
        _ => "Server Error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml(req: &str, resp: &str, o: &Options) -> String {
        generate(req, resp, o).expect("generate")
    }

    #[test]
    fn builds_a_post_stub_with_components() {
        let o = Options {
            path: "/users".into(),
            status: 201,
            ..Default::default()
        };
        let out = yaml(r#"{"name":"Ada","age":36}"#, r#"{"id":7}"#, &o);
        assert!(out.contains("openapi:"));
        assert!(out.contains("3.1.0"));
        assert!(out.contains("/users:"));
        assert!(out.contains("post:"));
        assert!(out.contains("operationId: postUsers"));
        assert!(out.contains("summary: Create users"));
        assert!(out.contains("requestBody:"));
        assert!(out.contains("$ref: '#/components/schemas/UsersRequest'"));
        assert!(out.contains("$ref: '#/components/schemas/UsersResponse'"));
        assert!(out.contains("description: Created"));
        assert!(out.contains("type: integer"));
    }

    #[test]
    fn json_output_inlines_schemas_when_components_off() {
        let o = Options {
            format: "json".into(),
            components: false,
            include_examples: false,
            ..Default::default()
        };
        let out = yaml("", r#"{"ok":true}"#, &o);
        let doc: Value = serde_json::from_str(&out).unwrap();
        assert!(doc.get("components").is_none());
        let schema = &doc["paths"]["/items"]["post"]["responses"]["200"]["content"]
            ["application/json"]["schema"];
        assert_eq!(schema["properties"]["ok"]["type"], json!("boolean"));
        assert_eq!(schema["required"], json!(["ok"]));
        assert!(
            doc["paths"]["/items"]["post"]["responses"]["200"]["content"]["application/json"]
                .get("example")
                .is_none()
        );
    }

    #[test]
    fn array_elements_merge_keys_and_types() {
        let o = Options {
            format: "json".into(),
            components: false,
            ..Default::default()
        };
        let out = yaml("", r#"[{"id":1,"note":"x"},{"id":2.5}]"#, &o);
        let doc: Value = serde_json::from_str(&out).unwrap();
        let items = &doc["paths"]["/items"]["post"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["items"];
        // 1 then 2.5 → number (integer is absorbed), and `note` is optional.
        assert_eq!(items["properties"]["id"]["type"], json!("number"));
        assert_eq!(items["required"], json!(["id"]));
        assert!(items["properties"]["note"].is_object());
    }

    #[test]
    fn detects_string_formats_and_can_be_turned_off() {
        let sample = r#"{"at":"2024-03-01T10:00:00Z","who":"ada@example.com","id":"7f0e5b2c-2b7a-4a1e-9c3d-8f1b2a3c4d5e","home":"https://example.com/x","ip":"10.0.0.7","plain":"hello"}"#;
        let o = Options {
            format: "json".into(),
            components: false,
            ..Default::default()
        };
        let doc: Value = serde_json::from_str(&yaml("", sample, &o)).unwrap();
        let props = &doc["paths"]["/items"]["post"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["properties"];
        assert_eq!(props["at"]["format"], json!("date-time"));
        assert_eq!(props["who"]["format"], json!("email"));
        assert_eq!(props["id"]["format"], json!("uuid"));
        assert_eq!(props["home"]["format"], json!("uri"));
        assert_eq!(props["ip"]["format"], json!("ipv4"));
        assert!(props["plain"].get("format").is_none());

        let off = Options {
            detect_formats: false,
            ..o.clone()
        };
        let doc: Value = serde_json::from_str(&yaml("", sample, &off)).unwrap();
        let props = &doc["paths"]["/items"]["post"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["properties"];
        assert!(props["at"].get("format").is_none());
    }

    #[test]
    fn extracts_nested_objects_into_components() {
        let o = Options {
            format: "json".into(),
            path: "/orders".into(),
            extract_nested: true,
            ..Default::default()
        };
        let doc: Value = serde_json::from_str(&yaml(
            "",
            r#"{"id":1,"customer":{"name":"Ada"},"lines":[{"sku":"A1","qty":2}]}"#,
            &o,
        ))
        .unwrap();
        let schemas = &doc["components"]["schemas"];
        assert_eq!(
            schemas["OrdersResponse"]["properties"]["customer"]["$ref"],
            json!("#/components/schemas/Customer")
        );
        assert_eq!(
            schemas["OrdersResponse"]["properties"]["lines"]["items"]["$ref"],
            json!("#/components/schemas/Line")
        );
        assert_eq!(
            schemas["Customer"]["properties"]["name"]["type"],
            json!("string")
        );
        assert_eq!(
            schemas["Line"]["properties"]["qty"]["type"],
            json!("integer")
        );
    }

    #[test]
    fn path_and_query_parameters_are_emitted() {
        let o = Options {
            method: "get".into(),
            path: "/users/{userId}/orders".into(),
            query: "?limit=10&active=true&sort=name".into(),
            format: "json".into(),
            ..Default::default()
        };
        let doc: Value = serde_json::from_str(&yaml("", r#"[]"#, &o)).unwrap();
        let params = doc["paths"]["/users/{userId}/orders"]["get"]["parameters"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(params.len(), 4);
        assert_eq!(params[0]["name"], json!("userId"));
        assert_eq!(params[0]["in"], json!("path"));
        assert_eq!(params[0]["required"], json!(true));
        assert_eq!(params[1]["schema"]["type"], json!("integer"));
        assert_eq!(params[1]["example"], json!(10));
        assert_eq!(params[2]["schema"]["type"], json!("boolean"));
        assert_eq!(params[3]["schema"]["type"], json!("string"));
        assert_eq!(
            doc["paths"]["/users/{userId}/orders"]["get"]["operationId"],
            json!("getUsersByUserIdOrders")
        );
    }

    #[test]
    fn security_and_error_responses_are_wired_up() {
        let o = Options {
            format: "json".into(),
            security: "bearer".into(),
            include_error_responses: true,
            ..Default::default()
        };
        let doc: Value = serde_json::from_str(&yaml("", r#"{"ok":true}"#, &o)).unwrap();
        let op = &doc["paths"]["/items"]["post"];
        assert_eq!(op["security"], json!([{ "BearerAuth": [] }]));
        assert_eq!(
            doc["components"]["securitySchemes"]["BearerAuth"]["scheme"],
            json!("bearer")
        );
        assert_eq!(op["responses"]["400"]["description"], json!("Bad Request"));
        assert_eq!(
            op["responses"]["500"]["content"]["application/json"]["schema"]["$ref"],
            json!("#/components/schemas/Error")
        );
    }

    #[test]
    fn response_only_204_has_no_content() {
        let o = Options {
            method: "delete".into(),
            path: "/users/{id}".into(),
            status: 204,
            format: "json".into(),
            ..Default::default()
        };
        let doc: Value = serde_json::from_str(&yaml("", "", &o)).unwrap();
        let resp = &doc["paths"]["/users/{id}"]["delete"]["responses"]["204"];
        assert_eq!(resp["description"], json!("No Content"));
        assert!(resp.get("content").is_none());
        assert!(doc.get("components").is_none());
    }

    #[test]
    fn server_title_and_version_are_honored() {
        let o = Options {
            format: "json".into(),
            title: "Orders API".into(),
            api_version: "2.4.0".into(),
            server_url: "https://api.example.com/v2".into(),
            tag: "billing".into(),
            operation_id: "createOrder".into(),
            ..Default::default()
        };
        let doc: Value = serde_json::from_str(&yaml(r#"{"sku":"A1"}"#, "", &o)).unwrap();
        assert_eq!(doc["info"]["title"], json!("Orders API"));
        assert_eq!(doc["info"]["version"], json!("2.4.0"));
        assert_eq!(
            doc["servers"][0]["url"],
            json!("https://api.example.com/v2")
        );
        assert_eq!(doc["paths"]["/items"]["post"]["tags"], json!(["billing"]));
        assert_eq!(
            doc["paths"]["/items"]["post"]["operationId"],
            json!("createOrder")
        );
        // The component base follows the operation id when one is supplied.
        assert!(doc["components"]["schemas"]["CreateOrderRequest"].is_object());
    }

    #[test]
    fn null_and_empty_array_stay_honest() {
        let o = Options {
            format: "json".into(),
            components: false,
            ..Default::default()
        };
        let doc: Value =
            serde_json::from_str(&yaml("", r#"{"deleted_at":null,"tags":[]}"#, &o)).unwrap();
        let props = &doc["paths"]["/items"]["post"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["properties"];
        assert_eq!(props["deleted_at"]["type"], json!("null"));
        assert_eq!(props["tags"]["items"], json!({}));
    }

    #[test]
    fn rejects_blank_input() {
        let err = generate("  ", "", &Options::default()).unwrap_err();
        assert!(err.contains("both were blank"), "{err}");
    }

    #[test]
    fn rejects_invalid_json() {
        let err = generate("not json", "", &Options::default()).unwrap_err();
        assert!(err.contains("request_json is not valid JSON"), "{err}");
        let err = generate("", "{oops}", &Options::default()).unwrap_err();
        assert!(err.contains("response_json is not valid JSON"), "{err}");
    }

    #[test]
    fn rejects_bad_path_status_method_and_format() {
        let o = Options {
            path: "users".into(),
            ..Default::default()
        };
        assert!(generate("", "{}", &o)
            .unwrap_err()
            .contains("must start with"));

        let o = Options {
            status: 99,
            ..Default::default()
        };
        assert!(generate("", "{}", &o)
            .unwrap_err()
            .contains("status must be"));

        let o = Options {
            method: "fetch".into(),
            ..Default::default()
        };
        assert!(generate("", "{}", &o)
            .unwrap_err()
            .contains("method must be one of"));

        let o = Options {
            format: "xml".into(),
            ..Default::default()
        };
        assert!(generate("", "{}", &o)
            .unwrap_err()
            .contains("format must be one of"));

        let o = Options {
            content_type: "  ".into(),
            ..Default::default()
        };
        assert!(generate("", "{}", &o).unwrap_err().contains("content_type"));
    }

    #[test]
    fn rejects_oversized_and_overdeep_samples() {
        let big = "\"".to_string() + &"a".repeat(MAX_INPUT_BYTES) + "\"";
        assert!(generate("", &big, &Options::default())
            .unwrap_err()
            .contains("too large"));

        let deep = "[".repeat(MAX_DEPTH + 2) + &"]".repeat(MAX_DEPTH + 2);
        assert!(generate("", &deep, &Options::default())
            .unwrap_err()
            .contains("nests deeper"));
    }

    #[test]
    fn output_is_deterministic() {
        let o = Options {
            extract_nested: true,
            ..Default::default()
        };
        let sample = r#"{"a":{"b":1},"c":[{"d":"x"},{"d":"y","e":2}]}"#;
        assert_eq!(yaml(sample, sample, &o), yaml(sample, sample, &o));
    }
}
