//! postman-collection-converter core — convert a Postman Collection v2.x export
//! or an Insomnia JSON export (format 4) into ready-to-run code: curl commands,
//! JavaScript `fetch()` calls, or axios calls. Pure compute, shared by the chat
//! skill block, the CLI, and the web page. No wafer/wasm-bindgen deps.
//!
//! Covered per request: method, URL, headers (disabled ones skipped), body
//! modes (raw/JSON, x-www-form-urlencoded, multipart form-data, file, GraphQL),
//! and auth (basic, bearer, API key in header or query — request-level, or
//! collection-level inherited). `{{variable}}` placeholders are filled from
//! collection variables / Insomnia environments plus an optional user-supplied
//! variables input; unresolved placeholders are left verbatim.

use serde_json::Value;
use std::collections::HashMap;

/// Hard cap on the number of requests converted in one call.
pub const MAX_REQUESTS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Target {
    Curl,
    Fetch,
    Axios,
}

#[derive(Debug, Clone)]
struct FormField {
    key: String,
    value: String, // file path when `is_file`
    is_file: bool,
}

#[derive(Debug, Clone)]
enum Body {
    None,
    /// Raw text body; `json` = emit as JSON (adds Content-Type when missing).
    Raw {
        text: String,
        json: bool,
    },
    UrlEncoded(Vec<(String, String)>),
    Form(Vec<FormField>),
    File {
        src: String,
    },
}

#[derive(Debug, Clone)]
enum Auth {
    Basic { user: String, pass: String },
    Bearer(String),
    ApiKeyHeader { key: String, value: String },
    ApiKeyQuery { key: String, value: String },
}

#[derive(Debug, Clone)]
struct Req {
    label: String,
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Body,
    auth: Option<Auth>,
}

/// Convert a collection export to code. `target` = curl|fetch|axios (default
/// curl when empty); `variables` = optional Postman environment export JSON, a
/// plain JSON object, or KEY=VALUE lines; `multiline` formats curl with
/// backslash continuations (ignored for fetch/axios).
pub fn convert(
    collection: &str,
    target: &str,
    variables: &str,
    multiline: bool,
) -> Result<String, String> {
    let target = parse_target(target)?;
    if collection.trim().is_empty() {
        return Err("collection is empty — paste the exported collection JSON".into());
    }
    let user_vars = parse_variables(variables)?;
    let root: Value = serde_json::from_str(collection.trim())
        .map_err(|e| format!("collection is not valid JSON: {e}"))?;

    let (mut reqs, mut vars) = if root.get("info").is_some() && root.get("item").is_some() {
        extract_postman(&root)?
    } else if root.get("resources").and_then(Value::as_array).is_some() {
        extract_insomnia(&root)?
    } else {
        return Err(
            "unrecognized collection format: expected a Postman Collection v2.x export \
             (top-level `info` + `item`) or an Insomnia JSON export (format 4, top-level `resources`)"
                .into(),
        );
    };

    if reqs.is_empty() {
        return Err("no requests found in the collection".into());
    }
    if reqs.len() > MAX_REQUESTS {
        return Err(format!(
            "collection contains {} requests; the limit is {MAX_REQUESTS}",
            reqs.len()
        ));
    }

    // User-supplied variables override collection/environment values.
    for (k, v) in user_vars {
        vars.insert(k, v);
    }
    for r in &mut reqs {
        apply_vars(r, &vars);
    }

    let snippets: Vec<String> = reqs
        .iter()
        .map(|r| match target {
            Target::Curl => gen_curl(r, multiline),
            Target::Fetch => gen_fetch(r),
            Target::Axios => gen_axios(r),
        })
        .collect();
    Ok(snippets.join("\n\n"))
}

fn parse_target(s: &str) -> Result<Target, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "curl" => Ok(Target::Curl),
        "fetch" => Ok(Target::Fetch),
        "axios" => Ok(Target::Axios),
        other => Err(format!(
            "unknown target '{other}' (expected curl, fetch, or axios)"
        )),
    }
}

// ---------------------------------------------------------------- variables

/// Parse the optional variables input: a Postman environment export
/// (`{"values":[{"key":..,"value":..,"enabled":..}]}`), a plain JSON object,
/// or KEY=VALUE lines (# comments and blank lines skipped).
fn parse_variables(input: &str) -> Result<HashMap<String, String>, String> {
    let t = input.trim();
    let mut map = HashMap::new();
    if t.is_empty() {
        return Ok(map);
    }
    if t.starts_with('{') {
        let v: Value =
            serde_json::from_str(t).map_err(|e| format!("variables is not valid JSON: {e}"))?;
        let obj = v
            .as_object()
            .ok_or_else(|| "variables JSON must be an object".to_string())?;
        if let Some(values) = obj.get("values").and_then(Value::as_array) {
            // Postman environment export.
            for entry in values {
                if entry.get("enabled").and_then(Value::as_bool) == Some(false) {
                    continue;
                }
                if let Some(k) = entry.get("key").and_then(Value::as_str) {
                    map.insert(
                        k.to_string(),
                        value_to_string(entry.get("value").unwrap_or(&Value::Null)),
                    );
                }
            }
        } else {
            for (k, val) in obj {
                map.insert(k.clone(), value_to_string(val));
            }
        }
        return Ok(map);
    }
    for (i, line) in t.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line
            .split_once('=')
            .ok_or_else(|| format!("variables line {} is not KEY=VALUE: {line}", i + 1))?;
        map.insert(k.trim().to_string(), v.trim().to_string());
    }
    Ok(map)
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Replace `{{name}}` / `{{ name }}` / Insomnia's `{{ _.name }}` placeholders.
/// Unknown placeholders are left verbatim.
fn substitute(s: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        let Some(end_rel) = rest[start + 2..].find("}}") else {
            break;
        };
        let inner = &rest[start + 2..start + 2 + end_rel];
        let key = inner.trim().trim_start_matches("_.").trim();
        out.push_str(&rest[..start]);
        match vars.get(key) {
            Some(v) => out.push_str(v),
            None => out.push_str(&rest[start..start + 2 + end_rel + 2]),
        }
        rest = &rest[start + 2 + end_rel + 2..];
    }
    out.push_str(rest);
    out
}

