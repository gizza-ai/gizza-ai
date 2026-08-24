//! audio-feature-csv-exporter core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Turns one audio file into a **frame-level feature table** — the low-level
//! descriptors used for audio analysis, tagging, QC dashboards and classic
//! (non-deep) audio ML — and renders it as CSV, TSV or JSON.
//!
//! Features per analysis frame:
//!
//! | column         | definition                                                        |
//! |----------------|-------------------------------------------------------------------|
//! | `rms`          | `sqrt(mean(x^2))` of the raw (unwindowed) frame, linear or dBFS   |
//! | `centroid_hz`  | `sum(f_k * S_k) / sum(S_k)` — the magnitude spectrum's mean freq  |
//! | `zcr`          | sign changes in the frame / frame length (0-1)                    |
//! | `rolloff_hz`   | lowest frequency below which `rolloff_percent` of `sum(S_k)` sits |
//! | `flatness`     | geometric mean / arithmetic mean of the power spectrum (0-1)      |
//! | `bandwidth_hz` | `sqrt(sum(S_k * (f_k - centroid)^2) / sum(S_k))` (optional)       |
//! | `flux`         | L2 norm of the positive change in the L1-normalised spectrum      |
//!
//! Pipeline:
//!
//! 1. decode the pasted file bytes (base64/hex) → symphonia demuxes the
//!    container and decodes the FIRST decodable audio track;
//! 2. pick or downmix a channel inside the packet loop (native-rate
//!    multi-channel PCM is never held in memory);
//! 3. optionally resample to a fixed analysis rate (box pre-filter on
//!    downsampling, then linear interpolation);
//! 4. frame into `frame_ms` windows every `hop_ms` — complete frames from
//!    sample 0 by default, or librosa-style centred frames when `center` is on
//!    (the signal is zero-padded by half a frame at both ends);
//! 5. time-domain features (RMS, ZCR) read the raw frame; spectral features
//!    read the analysis-windowed frame zero-padded to the next power of two and
//!    transformed with a radix-2 FFT.
//!
//! Definitions follow librosa's `feature.*` family so exported tables line up
//! with the reference implementation. Pure Rust → runs on every backend,
//! including the chat Service Worker.

use std::f64::consts::PI;
use std::io::Cursor;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Hard cap on decoded input bytes (24 MiB). Pasting more than this as
/// base64/hex is rejected before any decoding work happens.
pub const MAX_INPUT_BYTES: usize = 24 * 1024 * 1024;

/// Hard cap on buffered mono samples (~83 s at 48 kHz, ~250 s at 16 kHz).
/// Longer audio is analysed up to this point and the truncation is reported.
pub const MAX_SAMPLES: usize = 4_000_000;

/// Hard cap on emitted rows, so a 1 ms hop can't produce a gigabyte of CSV.
pub const MAX_OUTPUT_FRAMES: usize = 200_000;

/// Largest channel count the decoder accepts.
pub const MAX_CHANNELS: usize = 16;

// Parameter bounds — mirrored by the descriptor in ../../src/lib.rs.
pub const MIN_FRAME_MS: f64 = 1.0;
pub const MAX_FRAME_MS: f64 = 500.0;
pub const MIN_HOP_MS: f64 = 1.0;
pub const MAX_HOP_MS: f64 = 500.0;
pub const MIN_ROLLOFF_PERCENT: f64 = 1.0;
pub const MAX_ROLLOFF_PERCENT: f64 = 99.0;
pub const MAX_DECIMALS: i64 = 8;
pub const MIN_RESAMPLE_HZ: i64 = 4_000;
pub const MAX_RESAMPLE_HZ: i64 = 48_000;

/// librosa's `amin` for `spectral_flatness`: the power-spectrum floor, so a
/// digitally silent frame yields flatness 1.0 instead of 0/0.
const AMIN: f64 = 1e-10;

/// Amplitude floor for the dBFS conversions (-200 dBFS).
const DB_FLOOR: f64 = 1e-10;

const SUPPORTED: &str = "supported containers: WAV, AIFF, CAF, FLAC, MP3, OGG, MP4/MOV/M4A, \
                         MKV/WebM, AAC-ADTS; audio codecs: PCM, ADPCM, FLAC, MP3, AAC-LC, ALAC, \
                         Vorbis (Opus, AC-3 and DTS are not supported)";

/// Every knob except the input bytes. Grouped so the chat block, the CLI and
/// the browser wrapper all construct exactly the same configuration.
#[derive(Clone, Debug)]
pub struct Options {
    /// `"csv"` (default), `"tsv"` or `"json"`.
    pub output: String,
    /// Analysis frame length, milliseconds.
    pub frame_ms: f64,
    /// Frame hop (step), milliseconds.
    pub hop_ms: f64,
    /// `"hann"` (default), `"hamming"`, `"blackman"` or `"rectangular"`.
    pub window: String,
    /// Zero-pad half a frame at both ends so frame `t` is centred on
    /// `t * hop` (librosa's `center=True`).
    pub center: bool,
    /// `"mix"` (default downmix), `"left"` or `"right"`.
    pub channel: String,
    /// Analysis sample rate; `0` keeps the file's own rate.
    pub resample_hz: i64,
    /// Emit the frame RMS level column.
    pub rms: bool,
    /// Emit the spectral centroid column.
    pub centroid: bool,
    /// Emit the zero-crossing-rate column.
    pub zcr: bool,
    /// Emit the spectral rolloff column.
    pub rolloff: bool,
    /// Emit the spectral flatness column.
    pub flatness: bool,
    /// Emit the spectral bandwidth (spread) column.
    pub bandwidth: bool,
    /// Emit the spectral flux column.
    pub flux: bool,
    /// Rolloff energy percentage (1-99).
    pub rolloff_percent: f64,
    /// `"dbfs"` (default) or `"linear"`.
    pub rms_scale: String,
    /// `"ratio"` (default) or `"db"`.
    pub flatness_scale: String,
    /// Prepend a `time_s` column.
    pub include_time: bool,
    /// Prepend a `frame` index column.
    pub include_frame: bool,
    /// Decimal places in the rendered numbers.
    pub decimals: i64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            output: "csv".into(),
            frame_ms: 25.0,
            hop_ms: 10.0,
            window: "hann".into(),
            center: false,
            channel: "mix".into(),
            resample_hz: 0,
            rms: true,
            centroid: true,
            zcr: true,
            rolloff: true,
            flatness: true,
            bandwidth: false,
            flux: false,
            rolloff_percent: 85.0,
            rms_scale: "dbfs".into(),
            flatness_scale: "ratio".into(),
            include_time: true,
            include_frame: false,
            decimals: 6,
        }
    }
}

