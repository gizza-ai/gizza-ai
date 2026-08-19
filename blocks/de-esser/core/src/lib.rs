//! gizza-ai/de-esser core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! A de-esser tames harsh sibilance — the `s`, `sh`, `t` and `ts` bursts that
//! spike in the upper band of a vocal, narration or podcast track. Unlike a
//! static EQ cut (see audio-eq / audio-filter), a de-esser is DYNAMIC: it only
//! ducks the high band while sibilance is actually present, so the rest of the
//! voice keeps its air and brightness. It is also not a downward noise gate
//! (audio-noise-gate keys off overall level, not a band) and not spectral
//! denoising (audio-noise-reduce).
//!
//! Implementation uses ffmpeg's dedicated `deesser` filter, which splits the
//! signal with a one-pole crossover, measures the energy of the upper (ess)
//! band and applies level-dependent ducking to it. `-vn` drops attached-picture
//! streams (album art); output is re-encoded because de-essing rewrites samples.
//!
//! ## Why the controls are 1..100 scales, not Hz / dB
//!
//! ffmpeg's `deesser` takes three unitless 0..1 doubles (`i`, `m`, `f`) rather
//! than a centre frequency in Hz and a threshold in dB. `f` in particular is a
//! one-pole crossover coefficient (roughly `f²`), so the split point it implies
//! moves with the input sample rate and cannot be quoted as a fixed Hz value at
//! argv-build time. Rather than invent an unverifiable Hz mapping, the tool
//! exposes honest 1..100 percentage scales that map linearly onto the filter's
//! own 0..1 range, and documents the measured behaviour (see `page/content.md`).
//!
//! Measured on ffmpeg 7.1.4 with a 44.1 kHz mix of a 440 Hz tone and a 7 kHz
//! tone at equal level, `amount = 90`, `max_reduction = 50` — showing how `band`
//! trades sibilance reduction against collateral damage to the vocal body:
//!
//! | `band` | 440 Hz body | 7 kHz "ess" |
//! |-------:|------------:|------------:|
//! |     40 |    −2.3 dB  |    −16.6 dB |
//! |     50 |    −1.2 dB  |    −13.7 dB |
//! |     70 |    −0.5 dB  |     −8.2 dB |
//! |     90 |    −0.4 dB  |     −3.1 dB |
//!
//! Low `band` values pull the crossover down far enough that the body of the
//! voice is ducked too; high values restrict the effect to the very top. 70 is
//! the default because it is the highest setting that still removes a clearly
//! audible amount of sibilance while leaving the body essentially untouched.

/// Output audio formats de-esser can write (family-standard set).
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

/// What the filter writes to its output — the processed track, the sibilance it
/// removed (for auditioning), or the untouched input (for A/B comparison).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    /// The de-essed track. The normal choice.
    Output,
    /// Only the sibilance the de-esser is removing — an audition/"listen" mode
    /// for checking that the band is right before committing.
    Ess,
    /// The unprocessed input, so a before/after pair can be rendered.
    Input,
}

impl Mode {
    /// The `s=` value ffmpeg's `deesser` expects.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Output => "o",
            Mode::Ess => "e",
            Mode::Input => "i",
        }
    }
}

/// Parse the user-facing mode string. Empty defaults to output.
pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "output" => Ok(Mode::Output),
        "ess" => Ok(Mode::Ess),
        "input" => Ok(Mode::Input),
        other => Err(format!("mode {other:?} not supported (output|ess|input)")),
    }
}

