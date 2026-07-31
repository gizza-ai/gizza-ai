//! gizza-ai/audio-noise-gate core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! A noise gate attenuates audio that falls BELOW a threshold, so background
//! hiss, room tone, hum, and breaths in the quiet gaps between words or notes
//! are pushed down (or silenced) while the wanted signal above the threshold
//! passes untouched. It uses ffmpeg's dedicated `agate` filter in its default
//! downward mode. This is a DYNAMICS gate (level-based, keyed off amplitude),
//! NOT spectral denoising (that's the separate audio-noise-reduce tool, which
//! subtracts a hiss profile from the whole signal), and it does NOT shorten the
//! file the way silence-removal tools do — the gaps stay in place, just quieter.
//!
//! The four classic gate controls are exposed directly — `threshold` (dB, the
//! level a signal must exceed to open the gate), `reduction` (dB the signal is
//! attenuated by while the gate is closed — the floor), `attack` (ms to open)
//! and `release` (ms to close) — plus a `ratio` (how steeply the gate clamps
//! down) and a `detection` mode (peak vs RMS). `-vn` drops attached-picture
//! streams (album art); the output is re-encoded because `agate` rewrites
//! samples.

/// Output audio formats audio-noise-gate can write (family-standard set).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Mp3,
    Wav,
    Ogg,
    Flac,
    M4a,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Mp3 => "mp3",
            Format::Wav => "wav",
            Format::Ogg => "ogg",
            Format::Flac => "flac",
            Format::M4a => "m4a",
        }
    }

    /// IANA media type for the produced file.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Mp3 => "audio/mpeg",
            Format::Wav => "audio/wav",
            Format::Ogg => "audio/ogg",
            Format::Flac => "audio/flac",
            Format::M4a => "audio/mp4",
        }
    }

    /// Encoder argv fragment (`-c:a …`); lossy formats are fixed at 192 kbps.
    fn codec_args(self) -> Vec<String> {
        match self {
            Format::Mp3 => vec![
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                "192k".into(),
            ],
            Format::Wav => vec!["-c:a".into(), "pcm_s16le".into()],
            Format::Ogg => vec![
                "-c:a".into(),
                "libvorbis".into(),
                "-b:a".into(),
                "192k".into(),
            ],
            Format::Flac => vec!["-c:a".into(), "flac".into()],
            Format::M4a => vec!["-c:a".into(), "aac".into()],
        }
    }
}

/// Parse the user-facing format string. Empty defaults to mp3.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "mp3" => Ok(Format::Mp3),
        "wav" => Ok(Format::Wav),
        "ogg" => Ok(Format::Ogg),
        "flac" => Ok(Format::Flac),
        "m4a" => Ok(Format::M4a),
        other => Err(format!(
            "format {other:?} not supported (mp3|wav|ogg|flac|m4a)"
        )),
    }
}

/// How the gate measures level to decide open/closed.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Detection {
    /// Root-mean-square — tracks average loudness, smoother, ignores brief peaks.
    Rms,
    /// Peak — reacts to instantaneous sample peaks, catches sharp transients.
    Peak,
}

impl Detection {
    /// The `detection=` value ffmpeg's `agate` expects.
    pub fn as_str(self) -> &'static str {
        match self {
            Detection::Rms => "rms",
            Detection::Peak => "peak",
        }
    }
}

/// Parse the user-facing detection string. Empty defaults to rms.
pub fn parse_detection(s: &str) -> Result<Detection, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "rms" => Ok(Detection::Rms),
        "peak" => Ok(Detection::Peak),
        other => Err(format!("detection {other:?} not supported (rms|peak)")),
    }
}

// Accepted control ranges. The gate UI speaks dB and ms; ffmpeg's `agate`
// options are linear scalars (threshold/range are 0..1 amplitude), so the
// filter builder converts them.
/// Threshold, in dB below full scale, a signal must EXCEED to open the gate.
pub const THRESHOLD_MIN_DB: f64 = -80.0;
pub const THRESHOLD_MAX_DB: f64 = 0.0;
/// Reduction in dB applied while the gate is closed (how much quieter the
/// below-threshold signal gets). 0 = no attenuation (a no-op); larger =
/// closer to silent. Mapped to `agate`'s linear `range` floor.
pub const REDUCTION_MIN_DB: f64 = 0.0;
pub const REDUCTION_MAX_DB: f64 = 80.0;
/// Attack time in milliseconds — how fast the gate opens once level rises.
pub const ATTACK_MIN_MS: f64 = 0.01;
pub const ATTACK_MAX_MS: f64 = 2000.0;
/// Release time in milliseconds — how fast the gate closes once level drops.
pub const RELEASE_MIN_MS: f64 = 0.01;
pub const RELEASE_MAX_MS: f64 = 9000.0;
/// Gate ratio — how steeply gain is pulled down below the threshold.
pub const RATIO_MIN: f64 = 1.0;
pub const RATIO_MAX: f64 = 20.0;

