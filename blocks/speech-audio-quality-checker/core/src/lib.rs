//! speech-audio-quality-checker core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Decodes a short audio clip (given as base64 or hex bytes), parses it as an
//! uncompressed PCM/IEEE-float WAV (RIFF/WAVE), measures the signal properties
//! that matter for automatic speech recognition (ASR) — sample rate, channel
//! count, duration, peak/RMS level, an estimated signal-to-noise ratio, and
//! clipping — and grades each against transcription-readiness thresholds.
//!
//! Only uncompressed WAV is decoded here (PCM 8/16/24/32-bit integer and
//! 32/64-bit IEEE float). Compressed containers/codecs (MP3, AAC/M4A,
//! Opus/Ogg, FLAC, A-law/mu-law) are rejected with a clear message rather than
//! guessed at — see `parse_wav`.

/// Analyze one audio clip and render a readiness report or a JSON object.
///
/// - `input`: the audio file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `output`: `"report"` (default, human-readable) or `"json"`.
/// - `target_sample_rate`: the sample rate ASR wants (Hz); the file's rate is
///   PASS if ≥ this, WARN if between 8000 and this, FAIL below 8000.
/// - `min_snr_db`: the SNR (dB) required to PASS; 10–`min` WARNs, below 10 FAILs.
/// - `max_clipping_pct`: the clipped-sample percentage allowed to PASS.
/// - `clipping_threshold`: |sample| at/above this fraction of full scale
///   (0.8–1.0) counts as clipped.
///
/// Returns a user-facing error string for undecodable input, a non-WAV or
/// compressed file, or a malformed WAV.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    target_sample_rate: u32,
    min_snr_db: f64,
    max_clipping_pct: f64,
    clipping_threshold: f64,
) -> Result<String, String> {
    let out_mode = match output.trim() {
        "" | "report" => OutputMode::Report,
        "json" => OutputMode::Json,
        other => {
            return Err(format!(
                "invalid output {other:?}: expected \"report\" or \"json\""
            ))
        }
    };
    if !(0.8..=1.0).contains(&clipping_threshold) {
        return Err(format!(
            "invalid clipping_threshold {clipping_threshold}: expected 0.8-1.0"
        ));
    }
    if !(0.0..=60.0).contains(&min_snr_db) {
        return Err(format!("invalid min_snr_db {min_snr_db}: expected 0-60"));
    }
    if !(0.0..=100.0).contains(&max_clipping_pct) {
        return Err(format!(
            "invalid max_clipping_pct {max_clipping_pct}: expected 0-100"
        ));
    }
    if !(8000..=48000).contains(&target_sample_rate) {
        return Err(format!(
            "invalid target_sample_rate {target_sample_rate}: expected 8000-48000"
        ));
    }

    let bytes = decode_bytes(input, input_format)?;
    let wav = parse_wav(&bytes)?;
    let metrics = analyze(&wav, clipping_threshold);
    let checks = grade(
        &wav,
        &metrics,
        target_sample_rate,
        min_snr_db,
        max_clipping_pct,
    );
    let verdict = Verdict::from_checks(&checks);

    Ok(match out_mode {
        OutputMode::Report => render_report(
            &wav,
            &metrics,
            &checks,
            &verdict,
            target_sample_rate,
            min_snr_db,
            max_clipping_pct,
            clipping_threshold,
        ),
        OutputMode::Json => render_json(
            &wav,
            &metrics,
            &checks,
            &verdict,
            target_sample_rate,
            min_snr_db,
            max_clipping_pct,
            clipping_threshold,
        ),
    })
}

enum OutputMode {
    Report,
    Json,
}

// ---------------------------------------------------------------------------
// WAV parsing
// ---------------------------------------------------------------------------

/// Decoded, normalized audio. `samples` are interleaved across channels and
/// scaled to the range [-1.0, 1.0] regardless of the source bit depth.
struct WavData {
    format_name: String,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    /// Interleaved, normalized to [-1.0, 1.0].
    samples: Vec<f32>,
}

impl WavData {
    /// Number of sample frames (one frame = one sample per channel).
    fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / self.channels as usize
        }
    }
    fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frames() as f64 / self.sample_rate as f64
        }
    }
}

fn u16_le(b: &[u8], off: usize) -> u16 {
    (b[off] as u16) | ((b[off + 1] as u16) << 8)
}
fn u32_le(b: &[u8], off: usize) -> u32 {
    (b[off] as u32)
        | ((b[off + 1] as u32) << 8)
        | ((b[off + 2] as u32) << 16)
        | ((b[off + 3] as u32) << 24)
}

