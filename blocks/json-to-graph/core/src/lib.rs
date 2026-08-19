//! json-to-graph core — pure compute, shared by the chat skill block and the web page.
//! Turns a JSON document's structure into a node-link graph rendered as Mermaid
//! `flowchart` source or Graphviz `digraph` (DOT) source. No wafer/wasm-bindgen deps.

use std::collections::VecDeque;

use serde_json::Value;

/// Output syntax.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Mermaid,
    Dot,
}

impl Format {
    pub fn parse(s: &str) -> Format {
        match s.trim().to_ascii_lowercase().as_str() {
            "dot" | "graphviz" | "gv" => Format::Dot,
            _ => Format::Mermaid,
        }
    }
}

/// Layout direction. Mermaid uses `TD/LR/BT/RL`, Graphviz `rankdir` uses
/// `TB/LR/BT/RL` — `TD` maps to `TB`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Td,
    Lr,
    Bt,
    Rl,
}

impl Direction {
    pub fn parse(s: &str) -> Direction {
        match s.trim().to_ascii_uppercase().as_str() {
            "LR" => Direction::Lr,
            "BT" => Direction::Bt,
            "RL" => Direction::Rl,
            _ => Direction::Td,
        }
    }
    fn mermaid(self) -> &'static str {
        match self {
            Direction::Td => "TD",
            Direction::Lr => "LR",
            Direction::Bt => "BT",
            Direction::Rl => "RL",
        }
    }
    fn rankdir(self) -> &'static str {
        match self {
            Direction::Td => "TB",
            Direction::Lr => "LR",
            Direction::Bt => "BT",
            Direction::Rl => "RL",
        }
    }
}

/// Rendering options. `0` means "no limit" for `max_depth`, `max_array_items`
/// and `value_max_len`.
#[derive(Clone, Debug)]
pub struct Options {
    pub format: Format,
    pub direction: Direction,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_array_items: usize,
    pub include_values: bool,
    pub value_max_len: usize,
    pub show_types: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: Format::Mermaid,
            direction: Direction::Td,
            max_depth: 0,
            max_nodes: 300,
            max_array_items: 0,
            include_values: true,
            value_max_len: 40,
            show_types: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    Object,
    Array,
    Scalar,
    Ellipsis,
}

struct Node {
    label: String,
    shape: Shape,
}

enum Item<'a> {
    Val {
        parent: Option<usize>,
        key: Option<String>,
        value: &'a Value,
        depth: usize,
    },
    Ell {
        parent: usize,
        label: String,
    },
}

