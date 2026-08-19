//! gizza-ai/histogram-bin-calculator core — recommend how many histogram bins a
//! dataset should have, and show the histogram that results.
//!
//! Five classic bin-selection rules are evaluated side by side (Sturges, Scott,
//! Freedman–Diaconis, Rice, square-root) plus numpy-style `auto` and a manual
//! bin count. The chosen rule is then applied to build the actual bin edges,
//! counts, percentages, cumulative totals, and densities.
//!
//! Pure-Rust, dependency-free besides serde for the JSON output shape.

use serde::Serialize;

/// Hard cap on the number of values accepted, so a pasted spreadsheet column
/// can't wedge the browser tab.
pub const MAX_VALUES: usize = 100_000;
/// Hard cap on bins, so a near-zero rule width can't ask for millions of rows.
pub const MAX_BINS: usize = 1_000;
/// Width of the longest ASCII bar in the report chart.
const BAR_WIDTH: usize = 40;

/// A bin-count/bin-width recommendation from one rule.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuleResult {
    /// Canonical rule id (`sturges`, `scott`, …).
    pub rule: String,
    /// Human-readable rule name.
    pub name: String,
    /// Recommended number of bins over the histogram range.
    pub bins: usize,
    /// Width of each bin once `bins` bins span the range (`range / bins`).
    pub width: f64,
    /// The formula, plus any raw width or fallback the rule produced.
    pub note: String,
}

/// One histogram bin.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Bin {
    /// 1-based bin number.
    pub index: usize,
    pub lower: f64,
    pub upper: f64,
    /// Interval label honouring the closed side, e.g. `[0, 10)`.
    pub label: String,
    /// Class mark — the centre of the interval, `(lower + upper) / 2`.
    pub midpoint: f64,
    pub count: usize,
    /// Share of the counted values, 0–100.
    pub percent: f64,
    pub cumulative_count: usize,
    pub cumulative_percent: f64,
    /// `count / (n * width)` — the height of a unit-area histogram.
    pub density: f64,
}

/// Descriptive statistics of the input, as used by the rules themselves.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    pub n: usize,
    pub min: f64,
    pub max: f64,
    pub range: f64,
    pub mean: f64,
    pub median: f64,
    /// Sample standard deviation (divide by n-1) — the σ̂ Scott's rule takes.
    pub std_dev: f64,
    pub q1: f64,
    pub q3: f64,
    pub iqr: f64,
    /// Moment skewness g1; 0 for a symmetric sample.
    pub skewness: f64,
    /// Excess kurtosis g2 (0 for a normal sample).
    pub excess_kurtosis: f64,
    /// Values outside the 1.5 × IQR fences.
    pub outliers: usize,
}

/// The full analysis: input stats, every rule's recommendation, and the
/// histogram built from the chosen one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub summary: Summary,
    pub rules: Vec<RuleResult>,
    pub chosen: RuleResult,
    /// Lower edge of the first bin.
    pub range_start: f64,
    /// Upper edge of the last bin.
    pub range_end: f64,
    pub bin_width: f64,
    pub bins: Vec<Bin>,
    /// Values counted into the bins (n minus anything outside an explicit range).
    pub counted: usize,
    /// Values below `range_start`, dropped from the histogram.
    pub excluded_below: usize,
    /// Values above `range_end`, dropped from the histogram.
    pub excluded_above: usize,
    /// `true` when intervals are `(a, b]` instead of `[a, b)`.
    pub right_closed: bool,
    /// Anything the caller should know: a degenerate range, a bin cap, a
    /// fallback when a rule was undefined.
    pub notes: Vec<String>,
}

/// Round to `precision` decimals and print minimally: `1.0` → `1`,
/// `-0.0` → `0`, `0.50` → `0.5`.
fn fmt_num(x: f64, precision: u32) -> String {
    if !x.is_finite() {
        return "n/a".to_string();
    }
    let s = format!("{:.*}", precision as usize, x);
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

/// Round a value to `precision` decimals, for the machine-readable outputs.
fn round_to(x: f64, precision: u32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = 10f64.powi(precision as i32);
    let r = (x * f).round() / f;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Linear-interpolation percentile (the numpy/Excel-inclusive default) over an
/// already-sorted slice. `p` is in 0.0..=1.0.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    if n == 1 {
        return sorted[0];
    }
    let rank = p * (n as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo as f64)
    }
}