/// Parse a single RIFF/WAVE file: the 12-byte RIFF header, then the `fmt ` and
/// `data` chunks (other chunks — `LIST`, `fact`, `bext`, … — are skipped).
fn parse_wav(b: &[u8]) -> Result<WavData, String> {
    if b.len() < 12 {
        return Err(format!(
            "not a WAV file: only {} bytes, too short for a RIFF header",
            b.len()
        ));
    }
    if &b[0..4] != b"RIFF" {
        return Err(sniff_container(b));
    }
    if &b[8..12] != b"WAVE" {
        return Err("not a WAV file: RIFF header is not of type WAVE".into());
    }

    let mut pos = 12usize;
    let mut fmt: Option<FmtChunk> = None;
    let mut data: Option<&[u8]> = None;

    while pos + 8 <= b.len() {
        let id = &b[pos..pos + 4];
        let size = u32_le(b, pos + 4) as usize;
        let body_start = pos + 8;
        // A truncated final chunk: clamp so we still read what's present.
        let body_end = body_start.saturating_add(size).min(b.len());
        let body = &b[body_start..body_end];
        match id {
            b"fmt " => fmt = Some(parse_fmt(body)?),
            b"data" => data = Some(body),
            _ => {}
        }
        // Chunks are word-aligned: an odd size is padded with one byte.
        let advance = 8 + size + (size & 1);
        pos = match pos.checked_add(advance) {
            Some(p) => p,
            None => break,
        };
    }

    let fmt = fmt.ok_or("malformed WAV: no `fmt ` chunk found")?;
    let data = data.ok_or("malformed WAV: no `data` chunk found")?;

    if fmt.channels == 0 {
        return Err("malformed WAV: channel count is 0".into());
    }
    if fmt.sample_rate == 0 {
        return Err("malformed WAV: sample rate is 0".into());
    }

    let (samples, format_name) = decode_samples(&fmt, data)?;
    Ok(WavData {
        format_name,
        sample_rate: fmt.sample_rate,
        channels: fmt.channels,
        bits_per_sample: fmt.bits_per_sample,
        samples,
    })
}

/// Best-effort identification of a non-RIFF file so the error names the codec
/// the user pasted instead of a generic "not a WAV".
fn sniff_container(b: &[u8]) -> String {
    let starts = |sig: &[u8]| b.len() >= sig.len() && &b[..sig.len()] == sig;
    let named = if starts(b"OggS") {
        Some("an Ogg container (Vorbis/Opus)")
    } else if starts(b"fLaC") {
        Some("a FLAC file")
    } else if starts(b"ID3") || (b.len() >= 2 && b[0] == 0xFF && (b[1] & 0xE0) == 0xE0) {
        Some("an MP3 file")
    } else if b.len() >= 12 && &b[4..8] == b"ftyp" {
        Some("an MP4/M4A container (AAC/ALAC)")
    } else if starts(b"FORM") {
        Some("an AIFF file")
    } else {
        None
    };
    match named {
        Some(what) => format!(
            "unsupported format: this looks like {what}, but only uncompressed \
             WAV (RIFF/WAVE) is decoded. Convert it to WAV first."
        ),
        None => "not a WAV file: missing the 'RIFF' signature. Only uncompressed \
                 WAV (RIFF/WAVE) is supported; convert other formats to WAV first."
            .into(),
    }
}

struct FmtChunk {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
}

// WAVE format tags.
const WAVE_FORMAT_PCM: u16 = 0x0001;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
const WAVE_FORMAT_ALAW: u16 = 0x0006;
const WAVE_FORMAT_MULAW: u16 = 0x0007;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

fn parse_fmt(body: &[u8]) -> Result<FmtChunk, String> {
    if body.len() < 16 {
        return Err("malformed WAV: `fmt ` chunk is shorter than 16 bytes".into());
    }
    let mut audio_format = u16_le(body, 0);
    let channels = u16_le(body, 2);
    let sample_rate = u32_le(body, 4);
    let bits_per_sample = u16_le(body, 14);

    // WAVE_FORMAT_EXTENSIBLE stores the real tag in the first 2 bytes of the
    // SubFormat GUID inside the extension.
    if audio_format == WAVE_FORMAT_EXTENSIBLE && body.len() >= 26 {
        let ext_size = u16_le(body, 16) as usize;
        if ext_size >= 22 && body.len() >= 18 + 22 {
            audio_format = u16_le(body, 24);
        }
    }
    Ok(FmtChunk {
        audio_format,
        channels,
        sample_rate,
        bits_per_sample,
    })
}

