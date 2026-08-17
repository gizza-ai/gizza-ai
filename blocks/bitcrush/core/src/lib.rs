//! gizza-ai/bitcrush core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Bitcrushing is two independent degradations applied together: quantising each
//! sample to fewer bits (adds gritty quantisation noise) and holding each sample
//! for several output samples (drops the effective sample rate and folds the top
//! of the spectrum back down as aliasing). Both come from stock ffmpeg's
//! `acrusher` filter: `bits` sets the resolution, `samples` sets how many output
//! samples share one input value.
//!
//! `acrusher`'s `samples` is a *divisor* of whatever rate reaches the filter, so
//! a target expressed in Hz would mean something different for a 44.1 kHz file
//! than for a 48 kHz one. To make the advertised target rate exact for every
//! input, the chain resamples to a fixed [`WORK_RATE`] (48 kHz) first and then
//! crushes: `samples = round(48000 / target)`. The rate therefore snaps to the
//! nearest `48000 ÷ N` — [`effective_rate_hz`] reports what was actually applied.
//!
//! Inputs are capped at 10 MiB. `-vn` drops attached-picture streams (album art).

/// Output audio formats bitcrush can write (family-standard set).
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

/// Quantiser curve: linear steps (classic fixed-point crush) or logarithmic
/// steps (coarser near silence, closer to old companded gear).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    Lin,
    Log,
}

impl Mode {
    /// The value `acrusher`'s `mode` option expects.
    pub fn as_arg(self) -> &'static str {
        match self {
            Mode::Lin => "lin",
            Mode::Log => "log",
        }
    }
}

/// Parse the user-facing mode string. Empty defaults to linear.
pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "lin" => Ok(Mode::Lin),
        "log" => Ok(Mode::Log),
        other => Err(format!("mode {other:?} not supported (lin|log)")),
    }
}

/// Fixed rate the audio is resampled to before crushing, so a target sample
/// rate in Hz means the same thing for every input file.
pub const WORK_RATE: f64 = 48_000.0;
/// `acrusher` accepts a sample-hold divisor of 1–250; that bounds the reachable
/// target rate to `WORK_RATE / 250` … `WORK_RATE`.
const MIN_HOLD: f64 = 1.0;
const MAX_HOLD: f64 = 250.0;

// Defaults (kept in sync with the descriptor + the drift-guard schema).
pub const DEFAULT_BITS: f64 = 8.0;
pub const DEFAULT_SAMPLE_RATE_HZ: f64 = 8000.0;
pub const DEFAULT_MIX: f64 = 1.0;
pub const DEFAULT_DRIVE: f64 = 1.0;
pub const DEFAULT_OUTPUT_GAIN: f64 = 1.0;
pub const DEFAULT_ANTI_ALIAS: f64 = 0.5;

// Accepted ranges (kept in sync with the descriptor's .min()/.max()).
pub const MIN_BITS: f64 = 1.0;
pub const MAX_BITS: f64 = 16.0;
pub const MIN_SAMPLE_RATE_HZ: f64 = 192.0;
pub const MAX_SAMPLE_RATE_HZ: f64 = 48_000.0;
pub const MIN_MIX: f64 = 0.0;
pub const MAX_MIX: f64 = 1.0;
pub const MIN_DRIVE: f64 = 0.25;
pub const MAX_DRIVE: f64 = 4.0;
pub const MIN_OUTPUT_GAIN: f64 = 0.25;
pub const MAX_OUTPUT_GAIN: f64 = 4.0;
pub const MIN_ANTI_ALIAS: f64 = 0.0;
pub const MAX_ANTI_ALIAS: f64 = 1.0;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`3` not `3.0`,
/// `0.5` stays `0.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn validate_range(name: &str, v: f64, lo: f64, hi: f64) -> Result<(), String> {
    if !v.is_finite() || !(lo..=hi).contains(&v) {
        return Err(format!(
            "{name} must be between {} and {}, got {v}",
            fmt_num(lo),
            fmt_num(hi)
        ));
    }
    Ok(())
}

/// How many output samples share one input value for a requested target rate.
/// Whole numbers only — `acrusher` holds a whole count of samples, so a request
/// that isn't an exact divisor of [`WORK_RATE`] snaps to the nearest one.
pub fn hold_samples(sample_rate_hz: f64) -> f64 {
    (WORK_RATE / sample_rate_hz)
        .round()
        .clamp(MIN_HOLD, MAX_HOLD)
}

