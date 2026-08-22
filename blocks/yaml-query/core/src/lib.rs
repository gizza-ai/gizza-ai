//! gizza-ai/yaml-query core — read, filter and transform YAML with jq-style
//! (yq-style) expressions, and convert the result between YAML and JSON.
//!
//! Pure compute, shared by the chat skill block, the CLI, and the web page. No
//! wafer/wasm-bindgen deps and no WASI/host calls, so it runs in the gizza wafer
//! runtime (chat), the CLI, and the browser page alike.
//!
//! Pipeline: parse YAML (or JSON) into a universal `serde_json::Value` →
//! evaluate the jq program with `jaq` (a pure-Rust jq implementation, jq
//! standard library included) → serialize each produced value back to YAML or
//! JSON.
//!
//! A jq filter yields a *stream* — zero, one, or many values — so the result is
//! a `Vec<String>`, one serialized value per output. Multi-document YAML streams
//! (`---` separated, the Kubernetes shape) are supported: `DocMode::Each` runs
//! the filter once per document, `DocMode::Slurp` feeds all documents to the
//! filter as a single array (jq's `-s`).

use jaq_interpret::{Ctx, FilterT, ParseCtx, RcIter, Val};
use serde::Deserialize;
use serde_json::Value as Json;
use serde_yml::Value as Yaml;

/// Largest input document accepted, in bytes. Bigger inputs are rejected with a
/// clear message instead of locking up the browser tab.
pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;

/// Largest number of values a filter may produce before it is cut off. Guards
/// against unbounded generators (`repeat(.)`, `range(1e12)`).
pub const MAX_OUTPUTS: usize = 50_000;

/// How the input text should be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InFormat {
    /// Sniff: a document starting with `{` or `[` is tried as JSON first, then
    /// YAML; anything else is parsed as YAML (which is a superset of JSON).
    Auto,
    Yaml,
    Json,
}

/// How each produced value should be serialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Yaml,
    Json,
}

/// How a multi-document YAML stream is fed to the filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocMode {
    /// Run the filter once per document and concatenate the outputs (default,
    /// matches `yq`).
    Each,
    /// Collect every document into one array and run the filter once over it
    /// (jq's `--slurp` / `-s`).
    Slurp,
}

impl InFormat {
    pub fn parse(s: &str) -> Result<InFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(InFormat::Auto),
            "yaml" | "yml" => Ok(InFormat::Yaml),
            "json" => Ok(InFormat::Json),
            other => Err(format!("unknown input_format '{other}' (expected auto, yaml, or json)")),
        }
    }
}

impl OutFormat {
    pub fn parse(s: &str) -> Result<OutFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "yaml" | "yml" => Ok(OutFormat::Yaml),
            "json" => Ok(OutFormat::Json),
            other => Err(format!("unknown output_format '{other}' (expected yaml or json)")),
        }
    }
}

impl DocMode {
    pub fn parse(s: &str) -> Result<DocMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "each" => Ok(DocMode::Each),
            "slurp" | "all" => Ok(DocMode::Slurp),
            other => Err(format!("unknown documents mode '{other}' (expected each or slurp)")),
        }
    }
}

/// Everything that isn't the document or the program.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub input_format: InFormat,
    pub output_format: OutFormat,
    pub documents: DocMode,
    /// Indent JSON output. Ignored for YAML output (always block style).
    pub pretty: bool,
    /// Emit string results without quotes or escaping (jq's `-r`).
    pub raw_output: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: InFormat::Auto,
            output_format: OutFormat::Yaml,
            documents: DocMode::Each,
            pretty: true,
            raw_output: false,
        }
    }
}

// ---------------------------------------------------------------------------
// YAML → universal JSON value
// ---------------------------------------------------------------------------