/// Decode raw `data` bytes to interleaved f32 in [-1.0, 1.0] and describe the
/// sample format for the report.
fn decode_samples(fmt: &FmtChunk, data: &[u8]) -> Result<(Vec<f32>, String), String> {
    match fmt.audio_format {
        WAVE_FORMAT_PCM => match fmt.bits_per_sample {
            8 => Ok((decode_pcm8(data), "PCM 8-bit integer WAV".into())),
            16 => Ok((decode_pcm16(data), "PCM 16-bit integer WAV".into())),
            24 => Ok((decode_pcm24(data), "PCM 24-bit integer WAV".into())),
            32 => Ok((decode_pcm32(data), "PCM 32-bit integer WAV".into())),
            other => Err(format!(
                "unsupported PCM bit depth: {other}-bit. Supported: 8, 16, 24, 32-bit integer."
            )),
        },
        WAVE_FORMAT_IEEE_FLOAT => match fmt.bits_per_sample {
            32 => Ok((decode_f32(data), "IEEE 32-bit float WAV".into())),
            64 => Ok((decode_f64(data), "IEEE 64-bit float WAV".into())),
            other => Err(format!(
                "unsupported IEEE-float bit depth: {other}-bit. Supported: 32 and 64-bit float."
            )),
        },
        WAVE_FORMAT_ALAW => Err("unsupported format: A-law compressed WAV. \
             Convert to uncompressed PCM WAV first."
            .into()),
        WAVE_FORMAT_MULAW => Err("unsupported format: mu-law compressed WAV. \
             Convert to uncompressed PCM WAV first."
            .into()),
        other => Err(format!(
            "unsupported WAV format tag 0x{other:04x}: only uncompressed PCM \
             (0x0001) and IEEE float (0x0003) are decoded."
        )),
    }
}

fn decode_pcm8(data: &[u8]) -> Vec<f32> {
    // 8-bit WAV PCM is unsigned, centered at 128.
    data.iter().map(|&b| (b as f32 - 128.0) / 128.0).collect()
}
fn decode_pcm16(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(2)
        .map(|c| {
            let v = i16::from_le_bytes([c[0], c[1]]);
            v as f32 / 32768.0
        })
        .collect()
}
fn decode_pcm24(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(3)
        .map(|c| {
            // Sign-extend a 24-bit little-endian sample into i32.
            let raw = (c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16);
            let v = (raw << 8) >> 8;
            v as f32 / 8_388_608.0
        })
        .collect()
}
fn decode_pcm32(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|c| {
            let v = i32::from_le_bytes([c[0], c[1], c[2], c[3]]);
            v as f32 / 2_147_483_648.0
        })
        .collect()
}
fn decode_f32(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
fn decode_f64(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(8)
        .map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]) as f32)
        .collect()
}

// ---------------------------------------------------------------------------
// Signal analysis
// ---------------------------------------------------------------------------

/// Floor for dBFS readouts: a fully-silent signal reports this instead of
/// -infinity, keeping the output finite and comparable.
const DBFS_FLOOR: f64 = -120.0;

struct Metrics {
    peak_dbfs: f64,
    rms_dbfs: f64,
    snr_db: f64,
    clipping_pct: f64,
    longest_clipped_run: usize,
    clipped_samples: usize,
    total_samples: usize,
}

fn amp_to_dbfs(amp: f64) -> f64 {
    if amp <= 0.0 {
        DBFS_FLOOR
    } else {
        (20.0 * amp.log10()).max(DBFS_FLOOR)
    }
}

