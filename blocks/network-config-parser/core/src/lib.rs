//! gizza-ai/network-config-parser core — pure, no wafer/wasm-bindgen deps.
//!
//! Parses a hierarchical network device configuration into a tree of sections.
//! Two syntaxes are handled:
//!
//! - **indent** — Cisco IOS / IOS-XE / NX-OS and Arista EOS style, where nesting
//!   is expressed by leading whitespace and `!` lines are section separators.
//! - **brace** — Juniper Junos style, where `{ … }` delimits nested blocks and
//!   statements end with `;`.
//!
//! The result renders as a nested JSON tree, a flat list of full command paths,
//! or a stats report. An optional case-insensitive `filter` prunes the tree to
//! matching sections/lines while preserving their surrounding context.
//!
//! The chat schema is single-sourced from the block's `descriptor()`; this crate
//! is the pure engine shared by the chat block, the CLI, and the web page.

use serde_json::{json, Map, Value};

/// Hard cap on input lines — a guard against a pathological paste. A real device
/// configuration is far under this.
pub const MAX_LINES: usize = 200_000;

/// Columns a tab advances the indent counter (indent syntax only).
const TAB_WIDTH: usize = 8;

/// Which config syntax to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    /// Detect brace vs. indent from the text.
    Auto,
    /// Cisco IOS / NX-OS / Arista EOS — nesting by leading whitespace.
    Indent,
    /// Juniper Junos — nesting by `{ … }`, statements end with `;`.
    Brace,
}

impl Syntax {
    pub fn parse(s: &str) -> Result<Syntax, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Syntax::Auto,
            "indent" | "cisco" | "ios" | "arista" | "eos" | "nxos" => Syntax::Indent,
            "brace" | "juniper" | "junos" | "braces" => Syntax::Brace,
            other => {
                return Err(format!(
                    "unknown syntax '{other}' (use auto, indent, or brace)"
                ))
            }
        })
    }
}

/// Output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Nested JSON tree: an array of `{ line, children }` nodes.
    Tree,
    /// Flat list of full command paths (one per leaf statement).
    Paths,
    /// `{ syntax, sections, stats }` — a summary of the parsed config.
    Report,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "tree" | "json" => Output::Tree,
            "paths" | "flat" | "set" => Output::Paths,
            "report" | "stats" => Output::Report,
            other => {
                return Err(format!("unknown output '{other}' (use tree, paths, or report)"))
            }
        })
    }
}

/// Whether comment lines are dropped or kept as tree nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comments {
    Strip,
    Keep,
}

impl Comments {
    pub fn parse(s: &str) -> Result<Comments, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "strip" | "drop" | "remove" => Comments::Strip,
            "keep" | "include" => Comments::Keep,
            other => {
                return Err(format!("unknown comments '{other}' (use strip or keep)"))
            }
        })
    }
}

/// A parsed configuration node: a line plus the block nested under it.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub line: String,
    pub children: Vec<Node>,
}

impl Node {
    fn leaf(line: String) -> Node {
        Node {
            line,
            children: Vec::new(),
        }
    }
    fn to_value(&self) -> Value {
        let children: Vec<Value> = self.children.iter().map(Node::to_value).collect();
        json!({ "line": self.line, "children": children })
    }
}

