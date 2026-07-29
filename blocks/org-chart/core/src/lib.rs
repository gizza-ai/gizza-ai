//! gizza-ai/org-chart core — render an organizational chart as an SVG image
//! from an indented outline OR JSON of people and managers. Pure-Rust
//! (hand-built SVG, no drawing deps). No wafer/wasm-bindgen deps.
//!
//! Two input shapes:
//!   * **Indented outline** — one person per line; leading whitespace (tabs or
//!     spaces) sets the reporting depth. Optional `Name | Title | Department`.
//!   * **JSON** — either nested `{"name":..,"title":..,"department":..,
//!     "children":[..]}` (object or array of roots) or a flat list
//!     `[{"name":..,"manager":..,"title":..}]` linked by manager reference.
//!
//! Layout is a tidy top-down (or left-to-right) tree with uniform node boxes,
//! so sibling subtrees never overlap. Output is an SVG string.

use serde::Deserialize;
use serde_json::Value;

/// Hard cap on people so the SVG stays bounded (also protects the 4 MiB envelope).
pub const MAX_NODES: usize = 400;

/// A person and their direct reports.
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub title: String,
    pub dept: String,
    pub children: Vec<Node>,
    // Layout (filled in during rendering): centre of the node box.
    cx: f64,
    cy: f64,
}

impl Node {
    fn new(name: String, title: String, dept: String) -> Node {
        Node { name, title, dept, children: Vec::new(), cx: 0.0, cy: 0.0 }
    }
}

fn count(nodes: &[Node]) -> usize {
    nodes.iter().map(|n| 1 + count(&n.children)).sum()
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse the `data` string into a forest of roots (an org can have >1 top person).
pub fn parse(data: &str) -> Result<Vec<Node>, String> {
    let trimmed = data.trim_start();
    let roots = if trimmed.starts_with('{') || trimmed.starts_with('[') {
        parse_json(trimmed)?
    } else {
        parse_outline(data)?
    };
    if roots.is_empty() {
        return Err("no people found — provide at least one line (or JSON node)".into());
    }
    let n = count(&roots);
    if n > MAX_NODES {
        return Err(format!("too many people ({n}); the maximum is {MAX_NODES}"));
    }
    Ok(roots)
}

/// Split a person line on `|` into name / title / department.
fn split_person(text: &str) -> (String, String, String) {
    let mut parts = text.split('|');
    let name = parts.next().unwrap_or("").trim().to_string();
    let title = parts.next().unwrap_or("").trim().to_string();
    let dept = parts.next().unwrap_or("").trim().to_string();
    (name, title, dept)
}

/// Indented-outline parser. Leading whitespace (tab = 4 columns) sets depth via
/// an indent stack, so any consistent step (2 spaces, 4 spaces, tabs) works.
fn parse_outline(data: &str) -> Result<Vec<Node>, String> {
    let mut flat: Vec<(usize, Node)> = Vec::new();
    let mut indents: Vec<usize> = Vec::new(); // ancestor indent widths
    for raw in data.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        let mut width = 0usize;
        for c in raw.chars() {
            match c {
                ' ' => width += 1,
                '\t' => width += 4,
                _ => break,
            }
        }
        while indents.last().map_or(false, |&w| w >= width) {
            indents.pop();
        }
        let level = indents.len();
        indents.push(width);
        let (name, title, dept) = split_person(raw.trim());
        if name.is_empty() {
            return Err("a person line has no name (text before the first `|`)".into());
        }
        flat.push((level, Node::new(name, title, dept)));
    }
    // Build the tree from the level sequence.
    fn build(items: &[(usize, Node)], i: &mut usize, level: usize) -> Vec<Node> {
        let mut out = Vec::new();
        while *i < items.len() && items[*i].0 == level {
            let mut node = items[*i].1.clone();
            *i += 1;
            node.children = build(items, i, level + 1);
            out.push(node);
        }
        out
    }
    let mut i = 0usize;
    Ok(build(&flat, &mut i, 0))
}

#[derive(Deserialize)]
struct JsonNode {
    #[serde(alias = "label", alias = "id")]
    name: Option<String>,
    #[serde(default, alias = "role")]
    title: Option<String>,
    #[serde(default, alias = "dept", alias = "team")]
    department: Option<String>,
    #[serde(default)]
    children: Vec<JsonNode>,
}

