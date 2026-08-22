//! openapi-to-curl core — turn an OpenAPI 3.x / Swagger 2.0 document into a
//! ready-to-run `curl` command for every endpoint in the spec.
//!
//! Pure Rust: parse the document (JSON or YAML) into a `serde_json::Value` with
//! key order preserved, walk `paths` → path item → operation, resolve local
//! `$ref`s, build sample values from `example`/`default`/`enum`/`format`/type,
//! and render one curl command per operation. Output is deterministic — the
//! same document always produces byte-identical output — so the result can be
//! committed, diffed, or pasted straight into a terminal.
//!
//! Nothing is fetched: remote `$ref`s stay unresolved (their sample collapses to
//! `null`) because the block is offline by design.

use serde_json::{Map, Value};

/// How deep a schema is expanded before samples collapse (`max_depth` ceiling).
pub const MAX_DEPTH: u32 = 8;

/// Base URL used when the document declares no server at all.
pub const FALLBACK_BASE_URL: &str = "https://api.example.com";

/// Operation keys inside a path item, in OpenAPI's own order (deterministic).
const METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// What the generated commands send as credentials.
#[derive(Clone, Copy, PartialEq)]
enum AuthMode {
    /// Derive per operation from `security` + the declared security schemes.
    Auto,
    /// Never emit credentials.
    None,
    Bearer,
    Basic,
    ApiKey,
}

/// How the generated commands are laid out.
#[derive(Clone, Copy, PartialEq)]
enum OutputFormat {
    /// A runnable bash script with `BASE_URL`/credential variables.
    Shell,
    /// Bare curl commands, one per operation.
    Commands,
    /// Markdown with a heading + fenced block per operation.
    Markdown,
    /// A JSON array of operation records (machine readable).
    Json,
}

/// A credential resolved for one operation.
#[derive(Clone, PartialEq)]
enum Credential {
    None,
    /// `-H "Authorization: Bearer …"`
    Bearer(String),
    /// `-u "…"`
    Basic(String),
    /// An api key carried in a header, query parameter, or cookie.
    ApiKey {
        name: String,
        location: String,
        value: String,
    },
}

/// One rendered operation, before it is wrapped in an output format.
struct Rendered {
    method: String,
    path: String,
    operation_id: String,
    summary: String,
    tags: Vec<String>,
    deprecated: bool,
    /// Fully qualified URL (used by commands/markdown/json output).
    url_absolute: String,
    /// URL written against `$BASE_URL` when the op uses the shared base.
    url_shell: String,
    /// Already-quoted curl arguments after the URL (headers, body, auth).
    args: Vec<String>,
}

/// Shell variables the script preamble must declare.
#[derive(Default)]
struct VarsUsed {
    base_url: bool,
    token: bool,
    basic: bool,
    api_key: bool,
}

/// Everything the walker needs that is constant for one run.
struct Ctx<'a> {
    root: &'a Map<String, Value>,
    swagger2: bool,
    max_depth: usize,
    include_optional: bool,
    pretty_body: bool,
    auth: AuthMode,
    auth_value: String,
}

/// Generate curl commands for every operation in `spec`.
///
/// * `input_format` — `auto` | `json` | `yaml`
/// * `base_url` — override for the spec's own server URL (blank = use the spec)
/// * `auth` — `auto` | `none` | `bearer` | `basic` | `api_key`
/// * `auth_value` — literal credential (blank = a `$TOKEN`-style placeholder)
/// * `methods` / `tags` / `path_filter` — comma-separated / substring filters
/// * `output_format` — `shell` | `commands` | `markdown` | `json`
#[allow(clippy::too_many_arguments)]
pub fn generate(
    spec: &str,
    input_format: &str,
    base_url: &str,
    auth: &str,
    auth_value: &str,
    methods: &str,
    tags: &str,
    path_filter: &str,
    include_optional: bool,
    output_format: &str,
    multiline: bool,
    pretty_body: bool,
    include_comments: bool,
    max_depth: u32,
) -> Result<String, String> {
    let doc = parse_document(spec, input_format)?;
    let auth_mode = parse_auth(auth)?;
    let format = parse_output_format(output_format)?;
    let wanted_methods = parse_method_filter(methods)?;
    let wanted_tags = parse_csv(tags);
    let depth = parse_depth(max_depth)?;

    let root = doc
        .as_object()
        .ok_or_else(|| "the document is not an object — expected an OpenAPI or Swagger spec at the top level".to_string())?;
    let swagger2 = root
        .get("swagger")
        .and_then(Value::as_str)
        .is_some_and(|v| v.starts_with('2'));

    let paths = root
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "the document has no `paths` object — expected an OpenAPI 3.x or Swagger 2.0 spec"
                .to_string()
        })?;
    if paths.is_empty() {
        return Err("the `paths` object is empty — there are no endpoints to generate".to_string());
    }

    let ctx = Ctx {
        root,
        swagger2,
        max_depth: depth,
        include_optional,
        pretty_body,
        auth: auth_mode,
        auth_value: auth_value.trim().to_string(),
    };

    let global_base = match base_url.trim() {
        "" => spec_base_url(root, swagger2),
        given => given.trim_end_matches('/').to_string(),
    };
    let global_base = if global_base.is_empty() {
        FALLBACK_BASE_URL.to_string()
    } else {
        global_base
    };

    let mut vars = VarsUsed::default();
    let mut ops: Vec<Rendered> = Vec::new();
    let mut total = 0usize;

    for (path, item) in paths {
        let Some(item) = ctx.deref(item).as_object().cloned() else {
            continue;
        };
        let item_params = item.get("parameters").cloned().unwrap_or(Value::Null);
        for method in METHODS {
            let Some(op) = item.get(method).and_then(Value::as_object) else {
                continue;
            };
            total += 1;
            if !wanted_methods.is_empty() && !wanted_methods.iter().any(|m| m == method) {
                continue;
            }
            if !tag_matches(op, &wanted_tags) {
                continue;
            }
            let needle = path_filter.trim().to_ascii_lowercase();
            if !needle.is_empty() && !path.to_ascii_lowercase().contains(&needle) {
                continue;
            }
            let base = ctx
                .operation_base(op, &item)
                .filter(|_| base_url.trim().is_empty())
                .unwrap_or_else(|| global_base.clone());
            ops.push(ctx.render_operation(
                method,
                path,
                op,
                &item_params,
                &base,
                &global_base,
                &mut vars,
            ));
        }
    }

    if ops.is_empty() {
        return Err(format!(
            "no operations matched the filters — the document declares {total} operation(s); \
             relax methods/tags/path_filter or leave them blank"
        ));
    }

    Ok(match format {
        OutputFormat::Shell => {
            render_shell(&ops, &global_base, &vars, multiline, include_comments, &ctx)
        }
        OutputFormat::Commands => render_commands(&ops, multiline, include_comments),
        OutputFormat::Markdown => render_markdown(&ops, &global_base, multiline, &ctx),
        OutputFormat::Json => render_json(&ops, &global_base, multiline)?,
    })
}

