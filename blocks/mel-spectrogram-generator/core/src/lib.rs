//! mel-spectrogram-generator core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Turns one audio file into a **mel-scaled spectrogram PNG** — the perceptual
//! time/frequency view used for speech and music work, and the exact front end
//! most audio ML models (speech recognition, keyword spotting, audio tagging)
//! consume.
//!
//! Pipeline (the librosa/torchaudio order):
//!
//! 1. decode the pasted file bytes (base64/hex) → symphonia demuxes the
//!    container and decodes the FIRST decodable audio track;
//! 2. pick one channel or downmix to mono inside the packet loop (native-rate
//!    multi-channel PCM is never held in memory);
//! 3. optionally resample to a fixed analysis rate (box pre-filter on
//!    downsampling, then linear interpolation);
//! 4. optionally reflect-pad by `n_fft / 2` so frame `t` is CENTRED on sample
//!    `t · hop` (librosa's `center=True` default);
//! 5. frame into `n_fft` windows every `hop_length` samples, apply the analysis
//!    window, complex FFT (rustfft), one-sided window-normalised power spectrum
//!    (a full-scale sine reads ≈ 0 dBFS);
//! 6. triangular mel filterbank (Slaney or HTK mel scale; Slaney also applies
//!    librosa's equal-area filter normalisation);
//! 7. map power → dB (relative to the clip's peak or to full scale), or keep it
//!    linear as power/magnitude, and clamp to a `top_db` dynamic range;
//! 8. box-resample the frames × bands matrix to the requested pixel size, apply
//!    a colormap, encode a PNG with low frequencies at the BOTTOM.
//!
//! Pure Rust → runs on every backend, including the chat Service Worker.

use std::io::Cursor;

use image::ImageEncoder;
use rustfft::{num_complex::Complex, FftPlanner};
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

/// Hard cap on buffered samples (~83 s at 48 kHz, ~250 s at 16 kHz). Longer
/// audio is analysed up to this point and the truncation is reported.
pub const MAX_SAMPLES: usize = 4_000_000;

/// Hard cap on analysed STFT frames (spectrogram columns).
pub const MAX_FRAMES: usize = 12_000;

/// Hard cap on total FFT work (`frames × n_fft`), so a 1-sample hop with a
/// large `n_fft` can't wedge the browser tab.
pub const MAX_FFT_WORK: usize = 24_000_000;

/// Largest channel count the decoder accepts.
pub const MAX_CHANNELS: usize = 16;

// Parameter bounds — mirrored by the descriptor in ../../src/lib.rs.
pub const MIN_N_FFT: i64 = 64;
pub const MAX_N_FFT: i64 = 8192;
pub const MAX_HOP: i64 = 8192;
pub const MIN_N_MELS: i64 = 8;
pub const MAX_N_MELS: i64 = 512;
pub const MAX_FREQ_HZ: f64 = 24_000.0;
pub const MIN_TOP_DB: f64 = 10.0;
pub const MAX_TOP_DB: f64 = 160.0;
pub const MIN_WIDTH: i64 = 32;
pub const MAX_WIDTH: i64 = 4096;
pub const MIN_HEIGHT: i64 = 32;
pub const MAX_HEIGHT: i64 = 2048;
pub const MIN_RESAMPLE_HZ: i64 = 4_000;
pub const MAX_RESAMPLE_HZ: i64 = 48_000;

/// Power floor for the log stages (−200 dB), so digital silence still yields a
/// finite value instead of `-inf`.
const POWER_FLOOR: f64 = 1e-20;

const SUPPORTED: &str = "supported containers: WAV, AIFF, CAF, FLAC, MP3, OGG, MP4/MOV/M4A, \
                         MKV/WebM, AAC-ADTS; audio codecs: PCM, ADPCM, FLAC, MP3, AAC-LC, ALAC, \
                         Vorbis (Opus, AC-3 and DTS are not supported)";

/// Every knob except the input bytes. Grouped so the chat block, the CLI and
/// the browser wrapper all construct exactly the same configuration.
#[derive(Clone, Debug)]
pub struct Options {
    /// FFT / analysis-window size in samples.
    pub n_fft: i64,
    /// Frame step in samples; `0` means `n_fft / 4` (the librosa default).
    pub hop_length: i64,
    /// Number of triangular mel bands (image rows before scaling).
    pub n_mels: i64,
    /// Lowest mel-band edge, Hz.
    pub fmin: f64,
    /// Highest mel-band edge, Hz; `0` means Nyquist.
    pub fmax: f64,
    /// `"slaney"` (default) or `"htk"`.
    pub mel_scale: String,
    /// `"hann"` (default), `"hamming"`, `"blackman"` or `"rectangular"`.
    pub window: String,
    /// `"db"` (default), `"power"` or `"magnitude"`.
    pub scale: String,
    /// Dynamic range in dB shown below the reference level.
    pub top_db: f64,
    /// `"peak"` (default, 0 dB = the clip's loudest cell) or `"full_scale"`.
    pub db_reference: String,
    /// `"magma"`, `"inferno"`, `"viridis"`, `"plasma"`, `"turbo"`,
    /// `"grayscale"` or `"grayscale_inverted"`.
    pub colormap: String,
    /// Output width in pixels; `0` means one column per analysis frame.
    pub width: i64,
    /// Output height in pixels; `0` means one row per mel band.
    pub height: i64,
    /// `"mix"` (default downmix), `"left"` or `"right"`.
    pub channel: String,
    /// Reflect-pad by `n_fft / 2` so frames are centred (librosa `center=True`).
    pub center: bool,
    /// Analysis sample rate; `0` keeps the file's own rate.
    pub resample_hz: i64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            n_fft: 2048,
            hop_length: 0,
            n_mels: 128,
            fmin: 0.0,
            fmax: 0.0,
            mel_scale: "slaney".into(),
            window: "hann".into(),
            scale: "db".into(),
            top_db: 80.0,
            db_reference: "peak".into(),
            colormap: "magma".into(),
            width: 1000,
            height: 400,
            channel: "mix".into(),
            center: true,
            resample_hz: 0,
        }
    }
}

