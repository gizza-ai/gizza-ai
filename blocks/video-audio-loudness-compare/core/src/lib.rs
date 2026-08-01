//! gizza-ai/video-audio-loudness-compare core — pure DSP shared by the chat
//! skill block and the CLI. No wafer/wasm-bindgen deps.
//!
//! Measures the ITU-R BS.1770 / EBU R128 loudness of TWO recordings (video or
//! audio, any mix) and reports the loudness GAP between them:
//!
//! 1. **Decode** — symphonia demuxes the container (MP4/MOV/M4A, MKV/WebM, OGG,
//!    WAV, AIFF, CAF, FLAC, MP3, AAC-ADTS), picks the first decodable AUDIO track
//!    (video tracks appear as `CODEC_TYPE_NULL` and are skipped) and streams the
//!    decoded PCM frame-by-frame straight into the `ebur128` analyzer, so the
//!    whole signal is never buffered (only the compressed input bytes, capped by
//!    the caller, and ebur128's small bounded history live in the wasm sandbox).
//! 2. **Measure** — per file: integrated loudness (LUFS), 4×-oversampled true
//!    peak (dBTP), sample peak (dBFS), loudness range (LRA), and the max
//!    momentary / short-term windows. All BEFORE any gain — this tool is
//!    measurement-only and never rewrites audio (use audio-normalize,
//!    normalize-peak or loudness-matched-ab-prep for that).
//! 3. **Compare** — which file is louder and by how many LU, the true-peak and
//!    LRA gaps, and the exact linear/dB gain that would level one file to the
//!    other (or to a target), with a headroom warning when that gain would push a
//!    file's true peak past the chosen ceiling.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker.

use std::io::Cursor;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// EBU R128 gating operates on 400 ms blocks; under ~1 s the integrated figure
/// is dominated by the gate, so refuse anything shorter outright.
pub const MIN_SECONDS: f64 = 1.0;

/// Below this LU difference the two files are treated as already equally loud.
pub const ALREADY_MATCHED_LU: f64 = 0.05;

/// Published delivery loudness targets used for the per-file vs-target offset
/// table (see the competitor-analysis doc). LKFS ≡ LUFS.
pub const PRESET_TARGETS: &[(&str, f64)] = &[
    ("streaming", -14.0),   // Spotify / YouTube / Tidal integrated target
    ("apple_music", -16.0), // Apple Music / AES streaming
    ("ebu_r128", -23.0),    // EU broadcast
    ("atsc_a85", -24.0),    // US CALM Act
];

/// Raw ITU-R BS.1770 / EBU R128 measurements of one file (no gain applied).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measured {
    /// Integrated loudness, LUFS (K-weighted, gated).
    pub integrated_lufs: f64,
    /// True peak, dBTP (4× oversampled), max across channels.
    pub true_peak_dbtp: f64,
    /// Sample peak, dBFS (raw, no oversampling), max across channels.
    pub sample_peak_dbfs: f64,
    /// Loudness range, LU.
    pub lra: f64,
    /// Maximum momentary loudness (400 ms window), LUFS.
    pub momentary_max_lufs: f64,
    /// Maximum short-term loudness (3 s window), LUFS.
    pub short_term_max_lufs: f64,
    pub duration_seconds: f64,
    pub sample_rate: u32,
    pub channels: u16,
}

impl Measured {
    /// Peak-to-loudness ratio (true peak − integrated), LU — a dynamics
    /// indicator: higher = more dynamic, lower = more compressed/limited.
    pub fn plr_lu(&self) -> f64 {
        if self.true_peak_dbtp.is_finite() {
            self.true_peak_dbtp - self.integrated_lufs
        } else {
            f64::NEG_INFINITY
        }
    }
}

