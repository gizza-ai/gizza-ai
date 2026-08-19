//! json-mask core — prune a JSON document to a subtree using the Google
//! Partial-Response / json-mask field syntax (`a,b(c,d)`), shared by the chat
//! skill block and the web page.
//!
//! The language (identical to Google's `?fields=` partial response and the
//! `json-mask` family of libraries):
//!
//! ```text
//! a                    keep field `a` and everything under it
//! a,b,c                comma-separated sibling list
//! a/b/c                path — keep `c` inside `b` inside `a`
//! a(b,c)               sub-selection — from `a`, keep only `b` and `c`
//! a/*/c                `*` matches every key of an object
//! \, \/ \( \) \* \\    backslash escapes a literal special character in a key
//! ```
//!
//! Unlike JSONPath/jq/JSONata (which return a flat list of matched nodes) a
//! mask *prunes in place*: the output keeps the shape of the input, so a client
//! can consume it with the same code it used for the full document.
//!
//! Arrays are transparent — a selection is applied to every element and the
//! array's length and order are preserved.

use serde_json::{Map, Value};

/// Largest JSON document accepted, in bytes.
pub const MAX_INPUT_BYTES: usize = 5_000_000;
/// Largest mask expression accepted, in bytes.
pub const MAX_MASK_BYTES: usize = 4_000;
/// Deepest nesting accepted in a mask expression (`a(b(c(…)))` / `a/b/c/…`).
pub const MAX_MASK_DEPTH: usize = 64;

/// Whether the mask selects what to KEEP or what to REMOVE.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Keep only what the mask selects (the classic json-mask behaviour).
    Keep,
    /// Keep everything except what the mask selects.
    Remove,
}

/// How the masked JSON is rendered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    /// 2-space indented.
    Pretty,
    /// Single line, no whitespace.
    Compact,
}

/// What to do when a field named by the mask isn't present in the document.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OnMissing {
    /// Leave it out (what every json-mask implementation does).
    Omit,
    /// Emit the key with a JSON `null` value, so every record has the same shape.
    Null,
    /// Fail with an error naming the field and its path.
    Error,
}

/// What to do with objects/arrays that end up empty after masking.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Empty {
    /// Keep the empty `{}` / `[]` husk.
    Keep,
    /// Drop it (recursively — a container left empty by dropping is dropped too).
    Drop,
}

/// Masking options. `Default` is the competitor-compatible configuration.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub mode: Mode,
    pub format: Format,
    pub on_missing: OnMissing,
    pub empty: Empty,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Keep,
            format: Format::Pretty,
            on_missing: OnMissing::Omit,
            empty: Empty::Keep,
        }
    }
}

impl Mode {
    /// Parse the `mode` option; an empty string selects the default.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "keep" => Ok(Mode::Keep),
            "remove" => Ok(Mode::Remove),
            other => Err(format!(
                "unknown mode '{other}' — expected 'keep' or 'remove'"
            )),
        }
    }
}

impl Format {
    /// Parse the `format` option; an empty string selects the default.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "pretty" => Ok(Format::Pretty),
            "compact" => Ok(Format::Compact),
            other => Err(format!(
                "unknown format '{other}' — expected 'pretty' or 'compact'"
            )),
        }
    }
}

impl OnMissing {
    /// Parse the `on_missing` option; an empty string selects the default.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "omit" => Ok(OnMissing::Omit),
            "null" => Ok(OnMissing::Null),
            "error" => Ok(OnMissing::Error),
            other => Err(format!(
                "unknown on_missing '{other}' — expected 'omit', 'null' or 'error'"
            )),
        }
    }
}

impl Empty {
    /// Parse the `empty` option; an empty string selects the default.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "keep" => Ok(Empty::Keep),
            "drop" => Ok(Empty::Drop),
            other => Err(format!(
                "unknown empty '{other}' — expected 'keep' or 'drop'"
            )),
        }
    }
}

