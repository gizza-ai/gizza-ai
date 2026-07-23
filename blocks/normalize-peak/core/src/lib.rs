//! gizza-ai/normalize-peak core — pure DSP shared by the chat skill block and the
//! CLI. No wafer/wasm-bindgen deps.
//!
//! Sample-peak normalization: find the loudest single sample in a file (its
//! sample peak, in dBFS) and apply ONE constant gain so that peak lands exactly
//! on a chosen target dBFS. Dynamics are untouched — only the overall level
//! changes. This is distinct from loudness (LUFS) normalization and from a
//! fixed-dB volume change: it *measures* the peak and computes the exact gain.
//!
//! Decode is symphonia (wav/flac/mp3/ogg-vorbis/m4a → interleaved f32 PCM);
//! output is always a WAV (16/24-bit int or 32-bit float) via hound.
//!
//! Sized for the 64 MiB wasm sandbox: ≤ 16 MiB decoded PCM (~47 s of 44.1 kHz
//! stereo) and ≤ 16 MiB output WAV. Peak normalization to a target ≤ 0 dBFS can
//! never clip (the loudest sample lands at or below full scale by construction).

use std::io::Cursor;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decoded interleaved-f32 PCM cap (16 MiB ≈ 47 s of 44.1 kHz stereo).
pub const MAX_PCM_BYTES: usize = 16 * 1024 * 1024;
/// Uncompressed output WAV cap, sized so the WAV + its base64 envelope fit the
/// 64 MiB wasm sandbox.
pub const MAX_OUTPUT_WAV_BYTES: usize = 16 * 1024 * 1024;

/// Output WAV sample format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitDepth {
    Int16,
    Int24,
    Float32,
}

pub fn parse_bit_depth(s: &str) -> Result<BitDepth, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "16" => Ok(BitDepth::Int16),
        "24" => Ok(BitDepth::Int24),
        "32f" => Ok(BitDepth::Float32),
        other => Err(format!(
            "unknown bit_depth '{other}' (expected 16, 24 or 32f)"
        )),
    }
}

impl BitDepth {
    pub fn bytes_per_sample(self) -> usize {
        match self {
            BitDepth::Int16 => 2,
            BitDepth::Int24 => 3,
            BitDepth::Float32 => 4,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            BitDepth::Int16 => "16-bit PCM",
            BitDepth::Int24 => "24-bit PCM",
            BitDepth::Float32 => "32-bit float",
        }
    }
}

/// Interleaved f32 PCM.
#[derive(Debug)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub rate: u32,
    pub channels: u16,
}

impl DecodedAudio {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }
    pub fn duration_secs(&self) -> f64 {
        if self.rate == 0 {
            0.0
        } else {
            self.frames() as f64 / self.rate as f64
        }
    }
}

/// Decode any supported container/codec to interleaved f32 PCM.
pub fn decode_audio(bytes: Vec<u8>) -> Result<DecodedAudio, String> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| {
            format!("unrecognized or unsupported audio format (wav, flac, mp3, ogg-vorbis and m4a are supported): {e}")
        })?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| "no decodable audio track found".to_string())?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("unsupported audio codec: {e}"))?;

    let mut samples: Vec<f32> = Vec::new();
    let mut rate: u32 = 0;
    let mut channels: usize = 0;
    let mut sbuf: Option<SampleBuffer<f32>> = None;

    loop {
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
                    rate = spec.rate;
                    channels = ch;
                    if channels == 0 || channels > 2 {
                        return Err(format!(
                            "only mono or stereo audio is supported (found {channels} channels)"
                        ));
                    }
                } else if spec.rate != rate || ch != channels {
                    return Err("variable sample rate / channel layout is not supported".to_string());
                }
                let needed = decoded.capacity() as u64;
                let recreate = match &sbuf {
                    Some(b) => b.capacity() < decoded.capacity() * channels,
                    None => true,
                };
                if recreate {
                    sbuf = Some(SampleBuffer::<f32>::new(needed, spec));
                }
                let buf = sbuf.as_mut().expect("sample buffer just created");
                buf.copy_interleaved_ref(decoded);
                let add = buf.samples().len();
                let elem = std::mem::size_of::<f32>();
                if (samples.len() + add) * elem > MAX_PCM_BYTES {
                    return Err(format!(
                        "decodes to more than {} MiB of PCM (~47 s of 44.1 kHz stereo). \
                         Trim the file to a shorter section (e.g. with an audio trim tool) and retry.",
                        MAX_PCM_BYTES / (1024 * 1024)
                    ));
                }
                if samples.capacity() < samples.len() + add {
                    // Grow in bounded 4 MiB steps, never past the cap: Vec's
                    // amortized DOUBLING would hold ~16+32 MiB alive during the
                    // realloc near the cap and OOM-trap the 64 MiB wasm sandbox.
                    let step = 4 * 1024 * 1024 / elem;
                    let target = (samples.len() + add)
                        .max(samples.len() + step)
                        .min(MAX_PCM_BYTES / elem);
                    samples.reserve_exact(target - samples.len());
                }
                samples.extend_from_slice(buf.samples());
            }
            // A corrupt packet is skippable; keep going with the rest.
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode failed: {e}")),
        }
    }

    if samples.is_empty() || rate == 0 {
        return Err("no audio could be decoded".to_string());
    }
    Ok(DecodedAudio {
        samples,
        rate,
        channels: channels as u16,
    })
}

