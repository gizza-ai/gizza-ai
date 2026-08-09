//! yaml-deep-merge core — deep-merge a stream of `---`-separated YAML documents
//! left to right, the way Helm layers values files. Pure compute, shared by the
//! chat skill block and the web page (no wafer/wasm-bindgen deps).
//!
//! Merge model:
//! * mappings merge recursively (or only at the top level when `deep` is off);
//! * lists are replaced, appended, de-duplicated, or merged item-by-item on a
//!   shared key (`array_key`, e.g. `name` for Kubernetes-style lists);
//! * a key set to `null` in a later document REMOVES it (Helm's "deleting a
//!   default key" behaviour) unless `null_deletes` is turned off;
//! * `precedence` picks the winner of a scalar conflict — the later document
//!   (Helm's rule), the earlier one, or a hard error naming the path.
//!
//! Data is normalized, not round-tripped verbatim: comments, blank lines and
//! anchors are not part of the value model (aliases are expanded to the value
//! they reference), so the output is re-emitted canonically.

use gizza_ai_yaml_formatter_core::{format_yaml, Options as FmtOptions, Sort, Style};
use serde::Deserialize;
use serde_yml::{Mapping, Value};

/// Most documents a single merge accepts.
pub const MAX_DOCUMENTS: usize = 20;
/// Most input bytes a single merge accepts (1 MiB).
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;

/// What to do when the same key holds a list in two documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayMerge {
    /// The winning document's list replaces the other (Helm's behaviour).
    Replace,
    /// Later items are appended to the earlier list.
    Append,
    /// Lists are appended, then duplicate items are dropped.
    Unique,
    /// Items that share the same `array_key` value are merged into each other.
    ByKey,
}

/// Who wins when two documents set the same key to different scalars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precedence {
    /// The later document wins (Helm's rule).
    Last,
    /// The earlier document wins; later documents may only add new keys.
    First,
    /// Any disagreement is reported as an error naming the path.
    Error,
}

/// Merge options resolved from the surface's params.
#[derive(Debug, Clone)]
pub struct Options {
    pub array_merge: ArrayMerge,
    /// Field name used to line up list items when `array_merge` is `ByKey`.
    pub array_key: String,
    pub precedence: Precedence,
    /// `true` = recurse into nested mappings; `false` = merge top-level keys only.
    pub deep: bool,
    /// `true` = a `null` in a later document deletes the key.
    pub null_deletes: bool,
    /// `true` = sort every mapping's keys A→Z instead of keeping source order.
    pub sort_keys: bool,
    /// Output indentation, 1–8 spaces per level.
    pub indent: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            array_merge: ArrayMerge::Replace,
            array_key: "name".to_string(),
            precedence: Precedence::Last,
            deep: true,
            null_deletes: true,
            sort_keys: false,
            indent: 2,
        }
    }
}

/// Parse `array_merge` text → [`ArrayMerge`] (empty = the default, `replace`).
pub fn parse_array_merge(s: &str) -> Result<ArrayMerge, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "replace" | "" => Ok(ArrayMerge::Replace),
        "append" | "concat" | "concatenate" => Ok(ArrayMerge::Append),
        "unique" => Ok(ArrayMerge::Unique),
        "by_key" | "by-key" | "bykey" => Ok(ArrayMerge::ByKey),
        other => Err(format!(
            "unknown array_merge '{other}' (use 'replace', 'append', 'unique', or 'by_key')"
        )),
    }
}

/// Parse `precedence` text → [`Precedence`] (empty = the default, `last`).
pub fn parse_precedence(s: &str) -> Result<Precedence, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "last" | "" => Ok(Precedence::Last),
        "first" => Ok(Precedence::First),
        "error" => Ok(Precedence::Error),
        other => {
            Err(format!("unknown precedence '{other}' (use 'last', 'first', or 'error')"))
        }
    }
}

/// Parse `object_merge` text → `deep` flag (empty = the default, `deep`).
pub fn parse_object_merge(s: &str) -> Result<bool, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "deep" | "" => Ok(true),
        "shallow" => Ok(false),
        other => Err(format!("unknown object_merge '{other}' (use 'deep' or 'shallow')")),
    }
}