#[derive(Deserialize)]
struct FlatNode {
    #[serde(alias = "label", alias = "id")]
    name: String,
    #[serde(default, alias = "parent", alias = "reportsTo", alias = "boss")]
    manager: Option<String>,
    #[serde(default, alias = "role")]
    title: Option<String>,
    #[serde(default, alias = "dept", alias = "team")]
    department: Option<String>,
}

fn parse_json(data: &str) -> Result<Vec<Node>, String> {
    let value: Value = serde_json::from_str(data)
        .map_err(|e| format!("`data` looks like JSON but did not parse: {e}"))?;
    let is_flat = match &value {
        Value::Array(a) => a.iter().any(|el| {
            el.get("manager").is_some()
                || el.get("parent").is_some()
                || el.get("reportsTo").is_some()
                || el.get("boss").is_some()
        }),
        _ => false,
    };
    if is_flat {
        let flat: Vec<FlatNode> = serde_json::from_value(value)
            .map_err(|e| format!("flat people list did not parse: {e}"))?;
        build_from_flat(flat)
    } else {
        let roots: Vec<JsonNode> = match value {
            Value::Array(_) => serde_json::from_str(data)
                .map_err(|e| format!("nested people list did not parse: {e}"))?,
            _ => vec![serde_json::from_str(data)
                .map_err(|e| format!("nested people object did not parse: {e}"))?],
        };
        roots.into_iter().map(json_to_node).collect()
    }
}

fn json_to_node(j: JsonNode) -> Result<Node, String> {
    let name = j
        .name
        .filter(|s| !s.trim().is_empty())
        .ok_or("every node needs a non-empty \"name\" (or \"label\"/\"id\")")?;
    let mut node = Node::new(
        name.trim().to_string(),
        j.title.unwrap_or_default().trim().to_string(),
        j.department.unwrap_or_default().trim().to_string(),
    );
    node.children = j.children.into_iter().map(json_to_node).collect::<Result<_, _>>()?;
    Ok(node)
}

/// Build a tree from a flat manager-linked list, rejecting cycles / dangling refs.
fn build_from_flat(flat: Vec<FlatNode>) -> Result<Vec<Node>, String> {
    use std::collections::HashMap;
    let mut index: HashMap<String, usize> = HashMap::new();
    for (i, f) in flat.iter().enumerate() {
        let key = f.name.trim().to_string();
        if key.is_empty() {
            return Err("every person needs a non-empty \"name\"".into());
        }
        if index.insert(key.clone(), i).is_some() {
            return Err(format!("duplicate person name \"{key}\" — names must be unique in a flat list"));
        }
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); flat.len()];
    let mut roots: Vec<usize> = Vec::new();
    for (i, f) in flat.iter().enumerate() {
        match &f.manager {
            Some(m) if !m.trim().is_empty() => {
                let mk = m.trim();
                match index.get(mk) {
                    Some(&mi) if mi != i => children[mi].push(i),
                    Some(_) => return Err(format!("\"{}\" lists itself as its own manager", f.name.trim())),
                    None => return Err(format!(
                        "\"{}\" reports to \"{mk}\", who is not in the list",
                        f.name.trim()
                    )),
                }
            }
            _ => roots.push(i),
        }
    }
    if roots.is_empty() {
        return Err("no top person found — every person has a manager (there is a reporting cycle)".into());
    }
    // Detect cycles: every node must be reachable from a root exactly once.
    let mut seen = vec![false; flat.len()];
    fn visit(i: usize, children: &[Vec<usize>], seen: &mut [bool]) -> Result<(), String> {
        if seen[i] {
            return Err("a reporting cycle was detected in the manager links".into());
        }
        seen[i] = true;
        for &c in &children[i] {
            visit(c, children, seen)?;
        }
        Ok(())
    }
    for &r in &roots {
        visit(r, &children, &mut seen)?;
    }
    if seen.iter().any(|&s| !s) {
        return Err("a reporting cycle was detected in the manager links".into());
    }
    fn to_node(i: usize, flat: &[FlatNode], children: &[Vec<usize>]) -> Node {
        let f = &flat[i];
        let mut node = Node::new(
            f.name.trim().to_string(),
            f.title.clone().unwrap_or_default().trim().to_string(),
            f.department.clone().unwrap_or_default().trim().to_string(),
        );
        node.children = children[i].iter().map(|&c| to_node(c, flat, children)).collect();
        node
    }
    Ok(roots.into_iter().map(|r| to_node(r, &flat, &children)).collect())
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Accept a CSS colour but reject anything that could break out of the attribute.
fn sanitize_color(c: &str, fallback: &str) -> String {
    let c = c.trim();
    let ok = !c.is_empty()
        && c.len() <= 32
        && c.chars().all(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(ch, '#' | '(' | ')' | ',' | '.' | '%' | ' ' | '-')
        });
    if ok { c.to_string() } else { fallback.to_string() }
}