/// Parse a column of numbers separated by newlines, commas, semicolons, pipes,
/// tabs, or spaces. Plain decimals and scientific notation only.
fn parse_numbers(text: &str) -> Result<Vec<f64>, String> {
    let mut nums = Vec::new();
    for tok in text.split(|c: char| {
        c.is_whitespace() || c == ',' || c == ';' || c == '|' || c == '\t'
    }) {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        match t.parse::<f64>() {
            Ok(v) if v.is_finite() => nums.push(v),
            _ => {
                return Err(format!(
                    "'{t}' is not a number — paste plain decimals or scientific notation (e.g. 12, -3.5, 1.2e3) and strip currency symbols or thousands separators"
                ))
            }
        }
        if nums.len() > MAX_VALUES {
            return Err(format!("too many values — the limit is {MAX_VALUES}"));
        }
    }
    Ok(nums)
}

/// Optional numeric bound: blank/`auto` means "use the data".
fn parse_bound(s: &str, what: &str) -> Result<Option<f64>, String> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    match t.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(Some(v)),
        _ => Err(format!(
            "{what} must be blank (use the data) or a finite number (got '{t}')"
        )),
    }
}

/// Smallest "nice" step (1, 2, 2.5 or 5 × a power of ten) that is at least `w`.
fn nice_width(w: f64) -> f64 {
    if !(w > 0.0) || !w.is_finite() {
        return w;
    }
    let exp = w.log10().floor();
    let pow = 10f64.powf(exp);
    let frac = w / pow;
    let mult = if frac <= 1.0 {
        1.0
    } else if frac <= 2.0 {
        2.0
    } else if frac <= 2.5 {
        2.5
    } else if frac <= 5.0 {
        5.0
    } else {
        10.0
    };
    mult * pow
}

/// Convert a rule's raw bin WIDTH into a bin count over `range`.
fn bins_from_width(range: f64, h: f64) -> Option<usize> {
    if !(h > 0.0) || !h.is_finite() || !(range > 0.0) {
        return None;
    }
    let k = (range / h).ceil();
    if !k.is_finite() || k < 1.0 {
        return Some(1);
    }
    Some((k as usize).min(MAX_BINS).max(1))
}

fn count_rule(rule: &str, name: &str, k: usize, range: f64, note: String) -> RuleResult {
    let k = k.clamp(1, MAX_BINS);
    RuleResult {
        rule: rule.to_string(),
        name: name.to_string(),
        bins: k,
        width: if range > 0.0 { range / k as f64 } else { 0.0 },
        note,
    }
}

/// Evaluate every rule against the sample. `range` is the histogram span the
/// bins have to cover (which may be a caller-supplied range, not min..max).
fn all_rules(s: &Summary, range: f64) -> Vec<RuleResult> {
    let n = s.n as f64;
    let mut out = Vec::new();

    let sturges_k = (n.log2().ceil() as usize) + 1;
    out.push(count_rule(
        "sturges",
        "Sturges",
        sturges_k,
        range,
        "k = ceil(log2(n)) + 1 — the classic default; over-smooths past n ≈ 200 or skewed data"
            .to_string(),
    ));

    let scott_h = 3.49 * s.std_dev * n.powf(-1.0 / 3.0);
    match bins_from_width(range, scott_h) {
        Some(k) => out.push(count_rule(
            "scott",
            "Scott",
            k,
            range,
            format!(
                "h = 3.49 × sd × n^(-1/3) = {} — minimises error for roughly normal data",
                fmt_num(scott_h, 6)
            ),
        )),
        None => out.push(RuleResult {
            rule: "scott".to_string(),
            name: "Scott".to_string(),
            bins: sturges_k.clamp(1, MAX_BINS),
            width: if range > 0.0 {
                range / sturges_k.clamp(1, MAX_BINS) as f64
            } else {
                0.0
            },
            note: "h = 3.49 × sd × n^(-1/3) is undefined (standard deviation is 0) — fell back to Sturges"
                .to_string(),
        }),
    }

    let fd_h = 2.0 * s.iqr * n.powf(-1.0 / 3.0);
    match bins_from_width(range, fd_h) {
        Some(k) => out.push(count_rule(
            "freedman_diaconis",
            "Freedman–Diaconis",
            k,
            range,
            format!(
                "h = 2 × IQR × n^(-1/3) = {} — robust to outliers and skew",
                fmt_num(fd_h, 6)
            ),
        )),
        None => out.push(RuleResult {
            rule: "freedman_diaconis".to_string(),
            name: "Freedman–Diaconis".to_string(),
            bins: sturges_k.clamp(1, MAX_BINS),
            width: if range > 0.0 {
                range / sturges_k.clamp(1, MAX_BINS) as f64
            } else {
                0.0
            },
            note: "h = 2 × IQR × n^(-1/3) is undefined (IQR is 0) — fell back to Sturges"
                .to_string(),
        }),
    }

    out.push(count_rule(
        "rice",
        "Rice",
        (2.0 * n.powf(1.0 / 3.0)).ceil() as usize,
        range,
        "k = ceil(2 × n^(1/3)) — a simple alternative to Sturges for larger samples".to_string(),
    ));

    out.push(count_rule(
        "sqrt",
        "Square root",
        n.sqrt().ceil() as usize,
        range,
        "k = ceil(sqrt(n)) — the spreadsheet default; quick but ignores spread".to_string(),
    ));

    // numpy's `auto`: the finer of Sturges and Freedman–Diaconis.
    let fd = out.iter().find(|r| r.rule == "freedman_diaconis").unwrap();
    let auto_k = if s.iqr > 0.0 {
        sturges_k.max(fd.bins)
    } else {
        sturges_k
    };
    let auto_note = if s.iqr > 0.0 {
        format!(
            "max(Sturges {sturges_k}, Freedman–Diaconis {}) — the numpy 'auto' rule",
            fd.bins
        )
    } else {
        "IQR is 0, so Freedman–Diaconis is undefined — Sturges only".to_string()
    };
    out.insert(0, count_rule("auto", "Auto", auto_k, range, auto_note));

    out
}

