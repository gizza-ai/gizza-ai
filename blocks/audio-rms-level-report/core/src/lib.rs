//! audio-rms-level-report core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Measures the **level** of one audio file and reports a per-channel summary:
//! RMS level, sample peak, average (mean absolute) level — each in both dBFS and
//! linear full-scale amplitude — plus crest factor, headroom, DC offset,
//! short-window RMS peak/trough, zero crossings, and clipped-sample facts. An
//! `overall` row aggregates every channel together.
//!
//! Pipeline: decode the pasted file bytes (base64/hex) → symphonia demuxes the
//! container and decodes the FIRST decodable audio track (video tracks are
//! `CODEC_TYPE_NULL` and skipped) → every frame is folded into fixed-size
//! per-channel accumulators → render JSON, a text report, or CSV.
//!
//! The decoded signal is NEVER buffered: each sample updates running sums, so
//! memory is O(channels) regardless of clip length. This is a whole-file
//! *summary*, not a time series — for windowed levels over time use the
//! video-audio-rms-timeline tool.
//!
//! Pure Rust → runs on every backend, including the chat Service Worker.

use std::io::Cursor;

use symphonia::core::audio::{Channels, SampleBuffer};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Level of digital silence in dBFS. A linear level of 0 is `-inf` dB, which is
/// not JSON/CSV friendly, so it is floored to this value.
pub const SILENCE_FLOOR_DBFS: f64 = -120.0;

/// Hard cap on decoded input bytes (64 MiB). Pasting more than this as
/// base64/hex is rejected before any decoding work happens.
pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;

/// Hard cap on analyzed frames per channel (~10 min at 48 kHz). Longer audio is
/// truncated to keep decode time bounded; the truncation is reported as a fact.
pub const MAX_FRAMES: u64 = 30_000_000;

/// Largest channel count the report supports.
pub const MAX_CHANNELS: usize = 16;

/// Short-window length bounds, in milliseconds, for the RMS peak/trough figures.
pub const MIN_WINDOW_MS: f64 = 1.0;
pub const MAX_WINDOW_MS: f64 = 10_000.0;

/// Clip-detection threshold bounds (absolute full-scale amplitude).
pub const MIN_CLIP_THRESHOLD: f64 = 0.5;
pub const MAX_CLIP_THRESHOLD: f64 = 1.0;

const SUPPORTED: &str = "supported containers: WAV, AIFF, CAF, FLAC, MP3, OGG, MP4/MOV/M4A, \
                         MKV/WebM, AAC-ADTS; audio codecs: PCM, ADPCM, FLAC, MP3, AAC-LC, ALAC, \
                         Vorbis (Opus, AC-3 and DTS are not supported)";

/// Output rendering.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OutMode {
    Json,
    Report,
    Csv,
}

/// Running level accumulators for ONE channel. Fixed size — samples are folded
/// in one at a time and never stored.
#[derive(Clone)]
struct Acc {
    label: String,
    n: u64,
    sum_sq: f64,
    sum_abs: f64,
    sum_signed: f64,
    peak: f64,
    clipped: u64,
    clip_run: u64,
    longest_clip_run: u64,
    zero_crossings: u64,
    prev_positive: Option<bool>,
    // Short-window RMS tracking (astats-style peak/trough of the window RMS).
    win_n: u64,
    win_sum_sq: f64,
    win_rms_max: Option<f64>,
    win_rms_min: Option<f64>,
}

impl Acc {
    fn new(label: String) -> Self {
        Self {
            label,
            n: 0,
            sum_sq: 0.0,
            sum_abs: 0.0,
            sum_signed: 0.0,
            peak: 0.0,
            clipped: 0,
            clip_run: 0,
            longest_clip_run: 0,
            zero_crossings: 0,
            prev_positive: None,
            win_n: 0,
            win_sum_sq: 0.0,
            win_rms_max: None,
            win_rms_min: None,
        }
    }

    /// Fold one sample (full-scale amplitude, nominally -1..1) into the sums.
    fn push(&mut self, v: f64, clip_threshold: f64, win_len: u64) {
        let a = v.abs();
        self.n += 1;
        self.sum_sq += v * v;
        self.sum_abs += a;
        self.sum_signed += v;
        if a > self.peak {
            self.peak = a;
        }
        if a >= clip_threshold {
            self.clipped += 1;
            self.clip_run += 1;
            if self.clip_run > self.longest_clip_run {
                self.longest_clip_run = self.clip_run;
            }
        } else {
            self.clip_run = 0;
        }
        // Zero crossings: a sign change between consecutive non-zero samples.
        if v != 0.0 {
            let positive = v > 0.0;
            if let Some(prev) = self.prev_positive {
                if prev != positive {
                    self.zero_crossings += 1;
                }
            }
            self.prev_positive = Some(positive);
        }
        self.win_sum_sq += v * v;
        self.win_n += 1;
        if self.win_n >= win_len {
            self.close_window();
        }
    }

