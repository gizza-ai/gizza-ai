//! gizza-ai/video-audio-sync-offset-finder core — pure DSP shared by the chat
//! skill block and the CLI. No wafer/wasm-bindgen deps.
//!
//! Finds the time offset that best aligns two recordings of the same event
//! (video or audio, any mix) by cross-correlating their audio:
//!
//! 1. **Decode** — symphonia demuxes the container (MP4/MOV/M4A, MKV/WebM,
//!    OGG, WAV, AIFF, CAF, FLAC, MP3, AAC-ADTS), picks the first decodable
//!    AUDIO track (video tracks appear as `CODEC_TYPE_NULL` and are skipped),
//!    downmixes to mono and integrate-and-dump resamples to 8 kHz on the fly
//!    (never holds native-rate PCM — sized for the 64 MiB wasm sandbox).
//! 2. **Coarse pass** — novelty envelopes (first difference of 100 Hz
//!    log-energy — differencing whitens the correlation floor so the peak
//!    stands out) are cross-correlated over every candidate lag via FFT,
//!    normalized per lag with prefix-sum energies (true NCC, no center bias).
//!    The peak gives the offset to ±10 ms plus a BBC-audio-offset-finder-style
//!    standard score (z-score of the peak against all candidate lags).
//! 3. **Fine pass** — up to 20 s of the 8 kHz waveforms, pre-aligned at the
//!    coarse offset, are cross-correlated over ±0.6 s. If the waveform peak is
//!    strong the offset is refined to ~1 ms (sub-sample via parabolic
//!    interpolation); otherwise the envelope result stands (different
//!    microphones can share an envelope but not waveform phase).
//!
//! Sign convention: `offset_seconds > 0` means file 2 STARTS `offset` seconds
//! AFTER file 1 (file 2's t=0 aligns with file 1's t=offset).

use std::io::Cursor;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Everything is analyzed at this rate (BBC audio-offset-finder's default).
pub const ANALYSIS_RATE: u32 = 8000;
/// Envelope hop: 80 samples at 8 kHz = 10 ms → 100 Hz envelope rate.
pub const ENV_HOP: usize = 80;
/// Envelope frames per second.
pub const ENV_RATE: usize = ANALYSIS_RATE as usize / ENV_HOP;
/// Bounds for the `analyze_seconds` parameter.
pub const MIN_ANALYZE_SECONDS: f64 = 5.0;
pub const MAX_ANALYZE_SECONDS: f64 = 240.0;
pub const DEFAULT_ANALYZE_SECONDS: f64 = 120.0;
/// Each file must decode to at least this much audio.
pub const MIN_SECONDS_PER_FILE: f64 = 2.0;
/// Fine pass: window length and search half-range around the coarse offset.
const FINE_WINDOW_SECONDS: f64 = 20.0;
const FINE_MAX_LAG: usize = (0.6 * ANALYSIS_RATE as f64) as usize; // ±0.6 s
/// Minimum waveform NCC for the fine pass to override the envelope result.
const FINE_MIN_NCC: f64 = 0.25;
/// Minimum envelope overlap (frames) for a candidate lag: 1 s.
const MIN_ENV_OVERLAP: usize = ENV_RATE;
/// Minimum fine-pass window: 2 s of aligned overlap.
const MIN_FINE_WINDOW: usize = 2 * ANALYSIS_RATE as usize;
/// RMS below this (full scale = 1.0) counts as silence.
const SILENCE_RMS: f64 = 1e-5;

// ---------------------------------------------------------------------------
// FFT (iterative radix-2, f32 data / f64 twiddles — no external crate)
// ---------------------------------------------------------------------------

/// In-place complex FFT. `n` must be a power of two. `inverse` includes the
/// 1/n scale so `fft(fft(x), inverse)` round-trips.
fn fft_inplace(re: &mut [f32], im: &mut [f32], inverse: bool) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    // Bit-reversal permutation.
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
        let ang = if inverse { 2.0 } else { -2.0 } * std::f64::consts::PI / len as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        let half = len / 2;
        let mut base = 0usize;
        while base < n {
            let (mut cr, mut ci) = (1.0f64, 0.0f64);
            for k in 0..half {
                let (ur, ui) = (re[base + k] as f64, im[base + k] as f64);
                let (xr, xi) = (re[base + k + half] as f64, im[base + k + half] as f64);
                let (vr, vi) = (xr * cr - xi * ci, xr * ci + xi * cr);
                re[base + k] = (ur + vr) as f32;
                im[base + k] = (ui + vi) as f32;
                re[base + k + half] = (ur - vr) as f32;
                im[base + k + half] = (ui - vi) as f32;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
            base += len;
        }
        len <<= 1;
    }
    if inverse {
        let inv = 1.0 / n as f32;
        for v in re.iter_mut() {
            *v *= inv;
        }
        for v in im.iter_mut() {
            *v *= inv;
        }
    }
}