// ---------------------------------------------------------------------------
// operation rendering
// ---------------------------------------------------------------------------

impl Ctx<'_> {
    /// Build one curl invocation for a single operation.
    #[allow(clippy::too_many_arguments)]
    fn render_operation(
        &self,
        method: &str,
        path: &str,
        op: &Map<String, Value>,
        item_params: &Value,
        base: &str,
        global_base: &str,
        vars: &mut VarsUsed,
    ) -> Rendered {
        let params = self.merged_parameters(op, item_params);
        let credential = self.credential_for(op);

        // --- path ---------------------------------------------------------
        let mut url_path = path.to_string();
        for p in params.iter().filter(|p| p.location == "path") {
            let sample = if p.value == "string" {
                p.name.clone()
            } else {
                p.value.clone()
            };
            url_path = url_path.replace(&format!("{{{}}}", p.name), &pct(&sample));
        }

        // --- query --------------------------------------------------------
        let mut query: Vec<String> = Vec::new();
        for p in params.iter().filter(|p| p.location == "query") {
            if !p.required && !self.include_optional {
                continue;
            }
            query.push(format!("{}={}", pct(&p.name), pct(&p.value)));
        }
        if let Credential::ApiKey {
            name,
            location,
            value,
        } = &credential
        {
            if location == "query" {
                query.push(format!("{}={}", pct(name), value.clone()));
            }
        }
        let query = if query.is_empty() {
            String::new()
        } else {
            format!("?{}", query.join("&"))
        };

        // --- headers ------------------------------------------------------
        let mut args: Vec<String> = Vec::new();
        let body = self.request_body(op, &params);
        if let Some(accept) = self.accept_header(op) {
            args.push(format!("-H {}", dq(&format!("Accept: {accept}"))));
        }
        if let Some(b) = &body {
            if let Some(ct) = &b.content_type {
                args.push(format!("-H {}", dq(&format!("Content-Type: {ct}"))));
            }
        }
        for p in params.iter().filter(|p| p.location == "header") {
            if !p.required && !self.include_optional {
                continue;
            }
            let lower = p.name.to_ascii_lowercase();
            if lower == "authorization" && credential != Credential::None {
                continue; // the auth argument below already carries this header
            }
            if lower == "accept" || lower == "content-type" {
                continue; // emitted from the spec's content types above
            }
            args.push(format!("-H {}", dq(&format!("{}: {}", p.name, p.value))));
        }
        let cookies: Vec<String> = params
            .iter()
            .filter(|p| p.location == "cookie" && (p.required || self.include_optional))
            .map(|p| format!("{}={}", p.name, p.value))
            .collect();
        if !cookies.is_empty() {
            args.push(format!("-b {}", dq(&cookies.join("; "))));
        }

        // --- auth ---------------------------------------------------------
        match &credential {
            Credential::None => {}
            Credential::Bearer(value) => {
                if value.contains("$TOKEN") {
                    vars.token = true;
                }
                args.push(format!(
                    "-H {}",
                    dq_keep_vars(&format!("Authorization: Bearer {value}"))
                ));
            }
            Credential::Basic(value) => {
                if value.contains("$API_USER") {
                    vars.basic = true;
                }
                args.push(format!("-u {}", dq_keep_vars(value)));
            }
            Credential::ApiKey {
                name,
                location,
                value,
            } => {
                if value.contains("$API_KEY") {
                    vars.api_key = true;
                }
                match location.as_str() {
                    "query" => {}
                    "cookie" => args.push(format!(
                        "-b {}",
                        dq_keep_vars(&format!("{name}={value}"))
                    )),
                    _ => args.push(format!(
                        "-H {}",
                        dq_keep_vars(&format!("{name}: {value}"))
                    )),
                }
            }
        }

        // --- body ---------------------------------------------------------
        if let Some(b) = body {
            args.extend(b.args);
        }

        let url_absolute = format!("{base}{url_path}{query}");
        let url_shell = if base == global_base {
            vars.base_url = true;
            format!("$BASE_URL{url_path}{query}")
        } else {
            url_absolute.clone()
        };

        Rendered {
            method: method.to_ascii_uppercase(),
            path: path.to_string(),
            operation_id: op
                .get("operationId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            summary: op
                .get("summary")
                .or_else(|| op.get("description"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_string(),
            tags: op
                .get("tags")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            deprecated: op
                .get("deprecated")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            url_absolute,
            url_shell,
            args,
        }
    }

    /// Path-item parameters merged with operation parameters (op wins on
    /// `name` + `in`), each resolved to a concrete sample value.
    fn merged_parameters(&self, op: &Map<String, Value>, item_params: &Value) -> Vec<Par> {
        let mut out: Vec<Par> = Vec::new();
        for source in [item_params, op.get("parameters").unwrap_or(&Value::Null)] {
            let Some(list) = source.as_array() else {
                continue;
            };
            for raw in list {
                let resolved = self.deref(raw);
                let Some(p) = resolved.as_object() else {
                    continue;
                };
                let Some(name) = p.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let location = p
                    .get("in")
                    .and_then(Value::as_str)
                    .unwrap_or("query")
                    .to_ascii_lowercase();
                if location == "body" || location == "formdata" {
                    continue; // Swagger 2.0 body/formData are handled as the request body
                }
                let par = Par {
                    name: name.to_string(),
                    required: p
                        .get("required")
                        .and_then(Value::as_bool)
                        .unwrap_or(location == "path"),
                    value: self.parameter_sample(p),
                    location,
                };
                match out
                    .iter()
                    .position(|e| e.name == par.name && e.location == par.location)
                {
                    Some(i) => out[i] = par,
                    None => out.push(par),
                }
            }
        }
        out
    }

    /// A parameter's example value, as the string curl will send.
    fn parameter_sample(&self, p: &Map<String, Value>) -> String {
        if let Some(v) = p.get("example") {
            return scalar_to_string(v);
        }
        if let Some(v) = p
            .get("examples")
            .and_then(Value::as_object)
            .and_then(|m| m.values().next())
            .and_then(|e| e.get("value").or(Some(e)))
        {
            return scalar_to_string(v);
        }
        // Swagger 2.0 puts the type directly on the parameter object.
        let schema = match p.get("schema") {
            Some(s) => s.clone(),
            None => Value::Object(p.clone()),
        };
        scalar_to_string(&self.sample(&schema, 0, &mut Vec::new()))
    }

    /// The `Accept` header value declared for the operation's success response.
    fn accept_header(&self, op: &Map<String, Value>) -> Option<String> {
        if self.swagger2 {
            return op
                .get("produces")
                .or_else(|| self.root.get("produces"))
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        let responses = op.get("responses").and_then(Value::as_object)?;
        let mut codes: Vec<&String> = responses.keys().collect();
        codes.sort();
        let key = codes
            .iter()
            .find(|c| c.starts_with('2'))
            .copied()
            .or_else(|| codes.iter().find(|c| c.as_str() == "default").copied())?;
        let resolved = self.deref(responses.get(key)?);
        resolved
            .get("content")
            .and_then(Value::as_object)
            .and_then(|c| pick_content_type(c))
            .map(|(ct, _)| ct)
    }

    /// The request body arguments (`-d`, `-F`, `--data-urlencode`) plus the
    /// `Content-Type` header value the operation declares.
    fn request_body(&self, op: &Map<String, Value>, params: &[Par]) -> Option<BodyArgs> {
        if self.swagger2 {
            return self.swagger2_body(op, params);
        }
        let rb = self.deref(op.get("requestBody")?);
        let content = rb.get("content")?.as_object()?.clone();
        let (content_type, schema) = pick_content_type(&content)?;
        let schema = self.deref(schema.get("schema").unwrap_or(&Value::Null));
        Some(self.body_for(&content_type, &schema))
    }

    /// Swagger 2.0 carries the body in `parameters` (`in: body` / `in: formData`).
    fn swagger2_body(&self, op: &Map<String, Value>, _params: &[Par]) -> Option<BodyArgs> {
        let list = op.get("parameters")?.as_array()?;
        let resolved: Vec<Value> = list.iter().map(|p| self.deref(p)).collect();
        let consumes = op
            .get("consumes")
            .or_else(|| self.root.get("consumes"))
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str);

        if let Some(body) = resolved
            .iter()
            .find(|p| p.get("in").and_then(Value::as_str) == Some("body"))
        {
            let schema = self.deref(body.get("schema").unwrap_or(&Value::Null));
            return Some(self.body_for(consumes.unwrap_or("application/json"), &schema));
        }

        let form: Vec<&Value> = resolved
            .iter()
            .filter(|p| p.get("in").and_then(Value::as_str) == Some("formData"))
            .collect();
        if form.is_empty() {
            return None;
        }
        let content_type = consumes.unwrap_or("application/x-www-form-urlencoded");
        let mut props = Map::new();
        for p in form {
            let Some(name) = p.get("name").and_then(Value::as_str) else {
                continue;
            };
            if !self.include_optional && !p.get("required").and_then(Value::as_bool).unwrap_or(false)
            {
                continue;
            }
            props.insert(name.to_string(), Value::Object(p.as_object()?.clone()));
        }
        let mut schema = Map::new();
        schema.insert("type".into(), Value::String("object".into()));
        schema.insert("properties".into(), Value::Object(props));
        Some(self.body_for(content_type, &Value::Object(schema)))
    }

    /// Turn a content type + schema into curl arguments.
    fn body_for(&self, content_type: &str, schema: &Value) -> BodyArgs {
        let base_type = content_type
            .split(';')
            .next()
            .unwrap_or(content_type)
            .trim()
            .to_ascii_lowercase();
        let sample = self.sample(schema, 0, &mut Vec::new());

        if base_type.starts_with("multipart/") {
            let mut args = Vec::new();
            if let Some(obj) = sample.as_object() {
                for (k, v) in obj {
                    let value = if is_binary(schema, k) {
                        "@/path/to/file".to_string()
                    } else {
                        scalar_to_string(v)
                    };
                    args.push(format!("-F {}", sq(&format!("{k}={value}"))));
                }
            }
            if args.is_empty() {
                args.push(format!("-F {}", sq("field=value")));
            }
            // curl sets the multipart boundary itself; do not pin Content-Type.
            return BodyArgs {
                content_type: None,
                args,
            };
        }

        if base_type == "application/x-www-form-urlencoded" {
            let mut args = Vec::new();
            if let Some(obj) = sample.as_object() {
                for (k, v) in obj {
                    args.push(format!(
                        "--data-urlencode {}",
                        sq(&format!("{k}={}", scalar_to_string(v)))
                    ));
                }
            }
            if args.is_empty() {
                args.push(format!("--data-urlencode {}", sq("field=value")));
            }
            return BodyArgs {
                content_type: Some(content_type.to_string()),
                args,
            };
        }

        let payload = if base_type.contains("json") {
            if self.pretty_body {
                serde_json::to_string_pretty(&sample).unwrap_or_else(|_| "{}".into())
            } else {
                serde_json::to_string(&sample).unwrap_or_else(|_| "{}".into())
            }
        } else {
            scalar_to_string(&sample)
        };
        BodyArgs {
            content_type: Some(content_type.to_string()),
            args: vec![format!("-d {}", sq(&payload))],
        }
    }

    /// The credential this operation should send, honouring the `auth` mode,
    /// the operation's own `security`, and the document's security schemes.
    fn credential_for(&self, op: &Map<String, Value>) -> Credential {
        if self.auth == AuthMode::None {
            return Credential::None;
        }
        let schemes = self
            .root
            .get("components")
            .and_then(Value::as_object)
            .and_then(|c| c.get("securitySchemes"))
            .or_else(|| self.root.get("securityDefinitions"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        let requirement = op
            .get("security")
            .or_else(|| self.root.get("security"))
            .and_then(Value::as_array);

        // Explicit modes ignore `security` but still borrow the declared api key
        // name/location when the document has one.
        let named: Option<Map<String, Value>> = match self.auth {
            AuthMode::Auto => requirement
                .and_then(|reqs| reqs.iter().find_map(|r| r.as_object()?.keys().next().cloned()))
                .and_then(|name| schemes.get(&name).cloned())
                .and_then(|s| self.deref(&s).as_object().cloned()),
            _ => None,
        };

        match self.auth {
            AuthMode::Auto => {
                let Some(reqs) = requirement else {
                    return Credential::None;
                };
                if reqs.iter().all(|r| r.as_object().is_none_or(Map::is_empty)) {
                    return Credential::None; // `security: []` — explicitly public
                }
                match named {
                    Some(scheme) => self.credential_from_scheme(&scheme),
                    None => Credential::Bearer(self.placeholder("$TOKEN")),
                }
            }
            AuthMode::Bearer => Credential::Bearer(self.placeholder("$TOKEN")),
            AuthMode::Basic => Credential::Basic(self.placeholder("$API_USER:$API_PASSWORD")),
            AuthMode::ApiKey => {
                let declared = schemes
                    .values()
                    .map(|s| self.deref(s))
                    .find(|s| s.get("type").and_then(Value::as_str) == Some("apiKey"));
                let (name, location) = declared
                    .as_ref()
                    .map(|s| {
                        (
                            s.get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("X-API-Key")
                                .to_string(),
                            s.get("in")
                                .and_then(Value::as_str)
                                .unwrap_or("header")
                                .to_string(),
                        )
                    })
                    .unwrap_or_else(|| ("X-API-Key".to_string(), "header".to_string()));
                Credential::ApiKey {
                    name,
                    location,
                    value: self.placeholder("$API_KEY"),
                }
            }
            AuthMode::None => Credential::None,
        }
    }

    /// Map one security scheme object onto a curl credential.
    fn credential_from_scheme(&self, scheme: &Map<String, Value>) -> Credential {
        let ty = scheme
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let http_scheme = scheme
            .get("scheme")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        match (ty.as_str(), http_scheme.as_str()) {
            ("http", "basic") | ("basic", _) => {
                Credential::Basic(self.placeholder("$API_USER:$API_PASSWORD"))
            }
            ("apikey", _) => Credential::ApiKey {
                name: scheme
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("X-API-Key")
                    .to_string(),
                location: scheme
                    .get("in")
                    .and_then(Value::as_str)
                    .unwrap_or("header")
                    .to_ascii_lowercase(),
                value: self.placeholder("$API_KEY"),
            },
            _ => Credential::Bearer(self.placeholder("$TOKEN")),
        }
    }

    /// The literal credential when one was supplied, else the shell placeholder.
    fn placeholder(&self, default: &str) -> String {
        if self.auth_value.is_empty() {
            default.to_string()
        } else {
            self.auth_value.clone()
        }
    }

    /// The server URL declared on the operation or its path item (3.x only).
    fn operation_base(&self, op: &Map<String, Value>, item: &Map<String, Value>) -> Option<String> {
        let servers = op.get("servers").or_else(|| item.get("servers"))?;
        let first = servers.as_array()?.first()?;
        let url = server_url(first)?;
        Some(url.trim_end_matches('/').to_string())
    }

    /// Resolve a local `$ref` (one hop is enough for well-formed documents;
    /// chained refs resolve iteratively). Remote refs are returned as-is.
    fn deref(&self, v: &Value) -> Value {
        let mut current = v.clone();
        for _ in 0..8 {
            let Some(pointer) = current.get("$ref").and_then(Value::as_str) else {
                return current;
            };
            let Some(target) = self.lookup(pointer) else {
                return Value::Null;
            };
            current = target;
        }
        current
    }

    /// Follow a `#/a/b/c` JSON pointer inside this document.
    fn lookup(&self, pointer: &str) -> Option<Value> {
        let rest = pointer.strip_prefix("#/")?;
        let mut node = Value::Object(self.root.clone());
        for raw in rest.split('/') {
            let key = raw.replace("~1", "/").replace("~0", "~");
            node = node.get(&key)?.clone();
        }
        Some(node)
    }

    /// Build a sample JSON value for a schema, guarding depth and `$ref` cycles.
    fn sample(&self, schema: &Value, depth: usize, seen: &mut Vec<String>) -> Value {
        if depth > self.max_depth {
            return Value::Null;
        }
        if let Some(pointer) = schema.get("$ref").and_then(Value::as_str) {
            if seen.iter().any(|s| s == pointer) {
                return Value::Null; // recursive schema — stop here
            }
            let Some(target) = self.lookup(pointer) else {
                return Value::Null; // remote/unknown ref: nothing to expand offline
            };
            seen.push(pointer.to_string());
            let out = self.sample(&target, depth, seen);
            seen.pop();
            return out;
        }
        let Some(obj) = schema.as_object() else {
            return Value::String("string".into());
        };
        if let Some(v) = obj.get("example").or_else(|| obj.get("default")) {
            return v.clone();
        }
        if let Some(v) = obj.get("const") {
            return v.clone();
        }
        if let Some(first) = obj.get("enum").and_then(Value::as_array).and_then(|a| a.first()) {
            return first.clone();
        }
        if let Some(all) = obj.get("allOf").and_then(Value::as_array) {
            let mut merged = Map::new();
            for part in all {
                if let Some(m) = self.sample(part, depth, seen).as_object() {
                    for (k, v) in m {
                        merged.insert(k.clone(), v.clone());
                    }
                }
            }
            return Value::Object(merged);
        }
        for key in ["oneOf", "anyOf"] {
            if let Some(first) = obj.get(key).and_then(Value::as_array).and_then(|a| a.first()) {
                return self.sample(first, depth, seen);
            }
        }

        let ty = obj
            .get("type")
            .and_then(|t| match t {
                Value::String(s) => Some(s.clone()),
                Value::Array(a) => a.first().and_then(Value::as_str).map(str::to_string),
                _ => None,
            })
            .unwrap_or_else(|| {
                if obj.contains_key("properties") {
                    "object".into()
                } else if obj.contains_key("items") {
                    "array".into()
                } else {
                    "string".into()
                }
            });

        match ty.as_str() {
            "object" => {
                let mut out = Map::new();
                let required: Vec<&str> = obj
                    .get("required")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                if let Some(props) = obj.get("properties").and_then(Value::as_object) {
                    for (name, sub) in props {
                        let is_required = required.is_empty() || required.contains(&name.as_str());
                        if !is_required && !self.include_optional {
                            continue;
                        }
                        if sub.get("readOnly").and_then(Value::as_bool) == Some(true) {
                            continue; // servers ignore read-only fields on write
                        }
                        out.insert(name.clone(), self.sample(sub, depth + 1, seen));
                    }
                }
                if out.is_empty() {
                    if let Some(extra) = obj.get("additionalProperties") {
                        if extra.is_object() {
                            out.insert("key".into(), self.sample(extra, depth + 1, seen));
                        }
                    }
                }
                Value::Object(out)
            }
            "array" => {
                let item = obj.get("items").cloned().unwrap_or(Value::Null);
                Value::Array(vec![self.sample(&item, depth + 1, seen)])
            }
            "integer" => obj
                .get("minimum")
                .and_then(Value::as_i64)
                .map(|n| Value::from(n))
                .unwrap_or_else(|| Value::from(0)),
            "number" => obj
                .get("minimum")
                .and_then(Value::as_f64)
                .map(Value::from)
                .unwrap_or_else(|| Value::from(0.0)),
            "boolean" => Value::Bool(true),
            "null" => Value::Null,
            _ => Value::String(string_sample(obj)),
        }
    }
}

/// One resolved path/query/header/cookie parameter.
struct Par {
    name: String,
    location: String,
    required: bool,
    value: String,
}

/// The curl arguments (and content type) that carry a request body.
struct BodyArgs {
    content_type: Option<String>,
    args: Vec<String>,
}

// ---------------------------------------------------------------------------
// output formats
// ---------------------------------------------------------------------------

/// Assemble a curl invocation from a URL expression and its arguments.
fn command(url: &str, method: &str, args: &[String], multiline: bool) -> String {
    let mut parts = vec![format!("curl -X {method}"), dq_keep_vars(url)];
    parts.extend(args.iter().cloned());
    if multiline {
        parts.join(" \\\n  ")
    } else {
        parts.join(" ")
    }
}

fn render_shell(
    ops: &[Rendered],
    base: &str,
    vars: &VarsUsed,
    multiline: bool,
    comments: bool,
    ctx: &Ctx,
) -> String {
    let mut out = String::from("#!/usr/bin/env bash\n");
    if comments {
        out.push_str(&format!("# curl examples for {}\n", ctx.title()));
        out.push_str("# Generated from an OpenAPI/Swagger document. Values are samples — edit before sending.\n");
    }
    out.push_str("set -euo pipefail\n\n");
    if vars.base_url {
        out.push_str(&format!("BASE_URL=\"${{BASE_URL:-{base}}}\"\n"));
    }
    if vars.token {
        out.push_str("TOKEN=\"${TOKEN:-replace-with-your-token}\"\n");
    }
    if vars.basic {
        out.push_str("API_USER=\"${API_USER:-user}\"\nAPI_PASSWORD=\"${API_PASSWORD:-password}\"\n");
    }
    if vars.api_key {
        out.push_str("API_KEY=\"${API_KEY:-replace-with-your-api-key}\"\n");
    }
    if vars.base_url || vars.token || vars.basic || vars.api_key {
        out.push('\n');
    }
    for op in ops {
        if comments {
            out.push_str(&format!("# {} {}", op.method, op.path));
            if !op.summary.is_empty() {
                out.push_str(&format!(" — {}", op.summary));
            }
            out.push('\n');
            if op.deprecated {
                out.push_str("# deprecated\n");
            }
        }
        out.push_str(&command(&op.url_shell, &op.method, &op.args, multiline));
        out.push_str("\n\n");
    }
    out.trim_end().to_string() + "\n"
}

fn render_commands(ops: &[Rendered], multiline: bool, comments: bool) -> String {
    let mut out = String::new();
    for op in ops {
        if comments {
            out.push_str(&format!("# {} {}", op.method, op.path));
            if !op.summary.is_empty() {
                out.push_str(&format!(" — {}", op.summary));
            }
            out.push('\n');
        }
        out.push_str(&command(&op.url_absolute, &op.method, &op.args, multiline));
        out.push_str("\n\n");
    }
    out.trim_end().to_string() + "\n"
}

fn render_markdown(ops: &[Rendered], base: &str, multiline: bool, ctx: &Ctx) -> String {
    let mut out = format!("# {} — curl examples\n\n", ctx.title());
    out.push_str(&format!("Base URL: `{base}`\n\n"));
    for op in ops {
        out.push_str(&format!("## {} {}\n\n", op.method, op.path));
        if !op.summary.is_empty() {
            out.push_str(&format!("{}\n\n", op.summary));
        }
        if op.deprecated {
            out.push_str("**Deprecated.**\n\n");
        }
        if !op.tags.is_empty() {
            out.push_str(&format!("Tags: {}\n\n", op.tags.join(", ")));
        }
        out.push_str("```bash\n");
        out.push_str(&command(&op.url_absolute, &op.method, &op.args, multiline));
        out.push_str("\n```\n\n");
    }
    out.trim_end().to_string() + "\n"
}

fn render_json(ops: &[Rendered], base: &str, multiline: bool) -> Result<String, String> {
    let list: Vec<Value> = ops
        .iter()
        .map(|op| {
            let mut m = Map::new();
            m.insert("operationId".into(), Value::String(op.operation_id.clone()));
            m.insert("method".into(), Value::String(op.method.clone()));
            m.insert("path".into(), Value::String(op.path.clone()));
            m.insert("url".into(), Value::String(op.url_absolute.clone()));
            m.insert("summary".into(), Value::String(op.summary.clone()));
            m.insert(
                "tags".into(),
                Value::Array(op.tags.iter().cloned().map(Value::String).collect()),
            );
            m.insert("deprecated".into(), Value::Bool(op.deprecated));
            m.insert(
                "curl".into(),
                Value::String(command(&op.url_absolute, &op.method, &op.args, multiline)),
            );
            Value::Object(m)
        })
        .collect();
    let mut root = Map::new();
    root.insert("baseUrl".into(), Value::String(base.to_string()));
    root.insert("count".into(), Value::from(list.len()));
    root.insert("operations".into(), Value::Array(list));
    serde_json::to_string_pretty(&Value::Object(root))
        .map(|s| s + "\n")
        .map_err(|e| format!("could not serialize the operations as JSON: {e}"))
}

// ---------------------------------------------------------------------------
// input parsing + small helpers
// ---------------------------------------------------------------------------

impl Ctx<'_> {
    /// `info.title` (+ version) for output headers, with a neutral fallback.
    fn title(&self) -> String {
        let info = self.root.get("info").and_then(Value::as_object);
        let title = info
            .and_then(|i| i.get("title"))
            .and_then(Value::as_str)
            .unwrap_or("the API")
            .trim();
        let version = info
            .and_then(|i| i.get("version"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if version.is_empty() {
            title.to_string()
        } else {
            format!("{title} {version}")
        }
    }
}

fn parse_document(spec: &str, input_format: &str) -> Result<Value, String> {
    if spec.trim().is_empty() {
        return Err(
            "spec is empty — paste an OpenAPI 3.x or Swagger 2.0 document (JSON or YAML)"
                .to_string(),
        );
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

fn parse_auth(v: &str) -> Result<AuthMode, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(AuthMode::Auto),
        "none" => Ok(AuthMode::None),
        "bearer" => Ok(AuthMode::Bearer),
        "basic" => Ok(AuthMode::Basic),
        "api_key" | "apikey" => Ok(AuthMode::ApiKey),
        other => Err(format!(
            "unknown auth `{other}` — use auto, none, bearer, basic, or api_key"
        )),
    }
}

fn parse_output_format(v: &str) -> Result<OutputFormat, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "shell" => Ok(OutputFormat::Shell),
        "commands" => Ok(OutputFormat::Commands),
        "markdown" => Ok(OutputFormat::Markdown),
        "json" => Ok(OutputFormat::Json),
        other => Err(format!(
            "unknown output_format `{other}` — use shell, commands, markdown, or json"
        )),
    }
}

fn parse_depth(max_depth: u32) -> Result<usize, String> {
    if max_depth == 0 || max_depth > MAX_DEPTH {
        return Err(format!(
            "max_depth must be between 1 and {MAX_DEPTH} (got {max_depth})"
        ));
    }
    Ok(max_depth as usize)
}

fn parse_method_filter(methods: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for raw in methods.split(',') {
        let m = raw.trim().to_ascii_lowercase();
        if m.is_empty() {
            continue;
        }
        if !METHODS.contains(&m.as_str()) {
            return Err(format!(
                "unknown HTTP method `{}` in methods — use any of {}",
                raw.trim(),
                METHODS.join(", ")
            ));
        }
        out.push(m);
    }
    Ok(out)
}

fn parse_csv(v: &str) -> Vec<String> {
    v.split(',')
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

/// A server entry's URL, with `{variable}` placeholders filled from defaults.
fn server_url(server: &Value) -> Option<String> {
    let url = server.get("url").and_then(Value::as_str)?;
    let mut out = url.to_string();
    if let Some(vars) = server.get("variables").and_then(Value::as_object) {
        for (name, spec) in vars {
            let value = spec
                .get("default")
                .map(scalar_to_string)
                .or_else(|| {
                    spec.get("enum")
                        .and_then(Value::as_array)
                        .and_then(|a| a.first())
                        .map(scalar_to_string)
                })
                .unwrap_or_else(|| name.clone());
            out = out.replace(&format!("{{{name}}}"), &value);
        }
    }
    Some(out)
}

/// The base URL the document itself declares.
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
    root.get("servers")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(server_url)
        .map(|u| u.trim_end_matches('/').to_string())
        .unwrap_or_default()
}

/// Prefer JSON content types, then anything else, deterministically.
fn pick_content_type(content: &Map<String, Value>) -> Option<(String, Value)> {
    if let Some(v) = content.get("application/json") {
        return Some(("application/json".to_string(), v.clone()));
    }
    if let Some((k, v)) = content.iter().find(|(k, _)| k.contains("json")) {
        return Some((k.clone(), v.clone()));
    }
    content.iter().next().map(|(k, v)| (k.clone(), v.clone()))
}

/// True when a body property is declared as a binary upload.
fn is_binary(schema: &Value, property: &str) -> bool {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|p| p.get(property))
        .map(|p| {
            p.get("format").and_then(Value::as_str) == Some("binary")
                || p.get("type").and_then(Value::as_str) == Some("file")
        })
        .unwrap_or(false)
}

/// A readable sample for a `type: string` schema, honouring `format`.
fn string_sample(obj: &Map<String, Value>) -> String {
    match obj
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "date-time" => "2026-01-01T00:00:00Z".into(),
        "date" => "2026-01-01".into(),
        "time" => "12:00:00".into(),
        "email" => "user@example.com".into(),
        "uuid" => "00000000-0000-0000-0000-000000000000".into(),
        "uri" | "url" | "uri-reference" => "https://example.com".into(),
        "hostname" => "example.com".into(),
        "ipv4" => "192.0.2.1".into(),
        "ipv6" => "2001:db8::1".into(),
        "byte" => "c3RyaW5n".into(),
        "binary" | "file" => "@/path/to/file".into(),
        "password" => "s3cr3t".into(),
        _ => "string".into(),
    }
}

/// Render a JSON value the way it travels on the wire (URL/header/form value).
fn scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => a
            .iter()
            .map(scalar_to_string)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => serde_json::to_string(v).unwrap_or_default(),
    }
}