/// Everything the block/CLI needs to build its summary.
#[derive(Debug)]
pub struct Normalized {
    pub wav: Vec<u8>,
    /// Overall measured sample peak (dBFS), max across channels, AFTER any DC
    /// removal but BEFORE gain.
    pub measured_peak_dbfs: f64,
    /// Per-channel measured sample peak (dBFS), pre-gain, post-DC-removal.
    pub channel_peaks_dbfs: Vec<f64>,
    /// Applied gain (dB). For per-channel mode this is the gain of the channel
    /// that was previously the quietest (the largest boost); see
    /// `channel_gains_db` for the exact per-channel gains.
    pub applied_gain_db: f64,
    /// Per-channel applied gain (dB). In linked mode every entry is equal.
    pub channel_gains_db: Vec<f64>,
    /// Resulting overall sample peak (dBFS) after gain — equals the target
    /// (within rounding) unless a channel was silent.
    pub new_peak_dbfs: f64,
    /// DC offset removed from each channel (linear, -1..1). All-zero when
    /// `remove_dc` was false.
    pub dc_removed: Vec<f64>,
    pub per_channel: bool,
    pub removed_dc: bool,
    pub target_dbfs: f64,
    pub rate: u32,
    pub channels: u16,
    pub duration_secs: f64,
    pub report: String,
}

fn to_dbfs(linear: f64) -> f64 {
    if linear > 0.0 {
        20.0 * linear.log10()
    } else {
        f64::NEG_INFINITY
    }
}

fn fmt_db(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.2}")
    } else {
        "-inf".to_string()
    }
}

fn fmt_gain(g: f64) -> String {
    if g.abs() < 1e-9 {
        "0.00 dB (unchanged)".to_string()
    } else {
        format!("{g:+.2} dB")
    }
}

/// Encode interleaved f32 PCM to a WAV at `depth`. Target-peak output never
/// clips (peak ≤ 1.0 by construction), but int quantization still clamps
/// defensively.
fn encode_wav(
    samples: &[f32],
    rate: u32,
    channels: u16,
    depth: BitDepth,
) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: match depth {
            BitDepth::Int16 => 16,
            BitDepth::Int24 => 24,
            BitDepth::Float32 => 32,
        },
        sample_format: match depth {
            BitDepth::Float32 => hound::SampleFormat::Float,
            _ => hound::SampleFormat::Int,
        },
    };
    let mut out: Vec<u8> = Vec::new();
    {
        let cursor = Cursor::new(&mut out);
        let mut writer =
            hound::WavWriter::new(cursor, spec).map_err(|e| format!("WAV writer init: {e}"))?;
        for &s in samples {
            let v = s as f64;
            match depth {
                BitDepth::Int16 => {
                    let q = (v * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
                    writer.write_sample(q).map_err(|e| format!("WAV write: {e}"))?;
                }
                BitDepth::Int24 => {
                    let q = (v * 8_388_608.0).round().clamp(-8_388_608.0, 8_388_607.0) as i32;
                    writer.write_sample(q).map_err(|e| format!("WAV write: {e}"))?;
                }
                BitDepth::Float32 => {
                    writer
                        .write_sample(v as f32)
                        .map_err(|e| format!("WAV write: {e}"))?;
                }
            }
        }
        writer.finalize().map_err(|e| format!("WAV finalize: {e}"))?;
    }
    Ok(out)
}

