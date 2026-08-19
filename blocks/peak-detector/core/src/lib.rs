//! peak-detector core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Finds local maxima (peaks) and local minima (valleys) in a pasted 1-D signal
//! and reports each one's index, value, prominence and width. Every candidate
//! can be filtered by amplitude band, vertical drop to its immediate
//! neighbours, spacing from other peaks, prominence and width — the same knobs
//! the reference implementations expose.
//!
//! Conventions, all deliberate and documented on the page:
//!   * indices are **0-based** (the first sample is index 0);
//!   * the first and last sample are never peaks (only one neighbour);
//!   * a flat top (plateau) collapses to its middle sample, rounded down;
//!   * prominence is the vertical drop the signal must make on both sides
//!     before rising back above the peak (or hitting an end);
//!   * width is measured across the peak at `rel_height` of its prominence,
//!     with linear interpolation between samples;
//!   * filters are evaluated in the documented order
//!     amplitude → threshold → distance → prominence → width.
//! Report-only: nothing is fetched, persisted, or re-ordered in the input.

use serde::Serialize;

/// Hard cap on how many values one run will analyse.
pub const MAX_VALUES: usize = 20_000;
/// Largest accepted moving-average smoothing window.
pub const MAX_SMOOTH: u64 = 501;
/// Largest accepted `max_peaks` value.
pub const MAX_PEAKS_CAP: u64 = 10_000;

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// One detected extremum.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Peak {
    /// "maximum" or "minimum".
    pub kind: String,
    /// 0-based index of the peak sample (middle of a plateau, rounded down).
    pub index: usize,
    /// The peak's value in the analysed (optionally smoothed) signal.
    pub value: f64,
    /// Vertical drop on both sides before the signal rises back above the peak.
    pub prominence: f64,
    /// 0-based index of the lowest point of the left descent.
    pub left_base: usize,
    /// 0-based index of the lowest point of the right descent.
    pub right_base: usize,
    /// Width in samples measured at `rel_height` of the prominence.
    pub width: f64,
    /// The signal value the width was measured at.
    pub width_height: f64,
    /// Interpolated left crossing point of the width line.
    pub left_ip: f64,
    /// Interpolated right crossing point of the width line.
    pub right_ip: f64,
    /// How many equal samples form this peak's flat top (1 = a single sample).
    pub plateau_size: usize,
    /// Drop from the peak to the sample just left of its flat top.
    pub left_threshold: f64,
    /// Drop from the peak to the sample just right of its flat top.
    pub right_threshold: f64,
}

/// Summary of the analysed signal.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SignalStats {
    /// Number of numeric values analysed.
    pub count: usize,
    /// Smallest value.
    pub min: f64,
    /// Largest value.
    pub max: f64,
    /// Arithmetic mean.
    pub mean: f64,
    /// `max - min`.
    pub range: f64,
    /// Moving-average window actually applied (1 = no smoothing).
    pub smooth_window: usize,
    /// Non-numeric tokens skipped.
    pub ignored_tokens: usize,
}

/// Echo of the filters that were actually in force.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Filters {
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub threshold: f64,
    pub min_distance: usize,
    pub min_prominence: f64,
    pub min_width: f64,
    pub rel_height: f64,
    pub max_peaks: usize,
    pub sort_by: String,
}

/// The full detection report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// "maxima", "minima" or "both".
    pub mode: String,
    /// Peaks in this report (after filtering, sorting and `max_peaks`).
    pub count: usize,
    /// Maxima in this report.
    pub maxima_count: usize,
    /// Minima in this report.
    pub minima_count: usize,
    /// How many peaks passed every filter, before `max_peaks` truncation.
    pub matched: usize,
    /// The peaks themselves.
    pub peaks: Vec<Peak>,
    /// How many local extrema existed before any filter was applied.
    pub candidates: usize,
    pub signal: SignalStats,
    pub filters: Filters,
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Parsed, validated options for one run.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// "maxima" | "minima" | "both".
    pub mode: String,
    /// "auto" | "comma" | "newline" | "space" | "semicolon" | "tab" | "pipe".
    pub separator: String,
    /// Odd moving-average window; 1 = no smoothing.
    pub smooth: usize,
    /// Lowest accepted extremum value (None = no floor).
    pub min_value: Option<f64>,
    /// Highest accepted extremum value (None = no ceiling).
    pub max_value: Option<f64>,
    /// Minimum drop to BOTH immediate neighbours; 0 = off.
    pub threshold: f64,
    /// Minimum samples between kept peaks of the same kind; 0 = off.
    pub min_distance: usize,
    /// Minimum prominence; 0 = off.
    pub min_prominence: f64,
    /// Minimum width in samples; 0 = off.
    pub min_width: f64,
    /// Fraction of the prominence the width is measured at (0-1).
    pub rel_height: f64,
    /// Keep at most this many peaks after sorting; 0 = all.
    pub max_peaks: usize,
    /// "position" | "prominence" | "value".
    pub sort_by: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: "maxima".into(),
            separator: "auto".into(),
            smooth: 1,
            min_value: None,
            max_value: None,
            threshold: 0.0,
            min_distance: 0,
            min_prominence: 0.0,
            min_width: 0.0,
            rel_height: 0.5,
            max_peaks: 0,
            sort_by: "position".into(),
        }
    }
}

