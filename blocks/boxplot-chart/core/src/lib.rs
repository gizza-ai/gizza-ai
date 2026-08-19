//! Box-and-whisker plot rendering, pure and deterministic.
//!
//! Parses pasted data in one of three layouts (a flat list of values, a tidy
//! `group,value` table, or one column per group), computes a five-number
//! summary per group with a choice of quartile rule, derives whiskers/outliers,
//! and renders a standalone SVG — or a text summary table / JSON stats block.
//!
//! No plotting library, no I/O: the same code runs in the chat Service Worker,
//! the CLI, and the browser page.
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt::Write as _;

/// Hard caps so a paste-bomb can't hang the browser tab.
pub const MAX_GROUPS: usize = 60;
pub const MAX_VALUES: usize = 100_000;

/// Every knob the renderer accepts. Mirrors the block descriptor 1:1.
#[derive(Debug, Clone)]
pub struct Options {
    /// `auto` | `values` | `long` | `wide`
    pub layout: String,
    /// For `long`: the category column (header name or 1-based index).
    pub group_column: String,
    /// For `long`: the numeric column (header name or 1-based index).
    pub value_column: String,
    /// `linear` | `inclusive` | `exclusive`
    pub quartile_method: String,
    /// `tukey` | `minmax` | `percentile`
    pub whiskers: String,
    /// Tukey fence multiplier k (whiskers = `tukey`).
    pub iqr_multiplier: f64,
    /// Lower percentile for `whiskers = percentile` (upper is 100 - this).
    pub percentile: f64,
    /// `outliers` | `all` | `none`
    pub points: String,
    pub show_mean: bool,
    pub notched: bool,
    /// `vertical` | `horizontal`
    pub orientation: String,
    pub grid: bool,
    pub title: String,
    pub value_label: String,
    pub group_label: String,
    pub width: u32,
    pub height: u32,
    pub color: String,
    /// `light` | `dark`
    pub theme: String,
    /// `svg` | `summary` | `json`
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            layout: "auto".into(),
            group_column: String::new(),
            value_column: String::new(),
            quartile_method: "linear".into(),
            whiskers: "tukey".into(),
            iqr_multiplier: 1.5,
            percentile: 5.0,
            points: "outliers".into(),
            show_mean: true,
            notched: false,
            orientation: "vertical".into(),
            grid: true,
            title: String::new(),
            value_label: String::new(),
            group_label: String::new(),
            width: 800,
            height: 480,
            color: "#2563eb".into(),
            theme: "light".into(),
            output: "svg".into(),
        }
    }
}

/// The five-number summary (plus fences, whiskers and outliers) of one group.
#[derive(Debug, Clone)]
pub struct GroupStats {
    pub name: String,
    pub n: usize,
    pub min: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub max: f64,
    pub iqr: f64,
    pub mean: f64,
    pub std_dev: f64,
    pub lower_fence: f64,
    pub upper_fence: f64,
    pub whisker_low: f64,
    pub whisker_high: f64,
    pub outliers: Vec<f64>,
    pub notch_low: f64,
    pub notch_high: f64,
    pub values: Vec<f64>,
}

/// Entry point: parse `data`, compute the stats, render in `opts.output` form.
pub fn render(data: &str, opts: &Options) -> Result<String, String> {
    let stats = analyze(data, opts)?;
    match normalize(&opts.output, "svg") {
        "svg" => Ok(render_svg(&stats, opts)),
        "summary" => Ok(render_summary(&stats, opts)),
        "json" => Ok(render_json(&stats, opts)),
        other => Err(format!(
            "unknown output '{other}': expected one of svg, summary, json"
        )),
    }
}

/// Parse + summarize without rendering (used by every output format and tests).
pub fn analyze(data: &str, opts: &Options) -> Result<Vec<GroupStats>, String> {
    let method = normalize(&opts.quartile_method, "linear");
    if !matches!(method, "linear" | "inclusive" | "exclusive") {
        return Err(format!(
            "unknown quartile_method '{method}': expected one of linear, inclusive, exclusive"
        ));
    }
    let whiskers = normalize(&opts.whiskers, "tukey");
    if !matches!(whiskers, "tukey" | "minmax" | "percentile") {
        return Err(format!(
            "unknown whiskers '{whiskers}': expected one of tukey, minmax, percentile"
        ));
    }
    let points = normalize(&opts.points, "outliers");
    if !matches!(points, "outliers" | "all" | "none") {
        return Err(format!(
            "unknown points '{points}': expected one of outliers, all, none"
        ));
    }
    if !opts.iqr_multiplier.is_finite() || opts.iqr_multiplier < 0.0 || opts.iqr_multiplier > 10.0 {
        return Err(format!(
            "iqr_multiplier must be between 0 and 10, got {}",
            fmt_num(opts.iqr_multiplier)
        ));
    }
    if !opts.percentile.is_finite() || opts.percentile < 0.0 || opts.percentile >= 50.0 {
        return Err(format!(
            "percentile must be at least 0 and below 50 (it is the LOWER whisker percentile; the upper is 100 minus it), got {}",
            fmt_num(opts.percentile)
        ));
    }
    let groups = build_groups(data, opts)?;
    Ok(groups
        .into_iter()
        .map(|(name, values)| summarize(name, values, method, whiskers, opts))
        .collect())
}

// ---------------------------------------------------------------------------
// parsing
// ---------------------------------------------------------------------------

fn normalize<'a>(s: &'a str, fallback: &'a str) -> &'a str {
    let t = s.trim();
    if t.is_empty() {
        fallback
    } else {
        t
    }
}

fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim().trim_start_matches('+');
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Pick the column delimiter: an explicit tab/comma/semicolon beats whitespace.
fn detect_delim(lines: &[&str]) -> Option<char> {
    let mut tab = 0usize;
    let mut comma = 0usize;
    let mut semi = 0usize;
    for line in lines {
        tab += line.matches('\t').count();
        comma += line.matches(',').count();
        semi += line.matches(';').count();
    }
    if tab > 0 {
        Some('\t')
    } else if comma > 0 {
        Some(',')
    } else if semi > 0 {
        Some(';')
    } else {
        None
    }
}

