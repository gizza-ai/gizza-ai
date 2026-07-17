//! csv-chart-generator core — turn chosen columns of a CSV into a bar, line,
//! scatter, or histogram chart, rendered as a standalone SVG string. Pure-Rust
//! (hand-built SVG, no drawing/plotting deps), so it runs on every backend
//! including the chat Service Worker. No wafer/wasm-bindgen deps here.

/// Rendering options resolved from the tool params.
pub struct Options {
    /// One of "bar" | "line" | "scatter" | "histogram".
    pub chart_type: String,
    /// Column for the X axis (bar/line/scatter) or the binned column
    /// (histogram): a header name or a 1-based index.
    pub x_column: String,
    /// Column for the Y axis (bar/line/scatter): a header name or 1-based index.
    /// Ignored for histogram.
    pub y_column: String,
    /// Optional chart title drawn above the plot.
    pub title: String,
    /// SVG width in pixels.
    pub width: u32,
    /// SVG height in pixels.
    pub height: u32,
    /// Series colour (bars/line/points): any CSS colour.
    pub color: String,
    /// Number of buckets for a histogram (2..=100). Ignored otherwise.
    pub bins: u32,
}

/// Parse CSV text into rows of trimmed string cells using the `csv` crate,
/// which handles quoted fields with embedded commas/newlines and doubled-quote
/// (`""`) escapes. Ragged rows are allowed (`flexible`); blank rows are dropped.
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records().flatten() {
        rows.push(rec.iter().map(|c| c.trim().to_string()).collect());
    }
    rows.retain(|r| !r.iter().all(|c| c.is_empty()));
    rows
}

fn is_number(s: &str) -> bool {
    s.trim().parse::<f64>().is_ok()
}

/// A first row that is not entirely numeric is treated as a header.
fn has_header(rows: &[Vec<String>]) -> bool {
    rows.first()
        .map(|r| !r.iter().all(|c| is_number(c)))
        .unwrap_or(false)
}

/// Resolve a column spec (header name, case-insensitive, or 1-based index) to a
/// 0-based column index.
fn resolve_col(spec: &str, header: Option<&Vec<String>>, ncols: usize) -> Result<usize, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("no column specified".into());
    }
    if let Some(h) = header {
        if let Some(i) = h.iter().position(|c| c.eq_ignore_ascii_case(spec)) {
            return Ok(i);
        }
    }
    if let Ok(n) = spec.parse::<usize>() {
        if n >= 1 && n <= ncols {
            return Ok(n - 1);
        }
        return Err(format!(
            "column index {n} is out of range (the CSV has {ncols} column(s))"
        ));
    }
    match header {
        Some(h) => Err(format!(
            "column '{spec}' was not found; available columns are: {}",
            h.iter()
                .map(|c| format!("'{c}'"))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        None => Err(format!(
            "column '{spec}' not found — the CSV has no header row, so use a 1-based index (1..{ncols})"
        )),
    }
}

fn col_name(header: Option<&Vec<String>>, idx: usize) -> String {
    header
        .and_then(|h| h.get(idx).cloned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("Column {}", idx + 1))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Format a number for an axis/label: integer when whole, else up to 3 decimals.
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

struct Frame {
    width: f64,
    height: f64,
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
}

impl Frame {
    fn plot_w(&self) -> f64 {
        (self.width - self.left - self.right).max(20.0)
    }
    fn plot_h(&self) -> f64 {
        (self.height - self.top - self.bottom).max(20.0)
    }
}

fn svg_open(f: &Frame, title: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" font-family=\"sans-serif\">\n",
        w = fmt_num(f.width),
        h = fmt_num(f.height)
    ));
    s.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>\n",
        w = fmt_num(f.width),
        h = fmt_num(f.height)
    ));
    if !title.trim().is_empty() {
        s.push_str(&format!(
            "<text x=\"{x}\" y=\"22\" text-anchor=\"middle\" font-size=\"18\" font-weight=\"bold\" fill=\"#222222\">{t}</text>\n",
            x = fmt_num(f.width / 2.0),
            t = esc(title.trim())
        ));
    }
    s
}

