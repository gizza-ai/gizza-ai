//! mermaid-from-data core — pure compute, shared by the chat skill block and
//! the web page. Turns a structured list of nodes and edges (delimited rows or
//! `A -> B` arrow lines) into Mermaid diagram source: a `flowchart`, a
//! `classDiagram` or an `erDiagram`. No I/O, no rendering — text in, Mermaid
//! source out.

use std::collections::HashMap;

/// Largest accepted size for either the node list or the edge list.
pub const MAX_BYTES: usize = 2_000_000;
/// Largest number of distinct nodes (declared + discovered from edges).
pub const MAX_NODES: usize = 500;
/// Largest number of edges.
pub const MAX_EDGES: usize = 1000;

/// Arrow tokens accepted in `A -> B` rows, longest first so `-.->` wins over `->`.
const ARROWS: [(&str, &str); 8] = [
    ("-.->", "dotted"),
    ("<-->", "bidirectional"),
    ("==>", "thick"),
    ("-->", ""),
    ("=>", "thick"),
    ("->", ""),
    ("→", ""),
    ("⇒", "thick"),
];

/// A node as declared in the node list (or discovered on an edge).
struct Node {
    /// Mermaid-safe identifier derived from the id the user wrote.
    id: String,
    /// Display label (flowchart) — defaults to `raw`.
    label: String,
    /// Per-node shape (flowchart), annotation (class) or unused (er).
    detail: String,
    /// Semicolon-separated members (class) / attributes (er).
    body: String,
    /// Subgraph group (flowchart only).
    group: String,
    /// True when the node came from the node list rather than from an edge.
    declared: bool,
}

/// One edge between two nodes.
struct Edge {
    from: usize,
    to: usize,
    label: String,
    style: String,
}

/// Build Mermaid diagram source from a node list and an edge list.
///
/// * `edges` — one edge per line: `from<delim>to<delim>label<delim>style`, or
///   `from -> to : label`. Blank lines and `#` / `//` comments are skipped.
/// * `nodes` — optional, one node per line: `id<delim>label<delim>shape<delim>group`.
/// * `diagram` — `flowchart` | `class` | `er` (default `flowchart`).
/// * `direction` — `TD` | `LR` | `BT` | `RL` (default `TD`).
/// * `row_mode` — `pair` (default) or `chain` (every column of a row is a node,
///   consecutive columns are linked).
/// * `delimiter` — `auto` (default) | `pipe` | `comma` | `tab`.
/// * `node_shape` — default flowchart shape.
/// * `edge_style` — default flowchart edge style.
/// * `title` — optional diagram title (YAML front matter).
/// * `fence` — wrap the result in a ```` ```mermaid ```` code fence.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    edges: &str,
    nodes: &str,
    diagram: &str,
    direction: &str,
    row_mode: &str,
    delimiter: &str,
    node_shape: &str,
    edge_style: &str,
    title: &str,
    fence: bool,
) -> Result<String, String> {
    if edges.len() > MAX_BYTES {
        return Err(format!(
            "edges: input is {} bytes, the maximum is {MAX_BYTES} bytes",
            edges.len()
        ));
    }
    if nodes.len() > MAX_BYTES {
        return Err(format!(
            "nodes: input is {} bytes, the maximum is {MAX_BYTES} bytes",
            nodes.len()
        ));
    }

    let diagram = pick(
        diagram,
        "flowchart",
        &["flowchart", "class", "er"],
        "diagram",
    )?;
    let direction = pick(
        direction,
        "TD",
        &["TD", "TB", "LR", "BT", "RL"],
        "direction",
    )?;
    let row_mode = pick(row_mode, "pair", &["pair", "chain"], "row_mode")?;
    let delim_opt = pick(
        delimiter,
        "auto",
        &["auto", "pipe", "comma", "tab"],
        "delimiter",
    )?;
    let default_shape = pick(node_shape, "rect", SHAPES, "node_shape")?;
    let default_style = pick(edge_style, "arrow", FLOW_STYLES, "edge_style")?;

    let mut g = Graph::default();
    parse_nodes(&mut g, nodes, delim(&delim_opt, nodes), &diagram)?;
    parse_edges(
        &mut g,
        edges,
        delim(&delim_opt, edges),
        &row_mode,
        &diagram,
        &default_style,
    )?;

    if g.nodes.is_empty() {
        return Err(
            "edges: no nodes or edges found — write one edge per line, e.g. \"Start -> Finish\" or \"Start, Finish, ships\""
                .to_string(),
        );
    }

    let body = match diagram.as_str() {
        "class" => render_class(&g, &direction),
        "er" => render_er(&g, &direction),
        _ => render_flowchart(&g, &direction, &default_shape),
    };

    let mut out = String::new();
    let title = title.replace(['\r', '\n'], " ").trim().to_string();
    if !title.is_empty() {
        out.push_str("---\ntitle: ");
        out.push_str(&title);
        out.push_str("\n---\n");
    }
    out.push_str(&body);
    if fence {
        return Ok(format!("```mermaid\n{out}\n```"));
    }
    Ok(out)
}

