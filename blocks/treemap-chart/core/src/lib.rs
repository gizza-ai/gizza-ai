//! Treemap layout + deterministic SVG rendering.
//!
//! Takes pasted `label,value` rows, `parent/child,value` paths, or a grouped table
//! (`region,city,value`), aggregates them into a tree, tiles the rectangle with a
//! squarified / slice-and-dice / binary algorithm, and renders self-contained SVG.
//! No external crates, no I/O, fully deterministic.

pub const MAX_ROWS: usize = 20_000;
pub const MAX_TILES: usize = 5_000;
pub const MAX_DEPTH: usize = 12;

/// Every knob the block, CLI, and page expose.
#[derive(Clone, Debug)]
pub struct Options {
    pub layout: String,
    pub path_separator: String,
    pub sort: String,
    pub tiling: String,
    pub max_depth: u32,
    pub top_n: u32,
    pub show_labels: bool,
    pub show_values: bool,
    pub show_percent: bool,
    pub label_position: String,
    pub font_size: f64,
    pub palette: String,
    pub color: String,
    pub background: String,
    pub border_width: f64,
    pub corner_radius: f64,
    pub title: String,
    pub legend: bool,
    pub width: u32,
    pub height: u32,
    pub theme: String,
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            layout: "auto".into(),
            path_separator: "/".into(),
            sort: "value_desc".into(),
            tiling: "squarified".into(),
            max_depth: 0,
            top_n: 0,
            show_labels: true,
            show_values: true,
            show_percent: false,
            label_position: "top".into(),
            font_size: 13.0,
            palette: "default".into(),
            color: "#2563eb".into(),
            background: "".into(),
            border_width: 2.0,
            corner_radius: 2.0,
            title: "".into(),
            legend: false,
            width: 800,
            height: 500,
            theme: "light".into(),
            output: "svg".into(),
        }
    }
}

// ---------------------------------------------------------------- geometry

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

// ---------------------------------------------------------------- parsing

fn split_fields(line: &str) -> Vec<String> {
    let t = line.trim_end();
    if t.contains('\t') {
        t.split('\t').map(|s| s.trim().to_string()).collect()
    } else if t.contains(',') {
        t.split(',').map(|s| s.trim().to_string()).collect()
    } else if t.contains(';') {
        t.split(';').map(|s| s.trim().to_string()).collect()
    } else if let Some(p) = t.rfind(char::is_whitespace) {
        let (a, b) = t.split_at(p);
        vec![a.trim().to_string(), b.trim().to_string()]
    } else {
        vec![t.trim().to_string()]
    }
}

/// Parse a numeric cell, tolerating thousands separators, currency marks and a trailing `%`.
fn parse_value(tok: &str) -> Option<f64> {
    let mut s = String::new();
    for ch in tok.chars() {
        match ch {
            '0'..='9' | '.' | '-' | '+' => s.push(ch),
            ',' | ' ' | '_' | '\'' | '$' | '%' => {}
            '\u{20ac}' | '\u{a3}' | '\u{a5}' => {} // € £ ¥
            _ => return None,
        }
    }
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

struct Row {
    path: Vec<String>,
    value: f64,
}

fn detect_layout(rows: &[Vec<String>], sep: &str) -> &'static str {
    let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if max_cols >= 3 {
        return "grouped";
    }
    if !sep.is_empty() && rows.iter().any(|r| r.first().is_some_and(|f| f.contains(sep))) {
        return "path";
    }
    "flat"
}

fn parse_rows(data: &str, opts: &Options) -> Result<(Vec<Row>, String), String> {
    let mut raw: Vec<(usize, Vec<String>)> = Vec::new();
    for (i, line) in data.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        raw.push((i + 1, split_fields(line)));
        if raw.len() > MAX_ROWS + 1 {
            return Err(format!(
                "too many rows: this tool accepts at most {MAX_ROWS} data rows, aggregate the tail before pasting"
            ));
        }
    }
    if raw.is_empty() {
        return Err("data is empty: paste at least one `label,value` row".into());
    }

    // Header row = first row whose last cell is not a number, when a later row has one.
    let first_last_numeric = raw[0].1.last().and_then(|s| parse_value(s)).is_some();
    let any_later_numeric = raw
        .iter()
        .skip(1)
        .any(|(_, f)| f.last().and_then(|s| parse_value(s)).is_some());
    if !first_last_numeric {
        if !any_later_numeric {
            let (ln, f) = &raw[0];
            return Err(format!(
                "line {ln}: expected a number in the last column, got `{}` — rows look like `label,value`",
                f.last().cloned().unwrap_or_default()
            ));
        }
        raw.remove(0);
        if raw.is_empty() {
            return Err("no data rows found after the header row".into());
        }
    }

    let fields: Vec<Vec<String>> = raw.iter().map(|(_, f)| f.clone()).collect();
    let sep = opts.path_separator.as_str();
    let layout = match opts.layout.trim() {
        "" | "auto" => detect_layout(&fields, sep),
        "flat" => "flat",
        "path" => "path",
        "grouped" => "grouped",
        other => {
            return Err(format!(
                "unknown layout `{other}`: expected auto, flat, path, or grouped"
            ))
        }
    };

    let cap = if opts.max_depth == 0 {
        MAX_DEPTH
    } else {
        (opts.max_depth as usize).min(MAX_DEPTH)
    };

    let mut out: Vec<Row> = Vec::with_capacity(raw.len());
    for (ln, f) in raw {
        if f.len() < 2 {
            return Err(format!(
                "line {ln}: expected `label,value` but found only one field (`{}`)",
                f.first().cloned().unwrap_or_default()
            ));
        }
        let last = f.last().unwrap();
        let label_cells = &f[..f.len() - 1];
        let label = label_cells.join(" / ");
        let value = match parse_value(last) {
            Some(v) => v,
            None => {
                return Err(format!(
                    "line {ln}: expected a number for the value of `{label}`, got `{last}`"
                ))
            }
        };
        if value < 0.0 {
            return Err(format!(
                "line {ln}: value for `{label}` is {}, but treemap areas require values of 0 or more",
                fmt_value(value)
            ));
        }

        let mut path: Vec<String> = match layout {
            "path" => {
                if sep.is_empty() {
                    vec![label_cells[0].clone()]
                } else {
                    label_cells[0]
                        .split(sep)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                }
            }
            "grouped" => label_cells
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            // flat
            _ => vec![label_cells[0].trim().to_string()],
        };
        if path.is_empty() {
            path.push("(unnamed)".into());
        }
        path.truncate(cap);
        out.push(Row { path, value });
    }
    Ok((out, layout.to_string()))
}

