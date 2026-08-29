//! gizza-ai/graphql-introspect — run the GraphQL introspection query against a
//! live endpoint and render the result as SDL, a type list, Markdown docs, or
//! the raw introspection JSON.
//!
//! Network block (same family as `web-fetch` / `http-request`): it POSTs (or
//! GETs) the introspection document via `wafer-run/network`, then converts the
//! `data.__schema` payload into schema text locally. No page — the chat and CLI
//! surfaces are the verifiable ones (see `.claude/skills/new-tool/SKILL.md`
//! step 3, "network — treat as a chat-only block").
//!
//! Everything except the single host request is pure: the query builder, the
//! introspection→SDL/Markdown/type-list printers and the error classifier are
//! plain functions compiled (and unit-tested) on the host, exactly like
//! `http-request`'s helpers.

// The #[wafer_block] macro emits wasm-only registration; the host call and the
// `Args` type are only used inside that impl. The pure query-building and
// schema-rendering helpers below are compiled (and unit-tested) on the host.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wafer_sdk::*;

/// Internal cap on each rendered text field (`schema`, `types`). Not a user
/// knob — it exists only so a very large schema can't exhaust the per-call wasm
/// fuel/memory budget while building the report. Output over this is clipped
/// and flagged via the `truncated` field.
const MAX_OUTPUT_BYTES: usize = 1 << 20; // 1 MiB

/// Scalars every GraphQL service defines. Hidden unless `include_builtins`.
const BUILTIN_SCALARS: [&str; 5] = ["Boolean", "Float", "ID", "Int", "String"];
/// Directives defined by the GraphQL spec itself. Hidden unless `include_builtins`.
const BUILTIN_DIRECTIVES: [&str; 4] = ["deprecated", "include", "skip", "specifiedBy"];

/// Output shapes. `sdl` is the default (what every schema-download tool emits);
/// `types` is the flat type list; `markdown` is a docs table per type; `json`
/// is the raw `{ "__schema": … }` object that GraphQL tooling consumes.
const ALLOWED_FORMATS: [&str; 4] = ["sdl", "types", "markdown", "json"];
/// Introspection is a normal query, so both transports are legal; POST is what
/// clients use by default, GET is the fallback for endpoints that only allow it.
const ALLOWED_METHODS: [&str; 2] = ["POST", "GET"];

#[derive(Deserialize)]
struct Args {
    url: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    descriptions: Option<bool>,
    #[serde(default)]
    include_deprecated: Option<bool>,
    #[serde(default)]
    include_builtins: Option<bool>,
    #[serde(default)]
    sort: Option<bool>,
    #[serde(default)]
    specified_by_url: Option<bool>,
    #[serde(default)]
    repeatable_directives: Option<bool>,
}

#[derive(Serialize)]
struct ToolResp {
    /// The method + final URL actually requested, e.g. `"POST https://…/graphql"`.
    request: String,
    /// `"<code> <reason>"`, e.g. `"200 OK"`.
    status: String,
    /// Numeric status code, for programmatic use.
    status_code: u16,
    /// The `format` that produced `schema`.
    format: String,
    /// Root operation type names (absent when the schema doesn't define one).
    query_type: Option<String>,
    mutation_type: Option<String>,
    subscription_type: Option<String>,
    /// Number of types in `types` (after built-in filtering).
    type_count: usize,
    /// One `Name (KIND, N fields)` line per type — always present, whatever the
    /// chosen `format`.
    types: String,
    /// The schema itself, rendered per `format`.
    schema: String,
    /// True when `schema` or `types` was clipped to the internal 1 MiB cap.
    truncated: bool,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
/// `Input::None` — `url` is a normal required string param (an endpoint to call,
/// not a media source), so there is no `url`⊕`ref` `oneOf`. `headers` is a
/// `Param::string_map` (name→value), same shape as http-request's.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("url")
                .required()
                .describe("Absolute http(s) URL of the GraphQL endpoint, e.g. https://countries.trevorblades.com/graphql. Point it at the API endpoint itself, not at an HTML playground page."),
        )
        .param(Param::string_map("headers").describe(
            "Request headers as a name->value map, for endpoints that need auth — e.g. {\"Authorization\": \"Bearer <token>\"}. Content-Type and Accept default to application/json.",
        ))
        .param(
            Param::enumv("method", ALLOWED_METHODS)
                .default("POST")
                .describe("How to send the introspection query: POST with a JSON body (default), or GET with the query in the ?query= parameter for endpoints that only allow GET."),
        )
        .param(
            Param::enumv("format", ALLOWED_FORMATS)
                .default("sdl")
                .describe("Output shape: 'sdl' (default) prints the schema in GraphQL SDL; 'types' prints just the type list; 'markdown' prints per-type documentation tables; 'json' returns the raw {\"__schema\": ...} introspection object for other tooling."),
        )
        .param(
            Param::boolean("descriptions")
                .default(true)
                .describe("Request and print type/field descriptions (default true). Set false for a compact schema."),
        )
        .param(
            Param::boolean("include_deprecated")
                .default(true)
                .describe("Include deprecated fields and enum values, annotated with @deprecated (default true)."),
        )
        .param(
            Param::boolean("include_builtins")
                .default(false)
                .describe("Include built-in scalars (Int, Float, String, Boolean, ID), the __Schema/__Type introspection types, and the spec directives @skip/@include/@deprecated/@specifiedBy (default false)."),
        )
        .param(
            Param::boolean("sort")
                .default(false)
                .describe("Sort types, fields, arguments and enum values alphabetically for stable diffs (default false keeps the server's own order)."),
        )
        .param(
            Param::boolean("specified_by_url")
                .default(false)
                .describe("Ask for each custom scalar's specifiedByURL and print it as @specifiedBy(url: \"...\") (default false — older servers reject this field)."),
        )
        .param(
            Param::boolean("repeatable_directives")
                .default(false)
                .describe("Ask for each directive's isRepeatable flag and print the 'repeatable' keyword (default false — older servers reject this field)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Everything that changes what we ask for and how we print it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Options {
    descriptions: bool,
    include_deprecated: bool,
    include_builtins: bool,
    sort: bool,
    specified_by_url: bool,
    repeatable_directives: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            descriptions: true,
            include_deprecated: true,
            include_builtins: false,
            sort: false,
            specified_by_url: false,
            repeatable_directives: false,
        }
    }
}

impl Options {
    fn from_args(a: &Args) -> Self {
        let d = Options::default();
        Self {
            descriptions: a.descriptions.unwrap_or(d.descriptions),
            include_deprecated: a.include_deprecated.unwrap_or(d.include_deprecated),
            include_builtins: a.include_builtins.unwrap_or(d.include_builtins),
            sort: a.sort.unwrap_or(d.sort),
            specified_by_url: a.specified_by_url.unwrap_or(d.specified_by_url),
            repeatable_directives: a.repeatable_directives.unwrap_or(d.repeatable_directives),
        }
    }
}

/// Validate + normalize the transport. `None` defaults to `POST`.
fn normalize_method(method: Option<&str>) -> Result<String, SkillError> {
    let raw = method.unwrap_or("POST").trim();
    if raw.is_empty() {
        return Ok("POST".to_string());
    }
    let upper = raw.to_ascii_uppercase();
    if ALLOWED_METHODS.contains(&upper.as_str()) {
        Ok(upper)
    } else {
        Err(SkillError::InvalidArgs(format!(
            "invalid graphql-introspect args: unsupported method {raw:?} (allowed: {})",
            ALLOWED_METHODS.join(", ")
        )))
    }
}

