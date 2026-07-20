//! gizza-ai/loudness-matched-ab-prep core — pure DSP shared by the chat skill
//! block and the CLI. No wafer/wasm-bindgen deps.
//!
//! Measures the integrated loudness (ITU-R BS.1770 / EBU R128, via the pure-Rust
//! `ebur128` crate) of two audio files, computes the gain that levels them, and
//! re-encodes BOTH through the identical PCM→WAV pipeline so they can be A/B
//! compared without a loudness bias ("louder sounds better"). Decode is
//! symphonia (wav/flac/mp3/ogg-vorbis/m4a); output is a STORED zip with
//! `a-matched.wav`, `b-matched.wav` and `report.txt`.
//!
//! Sized for the 64 MiB wasm sandbox: ≤ 16 MiB decoded PCM per file (~47 s of
//! 44.1 kHz stereo) and ≤ 10 MiB total output WAV (~30 s per file at 16-bit
//! 44.1 kHz stereo) — this preps short representative A/B sections, not whole
//! albums. Gain is LINEAR only (no limiter): `mode=quieter` never boosts;
//! `mode=target` may boost and hard-clamps int output (clipped samples are
//! counted and reported).

use std::io::{Cursor, Write};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decoded interleaved-f32 PCM cap per file (16 MiB ≈ 47 s of 44.1 kHz stereo).
pub const MAX_PCM_BYTES_PER_FILE: usize = 16 * 1024 * 1024;
/// EBU R128 gating needs 400 ms blocks; anything under a few seconds measures
/// noise, so refuse it outright.
pub const MIN_SECONDS_PER_FILE: f64 = 3.0;
/// Total uncompressed output WAV cap (both files + report), sized so the zip +
/// its base64 envelope fit the 64 MiB wasm sandbox.
pub const MAX_OUTPUT_WAV_BYTES: usize = 10 * 1024 * 1024;

/// How the two files are levelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Turn the LOUDER file down to the quieter one's loudness (never boosts).
    Quieter,
    /// Gain BOTH files to an explicit LUFS target (may boost).
    Target,
}

pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "quieter" => Ok(Mode::Quieter),
        "target" => Ok(Mode::Target),
        other => Err(format!(
            "unknown mode '{other}' (expected quieter or target)"
        )),
    }
}

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

/// Per-file measurements (all BEFORE any gain).
#[derive(Debug, Clone, Copy)]
pub struct TrackStats {
    /// Integrated loudness, LUFS (BS.1770 K-weighted, gated).
    pub lufs: f64,
    /// True peak in dBTP (4x oversampled), max across channels.
    pub true_peak_dbtp: f64,
    /// Loudness range, LU.
    pub lra: f64,
    /// Plain RMS level in dBFS (all channels pooled).
    pub rms_dbfs: f64,
    pub duration_secs: f64,
    pub rate: u32,
    pub channels: u16,
}

/// Decode any supported container/codec to interleaved f32 PCM.
/// `label` is "file A"/"file B" (with the filename) for error messages.
pub fn decode_audio(label: &str, bytes: Vec<u8>) -> Result<DecodedAudio, String> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| {
            format!("{label}: unrecognized or unsupported audio format (wav, flac, mp3, ogg-vorbis and m4a are supported): {e}")
        })?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| format!("{label}: no decodable audio track found"))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("{label}: unsupported audio codec: {e}"))?;

    let mut samples: Vec<f32> = Vec::new();
    let mut rate: u32 = 0;
    let mut channels: usize = 0;
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
                    if channels == 0 || channels > 2 {
                        return Err(format!(
                            "{label}: only mono or stereo audio is supported (found {channels} channels)"
                        ));
                    }
                } else if spec.rate != rate || ch != channels {
                    return Err(format!(
                        "{label}: variable sample rate / channel layout is not supported"
                    ));
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
                if (samples.len() + add) * elem > MAX_PCM_BYTES_PER_FILE {
                    return Err(format!(
                        "{label}: decodes to more than {} MiB of PCM (~47 s of 44.1 kHz stereo). \
                         This tool preps short A/B sections — trim each file to ≤ 30 s (e.g. with an audio trim tool) and retry.",
                        MAX_PCM_BYTES_PER_FILE / (1024 * 1024)
                    ));
                }
                if samples.capacity() < samples.len() + add {
                    // Grow in bounded 4 MiB steps, never past the cap: Vec's
                    // amortized DOUBLING would hold ~16+32 MiB alive during the
                    // realloc near the cap and OOM-trap the 64 MiB wasm sandbox
                    // (seen with a long m4a before this guard).
                    let step = 4 * 1024 * 1024 / elem;
                    let target = (samples.len() + add)
                        .max(samples.len() + step)
                        .min(MAX_PCM_BYTES_PER_FILE / elem);
                    samples.reserve_exact(target - samples.len());
                }
                samples.extend_from_slice(buf.samples());
            }
            // A corrupt packet is skippable; keep going with the rest.
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("{label}: decode failed: {e}")),
        }
    }

    if samples.is_empty() || rate == 0 {
        return Err(format!("{label}: no audio could be decoded"));
    }
    Ok(DecodedAudio {
        samples,
        rate,
        channels: channels as u16,
    })
}

