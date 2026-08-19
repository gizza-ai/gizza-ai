//! tempo-map-extractor core — turn a list of beat times into a tempo map:
//! the BPM-versus-time curve of a performance, plus its statistics and
//! DAW-ready exports. Pure Rust, no wafer/wasm-bindgen deps, so the chat
//! block, the CLI and the browser page all share this one implementation.
//!
//! The input is the artefact every beat tracker, DAW marker export and tap
//! session already produces: a column of beat timestamps. Each consecutive
//! pair of beats defines one instantaneous tempo, so N beats yield N-1 tempo
//! readings — that series *is* the tempo map.

use serde_json::{json, Value as Json};

/// Hard cap on parsed beat times per run.
pub const MAX_BEATS: usize = 20_000;
/// Hard cap on emitted rows (relevant when `grid_seconds` resamples the curve).
pub const MAX_ROWS: usize = 100_000;

/// Every knob the tool exposes. Strings stay strings so the CLI, chat and page
/// can pass raw user input and get one shared set of error messages.
pub struct Spec<'a> {
    pub beats: &'a str,
    pub time_unit: &'a str,
    pub fps: f64,
    pub beat_unit: &'a str,
    pub smoothing: usize,
    pub smooth_method: &'a str,
    pub grid_seconds: f64,
    pub min_interval_ms: f64,
    pub offset_seconds: f64,
    pub decimals: usize,
    pub output: &'a str,
    pub ppq: usize,
}

impl Default for Spec<'_> {
    fn default() -> Self {
        Spec {
            beats: "",
            time_unit: "auto",
            fps: 30.0,
            beat_unit: "quarter",
            smoothing: 1,
            smooth_method: "mean",
            grid_seconds: 0.0,
            min_interval_ms: 0.0,
            offset_seconds: 0.0,
            decimals: 2,
            output: "csv",
            ppq: 960,
        }
    }
}

/// One row of the tempo map.
struct Row {
    /// 1-based index of the beat the reading starts on.
    beat: usize,
    /// Time of the reading, in seconds.
    time: f64,
    /// Length of the beat interval this reading came from, in milliseconds.
    interval_ms: f64,
    /// Tempo at this point, in quarter-note BPM.
    bpm: f64,
}

/// Summary statistics of the whole curve (always computed per beat, so they do
/// not change when `grid_seconds` resamples the emitted rows).
struct Stats {
    beats: usize,
    intervals: usize,
    first_time: f64,
    last_time: f64,
    span: f64,
    mean: f64,
    median: f64,
    min: f64,
    max: f64,
    drift: f64,
    stdev: f64,
    overall: f64,
    jitter_ms: f64,
    slope: f64,
}

