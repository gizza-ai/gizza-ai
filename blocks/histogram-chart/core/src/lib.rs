//! Histogram binning and rendering, pure and deterministic.
//!
//! Parses a pasted list of numbers (any common separator, optional header row),
//! chooses bin edges from an automatic rule or an explicit bin count/width,
//! tallies each bin under the requested y-axis normalisation, and renders a
//! standalone SVG — or a frequency table / CSV / JSON stats block.
//!
//! No plotting library, no I/O: the same code runs in the chat Service Worker,
//! the CLI, and the browser page.
#![forbid(unsafe_code)]

use std::fmt::Write as _;

/// Hard caps so a paste-bomb can't hang the browser tab.
pub const MAX_VALUES: usize = 100_000;
pub const MAX_BINS: usize = 500;
/// A histogram of one point has no width; two is the smallest meaningful input.
pub const MIN_VALUES: usize = 2;

/// Every knob the renderer accepts. Mirrors the block descriptor 1:1.
#[derive(Debug, Clone)]
pub struct Options {
    pub bin_method: String,
    pub bins: u32,
    pub bin_width: f64,
    pub range_min: String,
    pub range_max: String,
    pub normalize: String,
    pub right_closed: bool,
    pub show_values: bool,
    pub show_mean: bool,
    pub show_median: bool,
    pub normal_curve: bool,
    pub rug: bool,
    pub grid: bool,
    pub orientation: String,
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub width: u32,
    pub height: u32,
    pub color: String,
    pub opacity: f64,
    pub theme: String,
    pub precision: u32,
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            bin_method: "auto".into(),
            bins: 10,
            bin_width: 0.0,
            range_min: String::new(),
            range_max: String::new(),
            normalize: "count".into(),
            right_closed: false,
            show_values: false,
            show_mean: false,
            show_median: false,
            normal_curve: false,
            rug: false,
            grid: true,
            orientation: "vertical".into(),
            title: String::new(),
            x_label: String::new(),
            y_label: String::new(),
            width: 800,
            height: 480,
            color: "#2563eb".into(),
            opacity: 0.9,
            theme: "light".into(),
            precision: 4,
            output: "svg".into(),
        }
    }
}

/// One bin: its half-open interval, raw count, and normalised plot value.
#[derive(Debug, Clone)]
pub struct Bin {
    pub lower: f64,
    pub upper: f64,
    pub count: usize,
    /// Value actually plotted, per `Options::normalize`.
    pub value: f64,
}

/// Everything the renderers need, computed once.
#[derive(Debug, Clone)]
pub struct Histogram {
    pub bins: Vec<Bin>,
    pub bin_width: f64,
    /// Rule that actually produced the bin count (never `auto`).
    pub rule: String,
    /// Values kept after range clipping — the denominator for normalisation.
    pub n: usize,
    /// Values excluded by an explicit `range_min`/`range_max`.
    pub excluded: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub median: f64,
    pub sd: f64,
    pub q1: f64,
    pub q3: f64,
    /// Ascending kept values — needed for the rug plot.
    pub sorted: Vec<f64>,
}

const FONT: &str = "system-ui, -apple-system, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif";

// ---------------------------------------------------------------------------
// entry points
// ---------------------------------------------------------------------------

/// Parse, bin, and render in the requested `output` format.
pub fn render(data: &str, opts: &Options) -> Result<String, String> {
    let hist = analyze(data, opts)?;
    match normalize_opt(&opts.output, "svg") {
        "svg" => Ok(render_svg(&hist, opts)),
        "table" => Ok(render_table(&hist, opts)),
        "csv" => Ok(render_csv(&hist, opts)),
        "json" => Ok(render_json(&hist, opts)),
        other => Err(format!(
            "unknown output '{other}': expected one of svg, table, csv, json"
        )),
    }
}