fn analyze(wav: &WavData, clipping_threshold: f64) -> Metrics {
    let total = wav.samples.len();

    // Peak + RMS + clipping over the interleaved sample stream.
    let mut peak = 0.0f64;
    let mut sum_sq = 0.0f64;
    let mut clipped = 0usize;
    let mut longest_run = 0usize;
    let mut cur_run = 0usize;
    let thr = clipping_threshold as f32;
    for &s in &wav.samples {
        let a = s.abs();
        if (a as f64) > peak {
            peak = a as f64;
        }
        sum_sq += (s as f64) * (s as f64);
        if a >= thr {
            clipped += 1;
            cur_run += 1;
            if cur_run > longest_run {
                longest_run = cur_run;
            }
        } else {
            cur_run = 0;
        }
    }
    let rms = if total > 0 {
        (sum_sq / total as f64).sqrt()
    } else {
        0.0
    };
    let clipping_pct = if total > 0 {
        clipped as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    Metrics {
        peak_dbfs: amp_to_dbfs(peak),
        rms_dbfs: amp_to_dbfs(rms),
        snr_db: estimate_snr(wav),
        clipping_pct,
        longest_clipped_run: longest_run,
        clipped_samples: clipped,
        total_samples: total,
    }
}

/// Estimate SNR with a documented percentile / noise-floor heuristic.
///
/// The clip is downmixed to mono and split into short (20 ms) frames. Each
/// frame's RMS level is measured in dBFS. Speech has loud voiced frames and
/// quiet pauses, so the distribution of frame levels separates signal from
/// noise: we take the 90th percentile of frame levels as the signal level and
/// the 10th percentile as the noise floor. The estimate is
/// `SNR ≈ signal_level − noise_floor`, clamped to ≥ 0.
///
/// This is a level-statistics estimate, not a true speech-vs-noise SNR (it has
/// no voice-activity model), and it needs several frames to be meaningful — a
/// clip shorter than two frames returns a low-confidence value of 0.
fn estimate_snr(wav: &WavData) -> f64 {
    let ch = wav.channels.max(1) as usize;
    let frames_total = wav.frames();
    if frames_total == 0 {
        return 0.0;
    }
    // Downmix to mono, one value per sample frame.
    let mut mono = Vec::with_capacity(frames_total);
    for f in 0..frames_total {
        let base = f * ch;
        let mut acc = 0.0f64;
        for c in 0..ch {
            acc += wav.samples[base + c] as f64;
        }
        mono.push(acc / ch as f64);
    }

    let frame_len = ((wav.sample_rate as f64 * 0.02) as usize).max(1);
    let mut frame_db: Vec<f64> = Vec::new();
    for chunk in mono.chunks(frame_len) {
        if chunk.is_empty() {
            continue;
        }
        let sum_sq: f64 = chunk.iter().map(|x| x * x).sum();
        let rms = (sum_sq / chunk.len() as f64).sqrt();
        frame_db.push(amp_to_dbfs(rms));
    }
    if frame_db.len() < 2 {
        return 0.0;
    }
    frame_db.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let signal = percentile(&frame_db, 0.90);
    let noise = percentile(&frame_db, 0.10);
    (signal - noise).max(0.0)
}

/// Nearest-rank percentile of an ascending-sorted slice. `p` in [0, 1].
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// ---------------------------------------------------------------------------
// Grading
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Pass,
    Warn,
    Fail,
}

impl Status {
    fn tag(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
        }
    }
}

struct Check {
    name: &'static str,
    status: Status,
    detail: String,
}

