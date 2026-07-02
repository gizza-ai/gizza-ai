//! gizza-ai/trim-audio core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Keep mode extracts the `[start, end]` selection via `atrim` + `asetpts`
//! (sample-accurate, timeline reset to 0 so the optional fade offsets are
//! computable from `end - start`). Remove mode deletes the selection and joins
//! the rest via `aselect='not(between(t,start,end))'` + `asetpts` — frame
//! granular (~20-40 ms), which is fine for audio edits. Filters always
//! re-encode, so the output codec follows the chosen [`Format`].
//!
//! `end == 0.0` is the "to the end of the track" sentinel (the page sends 0
//! for an empty field; chat/CLI may omit the param): keep drops `atrim`'s
//! `end`, remove keeps `[0, start]` via `atrim=end=start`, and a to-end fade
//! uses the duration-free `areverse,afade,areverse` trick because the track
//! length is unknown at argv-build time. A whole-file request (start 0 with
//! no end) is rejected with a guiding error rather than run as a no-op.

/// Output audio formats trim-audio can write. All five encoders are present in
/// both the native ffmpeg runtime (chat/CLI) and the browser @ffmpeg/core
/// build: libmp3lame, pcm_s16le, libvorbis, flac and aac (the audio-convert
/// Playwright ogg test proved libvorbis is present in-browser).
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

    /// Encoder argv fragment (`-c:a …`). MP3 is fixed at 192 kbps (a good
    /// size/quality balance); WAV is lossless 16-bit PCM; FLAC is lossless
    /// compressed; M4A uses ffmpeg's built-in AAC encoder.
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

/// Parse the user-facing format string (the values the chat schema + page
/// accept). Empty defaults to mp3.
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

/// What to do with the `[start, end]` selection.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    /// Output is just the selection.
    Keep,
    /// Output is everything except the selection.
    Remove,
}

/// Parse the user-facing mode string. Empty defaults to keep.
pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "keep" => Ok(Mode::Keep),
        "remove" => Ok(Mode::Remove),
        other => Err(format!("mode {other:?} not supported (keep|remove)")),
    }
}

/// Max fade length in seconds. Shortened to a quarter of the kept selection
/// for short clips so the fade-in and fade-out never overlap.
pub const FADE_SECS: f64 = 0.5;

/// Flat fade length for to-end trims, where the selection length is unknown
/// at argv-build time so the dur/4 anti-overlap clamp can't be computed.
/// 0.15 s fully de-clicks a cut edge while shrinking the degenerate window
/// (both fades overlapping and attenuating the whole clip) to remainders
/// under ~0.3 s.
pub const TO_END_FADE_SECS: f64 = 0.15;

/// Upper bound for start/end times in seconds (24 h). ffmpeg's atrim duration
/// parser overflows around 9.2e18 µs, turning absurd-but-finite values into a
/// cryptic "Numerical result out of range" — reject them with a real message.
pub const MAX_TIME_S: f64 = 86_400.0;

/// The `end` value that means "to the end of the track" (empty page field,
/// omitted chat/CLI param). Single source of the sentinel predicate — used by
/// [`build_filter`], [`plan_trim_audio`] and the block's summary formatting.
pub fn is_to_end(end: f64) -> bool {
    end == 0.0
}