    fn close_window(&mut self) {
        if self.win_n == 0 {
            return;
        }
        let rms = (self.win_sum_sq / self.win_n as f64).sqrt();
        self.win_rms_max = Some(self.win_rms_max.map_or(rms, |m: f64| m.max(rms)));
        self.win_rms_min = Some(self.win_rms_min.map_or(rms, |m: f64| m.min(rms)));
        self.win_n = 0;
        self.win_sum_sq = 0.0;
    }

    /// Fold a trailing partial window in only when no full window ever closed
    /// (clip shorter than the window) — otherwise a short tail would drag the
    /// trough figure down and misreport the quietest passage.
    fn finish(&mut self) {
        if self.win_rms_max.is_none() {
            self.close_window();
        }
    }

    fn stats(&self) -> Stats {
        let n = self.n.max(1) as f64;
        let rms = (self.sum_sq / n).sqrt();
        let avg = self.sum_abs / n;
        Stats {
            label: self.label.clone(),
            samples: self.n,
            rms,
            peak: self.peak,
            avg,
            dc_offset: self.sum_signed / n,
            rms_peak: self.win_rms_max,
            rms_trough: self.win_rms_min,
            clipped: self.clipped,
            longest_clip_run: self.longest_clip_run,
            zero_crossings: self.zero_crossings,
        }
    }
}

/// One finished channel's (or the overall) level figures, in linear units.
struct Stats {
    label: String,
    samples: u64,
    rms: f64,
    peak: f64,
    avg: f64,
    dc_offset: f64,
    rms_peak: Option<f64>,
    rms_trough: Option<f64>,
    clipped: u64,
    longest_clip_run: u64,
    zero_crossings: u64,
}

impl Stats {
    fn clipped_pct(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.clipped as f64 * 100.0 / self.samples as f64
        }
    }
    /// Peak-to-RMS ratio in dB. Undefined for digital silence → 0.
    fn crest_db(&self) -> f64 {
        if self.rms <= 0.0 || self.peak <= 0.0 {
            0.0
        } else {
            dbfs(self.peak) - dbfs(self.rms)
        }
    }
    /// Distance from the sample peak up to full scale (never negative).
    fn headroom_db(&self) -> f64 {
        (-dbfs(self.peak)).max(0.0)
    }
}

/// Decoded stream facts + the per-channel accumulators.
struct Analysis {
    sample_rate: u32,
    channels: Vec<Acc>,
    frames: u64,
    truncated: bool,
}

/// Analyze one audio file and render a per-channel level summary.
///
/// - `input`: the audio file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `output`: `"json"` (default), `"report"` (aligned text table) or `"csv"`.
/// - `rms_window_ms`: short-window length in ms for the RMS peak/trough figures
///   (1–10000, default 50; blank/0 → 50).
/// - `clip_threshold`: absolute amplitude counted as clipped (0.5–1.0, default
///   0.99; blank/0 → 0.99).
///
/// Returns a user-facing error string for empty/oversized input, undecodable
/// bytes, an unsupported container/codec, a file with no audio track, or
/// out-of-range parameters.
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    rms_window_ms: f64,
    clip_threshold: f64,
) -> Result<String, String> {
    let out_mode = match output.trim() {
        "" | "json" => OutMode::Json,
        "report" => OutMode::Report,
        "csv" => OutMode::Csv,
        other => {
            return Err(format!(
                "invalid output {other:?}: expected \"json\", \"report\" or \"csv\""
            ))
        }
    };
    // The page/CLI send 0 for a blank number field — treat that as "default".
    let window_ms = if rms_window_ms == 0.0 { 50.0 } else { rms_window_ms };
    if !window_ms.is_finite() || !(MIN_WINDOW_MS..=MAX_WINDOW_MS).contains(&window_ms) {
        return Err(format!(
            "invalid rms_window_ms {rms_window_ms}: expected {MIN_WINDOW_MS}-{MAX_WINDOW_MS} ms"
        ));
    }
    let clip_threshold = if clip_threshold == 0.0 { 0.99 } else { clip_threshold };
    if !clip_threshold.is_finite()
        || !(MIN_CLIP_THRESHOLD..=MAX_CLIP_THRESHOLD).contains(&clip_threshold)
    {
        return Err(format!(
            "invalid clip_threshold {clip_threshold}: expected \
             {MIN_CLIP_THRESHOLD}-{MAX_CLIP_THRESHOLD} (absolute full-scale amplitude)"
        ));
    }

    let bytes = decode_bytes(input, input_format)?;
    let analysis = analyze(bytes, window_ms, clip_threshold)?;

    let per_channel: Vec<Stats> = analysis.channels.iter().map(|c| c.stats()).collect();
    let overall = overall_stats(&analysis.channels);

    Ok(match out_mode {
        OutMode::Json => render_json(&analysis, &per_channel, &overall, window_ms, clip_threshold),
        OutMode::Report => {
            render_report(&analysis, &per_channel, &overall, window_ms, clip_threshold)
        }
        OutMode::Csv => render_csv(&per_channel, &overall),
    })
}

