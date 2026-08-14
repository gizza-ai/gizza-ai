//! config-merge core — merge layered configuration files that may each be in a
//! DIFFERENT format (JSON, YAML, TOML or `.env`) into one document, with
//! left-to-right override precedence and shell-style `${VAR}` substitution.
//! Pure compute, shared by the chat skill block and the web page (no
//! wafer/wasm-bindgen deps).
//!
//! Model:
//! * every layer is parsed into one universal `serde_json::Value` tree, so a
//!   `.env` layer can override a TOML base in a single pass;
//! * layers are merged left to right — `layer1` is the base, `layer4` has the
//!   final say;
//! * mappings merge recursively (or only at the top level when `object_merge`
//!   is `shallow`), arrays are replaced / appended / de-duplicated, and a key
//!   set to `null` in a later layer removes it unless `null_deletes` is off;
//! * after the merge, `${VAR}` / `${VAR:-default}` / `${VAR-default}` references
//!   inside string values are resolved against the explicit `vars` list first
//!   and then against the merged document itself.
//!
//! Data is normalized, not round-tripped verbatim: comments, blank lines and
//! YAML anchors are not part of the value model, so the output is re-emitted
//! canonically in the requested format.

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Most input bytes a single merge accepts, summed across the layers + vars.
pub const MAX_INPUT_BYTES: usize = 256 * 1024;
/// Deepest nesting level the merge/flatten walk will follow.
pub const MAX_DEPTH: usize = 64;
/// Separator that maps flat environment keys onto nested config paths.
pub const ENV_PATH_SEPARATOR: &str = "__";
/// How many times substitution re-walks the tree before giving up on a cycle.
pub const MAX_SUBSTITUTION_PASSES: usize = 10;
/// Names used for the four slots when `layer_names` leaves one blank.
pub const DEFAULT_LAYER_NAMES: [&str; 4] = ["base", "override", "environment", "local"];

/// A configuration syntax a layer can be written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Yaml,
    Toml,
    Env,
}

impl Format {
    pub fn name(&self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Yaml => "yaml",
            Format::Toml => "toml",
            Format::Env => "env",
        }
    }
}

/// How the layers' syntax is decided: sniffed per layer, or forced for all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    Auto,
    Fixed(Format),
}

/// The document the merge is rendered as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Yaml,
    Toml,
    Env,
    Report,
}

/// How nested mappings combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectMerge {
    Deep,
    Shallow,
}

/// How two lists at the same path combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayMerge {
    Replace,
    Append,
    Unique,
}

/// Whether a later layer's key may match an existing key of a different case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCase {
    /// `DEBUG` in a `.env` layer overrides `debug` in a JSON base, keeping the
    /// base's spelling. This is what makes an env layer able to override a
    /// lower-case config file at all.
    Match,
    /// Keys are compared byte for byte, so `DEBUG` and `debug` stay separate.
    Preserve,
}

fn parse_key_case(s: &str) -> Result<KeyCase, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "match" | "" => Ok(KeyCase::Match),
        "preserve" => Ok(KeyCase::Preserve),
        other => Err(format!(
            "unknown key_case '{other}' (use match or preserve)"
        )),
    }
}

fn parse_input_format(s: &str) -> Result<InputFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "auto" | "" => Ok(InputFormat::Auto),
        "json" => Ok(InputFormat::Fixed(Format::Json)),
        "yaml" | "yml" => Ok(InputFormat::Fixed(Format::Yaml)),
        "toml" => Ok(InputFormat::Fixed(Format::Toml)),
        "env" | "dotenv" => Ok(InputFormat::Fixed(Format::Env)),
        other => Err(format!(
            "unknown input_format '{other}' (use auto, json, yaml, toml or env)"
        )),
    }
}

fn parse_output_format(s: &str) -> Result<OutputFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "json" | "" => Ok(OutputFormat::Json),
        "yaml" | "yml" => Ok(OutputFormat::Yaml),
        "toml" => Ok(OutputFormat::Toml),
        "env" | "dotenv" => Ok(OutputFormat::Env),
        "report" => Ok(OutputFormat::Report),
        other => Err(format!(
            "unknown output '{other}' (use json, yaml, toml, env or report)"
        )),
    }
}

fn parse_object_merge(s: &str) -> Result<ObjectMerge, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "deep" | "" => Ok(ObjectMerge::Deep),
        "shallow" => Ok(ObjectMerge::Shallow),
        other => Err(format!(
            "unknown object_merge '{other}' (use deep or shallow)"
        )),
    }
}

fn parse_array_merge(s: &str) -> Result<ArrayMerge, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "replace" | "" => Ok(ArrayMerge::Replace),
        "append" | "concat" => Ok(ArrayMerge::Append),
        "unique" => Ok(ArrayMerge::Unique),
        other => Err(format!(
            "unknown array_merge '{other}' (use replace, append or unique)"
        )),
    }
}

/// One merge request, straight from a surface's params.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// The four layer bodies, lowest precedence first. Blank slots are skipped.
    pub layers: [&'a str; 4],
    /// Comma-separated display names for the four slots.
    pub layer_names: &'a str,
    pub input_format: &'a str,
    pub output: &'a str,
    pub object_merge: &'a str,
    pub array_merge: &'a str,
    pub key_case: &'a str,
    pub null_deletes: bool,
    pub substitute: bool,
    /// Extra `KEY=VALUE` substitution variables that never appear in the output.
    pub vars: &'a str,
    pub sort_keys: bool,
    pub indent: usize,
}

/// A layer that actually carried content.
struct Layer {
    name: String,
    format: Format,
    tree: Value,
}

