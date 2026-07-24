//! gizza-ai/audio-bleep-censor core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Censors one or more time regions of an audio file. Each region is either
//! replaced with a bleep tone (`bleep`), silenced (`mute`), or dropped to a low
//! "duck" level (`duck`). Muting/ducking is a single `volume` filter gated by an
//! `enable='between(t,s,e)+…'` timeline expression. Bleeping mutes the voice in
//! the regions and mixes a gated sine over just those regions with `amix`, so
//! audio OUTSIDE the regions is untouched. The whole graph is built BEFORE the
//! file is decoded, so it never needs the input duration.

/// Output audio formats this tool can write (audio-family-standard set).
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
            Format::Mp3 => vec!["-c:a".into(), "libmp3lame".into(), "-b:a".into(), "192k".into()],
            Format::Wav => vec!["-c:a".into(), "pcm_s16le".into()],
            Format::Ogg => vec!["-c:a".into(), "libvorbis".into(), "-b:a".into(), "192k".into()],
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
        other => Err(format!("format {other:?} not supported (mp3|wav|ogg|flac|m4a)")),
    }
}

/// How a censored region is replaced.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    /// Mix a sine "bleep" tone over the region (voice muted underneath).
    Bleep,
    /// Silence the region completely.
    Mute,
    /// Lower the region to a quiet level so a trace of speech remains.
    Duck,
}

impl Mode {
    /// Parse the descriptor/CLI/page string value. Empty defaults to bleep.
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "bleep" | "beep" => Ok(Mode::Bleep),
            "mute" | "silence" => Ok(Mode::Mute),
            "duck" | "lower" => Ok(Mode::Duck),
            other => Err(format!("mode {other:?} not supported (bleep|mute|duck)")),
        }
    }
}

/// Volume the region is dropped to in `duck` mode (~-20 dB — audible trace).
const DUCK_LEVEL: &str = "0.1";
/// Linear amplitude of the bleep tone before it is gated into the regions.
const BLEEP_AMP: &str = "0.5";
/// Sample rate the bleep graph normalizes both branches to before mixing.
const BLEEP_RATE: u32 = 44_100;

/// Default bleep tone frequency (classic ~1 kHz TV bleep).
pub const DEFAULT_TONE_HZ: f64 = 1000.0;
/// Accepted bleep tone frequency range, in Hz.
pub const MIN_TONE_HZ: f64 = 100.0;
pub const MAX_TONE_HZ: f64 = 8000.0;
/// Largest number of regions accepted in one run.
pub const MAX_REGIONS: usize = 50;

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

/// Parse one timestamp: bare seconds (`1.5`), `mm:ss(.ms)`, or `hh:mm:ss(.ms)`.
/// Returns the time in seconds. Empty/negative/non-finite is rejected.
pub fn parse_time(s: &str) -> Result<f64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty timestamp".into());
    }
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() > 3 {
        return Err(format!("timestamp {s:?} has too many ':' parts (use s, mm:ss, or hh:mm:ss)"));
    }
    let mut total = 0.0f64;
    for (i, p) in parts.iter().enumerate() {
        let v: f64 = p
            .trim()
            .parse()
            .map_err(|_| format!("timestamp {s:?} has a non-numeric part {p:?}"))?;
        if !v.is_finite() || v < 0.0 {
            return Err(format!("timestamp {s:?} must be non-negative"));
        }
        // For mm:ss / hh:mm:ss the non-leading parts are 0-59.999… units.
        if parts.len() > 1 && i > 0 && v >= 60.0 {
            return Err(format!("timestamp {s:?}: minutes/seconds part {p:?} must be < 60"));
        }
        total = total * 60.0 + v;
    }
    Ok(total)
}