/// Parse + bin without rendering. Useful for tests and other callers.
pub fn analyze(data: &str, opts: &Options) -> Result<Histogram, String> {
    let values = parse_values(data)?;
    let (lo, hi, excluded, kept) = resolve_range(&values, opts)?;
    let n = kept.len();
    if n == 0 {
        return Err("no values fall inside the requested range".into());
    }

    let mut sorted = kept.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite values"));

    let mean = sorted.iter().sum::<f64>() / n as f64;
    let sd = if n > 1 {
        (sorted.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt()
    } else {
        0.0
    };
    let median = median_of(&sorted);
    let q1 = quantile_linear(&sorted, 0.25);
    let q3 = quantile_linear(&sorted, 0.75);

    let (k, rule) = bin_count(&sorted, lo, hi, mean, sd, q1, q3, opts)?;
    let width = (hi - lo) / k as f64;

    let mut counts = vec![0usize; k];
    let right_closed = opts.right_closed;
    for &v in &sorted {
        let mut idx = ((v - lo) / width).floor() as isize;
        if right_closed {
            // (a, b]: a value exactly on an interior edge belongs to the bin below.
            let on_edge = (v - lo) / width;
            if (on_edge - on_edge.round()).abs() < 1e-12 {
                idx = on_edge.round() as isize - 1;
            }
        }
        let idx = idx.clamp(0, k as isize - 1) as usize;
        counts[idx] += 1;
    }

    let mut bins = Vec::with_capacity(k);
    let mut running = 0usize;
    for (i, &c) in counts.iter().enumerate() {
        running += c;
        let value = match normalize_opt(&opts.normalize, "count") {
            "count" => c as f64,
            "relative" => c as f64 / n as f64,
            "percent" => 100.0 * c as f64 / n as f64,
            "density" => c as f64 / (n as f64 * width),
            "cumulative_count" => running as f64,
            "cumulative_percent" => 100.0 * running as f64 / n as f64,
            other => {
                return Err(format!(
                    "unknown normalize '{other}': expected one of count, relative, percent, \
                     density, cumulative_count, cumulative_percent"
                ))
            }
        };
        bins.push(Bin {
            lower: lo + i as f64 * width,
            upper: lo + (i + 1) as f64 * width,
            count: c,
            value,
        });
    }

    Ok(Histogram {
        bins,
        bin_width: width,
        rule,
        n,
        excluded,
        min: sorted[0],
        max: sorted[n - 1],
        mean,
        median,
        sd,
        q1,
        q3,
        sorted,
    })
}

// ---------------------------------------------------------------------------
// parsing
// ---------------------------------------------------------------------------

fn normalize_opt<'a>(s: &'a str, fallback: &'a str) -> &'a str {
    let t = s.trim();
    if t.is_empty() {
        fallback
    } else {
        t
    }
}

fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim().trim_matches('"').trim();
    let t = t.strip_prefix('+').unwrap_or(t);
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Split on any of newline / comma / tab / semicolon / whitespace.
fn tokens_of(line: &str) -> Vec<&str> {
    line.split(|c: char| c == ',' || c == '\t' || c == ';' || c.is_whitespace())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect()
}

/// Every numeric token in the paste. A leading all-text row is treated as a
/// header; any other non-numeric token is a line-numbered error.
fn parse_values(data: &str) -> Result<Vec<f64>, String> {
    let lines: Vec<&str> = data.lines().collect();
    let mut out: Vec<f64> = Vec::new();
    let mut seen_data = false;

    for (i, line) in lines.iter().enumerate() {
        let toks = tokens_of(line);
        if toks.is_empty() {
            continue;
        }
        let all_text = toks.iter().all(|t| parse_num(t).is_none());
        if all_text && !seen_data {
            // header row — skip it
            continue;
        }
        for t in toks {
            match parse_num(t) {
                Some(v) => {
                    if out.len() >= MAX_VALUES {
                        return Err(format!(
                            "too many values: this tool accepts at most {MAX_VALUES} numbers"
                        ));
                    }
                    out.push(v);
                    seen_data = true;
                }
                None => {
                    return Err(format!(
                        "line {}: '{}' is not a number (use plain decimals like 12.5 or \
                         scientific notation like 1.2e3)",
                        i + 1,
                        t
                    ))
                }
            }
        }
    }

    if out.len() < MIN_VALUES {
        return Err(format!(
            "need at least {MIN_VALUES} numbers to build a histogram, found {}",
            out.len()
        ));
    }
    Ok(out)
}

/// `(lo, hi, excluded, kept)`. An explicit bound clips values outside it.
fn resolve_range(values: &[f64], opts: &Options) -> Result<(f64, f64, usize, Vec<f64>), String> {
    let user_min = parse_bound("range_min", &opts.range_min)?;
    let user_max = parse_bound("range_max", &opts.range_max)?;

    let data_min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let data_max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let mut lo = user_min.unwrap_or(data_min);
    let mut hi = user_max.unwrap_or(data_max);
    if lo > hi {
        return Err(format!(
            "range_min ({}) must be less than range_max ({})",
            fmt_num(lo),
            fmt_num(hi)
        ));
    }

    let kept: Vec<f64> = if user_min.is_some() || user_max.is_some() {
        values.iter().cloned().filter(|v| *v >= lo && *v <= hi).collect()
    } else {
        values.to_vec()
    };
    let excluded = values.len() - kept.len();

    if lo == hi {
        // All values identical (or a zero-width explicit range): widen so the
        // chart has an axis instead of dividing by zero.
        let pad = if lo == 0.0 { 0.5 } else { lo.abs() * 0.05 };
        lo -= pad;
        hi += pad;
    }
    Ok((lo, hi, excluded, kept))
}

fn parse_bound(name: &str, raw: &str) -> Result<Option<f64>, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(None);
    }
    parse_num(t)
        .map(Some)
        .ok_or_else(|| format!("{name} must be a finite number, got '{t}'"))
}

// ---------------------------------------------------------------------------
// binning rules
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn bin_count(
    sorted: &[f64],
    lo: f64,
    hi: f64,
    mean: f64,
    sd: f64,
    q1: f64,
    q3: f64,
    opts: &Options,
) -> Result<(usize, String), String> {
    let n = sorted.len() as f64;
    let span = hi - lo;
    let cube = n.powf(-1.0 / 3.0);

    let from_width = |h: f64| -> usize {
        if h > 0.0 && h.is_finite() {
            (span / h).ceil() as usize
        } else {
            0
        }
    };
    let sturges = || ((n.log2()).ceil() as usize) + 1;
    let fd = || from_width(2.0 * (q3 - q1) * cube);
    let scott = || from_width(3.49 * sd * cube);

    let (raw, rule) = match normalize_opt(&opts.bin_method, "auto") {
        // numpy's "auto": the finer of Freedman-Diaconis and Sturges.
        "auto" => {
            let f = fd();
            let s = sturges();
            if f == 0 {
                (s, "sturges".to_string())
            } else if f >= s {
                (f, "freedman_diaconis".to_string())
            } else {
                (s, "sturges".to_string())
            }
        }
        "sturges" => (sturges(), "sturges".to_string()),
        "scott" => (scott(), "scott".to_string()),
        "freedman_diaconis" => (fd(), "freedman_diaconis".to_string()),
        "rice" => ((2.0 * n.powf(1.0 / 3.0)).ceil() as usize, "rice".to_string()),
        "doane" => (doane(sorted, mean, sd), "doane".to_string()),
        "sqrt" => (n.sqrt().ceil() as usize, "sqrt".to_string()),
        "count" => (opts.bins as usize, "count".to_string()),
        "width" => {
            if !(opts.bin_width > 0.0) || !opts.bin_width.is_finite() {
                return Err(
                    "bin_method=width needs a positive bin_width (for example 5)".to_string()
                );
            }
            let k = (span / opts.bin_width).ceil() as usize;
            if k > MAX_BINS {
                return Err(format!(
                    "bin_width {} splits the range into {k} bins, over the {MAX_BINS}-bin cap — \
                     use a larger bin_width",
                    fmt_num(opts.bin_width)
                ));
            }
            (k.max(1), "width".to_string())
        }
        other => {
            return Err(format!(
                "unknown bin_method '{other}': expected one of auto, sturges, scott, \
                 freedman_diaconis, rice, doane, sqrt, count, width"
            ))
        }
    };

    Ok((raw.clamp(1, MAX_BINS), rule))
}

