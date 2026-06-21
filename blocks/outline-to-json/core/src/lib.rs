//! outline-to-json core — convert an indented text outline into nested JSON.
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.
//!
//! Each non-blank line is a node; the leading whitespace (indentation) of a line
//! relative to its predecessors determines nesting. A more-indented line is a
//! child of the nearest preceding less-indented line. Tabs and spaces are both
//! supported; a tab counts as `tab_size` columns (default 4) so mixed indentation
//! still nests sensibly.

use serde_json::{json, Map, Value};

/// Output shape.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    /// Array of node objects: `[{ "text": "a", "children": [ … ] }]`.
    /// Leaf nodes have an empty `children` array. Order is preserved.
    Children,
    /// Nested object keyed by node text: `{ "a": { "b": {} } }`.
    /// A leaf is an empty object. Duplicate sibling keys keep the LAST value.
    Nested,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "children" => Ok(Format::Children),
            "nested" => Ok(Format::Nested),
            other => Err(format!(
                "invalid format {other:?}: expected \"children\" or \"nested\""
            )),
        }
    }
}

/// A parsed node in the outline tree.
struct Node {
    text: String,
    children: Vec<Node>,
}

/// Convert an indented text `outline` into JSON.
///
/// - `format` (`"children"` | `"nested"`, blank → `"children"`): output shape.
/// - `tab_size`: how many columns a literal tab counts for when measuring
///   indentation (clamped to `1..=16`; 0/blank → 4). Lets you mix tabs & spaces.
/// - `pretty`: pretty-print the JSON with 2-space indentation (default true).
///
/// Blank lines (including whitespace-only lines) are ignored. Returns `Err` on an
/// invalid `format`, on empty input (no content lines), or when the first content
/// line is itself indented (there is no root to attach it to).
pub fn convert(outline: &str, format: &str, tab_size: u32, pretty: bool) -> Result<String, String> {
    let fmt = Format::parse(format)?;
    let tab = if tab_size == 0 { 4 } else { tab_size.clamp(1, 16) } as usize;

    // (indent_columns, trimmed_text) for each non-blank line.
    let lines: Vec<(usize, String)> = outline
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| (indent_columns(l, tab), l.trim().to_string()))
        .collect();

    if lines.is_empty() {
        return Err("outline is empty: provide at least one non-blank line".into());
    }
    if lines[0].0 != 0 {
        return Err("the first line must not be indented (it is the root)".into());
    }

    let roots = build_forest(&lines);
    let value = match fmt {
        Format::Children => forest_to_children(&roots),
        Format::Nested => forest_to_nested(&roots),
    };

    let s = if pretty {
        serde_json::to_string_pretty(&value)
    } else {
        serde_json::to_string(&value)
    };
    s.map_err(|e| format!("failed to serialize JSON: {e}"))
}

/// Count the indentation width of a line in columns, expanding leading tabs to
/// `tab` columns each. Stops at the first non-whitespace character.
fn indent_columns(line: &str, tab: usize) -> usize {
    let mut cols = 0;
    for ch in line.chars() {
        match ch {
            ' ' => cols += 1,
            '\t' => cols += tab,
            _ => break,
        }
    }
    cols
}

/// Build a forest from `(indent, text)` lines using an explicit stack of open
/// ancestor indent levels. A line attaches to the nearest preceding line with
/// strictly smaller indentation; equal indentation makes it a sibling. Indentation
/// depth need not be uniform — only the relative order matters. We navigate the
/// tree by index (Rust can't hold mutable references up the tree).
fn build_forest(lines: &[(usize, String)]) -> Vec<Node> {
    let mut roots: Vec<Node> = Vec::new();
    let mut stack: Vec<usize> = Vec::new(); // indent columns at each open level

    for (indent, text) in lines {
        // Pop ancestors that are not strictly less-indented than this line.
        while let Some(&top) = stack.last() {
            if top >= *indent {
                stack.pop();
            } else {
                break;
            }
        }
        let node = Node {
            text: text.clone(),
            children: Vec::new(),
        };
        let path = stack_path_indices(&roots, &stack);
        insert_at_path(&mut roots, &path, node);
        stack.push(*indent);
    }
    roots
}

/// Resolve the stack of open indent levels into the concrete child-index path of
/// the last node at each level (always the last child, since we append in order).
fn stack_path_indices(roots: &[Node], stack: &[usize]) -> Vec<usize> {
    let mut path = Vec::with_capacity(stack.len());
    let mut level = roots;
    for _ in stack {
        if level.is_empty() {
            break;
        }
        let idx = level.len() - 1;
        path.push(idx);
        level = &level[idx].children;
    }
    path
}