// Family defaults — the single source for the chat descriptor defaults and the
// page's "empty field means use the default" fallbacks (see `web/src/lib.rs`).
/// Default threshold in dB (a typical spoken-word noise floor sits below this).
pub const DEFAULT_THRESHOLD_DB: f64 = -35.0;
/// Default reduction in dB while the gate is closed.
pub const DEFAULT_REDUCTION_DB: f64 = 30.0;
/// Default attack in ms (quick, so word onsets aren't clipped).
pub const DEFAULT_ATTACK_MS: f64 = 10.0;
/// Default release in ms (smooth close, avoids chattering).
pub const DEFAULT_RELEASE_MS: f64 = 250.0;
/// Default ratio — a firm downward gate.
pub const DEFAULT_RATIO: f64 = 2.0;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`2` not `2.0`,
/// `-18.5` stays `-18.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.5}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn validate_range(name: &str, v: f64, min: f64, max: f64, unit: &str) -> Result<(), String> {
    if !v.is_finite() || v < min || v > max {
        return Err(format!(
            "{name} must be between {} and {}{unit}, got {}",
            fmt_num(min),
            fmt_num(max),
            fmt_num(v)
        ));
    }
    Ok(())
}

/// Convert dB to the linear amplitude scalar ffmpeg's `agate` expects.
fn db_to_linear(db: f64) -> f64 {
    10_f64.powf(db / 20.0)
}