/// Extract the tempo map described by `spec`.
pub fn extract(spec: &Spec) -> Result<String, String> {
    let unit = spec.time_unit.trim();
    let unit = if unit.is_empty() { "auto" } else { unit };
    if !matches!(unit, "auto" | "seconds" | "milliseconds") {
        return Err(format!(
            "unknown time_unit '{unit}' (expected auto, seconds or milliseconds)"
        ));
    }
    if !spec.fps.is_finite() || spec.fps < 1.0 || spec.fps > 240.0 {
        return Err(format!(
            "fps must be between 1 and 240 (got {})",
            trim_num(spec.fps)
        ));
    }
    let qpb = quarters_per_beat(spec.beat_unit)?;
    let method = spec.smooth_method.trim();
    let method = if method.is_empty() { "mean" } else { method };
    if !matches!(method, "mean" | "median") {
        return Err(format!(
            "unknown smooth_method '{method}' (expected mean or median)"
        ));
    }
    if spec.smoothing < 1 || spec.smoothing > 64 {
        return Err(format!(
            "smoothing must be between 1 and 64 beats (got {})",
            spec.smoothing
        ));
    }
    if spec.decimals > 4 {
        return Err(format!(
            "decimals must be between 0 and 4 (got {})",
            spec.decimals
        ));
    }
    if !spec.grid_seconds.is_finite() || spec.grid_seconds < 0.0 || spec.grid_seconds > 3600.0 {
        return Err(format!(
            "grid_seconds must be between 0 and 3600 (got {})",
            trim_num(spec.grid_seconds)
        ));
    }
    if !spec.min_interval_ms.is_finite() || spec.min_interval_ms < 0.0 || spec.min_interval_ms > 10_000.0
    {
        return Err(format!(
            "min_interval_ms must be between 0 and 10000 (got {})",
            trim_num(spec.min_interval_ms)
        ));
    }
    if !spec.offset_seconds.is_finite() {
        return Err("offset_seconds must be a finite number".into());
    }
    if spec.ppq < 24 || spec.ppq > 15_360 {
        return Err(format!(
            "ppq must be between 24 and 15360 ticks per quarter note (got {})",
            spec.ppq
        ));
    }
    let output = spec.output.trim();
    let output = if output.is_empty() { "csv" } else { output };
    if !matches!(
        output,
        "csv" | "tsv" | "json" | "table" | "audacity" | "midi" | "summary"
    ) {
        return Err(format!(
            "unknown output '{output}' (expected csv, tsv, json, table, audacity, midi or summary)"
        ));
    }
    if output == "midi" && spec.grid_seconds > 0.0 {
        return Err(
            "grid_seconds cannot be combined with output=midi: MIDI tempo events must land on real beats, not on an even time grid. Set grid_seconds to 0 for the MIDI tempo map."
                .into(),
        );
    }

    let times = parse_beats(spec.beats, unit, spec.fps, spec.offset_seconds, spec.min_interval_ms)?;

    // N beats -> N-1 intervals -> N-1 tempo readings.
    let raw: Vec<f64> = times
        .windows(2)
        .map(|w| 60.0 / (w[1] - w[0]) * qpb)
        .collect();
    let smoothed = smooth(&raw, spec.smoothing, method);
    let stats = stats(&times, &raw, &smoothed, qpb);

    let rows = if spec.grid_seconds > 0.0 {
        grid_rows(&times, &smoothed, spec.grid_seconds)?
    } else {
        beat_rows(&times, &smoothed)
    };

    Ok(match output {
        "csv" => delimited(&rows, ',', spec.decimals),
        "tsv" => delimited(&rows, '\t', spec.decimals),
        "json" => render_json(&rows, &stats, spec, qpb),
        "table" => render_table(&rows, &stats, spec.decimals),
        "audacity" => render_audacity(&rows, &times, spec.grid_seconds, spec.decimals),
        "midi" => render_midi(&rows, qpb, spec.ppq, spec.decimals),
        _ => render_summary(&stats, spec.decimals),
    })
}

// ---------------------------------------------------------------- parsing

/// Quarter notes spanned by one tapped/marked beat.
fn quarters_per_beat(unit: &str) -> Result<f64, String> {
    let u = unit.trim();
    let u = if u.is_empty() { "quarter" } else { u };
    Ok(match u {
        "whole" => 4.0,
        "dotted-half" => 3.0,
        "half" => 2.0,
        "dotted-quarter" => 1.5,
        "quarter" => 1.0,
        "dotted-eighth" => 0.75,
        "eighth" => 0.5,
        "triplet-eighth" => 1.0 / 3.0,
        "sixteenth" => 0.25,
        other => {
            return Err(format!(
                "unknown beat_unit '{other}' (expected whole, dotted-half, half, dotted-quarter, quarter, dotted-eighth, eighth, triplet-eighth or sixteenth)"
            ))
        }
    })
}