/// Build the `-af` filtergraph for a trim. The `aselect` expression is quoted
/// so its commas survive ffmpeg's filtergraph parser (argv is passed directly,
/// no shell involved). `end == 0.0` means "to the end of the track".
pub fn build_filter(mode: Mode, start: f64, end: f64, fade: bool) -> String {
    let to_end = is_to_end(end);
    match mode {
        Mode::Keep => {
            let mut f = if to_end {
                format!("atrim=start={start},asetpts=N/SR/TB")
            } else {
                format!("atrim=start={start}:end={end},asetpts=N/SR/TB")
            };
            if fade {
                if to_end {
                    // Selection length is unknown, so fades are a flat
                    // TO_END_FADE_SECS and the out-fade needs the duration-free
                    // areverse trick — audio-fade's core owns that chain.
                    f.push(',');
                    f.push_str(
                        &gizza_ai_audio_fade_core::build_filter(
                            TO_END_FADE_SECS,
                            TO_END_FADE_SECS,
                        )
                        .expect("const fade lengths are in audio-fade's accepted range"),
                    );
                } else {
                    let dur = end - start;
                    let fd = FADE_SECS.min(dur / 4.0);
                    let out_st = dur - fd;
                    f.push_str(&format!(
                        ",afade=t=in:st=0:d={fd},afade=t=out:st={out_st}:d={fd}"
                    ));
                }
            }
            f
        }
        Mode::Remove => {
            if to_end {
                // Removing [start, EOF] == keeping [0, start], sample-accurate.
                format!("atrim=end={start},asetpts=N/SR/TB")
            } else {
                format!("aselect='not(between(t,{start},{end}))',asetpts=N/SR/TB")
            }
        }
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) for a trim of `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    mode: Mode,
    start: f64,
    end: f64,
    fade: bool,
    format: Format,
) -> Vec<String> {
    // -vn drops any video stream: audio files with embedded album art carry an
    // attached-picture (video) stream that breaks audio-only muxers like wav.
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        build_filter(mode, start, end, fade),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate params, parse `mode`/`format`, and return `(argv, out_name)` for an
/// input file. `out_name` is `out.<ext>` for the chosen format. Single source
/// shared by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_trim_audio(
    in_name: &str,
    start: f64,
    end: f64,
    mode: &str,
    format: &str,
    fade: bool,
) -> Result<(Vec<String>, String), String> {
    if !start.is_finite() || !(0.0..=MAX_TIME_S).contains(&start) {
        return Err(format!(
            "start must be between 0 and {MAX_TIME_S} seconds, got {start}"
        ));
    }
    if !end.is_finite() || !(0.0..=MAX_TIME_S).contains(&end) {
        return Err(format!(
            "end must be between 0 and {MAX_TIME_S} seconds (0 or empty = end of track), got {end}"
        ));
    }
    let to_end = is_to_end(end);
    if !to_end && end <= start {
        return Err(format!(
            "end must be greater than start, or 0/empty for \"to the end of the track\" \
             (start={start}, got end={end})"
        ));
    }
    let mode = parse_mode(mode)?;
    if to_end && start == 0.0 {
        return Err(match (mode, fade) {
            (Mode::Keep, true) => "start 0 with no end selects the whole file — trim-audio \
                                   won't re-encode it just to fade the edges; use the \
                                   audio-fade tool for whole-track fades, or set start/end \
                                   to trim"
                .to_string(),
            (Mode::Keep, false) => "start 0 with no end selects the whole file — nothing to \
                                    trim; set start (e.g. 2 drops the first two seconds) \
                                    and/or end (e.g. 30 cuts everything after 0:30)"
                .to_string(),
            (Mode::Remove, _) => "removing from 0 with no end would delete the whole file — \
                                  set a later start or an explicit end"
                .to_string(),
        });
    }
    let fmt = parse_format(format)?;
    if fade && mode == Mode::Remove {
        return Err(
            "fade only applies to keep mode (remove mode's output length isn't known up front)"
                .to_string(),
        );
    }
    let out_name = format!("out.{}", fmt.ext());
    Ok((
        build_argv(in_name, &out_name, mode, start, end, fade, fmt),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_argv_order_and_values() {
        let (argv, out) = plan_trim_audio("in.mp3", 0.5, 1.5, "keep", "mp3", false).unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "atrim=start=0.5:end=1.5,asetpts=N/SR/TB",
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
    fn remove_filter_quotes_the_between_expression() {
        let f = build_filter(Mode::Remove, 1.0, 2.0, false);
        assert_eq!(f, "aselect='not(between(t,1,2))',asetpts=N/SR/TB");
    }

    #[test]
    fn keep_fade_appends_afade_pair_with_computed_offsets() {
        // dur = 1.0 → fd = min(0.5, 0.25) = 0.25, fade-out starts at 0.75.
        let f = build_filter(Mode::Keep, 0.5, 1.5, true);
        assert_eq!(
            f,
            "atrim=start=0.5:end=1.5,asetpts=N/SR/TB,afade=t=in:st=0:d=0.25,afade=t=out:st=0.75:d=0.25"
        );
    }

    #[test]
    fn keep_fade_uses_full_half_second_on_long_clips() {
        // dur = 10 → fd = 0.5, fade-out starts at 9.5.
        let f = build_filter(Mode::Keep, 0.0, 10.0, true);
        assert!(f.ends_with(",afade=t=in:st=0:d=0.5,afade=t=out:st=9.5:d=0.5"));
    }

    #[test]
    fn format_selects_codec_and_out_name() {
        let (argv, out) = plan_trim_audio("in.wav", 0.0, 2.0, "keep", "wav", false).unwrap();
        assert_eq!(out, "out.wav");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "pcm_s16le"));
        let (argv, out) = plan_trim_audio("in.mp3", 0.0, 2.0, "keep", "flac", false).unwrap();
        assert_eq!(out, "out.flac");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "flac"));
        let (argv, out) = plan_trim_audio("in.mp3", 0.0, 2.0, "keep", "m4a", false).unwrap();
        assert_eq!(out, "out.m4a");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        let (argv, out) = plan_trim_audio("in.mp3", 0.0, 2.0, "keep", "ogg", false).unwrap();
        assert_eq!(out, "out.ogg");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libvorbis"));
    }

    #[test]
    fn empty_mode_and_format_default_to_keep_mp3() {
        let (argv, out) = plan_trim_audio("in.ogg", 1.0, 3.0, "", "", false).unwrap();
        assert_eq!(out, "out.mp3");
        assert!(argv.iter().any(|a| a.starts_with("atrim=")));
    }

    #[test]
    fn rejects_negative_or_non_finite_start() {
        assert!(plan_trim_audio("a.mp3", -1.0, 2.0, "keep", "mp3", false).is_err());
        assert!(plan_trim_audio("a.mp3", f64::NAN, 2.0, "keep", "mp3", false).is_err());
    }

    #[test]
    fn rejects_end_not_greater_than_start() {
        assert!(plan_trim_audio("a.mp3", 2.0, 2.0, "keep", "mp3", false).is_err());
        assert!(plan_trim_audio("a.mp3", 2.0, 1.0, "keep", "mp3", false).is_err());
        assert!(plan_trim_audio("a.mp3", 0.0, f64::INFINITY, "keep", "mp3", false).is_err());
        assert!(plan_trim_audio("a.mp3", 0.0, -1.0, "keep", "mp3", false).is_err());
        // The bad-end error teaches the to-end escape hatch.
        let err = plan_trim_audio("a.mp3", 2.0, 1.0, "keep", "mp3", false).unwrap_err();
        assert!(err.contains("to the end of the track"), "{err}");
    }

    #[test]
    fn end_zero_means_to_end_of_track() {
        // keep: atrim without an end bound.
        let (argv, _) = plan_trim_audio("in.mp3", 1.0, 0.0, "keep", "mp3", false).unwrap();
        assert!(
            argv.iter().any(|a| a == "atrim=start=1,asetpts=N/SR/TB"),
            "{argv:?}"
        );
        // remove: removing [start, EOF] keeps [0, start].
        let f = build_filter(Mode::Remove, 1.5, 0.0, false);
        assert_eq!(f, "atrim=end=1.5,asetpts=N/SR/TB");
    }

    #[test]
    fn to_end_fade_uses_duration_free_areverse_trick() {
        // Flat TO_END_FADE_SECS ramps; the chain itself is audio-fade-core's
        // (shared implementation, no drift between the two blocks).
        let f = build_filter(Mode::Keep, 1.0, 0.0, true);
        assert_eq!(
            f,
            "atrim=start=1,asetpts=N/SR/TB,afade=t=in:st=0:d=0.15,areverse,afade=t=in:st=0:d=0.15,areverse"
        );
        // No t=out / st=<offset> term may appear — the duration is unknown.
        assert!(!f.contains("t=out"), "{f}");
        assert_eq!(
            f.split_once("asetpts=N/SR/TB,").unwrap().1,
            gizza_ai_audio_fade_core::build_filter(TO_END_FADE_SECS, TO_END_FADE_SECS).unwrap()
        );
    }

    #[test]
    fn whole_file_requests_get_guiding_errors() {
        let err = plan_trim_audio("a.mp3", 0.0, 0.0, "keep", "mp3", false).unwrap_err();
        assert!(err.contains("whole file"), "{err}");
        assert!(err.contains("set start"), "{err}");
        let err = plan_trim_audio("a.mp3", 0.0, 0.0, "remove", "mp3", false).unwrap_err();
        assert!(err.contains("delete the whole file"), "{err}");
        // With fade the request isn't an identity op — the message must not
        // claim "nothing to trim"; it points at the dedicated audio-fade tool.
        let err = plan_trim_audio("a.mp3", 0.0, 0.0, "keep", "mp3", true).unwrap_err();
        assert!(err.contains("audio-fade"), "{err}");
        assert!(!err.contains("nothing to trim"), "{err}");
    }

    #[test]
    fn absurd_times_get_a_real_message_not_ffmpeg_overflow() {
        // ffmpeg's atrim duration parser overflows near 9.2e18 µs; huge finite
        // values must be caught by our validation, not a cryptic parser error.
        let err = plan_trim_audio("a.mp3", 0.0, 1e20, "keep", "mp3", false).unwrap_err();
        assert!(err.contains("86400"), "{err}");
        let err = plan_trim_audio("a.mp3", 1e20, 0.0, "keep", "mp3", false).unwrap_err();
        assert!(err.contains("86400"), "{err}");
        // The bound itself is generous and inclusive.
        assert!(plan_trim_audio("a.mp3", 0.0, 86_400.0, "keep", "mp3", false).is_ok());
    }

    #[test]
    fn to_end_fade_still_rejected_in_remove_mode() {
        let err = plan_trim_audio("a.mp3", 1.0, 0.0, "remove", "mp3", true).unwrap_err();
        assert!(err.contains("keep mode"), "{err}");
    }

    #[test]
    fn rejects_fade_in_remove_mode() {
        let err = plan_trim_audio("a.mp3", 0.0, 1.0, "remove", "mp3", true).unwrap_err();
        assert!(err.contains("keep mode"));
    }

    #[test]
    fn rejects_unknown_mode_and_format() {
        assert!(plan_trim_audio("a.mp3", 0.0, 1.0, "shuffle", "mp3", false).is_err());
        assert!(plan_trim_audio("a.mp3", 0.0, 1.0, "keep", "aiff", false).is_err());
    }

    #[test]
    fn format_ext_and_mime_pairs() {
        assert_eq!(Format::Mp3.mime(), "audio/mpeg");
        assert_eq!(Format::Wav.mime(), "audio/wav");
        assert_eq!(Format::Ogg.mime(), "audio/ogg");
        assert_eq!(Format::Flac.mime(), "audio/flac");
        assert_eq!(Format::M4a.mime(), "audio/mp4");
        assert_eq!(Format::M4a.ext(), "m4a");
    }

    #[test]
    fn parse_mode_known_and_default() {
        assert_eq!(parse_mode("").unwrap(), Mode::Keep);
        assert_eq!(parse_mode("KEEP").unwrap(), Mode::Keep);
        assert_eq!(parse_mode("remove").unwrap(), Mode::Remove);
    }
}