/// Build the `agate` filter string from the controls. Values are assumed
/// already range-validated. The UI exposes `threshold` in dB and `reduction`
/// as a positive dB attenuation, but ffmpeg's `agate` wants linear scalars:
/// `threshold` is a 0..1 amplitude and `range` is the 0..1 residual-gain floor
/// (1 = untouched, 0 = fully muted), so `reduction` dB maps to
/// `range = 10^(-reduction/20)`.
pub fn build_filter(
    threshold: f64,
    reduction: f64,
    ratio: f64,
    attack: f64,
    release: f64,
    detection: Detection,
) -> String {
    let threshold_linear = db_to_linear(threshold);
    let range_linear = db_to_linear(-reduction);
    format!(
        "agate=threshold={}:range={}:ratio={}:attack={}:release={}:detection={}",
        fmt_num(threshold_linear),
        fmt_num(range_linear),
        fmt_num(ratio),
        fmt_num(attack),
        fmt_num(release),
        detection.as_str()
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to gate `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, filter: &str, format: Format) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        filter.to_string(),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate the six controls + `format`, then return `(argv, out_name)`.
/// Rejects a pure no-op (0 dB reduction leaves the signal untouched). Single
/// source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
#[allow(clippy::too_many_arguments)]
pub fn plan_gate(
    in_name: &str,
    threshold: f64,
    reduction: f64,
    ratio: f64,
    attack: f64,
    release: f64,
    detection: &str,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    validate_range("threshold", threshold, THRESHOLD_MIN_DB, THRESHOLD_MAX_DB, " dB")?;
    validate_range("reduction", reduction, REDUCTION_MIN_DB, REDUCTION_MAX_DB, " dB")?;
    validate_range("ratio", ratio, RATIO_MIN, RATIO_MAX, ":1")?;
    validate_range("attack", attack, ATTACK_MIN_MS, ATTACK_MAX_MS, " ms")?;
    validate_range("release", release, RELEASE_MIN_MS, RELEASE_MAX_MS, " ms")?;
    if reduction == 0.0 {
        return Err(
            "reduction 0 dB does nothing — the gate would leave quiet passages untouched. \
             Raise reduction (e.g. 30 dB to push background down, or 80 dB for near-silence)"
                .to_string(),
        );
    }
    let det = parse_detection(detection)?;
    let fmt = parse_format(format)?;
    let filter = build_filter(threshold, reduction, ratio, attack, release, det);
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &filter, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_argv_order_and_values() {
        let (argv, out) =
            plan_gate("in.mp3", -35.0, 30.0, 2.0, 10.0, 250.0, "rms", "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "agate=threshold=0.01778:range=0.03162:ratio=2:attack=10:release=250:detection=rms",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "192k",
                "out.mp3",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn db_controls_are_converted_to_ffmpeg_linear_scalars() {
        // threshold 0 dB -> linear 1; reduction 80 dB -> range ~0.0001 (near
        // silence). reduction 0 is a no-op so it can't be used here.
        let f = build_filter(0.0, 80.0, 5.0, 0.01, 0.01, Detection::Peak);
        assert!(f.contains("threshold=1:"), "{f}");
        assert!(f.contains("range=0.0001:"), "{f}");
        assert!(f.contains("detection=peak"), "{f}");
    }

    #[test]
    fn detection_defaults_and_parses() {
        assert_eq!(parse_detection("").unwrap(), Detection::Rms);
        assert_eq!(parse_detection("RMS").unwrap(), Detection::Rms);
        assert_eq!(parse_detection("Peak").unwrap(), Detection::Peak);
        assert!(parse_detection("avg").is_err());
    }

    #[test]
    fn every_format_maps_to_its_codec() {
        for (f, codec) in [
            ("mp3", "libmp3lame"),
            ("wav", "pcm_s16le"),
            ("ogg", "libvorbis"),
            ("flac", "flac"),
            ("m4a", "aac"),
        ] {
            let (argv, out) =
                plan_gate("in.mp3", -35.0, 30.0, 2.0, 10.0, 250.0, "rms", f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
            assert!(out.ends_with(f), "out name keeps format ext: {out}");
        }
    }

    #[test]
    fn parse_format_defaults_empty_to_mp3() {
        assert_eq!(parse_format("").unwrap(), Format::Mp3);
        assert_eq!(parse_format("FLAC").unwrap(), Format::Flac);
        assert!(parse_format("aiff").is_err());
    }

    #[test]
    fn boundaries_are_accepted() {
        assert!(plan_gate("a.mp3", -80.0, 0.001, 1.0, 0.01, 0.01, "rms", "mp3").is_ok());
        assert!(plan_gate("a.mp3", 0.0, 80.0, 20.0, 2000.0, 9000.0, "peak", "mp3").is_ok());
    }

    #[test]
    fn out_of_range_controls_are_rejected_and_named() {
        // threshold too low / too high
        assert!(plan_gate("a.mp3", -81.0, 30.0, 2.0, 10.0, 250.0, "rms", "mp3").is_err());
        let err = plan_gate("a.mp3", 0.5, 30.0, 2.0, 10.0, 250.0, "rms", "mp3").unwrap_err();
        assert!(err.contains("threshold"), "{err}");
        // reduction above 80
        let err = plan_gate("a.mp3", -35.0, 81.0, 2.0, 10.0, 250.0, "rms", "mp3").unwrap_err();
        assert!(err.contains("reduction"), "{err}");
        // ratio below 1 / above 20
        assert!(plan_gate("a.mp3", -35.0, 30.0, 0.5, 10.0, 250.0, "rms", "mp3").is_err());
        let err = plan_gate("a.mp3", -35.0, 30.0, 25.0, 10.0, 250.0, "rms", "mp3").unwrap_err();
        assert!(err.contains("ratio"), "{err}");
        // attack / release out of range
        assert!(plan_gate("a.mp3", -35.0, 30.0, 2.0, 0.0, 250.0, "rms", "mp3").is_err());
        assert!(plan_gate("a.mp3", -35.0, 30.0, 2.0, 10.0, 9001.0, "rms", "mp3").is_err());
        // bad detection
        assert!(plan_gate("a.mp3", -35.0, 30.0, 2.0, 10.0, 250.0, "avg", "mp3").is_err());
        // non-finite
        assert!(plan_gate("a.mp3", f64::NAN, 30.0, 2.0, 10.0, 250.0, "rms", "mp3").is_err());
    }

    #[test]
    fn zero_reduction_is_rejected_as_no_op() {
        let err = plan_gate("a.mp3", -35.0, 0.0, 2.0, 10.0, 250.0, "rms", "mp3").unwrap_err();
        assert!(err.contains("does nothing"), "{err}");
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) =
            plan_gate("in.mp3", -35.0, 30.0, 2.0, 10.0, 250.0, "rms", "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(2.0), "2");
        assert_eq!(fmt_num(-35.0), "-35");
        assert_eq!(fmt_num(2.5), "2.5");
    }
}