// Accepted control ranges. Every control is a 1..100 percentage of ffmpeg's own
// 0..1 option range. The minimum is 1, not 0, because each control has a
// documented no-op at one end and excluding it keeps the tool from silently
// returning the input unchanged (see the per-constant notes).
/// How hard sibilance is ducked once detected (ffmpeg `i`). `i = 0` is a total
/// bypass, so the scale starts at 1.
pub const AMOUNT_MIN: f64 = 1.0;
pub const AMOUNT_MAX: f64 = 100.0;
/// Where the sibilance crossover sits (ffmpeg `f`). Higher = only the very top
/// of the spectrum is treated as sibilance.
pub const BAND_MIN: f64 = 1.0;
pub const BAND_MAX: f64 = 100.0;
/// Ceiling on how deep the ducking may go (ffmpeg `m`, inverted). At
/// `max_reduction = 0` ffmpeg's `m` would be 1.0, which nulls the effect
/// entirely, so the scale starts at 1.
pub const MAX_REDUCTION_MIN: f64 = 1.0;
pub const MAX_REDUCTION_MAX: f64 = 100.0;

// Family defaults — the single source for the chat descriptor defaults and the
// page's "empty field means use the default" fallbacks (see `web/src/lib.rs`).
/// Default amount — audible de-essing without a lisp.
pub const DEFAULT_AMOUNT: f64 = 60.0;
/// Default band — high enough to leave the body of the voice alone (see the
/// measured table in the module docs).
pub const DEFAULT_BAND: f64 = 70.0;
/// Default reduction ceiling — matches ffmpeg's own `m = 0.5`.
pub const DEFAULT_MAX_REDUCTION: f64 = 50.0;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`2` not `2.0`,
/// `0.7` stays `0.7`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.5}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn validate_range(name: &str, v: f64, min: f64, max: f64) -> Result<(), String> {
    if !v.is_finite() || v < min || v > max {
        return Err(format!(
            "{name} must be between {} and {}, got {}",
            fmt_num(min),
            fmt_num(max),
            fmt_num(v)
        ));
    }
    Ok(())
}