fn grade(
    wav: &WavData,
    m: &Metrics,
    target_sample_rate: u32,
    min_snr_db: f64,
    max_clipping_pct: f64,
) -> Vec<Check> {
    let mut checks = Vec::new();

    // Sample rate.
    let (sr_status, sr_detail) = if wav.sample_rate >= target_sample_rate {
        (
            Status::Pass,
            format!(
                "{} Hz meets the {} Hz target",
                wav.sample_rate, target_sample_rate
            ),
        )
    } else if wav.sample_rate >= 8000 {
        (
            Status::Warn,
            format!(
                "{} Hz is below the {} Hz target — upsampling adds no detail",
                wav.sample_rate, target_sample_rate
            ),
        )
    } else {
        (
            Status::Fail,
            format!("{} Hz is below the 8000 Hz ASR minimum", wav.sample_rate),
        )
    };
    checks.push(Check {
        name: "Sample rate",
        status: sr_status,
        detail: sr_detail,
    });

    // Channels.
    let (ch_status, ch_detail) = if wav.channels == 1 {
        (Status::Pass, "mono".to_string())
    } else {
        (
            Status::Warn,
            format!(
                "{} channels — ASR downmixes to mono (extra size, no accuracy gain)",
                wav.channels
            ),
        )
    };
    checks.push(Check {
        name: "Channels",
        status: ch_status,
        detail: ch_detail,
    });

    // SNR.
    let (snr_status, snr_detail) = if m.snr_db >= min_snr_db {
        (
            Status::Pass,
            format!(
                "{:.1} dB meets the {:.0} dB target ({})",
                m.snr_db,
                min_snr_db,
                snr_band(m.snr_db)
            ),
        )
    } else if m.snr_db >= 10.0 {
        (
            Status::Warn,
            format!(
                "{:.1} dB is below the {:.0} dB target ({})",
                m.snr_db,
                min_snr_db,
                snr_band(m.snr_db)
            ),
        )
    } else {
        (
            Status::Fail,
            format!("{:.1} dB is very noisy ({})", m.snr_db, snr_band(m.snr_db)),
        )
    };
    checks.push(Check {
        name: "SNR",
        status: snr_status,
        detail: snr_detail,
    });

    // Clipping.
    let (clip_status, clip_detail) = if m.clipping_pct <= max_clipping_pct {
        (
            Status::Pass,
            format!(
                "{:.2}% clipped is within the {:.2}% limit",
                m.clipping_pct, max_clipping_pct
            ),
        )
    } else if m.clipping_pct <= max_clipping_pct * 5.0 {
        (
            Status::Warn,
            format!(
                "{:.2}% clipped exceeds the {:.2}% limit (longest run {} samples)",
                m.clipping_pct, max_clipping_pct, m.longest_clipped_run
            ),
        )
    } else {
        (
            Status::Fail,
            format!(
                "{:.2}% clipped — heavy distortion (longest run {} samples)",
                m.clipping_pct, m.longest_clipped_run
            ),
        )
    };
    checks.push(Check {
        name: "Clipping",
        status: clip_status,
        detail: clip_detail,
    });

    // Level (loudness) — very quiet audio is mostly noise; near-0 dBFS risks clipping.
    let (lvl_status, lvl_detail) = if m.peak_dbfs < -30.0 {
        (
            Status::Warn,
            format!(
                "peak {:.1} dBFS is very quiet — the recording may be mostly noise",
                m.peak_dbfs
            ),
        )
    } else {
        (
            Status::Pass,
            format!("peak {:.1} dBFS, RMS {:.1} dBFS", m.peak_dbfs, m.rms_dbfs),
        )
    };
    checks.push(Check {
        name: "Level",
        status: lvl_status,
        detail: lvl_detail,
    });

    checks
}

fn snr_band(snr: f64) -> &'static str {
    if snr >= 40.0 {
        "very clean"
    } else if snr >= 20.0 {
        "clean"
    } else if snr >= 10.0 {
        "noisy"
    } else {
        "very noisy"
    }
}

enum Verdict {
    Ready,
    Caveats,
    NotReady,
}