/// Everything the report needs that the merged tree alone can't say.
#[derive(Default)]
struct Trace {
    /// path → indices (into the used-layer list) of every layer that set it.
    sources: BTreeMap<String, Vec<usize>>,
    /// path → the layer index whose `null` removed it.
    removed: Vec<(String, usize)>,
    /// (path, expression, where the value came from).
    substituted: Vec<(String, String, String)>,
    /// (path, expression) references that had no value and no default.
    unresolved: Vec<(String, String)>,
}

/// Merge the request's layers and render the result.
pub fn merge_config(req: Request) -> Result<String, String> {
    let total: usize = req.layers.iter().map(|l| l.len()).sum::<usize>() + req.vars.len();
    if total > MAX_INPUT_BYTES {
        return Err(format!(
            "input too large: {total} bytes across the layers and vars (max {MAX_INPUT_BYTES})"
        ));
    }
    let indent = req.indent.clamp(1, 8);
    let input_format = parse_input_format(req.input_format)?;
    let output = parse_output_format(req.output)?;
    let rules = MergeRules {
        object_merge: parse_object_merge(req.object_merge)?,
        array_merge: parse_array_merge(req.array_merge)?,
        key_case: parse_key_case(req.key_case)?,
        null_deletes: req.null_deletes,
    };

    let names = resolve_layer_names(req.layer_names);
    let mut layers: Vec<Layer> = Vec::new();
    for (i, body) in req.layers.iter().enumerate() {
        if body.trim().is_empty() {
            continue;
        }
        let format = match input_format {
            InputFormat::Fixed(f) => f,
            InputFormat::Auto => detect_format(body),
        };
        let tree = parse_layer(body, format)
            .map_err(|e| format!("layer {} ({}): {e}", i + 1, names[i]))?;
        if !tree.is_object() {
            return Err(format!(
                "layer {} ({}) must be a mapping at the top level — got {}. Wrap bare lists or scalars in a key.",
                i + 1,
                names[i],
                type_name(&tree)
            ));
        }
        layers.push(Layer {
            name: names[i].clone(),
            format,
            tree,
        });
    }
    if layers.is_empty() {
        return Err("no input: paste at least one configuration layer".into());
    }

    let mut trace = Trace::default();
    let mut merged = Map::new();
    for (idx, layer) in layers.iter().enumerate() {
        let Value::Object(over) = layer.tree.clone() else {
            unreachable!("layers are validated as mappings above")
        };
        merge_into(&mut merged, over, &rules, "", idx, 0, &mut trace)?;
    }
    let mut merged = Value::Object(merged);

    if req.sort_keys {
        sort_tree(&mut merged);
    }
    if req.substitute {
        substitute_tree(&mut merged, req.vars, &mut trace)?;
    }

    match output {
        OutputFormat::Json => render_json(&merged, indent),
        OutputFormat::Yaml => render_yaml(&merged, indent),
        OutputFormat::Toml => render_toml(&merged),
        OutputFormat::Env => render_env(&merged),
        OutputFormat::Report => render_report(&merged, &layers, &trace),
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "a list",
        Value::Object(_) => "a mapping",
    }
}