/// Split the pasted input into beat times in seconds, apply the offset, drop
/// beats closer together than `min_interval_ms`, and validate the result.
fn parse_beats(
    input: &str,
    unit: &str,
    fps: f64,
    offset: f64,
    min_interval_ms: f64,
) -> Result<Vec<f64>, String> {
    // Strip comments and blank lines first.
    let lines: Vec<(usize, &str)> = input
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, strip_comment(l)))
        .filter(|(_, l)| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err("no beat times found: paste one beat timestamp per line (or a single comma-separated line)".into());
    }

    // One content line => it is a list of times. Many lines => the first
    // field of each line is the time (so label tracks and CSVs just work).
    let tokens: Vec<(usize, &str)> = if lines.len() == 1 {
        let (ln, line) = lines[0];
        split_fields(line).into_iter().map(|t| (ln, t)).collect()
    } else {
        lines
            .iter()
            .filter_map(|(ln, line)| split_fields(line).into_iter().next().map(|t| (*ln, t)))
            .collect()
    };

    let mut raw: Vec<f64> = Vec::with_capacity(tokens.len());
    for (idx, (line_no, tok)) in tokens.iter().enumerate() {
        match parse_time(tok, unit, fps) {
            Ok(v) => raw.push(v),
            Err(e) => {
                // Tolerate a single header row ("time,bpm", "start\tend\tlabel").
                if idx == 0 && tokens.len() > 1 {
                    continue;
                }
                return Err(format!("line {line_no}: {e}"));
            }
        }
        if raw.len() > MAX_BEATS {
            return Err(format!(
                "too many beat times: this tool reads at most {MAX_BEATS} per run"
            ));
        }
    }
    if raw.is_empty() {
        return Err("no beat times found: every line was a comment, blank or a header".into());
    }

    let min_gap = min_interval_ms / 1000.0;
    let mut kept: Vec<f64> = Vec::with_capacity(raw.len());
    for (i, t) in raw.iter().enumerate() {
        let t = t + offset;
        match kept.last() {
            None => kept.push(t),
            Some(&prev) => {
                if t < prev {
                    return Err(format!(
                        "beat times must increase: beat {} is at {} s, before beat {} at {} s. Sort the list first.",
                        i + 1,
                        trim_num(t),
                        i,
                        trim_num(prev)
                    ));
                }
                if t - prev < min_gap {
                    continue; // double-tap / duplicate marker, filtered out
                }
                if t == prev {
                    return Err(format!(
                        "duplicate beat time {} s at beat {}. Remove it, or set min_interval_ms above 0 to drop beats that land too close together.",
                        trim_num(t),
                        i + 1
                    ));
                }
                kept.push(t);
            }
        }
    }

    if kept.len() < 2 {
        return Err(format!(
            "need at least 2 beat times to measure a tempo (got {} after filtering). One timestamp has no interval before or after it.",
            kept.len()
        ));
    }
    Ok(kept)
}

/// Drop `#` / `//` comments and surrounding whitespace.
fn strip_comment(line: &str) -> &str {
    let mut end = line.len();
    if let Some(p) = line.find('#') {
        end = end.min(p);
    }
    if let Some(p) = line.find("//") {
        end = end.min(p);
    }
    line[..end].trim()
}

/// Split a line into fields on tabs, commas, semicolons or runs of spaces.
fn split_fields(line: &str) -> Vec<&str> {
    line.split(|c: char| c == '\t' || c == ',' || c == ';' || c == ' ')
        .map(|t| t.trim().trim_matches('"'))
        .filter(|t| !t.is_empty())
        .collect()
}

/// Parse one timestamp token into seconds.
///
/// Accepts decimal seconds (`12.5`), an explicit unit suffix (`1500ms`, `12s`),
/// `m:ss(.mmm)`, `h:mm:ss(.mmm)` and `hh:mm:ss:ff` frame timecode.
fn parse_time(tok: &str, unit: &str, fps: f64) -> Result<f64, String> {
    let t = tok.trim();
    if t.is_empty() {
        return Err("empty timestamp".into());
    }
    if t.contains(':') {
        let parts: Vec<&str> = t.split(':').collect();
        let bad = || format!("cannot read timecode '{t}' (expected m:ss, h:mm:ss, or hh:mm:ss:ff)");
        let num = |s: &str| s.trim().parse::<f64>().map_err(|_| bad());
        return match parts.len() {
            2 => Ok(num(parts[0])? * 60.0 + num(parts[1])?),
            3 => Ok(num(parts[0])? * 3600.0 + num(parts[1])? * 60.0 + num(parts[2])?),
            4 => Ok(num(parts[0])? * 3600.0
                + num(parts[1])? * 60.0
                + num(parts[2])?
                + num(parts[3])? / fps),
            _ => Err(bad()),
        };
    }
    let lower = t.to_ascii_lowercase();
    let (body, scale) = if let Some(b) = lower.strip_suffix("ms") {
        (b, 0.001)
    } else if let Some(b) = lower.strip_suffix("sec") {
        (b, 1.0)
    } else if let Some(b) = lower.strip_suffix('s') {
        (b, 1.0)
    } else if unit == "milliseconds" {
        (lower.as_str(), 0.001)
    } else {
        (lower.as_str(), 1.0)
    };
    let v: f64 = body
        .trim()
        .parse()
        .map_err(|_| format!("cannot read '{t}' as a beat time (expected seconds like 1.75, a timecode like 0:01.750, or a value with a unit like 1750ms)"))?;
    if !v.is_finite() {
        return Err(format!("beat time '{t}' is not a finite number"));
    }
    Ok(v * scale)
}

// ---------------------------------------------------------------- curve

