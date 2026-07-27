//! gizza-ai/audio-compressor core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Dynamic-range compression via ffmpeg's `acompressor` filter: quiet and loud
//! passages are pushed closer together so the audio sits at a steadier level.
//! The four classic compressor controls are exposed directly —
//! `threshold` (dB, where compression starts), `ratio` (how hard the signal
//! above the threshold is squeezed), `attack` (ms to clamp down) and
//! `release` (ms to let go) — plus a `makeup` gain (dB) to restore the loudness
//! the compressor pulled down. This is loudness/dynamics compression, NOT
//! file-size compression (that's the separate audio-compress tool). `-vn` drops
//! attached-picture streams (album art); the output is re-encoded because
//! `acompressor` rewrites samples.

/// Output audio formats audio-compressor can write (family-standard set).
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

// Accepted control ranges. These mirror ffmpeg `acompressor`'s own limits where
// a musical range is narrower: threshold is expressed in dB (acompressor maps
// -60..0 dB into its 0.000977..1 linear window), makeup as a dB boost (0 dB =
// unity, the filter's `1`; +24 dB ≈ 15.8× stays well inside its 1..64 range).
/// Threshold, in dB below full scale, where compression begins.
pub const THRESHOLD_MIN_DB: f64 = -60.0;
pub const THRESHOLD_MAX_DB: f64 = 0.0;
/// Compression ratio (`ratio:1`). 1 = no compression; 20 = near-limiting.
pub const RATIO_MIN: f64 = 1.0;
pub const RATIO_MAX: f64 = 20.0;
/// Attack time in milliseconds.
pub const ATTACK_MIN_MS: f64 = 0.01;
pub const ATTACK_MAX_MS: f64 = 2000.0;
/// Release time in milliseconds.
pub const RELEASE_MIN_MS: f64 = 0.01;
pub const RELEASE_MAX_MS: f64 = 9000.0;
/// Make-up gain in dB (0 = leave level as the compressor set it).
pub const MAKEUP_MIN_DB: f64 = 0.0;
pub const MAKEUP_MAX_DB: f64 = 24.0;

// Family defaults — the single source for the chat descriptor defaults and the
// page's "empty field means use the default" fallbacks (see `web/src/lib.rs`).
/// Default threshold in dB (catches program peaks without over-compressing).
pub const DEFAULT_THRESHOLD_DB: f64 = -20.0;
/// Default ratio — a firm, even level.
pub const DEFAULT_RATIO: f64 = 4.0;
/// Default attack in ms.
pub const DEFAULT_ATTACK_MS: f64 = 20.0;
/// Default release in ms.
pub const DEFAULT_RELEASE_MS: f64 = 250.0;
/// Default make-up gain in dB (0 = no post-compression level boost).
pub const DEFAULT_MAKEUP_DB: f64 = 0.0;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`4` not `4.0`,
/// `-18.5` stays `-18.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
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

/// Convert dB to the linear amplitude scalar ffmpeg's `acompressor` expects.
fn db_to_linear(db: f64) -> f64 {
    10_f64.powf(db / 20.0)
}

