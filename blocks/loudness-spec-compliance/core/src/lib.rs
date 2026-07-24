//! loudness-spec-compliance core — pure compute, shared by the chat skill block
//! and the CLI. No wafer/wasm-bindgen deps.
//!
//! Decodes one audio file (symphonia: wav/flac/mp3/ogg-Vorbis/m4a), measures its
//! ITU-R BS.1770 / EBU R128 **integrated loudness (LUFS)**, **true peak (dBTP,
//! 4× oversampled)** and **loudness range (LRA)** via the pure-Rust `ebur128`
//! crate, then compares each measurement against a named delivery spec (EBU R128,
//! ATSC A/85, or a streaming target) and returns a per-criterion PASS/FAIL
//! verdict plus an overall pass. Measure + verdict only — it never rewrites the
//! audio (that is what audio-normalize / normalize-peak / loudness-matched-ab-prep
//! do).
//!
//! Memory: PCM is streamed frame-by-frame straight into the `ebur128` analyzer,
//! so the whole decoded signal is never held at once — only the compressed input
//! bytes (capped by the caller) and ebur128's small bounded gating history live
//! in the 64 MiB wasm sandbox. Files of any length within the input-byte cap can
//! therefore be measured.

use std::io::Cursor;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// EBU R128 gating operates on 400 ms blocks; under ~1 s the integrated figure
/// is dominated by the gate and any LRA is meaningless, so refuse it outright.
pub const MIN_SECONDS: f64 = 1.0;

/// A named delivery specification: the loudness/true-peak targets a file must
/// meet to be "compliant".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spec {
    /// Human-readable name shown in the report.
    pub name: &'static str,
    /// Target integrated loudness (LUFS / LKFS — identical unit).
    pub target_lufs: f64,
    /// Permitted deviation from `target_lufs`, in LU (a ± window).
    pub tolerance_lu: f64,
    /// Maximum true peak, dBTP (measured must be at or below this).
    pub max_true_peak: f64,
    /// Optional loudness-range ceiling, LU. `None` = LRA is reported but not a
    /// pass/fail criterion (most specs don't cap it).
    pub max_lra: Option<f64>,
}

/// Resolve a `standard` name (plus the custom-spec overrides, which are only
/// consulted when `standard == "custom"`) into a concrete [`Spec`].
///
/// Preset values are the published delivery targets (see the competitor-analysis
/// doc). LKFS ≡ LUFS. Streaming presets use a ±1.0 LU window — the loudness a
/// normalizing platform will not re-touch.
pub fn resolve_spec(
    standard: &str,
    target_lufs: f64,
    tolerance_lu: f64,
    max_true_peak: f64,
    max_lra: f64,
) -> Result<Spec, String> {
    let s = standard.trim().to_ascii_lowercase();
    let spec = match s.as_str() {
        "ebu-r128" => Spec {
            name: "EBU R128",
            target_lufs: -23.0,
            tolerance_lu: 1.0,
            max_true_peak: -1.0,
            max_lra: None,
        },
        "atsc-a85" => Spec {
            name: "ATSC A/85",
            target_lufs: -24.0,
            tolerance_lu: 2.0,
            max_true_peak: -2.0,
            max_lra: None,
        },
        "spotify" => Spec {
            name: "Spotify",
            target_lufs: -14.0,
            tolerance_lu: 1.0,
            max_true_peak: -1.0,
            max_lra: None,
        },
        "youtube" => Spec {
            name: "YouTube",
            target_lufs: -14.0,
            tolerance_lu: 1.0,
            max_true_peak: -1.0,
            max_lra: None,
        },
        "apple-music" => Spec {
            name: "Apple Music",
            target_lufs: -16.0,
            tolerance_lu: 1.0,
            max_true_peak: -1.0,
            max_lra: None,
        },
        "amazon-music" => Spec {
            name: "Amazon Music",
            target_lufs: -14.0,
            tolerance_lu: 1.0,
            // Amazon's inter-sample-clip headroom for Echo/Alexa playback.
            max_true_peak: -2.0,
            max_lra: None,
        },
        "tidal" => Spec {
            name: "Tidal",
            target_lufs: -14.0,
            tolerance_lu: 1.0,
            max_true_peak: -1.0,
            max_lra: None,
        },
        "custom" => {
            if !target_lufs.is_finite() || !(-40.0..=0.0).contains(&target_lufs) {
                return Err(format!(
                    "custom target_lufs must be between -40 and 0 LUFS, got {target_lufs}"
                ));
            }
            if !tolerance_lu.is_finite() || !(0.0..=12.0).contains(&tolerance_lu) {
                return Err(format!(
                    "custom tolerance_lu must be between 0 and 12 LU, got {tolerance_lu}"
                ));
            }
            if !max_true_peak.is_finite() || !(-20.0..=0.0).contains(&max_true_peak) {
                return Err(format!(
                    "custom max_true_peak must be between -20 and 0 dBTP, got {max_true_peak}"
                ));
            }
            if !max_lra.is_finite() || !(0.0..=60.0).contains(&max_lra) {
                return Err(format!(
                    "custom max_lra must be between 0 and 60 LU (0 = do not check LRA), got {max_lra}"
                ));
            }
            Spec {
                name: "custom",
                target_lufs,
                tolerance_lu,
                max_true_peak,
                // 0 (or negative) means "don't apply an LRA ceiling".
                max_lra: (max_lra > 0.0).then_some(max_lra),
            }
        }
        other => {
            return Err(format!(
                "unknown standard '{other}' (expected ebu-r128, atsc-a85, spotify, youtube, apple-music, amazon-music, tidal or custom)"
            ))
        }
    };
    Ok(spec)
}