/// Linear cross-correlation via FFT: returns `c` of length `la + lb - 1`
/// where `c[i]` = Σ_t a[t + k]·b[t] with `k = i - (lb - 1)`
/// (k > 0 ⇒ b's content appears k samples LATER in a's timeline).
fn xcorr_raw(a: &[f32], b: &[f32]) -> Vec<f32> {
    let (la, lb) = (a.len(), b.len());
    let n = (la + lb).next_power_of_two();
    let mut ar = vec![0f32; n];
    let mut ai = vec![0f32; n];
    ar[..la].copy_from_slice(a);
    let mut br = vec![0f32; n];
    let mut bi = vec![0f32; n];
    br[..lb].copy_from_slice(b);
    fft_inplace(&mut ar, &mut ai, false);
    fft_inplace(&mut br, &mut bi, false);
    // A · conj(B)
    for i in 0..n {
        let (x, y) = (ar[i], ai[i]);
        let (u, v) = (br[i], bi[i]);
        ar[i] = x * u + y * v;
        ai[i] = y * u - x * v;
    }
    fft_inplace(&mut ar, &mut ai, true);
    // Circular index → linear lag: k ≥ 0 at ar[k], k < 0 at ar[n + k].
    let mut out = vec![0f32; la + lb - 1];
    for k in 0..la {
        out[lb - 1 + k] = ar[k];
    }
    for k in 1..lb {
        out[lb - 1 - k] = ar[n - k];
    }
    out
}

/// Per-lag normalized cross-correlation. `raw[i]` (from `xcorr_raw`) is
/// divided by sqrt of the two overlapping windows' energies (prefix sums), so
/// long overlaps are not favored over short ones. Lags whose overlap is under
/// `min_overlap` samples get NCC 0 (never selected).
fn ncc_from_raw(a: &[f32], b: &[f32], raw: &[f32], min_overlap: usize) -> Vec<f32> {
    let (la, lb) = (a.len(), b.len());
    let mut pa = vec![0f64; la + 1];
    for (i, v) in a.iter().enumerate() {
        pa[i + 1] = pa[i] + (*v as f64) * (*v as f64);
    }
    let mut pb = vec![0f64; lb + 1];
    for (i, v) in b.iter().enumerate() {
        pb[i + 1] = pb[i] + (*v as f64) * (*v as f64);
    }
    let mut out = vec![0f32; raw.len()];
    for (i, o) in out.iter_mut().enumerate() {
        let k = i as isize - (lb as isize - 1);
        // b index t is valid where both b[t] and a[t + k] exist.
        let t0 = (-k).max(0) as usize;
        let t1 = lb.min((la as isize - k).max(0) as usize);
        if t1 <= t0 || t1 - t0 < min_overlap {
            continue;
        }
        let ea = pa[(t1 as isize + k) as usize] - pa[(t0 as isize + k) as usize];
        let eb = pb[t1] - pb[t0];
        let denom = (ea * eb).sqrt();
        if denom > 1e-12 {
            *o = (raw[i] as f64 / denom) as f32;
        }
    }
    out
}

/// Novelty envelope at 100 Hz: first difference of the log-energy envelope,
/// mean-removed. Differencing decorrelates neighboring lags (a smooth envelope
/// would correlate over a wide lag range and drown the true peak's z-score).
fn envelope(x: &[f32]) -> Vec<f32> {
    let n = x.len() / ENV_HOP;
    let mut env = Vec::with_capacity(n);
    for w in 0..n {
        let s = &x[w * ENV_HOP..(w + 1) * ENV_HOP];
        let e = s.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / ENV_HOP as f64;
        env.push((e.sqrt() + 1e-6).ln() as f32);
    }
    let mut nov: Vec<f32> = env.windows(2).map(|w| w[1] - w[0]).collect();
    let mean = nov.iter().map(|v| *v as f64).sum::<f64>() / nov.len().max(1) as f64;
    for v in nov.iter_mut() {
        *v = (*v as f64 - mean) as f32;
    }
    nov
}

// ---------------------------------------------------------------------------
// Alignment
// ---------------------------------------------------------------------------