/// One selected field in a compiled mask.
///
/// `children == None` means "keep this field's whole subtree"; `Some(list)`
/// means "descend and keep only these".
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// The field name (already unescaped). `"*"` when `wildcard` is set.
    pub name: String,
    /// True for the `*` wildcard, which matches every key of an object.
    pub wildcard: bool,
    /// Sub-selection, or `None` for "the whole subtree".
    pub children: Option<Vec<Node>>,
}

// ---------------------------------------------------------------- mask parser

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    /// `Props ::= Prop | Prop "," Props`
    fn props(&mut self, depth: usize) -> Result<Vec<Node>, String> {
        if depth > MAX_MASK_DEPTH {
            return Err(format!(
                "mask nests deeper than {MAX_MASK_DEPTH} levels — simplify the expression"
            ));
        }
        let mut out: Vec<Node> = Vec::new();
        loop {
            let node = self.prop(depth)?;
            merge_into(&mut out, node);
            self.skip_ws();
            if self.peek() == Some(',') {
                self.pos += 1;
                continue;
            }
            break;
        }
        Ok(out)
    }

    /// `Prop ::= NAME | NAME "/" Prop | NAME "(" Props ")"`
    fn prop(&mut self, depth: usize) -> Result<Node, String> {
        let (name, wildcard) = self.name()?;
        self.skip_ws();
        match self.peek() {
            Some('/') => {
                self.pos += 1;
                let child = self.prop(depth + 1)?;
                Ok(Node {
                    name,
                    wildcard,
                    children: Some(vec![child]),
                })
            }
            Some('(') => {
                self.pos += 1;
                self.skip_ws();
                if self.peek() == Some(')') {
                    return Err(format!(
                        "empty sub-selection in '{name}()' — list at least one field inside the parentheses"
                    ));
                }
                let inner = self.props(depth + 1)?;
                self.skip_ws();
                if self.peek() != Some(')') {
                    return Err(format!(
                        "unclosed '(' after '{name}' — every sub-selection needs a matching ')'"
                    ));
                }
                self.pos += 1;
                Ok(Node {
                    name,
                    wildcard,
                    children: Some(inner),
                })
            }
            _ => Ok(Node {
                name,
                wildcard,
                children: None,
            }),
        }
    }

    /// A field name: any run of characters up to an unescaped `, / ( )`.
    /// Returns the unescaped name plus whether it is the `*` wildcard.
    fn name(&mut self) -> Result<(String, bool), String> {
        // (char, was_escaped) so trimming can't eat an escaped space.
        let mut buf: Vec<(char, bool)> = Vec::new();
        let mut bare_star = false;
        while let Some(c) = self.peek() {
            match c {
                ',' | '/' | '(' | ')' => break,
                '\\' => {
                    self.pos += 1;
                    match self.peek() {
                        Some(e) => {
                            buf.push((e, true));
                            self.pos += 1;
                        }
                        None => {
                            return Err(
                                "mask ends with a dangling '\\' — escape it as '\\\\' for a literal backslash".into(),
                            )
                        }
                    }
                }
                '*' => {
                    bare_star = true;
                    buf.push((c, false));
                    self.pos += 1;
                }
                _ => {
                    buf.push((c, false));
                    self.pos += 1;
                }
            }
        }
        // Trim unescaped surrounding whitespace so 'a, b(c, d)' parses like 'a,b(c,d)'.
        while matches!(buf.first(), Some((c, false)) if c.is_whitespace()) {
            buf.remove(0);
        }
        while matches!(buf.last(), Some((c, false)) if c.is_whitespace()) {
            buf.pop();
        }
        let name: String = buf.iter().map(|(c, _)| *c).collect();
        if name.is_empty() {
            return Err(format!(
                "empty field name at position {} — check for a stray ',', '/' or '()'",
                self.pos + 1
            ));
        }
        if bare_star {
            if name == "*" {
                return Ok((name, true));
            }
            return Err(format!(
                "unescaped '*' inside field name '{name}' — write '*' on its own for the wildcard, or '\\*' for a literal asterisk"
            ));
        }
        Ok((name, false))
    }
}

