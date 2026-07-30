//! pie-donut-chart-svg core — turn labeled values into a standalone pie or donut
//! chart, rendered as a self-contained SVG string. Pure-Rust (hand-built SVG, no
//! drawing/plotting deps beyond a tiny JSON reader for the optional JSON-array
//! input), so it runs on every backend including the chat Service Worker. No
//! wafer/wasm-bindgen deps here.

/// Rendering options resolved from the tool params.
pub struct Options {
    /// "pie" | "donut".
    pub chart_type: String,
    /// SVG width in pixels.
    pub width: u32,
    /// SVG height in pixels.
    pub height: u32,
    /// Donut inner-radius ratio (0.0..0.9) of the outer radius. Ignored for a pie.
    pub donut_hole: f64,
    /// Angle in degrees for the first slice's leading edge, measured clockwise
    /// from 12 o'clock. 0 starts at the top.
    pub start_angle: f64,
    /// Comma-separated CSS colours cycled across slices; empty uses the built-in
    /// palette.
    pub colors: String,
    /// Draw each slice's label text on the slice.
    pub show_labels: bool,
    /// Draw each slice's percentage on the slice.
    pub show_percentages: bool,
    /// Include the raw value in the legend rows.
    pub show_values: bool,
    /// Legend placement: "none" | "right" | "bottom".
    pub legend: String,
    /// Slice ordering: "input" | "descending" | "ascending" (by value).
    pub sort: String,
    /// Optional title drawn centered at the top.
    pub title: String,
    /// Background fill: any CSS colour, or "none"/"transparent" for no backdrop.
    pub background: String,
}

/// Built-in 10-colour categorical palette (Tableau-10 style), used when the
/// caller does not pass an explicit `colors` list.
const PALETTE: [&str; 10] = [
    "#4e79a7", "#f28e2b", "#e15759", "#76b7b2", "#59a14f", "#edc948", "#b07aa1",
    "#ff9da7", "#9c755f", "#bab0ac",
];

struct Slice {
    label: String,
    value: f64,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Format a number for a label: integer when whole, else up to 3 decimals.
fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    if (v.round() - v).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Percentage rounded to 1 decimal, trailing zero/point trimmed (e.g. `25`, `12.5`).
fn fmt_pct(p: f64) -> String {
    let r = (p * 10.0).round() / 10.0;
    let s = format!("{r:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Parse the `data` input into labeled slices.
///
/// Accepts a JSON array (`[["A",5],{"label":"B","value":3}]`) when the trimmed
/// input starts with `[`, otherwise one entry per line (also split on `;`) where
/// each entry is `label <sep> value` and the separator is the last `,`, `:` or
/// `=` on the line. Returns a clear error naming any bad entry.
fn parse_data(input: &str) -> Result<Vec<Slice>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("no data — enter at least one `label, value` pair".into());
    }
    let slices = if trimmed.starts_with('[') {
        parse_json(trimmed)?
    } else {
        parse_lines(trimmed)?
    };
    if slices.is_empty() {
        return Err("no data rows found — enter at least one `label, value` pair".into());
    }
    for s in &slices {
        if !s.value.is_finite() {
            return Err(format!("value for '{}' is not a finite number", s.label));
        }
        if s.value < 0.0 {
            return Err(format!(
                "value for '{}' is {} — pie/donut slices cannot be negative",
                s.label,
                fmt_num(s.value)
            ));
        }
    }
    let total: f64 = slices.iter().map(|s| s.value).sum();
    if total <= 0.0 {
        return Err("all values are zero — nothing to chart".into());
    }
    Ok(slices)
}

fn parse_json(input: &str) -> Result<Vec<Slice>, String> {
    let v: serde_json::Value = serde_json::from_str(input)
        .map_err(|e| format!("data looks like JSON but did not parse: {e}"))?;
    let arr = v
        .as_array()
        .ok_or_else(|| "JSON data must be an array of [label, value] pairs or objects".to_string())?;
    let mut out = Vec::new();
    for (i, item) in arr.iter().enumerate() {
        let (label, value) = match item {
            serde_json::Value::Array(pair) => {
                if pair.len() < 2 {
                    return Err(format!("JSON entry {} must be [label, value]", i + 1));
                }
                let label = json_str(&pair[0]);
                let value = json_num(&pair[1])
                    .ok_or_else(|| format!("JSON entry {} has a non-numeric value", i + 1))?;
                (label, value)
            }
            serde_json::Value::Object(map) => {
                let label = map
                    .get("label")
                    .or_else(|| map.get("name"))
                    .or_else(|| map.get("key"))
                    .map(json_str)
                    .unwrap_or_else(|| format!("Item {}", i + 1));
                let raw = map
                    .get("value")
                    .or_else(|| map.get("count"))
                    .or_else(|| map.get("y"))
                    .ok_or_else(|| format!("JSON entry {} is missing a 'value' field", i + 1))?;
                let value = json_num(raw)
                    .ok_or_else(|| format!("JSON entry {} has a non-numeric value", i + 1))?;
                (label, value)
            }
            _ => {
                return Err(format!(
                    "JSON entry {} must be a [label, value] pair or an object",
                    i + 1
                ))
            }
        };
        out.push(Slice { label, value });
    }
    Ok(out)
}

fn json_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string().trim_matches('"').to_string(),
    }
}