/// How the offset was measured and how much to trust it.
#[derive(Debug, Clone)]
pub struct Alignment {
    /// Seconds file 2 starts AFTER file 1 (negative: file 2 starts first).
    pub offset_seconds: f64,
    /// "waveform" (fine pass locked, ~1 ms) or "envelope" (coarse only, ±10 ms).
    pub method: &'static str,
    /// |NCC| at the reported peak (waveform NCC if the fine pass locked,
    /// envelope NCC otherwise), 0..1.
    pub correlation: f64,
    /// Standard score of the envelope peak (peak − mean)/σ over all candidate
    /// lags — same reading as BBC audio-offset-finder: ≥ 10 strong, < 5 weak.
    pub score: f64,
    /// True when the best waveform match has inverted polarity (one recording
    /// chain flips the signal); the offset is still valid.
    pub polarity_inverted: bool,
    /// ±seconds: 0.001 for waveform, 0.01 for envelope.
    pub precision_seconds: f64,
    /// Seconds of audio the two files share at the reported alignment.
    pub overlap_seconds: f64,
    /// True when the peak sits at the edge of the searched lag range (the true
    /// offset may lie outside `max_offset` / the analyzed span).
    pub at_search_edge: bool,
}

/// Map a standard score + waveform-lock to the advertised confidence label.
///
/// The z-score of the best of thousands of candidate lags is ~3.5–4.5 even for
/// UNRELATED recordings (max-of-N inflation) — that is why scores below 5 are
/// unreliable on their own (the same reading BBC audio-offset-finder gives).
/// A locked waveform fine pass is independent evidence, so it upgrades a
/// sub-5 score from "none" to "low".
pub fn confidence_label(score: f64, waveform_locked: bool) -> &'static str {
    if score >= 10.0 {
        "high"
    } else if score >= 5.0 {
        "medium"
    } else if waveform_locked {
        "low"
    } else {
        "none"
    }
}