/// Parse a network device `config` and render it per the given options.
///
/// - `syntax`: `auto` (default) | `indent` (Cisco/NX-OS/Arista) | `brace` (Juniper).
/// - `output`: `tree` (nested JSON) | `paths` (flat command paths) | `report`.
/// - `filter`: case-insensitive substring; when non-empty, keeps only matching
///   sections/lines and their ancestor context.
/// - `comments`: `strip` (default) drops comment/separator lines; `keep` keeps
///   them as nodes.
pub fn parse(
    config: &str,
    syntax: &str,
    output: &str,
    filter: &str,
    comments: &str,
) -> Result<String, String> {
    let syntax = Syntax::parse(syntax)?;
    let output = Output::parse(output)?;
    let comments = Comments::parse(comments)?;

    if config.trim().is_empty() {
        return Err("empty configuration — paste a device config to parse".into());
    }
    if config.lines().count() > MAX_LINES {
        return Err(format!("input exceeds the {MAX_LINES}-line limit"));
    }

    let effective = match syntax {
        Syntax::Auto => detect_syntax(config),
        other => other,
    };

    let (roots, comment_count) = match effective {
        Syntax::Brace => parse_brace(config, comments)?,
        // Auto resolves to Indent or Brace above; Indent is the fallthrough.
        _ => parse_indent(config, comments),
    };

    let needle = filter.trim().to_ascii_lowercase();
    let roots = if needle.is_empty() {
        roots
    } else {
        roots
            .into_iter()
            .filter_map(|n| filter_node(&n, &needle, false))
            .collect()
    };

    render(&roots, effective, output, comment_count)
}

/// Auto-detect: a line whose trimmed form ends in `{` is the Junos block opener,
/// which indentation-style configs never produce — so its presence means brace.
fn detect_syntax(config: &str) -> Syntax {
    for raw in config.lines() {
        let line = strip_line_comment_brace(raw).trim().to_string();
        if line.ends_with('{') {
            return Syntax::Brace;
        }
    }
    Syntax::Indent
}

/// True when the first non-space character marks a Cisco/indent comment or
/// separator: `!` (IOS) or `#` (used in some show/NX-OS captures).
fn is_indent_comment(line: &str) -> bool {
    matches!(line.trim_start().chars().next(), Some('!') | Some('#'))
}

/// Indentation width of `line` in columns (tabs advance to the next multiple of
/// TAB_WIDTH), counting only leading whitespace.
fn indent_width(line: &str) -> usize {
    let mut col = 0usize;
    for c in line.chars() {
        match c {
            ' ' => col += 1,
            '\t' => col += TAB_WIDTH - (col % TAB_WIDTH),
            _ => break,
        }
    }
    col
}

/// Parse Cisco/Arista/NX-OS indentation style. Returns the root nodes and the
/// number of comment/separator lines seen.
fn parse_indent(config: &str, comments: Comments) -> (Vec<Node>, usize) {
    let mut roots: Vec<Node> = Vec::new();
    // Stack of (indent_column, index-chain) locating the currently open
    // ancestors. Each chain addresses a node reachable from `roots`.
    let mut stack: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut comment_count = 0usize;

    for raw in config.lines() {
        let trimmed = raw.trim_end();
        if trimmed.trim().is_empty() {
            continue;
        }
        let is_comment = is_indent_comment(trimmed);
        // A bare `!` is a pure separator — never a node, regardless of policy.
        if trimmed.trim() == "!" {
            comment_count += 1;
            continue;
        }
        if is_comment {
            comment_count += 1;
            if comments == Comments::Strip {
                continue;
            }
        }

        let indent = indent_width(trimmed);
        let text = trimmed.trim().to_string();

        // Pop ancestors whose indent is >= this line's — they can't be parents.
        while let Some(&(ind, _)) = stack.last() {
            if ind >= indent {
                stack.pop();
            } else {
                break;
            }
        }

        let node = Node::leaf(text);
        let parent = stack.last().map(|(_, p)| p.clone());
        let path = push_node(&mut roots, parent.as_slice(), node);
        stack.push((indent, path));
    }

    (roots, comment_count)
}

/// Walk `roots` by an index chain to a mutable node reference.
fn node_at_mut<'a>(roots: &'a mut [Node], path: &[usize]) -> &'a mut Node {
    let (first, rest) = path.split_first().expect("non-empty path");
    let mut node = &mut roots[*first];
    for idx in rest {
        node = &mut node.children[*idx];
    }
    node
}

/// Strip a `#`-to-end-of-line comment from a brace-style line (Junos `#`
/// comments run to EOL). Block `/* */` comments are removed separately, before
/// line splitting.
fn strip_line_comment_brace(line: &str) -> String {
    match line.find('#') {
        Some(i) => line[..i].to_string(),
        None => line.to_string(),
    }
}