/// Merge `node` into `list`, combining duplicates (`a/b,a/c` → `a(b,c)`).
/// A bare `a` (whole subtree) always wins over a narrower `a(b)`.
fn merge_into(list: &mut Vec<Node>, node: Node) {
    if let Some(existing) = list
        .iter_mut()
        .find(|n| n.wildcard == node.wildcard && n.name == node.name)
    {
        match (existing.children.as_mut(), node.children) {
            // Already keeping the whole subtree — nothing can widen it further.
            (None, _) => {}
            // A bare name widens a previous sub-selection to the whole subtree.
            (Some(_), None) => existing.children = None,
            (Some(ex), Some(new)) => {
                for c in new {
                    merge_into(ex, c);
                }
            }
        }
        return;
    }
    list.push(node);
}

/// Compile a mask expression into its selection tree.
///
/// Returns a descriptive error for an empty or malformed expression.
pub fn parse_mask(mask: &str) -> Result<Vec<Node>, String> {
    if mask.len() > MAX_MASK_BYTES {
        return Err(format!(
            "mask is {} bytes; the maximum is {MAX_MASK_BYTES}",
            mask.len()
        ));
    }
    if mask.trim().is_empty() {
        return Err("mask is empty — give at least one field, e.g. 'a,b(c,d)'".into());
    }
    let mut p = Parser {
        chars: mask.chars().collect(),
        pos: 0,
    };
    let nodes = p.props(0)?;
    p.skip_ws();
    if let Some(c) = p.peek() {
        return Err(format!(
            "unexpected '{c}' at position {} in the mask — a ')' without a matching '('?",
            p.pos + 1
        ));
    }
    Ok(nodes)
}

// ---------------------------------------------------------------- masking

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Render a location for an error message: `$`, `$.items[0].title`.
fn show_path(path: &[String]) -> String {
    let mut out = String::from("$");
    for seg in path {
        if seg.starts_with('[') {
            out.push_str(seg);
        } else {
            out.push('.');
            out.push_str(seg);
        }
    }
    out
}

fn is_empty_container(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.is_empty(),
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

/// The children that apply to a key, merged across every node that matched it.
/// `None` means "keep the whole subtree".
fn merged_children<'a>(matches: &[&'a Node]) -> Option<Vec<&'a Node>> {
    let mut merged: Vec<&Node> = Vec::new();
    for m in matches {
        match &m.children {
            None => return None,
            Some(ch) => merged.extend(ch.iter()),
        }
    }
    Some(merged)
}

/// Apply `children` to the value stored at a key.
///
/// `Ok(None)` means the selection can't be satisfied here — a sub-selection was
/// asked for but the value is a scalar, so there is nothing to descend into.
fn select_value(
    value: &Value,
    children: Option<&[&Node]>,
    path: &mut Vec<String>,
    opts: &Options,
) -> Result<Option<Value>, String> {
    let Some(children) = children else {
        // Bare name — the whole subtree comes along.
        return Ok(Some(value.clone()));
    };
    match value {
        Value::Object(map) => Ok(Some(Value::Object(keep_object(map, children, path, opts)?))),
        Value::Array(items) => {
            // Arrays are transparent: the same selection is applied to every element.
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                path.push(format!("[{i}]"));
                let masked = select_value(item, Some(children), path, opts)?;
                path.pop();
                match masked {
                    Some(v) => {
                        if opts.empty == Empty::Drop && is_empty_container(&v) {
                            continue;
                        }
                        out.push(v);
                    }
                    None => match opts.on_missing {
                        OnMissing::Omit => continue,
                        OnMissing::Null => out.push(Value::Null),
                        OnMissing::Error => {
                            return Err(format!(
                                "cannot select fields inside {} at {}[{i}] — it is not an object",
                                type_name(item),
                                show_path(path)
                            ))
                        }
                    },
                }
            }
            Ok(Some(Value::Array(out)))
        }
        // Scalar with a sub-selection: nothing to descend into.
        _ => Ok(None),
    }
}