/// Flowchart node shapes, in the order they appear on the page's dropdown.
const SHAPES: &[&str] = &[
    "rect",
    "round",
    "stadium",
    "subroutine",
    "cylinder",
    "circle",
    "diamond",
    "hexagon",
    "parallelogram",
    "trapezoid",
];

/// Flowchart edge styles.
const FLOW_STYLES: &[&str] = &["arrow", "open", "dotted", "thick", "bidirectional"];

/// Class-diagram relationship names (aliases are folded in `class_op`).
const CLASS_RELATIONS: &[&str] = &[
    "association",
    "inheritance",
    "composition",
    "aggregation",
    "dependency",
    "realization",
    "link",
];

#[derive(Default)]
struct Graph {
    nodes: Vec<Node>,
    index: HashMap<String, usize>,
    used_ids: HashMap<String, usize>,
    edges: Vec<Edge>,
    groups: Vec<String>,
}

impl Graph {
    /// Look up (or create) a node by its raw id, returning its index.
    fn intern(&mut self, raw: &str) -> Result<usize, String> {
        if let Some(i) = self.index.get(raw) {
            return Ok(*i);
        }
        if self.nodes.len() >= MAX_NODES {
            return Err(format!(
                "too many nodes: the maximum is {MAX_NODES} (a larger diagram is unreadable in any renderer)"
            ));
        }
        let id = self.unique_id(raw);
        let i = self.nodes.len();
        self.nodes.push(Node {
            id,
            label: raw.to_string(),
            detail: String::new(),
            body: String::new(),
            group: String::new(),
            declared: false,
        });
        self.index.insert(raw.to_string(), i);
        Ok(i)
    }

    /// Sanitize `raw` into a Mermaid identifier, de-duplicating collisions.
    fn unique_id(&mut self, raw: &str) -> String {
        let base = sanitize_id(raw);
        let n = self.used_ids.entry(base.clone()).or_insert(0);
        *n += 1;
        if *n == 1 {
            base
        } else {
            format!("{base}_{n}")
        }
    }
}

/// Turn arbitrary text into a Mermaid-safe identifier.
fn sanitize_id(raw: &str) -> String {
    let mut s: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    while s.len() > 1 && s.ends_with('_') {
        s.pop();
    }
    if s.is_empty() || s == "_" {
        s = "n".to_string();
    }
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert(0, 'n');
    }
    // `end` breaks the flowchart parser; `o`/`x` collide with edge endings.
    if matches!(
        s.as_str(),
        "end"
            | "graph"
            | "subgraph"
            | "class"
            | "click"
            | "style"
            | "linkStyle"
            | "direction"
            | "o"
            | "x"
    ) {
        s.push('_');
    }
    s
}

/// Escape a display label for use inside a Mermaid `"…"` string.
fn esc_label(raw: &str) -> String {
    raw.replace("\\n", "<br/>")
        .replace(['\r', '\n'], "<br/>")
        .replace('"', "#quot;")
        .replace('|', "#124;")
}