/// Raw ITU-R BS.1770 / EBU R128 measurements of a file (no verdict).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measured {
    /// Integrated loudness, LUFS (K-weighted, gated).
    pub integrated_lufs: f64,
    /// True peak, dBTP (4× oversampled), max across channels.
    pub true_peak_dbtp: f64,
    /// Loudness range, LU.
    pub lra: f64,
    pub duration_seconds: f64,
    pub sample_rate: u32,
    pub channels: u16,
}

/// One pass/fail criterion of the compliance verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    /// Machine key: `integrated_loudness`, `true_peak`, or `loudness_range`.
    pub criterion: &'static str,
    /// The measured value (LUFS / dBTP / LU).
    pub measured: f64,
    /// Human-readable limit, e.g. `-23.0 ± 1.0 LUFS` or `≤ -1.0 dBTP`.
    pub limit: String,
    pub pass: bool,
    /// Human-readable margin, e.g. `+0.6 LU from target` / `0.3 dB under ceiling`.
    pub detail: String,
}

/// The full verdict: the resolved spec, the measurements, the per-criterion
/// checks and the overall pass.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub spec: Spec,
    pub measured: Measured,
    pub checks: Vec<Check>,
    pub pass: bool,
    pub summary: String,
}

/// Decode `bytes` and measure integrated loudness / true peak / LRA, streaming
/// PCM straight into the analyzer so the whole signal is never buffered.
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
            format!("unrecognized or unsupported audio format (wav, flac, mp3, ogg-Vorbis and m4a are supported): {e}")
        })?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("no decodable audio track found")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("unsupported audio codec: {e}"))?;

    let mut ebu: Option<ebur128::EbuR128> = None;
    let mut rate: u32 = 0;
    let mut channels: usize = 0;
    let mut frames: u64 = 0;
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
                            ebur128::Mode::I | ebur128::Mode::LRA | ebur128::Mode::TRUE_PEAK,
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
                ebu.as_mut()
                    .expect("analyzer initialized above")
                    .add_frames_f32(samples)
                    .map_err(|e| format!("loudness analysis failed: {e:?}"))?;
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
    let mut tp_linear: f64 = 0.0;
    for ch in 0..channels as u32 {
        let p = ebu
            .true_peak(ch)
            .map_err(|e| format!("true-peak analysis failed: {e:?}"))?;
        tp_linear = tp_linear.max(p);
    }
    let true_peak_dbtp = if tp_linear > 0.0 {
        20.0 * tp_linear.log10()
    } else {
        f64::NEG_INFINITY
    };

    Ok(Measured {
        integrated_lufs,
        true_peak_dbtp,
        lra,
        duration_seconds,
        sample_rate: rate,
        channels: channels as u16,
    })
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

