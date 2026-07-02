//! gizza-ai/audio-fade core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Adds a fade-in and/or fade-out. `afade=t=in` handles the start; the end is
//! faded with the `areverse,afade=t=in,areverse` trick because the argv is
//! built BEFORE the input is decoded — the fade-out start time (duration −
//! fade) isn't knowable here, but fading-in a reversed stream needs no
//! duration at all. Inputs are capped at 10 MiB so the two full-buffer
//! reverses are cheap. `-vn` drops attached-picture streams (album art).

/// Output audio formats audio-fade can write (family-standard set).
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

/// Default fade length (seconds) for each side when the param is omitted.
pub const DEFAULT_FADE_S: f64 = 3.0;
/// Longest accepted fade per side, in seconds.
pub const MAX_FADE_S: f64 = 30.0;

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

fn validate_fade(name: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() || !(0.0..=MAX_FADE_S).contains(&v) {
        return Err(format!(
            "{name} must be between 0 and {MAX_FADE_S} seconds, got {v}"
        ));
    }
    Ok(())
}

/// Build the `-af` chain: an `afade=t=in` stage when `fade_in` > 0, and the
/// duration-free `areverse,afade=t=in,areverse` stage when `fade_out` > 0.
/// A side at 0 is skipped; both at 0 is rejected as a no-op.
pub fn build_filter(fade_in: f64, fade_out: f64) -> Result<String, String> {
    let mut stages = Vec::new();
    if fade_in > 0.0 {
        stages.push(format!("afade=t=in:st=0:d={}", fmt_num(fade_in)));
    }
    if fade_out > 0.0 {
        stages.push(format!(
            "areverse,afade=t=in:st=0:d={},areverse",
            fmt_num(fade_out)
        ));
    }
    if stages.is_empty() {
        return Err(
            "both fades are 0 — nothing to change; set fade_in and/or fade_out in seconds \
             (e.g. 3 for a gentle three-second fade)"
                .to_string(),
        );
    }
    Ok(stages.join(","))
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to fade `in_name` into
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

/// Validate both fade lengths, parse `format`, and return `(argv, out_name)`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan_fade(
    in_name: &str,
    fade_in: f64,
    fade_out: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    validate_fade("fade_in", fade_in)?;
    validate_fade("fade_out", fade_out)?;
    let filter = build_filter(fade_in, fade_out)?;
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &filter, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_fades_argv_order_and_values() {
        let (argv, out) = plan_fade("in.mp3", 3.0, 3.0, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "afade=t=in:st=0:d=3,areverse,afade=t=in:st=0:d=3,areverse",
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
    fn single_sided_fades_skip_the_other_stage() {
        assert_eq!(
            build_filter(2.0, 0.0).unwrap(),
            "afade=t=in:st=0:d=2"
        );
        assert_eq!(
            build_filter(0.0, 1.5).unwrap(),
            "areverse,afade=t=in:st=0:d=1.5,areverse"
        );
    }

    #[test]
    fn fade_out_never_needs_a_start_time() {
        // The whole point of the areverse trick: no `st=<duration - d>` term
        // may appear, because the input duration is unknown at argv-build time.
        let f = build_filter(0.0, 5.0).unwrap();
        assert!(!f.contains("t=out"), "{f}");
        assert!(f.starts_with("areverse,") && f.ends_with(",areverse"), "{f}");
    }

    #[test]
    fn both_zero_is_rejected_as_a_no_op() {
        let err = plan_fade("in.mp3", 0.0, 0.0, "mp3").unwrap_err();
        assert!(err.contains("nothing to change"), "{err}");
    }

    #[test]
    fn out_of_range_or_non_finite_fades_are_rejected() {
        assert!(plan_fade("a.mp3", -1.0, 3.0, "mp3").is_err());
        assert!(plan_fade("a.mp3", 3.0, 31.0, "mp3").is_err());
        assert!(plan_fade("a.mp3", f64::NAN, 3.0, "mp3").is_err());
        // Boundaries are valid.
        assert!(plan_fade("a.mp3", 30.0, 0.0, "mp3").is_ok());
        let err = plan_fade("a.mp3", 3.0, 31.0, "mp3").unwrap_err();
        assert!(err.contains("fade_out"), "names the offending side: {err}");
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
            let (argv, _) = plan_fade("in.mp3", 3.0, 3.0, f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_fade("in.mp3", 3.0, 0.0, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn parse_format_defaults_empty_to_mp3() {
        assert_eq!(parse_format("").unwrap(), Format::Mp3);
        assert_eq!(parse_format("M4A").unwrap(), Format::M4a);
        assert!(parse_format("aiff").is_err());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(3.0), "3");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(12.25), "12.25");
    }
}