/// Validate + normalize the output format. `None` defaults to `sdl`.
fn normalize_format(format: Option<&str>) -> Result<String, SkillError> {
    let raw = format.unwrap_or("sdl").trim();
    if raw.is_empty() {
        return Ok("sdl".to_string());
    }
    let lower = raw.to_ascii_lowercase();
    if ALLOWED_FORMATS.contains(&lower.as_str()) {
        Ok(lower)
    } else {
        Err(SkillError::InvalidArgs(format!(
            "invalid graphql-introspect args: unsupported format {raw:?} (allowed: {})",
            ALLOWED_FORMATS.join(", ")
        )))
    }
}

// ---------------------------------------------------------------------------
// Introspection query
// ---------------------------------------------------------------------------

/// Build the introspection document to send.
///
/// Deliberately conservative: `specifiedByURL` and `isRepeatable` are only
/// requested when asked for, because servers on older GraphQL runtimes reject
/// unknown introspection fields outright, and `__schema { description }` is
/// never requested for the same reason. `includeDeprecated: true` is the one
/// long-standing argument every implementation accepts.
fn build_introspection_query(o: &Options) -> String {
    let desc = |indent: &str| {
        if o.descriptions {
            format!("\n{indent}description")
        } else {
            String::new()
        }
    };
    let dep = if o.include_deprecated {
        "(includeDeprecated: true)"
    } else {
        ""
    };
    let specified_by = if o.specified_by_url {
        "\n  specifiedByURL"
    } else {
        ""
    };
    let repeatable = if o.repeatable_directives {
        "\n    isRepeatable"
    } else {
        ""
    };
    format!(
        r#"query IntrospectionQuery {{
  __schema {{
    queryType {{ name }}
    mutationType {{ name }}
    subscriptionType {{ name }}
    types {{ ...FullType }}
    directives {{
      name{directive_desc}{repeatable}
      locations
      args {{ ...InputValue }}
    }}
  }}
}}

fragment FullType on __Type {{
  kind
  name{type_desc}{specified_by}
  fields{dep} {{
    name{field_desc}
    args {{ ...InputValue }}
    type {{ ...TypeRef }}
    isDeprecated
    deprecationReason
  }}
  inputFields {{ ...InputValue }}
  interfaces {{ ...TypeRef }}
  enumValues{dep} {{
    name{value_desc}
    isDeprecated
    deprecationReason
  }}
  possibleTypes {{ ...TypeRef }}
}}

fragment InputValue on __InputValue {{
  name{arg_desc}
  type {{ ...TypeRef }}
  defaultValue
}}

fragment TypeRef on __Type {{
  kind
  name
  ofType {{ kind name ofType {{ kind name ofType {{ kind name ofType {{ kind name ofType {{ kind name ofType {{ kind name ofType {{ kind name }} }} }} }} }} }} }}
}}"#,
        directive_desc = desc("      "),
        repeatable = repeatable,
        type_desc = desc("  "),
        specified_by = specified_by,
        dep = dep,
        field_desc = desc("    "),
        value_desc = desc("    "),
        arg_desc = desc("  "),
    )
}

// ---------------------------------------------------------------------------
// Introspection result model
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Schema {
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "queryType", default)]
    query_type: Option<NamedRef>,
    #[serde(rename = "mutationType", default)]
    mutation_type: Option<NamedRef>,
    #[serde(rename = "subscriptionType", default)]
    subscription_type: Option<NamedRef>,
    #[serde(default)]
    types: Vec<FullType>,
    #[serde(default)]
    directives: Vec<Directive>,
}

#[derive(Debug, Deserialize)]
struct NamedRef {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FullType {
    kind: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "specifiedByURL", default)]
    specified_by_url: Option<String>,
    #[serde(default)]
    fields: Option<Vec<Field>>,
    #[serde(rename = "inputFields", default)]
    input_fields: Option<Vec<InputValue>>,
    #[serde(default)]
    interfaces: Option<Vec<TypeRef>>,
    #[serde(rename = "enumValues", default)]
    enum_values: Option<Vec<EnumValue>>,
    #[serde(rename = "possibleTypes", default)]
    possible_types: Option<Vec<TypeRef>>,
}

#[derive(Debug, Deserialize)]
struct Field {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    args: Vec<InputValue>,
    #[serde(rename = "type")]
    ty: TypeRef,
    #[serde(rename = "isDeprecated", default)]
    is_deprecated: bool,
    #[serde(rename = "deprecationReason", default)]
    deprecation_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InputValue {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "type")]
    ty: TypeRef,
    #[serde(rename = "defaultValue", default)]
    default_value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EnumValue {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "isDeprecated", default)]
    is_deprecated: bool,
    #[serde(rename = "deprecationReason", default)]
    deprecation_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TypeRef {
    kind: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(rename = "ofType", default)]
    of_type: Option<Box<TypeRef>>,
}

#[derive(Debug, Deserialize)]
struct Directive {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    locations: Vec<String>,
    #[serde(default)]
    args: Vec<InputValue>,
    #[serde(rename = "isRepeatable", default)]
    is_repeatable: bool,
}

// ---------------------------------------------------------------------------
// Response classification
// ---------------------------------------------------------------------------

/// First `max` bytes of a body as lossy text, for error messages.
fn body_snippet(body: &[u8], max: usize) -> String {
    let end = body.len().min(max);
    String::from_utf8_lossy(&body[..end])
        .replace(['\n', '\r'], " ")
        .trim()
        .to_string()
}

/// Pull `data.__schema` out of a GraphQL response, turning every other outcome
/// into a message that says what to do next.
///
/// Order matters: a server that disables introspection answers `200` *or* `400`
/// with a populated `errors` array, so GraphQL errors are reported before the
/// HTTP status is judged — otherwise the caller just sees "HTTP 400" and loses
/// the reason.
fn extract_schema_value(body: &[u8], status: u16, url: &str) -> Result<Value, SkillError> {
    let parsed: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => {
            if status >= 400 {
                return Err(SkillError::HttpStatus {
                    status,
                    url: url.to_string(),
                });
            }
            return Err(SkillError::InvalidArgs(format!(
                "{url} did not return JSON (HTTP {status}). First bytes: {:?}. Check that the URL is the GraphQL endpoint itself (usually /graphql or /api/graphql), not the HTML playground page.",
                body_snippet(body, 200)
            )));
        }
    };

    if let Some(errors) = parsed.get("errors").and_then(|e| e.as_array()) {
        if !errors.is_empty() {
            let messages: Vec<String> = errors
                .iter()
                .take(5)
                .map(|e| {
                    e.get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("(no message)")
                        .to_string()
                })
                .collect();
            return Err(SkillError::InvalidArgs(format!(
                "GraphQL endpoint returned errors (HTTP {status}): {}. Introspection is commonly disabled in production; if the endpoint needs auth, pass it in headers. If the message names an unknown field, retry with specified_by_url=false and repeatable_directives=false.",
                messages.join("; ")
            )));
        }
    }

    if status >= 400 {
        return Err(SkillError::HttpStatus {
            status,
            url: url.to_string(),
        });
    }

    // `data.__schema` is the standard shape; a bare `__schema` document (some
    // gateways and saved introspection files) is accepted too.
    parsed
        .get("data")
        .and_then(|d| d.get("__schema"))
        .or_else(|| parsed.get("__schema"))
        .cloned()
        .ok_or_else(|| {
            SkillError::InvalidArgs(format!(
                "response from {url} has no data.__schema, so it is not a GraphQL introspection result. First bytes: {:?}",
                body_snippet(body, 200)
            ))
        })
}