/// Resolve an enum-ish option, mapping "" to `default` and rejecting anything else.
fn pick(value: &str, default: &str, allowed: &[&str], field: &str) -> Result<String, String> {
    let v = value.trim().to_ascii_lowercase();
    if v.is_empty() {
        return Ok(default.to_string());
    }
    // `direction` is compared upper-case by its caller; allow both.
    for a in allowed {
        if a.eq_ignore_ascii_case(&v) {
            return Ok((*a).to_string());
        }
    }
    Err(format!(
        "{field}: expected one of {}, got \"{}\"",
        allowed.join(", "),
        value.trim()
    ))
}

/// Resolve the column delimiter, sniffing the text itself in `auto` mode.
fn delim(opt: &str, text: &str) -> char {
    match opt {
        "pipe" => '|',
        "comma" => ',',
        "tab" => '\t',
        _ => {
            if text.contains('\t') {
                '\t'
            } else if text.contains('|') {
                '|'
            } else {
                ','
            }
        }
    }
}

/// Split one row into trimmed fields, honouring `"quoted, fields"`.
fn split_fields(line: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' && cur.trim().is_empty() {
            cur.clear();
            quoted = true;
        } else if c == delim {
            out.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(c);
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn strip_outer_quotes(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t[1..t.len() - 1].replace("\"\"", "\"")
    } else {
        t.to_string()
    }
}

/// Iterate the meaningful rows of a block of text (1-based line numbers).
fn rows(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().filter_map(|(i, l)| {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("//") {
            None
        } else {
            Some((i + 1, t))
        }
    })
}

/// True when the first row is a spreadsheet header rather than data.
fn is_header(fields: &[String], first: &[&str], second: &[&str]) -> bool {
    if fields.len() < 2 {
        return false;
    }
    let a = fields[0].to_ascii_lowercase();
    let b = fields[1].to_ascii_lowercase();
    first.contains(&a.as_str()) && second.contains(&b.as_str())
}

fn parse_nodes(g: &mut Graph, text: &str, d: char, diagram: &str) -> Result<(), String> {
    let mut first = true;
    for (lineno, line) in rows(text) {
        let f = split_fields(line, d);
        if first {
            first = false;
            if is_header(
                &f,
                &["id", "node", "name", "key", "entity", "class"],
                &[
                    "label",
                    "title",
                    "text",
                    "name",
                    "shape",
                    "type",
                    "members",
                    "attributes",
                ],
            ) {
                continue;
            }
        }
        if f[0].is_empty() {
            return Err(format!(
                "nodes: line {lineno}: the first column must be the node id, got \"{line}\""
            ));
        }
        let i = g.intern(&f[0])?;
        let col1 = f.get(1).cloned().unwrap_or_default();
        let col2 = f.get(2).cloned().unwrap_or_default();
        let col3 = f.get(3).cloned().unwrap_or_default();
        if diagram == "flowchart" {
            if !col1.is_empty() {
                g.nodes[i].label = col1;
            }
            if !col2.is_empty() {
                let shape = pick(
                    &col2,
                    "rect",
                    SHAPES,
                    &format!("nodes: line {lineno}: shape"),
                )?;
                g.nodes[i].detail = shape;
            }
            if !col3.is_empty() {
                if !g.groups.contains(&col3) {
                    g.groups.push(col3.clone());
                }
                g.nodes[i].group = col3;
            }
        } else {
            // class: members + annotation. er: attributes.
            g.nodes[i].body = col1;
            g.nodes[i].detail = col2;
        }
        g.nodes[i].declared = true;
    }
    Ok(())
}

