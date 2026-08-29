//! pareto-chart core — parse `label,value` rows, rank them, and render a deterministic
//! Pareto chart (sorted bars on a value axis + a cumulative-percentage line on a second
//! 0-100% axis) as self-contained SVG, an aligned summary table, or JSON.
//!
//! Pure compute, shared by the chat skill block and the web page. No wafer/wasm-bindgen
//! deps, no floating clock, no external data — the same input always renders byte-identical
//! output.

/// Hard cap on pasted rows, so a runaway paste fails fast instead of hanging the browser.
pub const MAX_ROWS: usize = 10_000;
/// Hard cap on distinct categories after duplicate labels are summed together.
pub const MAX_CATEGORIES: usize = 500;

#[derive(Clone, Debug)]
pub struct Options {
    pub delimiter: String,
    pub header: String,
    pub sort: String,
    pub max_categories: u32,
    pub other_label: String,
    pub threshold: f64,
    pub highlight_vital_few: bool,
    pub show_cumulative: bool,
    pub show_values: bool,
    pub show_cumulative_labels: bool,
    pub decimals: u32,
    pub title: String,
    pub category_label: String,
    pub value_label: String,
    pub percent_label: String,
    pub label_angle: f64,
    pub color: String,
    pub vital_color: String,
    pub line_color: String,
    pub threshold_color: String,
    pub background: String,
    pub bar_width: f64,
    pub line_width: f64,
    pub point_radius: f64,
    pub grid: bool,
    pub legend: bool,
    pub font_size: f64,
    pub width: u32,
    pub height: u32,
    pub theme: String,
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            delimiter: "auto".into(),
            header: "auto".into(),
            sort: "desc".into(),
            max_categories: 0,
            other_label: "Other".into(),
            threshold: 80.0,
            highlight_vital_few: true,
            show_cumulative: true,
            show_values: false,
            show_cumulative_labels: false,
            decimals: 1,
            title: String::new(),
            category_label: String::new(),
            value_label: String::new(),
            percent_label: "Cumulative %".into(),
            label_angle: 0.0,
            color: "#2563eb".into(),
            vital_color: "#f97316".into(),
            line_color: "#dc2626".into(),
            threshold_color: "#94a3b8".into(),
            background: String::new(),
            bar_width: 0.8,
            line_width: 2.0,
            point_radius: 3.5,
            grid: true,
            legend: true,
            font_size: 13.0,
            width: 820,
            height: 520,
            theme: "light".into(),
            output: "svg".into(),
        }
    }
}

/// One ranked category with its share of the total.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: f64,
    /// Share of the grand total, 0-100.
    pub percent: f64,
    /// Running sum of `value` in display order.
    pub cumulative: f64,
    /// Running share of the grand total in display order, 0-100.
    pub cumulative_percent: f64,
    /// True while the running share has not yet passed the threshold (inclusive of the
    /// row that crosses it). Always false when `threshold` is 0.
    pub vital: bool,
    /// True for the synthetic bucket that absorbs the tail past `max_categories`.
    pub is_other: bool,
}

/// The full analysis behind every output format.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub rows: Vec<Row>,
    pub total: f64,
    /// Index of the row whose cumulative percentage first reaches the threshold.
    pub crossing_index: Option<usize>,
    /// Header names picked up from the pasted table, when there was a header row.
    pub header_names: Option<(String, String)>,
}

// ---------------------------------------------------------------------------
// parsing
// ---------------------------------------------------------------------------

/// Split on `d`, honouring `"…"` quoting so `A,"1,250"` stays two fields.
fn split_delimited(line: &str, d: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cur.push(ch);
            }
        } else if ch == '"' {
            quoted = true;
        } else if ch == d {
            out.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(ch);
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn split_row(line: &str, delim: &str) -> Vec<String> {
    match delim {
        "comma" => split_delimited(line, ','),
        "tab" => split_delimited(line, '\t'),
        "semicolon" => split_delimited(line, ';'),
        "pipe" => split_delimited(line, '|'),
        // Whitespace rows are `label with spaces  value`: split at the LAST run of spaces so
        // multi-word labels survive.
        _ => match line.rsplit_once(char::is_whitespace) {
            Some((head, tail)) if !tail.trim().is_empty() => {
                vec![head.trim().to_string(), tail.trim().to_string()]
            }
            _ => vec![line.trim().to_string()],
        },
    }
}

/// Pick the delimiter that actually separates the pasted rows.
fn detect_delimiter(lines: &[&str]) -> String {
    for (name, ch) in [
        ("tab", '\t'),
        ("semicolon", ';'),
        ("pipe", '|'),
        ("comma", ','),
    ] {
        // A delimiter only counts if EVERY row carries it, so a stray comma inside one
        // whitespace-separated label can't hijack the whole table.
        if lines.iter().all(|l| l.contains(ch)) {
            return name.to_string();
        }
    }
    "whitespace".to_string()
}

/// Read a number written the way people paste numbers: `1,234`, `$1 234.50`, `12%`, `(3)`.
fn parse_value(raw: &str) -> Option<f64> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    let negated = t.starts_with('(') && t.ends_with(')');
    let mut cleaned = String::new();
    for c in t.chars() {
        match c {
            '0'..='9' | '.' | '-' | '+' | 'e' | 'E' => cleaned.push(c),
            ',' | '_' | ' ' | '\'' | '%' | '$' | '£' | '€' | '¥' | '(' | ')' => {}
            _ => return None,
        }
    }
    if cleaned.is_empty() {
        return None;
    }
    let v = cleaned.parse::<f64>().ok()?;
    if !v.is_finite() {
        return None;
    }
    Some(if negated { -v } else { v })
}