/// Percent-encode everything outside the unreserved set.
fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Double-quote for the shell, escaping expansion entirely.
fn dq(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' | '\\' | '$' | '`' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Double-quote but leave `$VAR` expansion intact (used for URLs and the
/// credential arguments this tool builds itself).
fn dq_keep_vars(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' | '\\' | '`' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Single-quote for the shell (bodies keep `$` literal).
fn sq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const PET_SPEC: &str = r##"{
      "openapi": "3.1.0",
      "info": { "title": "Pet API", "version": "1.0.0" },
      "servers": [{ "url": "https://api.example.com/v1" }],
      "paths": {
        "/pets/{petId}": {
          "get": {
            "operationId": "getPet",
            "tags": ["pets"],
            "summary": "Get a pet",
            "parameters": [
              { "name": "petId", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "expand", "in": "query", "schema": { "type": "string" } }
            ],
            "responses": { "200": { "description": "OK", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } } } }
          }
        },
        "/pets": {
          "post": {
            "operationId": "createPet",
            "tags": ["pets"],
            "requestBody": {
              "required": true,
              "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } }
            },
            "responses": { "201": { "description": "Created" } }
          }
        }
      },
      "components": {
        "schemas": {
          "Pet": {
            "type": "object",
            "required": ["name"],
            "properties": {
              "id": { "type": "integer" },
              "name": { "type": "string", "example": "Rex" },
              "tag": { "type": "string" }
            }
          }
        }
      }
    }"##;

    fn gen(spec: &str) -> String {
        generate(
            spec, "auto", "", "auto", "", "", "", "", false, "shell", true, false, true, 4,
        )
        .unwrap()
    }

    #[test]
    fn generates_a_command_per_operation_with_sample_values() {
        let out = gen(PET_SPEC);
        assert!(out.starts_with("#!/usr/bin/env bash\n"), "{out}");
        assert!(out.contains("BASE_URL=\"${BASE_URL:-https://api.example.com/v1}\""), "{out}");
        assert!(out.contains("curl -X GET \\\n  \"$BASE_URL/pets/petId\""), "{out}");
        assert!(out.contains("curl -X POST \\\n  \"$BASE_URL/pets\""), "{out}");
        assert!(out.contains("-H \"Content-Type: application/json\""), "{out}");
        assert!(out.contains(r##"-d '{"name":"Rex"}'"##), "{out}");
        assert!(out.contains("# GET /pets/{petId} — Get a pet"), "{out}");
        // an optional query parameter stays out unless asked for
        assert!(!out.contains("expand="), "{out}");
    }

    #[test]
    fn include_optional_adds_optional_query_params_and_body_fields() {
        let out = generate(
            PET_SPEC, "auto", "", "auto", "", "", "", "", true, "shell", true, false, true, 4,
        )
        .unwrap();
        assert!(out.contains("/pets/petId?expand=string"), "{out}");
        assert!(out.contains(r##"-d '{"id":0,"name":"Rex","tag":"string"}'"##), "{out}");
    }

    #[test]
    fn base_url_override_and_single_line_output() {
        let out = generate(
            PET_SPEC,
            "auto",
            "http://localhost:8080/",
            "none",
            "",
            "get",
            "",
            "",
            false,
            "commands",
            false,
            false,
            false,
            4,
        )
        .unwrap();
        assert_eq!(
            out,
            "curl -X GET \"http://localhost:8080/pets/petId\" -H \"Accept: application/json\"\n"
        );
    }

    #[test]
    fn yaml_input_with_bearer_auth_from_security_schemes() {
        let spec = r##"
openapi: 3.0.3
info:
  title: Secure API
  version: "2"
servers:
  - url: https://api.example.com
security:
  - bearerAuth: []
paths:
  /me:
    get:
      operationId: getMe
      responses:
        "200":
          description: OK
components:
  securitySchemes:
    bearerAuth:
      type: http
      scheme: bearer
"##;
        let out = gen(spec);
        assert!(out.contains("TOKEN=\"${TOKEN:-replace-with-your-token}\""), "{out}");
        assert!(out.contains("-H \"Authorization: Bearer $TOKEN\""), "{out}");
    }

    #[test]
    fn api_key_scheme_lands_in_the_declared_header_or_query() {
        let spec = r##"{
          "openapi":"3.0.0",
          "servers":[{"url":"https://api.example.com"}],
          "security":[{"apiKey":[]}],
          "paths":{"/things":{"get":{"responses":{"200":{"description":"OK"}}}}},
          "components":{"securitySchemes":{"apiKey":{"type":"apiKey","name":"api_key","in":"query"}}}
        }"##;
        let out = gen(spec);
        assert!(out.contains("$BASE_URL/things?api_key=$API_KEY"), "{out}");
        assert!(out.contains("API_KEY=\"${API_KEY:-replace-with-your-api-key}\""), "{out}");
    }

    #[test]
    fn explicit_auth_value_replaces_the_placeholder() {
        let out = generate(
            PET_SPEC, "auto", "", "bearer", "abc123", "get", "", "", false, "commands", false,
            false, false, 4,
        )
        .unwrap();
        assert!(out.contains("-H \"Authorization: Bearer abc123\""), "{out}");
        assert!(!out.contains("$TOKEN"), "{out}");
    }

    #[test]
    fn swagger2_body_and_base_url() {
        let spec = r##"{
          "swagger":"2.0",
          "host":"legacy.example.com",
          "basePath":"/api",
          "schemes":["https"],
          "consumes":["application/json"],
          "paths":{"/users":{"post":{"operationId":"createUser","parameters":[
            {"name":"body","in":"body","required":true,"schema":{"$ref":"#/definitions/User"}}
          ],"responses":{"201":{"description":"Created"}}}}},
          "definitions":{"User":{"type":"object","required":["email"],"properties":{"email":{"type":"string","format":"email"}}}}
        }"##;
        let out = gen(spec);
        assert!(out.contains("BASE_URL=\"${BASE_URL:-https://legacy.example.com/api}\""), "{out}");
        assert!(out.contains(r##"-d '{"email":"user@example.com"}'"##), "{out}");
    }

    #[test]
    fn multipart_and_form_bodies_use_curl_form_flags() {
        let spec = r##"{
          "openapi":"3.0.0",
          "servers":[{"url":"https://api.example.com"}],
          "paths":{"/upload":{"post":{"requestBody":{"content":{"multipart/form-data":{"schema":{
            "type":"object","required":["file","title"],
            "properties":{"file":{"type":"string","format":"binary"},"title":{"type":"string","example":"report"}}}}}},
            "responses":{"200":{"description":"OK"}}}},
          "/login":{"post":{"requestBody":{"content":{"application/x-www-form-urlencoded":{"schema":{
            "type":"object","required":["user"],"properties":{"user":{"type":"string","example":"ada"}}}}}},
            "responses":{"200":{"description":"OK"}}}}}
        }"##;
        let out = gen(spec);
        assert!(out.contains("-F 'file=@/path/to/file'"), "{out}");
        assert!(out.contains("-F 'title=report'"), "{out}");
        assert!(!out.contains("Content-Type: multipart/form-data"), "{out}");
        assert!(out.contains("--data-urlencode 'user=ada'"), "{out}");
    }

    #[test]
    fn markdown_and_json_output_formats() {
        let md = generate(
            PET_SPEC, "auto", "", "auto", "", "get", "", "", false, "markdown", false, false, true,
            4,
        )
        .unwrap();
        assert!(md.starts_with("# Pet API 1.0.0 — curl examples\n"), "{md}");
        assert!(md.contains("## GET /pets/{petId}"), "{md}");
        assert!(md.contains("```bash\ncurl -X GET \"https://api.example.com/v1/pets/petId\""), "{md}");

        let js = generate(
            PET_SPEC, "auto", "", "auto", "", "get", "", "", false, "json", false, false, true, 4,
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&js).unwrap();
        assert_eq!(parsed["count"], Value::from(1));
        assert_eq!(parsed["operations"][0]["operationId"], Value::from("getPet"));
        assert_eq!(parsed["operations"][0]["method"], Value::from("GET"));
        assert!(parsed["operations"][0]["curl"]
            .as_str()
            .unwrap()
            .contains("https://api.example.com/v1/pets/petId"));
    }

    #[test]
    fn recursive_refs_and_depth_cap_terminate() {
        let spec = r##"{
          "openapi":"3.0.0",
          "servers":[{"url":"https://api.example.com"}],
          "paths":{"/nodes":{"post":{"requestBody":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Node"}}}},"responses":{"200":{"description":"OK"}}}}},
          "components":{"schemas":{"Node":{"type":"object","required":["child"],"properties":{"child":{"$ref":"#/components/schemas/Node"}}}}}
        }"##;
        // The cycle guard expands each `$ref` once and then stops: no runaway
        // recursion, and the output is stable.
        let out = gen(spec);
        assert!(out.contains(r##"-d '{"child":{"child":null}}'"##), "{out}");
    }

    #[test]
    fn pretty_body_expands_json_across_lines() {
        let out = generate(
            PET_SPEC, "auto", "", "none", "", "post", "", "", false, "commands", false, true,
            false, 4,
        )
        .unwrap();
        assert!(out.contains("-d '{\n  \"name\": \"Rex\"\n}'"), "{out}");
    }

    #[test]
    fn filters_narrow_the_output() {
        let out = generate(
            PET_SPEC, "auto", "", "none", "", "", "pets", "/pets/", false, "commands", false,
            false, false, 4,
        )
        .unwrap();
        assert_eq!(out.lines().filter(|l| l.starts_with("curl")).count(), 1);
        assert!(out.contains("/pets/petId"), "{out}");
    }

    #[test]
    fn empty_spec_is_rejected() {
        let err = generate(
            "   ", "auto", "", "auto", "", "", "", "", false, "shell", true, false, true, 4,
        )
        .unwrap_err();
        assert!(err.contains("spec is empty"), "{err}");
    }

    #[test]
    fn document_without_paths_is_rejected() {
        let err = generate(
            r##"{"openapi":"3.0.0"}"##, "auto", "", "auto", "", "", "", "", false, "shell", true,
            false, true, 4,
        )
        .unwrap_err();
        assert!(err.contains("no `paths` object"), "{err}");
    }

    #[test]
    fn unknown_options_are_rejected_with_the_allowed_values() {
        let bad_format = generate(
            PET_SPEC, "auto", "", "auto", "", "", "", "", false, "curlrc", true, false, true, 4,
        )
        .unwrap_err();
        assert!(bad_format.contains("shell, commands, markdown, or json"), "{bad_format}");

        let bad_method = generate(
            PET_SPEC, "auto", "", "auto", "", "fetch", "", "", false, "shell", true, false, true, 4,
        )
        .unwrap_err();
        assert!(bad_method.contains("unknown HTTP method `fetch`"), "{bad_method}");

        let bad_auth = generate(
            PET_SPEC, "auto", "", "oauth", "", "", "", "", false, "shell", true, false, true, 4,
        )
        .unwrap_err();
        assert!(bad_auth.contains("auto, none, bearer, basic, or api_key"), "{bad_auth}");

        let bad_depth = generate(
            PET_SPEC, "auto", "", "auto", "", "", "", "", false, "shell", true, false, true, 99,
        )
        .unwrap_err();
        assert!(bad_depth.contains("max_depth must be between 1 and 8"), "{bad_depth}");
    }

    #[test]
    fn filters_that_match_nothing_explain_themselves() {
        let err = generate(
            PET_SPEC, "auto", "", "auto", "", "delete", "", "", false, "shell", true, false, true,
            4,
        )
        .unwrap_err();
        assert!(err.contains("no operations matched the filters"), "{err}");
        assert!(err.contains("2 operation(s)"), "{err}");
    }

    #[test]
    fn shell_metacharacters_in_sample_values_are_quoted_safely() {
        let spec = r##"{
          "openapi":"3.0.0",
          "servers":[{"url":"https://api.example.com"}],
          "paths":{"/echo":{"post":{"requestBody":{"content":{"application/json":{"schema":{
            "type":"object","required":["note"],"properties":{"note":{"type":"string","example":"it's $HOME `now`"}}}}}},
            "responses":{"200":{"description":"OK"}}}}}
        }"##;
        let out = gen(spec);
        assert!(out.contains(r##"-d '{"note":"it'\''s $HOME `now`"}'"##), "{out}");
    }
}