fn parse_edges(
    g: &mut Graph,
    text: &str,
    d: char,
    row_mode: &str,
    diagram: &str,
    default_style: &str,
) -> Result<(), String> {
    // `edge_style` and the arrow tokens both name FLOWCHART styles; class and ER
    // rows without an explicit style column fall back to "" instead, which
    // class_op/er_op resolve to association / one-to-many.
    let flow = diagram == "flowchart";
    let default_style = if flow { default_style } else { "" };
    let mut first = true;
    for (lineno, line) in rows(text) {
        // Arrow form: "A -> B", "A -.-> B -> C", optionally "… : label".
        if let Some((chain, arrow_style)) = split_arrows(line) {
            let (chain, label) = split_arrow_label(chain);
            let style = if arrow_style.is_empty() || !flow {
                default_style.to_string()
            } else {
                arrow_style
            };
            let mut prev: Option<usize> = None;
            for part in chain {
                if part.is_empty() {
                    return Err(format!(
                        "edges: line {lineno}: an arrow needs a node on both sides, got \"{line}\""
                    ));
                }
                let part = strip_outer_quotes(&part);
                let i = g.intern(&part)?;
                if let Some(p) = prev {
                    push_edge(g, p, i, label.clone(), style.clone(), diagram, lineno)?;
                }
                prev = Some(i);
            }
            first = false;
            continue;
        }

        let f = split_fields(line, d);
        if first {
            first = false;
            if is_header(
                &f,
                &["from", "source", "src", "parent", "start"],
                &["to", "target", "dst", "child", "end"],
            ) {
                continue;
            }
        }
        let cells: Vec<&String> = f.iter().filter(|c| !c.is_empty()).collect();
        if cells.is_empty() {
            continue;
        }
        if cells.len() == 1 {
            // A lone value declares an isolated node.
            g.intern(cells[0])?;
            continue;
        }
        if row_mode == "chain" {
            let mut prev: Option<usize> = None;
            for c in &cells {
                let i = g.intern(c)?;
                if let Some(p) = prev {
                    push_edge(
                        g,
                        p,
                        i,
                        String::new(),
                        default_style.to_string(),
                        diagram,
                        lineno,
                    )?;
                }
                prev = Some(i);
            }
            continue;
        }
        let from = g.intern(&f[0])?;
        let to = g.intern(&f[1])?;
        let label = f.get(2).cloned().unwrap_or_default();
        let style = match f.get(3) {
            Some(s) if !s.is_empty() => s.clone(),
            _ => default_style.to_string(),
        };
        push_edge(g, from, to, label, style, diagram, lineno)?;
    }
    Ok(())
}

/// Validate + record one edge.
fn push_edge(
    g: &mut Graph,
    from: usize,
    to: usize,
    label: String,
    style: String,
    diagram: &str,
    lineno: usize,
) -> Result<(), String> {
    if g.edges.len() >= MAX_EDGES {
        return Err(format!(
            "too many edges: the maximum is {MAX_EDGES} (a larger diagram is unreadable in any renderer)"
        ));
    }
    // Fail fast on an unknown style/relation so the error names the line.
    match diagram {
        "class" => {
            class_op(&style).ok_or_else(|| {
                format!(
                    "edges: line {lineno}: expected a class relationship ({}) or a Mermaid operator such as <|--, got \"{style}\"",
                    CLASS_RELATIONS.join(", ")
                )
            })?;
        }
        "er" => {
            er_op(&style).ok_or_else(|| {
                format!(
                    "edges: line {lineno}: expected a cardinality (one-to-many, one-to-one, many-to-many, many-to-one, one-to-one-or-more, zero-or-one-to-many) or a Mermaid token such as ||--o{{, got \"{style}\""
                )
            })?;
        }
        _ => {
            flow_op(&style).ok_or_else(|| {
                format!(
                    "edges: line {lineno}: expected an edge style ({}) or a Mermaid operator such as -->, got \"{style}\"",
                    FLOW_STYLES.join(", ")
                )
            })?;
        }
    }
    g.edges.push(Edge {
        from,
        to,
        label,
        style,
    });
    Ok(())
}