fn resolve_layer_names(raw: &str) -> [String; 4] {
    let mut out = DEFAULT_LAYER_NAMES.map(|s| s.to_string());
    for (i, part) in raw.split(',').enumerate() {
        if i >= 4 {
            break;
        }
        let part = part.trim();
        if !part.is_empty() {
            out[i] = part.to_string();
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Format detection + parsing
// ---------------------------------------------------------------------------

/// Sniff a layer's syntax. The order is deliberate: JSON is unambiguous, a TOML
/// table header (`[section]`) is unambiguous, a flat `KEY=value` block with no
/// spaces around `=` is the `.env` convention, and TOML idiomatically writes
/// `key = value` WITH spaces — anything left is treated as YAML.
pub fn detect_format(text: &str) -> Format {
    let trimmed = text.trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && serde_json::from_str::<Value>(text).is_ok()
    {
        return Format::Json;
    }
    let content: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if content.is_empty() {
        return Format::Yaml;
    }
    if content
        .iter()
        .any(|l| l.starts_with('[') && l.ends_with(']'))
    {
        return Format::Toml;
    }
    if content.iter().all(|l| is_env_line(l)) {
        return Format::Env;
    }
    if content.iter().all(|l| l.contains('=')) && toml::from_str::<toml::Value>(text).is_ok() {
        return Format::Toml;
    }
    Format::Yaml
}

/// `KEY=value` / `export KEY=value` with NO whitespace before the `=`.
fn is_env_line(line: &str) -> bool {
    let body = line.strip_prefix("export ").unwrap_or(line).trim_start();
    let Some(eq) = body.find('=') else {
        return false;
    };
    let key = &body[..eq];
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        && !key.starts_with(|c: char| c.is_ascii_digit())
}

fn parse_layer(text: &str, format: Format) -> Result<Value, String> {
    match format {
        Format::Json => serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}")),
        Format::Yaml => serde_yml::from_str(text).map_err(|e| format!("invalid YAML: {e}")),
        Format::Toml => toml::from_str(text).map_err(|e| format!("invalid TOML: {e}")),
        Format::Env => parse_env(text),
    }
}

/// Parse a `.env` body into a nested tree, splitting keys on `__`.
fn parse_env(text: &str) -> Result<Value, String> {
    let mut root = Map::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let body = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some(eq) = body.find('=') else {
            return Err(format!(
                "line {}: expected KEY=VALUE, got '{}'",
                lineno + 1,
                truncate(line, 40)
            ));
        };
        let key = body[..eq].trim();
        if key.is_empty() {
            return Err(format!("line {}: empty key before '='", lineno + 1));
        }
        let value = parse_env_value(body[eq + 1..].trim_start(), lineno + 1)?;
        let path: Vec<&str> = key.split(ENV_PATH_SEPARATOR).collect();
        let path: Vec<&str> = if path.iter().any(|s| s.is_empty()) {
            vec![key]
        } else {
            path
        };
        insert_path(&mut root, &path, Value::String(value), key)?;
    }
    Ok(Value::Object(root))
}

fn parse_env_value(rest: &str, lineno: usize) -> Result<String, String> {
    let mut chars = rest.chars().peekable();
    match chars.peek() {
        Some('"') => {
            chars.next();
            let mut out = String::new();
            let mut closed = false;
            while let Some(c) = chars.next() {
                match c {
                    '\\' => match chars.next() {
                        Some('n') => out.push('\n'),
                        Some('t') => out.push('\t'),
                        Some('r') => out.push('\r'),
                        Some(other) => out.push(other),
                        None => return Err(format!("line {lineno}: trailing backslash")),
                    },
                    '"' => {
                        closed = true;
                        break;
                    }
                    other => out.push(other),
                }
            }
            if !closed {
                return Err(format!("line {lineno}: unterminated double-quoted value"));
            }
            Ok(out)
        }
        Some('\'') => {
            chars.next();
            let mut out = String::new();
            let mut closed = false;
            for c in chars.by_ref() {
                if c == '\'' {
                    closed = true;
                    break;
                }
                out.push(c);
            }
            if !closed {
                return Err(format!("line {lineno}: unterminated single-quoted value"));
            }
            Ok(out)
        }
        _ => {
            // Unquoted: an inline comment starts at " #".
            let raw: String = chars.collect();
            let cut = raw.find(" #").unwrap_or(raw.len());
            Ok(raw[..cut].trim_end().to_string())
        }
    }
}

fn insert_path(
    root: &mut Map<String, Value>,
    path: &[&str],
    value: Value,
    key: &str,
) -> Result<(), String> {
    let mut cursor = root;
    for (i, seg) in path.iter().enumerate() {
        if i + 1 == path.len() {
            cursor.insert((*seg).to_string(), value);
            return Ok(());
        }
        let entry = cursor
            .entry((*seg).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            return Err(format!(
                "key '{key}' needs '{seg}' to be a mapping, but it already holds {}",
                type_name(entry)
            ));
        }
        cursor = entry.as_object_mut().expect("checked above");
    }
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// The merge knobs, resolved once per request.
#[derive(Debug, Clone, Copy)]
struct MergeRules {
    object_merge: ObjectMerge,
    array_merge: ArrayMerge,
    key_case: KeyCase,
    null_deletes: bool,
}

/// The key in `base_map` that `key` should land on: itself, or — under
/// `KeyCase::Match` — an existing key that differs only in case.
fn canonical_key(base_map: &Map<String, Value>, key: &str, rules: &MergeRules) -> Option<String> {
    if base_map.contains_key(key) {
        return Some(key.to_string());
    }
    if rules.key_case == KeyCase::Match {
        return base_map
            .keys()
            .find(|k| k.eq_ignore_ascii_case(key))
            .cloned();
    }
    None
}

fn merge_into(
    base_map: &mut Map<String, Value>,
    over_map: Map<String, Value>,
    rules: &MergeRules,
    path: &str,
    layer: usize,
    depth: usize,
    trace: &mut Trace,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "nesting deeper than {MAX_DEPTH} levels at '{path}' — refusing to merge"
        ));
    }
    for (raw_key, over_val) in over_map {
        let existing = canonical_key(base_map, &raw_key, rules);
        let key = existing.clone().unwrap_or_else(|| raw_key.clone());
        let child_path = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };

        if over_val.is_null() && rules.null_deletes {
            if base_map.shift_remove(&key).is_some() {
                trace.removed.push((child_path, layer));
            }
            continue;
        }
        let Some(_) = existing else {
            record_subtree(&over_val, &child_path, layer, trace)?;
            base_map.insert(key, over_val);
            continue;
        };
        let base_val = base_map.get_mut(&key).expect("canonical_key found it");
        let both_objects = base_val.is_object() && over_val.is_object();
        let both_arrays = base_val.is_array() && over_val.is_array();
        if both_objects && rules.object_merge == ObjectMerge::Deep {
            let (Value::Object(base_child), Value::Object(over_child)) = (&mut *base_val, over_val)
            else {
                unreachable!("checked is_object above")
            };
            merge_into(
                base_child,
                over_child,
                rules,
                &child_path,
                layer,
                depth + 1,
                trace,
            )?;
        } else if both_arrays {
            let (Value::Array(a), Value::Array(b)) = (&mut *base_val, over_val) else {
                unreachable!("checked is_array above")
            };
            match rules.array_merge {
                ArrayMerge::Replace => *a = b,
                ArrayMerge::Append => a.extend(b),
                ArrayMerge::Unique => {
                    for item in b {
                        if !a.contains(&item) {
                            a.push(item);
                        }
                    }
                }
            }
            trace.sources.entry(child_path).or_default().push(layer);
        } else {
            record_subtree(&over_val, &child_path, layer, trace)?;
            *base_val = over_val;
        }
    }
    Ok(())
}

/// Note every leaf path a layer set, under the path the value actually landed
/// on, so the report can name the winner even when key_case rewrote the key.
fn record_subtree(
    value: &Value,
    path: &str,
    layer: usize,
    trace: &mut Trace,
) -> Result<(), String> {
    let mut leaves = Vec::new();
    flatten_leaves(value, path, &mut leaves, 0)?;
    for (p, _) in leaves {
        let entry = trace.sources.entry(p).or_default();
        if entry.last() != Some(&layer) {
            entry.push(layer);
        }
    }
    Ok(())
}