/// Aggregate every channel into one "overall" row (astats' `Overall`): the RMS
/// of all samples, the loudest sample anywhere, totals for the counts.
fn overall_stats(channels: &[Acc]) -> Stats {
    let mut all = Stats {
        label: "overall".into(),
        samples: 0,
        rms: 0.0,
        peak: 0.0,
        avg: 0.0,
        dc_offset: 0.0,
        rms_peak: None,
        rms_trough: None,
        clipped: 0,
        longest_clip_run: 0,
        zero_crossings: 0,
    };
    let mut sum_sq = 0.0;
    let mut sum_abs = 0.0;
    let mut sum_signed = 0.0;
    for c in channels {
        all.samples += c.n;
        sum_sq += c.sum_sq;
        sum_abs += c.sum_abs;
        sum_signed += c.sum_signed;
        all.peak = all.peak.max(c.peak);
        all.clipped += c.clipped;
        all.longest_clip_run = all.longest_clip_run.max(c.longest_clip_run);
        all.zero_crossings += c.zero_crossings;
        if let Some(v) = c.win_rms_max {
            all.rms_peak = Some(all.rms_peak.map_or(v, |m: f64| m.max(v)));
        }
        if let Some(v) = c.win_rms_min {
            all.rms_trough = Some(all.rms_trough.map_or(v, |m: f64| m.min(v)));
        }
    }
    let n = all.samples.max(1) as f64;
    all.rms = (sum_sq / n).sqrt();
    all.avg = sum_abs / n;
    all.dc_offset = sum_signed / n;
    all
}

// ---------------------------------------------------------------------------
// Units + number formatting
// ---------------------------------------------------------------------------

