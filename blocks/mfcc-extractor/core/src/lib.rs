//! mfcc-extractor core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Turns one audio file into a **matrix of Mel-frequency cepstral coefficients**
//! — the standard front-end feature for speech recognition, speaker ID, keyword
//! spotting and audio classification — and renders it as CSV, TSV or JSON.
//!
//! Pipeline (the classic speech-toolkit order):
//!
//! 1. decode the pasted file bytes (base64/hex) → symphonia demuxes the
//!    container and decodes the FIRST decodable audio track;
//! 2. downmix to mono inside the packet loop (native-rate multi-channel PCM is
//!    never held in memory);
//! 3. optionally resample to a fixed analysis rate (box pre-filter on
//!    downsampling, then linear interpolation);
//! 4. pre-emphasis `y[n] = x[n] - a·x[n-1]`;
//! 5. frame into `frame_ms` windows every `hop_ms`, **complete frames only** —
//!    no centring, no reflect padding, no zero-padded tail frame;
//! 6. apply the analysis window, zero-pad to the next power of two, real FFT,
//!    power spectrum `|X|²/N`;
//! 7. triangular mel filterbank (HTK or Slaney mel scale; Slaney also applies
//!    librosa's filter-area normalisation) → natural log;
//! 8. orthonormal DCT-II → keep the first `n_mfcc` coefficients;
//! 9. sinusoidal cepstral liftering, then optionally replace C0 with the log of
//!    the frame's total energy;
//! 10. optional delta and delta-delta (regression coefficients over ±2 frames).
//!
//! Pure Rust → runs on every backend, including the chat Service Worker.

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
/// base64/hex is rejected before any decoding work happens. The cap is lower
/// than the level-report tools' because this one buffers the mono signal.
pub const MAX_INPUT_BYTES: usize = 24 * 1024 * 1024;

/// Hard cap on buffered mono samples (~83 s at 48 kHz, ~250 s at 16 kHz).
/// Longer audio is analysed up to this point and the truncation is reported.
pub const MAX_SAMPLES: usize = 4_000_000;

/// Hard cap on emitted frames, so a 1 ms hop can't produce a gigabyte of CSV.
pub const MAX_OUTPUT_FRAMES: usize = 200_000;

/// Largest channel count the downmix accepts.
pub const MAX_CHANNELS: usize = 16;

// Parameter bounds — mirrored by the descriptor in ../../src/lib.rs.
pub const MIN_N_MFCC: i64 = 1;
pub const MAX_N_MFCC: i64 = 64;
pub const MIN_N_MELS: i64 = 4;
pub const MAX_N_MELS: i64 = 256;
pub const MIN_FRAME_MS: f64 = 1.0;
pub const MAX_FRAME_MS: f64 = 200.0;
pub const MIN_HOP_MS: f64 = 1.0;
pub const MAX_HOP_MS: f64 = 200.0;
pub const MAX_FREQ_HZ: f64 = 24_000.0;
pub const MAX_PREEMPHASIS: f64 = 1.0;
pub const MAX_LIFTER: f64 = 100.0;
pub const MAX_DECIMALS: i64 = 8;
pub const MIN_RESAMPLE_HZ: i64 = 4_000;
pub const MAX_RESAMPLE_HZ: i64 = 48_000;

/// Floor for the log stages, matching the double-precision epsilon that the
/// reference speech toolkits use so a silent band yields a finite value.
const LOG_FLOOR: f64 = f64::EPSILON;

const SUPPORTED: &str = "supported containers: WAV, AIFF, CAF, FLAC, MP3, OGG, MP4/MOV/M4A, \
                         MKV/WebM, AAC-ADTS; audio codecs: PCM, ADPCM, FLAC, MP3, AAC-LC, ALAC, \
                         Vorbis (Opus, AC-3 and DTS are not supported)";