/// Decode `bytes` and measure loudness, streaming PCM straight into the analyzer
/// so the whole decoded signal is never held at once.
pub fn measure(bytes: Vec<u8>) -> Result<Measured, String> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| {
            format!("unrecognized or unsupported media format (MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3 and AAC-ADTS are supported): {e}")
        })?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("no decodable audio track found (a video with no audio track cannot be loudness-measured)")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("unsupported audio codec: {e}"))?;

    let mut ebu: Option<ebur128::EbuR128> = None;
    let mut rate: u32 = 0;
    let mut channels: usize = 0;
    let mut frames: u64 = 0;
    let mut sbuf: Option<SampleBuffer<f32>> = None;
    let mut momentary_max = f64::NEG_INFINITY;
    let mut short_term_max = f64::NEG_INFINITY;

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
                if ebu.is_none() {
                    rate = spec.rate;
                    channels = ch;
                    if channels == 0 || channels > 2 {
                        return Err(format!(
                            "only mono or stereo audio is supported (found {channels} channels)"
                        ));
                    }
                    ebu = Some(
                        ebur128::EbuR128::new(
                            channels as u32,
                            rate,
                            ebur128::Mode::I
                                | ebur128::Mode::M
                                | ebur128::Mode::S
                                | ebur128::Mode::LRA
                                | ebur128::Mode::SAMPLE_PEAK
                                | ebur128::Mode::TRUE_PEAK,
                        )
                        .map_err(|e| format!("loudness analyzer init failed: {e:?}"))?,
                    );
                } else if spec.rate != rate || ch != channels {
                    return Err("variable sample rate / channel layout is not supported".into());
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
                let samples = buf.samples();
                frames += (samples.len() / channels.max(1)) as u64;
                let analyzer = ebu.as_mut().expect("analyzer initialized above");
                analyzer
                    .add_frames_f32(samples)
                    .map_err(|e| format!("loudness analysis failed: {e:?}"))?;
                // Sample the sliding windows as we stream so we capture their
                // MAX over the whole file (a single query at the end only sees
                // the trailing window).
                if let Ok(m) = analyzer.loudness_momentary() {
                    if m.is_finite() {
                        momentary_max = momentary_max.max(m);
                    }
                }
                if let Ok(s) = analyzer.loudness_shortterm() {
                    if s.is_finite() {
                        short_term_max = short_term_max.max(s);
                    }
                }
            }
            // A corrupt packet is skippable; keep going with the rest.
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode failed: {e}")),
        }
    }

    let ebu = ebu.ok_or("no audio could be decoded")?;
    if rate == 0 {
        return Err("no audio could be decoded".into());
    }
    let duration_seconds = frames as f64 / rate as f64;
    if duration_seconds < MIN_SECONDS {
        return Err(format!(
            "too short to measure ({duration_seconds:.2} s — EBU R128 gating needs at least {MIN_SECONDS:.0} s of audio)"
        ));
    }

    let integrated_lufs = ebu
        .loudness_global()
        .map_err(|e| format!("loudness analysis failed: {e:?}"))?;
    if !integrated_lufs.is_finite() {
        return Err(
            "is silent or too quiet to measure (integrated loudness fell below the -70 LUFS gate)"
                .into(),
        );
    }
    let lra = ebu
        .loudness_range()
        .map_err(|e| format!("loudness-range analysis failed: {e:?}"))?;

    let dbfs = |linear: f64| {
        if linear > 0.0 {
            20.0 * linear.log10()
        } else {
            f64::NEG_INFINITY
        }
    };
    let mut tp_linear: f64 = 0.0;
    let mut sp_linear: f64 = 0.0;
    for ch in 0..channels as u32 {
        tp_linear = tp_linear.max(
            ebu.true_peak(ch)
                .map_err(|e| format!("true-peak analysis failed: {e:?}"))?,
        );
        sp_linear = sp_linear.max(
            ebu.sample_peak(ch)
                .map_err(|e| format!("sample-peak analysis failed: {e:?}"))?,
        );
    }

    Ok(Measured {
        integrated_lufs,
        true_peak_dbtp: dbfs(tp_linear),
        sample_peak_dbfs: dbfs(sp_linear),
        lra,
        momentary_max_lufs: momentary_max,
        short_term_max_lufs: short_term_max,
        duration_seconds,
        sample_rate: rate,
        channels: channels as u16,
    })
}

/// Which loudness the gain-to-match suggestion levels both files to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchTarget {
    /// Level file 2 to file 1's loudness (file 1 unchanged). Default.
    First,
    /// Level file 1 to file 2's loudness (file 2 unchanged).
    Second,
    /// Bring the quieter file UP to the louder one (may boost / clip).
    Louder,
    /// Bring the louder file DOWN to the quieter one (never boosts — safe).
    Quieter,
    /// Level BOTH files to an explicit LUFS target.
    Target,
}

