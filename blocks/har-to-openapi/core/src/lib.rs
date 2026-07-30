//! har-to-openapi core — infer a draft OpenAPI 3.x document from a HAR (HTTP
//! Archive) capture. Pure Rust (serde_json with preserved key order + serde_yml);
//! no I/O, no network, deterministic for a given input.
//!
//! Pipeline: parse the HAR JSON → walk `log.entries` → group requests by
//! (templated path, method) → infer path/query parameters, a request-body schema,
//! and one response schema per status code from the captured JSON bodies → assemble
//! an OpenAPI `paths`/`components`-free document and serialize to YAML or JSON.
//!
//! Scope is a deterministic best-effort draft, not a validator's output: it covers
//! what a HAR reliably reveals (endpoints, methods, params, response shapes). It does
//! NOT redact secrets (run the `har-redact` tool first) or guess auth/security schemes.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// HTTP methods recognized as operations, in canonical OpenAPI emit order.
const METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// Options controlling the generated document. All have sensible defaults so a
/// bare HAR still produces a useful draft.
pub struct Options {
    /// `"yaml"` (default) or `"json"`.
    pub format: String,
    /// OpenAPI version string to stamp — `"3.0.3"` (default) or `"3.1.0"`.
    pub openapi_version: String,
    /// Collapse numeric / UUID / long-hex path segments into `{param}` templates
    /// so `/users/1` and `/users/2` become one `/users/{user}` path.
    pub parameterize_paths: bool,
    /// Infer scalar JSON-Schema types (integer/number/boolean) for query and path
    /// parameters from their observed values; when false every param is `string`.
    pub infer_types: bool,
    /// Include a captured `example` alongside each request/response body schema.
    pub include_examples: bool,
    /// Case-insensitive substring; keep only requests whose host contains it.
    /// Empty = every host.
    pub domain: String,
    /// API title for `info.title`; empty = inferred from the first host.
    pub title: String,
    /// Drop any operation that never returned a 2xx response.
    pub drop_unsuccessful: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: "yaml".into(),
            openapi_version: "3.0.3".into(),
            parameterize_paths: true,
            infer_types: true,
            include_examples: true,
            domain: String::new(),
            title: String::new(),
            drop_unsuccessful: false,
        }
    }
}

/// A collected query/path parameter: its inferred schema type and a sample value.
struct ParamInfo {
    ty: &'static str,
    example: Option<String>,
    is_path: bool,
}

/// A collected response for one status code.
struct RespInfo {
    mime: Option<String>,
    schema: Option<Value>,
    example: Option<Value>,
}

/// One (path, method) operation accumulated across all matching HAR entries.
#[derive(Default)]
struct Op {
    // Parameters keyed by name; emitted sorted by name for deterministic output.
    params: BTreeMap<String, ParamInfo>,
    req_body: Option<(String, Value, Option<Value>)>,
    responses: BTreeMap<String, RespInfo>,
    had_2xx: bool,
    count: usize,
}

/// Thin adapter used by the chat/CLI block and the browser wrapper: builds an
/// [`Options`] from individual typed fields and calls [`generate`]. Keeps the
/// Options-construction logic single-sourced across both call sites.
#[allow(clippy::too_many_arguments)]
pub fn run(
    har: &str,
    format: &str,
    openapi_version: &str,
    parameterize_paths: bool,
    infer_types: bool,
    include_examples: bool,
    domain: &str,
    title: &str,
    drop_unsuccessful: bool,
) -> Result<String, String> {
    generate(
        har,
        &Options {
            format: if format.trim().is_empty() { "yaml".into() } else { format.trim().to_lowercase() },
            openapi_version: if openapi_version.trim().is_empty() {
                "3.0.3".into()
            } else {
                openapi_version.trim().to_string()
            },
            parameterize_paths,
            infer_types,
            include_examples,
            domain: domain.to_string(),
            title: title.to_string(),
            drop_unsuccessful,
        },
    )
}