/// Split a row on arrow tokens. Returns the node texts plus the style implied
/// by the arrow that was used (`""` = use the default style).
fn split_arrows(line: &str) -> Option<(Vec<String>, String)> {
    let mut parts: Vec<String> = Vec::new();
    let mut style = String::new();
    let mut rest = line;
    let mut found = false;
    loop {
        let mut hit: Option<(usize, &str, &str)> = None;
        for (tok, st) in ARROWS.iter() {
            if let Some(pos) = rest.find(tok) {
                match hit {
                    Some((p, t, _)) if p < pos || (p == pos && t.len() >= tok.len()) => {}
                    _ => hit = Some((pos, tok, st)),
                }
            }
        }
        match hit {
            Some((pos, tok, st)) => {
                found = true;
                if !st.is_empty() {
                    style = st.to_string();
                }
                parts.push(rest[..pos].trim().to_string());
                rest = &rest[pos + tok.len()..];
            }
            None => {
                parts.push(rest.trim().to_string());
                break;
            }
        }
    }
    if found {
        Some((parts, style))
    } else {
        None
    }
}

/// Peel a trailing `: label` off the last node of an arrow row.
fn split_arrow_label(mut chain: Vec<String>) -> (Vec<String>, String) {
    let mut label = String::new();
    if let Some(last) = chain.last_mut() {
        if let Some(pos) = last.find(':') {
            label = last[pos + 1..].trim().to_string();
            *last = last[..pos].trim().to_string();
        }
    }
    (chain, label)
}

/// Accept a raw Mermaid operator written straight into the style column.
fn raw_op(raw: &str, allowed: &str) -> Option<String> {
    if !raw.is_empty() && raw.chars().all(|c| allowed.contains(c)) {
        Some(raw.to_string())
    } else {
        None
    }
}

/// Map a flowchart edge style (name or raw Mermaid operator) to its operator.
fn flow_op(style: &str) -> Option<String> {
    match style.trim().to_ascii_lowercase().as_str() {
        "" | "arrow" => Some("-->".into()),
        "open" | "line" => Some("---".into()),
        "dotted" | "dashed" => Some("-.->".into()),
        "thick" | "bold" => Some("==>".into()),
        "bidirectional" | "both" => Some("<-->".into()),
        raw => raw_op(raw, "-.=<>~o"),
    }
}

/// Map a class relationship name (or raw operator) to its Mermaid operator.
fn class_op(rel: &str) -> Option<String> {
    match rel.trim().to_ascii_lowercase().as_str() {
        "" | "association" | "associates" => Some("-->".into()),
        "inheritance" | "extends" | "inherits" | "is-a" => Some("<|--".into()),
        "composition" | "owns" | "composes" => Some("*--".into()),
        "aggregation" | "has" | "aggregates" => Some("o--".into()),
        "dependency" | "uses" | "depends" => Some("..>".into()),
        "realization" | "implements" => Some("..|>".into()),
        "link" | "solid" => Some("--".into()),
        raw => raw_op(raw, "-.<>|*o"),
    }
}

/// Map an ER cardinality name (or raw token) to its Mermaid relationship token.
fn er_op(card: &str) -> Option<String> {
    match card.trim().to_ascii_lowercase().as_str() {
        "" | "one-to-many" | "1-n" | "1:n" => Some("||--o{".into()),
        "one-to-one" | "1-1" | "1:1" => Some("||--||".into()),
        "many-to-many" | "n-n" | "n:m" => Some("}o--o{".into()),
        "many-to-one" | "n-1" | "n:1" => Some("}o--||".into()),
        "one-to-one-or-more" | "1-1n" => Some("||--|{".into()),
        "zero-or-one-to-many" | "01-n" => Some("|o--o{".into()),
        raw => raw_op(raw, "|o{}.-"),
    }
}

/// Wrap a label in the shape the node asked for.
fn shape_wrap(shape: &str, label: &str) -> String {
    match shape {
        "round" => format!("(\"{label}\")"),
        "stadium" => format!("([\"{label}\"])"),
        "subroutine" => format!("[[\"{label}\"]]"),
        "cylinder" => format!("[(\"{label}\")]"),
        "circle" => format!("((\"{label}\"))"),
        "diamond" => format!("{{\"{label}\"}}"),
        "hexagon" => format!("{{{{\"{label}\"}}}}"),
        "parallelogram" => format!("[/\"{label}\"/]"),
        "trapezoid" => format!("[/\"{label}\"\\]"),
        _ => format!("[\"{label}\"]"),
    }
}