fn apply_vars(req: &mut Req, vars: &HashMap<String, String>) {
    req.url = substitute(&req.url, vars);
    for (k, v) in &mut req.headers {
        *k = substitute(k, vars);
        *v = substitute(v, vars);
    }
    match &mut req.body {
        Body::Raw { text, .. } => *text = substitute(text, vars),
        Body::UrlEncoded(pairs) => {
            for (k, v) in pairs {
                *k = substitute(k, vars);
                *v = substitute(v, vars);
            }
        }
        Body::Form(fields) => {
            for f in fields {
                f.key = substitute(&f.key, vars);
                f.value = substitute(&f.value, vars);
            }
        }
        Body::File { src } => *src = substitute(src, vars),
        Body::None => {}
    }
    if let Some(auth) = &mut req.auth {
        match auth {
            Auth::Basic { user, pass } => {
                *user = substitute(user, vars);
                *pass = substitute(pass, vars);
            }
            Auth::Bearer(t) => *t = substitute(t, vars),
            Auth::ApiKeyHeader { key, value } | Auth::ApiKeyQuery { key, value } => {
                *key = substitute(key, vars);
                *value = substitute(value, vars);
            }
        }
    }
}

// ------------------------------------------------------- shared parse helpers

fn is_disabled(v: &Value) -> bool {
    v.get("disabled").and_then(Value::as_bool) == Some(true)
}

