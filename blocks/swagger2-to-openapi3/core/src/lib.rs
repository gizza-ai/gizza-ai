//! swagger2-to-openapi3 core — upgrade a Swagger 2.0 spec (JSON or YAML) into an
//! equivalent OpenAPI 3.0 document. Pure-Rust: parse the source into a universal
//! `serde_json::Value` (key order preserved), structurally rewrite it to the 3.0
//! shape, then serialize back to JSON or YAML.
//!
//! Scope is a reasonable, deterministic subset of the full swagger2openapi
//! migration — it covers the transforms that the overwhelming majority of specs
//! need (version, info, host/basePath/schemes → servers, definitions →
//! components.schemas, body/formData/query parameters → requestBody + schema-wrapped
//! parameters, securityDefinitions → securitySchemes, consumes/produces → content,
//! response schemas → content, and `$ref` retargeting). Rarer nuances (external
//! `$ref` resolution, shared body-parameter `$ref`s, tsv collectionFormat) are
//! documented as limits rather than silently mis-converted.

use serde_json::{Map, Value};

pub const DEFAULT_TARGET: &str = "3.0.3";
pub const MAX_INDENT: u32 = 10;

/// The four OpenAPI 3.0.x patch versions this tool can target.
pub const TARGET_VERSIONS: [&str; 4] = ["3.0.3", "3.0.2", "3.0.1", "3.0.0"];

/// HTTP method keys that mark an operation inside a Path Item.
const METHODS: [&str; 7] = ["get", "put", "post", "delete", "options", "head", "patch"];

/// Convert a Swagger 2.0 document into an OpenAPI 3.0 document.
///
/// * `input_format` — `"auto"` (default), `"json"`, or `"yaml"`.
/// * `output_format` — `"json"` (default) or `"yaml"`.
/// * `target_version` — an OpenAPI 3.0.x string (default `3.0.3`).
/// * `indent` — JSON indent spaces, `0`–`10` (`0` = minified). Ignored for YAML.
/// * `patch` — when true, fill in the few fields 3.0 requires but 2.0 may omit
///   (a missing `info.title`/`info.version`, missing response descriptions) so the
///   result validates; when false, only structural changes are made.
pub fn convert(
    spec: &str,
    input_format: &str,
    output_format: &str,
    target_version: &str,
    indent: u32,
    patch: bool,
) -> Result<String, String> {
    if spec.trim().is_empty() {
        return Err("no input: paste a Swagger 2.0 spec (JSON or YAML)".into());
    }
    let target = normalize_target(target_version)?;
    let root = parse_input(spec, input_format)?;
    let obj = root
        .as_object()
        .ok_or("the spec must be a JSON/YAML object at the top level")?;

    if obj.contains_key("openapi") {
        return Err(
            "this looks like an OpenAPI 3.x document already (found an 'openapi' field), not Swagger 2.0"
                .into(),
        );
    }
    match obj.get("swagger").and_then(Value::as_str) {
        Some("2.0") => {}
        Some(other) => {
            return Err(format!(
                "unsupported version 'swagger: {other}' — only Swagger '2.0' can be upgraded"
            ))
        }
        None => {
            if !patch {
                return Err(
                    "missing 'swagger: \"2.0\"' — this is not a recognizable Swagger 2.0 spec (enable Patch to convert anyway)"
                        .into(),
                );
            }
        }
    }

    // Global fallbacks for consumes/produces (operations inherit these).
    let global_consumes = string_array(obj.get("consumes"));
    let global_produces = string_array(obj.get("produces"));

    let mut out = Map::new();
    out.insert("openapi".into(), Value::String(target));

    // info — required by 3.0; patch fills the two required sub-fields if absent.
    let mut info = obj
        .get("info")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if patch {
        info.entry("title")
            .or_insert_with(|| Value::String("API".into()));
        info.entry("version")
            .or_insert_with(|| Value::String("1.0.0".into()));
    }
    out.insert("info".into(), Value::Object(info));

    // servers — from host + basePath + schemes.
    if let Some(servers) = build_servers(obj) {
        out.insert("servers".into(), servers);
    }

    // paths — operations rewritten to the 3.0 shape.
    let paths = obj.get("paths").and_then(Value::as_object);
    let paths_out = transform_paths(paths, &global_consumes, &global_produces, patch);
    out.insert("paths".into(), Value::Object(paths_out));

    // components — schemas / parameters / responses / securitySchemes.
    if let Some(components) = build_components(obj, &global_produces, patch) {
        out.insert("components".into(), components);
    }

    // Carry over untouched top-level fields (references stay valid — security
    // scheme names are unchanged; `$ref`s are retargeted below).
    for key in ["security", "tags", "externalDocs"] {
        if let Some(v) = obj.get(key) {
            out.insert(key.into(), v.clone());
        }
    }
    // Preserve vendor extensions (x-*) at the document root.
    for (k, v) in obj {
        if k.starts_with("x-") {
            out.insert(k.clone(), v.clone());
        }
    }

    let mut result = Value::Object(out);
    // Single pass: retarget every `$ref` and apply 3.0 schema fixups everywhere.
    rewrite_refs(&mut result);

    serialize(&result, output_format, indent)
}

