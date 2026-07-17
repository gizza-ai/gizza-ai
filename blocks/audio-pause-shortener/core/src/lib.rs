//! gizza-ai/audio-pause-shortener core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! # What this does (and how it differs from audio-silence-remove)
//!
//! This *shortens* over-long pauses instead of stripping silence. Every silent
//! gap longer than `max_pause` seconds is collapsed down to exactly
//! `target_pause` seconds — a natural pacing edit (Audacity calls it "Truncate
//! Silence", Descript "shorten word gaps"). Pauses shorter than `max_pause` are
//! left completely untouched, so the speech keeps its natural rhythm; only the
//! dragging gaps are tightened. Sibling `audio-silence-remove` instead removes
//! ALL gaps down to a fixed 0.25 s beat (including trimming leading silence).
//!
//! It uses ffmpeg's single-pass `silenceremove` filter with only the STOP side:
//! `stop_duration` = the gap length that triggers a trim (`max_pause`),
//! `stop_silence` = the amount kept afterward (`target_pause`). Leading silence
//! is deliberately NOT trimmed — this tool caps long pauses, it does not strip
//! dead air. `-vn` drops attached-picture streams (album art).

/// Output audio formats audio-pause-shortener can write (family-standard set).
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

/// Defaults: audio quieter than -30 dB counts as silence; pauses longer than
/// 1.5 s are shortened, and each is collapsed to a 0.5 s beat.
pub const DEFAULT_THRESHOLD_DB: f64 = -30.0;
pub const DEFAULT_MAX_PAUSE_S: f64 = 1.5;
pub const DEFAULT_TARGET_PAUSE_S: f64 = 0.5;

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

/// Build the `silenceremove` filter that shortens (does not strip) pauses.
///
/// Only the STOP side is used (`stop_periods=-1`), so leading silence is left
/// intact and every internal/trailing gap at least `max_pause` seconds long is
/// collapsed to `target_pause` seconds (`stop_silence` = amount kept). Gaps
/// shorter than `max_pause` never reach `stop_duration` and pass through
/// untouched. `start_periods`/`start_duration` are intentionally absent — the
/// start side would trim leading silence (that is audio-silence-remove's job)
/// and `start_duration` can swallow short opening bursts.
pub fn build_filter(threshold_db: f64, max_pause_s: f64, target_pause_s: f64) -> String {
    let t = fmt_num(threshold_db);
    let d = fmt_num(max_pause_s);
    let k = fmt_num(target_pause_s);
    format!(
        "silenceremove=stop_periods=-1:stop_duration={d}:stop_threshold={t}dB:stop_silence={k}"
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to shorten pauses in `in_name`
/// into `out_name`. Shared verbatim by the web page (`build_argv`) and the chat
/// block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    threshold_db: f64,
    max_pause_s: f64,
    target_pause_s: f64,
    format: Format,
) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        build_filter(threshold_db, max_pause_s, target_pause_s),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate params, parse `format`, and return `(argv, out_name)`. Single
/// source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
///
/// `target_pause` must be strictly less than `max_pause`: keeping at least as
/// much silence as the trigger length would be a no-op (nothing to shorten),
/// so it is rejected with a clear message rather than silently doing nothing.
pub fn plan_pause_shorten(
    in_name: &str,
    threshold_db: f64,
    max_pause_s: f64,
    target_pause_s: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    if !threshold_db.is_finite() || threshold_db > 0.0 {
        return Err(format!(
            "threshold_db must be <= 0 dB and finite (silence is below 0 dB; e.g. -30), got {threshold_db}"
        ));
    }
    if !max_pause_s.is_finite() || max_pause_s <= 0.0 {
        return Err(format!(
            "max_pause must be > 0 seconds and finite, got {max_pause_s}"
        ));
    }
    if !target_pause_s.is_finite() || target_pause_s < 0.0 {
        return Err(format!(
            "target_pause must be >= 0 seconds and finite, got {target_pause_s}"
        ));
    }
    if target_pause_s >= max_pause_s {
        return Err(format!(
            "target_pause ({target_pause_s}) must be less than max_pause ({max_pause_s}) — otherwise nothing would be shortened"
        ));
    }
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((
        build_argv(in_name, &out_name, threshold_db, max_pause_s, target_pause_s, fmt),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_order_and_default_filter() {
        let (argv, out) = plan_pause_shorten("in.mp3", -30.0, 1.5, 0.5, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "silenceremove=stop_periods=-1:stop_duration=1.5:stop_threshold=-30dB:stop_silence=0.5",
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
    fn custom_threshold_and_durations_render_in_filter() {
        let f = build_filter(-45.5, 2.0, 0.75);
        assert!(f.contains("stop_threshold=-45.5dB"));
        assert!(f.contains("stop_duration=2"));
        assert!(f.contains("stop_silence=0.75"));
        // Only the STOP side is used — leading silence is never trimmed, and
        // start_duration (which can swallow short opening bursts) must be absent.
        assert!(!f.contains("start_periods"));
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
            let (argv, _) = plan_pause_shorten("in.mp3", -30.0, 1.5, 0.5, f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_pause_shorten("in.mp3", -30.0, 1.5, 0.5, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn rejects_bad_params() {
        // positive threshold
        assert!(plan_pause_shorten("a.mp3", 5.0, 1.5, 0.5, "mp3").is_err());
        // non-finite threshold
        assert!(plan_pause_shorten("a.mp3", f64::NAN, 1.5, 0.5, "mp3").is_err());
        // max_pause not > 0
        assert!(plan_pause_shorten("a.mp3", -30.0, 0.0, 0.5, "mp3").is_err());
        // negative target
        assert!(plan_pause_shorten("a.mp3", -30.0, 1.5, -0.1, "mp3").is_err());
    }

    #[test]
    fn rejects_target_not_less_than_max() {
        let err = plan_pause_shorten("a.mp3", -30.0, 1.0, 1.0, "mp3").unwrap_err();
        assert!(err.contains("less than max_pause"));
        assert!(plan_pause_shorten("a.mp3", -30.0, 1.0, 2.0, "mp3").is_err());
    }

    #[test]
    fn zero_threshold_and_zero_target_are_accepted() {
        // 0 dB threshold is a valid (if extreme) edge; target 0 fully removes
        // the long pauses (still shortening, just to nothing).
        assert!(plan_pause_shorten("a.mp3", 0.0, 1.5, 0.0, "mp3").is_ok());
    }

    #[test]
    fn rejects_unknown_format() {
        assert!(plan_pause_shorten("a.mp3", -30.0, 1.5, 0.5, "aiff").is_err());
    }
}