/// Render a YAML mapping key as an object key. JSON/jq object keys must be
/// strings, so scalar keys are stringified (`1:` → `"1"`) and collection keys
/// are rejected rather than silently mangled.
fn key_to_string(k: Yaml) -> Result<String, String> {
    Ok(match k {
        Yaml::String(s) => s,
        Yaml::Bool(b) => b.to_string(),
        Yaml::Number(n) => n.to_string(),
        Yaml::Null => "null".to_string(),
        Yaml::Tagged(t) => key_to_string(t.value)?,
        Yaml::Sequence(_) | Yaml::Mapping(_) => {
            return Err(
                "YAML mapping key is a sequence or mapping; jq object keys must be scalars".into(),
            )
        }
    })
}

/// The YAML merge key. `<<: *anchor` splices another mapping's entries into the
/// current one — the shape every `.gitlab-ci.yml` / docker-compose `x-` defaults
/// block relies on. `serde_yml` resolves the *alias* but leaves the `<<` key in
/// place, so the splice is applied here.
const MERGE_KEY: &str = "<<";

/// A merge source is a mapping, or a sequence of mappings. Anything else (a
/// scalar under `<<`) is a literal key named `<<`, not a merge, and is kept.
fn is_mergeable(v: &Yaml) -> bool {
    match v {
        Yaml::Mapping(_) => true,
        Yaml::Sequence(items) => !items.is_empty() && items.iter().all(is_mergeable),
        Yaml::Tagged(t) => is_mergeable(&t.value),
        _ => false,
    }
}

/// Splice a merge source into `obj`. Per the YAML merge-key spec the explicit
/// keys already in `obj` win, and inside a sequence the earlier mapping wins —
/// so only keys not yet present are inserted.
fn apply_merge(obj: &mut serde_json::Map<String, Json>, source: Yaml) -> Result<(), String> {
    match source {
        Yaml::Mapping(map) => {
            for (k, val) in map {
                let key = key_to_string(k)?;
                if key == MERGE_KEY && is_mergeable(&val) {
                    apply_merge(obj, val)?;
                    continue;
                }
                if !obj.contains_key(&key) {
                    obj.insert(key, yaml_to_json(val)?);
                }
            }
        }
        Yaml::Sequence(items) => {
            for item in items {
                apply_merge(obj, item)?;
            }
        }
        Yaml::Tagged(t) => apply_merge(obj, t.value)?,
        // Guarded by is_mergeable at every call site.
        _ => {}
    }
    Ok(())
}

/// Convert a parsed YAML value into the universal `serde_json::Value` jaq works
/// on. Custom tags (`!Ref`, `!!str`) are unwrapped to their value; `.nan`,
/// `.inf` and `-.inf` become null because JSON has no representation for them.
fn yaml_to_json(v: Yaml) -> Result<Json, String> {
    Ok(match v {
        Yaml::Null => Json::Null,
        Yaml::Bool(b) => Json::Bool(b),
        Yaml::Number(n) => {
            if let Some(i) = n.as_i64() {
                Json::from(i)
            } else if let Some(u) = n.as_u64() {
                Json::from(u)
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f).map(Json::Number).unwrap_or(Json::Null)
            } else {
                Json::Null
            }
        }
        Yaml::String(s) => Json::String(s),
        Yaml::Sequence(seq) => {
            let mut out = Vec::with_capacity(seq.len());
            for item in seq {
                out.push(yaml_to_json(item)?);
            }
            Json::Array(out)
        }
        Yaml::Mapping(map) => {
            let mut obj = serde_json::Map::new();
            let mut merges: Vec<Yaml> = Vec::new();
            for (k, val) in map {
                let key = key_to_string(k)?;
                // YAML merge key: `<<: *anchor` (or a list of anchors) splices
                // another mapping in. Explicit keys win, so merges are applied
                // after every real key has been placed.
                if key == MERGE_KEY && is_mergeable(&val) {
                    merges.push(val);
                    continue;
                }
                obj.insert(key, yaml_to_json(val)?);
            }
            for source in merges {
                apply_merge(&mut obj, source)?;
            }
            Json::Object(obj)
        }
        Yaml::Tagged(t) => yaml_to_json(t.value)?,
    })
}