/// Split one line into cells, honouring `"quoted, fields"` and `""` escapes.
fn split_line(line: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == delim {
            out.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(ch);
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn cells_of(line: &str, delim: Option<char>) -> Vec<String> {
    match delim {
        Some(d) => split_line(line, d),
        None => line.split_whitespace().map(|s| s.to_string()).collect(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Layout {
    Values,
    Long,
    Wide,
}

fn resolve_layout(spec: &str) -> Result<Option<Layout>, String> {
    Ok(match normalize(spec, "auto") {
        "auto" => None,
        "values" | "list" => Some(Layout::Values),
        "long" | "grouped" | "tidy" => Some(Layout::Long),
        "wide" | "columns" => Some(Layout::Wide),
        other => {
            return Err(format!(
                "unknown layout '{other}': expected one of auto, values, long, wide"
            ))
        }
    })
}

/// Ordered group accumulator: first-seen order, like the input.
struct Bins {
    order: Vec<String>,
    map: HashMap<String, Vec<f64>>,
    total: usize,
}

impl Bins {
    fn new() -> Self {
        Self {
            order: Vec::new(),
            map: HashMap::new(),
            total: 0,
        }
    }
    fn push(&mut self, name: &str, v: f64) -> Result<(), String> {
        if !self.map.contains_key(name) {
            if self.order.len() >= MAX_GROUPS {
                return Err(format!(
                    "too many groups: this tool plots at most {MAX_GROUPS} boxes, and the data has more"
                ));
            }
            self.order.push(name.to_string());
            self.map.insert(name.to_string(), Vec::new());
        }
        self.total += 1;
        if self.total > MAX_VALUES {
            return Err(format!(
                "too many values: this tool accepts at most {MAX_VALUES} numbers"
            ));
        }
        self.map.get_mut(name).expect("group just inserted").push(v);
        Ok(())
    }
    fn finish(self) -> Vec<(String, Vec<f64>)> {
        let mut map = self.map;
        self.order
            .into_iter()
            .filter_map(|k| {
                let v = map.remove(&k)?;
                if v.is_empty() {
                    None
                } else {
                    Some((k, v))
                }
            })
            .collect()
    }
}

fn build_groups(data: &str, opts: &Options) -> Result<Vec<(String, Vec<f64>)>, String> {
    let text = data.trim_start_matches('\u{feff}');
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return Err("no data: paste numbers (one per line, or comma/space separated), a `group,value` table, or one column per group".into());
    }
    let delim = detect_delim(&lines);
    let rows: Vec<Vec<String>> = lines.iter().map(|l| cells_of(l, delim)).collect();
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);

    let explicit = resolve_layout(&opts.layout)?;
    let want_long_cols =
        !opts.group_column.trim().is_empty() || !opts.value_column.trim().is_empty();
    let layout = match explicit {
        Some(l) => l,
        None => detect_layout(&rows, ncols, delim, want_long_cols),
    };

    let groups = match layout {
        Layout::Values => collect_values(&lines, delim)?,
        Layout::Long => collect_long(&rows, ncols, opts)?,
        Layout::Wide => collect_wide(&rows, ncols)?,
    };
    if groups.is_empty() {
        return Err(
            "no numeric values found: check that the value column actually holds numbers".into(),
        );
    }
    Ok(groups)
}

fn row_is_all_text(row: &[String]) -> bool {
    let filled: Vec<&String> = row.iter().filter(|c| !c.trim().is_empty()).collect();
    !filled.is_empty() && filled.iter().all(|c| parse_num(c).is_none())
}

fn detect_layout(
    rows: &[Vec<String>],
    ncols: usize,
    delim: Option<char>,
    want_long_cols: bool,
) -> Layout {
    if want_long_cols && ncols >= 2 {
        return Layout::Long;
    }
    // Whitespace-separated text is a free-form number list, never a table.
    if delim.is_none() || ncols <= 1 {
        return Layout::Values;
    }
    let header = row_is_all_text(&rows[0]);
    let data_rows = if header { &rows[1..] } else { rows };
    if data_rows.is_empty() {
        return Layout::Values;
    }
    // A rectangular table is a table; ragged lines of numbers ("3, 7, 9" then
    // "12") are a pasted list, and so is a single row ("3, 7, 9, 12").
    let all_numeric = data_rows.iter().all(|r| {
        r.iter()
            .filter(|c| !c.trim().is_empty())
            .all(|c| parse_num(c).is_some())
    });
    let ragged = rows.iter().any(|r| r.len() != rows[0].len());
    if all_numeric && (ragged || data_rows.len() == 1) {
        return Layout::Values;
    }
    // A leading non-numeric column across the data rows means tidy group/value.
    let first_col_text = data_rows
        .iter()
        .filter_map(|r| r.first())
        .filter(|c| !c.trim().is_empty())
        .all(|c| parse_num(c).is_none());
    if ncols == 2 && first_col_text {
        Layout::Long
    } else {
        Layout::Wide
    }
}

fn collect_values(lines: &[&str], delim: Option<char>) -> Result<Vec<(String, Vec<f64>)>, String> {
    let mut name = String::from("values");
    let mut vals: Vec<f64> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let cells: Vec<String> = cells_of(line, delim)
            .into_iter()
            .filter(|c| !c.trim().is_empty())
            .collect();
        if cells.is_empty() {
            continue;
        }
        if i == 0 && cells.iter().all(|c| parse_num(c).is_none()) {
            // A single header line names the series instead of erroring.
            name = cells[0].clone();
            continue;
        }
        for c in cells {
            match parse_num(&c) {
                Some(v) => {
                    if vals.len() >= MAX_VALUES {
                        return Err(format!(
                            "too many values: this tool accepts at most {MAX_VALUES} numbers"
                        ));
                    }
                    vals.push(v);
                }
                None => {
                    return Err(format!(
                        "line {}: expected a number, got '{}' — the values layout takes numbers separated by commas, spaces, tabs or newlines (one optional header line is allowed)",
                        i + 1,
                        c
                    ))
                }
            }
        }
    }
    if vals.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![(name, vals)])
}

fn resolve_column(
    spec: &str,
    headers: Option<&Vec<String>>,
    default_idx: usize,
    ncols: usize,
    what: &str,
) -> Result<usize, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        if default_idx >= ncols {
            return Err(format!(
                "{what} defaults to column {} but the data has only {ncols} column(s) — paste a `group,value` table or set the column explicitly",
                default_idx + 1
            ));
        }
        return Ok(default_idx);
    }
    if let Ok(idx) = spec.parse::<usize>() {
        if idx >= 1 && idx <= ncols {
            return Ok(idx - 1);
        }
        return Err(format!(
            "{what} '{spec}' is out of range: the data has {ncols} column(s), so use 1..{ncols}"
        ));
    }
    match headers {
        Some(h) => h
            .iter()
            .position(|c| c.trim().eq_ignore_ascii_case(spec))
            .ok_or_else(|| {
                format!(
                    "{what} '{spec}' not found: available columns are {}",
                    h.iter()
                        .map(|c| format!("'{}'", c.trim()))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }),
        None => Err(format!(
            "{what} '{spec}' is a name but the data has no header row — add headers or use a 1-based column number"
        )),
    }
}

