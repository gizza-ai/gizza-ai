//! gizza-ai/video-fade core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Ramps a clip up from (and down to) a solid colour — black by default — at the
//! start and end, and ramps the audio out of / into silence over the same spans.
//! The picture ramp is ffmpeg's `fade` filter, the sound ramp its `afade`
//! sibling; both take an absolute start time, so a fade-out needs the clip's
//! length. The argv is built BEFORE the file is decoded (the page's wasm builder
//! never sees the bytes), so the caller supplies `duration` — the clip's exact
//! length in seconds — whenever `fade_out` is greater than 0.
//!
//! Fading the picture rewrites pixels, so that path re-encodes to H.264/AAC in
//! an MP4. A sound-only fade leaves the picture bit-for-bit identical
//! (`-c:v copy`) and keeps the input container.

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

/// Longest accepted fade per side, in seconds.
pub const MAX_FADE_S: f64 = 30.0;
/// Longest accepted clip length, in seconds (10 hours).
pub const MAX_DURATION_S: f64 = 36_000.0;
/// Longest accepted `color` value, in characters.
pub const MAX_COLOR_LEN: usize = 32;

/// Which streams the fade is applied to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Streams {
    /// Picture and sound (default).
    Both,
    /// Picture only — the audio track is left at full level.
    Video,
    /// Sound only — the picture is stream-copied untouched.
    Audio,
}

impl Streams {
    /// True when the picture is ramped (and therefore re-encoded).
    pub fn fades_video(self) -> bool {
        matches!(self, Self::Both | Self::Video)
    }
    /// True when the sound is ramped.
    pub fn fades_audio(self) -> bool {
        matches!(self, Self::Both | Self::Audio)
    }
}

/// Output quality for the re-encoded picture, as an x264 CRF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    High,
    Balanced,
    Small,
}

impl Quality {
    pub fn crf(self) -> &'static str {
        match self {
            Self::High => "18",
            Self::Balanced => "23",
            Self::Small => "28",
        }
    }
}

pub fn parse_streams(value: &str) -> Result<Streams, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "both" => Ok(Streams::Both),
        "video" => Ok(Streams::Video),
        "audio" => Ok(Streams::Audio),
        other => Err(format!(
            "streams {other:?} not supported; expected both, video or audio"
        )),
    }
}

pub fn parse_quality(value: &str) -> Result<Quality, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "high" => Ok(Quality::High),
        "" | "balanced" => Ok(Quality::Balanced),
        "small" => Ok(Quality::Small),
        other => Err(format!(
            "quality {other:?} not supported; expected high, balanced or small"
        )),
    }
}

/// Audio encoder for the kept output container. WebM can only hold Opus/Vorbis,
/// so AAC is invalid there; every other container we keep accepts AAC.
pub fn audio_codec(out_ext: &str) -> &'static str {
    if out_ext.eq_ignore_ascii_case("webm") {
        "libopus"
    } else {
        "aac"
    }
}

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

/// Accept the colour spellings ffmpeg understands (`black`, `#101820`,
/// `0x101820`, `white@0.5`) while rejecting anything that could break out of the
/// filter argument — `,` `:` `[` `]` `'` and whitespace all change the meaning
/// of a filtergraph, so a colour containing them is an error, never an escape.
pub fn validate_color(color: &str) -> Result<String, String> {
    let c = color.trim();
    if c.is_empty() {
        return Ok("black".to_string());
    }
    if c.len() > MAX_COLOR_LEN {
        return Err(format!(
            "color must be at most {MAX_COLOR_LEN} characters, got {}",
            c.len()
        ));
    }
    if let Some(bad) = c
        .chars()
        .find(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '#' | '@' | '.' | '_' | '-')))
    {
        return Err(format!(
            "color contains the unsupported character {bad:?} — use a colour name (black, white), \
             a hex value (#101820) or name@alpha (black@0.8)"
        ));
    }
    Ok(c.to_string())
}

/// Every fade length the plan needs, already validated against each other.
#[derive(Debug, Clone, Copy)]
struct Timing {
    fade_in: f64,
    fade_out: f64,
    /// Absolute start time of the fade-out; only meaningful when `fade_out` > 0.
    out_start: f64,
}

fn validate_timing(fade_in: f64, fade_out: f64, duration: f64) -> Result<Timing, String> {
    validate_fade("fade_in", fade_in)?;
    validate_fade("fade_out", fade_out)?;
    if fade_in <= 0.0 && fade_out <= 0.0 {
        return Err(
            "both fades are 0 — nothing to change; set fade_in and/or fade_out in seconds \
             (e.g. 1.5 for a gentle fade)"
                .to_string(),
        );
    }
    let mut out_start = 0.0;
    if fade_out > 0.0 {
        if !duration.is_finite() || duration <= 0.0 {
            return Err(format!(
                "duration is required when fade_out is greater than 0 — enter the clip's exact \
                 length in seconds (e.g. 12.5), got {duration}"
            ));
        }
        if duration > MAX_DURATION_S {
            return Err(format!(
                "duration must be at most {MAX_DURATION_S} seconds, got {duration}"
            ));
        }
        if fade_in + fade_out > duration {
            return Err(format!(
                "fade_in + fade_out ({}) is longer than the clip ({} s) — shorten the fades or \
                 correct the duration",
                fmt_num(fade_in + fade_out),
                fmt_num(duration)
            ));
        }
        out_start = duration - fade_out;
    }
    Ok(Timing {
        fade_in,
        fade_out,
        out_start,
    })
}