fn normalize_target(v: &str) -> Result<String, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(DEFAULT_TARGET.to_string());
    }
    if TARGET_VERSIONS.contains(&t) {
        Ok(t.to_string())
    } else {
        Err(format!(
            "unsupported target version '{t}' — choose one of {}",
            TARGET_VERSIONS.join(", ")
        ))
    }
}

fn parse_input(spec: &str, input_format: &str) -> Result<Value, String> {
    match input_format.trim().to_ascii_lowercase().as_str() {
        "json" => serde_json::from_str(spec).map_err(|e| format!("invalid JSON: {e}")),
        "yaml" | "yml" => serde_yml::from_str(spec).map_err(|e| format!("invalid YAML: {e}")),
        "" | "auto" => {
            // JSON is a subset of YAML; try strict JSON first for precise errors,
            // then fall back to YAML (which also parses JSON).
            if let Ok(v) = serde_json::from_str::<Value>(spec) {
                Ok(v)
            } else {
                serde_yml::from_str(spec)
                    .map_err(|e| format!("could not parse input as JSON or YAML: {e}"))
            }
        }
        other => Err(format!(
            "unknown input format '{other}' (use auto, json, or yaml)"
        )),
    }
}

fn serialize(value: &Value, output_format: &str, indent: u32) -> Result<String, String> {
    match output_format.trim().to_ascii_lowercase().as_str() {
        "" | "json" => {
            let indent = indent.min(MAX_INDENT) as usize;
            if indent == 0 {
                serde_json::to_string(value).map_err(|e| format!("JSON encode: {e}"))
            } else {
                let pad = vec![b' '; indent];
                let mut buf = Vec::new();
                let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
                let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
                use serde::Serialize;
                value
                    .serialize(&mut ser)
                    .map_err(|e| format!("JSON encode: {e}"))?;
                String::from_utf8(buf).map_err(|e| format!("JSON encode: {e}"))
            }
        }
        "yaml" | "yml" => serde_yml::to_string(value).map_err(|e| format!("YAML encode: {e}")),
        other => Err(format!(
            "unknown output format '{other}' (use json or yaml)"
        )),
    }
}

/// Collect a JSON array of strings (e.g. schemes / consumes / produces).
fn string_array(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// host + basePath + schemes → a `servers` array. One entry per scheme (or a
/// single protocol-relative URL when no scheme is given). Returns None when
/// there is neither a host nor a basePath to describe.
fn build_servers(obj: &Map<String, Value>) -> Option<Value> {
    let host = obj.get("host").and_then(Value::as_str).unwrap_or("");
    let base = obj.get("basePath").and_then(Value::as_str).unwrap_or("");
    if host.is_empty() && base.is_empty() {
        return None;
    }
    let schemes = string_array(obj.get("schemes"));
    let mut servers = Vec::new();
    if host.is_empty() {
        // basePath only → a relative server URL.
        servers.push(server_entry(base.to_string()));
    } else if schemes.is_empty() {
        // No scheme declared → protocol-relative URL.
        servers.push(server_entry(format!("//{host}{base}")));
    } else {
        for scheme in &schemes {
            servers.push(server_entry(format!("{scheme}://{host}{base}")));
        }
    }
    Some(Value::Array(servers))
}

fn server_entry(url: String) -> Value {
    let mut m = Map::new();
    m.insert("url".into(), Value::String(url));
    Value::Object(m)
}

fn transform_paths(
    paths: Option<&Map<String, Value>>,
    global_consumes: &[String],
    global_produces: &[String],
    patch: bool,
) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(paths) = paths else { return out };
    for (path, item) in paths {
        // x-* extensions on the paths object itself pass through untouched.
        if path.starts_with("x-") || !item.is_object() {
            out.insert(path.clone(), item.clone());
            continue;
        }
        let item = item.as_object().unwrap();
        let mut new_item = Map::new();
        for (key, val) in item {
            if METHODS.contains(&key.as_str()) {
                if let Some(op) = val.as_object() {
                    new_item.insert(
                        key.clone(),
                        transform_operation(op, global_consumes, global_produces, patch),
                    );
                }
            } else if key == "parameters" {
                // Shared path-level parameters: keep only the non-body ones
                // (3.0 forbids a requestBody at the path level).
                let (params, _body, _form) = split_parameters(val);
                if !params.is_empty() {
                    new_item.insert("parameters".into(), Value::Array(params));
                }
            } else {
                // $ref, summary, description, x-* — pass through.
                new_item.insert(key.clone(), val.clone());
            }
        }
        out.insert(path.clone(), Value::Object(new_item));
    }
    out
}