fn collect_long(
    rows: &[Vec<String>],
    ncols: usize,
    opts: &Options,
) -> Result<Vec<(String, Vec<f64>)>, String> {
    if ncols < 2 {
        return Err(
            "the long layout needs two columns (group, value); this data has one — use layout=values for a plain number list".into(),
        );
    }
    // The value column decides whether row 0 is a header: in tidy data the group
    // column is text on every row, so "row 0 has text" proves nothing.
    let vspec = opts.value_column.trim();
    let tentative_v = match vspec.parse::<usize>() {
        Ok(i) if i >= 1 && i <= ncols => i - 1,
        _ => 1,
    };
    let value_is_name = !vspec.is_empty() && vspec.parse::<usize>().is_err();
    let group_is_name = {
        let g = opts.group_column.trim();
        !g.is_empty() && g.parse::<usize>().is_err()
    };
    let header_row = value_is_name
        || group_is_name
        || rows
            .first()
            .and_then(|r| r.get(tentative_v))
            .map(|c| !c.trim().is_empty() && parse_num(c).is_none())
            .unwrap_or(false);
    let headers = if header_row { rows.first() } else { None };
    let gidx = resolve_column(&opts.group_column, headers, 0, ncols, "group_column")?;
    let vidx = resolve_column(&opts.value_column, headers, 1, ncols, "value_column")?;

    let mut bins = Bins::new();
    let start = if header_row { 1 } else { 0 };
    for (i, row) in rows.iter().enumerate().skip(start) {
        let raw_group = row.get(gidx).map(|s| s.trim()).unwrap_or("");
        let raw_value = row.get(vidx).map(|s| s.trim()).unwrap_or("");
        if raw_value.is_empty() && raw_group.is_empty() {
            continue;
        }
        if raw_value.is_empty() {
            continue; // a group with no reading on this row
        }
        let v = parse_num(raw_value).ok_or_else(|| {
            format!(
                "line {}: column {} (value_column) expected a number, got '{}'",
                i + 1,
                vidx + 1,
                raw_value
            )
        })?;
        let name = if raw_group.is_empty() {
            "(no group)"
        } else {
            raw_group
        };
        bins.push(name, v)?;
    }
    Ok(bins.finish())
}

fn collect_wide(rows: &[Vec<String>], ncols: usize) -> Result<Vec<(String, Vec<f64>)>, String> {
    let header_row = rows.len() > 1 && row_is_all_text(&rows[0]);
    let headers = if header_row { Some(&rows[0]) } else { None };
    let start = if header_row { 1 } else { 0 };
    let mut bins = Bins::new();
    for c in 0..ncols {
        let name = headers
            .and_then(|h| h.get(c))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Column {}", c + 1));
        for (i, row) in rows.iter().enumerate().skip(start) {
            let cell = row.get(c).map(|s| s.trim()).unwrap_or("");
            if cell.is_empty() {
                continue;
            }
            let v = parse_num(cell).ok_or_else(|| {
                format!(
                    "line {}: column {} ('{}') expected a number, got '{}' — in the wide layout every cell under a header must be numeric or blank",
                    i + 1,
                    c + 1,
                    name,
                    cell
                )
            })?;
            bins.push(&name, v)?;
        }
    }
    Ok(bins.finish())
}

// ---------------------------------------------------------------------------
// statistics
// ---------------------------------------------------------------------------

fn median_of(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

/// Linear-interpolated quantile — R type 7 / numpy / Excel `PERCENTILE.INC`.
pub fn quantile_linear(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let pos = p.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        sorted[lo] + (pos - lo as f64) * (sorted[hi] - sorted[lo])
    }
}