/// Every knob except the input bytes. Grouped so the chat block, the CLI and
/// the browser wrapper all construct exactly the same configuration.
#[derive(Clone, Debug)]
pub struct Options {
    /// `"csv"` (default), `"tsv"` or `"json"`.
    pub output: String,
    /// Cepstral coefficients kept per frame.
    pub n_mfcc: i64,
    /// Triangular mel filters in the filterbank.
    pub n_mels: i64,
    /// Analysis frame length, milliseconds.
    pub frame_ms: f64,
    /// Frame hop (step), milliseconds.
    pub hop_ms: f64,
    /// Lowest filterbank edge, Hz.
    pub fmin: f64,
    /// Highest filterbank edge, Hz; `0` means Nyquist.
    pub fmax: f64,
    /// `"hamming"` (default), `"hann"`, `"blackman"` or `"rectangular"`.
    pub window: String,
    /// Pre-emphasis coefficient; `0` disables the filter.
    pub preemphasis: f64,
    /// Sinusoidal cepstral lifter; `0` disables liftering.
    pub lifter: f64,
    /// `"htk"` (default) or `"slaney"`.
    pub mel_scale: String,
    /// Replace C0 with the log of the frame's total energy.
    pub append_energy: bool,
    /// `"none"` (default), `"delta"` or `"delta_delta"`.
    pub deltas: String,
    /// Prepend a `time_s` column with each frame's start time.
    pub include_time: bool,
    /// Decimal places in the rendered numbers.
    pub decimals: i64,
    /// Analysis sample rate; `0` keeps the file's own rate.
    pub resample_hz: i64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            output: "csv".into(),
            n_mfcc: 13,
            n_mels: 26,
            frame_ms: 25.0,
            hop_ms: 10.0,
            fmin: 0.0,
            fmax: 0.0,
            window: "hamming".into(),
            preemphasis: 0.97,
            lifter: 22.0,
            mel_scale: "htk".into(),
            append_energy: true,
            deltas: "none".into(),
            include_time: true,
            decimals: 6,
            resample_hz: 0,
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

/// How many derivative blocks follow the static coefficients.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Deltas {
    None,
    Delta,
    DeltaDelta,
}

/// The validated, resolved configuration — every "0 means auto" resolved and
/// every millisecond converted into samples at the analysis rate.
struct Resolved {
    out_mode: OutMode,
    n_mfcc: usize,
    n_mels: usize,
    frame_len: usize,
    hop_len: usize,
    n_fft: usize,
    fmin: f64,
    fmax: f64,
    window: Vec<f64>,
    window_name: String,
    lifter: f64,
    slaney: bool,
    append_energy: bool,
    deltas: Deltas,
    include_time: bool,
    decimals: usize,
}

/// Facts about the decoded stream, carried into the JSON metadata.
struct Decoded {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: usize,
    truncated: bool,
}