/// Linear full-scale amplitude → dBFS, floored at [`SILENCE_FLOOR_DBFS`].
fn dbfs(linear: f64) -> f64 {
    if linear <= 0.0 {
        SILENCE_FLOOR_DBFS
    } else {
        (20.0 * linear.log10()).max(SILENCE_FLOOR_DBFS)
    }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
fn round6(v: f64) -> f64 {
    (v * 1_000_000.0).round() / 1_000_000.0
}
/// dBFS value, rounded for output.
fn db(linear: f64) -> f64 {
    round3(dbfs(linear))
}

/// Trim trailing zeros so the JSON/CSV stays compact but exact (`-6.021`, `0`).
fn num(v: f64) -> String {
    let v = if v == 0.0 { 0.0 } else { v }; // normalize -0
    let mut s = format!("{v}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

/// `Some(dbfs)` rendered as a number, `None` (no window closed) as `null`.
fn opt_db(v: Option<f64>) -> String {
    match v {
        Some(x) => num(db(x)),
        None => "null".into(),
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_json(
    a: &Analysis,
    per_channel: &[Stats],
    overall: &Stats,
    window_ms: f64,
    clip_threshold: f64,
) -> String {
    let layout: Vec<String> = a.channels.iter().map(|c| format!("{:?}", c.label)).collect();
    let mut out = String::with_capacity(512 + per_channel.len() * 400);
    out.push_str("{\n");
    out.push_str(&format!("  \"sample_rate\": {},\n", a.sample_rate));
    out.push_str(&format!("  \"channels\": {},\n", a.channels.len()));
    out.push_str(&format!("  \"channel_layout\": [{}],\n", layout.join(", ")));
    out.push_str(&format!(
        "  \"duration_s\": {},\n",
        num(round3(a.frames as f64 / a.sample_rate as f64))
    ));
    out.push_str(&format!("  \"frames\": {},\n", a.frames));
    out.push_str(&format!("  \"total_samples\": {},\n", overall.samples));
    out.push_str(&format!("  \"rms_window_ms\": {},\n", num(window_ms)));
    out.push_str(&format!(
        "  \"clip_threshold\": {},\n",
        num(round6(clip_threshold))
    ));
    out.push_str(&format!("  \"truncated\": {},\n", a.truncated));
    out.push_str("  \"per_channel\": [\n");
    for (i, s) in per_channel.iter().enumerate() {
        let comma = if i + 1 < per_channel.len() { "," } else { "" };
        out.push_str(&format!(
            "    {{\"channel\": {}, \"label\": {:?}, {}}}{comma}\n",
            i + 1,
            s.label,
            stats_fields(s)
        ));
    }
    out.push_str("  ],\n");
    out.push_str(&format!("  \"overall\": {{{}}}\n", stats_fields(overall)));
    out.push('}');
    out
}

/// The shared measurement fields of one JSON stats object.
fn stats_fields(s: &Stats) -> String {
    format!(
        "\"samples\": {}, \"rms_dbfs\": {}, \"rms_linear\": {}, \"peak_dbfs\": {}, \
         \"peak_linear\": {}, \"average_dbfs\": {}, \"average_linear\": {}, \
         \"rms_peak_dbfs\": {}, \"rms_trough_dbfs\": {}, \"crest_factor_db\": {}, \
         \"headroom_db\": {}, \"dc_offset\": {}, \"clipped_samples\": {}, \
         \"clipped_pct\": {}, \"longest_clipped_run\": {}, \"zero_crossings\": {}",
        s.samples,
        num(db(s.rms)),
        num(round6(s.rms)),
        num(db(s.peak)),
        num(round6(s.peak)),
        num(db(s.avg)),
        num(round6(s.avg)),
        opt_db(s.rms_peak),
        opt_db(s.rms_trough),
        num(round3(s.crest_db())),
        num(round3(s.headroom_db())),
        num(round6(s.dc_offset)),
        s.clipped,
        num(round3(s.clipped_pct())),
        s.longest_clip_run,
        s.zero_crossings,
    )
}

fn render_report(
    a: &Analysis,
    per_channel: &[Stats],
    overall: &Stats,
    window_ms: f64,
    clip_threshold: f64,
) -> String {
    let layout: Vec<&str> = a.channels.iter().map(|c| c.label.as_str()).collect();
    let mut out = String::new();
    out.push_str("Audio level report\n");
    out.push_str("==================\n");
    out.push_str(&format!(
        "Format      : {} Hz, {} channel(s) ({}), {} s, {} frames\n",
        a.sample_rate,
        a.channels.len(),
        layout.join(" / "),
        num(round3(a.frames as f64 / a.sample_rate as f64)),
        a.frames,
    ));
    out.push_str(&format!(
        "Settings    : RMS window {} ms, clip threshold |sample| >= {}\n",
        num(window_ms),
        num(round6(clip_threshold))
    ));
    if a.truncated {
        out.push_str(&format!(
            "Note        : analysis truncated at {MAX_FRAMES} frames per channel\n"
        ));
    }
    out.push('\n');
    out.push_str(
        "Channel      RMS dBFS   Peak dBFS    Avg dBFS    Crest dB    Headroom     Clipped\n",
    );
    out.push_str(
        "-------------------------------------------------------------------------------\n",
    );
    for s in per_channel.iter().chain(std::iter::once(overall)) {
        out.push_str(&format!(
            "{:<10} {:>11.3} {:>11.3} {:>11.3} {:>11.3} {:>11.3} {:>11}\n",
            s.label,
            db(s.rms),
            db(s.peak),
            db(s.avg),
            round3(s.crest_db()),
            round3(s.headroom_db()),
            s.clipped,
        ));
    }
    out.push('\n');
    for s in per_channel.iter().chain(std::iter::once(overall)) {
        out.push_str(&format!(
            "{}: RMS {} (linear {}), peak {} (linear {}), short-window RMS peak {} / trough {}, \
             DC offset {}, clipped {} sample(s) = {}% (longest run {}), zero crossings {}\n",
            s.label,
            num(db(s.rms)),
            num(round6(s.rms)),
            num(db(s.peak)),
            num(round6(s.peak)),
            opt_db(s.rms_peak),
            opt_db(s.rms_trough),
            num(round6(s.dc_offset)),
            s.clipped,
            num(round3(s.clipped_pct())),
            s.longest_clip_run,
            s.zero_crossings,
        ));
    }
    out
}

fn render_csv(per_channel: &[Stats], overall: &Stats) -> String {
    let mut out = String::new();
    out.push_str(
        "channel,label,samples,rms_dbfs,peak_dbfs,average_dbfs,rms_linear,peak_linear,\
         average_linear,rms_peak_dbfs,rms_trough_dbfs,crest_factor_db,headroom_db,dc_offset,\
         clipped_samples,clipped_pct,longest_clipped_run,zero_crossings\n",
    );
    for (i, s) in per_channel
        .iter()
        .chain(std::iter::once(overall))
        .enumerate()
    {
        let idx = if i < per_channel.len() {
            (i + 1).to_string()
        } else {
            "overall".to_string()
        };
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            idx,
            s.label,
            s.samples,
            num(db(s.rms)),
            num(db(s.peak)),
            num(db(s.avg)),
            num(round6(s.rms)),
            num(round6(s.peak)),
            num(round6(s.avg)),
            opt_db(s.rms_peak),
            opt_db(s.rms_trough),
            num(round3(s.crest_db())),
            num(round3(s.headroom_db())),
            num(round6(s.dc_offset)),
            s.clipped,
            num(round3(s.clipped_pct())),
            s.longest_clip_run,
            s.zero_crossings,
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Decode + measure (base64/hex → symphonia → per-channel accumulators)
// ---------------------------------------------------------------------------

/// Short label for one channel bit flag, in the interleaved bit order.
fn channel_label(flag: Channels) -> &'static str {
    match flag {
        Channels::FRONT_LEFT => "L",
        Channels::FRONT_RIGHT => "R",
        Channels::FRONT_CENTRE => "C",
        Channels::LFE1 => "LFE",
        Channels::REAR_LEFT => "Rl",
        Channels::REAR_RIGHT => "Rr",
        Channels::FRONT_LEFT_CENTRE => "Lc",
        Channels::FRONT_RIGHT_CENTRE => "Rc",
        Channels::REAR_CENTRE => "Cs",
        Channels::SIDE_LEFT => "Sl",
        Channels::SIDE_RIGHT => "Sr",
        Channels::TOP_CENTRE => "Tc",
        Channels::TOP_FRONT_LEFT => "Tfl",
        Channels::TOP_FRONT_CENTRE => "Tfc",
        Channels::TOP_FRONT_RIGHT => "Tfr",
        Channels::TOP_REAR_LEFT => "Trl",
        Channels::TOP_REAR_CENTRE => "Trc",
        Channels::TOP_REAR_RIGHT => "Trr",
        Channels::LFE2 => "LFE2",
        _ => "ch",
    }
}

/// Labels for a channel map: the declared layout when it is usable, else
/// positional `ch1..chN` names (and `M` for a lone mono channel).
fn labels_for(map: Channels, count: usize) -> Vec<String> {
    // A single channel is mono whatever position flag the demuxer picked
    // (symphonia maps Layout::Mono onto FRONT_LEFT).
    if count == 1 {
        return vec!["M".to_string()];
    }
    if map.count() == count {
        let mut seen_generic = 0usize;
        return map
            .iter()
            .map(|f| {
                let l = channel_label(f);
                if l == "ch" {
                    seen_generic += 1;
                    format!("ch{seen_generic}")
                } else {
                    l.to_string()
                }
            })
            .collect();
    }
    (1..=count).map(|i| format!("ch{i}")).collect()
}

/// Demux `bytes`, decode the first decodable audio track, and fold every sample
/// into the per-channel accumulators. Caps frames at [`MAX_FRAMES`].
fn analyze(bytes: Vec<u8>, window_ms: f64, clip_threshold: f64) -> Result<Analysis, String> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("unrecognized or unsupported media format ({SUPPORTED}): {e}"))?;
    let mut format = probed.format;

    // Pick the first track a decoder can be built for. Video tracks are
    // CODEC_TYPE_NULL; known-but-undecodable audio (e.g. Opus) fails decoder
    // construction and is reported honestly.
    let mut decoder = None;
    let mut track_id = 0;
    let mut seen: Vec<String> = Vec::new();
    for t in format.tracks() {
        if t.codec_params.codec == CODEC_TYPE_NULL {
            continue;
        }
        match symphonia::default::get_codecs().make(&t.codec_params, &DecoderOptions::default()) {
            Ok(d) => {
                decoder = Some(d);
                track_id = t.id;
                break;
            }
            Err(_) => seen.push(format!("{}", t.codec_params.codec)),
        }
    }
    let mut decoder = decoder.ok_or_else(|| {
        if seen.is_empty() {
            format!("no decodable audio track found (an image or video-only file has no levels to measure); {SUPPORTED}")
        } else {
            format!("the audio track's codec is not supported ({}); {SUPPORTED}", seen.join(", "))
        }
    })?;

    let mut accs: Vec<Acc> = Vec::new();
    let mut rate: u32 = 0;
    let mut channels: usize = 0;
    let mut win_len: u64 = 0;
    let mut frames: u64 = 0;
    let mut truncated = false;
    let mut sbuf: Option<SampleBuffer<f32>> = None;

    'outer: loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(format!("error reading audio: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                if decoded.frames() == 0 {
                    continue;
                }
                let spec = *decoded.spec();
                let ch = spec.channels.count();
                if rate == 0 {
                    if spec.rate == 0 {
                        return Err("stream reports a zero sample rate".into());
                    }
                    if ch == 0 || ch > MAX_CHANNELS {
                        return Err(format!(
                            "unsupported channel count {ch} (1-{MAX_CHANNELS} channels are supported)"
                        ));
                    }
                    rate = spec.rate;
                    channels = ch;
                    win_len = ((window_ms / 1000.0 * rate as f64).round() as u64).max(1);
                    accs = labels_for(spec.channels, ch).into_iter().map(Acc::new).collect();
                } else if spec.rate != rate || ch != channels {
                    return Err("variable sample rate / channel layout is not supported".into());
                }
                let recreate = match &sbuf {
                    Some(b) => b.capacity() < decoded.capacity() * channels,
                    None => true,
                };
                if recreate {
                    sbuf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                }
                let buf = sbuf.as_mut().expect("sample buffer just created");
                buf.copy_interleaved_ref(decoded);
                for frame in buf.samples().chunks_exact(channels) {
                    if frames >= MAX_FRAMES {
                        truncated = true;
                        break 'outer;
                    }
                    for (acc, &v) in accs.iter_mut().zip(frame) {
                        acc.push(v as f64, clip_threshold, win_len);
                    }
                    frames += 1;
                }
            }
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode failed: {e}")),
        }
    }

    if rate == 0 || frames == 0 {
        return Err(format!(
            "no audio could be decoded from these bytes ({SUPPORTED})"
        ));
    }
    for acc in &mut accs {
        acc.finish();
    }
    Ok(Analysis {
        sample_rate: rate,
        channels: accs,
        frames,
        truncated,
    })
}