/// Generate the OpenAPI document. Returns the serialized YAML/JSON string.
pub fn generate(har: &str, opts: &Options) -> Result<String, String> {
    if har.trim().is_empty() {
        return Err("no input: paste a HAR capture (the JSON from DevTools → Network → \"Save all as HAR\")".into());
    }
    let root: Value = serde_json::from_str(har)
        .map_err(|e| format!("invalid HAR: not valid JSON ({e})"))?;
    let entries = root
        .get("log")
        .and_then(|l| l.get("entries"))
        .and_then(Value::as_array)
        .ok_or("not a HAR file: missing log.entries array (expected the { \"log\": { \"entries\": [ … ] } } shape)")?;

    let domain = opts.domain.trim().to_lowercase();
    // path -> (method_rank -> (method, Op))
    let mut paths: BTreeMap<String, BTreeMap<usize, (String, Op)>> = BTreeMap::new();
    let mut servers: Vec<String> = Vec::new();
    let mut first_host: Option<String> = None;
    let mut matched = 0usize;

    for entry in entries {
        let req = match entry.get("request") {
            Some(r) => r,
            None => continue,
        };
        let method = req
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("GET")
            .to_lowercase();
        let rank = match METHODS.iter().position(|m| *m == method) {
            Some(i) => i,
            None => continue, // skip non-standard verbs (CONNECT, custom, …)
        };
        let url = match req.get("url").and_then(Value::as_str) {
            Some(u) => u,
            None => continue,
        };
        let parsed = match parse_url(url) {
            Some(p) => p,
            None => continue,
        };
        if !domain.is_empty() && !parsed.host.to_lowercase().contains(&domain) {
            continue;
        }
        matched += 1;
        if first_host.is_none() {
            first_host = Some(parsed.host.clone());
        }
        let origin = format!("{}://{}", parsed.scheme, parsed.host);
        if !servers.contains(&origin) {
            servers.push(origin);
        }

        // Template the path + collect path params from the replaced segments.
        let (tmpl_path, path_params) = if opts.parameterize_paths {
            template_path(&parsed.path)
        } else {
            (parsed.path.clone(), Vec::new())
        };

        let op_slot = paths
            .entry(tmpl_path)
            .or_default()
            .entry(rank)
            .or_insert_with(|| (method.clone(), Op::default()));
        let op = &mut op_slot.1;
        op.count += 1;

        // Path parameters.
        for (name, raw) in &path_params {
            let ty = if opts.infer_types { infer_scalar(raw) } else { "string" };
            op.params
                .entry(name.clone())
                .or_insert_with(|| ParamInfo {
                    ty,
                    example: Some(raw.clone()),
                    is_path: true,
                });
        }
        // Query parameters (from the parsed URL + HAR's queryString array).
        for (name, val) in collect_query(req, &parsed.query) {
            if name.is_empty() {
                continue;
            }
            let ty = if opts.infer_types { infer_scalar(&val) } else { "string" };
            op.params.entry(name).or_insert_with(|| ParamInfo {
                ty,
                example: Some(val.clone()),
                is_path: false,
            });
        }

        // Request body schema (JSON post data only).
        if op.req_body.is_none() {
            if let Some((mime, text)) = post_data(req) {
                if is_json_mime(&mime) {
                    if let Ok(v) = serde_json::from_str::<Value>(&text) {
                        let schema = infer_schema(&v);
                        let example = if opts.include_examples { Some(v) } else { None };
                        op.req_body = Some((json_mime(&mime), schema, example));
                    }
                }
            }
        }

        // Response.
        if let Some(resp) = entry.get("response") {
            let status = resp.get("status").and_then(Value::as_i64).unwrap_or(0);
            if (200..300).contains(&status) {
                op.had_2xx = true;
            }
            if status >= 100 {
                let code = status.to_string();
                if !op.responses.contains_key(&code) {
                    op.responses.insert(code, build_response(resp, opts));
                }
            }
        }
    }

    if matched == 0 {
        return Err(if domain.is_empty() {
            "the HAR contained no usable HTTP(S) requests".into()
        } else {
            format!("no requests matched the domain filter \"{}\"", opts.domain.trim())
        });
    }

    // Assemble the document (serde_json preserves insertion order).
    let mut doc = Map::new();
    doc.insert("openapi".into(), Value::String(normalize_version(&opts.openapi_version)));

    let title = if !opts.title.trim().is_empty() {
        opts.title.trim().to_string()
    } else {
        format!("{} API", first_host.as_deref().unwrap_or("Captured"))
    };
    let mut info = Map::new();
    info.insert("title".into(), Value::String(title));
    info.insert("version".into(), Value::String("1.0.0".into()));
    info.insert(
        "description".into(),
        Value::String(format!(
            "Draft OpenAPI spec inferred from {matched} request(s) in a HAR capture. Review before use."
        )),
    );
    doc.insert("info".into(), Value::Object(info));

    if !servers.is_empty() {
        doc.insert(
            "servers".into(),
            Value::Array(
                servers
                    .into_iter()
                    .map(|u| json!({ "url": u }))
                    .collect(),
            ),
        );
    }

    let mut paths_out = Map::new();
    for (path, ops) in &paths {
        let mut item = Map::new();
        for (_, (method, op)) in ops {
            if opts.drop_unsuccessful && !op.had_2xx {
                continue;
            }
            item.insert(method.clone(), build_operation(op, opts));
        }
        if !item.is_empty() {
            paths_out.insert(path.clone(), Value::Object(item));
        }
    }
    if paths_out.is_empty() {
        return Err(
            "every captured operation was dropped (no 2xx responses); turn off \"drop operations without a 2xx response\"".into(),
        );
    }
    doc.insert("paths".into(), Value::Object(paths_out));

    let value = Value::Object(doc);
    match opts.format.trim().to_lowercase().as_str() {
        "json" => serde_json::to_string_pretty(&value).map_err(|e| format!("JSON encode error: {e}")),
        "" | "yaml" | "yml" => {
            serde_yml::to_string(&value).map_err(|e| format!("YAML encode error: {e}"))
        }
        other => Err(format!("unknown format \"{other}\" (use \"yaml\" or \"json\")")),
    }
}