fn transform_operation(
    op: &Map<String, Value>,
    global_consumes: &[String],
    global_produces: &[String],
    patch: bool,
) -> Value {
    let mut new_op = Map::new();
    let consumes = {
        let local = string_array(op.get("consumes"));
        if local.is_empty() {
            global_consumes.to_vec()
        } else {
            local
        }
    };
    let produces = {
        let local = string_array(op.get("produces"));
        if local.is_empty() {
            global_produces.to_vec()
        } else {
            local
        }
    };

    for (key, val) in op {
        match key.as_str() {
            // consumes/produces are folded into requestBody/response content.
            "consumes" | "produces" | "schemes" => {}
            "parameters" => {
                let (params, body, form) = split_parameters(val);
                if !params.is_empty() {
                    new_op.insert("parameters".into(), Value::Array(params));
                }
                if let Some(b) = body {
                    new_op.insert("requestBody".into(), body_to_request_body(&b, &consumes));
                } else if !form.is_empty() {
                    new_op.insert(
                        "requestBody".into(),
                        formdata_to_request_body(&form, &consumes),
                    );
                }
            }
            "responses" => {
                new_op.insert("responses".into(), transform_responses(val, &produces, patch));
            }
            _ => {
                new_op.insert(key.clone(), val.clone());
            }
        }
    }
    Value::Object(new_op)
}

/// Split a 2.0 `parameters` array into (schema-wrapped non-body params, the lone
/// body param, the formData params).
fn split_parameters(val: &Value) -> (Vec<Value>, Option<Value>, Vec<Value>) {
    let mut params = Vec::new();
    let mut body = None;
    let mut form = Vec::new();
    if let Some(arr) = val.as_array() {
        for p in arr {
            let Some(pm) = p.as_object() else { continue };
            // A pure `$ref` parameter (points at #/parameters/X) passes through;
            // its ref is retargeted globally later.
            if pm.contains_key("$ref") {
                params.push(p.clone());
                continue;
            }
            match pm.get("in").and_then(Value::as_str) {
                Some("body") => body = Some(p.clone()),
                Some("formData") => form.push(p.clone()),
                _ => params.push(transform_parameter(pm)),
            }
        }
    }
    (params, body, form)
}

/// A non-body 2.0 parameter → a 3.0 parameter (its type facets move into `schema`).
fn transform_parameter(pm: &Map<String, Value>) -> Value {
    let mut out = Map::new();
    // Fields that stay on the Parameter Object itself.
    for key in ["name", "in", "description", "required", "deprecated", "allowEmptyValue"] {
        if let Some(v) = pm.get(key) {
            out.insert(key.into(), v.clone());
        }
    }
    // collectionFormat → style/explode (arrays only).
    if pm.get("type").and_then(Value::as_str) == Some("array") {
        if let Some(cf) = pm.get("collectionFormat").and_then(Value::as_str) {
            match cf {
                "csv" => {}
                "ssv" => {
                    out.insert("style".into(), Value::String("spaceDelimited".into()));
                }
                "pipes" => {
                    out.insert("style".into(), Value::String("pipeDelimited".into()));
                }
                "multi" => {
                    out.insert("explode".into(), Value::Bool(true));
                }
                _ => {}
            }
        }
    }
    out.insert("schema".into(), param_to_schema(pm));
    Value::Object(out)
}