/// Waveform fine pass: NCC of up to 20 s of the 8 kHz signals, pre-aligned at
/// `coarse_k8`, over ±0.6 s. Returns `(delta_samples, |ncc|, inverted)` when
/// the peak is strong (≥ FINE_MIN_NCC) and not pinned to the search edge.
fn fine_align(a0: &[f32], b0: &[f32], coarse_k8: isize) -> Option<(f64, f64, bool)> {
    let astart = coarse_k8.max(0) as usize;
    let bstart = (-coarse_k8).max(0) as usize;
    if astart >= a0.len() || bstart >= b0.len() {
        return None;
    }
    let overlap = (a0.len() - astart).min(b0.len() - bstart);
    let window = overlap.min((FINE_WINDOW_SECONDS * ANALYSIS_RATE as f64) as usize);
    if window < MIN_FINE_WINDOW {
        return None;
    }
    let center = (overlap - window) / 2;
    let a_seg = &a0[astart + center..astart + center + window];
    let b_seg = &b0[bstart + center..bstart + center + window];
    let raw_f = xcorr_raw(a_seg, b_seg);
    let ncc_f = ncc_from_raw(a_seg, b_seg, &raw_f, window - FINE_MAX_LAG);
    let mid = window - 1; // index of lag 0
    let lo = mid.saturating_sub(FINE_MAX_LAG);
    let hi = (mid + FINE_MAX_LAG).min(ncc_f.len() - 1);
    let mut pk = mid;
    for i in lo..=hi {
        if ncc_f[i].abs() > ncc_f[pk].abs() {
            pk = i;
        }
    }
    let v = ncc_f[pk] as f64;
    if v.abs() >= FINE_MIN_NCC && pk > lo && pk < hi {
        // Parabolic interpolation on the signed NCC around the peak.
        let sign = if v < 0.0 { -1.0 } else { 1.0 };
        let (y0, y1, y2) = (
            ncc_f[pk - 1] as f64 * sign,
            v * sign,
            ncc_f[pk + 1] as f64 * sign,
        );
        let denom = y0 - 2.0 * y1 + y2;
        let delta = if denom.abs() > 1e-12 {
            (0.5 * (y0 - y2) / denom).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        Some(((pk as f64 - mid as f64) + delta, v.abs(), v < 0.0))
    } else {
        None
    }
}

/// Cross-correlate two mono 8 kHz signals (consumed — they are mean-removed in
/// place to stay inside the 64 MiB wasm sandbox). `max_offset_seconds`
/// restricts the search to |offset| ≤ that many seconds (0 = search everything).
pub fn find_offset(
    mut a0: Vec<f32>,
    mut b0: Vec<f32>,
    max_offset_seconds: f64,
) -> Result<Alignment, String> {
    let min_len = (MIN_SECONDS_PER_FILE * ANALYSIS_RATE as f64) as usize;
    if a0.len() < min_len || b0.len() < min_len {
        return Err(format!(
            "each file must contain at least {MIN_SECONDS_PER_FILE:.0} s of audio to correlate"
        ));
    }
    for (label, x) in [("file 1", &a0), ("file 2", &b0)] {
        let rms =
            (x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len() as f64).sqrt();
        if rms < SILENCE_RMS {
            return Err(format!(
                "{label} appears to be silent — there is no audio content to correlate"
            ));
        }
    }

    // Mean-remove in place (DC offsets would dominate the correlation).
    for x in [&mut a0, &mut b0] {
        let dc = x.iter().map(|v| *v as f64).sum::<f64>() / x.len() as f64;
        for v in x.iter_mut() {
            *v = (*v as f64 - dc) as f32;
        }
    }

    // ---- Coarse pass: novelty-envelope NCC over every candidate lag ----
    let ea = envelope(&a0);
    let eb = envelope(&b0);
    let raw = xcorr_raw(&ea, &eb);
    let ncc = ncc_from_raw(&ea, &eb, &raw, MIN_ENV_OVERLAP);
    let max_lag_env: Option<isize> = if max_offset_seconds > 0.0 {
        Some((max_offset_seconds * ENV_RATE as f64).round() as isize)
    } else {
        None
    };
    let lag_of = |i: usize| i as isize - (eb.len() as isize - 1);
    // Per-lag test statistic: NCC × sqrt(overlap points). Under the null a
    // lag's NCC has std ≈ 1/sqrt(overlap), so raw NCC values are NOT
    // comparable across lags (short overlaps are noisier and would dominate
    // both the argmax and the z-score); the stat is ~N(0,1) at every lag.
    let overlap_at = |i: usize| -> usize {
        let k = lag_of(i);
        let t0 = (-k).max(0) as usize;
        let t1 = eb.len().min((ea.len() as isize - k).max(0) as usize);
        t1.saturating_sub(t0)
    };
    let allowed: Vec<usize> = (0..ncc.len())
        .filter(|&i| max_lag_env.is_none_or(|m| lag_of(i).abs() <= m))
        .filter(|&i| overlap_at(i) >= MIN_ENV_OVERLAP)
        .collect();
    if allowed.is_empty() {
        return Err(
            "no candidate alignment has at least 1 s of overlap — the files are too short \
             for the requested max_offset"
                .to_string(),
        );
    }
    let stat_of = |i: usize| ncc[i] as f64 * (overlap_at(i) as f64).sqrt();
    let (mut sum, mut sum2) = (0.0f64, 0.0f64);
    for &i in &allowed {
        let s = stat_of(i);
        sum += s;
        sum2 += s * s;
    }
    let mean = sum / allowed.len() as f64;
    let std = (sum2 / allowed.len() as f64 - mean * mean).max(0.0).sqrt();

    // Top coarse candidates by stat, separated by ≥ 0.5 s, each verified by
    // the waveform fine pass; the best locked candidate wins (SyncSink's
    // verify-by-crosscovariance shape). Fall back to the best coarse stat.
    const CANDIDATES: usize = 5;
    const SEPARATION: isize = ENV_RATE as isize / 2; // 0.5 s
    let mut pool = allowed.clone();
    let mut candidates: Vec<usize> = Vec::new();
    for _ in 0..CANDIDATES {
        let Some(&best) = pool
            .iter()
            .max_by(|&&x, &&y| stat_of(x).total_cmp(&stat_of(y)))
        else {
            break;
        };
        candidates.push(best);
        pool.retain(|&i| (lag_of(i) - lag_of(best)).abs() > SEPARATION);
    }

    let mut chosen = candidates[0];
    let mut fine: Option<(f64, f64, bool)> = None; // (delta_samples, |ncc|, inverted)
    let mut best_fine = 0.0f64;
    for &cand in &candidates {
        let coarse_k8 = lag_of(cand) * ENV_HOP as isize;
        if let Some((delta, nccv, inverted)) = fine_align(&a0, &b0, coarse_k8) {
            if nccv > best_fine {
                best_fine = nccv;
                chosen = cand;
                fine = Some((delta, nccv, inverted));
            }
        }
    }

    let env_lag = lag_of(chosen);
    let score = if std > 1e-9 {
        (stat_of(chosen) - mean) / std
    } else {
        0.0
    };
    // Edge = chosen peak at the first/last allowed lag (search range exhausted).
    let at_search_edge = {
        let lags: Vec<isize> = allowed.iter().map(|&i| lag_of(i)).collect();
        let (lo, hi) = (
            *lags.iter().min().expect("non-empty"),
            *lags.iter().max().expect("non-empty"),
        );
        env_lag == lo || env_lag == hi
    };
    let coarse_k8 = env_lag * ENV_HOP as isize;

    let (offset_samples, method, correlation, precision, polarity_inverted) = match fine {
        Some((delta, nccv, inverted)) => {
            (coarse_k8 as f64 + delta, "waveform", nccv, 0.001, inverted)
        }
        None => (
            coarse_k8 as f64,
            "envelope",
            ncc[chosen] as f64,
            0.01,
            false,
        ),
    };
    let offset_seconds = offset_samples / ANALYSIS_RATE as f64;
    // Overlap at the final alignment.
    let k8 = offset_samples.round() as isize;
    let fa = k8.max(0) as usize;
    let fb = (-k8).max(0) as usize;
    let overlap_seconds = if fa < a0.len() && fb < b0.len() {
        (a0.len() - fa).min(b0.len() - fb) as f64 / ANALYSIS_RATE as f64
    } else {
        0.0
    };

    Ok(Alignment {
        offset_seconds,
        method,
        correlation: correlation.clamp(0.0, 1.0),
        score,
        polarity_inverted,
        precision_seconds: precision,
        overlap_seconds,
        at_search_edge,
    })
}

// ---------------------------------------------------------------------------
// Decode (symphonia → mono 8 kHz)
// ---------------------------------------------------------------------------

/// Incremental integrate-and-dump resampler to 8 kHz mono. Averages every
/// input sample that falls into each 1/8000 s output bin (a crude but
/// effective anti-alias low-pass for correlation); zero-order-holds across
/// bins an upsampled input skips.
struct Resampler {
    rate: f64,
    in_idx: u64,
    acc: f64,
    acc_n: u32,
    last: f32,
    out: Vec<f32>,
    max_out: usize,
}

impl Resampler {
    fn new(rate: u32, max_out: usize) -> Self {
        Resampler {
            rate: rate as f64,
            in_idx: 0,
            acc: 0.0,
            acc_n: 0,
            last: 0.0,
            out: Vec::with_capacity(max_out.min(1 << 22)),
            max_out,
        }
    }

    fn full(&self) -> bool {
        self.out.len() >= self.max_out
    }

    /// Push one mono input sample; emits output bins as their input span ends.
    fn push(&mut self, v: f64) {
        if self.full() {
            return;
        }
        self.acc += v;
        self.acc_n += 1;
        self.in_idx += 1;
        // The bin for output index o spans input [o·rate/8000, (o+1)·rate/8000).
        let next_boundary = (self.out.len() as f64 + 1.0) * self.rate / ANALYSIS_RATE as f64;
        while self.in_idx as f64 >= next_boundary && !self.full() {
            let bin = if self.acc_n > 0 {
                (self.acc / self.acc_n as f64) as f32
            } else {
                self.last // upsampling: hold the previous value
            };
            self.out.push(bin);
            self.last = bin;
            self.acc = 0.0;
            self.acc_n = 0;
            // Re-evaluate: with rate < 8000 several output bins can end within
            // one input sample.
            let nb = (self.out.len() as f64 + 1.0) * self.rate / ANALYSIS_RATE as f64;
            if nb > self.in_idx as f64 {
                break;
            }
        }
    }
}

/// One decoded input: mono 8 kHz analysis signal + stream facts for the report.
#[derive(Debug)]
pub struct DecodedTrack {
    pub mono8k: Vec<f32>,
    pub native_rate: u32,
    pub channels: u16,
    /// Seconds actually analyzed (≤ analyze_seconds).
    pub analyzed_seconds: f64,
    /// True when the file had more audio than the analysis window took.
    pub truncated: bool,
}

const SUPPORTED: &str = "supported containers: MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, \
                         FLAC, MP3, AAC-ADTS; audio codecs: AAC-LC, ALAC, MP3, Vorbis, FLAC, \
                         PCM, ADPCM (Opus, AC-3 and DTS are not supported)";

/// Demux `bytes`, decode the first decodable audio track, downmix to mono and
/// resample to 8 kHz. `label` ("file 1 (name.mp4)") prefixes error messages.
pub fn decode_to_mono_8k(
    label: &str,
    bytes: Vec<u8>,
    analyze_seconds: f64,
) -> Result<DecodedTrack, String> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| {
            format!("{label}: unrecognized or unsupported media format ({SUPPORTED}): {e}")
        })?;
    let mut format = probed.format;

    // Pick the first track a decoder can actually be built for. Video tracks
    // are CODEC_TYPE_NULL; known-but-undecodable audio (e.g. Opus) fails
    // decoder construction and is reported honestly.
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
            format!("{label}: no decodable audio track found (a silent/video-only file cannot be aligned by audio); {SUPPORTED}")
        } else {
            format!("{label}: the audio track's codec is not supported ({}); {SUPPORTED}", seen.join(", "))
        }
    })?;

    let max_out = (analyze_seconds * ANALYSIS_RATE as f64).ceil() as usize;
    let mut rs: Option<Resampler> = None;
    let mut rate: u32 = 0;
    let mut channels: usize = 0;
    let mut truncated = false;
    let mut sbuf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(format!("{label}: error reading audio: {e}")),
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
                    rate = spec.rate;
                    channels = ch;
                    if rate == 0 {
                        return Err(format!("{label}: stream reports a zero sample rate"));
                    }
                    if channels == 0 || channels > 8 {
                        return Err(format!(
                            "{label}: unsupported channel count {channels} (1–8 channels are supported)"
                        ));
                    }
                    rs = Some(Resampler::new(rate, max_out));
                } else if spec.rate != rate || ch != channels {
                    return Err(format!(
                        "{label}: variable sample rate / channel layout is not supported"
                    ));
                }
                let rs = rs.as_mut().expect("resampler initialized");
                if rs.full() {
                    truncated = true;
                    break;
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
                    let mono = frame.iter().map(|v| *v as f64).sum::<f64>() / channels as f64;
                    rs.push(mono);
                }
            }
            // A corrupt packet is skippable; keep going with the rest.
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("{label}: decode failed: {e}")),
        }
    }

    let rs = rs.ok_or_else(|| format!("{label}: no audio could be decoded"))?;
    let mono8k = rs.out;
    if mono8k.is_empty() {
        return Err(format!("{label}: no audio could be decoded"));
    }
    let analyzed_seconds = mono8k.len() as f64 / ANALYSIS_RATE as f64;
    if analyzed_seconds < MIN_SECONDS_PER_FILE {
        return Err(format!(
            "{label}: only {analyzed_seconds:.2} s of audio decoded — at least \
             {MIN_SECONDS_PER_FILE:.0} s are needed to correlate"
        ));
    }
    Ok(DecodedTrack {
        mono8k,
        native_rate: rate,
        channels: channels as u16,
        analyzed_seconds,
        truncated,
    })
}