/// Build one operation object (parameters, requestBody, responses).
fn build_operation(op: &Op, opts: &Options) -> Value {
    let mut out = Map::new();

    // Parameters, sorted by name for stable output.
    let mut params: Vec<Value> = Vec::new();
    for (name, info) in &op.params {
        let mut p = Map::new();
        p.insert("name".into(), Value::String(name.clone()));
        p.insert("in".into(), Value::String(if info.is_path { "path" } else { "query" }.into()));
        p.insert("required".into(), Value::Bool(info.is_path));
        p.insert("schema".into(), json!({ "type": info.ty }));
        if opts.include_examples {
            if let Some(ex) = &info.example {
                p.insert("example".into(), typed_example(info.ty, ex));
            }
        }
        params.push(Value::Object(p));
    }
    if !params.is_empty() {
        out.insert("parameters".into(), Value::Array(params));
    }

    // Request body.
    if let Some((mime, schema, example)) = &op.req_body {
        let mut media = Map::new();
        media.insert("schema".into(), schema.clone());
        if let Some(ex) = example {
            media.insert("example".into(), ex.clone());
        }
        out.insert(
            "requestBody".into(),
            json!({ "content": { mime.clone(): Value::Object(media) } }),
        );
    }

    // Responses (must be non-empty per the spec).
    let mut responses = Map::new();
    if op.responses.is_empty() {
        responses.insert(
            "default".into(),
            json!({ "description": "No response body captured" }),
        );
    } else {
        for (code, info) in &op.responses {
            let mut r = Map::new();
            r.insert("description".into(), Value::String(describe_status(code)));
            if let Some(mime) = &info.mime {
                let mut media = Map::new();
                media.insert(
                    "schema".into(),
                    info.schema.clone().unwrap_or_else(|| json!({ "type": "string" })),
                );
                if opts.include_examples {
                    if let Some(ex) = &info.example {
                        media.insert("example".into(), ex.clone());
                    }
                }
                r.insert("content".into(), json!({ mime.clone(): Value::Object(media) }));
            }
            responses.insert(code.clone(), Value::Object(r));
        }
    }
    out.insert("responses".into(), Value::Object(responses));

    Value::Object(out)
}

