//! json-to-markdown core — render an arbitrary JSON document (e.g. a notes-app
//! export/backup) as readable, nested Markdown. Pure-Rust (`serde_json` with
//! `preserve_order` so object keys keep their document order). No
//! wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.
//!
//! Rendering model
//! ---------------
//! * A top-level **object** becomes a set of Markdown sections: each scalar key
//!   is a `- **key**: value` bullet, and each nested object/array key becomes a
//!   heading (`#`…`######`, starting at `heading_level` and getting one level
//!   deeper per nesting) followed by its rendered contents.
//! * A top-level **array** (or any array nested under a key) renders as a
//!   GitHub-flavored **table** when it is a uniform array of flat objects (the
//!   default `auto` behavior), otherwise as a nested **bullet list**. `array_mode`
//!   forces `table` or `list`.
//! * A top-level **scalar** renders as itself.
//! * Once nesting passes `max_depth`, a subtree is emitted verbatim as a fenced
//!   `json` code block instead of being expanded further (keeps very deep data
//!   readable and bounded).

use serde_json::{Map, Value};

/// Options controlling the Markdown rendering. See the module docs.
#[derive(Clone, Copy)]
struct Opts {
    /// Heading level (1..=6) used for the outermost object/array sections.
    heading_level: u32,
    /// How arrays of objects render.
    array_mode: ArrayMode,
    /// Sort object keys / table columns alphabetically instead of document order.
    sort_keys: bool,
    /// Nesting depth (1..=20) beyond which subtrees collapse to a fenced JSON block.
    max_depth: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum ArrayMode {
    /// Uniform flat-object arrays become tables; everything else becomes a list.
    Auto,
    /// Every array of objects becomes a table (non-scalar cells become inline JSON).
    Table,
    /// Every array becomes a nested bullet list.
    List,
}

impl ArrayMode {
    fn parse(mode: &str) -> Result<Self, String> {
        match mode {
            "" | "auto" => Ok(ArrayMode::Auto),
            "table" => Ok(ArrayMode::Table),
            "list" => Ok(ArrayMode::List),
            other => Err(format!(
                "invalid array_mode {other:?}: expected \"auto\", \"table\", or \"list\""
            )),
        }
    }
}

/// Lower and upper bounds for the tunable numeric params. Referenced by the
/// descriptor so the LLM-facing schema can't drift from what's enforced here.
pub const MAX_HEADING_LEVEL: u32 = 6;
pub const MAX_MAX_DEPTH: u32 = 20;

/// Render `json` (an arbitrary JSON document) as nested Markdown.
///
/// - `heading_level` (1..=6, clamped): heading level for the top-level sections.
/// - `array_mode` (`"auto"` | `"table"` | `"list"`, blank → `"auto"`): how arrays
///   of objects render.
/// - `sort_keys`: sort object keys and table columns alphabetically (default:
///   document order).
/// - `max_depth` (1..=20, clamped): nesting levels expanded as Markdown before a
///   subtree collapses to a fenced `json` block.
///
/// Returns `Err` on invalid JSON or an unknown `array_mode`.
pub fn to_markdown(
    json: &str,
    heading_level: u32,
    array_mode: &str,
    sort_keys: bool,
    max_depth: u32,
) -> Result<String, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let opts = Opts {
        heading_level: heading_level.clamp(1, MAX_HEADING_LEVEL),
        array_mode: ArrayMode::parse(array_mode)?,
        sort_keys,
        max_depth: max_depth.clamp(1, MAX_MAX_DEPTH),
    };

    let mut out = String::new();
    match &value {
        Value::Object(map) => render_sections(map, opts.heading_level, 1, opts, &mut out),
        Value::Array(arr) => render_array(arr, opts.heading_level, 1, opts, &mut out),
        // A bare scalar document: emit it as-is (no flattening).
        scalar => out.push_str(&scalar_to_string(scalar)),
    }
    Ok(tidy(&out))
}