/// Draw the Y axis with `nticks` gridlines/labels between lo..hi.
fn y_axis(f: &Frame, lo: f64, hi: f64, nticks: usize, label: &str, s: &mut String) {
    let sy = |v: f64| f.top + f.plot_h() - (v - lo) / (hi - lo) * f.plot_h();
    for i in 0..=nticks {
        let v = lo + (hi - lo) * (i as f64) / (nticks as f64);
        let y = sy(v);
        s.push_str(&format!(
            "<line x1=\"{x1}\" y1=\"{y}\" x2=\"{x2}\" y2=\"{y}\" stroke=\"#e6e6e6\" stroke-width=\"1\"/>\n",
            x1 = fmt_num(f.left),
            x2 = fmt_num(f.left + f.plot_w()),
            y = fmt_num(y)
        ));
        s.push_str(&format!(
            "<text x=\"{x}\" y=\"{y}\" text-anchor=\"end\" font-size=\"11\" fill=\"#555555\">{t}</text>\n",
            x = fmt_num(f.left - 8.0),
            y = fmt_num(y + 4.0),
            t = fmt_num(v)
        ));
    }
    s.push_str(&format!(
        "<line x1=\"{x}\" y1=\"{y1}\" x2=\"{x}\" y2=\"{y2}\" stroke=\"#888888\" stroke-width=\"1\"/>\n",
        x = fmt_num(f.left),
        y1 = fmt_num(f.top),
        y2 = fmt_num(f.top + f.plot_h())
    ));
    if !label.is_empty() {
        s.push_str(&format!(
            "<text x=\"14\" y=\"{y}\" text-anchor=\"middle\" font-size=\"12\" fill=\"#333333\" transform=\"rotate(-90 14 {y})\">{t}</text>\n",
            y = fmt_num(f.top + f.plot_h() / 2.0),
            t = esc(label)
        ));
    }
}

fn x_axis_line(f: &Frame, baseline_y: f64, label: &str, s: &mut String) {
    s.push_str(&format!(
        "<line x1=\"{x1}\" y1=\"{y}\" x2=\"{x2}\" y2=\"{y}\" stroke=\"#888888\" stroke-width=\"1\"/>\n",
        x1 = fmt_num(f.left),
        x2 = fmt_num(f.left + f.plot_w()),
        y = fmt_num(baseline_y)
    ));
    if !label.is_empty() {
        s.push_str(&format!(
            "<text x=\"{x}\" y=\"{y}\" text-anchor=\"middle\" font-size=\"12\" fill=\"#333333\">{t}</text>\n",
            x = fmt_num(f.left + f.plot_w() / 2.0),
            y = fmt_num(f.height - 6.0),
            t = esc(label)
        ));
    }
}

fn data_rows(rows: &[Vec<String>], header_present: bool) -> &[Vec<String>] {
    if header_present {
        &rows[1..]
    } else {
        rows
    }
}

fn nice_top(max: f64) -> f64 {
    if max <= 0.0 {
        return 1.0;
    }
    max * 1.08
}

/// Render a bar chart: categorical X labels, numeric Y.
fn render_bar(
    f: &Frame,
    header: Option<&Vec<String>>,
    data: &[Vec<String>],
    xi: usize,
    yi: usize,
    color: &str,
    s: &mut String,
) -> Result<(), String> {
    let mut cats: Vec<String> = Vec::new();
    let mut vals: Vec<f64> = Vec::new();
    for r in data {
        let xv = r.get(xi).cloned().unwrap_or_default();
        let yraw = r.get(yi).map(|c| c.as_str()).unwrap_or("");
        let yv: f64 = yraw
            .trim()
            .parse()
            .map_err(|_| format!("Y value '{yraw}' is not a number (bar charts need a numeric Y column)"))?;
        cats.push(xv);
        vals.push(yv);
    }
    if vals.is_empty() {
        return Err("no data rows to plot".into());
    }
    let maxv = vals.iter().cloned().fold(0.0_f64, f64::max);
    let minv = vals.iter().cloned().fold(0.0_f64, f64::min);
    let hi = nice_top(maxv);
    let lo = if minv < 0.0 { minv * 1.08 } else { 0.0 };
    let sy = |v: f64| f.top + f.plot_h() - (v - lo) / (hi - lo) * f.plot_h();

    y_axis(f, lo, hi, 5, &col_name(header, yi), s);
    let baseline = sy(0.0);
    x_axis_line(f, baseline, &col_name(header, xi), s);

    let n = vals.len() as f64;
    let slot = f.plot_w() / n;
    let bw = (slot * 0.7).max(1.0);
    let label_every = ((n / 16.0).ceil() as usize).max(1);
    for (i, (cat, &v)) in cats.iter().zip(vals.iter()).enumerate() {
        let cx = f.left + slot * (i as f64) + slot / 2.0;
        let x = cx - bw / 2.0;
        let y_top = sy(v.max(0.0));
        let y_bot = sy(v.min(0.0));
        let h = (y_bot - y_top).abs().max(0.0);
        s.push_str(&format!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"{c}\"/>\n",
            x = fmt_num(x),
            y = fmt_num(y_top),
            w = fmt_num(bw),
            h = fmt_num(h),
            c = esc(color)
        ));
        if i % label_every == 0 {
            s.push_str(&format!(
                "<text x=\"{x}\" y=\"{y}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#555555\">{t}</text>\n",
                x = fmt_num(cx),
                y = fmt_num(baseline + 16.0),
                t = esc(cat)
            ));
        }
    }
    Ok(())
}