/// Build a RespInfo from a HAR `response` object.
fn build_response(resp: &Value, opts: &Options) -> RespInfo {
    let content = resp.get("content");
    let mime = content
        .and_then(|c| c.get("mimeType"))
        .and_then(Value::as_str)
        .map(strip_charset)
        .filter(|m| !m.is_empty());
    let text = content
        .and_then(|c| c.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let (schema, example) = match (&mime, text) {
        (Some(m), t) if is_json_mime(m) && !t.trim().is_empty() => {
            match serde_json::from_str::<Value>(t) {
                Ok(v) => {
                    let sch = infer_schema(&v);
                    let ex = if opts.include_examples { Some(v) } else { None };
                    (Some(sch), ex)
                }
                Err(_) => (Some(json!({ "type": "string" })), None),
            }
        }
        (Some(_), _) => (Some(json!({ "type": "string" })), None),
        (None, _) => (None, None),
    };
    RespInfo {
        mime: mime.map(|m| json_mime(&m)),
        schema,
        example,
    }
}

/// Recursively infer a JSON-Schema fragment from a concrete JSON value.
fn infer_schema(v: &Value) -> Value {
    match v {
        Value::Null => json!({ "nullable": true }),
        Value::Bool(_) => json!({ "type": "boolean" }),
        Value::Number(n) => {
            if n.is_f64() && n.as_f64().map(|f| f.fract() != 0.0).unwrap_or(false) {
                json!({ "type": "number" })
            } else {
                json!({ "type": "integer" })
            }
        }
        Value::String(_) => json!({ "type": "string" }),
        Value::Array(items) => {
            let inner = items.first().map(infer_schema).unwrap_or_else(|| json!({}));
            json!({ "type": "array", "items": inner })
        }
        Value::Object(map) => {
            let mut props = Map::new();
            for (k, val) in map {
                props.insert(k.clone(), infer_schema(val));
            }
            json!({ "type": "object", "properties": Value::Object(props) })
        }
    }
}

/// Guess a scalar JSON-Schema type name from a raw string value.
fn infer_scalar(raw: &str) -> &'static str {
    let t = raw.trim();
    if t.is_empty() {
        return "string";
    }
    if t == "true" || t == "false" {
        return "boolean";
    }
    if t.parse::<i64>().is_ok() {
        return "integer";
    }
    if t.parse::<f64>().is_ok() {
        return "number";
    }
    "string"
}

/// Render an example value in the parameter's inferred type.
fn typed_example(ty: &str, raw: &str) -> Value {
    let t = raw.trim();
    match ty {
        "integer" => t.parse::<i64>().map(|n| json!(n)).unwrap_or_else(|_| json!(raw)),
        "number" => t.parse::<f64>().map(|n| json!(n)).unwrap_or_else(|_| json!(raw)),
        "boolean" => match t {
            "true" => json!(true),
            "false" => json!(false),
            _ => json!(raw),
        },
        _ => json!(raw),
    }
}

/// A minimally-parsed absolute URL.
struct ParsedUrl {
    scheme: String,
    host: String,
    path: String,
    query: String,
}

/// Parse `scheme://host[:port]/path?query#frag`; None for non-absolute / non-http.
fn parse_url(url: &str) -> Option<ParsedUrl> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    // host ends at the first '/', '?' or '#'.
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = &rest[..end];
    if host.is_empty() {
        return None;
    }
    let after = &rest[end..];
    let after = after.split('#').next().unwrap_or(after); // drop fragment
    let (path, query) = match after.split_once('?') {
        Some((p, q)) => (p, q),
        None => (after, ""),
    };
    let path = if path.is_empty() { "/" } else { path };
    Some(ParsedUrl {
        scheme: scheme.to_string(),
        host: host.to_string(),
        path: path.to_string(),
        query: query.to_string(),
    })
}

/// Replace id-like path segments with `{name}` templates; returns the templated
/// path plus the (param_name, original_value) pairs for the replaced segments.
fn template_path(path: &str) -> (String, Vec<(String, String)>) {
    let mut params: Vec<(String, String)> = Vec::new();
    let mut used: BTreeMap<String, u32> = BTreeMap::new();
    let segs: Vec<&str> = path.split('/').collect();
    let mut out: Vec<String> = Vec::with_capacity(segs.len());
    for (i, seg) in segs.iter().enumerate() {
        if seg.is_empty() || !is_id_like(seg) {
            out.push((*seg).to_string());
            continue;
        }
        // Name after the preceding static segment (singularized), else "param".
        let base = segs
            .get(i.wrapping_sub(1))
            .filter(|_| i > 0)
            .map(|s| singularize(s))
            .filter(|s| !s.is_empty() && !is_id_like(s))
            .unwrap_or_else(|| "param".to_string());
        let name = match used.get_mut(&base) {
            Some(n) => {
                *n += 1;
                format!("{base}{n}")
            }
            None => {
                used.insert(base.clone(), 0);
                base
            }
        };
        params.push((name.clone(), (*seg).to_string()));
        out.push(format!("{{{name}}}"));
    }
    (out.join("/"), params)
}