/// Render an object as heading sections + scalar bullets.
fn render_sections(map: &Map<String, Value>, level: u32, depth: u32, opts: Opts, out: &mut String) {
    if map.is_empty() {
        push_line(out, "_(empty object)_");
        return;
    }
    for key in ordered_keys(map, opts.sort_keys) {
        let v = &map[key];
        match v {
            Value::Object(_) | Value::Array(_) => {
                // A heading needs a blank line before it to render as a heading
                // (rather than being absorbed into a preceding list).
                push_blank(out);
                push_line(out, &format!("{} {}", "#".repeat(level as usize), key));
                push_blank(out);
                if depth >= opts.max_depth {
                    push_fenced_json(v, out);
                    continue;
                }
                match v {
                    Value::Object(child) => render_sections(
                        child,
                        (level + 1).min(MAX_HEADING_LEVEL),
                        depth + 1,
                        opts,
                        out,
                    ),
                    Value::Array(child) => render_array(
                        child,
                        (level + 1).min(MAX_HEADING_LEVEL),
                        depth + 1,
                        opts,
                        out,
                    ),
                    _ => unreachable!(),
                }
                push_blank(out);
            }
            scalar => push_line(out, &format!("- **{}**: {}", key, inline(scalar))),
        }
    }
}

/// Render an array as a table (when eligible) or a nested bullet list.
fn render_array(arr: &[Value], level: u32, depth: u32, opts: Opts, out: &mut String) {
    let _ = level;
    if arr.is_empty() {
        push_line(out, "_(empty array)_");
        return;
    }
    if depth > opts.max_depth {
        push_fenced_json(&Value::Array(arr.to_vec()), out);
        return;
    }
    let use_table = match opts.array_mode {
        ArrayMode::List => false,
        ArrayMode::Table => all_objects(arr),
        ArrayMode::Auto => is_flat_object_array(arr),
    };
    if use_table {
        render_table(arr, opts, out);
    } else {
        list_array(arr, 0, depth, opts, out);
    }
}

/// Render a uniform array of objects as a GitHub-flavored pipe table.
fn render_table(arr: &[Value], opts: Opts, out: &mut String) {
    // Column set = union of keys across all rows, in first-seen (or sorted) order.
    let mut cols: Vec<String> = Vec::new();
    for elem in arr {
        if let Value::Object(map) = elem {
            for k in map.keys() {
                if !cols.iter().any(|c| c == k) {
                    cols.push(k.clone());
                }
            }
        }
    }
    if opts.sort_keys {
        cols.sort();
    }
    push_line(out, &format!("| {} |", cols.join(" | ")));
    push_line(
        out,
        &format!(
            "| {} |",
            cols.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
        ),
    );
    for elem in arr {
        let cells: Vec<String> = cols
            .iter()
            .map(|c| match elem {
                Value::Object(map) => map.get(c).map(cell).unwrap_or_default(),
                _ => String::new(),
            })
            .collect();
        push_line(out, &format!("| {} |", cells.join(" | ")));
    }
}

/// Render an array's elements as a bullet list at the given indent.
fn list_array(arr: &[Value], indent: usize, depth: u32, opts: Opts, out: &mut String) {
    let pad = "  ".repeat(indent);
    for elem in arr {
        match elem {
            Value::Object(map) if !map.is_empty() && depth < opts.max_depth => {
                // A bullet whose children are the object's fields, indented once more.
                push_line(out, &format!("{pad}-"));
                list_object(map, indent + 1, depth + 1, opts, out);
            }
            Value::Array(inner) if !inner.is_empty() && depth < opts.max_depth => {
                push_line(out, &format!("{pad}-"));
                list_array(inner, indent + 1, depth + 1, opts, out);
            }
            Value::Object(_) | Value::Array(_) => {
                // Empty container, or past the depth cap → compact inline JSON.
                push_line(out, &format!("{pad}- `{}`", compact_json(elem)));
            }
            scalar => push_line(out, &format!("{pad}- {}", inline(scalar))),
        }
    }
}

