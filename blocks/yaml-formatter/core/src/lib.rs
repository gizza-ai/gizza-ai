//! yaml-formatter core — reindent and normalize YAML with consistent spacing,
//! key-ordering options, and a block/flow style choice. Pure compute, shared by
//! the chat skill block and the web page (no wafer/wasm-bindgen deps).
//!
//! Input is parsed with `serde_yml` into an order-preserving `Value` model and
//! re-emitted by a hand-rolled writer so we control indent width, key order, and
//! block-vs-flow style exactly (the libyaml emitter exposes none of those knobs).
//! Data is normalized, not round-tripped verbatim: comments, blank lines, anchors
//! and aliases (aliases are expanded to their referenced value) are not preserved.

use serde::Deserialize;
use serde_yml::{Mapping, Value};

/// Output layout: multi-line `block` style (beautify) or compact single-line
/// `flow` style (minify).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Block,
    Flow,
}

/// Recursive key ordering applied to every mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Preserve,
    Asc,
    Desc,
}

/// Formatting options resolved from the surface's params.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Spaces of indentation per nesting level for block style (clamped 1..=8).
    pub indent: usize,
    pub style: Style,
    pub sort: Sort,
}

impl Default for Options {
    fn default() -> Self {
        Options { indent: 2, style: Style::Block, sort: Sort::Preserve }
    }
}

/// Parse `style` text → [`Style`] (`block`/`beautify` vs `flow`/`minify`/`compact`).
pub fn parse_style(s: &str) -> Result<Style, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "block" | "beautify" | "" => Ok(Style::Block),
        "flow" | "minify" | "compact" => Ok(Style::Flow),
        other => Err(format!("unknown style '{other}' (use 'block' or 'flow')")),
    }
}

/// Parse `sort_keys` text → [`Sort`].
pub fn parse_sort(s: &str) -> Result<Sort, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "preserve" | "" => Ok(Sort::Preserve),
        "asc" | "ascending" | "a-z" => Ok(Sort::Asc),
        "desc" | "descending" | "z-a" => Ok(Sort::Desc),
        other => Err(format!("unknown sort_keys '{other}' (use 'preserve', 'asc', or 'desc')")),
    }
}

/// Reindent + normalize `input` YAML per `opts`. Handles multi-document streams
/// (`---` separated); each document is normalized independently.
pub fn format_yaml(input: &str, opts: Options) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("no input: paste some YAML to format".into());
    }

    let mut docs: Vec<Value> = Vec::new();
    for de in serde_yml::Deserializer::from_str(input) {
        let v = Value::deserialize(de).map_err(|e| format!("invalid YAML: {e}"))?;
        docs.push(v);
    }
    if docs.is_empty() {
        return Err("no input: the document is empty".into());
    }

    let rendered: Vec<String> = docs
        .into_iter()
        .map(|mut doc| {
            if opts.sort != Sort::Preserve {
                sort_value(&mut doc, opts.sort);
            }
            render_document(&doc, opts)
        })
        .collect();

    // A single document renders on its own; a stream is joined with `---` markers.
    if rendered.len() == 1 {
        Ok(rendered.into_iter().next().unwrap())
    } else {
        Ok(rendered.join("---\n"))
    }
}

/// Render one document (always ends with a trailing newline).
fn render_document(doc: &Value, opts: Options) -> String {
    match opts.style {
        Style::Flow => format!("{}\n", flow_value(doc)),
        Style::Block => {
            let mut out = String::new();
            match doc {
                Value::Mapping(m) if !m.is_empty() => block_map(&mut out, m, "", opts),
                Value::Sequence(s) if !s.is_empty() => block_seq(&mut out, s, "", opts),
                // Scalars, empty containers and tagged roots have no block layout.
                other => out.push_str(&format!("{}\n", flow_value(other))),
            }
            out
        }
    }
}

/// Recursively reorder mapping keys. Children are sorted first so nested maps are
/// ordered regardless of their parent's order.
fn sort_value(v: &mut Value, sort: Sort) {
    match v {
        Value::Mapping(m) => {
            for (_k, val) in m.iter_mut() {
                sort_value(val, sort);
            }
            let mut entries: Vec<(Value, Value)> =
                m.iter().map(|(k, val)| (k.clone(), val.clone())).collect();
            entries.sort_by(|a, b| key_sort_string(&a.0).cmp(&key_sort_string(&b.0)));
            if sort == Sort::Desc {
                entries.reverse();
            }
            let mut fresh = Mapping::new();
            for (k, val) in entries {
                fresh.insert(k, val);
            }
            *m = fresh;
        }
        Value::Sequence(s) => {
            for item in s.iter_mut() {
                sort_value(item, sort);
            }
        }
        Value::Tagged(t) => sort_value(&mut t.value, sort),
        _ => {}
    }
}

/// Comparable text for a mapping key (used only for ordering).
fn key_sort_string(k: &Value) -> String {
    match k {
        Value::String(s) => s.clone(),
        other => flow_value(other),
    }
}

// ---- block style -----------------------------------------------------------