fn summarize(values: &[f64]) -> Summary {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = sorted.len();
    let nf = n as f64;
    let mean = sorted.iter().sum::<f64>() / nf;
    let m2 = sorted.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / nf;
    let m3 = sorted.iter().map(|v| (v - mean).powi(3)).sum::<f64>() / nf;
    let m4 = sorted.iter().map(|v| (v - mean).powi(4)).sum::<f64>() / nf;
    let std_dev = if n >= 2 {
        (sorted.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (nf - 1.0)).sqrt()
    } else {
        0.0
    };
    let q1 = percentile(&sorted, 0.25);
    let q3 = percentile(&sorted, 0.75);
    let iqr = q3 - q1;
    let (lo_fence, hi_fence) = (q1 - 1.5 * iqr, q3 + 1.5 * iqr);
    Summary {
        n,
        min: sorted[0],
        max: sorted[n - 1],
        range: sorted[n - 1] - sorted[0],
        mean,
        median: percentile(&sorted, 0.5),
        std_dev,
        q1,
        q3,
        iqr,
        skewness: if m2 > 0.0 { m3 / m2.powf(1.5) } else { 0.0 },
        excess_kurtosis: if m2 > 0.0 { m4 / (m2 * m2) - 3.0 } else { 0.0 },
        outliers: sorted
            .iter()
            .filter(|v| **v < lo_fence || **v > hi_fence)
            .count(),
    }
}

fn interval_label(lo: f64, hi: f64, first: bool, last: bool, right_closed: bool, p: u32) -> String {
    let (open, close) = if right_closed {
        (if first { '[' } else { '(' }, ']')
    } else {
        ('[', if last { ']' } else { ')' })
    };
    format!("{open}{}, {}{close}", fmt_num(lo, p), fmt_num(hi, p))
}

/// Canonicalise a rule name, or explain what the accepted ones are.
fn canonical_rule(rule: &str) -> Result<&'static str, String> {
    let rule_id = rule.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    Ok(match rule_id.as_str() {
        "" | "auto" => "auto",
        "sturges" => "sturges",
        "scott" => "scott",
        "freedman_diaconis" | "fd" => "freedman_diaconis",
        "rice" => "rice",
        "sqrt" | "square_root" => "sqrt",
        "manual" => "manual",
        other => {
            return Err(format!(
                "unknown rule '{other}' — use auto, sturges, scott, freedman_diaconis, rice, sqrt, or manual"
            ))
        }
    })
}