/// Parse the input into one value per YAML document.
fn parse_documents(input: &str, format: InFormat) -> Result<Vec<Json>, String> {
    let looks_json = {
        let t = input.trim_start();
        t.starts_with('{') || t.starts_with('[')
    };
    match format {
        InFormat::Json => {
            let v: Json =
                serde_json::from_str(input).map_err(|e| format!("invalid JSON input: {e}"))?;
            Ok(vec![v])
        }
        InFormat::Auto if looks_json => match serde_json::from_str::<Json>(input) {
            Ok(v) => Ok(vec![v]),
            Err(_) => parse_yaml_stream(input),
        },
        InFormat::Auto | InFormat::Yaml => parse_yaml_stream(input),
    }
}

/// True when the input carries no YAML content at all — only blank lines,
/// comments, and document markers. `serde_yml` yields a single *null* document
/// for that, which reads like a query that legitimately matched nothing; an
/// explicit error is more useful than a silent `null`.
fn is_effectively_empty(input: &str) -> bool {
    input.lines().all(|line| {
        let t = line.trim();
        t.is_empty() || t.starts_with('#') || t == "---" || t == "..."
    })
}

fn parse_yaml_stream(input: &str) -> Result<Vec<Json>, String> {
    if is_effectively_empty(input) {
        return Err("no YAML documents found in the input (it is empty or only comments)".into());
    }
    let mut docs = Vec::new();
    for doc in serde_yml::Deserializer::from_str(input) {
        let value =
            Yaml::deserialize(doc).map_err(|e| format!("invalid YAML input: {e}"))?;
        docs.push(yaml_to_json(value)?);
    }
    if docs.is_empty() {
        return Err("no YAML documents found in the input (it is empty or only comments)".into());
    }
    Ok(docs)
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

fn serialize(value: &Json, opts: &Options) -> Result<String, String> {
    if opts.raw_output {
        if let Json::String(s) = value {
            return Ok(s.clone());
        }
    }
    match opts.output_format {
        OutFormat::Json => if opts.pretty {
            serde_json::to_string_pretty(value)
        } else {
            serde_json::to_string(value)
        }
        .map_err(|e| format!("JSON encode error: {e}")),
        OutFormat::Yaml => serde_yml::to_string(value)
            .map(|s| s.trim_end_matches('\n').to_string())
            .map_err(|e| format!("YAML encode error: {e}")),
    }
}

/// Join the produced values into the single blob the page and CLI display.
/// Multiple YAML values are joined as a proper `---` document stream; JSON and
/// raw values are joined by newline.
pub fn join_outputs(outputs: &[String], opts: &Options) -> String {
    let separator = match (opts.output_format, opts.raw_output, outputs.len()) {
        (OutFormat::Yaml, false, n) if n > 1 => "\n---\n",
        _ => "\n",
    };
    outputs.join(separator)
}

// ---------------------------------------------------------------------------
// The query itself
// ---------------------------------------------------------------------------

/// Compile `program` and run it over every input value, returning one
/// serialized string per produced value.
pub fn run_query(input: &str, program: &str, opts: &Options) -> Result<Vec<String>, String> {
    if input.trim().is_empty() {
        return Err("no input: paste a YAML (or JSON) document".into());
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {} bytes ({} MB)",
            input.len(),
            MAX_INPUT_BYTES,
            MAX_INPUT_BYTES / (1024 * 1024)
        ));
    }
    if program.trim().is_empty() {
        return Err("no query: use '.' for the whole document, or e.g. '.services.web.ports'".into());
    }

    let docs = parse_documents(input, opts.input_format)?;
    let roots = match opts.documents {
        DocMode::Each => docs,
        DocMode::Slurp => vec![Json::Array(docs)],
    };

    // Context with jaq's core natives + the jq standard library (map, select,
    // group_by, to_entries, …).
    let mut defs = ParseCtx::new(Vec::new());
    defs.insert_natives(jaq_core::core());
    defs.insert_defs(jaq_std::std());

    let (parsed, errs) = jaq_parse::parse(program, jaq_parse::main());
    if !errs.is_empty() {
        let msg = errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ");
        return Err(format!("query parse error: {msg}"));
    }
    let parsed = parsed.ok_or_else(|| "query produced no filter".to_string())?;

    let filter = defs.compile(parsed);
    if !defs.errs.is_empty() {
        let msg = defs
            .errs
            .iter()
            .map(|(e, _span)| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("query compile error: {msg}"));
    }

    let mut results = Vec::new();
    for root in roots {
        let inputs = RcIter::new(core::iter::empty());
        for item in filter.run((Ctx::new([], &inputs), Val::from(root))) {
            let v = item.map_err(|e| format!("query runtime error: {e}"))?;
            if results.len() >= MAX_OUTPUTS {
                return Err(format!(
                    "the query produced more than {MAX_OUTPUTS} values; narrow it or wrap it in [ … ]"
                ));
            }
            results.push(serialize(&Json::from(v), opts)?);
        }
    }
    Ok(results)
}