/// Parse a `start-end, start-end, …` region list into `(start, end)` seconds.
/// Times may be bare seconds or `mm:ss`/`hh:mm:ss`. Enforces `0 ≤ start < end`,
/// at least one region, and at most [`MAX_REGIONS`].
pub fn parse_regions(s: &str) -> Result<Vec<(f64, f64)>, String> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (a, b) = part
            .split_once('-')
            .ok_or_else(|| format!("region {part:?} must be written start-end (e.g. 1.5-2.0)"))?;
        let start = parse_time(a)?;
        let end = parse_time(b)?;
        if !(end > start) {
            return Err(format!(
                "region {part:?}: end ({}) must be greater than start ({})",
                fmt_num(end),
                fmt_num(start)
            ));
        }
        out.push((start, end));
    }
    if out.is_empty() {
        return Err(
            "provide at least one region as start-end seconds, e.g. \"1.5-2.0\" or \"0:07-0:08.5\""
                .into(),
        );
    }
    if out.len() > MAX_REGIONS {
        return Err(format!("too many regions ({}); at most {MAX_REGIONS} per run", out.len()));
    }
    Ok(out)
}

/// The `between(t,s,e)+…` timeline expression that is true inside every region.
fn regions_expr(regions: &[(f64, f64)]) -> String {
    regions
        .iter()
        .map(|(s, e)| format!("between(t,{},{})", fmt_num(*s), fmt_num(*e)))
        .collect::<Vec<_>>()
        .join("+")
}