/// The whole pipeline: decode → (optional DC removal) → measure sample peak →
/// scale to `target_dbfs` (linked or per-channel) → encode WAV.
///
/// `target_dbfs` must be in -60..=0. Refuses digital silence (a file whose
/// loudest sample is exactly 0).
pub fn normalize_peak(
    bytes: Vec<u8>,
    target_dbfs: f64,
    remove_dc: bool,
    per_channel: bool,
    depth: BitDepth,
) -> Result<Normalized, String> {
    if !(-60.0..=0.0).contains(&target_dbfs) {
        return Err(format!(
            "target must be between -60 and 0 dBFS, got {target_dbfs}"
        ));
    }
    let audio = decode_audio(bytes)?;
    let rate = audio.rate;
    let channels = audio.channels as usize;
    let duration_secs = audio.duration_secs();
    let mut samples = audio.samples;

    // Output-size precheck before any further work.
    let est_out = 44 + samples.len() * depth.bytes_per_sample();
    if est_out > MAX_OUTPUT_WAV_BYTES {
        return Err(format!(
            "output would be ~{:.1} MiB of WAV (cap {} MiB). Use a shorter section or a lower bit_depth.",
            est_out as f64 / (1024.0 * 1024.0),
            MAX_OUTPUT_WAV_BYTES / (1024 * 1024)
        ));
    }

    // Optional DC-offset removal: subtract each channel's mean so the waveform
    // is centred on 0 before the peak is measured (matches Audacity's option).
    let mut dc_removed = vec![0.0f64; channels];
    if remove_dc {
        for ch in 0..channels {
            let mut sum = 0.0f64;
            let mut n = 0u64;
            let mut i = ch;
            while i < samples.len() {
                sum += samples[i] as f64;
                n += 1;
                i += channels;
            }
            let mean = if n > 0 { sum / n as f64 } else { 0.0 };
            dc_removed[ch] = mean;
            if mean != 0.0 {
                let mut i = ch;
                while i < samples.len() {
                    samples[i] = (samples[i] as f64 - mean) as f32;
                    i += channels;
                }
            }
        }
    }

    // Measure per-channel sample peak (max |sample|).
    let mut channel_peaks = vec![0.0f64; channels];
    for (i, &s) in samples.iter().enumerate() {
        let ch = i % channels;
        let a = (s as f64).abs();
        if a > channel_peaks[ch] {
            channel_peaks[ch] = a;
        }
    }
    let overall_peak = channel_peaks.iter().cloned().fold(0.0f64, f64::max);
    if overall_peak <= 0.0 {
        return Err(
            "the file is digital silence (loudest sample is 0) — there is no peak to normalize"
                .to_string(),
        );
    }

    let target_linear = 10f64.powf(target_dbfs / 20.0);

    // Compute the per-channel gain, then apply.
    let mut channel_gains_lin = vec![1.0f64; channels];
    if per_channel {
        for ch in 0..channels {
            channel_gains_lin[ch] = if channel_peaks[ch] > 0.0 {
                target_linear / channel_peaks[ch]
            } else {
                // A silent channel among non-silent ones: cannot scale silence.
                1.0
            };
        }
    } else {
        let g = target_linear / overall_peak;
        for ch in 0..channels {
            channel_gains_lin[ch] = g;
        }
    }

    for (i, s) in samples.iter_mut().enumerate() {
        let ch = i % channels;
        *s = (*s as f64 * channel_gains_lin[ch]) as f32;
    }

    // Resulting peak per channel (post-gain) and overall.
    let new_channel_peaks: Vec<f64> = channel_peaks
        .iter()
        .zip(&channel_gains_lin)
        .map(|(p, g)| p * g)
        .collect();
    let new_overall = new_channel_peaks.iter().cloned().fold(0.0f64, f64::max);

    let channel_peaks_dbfs: Vec<f64> = channel_peaks.iter().map(|&p| to_dbfs(p)).collect();
    let channel_gains_db: Vec<f64> = channel_gains_lin.iter().map(|&g| to_dbfs(g)).collect();
    // Headline applied gain: the largest (loudest boost) so the summary reflects
    // the most-changed channel; equals every channel in linked mode.
    let applied_gain_db = channel_gains_db
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    let wav = encode_wav(&samples, rate, channels as u16, depth)?;
    if wav.len() > MAX_OUTPUT_WAV_BYTES {
        return Err(format!(
            "output WAV is {:.1} MiB (cap {} MiB). Use a shorter section or a lower bit_depth.",
            wav.len() as f64 / (1024.0 * 1024.0),
            MAX_OUTPUT_WAV_BYTES / (1024 * 1024)
        ));
    }

    // Build the human report.
    let chan_word = if channels == 1 { "mono" } else { "stereo" };
    let mut report = String::new();
    report.push_str("peak normalization\n==================\n");
    report.push_str(&format!(
        "target: {target_dbfs:.2} dBFS ({} sample peak)\noutput: {} WAV\nsource: {:.2} s @ {} Hz, {}\nDC removal: {}\nmode: {}\n\n",
        if per_channel { "each channel to" } else { "loudest channel to" },
        depth.label(),
        duration_secs,
        rate,
        chan_word,
        if remove_dc { "on" } else { "off" },
        if per_channel {
            "per-channel (each channel scaled to hit the target itself)"
        } else {
            "linked (one common gain preserves the L/R balance)"
        },
    ));
    report.push_str(&format!(
        "measured sample peak: {} dBFS\napplied gain: {}\nnew sample peak: {} dBFS\n",
        fmt_db(to_dbfs(overall_peak)),
        fmt_gain(applied_gain_db),
        fmt_db(to_dbfs(new_overall)),
    ));
    if channels > 1 {
        for ch in 0..channels {
            report.push_str(&format!(
                "  channel {}: peak {} dBFS -> gain {} -> {} dBFS{}\n",
                ch + 1,
                fmt_db(channel_peaks_dbfs[ch]),
                fmt_gain(channel_gains_db[ch]),
                fmt_db(to_dbfs(new_channel_peaks[ch])),
                if remove_dc && dc_removed[ch] != 0.0 {
                    format!(" (DC {:+.4} removed)", dc_removed[ch])
                } else {
                    String::new()
                },
            ));
        }
    } else if remove_dc && dc_removed[0] != 0.0 {
        report.push_str(&format!("DC offset removed: {:+.4}\n", dc_removed[0]));
    }

    Ok(Normalized {
        wav,
        measured_peak_dbfs: to_dbfs(overall_peak),
        channel_peaks_dbfs,
        applied_gain_db,
        channel_gains_db,
        new_peak_dbfs: to_dbfs(new_overall),
        dc_removed,
        per_channel,
        removed_dc: remove_dc,
        target_dbfs,
        rate,
        channels: channels as u16,
        duration_secs,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stereo WAV: left channel a sine at `amp_l`, right at `amp_r`, plus an
    /// optional constant DC offset added to both channels. 16-bit, `secs` long.
    fn stereo_wav(amp_l: f32, amp_r: f32, dc: f32, secs: f32, rate: u32) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut out = Vec::new();
        {
            let mut w = hound::WavWriter::new(Cursor::new(&mut out), spec).unwrap();
            let n = (secs * rate as f32) as usize;
            for i in 0..n {
                let t = i as f32 / rate as f32;
                let base = (t * 440.0 * std::f32::consts::TAU).sin();
                let l = (base * amp_l + dc).clamp(-1.0, 1.0);
                let r = (base * amp_r + dc).clamp(-1.0, 1.0);
                w.write_sample((l * 32767.0) as i16).unwrap();
                w.write_sample((r * 32767.0) as i16).unwrap();
            }
            w.finalize().unwrap();
        }
        out
    }

    fn mono_wav(amp: f32, secs: f32, rate: u32) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut out = Vec::new();
        {
            let mut w = hound::WavWriter::new(Cursor::new(&mut out), spec).unwrap();
            let n = (secs * rate as f32) as usize;
            for i in 0..n {
                let t = i as f32 / rate as f32;
                let s = (t * 440.0 * std::f32::consts::TAU).sin() * amp;
                w.write_sample((s * 32767.0) as i16).unwrap();
            }
            w.finalize().unwrap();
        }
        out
    }

    fn decoded_peak_dbfs(wav: Vec<u8>) -> f64 {
        let d = decode_audio(wav).unwrap();
        let peak = d.samples.iter().fold(0f32, |m, &s| m.max(s.abs())) as f64;
        to_dbfs(peak)
    }

    #[test]
    fn linked_normalize_brings_peak_to_target() {
        // Loudest sample well below full scale; normalize up to -1 dBFS.
        let wav = mono_wav(0.25, 1.0, 44100);
        let m = normalize_peak(wav, -1.0, false, false, BitDepth::Float32).unwrap();
        assert!(m.measured_peak_dbfs < -6.0, "peak was {}", m.measured_peak_dbfs);
        assert!(m.applied_gain_db > 0.0, "should boost: {}", m.applied_gain_db);
        assert!(
            (m.new_peak_dbfs - -1.0).abs() < 0.05,
            "new peak should hit target: {}",
            m.new_peak_dbfs
        );
        // Verify against the actually-decoded output (32f preserves it exactly).
        let out = decoded_peak_dbfs(m.wav);
        assert!((out - -1.0).abs() < 0.1, "decoded peak {out}");
    }

    #[test]
    fn linked_mode_preserves_channel_balance() {
        // L twice as loud as R. Linked scaling keeps the 6 dB gap; only the
        // loudest channel (L) reaches the target.
        let wav = stereo_wav(0.5, 0.25, 0.0, 1.0, 44100);
        let m = normalize_peak(wav, -1.0, false, false, BitDepth::Float32).unwrap();
        assert!(!m.per_channel);
        let gap = m.channel_peaks_dbfs[0] - m.channel_peaks_dbfs[1];
        assert!((gap - 6.0).abs() < 0.3, "L should be ~6 dB above R: {gap}");
        // Same gain on both channels.
        assert!(
            (m.channel_gains_db[0] - m.channel_gains_db[1]).abs() < 1e-6,
            "linked gain must be equal per channel"
        );
        assert!((m.new_peak_dbfs - -1.0).abs() < 0.05);
    }

    #[test]
    fn per_channel_mode_hits_target_on_every_channel() {
        let wav = stereo_wav(0.5, 0.25, 0.0, 1.0, 44100);
        let m = normalize_peak(wav, -3.0, false, true, BitDepth::Float32).unwrap();
        assert!(m.per_channel);
        // Each channel's post-gain peak sits at the target.
        for ch in 0..2 {
            let after = m.channel_peaks_dbfs[ch] + m.channel_gains_db[ch];
            assert!(
                (after - -3.0).abs() < 0.05,
                "channel {ch} should hit target, got {after}"
            );
        }
        // The two gains differ (quieter channel boosted more).
        assert!(
            (m.channel_gains_db[1] - m.channel_gains_db[0]).abs() > 3.0,
            "per-channel gains should differ"
        );
    }

    #[test]
    fn remove_dc_recenters_before_scaling() {
        // Sine at 0.3 with a +0.2 DC offset. Without DC removal the peak is
        // 0.5 (0.3+0.2); with it, the peak is 0.3.
        let with_dc = stereo_wav(0.3, 0.3, 0.2, 1.0, 44100);
        let off = normalize_peak(with_dc.clone(), -1.0, false, false, BitDepth::Float32).unwrap();
        let on = normalize_peak(with_dc, -1.0, true, false, BitDepth::Float32).unwrap();
        assert!(on.removed_dc);
        assert!(
            (on.dc_removed[0] - 0.2).abs() < 0.01,
            "should report ~0.2 DC removed: {}",
            on.dc_removed[0]
        );
        // Measured peak with DC removal is lower (the offset no longer counts).
        assert!(
            on.measured_peak_dbfs < off.measured_peak_dbfs - 3.0,
            "DC removal should reduce measured peak: {} vs {}",
            on.measured_peak_dbfs,
            off.measured_peak_dbfs
        );
        // Both still land on the target.
        assert!((on.new_peak_dbfs - -1.0).abs() < 0.05);
        // Output is DC-centred: the decoded mean is ~0.
        let d = decode_audio(on.wav).unwrap();
        let mean: f64 = d.samples.iter().map(|&s| s as f64).sum::<f64>() / d.samples.len() as f64;
        assert!(mean.abs() < 0.01, "output should be DC-centred, mean {mean}");
    }

    #[test]
    fn target_zero_dbfs_maximizes_without_error() {
        let wav = mono_wav(0.4, 1.0, 44100);
        let m = normalize_peak(wav, 0.0, false, false, BitDepth::Int16).unwrap();
        assert!((m.new_peak_dbfs - 0.0).abs() < 0.05, "peak {}", m.new_peak_dbfs);
        // 16-bit output round-trips near full scale without overflow.
        let out = decoded_peak_dbfs(m.wav);
        assert!(out > -0.2 && out <= 0.0, "decoded peak {out}");
    }

    #[test]
    fn silence_is_refused() {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut silent = Vec::new();
        {
            let mut w = hound::WavWriter::new(Cursor::new(&mut silent), spec).unwrap();
            for _ in 0..(44100 * 2) {
                w.write_sample(0i16).unwrap();
            }
            w.finalize().unwrap();
        }
        let err = normalize_peak(silent, -1.0, false, false, BitDepth::Int16).unwrap_err();
        assert!(err.contains("digital silence"), "{err}");
    }

    #[test]
    fn garbage_bytes_are_a_clear_error() {
        let err = normalize_peak(vec![0u8; 64], -1.0, false, false, BitDepth::Int16).unwrap_err();
        assert!(err.contains("unrecognized or unsupported"), "{err}");
    }

    #[test]
    fn target_out_of_range_is_rejected() {
        let wav = mono_wav(0.5, 0.5, 44100);
        let err = normalize_peak(wav.clone(), 3.0, false, false, BitDepth::Int16).unwrap_err();
        assert!(err.contains("between -60 and 0"), "{err}");
        let err = normalize_peak(wav, -80.0, false, false, BitDepth::Int16).unwrap_err();
        assert!(err.contains("between -60 and 0"), "{err}");
    }

    #[test]
    fn all_bit_depths_write_the_advertised_wav_format() {
        for (depth, bits, fmt) in [
            (BitDepth::Int16, 16, hound::SampleFormat::Int),
            (BitDepth::Int24, 24, hound::SampleFormat::Int),
            (BitDepth::Float32, 32, hound::SampleFormat::Float),
        ] {
            let wav = stereo_wav(0.4, 0.4, 0.0, 0.5, 44100);
            let m = normalize_peak(wav, -1.0, false, false, depth).unwrap();
            let r = hound::WavReader::new(Cursor::new(m.wav)).unwrap();
            assert_eq!(r.spec().bits_per_sample, bits);
            assert_eq!(r.spec().sample_format, fmt);
            assert_eq!(r.spec().channels, 2);
        }
    }

    #[test]
    fn bit_depth_parser_rejects_unknowns() {
        assert_eq!(parse_bit_depth("16").unwrap(), BitDepth::Int16);
        assert_eq!(parse_bit_depth("24").unwrap(), BitDepth::Int24);
        assert_eq!(parse_bit_depth("32f").unwrap(), BitDepth::Float32);
        assert!(parse_bit_depth("8").unwrap_err().contains("unknown bit_depth"));
    }

    #[test]
    fn report_states_measured_gain_and_new_peak() {
        let wav = mono_wav(0.25, 0.5, 44100);
        let m = normalize_peak(wav, -1.0, false, false, BitDepth::Int16).unwrap();
        assert!(m.report.contains("measured sample peak:"), "{}", m.report);
        assert!(m.report.contains("applied gain:"), "{}", m.report);
        assert!(m.report.contains("new sample peak:"), "{}", m.report);
    }
}