/// Keep-mode walk over one object, preserving the source key order.
fn keep_object(
    map: &Map<String, Value>,
    nodes: &[&Node],
    path: &mut Vec<String>,
    opts: &Options,
) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    let mut hit = vec![false; nodes.len()];

    for (key, value) in map {
        let matches: Vec<&Node> = nodes
            .iter()
            .enumerate()
            .filter(|(i, n)| {
                let m = n.wildcard || n.name == *key;
                if m {
                    hit[*i] = true;
                }
                m
            })
            .map(|(_, n)| *n)
            .collect();
        if matches.is_empty() {
            continue;
        }
        let children = merged_children(&matches);
        path.push(key.clone());
        let selected = select_value(value, children.as_deref(), path, opts);
        path.pop();
        match selected? {
            Some(v) => {
                if opts.empty == Empty::Drop && is_empty_container(&v) {
                    continue;
                }
                out.insert(key.clone(), v);
            }
            None => match opts.on_missing {
                OnMissing::Omit => continue,
                OnMissing::Null => {
                    out.insert(key.clone(), Value::Null);
                }
                OnMissing::Error => {
                    return Err(format!(
                        "cannot select fields inside {} at {}.{key} — it is not an object",
                        type_name(value),
                        show_path(path)
                    ))
                }
            },
        }
    }

    // Fields the mask asked for that the document doesn't have. The wildcard
    // never "misses" — it simply matches whatever is there.
    for (i, node) in nodes.iter().enumerate() {
        if hit[i] || node.wildcard {
            continue;
        }
        match opts.on_missing {
            OnMissing::Omit => {}
            OnMissing::Null => {
                out.insert(node.name.clone(), Value::Null);
            }
            OnMissing::Error => {
                return Err(format!(
                    "field '{}' not found at {}",
                    node.name,
                    show_path(path)
                ))
            }
        }
    }
    Ok(out)
}

/// Remove-mode walk: everything survives except what the mask names.
fn remove_value(
    value: &Value,
    nodes: &[&Node],
    path: &mut Vec<String>,
    opts: &Options,
) -> Result<Value, String> {
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            let mut hit = vec![false; nodes.len()];
            for (key, v) in map {
                let matches: Vec<&Node> = nodes
                    .iter()
                    .enumerate()
                    .filter(|(i, n)| {
                        let m = n.wildcard || n.name == *key;
                        if m {
                            hit[*i] = true;
                        }
                        m
                    })
                    .map(|(_, n)| *n)
                    .collect();
                if matches.is_empty() {
                    out.insert(key.clone(), v.clone());
                    continue;
                }
                match merged_children(&matches) {
                    // A bare name in remove mode deletes the whole subtree.
                    None => continue,
                    Some(children) => {
                        path.push(key.clone());
                        let kept = remove_value(v, &children, path, opts);
                        path.pop();
                        let kept = kept?;
                        if opts.empty == Empty::Drop && is_empty_container(&kept) {
                            continue;
                        }
                        out.insert(key.clone(), kept);
                    }
                }
            }
            if opts.on_missing == OnMissing::Error {
                for (i, node) in nodes.iter().enumerate() {
                    if !hit[i] && !node.wildcard {
                        return Err(format!(
                            "field '{}' not found at {}",
                            node.name,
                            show_path(path)
                        ));
                    }
                }
            }
            Ok(Value::Object(out))
        }
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                path.push(format!("[{i}]"));
                let kept = remove_value(item, nodes, path, opts);
                path.pop();
                let kept = kept?;
                if opts.empty == Empty::Drop && is_empty_container(&kept) {
                    continue;
                }
                out.push(kept);
            }
            Ok(Value::Array(out))
        }
        // Nothing to remove from a scalar.
        other => {
            if opts.on_missing == OnMissing::Error {
                return Err(format!(
                    "cannot remove fields inside {} at {} — it is not an object",
                    type_name(other),
                    show_path(path)
                ));
            }
            Ok(other.clone())
        }
    }
}