/// Pull `(label, value)` out of one already-split row.
fn row_pair(fields: &[String]) -> Option<(String, f64)> {
    if fields.len() < 2 {
        return None;
    }
    // The label is the first field; the value is the first later field that reads as a
    // number, so `category,count,note` and `category,count` both work.
    let label = fields[0].trim().to_string();
    for f in &fields[1..] {
        if let Some(v) = parse_value(f) {
            return Some((label, v));
        }
    }
    None
}

/// Parse + rank the pasted table into the analysis every output format renders.
pub fn analyze(data: &str, o: &Options) -> Result<Analysis, String> {
    let lines: Vec<&str> = data
        .lines()
        .map(|l| match l.find('#') {
            Some(i) => &l[..i],
            None => l,
        })
        .map(|l| l.trim_end_matches('\r').trim())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.is_empty() {
        return Err("no data rows found — paste one `label,value` per line, for example `Late delivery,45`".into());
    }
    if lines.len() > MAX_ROWS {
        return Err(format!(
            "too many rows: got {}, the limit is {MAX_ROWS}",
            lines.len()
        ));
    }

    let delim = match o.delimiter.as_str() {
        "auto" => detect_delimiter(&lines),
        "comma" | "tab" | "semicolon" | "pipe" | "whitespace" => o.delimiter.clone(),
        other => {
            return Err(format!(
                "delimiter must be auto, comma, tab, semicolon, pipe, or whitespace, got `{other}`"
            ))
        }
    };

    let split: Vec<Vec<String>> = lines.iter().map(|l| split_row(l, &delim)).collect();

    // Header detection: a first row whose value column is not a number is a header.
    let first_is_header = match o.header.as_str() {
        "yes" => true,
        "no" => false,
        "auto" => row_pair(&split[0]).is_none() && split[0].len() >= 2,
        other => return Err(format!("header must be auto, yes, or no, got `{other}`")),
    };
    let header_names = if first_is_header && split[0].len() >= 2 {
        Some((split[0][0].clone(), split[0][1].clone()))
    } else {
        None
    };
    let body_start = usize::from(first_is_header);
    if body_start >= split.len() {
        return Err("the input has a header row but no data rows below it".into());
    }

    // Sum duplicate labels, keeping first-seen order so `sort=input` is meaningful.
    let mut order: Vec<String> = Vec::new();
    let mut sums: Vec<f64> = Vec::new();
    for (i, fields) in split.iter().enumerate().skip(body_start) {
        let line_no = i + 1;
        let (label, value) = row_pair(fields).ok_or_else(|| {
            format!(
                "line {line_no}: expected `label{}value`, got `{}`",
                if delim == "whitespace" {
                    " "
                } else {
                    delimiter_char(&delim)
                },
                lines[i]
            )
        })?;
        if value < 0.0 {
            return Err(format!(
                "line {line_no}: values must be zero or positive for a Pareto chart, got `{value}` for `{label}`"
            ));
        }
        let label = if label.is_empty() {
            format!("(row {line_no})")
        } else {
            label
        };
        match order.iter().position(|l| *l == label) {
            Some(k) => sums[k] += value,
            None => {
                order.push(label);
                sums.push(value);
            }
        }
    }

    if order.len() > MAX_CATEGORIES {
        return Err(format!(
            "too many categories: got {}, the limit is {MAX_CATEGORIES} — set max_categories to bucket the tail into one `Other` bar",
            order.len()
        ));
    }

    let mut pairs: Vec<(String, f64)> = order.into_iter().zip(sums).collect();
    match o.sort.as_str() {
        // Ties keep input order: `sort_by` is stable, so equal values never shuffle.
        "desc" => pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)),
        "asc" => pairs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)),
        "input" => {}
        other => return Err(format!("sort must be desc, asc, or input, got `{other}`")),
    }

    // Bucket the tail so a long-tail table still reads as a chart.
    let cap = o.max_categories as usize;
    let mut is_other = vec![false; pairs.len()];
    if cap > 0 && pairs.len() > cap {
        let tail: f64 = pairs[cap..].iter().map(|p| p.1).sum();
        pairs.truncate(cap);
        is_other.truncate(cap);
        let label = if o.other_label.trim().is_empty() {
            "Other".to_string()
        } else {
            o.other_label.trim().to_string()
        };
        pairs.push((label, tail));
        is_other.push(true);
    }

    let total: f64 = pairs.iter().map(|p| p.1).sum();
    if total <= 0.0 {
        return Err(
            "every value is zero — a Pareto chart needs at least one category above zero".into(),
        );
    }
    if !(0.0..=100.0).contains(&o.threshold) {
        return Err(format!(
            "threshold must be between 0 and 100, got `{}`",
            o.threshold
        ));
    }

    let mut rows = Vec::with_capacity(pairs.len());
    let mut running = 0.0;
    let mut crossing_index = None;
    for (i, (label, value)) in pairs.into_iter().enumerate() {
        running += value;
        let cumulative_percent = running / total * 100.0;
        // `vital` marks the leading block that reaches the threshold, the crossing row
        // included — that block IS the "vital few".
        let vital = o.threshold > 0.0 && crossing_index.is_none();
        if o.threshold > 0.0 && crossing_index.is_none() && cumulative_percent + 1e-9 >= o.threshold
        {
            crossing_index = Some(i);
        }
        rows.push(Row {
            label,
            value,
            percent: value / total * 100.0,
            cumulative: running,
            cumulative_percent,
            vital,
            is_other: is_other[i],
        });
    }

    Ok(Analysis {
        rows,
        total,
        crossing_index,
        header_names,
    })
}