/// A segment looks like an id: all digits, a UUID, or a long hex/ULID token.
fn is_id_like(seg: &str) -> bool {
    if seg.len() >= 2 && seg.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // UUID 8-4-4-4-12
    let hyphens = seg.matches('-').count();
    if seg.len() == 36 && hyphens == 4 && seg.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return true;
    }
    // Long opaque token (24+ chars, alphanumeric, contains a digit): ULID / hash.
    if seg.len() >= 24
        && seg.chars().all(|c| c.is_ascii_alphanumeric())
        && seg.chars().any(|c| c.is_ascii_digit())
    {
        return true;
    }
    false
}

/// Crude singularization for path-param names (users → user, entries → entry).
fn singularize(seg: &str) -> String {
    let s: String = seg.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if s.len() > 3 && s.ends_with("ies") {
        format!("{}y", &s[..s.len() - 3])
    } else if s.len() > 1 && s.ends_with('s') && !s.ends_with("ss") {
        s[..s.len() - 1].to_string()
    } else {
        s
    }
}

/// Collect query parameters from the parsed URL query string and the HAR
/// `request.queryString` array (union; first-seen name wins).
fn collect_query(req: &Value, query: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    for pair in query.split('&').filter(|s| !s.is_empty()) {
        let (name, val) = match pair.split_once('=') {
            Some((n, v)) => (percent_decode(n), percent_decode(v)),
            None => (percent_decode(pair), String::new()),
        };
        if seen.insert(name.clone(), ()).is_none() {
            out.push((name, val));
        }
    }
    if let Some(arr) = req.get("queryString").and_then(Value::as_array) {
        for q in arr {
            let name = q.get("name").and_then(Value::as_str).unwrap_or("");
            let val = q.get("value").and_then(Value::as_str).unwrap_or("");
            if !name.is_empty() && seen.insert(name.to_string(), ()).is_none() {
                out.push((name.to_string(), val.to_string()));
            }
        }
    }
    out
}

/// Extract `(mimeType, text)` from `request.postData` if present.
fn post_data(req: &Value) -> Option<(String, String)> {
    let pd = req.get("postData")?;
    let mime = pd.get("mimeType").and_then(Value::as_str).unwrap_or("");
    let text = pd.get("text").and_then(Value::as_str)?;
    if text.trim().is_empty() {
        return None;
    }
    Some((strip_charset(mime), text.to_string()))
}

fn strip_charset(mime: &str) -> String {
    mime.split(';').next().unwrap_or(mime).trim().to_lowercase()
}

fn is_json_mime(mime: &str) -> bool {
    let m = mime.to_lowercase();
    m == "application/json" || m.ends_with("+json") || m.contains("json")
}

/// Normalize a JSON-ish mime to a canonical content-type key.
fn json_mime(mime: &str) -> String {
    if mime.is_empty() {
        "application/json".into()
    } else {
        mime.to_string()
    }
}

fn normalize_version(v: &str) -> String {
    match v.trim() {
        "3.1" | "3.1.0" => "3.1.0".into(),
        "" | "3.0" | "3.0.3" => "3.0.3".into(),
        other => other.to_string(),
    }
}