fn block_map(out: &mut String, m: &Mapping, prefix: &str, opts: Options) {
    let unit = " ".repeat(opts.indent);
    for (k, v) in m {
        let key = block_key(k);
        match v {
            Value::Mapping(mm) if !mm.is_empty() => {
                out.push_str(&format!("{prefix}{key}:\n"));
                block_map(out, mm, &format!("{prefix}{unit}"), opts);
            }
            Value::Sequence(ss) if !ss.is_empty() => {
                out.push_str(&format!("{prefix}{key}:\n"));
                block_seq(out, ss, &format!("{prefix}{unit}"), opts);
            }
            _ => out.push_str(&format!("{prefix}{key}: {}\n", inline_block_value(v))),
        }
    }
}

fn block_seq(out: &mut String, s: &[Value], prefix: &str, opts: Options) {
    // A dash + space is a fixed 2-column offset, independent of `indent`, so a
    // nested container after `- ` keeps `- key:` alignment at any indent width.
    let cont = format!("{prefix}  ");
    for item in s {
        match item {
            Value::Mapping(mm) if !mm.is_empty() => {
                let mut tmp = String::new();
                block_map(&mut tmp, mm, &cont, opts);
                out.push_str(&format!("{prefix}- "));
                out.push_str(&tmp[cont.len()..]);
            }
            Value::Sequence(ss) if !ss.is_empty() => {
                let mut tmp = String::new();
                block_seq(&mut tmp, ss, &cont, opts);
                out.push_str(&format!("{prefix}- "));
                out.push_str(&tmp[cont.len()..]);
            }
            _ => out.push_str(&format!("{prefix}- {}\n", inline_block_value(item))),
        }
    }
}

fn block_key(k: &Value) -> String {
    match k {
        Value::String(s) => string_scalar(s, false),
        other => flow_value(other),
    }
}

/// Render a leaf value that sits on a `key: ` / `- ` block line. Scalars use
/// block-context quoting (a bare `,`/`[`/`:` inside a plain scalar is fine in
/// block context); empty and tagged containers fall back to compact flow.
fn inline_block_value(v: &Value) -> String {
    match v {
        Value::String(s) => string_scalar(s, false),
        Value::Null | Value::Bool(_) | Value::Number(_) => flow_value(v),
        other => flow_value(other),
    }
}

// ---- flow style ------------------------------------------------------------

