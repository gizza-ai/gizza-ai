//! gizza-ai/audio-loop core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Repeats a short sound either to a target `duration` (loop infinitely via
//! `-stream_loop -1`, trimmed with `-t`) or a total number of plays (`count`
//! total → `count - 1` extra loops). Unlike loop-video's `-c copy`, the audio
//! family re-encodes, so the loop joins happen at the decoded-PCM level —
//! seamless as long as the clip itself starts/ends cleanly. `-stream_loop` is
//! an INPUT option and must precede `-i`. `-vn` drops attached-picture
//! streams (album art).

/// Output audio formats audio-loop can write (family-standard set).
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

/// Default target duration (seconds) when neither param is supplied.
pub const DEFAULT_DURATION_S: f64 = 30.0;
/// Longest producible output, in seconds (an hour of 192 kbps mp3 ≈ 86 MB —
/// far past the 10 MiB output cap, which stays the real limit).
pub const MAX_DURATION_S: f64 = 3600.0;
/// Most total plays in count mode.
pub const MAX_COUNT: u32 = 100;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`30` not
/// `30.0`, `2.5` stays `2.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`). `stream_loop` is ffmpeg's
/// `-stream_loop` value (number of EXTRA plays; `-1` = infinite) and must
/// precede `-i`; `duration` (when set) trims the output with `-t`.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    stream_loop: i64,
    duration: Option<f64>,
    format: Format,
) -> Vec<String> {
    let mut argv = vec![
        "-stream_loop".to_string(),
        stream_loop.to_string(),
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
    ];
    if let Some(d) = duration {
        argv.push("-t".to_string());
        argv.push(fmt_num(d));
    }
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Plan a loop. `duration > 0` takes precedence (repeat infinitely, cut to
/// exactly `duration` seconds); with `duration` 0, `count` is the total
/// number of plays (2-100). Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_loop(
    in_name: &str,
    duration: f64,
    count: u32,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    if duration > 0.0 {
        if !duration.is_finite() || duration > MAX_DURATION_S {
            return Err(format!(
                "duration must be between 0 and {MAX_DURATION_S} seconds, got {duration}"
            ));
        }
        Ok((
            build_argv(in_name, &out_name, -1, Some(duration), fmt),
            out_name,
        ))
    } else if duration < 0.0 || !duration.is_finite() {
        Err(format!(
            "duration must be between 0 and {MAX_DURATION_S} seconds, got {duration}"
        ))
    } else {
        if !(2..=MAX_COUNT).contains(&count) {
            return Err(format!(
                "nothing to loop — set duration (seconds of output) or count \
                 (2-{MAX_COUNT} total plays); got duration 0 and count {count}"
            ));
        }
        // count = total plays → (count - 1) extra loops.
        Ok((
            build_argv(in_name, &out_name, i64::from(count) - 1, None, fmt),
            out_name,
        ))
    }
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
    fn duration_mode_argv_order_and_values() {
        let (argv, out) = plan_loop("in.mp3", 30.0, 0, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-stream_loop",
                "-1",
                "-i",
                "in.mp3",
                "-vn",
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
    fn stream_loop_is_an_input_option_before_i() {
        let (argv, _) = plan_loop("in.wav", 10.0, 0, "wav").unwrap();
        let sl = argv.iter().position(|a| a == "-stream_loop").unwrap();
        let i = argv.iter().position(|a| a == "-i").unwrap();
        assert!(sl < i, "-stream_loop must precede -i");
    }

    #[test]
    fn count_mode_uses_extra_plays_and_no_trim() {
        let (argv, _) = plan_loop("in.mp3", 0.0, 3, "mp3").unwrap();
        assert_eq!(arg_after(&argv, "-stream_loop"), Some("2")); // 3 plays = 2 extra
        assert!(!argv.iter().any(|a| a == "-t"));
    }

    #[test]
    fn duration_takes_precedence_over_count() {
        let (argv, _) = plan_loop("in.mp3", 12.5, 3, "mp3").unwrap();
        assert_eq!(arg_after(&argv, "-stream_loop"), Some("-1"));
        assert_eq!(arg_after(&argv, "-t"), Some("12.5"));
    }

    #[test]
    fn zero_duration_and_low_count_is_a_guiding_error() {
        for c in [0, 1] {
            let err = plan_loop("in.mp3", 0.0, c, "mp3").unwrap_err();
            assert!(err.contains("nothing to loop"), "count {c}: {err}");
        }
    }

    #[test]
    fn out_of_range_values_are_rejected() {
        assert!(plan_loop("a.mp3", 3601.0, 0, "mp3").is_err());
        assert!(plan_loop("a.mp3", -5.0, 0, "mp3").is_err());
        assert!(plan_loop("a.mp3", f64::NAN, 0, "mp3").is_err());
        assert!(plan_loop("a.mp3", 0.0, 101, "mp3").is_err());
        // Boundaries are valid.
        assert!(plan_loop("a.mp3", 3600.0, 0, "mp3").is_ok());
        assert!(plan_loop("a.mp3", 0.0, 100, "mp3").is_ok());
        assert!(plan_loop("a.mp3", 0.0, 2, "mp3").is_ok());
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
            let (argv, _) = plan_loop("in.mp3", 10.0, 0, f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_loop("in.mp3", 10.0, 0, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn parse_format_defaults_empty_to_mp3() {
        assert_eq!(parse_format("").unwrap(), Format::Mp3);
        assert_eq!(parse_format("OGG").unwrap(), Format::Ogg);
        assert!(parse_format("aiff").is_err());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(30.0), "30");
        assert_eq!(fmt_num(12.5), "12.5");
    }
}