/// Minimal percent-decoder for query keys/values (%XX + '+' → space).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                if let (Some(h), Some(l)) = (hi, lo) {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn describe_status(code: &str) -> String {
    match code {
        "200" => "OK",
        "201" => "Created",
        "202" => "Accepted",
        "204" => "No Content",
        "301" => "Moved Permanently",
        "302" => "Found",
        "304" => "Not Modified",
        "400" => "Bad Request",
        "401" => "Unauthorized",
        "403" => "Forbidden",
        "404" => "Not Found",
        "409" => "Conflict",
        "422" => "Unprocessable Entity",
        "429" => "Too Many Requests",
        "500" => "Internal Server Error",
        "502" => "Bad Gateway",
        "503" => "Service Unavailable",
        _ => "Response",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_har() -> &'static str {
        r#"{
          "log": {
            "entries": [
              {
                "request": {
                  "method": "GET",
                  "url": "https://api.example.com/users/42?active=true&page=2",
                  "queryString": [
                    {"name": "active", "value": "true"},
                    {"name": "page", "value": "2"}
                  ]
                },
                "response": {
                  "status": 200,
                  "content": {
                    "mimeType": "application/json; charset=utf-8",
                    "text": "{\"id\":42,\"name\":\"Ada\",\"admin\":false}"
                  }
                }
              },
              {
                "request": {
                  "method": "POST",
                  "url": "https://api.example.com/users",
                  "postData": {
                    "mimeType": "application/json",
                    "text": "{\"name\":\"Grace\",\"age\":30}"
                  }
                },
                "response": { "status": 201, "content": { "mimeType": "application/json", "text": "{\"id\":43}" } }
              },
              {
                "request": { "method": "GET", "url": "https://api.example.com/users/99" },
                "response": { "status": 404, "content": { "mimeType": "text/plain", "text": "nope" } }
              }
            ]
          }
        }"#
    }

    #[test]
    fn happy_yaml_infers_paths_params_and_schemas() {
        let out = generate(sample_har(), &Options::default()).unwrap();
        // path templated + grouped: /users and /users/{user}
        assert!(out.contains("/users/{user}:"), "templated path missing:\n{out}");
        assert!(out.contains("/users:"), "collection path missing:\n{out}");
        // both GET and POST present
        assert!(out.contains("get:"));
        assert!(out.contains("post:"));
        // path param typed as integer (42/99 are numeric) + query params
        assert!(out.contains("name: user"));
        assert!(out.contains("in: path"));
        assert!(out.contains("name: active"));
        assert!(out.contains("type: boolean"));
        // request body + response schema inferred
        assert!(out.contains("requestBody"));
        assert!(out.contains("openapi: '3.0.3'"));
        assert!(out.contains("servers:"));
    }

    #[test]
    fn json_format_and_version_and_no_examples() {
        let opts = Options {
            format: "json".into(),
            openapi_version: "3.1.0".into(),
            include_examples: false,
            ..Options::default()
        };
        let out = generate(sample_har(), &opts).unwrap();
        let v: Value = serde_json::from_str(&out).expect("valid JSON output");
        assert_eq!(v["openapi"], "3.1.0");
        // With examples off, no "example" key should appear anywhere.
        assert!(!out.contains("\"example\""), "examples leaked:\n{out}");
        assert!(v["paths"]["/users/{user}"]["get"].is_object());
    }

    #[test]
    fn parameterize_off_keeps_literal_paths() {
        let opts = Options { parameterize_paths: false, ..Options::default() };
        let out = generate(sample_har(), &opts).unwrap();
        assert!(out.contains("/users/42:"), "literal path missing:\n{out}");
        assert!(!out.contains("{user}"));
    }

    #[test]
    fn domain_filter_and_drop_unsuccessful() {
        let opts = Options { domain: "example.com".into(), drop_unsuccessful: true, ..Options::default() };
        let out = generate(sample_har(), &opts).unwrap();
        // POST /users (201) stays; /users/{user} GET had a 200 in entry 1 so it survives.
        assert!(out.contains("post:"));
        assert!(out.contains("/users/{user}:"));
    }

    #[test]
    fn err_on_non_json() {
        let e = generate("not json at all", &Options::default()).unwrap_err();
        assert!(e.contains("not valid JSON"), "got: {e}");
    }

    #[test]
    fn err_on_missing_entries() {
        let e = generate(r#"{"log":{}}"#, &Options::default()).unwrap_err();
        assert!(e.contains("log.entries"), "got: {e}");
    }

    #[test]
    fn err_on_empty() {
        assert!(generate("   ", &Options::default()).unwrap_err().contains("no input"));
    }

    #[test]
    fn err_on_domain_no_match() {
        let opts = Options { domain: "nowhere.test".into(), ..Options::default() };
        let e = generate(sample_har(), &opts).unwrap_err();
        assert!(e.contains("no requests matched"), "got: {e}");
    }

    #[test]
    fn uuid_segment_parameterized() {
        let har = r#"{"log":{"entries":[
          {"request":{"method":"GET","url":"https://x.io/orders/550e8400-e29b-41d4-a716-446655440000"},
           "response":{"status":200,"content":{"mimeType":"application/json","text":"{}"}}}
        ]}}"#;
        let out = generate(har, &Options::default()).unwrap();
        assert!(out.contains("/orders/{order}:"), "uuid not templated:\n{out}");
    }
}