/// Build the per-criterion checks + overall verdict from measurements and a spec.
pub fn verdict(measured: Measured, spec: Spec) -> Report {
    let mut checks = Vec::new();

    // Integrated loudness: within target ± tolerance.
    let delta = measured.integrated_lufs - spec.target_lufs;
    let loud_pass = delta.abs() <= spec.tolerance_lu + 1e-9;
    checks.push(Check {
        criterion: "integrated_loudness",
        measured: round1(measured.integrated_lufs),
        limit: format!("{:.1} ± {:.1} LUFS", spec.target_lufs, spec.tolerance_lu),
        pass: loud_pass,
        detail: format!(
            "{:+.1} LU from target ({})",
            delta,
            if loud_pass {
                "within tolerance"
            } else if delta > 0.0 {
                "too loud"
            } else {
                "too quiet"
            }
        ),
    });

    // True peak: at or below the ceiling.
    let tp_pass = measured.true_peak_dbtp <= spec.max_true_peak + 1e-9;
    let tp_margin = spec.max_true_peak - measured.true_peak_dbtp;
    checks.push(Check {
        criterion: "true_peak",
        measured: round1(measured.true_peak_dbtp),
        limit: format!("\u{2264} {:.1} dBTP", spec.max_true_peak),
        pass: tp_pass,
        detail: if !measured.true_peak_dbtp.is_finite() {
            "digital silence (no true peak)".to_string()
        } else if tp_pass {
            format!("{:.1} dB under ceiling", tp_margin)
        } else {
            format!("{:.1} dB over ceiling", -tp_margin)
        },
    });

    // Loudness range: only a pass/fail criterion when the spec caps it.
    if let Some(max_lra) = spec.max_lra {
        let lra_pass = measured.lra <= max_lra + 1e-9;
        checks.push(Check {
            criterion: "loudness_range",
            measured: round1(measured.lra),
            limit: format!("\u{2264} {:.1} LU", max_lra),
            pass: lra_pass,
            detail: if lra_pass {
                format!("{:.1} LU under ceiling", max_lra - measured.lra)
            } else {
                format!("{:.1} LU over ceiling", measured.lra - max_lra)
            },
        });
    }

    let pass = checks.iter().all(|c| c.pass);
    let failed: Vec<&str> = checks
        .iter()
        .filter(|c| !c.pass)
        .map(|c| c.criterion)
        .collect();
    let summary = if pass {
        format!(
            "PASS — meets {} ({} LUFS integrated, {} dBTP true peak, {} LU range).",
            spec.name,
            fmt_num(measured.integrated_lufs),
            fmt_num(measured.true_peak_dbtp),
            fmt_num(measured.lra),
        )
    } else {
        format!(
            "FAIL — does not meet {} ({} out of spec). Measured {} LUFS integrated, {} dBTP true peak, {} LU range.",
            spec.name,
            failed.join(" + "),
            fmt_num(measured.integrated_lufs),
            fmt_num(measured.true_peak_dbtp),
            fmt_num(measured.lra),
        )
    };

    Report {
        spec,
        measured,
        checks,
        pass,
        summary,
    }
}