// ---------------------------------------------------------------------------
// Top-level API
// ---------------------------------------------------------------------------

/// Per-file facts for the response.
#[derive(Debug, Clone)]
pub struct FileFacts {
    pub sample_rate: u32,
    pub channels: u16,
    pub analyzed_seconds: f64,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct SyncReport {
    pub alignment: Alignment,
    pub a: FileFacts,
    pub b: FileFacts,
}

/// Decode both files and find the offset. `a_label`/`b_label` name the files
/// in error messages ("file 1 (cam.mp4)").
pub fn sync_offset(
    a_bytes: Vec<u8>,
    a_label: &str,
    b_bytes: Vec<u8>,
    b_label: &str,
    analyze_seconds: f64,
    max_offset_seconds: f64,
) -> Result<SyncReport, String> {
    if !(MIN_ANALYZE_SECONDS..=MAX_ANALYZE_SECONDS).contains(&analyze_seconds) {
        return Err(format!(
            "analyze_seconds must be between {MIN_ANALYZE_SECONDS:.0} and {MAX_ANALYZE_SECONDS:.0}, got {analyze_seconds}"
        ));
    }
    if !(0.0..=MAX_ANALYZE_SECONDS).contains(&max_offset_seconds) {
        return Err(format!(
            "max_offset must be between 0 (no limit) and {MAX_ANALYZE_SECONDS:.0} seconds, got {max_offset_seconds}"
        ));
    }
    let a = decode_to_mono_8k(a_label, a_bytes, analyze_seconds)?;
    let b = decode_to_mono_8k(b_label, b_bytes, analyze_seconds)?;
    let fa = FileFacts {
        sample_rate: a.native_rate,
        channels: a.channels,
        analyzed_seconds: round2(a.analyzed_seconds),
        truncated: a.truncated,
    };
    let fb = FileFacts {
        sample_rate: b.native_rate,
        channels: b.channels,
        analyzed_seconds: round2(b.analyzed_seconds),
        truncated: b.truncated,
    };
    let alignment = find_offset(a.mono8k, b.mono8k, max_offset_seconds)?;
    Ok(SyncReport {
        alignment,
        a: fa,
        b: fb,
    })
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic white noise in [-1, 1] (xorshift64*).
    struct Rng(u64);
    impl Rng {
        fn next_f(&mut self) -> f64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            let u = (x.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64;
            u / (1u64 << 53) as f64 * 2.0 - 1.0
        }
    }

    /// Master test signal: seeded noise with an APERIODIC slow amplitude
    /// envelope (piecewise-linear between random levels every 0.5 s) so both
    /// the envelope pass and the waveform pass have structure to lock onto.
    fn master(seed: u64, seconds: f64) -> Vec<f32> {
        let n = (seconds * ANALYSIS_RATE as f64) as usize;
        let mut rng = Rng(seed | 1);
        let seg = ANALYSIS_RATE as usize / 2; // 0.5 s
        let n_levels = n / seg + 2;
        let levels: Vec<f64> = (0..n_levels)
            .map(|_| 0.05 + 0.95 * (rng.next_f() * 0.5 + 0.5))
            .collect();
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let pos = i as f64 / seg as f64;
            let idx = pos as usize;
            let frac = pos - idx as f64;
            let amp = levels[idx] * (1.0 - frac) + levels[idx + 1] * frac;
            out.push((rng.next_f() * amp * 0.5) as f32);
        }
        out
    }

