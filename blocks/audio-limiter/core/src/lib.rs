//! gizza-ai/audio-limiter core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Brick-wall peak limiting via ffmpeg's `alimiter` (lookahead) filter: an
//! optional input gain drives the signal, then a hard ceiling stops any peak
//! from crossing it. The UI speaks dB — `ceiling` (dBFS, the brick wall) and
//! `gain` (dB of drive applied before limiting) — while `alimiter` wants linear
//! scalars (`limit` 0.0625..1, `level_in` 0.015625..64), so core converts.
//! `attack`/`release` are the filter's own millisecond ranges. `smooth_release`
//! maps to the filter's `asc` option (release times averaged over recent gain
//! reduction, which sounds less pumpy on dense material) and `auto_level` maps
//! to its `level` option (re-normalize the limited signal back up to full
//! scale, i.e. loudness-maximizer behaviour — OFF by default so the ceiling is
//! actually honoured).
//!
//! This is peak limiting, NOT loudness normalization (that's `audio-normalize`)
//! and NOT dynamic-range compression with a ratio (that's `audio-compressor`).
//! `-vn` drops attached-picture streams (album art); the output is re-encoded
//! because the limiter rewrites samples.

/// Output audio formats audio-limiter can write (family-standard set).
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

    /// True when the format stores samples losslessly, so the written peaks
    /// match the limiter's ceiling exactly (lossy codecs can overshoot).
    pub fn is_lossless(self) -> bool {
        matches!(self, Format::Wav | Format::Flac)
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

// Accepted control ranges. Each one is the dB image of `alimiter`'s own linear
// limit, so nothing we emit can be clamped or rejected by the filter:
// `limit` 0.0625..1 → -24.08..0 dB (we stop at -24 dB, safely inside), and
// `level_in` 0.015625..64 → -36..+36 dB (we stop at ±20 dB, a musical range).
/// Brick-wall ceiling in dBFS: no output sample is allowed above this.
pub const CEILING_MIN_DB: f64 = -24.0;
pub const CEILING_MAX_DB: f64 = 0.0;
/// Input gain (drive) in dB applied BEFORE the ceiling.
pub const GAIN_MIN_DB: f64 = -20.0;
pub const GAIN_MAX_DB: f64 = 20.0;
/// Lookahead attack time in milliseconds (the filter's own range).
pub const ATTACK_MIN_MS: f64 = 0.1;
pub const ATTACK_MAX_MS: f64 = 80.0;
/// Release time in milliseconds (the filter's own range).
pub const RELEASE_MIN_MS: f64 = 1.0;
pub const RELEASE_MAX_MS: f64 = 8000.0;

// Family defaults — the single source for the chat descriptor defaults and the
// page's "empty field means use the default" fallbacks (see `web/src/lib.rs`).
/// Default ceiling in dBFS (-1 leaves the headroom lossy encoders need).
pub const DEFAULT_CEILING_DB: f64 = -1.0;
/// Default input gain in dB (0 = limit only, don't drive the signal).
pub const DEFAULT_GAIN_DB: f64 = 0.0;
/// Default attack in ms (fast enough to catch transients transparently).
pub const DEFAULT_ATTACK_MS: f64 = 5.0;
/// Default release in ms.
pub const DEFAULT_RELEASE_MS: f64 = 50.0;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`5` not `5.0`,
/// `-1.5` stays `-1.5`) — compact and locale-independent.
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

/// Convert dB to the linear amplitude scalar ffmpeg's `alimiter` expects.
fn db_to_linear(db: f64) -> f64 {
    10_f64.powf(db / 20.0)
}

/// Build the `alimiter` filter string. Values are assumed already
/// range-validated. Booleans are emitted as `0`/`1` (the widest-compatible
/// spelling of an ffmpeg boolean option, including the browser ffmpeg build).
pub fn build_filter(
    ceiling: f64,
    gain: f64,
    attack: f64,
    release: f64,
    smooth_release: bool,
    auto_level: bool,
) -> String {
    format!(
        "alimiter=level_in={}:level_out=1:limit={}:attack={}:release={}:asc={}:level={}",
        fmt_num(db_to_linear(gain)),
        fmt_num(db_to_linear(ceiling)),
        fmt_num(attack),
        fmt_num(release),
        u8::from(smooth_release),
        u8::from(auto_level)
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to limit `in_name` into
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

/// Validate the controls + `format`, then return `(argv, out_name)`. Rejects a
/// pure no-op (a 0 dB ceiling with no drive, no auto-level and no smoothing
/// leaves already-legal audio untouched). Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
#[allow(clippy::too_many_arguments)]
pub fn plan_limit(
    in_name: &str,
    ceiling: f64,
    gain: f64,
    attack: f64,
    release: f64,
    smooth_release: bool,
    auto_level: bool,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    validate_range("ceiling", ceiling, CEILING_MIN_DB, CEILING_MAX_DB, " dB")?;
    validate_range("gain", gain, GAIN_MIN_DB, GAIN_MAX_DB, " dB")?;
    validate_range("attack", attack, ATTACK_MIN_MS, ATTACK_MAX_MS, " ms")?;
    validate_range("release", release, RELEASE_MIN_MS, RELEASE_MAX_MS, " ms")?;
    if ceiling == 0.0 && gain == 0.0 && !auto_level {
        return Err(
            "a 0 dB ceiling with 0 dB gain leaves already-legal audio unchanged — lower the \
             ceiling (e.g. -1 dB, the usual safety margin) or add input gain to push the level up"
                .to_string(),
        );
    }
    let fmt = parse_format(format)?;
    let filter = build_filter(ceiling, gain, attack, release, smooth_release, auto_level);
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &filter, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_argv_order_and_values() {
        let (argv, out) =
            plan_limit("in.mp3", -1.0, 0.0, 5.0, 50.0, false, false, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "alimiter=level_in=1:level_out=1:limit=0.891:attack=5:release=50:asc=0:level=0",
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
        // -6 dB ceiling ≈ 0.501 linear, +6 dB drive ≈ 1.995 linear.
        let f = build_filter(-6.0, 6.0, 1.0, 100.0, false, false);
        assert!(f.contains("limit=0.501"), "{f}");
        assert!(f.contains("level_in=1.995"), "{f}");
        // The extreme ends stay inside alimiter's own accepted windows
        // (limit 0.0625..1, level_in 0.015625..64).
        let f = build_filter(CEILING_MIN_DB, GAIN_MIN_DB, 0.1, 1.0, false, false);
        assert!(f.contains("limit=0.063"), "{f}");
        assert!(f.contains("level_in=0.1"), "{f}");
        let f = build_filter(CEILING_MAX_DB, GAIN_MAX_DB, 80.0, 8000.0, false, false);
        assert!(f.contains("limit=1"), "{f}");
        assert!(f.contains("level_in=10"), "{f}");
    }

    #[test]
    fn booleans_emit_numeric_flags() {
        let off = build_filter(-1.0, 0.0, 5.0, 50.0, false, false);
        assert!(off.contains("asc=0") && off.contains("level=0"), "{off}");
        let on = build_filter(-1.0, 0.0, 5.0, 50.0, true, true);
        assert!(on.contains("asc=1") && on.contains("level=1"), "{on}");
    }

    #[test]
    fn fractional_values_stay_precise_in_filter() {
        let f = build_filter(-1.5, -3.5, 0.5, 125.0, false, false);
        assert_eq!(
            f,
            "alimiter=level_in=0.668:level_out=1:limit=0.841:attack=0.5:release=125:asc=0:level=0"
        );
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
                plan_limit("in.mp3", -1.0, 0.0, 5.0, 50.0, false, false, f).unwrap();
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
        assert!(Format::Wav.is_lossless() && Format::Flac.is_lossless());
        assert!(!Format::Mp3.is_lossless() && !Format::M4a.is_lossless());
    }

    #[test]
    fn boundaries_are_accepted() {
        assert!(plan_limit("a.mp3", -24.0, -20.0, 0.1, 1.0, true, false, "mp3").is_ok());
        assert!(plan_limit("a.mp3", 0.0, 20.0, 80.0, 8000.0, false, true, "mp3").is_ok());
    }

    #[test]
    fn out_of_range_controls_are_rejected_and_named() {
        // ceiling below alimiter's floor / above 0 dBFS
        assert!(plan_limit("a.mp3", -24.1, 0.0, 5.0, 50.0, false, false, "mp3").is_err());
        let err = plan_limit("a.mp3", 0.5, 0.0, 5.0, 50.0, false, false, "mp3").unwrap_err();
        assert!(err.contains("ceiling"), "{err}");
        // gain out of range
        let err = plan_limit("a.mp3", -1.0, 21.0, 5.0, 50.0, false, false, "mp3").unwrap_err();
        assert!(err.contains("gain"), "{err}");
        assert!(plan_limit("a.mp3", -1.0, -20.5, 5.0, 50.0, false, false, "mp3").is_err());
        // attack / release outside the filter's windows
        let err = plan_limit("a.mp3", -1.0, 0.0, 0.05, 50.0, false, false, "mp3").unwrap_err();
        assert!(err.contains("attack"), "{err}");
        assert!(plan_limit("a.mp3", -1.0, 0.0, 81.0, 50.0, false, false, "mp3").is_err());
        let err = plan_limit("a.mp3", -1.0, 0.0, 5.0, 0.5, false, false, "mp3").unwrap_err();
        assert!(err.contains("release"), "{err}");
        assert!(plan_limit("a.mp3", -1.0, 0.0, 5.0, 8001.0, false, false, "mp3").is_err());
        // non-finite
        assert!(plan_limit("a.mp3", f64::NAN, 0.0, 5.0, 50.0, false, false, "mp3").is_err());
    }

    #[test]
    fn pure_no_op_is_rejected() {
        let err = plan_limit("a.mp3", 0.0, 0.0, 5.0, 50.0, false, false, "mp3").unwrap_err();
        assert!(err.contains("unchanged"), "{err}");
        // A 0 dB ceiling is fine as long as something else does work:
        // drive the signal into it, or let auto-level maximize afterwards.
        assert!(plan_limit("a.mp3", 0.0, 3.0, 5.0, 50.0, false, false, "mp3").is_ok());
        assert!(plan_limit("a.mp3", 0.0, 0.0, 5.0, 50.0, false, true, "mp3").is_ok());
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_limit("in.mp3", -1.0, 0.0, 5.0, 50.0, false, false, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(5.0), "5");
        assert_eq!(fmt_num(-1.0), "-1");
        assert_eq!(fmt_num(0.5), "0.5");
    }
}
