//! gizza-ai/risk-matrix core — plot likelihood-vs-impact items onto a colored
//! risk-matrix heatmap (the classic green/amber/red probability-impact grid used
//! in project / security / safety risk registers). Pure SVG string building, no
//! deps. Each cell is shaded by its risk score (likelihood × impact) relative to
//! the grid maximum; items are placed as numbered markers in their cell with a
//! legend listing name, L×I = score and the risk band. Unlike heatmap-chart (an
//! arbitrary numeric grid), the axes here are an ordinal likelihood scale and an
//! ordinal impact scale and the coloring encodes a risk band, not a raw value.

/// One risk item: a name plus integer likelihood and impact ratings (1..=size).
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub name: String,
    pub likelihood: usize,
    pub impact: usize,
}

/// Risk band of a cell, by its score relative to the grid maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    Low,
    Medium,
    High,
}

impl Band {
    pub fn label(self) -> &'static str {
        match self {
            Band::Low => "Low",
            Band::Medium => "Medium",
            Band::High => "High",
        }
    }
    /// Pale zone fill (cell background).
    fn zone_fill(self) -> &'static str {
        match self {
            Band::Low => "#c8e6c9",
            Band::Medium => "#ffe0b2",
            Band::High => "#ffcdd2",
        }
    }
    /// Strong marker / accent color.
    fn accent(self) -> &'static str {
        match self {
            Band::Low => "#2e7d32",
            Band::Medium => "#ef6c00",
            Band::High => "#b71c1c",
        }
    }
}

/// Classify a cell's risk score (likelihood × impact) into a band. `amber_at` and
/// `red_at` are fractions of the maximum score (size×size): score/max ≤ amber_at →
/// Low, ≤ red_at → Medium, else High.
pub fn band_for(score: usize, max_score: usize, amber_at: f64, red_at: f64) -> Band {
    let ratio = if max_score == 0 {
        0.0
    } else {
        score as f64 / max_score as f64
    };
    if ratio <= amber_at {
        Band::Low
    } else if ratio <= red_at {
        Band::Medium
    } else {
        Band::High
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Parse one item per line: `name, likelihood, impact`. The last two
/// comma-separated fields are the integer ratings; everything before them is the
/// name (so a name may itself contain commas).
pub fn parse_items(input: &str, size: usize) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    for (li, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            return Err(format!(
                "line {}: expected 'name, likelihood, impact'",
                li + 1
            ));
        }
        let n = parts.len();
        let impact = parse_rating(parts[n - 1], size, li + 1, "impact")?;
        let likelihood = parse_rating(parts[n - 2], size, li + 1, "likelihood")?;
        let name = parts[..n - 2].join(",").trim().to_string();
        if name.is_empty() {
            return Err(format!("line {}: item name is empty", li + 1));
        }
        items.push(Item {
            name,
            likelihood,
            impact,
        });
    }
    if items.is_empty() {
        return Err("no items found (one 'name, likelihood, impact' per line)".into());
    }
    Ok(items)
}

fn parse_rating(tok: &str, size: usize, line: usize, which: &str) -> Result<usize, String> {
    let v: i64 = tok
        .trim()
        .parse()
        .map_err(|_| format!("line {line}: {which} '{}' is not an integer", tok.trim()))?;
    if v < 1 || v as usize > size {
        return Err(format!("line {line}: {which} {v} out of range 1..={size}"));
    }
    Ok(v as usize)
}

fn labels_or(csv: &str, n: usize) -> Vec<String> {
    let given: Vec<String> = csv
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if given.len() == n {
        given
    } else {
        (1..=n).map(|i| i.to_string()).collect()
    }
}