/// `(q1, median, q3)` under the chosen rule. `sorted` must be ascending.
pub fn quartiles(sorted: &[f64], method: &str) -> (f64, f64, f64) {
    let med = median_of(sorted);
    let n = sorted.len();
    if n == 0 {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    if n == 1 {
        return (sorted[0], sorted[0], sorted[0]);
    }
    match method {
        "inclusive" | "exclusive" => {
            let (lower, upper) = if n % 2 == 0 {
                (&sorted[..n / 2], &sorted[n / 2..])
            } else if method == "inclusive" {
                (&sorted[..n / 2 + 1], &sorted[n / 2..])
            } else {
                (&sorted[..n / 2], &sorted[n / 2 + 1..])
            };
            let q1 = if lower.is_empty() {
                med
            } else {
                median_of(lower)
            };
            let q3 = if upper.is_empty() {
                med
            } else {
                median_of(upper)
            };
            (q1, med, q3)
        }
        // "linear" and anything already validated upstream
        _ => (
            quantile_linear(sorted, 0.25),
            med,
            quantile_linear(sorted, 0.75),
        ),
    }
}

fn summarize(
    name: String,
    mut values: Vec<f64>,
    method: &str,
    whiskers: &str,
    opts: &Options,
) -> GroupStats {
    values.sort_by(|a, b| a.partial_cmp(b).expect("finite values only"));
    let n = values.len();
    let (q1, median, q3) = quartiles(&values, method);
    let iqr = q3 - q1;
    let min = values[0];
    let max = values[n - 1];
    let mean = values.iter().sum::<f64>() / n as f64;
    let std_dev = if n > 1 {
        (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt()
    } else {
        0.0
    };
    let lower_fence = q1 - opts.iqr_multiplier * iqr;
    let upper_fence = q3 + opts.iqr_multiplier * iqr;

    let (whisker_low, whisker_high) = match whiskers {
        "minmax" => (min, max),
        "percentile" => {
            let p = opts.percentile / 100.0;
            (
                quantile_linear(&values, p),
                quantile_linear(&values, 1.0 - p),
            )
        }
        // tukey: pull the whiskers in to the most extreme point inside the fences
        _ => {
            let lo = values
                .iter()
                .copied()
                .find(|v| *v >= lower_fence)
                .unwrap_or(min);
            let hi = values
                .iter()
                .rev()
                .copied()
                .find(|v| *v <= upper_fence)
                .unwrap_or(max);
            (lo, hi)
        }
    };
    let outliers: Vec<f64> = if whiskers == "minmax" {
        Vec::new()
    } else {
        values
            .iter()
            .copied()
            .filter(|v| *v < whisker_low || *v > whisker_high)
            .collect()
    };
    let half_notch = 1.58 * iqr / (n as f64).sqrt();
    GroupStats {
        name,
        n,
        min,
        q1,
        median,
        q3,
        max,
        iqr,
        mean,
        std_dev,
        lower_fence,
        upper_fence,
        whisker_low,
        whisker_high,
        outliers,
        notch_low: (median - half_notch).max(q1),
        notch_high: (median + half_notch).min(q3),
        values,
    }
}

// ---------------------------------------------------------------------------
// formatting helpers
// ---------------------------------------------------------------------------

/// Compact human number: integers stay bare, decimals lose trailing zeros.
pub fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "n/a".into();
    }
    if v == 0.0 {
        return "0".into();
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let mag = v.abs();
    let decimals = if mag >= 100.0 {
        2
    } else if mag >= 1.0 {
        4
    } else {
        6
    };
    let s = format!("{v:.decimals$}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn fmt_tick(v: f64, step: f64) -> String {
    let decimals = if step >= 1.0 {
        0
    } else {
        (-step.log10().floor()) as usize
    }
    .min(6);
    let s = format!("{v:.decimals$}");
    if s.contains('.') {
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        if s == "-0" {
            "0".into()
        } else {
            s
        }
    } else if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_num(v: f64) -> String {
    if v.is_finite() {
        let s = format!("{v}");
        s
    } else {
        "null".into()
    }
}

fn nice_step(range: f64, target: usize) -> f64 {
    if !(range > 0.0) || target == 0 {
        return 1.0;
    }
    let raw = range / target as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let mult = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    mult * mag
}

// ---------------------------------------------------------------------------
// text + JSON output
// ---------------------------------------------------------------------------

fn render_summary(stats: &[GroupStats], opts: &Options) -> String {
    let headers = [
        "Group", "N", "Min", "Q1", "Median", "Q3", "Max", "IQR", "Mean", "SD", "Outliers",
    ];
    let rows: Vec<Vec<String>> = stats
        .iter()
        .map(|s| {
            let outliers = if s.outliers.is_empty() {
                "—".to_string()
            } else {
                let shown: Vec<String> = s.outliers.iter().take(6).map(|v| fmt_num(*v)).collect();
                if s.outliers.len() > 6 {
                    format!("{} … (+{})", shown.join(", "), s.outliers.len() - 6)
                } else {
                    shown.join(", ")
                }
            };
            vec![
                s.name.clone(),
                s.n.to_string(),
                fmt_num(s.min),
                fmt_num(s.q1),
                fmt_num(s.median),
                fmt_num(s.q3),
                fmt_num(s.max),
                fmt_num(s.iqr),
                fmt_num(s.mean),
                fmt_num(s.std_dev),
                outliers,
            ]
        })
        .collect();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let pad = |cell: &str, w: usize| {
        let len = cell.chars().count();
        format!("{cell}{}", " ".repeat(w.saturating_sub(len)))
    };
    let mut out = String::new();
    let head: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| pad(h, widths[i]))
        .collect();
    let _ = writeln!(out, "{}", head.join("  ").trim_end());
    let _ = writeln!(
        out,
        "{}",
        widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("  ")
    );
    for row in &rows {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, c)| pad(c, widths[i]))
            .collect();
        let _ = writeln!(out, "{}", line.join("  ").trim_end());
    }
    let whiskers = normalize(&opts.whiskers, "tukey");
    let rule = match whiskers {
        "minmax" => "whiskers: min to max (no outliers marked)".to_string(),
        "percentile" => format!(
            "whiskers: {}th to {}th percentile",
            fmt_num(opts.percentile),
            fmt_num(100.0 - opts.percentile)
        ),
        _ => format!(
            "whiskers: Tukey, fences at Q1 - {k} x IQR and Q3 + {k} x IQR",
            k = fmt_num(opts.iqr_multiplier)
        ),
    };
    let _ = write!(
        out,
        "\nquartile method: {}\n{}",
        normalize(&opts.quartile_method, "linear"),
        rule
    );
    out
}

fn render_json(stats: &[GroupStats], opts: &Options) -> String {
    let mut out = String::from("{\n");
    let _ = writeln!(
        out,
        "  \"quartile_method\": {},",
        json_str(normalize(&opts.quartile_method, "linear"))
    );
    let _ = writeln!(
        out,
        "  \"whiskers\": {},",
        json_str(normalize(&opts.whiskers, "tukey"))
    );
    let _ = writeln!(
        out,
        "  \"iqr_multiplier\": {},",
        json_num(opts.iqr_multiplier)
    );
    out.push_str("  \"groups\": [\n");
    for (i, s) in stats.iter().enumerate() {
        let outliers: Vec<String> = s.outliers.iter().map(|v| json_num(*v)).collect();
        let _ = write!(
            out,
            "    {{\n      \"name\": {}, \"n\": {},\n      \"min\": {}, \"q1\": {}, \"median\": {}, \"q3\": {}, \"max\": {},\n      \"iqr\": {}, \"mean\": {}, \"std_dev\": {},\n      \"lower_fence\": {}, \"upper_fence\": {},\n      \"whisker_low\": {}, \"whisker_high\": {},\n      \"outliers\": [{}]\n    }}",
            json_str(&s.name),
            s.n,
            json_num(s.min),
            json_num(s.q1),
            json_num(s.median),
            json_num(s.q3),
            json_num(s.max),
            json_num(s.iqr),
            json_num(s.mean),
            json_num(s.std_dev),
            json_num(s.lower_fence),
            json_num(s.upper_fence),
            json_num(s.whisker_low),
            json_num(s.whisker_high),
            outliers.join(", ")
        );
        out.push_str(if i + 1 == stats.len() { "\n" } else { ",\n" });
    }
    out.push_str("  ]\n}");
    out
}