/// Output rendering.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OutMode {
    Csv,
    Tsv,
    Json,
}

/// The validated, resolved configuration — every "0 means auto" resolved and
/// every millisecond converted into samples at the analysis rate.
struct Resolved {
    out_mode: OutMode,
    frame_len: usize,
    hop_len: usize,
    n_fft: usize,
    window: Vec<f64>,
    window_name: String,
    center: bool,
    rolloff_fraction: f64,
    rms_db: bool,
    flatness_db: bool,
    include_time: bool,
    include_frame: bool,
    decimals: usize,
    /// Which feature columns are on, in output order.
    cols: Vec<Feature>,
}

/// One optional feature column.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Feature {
    Rms,
    Centroid,
    Zcr,
    Rolloff,
    Flatness,
    Bandwidth,
    Flux,
}

impl Feature {
    /// Whether the column needs the FFT of the frame.
    fn spectral(self) -> bool {
        !matches!(self, Feature::Rms | Feature::Zcr)
    }
}

/// Facts about the decoded stream, carried into the JSON metadata.
struct Decoded {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: usize,
    truncated: bool,
}

/// Export frame-level audio features from one audio file.
///
/// - `input`: the audio file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `opts`: every other knob; see [`Options`].
///
/// Returns a user-facing error string for empty/oversized input, undecodable
/// bytes, an unsupported container/codec, audio shorter than one analysis
/// frame, no selected features, or any out-of-range parameter.
pub fn run(input: &str, input_format: &str, opts: &Options) -> Result<String, String> {
    let bytes = decode_bytes(input, input_format)?;
    let channel = parse_channel(&opts.channel)?;
    let decoded = decode_audio(bytes, channel)?;
    let source_rate = decoded.sample_rate;

    // Resample first: every millisecond bound resolves at the analysis rate.
    let (signal, rate) = match opts.resample_hz {
        0 => (decoded.samples, source_rate),
        hz => {
            if !(MIN_RESAMPLE_HZ..=MAX_RESAMPLE_HZ).contains(&hz) {
                return Err(format!(
                    "invalid resample_hz {hz}: expected 0 (keep the file's rate) or \
                     {MIN_RESAMPLE_HZ}-{MAX_RESAMPLE_HZ} Hz"
                ));
            }
            let target = hz as u32;
            if target == source_rate {
                (decoded.samples, source_rate)
            } else {
                (resample(&decoded.samples, source_rate, target), target)
            }
        }
    };

    let cfg = resolve(opts, rate)?;

    // Centring pads half a frame of zeros at both ends (librosa center=True
    // with the default constant pad mode), so frame t is centred on t*hop.
    let pad = if cfg.center { cfg.frame_len / 2 } else { 0 };
    let padded_len = signal.len() + 2 * pad;
    if padded_len < cfg.frame_len {
        return Err(format!(
            "audio is {} samples ({:.3} s at {rate} Hz) but one analysis frame is {} samples \
             ({} ms) — use a shorter frame_ms, turn on center, or use a longer clip",
            signal.len(),
            signal.len() as f64 / rate as f64,
            cfg.frame_len,
            opts.frame_ms
        ));
    }

    let total_frames = 1 + (padded_len - cfg.frame_len) / cfg.hop_len;
    let frames = total_frames.min(MAX_OUTPUT_FRAMES);
    let frames_truncated = frames < total_frames;

    let table = extract(&signal, &cfg, rate, pad, frames);

    Ok(render(
        &table,
        &cfg,
        opts,
        rate,
        source_rate,
        decoded.channels,
        signal.len(),
        decoded.truncated,
        frames_truncated,
        total_frames,
    ))
}

// ---------------------------------------------------------------------------
// Parameter validation
// ---------------------------------------------------------------------------

/// Which source channel feeds the analysis.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Channel {
    Mix,
    Left,
    Right,
}

fn parse_channel(name: &str) -> Result<Channel, String> {
    match name.trim() {
        "" | "mix" => Ok(Channel::Mix),
        "left" => Ok(Channel::Left),
        "right" => Ok(Channel::Right),
        other => Err(format!(
            "invalid channel {other:?}: expected \"mix\", \"left\" or \"right\""
        )),
    }
}

fn resolve(opts: &Options, rate: u32) -> Result<Resolved, String> {
    let out_mode = match opts.output.trim() {
        "" | "csv" => OutMode::Csv,
        "tsv" => OutMode::Tsv,
        "json" => OutMode::Json,
        other => {
            return Err(format!(
                "invalid output {other:?}: expected \"csv\", \"tsv\" or \"json\""
            ))
        }
    };
    let rms_db = match opts.rms_scale.trim() {
        "" | "dbfs" => true,
        "linear" => false,
        other => {
            return Err(format!(
                "invalid rms_scale {other:?}: expected \"dbfs\" or \"linear\""
            ))
        }
    };
    let flatness_db = match opts.flatness_scale.trim() {
        "" | "ratio" => false,
        "db" => true,
        other => {
            return Err(format!(
                "invalid flatness_scale {other:?}: expected \"ratio\" or \"db\""
            ))
        }
    };
    if !opts.frame_ms.is_finite() || !(MIN_FRAME_MS..=MAX_FRAME_MS).contains(&opts.frame_ms) {
        return Err(format!(
            "invalid frame_ms {}: expected {MIN_FRAME_MS}-{MAX_FRAME_MS} ms",
            opts.frame_ms
        ));
    }
    if !opts.hop_ms.is_finite() || !(MIN_HOP_MS..=MAX_HOP_MS).contains(&opts.hop_ms) {
        return Err(format!(
            "invalid hop_ms {}: expected {MIN_HOP_MS}-{MAX_HOP_MS} ms",
            opts.hop_ms
        ));
    }
    if !opts.rolloff_percent.is_finite()
        || !(MIN_ROLLOFF_PERCENT..=MAX_ROLLOFF_PERCENT).contains(&opts.rolloff_percent)
    {
        return Err(format!(
            "invalid rolloff_percent {}: expected {MIN_ROLLOFF_PERCENT}-{MAX_ROLLOFF_PERCENT} \
             percent of the frame's spectral energy",
            opts.rolloff_percent
        ));
    }
    if !(0..=MAX_DECIMALS).contains(&opts.decimals) {
        return Err(format!(
            "invalid decimals {}: expected 0-{MAX_DECIMALS}",
            opts.decimals
        ));
    }

    let mut cols = Vec::new();
    for (on, feature) in [
        (opts.rms, Feature::Rms),
        (opts.centroid, Feature::Centroid),
        (opts.zcr, Feature::Zcr),
        (opts.rolloff, Feature::Rolloff),
        (opts.flatness, Feature::Flatness),
        (opts.bandwidth, Feature::Bandwidth),
        (opts.flux, Feature::Flux),
    ] {
        if on {
            cols.push(feature);
        }
    }
    if cols.is_empty() {
        return Err("no features selected: turn on at least one of rms, centroid, zcr, rolloff, \
                    flatness, bandwidth or flux"
            .into());
    }

    let frame_len = ((opts.frame_ms / 1000.0 * rate as f64).round() as usize).max(2);
    let hop_len = ((opts.hop_ms / 1000.0 * rate as f64).round() as usize).max(1);
    let mut n_fft = 2usize;
    while n_fft < frame_len {
        n_fft <<= 1;
    }

    let (window, window_name) = analysis_window(&opts.window, frame_len)?;

    Ok(Resolved {
        out_mode,
        frame_len,
        hop_len,
        n_fft,
        window,
        window_name,
        center: opts.center,
        rolloff_fraction: opts.rolloff_percent / 100.0,
        rms_db,
        flatness_db,
        include_time: opts.include_time,
        include_frame: opts.include_frame,
        decimals: opts.decimals as usize,
        cols,
    })
}