/// Render the risk matrix as an SVG string. `size` is the grid dimension (NxN);
/// likelihood is the X axis (1..=size, left→right), impact is the Y axis
/// (1..=size, bottom→top, so the High-risk corner is top-right).
#[allow(clippy::too_many_arguments)]
pub fn render_svg(
    items_in: &str,
    size: usize,
    likelihood_labels: &str,
    impact_labels: &str,
    amber_at: f64,
    red_at: f64,
    title: &str,
) -> Result<String, String> {
    if size < 2 || size > 10 {
        return Err("size must be between 2 and 10".into());
    }
    if !(amber_at.is_finite() && red_at.is_finite())
        || amber_at <= 0.0
        || red_at <= amber_at
        || red_at > 1.0
    {
        return Err("require 0 < amber_at < red_at <= 1".into());
    }
    let items = parse_items(items_in, size)?;
    let max_score = size * size;

    let l_labels = labels_or(likelihood_labels, size);
    let i_labels = labels_or(impact_labels, size);

    // Group item indices by cell (likelihood, impact).
    let cell_items = |l: usize, i: usize| -> Vec<usize> {
        items
            .iter()
            .enumerate()
            .filter(|(_, it)| it.likelihood == l && it.impact == i)
            .map(|(k, _)| k)
            .collect::<Vec<_>>()
    };

    let cell = 64.0_f64;
    let grid_x = 120.0_f64; // room for impact axis title + labels
    let top_pad = if title.is_empty() { 18.0 } else { 44.0 };
    let grid_y = top_pad;
    let grid_w = size as f64 * cell;
    let grid_h = size as f64 * cell;
    let bottom_labels = 52.0_f64; // likelihood labels + axis title
    let legend_y = grid_y + grid_h + bottom_labels;
    let legend_line = 19.0_f64;
    let band_key_h = 24.0_f64; // header row holds the Low/Med/High key
    let legend_h = 26.0 + band_key_h + items.len() as f64 * legend_line + 8.0;
    let w = grid_x + grid_w + 30.0;
    let h = legend_y + legend_h;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="sans-serif"><rect width="{w}" height="{h}" fill="#ffffff"/>"##
    ));
    if !title.is_empty() {
        svg.push_str(&format!(
            r##"<text x="{x}" y="28" text-anchor="middle" font-size="17" font-weight="bold" fill="#111">{t}</text>"##,
            x = grid_x + grid_w / 2.0,
            t = esc(title)
        ));
    }

    // Cells: row index r from top (0 = highest impact = size).
    for r in 0..size {
        let impact = size - r;
        for c in 0..size {
            let likelihood = c + 1;
            let score = likelihood * impact;
            let band = band_for(score, max_score, amber_at, red_at);
            let x = grid_x + c as f64 * cell;
            let y = grid_y + r as f64 * cell;
            svg.push_str(&format!(
                r##"<rect x="{x}" y="{y}" width="{cell}" height="{cell}" fill="{f}" stroke="#ffffff" stroke-width="2"/>"##,
                f = band.zone_fill()
            ));
            // Faint score in the cell corner for reference.
            svg.push_str(&format!(
                r##"<text x="{tx}" y="{ty}" font-size="10" fill="#90909a">{score}</text>"##,
                tx = x + 5.0,
                ty = y + 13.0
            ));

            // Place numbered markers for any items in this cell.
            let here = cell_items(likelihood, impact);
            if !here.is_empty() {
                place_markers(&mut svg, &items, &here, x, y, cell, max_score, amber_at, red_at);
            }
        }
    }

    // Impact axis labels (left) + axis title.
    for r in 0..size {
        let impact = size - r;
        let cy = grid_y + r as f64 * cell + cell / 2.0 + 4.0;
        svg.push_str(&format!(
            r##"<text x="{lx}" y="{cy}" text-anchor="end" font-size="12" fill="#333">{l}</text>"##,
            lx = grid_x - 10.0,
            l = esc(&i_labels[impact - 1])
        ));
    }
    svg.push_str(&format!(
        r##"<text x="22" y="{y}" text-anchor="middle" font-size="13" font-weight="bold" fill="#111" transform="rotate(-90 22 {y})">Impact &#8593;</text>"##,
        y = grid_y + grid_h / 2.0
    ));

    // Likelihood axis labels (bottom) + axis title.
    for c in 0..size {
        let cx = grid_x + c as f64 * cell + cell / 2.0;
        svg.push_str(&format!(
            r##"<text x="{cx}" y="{y}" text-anchor="middle" font-size="12" fill="#333">{l}</text>"##,
            y = grid_y + grid_h + 20.0,
            l = esc(&l_labels[c])
        ));
    }
    svg.push_str(&format!(
        r##"<text x="{x}" y="{y}" text-anchor="middle" font-size="13" font-weight="bold" fill="#111">Likelihood &#8594;</text>"##,
        x = grid_x + grid_w / 2.0,
        y = grid_y + grid_h + 44.0
    ));

    // Legend: a Low/Medium/High color key, then the numbered item list with band.
    let lx = grid_x;
    svg.push_str(&format!(
        r##"<text x="{lx}" y="{y}" font-size="13" font-weight="bold" fill="#111">Risk register</text>"##,
        y = legend_y + 14.0
    ));
    let key_y = legend_y + 14.0;
    let mut kx = lx + 110.0;
    for band in [Band::Low, Band::Medium, Band::High] {
        svg.push_str(&format!(
            r##"<rect x="{kx}" y="{ry}" width="14" height="14" fill="{f}" stroke="#999"/><text x="{tx}" y="{ty}" font-size="11" fill="#333">{l}</text>"##,
            ry = key_y - 11.0,
            f = band.zone_fill(),
            tx = kx + 18.0,
            ty = key_y,
            l = band.label()
        ));
        kx += 18.0 + 14.0 + band.label().len() as f64 * 7.5 + 12.0;
    }
    for (k, it) in items.iter().enumerate() {
        let score = it.likelihood * it.impact;
        let band = band_for(score, max_score, amber_at, red_at);
        let yy = legend_y + 26.0 + band_key_h + k as f64 * legend_line + 12.0;
        // marker swatch
        svg.push_str(&format!(
            r##"<circle cx="{cx}" cy="{cy}" r="8" fill="{f}"/><text x="{cx}" y="{ty}" text-anchor="middle" font-size="10" font-weight="bold" fill="#ffffff">{n}</text>"##,
            cx = lx + 9.0,
            cy = yy - 4.0,
            ty = yy - 0.5,
            f = band.accent(),
            n = k + 1
        ));
        svg.push_str(&format!(
            r##"<text x="{tx}" y="{yy}" font-size="12" fill="#222">{name} — L{l}&#215;I{i} = {score} ({b})</text>"##,
            tx = lx + 24.0,
            name = esc(&it.name),
            l = it.likelihood,
            i = it.impact,
            b = band.label()
        ));
    }

    svg.push_str("</svg>");
    Ok(svg)
}