/// The rendered spectrogram plus the facts the chat/CLI/page surfaces report.
#[derive(Clone, Debug)]
pub struct Rendered {
    /// PNG bytes (RGB, no alpha).
    pub png: Vec<u8>,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// STFT frames (matrix columns) actually analysed.
    pub frames: usize,
    /// Mel bands (matrix rows).
    pub n_mels: usize,
    /// Analysis sample rate in Hz (after any resampling).
    pub sample_rate: u32,
    /// Source sample rate in Hz.
    pub source_rate: u32,
    /// Source channel count.
    pub channels: usize,
    /// Seconds of audio analysed.
    pub duration_s: f64,
    /// Loudest mel cell, dBFS (0 dBFS = a full-scale sine).
    pub peak_db: f64,
    /// Centre frequency of the loudest mel band, Hz.
    pub peak_hz: f64,
    /// Human-readable settings report — what the page shows under the image.
    pub summary: String,
}

/// Resolved, validated configuration: every "0 means auto" filled in.
struct Resolved {
    n_fft: usize,
    hop: usize,
    n_mels: usize,
    fmin: f64,
    fmax: f64,
    slaney: bool,
    window: Vec<f64>,
    window_name: String,
    scale: Scale,
    top_db: f64,
    ref_peak: bool,
    colormap: Colormap,
    width: usize,
    height: usize,
    center: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scale {
    Db,
    Power,
    Magnitude,
}

/// Facts about the decoded stream.
struct Decoded {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: usize,
    truncated: bool,
    /// Set when "left"/"right" was asked for but the source has fewer channels.
    channel_fallback: bool,
}

/// Render one audio file as a mel-spectrogram PNG.
///
/// - `input`: the audio file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `opts`: every other knob; see [`Options`].
///
/// Returns a user-facing error string for empty/oversized input, undecodable
/// bytes, an unsupported container/codec, audio shorter than one analysis
/// frame, or any out-of-range parameter.
pub fn run(input: &str, input_format: &str, opts: &Options) -> Result<Rendered, String> {
    let bytes = decode_bytes(input, input_format)?;
    let channel = Channel::parse(&opts.channel)?;
    let decoded = decode_audio(bytes, channel)?;
    let source_rate = decoded.sample_rate;

    // Resample first: every sample-count bound resolves at the analysis rate.
    let (signal, rate) = match opts.resample_hz {
        0 => (decoded.samples.clone(), source_rate),
        hz => {
            if !(MIN_RESAMPLE_HZ..=MAX_RESAMPLE_HZ).contains(&hz) {
                return Err(format!(
                    "invalid resample_hz {hz}: expected 0 (keep the file's rate) or \
                     {MIN_RESAMPLE_HZ}-{MAX_RESAMPLE_HZ} Hz"
                ));
            }
            let target = hz as u32;
            if target == source_rate {
                (decoded.samples.clone(), source_rate)
            } else {
                (resample(&decoded.samples, source_rate, target), target)
            }
        }
    };

    let mut cfg = resolve(opts, rate)?;

    // Centred analysis reflect-pads both ends; otherwise only complete frames
    // inside the signal are used, so the clip must hold at least one.
    if !cfg.center && signal.len() < cfg.n_fft {
        return Err(format!(
            "audio is {} samples ({:.3} s at {rate} Hz) but one analysis frame is {} samples — \
             use a smaller n_fft, enable centred frames, or a longer clip",
            signal.len(),
            signal.len() as f64 / rate as f64,
            cfg.n_fft
        ));
    }

    let total_frames = if cfg.center {
        1 + signal.len() / cfg.hop
    } else {
        1 + (signal.len() - cfg.n_fft) / cfg.hop
    };
    let work_cap = (MAX_FFT_WORK / cfg.n_fft).max(1);
    let frames = total_frames.min(MAX_FRAMES).min(work_cap);
    let frames_truncated = frames < total_frames;

    if cfg.width == 0 {
        cfg.width = frames.max(1);
    }
    if cfg.height == 0 {
        cfg.height = cfg.n_mels.max(1);
    }

    let bank = mel_filterbank(&cfg, rate)?;
    let matrix = stft_mel(&signal, &cfg, &bank);

    let mel_centres = mel_band_centres(&cfg);
    let (peak_db, peak_hz) = peak_cell(&matrix, &mel_centres);

    let normalized = normalize(&matrix, &cfg);
    let pixels = rasterize(&normalized, &cfg);
    let png = encode_png(&pixels, cfg.width as u32, cfg.height as u32)?;

    let duration_s = signal.len() as f64 / rate as f64;
    let analysed_s = (frames.saturating_sub(1) * cfg.hop) as f64 / rate as f64;
    let summary = summarize(
        &cfg,
        opts,
        rate,
        source_rate,
        &decoded,
        frames,
        total_frames,
        duration_s,
        analysed_s,
        frames_truncated,
        peak_db,
        peak_hz,
        png.len(),
    );

    Ok(Rendered {
        png,
        width: cfg.width as u32,
        height: cfg.height as u32,
        frames,
        n_mels: cfg.n_mels,
        sample_rate: rate,
        source_rate,
        channels: decoded.channels,
        duration_s: if frames_truncated {
            analysed_s
        } else {
            duration_s
        },
        peak_db,
        peak_hz,
        summary,
    })
}

// ---------------------------------------------------------------------------
// Parameter validation
// ---------------------------------------------------------------------------

fn resolve(opts: &Options, rate: u32) -> Result<Resolved, String> {
    if !(MIN_N_FFT..=MAX_N_FFT).contains(&opts.n_fft) {
        return Err(format!(
            "invalid n_fft {}: expected {MIN_N_FFT}-{MAX_N_FFT} samples",
            opts.n_fft
        ));
    }
    if !(0..=MAX_HOP).contains(&opts.hop_length) {
        return Err(format!(
            "invalid hop_length {}: expected 0 (n_fft/4) or 1-{MAX_HOP} samples",
            opts.hop_length
        ));
    }
    if !(MIN_N_MELS..=MAX_N_MELS).contains(&opts.n_mels) {
        return Err(format!(
            "invalid n_mels {}: expected {MIN_N_MELS}-{MAX_N_MELS} mel bands",
            opts.n_mels
        ));
    }
    if !opts.top_db.is_finite() || !(MIN_TOP_DB..=MAX_TOP_DB).contains(&opts.top_db) {
        return Err(format!(
            "invalid top_db {}: expected {MIN_TOP_DB}-{MAX_TOP_DB} dB of dynamic range",
            opts.top_db
        ));
    }
    if opts.width != 0 && !(MIN_WIDTH..=MAX_WIDTH).contains(&opts.width) {
        return Err(format!(
            "invalid width {}: expected 0 (one pixel column per frame) or {MIN_WIDTH}-{MAX_WIDTH} \
             pixels",
            opts.width
        ));
    }
    if opts.height != 0 && !(MIN_HEIGHT..=MAX_HEIGHT).contains(&opts.height) {
        return Err(format!(
            "invalid height {}: expected 0 (one pixel row per mel band) or \
             {MIN_HEIGHT}-{MAX_HEIGHT} pixels",
            opts.height
        ));
    }

    let slaney = match opts.mel_scale.trim() {
        "" | "slaney" => true,
        "htk" => false,
        other => {
            return Err(format!(
                "invalid mel_scale {other:?}: expected \"slaney\" or \"htk\""
            ))
        }
    };
    let scale = match opts.scale.trim() {
        "" | "db" => Scale::Db,
        "power" => Scale::Power,
        "magnitude" => Scale::Magnitude,
        other => {
            return Err(format!(
                "invalid scale {other:?}: expected \"db\", \"power\" or \"magnitude\""
            ))
        }
    };
    let ref_peak = match opts.db_reference.trim() {
        "" | "peak" => true,
        "full_scale" => false,
        other => {
            return Err(format!(
                "invalid db_reference {other:?}: expected \"peak\" or \"full_scale\""
            ))
        }
    };
    let colormap = Colormap::parse(&opts.colormap)?;

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

    let n_fft = opts.n_fft as usize;
    let hop = if opts.hop_length == 0 {
        (n_fft / 4).max(1)
    } else {
        opts.hop_length as usize
    };
    let (window, window_name) = analysis_window(&opts.window, n_fft)?;

    Ok(Resolved {
        n_fft,
        hop,
        n_mels: opts.n_mels as usize,
        fmin: opts.fmin,
        fmax,
        slaney,
        window,
        window_name,
        scale,
        top_db: opts.top_db,
        ref_peak,
        colormap,
        width: opts.width as usize,
        height: opts.height as usize,
        center: opts.center,
    })
}

/// Periodic window coefficients (`denom = len`), the convention librosa and
/// torchaudio use for spectrogram analysis.
fn analysis_window(name: &str, len: usize) -> Result<(Vec<f64>, String), String> {
    let denom = len.max(1) as f64;
    let cos_at = |i: usize, k: f64| (2.0 * std::f64::consts::PI * k * i as f64 / denom).cos();
    let (name, w): (&str, Vec<f64>) = match name.trim() {
        "" | "hann" => (
            "hann",
            (0..len).map(|i| 0.5 - 0.5 * cos_at(i, 1.0)).collect(),
        ),
        "hamming" => (
            "hamming",
            (0..len).map(|i| 0.54 - 0.46 * cos_at(i, 1.0)).collect(),
        ),
        "blackman" => (
            "blackman",
            (0..len)
                .map(|i| 0.42 - 0.5 * cos_at(i, 1.0) + 0.08 * cos_at(i, 2.0))
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

/// Which channel of the source feeds the analysis.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Channel {
    Mix,
    Left,
    Right,
}

impl Channel {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "mix" => Ok(Channel::Mix),
            "left" => Ok(Channel::Left),
            "right" => Ok(Channel::Right),
            other => Err(format!(
                "invalid channel {other:?}: expected \"mix\", \"left\" or \"right\""
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Resampling
// ---------------------------------------------------------------------------

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

/// Mel-band edge frequencies (`n_mels + 2` of them: each band spans a triple).
fn mel_edges(cfg: &Resolved) -> Vec<f64> {
    let mel_lo = hz_to_mel(cfg.fmin, cfg.slaney);
    let mel_hi = hz_to_mel(cfg.fmax, cfg.slaney);
    (0..cfg.n_mels + 2)
        .map(|i| {
            let mel = mel_lo + (mel_hi - mel_lo) * i as f64 / (cfg.n_mels + 1) as f64;
            mel_to_hz(mel, cfg.slaney)
        })
        .collect()
}

/// Centre frequency of each mel band, for the "loudest band" report.
fn mel_band_centres(cfg: &Resolved) -> Vec<f64> {
    let edges = mel_edges(cfg);
    (0..cfg.n_mels).map(|i| edges[i + 1]).collect()
}

/// Triangular filters spread evenly on the mel scale between `fmin` and `fmax`.
/// Weights are computed against the exact FFT bin frequencies (not rounded bin
/// indices); the Slaney scale additionally normalises each filter by its
/// bandwidth so bands carry equal area (librosa's `norm="slaney"`).
fn mel_filterbank(cfg: &Resolved, rate: u32) -> Result<Vec<Filter>, String> {
    let n_bins = cfg.n_fft / 2 + 1;
    let bin_hz = rate as f64 / cfg.n_fft as f64;
    let edges = mel_edges(cfg);

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
            "{empty} of the {} mel bands are narrower than one FFT bin ({bin_hz:.1} Hz) and would \
             always read silent — use fewer n_mels, a larger n_fft, or a wider fmin/fmax range",
            cfg.n_mels
        ));
    }
    Ok(bank)
}

// ---------------------------------------------------------------------------
// STFT → mel power matrix
// ---------------------------------------------------------------------------

/// Reflect-padded sample read, used for the centred (`center = true`) framing:
/// index `-1` reads sample 1, index `len` reads sample `len - 2`, exactly like
/// numpy's `pad(mode="reflect")`. Falls back to clamping for signals shorter
/// than the padding.
fn sample_at(signal: &[f32], idx: isize) -> f64 {
    let len = signal.len() as isize;
    if len == 0 {
        return 0.0;
    }
    let mut i = idx;
    if len > 1 {
        let period = 2 * (len - 1);
        i = i.rem_euclid(period);
        if i >= len {
            i = period - i;
        }
    } else {
        i = 0;
    }
    signal[i.clamp(0, len - 1) as usize] as f64
}

/// Frame → window → FFT → one-sided power → mel bands. Returns `frames` rows of
/// `n_mels` mel powers, where 1.0 ≈ a full-scale sine in that band (0 dBFS).
fn stft_mel(signal: &[f32], cfg: &Resolved, bank: &[Filter]) -> Vec<Vec<f64>> {
    let work_cap = (MAX_FFT_WORK / cfg.n_fft).max(1);
    let total = if cfg.center {
        1 + signal.len() / cfg.hop
    } else {
        1 + (signal.len().saturating_sub(cfg.n_fft)) / cfg.hop
    };
    let frames = total.min(MAX_FRAMES).min(work_cap);

    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(cfg.n_fft);
    let n_bins = cfg.n_fft / 2 + 1;
    // Window-sum normalisation: a full-scale sine inside the window reads a
    // one-sided magnitude of ~1.0 regardless of window shape or n_fft.
    let win_sum: f64 = cfg.window.iter().sum::<f64>().max(f64::EPSILON);
    let offset = if cfg.center {
        cfg.n_fft as isize / 2
    } else {
        0
    };

    let mut buf = vec![Complex { re: 0.0, im: 0.0 }; cfg.n_fft];
    let mut power = vec![0.0f64; n_bins];
    let mut out = Vec::with_capacity(frames);

    for f in 0..frames {
        let start = (f * cfg.hop) as isize - offset;
        for (i, slot) in buf.iter_mut().enumerate() {
            let s = if cfg.center {
                sample_at(signal, start + i as isize)
            } else {
                signal.get(start as usize + i).copied().unwrap_or(0.0) as f64
            };
            *slot = Complex {
                re: s * cfg.window[i],
                im: 0.0,
            };
        }
        fft.process(&mut buf);
        for (k, p) in power.iter_mut().enumerate() {
            // One-sided: every bin except DC and Nyquist carries its mirror.
            let fold = if k == 0 || (cfg.n_fft % 2 == 0 && k == n_bins - 1) {
                1.0
            } else {
                2.0
            };
            let mag = buf[k].norm() * fold / win_sum;
            *p = mag * mag;
        }

        let mut row = Vec::with_capacity(cfg.n_mels);
        for filt in bank {
            let mut sum = 0.0f64;
            for (i, w) in filt.weights.iter().enumerate() {
                sum += power[filt.start + i] * w;
            }
            row.push(sum);
        }
        out.push(row);
    }
    out
}

/// Loudest mel cell as (dBFS, band centre frequency).
fn peak_cell(matrix: &[Vec<f64>], centres: &[f64]) -> (f64, f64) {
    let mut best = 0.0f64;
    let mut band = 0usize;
    for row in matrix {
        for (m, v) in row.iter().enumerate() {
            if *v > best {
                best = *v;
                band = m;
            }
        }
    }
    (
        10.0 * best.max(POWER_FLOOR).log10(),
        centres.get(band).copied().unwrap_or(0.0),
    )
}

// ---------------------------------------------------------------------------
// Normalisation (power → 0..1 brightness)
// ---------------------------------------------------------------------------

/// Map every mel power to 0..1 using the chosen scale, reference and dynamic
/// range. 1.0 is the reference level, 0.0 the bottom of the range.
fn normalize(matrix: &[Vec<f64>], cfg: &Resolved) -> Vec<Vec<f64>> {
    let peak = matrix
        .iter()
        .flat_map(|r| r.iter())
        .fold(0.0f64, |a, b| a.max(*b));
    let reference = if cfg.ref_peak {
        peak.max(POWER_FLOOR)
    } else {
        1.0
    };
    matrix
        .iter()
        .map(|row| {
            row.iter()
                .map(|v| {
                    let t = match cfg.scale {
                        Scale::Db => {
                            let db = 10.0 * (v.max(POWER_FLOOR) / reference).log10();
                            1.0 + db / cfg.top_db
                        }
                        Scale::Power => v / reference,
                        Scale::Magnitude => (v / reference).sqrt(),
                    };
                    t.clamp(0.0, 1.0)
                })
                .collect()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Rasterisation
// ---------------------------------------------------------------------------

/// Box-resample the frames × bands matrix onto the pixel grid and apply the
/// colormap. Row 0 of the image is the HIGHEST mel band: low frequencies sit at
/// the bottom, the convention every spectrogram viewer uses. Downscaling
/// averages every covered cell (nothing is dropped); upscaling repeats cells.
fn rasterize(matrix: &[Vec<f64>], cfg: &Resolved) -> Vec<u8> {
    let (w, h) = (cfg.width, cfg.height);
    let frames = matrix.len().max(1);
    let bands = cfg.n_mels.max(1);
    let mut px = vec![0u8; w * h * 3];

    // Precompute the [lo, hi) source spans each output column/row covers.
    let span = |i: usize, out: usize, src: usize| -> (usize, usize) {
        let lo = i * src / out;
        let hi = (((i + 1) * src).div_ceil(out)).max(lo + 1).min(src);
        (lo, hi)
    };

    for y in 0..h {
        // Flip: image row 0 = top = highest band.
        let (b_lo, b_hi) = span(h - 1 - y, h, bands);
        for x in 0..w {
            let (f_lo, f_hi) = span(x, w, frames);
            let mut acc = 0.0f64;
            let mut n = 0usize;
            for row in matrix.iter().take(f_hi).skip(f_lo) {
                for v in row.iter().take(b_hi).skip(b_lo) {
                    acc += *v;
                    n += 1;
                }
            }
            let t = if n == 0 { 0.0 } else { acc / n as f64 };
            let [r, g, b] = cfg.colormap.sample(t);
            let o = (y * w + x) * 3;
            px[o] = r;
            px[o + 1] = g;
            px[o + 2] = b;
        }
    }
    px
}

fn encode_png(pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(pixels, w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// Colormaps
// ---------------------------------------------------------------------------

/// Perceptual colormaps, each as nine evenly spaced anchor colours that are
/// linearly interpolated. Close enough to the published matplotlib/Google maps
/// for a spectrogram view, with no lookup-table dependency.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Colormap {
    Magma,
    Inferno,
    Viridis,
    Plasma,
    Turbo,
    Grayscale,
    GrayscaleInverted,
}

const MAGMA: [[u8; 3]; 9] = [
    [0, 0, 4],
    [28, 16, 68],
    [79, 18, 123],
    [129, 37, 129],
    [181, 54, 122],
    [229, 80, 100],
    [251, 135, 97],
    [254, 194, 135],
    [252, 253, 191],
];
const INFERNO: [[u8; 3]; 9] = [
    [0, 0, 4],
    [22, 11, 57],
    [66, 10, 104],
    [106, 23, 110],
    [147, 38, 103],
    [188, 55, 84],
    [221, 81, 58],
    [243, 141, 24],
    [252, 255, 164],
];
const VIRIDIS: [[u8; 3]; 9] = [
    [68, 1, 84],
    [72, 40, 120],
    [62, 74, 137],
    [49, 104, 142],
    [38, 130, 142],
    [31, 158, 137],
    [53, 183, 121],
    [109, 205, 89],
    [253, 231, 37],
];
const PLASMA: [[u8; 3]; 9] = [
    [13, 8, 135],
    [75, 3, 161],
    [125, 3, 168],
    [168, 34, 150],
    [203, 70, 121],
    [229, 107, 93],
    [248, 148, 65],
    [253, 195, 40],
    [240, 249, 33],
];
const TURBO: [[u8; 3]; 9] = [
    [48, 18, 59],
    [70, 107, 227],
    [40, 181, 235],
    [44, 224, 168],
    [108, 248, 96],
    [185, 246, 50],
    [238, 201, 47],
    [251, 133, 28],
    [122, 4, 3],
];

impl Colormap {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "magma" => Ok(Colormap::Magma),
            "inferno" => Ok(Colormap::Inferno),
            "viridis" => Ok(Colormap::Viridis),
            "plasma" => Ok(Colormap::Plasma),
            "turbo" => Ok(Colormap::Turbo),
            "grayscale" => Ok(Colormap::Grayscale),
            "grayscale_inverted" => Ok(Colormap::GrayscaleInverted),
            other => Err(format!(
                "invalid colormap {other:?}: expected \"magma\", \"inferno\", \"viridis\", \
                 \"plasma\", \"turbo\", \"grayscale\" or \"grayscale_inverted\""
            )),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Colormap::Magma => "magma",
            Colormap::Inferno => "inferno",
            Colormap::Viridis => "viridis",
            Colormap::Plasma => "plasma",
            Colormap::Turbo => "turbo",
            Colormap::Grayscale => "grayscale",
            Colormap::GrayscaleInverted => "grayscale_inverted",
        }
    }

    /// Colour for a 0..1 brightness (0 = quietest, 1 = the reference level).
    pub fn sample(&self, t: f64) -> [u8; 3] {
        let t = t.clamp(0.0, 1.0);
        let anchors = match self {
            Colormap::Magma => &MAGMA,
            Colormap::Inferno => &INFERNO,
            Colormap::Viridis => &VIRIDIS,
            Colormap::Plasma => &PLASMA,
            Colormap::Turbo => &TURBO,
            Colormap::Grayscale => {
                let v = (t * 255.0).round() as u8;
                return [v, v, v];
            }
            Colormap::GrayscaleInverted => {
                let v = ((1.0 - t) * 255.0).round() as u8;
                return [v, v, v];
            }
        };
        let last = anchors.len() - 1;
        let pos = t * last as f64;
        let i = (pos.floor() as usize).min(last);
        let j = (i + 1).min(last);
        let frac = pos - i as f64;
        let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * frac).round() as u8;
        [
            mix(anchors[i][0], anchors[j][0]),
            mix(anchors[i][1], anchors[j][1]),
            mix(anchors[i][2], anchors[j][2]),
        ]
    }
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

fn hz(v: f64) -> String {
    if v >= 1000.0 {
        format!("{:.2} kHz", v / 1000.0)
    } else {
        format!("{v:.0} Hz")
    }
}

#[allow(clippy::too_many_arguments)]
fn summarize(
    cfg: &Resolved,
    opts: &Options,
    rate: u32,
    source_rate: u32,
    decoded: &Decoded,
    frames: usize,
    total_frames: usize,
    duration_s: f64,
    analysed_s: f64,
    frames_truncated: bool,
    peak_db: f64,
    peak_hz: f64,
    png_bytes: usize,
) -> String {
    let mut s = String::new();
    let scale = match cfg.scale {
        Scale::Db => {
            if cfg.ref_peak {
                format!("dB, {:.0} dB below the clip peak", cfg.top_db)
            } else {
                format!("dBFS, {:.0} dB below full scale", cfg.top_db)
            }
        }
        Scale::Power => "linear power".to_string(),
        Scale::Magnitude => "linear magnitude".to_string(),
    };
    s.push_str(&format!(
        "{}x{} PNG ({} KiB), {} colormap, low frequencies at the bottom.\n",
        cfg.width,
        cfg.height,
        png_bytes.div_ceil(1024),
        cfg.colormap.name()
    ));
    s.push_str(&format!(
        "Source: {} Hz, {} channel{} ({}), {:.2} s analysed.\n",
        source_rate,
        decoded.channels,
        if decoded.channels == 1 { "" } else { "s" },
        match Channel::parse(&opts.channel).unwrap_or(Channel::Mix) {
            Channel::Mix => "downmixed to mono",
            Channel::Left =>
                if decoded.channel_fallback {
                    "mono source, left = the only channel"
                } else {
                    "left channel only"
                },
            Channel::Right =>
                if decoded.channel_fallback {
                    "mono source, right = the only channel"
                } else {
                    "right channel only"
                },
        },
        if frames_truncated {
            analysed_s
        } else {
            duration_s
        }
    ));
    if rate != source_rate {
        s.push_str(&format!("Resampled to {rate} Hz for analysis.\n"));
    }
    s.push_str(&format!(
        "Analysis: n_fft {} ({:.1} ms), hop {} ({:.1} ms), {} window, {}centred frames.\n",
        cfg.n_fft,
        1000.0 * cfg.n_fft as f64 / rate as f64,
        cfg.hop,
        1000.0 * cfg.hop as f64 / rate as f64,
        cfg.window_name,
        if cfg.center { "" } else { "un" }
    ));
    s.push_str(&format!(
        "Mel: {} bands, {}-{} ({} scale), {} frames x {} bands, {}.\n",
        cfg.n_mels,
        hz(cfg.fmin),
        hz(cfg.fmax),
        if cfg.slaney { "Slaney" } else { "HTK" },
        frames,
        cfg.n_mels,
        scale
    ));
    s.push_str(&format!(
        "Loudest band: {} at {:.1} dBFS.\n",
        hz(peak_hz),
        peak_db
    ));
    if decoded.truncated {
        s.push_str(&format!(
            "Note: only the first {MAX_SAMPLES} samples were decoded (the clip is longer).\n"
        ));
    }
    if frames_truncated {
        s.push_str(&format!(
            "Note: {frames} of {total_frames} frames rendered (per-run analysis cap) — use a \
             larger hop_length for the whole clip.\n"
        ));
    }
    s
}

// ---------------------------------------------------------------------------
// Decode (base64/hex → symphonia → f32 samples)
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
                "no decodable audio track found (an image or video-only file has no spectrogram \
                 to draw); {SUPPORTED}"
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
    let mut channel_fallback = false;
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
            Ok(audio) => {
                if audio.frames() == 0 {
                    continue;
                }
                let spec = *audio.spec();
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
                    channel_fallback = match channel {
                        Channel::Mix => false,
                        Channel::Left => false,
                        Channel::Right => ch < 2,
                    };
                } else if spec.rate != rate || ch != channels {
                    return Err("variable sample rate / channel layout is not supported".into());
                }
                let recreate = match &sbuf {
                    Some(b) => b.capacity() < audio.capacity() * channels,
                    None => true,
                };
                if recreate {
                    sbuf = Some(SampleBuffer::<f32>::new(audio.capacity() as u64, spec));
                }
                let buf = sbuf.as_mut().expect("sample buffer just created");
                buf.copy_interleaved_ref(audio);
                let inv = 1.0 / channels as f32;
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
        channel_fallback,
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

    /// 16-bit mono WAV of a sine at `freq`, as base64 — the fixture every
    /// happy-path test analyses.
    fn sine_wav_base64(freq: f64, rate: u32, seconds: f64, amplitude: f64) -> String {
        let n = (rate as f64 * seconds) as usize;
        let mut data = Vec::with_capacity(n * 2);
        for i in 0..n {
            let t = i as f64 / rate as f64;
            let v = (amplitude * (2.0 * std::f64::consts::PI * freq * t).sin() * 32767.0) as i16;
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        base64_encode(&wav)
    }

    fn base64_encode(bytes: &[u8]) -> String {
        const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            out.push(A[(n >> 18) as usize & 63] as char);
            out.push(A[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 {
                A[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                A[n as usize & 63] as char
            } else {
                '='
            });
        }
        out
    }

    /// Decode a PNG back to (width, height, RGB pixels) for pixel assertions.
    fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .expect("valid PNG")
            .to_rgb8();
        (img.width(), img.height(), img.into_raw())
    }

    #[test]
    fn renders_a_png_of_the_requested_size_with_energy_at_the_tone() {
        let wav = sine_wav_base64(440.0, 16_000, 1.0, 0.5);
        let opts = Options {
            width: 200,
            height: 128,
            n_mels: 128,
            colormap: "grayscale".into(),
            ..Options::default()
        };
        let r = run(&wav, "base64", &opts).expect("renders");
        assert_eq!((r.width, r.height), (200, 128));
        assert_eq!(r.sample_rate, 16_000);
        assert_eq!(r.n_mels, 128);
        assert!(r.frames > 20, "frames = {}", r.frames);

        let (w, h, px) = decode_png(&r.png);
        assert_eq!((w, h), (200, 128));
        assert_eq!(px.len(), 200 * 128 * 3);
        // PNG magic — the bytes really are a PNG, not just an image-crate value.
        assert_eq!(
            &r.png[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );

        // A 440 Hz tone at 16 kHz with fmax = 8 kHz sits low in the mel range:
        // the bottom third must be far brighter than the top row.
        let lum = |x: usize, y: usize| px[(y * 200 + x) * 3] as u32;
        let bottom: u32 = (0..200).map(|x| lum(x, 100)).sum();
        let top: u32 = (0..200).map(|x| lum(x, 2)).sum();
        assert!(
            bottom > top * 4,
            "tone energy should sit low: bottom {bottom} vs top {top}"
        );
        // The loudest band should be within a mel band or two of 440 Hz.
        assert!((r.peak_hz - 440.0).abs() < 60.0, "peak_hz = {}", r.peak_hz);
        // The Hann-windowed STFT/filterbank peak for this half-scale sine is
        // stable and well above the noise floor (absolute dBFS is intentionally
        // implementation-specific because the mel filters are area-normalized).
        assert!(
            (-24.0..=-12.0).contains(&r.peak_db),
            "peak_db = {} (expected a stable windowed/filterbank peak)",
            r.peak_db
        );
        assert!(r.summary.contains("200x128 PNG"));
        assert!(r.summary.contains("Slaney"));
    }

    #[test]
    fn natural_size_is_one_pixel_per_frame_and_band() {
        let wav = sine_wav_base64(440.0, 16_000, 0.5, 0.5);
        let opts = Options {
            width: 0,
            height: 0,
            n_mels: 40,
            n_fft: 512,
            hop_length: 256,
            ..Options::default()
        };
        let r = run(&wav, "base64", &opts).expect("renders");
        assert_eq!(r.height, 40);
        assert_eq!(r.width, r.frames as u32);
        // center = true → 1 + samples/hop frames (librosa's frame count).
        assert_eq!(r.frames, 1 + 8000 / 256);
    }

    #[test]
    fn htk_and_slaney_scales_and_every_colormap_render() {
        let wav = sine_wav_base64(1000.0, 22_050, 0.4, 0.8);
        for cmap in [
            "magma",
            "inferno",
            "viridis",
            "plasma",
            "turbo",
            "grayscale",
            "grayscale_inverted",
        ] {
            for scale in ["db", "power", "magnitude"] {
                for mel in ["slaney", "htk"] {
                    let opts = Options {
                        width: 64,
                        height: 64,
                        colormap: cmap.into(),
                        scale: scale.into(),
                        mel_scale: mel.into(),
                        ..Options::default()
                    };
                    let r = run(&wav, "base64", &opts)
                        .unwrap_or_else(|e| panic!("{cmap}/{scale}/{mel}: {e}"));
                    assert_eq!((r.width, r.height), (64, 64));
                }
            }
        }
    }

    #[test]
    fn full_scale_reference_and_top_db_change_the_floor() {
        let wav = sine_wav_base64(440.0, 16_000, 0.5, 0.02); // ~-34 dBFS
        let peak_ref = run(
            &wav,
            "base64",
            &Options {
                width: 64,
                height: 64,
                colormap: "grayscale".into(),
                ..Options::default()
            },
        )
        .expect("renders");
        let fs_ref = run(
            &wav,
            "base64",
            &Options {
                width: 64,
                height: 64,
                colormap: "grayscale".into(),
                db_reference: "full_scale".into(),
                ..Options::default()
            },
        )
        .expect("renders");
        let brightest = |png: &[u8]| decode_png(png).2.iter().copied().max().unwrap_or(0);
        // Peak-referenced is visibly brighter; full-scale keeps the quiet clip
        // below the top of the range.
        assert!(brightest(&peak_ref.png) > 200);
        assert!(
            brightest(&fs_ref.png) < brightest(&peak_ref.png),
            "full-scale reference should stay below peak reference, got {} vs {}",
            brightest(&fs_ref.png),
            brightest(&fs_ref.png)
        );
    }

    #[test]
    fn hex_input_matches_base64_input() {
        let wav_b64 = sine_wav_base64(300.0, 8_000, 0.3, 0.5);
        let bytes = decode_base64(&wav_b64).unwrap();
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let opts = Options {
            width: 48,
            height: 48,
            ..Options::default()
        };
        let a = run(&wav_b64, "base64", &opts).expect("base64");
        let b = run(&hex, "hex", &opts).expect("hex");
        assert_eq!(a.png, b.png);
    }

    #[test]
    fn rejects_empty_input() {
        let err = run("   ", "base64", &Options::default()).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn rejects_non_audio_bytes() {
        let err = run("aGVsbG8gd29ybGQ", "base64", &Options::default()).unwrap_err();
        assert!(
            err.contains("unrecognized or unsupported media format"),
            "{err}"
        );
    }

    #[test]
    fn rejects_out_of_range_parameters() {
        let wav = sine_wav_base64(440.0, 16_000, 0.2, 0.5);
        let cases: Vec<(Options, &str)> = vec![
            (
                Options {
                    n_fft: 16,
                    ..Options::default()
                },
                "invalid n_fft",
            ),
            (
                Options {
                    n_mels: 4,
                    ..Options::default()
                },
                "invalid n_mels",
            ),
            (
                Options {
                    top_db: 500.0,
                    ..Options::default()
                },
                "invalid top_db",
            ),
            (
                Options {
                    width: 9000,
                    ..Options::default()
                },
                "invalid width",
            ),
            (
                Options {
                    height: 5,
                    ..Options::default()
                },
                "invalid height",
            ),
            (
                Options {
                    colormap: "rainbow".into(),
                    ..Options::default()
                },
                "invalid colormap",
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
                    scale: "linear".into(),
                    ..Options::default()
                },
                "invalid scale",
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
                    db_reference: "rms".into(),
                    ..Options::default()
                },
                "invalid db_reference",
            ),
            (
                Options {
                    channel: "centre".into(),
                    ..Options::default()
                },
                "invalid channel",
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
                    fmin: 4000.0,
                    fmax: 1000.0,
                    ..Options::default()
                },
                "invalid frequency range",
            ),
        ];
        for (opts, needle) in cases {
            let err = run(&wav, "base64", &opts).unwrap_err();
            assert!(err.contains(needle), "expected {needle:?}, got {err}");
        }
    }

    #[test]
    fn rejects_bands_narrower_than_one_fft_bin() {
        let wav = sine_wav_base64(440.0, 48_000, 0.3, 0.5);
        let err = run(
            &wav,
            "base64",
            &Options {
                n_fft: 128,
                n_mels: 256,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("narrower than one FFT bin"), "{err}");
    }

    #[test]
    fn uncentred_analysis_rejects_a_clip_shorter_than_one_frame() {
        let wav = sine_wav_base64(440.0, 8_000, 0.01, 0.5); // 80 samples
        let err = run(
            &wav,
            "base64",
            &Options {
                n_fft: 2048,
                center: false,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("one analysis frame"), "{err}");
    }

    #[test]
    fn resampling_reports_the_analysis_rate() {
        let wav = sine_wav_base64(440.0, 44_100, 0.5, 0.5);
        let r = run(
            &wav,
            "base64",
            &Options {
                resample_hz: 16_000,
                width: 64,
                height: 64,
                ..Options::default()
            },
        )
        .expect("renders");
        assert_eq!(r.sample_rate, 16_000);
        assert_eq!(r.source_rate, 44_100);
        assert!(r.summary.contains("Resampled to 16000 Hz"), "{}", r.summary);
    }

    #[test]
    fn fmin_fmax_narrow_the_band_range() {
        let wav = sine_wav_base64(440.0, 16_000, 0.4, 0.5);
        let r = run(
            &wav,
            "base64",
            &Options {
                fmin: 300.0,
                fmax: 600.0,
                n_mels: 32,
                width: 64,
                height: 64,
                ..Options::default()
            },
        )
        .expect("renders");
        assert!(r.summary.contains("300 Hz-600 Hz"), "{}", r.summary);
        assert!((r.peak_hz - 440.0).abs() < 40.0, "peak_hz = {}", r.peak_hz);
    }

    #[test]
    fn grayscale_inverted_is_the_photographic_negative_of_grayscale() {
        assert_eq!(Colormap::Grayscale.sample(0.0), [0, 0, 0]);
        assert_eq!(Colormap::Grayscale.sample(1.0), [255, 255, 255]);
        assert_eq!(Colormap::GrayscaleInverted.sample(0.0), [255, 255, 255]);
        assert_eq!(Colormap::GrayscaleInverted.sample(1.0), [0, 0, 0]);
        assert_eq!(Colormap::Magma.sample(0.0), [0, 0, 4]);
        assert_eq!(Colormap::Viridis.sample(1.0), [253, 231, 37]);
    }

    #[test]
    fn reflect_padding_mirrors_the_signal_edges() {
        let s = [0.0f32, 1.0, 2.0, 3.0];
        assert_eq!(sample_at(&s, -1), 1.0);
        assert_eq!(sample_at(&s, -2), 2.0);
        assert_eq!(sample_at(&s, 4), 2.0);
        assert_eq!(sample_at(&s, 5), 1.0);
        assert_eq!(sample_at(&s, 0), 0.0);
    }
}