/// Render a histogram: bin a single numeric column into `bins` buckets.
fn render_histogram(
    f: &Frame,
    header: Option<&Vec<String>>,
    data: &[Vec<String>],
    xi: usize,
    bins: u32,
    color: &str,
    s: &mut String,
) -> Result<(), String> {
    let mut vals: Vec<f64> = Vec::new();
    for r in data {
        if let Some(cell) = r.get(xi) {
            if cell.trim().is_empty() {
                continue;
            }
            let v: f64 = cell
                .trim()
                .parse()
                .map_err(|_| format!("value '{cell}' is not a number (histograms need a numeric column)"))?;
            if v.is_finite() {
                vals.push(v);
            }
        }
    }
    if vals.is_empty() {
        return Err("no numeric values to bin".into());
    }
    let bins = bins.clamp(2, 100) as usize;
    let minv = vals.iter().cloned().fold(f64::INFINITY, f64::min);
    let mut maxv = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if (maxv - minv).abs() < 1e-12 {
        maxv = minv + 1.0;
    }
    let span = maxv - minv;
    let bw = span / bins as f64;
    let mut counts = vec![0u32; bins];
    for &v in &vals {
        let mut b = ((v - minv) / bw).floor() as isize;
        if b < 0 {
            b = 0;
        }
        if b >= bins as isize {
            b = bins as isize - 1;
        }
        counts[b as usize] += 1;
    }
    let maxc = counts.iter().cloned().max().unwrap_or(1).max(1) as f64;
    let hi = nice_top(maxc);
    let sy = |v: f64| f.top + f.plot_h() - v / hi * f.plot_h();

    y_axis(f, 0.0, hi, 5, "Frequency", s);
    let baseline = sy(0.0);
    x_axis_line(f, baseline, &col_name(header, xi), s);

    let slot = f.plot_w() / bins as f64;
    let label_every = ((bins as f64 / 10.0).ceil() as usize).max(1);
    for (i, &c) in counts.iter().enumerate() {
        let x = f.left + slot * i as f64;
        let y = sy(c as f64);
        let h = (baseline - y).max(0.0);
        s.push_str(&format!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"{col}\" stroke=\"#ffffff\" stroke-width=\"1\"/>\n",
            x = fmt_num(x),
            y = fmt_num(y),
            w = fmt_num(slot),
            h = fmt_num(h),
            col = esc(color)
        ));
        if i % label_every == 0 {
            let edge = minv + bw * i as f64;
            s.push_str(&format!(
                "<text x=\"{x}\" y=\"{y}\" text-anchor=\"middle\" font-size=\"10\" fill=\"#555555\">{t}</text>\n",
                x = fmt_num(x),
                y = fmt_num(baseline + 16.0),
                t = fmt_num(edge)
            ));
        }
    }
    s.push_str(&format!(
        "<text x=\"{x}\" y=\"{y}\" text-anchor=\"middle\" font-size=\"10\" fill=\"#555555\">{t}</text>\n",
        x = fmt_num(f.left + f.plot_w()),
        y = fmt_num(baseline + 16.0),
        t = fmt_num(maxv)
    ));
    Ok(())
}