fn pick(value: &str, allowed: &[&str], field: &str) -> Result<String, String> {
    let v = value.trim().to_ascii_lowercase();
    if v.is_empty() {
        return Ok(allowed[0].to_string());
    }
    if allowed.contains(&v.as_str()) {
        Ok(v)
    } else {
        Err(format!(
            "unknown {field} '{}' — expected one of: {}",
            value.trim(),
            allowed.join(", ")
        ))
    }
}

/// Parse an optional numeric bound: blank means "no bound".
fn optional_number(raw: &str, field: &str) -> Result<Option<f64>, String> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    match t.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(Some(v)),
        _ => Err(format!(
            "{field} must be a number (or blank for no limit) — got '{t}'"
        )),
    }
}

fn non_negative(value: f64, field: &str) -> Result<f64, String> {
    if !value.is_finite() || value < 0.0 {
        return Err(format!(
            "{field} must be a finite number of 0 or more (0 turns the filter off) — got {value}"
        ));
    }
    Ok(value)
}

impl Options {
    /// Build options from raw string/number fields (CLI, chat, page).
    #[allow(clippy::too_many_arguments)]
    pub fn from_raw(
        mode: &str,
        separator: &str,
        smooth: u64,
        min_value: &str,
        max_value: &str,
        threshold: f64,
        min_distance: u64,
        min_prominence: f64,
        min_width: f64,
        rel_height: f64,
        max_peaks: u64,
        sort_by: &str,
    ) -> Result<Options, String> {
        let mode = pick(mode, &["maxima", "minima", "both"], "mode")?;
        let separator = pick(
            separator,
            &[
                "auto",
                "comma",
                "newline",
                "space",
                "semicolon",
                "tab",
                "pipe",
            ],
            "separator",
        )?;
        let sort_by = pick(sort_by, &["position", "prominence", "value"], "sort_by")?;

        if smooth > MAX_SMOOTH {
            return Err(format!(
                "smooth must be between 0 and {MAX_SMOOTH} samples (0 or 1 = no smoothing) — got {smooth}"
            ));
        }
        if smooth > 1 && smooth % 2 == 0 {
            return Err(format!(
                "smooth must be an odd window so it stays centred on the sample — got {smooth}; \
                 try {} or {}",
                smooth - 1,
                smooth + 1
            ));
        }
        let smooth = smooth.max(1) as usize;

        if min_distance as usize > MAX_VALUES {
            return Err(format!(
                "min_distance must be between 0 and {MAX_VALUES} samples (0 turns it off) — got {min_distance}"
            ));
        }
        if max_peaks > MAX_PEAKS_CAP {
            return Err(format!(
                "max_peaks must be between 0 and {MAX_PEAKS_CAP} (0 returns every peak) — got {max_peaks}"
            ));
        }
        if !rel_height.is_finite() || !(0.0..=1.0).contains(&rel_height) {
            return Err(format!(
                "rel_height must be between 0 and 1 — 0.5 measures the width at half the peak's \
                 prominence, 1 measures it at the peak's base — got {rel_height}"
            ));
        }

        let min_value = optional_number(min_value, "min_value")?;
        let max_value = optional_number(max_value, "max_value")?;
        if let (Some(lo), Some(hi)) = (min_value, max_value) {
            if lo > hi {
                return Err(format!(
                    "min_value ({lo}) is above max_value ({hi}) — no value can pass that band"
                ));
            }
        }

        Ok(Options {
            mode,
            separator,
            smooth,
            min_value,
            max_value,
            threshold: non_negative(threshold, "threshold")?,
            min_distance: min_distance as usize,
            min_prominence: non_negative(min_prominence, "min_prominence")?,
            min_width: non_negative(min_width, "min_width")?,
            rel_height,
            max_peaks: max_peaks as usize,
            sort_by,
        })
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn is_delimiter(c: char, separator: &str) -> bool {
    match separator {
        "comma" => c == ',',
        "newline" => c == '\n' || c == '\r',
        "space" => c == ' ',
        "semicolon" => c == ';',
        "tab" => c == '\t',
        "pipe" => c == '|',
        // auto: any whitespace plus the usual list punctuation.
        _ => c.is_whitespace() || c == ',' || c == ';' || c == '|',
    }
}

/// Split raw input into non-empty tokens using the chosen separator.
pub fn tokenize(input: &str, separator: &str) -> Vec<String> {
    input
        .split(|c: char| is_delimiter(c, separator))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Parse the input into the signal. Non-numeric tokens are skipped and counted
/// so a pasted column header or a stray label does not stop the run; a run with
/// no numbers at all is an error.
pub fn parse_signal(input: &str, separator: &str) -> Result<(Vec<f64>, usize), String> {
    let tokens = tokenize(input, separator);
    let mut values: Vec<f64> = Vec::with_capacity(tokens.len());
    let mut ignored = 0usize;
    for token in &tokens {
        match token.parse::<f64>() {
            // NaN/inf have no meaningful ordering against neighbours.
            Ok(v) if v.is_finite() => values.push(v),
            _ => ignored += 1,
        }
        if values.len() > MAX_VALUES {
            return Err(format!(
                "too many values: this tool analyses up to {MAX_VALUES} samples per run (got more)"
            ));
        }
    }
    if values.is_empty() {
        return Err(
            "no numbers found — paste the signal as values separated by commas, spaces or newlines \
             (for example 0, 1, 0, 3, 0, 2, 0)"
                .into(),
        );
    }
    Ok((values, ignored))
}

/// Centred moving average. The window shrinks at the edges so the output has
/// the same length as the input and no artificial ramp-in.
pub fn smooth_signal(values: &[f64], window: usize) -> Vec<f64> {
    if window <= 1 {
        return values.to_vec();
    }
    let half = window / 2;
    let n = values.len();
    (0..n)
        .map(|i| {
            let lo = i.saturating_sub(half);
            let hi = (i + half + 1).min(n);
            let slice = &values[lo..hi];
            slice.iter().sum::<f64>() / slice.len() as f64
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// A local maximum of `work`, before any measurement or filtering.
struct Candidate {
    index: usize,
    left_edge: usize,
    right_edge: usize,
}

/// Find every local maximum of `work`, collapsing flat tops to their middle
/// sample. The first and last sample can never qualify.
fn local_maxima(work: &[f64]) -> Vec<Candidate> {
    let n = work.len();
    let mut out = Vec::new();
    if n < 3 {
        return out;
    }
    let mut i = 1usize;
    while i < n - 1 {
        if work[i - 1] < work[i] {
            let mut ahead = i + 1;
            while ahead < n && work[ahead] == work[i] {
                ahead += 1;
            }
            if ahead < n && work[ahead] < work[i] {
                out.push(Candidate {
                    index: (i + ahead - 1) / 2,
                    left_edge: i,
                    right_edge: ahead - 1,
                });
            }
            i = ahead;
        } else {
            i += 1;
        }
    }
    out
}

/// Prominence of `work[peak]`: the drop the signal makes on both sides before
/// climbing back above the peak (or reaching an end). Returns
/// `(prominence, left_base, right_base)`.
fn prominence_of(work: &[f64], peak: usize) -> (f64, usize, usize) {
    let h = work[peak];
    let n = work.len();

    let mut left_min = h;
    let mut left_base = peak;
    let mut i = peak as isize;
    while i >= 0 && work[i as usize] <= h {
        if work[i as usize] < left_min {
            left_min = work[i as usize];
            left_base = i as usize;
        }
        i -= 1;
    }

    let mut right_min = h;
    let mut right_base = peak;
    let mut j = peak;
    while j < n && work[j] <= h {
        if work[j] < right_min {
            right_min = work[j];
            right_base = j;
        }
        j += 1;
    }

    (h - left_min.max(right_min), left_base, right_base)
}

/// Width of a peak measured at `rel_height` of its prominence, interpolated
/// between samples. Returns `(width, height_line, left_ip, right_ip)`.
fn width_of(
    work: &[f64],
    peak: usize,
    prominence: f64,
    left_base: usize,
    right_base: usize,
    rel_height: f64,
) -> (f64, f64, f64, f64) {
    let line = work[peak] - prominence * rel_height;

    let mut i = peak;
    while i > left_base && line < work[i] {
        i -= 1;
    }
    let mut left_ip = i as f64;
    if work[i] < line {
        let rise = work[i + 1] - work[i];
        if rise > 0.0 {
            left_ip += (line - work[i]) / rise;
        }
    }

    let mut j = peak;
    while j < right_base && line < work[j] {
        j += 1;
    }
    let mut right_ip = j as f64;
    if work[j] < line {
        let fall = work[j - 1] - work[j];
        if fall > 0.0 {
            right_ip -= (line - work[j]) / fall;
        }
    }

    ((right_ip - left_ip).max(0.0), line, left_ip, right_ip)
}

/// Keep the tallest peak and drop every peak closer than `distance` samples to
/// an already-kept one (the reference implementations' rule).
fn select_by_distance(peaks: &mut Vec<Peak>, distance: usize, sign: f64) {
    if distance == 0 || peaks.len() < 2 {
        return;
    }
    let mut order: Vec<usize> = (0..peaks.len()).collect();
    // `sign` puts the most extreme peak first in its own direction.
    order.sort_by(|&a, &b| {
        (peaks[a].value * sign)
            .partial_cmp(&(peaks[b].value * sign))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut keep = vec![true; peaks.len()];
    for &j in order.iter().rev() {
        if !keep[j] {
            continue;
        }
        let mut k = j;
        while k > 0 && peaks[j].index - peaks[k - 1].index < distance {
            k -= 1;
            keep[k] = false;
        }
        let mut k = j + 1;
        while k < peaks.len() && peaks[k].index - peaks[j].index < distance {
            keep[k] = false;
            k += 1;
        }
    }
    let mut iter = keep.iter();
    peaks.retain(|_| *iter.next().unwrap());
}

/// Detect one direction. `sign` is `1.0` for maxima and `-1.0` for minima; the
/// minima pass simply runs the maxima algorithm on the negated signal and maps
/// the results back, which is the documented idiom in the reference tools.
fn detect_side(signal: &[f64], opts: &Options, sign: f64, kind: &str) -> (usize, Vec<Peak>) {
    let work: Vec<f64> = if sign < 0.0 {
        signal.iter().map(|v| -v).collect()
    } else {
        signal.to_vec()
    };

    let candidates = local_maxima(&work);
    let total = candidates.len();
    let mut peaks: Vec<Peak> = Vec::new();

    for c in &candidates {
        let value = work[c.index];
        let left_threshold = value - work[c.left_edge - 1];
        let right_threshold = value - work[c.right_edge + 1];

        // 1. amplitude band (on the value as the user sees it)
        let user_value = value * sign;
        if let Some(lo) = opts.min_value {
            if user_value < lo {
                continue;
            }
        }
        if let Some(hi) = opts.max_value {
            if user_value > hi {
                continue;
            }
        }
        // 2. drop to both immediate neighbours
        if opts.threshold > 0.0 && left_threshold.min(right_threshold) < opts.threshold {
            continue;
        }

        let (prominence, left_base, right_base) = prominence_of(&work, c.index);
        let (width, line, left_ip, right_ip) = width_of(
            &work,
            c.index,
            prominence,
            left_base,
            right_base,
            opts.rel_height,
        );

        peaks.push(Peak {
            kind: kind.to_string(),
            index: c.index,
            value: user_value,
            prominence,
            left_base,
            right_base,
            width,
            width_height: line * sign,
            left_ip,
            right_ip,
            plateau_size: c.right_edge - c.left_edge + 1,
            left_threshold,
            right_threshold,
        });
    }

    // 3. spacing (tallest first), then 4. prominence, then 5. width
    select_by_distance(&mut peaks, opts.min_distance, sign);
    peaks.retain(|p| p.prominence >= opts.min_prominence);
    peaks.retain(|p| p.width >= opts.min_width);

    (total, peaks)
}

/// Run the full detection and build the report.
pub fn detect(input: &str, opts: &Options) -> Result<Report, String> {
    let (raw, ignored_tokens) = parse_signal(input, &opts.separator)?;
    let signal = smooth_signal(&raw, opts.smooth);

    let mut candidates = 0usize;
    let mut peaks: Vec<Peak> = Vec::new();
    if opts.mode == "maxima" || opts.mode == "both" {
        let (n, mut found) = detect_side(&signal, opts, 1.0, "maximum");
        candidates += n;
        peaks.append(&mut found);
    }
    if opts.mode == "minima" || opts.mode == "both" {
        let (n, mut found) = detect_side(&signal, opts, -1.0, "minimum");
        candidates += n;
        peaks.append(&mut found);
    }

    match opts.sort_by.as_str() {
        "prominence" => peaks.sort_by(|a, b| {
            b.prominence
                .partial_cmp(&a.prominence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.index.cmp(&b.index))
        }),
        "value" => peaks.sort_by(|a, b| {
            b.value
                .partial_cmp(&a.value)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.index.cmp(&b.index))
        }),
        _ => peaks.sort_by(|a, b| a.index.cmp(&b.index).then(a.kind.cmp(&b.kind))),
    }

    let matched = peaks.len();
    if opts.max_peaks > 0 && peaks.len() > opts.max_peaks {
        peaks.truncate(opts.max_peaks);
    }

    let min = signal.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = signal.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mean = signal.iter().sum::<f64>() / signal.len() as f64;

    Ok(Report {
        mode: opts.mode.clone(),
        count: peaks.len(),
        maxima_count: peaks.iter().filter(|p| p.kind == "maximum").count(),
        minima_count: peaks.iter().filter(|p| p.kind == "minimum").count(),
        matched,
        candidates,
        peaks,
        signal: SignalStats {
            count: signal.len(),
            min,
            max,
            mean,
            range: max - min,
            smooth_window: opts.smooth,
            ignored_tokens,
        },
        filters: Filters {
            min_value: opts.min_value,
            max_value: opts.max_value,
            threshold: opts.threshold,
            min_distance: opts.min_distance,
            min_prominence: opts.min_prominence,
            min_width: opts.min_width,
            rel_height: opts.rel_height,
            max_peaks: opts.max_peaks,
            sort_by: opts.sort_by.clone(),
        },
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Format a value the way a person wrote it: whole numbers without a `.0`,
/// everything else rounded to 6 decimals with trailing zeros trimmed.
pub fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return format!("{v}");
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        one.to_string()
    } else {
        many.to_string()
    }
}

fn render_table(report: &Report, show_kind: bool) -> String {
    let mut header: Vec<String> = vec!["#".into()];
    if show_kind {
        header.push("kind".into());
    }
    header.extend(
        ["index", "value", "prominence", "width"]
            .iter()
            .map(|s| s.to_string()),
    );

    let mut rows: Vec<Vec<String>> = vec![header];
    for (i, p) in report.peaks.iter().enumerate() {
        let mut row = vec![format!("{}", i + 1)];
        if show_kind {
            row.push(if p.kind == "maximum" { "max".into() } else { "min".into() });
        }
        row.push(format!("{}", p.index));
        row.push(fmt_num(p.value));
        row.push(fmt_num(p.prominence));
        row.push(format!("{:.2}", p.width));
        rows.push(row);
    }

    let cols = rows[0].len();
    let widths: Vec<usize> = (0..cols)
        .map(|c| rows.iter().map(|r| r[c].len()).max().unwrap_or(0))
        .collect();

    rows.iter()
        .map(|r| {
            let cells: Vec<String> = r
                .iter()
                .enumerate()
                .map(|(c, cell)| format!("{cell:>width$}", width = widths[c]))
                .collect();
            format!("  {}", cells.join("  "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn filter_summary(opts: &Filters, smooth: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    if smooth > 1 {
        parts.push(format!("smooth={smooth}"));
    }
    if let Some(v) = opts.min_value {
        parts.push(format!("min_value={}", fmt_num(v)));
    }
    if let Some(v) = opts.max_value {
        parts.push(format!("max_value={}", fmt_num(v)));
    }
    if opts.threshold > 0.0 {
        parts.push(format!("threshold={}", fmt_num(opts.threshold)));
    }
    if opts.min_distance > 0 {
        parts.push(format!("min_distance={}", opts.min_distance));
    }
    if opts.min_prominence > 0.0 {
        parts.push(format!("min_prominence={}", fmt_num(opts.min_prominence)));
    }
    if opts.min_width > 0.0 {
        parts.push(format!("min_width={}", fmt_num(opts.min_width)));
    }
    if (opts.rel_height - 0.5).abs() > f64::EPSILON {
        parts.push(format!("rel_height={}", fmt_num(opts.rel_height)));
    }
    if opts.max_peaks > 0 {
        parts.push(format!("max_peaks={}", opts.max_peaks));
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

/// Render the report as the plain-text page/CLI output.
pub fn render_text(report: &Report) -> String {
    let mut out = String::new();
    let n = report.count;
    let total = report.signal.count;

    let noun = match report.mode.as_str() {
        "minima" => plural(n, "minimum", "minima"),
        "both" => plural(n, "extremum", "extrema"),
        _ => plural(n, "maximum", "maxima"),
    };

    if n == 0 {
        out.push_str(&format!("No {noun} found in {total} values.\n"));
        if report.candidates > 0 {
            out.push_str(&format!(
                "{} local {} were found but none passed the filters — lower min_prominence, \
                 min_width, threshold or the value band.\n",
                report.candidates,
                plural(report.candidates, "extremum", "extrema")
            ));
        } else if total < 3 {
            out.push_str(
                "A peak needs a sample on each side, so at least 3 values are required.\n",
            );
        } else {
            out.push_str(
                "The signal never rises and falls again — the first and last sample can never be \
                 peaks. Try mode=both, or smoothing a noisy signal first.\n",
            );
        }
    } else {
        out.push_str(&format!("{n} {noun} found in {total} values.\n"));
        if report.mode == "both" {
            out.push_str(&format!(
                "{} maxima, {} minima.\n",
                report.maxima_count, report.minima_count
            ));
        }
        out.push('\n');
        out.push_str(&render_table(report, report.mode == "both"));
        out.push('\n');
    }

    out.push('\n');

    if let Some(top) = report
        .peaks
        .iter()
        .filter(|p| p.kind == "maximum")
        .max_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal))
    {
        out.push_str(&format!(
            "Highest peak: index {}, value {} (prominence {})\n",
            top.index,
            fmt_num(top.value),
            fmt_num(top.prominence)
        ));
    }
    if let Some(low) = report
        .peaks
        .iter()
        .filter(|p| p.kind == "minimum")
        .min_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal))
    {
        out.push_str(&format!(
            "Deepest valley: index {}, value {} (prominence {})\n",
            low.index,
            fmt_num(low.value),
            fmt_num(low.prominence)
        ));
    }

    if report.count >= 2 {
        let mut idx: Vec<usize> = report.peaks.iter().map(|p| p.index).collect();
        idx.sort_unstable();
        let span = (idx[idx.len() - 1] - idx[0]) as f64;
        out.push_str(&format!(
            "Mean spacing: {:.2} samples\n",
            span / (idx.len() - 1) as f64
        ));
    }

    if report.matched > report.count {
        out.push_str(&format!(
            "Showing {} of {} matching peaks (max_peaks={})\n",
            report.count, report.matched, report.filters.max_peaks
        ));
    }

    out.push_str(&format!(
        "Signal: {} values, min {}, max {}, mean {}\n",
        report.signal.count,
        fmt_num(report.signal.min),
        fmt_num(report.signal.max),
        fmt_num(report.signal.mean)
    ));
    if report.signal.smooth_window > 1 {
        out.push_str(&format!(
            "Smoothing: moving average over {} samples (values above are smoothed)\n",
            report.signal.smooth_window
        ));
    }
    if report.signal.ignored_tokens > 0 {
        out.push_str(&format!(
            "Ignored non-numeric tokens: {}\n",
            report.signal.ignored_tokens
        ));
    }
    out.push_str(&format!(
        "Filters: {}\n",
        filter_summary(&report.filters, report.signal.smooth_window)
    ));

    out.trim_end().to_string()
}

/// Render the peak table as CSV, ready to paste into a spreadsheet.
pub fn render_csv(report: &Report) -> String {
    let mut out = String::from(
        "kind,index,value,prominence,width,width_height,left_base,right_base,left_ip,right_ip,plateau_size\n",
    );
    for p in &report.peaks {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            p.kind,
            p.index,
            fmt_num(p.value),
            fmt_num(p.prominence),
            fmt_num(p.width),
            fmt_num(p.width_height),
            p.left_base,
            p.right_base,
            fmt_num(p.left_ip),
            fmt_num(p.right_ip),
            p.plateau_size
        ));
    }
    out.trim_end().to_string()
}

/// Render the report as pretty JSON.
pub fn render_json(report: &Report) -> Result<String, String> {
    serde_json::to_string_pretty(report).map_err(|e| format!("could not serialize report: {e}"))
}

/// Full run: parse options, detect, render in the requested format.
#[allow(clippy::too_many_arguments)]
pub fn run_with_options(
    data: &str,
    mode: &str,
    separator: &str,
    smooth: u64,
    min_value: &str,
    max_value: &str,
    threshold: f64,
    min_distance: u64,
    min_prominence: f64,
    min_width: f64,
    rel_height: f64,
    max_peaks: u64,
    sort_by: &str,
    format: &str,
) -> Result<String, String> {
    let format = pick(format, &["text", "json", "csv"], "format")?;
    let opts = Options::from_raw(
        mode,
        separator,
        smooth,
        min_value,
        max_value,
        threshold,
        min_distance,
        min_prominence,
        min_width,
        rel_height,
        max_peaks,
        sort_by,
    )?;
    let report = detect(data, &opts)?;
    match format.as_str() {
        "json" => render_json(&report),
        "csv" => Ok(render_csv(&report)),
        _ => Ok(render_text(&report)),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAW: &str = "0, 1, 0, 3, 0, 2, 0";

    fn run(data: &str, mode: &str, format: &str) -> String {
        run_with_options(
            data, mode, "auto", 0, "", "", 0.0, 0, 0.0, 0.0, 0.5, 0, "position", format,
        )
        .unwrap()
    }

    fn text(data: &str) -> String {
        run(data, "maxima", "text")
    }

    #[test]
    fn finds_every_local_maximum_with_measurements() {
        let out = text(SAW);
        assert_eq!(
            out,
            "3 maxima found in 7 values.\n\
             \n\
             \x20 #  index  value  prominence  width\n\
             \x20 1      1      1           1   1.00\n\
             \x20 2      3      3           3   1.00\n\
             \x20 3      5      2           2   1.00\n\
             \n\
             Highest peak: index 3, value 3 (prominence 3)\n\
             Mean spacing: 2.00 samples\n\
             Signal: 7 values, min 0, max 3, mean 0.857143\n\
             Filters: none",
            "{out}"
        );
    }

    #[test]
    fn endpoints_are_never_peaks() {
        let out = text("5, 1, 0, 1, 5");
        assert!(out.starts_with("No maxima found in 5 values."), "{out}");
        assert!(out.contains("first and last sample can never be peaks"), "{out}");
    }

    #[test]
    fn minima_mode_finds_valleys_with_positive_prominence() {
        let out = run("5, 1, 5, 2, 5", "minima", "text");
        assert!(out.starts_with("2 minima found in 5 values."), "{out}");
        assert!(out.contains("Deepest valley: index 1, value 1 (prominence 4)"), "{out}");
        assert!(!out.contains("Highest peak"), "{out}");
    }

    #[test]
    fn both_mode_labels_each_extremum() {
        let out = run("0, 4, 1, 5, 0", "both", "text");
        assert!(out.starts_with("3 extrema found in 5 values."), "{out}");
        assert!(out.contains("2 maxima, 1 minima."), "{out}");
        assert!(out.contains("kind"), "{out}");
        assert!(out.contains("min"), "{out}");
        assert!(out.contains("Highest peak: index 3, value 5"), "{out}");
        assert!(out.contains("Deepest valley: index 2, value 1"), "{out}");
    }

    #[test]
    fn plateau_collapses_to_its_middle_sample() {
        // Flat top over indices 2,3,4 -> middle is 3.
        let out = text("0, 1, 5, 5, 5, 1, 0");
        assert!(out.contains("  1      3      5"), "{out}");
        let json = run("0, 1, 5, 5, 5, 1, 0", "maxima", "json");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["peaks"][0]["index"], serde_json::json!(3));
        assert_eq!(v["peaks"][0]["plateau_size"], serde_json::json!(3));
    }

    #[test]
    fn even_plateau_rounds_down() {
        // Flat top over indices 2,3 -> (2+3)/2 = 2.
        let json = run("0, 1, 5, 5, 1, 0", "maxima", "json");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["peaks"][0]["index"], serde_json::json!(2));
        assert_eq!(v["peaks"][0]["plateau_size"], serde_json::json!(2));
    }

    #[test]
    fn min_prominence_drops_the_shallow_bump() {
        // Index 1 only stands 1 above the dip before the taller peak at index 3.
        let out = run_with_options(
            "0, 10, 9, 11, 0", "maxima", "auto", 0, "", "", 0.0, 0, 2.0, 0.0, 0.5, 0, "position",
            "text",
        )
        .unwrap();
        assert!(out.starts_with("1 maximum found in 5 values."), "{out}");
        assert!(out.contains("Highest peak: index 3, value 11 (prominence 11)"), "{out}");
        assert!(out.contains("Filters: min_prominence=2"), "{out}");
    }

    #[test]
    fn value_band_filters_by_amplitude() {
        let base = "0, 2, 0, 6, 0, 9, 0";
        let lo = run_with_options(
            base, "maxima", "auto", 0, "5", "", 0.0, 0, 0.0, 0.0, 0.5, 0, "position", "text",
        )
        .unwrap();
        assert!(lo.starts_with("2 maxima found"), "{lo}");
        let band = run_with_options(
            base, "maxima", "auto", 0, "5", "7", 0.0, 0, 0.0, 0.0, 0.5, 0, "position", "text",
        )
        .unwrap();
        assert!(band.starts_with("1 maximum found"), "{band}");
        assert!(band.contains("Highest peak: index 3, value 6"), "{band}");
    }

    #[test]
    fn min_distance_keeps_the_tallest_of_a_close_pair() {
        let out = run_with_options(
            "0, 3, 0, 9, 0, 1, 0", "maxima", "auto", 0, "", "", 0.0, 3, 0.0, 0.0, 0.5, 0,
            "position", "text",
        )
        .unwrap();
        assert!(out.starts_with("1 maximum found in 7 values."), "{out}");
        assert!(out.contains("Highest peak: index 3, value 9"), "{out}");
    }

    #[test]
    fn threshold_requires_a_drop_to_both_neighbours() {
        // The peak at index 3 only rises 1 above its right neighbour.
        let out = run_with_options(
            "0, 5, 4, 6, 5, 0", "maxima", "auto", 0, "", "", 2.0, 0, 0.0, 0.0, 0.5, 0, "position",
            "text",
        )
        .unwrap();
        assert!(out.contains("Filters: threshold=2"), "{out}");
        assert!(out.starts_with("No maxima found in 6 values."), "{out}");
        assert!(out.contains("2 local extrema were found but none passed"), "{out}");
    }

    #[test]
    fn min_width_rejects_a_narrow_spike() {
        let out = run_with_options(
            "0, 0, 9, 0, 0, 1, 2, 3, 2, 1, 0",
            "maxima", "auto", 0, "", "", 0.0, 0, 0.0, 2.0, 0.5, 0, "position", "text",
        )
        .unwrap();
        assert!(out.starts_with("1 maximum found in 11 values."), "{out}");
        assert!(out.contains("Highest peak: index 7, value 3"), "{out}");
    }

    #[test]
    fn rel_height_widens_the_measurement_towards_the_base() {
        let half = run_with_options(
            "0, 1, 2, 3, 2, 1, 0", "maxima", "auto", 0, "", "", 0.0, 0, 0.0, 0.0, 0.5, 0,
            "position", "json",
        )
        .unwrap();
        let full = run_with_options(
            "0, 1, 2, 3, 2, 1, 0", "maxima", "auto", 0, "", "", 0.0, 0, 0.0, 0.0, 1.0, 0,
            "position", "json",
        )
        .unwrap();
        let h: serde_json::Value = serde_json::from_str(&half).unwrap();
        let f: serde_json::Value = serde_json::from_str(&full).unwrap();
        assert_eq!(h["peaks"][0]["width"], serde_json::json!(3.0));
        assert_eq!(f["peaks"][0]["width"], serde_json::json!(6.0));
    }

    #[test]
    fn smoothing_merges_noise_into_one_peak() {
        let noisy = "0, 1, 0, 8, 7, 9, 7, 8, 0, 1, 0";
        let raw = text(noisy);
        assert!(raw.starts_with("5 maxima found in 11 values."), "{raw}");
        let smoothed = run_with_options(
            noisy, "maxima", "auto", 5, "", "", 0.0, 0, 1.0, 0.0, 0.5, 0, "position", "text",
        )
        .unwrap();
        assert!(smoothed.starts_with("1 maximum found in 11 values."), "{smoothed}");
        assert!(smoothed.contains("Smoothing: moving average over 5 samples"), "{smoothed}");
    }

    #[test]
    fn sort_and_max_peaks_pick_the_top_results() {
        let data = "0, 2, 0, 9, 0, 5, 0";
        let by_prom = run_with_options(
            data, "maxima", "auto", 0, "", "", 0.0, 0, 0.0, 0.0, 0.5, 2, "prominence", "csv",
        )
        .unwrap();
        let lines: Vec<&str> = by_prom.lines().collect();
        assert_eq!(lines.len(), 3, "{by_prom}");
        assert!(lines[1].starts_with("maximum,3,9,9,"), "{by_prom}");
        assert!(lines[2].starts_with("maximum,5,5,5,"), "{by_prom}");

        let text_out = run_with_options(
            data, "maxima", "auto", 0, "", "", 0.0, 0, 0.0, 0.0, 0.5, 2, "value", "text",
        )
        .unwrap();
        assert!(text_out.contains("Showing 2 of 3 matching peaks (max_peaks=2)"), "{text_out}");
    }

    #[test]
    fn json_carries_the_machine_readable_report() {
        let out = run(SAW, "maxima", "json");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["mode"], serde_json::json!("maxima"));
        assert_eq!(v["count"], serde_json::json!(3));
        assert_eq!(v["candidates"], serde_json::json!(3));
        assert_eq!(v["peaks"][1]["index"], serde_json::json!(3));
        assert_eq!(v["peaks"][1]["value"], serde_json::json!(3.0));
        assert_eq!(v["peaks"][1]["prominence"], serde_json::json!(3.0));
        assert_eq!(v["peaks"][1]["left_base"], serde_json::json!(2));
        assert_eq!(v["peaks"][1]["right_base"], serde_json::json!(4));
        assert_eq!(v["peaks"][1]["left_ip"], serde_json::json!(2.5));
        assert_eq!(v["peaks"][1]["right_ip"], serde_json::json!(3.5));
        assert_eq!(v["signal"]["count"], serde_json::json!(7));
    }

    #[test]
    fn csv_has_a_header_and_one_row_per_peak() {
        let out = run(SAW, "maxima", "csv");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "kind,index,value,prominence,width,width_height,left_base,right_base,left_ip,right_ip,plateau_size"
        );
        assert_eq!(lines.len(), 4, "{out}");
        assert_eq!(lines[2], "maximum,3,3,3,1,1.5,2,4,2.5,3.5,1");
    }

    #[test]
    fn separator_choices_split_as_advertised() {
        for (sep, input) in [
            ("comma", "0,1,0"),
            ("newline", "0\n1\n0"),
            ("space", "0 1 0"),
            ("semicolon", "0;1;0"),
            ("tab", "0\t1\t0"),
            ("pipe", "0|1|0"),
            ("auto", "0, 1 ; 0"),
        ] {
            let out = run_with_options(
                input, "maxima", sep, 0, "", "", 0.0, 0, 0.0, 0.0, 0.5, 0, "position", "text",
            )
            .unwrap();
            assert!(out.starts_with("1 maximum found in 3 values."), "{sep}: {out}");
        }
    }

    #[test]
    fn non_numeric_tokens_are_skipped_and_counted() {
        let out = text("value\n0\n1\n0\nn/a");
        assert!(out.starts_with("1 maximum found in 3 values."), "{out}");
        assert!(out.contains("Ignored non-numeric tokens: 2"), "{out}");
    }

    #[test]
    fn decimals_negatives_and_scientific_notation_parse() {
        let out = text("-1e1, -2.5, .5, -2.5, -1e1");
        assert!(out.starts_with("1 maximum found in 5 values."), "{out}");
        assert!(out.contains("Highest peak: index 2, value 0.5 (prominence 10.5)"), "{out}");
    }

    #[test]
    fn value_cap_is_enforced_at_the_boundary() {
        let at_cap: String = (0..MAX_VALUES)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let out = text(&at_cap);
        assert!(out.contains(&format!("Signal: {MAX_VALUES} values")), "{}", &out[..90]);

        let over = format!("{at_cap},1");
        let err = run_with_options(
            &over, "maxima", "auto", 0, "", "", 0.0, 0, 0.0, 0.0, 0.5, 0, "position", "text",
        )
        .unwrap_err();
        assert!(err.contains("analyses up to 20000 samples"), "{err}");
    }

    // ---- error paths ----

    #[test]
    fn empty_input_errors() {
        let err = run_with_options(
            "   ", "maxima", "auto", 0, "", "", 0.0, 0, 0.0, 0.0, 0.5, 0, "position", "text",
        )
        .unwrap_err();
        assert!(err.contains("no numbers found"), "{err}");
    }

    #[test]
    fn unknown_enum_value_errors_and_lists_the_choices() {
        let err = run_with_options(
            "0 1 0", "sideways", "auto", 0, "", "", 0.0, 0, 0.0, 0.0, 0.5, 0, "position", "text",
        )
        .unwrap_err();
        assert!(
            err.contains("unknown mode 'sideways' — expected one of: maxima, minima, both"),
            "{err}"
        );
    }

    #[test]
    fn even_smooth_window_errors_with_a_suggestion() {
        let err = run_with_options(
            "0 1 0", "maxima", "auto", 4, "", "", 0.0, 0, 0.0, 0.0, 0.5, 0, "position", "text",
        )
        .unwrap_err();
        assert!(err.contains("smooth must be an odd window"), "{err}");
        assert!(err.contains("try 3 or 5"), "{err}");
    }

    #[test]
    fn rel_height_out_of_range_errors() {
        let err = run_with_options(
            "0 1 0", "maxima", "auto", 0, "", "", 0.0, 0, 0.0, 0.0, 1.5, 0, "position", "text",
        )
        .unwrap_err();
        assert!(err.contains("rel_height must be between 0 and 1"), "{err}");
    }

    #[test]
    fn negative_prominence_errors() {
        let err = run_with_options(
            "0 1 0", "maxima", "auto", 0, "", "", 0.0, 0, -1.0, 0.0, 0.5, 0, "position", "text",
        )
        .unwrap_err();
        assert!(err.contains("min_prominence must be a finite number of 0 or more"), "{err}");
    }

    #[test]
    fn inverted_value_band_errors() {
        let err = run_with_options(
            "0 1 0", "maxima", "auto", 0, "9", "1", 0.0, 0, 0.0, 0.0, 0.5, 0, "position", "text",
        )
        .unwrap_err();
        assert!(err.contains("is above max_value"), "{err}");
    }

    #[test]
    fn non_numeric_bound_errors() {
        let err = run_with_options(
            "0 1 0", "maxima", "auto", 0, "high", "", 0.0, 0, 0.0, 0.0, 0.5, 0, "position", "text",
        )
        .unwrap_err();
        assert!(err.contains("min_value must be a number"), "{err}");
    }

    #[test]
    fn smooth_and_max_peaks_caps_error() {
        let err = run_with_options(
            "0 1 0", "maxima", "auto", 503, "", "", 0.0, 0, 0.0, 0.0, 0.5, 0, "position", "text",
        )
        .unwrap_err();
        assert!(err.contains("smooth must be between 0 and 501"), "{err}");

        let err = run_with_options(
            "0 1 0", "maxima", "auto", 0, "", "", 0.0, 0, 0.0, 0.0, 0.5, 10_001, "position", "text",
        )
        .unwrap_err();
        assert!(err.contains("max_peaks must be between 0 and 10000"), "{err}");
    }
}
