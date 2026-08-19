//! postman-collection-extractor core — flatten a Postman Collection v2.x export
//! into a list of requests: folder path, name, method, URL, headers, body, and
//! auth type. Pure Rust (serde/serde_json only), shared by the chat skill
//! block, the CLI, and the web page. No wafer/wasm-bindgen deps.
//!
//! Sibling scope note: `blocks/postman-collection-converter` turns the same
//! input into runnable curl/fetch/axios code — this block is the *inventory*
//! (what endpoints exist, with their headers and bodies), the Postman analogue
//! of `blocks/har-request-extract`.
//!
//! Deliberately FORGIVING per item (a request missing `method` still lists as
//! GET, a missing URL lists empty) so a partially-broken export is still
//! usable; only non-JSON / non-collection input is a hard error.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

/// Hard cap on requests flattened in one run.
pub const MAX_REQUESTS: usize = 500;
/// Per-request body characters kept in the output; longer bodies are truncated.
pub const MAX_BODY_CHARS: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Format {
    List,
    Table,
    Json,
    Csv,
    Markdown,
    Urls,
}

fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "list" => Ok(Format::List),
        "table" => Ok(Format::Table),
        "json" => Ok(Format::Json),
        "csv" => Ok(Format::Csv),
        "markdown" | "md" => Ok(Format::Markdown),
        "urls" | "url" => Ok(Format::Urls),
        other => Err(format!(
            "unknown format '{other}': expected list, table, json, csv, markdown, or urls"
        )),
    }
}

/// One flattened request. `index` is the 1-based position in the ORIGINAL
/// collection order, so rows stay cross-referenceable after filtering.
#[derive(Debug, Clone)]
struct Req {
    index: usize,
    /// `"Users / Admin"`; empty for a request at the collection root.
    folder: String,
    name: String,
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    /// none | raw | json | urlencoded | formdata | file | graphql
    body_mode: String,
    body: String,
    /// none | inherited | bearer | basic | apikey | oauth2 | …
    auth: String,
}

/// JSON output row — the field order here is the emitted key order.
#[derive(Serialize)]
struct JsonRow<'a> {
    index: usize,
    folder: &'a str,
    name: &'a str,
    method: &'a str,
    url: &'a str,
    auth: &'a str,
    headers: Vec<JsonHeader<'a>>,
    body_mode: &'a str,
    body: &'a str,
}

#[derive(Serialize)]
struct JsonHeader<'a> {
    name: &'a str,
    value: &'a str,
}

/// Flatten a Postman collection into a request list.
///
/// `format` = list|table|json|csv|markdown|urls (empty = list); `method` /
/// `url_contains` / `folder` are optional case-insensitive filters;
/// `variables` accepts a JSON object, a Postman environment export, or
/// `KEY=VALUE` lines; `resolve_variables` substitutes `{{placeholders}}` from
/// the collection's own variables plus `variables`.
pub fn extract(
    collection: &str,
    format: &str,
    method: &str,
    url_contains: &str,
    folder: &str,
    variables: &str,
    resolve_variables: bool,
) -> Result<String, String> {
    let format = parse_format(format)?;
    if collection.trim().is_empty() {
        return Err("collection is empty — paste the exported Postman Collection JSON".into());
    }
    let root: Value = serde_json::from_str(collection.trim())
        .map_err(|e| format!("collection is not valid JSON: {e}"))?;
    let items = root.get("item").and_then(Value::as_array).ok_or_else(|| {
        "unrecognized format: expected a Postman Collection v2.0/v2.1 export — a JSON object with \
         a top-level `item` array. Collection v1 exports must be converted to v2 first."
            .to_string()
    })?;

    let collection_auth = auth_type(root.get("auth"));
    let mut reqs = Vec::new();
    walk(items, &[], &collection_auth, &mut reqs);
    if reqs.is_empty() {
        return Err("no requests found in the collection".into());
    }
    let total = reqs.len();
    if total > MAX_REQUESTS {
        return Err(format!(
            "collection contains {total} requests; the limit is {MAX_REQUESTS} per run"
        ));
    }

    if resolve_variables {
        let mut vars = collection_variables(&root);
        for (k, v) in parse_variables(variables)? {
            vars.insert(k, v);
        }
        if !vars.is_empty() {
            for r in &mut reqs {
                apply_vars(r, &vars);
            }
        }
    }
    for r in &mut reqs {
        r.body = truncate(&r.body);
    }

    let kept: Vec<&Req> = reqs
        .iter()
        .filter(|r| matches_filters(r, method, url_contains, folder))
        .collect();
    if kept.is_empty() {
        return Err(format!(
            "no requests match the filters — the collection has {total} request(s); \
             relax the method / URL / folder filter"
        ));
    }

    Ok(match format {
        Format::List => render_list(&kept, total),
        Format::Table => render_table(&kept, total),
        Format::Json => render_json(&kept),
        Format::Csv => render_csv(&kept),
        Format::Markdown => render_markdown(&kept),
        Format::Urls => render_urls(&kept),
    })
}