/// Symmetric window coefficients.
fn analysis_window(name: &str, len: usize) -> Result<(Vec<f64>, String), String> {
    let denom = (len - 1).max(1) as f64;
    let (name, w): (&str, Vec<f64>) = match name.trim() {
        "" | "hann" => (
            "hann",
            (0..len)
                .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / denom).cos())
                .collect(),
        ),
        "hamming" => (
            "hamming",
            (0..len)
                .map(|i| 0.54 - 0.46 * (2.0 * PI * i as f64 / denom).cos())
                .collect(),
        ),
        "blackman" => (
            "blackman",
            (0..len)
                .map(|i| {
                    let x = 2.0 * PI * i as f64 / denom;
                    0.42 - 0.5 * x.cos() + 0.08 * (2.0 * x).cos()
                })
                .collect(),
        ),
        "rectangular" => ("rectangular", vec![1.0; len]),
        other => {
            return Err(format!(
                "invalid window {other:?}: expected \"hann\", \"hamming\", \"blackman\" or \
                 \"rectangular\""
            ))
        }
    };
    Ok((w, name.to_string()))
}

// ---------------------------------------------------------------------------
// Resampling
// ---------------------------------------------------------------------------

/// Box pre-filter (only when downsampling) followed by linear interpolation.
/// Not a polyphase resampler — good enough to keep aliasing out of the
/// spectral statistics, and documented as such on the page.
fn resample(signal: &[f32], from: u32, to: u32) -> Vec<f32> {
    if signal.is_empty() || from == to {
        return signal.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let filtered: Vec<f32> = if to < from {
        let width = ((from as f64 / to as f64).round() as usize).max(2);
        let mut acc = 0.0f64;
        let mut out = Vec::with_capacity(signal.len());
        for i in 0..signal.len() {
            acc += signal[i] as f64;
            if i >= width {
                acc -= signal[i - width] as f64;
            }
            let n = (i + 1).min(width) as f64;
            out.push((acc / n) as f32);
        }
        out
    } else {
        signal.to_vec()
    };
    let out_len = ((filtered.len() as f64 * ratio).round() as usize).max(1);
    let mut out = Vec::with_capacity(out_len);
    let step = 1.0 / ratio;
    for i in 0..out_len {
        let pos = i as f64 * step;
        let lo = pos.floor() as usize;
        if lo + 1 >= filtered.len() {
            out.push(*filtered.last().expect("non-empty"));
            continue;
        }
        let frac = pos - lo as f64;
        out.push((filtered[lo] as f64 * (1.0 - frac) + filtered[lo + 1] as f64 * frac) as f32);
    }
    out
}

// ---------------------------------------------------------------------------
// FFT (iterative radix-2, precomputed twiddles)
// ---------------------------------------------------------------------------

/// Twiddle table for one transform size: `e^(-2πi·j/n)` for `j < n/2`.
fn twiddles(n: usize) -> (Vec<f64>, Vec<f64>) {
    let half = n / 2;
    let mut re = Vec::with_capacity(half);
    let mut im = Vec::with_capacity(half);
    for j in 0..half {
        let ang = -2.0 * PI * j as f64 / n as f64;
        re.push(ang.cos());
        im.push(ang.sin());
    }
    (re, im)
}

fn fft_in_place(re: &mut [f64], im: &mut [f64], tw_re: &[f64], tw_im: &[f64]) {
    let n = re.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2usize;
    while len <= n {
        let half = len / 2;
        let stride = n / len;
        let mut base = 0usize;
        while base < n {
            for k in 0..half {
                let (wr, wi) = (tw_re[k * stride], tw_im[k * stride]);
                let (ar, ai) = (re[base + k], im[base + k]);
                let (br, bi) = (re[base + k + half], im[base + k + half]);
                let (vr, vi) = (br * wr - bi * wi, br * wi + bi * wr);
                re[base + k] = ar + vr;
                im[base + k] = ai + vi;
                re[base + k + half] = ar - vr;
                im[base + k + half] = ai - vi;
            }
            base += len;
        }
        len <<= 1;
    }
}

// ---------------------------------------------------------------------------
// Feature extraction
// ---------------------------------------------------------------------------

/// Read sample `i` of the (virtually) zero-padded signal.
#[inline]
fn padded(signal: &[f32], pad: usize, i: usize) -> f64 {
    if i < pad {
        return 0.0;
    }
    match signal.get(i - pad) {
        Some(v) => *v as f64,
        None => 0.0,
    }
}

/// One row per frame, one value per selected feature (in `cfg.cols` order).
fn extract(signal: &[f32], cfg: &Resolved, rate: u32, pad: usize, frames: usize) -> Vec<Vec<f64>> {
    let need_fft = cfg.cols.iter().any(|c| c.spectral());
    let need_flux = cfg.cols.contains(&Feature::Flux);
    let n_bins = cfg.n_fft / 2 + 1;
    let bin_hz = rate as f64 / cfg.n_fft as f64;
    let (tw_re, tw_im) = twiddles(cfg.n_fft);

    let mut re = vec![0.0f64; cfg.n_fft];
    let mut im = vec![0.0f64; cfg.n_fft];
    let mut mag = vec![0.0f64; n_bins];
    let mut prev_norm = vec![0.0f64; n_bins];
    let mut have_prev = false;
    let mut out = Vec::with_capacity(frames);

    for f in 0..frames {
        let start = f * cfg.hop_len;

        // --- time-domain pass over the raw (unwindowed) frame --------------
        let mut sq = 0.0f64;
        let mut crossings = 0usize;
        let mut prev_sample = 0.0f64;
        for i in 0..cfg.frame_len {
            let x = padded(signal, pad, start + i);
            sq += x * x;
            if i > 0 && (prev_sample >= 0.0) != (x >= 0.0) {
                crossings += 1;
            }
            prev_sample = x;
        }
        let rms = (sq / cfg.frame_len as f64).sqrt();
        let zcr = crossings as f64 / cfg.frame_len as f64;

        // --- spectral pass over the windowed, zero-padded frame ------------
        let (mut total, mut centroid, mut rolloff, mut flatness, mut bandwidth, mut flux) =
            (0.0f64, 0.0f64, 0.0f64, 1.0f64, 0.0f64, 0.0f64);
        if need_fft {
            for (i, slot) in re.iter_mut().enumerate() {
                *slot = if i < cfg.frame_len {
                    padded(signal, pad, start + i) * cfg.window[i]
                } else {
                    0.0
                };
            }
            im.iter_mut().for_each(|v| *v = 0.0);
            fft_in_place(&mut re, &mut im, &tw_re, &tw_im);

            let mut log_sum = 0.0f64;
            let mut power_sum = 0.0f64;
            for k in 0..n_bins {
                let m = (re[k] * re[k] + im[k] * im[k]).sqrt();
                mag[k] = m;
                total += m;
                centroid += m * k as f64 * bin_hz;
                let p = (m * m).max(AMIN);
                log_sum += p.ln();
                power_sum += p;
            }
            // Flatness: geometric mean / arithmetic mean of the floored power
            // spectrum (librosa spectral_flatness, power=2.0, amin=1e-10).
            flatness = ((log_sum / n_bins as f64).exp() / (power_sum / n_bins as f64)).clamp(0.0, 1.0);

            if total > 0.0 {
                centroid /= total;
                let threshold = cfg.rolloff_fraction * total;
                let mut acc = 0.0f64;
                for k in 0..n_bins {
                    acc += mag[k];
                    if acc >= threshold {
                        rolloff = k as f64 * bin_hz;
                        break;
                    }
                }
                let mut spread = 0.0f64;
                for k in 0..n_bins {
                    let d = k as f64 * bin_hz - centroid;
                    spread += mag[k] * d * d;
                }
                bandwidth = (spread / total).sqrt();
            } else {
                centroid = 0.0;
            }

            if need_flux {
                let inv = if total > 0.0 { 1.0 / total } else { 0.0 };
                let mut acc = 0.0f64;
                for k in 0..n_bins {
                    let cur = mag[k] * inv;
                    if have_prev {
                        let d = cur - prev_norm[k];
                        if d > 0.0 {
                            acc += d * d;
                        }
                    }
                    prev_norm[k] = cur;
                }
                flux = if have_prev { acc.sqrt() } else { 0.0 };
                have_prev = true;
            }
        }

        let mut row = Vec::with_capacity(cfg.cols.len());
        for col in &cfg.cols {
            row.push(match col {
                Feature::Rms => {
                    if cfg.rms_db {
                        20.0 * rms.max(DB_FLOOR).log10()
                    } else {
                        rms
                    }
                }
                Feature::Centroid => centroid,
                Feature::Zcr => zcr,
                Feature::Rolloff => rolloff,
                Feature::Flatness => {
                    if cfg.flatness_db {
                        10.0 * flatness.max(AMIN).log10()
                    } else {
                        flatness
                    }
                }
                Feature::Bandwidth => bandwidth,
                Feature::Flux => flux,
            });
        }
        out.push(row);
    }
    out
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn column_names(cfg: &Resolved) -> Vec<String> {
    let mut cols = Vec::new();
    if cfg.include_frame {
        cols.push("frame".to_string());
    }
    if cfg.include_time {
        cols.push("time_s".to_string());
    }
    for col in &cfg.cols {
        cols.push(
            match col {
                Feature::Rms => {
                    if cfg.rms_db {
                        "rms_dbfs"
                    } else {
                        "rms"
                    }
                }
                Feature::Centroid => "centroid_hz",
                Feature::Zcr => "zcr",
                Feature::Rolloff => "rolloff_hz",
                Feature::Flatness => {
                    if cfg.flatness_db {
                        "flatness_db"
                    } else {
                        "flatness"
                    }
                }
                Feature::Bandwidth => "bandwidth_hz",
                Feature::Flux => "flux",
            }
            .to_string(),
        );
    }
    cols
}

fn num(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return format!("{:.*}", decimals, 0.0);
    }
    let s = format!("{:.*}", decimals, v);
    // Avoid a signed zero in the output ("-0.000000" reads as a bug).
    if s.trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        s.trim_start_matches('-').to_string()
    } else {
        s
    }
}