/// Truncate a label to at most `max` chars, adding an ellipsis if cut.
fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let mut out: String = chars[..max - 1].iter().collect();
    out.push('…');
    out
}

const NAME_FS: f64 = 13.0;
const SUB_FS: f64 = 11.0;
const PAD_X: f64 = 14.0;
const PAD_Y: f64 = 9.0;
const LINE_NAME: f64 = 17.0;
const LINE_SUB: f64 = 15.0;
const SIB_GAP: f64 = 26.0; // gap between sibling boxes on the cross axis
const LEVEL_GAP_V: f64 = 44.0; // vertical gap between depth rows
const LEVEL_GAP_H: f64 = 68.0; // horizontal gap between depth columns
const MARGIN: f64 = 24.0;
const NODE_W_MIN: f64 = 96.0;
const NODE_W_MAX: f64 = 280.0;

fn text_w(s: &str, char_w: f64) -> f64 {
    s.chars().count() as f64 * char_w
}

/// Render the org chart to an SVG string.
///
/// * `direction` — "vertical" (top-down, default) or "horizontal" (left-to-right).
/// * `color` — accent colour for the node bar/border (any CSS colour).
pub fn render_svg(data: &str, title: &str, direction: &str, color: &str) -> Result<String, String> {
    let mut roots = parse(data)?;
    let horizontal = direction.trim().eq_ignore_ascii_case("horizontal");
    let accent = sanitize_color(color, "#4e79a7");

    let has_title_line = roots.iter().any(|n| any_field(n, &|x: &Node| !x.title.is_empty()));
    let has_dept_line = roots.iter().any(|n| any_field(n, &|x: &Node| !x.dept.is_empty()));

    // Uniform node box: width from the widest label, height from the richest node.
    let name_cw = NAME_FS * 0.60;
    let sub_cw = SUB_FS * 0.58;
    let mut widest = 0.0f64;
    for_each(&roots, &mut |n| {
        widest = widest
            .max(text_w(&n.name, name_cw))
            .max(text_w(&n.title, sub_cw))
            .max(text_w(&n.dept, sub_cw));
    });
    let node_w = (widest + 2.0 * PAD_X).clamp(NODE_W_MIN, NODE_W_MAX);
    let node_h = 2.0 * PAD_Y
        + LINE_NAME
        + if has_title_line { LINE_SUB } else { 0.0 }
        + if has_dept_line { LINE_SUB } else { 0.0 };

    let max_name = ((node_w - 2.0 * PAD_X) / name_cw).floor().max(1.0) as usize;
    let max_sub = ((node_w - 2.0 * PAD_X) / sub_cw).floor().max(1.0) as usize;

    // --- Layout: leaf-pack the cross axis, place depth on the other axis. ---
    let cross_slot = if horizontal { node_h + SIB_GAP } else { node_w + SIB_GAP };
    let mut cursor = 0.0f64;
    for r in &mut roots {
        pack(r, cross_slot, horizontal, &mut cursor);
    }
    let title_space = if title.trim().is_empty() { 0.0 } else { 34.0 };
    for r in &mut roots {
        place_depth(r, 0, horizontal, node_w, node_h, title_space);
    }

    // Canvas bounds from the placed boxes.
    let (mut min_x, mut min_y, mut max_x, mut max_y) =
        (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for_each(&roots, &mut |n| {
        min_x = min_x.min(n.cx - node_w / 2.0);
        max_x = max_x.max(n.cx + node_w / 2.0);
        min_y = min_y.min(n.cy - node_h / 2.0);
        max_y = max_y.max(n.cy + node_h / 2.0);
    });
    // Shift so the top-left box sits at (MARGIN, MARGIN + title_space).
    let dx = MARGIN - min_x;
    let dy = MARGIN + title_space - min_y;
    for_each_mut(&mut roots, &mut |n| {
        n.cx += dx;
        n.cy += dy;
    });
    let width = (max_x - min_x) + 2.0 * MARGIN;
    let height = (max_y - min_y) + 2.0 * MARGIN + title_space;

    // --- Emit SVG. ---
    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.0} {h:.0}" font-family="Segoe UI, Helvetica, Arial, sans-serif">"##,
        w = width.ceil(),
        h = height.ceil()
    ));
    svg.push_str(&format!(
        r##"<rect width="{w:.0}" height="{h:.0}" fill="#ffffff"/>"##,
        w = width.ceil(),
        h = height.ceil()
    ));
    if !title.trim().is_empty() {
        svg.push_str(&format!(
            r##"<text x="{x:.1}" y="26" text-anchor="middle" font-size="18" font-weight="700" fill="#1f2933">{t}</text>"##,
            x = width / 2.0,
            t = esc(title.trim())
        ));
    }

    // Connectors first (so boxes draw on top).
    let mut edges = String::new();
    for r in &roots {
        emit_edges(r, horizontal, node_w, node_h, &mut edges);
    }
    svg.push_str(&format!(r##"<g fill="none" stroke="#9aa5b1" stroke-width="1.5">{edges}</g>"##));

    // Boxes.
    for_each(&roots, &mut |n| {
        let x = n.cx - node_w / 2.0;
        let y = n.cy - node_h / 2.0;
        svg.push_str(&format!(
            r##"<rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" rx="6" ry="6" fill="#ffffff" stroke="{a}" stroke-width="1.5"/>"##,
            w = node_w,
            h = node_h,
            a = accent
        ));
        // Accent bar along the top edge.
        svg.push_str(&format!(
            r##"<rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="4" rx="2" ry="2" fill="{a}"/>"##,
            w = node_w,
            a = accent
        ));
        let cx = n.cx;
        let mut ty = y + PAD_Y + LINE_NAME - 4.0;
        svg.push_str(&format!(
            r##"<text x="{cx:.1}" y="{ty:.1}" text-anchor="middle" font-size="{fs}" font-weight="600" fill="#1f2933">{name}</text>"##,
            fs = NAME_FS,
            name = esc(&truncate(&n.name, max_name))
        ));
        if !n.title.is_empty() {
            ty += LINE_SUB;
            svg.push_str(&format!(
                r##"<text x="{cx:.1}" y="{ty:.1}" text-anchor="middle" font-size="{fs}" fill="#52606d">{t}</text>"##,
                fs = SUB_FS,
                t = esc(&truncate(&n.title, max_sub))
            ));
        }
        if !n.dept.is_empty() {
            ty += LINE_SUB;
            svg.push_str(&format!(
                r##"<text x="{cx:.1}" y="{ty:.1}" text-anchor="middle" font-size="{fs}" fill="#7b8794">{d}</text>"##,
                fs = SUB_FS,
                d = esc(&truncate(&n.dept, max_sub))
            ));
        }
    });

    svg.push_str("</svg>");
    Ok(svg)
}

fn any_field(n: &Node, f: &dyn Fn(&Node) -> bool) -> bool {
    f(n) || n.children.iter().any(|c| any_field(c, f))
}

fn for_each(nodes: &[Node], f: &mut impl FnMut(&Node)) {
    for n in nodes {
        f(n);
        for_each(&n.children, f);
    }
}

fn for_each_mut(nodes: &mut [Node], f: &mut impl FnMut(&mut Node)) {
    for n in nodes {
        f(n);
        for_each_mut(&mut n.children, f);
    }
}

/// Leaf-pack the cross axis: leaves take the next slot, parents centre over kids.
fn pack(node: &mut Node, slot: f64, horizontal: bool, cursor: &mut f64) -> f64 {
    let center = if node.children.is_empty() {
        let c = *cursor + slot / 2.0;
        *cursor += slot;
        c
    } else {
        let mut first = None;
        let mut last = 0.0;
        for ch in &mut node.children {
            let c = pack(ch, slot, horizontal, cursor);
            if first.is_none() {
                first = Some(c);
            }
            last = c;
        }
        (first.unwrap() + last) / 2.0
    };
    if horizontal {
        node.cy = center;
    } else {
        node.cx = center;
    }
    center
}

/// Place each node along the depth axis (row for vertical, column for horizontal).
fn place_depth(node: &mut Node, depth: usize, horizontal: bool, node_w: f64, node_h: f64, title_space: f64) {
    if horizontal {
        node.cx = depth as f64 * (node_w + LEVEL_GAP_H) + node_w / 2.0;
    } else {
        node.cy = title_space + depth as f64 * (node_h + LEVEL_GAP_V) + node_h / 2.0;
    }
    for ch in &mut node.children {
        place_depth(ch, depth + 1, horizontal, node_w, node_h, title_space);
    }
}

fn emit_edges(node: &Node, horizontal: bool, node_w: f64, node_h: f64, out: &mut String) {
    for ch in &node.children {
        if horizontal {
            let px = node.cx + node_w / 2.0;
            let py = node.cy;
            let cxp = ch.cx - node_w / 2.0;
            let cyp = ch.cy;
            let mid = (px + cxp) / 2.0;
            out.push_str(&format!(r##"<path d="M{px:.1} {py:.1} H{mid:.1} V{cyp:.1} H{cxp:.1}"/>"##));
        } else {
            let px = node.cx;
            let py = node.cy + node_h / 2.0;
            let cxp = ch.cx;
            let cyp = ch.cy - node_h / 2.0;
            let mid = (py + cyp) / 2.0;
            out.push_str(&format!(r##"<path d="M{px:.1} {py:.1} V{mid:.1} H{cxp:.1} V{cyp:.1}"/>"##));
        }
        emit_edges(ch, horizontal, node_w, node_h, out);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_builds_hierarchy() {
        let data = "CEO | Chief Exec\n  VP Eng | VP Engineering\n    Dev A\n    Dev B\n  VP Sales";
        let roots = parse(data).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "CEO");
        assert_eq!(roots[0].title, "Chief Exec");
        assert_eq!(roots[0].children.len(), 2);
        assert_eq!(roots[0].children[0].name, "VP Eng");
        assert_eq!(roots[0].children[0].children.len(), 2);
        assert_eq!(roots[0].children[0].children[1].name, "Dev B");
        assert_eq!(roots[0].children[1].name, "VP Sales");
    }

    #[test]
    fn flat_json_links_by_manager() {
        let data = r#"[
            {"name":"Ada","title":"CEO"},
            {"name":"Grace","manager":"Ada"},
            {"name":"Linus","manager":"Ada"}
        ]"#;
        let roots = parse(data).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "Ada");
        assert_eq!(roots[0].children.len(), 2);
    }

    #[test]
    fn nested_json_object_root() {
        let data = r#"{"name":"Root","children":[{"name":"Child","title":"IC"}]}"#;
        let roots = parse(data).unwrap();
        assert_eq!(roots[0].children[0].title, "IC");
    }

    #[test]
    fn renders_svg_with_boxes() {
        let svg = render_svg("CEO\n  Worker", "My Org", "vertical", "#336699").unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("My Org"));
        assert!(svg.contains("CEO"));
        assert!(svg.contains("Worker"));
        // one connector between CEO and Worker
        assert_eq!(svg.matches("<path").count(), 1);
        assert!(svg.contains("#336699"));
    }

    #[test]
    fn err_on_empty() {
        assert!(parse("   \n  \n").is_err());
    }

    #[test]
    fn err_on_dangling_manager() {
        let data = r#"[{"name":"A","manager":"Ghost"}]"#;
        assert!(parse(data).is_err());
    }

    #[test]
    fn err_on_cycle() {
        let data = r#"[{"name":"A","manager":"B"},{"name":"B","manager":"A"}]"#;
        assert!(parse(data).is_err());
    }

    #[test]
    fn err_on_too_many() {
        let mut s = String::new();
        for i in 0..(MAX_NODES + 1) {
            s.push_str(&format!("P{i}\n"));
        }
        assert!(parse(&s).is_err());
    }

    #[test]
    fn color_is_sanitized() {
        // A colour with a quote is rejected → fallback used, no injection.
        let svg = render_svg("A", "", "vertical", "\"><script>").unwrap();
        assert!(!svg.contains("<script>"));
        assert!(svg.contains("#4e79a7"));
    }

    #[test]
    fn horizontal_direction_places_columns() {
        let svg = render_svg("Boss\n  Report", "", "horizontal", "#000").unwrap();
        assert!(svg.contains("<path"));
        assert!(svg.starts_with("<svg"));
    }
}