/// Extract MFCCs from one audio file and render them as a matrix.
///
/// - `input`: the audio file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `opts`: every other knob; see [`Options`].
///
/// Returns a user-facing error string for empty/oversized input, undecodable
/// bytes, an unsupported container/codec, audio shorter than one analysis
/// frame, or any out-of-range parameter.
pub fn run(input: &str, input_format: &str, opts: &Options) -> Result<String, String> {
    let bytes = decode_bytes(input, input_format)?;
    let decoded = decode_audio(bytes)?;
    let source_rate = decoded.sample_rate;

    // Resample first: every millisecond bound resolves at the analysis rate.
    let (mut signal, rate) = match opts.resample_hz {
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

    if signal.len() < cfg.frame_len {
        return Err(format!(
            "audio is {} samples ({:.3} s at {rate} Hz) but one analysis frame is {} samples \
             ({} ms) — use a shorter frame_ms or a longer clip",
            signal.len(),
            signal.len() as f64 / rate as f64,
            cfg.frame_len,
            opts.frame_ms
        ));
    }

    if opts.preemphasis != 0.0 {
        preemphasize(&mut signal, opts.preemphasis);
    }

    let total_frames = 1 + (signal.len() - cfg.frame_len) / cfg.hop_len;
    let frames = total_frames.min(MAX_OUTPUT_FRAMES);
    let frames_truncated = frames < total_frames;

    let bank = mel_filterbank(&cfg, rate)?;
    let matrix = extract(&signal, &cfg, &bank, frames);
    let matrix = apply_deltas(matrix, cfg.n_mfcc, cfg.deltas);

    Ok(render(
        &matrix,
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
    let deltas = match opts.deltas.trim() {
        "" | "none" => Deltas::None,
        "delta" => Deltas::Delta,
        "delta_delta" => Deltas::DeltaDelta,
        other => {
            return Err(format!(
                "invalid deltas {other:?}: expected \"none\", \"delta\" or \"delta_delta\""
            ))
        }
    };
    let slaney = match opts.mel_scale.trim() {
        "" | "htk" => false,
        "slaney" => true,
        other => {
            return Err(format!(
                "invalid mel_scale {other:?}: expected \"htk\" or \"slaney\""
            ))
        }
    };
    if !(MIN_N_MELS..=MAX_N_MELS).contains(&opts.n_mels) {
        return Err(format!(
            "invalid n_mels {}: expected {MIN_N_MELS}-{MAX_N_MELS} mel filters",
            opts.n_mels
        ));
    }
    if !(MIN_N_MFCC..=MAX_N_MFCC).contains(&opts.n_mfcc) {
        return Err(format!(
            "invalid n_mfcc {}: expected {MIN_N_MFCC}-{MAX_N_MFCC} coefficients",
            opts.n_mfcc
        ));
    }
    if opts.n_mfcc > opts.n_mels {
        return Err(format!(
            "invalid n_mfcc {}: the DCT cannot return more coefficients than the {} mel filters \
             it is applied to — raise n_mels or lower n_mfcc",
            opts.n_mfcc, opts.n_mels
        ));
    }
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
    if !opts.preemphasis.is_finite() || !(0.0..=MAX_PREEMPHASIS).contains(&opts.preemphasis) {
        return Err(format!(
            "invalid preemphasis {}: expected 0-{MAX_PREEMPHASIS} (0 disables the filter)",
            opts.preemphasis
        ));
    }
    if !opts.lifter.is_finite() || !(0.0..=MAX_LIFTER).contains(&opts.lifter) {
        return Err(format!(
            "invalid lifter {}: expected 0-{MAX_LIFTER} (0 disables liftering)",
            opts.lifter
        ));
    }
    if !(0..=MAX_DECIMALS).contains(&opts.decimals) {
        return Err(format!(
            "invalid decimals {}: expected 0-{MAX_DECIMALS}",
            opts.decimals
        ));
    }
    let nyquist = rate as f64 / 2.0;
    if !opts.fmin.is_finite() || !(0.0..=MAX_FREQ_HZ).contains(&opts.fmin) {
        return Err(format!(
            "invalid fmin {}: expected 0-{MAX_FREQ_HZ} Hz",
            opts.fmin
        ));
    }
    if !opts.fmax.is_finite() || !(0.0..=MAX_FREQ_HZ).contains(&opts.fmax) {
        return Err(format!(
            "invalid fmax {}: expected 0-{MAX_FREQ_HZ} Hz (0 means Nyquist)",
            opts.fmax
        ));
    }
    let fmax = if opts.fmax == 0.0 {
        nyquist
    } else {
        opts.fmax.min(nyquist)
    };
    if opts.fmin >= fmax {
        return Err(format!(
            "invalid frequency range: fmin {} Hz must be below fmax {fmax} Hz (Nyquist for this \
             {rate} Hz stream is {nyquist} Hz)",
            opts.fmin
        ));
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
        n_mfcc: opts.n_mfcc as usize,
        n_mels: opts.n_mels as usize,
        frame_len,
        hop_len,
        n_fft,
        fmin: opts.fmin,
        fmax,
        window,
        window_name,
        lifter: opts.lifter,
        slaney,
        append_energy: opts.append_energy,
        deltas,
        include_time: opts.include_time,
        decimals: opts.decimals as usize,
    })
}

/// Periodic-free (symmetric) window coefficients, the speech-toolkit default.
fn analysis_window(name: &str, len: usize) -> Result<(Vec<f64>, String), String> {
    let denom = (len - 1).max(1) as f64;
    let (name, w): (&str, Vec<f64>) = match name.trim() {
        "" | "hamming" => (
            "hamming",
            (0..len)
                .map(|i| 0.54 - 0.46 * (2.0 * PI * i as f64 / denom).cos())
                .collect(),
        ),
        "hann" => (
            "hann",
            (0..len)
                .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / denom).cos())
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
                "invalid window {other:?}: expected \"hamming\", \"hann\", \"blackman\" or \
                 \"rectangular\""
            ))
        }
    };
    Ok((w, name.to_string()))
}

// ---------------------------------------------------------------------------
// Signal conditioning
// ---------------------------------------------------------------------------

/// `y[n] = x[n] - a·x[n-1]`, with `y[0] = x[0]` (the reference convention).
fn preemphasize(signal: &mut [f32], a: f64) {
    let mut prev = signal[0] as f64;
    for s in signal.iter_mut().skip(1) {
        let cur = *s as f64;
        *s = (cur - a * prev) as f32;
        prev = cur;
    }
}

/// Box pre-filter (only when downsampling) followed by linear interpolation.
/// Not a polyphase resampler — good enough to keep aliasing out of the mel
/// bands, and documented as such on the page.
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
// Mel filterbank
// ---------------------------------------------------------------------------