// ---------------------------------------------------------------- tree

#[derive(Debug)]
struct Node {
    name: String,
    self_value: f64,
    value: f64,
    order: usize,
    children: Vec<Node>,
}

impl Node {
    fn new(name: &str, order: usize) -> Node {
        Node {
            name: name.to_string(),
            self_value: 0.0,
            value: 0.0,
            order,
            children: Vec::new(),
        }
    }
    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

fn build_tree(rows: &[Row]) -> Node {
    let mut root = Node::new("", 0);
    let mut counter = 0usize;
    for row in rows {
        let mut cur = &mut root;
        for seg in &row.path {
            let idx = match cur.children.iter().position(|c| &c.name == seg) {
                Some(i) => i,
                None => {
                    counter += 1;
                    cur.children.push(Node::new(seg, counter));
                    cur.children.len() - 1
                }
            };
            cur = &mut cur.children[idx];
        }
        cur.self_value += row.value;
    }
    root
}

/// Values recorded directly on a node that also has children become a `(direct)` tile
/// so the child rectangles still tile the parent's full area.
fn normalize(node: &mut Node) {
    if !node.children.is_empty() && node.self_value > 0.0 {
        let mut direct = Node::new("(direct)", usize::MAX);
        direct.self_value = node.self_value;
        direct.value = node.self_value;
        node.self_value = 0.0;
        node.children.push(direct);
    }
    let mut total = 0.0;
    for c in node.children.iter_mut() {
        normalize(c);
        total += c.value;
    }
    node.value = if node.children.is_empty() {
        node.self_value
    } else {
        total
    };
}

fn sort_children(node: &mut Node, mode: &str) {
    match mode {
        "value_asc" => node
            .children
            .sort_by(|a, b| a.value.total_cmp(&b.value).then(a.order.cmp(&b.order))),
        "input" => node.children.sort_by(|a, b| a.order.cmp(&b.order)),
        "label" => node
            .children
            .sort_by(|a, b| a.name.cmp(&b.name).then(a.order.cmp(&b.order))),
        // value_desc (default)
        _ => node
            .children
            .sort_by(|a, b| b.value.total_cmp(&a.value).then(a.order.cmp(&b.order))),
    }
    for c in node.children.iter_mut() {
        sort_children(c, mode);
    }
}

fn apply_top_n(node: &mut Node, n: usize) {
    if n > 0 && node.children.len() > n {
        node.children
            .sort_by(|a, b| b.value.total_cmp(&a.value).then(a.order.cmp(&b.order)));
        let rest: Vec<Node> = node.children.split_off(n);
        let sum: f64 = rest.iter().map(|c| c.value).sum();
        if sum > 0.0 {
            let mut other = Node::new("Other", usize::MAX);
            other.self_value = sum;
            other.value = sum;
            node.children.push(other);
        }
    }
    for c in node.children.iter_mut() {
        apply_top_n(c, n);
    }
}

fn count_nodes(node: &Node, depth: usize, leaves: &mut usize, groups: &mut usize, max: &mut usize) {
    if depth > *max {
        *max = depth;
    }
    if depth > 0 {
        if node.is_leaf() {
            *leaves += 1;
        } else {
            *groups += 1;
        }
    }
    for c in &node.children {
        count_nodes(c, depth + 1, leaves, groups, max);
    }
}

// ---------------------------------------------------------------- tiling

fn tile_equal(n: usize, r: Rect) -> Vec<Rect> {
    let mut out = Vec::with_capacity(n);
    if n == 0 {
        return out;
    }
    let w = r.w / n as f64;
    for i in 0..n {
        out.push(Rect {
            x: r.x + w * i as f64,
            y: r.y,
            w,
            h: r.h,
        });
    }
    out
}

fn squarify(values: &[f64], rect: Rect) -> Vec<Rect> {
    let n = values.len();
    let mut out = vec![
        Rect {
            x: rect.x,
            y: rect.y,
            w: 0.0,
            h: 0.0
        };
        n
    ];
    let total: f64 = values.iter().sum();
    if n == 0 || total <= 0.0 || rect.w <= 0.0 || rect.h <= 0.0 {
        return out;
    }
    let mut r = rect;
    let mut remaining = total;
    let mut i = 0usize;
    while i < n {
        if r.w <= 0.0 || r.h <= 0.0 || remaining <= 0.0 {
            break;
        }
        let scale = (r.w * r.h) / remaining;
        let short = r.w.min(r.h);
        let mut j = i;
        let mut row_area = 0.0f64;
        let mut best = f64::INFINITY;
        let mut rmin = f64::INFINITY;
        let mut rmax = 0.0f64;
        while j < n {
            let a = values[j] * scale;
            let new_area = row_area + a;
            let nmin = rmin.min(a);
            let nmax = rmax.max(a);
            let w2 = short * short;
            let s2 = new_area * new_area;
            let ratio = if new_area <= 0.0 || nmin <= 0.0 {
                f64::INFINITY
            } else {
                (w2 * nmax / s2).max(s2 / (w2 * nmin))
            };
            if j > i && ratio > best {
                break;
            }
            best = ratio;
            row_area = new_area;
            rmin = nmin;
            rmax = nmax;
            j += 1;
        }
        let row_value: f64 = values[i..j].iter().sum();
        if r.w <= r.h {
            let strip_h = (row_area / r.w).min(r.h);
            let mut cx = r.x;
            for (k, item) in out.iter_mut().enumerate().take(j).skip(i) {
                let cw = if strip_h > 0.0 {
                    values[k] * scale / strip_h
                } else {
                    0.0
                };
                *item = Rect {
                    x: cx,
                    y: r.y,
                    w: cw,
                    h: strip_h,
                };
                cx += cw;
            }
            r = Rect {
                x: r.x,
                y: r.y + strip_h,
                w: r.w,
                h: (r.h - strip_h).max(0.0),
            };
        } else {
            let strip_w = (row_area / r.h).min(r.w);
            let mut cy = r.y;
            for (k, item) in out.iter_mut().enumerate().take(j).skip(i) {
                let ch = if strip_w > 0.0 {
                    values[k] * scale / strip_w
                } else {
                    0.0
                };
                *item = Rect {
                    x: r.x,
                    y: cy,
                    w: strip_w,
                    h: ch,
                };
                cy += ch;
            }
            r = Rect {
                x: r.x + strip_w,
                y: r.y,
                w: (r.w - strip_w).max(0.0),
                h: r.h,
            };
        }
        remaining -= row_value;
        i = j;
    }
    out
}

fn slice_dice(values: &[f64], rect: Rect, depth: usize) -> Vec<Rect> {
    let total: f64 = values.iter().sum();
    let mut out = Vec::with_capacity(values.len());
    if total <= 0.0 {
        return tile_equal(values.len(), rect);
    }
    if depth % 2 == 0 {
        let mut cx = rect.x;
        for v in values {
            let w = rect.w * v / total;
            out.push(Rect {
                x: cx,
                y: rect.y,
                w,
                h: rect.h,
            });
            cx += w;
        }
    } else {
        let mut cy = rect.y;
        for v in values {
            let h = rect.h * v / total;
            out.push(Rect {
                x: rect.x,
                y: cy,
                w: rect.w,
                h,
            });
            cy += h;
        }
    }
    out
}

fn binary_split(values: &[f64], idx: &[usize], rect: Rect, out: &mut [Rect]) {
    if idx.is_empty() {
        return;
    }
    if idx.len() == 1 {
        out[idx[0]] = rect;
        return;
    }
    let total: f64 = idx.iter().map(|&i| values[i]).sum();
    if total <= 0.0 {
        for (k, r) in tile_equal(idx.len(), rect).into_iter().enumerate() {
            out[idx[k]] = r;
        }
        return;
    }
    let half = total / 2.0;
    let mut acc = 0.0;
    let mut split = 1usize;
    for (k, &i) in idx.iter().enumerate() {
        let next = acc + values[i];
        if next >= half {
            // pick whichever boundary lands closer to the half-way point
            split = if (half - acc).abs() < (next - half).abs() && k > 0 {
                k
            } else {
                k + 1
            };
            break;
        }
        acc = next;
        split = k + 2;
    }
    let split = split.clamp(1, idx.len() - 1);
    let left_sum: f64 = idx[..split].iter().map(|&i| values[i]).sum();
    let frac = left_sum / total;
    let (a, b) = if rect.w >= rect.h {
        let w = rect.w * frac;
        (
            Rect {
                x: rect.x,
                y: rect.y,
                w,
                h: rect.h,
            },
            Rect {
                x: rect.x + w,
                y: rect.y,
                w: rect.w - w,
                h: rect.h,
            },
        )
    } else {
        let h = rect.h * frac;
        (
            Rect {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h,
            },
            Rect {
                x: rect.x,
                y: rect.y + h,
                w: rect.w,
                h: rect.h - h,
            },
        )
    };
    binary_split(values, &idx[..split], a, out);
    binary_split(values, &idx[split..], b, out);
}

fn tile(values: &[f64], rect: Rect, mode: &str, depth: usize) -> Vec<Rect> {
    // Non-positive values get zero-area rectangles and are never drawn.
    let live: Vec<usize> = (0..values.len()).filter(|&i| values[i] > 0.0).collect();
    let live_vals: Vec<f64> = live.iter().map(|&i| values[i]).collect();
    let placed = match mode {
        "slice_dice" => slice_dice(&live_vals, rect, depth),
        "binary" => {
            let mut o = vec![
                Rect {
                    x: rect.x,
                    y: rect.y,
                    w: 0.0,
                    h: 0.0
                };
                live_vals.len()
            ];
            let idx: Vec<usize> = (0..live_vals.len()).collect();
            binary_split(&live_vals, &idx, rect, &mut o);
            o
        }
        _ => squarify(&live_vals, rect),
    };
    let mut out = vec![
        Rect {
            x: rect.x,
            y: rect.y,
            w: 0.0,
            h: 0.0
        };
        values.len()
    ];
    for (k, &i) in live.iter().enumerate() {
        out[i] = placed[k];
    }
    out
}

// ---------------------------------------------------------------- tiles

#[derive(Clone, Debug)]
pub struct Tile {
    pub path: String,
    pub name: String,
    pub value: f64,
    pub depth: usize,
    pub is_leaf: bool,
    pub rect: Rect,
    pub color_index: usize,
    pub shade: f64,
    pub header: bool,
}

#[allow(clippy::too_many_arguments)]
fn layout_children(
    node: &Node,
    rect: Rect,
    depth: usize,
    prefix: &str,
    color_index: usize,
    opts: &Options,
    tiles: &mut Vec<Tile>,
) {
    let values: Vec<f64> = node.children.iter().map(|c| c.value).collect();
    let rects = tile(&values, rect, &opts.tiling, depth);
    let siblings = node.children.len();
    for (i, child) in node.children.iter().enumerate() {
        let r = rects[i];
        let ci = if depth == 0 { i } else { color_index };
        let shade = if depth == 0 || siblings <= 1 {
            0.0
        } else {
            0.30 * (i as f64) / ((siblings - 1) as f64)
        };
        let path = if prefix.is_empty() {
            child.name.clone()
        } else {
            format!("{prefix} / {}", child.name)
        };
        if child.is_leaf() {
            tiles.push(Tile {
                path,
                name: child.name.clone(),
                value: child.value,
                depth: depth + 1,
                is_leaf: true,
                rect: r,
                color_index: ci,
                shade,
                header: false,
            });
        } else {
            let pad = (opts.border_width * 0.5 + 2.0).max(2.0);
            let mut inner = Rect {
                x: r.x + pad,
                y: r.y + pad,
                w: (r.w - 2.0 * pad).max(0.0),
                h: (r.h - 2.0 * pad).max(0.0),
            };
            let hh = opts.font_size + 5.0;
            let header = inner.w > 24.0 && inner.h > hh + 12.0;
            if header {
                inner.y += hh;
                inner.h = (inner.h - hh).max(0.0);
            }
            tiles.push(Tile {
                path: path.clone(),
                name: child.name.clone(),
                value: child.value,
                depth: depth + 1,
                is_leaf: false,
                rect: r,
                color_index: ci,
                shade,
                header,
            });
            layout_children(child, inner, depth + 1, &path, ci, opts, tiles);
        }
    }
}

// ---------------------------------------------------------------- colors

fn palette_of(name: &str) -> &'static [&'static str] {
    match name {
        "pastel" => &[
            "#a5b4fc", "#bae6fd", "#99f6e4", "#bbf7d0", "#fef08a", "#fed7aa", "#fecaca", "#fbcfe8",
            "#e9d5ff", "#c7d2fe",
        ],
        "dusk" => &[
            "#1e3a8a", "#075985", "#115e59", "#14532d", "#713f12", "#7c2d12", "#7f1d1d", "#831843",
            "#581c87", "#312e81",
        ],
        "earth" => &[
            "#8c6d46", "#a9884f", "#6f7f4b", "#4f6b52", "#93704a", "#b08968", "#7d6b57", "#5c5343",
            "#9c6f44", "#66584a",
        ],
        "ocean" => &[
            "#0c4a6e", "#0369a1", "#0284c7", "#0891b2", "#06b6d4", "#14b8a6", "#2dd4bf", "#38bdf8",
            "#1d4ed8", "#3b82f6",
        ],
        _ => &[
            "#2563eb", "#0ea5e9", "#14b8a6", "#22c55e", "#eab308", "#f97316", "#ef4444", "#ec4899",
            "#a855f7", "#6366f1",
        ],
    }
}

