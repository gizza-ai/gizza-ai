//! outline-to-mindmap core — turn an indented text outline into a mind-map SVG.
//!
//! Pure Rust (no wafer/wasm-bindgen deps), so it runs on every backend
//! including the chat Service Worker. The pipeline is: parse the indented
//! outline into a tree (stack-based, tolerant of tabs / mixed indents and
//! optional bullet markers) → tidy-tree layout (left-to-right or top-down) →
//! emit a standalone, scalable SVG. Nothing is uploaded; the SVG is returned as
//! text markup.

use std::fmt::Write as _;

/// Layout direction for the mind map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Central topic on the left, branches grow rightward (classic mind map).
    Right,
    /// Central topic on top, branches grow downward (org-chart / tree style).
    Down,
}

impl Direction {
    /// Parse a direction string ("right"/"down", case-insensitive). Anything
    /// else (incl. blank) falls back to `Right`.
    pub fn parse(s: &str) -> Direction {
        match s.trim().to_ascii_lowercase().as_str() {
            "down" | "vertical" | "top-down" | "tree" => Direction::Down,
            _ => Direction::Right,
        }
    }
}

/// Render options.
#[derive(Clone, Debug)]
pub struct Options {
    /// Layout direction.
    pub direction: Direction,
    /// Colorize each top-level branch (and its descendants) with a distinct
    /// color. When false the map is rendered in a neutral monochrome theme.
    pub colorful: bool,
    /// Recolor for a dark background (dark canvas, light text).
    pub dark_mode: bool,
    /// Label for the synthetic central node used when the outline has more than
    /// one top-level item. Ignored when the outline already has a single root.
    pub title: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            direction: Direction::Right,
            colorful: true,
            dark_mode: false,
            title: "Mind Map".to_string(),
        }
    }
}

// Layout / rendering constants.
const NODE_H: f64 = 38.0;
const CHAR_W: f64 = 7.6; // ~ average glyph advance at 14px sans-serif
const PAD_X: f64 = 14.0; // horizontal text padding inside a node
const MIN_W: f64 = 56.0;
const MAX_LABEL: usize = 80; // truncate very long labels with an ellipsis
const H_GAP: f64 = 56.0; // gap between depth columns (Right) / sibling gap (Down)
const V_GAP: f64 = 16.0; // gap between sibling subtrees (Right) / depth rows (Down)
const MARGIN: f64 = 24.0;
const FONT: f64 = 14.0;
const MAX_NODES: usize = 4000;
const MAX_DEPTH: usize = 200;

const PALETTE: [&str; 8] = [
    "#2563eb", "#16a34a", "#dc2626", "#9333ea", "#ea580c", "#0891b2", "#db2777", "#65a30d",
];

struct Node {
    text: String,
    children: Vec<Node>,
}

/// A laid-out node ready for rendering.
struct Placed {
    text: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    parent: Option<usize>,
    branch: usize, // index into PALETTE for this node's top-level branch
    is_center: bool,
}

/// Turn an indented outline into a standalone mind-map SVG string.
pub fn render(outline: &str, opts: &Options) -> Result<String, String> {
    let roots = parse_outline(outline)?;
    if roots.is_empty() {
        return Err("Outline is empty — add at least one line of text.".into());
    }

    // Establish a single center node. A lone root becomes the center; multiple
    // top-level items hang under a synthetic central node.
    let center = if roots.len() == 1 {
        roots.into_iter().next().unwrap()
    } else {
        let title = opts.title.trim();
        Node {
            text: if title.is_empty() { "Mind Map".into() } else { title.into() },
            children: roots,
        }
    };

    let mut placed: Vec<Placed> = Vec::new();
    flatten(&center, None, usize::MAX, &mut placed);
    if placed.len() > MAX_NODES {
        return Err(format!(
            "Outline is too large ({} nodes); the limit is {}.",
            placed.len(),
            MAX_NODES
        ));
    }

    // Node sizes from label length.
    for p in placed.iter_mut() {
        p.w = node_width(&p.text);
        p.h = NODE_H;
    }

    let (width, height) = match opts.direction {
        Direction::Right => layout_right(&mut placed),
        Direction::Down => layout_down(&mut placed),
    };

    Ok(emit_svg(&placed, width, height, opts))
}

/// Strip a leading bullet / list marker from a line (already left-trimmed).
fn strip_marker(s: &str) -> &str {
    // Single-char bullets: - * + • – —
    for m in ["- ", "* ", "+ ", "• ", "– ", "— ", "·  ", "· "] {
        if let Some(rest) = s.strip_prefix(m) {
            return rest.trim_start();
        }
    }
    // Numbered: "1." / "1)" / "12. " etc.
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let rest = &s[digits.len()..];
        if let Some(r) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return r.trim_start();
        }
    }
    s
}