/// Parse the input, evaluate every rule, and build the histogram the chosen
/// rule implies. Rendering is left to the callers of [`run`].
#[allow(clippy::too_many_arguments)]
fn analyze(
    numbers: &str,
    rule: &str,
    bins: u32,
    range_min: &str,
    range_max: &str,
    nice_edges: bool,
    right_closed: bool,
    precision: u32,
) -> Result<Report, String> {
    if precision > 12 {
        return Err(format!("precision must be 0-12 (got {precision})"));
    }
    let rule_id = canonical_rule(rule)?;

    let values = parse_numbers(numbers)?;
    if values.len() < 2 {
        return Err(format!(
            "need at least 2 numbers to size a histogram (got {})",
            values.len()
        ));
    }
    let summary = summarize(&values);

    let lo_opt = parse_bound(range_min, "range_min")?;
    let hi_opt = parse_bound(range_max, "range_max")?;
    let mut start = lo_opt.unwrap_or(summary.min);
    let mut end = hi_opt.unwrap_or(summary.max);
    if end < start {
        return Err(format!(
            "range_min ({}) must be less than range_max ({})",
            fmt_num(start, precision),
            fmt_num(end, precision)
        ));
    }

    let mut notes: Vec<String> = Vec::new();
    let span = end - start;
    let rules = all_rules(&summary, span);

    let chosen = if rule_id == "manual" {
        let k = bins as usize;
        if k < 1 || k > MAX_BINS {
            return Err(format!("bins must be 1-{MAX_BINS} (got {bins})"));
        }
        count_rule(
            "manual",
            "Manual",
            k,
            span,
            format!("bin count you set ({k})"),
        )
    } else {
        rules
            .iter()
            .find(|r| r.rule == rule_id)
            .cloned()
            .expect("every non-manual rule is evaluated")
    };

    // Degenerate span: every value is identical (or the caller asked for a
    // zero-width range). One bin is the only honest answer.
    if !(span > 0.0) {
        notes.push(
            "Every value is the same, so there is no range to divide — a histogram needs spread. One bin is shown."
                .to_string(),
        );
        let counted = values.iter().filter(|v| **v == start).count();
        let below = values.iter().filter(|v| **v < start).count();
        let above = values.iter().filter(|v| **v > end).count();
        let pct = if counted > 0 { 100.0 } else { 0.0 };
        return Ok(Report {
            bins: vec![Bin {
                index: 1,
                lower: round_to(start, precision),
                upper: round_to(end, precision),
                label: format!("[{}, {}]", fmt_num(start, precision), fmt_num(end, precision)),
                midpoint: round_to((start + end) / 2.0, precision),
                count: counted,
                percent: round_to(pct, 4),
                cumulative_count: counted,
                cumulative_percent: round_to(pct, 4),
                density: 0.0,
            }],
            summary,
            rules,
            chosen: RuleResult {
                bins: 1,
                width: 0.0,
                ..chosen
            },
            range_start: round_to(start, precision),
            range_end: round_to(end, precision),
            bin_width: 0.0,
            counted,
            excluded_below: below,
            excluded_above: above,
            right_closed,
            notes,
        });
    }

    let mut k = chosen.bins;
    let mut width = span / k as f64;

    if nice_edges {
        let nice = nice_width(width);
        if lo_opt.is_none() {
            // Snap the first edge down to a multiple of the rounded width.
            start = (start / nice).floor() * nice;
        }
        if hi_opt.is_none() {
            end = summary.max;
        }
        k = (((end - start) / nice).ceil() as usize).clamp(1, MAX_BINS);
        width = nice;
        end = start + width * k as f64;
        notes.push(format!(
            "Rounded edges on: bin width snapped up to {} and the first edge to {}, giving {k} bins.",
            fmt_num(width, precision),
            fmt_num(start, precision)
        ));
    }

    if k >= MAX_BINS {
        notes.push(format!(
            "Bin count capped at {MAX_BINS}; the rule asked for more than the cap allows."
        ));
    }

    let mut counts = vec![0usize; k];
    let (mut below, mut above) = (0usize, 0usize);
    for v in &values {
        if *v < start {
            below += 1;
            continue;
        }
        if *v > end {
            above += 1;
            continue;
        }
        let idx = if right_closed {
            let raw = ((v - start) / width).ceil() as i64 - 1;
            raw.max(0) as usize
        } else {
            ((v - start) / width).floor().max(0.0) as usize
        };
        counts[idx.min(k - 1)] += 1;
    }
    let counted: usize = counts.iter().sum();

    let denom = counted.max(1) as f64;
    let mut running = 0usize;
    let mut bins_out = Vec::with_capacity(k);
    for (i, c) in counts.iter().enumerate() {
        running += c;
        let lower = start + width * i as f64;
        let upper = start + width * (i + 1) as f64;
        bins_out.push(Bin {
            index: i + 1,
            lower: round_to(lower, precision),
            upper: round_to(upper, precision),
            label: interval_label(lower, upper, i == 0, i + 1 == k, right_closed, precision),
            midpoint: round_to((lower + upper) / 2.0, precision),
            count: *c,
            percent: round_to(*c as f64 / denom * 100.0, 4),
            cumulative_count: running,
            cumulative_percent: round_to(running as f64 / denom * 100.0, 4),
            density: round_to(*c as f64 / (denom * width), 8),
        });
    }

    if below > 0 || above > 0 {
        notes.push(format!(
            "{below} value(s) fell below the range and {above} above it — they are not counted."
        ));
    }

    Ok(Report {
        summary,
        rules,
        chosen: RuleResult {
            bins: k,
            width,
            ..chosen
        },
        range_start: round_to(start, precision),
        range_end: round_to(end, precision),
        bin_width: round_to(width, precision),
        bins: bins_out,
        counted,
        excluded_below: below,
        excluded_above: above,
        right_closed,
        notes,
    })
}