fn parse_hex(s: &str) -> Option<(f64, f64, f64)> {
    let t = s.trim().trim_start_matches('#');
    let bytes: Vec<char> = t.chars().collect();
    let hex = |c: char| c.to_digit(16).map(|d| d as f64);
    match bytes.len() {
        3 => Some((
            hex(bytes[0])? * 17.0,
            hex(bytes[1])? * 17.0,
            hex(bytes[2])? * 17.0,
        )),
        6 => Some((
            hex(bytes[0])? * 16.0 + hex(bytes[1])?,
            hex(bytes[2])? * 16.0 + hex(bytes[3])?,
            hex(bytes[4])? * 16.0 + hex(bytes[5])?,
        )),
        _ => None,
    }
}

fn to_hex(r: f64, g: f64, b: f64) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8
    )
}

/// Mix `base` toward white by `t` (0..1). Returns the input unchanged when it is not hex.
fn lighten(base: &str, t: f64) -> String {
    match parse_hex(base) {
        Some((r, g, b)) if t > 0.0 => to_hex(
            r + (255.0 - r) * t,
            g + (255.0 - g) * t,
            b + (255.0 - b) * t,
        ),
        _ => base.trim().to_string(),
    }
}

fn contrast_text(fill: &str, fallback: &str) -> String {
    match parse_hex(fill) {
        Some((r, g, b)) => {
            let lum = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0;
            if lum > 0.62 { "#111827" } else { "#ffffff" }.to_string()
        }
        None => fallback.to_string(),
    }
}