    fn slice_s(x: &[f32], from_s: f64, to_s: f64) -> Vec<f32> {
        let f = (from_s * ANALYSIS_RATE as f64) as usize;
        let t = (to_s * ANALYSIS_RATE as f64) as usize;
        x[f..t].to_vec()
    }

    #[test]
    fn fft_round_trips() {
        let mut rng = Rng(7);
        let re0: Vec<f32> = (0..256).map(|_| rng.next_f() as f32).collect();
        let im0: Vec<f32> = (0..256).map(|_| rng.next_f() as f32).collect();
        let (mut re, mut im) = (re0.clone(), im0.clone());
        fft_inplace(&mut re, &mut im, false);
        fft_inplace(&mut re, &mut im, true);
        for i in 0..256 {
            assert!((re[i] - re0[i]).abs() < 1e-4, "re[{i}]");
            assert!((im[i] - im0[i]).abs() < 1e-4, "im[{i}]");
        }
    }

    #[test]
    fn xcorr_matches_direct_computation() {
        let mut rng = Rng(99);
        let a: Vec<f32> = (0..13).map(|_| rng.next_f() as f32).collect();
        let b: Vec<f32> = (0..7).map(|_| rng.next_f() as f32).collect();
        let fast = xcorr_raw(&a, &b);
        assert_eq!(fast.len(), a.len() + b.len() - 1);
        for (i, got) in fast.iter().enumerate() {
            let k = i as isize - (b.len() as isize - 1);
            let mut want = 0f64;
            for (t, bv) in b.iter().enumerate() {
                let ai = t as isize + k;
                if ai >= 0 && (ai as usize) < a.len() {
                    want += a[ai as usize] as f64 * *bv as f64;
                }
            }
            assert!(
                (*got as f64 - want).abs() < 1e-4,
                "lag {k}: fft {got} direct {want}"
            );
        }
    }