/// Merge every YAML document in `documents` (a `---`-separated stream) left to
/// right and re-emit the result as block-style YAML.
pub fn merge_yaml(documents: &str, opts: &Options) -> Result<String, String> {
    if documents.trim().is_empty() {
        return Err(
            "no input: paste two or more YAML documents separated by a line containing ---".into(),
        );
    }
    if documents.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {} bytes (1 MiB) per merge",
            documents.len(),
            MAX_INPUT_BYTES
        ));
    }
    if !(1..=8).contains(&opts.indent) {
        return Err(format!("indent must be 1-8 spaces, got {}", opts.indent));
    }
    if opts.array_merge == ArrayMerge::ByKey && opts.array_key.trim().is_empty() {
        return Err("array_merge=by_key needs a non-empty array_key (e.g. 'name')".into());
    }

    let mut docs: Vec<Value> = Vec::new();
    for (i, de) in serde_yml::Deserializer::from_str(documents).enumerate() {
        if i >= MAX_DOCUMENTS {
            return Err(format!(
                "too many documents: the limit is {MAX_DOCUMENTS} per merge"
            ));
        }
        let v = Value::deserialize(de)
            .map_err(|e| format!("document #{} is not valid YAML: {e}", i + 1))?;
        docs.push(v);
    }

    // Empty documents (a stray `---`, an empty values file) contribute nothing,
    // exactly like an empty Helm values file.
    let mut layers: Vec<(usize, Value)> = docs
        .into_iter()
        .enumerate()
        .filter(|(_, v)| !v.is_null())
        .map(|(i, v)| (i + 1, v))
        .collect();
    if layers.is_empty() {
        return Err("no YAML documents found: every document was empty".into());
    }

    let (_, mut acc) = layers.remove(0);
    for (doc_no, layer) in layers {
        acc = merge_value(&acc, &layer, opts, 0, "", doc_no)?;
    }

    let raw = serde_yml::to_string(&acc)
        .map_err(|e| format!("could not serialize the merged document: {e}"))?;
    format_yaml(
        &raw,
        FmtOptions {
            indent: opts.indent,
            style: Style::Block,
            sort: if opts.sort_keys { Sort::Asc } else { Sort::Preserve },
        },
    )
}

/// String-typed entry point shared by the CLI/chat block and the web page.
#[allow(clippy::too_many_arguments)]
pub fn merge_documents(
    documents: &str,
    precedence: &str,
    object_merge: &str,
    array_merge: &str,
    array_key: &str,
    null_deletes: bool,
    sort_keys: bool,
    indent: usize,
) -> Result<String, String> {
    let key = array_key.trim();
    let opts = Options {
        array_merge: parse_array_merge(array_merge)?,
        array_key: if key.is_empty() { "name".to_string() } else { key.to_string() },
        precedence: parse_precedence(precedence)?,
        deep: parse_object_merge(object_merge)?,
        null_deletes,
        sort_keys,
        indent,
    };
    merge_yaml(documents, &opts)
}

// ---- merge ------------------------------------------------------------------

fn merge_value(
    a: &Value,
    b: &Value,
    opts: &Options,
    depth: usize,
    path: &str,
    doc: usize,
) -> Result<Value, String> {
    // Shallow mode only merges the top level; below it the winning value replaces
    // the whole subtree.
    if !opts.deep && depth > 0 {
        return resolve(a, b, opts, path, doc);
    }
    match (a, b) {
        (Value::Mapping(am), Value::Mapping(bm)) => {
            Ok(Value::Mapping(merge_map(am, bm, opts, depth, path, doc)?))
        }
        (Value::Sequence(av), Value::Sequence(bv)) => merge_seq(av, bv, opts, depth, path, doc),
        _ => resolve(a, b, opts, path, doc),
    }
}

fn merge_map(
    am: &Mapping,
    bm: &Mapping,
    opts: &Options,
    depth: usize,
    path: &str,
    doc: usize,
) -> Result<Mapping, String> {
    let mut out = Mapping::new();
    // Base order first, so the earlier document's key order is preserved.
    for (k, av) in am.iter() {
        match bm.get(k) {
            Some(bv) => {
                if opts.null_deletes && bv.is_null() {
                    continue; // Helm: `key: null` in an override removes the key.
                }
                let child = child_path(path, k);
                out.insert(k.clone(), merge_value(av, bv, opts, depth + 1, &child, doc)?);
            }
            None => {
                out.insert(k.clone(), av.clone());
            }
        }
    }
    // Then keys only the later document has.
    for (k, bv) in bm.iter() {
        if am.contains_key(k) {
            continue;
        }
        if opts.null_deletes && bv.is_null() {
            continue; // deleting a key that isn't there is a no-op, not a null value.
        }
        out.insert(k.clone(), bv.clone());
    }
    Ok(out)
}