// ---------------------------------------------------------------- formatting

pub fn fmt_value(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    if (v - v.round()).abs() < 1e-9 && v.abs() < 1e15 {
        let n = v.round() as i64;
        let neg = n < 0;
        let digits = n.abs().to_string();
        let mut grouped = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                grouped.push(',');
            }
            grouped.push(c);
        }
        if neg {
            format!("-{grouped}")
        } else {
            grouped
        }
    } else {
        let s = format!("{v:.2}");
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        if s.is_empty() || s == "-" {
            "0".into()
        } else {
            s
        }
    }
}

fn fmt_json_num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    if (v - v.round()).abs() < 1e-9 && v.abs() < 1e15 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn fmt_pct(share: f64) -> String {
    format!("{:.1}%", share * 100.0)
}

fn esc_xml(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

fn esc_json(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn truncate_to(text: &str, max_chars: usize) -> Option<String> {
    if max_chars < 2 {
        return None;
    }
    let n = text.chars().count();
    if n <= max_chars {
        return Some(text.to_string());
    }
    let keep = max_chars.saturating_sub(1);
    let mut s: String = text.chars().take(keep).collect();
    s.push('\u{2026}');
    Some(s)
}

// ---------------------------------------------------------------- render

struct Theme {
    bg: String,
    text: String,
    muted: String,
    stroke: String,
}

fn theme_of(opts: &Options) -> Theme {
    let dark = opts.theme.trim() == "dark";
    let bg = if opts.background.trim().is_empty() {
        if dark { "#0f172a" } else { "#ffffff" }.to_string()
    } else {
        opts.background.trim().to_string()
    };
    Theme {
        bg,
        text: if dark { "#e2e8f0" } else { "#0f172a" }.into(),
        muted: if dark { "#94a3b8" } else { "#475569" }.into(),
        stroke: if dark { "#0f172a" } else { "#ffffff" }.into(),
    }
}

fn tile_fill(t: &Tile, opts: &Options, leaf_rank: f64) -> String {
    if opts.palette.trim() == "mono" {
        let base = if opts.color.trim().is_empty() {
            "#2563eb"
        } else {
            opts.color.trim()
        };
        lighten(base, 0.55 * leaf_rank)
    } else {
        let pal = palette_of(opts.palette.trim());
        lighten(pal[t.color_index % pal.len()], t.shade)
    }
}

pub fn render(data: &str, opts: &Options) -> Result<String, String> {
    let (rows, layout) = parse_rows(data, opts)?;
    let mut root = build_tree(&rows);
    normalize(&mut root);
    apply_top_n(&mut root, opts.top_n as usize);
    sort_children(&mut root, opts.sort.trim());

    let mut leaves = 0usize;
    let mut groups = 0usize;
    let mut depth = 0usize;
    count_nodes(&root, 0, &mut leaves, &mut groups, &mut depth);
    if leaves > MAX_TILES {
        return Err(format!(
            "too many tiles: {leaves} leaves exceed the {MAX_TILES} tile cap — raise `top_n` grouping or lower `max_depth`"
        ));
    }
    let total = root.value;
    if total <= 0.0 {
        return Err("all values are zero: a treemap needs at least one positive value".into());
    }

    let width = (opts.width.max(1) as f64).clamp(320.0, 2400.0);
    let height = (opts.height.max(1) as f64).clamp(240.0, 1800.0);
    let font = opts.font_size.clamp(6.0, 48.0);
    let border = opts.border_width.clamp(0.0, 12.0);
    let radius = opts.corner_radius.clamp(0.0, 24.0);

    let margin = 10.0;
    let title = opts.title.trim().to_string();
    let title_size = (font * 1.45).max(12.0);
    let mut top = margin;
    if !title.is_empty() {
        top += title_size + 8.0;
    }
    let mut bottom = margin;
    let legend_h = font + 14.0;
    if opts.legend {
        bottom += legend_h;
    }
    let plot = Rect {
        x: margin,
        y: top,
        w: width - 2.0 * margin,
        h: height - top - bottom,
    };
    if plot.w < 40.0 || plot.h < 40.0 {
        return Err(format!(
            "chart area is too small: {}x{} leaves {}x{} for tiles after the title and legend — increase height or width",
            fmt_value(width),
            fmt_value(height),
            fmt_value(plot.w.max(0.0)),
            fmt_value(plot.h.max(0.0))
        ));
    }

    let mut eff = opts.clone();
    eff.font_size = font;
    eff.border_width = border;

    let mut tiles: Vec<Tile> = Vec::new();
    layout_children(&root, plot, 0, "", 0, &eff, &mut tiles);

    match opts.output.trim() {
        "summary" => Ok(render_summary(
            &tiles, total, leaves, groups, depth, &layout, opts,
        )),
        "json" => Ok(render_json(
            &tiles, total, leaves, groups, depth, &layout, opts,
        )),
        "svg" | "" => Ok(render_svg(
            &tiles, total, width, height, plot, font, border, radius, legend_h, &eff,
        )),
        other => Err(format!(
            "unknown output `{other}`: expected svg, summary, or json"
        )),
    }
}

/// Leaf rank in 0..=1 by value (largest = 0), used for the `mono` palette ramp.
fn leaf_ranks(tiles: &[Tile]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..tiles.len()).filter(|&i| tiles[i].is_leaf).collect();
    idx.sort_by(|&a, &b| tiles[b].value.total_cmp(&tiles[a].value).then(a.cmp(&b)));
    let n = idx.len();
    let mut out = vec![0.0; tiles.len()];
    for (rank, &i) in idx.iter().enumerate() {
        out[i] = if n <= 1 {
            0.0
        } else {
            rank as f64 / (n - 1) as f64
        };
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn render_svg(
    tiles: &[Tile],
    total: f64,
    width: f64,
    height: f64,
    plot: Rect,
    font: f64,
    border: f64,
    radius: f64,
    legend_h: f64,
    opts: &Options,
) -> String {
    let th = theme_of(opts);
    let ranks = leaf_ranks(tiles);
    let mut s = String::with_capacity(4096 + tiles.len() * 220);
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" role=\"img\" aria-label=\"Treemap chart\">\n",
        fmt_value(width), fmt_value(height), fmt_value(width), fmt_value(height)
    ));
    s.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
        fmt_value(width),
        fmt_value(height),
        esc_xml(&th.bg)
    ));
    s.push_str(&format!(
        "<g font-family=\"system-ui,-apple-system,Segoe UI,Roboto,Helvetica,Arial,sans-serif\" font-size=\"{}\">\n",
        round2(font)
    ));

    let title = opts.title.trim();
    if !title.is_empty() {
        let ts = (font * 1.45).max(12.0);
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"{}\" font-weight=\"600\" fill=\"{}\">{}</text>\n",
            round2(plot.x),
            round2(plot.y - 8.0),
            round2(ts),
            esc_xml(&th.text),
            esc_xml(title)
        ));
    }

    // Group frames + headers first, leaves on top.
    for t in tiles.iter().filter(|t| !t.is_leaf) {
        if t.rect.w <= 1.0 || t.rect.h <= 1.0 {
            continue;
        }
        let fill = tile_fill(t, opts, 0.0);
        s.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" fill-opacity=\"0.14\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
            round2(t.rect.x), round2(t.rect.y), round2(t.rect.w), round2(t.rect.h),
            round2(radius), esc_xml(&fill), esc_xml(&fill), round2(border.max(1.0))
        ));
        if t.header && opts.show_labels {
            let avail = t.rect.w - 8.0;
            let max_chars = (avail / (font * 0.58)).floor().max(0.0) as usize;
            if let Some(label) = truncate_to(&t.name, max_chars) {
                s.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-weight=\"600\" fill=\"{}\">{}</text>\n",
                    round2(t.rect.x + 4.0),
                    round2(t.rect.y + 3.0 + font),
                    esc_xml(&th.text),
                    esc_xml(&label)
                ));
            }
        }
    }

    for (i, t) in tiles.iter().enumerate() {
        if !t.is_leaf || t.rect.w <= 0.5 || t.rect.h <= 0.5 {
            continue;
        }
        let inset = (border * 0.5).min(t.rect.w / 4.0).min(t.rect.h / 4.0);
        let r = Rect {
            x: t.rect.x + inset,
            y: t.rect.y + inset,
            w: (t.rect.w - 2.0 * inset).max(0.0),
            h: (t.rect.h - 2.0 * inset).max(0.0),
        };
        let fill = tile_fill(t, opts, ranks[i]);
        s.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"><title>{}</title></rect>\n",
            round2(r.x), round2(r.y), round2(r.w), round2(r.h), round2(radius),
            esc_xml(&fill), esc_xml(&th.stroke), round2(border),
            esc_xml(&format!("{}: {} ({})", t.path, fmt_value(t.value), fmt_pct(t.value / total)))
        ));

        let mut lines: Vec<String> = Vec::new();
        if opts.show_labels {
            lines.push(t.name.clone());
        }
        let share = fmt_pct(t.value / total);
        match (opts.show_values, opts.show_percent) {
            (true, true) => lines.push(format!("{} ({})", fmt_value(t.value), share)),
            (true, false) => lines.push(fmt_value(t.value)),
            (false, true) => lines.push(share),
            (false, false) => {}
        }
        if lines.is_empty() {
            continue;
        }

        let pad = 4.0;
        let line_h = font * 1.25;
        let avail_w = r.w - 2.0 * pad;
        let avail_h = r.h - 2.0 * pad;
        let max_lines = (avail_h / line_h).floor().max(0.0) as usize;
        if max_lines == 0 || avail_w <= font * 1.2 {
            continue;
        }
        let shown: Vec<String> = lines.into_iter().take(max_lines).collect();
        let block_h = shown.len() as f64 * line_h;
        let centered = opts.label_position.trim() == "center";
        let first_baseline = match opts.label_position.trim() {
            "center" => r.y + (r.h - block_h) / 2.0 + font,
            "bottom" => r.y + r.h - pad - block_h + font,
            _ => r.y + pad + font,
        };
        let fill_text = contrast_text(&fill, &th.text);
        let max_chars = (avail_w / (font * 0.58)).floor().max(0.0) as usize;
        for (li, line) in shown.iter().enumerate() {
            let Some(txt) = truncate_to(line, max_chars) else {
                continue;
            };
            let (x, anchor) = if centered {
                (r.x + r.w / 2.0, " text-anchor=\"middle\"")
            } else {
                (r.x + pad, "")
            };
            let weight = if li == 0 && opts.show_labels {
                " font-weight=\"600\""
            } else {
                ""
            };
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\"{}{} fill=\"{}\">{}</text>\n",
                round2(x),
                round2(first_baseline + li as f64 * line_h),
                anchor,
                weight,
                esc_xml(&fill_text),
                esc_xml(&txt)
            ));
        }
    }

    if opts.legend {
        let sw = font * 0.85;
        let y = height - legend_h + (legend_h - sw) / 2.0;
        let mut x = plot.x;
        for t in tiles.iter().filter(|t| t.depth == 1) {
            let label = format!("{} ({})", t.name, fmt_pct(t.value / total));
            let w = sw + 6.0 + label.chars().count() as f64 * font * 0.55 + 14.0;
            if x + w > plot.x + plot.w {
                break;
            }
            let fill = tile_fill(t, opts, 0.0);
            s.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{}\"/>\n",
                round2(x),
                round2(y),
                round2(sw),
                round2(sw),
                esc_xml(&fill)
            ));
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" fill=\"{}\">{}</text>\n",
                round2(x + sw + 6.0),
                round2(y + sw * 0.85),
                esc_xml(&th.muted),
                esc_xml(&label)
            ));
            x += w;
        }
    }

    s.push_str("</g>\n</svg>");
    s
}