/// Build a Schema Object from the type facets carried inline on a 2.0 parameter.
fn param_to_schema(pm: &Map<String, Value>) -> Value {
    const FACETS: [&str; 15] = [
        "type",
        "format",
        "items",
        "enum",
        "default",
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "minLength",
        "maxLength",
        "pattern",
        "minItems",
        "maxItems",
        "multipleOf",
    ];
    let mut schema = Map::new();
    for key in FACETS {
        if let Some(v) = pm.get(key) {
            schema.insert(key.into(), v.clone());
        }
    }
    if let Some(v) = pm.get("uniqueItems") {
        schema.insert("uniqueItems".into(), v.clone());
    }
    Value::Object(schema)
}

fn body_to_request_body(body: &Value, consumes: &[String]) -> Value {
    let bm = body.as_object().cloned().unwrap_or_default();
    let mut rb = Map::new();
    if let Some(d) = bm.get("description") {
        rb.insert("description".into(), d.clone());
    }
    if bm.get("required").and_then(Value::as_bool) == Some(true) {
        rb.insert("required".into(), Value::Bool(true));
    }
    let schema = bm.get("schema").cloned().unwrap_or(Value::Object(Map::new()));
    let media_types = if consumes.is_empty() {
        vec!["application/json".to_string()]
    } else {
        consumes.to_vec()
    };
    let mut content = Map::new();
    for mt in media_types {
        let mut wrap = Map::new();
        wrap.insert("schema".into(), schema.clone());
        content.insert(mt, Value::Object(wrap));
    }
    rb.insert("content".into(), Value::Object(content));
    Value::Object(rb)
}

fn formdata_to_request_body(form: &[Value], consumes: &[String]) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    let mut has_file = false;
    for p in form {
        let Some(pm) = p.as_object() else { continue };
        let name = pm
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        // 2.0 `type: file` → a binary string schema in 3.0.
        let schema = if pm.get("type").and_then(Value::as_str) == Some("file") {
            has_file = true;
            let mut s = Map::new();
            s.insert("type".into(), Value::String("string".into()));
            s.insert("format".into(), Value::String("binary".into()));
            if let Some(d) = pm.get("description") {
                s.insert("description".into(), d.clone());
            }
            Value::Object(s)
        } else {
            let mut s = param_to_schema(pm);
            if let (Some(o), Some(d)) = (s.as_object_mut(), pm.get("description")) {
                o.insert("description".into(), d.clone());
            }
            s
        };
        if pm.get("required").and_then(Value::as_bool) == Some(true) {
            required.push(Value::String(name.clone()));
        }
        properties.insert(name, schema);
    }
    let mut obj_schema = Map::new();
    obj_schema.insert("type".into(), Value::String("object".into()));
    obj_schema.insert("properties".into(), Value::Object(properties));
    if !required.is_empty() {
        obj_schema.insert("required".into(), Value::Array(required));
    }

    // A form with a file part is multipart; otherwise urlencoded, unless the
    // operation declared a specific form consumes media type.
    let media = consumes
        .iter()
        .find(|c| c.contains("multipart") || c.contains("form-urlencoded"))
        .cloned()
        .unwrap_or_else(|| {
            if has_file {
                "multipart/form-data".into()
            } else {
                "application/x-www-form-urlencoded".into()
            }
        });

    let mut wrap = Map::new();
    wrap.insert("schema".into(), Value::Object(obj_schema));
    let mut content = Map::new();
    content.insert(media, Value::Object(wrap));
    let mut rb = Map::new();
    rb.insert("content".into(), Value::Object(content));
    Value::Object(rb)
}

fn transform_responses(val: &Value, produces: &[String], patch: bool) -> Value {
    let mut out = Map::new();
    if let Some(obj) = val.as_object() {
        for (code, resp) in obj {
            if code.starts_with("x-") {
                out.insert(code.clone(), resp.clone());
            } else {
                out.insert(code.clone(), transform_response(resp, produces, patch));
            }
        }
    }
    Value::Object(out)
}