fn pad(s: &str, w: usize) -> String {
    let len = s.chars().count();
    if len >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - len))
    }
}

fn lpad(s: &str, w: usize) -> String {
    let len = s.chars().count();
    if len >= w {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(w - len))
    }
}

fn bin_columns(cumulative: bool, density: bool) -> Vec<&'static str> {
    let mut cols = vec!["bin", "lower", "upper", "label", "count", "percent"];
    if cumulative {
        cols.push("cumulative_count");
        cols.push("cumulative_percent");
    }
    if density {
        cols.push("density");
    }
    cols
}

fn bin_row(b: &Bin, cumulative: bool, density: bool, p: u32) -> Vec<String> {
    let mut row = vec![
        b.index.to_string(),
        fmt_num(b.lower, p),
        fmt_num(b.upper, p),
        b.label.clone(),
        b.count.to_string(),
        format!("{}%", fmt_num(b.percent, 2)),
    ];
    if cumulative {
        row.push(b.cumulative_count.to_string());
        row.push(format!("{}%", fmt_num(b.cumulative_percent, 2)));
    }
    if density {
        row.push(fmt_num(b.density, 6));
    }
    row
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_delimited(r: &Report, cumulative: bool, density: bool, p: u32, sep: char) -> String {
    let cols = bin_columns(cumulative, density);
    let mut lines = Vec::with_capacity(r.bins.len() + 1);
    let esc = |s: &str| {
        if sep == ',' {
            csv_escape(s)
        } else {
            s.replace('\t', " ")
        }
    };
    lines.push(
        cols.iter()
            .map(|c| esc(c))
            .collect::<Vec<_>>()
            .join(&sep.to_string()),
    );
    for b in &r.bins {
        lines.push(
            bin_row(b, cumulative, density, p)
                .iter()
                .map(|c| esc(c))
                .collect::<Vec<_>>()
                .join(&sep.to_string()),
        );
    }
    lines.join("\n")
}

fn render_report(r: &Report, cumulative: bool, density: bool, chart: bool, p: u32) -> String {
    let mut out = String::new();
    let s = &r.summary;

    out.push_str("Recommended histogram bins\n");
    out.push_str("==========================\n\n");
    out.push_str(&format!(
        "Values: {}   Min: {}   Max: {}   Range: {}\n",
        s.n,
        fmt_num(s.min, p),
        fmt_num(s.max, p),
        fmt_num(s.range, p)
    ));
    out.push_str(&format!(
        "Mean: {}   Median: {}   Std dev (sample): {}\n",
        fmt_num(s.mean, p),
        fmt_num(s.median, p),
        fmt_num(s.std_dev, p)
    ));
    out.push_str(&format!(
        "Q1: {}   Q3: {}   IQR: {}   Outliers (1.5 × IQR): {}\n",
        fmt_num(s.q1, p),
        fmt_num(s.q3, p),
        fmt_num(s.iqr, p),
        s.outliers
    ));
    out.push_str(&format!(
        "Skewness: {}   Excess kurtosis: {}\n\n",
        fmt_num(s.skewness, 4),
        fmt_num(s.excess_kurtosis, 4)
    ));

    // Rule comparison table.
    let headers = ["Rule", "Bins", "Bin width", "How it is derived"];
    let rows: Vec<Vec<String>> = r
        .rules
        .iter()
        .map(|rr| {
            vec![
                rr.name.clone(),
                rr.bins.to_string(),
                fmt_num(rr.width, p),
                rr.note.clone(),
            ]
        })
        .collect();
    let mut w = [0usize; 3];
    for (i, item) in w.iter_mut().enumerate() {
        *item = headers[i].chars().count();
        for row in &rows {
            *item = (*item).max(row[i].chars().count());
        }
    }
    out.push_str(&format!(
        "{}  {}  {}  {}\n",
        pad(headers[0], w[0]),
        lpad(headers[1], w[1]),
        lpad(headers[2], w[2]),
        headers[3]
    ));
    for row in &rows {
        out.push_str(&format!(
            "{}  {}  {}  {}\n",
            pad(&row[0], w[0]),
            lpad(&row[1], w[1]),
            lpad(&row[2], w[2]),
            row[3]
        ));
    }

    out.push_str(&format!(
        "\nUsing {}: {} bins of width {} over [{}, {}]\n",
        r.chosen.name,
        r.chosen.bins,
        fmt_num(r.bin_width, p),
        fmt_num(r.range_start, p),
        fmt_num(r.range_end, p)
    ));
    out.push_str(&format!(
        "Intervals are {}.\n\n",
        if r.right_closed {
            "right-closed (a, b] — the first bin also includes its lower edge"
        } else {
            "left-closed [a, b) — the last bin also includes its upper edge"
        }
    ));

    // Bin table.
    let mut headers: Vec<String> = vec![
        "Bin".into(),
        "Range".into(),
        "Count".into(),
        "Percent".into(),
    ];
    if cumulative {
        headers.push("Cum.".into());
        headers.push("Cum. %".into());
    }
    if density {
        headers.push("Density".into());
    }
    if chart {
        headers.push("Histogram".into());
    }
    let max_count = r.bins.iter().map(|b| b.count).max().unwrap_or(0);
    let rows: Vec<Vec<String>> = r
        .bins
        .iter()
        .map(|b| {
            let mut row = vec![
                b.index.to_string(),
                b.label.clone(),
                b.count.to_string(),
                format!("{}%", fmt_num(b.percent, 2)),
            ];
            if cumulative {
                row.push(b.cumulative_count.to_string());
                row.push(format!("{}%", fmt_num(b.cumulative_percent, 2)));
            }
            if density {
                row.push(fmt_num(b.density, 6));
            }
            if chart {
                let bar = if b.count == 0 || max_count == 0 {
                    0
                } else {
                    ((b.count as f64 / max_count as f64) * BAR_WIDTH as f64).round().max(1.0)
                        as usize
                };
                row.push("#".repeat(bar));
            }
            row
        })
        .collect();
    let ncol = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in &rows {
        for (i, c) in row.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    let render_row = |cells: &[String]| -> String {
        let mut parts = Vec::with_capacity(ncol);
        for (i, c) in cells.iter().enumerate() {
            // Bin number, counts and percentages read better right-aligned;
            // the interval label and the bar chart left-aligned.
            let is_last = i + 1 == ncol;
            if i == 1 || (chart && is_last) {
                parts.push(pad(c, widths[i]));
            } else {
                parts.push(lpad(c, widths[i]));
            }
        }
        parts.join("  ").trim_end().to_string()
    };
    out.push_str(&render_row(&headers));
    out.push('\n');
    for row in &rows {
        out.push_str(&render_row(row));
        out.push('\n');
    }

    out.push_str(&format!("\nCounted {} of {} values.\n", r.counted, s.n));
    for n in &r.notes {
        out.push_str(&format!("Note: {n}\n"));
    }
    out.trim_end().to_string()
}

/// Full entry point: analyse `numbers` and render the requested output format.
#[allow(clippy::too_many_arguments)]
pub fn run(
    numbers: &str,
    rule: &str,
    bins: u32,
    range_min: &str,
    range_max: &str,
    nice_edges: bool,
    right_closed: bool,
    precision: u32,
    output: &str,
    cumulative: bool,
    density: bool,
    chart: bool,
) -> Result<String, String> {
    let report = analyze(
        numbers,
        rule,
        bins,
        range_min,
        range_max,
        nice_edges,
        right_closed,
        precision,
    )?;
    match output.trim().to_ascii_lowercase().as_str() {
        "" | "report" => Ok(render_report(&report, cumulative, density, chart, precision)),
        "table" => Ok(render_delimited(
            &report, cumulative, density, precision, '\t',
        )),
        "csv" => Ok(render_delimited(
            &report, cumulative, density, precision, ',',
        )),
        "json" => serde_json::to_string_pretty(&report).map_err(|e| e.to_string()),
        other => Err(format!(
            "unknown output '{other}' — use report, table, csv, or json"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "1 2 2 3 4 7 8 9 12 15 22 40";

    #[test]
    fn summary_matches_hand_computed_stats() {
        let r = analyze(SAMPLE, "auto", 10, "", "", false, false, 4).unwrap();
        assert_eq!(r.summary.n, 12);
        assert_eq!(r.summary.min, 1.0);
        assert_eq!(r.summary.max, 40.0);
        assert_eq!(r.summary.range, 39.0);
        assert_eq!(r.summary.median, 7.5);
        assert_eq!(r.summary.q1, 2.75);
        assert_eq!(r.summary.q3, 12.75);
        assert_eq!(r.summary.iqr, 10.0);
    }

    #[test]
    fn sturges_rule_is_ceil_log2_plus_one() {
        let r = analyze(SAMPLE, "sturges", 10, "", "", false, false, 4).unwrap();
        // n = 12 -> ceil(log2 12) = 4, + 1 = 5 bins.
        assert_eq!(r.chosen.bins, 5);
        assert!((r.bin_width - 39.0 / 5.0).abs() < 1e-9);
    }

    #[test]
    fn square_root_and_rice_rules() {
        let sqrt = analyze(SAMPLE, "sqrt", 10, "", "", false, false, 4).unwrap();
        assert_eq!(sqrt.chosen.bins, 4); // ceil(sqrt(12)) = 4
        let rice = analyze(SAMPLE, "rice", 10, "", "", false, false, 4).unwrap();
        assert_eq!(rice.chosen.bins, 5); // ceil(2 * 12^(1/3)) = ceil(4.579) = 5
    }

    #[test]
    fn freedman_diaconis_uses_the_iqr() {
        let r = analyze(SAMPLE, "freedman_diaconis", 10, "", "", false, false, 4).unwrap();
        // h = 2 * 10 * 12^(-1/3) = 8.7358 -> ceil(39 / 8.7358) = 5 bins.
        assert_eq!(r.chosen.bins, 5);
    }

    #[test]
    fn auto_is_the_finer_of_sturges_and_fd() {
        let r = analyze(SAMPLE, "auto", 10, "", "", false, false, 4).unwrap();
        let sturges = r.rules.iter().find(|x| x.rule == "sturges").unwrap().bins;
        let fd = r
            .rules
            .iter()
            .find(|x| x.rule == "freedman_diaconis")
            .unwrap()
            .bins;
        assert_eq!(r.chosen.bins, sturges.max(fd));
    }

    #[test]
    fn every_rule_is_reported_side_by_side() {
        let r = analyze(SAMPLE, "scott", 10, "", "", false, false, 4).unwrap();
        let ids: Vec<&str> = r.rules.iter().map(|x| x.rule.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "auto",
                "sturges",
                "scott",
                "freedman_diaconis",
                "rice",
                "sqrt"
            ]
        );
    }

    #[test]
    fn bins_partition_the_data_and_counts_sum_to_n() {
        let r = analyze(SAMPLE, "manual", 4, "", "", false, false, 4).unwrap();
        assert_eq!(r.bins.len(), 4);
        assert_eq!(r.bins.iter().map(|b| b.count).sum::<usize>(), 12);
        assert_eq!(r.counted, 12);
        // width = 39/4 = 9.75 -> [1, 10.75) [10.75, 20.5) [20.5, 30.25) [30.25, 40]
        assert_eq!(r.bins[0].label, "[1, 10.75)");
        assert_eq!(r.bins[3].label, "[30.25, 40]");
        assert_eq!(r.bins[0].count, 8);
        assert_eq!(r.bins[1].count, 2);
        assert_eq!(r.bins[3].count, 1);
        assert_eq!(r.bins[3].cumulative_count, 12);
    }

    #[test]
    fn right_closed_intervals_move_the_edge_values() {
        let left = analyze("0 5 10", "manual", 2, "", "", false, false, 4).unwrap();
        // [0, 5) [5, 10] -> 5 lands in the second bin.
        assert_eq!(left.bins[0].count, 1);
        assert_eq!(left.bins[1].count, 2);
        let right = analyze("0 5 10", "manual", 2, "", "", false, true, 4).unwrap();
        // [0, 5] (5, 10] -> 5 lands in the first bin.
        assert_eq!(right.bins[0].label, "[0, 5]");
        assert_eq!(right.bins[1].label, "(5, 10]");
        assert_eq!(right.bins[0].count, 2);
        assert_eq!(right.bins[1].count, 1);
    }

    #[test]
    fn nice_edges_round_the_width_and_the_first_edge() {
        let r = analyze(SAMPLE, "manual", 4, "", "", true, false, 4).unwrap();
        // raw width 9.75 -> nice 10, first edge floor(1/10)*10 = 0.
        assert_eq!(r.bin_width, 10.0);
        assert_eq!(r.range_start, 0.0);
        assert_eq!(r.bins.len(), 4);
        assert_eq!(r.bins[0].label, "[0, 10)");
    }

    #[test]
    fn explicit_range_excludes_outside_values() {
        let r = analyze("1 2 3 4 5 100", "manual", 2, "0", "10", false, false, 4).unwrap();
        assert_eq!(r.range_start, 0.0);
        assert_eq!(r.range_end, 10.0);
        assert_eq!(r.excluded_above, 1);
        assert_eq!(r.counted, 5);
        assert!(r.notes.iter().any(|n| n.contains("above it")));
    }

    #[test]
    fn density_and_percent_are_normalised_to_the_counted_values() {
        let r = analyze("0 0 0 0 10 10 10 10", "manual", 2, "", "", false, false, 6).unwrap();
        assert_eq!(r.bins[0].count, 4);
        assert_eq!(r.bins[0].percent, 50.0);
        // 4 / (8 * 5) = 0.1
        assert_eq!(r.bins[0].density, 0.1);
    }

    #[test]
    fn constant_data_collapses_to_one_bin_with_a_note() {
        let r = analyze("7 7 7 7", "auto", 10, "", "", false, false, 4).unwrap();
        assert_eq!(r.bins.len(), 1);
        assert_eq!(r.bins[0].count, 4);
        assert_eq!(r.bins[0].label, "[7, 7]");
        assert!(r.notes.iter().any(|n| n.contains("no range to divide")));
    }

    #[test]
    fn csv_output_has_a_header_and_one_row_per_bin() {
        let out = run(
            SAMPLE, "manual", 2, "", "", false, false, 4, "csv", true, true, false,
        )
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "bin,lower,upper,label,count,percent,cumulative_count,cumulative_percent,density"
        );
        assert_eq!(lines.len(), 3);
        assert!(
            lines[1].starts_with("1,1,20.5,\"[1, 20.5)\",10,83.33%,10,83.33%,"),
            "{}",
            lines[1]
        );
    }

    #[test]
    fn json_output_round_trips() {
        let out = run(
            SAMPLE, "sturges", 10, "", "", false, false, 4, "json", false, false, false,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["chosen"]["rule"], "sturges");
        assert_eq!(v["chosen"]["bins"], 5);
        assert_eq!(v["summary"]["n"], 12);
        assert_eq!(v["rules"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn report_shows_the_rule_table_and_ascii_bars() {
        let out = run(
            SAMPLE, "auto", 10, "", "", false, false, 4, "report", false, false, true,
        )
        .unwrap();
        assert!(out.contains("Recommended histogram bins"));
        assert!(out.contains("Freedman–Diaconis"));
        assert!(out.contains("Square root"));
        assert!(out.contains('#'));
        assert!(out.contains("Counted 12 of 12 values."));
    }

    #[test]
    fn separators_are_interchangeable() {
        let a = analyze("1,2,3,4", "manual", 2, "", "", false, false, 4).unwrap();
        let b = analyze("1\n2\n3\n4", "manual", 2, "", "", false, false, 4).unwrap();
        let c = analyze("1; 2 | 3\t4", "manual", 2, "", "", false, false, 4).unwrap();
        assert_eq!(a.bins, b.bins);
        assert_eq!(a.bins, c.bins);
    }

    #[test]
    fn non_numeric_value_is_rejected_with_the_offending_token() {
        let err = analyze("1 2 $3", "auto", 10, "", "", false, false, 4).unwrap_err();
        assert!(err.contains("'$3' is not a number"), "{err}");
    }

    #[test]
    fn too_few_numbers_is_an_error() {
        let err = analyze("5", "auto", 10, "", "", false, false, 4).unwrap_err();
        assert!(err.contains("at least 2 numbers"), "{err}");
    }

    #[test]
    fn bad_rule_bins_range_and_output_are_rejected() {
        assert!(analyze(SAMPLE, "elbow", 10, "", "", false, false, 4)
            .unwrap_err()
            .contains("unknown rule"));
        assert!(analyze(SAMPLE, "manual", 0, "", "", false, false, 4)
            .unwrap_err()
            .contains("bins must be 1-1000"));
        assert!(analyze(SAMPLE, "manual", 5001, "", "", false, false, 4)
            .unwrap_err()
            .contains("bins must be 1-1000"));
        assert!(analyze(SAMPLE, "auto", 10, "50", "10", false, false, 4)
            .unwrap_err()
            .contains("must be less than"));
        assert!(analyze(SAMPLE, "auto", 10, "abc", "", false, false, 4)
            .unwrap_err()
            .contains("range_min"));
        assert!(analyze(SAMPLE, "auto", 10, "", "", false, false, 13)
            .unwrap_err()
            .contains("precision must be 0-12"));
        assert!(run(
            SAMPLE, "auto", 10, "", "", false, false, 4, "chart", false, false, false
        )
        .unwrap_err()
        .contains("unknown output"));
    }

    #[test]
    fn nice_width_picks_one_two_two_and_a_half_or_five() {
        assert_eq!(nice_width(0.9), 1.0);
        assert_eq!(nice_width(1.4), 2.0);
        assert_eq!(nice_width(2.3), 2.5);
        assert_eq!(nice_width(4.1), 5.0);
        assert_eq!(nice_width(9.75), 10.0);
        assert_eq!(nice_width(120.0), 200.0);
    }
}