/// Render a scatter or line chart from an X and Y column.
fn render_xy(
    f: &Frame,
    header: Option<&Vec<String>>,
    data: &[Vec<String>],
    xi: usize,
    yi: usize,
    line: bool,
    color: &str,
    s: &mut String,
) -> Result<(), String> {
    // For a line chart, non-numeric X falls back to row index with the raw cell
    // as the tick label; scatter requires numeric X.
    let x_all_numeric = data
        .iter()
        .filter_map(|r| r.get(xi))
        .all(|c| c.trim().is_empty() || is_number(c));

    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    let mut xlabels: Vec<String> = Vec::new();
    for (idx, r) in data.iter().enumerate() {
        let xraw = r.get(xi).map(|c| c.as_str()).unwrap_or("");
        let yraw = r.get(yi).map(|c| c.as_str()).unwrap_or("");
        if yraw.trim().is_empty() {
            continue;
        }
        let yv: f64 = yraw
            .trim()
            .parse()
            .map_err(|_| format!("Y value '{yraw}' is not a number"))?;
        let xv: f64 = if x_all_numeric {
            if xraw.trim().is_empty() {
                continue;
            }
            xraw.trim()
                .parse()
                .map_err(|_| format!("X value '{xraw}' is not a number"))?
        } else if line {
            idx as f64
        } else {
            return Err(format!(
                "X value '{xraw}' is not a number (scatter charts need a numeric X column)"
            ));
        };
        xs.push(xv);
        ys.push(yv);
        xlabels.push(xraw.to_string());
    }
    if xs.is_empty() {
        return Err("no data rows to plot".into());
    }

    let (mut xmin, mut xmax) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut ymin, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..xs.len() {
        xmin = xmin.min(xs[i]);
        xmax = xmax.max(xs[i]);
        ymin = ymin.min(ys[i]);
        ymax = ymax.max(ys[i]);
    }
    if (xmax - xmin).abs() < 1e-12 {
        xmin -= 1.0;
        xmax += 1.0;
    }
    if (ymax - ymin).abs() < 1e-12 {
        ymin -= 1.0;
        ymax += 1.0;
    }
    let ypad = (ymax - ymin) * 0.05;
    let (ylo, yhi) = (ymin - ypad, ymax + ypad);
    let xpad = (xmax - xmin) * 0.03;
    let (xlo, xhi) = (xmin - xpad, xmax + xpad);

    let sx = |x: f64| f.left + (x - xlo) / (xhi - xlo) * f.plot_w();
    let sy = |y: f64| f.top + f.plot_h() - (y - ylo) / (yhi - ylo) * f.plot_h();

    y_axis(f, ylo, yhi, 5, &col_name(header, yi), s);
    x_axis_line(f, f.top + f.plot_h(), &col_name(header, xi), s);

    let base_y = f.top + f.plot_h();
    if x_all_numeric {
        let nticks = 5usize;
        for i in 0..=nticks {
            let v = xlo + (xhi - xlo) * (i as f64) / (nticks as f64);
            s.push_str(&format!(
                "<text x=\"{x}\" y=\"{y}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#555555\">{t}</text>\n",
                x = fmt_num(sx(v)),
                y = fmt_num(base_y + 16.0),
                t = fmt_num(v)
            ));
        }
    } else {
        let n = xs.len();
        let step = (((n as f64) / 12.0).ceil() as usize).max(1);
        for i in (0..n).step_by(step) {
            s.push_str(&format!(
                "<text x=\"{x}\" y=\"{y}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#555555\">{t}</text>\n",
                x = fmt_num(sx(xs[i])),
                y = fmt_num(base_y + 16.0),
                t = esc(&xlabels[i])
            ));
        }
    }

    if line {
        let mut order: Vec<usize> = (0..xs.len()).collect();
        order.sort_by(|&a, &b| xs[a].partial_cmp(&xs[b]).unwrap_or(std::cmp::Ordering::Equal));
        let pts: Vec<String> = order
            .iter()
            .map(|&i| format!("{},{}", fmt_num(sx(xs[i])), fmt_num(sy(ys[i]))))
            .collect();
        s.push_str(&format!(
            "<polyline fill=\"none\" stroke=\"{c}\" stroke-width=\"2\" points=\"{p}\"/>\n",
            c = esc(color),
            p = pts.join(" ")
        ));
        for &i in &order {
            s.push_str(&format!(
                "<circle cx=\"{cx}\" cy=\"{cy}\" r=\"3\" fill=\"{c}\"/>\n",
                cx = fmt_num(sx(xs[i])),
                cy = fmt_num(sy(ys[i])),
                c = esc(color)
            ));
        }
    } else {
        for i in 0..xs.len() {
            s.push_str(&format!(
                "<circle cx=\"{cx}\" cy=\"{cy}\" r=\"4\" fill=\"{c}\" fill-opacity=\"0.75\"/>\n",
                cx = fmt_num(sx(xs[i])),
                cy = fmt_num(sy(ys[i])),
                c = esc(color)
            ));
        }
    }
    Ok(())
}