/// Build the `deesser` filter string from the controls. Values are assumed
/// already range-validated.
///
/// `amount` and `band` map straight onto ffmpeg's `i` and `f` (percent → 0..1).
/// `max_reduction` is INVERTED onto ffmpeg's `m`: internally `m` scales a
/// ceiling of `1/(3m)` on the ducking, so a SMALLER `m` permits a DEEPER cut.
/// Users expect "max reduction 100" to mean "cut as deep as needed", so
/// `m = 1 − max_reduction/100`.
pub fn build_filter(amount: f64, band: f64, max_reduction: f64, mode: Mode) -> String {
    format!(
        "deesser=i={}:m={}:f={}:s={}",
        fmt_num(amount / 100.0),
        fmt_num(1.0 - max_reduction / 100.0),
        fmt_num(band / 100.0),
        mode.as_str()
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to de-ess `in_name` into
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

/// Validate the three controls + `mode` + `format`, then return
/// `(argv, out_name)`. Single source shared by the chat block (`src/lib.rs`) and
/// the web page (`web/src/lib.rs`).
pub fn plan_deess(
    in_name: &str,
    amount: f64,
    band: f64,
    max_reduction: f64,
    mode: &str,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    validate_range("amount", amount, AMOUNT_MIN, AMOUNT_MAX)?;
    validate_range("band", band, BAND_MIN, BAND_MAX)?;
    validate_range(
        "max_reduction",
        max_reduction,
        MAX_REDUCTION_MIN,
        MAX_REDUCTION_MAX,
    )?;
    let m = parse_mode(mode)?;
    let fmt = parse_format(format)?;
    let filter = build_filter(amount, band, max_reduction, m);
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &filter, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_argv_order_and_values() {
        let (argv, out) = plan_deess("in.mp3", 60.0, 70.0, 50.0, "output", "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "deesser=i=0.6:m=0.5:f=0.7:s=o",
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
    fn defaults_are_the_documented_ones() {
        let (argv, _) = plan_deess(
            "in.wav",
            DEFAULT_AMOUNT,
            DEFAULT_BAND,
            DEFAULT_MAX_REDUCTION,
            "",
            "",
        )
        .unwrap();
        assert!(argv.iter().any(|a| a == "deesser=i=0.6:m=0.5:f=0.7:s=o"));
    }

    #[test]
    fn percent_controls_map_onto_the_filters_zero_to_one_range() {
        // amount/band map straight through; max_reduction is inverted because a
        // SMALLER ffmpeg `m` permits a DEEPER cut.
        assert_eq!(
            build_filter(100.0, 100.0, 100.0, Mode::Output),
            "deesser=i=1:m=0:f=1:s=o"
        );
        assert_eq!(
            build_filter(1.0, 1.0, 1.0, Mode::Output),
            "deesser=i=0.01:m=0.99:f=0.01:s=o"
        );
        assert_eq!(
            build_filter(33.0, 45.0, 33.0, Mode::Output),
            "deesser=i=0.33:m=0.67:f=0.45:s=o"
        );
    }

    #[test]
    fn mode_defaults_and_parses_to_ffmpeg_letters() {
        assert_eq!(parse_mode("").unwrap(), Mode::Output);
        assert_eq!(parse_mode("Output").unwrap(), Mode::Output);
        assert_eq!(parse_mode("ESS").unwrap(), Mode::Ess);
        assert_eq!(parse_mode("input").unwrap(), Mode::Input);
        assert!(parse_mode("solo").is_err());
        assert_eq!(Mode::Output.as_str(), "o");
        assert_eq!(Mode::Ess.as_str(), "e");
        assert_eq!(Mode::Input.as_str(), "i");
    }

    #[test]
    fn ess_mode_renders_the_removed_sibilance() {
        let (argv, _) = plan_deess("in.wav", 60.0, 70.0, 50.0, "ess", "wav").unwrap();
        assert!(
            argv.iter().any(|a| a.ends_with(":s=e")),
            "ess mode must set s=e: {argv:?}"
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
            let (argv, out) = plan_deess("in.mp3", 60.0, 70.0, 50.0, "output", f).unwrap();
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
        assert_eq!(Format::M4a.mime(), "audio/mp4");
        assert_eq!(Format::Flac.mime(), "audio/flac");
    }

    #[test]
    fn boundaries_are_accepted() {
        assert!(plan_deess("a.mp3", 1.0, 1.0, 1.0, "output", "mp3").is_ok());
        assert!(plan_deess("a.mp3", 100.0, 100.0, 100.0, "input", "flac").is_ok());
    }

    #[test]
    fn out_of_range_controls_are_rejected_and_named() {
        // amount 0 is ffmpeg's total bypass and sits below the minimum.
        let err = plan_deess("a.mp3", 0.0, 70.0, 50.0, "output", "mp3").unwrap_err();
        assert!(err.contains("amount"), "{err}");
        assert!(err.contains("between 1 and 100"), "{err}");
        assert!(plan_deess("a.mp3", 101.0, 70.0, 50.0, "output", "mp3").is_err());
        // band
        let err = plan_deess("a.mp3", 60.0, 0.0, 50.0, "output", "mp3").unwrap_err();
        assert!(err.contains("band"), "{err}");
        assert!(plan_deess("a.mp3", 60.0, 100.5, 50.0, "output", "mp3").is_err());
        // max_reduction 0 would null the effect (ffmpeg m = 1.0).
        let err = plan_deess("a.mp3", 60.0, 70.0, 0.0, "output", "mp3").unwrap_err();
        assert!(err.contains("max_reduction"), "{err}");
        assert!(plan_deess("a.mp3", 60.0, 70.0, 120.0, "output", "mp3").is_err());
        // bad mode / format
        assert!(plan_deess("a.mp3", 60.0, 70.0, 50.0, "listen", "mp3").is_err());
        assert!(plan_deess("a.mp3", 60.0, 70.0, 50.0, "output", "aiff").is_err());
        // non-finite
        assert!(plan_deess("a.mp3", f64::NAN, 70.0, 50.0, "output", "mp3").is_err());
        assert!(plan_deess("a.mp3", 60.0, f64::INFINITY, 50.0, "output", "mp3").is_err());
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_deess("in.mp3", 60.0, 70.0, 50.0, "output", "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(1.0), "1");
        assert_eq!(fmt_num(0.0), "0");
        assert_eq!(fmt_num(0.7), "0.7");
        assert_eq!(fmt_num(0.01), "0.01");
    }
}
