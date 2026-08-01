//! json-to-html-table core — render a JSON array or object as a clean HTML or
//! Markdown table. Pure-Rust (`serde_json` with `preserve_order` so object keys
//! keep their document order). No wafer/wasm-bindgen deps.
//!
//! Input shapes handled:
//! - **Array of objects** `[{…},{…}]` → columns are the union of the objects'
//!   keys in first-seen order; one row per object; a missing key renders as
//!   `null_text`.
//! - **Array of arrays** `[[…],[…]]` → each inner array is a row; with
//!   `header=true` the first inner array is the header, otherwise `Column N`.
//! - **Array of scalars** `[1,2,3]` → a single-column table (each scalar a row).
//! - **Single object** `{…}` → a two-column `key` / `value` table.
//!
//! Nested objects/arrays are handled per the `nested` strategy:
//! - `json` (default) — the nested value is a compact JSON string in the cell.
//! - `table` — the nested value becomes a nested `<table>` (HTML only; Markdown
//!   can't nest tables, so it falls back to a compact JSON string).
//! - `flatten` — nested objects are hoisted into dotted-key columns
//!   (`user.id`, `user.name`); nested arrays still render as a compact JSON
//!   string.

use serde_json::Value;

/// Output table format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Html,
    Markdown,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "html" | "" => Ok(Format::Html),
            "markdown" | "md" => Ok(Format::Markdown),
            other => Err(format!("unknown format '{other}' (use html or markdown)")),
        }
    }
}

/// How nested objects/arrays are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nested {
    /// Compact JSON string in the cell, e.g. `{"a":1}` or `[1,2]`.
    Json,
    /// A nested `<table>` (HTML only; Markdown falls back to compact JSON).
    Table,
    /// Hoist nested objects into dotted-key columns; arrays stay compact JSON.
    Flatten,
}

impl Nested {
    pub fn parse(s: &str) -> Result<Nested, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" | "" => Ok(Nested::Json),
            "table" => Ok(Nested::Table),
            "flatten" => Ok(Nested::Flatten),
            other => Err(format!(
                "unknown nested '{other}' (use json, table or flatten)"
            )),
        }
    }
}

/// Options controlling the JSON → table rendering.
#[derive(Debug, Clone)]
pub struct Options {
    pub format: Format,
    /// For array-of-arrays / array-of-scalars input, treat the first row as the
    /// header (otherwise synthesize `Column N`). Arrays-of-objects always use
    /// the object keys, and a single object always uses `key`/`value`.
    pub header: bool,
    /// Text rendered for a JSON `null` or a missing column value.
    pub null_text: String,
    /// How nested objects/arrays are handled.
    pub nested: Nested,
    /// Optional `<caption>` text for the HTML table (HTML only). Empty = none.
    pub caption: String,
    /// CSS class(es) added to the top-level `<table>` (HTML only). Empty = none.
    pub table_class: String,
    /// Pretty-print (indented, multi-line) HTML when true; single-line when
    /// false (HTML only — Markdown is unaffected).
    pub pretty: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: Format::Html,
            header: true,
            null_text: String::new(),
            nested: Nested::Json,
            caption: String::new(),
            table_class: String::new(),
            pretty: true,
        }
    }
}

/// A normalized grid: a header row plus body rows of raw JSON values.
struct Grid {
    header: Vec<String>,
    rows: Vec<Vec<Value>>,
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn md_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', " ")
}

fn compact_json(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// Plain (unescaped) text for a scalar/null value. Objects/arrays are compact
/// JSON — used for header cells, where nested tables never apply.
fn scalar_text(v: &Value, null_text: &str) -> String {
    match v {
        Value::Null => null_text.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => compact_json(v),
    }
}

/// Hoist an object's leaf values into `(dotted_key, value)` pairs. A non-empty
/// object recurses; empty objects, arrays, and scalars are leaves at their
/// current key.
fn flatten_leaves(prefix: &str, v: &Value, out: &mut Vec<(String, Value)>) {
    match v {
        Value::Object(map) if !map.is_empty() => {
            for (k, val) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_leaves(&key, val, out);
            }
        }
        _ => out.push((prefix.to_string(), v.clone())),
    }
}

