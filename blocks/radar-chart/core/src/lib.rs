//! Radar (spider) chart layout + deterministic SVG rendering.
//!
//! Takes a pasted table — a wide `series,Axis1,Axis2,…` matrix, a long
//! `series,axis,value` list, or a single-series `axis,value` list — normalizes each
//! value onto a radial scale, and renders a self-contained SVG web/polygon chart.
//! No external crates, no I/O, no clock: identical input always yields identical output.

pub const MAX_ROWS: usize = 5_000;
pub const MAX_AXES: usize = 60;
pub const MIN_AXES: usize = 3;
pub const MAX_SERIES: usize = 24;

/// Every knob the block, CLI, and page expose.
#[derive(Clone, Debug)]
pub struct Options {
    pub layout: String,
    pub scale: String,
    pub scale_min: f64,
    pub scale_max: f64,
    pub rings: u32,
    pub grid_shape: String,
    pub show_spokes: bool,
    pub show_axis_labels: bool,
    pub show_ticks: bool,
    pub show_values: bool,
    pub fill_opacity: f64,
    pub line_width: f64,
    pub point_radius: f64,
    pub start_angle: f64,
    pub direction: String,
    pub palette: String,
    pub colors: String,
    pub background: String,
    pub title: String,
    pub legend: bool,
    pub font_size: f64,
    pub width: u32,
    pub height: u32,
    pub theme: String,
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            layout: "auto".into(),
            scale: "shared".into(),
            scale_min: 0.0,
            scale_max: 0.0,
            rings: 5,
            grid_shape: "polygon".into(),
            show_spokes: true,
            show_axis_labels: true,
            show_ticks: true,
            show_values: false,
            fill_opacity: 0.25,
            line_width: 2.0,
            point_radius: 3.0,
            start_angle: 0.0,
            direction: "clockwise".into(),
            palette: "default".into(),
            colors: "".into(),
            background: "".into(),
            title: "".into(),
            legend: true,
            font_size: 13.0,
            width: 700,
            height: 560,
            theme: "light".into(),
            output: "svg".into(),
        }
    }
}

// ---------------------------------------------------------------- parsing

/// Split one pasted row into cells. Tab, comma and semicolon give full multi-column
/// splits; a plain space row is read as exactly two cells (`label value`) so that
/// multi-word axis names survive.
fn split_fields(line: &str) -> Vec<String> {
    let t = line.trim_end();
    if t.contains('\t') {
        t.split('\t').map(|s| s.trim().to_string()).collect()
    } else if t.contains(',') {
        t.split(',').map(|s| s.trim().to_string()).collect()
    } else if t.contains(';') {
        t.split(';').map(|s| s.trim().to_string()).collect()
    } else if let Some(p) = t.trim_end().rfind(char::is_whitespace) {
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

fn is_num(tok: &str) -> bool {
    parse_value(tok).is_some()
}

/// One plotted series: a name plus one value per axis (missing cells count as the axis floor).
#[derive(Clone, Debug)]
pub struct Series {
    pub name: String,
    pub values: Vec<f64>,
    pub present: Vec<bool>,
}

#[derive(Clone, Debug)]
pub struct Chart {
    pub axes: Vec<String>,
    pub series: Vec<Series>,
    pub layout: String,
}

fn read_rows(data: &str) -> Result<Vec<(usize, Vec<String>)>, String> {
    let mut raw: Vec<(usize, Vec<String>)> = Vec::new();
    for (i, line) in data.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        raw.push((i + 1, split_fields(line)));
        if raw.len() > MAX_ROWS {
            return Err(format!(
                "too many rows: this tool accepts at most {MAX_ROWS} data rows, trim the table before pasting"
            ));
        }
    }
    if raw.is_empty() {
        return Err(
            "data is empty: paste a header row of axis names and one row of numbers per series"
                .into(),
        );
    }
    Ok(raw)
}

fn detect_layout(rows: &[(usize, Vec<String>)]) -> &'static str {
    let max_cols = rows.iter().map(|(_, f)| f.len()).max().unwrap_or(0);
    if max_cols <= 2 {
        return "single";
    }
    // `series,axis,value` rows have a non-numeric middle cell on every data row.
    let body: Vec<&(usize, Vec<String>)> = rows.iter().collect();
    let long_shaped = max_cols == 3
        && body
            .iter()
            .skip(1)
            .all(|(_, f)| f.len() == 3 && !is_num(&f[1]) && is_num(&f[2]));
    if long_shaped && rows.len() > 1 {
        return "long";
    }
    "wide"
}