#[allow(clippy::too_many_arguments)]
fn place_markers(
    svg: &mut String,
    items: &[Item],
    here: &[usize],
    x: f64,
    y: f64,
    cell: f64,
    max_score: usize,
    amber_at: f64,
    red_at: f64,
) {
    let n = here.len();
    // Arrange markers in a near-square sub-grid within the cell.
    let cols = (n as f64).sqrt().ceil() as usize;
    let rows = n.div_ceil(cols);
    let r = (cell / (cols.max(rows) as f64 * 2.4)).clamp(7.0, 12.0);
    let pad = 8.0;
    let span_x = cell - 2.0 * pad;
    let span_y = cell - 2.0 * pad;
    for (idx, &k) in here.iter().enumerate() {
        let cc = idx % cols;
        let rr = idx / cols;
        let mx = x + pad
            + if cols == 1 {
                span_x / 2.0
            } else {
                cc as f64 / (cols - 1) as f64 * span_x
            };
        let my = y + pad
            + if rows == 1 {
                span_y / 2.0
            } else {
                rr as f64 / (rows.max(2) - 1) as f64 * span_y
            };
        let it = &items[k];
        let band = band_for(it.likelihood * it.impact, max_score, amber_at, red_at);
        svg.push_str(&format!(
            r##"<circle cx="{mx}" cy="{my}" r="{r}" fill="{f}" stroke="#ffffff" stroke-width="1.5"/><text x="{mx}" y="{ty}" text-anchor="middle" font-size="{fs}" font-weight="bold" fill="#ffffff">{num}</text>"##,
            f = band.accent(),
            ty = my + r * 0.38,
            fs = (r * 1.1).round(),
            num = k + 1
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_items() {
        let items = parse_items("Server outage, 4, 5\nData leak, 2, 5", 5).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "Server outage");
        assert_eq!(items[0].likelihood, 4);
        assert_eq!(items[0].impact, 5);
        assert_eq!(items[1].name, "Data leak");
    }

    #[test]
    fn name_may_contain_commas() {
        let items = parse_items("Loss of power, building A, 3, 2", 5).unwrap();
        assert_eq!(items[0].name, "Loss of power, building A");
        assert_eq!(items[0].likelihood, 3);
        assert_eq!(items[0].impact, 2);
    }

    #[test]
    fn bands_by_threshold() {
        // 5x5, max 25, amber_at 0.25 (<=6.25), red_at 0.5 (<=12.5).
        assert_eq!(band_for(4, 25, 0.25, 0.5), Band::Low); // ratio .16
        assert_eq!(band_for(6, 25, 0.25, 0.5), Band::Low); // .24
        assert_eq!(band_for(9, 25, 0.25, 0.5), Band::Medium); // .36
        assert_eq!(band_for(12, 25, 0.25, 0.5), Band::Medium); // .48
        assert_eq!(band_for(15, 25, 0.25, 0.5), Band::High); // .6
        assert_eq!(band_for(25, 25, 0.25, 0.5), Band::High);
    }

    #[test]
    fn renders_svg_with_legend_and_zones() {
        let svg =
            render_svg("Server outage, 4, 5\nMinor bug, 2, 1", 5, "", "", 0.25, 0.5, "Risks")
                .unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("Risks"));
        assert!(svg.contains("Server outage"));
        assert!(svg.contains("Likelihood"));
        assert!(svg.contains("Impact"));
        // High zone fill present (top-right has score 25)
        assert!(svg.contains("#ffcdd2"));
        // Legend shows the L×I = score line for the outage (20, High)
        assert!(svg.contains("= 20 (High)"));
        assert!(svg.contains("= 2 (Low)"));
        // Band key explains all three zones (>Medium< only appears in the key here)
        assert!(svg.contains(">Low</text>"));
        assert!(svg.contains(">Medium</text>"));
        assert!(svg.contains(">High</text>"));
    }

    #[test]
    fn custom_size_and_labels() {
        let svg = render_svg(
            "A, 1, 1\nB, 3, 3",
            3,
            "Rare,Possible,Likely",
            "Low,Med,Sev",
            0.25,
            0.5,
            "",
        )
        .unwrap();
        assert!(svg.contains(">Rare<") && svg.contains(">Likely<"));
        assert!(svg.contains(">Sev<"));
    }

    #[test]
    fn errors() {
        assert!(render_svg("", 5, "", "", 0.25, 0.5, "").is_err()); // empty
        assert!(render_svg("Foo, 9, 1", 5, "", "", 0.25, 0.5, "").is_err()); // likelihood out of range
        assert!(render_svg("Foo, x, 1", 5, "", "", 0.25, 0.5, "").is_err()); // non-integer
        assert!(render_svg("Foo, 1", 5, "", "", 0.25, 0.5, "").is_err()); // too few fields
        assert!(render_svg("A, 1, 1", 1, "", "", 0.25, 0.5, "").is_err()); // size too small
        assert!(render_svg("A, 1, 1", 5, "", "", 0.6, 0.5, "").is_err()); // amber >= red
    }

    #[test]
    fn stacks_multiple_items_in_one_cell() {
        // Three items in the same cell must each render a marker.
        let svg = render_svg("A, 2, 2\nB, 2, 2\nC, 2, 2", 5, "", "", 0.25, 0.5, "").unwrap();
        let circles = svg.matches("<circle").count();
        // 3 cell markers + 3 legend swatches = 6
        assert!(circles >= 6, "expected >=6 circles, got {circles}");
    }
}