// ---------------------------------------------------------------- collection

/// Depth-first walk: an item with a nested `item` array is a folder, an item
/// with a `request` is a request. Both can carry an `auth` block.
fn walk(items: &[Value], path: &[String], inherited_auth: &str, out: &mut Vec<Req>) {
    for item in items {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let auth = match auth_type(item.get("auth")).as_str() {
            "none" => inherited_auth.to_string(),
            a => a.to_string(),
        };
        if let Some(children) = item.get("item").and_then(Value::as_array) {
            let mut next = path.to_vec();
            if !name.is_empty() {
                next.push(name);
            }
            walk(children, &next, &auth, out);
        } else if let Some(request) = item.get("request") {
            let index = out.len() + 1;
            out.push(build_req(index, path, &name, request, &auth));
        }
    }
}

fn build_req(index: usize, path: &[String], name: &str, request: &Value, inherited: &str) -> Req {
    // Shorthand form: `"request": "https://example.com/x"` (a bare GET URL).
    if let Some(url) = request.as_str() {
        return Req {
            index,
            folder: path.join(" / "),
            name: name.to_string(),
            method: "GET".into(),
            url: url.to_string(),
            headers: Vec::new(),
            body_mode: "none".into(),
            body: String::new(),
            auth: inherited.to_string(),
        };
    }
    let auth = match auth_type(request.get("auth")).as_str() {
        "none" => inherited.to_string(),
        a => a.to_string(),
    };
    Req {
        index,
        folder: path.join(" / "),
        name: name.to_string(),
        method: request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("GET")
            .trim()
            .to_ascii_uppercase(),
        url: extract_url(request.get("url")),
        headers: extract_headers(request.get("header")),
        body_mode: body_mode(request.get("body")),
        body: extract_body(request.get("body")),
        auth,
    }
}

/// `auth` is either absent, `{"type":"bearer", …}`, or `{"type":"noauth"}`.
/// A request whose own auth is absent reports the enclosing folder's /
/// collection's type as `inherited` (Postman's own wording for that state).
fn auth_type(auth: Option<&Value>) -> String {
    match auth.and_then(|a| a.get("type")).and_then(Value::as_str) {
        None | Some("noauth") => "none".into(),
        Some(t) => {
            let t = t.trim().to_ascii_lowercase();
            if t.is_empty() {
                "none".into()
            } else {
                t
            }
        }
    }
}