fn parse_wide(rows: &[(usize, Vec<String>)]) -> Result<Chart, String> {
    // A header row is one whose cells after the first are all non-numeric.
    let (header, body) = {
        let (_, first) = &rows[0];
        let looks_header = first.len() >= 2 && first[1..].iter().all(|c| !is_num(c));
        if looks_header {
            (Some(first.clone()), &rows[1..])
        } else {
            (None, &rows[..])
        }
    };
    if body.is_empty() {
        return Err(
            "no data rows found after the header row: add one row of numbers per series".into(),
        );
    }

    let cols = body.iter().map(|(_, f)| f.len()).max().unwrap_or(0);
    let axes: Vec<String> = match &header {
        Some(h) => h[1..].iter().map(|s| s.to_string()).collect(),
        None => (1..cols).map(|i| format!("Axis {i}")).collect(),
    };
    if axes.is_empty() {
        return Err(
            "no axis names found: the header row should read `series,Axis 1,Axis 2,Axis 3`".into(),
        );
    }

    let mut series: Vec<Series> = Vec::new();
    for (ln, f) in body {
        if f.len() < 2 {
            return Err(format!(
                "line {ln}: expected a series name followed by one number per axis, found only `{}`",
                f.first().cloned().unwrap_or_default()
            ));
        }
        let name = f[0].trim().to_string();
        let cells = &f[1..];
        if cells.len() != axes.len() {
            return Err(format!(
                "line {ln}: series `{name}` has {} value(s) but there are {} axes ({}) — every row needs one number per axis",
                cells.len(),
                axes.len(),
                axes.join(", ")
            ));
        }
        let mut values = Vec::with_capacity(cells.len());
        for (i, c) in cells.iter().enumerate() {
            match parse_value(c) {
                Some(v) => values.push(v),
                None => {
                    return Err(format!(
                        "line {ln}: expected a number for `{name}` on axis `{}`, got `{c}`",
                        axes[i]
                    ))
                }
            }
        }
        let present = vec![true; values.len()];
        series.push(Series {
            name: if name.is_empty() {
                format!("Series {}", series.len() + 1)
            } else {
                name
            },
            values,
            present,
        });
    }
    Ok(Chart {
        axes,
        series,
        layout: "wide".into(),
    })
}

fn parse_long(rows: &[(usize, Vec<String>)]) -> Result<Chart, String> {
    let body = if rows[0].1.len() == 3 && !is_num(&rows[0].1[2]) {
        &rows[1..]
    } else {
        &rows[..]
    };
    if body.is_empty() {
        return Err("no data rows found after the header row: add `series,axis,value` rows".into());
    }

    let mut axes: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut cells: Vec<(usize, usize, f64)> = Vec::new();
    for (ln, f) in body {
        if f.len() != 3 {
            return Err(format!(
                "line {ln}: long layout expects exactly `series,axis,value` (3 columns), found {}",
                f.len()
            ));
        }
        let (s, a, v) = (f[0].trim(), f[1].trim(), f[2].trim());
        let value = parse_value(v).ok_or_else(|| {
            format!("line {ln}: expected a number for `{s}` on axis `{a}`, got `{v}`")
        })?;
        let si = match names.iter().position(|n| n == s) {
            Some(i) => i,
            None => {
                names.push(s.to_string());
                names.len() - 1
            }
        };
        let ai = match axes.iter().position(|n| n == a) {
            Some(i) => i,
            None => {
                axes.push(a.to_string());
                axes.len() - 1
            }
        };
        cells.push((si, ai, value));
    }

    let mut series: Vec<Series> = names
        .into_iter()
        .enumerate()
        .map(|(i, n)| Series {
            name: if n.is_empty() {
                format!("Series {}", i + 1)
            } else {
                n
            },
            values: vec![0.0; axes.len()],
            present: vec![false; axes.len()],
        })
        .collect();
    for (si, ai, v) in cells {
        series[si].values[ai] = v;
        series[si].present[ai] = true;
    }
    Ok(Chart {
        axes,
        series,
        layout: "long".into(),
    })
}

fn parse_single(rows: &[(usize, Vec<String>)]) -> Result<Chart, String> {
    let first_numeric = rows[0].1.last().map(|s| is_num(s)).unwrap_or(false);
    let any_later_numeric = rows
        .iter()
        .skip(1)
        .any(|(_, f)| f.last().map(|s| is_num(s)).unwrap_or(false));
    let body = if !first_numeric {
        if !any_later_numeric {
            let (ln, f) = &rows[0];
            return Err(format!(
                "line {ln}: expected a number in the last column, got `{}` — rows look like `Camera,8`",
                f.last().cloned().unwrap_or_default()
            ));
        }
        &rows[1..]
    } else {
        &rows[..]
    };
    if body.is_empty() {
        return Err("no data rows found after the header row: add `axis,value` rows".into());
    }

    let mut axes = Vec::with_capacity(body.len());
    let mut values = Vec::with_capacity(body.len());
    for (ln, f) in body {
        if f.len() < 2 {
            return Err(format!(
                "line {ln}: expected `axis,value` but found only one field (`{}`)",
                f.first().cloned().unwrap_or_default()
            ));
        }
        let name = f[..f.len() - 1].join(" ").trim().to_string();
        let last = f.last().unwrap();
        let value = parse_value(last).ok_or_else(|| {
            format!("line {ln}: expected a number for axis `{name}`, got `{last}`")
        })?;
        axes.push(if name.is_empty() {
            format!("Axis {}", axes.len() + 1)
        } else {
            name
        });
        values.push(value);
    }
    let present = vec![true; values.len()];
    Ok(Chart {
        axes,
        series: vec![Series {
            name: "Series 1".into(),
            values,
            present,
        }],
        layout: "single".into(),
    })
}