/// Render an object's fields as a bullet list at the given indent.
fn list_object(map: &Map<String, Value>, indent: usize, depth: u32, opts: Opts, out: &mut String) {
    let pad = "  ".repeat(indent);
    for key in ordered_keys(map, opts.sort_keys) {
        let v = &map[key];
        match v {
            Value::Object(child) if !child.is_empty() && depth < opts.max_depth => {
                push_line(out, &format!("{pad}- **{key}**:"));
                list_object(child, indent + 1, depth + 1, opts, out);
            }
            Value::Array(child) if !child.is_empty() && depth < opts.max_depth => {
                push_line(out, &format!("{pad}- **{key}**:"));
                list_array(child, indent + 1, depth + 1, opts, out);
            }
            Value::Object(_) | Value::Array(_) => {
                push_line(out, &format!("{pad}- **{key}**: `{}`", compact_json(v)));
            }
            scalar => push_line(out, &format!("{pad}- **{key}**: {}", inline(scalar))),
        }
    }
}

// ---- helpers ---------------------------------------------------------------

fn ordered_keys<'a>(map: &'a Map<String, Value>, sort: bool) -> Vec<&'a String> {
    let mut keys: Vec<&String> = map.keys().collect();
    if sort {
        keys.sort();
    }
    keys
}

fn all_objects(arr: &[Value]) -> bool {
    !arr.is_empty() && arr.iter().all(|v| v.is_object())
}

/// A uniform array of objects whose every value is a scalar — the clean table case.
fn is_flat_object_array(arr: &[Value]) -> bool {
    all_objects(arr)
        && arr.iter().all(|v| {
            v.as_object()
                .unwrap()
                .values()
                .all(|c| !c.is_object() && !c.is_array())
        })
}

/// Scalar → plain string. `null` → empty; strings verbatim; numbers/bools as text.
fn scalar_to_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        // Containers never reach here from the scalar paths.
        other => compact_json(other),
    }
}

/// Inline a scalar for a bullet/heading context: single-line (newlines → spaces).
fn inline(v: &Value) -> String {
    scalar_to_string(v).replace(['\n', '\r'], " ")
}

/// A table cell: single-line and pipe-escaped. Container cells become inline JSON.
fn cell(v: &Value) -> String {
    let raw = if v.is_object() || v.is_array() {
        compact_json(v)
    } else {
        scalar_to_string(v)
    };
    raw.replace(['\n', '\r'], " ").replace('|', "\\|")
}