/// Build the `acompressor` filter string from the five controls. Values are
/// assumed already range-validated. The UI exposes `threshold`/`makeup` in dB,
/// but ffmpeg's `acompressor` options are linear scalars: threshold is
/// 0.000976563..1 and makeup is 1..64.
pub fn build_filter(threshold: f64, ratio: f64, attack: f64, release: f64, makeup: f64) -> String {
    let threshold_linear = db_to_linear(threshold);
    let makeup_linear = db_to_linear(makeup);
    format!(
        "acompressor=threshold={}:ratio={}:attack={}:release={}:makeup={}",
        fmt_num(threshold_linear),
        fmt_num(ratio),
        fmt_num(attack),
        fmt_num(release),
        fmt_num(makeup_linear)
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to compress `in_name` into
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

/// Validate the five controls + `format`, then return `(argv, out_name)`.
/// Rejects a pure no-op (ratio 1 with no make-up gain does nothing). Single
/// source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
#[allow(clippy::too_many_arguments)]
pub fn plan_compress(
    in_name: &str,
    threshold: f64,
    ratio: f64,
    attack: f64,
    release: f64,
    makeup: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    validate_range(
        "threshold",
        threshold,
        THRESHOLD_MIN_DB,
        THRESHOLD_MAX_DB,
        " dB",
    )?;
    validate_range("ratio", ratio, RATIO_MIN, RATIO_MAX, ":1")?;
    validate_range("attack", attack, ATTACK_MIN_MS, ATTACK_MAX_MS, " ms")?;
    validate_range("release", release, RELEASE_MIN_MS, RELEASE_MAX_MS, " ms")?;
    validate_range("makeup", makeup, MAKEUP_MIN_DB, MAKEUP_MAX_DB, " dB")?;
    if ratio <= 1.0 && makeup == 0.0 {
        return Err(
            "ratio 1 with 0 dB make-up gain does nothing — raise ratio above 1 to compress \
             (e.g. 4 for a firm, even level) or add make-up gain to boost the level"
                .to_string(),
        );
    }
    let fmt = parse_format(format)?;
    let filter = build_filter(threshold, ratio, attack, release, makeup);
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &filter, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_argv_order_and_values() {
        let (argv, out) = plan_compress("in.mp3", -20.0, 4.0, 20.0, 250.0, 6.0, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "acompressor=threshold=0.1:ratio=4:attack=20:release=250:makeup=1.995",
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
    fn fractional_values_stay_precise_in_filter() {
        let f = build_filter(-18.5, 2.5, 0.5, 125.0, 3.0);
        assert_eq!(
            f,
            "acompressor=threshold=0.119:ratio=2.5:attack=0.5:release=125:makeup=1.413"
        );
    }

    #[test]
    fn db_controls_are_converted_to_ffmpeg_linear_scalars() {
        let f = build_filter(-60.0, 1.0, 0.01, 0.01, 24.0);
        assert!(f.contains("threshold=0.001"), "{f}");
        assert!(f.contains("makeup=15.849"), "{f}");
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
            let (argv, out) = plan_compress("in.mp3", -20.0, 4.0, 20.0, 250.0, 0.0, f).unwrap();
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
        assert!(plan_compress("a.mp3", -60.0, 1.0, 0.01, 0.01, 24.0, "mp3").is_ok());
        assert!(plan_compress("a.mp3", 0.0, 20.0, 2000.0, 9000.0, 0.0, "mp3").is_ok());
    }

    #[test]
    fn out_of_range_controls_are_rejected_and_named() {
        // threshold too low / too high
        assert!(plan_compress("a.mp3", -61.0, 4.0, 20.0, 250.0, 0.0, "mp3").is_err());
        let err = plan_compress("a.mp3", 0.5, 4.0, 20.0, 250.0, 0.0, "mp3").unwrap_err();
        assert!(err.contains("threshold"), "{err}");
        // ratio below 1 / above 20
        assert!(plan_compress("a.mp3", -20.0, 0.5, 20.0, 250.0, 0.0, "mp3").is_err());
        let err = plan_compress("a.mp3", -20.0, 25.0, 20.0, 250.0, 0.0, "mp3").unwrap_err();
        assert!(err.contains("ratio"), "{err}");
        // attack / release out of range
        assert!(plan_compress("a.mp3", -20.0, 4.0, 0.0, 250.0, 0.0, "mp3").is_err());
        assert!(plan_compress("a.mp3", -20.0, 4.0, 20.0, 9001.0, 0.0, "mp3").is_err());
        // makeup out of range
        assert!(plan_compress("a.mp3", -20.0, 4.0, 20.0, 250.0, -1.0, "mp3").is_err());
        let err = plan_compress("a.mp3", -20.0, 4.0, 20.0, 250.0, 25.0, "mp3").unwrap_err();
        assert!(err.contains("makeup"), "{err}");
        // non-finite
        assert!(plan_compress("a.mp3", f64::NAN, 4.0, 20.0, 250.0, 0.0, "mp3").is_err());
    }

    #[test]
    fn pure_no_op_is_rejected() {
        let err = plan_compress("a.mp3", -20.0, 1.0, 20.0, 250.0, 0.0, "mp3").unwrap_err();
        assert!(err.contains("does nothing"), "{err}");
        // ratio 1 WITH make-up gain is a valid level boost, not a no-op.
        assert!(plan_compress("a.mp3", -20.0, 1.0, 20.0, 250.0, 6.0, "mp3").is_ok());
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_compress("in.mp3", -20.0, 4.0, 20.0, 250.0, 0.0, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(4.0), "4");
        assert_eq!(fmt_num(-20.0), "-20");
        assert_eq!(fmt_num(2.5), "2.5");
    }
}