/// Full pipeline: decode + measure `bytes`, then check against `spec`.
pub fn check_compliance(bytes: Vec<u8>, spec: Spec) -> Result<Report, String> {
    let measured = measure(bytes)?;
    Ok(verdict(measured, spec))
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

    #[test]
    fn presets_have_the_published_targets() {
        let ebu = resolve_spec("ebu-r128", 0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(ebu.target_lufs, -23.0);
        assert_eq!(ebu.tolerance_lu, 1.0);
        assert_eq!(ebu.max_true_peak, -1.0);
        let atsc = resolve_spec("atsc-a85", 0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(atsc.target_lufs, -24.0);
        assert_eq!(atsc.max_true_peak, -2.0);
        let sp = resolve_spec("spotify", 0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(sp.target_lufs, -14.0);
        assert_eq!(sp.max_true_peak, -1.0);
        let am = resolve_spec("amazon-music", 0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(am.max_true_peak, -2.0);
        let apple = resolve_spec("apple-music", 0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(apple.target_lufs, -16.0);
    }

    #[test]
    fn unknown_standard_is_rejected() {
        let err = resolve_spec("netflix", 0.0, 0.0, 0.0, 0.0).unwrap_err();
        assert!(err.contains("unknown standard"), "{err}");
    }

    #[test]
    fn custom_spec_uses_overrides_and_lra_zero_means_unchecked() {
        let s = resolve_spec("custom", -18.0, 0.5, -3.0, 0.0).unwrap();
        assert_eq!(s.name, "custom");
        assert_eq!(s.target_lufs, -18.0);
        assert_eq!(s.tolerance_lu, 0.5);
        assert_eq!(s.max_true_peak, -3.0);
        assert_eq!(s.max_lra, None);
        let capped = resolve_spec("custom", -18.0, 0.5, -3.0, 9.0).unwrap();
        assert_eq!(capped.max_lra, Some(9.0));
    }

    #[test]
    fn custom_spec_rejects_out_of_range_overrides() {
        assert!(resolve_spec("custom", -60.0, 1.0, -1.0, 0.0)
            .unwrap_err()
            .contains("target_lufs"));
        assert!(resolve_spec("custom", -18.0, 20.0, -1.0, 0.0)
            .unwrap_err()
            .contains("tolerance_lu"));
        assert!(resolve_spec("custom", -18.0, 1.0, 3.0, 0.0)
            .unwrap_err()
            .contains("max_true_peak"));
        assert!(resolve_spec("custom", -18.0, 1.0, -1.0, 99.0)
            .unwrap_err()
            .contains("max_lra"));
    }

    #[test]
    fn a_neg23_lufs_tone_passes_ebu_r128() {
        // A steady 997 Hz sine at amp ~0.0708 measures very close to -23 LUFS.
        let wav = sine_wav(0.0708, 6.0, 44100);
        let spec = resolve_spec("ebu-r128", 0.0, 0.0, 0.0, 0.0).unwrap();
        let r = check_compliance(wav, spec).unwrap();
        assert!(
            (r.measured.integrated_lufs - -23.0).abs() < 1.0,
            "expected ~-23 LUFS, got {}",
            r.measured.integrated_lufs
        );
        let loud = r
            .checks
            .iter()
            .find(|c| c.criterion == "integrated_loudness")
            .unwrap();
        assert!(loud.pass, "integrated should pass: {}", loud.detail);
        assert!(r.pass, "overall should PASS: {}", r.summary);
        assert!(r.summary.starts_with("PASS"), "{}", r.summary);
    }

    #[test]
    fn a_loud_tone_fails_the_integrated_check() {
        // amp 0.5 is far louder than -23 LUFS → integrated FAIL against EBU R128.
        let wav = sine_wav(0.5, 6.0, 44100);
        let spec = resolve_spec("ebu-r128", 0.0, 0.0, 0.0, 0.0).unwrap();
        let r = check_compliance(wav, spec).unwrap();
        assert!(!r.pass, "loud tone must FAIL: {}", r.summary);
        let loud = r
            .checks
            .iter()
            .find(|c| c.criterion == "integrated_loudness")
            .unwrap();
        assert!(!loud.pass);
        assert!(loud.detail.contains("too loud"), "{}", loud.detail);
        assert!(r.summary.starts_with("FAIL"), "{}", r.summary);
        assert!(r.summary.contains("integrated_loudness"), "{}", r.summary);
    }

    #[test]
    fn true_peak_ceiling_is_enforced() {
        // Full-scale sine: true peak ~0 dBTP, well over EBU's -1 dBTP ceiling.
        let wav = sine_wav(0.999, 6.0, 44100);
        let spec = resolve_spec("ebu-r128", 0.0, 0.0, 0.0, 0.0).unwrap();
        let r = check_compliance(wav, spec).unwrap();
        let tp = r.checks.iter().find(|c| c.criterion == "true_peak").unwrap();
        assert!(!tp.pass, "true peak should exceed ceiling: {}", tp.detail);
        assert!(tp.detail.contains("over ceiling"), "{}", tp.detail);
    }

    #[test]
    fn custom_lra_cap_adds_a_third_check() {
        let wav = sine_wav(0.0708, 6.0, 44100);
        // A steady tone has ~0 LRA, so any positive cap passes.
        let spec = resolve_spec("custom", -23.0, 1.0, -1.0, 5.0).unwrap();
        let r = check_compliance(wav, spec).unwrap();
        assert_eq!(r.checks.len(), 3, "LRA cap adds a loudness_range check");
        let lra = r
            .checks
            .iter()
            .find(|c| c.criterion == "loudness_range")
            .unwrap();
        assert!(lra.pass, "steady tone LRA under cap: {}", lra.detail);
        // Without a cap there are only two checks.
        let no_cap = resolve_spec("ebu-r128", 0.0, 0.0, 0.0, 0.0).unwrap();
        let wav2 = sine_wav(0.0708, 6.0, 44100);
        assert_eq!(check_compliance(wav2, no_cap).unwrap().checks.len(), 2);
    }

    #[test]
    fn garbage_bytes_are_a_clear_error() {
        let err = measure(vec![0u8; 128]).unwrap_err();
        assert!(err.contains("unrecognized or unsupported"), "{err}");
    }

    #[test]
    fn too_short_is_rejected() {
        let wav = sine_wav(0.1, 0.4, 44100);
        let err = measure(wav).unwrap_err();
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
            // Mono sums no second channel, so it reads ~3 LU quieter than the
            // same amplitude in stereo — amp 0.1 lands a mono tone near -23 LUFS.
            for i in 0..(44100 * 5) {
                let t = i as f32 / 44100.0;
                let s = (t * 997.0 * std::f32::consts::TAU).sin() * 0.1;
                w.write_sample((s * 32767.0) as i16).unwrap();
            }
            w.finalize().unwrap();
        }
        let m = measure(mono).unwrap();
        assert_eq!(m.channels, 1);
        assert!(
            (m.integrated_lufs - -23.0).abs() < 1.5,
            "{}",
            m.integrated_lufs
        );
    }
}