// ---------------------------------------------------------------------------
// Byte input decode (base64 / hex)
// ---------------------------------------------------------------------------

fn decode_bytes(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty: paste the audio file bytes as base64 or hex".into());
    }
    let bytes = match input_format.trim() {
        "" | "base64" => decode_base64(trimmed),
        "hex" => decode_hex(trimmed),
        other => Err(format!(
            "invalid input_format {other:?}: expected \"base64\" or \"hex\""
        )),
    }?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes: the limit is {MAX_INPUT_BYTES} bytes (64 MiB) — trim the clip first",
            bytes.len()
        ));
    }
    Ok(bytes)
}

/// Standard + URL-safe base64, padding optional.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    const INVALID: u8 = 255;
    let val = |c: u8| -> u8 {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => INVALID,
        }
    };
    let mut buf = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c);
        if v == INVALID {
            return Err(format!(
                "invalid base64 character {:?} — paste base64 bytes, or switch the encoding to hex",
                c as char
            ));
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let compact: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if compact.len() % 2 != 0 {
        return Err("invalid hex: odd number of digits".into());
    }
    let bytes = compact.as_bytes();
    let mut out = Vec::with_capacity(compact.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = hex_val(pair[0])?;
        let lo = hex_val(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_val(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!(
            "invalid hex digit {:?} — paste hex bytes, or switch the encoding to base64",
            c as char
        )),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal PCM16 WAV from interleaved f32 samples in [-1, 1].
    fn make_wav(sample_rate: u32, channels: u16, samples: &[f32]) -> Vec<u8> {
        let bits = 16u16;
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut data: Vec<u8> = Vec::new();
        for &s in samples {
            let s = s.clamp(-1.0, 1.0);
            data.extend_from_slice(&((s * 32767.0) as i16).to_le_bytes());
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        out
    }

    fn b64(bytes: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = *chunk.get(1).unwrap_or(&0) as u32;
            let b2 = *chunk.get(2).unwrap_or(&0) as u32;
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(TABLE[(n >> 18) as usize & 63] as char);
            out.push(TABLE[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 {
                TABLE[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                TABLE[(n & 63) as usize] as char
            } else {
                '='
            });
        }
        out
    }

    /// Grab the numeric value of `key` from the JSON output.
    fn field(json: &str, key: &str) -> f64 {
        let needle = format!("\"{key}\": ");
        let at = json.find(&needle).unwrap_or_else(|| panic!("no {key} in {json}"));
        let rest = &json[at + needle.len()..];
        let end = rest
            .find(|c: char| !matches!(c, '0'..='9' | '-' | '.' | 'e'))
            .unwrap_or(rest.len());
        rest[..end].parse().unwrap_or_else(|_| panic!("bad {key}: {}", &rest[..end]))
    }

    #[test]
    fn constant_half_scale_mono_reports_matching_rms_peak_average() {
        // A DC-constant 0.5 signal: RMS = peak = mean |amplitude| = 0.5 → -6.02 dBFS.
        let wav = make_wav(8000, 1, &vec![0.5f32; 8000]);
        let out = run(&b64(&wav), "base64", "json", 50.0, 0.99).unwrap();
        assert!(out.contains("\"sample_rate\": 8000"), "got: {out}");
        assert!(out.contains("\"channels\": 1"));
        assert!(out.contains("\"channel_layout\": [\"M\"]"), "got: {out}");
        assert!(out.contains("\"duration_s\": 1"));
        assert!(out.contains("\"frames\": 8000"));
        assert!((field(&out, "rms_dbfs") + 6.021).abs() < 0.01);
        assert!((field(&out, "peak_dbfs") + 6.021).abs() < 0.01);
        assert!((field(&out, "average_dbfs") + 6.021).abs() < 0.01);
        assert!((field(&out, "rms_linear") - 0.5).abs() < 1e-3);
        assert!((field(&out, "peak_linear") - 0.5).abs() < 1e-3);
        assert!((field(&out, "average_linear") - 0.5).abs() < 1e-3);
        // Crest factor of a constant signal is 0 dB; headroom is 6 dB.
        assert!(field(&out, "crest_factor_db").abs() < 0.01);
        assert!((field(&out, "headroom_db") - 6.021).abs() < 0.01);
        assert_eq!(field(&out, "clipped_samples"), 0.0);
        assert_eq!(field(&out, "zero_crossings"), 0.0);
        assert!(out.contains("\"truncated\": false"));
    }

    #[test]
    fn stereo_channels_are_measured_separately_not_downmixed() {
        // L = +1.0 (full scale), R = 0.25. A mono downmix would report one
        // middle level; a per-channel report must keep them apart.
        let samples: Vec<f32> = (0..16000)
            .map(|i| if i % 2 == 0 { 1.0 } else { 0.25 })
            .collect();
        let wav = make_wav(8000, 2, &samples);
        let out = run(&b64(&wav), "base64", "csv", 50.0, 0.99).unwrap();
        let lines: Vec<&str> = out.trim_end().lines().collect();
        assert_eq!(lines.len(), 4, "header + L + R + overall");
        let l: Vec<&str> = lines[1].split(',').collect();
        let r: Vec<&str> = lines[2].split(',').collect();
        let o: Vec<&str> = lines[3].split(',').collect();
        assert_eq!(l[1], "L");
        assert_eq!(r[1], "R");
        assert_eq!(o[0], "overall");
        // L ≈ 0 dBFS, R ≈ -12.04 dBFS.
        assert!(l[3].parse::<f64>().unwrap().abs() < 0.05, "L rms {}", l[3]);
        assert!((r[3].parse::<f64>().unwrap() + 12.041).abs() < 0.05, "R rms {}", r[3]);
        // Overall RMS of {1.0, 0.25} = sqrt((1+0.0625)/2) ≈ 0.7289 → -2.75 dBFS.
        assert!((o[3].parse::<f64>().unwrap() + 2.75).abs() < 0.05, "overall {}", o[3]);
        assert!(o[4].parse::<f64>().unwrap().abs() < 0.05, "overall peak {}", o[4]);
    }

    #[test]
    fn clipping_is_counted_per_channel_with_the_longest_run() {
        // 100 samples: the first 10 are full scale (one run), then silence.
        let mut samples = vec![1.0f32; 10];
        samples.extend(vec![0.0f32; 90]);
        let wav = make_wav(8000, 1, &samples);
        let out = run(&b64(&wav), "base64", "json", 50.0, 0.99).unwrap();
        assert_eq!(field(&out, "clipped_samples"), 10.0);
        assert_eq!(field(&out, "longest_clipped_run"), 10.0);
        assert!((field(&out, "clipped_pct") - 10.0).abs() < 1e-6);
        // A lower threshold catches more samples; a threshold above the signal none.
        let quiet = make_wav(8000, 1, &vec![0.6f32; 100]);
        let strict = run(&b64(&quiet), "base64", "json", 50.0, 0.99).unwrap();
        assert_eq!(field(&strict, "clipped_samples"), 0.0);
        let loose = run(&b64(&quiet), "base64", "json", 50.0, 0.5).unwrap();
        assert_eq!(field(&loose, "clipped_samples"), 100.0);
    }

    #[test]
    fn silence_floors_at_minus_120_dbfs() {
        let wav = make_wav(8000, 1, &vec![0.0f32; 4000]);
        let out = run(&b64(&wav), "base64", "json", 50.0, 0.99).unwrap();
        assert_eq!(field(&out, "rms_dbfs"), SILENCE_FLOOR_DBFS);
        assert_eq!(field(&out, "peak_dbfs"), SILENCE_FLOOR_DBFS);
        assert_eq!(field(&out, "average_dbfs"), SILENCE_FLOOR_DBFS);
        // Crest factor is undefined for silence — reported as 0, not NaN.
        assert_eq!(field(&out, "crest_factor_db"), 0.0);
        assert!(!out.contains("NaN") && !out.contains("inf"), "got: {out}");
    }

    #[test]
    fn short_window_rms_peak_and_trough_track_loud_and_quiet_passages() {
        // 0.5 s of full scale then 0.5 s of -20 dBFS (0.1), 50 ms windows.
        let mut samples = vec![1.0f32; 4000];
        samples.extend(vec![0.1f32; 4000]);
        let wav = make_wav(8000, 1, &samples);
        let out = run(&b64(&wav), "base64", "json", 50.0, 0.99).unwrap();
        assert!(field(&out, "rms_peak_dbfs").abs() < 0.05, "loud window ~0 dBFS");
        assert!(
            (field(&out, "rms_trough_dbfs") + 20.0).abs() < 0.05,
            "quiet window ~-20 dBFS"
        );
        // The whole-file RMS sits between the two.
        let rms = field(&out, "rms_dbfs");
        assert!(rms < -2.0 && rms > -4.0, "whole-file rms {rms}");
    }

    #[test]
    fn dc_offset_and_zero_crossings_are_reported() {
        // Alternating ±0.5 → mean 0, and a crossing between every sample pair.
        let samples: Vec<f32> = (0..1000)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        let wav = make_wav(8000, 1, &samples);
        let out = run(&b64(&wav), "base64", "json", 50.0, 0.99).unwrap();
        assert!(field(&out, "dc_offset").abs() < 1e-3, "no DC on a symmetric wave");
        assert_eq!(field(&out, "zero_crossings"), 999.0);
        // A constant positive signal is pure DC with no crossings.
        let dc = make_wav(8000, 1, &vec![0.25f32; 1000]);
        let out = run(&b64(&dc), "base64", "json", 50.0, 0.99).unwrap();
        assert!((field(&out, "dc_offset") - 0.25).abs() < 1e-3);
        assert_eq!(field(&out, "zero_crossings"), 0.0);
    }

    #[test]
    fn report_output_is_an_aligned_table_with_an_overall_row() {
        let wav = make_wav(8000, 2, &vec![0.5f32; 16000]);
        let out = run(&b64(&wav), "base64", "report", 50.0, 0.99).unwrap();
        assert!(out.starts_with("Audio level report"), "got: {out}");
        assert!(out.contains("8000 Hz, 2 channel(s) (L / R), 1 s, 8000 frames"), "got: {out}");
        assert!(out.contains("RMS window 50 ms, clip threshold |sample| >= 0.99"));
        assert!(out.contains("Channel      RMS dBFS"));
        assert!(out.contains("\noverall "), "overall row present: {out}");
        assert!(out.contains("zero crossings"));
    }

    #[test]
    fn hex_input_is_accepted() {
        let wav = make_wav(8000, 1, &vec![0.5f32; 800]);
        let hex: String = wav.iter().map(|b| format!("{b:02x}")).collect();
        let out = run(&hex, "hex", "json", 50.0, 0.99).unwrap();
        assert!((field(&out, "rms_linear") - 0.5).abs() < 1e-3);
    }

    #[test]
    fn blank_numbers_fall_back_to_defaults() {
        // The page sends 0 for an empty number box — that must mean "default",
        // not "invalid".
        let wav = make_wav(8000, 1, &vec![0.5f32; 800]);
        let out = run(&b64(&wav), "base64", "", 0.0, 0.0).unwrap();
        assert!(out.contains("\"rms_window_ms\": 50"), "got: {out}");
        assert!(out.contains("\"clip_threshold\": 0.99"));
    }

    #[test]
    fn empty_input_errors() {
        let e = run("", "base64", "json", 50.0, 0.99).unwrap_err();
        assert!(e.contains("empty"), "got: {e}");
    }

    #[test]
    fn non_audio_bytes_error_clearly() {
        let e = run(&b64(b"this is not a media file"), "base64", "json", 50.0, 0.99).unwrap_err();
        assert!(e.contains("unrecognized or unsupported media format"), "got: {e}");
    }

    #[test]
    fn invalid_output_errors() {
        let wav = make_wav(8000, 1, &vec![0.1f32; 800]);
        let e = run(&b64(&wav), "base64", "bogus", 50.0, 0.99).unwrap_err();
        assert!(e.contains("output") && e.contains("json"), "got: {e}");
    }

    #[test]
    fn out_of_range_window_and_threshold_error() {
        let wav = make_wav(8000, 1, &vec![0.1f32; 800]);
        let e = run(&b64(&wav), "base64", "json", 99_999.0, 0.99).unwrap_err();
        assert!(e.contains("rms_window_ms"), "got: {e}");
        let e = run(&b64(&wav), "base64", "json", 50.0, 1.5).unwrap_err();
        assert!(e.contains("clip_threshold"), "got: {e}");
    }

    #[test]
    fn invalid_encoding_characters_say_what_to_do() {
        let e = run("!!!!not base64!!!!", "base64", "json", 50.0, 0.99).unwrap_err();
        assert!(e.contains("invalid base64 character") && e.contains("hex"), "got: {e}");
        let e = run("zz", "hex", "json", 50.0, 0.99).unwrap_err();
        assert!(e.contains("invalid hex digit"), "got: {e}");
        let e = run("abc", "hex", "json", 50.0, 0.99).unwrap_err();
        assert!(e.contains("odd number of digits"), "got: {e}");
        let e = run("AAAA", "rot13", "json", 50.0, 0.99).unwrap_err();
        assert!(e.contains("input_format"), "got: {e}");
    }

    #[test]
    fn clip_shorter_than_the_window_still_reports_a_window_rms() {
        // 10 ms of audio with a 50 ms window: no full window closes, so the
        // trailing partial window is used rather than reporting null.
        let wav = make_wav(8000, 1, &vec![0.5f32; 80]);
        let out = run(&b64(&wav), "base64", "json", 50.0, 0.99).unwrap();
        assert!(!out.contains("\"rms_peak_dbfs\": null"), "got: {out}");
        assert!((field(&out, "rms_peak_dbfs") + 6.021).abs() < 0.05);
    }
}
