//! gizza-ai/audio-fit-to-length core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Fits an audio file to an EXACT target duration: if the clip is shorter than
//! the target it is padded with silence; if longer it is trimmed. Both branches
//! come from one filter chain — `apad=whole_dur=T` extends the stream to
//! `max(inputDur, T)`, and `-t T` then guarantees exactly T seconds. Silence can
//! be added at the END (default) or the START; the START case reverses the stream
//! so the appended silence lands at the head (`areverse,apad,areverse`), with
//! `whole_dur` keeping the stream finite so the second reverse terminates. When
//! the clip is longer than the target it is trimmed from the end regardless of pad
//! position (position only governs where silence is added). The audio is
//! re-encoded, so silence joins the decoded PCM cleanly; `-vn` drops album art.

/// Output audio formats audio-fit-to-length can write (family-standard set).
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

/// Where silence is added when the clip is shorter than the target.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PadPosition {
    /// Append silence after the clip (the clip starts at 0). Default.
    End,
    /// Prepend silence before the clip (the clip ends at the target).
    Start,
}

/// Parse the pad-position string. Empty defaults to end.
pub fn parse_pad(s: &str) -> Result<PadPosition, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "end" => Ok(PadPosition::End),
        "start" => Ok(PadPosition::Start),
        other => Err(format!("pad {other:?} not supported (end|start)")),
    }
}

/// Default target duration (seconds) when none is supplied.
pub const DEFAULT_DURATION_S: f64 = 30.0;
/// Longest producible output, in seconds (1 hour). The 10 MiB output cap is the
/// real practical limit (an hour of 192 kbps mp3 ≈ 86 MB).
pub const MAX_DURATION_S: f64 = 3600.0;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`30` not `30.0`,
/// `2.5` stays `2.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`). `duration` is the exact target
/// output length in seconds; `pad` picks where silence is added when the input
/// is shorter than the target.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    duration: f64,
    pad: PadPosition,
    format: Format,
) -> Vec<String> {
    let t = fmt_num(duration);
    // apad=whole_dur=T extends the stream to max(inputDur, T); -t T then trims to
    // exactly T. For START padding, reverse so the appended silence lands at the
    // head (whole_dur keeps the reversed stream finite for the second areverse).
    let filter = match pad {
        PadPosition::End => format!("apad=whole_dur={t}"),
        PadPosition::Start => format!("areverse,apad=whole_dur={t},areverse"),
    };
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        filter,
        "-t".to_string(),
        t,
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Plan a fit-to-length. `duration` must be in (0, MAX_DURATION_S]. Single source
/// shared by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_fit(
    in_name: &str,
    duration: f64,
    pad: &str,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let fmt = parse_format(format)?;
    let pad = parse_pad(pad)?;
    if !duration.is_finite() || duration <= 0.0 || duration > MAX_DURATION_S {
        return Err(format!(
            "duration must be between 0 (exclusive) and {MAX_DURATION_S} seconds, got {duration}"
        ));
    }
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, duration, pad, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg_after<'a>(argv: &'a [String], key: &str) -> Option<&'a str> {
        argv.iter()
            .position(|a| a == key)
            .and_then(|i| argv.get(i + 1))
            .map(|s| s.as_str())
    }

    #[test]
    fn end_pad_argv_order_and_values() {
        let (argv, out) = plan_fit("in.mp3", 30.0, "end", "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "apad=whole_dur=30",
                "-t",
                "30",
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
    fn start_pad_reverses_around_apad() {
        let (argv, _) = plan_fit("in.wav", 12.5, "start", "wav").unwrap();
        assert_eq!(arg_after(&argv, "-af"), Some("areverse,apad=whole_dur=12.5,areverse"));
        assert_eq!(arg_after(&argv, "-t"), Some("12.5"));
    }

    #[test]
    fn t_always_equals_target_and_matches_whole_dur() {
        let (argv, _) = plan_fit("in.mp3", 7.0, "end", "mp3").unwrap();
        let t = arg_after(&argv, "-t").unwrap();
        assert_eq!(t, "7");
        let af = arg_after(&argv, "-af").unwrap();
        assert!(af.contains("whole_dur=7"), "filter must pad to the same T: {af}");
    }

    #[test]
    fn zero_negative_and_out_of_range_durations_are_rejected() {
        assert!(plan_fit("a.mp3", 0.0, "end", "mp3").is_err());
        assert!(plan_fit("a.mp3", -5.0, "end", "mp3").is_err());
        assert!(plan_fit("a.mp3", f64::NAN, "end", "mp3").is_err());
        assert!(plan_fit("a.mp3", 3601.0, "end", "mp3").is_err());
        // Boundaries: just over 0 and exactly the max are valid.
        assert!(plan_fit("a.mp3", 0.001, "end", "mp3").is_ok());
        assert!(plan_fit("a.mp3", 3600.0, "end", "mp3").is_ok());
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
            let (argv, _) = plan_fit("in.mp3", 10.0, "end", f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_fit("in.mp3", 10.0, "end", "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn parse_helpers_default_and_reject() {
        assert_eq!(parse_format("").unwrap(), Format::Mp3);
        assert_eq!(parse_format("OGG").unwrap(), Format::Ogg);
        assert!(parse_format("aiff").is_err());
        assert_eq!(parse_pad("").unwrap(), PadPosition::End);
        assert_eq!(parse_pad("START").unwrap(), PadPosition::Start);
        assert!(parse_pad("middle").is_err());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(30.0), "30");
        assert_eq!(fmt_num(12.5), "12.5");
    }
}
