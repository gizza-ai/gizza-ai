//! gizza-ai/audio-ringtone core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Cuts a `[start, end]` slice of a song (≤ 40 s — iPhone's ringtone cap;
//! 30 s by default), optionally normalizes loudness to -14 LUFS so the tone
//! rings loud on a phone speaker, adds short edge fades, and encodes as
//! `m4r` (AAC in the iPod/MP4 container iPhones require — ffmpeg doesn't know
//! the `.m4r` extension, so `-f ipod` is passed explicitly) or `mp3`.
//!
//! Filter order matters: trim first (so loudnorm measures only the slice),
//! then `loudnorm`, then the fades (after loudnorm, so it can't boost the
//! faded edges back up). The fade-out uses the duration-free
//! `areverse,afade=t=in,areverse` trick: the argv is built BEFORE the input
//! is decoded, so if the source is shorter than the selection the real clip
//! length is unknowable here — fading-in a reversed stream needs no duration
//! at all, and the ≤ 40 s cap keeps the two full-buffer reverses cheap.
//! `loudnorm` internally resamples to 192 kHz, so `-ar 44100` is added when
//! normalizing to bring the output back to a standard rate. `-vn` drops
//! attached-picture streams (album art).

/// Output ringtone formats.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    M4r,
    Mp3,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::M4r => "m4r",
            Format::Mp3 => "mp3",
        }
    }

    /// IANA media type for the produced file (`.m4r` is AAC audio in an
    /// MP4 container, so `audio/mp4`).
    pub fn mime(self) -> &'static str {
        match self {
            Format::M4r => "audio/mp4",
            Format::Mp3 => "audio/mpeg",
        }
    }

    /// Encoder argv fragment. Both formats encode at 192 kbps; m4r also needs
    /// the explicit `-f ipod` muxer because ffmpeg can't infer it from `.m4r`.
    fn codec_args(self) -> Vec<String> {
        match self {
            Format::M4r => vec![
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
                "-f".into(),
                "ipod".into(),
            ],
            Format::Mp3 => vec![
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                "192k".into(),
            ],
        }
    }
}

/// Parse the user-facing format string. Empty defaults to m4r (iPhone).
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "m4r" => Ok(Format::M4r),
        "mp3" => Ok(Format::Mp3),
        other => Err(format!("format {other:?} not supported (m4r|mp3)")),
    }
}

/// Ringtone length (seconds) when `end` is 0/omitted: `start + 30`.
pub const DEFAULT_LEN_S: f64 = 30.0;
/// Longest accepted ringtone — iPhone rejects ringtones over 40 seconds.
pub const MAX_LEN_S: f64 = 40.0;
/// Shortest useful ringtone.
pub const MIN_LEN_S: f64 = 1.0;
/// Longest accepted fade per side, in seconds.
pub const MAX_FADE_S: f64 = 5.0;
/// Default fade per side — a short anti-click/anti-abrupt-cut ramp.
pub const DEFAULT_FADE_S: f64 = 0.5;
/// EBU R128 one-pass loudness target: -14 LUFS integrated (streaming-loud),
/// -1.5 dBTP true peak, LRA 11 — same shape as the audio-normalize block.
pub const LOUDNORM_FILTER: &str = "loudnorm=I=-14:TP=-1.5:LRA=11";

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

/// True when `end` means "start + 30 s" (0/omitted — matches the page's empty
/// field and the waveform's cleared selection).
pub fn is_default_end(end: f64) -> bool {
    end <= 0.0
}