/// Doane's rule: Sturges corrected for skewness.
fn doane(sorted: &[f64], mean: f64, sd: f64) -> usize {
    let n = sorted.len() as f64;
    if n < 3.0 || sd <= 0.0 {
        return ((n.log2()).ceil() as usize) + 1;
    }
    let g1 = sorted.iter().map(|v| ((v - mean) / sd).powi(3)).sum::<f64>() / n;
    let sigma_g1 = (6.0 * (n - 2.0) / ((n + 1.0) * (n + 3.0))).sqrt();
    let k = 1.0 + n.log2() + (1.0 + g1.abs() / sigma_g1).log2();
    k.ceil().max(1.0) as usize
}

// ---------------------------------------------------------------------------
// stats helpers
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

// ---------------------------------------------------------------------------
// formatting helpers
// ---------------------------------------------------------------------------

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

/// Round to `p` decimals, then drop trailing zeros.
fn fmt_p(v: f64, p: u32) -> String {
    if !v.is_finite() {
        return "n/a".into();
    }
    let s = format!("{v:.prec$}", prec = p.min(12) as usize);
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
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
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
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
        format!("{v}")
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

fn fmt_px(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    let s = format!("{r}");
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// `[1, 2)` or `(1, 2]`, honouring the closure convention.
fn interval_label(b: &Bin, is_last: bool, is_first: bool, right_closed: bool, p: u32) -> String {
    let lo = fmt_p(b.lower, p);
    let hi = fmt_p(b.upper, p);
    if right_closed {
        if is_first {
            format!("[{lo}, {hi}]")
        } else {
            format!("({lo}, {hi}]")
        }
    } else if is_last {
        format!("[{lo}, {hi}]")
    } else {
        format!("[{lo}, {hi})")
    }
}

fn y_axis_title(normalize: &str) -> &'static str {
    match normalize {
        "relative" => "Relative frequency",
        "percent" => "Percent",
        "density" => "Density",
        "cumulative_count" => "Cumulative count",
        "cumulative_percent" => "Cumulative percent",
        _ => "Count",
    }
}

// ---------------------------------------------------------------------------
// text + CSV + JSON output
// ---------------------------------------------------------------------------

fn render_table(h: &Histogram, opts: &Options) -> String {
    let p = opts.precision;
    let right_closed = opts.right_closed;
    let norm = normalize_opt(&opts.normalize, "count");

    let last = h.bins.len() - 1;
    let rows: Vec<(String, String, String, String, String)> = h
        .bins
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let mut running = 0usize;
            for bb in &h.bins[..=i] {
                running += bb.count;
            }
            (
                interval_label(b, i == last, i == 0, right_closed, p),
                b.count.to_string(),
                fmt_p(100.0 * b.count as f64 / h.n as f64, 2),
                fmt_p(100.0 * running as f64 / h.n as f64, 2),
                fmt_p(b.value, p),
            )
        })
        .collect();

    let headers = ["Bin", "Count", "Percent", "Cumulative %", y_axis_title(norm)];
    let mut w = headers.iter().map(|s| s.len()).collect::<Vec<_>>();
    for r in &rows {
        let cells = [&r.0, &r.1, &r.2, &r.3, &r.4];
        for (i, c) in cells.iter().enumerate() {
            w[i] = w[i].max(c.len());
        }
    }

    let mut out = String::new();
    if !opts.title.trim().is_empty() {
        let _ = writeln!(out, "{}\n", opts.title.trim());
    }
    for (i, hd) in headers.iter().enumerate() {
        let _ = write!(out, "{:<width$}", hd, width = w[i]);
        if i + 1 < headers.len() {
            out.push_str("  ");
        }
    }
    out.push('\n');
    for (i, _) in headers.iter().enumerate() {
        let _ = write!(out, "{}", "-".repeat(w[i]));
        if i + 1 < headers.len() {
            out.push_str("  ");
        }
    }
    out.push('\n');
    for r in &rows {
        let cells = [&r.0, &r.1, &r.2, &r.3, &r.4];
        for (i, c) in cells.iter().enumerate() {
            let _ = write!(out, "{:<width$}", c, width = w[i]);
            if i + 1 < cells.len() {
                out.push_str("  ");
            }
        }
        out.push('\n');
    }

    let _ = write!(
        out,
        "\nbins: {} ({} rule), width {}\n\
         n: {}  min: {}  max: {}  mean: {}  median: {}  sd: {}  q1: {}  q3: {}",
        h.bins.len(),
        h.rule,
        fmt_p(h.bin_width, p),
        h.n,
        fmt_p(h.min, p),
        fmt_p(h.max, p),
        fmt_p(h.mean, p),
        fmt_p(h.median, p),
        fmt_p(h.sd, p),
        fmt_p(h.q1, p),
        fmt_p(h.q3, p),
    );
    if h.excluded > 0 {
        let _ = write!(out, "\nexcluded by range: {}", h.excluded);
    }
    out
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(h: &Histogram, opts: &Options) -> String {
    let p = opts.precision;
    let last = h.bins.len() - 1;
    let mut out = String::from("bin,lower,upper,count,percent,cumulative_count,cumulative_percent,value\n");
    let mut running = 0usize;
    for (i, b) in h.bins.iter().enumerate() {
        running += b.count;
        let _ = writeln!(
            out,
            "{},{},{},{},{},{},{},{}",
            csv_cell(&interval_label(b, i == last, i == 0, opts.right_closed, p)),
            fmt_p(b.lower, p),
            fmt_p(b.upper, p),
            b.count,
            fmt_p(100.0 * b.count as f64 / h.n as f64, 2),
            running,
            fmt_p(100.0 * running as f64 / h.n as f64, 2),
            fmt_p(b.value, p),
        );
    }
    out.trim_end().to_string()
}