/// Convenience wrapper for the page/CLI: run the query and join the stream into
/// one displayable blob.
pub fn run_query_text(input: &str, program: &str, opts: &Options) -> Result<String, String> {
    let outputs = run_query(input, program, opts)?;
    Ok(join_outputs(&outputs, opts))
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPOSE: &str = r#"services:
  web:
    image: nginx:1.27
    ports:
      - "80:80"
      - "443:443"
  db:
    image: postgres:16
    ports:
      - "5432:5432"
"#;

    fn json_opts() -> Options {
        Options { output_format: OutFormat::Json, pretty: false, ..Options::default() }
    }

    // ---- happy paths ----

    #[test]
    fn docker_compose_ports() {
        let out = run_query_text(COMPOSE, ".services.web.ports", &Options::default()).unwrap();
        assert_eq!(out, "- '80:80'\n- '443:443'");
    }

    #[test]
    fn scalar_out_as_yaml() {
        let out = run_query_text(COMPOSE, ".services.web.image", &Options::default()).unwrap();
        assert_eq!(out, "nginx:1.27");
    }

    #[test]
    fn json_output_compact() {
        let out = run_query_text(COMPOSE, ".services | keys", &json_opts()).unwrap();
        assert_eq!(out, r#"["db","web"]"#);
    }

    #[test]
    fn json_output_pretty() {
        let opts = Options { output_format: OutFormat::Json, pretty: true, ..Options::default() };
        let out = run_query_text("a: 1\n", ".", &opts).unwrap();
        assert_eq!(out, "{\n  \"a\": 1\n}");
    }

    #[test]
    fn stream_of_values_joined_as_yaml_documents() {
        let out = run_query_text("- a\n- b\n", ".[]", &Options::default()).unwrap();
        assert_eq!(out, "a\n---\nb");
    }

    #[test]
    fn raw_output_strips_quotes() {
        let opts = Options { raw_output: true, output_format: OutFormat::Json, ..Options::default() };
        let out = run_query_text(COMPOSE, ".services.db.image", &opts).unwrap();
        assert_eq!(out, "postgres:16");
        // Non-strings are unaffected by raw output.
        let n = run_query_text("n: 7\n", ".n", &opts).unwrap();
        assert_eq!(n, "7");
    }

    #[test]
    fn jq_standard_library_is_available() {
        let out = run_query_text(COMPOSE, "[.services[].image] | sort", &json_opts()).unwrap();
        assert_eq!(out, r#"["nginx:1.27","postgres:16"]"#);
    }

    #[test]
    fn multi_document_each_and_slurp() {
        let stream = "kind: Service\nname: web\n---\nkind: Deployment\nname: api\n";
        let each = run_query(stream, ".kind", &json_opts()).unwrap();
        assert_eq!(each, vec![r#""Service""#, r#""Deployment""#]);

        let slurped = Options { documents: DocMode::Slurp, ..json_opts() };
        let all = run_query_text(stream, "map(.name)", &slurped).unwrap();
        assert_eq!(all, r#"["web","api"]"#);
    }

    #[test]
    fn json_input_is_accepted_by_auto_sniffing() {
        let out = run_query_text(r#"{"a":{"b":[1,2]}}"#, ".a.b | add", &json_opts()).unwrap();
        assert_eq!(out, "3");
    }

    #[test]
    fn explicit_json_input_format() {
        let opts = Options { input_format: InFormat::Json, ..json_opts() };
        let out = run_query_text(r#"{"a":1}"#, ".a", &opts).unwrap();
        assert_eq!(out, "1");
    }

    #[test]
    fn anchors_and_aliases_are_resolved() {
        let yaml = "defaults: &d\n  retries: 3\njob:\n  <<: *d\n  name: build\n";
        let out = run_query_text(yaml, ".job.retries", &json_opts()).unwrap();
        assert_eq!(out, "3");
    }

    #[test]
    fn custom_tags_unwrap_to_their_value() {
        let out = run_query_text("bucket: !Ref MyBucket\n", ".bucket", &json_opts()).unwrap();
        assert_eq!(out, r#""MyBucket""#);
    }

    #[test]
    fn non_string_mapping_keys_are_stringified() {
        let out = run_query_text("1: one\ntrue: yes\n", "keys", &json_opts()).unwrap();
        assert_eq!(out, r#"["1","true"]"#);
    }

    #[test]
    fn yaml_output_of_a_nested_object() {
        let out = run_query_text(COMPOSE, ".services.db", &Options::default()).unwrap();
        // `5432:5432` would re-read as a mapping, so the emitter quotes it.
        assert_eq!(out, "image: postgres:16\nports:\n- '5432:5432'");
    }

    #[test]
    fn enum_parsers_accept_aliases() {
        assert_eq!(InFormat::parse("YML").unwrap(), InFormat::Yaml);
        assert_eq!(OutFormat::parse("").unwrap(), OutFormat::Yaml);
        assert_eq!(DocMode::parse("slurp").unwrap(), DocMode::Slurp);
    }

    // ---- errors ----

    #[test]
    fn empty_input_errors() {
        let e = run_query_text("   ", ".", &Options::default()).unwrap_err();
        assert!(e.contains("no input"), "{e}");
    }

    #[test]
    fn empty_query_errors() {
        let e = run_query_text("a: 1\n", "  ", &Options::default()).unwrap_err();
        assert!(e.contains("no query"), "{e}");
    }

    #[test]
    fn invalid_yaml_errors() {
        let e = run_query_text("a: [1, 2\nb: 3\n", ".", &Options::default()).unwrap_err();
        assert!(e.contains("invalid YAML input"), "{e}");
    }

    #[test]
    fn invalid_json_errors_when_json_is_forced() {
        let opts = Options { input_format: InFormat::Json, ..Options::default() };
        let e = run_query_text("a: 1\n", ".", &opts).unwrap_err();
        assert!(e.contains("invalid JSON input"), "{e}");
    }

    #[test]
    fn query_parse_error() {
        let e = run_query_text("a: 1\n", ".a |", &Options::default()).unwrap_err();
        assert!(e.contains("query parse error"), "{e}");
    }

    #[test]
    fn query_runtime_error() {
        let e = run_query_text("a: 1\nb: x\n", ".a + .b", &Options::default()).unwrap_err();
        assert!(e.contains("query runtime error"), "{e}");
    }

    #[test]
    fn unknown_format_names_error() {
        assert!(InFormat::parse("xml").is_err());
        assert!(OutFormat::parse("toml").is_err());
        assert!(DocMode::parse("both").is_err());
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = format!("a: \"{}\"\n", "x".repeat(MAX_INPUT_BYTES));
        let e = run_query_text(&big, ".", &Options::default()).unwrap_err();
        assert!(e.contains("the limit is"), "{e}");
    }

    #[test]
    fn unbounded_generator_is_capped() {
        let e = run_query_text("a: 1\n", "range(1000000)", &Options::default()).unwrap_err();
        assert!(e.contains("more than"), "{e}");
    }

    #[test]
    fn empty_document_errors() {
        let e = run_query_text("# just a comment\n", ".", &Options::default()).unwrap_err();
        assert!(e.contains("no YAML documents"), "{e}");
    }
}