/// Deserialize the `__schema` object into the typed model.
fn parse_schema(value: &Value) -> Result<Schema, SkillError> {
    serde_json::from_value(value.clone()).map_err(|e| {
        SkillError::InvalidArgs(format!("could not read the introspection result: {e}"))
    })
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

fn is_builtin_type(name: &str) -> bool {
    name.starts_with("__") || BUILTIN_SCALARS.contains(&name)
}

/// Render a (possibly wrapped) type reference, e.g. `[Character!]!`.
fn render_type_ref(t: &TypeRef) -> String {
    match t.kind.as_str() {
        "NON_NULL" => match &t.of_type {
            Some(inner) => format!("{}!", render_type_ref(inner)),
            None => "Unknown!".to_string(),
        },
        "LIST" => match &t.of_type {
            Some(inner) => format!("[{}]", render_type_ref(inner)),
            None => "[Unknown]".to_string(),
        },
        _ => t.name.clone().unwrap_or_else(|| "Unknown".to_string()),
    }
}

/// A non-empty description, or `None` when descriptions are off/blank.
fn describable<'a>(desc: Option<&'a String>, o: &Options) -> Option<&'a str> {
    if !o.descriptions {
        return None;
    }
    desc.map(|d| d.as_str()).filter(|d| !d.trim().is_empty())
}

/// Append a description as a GraphQL block string. Single-line descriptions
/// print inline (`"""Text"""`), matching what schema printers emit.
fn push_description(out: &mut String, desc: &str, indent: &str) {
    let escaped = desc.replace("\"\"\"", "\\\"\"\"");
    let one_line = !escaped.contains('\n') && !escaped.starts_with('"') && !escaped.ends_with('"');
    if one_line {
        out.push_str(indent);
        out.push_str("\"\"\"");
        out.push_str(escaped.trim_end());
        out.push_str("\"\"\"\n");
        return;
    }
    out.push_str(indent);
    out.push_str("\"\"\"\n");
    for line in escaped.lines() {
        if line.trim().is_empty() {
            out.push('\n');
        } else {
            out.push_str(indent);
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str(indent);
    out.push_str("\"\"\"\n");
}

/// GraphQL string literal (same escaping rules as JSON).
fn quote_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("{s:?}"))
}

/// ` @deprecated` / ` @deprecated(reason: "…")`, or empty when not deprecated.
/// `"No longer supported"` is the spec default reason, so it is left implicit.
fn deprecation_suffix(is_deprecated: bool, reason: Option<&String>) -> String {
    if !is_deprecated {
        return String::new();
    }
    match reason.map(|r| r.as_str()) {
        None | Some("") | Some("No longer supported") => " @deprecated".to_string(),
        Some(r) => format!(" @deprecated(reason: {})", quote_string(r)),
    }
}

fn render_input_value_inline(v: &InputValue) -> String {
    let mut s = format!("{}: {}", v.name, render_type_ref(&v.ty));
    if let Some(d) = v.default_value.as_deref().filter(|d| !d.is_empty()) {
        s.push_str(&format!(" = {d}"));
    }
    s
}

/// Render an argument list. Multi-line (one arg per line, with descriptions)
/// only when at least one argument is documented — otherwise inline.
fn render_args(args: &[InputValue], indent: &str, o: &Options) -> String {
    if args.is_empty() {
        return String::new();
    }
    let documented = args
        .iter()
        .any(|a| describable(a.description.as_ref(), o).is_some());
    if !documented {
        let inner: Vec<String> = args.iter().map(render_input_value_inline).collect();
        return format!("({})", inner.join(", "));
    }
    let inner_indent = format!("{indent}  ");
    let mut s = String::from("(\n");
    for a in args {
        if let Some(d) = describable(a.description.as_ref(), o) {
            push_description(&mut s, d, &inner_indent);
        }
        s.push_str(&inner_indent);
        s.push_str(&render_input_value_inline(a));
        s.push('\n');
    }
    s.push_str(indent);
    s.push(')');
    s
}

/// Fields to print: deprecated ones are dropped when `include_deprecated` is
/// off (a server that honored the flag already omitted them; a saved
/// introspection file may not have).
fn visible_fields<'a>(fields: &'a [Field], o: &Options) -> Vec<&'a Field> {
    let kept: Vec<&Field> = fields
        .iter()
        .filter(|f| o.include_deprecated || !f.is_deprecated)
        .collect();
    let mut out = kept;
    if o.sort {
        out.sort_by(|a, b| a.name.cmp(&b.name));
    }
    out
}

fn visible_enum_values<'a>(values: &'a [EnumValue], o: &Options) -> Vec<&'a EnumValue> {
    let mut out: Vec<&EnumValue> = values
        .iter()
        .filter(|v| o.include_deprecated || !v.is_deprecated)
        .collect();
    if o.sort {
        out.sort_by(|a, b| a.name.cmp(&b.name));
    }
    out
}

fn visible_args<'a>(args: &'a [InputValue], o: &Options) -> Vec<InputValue>
where
    InputValue: 'a,
{
    // Arguments are cloned-by-reference through render_args, which needs a
    // slice; sorting therefore materializes a new Vec of references' data.
    let mut idx: Vec<usize> = (0..args.len()).collect();
    if o.sort {
        idx.sort_by(|&a, &b| args[a].name.cmp(&args[b].name));
    }
    idx.into_iter()
        .map(|i| InputValue {
            name: args[i].name.clone(),
            description: args[i].description.clone(),
            ty: clone_type_ref(&args[i].ty),
            default_value: args[i].default_value.clone(),
        })
        .collect()
}

fn clone_type_ref(t: &TypeRef) -> TypeRef {
    TypeRef {
        kind: t.kind.clone(),
        name: t.name.clone(),
        of_type: t.of_type.as_ref().map(|i| Box::new(clone_type_ref(i))),
    }
}

/// Types to print, in schema order (or alphabetical when `sort`), with
/// built-ins filtered out unless asked for.
fn visible_types<'a>(schema: &'a Schema, o: &Options) -> Vec<&'a FullType> {
    let mut out: Vec<&FullType> = schema
        .types
        .iter()
        .filter(|t| match t.name.as_deref() {
            Some(n) => o.include_builtins || !is_builtin_type(n),
            None => false,
        })
        .collect();
    if o.sort {
        out.sort_by(|a, b| a.name.cmp(&b.name));
    }
    out
}

fn visible_directives<'a>(schema: &'a Schema, o: &Options) -> Vec<&'a Directive> {
    let mut out: Vec<&Directive> = schema
        .directives
        .iter()
        .filter(|d| o.include_builtins || !BUILTIN_DIRECTIVES.contains(&d.name.as_str()))
        .collect();
    if o.sort {
        out.sort_by(|a, b| a.name.cmp(&b.name));
    }
    out
}

fn root_name(r: &Option<NamedRef>) -> Option<String> {
    r.as_ref().and_then(|n| n.name.clone())
}

// ---------------------------------------------------------------------------
// SDL rendering
// ---------------------------------------------------------------------------

/// A `schema { … }` block is only needed when a root operation type has a
/// non-conventional name (`Query`/`Mutation`/`Subscription` are implied).
fn needs_schema_block(schema: &Schema) -> bool {
    let odd = |actual: Option<String>, conventional: &str| match actual {
        Some(n) => n != conventional,
        None => false,
    };
    odd(root_name(&schema.query_type), "Query")
        || odd(root_name(&schema.mutation_type), "Mutation")
        || odd(root_name(&schema.subscription_type), "Subscription")
}

fn render_schema_block(schema: &Schema, o: &Options) -> String {
    let mut s = String::new();
    if let Some(d) = describable(schema.description.as_ref(), o) {
        push_description(&mut s, d, "");
    }
    s.push_str("schema {\n");
    for (label, name) in [
        ("query", root_name(&schema.query_type)),
        ("mutation", root_name(&schema.mutation_type)),
        ("subscription", root_name(&schema.subscription_type)),
    ] {
        if let Some(n) = name {
            s.push_str(&format!("  {label}: {n}\n"));
        }
    }
    s.push('}');
    s
}