fn transform_response(resp: &Value, produces: &[String], patch: bool) -> Value {
    // A `$ref` response passes through (retargeted globally later).
    let Some(rm) = resp.as_object() else {
        return resp.clone();
    };
    if rm.contains_key("$ref") {
        return resp.clone();
    }
    let mut out = Map::new();
    match rm.get("description") {
        Some(d) => {
            out.insert("description".into(), d.clone());
        }
        None if patch => {
            out.insert("description".into(), Value::String("No description".into()));
        }
        None => {}
    }
    // headers: 2.0 inline-typed → 3.0 header with a `schema`.
    if let Some(headers) = rm.get("headers").and_then(Value::as_object) {
        let mut hout = Map::new();
        for (hname, hval) in headers {
            if let Some(hm) = hval.as_object() {
                let mut h = Map::new();
                if let Some(d) = hm.get("description") {
                    h.insert("description".into(), d.clone());
                }
                h.insert("schema".into(), param_to_schema(hm));
                hout.insert(hname.clone(), Value::Object(h));
            } else {
                hout.insert(hname.clone(), hval.clone());
            }
        }
        out.insert("headers".into(), Value::Object(hout));
    }
    // schema (+ optional examples) → content keyed by the produced media types.
    if let Some(schema) = rm.get("schema") {
        let media_types = if produces.is_empty() {
            vec!["application/json".to_string()]
        } else {
            produces.to_vec()
        };
        let examples = rm.get("examples").and_then(Value::as_object);
        let mut content = Map::new();
        for mt in media_types {
            let mut wrap = Map::new();
            wrap.insert("schema".into(), schema.clone());
            if let Some(ex) = examples.and_then(|e| e.get(&mt)) {
                wrap.insert("example".into(), ex.clone());
            }
            content.insert(mt, Value::Object(wrap));
        }
        out.insert("content".into(), Value::Object(content));
    }
    Value::Object(out)
}

/// Assemble the `components` object from definitions / parameters / responses /
/// securityDefinitions. Returns None when the spec has none of them.
fn build_components(
    obj: &Map<String, Value>,
    global_produces: &[String],
    patch: bool,
) -> Option<Value> {
    let mut components = Map::new();

    if let Some(defs) = obj.get("definitions").and_then(Value::as_object) {
        components.insert("schemas".into(), Value::Object(defs.clone()));
    }

    if let Some(params) = obj.get("parameters").and_then(Value::as_object) {
        let mut pout = Map::new();
        for (name, p) in params {
            if let Some(pm) = p.as_object() {
                // Shared body/formData parameters can't live in components.parameters;
                // keep only the reusable non-body ones (an edge documented as a limit).
                match pm.get("in").and_then(Value::as_str) {
                    Some("body") | Some("formData") => {}
                    _ => {
                        pout.insert(name.clone(), transform_parameter(pm));
                    }
                }
            }
        }
        if !pout.is_empty() {
            components.insert("parameters".into(), Value::Object(pout));
        }
    }

    if let Some(resps) = obj.get("responses").and_then(Value::as_object) {
        let mut rout = Map::new();
        for (name, r) in resps {
            rout.insert(name.clone(), transform_response(r, global_produces, patch));
        }
        components.insert("responses".into(), Value::Object(rout));
    }

    if let Some(defs) = obj.get("securityDefinitions").and_then(Value::as_object) {
        let mut sout = Map::new();
        for (name, def) in defs {
            sout.insert(name.clone(), transform_security_scheme(def));
        }
        components.insert("securitySchemes".into(), Value::Object(sout));
    }

    if components.is_empty() {
        None
    } else {
        Some(Value::Object(components))
    }
}

fn transform_security_scheme(def: &Value) -> Value {
    let Some(dm) = def.as_object() else {
        return def.clone();
    };
    let mut out = Map::new();
    let desc = dm.get("description").cloned();
    match dm.get("type").and_then(Value::as_str) {
        Some("basic") => {
            out.insert("type".into(), Value::String("http".into()));
            out.insert("scheme".into(), Value::String("basic".into()));
            if let Some(d) = desc {
                out.insert("description".into(), d);
            }
        }
        Some("apiKey") => {
            out.insert("type".into(), Value::String("apiKey".into()));
            if let Some(d) = desc {
                out.insert("description".into(), d);
            }
            for key in ["name", "in"] {
                if let Some(v) = dm.get(key) {
                    out.insert(key.into(), v.clone());
                }
            }
        }
        Some("oauth2") => {
            out.insert("type".into(), Value::String("oauth2".into()));
            if let Some(d) = desc {
                out.insert("description".into(), d);
            }
            let flow = dm.get("flow").and_then(Value::as_str).unwrap_or("");
            let (flow_name, urls): (&str, &[&str]) = match flow {
                "implicit" => ("implicit", &["authorizationUrl"]),
                "password" => ("password", &["tokenUrl"]),
                "application" => ("clientCredentials", &["tokenUrl"]),
                "accessCode" => ("authorizationCode", &["authorizationUrl", "tokenUrl"]),
                _ => ("implicit", &["authorizationUrl"]),
            };
            let mut flow_obj = Map::new();
            for url_key in urls {
                if let Some(v) = dm.get(*url_key) {
                    flow_obj.insert((*url_key).into(), v.clone());
                }
            }
            let scopes = dm
                .get("scopes")
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new()));
            flow_obj.insert("scopes".into(), scopes);
            let mut flows = Map::new();
            flows.insert(flow_name.into(), Value::Object(flow_obj));
            out.insert("flows".into(), Value::Object(flows));
        }
        _ => return def.clone(),
    }
    Value::Object(out)
}