/// Indentation width of a line: tabs expand to the next multiple of 4 so mixed
/// tab/space indents still order correctly.
fn indent_width(line: &str) -> usize {
    let mut col = 0usize;
    for c in line.chars() {
        match c {
            ' ' => col += 1,
            '\t' => col = (col / 4 + 1) * 4,
            _ => break,
        }
    }
    col
}

/// Parse an indented outline into a forest of trees (stack-based; tolerant of
/// irregular indentation — deeper indent = deeper nesting).
fn parse_outline(outline: &str) -> Result<Vec<Node>, String> {
    let mut roots: Vec<Node> = Vec::new();
    // Stack of (indent_width, path) where path indexes into the tree from a root
    // down to the current node. We store paths so we can re-borrow mutably.
    let mut stack: Vec<(usize, Vec<usize>)> = Vec::new();

    for raw in outline.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        let indent = indent_width(raw);
        let mut text = strip_marker(raw.trim_start()).trim().to_string();
        if text.is_empty() {
            continue;
        }
        if text.chars().count() > MAX_LABEL {
            let truncated: String = text.chars().take(MAX_LABEL - 1).collect();
            text = format!("{truncated}…");
        }

        // Pop until the stack top is strictly shallower than this line.
        while let Some((ind, _)) = stack.last() {
            if *ind >= indent {
                stack.pop();
            } else {
                break;
            }
        }
        if stack.len() > MAX_DEPTH {
            return Err(format!("Outline nesting exceeds the {MAX_DEPTH}-level limit."));
        }

        let node = Node { text, children: Vec::new() };
        let path = if let Some((_, parent_path)) = stack.last() {
            let parent_path = parent_path.clone();
            let child = child_at(&mut roots, &parent_path);
            child.children.push(node);
            let mut p = parent_path;
            p.push(child.children.len() - 1);
            p
        } else {
            roots.push(node);
            vec![roots.len() - 1]
        };
        stack.push((indent, path));
    }

    Ok(roots)
}

/// Follow a path (root index, then child indices) to a node.
fn child_at<'a>(roots: &'a mut [Node], path: &[usize]) -> &'a mut Node {
    let mut node = &mut roots[path[0]];
    for &i in &path[1..] {
        node = &mut node.children[i];
    }
    node
}

/// Flatten the tree into `Placed` entries (positions filled later). `branch` is
/// the palette index of the top-level branch this node belongs to.
fn flatten(node: &Node, parent: Option<usize>, inherited_branch: usize, out: &mut Vec<Placed>) {
    let idx = out.len();
    let is_center = parent.is_none();
    out.push(Placed {
        text: node.text.clone(),
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
        parent,
        branch: inherited_branch,
        is_center,
    });
    for (i, child) in node.children.iter().enumerate() {
        // Children of the center start a new branch color; deeper nodes inherit.
        let branch = if is_center { i % PALETTE.len() } else { inherited_branch };
        flatten(child, Some(idx), branch, out);
    }
}

fn node_width(text: &str) -> f64 {
    let chars = text.chars().count() as f64;
    (chars * CHAR_W + PAD_X * 2.0).max(MIN_W)
}

/// Children index lists, by parent.
fn children_of(placed: &[Placed]) -> Vec<Vec<usize>> {
    let mut kids = vec![Vec::new(); placed.len()];
    for (i, p) in placed.iter().enumerate() {
        if let Some(par) = p.parent {
            kids[par].push(i);
        }
    }
    kids
}

/// Tidy left-to-right layout: depth → x column, subtree → vertical band.
fn layout_right(placed: &mut [Placed]) -> (f64, f64) {
    let kids = children_of(placed);
    let depth = depths(placed, &kids);
    let max_depth = depth.iter().copied().max().unwrap_or(0);

    // Column x = cumulative max width per depth.
    let mut col_max_w = vec![MIN_W; max_depth + 1];
    for (i, p) in placed.iter().enumerate() {
        if p.w > col_max_w[depth[i]] {
            col_max_w[depth[i]] = p.w;
        }
    }
    let mut col_x = vec![MARGIN; max_depth + 1];
    for d in 1..=max_depth {
        col_x[d] = col_x[d - 1] + col_max_w[d - 1] + H_GAP;
    }

    let mut cursor = MARGIN;
    place_y(0, &kids, placed, &depth, &col_x, &mut cursor);

    let width = col_x[max_depth] + col_max_w[max_depth] + MARGIN;
    let height = placed.iter().map(|p| p.y + p.h).fold(0.0_f64, f64::max) + MARGIN;
    (width, height)
}