impl Verdict {
    fn from_checks(checks: &[Check]) -> Verdict {
        if checks.iter().any(|c| c.status == Status::Fail) {
            Verdict::NotReady
        } else if checks.iter().any(|c| c.status == Status::Warn) {
            Verdict::Caveats
        } else {
            Verdict::Ready
        }
    }
    fn line(&self) -> &'static str {
        match self {
            Verdict::Ready => "READY for ASR / transcription",
            Verdict::Caveats => "USABLE with caveats — see the WARN checks above",
            Verdict::NotReady => "NOT READY — fix the FAIL checks before transcribing",
        }
    }
    fn code(&self) -> &'static str {
        match self {
            Verdict::Ready => "ready",
            Verdict::Caveats => "usable_with_caveats",
            Verdict::NotReady => "not_ready",
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn render_report(
    wav: &WavData,
    m: &Metrics,
    checks: &[Check],
    verdict: &Verdict,
    target_sample_rate: u32,
    min_snr_db: f64,
    max_clipping_pct: f64,
    clipping_threshold: f64,
) -> String {
    let mut s = String::new();
    s.push_str("Speech audio quality — ASR readiness\n");
    s.push_str("====================================\n\n");
    let ch_label = match wav.channels {
        1 => "mono".to_string(),
        2 => "stereo".to_string(),
        n => format!("{n} channels"),
    };
    s.push_str(&format!("Format:        {}\n", wav.format_name));
    s.push_str(&format!("Sample rate:   {} Hz\n", wav.sample_rate));
    s.push_str(&format!("Channels:      {} ({})\n", wav.channels, ch_label));
    s.push_str(&format!("Duration:      {:.3} s\n", wav.duration_secs()));
    s.push_str(&format!("Peak level:    {:.2} dBFS\n", m.peak_dbfs));
    s.push_str(&format!("RMS level:     {:.2} dBFS\n", m.rms_dbfs));
    s.push_str(&format!(
        "Estimated SNR: {:.1} dB ({})\n",
        m.snr_db,
        snr_band(m.snr_db)
    ));
    s.push_str(&format!(
        "Clipping:      {:.2}% ({}/{} samples, longest run {})\n",
        m.clipping_pct, m.clipped_samples, m.total_samples, m.longest_clipped_run
    ));

    s.push_str("\nChecks\n");
    for c in checks {
        s.push_str(&format!(
            "  [{}] {:<12} {}\n",
            c.status.tag(),
            c.name,
            c.detail
        ));
    }

    s.push_str(&format!("\nVerdict: {}\n", verdict.line()));

    s.push_str(&format!(
        "\nThresholds: target rate {} Hz, min SNR {:.0} dB, max clipping {:.2}%, \
         clip threshold {:.2} FS\n",
        target_sample_rate, min_snr_db, max_clipping_pct, clipping_threshold
    ));
    s.push_str(
        "SNR is a 20 ms-frame percentile estimate (90th − 10th percentile level), \
         not a voice-activity SNR.\n",
    );
    s
}

#[allow(clippy::too_many_arguments)]
fn render_json(
    wav: &WavData,
    m: &Metrics,
    checks: &[Check],
    verdict: &Verdict,
    target_sample_rate: u32,
    min_snr_db: f64,
    max_clipping_pct: f64,
    clipping_threshold: f64,
) -> String {
    let mut s = String::new();
    s.push('{');
    s.push_str(&format!("\"format\":{},", json_str(&wav.format_name)));
    s.push_str(&format!("\"sample_rate\":{},", wav.sample_rate));
    s.push_str(&format!("\"channels\":{},", wav.channels));
    s.push_str(&format!("\"bits_per_sample\":{},", wav.bits_per_sample));
    s.push_str(&format!(
        "\"duration_secs\":{},",
        fmt_num(wav.duration_secs(), 3)
    ));
    s.push_str(&format!("\"peak_dbfs\":{},", fmt_num(m.peak_dbfs, 2)));
    s.push_str(&format!("\"rms_dbfs\":{},", fmt_num(m.rms_dbfs, 2)));
    s.push_str(&format!("\"snr_db\":{},", fmt_num(m.snr_db, 1)));
    s.push_str(&format!("\"snr_band\":{},", json_str(snr_band(m.snr_db))));
    s.push_str(&format!("\"clipping_pct\":{},", fmt_num(m.clipping_pct, 2)));
    s.push_str(&format!("\"clipped_samples\":{},", m.clipped_samples));
    s.push_str(&format!("\"total_samples\":{},", m.total_samples));
    s.push_str(&format!(
        "\"longest_clipped_run\":{},",
        m.longest_clipped_run
    ));

    s.push_str("\"checks\":[");
    for (i, c) in checks.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"name\":{},\"status\":{},\"detail\":{}}}",
            json_str(c.name),
            json_str(c.status.tag()),
            json_str(&c.detail)
        ));
    }
    s.push_str("],");

    s.push_str(&format!("\"verdict\":{},", json_str(verdict.code())));
    s.push_str("\"thresholds\":{");
    s.push_str(&format!("\"target_sample_rate\":{},", target_sample_rate));
    s.push_str(&format!("\"min_snr_db\":{},", fmt_num(min_snr_db, 2)));
    s.push_str(&format!(
        "\"max_clipping_pct\":{},",
        fmt_num(max_clipping_pct, 2)
    ));
    s.push_str(&format!(
        "\"clipping_threshold\":{}",
        fmt_num(clipping_threshold, 2)
    ));
    s.push('}');
    s.push('}');
    s
}

/// Format a float with `decimals` fixed places, normalizing `-0` to `0`.
fn fmt_num(v: f64, decimals: usize) -> String {
    let mut out = format!("{:.*}", decimals, v);
    // Collapse "-0.00" → "0.00".
    if out.starts_with('-') && out[1..].chars().all(|c| c == '0' || c == '.') {
        out.remove(0);
    }
    out
}

/// Minimal JSON string escaper for the (controlled) text we emit.
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// Byte decoding (hex / base64)
// ---------------------------------------------------------------------------