// ---------------------------------------------------------------------------
// SVG rendering
// ---------------------------------------------------------------------------

struct Theme {
    bg: &'static str,
    text: &'static str,
    muted: &'static str,
    axis: &'static str,
    grid: &'static str,
}

fn theme_of(name: &str) -> Theme {
    match name {
        "dark" => Theme {
            bg: "#0f172a",
            text: "#e2e8f0",
            muted: "#94a3b8",
            axis: "#475569",
            grid: "#1e293b",
        },
        _ => Theme {
            bg: "#ffffff",
            text: "#0f172a",
            muted: "#475569",
            axis: "#94a3b8",
            grid: "#e2e8f0",
        },
    }
}

const FONT: &str = "system-ui, -apple-system, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif";

fn render_svg(stats: &[GroupStats], opts: &Options) -> String {
    let vertical = normalize(&opts.orientation, "vertical") != "horizontal";
    let points = normalize(&opts.points, "outliers");
    let theme = theme_of(normalize(&opts.theme, "light"));
    let color = {
        let c = opts.color.trim();
        if c.is_empty() {
            "#2563eb"
        } else {
            c
        }
    };
    let w = opts.width.clamp(240, 4000) as f64;
    let h = opts.height.clamp(200, 4000) as f64;

    // --- value domain -----------------------------------------------------
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for s in stats {
        let mut consider = vec![s.whisker_low, s.whisker_high, s.q1, s.q3, s.median];
        if opts.show_mean {
            consider.push(s.mean);
        }
        if points == "all" {
            consider.push(s.min);
            consider.push(s.max);
        } else if points == "outliers" {
            consider.extend(s.outliers.iter().copied());
        }
        for v in consider {
            if v.is_finite() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
    }
    if !lo.is_finite() || !hi.is_finite() {
        lo = 0.0;
        hi = 1.0;
    }
    if (hi - lo).abs() < f64::EPSILON {
        let pad = if lo.abs() > 0.0 { lo.abs() * 0.1 } else { 1.0 };
        lo -= pad;
        hi += pad;
    }
    let span = hi - lo;
    let (dmin, dmax) = (lo - span * 0.08, hi + span * 0.08);
    let step = nice_step(dmax - dmin, 6);
    let tick_start = (dmin / step).ceil() * step;
    let mut ticks: Vec<f64> = Vec::new();
    let mut t = tick_start;
    while t <= dmax + step * 1e-9 && ticks.len() < 40 {
        ticks.push(t);
        t += step;
    }

    // --- margins ----------------------------------------------------------
    let tick_chars = ticks
        .iter()
        .map(|v| fmt_tick(*v, step).chars().count())
        .max()
        .unwrap_or(3);
    let name_chars = stats
        .iter()
        .map(|s| s.name.chars().count())
        .max()
        .unwrap_or(4);
    let has_title = !opts.title.trim().is_empty();
    let has_value_label = !opts.value_label.trim().is_empty();
    let has_group_label = !opts.group_label.trim().is_empty();
    let (m_top, m_right, m_bottom, m_left) = if vertical {
        (
            if has_title { 52.0 } else { 24.0 },
            24.0,
            42.0 + if has_group_label { 24.0 } else { 0.0 },
            (14.0 + tick_chars as f64 * 7.5).max(44.0) + if has_value_label { 22.0 } else { 0.0 },
        )
    } else {
        (
            if has_title { 52.0 } else { 24.0 },
            28.0,
            40.0 + if has_value_label { 24.0 } else { 0.0 },
            (14.0 + (name_chars.min(18) as f64) * 7.0).max(56.0)
                + if has_group_label { 22.0 } else { 0.0 },
        )
    };
    let plot_left = m_left;
    let plot_right = (w - m_right).max(m_left + 40.0);
    let plot_top = m_top;
    let plot_bottom = (h - m_bottom).max(m_top + 40.0);

    // Value axis runs bottom→top when vertical, left→right when horizontal.
    let (p0, p1) = if vertical {
        (plot_bottom, plot_top)
    } else {
        (plot_left, plot_right)
    };
    let vpos = |v: f64| -> f64 {
        if (dmax - dmin).abs() < f64::EPSILON {
            (p0 + p1) / 2.0
        } else {
            p0 + (v - dmin) / (dmax - dmin) * (p1 - p0)
        }
    };
    // Category axis: bands across the other dimension.
    let (c0, c1) = if vertical {
        (plot_left, plot_right)
    } else {
        (plot_top, plot_bottom)
    };
    let n = stats.len().max(1) as f64;
    let band = (c1 - c0) / n;
    let center = |i: usize| c0 + band * (i as f64 + 0.5);
    let hw = (band * 0.32).min(46.0).max(4.0);

    // (value, category-offset) → (x, y)
    let xy = |v: f64, c: f64| -> (f64, f64) {
        if vertical {
            (c, vpos(v))
        } else {
            (vpos(v), c)
        }
    };

    let mut svg = String::with_capacity(4096);
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img" font-family="{FONT}">"#,
        w = fmt_px(w),
        h = fmt_px(h),
        FONT = FONT
    );
    let desc = format!(
        "Box and whisker plot of {} group{}",
        stats.len(),
        if stats.len() == 1 { "" } else { "s" }
    );
    let _ = write!(
        svg,
        "<title>{}</title><desc>{}</desc>",
        esc(if has_title {
            opts.title.trim()
        } else {
            desc.as_str()
        }),
        esc(&desc)
    );
    let _ = write!(
        svg,
        r#"<rect width="100%" height="100%" fill="{}"/>"#,
        esc(theme.bg)
    );

    if has_title {
        let _ = write!(
            svg,
            r#"<text x="{}" y="30" text-anchor="middle" font-size="18" font-weight="600" fill="{}">{}</text>"#,
            fmt_px((plot_left + plot_right) / 2.0),
            esc(theme.text),
            esc(opts.title.trim())
        );
    }

    // --- grid + value ticks ----------------------------------------------
    for v in &ticks {
        let (x1, y1) = xy(*v, if vertical { plot_left } else { plot_top });
        let (x2, y2) = xy(*v, if vertical { plot_right } else { plot_bottom });
        if opts.grid {
            let _ = write!(
                svg,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
                fmt_px(x1),
                fmt_px(y1),
                fmt_px(x2),
                fmt_px(y2),
                esc(theme.grid)
            );
        }
        let label = fmt_tick(*v, step);
        if vertical {
            let _ = write!(
                svg,
                r#"<text x="{}" y="{}" text-anchor="end" font-size="12" fill="{}">{}</text>"#,
                fmt_px(plot_left - 8.0),
                fmt_px(y1 + 4.0),
                esc(theme.muted),
                esc(&label)
            );
        } else {
            let _ = write!(
                svg,
                r#"<text x="{}" y="{}" text-anchor="middle" font-size="12" fill="{}">{}</text>"#,
                fmt_px(x1),
                fmt_px(plot_bottom + 18.0),
                esc(theme.muted),
                esc(&label)
            );
        }
    }

    // --- axes -------------------------------------------------------------
    let _ = write!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
        fmt_px(plot_left),
        fmt_px(plot_bottom),
        fmt_px(plot_right),
        fmt_px(plot_bottom),
        esc(theme.axis)
    );
    let _ = write!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
        fmt_px(plot_left),
        fmt_px(plot_top),
        fmt_px(plot_left),
        fmt_px(plot_bottom),
        esc(theme.axis)
    );

    // --- one box per group ------------------------------------------------
    for (i, s) in stats.iter().enumerate() {
        let c = center(i);
        // whisker spine + caps
        for (a, b) in [(s.whisker_low, s.q1), (s.q3, s.whisker_high)] {
            let (x1, y1) = xy(a, c);
            let (x2, y2) = xy(b, c);
            let _ = write!(
                svg,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1.5"/>"#,
                fmt_px(x1),
                fmt_px(y1),
                fmt_px(x2),
                fmt_px(y2),
                esc(theme.text)
            );
        }
        for v in [s.whisker_low, s.whisker_high] {
            let (x1, y1) = xy(v, c - hw * 0.5);
            let (x2, y2) = xy(v, c + hw * 0.5);
            let _ = write!(
                svg,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1.5"/>"#,
                fmt_px(x1),
                fmt_px(y1),
                fmt_px(x2),
                fmt_px(y2),
                esc(theme.text)
            );
        }
        // the box itself
        if opts.notched {
            let pts: Vec<(f64, f64)> = vec![
                (s.q3, -hw),
                (s.q3, hw),
                (s.notch_high, hw),
                (s.median, hw * 0.5),
                (s.notch_low, hw),
                (s.q1, hw),
                (s.q1, -hw),
                (s.notch_low, -hw),
                (s.median, -hw * 0.5),
                (s.notch_high, -hw),
            ];
            let path: Vec<String> = pts
                .iter()
                .map(|(v, off)| {
                    let (x, y) = xy(*v, c + off);
                    format!("{},{}", fmt_px(x), fmt_px(y))
                })
                .collect();
            let _ = write!(
                svg,
                r#"<polygon points="{}" fill="{}" fill-opacity="0.72" stroke="{}" stroke-width="1.5"/>"#,
                path.join(" "),
                esc(color),
                esc(theme.text)
            );
        } else {
            let (x, y, bw, bh) = rect_between(s.q1, s.q3, c, hw, vertical, &vpos);
            let _ = write!(
                svg,
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" fill-opacity="0.72" stroke="{}" stroke-width="1.5" rx="2"/>"#,
                fmt_px(x),
                fmt_px(y),
                fmt_px(bw),
                fmt_px(bh),
                esc(color),
                esc(theme.text)
            );
        }
        // median
        let (mx1, my1) = xy(s.median, c - hw);
        let (mx2, my2) = xy(s.median, c + hw);
        let _ = write!(
            svg,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2.5"/>"#,
            fmt_px(mx1),
            fmt_px(my1),
            fmt_px(mx2),
            fmt_px(my2),
            esc(theme.text)
        );
        // mean marker (diamond)
        if opts.show_mean {
            let (cx, cy) = xy(s.mean, c);
            let r = 4.5;
            let _ = write!(
                svg,
                r#"<polygon points="{},{} {},{} {},{} {},{}" fill="{}" stroke="{}" stroke-width="1"/>"#,
                fmt_px(cx),
                fmt_px(cy - r),
                fmt_px(cx + r),
                fmt_px(cy),
                fmt_px(cx),
                fmt_px(cy + r),
                fmt_px(cx - r),
                fmt_px(cy),
                esc(theme.bg),
                esc(theme.text)
            );
        }
        // points
        match points {
            "all" => {
                for (k, v) in s.values.iter().enumerate() {
                    // deterministic spread so identical values don't stack
                    let off = ((k % 5) as f64 - 2.0) * (hw * 0.22);
                    let (cx, cy) = xy(*v, c + off);
                    let outlier = *v < s.whisker_low || *v > s.whisker_high;
                    let _ = write!(
                        svg,
                        r#"<circle cx="{}" cy="{}" r="{}" fill="{}" fill-opacity="{}" stroke="{}" stroke-width="0.75"/>"#,
                        fmt_px(cx),
                        fmt_px(cy),
                        if outlier { "3.4" } else { "2.4" },
                        esc(if outlier { color } else { theme.muted }),
                        if outlier { "0.95" } else { "0.55" },
                        esc(theme.text)
                    );
                }
            }
            "outliers" => {
                for v in &s.outliers {
                    let (cx, cy) = xy(*v, c);
                    let _ = write!(
                        svg,
                        r#"<circle cx="{}" cy="{}" r="3.4" fill="none" stroke="{}" stroke-width="1.5"/>"#,
                        fmt_px(cx),
                        fmt_px(cy),
                        esc(color)
                    );
                }
            }
            _ => {}
        }
        // category label
        let name = esc(&s.name);
        if vertical {
            let _ = write!(
                svg,
                r#"<text x="{}" y="{}" text-anchor="middle" font-size="12" fill="{}">{}</text>"#,
                fmt_px(c),
                fmt_px(plot_bottom + 20.0),
                esc(theme.text),
                name
            );
        } else {
            let _ = write!(
                svg,
                r#"<text x="{}" y="{}" text-anchor="end" font-size="12" fill="{}">{}</text>"#,
                fmt_px(plot_left - 8.0),
                fmt_px(c + 4.0),
                esc(theme.text),
                name
            );
        }
    }

    // --- axis labels ------------------------------------------------------
    let value_axis_label = opts.value_label.trim();
    let group_axis_label = opts.group_label.trim();
    if !value_axis_label.is_empty() {
        if vertical {
            let x = 16.0;
            let y = (plot_top + plot_bottom) / 2.0;
            let _ = write!(
                svg,
                r#"<text x="{x}" y="{y}" text-anchor="middle" font-size="13" fill="{fill}" transform="rotate(-90 {x} {y})">{label}</text>"#,
                x = fmt_px(x),
                y = fmt_px(y),
                fill = esc(theme.muted),
                label = esc(value_axis_label)
            );
        } else {
            let _ = write!(
                svg,
                r#"<text x="{}" y="{}" text-anchor="middle" font-size="13" fill="{}">{}</text>"#,
                fmt_px((plot_left + plot_right) / 2.0),
                fmt_px(h - 10.0),
                esc(theme.muted),
                esc(value_axis_label)
            );
        }
    }
    if !group_axis_label.is_empty() {
        if vertical {
            let _ = write!(
                svg,
                r#"<text x="{}" y="{}" text-anchor="middle" font-size="13" fill="{}">{}</text>"#,
                fmt_px((plot_left + plot_right) / 2.0),
                fmt_px(h - 10.0),
                esc(theme.muted),
                esc(group_axis_label)
            );
        } else {
            let x = 16.0;
            let y = (plot_top + plot_bottom) / 2.0;
            let _ = write!(
                svg,
                r#"<text x="{x}" y="{y}" text-anchor="middle" font-size="13" fill="{fill}" transform="rotate(-90 {x} {y})">{label}</text>"#,
                x = fmt_px(x),
                y = fmt_px(y),
                fill = esc(theme.muted),
                label = esc(group_axis_label)
            );
        }
    }

    svg.push_str("</svg>");
    svg
}