/// Build the normalized grid from the top-level JSON value.
fn build_grid(v: &Value, opt: &Options) -> Result<Grid, String> {
    match v {
        Value::Object(map) => {
            if map.is_empty() {
                return Err("JSON object is empty — nothing to tabulate".into());
            }
            let rows = if opt.nested == Nested::Flatten {
                let mut leaves = Vec::new();
                flatten_leaves("", v, &mut leaves);
                leaves
                    .into_iter()
                    .map(|(k, val)| vec![Value::String(k), val])
                    .collect()
            } else {
                map.iter()
                    .map(|(k, val)| vec![Value::String(k.clone()), val.clone()])
                    .collect()
            };
            Ok(Grid {
                header: vec!["key".into(), "value".into()],
                rows,
            })
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                return Err("JSON array is empty — nothing to tabulate".into());
            }
            if arr.iter().all(Value::is_object) {
                if opt.nested == Nested::Flatten {
                    // Flattened dotted-key columns, union across all objects.
                    let per_obj: Vec<Vec<(String, Value)>> = arr
                        .iter()
                        .map(|obj| {
                            let mut leaves = Vec::new();
                            flatten_leaves("", obj, &mut leaves);
                            leaves
                        })
                        .collect();
                    let mut cols: Vec<String> = Vec::new();
                    for leaves in &per_obj {
                        for (k, _) in leaves {
                            if !cols.iter().any(|c| c == k) {
                                cols.push(k.clone());
                            }
                        }
                    }
                    let rows = per_obj
                        .iter()
                        .map(|leaves| {
                            cols.iter()
                                .map(|c| {
                                    leaves
                                        .iter()
                                        .find(|(k, _)| k == c)
                                        .map(|(_, v)| v.clone())
                                        .unwrap_or(Value::Null)
                                })
                                .collect()
                        })
                        .collect();
                    return Ok(Grid { header: cols, rows });
                }
                // Array of objects → union of keys in first-seen order.
                let mut cols: Vec<String> = Vec::new();
                for obj in arr {
                    if let Value::Object(map) = obj {
                        for k in map.keys() {
                            if !cols.iter().any(|c| c == k) {
                                cols.push(k.clone());
                            }
                        }
                    }
                }
                let rows = arr
                    .iter()
                    .map(|obj| {
                        cols.iter()
                            .map(|c| obj.get(c).cloned().unwrap_or(Value::Null))
                            .collect()
                    })
                    .collect();
                Ok(Grid { header: cols, rows })
            } else {
                // Array of rows: inner arrays become cells, scalars a single cell.
                let mut rowvals: Vec<Vec<Value>> = arr
                    .iter()
                    .map(|e| match e {
                        Value::Array(inner) => inner.clone(),
                        other => vec![other.clone()],
                    })
                    .collect();
                let width = rowvals.iter().map(Vec::len).max().unwrap_or(1).max(1);
                for r in &mut rowvals {
                    while r.len() < width {
                        r.push(Value::Null);
                    }
                }
                if opt.header {
                    let head_vals = rowvals.remove(0);
                    let head: Vec<String> = head_vals
                        .iter()
                        .map(|v| scalar_text(v, &opt.null_text))
                        .collect();
                    Ok(Grid {
                        header: head,
                        rows: rowvals,
                    })
                } else {
                    let head: Vec<String> = (1..=width).map(|i| format!("Column {i}")).collect();
                    Ok(Grid {
                        header: head,
                        rows: rowvals,
                    })
                }
            }
        }
        _ => Err("expected a JSON array or object, got a scalar value".into()),
    }
}

/// Render a cell value as HTML (already escaped / including any nested table).
fn html_cell(v: &Value, opt: &Options) -> String {
    match v {
        Value::Object(_) | Value::Array(_) if opt.nested == Nested::Table => {
            // Recurse into a compact nested table; fall back to compact JSON on
            // an empty object/array (which build_grid rejects).
            match build_grid(v, opt) {
                Ok(grid) => {
                    let mut nested_opt = opt.clone();
                    nested_opt.pretty = false;
                    nested_opt.caption = String::new();
                    nested_opt.table_class = String::new();
                    render_html(&grid, &nested_opt)
                }
                Err(_) => html_escape(&compact_json(v)),
            }
        }
        Value::Object(_) | Value::Array(_) => html_escape(&compact_json(v)),
        _ => html_escape(&scalar_text(v, &opt.null_text)),
    }
}