/// Render the requested chart from CSV text to an SVG string.
pub fn render(csv: &str, opts: &Options) -> Result<String, String> {
    let chart = opts.chart_type.trim().to_lowercase();
    if !matches!(chart.as_str(), "bar" | "line" | "scatter" | "histogram") {
        return Err(format!(
            "chart_type '{}' is not supported — use bar, line, scatter, or histogram",
            opts.chart_type
        ));
    }
    let rows = parse_csv(csv);
    if rows.is_empty() {
        return Err("the CSV is empty — paste at least a header and one data row".into());
    }
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if ncols == 0 {
        return Err("the CSV has no columns".into());
    }
    let header_present = has_header(&rows);
    let header: Option<&Vec<String>> = if header_present { rows.first() } else { None };

    let data = data_rows(&rows, header_present);
    if data.is_empty() {
        return Err("no data rows found (the CSV has only a header)".into());
    }

    let width = opts.width.clamp(200, 4000) as f64;
    let height = opts.height.clamp(150, 4000) as f64;
    let f = Frame {
        width,
        height,
        left: 60.0,
        right: 24.0,
        top: if opts.title.trim().is_empty() { 24.0 } else { 44.0 },
        bottom: 56.0,
    };
    let color = if opts.color.trim().is_empty() {
        "#2563eb"
    } else {
        opts.color.trim()
    };

    let mut s = svg_open(&f, &opts.title);

    // Resolve the Y column (numeric series) for every chart type. Histograms
    // bin this column; bar/line/scatter also need an X column.
    let yi = resolve_col(&opts.y_column, header, ncols).map_err(|e| format!("y_column: {e}"))?;
    match chart.as_str() {
        "histogram" => render_histogram(&f, header, data, yi, opts.bins, color, &mut s)?,
        "bar" => {
            let xi = resolve_col(&opts.x_column, header, ncols).map_err(|e| format!("x_column: {e}"))?;
            render_bar(&f, header, data, xi, yi, color, &mut s)?;
        }
        "line" | "scatter" => {
            let xi = resolve_col(&opts.x_column, header, ncols).map_err(|e| format!("x_column: {e}"))?;
            render_xy(&f, header, data, xi, yi, chart == "line", color, &mut s)?;
        }
        _ => unreachable!(),
    }

    s.push_str("</svg>");
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(ct: &str, x: &str, y: &str) -> Options {
        Options {
            chart_type: ct.into(),
            x_column: x.into(),
            y_column: y.into(),
            title: "".into(),
            width: 800,
            height: 400,
            color: "#4e79a7".into(),
            bins: 10,
        }
    }

    #[test]
    fn bar_chart_by_header_name() {
        let csv = "fruit,count\napple,5\nbanana,3\ncherry,8";
        let svg = render(csv, &opts("bar", "fruit", "count")).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("<rect"));
        assert!(svg.contains("apple"));
        assert!(svg.contains("#4e79a7"));
    }

    #[test]
    fn scatter_by_1based_index() {
        let csv = "1,2\n2,4\n3,9\n4,16";
        let svg = render(csv, &opts("scatter", "1", "2")).unwrap();
        assert!(svg.contains("<circle"));
    }

    #[test]
    fn line_chart_sorted() {
        let csv = "x,y\n3,30\n1,10\n2,20";
        let svg = render(csv, &opts("line", "x", "y")).unwrap();
        assert!(svg.contains("<polyline"));
    }

    #[test]
    fn histogram_bins_y_column() {
        let csv = "v\n1\n1\n2\n2\n2\n3";
        let mut o = opts("histogram", "", "v");
        o.bins = 3;
        let svg = render(csv, &o).unwrap();
        assert!(svg.contains("Frequency"));
        assert!(svg.contains("<rect"));
    }

    #[test]
    fn title_is_rendered_and_escaped() {
        let csv = "a,b\n1,2\n3,4";
        let mut o = opts("bar", "a", "b");
        o.title = "Sales <2024>".into();
        let svg = render(csv, &o).unwrap();
        assert!(svg.contains("Sales &lt;2024&gt;"));
    }

    #[test]
    fn err_unknown_chart_type() {
        let csv = "a,b\n1,2";
        let e = render(csv, &opts("pie", "a", "b")).unwrap_err();
        assert!(e.contains("chart_type"));
    }

    #[test]
    fn err_missing_column() {
        let csv = "a,b\n1,2";
        let e = render(csv, &opts("bar", "nope", "b")).unwrap_err();
        assert!(e.contains("x_column"));
        assert!(e.contains("not found"));
    }

    #[test]
    fn err_non_numeric_y() {
        let csv = "a,b\nx,hello\ny,world";
        let e = render(csv, &opts("bar", "a", "b")).unwrap_err();
        assert!(e.contains("not a number"));
    }

    #[test]
    fn empty_csv_errors() {
        let e = render("   ", &opts("bar", "a", "b")).unwrap_err();
        assert!(e.contains("empty"));
    }

    #[test]
    fn quoted_fields_with_commas() {
        let csv = "label,amount\n\"a, b\",5\n\"c, d\",7";
        let svg = render(csv, &opts("bar", "label", "amount")).unwrap();
        assert!(svg.contains("a, b"));
    }
}