/// Insert `node` as the last child along `path` (a chain of last-child indices).
fn insert_at_path(roots: &mut Vec<Node>, path: &[usize], node: Node) {
    let mut level = roots;
    for &idx in path {
        level = &mut level[idx].children;
    }
    level.push(node);
}

fn forest_to_children(nodes: &[Node]) -> Value {
    Value::Array(
        nodes
            .iter()
            .map(|n| {
                json!({
                    "text": n.text,
                    "children": forest_to_children(&n.children),
                })
            })
            .collect(),
    )
}

fn forest_to_nested(nodes: &[Node]) -> Value {
    let mut map = Map::new();
    for n in nodes {
        map.insert(n.text.clone(), forest_to_nested(&n.children));
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn children_format_nests_by_indentation() {
        let out = convert("a\n  b\n  c\n    d\ne", "children", 4, false).unwrap();
        assert_eq!(
            out,
            r#"[{"text":"a","children":[{"text":"b","children":[]},{"text":"c","children":[{"text":"d","children":[]}]}]},{"text":"e","children":[]}]"#
        );
    }

    #[test]
    fn nested_format_keys_by_text() {
        let out = convert("a\n  b\n    c", "nested", 4, false).unwrap();
        assert_eq!(out, r#"{"a":{"b":{"c":{}}}}"#);
    }

    #[test]
    fn defaults_to_children_pretty() {
        // blank format => children; pretty true via convert arg.
        let out = convert("root\n  child", "", 4, true).unwrap();
        assert!(out.starts_with("[\n"), "expected pretty output, got: {out}");
        assert!(out.contains("\"text\": \"root\""));
    }

    #[test]
    fn tabs_count_as_tab_size_columns() {
        // A tab-indented child nests under its parent.
        let out = convert("a\n\tb", "nested", 4, false).unwrap();
        assert_eq!(out, r#"{"a":{"b":{}}}"#);
    }

    #[test]
    fn blank_lines_are_ignored() {
        let out = convert("a\n\n  b\n   \n  c", "nested", 4, false).unwrap();
        assert_eq!(out, r#"{"a":{"b":{},"c":{}}}"#);
    }

    #[test]
    fn dedent_pops_back_to_ancestor() {
        // c dedents past b back under a as a sibling of b.
        let out = convert("a\n  b\n    deep\n  c", "children", 4, false).unwrap();
        assert_eq!(
            out,
            r#"[{"text":"a","children":[{"text":"b","children":[{"text":"deep","children":[]}]},{"text":"c","children":[]}]}]"#
        );
    }

    #[test]
    fn multiple_roots_form_an_array() {
        let out = convert("one\ntwo\nthree", "children", 4, false).unwrap();
        assert_eq!(
            out,
            r#"[{"text":"one","children":[]},{"text":"two","children":[]},{"text":"three","children":[]}]"#
        );
    }

    #[test]
    fn nonuniform_indentation_uses_relative_depth() {
        // 3-space then 7-space indentation still nests two levels deep.
        let out = convert("a\n   b\n       c", "nested", 4, false).unwrap();
        assert_eq!(out, r#"{"a":{"b":{"c":{}}}}"#);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = convert("   \n\n", "children", 4, false).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn leading_indent_on_first_line_is_an_error() {
        let err = convert("  a\nb", "children", 4, false).unwrap_err();
        assert!(err.contains("first line"), "got: {err}");
    }

    #[test]
    fn invalid_format_is_an_error() {
        let err = convert("a", "tree", 4, false).unwrap_err();
        assert!(err.contains("invalid format"), "got: {err}");
    }

    #[test]
    fn tab_size_zero_defaults_to_four() {
        // tab_size 0 should behave as 4; a single tab still nests one level.
        let out = convert("a\n\tb", "nested", 0, false).unwrap();
        assert_eq!(out, r#"{"a":{"b":{}}}"#);
    }

    #[test]
    fn nested_duplicate_siblings_keep_last() {
        let out = convert("a\n  x\n  x", "nested", 4, false).unwrap();
        // Object keys are unique; the last sibling wins (children format keeps both).
        assert_eq!(out, r#"{"a":{"x":{}}}"#);
        let out2 = convert("a\n  x\n  x", "children", 4, false).unwrap();
        assert_eq!(
            out2,
            r#"[{"text":"a","children":[{"text":"x","children":[]},{"text":"x","children":[]}]}]"#
        );
    }
}