/// Measure integrated loudness / true peak / LRA / RMS of decoded PCM.
pub fn measure(label: &str, audio: &DecodedAudio) -> Result<TrackStats, String> {
    if audio.duration_secs() < MIN_SECONDS_PER_FILE {
        return Err(format!(
            "{label}: too short to loudness-match ({:.1} s — EBU R128 gating needs at least {MIN_SECONDS_PER_FILE:.0} s)",
            audio.duration_secs()
        ));
    }
    let mut ebu = ebur128::EbuR128::new(
        audio.channels as u32,
        audio.rate,
        ebur128::Mode::I | ebur128::Mode::LRA | ebur128::Mode::TRUE_PEAK,
    )
    .map_err(|e| format!("{label}: loudness analyzer init failed: {e:?}"))?;
    ebu.add_frames_f32(&audio.samples)
        .map_err(|e| format!("{label}: loudness analysis failed: {e:?}"))?;
    let lufs = ebu
        .loudness_global()
        .map_err(|e| format!("{label}: loudness analysis failed: {e:?}"))?;
    if !lufs.is_finite() {
        return Err(format!(
            "{label}: is silent or too quiet to measure (integrated loudness fell below the -70 LUFS gate)"
        ));
    }
    let lra = ebu
        .loudness_range()
        .map_err(|e| format!("{label}: loudness-range analysis failed: {e:?}"))?;
    let mut tp_linear: f64 = 0.0;
    for ch in 0..audio.channels as u32 {
        let p = ebu
            .true_peak(ch)
            .map_err(|e| format!("{label}: true-peak analysis failed: {e:?}"))?;
        tp_linear = tp_linear.max(p);
    }
    let true_peak_dbtp = if tp_linear > 0.0 {
        20.0 * tp_linear.log10()
    } else {
        f64::NEG_INFINITY
    };
    let sum_sq: f64 = audio.samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    let rms = (sum_sq / audio.samples.len() as f64).sqrt();
    let rms_dbfs = if rms > 0.0 {
        20.0 * rms.log10()
    } else {
        f64::NEG_INFINITY
    };
    Ok(TrackStats {
        lufs,
        true_peak_dbtp,
        lra,
        rms_dbfs,
        duration_secs: audio.duration_secs(),
        rate: audio.rate,
        channels: audio.channels,
    })
}

/// Apply `gain_db` (linear, no limiter) and encode a WAV at `depth`.
/// Returns (wav bytes, clipped-sample count). Int depths hard-clamp; 32f
/// preserves overs (they're still COUNTED so the caller can warn).
pub fn encode_wav(
    audio: &DecodedAudio,
    gain_db: f64,
    depth: BitDepth,
) -> Result<(Vec<u8>, u64), String> {
    let gain = 10f64.powf(gain_db / 20.0);
    let spec = hound::WavSpec {
        channels: audio.channels,
        sample_rate: audio.rate,
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
    let mut clipped: u64 = 0;
    {
        let cursor = Cursor::new(&mut out);
        let mut writer =
            hound::WavWriter::new(cursor, spec).map_err(|e| format!("WAV writer init: {e}"))?;
        for &s in &audio.samples {
            let v = s as f64 * gain;
            if v > 1.0 || v < -1.0 {
                clipped += 1;
            }
            match depth {
                BitDepth::Int16 => {
                    let q = (v * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
                    writer
                        .write_sample(q)
                        .map_err(|e| format!("WAV write: {e}"))?;
                }
                BitDepth::Int24 => {
                    let q = (v * 8_388_608.0).round().clamp(-8_388_608.0, 8_388_607.0) as i32;
                    writer
                        .write_sample(q)
                        .map_err(|e| format!("WAV write: {e}"))?;
                }
                BitDepth::Float32 => {
                    writer
                        .write_sample(v as f32)
                        .map_err(|e| format!("WAV write: {e}"))?;
                }
            }
        }
        writer
            .finalize()
            .map_err(|e| format!("WAV finalize: {e}"))?;
    }
    Ok((out, clipped))
}

/// Everything the block needs to build its summary + the zip itself.
/// (Debug elides the multi-MiB zip — it's only used by test `unwrap_err`s.)
#[derive(Debug)]
pub struct MatchedPair {
    pub zip: Vec<u8>,
    pub a: TrackStats,
    pub b: TrackStats,
    pub gain_a_db: f64,
    pub gain_b_db: f64,
    pub clipped_a: u64,
    pub clipped_b: u64,
    /// The loudness both files sit at after matching.
    pub matched_lufs: f64,
    pub report: String,
}

fn fmt_db(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.2}")
    } else {
        "-inf".to_string()
    }
}