#[allow(clippy::too_many_arguments)]
fn render(
    table: &[Vec<f64>],
    cfg: &Resolved,
    opts: &Options,
    rate: u32,
    source_rate: u32,
    channels: usize,
    samples: usize,
    samples_truncated: bool,
    frames_truncated: bool,
    total_frames: usize,
) -> String {
    let cols = column_names(cfg);
    let time = |t: usize| t as f64 * cfg.hop_len as f64 / rate as f64;
    let cells = |t: usize, row: &Vec<f64>| -> Vec<String> {
        let mut cells: Vec<String> = Vec::with_capacity(cols.len());
        if cfg.include_frame {
            cells.push(t.to_string());
        }
        if cfg.include_time {
            cells.push(num(time(t), cfg.decimals.max(3)));
        }
        for v in row {
            cells.push(num(*v, cfg.decimals));
        }
        cells
    };

    match cfg.out_mode {
        OutMode::Csv | OutMode::Tsv => {
            let sep = if cfg.out_mode == OutMode::Csv {
                ","
            } else {
                "\t"
            };
            let mut out = String::with_capacity(table.len() * cols.len() * 10 + 64);
            out.push_str(&cols.join(sep));
            out.push('\n');
            for (t, row) in table.iter().enumerate() {
                out.push_str(&cells(t, row).join(sep));
                out.push('\n');
            }
            out
        }
        OutMode::Json => {
            let mut out = String::with_capacity(table.len() * cols.len() * 10 + 512);
            out.push_str("{\n");
            out.push_str(&format!("  \"sample_rate\": {rate},\n"));
            out.push_str(&format!("  \"source_sample_rate\": {source_rate},\n"));
            out.push_str(&format!("  \"source_channels\": {channels},\n"));
            out.push_str(&format!("  \"channel\": \"{}\",\n", channel_name(opts)));
            out.push_str(&format!(
                "  \"duration_s\": {},\n",
                num(samples as f64 / rate as f64, 3)
            ));
            out.push_str(&format!("  \"frames\": {},\n", table.len()));
            out.push_str(&format!("  \"frames_available\": {total_frames},\n"));
            out.push_str(&format!("  \"frame_length_samples\": {},\n", cfg.frame_len));
            out.push_str(&format!("  \"hop_length_samples\": {},\n", cfg.hop_len));
            out.push_str(&format!("  \"fft_size\": {},\n", cfg.n_fft));
            out.push_str(&format!(
                "  \"bin_width_hz\": {},\n",
                num(rate as f64 / cfg.n_fft as f64, 4)
            ));
            out.push_str(&format!("  \"window\": \"{}\",\n", cfg.window_name));
            out.push_str(&format!("  \"center\": {},\n", cfg.center));
            out.push_str(&format!(
                "  \"rolloff_percent\": {},\n",
                num(opts.rolloff_percent, 3)
            ));
            out.push_str(&format!(
                "  \"rms_scale\": \"{}\",\n",
                if cfg.rms_db { "dbfs" } else { "linear" }
            ));
            out.push_str(&format!(
                "  \"flatness_scale\": \"{}\",\n",
                if cfg.flatness_db { "db" } else { "ratio" }
            ));
            out.push_str(&format!("  \"samples_truncated\": {samples_truncated},\n"));
            out.push_str(&format!("  \"frames_truncated\": {frames_truncated},\n"));
            let quoted: Vec<String> = cols.iter().map(|c| format!("\"{c}\"")).collect();
            out.push_str(&format!("  \"columns\": [{}],\n", quoted.join(", ")));
            out.push_str("  \"features\": [\n");
            for (t, row) in table.iter().enumerate() {
                out.push_str(&format!("    [{}]", cells(t, row).join(", ")));
                if t + 1 < table.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("  ]\n}\n");
            out
        }
    }
}

fn channel_name(opts: &Options) -> &'static str {
    match opts.channel.trim() {
        "left" => "left",
        "right" => "right",
        _ => "mix",
    }
}