fn render_directive(d: &Directive, o: &Options) -> String {
    let mut s = String::new();
    if let Some(desc) = describable(d.description.as_ref(), o) {
        push_description(&mut s, desc, "");
    }
    s.push_str(&format!("directive @{}", d.name));
    s.push_str(&render_args(&visible_args(&d.args, o), "", o));
    if o.repeatable_directives && d.is_repeatable {
        s.push_str(" repeatable");
    }
    s.push_str(&format!(" on {}", d.locations.join(" | ")));
    s
}

fn render_field(f: &Field, o: &Options) -> String {
    let mut s = String::new();
    if let Some(d) = describable(f.description.as_ref(), o) {
        push_description(&mut s, d, "  ");
    }
    s.push_str("  ");
    s.push_str(&f.name);
    s.push_str(&render_args(&visible_args(&f.args, o), "  ", o));
    s.push_str(&format!(": {}", render_type_ref(&f.ty)));
    s.push_str(&deprecation_suffix(
        f.is_deprecated,
        f.deprecation_reason.as_ref(),
    ));
    s
}

fn render_type(t: &FullType, o: &Options) -> String {
    let name = t.name.clone().unwrap_or_default();
    let mut s = String::new();
    if let Some(d) = describable(t.description.as_ref(), o) {
        push_description(&mut s, d, "");
    }
    match t.kind.as_str() {
        "SCALAR" => {
            s.push_str(&format!("scalar {name}"));
            if o.specified_by_url {
                if let Some(url) = t.specified_by_url.as_deref().filter(|u| !u.is_empty()) {
                    s.push_str(&format!(" @specifiedBy(url: {})", quote_string(url)));
                }
            }
        }
        "OBJECT" | "INTERFACE" => {
            let keyword = if t.kind == "OBJECT" {
                "type"
            } else {
                "interface"
            };
            s.push_str(&format!("{keyword} {name}"));
            let ifaces: Vec<String> = t
                .interfaces
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(render_type_ref)
                .collect();
            if !ifaces.is_empty() {
                s.push_str(&format!(" implements {}", ifaces.join(" & ")));
            }
            let fields = visible_fields(t.fields.as_deref().unwrap_or_default(), o);
            if fields.is_empty() {
                // A type with no printable fields is still worth emitting, as
                // an empty body, so references to it stay resolvable.
                s.push_str(" {\n}");
            } else {
                s.push_str(" {\n");
                for f in fields {
                    s.push_str(&render_field(f, o));
                    s.push('\n');
                }
                s.push('}');
            }
        }
        "UNION" => {
            let members: Vec<String> = t
                .possible_types
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(render_type_ref)
                .collect();
            let members = if o.sort {
                let mut m = members;
                m.sort();
                m
            } else {
                members
            };
            s.push_str(&format!("union {name} = {}", members.join(" | ")));
        }
        "ENUM" => {
            s.push_str(&format!("enum {name} {{\n"));
            for v in visible_enum_values(t.enum_values.as_deref().unwrap_or_default(), o) {
                if let Some(d) = describable(v.description.as_ref(), o) {
                    push_description(&mut s, d, "  ");
                }
                s.push_str(&format!(
                    "  {}{}\n",
                    v.name,
                    deprecation_suffix(v.is_deprecated, v.deprecation_reason.as_ref())
                ));
            }
            s.push('}');
        }
        "INPUT_OBJECT" => {
            s.push_str(&format!("input {name} {{\n"));
            for f in visible_args(t.input_fields.as_deref().unwrap_or_default(), o) {
                if let Some(d) = describable(f.description.as_ref(), o) {
                    push_description(&mut s, d, "  ");
                }
                s.push_str(&format!("  {}\n", render_input_value_inline(&f)));
            }
            s.push('}');
        }
        other => {
            // Unknown kind (a future spec addition): emit a comment rather than
            // silently dropping the type.
            s.push_str(&format!("# unsupported type kind {other} for {name}"));
        }
    }
    s
}

/// Render the whole schema as SDL.
fn render_sdl(schema: &Schema, o: &Options) -> String {
    let mut blocks: Vec<String> = Vec::new();
    if needs_schema_block(schema) || describable(schema.description.as_ref(), o).is_some() {
        blocks.push(render_schema_block(schema, o));
    }
    for d in visible_directives(schema, o) {
        blocks.push(render_directive(d, o));
    }
    for t in visible_types(schema, o) {
        blocks.push(render_type(t, o));
    }
    if blocks.is_empty() {
        return String::new();
    }
    format!("{}\n", blocks.join("\n\n"))
}

// ---------------------------------------------------------------------------
// Type list + Markdown rendering
// ---------------------------------------------------------------------------

/// `Character (OBJECT, 9 fields)` — one line per type, in the same order the
/// SDL uses.
fn render_type_list(schema: &Schema, o: &Options) -> String {
    let lines: Vec<String> = visible_types(schema, o)
        .into_iter()
        .map(|t| {
            let name = t.name.clone().unwrap_or_default();
            let detail = match t.kind.as_str() {
                "OBJECT" | "INTERFACE" => Some(count_label(
                    visible_fields(t.fields.as_deref().unwrap_or_default(), o).len(),
                    "field",
                )),
                "INPUT_OBJECT" => Some(count_label(
                    t.input_fields.as_deref().unwrap_or_default().len(),
                    "input field",
                )),
                "ENUM" => Some(count_label(
                    visible_enum_values(t.enum_values.as_deref().unwrap_or_default(), o).len(),
                    "value",
                )),
                "UNION" => Some(count_label(
                    t.possible_types.as_deref().unwrap_or_default().len(),
                    "member",
                )),
                _ => None,
            };
            match detail {
                Some(d) => format!("{name} ({}, {d})", t.kind),
                None => format!("{name} ({})", t.kind),
            }
        })
        .collect();
    lines.join("\n")
}