/// Post-order y placement for the Right layout; returns this node's center y.
fn place_y(
    i: usize,
    kids: &[Vec<usize>],
    placed: &mut [Placed],
    depth: &[usize],
    col_x: &[f64],
    cursor: &mut f64,
) -> f64 {
    placed[i].x = col_x[depth[i]];
    let center_y;
    if kids[i].is_empty() {
        center_y = *cursor + NODE_H / 2.0;
        *cursor += NODE_H + V_GAP;
    } else {
        let mut first = 0.0;
        let mut last = 0.0;
        for (k, &c) in kids[i].iter().enumerate() {
            let cy = place_y(c, kids, placed, depth, col_x, cursor);
            if k == 0 {
                first = cy;
            }
            last = cy;
        }
        center_y = (first + last) / 2.0;
    }
    placed[i].y = center_y - NODE_H / 2.0;
    center_y
}

/// Tidy top-down layout: depth → y row, subtree → horizontal band.
fn layout_down(placed: &mut [Placed]) -> (f64, f64) {
    let kids = children_of(placed);
    let depth = depths(placed, &kids);
    let max_depth = depth.iter().copied().max().unwrap_or(0);

    let row_y: Vec<f64> = (0..=max_depth)
        .map(|d| MARGIN + d as f64 * (NODE_H + V_GAP + 22.0))
        .collect();

    let mut cursor = MARGIN;
    place_x(0, &kids, placed, &depth, &row_y, &mut cursor);

    let width = placed.iter().map(|p| p.x + p.w).fold(0.0_f64, f64::max) + MARGIN;
    let height = row_y[max_depth] + NODE_H + MARGIN;
    (width, height)
}

/// Post-order x placement for the Down layout; returns this node's center x.
fn place_x(
    i: usize,
    kids: &[Vec<usize>],
    placed: &mut [Placed],
    depth: &[usize],
    row_y: &[f64],
    cursor: &mut f64,
) -> f64 {
    placed[i].y = row_y[depth[i]];
    let center_x;
    if kids[i].is_empty() {
        center_x = *cursor + placed[i].w / 2.0;
        *cursor += placed[i].w + H_GAP;
    } else {
        let mut first = 0.0;
        let mut last = 0.0;
        for (k, &c) in kids[i].iter().enumerate() {
            let cx = place_x(c, kids, placed, depth, row_y, cursor);
            if k == 0 {
                first = cx;
            }
            last = cx;
        }
        center_x = (first + last) / 2.0;
    }
    placed[i].x = center_x - placed[i].w / 2.0;
    center_x
}

