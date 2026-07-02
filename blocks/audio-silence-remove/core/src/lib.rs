//! gizza-ai/audio-silence-remove core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Strips leading, middle and trailing silent gaps with ffmpeg's single-pass
//! `silenceremove` filter (audio-only, so unlike video-silence-cut no two-pass
//! detect step is needed). A fixed 0.25 s of each removed gap is kept
//! (`*_silence=0.25`) so speech keeps natural micro-pauses instead of jarring
//! hard cuts. `-vn` drops attached-picture streams (album art).

/// Output audio formats audio-silence-remove can write (family-standard set).
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

/// Defaults shared with video-silence-cut: audio quieter than -30 dB counts as
/// silence, and a gap must last at least 0.5 s to be trimmed.
pub const DEFAULT_THRESHOLD_DB: f64 = -30.0;
pub const DEFAULT_MIN_SILENCE_S: f64 = 0.5;

/// Seconds of each removed gap that are kept, so cuts sound natural.
pub const KEEP_SILENCE_S: f64 = 0.25;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`-30` not
/// `-30.0`, `0.5` stays `0.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the `silenceremove` filter: trim leading silence (`start_periods=1`)
/// and every later gap (`stop_periods=-1`) longer than `min_silence_s`,
/// keeping [`KEEP_SILENCE_S`] of each.
///
/// `min_silence_s` maps ONLY to `stop_duration` (the gap length that triggers
/// trimming). It must NOT be passed as `start_duration` — that option is the
/// length of NON-silence required before audio counts as "started", so any
/// value above the first burst's length makes silenceremove swallow the whole
/// file (a 0.11 s beep with `start_duration=0.5` produced empty output).
pub fn build_filter(threshold_db: f64, min_silence_s: f64) -> String {
    let t = fmt_num(threshold_db);
    let d = fmt_num(min_silence_s);
    let k = fmt_num(KEEP_SILENCE_S);
    format!(
        "silenceremove=start_periods=1:start_threshold={t}dB:start_silence={k}:stop_periods=-1:stop_duration={d}:stop_threshold={t}dB:stop_silence={k}"
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to de-silence `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    threshold_db: f64,
    min_silence_s: f64,
    format: Format,
) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        build_filter(threshold_db, min_silence_s),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate params (same rules as video-silence-cut), parse `format`, and
/// return `(argv, out_name)`. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_silence_remove(
    in_name: &str,
    threshold_db: f64,
    min_silence_s: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    if !threshold_db.is_finite() || threshold_db > 0.0 {
        return Err(format!(
            "threshold_db must be <= 0 dB and finite (silence is below 0 dB; e.g. -30), got {threshold_db}"
        ));
    }
    if !min_silence_s.is_finite() || min_silence_s <= 0.0 {
        return Err(format!(
            "min_silence must be > 0 seconds and finite, got {min_silence_s}"
        ));
    }
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((
        build_argv(in_name, &out_name, threshold_db, min_silence_s, fmt),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_order_and_default_filter() {
        let (argv, out) = plan_silence_remove("in.mp3", -30.0, 0.5, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "silenceremove=start_periods=1:start_threshold=-30dB:start_silence=0.25:stop_periods=-1:stop_duration=0.5:stop_threshold=-30dB:stop_silence=0.25",
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
    fn fmt_num_drops_trailing_zero_but_keeps_fractions() {
        assert_eq!(fmt_num(-30.0), "-30");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(-40.5), "-40.5");
        assert_eq!(fmt_num(1.25), "1.25");
    }

    #[test]
    fn custom_threshold_and_gap_render_in_filter() {
        let f = build_filter(-45.5, 1.0);
        assert!(f.contains("start_threshold=-45.5dB"));
        assert!(f.contains("stop_threshold=-45.5dB"));
        assert!(f.contains("stop_duration=1"));
        // start_duration must NOT appear: it means "non-silence needed before
        // audio counts as started" and swallows short bursts (see build_filter).
        assert!(!f.contains("start_duration"));
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
            let (argv, _) = plan_silence_remove("in.mp3", -30.0, 0.5, f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_silence_remove("in.mp3", -30.0, 0.5, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn rejects_positive_threshold_and_bad_gap() {
        assert!(plan_silence_remove("a.mp3", 5.0, 0.5, "mp3").is_err());
        assert!(plan_silence_remove("a.mp3", f64::NAN, 0.5, "mp3").is_err());
        assert!(plan_silence_remove("a.mp3", -30.0, 0.0, "mp3").is_err());
        assert!(plan_silence_remove("a.mp3", -30.0, -1.0, "mp3").is_err());
        let err = plan_silence_remove("a.mp3", 5.0, 0.5, "mp3").unwrap_err();
        assert!(err.contains("below 0 dB"));
    }

    #[test]
    fn zero_threshold_is_accepted() {
        assert!(plan_silence_remove("a.mp3", 0.0, 0.5, "mp3").is_ok());
    }

    #[test]
    fn rejects_unknown_format() {
        assert!(plan_silence_remove("a.mp3", -30.0, 0.5, "aiff").is_err());
    }
}