/// Picture ramp: `fade=t=in` from `color` at the head, `fade=t=out` to `color`
/// at the tail. Stages are skipped when their side is 0.
fn video_filter(t: Timing, color: &str) -> String {
    let mut stages = Vec::new();
    if t.fade_in > 0.0 {
        stages.push(format!(
            "fade=t=in:st=0:d={}:color={color}",
            fmt_num(t.fade_in)
        ));
    }
    if t.fade_out > 0.0 {
        stages.push(format!(
            "fade=t=out:st={}:d={}:color={color}",
            fmt_num(t.out_start),
            fmt_num(t.fade_out)
        ));
    }
    stages.join(",")
}

/// Sound ramp mirroring [`video_filter`], from and to silence.
fn audio_filter(t: Timing) -> String {
    let mut stages = Vec::new();
    if t.fade_in > 0.0 {
        stages.push(format!("afade=t=in:st=0:d={}", fmt_num(t.fade_in)));
    }
    if t.fade_out > 0.0 {
        stages.push(format!(
            "afade=t=out:st={}:d={}",
            fmt_num(t.out_start),
            fmt_num(t.fade_out)
        ));
    }
    stages.join(",")
}

/// Validate everything and return `(argv, out_name)` — the ffmpeg argv without a
/// leading `ffmpeg`, plus the output filename.
///
/// `duration` is the clip's exact length in seconds and is only needed when
/// `fade_out` > 0 (pass 0 otherwise). A picture fade re-encodes to H.264/AAC MP4;
/// `streams = "audio"` copies the picture and keeps the input container.
pub fn plan(
    in_name: &str,
    fade_in: f64,
    fade_out: f64,
    duration: f64,
    streams: &str,
    color: &str,
    quality: &str,
) -> Result<(Vec<String>, String), String> {
    let timing = validate_timing(fade_in, fade_out, duration)?;
    let streams = parse_streams(streams)?;
    let quality = parse_quality(quality)?;
    let color = validate_color(color)?;

    let mut argv = vec!["-i".to_string(), in_name.to_string()];
    let out_name;

    if streams.fades_video() {
        // Ramped pixels must be re-encoded; H.264 in MP4 is the one container
        // that holds every input we accept (webm's VP9/Opus cannot).
        out_name = "out.mp4".to_string();
        argv.extend(["-vf".to_string(), video_filter(timing, &color)]);
        argv.extend([
            "-c:v".to_string(),
            "libx264".to_string(),
            "-crf".to_string(),
            quality.crf().to_string(),
            "-preset".to_string(),
            "veryfast".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
        ]);
        if streams.fades_audio() {
            argv.extend(["-af".to_string(), audio_filter(timing)]);
        }
        argv.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
        ]);
    } else {
        // Sound only: the picture is copied byte-for-byte and the container
        // survives, so only the audio codec has to match it.
        let ext = copy_out_ext(in_name);
        out_name = format!("out.{ext}");
        argv.extend(["-c:v".to_string(), "copy".to_string()]);
        argv.extend(["-af".to_string(), audio_filter(timing)]);
        argv.extend(["-c:a".to_string(), audio_codec(ext).to_string()]);
    }

    argv.push(out_name.clone());
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn both_sides_both_streams_argv() {
        let (argv, out) = plan("in.mp4", 1.0, 2.0, 10.0, "both", "black", "balanced").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            s(&[
                "-i",
                "in.mp4",
                "-vf",
                "fade=t=in:st=0:d=1:color=black,fade=t=out:st=8:d=2:color=black",
                "-c:v",
                "libx264",
                "-crf",
                "23",
                "-preset",
                "veryfast",
                "-pix_fmt",
                "yuv420p",
                "-af",
                "afade=t=in:st=0:d=1,afade=t=out:st=8:d=2",
                "-c:a",
                "aac",
                "-movflags",
                "+faststart",
                "out.mp4",
            ])
        );
    }

    #[test]
    fn fade_out_start_is_duration_minus_fade() {
        let (argv, _) = plan("in.mp4", 0.0, 1.5, 4.25, "video", "black", "balanced").unwrap();
        let vf = argv[argv.iter().position(|a| a == "-vf").unwrap() + 1].clone();
        assert_eq!(vf, "fade=t=out:st=2.75:d=1.5:color=black");
        // Picture-only: no audio ramp is added.
        assert!(!argv.iter().any(|a| a == "-af"));
    }

    #[test]
    fn audio_only_copies_the_picture_and_keeps_the_container() {
        let (argv, out) = plan("clip.webm", 2.0, 0.0, 0.0, "audio", "black", "high").unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(!argv.iter().any(|a| a == "-vf"));
        // Never drops a stream.
        assert!(!argv.iter().any(|a| a == "-vn" || a == "-an"));
    }

    #[test]
    fn quality_maps_to_crf() {
        for (q, crf) in [("high", "18"), ("balanced", "23"), ("small", "28")] {
            let (argv, _) = plan("in.mp4", 1.0, 0.0, 0.0, "both", "black", q).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-crf" && w[1] == crf),
                "{q} -> {crf}: {argv:?}"
            );
        }
    }

    #[test]
    fn color_reaches_both_fade_stages() {
        let (argv, _) = plan("in.mp4", 1.0, 1.0, 5.0, "video", "#ffffff", "balanced").unwrap();
        let vf = argv[argv.iter().position(|a| a == "-vf").unwrap() + 1].clone();
        assert_eq!(
            vf,
            "fade=t=in:st=0:d=1:color=#ffffff,fade=t=out:st=4:d=1:color=#ffffff"
        );
    }

    #[test]
    fn video_fade_always_normalizes_to_mp4() {
        for ext in ["webm", "mkv", "mov", "avi"] {
            let (_, out) = plan(&format!("c.{ext}"), 1.0, 0.0, 0.0, "both", "black", "small")
                .unwrap();
            assert_eq!(out, "out.mp4", "{ext}");
        }
    }

    #[test]
    fn both_zero_is_rejected_as_a_no_op() {
        let err = plan("in.mp4", 0.0, 0.0, 5.0, "both", "black", "balanced").unwrap_err();
        assert!(err.contains("nothing to change"), "{err}");
    }

    #[test]
    fn fade_out_without_duration_is_rejected() {
        let err = plan("in.mp4", 0.0, 2.0, 0.0, "both", "black", "balanced").unwrap_err();
        assert!(err.contains("duration is required"), "{err}");
        // fade-in only never needs the duration.
        assert!(plan("in.mp4", 2.0, 0.0, 0.0, "both", "black", "balanced").is_ok());
    }

    #[test]
    fn fades_longer_than_the_clip_are_rejected() {
        let err = plan("in.mp4", 3.0, 3.0, 5.0, "both", "black", "balanced").unwrap_err();
        assert!(err.contains("longer than the clip"), "{err}");
        // Exactly filling the clip is allowed.
        assert!(plan("in.mp4", 2.5, 2.5, 5.0, "both", "black", "balanced").is_ok());
    }

    #[test]
    fn out_of_range_or_non_finite_fades_are_rejected() {
        assert!(plan("a.mp4", -1.0, 0.0, 0.0, "both", "black", "balanced").is_err());
        assert!(plan("a.mp4", 31.0, 0.0, 0.0, "both", "black", "balanced").is_err());
        assert!(plan("a.mp4", f64::NAN, 0.0, 0.0, "both", "black", "balanced").is_err());
        assert!(plan("a.mp4", 0.0, f64::INFINITY, 60.0, "both", "black", "balanced").is_err());
        // Boundary is valid.
        assert!(plan("a.mp4", 30.0, 0.0, 0.0, "both", "black", "balanced").is_ok());
        let err = plan("a.mp4", 0.0, 31.0, 60.0, "both", "black", "balanced").unwrap_err();
        assert!(err.contains("fade_out"), "names the offending side: {err}");
        let err =
            plan("a.mp4", 1.0, 1.0, MAX_DURATION_S + 1.0, "both", "black", "balanced").unwrap_err();
        assert!(err.contains("duration must be at most"), "{err}");
    }

    #[test]
    fn unknown_enum_values_are_rejected_with_the_allowed_list() {
        let err = plan("a.mp4", 1.0, 0.0, 0.0, "picture", "black", "balanced").unwrap_err();
        assert!(err.contains("expected both, video or audio"), "{err}");
        let err = plan("a.mp4", 1.0, 0.0, 0.0, "both", "black", "lossless").unwrap_err();
        assert!(err.contains("expected high, balanced or small"), "{err}");
    }

    #[test]
    fn filtergraph_metacharacters_in_color_are_rejected() {
        for bad in ["black,crop=1:1", "bl ack", "red:2", "white'"] {
            assert!(
                validate_color(bad).is_err(),
                "{bad:?} must not reach the filtergraph"
            );
        }
        assert_eq!(validate_color("").unwrap(), "black");
        assert_eq!(validate_color(" black ").unwrap(), "black");
        assert_eq!(validate_color("white@0.5").unwrap(), "white@0.5");
        assert_eq!(validate_color("0x101820").unwrap(), "0x101820");
        assert!(validate_color(&"a".repeat(MAX_COLOR_LEN + 1)).is_err());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(3.0), "3");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(2.75), "2.75");
    }
}