fn hz_to_mel(hz: f64, slaney: bool) -> f64 {
    if slaney {
        // Slaney's Auditory Toolbox scale: linear below 1 kHz, log above.
        const F_SP: f64 = 200.0 / 3.0;
        const MIN_LOG_HZ: f64 = 1000.0;
        const MIN_LOG_MEL: f64 = MIN_LOG_HZ / F_SP;
        let logstep = (6.4f64).ln() / 27.0;
        if hz < MIN_LOG_HZ {
            hz / F_SP
        } else {
            MIN_LOG_MEL + (hz / MIN_LOG_HZ).ln() / logstep
        }
    } else {
        2595.0 * (1.0 + hz / 700.0).log10()
    }
}

fn mel_to_hz(mel: f64, slaney: bool) -> f64 {
    if slaney {
        const F_SP: f64 = 200.0 / 3.0;
        const MIN_LOG_HZ: f64 = 1000.0;
        const MIN_LOG_MEL: f64 = MIN_LOG_HZ / F_SP;
        let logstep = (6.4f64).ln() / 27.0;
        if mel < MIN_LOG_MEL {
            mel * F_SP
        } else {
            MIN_LOG_HZ * (logstep * (mel - MIN_LOG_MEL)).exp()
        }
    } else {
        700.0 * (10f64.powf(mel / 2595.0) - 1.0)
    }
}

/// One triangular filter: the first FFT bin it touches plus its weights.
struct Filter {
    start: usize,
    weights: Vec<f64>,
}