fn flatten_leaves(
    v: &Value,
    path: &str,
    out: &mut Vec<(String, Value)>,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "nesting deeper than {MAX_DEPTH} levels at '{path}'"
        ));
    }
    match v {
        Value::Object(map) if !map.is_empty() => {
            for (k, child) in map {
                let child_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                flatten_leaves(child, &child_path, out, depth + 1)?;
            }
        }
        other => out.push((path.to_string(), other.clone())),
    }
    Ok(())
}

fn sort_tree(v: &mut Value) {
    match v {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, child) in entries.iter_mut() {
                sort_tree(child);
            }
            *map = entries.into_iter().collect();
        }
        Value::Array(items) => items.iter_mut().for_each(sort_tree),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Variable substitution
// ---------------------------------------------------------------------------

fn parse_vars(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let body = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some(eq) = body.find('=') else {
            return Err(format!(
                "vars line {}: expected KEY=VALUE, got '{}'",
                lineno + 1,
                truncate(line, 40)
            ));
        };
        let key = body[..eq].trim();
        if key.is_empty() {
            return Err(format!("vars line {}: empty key before '='", lineno + 1));
        }
        out.insert(
            key.to_string(),
            parse_env_value(body[eq + 1..].trim_start(), lineno + 1)?,
        );
    }
    Ok(out)
}

/// Values the merged document itself can supply, keyed by both `a.b.c` and
/// `A__B__C` so env-shaped and config-shaped references both resolve.
fn build_lookup(vars: &BTreeMap<String, String>, tree: &Value) -> BTreeMap<String, String> {
    let mut lookup = BTreeMap::new();
    let mut leaves = Vec::new();
    let _ = flatten_leaves(tree, "", &mut leaves, 0);
    for (path, value) in leaves {
        let text = match &value {
            Value::String(s) => s.clone(),
            Value::Null | Value::Array(_) | Value::Object(_) => continue,
            other => other.to_string(),
        };
        lookup.insert(path.replace('.', ENV_PATH_SEPARATOR), text.clone());
        lookup.insert(path, text);
    }
    // Explicit vars outrank anything the document itself defines.
    for (k, v) in vars {
        lookup.insert(k.clone(), v.clone());
    }
    lookup
}

fn substitute_tree(tree: &mut Value, vars_text: &str, trace: &mut Trace) -> Result<(), String> {
    let vars = parse_vars(vars_text)?;
    for _ in 0..MAX_SUBSTITUTION_PASSES {
        let lookup = build_lookup(&vars, tree);
        let mut changed = false;
        // Resolved references accumulate (each one is expanded in exactly one
        // pass); the unresolved set is only true on the final, quiet pass.
        trace.unresolved.clear();
        substitute_node(tree, "", &lookup, &vars, &mut changed, trace)?;
        if !changed {
            return Ok(());
        }
    }
    Err(format!(
        "variable substitution did not settle after {MAX_SUBSTITUTION_PASSES} passes — check for a reference cycle such as A=${{B}} and B=${{A}}"
    ))
}