fn validate_tone(hz: f64) -> Result<(), String> {
    if !hz.is_finite() || !(MIN_TONE_HZ..=MAX_TONE_HZ).contains(&hz) {
        return Err(format!(
            "tone_hz must be between {MIN_TONE_HZ} and {MAX_TONE_HZ} Hz, got {}",
            fmt_num(hz)
        ));
    }
    Ok(())
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that censors `regions` of
/// `in_name` into `out_name` using `mode`/`tone_hz`, encoded as `format`.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    regions: &[(f64, f64)],
    mode: Mode,
    tone_hz: f64,
    format: Format,
) -> Vec<String> {
    let expr = regions_expr(regions);
    let mut argv = vec!["-i".to_string(), in_name.to_string()];
    match mode {
        Mode::Bleep => {
            let hz = fmt_num(tone_hz);
            let graph = format!(
                "[0:a]aformat=sample_rates={rate}:channel_layouts=stereo,volume=0:enable='{expr}'[v];\
                 sine=frequency={hz}:sample_rate={rate},aformat=channel_layouts=stereo,volume={amp},volume=0:enable='not({expr})'[b];\
                 [v][b]amix=inputs=2:duration=first:normalize=0[out]",
                rate = BLEEP_RATE,
                amp = BLEEP_AMP,
            );
            argv.push("-filter_complex".to_string());
            argv.push(graph);
            argv.push("-map".to_string());
            argv.push("[out]".to_string());
        }
        Mode::Mute | Mode::Duck => {
            let level = if mode == Mode::Mute { "0" } else { DUCK_LEVEL };
            argv.push("-vn".to_string());
            argv.push("-af".to_string());
            argv.push(format!("volume={level}:enable='{expr}'"));
        }
    }
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Parse + validate every field and return `(argv, out_name)`. Single source
/// shared by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    regions: &str,
    mode: &str,
    tone_hz: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let regions = parse_regions(regions)?;
    let mode = Mode::parse(mode)?;
    if mode == Mode::Bleep {
        validate_tone(tone_hz)?;
    }
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &regions, mode, tone_hz, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bleep_mixes_a_gated_tone_over_muted_regions() {
        let (argv, out) = plan("in.mp3", "1.5-2, 5-6", "bleep", 1000.0, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        let fc = argv.iter().position(|a| a == "-filter_complex").unwrap();
        assert_eq!(
            argv[fc + 1],
            "[0:a]aformat=sample_rates=44100:channel_layouts=stereo,volume=0:enable='between(t,1.5,2)+between(t,5,6)'[v];\
             sine=frequency=1000:sample_rate=44100,aformat=channel_layouts=stereo,volume=0.5,volume=0:enable='not(between(t,1.5,2)+between(t,5,6))'[b];\
             [v][b]amix=inputs=2:duration=first:normalize=0[out]"
        );
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "[out]"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libmp3lame"));
    }

    #[test]
    fn mute_is_a_single_volume0_filter() {
        let (argv, _) = plan("in.wav", "0:07-0:08.5", "mute", 1000.0, "wav").unwrap();
        let af = argv.iter().position(|a| a == "-af").unwrap();
        // 0:07 = 7 s, 0:08.5 = 8.5 s.
        assert_eq!(argv[af + 1], "volume=0:enable='between(t,7,8.5)'");
        assert!(argv.iter().any(|a| a == "-vn"), "album art dropped");
    }

    #[test]
    fn duck_lowers_instead_of_silencing() {
        let (argv, _) = plan("in.mp3", "1-2", "duck", 1000.0, "mp3").unwrap();
        let af = argv.iter().position(|a| a == "-af").unwrap();
        assert_eq!(argv[af + 1], "volume=0.1:enable='between(t,1,2)'");
    }

    #[test]
    fn tone_hz_flows_into_the_sine_source() {
        let (argv, _) = plan("in.mp3", "1-2", "bleep", 440.0, "mp3").unwrap();
        let fc = argv.iter().position(|a| a == "-filter_complex").unwrap();
        assert!(argv[fc + 1].contains("sine=frequency=440:"), "{}", argv[fc + 1]);
    }

    #[test]
    fn empty_regions_is_rejected() {
        let err = plan("in.mp3", "  ,  ", "bleep", 1000.0, "mp3").unwrap_err();
        assert!(err.contains("at least one region"), "{err}");
    }

    #[test]
    fn end_before_start_is_rejected() {
        let err = plan("in.mp3", "5-2", "mute", 1000.0, "mp3").unwrap_err();
        assert!(err.contains("must be greater than start"), "{err}");
    }

    #[test]
    fn malformed_region_is_rejected() {
        assert!(plan("in.mp3", "1.5", "mute", 1000.0, "mp3").is_err());
        assert!(plan("in.mp3", "1:2:3:4-5", "mute", 1000.0, "mp3").is_err());
    }

    #[test]
    fn tone_hz_out_of_range_is_rejected_only_for_bleep() {
        assert!(plan("in.mp3", "1-2", "bleep", 50.0, "mp3").is_err());
        assert!(plan("in.mp3", "1-2", "bleep", 9000.0, "mp3").is_err());
        // Out-of-range tone is ignored when not bleeping.
        assert!(plan("in.mp3", "1-2", "mute", 9000.0, "mp3").is_ok());
        // Boundaries are valid.
        assert!(plan("in.mp3", "1-2", "bleep", 100.0, "mp3").is_ok());
        assert!(plan("in.mp3", "1-2", "bleep", 8000.0, "mp3").is_ok());
    }

    #[test]
    fn too_many_regions_is_rejected() {
        let many = (0..MAX_REGIONS + 1)
            .map(|i| format!("{}-{}", i * 2, i * 2 + 1))
            .collect::<Vec<_>>()
            .join(",");
        assert!(plan("in.mp3", &many, "mute", 1000.0, "mp3").unwrap_err().contains("too many"));
    }

    #[test]
    fn parse_time_handles_all_forms() {
        assert_eq!(parse_time("1.5").unwrap(), 1.5);
        assert_eq!(parse_time("0:07").unwrap(), 7.0);
        assert_eq!(parse_time("1:30").unwrap(), 90.0);
        assert_eq!(parse_time("1:02:03.5").unwrap(), 3723.5);
        assert!(parse_time("").is_err());
        assert!(parse_time("1:75").is_err()); // seconds >= 60
        assert!(parse_time("-3").is_err());
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
            let (argv, _) = plan("in.mp3", "1-2", "mute", 1000.0, f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn mode_and_format_parse_aliases_and_reject_junk() {
        assert_eq!(Mode::parse("").unwrap(), Mode::Bleep);
        assert_eq!(Mode::parse("Silence").unwrap(), Mode::Mute);
        assert_eq!(Mode::parse("lower").unwrap(), Mode::Duck);
        assert!(Mode::parse("scramble").is_err());
        assert_eq!(parse_format("M4A").unwrap(), Format::M4a);
        assert!(parse_format("aiff").is_err());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(3.0), "3");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(8.25), "8.25");
    }
}