fn json_num(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_lines(input: &str) -> Result<Vec<Slice>, String> {
    let mut out = Vec::new();
    for raw in input.split(['\n', ';']) {
        let entry = raw.trim();
        if entry.is_empty() {
            continue;
        }
        let sep = entry
            .char_indices()
            .filter(|(_, c)| matches!(c, ',' | ':' | '='))
            .map(|(i, _)| i)
            .next_back();
        let sep = sep.ok_or_else(|| {
            format!("line '{entry}' has no separator — use `label, value` (or `:`/`=`)")
        })?;
        let label = entry[..sep].trim().to_string();
        let value_str = entry[sep + 1..].trim();
        if label.is_empty() {
            return Err(format!("line '{entry}' is missing a label"));
        }
        let value: f64 = value_str
            .replace(['_', ','], "")
            .parse()
            .map_err(|_| format!("value '{value_str}' for '{label}' is not a number"))?;
        out.push(Slice { label, value });
    }
    Ok(out)
}

/// Resolve the per-slice colour list: the caller's comma-separated colours when
/// given, else the built-in palette.
fn colors_for(colors: &str) -> Vec<String> {
    let custom: Vec<String> = colors
        .split(',')
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect();
    if custom.is_empty() {
        PALETTE.iter().map(|c| c.to_string()).collect()
    } else {
        custom
    }
}

/// Point on a circle of radius `r` at `deg` degrees clockwise from 12 o'clock.
fn point(cx: f64, cy: f64, r: f64, deg: f64) -> (f64, f64) {
    let t = deg.to_radians();
    (cx + r * t.sin(), cy - r * t.cos())
}

fn fmt2(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Path for one pie slice (a wedge from the centre) spanning a0..a1 degrees.
fn pie_slice_path(cx: f64, cy: f64, r: f64, a0: f64, a1: f64) -> String {
    let (x0, y0) = point(cx, cy, r, a0);
    let (x1, y1) = point(cx, cy, r, a1);
    let large = if (a1 - a0) > 180.0 { 1 } else { 0 };
    format!(
        "M{cx},{cy} L{x0},{y0} A{r},{r} 0 {large} 1 {x1},{y1} Z",
        cx = fmt2(cx),
        cy = fmt2(cy),
        x0 = fmt2(x0),
        y0 = fmt2(y0),
        r = fmt2(r),
        x1 = fmt2(x1),
        y1 = fmt2(y1)
    )
}

/// Path for one donut (annular) slice between inner radius `ri` and outer `ro`.
fn donut_slice_path(cx: f64, cy: f64, ri: f64, ro: f64, a0: f64, a1: f64) -> String {
    let (ox0, oy0) = point(cx, cy, ro, a0);
    let (ox1, oy1) = point(cx, cy, ro, a1);
    let (ix1, iy1) = point(cx, cy, ri, a1);
    let (ix0, iy0) = point(cx, cy, ri, a0);
    let large = if (a1 - a0) > 180.0 { 1 } else { 0 };
    format!(
        "M{ox0},{oy0} A{ro},{ro} 0 {large} 1 {ox1},{oy1} L{ix1},{iy1} A{ri},{ri} 0 {large} 0 {ix0},{iy0} Z",
        ox0 = fmt2(ox0), oy0 = fmt2(oy0),
        ro = fmt2(ro), large = large,
        ox1 = fmt2(ox1), oy1 = fmt2(oy1),
        ix1 = fmt2(ix1), iy1 = fmt2(iy1),
        ri = fmt2(ri),
        ix0 = fmt2(ix0), iy0 = fmt2(iy0),
    )
}

/// Render a full ring (a single 100% donut slice) as two half-annulus paths so
/// the arc endpoints never coincide.
fn full_ring(cx: f64, cy: f64, ri: f64, ro: f64, color: &str) -> String {
    let mut s = String::new();
    for (a0, a1) in [(0.0, 180.0), (180.0, 360.0)] {
        s.push_str(&format!(
            "<path d=\"{d}\" fill=\"{c}\" stroke=\"#ffffff\" stroke-width=\"1\"/>\n",
            d = donut_slice_path(cx, cy, ri, ro, a0, a1),
            c = esc(color)
        ));
    }
    s
}

fn legend_text(label: &str, value: f64, pct: f64, show_values: bool, show_percentages: bool) -> String {
    let mut extras = Vec::new();
    if show_values {
        extras.push(fmt_num(value));
    }
    if show_percentages {
        extras.push(format!("{}%", fmt_pct(pct)));
    }
    if extras.is_empty() {
        label.to_string()
    } else {
        format!("{label} ({})", extras.join(", "))
    }
}

/// Render the requested pie/donut chart from labeled values to an SVG string.
pub fn render(data: &str, opts: &Options) -> Result<String, String> {
    let chart = opts.chart_type.trim().to_lowercase();
    if !matches!(chart.as_str(), "pie" | "donut") {
        return Err(format!(
            "chart_type '{}' is not supported — use pie or donut",
            opts.chart_type
        ));
    }
    let legend = opts.legend.trim().to_lowercase();
    if !matches!(legend.as_str(), "none" | "right" | "bottom") {
        return Err(format!(
            "legend '{}' is not supported — use none, right, or bottom",
            opts.legend
        ));
    }
    let sort = opts.sort.trim().to_lowercase();
    if !matches!(sort.as_str(), "input" | "descending" | "ascending") {
        return Err(format!(
            "sort '{}' is not supported — use input, descending, or ascending",
            opts.sort
        ));
    }

    let mut slices = parse_data(data)?;
    match sort.as_str() {
        "descending" => slices.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap()),
        "ascending" => slices.sort_by(|a, b| a.value.partial_cmp(&b.value).unwrap()),
        _ => {}
    }
    let total: f64 = slices.iter().map(|s| s.value).sum();
    let palette = colors_for(&opts.colors);

    let width = opts.width.clamp(120, 4000) as f64;
    let height = opts.height.clamp(120, 4000) as f64;
    let title = opts.title.trim();
    let top_pad = if title.is_empty() { 12.0 } else { 40.0 };

    // Reserve room for the legend, then centre the chart in what's left.
    let legend_right_w = if legend == "right" { legend_right_width(&slices) } else { 0.0 };
    let legend_bottom_h = if legend == "bottom" {
        legend_bottom_height(&slices, width)
    } else {
        0.0
    };
    let area_left = 12.0;
    let area_top = top_pad;
    let area_w = (width - area_left - 12.0 - legend_right_w).max(40.0);
    let area_h = (height - area_top - 12.0 - legend_bottom_h).max(40.0);
    let cx = area_left + area_w / 2.0;
    let cy = area_top + area_h / 2.0;
    let ro = (area_w.min(area_h) / 2.0 * 0.94).max(10.0);
    let ri = if chart == "donut" {
        ro * opts.donut_hole.clamp(0.0, 0.9)
    } else {
        0.0
    };

    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" font-family=\"sans-serif\">\n",
        w = fmt_num(width),
        h = fmt_num(height)
    ));
    let bg = opts.background.trim();
    if !bg.is_empty() && !matches!(bg.to_lowercase().as_str(), "none" | "transparent") {
        s.push_str(&format!(
            "<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"{c}\"/>\n",
            w = fmt_num(width),
            h = fmt_num(height),
            c = esc(bg)
        ));
    }
    if !title.is_empty() {
        s.push_str(&format!(
            "<text x=\"{x}\" y=\"26\" text-anchor=\"middle\" font-size=\"18\" font-weight=\"bold\" fill=\"#222222\">{t}</text>\n",
            x = fmt_num(width / 2.0),
            t = esc(title)
        ));
    }

    // Slices, clockwise from `start_angle`.
    let mut angle = opts.start_angle;
    let full = slices.len() == 1;
    for (i, sl) in slices.iter().enumerate() {
        let frac = sl.value / total;
        let sweep = frac * 360.0;
        let a0 = angle;
        let a1 = angle + sweep;
        angle = a1;
        let color = &palette[i % palette.len()];
        if sl.value <= 0.0 {
            continue;
        }
        if full || sweep >= 359.999 {
            if chart == "donut" {
                s.push_str(&full_ring(cx, cy, ri, ro, color));
            } else {
                s.push_str(&format!(
                    "<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\" fill=\"{c}\"/>\n",
                    cx = fmt2(cx),
                    cy = fmt2(cy),
                    r = fmt2(ro),
                    c = esc(color)
                ));
            }
        } else {
            let d = if chart == "donut" {
                donut_slice_path(cx, cy, ri, ro, a0, a1)
            } else {
                pie_slice_path(cx, cy, ro, a0, a1)
            };
            s.push_str(&format!(
                "<path d=\"{d}\" fill=\"{c}\" stroke=\"#ffffff\" stroke-width=\"1\"/>\n",
                c = esc(color)
            ));
        }

        // On-slice label / percentage, only when the wedge is wide enough to read.
        if (opts.show_labels || opts.show_percentages) && sweep >= 12.0 {
            let mid = (a0 + a1) / 2.0;
            let lr = if chart == "donut" {
                (ri + ro) / 2.0
            } else {
                ro * 0.62
            };
            let (lx, ly) = point(cx, cy, lr, mid);
            let pct = frac * 100.0;
            let mut lines: Vec<String> = Vec::new();
            if opts.show_labels {
                lines.push(esc(&sl.label));
            }
            if opts.show_percentages {
                lines.push(format!("{}%", fmt_pct(pct)));
            }
            let n = lines.len() as f64;
            for (li, line) in lines.iter().enumerate() {
                let dy = (li as f64 - (n - 1.0) / 2.0) * 14.0 + 4.0;
                s.push_str(&format!(
                    "<text x=\"{x}\" y=\"{y}\" text-anchor=\"middle\" font-size=\"12\" fill=\"#ffffff\">{t}</text>\n",
                    x = fmt2(lx),
                    y = fmt2(ly + dy),
                    t = line
                ));
            }
        }
    }

    // Legend.
    if legend == "right" {
        let mut ly = area_top + 4.0;
        let lx = width - legend_right_w + 6.0;
        for (i, sl) in slices.iter().enumerate() {
            let color = &palette[i % palette.len()];
            let pct = sl.value / total * 100.0;
            s.push_str(&legend_row(lx, ly, color, &legend_text(&sl.label, sl.value, pct, opts.show_values, opts.show_percentages)));
            ly += 22.0;
        }
    } else if legend == "bottom" {
        let item_w = legend_item_width(&slices);
        let per_row = (width / item_w).floor().max(1.0) as usize;
        let mut lx = 12.0;
        let mut ly = height - legend_bottom_h + 12.0;
        for (i, sl) in slices.iter().enumerate() {
            if i > 0 && i % per_row == 0 {
                lx = 12.0;
                ly += 22.0;
            }
            let color = &palette[i % palette.len()];
            let pct = sl.value / total * 100.0;
            s.push_str(&legend_row(lx, ly, color, &legend_text(&sl.label, sl.value, pct, opts.show_values, opts.show_percentages)));
            lx += item_w;
        }
    }

    s.push_str("</svg>");
    Ok(s)
}