fn render_html(grid: &Grid, opt: &Options) -> String {
    let open = if opt.table_class.trim().is_empty() {
        "<table>".to_string()
    } else {
        format!("<table class=\"{}\">", html_escape(opt.table_class.trim()))
    };
    let caption = opt.caption.trim();
    let ths: String = grid
        .header
        .iter()
        .map(|h| format!("<th>{}</th>", html_escape(h)))
        .collect();
    if opt.pretty {
        let mut out = String::new();
        out.push_str(&open);
        out.push('\n');
        if !caption.is_empty() {
            out.push_str(&format!("  <caption>{}</caption>\n", html_escape(caption)));
        }
        out.push_str("  <thead>\n");
        out.push_str(&format!("    <tr>{ths}</tr>\n"));
        out.push_str("  </thead>\n  <tbody>\n");
        for row in &grid.rows {
            let tds: String = row
                .iter()
                .map(|c| format!("<td>{}</td>", html_cell(c, opt)))
                .collect();
            out.push_str(&format!("    <tr>{tds}</tr>\n"));
        }
        out.push_str("  </tbody>\n</table>");
        out
    } else {
        let mut out = String::new();
        out.push_str(&open);
        if !caption.is_empty() {
            out.push_str(&format!("<caption>{}</caption>", html_escape(caption)));
        }
        out.push_str(&format!("<thead><tr>{ths}</tr></thead><tbody>"));
        for row in &grid.rows {
            let tds: String = row
                .iter()
                .map(|c| format!("<td>{}</td>", html_cell(c, opt)))
                .collect();
            out.push_str(&format!("<tr>{tds}</tr>"));
        }
        out.push_str("</tbody></table>");
        out
    }
}

fn md_cell(v: &Value, null_text: &str) -> String {
    match v {
        Value::Object(_) | Value::Array(_) => md_escape(&compact_json(v)),
        _ => md_escape(&scalar_text(v, null_text)),
    }
}