pub fn parse(data: &str, opts: &Options) -> Result<Chart, String> {
    let rows = read_rows(data)?;
    let layout = match opts.layout.trim() {
        "" | "auto" => detect_layout(&rows),
        "wide" => "wide",
        "long" => "long",
        "single" => "single",
        other => {
            return Err(format!(
                "unknown layout `{other}`: expected auto, wide, long, or single"
            ))
        }
    };
    let chart = match layout {
        "long" => parse_long(&rows)?,
        "single" => parse_single(&rows)?,
        _ => parse_wide(&rows)?,
    };

    if chart.axes.len() < MIN_AXES {
        return Err(format!(
            "a radar chart needs at least {MIN_AXES} axes, found {} ({}) — add more columns, or use a bar chart for one or two measures",
            chart.axes.len(),
            if chart.axes.is_empty() {
                "none".to_string()
            } else {
                chart.axes.join(", ")
            }
        ));
    }
    if chart.axes.len() > MAX_AXES {
        return Err(format!(
            "too many axes: {} exceeds the {MAX_AXES} axis cap — a radar chart stays readable up to about 8 axes",
            chart.axes.len()
        ));
    }
    if chart.series.len() > MAX_SERIES {
        return Err(format!(
            "too many series: {} exceeds the {MAX_SERIES} series cap — overlay 2 to 4 for a readable chart",
            chart.series.len()
        ));
    }
    Ok(chart)
}

// ---------------------------------------------------------------- scale

/// Inclusive value domain for one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Domain {
    pub min: f64,
    pub max: f64,
}

impl Domain {
    fn t(&self, v: f64) -> f64 {
        let range = self.max - self.min;
        if range <= 0.0 {
            return if v > self.min { 1.0 } else { 0.0 };
        }
        ((v - self.min) / range).clamp(0.0, 1.0)
    }
}

/// Nudge an auto maximum up to a round-ish number so ring ticks read cleanly.
fn nice_max(v: f64) -> f64 {
    if !v.is_finite() || v <= 0.0 {
        return 1.0;
    }
    let mag = 10f64.powf(v.log10().floor());
    let n = v / mag;
    let step = if n <= 1.0 {
        1.0
    } else if n <= 2.0 {
        2.0
    } else if n <= 2.5 {
        2.5
    } else if n <= 5.0 {
        5.0
    } else {
        10.0
    };
    step * mag
}

pub fn domains(chart: &Chart, opts: &Options) -> Result<Vec<Domain>, String> {
    let mode = match opts.scale.trim() {
        "" => "shared",
        m @ ("shared" | "per_axis" | "percent") => m,
        other => {
            return Err(format!(
                "unknown scale `{other}`: expected shared, per_axis, or percent"
            ))
        }
    };
    let n = chart.axes.len();
    let smin = if opts.scale_min.is_finite() {
        opts.scale_min
    } else {
        0.0
    };

    if mode == "percent" {
        let max = if opts.scale_max > smin {
            opts.scale_max
        } else {
            100.0
        };
        return Ok(vec![Domain { min: smin, max }; n]);
    }

    if opts.scale_max != 0.0 && opts.scale_max <= smin {
        return Err(format!(
            "scale_max ({}) must be greater than scale_min ({}) — leave scale_max at 0 to derive it from the data",
            fmt_value(opts.scale_max),
            fmt_value(smin)
        ));
    }

    if mode == "per_axis" {
        let mut out = Vec::with_capacity(n);
        for a in 0..n {
            let peak = chart
                .series
                .iter()
                .map(|s| s.values[a])
                .fold(f64::NEG_INFINITY, f64::max);
            let max = if opts.scale_max > 0.0 {
                opts.scale_max
            } else {
                peak
            };
            out.push(Domain {
                min: smin,
                max: if max > smin { max } else { smin + 1.0 },
            });
        }
        return Ok(out);
    }

    let peak = chart
        .series
        .iter()
        .flat_map(|s| s.values.iter().copied())
        .fold(f64::NEG_INFINITY, f64::max);
    let max = if opts.scale_max > 0.0 {
        opts.scale_max
    } else {
        nice_max(peak - smin) + smin
    };
    Ok(vec![
        Domain {
            min: smin,
            max: if max > smin { max } else { smin + 1.0 },
        };
        n
    ])
}

// ---------------------------------------------------------------- colors