fn substitute_node(
    node: &mut Value,
    path: &str,
    lookup: &BTreeMap<String, String>,
    vars: &BTreeMap<String, String>,
    changed: &mut bool,
    trace: &mut Trace,
) -> Result<(), String> {
    match node {
        Value::String(s) => {
            let replaced = expand(s, path, lookup, vars, trace);
            if &replaced != s {
                *s = replaced;
                *changed = true;
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter_mut().enumerate() {
                let child = format!("{path}[{i}]");
                substitute_node(item, &child, lookup, vars, changed, trace)?;
            }
        }
        Value::Object(map) => {
            for (k, child) in map.iter_mut() {
                let child_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                substitute_node(child, &child_path, lookup, vars, changed, trace)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Expand `${VAR}`, `${VAR:-default}` and `${VAR-default}` in one string.
/// `$$` is a literal `$`; a bare `$VAR` is left alone so passwords and regexes
/// survive untouched; an unresolvable reference is left verbatim and reported.
fn expand(
    input: &str,
    path: &str,
    lookup: &BTreeMap<String, String>,
    vars: &BTreeMap<String, String>,
    trace: &mut Trace,
) -> String {
    let bytes: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != '$' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i + 1] == '$' {
            out.push('$');
            i += 2;
            continue;
        }
        if i + 1 >= bytes.len() || bytes[i + 1] != '{' {
            out.push('$');
            i += 1;
            continue;
        }
        let Some(close) = bytes[i + 2..].iter().position(|c| *c == '}') else {
            out.push('$');
            i += 1;
            continue;
        };
        let expr: String = bytes[i + 2..i + 2 + close].iter().collect();
        i += close + 3;
        let (name, default, empty_counts) = split_expr(&expr);
        let found = lookup.get(name);
        let use_default = match found {
            None => true,
            Some(v) => empty_counts && v.is_empty(),
        };
        match (use_default, default) {
            (false, _) => {
                let value = found.expect("present when use_default is false").clone();
                let origin = if vars.contains_key(name) {
                    "vars"
                } else {
                    "merged config"
                };
                trace.substituted.push((
                    path.to_string(),
                    format!("${{{expr}}}"),
                    origin.to_string(),
                ));
                out.push_str(&value);
            }
            (true, Some(d)) => {
                trace.substituted.push((
                    path.to_string(),
                    format!("${{{expr}}}"),
                    "default".to_string(),
                ));
                out.push_str(d);
            }
            (true, None) => {
                trace
                    .unresolved
                    .push((path.to_string(), format!("${{{expr}}}")));
                out.push_str(&format!("${{{expr}}}"));
            }
        }
    }
    out
}

/// `NAME`, `NAME:-default` (default when unset OR empty), `NAME-default`
/// (default only when unset).
fn split_expr(expr: &str) -> (&str, Option<&str>, bool) {
    if let Some(pos) = expr.find(":-") {
        (&expr[..pos], Some(&expr[pos + 2..]), true)
    } else if let Some(pos) = expr.find('-') {
        (&expr[..pos], Some(&expr[pos + 1..]), false)
    } else {
        (expr, None, false)
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_json(value: &Value, indent: usize) -> Result<String, String> {
    let pad = " ".repeat(indent);
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(pad.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    value
        .serialize(&mut ser)
        .map_err(|e| format!("JSON encode: {e}"))?;
    let mut out = String::from_utf8(buf).map_err(|e| format!("JSON encode: {e}"))?;
    out.push('\n');
    Ok(out)
}

fn render_yaml(value: &Value, indent: usize) -> Result<String, String> {
    if value == &Value::Object(Map::new()) {
        return Ok("{}\n".to_string());
    }
    let mut out = String::new();
    write_yaml_value(value, 0, indent, &mut out)?;
    Ok(out)
}

fn write_yaml_value(
    value: &Value,
    depth: usize,
    indent: usize,
    out: &mut String,
) -> Result<(), String> {
    let pad = " ".repeat(depth * indent);
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key.contains('\n') || key.contains(':') {
                    return Err(format!(
                        "YAML output cannot safely render key '{key}' — choose JSON output"
                    ));
                }
                match child {
                    Value::Object(m) if !m.is_empty() => {
                        out.push_str(&format!("{pad}{key}:\n"));
                        write_yaml_value(child, depth + 1, indent, out)?;
                    }
                    Value::Array(items) if !items.is_empty() => {
                        out.push_str(&format!("{pad}{key}:\n"));
                        write_yaml_array(items, depth + 1, indent, out)?;
                    }
                    _ => out.push_str(&format!("{pad}{key}: {}\n", yaml_scalar(child)?)),
                }
            }
        }
        Value::Array(items) => write_yaml_array(items, depth, indent, out)?,
        scalar => out.push_str(&format!("{pad}{}\n", yaml_scalar(scalar)?)),
    }
    Ok(())
}

fn write_yaml_array(
    items: &[Value],
    depth: usize,
    indent: usize,
    out: &mut String,
) -> Result<(), String> {
    let pad = " ".repeat(depth * indent);
    for item in items {
        match item {
            Value::Object(m) if !m.is_empty() => {
                out.push_str(&format!("{pad}-\n"));
                write_yaml_value(item, depth + 1, indent, out)?;
            }
            Value::Array(nested) if !nested.is_empty() => {
                out.push_str(&format!("{pad}-\n"));
                write_yaml_array(nested, depth + 1, indent, out)?;
            }
            _ => out.push_str(&format!("{pad}- {}\n", yaml_scalar(item)?)),
        }
    }
    Ok(())
}

fn yaml_scalar(value: &Value) -> Result<String, String> {
    Ok(match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if s.is_empty()
                || s.starts_with(char::is_whitespace)
                || s.ends_with(char::is_whitespace)
                || s.contains('\n')
                || s.contains('#')
                || s.contains(':')
                || matches!(s.as_str(), "true" | "false" | "null")
                || s.parse::<f64>().is_ok()
            {
                serde_json::to_string(s).map_err(|e| format!("YAML encode: {e}"))?
            } else {
                s.clone()
            }
        }
        Value::Array(_) | Value::Object(_) => "{}".to_string(),
    })
}

fn render_toml(value: &Value) -> Result<String, String> {
    let mut stripped = value.clone();
    strip_nulls(&mut stripped);
    toml_reorder(&mut stripped);
    toml::to_string_pretty(&stripped).map_err(|e| {
        format!("TOML encode: {e} (TOML needs a table root and cannot represent null — nulls are dropped)")
    })
}

fn strip_nulls(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let keys: Vec<String> = map
                .iter()
                .filter(|(_, v)| v.is_null())
                .map(|(k, _)| k.clone())
                .collect();
            for k in keys {
                map.shift_remove(&k);
            }
            for (_, v) in map.iter_mut() {
                strip_nulls(v);
            }
        }
        Value::Array(items) => {
            items.retain(|v| !v.is_null());
            items.iter_mut().for_each(strip_nulls);
        }
        _ => {}
    }
}

/// TOML's serializer refuses to write a scalar after a table, so emit every
/// map's plain values first and its sub-tables afterwards.
fn toml_reorder(value: &mut Value) {
    if let Value::Object(map) = value {
        let entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
        let (tables, plains): (Vec<_>, Vec<_>) = entries.into_iter().partition(is_toml_table);
        let mut rebuilt: Map<String, Value> = Map::new();
        for (k, mut v) in plains {
            toml_reorder(&mut v);
            rebuilt.insert(k, v);
        }
        for (k, mut v) in tables {
            toml_reorder(&mut v);
            rebuilt.insert(k, v);
        }
        *map = rebuilt;
    } else if let Value::Array(items) = value {
        items.iter_mut().for_each(toml_reorder);
    }
}

fn is_toml_table(entry: &(String, Value)) -> bool {
    match &entry.1 {
        Value::Object(_) => true,
        Value::Array(items) => items.iter().any(Value::is_object),
        _ => false,
    }
}

