//! gizza-ai/correlation-heatmap core — pure-Rust correlation matrix + SVG
//! heatmap. No deps. Parses rows of numbers (each column = a variable, each row
//! = an observation), computes a pairwise Pearson or Spearman correlation
//! matrix, and renders a diverging-color SVG heatmap with cell values + labels.

#[derive(Clone, Copy, PartialEq)]
pub enum Method { Pearson, Spearman }

impl Method {
    pub fn parse(s: &str) -> Result<Method, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "pearson" => Ok(Method::Pearson),
            "spearman" => Ok(Method::Spearman),
            other => Err(format!("method must be pearson or spearman (got '{other}')")),
        }
    }
}

/// Parse rows of comma/space/tab-separated numbers into columns (variables).
/// All rows must have the same width. Returns `columns[var][obs]`.
fn parse_columns(data: &str) -> Result<Vec<Vec<f64>>, String> {
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for (li, line) in data.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut row = Vec::new();
        for tok in line.split([',', ' ', '\t']).filter(|t| !t.trim().is_empty()) {
            let v: f64 = tok.trim().parse().map_err(|_| format!("row {}: '{tok}' is not a number", li + 1))?;
            if !v.is_finite() {
                return Err(format!("row {}: non-finite value", li + 1));
            }
            row.push(v);
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    if rows.len() < 2 {
        return Err("need at least 2 rows (observations)".into());
    }
    let ncol = rows[0].len();
    if ncol < 2 {
        return Err("need at least 2 columns (variables) per row".into());
    }
    if rows.iter().any(|r| r.len() != ncol) {
        return Err("all rows must have the same number of columns".into());
    }
    let mut cols = vec![Vec::with_capacity(rows.len()); ncol];
    for r in &rows {
        for (j, &v) in r.iter().enumerate() {
            cols[j].push(v);
        }
    }
    Ok(cols)
}

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let mut cov = 0.0;
    let mut vx = 0.0;
    let mut vy = 0.0;
    for i in 0..x.len() {
        let dx = x[i] - mx;
        let dy = y[i] - my;
        cov += dx * dy;
        vx += dx * dx;
        vy += dy * dy;
    }
    if vx == 0.0 || vy == 0.0 {
        return 0.0; // a constant column has undefined correlation; report 0
    }
    (cov / (vx.sqrt() * vy.sqrt())).clamp(-1.0, 1.0)
}

/// Fractional ranks (average ranks for ties) — for Spearman.
fn ranks(x: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..x.len()).collect();
    idx.sort_by(|&a, &b| x[a].partial_cmp(&x[b]).unwrap());
    let mut r = vec![0.0; x.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && x[idx[j + 1]] == x[idx[i]] {
            j += 1;
        }
        let avg = ((i + j) as f64) / 2.0 + 1.0; // 1-based average rank
        for k in i..=j {
            r[idx[k]] = avg;
        }
        i = j + 1;
    }
    r
}

/// Compute the NxN correlation matrix for the given columns.
pub fn correlation_matrix(cols: &[Vec<f64>], method: Method) -> Vec<Vec<f64>> {
    let n = cols.len();
    let ranked: Vec<Vec<f64>> = if method == Method::Spearman {
        cols.iter().map(|c| ranks(c)).collect()
    } else {
        cols.to_vec()
    };
    let mut m = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            m[i][j] = if i == j { 1.0 } else { pearson(&ranked[i], &ranked[j]) };
        }
    }
    m
}