/// Mask `json` with `mask`, returning the pruned document as a JSON string.
///
/// Errors carry the offending field/position and what was expected.
pub fn mask_json(json: &str, mask: &str, opts: Options) -> Result<String, String> {
    if json.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "JSON input is {} bytes; the maximum is {MAX_INPUT_BYTES}",
            json.len()
        ));
    }
    if json.trim().is_empty() {
        return Err("JSON input is empty — paste a JSON object or array".into());
    }
    let doc: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let nodes = parse_mask(mask)?;
    let refs: Vec<&Node> = nodes.iter().collect();

    if !matches!(doc, Value::Object(_) | Value::Array(_)) {
        return Err(format!(
            "expected a JSON object or array at the top level to apply a field mask, got {}",
            type_name(&doc)
        ));
    }

    let mut path: Vec<String> = Vec::new();
    let masked = match opts.mode {
        Mode::Keep => select_value(&doc, Some(&refs), &mut path, &opts)?.ok_or_else(|| {
            format!(
                "expected a JSON object or array at the top level to apply a field mask, got {}",
                type_name(&doc)
            )
        })?,
        Mode::Remove => remove_value(&doc, &refs, &mut path, &opts)?,
    };

    let rendered = match opts.format {
        Format::Pretty => serde_json::to_string_pretty(&masked),
        Format::Compact => serde_json::to_string(&masked),
    };
    rendered.map_err(|e| format!("could not serialize the masked JSON: {e}"))
}