fn legend_row(x: f64, y: f64, color: &str, text: &str) -> String {
    format!(
        "<rect x=\"{x}\" y=\"{ry}\" width=\"12\" height=\"12\" fill=\"{c}\"/>\n<text x=\"{tx}\" y=\"{ty}\" font-size=\"12\" fill=\"#333333\">{t}</text>\n",
        x = fmt2(x),
        ry = fmt2(y),
        c = esc(color),
        tx = fmt2(x + 18.0),
        ty = fmt2(y + 11.0),
        t = esc(text)
    )
}

/// Estimate a legend item's width from its longest label (~7px/char + swatch).
fn legend_item_width(slices: &[Slice]) -> f64 {
    let max_chars = slices
        .iter()
        .map(|s| s.label.chars().count() + 12)
        .max()
        .unwrap_or(8);
    (24.0 + max_chars as f64 * 7.0).clamp(90.0, 320.0)
}

fn legend_right_width(slices: &[Slice]) -> f64 {
    legend_item_width(slices).min(280.0)
}

fn legend_bottom_height(slices: &[Slice], width: f64) -> f64 {
    let item_w = legend_item_width(slices);
    let per_row = (width / item_w).floor().max(1.0) as usize;
    let rows = slices.len().div_ceil(per_row);
    rows as f64 * 22.0 + 12.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            chart_type: "pie".into(),
            width: 640,
            height: 400,
            donut_hole: 0.55,
            start_angle: 0.0,
            colors: "".into(),
            show_labels: false,
            show_percentages: true,
            show_values: false,
            legend: "right".into(),
            sort: "input".into(),
            title: "".into(),
            background: "#ffffff".into(),
        }
    }

    #[test]
    fn pie_from_lines() {
        let svg = render("Apple, 30\nBanana, 20\nCherry, 50", &opts()).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("<path"));
        assert!(svg.contains("Apple"));
        assert!(svg.contains("#4e79a7")); // first palette colour
        assert!(svg.contains("50%")); // cherry is half
    }

    #[test]
    fn donut_has_inner_cutout_path() {
        let mut o = opts();
        o.chart_type = "donut".into();
        let svg = render("A,1\nB,1\nC,1\nD,1", &o).unwrap();
        // annular slices use an inner arc (two A commands per path).
        assert!(svg.matches(" A").count() >= 8);
        assert!(svg.contains("<path"));
    }

    #[test]
    fn colon_and_equals_separators() {
        let svg = render("Yes: 3\nNo = 1", &opts()).unwrap();
        assert!(svg.contains("Yes"));
        assert!(svg.contains("No"));
    }

    #[test]
    fn semicolon_separated_entries() {
        let svg = render("A,1; B,2; C,3", &opts()).unwrap();
        assert!(svg.contains("<path"));
    }

    #[test]
    fn json_array_of_pairs() {
        let svg = render(r#"[["A", 5], ["B", 15]]"#, &opts()).unwrap();
        assert!(svg.contains("<path"));
    }

    #[test]
    fn json_array_of_objects() {
        let svg = render(r#"[{"label":"A","value":5},{"name":"B","value":15}]"#, &opts()).unwrap();
        assert!(svg.contains("<path"));
    }

    #[test]
    fn custom_colors_cycle() {
        let mut o = opts();
        o.colors = "#111111, #222222".into();
        let svg = render("A,1\nB,1\nC,1", &o).unwrap();
        assert!(svg.contains("#111111"));
        assert!(svg.contains("#222222"));
    }

    #[test]
    fn sort_descending_orders_slices() {
        let mut o = opts();
        o.sort = "descending".into();
        o.legend = "right".into();
        let svg = render("Small,1\nBig,100", &o).unwrap();
        // "Big" legend row should appear before "Small" in output order.
        let big = svg.find("Big").unwrap();
        let small = svg.find("Small").unwrap();
        assert!(big < small);
    }

    #[test]
    fn title_rendered_and_escaped() {
        let mut o = opts();
        o.title = "Share <2024>".into();
        let svg = render("A,1\nB,1", &o).unwrap();
        assert!(svg.contains("Share &lt;2024&gt;"));
    }

    #[test]
    fn show_labels_and_values() {
        let mut o = opts();
        o.show_labels = true;
        o.show_values = true;
        let svg = render("Alpha,40\nBeta,60", &o).unwrap();
        // on-slice label text
        assert!(svg.contains(">Alpha<"));
        // legend carries the raw value
        assert!(svg.contains("Alpha (40"));
    }

    #[test]
    fn legend_bottom_and_none() {
        let mut o = opts();
        o.legend = "bottom".into();
        let svg = render("A,1\nB,2\nC,3", &o).unwrap();
        assert!(svg.contains("<rect")); // swatches
        o.legend = "none".into();
        let svg2 = render("A,1\nB,2\nC,3", &o).unwrap();
        // no legend swatches: only the (optional) background rect, at x="0"
        assert!(!svg2.contains("width=\"12\" height=\"12\""));
    }

    #[test]
    fn single_slice_is_full_circle() {
        let svg = render("Everything, 42", &opts()).unwrap();
        assert!(svg.contains("<circle"));
    }

    #[test]
    fn single_slice_donut_is_full_ring() {
        let mut o = opts();
        o.chart_type = "donut".into();
        let svg = render("Everything, 42", &o).unwrap();
        assert!(svg.contains("<path")); // two half-ring paths
        assert!(!svg.contains("<circle"));
    }

    #[test]
    fn start_angle_and_donut_hole_accepted() {
        let mut o = opts();
        o.chart_type = "donut".into();
        o.start_angle = 90.0;
        o.donut_hole = 0.3;
        let svg = render("A,1\nB,1", &o).unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn transparent_background_skips_rect() {
        let mut o = opts();
        o.background = "transparent".into();
        o.legend = "none".into();
        let svg = render("A,1\nB,1", &o).unwrap();
        assert!(!svg.contains("x=\"0\" y=\"0\""));
    }

    // --- error paths ---

    #[test]
    fn err_empty() {
        let e = render("   ", &opts()).unwrap_err();
        assert!(e.contains("no data"));
    }

    #[test]
    fn err_negative() {
        let e = render("A,5\nB,-3", &opts()).unwrap_err();
        assert!(e.contains("negative"));
    }

    #[test]
    fn err_non_numeric() {
        let e = render("A,five", &opts()).unwrap_err();
        assert!(e.contains("not a number"));
    }

    #[test]
    fn err_all_zero() {
        let e = render("A,0\nB,0", &opts()).unwrap_err();
        assert!(e.contains("all values are zero"));
    }

    #[test]
    fn err_no_separator() {
        let e = render("just a label", &opts()).unwrap_err();
        assert!(e.contains("separator"));
    }

    #[test]
    fn err_unknown_chart_type() {
        let mut o = opts();
        o.chart_type = "bar".into();
        let e = render("A,1", &o).unwrap_err();
        assert!(e.contains("chart_type"));
    }

    #[test]
    fn err_unknown_legend() {
        let mut o = opts();
        o.legend = "left".into();
        let e = render("A,1", &o).unwrap_err();
        assert!(e.contains("legend"));
    }

    #[test]
    fn err_unknown_sort() {
        let mut o = opts();
        o.sort = "random".into();
        let e = render("A,1", &o).unwrap_err();
        assert!(e.contains("sort"));
    }
}