/// Axis-aligned rect spanning `v_lo..v_hi` on the value axis, `±hw` across.
fn rect_between(
    v_lo: f64,
    v_hi: f64,
    c: f64,
    hw: f64,
    vertical: bool,
    vpos: &dyn Fn(f64) -> f64,
) -> (f64, f64, f64, f64) {
    let a = vpos(v_lo);
    let b = vpos(v_hi);
    let (near, far) = if a <= b { (a, b) } else { (b, a) };
    if vertical {
        (c - hw, near, hw * 2.0, (far - near).max(1.0))
    } else {
        (near, c - hw, (far - near).max(1.0), hw * 2.0)
    }
}

/// SVG coordinates: at most 2 decimals, no trailing zeros — keeps output small
/// and byte-stable across platforms.
fn fmt_px(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let r = (v * 100.0).round() / 100.0;
    if r == 0.0 {
        return "0".into();
    }
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn renders_grouped_svg_with_boxes_and_labels() {
        let data = "group,value\nA,1\nA,2\nA,3\nA,4\nB,5\nB,6\nB,7\nB,20";
        let mut o = opts();
        o.title = "Latency by group".into();
        let svg = render(data, &o).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("Latency by group"));
        assert!(svg.contains(">A<"));
        assert!(svg.contains(">B<"));
        assert!(svg.contains("<rect"));
        assert!(svg.contains("#2563eb"));
        // 20 is beyond Q3 + 1.5*IQR for group B → drawn as an outlier circle
        assert!(svg.contains("<circle"));
    }

    #[test]
    fn quartile_methods_differ_on_even_n() {
        let sorted = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        assert_eq!(quartiles(&sorted, "linear"), (2.75, 4.5, 6.25));
        assert_eq!(quartiles(&sorted, "inclusive"), (2.5, 4.5, 6.5));
        assert_eq!(quartiles(&sorted, "exclusive"), (2.5, 4.5, 6.5));
    }

    #[test]
    fn quartile_methods_differ_on_odd_n() {
        let sorted = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        assert_eq!(quartiles(&sorted, "linear"), (3.0, 5.0, 7.0));
        assert_eq!(quartiles(&sorted, "inclusive"), (3.0, 5.0, 7.0));
        assert_eq!(quartiles(&sorted, "exclusive"), (2.5, 5.0, 7.5));
    }

    #[test]
    fn tukey_whiskers_and_outliers() {
        let stats = analyze("1\n2\n3\n4\n5\n100", &opts()).unwrap();
        let s = &stats[0];
        assert_eq!(s.n, 6);
        assert_eq!(s.name, "values");
        assert_eq!(s.q1, 2.25);
        assert_eq!(s.q3, 4.75);
        assert_eq!(s.median, 3.5);
        assert_eq!(s.whisker_low, 1.0);
        assert_eq!(s.whisker_high, 5.0);
        assert_eq!(s.outliers, vec![100.0]);
    }

    #[test]
    fn minmax_whiskers_swallow_outliers() {
        let mut o = opts();
        o.whiskers = "minmax".into();
        let s = &analyze("1,2,3,4,5,100", &o).unwrap()[0];
        assert_eq!(s.whisker_low, 1.0);
        assert_eq!(s.whisker_high, 100.0);
        assert!(s.outliers.is_empty());
    }

    #[test]
    fn percentile_whiskers_use_the_requested_percentile() {
        let mut o = opts();
        o.whiskers = "percentile".into();
        o.percentile = 10.0;
        let data: String = (1..=11).map(|i| format!("{i}\n")).collect();
        let s = &analyze(&data, &o).unwrap()[0];
        assert_eq!(s.whisker_low, 2.0);
        assert_eq!(s.whisker_high, 10.0);
        assert_eq!(s.outliers, vec![1.0, 11.0]);
    }

    #[test]
    fn iqr_multiplier_widens_the_fences() {
        let mut o = opts();
        o.iqr_multiplier = 3.0;
        let s = &analyze("1\n2\n3\n4\n5\n12", &o).unwrap()[0];
        assert!(s.outliers.is_empty(), "k=3 keeps 12 inside the fence");
    }

    #[test]
    fn wide_layout_makes_one_box_per_column() {
        let data = "north,south\n10,20\n12,24\n14,26\n,28";
        let stats = analyze(data, &opts()).unwrap();
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].name, "north");
        assert_eq!(stats[0].n, 3);
        assert_eq!(stats[1].name, "south");
        assert_eq!(stats[1].n, 4);
    }

    #[test]
    fn long_layout_columns_by_name() {
        let data = "region,team,ms\nEU,a,10\nUS,b,20\nEU,c,30";
        let mut o = opts();
        o.group_column = "region".into();
        o.value_column = "ms".into();
        let stats = analyze(data, &o).unwrap();
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].name, "EU");
        assert_eq!(stats[0].values, vec![10.0, 30.0]);
        assert_eq!(stats[1].name, "US");
    }

    #[test]
    fn values_layout_accepts_mixed_separators_and_a_header() {
        let stats = analyze("score\n3, 7, 9\n12\n15", &opts()).unwrap();
        assert_eq!(stats[0].name, "score");
        assert_eq!(stats[0].n, 5);
        assert_eq!(stats[0].min, 3.0);
        assert_eq!(stats[0].max, 15.0);
    }

    #[test]
    fn single_row_of_numbers_is_a_list_not_many_groups() {
        let stats = analyze("3, 7, 9, 12", &opts()).unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].n, 4);
    }

    #[test]
    fn horizontal_orientation_swaps_the_axes() {
        let mut o = opts();
        o.orientation = "horizontal".into();
        let svg = render("a,1\na,2\nb,3\nb,9", &o).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains(">a<"));
    }

    #[test]
    fn summary_output_lists_the_five_number_summary() {
        let mut o = opts();
        o.output = "summary".into();
        let out = render("1\n2\n3\n4\n5\n100", &o).unwrap();
        assert!(out.contains("Group"));
        assert!(out.contains("Median"));
        assert!(out.contains("100"));
        assert!(out.contains("quartile method: linear"));
        assert!(out.contains("Tukey"));
    }

    #[test]
    fn json_output_is_parseable_shaped_stats() {
        let mut o = opts();
        o.output = "json".into();
        let out = render("group,value\nA,1\nA,2\nA,3\nA,10", &o).unwrap();
        assert!(out.contains("\"name\": \"A\""));
        assert!(out.contains("\"median\": 2.5"));
        assert!(out.contains("\"outliers\": [10]"));
        assert!(out.trim_end().ends_with('}'));
    }

    #[test]
    fn theme_and_size_are_honoured() {
        let mut o = opts();
        o.theme = "dark".into();
        o.width = 640;
        o.height = 360;
        o.color = "tomato".into();
        let svg = render("1\n2\n3\n4\n5", &o).unwrap();
        assert!(svg.contains(r#"width="640""#));
        assert!(svg.contains(r#"height="360""#));
        assert!(svg.contains("#0f172a"));
        assert!(svg.contains("tomato"));
    }

    #[test]
    fn notched_boxes_draw_a_polygon() {
        let mut o = opts();
        o.notched = true;
        let svg = render("1\n2\n3\n4\n5\n6\n7\n8", &o).unwrap();
        assert!(svg.contains("<polygon"));
    }

    #[test]
    fn points_all_draws_every_observation() {
        let mut o = opts();
        o.points = "all".into();
        let svg = render("1\n2\n3\n4\n5", &o).unwrap();
        assert_eq!(svg.matches("<circle").count(), 5);
    }

    #[test]
    fn points_none_hides_outlier_dots() {
        let mut o = opts();
        o.points = "none".into();
        let svg = render("1\n2\n3\n4\n5\n100", &o).unwrap();
        assert!(!svg.contains("<circle"));
    }

    #[test]
    fn xml_special_characters_are_escaped() {
        let svg = render("group,value\n<A & B>,1\n<A & B>,2", &opts()).unwrap();
        assert!(svg.contains("&lt;A &amp; B&gt;"));
        assert!(!svg.contains("<A & B>"));
    }

    #[test]
    fn empty_input_is_an_actionable_error() {
        let err = render("   \n\n", &opts()).unwrap_err();
        assert!(err.contains("no data"), "{err}");
    }

    #[test]
    fn non_numeric_value_names_the_line() {
        let err = render("group,value\nA,1\nA,oops", &opts()).unwrap_err();
        assert!(err.contains("line 3"), "{err}");
        assert!(err.contains("oops"), "{err}");
    }

    #[test]
    fn unknown_column_lists_the_available_ones() {
        let mut o = opts();
        o.value_column = "nope".into();
        let err = render("region,ms\nEU,1\nUS,2", &o).unwrap_err();
        assert!(err.contains("not found"), "{err}");
        assert!(err.contains("'ms'"), "{err}");
    }

    #[test]
    fn unknown_enum_values_are_rejected() {
        let mut o = opts();
        o.quartile_method = "banana".into();
        assert!(render("1\n2\n3", &o)
            .unwrap_err()
            .contains("unknown quartile_method"));
        let mut o = opts();
        o.whiskers = "beard".into();
        assert!(render("1\n2\n3", &o).unwrap_err().contains("unknown whiskers"));
        let mut o = opts();
        o.output = "png".into();
        assert!(render("1\n2\n3", &o).unwrap_err().contains("unknown output"));
    }

    #[test]
    fn out_of_range_knobs_explain_the_range() {
        let mut o = opts();
        o.iqr_multiplier = 42.0;
        assert!(render("1\n2\n3", &o)
            .unwrap_err()
            .contains("between 0 and 10"));
        let mut o = opts();
        o.percentile = 60.0;
        assert!(render("1\n2\n3", &o).unwrap_err().contains("below 50"));
    }

    #[test]
    fn too_many_groups_is_capped() {
        let mut data = String::from("group,value\n");
        for i in 0..(MAX_GROUPS + 5) {
            let _ = writeln!(data, "g{i},{i}");
        }
        let err = render(&data, &opts()).unwrap_err();
        assert!(err.contains("too many groups"), "{err}");
    }

    #[test]
    fn rendering_is_deterministic() {
        let data = "group,value\nA,1\nA,5\nB,2\nB,9";
        assert_eq!(render(data, &opts()).unwrap(), render(data, &opts()).unwrap());
    }
}