pub fn parse_match_target(s: &str) -> Result<MatchTarget, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "first" => Ok(MatchTarget::First),
        "second" => Ok(MatchTarget::Second),
        "louder" => Ok(MatchTarget::Louder),
        "quieter" => Ok(MatchTarget::Quieter),
        "target" => Ok(MatchTarget::Target),
        other => Err(format!(
            "unknown match_target '{other}' (expected first, second, louder, quieter or target)"
        )),
    }
}

/// The gain one file needs to reach the chosen reference loudness, plus whether
/// that gain would push its true peak past the ceiling.
#[derive(Debug, Clone, PartialEq)]
pub struct Adjustment {
    pub label: &'static str,
    pub gain_db: f64,
    pub gain_linear: f64,
    /// True peak after applying `gain_db` (linear gain shifts the peak by the
    /// same number of dB), dBTP.
    pub true_peak_after_dbtp: f64,
    /// The leveled true peak exceeds `true_peak_ceiling`.
    pub clips: bool,
}

/// The full two-file loudness comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// "file 1", "file 2", or "equal".
    pub louder: &'static str,
    /// Absolute integrated-loudness difference, LU.
    pub loudness_gap_lu: f64,
    /// Absolute true-peak difference, dB.
    pub true_peak_gap_db: f64,
    /// Absolute loudness-range difference, LU.
    pub lra_gap_lu: f64,
    /// The loudness both files are leveled to for the gain-to-match suggestion.
    pub reference_lufs: f64,
    /// Per-file adjustments (file 1 then file 2).
    pub adjustments: Vec<Adjustment>,
    pub warning: Option<String>,
    pub summary: String,
}

fn fmt_num(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.1}")
    } else if v.is_sign_negative() {
        "-inf".to_string()
    } else {
        "inf".to_string()
    }
}

/// Round to one decimal for the JSON payload (measurements are ±0.1 precise).
pub fn round1(v: f64) -> f64 {
    if v.is_finite() {
        (v * 10.0).round() / 10.0
    } else {
        v
    }
}

/// Round to two decimals (gains / linear multipliers).
pub fn round2(v: f64) -> f64 {
    if v.is_finite() {
        (v * 100.0).round() / 100.0
    } else {
        v
    }
}

/// Compare two measured files: the gap, and the gain each file needs to reach
/// the reference loudness selected by `target`.
pub fn compare(
    a: &Measured,
    b: &Measured,
    target: MatchTarget,
    target_lufs: f64,
    true_peak_ceiling: f64,
) -> Comparison {
    let delta = a.integrated_lufs - b.integrated_lufs;
    let louder = if delta.abs() < ALREADY_MATCHED_LU {
        "equal"
    } else if delta > 0.0 {
        "file 1"
    } else {
        "file 2"
    };

    let reference_lufs = match target {
        MatchTarget::First => a.integrated_lufs,
        MatchTarget::Second => b.integrated_lufs,
        MatchTarget::Louder => a.integrated_lufs.max(b.integrated_lufs),
        MatchTarget::Quieter => a.integrated_lufs.min(b.integrated_lufs),
        MatchTarget::Target => target_lufs,
    };

    let mk = |label: &'static str, m: &Measured| {
        let gain_db = reference_lufs - m.integrated_lufs;
        let true_peak_after = if m.true_peak_dbtp.is_finite() {
            m.true_peak_dbtp + gain_db
        } else {
            f64::NEG_INFINITY
        };
        Adjustment {
            label,
            gain_db,
            gain_linear: 10f64.powf(gain_db / 20.0),
            true_peak_after_dbtp: true_peak_after,
            clips: true_peak_after.is_finite() && true_peak_after > true_peak_ceiling + 1e-9,
        }
    };
    let adjustments = vec![mk("file 1", a), mk("file 2", b)];

    let mut warnings: Vec<String> = Vec::new();
    for adj in &adjustments {
        if adj.clips {
            warnings.push(format!(
                "Leveling {} to {:.1} LUFS applies {:+.2} dB and pushes its true peak to {:.1} dBTP, above the {:.1} dBTP ceiling — apply a limiter or pick a lower reference to avoid inter-sample clipping.",
                adj.label, reference_lufs, adj.gain_db, adj.true_peak_after_dbtp, true_peak_ceiling
            ));
        }
    }
    let warning = (!warnings.is_empty()).then(|| warnings.join(" "));

    let gap = delta.abs();
    let summary = if louder == "equal" {
        format!(
            "Both files are equally loud ({} vs {} LUFS integrated, within {:.2} LU).",
            fmt_num(a.integrated_lufs),
            fmt_num(b.integrated_lufs),
            ALREADY_MATCHED_LU
        )
    } else {
        let (loud_name, gain_target) = if louder == "file 1" {
            ("File 1", "file 2")
        } else {
            ("File 2", "file 1")
        };
        format!(
            "{loud_name} is {gap:.1} LU louder than {gain_target} ({} vs {} LUFS integrated; true peaks {} vs {} dBTP). Reference {:.1} LUFS: file 1 {:+.2} dB, file 2 {:+.2} dB.",
            fmt_num(a.integrated_lufs),
            fmt_num(b.integrated_lufs),
            fmt_num(a.true_peak_dbtp),
            fmt_num(b.true_peak_dbtp),
            reference_lufs,
            adjustments[0].gain_db,
            adjustments[1].gain_db,
        )
    };

    Comparison {
        louder,
        loudness_gap_lu: gap,
        true_peak_gap_db: (a.true_peak_dbtp - b.true_peak_dbtp).abs(),
        lra_gap_lu: (a.lra - b.lra).abs(),
        reference_lufs,
        adjustments,
        warning,
        summary,
    }
}