/// Key/value pairs from an array of `{key|name, value, disabled?}` objects.
fn kv_pairs(v: Option<&Value>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(arr) = v.and_then(Value::as_array) {
        for e in arr {
            if is_disabled(e) {
                continue;
            }
            let k = e
                .get("key")
                .or_else(|| e.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if k.is_empty() {
                continue;
            }
            out.push((
                k.to_string(),
                value_to_string(e.get("value").unwrap_or(&Value::Null)),
            ));
        }
    }
    out
}

fn looks_like_json(s: &str) -> bool {
    let t = s.trim();
    (t.starts_with('{') || t.starts_with('[')) && serde_json::from_str::<Value>(t).is_ok()
}

fn make_label(path: &[String], name: &str, method: &str, url: &str) -> String {
    let name = if name.trim().is_empty() {
        format!("{method} {url}")
    } else {
        name.trim().to_string()
    };
    let label = if path.is_empty() {
        name
    } else {
        format!("{} / {}", path.join(" / "), name)
    };
    // Comments are single-line — never let a stray newline break the code.
    label.replace(['\n', '\r'], " ")
}

// ---------------------------------------------------------------- Postman

fn extract_postman(root: &Value) -> Result<(Vec<Req>, HashMap<String, String>), String> {
    let mut vars = HashMap::new();
    if let Some(arr) = root.get("variable").and_then(Value::as_array) {
        for v in arr {
            if is_disabled(v) {
                continue;
            }
            if let Some(k) = v.get("key").and_then(Value::as_str) {
                vars.insert(
                    k.to_string(),
                    value_to_string(v.get("value").unwrap_or(&Value::Null)),
                );
            }
        }
    }
    let coll_auth = root.get("auth");
    let items = root
        .get("item")
        .and_then(Value::as_array)
        .ok_or_else(|| "Postman collection `item` must be an array".to_string())?;
    let mut reqs = Vec::new();
    let mut path = Vec::new();
    for it in items {
        walk_postman_item(it, &mut path, coll_auth, &mut reqs);
    }
    Ok((reqs, vars))
}

fn walk_postman_item(
    item: &Value,
    path: &mut Vec<String>,
    coll_auth: Option<&Value>,
    out: &mut Vec<Req>,
) {
    if let Some(children) = item.get("item").and_then(Value::as_array) {
        // Folder.
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let named = !name.trim().is_empty();
        if named {
            path.push(name);
        }
        for c in children {
            walk_postman_item(c, path, coll_auth, out);
        }
        if named {
            path.pop();
        }
        return;
    }
    let Some(request) = item.get("request") else {
        return;
    };
    let name = item.get("name").and_then(Value::as_str).unwrap_or("");
    // A request may be a bare URL string.
    if let Some(url) = request.as_str() {
        let label = make_label(path, name, "GET", url);
        out.push(Req {
            label,
            method: "GET".into(),
            url: url.to_string(),
            headers: Vec::new(),
            body: Body::None,
            auth: None,
        });
        return;
    }
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .to_ascii_uppercase();
    let url = postman_url(request.get("url"));
    let headers = kv_pairs(request.get("header"));
    let body = parse_postman_body(request.get("body"));
    let auth = request
        .get("auth")
        .or(coll_auth)
        .and_then(parse_postman_auth);
    let label = make_label(path, name, &method, &url);
    out.push(Req {
        label,
        method,
        url,
        headers,
        body,
        auth,
    });
}

fn postman_url(u: Option<&Value>) -> String {
    let Some(u) = u else {
        return String::new();
    };
    if let Some(s) = u.as_str() {
        return s.to_string();
    }
    if let Some(raw) = u.get("raw").and_then(Value::as_str) {
        if !raw.is_empty() {
            return raw.to_string();
        }
    }
    // Reassemble from parts.
    let mut s = String::new();
    if let Some(p) = u.get("protocol").and_then(Value::as_str) {
        s.push_str(p);
        s.push_str("://");
    }
    match u.get("host") {
        Some(Value::Array(a)) => s.push_str(
            &a.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("."),
        ),
        Some(Value::String(h)) => s.push_str(h),
        _ => {}
    }
    if let Some(port) = u.get("port").and_then(Value::as_str) {
        s.push(':');
        s.push_str(port);
    }
    match u.get("path") {
        Some(Value::Array(a)) => {
            for seg in a.iter().filter_map(Value::as_str) {
                s.push('/');
                s.push_str(seg);
            }
        }
        Some(Value::String(p)) => {
            if !p.starts_with('/') {
                s.push('/');
            }
            s.push_str(p);
        }
        _ => {}
    }
    let q = kv_pairs(u.get("query"));
    if !q.is_empty() {
        s.push('?');
        s.push_str(
            &q.iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("&"),
        );
    }
    s
}

fn parse_postman_body(b: Option<&Value>) -> Body {
    let Some(b) = b else {
        return Body::None;
    };
    match b.get("mode").and_then(Value::as_str) {
        Some("raw") => {
            let text = b
                .get("raw")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if text.trim().is_empty() {
                return Body::None;
            }
            let lang = b
                .get("options")
                .and_then(|o| o.get("raw"))
                .and_then(|r| r.get("language"))
                .and_then(Value::as_str);
            let json = lang == Some("json") || (lang.is_none() && looks_like_json(&text));
            Body::Raw { text, json }
        }
        Some("urlencoded") => Body::UrlEncoded(kv_pairs(b.get("urlencoded"))),
        Some("formdata") => {
            let mut fields = Vec::new();
            if let Some(arr) = b.get("formdata").and_then(Value::as_array) {
                for f in arr {
                    if is_disabled(f) {
                        continue;
                    }
                    let key = f
                        .get("key")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let is_file = f.get("type").and_then(Value::as_str) == Some("file");
                    let value = if is_file {
                        value_to_string(f.get("src").unwrap_or(&Value::Null))
                    } else {
                        value_to_string(f.get("value").unwrap_or(&Value::Null))
                    };
                    fields.push(FormField {
                        key,
                        value,
                        is_file,
                    });
                }
            }
            Body::Form(fields)
        }
        Some("file") => Body::File {
            src: b
                .get("file")
                .and_then(|f| f.get("src"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        },
        Some("graphql") => {
            let g = b.get("graphql");
            let query = g
                .and_then(|g| g.get("query"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let vars_raw = g
                .and_then(|g| g.get("variables"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let vars_val: Value =
                serde_json::from_str(vars_raw).unwrap_or(Value::Object(serde_json::Map::new()));
            let payload = serde_json::json!({ "query": query, "variables": vars_val });
            Body::Raw {
                text: serde_json::to_string(&payload).unwrap_or_default(),
                json: true,
            }
        }
        _ => Body::None,
    }
}

/// Auth params come as an array of `{key, value}` (schema v2.1) or a plain
/// object (v2.0) — handle both.
fn postman_auth_param(section: &Value, key: &str) -> Option<String> {
    match section {
        Value::Array(a) => a
            .iter()
            .find(|e| e.get("key").and_then(Value::as_str) == Some(key))
            .map(|e| value_to_string(e.get("value").unwrap_or(&Value::Null))),
        Value::Object(o) => o.get(key).map(value_to_string),
        _ => None,
    }
}

fn parse_postman_auth(auth: &Value) -> Option<Auth> {
    let t = auth.get("type").and_then(Value::as_str)?;
    let get = |key: &str| -> String {
        auth.get(t)
            .and_then(|s| postman_auth_param(s, key))
            .unwrap_or_default()
    };
    match t {
        "basic" => Some(Auth::Basic {
            user: get("username"),
            pass: get("password"),
        }),
        "bearer" => Some(Auth::Bearer(get("token"))),
        "apikey" => {
            let key = get("key");
            let value = get("value");
            if get("in") == "query" {
                Some(Auth::ApiKeyQuery { key, value })
            } else {
                Some(Auth::ApiKeyHeader { key, value })
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------- Insomnia

fn extract_insomnia(root: &Value) -> Result<(Vec<Req>, HashMap<String, String>), String> {
    let resources = root
        .get("resources")
        .and_then(Value::as_array)
        .ok_or_else(|| "Insomnia export `resources` must be an array".to_string())?;

    // Folder (request group) tree.
    let mut groups: HashMap<&str, (&str, Option<&str>)> = HashMap::new();
    for r in resources {
        if r.get("_type").and_then(Value::as_str) == Some("request_group") {
            if let Some(id) = r.get("_id").and_then(Value::as_str) {
                groups.insert(
                    id,
                    (
                        r.get("name").and_then(Value::as_str).unwrap_or(""),
                        r.get("parentId").and_then(Value::as_str),
                    ),
                );
            }
        }
    }

    // Environments: base first, sub-environments override (export order).
    let mut vars = HashMap::new();
    for r in resources {
        if r.get("_type").and_then(Value::as_str) == Some("environment") {
            if let Some(data) = r.get("data").and_then(Value::as_object) {
                for (k, v) in data {
                    vars.insert(k.clone(), value_to_string(v));
                }
            }
        }
    }

    let mut reqs = Vec::new();
    for r in resources {
        if r.get("_type").and_then(Value::as_str) != Some("request") {
            continue;
        }
        let mut path = Vec::new();
        let mut pid = r.get("parentId").and_then(Value::as_str);
        let mut hops = 0;
        while let Some(id) = pid {
            hops += 1;
            if hops > 64 {
                break;
            }
            match groups.get(id) {
                Some((name, parent)) => {
                    if !name.trim().is_empty() {
                        path.push(name.to_string());
                    }
                    pid = *parent;
                }
                None => break,
            }
        }
        path.reverse();
        let method = r
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("GET")
            .to_ascii_uppercase();
        let url = r
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let headers = kv_pairs(r.get("headers"));
        let body = parse_insomnia_body(r.get("body"));
        let auth = parse_insomnia_auth(r.get("authentication"));
        let name = r.get("name").and_then(Value::as_str).unwrap_or("");
        let label = make_label(&path, name, &method, &url);
        reqs.push(Req {
            label,
            method,
            url,
            headers,
            body,
            auth,
        });
    }
    Ok((reqs, vars))
}

fn parse_insomnia_body(b: Option<&Value>) -> Body {
    let Some(b) = b else {
        return Body::None;
    };
    let mime = b.get("mimeType").and_then(Value::as_str).unwrap_or("");
    if mime.contains("form-urlencoded") {
        return Body::UrlEncoded(kv_pairs(b.get("params")));
    }
    if mime.contains("multipart") {
        let mut fields = Vec::new();
        if let Some(arr) = b.get("params").and_then(Value::as_array) {
            for f in arr {
                if is_disabled(f) {
                    continue;
                }
                let key = f
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let file = f.get("fileName").and_then(Value::as_str).unwrap_or("");
                if f.get("type").and_then(Value::as_str) == Some("file") || !file.is_empty() {
                    fields.push(FormField {
                        key,
                        value: file.to_string(),
                        is_file: true,
                    });
                } else {
                    fields.push(FormField {
                        key,
                        value: value_to_string(f.get("value").unwrap_or(&Value::Null)),
                        is_file: false,
                    });
                }
            }
        }
        return Body::Form(fields);
    }
    let text = b.get("text").and_then(Value::as_str).unwrap_or("");
    if text.trim().is_empty() {
        return Body::None;
    }
    if mime.contains("graphql") {
        // Insomnia stores GraphQL as JSON text: {"query": "...", "variables": {...}}.
        if let Ok(v) = serde_json::from_str::<Value>(text) {
            let query = v
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let vars_val = match v.get("variables") {
                // Variables may themselves be a JSON string.
                Some(Value::String(s)) => {
                    serde_json::from_str(s).unwrap_or(Value::Object(serde_json::Map::new()))
                }
                Some(other) => other.clone(),
                None => Value::Object(serde_json::Map::new()),
            };
            let payload = serde_json::json!({ "query": query, "variables": vars_val });
            return Body::Raw {
                text: serde_json::to_string(&payload).unwrap_or_default(),
                json: true,
            };
        }
    }
    let json = mime.contains("json") || (mime.is_empty() && looks_like_json(text));
    Body::Raw {
        text: text.to_string(),
        json,
    }
}

fn parse_insomnia_auth(a: Option<&Value>) -> Option<Auth> {
    let a = a?;
    if is_disabled(a) {
        return None;
    }
    let get = |k: &str| value_to_string(a.get(k).unwrap_or(&Value::Null));
    match a.get("type").and_then(Value::as_str)? {
        "basic" => Some(Auth::Basic {
            user: get("username"),
            pass: get("password"),
        }),
        "bearer" => Some(Auth::Bearer(get("token"))),
        "apikey" => {
            let key = get("key");
            let value = get("value");
            if get("addTo") == "queryParams" {
                Some(Auth::ApiKeyQuery { key, value })
            } else {
                Some(Auth::ApiKeyHeader { key, value })
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------- generation

/// Add `Content-Type: application/json` for a JSON raw body when absent.
fn effective_headers(req: &Req) -> Vec<(String, String)> {
    let mut headers = req.headers.clone();
    if let Body::Raw { json: true, .. } = &req.body {
        if !headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        {
            headers.push(("Content-Type".into(), "application/json".into()));
        }
    }
    headers
}

/// Shell single-quote.
fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// JavaScript single-quoted string literal.
fn jsq(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

fn urlenc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn append_query(url: &str, key: &str, value: &str) -> String {
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}{}={}", urlenc(key), urlenc(value))
}

fn base64(s: &str) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.len();
        let x = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(x >> 18) as usize & 63] as char);
        out.push(T[(x >> 12) as usize & 63] as char);
        out.push(if n > 1 {
            T[(x >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if n > 2 {
            T[x as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn gen_curl(req: &Req, multiline: bool) -> String {
    let mut url = req.url.clone();
    let mut headers = effective_headers(req);
    let mut basic: Option<String> = None;
    match &req.auth {
        Some(Auth::Basic { user, pass }) => basic = Some(format!("{user}:{pass}")),
        Some(Auth::Bearer(t)) => headers.push(("Authorization".into(), format!("Bearer {t}"))),
        Some(Auth::ApiKeyHeader { key, value }) => headers.push((key.clone(), value.clone())),
        Some(Auth::ApiKeyQuery { key, value }) => url = append_query(&url, key, value),
        None => {}
    }

    let mut parts: Vec<String> = Vec::new();
    let mut first = String::from("curl");
    match req.method.as_str() {
        "GET" => {}
        "HEAD" => first.push_str(" -I"),
        m => {
            first.push_str(" -X ");
            first.push_str(m);
        }
    }
    first.push(' ');
    first.push_str(&shq(&url));
    parts.push(first);
    for (k, v) in &headers {
        parts.push(format!("-H {}", shq(&format!("{k}: {v}"))));
    }
    if let Some(u) = &basic {
        parts.push(format!("-u {}", shq(u)));
    }
    match &req.body {
        Body::None => {}
        Body::Raw { text, .. } => parts.push(format!("--data-raw {}", shq(text))),
        Body::UrlEncoded(pairs) => {
            for (k, v) in pairs {
                parts.push(format!("--data-urlencode {}", shq(&format!("{k}={v}"))));
            }
        }
        Body::Form(fields) => {
            for f in fields {
                let spec = if f.is_file {
                    format!("{}=@{}", f.key, f.value)
                } else {
                    format!("{}={}", f.key, f.value)
                };
                parts.push(format!("-F {}", shq(&spec)));
            }
        }
        Body::File { src } => parts.push(format!("--data-binary {}", shq(&format!("@{src}")))),
    }

    let joined = if multiline && parts.len() > 1 {
        parts.join(" \\\n  ")
    } else {
        parts.join(" ")
    };
    format!("# {}\n{}", req.label, joined)
}

/// The request body as a JS expression, plus any setup lines (FormData).
fn js_body(req: &Req, stringify_json: bool) -> (String, Option<String>) {
    let mut pre = String::new();
    let expr = match &req.body {
        Body::None => None,
        Body::Raw { text, json } => Some(if *json {
            match serde_json::from_str::<Value>(text) {
                // Compact JSON is a valid JS literal.
                Ok(v) => {
                    let compact = serde_json::to_string(&v).unwrap_or_else(|_| text.clone());
                    if stringify_json {
                        format!("JSON.stringify({compact})")
                    } else {
                        compact
                    }
                }
                Err(_) => jsq(text),
            }
        } else {
            jsq(text)
        }),
        Body::UrlEncoded(pairs) => Some(format!(
            "new URLSearchParams([{}])",
            pairs
                .iter()
                .map(|(k, v)| format!("[{}, {}]", jsq(k), jsq(v)))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        Body::Form(fields) => {
            pre.push_str("const formData = new FormData();\n");
            for f in fields {
                if f.is_file {
                    pre.push_str(&format!(
                        "formData.append({}, {}); // attach the real file here\n",
                        jsq(&f.key),
                        jsq(&format!("<file: {}>", f.value))
                    ));
                } else {
                    pre.push_str(&format!(
                        "formData.append({}, {});\n",
                        jsq(&f.key),
                        jsq(&f.value)
                    ));
                }
            }
            Some("formData".into())
        }
        Body::File { src } => Some(jsq(&format!("<contents of {src}>"))),
    };
    (pre, expr)
}

fn gen_fetch(req: &Req) -> String {
    let mut url = req.url.clone();
    let mut headers = effective_headers(req);
    match &req.auth {
        Some(Auth::Basic { user, pass }) => headers.push((
            "Authorization".into(),
            format!("Basic {}", base64(&format!("{user}:{pass}"))),
        )),
        Some(Auth::Bearer(t)) => headers.push(("Authorization".into(), format!("Bearer {t}"))),
        Some(Auth::ApiKeyHeader { key, value }) => headers.push((key.clone(), value.clone())),
        Some(Auth::ApiKeyQuery { key, value }) => url = append_query(&url, key, value),
        None => {}
    }
    // fetch's body must be a string / URLSearchParams / FormData — stringify JSON.
    let (pre, body_expr) = js_body(req, true);

    let mut s = format!("// {}\n", req.label);
    s.push_str(&pre);
    if req.method == "GET" && headers.is_empty() && body_expr.is_none() {
        s.push_str(&format!("const response = await fetch({});\n", jsq(&url)));
    } else {
        s.push_str(&format!("const response = await fetch({}, {{\n", jsq(&url)));
        s.push_str(&format!("  method: {},\n", jsq(&req.method)));
        if !headers.is_empty() {
            s.push_str("  headers: {\n");
            for (k, v) in &headers {
                s.push_str(&format!("    {}: {},\n", jsq(k), jsq(v)));
            }
            s.push_str("  },\n");
        }
        if let Some(b) = body_expr {
            s.push_str(&format!("  body: {b},\n"));
        }
        s.push_str("});\n");
    }
    s.push_str("console.log(await response.text());");
    s
}

fn gen_axios(req: &Req) -> String {
    let mut url = req.url.clone();
    let mut headers = effective_headers(req);
    let mut basic: Option<(String, String)> = None;
    match &req.auth {
        Some(Auth::Basic { user, pass }) => basic = Some((user.clone(), pass.clone())),
        Some(Auth::Bearer(t)) => headers.push(("Authorization".into(), format!("Bearer {t}"))),
        Some(Auth::ApiKeyHeader { key, value }) => headers.push((key.clone(), value.clone())),
        Some(Auth::ApiKeyQuery { key, value }) => url = append_query(&url, key, value),
        None => {}
    }
    // axios serializes plain objects itself — pass JSON bodies as object literals.
    let (pre, body_expr) = js_body(req, false);

    let mut s = format!("// {}\n", req.label);
    s.push_str(&pre);
    s.push_str("const response = await axios({\n");
    s.push_str(&format!(
        "  method: {},\n",
        jsq(&req.method.to_ascii_lowercase())
    ));
    s.push_str(&format!("  url: {},\n", jsq(&url)));
    if !headers.is_empty() {
        s.push_str("  headers: {\n");
        for (k, v) in &headers {
            s.push_str(&format!("    {}: {},\n", jsq(k), jsq(v)));
        }
        s.push_str("  },\n");
    }
    if let Some((u, p)) = &basic {
        s.push_str(&format!(
            "  auth: {{ username: {}, password: {} }},\n",
            jsq(u),
            jsq(p)
        ));
    }
    if let Some(b) = body_expr {
        s.push_str(&format!("  data: {b},\n"));
    }
    s.push_str("});\nconsole.log(response.data);");
    s
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    const POSTMAN_SIMPLE: &str = r#"{
        "info": { "name": "Demo", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
        "item": [
            {
                "name": "Get user",
                "request": {
                    "method": "GET",
                    "header": [{ "key": "Accept", "value": "application/json" }],
                    "url": { "raw": "https://api.example.com/users/1" }
                }
            }
        ]
    }"#;

    #[test]
    fn postman_get_to_curl_multiline() {
        let out = convert(POSTMAN_SIMPLE, "curl", "", true).unwrap();
        assert_eq!(
            out,
            "# Get user\ncurl 'https://api.example.com/users/1' \\\n  -H 'Accept: application/json'"
        );
    }

    #[test]
    fn postman_get_to_curl_single_line() {
        let out = convert(POSTMAN_SIMPLE, "curl", "", false).unwrap();
        assert_eq!(
            out,
            "# Get user\ncurl 'https://api.example.com/users/1' -H 'Accept: application/json'"
        );
    }

    #[test]
    fn postman_post_json_folder_auth_to_curl() {
        let coll = r#"{
            "info": { "name": "Demo" },
            "item": [{
                "name": "Users",
                "item": [{
                    "name": "Create user",
                    "request": {
                        "method": "POST",
                        "auth": { "type": "bearer", "bearer": [{ "key": "token", "value": "abc123" }] },
                        "url": "https://api.example.com/users",
                        "body": { "mode": "raw", "raw": "{\"name\":\"Ada\"}", "options": { "raw": { "language": "json" } } }
                    }
                }]
            }]
        }"#;
        let out = convert(coll, "curl", "", true).unwrap();
        assert_eq!(
            out,
            "# Users / Create user\ncurl -X POST 'https://api.example.com/users' \\\n  -H 'Content-Type: application/json' \\\n  -H 'Authorization: Bearer abc123' \\\n  --data-raw '{\"name\":\"Ada\"}'"
        );
    }

    #[test]
    fn postman_basic_auth_to_fetch_base64() {
        let coll = r#"{
            "info": { "name": "Demo" },
            "item": [{
                "name": "Account",
                "request": {
                    "method": "GET",
                    "auth": { "type": "basic", "basic": [
                        { "key": "username", "value": "alice" },
                        { "key": "password", "value": "s3cret" }
                    ]},
                    "url": "https://api.example.com/account"
                }
            }]
        }"#;
        let out = convert(coll, "fetch", "", true).unwrap();
        // echo -n 'alice:s3cret' | base64 → YWxpY2U6czNjcmV0
        assert!(
            out.contains("'Authorization': 'Basic YWxpY2U6czNjcmV0'"),
            "got: {out}"
        );
        assert!(out.contains("method: 'GET'"), "got: {out}");
    }

    #[test]
    fn postman_v20_object_auth_and_apikey_query() {
        let coll = r#"{
            "info": { "name": "Demo" },
            "item": [{
                "name": "List",
                "request": {
                    "method": "GET",
                    "auth": { "type": "apikey", "apikey": { "key": "api_key", "value": "k-1", "in": "query" } },
                    "url": "https://api.example.com/items?page=2"
                }
            }]
        }"#;
        let out = convert(coll, "curl", "", true).unwrap();
        assert!(
            out.contains("'https://api.example.com/items?page=2&api_key=k-1'"),
            "got: {out}"
        );
    }

    #[test]
    fn variables_precedence_and_forms() {
        let coll = r#"{
            "info": { "name": "Demo" },
            "variable": [
                { "key": "base", "value": "https://coll.example.com" },
                { "key": "who", "value": "collection" }
            ],
            "item": [{ "name": "Ping", "request": { "method": "GET", "url": "{{base}}/ping?u={{who}}&x={{missing}}" } }]
        }"#;
        // KEY=VALUE lines override collection variables; unknown stays verbatim.
        let out = convert(
            coll,
            "curl",
            "base=https://user.example.com\nwho=user",
            true,
        )
        .unwrap();
        assert!(
            out.contains("'https://user.example.com/ping?u=user&x={{missing}}'"),
            "got: {out}"
        );
        // Plain JSON object form.
        let out2 = convert(coll, "curl", r#"{"who":"json"}"#, true).unwrap();
        assert!(
            out2.contains("'https://coll.example.com/ping?u=json"),
            "got: {out2}"
        );
        // Postman environment export form (disabled entries skipped).
        let env = r#"{"values":[{"key":"who","value":"env","enabled":true},{"key":"base","value":"http://off.example.com","enabled":false}]}"#;
        let out3 = convert(coll, "curl", env, true).unwrap();
        assert!(
            out3.contains("'https://coll.example.com/ping?u=env"),
            "got: {out3}"
        );
    }

    #[test]
    fn postman_urlencoded_and_formdata() {
        let coll = r#"{
            "info": { "name": "Demo" },
            "item": [
                { "name": "Login", "request": {
                    "method": "POST",
                    "url": "https://example.com/login",
                    "body": { "mode": "urlencoded", "urlencoded": [
                        { "key": "user", "value": "ada" },
                        { "key": "skip", "value": "x", "disabled": true },
                        { "key": "pass", "value": "p w" }
                    ]}
                }},
                { "name": "Upload", "request": {
                    "method": "POST",
                    "url": "https://example.com/upload",
                    "body": { "mode": "formdata", "formdata": [
                        { "key": "title", "value": "Holiday", "type": "text" },
                        { "key": "photo", "src": "photo.png", "type": "file" }
                    ]}
                }}
            ]
        }"#;
        let out = convert(coll, "curl", "", true).unwrap();
        assert!(out.contains("--data-urlencode 'user=ada'"), "got: {out}");
        assert!(!out.contains("skip=x"), "disabled field leaked: {out}");
        assert!(out.contains("--data-urlencode 'pass=p w'"), "got: {out}");
        assert!(out.contains("-F 'title=Holiday'"), "got: {out}");
        assert!(out.contains("-F 'photo=@photo.png'"), "got: {out}");
        let js = convert(coll, "fetch", "", true).unwrap();
        assert!(
            js.contains("new URLSearchParams([['user', 'ada'], ['pass', 'p w']])"),
            "got: {js}"
        );
        assert!(
            js.contains("formData.append('title', 'Holiday');"),
            "got: {js}"
        );
        assert!(js.contains("body: formData"), "got: {js}");
    }

    #[test]
    fn postman_graphql_body() {
        let coll = r#"{
            "info": { "name": "Demo" },
            "item": [{ "name": "GQL", "request": {
                "method": "POST",
                "url": "https://example.com/graphql",
                "body": { "mode": "graphql", "graphql": { "query": "query { me { id } }", "variables": "{\"a\":1}" } }
            }}]
        }"#;
        let out = convert(coll, "curl", "", true).unwrap();
        assert!(
            out.contains(r#"--data-raw '{"query":"query { me { id } }","variables":{"a":1}}'"#),
            "got: {out}"
        );
        assert!(
            out.contains("-H 'Content-Type: application/json'"),
            "got: {out}"
        );
    }

    #[test]
    fn axios_json_body_is_object_literal() {
        let coll = r#"{
            "info": { "name": "Demo" },
            "item": [{ "name": "Create", "request": {
                "method": "POST",
                "url": "https://api.example.com/users",
                "header": [{ "key": "X-Trace", "value": "1" }],
                "body": { "mode": "raw", "raw": "{\"name\":\"Ada\",\"age\":36}", "options": { "raw": { "language": "json" } } }
            }}]
        }"#;
        let out = convert(coll, "axios", "", true).unwrap();
        assert_eq!(
            out,
            "// Create\nconst response = await axios({\n  method: 'post',\n  url: 'https://api.example.com/users',\n  headers: {\n    'X-Trace': '1',\n    'Content-Type': 'application/json',\n  },\n  data: {\"name\":\"Ada\",\"age\":36},\n});\nconsole.log(response.data);"
        );
    }

    #[test]
    fn insomnia_export_with_env_and_folder() {
        let coll = r#"{
            "_type": "export",
            "__export_format": 4,
            "resources": [
                { "_id": "env_1", "_type": "environment", "data": { "base_url": "https://api.example.com", "token": "t-9" } },
                { "_id": "fld_1", "_type": "request_group", "name": "Admin", "parentId": "wrk_1" },
                { "_id": "req_1", "_type": "request", "parentId": "fld_1", "name": "List users",
                  "method": "GET", "url": "{{ _.base_url }}/users",
                  "headers": [{ "name": "Accept", "value": "application/json" }],
                  "authentication": { "type": "bearer", "token": "{{ _.token }}" } }
            ]
        }"#;
        let out = convert(coll, "curl", "", true).unwrap();
        assert_eq!(
            out,
            "# Admin / List users\ncurl 'https://api.example.com/users' \\\n  -H 'Accept: application/json' \\\n  -H 'Authorization: Bearer t-9'"
        );
    }

    #[test]
    fn insomnia_json_body_to_fetch() {
        let coll = r#"{
            "_type": "export",
            "__export_format": 4,
            "resources": [
                { "_id": "req_1", "_type": "request", "name": "Create",
                  "method": "POST", "url": "https://api.example.com/users",
                  "body": { "mimeType": "application/json", "text": "{\"name\":\"Ada\"}" } }
            ]
        }"#;
        let out = convert(coll, "fetch", "", true).unwrap();
        assert!(
            out.contains("body: JSON.stringify({\"name\":\"Ada\"})"),
            "got: {out}"
        );
        assert!(
            out.contains("'Content-Type': 'application/json'"),
            "got: {out}"
        );
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        let coll = r#"{
            "info": { "name": "Demo" },
            "item": [{ "name": "Odd", "request": {
                "method": "POST",
                "url": "https://example.com/x",
                "body": { "mode": "raw", "raw": "it's raw", "options": { "raw": { "language": "text" } } }
            }}]
        }"#;
        let out = convert(coll, "curl", "", false).unwrap();
        assert!(out.contains(r#"--data-raw 'it'\''s raw'"#), "got: {out}");
    }

    #[test]
    fn request_cap_at_and_over_boundary() {
        let mk = |n: usize| {
            let items: Vec<String> = (0..n)
                .map(|i| {
                    format!(
                        r#"{{ "name": "r{i}", "request": {{ "method": "GET", "url": "https://example.com/{i}" }} }}"#
                    )
                })
                .collect();
            format!(
                r#"{{ "info": {{ "name": "Big" }}, "item": [{}] }}"#,
                items.join(",")
            )
        };
        let ok = convert(&mk(200), "curl", "", false).unwrap();
        assert_eq!(ok.matches("\n\n").count(), 199);
        let err = convert(&mk(201), "curl", "", false).unwrap_err();
        assert_eq!(err, "collection contains 201 requests; the limit is 200");
    }

    #[test]
    fn error_invalid_json() {
        let err = convert("not json", "curl", "", true).unwrap_err();
        assert!(
            err.starts_with("collection is not valid JSON:"),
            "got: {err}"
        );
    }

    #[test]
    fn error_unrecognized_format() {
        let err = convert(r#"{"foo": 1}"#, "curl", "", true).unwrap_err();
        assert!(
            err.starts_with("unrecognized collection format:"),
            "got: {err}"
        );
    }

    #[test]
    fn error_no_requests() {
        let err = convert(r#"{"info":{"name":"Empty"},"item":[]}"#, "curl", "", true).unwrap_err();
        assert_eq!(err, "no requests found in the collection");
    }

    #[test]
    fn error_unknown_target() {
        let err = convert(POSTMAN_SIMPLE, "python", "", true).unwrap_err();
        assert_eq!(
            err,
            "unknown target 'python' (expected curl, fetch, or axios)"
        );
    }

    #[test]
    fn error_bad_variables_line() {
        let err = convert(POSTMAN_SIMPLE, "curl", "novalue", true).unwrap_err();
        assert_eq!(err, "variables line 1 is not KEY=VALUE: novalue");
    }

    #[test]
    fn head_uses_dash_i_and_url_object_reassembly() {
        let coll = r#"{
            "info": { "name": "Demo" },
            "item": [{ "name": "Check", "request": {
                "method": "HEAD",
                "url": { "protocol": "https", "host": ["api", "example", "com"], "path": ["status"], "query": [{ "key": "v", "value": "1" }] }
            }}]
        }"#;
        let out = convert(coll, "curl", "", false).unwrap();
        assert_eq!(out, "# Check\ncurl -I 'https://api.example.com/status?v=1'");
    }
}