/// Diverging color for a value in [-1, 1]: blue (negative) → white (0) → red (+).
fn cell_color(v: f64) -> String {
    let t = v.clamp(-1.0, 1.0);
    let (r, g, b) = if t >= 0.0 {
        (255.0, 255.0 * (1.0 - t), 255.0 * (1.0 - t))
    } else {
        (255.0 * (1.0 + t), 255.0 * (1.0 + t), 255.0)
    };
    format!("#{:02x}{:02x}{:02x}", r as u8, g as u8, b as u8)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Render the full heatmap SVG from raw `data`. `labels_csv` optionally names the
/// columns (defaults to v1..vN). `title` optional.
pub fn render_svg(data: &str, method: Method, labels_csv: &str, title: &str) -> Result<String, String> {
    let cols = parse_columns(data)?;
    let n = cols.len();
    let m = correlation_matrix(&cols, method);

    let labels: Vec<String> = {
        let given: Vec<String> = labels_csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if given.len() == n { given } else { (1..=n).map(|i| format!("v{i}")).collect() }
    };

    let cell = 56.0_f64;
    let label_pad = 70.0_f64;
    let top = if title.is_empty() { 24.0 } else { 48.0 };
    let w = label_pad + n as f64 * cell + 20.0;
    let h = top + label_pad + n as f64 * cell + 10.0;
    let grid_x = label_pad;
    let grid_y = top + 4.0;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="sans-serif"><rect width="{w}" height="{h}" fill="#ffffff"/>"##
    ));
    if !title.is_empty() {
        svg.push_str(&format!(
            r##"<text x="{x}" y="28" text-anchor="middle" font-size="16" font-weight="bold" fill="#111">{t}</text>"##,
            x = w / 2.0, t = esc(title)
        ));
    }
    for i in 0..n {
        for j in 0..n {
            let x = grid_x + j as f64 * cell;
            let y = grid_y + i as f64 * cell;
            let v = m[i][j];
            let txt_fill = if v.abs() > 0.6 { "#ffffff" } else { "#222222" };
            svg.push_str(&format!(
                r##"<rect x="{x}" y="{y}" width="{cell}" height="{cell}" fill="{c}" stroke="#ffffff"/><text x="{tx}" y="{ty}" text-anchor="middle" font-size="12" fill="{txt_fill}">{val:.2}</text>"##,
                c = cell_color(v), tx = x + cell / 2.0, ty = y + cell / 2.0 + 4.0, val = v
            ));
        }
        // row label (left) + column label (top, rotated)
        let ly = grid_y + i as f64 * cell + cell / 2.0 + 4.0;
        svg.push_str(&format!(
            r##"<text x="{lx}" y="{ly}" text-anchor="end" font-size="12" fill="#333">{lab}</text>"##,
            lx = grid_x - 8.0, lab = esc(&labels[i])
        ));
        let cx = grid_x + i as f64 * cell + cell / 2.0;
        let cy = grid_y - 8.0;
        svg.push_str(&format!(
            r##"<text x="{cx}" y="{cy}" text-anchor="start" font-size="12" fill="#333" transform="rotate(-45 {cx} {cy})">{lab}</text>"##,
            lab = esc(&labels[i])
        ));
    }
    svg.push_str("</svg>");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_positive_correlation() {
        let cols = vec![vec![1.0, 2.0, 3.0, 4.0], vec![2.0, 4.0, 6.0, 8.0]];
        let m = correlation_matrix(&cols, Method::Pearson);
        assert!((m[0][1] - 1.0).abs() < 1e-9, "got {}", m[0][1]);
        assert!((m[0][0] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn perfect_negative_correlation() {
        let cols = vec![vec![1.0, 2.0, 3.0, 4.0], vec![4.0, 3.0, 2.0, 1.0]];
        let m = correlation_matrix(&cols, Method::Pearson);
        assert!((m[0][1] + 1.0).abs() < 1e-9, "got {}", m[0][1]);
    }

    #[test]
    fn spearman_handles_monotonic_nonlinear() {
        // y = x^3 is monotonic → Spearman = 1 even though Pearson < 1
        let x: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y: Vec<f64> = x.iter().map(|&v| v.powi(3)).collect();
        let cols = vec![x, y];
        let sp = correlation_matrix(&cols, Method::Spearman);
        assert!((sp[0][1] - 1.0).abs() < 1e-9, "spearman got {}", sp[0][1]);
    }

    #[test]
    fn renders_svg_with_cells() {
        let data = "1,2\n2,4\n3,6\n4,8";
        let svg = render_svg(data, Method::Pearson, "a,b", "Corr").unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("1.00")); // diagonal
        assert!(svg.contains("Corr"));
        assert!(svg.contains(">a<") && svg.contains(">b<"));
    }

    #[test]
    fn cell_color_endpoints() {
        assert_eq!(cell_color(1.0), "#ff0000");
        assert_eq!(cell_color(-1.0), "#0000ff");
        assert_eq!(cell_color(0.0), "#ffffff");
    }

    #[test]
    fn parse_errors() {
        assert!(render_svg("1,2", Method::Pearson, "", "").is_err()); // <2 rows
        assert!(render_svg("1\n2\n3", Method::Pearson, "", "").is_err()); // <2 cols
        assert!(render_svg("1,2\n3,x", Method::Pearson, "", "").is_err()); // non-numeric
        assert!(render_svg("1,2\n3,4,5", Method::Pearson, "", "").is_err()); // ragged
    }
}