fn merge_seq(
    av: &[Value],
    bv: &[Value],
    opts: &Options,
    depth: usize,
    path: &str,
    doc: usize,
) -> Result<Value, String> {
    let out: Vec<Value> = match opts.array_merge {
        ArrayMerge::Replace => {
            if av == bv {
                av.to_vec()
            } else {
                match opts.precedence {
                    Precedence::Last => bv.to_vec(),
                    Precedence::First => av.to_vec(),
                    Precedence::Error => {
                        return Err(format!(
                            "list conflict at {}: document #{doc} sets {} item(s) where an earlier document set {} item(s) — precedence=error rejects any disagreement (use precedence=last/first, or array_merge=append/unique/by_key to combine the lists)",
                            path_label(path),
                            bv.len(),
                            av.len()
                        ))
                    }
                }
            }
        }
        ArrayMerge::Append => {
            let mut o = av.to_vec();
            o.extend(bv.iter().cloned());
            o
        }
        ArrayMerge::Unique => {
            let mut o: Vec<Value> = Vec::new();
            for v in av.iter().chain(bv.iter()) {
                if !o.contains(v) {
                    o.push(v.clone());
                }
            }
            o
        }
        ArrayMerge::ByKey => merge_by_key(av, bv, opts, depth, path, doc)?,
    };
    Ok(Value::Sequence(out))
}

/// Line list items up on a shared key (`containers` by `name`, `env` by `name`, …)
/// and merge the matching pairs; unmatched items are appended in order.
fn merge_by_key(
    av: &[Value],
    bv: &[Value],
    opts: &Options,
    depth: usize,
    path: &str,
    doc: usize,
) -> Result<Vec<Value>, String> {
    let key = Value::String(opts.array_key.clone());
    let mut out: Vec<Value> = av.to_vec();
    for item in bv {
        let id = item.as_mapping().and_then(|m| m.get(&key)).cloned();
        let Some(idv) = id else {
            // No key field (or not a mapping at all) → nothing to line up on.
            out.push(item.clone());
            continue;
        };
        let pos = out
            .iter()
            .position(|e| e.as_mapping().and_then(|m| m.get(&key)) == Some(&idv));
        match pos {
            Some(i) => {
                let child = format!("{}[{}={}]", path, opts.array_key, scalar_text(&idv));
                out[i] = merge_value(&out[i], item, opts, depth + 1, &child, doc)?;
            }
            None => out.push(item.clone()),
        }
    }
    Ok(out)
}

fn resolve(
    a: &Value,
    b: &Value,
    opts: &Options,
    path: &str,
    doc: usize,
) -> Result<Value, String> {
    if opts.null_deletes && b.is_null() {
        // Reached only outside a mapping (a list item, the document root): there
        // is no key to remove, so the earlier value stands.
        return Ok(a.clone());
    }
    if a == b {
        return Ok(b.clone());
    }
    match opts.precedence {
        Precedence::Last => Ok(b.clone()),
        Precedence::First => Ok(a.clone()),
        Precedence::Error => Err(format!(
            "conflict at {}: document #{doc} sets {}, but an earlier document set {} — precedence=error rejects any disagreement (use precedence=last to let the later document win, or first to keep the base value)",
            path_label(path),
            brief(b),
            brief(a)
        )),
    }
}

// ---- paths + messages -------------------------------------------------------

fn child_path(path: &str, k: &Value) -> String {
    let key = scalar_text(k);
    if path.is_empty() {
        key
    } else {
        format!("{path}.{key}")
    }
}

fn path_label(path: &str) -> String {
    if path.is_empty() {
        "the document root".to_string()
    } else {
        format!("`{path}`")
    }
}

fn scalar_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Null => "null".to_string(),
        _ => "?".to_string(),
    }
}