fn render_env(value: &Value) -> Result<String, String> {
    let mut leaves = Vec::new();
    flatten_leaves(value, "", &mut leaves, 0)?;
    let mut out = String::new();
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for (path, leaf) in leaves {
        match leaf {
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    emit_env_line(&format!("{path}.{i}"), item, &mut out, &mut seen)?;
                }
            }
            other => emit_env_line(&path, &other, &mut out, &mut seen)?,
        }
    }
    Ok(out)
}

fn emit_env_line(
    path: &str,
    value: &Value,
    out: &mut String,
    seen: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    if value.is_object() || value.is_array() {
        // Nested containers inside a list can't be flattened to one key.
        return Err(format!(
            "env output cannot represent the nested value at '{path}' — use json, yaml or toml for lists of mappings"
        ));
    }
    let key = path.replace('.', ENV_PATH_SEPARATOR).to_ascii_uppercase();
    if let Some(other) = seen.get(&key) {
        if other != path {
            return Err(format!(
                "env output key collision: '{other}' and '{path}' both become {key} — rename one or pick another output format"
            ));
        }
    }
    seen.insert(key.clone(), path.to_string());
    let text = match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let needs_quotes = text.is_empty()
        || text
            .contains(|c: char| c.is_whitespace() || c == '#' || c == '"' || c == '\'' || c == '$');
    if needs_quotes {
        let escaped = text
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        out.push_str(&format!("{key}=\"{escaped}\"\n"));
    } else {
        out.push_str(&format!("{key}={text}\n"));
    }
    Ok(())
}