/// Remove `/* … */` block comments from the whole text, returning the cleaned
/// text and the count of comments removed.
fn remove_block_comments(text: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut count = 0usize;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            count += 1;
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            // Replace the comment with a space so it can't glue two tokens.
            out.push(' ');
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    (out, count)
}

/// Parse Juniper Junos brace style. Returns the root nodes and the comment count.
fn parse_brace(config: &str, comments: Comments) -> Result<(Vec<Node>, usize), String> {
    let (cleaned, mut comment_count) = remove_block_comments(config);

    let mut roots: Vec<Node> = Vec::new();
    // Stack of index-chains for open `{` blocks.
    let mut stack: Vec<Vec<usize>> = Vec::new();

    for (i, raw) in cleaned.lines().enumerate() {
        let line_no = i + 1;
        let no_hash = strip_line_comment_brace(raw);
        let has_hash_comment = no_hash.len() != raw.len();
        let line = no_hash.trim();
        if line.is_empty() {
            if has_hash_comment {
                comment_count += 1;
                if comments == Comments::Keep {
                    let text = raw.trim().to_string();
                    push_node(&mut roots, &stack, Node::leaf(text));
                }
            }
            continue;
        }
        // A closing brace may trail a statement, e.g. `foo; }` or bare `}`.
        let mut rest = line;
        while !rest.is_empty() {
            if let Some(stripped) = rest.strip_prefix('}') {
                if stack.pop().is_none() {
                    return Err(format!(
                        "line {line_no}: unexpected '}}' with no open block"
                    ));
                }
                rest = stripped.trim_start_matches([';', ' ', '\t']).trim();
                continue;
            }
            if let Some(open) = rest.find('{') {
                let header = rest[..open].trim().to_string();
                if header.is_empty() {
                    return Err(format!("line {line_no}: '{{' with no section name"));
                }
                let path = push_node(&mut roots, &stack, Node::leaf(header));
                stack.push(path);
                rest = rest[open + 1..].trim();
                continue;
            }
            // A leaf statement (possibly several `;`-separated on one line).
            let (stmt, tail) = match rest.find(';') {
                Some(s) => (rest[..s].trim(), rest[s + 1..].trim()),
                None => (rest, ""),
            };
            if !stmt.is_empty() {
                push_node(&mut roots, &stack, Node::leaf(stmt.to_string()));
            }
            rest = tail;
        }
    }

    if !stack.is_empty() {
        return Err(format!(
            "unbalanced braces: {} block(s) left open at end of input",
            stack.len()
        ));
    }
    Ok((roots, comment_count))
}

/// Push `node` under the block addressed by `stack.last()` (or at the root), and
/// return its index chain.
fn push_node(roots: &mut Vec<Node>, stack: &[Vec<usize>], node: Node) -> Vec<usize> {
    if let Some(parent_path) = stack.last() {
        let parent = node_at_mut(roots, parent_path);
        parent.children.push(node);
        let mut p = parent_path.clone();
        p.push(parent.children.len() - 1);
        p
    } else {
        roots.push(node);
        vec![roots.len() - 1]
    }
}

/// Prune a node to `needle`. If the node's own line matches (or an ancestor
/// already matched), the whole subtree is kept; otherwise only descendants that
/// themselves match survive (keeping the ancestor chain for context).
fn filter_node(node: &Node, needle: &str, ancestor_match: bool) -> Option<Node> {
    let self_match = node.line.to_ascii_lowercase().contains(needle);
    if ancestor_match || self_match {
        return Some(node.clone());
    }
    let kept: Vec<Node> = node
        .children
        .iter()
        .filter_map(|c| filter_node(c, needle, false))
        .collect();
    if kept.is_empty() {
        None
    } else {
        Some(Node {
            line: node.line.clone(),
            children: kept,
        })
    }
}