// ---------------------------------------------------------------------------
// Decode (base64/hex → symphonia → mono f32)
// ---------------------------------------------------------------------------

/// Demux `bytes`, decode the first decodable audio track and reduce it to one
/// channel as it streams. Caps buffered samples at [`MAX_SAMPLES`].
fn decode_audio(bytes: Vec<u8>, channel: Channel) -> Result<Decoded, String> {
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
            format!(
                "no decodable audio track found (an image or video-only file has no audio \
                 features to export); {SUPPORTED}"
            )
        } else {
            format!(
                "the audio track's codec is not supported ({}); {SUPPORTED}",
                seen.join(", ")
            )
        }
    })?;

    let mut samples: Vec<f32> = Vec::new();
    let mut rate: u32 = 0;
    let mut channels: usize = 0;
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
                            "unsupported channel count {ch} (1-{MAX_CHANNELS} channels are \
                             supported)"
                        ));
                    }
                    rate = spec.rate;
                    channels = ch;
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
                let inv = 1.0 / channels as f32;
                // A mono source has no separate right channel — fall back to
                // its only channel rather than erroring.
                let pick = match channel {
                    Channel::Mix => usize::MAX,
                    Channel::Left => 0,
                    Channel::Right => 1.min(channels - 1),
                };
                for frame in buf.samples().chunks_exact(channels) {
                    if samples.len() >= MAX_SAMPLES {
                        truncated = true;
                        break 'outer;
                    }
                    // Grow in fixed steps: Vec's amortized doubling keeps the
                    // old and new buffers alive at once, which trips the
                    // 64 MiB sandbox on multi-MiB signals.
                    if samples.len() == samples.capacity() {
                        let step = 1_000_000.min(MAX_SAMPLES - samples.capacity());
                        samples.reserve_exact(step.max(1));
                    }
                    samples.push(if pick == usize::MAX {
                        frame.iter().sum::<f32>() * inv
                    } else {
                        frame[pick]
                    });
                }
            }
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode failed: {e}")),
        }
    }

    if rate == 0 || samples.is_empty() {
        return Err(format!(
            "no audio could be decoded from these bytes ({SUPPORTED})"
        ));
    }
    Ok(Decoded {
        samples,
        sample_rate: rate,
        channels,
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
            "input is {} bytes: the limit is {MAX_INPUT_BYTES} bytes (24 MiB) — trim the clip first",
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

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// One second of a `freq` Hz tone at 16 kHz, mono, amplitude 0.5.
    fn tone(freq: f64) -> Vec<u8> {
        let rate = 16_000u32;
        let samples: Vec<f32> = (0..rate)
            .map(|i| (2.0 * PI * freq * i as f64 / rate as f64).sin() as f32 * 0.5)
            .collect();
        make_wav(rate, 1, &samples)
    }

    fn tone_wav() -> Vec<u8> {
        tone(440.0)
    }

    /// Split a CSV rendering into header + data rows.
    fn rows(csv: &str) -> Vec<Vec<String>> {
        csv.trim_end()
            .split('\n')
            .map(|l| l.split(',').map(|c| c.to_string()).collect())
            .collect()
    }

    fn col(csv: &str, name: &str, row: usize) -> f64 {
        let r = rows(csv);
        let idx = r[0].iter().position(|c| c == name).expect("column present");
        r[row][idx].parse().expect("numeric cell")
    }

    // -- happy paths --------------------------------------------------------

    #[test]
    fn csv_has_the_default_header_and_one_row_per_frame() {
        let out = run(&b64(&tone_wav()), "base64", &Options::default()).unwrap();
        let r = rows(&out);
        assert_eq!(
            r[0],
            vec!["time_s", "rms_dbfs", "centroid_hz", "zcr", "rolloff_hz", "flatness"]
        );
        // 16000 samples, 400-sample frames every 160 → 1 + (16000-400)/160 = 98.
        assert_eq!(r.len(), 99, "header + 98 frames");
        assert_eq!(r[1][0], "0.000000", "first frame starts at t=0");
        assert_eq!(r[2][0], "0.010000", "second frame one 10 ms hop later");
    }

    #[test]
    fn a_440_hz_tone_lands_on_its_own_centroid_rms_and_zero_crossing_rate() {
        let out = run(&b64(&tone_wav()), "base64", &Options::default()).unwrap();
        // Steady 0.5-amplitude sine: RMS = 0.5/sqrt(2) = 0.3536 → -9.03 dBFS.
        let rms = col(&out, "rms_dbfs", 30);
        assert!((rms - -9.03).abs() < 0.1, "rms_dbfs = {rms}");
        // The centroid of a pure tone sits at the tone (window leakage pulls it
        // slightly up because the spectrum is one-sided).
        let c = col(&out, "centroid_hz", 30);
        assert!((c - 440.0).abs() < 25.0, "centroid = {c} Hz");
        // 440 Hz at 16 kHz → 880 crossings/s → 22 per 400-sample (25 ms) frame.
        let z = col(&out, "zcr", 30);
        assert!((z - 22.0 / 400.0).abs() < 0.004, "zcr = {z}");
        // 85% rolloff of a single partial is at (or just above) the partial.
        let r = col(&out, "rolloff_hz", 30);
        assert!((375.0..=560.0).contains(&r), "rolloff = {r} Hz");
        // A pure tone is the least flat signal there is.
        let f = col(&out, "flatness", 30);
        assert!(f < 0.01, "flatness = {f}");
    }

    #[test]
    fn a_higher_tone_raises_the_centroid_rolloff_and_zcr() {
        let low = run(&b64(&tone(440.0)), "base64", &Options::default()).unwrap();
        let high = run(&b64(&tone(3000.0)), "base64", &Options::default()).unwrap();
        assert!(col(&high, "centroid_hz", 30) > col(&low, "centroid_hz", 30) + 2000.0);
        assert!(col(&high, "rolloff_hz", 30) > col(&low, "rolloff_hz", 30) + 2000.0);
        assert!(col(&high, "zcr", 30) > col(&low, "zcr", 30) * 5.0);
        // 3000 Hz at 16 kHz → 6000 crossings/s → 150 per 400-sample frame.
        let z = col(&high, "zcr", 30);
        assert!((z - 150.0 / 400.0).abs() < 0.01, "zcr = {z}");
    }

    #[test]
    fn white_noise_is_flat_and_a_tone_is_not() {
        // Deterministic pseudo-noise (xorshift) so the test never flakes.
        let rate = 16_000u32;
        let mut state = 0x9E3779B9u32;
        let noise: Vec<f32> = (0..rate)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state as f64 / u32::MAX as f64 * 2.0 - 1.0) as f32 * 0.5
            })
            .collect();
        let n = run(&b64(&make_wav(rate, 1, &noise)), "base64", &Options::default()).unwrap();
        let t = run(&b64(&tone_wav()), "base64", &Options::default()).unwrap();
        let noise_flat = col(&n, "flatness", 30);
        let tone_flat = col(&t, "flatness", 30);
        assert!(noise_flat > 0.2, "noise flatness = {noise_flat}");
        assert!(
            noise_flat > tone_flat * 20.0,
            "noise {noise_flat} should be far flatter than tone {tone_flat}"
        );
        // Broadband noise sits near the middle of the band.
        let c = col(&n, "centroid_hz", 30);
        assert!((2000.0..6000.0).contains(&c), "noise centroid = {c} Hz");
    }

    #[test]
    fn digital_silence_reports_the_floor_not_nan() {
        let wav = make_wav(16_000, 1, &vec![0.0f32; 16_000]);
        let out = run(
            &b64(&wav),
            "base64",
            &Options {
                bandwidth: true,
                flux: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(col(&out, "rms_dbfs", 5), -200.0);
        assert_eq!(col(&out, "centroid_hz", 5), 0.0);
        assert_eq!(col(&out, "zcr", 5), 0.0);
        assert_eq!(col(&out, "rolloff_hz", 5), 0.0);
        // librosa's amin floor makes a silent frame perfectly "flat".
        assert_eq!(col(&out, "flatness", 5), 1.0);
        assert_eq!(col(&out, "bandwidth_hz", 5), 0.0);
        assert_eq!(col(&out, "flux", 5), 0.0);
        assert!(!out.contains("NaN") && !out.contains("inf"), "{}", &out[..80]);
    }

    #[test]
    fn feature_toggles_pick_the_columns_and_their_order() {
        let out = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                rms: false,
                centroid: true,
                zcr: false,
                rolloff: false,
                flatness: false,
                bandwidth: true,
                flux: true,
                include_time: false,
                include_frame: true,
                ..Options::default()
            },
        )
        .unwrap();
        let r = rows(&out);
        assert_eq!(r[0], vec!["frame", "centroid_hz", "bandwidth_hz", "flux"]);
        assert_eq!(r[1][0], "0");
        assert_eq!(r[2][0], "1");
        // The first frame has no predecessor, so its flux is defined as 0.
        assert_eq!(col(&out, "flux", 1), 0.0);
    }

    #[test]
    fn scales_switch_the_column_names_and_values() {
        let db = run(&b64(&tone_wav()), "base64", &Options::default()).unwrap();
        let lin = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                rms_scale: "linear".into(),
                flatness_scale: "db".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(rows(&lin)[0].contains(&"rms".to_string()));
        assert!(rows(&lin)[0].contains(&"flatness_db".to_string()));
        let linear = col(&lin, "rms", 30);
        assert!((linear - 0.3536).abs() < 0.01, "linear rms = {linear}");
        // dBFS is 20*log10 of the same value.
        assert!((20.0 * linear.log10() - col(&db, "rms_dbfs", 30)).abs() < 0.01);
        // Flatness in dB is 10*log10 of the ratio, so a tone is strongly negative.
        assert!(col(&lin, "flatness_db", 30) < -20.0);
    }

    #[test]
    fn rolloff_percent_moves_the_cut_frequency() {
        let cut = |bytes: &[u8], percent: f64| {
            let out = run(
                &b64(bytes),
                "base64",
                &Options {
                    rolloff_percent: percent,
                    ..Options::default()
                },
            )
            .unwrap();
            col(&out, "rolloff_hz", 30)
        };

        // A Hann-windowed pure tone concentrates essentially all of its
        // magnitude in the main lobe around the partial — the sidelobes start
        // 31 dB down and fall off at 18 dB/octave — so raising the percentage
        // walks the cut up a few 31.25 Hz bins and never leaves the
        // neighbourhood of the 440 Hz partial.
        let wav = tone_wav();
        let (lo, hi) = (cut(&wav, 10.0), cut(&wav, 99.0));
        assert!(lo < hi, "10% cut {lo} Hz should sit below the 99% cut {hi} Hz");
        assert!(
            (375.0..=450.0).contains(&lo),
            "10% of a tone's magnitude is reached just below the partial: {lo} Hz"
        );
        assert!(
            (450.0..=700.0).contains(&hi),
            "even 99% stays within a few bins of the partial: {hi} Hz"
        );

        // Broadband noise is where the knob really bites: the same 10 → 99
        // sweep drags the cut across most of the 0-8 kHz band.
        let rate = 16_000u32;
        let mut state = 0x9E3779B9u32;
        let noise: Vec<f32> = (0..rate)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state as f64 / u32::MAX as f64 * 2.0 - 1.0) as f32 * 0.5
            })
            .collect();
        let nwav = make_wav(rate, 1, &noise);
        let (n10, n50, n95) = (cut(&nwav, 10.0), cut(&nwav, 50.0), cut(&nwav, 95.0));
        assert!(n10 < n50 && n50 < n95, "noise cuts {n10} < {n50} < {n95} Hz");
        assert!(n10 < 1500.0, "10% of flat noise sits low in the band: {n10} Hz");
        assert!(
            (3000.0..=5000.0).contains(&n50),
            "half of flat noise's magnitude sits near mid-band: {n50} Hz"
        );
        assert!(
            n95 > 7000.0,
            "95% of flat noise reaches close to Nyquist: {n95} Hz"
        );
    }

    #[test]
    fn center_adds_the_librosa_frame_count_and_a_quiet_first_frame() {
        let plain = run(&b64(&tone_wav()), "base64", &Options::default()).unwrap();
        let centred = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                center: true,
                ..Options::default()
            },
        )
        .unwrap();
        // librosa center=True → 1 + len//hop frames = 1 + 16000/160 = 101.
        assert_eq!(rows(&centred).len(), 102);
        assert_eq!(rows(&plain).len(), 99);
        // The first centred frame is half zero-padding, so it is ~6 dB quieter.
        let first = col(&centred, "rms_dbfs", 1);
        let mid = col(&centred, "rms_dbfs", 30);
        assert!(first < mid - 2.0, "{first} should be below {mid}");
    }

    #[test]
    fn json_reports_the_resolved_settings_alongside_the_matrix() {
        let out = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                output: "json".into(),
                bandwidth: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.contains("\"sample_rate\": 16000"), "{out:.400}");
        assert!(out.contains("\"source_channels\": 1"));
        assert!(out.contains("\"channel\": \"mix\""));
        assert!(out.contains("\"frames\": 98"));
        assert!(out.contains("\"frame_length_samples\": 400"));
        assert!(out.contains("\"hop_length_samples\": 160"));
        assert!(out.contains("\"fft_size\": 512"));
        assert!(out.contains("\"window\": \"hann\""));
        assert!(out.contains("\"center\": false"));
        assert!(out.contains("\"rolloff_percent\": 85.000"));
        assert!(out.contains("\"rms_scale\": \"dbfs\""));
        assert!(out.contains("\"samples_truncated\": false"));
        assert!(out.contains(
            "\"columns\": [\"time_s\", \"rms_dbfs\", \"centroid_hz\", \"zcr\", \"rolloff_hz\", \
             \"flatness\", \"bandwidth_hz\"]"
        ));
        assert!(out.contains("\"features\": ["));
    }

    #[test]
    fn tsv_decimals_and_hex_input_all_behave() {
        let wav = tone_wav();
        let out = run(
            &b64(&wav),
            "base64",
            &Options {
                output: "tsv".into(),
                decimals: 2,
                ..Options::default()
            },
        )
        .unwrap();
        let lines: Vec<&str> = out.trim_end().split('\n').collect();
        assert_eq!(lines[0], "time_s\trms_dbfs\tcentroid_hz\tzcr\trolloff_hz\tflatness");
        let cells: Vec<&str> = lines[1].split('\t').collect();
        assert_eq!(cells.len(), 6);
        // Timestamps keep at least 3 decimals; features honour `decimals`.
        assert_eq!(cells[0].split('.').nth(1).unwrap().len(), 3);
        assert_eq!(cells[1].split('.').nth(1).unwrap().len(), 2);

        // The same bytes as hex give byte-identical output.
        let a = run(&b64(&wav), "base64", &Options::default()).unwrap();
        let b = run(&hex(&wav), "hex", &Options::default()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn channel_selection_reads_the_requested_side() {
        // Left = a 440 Hz tone, right = digital silence.
        let rate = 16_000u32;
        let mut inter = Vec::new();
        for i in 0..rate {
            inter.push((2.0 * PI * 440.0 * i as f64 / rate as f64).sin() as f32 * 0.5);
            inter.push(0.0);
        }
        let wav = make_wav(rate, 2, &inter);
        let left = run(
            &b64(&wav),
            "base64",
            &Options {
                channel: "left".into(),
                ..Options::default()
            },
        )
        .unwrap();
        let right = run(
            &b64(&wav),
            "base64",
            &Options {
                channel: "right".into(),
                ..Options::default()
            },
        )
        .unwrap();
        let mix = run(&b64(&wav), "base64", &Options::default()).unwrap();
        assert!((col(&left, "rms_dbfs", 30) - -9.03).abs() < 0.1);
        assert_eq!(col(&right, "rms_dbfs", 30), -200.0);
        // The downmix halves the amplitude → 6 dB below the left channel alone.
        assert!((col(&mix, "rms_dbfs", 30) - col(&left, "rms_dbfs", 30) + 6.02).abs() < 0.1);
    }

    #[test]
    fn resampling_changes_the_reported_rate_and_frame_count() {
        // 0.5 s at 44.1 kHz downsampled to 16 kHz → 8000 samples → 48 frames.
        let rate = 44_100u32;
        let samples: Vec<f32> = (0..rate / 2)
            .map(|i| (2.0 * PI * 300.0 * i as f64 / rate as f64).sin() as f32 * 0.4)
            .collect();
        let out = run(
            &b64(&make_wav(rate, 1, &samples)),
            "base64",
            &Options {
                output: "json".into(),
                resample_hz: 16_000,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.contains("\"sample_rate\": 16000"), "{out:.400}");
        assert!(out.contains("\"source_sample_rate\": 44100"));
        assert!(out.contains("\"frames\": 48"));
    }

    #[test]
    fn flux_spikes_at_a_change_of_tone() {
        // Half a second of 440 Hz followed by half a second of 3 kHz.
        let rate = 16_000u32;
        let mut samples = Vec::new();
        for i in 0..rate / 2 {
            samples.push((2.0 * PI * 440.0 * i as f64 / rate as f64).sin() as f32 * 0.5);
        }
        for i in 0..rate / 2 {
            samples.push((2.0 * PI * 3000.0 * i as f64 / rate as f64).sin() as f32 * 0.5);
        }
        let out = run(
            &b64(&make_wav(rate, 1, &samples)),
            "base64",
            &Options {
                flux: true,
                ..Options::default()
            },
        )
        .unwrap();
        let steady = col(&out, "flux", 20);
        // The switch happens at 0.5 s = frame 50 (hop 160 samples).
        let jump = (48..=53)
            .map(|t| col(&out, "flux", t))
            .fold(0.0f64, f64::max);
        assert!(steady < 0.05, "steady flux = {steady}");
        assert!(jump > 0.3, "flux at the transition = {jump}");
    }

    // -- errors -------------------------------------------------------------

    #[test]
    fn empty_input_is_rejected() {
        let err = run("   ", "base64", &Options::default()).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn non_audio_bytes_are_rejected() {
        let err = run(
            &b64(b"not an audio file at all"),
            "base64",
            &Options::default(),
        )
        .unwrap_err();
        assert!(
            err.contains("unrecognized or unsupported media format"),
            "{err}"
        );
    }

    #[test]
    fn clip_shorter_than_one_frame_is_rejected() {
        // 100 samples at 16 kHz = 6.25 ms, shorter than the 25 ms frame.
        let wav = make_wav(16_000, 1, &vec![0.1f32; 100]);
        let err = run(&b64(&wav), "base64", &Options::default()).unwrap_err();
        assert!(err.contains("one analysis frame"), "{err}");
    }

    #[test]
    fn selecting_no_features_is_rejected() {
        let err = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                rms: false,
                centroid: false,
                zcr: false,
                rolloff: false,
                flatness: false,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("no features selected"), "{err}");
    }

    #[test]
    fn invalid_enums_and_ranges_are_rejected() {
        for (opts, needle) in [
            (
                Options {
                    output: "xml".into(),
                    ..Options::default()
                },
                "invalid output",
            ),
            (
                Options {
                    window: "gaussian".into(),
                    ..Options::default()
                },
                "invalid window",
            ),
            (
                Options {
                    channel: "middle".into(),
                    ..Options::default()
                },
                "invalid channel",
            ),
            (
                Options {
                    rms_scale: "lufs".into(),
                    ..Options::default()
                },
                "invalid rms_scale",
            ),
            (
                Options {
                    flatness_scale: "percent".into(),
                    ..Options::default()
                },
                "invalid flatness_scale",
            ),
            (
                Options {
                    frame_ms: 900.0,
                    ..Options::default()
                },
                "invalid frame_ms",
            ),
            (
                Options {
                    hop_ms: 0.0,
                    ..Options::default()
                },
                "invalid hop_ms",
            ),
            (
                Options {
                    rolloff_percent: 100.0,
                    ..Options::default()
                },
                "invalid rolloff_percent",
            ),
            (
                Options {
                    decimals: 12,
                    ..Options::default()
                },
                "invalid decimals",
            ),
            (
                Options {
                    resample_hz: 100,
                    ..Options::default()
                },
                "invalid resample_hz",
            ),
        ] {
            let err = run(&b64(&tone_wav()), "base64", &opts).unwrap_err();
            assert!(err.contains(needle), "expected {needle:?} in {err}");
        }
    }

    #[test]
    fn bad_encoding_is_reported_per_format() {
        let err = run("****", "base64", &Options::default()).unwrap_err();
        assert!(err.contains("invalid base64 character"), "{err}");
        let err = run("zz", "hex", &Options::default()).unwrap_err();
        assert!(err.contains("invalid hex digit"), "{err}");
        let err = run("aa", "rot13", &Options::default()).unwrap_err();
        assert!(err.contains("invalid input_format"), "{err}");
    }

    // -- DSP unit checks ----------------------------------------------------

    #[test]
    fn fft_matches_a_naive_dft() {
        let n = 32usize;
        let input: Vec<f64> = (0..n).map(|i| (i as f64 * 0.37).sin() + 0.2).collect();
        let (tr, ti) = twiddles(n);
        let mut re = input.clone();
        let mut im = vec![0.0; n];
        fft_in_place(&mut re, &mut im, &tr, &ti);
        for k in 0..n / 2 + 1 {
            let (mut dr, mut di) = (0.0f64, 0.0f64);
            for (t, x) in input.iter().enumerate() {
                let ang = -2.0 * PI * k as f64 * t as f64 / n as f64;
                dr += x * ang.cos();
                di += x * ang.sin();
            }
            assert!((re[k] - dr).abs() < 1e-9, "bin {k}: {} vs {dr}", re[k]);
            assert!((im[k] - di).abs() < 1e-9, "bin {k}: {} vs {di}", im[k]);
        }
    }

    #[test]
    fn a_dc_signal_has_a_zero_centroid_and_no_crossings() {
        let wav = make_wav(16_000, 1, &vec![0.5f32; 16_000]);

        // A rectangular 32 ms frame at 16 kHz is 512 samples — exactly the FFT
        // size, so there is no zero-padding and no window to smear the
        // transform. A DC signal then lands wholly in bin 0 and every spectral
        // column is exactly 0 Hz.
        let exact = run(
            &b64(&wav),
            "base64",
            &Options {
                bandwidth: true,
                frame_ms: 32.0,
                window: "rectangular".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(col(&exact, "centroid_hz", 30), 0.0);
        assert_eq!(col(&exact, "rolloff_hz", 30), 0.0);
        assert_eq!(col(&exact, "bandwidth_hz", 30), 0.0);
        assert_eq!(col(&exact, "zcr", 30), 0.0);
        assert!((col(&exact, "rms_dbfs", 30) - -6.02).abs() < 0.1);

        // Under the default framing the same DC signal is a 400-sample Hann
        // frame zero-padded to 512, so its transform is the window's main lobe
        // centred on DC. The one-sided spectrum keeps only the upper half of
        // that lobe, so the spectral columns sit just above 0 Hz rather than on
        // it — exactly what librosa reports for the same input. This is
        // leakage, not energy: it must stay inside the first couple of
        // 31.25 Hz bins.
        let bin = 16_000.0 / 512.0;
        let out = run(
            &b64(&wav),
            "base64",
            &Options {
                bandwidth: true,
                ..Options::default()
            },
        )
        .unwrap();
        let c = col(&out, "centroid_hz", 30);
        assert!(
            (0.0..bin).contains(&c),
            "centroid = {c} Hz, expected inside the first bin (< {bin} Hz)"
        );
        // 85% of the main lobe's magnitude is reached one bin above DC.
        assert_eq!(col(&out, "rolloff_hz", 30), bin);
        let bw = col(&out, "bandwidth_hz", 30);
        assert!(
            bw < 1.5 * bin,
            "bandwidth = {bw} Hz, expected under 1.5 bins ({} Hz)",
            1.5 * bin
        );
        assert_eq!(col(&out, "zcr", 30), 0.0);
        assert!((col(&out, "rms_dbfs", 30) - -6.02).abs() < 0.1);
    }
}