fn count_label(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// Flatten a description onto one line and escape table pipes.
fn md_cell(desc: Option<&str>) -> String {
    match desc {
        None => String::new(),
        Some(d) => d
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace('|', "\\|"),
    }
}

fn render_markdown(schema: &Schema, o: &Options) -> String {
    let types = visible_types(schema, o);
    let mut s = String::from("# GraphQL schema\n\n");
    for (label, name) in [
        ("Query", root_name(&schema.query_type)),
        ("Mutation", root_name(&schema.mutation_type)),
        ("Subscription", root_name(&schema.subscription_type)),
    ] {
        if let Some(n) = name {
            s.push_str(&format!("- **{label} root:** `{n}`\n"));
        }
    }
    s.push_str(&format!("- **Types:** {}\n", types.len()));

    for t in types {
        let name = t.name.clone().unwrap_or_default();
        s.push_str(&format!("\n## {name} `{}`\n", t.kind));
        if let Some(d) = describable(t.description.as_ref(), o) {
            s.push_str(&format!("\n{}\n", md_cell(Some(d))));
        }
        match t.kind.as_str() {
            "OBJECT" | "INTERFACE" => {
                let fields = visible_fields(t.fields.as_deref().unwrap_or_default(), o);
                if !fields.is_empty() {
                    s.push_str("\n| Field | Type | Description |\n| --- | --- | --- |\n");
                    for f in fields {
                        let dep = if f.is_deprecated {
                            " _(deprecated)_"
                        } else {
                            ""
                        };
                        s.push_str(&format!(
                            "| `{}` | `{}` | {}{} |\n",
                            f.name,
                            render_type_ref(&f.ty),
                            md_cell(describable(f.description.as_ref(), o)),
                            dep
                        ));
                    }
                }
            }
            "INPUT_OBJECT" => {
                let fields = visible_args(t.input_fields.as_deref().unwrap_or_default(), o);
                if !fields.is_empty() {
                    s.push_str("\n| Input field | Type | Default | Description |\n| --- | --- | --- | --- |\n");
                    for f in &fields {
                        s.push_str(&format!(
                            "| `{}` | `{}` | {} | {} |\n",
                            f.name,
                            render_type_ref(&f.ty),
                            f.default_value
                                .as_deref()
                                .filter(|d| !d.is_empty())
                                .map(|d| format!("`{d}`"))
                                .unwrap_or_default(),
                            md_cell(describable(f.description.as_ref(), o))
                        ));
                    }
                }
            }
            "ENUM" => {
                let values = visible_enum_values(t.enum_values.as_deref().unwrap_or_default(), o);
                if !values.is_empty() {
                    s.push_str("\n| Value | Description |\n| --- | --- |\n");
                    for v in values {
                        let dep = if v.is_deprecated {
                            " _(deprecated)_"
                        } else {
                            ""
                        };
                        s.push_str(&format!(
                            "| `{}` | {}{} |\n",
                            v.name,
                            md_cell(describable(v.description.as_ref(), o)),
                            dep
                        ));
                    }
                }
            }
            "UNION" => {
                let members: Vec<String> = t
                    .possible_types
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .map(|m| format!("`{}`", render_type_ref(m)))
                    .collect();
                if !members.is_empty() {
                    s.push_str(&format!("\nMembers: {}\n", members.join(", ")));
                }
            }
            _ => {}
        }
    }
    s
}

/// Render `format` into the text the caller gets back.
fn render_output(
    schema: &Schema,
    raw: &Value,
    format: &str,
    o: &Options,
) -> Result<String, SkillError> {
    Ok(match format {
        "types" => render_type_list(schema, o),
        "markdown" => render_markdown(schema, o),
        "json" => serde_json::to_string_pretty(&serde_json::json!({ "__schema": raw }))
            .map_err(|e| SkillError::Serialize(format!("serialize introspection json: {e}")))?,
        _ => render_sdl(schema, o),
    })
}

/// Clip `s` to `max` bytes on a char boundary. Returns `(text, was_truncated)`.
fn truncate_text(s: String, max: usize) -> (String, bool) {
    if s.len() <= max {
        return (s, false);
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    (s[..end].to_string(), true)
}

/// Percent-encode a query component per RFC 3986 (used for `method=GET`).
/// No `url` crate dependency — keeps the wasm payload small (mirrors
/// http-request's inline encoder).
fn percent_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(hex_upper(b >> 4));
                out.push(hex_upper(b & 0x0f));
            }
        }
    }
    out
}

fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + (nibble - 10)) as char,
    }
}

/// Build the GET URL carrying the introspection query in `?query=`.
fn build_get_url(base: &str, query: &str) -> String {
    let sep = if base.contains('?') { '&' } else { '?' };
    format!("{base}{sep}query={}", percent_encode_component(query))
}

/// HTTP status reason phrases, class-based fallback for unknown codes.
fn reason_phrase(code: u16) -> &'static str {
    match code {
        200 => "OK",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => match code {
            100..=199 => "Informational",
            200..=299 => "Success",
            300..=399 => "Redirect",
            400..=499 => "Client Error",
            500..=599 => "Server Error",
            _ => "Unknown",
        },
    }
}

fn format_status(code: u16) -> String {
    format!("{code} {}", reason_phrase(code))
}

/// Case-insensitive "does this header map already carry `name`?".
fn has_header(headers: &HashMap<String, String>, name: &str) -> bool {
    headers.keys().any(|k| k.eq_ignore_ascii_case(name))
}

// ---------------------------------------------------------------------------
// Block registration
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
struct GraphqlIntrospect;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/graphql-introspect",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Introspect a live GraphQL endpoint and return its SDL schema and type list",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Introspect a live GraphQL endpoint: send the introspection query to an http(s) GraphQL URL and get its schema back as SDL (default), a flat type list, Markdown documentation tables, or the raw {\"__schema\": ...} introspection JSON. Every response also includes the type list and the query/mutation/subscription root type names. Supports custom headers for authenticated APIs, POST (default) or GET transport, optional descriptions, deprecated members, built-in scalars and directives, alphabetical sorting, @specifiedBy URLs and repeatable directives. Only public URLs are allowed — loopback and private addresses are blocked, and the endpoint must have introspection enabled.",
        parameters = schema_json()
    ),
)]
impl GraphqlIntrospect {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("graphql-introspect")?;
    let method = normalize_method(args.method.as_deref())?;
    let format = normalize_format(args.format.as_deref())?;
    let opts = Options::from_args(&args);
    let query = build_introspection_query(&opts);

    let mut headers = args.headers.clone();
    if !has_header(&headers, "accept") {
        headers.insert("Accept".to_string(), "application/json".to_string());
    }
    // GET carries the document in `?query=`; POST sends the standard
    // `{"query": …, "operationName": …}` JSON envelope.
    let (url, req_body) = if method == "GET" {
        (build_get_url(&args.url, &query), None)
    } else {
        if !has_header(&headers, "content-type") {
            headers.insert("Content-Type".to_string(), "application/json".to_string());
        }
        let envelope = serde_json::json!({
            "query": query,
            "operationName": "IntrospectionQuery",
        })
        .to_string();
        (args.url.clone(), Some(envelope))
    };

    let resp = wafer_sdk::clients::network::do_request(
        &method,
        &url,
        &headers,
        req_body.as_deref().map(|s| s.as_bytes()),
    )?;

    let raw = extract_schema_value(&resp.body, resp.status_code, &args.url)?;
    let schema = parse_schema(&raw)?;

    let (types, types_truncated) =
        truncate_text(render_type_list(&schema, &opts), MAX_OUTPUT_BYTES);
    let (rendered, schema_truncated) = truncate_text(
        render_output(&schema, &raw, &format, &opts)?,
        MAX_OUTPUT_BYTES,
    );