fn delimiter_char(delim: &str) -> &'static str {
    match delim {
        "tab" => "\t",
        "semicolon" => ";",
        "pipe" => "|",
        _ => ",",
    }
}

// ---------------------------------------------------------------------------
// formatting helpers
// ---------------------------------------------------------------------------

fn fmt_pct(v: f64, decimals: u32) -> String {
    format!("{:.*}", decimals as usize, v)
}

/// Values print without trailing zero noise: 45 stays `45`, 45.5 stays `45.5`.
fn fmt_value(v: f64, decimals: u32) -> String {
    let s = format!("{:.*}", decimals as usize, v);
    if s.contains('.') {
        let t = s.trim_end_matches('0').trim_end_matches('.');
        if t.is_empty() || t == "-" {
            "0".to_string()
        } else {
            t.to_string()
        }
    } else {
        s
    }
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Round a coordinate to 2 decimals so the SVG is compact and byte-stable.
fn c(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    let s = format!("{r:.2}");
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" || t == "-0" {
        "0".to_string()
    } else {
        t.to_string()
    }
}

/// Round an axis maximum up to a readable 1/2/2.5/5/10 step.
fn nice_max(v: f64, intervals: f64) -> f64 {
    if v <= 0.0 {
        return 1.0;
    }
    let rough = v / intervals;
    let mag = 10f64.powf(rough.log10().floor());
    let norm = rough / mag;
    let step = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 2.5 {
        2.5
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    step * mag * intervals
}

// ---------------------------------------------------------------------------
// theme
// ---------------------------------------------------------------------------

struct Theme {
    background: String,
    text: String,
    muted: String,
    axis: String,
    grid: String,
}

fn theme_of(o: &Options) -> Result<Theme, String> {
    let mut t = match o.theme.as_str() {
        "light" => Theme {
            background: "#ffffff".into(),
            text: "#0f172a".into(),
            muted: "#475569".into(),
            axis: "#94a3b8".into(),
            grid: "#e2e8f0".into(),
        },
        "dark" => Theme {
            background: "#0f172a".into(),
            text: "#f1f5f9".into(),
            muted: "#cbd5e1".into(),
            axis: "#64748b".into(),
            grid: "#1e293b".into(),
        },
        other => return Err(format!("theme must be light or dark, got `{other}`")),
    };
    if !o.background.trim().is_empty() {
        t.background = o.background.trim().to_string();
    }
    Ok(t)
}

// ---------------------------------------------------------------------------
// render
// ---------------------------------------------------------------------------

/// Parse `data` and render it in the format named by `o.output`.
pub fn render(data: &str, o: &Options) -> Result<String, String> {
    let a = analyze(data, o)?;
    match o.output.as_str() {
        "svg" => render_svg(&a, o),
        "summary" => Ok(render_summary(&a, o)),
        "json" => render_json(&a, o),
        other => Err(format!(
            "output must be svg, summary, or json, got `{other}`"
        )),
    }
}

fn axis_titles(a: &Analysis, o: &Options) -> (String, String) {
    // An explicit label always wins; otherwise reuse the pasted header row, which is
    // usually exactly what the axes should say.
    let cat = if !o.category_label.trim().is_empty() {
        o.category_label.trim().to_string()
    } else {
        a.header_names
            .as_ref()
            .map(|h| h.0.clone())
            .unwrap_or_default()
    };
    let val = if !o.value_label.trim().is_empty() {
        o.value_label.trim().to_string()
    } else {
        a.header_names
            .as_ref()
            .map(|h| h.1.clone())
            .unwrap_or_default()
    };
    (cat, val)
}

fn render_svg(a: &Analysis, o: &Options) -> Result<String, String> {
    let th = theme_of(o)?;
    let fs = o.font_size.clamp(6.0, 48.0);
    let w = o.width.clamp(320, 2400) as f64;
    let h = o.height.clamp(240, 1800) as f64;
    let bar_frac = o.bar_width.clamp(0.05, 1.0);
    let lw = o.line_width.clamp(0.0, 8.0);
    let pr = o.point_radius.clamp(0.0, 12.0);
    let angle = o.label_angle.clamp(0.0, 90.0);
    let dec = o.decimals.min(6);
    let n = a.rows.len();
    let (cat_title, val_title) = axis_titles(a, o);
    let pct_title = o.percent_label.trim().to_string();

    let intervals = 5.0;
    let vmax_raw = a.rows.iter().fold(0.0f64, |m, r| m.max(r.value));
    let vmax = nice_max(vmax_raw, intervals);

    // ---- margins -------------------------------------------------------
    let pad = 16.0;
    let char_w = fs * 0.6;
    let title_h = if o.title.trim().is_empty() {
        0.0
    } else {
        fs * 2.2
    };
    let tick_w = (0..=intervals as usize)
        .map(|i| fmt_value(vmax * i as f64 / intervals, dec).chars().count())
        .max()
        .unwrap_or(1) as f64
        * char_w;
    let left = pad + tick_w + 10.0 + if val_title.is_empty() { 0.0 } else { fs * 1.5 };
    let right = pad
        + if o.show_cumulative {
            "100%".chars().count() as f64 * char_w + 10.0
        } else {
            0.0
        }
        + if pct_title.is_empty() || !o.show_cumulative {
            0.0
        } else {
            fs * 1.5
        };

    let longest_label = a
        .rows
        .iter()
        .map(|r| r.label.chars().count())
        .max()
        .unwrap_or(1) as f64;
    let label_h = if angle > 0.0 {
        (longest_label * char_w * (angle.to_radians()).sin()).min(h * 0.45) + fs * 0.9
    } else {
        fs * 1.6
    };
    let legend_h = if o.legend { fs * 2.0 } else { 0.0 };
    let bottom = pad + label_h + if cat_title.is_empty() { 0.0 } else { fs * 1.7 } + legend_h;

    let x0 = left;
    let x1 = w - right;
    let y0 = pad + title_h;
    let y1 = h - bottom;
    let plot_w = x1 - x0;
    let plot_h = y1 - y0;
    if plot_w < 40.0 || plot_h < 40.0 {
        return Err(format!(
            "the chart area collapsed to {}x{}px — increase width/height, shorten the category labels, or lower label_angle/font_size",
            c(plot_w.max(0.0)),
            c(plot_h.max(0.0))
        ));
    }

    let slot = plot_w / n as f64;
    let bar_w = (slot * bar_frac).max(1.0);
    let y_value = |v: f64| y1 - (v / vmax).clamp(0.0, 1.0) * plot_h;
    let y_pct = |p: f64| y1 - (p / 100.0).clamp(0.0, 1.0) * plot_h;
    let x_center = |i: usize| x0 + slot * (i as f64 + 0.5);

    let mut s = String::with_capacity(2048 + n * 220);
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" role=\"img\" aria-label=\"{}\">",
        c(w), c(h), c(w), c(h),
        escape_xml(&if o.title.trim().is_empty() { "Pareto chart".to_string() } else { o.title.trim().to_string() })
    ));
    s.push_str(&format!(
        "<rect width=\"{}\" height=\"{}\" fill=\"{}\"/>",
        c(w),
        c(h),
        escape_xml(th.background.trim())
    ));

    if !o.title.trim().is_empty() {
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-family=\"system-ui, -apple-system, Segoe UI, Roboto, sans-serif\" font-size=\"{}\" font-weight=\"600\" fill=\"{}\">{}</text>",
            c(w / 2.0), c(pad + fs * 1.25), c(fs * 1.25), escape_xml(&th.text), escape_xml(o.title.trim())
        ));
    }

    // ---- grid + value axis ---------------------------------------------
    s.push_str(&format!(
        "<g font-family=\"system-ui, -apple-system, Segoe UI, Roboto, sans-serif\" font-size=\"{}\">",
        c(fs * 0.85)
    ));
    for i in 0..=intervals as usize {
        let frac = i as f64 / intervals;
        let y = y1 - frac * plot_h;
        if o.grid && i > 0 {
            s.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>",
                c(x0),
                c(y),
                c(x1),
                c(y),
                escape_xml(&th.grid)
            ));
        }
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" fill=\"{}\">{}</text>",
            c(x0 - 8.0),
            c(y + fs * 0.3),
            escape_xml(&th.muted),
            escape_xml(&fmt_value(vmax * frac, dec))
        ));
        if o.show_cumulative {
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"start\" fill=\"{}\">{}%</text>",
                c(x1 + 8.0),
                c(y + fs * 0.3),
                escape_xml(&th.muted),
                c(frac * 100.0)
            ));
        }
    }
    s.push_str("</g>");

    // ---- bars ----------------------------------------------------------
    let bar_color = if o.color.trim().is_empty() {
        "#2563eb"
    } else {
        o.color.trim()
    };
    let vital_color = if o.vital_color.trim().is_empty() {
        "#f97316"
    } else {
        o.vital_color.trim()
    };
    for (i, r) in a.rows.iter().enumerate() {
        let bx = x_center(i) - bar_w / 2.0;
        let by = y_value(r.value);
        let bh = (y1 - by).max(0.0);
        let fill = if o.highlight_vital_few && r.vital && o.threshold > 0.0 {
            vital_color
        } else {
            bar_color
        };
        s.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"><title>{}: {} ({}% of total, {}% cumulative)</title></rect>",
            c(bx), c(by), c(bar_w), c(bh), escape_xml(fill),
            escape_xml(&r.label), escape_xml(&fmt_value(r.value, dec)),
            escape_xml(&fmt_pct(r.percent, dec)), escape_xml(&fmt_pct(r.cumulative_percent, dec))
        ));
        if o.show_values && bh > 0.0 {
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-family=\"system-ui, -apple-system, Segoe UI, Roboto, sans-serif\" font-size=\"{}\" fill=\"{}\">{}</text>",
                c(x_center(i)), c(by - 5.0), c(fs * 0.85), escape_xml(&th.text),
                escape_xml(&fmt_value(r.value, dec))
            ));
        }
    }

    // ---- axes ----------------------------------------------------------
    s.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"/><line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>",
        c(x0), c(y0), c(x0), c(y1), escape_xml(&th.axis),
        c(x0), c(y1), c(x1), c(y1), escape_xml(&th.axis)
    ));
    if o.show_cumulative {
        s.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>",
            c(x1),
            c(y0),
            c(x1),
            c(y1),
            escape_xml(&th.axis)
        ));
    }

    // ---- threshold line -------------------------------------------------
    let threshold_color = if o.threshold_color.trim().is_empty() {
        "#94a3b8"
    } else {
        o.threshold_color.trim()
    };
    if o.threshold > 0.0 {
        let ty = y_pct(o.threshold);
        s.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1.5\" stroke-dasharray=\"6 4\"><title>{}% threshold</title></line>",
            c(x0), c(ty), c(x1), c(ty), escape_xml(threshold_color), escape_xml(&fmt_value(o.threshold, dec))
        ));
    }

    // ---- cumulative line ------------------------------------------------
    let line_color = if o.line_color.trim().is_empty() {
        "#dc2626"
    } else {
        o.line_color.trim()
    };
    if o.show_cumulative {
        if lw > 0.0 && n > 1 {
            let pts: Vec<String> = a
                .rows
                .iter()
                .enumerate()
                .map(|(i, r)| format!("{},{}", c(x_center(i)), c(y_pct(r.cumulative_percent))))
                .collect();
            s.push_str(&format!(
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"/>",
                pts.join(" "), escape_xml(line_color), c(lw)
            ));
        }
        for (i, r) in a.rows.iter().enumerate() {
            let px = x_center(i);
            let py = y_pct(r.cumulative_percent);
            if pr > 0.0 {
                s.push_str(&format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"><title>{}: {}% cumulative</title></circle>",
                    c(px), c(py), c(pr), escape_xml(line_color),
                    escape_xml(&r.label), escape_xml(&fmt_pct(r.cumulative_percent, dec))
                ));
            }
            if o.show_cumulative_labels {
                s.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-family=\"system-ui, -apple-system, Segoe UI, Roboto, sans-serif\" font-size=\"{}\" fill=\"{}\">{}%</text>",
                    c(px), c(py - pr - 5.0), c(fs * 0.8), escape_xml(line_color),
                    escape_xml(&fmt_pct(r.cumulative_percent, dec))
                ));
            }
        }
    }

    // ---- category labels -------------------------------------------------
    s.push_str(&format!(
        "<g font-family=\"system-ui, -apple-system, Segoe UI, Roboto, sans-serif\" font-size=\"{}\" fill=\"{}\">",
        c(fs * 0.85),
        escape_xml(&th.text)
    ));
    for (i, r) in a.rows.iter().enumerate() {
        let px = x_center(i);
        if angle > 0.0 {
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" transform=\"rotate({} {} {})\">{}</text>",
                c(px), c(y1 + fs * 1.1), c(-angle), c(px), c(y1 + fs * 1.1), escape_xml(&r.label)
            ));
        } else {
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
                c(px),
                c(y1 + fs * 1.2),
                escape_xml(&r.label)
            ));
        }
    }
    s.push_str("</g>");

    // ---- axis titles -----------------------------------------------------
    if !val_title.is_empty() {
        let ty = (y0 + y1) / 2.0;
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" transform=\"rotate(-90 {} {})\" font-family=\"system-ui, -apple-system, Segoe UI, Roboto, sans-serif\" font-size=\"{}\" fill=\"{}\">{}</text>",
            c(pad + fs * 0.4), c(ty), c(pad + fs * 0.4), c(ty), c(fs * 0.9), escape_xml(&th.muted), escape_xml(&val_title)
        ));
    }
    if o.show_cumulative && !pct_title.is_empty() {
        let ty = (y0 + y1) / 2.0;
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" transform=\"rotate(90 {} {})\" font-family=\"system-ui, -apple-system, Segoe UI, Roboto, sans-serif\" font-size=\"{}\" fill=\"{}\">{}</text>",
            c(w - pad - fs * 0.4), c(ty), c(w - pad - fs * 0.4), c(ty), c(fs * 0.9), escape_xml(&th.muted), escape_xml(&pct_title)
        ));
    }
    if !cat_title.is_empty() {
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-family=\"system-ui, -apple-system, Segoe UI, Roboto, sans-serif\" font-size=\"{}\" fill=\"{}\">{}</text>",
            c((x0 + x1) / 2.0), c(y1 + label_h + fs * 1.2), c(fs * 0.9), escape_xml(&th.muted), escape_xml(&cat_title)
        ));
    }

    // ---- legend ----------------------------------------------------------
    if o.legend {
        let mut items: Vec<(String, String, bool)> = Vec::new();
        let value_name = if val_title.is_empty() {
            "Value".to_string()
        } else {
            val_title.clone()
        };
        if o.highlight_vital_few && o.threshold > 0.0 {
            items.push((
                vital_color.to_string(),
                format!("Vital few (to {}%)", fmt_value(o.threshold, dec)),
                false,
            ));
        }
        items.push((bar_color.to_string(), value_name, false));
        if o.show_cumulative {
            items.push((line_color.to_string(), "Cumulative %".to_string(), true));
        }
        if o.threshold > 0.0 {
            items.push((
                threshold_color.to_string(),
                format!("{}% threshold", fmt_value(o.threshold, dec)),
                true,
            ));
        }
        let sw = fs * 0.9;
        let gap = fs * 1.2;
        let widths: Vec<f64> = items
            .iter()
            .map(|(_, t, _)| sw + 6.0 + t.chars().count() as f64 * char_w * 0.9)
            .collect();
        let total_w: f64 = widths.iter().sum::<f64>() + gap * (items.len().max(1) - 1) as f64;
        let mut lx = (w - total_w) / 2.0;
        let ly = h - pad - fs * 0.5;
        s.push_str(&format!(
            "<g font-family=\"system-ui, -apple-system, Segoe UI, Roboto, sans-serif\" font-size=\"{}\" fill=\"{}\">",
            c(fs * 0.9),
            escape_xml(&th.text)
        ));
        for (k, (color, text, is_line)) in items.iter().enumerate() {
            if *is_line {
                s.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"2.5\"/>",
                    c(lx), c(ly - fs * 0.3), c(lx + sw), c(ly - fs * 0.3), escape_xml(color)
                ));
            } else {
                s.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                    c(lx),
                    c(ly - sw),
                    c(sw),
                    c(sw),
                    escape_xml(color)
                ));
            }
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\">{}</text>",
                c(lx + sw + 6.0),
                c(ly),
                escape_xml(text)
            ));
            lx += widths[k] + gap;
        }
        s.push_str("</g>");
    }

    s.push_str("</svg>");
    Ok(s)
}