/// v2.0 stores `url` as a string; v2.1 as an object with `raw` plus parsed
/// parts. Prefer `raw` (it is what Postman displays), else rebuild from the
/// parts, dropping disabled query params.
fn extract_url(url: Option<&Value>) -> String {
    let Some(url) = url else {
        return String::new();
    };
    if let Some(s) = url.as_str() {
        return s.trim().to_string();
    }
    if let Some(raw) = url.get("raw").and_then(Value::as_str) {
        if !raw.trim().is_empty() {
            return raw.trim().to_string();
        }
    }
    let mut out = String::new();
    if let Some(p) = url.get("protocol").and_then(Value::as_str) {
        out.push_str(p);
        out.push_str("://");
    }
    out.push_str(&join_parts(url.get("host"), "."));
    let path = join_parts(url.get("path"), "/");
    if !path.is_empty() {
        out.push('/');
        out.push_str(&path);
    }
    let query: Vec<String> = url
        .get("query")
        .and_then(Value::as_array)
        .map(|q| {
            q.iter()
                .filter(|e| !disabled(e))
                .map(|e| {
                    let k = str_field(e, "key");
                    let v = str_field(e, "value");
                    if v.is_empty() {
                        k
                    } else {
                        format!("{k}={v}")
                    }
                })
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if !query.is_empty() {
        out.push('?');
        out.push_str(&query.join("&"));
    }
    out
}

/// `host`/`path` are arrays of segments in v2.1 but plain strings in some
/// exports.
fn join_parts(v: Option<&Value>, sep: &str) -> String {
    match v {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(sep),
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

/// Headers are an array of `{key,value,disabled}` (disabled ones are skipped,
/// matching what Postman would actually send) or a raw `K: V` block.
fn extract_headers(header: Option<&Value>) -> Vec<(String, String)> {
    match header {
        Some(Value::Array(a)) => a
            .iter()
            .filter(|h| !disabled(h))
            .map(|h| (str_field(h, "key"), str_field(h, "value")))
            .filter(|(k, _)| !k.is_empty())
            .collect(),
        Some(Value::String(s)) => s
            .lines()
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .filter(|(k, _)| !k.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn body_mode(body: Option<&Value>) -> String {
    let Some(body) = body else {
        return "none".into();
    };
    match body.get("mode").and_then(Value::as_str).unwrap_or("") {
        "raw" => {
            let lang = body
                .get("options")
                .and_then(|o| o.get("raw"))
                .and_then(|r| r.get("language"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if lang.eq_ignore_ascii_case("json") {
                "json".into()
            } else {
                "raw".into()
            }
        }
        "urlencoded" => "urlencoded".into(),
        "formdata" => "formdata".into(),
        "file" => "file".into(),
        "graphql" => "graphql".into(),
        _ => "none".into(),
    }
}

fn extract_body(body: Option<&Value>) -> String {
    let Some(body) = body else {
        return String::new();
    };
    match body.get("mode").and_then(Value::as_str).unwrap_or("") {
        "raw" => body
            .get("raw")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        "urlencoded" => body
            .get("urlencoded")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|f| !disabled(f))
                    .map(|f| format!("{}={}", str_field(f, "key"), str_field(f, "value")))
                    .collect::<Vec<_>>()
                    .join("&")
            })
            .unwrap_or_default(),
        // form-data fields print one per line; file parts show `@<src>` so the
        // upload is visible without pretending a value exists.
        "formdata" => body
            .get("formdata")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|f| !disabled(f))
                    .map(|f| {
                        let key = str_field(f, "key");
                        if str_field(f, "type") == "file" {
                            format!("{key}=@{}", file_src(f.get("src")))
                        } else {
                            format!("{key}={}", str_field(f, "value"))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        "file" => {
            let src = file_src(body.get("file").and_then(|f| f.get("src")));
            if src.is_empty() {
                String::new()
            } else {
                format!("@{src}")
            }
        }
        "graphql" => {
            let g = body.get("graphql");
            let query = g
                .and_then(|g| g.get("query"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let vars = g.and_then(|g| g.get("variables"));
            match vars {
                Some(Value::String(s)) if !s.trim().is_empty() => {
                    format!("{query}\n\nvariables: {}", s.trim())
                }
                Some(v @ Value::Object(_)) => format!("{query}\n\nvariables: {v}"),
                _ => query,
            }
        }
        _ => String::new(),
    }
}

/// `src` is a path string, or an array of paths for a multi-file field.
fn file_src(src: Option<&Value>) -> String {
    match src {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

fn disabled(v: &Value) -> bool {
    v.get("disabled").and_then(Value::as_bool).unwrap_or(false)
}

fn str_field(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

// ----------------------------------------------------------------- variables

/// Collection-level `variable: [{key, value}]` (v2.x). Disabled entries are
/// skipped, as Postman does when resolving.
fn collection_variables(root: &Value) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(vars) = root.get("variable").and_then(Value::as_array) {
        for v in vars.iter().filter(|v| !disabled(v)) {
            let k = str_field(v, "key");
            if !k.is_empty() {
                map.insert(k, str_field(v, "value"));
            }
        }
    }
    map
}

/// Accepts a JSON object (`{"baseUrl":"…"}`), a Postman **environment** export
/// (`{"values":[{"key","value","enabled"}]}`), a collection export's
/// `variable` array, or plain `KEY=VALUE` lines.
fn parse_variables(input: &str) -> Result<HashMap<String, String>, String> {
    let text = input.trim();
    let mut map = HashMap::new();
    if text.is_empty() {
        return Ok(map);
    }
    if text.starts_with('{') || text.starts_with('[') {
        let v: Value = serde_json::from_str(text)
            .map_err(|e| format!("variables is not valid JSON: {e}"))?;
        let list = match &v {
            Value::Array(a) => Some(a.clone()),
            Value::Object(_) => v
                .get("values")
                .or_else(|| v.get("variable"))
                .and_then(Value::as_array)
                .cloned(),
            _ => None,
        };
        if let Some(list) = list {
            for e in list.iter() {
                let enabled = e
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
                    && !disabled(e);
                let k = str_field(e, "key");
                if enabled && !k.is_empty() {
                    map.insert(k, str_field(e, "value"));
                }
            }
            return Ok(map);
        }
        if let Value::Object(o) = v {
            for (k, val) in o {
                let s = match val {
                    Value::String(s) => s,
                    other => other.to_string(),
                };
                map.insert(k, s);
            }
            return Ok(map);
        }
        return Err("variables must be a JSON object, an environment export, or KEY=VALUE lines".into());
    }
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line
            .split_once('=')
            .ok_or_else(|| format!("variables line '{line}' is not KEY=VALUE"))?;
        map.insert(k.trim().to_string(), v.trim().to_string());
    }
    Ok(map)
}

/// Replace `{{key}}` everywhere it appears. Unknown placeholders are left
/// verbatim so nothing silently disappears from the inventory.
fn apply_vars(r: &mut Req, vars: &HashMap<String, String>) {
    r.url = substitute(&r.url, vars);
    r.body = substitute(&r.body, vars);
    for (k, v) in &mut r.headers {
        *k = substitute(k, vars);
        *v = substitute(v, vars);
    }
}

fn substitute(s: &str, vars: &HashMap<String, String>) -> String {
    if !s.contains("{{") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key.trim()) {
                    Some(v) => out.push_str(v),
                    None => {
                        out.push_str("{{");
                        out.push_str(key);
                        out.push_str("}}");
                    }
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str("{{");
                out.push_str(after);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn truncate(s: &str) -> String {
    if s.chars().count() <= MAX_BODY_CHARS {
        return s.to_string();
    }
    let kept: String = s.chars().take(MAX_BODY_CHARS).collect();
    format!("{kept}… (truncated at {MAX_BODY_CHARS} characters)")
}

// ------------------------------------------------------------------ filtering

fn matches_filters(r: &Req, method: &str, url_contains: &str, folder: &str) -> bool {
    let method = method.trim();
    if !method.is_empty() && !r.method.eq_ignore_ascii_case(method) {
        return false;
    }
    let url_contains = url_contains.trim();
    if !url_contains.is_empty()
        && !r
            .url
            .to_lowercase()
            .contains(&url_contains.to_lowercase())
    {
        return false;
    }
    let folder = folder.trim();
    if !folder.is_empty() && !r.folder.to_lowercase().contains(&folder.to_lowercase()) {
        return false;
    }
    true
}

// ------------------------------------------------------------------ rendering

fn summary(kept: &[&Req], total: usize) -> String {
    let mut folders: Vec<&str> = kept
        .iter()
        .map(|r| r.folder.as_str())
        .filter(|f| !f.is_empty())
        .collect();
    folders.sort_unstable();
    folders.dedup();
    format!(
        "{} of {} requests · {} folder{}",
        kept.len(),
        total,
        folders.len(),
        if folders.len() == 1 { "" } else { "s" }
    )
}

fn render_list(kept: &[&Req], total: usize) -> String {
    let mut out = format!("{}\n", summary(kept, total));
    for r in kept {
        let name = if r.name.is_empty() {
            "(unnamed)"
        } else {
            &r.name
        };
        out.push_str(&format!("\n{}. {name}\n", r.index));
        out.push_str(&format!("   {} {}\n", r.method, r.url));
        if !r.folder.is_empty() {
            out.push_str(&format!("   Folder: {}\n", r.folder));
        }
        if r.auth != "none" {
            out.push_str(&format!("   Auth: {}\n", r.auth));
        }
        if r.headers.is_empty() {
            out.push_str("   Headers: (none)\n");
        } else {
            out.push_str("   Headers:\n");
            for (k, v) in &r.headers {
                out.push_str(&format!("     {k}: {v}\n"));
            }
        }
        if r.body_mode == "none" || r.body.is_empty() {
            out.push_str("   Body: (none)\n");
        } else {
            out.push_str(&format!("   Body ({}):\n", r.body_mode));
            for line in r.body.lines() {
                out.push_str(&format!("     {line}\n"));
            }
        }
    }
    out
}

fn render_table(kept: &[&Req], total: usize) -> String {
    let rows: Vec<[String; 5]> = kept
        .iter()
        .map(|r| {
            [
                r.index.to_string(),
                r.method.clone(),
                r.headers.len().to_string(),
                r.body_mode.clone(),
                if r.name.is_empty() {
                    "(unnamed)".to_string()
                } else {
                    r.name.clone()
                },
            ]
        })
        .collect();
    let head = ["#", "METHOD", "HDRS", "BODY", "NAME"];
    let mut w = [0usize; 5];
    for (i, h) in head.iter().enumerate() {
        w[i] = h.chars().count();
        for row in &rows {
            w[i] = w[i].max(row[i].chars().count());
        }
    }
    let mut out = format!("{}\n\n", summary(kept, total));
    // The URL column is last and unpadded (URLs are long); every earlier
    // column is padded identically in the header and the rows so they line up.
    for (i, h) in head.iter().enumerate() {
        out.push_str(&pad(h, w[i]));
        out.push_str("  ");
    }
    out.push_str("URL\n");
    for (row, r) in rows.iter().zip(kept.iter()) {
        let mut line = String::new();
        for i in 0..5 {
            line.push_str(&pad(&row[i], w[i]));
            line.push_str("  ");
        }
        out.push_str(&line);
        out.push_str(&r.url);
        out.push('\n');
    }
    out
}

fn pad(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - len))
    }
}

fn render_json(kept: &[&Req]) -> String {
    let rows: Vec<JsonRow> = kept
        .iter()
        .map(|r| JsonRow {
            index: r.index,
            folder: &r.folder,
            name: &r.name,
            method: &r.method,
            url: &r.url,
            auth: &r.auth,
            headers: r
                .headers
                .iter()
                .map(|(k, v)| JsonHeader { name: k, value: v })
                .collect(),
            body_mode: &r.body_mode,
            body: &r.body,
        })
        .collect();
    serde_json::to_string_pretty(&rows).unwrap_or_else(|e| format!("[] // serialization error: {e}"))
}

fn render_csv(kept: &[&Req]) -> String {
    let mut out =
        String::from("index,folder,name,method,url,auth,headers,body_mode,body\n");
    for r in kept {
        let headers = r
            .headers
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join("; ");
        let fields = [
            r.index.to_string(),
            r.folder.clone(),
            r.name.clone(),
            r.method.clone(),
            r.url.clone(),
            r.auth.clone(),
            headers,
            r.body_mode.clone(),
            r.body.clone(),
        ];
        out.push_str(
            &fields
                .iter()
                .map(|f| csv_field(f))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

/// RFC 4180 quoting — a multi-line JSON body stays one CSV cell.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_markdown(kept: &[&Req]) -> String {
    let mut out = String::from("| # | Method | Name | URL | Headers | Body |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for r in kept {
        let headers = r
            .headers
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join("<br>");
        let name = if r.name.is_empty() {
            "(unnamed)".to_string()
        } else {
            r.name.clone()
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            r.index,
            md_cell(&r.method),
            md_cell(&name),
            md_cell(&r.url),
            md_cell(&headers),
            md_cell(&r.body),
        ));
    }
    out
}

/// Pipes would break the table and newlines would end the row.
fn md_cell(s: &str) -> String {
    if s.is_empty() {
        return "—".into();
    }
    s.replace('|', "\\|").replace("\r\n", "<br>").replace(['\n', '\r'], "<br>")
}

/// One URL per line, first occurrence wins — a duplicate endpoint hit by
/// several requests is listed once.
fn render_urls(kept: &[&Req]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = String::new();
    for r in kept {
        if r.url.is_empty() || seen.contains(&r.url.as_str()) {
            continue;
        }
        seen.push(&r.url);
        out.push_str(&r.url);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLLECTION: &str = r#"{
        "info": { "name": "Demo API", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
        "variable": [{ "key": "baseUrl", "value": "https://api.example.com" }],
        "auth": { "type": "bearer", "bearer": [{ "key": "token", "value": "secret" }] },
        "item": [
            { "name": "List users", "request": { "method": "GET", "url": { "raw": "{{baseUrl}}/users?page=1" } } },
            {
                "name": "Users",
                "item": [
                    {
                        "name": "Create user",
                        "request": {
                            "method": "post",
                            "url": "{{baseUrl}}/users",
                            "header": [
                                { "key": "Content-Type", "value": "application/json" },
                                { "key": "X-Debug", "value": "1", "disabled": true }
                            ],
                            "body": { "mode": "raw", "raw": "{\"name\":\"Ada\"}", "options": { "raw": { "language": "json" } } }
                        }
                    }
                ]
            }
        ]
    }"#;

    fn run(format: &str) -> String {
        extract(COLLECTION, format, "", "", "", "", true).unwrap()
    }

    #[test]
    fn list_shows_method_url_headers_and_body() {
        let out = run("list");
        assert_eq!(
            out,
            "2 of 2 requests · 1 folder\n\
             \n\
             1. List users\n\
             \x20  GET https://api.example.com/users?page=1\n\
             \x20  Auth: bearer\n\
             \x20  Headers: (none)\n\
             \x20  Body: (none)\n\
             \n\
             2. Create user\n\
             \x20  POST https://api.example.com/users\n\
             \x20  Folder: Users\n\
             \x20  Auth: bearer\n\
             \x20  Headers:\n\
             \x20    Content-Type: application/json\n\
             \x20  Body (json):\n\
             \x20    {\"name\":\"Ada\"}\n"
        );
    }

    #[test]
    fn table_is_aligned_with_a_summary() {
        let out = run("table");
        assert_eq!(
            out,
            "2 of 2 requests · 1 folder\n\
             \n\
             #  METHOD  HDRS  BODY  NAME         URL\n\
             1  GET     0     none  List users   https://api.example.com/users?page=1\n\
             2  POST    1     json  Create user  https://api.example.com/users\n"
        );
    }

    #[test]
    fn csv_quotes_bodies_and_keeps_columns() {
        let out = run("csv");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "index,folder,name,method,url,auth,headers,body_mode,body");
        assert_eq!(
            lines[2],
            "2,Users,Create user,POST,https://api.example.com/users,bearer,Content-Type: application/json,json,\"{\"\"name\"\":\"\"Ada\"\"}\""
        );
    }

    #[test]
    fn json_rows_carry_every_field() {
        let v: Value = serde_json::from_str(&run("json")).unwrap();
        let second = &v[1];
        assert_eq!(second["method"], "POST");
        assert_eq!(second["folder"], "Users");
        assert_eq!(second["auth"], "bearer");
        assert_eq!(second["headers"][0]["name"], "Content-Type");
        assert_eq!(second["body_mode"], "json");
        assert_eq!(second["body"], "{\"name\":\"Ada\"}");
        // The disabled header is not part of what Postman would send.
        assert_eq!(second["headers"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn markdown_escapes_and_urls_dedupe() {
        assert!(run("markdown").contains("| 2 | POST | Create user | https://api.example.com/users |"));
        assert_eq!(
            run("urls"),
            "https://api.example.com/users?page=1\nhttps://api.example.com/users\n"
        );
    }

    #[test]
    fn unresolved_placeholders_stay_verbatim_when_resolution_is_off() {
        let out = extract(COLLECTION, "urls", "", "", "", "", false).unwrap();
        assert_eq!(out, "{{baseUrl}}/users?page=1\n{{baseUrl}}/users\n");
    }

    #[test]
    fn user_variables_override_collection_variables() {
        let out = extract(COLLECTION, "urls", "", "", "", "baseUrl=https://staging.test", true)
            .unwrap();
        assert_eq!(
            out,
            "https://staging.test/users?page=1\nhttps://staging.test/users\n"
        );
        let env = r#"{"values":[{"key":"baseUrl","value":"https://env.test","enabled":true}]}"#;
        assert!(extract(COLLECTION, "urls", "", "", "", env, true)
            .unwrap()
            .starts_with("https://env.test/users"));
    }

    #[test]
    fn filters_narrow_by_method_url_and_folder() {
        assert_eq!(
            extract(COLLECTION, "urls", "post", "", "", "", true).unwrap(),
            "https://api.example.com/users\n"
        );
        assert_eq!(
            extract(COLLECTION, "urls", "", "page=1", "", "", true).unwrap(),
            "https://api.example.com/users?page=1\n"
        );
        assert!(extract(COLLECTION, "table", "", "", "users", "", true)
            .unwrap()
            .starts_with("1 of 2 requests · 1 folder"));
    }

    #[test]
    fn url_object_without_raw_is_rebuilt_from_parts() {
        let c = r#"{"item":[{"name":"Search","request":{"method":"GET","url":{
            "protocol":"https","host":["api","example","com"],"path":["v1","search"],
            "query":[{"key":"q","value":"ada"},{"key":"debug","value":"1","disabled":true}]}}}]}"#;
        assert_eq!(
            extract(c, "urls", "", "", "", "", true).unwrap(),
            "https://api.example.com/v1/search?q=ada\n"
        );
    }

    #[test]
    fn form_and_graphql_bodies_are_flattened() {
        let c = r#"{"item":[
            {"name":"Upload","request":{"method":"POST","url":"https://x.test/u","body":{"mode":"formdata","formdata":[
                {"key":"title","value":"Report"},{"key":"file","type":"file","src":"/tmp/a.pdf"}]}}},
            {"name":"Query","request":{"method":"POST","url":"https://x.test/gql","body":{"mode":"graphql","graphql":{
                "query":"{ me { id } }","variables":"{\"a\":1}"}}}},
            {"name":"Form","request":{"method":"POST","url":"https://x.test/f","body":{"mode":"urlencoded","urlencoded":[
                {"key":"a","value":"1"},{"key":"b","value":"2","disabled":true}]}}}
        ]}"#;
        let v: Value = serde_json::from_str(&extract(c, "json", "", "", "", "", true).unwrap()).unwrap();
        assert_eq!(v[0]["body"], "title=Report\nfile=@/tmp/a.pdf");
        assert_eq!(v[1]["body"], "{ me { id } }\n\nvariables: {\"a\":1}");
        assert_eq!(v[2]["body"], "a=1");
        assert_eq!(v[2]["body_mode"], "urlencoded");
    }

    #[test]
    fn long_bodies_are_truncated() {
        let big = "x".repeat(MAX_BODY_CHARS + 50);
        let c = format!(
            r#"{{"item":[{{"name":"Big","request":{{"method":"POST","url":"https://x.test/b","body":{{"mode":"raw","raw":"{big}"}}}}}}]}}"#
        );
        let v: Value = serde_json::from_str(&extract(&c, "json", "", "", "", "", true).unwrap()).unwrap();
        let body = v[0]["body"].as_str().unwrap();
        assert!(body.ends_with("… (truncated at 2000 characters)"));
        assert_eq!(body.chars().filter(|c| *c == 'x').count(), MAX_BODY_CHARS);
    }

    #[test]
    fn nested_folders_join_with_a_separator() {
        let c = r#"{"item":[{"name":"Admin","item":[{"name":"Users","item":[
            {"name":"Delete","request":{"method":"DELETE","url":"https://x.test/u/1"}}]}]}]}"#;
        let v: Value = serde_json::from_str(&extract(c, "json", "", "", "", "", true).unwrap()).unwrap();
        assert_eq!(v[0]["folder"], "Admin / Users");
    }

    #[test]
    fn errors_are_specific() {
        assert!(extract("", "list", "", "", "", "", true)
            .unwrap_err()
            .contains("collection is empty"));
        assert!(extract("not json", "list", "", "", "", "", true)
            .unwrap_err()
            .contains("not valid JSON"));
        assert!(extract(r#"{"info":{"name":"x"}}"#, "list", "", "", "", "", true)
            .unwrap_err()
            .contains("expected a Postman Collection v2.0/v2.1 export"));
        assert!(extract(r#"{"item":[]}"#, "list", "", "", "", "", true)
            .unwrap_err()
            .contains("no requests found"));
        assert!(extract(COLLECTION, "yaml", "", "", "", "", true)
            .unwrap_err()
            .contains("unknown format 'yaml'"));
        assert!(extract(COLLECTION, "list", "PATCH", "", "", "", true)
            .unwrap_err()
            .contains("no requests match the filters"));
        assert!(extract(COLLECTION, "list", "", "", "", "oops", true)
            .unwrap_err()
            .contains("not KEY=VALUE"));
    }

    #[test]
    fn request_cap_is_enforced() {
        let items: Vec<String> = (0..=MAX_REQUESTS)
            .map(|i| format!(r#"{{"name":"r{i}","request":{{"method":"GET","url":"https://x.test/{i}"}}}}"#))
            .collect();
        let c = format!(r#"{{"item":[{}]}}"#, items.join(","));
        assert!(extract(&c, "list", "", "", "", "", true)
            .unwrap_err()
            .contains("the limit is 500"));
    }
}