fn compact_json(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn push_fenced_json(v: &Value, out: &mut String) {
    push_line(out, "```json");
    push_line(out, &serde_json::to_string_pretty(v).unwrap_or_default());
    push_line(out, "```");
    push_blank(out);
}

fn push_line(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
}

fn push_blank(out: &mut String) {
    out.push('\n');
}

/// Trim trailing per-line whitespace, collapse 3+ blank lines to 1, and trim the
/// leading/trailing blank lines so the document is clean.
fn tidy(s: &str) -> String {
    let mut lines: Vec<&str> = s.lines().map(|l| l.trim_end()).collect();
    // Collapse runs of blank lines to a single blank line.
    let mut collapsed: Vec<&str> = Vec::with_capacity(lines.len());
    let mut prev_blank = false;
    for l in lines.drain(..) {
        let blank = l.is_empty();
        if blank && prev_blank {
            continue;
        }
        collapsed.push(l);
        prev_blank = blank;
    }
    while collapsed.first().is_some_and(|l| l.is_empty()) {
        collapsed.remove(0);
    }
    while collapsed.last().is_some_and(|l| l.is_empty()) {
        collapsed.pop();
    }
    collapsed.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(json: &str) -> String {
        to_markdown(json, 2, "auto", false, 6).unwrap()
    }

    #[test]
    fn top_level_object_scalars_and_nested_section() {
        let out = md(r#"{"title":"My Note","pinned":true,"meta":{"tags":"work","words":42}}"#);
        assert_eq!(
            out,
            "- **title**: My Note\n\
             - **pinned**: true\n\
             \n\
             ## meta\n\
             \n\
             - **tags**: work\n\
             - **words**: 42"
        );
    }

    #[test]
    fn flat_object_array_renders_as_table() {
        let out = md(r#"[{"name":"Ada","role":"Editor"},{"name":"Bob","role":"Reviewer"}]"#);
        assert_eq!(
            out,
            "| name | role |\n\
             | --- | --- |\n\
             | Ada | Editor |\n\
             | Bob | Reviewer |"
        );
    }

    #[test]
    fn table_unions_columns_and_blanks_missing_cells() {
        let out = md(r#"[{"a":1},{"a":2,"b":3}]"#);
        assert_eq!(out, "| a | b |\n| --- | --- |\n| 1 |  |\n| 2 | 3 |");
    }

    #[test]
    fn scalar_array_renders_as_bullets() {
        let out = md(r#"["red","green","blue"]"#);
        assert_eq!(out, "- red\n- green\n- blue");
    }

    #[test]
    fn heading_level_option_sets_starting_depth() {
        let out = to_markdown(r#"{"a":{"b":1}}"#, 3, "auto", false, 6).unwrap();
        assert_eq!(out, "### a\n\n- **b**: 1");
    }

    #[test]
    fn sort_keys_sorts_object_keys_and_columns() {
        let out = to_markdown(r#"{"b":1,"a":2}"#, 2, "auto", true, 6).unwrap();
        assert_eq!(out, "- **a**: 2\n- **b**: 1");
        let tbl = to_markdown(r#"[{"b":1,"a":2}]"#, 2, "auto", true, 6).unwrap();
        assert_eq!(tbl, "| a | b |\n| --- | --- |\n| 2 | 1 |");
    }

    #[test]
    fn list_mode_forces_bullets_for_object_array() {
        let out = to_markdown(r#"[{"name":"Ada","role":"Editor"}]"#, 2, "list", false, 6).unwrap();
        assert_eq!(out, "-\n  - **name**: Ada\n  - **role**: Editor");
    }

    #[test]
    fn nested_object_array_uses_bullet_hierarchy_not_table() {
        // An array whose objects contain a nested array is not flat → bullets.
        let out = md(r#"[{"name":"Ada","tags":["x","y"]}]"#);
        assert_eq!(out, "-\n  - **name**: Ada\n  - **tags**:\n    - x\n    - y");
    }

    #[test]
    fn null_renders_as_empty_and_pipes_escaped_in_tables() {
        let out = md(r#"[{"k":null,"v":"a|b"}]"#);
        assert_eq!(out, "| k | v |\n| --- | --- |\n|  | a\\|b |");
    }

    #[test]
    fn max_depth_collapses_deep_subtree_to_code_fence() {
        let out = to_markdown(r#"{"a":{"b":{"c":1}}}"#, 2, "auto", false, 2).unwrap();
        assert_eq!(out, "## a\n\n### b\n\n```json\n{\n  \"c\": 1\n}\n```");
    }

    #[test]
    fn bare_scalar_document_renders_verbatim() {
        assert_eq!(md(r#""hello world""#), "hello world");
        assert_eq!(md("42"), "42");
    }

    #[test]
    fn invalid_json_is_an_error() {
        let err = to_markdown("{not json", 2, "auto", false, 6).unwrap_err();
        assert!(err.contains("invalid JSON"), "got: {err}");
    }

    #[test]
    fn invalid_array_mode_is_an_error() {
        let err = to_markdown("{}", 2, "grid", false, 6).unwrap_err();
        assert!(err.contains("invalid array_mode"), "got: {err}");
    }

    #[test]
    fn empty_containers_are_labeled() {
        assert_eq!(md("{}"), "_(empty object)_");
        assert_eq!(md("[]"), "_(empty array)_");
    }
}