fn round2(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    let s = format!("{r:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn render_summary(
    tiles: &[Tile],
    total: f64,
    leaves: usize,
    groups: usize,
    depth: usize,
    layout: &str,
    opts: &Options,
) -> String {
    let headers = ["Path", "Value", "Share", "Depth", "Kind"];
    let rows: Vec<[String; 5]> = tiles
        .iter()
        .map(|t| {
            [
                t.path.clone(),
                fmt_value(t.value),
                fmt_pct(t.value / total),
                t.depth.to_string(),
                if t.is_leaf { "tile" } else { "group" }.to_string(),
            ]
        })
        .collect();
    let mut w = [0usize; 5];
    for (i, h) in headers.iter().enumerate() {
        w[i] = h.chars().count();
    }
    for r in &rows {
        for i in 0..5 {
            w[i] = w[i].max(r[i].chars().count());
        }
    }
    let pad_left = |s: &str, n: usize| format!("{}{}", " ".repeat(n - s.chars().count()), s);
    let pad_right = |s: &str, n: usize| format!("{}{}", s, " ".repeat(n - s.chars().count()));

    let mut out = String::new();
    out.push_str(&format!(
        "{}  {}  {}  {}  {}\n",
        pad_right(headers[0], w[0]),
        pad_left(headers[1], w[1]),
        pad_left(headers[2], w[2]),
        pad_left(headers[3], w[3]),
        pad_right(headers[4], w[4])
    ));
    out.push_str(&format!(
        "{}  {}  {}  {}  {}\n",
        "-".repeat(w[0]),
        "-".repeat(w[1]),
        "-".repeat(w[2]),
        "-".repeat(w[3]),
        "-".repeat(w[4])
    ));
    for r in &rows {
        out.push_str(&format!(
            "{}  {}  {}  {}  {}\n",
            pad_right(&r[0], w[0]),
            pad_left(&r[1], w[1]),
            pad_left(&r[2], w[2]),
            pad_left(&r[3], w[3]),
            pad_right(&r[4], w[4])
        ));
    }
    out.push_str(&format!(
        "\ntotal: {}\ntiles: {}\ngroups: {}\ndepth: {}\nlayout: {}\nsort: {}\ntiling: {}",
        fmt_value(total),
        leaves,
        groups,
        depth,
        layout,
        opts.sort.trim(),
        opts.tiling.trim()
    ));
    out
}

fn render_json(
    tiles: &[Tile],
    total: f64,
    leaves: usize,
    groups: usize,
    depth: usize,
    layout: &str,
    opts: &Options,
) -> String {
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"total\": {},\n", fmt_json_num(total)));
    s.push_str(&format!("  \"tiles\": {leaves},\n"));
    s.push_str(&format!("  \"groups\": {groups},\n"));
    s.push_str(&format!("  \"depth\": {depth},\n"));
    s.push_str(&format!("  \"layout\": \"{}\",\n", esc_json(layout)));
    s.push_str(&format!(
        "  \"sort\": \"{}\",\n",
        esc_json(opts.sort.trim())
    ));
    s.push_str(&format!(
        "  \"tiling\": \"{}\",\n",
        esc_json(opts.tiling.trim())
    ));
    s.push_str("  \"nodes\": [\n");
    for (i, t) in tiles.iter().enumerate() {
        s.push_str(&format!(
            "    {{ \"path\": \"{}\", \"name\": \"{}\", \"value\": {}, \"share\": {}, \"depth\": {}, \"kind\": \"{}\", \"x\": {}, \"y\": {}, \"width\": {}, \"height\": {} }}{}\n",
            esc_json(&t.path),
            esc_json(&t.name),
            fmt_json_num(t.value),
            fmt_json_num((t.value / total * 1e6).round() / 1e6),
            t.depth,
            if t.is_leaf { "tile" } else { "group" },
            round2(t.rect.x),
            round2(t.rect.y),
            round2(t.rect.w),
            round2(t.rect.h),
            if i + 1 == tiles.len() { "" } else { "," }
        ));
    }
    s.push_str("  ]\n}");
    s
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn flat_list_renders_svg_with_labels_and_values() {
        let svg = render("Apple,50\nBanana,30\nCherry,20", &opts()).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains(">Apple<"));
        assert!(svg.contains(">50<"));
        assert!(svg.contains("#2563eb"));
    }

    #[test]
    fn tiles_fill_the_plot_area_exactly() {
        let mut o = opts();
        o.output = "json".into();
        let json = render("A,50\nB,30\nC,20", &o).unwrap();
        // plot = 10,10 .. 790x480 for the default 800x500 canvas with no title/legend
        assert!(json.contains("\"total\": 100"));
        assert!(json.contains("\"tiles\": 3"));
        let area: f64 = json
            .lines()
            .filter(|l| l.contains("\"kind\": \"tile\""))
            .map(|l| {
                let get = |k: &str| -> f64 {
                    let at = l.find(k).unwrap() + k.len();
                    let rest = &l[at..];
                    let end = rest.find([',', ' ', '}']).unwrap_or(rest.len());
                    rest[..end].trim().parse().unwrap()
                };
                get("\"width\": ") * get("\"height\": ")
            })
            .sum();
        assert!((area - 780.0 * 480.0).abs() < 5.0, "tiles cover the plot: {area}");
    }

    #[test]
    fn grouped_table_builds_a_two_level_tree() {
        let mut o = opts();
        o.output = "summary".into();
        let out = render("region,city,people\nEU,Paris,10\nEU,Rome,6\nUS,Austin,4", &o).unwrap();
        assert!(out.contains("EU / Paris"));
        assert!(out.contains("groups: 2"));
        assert!(out.contains("tiles: 3"));
        assert!(out.contains("depth: 2"));
        assert!(out.contains("layout: grouped"));
    }

    #[test]
    fn path_layout_splits_on_the_separator() {
        let mut o = opts();
        o.output = "summary".into();
        o.layout = "path".into();
        let out = render("src/app.rs,120\nsrc/lib.rs,80\ndocs/readme.md,40", &o).unwrap();
        assert!(out.contains("src / app.rs"));
        assert!(out.contains("docs / readme.md"));
        assert!(out.contains("layout: path"));
    }

    #[test]
    fn header_row_is_skipped_and_values_are_aggregated() {
        let mut o = opts();
        o.output = "summary".into();
        let out = render("label,value\nA,10\nA,15\nB,5", &o).unwrap();
        assert!(out.contains("total: 30"));
        assert!(out.contains("tiles: 2"));
        assert!(!out.contains("label"));
    }

    #[test]
    fn top_n_folds_the_tail_into_other() {
        let mut o = opts();
        o.output = "summary".into();
        o.top_n = 2;
        let out = render("A,50\nB,30\nC,15\nD,5", &o).unwrap();
        assert!(out.contains("Other"));
        assert!(out.contains("tiles: 3"));
        assert!(out.contains("total: 100"));
    }

    #[test]
    fn max_depth_collapses_deeper_levels() {
        let mut o = opts();
        o.output = "summary".into();
        o.layout = "path".into();
        o.max_depth = 1;
        let out = render("a/b/c,10\na/x/y,20\nz/q,5", &o).unwrap();
        assert!(out.contains("depth: 1"));
        assert!(out.contains("tiles: 2"));
        assert!(!out.contains(" / "));
    }

    #[test]
    fn sort_modes_change_the_first_tile() {
        let mut o = opts();
        o.output = "summary".into();
        o.sort = "value_asc".into();
        let out = render("Big,90\nSmall,10", &o).unwrap();
        let first = out.lines().nth(2).unwrap();
        assert!(first.starts_with("Small"), "got {first}");
        o.sort = "label".into();
        let out = render("Zeta,1\nAlpha,2", &o).unwrap();
        assert!(out.lines().nth(2).unwrap().starts_with("Alpha"));
    }

    #[test]
    fn every_tiling_mode_covers_the_area() {
        for mode in ["squarified", "slice_dice", "binary"] {
            let mut o = opts();
            o.tiling = mode.into();
            let svg = render("A,40\nB,30\nC,20\nD,10", &o).unwrap();
            assert!(svg.contains("<rect"), "{mode}");
            assert!(svg.contains(">A<"), "{mode}");
            assert!(svg.contains(">D<"), "{mode}");
        }
    }

    #[test]
    fn percent_and_center_labels_render() {
        let mut o = opts();
        o.show_percent = true;
        o.show_values = false;
        o.label_position = "center".into();
        let svg = render("Alpha,25\nBeta,75", &o).unwrap();
        assert!(svg.contains("75.0%"));
        assert!(svg.contains("text-anchor=\"middle\""));
    }

    #[test]
    fn mono_palette_shades_by_value_rank() {
        let mut o = opts();
        o.palette = "mono".into();
        o.color = "#000000".into();
        let svg = render("A,50\nB,30\nC,20", &o).unwrap();
        assert!(svg.contains("fill=\"#000000\""), "largest tile keeps the base");
        assert!(svg.contains("#8c8c8c"), "smallest tile is mixed 55% to white");
    }

    #[test]
    fn dark_theme_and_legend_render() {
        let mut o = opts();
        o.theme = "dark".into();
        o.legend = true;
        o.title = "Storage".into();
        let svg = render("A,50\nB,50", &o).unwrap();
        assert!(svg.contains("fill=\"#0f172a\""));
        assert!(svg.contains(">Storage<"));
        assert!(svg.contains(">A (50.0%)<"));
    }

    #[test]
    fn error_on_empty_input() {
        let err = render("   \n\n", &opts()).unwrap_err();
        assert!(err.contains("data is empty"), "{err}");
    }

    #[test]
    fn error_on_non_numeric_value() {
        let err = render("Apple,50\nBanana,lots", &opts()).unwrap_err();
        assert_eq!(
            err,
            "line 2: expected a number for the value of `Banana`, got `lots`"
        );
    }

    #[test]
    fn error_on_negative_value() {
        let err = render("Apple,50\nBanana,-3", &opts()).unwrap_err();
        assert!(err.contains("line 2"), "{err}");
        assert!(err.contains("values of 0 or more"), "{err}");
    }

    #[test]
    fn error_on_single_field_row() {
        let err = render("Apple,50\nBanana", &opts()).unwrap_err();
        assert!(err.contains("only one field"), "{err}");
    }

    #[test]
    fn error_when_every_value_is_zero() {
        let err = render("A,0\nB,0", &opts()).unwrap_err();
        assert!(err.contains("at least one positive value"), "{err}");
    }

    #[test]
    fn error_on_unknown_output() {
        let mut o = opts();
        o.output = "png".into();
        let err = render("A,1", &o).unwrap_err();
        assert!(err.contains("expected svg, summary, or json"), "{err}");
    }

    #[test]
    fn currency_and_thousands_separators_parse() {
        let mut o = opts();
        o.output = "summary".into();
        let out = render("Rent\t$1,250\nFood\t420", &o).unwrap();
        assert!(out.contains("1,250"));
        assert!(out.contains("total: 1,670"));
    }

    #[test]
    fn direct_value_on_a_parent_becomes_its_own_tile() {
        let mut o = opts();
        o.output = "summary".into();
        o.layout = "path".into();
        let out = render("a,10\na/b,30", &o).unwrap();
        assert!(out.contains("a / (direct)"));
        assert!(out.contains("total: 40"));
    }

    #[test]
    fn xml_special_characters_are_escaped() {
        let svg = render("A&B <x>,10\nPlain,5", &opts()).unwrap();
        assert!(svg.contains("A&amp;B &lt;x&gt;"));
        assert!(!svg.contains("<x>"));
    }
}