fn decode_bytes(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty: paste the audio bytes as base64 or hex".into());
    }
    match input_format.trim() {
        "" | "base64" => decode_base64(trimmed),
        "hex" => decode_hex(trimmed),
        other => Err(format!(
            "invalid input_format {other:?}: expected \"base64\" or \"hex\""
        )),
    }
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
        _ => Err(format!("invalid hex digit {:?}", c as char)),
    }
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
    let mut out = Vec::new();
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c);
        if v == INVALID {
            return Err(format!("invalid base64 character {:?}", c as char));
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal PCM WAV from interleaved f32 samples in [-1, 1].
    fn make_wav(sample_rate: u32, channels: u16, bits: u16, samples: &[f32]) -> Vec<u8> {
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut data: Vec<u8> = Vec::new();
        for &s in samples {
            let s = s.clamp(-1.0, 1.0);
            match bits {
                8 => data.push(((s * 127.0) as i32 + 128) as u8),
                16 => data.extend_from_slice(&((s * 32767.0) as i16).to_le_bytes()),
                24 => {
                    let v = (s * 8_388_607.0) as i32;
                    data.extend_from_slice(&v.to_le_bytes()[0..3]);
                }
                32 => data.extend_from_slice(&((s as f64 * 2_147_483_647.0) as i32).to_le_bytes()),
                _ => unreachable!(),
            }
        }
        build_riff(
            WAVE_FORMAT_PCM,
            channels,
            sample_rate,
            bits,
            byte_rate,
            block_align,
            &data,
        )
    }

    fn make_float_wav(sample_rate: u32, channels: u16, samples: &[f32]) -> Vec<u8> {
        let bits = 32u16;
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut data = Vec::new();
        for &s in samples {
            data.extend_from_slice(&s.to_le_bytes());
        }
        build_riff(
            WAVE_FORMAT_IEEE_FLOAT,
            channels,
            sample_rate,
            bits,
            byte_rate,
            block_align,
            &data,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build_riff(
        fmt_tag: u16,
        channels: u16,
        sample_rate: u32,
        bits: u16,
        byte_rate: u32,
        block_align: u16,
        data: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&fmt_tag.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
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
                TABLE[n as usize & 63] as char
            } else {
                '='
            });
        }
        out
    }

    /// A clip with loud voiced frames followed by quiet frames → high SNR.
    fn speechy(n: usize) -> Vec<f32> {
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            let loud = i < n * 6 / 10;
            let phase = i as f32 * 0.3;
            let amp = if loud { 0.5 } else { 0.001 };
            v.push(amp * phase.sin());
        }
        v
    }

    #[test]
    fn happy_report_mono_16k() {
        let wav = make_wav(16000, 1, 16, &speechy(4000));
        let out = run(&b64(&wav), "base64", "report", 16000, 20.0, 1.0, 0.99).unwrap();
        assert!(out.contains("Sample rate:   16000 Hz"), "{out}");
        assert!(out.contains("Channels:      1 (mono)"), "{out}");
        assert!(out.contains("PCM 16-bit integer WAV"), "{out}");
        assert!(out.contains("Verdict: READY"), "{out}");
        assert!(out.contains("[PASS] Sample rate"), "{out}");
    }

    #[test]
    fn json_output_has_fields() {
        let wav = make_wav(16000, 1, 16, &speechy(4000));
        let out = run(&b64(&wav), "base64", "json", 16000, 20.0, 1.0, 0.99).unwrap();
        assert!(out.starts_with('{') && out.ends_with('}'), "{out}");
        assert!(out.contains("\"sample_rate\":16000"), "{out}");
        assert!(out.contains("\"channels\":1"), "{out}");
        assert!(out.contains("\"verdict\":\"ready\""), "{out}");
        assert!(out.contains("\"snr_db\":"), "{out}");
        assert_eq!(out.matches('{').count(), out.matches('}').count());
    }

    #[test]
    fn low_sample_rate_fails() {
        let wav = make_wav(4000, 1, 16, &speechy(1000));
        let out = run(&b64(&wav), "base64", "report", 16000, 20.0, 1.0, 0.99).unwrap();
        assert!(out.contains("[FAIL] Sample rate"), "{out}");
        assert!(out.contains("Verdict: NOT READY"), "{out}");
    }

    #[test]
    fn stereo_warns_but_can_be_ready() {
        let mono = speechy(4000);
        let mut stereo = Vec::with_capacity(mono.len() * 2);
        for &s in &mono {
            stereo.push(s);
            stereo.push(s);
        }
        let wav = make_wav(16000, 2, 16, &stereo);
        let out = run(&b64(&wav), "base64", "report", 16000, 20.0, 1.0, 0.99).unwrap();
        assert!(out.contains("[WARN] Channels"), "{out}");
        assert!(out.contains("Verdict: USABLE"), "{out}");
    }

    #[test]
    fn clipping_is_detected() {
        let clipped = vec![1.0f32; 2000];
        let wav = make_wav(16000, 1, 16, &clipped);
        let out = run(&b64(&wav), "base64", "json", 16000, 20.0, 1.0, 0.99).unwrap();
        assert!(out.contains("\"clipping_pct\":100.00"), "{out}");
        assert!(out.contains("\"longest_clipped_run\":2000"), "{out}");
        assert!(out.contains("\"verdict\":\"not_ready\""), "{out}");
    }

    #[test]
    fn parses_all_pcm_depths_and_float() {
        for bits in [8u16, 16, 24, 32] {
            let wav = make_wav(16000, 1, bits, &[0.5, -0.5, 0.25, -0.25]);
            let out = run(&b64(&wav), "base64", "json", 16000, 20.0, 1.0, 0.99).unwrap();
            assert!(
                out.contains(&format!("\"bits_per_sample\":{bits}")),
                "bits {bits}: {out}"
            );
        }
        let fwav = make_float_wav(16000, 1, &[0.5, -0.5, 0.25, -0.25]);
        let out = run(&b64(&fwav), "base64", "json", 16000, 20.0, 1.0, 0.99).unwrap();
        assert!(out.contains("IEEE 32-bit float WAV"), "{out}");
    }

    #[test]
    fn hex_input_works() {
        let wav = make_wav(16000, 1, 16, &[0.5, -0.5]);
        let hex: String = wav.iter().map(|b| format!("{b:02x}")).collect();
        let out = run(&hex, "hex", "report", 16000, 20.0, 1.0, 0.99).unwrap();
        assert!(out.contains("16000 Hz"), "{out}");
    }

    #[test]
    fn peak_dbfs_full_scale_is_zero() {
        let wav = make_wav(16000, 1, 16, &[1.0, -1.0, 1.0, -1.0]);
        let out = run(&b64(&wav), "base64", "json", 16000, 20.0, 1.0, 0.99).unwrap();
        assert!(out.contains("\"peak_dbfs\":0.00"), "{out}");
    }

    #[test]
    fn error_invalid_base64() {
        let err = run("not base64 @@@", "base64", "report", 16000, 20.0, 1.0, 0.99).unwrap_err();
        assert!(err.contains("base64"), "{err}");
    }

    #[test]
    fn error_odd_hex() {
        let err = run("abc", "hex", "report", 16000, 20.0, 1.0, 0.99).unwrap_err();
        assert!(err.contains("odd number"), "{err}");
    }

    #[test]
    fn error_not_a_wav() {
        let out = run(
            &b64(b"hello world not a wav at all"),
            "base64",
            "report",
            16000,
            20.0,
            1.0,
            0.99,
        );
        assert!(out.is_err());
        assert!(out.unwrap_err().contains("not a WAV"));
    }

    #[test]
    fn error_compressed_alaw() {
        let data = vec![0u8; 16];
        let out = build_riff(WAVE_FORMAT_ALAW, 1, 8000, 8, 8000, 1, &data);
        let err = run(&b64(&out), "base64", "report", 16000, 20.0, 1.0, 0.99).unwrap_err();
        assert!(err.contains("A-law"), "{err}");
    }

    #[test]
    fn error_mp3_sniffed() {
        let err = run(
            &b64(b"ID3\x04\x00\x00\x00\x00\x00\x00rest"),
            "base64",
            "report",
            16000,
            20.0,
            1.0,
            0.99,
        )
        .unwrap_err();
        assert!(err.contains("MP3"), "{err}");
    }

    #[test]
    fn error_bad_output_mode() {
        let wav = make_wav(16000, 1, 16, &[0.1]);
        let err = run(&b64(&wav), "base64", "xml", 16000, 20.0, 1.0, 0.99).unwrap_err();
        assert!(err.contains("output"), "{err}");
    }

    #[test]
    fn error_clip_threshold_out_of_range() {
        let wav = make_wav(16000, 1, 16, &[0.1]);
        let err = run(&b64(&wav), "base64", "report", 16000, 20.0, 1.0, 0.5).unwrap_err();
        assert!(err.contains("clipping_threshold"), "{err}");
    }

    #[test]
    fn skips_unknown_chunks() {
        let data: Vec<u8> = (0..8).flat_map(|_| 100i16.to_le_bytes()).collect();
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((36 + 16 + data.len()) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&WAVE_FORMAT_PCM.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&16000u32.to_le_bytes());
        out.extend_from_slice(&32000u32.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        // LIST chunk (8-byte body) inserted before data — must be skipped.
        out.extend_from_slice(b"LIST");
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(b"INFOjunk");
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        let res = run(&b64(&out), "base64", "json", 16000, 20.0, 1.0, 0.99).unwrap();
        assert!(res.contains("\"sample_rate\":16000"), "{res}");
    }
}