/// Render a value as a single-line flow string (used wholesale for flow style,
/// and for every leaf/inline node in block style).
fn flow_value(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => string_scalar(s, true),
        Value::Sequence(s) => {
            let items: Vec<String> = s.iter().map(flow_value).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Mapping(m) => {
            let items: Vec<String> = m
                .iter()
                .map(|(k, val)| format!("{}: {}", flow_key(k), flow_value(val)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
        Value::Tagged(t) => format!("{} {}", t.tag, flow_value(&t.value)),
    }
}

fn flow_key(k: &Value) -> String {
    match k {
        Value::String(s) => string_scalar(s, true),
        other => flow_value(other),
    }
}

// ---- scalar quoting --------------------------------------------------------

/// Render a string scalar, quoting it only when a plain scalar would be
/// re-parsed as something else (a bool/null/number) or would be syntactically
/// unsafe. `flow` tightens the rules for flow context (`,[]{}:`). Over-quoting is
/// always valid YAML; this policy never emits an ambiguous plain scalar.
fn string_scalar(s: &str, flow: bool) -> String {
    if is_safe_plain(s, flow) {
        s.to_string()
    } else {
        double_quote(s)
    }
}

fn is_safe_plain(s: &str, flow: bool) -> bool {
    if s.is_empty() {
        return false;
    }
    if s.starts_with(' ') || s.ends_with(' ') {
        return false;
    }
    let first = s.chars().next().unwrap();
    // Leading YAML indicator characters force quoting.
    if "-?:,[]{}#&*!|>'\"%@`~".contains(first) {
        return false;
    }
    for c in s.chars() {
        if c == '\n' || c == '\r' || c == '\t' || (c as u32) < 0x20 {
            return false;
        }
    }
    // `: ` starts a mapping value; ` #` starts a comment; a trailing `:` is a key.
    if s.contains(": ") || s.contains(" #") || s.ends_with(':') {
        return false;
    }
    if flow {
        // Flow scalars can't carry flow indicators unquoted.
        if s.contains([',', '[', ']', '{', '}', ':']) {
            return false;
        }
    }
    // Anything that would resolve to a non-string plain scalar must be quoted.
    let low = s.to_ascii_lowercase();
    if matches!(
        low.as_str(),
        // serde_yml uses the YAML 1.2 core schema: only true/false/null (+ `~`)
        // resolve to non-strings, NOT yes/no/on/off, so we don't over-quote those.
        "true" | "false" | "null" | "~" | ".inf" | "-.inf" | "+.inf" | ".nan"
    ) {
        return false;
    }
    if s.parse::<i64>().is_ok() || s.parse::<u64>().is_ok() || s.parse::<f64>().is_ok() {
        return false;
    }
    true
}

fn double_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(input: &str, indent: usize, style: Style, sort: Sort) -> String {
        format_yaml(input, Options { indent, style, sort }).unwrap()
    }

    #[test]
    fn reindents_block_default() {
        let out = fmt("name:   gizza\ntags: [a, b]\n", 2, Style::Block, Sort::Preserve);
        assert_eq!(out, "name: gizza\ntags:\n  - a\n  - b\n");
    }

    #[test]
    fn indent_width_four() {
        let out = fmt("a:\n  b:\n    c: 1\n", 4, Style::Block, Sort::Preserve);
        assert_eq!(out, "a:\n    b:\n        c: 1\n");
    }

    #[test]
    fn nested_map_and_seq() {
        let out = fmt("nested: {x: 1, y: 2}\n", 2, Style::Block, Sort::Preserve);
        assert_eq!(out, "nested:\n  x: 1\n  y: 2\n");
    }

    #[test]
    fn seq_of_maps_alignment() {
        let out = fmt("- name: a\n  age: 1\n- name: b\n", 2, Style::Block, Sort::Preserve);
        assert_eq!(out, "- name: a\n  age: 1\n- name: b\n");
    }

    #[test]
    fn seq_of_maps_indent_four_keeps_dash_offset() {
        // The `- ` offset stays 2 columns; the second key aligns under the first.
        let out = fmt("- k1: 1\n  k2: 2\n", 4, Style::Block, Sort::Preserve);
        assert_eq!(out, "- k1: 1\n  k2: 2\n");
    }

    #[test]
    fn preserves_key_order() {
        let out = fmt("b: 1\na: 2\nc: 3\n", 2, Style::Block, Sort::Preserve);
        assert_eq!(out, "b: 1\na: 2\nc: 3\n");
    }

    #[test]
    fn sorts_keys_ascending_recursively() {
        let out = fmt("b: 1\na:\n  z: 1\n  y: 2\n", 2, Style::Block, Sort::Asc);
        assert_eq!(out, "a:\n  y: 2\n  z: 1\nb: 1\n");
    }

    #[test]
    fn sorts_keys_descending() {
        let out = fmt("a: 1\nb: 2\nc: 3\n", 2, Style::Block, Sort::Desc);
        assert_eq!(out, "c: 3\nb: 2\na: 1\n");
    }

    #[test]
    fn flow_style_is_compact() {
        let out = fmt("name: gizza\ntags:\n  - a\n  - b\n", 2, Style::Flow, Sort::Preserve);
        assert_eq!(out, "{name: gizza, tags: [a, b]}\n");
    }

    #[test]
    fn quotes_ambiguous_strings() {
        // Values that would parse as bool/null/number must be quoted to stay strings.
        let out = fmt("a: \"true\"\nb: \"123\"\nc: \"null\"\n", 2, Style::Block, Sort::Preserve);
        assert_eq!(out, "a: \"true\"\nb: \"123\"\nc: \"null\"\n");
    }

    #[test]
    fn keeps_real_scalars_unquoted() {
        let out = fmt("flag: true\ncount: 42\nrate: 1.5\nempty: null\n", 2, Style::Block, Sort::Preserve);
        assert_eq!(out, "flag: true\ncount: 42\nrate: 1.5\nempty: null\n");
    }

    #[test]
    fn quotes_value_with_colon_space_and_hash() {
        let out = fmt("url: \"key: value\"\n", 2, Style::Block, Sort::Preserve);
        assert_eq!(out, "url: \"key: value\"\n");
    }

    #[test]
    fn keeps_url_plain_in_block() {
        let out = fmt("home: http://example.com/path\n", 2, Style::Block, Sort::Preserve);
        assert_eq!(out, "home: http://example.com/path\n");
    }

    #[test]
    fn multi_document_stream() {
        let out = fmt("a: 1\n---\nb: 2\n", 2, Style::Block, Sort::Preserve);
        assert_eq!(out, "a: 1\n---\nb: 2\n");
    }

    #[test]
    fn empty_containers() {
        let out = fmt("a: {}\nb: []\n", 2, Style::Block, Sort::Preserve);
        assert_eq!(out, "a: {}\nb: []\n");
    }

    #[test]
    fn rejects_invalid_yaml() {
        let err = format_yaml("a: [1, 2", Options::default()).unwrap_err();
        assert!(err.contains("invalid YAML"), "got: {err}");
    }

    #[test]
    fn rejects_empty_input() {
        assert!(format_yaml("   ", Options::default()).unwrap_err().contains("no input"));
    }

    #[test]
    fn parse_helpers() {
        assert_eq!(parse_style("Flow").unwrap(), Style::Flow);
        assert_eq!(parse_style("beautify").unwrap(), Style::Block);
        assert!(parse_style("weird").is_err());
        assert_eq!(parse_sort("ASC").unwrap(), Sort::Asc);
        assert_eq!(parse_sort("").unwrap(), Sort::Preserve);
        assert!(parse_sort("random").is_err());
    }
}