fn palette_of(name: &str) -> &'static [&'static str] {
    match name {
        "pastel" => &[
            "#818cf8", "#38bdf8", "#5eead4", "#86efac", "#fde047", "#fdba74", "#fca5a5", "#f9a8d4",
            "#d8b4fe", "#a5b4fc",
        ],
        "dusk" => &[
            "#1e3a8a", "#075985", "#115e59", "#3f6212", "#854d0e", "#7c2d12", "#9f1239", "#831843",
            "#581c87", "#312e81",
        ],
        "earth" => &[
            "#8c6d46", "#a9884f", "#6f7f4b", "#4f6b52", "#93704a", "#b08968", "#7d6b57", "#5c5343",
            "#9c6f44", "#66584a",
        ],
        "ocean" => &[
            "#0c4a6e", "#0369a1", "#0891b2", "#06b6d4", "#14b8a6", "#2dd4bf", "#38bdf8", "#1d4ed8",
            "#3b82f6", "#60a5fa",
        ],
        _ => &[
            "#2563eb", "#f97316", "#14b8a6", "#a855f7", "#22c55e", "#ef4444", "#eab308", "#ec4899",
            "#0ea5e9", "#6366f1",
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

/// The colour of each series, honouring an explicit `colors` list first.
pub fn series_colors(chart: &Chart, opts: &Options) -> Vec<String> {
    let overrides: Vec<String> = opts
        .colors
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let n = chart.series.len();
    let mono_base = {
        let first = overrides.first().cloned().unwrap_or_default();
        if first.is_empty() {
            "#2563eb".to_string()
        } else {
            first
        }
    };
    (0..n)
        .map(|i| {
            if !overrides.is_empty() && opts.palette.trim() != "mono" {
                return overrides[i % overrides.len()].clone();
            }
            if opts.palette.trim() == "mono" {
                let t = if n <= 1 {
                    0.0
                } else {
                    0.55 * (i as f64) / ((n - 1) as f64)
                };
                return lighten(&mono_base, t);
            }
            let pal = palette_of(opts.palette.trim());
            pal[i % pal.len()].to_string()
        })
        .collect()
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

// ---------------------------------------------------------------- geometry

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Screen angle in radians for axis `i`; axis 0 sits at the top by default.
fn axis_angle(i: usize, n: usize, opts: &Options) -> f64 {
    let dir = if opts.direction.trim() == "counterclockwise" {
        -1.0
    } else {
        1.0
    };
    let deg = opts.start_angle - 90.0 + dir * (i as f64) * 360.0 / (n as f64);
    deg * std::f64::consts::PI / 180.0
}

fn polar(cx: f64, cy: f64, r: f64, ang: f64) -> Point {
    Point {
        x: cx + r * ang.cos(),
        y: cy + r * ang.sin(),
    }
}

struct Theme {
    bg: String,
    text: String,
    muted: String,
    grid: String,
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
        grid: if dark { "#334155" } else { "#cbd5e1" }.into(),
        stroke: if dark { "#0f172a" } else { "#ffffff" }.into(),
    }
}

// ---------------------------------------------------------------- render

pub fn render(data: &str, opts: &Options) -> Result<String, String> {
    let chart = parse(data, opts)?;
    let doms = domains(&chart, opts)?;
    let colors = series_colors(&chart, opts);

    let width = (opts.width.max(1) as f64).clamp(320.0, 2400.0);
    let height = (opts.height.max(1) as f64).clamp(240.0, 1800.0);
    let font = opts.font_size.clamp(6.0, 48.0);
    let rings = opts.rings.min(10);
    let fill_opacity = opts.fill_opacity.clamp(0.0, 1.0);
    let line_width = opts.line_width.clamp(0.0, 8.0);
    let point_radius = opts.point_radius.clamp(0.0, 12.0);

    let margin = 12.0;
    let title = opts.title.trim().to_string();
    let title_size = (font * 1.45).max(12.0);
    let mut top = margin;
    if !title.is_empty() {
        top += title_size + 10.0;
    }
    let legend_h = font + 16.0;
    let mut bottom = margin;
    if opts.legend {
        bottom += legend_h;
    }
    let plot_w = width - 2.0 * margin;
    let plot_h = height - top - bottom;

    // Axis captions live outside the outer ring, so reserve room for them.
    let (pad_x, pad_y) = if opts.show_axis_labels {
        (font * 4.6, font * 2.4)
    } else {
        (font * 0.6, font * 0.6)
    };
    let radius = (plot_w / 2.0 - pad_x).min(plot_h / 2.0 - pad_y);
    if radius < 30.0 {
        return Err(format!(
            "chart area is too small: {}x{} leaves a radius of {} after the title, legend and axis captions — increase width/height, lower font_size, or turn off axis labels",
            fmt_value(width),
            fmt_value(height),
            fmt_value(radius.max(0.0))
        ));
    }
    let cx = margin + plot_w / 2.0;
    let cy = top + plot_h / 2.0;

    match opts.output.trim() {
        "summary" => Ok(render_summary(&chart, &doms, opts)),
        "json" => Ok(render_json(
            &chart, &doms, &colors, cx, cy, radius, width, height, opts,
        )),
        "svg" | "" => Ok(render_svg(
            &chart,
            &doms,
            &colors,
            RenderGeom {
                width,
                height,
                cx,
                cy,
                radius,
                font,
                title_size,
                legend_h,
                rings,
                fill_opacity,
                line_width,
                point_radius,
            },
            opts,
        )),
        other => Err(format!(
            "unknown output `{other}`: expected svg, summary, or json"
        )),
    }
}

struct RenderGeom {
    width: f64,
    height: f64,
    cx: f64,
    cy: f64,
    radius: f64,
    font: f64,
    title_size: f64,
    legend_h: f64,
    rings: u32,
    fill_opacity: f64,
    line_width: f64,
    point_radius: f64,
}

fn ring_path(cx: f64, cy: f64, r: f64, n: usize, opts: &Options) -> String {
    let mut d = String::new();
    for i in 0..n {
        let p = polar(cx, cy, r, axis_angle(i, n, opts));
        d.push_str(&format!(
            "{}{} {}",
            if i == 0 { "M" } else { " L" },
            round2(p.x),
            round2(p.y)
        ));
    }
    d.push_str(" Z");
    d
}

fn render_svg(
    chart: &Chart,
    doms: &[Domain],
    colors: &[String],
    g: RenderGeom,
    opts: &Options,
) -> String {
    let th = theme_of(opts);
    let n = chart.axes.len();
    let circle_grid = opts.grid_shape.trim() == "circle";
    let mut s = String::with_capacity(4096 + n * chart.series.len() * 160);

    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" role=\"img\" aria-label=\"Radar chart\">\n",
        fmt_value(g.width), fmt_value(g.height), fmt_value(g.width), fmt_value(g.height)
    ));
    s.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
        fmt_value(g.width),
        fmt_value(g.height),
        esc_xml(&th.bg)
    ));
    s.push_str(&format!(
        "<g font-family=\"system-ui,-apple-system,Segoe UI,Roboto,Helvetica,Arial,sans-serif\" font-size=\"{}\">\n",
        round2(g.font)
    ));

    let title = opts.title.trim();
    if !title.is_empty() {
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"{}\" font-weight=\"600\" text-anchor=\"middle\" fill=\"{}\">{}</text>\n",
            round2(g.width / 2.0),
            round2(12.0 + g.title_size),
            round2(g.title_size),
            esc_xml(&th.text),
            esc_xml(title)
        ));
    }

    // --- grid rings
    let ring_count = g.rings.max(1);
    for k in 1..=ring_count {
        let r = g.radius * (k as f64) / (ring_count as f64);
        let outer = k == ring_count;
        let w = if outer { 1.4 } else { 1.0 };
        if circle_grid {
            s.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
                round2(g.cx), round2(g.cy), round2(r), esc_xml(&th.grid), w
            ));
        } else {
            s.push_str(&format!(
                "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
                ring_path(g.cx, g.cy, r, n, opts),
                esc_xml(&th.grid),
                w
            ));
        }
    }

    // --- spokes
    if opts.show_spokes {
        for i in 0..n {
            let p = polar(g.cx, g.cy, g.radius, axis_angle(i, n, opts));
            s.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>\n",
                round2(g.cx),
                round2(g.cy),
                round2(p.x),
                round2(p.y),
                esc_xml(&th.grid)
            ));
        }
    }

    // --- ring tick labels
    if opts.show_ticks && g.rings > 0 {
        let per_axis = opts.scale.trim() == "per_axis";
        for k in 1..=g.rings {
            let f = (k as f64) / (g.rings as f64);
            let r = g.radius * f;
            let label = if per_axis {
                format!("{:.0}%", f * 100.0)
            } else {
                fmt_value(doms[0].min + (doms[0].max - doms[0].min) * f)
            };
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\">{}</text>\n",
                round2(g.cx + 5.0),
                round2(g.cy - r + g.font * 0.34),
                round2(g.font * 0.82),
                esc_xml(&th.muted),
                esc_xml(&label)
            ));
        }
    }

    // --- axis captions
    if opts.show_axis_labels {
        for (i, name) in chart.axes.iter().enumerate() {
            let ang = axis_angle(i, n, opts);
            let p = polar(g.cx, g.cy, g.radius + g.font * 0.85, ang);
            let c = ang.cos();
            let anchor = if c.abs() < 0.2 {
                "middle"
            } else if c > 0.0 {
                "start"
            } else {
                "end"
            };
            let dy = g.font * 0.36 + ang.sin() * g.font * 0.5;
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"{}\" fill=\"{}\">{}</text>\n",
                round2(p.x),
                round2(p.y + dy),
                anchor,
                esc_xml(&th.text),
                esc_xml(name)
            ));
        }
    }

    // --- series polygons
    for (si, ser) in chart.series.iter().enumerate() {
        let color = &colors[si];
        let mut d = String::new();
        for i in 0..n {
            let t = doms[i].t(ser.values[i]);
            let p = polar(g.cx, g.cy, g.radius * t, axis_angle(i, n, opts));
            d.push_str(&format!(
                "{}{} {}",
                if i == 0 { "M" } else { " L" },
                round2(p.x),
                round2(p.y)
            ));
        }
        d.push_str(" Z");
        s.push_str(&format!(
            "<path d=\"{}\" fill=\"{}\" fill-opacity=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"><title>{}</title></path>\n",
            d,
            esc_xml(color),
            round2(g.fill_opacity),
            esc_xml(color),
            round2(g.line_width),
            esc_xml(&ser.name)
        ));
    }

    // --- vertex markers + value labels, drawn above every polygon
    for (si, ser) in chart.series.iter().enumerate() {
        let color = &colors[si];
        for i in 0..n {
            let t = doms[i].t(ser.values[i]);
            let ang = axis_angle(i, n, opts);
            let p = polar(g.cx, g.cy, g.radius * t, ang);
            if g.point_radius > 0.0 {
                s.push_str(&format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1\"><title>{}</title></circle>\n",
                    round2(p.x), round2(p.y), round2(g.point_radius),
                    esc_xml(color), esc_xml(&th.stroke),
                    esc_xml(&format!("{} · {}: {}", ser.name, chart.axes[i], fmt_value(ser.values[i])))
                ));
            }
            if opts.show_values {
                let lp = polar(
                    g.cx,
                    g.cy,
                    g.radius * t + g.point_radius + g.font * 0.75,
                    ang,
                );
                s.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" font-weight=\"600\" fill=\"{}\">{}</text>\n",
                    round2(lp.x),
                    round2(lp.y + g.font * 0.34),
                    round2(g.font * 0.85),
                    esc_xml(color),
                    esc_xml(&fmt_value(ser.values[i]))
                ));
            }
        }
    }

    // --- legend
    if opts.legend {
        let sw = g.font * 0.85;
        let gap = g.font * 1.1;
        let widths: Vec<f64> = chart
            .series
            .iter()
            .map(|s| sw + 6.0 + s.name.chars().count() as f64 * g.font * 0.55)
            .collect();
        let total: f64 = widths.iter().sum::<f64>() + gap * (widths.len().max(1) - 1) as f64;
        let mut x = (g.width - total) / 2.0;
        if x < 12.0 {
            x = 12.0;
        }
        let y = g.height - g.legend_h + (g.legend_h - sw) / 2.0 - 2.0;
        for (si, ser) in chart.series.iter().enumerate() {
            if x + widths[si] > g.width - 8.0 {
                break;
            }
            s.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{}\"/>\n",
                round2(x),
                round2(y),
                round2(sw),
                round2(sw),
                esc_xml(&colors[si])
            ));
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" fill=\"{}\">{}</text>\n",
                round2(x + sw + 6.0),
                round2(y + sw * 0.85),
                esc_xml(&th.muted),
                esc_xml(&ser.name)
            ));
            x += widths[si] + gap;
        }
    }

    s.push_str("</g>\n</svg>");
    s
}