    let tool = ToolResp {
        request: format!("{method} {url}"),
        status: format_status(resp.status_code),
        status_code: resp.status_code,
        format,
        query_type: root_name(&schema.query_type),
        mutation_type: root_name(&schema.mutation_type),
        subscription_type: root_name(&schema.subscription_type),
        type_count: visible_types(&schema, &opts).len(),
        types,
        schema: rendered,
        truncated: types_truncated || schema_truncated,
    };
    serde_json::to_vec(&tool)
        .map_err(|e| SkillError::Serialize(format!("serialize tool response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- chat-schema drift guard --------------------------------------------

    /// The descriptor is the single source for the chat schema AND the CLI, so
    /// pin its rendered JSON: any accidental param rename/removal fails here.
    #[test]
    fn schema_json_matches_expected_chat_schema() {
        let expected: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Absolute http(s) URL of the GraphQL endpoint, e.g. https://countries.trevorblades.com/graphql. Point it at the API endpoint itself, not at an HTML playground page." },
                    "headers": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Request headers as a name->value map, for endpoints that need auth — e.g. {\"Authorization\": \"Bearer <token>\"}. Content-Type and Accept default to application/json." },
                    "method":  { "type": "string", "enum": ["POST", "GET"], "default": "POST", "description": "How to send the introspection query: POST with a JSON body (default), or GET with the query in the ?query= parameter for endpoints that only allow GET." },
                    "format":  { "type": "string", "enum": ["sdl", "types", "markdown", "json"], "default": "sdl", "description": "Output shape: 'sdl' (default) prints the schema in GraphQL SDL; 'types' prints just the type list; 'markdown' prints per-type documentation tables; 'json' returns the raw {\"__schema\": ...} introspection object for other tooling." },
                    "descriptions": { "type": "boolean", "default": true, "description": "Request and print type/field descriptions (default true). Set false for a compact schema." },
                    "include_deprecated": { "type": "boolean", "default": true, "description": "Include deprecated fields and enum values, annotated with @deprecated (default true)." },
                    "include_builtins": { "type": "boolean", "default": false, "description": "Include built-in scalars (Int, Float, String, Boolean, ID), the __Schema/__Type introspection types, and the spec directives @skip/@include/@deprecated/@specifiedBy (default false)." },
                    "sort": { "type": "boolean", "default": false, "description": "Sort types, fields, arguments and enum values alphabetically for stable diffs (default false keeps the server's own order)." },
                    "specified_by_url": { "type": "boolean", "default": false, "description": "Ask for each custom scalar's specifiedByURL and print it as @specifiedBy(url: \"...\") (default false — older servers reject this field)." },
                    "repeatable_directives": { "type": "boolean", "default": false, "description": "Ask for each directive's isRepeatable flag and print the 'repeatable' keyword (default false — older servers reject this field)." }
                },
                "required": ["url"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, expected, "no LLM-facing chat-schema drift");
    }

    // --- fixtures -----------------------------------------------------------

    /// A small but representative introspection payload: object with args and a
    /// deprecated field, interface + implementor, union, enum, input object,
    /// custom scalar, custom + built-in directives, and built-in/introspection
    /// types that must be filtered out by default.
    fn fixture_schema_value() -> Value {
        serde_json::json!({
            "queryType": { "name": "Query" },
            "mutationType": null,
            "subscriptionType": null,
            "types": [
                {
                    "kind": "OBJECT",
                    "name": "Query",
                    "description": "Root queries.",
                    "fields": [
                        {
                            "name": "character",
                            "description": "Fetch one character.",
                            "args": [
                                { "name": "id", "description": null,
                                  "type": { "kind": "NON_NULL", "name": null, "ofType": { "kind": "SCALAR", "name": "ID", "ofType": null } },
                                  "defaultValue": null },
                                { "name": "limit", "description": null,
                                  "type": { "kind": "SCALAR", "name": "Int", "ofType": null },
                                  "defaultValue": "10" }
                            ],
                            "type": { "kind": "OBJECT", "name": "Character", "ofType": null },
                            "isDeprecated": false,
                            "deprecationReason": null
                        },
                        {
                            "name": "legacyCharacter",
                            "description": null,
                            "args": [],
                            "type": { "kind": "OBJECT", "name": "Character", "ofType": null },
                            "isDeprecated": true,
                            "deprecationReason": "Use character instead."
                        }
                    ],
                    "inputFields": null, "interfaces": [], "enumValues": null, "possibleTypes": null
                },
                {
                    "kind": "INTERFACE",
                    "name": "Node",
                    "description": null,
                    "fields": [
                        { "name": "id", "description": null, "args": [],
                          "type": { "kind": "NON_NULL", "name": null, "ofType": { "kind": "SCALAR", "name": "ID", "ofType": null } },
                          "isDeprecated": false, "deprecationReason": null }
                    ],
                    "inputFields": null, "interfaces": [], "enumValues": null,
                    "possibleTypes": [ { "kind": "OBJECT", "name": "Character", "ofType": null } ]
                },
                {
                    "kind": "OBJECT",
                    "name": "Character",
                    "description": "A person in the show.",
                    "fields": [
                        { "name": "id", "description": null, "args": [],
                          "type": { "kind": "NON_NULL", "name": null, "ofType": { "kind": "SCALAR", "name": "ID", "ofType": null } },
                          "isDeprecated": false, "deprecationReason": null },
                        { "name": "episodes", "description": null, "args": [],
                          "type": { "kind": "LIST", "name": null, "ofType": { "kind": "OBJECT", "name": "Episode", "ofType": null } },
                          "isDeprecated": false, "deprecationReason": null },
                        { "name": "status", "description": null, "args": [],
                          "type": { "kind": "ENUM", "name": "Status", "ofType": null },
                          "isDeprecated": false, "deprecationReason": null }
                    ],
                    "inputFields": null,
                    "interfaces": [ { "kind": "INTERFACE", "name": "Node", "ofType": null } ],
                    "enumValues": null, "possibleTypes": null
                },
                {
                    "kind": "OBJECT", "name": "Episode", "description": null,
                    "fields": [
                        { "name": "name", "description": null, "args": [],
                          "type": { "kind": "SCALAR", "name": "String", "ofType": null },
                          "isDeprecated": false, "deprecationReason": null }
                    ],
                    "inputFields": null, "interfaces": [], "enumValues": null, "possibleTypes": null
                },
                {
                    "kind": "UNION", "name": "SearchResult", "description": null,
                    "fields": null, "inputFields": null, "interfaces": null, "enumValues": null,
                    "possibleTypes": [
                        { "kind": "OBJECT", "name": "Character", "ofType": null },
                        { "kind": "OBJECT", "name": "Episode", "ofType": null }
                    ]
                },
                {
                    "kind": "ENUM", "name": "Status", "description": null,
                    "fields": null, "inputFields": null, "interfaces": null,
                    "enumValues": [
                        { "name": "ALIVE", "description": "Still going.", "isDeprecated": false, "deprecationReason": null },
                        { "name": "DEAD", "description": null, "isDeprecated": false, "deprecationReason": null },
                        { "name": "UNKNOWN", "description": null, "isDeprecated": true, "deprecationReason": "No longer supported" }
                    ],
                    "possibleTypes": null
                },
                {
                    "kind": "INPUT_OBJECT", "name": "CharacterFilter", "description": null,
                    "fields": null,
                    "inputFields": [
                        { "name": "name", "description": null, "type": { "kind": "SCALAR", "name": "String", "ofType": null }, "defaultValue": null },
                        { "name": "status", "description": null, "type": { "kind": "ENUM", "name": "Status", "ofType": null }, "defaultValue": "ALIVE" }
                    ],
                    "interfaces": null, "enumValues": null, "possibleTypes": null
                },
                {
                    "kind": "SCALAR", "name": "DateTime", "description": null,
                    "specifiedByURL": "https://scalars.graphql.org/andimarek/date-time",
                    "fields": null, "inputFields": null, "interfaces": null, "enumValues": null, "possibleTypes": null
                },
                {
                    "kind": "SCALAR", "name": "String", "description": "Built-in.",
                    "fields": null, "inputFields": null, "interfaces": null, "enumValues": null, "possibleTypes": null
                },
                {
                    "kind": "OBJECT", "name": "__Schema", "description": null,
                    "fields": [], "inputFields": null, "interfaces": [], "enumValues": null, "possibleTypes": null
                }
            ],
            "directives": [
                { "name": "auth", "description": "Requires a role.", "locations": ["FIELD_DEFINITION"],
                  "args": [ { "name": "role", "description": null, "type": { "kind": "SCALAR", "name": "String", "ofType": null }, "defaultValue": "\"user\"" } ],
                  "isRepeatable": true },
                { "name": "skip", "description": null, "locations": ["FIELD"],
                  "args": [], "isRepeatable": false }
            ]
        })
    }

    fn fixture() -> Schema {
        parse_schema(&fixture_schema_value()).unwrap()
    }

    // --- normalize_method / normalize_format --------------------------------

    #[test]
    fn normalize_method_defaults_to_post() {
        assert_eq!(normalize_method(None).unwrap(), "POST");
        assert_eq!(normalize_method(Some("")).unwrap(), "POST");
        assert_eq!(normalize_method(Some("get")).unwrap(), "GET");
    }

    #[test]
    fn normalize_method_rejects_unsupported() {
        let err = normalize_method(Some("PUT")).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("unsupported method"), "got {msg}");
        assert!(msg.contains("POST, GET"), "got {msg}");
    }

    #[test]
    fn normalize_format_defaults_and_lowercases() {
        assert_eq!(normalize_format(None).unwrap(), "sdl");
        assert_eq!(normalize_format(Some("SDL")).unwrap(), "sdl");
        assert_eq!(normalize_format(Some("markdown")).unwrap(), "markdown");
    }

    #[test]
    fn normalize_format_rejects_unknown() {
        let err = normalize_format(Some("yaml")).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("unsupported format"), "got {msg}");
        assert!(msg.contains("sdl, types, markdown, json"), "got {msg}");
    }