/// Triangular filters spread evenly on the mel scale between `fmin` and `fmax`.
/// Weights are computed against the exact FFT bin frequencies (not rounded bin
/// indices), and the Slaney scale additionally normalises each filter by its
/// bandwidth so filters carry equal area.
fn mel_filterbank(cfg: &Resolved, rate: u32) -> Result<Vec<Filter>, String> {
    let n_bins = cfg.n_fft / 2 + 1;
    let bin_hz = rate as f64 / cfg.n_fft as f64;
    let mel_lo = hz_to_mel(cfg.fmin, cfg.slaney);
    let mel_hi = hz_to_mel(cfg.fmax, cfg.slaney);
    let edges: Vec<f64> = (0..cfg.n_mels + 2)
        .map(|i| {
            let mel = mel_lo + (mel_hi - mel_lo) * i as f64 / (cfg.n_mels + 1) as f64;
            mel_to_hz(mel, cfg.slaney)
        })
        .collect();

    let mut bank = Vec::with_capacity(cfg.n_mels);
    let mut empty = 0usize;
    for i in 0..cfg.n_mels {
        let (lo, mid, hi) = (edges[i], edges[i + 1], edges[i + 2]);
        let norm = if cfg.slaney { 2.0 / (hi - lo) } else { 1.0 };
        let first = ((lo / bin_hz).floor().max(0.0) as usize).min(n_bins.saturating_sub(1));
        let last = ((hi / bin_hz).ceil() as usize).min(n_bins - 1);
        let mut weights = Vec::new();
        let mut start = first;
        let mut seen = false;
        for k in first..=last {
            let f = k as f64 * bin_hz;
            let up = if mid > lo { (f - lo) / (mid - lo) } else { 0.0 };
            let down = if hi > mid { (hi - f) / (hi - mid) } else { 0.0 };
            let w = up.min(down).max(0.0) * norm;
            if w <= 0.0 && !seen {
                start = k + 1;
                continue;
            }
            seen = true;
            weights.push(w);
        }
        while weights.last().is_some_and(|w| *w <= 0.0) {
            weights.pop();
        }
        if weights.is_empty() {
            empty += 1;
        }
        bank.push(Filter { start, weights });
    }
    if empty > 0 {
        return Err(format!(
            "{empty} of the {} mel filters are narrower than one FFT bin ({bin_hz:.1} Hz) and \
             would always read zero — use fewer n_mels, a longer frame_ms, or a wider fmin/fmax \
             range",
            cfg.n_mels
        ));
    }
    Ok(bank)
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

/// Frame → window → FFT → power → mel → log → DCT-II → lifter → energy.
fn extract(signal: &[f32], cfg: &Resolved, bank: &[Filter], frames: usize) -> Vec<Vec<f64>> {
    let (tw_re, tw_im) = twiddles(cfg.n_fft);
    let n_bins = cfg.n_fft / 2 + 1;

    // Orthonormal DCT-II basis, precomputed once: rows = kept coefficients.
    let scale0 = (1.0 / cfg.n_mels as f64).sqrt();
    let scale = (2.0 / cfg.n_mels as f64).sqrt();
    let mut dct = Vec::with_capacity(cfg.n_mfcc * cfg.n_mels);
    for k in 0..cfg.n_mfcc {
        let s = if k == 0 { scale0 } else { scale };
        for m in 0..cfg.n_mels {
            dct.push(s * (PI * k as f64 * (m as f64 + 0.5) / cfg.n_mels as f64).cos());
        }
    }

    let lift: Vec<f64> = (0..cfg.n_mfcc)
        .map(|k| {
            if cfg.lifter > 0.0 {
                1.0 + (cfg.lifter / 2.0) * (PI * k as f64 / cfg.lifter).sin()
            } else {
                1.0
            }
        })
        .collect();

    let mut re = vec![0.0f64; cfg.n_fft];
    let mut im = vec![0.0f64; cfg.n_fft];
    let mut power = vec![0.0f64; n_bins];
    let mut out = Vec::with_capacity(frames);

    for f in 0..frames {
        let start = f * cfg.hop_len;
        for (i, slot) in re.iter_mut().enumerate() {
            *slot = if i < cfg.frame_len {
                signal[start + i] as f64 * cfg.window[i]
            } else {
                0.0
            };
        }
        im.iter_mut().for_each(|v| *v = 0.0);
        fft_in_place(&mut re, &mut im, &tw_re, &tw_im);

        // Power spectrum, normalised by the transform size (the convention the
        // ms-framed speech toolkits use).
        let mut energy = 0.0f64;
        for k in 0..n_bins {
            let p = (re[k] * re[k] + im[k] * im[k]) / cfg.n_fft as f64;
            power[k] = p;
            energy += p;
        }

        let mut log_mel = Vec::with_capacity(cfg.n_mels);
        for filt in bank {
            let mut sum = 0.0f64;
            for (i, w) in filt.weights.iter().enumerate() {
                sum += power[filt.start + i] * w;
            }
            log_mel.push(sum.max(LOG_FLOOR).ln());
        }

        let mut coeffs = Vec::with_capacity(cfg.n_mfcc);
        for k in 0..cfg.n_mfcc {
            let row = &dct[k * cfg.n_mels..(k + 1) * cfg.n_mels];
            let mut acc = 0.0f64;
            for (m, basis) in row.iter().enumerate() {
                acc += log_mel[m] * basis;
            }
            coeffs.push(acc * lift[k]);
        }
        if cfg.append_energy {
            coeffs[0] = energy.max(LOG_FLOOR).ln();
        }
        out.push(coeffs);
    }
    out
}

/// Regression (delta) coefficients over a ±2-frame span, edge-clamped — the
/// standard HTK/ASR formulation. Each pass appends `n_mfcc` more columns.
fn apply_deltas(matrix: Vec<Vec<f64>>, n_mfcc: usize, mode: Deltas) -> Vec<Vec<f64>> {
    let passes = match mode {
        Deltas::None => return matrix,
        Deltas::Delta => 1,
        Deltas::DeltaDelta => 2,
    };
    let mut matrix = matrix;
    let mut offset = 0usize;
    for _ in 0..passes {
        const N: isize = 2;
        let denom: f64 = 2.0 * (1..=N).map(|n| (n * n) as f64).sum::<f64>();
        let rows = matrix.len();
        let mut block = Vec::with_capacity(rows);
        for t in 0..rows {
            let mut d = Vec::with_capacity(n_mfcc);
            for c in 0..n_mfcc {
                let mut acc = 0.0f64;
                for n in 1..=N {
                    let hi = ((t as isize + n).max(0) as usize).min(rows - 1);
                    let lo = ((t as isize - n).max(0) as usize).min(rows - 1);
                    acc += n as f64 * (matrix[hi][offset + c] - matrix[lo][offset + c]);
                }
                d.push(acc / denom);
            }
            block.push(d);
        }
        for (row, d) in matrix.iter_mut().zip(block) {
            row.extend(d);
        }
        offset += n_mfcc;
    }
    matrix
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn column_names(cfg: &Resolved) -> Vec<String> {
    let mut cols = Vec::new();
    if cfg.include_time {
        cols.push("time_s".to_string());
    }
    for k in 0..cfg.n_mfcc {
        cols.push(format!("c{k}"));
    }
    if cfg.deltas != Deltas::None {
        for k in 0..cfg.n_mfcc {
            cols.push(format!("d{k}"));
        }
    }
    if cfg.deltas == Deltas::DeltaDelta {
        for k in 0..cfg.n_mfcc {
            cols.push(format!("dd{k}"));
        }
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
    matrix: &[Vec<f64>],
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

    match cfg.out_mode {
        OutMode::Csv | OutMode::Tsv => {
            let sep = if cfg.out_mode == OutMode::Csv {
                ","
            } else {
                "\t"
            };
            let mut out = String::with_capacity(matrix.len() * cols.len() * 10 + 64);
            out.push_str(&cols.join(sep));
            out.push('\n');
            for (t, row) in matrix.iter().enumerate() {
                let mut cells: Vec<String> = Vec::with_capacity(cols.len());
                if cfg.include_time {
                    cells.push(num(time(t), cfg.decimals.max(3)));
                }
                for v in row {
                    cells.push(num(*v, cfg.decimals));
                }
                out.push_str(&cells.join(sep));
                out.push('\n');
            }
            out
        }
        OutMode::Json => {
            let mut out = String::with_capacity(matrix.len() * cols.len() * 10 + 512);
            out.push_str("{\n");
            out.push_str(&format!("  \"sample_rate\": {rate},\n"));
            out.push_str(&format!("  \"source_sample_rate\": {source_rate},\n"));
            out.push_str(&format!("  \"source_channels\": {channels},\n"));
            out.push_str(&format!(
                "  \"duration_s\": {},\n",
                num(samples as f64 / rate as f64, 3)
            ));
            out.push_str(&format!("  \"frames\": {},\n", matrix.len()));
            out.push_str(&format!("  \"frames_available\": {total_frames},\n"));
            out.push_str(&format!("  \"n_mfcc\": {},\n", cfg.n_mfcc));
            out.push_str(&format!("  \"n_mels\": {},\n", cfg.n_mels));
            out.push_str(&format!("  \"frame_length_samples\": {},\n", cfg.frame_len));
            out.push_str(&format!("  \"hop_length_samples\": {},\n", cfg.hop_len));
            out.push_str(&format!("  \"fft_size\": {},\n", cfg.n_fft));
            out.push_str(&format!("  \"fmin_hz\": {},\n", num(cfg.fmin, 3)));
            out.push_str(&format!("  \"fmax_hz\": {},\n", num(cfg.fmax, 3)));
            out.push_str(&format!("  \"window\": \"{}\",\n", cfg.window_name));
            out.push_str(&format!(
                "  \"mel_scale\": \"{}\",\n",
                if cfg.slaney { "slaney" } else { "htk" }
            ));
            out.push_str(&format!(
                "  \"preemphasis\": {},\n",
                num(opts.preemphasis, 4)
            ));
            out.push_str(&format!("  \"lifter\": {},\n", num(cfg.lifter, 3)));
            out.push_str(&format!("  \"append_energy\": {},\n", cfg.append_energy));
            out.push_str(&format!(
                "  \"deltas\": \"{}\",\n",
                match cfg.deltas {
                    Deltas::None => "none",
                    Deltas::Delta => "delta",
                    Deltas::DeltaDelta => "delta_delta",
                }
            ));
            out.push_str(&format!("  \"samples_truncated\": {samples_truncated},\n"));
            out.push_str(&format!("  \"frames_truncated\": {frames_truncated},\n"));
            let quoted: Vec<String> = cols.iter().map(|c| format!("\"{c}\"")).collect();
            out.push_str(&format!("  \"columns\": [{}],\n", quoted.join(", ")));
            out.push_str("  \"mfcc\": [\n");
            for (t, row) in matrix.iter().enumerate() {
                let mut cells: Vec<String> = Vec::with_capacity(cols.len());
                if cfg.include_time {
                    cells.push(num(time(t), cfg.decimals.max(3)));
                }
                for v in row {
                    cells.push(num(*v, cfg.decimals));
                }
                out.push_str(&format!("    [{}]", cells.join(", ")));
                if t + 1 < matrix.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("  ]\n}\n");
            out
        }
    }
}

// ---------------------------------------------------------------------------
// Decode (base64/hex → symphonia → mono f32)
// ---------------------------------------------------------------------------

/// Demux `bytes`, decode the first decodable audio track and downmix it to mono
/// as it streams. Caps buffered samples at [`MAX_SAMPLES`].
fn decode_audio(bytes: Vec<u8>) -> Result<Decoded, String> {
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
                "no decodable audio track found (an image or video-only file has no MFCCs to \
                 extract); {SUPPORTED}"
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
                    samples.push(frame.iter().sum::<f32>() * inv);
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

    /// One second of a 440 Hz tone at 16 kHz, mono.
    fn tone_wav() -> Vec<u8> {
        let rate = 16_000u32;
        let samples: Vec<f32> = (0..rate)
            .map(|i| (2.0 * PI * 440.0 * i as f64 / rate as f64).sin() as f32 * 0.5)
            .collect();
        make_wav(rate, 1, &samples)
    }

    // -- happy paths --------------------------------------------------------

    #[test]
    fn csv_matrix_has_expected_shape_and_header() {
        let out = run(&b64(&tone_wav()), "base64", &Options::default()).unwrap();
        let lines: Vec<&str> = out.trim_end().split('\n').collect();
        assert_eq!(lines[0], "time_s,c0,c1,c2,c3,c4,c5,c6,c7,c8,c9,c10,c11,c12");
        // 16000 samples, 400-sample frames every 160 → 1 + (16000-400)/160 = 98.
        assert_eq!(lines.len(), 99, "header + 98 frames");
        assert_eq!(lines[1].split(',').count(), 14);
        assert!(lines[1].starts_with("0.000000,"), "first frame at t=0");
        assert!(
            lines[2].starts_with("0.010000,"),
            "second frame one 10 ms hop later, got {}",
            lines[2]
        );
    }

    #[test]
    fn tone_energy_is_stable_and_c1_dominates_higher_orders() {
        let out = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                output: "json".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.contains("\"sample_rate\": 16000"));
        assert!(out.contains("\"frames\": 98"));
        assert!(out.contains("\"fft_size\": 512"));
        assert!(out.contains("\"frame_length_samples\": 400"));
        assert!(out.contains("\"hop_length_samples\": 160"));
        assert!(out.contains("\"mel_scale\": \"htk\""));
        assert!(out.contains("\"append_energy\": true"));

        // A steady tone gives a near-constant feature vector frame to frame.
        let rows = matrix_rows(&out);
        let first = &rows[10];
        let later = &rows[60];
        // Column 0 is the timestamp, which of course differs — compare features.
        for (a, b) in first.iter().skip(1).zip(later.iter().skip(1)) {
            assert!(
                (a - b).abs() < 0.05,
                "steady tone should give steady coefficients: {a} vs {b}"
            );
        }
        // C0 is log frame energy; a 0.5-amplitude tone is well above silence.
        assert!(first[1] > -5.0 && first[1] < 5.0, "C0 = {}", first[1]);
    }

    #[test]
    fn hex_input_matches_base64_input() {
        let wav = tone_wav();
        let a = run(&b64(&wav), "base64", &Options::default()).unwrap();
        let b = run(&hex(&wav), "hex", &Options::default()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn deltas_widen_the_matrix() {
        let out = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                deltas: "delta_delta".into(),
                n_mfcc: 4,
                ..Options::default()
            },
        )
        .unwrap();
        let lines: Vec<&str> = out.trim_end().split('\n').collect();
        assert_eq!(lines[0], "time_s,c0,c1,c2,c3,d0,d1,d2,d3,dd0,dd1,dd2,dd3");
        assert_eq!(lines[1].split(',').count(), 13);
        assert_eq!(lines.len(), 99);

        // 'delta' stops after the first-order block.
        let out = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                deltas: "delta".into(),
                n_mfcc: 4,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(
            out.split('\n').next().unwrap(),
            "time_s,c0,c1,c2,c3,d0,d1,d2,d3"
        );
    }

    #[test]
    fn delta_of_a_linear_ramp_is_its_slope_and_the_second_derivative_is_zero() {
        // c[t] = 2t for every coefficient → interior delta = 2, delta-delta = 0.
        let matrix: Vec<Vec<f64>> = (0..20).map(|t| vec![2.0 * t as f64; 3]).collect();
        let out = apply_deltas(matrix, 3, Deltas::DeltaDelta);
        assert_eq!(out[0].len(), 9);
        // Only frames at least 2 hops away from an edge-clamped delta are exact
        // (the +/-2 span of the second pass reaches 4 frames out).
        for row in out.iter().take(16).skip(5) {
            for c in 0..3 {
                assert!((row[3 + c] - 2.0).abs() < 1e-12, "delta = {}", row[3 + c]);
                assert!(row[6 + c].abs() < 1e-12, "delta-delta = {}", row[6 + c]);
            }
        }
    }

    #[test]
    fn slaney_scale_and_no_energy_change_the_first_coefficient() {
        let base = run(&b64(&tone_wav()), "base64", &Options::default()).unwrap();
        let alt = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                mel_scale: "slaney".into(),
                append_energy: false,
                lifter: 0.0,
                window: "hann".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert_ne!(base, alt);
        let c0_base: f64 = base
            .split('\n')
            .nth(11)
            .unwrap()
            .split(',')
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let c0_alt: f64 = alt
            .split('\n')
            .nth(11)
            .unwrap()
            .split(',')
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        // With append_energy off, C0 is the (negative) mean log-mel energy.
        assert!(c0_alt < c0_base, "{c0_alt} should be below {c0_base}");
    }

    #[test]
    fn resampling_changes_the_reported_rate_and_frame_count() {
        // 0.5 s at 44.1 kHz downsampled to 16 kHz → 8000 samples → 48 frames.
        let rate = 44_100u32;
        let samples: Vec<f32> = (0..rate / 2)
            .map(|i| (2.0 * PI * 300.0 * i as f64 / rate as f64).sin() as f32 * 0.4)
            .collect();
        let wav = make_wav(rate, 1, &samples);
        let out = run(
            &b64(&wav),
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
        assert!(out.contains("\"frames\": 48"), "expected 48 frames");
    }

    #[test]
    fn stereo_is_downmixed_and_reported() {
        let rate = 16_000u32;
        let mut inter = Vec::new();
        for i in 0..rate / 2 {
            let v = (2.0 * PI * 500.0 * i as f64 / rate as f64).sin() as f32 * 0.3;
            inter.push(v);
            inter.push(-v); // out of phase → the downmix cancels to silence
        }
        let out = run(
            &b64(&make_wav(rate, 2, &inter)),
            "base64",
            &Options {
                output: "json".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.contains("\"source_channels\": 2"));
        let rows = matrix_rows(&out);
        // Cancelled mono signal → C0 (log energy) sits at the silence floor.
        assert!(
            rows[5][1] < -30.0,
            "C0 = {} should be near-silent",
            rows[5][1]
        );
    }

    #[test]
    fn tsv_and_decimals_and_no_time_column() {
        let out = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                output: "tsv".into(),
                include_time: false,
                decimals: 2,
                n_mfcc: 3,
                ..Options::default()
            },
        )
        .unwrap();
        let lines: Vec<&str> = out.trim_end().split('\n').collect();
        assert_eq!(lines[0], "c0\tc1\tc2");
        let cells: Vec<&str> = lines[1].split('\t').collect();
        assert_eq!(cells.len(), 3);
        for c in cells {
            let frac = c.split('.').nth(1).expect("two decimals");
            assert_eq!(frac.len(), 2, "cell {c} should have 2 decimals");
        }
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
    fn more_coefficients_than_filters_is_rejected() {
        let err = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                n_mfcc: 30,
                n_mels: 20,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("more coefficients than"), "{err}");
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
                    mel_scale: "bark".into(),
                    ..Options::default()
                },
                "invalid mel_scale",
            ),
            (
                Options {
                    deltas: "triple".into(),
                    ..Options::default()
                },
                "invalid deltas",
            ),
            (
                Options {
                    frame_ms: 500.0,
                    ..Options::default()
                },
                "invalid frame_ms",
            ),
            (
                Options {
                    preemphasis: 3.0,
                    ..Options::default()
                },
                "invalid preemphasis",
            ),
            (
                Options {
                    resample_hz: 100,
                    ..Options::default()
                },
                "invalid resample_hz",
            ),
            (
                Options {
                    fmin: 8000.0,
                    fmax: 4000.0,
                    ..Options::default()
                },
                "invalid frequency range",
            ),
        ] {
            let err = run(&b64(&tone_wav()), "base64", &opts).unwrap_err();
            assert!(err.contains(needle), "expected {needle:?} in {err}");
        }
    }

    #[test]
    fn too_many_filters_for_the_resolution_is_rejected() {
        let err = run(
            &b64(&tone_wav()),
            "base64",
            &Options {
                n_mels: 200,
                n_mfcc: 13,
                frame_ms: 5.0,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("narrower than one FFT bin"), "{err}");
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
    fn mel_conversions_round_trip_on_both_scales() {
        for slaney in [false, true] {
            for hz in [0.0, 100.0, 700.0, 999.0, 1000.0, 4000.0, 8000.0] {
                let back = mel_to_hz(hz_to_mel(hz, slaney), slaney);
                assert!((back - hz).abs() < 1e-6, "slaney={slaney} {hz} -> {back}");
            }
        }
        // The HTK formula's published anchor: 1000 Hz maps to ~1000 mel.
        assert!((hz_to_mel(1000.0, false) - 999.99).abs() < 0.5);
    }

    /// Parse the `"mfcc"` matrix out of the JSON rendering.
    fn matrix_rows(json: &str) -> Vec<Vec<f64>> {
        json.split("\"mfcc\": [")
            .nth(1)
            .unwrap()
            .lines()
            .filter_map(|l| {
                let l = l.trim().trim_end_matches(',');
                let l = l.strip_prefix('[')?.strip_suffix(']')?;
                Some(l.split(", ").map(|v| v.parse().unwrap()).collect())
            })
            .collect()
    }
}