fn render_summary(a: &Analysis, o: &Options) -> String {
    let dec = o.decimals.min(6);
    let (_, val_title) = axis_titles(a, o);
    let value_head = if val_title.is_empty() {
        "Value".to_string()
    } else {
        val_title
    };
    let headers = [
        "#".to_string(),
        "Category".to_string(),
        value_head,
        "% of total".to_string(),
        "Cumulative".to_string(),
        "Cumulative %".to_string(),
        "Vital".to_string(),
    ];
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(a.rows.len() + 1);
    for (i, r) in a.rows.iter().enumerate() {
        rows.push(vec![
            (i + 1).to_string(),
            r.label.clone(),
            fmt_value(r.value, dec),
            fmt_pct(r.percent, dec),
            fmt_value(r.cumulative, dec),
            fmt_pct(r.cumulative_percent, dec),
            if o.threshold > 0.0 && r.vital {
                "yes".into()
            } else {
                "".into()
            },
        ]);
    }
    rows.push(vec![
        "".into(),
        "TOTAL".into(),
        fmt_value(a.total, dec),
        fmt_pct(100.0, dec),
        fmt_value(a.total, dec),
        fmt_pct(100.0, dec),
        "".into(),
    ]);

    let cols = headers.len();
    let mut widths = vec![0usize; cols];
    for (k, h) in headers.iter().enumerate() {
        widths[k] = h.chars().count();
    }
    for r in &rows {
        for (k, cell) in r.iter().enumerate() {
            widths[k] = widths[k].max(cell.chars().count());
        }
    }
    // Numeric columns read right-aligned; the label column stays left-aligned.
    let right = [true, false, true, true, true, true, false];
    let render_line = |cells: &[String]| -> String {
        let mut parts = Vec::with_capacity(cols);
        for (k, cell) in cells.iter().enumerate() {
            let pad = widths[k] - cell.chars().count();
            parts.push(if right[k] {
                format!("{}{}", " ".repeat(pad), cell)
            } else {
                format!("{}{}", cell, " ".repeat(pad))
            });
        }
        parts.join("  ").trim_end().to_string()
    };

    let mut out = String::new();
    if !o.title.trim().is_empty() {
        out.push_str(o.title.trim());
        out.push('\n');
        out.push('\n');
    }
    out.push_str(&render_line(&headers));
    out.push('\n');
    out.push_str(&format!(
        "{}\n",
        widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("  ")
    ));
    for r in &rows {
        out.push_str(&render_line(r));
        out.push('\n');
    }

    out.push('\n');
    let vital_count = a.rows.iter().filter(|r| r.vital).count();
    if o.threshold > 0.0 {
        match a.crossing_index {
            Some(i) => out.push_str(&format!(
                "Vital few: {} of {} categories reach {}% of the total ({}% at `{}`).",
                vital_count,
                a.rows.len(),
                fmt_value(o.threshold, dec),
                fmt_pct(a.rows[i].cumulative_percent, dec),
                a.rows[i].label
            )),
            None => out.push_str(&format!(
                "No category block reaches the {}% threshold.",
                fmt_value(o.threshold, dec)
            )),
        }
    } else {
        out.push_str("Threshold is 0, so no vital-few block is marked.");
    }
    out.push('\n');
    out
}