/// Recursively (a) retarget every `$ref` from the 2.0 namespaces to the 3.0
/// `components/*` namespaces and (b) apply the small 3.0 schema fixups
/// (discriminator string → object, `x-nullable` → `nullable`).
fn rewrite_refs(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get_mut("$ref") {
                for (from, to) in [
                    ("#/definitions/", "#/components/schemas/"),
                    ("#/parameters/", "#/components/parameters/"),
                    ("#/responses/", "#/components/responses/"),
                ] {
                    if let Some(rest) = r.strip_prefix(from) {
                        *r = format!("{to}{rest}");
                        break;
                    }
                }
            }
            // discriminator: 2.0 uses a bare property-name string; 3.0 uses an object.
            if let Some(Value::String(prop)) = map.get("discriminator") {
                let prop = prop.clone();
                let mut d = Map::new();
                d.insert("propertyName".into(), Value::String(prop));
                map.insert("discriminator".into(), Value::Object(d));
            }
            // x-nullable → nullable (a common 2.0 vendor extension).
            if let Some(v) = map.remove("x-nullable") {
                map.entry("nullable").or_insert(v);
            }
            for (_k, v) in map.iter_mut() {
                rewrite_refs(v);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                rewrite_refs(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn conv(spec: &Value) -> Value {
        let s = serde_json::to_string(spec).unwrap();
        let out = convert(&s, "json", "json", "3.0.3", 2, true).unwrap();
        serde_json::from_str(&out).unwrap()
    }

    fn sample() -> Value {
        json!({
            "swagger": "2.0",
            "info": {"title": "Pets", "version": "1.0.0"},
            "host": "api.example.com",
            "basePath": "/v1",
            "schemes": ["https", "http"],
            "consumes": ["application/json"],
            "produces": ["application/json"],
            "paths": {
                "/pets": {
                    "get": {
                        "parameters": [
                            {"name": "limit", "in": "query", "type": "integer", "format": "int32", "required": false},
                            {"name": "tags", "in": "query", "type": "array", "items": {"type": "string"}, "collectionFormat": "multi"}
                        ],
                        "responses": {
                            "200": {"description": "a list", "schema": {"$ref": "#/definitions/Pet"}}
                        }
                    },
                    "post": {
                        "parameters": [
                            {"name": "body", "in": "body", "required": true, "schema": {"$ref": "#/definitions/Pet"}}
                        ],
                        "responses": {"201": {"description": "created"}}
                    }
                }
            },
            "definitions": {
                "Pet": {"type": "object", "required": ["id"], "properties": {"id": {"type": "integer"}}}
            },
            "securityDefinitions": {
                "auth": {"type": "oauth2", "flow": "accessCode", "authorizationUrl": "https://a", "tokenUrl": "https://t", "scopes": {"read": "r"}},
                "key": {"type": "apiKey", "name": "X-Key", "in": "header"},
                "basic": {"type": "basic"}
            }
        })
    }

    #[test]
    fn happy_full_conversion() {
        let out = conv(&sample());
        assert_eq!(out["openapi"], json!("3.0.3"));
        assert_eq!(
            out["servers"],
            json!([{"url": "https://api.example.com/v1"}, {"url": "http://api.example.com/v1"}])
        );
        assert_eq!(out["components"]["schemas"]["Pet"]["type"], json!("object"));
        assert_eq!(
            out["paths"]["/pets"]["post"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            json!("#/components/schemas/Pet")
        );
        assert_eq!(out["paths"]["/pets"]["post"]["requestBody"]["required"], json!(true));
        let get_params = &out["paths"]["/pets"]["get"]["parameters"];
        assert_eq!(get_params[0]["schema"]["type"], json!("integer"));
        assert_eq!(get_params[0]["schema"]["format"], json!("int32"));
        assert!(get_params[0].get("type").is_none(), "type moved into schema");
        assert_eq!(get_params[1]["explode"], json!(true));
        assert_eq!(
            out["paths"]["/pets"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            json!("#/components/schemas/Pet")
        );
        let ss = &out["components"]["securitySchemes"];
        assert_eq!(ss["basic"], json!({"type": "http", "scheme": "basic"}));
        assert_eq!(ss["key"]["type"], json!("apiKey"));
        assert_eq!(ss["auth"]["flows"]["authorizationCode"]["tokenUrl"], json!("https://t"));
    }

    #[test]
    fn yaml_input_and_output() {
        let yaml = "swagger: '2.0'\ninfo:\n  title: Y\n  version: '1'\npaths: {}\n";
        let out = convert(yaml, "auto", "yaml", "3.0.3", 2, true).unwrap();
        assert!(out.contains("openapi:"), "yaml output has openapi key");
        assert!(out.contains("3.0.3"));
    }

    #[test]
    fn formdata_with_file_becomes_multipart() {
        let spec = json!({
            "swagger": "2.0",
            "info": {"title": "U", "version": "1"},
            "paths": {"/upload": {"post": {
                "consumes": ["multipart/form-data"],
                "parameters": [
                    {"name": "file", "in": "formData", "type": "file", "required": true},
                    {"name": "note", "in": "formData", "type": "string"}
                ],
                "responses": {"200": {"description": "ok"}}
            }}}
        });
        let out = conv(&spec);
        let rb = &out["paths"]["/upload"]["post"]["requestBody"];
        let schema = &rb["content"]["multipart/form-data"]["schema"];
        assert_eq!(schema["properties"]["file"]["format"], json!("binary"));
        assert_eq!(schema["required"], json!(["file"]));
    }

    #[test]
    fn patch_fills_missing_info() {
        let spec = json!({"swagger": "2.0", "paths": {}});
        let out = conv(&spec);
        assert_eq!(out["info"]["title"], json!("API"));
        assert_eq!(out["info"]["version"], json!("1.0.0"));
    }

    #[test]
    fn discriminator_and_nullable_fixups() {
        let spec = json!({
            "swagger": "2.0",
            "info": {"title": "D", "version": "1"},
            "paths": {},
            "definitions": {
                "Cat": {"type": "object", "discriminator": "petType", "properties": {"n": {"type": "string", "x-nullable": true}}}
            }
        });
        let out = conv(&spec);
        assert_eq!(
            out["components"]["schemas"]["Cat"]["discriminator"],
            json!({"propertyName": "petType"})
        );
        assert_eq!(
            out["components"]["schemas"]["Cat"]["properties"]["n"]["nullable"],
            json!(true)
        );
    }

    #[test]
    fn err_empty_input() {
        assert!(convert("", "auto", "json", "3.0.3", 2, true).is_err());
    }

    #[test]
    fn err_already_openapi3() {
        let spec = r#"{"openapi": "3.0.0", "info": {}, "paths": {}}"#;
        let e = convert(spec, "json", "json", "3.0.3", 2, true).unwrap_err();
        assert!(e.contains("already"), "got: {e}");
    }

    #[test]
    fn err_wrong_swagger_version() {
        let spec = r#"{"swagger": "1.2", "paths": {}}"#;
        let e = convert(spec, "json", "json", "3.0.3", 2, true).unwrap_err();
        assert!(e.contains("2.0"), "got: {e}");
    }

    #[test]
    fn err_missing_swagger_without_patch() {
        let spec = r#"{"info": {"title": "x", "version": "1"}, "paths": {}}"#;
        assert!(convert(spec, "json", "json", "3.0.3", 2, false).is_err());
    }

    #[test]
    fn err_bad_target_version() {
        let spec = r#"{"swagger": "2.0", "paths": {}}"#;
        assert!(convert(spec, "json", "json", "3.1.0", 2, true).is_err());
    }

    #[test]
    fn indent_zero_is_minified() {
        let spec = json!({"swagger": "2.0", "info": {"title": "M", "version": "1"}, "paths": {}});
        let s = serde_json::to_string(&spec).unwrap();
        let out = convert(&s, "json", "json", "3.0.3", 0, true).unwrap();
        assert!(!out.contains('\n'), "minified output has no newlines");
    }
}