fn fmt_gain(g: f64) -> String {
    if g == 0.0 {
        "0.00 dB (unchanged)".to_string()
    } else {
        format!("{g:+.2} dB")
    }
}

fn channel_word(ch: u16) -> &'static str {
    if ch == 1 {
        "mono"
    } else {
        "stereo"
    }
}

/// Below this LU difference the pair is considered already matched.
const ALREADY_MATCHED_LU: f64 = 0.05;

/// The whole pipeline: decode both → measure → derive gains → re-encode both →
/// zip (`a-matched.wav`, `b-matched.wav`, `report.txt`).
pub fn matched_ab_prep(
    a_bytes: Vec<u8>,
    a_name: &str,
    b_bytes: Vec<u8>,
    b_name: &str,
    mode: Mode,
    target_lufs: f64,
    depth: BitDepth,
) -> Result<MatchedPair, String> {
    let audio_a = decode_audio(&format!("file A ({a_name})"), a_bytes)?;
    let audio_b = decode_audio(&format!("file B ({b_name})"), b_bytes)?;
    let stats_a = measure(&format!("file A ({a_name})"), &audio_a)?;
    let stats_b = measure(&format!("file B ({b_name})"), &audio_b)?;

    let (gain_a, gain_b, matched_lufs) = match mode {
        Mode::Quieter => {
            let delta = stats_a.lufs - stats_b.lufs;
            if delta.abs() < ALREADY_MATCHED_LU {
                (0.0, 0.0, stats_a.lufs.min(stats_b.lufs))
            } else if delta > 0.0 {
                // A is louder → turn A down.
                (stats_b.lufs - stats_a.lufs, 0.0, stats_b.lufs)
            } else {
                (0.0, stats_a.lufs - stats_b.lufs, stats_a.lufs)
            }
        }
        Mode::Target => (
            target_lufs - stats_a.lufs,
            target_lufs - stats_b.lufs,
            target_lufs,
        ),
    };

    // Conservative output-size precheck (uncompressed WAV bytes) BEFORE
    // building any big buffer.
    let wav_bytes = |audio: &DecodedAudio| 44 + audio.samples.len() * depth.bytes_per_sample();
    let est_out = wav_bytes(&audio_a) + wav_bytes(&audio_b) + 4096;
    if est_out > MAX_OUTPUT_WAV_BYTES {
        return Err(format!(
            "matched output would be ~{:.1} MiB of WAV (cap {} MiB). Use shorter sections \
             (~30 s per file at 16-bit 44.1 kHz stereo), a lower bit_depth, or trim the files first.",
            est_out as f64 / (1024.0 * 1024.0),
            MAX_OUTPUT_WAV_BYTES / (1024 * 1024)
        ));
    }

    let (wav_a, clipped_a) = encode_wav(&audio_a, gain_a, depth)?;
    drop(audio_a);
    let (wav_b, clipped_b) = encode_wav(&audio_b, gain_b, depth)?;
    drop(audio_b);

    let delta_before = stats_a.lufs - stats_b.lufs;
    let louder_line = if delta_before.abs() < ALREADY_MATCHED_LU {
        "already matched (difference under 0.05 LU)".to_string()
    } else if delta_before > 0.0 {
        format!("{:.2} LU (A was louder)", delta_before.abs())
    } else {
        format!("{:.2} LU (B was louder)", delta_before.abs())
    };

    let mut report = String::new();
    report.push_str("loudness-matched A/B prep\n=========================\n");
    report.push_str(&format!(
        "mode: {}\noutput: {} WAV\n\n",
        match mode {
            Mode::Quieter => "quieter (louder file turned down; no boost)".to_string(),
            Mode::Target => format!("target ({target_lufs:.2} LUFS, both files gained)"),
        },
        depth.label()
    ));
    for (tag, name, out_name, st, gain, clipped) in [
        (
            "file A",
            a_name,
            "a-matched.wav",
            &stats_a,
            gain_a,
            clipped_a,
        ),
        (
            "file B",
            b_name,
            "b-matched.wav",
            &stats_b,
            gain_b,
            clipped_b,
        ),
    ] {
        report.push_str(&format!(
            "{tag} ({name}) -> {out_name}\n  integrated loudness: {} LUFS\n  true peak: {} dBTP\n  loudness range (LRA): {:.1} LU\n  RMS: {} dBFS\n  duration: {:.1} s @ {} Hz, {}\n  gain applied: {}\n  true peak after gain: {} dBTP\n",
            fmt_db(st.lufs),
            fmt_db(st.true_peak_dbtp),
            st.lra,
            fmt_db(st.rms_dbfs),
            st.duration_secs,
            st.rate,
            channel_word(st.channels),
            fmt_gain(gain),
            fmt_db(st.true_peak_dbtp + gain),
        ));
        if clipped > 0 {
            report.push_str(&format!(
                "  WARNING: {clipped} samples exceeded full scale{}\n",
                if depth == BitDepth::Float32 {
                    " (preserved by 32-bit float)"
                } else {
                    " and were hard-clamped — use bit_depth=32f or a lower target"
                }
            ));
        }
        report.push('\n');
    }
    report.push_str(&format!(
        "loudness difference before: {louder_line}\nboth files now at: {:.2} LUFS\n",
        matched_lufs
    ));

    let mut zip_buf: Vec<u8> = Vec::new();
    {
        let mut zw = zip::write::ZipWriter::new(Cursor::new(&mut zip_buf));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in [
            ("report.txt", report.as_bytes()),
            ("a-matched.wav", wav_a.as_slice()),
            ("b-matched.wav", wav_b.as_slice()),
        ] {
            zw.start_file(name, opts)
                .map_err(|e| format!("zip start_file: {e}"))?;
            zw.write_all(bytes).map_err(|e| format!("zip write: {e}"))?;
        }
        zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    }

    Ok(MatchedPair {
        zip: zip_buf,
        a: stats_a,
        b: stats_b,
        gain_a_db: gain_a,
        gain_b_db: gain_b,
        clipped_a,
        clipped_b,
        matched_lufs,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// 16-bit stereo WAV of a 997 Hz sine at `amp` (0..1), `secs` long.
    fn sine_wav(amp: f32, secs: f32, rate: u32) -> Vec<u8> {
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
                let s = (t * 997.0 * std::f32::consts::TAU).sin() * amp;
                let q = (s * 32767.0) as i16;
                w.write_sample(q).unwrap();
                w.write_sample(q).unwrap();
            }
            w.finalize().unwrap();
        }
        out
    }

    /// Quiet 997 Hz sine (amp 0.1 ≈ -23 LUFS) with a ±0.99 single-sample spike
    /// every half second: integrated loudness stays low (spikes are gated
    /// noise) while the true peak sits near full scale — so boosting to any
    /// target above ~-20 LUFS must push the spikes past 1.0.
    fn spiky_wav(secs: f32, rate: u32) -> Vec<u8> {
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
                let s = if i % (rate as usize / 2) == 0 {
                    0.99
                } else {
                    (t * 997.0 * std::f32::consts::TAU).sin() * 0.1
                };
                let q = (s * 32767.0) as i16;
                w.write_sample(q).unwrap();
                w.write_sample(q).unwrap();
            }
            w.finalize().unwrap();
        }
        out
    }

    fn unzip_entry(zip_bytes: &[u8], name: &str) -> Vec<u8> {
        let mut ar = zip::ZipArchive::new(Cursor::new(zip_bytes)).unwrap();
        let mut f = ar.by_name(name).unwrap();
        let mut v = Vec::new();
        f.read_to_end(&mut v).unwrap();
        v
    }

    fn lufs_of(wav: Vec<u8>) -> f64 {
        let d = decode_audio("test", wav).unwrap();
        measure("test", &d).unwrap().lufs
    }

    #[test]
    fn quieter_mode_attenuates_louder_file_to_match() {
        let a = sine_wav(0.5, 4.0, 44100); // louder
        let b = sine_wav(0.1, 4.0, 44100); // quieter
        let m = matched_ab_prep(
            a,
            "a.wav",
            b,
            "b.wav",
            Mode::Quieter,
            -16.0,
            BitDepth::Int24,
        )
        .unwrap();
        assert!(
            m.gain_a_db < -10.0,
            "A (louder) should be turned down: {}",
            m.gain_a_db
        );
        assert_eq!(m.gain_b_db, 0.0, "B (quieter) must be untouched");
        assert_eq!(m.clipped_a, 0);
        assert_eq!(m.clipped_b, 0);
        let la = lufs_of(unzip_entry(&m.zip, "a-matched.wav"));
        let lb = lufs_of(unzip_entry(&m.zip, "b-matched.wav"));
        assert!(
            (la - lb).abs() < 0.1,
            "outputs must be loudness-matched: {la} vs {lb}"
        );
        assert!(
            (lb - m.b.lufs).abs() < 0.05,
            "quieter file loudness unchanged"
        );
        let report = String::from_utf8(unzip_entry(&m.zip, "report.txt")).unwrap();
        assert!(report.contains("A was louder"), "report: {report}");
    }

    #[test]
    fn target_mode_gains_both_files_to_target() {
        let a = sine_wav(0.5, 4.0, 44100);
        let b = sine_wav(0.1, 4.0, 44100);
        let m =
            matched_ab_prep(a, "a.wav", b, "b.wav", Mode::Target, -20.0, BitDepth::Int16).unwrap();
        let la = lufs_of(unzip_entry(&m.zip, "a-matched.wav"));
        let lb = lufs_of(unzip_entry(&m.zip, "b-matched.wav"));
        assert!((la - -20.0).abs() < 0.3, "A at target: {la}");
        assert!((lb - -20.0).abs() < 0.3, "B at target: {lb}");
        assert_eq!(m.matched_lufs, -20.0);
    }

    #[test]
    fn boosting_past_full_scale_clamps_and_counts() {
        let a = sine_wav(0.5, 4.0, 44100);
        let b = spiky_wav(4.0, 44100); // low LUFS, near-full-scale peaks
        let m =
            matched_ab_prep(a, "a.wav", b, "b.wav", Mode::Target, -6.0, BitDepth::Int16).unwrap();
        assert!(m.gain_b_db > 6.0, "B must be boosted hard: {}", m.gain_b_db);
        assert!(
            m.clipped_b > 0,
            "boost past full scale must count clipped samples"
        );
        assert!(
            m.report.contains("hard-clamped"),
            "report warns: {}",
            m.report
        );
    }

    #[test]
    fn float32_output_preserves_overs() {
        let a = sine_wav(0.5, 4.0, 44100);
        let b = spiky_wav(4.0, 44100);
        let m = matched_ab_prep(
            a,
            "a.wav",
            b,
            "b.wav",
            Mode::Target,
            -6.0,
            BitDepth::Float32,
        )
        .unwrap();
        assert!(m.clipped_b > 0, "overs are still counted for 32f");
        let d = decode_audio("test", unzip_entry(&m.zip, "b-matched.wav")).unwrap();
        let peak = d.samples.iter().fold(0f32, |m, &s| m.max(s.abs()));
        assert!(peak > 1.0, "32f keeps samples above full scale: {peak}");
    }

    #[test]
    fn all_bit_depths_write_the_advertised_wav_format() {
        for (depth, bits, fmt) in [
            (BitDepth::Int16, 16, hound::SampleFormat::Int),
            (BitDepth::Int24, 24, hound::SampleFormat::Int),
            (BitDepth::Float32, 32, hound::SampleFormat::Float),
        ] {
            let a = sine_wav(0.4, 3.5, 44100);
            let b = sine_wav(0.2, 3.5, 44100);
            let m = matched_ab_prep(a, "a.wav", b, "b.wav", Mode::Quieter, -16.0, depth).unwrap();
            let wav = unzip_entry(&m.zip, "a-matched.wav");
            let r = hound::WavReader::new(Cursor::new(wav)).unwrap();
            assert_eq!(r.spec().bits_per_sample, bits);
            assert_eq!(r.spec().sample_format, fmt);
            assert_eq!(r.spec().channels, 2);
        }
    }

    #[test]
    fn garbage_bytes_error_names_the_file() {
        let err = decode_audio("file A (x.bin)", vec![0u8; 64]).unwrap_err();
        assert!(err.contains("file A (x.bin)"), "{err}");
        assert!(err.contains("unrecognized or unsupported"), "{err}");
    }

    #[test]
    fn silent_file_is_a_clear_error() {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut silent = Vec::new();
        {
            let mut w = hound::WavWriter::new(Cursor::new(&mut silent), spec).unwrap();
            for _ in 0..(44100 * 2 * 4) {
                w.write_sample(0i16).unwrap();
            }
            w.finalize().unwrap();
        }
        let loud = sine_wav(0.5, 4.0, 44100);
        let err = matched_ab_prep(
            loud,
            "a.wav",
            silent,
            "b.wav",
            Mode::Quieter,
            -16.0,
            BitDepth::Int16,
        )
        .unwrap_err();
        assert!(err.contains("file B (b.wav)"), "{err}");
        assert!(err.contains("silent or too quiet"), "{err}");
    }

    #[test]
    fn too_short_file_is_rejected() {
        let a = sine_wav(0.5, 4.0, 44100);
        let b = sine_wav(0.5, 1.0, 44100);
        let err = matched_ab_prep(
            a,
            "a.wav",
            b,
            "b.wav",
            Mode::Quieter,
            -16.0,
            BitDepth::Int16,
        )
        .unwrap_err();
        assert!(err.contains("too short"), "{err}");
    }

    #[test]
    fn output_cap_is_enforced_with_guidance() {
        // Two ~45 s stereo files at 24-bit blow the 10 MiB output cap while
        // staying inside the per-file PCM cap.
        let a = sine_wav(0.5, 45.0, 44100);
        let b = sine_wav(0.1, 45.0, 44100);
        let err = matched_ab_prep(
            a,
            "a.wav",
            b,
            "b.wav",
            Mode::Quieter,
            -16.0,
            BitDepth::Int24,
        )
        .unwrap_err();
        assert!(err.contains("cap 10 MiB"), "{err}");
        assert!(err.contains("shorter sections"), "{err}");
    }

    #[test]
    fn mode_and_bit_depth_parsers_reject_unknowns() {
        assert!(parse_mode("quieter").is_ok());
        assert!(parse_mode("target").is_ok());
        assert!(parse_mode("both").unwrap_err().contains("unknown mode"));
        assert!(parse_bit_depth("16").is_ok());
        assert!(parse_bit_depth("24").is_ok());
        assert!(parse_bit_depth("32f").is_ok());
        assert!(parse_bit_depth("8")
            .unwrap_err()
            .contains("unknown bit_depth"));
    }

    #[test]
    fn mono_file_matches_against_stereo() {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut mono = Vec::new();
        {
            let mut w = hound::WavWriter::new(Cursor::new(&mut mono), spec).unwrap();
            for i in 0..(44100 * 4) {
                let t = i as f32 / 44100.0;
                let s = (t * 997.0 * std::f32::consts::TAU).sin() * 0.4;
                w.write_sample((s * 32767.0) as i16).unwrap();
            }
            w.finalize().unwrap();
        }
        let stereo = sine_wav(0.1, 4.0, 44100);
        let m = matched_ab_prep(
            mono,
            "mono.wav",
            stereo,
            "stereo.wav",
            Mode::Quieter,
            -16.0,
            BitDepth::Int16,
        )
        .unwrap();
        let out = decode_audio("test", unzip_entry(&m.zip, "a-matched.wav")).unwrap();
        assert_eq!(out.channels, 1, "mono stays mono");
        let la = lufs_of(unzip_entry(&m.zip, "a-matched.wav"));
        let lb = lufs_of(unzip_entry(&m.zip, "b-matched.wav"));
        assert!(
            (la - lb).abs() < 0.1,
            "mono/stereo pair matched: {la} vs {lb}"
        );
    }

    #[test]
    fn already_matched_pair_applies_no_gain() {
        let a = sine_wav(0.3, 4.0, 44100);
        let b = sine_wav(0.3, 4.0, 44100);
        let m = matched_ab_prep(
            a,
            "a.wav",
            b,
            "b.wav",
            Mode::Quieter,
            -16.0,
            BitDepth::Int16,
        )
        .unwrap();
        assert_eq!(m.gain_a_db, 0.0);
        assert_eq!(m.gain_b_db, 0.0);
        assert!(m.report.contains("already matched"), "{}", m.report);
    }
}