/// Collect one full path per leaf (a node with no children), joining ancestor
/// lines and the leaf with ` / `.
fn collect_paths(node: &Node, prefix: &[String], out: &mut Vec<String>) {
    let mut here: Vec<String> = prefix.to_vec();
    here.push(node.line.clone());
    if node.children.is_empty() {
        out.push(here.join(" / "));
    } else {
        for c in &node.children {
            collect_paths(c, &here, out);
        }
    }
}

fn max_depth(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| 1 + max_depth(&n.children))
        .max()
        .unwrap_or(0)
}

fn count_lines(nodes: &[Node]) -> usize {
    nodes.iter().map(|n| 1 + count_lines(&n.children)).sum()
}

fn count_leaves(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| {
            if n.children.is_empty() {
                1
            } else {
                count_leaves(&n.children)
            }
        })
        .sum()
}

fn render(
    roots: &[Node],
    syntax: Syntax,
    output: Output,
    comment_count: usize,
) -> Result<String, String> {
    let syntax_name = match syntax {
        Syntax::Brace => "brace",
        _ => "indent",
    };

    let out = match output {
        Output::Tree => Value::Array(roots.iter().map(Node::to_value).collect()),
        Output::Paths => {
            let mut paths = Vec::new();
            for n in roots {
                collect_paths(n, &[], &mut paths);
            }
            Value::Array(paths.into_iter().map(Value::String).collect())
        }
        Output::Report => {
            let sections: Vec<Value> = roots
                .iter()
                .filter(|n| !n.children.is_empty())
                .map(|n| Value::String(n.line.clone()))
                .collect();
            let mut stats = Map::new();
            stats.insert("top_level_sections".into(), json!(sections.len()));
            stats.insert("top_level_lines".into(), json!(roots.len()));
            stats.insert("total_lines".into(), json!(count_lines(roots)));
            stats.insert("leaf_statements".into(), json!(count_leaves(roots)));
            stats.insert("max_depth".into(), json!(max_depth(roots)));
            stats.insert("comments".into(), json!(comment_count));
            json!({
                "syntax": syntax_name,
                "sections": sections,
                "stats": Value::Object(stats),
            })
        }
    };

    serde_json::to_string_pretty(&out).map_err(|e| format!("failed to serialize JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(cfg: &str, syntax: &str, output: &str) -> Value {
        serde_json::from_str(&parse(cfg, syntax, output, "", "strip").unwrap()).unwrap()
    }

    const CISCO: &str = "\
interface GigabitEthernet0/1
 description uplink
 ip address 10.0.0.1 255.255.255.0
 no shutdown
!
router ospf 1
 network 10.0.0.0 0.0.0.255 area 0
";

    const JUNOS: &str = "\
system {
    host-name router1;
    services {
        ssh;
    }
}
interfaces {
    ge-0/0/0 {
        unit 0 {
            family inet {
                address 10.0.0.1/24;
            }
        }
    }
}
";

    #[test]
    fn indent_tree_nesting() {
        let v = run(CISCO, "auto", "tree");
        assert_eq!(v[0]["line"], "interface GigabitEthernet0/1");
        assert_eq!(v[0]["children"][0]["line"], "description uplink");
        assert_eq!(v[0]["children"][1]["line"], "ip address 10.0.0.1 255.255.255.0");
        assert_eq!(v[0]["children"][2]["line"], "no shutdown");
        // The bare `!` separator is dropped, so router ospf is a sibling root.
        assert_eq!(v[1]["line"], "router ospf 1");
        assert_eq!(v[1]["children"][0]["line"], "network 10.0.0.0 0.0.0.255 area 0");
    }

    #[test]
    fn indent_auto_detects_indent() {
        // No `{` opener → auto picks indent, reported as such.
        let v = run(CISCO, "auto", "report");
        assert_eq!(v["syntax"], "indent");
        assert_eq!(v["stats"]["top_level_sections"], json!(2));
        assert_eq!(v["stats"]["comments"], json!(1)); // the bare `!`
    }

    #[test]
    fn brace_tree_nesting_and_autodetect() {
        let v = run(JUNOS, "auto", "tree");
        assert_eq!(v[0]["line"], "system");
        assert_eq!(v[0]["children"][0]["line"], "host-name router1");
        assert_eq!(v[0]["children"][1]["line"], "services");
        assert_eq!(v[0]["children"][1]["children"][0]["line"], "ssh");
        // Deep nesting survives.
        let addr = &v[1]["children"][0]["children"][0]["children"][0]["children"][0]["line"];
        assert_eq!(addr, "address 10.0.0.1/24");
        // Auto must report brace for this input.
        let r = run(JUNOS, "auto", "report");
        assert_eq!(r["syntax"], "brace");
    }

    #[test]
    fn paths_flatten_leaves() {
        let v = run(JUNOS, "brace", "paths");
        let paths: Vec<String> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().to_string())
            .collect();
        assert!(paths.contains(&"system / host-name router1".to_string()));
        assert!(paths.contains(&"system / services / ssh".to_string()));
        assert!(paths.contains(
            &"interfaces / ge-0/0/0 / unit 0 / family inet / address 10.0.0.1/24".to_string()
        ));
    }

    #[test]
    fn filter_keeps_block_and_context() {
        // Filtering a section header keeps the whole block.
        let v = serde_json::from_str::<Value>(
            &parse(CISCO, "indent", "tree", "ospf", "strip").unwrap(),
        )
        .unwrap();
        assert_eq!(v.as_array().unwrap().len(), 1);
        assert_eq!(v[0]["line"], "router ospf 1");
        assert_eq!(v[0]["children"][0]["line"], "network 10.0.0.0 0.0.0.255 area 0");

        // Filtering a deep value keeps its ancestor chain.
        let d = serde_json::from_str::<Value>(
            &parse(JUNOS, "brace", "tree", "ssh", "strip").unwrap(),
        )
        .unwrap();
        assert_eq!(d.as_array().unwrap().len(), 1);
        assert_eq!(d[0]["line"], "system");
        assert_eq!(d[0]["children"].as_array().unwrap().len(), 1);
        assert_eq!(d[0]["children"][0]["line"], "services");
        assert_eq!(d[0]["children"][0]["children"][0]["line"], "ssh");
    }

    #[test]
    fn comments_keep_includes_nodes() {
        let cfg = "! header comment\ninterface Vlan1\n ip address 1.1.1.1 255.255.255.0\n";
        let kept = serde_json::from_str::<Value>(
            &parse(cfg, "indent", "tree", "", "keep").unwrap(),
        )
        .unwrap();
        assert_eq!(kept[0]["line"], "! header comment");
        let stripped = run(cfg, "indent", "tree");
        assert_eq!(stripped[0]["line"], "interface Vlan1");
    }

    #[test]
    fn error_empty_config() {
        let e = parse("   \n\n", "auto", "tree", "", "strip").unwrap_err();
        assert!(e.contains("empty configuration"), "got: {e}");
    }

    #[test]
    fn error_unbalanced_braces() {
        let e = parse("system {\n host-name r1;\n", "brace", "tree", "", "strip").unwrap_err();
        assert!(e.contains("unbalanced braces"), "got: {e}");
    }

    #[test]
    fn error_unexpected_closing_brace() {
        let e = parse("host-name r1;\n}\n", "brace", "tree", "", "strip").unwrap_err();
        assert!(e.contains("unexpected '}'"), "got: {e}");
    }

    #[test]
    fn error_bad_syntax_and_output() {
        assert!(parse("a", "sideways", "tree", "", "strip")
            .unwrap_err()
            .contains("unknown syntax"));
        assert!(parse("a", "auto", "grid", "", "strip")
            .unwrap_err()
            .contains("unknown output"));
        assert!(parse("a", "auto", "tree", "", "loud")
            .unwrap_err()
            .contains("unknown comments"));
    }
}