    // --- query building -----------------------------------------------------

    #[test]
    fn query_defaults_include_descriptions_and_deprecated() {
        let q = build_introspection_query(&Options::default());
        assert!(q.contains("fields(includeDeprecated: true)"), "got {q}");
        assert!(q.contains("enumValues(includeDeprecated: true)"), "got {q}");
        assert!(q.contains("description"), "got {q}");
        // Compat: newer introspection fields are opt-in.
        assert!(!q.contains("specifiedByURL"), "got {q}");
        assert!(!q.contains("isRepeatable"), "got {q}");
        assert!(q.contains("fragment TypeRef on __Type"), "got {q}");
    }

    #[test]
    fn query_respects_opt_outs_and_opt_ins() {
        let o = Options {
            descriptions: false,
            include_deprecated: false,
            specified_by_url: true,
            repeatable_directives: true,
            ..Options::default()
        };
        let q = build_introspection_query(&o);
        assert!(!q.contains("description"), "got {q}");
        assert!(!q.contains("includeDeprecated"), "got {q}");
        assert!(q.contains("specifiedByURL"), "got {q}");
        assert!(q.contains("isRepeatable"), "got {q}");
    }

    // --- response classification -------------------------------------------

    #[test]
    fn extract_schema_value_reads_data_schema() {
        let body = br#"{"data":{"__schema":{"types":[]}}}"#;
        let v = extract_schema_value(body, 200, "https://x.test/graphql").unwrap();
        assert!(v.get("types").is_some());
    }

    #[test]
    fn extract_schema_value_accepts_bare_schema_document() {
        let body = br#"{"__schema":{"types":[]}}"#;
        assert!(extract_schema_value(body, 200, "https://x.test/graphql").is_ok());
    }