fn render_report(merged: &Value, layers: &[Layer], trace: &Trace) -> Result<String, String> {
    let mut out = String::new();
    out.push_str(&format!(
        "# Merged configuration — {} layer{}\n",
        layers.len(),
        if layers.len() == 1 { "" } else { "s" }
    ));
    out.push_str("# Lowest to highest precedence: ");
    let chain: Vec<String> = layers
        .iter()
        .map(|l| format!("{} ({})", l.name, l.format.name()))
        .collect();
    out.push_str(&chain.join(" -> "));
    out.push_str("\n\n");

    let mut leaves = Vec::new();
    flatten_leaves(merged, "", &mut leaves, 0)?;
    if leaves.is_empty() {
        out.push_str("(the merged configuration is empty)\n");
    }
    let width = leaves.iter().map(|(p, _)| p.len()).max().unwrap_or(0);
    let mut overrides: Vec<String> = Vec::new();
    for (path, value) in &leaves {
        let rendered = serde_json::to_string(value).map_err(|e| format!("JSON encode: {e}"))?;
        let setters = trace.sources.get(path);
        let comment = match setters {
            Some(ids) if !ids.is_empty() => {
                let winner = &layers[*ids.last().expect("non-empty")].name;
                if ids.len() > 1 {
                    let chain: Vec<&str> = ids.iter().map(|i| layers[*i].name.as_str()).collect();
                    overrides.push(format!("{path}: {}", chain.join(" -> ")));
                    format!("# {winner} (overrides {})", ids.len() - 1)
                } else {
                    format!("# {winner}")
                }
            }
            _ => "# substituted".to_string(),
        };
        out.push_str(&format!("{path:<width$} = {rendered}  {comment}\n"));
    }

    if !overrides.is_empty() {
        out.push_str(&format!("\n# Overridden keys ({})\n", overrides.len()));
        for line in &overrides {
            out.push_str(&format!("{line}\n"));
        }
    }
    if !trace.removed.is_empty() {
        out.push_str(&format!("\n# Removed by null ({})\n", trace.removed.len()));
        for (path, layer) in &trace.removed {
            out.push_str(&format!("{path}: null in {}\n", layers[*layer].name));
        }
    }
    if !trace.substituted.is_empty() {
        out.push_str(&format!(
            "\n# Substitutions ({})\n",
            trace.substituted.len()
        ));
        for (path, expr, origin) in &trace.substituted {
            out.push_str(&format!("{path}: {expr} from {origin}\n"));
        }
    }
    if !trace.unresolved.is_empty() {
        out.push_str(&format!("\n# Unresolved ({})\n", trace.unresolved.len()));
        for (path, expr) in &trace.unresolved {
            out.push_str(&format!(
                "{path}: {expr} has no value and no default — left as written\n"
            ));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req<'a>(layers: [&'a str; 4]) -> Request<'a> {
        Request {
            layers,
            layer_names: "",
            input_format: "auto",
            output: "json",
            object_merge: "deep",
            array_merge: "replace",
            key_case: "match",
            null_deletes: true,
            substitute: true,
            vars: "",
            sort_keys: false,
            indent: 2,
        }
    }

    #[test]
    fn merges_json_base_with_yaml_and_env_layers() {
        let mut r = req([
            r#"{"db":{"host":"localhost","port":5432},"debug":false}"#,
            "db:\n  host: db.internal\n",
            "DEBUG=true\nDB__PORT=6543\n",
            "",
        ]);
        r.output = "json";
        let out = merge_config(r).unwrap();
        assert_eq!(
            out,
            "{\n  \"db\": {\n    \"host\": \"db.internal\",\n    \"port\": \"6543\"\n  },\n  \"debug\": \"true\"\n}\n"
        );
    }

    #[test]
    fn key_case_preserve_keeps_env_keys_separate() {
        let mut r = req([r#"{"debug":false}"#, "DEBUG=true\n", "", ""]);
        r.key_case = "preserve";
        let out = merge_config(r).unwrap();
        assert!(out.contains(r#""debug": false"#), "{out}");
        assert!(out.contains(r#""DEBUG": "true""#), "{out}");
    }

    #[test]
    fn typed_flat_layer_can_be_forced_to_toml_to_keep_numbers() {
        // env values are text by definition; forcing TOML gives typed scalars.
        let auto = req([r#"{"port":80}"#, "port=8080\n", "", ""]);
        assert_eq!(merge_config(auto).unwrap(), "{\n  \"port\": \"8080\"\n}\n");
        let mut forced = req(["port = 80\n", "port=8080\n", "", ""]);
        forced.input_format = "toml";
        assert_eq!(merge_config(forced).unwrap(), "{\n  \"port\": 8080\n}\n");
    }

    #[test]
    fn rejects_a_non_mapping_layer() {
        let err = merge_config(req(["[1, 2, 3]", "", "", ""])).unwrap_err();
        assert!(err.contains("must be a mapping at the top level"), "{err}");
        assert!(err.contains("a list"), "{err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = merge_config(req(["", "  ", "\n", ""])).unwrap_err();
        assert!(err.contains("no input"), "{err}");
    }

    #[test]
    fn detects_each_format() {
        assert_eq!(detect_format("{\"a\":1}"), Format::Json);
        assert_eq!(detect_format("[server]\nport = 80\n"), Format::Toml);
        assert_eq!(detect_format("port = 80\nname = \"api\"\n"), Format::Toml);
        assert_eq!(detect_format("PORT=80\nexport NAME=api\n"), Format::Env);
        assert_eq!(detect_format("port: 80\nname: api\n"), Format::Yaml);
        assert_eq!(detect_format("DSN=postgres://u:p@h/db\n"), Format::Env);
    }

    #[test]
    fn input_format_override_beats_detection() {
        let mut r = req(["PORT=80\n", "", "", ""]);
        r.input_format = "toml";
        r.output = "json";
        let out = merge_config(r).unwrap();
        assert_eq!(out, "{\n  \"PORT\": 80\n}\n");
    }

    #[test]
    fn null_deletes_an_inherited_key() {
        let mut r = req([
            r#"{"cache":{"ttl":60,"size":10}}"#,
            "cache:\n  ttl: null\n",
            "",
            "",
        ]);
        r.output = "json";
        assert_eq!(
            merge_config(r).unwrap(),
            "{\n  \"cache\": {\n    \"size\": 10\n  }\n}\n"
        );
    }

    #[test]
    fn null_kept_when_deletion_is_off() {
        let mut r = req([r#"{"cache":{"ttl":60}}"#, "cache:\n  ttl: null\n", "", ""]);
        r.null_deletes = false;
        assert_eq!(
            merge_config(r).unwrap(),
            "{\n  \"cache\": {\n    \"ttl\": null\n  }\n}\n"
        );
    }

    #[test]
    fn shallow_merge_replaces_the_whole_subtree() {
        let mut r = req([
            r#"{"db":{"host":"a","port":1}}"#,
            r#"{"db":{"host":"b"}}"#,
            "",
            "",
        ]);
        r.object_merge = "shallow";
        assert_eq!(
            merge_config(r).unwrap(),
            "{\n  \"db\": {\n    \"host\": \"b\"\n  }\n}\n"
        );
    }

    #[test]
    fn array_strategies() {
        let base = r#"{"tags":["api","stable"]}"#;
        let over = r#"{"tags":["stable","canary"]}"#;
        let mut r = req([base, over, "", ""]);
        r.array_merge = "replace";
        assert!(merge_config(r.clone())
            .unwrap()
            .contains("\"stable\",\n    \"canary\""));
        r.array_merge = "append";
        let appended = merge_config(r.clone()).unwrap();
        assert_eq!(appended.matches("stable").count(), 2, "{appended}");
        r.array_merge = "unique";
        let unique = merge_config(r).unwrap();
        assert_eq!(unique.matches("stable").count(), 1, "{unique}");
    }

    #[test]
    fn substitutes_vars_defaults_and_config_references() {
        let mut r = req([
            r#"{"host":"db.internal","dsn":"postgres://${DB_USER}:${DB_PASSWORD}@${host}/${DB_NAME:-app}"}"#,
            "",
            "",
            "",
        ]);
        r.vars = "DB_USER=svc\nDB_PASSWORD=s3cr3t\n";
        let out = merge_config(r).unwrap();
        assert!(
            out.contains("postgres://svc:s3cr3t@db.internal/app"),
            "{out}"
        );
    }

    #[test]
    fn unresolved_reference_is_left_verbatim() {
        let mut r = req([r#"{"url":"https://${MISSING}/x"}"#, "", "", ""]);
        r.output = "report";
        let out = merge_config(r).unwrap();
        assert!(out.contains("https://${MISSING}/x"), "{out}");
        assert!(out.contains("# Unresolved (1)"), "{out}");
    }

    #[test]
    fn dollar_dollar_escapes_and_bare_dollar_is_literal() {
        let mut r = req([r#"{"a":"$${NOPE}","b":"pa$$word","c":"$HOME"}"#, "", "", ""]);
        r.output = "json";
        let out = merge_config(r).unwrap();
        assert!(out.contains(r#""a": "${NOPE}""#), "{out}");
        assert!(out.contains(r#""b": "pa$word""#), "{out}");
        assert!(out.contains(r#""c": "$HOME""#), "{out}");
    }

    #[test]
    fn substitution_cycle_is_reported() {
        let mut r = req([r#"{"a":"x${b}","b":"y${a}"}"#, "", "", ""]);
        r.output = "json";
        let err = merge_config(r).unwrap_err();
        assert!(err.contains("did not settle"), "{err}");
    }

    #[test]
    fn substitution_can_be_disabled() {
        let mut r = req([r#"{"a":"${b}","b":"x"}"#, "", "", ""]);
        r.substitute = false;
        let out = merge_config(r).unwrap();
        assert!(out.contains(r#""a": "${b}""#), "{out}");
    }

    #[test]
    fn renders_yaml_toml_and_env() {
        let layers = [r#"{"port":8080,"db":{"host":"h"}}"#, "", "", ""];
        let mut r = req(layers);
        r.output = "yaml";
        assert_eq!(
            merge_config(r.clone()).unwrap(),
            "port: 8080\ndb:\n  host: h\n"
        );
        r.output = "toml";
        assert_eq!(
            merge_config(r.clone()).unwrap(),
            "port = 8080\n\n[db]\nhost = \"h\"\n"
        );
        r.output = "env";
        assert_eq!(merge_config(r).unwrap(), "PORT=8080\nDB__HOST=h\n");
    }

    #[test]
    fn toml_output_emits_scalars_before_tables() {
        // The base sets a table first; TOML would refuse this order unmodified.
        let mut r = req([r#"{"db":{"host":"h"},"port":80}"#, "", "", ""]);
        r.output = "toml";
        let out = merge_config(r).unwrap();
        assert!(out.starts_with("port = 80"), "{out}");
    }

    #[test]
    fn env_output_rejects_a_list_of_mappings() {
        let mut r = req([r#"{"items":[{"a":1}]}"#, "", "", ""]);
        r.output = "env";
        let err = merge_config(r).unwrap_err();
        assert!(err.contains("cannot represent the nested value"), "{err}");
    }

    #[test]
    fn env_output_reports_key_collisions() {
        let mut r = req([r#"{"port":1,"PORT":2}"#, "", "", ""]);
        r.output = "env";
        r.key_case = "preserve";
        let err = merge_config(r).unwrap_err();
        assert!(err.contains("collision"), "{err}");
    }

    #[test]
    fn report_names_the_layer_that_set_each_value() {
        let mut r = req([
            r#"{"db":{"host":"localhost","port":5432}}"#,
            "db:\n  host: prod.db\n",
            "",
            "",
        ]);
        r.output = "report";
        r.layer_names = "defaults.json,prod.yaml";
        let out = merge_config(r).unwrap();
        assert!(
            out.contains("defaults.json (json) -> prod.yaml (yaml)"),
            "{out}"
        );
        assert!(
            out.contains("db.host = \"prod.db\"  # prod.yaml (overrides 1)"),
            "{out}"
        );
        assert!(out.contains("db.port = 5432  # defaults.json"), "{out}");
        assert!(out.contains("db.host: defaults.json -> prod.yaml"), "{out}");
    }

    #[test]
    fn sort_keys_orders_every_mapping() {
        let mut r = req([r#"{"z":1,"a":{"y":2,"b":3}}"#, "", "", ""]);
        r.sort_keys = true;
        let out = merge_config(r).unwrap();
        assert!(
            out.find("\"a\"").unwrap() < out.find("\"z\"").unwrap(),
            "{out}"
        );
        assert!(
            out.find("\"b\"").unwrap() < out.find("\"y\"").unwrap(),
            "{out}"
        );
    }

    #[test]
    fn indent_is_applied_and_clamped() {
        let mut r = req([r#"{"a":{"b":1}}"#, "", "", ""]);
        r.indent = 4;
        assert!(merge_config(r.clone()).unwrap().contains("\n    \"a\""));
        r.indent = 99;
        assert!(merge_config(r).unwrap().contains("\n        \"a\""));
    }

    #[test]
    fn env_layer_quotes_and_comments_are_handled() {
        let mut r = req([
            "GREETING=\"hello world\"\nRAW=plain # trailing\nLITERAL='a$b'\n",
            "",
            "",
            "",
        ]);
        r.substitute = false;
        let out = merge_config(r).unwrap();
        assert!(out.contains(r#""GREETING": "hello world""#), "{out}");
        assert!(out.contains(r#""RAW": "plain""#), "{out}");
        assert!(out.contains(r#""LITERAL": "a$b""#), "{out}");
    }

    #[test]
    fn rejects_input_over_the_cap() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let err = merge_config(req([&big, "", "", ""])).unwrap_err();
        assert!(err.contains("input too large"), "{err}");
    }

    #[test]
    fn accepts_input_exactly_at_the_cap() {
        // `A=` plus filler, padded to exactly MAX_INPUT_BYTES.
        let body = format!("A={}", "x".repeat(MAX_INPUT_BYTES - 2));
        assert_eq!(body.len(), MAX_INPUT_BYTES);
        let out = merge_config(req([&body, "", "", ""])).unwrap();
        assert!(out.starts_with("{\n  \"A\": \"xxx"), "{}", &out[..40]);
    }

    #[test]
    fn reports_an_unparseable_layer_with_its_slot() {
        let mut r = req([r#"{"a":1}"#, "", "", ""]);
        r.layers[1] = "{not json";
        r.input_format = "json";
        let err = merge_config(r).unwrap_err();
        assert!(err.contains("layer 2 (override)"), "{err}");
        assert!(err.contains("invalid JSON"), "{err}");
    }

    #[test]
    fn unknown_option_values_name_the_alternatives() {
        let mut r = req([r#"{"a":1}"#, "", "", ""]);
        r.array_merge = "smoosh";
        let err = merge_config(r).unwrap_err();
        assert!(err.contains("unknown array_merge 'smoosh'"), "{err}");
        assert!(err.contains("replace, append or unique"), "{err}");
    }
}