fn render_json(h: &Histogram, opts: &Options) -> String {
    let p = opts.precision;
    let last = h.bins.len() - 1;
    let mut out = String::from("{\n");
    let _ = writeln!(out, "  \"rule\": {},", json_str(&h.rule));
    let _ = writeln!(out, "  \"normalize\": {},", json_str(normalize_opt(&opts.normalize, "count")));
    let _ = writeln!(out, "  \"bin_count\": {},", h.bins.len());
    let _ = writeln!(out, "  \"bin_width\": {},", json_num(h.bin_width));
    let _ = writeln!(out, "  \"n\": {},", h.n);
    let _ = writeln!(out, "  \"excluded\": {},", h.excluded);
    let _ = writeln!(out, "  \"stats\": {{");
    let _ = writeln!(out, "    \"min\": {},", json_num(h.min));
    let _ = writeln!(out, "    \"max\": {},", json_num(h.max));
    let _ = writeln!(out, "    \"mean\": {},", json_num(h.mean));
    let _ = writeln!(out, "    \"median\": {},", json_num(h.median));
    let _ = writeln!(out, "    \"sd\": {},", json_num(h.sd));
    let _ = writeln!(out, "    \"q1\": {},", json_num(h.q1));
    let _ = writeln!(out, "    \"q3\": {}", json_num(h.q3));
    let _ = writeln!(out, "  }},");
    let _ = writeln!(out, "  \"bins\": [");
    let mut running = 0usize;
    for (i, b) in h.bins.iter().enumerate() {
        running += b.count;
        let comma = if i == last { "" } else { "," };
        let _ = writeln!(
            out,
            "    {{ \"label\": {}, \"lower\": {}, \"upper\": {}, \"count\": {}, \
             \"cumulative_count\": {}, \"value\": {} }}{}",
            json_str(&interval_label(b, i == last, i == 0, opts.right_closed, p)),
            json_num(b.lower),
            json_num(b.upper),
            b.count,
            running,
            json_num(b.value),
            comma
        );
    }
    out.push_str("  ]\n}");
    out
}

// ---------------------------------------------------------------------------
// SVG
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