    #[test]
    fn extract_schema_value_reports_graphql_errors_before_status() {
        let body =
            br#"{"errors":[{"message":"GraphQL introspection is not allowed"}],"data":null}"#;
        let err = extract_schema_value(body, 400, "https://x.test/graphql").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("introspection is not allowed"), "got {msg}");
        assert!(msg.contains("commonly disabled in production"), "got {msg}");
    }

    #[test]
    fn extract_schema_value_non_json_body_explains_endpoint_mixup() {
        let err = extract_schema_value(b"<!doctype html><html>", 200, "https://x.test/graphiql")
            .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("did not return JSON"), "got {msg}");
        assert!(msg.contains("playground"), "got {msg}");
    }

    #[test]
    fn extract_schema_value_http_error_without_json_is_http_status() {
        let err = extract_schema_value(b"nope", 404, "https://x.test/graphql").unwrap_err();
        assert!(matches!(err, SkillError::HttpStatus { status: 404, .. }));
    }

    #[test]
    fn extract_schema_value_json_without_schema_is_rejected() {
        let err = extract_schema_value(br#"{"data":{"hello":"world"}}"#, 200, "https://x.test/g")
            .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("no data.__schema"), "got {msg}");
    }

    // --- type refs ----------------------------------------------------------

    #[test]
    fn render_type_ref_wraps_lists_and_non_nulls() {
        let v = serde_json::json!({
            "kind": "NON_NULL", "name": null,
            "ofType": { "kind": "LIST", "name": null,
                "ofType": { "kind": "NON_NULL", "name": null,
                    "ofType": { "kind": "OBJECT", "name": "Character", "ofType": null } } }
        });
        let t: TypeRef = serde_json::from_value(v).unwrap();
        assert_eq!(render_type_ref(&t), "[Character!]!");
    }

    // --- SDL ----------------------------------------------------------------

    #[test]
    fn sdl_renders_the_whole_fixture_exactly() {
        let sdl = render_sdl(&fixture(), &Options::default());
        let expected = r#""""Requires a role."""
directive @auth(role: String = "user") on FIELD_DEFINITION

"""Root queries."""
type Query {
  """Fetch one character."""
  character(id: ID!, limit: Int = 10): Character
  legacyCharacter: Character @deprecated(reason: "Use character instead.")
}

interface Node {
  id: ID!
}

"""A person in the show."""
type Character implements Node {
  id: ID!
  episodes: [Episode]
  status: Status
}

type Episode {
  name: String
}

union SearchResult = Character | Episode

enum Status {
  """Still going."""
  ALIVE
  DEAD
  UNKNOWN @deprecated
}

input CharacterFilter {
  name: String
  status: Status = ALIVE
}

scalar DateTime
"#;
        assert_eq!(sdl, expected);
    }

    #[test]
    fn sdl_hides_builtins_by_default_and_shows_them_on_request() {
        let schema = fixture();
        let default_sdl = render_sdl(&schema, &Options::default());
        assert!(!default_sdl.contains("__Schema"), "got {default_sdl}");
        assert!(!default_sdl.contains("scalar String"), "got {default_sdl}");
        assert!(
            !default_sdl.contains("directive @skip"),
            "got {default_sdl}"
        );

        let with_builtins = render_sdl(
            &schema,
            &Options {
                include_builtins: true,
                ..Options::default()
            },
        );
        assert!(
            with_builtins.contains("type __Schema"),
            "got {with_builtins}"
        );
        assert!(
            with_builtins.contains("scalar String"),
            "got {with_builtins}"
        );
        assert!(
            with_builtins.contains("directive @skip on FIELD"),
            "got {with_builtins}"
        );
    }

    #[test]
    fn sdl_without_descriptions_is_compact() {
        let sdl = render_sdl(
            &fixture(),
            &Options {
                descriptions: false,
                ..Options::default()
            },
        );
        assert!(!sdl.contains("\"\"\""), "got {sdl}");
        assert!(
            sdl.contains("type Query {\n  character(id: ID!, limit: Int = 10): Character"),
            "got {sdl}"
        );
    }

    #[test]
    fn sdl_without_deprecated_drops_deprecated_members() {
        let sdl = render_sdl(
            &fixture(),
            &Options {
                include_deprecated: false,
                ..Options::default()
            },
        );
        assert!(!sdl.contains("legacyCharacter"), "got {sdl}");
        assert!(!sdl.contains("UNKNOWN"), "got {sdl}");
        assert!(sdl.contains("ALIVE"), "got {sdl}");
    }

    #[test]
    fn sdl_sort_orders_types_fields_and_enum_values() {
        let sdl = render_sdl(
            &fixture(),
            &Options {
                sort: true,
                ..Options::default()
            },
        );
        let pos = |needle: &str| {
            sdl.find(needle)
                .unwrap_or_else(|| panic!("missing {needle} in {sdl}"))
        };
        assert!(pos("type Character") < pos("input CharacterFilter"));
        assert!(pos("input CharacterFilter") < pos("scalar DateTime"));
        assert!(pos("scalar DateTime") < pos("type Episode"));
        assert!(pos("interface Node") < pos("type Query"));
        // Character's fields sort episodes < id < status.
        assert!(sdl.contains("type Character implements Node {\n  episodes: [Episode]\n  id: ID!\n  status: Status\n}"), "got {sdl}");
        // Union members sort too.
        assert!(
            sdl.contains("union SearchResult = Character | Episode"),
            "got {sdl}"
        );
    }

    #[test]
    fn sdl_specified_by_url_is_opt_in() {
        let schema = fixture();
        assert!(!render_sdl(&schema, &Options::default()).contains("@specifiedBy"));
        let with_url = render_sdl(
            &schema,
            &Options {
                specified_by_url: true,
                ..Options::default()
            },
        );
        assert!(
            with_url.contains(
                "scalar DateTime @specifiedBy(url: \"https://scalars.graphql.org/andimarek/date-time\")"
            ),
            "got {with_url}"
        );
    }

    #[test]
    fn sdl_repeatable_directive_is_opt_in() {
        let schema = fixture();
        assert!(!render_sdl(&schema, &Options::default()).contains("repeatable"));
        let with_rep = render_sdl(
            &schema,
            &Options {
                repeatable_directives: true,
                ..Options::default()
            },
        );
        assert!(
            with_rep.contains(
                "\"\"\"Requires a role.\"\"\"\ndirective @auth(role: String = \"user\") repeatable on FIELD_DEFINITION"
            ),
            "got {with_rep}"
        );
    }

    #[test]
    fn sdl_emits_schema_block_for_non_conventional_roots() {
        let v = serde_json::json!({
            "queryType": { "name": "RootQuery" },
            "mutationType": { "name": "RootMutation" },
            "types": [],
            "directives": []
        });
        let schema = parse_schema(&v).unwrap();
        let sdl = render_sdl(&schema, &Options::default());
        assert_eq!(
            sdl,
            "schema {\n  query: RootQuery\n  mutation: RootMutation\n}\n"
        );
    }

    #[test]
    fn sdl_omits_schema_block_for_conventional_roots() {
        let sdl = render_sdl(&fixture(), &Options::default());
        assert!(!sdl.contains("schema {"), "got {sdl}");
    }

    #[test]
    fn sdl_renders_multiline_descriptions_as_block_strings() {
        let v = serde_json::json!({
            "queryType": { "name": "Query" },
            "types": [ {
                "kind": "OBJECT", "name": "Query", "description": "Line one.\nLine two.",
                "fields": [ { "name": "ok", "description": null, "args": [],
                    "type": { "kind": "SCALAR", "name": "Boolean", "ofType": null },
                    "isDeprecated": false, "deprecationReason": null } ],
                "inputFields": null, "interfaces": [], "enumValues": null, "possibleTypes": null
            } ],
            "directives": []
        });
        let sdl = render_sdl(&parse_schema(&v).unwrap(), &Options::default());
        assert!(
            sdl.starts_with("\"\"\"\nLine one.\nLine two.\n\"\"\"\ntype Query {"),
            "got {sdl}"
        );
    }

    #[test]
    fn sdl_renders_documented_arguments_multiline() {
        let v = serde_json::json!({
            "queryType": { "name": "Query" },
            "types": [ {
                "kind": "OBJECT", "name": "Query", "description": null,
                "fields": [ { "name": "search", "description": null,
                    "args": [ { "name": "term", "description": "What to look for.",
                        "type": { "kind": "SCALAR", "name": "String", "ofType": null }, "defaultValue": null } ],
                    "type": { "kind": "SCALAR", "name": "String", "ofType": null },
                    "isDeprecated": false, "deprecationReason": null } ],
                "inputFields": null, "interfaces": [], "enumValues": null, "possibleTypes": null
            } ],
            "directives": []
        });
        let sdl = render_sdl(&parse_schema(&v).unwrap(), &Options::default());
        assert!(
            sdl.contains(
                "  search(\n    \"\"\"What to look for.\"\"\"\n    term: String\n  ): String"
            ),
            "got {sdl}"
        );
    }

    // --- type list ----------------------------------------------------------

    #[test]
    fn type_list_reports_kind_and_member_counts() {
        let list = render_type_list(&fixture(), &Options::default());
        assert_eq!(
            list,
            "Query (OBJECT, 2 fields)\n\
             Node (INTERFACE, 1 field)\n\
             Character (OBJECT, 3 fields)\n\
             Episode (OBJECT, 1 field)\n\
             SearchResult (UNION, 2 members)\n\
             Status (ENUM, 3 values)\n\
             CharacterFilter (INPUT_OBJECT, 2 input fields)\n\
             DateTime (SCALAR)"
        );
    }

    // --- markdown -----------------------------------------------------------

    #[test]
    fn markdown_has_roots_and_field_tables() {
        let md = render_markdown(&fixture(), &Options::default());
        assert!(
            md.starts_with("# GraphQL schema\n\n- **Query root:** `Query`\n"),
            "got {md}"
        );
        assert!(md.contains("- **Types:** 8"), "got {md}");
        assert!(md.contains("## Character `OBJECT`"), "got {md}");
        assert!(md.contains("| Field | Type | Description |"), "got {md}");
        assert!(md.contains("| `episodes` | `[Episode]` |"), "got {md}");
        assert!(md.contains("_(deprecated)_"), "got {md}");
        assert!(
            md.contains("| Input field | Type | Default | Description |"),
            "got {md}"
        );
        assert!(md.contains("| `status` | `Status` | `ALIVE` |"), "got {md}");
        assert!(md.contains("Members: `Character`, `Episode`"), "got {md}");
    }

    #[test]
    fn markdown_flattens_and_escapes_descriptions() {
        let v = serde_json::json!({
            "queryType": { "name": "Query" },
            "types": [ {
                "kind": "OBJECT", "name": "Query", "description": null,
                "fields": [ { "name": "ok", "description": "a | b\nnext line", "args": [],
                    "type": { "kind": "SCALAR", "name": "Boolean", "ofType": null },
                    "isDeprecated": false, "deprecationReason": null } ],
                "inputFields": null, "interfaces": [], "enumValues": null, "possibleTypes": null
            } ],
            "directives": []
        });
        let md = render_markdown(&parse_schema(&v).unwrap(), &Options::default());
        assert!(
            md.contains("| `ok` | `Boolean` | a \\| b next line |"),
            "got {md}"
        );
    }

    // --- json passthrough ---------------------------------------------------

    #[test]
    fn json_format_returns_the_raw_schema_object() {
        let raw = fixture_schema_value();
        let out = render_output(&fixture(), &raw, "json", &Options::default()).unwrap();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["__schema"]["queryType"]["name"], "Query");
        assert!(out.contains("\n  \"__schema\""), "pretty-printed: {out}");
    }

    // --- transport helpers --------------------------------------------------

    #[test]
    fn build_get_url_encodes_the_query() {
        let url = build_get_url("https://x.test/graphql", "{ __schema { types { name } } }");
        assert_eq!(
            url,
            "https://x.test/graphql?query=%7B%20__schema%20%7B%20types%20%7B%20name%20%7D%20%7D%20%7D"
        );
    }

    #[test]
    fn build_get_url_appends_to_existing_query_string() {
        let url = build_get_url("https://x.test/graphql?v=2", "{a}");
        assert_eq!(url, "https://x.test/graphql?v=2&query=%7Ba%7D");
    }

    #[test]
    fn has_header_is_case_insensitive() {
        let mut h = HashMap::new();
        h.insert("content-TYPE".to_string(), "application/json".to_string());
        assert!(has_header(&h, "Content-Type"));
        assert!(!has_header(&h, "Accept"));
    }

    #[test]
    fn format_status_known_and_class_fallback() {
        assert_eq!(format_status(200), "200 OK");
        assert_eq!(format_status(451), "451 Client Error");
    }

    #[test]
    fn truncate_text_clips_on_char_boundary() {
        let (s, was) = truncate_text("aä".to_string(), 2);
        assert_eq!(s, "a");
        assert!(was);
        let (s, was) = truncate_text("abc".to_string(), 10);
        assert_eq!(s, "abc");
        assert!(!was);
    }

    #[test]
    fn options_from_args_applies_defaults() {
        let args: Args = serde_json::from_str(r#"{"url":"https://x.test/graphql"}"#).unwrap();
        assert_eq!(Options::from_args(&args), Options::default());
        let args: Args = serde_json::from_str(
            r#"{"url":"https://x.test/graphql","sort":true,"descriptions":false}"#,
        )
        .unwrap();
        assert_eq!(
            Options::from_args(&args),
            Options {
                sort: true,
                descriptions: false,
                ..Options::default()
            }
        );
    }
}