/// Centred moving mean/median over `window` consecutive tempo readings.
fn smooth(raw: &[f64], window: usize, method: &str) -> Vec<f64> {
    if window <= 1 || raw.is_empty() {
        return raw.to_vec();
    }
    let back = (window - 1) / 2;
    let fwd = window / 2;
    (0..raw.len())
        .map(|i| {
            let lo = i.saturating_sub(back);
            let hi = (i + fwd + 1).min(raw.len());
            let slice = &raw[lo..hi];
            if method == "median" {
                median(slice)
            } else {
                slice.iter().sum::<f64>() / slice.len() as f64
            }
        })
        .collect()
}

fn median(values: &[f64]) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// One row per beat interval, timed at the beat the interval starts on.
fn beat_rows(times: &[f64], bpm: &[f64]) -> Vec<Row> {
    bpm.iter()
        .enumerate()
        .map(|(i, &b)| Row {
            beat: i + 1,
            time: times[i],
            interval_ms: (times[i + 1] - times[i]) * 1000.0,
            bpm: b,
        })
        .collect()
}

/// The same curve sampled onto an even time grid (step-hold within each beat).
fn grid_rows(times: &[f64], bpm: &[f64], step: f64) -> Result<Vec<Row>, String> {
    let start = times[0];
    let end = times[times.len() - 1];
    let count = ((end - start) / step).floor() as i64 + 1;
    if count <= 0 {
        return Err(format!(
            "grid_seconds {} is longer than the {} s covered by the beat times",
            trim_num(step),
            trim_num(end - start)
        ));
    }
    if count as usize > MAX_ROWS {
        return Err(format!(
            "grid_seconds {} would emit {} rows over {} s; the limit is {} rows. Use a larger grid_seconds.",
            trim_num(step),
            count,
            trim_num(end - start),
            MAX_ROWS
        ));
    }
    let mut idx = 0usize;
    let mut rows = Vec::with_capacity(count as usize);
    for k in 0..count {
        let t = start + step * k as f64;
        while idx + 1 < bpm.len() && times[idx + 1] <= t {
            idx += 1;
        }
        rows.push(Row {
            beat: idx + 1,
            time: t,
            interval_ms: (times[idx + 1] - times[idx]) * 1000.0,
            bpm: bpm[idx],
        });
    }
    Ok(rows)
}

fn stats(times: &[f64], raw: &[f64], smoothed: &[f64], qpb: f64) -> Stats {
    let n = smoothed.len();
    let mean = smoothed.iter().sum::<f64>() / n as f64;
    let min = smoothed.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = smoothed.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let stdev = sample_stdev(smoothed, mean);
    let intervals_ms: Vec<f64> = times.windows(2).map(|w| (w[1] - w[0]) * 1000.0).collect();
    let imean = intervals_ms.iter().sum::<f64>() / intervals_ms.len() as f64;
    let span = times[times.len() - 1] - times[0];
    // Least-squares slope of BPM against time, reported per minute.
    let tmean = times[..n].iter().sum::<f64>() / n as f64;
    let mut num = 0.0;
    let mut den = 0.0;
    for (i, &b) in smoothed.iter().enumerate() {
        let dt = times[i] - tmean;
        num += dt * (b - mean);
        den += dt * dt;
    }
    let slope = if den > 0.0 { num / den * 60.0 } else { 0.0 };
    Stats {
        beats: times.len(),
        intervals: raw.len(),
        first_time: times[0],
        last_time: times[times.len() - 1],
        span,
        mean,
        median: median(smoothed),
        min,
        max,
        drift: max - min,
        stdev,
        overall: if span > 0.0 {
            raw.len() as f64 * qpb * 60.0 / span
        } else {
            0.0
        },
        jitter_ms: sample_stdev(&intervals_ms, imean),
        slope,
    }
}

fn sample_stdev(values: &[f64], mean: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let ss: f64 = values.iter().map(|v| (v - mean) * (v - mean)).sum();
    (ss / (values.len() - 1) as f64).sqrt()
}

/// Plain-language verdict on how much the tempo moves.
fn stability(drift: f64) -> &'static str {
    if drift < 1.0 {
        "rock steady"
    } else if drift < 3.0 {
        "steady"
    } else if drift < 8.0 {
        "slight drift"
    } else if drift < 20.0 {
        "variable"
    } else {
        "highly variable"
    }
}

/// Direction of travel across the whole take.
fn trend(slope: f64) -> &'static str {
    if slope > 0.5 {
        "speeding up"
    } else if slope < -0.5 {
        "slowing down"
    } else {
        "holding steady"
    }
}