fn render_flowchart(g: &Graph, direction: &str, default_shape: &str) -> String {
    let dir = if direction == "TB" { "TD" } else { direction };
    let mut out = format!("flowchart {dir}\n");
    let decl = |n: &Node| -> String {
        let shape = if n.detail.is_empty() {
            default_shape
        } else {
            n.detail.as_str()
        };
        format!("{}{}", n.id, shape_wrap(shape, &esc_label(&n.label)))
    };
    for n in g.nodes.iter().filter(|n| n.group.is_empty()) {
        out.push_str(&format!("    {}\n", decl(n)));
    }
    for group in &g.groups {
        out.push_str(&format!(
            "    subgraph {}[\"{}\"]\n",
            sanitize_id(&format!("group_{group}")),
            esc_label(group)
        ));
        for n in g.nodes.iter().filter(|n| &n.group == group) {
            out.push_str(&format!("        {}\n", decl(n)));
        }
        out.push_str("    end\n");
    }
    for e in &g.edges {
        let op = flow_op(&e.style).unwrap_or_else(|| "-->".into());
        let from = &g.nodes[e.from].id;
        let to = &g.nodes[e.to].id;
        if e.label.is_empty() {
            out.push_str(&format!("    {from} {op} {to}\n"));
        } else {
            out.push_str(&format!(
                "    {from} {op}|\"{}\"| {to}\n",
                esc_label(&e.label)
            ));
        }
    }
    out.pop();
    out
}

/// Nodes that need their own declaration line in a class/ER diagram: the ones
/// declared in the node list, plus isolated ones no edge would introduce.
fn own_line_nodes(g: &Graph) -> impl Iterator<Item = &Node> {
    g.nodes.iter().enumerate().filter_map(|(i, n)| {
        let isolated = !g.edges.iter().any(|e| e.from == i || e.to == i);
        (n.declared || isolated).then_some(n)
    })
}

fn render_class(g: &Graph, direction: &str) -> String {
    let dir = if direction == "TD" { "TB" } else { direction };
    let mut out = format!("classDiagram\n    direction {dir}\n");
    for n in own_line_nodes(g) {
        let members: Vec<String> = n
            .body
            .split(';')
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .collect();
        let annotation = n.detail.trim();
        if members.is_empty() && annotation.is_empty() {
            out.push_str(&format!("    class {}\n", n.id));
            continue;
        }
        out.push_str(&format!("    class {} {{\n", n.id));
        if !annotation.is_empty() {
            let a = annotation.trim_start_matches('<').trim_end_matches('>');
            out.push_str(&format!("        <<{a}>>\n"));
        }
        for m in members {
            out.push_str(&format!("        {m}\n"));
        }
        out.push_str("    }\n");
    }
    for e in &g.edges {
        let op = class_op(&e.style).unwrap_or_else(|| "-->".into());
        let from = &g.nodes[e.from].id;
        let to = &g.nodes[e.to].id;
        if e.label.is_empty() {
            out.push_str(&format!("    {from} {op} {to}\n"));
        } else {
            out.push_str(&format!("    {from} {op} {to} : {}\n", one_line(&e.label)));
        }
    }
    out.pop();
    out
}

fn render_er(g: &Graph, direction: &str) -> String {
    let dir = if direction == "TD" { "TB" } else { direction };
    let mut out = format!("erDiagram\n    direction {dir}\n");
    for n in own_line_nodes(g) {
        let attrs: Vec<String> = n
            .body
            .split(';')
            .map(er_attribute)
            .filter(|a| !a.is_empty())
            .collect();
        if attrs.is_empty() {
            out.push_str(&format!("    {}\n", n.id));
            continue;
        }
        out.push_str(&format!("    {} {{\n", n.id));
        for a in attrs {
            out.push_str(&format!("        {a}\n"));
        }
        out.push_str("    }\n");
    }
    for e in &g.edges {
        let op = er_op(&e.style).unwrap_or_else(|| "||--o{".into());
        let from = &g.nodes[e.from].id;
        let to = &g.nodes[e.to].id;
        out.push_str(&format!(
            "    {from} {op} {to} : \"{}\"\n",
            esc_label(&one_line(&e.label))
        ));
    }
    out.pop();
    out
}