fn render_svg(h: &Histogram, opts: &Options) -> String {
    let theme = theme_of(normalize_opt(&opts.theme, "light"));
    let vertical = normalize_opt(&opts.orientation, "vertical") != "horizontal";
    let color = {
        let c = opts.color.trim();
        if c.is_empty() {
            "#2563eb"
        } else {
            c
        }
    };
    let opacity = if opts.opacity.is_finite() {
        opts.opacity.clamp(0.05, 1.0)
    } else {
        0.9
    };
    let p = opts.precision;
    let norm = normalize_opt(&opts.normalize, "count");

    let w = opts.width.clamp(320, 2400) as f64;
    let hgt = opts.height.clamp(240, 1800) as f64;

    let title = opts.title.trim();
    let x_label = if opts.x_label.trim().is_empty() {
        "Value".to_string()
    } else {
        opts.x_label.trim().to_string()
    };
    let y_label = if opts.y_label.trim().is_empty() {
        y_axis_title(norm).to_string()
    } else {
        opts.y_label.trim().to_string()
    };

    // Plot box.
    let pad_top = if title.is_empty() { 24.0 } else { 52.0 };
    let pad_left = if vertical { 74.0 } else { 118.0 };
    let pad_right = 24.0;
    let pad_bottom = 62.0;
    let plot_w = (w - pad_left - pad_right).max(40.0);
    let plot_h = (hgt - pad_top - pad_bottom).max(40.0);
    let x0 = pad_left;
    let y0 = pad_top;
    let x1 = x0 + plot_w;
    let y1 = y0 + plot_h;

    let lo = h.bins[0].lower;
    let hi = h.bins[h.bins.len() - 1].upper;
    let span = (hi - lo).max(f64::MIN_POSITIVE);

    let vmax_raw = h.bins.iter().map(|b| b.value).fold(0.0_f64, f64::max);
    let step = nice_step(vmax_raw.max(f64::MIN_POSITIVE), 5);
    let vmax = if vmax_raw <= 0.0 {
        step
    } else {
        (vmax_raw / step).ceil() * step
    };

    // value -> pixel along the measurement axis
    let vpx = |v: f64| -> f64 {
        let f = (v / vmax).clamp(0.0, 1.0);
        if vertical {
            y1 - f * plot_h
        } else {
            x0 + f * plot_w
        }
    };
    // data value -> pixel along the category axis
    let dpx = |d: f64| -> f64 {
        let f = ((d - lo) / span).clamp(0.0, 1.0);
        if vertical {
            x0 + f * plot_w
        } else {
            y1 - f * plot_h
        }
    };

    let mut s = String::with_capacity(4096);
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img" font-family="{FONT}">"#,
        w = fmt_px(w),
        h = fmt_px(hgt),
    );
    let _ = write!(
        s,
        r#"<title>{}</title>"#,
        esc(if title.is_empty() { "Histogram" } else { title })
    );
    let _ = write!(
        s,
        r#"<rect width="{}" height="{}" fill="{}"/>"#,
        fmt_px(w),
        fmt_px(hgt),
        theme.bg
    );

    if !title.is_empty() {
        let _ = write!(
            s,
            r#"<text x="{}" y="30" text-anchor="middle" font-size="18" font-weight="600" fill="{}">{}</text>"#,
            fmt_px(w / 2.0),
            theme.text,
            esc(title)
        );
    }

    // gridlines + measurement-axis ticks
    let mut t = 0.0_f64;
    while t <= vmax + step * 0.5 {
        let q = vpx(t);
        if opts.grid {
            if vertical {
                let _ = write!(
                    s,
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
                    fmt_px(x0),
                    fmt_px(q),
                    fmt_px(x1),
                    fmt_px(q),
                    theme.grid
                );
            } else {
                let _ = write!(
                    s,
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
                    fmt_px(q),
                    fmt_px(y0),
                    fmt_px(q),
                    fmt_px(y1),
                    theme.grid
                );
            }
        }
        if vertical {
            let _ = write!(
                s,
                r#"<text x="{}" y="{}" text-anchor="end" font-size="11" fill="{}">{}</text>"#,
                fmt_px(x0 - 8.0),
                fmt_px(q + 4.0),
                theme.muted,
                esc(&fmt_tick(t, step))
            );
        } else {
            let _ = write!(
                s,
                r#"<text x="{}" y="{}" text-anchor="middle" font-size="11" fill="{}">{}</text>"#,
                fmt_px(q),
                fmt_px(y1 + 16.0),
                theme.muted,
                esc(&fmt_tick(t, step))
            );
        }
        t += step;
    }

    // bars
    let last = h.bins.len() - 1;
    let gap = if h.bins.len() > 60 { 0.0 } else { 1.0 };
    for (i, b) in h.bins.iter().enumerate() {
        let a = dpx(b.lower);
        let c = dpx(b.upper);
        let base = vpx(0.0);
        let top = vpx(b.value);
        let (rx, ry, rw, rh) = if vertical {
            let left = a.min(c) + gap / 2.0;
            let width = (c - a).abs() - gap;
            (left, top.min(base), width.max(0.5), (base - top).abs())
        } else {
            let bot = a.min(c) + gap / 2.0;
            let height = (c - a).abs() - gap;
            (base.min(top), bot, (top - base).abs(), height.max(0.5))
        };
        if rw > 0.0 && rh > 0.0 {
            let _ = write!(
                s,
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" fill-opacity="{}"><title>{}: {}</title></rect>"#,
                fmt_px(rx),
                fmt_px(ry),
                fmt_px(rw),
                fmt_px(rh),
                esc(color),
                fmt_px(opacity),
                esc(&interval_label(b, i == last, i == 0, opts.right_closed, p)),
                esc(&fmt_p(b.value, p))
            );
        }
        if opts.show_values && b.count > 0 {
            let label = fmt_p(b.value, p);
            if vertical {
                let _ = write!(
                    s,
                    r#"<text x="{}" y="{}" text-anchor="middle" font-size="10" fill="{}">{}</text>"#,
                    fmt_px(rx + rw / 2.0),
                    fmt_px(ry - 4.0),
                    theme.text,
                    esc(&label)
                );
            } else {
                let _ = write!(
                    s,
                    r#"<text x="{}" y="{}" text-anchor="start" font-size="10" fill="{}">{}</text>"#,
                    fmt_px(rx + rw + 4.0),
                    fmt_px(ry + rh / 2.0 + 3.0),
                    theme.text,
                    esc(&label)
                );
            }
        }
    }

    // normal curve overlay, scaled to the plotted normalisation
    if opts.normal_curve && h.sd > 0.0 {
        let scale = match norm {
            "relative" => h.bin_width,
            "percent" => 100.0 * h.bin_width,
            "density" => 1.0,
            "cumulative_count" | "cumulative_percent" => 0.0, // curve is meaningless cumulatively
            _ => h.n as f64 * h.bin_width,
        };
        if scale > 0.0 {
            let steps = 120usize;
            let mut pts = String::new();
            for i in 0..=steps {
                let d = lo + span * (i as f64 / steps as f64);
                let z = (d - h.mean) / h.sd;
                let pdf = (-0.5 * z * z).exp() / (h.sd * (2.0 * std::f64::consts::PI).sqrt());
                let v = pdf * scale;
                let (px, py) = if vertical {
                    (dpx(d), vpx(v))
                } else {
                    (vpx(v), dpx(d))
                };
                if i > 0 {
                    pts.push(' ');
                }
                let _ = write!(pts, "{},{}", fmt_px(px), fmt_px(py));
            }
            let _ = write!(
                s,
                r#"<polyline points="{pts}" fill="none" stroke="{}" stroke-width="2" stroke-dasharray="6 3"/>"#,
                theme.text
            );
        }
    }

    // rug plot: one tick per observation on the category axis
    if opts.rug {
        for &v in &h.sorted {
            let q = dpx(v);
            if vertical {
                let _ = write!(
                    s,
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-opacity="0.5"/>"#,
                    fmt_px(q),
                    fmt_px(y1),
                    fmt_px(q),
                    fmt_px(y1 - 7.0),
                    esc(color)
                );
            } else {
                let _ = write!(
                    s,
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-opacity="0.5"/>"#,
                    fmt_px(x0),
                    fmt_px(q),
                    fmt_px(x0 + 7.0),
                    fmt_px(q),
                    esc(color)
                );
            }
        }
    }

    // mean / median markers
    let marker = |v: f64, label: &str, dash: &str, s: &mut String| {
        let q = dpx(v);
        if vertical {
            let _ = write!(
                s,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2" stroke-dasharray="{dash}"/>"#,
                fmt_px(q),
                fmt_px(y0),
                fmt_px(q),
                fmt_px(y1),
                theme.text
            );
            let _ = write!(
                s,
                r#"<text x="{}" y="{}" text-anchor="start" font-size="11" fill="{}">{} {}</text>"#,
                fmt_px(q + 4.0),
                fmt_px(y0 + 12.0),
                theme.text,
                esc(label),
                esc(&fmt_p(v, p))
            );
        } else {
            let _ = write!(
                s,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2" stroke-dasharray="{dash}"/>"#,
                fmt_px(x0),
                fmt_px(q),
                fmt_px(x1),
                fmt_px(q),
                theme.text
            );
            let _ = write!(
                s,
                r#"<text x="{}" y="{}" text-anchor="end" font-size="11" fill="{}">{} {}</text>"#,
                fmt_px(x1 - 4.0),
                fmt_px(q - 4.0),
                theme.text,
                esc(label),
                esc(&fmt_p(v, p))
            );
        }
    };
    if opts.show_mean {
        marker(h.mean, "mean", "4 3", &mut s);
    }
    if opts.show_median {
        marker(h.median, "median", "1 3", &mut s);
    }

    // category-axis tick labels: bin edges, thinned so they never collide
    let edge_count = h.bins.len() + 1;
    let room = if vertical { plot_w / 56.0 } else { plot_h / 22.0 };
    let stride = ((edge_count as f64 / room.max(1.0)).ceil() as usize).max(1);
    for i in (0..edge_count).step_by(stride) {
        let d = lo + i as f64 * h.bin_width;
        let q = dpx(d);
        if vertical {
            let _ = write!(
                s,
                r#"<text x="{}" y="{}" text-anchor="middle" font-size="11" fill="{}">{}</text>"#,
                fmt_px(q),
                fmt_px(y1 + 18.0),
                theme.muted,
                esc(&fmt_p(d, p.min(3)))
            );
        } else {
            let _ = write!(
                s,
                r#"<text x="{}" y="{}" text-anchor="end" font-size="11" fill="{}">{}</text>"#,
                fmt_px(x0 - 8.0),
                fmt_px(q + 4.0),
                theme.muted,
                esc(&fmt_p(d, p.min(3)))
            );
        }
    }

    // axes
    let _ = write!(
        s,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
        fmt_px(x0),
        fmt_px(y1),
        fmt_px(x1),
        fmt_px(y1),
        theme.axis
    );
    let _ = write!(
        s,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
        fmt_px(x0),
        fmt_px(y0),
        fmt_px(x0),
        fmt_px(y1),
        theme.axis
    );

    // axis titles (x = data axis, y = measurement axis, regardless of orientation)
    let (data_title, meas_title) = (x_label.as_str(), y_label.as_str());
    if vertical {
        let _ = write!(
            s,
            r#"<text x="{}" y="{}" text-anchor="middle" font-size="12" fill="{}">{}</text>"#,
            fmt_px((x0 + x1) / 2.0),
            fmt_px(hgt - 14.0),
            theme.text,
            esc(data_title)
        );
        let _ = write!(
            s,
            r#"<text x="16" y="{}" text-anchor="middle" font-size="12" fill="{}" transform="rotate(-90 16 {})">{}</text>"#,
            fmt_px((y0 + y1) / 2.0),
            theme.text,
            fmt_px((y0 + y1) / 2.0),
            esc(meas_title)
        );
    } else {
        let _ = write!(
            s,
            r#"<text x="{}" y="{}" text-anchor="middle" font-size="12" fill="{}">{}</text>"#,
            fmt_px((x0 + x1) / 2.0),
            fmt_px(hgt - 14.0),
            theme.text,
            esc(meas_title)
        );
        let _ = write!(
            s,
            r#"<text x="16" y="{}" text-anchor="middle" font-size="12" fill="{}" transform="rotate(-90 16 {})">{}</text>"#,
            fmt_px((y0 + y1) / 2.0),
            theme.text,
            fmt_px((y0 + y1) / 2.0),
            esc(data_title)
        );
    }

    s.push_str("</svg>");
    s
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn explicit_bin_count_bins_a_simple_list() {
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 4;
        o.output = "json".into();
        let h = analyze("1\n2\n3\n4\n5\n6\n7\n8", &o).unwrap();
        assert_eq!(h.bins.len(), 4);
        assert_eq!(h.n, 8);
        // range 1..8 over 4 bins => width 1.75, left-closed, last bin inclusive
        assert_eq!(
            h.bins.iter().map(|b| b.count).collect::<Vec<_>>(),
            vec![2, 2, 2, 2]
        );
        assert!((h.bin_width - 1.75).abs() < 1e-12);
    }

    #[test]
    fn counts_sum_to_n_for_every_rule() {
        let data: String = (1..=64).map(|i| format!("{i}\n")).collect();
        for rule in [
            "auto",
            "sturges",
            "scott",
            "freedman_diaconis",
            "rice",
            "doane",
            "sqrt",
        ] {
            let mut o = opts();
            o.bin_method = rule.into();
            let h = analyze(&data, &o).unwrap();
            assert_eq!(
                h.bins.iter().map(|b| b.count).sum::<usize>(),
                64,
                "rule {rule} lost values"
            );
            assert!(h.bins.len() >= 1 && h.bins.len() <= MAX_BINS);
            assert_ne!(h.rule, "auto", "auto must resolve to a concrete rule");
        }
    }

    #[test]
    fn sqrt_rule_uses_ceil_sqrt_n() {
        let data: String = (1..=100).map(|i| format!("{i} ")).collect();
        let mut o = opts();
        o.bin_method = "sqrt".into();
        let h = analyze(&data, &o).unwrap();
        assert_eq!(h.bins.len(), 10);
        assert_eq!(h.rule, "sqrt");
    }

    #[test]
    fn bin_width_mode_sets_the_edge_spacing() {
        let mut o = opts();
        o.bin_method = "width".into();
        o.bin_width = 5.0;
        let h = analyze("0\n1\n4\n6\n9\n10", &o).unwrap();
        assert_eq!(h.bins.len(), 2);
        assert!((h.bin_width - 5.0).abs() < 1e-12);
        assert_eq!(h.bins[0].count, 3); // 0,1,4
        assert_eq!(h.bins[1].count, 3); // 6,9,10 (last bin closes on the max)
    }

    #[test]
    fn right_closed_moves_edge_values_down_a_bin() {
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 2;
        let left = analyze("0\n5\n10", &o).unwrap();
        o.right_closed = true;
        let right = analyze("0\n5\n10", &o).unwrap();
        // left-closed [0,5) / [5,10]: the edge value 5 falls in the UPPER bin
        assert_eq!(left.bins.iter().map(|b| b.count).collect::<Vec<_>>(), vec![1, 2]);
        // right-closed [0,5] / (5,10]: the edge value 5 falls in the LOWER bin
        assert_eq!(right.bins.iter().map(|b| b.count).collect::<Vec<_>>(), vec![2, 1]);
    }

    #[test]
    fn normalisations_are_consistent() {
        let data = "1\n2\n3\n4";
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 2;

        o.normalize = "count".into();
        let c = analyze(data, &o).unwrap();
        assert_eq!(c.bins.iter().map(|b| b.value).collect::<Vec<_>>(), vec![2.0, 2.0]);

        o.normalize = "relative".into();
        let r = analyze(data, &o).unwrap();
        assert_eq!(r.bins.iter().map(|b| b.value).collect::<Vec<_>>(), vec![0.5, 0.5]);

        o.normalize = "percent".into();
        let pc = analyze(data, &o).unwrap();
        assert_eq!(pc.bins.iter().map(|b| b.value).collect::<Vec<_>>(), vec![50.0, 50.0]);

        o.normalize = "cumulative_count".into();
        let cc = analyze(data, &o).unwrap();
        assert_eq!(cc.bins.iter().map(|b| b.value).collect::<Vec<_>>(), vec![2.0, 4.0]);

        o.normalize = "cumulative_percent".into();
        let cp = analyze(data, &o).unwrap();
        assert_eq!(cp.bins.iter().map(|b| b.value).collect::<Vec<_>>(), vec![50.0, 100.0]);

        o.normalize = "density".into();
        let d = analyze(data, &o).unwrap();
        // density integrates to 1 over the range
        let area: f64 = d.bins.iter().map(|b| b.value * d.bin_width).sum();
        assert!((area - 1.0).abs() < 1e-12, "density must integrate to 1, got {area}");
    }

    #[test]
    fn range_bounds_clip_values_and_report_the_exclusions() {
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 2;
        o.range_min = "0".into();
        o.range_max = "10".into();
        let h = analyze("-5\n1\n2\n8\n9\n50", &o).unwrap();
        assert_eq!(h.n, 4);
        assert_eq!(h.excluded, 2);
        assert_eq!(h.bins[0].lower, 0.0);
        assert_eq!(h.bins[1].upper, 10.0);
    }

    #[test]
    fn header_row_and_mixed_separators_parse() {
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 2;
        let h = analyze("value\n1, 2\t3;4\n5 6", &o).unwrap();
        assert_eq!(h.n, 6);
        assert_eq!(h.min, 1.0);
        assert_eq!(h.max, 6.0);
    }

    #[test]
    fn identical_values_still_render() {
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 3;
        let h = analyze("7\n7\n7\n7", &o).unwrap();
        assert_eq!(h.n, 4);
        assert!(h.bin_width > 0.0);
        assert_eq!(h.bins.iter().map(|b| b.count).sum::<usize>(), 4);
    }

    #[test]
    fn scientific_notation_and_negatives_parse() {
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 2;
        let h = analyze("-1.5e2\n+3\n0.001\n1e3", &o).unwrap();
        assert_eq!(h.n, 4);
        assert_eq!(h.min, -150.0);
        assert_eq!(h.max, 1000.0);
    }

    // --- error paths -------------------------------------------------------

    #[test]
    fn non_numeric_cell_is_a_line_numbered_error() {
        let err = analyze("1\n2\nthree\n4", &opts()).unwrap_err();
        assert!(err.contains("line 3"), "got: {err}");
        assert!(err.contains("three"), "got: {err}");
    }

    #[test]
    fn too_few_values_is_an_error() {
        let err = analyze("42", &opts()).unwrap_err();
        assert!(err.contains("at least 2"), "got: {err}");
    }

    #[test]
    fn width_mode_without_a_width_is_an_error() {
        let mut o = opts();
        o.bin_method = "width".into();
        let err = analyze("1\n2\n3", &o).unwrap_err();
        assert!(err.contains("positive bin_width"), "got: {err}");
    }

    #[test]
    fn width_mode_over_the_bin_cap_is_an_error() {
        let mut o = opts();
        o.bin_method = "width".into();
        o.bin_width = 0.001;
        let err = analyze("0\n1000", &o).unwrap_err();
        assert!(err.contains("bin cap") || err.contains("500-bin cap"), "got: {err}");
    }

    #[test]
    fn inverted_range_is_an_error() {
        let mut o = opts();
        o.range_min = "10".into();
        o.range_max = "1".into();
        let err = analyze("1\n2\n3", &o).unwrap_err();
        assert!(err.contains("range_min"), "got: {err}");
    }

    #[test]
    fn unknown_enum_values_are_errors() {
        let mut o = opts();
        o.bin_method = "nope".into();
        assert!(analyze("1\n2\n3", &o).unwrap_err().contains("unknown bin_method"));

        let mut o = opts();
        o.normalize = "nope".into();
        assert!(analyze("1\n2\n3", &o).unwrap_err().contains("unknown normalize"));

        let mut o = opts();
        o.output = "nope".into();
        assert!(render("1\n2\n3", &o).unwrap_err().contains("unknown output"));
    }

    #[test]
    fn too_many_values_is_an_error() {
        let data: String = (0..MAX_VALUES + 5).map(|i| format!("{}\n", i % 97)).collect();
        let err = analyze(&data, &opts()).unwrap_err();
        assert!(err.contains("too many values"), "got: {err}");
    }

    // --- rendering ---------------------------------------------------------

    #[test]
    fn svg_is_wellformed_and_honours_style_options() {
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 4;
        o.title = "Response times".into();
        o.x_label = "ms".into();
        o.color = "#dc2626".into();
        o.theme = "dark".into();
        o.show_mean = true;
        o.show_median = true;
        o.rug = true;
        o.normal_curve = true;
        o.show_values = true;
        let svg = render("1\n2\n3\n4\n5\n6\n7\n8", &o).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("Response times"));
        assert!(svg.contains(">ms<"));
        assert!(svg.contains("#dc2626"));
        assert!(svg.contains("#0f172a")); // dark background
        assert!(svg.contains("<rect"));
        assert!(svg.contains("mean"));
        assert!(svg.contains("median"));
        assert!(svg.contains("<polyline")); // normal curve
        assert!(!svg.contains("NaN"));
    }

    #[test]
    fn horizontal_orientation_renders() {
        let mut o = opts();
        o.orientation = "horizontal".into();
        o.bin_method = "count".into();
        o.bins = 3;
        let svg = render("1\n2\n3\n4\n5\n6", &o).unwrap();
        assert!(svg.contains("<rect"));
        assert!(!svg.contains("NaN"));
    }

    #[test]
    fn table_output_is_exact() {
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 2;
        o.output = "table".into();
        let out = render("1\n2\n3\n4", &o).unwrap();
        assert!(out.contains("[1, 2.5)"), "got:\n{out}");
        assert!(out.contains("[2.5, 4]"), "got:\n{out}");
        assert!(out.contains("bins: 2 (count rule), width 1.5"), "got:\n{out}");
        assert!(out.contains("n: 4"), "got:\n{out}");
    }

    #[test]
    fn csv_output_has_a_header_and_one_row_per_bin() {
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 2;
        o.output = "csv".into();
        let out = render("1\n2\n3\n4", &o).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("bin,lower,upper,count,percent"));
        assert!(lines[1].contains("\"[1, 2.5)\""));
    }

    #[test]
    fn json_output_parses_as_json_shaped_text() {
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 2;
        o.output = "json".into();
        let out = render("1\n2\n3\n4", &o).unwrap();
        assert!(out.starts_with('{') && out.ends_with('}'));
        assert!(out.contains("\"rule\": \"count\""));
        assert!(out.contains("\"bin_count\": 2"));
        assert!(out.contains("\"cumulative_count\": 4"));
        assert!(!out.contains("NaN"));
    }

    #[test]
    fn title_and_labels_are_escaped() {
        let mut o = opts();
        o.title = "<script>x</script>".into();
        let svg = render("1\n2\n3", &o).unwrap();
        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&lt;script&gt;"));
    }

    #[test]
    fn output_is_deterministic() {
        let o = opts();
        let a = render("3\n1\n4\n1\n5\n9\n2\n6", &o).unwrap();
        let b = render("3\n1\n4\n1\n5\n9\n2\n6", &o).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn max_bins_is_the_cap() {
        let mut o = opts();
        o.bin_method = "count".into();
        o.bins = 500;
        let h = analyze("0\n1000", &o).unwrap();
        assert_eq!(h.bins.len(), MAX_BINS);
    }
}