/// Conventional Italian tempo marking for a BPM value.
fn tempo_family(bpm: f64) -> &'static str {
    if bpm < 40.0 {
        "Grave"
    } else if bpm < 60.0 {
        "Largo"
    } else if bpm < 76.0 {
        "Adagio"
    } else if bpm < 108.0 {
        "Andante"
    } else if bpm < 120.0 {
        "Moderato"
    } else if bpm < 156.0 {
        "Allegro"
    } else if bpm < 176.0 {
        "Vivace"
    } else {
        "Presto"
    }
}

// ---------------------------------------------------------------- output

fn fmt(v: f64, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, v);
    if s.starts_with("-") && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

/// Compact rendering used inside error messages.
fn trim_num(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() {
        "0".into()
    } else {
        s
    }
}

fn delimited(rows: &[Row], sep: char, decimals: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "time_seconds{sep}bpm{sep}beat{sep}interval_ms\n"
    ));
    for r in rows {
        out.push_str(&format!(
            "{}{sep}{}{sep}{}{sep}{}\n",
            fmt(r.time, 3),
            fmt(r.bpm, decimals),
            r.beat,
            fmt(r.interval_ms, 1)
        ));
    }
    out.trim_end().to_string()
}

fn render_json(rows: &[Row], s: &Stats, spec: &Spec, qpb: f64) -> String {
    let d = spec.decimals;
    let map: Vec<Json> = rows
        .iter()
        .map(|r| {
            json!({
                "time_seconds": round(r.time, 3),
                "bpm": round(r.bpm, d),
                "beat": r.beat,
                "interval_ms": round(r.interval_ms, 1),
            })
        })
        .collect();
    let v = json!({
        "beat_unit": spec.beat_unit.trim(),
        "quarters_per_beat": round(qpb, 4),
        "rows": map.len(),
        "sampling": if spec.grid_seconds > 0.0 { "grid" } else { "per-beat" },
        "summary": summary_json(s, d),
        "tempo_map": map,
    });
    serde_json::to_string_pretty(&v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn summary_json(s: &Stats, d: usize) -> Json {
    json!({
        "beats": s.beats,
        "intervals": s.intervals,
        "first_time_seconds": round(s.first_time, 3),
        "last_time_seconds": round(s.last_time, 3),
        "span_seconds": round(s.span, 3),
        "mean_bpm": round(s.mean, d),
        "median_bpm": round(s.median, d),
        "min_bpm": round(s.min, d),
        "max_bpm": round(s.max, d),
        "drift_bpm": round(s.drift, d),
        "stdev_bpm": round(s.stdev, d),
        "overall_bpm": round(s.overall, d),
        "jitter_ms": round(s.jitter_ms, 2),
        "slope_bpm_per_minute": round(s.slope, 2),
        "trend": trend(s.slope),
        "stability": stability(s.drift),
        "tempo_family": tempo_family(s.mean),
    })
}

fn round(v: f64, decimals: usize) -> f64 {
    let f = 10f64.powi(decimals as i32);
    let r = (v * f).round() / f;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn render_table(rows: &[Row], s: &Stats, decimals: usize) -> String {
    let mut cells: Vec<[String; 5]> = vec![[
        "beat".into(),
        "time_s".into(),
        "interval_ms".into(),
        "bpm".into(),
        "vs_mean".into(),
    ]];
    for r in rows {
        cells.push([
            r.beat.to_string(),
            fmt(r.time, 3),
            fmt(r.interval_ms, 1),
            fmt(r.bpm, decimals),
            format!("{}{}", if r.bpm - s.mean >= 0.0 { "+" } else { "" }, fmt(r.bpm - s.mean, decimals)),
        ]);
    }
    let mut width = [0usize; 5];
    for row in &cells {
        for (i, c) in row.iter().enumerate() {
            width[i] = width[i].max(c.chars().count());
        }
    }
    let mut out = String::new();
    for (ri, row) in cells.iter().enumerate() {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{:>w$}", c, w = width[i]))
            .collect();
        out.push_str(&line.join("  "));
        out.push('\n');
        if ri == 0 {
            let rule: Vec<String> = width.iter().map(|w| "-".repeat(*w)).collect();
            out.push_str(&rule.join("  "));
            out.push('\n');
        }
    }
    out.push('\n');
    out.push_str(&render_summary(s, decimals));
    out
}

fn render_summary(s: &Stats, d: usize) -> String {
    format!(
        "Beats: {}  ({} tempo readings over {} s)\n\
         Mean BPM: {}    Median: {}\n\
         Range: {} to {} BPM  (drift {} BPM)\n\
         Std dev: {} BPM    Interval jitter: {} ms\n\
         Overall average across the take: {} BPM\n\
         Tempo change: {} BPM per minute ({})\n\
         Stability: {}    Tempo marking: {}",
        s.beats,
        s.intervals,
        fmt(s.span, 3),
        fmt(s.mean, d),
        fmt(s.median, d),
        fmt(s.min, d),
        fmt(s.max, d),
        fmt(s.drift, d),
        fmt(s.stdev, d),
        fmt(s.jitter_ms, 2),
        fmt(s.overall, d),
        fmt(s.slope, 2),
        trend(s.slope),
        stability(s.drift),
        tempo_family(s.mean)
    )
}

/// Audacity-style label track: one label per reading, named with its BPM.
fn render_audacity(rows: &[Row], times: &[f64], grid: f64, decimals: usize) -> String {
    let last = times[times.len() - 1];
    let mut out = String::new();
    for (i, r) in rows.iter().enumerate() {
        let end = if grid > 0.0 {
            (r.time + grid).min(last)
        } else {
            times[i + 1]
        };
        out.push_str(&format!(
            "{}\t{}\t{} BPM\n",
            fmt(r.time, 6),
            fmt(end, 6),
            fmt(r.bpm, decimals)
        ));
    }
    out.trim_end().to_string()
}

/// Standard-MIDI-File tempo map: one tempo meta event per tempo change.
fn render_midi(rows: &[Row], qpb: f64, ppq: usize, decimals: usize) -> String {
    let mut out = String::from("tick,microseconds_per_quarter,bpm\n");
    let mut last = String::new();
    for r in rows {
        let bpm = fmt(r.bpm, decimals);
        if bpm == last {
            continue; // a tempo map only needs an event where the tempo changes
        }
        last = bpm.clone();
        let tick = ((r.beat - 1) as f64 * ppq as f64 * qpb).round() as i64;
        let us = (60_000_000.0 / r.bpm).round() as i64;
        out.push_str(&format!("{tick},{us},{bpm}\n"));
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec<'a>(beats: &'a str) -> Spec<'a> {
        Spec {
            beats,
            ..Default::default()
        }
    }

    #[test]
    fn steady_click_track_is_exactly_120_bpm() {
        let out = extract(&spec("0\n0.5\n1.0\n1.5")).unwrap();
        assert_eq!(
            out,
            "time_seconds,bpm,beat,interval_ms\n\
             0.000,120.00,1,500.0\n\
             0.500,120.00,2,500.0\n\
             1.000,120.00,3,500.0"
        );
    }

    #[test]
    fn ritardando_curve_slows_down() {
        // 0.5 s, 0.6 s, 0.75 s intervals -> 120, 100, 80 BPM.
        let out = extract(&Spec {
            decimals: 1,
            ..spec("0\n0.5\n1.1\n1.85")
        })
        .unwrap();
        assert_eq!(
            out,
            "time_seconds,bpm,beat,interval_ms\n\
             0.000,120.0,1,500.0\n\
             0.500,100.0,2,600.0\n\
             1.100,80.0,3,750.0"
        );
    }

    #[test]
    fn beat_unit_converts_tapped_pulse_to_quarter_bpm() {
        // Tapping half notes at 1 s apart is 120 quarter-note BPM.
        let half = extract(&Spec {
            beat_unit: "half",
            ..spec("0\n1\n2")
        })
        .unwrap();
        assert!(half.contains("0.000,120.00,1,1000.0"), "{half}");
        // Tapping eighths at 1 s apart is 30 quarter-note BPM.
        let eighth = extract(&Spec {
            beat_unit: "eighth",
            ..spec("0\n1\n2")
        })
        .unwrap();
        assert!(eighth.contains("0.000,30.00,1,1000.0"), "{eighth}");
    }

    #[test]
    fn timecode_and_millisecond_inputs_parse() {
        let tc = extract(&spec("0:00.000\n0:00.500\n0:01.000")).unwrap();
        assert!(tc.contains("120.00"), "{tc}");
        let frames = extract(&Spec {
            fps: 25.0,
            ..spec("00:00:00:00\n00:00:00:10\n00:00:00:20")
        })
        .unwrap();
        // 10 frames at 25 fps = 0.4 s = 150 BPM.
        assert!(frames.contains("150.00"), "{frames}");
        let ms = extract(&Spec {
            time_unit: "milliseconds",
            ..spec("0\n500\n1000")
        })
        .unwrap();
        assert!(ms.contains("120.00"), "{ms}");
    }

    #[test]
    fn label_track_columns_and_comments_are_tolerated() {
        let out = extract(&spec(
            "# beat markers\n0.000000\t0.500000\tbeat 1\n0.500000\t1.000000\tbeat 2\n1.000000\t1.500000\tbeat 3",
        ))
        .unwrap();
        assert!(out.contains("0.000,120.00,1,500.0"), "{out}");
    }

    #[test]
    fn single_line_list_is_split_into_beats() {
        let out = extract(&spec("0, 0.5, 1.0")).unwrap();
        assert_eq!(out.lines().count(), 3);
    }

    #[test]
    fn header_row_is_skipped() {
        let out = extract(&spec("time\n0\n0.5\n1.0")).unwrap();
        assert!(out.contains("120.00"), "{out}");
    }

    #[test]
    fn smoothing_averages_neighbouring_readings() {
        // Raw readings 120, 60, 120; a 3-wide centred mean pulls the middle up.
        let out = extract(&Spec {
            smoothing: 3,
            decimals: 1,
            ..spec("0\n0.5\n1.5\n2.0")
        })
        .unwrap();
        assert!(out.contains("0.500,100.0,2,1000.0"), "{out}");
        let med = extract(&Spec {
            smoothing: 3,
            smooth_method: "median",
            decimals: 1,
            ..spec("0\n0.5\n1.5\n2.0")
        })
        .unwrap();
        assert!(med.contains("0.500,120.0,2,1000.0"), "{med}");
    }

    #[test]
    fn min_interval_drops_double_taps() {
        let out = extract(&Spec {
            min_interval_ms: 100.0,
            ..spec("0\n0.02\n0.5\n1.0")
        })
        .unwrap();
        assert_eq!(out.lines().count(), 3); // header + 2 readings
        assert!(out.contains("120.00"), "{out}");
    }

    #[test]
    fn offset_shifts_every_time() {
        let out = extract(&Spec {
            offset_seconds: 10.0,
            ..spec("0\n0.5\n1.0")
        })
        .unwrap();
        assert!(out.contains("10.000,120.00,1,500.0"), "{out}");
    }

    #[test]
    fn grid_resamples_onto_an_even_time_axis() {
        let out = extract(&Spec {
            grid_seconds: 1.0,
            decimals: 1,
            ..spec("0\n0.5\n1.5\n2.5")
        })
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "time_seconds,bpm,beat,interval_ms");
        assert_eq!(lines[1], "0.000,120.0,1,500.0");
        assert_eq!(lines[2], "1.000,60.0,2,1000.0");
        assert_eq!(lines[3], "2.000,60.0,3,1000.0");
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn every_output_format_renders() {
        let beats = "0\n0.5\n1.0\n1.5";
        for out in ["csv", "tsv", "json", "table", "audacity", "midi", "summary"] {
            let s = extract(&Spec {
                output: out,
                ..spec(beats)
            })
            .unwrap_or_else(|e| panic!("{out}: {e}"));
            assert!(!s.trim().is_empty(), "{out} produced nothing");
        }
    }

    #[test]
    fn tsv_audacity_and_midi_shapes_are_exact() {
        let beats = "0\n0.5\n1.0";
        let tsv = extract(&Spec {
            output: "tsv",
            ..spec(beats)
        })
        .unwrap();
        assert_eq!(
            tsv,
            "time_seconds\tbpm\tbeat\tinterval_ms\n0.000\t120.00\t1\t500.0\n0.500\t120.00\t2\t500.0"
        );
        let lab = extract(&Spec {
            output: "audacity",
            ..spec(beats)
        })
        .unwrap();
        assert_eq!(
            lab,
            "0.000000\t0.500000\t120.00 BPM\n0.500000\t1.000000\t120.00 BPM"
        );
        // A steady tempo needs exactly one MIDI tempo event.
        let midi = extract(&Spec {
            output: "midi",
            ..spec(beats)
        })
        .unwrap();
        assert_eq!(midi, "tick,microseconds_per_quarter,bpm\n0,500000,120.00");
    }

    #[test]
    fn midi_ticks_follow_ppq_and_beat_unit() {
        let midi = extract(&Spec {
            output: "midi",
            ppq: 480,
            decimals: 0,
            ..spec("0\n0.5\n1.1\n1.7")
        })
        .unwrap();
        let lines: Vec<&str> = midi.lines().collect();
        assert_eq!(lines[1], "0,500000,120");
        assert_eq!(lines[2], "480,600000,100");
    }

    #[test]
    fn json_carries_the_summary_object() {
        let out = extract(&Spec {
            output: "json",
            ..spec("0\n0.5\n1.0\n1.6")
        })
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["beats"], 4);
        assert_eq!(v["summary"]["intervals"], 3);
        assert_eq!(v["summary"]["max_bpm"], 120.0);
        assert_eq!(v["summary"]["min_bpm"], 100.0);
        assert_eq!(v["summary"]["stability"], "highly variable");
        assert_eq!(v["summary"]["trend"], "slowing down");
        assert_eq!(v["tempo_map"][0]["bpm"], 120.0);
        assert_eq!(v["sampling"], "per-beat");
    }

    #[test]
    fn summary_reports_family_and_overall_tempo() {
        let out = extract(&Spec {
            output: "summary",
            ..spec("0\n0.5\n1.0\n1.5\n2.0")
        })
        .unwrap();
        assert!(out.contains("Mean BPM: 120.00"), "{out}");
        assert!(out.contains("Overall average across the take: 120.00 BPM"), "{out}");
        assert!(out.contains("Stability: rock steady"), "{out}");
        assert!(out.contains("Tempo marking: Allegro"), "{out}");
    }

    #[test]
    fn every_beat_unit_scales_the_reading() {
        // One beat per second, so quarter-note BPM == 60 * quarters-per-beat.
        for (unit, expected) in [
            ("whole", "240.0"),
            ("dotted-half", "180.0"),
            ("half", "120.0"),
            ("dotted-quarter", "90.0"),
            ("quarter", "60.0"),
            ("dotted-eighth", "45.0"),
            ("eighth", "30.0"),
            ("triplet-eighth", "20.0"),
            ("sixteenth", "15.0"),
        ] {
            let out = extract(&Spec {
                beat_unit: unit,
                decimals: 1,
                ..spec("0\n1\n2")
            })
            .unwrap();
            assert!(
                out.contains(&format!("0.000,{expected},1,1000.0")),
                "{unit}: {out}"
            );
        }
    }

    #[test]
    fn cap_boundary_is_enforced() {
        let ok: String = (0..MAX_BEATS)
            .map(|i| format!("{}\n", i as f64 * 0.5))
            .collect();
        assert!(extract(&spec(&ok)).is_ok());
        let over: String = (0..MAX_BEATS + 1)
            .map(|i| format!("{}\n", i as f64 * 0.5))
            .collect();
        let err = extract(&spec(&over)).unwrap_err();
        assert!(err.contains("too many beat times"), "{err}");
    }

    #[test]
    fn errors_explain_what_was_expected() {
        assert!(extract(&spec("")).unwrap_err().contains("no beat times"));
        assert!(extract(&spec("1.0")).unwrap_err().contains("at least 2"));
        assert!(extract(&spec("0\n1\n0.5"))
            .unwrap_err()
            .contains("must increase"));
        assert!(extract(&spec("0\n0.5\n0.5"))
            .unwrap_err()
            .contains("duplicate beat time"));
        assert!(extract(&spec("0\nbanana\n1"))
            .unwrap_err()
            .contains("cannot read"));
        assert!(extract(&Spec {
            beat_unit: "crotchet",
            ..spec("0\n1")
        })
        .unwrap_err()
        .contains("unknown beat_unit"));
        assert!(extract(&Spec {
            output: "xml",
            ..spec("0\n1")
        })
        .unwrap_err()
        .contains("unknown output"));
        assert!(extract(&Spec {
            output: "midi",
            grid_seconds: 1.0,
            ..spec("0\n1")
        })
        .unwrap_err()
        .contains("cannot be combined"));
        assert!(extract(&Spec {
            smoothing: 0,
            ..spec("0\n1")
        })
        .unwrap_err()
        .contains("smoothing must be"));
        assert!(extract(&Spec {
            fps: 0.0,
            ..spec("0\n1")
        })
        .unwrap_err()
        .contains("fps must be"));
        assert!(extract(&Spec {
            ppq: 12,
            ..spec("0\n1")
        })
        .unwrap_err()
        .contains("ppq must be"));
    }
}