/// String-argument entry point shared by the chat block and the web page.
pub fn run(
    json: &str,
    mask: &str,
    mode: &str,
    format: &str,
    on_missing: &str,
    empty: &str,
) -> Result<String, String> {
    let opts = Options {
        mode: Mode::parse(mode)?,
        format: Format::parse(format)?,
        on_missing: OnMissing::parse(on_missing)?,
        empty: Empty::parse(empty)?,
    };
    mask_json(json, mask, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compact(json: &str, mask: &str) -> String {
        mask_json(
            json,
            mask,
            Options {
                format: Format::Compact,
                ..Options::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn keeps_named_top_level_fields() {
        assert_eq!(compact(r#"{"a":1,"b":2,"c":3}"#, "a,c"), r#"{"a":1,"c":3}"#);
    }

    #[test]
    fn bare_name_keeps_the_whole_subtree() {
        assert_eq!(
            compact(r#"{"a":{"x":1,"y":[1,2]},"b":2}"#, "a"),
            r#"{"a":{"x":1,"y":[1,2]}}"#
        );
    }

    #[test]
    fn sub_selection_prunes_inside_a_parent() {
        // The canonical example from the syntax: a,b(c,d).
        assert_eq!(
            compact(
                r#"{"a":{"deep":true},"b":{"c":1,"d":{"e":2},"f":3},"g":4}"#,
                "a,b(c,d)"
            ),
            r#"{"a":{"deep":true},"b":{"c":1,"d":{"e":2}}}"#
        );
    }

    #[test]
    fn path_syntax_selects_through_parents() {
        assert_eq!(
            compact(r#"{"a":{"b":{"c":1,"z":9},"y":8}}"#, "a/b/c"),
            r#"{"a":{"b":{"c":1}}}"#
        );
    }

    #[test]
    fn path_and_sub_selection_combine() {
        assert_eq!(
            compact(
                r#"{"kind":"demo","items":[{"title":"First","chars":{"length":"short","accuracy":"high"},"status":"active"}]}"#,
                "kind,items(title,chars/length)"
            ),
            r#"{"kind":"demo","items":[{"title":"First","chars":{"length":"short"}}]}"#
        );
    }

    #[test]
    fn duplicate_branches_merge() {
        assert_eq!(
            compact(r#"{"a":{"b":1,"c":2,"d":3}}"#, "a/b,a/c"),
            r#"{"a":{"b":1,"c":2}}"#
        );
    }

    #[test]
    fn bare_name_widens_a_narrower_duplicate() {
        assert_eq!(
            compact(r#"{"a":{"b":1,"c":2}}"#, "a(b),a"),
            r#"{"a":{"b":1,"c":2}}"#
        );
    }

    #[test]
    fn arrays_map_the_selection_over_every_element() {
        assert_eq!(
            compact(r#"{"items":[{"t":"a","x":1},{"t":"b","x":2}]}"#, "items(t)"),
            r#"{"items":[{"t":"a"},{"t":"b"}]}"#
        );
    }

    #[test]
    fn top_level_array_is_transparent() {
        assert_eq!(
            compact(r#"[{"a":1,"b":2},{"a":3,"b":4}]"#, "a"),
            r#"[{"a":1},{"a":3}]"#
        );
    }

    #[test]
    fn wildcard_matches_every_key() {
        assert_eq!(
            compact(
                r#"{"m":{"u1":{"n":"A","p":1},"u2":{"n":"B","p":2}}}"#,
                "m/*/n"
            ),
            r#"{"m":{"u1":{"n":"A"},"u2":{"n":"B"}}}"#
        );
    }

    #[test]
    fn wildcard_at_the_root_keeps_everything() {
        assert_eq!(
            compact(r#"{"a":1,"b":{"c":2}}"#, "*"),
            r#"{"a":1,"b":{"c":2}}"#
        );
    }

    #[test]
    fn escapes_allow_special_characters_in_keys() {
        assert_eq!(compact(r#"{"a,b":1,"c":2}"#, r"a\,b"), r#"{"a,b":1}"#);
        assert_eq!(compact(r#"{"*":1,"b":2}"#, r"\*"), r#"{"*":1}"#);
        assert_eq!(compact(r#"{"a/b":1,"a":2}"#, r"a\/b"), r#"{"a/b":1}"#);
    }

    #[test]
    fn whitespace_around_names_is_ignored() {
        assert_eq!(
            compact(r#"{"a":1,"b":{"c":2,"d":3}}"#, "a, b(c, d)"),
            r#"{"a":1,"b":{"c":2,"d":3}}"#
        );
    }

    #[test]
    fn missing_fields_are_omitted_by_default() {
        assert_eq!(compact(r#"{"a":1}"#, "a,nope"), r#"{"a":1}"#);
    }

    #[test]
    fn on_missing_null_stabilises_the_shape() {
        let out = mask_json(
            r#"[{"a":1,"b":2},{"a":3}]"#,
            "a,b",
            Options {
                format: Format::Compact,
                on_missing: OnMissing::Null,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(out, r#"[{"a":1,"b":2},{"a":3,"b":null}]"#);
    }

    #[test]
    fn on_missing_error_names_the_field_and_path() {
        let err = mask_json(
            r#"{"user":{"id":1}}"#,
            "user(id,email)",
            Options {
                on_missing: OnMissing::Error,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert_eq!(err, "field 'email' not found at $.user");
    }

    #[test]
    fn empty_drop_removes_husks() {
        let json = r#"{"items":[{"t":"a"},{"other":1}]}"#;
        assert_eq!(compact(json, "items(t)"), r#"{"items":[{"t":"a"},{}]}"#);
        let dropped = mask_json(
            json,
            "items(t)",
            Options {
                format: Format::Compact,
                empty: Empty::Drop,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(dropped, r#"{"items":[{"t":"a"}]}"#);
    }

    #[test]
    fn remove_mode_inverts_the_selection() {
        let out = mask_json(
            r#"{"a":1,"b":{"c":2,"d":3},"e":4}"#,
            "a,b(c)",
            Options {
                format: Format::Compact,
                mode: Mode::Remove,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(out, r#"{"b":{"d":3},"e":4}"#);
    }

    #[test]
    fn remove_mode_strips_a_field_from_every_array_element() {
        let out = mask_json(
            r#"{"users":[{"name":"A","token":"x"},{"name":"B","token":"y"}]}"#,
            "users(token)",
            Options {
                format: Format::Compact,
                mode: Mode::Remove,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(out, r#"{"users":[{"name":"A"},{"name":"B"}]}"#);
    }

    #[test]
    fn scalar_sub_selection_is_omitted_by_default() {
        assert_eq!(compact(r#"{"a":5,"b":1}"#, "a(x),b"), r#"{"b":1}"#);
    }

    #[test]
    fn source_key_order_is_preserved() {
        assert_eq!(
            compact(r#"{"z":1,"m":2,"a":3}"#, "a,m,z"),
            r#"{"z":1,"m":2,"a":3}"#
        );
    }

    #[test]
    fn pretty_is_two_space_indented() {
        let out = mask_json(r#"{"a":{"b":1},"c":2}"#, "a", Options::default()).unwrap();
        assert_eq!(out, "{\n  \"a\": {\n    \"b\": 1\n  }\n}");
    }

    #[test]
    fn rejects_invalid_json() {
        let err = mask_json("{bad}", "a", Options::default()).unwrap_err();
        assert!(err.starts_with("invalid JSON:"), "{err}");
    }

    #[test]
    fn rejects_an_empty_mask() {
        let err = mask_json(r#"{"a":1}"#, "   ", Options::default()).unwrap_err();
        assert!(err.contains("mask is empty"), "{err}");
    }

    #[test]
    fn rejects_an_unclosed_parenthesis() {
        let err = parse_mask("a(b,c").unwrap_err();
        assert!(err.contains("unclosed '('"), "{err}");
    }

    #[test]
    fn rejects_a_stray_closing_parenthesis() {
        let err = parse_mask("a)b").unwrap_err();
        assert!(err.contains("unexpected ')'"), "{err}");
    }

    #[test]
    fn rejects_an_empty_sub_selection() {
        let err = parse_mask("a()").unwrap_err();
        assert!(err.contains("empty sub-selection"), "{err}");
    }

    #[test]
    fn rejects_an_empty_field_name() {
        let err = parse_mask("a,,b").unwrap_err();
        assert!(err.contains("empty field name"), "{err}");
    }

    #[test]
    fn rejects_a_star_inside_a_name() {
        let err = parse_mask("a*b").unwrap_err();
        assert!(err.contains("unescaped '*'"), "{err}");
    }

    #[test]
    fn rejects_a_dangling_backslash() {
        let err = parse_mask(r"a\").unwrap_err();
        assert!(err.contains("dangling"), "{err}");
    }

    #[test]
    fn rejects_a_scalar_document() {
        let err = mask_json("42", "a", Options::default()).unwrap_err();
        assert!(err.contains("got number"), "{err}");
    }

    #[test]
    fn rejects_an_oversized_mask() {
        let big = "a".repeat(MAX_MASK_BYTES + 1);
        let err = parse_mask(&big).unwrap_err();
        assert!(err.contains("maximum is"), "{err}");
    }

    #[test]
    fn rejects_an_over_deep_mask() {
        let deep = format!(
            "{}x{}",
            "a(".repeat(MAX_MASK_DEPTH + 2),
            ")".repeat(MAX_MASK_DEPTH + 2)
        );
        let err = parse_mask(&deep).unwrap_err();
        assert!(err.contains("nests deeper"), "{err}");
    }

    #[test]
    fn run_parses_string_options() {
        assert_eq!(
            run(r#"{"a":1,"b":2}"#, "a", "keep", "compact", "omit", "keep").unwrap(),
            r#"{"a":1}"#
        );
        let err = run(r#"{"a":1}"#, "a", "nope", "compact", "omit", "keep").unwrap_err();
        assert!(err.contains("unknown mode 'nope'"), "{err}");
    }

    #[test]
    fn run_defaults_on_empty_option_strings() {
        assert_eq!(
            run(r#"{"a":1,"b":2}"#, "b", "", "", "", "").unwrap(),
            "{\n  \"b\": 2\n}"
        );
    }
}