fn render_summary(chart: &Chart, doms: &[Domain], opts: &Options) -> String {
    let headers = ["Series", "Axis", "Value", "Scaled", "Source"];
    let mut rows: Vec<[String; 5]> = Vec::new();
    for ser in &chart.series {
        for (i, axis) in chart.axes.iter().enumerate() {
            rows.push([
                ser.name.clone(),
                axis.clone(),
                fmt_value(ser.values[i]),
                format!("{:.1}%", doms[i].t(ser.values[i]) * 100.0),
                if ser.present[i] { "given" } else { "missing" }.to_string(),
            ]);
        }
    }
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
    let line = |r: &[String; 5]| {
        format!(
            "{}  {}  {}  {}  {}\n",
            pad_right(&r[0], w[0]),
            pad_right(&r[1], w[1]),
            pad_left(&r[2], w[2]),
            pad_left(&r[3], w[3]),
            pad_right(&r[4], w[4])
        )
    };

    let mut out = String::new();
    out.push_str(&format!(
        "{}  {}  {}  {}  {}\n",
        pad_right(headers[0], w[0]),
        pad_right(headers[1], w[1]),
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
        out.push_str(&line(r));
    }

    out.push('\n');
    for ser in &chart.series {
        let sum: f64 = ser.values.iter().sum();
        let mean = sum / (chart.axes.len() as f64);
        let cover: f64 = (0..chart.axes.len())
            .map(|i| doms[i].t(ser.values[i]))
            .sum::<f64>()
            / (chart.axes.len() as f64);
        out.push_str(&format!(
            "{}: mean {} · scaled area {:.1}%\n",
            ser.name,
            fmt_value(mean),
            cover * 100.0
        ));
    }
    out.push_str(&format!(
        "\naxes: {}\nseries: {}\nlayout: {}\nscale: {}\ndomain: {} to {}",
        chart.axes.len(),
        chart.series.len(),
        chart.layout,
        opts.scale.trim(),
        fmt_value(doms[0].min),
        if opts.scale.trim() == "per_axis" {
            "per-axis maximum".to_string()
        } else {
            fmt_value(doms[0].max)
        }
    ));
    out
}

#[allow(clippy::too_many_arguments)]
fn render_json(
    chart: &Chart,
    doms: &[Domain],
    colors: &[String],
    cx: f64,
    cy: f64,
    radius: f64,
    width: f64,
    height: f64,
    opts: &Options,
) -> String {
    let n = chart.axes.len();
    let mut s = String::with_capacity(1024 + n * chart.series.len() * 120);
    s.push_str("{\n  \"layout\": \"");
    s.push_str(&esc_json(&chart.layout));
    s.push_str("\",\n  \"scale\": \"");
    s.push_str(&esc_json(opts.scale.trim()));
    s.push_str(&format!(
        "\",\n  \"width\": {},\n  \"height\": {},\n  \"center\": {{ \"x\": {}, \"y\": {} }},\n  \"radius\": {},\n",
        fmt_json_num(width),
        fmt_json_num(height),
        round2(cx),
        round2(cy),
        round2(radius)
    ));

    s.push_str("  \"axes\": [\n");
    for (i, a) in chart.axes.iter().enumerate() {
        let ang = axis_angle(i, n, opts);
        s.push_str(&format!(
            "    {{ \"name\": \"{}\", \"min\": {}, \"max\": {}, \"angle_deg\": {} }}{}\n",
            esc_json(a),
            fmt_json_num(doms[i].min),
            fmt_json_num(doms[i].max),
            round2(ang * 180.0 / std::f64::consts::PI + 90.0),
            if i + 1 == n { "" } else { "," }
        ));
    }
    s.push_str("  ],\n  \"series\": [\n");
    for (si, ser) in chart.series.iter().enumerate() {
        s.push_str(&format!(
            "    {{ \"name\": \"{}\", \"color\": \"{}\", \"points\": [\n",
            esc_json(&ser.name),
            esc_json(&colors[si])
        ));
        for i in 0..n {
            let t = doms[i].t(ser.values[i]);
            let p = polar(cx, cy, radius * t, axis_angle(i, n, opts));
            s.push_str(&format!(
                "      {{ \"axis\": \"{}\", \"value\": {}, \"t\": {}, \"x\": {}, \"y\": {} }}{}\n",
                esc_json(&chart.axes[i]),
                fmt_json_num(ser.values[i]),
                round2(t),
                round2(p.x),
                round2(p.y),
                if i + 1 == n { "" } else { "," }
            ));
        }
        s.push_str(&format!(
            "    ] }}{}\n",
            if si + 1 == chart.series.len() {
                ""
            } else {
                ","
            }
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
    fn wide_table_renders_svg_with_axes_and_legend() {
        let data = "product,Camera,Battery,Speed,Price\nPhone A,8,7,9,6\nPhone B,6,9,7,8";
        let svg = render(data, &opts()).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("Camera"));
        assert!(svg.contains("Battery"));
        assert!(svg.contains("Phone A"));
        assert!(svg.contains("Phone B"));
        // Two series → the first two default-palette colours.
        assert!(svg.contains("#2563eb"));
        assert!(svg.contains("#f97316"));
        // A tick label from the auto domain 0..10.
        assert!(svg.contains(">10<"));
    }

    #[test]
    fn single_series_two_column_rows_parse() {
        let c = parse("Camera,8\nBattery,7\nSpeed,9", &opts()).unwrap();
        assert_eq!(c.layout, "single");
        assert_eq!(c.axes, vec!["Camera", "Battery", "Speed"]);
        assert_eq!(c.series.len(), 1);
        assert_eq!(c.series[0].values, vec![8.0, 7.0, 9.0]);
    }

    #[test]
    fn long_layout_fills_missing_cells() {
        let data = "series,axis,value\nA,Camera,8\nA,Battery,7\nA,Speed,9\nB,Camera,5";
        let c = parse(data, &opts()).unwrap();
        assert_eq!(c.layout, "long");
        assert_eq!(c.axes, vec!["Camera", "Battery", "Speed"]);
        assert_eq!(c.series[1].values, vec![5.0, 0.0, 0.0]);
        assert_eq!(c.series[1].present, vec![true, false, false]);
    }

    #[test]
    fn header_is_optional_for_wide_rows() {
        let c = parse("A,1,2,3\nB,3,2,1", &opts()).unwrap();
        assert_eq!(c.axes, vec!["Axis 1", "Axis 2", "Axis 3"]);
        assert_eq!(c.series.len(), 2);
    }

    #[test]
    fn geometry_puts_the_first_axis_at_the_top() {
        let data = "s,A,B,C\nOnly,10,10,10";
        let mut o = opts();
        o.output = "json".into();
        let json = render(data, &o).unwrap();
        assert!(json.contains("\"angle_deg\": 0"));
        // Full-radius vertex on axis A sits straight above the centre.
        assert!(json.contains("\"axis\": \"A\", \"value\": 10, \"t\": 1"));
    }

    #[test]
    fn per_axis_scale_normalizes_each_axis_independently() {
        let data = "s,Revenue,Rating,Uptime\nX,50000,4,99\nY,25000,2,95";
        let mut o = opts();
        o.scale = "per_axis".into();
        o.output = "summary".into();
        let out = render(data, &o).unwrap();
        assert!(out.contains("per-axis maximum"), "{out}");
        // Revenue tops out at 50,000 while Rating tops out at 4 — both full radius.
        assert!(
            out.lines().any(|l| l.contains("X")
                && l.contains("Revenue")
                && l.contains("50,000")
                && l.contains("100.0%")),
            "{out}"
        );
        assert!(
            out.lines().any(|l| l.contains("X")
                && l.contains("Rating")
                && l.contains("4")
                && l.contains("100.0%")),
            "{out}"
        );
        // Under a shared scale the rating would be invisible next to the revenue.
        o.scale = "shared".into();
        let shared = render(data, &o).unwrap();
        assert!(
            shared.lines().any(|l| l.contains("X")
                && l.contains("Rating")
                && l.contains("4")
                && l.contains("0.0%")),
            "{shared}"
        );
    }

    #[test]
    fn percent_scale_pins_the_domain_to_a_hundred() {
        let mut o = opts();
        o.scale = "percent".into();
        o.output = "summary".into();
        let out = render("Reach,50\nDepth,25\nSpeed,100", &o).unwrap();
        assert!(out.contains("domain: 0 to 100"));
        assert!(out.contains("50.0%"));
    }

    #[test]
    fn explicit_colors_override_the_palette() {
        let mut o = opts();
        o.colors = "#ff0000, #00ff00".into();
        let svg = render("s,A,B,C\nOne,1,2,3\nTwo,3,2,1", &o).unwrap();
        assert!(svg.contains("#ff0000"));
        assert!(svg.contains("#00ff00"));
    }

    #[test]
    fn circle_grid_and_value_labels_are_optional() {
        let mut o = opts();
        o.grid_shape = "circle".into();
        o.show_values = true;
        o.show_axis_labels = false;
        let svg = render("A,1\nB,2\nC,3", &o).unwrap();
        assert!(svg.contains("<circle cx="));
        // Value label text for the largest point, in the series colour.
        assert!(
            svg.contains("font-weight=\"600\" fill=\"#2563eb\">3</text>"),
            "{svg}"
        );
        assert!(
            !svg.contains("text-anchor=\"end\""),
            "axis captions are off"
        );
    }

    #[test]
    fn labels_are_xml_escaped() {
        let svg = render("R&D,5\nQ<A>,3\nOps,4", &opts()).unwrap();
        assert!(svg.contains("R&amp;D"));
        assert!(svg.contains("Q&lt;A&gt;"));
        assert!(!svg.contains("<A>"));
    }

    #[test]
    fn output_is_deterministic() {
        let data = "p,A,B,C\nOne,1,5,3\nTwo,4,2,6";
        assert_eq!(
            render(data, &opts()).unwrap(),
            render(data, &opts()).unwrap()
        );
    }

    #[test]
    fn err_too_few_axes() {
        let e = render("Camera,8\nBattery,7", &opts()).unwrap_err();
        assert!(e.contains("at least 3 axes"), "{e}");
        assert!(e.contains("Camera, Battery"), "{e}");
    }

    #[test]
    fn err_non_numeric_cell_names_the_axis_and_line() {
        let e = render("p,A,B,C\nOne,1,two,3", &opts()).unwrap_err();
        assert!(e.contains("line 2"), "{e}");
        assert!(e.contains("axis `B`"), "{e}");
        assert!(e.contains("got `two`"), "{e}");
    }

    #[test]
    fn err_ragged_row() {
        let e = render("p,A,B,C\nOne,1,2", &opts()).unwrap_err();
        assert!(e.contains("2 value(s) but there are 3 axes"), "{e}");
    }

    #[test]
    fn err_empty_data() {
        let e = render("   \n# just a comment\n", &opts()).unwrap_err();
        assert!(e.contains("data is empty"), "{e}");
    }

    #[test]
    fn err_bad_scale_window() {
        let mut o = opts();
        o.scale_min = 10.0;
        o.scale_max = 5.0;
        let e = render("A,1\nB,2\nC,3", &o).unwrap_err();
        assert!(e.contains("must be greater than scale_min"), "{e}");
    }

    #[test]
    fn err_canvas_too_small_for_the_labels() {
        let mut o = opts();
        o.width = 320;
        o.height = 240;
        o.font_size = 40.0;
        let e = render("Alpha,1\nBeta,2\nGamma,3", &o).unwrap_err();
        assert!(e.contains("chart area is too small"), "{e}");
    }

    #[test]
    fn err_unknown_enum_values() {
        let mut o = opts();
        o.output = "png".into();
        let e = render("A,1\nB,2\nC,3", &o).unwrap_err();
        assert!(e.contains("unknown output `png`"), "{e}");
    }

    #[test]
    fn caps_are_enforced() {
        let axes: Vec<String> = (0..MAX_AXES + 1).map(|i| format!("A{i}")).collect();
        let vals: Vec<String> = (0..MAX_AXES + 1).map(|_| "1".to_string()).collect();
        let data = format!("s,{}\nOne,{}", axes.join(","), vals.join(","));
        let e = render(&data, &opts()).unwrap_err();
        assert!(e.contains("too many axes"), "{e}");
    }

    #[test]
    fn thousands_separators_and_currency_marks_parse() {
        let c = parse("k,Revenue,Cost,Margin\nQ1,$1 200,900,25%", &opts()).unwrap();
        assert_eq!(c.series[0].values, vec![1200.0, 900.0, 25.0]);
    }
}