    #[test]
    fn finds_positive_offset_to_millisecond() {
        let m = master(42, 30.0);
        let a = slice_s(&m, 0.0, 24.0);
        let b = slice_s(&m, 5.0, 17.0); // file 2 starts 5 s after file 1
        let al = find_offset(a, b, 0.0).expect("alignment");
        assert!(
            (al.offset_seconds - 5.0).abs() < 0.002,
            "offset {} ≠ 5.0",
            al.offset_seconds
        );
        assert_eq!(al.method, "waveform");
        assert!(al.correlation > 0.9, "correlation {}", al.correlation);
        assert_eq!(
            confidence_label(al.score, al.method == "waveform"),
            "high",
            "score {}",
            al.score
        );
        assert!(!al.polarity_inverted);
        assert!(!al.at_search_edge);
        assert!((al.overlap_seconds - 12.0).abs() < 0.1);
    }

    #[test]
    fn finds_negative_offset_when_swapped() {
        let m = master(42, 30.0);
        let a = slice_s(&m, 5.0, 17.0);
        let b = slice_s(&m, 0.0, 24.0);
        let al = find_offset(a, b, 0.0).expect("alignment");
        assert!(
            (al.offset_seconds + 5.0).abs() < 0.002,
            "offset {} ≠ -5.0",
            al.offset_seconds
        );
    }

    #[test]
    fn identical_files_align_at_zero() {
        let m = master(1234, 12.0);
        let al = find_offset(m.clone(), m, 0.0).expect("alignment");
        assert!(
            al.offset_seconds.abs() < 0.001,
            "offset {}",
            al.offset_seconds
        );
        assert_eq!(al.method, "waveform");
        assert!(al.correlation > 0.99);
        assert_eq!(confidence_label(al.score, al.method == "waveform"), "high");
    }

    #[test]
    fn survives_gain_difference_and_added_noise() {
        let m = master(42, 30.0);
        let a = slice_s(&m, 0.0, 24.0);
        let mut rng = Rng(777);
        // 30% level + independent noise at comparable power (≈0 dB SNR).
        let b: Vec<f32> = slice_s(&m, 5.0, 17.0)
            .iter()
            .map(|v| (0.3 * *v as f64 + 0.15 * rng.next_f()) as f32)
            .collect();
        let al = find_offset(a, b, 0.0).expect("alignment");
        assert!(
            (al.offset_seconds - 5.0).abs() < 0.011,
            "offset {} ≠ 5.0",
            al.offset_seconds
        );
        assert_ne!(confidence_label(al.score, al.method == "waveform"), "none");
    }