/// Full report: both measurements plus the comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct CompareReport {
    pub a: Measured,
    pub b: Measured,
    pub comparison: Comparison,
}

/// Full pipeline: decode + measure both files, then compare. `a_name` / `b_name`
/// are used only to attribute decode errors to the right file.
pub fn loudness_compare(
    a_bytes: Vec<u8>,
    a_name: &str,
    b_bytes: Vec<u8>,
    b_name: &str,
    target: MatchTarget,
    target_lufs: f64,
    true_peak_ceiling: f64,
) -> Result<CompareReport, String> {
    let a = measure(a_bytes).map_err(|e| format!("file 1 ({a_name}): {e}"))?;
    let b = measure(b_bytes).map_err(|e| format!("file 2 ({b_name}): {e}"))?;
    let comparison = compare(&a, &b, target, target_lufs, true_peak_ceiling);
    Ok(CompareReport { a, b, comparison })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{SampleFormat, WavSpec, WavWriter};

    /// 16-bit stereo WAV: `secs` of a 997 Hz sine at linear amplitude `amp`.
    fn sine_wav(amp: f32, secs: f32, rate: u32) -> Vec<u8> {
        let spec = WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut out = Vec::new();
        {
            let mut w = WavWriter::new(Cursor::new(&mut out), spec).unwrap();
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

    /// Quiet 997 Hz sine (amp 0.08 ≈ -25 LUFS) with a ±0.99 single-sample spike
    /// every half second: integrated loudness stays low (spikes are gated) while
    /// the true peak sits near full scale — so any boost pushes it past 0 dBTP.
    fn spiky_wav(secs: f32, rate: u32) -> Vec<u8> {
        let spec = WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut out = Vec::new();
        {
            let mut w = WavWriter::new(Cursor::new(&mut out), spec).unwrap();
            let n = (secs * rate as f32) as usize;
            for i in 0..n {
                let t = i as f32 / rate as f32;
                let s = if i % (rate as usize / 2) == 0 {
                    0.99
                } else {
                    (t * 997.0 * std::f32::consts::TAU).sin() * 0.08
                };
                let q = (s * 32767.0) as i16;
                w.write_sample(q).unwrap();
                w.write_sample(q).unwrap();
            }
            w.finalize().unwrap();
        }
        out
    }

    #[test]
    fn measures_the_core_metrics() {
        let m = measure(sine_wav(0.5, 5.0, 44100)).unwrap();
        assert_eq!(m.channels, 2);
        assert_eq!(m.sample_rate, 44100);
        assert!(m.integrated_lufs.is_finite());
        // A 0.5-amplitude sine peaks around -6 dBFS / dBTP.
        assert!(m.true_peak_dbtp > -7.0, "true peak: {}", m.true_peak_dbtp);
        assert!(
            m.sample_peak_dbfs > -7.0,
            "sample peak: {}",
            m.sample_peak_dbfs
        );
        // A steady tone has near-zero loudness range.
        assert!(m.lra < 1.0, "steady tone LRA ~0: {}", m.lra);
        // The sliding windows filled and were captured.
        assert!(m.momentary_max_lufs.is_finite(), "momentary max captured");
        assert!(m.short_term_max_lufs.is_finite(), "short-term max captured");
        assert!(m.plr_lu().is_finite());
    }

    #[test]
    fn identifies_the_louder_file_and_the_gap() {
        let a = measure(sine_wav(0.5, 5.0, 44100)).unwrap(); // louder
        let b = measure(sine_wav(0.1, 5.0, 44100)).unwrap(); // ~14 LU quieter
        let c = compare(&a, &b, MatchTarget::First, -14.0, -1.0);
        assert_eq!(c.louder, "file 1");
        assert!(
            (c.loudness_gap_lu - 14.0).abs() < 1.5,
            "amp 0.5 vs 0.1 ≈ 14 LU apart, got {}",
            c.loudness_gap_lu
        );
        assert!(c.summary.starts_with("File 1 is"), "{}", c.summary);
    }

    #[test]
    fn match_first_levels_file2_and_leaves_file1() {
        let a = measure(sine_wav(0.5, 5.0, 44100)).unwrap();
        let b = measure(sine_wav(0.1, 5.0, 44100)).unwrap();
        let c = compare(&a, &b, MatchTarget::First, -14.0, -1.0);
        assert_eq!(c.reference_lufs, a.integrated_lufs);
        assert_eq!(c.adjustments[0].gain_db, 0.0, "file 1 is the reference");
        assert!(
            c.adjustments[1].gain_db > 10.0,
            "file 2 boosted to file 1: {}",
            c.adjustments[1].gain_db
        );
    }

    #[test]
    fn match_quieter_only_attenuates_and_never_clips() {
        let a = measure(sine_wav(0.5, 5.0, 44100)).unwrap();
        let b = measure(sine_wav(0.1, 5.0, 44100)).unwrap();
        let c = compare(&a, &b, MatchTarget::Quieter, -14.0, -1.0);
        assert_eq!(c.reference_lufs, b.integrated_lufs.min(a.integrated_lufs));
        assert!(c.adjustments[0].gain_db < 0.0, "louder file turned down");
        assert_eq!(c.adjustments[1].gain_db, 0.0, "quieter file untouched");
        assert!(c.warning.is_none(), "attenuation never clips: {c:?}");
    }

    #[test]
    fn match_louder_boost_past_ceiling_warns() {
        // file 1 loud sine; file 2 quiet-but-spiky → boosting it to file 1's
        // loudness pushes its near-full-scale peaks over the ceiling.
        let a = measure(sine_wav(0.5, 5.0, 44100)).unwrap();
        let b = measure(spiky_wav(5.0, 44100)).unwrap();
        let c = compare(&a, &b, MatchTarget::Louder, -14.0, -1.0);
        assert!(c.adjustments[1].gain_db > 6.0, "spiky file boosted hard");
        assert!(c.adjustments[1].clips, "boosted true peak exceeds ceiling");
        assert!(
            c.warning.as_deref().unwrap_or("").contains("ceiling"),
            "warning names the ceiling: {:?}",
            c.warning
        );
    }

    #[test]
    fn match_target_levels_both_to_the_target() {
        let a = measure(sine_wav(0.5, 5.0, 44100)).unwrap();
        let b = measure(sine_wav(0.1, 5.0, 44100)).unwrap();
        let c = compare(&a, &b, MatchTarget::Target, -23.0, -1.0);
        assert_eq!(c.reference_lufs, -23.0);
        // gain = target − integrated for each.
        assert!((c.adjustments[0].gain_db - (-23.0 - a.integrated_lufs)).abs() < 1e-9);
        assert!((c.adjustments[1].gain_db - (-23.0 - b.integrated_lufs)).abs() < 1e-9);
    }

    #[test]
    fn equal_files_report_no_winner() {
        let a = measure(sine_wav(0.3, 5.0, 44100)).unwrap();
        let b = measure(sine_wav(0.3, 5.0, 44100)).unwrap();
        let c = compare(&a, &b, MatchTarget::First, -14.0, -1.0);
        assert_eq!(c.louder, "equal");
        assert!(c.loudness_gap_lu < ALREADY_MATCHED_LU);
        assert!(c.summary.contains("equally loud"), "{}", c.summary);
    }

    #[test]
    fn gain_linear_matches_gain_db() {
        let a = measure(sine_wav(0.5, 5.0, 44100)).unwrap();
        let b = measure(sine_wav(0.1, 5.0, 44100)).unwrap();
        let c = compare(&a, &b, MatchTarget::First, -14.0, -1.0);
        let adj = &c.adjustments[1];
        assert!((adj.gain_linear - 10f64.powf(adj.gain_db / 20.0)).abs() < 1e-9);
    }

    #[test]
    fn parse_match_target_rejects_unknown() {
        assert_eq!(parse_match_target("first").unwrap(), MatchTarget::First);
        assert_eq!(parse_match_target("QUIETER").unwrap(), MatchTarget::Quieter);
        assert_eq!(parse_match_target("target").unwrap(), MatchTarget::Target);
        assert!(parse_match_target("both")
            .unwrap_err()
            .contains("unknown match_target"));
    }

    #[test]
    fn mono_input_is_supported() {
        let spec = WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut mono = Vec::new();
        {
            let mut w = WavWriter::new(Cursor::new(&mut mono), spec).unwrap();
            for i in 0..(44100 * 4) {
                let t = i as f32 / 44100.0;
                let s = (t * 997.0 * std::f32::consts::TAU).sin() * 0.2;
                w.write_sample((s * 32767.0) as i16).unwrap();
            }
            w.finalize().unwrap();
        }
        let m = measure(mono).unwrap();
        assert_eq!(m.channels, 1);
        assert!(m.integrated_lufs.is_finite());
    }

    #[test]
    fn garbage_bytes_are_a_clear_error() {
        let err = measure(vec![0u8; 128]).unwrap_err();
        assert!(err.contains("unrecognized or unsupported"), "{err}");
    }

    #[test]
    fn too_short_is_rejected() {
        let err = measure(sine_wav(0.2, 0.4, 44100)).unwrap_err();
        assert!(err.contains("too short"), "{err}");
    }

    #[test]
    fn silence_is_rejected() {
        let spec = WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut silent = Vec::new();
        {
            let mut w = WavWriter::new(Cursor::new(&mut silent), spec).unwrap();
            for _ in 0..(44100 * 2 * 3) {
                w.write_sample(0i16).unwrap();
            }
            w.finalize().unwrap();
        }
        let err = measure(silent).unwrap_err();
        assert!(err.contains("silent or too quiet"), "{err}");
    }

    #[test]
    fn loudness_compare_attributes_decode_errors_to_the_file() {
        let good = sine_wav(0.3, 4.0, 44100);
        let err = loudness_compare(
            good,
            "a.wav",
            vec![0u8; 64],
            "b.mp4",
            MatchTarget::First,
            -14.0,
            -1.0,
        )
        .unwrap_err();
        assert!(err.contains("file 2 (b.mp4)"), "{err}");
        assert!(err.contains("unrecognized or unsupported"), "{err}");
    }

    #[test]
    fn end_to_end_compare_reports_louder_file() {
        let r = loudness_compare(
            sine_wav(0.5, 5.0, 44100),
            "loud.wav",
            sine_wav(0.1, 5.0, 44100),
            "quiet.wav",
            MatchTarget::First,
            -14.0,
            -1.0,
        )
        .unwrap();
        assert_eq!(r.comparison.louder, "file 1");
        assert!(r.a.integrated_lufs > r.b.integrated_lufs);
    }

    #[test]
    fn preset_targets_are_the_published_values() {
        let map: std::collections::HashMap<_, _> = PRESET_TARGETS.iter().copied().collect();
        assert_eq!(map["streaming"], -14.0);
        assert_eq!(map["apple_music"], -16.0);
        assert_eq!(map["ebu_r128"], -23.0);
        assert_eq!(map["atsc_a85"], -24.0);
    }
}