fn render_markdown(grid: &Grid, opt: &Options) -> String {
    let width = grid.header.len().max(1);
    let mut out = String::new();
    let head: Vec<String> = grid.header.iter().map(|h| md_escape(h)).collect();
    out.push_str(&format!("| {} |\n", head.join(" | ")));
    out.push_str(&format!("| {} |\n", vec!["---"; width].join(" | ")));
    for row in &grid.rows {
        let cells: Vec<String> = (0..width)
            .map(|i| md_cell(row.get(i).unwrap_or(&Value::Null), &opt.null_text))
            .collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    out.trim_end().to_string()
}

/// Render a JSON document (`json`) as an HTML or Markdown table.
pub fn to_table(json: &str, opt: &Options) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("input JSON is empty".into());
    }
    let value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let grid = build_grid(&value, opt)?;
    Ok(match opt.format {
        Format::Html => render_html(&grid, opt),
        Format::Markdown => render_markdown(&grid, opt),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opt(format: Format) -> Options {
        Options {
            format,
            ..Options::default()
        }
    }

    #[test]
    fn array_of_objects_html() {
        let out = to_table(
            r#"[{"id":1,"name":"Ada"},{"id":2,"name":"Linus"}]"#,
            &opt(Format::Html),
        )
        .unwrap();
        assert!(out.starts_with("<table>\n"));
        assert!(out.contains("<tr><th>id</th><th>name</th></tr>"));
        assert!(out.contains("<tr><td>1</td><td>Ada</td></tr>"));
        assert!(out.contains("<tr><td>2</td><td>Linus</td></tr>"));
        assert!(out.ends_with("</table>"));
    }

    #[test]
    fn array_of_objects_markdown_union_keys() {
        // second object introduces a new column; first object misses it → null_text
        let out = to_table(
            r#"[{"a":1},{"a":2,"b":3}]"#,
            &Options {
                format: Format::Markdown,
                null_text: "—".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(out, "| a | b |\n| --- | --- |\n| 1 | — |\n| 2 | 3 |");
    }

    #[test]
    fn single_object_key_value() {
        let out = to_table(r#"{"name":"Ada","age":36}"#, &opt(Format::Markdown)).unwrap();
        assert_eq!(
            out,
            "| key | value |\n| --- | --- |\n| name | Ada |\n| age | 36 |"
        );
    }

    #[test]
    fn array_of_arrays_header() {
        let out = to_table(r#"[["a","b"],[1,2],[3,4]]"#, &opt(Format::Markdown)).unwrap();
        assert_eq!(out, "| a | b |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |");
    }

    #[test]
    fn array_of_arrays_no_header_synthesizes_columns() {
        let out = to_table(
            r#"[[1,2],[3,4]]"#,
            &Options {
                format: Format::Markdown,
                header: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            "| Column 1 | Column 2 |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |"
        );
    }

    #[test]
    fn array_of_scalars_single_column() {
        let out = to_table(
            r#"["x","y","z"]"#,
            &Options {
                format: Format::Markdown,
                header: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(out, "| Column 1 |\n| --- |\n| x |\n| y |\n| z |");
    }

    #[test]
    fn ragged_rows_padded() {
        let out = to_table(r#"[["a","b","c"],[1,2],[3]]"#, &opt(Format::Markdown)).unwrap();
        assert_eq!(
            out,
            "| a | b | c |\n| --- | --- | --- |\n| 1 | 2 |  |\n| 3 |  |  |"
        );
    }

    #[test]
    fn nested_json_default() {
        let out = to_table(r#"[{"tags":["x","y"]}]"#, &opt(Format::Html)).unwrap();
        assert!(out.contains("<td>[&quot;x&quot;,&quot;y&quot;]</td>"));
    }

    #[test]
    fn nested_table_html() {
        let out = to_table(
            r#"[{"user":{"id":1}}]"#,
            &Options {
                format: Format::Html,
                nested: Nested::Table,
                ..Options::default()
            },
        )
        .unwrap();
        // nested object rendered as a compact inner <table>
        assert!(out.contains("<td><table><thead><tr><th>key</th><th>value</th></tr></thead>"));
        assert!(out.contains("<tr><td>id</td><td>1</td></tr>"));
    }

    #[test]
    fn nested_flatten_dotted_columns() {
        let out = to_table(
            r#"[{"user":{"id":1,"name":"Ada"}},{"user":{"id":2,"name":"Bo"}}]"#,
            &Options {
                format: Format::Markdown,
                nested: Nested::Flatten,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            "| user.id | user.name |\n| --- | --- |\n| 1 | Ada |\n| 2 | Bo |"
        );
    }

    #[test]
    fn flatten_single_object() {
        let out = to_table(
            r#"{"a":1,"b":{"c":2}}"#,
            &Options {
                format: Format::Markdown,
                nested: Nested::Flatten,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            "| key | value |\n| --- | --- |\n| a | 1 |\n| b.c | 2 |"
        );
    }

    #[test]
    fn nested_markdown_table_falls_back_to_json() {
        let out = to_table(
            r#"[{"tags":["x","y"]}]"#,
            &Options {
                format: Format::Markdown,
                nested: Nested::Table,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(out, "| tags |\n| --- |\n| [\"x\",\"y\"] |");
    }

    #[test]
    fn html_escapes_cells() {
        let out = to_table(r#"[{"x":"<b>&"}]"#, &opt(Format::Html)).unwrap();
        assert!(out.contains("<td>&lt;b&gt;&amp;</td>"));
    }

    #[test]
    fn markdown_escapes_pipe_and_newline() {
        let out = to_table("[{\"x\":\"a|b\\nc\"}]", &opt(Format::Markdown)).unwrap();
        assert!(out.contains("a\\|b c"));
    }

    #[test]
    fn caption_class_and_compact() {
        let out = to_table(
            r#"[{"a":1}]"#,
            &Options {
                format: Format::Html,
                caption: "My data".into(),
                table_class: "t t--zebra".into(),
                pretty: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            "<table class=\"t t--zebra\"><caption>My data</caption><thead><tr><th>a</th></tr></thead><tbody><tr><td>1</td></tr></tbody></table>"
        );
    }

    #[test]
    fn errors() {
        assert!(to_table("", &opt(Format::Html)).is_err());
        assert!(to_table("not json", &opt(Format::Html)).is_err());
        assert!(to_table("42", &opt(Format::Html)).is_err()); // scalar top-level
        assert!(to_table("[]", &opt(Format::Html)).is_err()); // empty array
        assert!(to_table("{}", &opt(Format::Html)).is_err()); // empty object
        assert!(Format::parse("latex").is_err());
        assert!(Nested::parse("bogus").is_err());
    }
}