/// Depth of each node (root = 0). Nodes are produced in pre-order by `flatten`,
/// so a parent always precedes its children and one forward pass suffices.
fn depths(placed: &[Placed], kids: &[Vec<usize>]) -> Vec<usize> {
    let mut depth = vec![0usize; placed.len()];
    for i in 0..placed.len() {
        for &c in &kids[i] {
            depth[c] = depth[i] + 1;
        }
    }
    depth
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn emit_svg(placed: &[Placed], width: f64, height: f64, opts: &Options) -> String {
    let bg = if opts.dark_mode { "#0f172a" } else { "#ffffff" };
    let center_fill = if opts.dark_mode { "#e2e8f0" } else { "#1e293b" };
    let center_text = if opts.dark_mode { "#0f172a" } else { "#ffffff" };
    let mono_fill = if opts.dark_mode { "#1e293b" } else { "#f1f5f9" };
    let mono_text = if opts.dark_mode { "#e2e8f0" } else { "#0f172a" };
    let mono_stroke = if opts.dark_mode { "#475569" } else { "#cbd5e1" };

    let w = width.round() as i64;
    let h = height.round() as i64;

    let mut s = String::new();
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\" font-family=\"-apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif\">"
    );
    let _ = write!(s, "<rect width=\"{w}\" height=\"{h}\" fill=\"{bg}\"/>");

    // Edges first (drawn under the nodes).
    for p in placed {
        let Some(par) = p.parent else { continue };
        let parent = &placed[par];
        let color = if opts.colorful {
            PALETTE[p.branch % PALETTE.len()]
        } else if opts.dark_mode {
            "#64748b"
        } else {
            "#94a3b8"
        };
        let path = match opts.direction {
            Direction::Right => {
                let x1 = parent.x + parent.w;
                let y1 = parent.y + parent.h / 2.0;
                let x2 = p.x;
                let y2 = p.y + p.h / 2.0;
                let dx = (x2 - x1) / 2.0;
                format!(
                    "M{x1:.1} {y1:.1} C{:.1} {y1:.1} {:.1} {y2:.1} {x2:.1} {y2:.1}",
                    x1 + dx,
                    x2 - dx
                )
            }
            Direction::Down => {
                let x1 = parent.x + parent.w / 2.0;
                let y1 = parent.y + parent.h;
                let x2 = p.x + p.w / 2.0;
                let y2 = p.y;
                let dy = (y2 - y1) / 2.0;
                format!(
                    "M{x1:.1} {y1:.1} C{x1:.1} {:.1} {x2:.1} {:.1} {x2:.1} {y2:.1}",
                    y1 + dy,
                    y2 - dy
                )
            }
        };
        let _ = write!(
            s,
            "<path d=\"{path}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2\" stroke-linecap=\"round\"/>",
        );
    }

    // Nodes.
    for p in placed {
        let (fill, text_color, stroke) = if p.is_center {
            (center_fill.to_string(), center_text, "none".to_string())
        } else if opts.colorful {
            let c = PALETTE[p.branch % PALETTE.len()];
            (c.to_string(), "#ffffff", "none".to_string())
        } else {
            (mono_fill.to_string(), mono_text, mono_stroke.to_string())
        };
        let rx = p.h / 2.0;
        let stroke_attr = if stroke == "none" {
            String::new()
        } else {
            format!(" stroke=\"{stroke}\" stroke-width=\"1.5\"")
        };
        let _ = write!(
            s,
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"{rx:.1}\" ry=\"{rx:.1}\" fill=\"{fill}\"{stroke_attr}/>",
            p.x, p.y, p.w, p.h
        );
        let weight = if p.is_center { "600" } else { "500" };
        let _ = write!(
            s,
            "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{text_color}\" font-size=\"{FONT}\" font-weight=\"{weight}\" \
             text-anchor=\"middle\" dominant-baseline=\"central\">{}</text>",
            p.x + p.w / 2.0,
            p.y + p.h / 2.0,
            esc(&p.text)
        );
    }

    s.push_str("</svg>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_outline() {
        let outline = "Project\n  Design\n    Wireframe\n  Build\n  Ship";
        let svg = render(outline, &Options::default()).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains(">Project</text>"));
        assert!(svg.contains(">Wireframe</text>"));
        // One root + 4 descendants = 5 nodes.
        assert_eq!(svg.matches("<text").count(), 5);
    }

    #[test]
    fn multiple_roots_get_synthetic_center() {
        let outline = "Apples\nOranges\nPears";
        let opts = Options { title: "Fruit".into(), ..Options::default() };
        let svg = render(outline, &opts).unwrap();
        assert!(svg.contains(">Fruit</text>"));
        assert_eq!(svg.matches("<text").count(), 4); // center + 3
    }

    #[test]
    fn strips_bullet_and_number_markers() {
        let outline = "Roadmap\n  - alpha\n  * beta\n  1. release";
        let svg = render(outline, &Options::default()).unwrap();
        assert!(svg.contains(">alpha</text>"));
        assert!(svg.contains(">beta</text>"));
        assert!(svg.contains(">release</text>"));
        assert!(!svg.contains(">- alpha</text>"));
    }

    #[test]
    fn handles_tab_indentation() {
        let outline = "Root\n\tChild A\n\t\tGrandchild\n\tChild B";
        let svg = render(outline, &Options::default()).unwrap();
        assert!(svg.contains(">Grandchild</text>"));
        assert_eq!(svg.matches("<text").count(), 4);
    }

    #[test]
    fn down_direction_renders() {
        let outline = "Top\n  Left\n  Right";
        let opts = Options { direction: Direction::Down, ..Options::default() };
        let svg = render(outline, &opts).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains(">Top</text>"));
    }

    #[test]
    fn colorful_uses_palette_monochrome_does_not() {
        let outline = "A\n  B";
        let colorful = render(outline, &Options { colorful: true, ..Options::default() }).unwrap();
        assert!(colorful.contains("#2563eb"));
        let mono = render(outline, &Options { colorful: false, ..Options::default() }).unwrap();
        assert!(!mono.contains("#2563eb"));
    }

    #[test]
    fn dark_mode_changes_background() {
        let outline = "A\n  B";
        let dark = render(outline, &Options { dark_mode: true, ..Options::default() }).unwrap();
        assert!(dark.contains("fill=\"#0f172a\""));
    }

    #[test]
    fn escapes_xml_special_chars() {
        let outline = "Plans & <ideas>\n  \"quoted\"";
        let svg = render(outline, &Options::default()).unwrap();
        assert!(svg.contains("Plans &amp; &lt;ideas&gt;"));
        assert!(!svg.contains("<ideas>"));
    }

    #[test]
    fn empty_outline_errors() {
        assert!(render("", &Options::default()).is_err());
        assert!(render("   \n  \n", &Options::default()).is_err());
    }

    #[test]
    fn direction_parse() {
        assert_eq!(Direction::parse("down"), Direction::Down);
        assert_eq!(Direction::parse("Right"), Direction::Right);
        assert_eq!(Direction::parse(""), Direction::Right);
        assert_eq!(Direction::parse("vertical"), Direction::Down);
    }
}