/// The sample rate actually produced for a requested target, after snapping to
/// the nearest whole sample-hold count (e.g. 22050 → 24000, 8000 → 8000).
pub fn effective_rate_hz(sample_rate_hz: f64) -> f64 {
    WORK_RATE / hold_samples(sample_rate_hz)
}

/// Build the `aresample=…,acrusher=…` filter chain. Assumes the inputs have
/// already been validated (see [`plan_bitcrush`]). `drive` scales the signal on
/// the way into the quantiser (more drive = more of the signal spread across
/// fewer steps = harsher crush); `output_gain` scales what comes back out, so a
/// hot drive can be brought back down without undoing the distortion it caused.
pub fn build_filter(
    bits: f64,
    sample_rate_hz: f64,
    mix: f64,
    drive: f64,
    output_gain: f64,
    anti_alias: f64,
    mode: Mode,
) -> String {
    format!(
        "aresample={},acrusher=level_in={}:level_out={}:bits={}:mode={}:mix={}:aa={}:samples={}",
        fmt_num(WORK_RATE),
        fmt_num(drive),
        fmt_num(output_gain),
        fmt_num(bits),
        mode.as_arg(),
        fmt_num(mix),
        fmt_num(anti_alias),
        fmt_num(hold_samples(sample_rate_hz)),
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`) applying `filter` to `in_name`
/// and writing `out_name`. Shared verbatim by the web page (`build_argv`) and
/// the chat block.
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

/// Validate every control, parse `mode`/`format`, and return `(argv, out_name)`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
#[allow(clippy::too_many_arguments)]
pub fn plan_bitcrush(
    in_name: &str,
    bits: f64,
    sample_rate_hz: f64,
    mix: f64,
    drive: f64,
    output_gain: f64,
    anti_alias: f64,
    mode: &str,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    validate_range("bits", bits, MIN_BITS, MAX_BITS)?;
    validate_range(
        "sample_rate_hz",
        sample_rate_hz,
        MIN_SAMPLE_RATE_HZ,
        MAX_SAMPLE_RATE_HZ,
    )?;
    validate_range("mix", mix, MIN_MIX, MAX_MIX)?;
    validate_range("drive", drive, MIN_DRIVE, MAX_DRIVE)?;
    validate_range("output_gain", output_gain, MIN_OUTPUT_GAIN, MAX_OUTPUT_GAIN)?;
    validate_range("anti_alias", anti_alias, MIN_ANTI_ALIAS, MAX_ANTI_ALIAS)?;
    let md = parse_mode(mode)?;
    let fmt = parse_format(format)?;
    let filter = build_filter(
        bits,
        sample_rate_hz,
        mix,
        drive,
        output_gain,
        anti_alias,
        md,
    );
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &filter, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_argv_order_and_values() {
        let (argv, out) = plan_bitcrush(
            "in.mp3",
            DEFAULT_BITS,
            DEFAULT_SAMPLE_RATE_HZ,
            DEFAULT_MIX,
            DEFAULT_DRIVE,
            DEFAULT_OUTPUT_GAIN,
            DEFAULT_ANTI_ALIAS,
            "lin",
            "mp3",
        )
        .unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                // 48000 / 8000 = 6 held samples
                "aresample=48000,acrusher=level_in=1:level_out=1:bits=8:mode=lin:mix=1:aa=0.5:samples=6",
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
    fn out_of_range_controls_are_rejected_by_name() {
        for (bad, needle) in [
            ((0.5, 8000.0, 1.0, 1.0, 1.0, 0.5), "bits"),
            ((17.0, 8000.0, 1.0, 1.0, 1.0, 0.5), "bits"),
            ((8.0, 100.0, 1.0, 1.0, 1.0, 0.5), "sample_rate_hz"),
            ((8.0, 96_000.0, 1.0, 1.0, 1.0, 0.5), "sample_rate_hz"),
            ((8.0, 8000.0, 1.5, 1.0, 1.0, 0.5), "mix"),
            ((8.0, 8000.0, 1.0, 5.0, 1.0, 0.5), "drive"),
            ((8.0, 8000.0, 1.0, 0.1, 1.0, 0.5), "drive"),
            ((8.0, 8000.0, 1.0, 1.0, 8.0, 0.5), "output_gain"),
            ((8.0, 8000.0, 1.0, 1.0, 0.0, 0.5), "output_gain"),
            ((8.0, 8000.0, 1.0, 1.0, 1.0, 1.5), "anti_alias"),
        ] {
            let (b, sr, mx, dr, og, aa) = bad;
            let err = plan_bitcrush("a.mp3", b, sr, mx, dr, og, aa, "lin", "mp3").unwrap_err();
            assert!(err.contains(needle), "expected {needle} in {err:?}");
        }
        // NaN is rejected rather than silently formatted into the filter.
        assert!(
            plan_bitcrush("a.mp3", f64::NAN, 8000.0, 1.0, 1.0, 1.0, 0.5, "lin", "mp3").is_err()
        );
    }

    #[test]
    fn range_boundaries_are_valid() {
        assert!(plan_bitcrush(
            "a.mp3",
            MIN_BITS,
            MIN_SAMPLE_RATE_HZ,
            MIN_MIX,
            MIN_DRIVE,
            MIN_OUTPUT_GAIN,
            MIN_ANTI_ALIAS,
            "lin",
            "mp3"
        )
        .is_ok());
        assert!(plan_bitcrush(
            "a.mp3",
            MAX_BITS,
            MAX_SAMPLE_RATE_HZ,
            MAX_MIX,
            MAX_DRIVE,
            MAX_OUTPUT_GAIN,
            MAX_ANTI_ALIAS,
            "log",
            "flac"
        )
        .is_ok());
    }

    #[test]
    fn hold_count_snaps_to_whole_divisors_of_the_work_rate() {
        // exact divisors stay exact
        assert_eq!(hold_samples(48_000.0), 1.0);
        assert_eq!(hold_samples(8000.0), 6.0);
        assert_eq!(hold_samples(4000.0), 12.0);
        assert_eq!(effective_rate_hz(8000.0), 8000.0);
        // 48000/22050 = 2.177 → 2 held samples → 24 kHz
        assert_eq!(hold_samples(22_050.0), 2.0);
        assert_eq!(effective_rate_hz(22_050.0), 24_000.0);
        // 48000/11025 = 4.354 → 4 → 12 kHz
        assert_eq!(effective_rate_hz(11_025.0), 12_000.0);
        // the bottom of the range stays inside acrusher's 1..=250 hold limit
        assert_eq!(hold_samples(MIN_SAMPLE_RATE_HZ), 250.0);
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
                plan_bitcrush("in.wav", 8.0, 8000.0, 1.0, 1.0, 1.0, 0.5, "lin", f).unwrap();
            assert!(out.ends_with(f), "out name {out} uses .{f}");
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn both_modes_reach_the_filter() {
        for (m, expect) in [("lin", "mode=lin"), ("LOG", "mode=log"), ("", "mode=lin")] {
            let (argv, _) =
                plan_bitcrush("in.wav", 8.0, 8000.0, 1.0, 1.0, 1.0, 0.5, m, "mp3").unwrap();
            assert!(argv.iter().any(|a| a.contains(expect)), "mode {m:?}");
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) =
            plan_bitcrush("in.mp3", 8.0, 8000.0, 1.0, 1.0, 1.0, 0.5, "lin", "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn bad_mode_and_format_are_rejected() {
        let err =
            plan_bitcrush("in.mp3", 8.0, 8000.0, 1.0, 1.0, 1.0, 0.5, "sine", "mp3").unwrap_err();
        assert!(err.contains("not supported"), "{err}");
        let err =
            plan_bitcrush("in.mp3", 8.0, 8000.0, 1.0, 1.0, 1.0, 0.5, "lin", "aiff").unwrap_err();
        assert!(err.contains("not supported"), "{err}");
    }

    #[test]
    fn dry_mix_and_fractional_controls_format_compactly() {
        let f = build_filter(12.0, 22_050.0, 0.35, 1.5, 1.0, 0.0, Mode::Log);
        assert_eq!(
            f,
            "aresample=48000,acrusher=level_in=1.5:level_out=1:bits=12:mode=log:mix=0.35:aa=0:samples=2"
        );
    }

    #[test]
    fn output_gain_rides_level_out_independently_of_drive() {
        // Drive hot into the quantiser, pull the result back down on the way out:
        // the crush stays, the clipping headroom comes back.
        let f = build_filter(6.0, 8000.0, 1.0, 3.0, 0.4, 0.5, Mode::Lin);
        assert!(f.contains("level_in=3"), "{f}");
        assert!(f.contains("level_out=0.4"), "{f}");
        let (argv, _) =
            plan_bitcrush("in.mp3", 6.0, 8000.0, 1.0, 3.0, 0.4, 0.5, "lin", "mp3").unwrap();
        assert!(
            argv.iter().any(|a| a.contains("level_out=0.4")),
            "output_gain reaches the argv: {argv:?}"
        );
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(3.0), "3");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(0.35), "0.35");
        assert_eq!(fmt_num(48_000.0), "48000");
    }
}