/// Normalize one `type name PK` attribute into Mermaid's attribute grammar.
fn er_attribute(entry: &str) -> String {
    let tokens: Vec<&str> = entry.split_whitespace().collect();
    if tokens.is_empty() {
        return String::new();
    }
    let safe = |t: &str| -> String {
        t.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || "_()[]?".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    if tokens.len() == 1 {
        return format!("string {}", safe(tokens[0]));
    }
    let mut line = format!("{} {}", safe(tokens[0]), safe(tokens[1]));
    for t in &tokens[2..] {
        let up = t.to_ascii_uppercase();
        if matches!(up.as_str(), "PK" | "FK" | "UK") {
            line.push(' ');
            line.push_str(&up);
        }
    }
    line
}

/// Collapse a label to a single line (used where Mermaid forbids a break).
fn one_line(s: &str) -> String {
    s.replace(['\r', '\n'], " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(edges: &str, nodes: &str) -> String {
        generate(edges, nodes, "", "", "", "", "", "", "", false).unwrap()
    }

    #[test]
    fn arrow_rows_make_a_flowchart() {
        assert_eq!(
            flow("Start -> Build\nBuild -> Ship : deploy", ""),
            "flowchart TD\n    Start[\"Start\"]\n    Build[\"Build\"]\n    Ship[\"Ship\"]\n    Start --> Build\n    Build -->|\"deploy\"| Ship"
        );
    }

    #[test]
    fn csv_rows_carry_label_and_style() {
        assert_eq!(
            flow("from,to,label,style\nweb,api,calls,arrow\napi,db,reads,dotted", ""),
            "flowchart TD\n    web[\"web\"]\n    api[\"api\"]\n    db[\"db\"]\n    web -->|\"calls\"| api\n    api -.->|\"reads\"| db"
        );
    }

    #[test]
    fn node_list_sets_labels_shapes_and_groups() {
        let out = flow(
            "api,db",
            "api | API service | round | Backend\ndb | Postgres | cylinder | Backend",
        );
        assert_eq!(
            out,
            "flowchart TD\n    subgraph group_Backend[\"Backend\"]\n        api(\"API service\")\n        db[(\"Postgres\")]\n    end\n    api --> db"
        );
    }

    #[test]
    fn chain_mode_links_every_column() {
        let out = generate("a,b,c", "", "", "LR", "chain", "", "", "", "", false).unwrap();
        assert_eq!(
            out,
            "flowchart LR\n    a[\"a\"]\n    b[\"b\"]\n    c[\"c\"]\n    a --> b\n    b --> c"
        );
    }

    #[test]
    fn ids_are_sanitized_and_deduplicated() {
        let out = flow("\"Order placed\" -> 2nd step\nOrder-placed -> 2nd step", "");
        assert!(out.contains("Order_placed[\"Order placed\"]"), "{out}");
        assert!(out.contains("n2nd_step[\"2nd step\"]"), "{out}");
        assert!(out.contains("Order_placed_2[\"Order-placed\"]"), "{out}");
    }

    #[test]
    fn quoted_fields_keep_their_commas() {
        let out = flow("\"Step one, then\",Done", "");
        assert!(out.contains("[\"Step one, then\"]"), "{out}");
    }

    #[test]
    fn class_diagram_renders_members_and_relations() {
        let out = generate(
            "Animal,Dog,,inheritance\nDog,Collar,wears,composition",
            "Animal | +String name; +eat()\nDog",
            "class",
            "LR",
            "",
            "",
            "",
            "",
            "",
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "classDiagram\n    direction LR\n    class Animal {\n        +String name\n        +eat()\n    }\n    class Dog\n    Animal <|-- Dog\n    Dog *-- Collar : wears"
        );
    }

    #[test]
    fn er_diagram_renders_attributes_and_cardinality() {
        let out = generate(
            "CUSTOMER,ORDER,places,one-to-many\nORDER,LINE_ITEM,contains,one-to-one-or-more",
            "CUSTOMER | string name PK; string email",
            "er",
            "",
            "",
            "",
            "",
            "",
            "",
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "erDiagram\n    direction TB\n    CUSTOMER {\n        string name PK\n        string email\n    }\n    CUSTOMER ||--o{ ORDER : \"places\"\n    ORDER ||--|{ LINE_ITEM : \"contains\""
        );
    }

    #[test]
    fn isolated_nodes_survive_class_and_er_diagrams() {
        let out = generate(
            "Animal,Dog,,inheritance\nOrphan",
            "",
            "class",
            "",
            "",
            "",
            "",
            "",
            "",
            false,
        )
        .unwrap();
        assert!(out.contains("class Orphan"), "{out}");
        let out = generate("A,B,has\nLONER", "", "er", "", "", "", "", "", "", false).unwrap();
        assert!(out.lines().any(|l| l.trim() == "LONER"), "{out}");
    }

    #[test]
    fn title_and_fence_wrap_the_diagram() {
        let out = generate("a -> b", "", "", "", "", "", "", "", "My flow", true).unwrap();
        assert_eq!(
            out,
            "```mermaid\n---\ntitle: My flow\n---\nflowchart TD\n    a[\"a\"]\n    b[\"b\"]\n    a --> b\n```"
        );
    }

    #[test]
    fn raw_mermaid_operators_pass_through() {
        let out = flow("a|b|next|-.-", "");
        assert!(out.contains("a -.-|\"next\"| b"), "{out}");
    }

    #[test]
    fn comments_blank_lines_and_lone_nodes_are_handled() {
        let out = flow("# comment\n\nOrphan\na -> b", "");
        assert!(out.contains("Orphan[\"Orphan\"]"), "{out}");
        assert_eq!(out.lines().filter(|l| l.contains("-->")).count(), 1);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = generate("   ", "", "", "", "", "", "", "", "", false).unwrap_err();
        assert!(err.contains("no nodes or edges found"), "{err}");
    }

    #[test]
    fn unknown_style_names_the_line() {
        let err = generate("a,b,,zigzag", "", "", "", "", "", "", "", "", false).unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("zigzag"), "{err}");
    }

    #[test]
    fn unknown_diagram_is_rejected() {
        let err = generate("a -> b", "", "gantt", "", "", "", "", "", "", false).unwrap_err();
        assert!(err.contains("diagram: expected one of"), "{err}");
    }

    #[test]
    fn edge_cap_is_enforced_at_the_boundary() {
        let wide: String = (0..MAX_EDGES)
            .map(|i| format!("a -> b{}\n", i % 400))
            .collect();
        assert!(generate(&wide, "", "", "", "", "", "", "", "", false).is_ok());
        let over: String = (0..=MAX_EDGES)
            .map(|i| format!("a -> b{}\n", i % 400))
            .collect();
        let err = generate(&over, "", "", "", "", "", "", "", "", false).unwrap_err();
        assert!(err.contains("too many edges"), "{err}");
    }

    #[test]
    fn node_cap_is_enforced() {
        let over: String = (0..=MAX_NODES).map(|i| format!("a{i} -> b{i}\n")).collect();
        let err = generate(&over, "", "", "", "", "", "", "", "", false).unwrap_err();
        assert!(err.contains("too many nodes"), "{err}");
    }

    #[test]
    fn every_shape_and_style_renders() {
        for s in SHAPES {
            let out = generate(
                "a,b",
                &format!("a,A,{s}"),
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                false,
            )
            .unwrap();
            assert!(out.contains("\"A\""), "{s}: {out}");
        }
        for s in FLOW_STYLES {
            let out = generate("a,b", "", "", "", "", "", "", s, "", false).unwrap();
            assert!(out.lines().last().unwrap().contains(" b"), "{s}: {out}");
        }
    }
}