    #[test]
    fn detects_inverted_polarity() {
        let m = master(42, 30.0);
        let a = slice_s(&m, 0.0, 24.0);
        let b: Vec<f32> = slice_s(&m, 5.0, 17.0).iter().map(|v| -*v).collect();
        let al = find_offset(a, b, 0.0).expect("alignment");
        assert!(
            (al.offset_seconds - 5.0).abs() < 0.002,
            "offset {}",
            al.offset_seconds
        );
        assert_eq!(al.method, "waveform");
        assert!(al.polarity_inverted, "polarity flip not detected");
    }

    #[test]
    fn unrelated_recordings_score_none() {
        let a = master(1, 20.0);
        let b = master(2, 20.0);
        let al = find_offset(a, b, 0.0).expect("alignment");
        assert_eq!(
            confidence_label(al.score, al.method == "waveform"),
            "none",
            "unrelated noise scored {} method {} ({})",
            al.score,
            al.method,
            al.offset_seconds
        );
    }

    #[test]
    fn max_offset_restricts_the_search() {
        let m = master(42, 30.0);
        let a = slice_s(&m, 0.0, 24.0);
        let b = slice_s(&m, 5.0, 17.0); // true offset +5, outside the window
        let al = find_offset(a, b, 2.0).expect("alignment");
        assert!(
            al.offset_seconds.abs() <= 2.7,
            "offset {} escaped max_offset",
            al.offset_seconds
        );
        assert!(
            (al.offset_seconds - 5.0).abs() > 1.0,
            "true offset should be unreachable under max_offset=2"
        );
    }

    #[test]
    fn too_short_input_is_rejected() {
        let m = master(5, 1.0);
        let err = find_offset(m.clone(), m, 0.0).unwrap_err();
        assert!(err.contains("at least 2 s"), "{err}");
    }

    #[test]
    fn silent_input_is_rejected() {
        let z = vec![0f32; 8 * ANALYSIS_RATE as usize];
        let m = master(5, 8.0);
        let err = find_offset(m, z, 0.0).unwrap_err();
        assert!(err.contains("file 2") && err.contains("silent"), "{err}");
    }

    #[test]
    fn resampler_downsamples_by_averaging() {
        // Constant input stays constant; length ≈ n·8000/rate.
        let mut rs = Resampler::new(44100, 1 << 20);
        for _ in 0..44100 {
            rs.push(0.25);
        }
        let out = rs.out;
        assert!(
            (out.len() as i64 - 8000).unsigned_abs() <= 1,
            "len {}",
            out.len()
        );
        assert!(out.iter().all(|v| (*v - 0.25).abs() < 1e-6));
    }

    #[test]
    fn resampler_upsamples_with_hold() {
        let mut rs = Resampler::new(4000, 1 << 20);
        for i in 0..4000 {
            rs.push(if i % 2 == 0 { 1.0 } else { -1.0 });
        }
        let out = rs.out;
        assert!(
            (out.len() as i64 - 8000).unsigned_abs() <= 2,
            "len {}",
            out.len()
        );
    }

    #[test]
    fn resampler_respects_output_cap() {
        let mut rs = Resampler::new(8000, 100);
        for _ in 0..1000 {
            rs.push(0.5);
        }
        assert_eq!(rs.out.len(), 100);
        assert!(rs.full());
    }

    #[test]
    fn garbage_bytes_are_rejected() {
        let err = decode_to_mono_8k("file 1 (x.bin)", vec![0xDE; 4096], 60.0).unwrap_err();
        assert!(
            err.contains("unrecognized") && err.contains("file 1 (x.bin)"),
            "{err}"
        );
    }

    #[test]
    fn sync_offset_validates_parameter_ranges() {
        let e = sync_offset(vec![], "file 1", vec![], "file 2", 3.0, 0.0).unwrap_err();
        assert!(e.contains("analyze_seconds"), "{e}");
        let e = sync_offset(vec![], "file 1", vec![], "file 2", 120.0, -1.0).unwrap_err();
        assert!(e.contains("max_offset"), "{e}");
        let e = sync_offset(vec![], "file 1", vec![], "file 2", 120.0, 500.0).unwrap_err();
        assert!(e.contains("max_offset"), "{e}");
    }

    #[test]
    fn confidence_labels_match_advertised_thresholds() {
        assert_eq!(confidence_label(15.0, true), "high");
        assert_eq!(confidence_label(10.0, false), "high");
        assert_eq!(confidence_label(7.0, false), "medium");
        assert_eq!(confidence_label(5.0, true), "medium");
        assert_eq!(confidence_label(4.9, true), "low");
        assert_eq!(confidence_label(4.9, false), "none");
        assert_eq!(confidence_label(1.0, false), "none");
    }
}