/// Render `json` as a node-link graph in the requested syntax.
pub fn to_graph(json: &str, opts: &Options) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("no input: paste a JSON document (object, array, or scalar)".into());
    }
    if opts.max_nodes == 0 {
        return Err("max_nodes must be at least 1".into());
    }
    let root: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;

    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut queue: VecDeque<Item> = VecDeque::new();
    queue.push_back(Item::Val {
        parent: None,
        key: None,
        value: &root,
        depth: 0,
    });

    // Breadth-first so that hitting `max_nodes` keeps the top levels of the
    // document (a depth-first walk would spend the budget on one branch).
    while nodes.len() < opts.max_nodes {
        let item = match queue.pop_front() {
            Some(i) => i,
            None => break,
        };
        match item {
            Item::Ell { parent, label } => {
                let id = nodes.len();
                nodes.push(Node {
                    label,
                    shape: Shape::Ellipsis,
                });
                edges.push((parent, id));
            }
            Item::Val {
                parent,
                key,
                value,
                depth,
            } => {
                let id = nodes.len();
                nodes.push(Node {
                    label: label_for(key.as_deref(), value, opts),
                    shape: shape_for(value),
                });
                if let Some(p) = parent {
                    edges.push((p, id));
                }
                let depth_capped = opts.max_depth > 0 && depth >= opts.max_depth;
                match value {
                    Value::Object(map) if !map.is_empty() => {
                        if depth_capped {
                            queue.push_back(Item::Ell {
                                parent: id,
                                label: format!("… {} keys hidden", map.len()),
                            });
                        } else {
                            for (k, v) in map {
                                queue.push_back(Item::Val {
                                    parent: Some(id),
                                    key: Some(k.clone()),
                                    value: v,
                                    depth: depth + 1,
                                });
                            }
                        }
                    }
                    Value::Array(arr) if !arr.is_empty() => {
                        if depth_capped {
                            queue.push_back(Item::Ell {
                                parent: id,
                                label: format!("… {} items hidden", arr.len()),
                            });
                        } else {
                            let shown = if opts.max_array_items == 0 {
                                arr.len()
                            } else {
                                opts.max_array_items.min(arr.len())
                            };
                            for (i, v) in arr.iter().enumerate().take(shown) {
                                queue.push_back(Item::Val {
                                    parent: Some(id),
                                    key: Some(format!("[{i}]")),
                                    value: v,
                                    depth: depth + 1,
                                });
                            }
                            if shown < arr.len() {
                                queue.push_back(Item::Ell {
                                    parent: id,
                                    label: format!("… {} more items", arr.len() - shown),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    let truncated = !queue.is_empty();

    Ok(match opts.format {
        Format::Mermaid => render_mermaid(&nodes, &edges, opts, truncated),
        Format::Dot => render_dot(&nodes, &edges, opts, truncated),
    })
}

fn shape_for(v: &Value) -> Shape {
    match v {
        Value::Object(_) => Shape::Object,
        Value::Array(_) => Shape::Array,
        _ => Shape::Scalar,
    }
}

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

fn truncate(s: &str, max: usize) -> String {
    if max == 0 || s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max).collect();
    format!("{head}…")
}

fn scalar_repr(v: &Value, max: usize) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{}\"", truncate(s, max)),
        _ => type_name(v).to_string(),
    }
}

fn label_for(key: Option<&str>, v: &Value, o: &Options) -> String {
    let base = match key {
        Some(k) => truncate(k, o.value_max_len),
        None => "root".to_string(),
    };
    match v {
        Value::Object(m) => {
            if o.show_types {
                format!("{base} {{{}}}", m.len())
            } else {
                base
            }
        }
        Value::Array(a) => {
            if o.show_types {
                format!("{base} [{}]", a.len())
            } else {
                base
            }
        }
        scalar => {
            if o.include_values {
                format!("{base}: {}", scalar_repr(scalar, o.value_max_len))
            } else if o.show_types {
                format!("{base}: {}", type_name(scalar))
            } else {
                base
            }
        }
    }
}

/// Mermaid renders labels as HTML, so anything that could be markup — and `#`
/// itself, which starts a Mermaid entity code — is written as a numeric entity.
fn escape_mermaid(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '#' => out.push_str("#35;"),
            '"' => out.push_str("#34;"),
            '&' => out.push_str("#38;"),
            '<' => out.push_str("#60;"),
            '>' => out.push_str("#62;"),
            '\n' | '\r' | '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// DOT labels are C-style quoted strings: backslash and quote need escaping and
/// a literal newline becomes the `\n` line break Graphviz understands.
fn escape_dot(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' | '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

fn render_mermaid(nodes: &[Node], edges: &[(usize, usize)], o: &Options, truncated: bool) -> String {
    let mut s = format!("flowchart {}\n", o.direction.mermaid());
    for (i, n) in nodes.iter().enumerate() {
        let l = escape_mermaid(&n.label);
        let body = match n.shape {
            Shape::Object => format!("[\"{l}\"]"),
            Shape::Array => format!("[[\"{l}\"]]"),
            Shape::Scalar => format!("(\"{l}\")"),
            Shape::Ellipsis => format!("([\"{l}\"])"),
        };
        s.push_str(&format!("    n{i}{body}\n"));
    }
    for (a, b) in edges {
        s.push_str(&format!("    n{a} --> n{b}\n"));
    }
    if truncated {
        s.push_str(&format!(
            "    %% truncated at the {}-node limit — raise max_nodes to show more\n",
            o.max_nodes
        ));
    }
    s
}

fn render_dot(nodes: &[Node], edges: &[(usize, usize)], o: &Options, truncated: bool) -> String {
    let mut s = String::from("digraph json {\n");
    s.push_str(&format!("  rankdir=\"{}\";\n", o.direction.rankdir()));
    s.push_str("  node [fontname=\"Helvetica\", fontsize=10];\n");
    for (i, n) in nodes.iter().enumerate() {
        let l = escape_dot(&n.label);
        let shape = match n.shape {
            Shape::Object => "box",
            Shape::Array => "box3d",
            Shape::Scalar => "ellipse",
            Shape::Ellipsis => "note",
        };
        s.push_str(&format!("  n{i} [label=\"{l}\", shape={shape}];\n"));
    }
    for (a, b) in edges {
        s.push_str(&format!("  n{a} -> n{b};\n"));
    }
    if truncated {
        s.push_str(&format!(
            "  // truncated at the {}-node limit — raise max_nodes to show more\n",
            o.max_nodes
        ));
    }
    s.push_str("}\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mermaid_happy_path() {
        let out = to_graph(r#"{"name":"Ada","tags":["x"]}"#, &Options::default()).unwrap();
        assert!(out.starts_with("flowchart TD\n"));
        assert!(out.contains("n0[\"root\"]"));
        assert!(out.contains("n1(\"name: #34;Ada#34;\")"));
        assert!(out.contains("n2[[\"tags\"]]"));
        assert!(out.contains("n3(\"[0]: #34;x#34;\")"));
        assert!(out.contains("n0 --> n1"));
        assert!(out.contains("n2 --> n3"));
    }

    #[test]
    fn dot_happy_path() {
        let o = Options {
            format: Format::Dot,
            direction: Direction::Lr,
            ..Options::default()
        };
        let out = to_graph(r#"{"a":{"b":1}}"#, &o).unwrap();
        assert!(out.starts_with("digraph json {\n"));
        assert!(out.contains("rankdir=\"LR\";"));
        assert!(out.contains("n0 [label=\"root\", shape=box];"));
        assert!(out.contains("n1 [label=\"a\", shape=box];"));
        assert!(out.contains("n2 [label=\"b: 1\", shape=ellipse];"));
        assert!(out.contains("n1 -> n2;"));
        assert!(out.trim_end().ends_with('}'));
    }

    #[test]
    fn rejects_invalid_json() {
        let err = to_graph("not json", &Options::default()).unwrap_err();
        assert!(err.starts_with("invalid JSON:"), "got {err}");
    }

    #[test]
    fn rejects_empty_input() {
        assert!(to_graph("   ", &Options::default()).is_err());
    }

    #[test]
    fn rejects_zero_max_nodes() {
        let o = Options {
            max_nodes: 0,
            ..Options::default()
        };
        assert_eq!(
            to_graph("{}", &o).unwrap_err(),
            "max_nodes must be at least 1"
        );
    }

    #[test]
    fn max_nodes_truncates_breadth_first() {
        let o = Options {
            max_nodes: 3,
            ..Options::default()
        };
        let out = to_graph(r#"{"a":{"x":1},"b":2,"c":3}"#, &o).unwrap();
        // The budget goes to root + its first two children, not down the "a" branch.
        assert!(out.contains("n0[\"root\"]"));
        assert!(out.contains("n1[\"a\"]"));
        assert!(out.contains("n2(\"b: 2\")"));
        assert!(!out.contains("x: 1"));
        assert!(!out.contains("c: 3"));
        assert_eq!(out.matches(" --> ").count(), 2);
    }

    #[test]
    fn max_nodes_emits_truncation_comment() {
        let o = Options {
            max_nodes: 2,
            ..Options::default()
        };
        let out = to_graph(r#"{"a":1,"b":2,"c":3}"#, &o).unwrap();
        assert!(out.contains("%% truncated at the 2-node limit"));
        assert!(out.contains("n1(\"a: 1\")"));
        assert!(!out.contains("n2"));
    }

    #[test]
    fn max_depth_collapses_deeper_levels() {
        let o = Options {
            max_depth: 2,
            ..Options::default()
        };
        let out = to_graph(r#"{"a":{"b":{"c":1,"d":2}}}"#, &o).unwrap();
        assert!(out.contains("([\"… 2 keys hidden\"])"));
        assert!(!out.contains("c: 1"));
    }

    #[test]
    fn max_array_items_caps_children() {
        let o = Options {
            max_array_items: 2,
            ..Options::default()
        };
        let out = to_graph("[10,20,30,40]", &o).unwrap();
        assert!(out.contains("[0]: 10"));
        assert!(out.contains("[1]: 20"));
        assert!(!out.contains("[2]: 30"));
        assert!(out.contains("… 2 more items"));
    }

    #[test]
    fn show_types_annotates_containers_and_scalars() {
        let o = Options {
            show_types: true,
            include_values: false,
            ..Options::default()
        };
        let out = to_graph(r#"{"a":[1,2],"b":"hi","c":null}"#, &o).unwrap();
        assert!(out.contains("root {3}"));
        assert!(out.contains("a [2]"));
        assert!(out.contains("b: string"));
        assert!(out.contains("c: null"));
    }

    #[test]
    fn include_values_off_keeps_keys_only() {
        let o = Options {
            include_values: false,
            ..Options::default()
        };
        let out = to_graph(r#"{"b":"hi"}"#, &o).unwrap();
        assert!(out.contains("n1(\"b\")"));
    }

    #[test]
    fn long_values_and_keys_are_truncated() {
        let o = Options {
            value_max_len: 5,
            ..Options::default()
        };
        let out = to_graph(r#"{"abcdefghij":"0123456789"}"#, &o).unwrap();
        assert!(out.contains("abcde…: #34;01234…#34;"), "got {out}");
    }

    #[test]
    fn value_max_len_zero_disables_truncation() {
        let o = Options {
            value_max_len: 0,
            ..Options::default()
        };
        let out = to_graph(r#"{"k":"0123456789"}"#, &o).unwrap();
        assert!(out.contains("k: #34;0123456789#34;"));
    }

    #[test]
    fn mermaid_escapes_markup_and_hashes() {
        let out = to_graph(r#"{"c":"<b> & #1"}"#, &Options::default()).unwrap();
        assert!(out.contains("c: #34;#60;b#62; #38; #35;1#34;"), "got {out}");
    }

    #[test]
    fn dot_escapes_quotes_backslashes_and_newlines() {
        let o = Options {
            format: Format::Dot,
            ..Options::default()
        };
        let out = to_graph(r#"{"p":"C:\\a \"q\"\nz"}"#, &o).unwrap();
        assert!(out.contains(r#"label="p: \"C:\\a \"q\"\nz\""#), "got {out}");
    }

    #[test]
    fn scalar_root_and_empty_containers() {
        let out = to_graph("42", &Options::default()).unwrap();
        assert!(out.contains("n0(\"root: 42\")"));
        let out = to_graph(r#"{"a":{},"b":[]}"#, &Options::default()).unwrap();
        assert!(out.contains("n1[\"a\"]"));
        assert!(out.contains("n2[[\"b\"]]"));
        assert_eq!(out.matches(" --> ").count(), 2);
    }

    #[test]
    fn directions_map_to_each_syntax() {
        for (d, m, r) in [
            (Direction::Td, "TD", "TB"),
            (Direction::Lr, "LR", "LR"),
            (Direction::Bt, "BT", "BT"),
            (Direction::Rl, "RL", "RL"),
        ] {
            let mermaid = to_graph(
                "{}",
                &Options {
                    direction: d,
                    ..Options::default()
                },
            )
            .unwrap();
            assert!(mermaid.starts_with(&format!("flowchart {m}\n")));
            let dot = to_graph(
                "{}",
                &Options {
                    direction: d,
                    format: Format::Dot,
                    ..Options::default()
                },
            )
            .unwrap();
            assert!(dot.contains(&format!("rankdir=\"{r}\";")));
        }
    }

    #[test]
    fn parsers_are_forgiving_and_default_sensibly() {
        assert_eq!(Format::parse("DOT"), Format::Dot);
        assert_eq!(Format::parse("graphviz"), Format::Dot);
        assert_eq!(Format::parse("anything else"), Format::Mermaid);
        assert_eq!(Direction::parse("lr"), Direction::Lr);
        assert_eq!(Direction::parse(""), Direction::Td);
    }
}