/// A short single-line rendering of a value, for error messages.
fn brief(v: &Value) -> String {
    let text = serde_yml::to_string(v).unwrap_or_default();
    let flat = text.trim().replace('\n', " ");
    if flat.chars().count() > 60 {
        format!("{}…", flat.chars().take(60).collect::<String>())
    } else {
        flat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merge(docs: &str, opts: Options) -> String {
        merge_yaml(docs, &opts).unwrap()
    }

    #[test]
    fn deep_merges_nested_mappings_later_wins() {
        let out = merge(
            "image:\n  repo: app\n  tag: 1.0\nreplicas: 1\n---\nimage:\n  tag: 2.0\ningress: true\n",
            Options::default(),
        );
        // `2.0` is a YAML float in both documents, so it stays an unquoted float.
        assert_eq!(out, "image:\n  repo: app\n  tag: 2.0\nreplicas: 1\ningress: true\n");
    }

    #[test]
    fn base_key_order_is_preserved() {
        let out = merge("b: 1\na: 1\n---\nc: 3\na: 2\n", Options::default());
        assert_eq!(out, "b: 1\na: 2\nc: 3\n");
    }

    #[test]
    fn sort_keys_reorders_recursively() {
        let out = merge(
            "b: 1\na:\n  z: 1\n---\na:\n  y: 2\n",
            Options { sort_keys: true, ..Options::default() },
        );
        assert_eq!(out, "a:\n  y: 2\n  z: 1\nb: 1\n");
    }

    #[test]
    fn null_deletes_a_key_helm_style() {
        let out = merge("resources:\n  limits:\n    cpu: 100m\n---\nresources:\n  limits: null\n", Options::default());
        assert_eq!(out, "resources: {}\n");
    }

    #[test]
    fn null_is_kept_as_a_value_when_deletion_is_off() {
        let out = merge(
            "a: 1\nb: 2\n---\nb: null\n",
            Options { null_deletes: false, ..Options::default() },
        );
        assert_eq!(out, "a: 1\nb: null\n");
    }

    #[test]
    fn arrays_replace_by_default() {
        let out = merge("ports:\n  - 80\n  - 443\n---\nports:\n  - 8080\n", Options::default());
        assert_eq!(out, "ports:\n  - 8080\n");
    }

    #[test]
    fn arrays_append() {
        let out = merge(
            "ports: [80]\n---\nports: [443]\n",
            Options { array_merge: ArrayMerge::Append, ..Options::default() },
        );
        assert_eq!(out, "ports:\n  - 80\n  - 443\n");
    }

    #[test]
    fn arrays_unique_drops_duplicates() {
        let out = merge(
            "tags: [a, b]\n---\ntags: [b, c]\n",
            Options { array_merge: ArrayMerge::Unique, ..Options::default() },
        );
        assert_eq!(out, "tags:\n  - a\n  - b\n  - c\n");
    }

    #[test]
    fn arrays_merge_by_key() {
        let out = merge(
            "env:\n  - name: LOG\n    value: info\n  - name: PORT\n    value: \"80\"\n---\nenv:\n  - name: LOG\n    value: debug\n  - name: EXTRA\n    value: \"1\"\n",
            Options { array_merge: ArrayMerge::ByKey, ..Options::default() },
        );
        assert_eq!(
            out,
            "env:\n  - name: LOG\n    value: debug\n  - name: PORT\n    value: \"80\"\n  - name: EXTRA\n    value: \"1\"\n"
        );
    }

    #[test]
    fn by_key_appends_items_without_the_key() {
        let out = merge(
            "env:\n  - name: A\n    value: \"1\"\n---\nenv:\n  - plain\n",
            Options { array_merge: ArrayMerge::ByKey, ..Options::default() },
        );
        assert_eq!(out, "env:\n  - name: A\n    value: \"1\"\n  - plain\n");
    }

    #[test]
    fn precedence_first_keeps_the_base_value_but_adds_new_keys() {
        let out = merge(
            "a: 1\n---\na: 2\nb: 3\n",
            Options { precedence: Precedence::First, ..Options::default() },
        );
        assert_eq!(out, "a: 1\nb: 3\n");
    }

    #[test]
    fn shallow_merge_replaces_whole_subtrees() {
        let out = merge(
            "image:\n  repo: app\n  tag: 1.0\nreplicas: 1\n---\nimage:\n  tag: 2.0\n",
            Options { deep: false, ..Options::default() },
        );
        assert_eq!(out, "image:\n  tag: 2.0\nreplicas: 1\n");
    }

    #[test]
    fn indent_width_is_configurable() {
        let out = merge(
            "a:\n  b: 1\n---\na:\n  c: 2\n",
            Options { indent: 4, ..Options::default() },
        );
        assert_eq!(out, "a:\n    b: 1\n    c: 2\n");
    }

    #[test]
    fn empty_documents_are_ignored() {
        let out = merge("---\na: 1\n---\n---\nb: 2\n", Options::default());
        assert_eq!(out, "a: 1\nb: 2\n");
    }

    #[test]
    fn a_single_document_is_normalized() {
        let out = merge("b:   1\na: [1, 2]\n", Options::default());
        assert_eq!(out, "b: 1\na:\n  - 1\n  - 2\n");
    }

    #[test]
    fn twenty_documents_are_accepted() {
        let docs: Vec<String> = (1..=MAX_DOCUMENTS).map(|i| format!("k{i}: {i}")).collect();
        let out = merge(&docs.join("\n---\n"), Options::default());
        assert!(out.starts_with("k1: 1\n"), "got: {out}");
        assert!(out.ends_with(&format!("k{MAX_DOCUMENTS}: {MAX_DOCUMENTS}\n")), "got: {out}");
    }

    // ---- errors ----

    #[test]
    fn rejects_more_than_the_document_cap() {
        let docs: Vec<String> = (1..=MAX_DOCUMENTS + 1).map(|i| format!("k{i}: {i}")).collect();
        let err = merge_yaml(&docs.join("\n---\n"), &Options::default()).unwrap_err();
        assert!(err.contains("too many documents"), "got: {err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = merge_yaml("   \n", &Options::default()).unwrap_err();
        assert!(err.contains("no input"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_yaml_naming_the_document() {
        let err = merge_yaml("a: 1\n---\nb: [1, 2\n", &Options::default()).unwrap_err();
        assert!(err.contains("document #2 is not valid YAML"), "got: {err}");
    }

    #[test]
    fn precedence_error_reports_the_conflicting_path() {
        let err = merge_yaml(
            "image:\n  tag: 1.0\n---\nimage:\n  tag: 2.0\n",
            &Options { precedence: Precedence::Error, ..Options::default() },
        )
        .unwrap_err();
        assert!(err.contains("conflict at `image.tag`"), "got: {err}");
        assert!(err.contains("document #2"), "got: {err}");
    }

    #[test]
    fn precedence_error_allows_identical_values() {
        let out = merge(
            "a: 1\n---\na: 1\nb: 2\n",
            Options { precedence: Precedence::Error, ..Options::default() },
        );
        assert_eq!(out, "a: 1\nb: 2\n");
    }

    #[test]
    fn precedence_error_reports_list_conflicts() {
        let err = merge_yaml(
            "ports: [80]\n---\nports: [443, 8443]\n",
            &Options { precedence: Precedence::Error, ..Options::default() },
        )
        .unwrap_err();
        assert!(err.contains("list conflict at `ports`"), "got: {err}");
    }

    #[test]
    fn rejects_out_of_range_indent() {
        let err = merge_yaml("a: 1", &Options { indent: 9, ..Options::default() }).unwrap_err();
        assert!(err.contains("indent must be 1-8"), "got: {err}");
    }

    #[test]
    fn rejects_oversized_input() {
        let big = format!("a: \"{}\"\n", "x".repeat(MAX_INPUT_BYTES));
        let err = merge_yaml(&big, &Options::default()).unwrap_err();
        assert!(err.contains("1 MiB"), "got: {err}");
    }

    #[test]
    fn parse_helpers_and_string_entry_point() {
        assert_eq!(parse_array_merge("BY_KEY").unwrap(), ArrayMerge::ByKey);
        assert_eq!(parse_array_merge("").unwrap(), ArrayMerge::Replace);
        assert!(parse_array_merge("nope").unwrap_err().contains("unknown array_merge"));
        assert_eq!(parse_precedence("First").unwrap(), Precedence::First);
        assert!(parse_precedence("weird").unwrap_err().contains("unknown precedence"));
        assert!(parse_object_merge("deep").unwrap());
        assert!(!parse_object_merge("shallow").unwrap());
        assert!(parse_object_merge("flat").unwrap_err().contains("unknown object_merge"));

        let out =
            merge_documents("a: 1\n---\na: 2\n", "last", "deep", "replace", "name", true, false, 2)
                .unwrap();
        assert_eq!(out, "a: 2\n");
        assert!(merge_documents("a: 1", "sideways", "deep", "replace", "name", true, false, 2)
            .is_err());
    }
}
