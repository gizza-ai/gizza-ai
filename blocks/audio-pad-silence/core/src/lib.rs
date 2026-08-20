//! gizza-ai/audio-pad-silence core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Adds a chosen length of digital silence to the START and/or the END of an
//! audio clip, leaving the clip itself untouched in the middle. Two filters do
//! the work and they compose in one chain:
//!
//! * `adelay=<ms>:all=1` shifts every channel later by N milliseconds, which is
//!   exactly "prepend N ms of silence". It wants INTEGER milliseconds, so a
//!   fractional-second lead-in is rounded to the nearest ms.
//! * `apad=pad_dur=<s>` appends a BOUNDED amount of silence after the stream
//!   ends. (Plain `apad` pads forever and needs `-t`/`whole_dur` to terminate —
//!   `pad_dur` is the finite form, so no output-duration guess is needed.)
//!
//! Output length is therefore `start + input + end` exactly. A side of `0` is
//! omitted from the chain rather than passed as a no-op filter, and at least one
//! side must be greater than zero (padding nothing is a user error, not a copy).
//! The audio is re-encoded so the silence joins the decoded PCM cleanly; `-vn`
//! drops embedded album art.

/// Output audio formats audio-pad-silence can write (family-standard set).
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

/// Default lead-in silence (seconds) when none is supplied.
pub const DEFAULT_START_S: f64 = 2.0;
/// Default trailing silence (seconds) when none is supplied.
pub const DEFAULT_END_S: f64 = 0.0;
/// Longest silence addable to ONE side, in seconds (1 hour) — matches what the
/// mainstream online padders advertise. The 10 MiB output cap is the real
/// practical limit (an hour of 192 kbps mp3 ≈ 86 MB).
pub const MAX_PAD_S: f64 = 3600.0;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`3` not `3.0`,
/// `1.5` stays `1.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Validate one side's padding length. Empty/absent is handled by the caller.
fn check_side(name: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() || v < 0.0 || v > MAX_PAD_S {
        return Err(format!(
            "{name} must be between 0 and {MAX_PAD_S} seconds, got {v}"
        ));
    }
    Ok(())
}

/// Build the `-af` filter chain for the requested padding, or `None` when both
/// sides are zero. `start`/`end` are seconds; `adelay` needs integer ms.
pub fn build_filter(start: f64, end: f64) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if start > 0.0 {
        let ms = (start * 1000.0).round() as i64;
        parts.push(format!("adelay={ms}:all=1"));
    }
    if end > 0.0 {
        parts.push(format!("apad=pad_dur={}", fmt_num(end)));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(","))
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`). Callers must have validated
/// `start`/`end` via [`plan_pad`], which is the only supported entry point.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    start: f64,
    end: f64,
    format: Format,
) -> Vec<String> {
    let filter = build_filter(start, end).unwrap_or_default();
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        filter,
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Plan a silence-pad. `start`/`end` are seconds in [0, MAX_PAD_S] and at least
/// one must be greater than zero. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_pad(
    in_name: &str,
    start: f64,
    end: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let fmt = parse_format(format)?;
    check_side("start", start)?;
    check_side("end", end)?;
    if start <= 0.0 && end <= 0.0 {
        return Err(
            "nothing to add: set start and/or end to a number of seconds greater than 0".into(),
        );
    }
    // adelay rounds to whole milliseconds; a positive request that rounds to 0
    // would silently produce a filter that does nothing.
    if start > 0.0 && (start * 1000.0).round() as i64 == 0 {
        return Err(format!(
            "start is too small to add: {start} s rounds to 0 ms (minimum 0.001 s)"
        ));
    }
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, start, end, fmt), out_name))
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
    fn both_sides_argv_order_and_values() {
        let (argv, out) = plan_pad("in.mp3", 2.0, 1.5, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "adelay=2000:all=1,apad=pad_dur=1.5",
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
    fn start_only_omits_apad_and_end_only_omits_adelay() {
        let (argv, _) = plan_pad("in.wav", 0.5, 0.0, "wav").unwrap();
        assert_eq!(arg_after(&argv, "-af"), Some("adelay=500:all=1"));

        let (argv, _) = plan_pad("in.wav", 0.0, 4.0, "wav").unwrap();
        assert_eq!(arg_after(&argv, "-af"), Some("apad=pad_dur=4"));
    }

    #[test]
    fn zero_on_both_sides_is_rejected() {
        let err = plan_pad("a.mp3", 0.0, 0.0, "mp3").unwrap_err();
        assert!(err.contains("nothing to add"), "got {err}");
    }

    #[test]
    fn negative_nan_and_over_cap_are_rejected() {
        assert!(plan_pad("a.mp3", -1.0, 0.0, "mp3").is_err());
        assert!(plan_pad("a.mp3", 0.0, -1.0, "mp3").is_err());
        assert!(plan_pad("a.mp3", f64::NAN, 1.0, "mp3").is_err());
        assert!(plan_pad("a.mp3", 0.0, f64::INFINITY, "mp3").is_err());
        assert!(plan_pad("a.mp3", 3601.0, 0.0, "mp3").is_err());
        assert!(plan_pad("a.mp3", 0.0, 3601.0, "mp3").is_err());
        // Exactly the cap is valid on either side.
        assert!(plan_pad("a.mp3", 3600.0, 0.0, "mp3").is_ok());
        assert!(plan_pad("a.mp3", 0.0, 3600.0, "mp3").is_ok());
    }

    #[test]
    fn sub_millisecond_start_is_rejected_rather_than_silently_dropped() {
        let err = plan_pad("a.mp3", 0.0004, 0.0, "mp3").unwrap_err();
        assert!(err.contains("rounds to 0 ms"), "got {err}");
        // 1 ms is the smallest usable lead-in.
        let (argv, _) = plan_pad("a.mp3", 0.001, 0.0, "mp3").unwrap();
        assert_eq!(arg_after(&argv, "-af"), Some("adelay=1:all=1"));
    }

    #[test]
    fn fractional_start_rounds_to_whole_milliseconds() {
        assert_eq!(build_filter(0.25, 0.0).unwrap(), "adelay=250:all=1");
        assert_eq!(build_filter(1.2345, 0.0).unwrap(), "adelay=1235:all=1");
        assert_eq!(build_filter(0.0, 0.0), None);
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
            let (argv, out) = plan_pad("in.mp3", 1.0, 1.0, f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
            assert!(out.ends_with(f), "out name must use the .{f} extension");
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_pad("in.mp3", 1.0, 0.0, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn parse_format_defaults_and_rejects() {
        assert_eq!(parse_format("").unwrap(), Format::Mp3);
        assert_eq!(parse_format("FLAC").unwrap(), Format::Flac);
        assert!(parse_format("aiff").is_err());
        assert!(plan_pad("a.mp3", 1.0, 0.0, "aiff").is_err());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(3.0), "3");
        assert_eq!(fmt_num(1.5), "1.5");
        assert_eq!(fmt_num(0.25), "0.25");
    }

    #[test]
    fn mime_matches_extension_family() {
        assert_eq!(Format::Mp3.mime(), "audio/mpeg");
        assert_eq!(Format::M4a.mime(), "audio/mp4");
        assert_eq!(Format::Wav.ext(), "wav");
    }
}