fn render_json(a: &Analysis, o: &Options) -> Result<String, String> {
    let dec = o.decimals.min(6) as usize;
    let round = |v: f64| (v * 10f64.powi(dec as i32 + 4)).round() / 10f64.powi(dec as i32 + 4);
    let mut s = String::from("{\"total\":");
    s.push_str(&round(a.total).to_string());
    s.push_str(",\"categories\":");
    s.push_str(&a.rows.len().to_string());
    s.push_str(",\"threshold\":");
    s.push_str(&o.threshold.to_string());
    s.push_str(",\"sort\":\"");
    s.push_str(&escape_json(&o.sort));
    s.push_str("\",\"crossing_index\":");
    match a.crossing_index {
        Some(i) => s.push_str(&i.to_string()),
        None => s.push_str("null"),
    }
    s.push_str(",\"crossing_label\":");
    match a.crossing_index {
        Some(i) => {
            s.push('"');
            s.push_str(&escape_json(&a.rows[i].label));
            s.push('"');
        }
        None => s.push_str("null"),
    }
    s.push_str(",\"vital_few_count\":");
    s.push_str(&a.rows.iter().filter(|r| r.vital).count().to_string());
    s.push_str(",\"rows\":[");
    for (i, r) in a.rows.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"rank\":{},\"label\":\"{}\",\"value\":{},\"percent\":{},\"cumulative\":{},\"cumulative_percent\":{},\"vital\":{},\"other\":{}}}",
            i + 1,
            escape_json(&r.label),
            round(r.value),
            round(r.percent),
            round(r.cumulative),
            round(r.cumulative_percent),
            r.vital,
            r.is_other
        ));
    }
    s.push_str("]}");
    Ok(s)
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Late delivery,45\nWrong item,30\nDamaged,15\nBilling error,7\nRude staff,3";

    #[test]
    fn sorts_descending_and_builds_cumulative_percentages() {
        let a = analyze("Damaged,15\nLate delivery,45\nWrong item,30", &Options::default()).unwrap();
        assert_eq!(a.total, 90.0);
        assert_eq!(a.rows[0].label, "Late delivery");
        assert_eq!(a.rows[0].value, 45.0);
        assert!((a.rows[0].cumulative_percent - 50.0).abs() < 1e-9);
        assert!((a.rows[1].cumulative_percent - 83.333_333_333).abs() < 1e-6);
        assert!((a.rows[2].cumulative_percent - 100.0).abs() < 1e-9);
        // 45 + 30 = 83.3% clears 80%, so the vital few is the first two rows.
        assert_eq!(a.crossing_index, Some(1));
        assert!(a.rows[0].vital && a.rows[1].vital && !a.rows[2].vital);
    }

    #[test]
    fn renders_svg_with_bars_a_cumulative_line_and_a_threshold() {
        let svg = render(SAMPLE, &Options::default()).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.ends_with("</svg>"));
        // Bars carry a <title> tooltip, so they are the only closing </rect> tags.
        assert_eq!(svg.matches("</rect>").count(), 5, "one bar per category");
        assert!(svg.contains("<polyline points="), "cumulative line drawn");
        assert!(svg.contains("stroke-dasharray=\"6 4\""), "threshold drawn");
        assert!(svg.contains("Late delivery"));
        // Deterministic: the same input renders byte-identically.
        assert_eq!(svg, render(SAMPLE, &Options::default()).unwrap());
    }

    #[test]
    fn summary_reports_the_vital_few() {
        let o = Options {
            output: "summary".into(),
            ..Default::default()
        };
        let out = render(SAMPLE, &o).unwrap();
        assert!(out.contains("Late delivery"));
        assert!(out.contains("TOTAL"));
        assert!(
            out.contains("Vital few: 3 of 5 categories reach 80% of the total (90.0% at `Damaged`)."),
            "got:\n{out}"
        );
    }

    #[test]
    fn json_exposes_the_crossing_row() {
        let o = Options {
            output: "json".into(),
            ..Default::default()
        };
        let out = render(SAMPLE, &o).unwrap();
        assert!(out.contains("\"crossing_index\":2"), "got {out}");
        assert!(out.contains("\"crossing_label\":\"Damaged\""));
        assert!(out.contains("\"vital_few_count\":3"));
        assert!(out.contains("\"total\":100"));
    }

    #[test]
    fn detects_tab_and_semicolon_tables() {
        for data in ["A\t3\nB\t1", "A;3\nB;1"] {
            let a = analyze(data, &Options::default()).unwrap();
            assert_eq!(a.rows.len(), 2);
            assert_eq!(a.rows[0].value, 3.0);
        }
    }

    #[test]
    fn whitespace_rows_keep_multi_word_labels() {
        let a = analyze("Late delivery 45\nWrong item 30", &Options::default()).unwrap();
        assert_eq!(a.rows[0].label, "Late delivery");
        assert_eq!(a.rows[1].label, "Wrong item");
    }

    #[test]
    fn skips_a_header_row_and_reuses_it_as_axis_titles() {
        let a = analyze("Reason,Count\nA,3\nB,1", &Options::default()).unwrap();
        assert_eq!(a.rows.len(), 2);
        assert_eq!(
            a.header_names,
            Some(("Reason".to_string(), "Count".to_string()))
        );
        let (cat, val) = axis_titles(&a, &Options::default());
        assert_eq!((cat.as_str(), val.as_str()), ("Reason", "Count"));
    }

    #[test]
    fn header_no_keeps_a_numeric_first_row() {
        let o = Options {
            header: "no".into(),
            ..Default::default()
        };
        let a = analyze("A,3\nB,1", &o).unwrap();
        assert_eq!(a.rows.len(), 2);
    }

    #[test]
    fn duplicate_labels_are_summed() {
        let a = analyze("A,3\nB,1\nA,4", &Options::default()).unwrap();
        assert_eq!(a.rows.len(), 2);
        assert_eq!(a.rows[0].label, "A");
        assert_eq!(a.rows[0].value, 7.0);
    }

    #[test]
    fn messy_numbers_parse() {
        let a = analyze("A,\"1,250\"\nB,$300.50\nC,12%", &Options::default()).unwrap();
        assert_eq!(a.rows[0].value, 1250.0);
        assert_eq!(a.rows[1].value, 300.5);
        assert_eq!(a.rows[2].value, 12.0);
    }

    #[test]
    fn max_categories_buckets_the_tail_into_other() {
        let o = Options {
            max_categories: 2,
            ..Default::default()
        };
        let a = analyze(SAMPLE, &o).unwrap();
        assert_eq!(a.rows.len(), 3);
        assert_eq!(a.rows[2].label, "Other");
        assert_eq!(a.rows[2].value, 25.0);
        assert!(a.rows[2].is_other);
        assert_eq!(a.total, 100.0);
    }

    #[test]
    fn sort_input_and_asc_are_honoured() {
        let o = Options {
            sort: "input".into(),
            ..Default::default()
        };
        assert_eq!(analyze(SAMPLE, &o).unwrap().rows[0].label, "Late delivery");
        let o = Options {
            sort: "asc".into(),
            ..Default::default()
        };
        assert_eq!(analyze(SAMPLE, &o).unwrap().rows[0].label, "Rude staff");
    }

    #[test]
    fn threshold_zero_hides_the_vital_few_and_the_line() {
        let o = Options {
            threshold: 0.0,
            ..Default::default()
        };
        let a = analyze(SAMPLE, &o).unwrap();
        assert!(a.rows.iter().all(|r| !r.vital));
        assert_eq!(a.crossing_index, None);
        let svg = render(SAMPLE, &o).unwrap();
        assert!(!svg.contains("stroke-dasharray"));
    }

    #[test]
    fn show_cumulative_false_drops_the_line_and_the_right_axis() {
        let o = Options {
            show_cumulative: false,
            ..Default::default()
        };
        let svg = render(SAMPLE, &o).unwrap();
        assert!(!svg.contains("<polyline"));
        assert!(!svg.contains("Cumulative %"));
    }

    #[test]
    fn dark_theme_and_custom_colors_reach_the_svg() {
        let o = Options {
            theme: "dark".into(),
            color: "steelblue".into(),
            line_color: "#0f0".into(),
            title: "Q3 defects".into(),
            ..Default::default()
        };
        let svg = render(SAMPLE, &o).unwrap();
        assert!(svg.contains("fill=\"#0f172a\""));
        assert!(svg.contains("fill=\"steelblue\""));
        assert!(svg.contains("stroke=\"#0f0\""));
        assert!(svg.contains("Q3 defects"));
    }

    #[test]
    fn labels_are_xml_escaped() {
        let svg = render("A &amp; <b>,5\nB,1", &Options::default()).unwrap();
        assert!(svg.contains("A &amp;amp; &lt;b&gt;"));
        assert!(!svg.contains("<b>"));
    }

    #[test]
    fn rejects_empty_input() {
        let err = analyze("   \n # only a comment\n", &Options::default()).unwrap_err();
        assert!(err.contains("no data rows found"), "got {err}");
    }

    #[test]
    fn rejects_a_row_without_a_number() {
        let err = analyze("A,3\nB,not-a-number", &Options::default()).unwrap_err();
        assert!(err.contains("line 2"), "got {err}");
        assert!(err.contains("expected `label,value`"), "got {err}");
    }

    #[test]
    fn rejects_negative_values() {
        let err = analyze("A,3\nB,-2", &Options::default()).unwrap_err();
        assert!(err.contains("zero or positive"), "got {err}");
    }

    #[test]
    fn rejects_an_all_zero_table() {
        let err = analyze("A,0\nB,0", &Options::default()).unwrap_err();
        assert!(err.contains("at least one category above zero"), "got {err}");
    }

    #[test]
    fn rejects_a_bad_threshold_and_a_bad_output() {
        let o = Options {
            threshold: 140.0,
            ..Default::default()
        };
        assert!(render(SAMPLE, &o)
            .unwrap_err()
            .contains("threshold must be between 0 and 100"));
        let o = Options {
            output: "png".into(),
            ..Default::default()
        };
        assert!(render(SAMPLE, &o)
            .unwrap_err()
            .contains("output must be svg, summary, or json"));
    }

    #[test]
    fn rejects_a_canvas_too_small_for_its_labels() {
        let o = Options {
            width: 320,
            height: 240,
            font_size: 48.0,
            label_angle: 90.0,
            ..Default::default()
        };
        let err = render(SAMPLE, &o).unwrap_err();
        assert!(err.contains("chart area collapsed"), "got {err}");
    }

    #[test]
    fn rejects_too_many_rows() {
        let big = (0..MAX_ROWS + 1)
            .map(|i| format!("L{i},1"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(analyze(&big, &Options::default())
            .unwrap_err()
            .contains("too many rows"));
    }

    #[test]
    fn rejects_too_many_categories() {
        let big = (0..MAX_CATEGORIES + 1)
            .map(|i| format!("L{i},1"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(analyze(&big, &Options::default())
            .unwrap_err()
            .contains("too many categories"));
    }

    #[test]
    fn nice_max_rounds_up_to_readable_steps() {
        assert_eq!(nice_max(45.0, 5.0), 50.0);
        assert_eq!(nice_max(9.0, 5.0), 10.0);
        assert_eq!(nice_max(0.0, 5.0), 1.0);
        assert_eq!(nice_max(230.0, 5.0), 250.0);
    }

    #[test]
    fn value_labels_and_cumulative_labels_are_optional() {
        let o = Options {
            show_values: true,
            show_cumulative_labels: true,
            ..Default::default()
        };
        let svg = render(SAMPLE, &o).unwrap();
        assert!(svg.contains(">45</text>"));
        assert!(svg.contains(">45.0%</text>"));
    }
}