/// Resolve the effective end time: 0/omitted → `start + DEFAULT_LEN_S`.
pub fn resolve_end(start: f64, end: f64) -> f64 {
    if is_default_end(end) {
        start + DEFAULT_LEN_S
    } else {
        end
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

/// Build the `-af` chain: trim → optional loudnorm → optional fades.
pub fn build_filter(
    start: f64,
    end: f64,
    fade_in: f64,
    fade_out: f64,
    normalize: bool,
) -> Result<String, String> {
    if !start.is_finite() || start < 0.0 {
        return Err(format!("start must be 0 or more seconds, got {start}"));
    }
    if !end.is_finite() || end < 0.0 {
        return Err(format!(
            "end must be 0 (= start + {}) or a positive number of seconds, got {end}",
            fmt_num(DEFAULT_LEN_S)
        ));
    }
    validate_fade("fade_in", fade_in)?;
    validate_fade("fade_out", fade_out)?;
    let resolved_end = resolve_end(start, end);
    let len = resolved_end - start;
    if len < MIN_LEN_S {
        return Err(format!(
            "selection is too short ({}s) — a ringtone needs at least {} second between start and end",
            fmt_num(len),
            fmt_num(MIN_LEN_S)
        ));
    }
    if len > MAX_LEN_S {
        return Err(format!(
            "selection is {}s long — iPhone caps ringtones at {} seconds, so keep end - start at or under that",
            fmt_num(len),
            fmt_num(MAX_LEN_S)
        ));
    }
    let mut stages = vec![format!(
        "atrim=start={}:end={},asetpts=N/SR/TB",
        fmt_num(start),
        fmt_num(resolved_end)
    )];
    if normalize {
        stages.push(LOUDNORM_FILTER.to_string());
    }
    if fade_in > 0.0 {
        stages.push(format!("afade=t=in:st=0:d={}", fmt_num(fade_in)));
    }
    if fade_out > 0.0 {
        stages.push(format!(
            "areverse,afade=t=in:st=0:d={},areverse",
            fmt_num(fade_out)
        ));
    }
    Ok(stages.join(","))
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to cut `in_name` into
/// `out_name`. `-ar 44100` is added when normalizing because loudnorm
/// internally resamples to 192 kHz.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    filter: &str,
    normalize: bool,
    format: Format,
) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        filter.to_string(),
    ];
    if normalize {
        argv.push("-ar".to_string());
        argv.push("44100".to_string());
    }
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate everything, parse `format`, and return `(argv, out_name)`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan_ringtone(
    in_name: &str,
    start: f64,
    end: f64,
    fade_in: f64,
    fade_out: f64,
    normalize: bool,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let filter = build_filter(start, end, fade_in, fade_out, normalize)?;
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((
        build_argv(in_name, &out_name, &filter, normalize, fmt),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_run_argv_order_and_values() {
        // start 0, end omitted (0) → 30 s slice; normalize on; 0.5 s fades; m4r.
        let (argv, out) = plan_ringtone("in.mp3", 0.0, 0.0, 0.5, 0.5, true, "m4r").unwrap();
        assert_eq!(out, "out.m4r");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "atrim=start=0:end=30,asetpts=N/SR/TB,\
                 loudnorm=I=-14:TP=-1.5:LRA=11,\
                 afade=t=in:st=0:d=0.5,\
                 areverse,afade=t=in:st=0:d=0.5,areverse",
                "-ar",
                "44100",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "-f",
                "ipod",
                "out.m4r",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn mp3_without_normalize_or_fades_is_a_bare_trim() {
        let (argv, out) = plan_ringtone("in.wav", 12.0, 42.0, 0.0, 0.0, false, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.wav",
                "-vn",
                "-af",
                "atrim=start=12:end=42,asetpts=N/SR/TB",
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
    fn m4r_always_forces_the_ipod_muxer() {
        // ffmpeg can't infer a muxer from the non-standard `.m4r` extension —
        // the argv must carry `-f ipod` or the whole run fails at mux time.
        let (argv, _) = plan_ringtone("in.mp3", 0.0, 0.0, 0.0, 0.0, false, "m4r").unwrap();
        assert!(
            argv.windows(2).any(|w| w[0] == "-f" && w[1] == "ipod"),
            "{argv:?}"
        );
    }

    #[test]
    fn end_zero_or_omitted_means_start_plus_30() {
        assert_eq!(resolve_end(45.5, 0.0), 75.5);
        assert!(is_default_end(0.0));
        assert!(!is_default_end(20.0));
        let (argv, _) = plan_ringtone("in.mp3", 45.5, 0.0, 0.0, 0.0, false, "mp3").unwrap();
        assert!(
            argv.iter().any(|a| a.contains("atrim=start=45.5:end=75.5")),
            "{argv:?}"
        );
    }

    #[test]
    fn fade_out_never_needs_a_start_time() {
        // The areverse trick: no `t=out`/`st=<duration - d>` may appear —
        // the real clip length is unknown at argv-build time (short sources).
        let f = build_filter(0.0, 10.0, 0.0, 2.0, false).unwrap();
        assert!(!f.contains("t=out"), "{f}");
        assert!(f.ends_with("areverse,afade=t=in:st=0:d=2,areverse"), "{f}");
    }

    #[test]
    fn normalize_stage_sits_between_trim_and_fades() {
        // Fades AFTER loudnorm — the other order lets loudnorm boost the
        // faded edges back up.
        let f = build_filter(1.0, 11.0, 0.5, 0.5, true).unwrap();
        let trim = f.find("atrim").unwrap();
        let norm = f.find("loudnorm").unwrap();
        let fade = f.find("afade").unwrap();
        assert!(trim < norm && norm < fade, "{f}");
    }

    #[test]
    fn cap_boundary_40s_ok_over_rejected() {
        // Exactly at the iPhone cap: fine.
        assert!(plan_ringtone("a.mp3", 5.0, 45.0, 0.0, 0.0, false, "m4r").is_ok());
        // One over: rejected, and the error explains the cap.
        let err = plan_ringtone("a.mp3", 5.0, 45.1, 0.0, 0.0, false, "m4r").unwrap_err();
        assert!(err.contains("40"), "{err}");
        assert!(err.contains("iPhone"), "{err}");
    }

    #[test]
    fn too_short_negative_or_non_finite_selections_are_rejected() {
        // Under the 1 s minimum.
        let err = plan_ringtone("a.mp3", 5.0, 5.5, 0.0, 0.0, false, "mp3").unwrap_err();
        assert!(err.contains("too short"), "{err}");
        // end before start (explicit) is also "too short".
        assert!(plan_ringtone("a.mp3", 10.0, 5.0, 0.0, 0.0, false, "mp3").is_err());
        // Exactly the 1 s minimum is fine.
        assert!(plan_ringtone("a.mp3", 5.0, 6.0, 0.0, 0.0, false, "mp3").is_ok());
        assert!(plan_ringtone("a.mp3", -1.0, 0.0, 0.0, 0.0, false, "mp3").is_err());
        assert!(plan_ringtone("a.mp3", f64::NAN, 0.0, 0.0, 0.0, false, "mp3").is_err());
        assert!(plan_ringtone("a.mp3", 0.0, f64::INFINITY, 0.0, 0.0, false, "mp3").is_err());
    }

    #[test]
    fn out_of_range_fades_are_rejected_boundaries_ok() {
        assert!(plan_ringtone("a.mp3", 0.0, 0.0, 5.1, 0.0, false, "mp3").is_err());
        let err = plan_ringtone("a.mp3", 0.0, 0.0, 0.0, -0.5, false, "mp3").unwrap_err();
        assert!(err.contains("fade_out"), "names the offending side: {err}");
        assert!(plan_ringtone("a.mp3", 0.0, 0.0, 5.0, 5.0, false, "mp3").is_ok());
        assert!(plan_ringtone("a.mp3", 0.0, 0.0, f64::NAN, 0.0, false, "mp3").is_err());
    }

    #[test]
    fn parse_format_defaults_empty_to_m4r() {
        assert_eq!(parse_format("").unwrap(), Format::M4r);
        assert_eq!(parse_format("M4R").unwrap(), Format::M4r);
        assert_eq!(parse_format("mp3").unwrap(), Format::Mp3);
        assert!(parse_format("wav").is_err());
    }

    #[test]
    fn format_ext_and_mime_pairs() {
        assert_eq!(Format::M4r.ext(), "m4r");
        assert_eq!(Format::M4r.mime(), "audio/mp4");
        assert_eq!(Format::Mp3.ext(), "mp3");
        assert_eq!(Format::Mp3.mime(), "audio/mpeg");
    }

    #[test]
    fn argv_always_drops_video_streams_and_only_resamples_when_normalizing() {
        let (argv, _) = plan_ringtone("in.mp3", 0.0, 0.0, 0.5, 0.5, true, "m4r").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
        assert!(argv.iter().any(|a| a == "44100"));
        let (argv, _) = plan_ringtone("in.mp3", 0.0, 0.0, 0.5, 0.5, false, "m4r").unwrap();
        assert!(!argv.iter().any(|a| a == "-ar"), "{argv:?}");
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(30.0), "30");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(45.25), "45.25");
    }
}
